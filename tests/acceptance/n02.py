"""N02: Rust sources in the production source manager and playback flow, on a real Host and packaged WKWebView."""
import argparse
import json
import os
from pathlib import Path
import re

from t00 import read_json, require
from t10 import resources
import t02
import t12
import t14
import t16

ROOT = Path(__file__).resolve().parents[2]
IDS = ["U02", "U05", "U08", "R03", "R05", "R06", "R10", "R13", "R15", "R16", "R24", "R26", "R27", "R29", "R36", "R49", "R50", "R63", "R66", "REQ-N02"]
LIVE_TEST = "host::maccms::tests::t08::t09::n02::n02_host_sources_webview_bridge"
REGRESSIONS = [
    "t04_one_failing_source_keeps_other_sources_and_stremio_working",
    "t04_access_refusal_is_reported_and_stops_the_source",
    "t09_host_session_bindings_switch_release_and_old_cleanup",
    "t09_active_over_five_minutes_release_and_original_network_context",
    "t09_same_episode_multiple_streams_and_late_close_release",
    "t09_range_if_range_200_206_416_and_head",
    "t10_active_session_release_clock_refresh_key_segment_and_close",
    "t11_release_expiry_switch_and_cap_revocation",
    "n00_type_probe_formats_html_unknown_and_bound",
    "t09_host_inflight_resolve_cancel_and_late_result_rejected",
]

class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.22", "N02", IDS
    DEPLOYMENT = "packaged Tauri WKWebView with real Host/gateway; controlled HTTP fixtures plus ordinary public requests to the supported sources; no cluster or Android"

    def redact_output(self, output):
        return t02.redact(output)

class Bridge(t16.Bridge):
    def finish(self):
        self.invoke(dict(operation="finish"))
        self.process.wait(timeout=30)
        self.reader.join(timeout=5)
        self.record["exit"] = self.process.returncode
        require(self.process.returncode == 0 and not self.reader.is_alive(), "Real Host bridge did not finish cleanly")

        vectors = [json.loads(line.split("T14_VECTOR:", 1)[1]) for line in self.lines if "T14_VECTOR:" in line]
        require(len(vectors) == 1 and vectors[0]["get_home"] > 0 and vectors[0]["restarts"] == 1 and vectors[0]["active_requests"] == 0, "Real Host evidence missing")
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in "".join(self.lines), "Actual named native test did not pass")
        require(not self.run.observed.get("host_log_sanitized"), "Native output contained credentials; stored log was sanitized")
        self.run.observed["host"] = vectors[0]

def validate(e):
    supported = read_json(ROOT / "docs/compatibility-report.json")["supported_sources"]
    f = e["first_run"]
    require(f["presets"] == supported and f["configs"] == 0, "Presets must be discoverable and nothing added on its own")
    require(f["cloudstream_section"] is False and f["android_text"] is False, "Android preparation and interaction entries must be gone")
    require(f["legacy"] is True and f["preference_kept"] is True, "An old preference must be explained and kept")
    i = e["invalid"]
    require(i["saves"] == 0 and len(i["messages"]) == 3 and all(re.search("[一-鿿]", m) for m in i["messages"].values()) and "hidden" not in str(i["messages"]),
            "Invalid drafts must not save and must read in Chinese without credentials")
    rows = e["public"]
    require([row["name"] for row in rows] == supported and all(row["duplicate_blocked"] is True for row in rows), "Every preset must be added once and then blocked")
    require(any(row["registration"] == "ready" and row["capabilities"]["resolve"] for row in rows), "At least one public source must register through the production button")
    require(all(row["kind"] in (None, "maccms") for row in rows), "Presets reuse the maccms kind")
    require(e["public_cleanup"]["after"] == 0, "Deleting the presets must leave no configuration")
    require(e["legacy_after_fallback"]["note"] is True and e["legacy_after_fallback"]["key"] is not None, "A fallback must not silently replace the preference")
    s = e["selection"]
    require(s["metadata_kept"] is True and s["legacy_cleared"] is True and s["switched"] is True, "An explicit choice updates the preference and leaves metadata alone")
    require(e["edit"]["same_id"] is True and e["edit"]["same_source"] is True, "Editing must keep the identity")
    require(e["disabled"]["option"] is False and e["disabled"]["selected_kept"] is True, "Disabling must withdraw only that source")
    g = e["failing"]
    require(g["message"] and g["others_ready"] is True and g["selection_kept"] is True, "A failing source must report its error and leave others alone")
    r = e["refused"]
    require("拒绝访问" in r["message"] and "403" in r["message"] and "已停止请求" in r["message"] and r["others_ready"] is True, "Access control must be named and leave others alone")
    d = e["diagnostics"]
    require(d["failure_listed"] is True and d["secret"] is False and d["old_engine"] is False, "Diagnostics must list failures, stay redacted and drop engine lines")
    m = e["limit"]
    require(m["configs"] == 32 and m["saves"] == 0, "The limit must stop a preset before saving")
    t = e["restart"]
    require(all(t[name] is True for name in ("new_epoch", "configs_kept", "playback_kept", "metadata_kept")), "Restart must restore sources and choices: " + str(t))
    p = e["flow"]
    require(p["video_id"] == p["context_video"] and p["opens_before_confirm"] == 0 and len(p["candidates"]) == 3 and p["source"] is True and p["advanced"] is True and p["gateway"] is True,
            "Manual confirmation must precede resolving the chosen video through the gateway: " + str(p))
    w = e["switched"]
    require(w["asked_again"] is True and w["different"] is True, "An unverified next episode must ask again and then play")
    a = e["ambiguous"]
    require(a["dialog"] is True and a["opens"] == 0 and a["session"] is None, "Unattended next must stop with nothing playing")
    require(e["cleanup"]["session_after_cancel"] is None, "Cancel must leave no session")
    require(e["errors"] == [], "Pages raised errors: " + str(e["errors"]))

