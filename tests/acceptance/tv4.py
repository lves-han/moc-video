"""TV4: eight captured default classifications, real Host and packaged MacCMS playback."""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import t01
import t12
import t14
import tv1
from t00 import Acceptance as CommandRunner, require, write_json
from t10 import resources

ROOT = Path(__file__).resolve().parents[2]
LIVE_TEST = "host::maccms::tests::t08::t09::tv4::tv4_packaged_host_bridge"
CHILDREN = {
    "TV-MACCMS-CAT": ["normalize-names", "excluded-classes", "hide-missing", "client-filter", "visual-spec"],
    "TV-MACCMS-PLAY": ["same-source-play", "history-resume", "stremio-binding-kept"],
}
IDS = [*CHILDREN, *[f"{parent}/{name}" for parent, names in CHILDREN.items() for name in names], "REQ-TV4"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.31", "TV4", IDS
    DEPLOYMENT = "actual packaged Tauri WKWebView and production Rust Host/gateway/SQLite; captured public classifications and controlled owned media"
    DEFERRED = [dict(target=target, status="NOT RUN", reason="ADR-005/006; macOS arm64 local acceptance") for target in t01.DEFERRED]
    command = CommandRunner.command


def case(run, parent, name, expected, function):
    return run.case(name, [parent, f"{parent}/{name}", "REQ-TV4"], expected, function)


def native(run):
    output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "tv4_",
                          "--", "--test-threads=1", "--nocapture", "--skip", "tv4_packaged_host_bridge"], timeout=600)
    expected = {"tv4_normalize_eight_sources_and_exclude", "tv4_client_filters_five_pages_and_facets",
                "tv4_same_source_history_and_binding_negatives", "tv4_stremio_cross_source_confirmation_kept"}
    names = re.findall(r"^test (?:\w+::)*(tv4_\w+) \.\.\.", output, re.M)
    require(set(names) == expected and "test result: ok. 4 passed; 0 failed; 0 ignored;" in output, "four named real Host behavior tests must pass")
    observations = {}
    for marker in ["TV4_CLASSES", "TV4_FILTER", "TV4_SAME_SOURCE", "TV4_CROSS_SOURCE"]:
        rows = [json.loads(line.split(marker + ":", 1)[1]) for line in output.splitlines() if line.startswith(marker + ":")]
        require(len(rows) == 1, f"one actual {marker} observation required")
        observations[marker] = rows[0]
    require(len(observations["TV4_CLASSES"]) == 8 and {s["source"] for s in observations["TV4_CLASSES"]} == {"优质", "量子", "红牛", "无尽", "光速", "极速", "豪华", "淘片"}, "eight shipped source samples required")
    write_json(run.folder / "host-observations.json", observations)
    run.observed["native"] = dict(tests=4, names=names, observations=observations)
    return run.observed["native"]


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-reporter=tap", "--test-concurrency=1", "tests/tv4.test.ts", "tests/tv3-images.test.ts"])
    require(all(re.search(fr"^# {key} {value}$", output, re.M) for key, value in [("tests", 6), ("pass", 6), ("fail", 0), ("skipped", 0)]), "six nonzero frontend binding/image regressions required")
    return dict(tests=6, passed=6, failed=0, skipped=0)


