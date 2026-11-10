"""T01 native Tauri media gate. Invoke every suite through the original run.py.

prepare installs the pinned inputs and generates our own media once; build creates
separate production and automation .app bundles. all never builds or downloads:
it validates their source binding, then runs the real packaged WKWebView for
1800 seconds per format and twenty complete process/window lifecycles.
functional reuses real open/controls/stop checks; endurance belongs to T0.28.
"""
import argparse
import contextlib
import datetime as dt
import hashlib
import http.client
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import plistlib
import queue
import re
import signal
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import uuid
import xml.etree.ElementTree as ET

from t00 import Acceptance as BaseAcceptance, file_hash, read_json, require, workflow, write_json

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests/fixtures/t01"
DATA = ROOT / "evidence/t01"
MEDIA = DATA / "media"
BUILD = DATA / "build.json"
IDS = ("U01", "U04", "R30", "R32", "R48", "R49", "R50", "REQ-T01")
DEFERRED = ("macos-x86_64", "windows-x86_64", "linux-x11", "linux-wayland")
spec = importlib.util.spec_from_file_location("t01_server", FIXTURES / "server.py")
server_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(server_module)
Fixture = server_module.Fixture


class Acceptance(BaseAcceptance):
    def __init__(self, suite, report, junit):
        self.suite, self.report_path, self.junit_path = suite, report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T01-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.cases, self.commands, self.observed = [], [], {}
        self.source = workflow.snapshot()

    def save(self):
        # prepare is explicitly allowed to create Cargo.lock; all/build are immutable.
        current = workflow.snapshot()
        if self.suite != "prepare":
            self.case("source-stability", ["REQ-T01"], "stable tested source",
                      lambda: require(current == self.source, "source changed during execution"))
        summaries = []
        if self.suite in ("all", "functional"):
            for identifier in IDS:
                children = [c for c in self.cases if identifier in c["requirements"]]
                passed = bool(children) and all(c["result"] == "PASS" or
                    (c["result"] == "SKIP" and not c["required"]) for c in children)
                failures = [dict(id=c["id"], result=c["result"], expected=c["expected"],
                                 actual=c["actual"], evidence=c["evidence"])
                            for c in children if c["result"] == "FAIL" or (c["result"] == "SKIP" and c["required"])]
                summaries.append(dict(id=identifier, result="PASS" if passed else "FAIL",
                    children=[c["id"] for c in children], actual=dict(children_present=bool(children), failures=failures)))
        counts = {name: sum(c["result"] == result for c in self.cases)
                  for name, result in (("passed", "PASS"), ("failed", "FAIL"), ("skipped", "SKIP"))}
        report = dict(card="T0.2", original_card="T01", suite=self.suite,
            utc_started=self.started, utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(),
            source_before=self.source, tested_source_digest=current, protected=workflow.protected_hash(),
            os=platform.platform(), arch=platform.machine(), cases=self.cases,
            summaries=summaries, counts=counts, commands=self.commands, observed=self.observed)
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T01-" + self.suite, tests=str(len(rows)),
            failures=str(sum(r["result"] == "FAIL" for r in rows)), errors="0", skipped=str(counts["skipped"]))
        for row in rows:
            node = ET.SubElement(xml, "testcase", name=row["id"], classname="T01")
            if row["result"] == "SKIP":
                ET.SubElement(node, "skipped", message=str(row["actual"]))
            elif row["result"] == "FAIL":
                ET.SubElement(node, "failure", message="T01 behavior assertion failed").text = str(row["actual"])
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        failed = next((case for case in self.cases if case["result"] == "FAIL"), None)
        if failed:
            print(failed["actual"], flush=True)
        return int(counts["failed"] > 0 or counts["passed"] == 0 or any(s["result"] == "FAIL" for s in summaries))


