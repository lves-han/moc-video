"""T22 current packaged Rust/WKWebView product measurements, original run.py only.

A1 performs five process-cold launches. A2 revalidates their receipt and raw
artifacts, then executes fresh 600s playback, fifty switches and 180s throughput.
"""
import argparse
import contextlib
import datetime as dt
import json
import math
import os
from pathlib import Path
import platform
import plistlib
import re
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import xml.etree.ElementTree as ET

import t01
import t02
import t10
import t12
import t21
from t00 import file_hash, read_json, require, workflow, write_json

ROOT = Path(__file__).resolve().parents[2]
IDS = ["R09", "R47", "R48", "C-RESOURCE", "REQ-T21", "REQ-T22"]
STREAM = "gateway::tests::t22::t22_gateway_twenty_mbps_three_minutes"
FAULTS = ["t09_slow_timeout_truncation_disconnect_and_close_cancel",
          "t09_host_inflight_media_revoke_and_control_responsive",
          "t18_unusable_database_path_reports_and_recovers_with_the_same_snapshot"]


def utc():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def numeric(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def p95(samples):
    require(samples and all(numeric(row) and row >= 0 for row in samples), "Finite nonempty raw response samples required")
    return sorted(samples)[math.ceil(.95 * len(samples)) - 1]


def append_sample(path, sample):
    with path.open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(sample) + "\n")


def gpu_probe():
    argv = ["/usr/sbin/ioreg", "-r", "-c", "AGXAccelerator", "-a"]
    result = subprocess.run(argv, capture_output=True, check=True, timeout=15)
    stats = [row["PerformanceStatistics"] for row in plistlib.loads(result.stdout)
             if isinstance(row.get("PerformanceStatistics"), dict)]
    require(stats, "Actual AGX PerformanceStatistics unavailable")
    # Keep raw numeric performance counters; device identifiers never enter logs.
    counters = [{key: value for key, value in row.items() if numeric(value)} for row in stats]
    require(any(any("tilization" in key or "ctivity" in key for key in row) for row in counters),
            "GPU utilization/activity sample unavailable")
    return dict(utc=utc(), scope="whole device, includes other applications", counters=counters, argv=argv)


def environment_probe():
    def get(*argv):
        return subprocess.run(argv, check=True, capture_output=True, text=True, timeout=15).stdout.strip()
    displays = json.loads(get("/usr/sbin/system_profiler", "-json", "SPDisplaysDataType"))
    return dict(os=platform.platform(), macos=platform.mac_ver()[0], arch=platform.machine(),
        model=get("/usr/sbin/sysctl", "-n", "hw.model"), cpu=get("/usr/sbin/sysctl", "-n", "machdep.cpu.brand_string"),
        memory_bytes=int(get("/usr/sbin/sysctl", "-n", "hw.memsize")), cpu_count=os.cpu_count(),
        gpu=[{key: row[key] for key in ("sppci_model", "spdisplays_vendor", "spdisplays_cores", "spdisplays_metal") if key in row}
             for row in displays["SPDisplaysDataType"]], gpu_sample=gpu_probe())


