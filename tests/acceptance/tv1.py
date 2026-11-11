"""TV1 production shell acceptance in the actual packaged Tauri WebView."""
import argparse
import json
import math
import os
import re
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import t01
import t12
import t14
from t00 import require, write_json, file_hash
from t10 import resources

ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / "evidence/acceptance/TV1-narrow-baseline.json"
IDS = ["TV-SHELL", "TV-THEME", "TV-FOCUS", "REQ-TV1"]
BEHAVIORS = {
    "TV-SHELL": ["overlay-titlebar-drag", "floating-sidebar-collapse", "narrow-unchanged", "permissions-exact"],
    "TV-THEME": ["desktop-dark-palette", "visual-tokens"],
    "TV-FOCUS": ["keyboard-navigation", "hover-focus-parity", "reduced-motion"],
}
REQUIREMENTS = IDS + [f"{parent}/{name}" for parent, names in BEHAVIORS.items() for name in names]
BASELINE_SHA = "ef92bef63e61bc7f835c62d3255a3393b59ad15f3356073dc63b93275849ebfd"
TOKENS = {
    "--tv-bg": "#0b0b0d", "--tv-surface-1": "rgba(255,255,255,.06)", "--tv-surface-2": "rgba(255,255,255,.10)",
    "--tv-surface-hover": "rgba(255,255,255,.16)", "--tv-glass": "rgba(30,30,34,.62)",
    "--tv-glass-filter": "blur(30px) saturate(160%)", "--tv-hairline": "rgba(255,255,255,.10)",
    "--tv-text": "#f5f5f7", "--tv-text-2": "rgba(235,235,245,.62)", "--tv-text-3": "rgba(235,235,245,.38)",
    "--tv-button": "#f5f5f7", "--tv-button-ink": "#0b0b0d", "--tv-danger": "#ff453a",
    "--tv-ease": "cubic-bezier(.2,.8,.2,1)", "--tv-focus-duration": "200ms", "--tv-page-duration": "300ms",
    "--tv-hero-duration": "600ms", "--tv-focus-scale": "1.08",
}
PERMISSIONS = "core:event:allow-listen core:event:allow-unlisten allow-player-fullscreen allow-runtime-status allow-runtime-prepare allow-get-sources allow-sync-catalog allow-get-home allow-get-details allow-resolve-episode allow-open-playback allow-close-playback allow-cancel-request allow-start-interaction allow-finish-interaction allow-get-settings allow-save-settings allow-get-resource-status allow-release-resolution allow-match-playback allow-list-history allow-save-history allow-remove-history allow-clear-history allow-get-media-cache allow-clear-media-cache allow-app-close-started allow-app-close-aborted allow-app-close-ready allow-window-bounds-now".split()
CSP = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: http://127.0.0.1:*; media-src 'self' blob: http://127.0.0.1:*; connect-src 'self' ipc: http://ipc.localhost http://127.0.0.1:*; worker-src 'self' blob:; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'"


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.28", "TV1", REQUIREMENTS
    DEPLOYMENT = "actual packaged Tauri WKWebView, real Host fixture, macOS arm64; RSS sampled process-watchdog"
    DEFERRED = [dict(target=target, status="NOT RUN", reason="ADR-005/006; local macOS arm64 required") for target in t01.DEFERRED] + [dict(target="1920px", status="NOT RUN", reason="built-in display logical width 1512; ADR-057")]


def behavior(run, parent, name, expected, function):
    return run.case("observed", [f"{parent}/{name}", parent, "REQ-TV1"], expected, function)


def near(value, expected, message, tolerance=.1):
    require(abs(float(str(value).removesuffix("px")) - expected) <= tolerance, f"{message}: {value}, expected {expected}")


