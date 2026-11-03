"""T13 behavior gate. Execute once under the original run.py (no nested runners).

Node exercises production state/IPC/preferences/history. A native WKWebView loads
the actual Vue app and records DOM interactions, decoded media and screenshots.
Native mobile execution remains in the separately authorized mobile stage.
"""
import argparse
import datetime as dt
import importlib.util
import itertools
import json
import math
import os
from pathlib import Path
import platform
import re
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid
import xml.etree.ElementTree as ET

from t00 import file_hash, read_json, require, workflow, write_json
from t11 import Acceptance as BaseAcceptance
import t12
import t01

ROOT = Path(__file__).resolve().parents[2]
IDS = ("U07", "U09", "R01", "R08", "R20", "R24", "R31", "R35", "R36", "R40", "R52", "R62", "R67", "REQ-T13")


def validate_ui(result):
    """Require the observed interaction matrix, beyond screenshot existence."""
    require(result.get("ok") is True and result.get("cases"), "native UI failed or did not run")
    rows = {row["id"]: row for row in result["cases"]}
    require(len(rows) == len(result["cases"]) and all(row["result"] == "PASS" for row in rows.values()), "duplicate or failed UI cases")
    if any(name.startswith("first-frame-") or name == "history-refresh-reset-and-preferences" for name in rows):
        for row in rows.values():
            actual = row["actual"]; first, expected = actual["first"], actual["expected"]
            require(first["palette"] == expected["color_palette"] and first["lang"] == expected["ui_locale"]
                    and first["scheme"] == ("dark" if first["dark"] else "light") and first["accent"] and actual["emptyHistory"] is True,
                    "first-frame preference/OS or session history reset differs")
        return
    required = {"identity-first-frame", "return-and-language-filter", "palette-language-system-matrix", "minimum-window-and-dpi",
                "storage-failure-feedback", "playback-locale-palette-and-shortcuts", "history-resume-merge-delete-clear-focus",
                "error-and-poster-contrast", "console-and-overlay-health", "history-reload-precondition", "history-refresh-reset-and-preferences"}
    require(required <= rows.keys(), "complete native interaction inventory missing")
    matrix = rows["palette-language-system-matrix"]["actual"]
    expected = set(itertools.product(("lime", "ocean", "violet", "rose", "amber"), (False, True), ("en", "zh-CN")))
    require(len(matrix) == 20 and {(row["palette"], row["dark"], row["lang"]) for row in matrix} == expected, "incomplete palette/OS/language matrix")
    for row in matrix:
        layout = row["layout"]
        require(layout["width"] == 390 and layout["scrollWidth"] <= layout["width"] + 1, "390px layout clips")
        for kind in ("contrast", "popupContrast", "catalogContrast", "playerContrast", "errorContrast"):
            require(row[kind] and all(isinstance(sample["contrast"], (int, float)) and math.isfinite(sample["contrast"])
                    and sample["contrast"] >= 4.5 for sample in row[kind]), "missing/failed measured text contrast")
    dpi = rows["minimum-window-and-dpi"]["actual"]
    require(len(dpi) == 4 and {row["native"]["zoom"] for row in dpi} == {1, 1.25, 1.5, 2}
            and all(row["native"]["width"] == 960 and row["native"]["height"] == 640
                    and row["layout"]["scrollWidth"] <= row["layout"]["width"] + 1 for row in dpi), "minimum window or DPI evidence incomplete")
    player = rows["playback-locale-palette-and-shortcuts"]["actual"]
    require(player["videoWidth"] > 0 and math.isfinite(player["duration"]) and player["duration"] > 0 and player["position"] > 0,
            "real decoded media metadata/progress missing")
    history = rows["history-resume-merge-delete-clear-focus"]["actual"]
    require(history["resumedPosition"] > 5 and 0 <= history["replayedPosition"] < 2 and history["mergedTitles"] == 1
            and history["latestEpisode"] == 3 and all(history[key] is True for key in ("cancelledDeletionPreservedData", "removed", "cleared")),
            "history resume/replay/merge/confirmation evidence differs")
    require(rows["console-and-overlay-health"]["actual"]["logs"] == [], "native UI console errors")


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T13-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def save(self):
        current = workflow.snapshot()
        self.case("stable-tested-source", ["REQ-T13"], "unchanged source throughout A1",
                  lambda: require(current == self.source, "source changed during A1"))
        summaries = []
        for identifier in IDS:
            children = [case for case in self.cases if identifier in case["requirements"]]
            summaries.append(dict(id=identifier, result="PASS" if children and all(case["result"] == "PASS" for case in children) else "FAIL",
                                  children=[case["id"] for case in children]))
        counts = dict(required=len(self.cases), ran=len(self.cases), passed=sum(case["result"] == "PASS" for case in self.cases),
                      failed=sum(case["result"] != "PASS" for case in self.cases), skipped=0)
        write_json(self.report_path, dict(card="T0.13", original_card="T13", suite="all", utc_started=self.started,
            utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
            tested_source_digest=current, protected=workflow.protected_hash(), platform=sys.platform,
            arch=platform.machine(), os=platform.platform(), observed=self.observed,
            commands=self.commands, cases=self.cases, summaries=summaries, counts=counts,
            deployment="packaged Tauri WKWebView with real Host/gateway plus explicit browser preview with local Vite/native WebKit; no cluster or Android",
            deferred=[dict(target=name, status="NOT RUN", reason=reason) for name, reason in (
                ("macos-x86_64/windows/linux", "ADR-005/006: local macOS arm64 is the required platform"),
                ("android/ios native run", "T13 checks public Rust library/cfg; mobile stage has its own authorization gate"),
                ("SQLite history persistence", "T18; this card exercises the declared session-only preview history"))]))
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T13-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T13", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T13 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return int(counts["failed"] > 0 or any(row["result"] != "PASS" for row in summaries))


