"""T0.18: Settings and SQLite history lifecycle through the protocol, the real Host and a packaged WKWebView."""
import argparse
import os
from pathlib import Path
import re
import uuid

from t00 import require
from t10 import resources
import n02
import t02
import t12
import t14

ROOT = Path(__file__).resolve().parents[2]
IDS = ["U01", "U09", "R11", "R36", "R38", "R39", "R40", "R41", "R42", "R43", "R64", "R65", "REQ-T18"]
NATIVE = {
    "t18_sqlite_file_columns_index_restart_and_parameter_binding",
    "t18_migration_failure_and_newer_version_keep_the_file",
    "t18_sequences_barriers_and_identical_retry",
    "t18_delete_and_clear_block_older_writes_from_returning",
    "t18_snapshot_must_match_the_session_identity",
    "t18_active_session_keeps_saving_after_match_records_are_collected",
    "t18_window_bounds_return_after_a_restart",
    "t18_every_close_request_waits_for_the_same_save",
    "t18_non_uuid_match_id_is_carried_and_bad_handles_are_refused",
    "t18_unusable_database_path_reports_and_recovers_with_the_same_snapshot",
    "t18_source_change_stops_new_snapshots_but_keeps_stored_history",
    "t18_fifty_switches_release_every_session_and_request",
}
SETTINGS = ["t05_settings_validation_and_failure_atomicity", "t05_migration_corruption_and_unknown_version", "t05_same_lock_commits_and_atomic_cancel"]
REGRESSIONS = ["t04_stale_work_and_exit_release_only_this_host"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.18", "T18", IDS
    DEPLOYMENT = "packaged Tauri WKWebView with real Host/gateway and a real SQLite file over owned fixtures and media; no cluster or Android"

    def redact_output(self, output):
        return t02.redact(output)


def validate(e):
    require(e["empty_before"] == 0, "History must start empty")
    s = e["saved"]
    require(s["rows"] == 1 and s["write_sequence"] >= 1 and s["position_ms"] > 0 and all(s[name] is True for name in ("video", "source", "identity", "content")), "A real snapshot must hold the confirmed identity: " + str(s))
    require(s["runtime_fields"] is False, "Stored history must hold no URL, capability or Session field")
    f = e["failed_write"]
    require(all(f[name] is True for name in ("notice", "row_kept", "same_sequence")) and f["preview_note"] is False, "A failed write must warn, keep the row and retry its sequence: " + str(f))
    r = e["removed"]
    require(r["kept_until_confirmed"] == 1 and r["rows_after"] == 0, "Removing must confirm first and not come back")
    t = e["restart"]
    require(t["new_epoch"] is True and t["rows"] == 1 and t["write_sequence"] == 0, "Rows must survive a restart under a new Host sequence")
    u = e["resumed"]
    require(u["video"] is True and abs(u["at"] - u["saved_ms"] / 1000) < .8, "Resume must reach the saved position of the saved video")
    require(e["cleared"]["rows"] == 0, "Clear must empty the database")
    b = e["bounds"]
    require(b["saved"]["width"] == b["width"] and b["saved"]["height"] == b["height"], "Saved bounds must use the native size the Host restores: " + str(b))
    require(e["closing"]["rows_before"] == 0 and e["closing"]["video"], "The close test must start with no stored row")
    require(e["errors"] == [], "Pages raised errors: " + str(e["errors"]))


def closed_window_kept_everything(bridge, e):
    """Runs after the real window has gone: the Host must hold what the page was still saving."""
    def invoke(command):
        return bridge.invoke(dict(command=command, args=dict(request_id=str(uuid.uuid4()), input={})))["data"]
    entries = invoke("list_history")["entries"]
    require(len(entries) == 1 and entries[0]["video_id"] == e["closing"]["video"] and entries[0]["position_ms"] > 0, "Closing must save the playing episode: " + str(entries))
    settings = invoke("get_settings")
    require(abs(settings["volume"] - .37) < .001 and settings["auto_next"] is True,
            f"Closing must finish slow queued changes and the latest audio: {settings['volume']}, {settings['auto_next']}")
    require(settings["window_bounds"] == e["bounds"]["saved"], "Closing must keep the window bounds")
    e["closing"].update(saved=dict(position_ms=entries[0]["position_ms"]), volume=settings["volume"], auto_next=settings["auto_next"])
    e["closing"].update(history_records=entries,settings_snapshot=settings)


def contracts(run):
    output = run.command(["{python}", "-B", "scripts/check-contracts.py"])
    match = re.search(r'"commands":\s*(\d+)', output)
    require(match and int(match[1]) == 27, "The working protocol must retain all existing commands and C17 watchlist/search commands")
    return output.strip()[:300]


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/history.test.ts", "tests/sources.test.ts"], timeout=300)
    count, fail = re.search(r"# pass (\d+)", output), re.search(r"# fail (\d+)", output)
    require("REQ-T18" in output and count and int(count[1]) > 0 and fail and int(fail[1]) == 0, "Frontend tests must run nonzero with no failures")
    return dict(tests=int(count[1]))


def native(run):
    argv = ["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml"]
    output = run.command(argv + ["--lib", "t18_", "--", "--test-threads=1", "--nocapture"], timeout=600)
    names = {name.split("::")[-1] for name in re.findall(r"^test (\S+) \.\.\.", output, re.M)}
    require(names == NATIVE and f"test result: ok. {len(NATIVE)} passed; 0 failed; 0 ignored;" in output, "All named T18 native tests must execute with no failure or skip")
    for name in SETTINGS:
        result = run.command(argv + ["--test", "t05", name, "--", "--test-threads=1", "--nocapture"], timeout=600)
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in result, f"Required settings regression did not pass: {name}")
    for name in REGRESSIONS:
        result = run.command(argv + ["--lib", name, "--", "--test-threads=1", "--nocapture"], timeout=600)
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in result, f"Required cleanup regression did not pass: {name}")
    total = len(NATIVE) + len(SETTINGS) + len(REGRESSIONS)
    run.observed["native_counts"] = dict(required=total, ran=total, passed=total, failed=0, skipped=0, status="PASS")
    return run.observed["native_counts"]