def security(root=ROOT):
    main = json.loads((root / "src-tauri/capabilities/main.json").read_text())
    expected = PERMISSIONS + ["core:window:allow-start-dragging"]
    require(len(main["permissions"]) == len(expected) and set(main["permissions"]) == set(expected), "Main permissions must be exactly baseline plus start-dragging")
    require(main["identifier"] == "main" and main["windows"] == ["main"] and set(main) == {"identifier", "description", "windows", "permissions"}, "Capability targets/keys changed")
    config = json.loads((root / "src-tauri/tauri.conf.json").read_text())
    require(config["app"]["security"] == dict(capabilities=["main"], csp=CSP), "CSP/security configuration differs from baseline")
    return dict(permissions=len(expected), csp="exact baseline match")


def validate_desktop(row):
    width = row["width"]
    require(width in [960, 1200, 1512] and not row["overflow"], "Actual desktop width/overflow differs")
    near(row["sidebar"]["x"], 12, "Sidebar left")
    near(row["sidebar"]["y"], 12, "Sidebar top")
    near(row["sidebar"]["height"], row["height"] - 24, "Sidebar bottom")
    near(row["sidebar"]["width"], 72 if width < 1200 else 220, "Default sidebar width")
    near(row["padding_left"], 116 if width < 1200 else 264, "Content left margin")
    near(row["padding_right"], 64 if width >= 1440 else 48, "Content right margin")
    near(row["drag"]["height"], 44, "Drag region height")
    require(row["topbar"] == row["legacy"] == "none" and row["hidden_source_labels"], "Desktop topbar/legacy/source selectors must move into sidebar")
    require(row["sidebar_background"] == "rgba(30, 30, 34, 0.62)" and row["sidebar_radius"] == "20px" and "1px" in row["sidebar_border"], "Sidebar glass geometry differs")
    require("blur(30px)" in row["webkit_glass"] and "saturate(1.6)" in row["webkit_glass"], "WebKit glass blur/saturation differs")
    require(row["sidebar_transition"] == "0.24s", "Sidebar transition differs")
    require(row["page_title"]["font"] == "34px" and row["page_title"]["weight"] == "700", "Page title typography differs")
    near(row["page_title"]["line"], 34 * 1.15, "Page title line height", .001)
    require(row["color_scheme"] == "dark" and row["background"] == "rgb(11, 11, 13)", "Desktop must remain dark")
    require(all(row["tokens"].get(key) == value for key, value in TOKENS.items()), "TV shared design variables differ")
    require([link["href"] for link in row["navigation"]] == ["#/home", "#/history", "#/sources", "#/settings"], "Current card navigation differs")
    for link in row["navigation"]:
        near(link["rect"]["height"], 36, "Navigation height")
        require(link["font"] == "14px" and link["weight"] == "500" and link["radius"] == "10px" and link["title"], "Navigation typography/radius/tooltip differs")
        near(link["icon"]["width"], 20, "Navigation icon size")
        require(link["stroke"] == "1.75px", "Icon stroke differs")
    require(row["navigation"][0]["current"] == "page" and row["navigation"][0]["background"] == "rgba(255, 255, 255, 0.14)", "Selected navigation state differs")
    if width >= 1200:
        require(len(row["sources"]) == 2 and all(item["height"] == 32 and item["font"] == "13px" and item["value"] for item in row["sources"]), "Sidebar source capsules differ")
    return dict(width=width, sidebar=row["sidebar"]["width"], tokens=len(TOKENS))


