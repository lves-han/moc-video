"""Independent corrupt/missing installer and clean-target evidence counterexamples."""
import copy
import datetime as dt
import json
from pathlib import Path
import plistlib
import sqlite3
import struct
import tempfile
import unittest
from unittest import mock
import zipfile

import t23


def lifecycle_fixture():
    manifest = dict(source="source", protected="baseline", utc="2026-11-07T00:00:00+00:00",
        files={"current-executable": {"sha256": "current"}}, upgrade_from=dict(files={"old-executable": {"sha256": "old"}}))
    settings = dict(schema_version=2, source_configs=[dict(kind="maccms", config_id="controlled-source"),
                    dict(kind="stremio", config_id="controlled-metadata")],
                    volume=.5, ui_locale="zh-CN", window_bounds=None)
    history = [dict(content_id="controlled-content", position_ms=7000, title="Owned media")]
    phases = dict(baseline=dict(app_exists=False, data_exists=False),
        installed=dict(destination="/Applications/墨客视频.app", app_files=manifest["files"]),
        initialized=dict(window_visible=True, ready=True, settings=settings),
        first_play=dict(samples=[dict(elapsed_seconds=0, position_seconds=1), dict(elapsed_seconds=6, position_seconds=7)],
                        moving_picture=True, audio_audible=True, errors=[]),
        upgrade_before=dict(app_files=manifest["upgrade_from"]["files"], settings=settings, history=history, ready=True, database_version=1),
        upgrade_after=dict(app_files=manifest["files"], settings=settings, history=history, ready=True, window_visible=True, database_version=2),
        uninstalled=dict(app_exists=False, data_exists=True, settings=settings, history=history, database_version=2),
        data_deleted=dict(app_exists=False, data_exists=False, explicit_test_data_deletion=True, remaining_owned_processes=[]))
    for index, row in enumerate(phases.values()):
        if "settings" in row:
            row["settings"] = copy.deepcopy(row["settings"])
        row.update(utc=(dt.datetime(2026, 11, 7, tzinfo=dt.timezone.utc) + dt.timedelta(seconds=10 * (index + 1))).isoformat(), evidence=["phase-capture"])
    value = dict(manifest_sha256="manifest", source="source", protected="baseline", target="macos-arm64", macos="14.0",
        clean_system_method="fresh-macos-install", human_observer="test fixture observer", security_settings_changed=False,
        quarantine_removed=False, security_exceptions=[], phases=copy.deepcopy(phases),
        security_commands=[dict(argv=["/usr/sbin/spctl", "--status"], exit=0, output="assessments enabled"),
            dict(argv=["/usr/bin/csrutil", "status"], exit=0, output="System Integrity Protection status: enabled.")])
    return manifest, value


