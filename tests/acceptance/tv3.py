"""TV3: production Host and actual packaged App/Home/Browse behavior."""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import t01
import t12
import t14
import tv1
from t00 import Acceptance as CommandRunner, require, write_json
from t10 import resources

ROOT = Path(__file__).resolve().parents[2]
LIVE_TEST = "host::maccms::tests::t08::t09::tv3::tv3_packaged_host_bridge"
CHILDREN = {
    "TV-HOME": ["hero-rotation-pause", "continue-watching", "category-rows", "lazy-rows", "visual-spec",
                "genre-year-rows", "media-row-spec", "catalog-pending", "filtered-row-isolation"],
    "TV-BROWSE": ["distinct-categories", "filter-bar-combination", "poster-wall-pagination", "see-all", "restore-state", "empty-result"],
}
IDS = [*CHILDREN, *[f"{parent}/{child}" for parent, children in CHILDREN.items() for child in children], "REQ-TV3"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.30", "TV3", IDS
    DEPLOYMENT = "actual packaged Tauri WKWebView with production Host; controlled TMDB-compatible fixture and live built-in TMDB"
    DEFERRED = [dict(target=target, status="NOT RUN", reason="ADR-005/006; macOS arm64 local acceptance") for target in t01.DEFERRED]
    command = CommandRunner.command


def case(run, parent, child, expected, function):
    return run.case(child, [f"{parent}/{child}", parent, "REQ-TV3"], expected, function)


def native(run):
    output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "tv3_",
                          "--", "--test-threads=1", "--nocapture", "--skip", "tv3_packaged_host_bridge"], timeout=600)
    names = re.findall(r"^test (?:\w+::)*(tv3_\w+) \.\.\.", output, re.M)
    expected = {"tv3_catalog_descriptions_zero_content_and_old_compatibility", "tv3_catalog_null_equivalence_and_invalid_combinations",
                "tv3_same_reference_filters_concurrent_and_pagination", "tv3_rejects_catalog_only_for_non_stremio_sources"}
    require(set(names) == expected and "test result: ok. 4 passed; 0 failed; 0 ignored;" in output, "all four named real Host tests must pass")
    observations = [json.loads(line.split("N01_OBSERVATION:", 1)[1]) for line in output.splitlines() if line.startswith("N01_OBSERVATION:")]
    write_json(run.folder / "host-observations.json", observations)
    return dict(tests=4, names=names, observation_log=str((run.folder / "host-observations.json").relative_to(ROOT)))


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-reporter=tap", "--test-concurrency=1", "tests/tv3.test.ts"])
    require(re.search(r"^# tests 5$", output, re.M) and re.search(r"^# pass 5$", output, re.M)
            and re.search(r"^# fail 0$", output, re.M) and re.search(r"^# skipped 0$", output, re.M), "five frontend race/state regressions must run and pass")
    return dict(tests=5, passed=5, failed=0, skipped=0)


class Bridge(t14.Bridge):
    def finish(self):
        self.invoke(dict(operation="finish"))
        self.process.wait(timeout=30)
        self.reader.join(timeout=5)
        self.record["exit"] = self.process.returncode
        require(self.process.returncode == 0 and not self.reader.is_alive(), "TV3 real Host did not finish cleanly")
        vectors = [json.loads(line.split("TV3_VECTOR:", 1)[1]) for line in self.lines if "TV3_VECTOR:" in line]
        require(len(vectors) == 1 and vectors[0]["get_home"] > 0 and vectors[0]["active_requests"] == 0,
                "TV3 bridge must have real home requests and no active requests on exit")
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in "".join(self.lines), "real Host bridge test did not pass")
        require(not self.run.observed.get("host_log_sanitized"), "native output contained unsanitized capabilities")
        self.run.observed["host"] = vectors[0]


def validate_pending(row):
    require(row["phase"] == "ready" and row["descriptions"] == 10 and len(row["rows"]) == 13, "catalog descriptions must produce viewport candidates")
    require(all(item["phase"] in ["pending", "loading"] and not item["empty"] for item in row["rows"]), "description empty-items must stay pending")
    require(row["content_requests"] == 0 and row["description_calls"] == 1, "descriptions must not issue actual catalog requests")
    require(0 < len(row["visible_requests"]) < len(row["rows"]), "only visible row candidates may begin content loading")
    return row


