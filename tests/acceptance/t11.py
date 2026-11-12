"""T11 native subtitles and actual Host -> packaged WKWebView integration.

Only the original run.py owns execution. CS callbacks are explicitly synthetic;
the absence of an observed DASH baseline is conditional NOT RUN, rechecked N02.
"""
import argparse
import copy
import datetime as dt
import hashlib
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import tempfile
import threading
import time
import xml.etree.ElementTree as ET

from t00 import Acceptance as BaseAcceptance, file_hash, read_json, require, workflow, write_json
from t09 import redact as redact_credentials
from t10 import resources
import t01

ROOT = Path(__file__).resolve().parents[2]
IDS = ("R26", "R29", "REQ-T11")
RUST_TESTS = {
    "gateway::tests::t11::t11_structured_text_timeline_and_independent_vectors",
    "gateway::tests::t11::t11_ass_comments_and_empty_lines_preserve_complete_cues",
    "gateway::tests::t11::t11_malformed_encoding_format_and_bounded_expansion",
    "gateway::tests::t11::t11_http_auth_queries_transformed_head_and_conditional",
    "gateway::tests::t11::t11_release_expiry_switch_and_cap_revocation",
    "gateway::tests::t11::t11_cross_origin_and_unsafe_subtitle_targets",
    "gateway::tests::t11::t11_error_diagnostics_limits_recovery_and_drm_types",
    "gateway::tests::t11::t11_pending_subtitle_close_cancels_before_conversion",
    "host::maccms::tests::t08::t09::t11::t11_host_open_release_switch_and_live_webview",
}
LIVE_TEST = next(name for name in RUST_TESTS if name.startswith("host::"))
EXPECTED = {
    "vtt": [(1.25, 3.75, "你好 & world\n第二行")],
    "srt": [(1.25, 3.75, "你好 & world\n2 < 3 <literal>"), (3723.004, 3724.005, 'later 中 "quote"')],
    "ass": [(1.25, 3.75, "你好 & world\n第二行,comma soft\u00a0space  2 < 3"), (3723.0, 3724.01, "latertext\nnew")],
}
ASS_BOUNDARY_INPUT = (
    "[Script Info]\nWrapStyle: 0\n[Events]\nFormat: Start,End,Text\n"
    "Dialogue: 0:00:04.25,0:00:05.75,前{literal}中文，2 < 3！{注释\\b1}后{\\b0}\\N第二行\n"
    "Dialogue: 0:00:06.25,0:00:07.75,\\N\\N前\\N\\N\\N后\\N\\N\n"
    "Dialogue: 0:00:08.25,0:00:09.75,{\\q2}\\n前\\n\\n后\\h尾\\n\n"
    "Dialogue: 0:00:10.25,0:00:11.75,\\N前{\\p1}m 0 0 l 1 1{comment}{\\p0}\\N\\N后\\N\n"
)
ASS_BOUNDARIES = [
    (4.25, 5.75, "前中文，2 < 3！后\n第二行"),
    (6.25, 7.75, "\n\n前\n\n\n后\n\n"),
    (8.25, 9.75, "\n前\n\n后\u00a0尾\n"),
    (10.25, 11.75, "\n前\n\n后\n"),
]


def redact(output):
    names = "(?:" + "|".join(map(re.escape, sorted(RUST_TESTS))) + ")"
    runner = re.compile(r"^(?:test " + names + r" \.\.\.(?=\s|$)|    " + names
                        + r"(?=\r?$)|thread '" + names + r"' \(\d+\) panicked at )")
    lines = []
    for line in output.splitlines(keepends=True):
        match = runner.match(line)
        end = match.end() if match else 0
        lines.append(line[:end] + redact_credentials(line[end:]))
    return "".join(lines)