class Acceptance(t02.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.26", "T22", IDS

    def __init__(self, suite, report, junit):
        super().__init__(report.resolve(), junit.resolve())
        self.suite = suite

    def save(self):
        required = {"resource-ownership", "independent-oracle-negatives", "reference-machine"}
        required |= ({"network-source-file-fault-recovery", "five-cold-starts-ui-faults-and-long-test-preparation"}
                     if self.suite == "startup-ui" else {"same-source-a1-five-startup-receipts", "current-packaged-build",
                     "gateway-ten-minute-playback-and-fifty-switches", "twenty-mbps-three-minute-stream"})
        actual = {row["id"].split("/", 1)[1] for row in self.cases}
        self.case("required-behavior-inventory", IDS, sorted(required),
                  lambda: require(required <= actual, "Required behavior cases missing: " + str(sorted(required - actual))))
        self.case("stable-tested-source", IDS, "same source/protected throughout suite",
                  lambda: require(workflow.snapshot() == self.source, "Source changed during suite"))
        summaries = []
        for identifier in IDS:
            children = [row for row in self.cases if identifier in row["requirements"]]
            summaries.append(dict(id=identifier, children=[row["id"] for row in children],
                result="PASS" if children and all(row["result"] == "PASS" for row in children) else "FAIL"))
        counts = dict(required=len(self.cases), ran=len(self.cases), passed=sum(row["result"] == "PASS" for row in self.cases),
                      failed=sum(row["result"] != "PASS" for row in self.cases), skipped=0)
        platforms = []
        for target in t01.DEFERRED:
            row = dict(card=self.CARD, target=target, status="NOT RUN", reason="ADR-005/006; no target machine",
                tested_source_digest=workflow.snapshot(), required_target="macos-arm64", utc=utc(),
                reproduction_current_arm64=["{python}", "-B", "tests/acceptance/t22.py", "--suite", self.suite,
                    "--report", str(self.report_path.relative_to(ROOT)), "--junit", str(self.junit_path.relative_to(ROOT))])
            path = self.folder / (target + ".json")
            write_json(path, row)
            platforms.append(dict(row, report=str(path.relative_to(ROOT))))
        artifacts = [dict(path=str(path.relative_to(ROOT)), sha256=file_hash(path), bytes=path.stat().st_size)
            for path in sorted(self.folder.rglob("*")) if path.is_file() and path.suffix in (".log", ".jsonl", ".json")
            and not any(part.endswith(".app") for part in path.parts) and path.name != "report.json"]
        report = dict(card=self.CARD, original_card="T22", suite=self.suite, utc_started=self.started, utc_finished=utc(),
            source_before=self.source, tested_source_digest=workflow.snapshot(), protected=workflow.protected_hash(),
            platform=sys.platform, arch=platform.machine(), os=platform.platform(), cases=self.cases, summaries=summaries,
            counts=counts, commands=self.commands, observed=self.observed, artifacts=artifacts,
            deployment="ordinary owned loopback fixtures and actual Rust/packaged macOS WKWebView; no cluster",
            limitations=dict(cold_start="fresh packaged Rust Host; window visibility confirmed through automation prototype, formal production UI separately exercised; OS disk caches retained",
                resource_backend="4096MiB process-tree plus identity-registered XPC RSS sampling; jobs2/thread1; transient overage possible",
                gpu="raw device-wide AGX counters and per-WebKit-GPU process CPU/RSS; other applications can contribute device load",
                audio="decoded media-element PCM RMS/frequency; physical speaker output uninstrumented"),
            platforms=platforms)
        write_json(self.report_path, report)
        immutable = self.folder / "report.json"
        write_json(immutable, report)
        print("T22_EVIDENCE_SHA256:" + file_hash(immutable), flush=True)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T22-" + self.suite, tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)))
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T22", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T22 behavior failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return int(counts["failed"] > 0 or any(row["result"] != "PASS" for row in summaries))


def environment(run):
    observed = json.loads(run.command(["{python}", "-B", str(Path(__file__).relative_to(ROOT)), "--probe-environment"], timeout=60))
    require(sys.platform == "darwin" and observed["arch"] == "arm64" and int(observed["macos"].split(".")[0]) >= 14,
            "Actual macOS14+ arm64 reference host required")
    require(observed["model"] and observed["cpu"] and observed["gpu"] and observed["memory_bytes"] > 0
            and all(row.get("sppci_model") for row in observed["gpu"]), "Actual reference CPU/GPU/memory inventory missing")
    versions = {}
    for command in ("rustc", "cargo", "node", "npm", "ffmpeg", "ffprobe"):
        versions[command] = run.command([command, "-version" if command.startswith("ff") else "--version"], timeout=30).splitlines()[0]
    webkit = plistlib.loads(Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist").read_bytes())
    observed.update(dependencies=versions, webkit={key: webkit.get(key) for key in ("CFBundleVersion", "CFBundleShortVersionString")},
        locks={name: file_hash(ROOT / name) for name in ("package-lock.json", "src-tauri/Cargo.lock")})
    run.observed["reference_machine"] = observed
    return observed


@contextlib.contextmanager
def packaged_paths(folder):
    previous = t01.DATA, t01.BUILD
    t01.DATA, t01.BUILD = folder, folder / "build.json"
    try:
        yield
    finally:
        t01.DATA, t01.BUILD = previous


def media(run, folder, prepare=t12.owned_media):
    # The short T12/21 collection remains unchanged. Only A2's first MP4 is long.
    prepare(run, folder)
    run.command(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "color=c=red:size=1920x1080:rate=30",
        "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000", "-t", "620", "-c:v", "libx264", "-preset", "ultrafast",
        "-pix_fmt", "yuv420p", "-g", "30", "-c:a", "aac", "-movflags", "+faststart", str(folder / "a.mp4")], timeout=300)
    probe = json.loads(run.command(["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", str(folder / "a.mp4")], timeout=30))
    video = next(row for row in probe["streams"] if row["codec_type"] == "video")
    audio = next(row for row in probe["streams"] if row["codec_type"] == "audio")
    require(video["codec_name"] == "h264" and audio["codec_name"] == "aac" and video["width"] == 1920
            and video["height"] == 1080 and video["r_frame_rate"] == "30/1" and float(probe["format"]["duration"]) >= 620,
            "Required owned 1080p30 H264/AAC endurance media missing")
    run.observed["endurance_media"] = dict(license="CC0-1.0; generated solid red and 440Hz sine", probe=probe,
        sha256=file_hash(folder / "a.mp4"), bytes=(folder / "a.mp4").stat().st_size)
    run.observed["media"]["files"] = [dict(name=path.name, bytes=path.stat().st_size, sha256=file_hash(path)) for path in sorted(folder.iterdir())]


def check_cleanup(session):
    require(session["exit"] == 0 and session["lingering"] == [] and session["forced_cleanup"] == []
            and session["wake_guard"]["exit"] == 0 and 0 < session["peak_combined_rss_bytes"] <= 4096 * 1024 ** 2,
            "Native/WebKit/wake guard cleanup or combined sampled budget failed")


def check_process(sample, native):
    owned = sample["processes"]
    support = sample["support_processes"]
    require({"application", "renderer"} <= {row["role"] for row in owned if row["alive"]}
            and {"python_observer", "real_host", "real_host_child"} <= {row["role"] for row in support if row["alive"]},
            "Actual application/renderer/observer/cargo/Rust Host process accounting incomplete")
    for row in owned + support:
        if row["alive"]:
            require(row["identity"] and row["pid"] > 0 and numeric(row["rss_bytes"]) and row["rss_bytes"] > 0
                    and numeric(row["cpu_seconds"]) and row["cpu_seconds"] >= 0, "Raw process identity/RSS/CPU missing")
    require(sample["detail_rss_bytes"] == sum(row["rss_bytes"] or 0 for row in owned), "Owned RSS detail total differs")
    for role, pid in native["webkit_pids"].items():
        if pid > 0:
            require(any(row["alive"] and row["pid"] == pid and row["role"] == role for row in owned), "Live WebKit role missing: " + role)


def check_startups(rows, sessions):
    require(len(rows) == len(sessions) == 5, "All five cold launches and process receipts required")
    identities = set()
    for row, session in zip(rows, sessions):
        require(row["pid"] == session["pid"] and row["identity"] == session["identity"], "Startup belongs to another process")
        identities.add((row["pid"], row["identity"]))
        require(row["host_ready"] is True and row["nonblank"] is True and row["framework_overlay"] is False
                and row["errors"] == [] and row["native"]["visible"] is True and row["native"]["focused"] is True,
                "Cold window/Host/console health failed")
        require(all(numeric(row[key]) and row[key] > 0 for key in ("ready_seconds", "health_seconds", "owned_rss_bytes")), "Raw startup duration/RSS missing")
        require(row["utc_finished"] > row["utc_started"] and len(row["responses"]) == 30
                and all(sample["valid"] is True and sample["command"] == "get_settings" for sample in row["responses"]),
                "Raw local no-network bridge response samples missing")
        p95([sample["milliseconds"] for sample in row["responses"]])
        check_process(row["process_sample"], row["native"])
        require(row["gpu"]["counters"], "Raw startup GPU sample missing")
        check_cleanup(session)
    require(len(identities) == 5, "Five distinct cold process identities required")
    return dict(launches=5, bridge_samples=150)


def check_ui(value):
    require(len(value["responses"]) == 10 and [row["zoom"] for row in value["layouts"]] == [1, 1.25, 1.5, 2]
            and all(row["no_overflow"] is True and row["controls"] > 0 and row["width"] > 0 for row in value["layouts"])
            and value["fullscreen"] is True and value["restored"] is True, "UI response/scale/fullscreen coverage missing")
    require(len(value["metadata_responses"]) == 30 and all(row["valid"] is True and row["command"] == "get_details"
            for row in value["metadata_responses"]), "Actual local no-external-network metadata response samples required")
    p95([row["milliseconds"] for row in value["responses"]])
    p95([row["milliseconds"] for row in value["metadata_responses"]])
    for row in value["layouts"]:
        native = row["native"]
        require(row["official_controls"] == 4 and row["play_pause"] is True
                and native["physical_width"] == 960 * native["scale_factor"]
                and native["physical_height"] == 640 * native["scale_factor"]
                and abs(row["width"] - 960 / row["zoom"]) <= 1
                and abs(row["height"] - (640 - native["top_content_inset"]) / row["zoom"]) <= 1,
                "Actual minimum 960x640 window, native zoom, or official skin interaction differs")


def startup_ui(run):
    run.observed["gui_session"] = t01.native_gui_session()
    require(run.observed["gui_session"]["screen_locked"] is not True, "macOS desktop locked; unlock before cold-start/UI acceptance")
    folder = run.folder / "packaged"
    folder.mkdir()
    run.observed["packaged_folder"] = str(folder.relative_to(ROOT))
    with packaged_paths(folder):
        t01.check_media()
        receipt = t01.build(run)
        run.observed["build"] = receipt
        executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/yingxiang-video"
        run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "--no-run"], timeout=600)
        rows = run.observed.setdefault("startups", [])
        with tempfile.TemporaryDirectory(prefix="t22-ready-", dir=run.folder) as temporary:
            live = Path(temporary)
            owned = live / "media"; owned.mkdir()
            media(run, owned)
            # Prepare the exact A2 media/AGX/20Mbps instrumentation in A1.
            run.observed["long_test_plan"] = dict(playback_seconds=600, throughput_seconds=180, throughput_bps=20_000_000,
                switches=50, sample_interval_seconds=5, stream_sample_interval_seconds=1, memory_mib=4096,
                deployment="ordinary loopback fixtures; no cluster", cleanup="identity-bound native/XPC shutdown; gateway/upstream ports rebind")
            bridge = None
            try:
                with t01.environment({"T12_MIN_OPENS": "1"}):
                    bridge = t12.Bridge(run, live, owned)
                bridge.minimum_opens = 1
                ready = bridge.wait(live / "ready.json", timeout=180)
                for index in range(1, 6):
                    with t01.Fixture(t01.MEDIA, invoke=bridge.invoke) as fixture:
                        launched_utc, launched_epoch = utc(), time.time()
                        session = t01.NativeSession(run, fixture, executable, index)
                        try:
                            began = session.started
                            session.wait(0, 30)
                            first_ready = time.monotonic() - began
                            value = session.command("t22-startup", timeout=90)
                            row = dict(value, pid=session.process.pid, identity=session.identity,
                                utc_started=launched_utc, utc_finished=utc(), first_message_seconds=first_ready,
                                ready_seconds=value["window_ready_epoch_ms"] / 1000 - launched_epoch,
                                health_seconds=value["health_ready_epoch_ms"] / 1000 - launched_epoch, owned_rss_bytes=session.observe(),
                                process_sample=t12.process_sample(session, bridge), gpu=gpu_probe())
                            rows.append(row)
                            append_sample(run.folder / "startup-samples.jsonl", row)
                            if index == 5:
                                start = session.command("t12", action="start", payload=ready, timeout=60)
                                session.record["commands"][-1]["args"] = dict(action="start", payload="actual Host DTO; private URLs redacted")
                                require(start["nonblank"] and not start["framework_overlay"] and not start["errors"], "Formal page failed")
                                run.observed["ui"] = session.command("t12", action="t22-ui", timeout=90)
                                check_ui(run.observed["ui"])
                                run.observed["faults"] = session.command("t12", action="errors", timeout=90)
                                t12.validate_errors(run.observed["faults"])
                                run.observed["stop"] = session.command("t12", action="stop")
                                t12.validate_stop(run.observed["stop"])
                                disposed = session.command("t12", action="dispose")
                                require(disposed["audio_closed"] and disposed["videos"] == 0 and not disposed["errors"], "Formal disposal leaked resources")
                                run.observed["disposed"] = disposed
                            session.close()
                            check_cleanup(session.record)
                        finally:
                            if not session.closed:
                                session.close(graceful=False)
                        require(fixture.active == 0, "Fixture retained HTTP after close")
                bridge.finish()
            finally:
                if bridge is not None:
                    bridge.close()
        result = check_startups(rows, run.observed["native_sessions"])
        bridge_ms = [sample["milliseconds"] for row in rows for sample in row["responses"]]
        metadata_ms = [row["milliseconds"] for row in run.observed["ui"]["metadata_responses"]]
        ui_ms = [row["milliseconds"] for row in run.observed["ui"]["responses"]]
        run.observed["targets"] = dict(window_seconds=[row["ready_seconds"] for row in rows], window_target_seconds=1.5,
            health_seconds=[row["health_seconds"] for row in rows], health_target_seconds=60,
            native_local_bridge_ms=bridge_ms, native_local_bridge_p95_ms=p95(bridge_ms),
            metadata_local_bridge_ms=metadata_ms, metadata_local_bridge_p95_ms=p95(metadata_ms), bridge_target_ms=100,
            ui_ms=ui_ms, ui_p95_ms=p95(ui_ms), ui_target_ms=100, policy="ADR-044: target gaps recorded; reliability mandatory")
        run.observed["target_gaps"] = dict(window_seconds=[max(0, row["ready_seconds"] - 1.5) for row in rows],
            health_seconds=[max(0, row["health_seconds"] - 60) for row in rows],
            native_bridge_ms=max(0, p95(bridge_ms) - 100), metadata_bridge_ms=max(0, p95(metadata_ms) - 100),
            ui_ms=max(0, p95(ui_ms) - 100),
            method="native visible/healthy timestamp minus launch wall clock; window upper bound includes automation transport; local get_details includes owned loopback/file bridge; UI click to actual paused state")
        return result