def validate_home(row):
    require(len(row["initial"]) == 13 and row["hero"]["slides"] == 5 and not row["errors"], "home structure/hero or runtime errors differ")
    expected = [{"sort": "trending_week"}, {"sort": "trending_week"}, {"sort": "latest"}, {"sort": "popular"},
                {"genre": "Action"}, {"genre": "Comedy"}, {"genre": "Science Fiction"}, {"genre": "Drama"}]
    require([item["filter"] for item in row["initial"][:8]] == expected, "home row order and exact genre/sort conditions differ")
    years = [item["filter"].get("year") for item in row["initial"][8:10]]
    year = time.gmtime().tm_year
    require(years == [str(year), str(year - 1)], "current and previous year rows missing")
    require([item["filter"].get("language") for item in row["initial"][-3:]] == ["Mandarin", "Korean", "Japanese"], "language rows missing")
    require(any(item["phase"] == "pending" for item in row["initial"]) and row["after"] > row["before"] and row["last_cards"] > 0,
            "offscreen rows must request only when scrolled into view")
    h = row["history"]
    require(h["count"] == 1 and h["calls"] > 0 and h["image"] and h["title"] == "Continue fixture"
            and "3" in h["episode"] and "12" in h["remaining"] and abs(h["ratio"] - .2) < .001,
            "real list_history/historyPoster, episode and remaining-time progress differ")
    return row


def validate_browse(rows):
    require([row["category"] for row in rows] == ["movie", "series", "anime", "documentary"], "all category entries must be used")
    require(rows[0]["kind"] == "movie" and rows[1]["kind"] == "series", "movie/series kinds differ")
    for row, genre in zip(rows[2:], ["Animation", "Documentary"]):
        require(row["filters"]["genre"] == genre and row["genres"] and all(genre in genres for genres in row["genres"]), "category must return its own genre")
    require(len({tuple(row["ids"]) for row in rows}) == 4, "categories rendered identical content")
    for row in rows:
        require([item["filter"].get("sort") for item in row["rows"][:2]] == ["trending_week", "latest"], "category trend/latest rows missing")
        require(all(row["rows"][i]["filter"].get("year") for i in [2, 3]) and len(row["rows"]) >= 5, "category year/genre rows missing")
    return rows


def validate_filters(row):
    require("sort=" not in row["cleared"]["hash"] and row["cleared"]["notice"] and row["cleared"]["sort_disabled"], "language must clear selected sort and explain it")
    selected = row["filter"]
    require(selected == dict(genre="Drama", year=str(time.gmtime().tm_year - 1), language="Mandarin"), "combined filter did not reach Host")
    require(row["items"] and all(item["year"] == int(selected["year"]) and "Drama" in item["genres"] and "Mandarin" in item["title"] for item in row["items"]), "Host results violate combined filter")
    require(row["region"]["disabled"] and row["region"]["reason"] and "Unavailable" in row["region"]["text"], "disabled region must have visible source explanation")
    require(row["keyboard"] == dict(before=0, after=1), "panel directional focus did not move")
    tv1.near(row["compact"]["rect"]["height"], 240, "Filtered hero height")
    require(row["hidden_plot"] and row["hidden_actions"] and "0.3s" in row["compact"]["transition-duration"], "compact hero state missing")
    tv1.near(row["compact_parallax"]["opacity"], .25, "Filtered hero scroll fade", .01)
    for key in ["genre", "year", "language"]: require(f"{key}=" in row["hash"], "selected condition missing from address")
    validate_filter_styles(row)
    return row