def validate_styles(row):
    require(row["width"] in [960, 1200, 1512] and not row["overflow"] and not row["errors"], "MacCMS viewport/console health differs")
    require(row["origin"] == "tauri://localhost" and "Moc" in row["title"], "real packaged page identity missing")
    s = row["styles"]
    hero, background, cover = [s[key] for key in [".catalog-hero", ".catalog-hero-slide.current img", ".hero-cover-poster"]]
    require(background["filter"] == "blur(40px)" and background["animation-name"] == "none", "poster backdrop needs stable 40px blur")
    require(background["transform"].startswith("matrix(1.2,"), "poster backdrop needs 1.2 scale")
    tv1.near(cover["rect"]["height"], hero["rect"]["height"] * .6, "Hero vertical poster height")
    require(cover["right"] == "64px" and cover["border-radius"] == "12px" and cover["object-fit"] == "cover", "Hero vertical poster geometry differs")
    require("rgb(11, 11, 13)" in s[".hero-bottom-mask"]["background-image"] and "rgba(11, 11, 13, 0.75)" in s[".hero-left-mask"]["background-image"], "TV3 masks changed")
    poster, image, badge = [s[key] for key in [".maccms-catalog .media-poster", ".maccms-catalog .media-poster img", ".media-update-label"]]
    tv1.near(poster["rect"]["height"] / poster["rect"]["width"], 1.5, "MacCMS poster 2:3 ratio", .01)
    require(image["object-fit"] == "cover" and image["opacity"] == "1" and poster["border-radius"] == "12px", "MacCMS image crop/fade/radius differs")
    require(badge["left"] == badge["bottom"] == "8px" and badge["height"] == "22px" and badge["padding-left"] == badge["padding-right"] == "8px"
            and badge["font-size"] == "11px" and badge["font-weight"] == "600", "update badge geometry/type differs")
    require(badge["max-width"] == "calc(100% - 16px)", "badge maximum-width rule changed")
    tv1.near(badge["width"], float(poster["width"].removesuffix("px")) - 16, "badge maximum width before focus scale", .2)
    require(badge["text-overflow"] == "ellipsis" and badge["white-space"] == "nowrap" and badge["background-color"] == "rgba(30, 30, 34, 0.62)"
            and "blur(30px)" in badge["-webkit-backdrop-filter"], "update badge glass/ellipsis differs")
    selected, plain = [s[key] for key in [".maccms-subcategories .filter-pill.selected", ".maccms-subcategories .filter-pill:not(.selected)"]]
    require(selected["background-color"] == "rgb(245, 245, 247)" and selected["color"] == "rgb(11, 11, 13)"
            and plain["background-color"] == "rgba(255, 255, 255, 0.1)" and plain["border-radius"] == "999px"
            and plain["font-size"] == "14px" and plain["height"] == "36px", "subcategory capsules differ from filter capsules")
    require(row["missing"] == 1 and "8.5" in row["rating"][0] and "★" not in row["rating"][1] and row["zero_rating_hidden"], "missing poster or zero-rating state differs")
    return dict(width=row["width"], poster=poster["rect"], badge=badge["rect"])


def validate_details(row):
    require(not row["errors"] and row["background"]["filter"] == "blur(40px)" and row["background"]["transform"].startswith("matrix(1.2,"), "detail backdrop differs")
    require("rgb(11, 11, 13)" in row["mask"]["background-image"] and "rgba(11, 11, 13, 0.75)" in row["mask"]["background-image"], "detail two-layer masks missing")
    tv1.near(row["poster"]["rect"]["height"], row["head"]["rect"]["height"] * .6, "detail vertical poster height")
    require(row["poster"]["right"] == "64px" and row["poster"]["border-radius"] == "12px", "detail poster inset/radius differs")
    require(row["source"]["font-size"] == "13px" and row["source"]["color"] == "rgba(235, 235, 245, 0.62)"
            and row["source_name"] in row["source"]["content"], "detail source hint missing from information line")
    return row


def validate_play(row):
    first, second, history, resumed = [row[key] for key in ["first", "second", "history", "resumed"]]
    require(first["decoded"] and second["decoded"] and resumed["decoded"] and first["video"] != second["video"] and first["session"] != second["session"], "same-source actual media/episode/session switching failed")
    require(history["namespace"] == "maccms" and history["video"] == second["video"] and history["position"] >= 6800 and history["poster"] and row["saves"] > 0, "MacCMS durable history/poster missing")
    require(resumed["video"] == second["video"] and resumed["session"] != second["session"] and abs(resumed["position"] - history["position"]) < 1500, "MacCMS new Session did not restore episode/position")
    require(row["metadata_kept"] and len(row["matches"]) >= 3 and all(match["namespace"] == "maccms" and match["same_source"] for match in row["matches"])
            and not row["errors"], "same-source binding or TMDB selection changed")
    return dict(saves=row["saves"], episodes=[first["video"], second["video"]], resumed=resumed["position"])