def resource_receipt(run):
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "original run.py resource wrapper required")
    cfg = workflow.config()
    require(cfg["memory_mib"] == 4096 and cfg["jobs"] == 2, "approved budget differs")
    require(os.environ.get("CARGO_BUILD_JOBS") == "2" and os.environ.get("RUST_TEST_THREADS") == "1", "build/test concurrency differs")
    require(Path(os.environ["CARGO_TARGET_DIR"]) == ROOT / "target", "target binding differs")
    ancestors = {p.pid for p in workflow.runtime.psutil.Process().parents()} | {os.getpid()}
    deadline = time.monotonic() + 5
    matches = []
    while time.monotonic() < deadline:
        matches = [(p, read_json(p)) for p in (ROOT / "evidence/runs").glob("*/receipt.json")]
        matches = [(p, r) for p, r in matches if r.get("status") == "running" and r.get("payload_pid") in ancestors
                   and workflow.runtime.alive(dict(pid=r["payload_pid"], identity=r["payload_identity"]))]
        if matches:
            break
        time.sleep(0.05)
    require(len(matches) == 1, "one original resource receipt must own this payload")
    path, receipt = matches[0]
    require(receipt["source"] == run.source and receipt["card"] == "T0.2", "resource source/card binding differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_enforcement"] == "rss-sampled"
            and env["memory_mib"] == 4096 and env["jobs"] == 2 and env["test_threads"] == 1,
            "actual macOS resource backend/budget differs")
    run.observed["resource_receipt"] = dict(path=str(path.relative_to(ROOT)), environment=env,
        note="macOS process-tree RSS sampling; transient overage possible; build jobs=2, no cgroup memory/CPU quota")
    require(sys.platform == "darwin" and platform.machine() == "arm64" and int(platform.mac_ver()[0].split(".")[0]) >= 14,
            "this gate requires the local macOS 14+ arm64 host, ADR-005/006")
    webkit = plistlib.loads(Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist").read_bytes())
    run.observed["system_webkit"] = {key: webkit.get(key) for key in ("CFBundleIdentifier", "CFBundleShortVersionString", "CFBundleVersion")}
    return run.observed["resource_receipt"]


def media_recipe():
    return [
        ["ffmpeg", "-nostdin", "-hide_banner", "-y", "-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=30",
         "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000", "-t", "30", "-c:v", "libx264",
         "-profile:v", "high", "-pix_fmt", "yuv420p", "-preset", "fast", "-crf", "23", "-threads", "2",
         "-g", "60", "-keyint_min", "60", "-sc_threshold", "0", "-c:a", "aac", "-b:a", "128k", "-ac", "2",
         "-movflags", "+faststart", "{media}/clip.mp4"],
        ["ffmpeg", "-nostdin", "-hide_banner", "-y", "-i", "{media}/clip.mp4", "-map", "0", "-c", "copy",
         "-hls_time", "2", "-hls_playlist_type", "vod", "-hls_flags", "independent_segments",
         "-hls_segment_filename", "{media}/segment-%03d.ts", "{media}/index.m3u8"],
    ]


def validate_media(manifest):
    require(manifest["recipe"] == media_recipe(), "media generation recipe changed; preserve the existing baseline")
    require(manifest["license"] == "CC0-1.0; project-generated test pattern and sine tone", "own-media provenance missing")
    video = next(s for s in manifest["ffprobe"]["streams"] if s["codec_type"] == "video")
    audio = next(s for s in manifest["ffprobe"]["streams"] if s["codec_type"] == "audio")
    require((video["codec_name"], video["width"], video["height"], video["r_frame_rate"], int(video["nb_frames"]))
            == ("h264", 1920, 1080, "30/1", 900), "fixture must be real 1080p30 H.264, 900 frames")
    require(audio["codec_name"] == "aac" and int(audio["sample_rate"]) == 48000 and audio["channels"] == 2,
            "fixture must contain stereo 48kHz AAC")
    require(abs(float(manifest["ffprobe"]["format"]["duration"]) - 30) < 0.1, "fixture duration differs")
    require({"clip.mp4", "index.m3u8", "subtitles.vtt"} <= set(manifest["files"])
            and any(n.endswith(".ts") for n in manifest["files"]), "media assets missing")


def check_media():
    manifest = read_json(MEDIA / "manifest.json")
    validate_media(manifest)
    actual = {p.name: file_hash(p) for p in MEDIA.iterdir() if p.is_file() and p.name != "manifest.json"}
    require(actual == manifest["files"], "generated media bytes differ from first-generation baseline")
    return manifest


def prepare(run):
    run.command(["npm", "ci", "--no-audit", "--no-fund"], timeout=600)
    require(read_json(ROOT / "node_modules/hls.js/package.json")["version"] == "1.7.3", "hls.js installed version differs")
    for package in ("@videojs/html", "@videojs/hlsjs-video", "@videojs/core"):
        require(read_json(ROOT / "node_modules" / package / "package.json")["version"] == "10.3.0", "Video.js installed version differs")
    cargo_lock = ROOT / "src-tauri/Cargo.lock"
    if not cargo_lock.exists():
        run.command(["cargo", "generate-lockfile", "--manifest-path", "src-tauri/Cargo.toml"], timeout=600)
    # Fetch exactly this resolution on later runs, retaining the first Cargo.lock.
    run.command(["cargo", "fetch", "--locked", "--manifest-path", "src-tauri/Cargo.toml"], timeout=1200)
    if (MEDIA / "manifest.json").exists():
        return dict(media=check_media(), reused=True, cargo_lock_sha256=file_hash(cargo_lock))
    require(not MEDIA.exists(), "partial media directory exists; preserve and inspect it before preparing again")
    DATA.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="media-staging-", dir=DATA) as staging:
        path = Path(staging)
        versions = {tool: run.command([tool, "-version"]) for tool in ("ffmpeg", "ffprobe")}
        for command in media_recipe():
            run.command([arg.replace("{media}", str(path)) for arg in command], timeout=300)
        (path / "subtitles.vtt").write_text("WEBVTT\n\n00:00:00.000 --> 00:00:29.950\n映象视频 · 自有 1080p30 测试片段\n", encoding="utf-8")
        probe = json.loads(run.command(["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", str(path / "clip.mp4")]))
        manifest = dict(recipe=media_recipe(), license="CC0-1.0; project-generated test pattern and sine tone",
            generated_utc=dt.datetime.now(dt.timezone.utc).isoformat(), tools=versions, ffprobe=probe,
            files={p.name: file_hash(p) for p in path.iterdir() if p.is_file()})
        validate_media(manifest)
        write_json(path / "manifest.json", manifest)
        path.rename(MEDIA)
    return dict(media=check_media(), reused=False, cargo_lock_sha256=file_hash(cargo_lock))


@contextlib.contextmanager
def environment(overrides):
    before = {key: os.environ.get(key) for key in overrides}
    try:
        os.environ.update(overrides)
        yield
    finally:
        for key, value in before.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


def tree_hashes(folder):
    return {p.relative_to(folder).as_posix(): file_hash(p) for p in sorted(folder.rglob("*")) if p.is_file()}