def webview(run):
    os.environ["T12_MIN_OPENS"] = "1"
    try:
        return t14.packaged(run, n02.LIVE_TEST, "t18", validate, "history", lambda e: dict(saved=e["saved"], closing=e["closing"]), bridge_class=n02.Bridge, prepare_media=t12.owned_media, command_timeout=360, after_close=closed_window_kept_everything)
    finally:
        os.environ.pop("T12_MIN_OPENS", None)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T18-all.json" and args.junit.resolve() == ROOT / "evidence/acceptance/T18-all.xml", "Exact T18 output paths required")
    t02.NATIVE = t02.NATIVE | NATIVE | set(SETTINGS) | set(REGRESSIONS) | {n02.LIVE_TEST.split("::")[-1]}
    run = Acceptance(args.report, args.junit)
    if run.case("resource-ownership", ["REQ-T18"], "scope/source bound 4096MiB RSS sampled backend", lambda: resources(run, card="T0.18")):
        run.case("protocol-positive-and-negative-cases", ["R38", "R64", "REQ-T18"], "four history commands in the working protocol with both kinds of cases", lambda: contracts(run))
        run.case("frontend-history-queue", ["R38", "R43", "REQ-T18"], "one queue, sequence reread, identical retry", lambda: frontend(run))
        run.case("sqlite-host-and-settings", IDS, "real file, migration rollback, sequences, barriers, Session identity, GC, 50 switches, settings failures", lambda: native(run))
        run.case("packaged-history", IDS, "real snapshot, failure notice and retry, remove, restart, resume, clear, and two real close requests during slow saves keeping history and settings", lambda: webview(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
