"""T0.37: current source, production MacCMS and three fresh native windows."""
import argparse
import json
import re
import tempfile
from pathlib import Path

import t01
import t02
import t12
import t14
from t00 import require
from t10 import resources

ROOT = Path(__file__).resolve().parents[2]
IDS = ["REQ-LINE-PREFERENCE", *[f"REQ-LINE-PREFERENCE/{name}" for name in (
    "three-lines", "later-episodes", "restart", "races", "identity-and-fallback", "storage-and-secrets", "playback-history", "same-source")]]
LIVE_TEST = "host::maccms::tests::t08::t09::t37::t37_packaged_host_bridge"


class Acceptance(t02.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.37", "LINE-PREFERENCE", IDS
    DEPLOYMENT = "current production App, MacCMS adapter, real Host/gateway and three independently launched packaged WKWebViews; owned H264 media"
    DEFERRED = [dict(target=target, status="NOT RUN", reason="ADR-005/006: required local macOS arm64") for target in t01.DEFERRED]

    def redact_output(self, output):
        return t02.redact(output)


class Bridge(t14.Bridge):
    def finish(self):
        super().finish()
        require(self.run.observed["host"]["restarts"] == 2, "Two real Host restarts are required")


def validate_cross(e):
    require(e["production_preserved"] is True, "Acceptance must preserve the user's production preference bytes")
    require(e["first_failure"]["saved"] is False and e["first_failure"]["opened"] == 1 and e["first_failure"]["code"], "First line must fail actual decoding without being remembered")
    require(e["second_failure"]["saved"] is False and e["second_failure"]["opened"] == 1 and e["second_failure"]["code"] == "MEDIA_UNSUPPORTED", "Second line must be unavailable without opening or saving a Session")
    require(e["resolve_only"] == dict(saved=False, opened=2), "Resolve/playable and an opened Host Session must not save before decoding")
    t = e["third"]
    require(t["route"] == "2" and t["saved"] is True and t["frames"] > 1 and t["position"] > .15 and t["null_coordinates"] is True and t["record_count"] == 1, "Third production MacCMS route must decode and save with null season/number")
    for name in ("second", "other"):
        require(e[name] == dict(route="2", fresh=True, dialog=False), f"{name} must choose the third line with fresh handles and no confirmation")
    require(e["no_failed_retry"] is True, "Failed earlier routes were retried before the remembered route")
    require([row["mode"] for row in e["ambiguities"]] == list(range(1, 12)), "All real entry ambiguity/content replacement mutations must run")
    require(all(row["confirmation"] is True and row["opens"] == 0 and row["kept"] is True for row in e["ambiguities"]), "Ambiguity must keep confirmation, stored success and zero new opens")
    require(e["unavailable_line"] == dict(confirmation=True, opens=0, kept=True) and e["source_unavailable"] == dict(confirmation=True, alternate=True, kept=True), "Unplayable remembered lines and disabled sources must use the original confirmation flow")
    require(e["unavailable_read"] == dict(notice=True, preserved=True) and e["corruption"] == dict(notice=True, preserved=True), "Storage read errors must report and preserve exact bytes")
    require(e["failed_write"] == dict(notice=True, kept=True, advanced=True, saved=False) and e["recovered_write"] is True, "Quota failure must keep playing, preserve confirmed bytes and recover")
    require(e["manual_override"]["route"] == "0", "A successful explicit manual line must replace the preference")
    require(e["race"] == dict(selected=True, context=True, kept=True, pending=True), "A late old response must lose to manual source selection")
    require(all(e["restart_choice"].values()), "Global manual source and per-film successful source must differ before restart")
    require(e["storage"] == dict(records=1, sensitive=False) and e["errors"] == [], "Durable records must contain no temporary or sensitive values; pages must have no uncaught errors")


def validate_same(e):
    require(e["production_preserved"] is True, "The second window must preserve production preferences")
    require(e["cross_restart"] == dict(route="2", changed_global=True, records=1), "A new native window/Host must read the film's successful source and third line from disk")
    require(e["same_second"] == dict(route="2", video=True, fresh=True, old_http=410), "Same-source route change must preserve video identity, create fresh handles and revoke old Session")
    require(e["same_third"] == dict(route="2", video=True, history_video=True, fresh=True, boundary=True), "Same-source next/history identity and group boundary must be preserved")
    require(e["storage"] == dict(records=2, sensitive=False) and e["errors"] == [], "Cross-source and same-source films must have isolated safe records")


def validate_restart(e):
    require(e["production_preserved"] is True, "The third window must preserve production preferences")
    require(e["same_restart"] == dict(route="2", video=True, saved=True), "Same-source third line must survive another complete native window/Host restart")
    require(e["original_fallback"] == [dict(mode=mode, route="0", video=True, original_group=True) for mode in (1, 2)], "Same-source reorder/ambiguity must retain the current original flow")
    require(e["revision"] == dict(changed=True, original_group=True), "A new source revision must use the original flow")
    require(e["other_film"] == dict(different=True, original_group=True), "Another film must retain its own original route")
    require(e["storage"] == dict(records=2, sensitive=False) and e["errors"] == [], "Revisions and rejected choices must keep confirmed safe records")


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap",
        "tests/line-preference.test.ts", "tests/state.test.ts", "tests/playback.test.ts", "tests/player.test.ts", "tests/playback-ui.test.ts", "tests/history.test.ts", "tests/tv4.test.ts"], timeout=300)
    passed = re.search(r"^# pass (\d+)$", output, re.M)
    require(passed and int(passed[1]) >= 80 and re.search(r"^# fail 0$", output, re.M) and "REQ-LINE-PREFERENCE" in output, "Current matching/playback/switch/history/manual-choice regressions must run and pass")
    run.command(["npm", "run", "check"], timeout=300)
    return dict(passed=int(passed[1]))