def check_endurance(rows):
    require(len(rows) >= 121, "600s first/last and 5s raw playback samples required")
    first, last = rows[0], rows[-1]
    seconds = last["host_monotonic"] - first["host_monotonic"]
    require(seconds >= 600 and last["value"]["sample"]["position"] - first["value"]["sample"]["position"] >= 590,
            "Full 10-minute wall/media playback missing")
    session_id = first["value"]["session_id"]
    for row in rows:
        value, sample = row["value"], row["value"]["sample"]
        require(value["input_audit"]["active"] is True
                and not any(event["kind"] == "seeking" or event["kind"] == "ratechange" and event["rate"] != 1
                            for event in value["input_audit"]["events"]),
                "Unexpected seek/rate change during continuous playback; inspect raw input audit")
        require(value["session_id"] == session_id and session_id and value["actual_gateway"] is True
                and value["playback_rate"] == 1 and sample["session_bound"] is True and sample["video_count"] == 1
                and sample["width"] == 1920 and sample["height"] == 1080 and not sample["paused"] and sample["media_error"] is None
                and sample["audio_state"] == "running" and sample["audio_rms"] > .001 and abs(sample["audio_peak_hz"] - 440) < 40,
                "Gateway identity, current 1080p video or decoded audio failed")
        require(numeric(value["ui_position"]) and abs(value["ui_position"] - sample["position"]) <= 1
                and value["visibility"] == "visible" and value["native"]["visible"] and not value["native"]["minimized"]
                and value["errors"] == [], "Actual progress UI/window/console failed")
        require(row["gpu"]["counters"] and row["process"]["owned_rss_bytes"] > 0, "Raw GPU/RSS sample missing")
        check_process(row["process"], value["native"])
    for a, b in zip(rows, rows[1:]):
        require(0 < b["host_monotonic"] - a["host_monotonic"] < 15
                and b["value"]["sample"]["position"] > a["value"]["sample"]["position"]
                and b["value"]["sample"]["frames"] > a["value"]["sample"]["frames"], "Playback sample gap or stalled video/media clock")
    return dict(seconds=seconds, media_seconds=last["value"]["sample"]["position"] - first["value"]["sample"]["position"], samples=len(rows),
        presented_frames=last["value"]["sample"]["frames"] - first["value"]["sample"]["frames"],
        dropped_frames=last["value"]["dropped_frames"] - first["value"]["dropped_frames"])