def validate_rows(styles, width):
    near(styles[".media-card"]["width"], max(132, min(width * .125, 220)), "Portrait width")
    near(styles[".media-poster"]["height"] / styles[".media-poster"]["width"], 1.5, "Portrait ratio", .01)
    near(styles[".media-row-landscape .media-card"]["width"], max(240, min(width * .22, 360)), "Landscape width")
    near(styles[".media-row-landscape .media-poster"]["height"] / styles[".media-row-landscape .media-poster"]["width"], 9/16, "Landscape ratio", .01)
    near(styles[".media-row-heading"]["height"], 28, "Heading height")
    near(styles[".media-row-track"]["gap"], 20 if width >= 1440 else 16, "Card gap")
    require(styles[".media-row-track"]["padding"].startswith("24px") and styles[".media-row-track"]["snap"] == "x mandatory" and styles[".media-row-track"]["scrollbar"] == "none", "Row padding/snap/scrollbar differs")
    for selector, font, weight in [(".media-row h2", "22px", "600"), (".media-card-title", "14px", "500"), (".media-card-detail", "13px", "400"), (".media-poster-missing", "15px", "600")]:
        require(styles[selector]["font"] == font and styles[selector]["weight"] == weight, f"Row typography differs: {selector}")
    require(styles[".media-poster"]["radius"] == "12px" and styles[".media-card-title"]["overflow"] == "ellipsis" and styles[".media-card-title"]["white_space"] == "nowrap", "Poster/title shape differs")
    require(styles[".media-loading .media-poster"]["animation"] == "1.2s" and styles[".media-loading .media-poster"]["name"] != "none" and .49 <= float(styles[".media-loading .media-poster"]["opacity"]) <= .81, "Loading opacity animation differs")
    near(styles[".media-row-next"]["width"], 44, "Pager width")
    near(styles[".media-row-next"]["height"], 44, "Pager height")
    require(styles[".media-row-next"]["radius"] == "999px" and "blur(30px)" in styles[".media-row-next"]["filter"], "Pager glass/radius differs")
    near(styles[".media-row-next"]["y"] + 22, styles[".media-poster"]["y"] + styles[".media-poster"]["height"] / 2, "Pager center aligns with poster", .1)
    return dict(width=width, portrait_width=styles[".media-card"]["width"], landscape_width=styles[".media-row-landscape .media-card"]["width"])


def native(run, action, pid, *args):
    output = run.command([str(run.folder / "tv1-native"), action, str(pid), *map(str, args)])
    return json.loads(output.splitlines()[-1])


def capture(run, pid, width, scale):
    window = native(run, "window", pid)
    path = run.folder / f"desktop-{width}.png"
    run.command(["/usr/sbin/screencapture", "-x", "-o", "-l", str(window["id"]), str(path)])
    data = path.read_bytes()
    require(data[:8] == b"\x89PNG\r\n\x1a\n", "Native screenshot is not PNG")
    pixels = struct.unpack(">II", data[16:24])
    require(pixels[0] == int(width * scale) and pixels[1] > 0 and len(data) > 10000, "Native screenshot dimensions/content differ")
    return dict(path=str(path.relative_to(ROOT)), sha256=file_hash(path), pixels=pixels, bytes=len(data))