class PlainVtt(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts = []

    def handle_data(self, data):
        self.parts.append(data)


def parse_vtt(value):
    require(value.startswith("WEBVTT\n\n"), "converted VTT signature differs")
    cues = []
    for block in value[8:].split("\n\n"):
        if not block.strip():
            continue
        lines = block.splitlines()
        if lines[0] in {"STYLE", "REGION", "NOTE"} or lines[0].startswith(("NOTE ", "NOTE\t")):
            continue
        index = next((i for i, line in enumerate(lines) if " --> " in line), None)
        require(index in (0, 1), "missing structured cue timing")
        match = re.fullmatch(r"((?:\d+:)?\d{2}:\d{2}\.\d{3}) --> ((?:\d+:)?\d{2}:\d{2}\.\d{3})(?: .*)?", lines[index])
        require(match is not None, "malformed converted cue timing")
        def seconds(clock):
            parts = [float(p) for p in clock.split(":")]
            return round(sum(p * 60 ** i for i, p in enumerate(reversed(parts))), 3)
        start, end = map(seconds, match.groups())
        require(end > start, "converted cue end does not follow start")
        parser = PlainVtt()
        parser.feed("\n".join(lines[index+1:]))
        parser.close()
        cues.append((start, end, "".join(parser.parts)))
    return cues


def check_vector(v):
    kind = v["kind"]
    if kind in ("conversion", "http"):
        require(v["format"] in EXPECTED, "unknown conversion format")
        require(parse_vtt(v["output"]) == EXPECTED[v["format"]], "independent timeline/text/newline oracle differs")
        if kind == "conversion":
            raw = v["input"].replace("\r\n", "\n").lstrip("\ufeff")
            if v["format"] == "vtt":
                require(raw.startswith("WEBVTT\n") and "00:01.250 --> 00:03.750" in raw, "VTT source differs")
            elif v["format"] == "srt":
                require(re.findall(r"(?m)^(\d+:\d+:\d+,\d+) --> (\d+:\d+:\d+,\d+)$", raw)
                        == [("00:00:01,250", "00:00:03,750"), ("01:02:03,004", "01:02:04,005")], "SRT original timing differs")
            else:
                require("Format: Layer, End, Start," in raw and "{\\p1}m 0 0 l 1 1{\\p0}" in raw
                        and "{literal}" in raw and "第二行,comma\\nsoft\\hspace" in raw,
                        "ASS original drawing/format/comma/literal/wrap inputs missing")
        else:
            data = v["output"].encode()
            require(int(v["head_length"]) == len(data) and v["etag"] == '"' + hashlib.sha256(data).hexdigest() + '"'
                    and v["same_cap"] is True and v["conditional_status"] == 304, "transformed entity/cap differs")
    elif kind == "ass-boundaries":
        require(v["input"] == ASS_BOUNDARY_INPUT, "ASS original boundary input differs")
        require(parse_vtt(v["output"]) == ASS_BOUNDARIES, "ASS complete boundary text/timeline differs")
    elif kind == "invalid":
        require(v["malformed"] >= 17 and v["input_limit"] is True and v["output_limit"] is True, "negative/limit coverage missing")
    elif kind == "lifetime":
        require(v == dict(kind="lifetime", at_ms=300001, resolution_expired=True, authenticated_after_expiry=True, continued_status=200,
                          old_status=410, wrong_cap=401, revoked_status=410, active_requests=0), "lifetime observation differs")
    elif kind == "isolation":
        require(v == dict(kind="isolation", cross_origin_stripped=True, direct_origin_stripped=True,
                          private_target_contacted=False), "subtitle credentials/SSRF isolation differs")
    elif kind == "drm":
        require(v["type"] in ("fairplay", "widevine") and v["error"]["code"] == "MEDIA_UNSUPPORTED"
                and v["error"]["details"]["drm_type"] == v["type"]
                and v["error"]["details"]["media_type"] == "hls", "DRM type error differs")
    elif kind == "cancel":
        require(v == dict(kind="cancel", late_bytes=0, active_requests=0), "late subtitle body or pending request remains")
    elif kind == "host":
        require(v["tracks"] == 3 and v["released_resolution"] is True and v["old_status"] == 410
                and v["new_status"] == 200 and v["closed_status"] == 410 and v["synthetic_callbacks"] is True,
                "actual Host session/track binding differs")
    else:
        raise ValueError("unexpected native observation")


def native_vectors(run, output):
    vectors = [json.loads(line.split("T11_VECTOR:", 1)[1]) for line in output.splitlines() if "T11_VECTOR:" in line]
    require(len(vectors) == 14, f"13 original plus one ASS boundary observation required, got {len(vectors)}")
    require({v["format"] for v in vectors if v["kind"] == "conversion"} == set(EXPECTED), "conversion formats incomplete")
    require({v["format"] for v in vectors if v["kind"] == "http"} == set(EXPECTED), "real HTTP formats incomplete")
    require({v["type"] for v in vectors if v["kind"] == "drm"} == {"fairplay", "widevine"}, "DRM fixtures incomplete")
    require(sum(v["kind"] == "ass-boundaries" for v in vectors) == 1, "ASS boundary observation missing")
    rejected = 0
    for vector in vectors:
        check_vector(vector)
        keys = dict(conversion="output", http="output", invalid="input_limit", lifetime="continued_status",
                    isolation="private_target_contacted", drm="type", cancel="late_bytes", host="tracks",
                    **{"ass-boundaries":"output"})
        broken = copy.deepcopy(vector)
        key = keys[vector["kind"]]
        broken[key] = "corrupt" if isinstance(broken[key], str) else not broken[key] if isinstance(broken[key], bool) else -1
        try:
            check_vector(broken)
        except (ValueError, KeyError, TypeError):
            rejected += 1
        else:
            raise ValueError("oracle accepted corrupt " + vector["kind"])
    path = run.folder / "native-observations.json"
    write_json(path, vectors)
    run.observed["native"] = dict(observations=len(vectors), corruptions_rejected=rejected, trace=str(path.relative_to(ROOT)))
    return run.observed["native"]


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        import uuid
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T11-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}
        self.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=0, passed=0, failed=0, skipped=0, status="NOT RUN")

    def redact_output(self, text):
        safe = redact(text)
        if safe != text:
            self.observed["native_log_sanitized"] = True
        return safe

    def command(self, argv, timeout=3000):
        argv = workflow.runtime.resolve_argv([str(a) for a in argv])
        path = self.folder / f"command-{len(self.commands)+1}.log"
        record = dict(argv=argv, exit=None, log=str(path.relative_to(ROOT)))
        self.commands.append(record)
        started = time.monotonic()
        process = None
        token = None
        with path.open("w", encoding="utf-8") as log:
            log.write(json.dumps(argv) + "\n")
            log.flush()
            try:
                process = subprocess.Popen(argv, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
                token = workflow.runtime.identity(process.pid)
                output, _ = process.communicate(timeout=timeout)
                record["exit"] = process.returncode
                safe = redact(output)
                log.write(safe)
                require(safe == output, "command output contained credentials; stored log was sanitized")
            finally:
                if process is not None and process.poll() is None:
                    require(token, "command process identity unavailable; original runner must clean its payload")
                    workflow.runtime.terminate(workflow.runtime.family(process.pid, token))
                    output, _ = process.communicate(timeout=10)
                    log.write(redact(output))
                record["seconds"] = time.monotonic()-started
        require(record["exit"] == 0, f"command failed ({record['exit']}); log={record['log']}")
        return safe

    def save(self):
        self.case("source-stability", ["REQ-T11"], "stable tested source", lambda: require(workflow.snapshot() == self.source, "source changed during A1"))
        summaries = []
        for identifier in IDS:
            children = [c for c in self.cases if identifier in c["requirements"]]
            summaries.append(dict(id=identifier, result="PASS" if children and all(c["result"] == "PASS" or c["result"] == "SKIP" and not c["required"] and c["id"] == "R29/dash-baseline" and self.observed.get("dash", {}).get("status") == "NOT RUN" for c in children) else "FAIL",
                                  children=[c["id"] for c in children]))
        counts = dict(required=sum(c["required"] for c in self.cases), ran=sum(c["result"] != "SKIP" for c in self.cases),
                      passed=sum(c["result"] == "PASS" for c in self.cases), failed=sum(c["result"] == "FAIL" for c in self.cases),
                      skipped=sum(c["result"] == "SKIP" for c in self.cases))
        report = dict(card="T0.11", original_card="T11", suite="all", utc_started=self.started,
            utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source, tested_source_digest=workflow.snapshot(),
            protected=workflow.protected_hash(), platform=platform.system(), os=platform.platform(), arch=platform.machine(),
            commands=self.commands, cases=self.cases, summaries=summaries, counts=counts, observed=self.observed,
            limitations=dict(ass="UTF-8 timed basic text/newlines; typography, positioning, karaoke effects and vector drawings omitted",
                cloudstream="Synthetic native callbacks; actual original providers/interceptors remain N02",
                endurance="short playback/switch/seek here; long product endurance per ADR-020"))
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T11-all", tests=str(len(rows)), failures=str(sum(r["result"] == "FAIL" for r in rows)), errors="0", skipped=str(counts["skipped"]))
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T11", name=row["id"])
            if row["result"] == "SKIP":
                ET.SubElement(node, "skipped", message=str(row["actual"]))
            elif row["result"] == "FAIL":
                ET.SubElement(node, "failure", message="T11 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return int(counts["failed"] > 0 or any(s["result"] == "FAIL" for s in summaries))


def dash_baseline(run):
    inventory_path = ROOT / "replica-spec/references/plugin-artifact-inventory.json"
    inventory = read_json(inventory_path)
    require(len(inventory["plugins"]) == 7, "seven original plugin acquisition artifacts required")
    artifacts = []
    for plugin in inventory["plugins"]:
        path = ROOT / "evidence/t00/inputs" / (plugin["internal_name"] + ".cs3")
        require(file_hash(path) == plugin["sha256"], "original plugin artifact changed")
        artifacts.append(dict(plugin=plugin["internal_name"], path=str(path.relative_to(ROOT)), sha256=file_hash(path)))
    inspected, found = [], []
    roots = [ROOT / "evidence/acceptance", ROOT / "evidence/t00", ROOT / "evidence/t01"]
    roots += [p for p in (ROOT / "evidence").iterdir() if p.is_dir() and ("baseline" in p.name.lower() or p.name.lower() in {"t02", "t03", "t04", "n02"})]
    def has_dash(value):
        if isinstance(value, dict):
            if value.get("kind") == "dash" or value.get("stream_kind") == "dash":
                return True
            return any(has_dash(v) for v in value.values())
        if isinstance(value, list):
            return any(has_dash(v) for v in value)
        return isinstance(value, str) and re.search(r"https?://[^\s\"<>]+\.mpd(?:[?#][^\s\"<>]*)?(?:$|\s)", value, re.I) is not None
    for root in roots:
        for path in sorted(root.rglob("*")):
            if not path.is_file() or path.is_relative_to(run.folder) or path.name.startswith("T11-") or "T11-" in str(path.parent):
                continue
            if path.suffix.lower() == ".mpd":
                found.append(str(path.relative_to(ROOT)))
                inspected.append(dict(path=str(path.relative_to(ROOT)), sha256=file_hash(path)))
            elif path.suffix.lower() in {".json", ".jsonl"}:
                values = [json.loads(line) for line in path.read_text().splitlines() if line.strip()] if path.suffix == ".jsonl" else [read_json(path)]
                if any(has_dash(v) for v in values):
                    found.append(str(path.relative_to(ROOT)))
                inspected.append(dict(path=str(path.relative_to(ROOT)), sha256=file_hash(path)))
    run.observed["dash"] = dict(status="REQUIRES TESTING" if found else "NOT RUN", artifacts=artifacts,
        inspected=inspected, found=found, reason="DASH observations require the complete conditional gate" if found else "No actual DASH media output found in existing evidence; original CS runtime/provider baseline is pending N02. Plugin package acquisition does not observe media output.",
        deferred_to="T0.22/N02 A1: BaseURL/List/Base/SIDX, constrained Number/Time/RepresentationID/Bandwidth, seek/query/illegal variables and real playback")
    require(not found, "DASH artifacts were observed: classify clear/DRM output and complete the applicable DASH/DRM gate before T11 can pass")
    run.cases.append(dict(id="R29/dash-baseline", requirements=["R29", "REQ-T11"], required=False,
        input="existing original artifact/baseline evidence", expected="conditional clear-DASH coverage when observed",
        actual="NOT RUN: no DASH media output observed; original provider output must be rechecked in N02", result="SKIP", evidence=[]))
    return dict(inspected=len(inspected), plugin_artifacts=len(artifacts), dash="NOT RUN", followup="N02")


def rust(run):
    try:
        output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "t11_", "--", "--test-threads=1", "--nocapture"])
    except Exception:
        output = (ROOT / run.commands[-1]["log"]).read_text(errors="replace")
        summary = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", output)
        passed, failed, skipped = map(int, summary.groups()) if summary else (0, 0, 0)
        run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=passed+failed+skipped, passed=passed,
            failed=failed, skipped=skipped, status="FAIL" if summary else "NOT RUN", log=run.commands[-1]["log"])
        raise
    names = set(re.findall(r"^test (\S+) \.\.\.", output, re.M))
    require(names == RUST_TESTS, "native test inventory differs")
    require(f"test result: ok. {len(RUST_TESTS)} passed; 0 failed; 0 ignored;" in output, "all named native tests must pass")
    run.observed["native_counts"] = dict(required=len(names), ran=len(names), passed=len(names), failed=0, skipped=0, status="PASS")
    return native_vectors(run, output)