def validate_production(frontend, executable):
    forbidden = (b"control/next", b"/probe/cors-denied", b"T01_FIXTURE_CAP", b"T01_FIXTURE_BASE", b"t01_control",
                 b"_webProcessIdentifier", b"_gpuProcessIdentifier", b"_networkProcessIdentifier", b"_topContentInset")
    for path in [executable, *frontend.rglob("*")]:
        if path.is_file():
            data = path.read_bytes()
            require(not any(marker in data for marker in forbidden), f"test bridge leaked into production: {path.name}")
            require(path.suffix not in {".py", ".ts", ".rs"}, f"test/source asset in production: {path.name}")
    return dict(forbidden_markers=[m.decode() for m in forbidden], executable_sha256=file_hash(executable), frontend_files=len(tree_hashes(frontend)))


def validate_automation(frontend, executable):
    require(b"T01_FIXTURE_CAP" in executable.read_bytes(), "automation native feature was not built")
    # The base URL supplies the leading slash; Vite preserves the relative path.
    require(any(all(marker in data for marker in (b"control/next", b"t01_control"))
                for data in (p.read_bytes() for p in frontend.rglob("*.js"))),
            "automation frontend mode was not built: command transport or native bridge missing")


def bundle(run, name, binary):
    # The technology gate needs a real macOS app bundle, including the ATS plist.
    # Distribution installers/signing/notarization are kept for the packaging cards.
    app = DATA / (name + ".app")
    if app.exists():
        shutil.rmtree(app)  # This exact fixture-owned output is reconstructed by this build only.
    executable = app / "Contents/MacOS/yingxiang-video"
    executable.parent.mkdir(parents=True)
    shutil.copy2(binary, executable)
    info = plistlib.loads((ROOT / "src-tauri/Info.plist").read_bytes())
    info.update(CFBundleExecutable="yingxiang-video", CFBundleIdentifier="app.yingxiang.video",
        CFBundleName=name, CFBundleDisplayName="映象视频", CFBundlePackageType="APPL",
        CFBundleShortVersionString="0.1.0", CFBundleVersion="1", LSMinimumSystemVersion="14.0", NSHighResolutionCapable=True)
    (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
    run.command(["codesign", "--force", "--sign", "-", "--timestamp=none", str(app)])
    run.command(["codesign", "--verify", "--deep", "--strict", str(app)])
    return app


def build(run):
    command_start = len(run.commands)
    check_media()
    require((ROOT / "src-tauri/Cargo.lock").exists(), "prepare must create Cargo.lock first")
    require("TAURI_CONFIG" not in os.environ, "build requires an unmodified production config")
    run.command(["cargo", "fmt", "--manifest-path", "src-tauri/Cargo.toml", "--", "--check"])
    run.command(["npm", "run", "build"], timeout=300)
    run.command(["cargo", "build", "--locked", "--release", "--manifest-path", "src-tauri/Cargo.toml"], timeout=3000)
    production = bundle(run, "YingxiangVideo", ROOT / "target/release/yingxiang-video")
    production_frontend = DATA / "production-frontend"
    if production_frontend.exists():
        shutil.rmtree(production_frontend)
    shutil.copytree(ROOT / "dist", production_frontend)
    production_gate = validate_production(production_frontend, production / "Contents/MacOS/yingxiang-video")
    run.command(["npm", "exec", "--", "vite", "build", "--mode", "t01", "--outDir", "target/t01/frontend-automation"], timeout=300)
    test_config = {"build": {"frontendDist": "../target/t01/frontend-automation"},
        "app": {"security": {"capabilities": ["main", {"identifier": "t01-only", "windows": ["main"], "permissions": ["allow-t01-control"]}]}}}
    with environment({"TAURI_CONFIG": json.dumps(test_config)}):
        run.command(["cargo", "build", "--locked", "--release", "--manifest-path", "src-tauri/Cargo.toml",
            "--features", "t01-automation"], timeout=3000)
    automation = bundle(run, "YingxiangVideoT01", ROOT / "target/release/yingxiang-video")
    validate_automation(ROOT / "target/t01/frontend-automation", automation / "Contents/MacOS/yingxiang-video")
    receipt = dict(source=run.source, protected=workflow.protected_hash(), cargo_lock=file_hash(ROOT / "src-tauri/Cargo.lock"),
        production=dict(path=str(production.relative_to(ROOT)), files=tree_hashes(production)),
        automation=dict(path=str(automation.relative_to(ROOT)), files=tree_hashes(automation)),
        production_frontend=tree_hashes(production_frontend), production_gate=production_gate,
        test_config=test_config, command_logs=run.commands[command_start:])
    write_json(BUILD, receipt)
    return receipt


def check_build(source):
    receipt = read_json(BUILD)
    require(receipt["source"] == source and receipt["protected"] == workflow.protected_hash(), "build source/protected digest differs; rebuild stable source")
    require(receipt["cargo_lock"] == file_hash(ROOT / "src-tauri/Cargo.lock"), "native dependency lock changed since build")
    for key in ("production", "automation"):
        require(tree_hashes(ROOT / receipt[key]["path"]) == receipt[key]["files"], f"{key} app bytes changed since build")
    frontend = DATA / "production-frontend"
    require(tree_hashes(frontend) == receipt["production_frontend"], "production frontend changed since build")
    validate_production(frontend, ROOT / receipt["production"]["path"] / "Contents/MacOS/yingxiang-video")
    info = plistlib.loads((ROOT / receipt["production"]["path"] / "Contents/Info.plist").read_bytes())
    require(info["NSAppTransportSecurity"] == {"NSAllowsLocalNetworking": True}, "ATS must retain a local-network-only exception")
    require(info["NSHighResolutionCapable"] and info["LSMinimumSystemVersion"] == "14.0", "DPI/minimum macOS metadata missing")
    require(all(row.get("exit") == 0 for row in receipt["command_logs"]), "build contains failed commands")
    return receipt


def http_request(fixture, method, path, headers=None):
    connection = http.client.HTTPConnection("127.0.0.1", fixture.port, timeout=10)
    try:
        connection.request(method, path, headers=headers or {})
        response = connection.getresponse()
        return response.status, dict(response.getheaders()), response.read()
    finally:
        connection.close()


def security(fixture):
    path = "/media/clip.mp4?cap=" + fixture.cap
    origin = {"Origin": "tauri://localhost"}
    status, headers, body = http_request(fixture, "GET", path, {**origin, "Range": "bytes=0-15"})
    require(status == 206 and len(body) == 16 and body == (MEDIA / "clip.mp4").read_bytes()[:16], "Range bytes differ")
    require(headers["Content-Range"].startswith("bytes 0-15/") and headers["Access-Control-Allow-Origin"] == origin["Origin"], "Range/CORS response differs")
    for bad in ("/media/clip.mp4", "/media/clip.mp4?cap=bad", path + "&cap=bad"):
        require(http_request(fixture, "GET", bad)[0] == 403, "missing/incorrect/duplicate cap accepted")
    require(http_request(fixture, "GET", path, {"Origin": "https://evil.invalid"})[0] == 403, "foreign Origin accepted")
    require(http_request(fixture, "GET", path, {"Host": "evil.invalid"})[0] == 421, "foreign Host accepted")
    for value in ("bytes=999999999999-", "bytes=8-3", "bytes=0-1,3-4", "bytes=-0"):
        require(http_request(fixture, "GET", path, {**origin, "Range": value})[0] == 416, "invalid Range accepted")
    status, head, body = http_request(fixture, "HEAD", path, origin)
    require(status == 200 and not body and int(head["Content-Length"]) == (MEDIA / "clip.mp4").stat().st_size, "HEAD differs")
    status, _, body = http_request(fixture, "GET", path, {**origin, "Range": "bytes=0-15", "If-Range": '"stale"'})
    require(status == 200 and len(body) == (MEDIA / "clip.mp4").stat().st_size, "stale If-Range must return full representation")
    require(http_request(fixture, "GET", path, {**origin, "Range": "bytes=0-15", "If-Range": headers["ETag"]})[0] == 206, "matching If-Range failed")
    status, preflight, body = http_request(fixture, "OPTIONS", "/media/clip.mp4", {**origin,
        "Access-Control-Request-Method": "GET", "Access-Control-Request-Headers": "range, if-range"})
    require(status == 204 and not body and preflight["Access-Control-Allow-Origin"] == origin["Origin"], "public CORS metadata preflight failed")
    require(http_request(fixture, "OPTIONS", "/media/clip.mp4", {"Origin": "https://evil.invalid", "Access-Control-Request-Method": "GET"})[0] == 403, "foreign preflight accepted")
    require(http_request(fixture, "POST", path)[0] == 405, "media POST accepted")
    require(http_request(fixture, "GET", "/media/../manifest.json?cap=" + fixture.cap)[0] == 404, "unregistered path accepted")
    status, _, playlist = http_request(fixture, "GET", "/media/index.m3u8?cap=" + fixture.cap, origin)
    children = [line for line in playlist.decode().splitlines() if line and not line.startswith("#")]
    require(status == 200 and len(children) >= 2 and all(line.endswith("?cap=" + fixture.cap) for line in children), "HLS child capability missing")
    return dict(range=True, head=True, if_range=True, invalid_range=True, caps=True, origins=True, host=True,
        options=True, child_resources=len(children), binding="127.0.0.1 on an OS-assigned random port", stream_chunk_bytes=65536)


class NativeSession:
    @staticmethod
    def launch_argv(executable):
        argv = [str(executable)]
        if sys.platform == "darwin":
            # Use Cocoa's volatile argument domain; retain the user's saved state.
            argv.extend(["-ApplePersistenceIgnoreState", "YES"])
        return argv

    def __init__(self, run, fixture, executable, index):
        self.run, self.fixture = run, fixture
        self.sequence = 0
        self.owned = {}
        self.samples = []
        self.closed = False
        self.path = run.folder / f"native-{index}.log"
        self.stream = self.path.open("w", encoding="utf-8")
        env = dict(os.environ, T01_FIXTURE_BASE=fixture.base, T01_FIXTURE_CAP=fixture.cap)
        # Keep the native app and WebKit helpers inside the original runner session.
        argv = self.launch_argv(executable)
        self.process = subprocess.Popen(argv, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace")
        def output():
            for line in self.process.stdout:
                safe = line.replace(fixture.cap, "<redacted>")
                self.stream.write(getattr(run, "redact_output", str)(safe))
                self.stream.flush()
        self.output_thread = threading.Thread(target=output, name=f"t01-output-{index}", daemon=True)
        self.output_thread.start()
        self.identity = workflow.runtime.identity(self.process.pid)
        self.owned[self.process.pid] = self.identity
        self.started = time.monotonic()
        self.record = dict(pid=self.process.pid, identity=self.identity, argv=argv, executable_sha256=file_hash(executable),
            log=str(self.path.relative_to(ROOT)), index=index, started_utc=dt.datetime.now(dt.timezone.utc).isoformat())
        run.observed.setdefault("native_sessions", []).append(self.record)
        self.wake_process = None
        try:
            self.start_wake_guard()
        except Exception:
            self.close(graceful=False)
            raise

    def start_wake_guard(self):
        # Keep the GUI test awake without changing the user's lock/sleep settings.
        # -w also releases the assertion if the native app exits unexpectedly.
        argv = ["/usr/bin/caffeinate", "-d", "-i", "-w", str(self.process.pid)]
        self.wake_process = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        token = workflow.runtime.identity(self.wake_process.pid)
        require(token and self.wake_process.poll() is None, "native GUI wake guard could not start")
        self.owned[self.wake_process.pid] = token
        self.record["wake_guard"] = dict(pid=self.wake_process.pid, identity=token, argv=argv)

    def observe(self, enforce_budget=True):
        process = workflow.runtime.process_for(self.process.pid, self.identity)
        current = {p.pid: p for p in ([process] + process.children(recursive=True))} if process else {}
        for pid, token in self.owned.items():
            owned = workflow.runtime.process_for(pid, token)
            if owned:
                current[pid] = owned
        rss = 0
        for child in current.values():
            try:
                self.owned[child.pid] = workflow.runtime.identity(child.pid)
                rss += child.memory_info().rss
            except workflow.runtime.psutil.NoSuchProcess:
                pass
        self.record["peak_owned_rss_bytes"] = max(self.record.get("peak_owned_rss_bytes", 0), rss)
        self.record["owned_processes"] = [{"pid": pid, "identity": token} for pid, token in self.owned.items()]
        parent = workflow.runtime.psutil.Process()
        budget_processes = {p.pid: p for p in [parent, *parent.children(recursive=True), *current.values()]}
        budget_rss = 0
        for child in budget_processes.values():
            try:
                budget_rss += child.memory_info().rss
            except workflow.runtime.psutil.NoSuchProcess:
                pass
        self.record["peak_combined_rss_bytes"] = max(self.record.get("peak_combined_rss_bytes", 0), budget_rss)
        if enforce_budget:
            require(budget_rss <= 4096 * 1024 * 1024, "combined host tree and WebKit XPC RSS exceeded approved 4096MiB sampled budget")
        return rss

    def register_webkit(self, native):
        require(native["pid"] == self.process.pid, "WebKit ownership report has the wrong host PID")
        values = native["webkit_pids"]
        require(isinstance(values, dict) and values.get("renderer", 0) > 0, "actual WebKit renderer PID is required")
        for role, pid in values.items():
            if pid <= 0:
                continue  # A helper can be absent before its first use; record the actual zero.
            process = workflow.runtime.psutil.Process(pid)
            require("WebKit" in process.name(), f"reported {role} PID is not a WebKit helper")
            token = workflow.runtime.identity(pid)
            require(token, f"reported {role} PID already exited")
            self.owned[pid] = token
        self.record["webkit_pids"] = values

    def drain_samples(self):
        while True:
            try:
                sample = self.fixture.samples.get_nowait()
            except queue.Empty:
                break
            sample["host_received_monotonic"] = time.monotonic()
            self.register_webkit(sample["native"])
            sample["owned_rss_bytes"] = self.observe()
            self.samples.append(sample)
            with (self.run.folder / f"samples-{self.record['index']}.jsonl").open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(sample) + "\n")

    def wait(self, identifier, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.observe()
            self.drain_samples()
            try:
                result = self.fixture.results.get(timeout=0.25)
            except queue.Empty:
                require(self.process.poll() is None, f"native app exited unexpectedly ({self.process.returncode}); {self.record['log']}")
                continue
            # Preserve failed results before raising, including the exact WebView
            # assertion snapshot; the last successful sample cannot explain it.
            result_path = self.run.folder / f"results-{self.record['index']}.jsonl"
            with result_path.open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(dict(host_received_monotonic=time.monotonic(), result=result)) + "\n")
            self.record["result_log"] = str(result_path.relative_to(ROOT))
            self.drain_samples()
            require(result.get("id") == identifier, "native result belongs to an unexpected command/session")
            require(result.get("ok") is True, f"{result.get('error', 'native command failed')}; result: {self.record['result_log']}")
            return result["value"]
        raise TimeoutError(f"native command {identifier} did not return within {timeout}s; {self.record['log']}")

    def command(self, op, timeout=30, **args):
        self.sequence += 1
        # ADR-060 applies to every T24 path, including failure diagnostics.
        self.fixture.commands.put(dict(id=self.sequence, op=op, **args,
            skip_audio_sampling=self.run.CARD == 'T0.35'))
        start = time.monotonic()
        value = self.wait(self.sequence, timeout)
        if isinstance(value, dict) and "native" in value:
            self.register_webkit(value["native"])
        row = dict(op=op, args=args, host_seconds=time.monotonic() - start, value=value)
        self.record.setdefault("commands", []).append(row)
        return value

    def close(self, graceful=True):
        if self.closed:
            return
        try:
            if graceful and self.process.poll() is None:
                shutdown = self.command("shutdown")
                require(shutdown["audio_state"] == "closed", "WebAudio output was not released")
                require(not shutdown["errors"], "Unhandled WebView errors: " + str(shutdown["errors"]))
                self.process.wait(timeout=15)
                require(self.process.returncode == 0, "native app exited nonzero")
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline and any(workflow.runtime.identity(pid) == token for pid, token in self.owned.items()):
                    time.sleep(0.1)
        finally:
            try:
                self.observe(enforce_budget=False)
            finally:
                # Identity-qualified cleanup also runs when diagnostic sampling fails.
                forced = [pid for pid, token in self.owned.items() if workflow.runtime.identity(pid) == token]
                workflow.runtime.terminate([dict(pid=pid, identity=token) for pid, token in self.owned.items()])
                self.process.wait(timeout=10)
                if self.wake_process is not None:
                    self.record.setdefault("wake_guard", {})["exit"] = self.wake_process.wait(timeout=5)
                self.output_thread.join(timeout=5)
                require(not self.output_thread.is_alive(), "native output pipe remains open")
                self.process.stdout.close()
                self.stream.close()
                self.closed = True
                lingering = [pid for pid, token in self.owned.items() if workflow.runtime.identity(pid) == token]
                self.record.update(exit=self.process.returncode, seconds=time.monotonic() - self.started, lingering=lingering, forced_cleanup=forced)
                sample_log = self.run.folder / f"samples-{self.record['index']}.jsonl"
                if sample_log.is_file():
                    self.record["sample_log"] = str(sample_log.relative_to(ROOT))
                require(not lingering, "owned native/WebKit processes remain")
                require(not (graceful and forced), "native/WebKit processes required forced cleanup after normal shutdown")


def validate_long(value, samples):
    require(value["duration_seconds"] >= 1800 and value["media_seconds"] >= 1770, "30-minute wall/media duration missing")
    require(value["samples"] >= 350 and len(samples) == value["samples"], "long-play raw sample coverage incomplete")
    require(value["unique_frames"] > 2 and value["final"]["frames"] > value["initial"]["frames"], "presented frames did not advance")
    require(all(s["width"] == 1920 and s["height"] == 1080 and not s["paused"] and s["media_error"] is None
        and s["audio_state"] == "running" and s["audio_rms"] > 0.001 and abs(s["audio_peak_hz"] - 440) < 30
        for s in samples), "long-play video/audio sample failed")
    confirmed = value.get("foreground", {}).get("confirmed", {})
    require(confirmed.get("visible") is True and confirmed.get("focused") is True
        and confirmed.get("minimized") is False, "long-play foreground activation confirmation missing")
    require(all(s.get("visibility") == "visible" and s.get("focused") is True
        and s.get("foreground_interruptions") == value["initial"].get("foreground_interruptions")
        and s.get("foreground_interruptions") is not None
        and s.get("native", {}).get("visible") is True and s.get("native", {}).get("focused") is True
        and s.get("native", {}).get("minimized") is False for s in samples),
        "long-play foreground condition lost or evidence missing")
    require(all(isinstance(s.get("ui_position"), (int, float)) and math.isfinite(s["ui_position"])
        and 0 <= s["ui_position"] <= s["duration"]
        and min(abs(s["ui_position"] - s["position"]),
                abs(s["duration"] - abs(s["ui_position"] - s["position"]))) <= 1 for s in samples),
        "long-play progress UI differs from media clock or evidence missing")
    require(samples[-1]["host_received_monotonic"] - samples[0]["host_received_monotonic"] >= 1790, "host-observed duration too short")
    require(all(b["time"] > a["time"] and b["frames"] > a["frames"] and b["raf"] > a["raf"] for a, b in zip(samples, samples[1:])), "frame/UI sample sequence stalled")
    require(all(b["ui_position"] != a["ui_position"] for a, b in zip(samples, samples[1:])), "progress UI sample sequence stalled")
    require(all(0 < b["host_received_monotonic"] - a["host_received_monotonic"] < 15 for a, b in zip(samples, samples[1:])), "host sample gap exceeds 15s")
    return dict(duration_seconds=value["duration_seconds"], media_seconds=value["media_seconds"], samples=len(samples),
        presented_frames=value["final"]["frames"] - value["initial"]["frames"], dropped_frames=value["final"]["dropped_frames"] - value["initial"]["dropped_frames"],
        maximum_raf_gap_ms=max(s["max_raf_gap_ms"] for s in samples), peak_owned_rss_bytes=max(s["owned_rss_bytes"] for s in samples),
        audio_measurement="real media-element PCM analyser connected to default WebAudio output; physical speaker audibility is not instrumented")


def validate_window(value, width=1200, zoom=1):
    native, layout = value["native"], value["layout"]
    scale, inset = native["scale_factor"], native["top_content_inset"]
    require(math.isfinite(scale) and scale > 0 and math.isfinite(inset) and 0 <= inset < 800,
            "invalid native scale or top content inset")
    require(native["physical_width"] == width * scale and native["physical_height"] == 800 * scale,
            "native logical window size differs")
    # WKWebView's automatic title-bar inset is measured in native logical points.
    # Keep the requested window size exact, and derive the CSS viewport from it.
    require(abs(layout["inner_width"] - width / zoom) <= 1
            and abs(layout["inner_height"] - (800 - inset) / zoom) <= 1,
            "WebView viewport differs from native frame, inset and zoom")
    require(layout["inner_width"] > 0 and layout["inner_height"] > 0
            and layout["scroll_width"] <= layout["inner_width"] + 1, "WebView layout overflows horizontally")
    return value


def native_gui_session():
    # Capture only the lock flag; the full registry also contains machine IDs.
    state = subprocess.run(["/usr/sbin/ioreg", "-n", "Root", "-d1"],
                           capture_output=True, text=True, timeout=10)
    require(state.returncode == 0, "macOS GUI session state could not be read")
    flag = re.search(r'"CGSSessionScreenIsLocked"\s*=\s*(Yes|No)', state.stdout)
    return dict(screen_locked=flag[1] == "Yes" if flag else None)


def validate_functional(opened, controlled, kind="mp4"):
    first, current = opened["sample"], controlled["sample"]
    require(current["width"] == 1920 and current["height"] == 1080 and not current["paused"]
            and current["media_error"] is None and current["frames"] > first["frames"],
            "functional video decoding or presented frames failed")
    if kind == "hls":
        # WebKit does not expose HLS PCM to MediaElementAudioSourceNode (bugs 180696/306493).
        require(controlled["restored"].get("audio_playing") is True, "functional native HLS audio is not playing")
        audio_measurement = "WebKit native audio playback state; HLS PCM measurement deferred to product testing"
    else:
        require(current["audio_state"] == "running" and current["audio_rms"] > 0.001
                and abs(current["audio_peak_hz"] - 440) < 30, "functional decoded audio failed")
        audio_measurement = "media-element decoded PCM RMS and tone frequency"
    require(current["raf"] > first["raf"] and math.isfinite(current["ui_position"])
            and 0 <= current["ui_position"] <= current["duration"]
            and abs(current["ui_position"] - current["position"]) <= 1,
            "functional UI progress differs from media clock")
    return dict(presented_frames=current["frames"] - first["frames"],
                ui_callbacks=current["raf"] - first["raf"], audio_rms=current["audio_rms"],
                audio_measurement=audio_measurement)


def native_all(run, fixture, receipt, *, endurance=True):
    run.observed["gui_session"] = native_gui_session()
    require(run.observed["gui_session"]["screen_locked"] is not True,
            "macOS GUI session is locked; unlock the desktop before native acceptance")
    executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/yingxiang-video"
    session = NativeSession(run, fixture, executable, 0)
    failed = False
    try:
        session.wait(0, 30)
        identity = session.command("identity")
        require(identity["native"]["pid"] == session.process.pid and identity["native"]["label"] == "main"
            and identity["native"]["tauri"] == "2.12.5", "real native host handshake differs")
        require(identity["origin"] == "tauri://localhost", "unexpected macOS packaged WebView Origin")
        run.observed["webview"] = identity
        require(run.case("packaged-native-window", ["U01", "R30", "R32", "REQ-T01"],
            "1200x800 native logical frame; CSS viewport matches measured WebKit inset",
            lambda: validate_window(identity)), "native window geometry gate failed")
        require(run.case("webview-security", ["R48", "REQ-T01"], "actual WebView Range/cap/CORS/CSP and rejected privileged IPC", lambda: session.command("security")), "WebView security gate failed")
        for kind in ("mp4", "hls"):
            opened = session.command("open", kind=kind)
            run.case(kind + "-embedded-decode", ["U04", "R30", "REQ-T01"], "1080p H264/AAC embedded playback", lambda opened=opened: require(opened["sample"]["width"] == 1920 and opened["sample"]["height"] == 1080, "decoded dimensions differ"))
            def controls():
                value = session.command("controls")
                if not endurance:
                    value["functional_observation"] = validate_functional(opened, value, kind)
                return value
            require(run.case(kind + "-controls", ["U04", "R32", "REQ-T01"], "pause, seek, cue toggle, volume, speed, native fullscreen", controls), "Playback controls gate failed")
            if endurance:
                before = len(session.samples)
                value = session.command("long", timeout=1900, seconds=1800)
                session.drain_samples()
                require(run.case(kind + "-30-minutes", ["U01", "U04", "R30", "R50", "REQ-T01"], "1800s real WebView wall time, frames/audio/UI samples", lambda: validate_long(value, session.samples[before:])), "Long playback gate failed")
            run.case(kind + "-release", ["R50", "REQ-T01"], "stop removes source/tracks/HLS/listeners and old frames", lambda: session.command("stop"))
        for width in (360, 390, 768, 960, 1200):
            run.case(f"layout-{width}", ["R32", "REQ-T01"], "real window resize and visible controls", lambda width=width: validate_window(session.command("layout", value=width), width=width))
        for zoom in (1.25, 1.5, 2, 1):
            run.case(f"dpi-{zoom}", ["R32", "REQ-T01"], "native WKWebView zoom with usable controls", lambda zoom=zoom: validate_window(session.command("zoom", value=zoom), zoom=zoom))
    except Exception:
        failed = True
        raise
    finally:
        session.close(graceful=not failed)
    if not endurance:
        run.observed["product_test_handoff"] = dict(owner="T0.28/A1", status="NOT RUN",
            seconds_per_format=1800, formats=["mp4", "hls"], full_window_cycles=20,
            hls_pcm="WebKit media-element PCM is unavailable; supported product-stage audio capture required",
            evidence="current product source/build and original resource/cleanup assertions required")
        return dict(functional_formats=2, sessions=1, endurance_owner="T0.28/A1")
    # Twenty genuine app/window lifecycles, each decoding media and releasing it.
    for index in range(1, 21):
        def lifecycle(index=index):
            session = NativeSession(run, fixture, executable, index)
            failed = False
            try:
                session.wait(0, 30)
                opened = session.command("open", kind="mp4" if index % 2 else "hls")
                require(opened["sample"]["frames"] > 0 and opened["sample"]["audio_rms"] > 0.001, "cycle did not decode video/audio")
                session.command("stop")
            except Exception:
                failed = True
                raise
            finally:
                session.close(graceful=not failed)
            return session.record
        passed = run.case(f"window-cycle-{index:02}", ["U01", "R50", "REQ-T01"], "open native window, decode, stop, close; zero owned processes", lifecycle)
        require(passed, "native lifecycle failed; stop and preserve its reproduction")
    require(len(run.observed["native_sessions"]) == 21 and all(not s["lingering"] and s["exit"] == 0 for s in run.observed["native_sessions"]), "native lifecycle cleanup incomplete")
    return dict(long_formats=2, full_window_cycles=20, sessions=21)


def regressions(run):
    output = run.command([sys.executable, "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t01.py", "-v"], timeout=180)
    require("Ran 0 tests" not in output and "\nOK" in output, "nonzero regression tests required")
    run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "tests/t01-automation.test.ts", "tests/player.test.ts"], timeout=180)
    return output