def native(run):
    # These four source-defined test names are exactly capability-shaped. Reuse
    # the established inventory-aware logger; arbitrary values still fail it.
    t02.NATIVE = t02.NATIVE | {"t10_cross_origin_private_key_and_closed_cap", "t02_unusable_poster_is_treated_as_no_poster",
        "n00_query_search_encoding_and_empty_results", "n00_public_addresses_dns_rebinding_and_peer"}
    output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "--", "--test-threads=1"], timeout=600)
    require("t37_same_source_route_keeps_history_video_and_revalidates ... ok" in output and re.search(r"test result: ok\. [1-9]\d* passed; 0 failed;", output), "Current real Host regressions and same-source revalidation must pass")
    result = re.search(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;", output)
    return dict(passed=int(result[1]), existing_ignored=int(result[2]))


def windows(run):
    require(t01.native_gui_session()["screen_locked"] is not True, "Unlock the macOS desktop for real window acceptance")
    previous = t01.DATA, t01.BUILD, t12.LIVE_TEST
    t01.DATA = run.folder / "packaged"
    t01.DATA.mkdir()
    t01.BUILD, t12.LIVE_TEST = t01.DATA / "build.json", LIVE_TEST
    bridge, session = None, None
    try:
        t01.check_media()
        t01.build(run)
        receipt = t01.check_build(run.source)
        executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/yingxiang-video"
        run.observed["prior_fixed_build"] = dict(commit="e6c9b946636adb29afe1bc849f536d2244b1dc38", status="PRE_CHANGE_T0.35_ONLY", validates_current_source=False)
        with tempfile.TemporaryDirectory(prefix="t37-owned-", dir=run.folder) as folder:
            folder = Path(folder)
            media = folder / "media"
            media.mkdir()
            t12.owned_media(run, media, seconds=18)
            bridge = Bridge(run, folder, media)
            ready = bridge.wait(folder / "ready.json", timeout=180)
            phases = [("cross-source", validate_cross), ("cross-restart-and-same-source", validate_same), ("same-source-restart", validate_restart)]
            for index, (action, validate) in enumerate(phases, 1):
                if index > 1:
                    bridge.invoke(dict(operation="restart"))
                with t01.Fixture(t01.MEDIA, invoke=bridge.invoke) as fixture:
                    session = t01.NativeSession(run, fixture, executable, index)
                    session.wait(0, 30)
                    session.command("identity")
                    e = session.command("t37", timeout=420, action=action, payload=ready)
                    run.observed[action] = e
                    validate(e)
                    session.command("identity")
                    session.close()
                    require(session.record.get("exit") == 0 and session.record.get("lingering") == [] and session.record.get("forced_cleanup") == [], "Owned native window and WebKit must exit cleanly")
                    require(fixture.active == 0, "Owned fixture requests remain")
            bridge.finish()
        return dict(windows=3, host_restarts=2, current_build_source=run.source, decoded=True, checks=[action for action, _ in phases])
    finally:
        try:
            if session is not None and not session.closed:
                session.close(graceful=False)
        finally:
            if bridge is not None:
                bridge.close()
            t01.DATA, t01.BUILD, t12.LIVE_TEST = previous


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/LINE-PREFERENCE.json" and args.junit.resolve() == ROOT / "evidence/acceptance/LINE-PREFERENCE.xml", "Exact T0.37 report paths required")
    run = Acceptance(args.report, args.junit)
    if run.case("resource-ownership", IDS[:1], "original actual resource backend, 4096MiB sampled on macOS, jobs2/threads1", lambda: resources(run, card="T0.37")):
        def oracles():
            output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_line_preference.py"])
            count = re.search(r"Ran (\d+) tests?", output)
            require(count and int(count[1]) >= 5, "Behavior oracle corruption tests must run")
            return dict(tests=int(count[1]))
        run.case("negative-evidence-oracles", IDS[:1], "mutated native evidence is rejected", oracles)
        run.case("current-frontend-regressions", IDS, "current matching/playback/episode/history/manual-choice, storage/races and decoded frame triggers", lambda: frontend(run))
        run.case("current-host-regressions", IDS, "full current native Host regression suite including source/session/history checks", lambda: native(run))
        run.case("three-native-windows", IDS, "production MacCMS three-line multi-episode original format; complete window/Host restarts; real decoded frame progression; ambiguity/storage/manual overrides", lambda: windows(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