def desktop(run, session):
    pid = session.process.pid
    snapshots, row_styles, screenshots = [], [], []
    for width in [960, 1200, 1512]:
        session.command("layout", value=width)
        session.command("tv1-theme", value=0)
        row = session.command("tv1", action="desktop")
        snapshots.append(row)
        validate_desktop(row)
        screenshots.append(capture(run, pid, width, row["native"]["scale_factor"]))
        fixture_state = session.command("tv1", action="fixture")
        styles = session.command("tv1", action="row-styles")
        images = session.command("tv1", action="images")
        require(images["loaded"] > 0 and images["valid"] and images["missing"] == fixture_state["expected_missing"], "Actual poster load/fade and missing-title states differ")
        row_styles.append(dict(width=width, styles=styles))
        validate_rows(styles, width)
    run.observed.update(desktop=snapshots, media_rows=row_styles, screenshots=screenshots)
    behavior(run, "TV-THEME", "visual-tokens", "exact shared tokens, typography, sizes, glass and MediaRow computed styles at three real widths", lambda: dict(layouts=[validate_desktop(row) for row in snapshots], rows=[validate_rows(row["styles"], row["width"]) for row in row_styles], screenshots=screenshots))
    session.command("layout", value=960)
    session.command("tv1", action="toggle")
    expanded = session.command("tv1", action="desktop")
    session.command("tv1", action="toggle")
    collapsed = session.command("tv1", action="desktop")
    session.command("layout", value=1200)
    routes = [session.command("tv1", action=f"nav-{name}") for name in ["history", "sources", "settings", "home"]]
    def sidebar():
        near(expanded["sidebar"]["width"], 220, "Expanded sidebar")
        near(collapsed["sidebar"]["width"], 72, "Collapsed sidebar")
        require(len(expanded["sources"]) == 2 and all(source["value"] for source in expanded["sources"]), "Both existing source selections must be in the expanded sidebar")
        require([row["route"] for row in routes] == ["history", "sources", "settings", "home"], "Existing navigation did not open actual pages")
        return dict(expanded=expanded["sidebar"], collapsed=collapsed["sidebar"], routes=routes)
    behavior(run, "TV-SHELL", "floating-sidebar-collapse", "default responsive collapse, operable source capsules and all four current routes", sidebar)
    original_card = session.command("tv1", action="focus-real-card")
    session.command("tv1", action="nav-history")
    native(run, "key", pid, 53)
    restored_card = session.command("tv1", action="home-ready")
    palettes = []
    for theme in [0, 1]:
        session.command("tv1-theme", value=theme)
        for name, accent in [("lime", "#c5ed88"), ("ocean", "#a7d0ff"), ("violet", "#cdb8ff"), ("rose", "#f5b6ce"), ("amber", "#f0ca85")]:
            value = session.command("tv1", action=f"palette-{name}")
            palettes.append(dict(theme=theme, palette=name, expected=accent, **value))
    def dark():
        require(len(palettes) == 10 and all(row["accent"] == row["expected"] and row["dark"] == "dark" for row in palettes), "Five night palettes must remain active under both OS themes")
        return dict(samples=palettes)
    behavior(run, "TV-THEME", "desktop-dark-palette", "desktop fixed dark under both real OS themes, five night accent colors", dark)
    session.command("tv1", action="palette-lime")
    session.command("tv1-theme", value=0)
    session.command("tv1", action="fixture")

    style = session.command("tv1-shell-style")
    before = native(run, "window", pid)
    native(run, "drag", pid, 1000, 20, -40, 40)
    after = native(run, "window", pid)
    # An actual button consumes the press, even when the pointer is dragged.
    native(run, "drag", pid, 44, 82, 30, 20)
    button_after = native(run, "window", pid)
    def drag():
        require(style == dict(full_size_content=True, title_hidden=True, transparent_titlebar=True), "Actual NSWindow Overlay/title visibility differs")
        require(abs(after["bounds"]["X"] - before["bounds"]["X"]) >= 20 and abs(after["bounds"]["Y"] - before["bounds"]["Y"]) >= 20, "Actual titlebar drag did not move the native window")
        require(button_after["bounds"] == after["bounds"], "Button unexpectedly started a native drag")
        return dict(style=style, before=before, after=after, button_after=button_after)
    behavior(run, "TV-SHELL", "overlay-titlebar-drag", "actual NSWindow Overlay, hidden title, native movement and button exclusion", drag)

    first = session.command("tv1", action="focus-first")
    navigation = []
    for key in [124, 125, 126, 123]:
        native(run, "key", pid, key)
        navigation.append(session.command("tv1", action="active"))
    session.command("tv1", action="focus-last")
    native(run, "key", pid, 124)
    boundary = session.command("tv1", action="active")
    session.command("tv1", action="focus-first")
    native(run, "key", pid, 53)
    escape_one = session.command("tv1", action="active")
    native(run, "key", pid, 53)
    escape_two = session.command("tv1", action="active")
    session.command("tv1", action="toggle")
    source_focus = session.command("tv1", action="focus-source")
    native(run, "key", pid, 53)
    escape_source = session.command("tv1", action="active")
    session.command("tv1", action="toggle")
    session.command("tv1", action="focus-first")
    paged = session.command("tv1", action="pager-next")
    def keyboard():
        require(first["focused"] and first["id"] == "tv1-0", "First card did not gain actual DOM focus")
        require(navigation[0]["id"] == "tv1-1" and navigation[0]["focus_visible"], "ArrowRight did not move visible keyboard focus to next card")
        require(navigation[1]["row"] == "TV1 landscape" and navigation[2]["row"] == "TV1 portrait", "ArrowDown/Up did not traverse actual content rows")
        require(all(item["focused"] for item in navigation) and boundary["focused"] and boundary["id"] == "tv1-13", "Edge navigation lost focus")
        require(escape_one["href"] == "#/home" and "tv-sidebar-toggle" in escape_two["class"], "Escape did not return content → sidebar → collapsed toggle")
        require(source_focus["tag"] == "SELECT" and "tv-sidebar-toggle" in escape_source["class"], "Escape from a source capsule must return to the collapsed sidebar toggle")
        require(original_card["href"] == restored_card["href"] and restored_card["focused"], "Escape from another route must restore the actual home card focus")
        require(paged["scroll"] > 0 and paged["active"]["focused"], "Horizontal paging did not scroll and preserve native focus")
        return dict(sequence=navigation, boundary=boundary, escape=[escape_one, escape_two, escape_source], restored=restored_card, paged=paged)
    behavior(run, "TV-FOCUS", "keyboard-navigation", "native arrow input changes document.activeElement across cards/rows; edge, Escape and scroll preserve focus", keyboard)

    target = session.command("tv1", action="hover-target")
    native(run, "hover", pid, target["x"], target["y"])
    hover = session.command("tv1", action="hover-style")
    native(run, "hover", pid, target["edge_x"], target["edge_y"])
    edge_hover = session.command("tv1", action="hover-style")
    native(run, "hover", pid, 1100, 40)
    session.command("tv1", action="focus-first")
    native(run, "key", pid, 124)
    focus = session.command("tv1", action="active")
    def parity():
        require(hover["hovered"] and not hover["focus_visible"] and focus["focus_visible"], "Actual mouse/keyboard modalities missing")
        require(hover["shadow"] == focus["shadow"] == "rgba(0, 0, 0, 0.55) 0px 18px 40px 0px", "Hover/focus shadows differ")
        require("1.08" in hover["transform"] and "1.08" in focus["transform"] and hover["highlight"] == focus["highlight"] == "1", "Hover/focus scale/highlight differ")
        require("2px" in focus["outline"] and "rgb(245, 245, 247)" in focus["outline"] and focus["outline_offset"] == "-2px" and focus["transition"] == "0.2s, 0.2s", "Keyboard inner ring/focus timing differs")
        require(all(abs(float(edge_hover[key].removesuffix("deg") or 0)) <= 4 for key in ["tilt_x", "tilt_y"]) and any(abs(float(edge_hover[key].removesuffix("deg") or 0)) > 1 for key in ["tilt_x", "tilt_y"]), "Pointer tilt must respond within four degrees")
        return dict(hover=hover, edge=edge_hover, focus=focus)
    behavior(run, "TV-FOCUS", "hover-focus-parity", "real hover/keyboard have scale1.08, same shadow/highlight, keyboard2px inner ring and200ms, tilt≤4°", parity)

    verify_motion(run, session)
    return dict(desktop_widths=3, screenshots=screenshots)