def windows(run):
    previous = t01.DATA, t01.BUILD, t12.LIVE_TEST
    t01.DATA = run.folder / "packaged"; t01.DATA.mkdir(); t01.BUILD = t01.DATA / "build.json"; t12.LIVE_TEST = LIVE_TEST
    session = bridge = None
    try:
        require(t01.native_gui_session()["screen_locked"] is not True, "Unlock macOS for required real-window acceptance")
        t01.check_media(); t01.build(run); receipt = t01.check_build(run.source)
        executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/moc-video"
        run.command(["clang", "-fobjc-arc", "-framework", "Cocoa", "-framework", "ApplicationServices", "tests/fixtures/tv1_native.m", "-o", str(run.folder / "tv1-native")])
        with tempfile.TemporaryDirectory(prefix="tv4-live-", dir=run.folder) as temporary:
            folder = Path(temporary); media = folder / "media"; media.mkdir(); t12.owned_media(run, media, seconds=20)
            bridge = t14.Bridge(run, folder, media); bridge.wait(folder / "ready.json", timeout=180)
            with t01.Fixture(t01.MEDIA, invoke=bridge.invoke) as fixture:
                session = t01.NativeSession(run, fixture, executable, 1); session.wait(0, 30); session.command("identity")
                regression = session.command("tv4", action="filter-pagination", timeout=180); run.observed["filter_pagination"] = regression
                case(run, "TV-MACCMS-CAT", "client-filter", "a failed new-filter first page closes old pagination; explicit retry starts page one and replaces old results", lambda: require(
                    regression["before"] == dict(count=5, more=True) and regression["failed"]["message"]
                    and not regression["failed"]["more_enabled"] and regression["continued"]["calls"] == []
                    and regression["continued"]["alert"] and regression["retried"]["count"] == 0 and not regression["retried"]["alert"]
                    and len(regression["retried"]["calls"]) == 1 and regression["retried"]["calls"][0]["page"] == 1
                    and regression["retried"]["calls"][0]["filter"] == dict(year="1900") and not regression["errors"],
                    "new-filter first-page failure retained old pagination or skipped its first-page retry: " + json.dumps(regression, ensure_ascii=False)))
                layouts, screenshots = [], []
                for width in [960, 1200, 1512]:
                    session.command("layout", value=width); row = session.command("tv4", action="styles", timeout=90); layouts.append(row)
                    screenshots.append(tv1.capture(run, session.process.pid, width, row["native"]["scale_factor"]))
                run.observed.update(layouts=layouts, screenshots=screenshots)
                case(run, "TV-MACCMS-CAT", "visual-spec", "exact ADR-057 computed geometry/type/glass/state and 960/1200/1512 screenshots", lambda: [validate_styles(row) for row in layouts])
                tv1.native(run, "hover", session.process.pid, layouts[-1]["hover"]["x"], layouts[-1]["hover"]["y"])
                hovered = session.command("tv4", action="hover"); run.observed["hover"] = hovered
                case(run, "TV-MACCMS-CAT", "visual-spec", "real subcategory hover surface matches filter capsules", lambda: require(hovered["background-color"] == "rgba(255, 255, 255, 0.16)", "subcategory hover style missing"))
                for category, ratio in [("short", 1.5), ("sports", 9 / 16)]:
                    row = session.command("tv4", action=category); run.observed[category] = row
                    case(run, "TV-MACCMS-CAT", "visual-spec", "short drama portrait and sports landscape cards", lambda row=row, ratio=ratio: tv1.near(row["poster"]["rect"]["height"] / row["poster"]["rect"]["width"], ratio, "category poster ratio", .01))
                filtered = session.command("tv4", action="filters", timeout=180); run.observed["filters"] = filtered
                case(run, "TV-MACCMS-CAT", "client-filter", "real filter controls combine year/region/language, recover empty results and change subcategory", lambda: require(
                    filtered["filtered"]["count"] == 5 and len(filtered["filtered"]["calls"]) > 0 and "No titles found" in filtered["empty"]["text"]
                    and filtered["pagination"]["count"] == filtered["pagination"]["unique"] == 8 and filtered["pagination"]["progress"] == "Scanned 16 · Found 8"
                    and filtered["empty"]["heading"]["font-size"] == "17px" and filtered["cards"] > 0 and filtered["selected"] == "港台综艺" and not filtered["errors"], "client filters/subcategory/empty state differ"))
                switched = session.command("tv4", action="switch", timeout=90); run.observed["switch"] = switched
                case(run, "TV-MACCMS-CAT", "hide-missing", "current source missing short/sports hides entries and invalid old category references are never sent", lambda: require(
                    "#/browse/variety" in switched["links"] and not {"#/browse/short", "#/browse/sports"} & set(switched["links"])
                    and switched["disabled"] == 0 and switched["selected"] == "华语综艺" and switched["calls"] and not switched["error"] and not switched["errors"], "source switch retained an unavailable category"))
                retry = session.command("tv4", action="retry", timeout=90); run.observed["retry"] = retry
                case(run, "TV-MACCMS-CAT", "client-filter", "actual 503 remains a visible error with local retry and recovers without an empty-state substitution", lambda: require(
                    retry["failed"]["message"] and not retry["failed"]["empty"] and retry["recovered"] == 5 and not retry["errors"], "real source failure/retry state differs"))
                detail = session.command("tv4", action="details-styles", timeout=90); run.observed["details"] = detail
                case(run, "TV-MACCMS-CAT", "visual-spec", "poster detail backdrop/masks/vertical cover and source label exact styles", lambda: validate_details(detail))
                playback = session.command("tv4", action="play", timeout=180); run.observed["playback"] = playback
                for child in ["same-source-play", "history-resume"]:
                    case(run, "TV-MACCMS-PLAY", child, "real MacCMS details -> official playback -> episode switch -> SQLite history -> poster and new-Session resume", lambda: validate_play(playback))
                require(session.command("tv4", action="errors") == [], "actual window raised uncaught errors")
                session.command("identity"); session.close()
                require(session.record.get("exit") == 0 and session.record.get("lingering") == [] and session.record.get("peak_combined_rss_bytes", 0) > 0, "owned App/WebKit process cleanup/RSS evidence missing")
                require(fixture.active == 0, "owned HTTP fixture requests remain")
            bridge.finish()
        return dict(window="actual packaged WKWebView", screenshots=screenshots)
    finally:
        try:
            if session is not None and not session.closed: session.close(graceful=False)
        finally:
            if bridge is not None: bridge.close()
            t01.DATA, t01.BUILD, t12.LIVE_TEST = previous


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("--suite", choices=["all", "native", "window"], required=True)
    parser.add_argument("--workflow-card", choices=["T0.31", "T0.32"], default="T0.31", help="resource receipt owner for the authorized T0.32 diagnostic rerun")
    parser.add_argument("--report", type=Path, required=True); parser.add_argument("--junit", type=Path, required=True); args = parser.parse_args()
    Acceptance.CARD = args.workflow_card
    run = Acceptance(args.report, args.junit)
    if args.suite != 'all': run.REQUIREMENTS = ['REQ-TV4']
    if run.case("resource-ownership", ["REQ-TV4"], "original runner, actual macOS arm64 and approved RSS budget", lambda: resources(run, card=run.CARD)):
        if args.suite in ['all', 'native']:
            native_ids = ["REQ-TV4", "TV-MACCMS-CAT", "TV-MACCMS-PLAY", "TV-MACCMS-CAT/normalize-names", "TV-MACCMS-CAT/excluded-classes", "TV-MACCMS-CAT/client-filter",
                          "TV-MACCMS-PLAY/same-source-play", "TV-MACCMS-PLAY/history-resume", "TV-MACCMS-PLAY/stremio-binding-kept"]
            run.case("native-behaviors", native_ids, "four real Host tests with eight source samples, bounded filters and strict playback/history bindings", lambda: native(run))
            run.case("frontend-bindings", ["TV-MACCMS-PLAY", "TV-MACCMS-PLAY/stremio-binding-kept", "REQ-TV4"], "six strict source/episode/history/image regressions", lambda: frontend(run))
        if args.suite in ['all', 'window']:
            run.case("packaged-pages", ["TV-MACCMS-CAT", "TV-MACCMS-PLAY", "REQ-TV4"], "actual source-bound pages/styles/playback/history and complete owned cleanup", lambda: windows(run))
    return run.save()


if __name__ == '__main__':
    awake = subprocess.Popen(['/usr/bin/caffeinate', '-di', '-w', str(os.getpid())]) if sys.platform == 'darwin' else None
    try:
        raise SystemExit(main())
    finally:
        if awake is not None: awake.terminate(); awake.wait(timeout=5)