def resources(run):
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "original run.py wrapper required")
    cfg = workflow.config()
    require(cfg["memory_mib"] == 4096 and cfg["jobs"] == 2, "approved resource budget differs")
    require(os.environ.get("CARGO_BUILD_JOBS") == "2" and os.environ.get("RUST_TEST_THREADS") == "1", "parallelism differs")
    require(Path(os.environ["CARGO_TARGET_DIR"]) == ROOT / "target", "target directory binding differs")
    require(Path.cwd().resolve() == ROOT and Path(sys.executable).absolute() == ROOT / ".venv/bin/python", "project/venv binding differs")
    ancestors = {process.pid for process in workflow.runtime.psutil.Process().parents()} | {os.getpid()}
    matches = []
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        matches = [(path, read_json(path)) for path in (ROOT / "evidence/runs").glob("*/receipt.json")
                   if read_json(path).get("status") == "running" and read_json(path).get("payload_pid") in ancestors]
        if matches:
            break
        time.sleep(0.05)
    require(len(matches) == 1, "one original resource receipt must own A1")
    path, receipt = matches[0]
    require(receipt["source"] == run.source and receipt["card"] == "T0.13", "receipt source/card differs")
    require(workflow.runtime.alive(dict(pid=receipt["payload_pid"], identity=receipt["payload_identity"])), "resource receipt process identity differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_mib"] == 4096 and env["jobs"] == 2
            and env["test_threads"] == 1, "resource backend/budget differs")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "A1 requires the approved local macOS arm64 host")
    require(env["memory_enforcement"] == "rss-sampled", "record the real macOS RSS sampling backend")
    run.observed["resources"] = dict(receipt=str(path.relative_to(ROOT)), environment=env,
        note="process tree plus owned WebKit XPC RSS sampled; transient overage possible; no cgroup memory/CPU hard isolation")
    return run.observed["resources"]