def all_cases(run):
    run.case("generated-media", ["U04", "R30", "REQ-T01"], "own 1080p30 H264/AAC source and unchanged MP4/HLS bytes", check_media)
    build_ok = run.case("production-artifacts", ["R49", "REQ-T01"], "stable-source production and test app; test server/bridge excluded", lambda: check_build(run.source))
    run.case("negative-regressions", ["R48", "R49", "R50", "REQ-T01"], "behavioral fixture/report rejection tests", lambda: regressions(run))
    if not build_ok or any(c["result"] == "FAIL" for c in run.cases):
        raise ValueError("native gate NOT RUN: prerequisites failed")
    receipt = read_json(BUILD)
    fixture = Fixture(MEDIA)
    port = fixture.port
    try:
        with fixture:
            require(run.case("loopback-security", ["R48", "REQ-T01"], "Origin/cap/Host/OPTIONS/Range/If-Range and HLS child auth", lambda: security(fixture)), "fixture security gate failed")
            endurance = run.suite == "all"
            run.case("real-tauri-lifecycles" if endurance else "real-tauri-functional",
                ["U01", "U04", "R30", "R32", "R50", "REQ-T01"],
                "two real 30-minute formats plus 20 complete window cycles" if endurance else
                "real MP4/HLS controls, decoded frames/audio/UI, layouts and normal cleanup",
                lambda: native_all(run, fixture, receipt, endurance=endurance))
            deadline = time.monotonic() + 10
            while fixture.active and time.monotonic() < deadline:
                time.sleep(0.1)
            require(fixture.active == 0, "media requests remain after closing windows")
    finally:
        run.observed["fixture"] = fixture.summary()
    def cleanup():
        require(not fixture.thread.is_alive() and fixture.active == 0, "fixture resources remain")
        with socket.socket() as probe:
            require(probe.connect_ex(("127.0.0.1", port)) != 0, "fixture port remains open")
        require(all(not s.get("lingering", ["unknown"]) for s in run.observed.get("native_sessions", [])), "owned native processes remain")
        return dict(fixture_thread_stopped=True, port_closed=True, active_requests=0,
            media_request_count=sum(v for k, v in fixture.summary()["requests"].items() if k.startswith("/media/")))
    run.case("all-owned-resources-released", ["R50", "REQ-T01"], "owned processes, media requests, fixture thread and port released", cleanup)
