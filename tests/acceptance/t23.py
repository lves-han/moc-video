"""T23: verify the current personal installer and raw clean-machine evidence.

Only original run.py executes this suite. The clean target is supplied by the
human; a developer HOME or an existing test app never supplies that evidence.
"""
import argparse
from contextlib import closing
import datetime as dt
import json
import math
import os
from pathlib import Path, PurePosixPath
import platform
import plistlib
import re
import signal
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time
import zipfile
from urllib.parse import quote

import t01
import t10
import t14
from t00 import file_hash, read_json, require, workflow

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "packaging"))
import release

IDS = ["U04", "R30", "R33", "R45", "R50", "REQ-T23"]
BEHAVIORS = {"resource-ownership", "reference-platform", "current-build-receipt", "dependency-notices",
             "production-payload", "archive-roundtrip", "actual-signing-status", "clean-macos-lifecycle",
             "independent-oracle-negatives"}


def inside(name, folder):
    path = Path(name)
    require(not path.is_absolute() and ".." not in path.parts, "Artifact path must be relative")
    path = ROOT / path
    require(path.resolve().is_relative_to(folder.resolve()), "Artifact outside its owned directory")
    return path


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.27", "T23", IDS
    DEPLOYMENT = "Local archive/signature inspection; hash-bound human clean-macOS observations; no cluster or VM launched"
    DEFERRED = [dict(target=target, status="NOT RUN", reason="ADR-005/006; no target machine",
        reproduce_argv=["{python}", "-B", "tests/acceptance/t23.py", "--suite", "all",
                        "--report", "evidence/acceptance/T23-all.json", "--junit", "evidence/acceptance/T23-all.xml"],
        prerequisites="Authorized C1 target/venv/lock rebinding and a target-specific installer/lifecycle run; current inspector requires macOS arm64")
        for target in t01.DEFERRED]

    def __init__(self, report, junit):
        super().__init__(report.resolve(), junit.resolve())

    def command(self, argv, timeout=300, accepted=(0,)):
        # These commands only inspect local artifacts/system metadata. No live
        # source requests, Apple credentials or capability URLs enter them.
        argv = workflow.runtime.resolve_argv([str(arg) for arg in argv])
        path = self.folder / f"command-{len(self.commands) + 1}.log"
        row = dict(argv=argv, log=path.relative_to(ROOT).as_posix(), exit=None)
        self.commands.append(row)
        process, identity = None, None
        started = time.monotonic()
        with path.open("w", encoding="utf-8") as stream:
            stream.write(json.dumps(argv, ensure_ascii=False) + "\n")
            stream.flush()
            try:
                process = subprocess.Popen(argv, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
                identity = workflow.runtime.identity(process.pid)
                row["exit"] = process.wait(timeout=timeout)
            finally:
                if process is not None and process.poll() is None:
                    require(identity, "Command identity unavailable; original executor must clean up")
                    workflow.runtime.terminate(workflow.runtime.family(process.pid, identity))
                    process.wait(timeout=15)
                row["seconds"] = time.monotonic() - started
        row["sha256"] = file_hash(path)
        require(accepted is None or row["exit"] in accepted, f"Command exit {row['exit']}; {row['log']}")
        return path.read_text(encoding="utf-8", errors="replace").split("\n", 1)[1]

    def save(self):
        actual = {row["id"].split("/", 1)[1] for row in self.cases}
        self.case("required-behavior-inventory", IDS, sorted(BEHAVIORS),
                  lambda: require(BEHAVIORS <= actual, "Missing required behavior cases: " + str(sorted(BEHAVIORS - actual))))
        self.observed["limitations"] = dict(resource_backend="Original process-tree RSS sampling; transient overage possible; jobs2/thread1",
            lifecycle="Human executes the documented clean-machine procedure; this run rechecks raw files, app trees, data and recording, not saved PASS flags",
            signing="Ad-hoc only; Developer ID and notarization pending; default system security/quarantine retained")
        result = super().save()
        immutable = self.folder / "report.json"
        if self.report_path != immutable:
            immutable.write_bytes(self.report_path.read_bytes())
        print("T23_EVIDENCE_SHA256:" + file_hash(immutable), flush=True)
        return result


def environment(run):
    require(sys.platform == "darwin" and platform.machine() == "arm64"
            and int(platform.mac_ver()[0].split(".")[0]) >= 14, "Actual macOS14+ arm64 required")
    webkit = plistlib.loads(Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist").read_bytes())
    versions = {tool: run.command([tool, "-version" if tool.startswith("ff") else "--version"]).splitlines()[0]
                for tool in ("rustc", "cargo", "node", "npm", "ffmpeg", "ffprobe")}
    inventory = read_json(release.INVENTORY)
    run.observed["platform_versions"] = dict(os=platform.platform(), macos=platform.mac_ver()[0], arch=platform.machine(),
        webview={key: webkit.get(key) for key in ("CFBundleShortVersionString", "CFBundleVersion")}, tools=versions,
        dependency_inventory=dict(path="licenses/dependencies.json", sha256=file_hash(release.INVENTORY),
            versions=[{key: package[key] for key in ("ecosystem", "name", "version")} for package in inventory["packages"]]))
    return run.observed["platform_versions"]


def artifact(run):
    require(release.CURRENT.is_file(), "NOT RUN: build the current installer with packaging/release.py --build first")
    pointer = read_json(release.CURRENT)
    path = inside(pointer["manifest"], ROOT / "evidence/releases")
    require(file_hash(path) == pointer["sha256"], "Current manifest hash differs")
    manifest = read_json(path)
    require(manifest["schema_version"] == 2 and manifest["card"] == "T0.27"
            and manifest["source"] == run.source and manifest["protected"] == workflow.protected_hash(), "Installer source/baseline is stale")
    require(manifest["target"] == "macos-arm64" and manifest["scope"] == "personal use only; not public distribution", "Release scope differs")
    owner = read_json(ROOT / run.observed["resources"]["receipt"])
    marker = "T23_MANIFEST_SHA256:" + pointer["sha256"]
    matches = []
    for receipt_path in (ROOT / "evidence/runs").glob("*-diag-t23-release-build/receipt.json"):
        receipt = read_json(receipt_path)
        log = ROOT / receipt["log"]
        if marker not in log.read_text(encoding="utf-8", errors="replace"):
            continue
        require(receipt["status"] == "finished" and receipt["exit"] == receipt["raw_exit"] == 0
                and receipt["source"] == receipt["after_source"] == run.source
                and receipt["protected"] == owner["protected"] and receipt["generation"] == owner["generation"]
                and receipt["card"] == "T0.27" and file_hash(log) == receipt["log_hash"], "Original successful build receipt missing/changed")
        require(receipt["argv"] == ["{python}", "-B", "packaging/release.py", "--build"], "Unexpected release build argv")
        env = receipt["environment"]
        require((env["memory_mib"], env["jobs"], env["test_threads"], env["memory_enforcement"]) == (4096, 2, 1, "rss-sampled"), "Build resource backend differs")
        matches.append(str(receipt_path.relative_to(ROOT)))
    require(len(matches) == 1, "One hash-bound successful original build receipt required")
    for command in manifest["commands"]:
        log = inside(command["log"], path.parent)
        require(file_hash(log) == command["sha256"], "Build command log changed")
        if command["exit"] != 0:
            require(command["argv"][:2] in (["/usr/sbin/spctl", "--assess"], ["/usr/bin/xcrun", "stapler"]), "Build contains an unexpected failure")
    run.observed["release"] = dict(manifest=str(path.relative_to(ROOT)), sha256=pointer["sha256"], build_receipt=matches[0],
        logical_bytes=manifest["logical_bytes"], allocated_bytes=manifest["allocated_bytes"], archives=manifest["archives"])
    return manifest, path


def payload(manifest, path):
    app = inside(manifest["app"], path.parent)
    frontend = inside(manifest["frontend"], path.parent)
    require(release.tree(app) == manifest["files"] and release.tree(frontend) == manifest["frontend_files"], "App/frontend tree changed")
    require(manifest["logical_bytes"] == sum(row["bytes"] for row in manifest["files"].values()), "App size differs")
    require(manifest["locks"] == {name: file_hash(ROOT / name) for name in read_json(release.INVENTORY)["locks"]}
            and manifest["inventory_sha256"] == file_hash(release.INVENTORY)
            and manifest["notices_sha256"] == file_hash(release.NOTICES), "Bundled dependency evidence is stale")
    return release.check_payload(app, frontend)


def signing(run, manifest, path):
    app = inside(manifest["app"], path.parent)
    run.command(["/usr/bin/codesign", "--verify", "--deep", "--strict", app])
    detail = run.command(["/usr/bin/codesign", "--display", "--verbose=4", app])
    gatekeeper = run.command(["/usr/sbin/spctl", "--assess", "--type", "execute", "--verbose=4", app], accepted=None)
    gatekeeper_exit = run.commands[-1]["exit"]
    stapler = run.command(["/usr/bin/xcrun", "stapler", "validate", app], accepted=None)
    staple_exit = run.commands[-1]["exit"]
    checked = release.check_signing(detail, 0, gatekeeper_exit, staple_exit)
    require(all(manifest["signing"].get(key) == value for key, value in checked.items()), "Manifest overstates actual signature/notarization")
    architecture = run.command(["/usr/bin/lipo", "-archs", app / "Contents/MacOS/yingxiang-video"]).strip()
    require(architecture == "arm64", "Thin arm64 executable required")
    linkage = run.command(["/usr/bin/otool", "-L", app / "Contents/MacOS/yingxiang-video"])
    dependencies = [line.strip().split(" (", 1)[0] for line in linkage.splitlines()[1:] if line.strip()]
    require(dependencies and all(name.startswith(("/System/Library/", "/usr/lib/")) for name in dependencies), "Executable depends on developer/non-system libraries")
    run.observed["signing"] = dict(checked, gatekeeper_exit=gatekeeper_exit, gatekeeper_output=gatekeeper,
        staple_exit=staple_exit, staple_output=stapler, detail=detail, linked_libraries=dependencies)
    return run.observed["signing"]


def check_zip(path, expected_bytes):
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        require(names and len(names) == len(set(names)), "Empty/duplicate archive entries")
        for row in archive.infolist():
            name = PurePosixPath(row.filename)
            require(not name.is_absolute() and ".." not in name.parts and "\\" not in row.filename, "Unsafe ZIP entry")
            require((row.external_attr >> 16) & 0o170000 != 0o120000, "Unexpected ZIP symlink")
        require(sum(row.file_size for row in archive.infolist()) == expected_bytes, "ZIP payload length differs")
        require(archive.testzip() is None, "ZIP CRC differs")


def archives(run, manifest, path):
    predecessor = manifest["upgrade_from"]
    report_path = inside(predecessor["report"], ROOT / "evidence/acceptance")
    require(file_hash(report_path) == predecessor["report_sha256"], "Predecessor report changed")
    report = read_json(report_path)
    receipt = read_json(inside(report["observed"]["resources"]["receipt"], ROOT / "evidence/runs"))
    original_log = inside(receipt["log"], ROOT / "evidence/runs")
    require(receipt["status"] == "finished" and receipt["exit"] == receipt["raw_exit"] == 0
            and receipt["card"] == "T0.26" and receipt["source"] == receipt["after_source"] == predecessor["source"]
            and file_hash(original_log) == receipt["log_hash"]
            and "T22_EVIDENCE_SHA256:" + predecessor["report_sha256"] in original_log.read_text(encoding="utf-8"), "Predecessor successful raw receipt/log no longer binds its report")
    require(release.tree(inside(predecessor["app"], ROOT / "evidence/acceptance")) == predecessor["files"], "Predecessor application changed")
    require(len(manifest["archives"]) == 2 and {Path(row["path"]).suffix for row in manifest["archives"]} == {".zip", ".dmg"}, "ZIP and DMG required")
    for row in manifest["archives"] + [manifest["upgrade_from"]["archive"]]:
        archive = inside(row["path"], path.parent)
        require(archive.stat().st_size == row["bytes"] and file_hash(archive) == row["sha256"], "Archive changed")
        with tempfile.TemporaryDirectory(prefix="archive-", dir=run.folder) as temporary:
            folder = Path(temporary)
            if archive.suffix == ".zip":
                previous = row == manifest["upgrade_from"]["archive"]
                files = manifest["upgrade_from"]["files"] if previous else manifest["files"]
                check_zip(archive, sum(entry["bytes"] for entry in files.values()))
                run.command(["/usr/bin/ditto", "-x", "-k", archive, folder])
                apps = list(folder.iterdir())
                require(len(apps) == 1 and apps[0].suffix == ".app" and release.tree(apps[0]) == files, "ZIP roundtrip tree differs")
                run.command(["/usr/bin/codesign", "--verify", "--deep", "--strict", apps[0]])
            else:
                run.command(["/usr/bin/hdiutil", "verify", archive])
                mount = folder / "mount"
                mount.mkdir()
                def cancelled(*_):
                    raise KeyboardInterrupt("Archive inspection cancelled")
                previous_term = signal.signal(signal.SIGTERM, cancelled)
                try:
                    output = run.command(["/usr/bin/hdiutil", "attach", "-readonly", "-nobrowse", "-plist", "-mountpoint", mount, archive])
                    # hdiutil may append diagnostics after its complete plist.
                    start, end = output.index("<?xml"), output.index("</plist>") + len("</plist>")
                    mounted = plistlib.loads(output[start:end].encode("utf-8"))
                    require(any(Path(row.get("mount-point", "")) == mount for row in mounted["system-entities"]), "Owned DMG mount missing")
                    require(release.tree(mount / "映象视频.app") == manifest["files"], "DMG roundtrip tree differs")
                    require((mount / "Applications").is_symlink() and os.readlink(mount / "Applications") == "/Applications", "DMG install shortcut differs")
                    require({entry.name for entry in mount.iterdir()} <= {"映象视频.app", "Applications", ".DS_Store", ".fseventsd", ".Trashes"}, "Unexpected DMG payload")
                    run.command(["/usr/bin/codesign", "--verify", "--deep", "--strict", mount / "映象视频.app"])
                finally:
                    try:
                        if os.path.ismount(mount):
                            run.command(["/usr/bin/hdiutil", "detach", mount], accepted=None)
                            if run.commands[-1]["exit"] != 0:
                                run.command(["/usr/bin/hdiutil", "detach", "-force", mount])
                            require(not os.path.ismount(mount), "Owned disk image remains mounted")
                    finally:
                        signal.signal(signal.SIGTERM, previous_term)
    before = plistlib.loads((inside(predecessor["app"], ROOT / "evidence/acceptance") / "Contents/Info.plist").read_bytes())
    require(before["CFBundleVersion"] == "1" and manifest["payload"]["bundle_version"] == "2", "Actual build upgrade pair missing")
    require(before["CFBundleIdentifier"] == manifest["payload"]["identifier"], "Upgrade application identity differs")
    return dict(archives=3, zip_roundtrips=2, readonly_dmg_roundtrips=1, upgrade_pair=["0.1.0 (1)", "0.1.0 (2)"], cleanup="Owned temporary extracts removed and DMG detached")


def number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def check_lifecycle(value, manifest, manifest_hash):
    """Check raw observations; no stored result/PASS field has authority."""
    require(value["manifest_sha256"] == manifest_hash and value["source"] == manifest["source"]
            and value["protected"] == manifest["protected"], "Lifecycle is for another installer/source")
    require(value["target"] == "macos-arm64" and int(value["macos"].split(".")[0]) >= 14, "Clean target OS/architecture differs")
    require(value["clean_system_method"] in {"fresh-macos-install", "restored-clean-snapshot", "separate-clean-mac", "dedicated-test-vm"}
            and isinstance(value["human_observer"], str) and value["human_observer"].strip(), "Human-attested clean system required")
    require(value["security_settings_changed"] is False and value["quarantine_removed"] is False
            and value["security_exceptions"] == [], "Default security/quarantine must be retained")
    security = value["security_commands"]
    for argv, text in [(["/usr/sbin/spctl", "--status"], "assessments enabled"),
                       (["/usr/bin/csrutil", "status"], "System Integrity Protection status: enabled")]:
        require(any(row["argv"] == argv and row["exit"] == 0 and text in row["output"] for row in security), "Actual default security observation missing")
    phases = value["phases"]
    require(set(phases) == {"baseline", "installed", "initialized", "first_play", "upgrade_before", "upgrade_after", "uninstalled", "data_deleted"}, "Lifecycle steps incomplete")
    for name, phase in phases.items():
        require(phase.get("evidence"), f"Raw evidence missing for {name}")
        require(dt.datetime.fromisoformat(phase["utc"]).tzinfo is not None,
                "Actual observation timestamp required")
    base, installed, init, play, before, after, uninstalled, deleted = (phases[key] for key in
        ("baseline", "installed", "initialized", "first_play", "upgrade_before", "upgrade_after", "uninstalled", "data_deleted"))
    require(base["app_exists"] is False and base["data_exists"] is False, "Pre-install application/data already existed")
    require(installed["destination"] == "/Applications/映象视频.app" and installed["app_files"] == manifest["files"], "Installed current bundle differs")
    require(init["window_visible"] is True and init["ready"] is True and init["settings"]["schema_version"] == 2, "First initialization did not produce a ready application/settings")
    samples = play.get("samples", [])
    require(all(number(row["elapsed_seconds"]) and number(row["position_seconds"]) for row in samples), "Nonfinite playback observation")
    require(play["moving_picture"] is True and play["audio_audible"] is True
            and play["errors"] == [], "First-play picture/audio observation failed")
    require(before["app_files"] == manifest["upgrade_from"]["files"] and after["app_files"] == manifest["files"], "Upgrade was not actual build 1 to build 2")
    predecessor_started = before["ready"] is True or (
        before.get("window_visible") is True
        and before.get("startup_failure") == "runtime checking; missing startup preparation in immutable build 1"
        and before.get("startup_evidence") in before["evidence"])
    require(predecessor_started and after["ready"] is True and after["window_visible"] is True,
            "Actual predecessor launch and upgraded ready application required")
    require(before["settings"]["schema_version"] == after["settings"]["schema_version"] == 2
            and before["settings"].get("source_configs"), "Nonempty persisted source settings required before upgrade")
    require({key: row for key, row in before["settings"].items() if key != "window_bounds"}
            == {key: row for key, row in after["settings"].items() if key != "window_bounds"}, "Upgrade lost persisted settings")
    require(before["history"] and before["history"] == after["history"], "Upgrade lost nonempty watch history")
    require(uninstalled["app_exists"] is False and uninstalled["data_exists"] is True
            and uninstalled["settings"] == after["settings"] and uninstalled["history"] == after["history"], "App removal did not preserve user data")
    require(deleted["app_exists"] is False and deleted["data_exists"] is False
            and deleted["explicit_test_data_deletion"] is True and deleted["remaining_owned_processes"] == [], "Explicit test-data cleanup or process cleanup incomplete")
    return dict(target=value["target"], clean_system_method=value["clean_system_method"], phases=list(phases),
        first_play_samples=len(samples), source_configs=len(before["settings"]["source_configs"]), history_rows=len(before["history"]))


def lifecycle(run, manifest, path):
    folder = ROOT / "evidence/releases/T23-clean-macos"
    report = folder / "observations.json"
    require(report.is_file(), "NOT RUN: clean macOS installation/initialization/first play/upgrade/uninstall evidence missing; packaging/T23.md")
    value = read_json(report)
    artifacts = value["artifacts"]
    used = {name for phase in value["phases"].values() for name in phase["evidence"]}
    require(used and used <= artifacts.keys(), "Lifecycle raw artifacts missing")
    for name, row in artifacts.items():
        raw = inside(row["path"], folder)
        require(raw.stat().st_size > 0 and file_hash(raw) == row["sha256"], f"Lifecycle artifact changed: {name}")
    for command in value["security_commands"]:
        command["output"] = inside(artifacts[command["evidence"]]["path"], folder).read_text(encoding="utf-8")
    for name in ("installed", "upgrade_before", "upgrade_after"):
        phase = value["phases"][name]
        require(phase["file_tree"] in phase["evidence"], "Installed file-tree capture missing")
        phase["app_files"] = read_json(inside(artifacts[phase["file_tree"]]["path"], folder))
    # Reopen actual target data, not the report's preservation verdict. Fixtures
    # use public/owned sources and contain no accounts or runtime media tokens.
    for name in ("initialized", "upgrade_before", "upgrade_after", "uninstalled"):
        phase = value["phases"][name]
        require(phase["settings_file"] in phase["evidence"], "Target settings capture missing")
        settings = inside(artifacts[phase["settings_file"]]["path"], folder)
        phase["settings"] = read_json(settings)
        if name != "initialized":
            require(phase["history_db"] in phase["evidence"], "Target history capture missing")
            database = inside(artifacts[phase["history_db"]]["path"], folder)
            with closing(sqlite3.connect("file:" + quote(str(database)) + "?mode=ro&immutable=1", uri=True)) as connection:
                connection.row_factory = sqlite3.Row
                require(connection.execute("PRAGMA integrity_check").fetchall()[0][0] == "ok"
                        and connection.execute("PRAGMA user_version").fetchone()[0] == 1, "Target history database is invalid")
                phase["history"] = [dict(row) for row in connection.execute("SELECT * FROM watch_history ORDER BY content_id")]
    checked = check_lifecycle(value, manifest, file_hash(path))
    if not value.get("recording"):
        play = value["phases"]["first_play"]
        confirmation = play.get("audio_confirmation")
        frames = play.get("screen_samples", [])
        require(confirmation in play["evidence"] and confirmation in artifacts,
                "User audio confirmation evidence missing")
        text = inside(artifacts[confirmation]["path"], folder).read_text(encoding="utf-8")
        require("能听到正常声音" in text, "Actual user audio confirmation missing")
        require(len(frames) >= 1 and all(frame in play["evidence"] for frame in frames),
                "First-play screen samples missing")
        images = [inside(artifacts[frame]["path"], folder).read_bytes() for frame in frames]
        require(all(data.startswith(b"\x89PNG\r\n\x1a\n") or (data.startswith(b"\xff\xd8") and data.endswith(b"\xff\xd9")) for data in images)
                and all(len(data) > 24 for data in images),
                "Distinct actual playback screen captures required")
        run.observed["clean_lifecycle"] = dict(checked, report=str(report.relative_to(ROOT)),
            sha256=file_hash(report), audio_confirmation=confirmation, screen_samples=frames,
            evidence="Native UI picture/progress observations and explicit user audio confirmation; no recording")
        return run.observed["clean_lifecycle"]
    require(value["recording"] in value["phases"]["first_play"]["evidence"], "First-play recording is not bound to its phase")
    recording = inside(artifacts[value["recording"]]["path"], folder)
    probe = json.loads(run.command(["ffprobe", "-v", "error", "-count_frames", "-show_entries", "format=duration:stream=codec_type,nb_read_frames", "-of", "json", recording]))
    duration = float(probe["format"]["duration"])
    samples = value["phases"]["first_play"]["samples"]
    require(number(duration) and duration >= samples[-1]["elapsed_seconds"] - samples[0]["elapsed_seconds"]
            and any(row["codec_type"] == "video" and int(row.get("nb_read_frames", 0)) > 1 for row in probe["streams"])
            and any(row["codec_type"] == "audio" for row in probe["streams"]), "Actual first-play picture/audio recording missing")
    audio = run.folder / "first-play-audio.f32"
    run.command(["ffmpeg", "-nostdin", "-n", "-v", "error", "-i", recording, "-vn", "-ac", "1", "-ar", "8000", "-f", "f32le", audio], timeout=180)
    # Decode samples without dumping the camera's location/device metadata.
    pcm = [row[0] for row in struct.iter_unpack("<f", audio.read_bytes())]
    require(pcm and all(number(row) for row in pcm), "Finite decoded recording audio samples required")
    rms = math.sqrt(sum(row * row for row in pcm) / len(pcm))
    require(rms > 0, "Playback recording has no decoded audio energy")
    run.observed["clean_lifecycle"] = dict(checked, report=str(report.relative_to(ROOT)), sha256=file_hash(report),
        recording_sha256=file_hash(recording), decoded_audio_samples=len(pcm), audio_rms=rms,
        decoded_audio=dict(path=str(audio.relative_to(ROOT)), sha256=file_hash(audio)),
        evidence="Human witnessed raw screen/speaker recording and phase/data captures rechecked in current A1")
    return run.observed["clean_lifecycle"]


def oracles(run):
    output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t23.py", "-v"])
    match = re.search(r"Ran (\d+) tests?", output)
    require(match and int(match[1]) > 0 and re.search(r"(?m)^OK\s*$", output), "Required nonzero evidence counterexamples failed")
    return dict(tests=int(match[1]))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    ready = run.case("resource-ownership", IDS, "Original run.py receipt and approved macOS resource backend", lambda: t10.resources(run, card="T0.27"))
    run.case("reference-platform", ["U04", "R30", "REQ-T23"], "Actual macOS14+ arm64/WebKit/tool versions", lambda: environment(run) if ready else require(False, "NOT RUN: original resource owner missing"))
    run.case("independent-oracle-negatives", IDS, "Corrupt/stale/empty/missing/unsealed evidence fails", lambda: oracles(run) if ready else require(False, "NOT RUN: original resource owner missing"))
    bound = []
    def bind():
        bound.extend(artifact(run))
        return run.observed["release"]
    run.case("current-build-receipt", IDS, "Same-source/protected generation, immutable manifest and original successful build", lambda: bind() if ready else require(False, "NOT RUN: original resource owner missing"))
    run.case("dependency-notices", ["R33", "R45", "REQ-T23"], "Recollect exact 430 installed locked npm/Cargo/SQLite notices and compare bytes", lambda: run.command(["{python}", "-B", "packaging/release.py", "--verify-licenses"]) if ready else require(False, "NOT RUN: original resource owner missing"))
    for name, requirements, expected, action in [
        ("production-payload", ["R33", "R45", "REQ-T23"], "Current local frontend, system-only dependencies, no Android materials/dev paths/test bridge", lambda: payload(*bound)),
        ("archive-roundtrip", ["U04", "R33", "R45", "REQ-T23"], "ZIP and read-only DMG payloads/signatures roundtrip; actual predecessor preserved", lambda: archives(run, *bound)),
        ("actual-signing-status", ["U04", "R45", "REQ-T23"], "Complete ad-hoc seal; actual Gatekeeper/stapler status; explicit unsigned release tasks", lambda: signing(run, *bound)),
        ("clean-macos-lifecycle", ["U04", "R30", "R50", "REQ-T23"], "Clean target/default security/current installer initialize/play/upgrade/data-preserving uninstall/explicit cleanup", lambda: lifecycle(run, *bound))]:
        run.case(name, requirements, expected, lambda action=action: action() if bound else require(False, "NOT RUN: current installer/build receipt missing"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