def frontend(run):
    files = ["state", "api", "i18n", "appearance", "history", "player", "playback", "t12-webview"]
    trace = run.folder / "state-trace.json"
    previous = os.environ.get("T13_STATE_TRACE")
    os.environ["T13_STATE_TRACE"] = str(trace)
    try:
        output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap",
                              *[f"tests/{name}.test.ts" for name in files]], timeout=180)
    finally:
        if previous is None:
            os.environ.pop("T13_STATE_TRACE", None)
        else:
            os.environ["T13_STATE_TRACE"] = previous
    count = re.search(r"^# tests (\d+)\s*$", output, re.M)
    passed = re.search(r"^# pass (\d+)\s*$", output, re.M)
    require(count and passed and int(count[1]) >= 20 and count[1] == passed[1], "nonzero state/settings/history tests must pass")
    require(re.search(r"^# fail 0\s*$", output, re.M) and re.search(r"^# skipped 0\s*$", output, re.M), "tests failed/skipped")
    named = re.findall(r"^ok \d+ - (.+)$", output, re.M)
    state_count = 0
    for name in named:
        if name.startswith(("U07/", "R01/", "R31/")):
            requirements = name.split(":", 1)[0].split("/")
            require(set(requirements) <= set(IDS), "state test declares an unknown requirement")
            state_count += 1
        elif name.startswith("T12"):
            requirements = ["R08", "R31", "R36", "REQ-T13"]
        elif name.startswith("T05"):
            requirements = ["R01", "R31", "R36", "REQ-T13"]
        elif any(word in name.lower() for word in ("history", "progress", "record", "completion")):
            requirements = ["U09", "R01", "REQ-T13"]
        elif any(word in name.lower() for word in ("palette", "color", "theme")):
            requirements = ["R40", "R67", "REQ-T13"]
        else:
            requirements = ["R08", "R62", "REQ-T13"]
        run.case(name, requirements, "production behavior assertions executed and passed",
                 lambda name=name: dict(test=name, exit=0, log=run.commands[-1]["log"]))
    require(state_count >= 13 and len(named) == int(count[1]), "complete named state behavior evidence required")
    require(trace.exists(), "actual AppState snapshots are missing")
    from jsonschema import Draft202012Validator, FormatChecker
    from referencing import Registry, Resource
    module = importlib.util.spec_from_file_location("t13_archived_contracts", ROOT / "replica-spec/validate_contracts.py")
    original = importlib.util.module_from_spec(module)
    module.loader.exec_module(original)
    bridge, app = read_json(ROOT / "protocol/bridge.openapi.json"), read_json(ROOT / "protocol/app.schema.json")
    registry = Registry().with_resource(original.BRIDGE_URI, Resource.from_contents({
        "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": original.BRIDGE_URI, **bridge,
    })).with_resource(app["$id"], Resource.from_contents(app))
    validator = Draft202012Validator({"$ref": app["$id"] + "#/$defs/AppState"}, registry=registry, format_checker=FormatChecker())
    snapshots = read_json(trace)
    require(len(snapshots) >= 6, "complete production state snapshot evidence required")
    for snapshot in snapshots:
        validator.validate(snapshot["value"])
        original.check_source_bindings(snapshot["value"])
        original.check_playback_resolutions(snapshot["value"])
        original.check_home_positions(snapshot["value"])
    run.case("actual-state-contracts", ["U07", "R31", "R36", "REQ-T13"], "actual state transitions satisfy original schema and cross bindings",
             lambda: dict(snapshots=len(snapshots), trace=str(trace.relative_to(ROOT))))
    run.command(["npm", "run", "check"], timeout=180)
    run.observed["frontend"] = dict(tests=int(count[1]), state_tests=state_count, exit=0)
    return run.observed["frontend"]