def validate_filter_styles(row):
    p, o, pill, w = (row[key] for key in ["panel", "option", "pill", "wall"])
    require(p["border-radius"] == "16px" and p["padding-left"] == "12px" and p["max-height"] == "360px"
            and "blur(30px)" in p["-webkit-backdrop-filter"] and p["animation-duration"] == "0.2s", "filter panel glass/animation differs")
    require(360 <= p["rect"]["width"] <= 480 and o["height"] == "36px" and o["border-radius"] == "10px", "filter panel/option size differs")
    tv1.near(o["rect"]["height"], 37.44 if o["transform"].startswith("matrix(1.04,") else 36, "filter option focus geometry")
    require(pill["rect"]["height"] == 36 and pill["padding-left"] == pill["padding-right"] == "14px"
            and pill["font-size"] == "14px" and pill["font-weight"] == "500" and pill["background-color"] == "rgb(245, 245, 247)"
            and pill["color"] == "rgb(11, 11, 13)", "selected capsule style differs")
    require(w["row-gap"] == "28px" and w["column-gap"] == "16px" and row["progress"]["font-size"] == "13px", "poster wall spacing/progress differs")


def validate_isolation(row):
    require(row["pending"] == "loading" and row["fast"] and row["slow"] and all("Comedy" in genres for genres in row["fast"])
            and all("Drama" in genres for genres in row["slow"]), "out-of-order filtered row results crossed")
    require(row["pagination"] == dict(comedy=40, drama=20) and all(item["cards"] == row["pagination"][item["genre"].lower()] for item in row["displayed"]), "row pagination/display must remain independent")
    for key in ["switched", "language"]:
        require(row[key]["old_key"] != row[key]["new_key"] and row[key]["old_result"] is None, "old source/locale response regained authority")
    require("Second" in row["switched"]["title"] and not row["errors"], "source switch failed or produced runtime errors")
    return row


def validate_styles(row):
    require(row["width"] in [960, 1200, 1512] and not row["overflow"] and not row["errors"], "window dimensions/overflow/runtime errors differ")
    s, width, height = row["styles"], row["width"], row["height"]
    hero = s[".catalog-hero"]
    tv1.near(hero["rect"]["height"], max(460, min(.72 * height, 760)), "Hero height")
    tv1.near(hero["rect"]["x"], 0, "Hero spans window left")
    tv1.near(hero["rect"]["width"], width, "Hero spans window width")
    require(hero["min-height"] == "460px" and s[".catalog-hero-slide"]["transition-duration"] == "0.6s", "hero minimum/fade differs")
    image = s[".catalog-hero-slide.current img"]
    require(image["object-fit"] == "cover" and image["object-position"] == "50% 20%" and image["animation-duration"] == "8s", "hero artwork/push duration differs")
    tv1.near(s[".hero-bottom-mask"]["rect"]["height"], hero["rect"]["height"] * .55, "Hero vertical mask")
    tv1.near(s[".hero-left-mask"]["rect"]["width"], width * .6, "Hero horizontal mask")
    require(s[".catalog-hero-copy"]["bottom"] == "56px" and s[".catalog-hero-copy"]["max-width"] == "560px", "Hero copy position differs")
    for selector, font, weight in [(".hero-eyebrow", "13px", "600"), (".hero-information", "14px", "400"), (".hero-plot", "15px", "400")]:
        require(s[selector]["font-size"] == font and s[selector]["font-weight"] == weight, f"Hero typography differs: {selector}")
    require(s[".hero-plot"]["-webkit-line-clamp"] == "3" and s[".hero-actions"]["margin-top"] == "20px", "Hero plot/buttons spacing differs")
    require(s[".hero-primary"]["rect"]["height"] == s[".hero-secondary"]["rect"]["height"] == 44
            and s[".hero-primary"]["rect"]["width"] >= 120 and s[".hero-primary"]["background-color"] == "rgb(245, 245, 247)", "Hero buttons differ")
    require(s[".hero-indicators"]["bottom"] == s[".hero-indicators"]["right"] == "48px", "indicator placement differs")
    tv1.near(s[".hero-indicators button:not(.active)"]["rect"]["width"], 6, "inactive dot")
    tv1.near(s[".hero-indicators button.active"]["rect"]["width"], 20, "active capsule")
    tv1.near(s[".hero-indicators button.active"]["rect"]["height"], 6, "dot height")
    require(s[".home-content-rows"]["margin-top"] == "-32px", "first row overlap differs")
    require(s[".continue-progress"]["height"] == "3px" and s[".continue-progress"]["background-color"] == "rgba(255, 255, 255, 0.3)"
            and s[".continue-play"]["width"] == s[".continue-play"]["height"] == "56px", "continue-watching progress/play geometry differs")
    scale = 1.04 if s[".continue-poster"]["transform"].startswith("matrix(1.04,") else 1
    tv1.near(s[".continue-progress"]["rect"]["height"], 3 * scale, "continue progress interaction geometry")
    tv1.near(s[".continue-play"]["rect"]["width"], 56 * scale, "continue play interaction geometry")
    require(s[".continue-poster"]["border-radius"] == "12px", "continue-watching card inherited a pill radius")
    tv1.near(s[".continue-poster"]["rect"]["height"] / s[".continue-poster"]["rect"]["width"], 9 / 16, "continue-watching landscape ratio", .01)
    require("rgb(11, 11, 13)" in s[".hero-bottom-mask"]["background-image"]
            and "rgba(11, 11, 13, 0.75)" in s[".hero-left-mask"]["background-image"], "Hero mask colors differ")
    tv1.near(row["parallax"]["opacity"], .25, "Hero scroll fade", .01)
    transform = re.search(r"matrix\([^,]+,[^,]+,[^,]+,[^,]+,[^,]+,\s*([^\)]+)", row["parallax"]["transform"])
    require(transform, "Hero scroll transform missing")
    tv1.near(transform[1], hero["rect"]["height"] * .6 * .4, "Hero parallax speed", 1)
    card, poster, track = [s[f".catalog-lazy-row {selector}"] for selector in [".media-card", ".media-poster", ".media-row-track"]]
    tv1.near(card["rect"]["width"], max(132, min(width * .125, 220)), "Real home MediaRow poster width")
    tv1.near(poster["rect"]["height"] / poster["rect"]["width"], 1.5, "Real home poster ratio", .01)
    require(poster["border-radius"] == "12px" and track["column-gap"] == ("20px" if width >= 1440 else "16px")
            and track["padding-top"] == track["padding-bottom"] == "24px" and track["scroll-snap-type"] == "x mandatory"
            and track["scrollbar-width"] == "none", "Real home MediaRow whitespace/snap differs")
    arrow = s[".catalog-lazy-row .media-row-next"]
    tv1.near(arrow["rect"]["width"], 44, "Real row pager width")
    tv1.near(arrow["rect"]["height"], 44, "Real row pager height")
    tv1.near(arrow["rect"]["y"] + 22, poster["rect"]["y"] + poster["rect"]["height"] / 2, "Real row pager center")
    return dict(width=width, hero_height=hero["rect"]["height"], poster_width=card["rect"]["width"])