def platform_matrix(run):
    targets = {"macos-x86_64": "x86_64-apple-darwin", "windows-x86_64": "x86_64-pc-windows-msvc",
               "linux-x11": "x86_64-unknown-linux-gnu", "linux-wayland": "x86_64-unknown-linux-gnu"}
    for target in DEFERRED:
        argv = ["{python}", "-B", "scripts/run.py", "t01-build-" + target, "--", "cargo", "build", "--locked",
                "--release", "--manifest-path", "src-tauri/Cargo.toml", "--target", targets[target]]
        run.cases.append(dict(id="R30/platform-" + target, requirements=["R30", "REQ-T01"], required=False,
            expected="deferred per ADR-005/006", actual="NOT RUN; native build and interactive sample command only; full native acceptance adapter pending authorization for this target",
            input={"build_argv": argv, "launch_argv": [f"target/{targets[target]}/release/yingxiang-video" + (".exe" if target.startswith("windows") else "")],
                   "platform": target, "prerequisite": "target machine, system WebView/codecs, installed Rust target; authorized C1 root/venv/global-lock binding; npm ci and npm run build via run.py",
                   "display_environment": {"GDK_BACKEND": "x11" if target == "linux-x11" else "wayland"} if target.startswith("linux") else {},
                   "full_scope_pending": "MP4/HLS 30min each, 20 windows, Origin/CSP/CORS/codec/audio/subtitles/DPI/cleanup"}, result="SKIP", evidence=[]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", choices=("prepare", "build", "negatives", "functional", "all"), required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.suite, args.report, args.junit)
    if args.suite in ("all", "functional"):
        platform_matrix(run)
    def cancelled(signum, _frame):
        raise RuntimeError(f"T01 cancelled by signal {signum}; owned-resource cleanup required")
    for signum in (signal.SIGTERM, signal.SIGINT):
        signal.signal(signum, cancelled)
    try:
        require(run.case("original-resource-wrapper", ["R50", "REQ-T01"], "original resource receipt bound to this source/card", lambda: resource_receipt(run)), "resource ownership not established")
        if args.suite == "prepare":
            run.case("pinned-inputs-and-own-media", ["REQ-T01"], "npm ci, locked Cargo fetch, generated 1080p30 MP4/HLS", lambda: prepare(run))
        elif args.suite == "build":
            run.case("separate-app-builds", ["REQ-T01"], "production excludes test harness; signed local test app", lambda: build(run))
        elif args.suite == "negatives":
            run.case("negative-regressions", ["REQ-T01"], "all rejection tests actually ran", lambda: regressions(run))
        else:
            all_cases(run)
    except Exception as error:
        run.case("unhandled-prerequisite", list(IDS), "complete native gate", lambda: require(False, f"{type(error).__name__}: {error}"))
    return run.save()


if __name__ == "__main__":
    sys.exit(main())
