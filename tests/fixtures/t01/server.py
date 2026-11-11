"""Single-process T01 fixture; never imported or bundled by the production app.

Only registered generated media can be read. The random port and 256-bit cap are
per run. Request records omit queries and headers to keep capabilities out of logs.
"""
from collections import Counter
import hashlib
import hmac
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import queue
import re
import secrets
import threading
from urllib.parse import parse_qs, urlsplit

ORIGINS = {"tauri://localhost", "http://tauri.localhost"}
TYPES = {".mp4": "video/mp4", ".m3u8": "application/vnd.apple.mpegurl",
         ".ts": "video/mp2t", ".vtt": "text/vtt; charset=utf-8"}


def byte_range(value, size):
    if value is None:
        return None
    if size <= 0:
        raise ValueError("empty resource")
    match = re.fullmatch(r"bytes=(\d*)-(\d*)", value)
    if not match or not any(match.groups()):
        raise ValueError("invalid or multiple Range")
    first, last = match.groups()
    if not first:
        count = int(last)
        if count <= 0:
            raise ValueError("empty suffix")
        return max(0, size - count), size - 1
    start, end = int(first), min(int(last), size - 1) if last else size - 1
    if start >= size or end < start:
        raise ValueError("unsatisfiable Range")
    return start, end


class Fixture:
    def __init__(self, media, invoke=None):
        self.media = Path(media)
        self.invoke = invoke
        self.cap = secrets.token_hex(32)
        self.commands = queue.Queue()
        self.results = queue.Queue()
        self.samples = queue.Queue()
        self.records = []
        self.active = 0
        self.lock = threading.Lock()
        self.files = {f"/media/{p.name}": p for p in self.media.iterdir() if p.suffix in TYPES}
        self.etags = {}
        for name, path in self.files.items():
            with path.open("rb") as stream:
                self.etags[name] = '"' + hashlib.file_digest(stream, "sha256").hexdigest() + '"'
        fixture = self

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *_args):
                pass

            def response(self, status, body=b"", headers=None):
                self.send_response(status)
                self.send_header("Content-Length", str(len(body)))
                self.send_header("Cache-Control", "no-store")
                self.send_header("X-Content-Type-Options", "nosniff")
                self.send_header("Connection", "close")
                self.close_connection = True
                if self.headers.get("Origin") in ORIGINS:
                    self.send_header("Access-Control-Allow-Origin", self.headers["Origin"])
                    self.send_header("Vary", "Origin")
                    self.send_header("Access-Control-Expose-Headers", "Content-Range, Accept-Ranges, ETag")
                for key, value in (headers or {}).items():
                    self.send_header(key, value)
                self.end_headers()
                if self.command != "HEAD":
                    self.wfile.write(body)

            def allowed(self, cap=True):
                parsed = urlsplit(self.path)
                if self.headers.get("Host") != f"127.0.0.1:{fixture.port}":
                    self.response(421)
                    return False
                if self.headers.get("Origin") not in ORIGINS | {None}:
                    self.response(403)
                    return False
                query = parse_qs(parsed.query, keep_blank_values=True)
                if cap and (set(query) != {"cap"} or len(query["cap"]) != 1
                            or not hmac.compare_digest(query["cap"][0].encode(), fixture.cap.encode())):
                    self.response(403)
                    return False
                return True

            def do_OPTIONS(self):
                if not self.allowed(cap=False):
                    return
                method = self.headers.get("Access-Control-Request-Method")
                requested = {x.strip().lower() for x in self.headers.get("Access-Control-Request-Headers", "").split(",") if x.strip()}
                methods = {"GET", "POST"} if urlsplit(self.path).path.startswith("/control/") else {"GET", "HEAD"}
                if method not in methods or not requested <= {"range", "if-range", "content-type"}:
                    self.response(403)
                    return
                self.response(204, headers={"Access-Control-Allow-Methods": ", ".join(sorted(methods | {"OPTIONS"})),
                    "Access-Control-Allow-Headers": "Range, If-Range, Content-Type"})

            def do_POST(self):
                if not self.allowed():
                    return
                path = urlsplit(self.path).path
                if path not in {"/control/result", "/control/sample"} and not (path == "/control/invoke" and fixture.invoke):
                    self.response(405)
                    return
                length = self.headers.get("Content-Length", "")
                if not length.isdigit() or not 0 < int(length) <= 1024 * 1024:
                    self.response(413)
                    return
                try:
                    value = json.loads(self.rfile.read(int(length)))
                    if not isinstance(value, dict):
                        raise ValueError("object required")
                except (ValueError, UnicodeError):
                    self.response(400)
                    return
                if path == "/control/invoke":
                    try:
                        result = fixture.invoke(value)
                        self.response(200, json.dumps(result).encode(), {"Content-Type": "application/json"})
                    except Exception:
                        self.response(500, b'{}', {"Content-Type": "application/json"})
                    return
                (fixture.results if path.endswith("result") else fixture.samples).put(value)
                self.response(204)

            def do_HEAD(self):
                self.do_GET()

            def do_GET(self):
                path = urlsplit(self.path).path
                with fixture.lock:
                    fixture.records.append({"path": path, "method": self.command,
                        "origin": self.headers.get("Origin"), "range": self.headers.get("Range")})
                if not self.allowed():
                    return
                if path == "/control/next":
                    try:
                        command = fixture.commands.get(timeout=0.5)
                    except queue.Empty:
                        self.response(204)
                    else:
                        with fixture.lock:
                            fixture.records.append({"path": path, "method": "DEQUEUE", "command_id": command["id"]})
                        self.response(200, json.dumps(command).encode(), {"Content-Type": "application/json"})
                    return
                if path == "/probe/cors-denied":
                    # Deliberately omit ACAO: WebView must reject reading this response.
                    self.send_response(200)
                    self.send_header("Content-Length", "6")
                    self.send_header("Connection", "close")
                    self.close_connection = True
                    self.end_headers()
                    self.wfile.write(b"denied")
                    return
                media = fixture.files.get(path)
                if media is None:
                    self.response(404)
                    return
                data = None
                if media.suffix == ".m3u8":
                    # Every child URI shares this playback capability, including native HLS.
                    lines = media.read_text().splitlines()
                    data = ("\n".join(line + f"?cap={fixture.cap}" if line and not line.startswith("#") else line for line in lines) + "\n").encode()
                size = len(data) if data is not None else media.stat().st_size
                etag = '"' + hashlib.sha256(data).hexdigest() + '"' if data is not None else fixture.etags[path]
                try:
                    selected = byte_range(self.headers.get("Range"), size) if self.headers.get("If-Range") in {None, etag} else None
                except ValueError:
                    self.response(416, headers={"Content-Range": f"bytes */{size}", "Accept-Ranges": "bytes"})
                    return
                start, end = selected if selected else (0, size - 1)
                self.send_response(206 if selected else 200)
                self.send_header("Content-Type", TYPES[media.suffix])
                self.send_header("Content-Length", str(end - start + 1))
                self.send_header("Accept-Ranges", "bytes")
                self.send_header("ETag", etag)
                self.send_header("Cache-Control", "no-store")
                self.send_header("Connection", "close")
                self.close_connection = True
                if selected:
                    self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
                if self.headers.get("Origin") in ORIGINS:
                    self.send_header("Access-Control-Allow-Origin", self.headers["Origin"])
                    self.send_header("Vary", "Origin")
                    self.send_header("Access-Control-Expose-Headers", "Content-Range, Accept-Ranges, ETag")
                self.end_headers()
                if self.command == "HEAD":
                    return
                with fixture.lock:
                    fixture.active += 1
                try:
                    if data is not None:
                        self.wfile.write(data[start:end + 1])
                    else:
                        with media.open("rb") as stream:
                            stream.seek(start)
                            remaining = end - start + 1
                            while remaining:
                                block = stream.read(min(65536, remaining))
                                if not block:
                                    raise OSError("fixture truncated during transfer")
                                self.wfile.write(block)
                                remaining -= len(block)
                except (BrokenPipeError, ConnectionResetError):
                    # Cancellation is normal for video seek and stop.
                    pass
                finally:
                    with fixture.lock:
                        fixture.active -= 1

        self.http = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.http.daemon_threads = True
        self.http.block_on_close = False
        self.port = self.http.server_address[1]
        self.base = f"http://127.0.0.1:{self.port}/"
        self.thread = threading.Thread(target=self.http.serve_forever, name="t01-fixture", daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_args):
        self.http.shutdown()
        self.http.server_close()
        self.thread.join(timeout=5)
        if self.thread.is_alive():
            raise RuntimeError("fixture thread did not stop")

    def summary(self):
        return {"deployment": "single-process loopback fixture; no cluster", "active_requests": self.active,
                "requests": dict(Counter(row["path"] for row in self.records)), "records": self.records}