def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/sources.test.ts", "tests/state.test.ts", "tests/api.test.ts"], timeout=300)
    count, fail = re.search(r"# pass (\d+)", output), re.search(r"# fail (\d+)", output)
    require("REQ-N02" in output and count and int(count[1]) > 0 and fail and int(fail[1]) == 0, "Frontend tests must run nonzero with no failures")
    return dict(tests=int(count[1]))

def native(run):
    argv = ["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib"]
    for name in REGRESSIONS:
        result = run.command(argv + [name, "--", "--test-threads=1", "--nocapture"], timeout=600)
        require(f"::{name} ... " in result and "test result: ok. 1 passed; 0 failed; 0 ignored;" in result, f"Required regression did not pass: {name}")
    run.observed["native_counts"] = dict(required=len(REGRESSIONS), ran=len(REGRESSIONS), passed=len(REGRESSIONS), failed=0, skipped=0, status="PASS")
    return dict(regressions=REGRESSIONS, **run.observed["native_counts"])

def webview(run):
    def summary(e):
        ready = [row["name"] for row in e["public"] if row["registration"] == "ready"]
        run.observed["public_sources"] = dict(stimulus="ordinary public requests through the production button", ready=ready, rows=e["public"], media="NOT RUN here; real media chain in T0.19/T0.24")
        run.observed["controlled"] = dict(stimulus="owned HTTP fixtures and owned media", flow=e["flow"], switched=e["switched"], ambiguous=e["ambiguous"])
        return dict(public_ready=ready, controlled_flow="recommendation/details/video/confirm/resolve/play/next")
    os.environ["T12_MIN_OPENS"] = "1"
    try:
        return t14.packaged(run, LIVE_TEST, "n02", validate, "sources", summary, bridge_class=Bridge, prepare_media=t12.owned_media, command_timeout=720)
    finally:
        os.environ.pop("T12_MIN_OPENS", None)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/N02-all.json" and args.junit.resolve() == ROOT / "evidence/acceptance/N02-all.xml", "Exact N02 output paths required")
    t02.NATIVE = t02.NATIVE | set(REGRESSIONS) | {LIVE_TEST.split("::")[-1]}
    run = Acceptance(args.report, args.junit)
    if run.case("resource-ownership", ["REQ-N02"], "scope/source bound 4096MiB RSS sampled backend", lambda: resources(run, card="T0.22")):
        run.case("frontend-behaviors", ["R13", "R24", "REQ-N02"], "presets match the report; Chinese validation and access messages", lambda: frontend(run))
        run.case("controlled-session-and-media-regressions", ["R05", "R10", "R13", "R15", "R16", "R36", "R49", "R63", "REQ-N02"],
                 "isolation, Session hold, candidate release, Range, HLS, subtitles, conditional DASH, cancel and cleanup", lambda: native(run))
        run.case("packaged-sources-to-playback", IDS, "public presets counted apart from the controlled recommendation-to-playback flow", lambda: webview(run))
    return run.save()

if __name__ == "__main__":
    raise SystemExit(main())