def endurance(run, session, bridge):
    readiness = session.command("t12", action="t22-ready", timeout=40)
    run.observed["endurance_readiness"] = readiness
    write_json(run.folder / "endurance-readiness.json", readiness)
    rows = run.observed.setdefault("endurance_samples", [])
    start = time.monotonic()
    while True:
        value = session.command("t12", action="t22-observe", timeout=30)
        row = dict(utc=utc(), host_monotonic=time.monotonic(), value=value, process=t12.process_sample(session, bridge), gpu=gpu_probe())
        rows.append(row)
        append_sample(run.folder / "endurance-samples.jsonl", row)
        if row["host_monotonic"] - rows[0]["host_monotonic"] >= 600:
            break
        due = start + len(rows) * 5
        time.sleep(max(0, due - time.monotonic()))
    run.observed["endurance"] = check_endurance(rows)


def a1_evidence(run):
    report_path = ROOT / "evidence/acceptance/T22-startup-ui.json"
    report = read_json(report_path)
    require(report["card"] == "T0.26" and report["suite"] == "startup-ui" and report["counts"]["failed"] == 0
            and report["counts"]["passed"] > 0 and all(row["result"] == "PASS" for row in report["summaries"])
            and {row["id"] for row in report["summaries"]} == set(IDS), "Passing A1 requirement inventory required")
    require(report["tested_source_digest"] == report["source_before"] == run.source
            and report["protected"] == workflow.protected_hash(), "A1 source/baseline differs")
    for key in ("model", "cpu", "macos", "arch", "gpu", "dependencies", "webkit", "locks"):
        require(report["observed"]["reference_machine"][key] == run.observed["reference_machine"][key], "A1 reference environment changed: " + key)
    receipt_path = ROOT / report["observed"]["resources"]["receipt"]
    receipt = read_json(receipt_path)
    current = read_json(ROOT / run.observed["resources"]["receipt"])
    require(receipt["card"] == "T0.26" and receipt["id"] == "A1" and receipt["status"] == "finished" and receipt["exit"] == 0
            and receipt["source"] == receipt["after_source"] == run.source and receipt["protected"] == current["protected"]
            and receipt["generation"] == current["generation"], "A1 original successful same-source/generation receipt required")
    log = ROOT / receipt["log"]
    require(file_hash(log) == receipt["log_hash"] and "T22_EVIDENCE_SHA256:" + file_hash(report_path) in log.read_text(),
            "A1 report is not bound to original receipt log")
    require(file_hash(ROOT / receipt["report_artifact"]) == receipt["report_hash"], "A1 JUnit archived bytes changed")
    for artifact in report["artifacts"]:
        path = ROOT / artifact["path"]
        require(path.is_file() and path.stat().st_size == artifact["bytes"] and file_hash(path) == artifact["sha256"],
                "A1 raw artifact missing/changed: " + artifact["path"])
    raw_path = ROOT / report["observed"]["packaged_folder"]
    raw = [json.loads(line) for line in (raw_path.parent / "startup-samples.jsonl").read_text().splitlines()]
    require(raw == report["observed"]["startups"], "A1 raw launch JSONL differs from reported launches")
    check_startups(raw, report["observed"]["native_sessions"])
    check_ui(report["observed"]["ui"])
    run.observed["a1_binding"] = dict(report=str(report_path.relative_to(ROOT)), sha256=file_hash(report_path), receipt=str(receipt_path.relative_to(ROOT)))
    return report