def verify_motion(run, session):
    pid = session.process.pid
    original_motion = session.command("tv1", action="motion")
    try:
        deadline = time.monotonic() + 10
        reduced = session.command("tv1", action="motion")
        while not reduced["reduced"] and time.monotonic() < deadline:
            override = session.command("tv1-motion", value=1)
            if override["renderer"] > 0:
                session.register_webkit(dict(pid=pid, webkit_pids={**session.record["webkit_pids"], "inspector": override["renderer"]}))
            time.sleep(.25)
            reduced = session.command("tv1", action="motion")
        session.command("tv1", action="focus-first")
        native(run, "key", pid, 124)
        reduced_focus = session.command("tv1", action="active")
        target = session.command("tv1", action="hover-target")
        native(run, "hover", pid, target["x"], target["y"])
        reduced_hover = session.command("tv1", action="hover-style")
        def motion():
            require(reduced["reduced"], "Web Inspector page override did not change actual WebView prefers-reduced-motion")
            require(reduced_focus["transform"] == reduced_hover["transform"] == "none" and reduced_hover["animation"] == "none", "Reduce Motion must disable focus scale/tilt")
            require(all(item["transform"] == "none" and item["animation"] == "none" for item in reduced["animations"]), "Reduce Motion left translation/scale/push-in running")
            return dict(media=reduced, focus=reduced_focus, hover=reduced_hover)
        behavior(run, "TV-FOCUS", "reduced-motion", "page-scoped Web Inspector ReducedMotion media override disables scale/tilt/push-in in actual WebView", motion)
    finally:
        session.command("tv1-motion", value=0)
        deadline = time.monotonic() + 5
        restored = session.command("tv1", action="motion")
        while restored["reduced"] != original_motion["reduced"] and time.monotonic() < deadline:
            time.sleep(.1)
            restored = session.command("tv1", action="motion")
        require(restored["reduced"] == original_motion["reduced"], "Page motion override was not removed on inspector close")