def wait_file(path, process, timeout=40):
    deadline = time.monotonic()+timeout
    while not path.exists():
        require(process.poll() is None, "native Host bridge exited before the WebView phase")
        require(time.monotonic() < deadline, "native Host bridge did not become ready")
        time.sleep(.05)


def validate_webview(value, episode):
    require(value["episode"] == episode and value["disabled"] is True and value["cue_ended"] is True
            and value["actual_gateway"] is True and value["new_frames"] > 0
            and value["old_tracks_removed"] == (0 if episode == "first" else 3), "WebView session switch/off/seek differs")
    expected = [dict(format=f, start=1.25, end=3.75, text=f"{episode} {f.upper()}\n第二行",
                     mode="showing", cues=5 if f == "ass" else 1) for f in ("vtt", "srt", "ass")]
    require(value["observations"] == expected, "actual WebView track cues/timeline differ")
    boundaries = value["ass_boundaries"]
    require(len(boundaries) == len(ASS_BOUNDARIES), "actual WebView ASS boundary cues missing")
    for row, (start, end, text) in zip(boundaries, ASS_BOUNDARIES):
        require(row["start"] == start and row["end"] == end and row["text"] == text
                and row["cue_ended"] is True, "actual WebView complete ASS boundary text/timeline differs")
        # The wire string carries markup; parse it independently and compare it
        # with the native getCueAsHTML().textContent observation.
        require(parse_vtt(f"WEBVTT\n\n00:00:{start:06.3f} --> 00:00:{end:06.3f}\n{row['wire_text']}\n\n") == [(start, end, text)],
                "actual WebView ASS wire/plain text evidence differs")
    sample = value["sample"]
    require(sample["media_error"] is None and sample["width"] == 1920 and sample["height"] == 1080 and sample["frames"] > 0
            and sample["diagnostics"]["tracks"] == 3, "actual native playback/track evidence missing")


