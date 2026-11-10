"""T12 formal App -> PlaybackView -> Api -> real Host -> gateway -> WKWebView.

Run only inside the original resource wrapper. Native callback track/line inputs
are synthetic; all business envelopes, handles and resource URLs come from Host.
"""
import argparse
import datetime as dt
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import tempfile
import threading
import time
import uuid
import xml.etree.ElementTree as ET

from t00 import file_hash, read_json, require, workflow, write_json
from t10 import resources
from t11 import Acceptance as NativeAcceptance
from t09 import redact
import t01

ROOT = Path(__file__).resolve().parents[2]
IDS = ("R08", "R23", "R30", "R32", "R34", "R48", "REQ-T12")
LIVE_TEST = "host::maccms::tests::t08::t09::t12::t12_host_formal_webview_bridge"


def summaries(cases):
    rows = []
    for identifier in IDS:
        children = [c for c in cases if identifier in c["requirements"]]
        rows.append(dict(id=identifier, children=[c["id"] for c in children],
                         result="PASS" if children and all(c["result"] == "PASS" for c in children) else "FAIL"))
    return rows


class Acceptance(NativeAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T12-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def redact_output(self, value):
        return redact(value)

    def save(self):
        self.case("source-stability", ["REQ-T12"], "exact tested source unchanged", lambda: require(workflow.snapshot() == self.source, "source changed during A1"))
        rows = summaries(self.cases)
        counts = dict(required=len(self.cases), ran=len(self.cases), passed=sum(c["result"] == "PASS" for c in self.cases),
                      failed=sum(c["result"] != "PASS" for c in self.cases), skipped=0)
        result = dict(card="T0.12", original_card="T12", suite="all", utc_started=self.started,
                      utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
                      tested_source_digest=workflow.snapshot(), protected=workflow.protected_hash(),
                      platform=platform.platform(), arch=platform.machine(), cases=self.cases, summaries=rows,
                      counts=counts, commands=self.commands, observed=self.observed,
                      native_counts=self.observed.get("native_counts", dict(required=1, ran=0, passed=0, failed=0, skipped=0, status="NOT RUN")),
                      deployment="ordinary native processes and owned loopback fixtures; no cluster",
                      limitations=dict(callbacks="synthetic native two-line/VTT callbacks supplement real MacCMS resolve; real original Cloudstream remains N02",
                                       performance="local CPU/RSS observations; original 4096MiB sampled budget, no new memory/CPU threshold; product endurance per ADR-020",
                                       platforms="macOS arm64 WKWebView; other platforms NOT RUN"))
        write_json(self.report_path, result)
        rows = self.cases + rows
        xml = ET.Element("testsuite", name="T12-all", tests=str(len(rows)), errors="0", skipped="0", failures=str(sum(r["result"] != "PASS" for r in rows)))
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T12", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T12 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return int(counts["failed"] > 0 or any(r["result"] != "PASS" for r in rows))


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-reporter=tap", "--test-concurrency=1", "tests/player.test.ts", "tests/playback.test.ts", "tests/api.test.ts", "tests/t12-webview.test.ts"])
    count = re.search(r"^# tests (\d+)$", output, re.M)
    require(count and int(count[1]) >= 40 and re.search(r"^# fail 0$", output, re.M), "nonzero behavior inventory or passing count missing")
    run.observed["frontend_tests"] = dict(tests=int(count[1]), failed=0)
    run.command(["npm", "run", "check"])
    return run.observed["frontend_tests"]


def owned_media(run, folder, *, seconds=2):
    for name, color, frequency in (("a", "red", 440), ("b", "blue", 880)):
        run.command(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", f"color=c={color}:size=1920x1080:rate=30",
                     "-f", "lavfi", "-i", f"sine=frequency={frequency}:sample_rate=48000", "-t", str(seconds), "-c:v", "libx264", "-preset", "ultrafast",
                     "-pix_fmt", "yuv420p",
                     "-g", "30", "-c:a", "aac", "-movflags", "+faststart", str(folder / f"{name}.mp4")], timeout=120)
    run.command(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i", str(folder / "a.mp4"), "-map", "0", "-c", "copy", "-f", "hls",
                 "-hls_time", "1", "-hls_list_size", "0", "-hls_segment_filename", str(folder / "seg-%d.ts"), str(folder / "index.m3u8")], timeout=120)
    inventory = []
    for path in sorted(folder.iterdir()):
        inventory.append(dict(name=path.name, bytes=path.stat().st_size, sha256=file_hash(path)))
    require(len(inventory) >= 5, "own H264/AAC/HLS inventory incomplete")
    run.observed["media"] = dict(license="CC0-1.0; generated solid color and sine", width=1920, height=1080, fps=30, seconds=seconds,
                                frequencies_hz=[440, 880], files=inventory)