def player(run):
    previous_media = t12.owned_media
    t12.owned_media = media
    try:
        result = t12.webview(run, build=False, after_start=lambda session, bridge: endurance(run, session, bridge))
        t21.check_resources(run.observed)
        return result
    finally:
        t12.owned_media = previous_media


def check_stream(vector, samples):
    require(vector["seconds"] >= 180 and vector["bytes"] >= 450_000_000 and vector["target_bps"] == 20_000_000
            and vector["payload_checked"] and vector["active_requests"] == vector["live_sessions"] == 0
            and vector["ports_released"] is True, "Full 20Mbps/3min transfer and cancellation/port cleanup required")
    require(vector["bytes"] >= 2_500_000 * (vector["seconds"] - 1), "Measured throughput missed the 20Mbps load by more than one sampling interval")
    trace = vector["samples"]
    require(len(trace) >= 180 and trace[-1]["seconds"] - trace[0]["seconds"] >= 178
            and trace[-1]["bytes"] == vector["bytes"] and trace[-1]["seconds"] <= vector["seconds"]
            and all(0 < b["seconds"] - a["seconds"] < 3 and b["bytes"] > a["bytes"] for a, b in zip(trace, trace[1:])),
            "Continuous raw throughput samples missing/stalled")
    require(len(samples) >= 175 and samples[-1]["monotonic"] - samples[0]["monotonic"] >= 175
            and all(0 < b["monotonic"] - a["monotonic"] < 3 for a, b in zip(samples, samples[1:]))
            and all(row["rss_bytes"] > 0 and numeric(row["cpu_seconds"]) and row["cpu_seconds"] >= 0
                    and row["identity"] == samples[0]["identity"] and row["gpu"]["counters"] for row in samples),
            "Raw stream CPU/RSS/GPU/identity samples incomplete")
    rss = [row["rss_bytes"] for row in samples]
    require(max(rss) <= 4096 * 1024 ** 2 and statistics.median(rss[-5:]) - statistics.median(rss[:5]) <= 64 * 1024 ** 2,
            "Streaming buffer RSS exceeded budget or grew over 64MiB")
    return dict(seconds=vector["seconds"], bytes=vector["bytes"], mbps=vector["bytes"] * 8 / vector["seconds"] / 1_000_000,
                resource_samples=len(samples), peak_rss_bytes=max(rss))