def hero_behavior(run, session):
    pid = session.process.pid
    tv1.native(run, "hover", pid, 20, 20)
    probe = session.command("tv3", action="hero-rotation")
    run.observed["hero_rotation_probe"] = probe
    first, rotated = probe["first"], probe["rotated"]
    require(probe["elapsed"] >= 8000, "Hero clock observation shorter than eight seconds")
    require(not first["paused"] and not rotated["paused"] and rotated["index"] != first["index"], "Hero did not rotate after eight seconds")
    dot = session.command("tv3", action="hero-dot")
    require(dot["index"] == 4, "Hero indicator click did not select fifth item")
    focused = session.command("tv3", action="hero-focus")
    time.sleep(8.3)
    focus_after = session.command("tv3", action="hero")
    require(focused["paused"] and focus_after["index"] == focused["index"] and focus_after["animation"] == "paused", "Hero focus pause failed")
    session.command("tv3", action="hero-blur")
    tv1.native(run, "hover", pid, 800, 180)
    hover = session.command("tv3", action="hero")
    time.sleep(8.3)
    hover_after = session.command("tv3", action="hero")
    require(hover["hover"] and hover["paused"] and hover_after["index"] == hover["index"], "Real mouse hover did not pause Hero")
    tv1.native(run, "hover", pid, 20, 20)
    session.command("tv3", action="hero-blur")
    tv1.native(run, "minimize", pid)
    time.sleep(8.5)
    session.command("tv3", action="hero")
    hidden = session.command("tv3", action="visibility")
    pair = next(((a, b) for a, b in zip(hidden, hidden[1:]) if a["hidden"] and not b["hidden"] and b["time"] - a["time"] >= 8000), None)
    require(pair and pair[0]["index"] == pair[1]["index"], "Real hidden window did not pause Hero for eight seconds")
    return dict(first=first, rotated=rotated, dot=dot, focused=focused, hover=hover, hidden=hidden)