def public_library(run):
    lib = (ROOT / "src-tauri/src/lib.rs").read_text()
    main = (ROOT / "src-tauri/src/main.rs").read_text()
    require("#[cfg_attr(mobile, tauri::mobile_entry_point)]" in lib and "pub fn run()" in lib
            and "yingxiang_video::run()" in main, "shared desktop/mobile library entry is missing")
    require(re.search(r"#\[cfg\(all\(not\(mobile\), feature = \"t01-automation\"\)\)\]\s*mod automation", lib), "desktop automation lacks mobile cfg isolation")
    require(re.search(r"#\[cfg\(not\(mobile\)\)\]\s*let window = window\s*\.inner_size", lib), "desktop window size lacks cfg isolation")
    require("#[cfg(mobile)]" in lib and "|_| false" in lib, "mobile handler must exclude desktop commands")
    run.command(["cargo", "check", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib"], timeout=600)
    run.command(["cargo", "check", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "--features", "t01-automation"], timeout=600)
    return dict(public_entry="run", desktop_library="cargo check PASS", automation_library="cargo check PASS",
                mobile_runtime="NOT RUN: separate mobile stage; cfg and public entry audited here")


def observe_native(run, process, folder, owned, samples):
    path = folder / "native-status.json"
    if path.exists():
        status = read_json(path)
        require(status.get("pid") == process.pid and "error" not in status, "native WebKit ownership diagnostic failed")
        for role, pid in status["webkit_pids"].items():
            if pid <= 0:
                continue
            token = workflow.runtime.identity(pid)
            if not token:
                continue
            require("WebKit" in workflow.runtime.psutil.Process(pid).name(), f"foreign {role} helper")
            if pid in owned:
                require(owned[pid] == token, "WebKit PID was reused")
            owned[pid] = token
    root = workflow.runtime.psutil.Process()
    processes = {item.pid: item for item in [root, *root.children(recursive=True)]}
    for pid, token in owned.items():
        if workflow.runtime.identity(pid) == token:
            processes[pid] = workflow.runtime.psutil.Process(pid)
    rss = 0
    for child in processes.values():
        try:
            rss += child.memory_info().rss
        except workflow.runtime.psutil.NoSuchProcess:
            pass
    require(rss <= 4096 * 1024 * 1024, "combined native/host tree/WebKit RSS exceeded approved sampled 4096MiB budget")
    sample = dict(utc=dt.datetime.now(dt.timezone.utc).isoformat(), combined_rss_bytes=rss,
                  processes=[dict(pid=pid, identity=token) for pid, token in owned.items()])
    samples.append(sample)
    with (folder / "rss-samples.jsonl").open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(sample) + "\n")


def native_session(run, executable, url, name, seed=None):
    folder = run.folder / name
    folder.mkdir()
    script = ROOT / "tests/fixtures/t13" / ("startup.js" if seed else "browser.js")
    report = folder / "ui.json"
    argv = [str(executable), url, str(script), str(report), str(folder)]
    if seed:
        path = folder / "seed.js"
        path.write_text("window.__T13_SEED_EXPECT__=" + json.dumps(seed) + ";\n"
            + "window.__T13_SEED_EXPECT__.ui_theme=matchMedia('(prefers-color-scheme: dark)').matches?'light':'dark';\n"
            + "localStorage.setItem('yingxiang-video.settings',JSON.stringify(window.__T13_SEED_EXPECT__));\n"
            + "localStorage.setItem('yingxiang-video.palette','rose');localStorage.setItem('yingxiang-video.locale','zh-CN');\n"
            + "localStorage.setItem('yingxiang-video.theme','dark');\n", encoding="utf-8")
        argv.append(str(path))
    owned, samples = {}, []
    started = time.monotonic()
    record = dict(argv=argv, cwd=str(ROOT), log=str((folder / "native.log").relative_to(ROOT)), exit=None)
    run.commands.append(record)
    with (folder / "native.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen(argv, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        owned[process.pid] = workflow.runtime.identity(process.pid)
        try:
            while process.poll() is None:
                observe_native(run, process, folder, owned, samples)
                require(time.monotonic() - started < 930, "native UI session timed out")
                time.sleep(0.1)
            record["exit"] = process.returncode
        finally:
            record["seconds"] = time.monotonic() - started
            workflow.runtime.terminate([dict(pid=pid, identity=token) for pid, token in owned.items() if token])
            process.wait(timeout=10)
            record["remaining"] = [pid for pid, token in owned.items() if token and workflow.runtime.identity(pid) == token]
            require(not record["remaining"], "owned native/WebKit resources remain")
    require(report.exists(), "native UI did not produce its assertion report")
    result = read_json(report)
    validate_ui(result)
    for case in result.get("cases", []):
        require(case["requirements"] and set(case["requirements"]) <= set(IDS), "UI case has unknown/missing requirements")
        def check(case=case):
            require(case["result"] == "PASS", json.dumps(case, ensure_ascii=False))
            return dict(report=str(report.relative_to(ROOT)), actual=case["actual"], native_exit=record["exit"])
        run.case(case["id"], case["requirements"], "real WKWebView interaction and DOM/media assertions", check)
    images = []
    for screenshot in result.get("screenshots", []):
        path = folder / screenshot["file"]
        require(path.parent == folder and path.exists() and path.read_bytes().startswith(b"\x89PNG\r\n\x1a\n")
                and path.stat().st_size == screenshot["bytes"] and screenshot["width"] > 0 and screenshot["height"] > 0,
                "missing/invalid screenshot evidence")
        images.append(dict(path=str(path.relative_to(ROOT)), sha256=file_hash(path), **screenshot))
    require(images and samples, "native screenshot/RSS evidence missing")
    require(record["exit"] == 0 and result.get("ok") is True and result.get("cases"), "native behavior gate failed: " + str(result.get("error", result.get("cases"))))
    return dict(report=str(report.relative_to(ROOT)), cases=len(result["cases"]), screenshots=images,
                peak_combined_rss_bytes=max(item["combined_rss_bytes"] for item in samples), resources="identity-qualified cleanup complete",
                log=record["log"], exit=record["exit"])


def browser(run):
    require(t01.native_gui_session()["screen_locked"] is not True, "macOS desktop locked; unlock before native UI acceptance")
    executable = run.folder / "t13-webkit"
    run.command(["xcrun", "--sdk", "macosx", "clang", "-fobjc-arc", "-framework", "Cocoa", "-framework", "WebKit",
                 "tests/fixtures/t13/webkit.m", "-o", str(executable)], timeout=180)
    with socket.socket() as lease:
        lease.bind(("127.0.0.1", 0))
        port = lease.getsockname()[1]
    argv = workflow.runtime.resolve_argv(["node", "node_modules/vite/bin/vite.js", "--host", "127.0.0.1", "--port", str(port), "--strictPort"])
    log_path = run.folder / "vite.log"
    record = dict(argv=argv, cwd=str(ROOT), log=str(log_path.relative_to(ROOT)), exit=None)
    run.commands.append(record)
    with log_path.open("w", encoding="utf-8") as log:
        service = subprocess.Popen(argv, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                                   env=dict(os.environ, ROLLDOWN_WORKER_THREADS="2", RAYON_NUM_THREADS="2"))
        token = workflow.runtime.identity(service.pid)
        started = time.monotonic()
        try:
            url = f"http://127.0.0.1:{port}/"
            deadline = time.monotonic() + 20
            while True:
                require(service.poll() is None, "Vite exited before serving the actual app")
                try:
                    with urllib.request.urlopen(url, timeout=1) as response:
                        require(response.status == 200, "Vite document failed")
                    break
                except (urllib.error.URLError, TimeoutError):
                    require(time.monotonic() < deadline, "Vite did not become ready")
                    time.sleep(0.1)
            sessions = [native_session(run, executable, url, "ui")]
            require(sessions[0]["cases"] >= 10, "complete real UI suite required")
            defaults = dict(schema_version=2, last_source_key=None, volume=0.65, muted=False, auto_next=True, window_bounds=None,
                            source_configs=[], last_metadata_config_id=None, last_playback_source_key=None, ui_theme="dark")
            for index, palette in enumerate(("lime", "ocean", "violet", "rose", "amber")):
                seed = dict(defaults, color_palette=palette, ui_locale="en" if index % 2 == 0 else "zh-CN")
                sessions.append(native_session(run, executable, url, "startup-" + palette, seed))
            run.observed["browser"] = dict(engine="installed macOS WKWebView", url=url,
                browser_plugin="Browser plugin not available; project-native WebKit used without new dependencies", sessions=sessions)
            return run.observed["browser"]
        finally:
            workflow.runtime.terminate([dict(pid=service.pid, identity=token)] if token else [])
            service.wait(timeout=10)
            record.update(exit=service.returncode, seconds=time.monotonic() - started, stopped_by_fixture=True)


def packaged_playback(run):
    # Reuse every existing real Host/player assertion, keeping prior build
    # receipts/artifacts untouched. Shared compiler output stays under target/.
    previous = t01.DATA, t01.BUILD
    t01.DATA = run.folder / "packaged"
    t01.DATA.mkdir()
    t01.BUILD = t01.DATA / "build.json"
    try:
        return t12.webview(run)
    finally:
        t01.DATA, t01.BUILD = previous


def main():
    if not __debug__:
        raise SystemExit("Acceptance requires assertions; remove -O")
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T13-all.json" and args.junit.resolve() == ROOT / "evidence/acceptance/T13-all.xml", "Exact T13 output paths required")
    run = Acceptance(args.report, args.junit)
    owned = run.case("resource-ownership", ["R31", "REQ-T13"], "original scope/source and local macOS arm64", lambda: resources(run))
    if owned:
        def oracles():
            output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t13.py"])
            count = re.search(r"Ran (\d+) tests?", output)
            require(count and int(count[1]) >= 4, "Independent T13 oracle negatives must run")
            return dict(tests=int(count[1]))
        run.case("independent-negative-oracles", ["REQ-T13"], "corrupted matrix/layout/history/frame evidence is rejected", oracles)
        run.case("state-settings-history-behaviors", ["U07", "U09", "R01", "R24", "R31", "R35", "R36", "R62", "R67", "REQ-T13"],
                 "complete nonzero production frontend tests", lambda: frontend(run))
        run.case("public-library-and-desktop-cfg", ["R08", "R52", "REQ-T13"], "shared lib compiles; desktop branches excluded by mobile cfg", lambda: public_library(run))
        run.case("packaged-playback-regression", ["R08", "R31", "R36", "REQ-T13"],
                 "new-source real Host/gateway/WKWebView, all original T12 50-cycle/media/error/LRU/EOS/resource assertions",
                 lambda: packaged_playback(run))
        run.case("native-ui", ["U07", "U09", "R01", "R08", "R20", "R31", "R36", "R40", "R62", "R67", "REQ-T13"],
                 "real window, all 20 palettes/modes/locales, 390px/DPI, media/history/focus/error and five first-frame restorations", lambda: browser(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