def throughput(run):
    path = run.folder / "stream-progress.json"
    samples, errors, stop = [], [], threading.Event()
    run.observed["stream_resources"] = samples
    def sample():
        identity = None
        next_sample = time.monotonic() + 1
        while not stop.wait(max(0, next_sample - time.monotonic())):
            next_sample += 1
            try:
                if not path.exists():
                    continue
                value = read_json(path)
                proc = workflow.runtime.psutil.Process(value["pid"])
                token = workflow.runtime.identity(proc.pid)
                require(os.getpid() in {parent.pid for parent in proc.parents()}, "Stream process outside acceptance tree")
                if identity is None:
                    identity = token
                require(token == identity and token, "Stream process identity changed")
                times = proc.cpu_times()
                row = dict(utc=utc(), monotonic=time.monotonic(), pid=proc.pid, identity=token,
                    rss_bytes=proc.memory_info().rss, cpu_seconds=times.user + times.system, gpu=gpu_probe())
                samples.append(row)
                append_sample(run.folder / "stream-resources.jsonl", row)
            except workflow.runtime.psutil.NoSuchProcess:
                break
            except Exception as cause:
                errors.append(str(cause))
                break
    monitor = threading.Thread(target=sample)
    monitor.start()
    try:
        with t01.environment({"T22_STREAM_SAMPLE": str(path)}):
            output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib",
                STREAM, "--", "--exact", "--ignored", "--test-threads=1", "--nocapture"], timeout=300)
    finally:
        stop.set(); monitor.join(timeout=20)
    require(not monitor.is_alive() and not errors, "Stream CPU/RSS/GPU sampler failed: " + str(errors))
    vectors = [json.loads(line.split("T22_STREAM:", 1)[1]) for line in output.splitlines() if "T22_STREAM:" in line]
    require(len(vectors) == 1 and "test result: ok. 1 passed; 0 failed; 0 ignored;" in output, "Explicit ignored endurance test did not run")
    run.observed["stream"] = vectors[0]
    return check_stream(vectors[0], samples)