def webview(run):
    run.observed["gui_session"] = t01.native_gui_session()
    require(run.observed["gui_session"]["screen_locked"] is not True,
            "macOS GUI session is locked; unlock the desktop before native acceptance")
    t01.check_media()
    t01.build(run)
    # The existing build function validates production/automation separation and
    # binds both bundles and dependency locks to the current source digest.
    receipt = t01.check_build(run.source)
    executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/moc-video"
    argv = workflow.runtime.resolve_argv(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", LIVE_TEST, "--", "--exact", "--test-threads=1", "--nocapture"])
    log_path = run.folder / "live-host.log"
    record = dict(argv=argv, exit=None, log=str(log_path.relative_to(ROOT)))
    run.commands.append(record)
    with tempfile.TemporaryDirectory(prefix="t11-bridge-", dir=run.folder) as folder, log_path.open("w", encoding="utf-8") as log:
        directory = Path(folder)
        process = subprocess.Popen(argv, cwd=ROOT, env=dict(os.environ, T11_BRIDGE_DIR=folder, T11_MEDIA=str(t01.MEDIA / "clip.mp4")), stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        token = workflow.runtime.identity(process.pid)
        known = {}
        output = []
        def collect():
            for line in process.stdout:
                safe = redact(line)
                output.append(safe)
                log.write(safe)
                log.flush()
        reader = threading.Thread(target=collect, name="t11-host-output", daemon=True)
        reader.start()
        session = None
        try:
            wait_file(directory / "ready.json", process)
            known.update({r["pid"]: r for r in workflow.runtime.family(process.pid, token)})
            views = read_json(directory / "ready.json")
            with t01.Fixture(t01.MEDIA) as fixture:
                # Keep transport evidence on failures as well as successful cues.
                run.observed["webview_transport"] = fixture.records
                session = t01.NativeSession(run, fixture, executable, 1)
                session.wait(0, 30)
                identity = session.command("identity")
                require(identity["origin"] == "tauri://localhost", "wrong packaged WebView origin")
                run.observed["webview_identity"] = identity
                for episode in ("first", "second"):
                    if episode == "second":
                        (directory / "close-first").touch()
                        wait_file(directory / "closed", process)
                    value = session.command("t11-subtitles", timeout=30, view=views[episode], episode=episode)
                    # NativeSession's general command ledger includes args; keep
                    # credentials in the transient bridge, outside report/logs.
                    session.record["commands"][-1]["args"] = dict(episode=episode, view="actual Host DTO; credentials redacted")
                    validate_webview(value, episode)
                    run.observed.setdefault("webview_cues", []).append(value)
                stop = session.command("stop")
                require(stop["source_removed"] is True and stop["sample"]["diagnostics"]["tracks"] == 0, "stop did not release subtitle resources")
                session.close()
                require(not run.observed.get("native_log_sanitized"), "native log contained a capability; persisted log was sanitized")
                require(fixture.active == 0, "fixture requests remain after native close")
                run.observed["webview_fixture"] = fixture.summary()
            (directory / "complete").touch()
            process.wait(timeout=30)
            reader.join(timeout=5)
            record["exit"] = process.returncode
            require(process.returncode == 0 and not reader.is_alive(), "live Host bridge failed or retained its output pipe")
            vectors = [json.loads(line.split("T11_VECTOR:", 1)[1]) for line in output if "T11_VECTOR:" in line]
            require(len(vectors) == 1 and vectors[0]["live_webview"] is True, "live Host bridge evidence missing")
            check_vector(vectors[0])
            run.observed["live_host"] = vectors[0]
            return dict(episodes=2, decoded_tracks=6,
                        ass_boundary_cues=sum(len(value["ass_boundaries"]) for value in run.observed["webview_cues"]),
                        bridge=record["log"], old_and_current_closed=True,
                        deployment="owned native processes and loopback fixtures; no cluster", browser="project packaged WKWebView harness; Browser plugin not available")
        finally:
            try:
                if session is not None and not session.closed:
                    session.close(graceful=False)
            finally:
                known.update({r["pid"]: r for r in workflow.runtime.family(process.pid, token)})
                if process.poll() is None:
                    workflow.runtime.terminate(list(known.values()))
                process.wait(timeout=10)
                reader.join(timeout=5)
                process.stdout.close()
                record["exit"] = process.returncode
                require(not reader.is_alive() and not any(workflow.runtime.alive(r) for r in known.values()), "owned Host bridge processes remain")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    options = parser.parse_args()
    require(__debug__, "acceptance requires assertions")
    require(options.report.resolve() == ROOT / "evidence/acceptance/T11-all.json" and options.junit.resolve() == ROOT / "evidence/acceptance/T11-all.xml", "T11 A1 output paths differ")
    run = Acceptance(options.report, options.junit)
    ready = run.case("resource-ownership", ["REQ-T11"], "original scope, local macOS arm64 and approved RSS sampled budget", lambda: resources(run, card="T0.11"))
    run.case("dash-evidence-inventory", ["R29", "REQ-T11"], "all existing observed DASH must enable its full gate", lambda: dash_baseline(run))
    native = run.case("native-subtitles", IDS, "eight original plus one ASS boundary native test and independent positive/negative oracles",
                      lambda: rust(run) if ready else require(False, "NOT RUN: original resource gate failed"))
    run.case("acceptance-oracle-regressions", ["REQ-T11"], "nonzero report/oracle/credential/DASH condition regressions",
             lambda: run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t11.py"]) if ready else require(False, "NOT RUN: resource gate failed"))
    run.case("frontend-subtitle-regressions", IDS, "player subtitle switching, off, legacy and late-event tests",
             lambda: run.command(["npm", "test"]) if ready else require(False, "NOT RUN: resource gate failed"))
    run.case("packaged-host-to-webview", IDS, "actual Host view -> authenticated gateway -> six native text tracks across two episodes",
             lambda: webview(run) if ready and native else require(False, "NOT RUN: native prerequisite failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
