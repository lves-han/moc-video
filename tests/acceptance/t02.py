"""T02: projected public JSON, public DNS safety, and actual public media playback.

Run only through the original resource executor. A failed live chain fails A1;
offline fixture success never promotes a public source.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import threading
from urllib.parse import urlsplit

from t00 import read_json, require, workflow
from t10 import resources
import t01
import t12
import t14

ROOT = Path(__file__).resolve().parents[2]
IDS = ["U02", "U05", "R03", "R12", "R16", "R45", "R49", "REQ-T02"]
LIVE_TEST = "host::maccms::tests::t02::t02_live_public_gateway_webview"
NATIVE = {
    "t02_capture_projection_removes_signed_urls_and_preserves_group_order",
    "t02_direct_route_selection_preserves_later_hls_group",
    "t02_source_support_is_judged_across_sampled_videos",
    "t02_minimal_shape_accepts_a_list_without_code_only",
    "t02_one_failing_detail_marks_the_program_incompatible",
    "t02_one_failing_card_marks_the_program_incompatible",
    "t02_media_host_comes_from_the_first_separator",
    "t02_html_response_is_a_source_state",
    "t02_unusable_poster_is_treated_as_no_poster",
    "t02_refused_api_stops_requests_to_that_source",
    "t02_challenge_page_after_details_stops_the_source",
    "t02_refused_media_host_is_requested_once_and_other_hosts_continue",
    "t02_dns_two_servers_failover_and_both_families",
    "t02_dns_one_silent_server_keeps_other_available",
    "t02_dns_both_failed_never_falls_back_to_hosts_or_system",
    "t02_dns_two_silent_servers_deadline_and_cancellation",
    "t02_dns_fake_ip_mixed_results_are_preserved",
    "t02_public_json_list_search_pagination_details_and_complete_groups",
    "t02_public_json_errors_zero_results_and_bounds_remain_explicit",
    "t02_json_media_removed_after_details_returns_no_links",
}
REGRESSIONS = [
    "t08_maccms_http_missing_search_manual_home_details_and_zero_error",
    "t08_empty_false_exception_late_cancel_timeout_and_revision",
    "t09_capabilities_bearer_cors_csp_and_resource_isolation",
]


def redact(output):
    # Cargo's runner prefixes contain long test identifiers, which the generic
    # capability scrubber treats as credentials. Preserve only named prefixes.
    from t09 import redact as credentials
    names = "(?:" + "|".join(map(re.escape, sorted(NATIVE | set(REGRESSIONS) | {LIVE_TEST.split("::")[-1]}))) + ")"
    prefix = re.compile(r"^(?:test (?:\w+::)*" + names + r" \.\.\.(?=\s|$)|    (?:\w+::)*" + names
                        + r"(?=\r?$)|thread '(?:\w+::)*" + names + r"' \(\d+\) panicked at )")
    result = []
    for line in output.splitlines(keepends=True):
        match = prefix.match(line)
        end = match.end() if match else 0
        def private_url(match):
            value = match[0]
            try:
                parsed = urlsplit(value)
            except ValueError:
                return "<redacted-invalid-url>"
            if parsed.hostname in {"maccms.fixture.test", "other.fixture.test", "127.0.0.1", "localhost"}:
                return value  # Owned fixture URLs; live gateway caps are checked below.
            return "<redacted-public-url>" if parsed.query or parsed.username or parsed.password else value
        safe = re.sub(r"https?://[^\s\"'<>]+", private_url, line[end:])
        result.append(line[:end] + credentials(safe))
    return "".join(result)


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.19", "T02", IDS

    def redact_output(self, output):
        return redact(output)

    def command(self, argv, timeout=3000):
        # Reuse the identity-aware process cleanup and command receipts, with
        # T02's inventory-aware redaction rather than another card's test names.
        import t11
        previous = t11.redact
        t11.redact = redact
        before = len(self.commands)
        overrides = dict(ROLLDOWN_WORKER_THREADS="2", RAYON_NUM_THREADS="2")
        try:
            with t01.environment(overrides):
                return super().command(argv, timeout)
        finally:
            if len(self.commands) > before:
                self.commands[-1]["env_overrides"] = overrides
            t11.redact = previous


def native(run):
    argv = ["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib"]
    output = run.command(argv + ["t02_", "--", "--test-threads=1", "--nocapture", "--skip", "t02_capture_public_json", "--skip", "t02_live_public_gateway_webview"])
    names = {name.split("::")[-1] for name in re.findall(r"^test (\S+) \.\.\.", output, re.M)}
    require(names == NATIVE and f"test result: ok. {len(NATIVE)} passed; 0 failed; 0 ignored;" in output,
            "All named T02 native tests must execute with no failure or skip")
    fixture = [json.loads(line.split("T02_FIXTURE:", 1)[1]) for line in output.splitlines() if "T02_FIXTURE:" in line]
    require(len(fixture) == 1 and fixture[0]["sources"] == 8 and fixture[0]["episodes"] >= 1200,
            "Public response fixture coverage is incomplete")
    for name in REGRESSIONS:
        result = run.command(argv + [name, "--", "--test-threads=1", "--nocapture"])
        # --nocapture prints observations between the test name and its "ok".
        require(f"::{name} ... " in result and "test result: ok. 1 passed; 0 failed; 0 ignored;" in result,
                "Required matching/no_links/cap isolation regression did not pass")
    run.observed["native_counts"] = dict(required=len(NATIVE) + len(REGRESSIONS), ran=len(NATIVE) + len(REGRESSIONS),
                                          passed=len(NATIVE) + len(REGRESSIONS), failed=0, skipped=0, status="PASS")
    return dict(public_fixture=fixture[0], regressions=REGRESSIONS, **run.observed["native_counts"])


def validate_sources(sources, selected):
    """ADR-051: a program defect fails acceptance; unavailable sources and media are recorded only."""
    names = []
    for source in sources:
        verdict = source.get("compatibility")
        require(verdict != "PROGRAM_INCOMPATIBLE", f"Adapter cannot use an accessible response from {source['name']}")
        require(verdict in {"COMPATIBLE", "SOURCE_UNAVAILABLE", "SOURCE_NO_CONTENT"}, f"{source['name']} has no compatibility result")
        summary = source["media_samples"]
        require(source["supported"] is (verdict == "COMPATIBLE") and summary["passed"] <= summary["attempted"] <= 8, "Source results are inconsistent")
        if source["supported"]:
            names.append(source["name"])
    require(any(row["name"] == selected and row["media_samples"]["passed"] > 0 for row in sources), "The short-play source must have an available sample")
    return names


def validate_short_play(value, *, check_audio=True):
    require(value["actual_gateway"] is True and value["error_count"] == 0, "Public media escaped its gateway or raised a WebView error")
    first, final = value["first"], value["final"]
    require(final["media_error"] is None and final["width"] > 0 and final["height"] > 0
            and final["frames"] > first["frames"] and final["position"] > first["position"] + 2
            and final["time"] >= first["time"] + 4900 and final["paused"] is False,
            "Real public media must decode frames and advance across five seconds of packaged WebView playback")
    if check_audio:
        require(value["native"]["audio_playing"] is True, "Real public media audio is not playing")


class Bridge(t12.Bridge):
    def __init__(self, run, folder):
        self.run, self.folder, self.sequence, self.lock = run, folder, 0, threading.Lock()
        argv = workflow.runtime.resolve_argv(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
            "--lib", LIVE_TEST, "--", "--exact", "--ignored", "--test-threads=1", "--nocapture"])
        path = run.folder / "public-host.log"
        self.record = dict(argv=argv, exit=None, log=str(path.relative_to(ROOT)))
        run.commands.append(self.record)
        self.log = path.open("w", encoding="utf-8")
        self.process = subprocess.Popen(argv, cwd=ROOT, env=dict(os.environ, T12_BRIDGE_DIR=str(folder)),
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        self.token = workflow.runtime.identity(self.process.pid)
        self.known, self.lines = {}, []

        def collect():
            for line in self.process.stdout:
                safe = redact(line)
                if safe != line:
                    run.observed["host_log_sanitized"] = True
                self.lines.append(safe)
                self.log.write(safe)
                self.log.flush()

        self.reader = threading.Thread(target=collect, name="t02-public-host-log", daemon=True)
        self.reader.start()

    def finish(self):
        self.invoke(dict(operation="finish"))
        self.process.wait(timeout=30)
        self.reader.join(timeout=5)
        self.record["exit"] = self.process.returncode
        require(self.process.returncode == 0 and not self.reader.is_alive(), "T02 live Host failed to close")
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in "".join(self.lines), "T02 live bridge test did not execute")
        vectors = [json.loads(line.split("T02_LIVE:", 1)[1]) for line in self.lines if "T02_LIVE:" in line]
        require(len(vectors) == 1 and vectors[0]["active_requests"] == 0, "T02 live cleanup evidence missing")
        require(not self.run.observed.get("host_log_sanitized"), "Live native output contained credentials; stored output was sanitized")
        self.run.observed["host"] = vectors[0]


def packaged_exclusion(receipt):
    config = read_json(ROOT / "src-tauri/tauri.conf.json")
    require(config["bundle"].get("resources") == [], "Bundle resources must exclude historical Android inputs")
    for key in ("production", "automation"):
        bundle = ROOT / receipt[key]["path"]
        require(not any(p.suffix.lower() in {".apk", ".dex", ".cs3"} or p.name == "android-engine" for p in bundle.rglob("*")),
                "Historical Android/DEX package reached a current bundle")
    return dict(production=True, automation=True, historical_artifacts="preserved outside both bundles")


def webview(run):
    require(t01.native_gui_session()["screen_locked"] is not True, "Unlock macOS before packaged public media acceptance")
    previous = t01.DATA, t01.BUILD
    t01.DATA = run.folder / "packaged"
    t01.DATA.mkdir()
    t01.BUILD = t01.DATA / "build.json"
    try:
        t01.check_media()
        t01.build(run)
        receipt = t01.check_build(run.source)
        run.observed["packaged_exclusion"] = packaged_exclusion(receipt)
        executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/moc-video"
        with tempfile.TemporaryDirectory(prefix="t02-live-", dir=run.folder) as temporary:
            folder = Path(temporary)
            bridge, session = None, None
            try:
                bridge = Bridge(run, folder)
                ready = bridge.wait(folder / "ready.json", timeout=900)
                run.observed["public_sources"] = ready["sources"]
                if ready["status"] != "READY":
                    bridge.finish()
                    require(False, "NOT RUN: no public candidate passed direct media prefix; public source evidence retained")
                run.observed["public_protocol"] = ready["protocol"]
                with t01.Fixture(t01.MEDIA) as fixture:
                    session = t01.NativeSession(run, fixture, executable, 1)
                    session.wait(0, 30)
                    identity = session.command("identity")
                    require(identity["origin"] == "tauri://localhost", "Packaged WKWebView origin differs")
                    value = session.command("t02-public", timeout=120, view=ready["view"], kind=ready["kind"])
                    session.record["commands"][-1]["args"] = dict(view="actual Rust gateway view; capability redacted", kind=ready["kind"])
                    run.observed["short_play"] = value
                    validate_short_play(value)
                    session.command("stop")
                    session.close()
                    require(session.record.get("exit") == 0 and session.record.get("lingering") == [], "Owned WKWebView processes remain")
                bridge.finish()
                require(run.observed["host"]["ready"] is True and run.observed["host"]["closed"] is True, "Public Host session was not closed")
                run.observed["supported_sources"] = validate_sources(ready["sources"], ready["source"])
                return dict(source=ready["source"], group_index=ready["group_index"], kind=ready["kind"], short_play_seconds=5,
                            chain="public JSON/list/detail/full episodes/Host resolve/Rust gateway/packaged WKWebView")
            finally:
                try:
                    if session is not None and not session.closed:
                        session.close(graceful=False)
                finally:
                    if bridge is not None:
                        path = folder / "public-observations.json"
                        if path.exists() and "public_sources" not in run.observed:
                            run.observed["public_sources"] = read_json(path)["sources"]
                        bridge.close()
    finally:
        t01.DATA, t01.BUILD = previous


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T02-all.json"
            and args.junit.resolve() == ROOT / "evidence/acceptance/T02-all.xml", "Exact T02 output paths required")
    run = Acceptance(args.report, args.junit)
    run.observed.update(supported_sources=[], public_gateway_webview="NOT RUN", platforms="macOS arm64 required; others NOT RUN (ADR-005/006)")
    ready = run.case("resource-ownership", ["REQ-T02"], "original stable-source receipt; 4096MiB RSS sampling; jobs=2/test_threads=1",
                     lambda: resources(run, card="T0.19"))
    if ready:
        def oracles():
            output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t02.py"])
            count = re.search(r"Ran (\d+) tests?", output)
            require(count and int(count[1]) >= 5, "Independent public playback evidence oracles must run")
            return dict(tests=int(count[1]))
        run.case("independent-playback-oracles", ["REQ-T02"], "stalled frames/time, missing audio, wrong gateway and credential output rejected", oracles)
    fixture = run.case("public-json-and-dns-safety", IDS, "named protocol fixtures, DNS failover/failure/cancel/SSRF, not_found/no_links and cap isolation",
                       lambda: native(run) if ready else require(False, "NOT RUN: resource ownership failed"))
    def live():
        require(fixture, "NOT RUN: native protocol and safety prerequisite failed")
        value = webview(run)
        run.observed["public_gateway_webview"] = "PASS"
        return value
    run.case("real-public-packaged-short-play", IDS, "at least one direct public media chain decodes in packaged WKWebView", live)
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