class Bridge:
    def __init__(self, run, folder, media):
        self.run, self.folder, self.sequence, self.lock = run, folder, 0, threading.Lock()
        argv = workflow.runtime.resolve_argv(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", LIVE_TEST, "--", "--exact", "--test-threads=1", "--nocapture"])
        path = run.folder / "real-host.log"
        self.record = dict(argv=argv, exit=None, log=str(path.relative_to(ROOT)))
        run.commands.append(self.record)
        self.log = path.open("w", encoding="utf-8")
        self.process = subprocess.Popen(argv, cwd=ROOT, env=dict(os.environ, T12_BRIDGE_DIR=str(folder), T12_MEDIA=str(media)), stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        self.token = workflow.runtime.identity(self.process.pid)
        self.known = {}
        self.lines = []
        def collect():
            for line in self.process.stdout:
                safe = redact(line)
                if safe != line:
                    run.observed["host_log_sanitized"] = True
                self.lines.append(safe)
                self.log.write(safe)
                self.log.flush()
        self.reader = threading.Thread(target=collect, name="t12-host-log", daemon=True)
        self.reader.start()

    def wait(self, path, timeout=60):
        deadline = time.monotonic() + timeout
        while not path.exists():
            require(self.process.poll() is None, "Real Host bridge exited; inspect real-host.log")
            require(time.monotonic() < deadline, "Real Host bridge response timed out")
            time.sleep(.01)
        return read_json(path)

    def invoke(self, value):
        with self.lock:
            self.sequence += 1
            path = self.folder / f"request-{self.sequence}.json"
            temporary = path.with_suffix(".tmp")
            write_json(temporary, value)
            temporary.replace(path)
            return self.wait(self.folder / f"response-{self.sequence}.json")

    def finish(self):
        self.invoke(dict(operation="finish"))
        self.process.wait(timeout=30)
        self.reader.join(timeout=5)
        self.record["exit"] = self.process.returncode
        require(self.process.returncode == 0 and not self.reader.is_alive(), "Real Host bridge did not finish cleanly")
        vectors = [json.loads(line.split("T12_VECTOR:", 1)[1]) for line in self.lines if "T12_VECTOR:" in line]
        require(len(vectors) == 1 and vectors[0]["opens"] >= getattr(self, "minimum_opens", 51) and vectors[0]["closed"] is True and vectors[0]["active_requests"] == 0, "Real Host lifetime evidence missing")
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in "".join(self.lines), "Actual named native test did not pass")
        require(not self.run.observed.get("host_log_sanitized"), "Native output contained credentials; stored log was sanitized")
        self.run.observed["host"] = vectors[0]
        self.run.observed["native_counts"] = dict(required=1, ran=1, passed=1, failed=0, skipped=0, status="PASS")

    def close(self):
        self.known.update({r["pid"]: r for r in workflow.runtime.family(self.process.pid, self.token)})
        if self.process.poll() is None:
            workflow.runtime.terminate(list(self.known.values()))
        self.process.wait(timeout=10)
        self.reader.join(timeout=5)
        self.process.stdout.close()
        self.log.close()
        self.record["exit"] = self.process.returncode
        require(not self.reader.is_alive() and not any(workflow.runtime.alive(r) for r in self.known.values()), "Owned Host processes remain")


def validate_cycle(value, index, *, check_audio=True):
    sample = value["sample"]
    require(value["index"] == index and value["old_tracks_disabled"] is True and value["session_changed"] is True
            and value["new_frames"] > 1 and value["actual_gateway"] is True, "Episode/old frame/track/session binding differs")
    require(sample["video_count"] == 1 and sample["tracks"] == 1 and sample["diagnostics"]["hls_instances"] <= 1
            and sample["diagnostics"]["listening"] is True and sample["diagnostics"]["retiring"] == 0
            and sample["width"] == 1920 and sample["height"] == 1080 and sample["media_error"] is None,
            "Duplicate player, retained transport, failed media or wrong decode")
    red, green, blue, alpha = sample["pixel"]
    is_red = index % 2 == 0 or index == 51
    require(alpha == 255 and (green < 50 and red > 180 and blue < 50 if is_red else green < 64 and blue > 180 and red < 50), "Old/incorrect episode frame is still visible")
    frequency = 440 if is_red else 880
    if check_audio and index == 51:
        require(value["native"]["audio_playing"] is True, "Native HLS audio is not playing")
    elif check_audio:
        require(sample["audio_state"] == "running" and sample["session_bound"] is True
                and sample["audio_rms"] > .001 and abs(sample["audio_peak_hz"] - frequency) < 40, "Old or missing episode audio")
    host = value["host"]
    require(host["live_sessions"] == 1 and host["requests"] == 0 and host["old_http_status"] == 410,
            "Suspended Session/request or old capability remains")
    require(not value["errors"], "Formal App console/unhandled errors")


def validate_stop(value):
    sample = value["sample"]
    require(value["source_removed"] is True and value["frames_after_stop"] <= 1 and sample["video_count"] == 1
            and sample["paused"] is True and sample["ready_state"] == 0 and sample["network_state"] == 0
            and sample["source_present"] is False and sample["tracks"] == 0 and sample["audio_rms"] < .001
            and sample["diagnostics"]["listening"] is False and sample["diagnostics"]["hls_instances"] == 0
            and sample["diagnostics"]["retiring"] == 0 and value["host"]["live_sessions"] == 0
            and value["host"]["requests"] == 0 and value["host"]["source_active"] == 0,
            "Stop retained old audio/frames/HTTP/session/listener/media resources")


def validate_eos(value):
    require(value["ended"] is True and value["dom_events"] == 1 and len(value["events"]) == 1, "Native EOS missing or duplicated")
    before, final = value["before"], value["final"]
    event = value["events"][0]
    require(before["session_id"] and event["trusted"] is True and event["state"]["ended"] is True
            and final["ended"] is True and final["phase"] == "ended"
            and all(state["epoch"] == before["epoch"] and state["session_id"] == before["session_id"]
                    for state in (event["state"], final)), "EOS must belong to the current epoch/session and trusted native event")
    require([stage["stage"] for stage in value["stages"]] == (["resume", "seek"] if before["paused"] else ["seek"]),
            "EOS playback/seek actions differ from the initial paused state")


def validate_image_expiry(value):
    require(value["expired_http_status"] == 410 and value["dto_refreshed"] is True and value["decoded"] is True
            and value["owning_ref"] == value["refreshed_ref"] and value["image_error"] == ""
            and value["page_epoch"] == value["page_epoch_after"]
            and value["expected_host_epoch"] == value["host_epoch_after"] and value["expected_host_epoch"] is not None,
            "Expired image HTTP/owning DTO/decoded image/epoch recovery differs")
    require(value["stages"] == ["before-eviction", "4096-lru-evicted", "expired-http-read", "host-dto-published",
                                "dto-recovery-finished", "refreshed-image-decoded"], "Real image recovery stages missing or reordered")
    errors = [event for event in value["events"] if event["type"] == "error"]
    loads = [event for event in value["events"] if event["type"] == "load"]
    require(len(errors) == 1 and len(loads) == 1, "Native image error/load must each occur once")
    error, loaded = errors[0], loads[0]
    require(error["trusted"] is True and error["original_dto"] is True and error["original_url"] is True and error["owning_url"] is True
            and error["resource_id"] == value["resource_id"] and error["natural_width"] == 0
            and loaded["trusted"] is True and loaded["original_dto"] is False and loaded["natural_width"] > 0
            and loaded["owning_url"] is True and loaded["resource_id"] == value["refreshed_resource_id"]
            and error["page_epoch"] == loaded["page_epoch"] == value["page_epoch"]
            and 0 <= error["at_ms"] < loaded["at_ms"] < 30000,
            "Actual native expired img error and refreshed img load evidence missing")
    calls = value["calls"]
    require([row["command"] for row in calls] == ["get_resource_status", "get_details"], "Image must query and refresh exactly once")
    query, refresh = calls
    for row in calls:
        require(isinstance(row["request_id"], str) and str(uuid.UUID(row["request_id"])) == row["request_id"]
                and row["request_id"] == row["response_request_id"] and row["engine_epoch"] == value["expected_host_epoch"]
                and error["at_ms"] <= row["started_ms"] <= row["finished_ms"] < loaded["at_ms"],
                "Image call request/Host identity or native event ordering differs")
    require(query["request_id"] != refresh["request_id"] and query["kind"] == query["response_kind"] == "image"
            and query["target_id"] == query["response_target_id"] == value["resource_id"]
            and query["state"] == "expired" and query["code"] == "SESSION_EXPIRED"
            and query["finished_ms"] - query["started_ms"] <= 3000
            and query["finished_ms"] <= refresh["started_ms"] and refresh["code"] is None
            and refresh["ref"] == refresh["response_ref"] == value["owning_ref"],
            "Image query expiry/3s budget/target or refreshed source revision differs")
    for command in ("get_resource_status", "get_details"):
        require(value["after"]["command_counts"].get(command, 0) - value["before"]["command_counts"].get(command, 0) == 1,
                "Real Host image query/refresh count differs")


def validate_errors(value):
    outcomes, calls, events = value["outcomes"], value["calls"], value["events"]
    require([row["mode"] for row in outcomes] == [1, 2, 3, 4, "unauthorized-query"], "Complete native error matrix is required")
    for index, row in enumerate(outcomes[:3]):
        mode, final, opened = row["mode"], row["final"], row["opened"]
        expected = "MEDIA_UNSUPPORTED" if mode == 3 else "UPSTREAM_FAILED"
        require(final["phase"] == "failed" and row["code"] == final["code"] == expected
                and final["pending_requests"] == 0 and final["assigned_bound"] is True
                and (final["current_path"] is None or final["current_path"] == final["session_path"])
                and row["before"]["stream_kind"] == opened["stream_kind"] == final["stream_kind"] == "progressive",
                "Native error classification or progressive codec premise differs")
        for key in ("epoch", "host_epoch", "session_id", "source_id", "source_revision", "episode_ref", "session_path"):
            require(opened[key] == final[key], "Error diagnosis belongs to another loading attempt")
        requests = [request for request in row["requests"] if request["mode"] == mode and request["target"].startswith("/media/")]
        require(requests and all(request["target"].split("?")[0].endswith(".mp4")
                and request["status"] == {1: 401, 2: 502, 3: 200}[mode]
                and request["mime"] == ("video/mp4" if mode == 3 else "text/plain") for request in requests),
                "Actual upstream media path/status/MIME differs from the error stimulus")
        next_row = outcomes[index + 1]
        queries = calls[row["call_start"]:next_row["call_start"]]
        require(len(queries) == 1, "Native video loading attempt must query exactly once")
        query = queries[0]
        response, before = query["response"], query["before"]
        require(query["mode"] == mode and query["command"] == "get_resource_status"
                and re.fullmatch(r"[0-9a-f-]{36}", query["request_id"], re.I)
                and query["request_id"] == response["request_id"] and response["engine_epoch"] == final["host_epoch"]
                and query["kind"] == response["data"]["kind"] == "playback"
                and query["target_id"] == response["data"]["target_id"] == final["session_id"]
                and all(before[key] == final[key] for key in ("epoch", "host_epoch", "session_id", "source_id", "source_revision", "episode_ref"))
                and 0 <= query["finished_ms"] - query["started_ms"] <= 3000,
                "Native status query identity/target/3s budget differs")
        status = response["data"]
        require(status["state"] == ("ready" if mode == 3 else "failed")
                and (status["error"] is None if mode == 3 else status["error"]["code"] == expected),
                "Gateway diagnosis differs from the native media stimulus")
        native = [event for event in events[row["event_start"]:next_row["event_start"]] if event["target"] == "video"]
        require(native and all(event["trusted"] is True and event["mode"] == mode and event["epoch"] == final["epoch"]
                and event["session_id"] == final["session_id"] and event["media_error"] for event in native)
                and native[0]["at_ms"] <= query["started_ms"], "Trusted current video error must precede its status query")
        media_code = final["media_error"]["code"]
        require(row["media_error"] == media_code and media_code in (1, 2, 3, 4)
                and (mode != 3 or media_code in (3, 4))
                and final["error"]["details"]["original_error"]["details"]["media_code"] == media_code,
                "Original native MediaError was lost or codec bytes did not reach the decoder")
        repeat = row.get("repeat")
        require(isinstance(repeat, dict), "Completed diagnosis needs a same-session native repeat")
        before_repeat, after_repeat = repeat["before"], repeat["after"]
        require(repeat["stimulus"] == "native video.load, same session URL after completed query"
                and query["finished_ms"] <= repeat["started_ms"] <= repeat["finished_ms"]
                and row["event_start"] < repeat["event_start"] < next_row["event_start"]
                and repeat["calls_before"] == repeat["calls_after"] == row["call_start"] + 1,
                "Repeat must follow query completion without querying again")
        for snapshot in (before_repeat, after_repeat):
            require(all(snapshot[key] == final[key] for key in
                        ("epoch", "host_epoch", "session_id", "source_id", "source_revision", "episode_ref", "session_path"))
                    and snapshot["assigned_bound"] is True and snapshot["pending_requests"] == 0
                    and snapshot["phase"] == "failed" and snapshot["code"] == expected
                    and snapshot["error"] == final["error"]
                    and snapshot["action"] == final["action"] == ("" if mode == 3 else "retry"),
                    "Repeated native error lost the completed classification, action or loading identity")
        repeated_events = events[repeat["event_start"]:next_row["event_start"]]
        require(any(event["target"] == "video" and event["trusted"] is True
                    and repeat["started_ms"] <= event["at_ms"] <= repeat["finished_ms"] for event in repeated_events)
                and after_repeat["media_error"] and (mode == 3 or repeat["retry_button"] is True),
                "Repeat requires a trusted native error and the formal retry control")
    require(value["track_error"] == "UPSTREAM_FAILED" and value["reconnects"] == 1, "Native track/reconnect behavior differs")


def process_sample(session, bridge=None):
    # Keep the original aggregate observation and its budget enforcement intact.
    # Detail reads follow it; their sum can differ as the live processes change.
    rss = session.observe()
    cpu = 0
    processes = []
    roles = {pid: role for role, pid in session.record.get("webkit_pids", {}).items() if pid > 0}
    roles[session.process.pid] = "application"
    if session.record.get("wake_guard"):
        roles[session.record["wake_guard"]["pid"]] = "wake_guard"
    def detail(pid, token, role):
        row = dict(pid=pid, identity=token, role=role, alive=False, rss_bytes=None, cpu_seconds=None)
        process = workflow.runtime.process_for(pid, token)
        if process:
            try:
                times = process.cpu_times()
                seconds = times.user + times.system
                row.update(alive=True, name=process.name(), rss_bytes=process.memory_info().rss, cpu_seconds=seconds)
            except workflow.runtime.psutil.NoSuchProcess:
                row.update(alive=False, rss_bytes=None, cpu_seconds=None)
        return row
    for pid, token in session.owned.items():
        row = detail(pid, token, roles.get(pid, "owned_child"))
        cpu += row["cpu_seconds"] or 0
        processes.append(row)
    # Host/observer memory belongs to the combined budget, outside owned_rss.
    support = [detail(os.getpid(), workflow.runtime.identity(os.getpid()), "python_observer")]
    if bridge is not None:
        support.append(detail(bridge.process.pid, bridge.token, "real_host"))
        for member in workflow.runtime.family(bridge.process.pid, bridge.token):
            if member["pid"] != bridge.process.pid:
                support.append(detail(member["pid"], member["identity"], "real_host_child"))
    result_log = session.run.folder / f"results-{session.record['index']}.jsonl"
    return dict(utc=dt.datetime.now(dt.timezone.utc).isoformat(), monotonic=time.monotonic(), owned_rss_bytes=rss, owned_cpu_seconds=cpu,
                processes=processes, support_processes=support, detail_rss_bytes=sum(row["rss_bytes"] or 0 for row in processes),
                observer=dict(command_results=len(session.record.get("commands", [])), queued_commands=session.fixture.commands.qsize(),
                              queued_results=session.fixture.results.qsize(), queued_samples=session.fixture.samples.qsize(),
                              retained_samples=len(session.samples), result_log_bytes=result_log.stat().st_size if result_log.exists() else 0,
                              host_log_lines=len(bridge.lines) if bridge else None,
                              host_requests=bridge.sequence if bridge else None))


def validate_source_rematch(value):
    require(value["stages"] == ["playing", "confirmation-visible", "cancelled", "playing-again"], "Source change confirmation flow differs")
    during = value["during_confirmation"]
    require(during["session"] is None and during["videos"] == 0 and during["route"] == "details", "Old playback remained during source confirmation")
    require(value["recovered"] is True and value["host"]["live_sessions"] == 0, "Playback did not resume or release after source confirmation")


RSS_LIMIT_BYTES = 512 * 1024 * 1024
RSS_GROWTH_BYTES = 64 * 1024 * 1024


def rss_windows(samples):
    require(len(samples) == 51 and all(isinstance(s["owned_rss_bytes"], (int, float))
            and not isinstance(s["owned_rss_bytes"], bool) and math.isfinite(s["owned_rss_bytes"])
            and s["owned_rss_bytes"] >= 0 for s in samples), "All 51 raw CPU/RSS samples are required")
    return [statistics.median(s["owned_rss_bytes"] for s in samples[i:i+5]) for i in range(1, 51, 5)]


def validate_rss_windows(medians):
    require(len(medians) == 10, "All ten five-switch RSS windows are required")
    # ADR-044: fifty switches may warm caches; only sizable growth indicates a leak.
    require(max(medians) <= RSS_LIMIT_BYTES, "Owned RSS exceeds 512MiB")
    require(medians[-1] - medians[0] <= RSS_GROWTH_BYTES, "Owned RSS leak: grew over 64MiB across fifty switches")


def diagnose_rss_failure(run, session, bridge):
    """Post-verdict only: preserve the failed series, then isolate owned lifetimes.

    VM summaries are diagnostic categories, never substituted for aggregate RSS.
    A denied probe is recorded once; no permission changes or retries follow.
    """
    stages = []
    run.observed["rss_failure_diagnostics"] = stages
    maps_available = platform.system() == "Darwin" and Path("/usr/bin/vmmap").is_file()
    for stage, action in (("live", None), ("media-stopped", "stop"),
                          ("observer-released", "release-observer"), ("view-disposed", "dispose")):
        row = dict(stage=stage, memory_maps=[])
        stages.append(row)
        try:
            if action:
                row["action"] = session.command("t12", action=action)
                if action == "stop":
                    validate_stop(row["action"])
            row["process_sample"] = process_sample(session, bridge)
            for process in row["process_sample"]["processes"]:
                if process["role"] not in ("renderer", "gpu", "GPU") or not process["alive"]:
                    continue
                record = dict(pid=process["pid"], identity=process["identity"], role=process["role"], status="NOT RUN")
                row["memory_maps"].append(record)
                if not maps_available:
                    record["reason"] = "vmmap unavailable or an earlier diagnostic probe failed"
                    continue
                command_count = len(run.commands)
                try:
                    require(workflow.runtime.alive(process), "memory map process identity changed")
                    run.command(["/usr/bin/vmmap", "-summary", str(process["pid"])], timeout=10)
                    require(workflow.runtime.alive(process), "memory map process identity changed")
                    record.update(status="CAPTURED", log=run.commands[-1]["log"])
                except Exception as cause:
                    record.update(status="FAILED", error=redact(str(cause)))
                    if len(run.commands) > command_count:
                        record["log"] = run.commands[-1]["log"]
                    maps_available = False
        except Exception as cause:
            row["error"] = redact(str(cause))
            # Keep the original RSS failure and leave remaining cleanup to the
            # existing identity-bound finally path when a stage cannot finish.
            break


def webview(run, *, check_security=False, after_start=None, build=True, before_identity=None):
    run.observed["gui_session"] = t01.native_gui_session()
    require(run.observed["gui_session"]["screen_locked"] is not True, "macOS desktop locked; unlock before packaged WKWebView acceptance")
    t01.check_media()
    if build:
        t01.build(run)
    receipt = t01.check_build(run.source)
    executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/yingxiang-video"
    session, bridge = None, None
    with tempfile.TemporaryDirectory(prefix="t12-live-", dir=run.folder) as temporary:
        folder = Path(temporary)
        media = folder / "media"; media.mkdir()
        owned_media(run, media)
        try:
            bridge = Bridge(run, folder, media)
            ready = bridge.wait(folder / "ready.json", timeout=180)
            with t01.Fixture(t01.MEDIA, invoke=bridge.invoke) as fixture:
                session = t01.NativeSession(run, fixture, executable, 1)
                session.wait(0, 30)
                if before_identity is not None:
                    before_identity(session)
                if check_security:
                    run.observed["webview_security"] = session.command("security")
                identity = session.command("identity")
                require(identity["origin"] == "tauri://localhost", "Incorrect packaged origin")
                start = session.command("t12", action="start", payload=ready, timeout=60)
                session.record["commands"][-1]["args"] = dict(action="start", payload="actual Host contexts/DTO; capability URLs transient and redacted")
                require(start["nonblank"] is True and start["framework_overlay"] is False and not start["errors"] and start["sample"]["video_count"] == 1, "Formal production App did not render")
                run.observed["formal_entry"] = start
                if after_start is not None:
                    after_start(session, bridge)
                samples = [process_sample(session, bridge)]
                run.observed["process_samples"] = samples
                cycles = []
                for index in range(1, 51):
                    value = session.command("t12", action="switch", payload=index, timeout=40)
                    # Keep the rejected raw cycle as well as every preceding one.
                    cycles.append(value)
                    run.observed["cycles"] = cycles
                    validate_cycle(value, index)
                    samples.append(process_sample(session, bridge))
                run.observed["cycles"] = cycles
                run.observed["process_samples"] = samples
                # Preserve every raw sample for the leak verdict and its failure diagnostics.
                medians = rss_windows(samples)
                run.observed["rss_windows"] = medians
                try:
                    validate_rss_windows(medians)
                except ValueError:
                    try:
                        diagnose_rss_failure(run, session, bridge)
                    except Exception as cause:
                        run.observed["rss_diagnostic_error"] = redact(str(cause))
                    raise
                hls = session.command("t12", action="switch", payload=51, timeout=40)
                validate_cycle(hls, 51)
                require(hls["sample"]["diagnostics"]["engine"] == "native", "WKWebView native HLS capability was ignored")
                run.observed["native_hls"] = hls
                for action in ("controls", "expire", "image-expire", "errors", "autoplay", "unknown-duration", "eos"):
                    run.observed[action] = session.command("t12", action=action, timeout=90)
                require(run.observed["controls"]["resolution_retained"] is True and run.observed["controls"]["paused"] is True
                        and run.observed["controls"]["session_changed"] is True, "Line switch/paused seek changed ownership")
                require(all(run.observed["expire"][key] is True for key in ("session_changed", "restored", "paused", "recovered_once")), "Expired ResumeSnapshot recovery changed")
                validate_image_expiry(run.observed["image-expire"])
                validate_errors(run.observed["errors"])
                require(run.observed["autoplay"]["continue_button"] is True and run.observed["autoplay"]["resumed"] is True, "Actual continue button did not recover autoplay")
                require(run.observed["unknown-duration"]["disabled"] is True and run.observed["unknown-duration"]["unknown_text"] is True, "Unknown duration UI/seek differs")
                validate_eos(run.observed["eos"])
                for width in (1200, 390):
                    run.observed[f"layout-{width}"] = session.command("t12", action="layout", payload=width)
                stop = session.command("t12", action="stop")
                validate_stop(stop)
                run.observed["stop"] = stop
                run.observed["idle_process_sample"] = process_sample(session, bridge)
                rematch = session.command("t12", action="source-rematch", timeout=60)
                validate_source_rematch(rematch)
                run.observed["source-rematch"] = rematch
                disposed = session.command("t12", action="dispose")
                require(disposed["audio_closed"] is True and disposed["videos"] == 0 and not disposed["errors"], "Unmount retains native media/AudioContext or errors")
                run.observed["disposed"] = disposed
                session.close()
                require(fixture.active == 0, "Owned fixture HTTP remains")
                run.observed["transport"] = fixture.summary()
            bridge.finish()
            return dict(actual_switches=len(cycles), separate_hls_cycle=True, frontend="production App/PlaybackView/state/Api/player", native="actual Host invoke/gateway and packaged WKWebView",
                        browser="Browser plugin not available; required target uses existing native WKWebView harness", cleanup="owned processes/HTTP/media/audio closed")
        finally:
            try:
                if session is not None and not session.closed:
                    session.close(graceful=False)
            finally:
                if bridge is not None:
                    bridge.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    options = parser.parse_args()
    require(options.report.resolve() == ROOT / "evidence/acceptance/T12-all.json" and options.junit.resolve() == ROOT / "evidence/acceptance/T12-all.xml", "Exact T12 A1 output paths required")
    run = Acceptance(options.report, options.junit)
    ready = run.case("resource-ownership", ["REQ-T12"], "original source/card-bound 4096MiB RSS sampled backend", lambda: resources(run, card="T0.12"))
    def oracles():
        output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t12.py"])
        count = re.search(r"Ran (\d+) tests?", output)
        require(count and int(count[1]) >= 6, "Nonzero independent negative-oracle inventory is missing")
        return dict(tests=int(count[1]))
    run.case("acceptance-oracle-negatives", ["REQ-T12"], "independent corrupted audio/frame/session/report evidence is rejected",
             oracles if ready else lambda: require(False, "NOT RUN: resource gate failed"))
    passed = run.case("formal-controller-and-errors", IDS, "strict events, 250ms, old epochs, once recovery, native resource error and identity/late-result counterexamples",
                      lambda: frontend(run) if ready else require(False, "NOT RUN: resource gate failed"))
    run.case("packaged-formal-host-webview", IDS, "50 real episode switches and native audio/frame/color/HTTP/session/track release plus native HLS, pause/seek/EOS/errors/LRU/CPU/RSS",
             lambda: webview(run) if ready and passed else require(False, "NOT RUN: frontend prerequisite failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