def windows(run, baseline=False, reference=False, conditions=False):
    if not baseline and not reference and not conditions:
        # The frozen first observation caught native appearance color transitions.
        # Reobserve its unchanged, hash-verified original executable in this A1;
        # compare every settled computed value to the current product afterwards.
        windows(run, reference=True)
    previous = t01.DATA, t01.BUILD, t12.LIVE_TEST
    t01.DATA = ROOT / json.loads(BASELINE.read_text())["build"] if reference else run.folder / "packaged"
    if reference:
        t01.DATA = t01.DATA.parent
    else:
        t01.DATA.mkdir()
    t01.BUILD = t01.DATA / "build.json"
    t12.LIVE_TEST = t14.LIVE_TEST
    session = bridge = None
    try:
        if not (run.folder / "tv1-native").exists():
            run.command(["clang", "-fobjc-arc", "-framework", "Cocoa", "-framework", "ApplicationServices", "tests/fixtures/tv1_native.m", "-o", str(run.folder / "tv1-native")])
        run.observed["native_permissions"] = native(run, "permissions", 0)
        require(t01.native_gui_session()["screen_locked"] is not True, "Unlock the local macOS desktop before TV1")
        t01.check_media()
        if not reference:
            t01.build(run)
        receipt = t01.check_build(json.loads(BASELINE.read_text())["source"] if reference else run.source)
        executable = ROOT / receipt["automation"]["path"] / "Contents/MacOS/moc-video"
        with tempfile.TemporaryDirectory(prefix="tv1-live-", dir=run.folder) as temporary:
            folder = Path(temporary)
            media = folder / "media"
            media.mkdir()
            bridge = t14.Bridge(run, folder, media)
            bridge.wait(folder / "ready.json", timeout=180)
            with t01.Fixture(t01.MEDIA, invoke=bridge.invoke) as fixture:
                session = t01.NativeSession(run, fixture, executable, 0 if reference else 1)
                session.wait(0, 30)
                session.command("identity")
                rows = []
                for width in [768, 390]:
                    session.command("layout", value=width)
                    for theme in [0, 1]:
                        session.command("tv1-theme", value=theme)
                        # Wait beyond both the native appearance delivery and
                        # the established anchor color transition before sampling.
                        time.sleep(1.5)
                        row = session.command("tv1", action="measure")
                        row["theme"] = theme
                        rows.append(row)
                run.observed["original_narrow" if reference else "narrow"] = rows
                require(all(not row["overflow"] for row in rows), "Narrow shell overflowed")
                if conditions:
                    session.command("layout", value=1200)
                    fixture_state = session.command("tv1", action="fixture")
                    images = session.command("tv1", action="images")
                    run.observed["image_conditions"] = dict(input=fixture_state, observed=images)
                    run.case("image-conditions", IDS, "decoded image/fade and missing count match the actual fixture input", lambda: require(images["loaded"] > 0 and images["valid"] and images["missing"] == fixture_state["expected_missing"], "Fixture image behavior differs"))
                    verify_motion(run, session)
                elif reference:
                    # Reobserve only the frozen original executable to resolve
                    # the one-channel color transition in the initial snapshot.
                    repeat = session.command("tv1", action="measure")
                    write_json(run.folder / "original-narrow-reference.json", dict(baseline_sha256=file_hash(BASELINE), original_build=receipt, rows=rows, settled_repeat=repeat))
                    require(file_hash(BASELINE) == BASELINE_SHA, "Initial narrow baseline was modified")
                    initial = json.loads(BASELINE.read_text())
                    for actual, frozen in zip(rows, initial["rows"]):
                        for selector in actual["nodes"]:
                            # Only anchor color was sampled during an animation;
                            # all other original style/geometry stays exact.
                            left = actual["nodes"][selector]
                            right = frozen["nodes"][selector]
                            if selector in [".brand", ".poster-link", ".card-title"]:
                                left = dict(rect=left["rect"], style={k: v for k, v in left["style"].items() if k != "color"})
                                right = dict(rect=right["rect"], style={k: v for k, v in right["style"].items() if k != "color"})
                            require(left == right, f"Unchanged original baseline geometry/style differs: {selector}")
                    run.observed["original_baseline_binding"] = dict(source=initial["source"], executable_sha256=session.record["executable_sha256"], baseline_sha256=file_hash(BASELINE), observation=str((run.folder / "original-narrow-reference.json").relative_to(ROOT)))
                elif baseline:
                    require(not BASELINE.exists(), "Frozen narrow baseline already exists; keep its original evidence")
                    write_json(BASELINE, dict(source=run.source, protected=t14.workflow.protected_hash(), build=str(t01.BUILD.relative_to(ROOT)), rows=rows))
                else:
                    def narrow():
                        require(file_hash(BASELINE) == BASELINE_SHA, "Frozen pre-change narrow evidence was modified")
                        expected = json.loads(BASELINE.read_text())
                        require(expected["protected"] == t14.workflow.protected_hash(), "Narrow baseline protected binding differs")
                        require([dict(width=row["width"], theme=row["theme"], nodes=row["nodes"]) for row in rows] ==
                                [dict(width=row["width"], theme=row["theme"], nodes=row["nodes"]) for row in run.observed["original_narrow"]], "Narrow production layout/style differs from the settled hash-verified original executable")
                        return dict(path=str(BASELINE.relative_to(ROOT)), sha256=file_hash(BASELINE), samples=len(rows))
                    behavior(run, "TV-SHELL", "narrow-unchanged", "768/390px × real light/dark match exact pre-change computed style/geometry evidence", narrow)
                    desktop(run, session)
                session.command("identity")
                session.close()
                require(session.record["exit"] == 0 and not session.record["lingering"], "Native fixture cleanup failed")
                require(fixture.active == 0, "Fixture HTTP remains")
            bridge.finish()
        return dict(narrow_samples=len(rows), build=str(t01.BUILD.relative_to(ROOT)))
    finally:
        try:
            if session and not session.closed:
                session.close(graceful=False)
        finally:
            if bridge:
                bridge.close()
            t01.DATA, t01.BUILD, t12.LIVE_TEST = previous


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["baseline", "narrow-reference", "conditions", "all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    if args.suite != "all":
        run.REQUIREMENTS = IDS
    else:
        require(args.report.resolve() == ROOT / "evidence/acceptance/TV1-all.json" and args.junit.resolve() == ROOT / "evidence/acceptance/TV1-all.xml", "Exact TV1 output paths required")
    if run.case("resource-ownership", ["REQ-TV1"], "original runner owns the approved RSS budget", lambda: resources(run, card="T0.28")):
        if args.suite == "all":
            behavior(run, "TV-SHELL", "permissions-exact", "permission set exactly baseline + allow-start-dragging; CSP byte-for-byte baseline", security)
        run.case("pre-change-narrow" if args.suite != "all" else "packaged-shell", IDS, "actual narrow layout and light/dark styles", lambda: windows(run, args.suite == "baseline", args.suite == "narrow-reference", args.suite == "conditions"))
    return run.save()


if __name__ == "__main__":
    # Reuse the established T22 wake guard through preflight/build phases too.
    awake = subprocess.Popen(["/usr/bin/caffeinate", "-di", "-w", str(os.getpid())]) if sys.platform == "darwin" else None
    try:
        code = main()
    finally:
        if awake is not None:
            awake.terminate()
            awake.wait(timeout=10)
    raise SystemExit(code)