class EvidenceTests(unittest.TestCase):
    def test_final_installer_and_original_build_receipt_belong_to_t036(self):
        with tempfile.TemporaryDirectory(prefix="t25-build-binding-", dir=t23.ROOT / "evidence") as temporary:
            root = Path(temporary)
            manifest_path = root / "evidence/releases/final/manifest.json"
            manifest_path.parent.mkdir(parents=True)
            pointer = root / "evidence/releases/T23-current.json"
            receipt_path = root / "evidence/runs/final-diag-t23-release-build/receipt.json"
            receipt_path.parent.mkdir(parents=True)
            owner_path = root / "evidence/runs/owner/receipt.json"
            owner_path.parent.mkdir(parents=True)
            owner_path.write_text(json.dumps(dict(protected="baseline", generation=0)))
            log = receipt_path.with_name("output.log")
            manifest = dict(schema_version=2, card="T0.36", source="source", protected="baseline", target="macos-arm64",
                scope="personal use only; not public distribution", commands=[], logical_bytes=1, allocated_bytes=512, archives=[])
            def write_manifest():
                manifest_path.write_text(json.dumps(manifest))
                digest = t23.file_hash(manifest_path)
                pointer.write_text(json.dumps(dict(manifest=manifest_path.relative_to(root).as_posix(), sha256=digest)))
                log.write_text("T23_MANIFEST_SHA256:" + digest)
            write_manifest()
            receipt = dict(status="finished", exit=0, raw_exit=0, source="source", after_source="source", protected="baseline",
                generation=0, card="T0.36", log=log.relative_to(root).as_posix(), log_hash=t23.file_hash(log),
                argv=["{python}", "-B", "packaging/release.py", "--build"],
                environment=dict(memory_mib=4096, jobs=2, test_threads=1, memory_enforcement="rss-sampled"))
            receipt_path.write_text(json.dumps(receipt))
            run = mock.Mock(CARD="T0.36", source="source", observed=dict(resources=dict(receipt=owner_path.relative_to(root).as_posix())))
            with mock.patch.object(t23, "ROOT", root), mock.patch.object(t23.release, "CURRENT", pointer), \
                    mock.patch.object(t23.workflow, "protected_hash", return_value="baseline"):
                self.assertEqual(t23.artifact(run)[0]["card"], "T0.36")
                receipt["card"] = "T0.27"
                receipt_path.write_text(json.dumps(receipt))
                with self.assertRaisesRegex(ValueError, "Original successful build receipt"):
                    t23.artifact(run)
                manifest["card"] = "T0.27"
                write_manifest()
                with self.assertRaisesRegex(ValueError, "source/baseline is stale"):
                    t23.artifact(run)

    def test_actual_ad_hoc_seal_keeps_apple_distribution_pending(self):
        detail = "Signature=adhoc\nTeamIdentifier=not set\nSealed Resources version=2 rules=13 files=2\n"
        result = t23.release.check_signing(detail, 0, 1, 65)
        self.assertFalse(result["developer_id_signed"])
        self.assertFalse(result["notarized"])
        self.assertFalse(result["gatekeeper_accepted"])
        self.assertTrue(result["sealed_resources"])

    def test_unsealed_or_overclaimed_signing_fails(self):
        detail = "Signature=adhoc\nTeamIdentifier=not set\nSealed Resources version=2 rules=13 files=2\n"
        broken = [(detail.replace("Sealed Resources version=2 rules=13 files=2", "Sealed Resources=none"), 0, 1, 65),
                  (detail + "Authority=Developer ID Application\n", 0, 1, 65), (detail, 1, 1, 65),
                  (detail, 0, 0, 65), (detail, 0, 1, 0), (detail, 0, None, 65), (detail, False, 1, 65)]
        for row in broken:
            with self.subTest(row=row), self.assertRaises(ValueError):
                t23.release.check_signing(*row)

    def test_lifecycle_preserves_real_upgrade_data(self):
        manifest, value = lifecycle_fixture()
        self.assertEqual(t23.check_lifecycle(value, manifest, "manifest")["history_rows"], 1)
        value["result"] = "FAIL"  # Saved verdicts have no effect on raw assertions.
        self.assertEqual(t23.check_lifecycle(value, manifest, "manifest")["first_play_samples"], 2)

    def test_other_installer_baseline_target_or_developer_home_fails(self):
        manifest, good = lifecycle_fixture()
        for key, wrong in [("manifest_sha256", "stale"), ("source", "stale"), ("protected", "stale"),
                           ("target", "macos-x86_64"), ("macos", "13.0"), ("human_observer", ""),
                           ("clean_system_method", "isolated-HOME"), ("clean_system_method", "existing-developer-machine")]:
            value = copy.deepcopy(good)
            value[key] = wrong
            with self.subTest(key=key, wrong=wrong), self.assertRaises(ValueError):
                t23.check_lifecycle(value, manifest, "manifest")

    def test_upgrade_accepts_only_the_documented_migration_differences(self):
        manifest, value = lifecycle_fixture()
        value["phases"]["upgrade_after"]["settings"]["metadata_preset_applied"] = True
        value["phases"]["uninstalled"]["settings"]["metadata_preset_applied"] = True
        value["phases"]["upgrade_after"]["settings"]["window_bounds"] = [0, 0, 1200, 800]
        value["phases"]["uninstalled"]["settings"]["window_bounds"] = [0, 0, 1200, 800]
        self.assertEqual(t23.check_lifecycle(value, manifest, "manifest")["history_rows"], 1)
        for phase, version in [("upgrade_before", 2), ("upgrade_after", 1), ("uninstalled", 1)]:
            wrong = copy.deepcopy(value)
            wrong["phases"][phase]["database_version"] = version
            with self.subTest(phase=phase), self.assertRaisesRegex(ValueError, "version 1 to 2"):
                t23.check_lifecycle(wrong, manifest, "manifest")
        value["phases"]["upgrade_before"]["settings"]["source_configs"] = [dict(kind="maccms")]
        with self.assertRaisesRegex(ValueError, "already contain Stremio"):
            t23.check_lifecycle(value, manifest, "manifest")

    def test_default_security_or_quarantine_bypass_fails(self):
        manifest, good = lifecycle_fixture()
        for key, wrong in [("security_settings_changed", True), ("quarantine_removed", True),
                           ("security_exceptions", ["Open Anyway"]), ("security_commands", [])]:
            value = copy.deepcopy(good)
            value[key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError):
                t23.check_lifecycle(value, manifest, "manifest")

    def test_missing_or_stale_lifecycle_phase_fails_despite_pass_flag(self):
        manifest, good = lifecycle_fixture()
        for name in good["phases"]:
            value = copy.deepcopy(good)
            value["result"] = "PASS"
            del value["phases"][name]
            with self.subTest(name=name), self.assertRaises(ValueError):
                t23.check_lifecycle(value, manifest, "manifest")
        for wrong in ("2026-11-07T00:00:00",):
            value = copy.deepcopy(good)
            value["phases"]["first_play"]["utc"] = wrong
            with self.subTest(stamp=wrong), self.assertRaises(ValueError):
                t23.check_lifecycle(value, manifest, "manifest")

    def test_stalled_silent_short_or_nonfinite_first_play_fails(self):
        manifest, good = lifecycle_fixture()
        changes = [dict(samples=[]), dict(samples=[dict(elapsed_seconds=0, position_seconds=0)]),
                   dict(samples=[dict(elapsed_seconds=float("nan"), position_seconds=7)]),
                   dict(audio_audible=False), dict(moving_picture=False), dict(errors=["playback failed"])]
        for change in changes:
            value = copy.deepcopy(good)
            value["phases"]["first_play"].update(change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                t23.check_lifecycle(value, manifest, "manifest")

    def test_same_build_upgrade_or_empty_lost_data_fails(self):
        manifest, good = lifecycle_fixture()
        changes = [("upgrade_before", "app_files", manifest["files"]), ("upgrade_after", "app_files", manifest["upgrade_from"]["files"]),
                   ("upgrade_before", "settings", dict(schema_version=2, source_configs=[])),
                   ("upgrade_before", "history", []), ("upgrade_after", "history", []),
                   ("upgrade_after", "settings", dict(schema_version=2, source_configs=["different"]))]
        for phase, key, wrong in changes:
            value = copy.deepcopy(good)
            value["phases"][phase][key] = wrong
            with self.subTest(phase=phase, key=key), self.assertRaises(ValueError):
                t23.check_lifecycle(value, manifest, "manifest")

    def test_removal_losing_user_data_or_leaving_processes_fails(self):
        manifest, good = lifecycle_fixture()
        for phase, key, wrong in [("uninstalled", "app_exists", True), ("uninstalled", "data_exists", False),
                                  ("uninstalled", "history", []), ("data_deleted", "data_exists", True),
                                  ("data_deleted", "explicit_test_data_deletion", False), ("data_deleted", "remaining_owned_processes", [123])]:
            value = copy.deepcopy(good)
            value["phases"][phase][key] = wrong
            with self.subTest(phase=phase, key=key), self.assertRaises(ValueError):
                t23.check_lifecycle(value, manifest, "manifest")

    def test_raw_phase_evidence_is_required(self):
        manifest, value = lifecycle_fixture()
        value["phases"]["initialized"]["evidence"] = []
        with self.assertRaises(ValueError):
            t23.check_lifecycle(value, manifest, "manifest")

    def test_upgrade_recovers_observed_immutable_predecessor_startup_failure(self):
        manifest, value = lifecycle_fixture()
        before = value["phases"]["upgrade_before"]
        before["ready"] = False
        with self.assertRaises(ValueError):
            t23.check_lifecycle(value, manifest, "manifest")
        before.update(window_visible=True,
                      startup_failure="runtime checking; missing startup preparation in immutable build 1",
                      startup_evidence=before["evidence"][0])
        self.assertEqual(t23.check_lifecycle(value, manifest, "manifest")["history_rows"], 1)
        value["phases"]["upgrade_after"]["ready"] = False
        with self.assertRaises(ValueError):
            t23.check_lifecycle(value, manifest, "manifest")

    def test_lifecycle_reopens_raw_settings_and_sqlite_instead_of_report_values(self):
        # This checks the importer with synthetic data. The camera/codec probes
        # are mocked here; actual clean-machine playback remains mandatory A1.
        with tempfile.TemporaryDirectory(prefix="t23-raw-", dir=t23.ROOT / "evidence") as temporary:
            root = Path(temporary)
            folder = root / "evidence/releases/T23-clean-macos"
            folder.mkdir(parents=True)
            manifest, value = lifecycle_fixture()
            manifest_path = root / "manifest.json"
            manifest_path.write_text(json.dumps(manifest))
            value["manifest_sha256"] = t23.file_hash(manifest_path)
            artifacts = {}
            def material(name, data):
                path = folder / name
                path.write_bytes(data)
                artifacts[name] = dict(path=str(path.relative_to(root)), sha256=t23.file_hash(path))
                return name
            material("phase-capture", b"synthetic observation fixture")
            value["recording"] = material("recording.mp4", b"synthetic movie; probes mocked")
            value["phases"]["first_play"]["evidence"].append(value["recording"])
            confirmation = material("user-audio.txt", "synthetic user confirmation: 能听到正常声音".encode())
            value["phases"]["first_play"]["audio_confirmation"] = confirmation
            value["phases"]["first_play"]["evidence"].append(confirmation)
            for index, command in enumerate(value["security_commands"]):
                command["evidence"] = material(f"security-{index}.txt", command.pop("output").encode())
            for name in ("installed", "upgrade_before", "upgrade_after"):
                phase = value["phases"][name]
                phase["file_tree"] = material(name + "-tree.json", json.dumps(phase.pop("app_files")).encode())
                phase["evidence"].append(phase["file_tree"])
            for name in ("initialized", "upgrade_before", "upgrade_after", "uninstalled"):
                phase = value["phases"][name]
                phase["settings_file"] = material(name + "-settings.json", json.dumps(phase.pop("settings")).encode())
                phase["evidence"].append(phase["settings_file"])
                phase["settings"] = {"fake": "PASS"}
                if name != "initialized":
                    database = folder / (name + ".db")
                    connection = sqlite3.connect(database)
                    try:
                        connection.execute("PRAGMA user_version=" + str(1 if name == "upgrade_before" else 2))
                        connection.execute("CREATE TABLE watch_history(content_id TEXT, position_ms INTEGER, title TEXT)")
                        connection.execute("INSERT INTO watch_history VALUES (?, ?, ?)", ("controlled-content", 7000, "Owned media"))
                        connection.commit()
                    finally:
                        connection.close()
                    artifacts[database.name] = dict(path=str(database.relative_to(root)), sha256=t23.file_hash(database))
                    phase["history_db"] = database.name
                    phase["evidence"].append(database.name)
                    phase["history"] = []
            value["artifacts"] = artifacts
            report = folder / "observations.json"
            report.write_text(json.dumps(value))
            run = mock.Mock(observed={}, folder=folder)
            def media_probe(argv, **_):
                if argv[0] == "ffprobe":
                    return json.dumps(dict(format=dict(duration="6"), streams=[dict(codec_type="video", nb_read_frames="180"), dict(codec_type="audio")]))
                self.assertEqual(argv[0], "ffmpeg")
                Path(argv[-1]).write_bytes(struct.pack("<ff", .1, -.1))
                return ""
            run.command.side_effect = media_probe
            with mock.patch.object(t23, "ROOT", root):
                self.assertEqual(t23.lifecycle(run, manifest, manifest_path)["history_rows"], 1)
                after = folder / value["phases"]["upgrade_after"]["settings_file"]
                settings = json.loads(after.read_text())
                settings["volume"] = .9
                after.write_text(json.dumps(settings))
                artifacts[after.name]["sha256"] = t23.file_hash(after)
                report.write_text(json.dumps(value))
                with self.assertRaisesRegex(ValueError, "lost persisted settings"):
                    t23.lifecycle(run, manifest, manifest_path)

    def test_empty_suite_cannot_pass_through_source_stability(self):
        with tempfile.TemporaryDirectory(prefix="t23-empty-", dir=t23.ROOT / "evidence") as temporary:
            run = t23.Acceptance.__new__(t23.Acceptance)
            run.folder = Path(temporary)
            run.report_path, run.junit_path = run.folder / "report.json", run.folder / "report.xml"
            run.source, run.started = "source", "2026-11-07T00:00:00+00:00"
            run.cases, run.commands, run.observed = [], [], {}
            with mock.patch.object(t23.workflow, "snapshot", return_value="source"), mock.patch.object(t23.workflow, "protected_hash", return_value="protected"):
                self.assertEqual(run.save(), 1)
            report = t23.read_json(run.report_path)
            self.assertTrue(all(row["result"] == "FAIL" for row in report["summaries"]))
            self.assertEqual({row["id"] for row in report["summaries"]}, set(t23.IDS))

    def test_package_suite_excludes_lifecycle_and_lifecycle_requires_it(self):
        package, lifecycle = t23.SUITES["package"], t23.SUITES["lifecycle"]
        self.assertNotIn("clean-macos-lifecycle", package["behaviors"])
        self.assertIn("clean-macos-lifecycle", lifecycle["behaviors"])
        self.assertEqual(package["requirements"], ["U04", "R33", "R45", "REQ-T23"])
        self.assertEqual(lifecycle["requirements"], ["U04", "R30", "R50", "REQ-T25-LIFECYCLE"])
        self.assertEqual((package["card"], lifecycle["card"]), ("T0.27", "T0.36"))

    def test_report_and_junit_name_the_suite_that_ran(self):
        with tempfile.TemporaryDirectory(prefix="t23-suite-", dir=t23.ROOT / "evidence") as temporary:
            run = t23.Acceptance.__new__(t23.Acceptance)
            run.folder = Path(temporary)
            run.report_path, run.junit_path = run.folder / "out.json", run.folder / "out.xml"
            run.source, run.started = "source", "2026-11-07T00:00:00+00:00"
            run.cases, run.commands, run.observed = [], [], {}
            with mock.patch.object(t23.workflow, "snapshot", return_value="source"), mock.patch.object(t23.workflow, "protected_hash", return_value="protected"):
                run.save()
            self.assertEqual(t23.read_json(run.report_path)["suite"], "package")
            self.assertIn('name="T23-package"', run.junit_path.read_text(encoding="utf-8"))

    def test_relative_report_arguments_resolve_from_project_root(self):
        report, junit = Path("evidence/acceptance/T23-all.json"), Path("evidence/acceptance/T23-all.xml")
        run = t23.Acceptance(report, junit)
        self.addCleanup(run.folder.rmdir)
        self.assertEqual(run.report_path.relative_to(t23.ROOT), report)
        self.assertEqual(run.junit_path.relative_to(t23.ROOT), junit)

    def test_traversal_and_absolute_artifact_paths_fail(self):
        for name in ("../outside", "/tmp/outside", "evidence/../package.json", "package.json"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                t23.inside(name, t23.ROOT / "evidence/releases")

    def test_symlink_and_executable_mode_change_are_detected(self):
        with tempfile.TemporaryDirectory(prefix="t23-tree-", dir=t23.ROOT / "evidence") as temporary:
            folder = Path(temporary)
            binary = folder / "binary"
            binary.write_bytes(b"product")
            binary.chmod(0o755)
            initial = t23.release.tree(folder)
            binary.chmod(0o644)
            self.assertNotEqual(initial, t23.release.tree(folder))
            (folder / "external").symlink_to(t23.ROOT / "package.json")
            with self.assertRaises(ValueError):
                t23.release.tree(folder)

    def test_zip_bytes_crc_and_traversal_are_rechecked(self):
        with tempfile.TemporaryDirectory(prefix="t23-zip-", dir=t23.ROOT / "evidence") as temporary:
            archive = Path(temporary) / "app.zip"
            with zipfile.ZipFile(archive, "w") as stream:
                stream.writestr("Product.app/Contents/binary", b"product")
            t23.check_zip(archive, 7)
            with self.assertRaises(ValueError):
                t23.check_zip(archive, 8)
            for name in ("../escape", "/absolute", "folder\\escape"):
                with zipfile.ZipFile(archive, "w") as stream:
                    stream.writestr(name, b"product")
                with self.subTest(name=name), self.assertRaises(ValueError):
                    t23.check_zip(archive, 7)

    def test_production_payload_excludes_cdn_android_and_test_bridge(self):
        with tempfile.TemporaryDirectory(prefix="t23-payload-", dir=t23.ROOT / "evidence") as temporary:
            root = Path(temporary)
            app, frontend, licenses = root / "Product.app", root / "frontend", root / "licenses"
            executable = app / "Contents/MacOS/moc-video"
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b"real product executable")
            executable.chmod(0o755)
            frontend.mkdir()
            (frontend / "index.html").write_text('<script type="module" src="./assets.js"></script>')
            (frontend / "assets.js").write_text("console.log('product')")
            (frontend / "art.webp").write_bytes(b"image")
            licenses.mkdir()
            for name in ("dependencies.json", "THIRD_PARTY_NOTICES.txt"):
                (licenses / name).write_text("license")
                target = app / "Contents/Resources/licenses" / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("license")
            seal = app / "Contents/_CodeSignature/CodeResources"
            seal.parent.mkdir()
            seal.write_text("seal")
            config = dict(identifier="app.yingxiang.video", version="0.1.0", bundle=dict(macOS=dict(bundleVersion="2")))
            (root / "src-tauri/icons").mkdir(parents=True)
            (root / "src-tauri/tauri.conf.json").write_text(json.dumps(config))
            (root / "src-tauri/icons/icon.icns").write_bytes(b"project icon")
            (app / "Contents/Resources/icon.icns").write_bytes(b"project icon")
            info = dict(CFBundleIdentifier="app.yingxiang.video", CFBundleShortVersionString="0.1.0", CFBundleVersion="2",
                CFBundleExecutable="moc-video", LSMinimumSystemVersion="14.0", NSHighResolutionCapable=True,
                NSAppTransportSecurity=dict(NSAllowsLocalNetworking=True), CFBundleIconFile="icon.icns")
            (app / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
            with mock.patch.object(t23.release, "ROOT", root), mock.patch.object(t23.release, "INVENTORY", licenses / "dependencies.json"), mock.patch.object(t23.release, "NOTICES", licenses / "THIRD_PARTY_NOTICES.txt"):
                self.assertEqual(t23.release.check_payload(app, frontend)["bundle_version"], "2")
                (app / "Contents/Resources/icon.icns").write_bytes(b"other icon")
                with self.assertRaises(ValueError):
                    t23.release.check_payload(app, frontend)
                (app / "Contents/Resources/icon.icns").write_bytes(b"project icon")
                for text in ('<script src="https://cdn.invalid/app.js"></script>', '<link rel="stylesheet" href="https://cdn.invalid/app.css">'):
                    (frontend / "index.html").write_text(text)
                    with self.subTest(text=text), self.assertRaises(ValueError):
                        t23.release.check_payload(app, frontend)
                (frontend / "index.html").write_text('<script src="./assets.js"></script>')
                (app / "Contents/Resources/engine.apk").write_bytes(b"forbidden")
                with self.assertRaises(ValueError):
                    t23.release.check_payload(app, frontend)
                (app / "Contents/Resources/engine.apk").unlink()
                for data in (b"T01_FIXTURE_CAP", str(root).encode() + b"/dev/path"):
                    executable.write_bytes(data)
                    with self.subTest(data=data), self.assertRaises(ValueError):
                        t23.release.check_payload(app, frontend)


if __name__ == "__main__":
    unittest.main()