def oracles(run):
    output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t22.py"], timeout=120)
    count = re.search(r"Ran (\d+) tests?", output)
    require(count and int(count[1]) >= 10 and re.search(r"^OK$", output, re.M), "Required T22 independent evidence negatives missing")
    return dict(tests=int(count[1]))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["startup-ui", "macos-arm64"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(__debug__, "Acceptance requires assertions")
    require(args.report.resolve() == ROOT / f"evidence/acceptance/T22-{args.suite}.json"
            and args.junit.resolve() == ROOT / f"evidence/acceptance/T22-{args.suite}.xml", "Exact T22 output paths required")
    t02.NATIVE |= set(FAULTS) | {STREAM.split("::")[-1], t12.LIVE_TEST.split("::")[-1]}
    run = Acceptance(args.suite, args.report, args.junit)
    ready = run.case("resource-ownership", ["C-RESOURCE", "REQ-T22"], "actual original run.py/4096MiB/jobs2/thread1 receipt", lambda: t10.resources(run, card="T0.26"))
    if ready:
        run.case("independent-oracle-negatives", IDS, "short/empty/stalled/foreign/missing-resource evidence rejected", lambda: oracles(run))
        run.case("reference-machine", ["R09", "R47", "C-RESOURCE", "REQ-T22"], "actual CPU/GPU/OS/WebKit/dependencies and raw AGX probe", lambda: environment(run))
        if args.suite == "startup-ui":
            run.case("network-source-file-fault-recovery", ["R09", "R48", "REQ-T21", "REQ-T22"], "real timeout/disconnect/source-revoke and unusable SQLite path/retry", lambda: t21.selected_native(run, ["--lib"], FAULTS))
            run.case("five-cold-starts-ui-faults-and-long-test-preparation", IDS,
                "five packaged fresh Rust Hosts, local IPC samples, minimum window/zoom/fullscreen, native media fault/retry/cleanup and actual long-media generation", lambda: startup_ui(run))
        else:
            a1 = {}
            def bind():
                a1.update(a1_evidence(run))
                return run.observed["a1_binding"]
            bound = run.case("same-source-a1-five-startup-receipts", ["R09", "R47", "C-RESOURCE", "REQ-T22"], "recompute five launches and all hash-bound A1 raw artifacts; no new cold start", bind)
            if bound:
                with packaged_paths(ROOT / a1["observed"]["packaged_folder"]):
                    built = run.case("current-packaged-build", IDS, "unchanged production/automation bundles and locks from A1", lambda: t01.check_build(run.source))
                    if built:
                        run.case("gateway-ten-minute-playback-and-fifty-switches", IDS, "600s actual official player 1080p30 H264AAC gateway/PCM/frames/progress, 50 clean switches and RSS windows", lambda: player(run))
            run.case("twenty-mbps-three-minute-stream", ["R09", "R48", "C-RESOURCE", "REQ-T21", "REQ-T22"], "180s 20Mbps actual gateway byte-checked paced stream, raw resource samples, cancellation and socket cleanup", lambda: throughput(run))
    return run.save()


if __name__ == "__main__":
    if sys.argv[1:] == ["--probe-environment"]:
        print(json.dumps(environment_probe()))
    else:
        raise SystemExit(main())