def live_builtin(run, session):
    live = session.command("tv3", action="builtin", timeout=300); run.observed["builtin"] = live
    preset = next(source for source in json.loads((ROOT / 'src/data/defaultSources.json').read_text()) if source['kind'] == 'stremio')
    run.case("live-builtin-tmdb", ["TV-HOME", "TV-BROWSE", "REQ-TV3"], "actual shipped TMDB manifest and viewport content render in packaged window", lambda: require(
        live["origin"]["manifest_urls"] == preset["manifest_urls"] and live["sections"] > 0 and live["cards"] > 0 and live["hero"]["slides"] == 5
        and any(c["input"].get("catalog_only") for c in live["calls"]) and any(c["titles"] for c in live["calls"]) and not live["errors"], "live built-in TMDB missing"))
    return live


def windows(run, *, hero_only=False, browse_only=False, builtin_only=False, review_only=False):
    previous = t01.DATA, t01.BUILD, t12.LIVE_TEST
    t01.DATA = run.folder / "packaged"; t01.DATA.mkdir()
    t01.BUILD = t01.DATA / "build.json"; t12.LIVE_TEST = LIVE_TEST
    session = bridge = None
    try:
        require(t01.native_gui_session()["screen_locked"] is not True, "Unlock the macOS desktop")
        t01.check_media(); t01.build(run)
        receipt = t01.check_build(run.source)
        executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/moc-video"
        run.command(["clang", "-fobjc-arc", "-framework", "Cocoa", "-framework", "ApplicationServices", "tests/fixtures/tv1_native.m", "-o", str(run.folder / "tv1-native")])
        with tempfile.TemporaryDirectory(prefix="tv3-live-", dir=run.folder) as temporary:
            folder = Path(temporary); media = folder / "media"; media.mkdir()
            bridge = Bridge(run, folder, media); bridge.wait(folder / "ready.json", timeout=180)
            with t01.Fixture(t01.MEDIA, invoke=bridge.invoke) as fixture:
                session = t01.NativeSession(run, fixture, executable, 1); session.wait(0, 30); session.command("identity")
                session.command("layout", value=1200)
                pending = session.command("tv3", action="pending", timeout=90)
                run.observed["pending"] = pending
                case(run, "TV-HOME", "catalog-pending", "real descriptions stay pending, zero content requests, visible rows start separately", lambda: validate_pending(pending))
                home = session.command("tv3", action="home"); run.observed["home"] = home
                for name in ["category-rows", "lazy-rows", "genre-year-rows", "continue-watching"]:
                    case(run, "TV-HOME", name, "real homepage ordered rows, lazy entry, year/genre routing and genuine history", lambda: validate_home(home))
                if review_only:
                    pagination = session.command("tv3", action="review-pagination")
                    run.observed["review_pagination"] = pagination
                    run.case("review-pagination-posters", ["REQ-TV3"], "existing decoded posters remain visible after images map is extended", lambda: require(
                        pagination["after"] > pagination["before"] > 0 and pagination["reused"] and pagination["same_url"]
                        and pagination["decoded"] and pagination["opacity"] == "1" and not pagination["errors"], "pagination hid an existing poster"))
                    session.command("tv3", action="review-breakpoint-leave")
                    session.command("layout", value=768)
                    narrow = session.command("tv3", action="review-breakpoint-return", timeout=90)
                    run.observed["review_narrow_return"] = narrow
                    run.case("review-narrow-return", ["REQ-TV3"], "desktop home -> details -> resize below 960 -> back loads actual narrow content", lambda: require(
                        narrow["width"] == 768 and not narrow["catalog"] and narrow["phase"] == "ready" and narrow["cards"] > 0
                        and any(not request.get("catalog_only") and not request.get("section_ref") for request in narrow["requests"])
                        and not narrow["errors"], "narrow return retained desktop descriptions"))
                    if narrow["cards"]:
                        session.command("tv3", action="review-breakpoint-leave")
                        session.command("layout", value=1200)
                        desktop = session.command("tv3", action="review-breakpoint-return", timeout=90)
                        run.observed["review_desktop_return"] = desktop
                        run.case("review-desktop-return", ["REQ-TV3"], "narrow home -> details -> resize above 960 -> back restores lazy catalog", lambda: require(
                            desktop["width"] == 1200 and desktop["catalog"] and desktop["phase"] == "ready" and desktop["cards"] > 0
                            and any(request.get("catalog_only") for request in desktop["requests"]) and not desktop["errors"], "desktop return retained narrow snapshot"))
                    session.command("identity"); session.close(); bridge.finish()
                    return dict(diagnostic="T0.30 review regressions", host_tests=1)
                if builtin_only:
                    live_builtin(run, session); session.command("identity"); session.close(); bridge.finish()
                    return dict(diagnostic="isolated live built-in TMDB", host_tests=1)
                if not browse_only:
                    case(run, "TV-HOME", "hero-rotation-pause", "five weekly trends; eight-second rotation; real hover/focus/hidden pauses; dot switching", lambda: hero_behavior(run, session))
                if hero_only:
                    session.command("identity"); session.close(); bridge.finish()
                    return dict(diagnostic="isolated Hero and window lifecycle", host_tests=1)
                layouts, screenshots = [], []
                for width in [960, 1200, 1512]:
                    session.command("layout", value=width)
                    row = session.command("tv3", action="styles"); layouts.append(row)
                    screenshots.append(tv1.capture(run, session.process.pid, width, row["native"]["scale_factor"]))
                run.observed.update(layouts=layouts, screenshots=screenshots)
                for name in ["visual-spec", "media-row-spec"]:
                    case(run, "TV-HOME", name, "exact computed styles and real homepage MediaRow geometry at 960/1200/1512", lambda: [validate_styles(row) for row in layouts])
                browse = session.command("tv3", action="browse", timeout=180); run.observed["browse"] = browse
                case(run, "TV-BROWSE", "distinct-categories", "four actual sidebar routes show their own kinds/genres plus trend/latest/year/genre rows", lambda: validate_browse(browse))
                filters = session.command("tv3", action="filters", timeout=180); run.observed["filters"] = filters
                case(run, "TV-BROWSE", "filter-bar-combination", "genre/year/language combined, sort cleared with notice, unavailable region, keyboard and computed styles", lambda: validate_filters(filters))
                filter_styles = session.command("tv3", action="filter-styles"); run.observed["filter_styles"] = filter_styles
                tv1.native(run, "hover", session.process.pid, filter_styles["target"]["x"], filter_styles["target"]["y"])
                hovered = session.command("tv3", action="filter-hover")
                def filter_states():
                    require(filter_styles["normal"]["background-color"] == "rgba(255, 255, 255, 0.1)" and hovered["background-color"] == "rgba(255, 255, 255, 0.16)", "filter default/real-hover surface differs")
                    require(filter_styles["sticky"]["position"] == "sticky" and filter_styles["sticky"]["rect"]["top"] == 12
                            and "blur(30px)" in filter_styles["sticky"]["-webkit-backdrop-filter"] and filter_styles["sticky_hit"], "filter bar sticky glass/input differs")
                    for option in filter_styles["options"]:
                        require(option["height"] == "36px" and option["radius"] == "10px", "filter options need 36px height and 10px radius")
                        # Shared TV1 button focus scales the 36px box by 1.04.
                        expected = 37.44 if option["transform"].startswith("matrix(1.04,") else 36
                        tv1.near(option["rendered_height"], expected, "filter option rendered focus scale")
                    tv1.near(filter_styles["header"]["rect"]["height"], min(.56 * layouts[-1]["height"], 560), "Category header height")
                    return dict(default=filter_styles, hover=hovered)
                case(run, "TV-HOME", "visual-spec", "filter default/hover/sticky states and category header retain exact specification", filter_states)
                pagination = session.command("tv3", action="pagination"); run.observed["pagination"] = pagination
                case(run, "TV-BROWSE", "poster-wall-pagination", "scroll sentinel appends distinct cards using the same filter", lambda: require(pagination["after"] > pagination["before"] > 0 and pagination["unique"] == pagination["after"] and 2 in pagination["requests"], "wall scroll pagination failed"))
                restored = session.command("tv3", action="restore"); run.observed["restore"] = restored
                case(run, "TV-BROWSE", "restore-state", "real detail and browser Back preserve query/results/scroll without reloading", lambda: require(restored["hash"] == restored["restored_hash"] and abs(restored["scroll"] - restored["restored_scroll"]) < 8 and restored["scroll"] > 0 and restored["ids"] == restored["restored_ids"] and restored["requests"] == 0, "browse return state lost"))
                see = session.command("tv3", action="see-all"); run.observed["see_all"] = see
                case(run, "TV-BROWSE", "see-all", "content-row heading enters its filtered poster wall", lambda: require(see["wanted"] == see["hash"] and "section=" in see["hash"] and see["count"] > 0, "see-all route lost row conditions"))
                empty = session.command("tv3", action="empty"); run.observed["empty"] = empty
                def empty_check():
                    require(empty["shortcuts"] == 3 and empty["recovered"] > 0 and "genre=" not in empty["hash"], "empty shortcuts did not remove condition and recover")
                    require(empty["icon"]["rect"]["width"] == 40 and empty["heading"]["font-size"] == "17px" and empty["heading"]["font-weight"] == "600" and empty["description"]["font-size"] == "14px", "empty-state style differs")
                    return empty
                case(run, "TV-BROWSE", "empty-result", "actual zero-match scan offers per-condition removal and recovers", empty_check)
                isolation = session.command("tv3", action="isolation", timeout=180); run.observed["isolation"] = isolation
                case(run, "TV-HOME", "filtered-row-isolation", "same ref concurrent/reordered filters, separate pagination/display, source and language stale responses", lambda: validate_isolation(isolation))
                retry = session.command("tv3", action="retry", timeout=90); run.observed["retry"] = retry
                case(run, "TV-HOME", "catalog-pending", "real upstream failure stays failed rather than empty and retries in place after source revision change", lambda: require(retry["failed"]["phase"] == "failed" and not retry["failed"]["empty"] and retry["failed"]["message"] and retry["phase"] == "ready" and retry["cards"] > 0 and not retry["error"], "pending/failure/retry state differs"))
                live_builtin(run, session)
                require(session.command("tv3", action="errors") == [], "Window raised uncaught errors")
                session.command("identity"); session.close()
                require(session.record.get("exit") == 0 and session.record.get("lingering") == [] and session.record.get("peak_combined_rss_bytes", 0) > 0, "packaged process/WebKit must exit with owned RSS evidence")
                require(fixture.active == 0, "fixture requests remain after closing")
            bridge.finish()
        return dict(window="actual packaged WKWebView", screenshots=screenshots, host_tests=1)
    finally:
        try:
            if session is not None and not session.closed: session.close(graceful=False)
        finally:
            if bridge is not None: bridge.close()
            t01.DATA, t01.BUILD, t12.LIVE_TEST = previous


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all", "native", "window", "hero", "browse", "builtin"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    if args.suite != "all": run.REQUIREMENTS = ["REQ-TV3"]
    if run.case("resource-ownership", ["REQ-TV3"], "approved original resource runner and macOS arm64", lambda: resources(run, card="T0.30")):
        if args.suite in ["all", "native"]:
            case(run, "TV-HOME", "catalog-pending", "real Host zero-content descriptions, legacy compatibility, null equivalence, invalid combinations and non-Stremio rejection", lambda: native(run))
            case(run, "TV-HOME", "filtered-row-isolation", "all five controller race/pagination/retry/navigation regressions", lambda: frontend(run))
        if args.suite in ["all", "window", "hero", "browse", "builtin"]:
            run.case("packaged-pages", ["TV-HOME", "TV-BROWSE", "REQ-TV3"], "real Host and actual production UI behavior, styles, screenshots and clean shutdown", lambda: windows(run, hero_only=args.suite == 'hero', browse_only=args.suite == 'browse', builtin_only=args.suite == 'builtin'))
    return run.save()


if __name__ == "__main__":
    awake = subprocess.Popen(["/usr/bin/caffeinate", "-di", "-w", str(os.getpid())]) if sys.platform == "darwin" else None
    try:
        code = main()
    finally:
        if awake is not None: awake.terminate(); awake.wait(timeout=10)
    raise SystemExit(code)
