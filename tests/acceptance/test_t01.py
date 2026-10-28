"""Behavioral regressions for the fixture and evidence validator, run under run.py."""
import copy
import importlib.util
import json
from pathlib import Path
import queue
import tempfile
import unittest
from unittest import mock
import xml.etree.ElementTree as ET

spec = importlib.util.spec_from_file_location("t01", Path(__file__).with_name("t01.py"))
t01 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t01)


class RangeTests(unittest.TestCase):
    def test_full_and_partial_ranges(self):
        for value, expected in ((None, None), ("bytes=0-15", (0, 15)), ("bytes=10-", (10, 99)),
                                ("bytes=-10", (90, 99)), ("bytes=-1000", (0, 99)), ("bytes=90-1000", (90, 99))):
            with self.subTest(value=value):
                self.assertEqual(t01.server_module.byte_range(value, 100), expected)

    def test_unsatisfiable_malformed_and_multiple_ranges(self):
        for value in ("bytes=100-", "bytes=8-3", "bytes=-0", "bytes=", "bytes=-", "items=0-1", "bytes=0-1,3-4"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                t01.server_module.byte_range(value, 100)
        with self.assertRaises(ValueError):
            t01.server_module.byte_range("bytes=0-", 0)


class FixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="t01-fixture-regression-")
        self.addCleanup(self.temp.cleanup)
        media = Path(self.temp.name)
        self.data = bytes(range(256)) * 1000
        (media / "clip.mp4").write_bytes(self.data)
        (media / "segment-000.ts").write_bytes(b"synthetic segment")
        (media / "index.m3u8").write_text("#EXTM3U\n#EXTINF:2,\nsegment-000.ts\n#EXT-X-ENDLIST\n")
        (media / "subtitles.vtt").write_text("WEBVTT\n")
        (media / "secret.json").write_text("not registered")
        self.fixture = self.enterContext(t01.Fixture(media))
        self.path = "/media/clip.mp4?cap=" + self.fixture.cap

    def request(self, method="GET", path=None, **headers):
        return t01.http_request(self.fixture, method, path or self.path, headers)

    def test_cap_required_even_without_origin(self):
        self.assertEqual(self.request(path="/media/clip.mp4")[0], 403)
        self.assertEqual(self.request(path="/media/clip.mp4?cap=bad")[0], 403)
        self.assertEqual(self.request(path=self.path + "&cap=bad")[0], 403)
        self.assertEqual(self.request(path="/media/clip.mp4?cap=%E4%B8%AD")[0], 403)
        self.assertEqual(self.request()[2], self.data)

    def test_fixed_origin_and_host(self):
        for origin in ("https://evil.invalid", "null", "http://127.0.0.1", "tauri://evil"):
            with self.subTest(origin=origin):
                status, headers, _ = self.request(Origin=origin)
                self.assertEqual(status, 403)
                self.assertNotIn("Access-Control-Allow-Origin", headers)
        self.assertEqual(self.request(Host="evil.invalid")[0], 421)
        status, headers, _ = self.request(Origin="tauri://localhost", Range="bytes=0-15")
        self.assertEqual(status, 206)
        self.assertEqual(headers["Access-Control-Allow-Origin"], "tauri://localhost")
        self.assertNotIn("Access-Control-Allow-Credentials", headers)

    def test_range_head_and_if_range(self):
        status, headers, body = self.request(Range="bytes=17-64")
        self.assertEqual((status, body), (206, self.data[17:65]))
        self.assertEqual(headers["Content-Range"], f"bytes 17-64/{len(self.data)}")
        self.assertEqual(self.request(Range="bytes=-20")[2], self.data[-20:])
        self.assertEqual(self.request(Range="bytes=17-64", **{"If-Range": headers["ETag"]})[0], 206)
        self.assertEqual(self.request(Range="bytes=17-64", **{"If-Range": '"old"'})[2], self.data)
        status, headers, body = self.request("HEAD")
        self.assertEqual((status, body, int(headers["Content-Length"])), (200, b"", len(self.data)))

    def test_416_has_unsatisfied_content_range(self):
        status, headers, _ = self.request(Range="bytes=999999999-")
        self.assertEqual(status, 416)
        self.assertEqual(headers["Content-Range"], f"bytes */{len(self.data)}")

    def test_preflight_is_metadata_only(self):
        status, headers, body = self.request("OPTIONS", "/media/clip.mp4", Origin="tauri://localhost",
            **{"Access-Control-Request-Method": "GET", "Access-Control-Request-Headers": "range, if-range"})
        self.assertEqual((status, body), (204, b""))
        self.assertEqual(headers["Access-Control-Allow-Origin"], "tauri://localhost")
        self.assertEqual(self.request("OPTIONS", "/media/clip.mp4", Origin="tauri://localhost",
            **{"Access-Control-Request-Method": "POST"})[0], 403)
        self.assertEqual(self.request("OPTIONS", "/media/clip.mp4", Origin="tauri://localhost",
            **{"Access-Control-Request-Method": "GET", "Access-Control-Request-Headers": "authorization"})[0], 403)
        self.assertEqual(self.request("GET", "/media/clip.mp4")[0], 403)

    def test_registered_files_only_and_no_media_mutation(self):
        for path in ("/media/../secret.json", "/media/secret.json", "/media/%2e%2e/clip.mp4"):
            self.assertEqual(self.request(path=path + "?cap=" + self.fixture.cap)[0], 404)
        self.assertEqual(self.request("POST")[0], 405)
        self.assertEqual(self.request()[2], self.data)

    def test_hls_children_have_same_cap_and_require_it(self):
        status, _, body = self.request(path="/media/index.m3u8?cap=" + self.fixture.cap)
        self.assertEqual(status, 200)
        uri = next(line for line in body.decode().splitlines() if line and not line.startswith("#"))
        self.assertEqual(uri, "segment-000.ts?cap=" + self.fixture.cap)
        self.assertEqual(self.request(path="/media/" + uri)[0], 200)
        self.assertEqual(self.request(path="/media/segment-000.ts")[0], 403)

    def test_request_records_redact_cap(self):
        self.request(Range="bytes=0-1")
        self.assertNotIn(self.fixture.cap, str(self.fixture.summary()))


class GuiSessionTests(unittest.TestCase):
    def test_locked_desktop_fails_before_launching_native_app(self):
        state = mock.Mock(returncode=0, stdout='"CGSSessionScreenIsLocked" = Yes')
        run = mock.Mock(observed={})
        with mock.patch.object(t01.subprocess, "run", return_value=state), \
             mock.patch.object(t01, "NativeSession") as native:
            with self.assertRaisesRegex(ValueError, "unlock the desktop"):
                t01.native_all(run, None, {})
            native.assert_not_called()
        self.assertEqual(run.observed["gui_session"], dict(screen_locked=True))

    def test_unlocked_desktop_is_recorded(self):
        state = mock.Mock(returncode=0, stdout='"CGSSessionScreenIsLocked" = No')
        with mock.patch.object(t01.subprocess, "run", return_value=state):
            self.assertEqual(t01.native_gui_session(), dict(screen_locked=False))

    def test_missing_lock_flag_remains_unknown(self):
        with mock.patch.object(t01.subprocess, "run", return_value=mock.Mock(returncode=0, stdout="")):
            self.assertIsNone(t01.native_gui_session()["screen_locked"])

    def test_state_read_failure_is_explicit(self):
        with mock.patch.object(t01.subprocess, "run", return_value=mock.Mock(returncode=1, stdout="")):
            with self.assertRaisesRegex(ValueError, "could not be read"):
                t01.native_gui_session()


class FunctionalStageTests(unittest.TestCase):
    def observations(self):
        first = dict(frames=10, raf=20, width=1920, height=1080)
        current = dict(width=1920, height=1080, paused=False, media_error=None, frames=50, raf=90,
            audio_state="running", audio_rms=0.06, audio_peak_hz=440, ui_position=5.5, position=5.7, duration=30)
        return dict(sample=first), dict(sample=current)

    def test_functional_observation_uses_real_control_results_without_fixed_duration(self):
        opened, controlled = self.observations()
        observed = t01.validate_functional(opened, controlled)
        self.assertEqual(observed["presented_frames"], 40)
        self.assertEqual(observed["ui_callbacks"], 70)

    def test_functional_observation_rejects_broken_video_audio_and_ui(self):
        for changes in (dict(frames=10), dict(width=1280), dict(paused=True), dict(media_error=3),
                        dict(audio_rms=0), dict(audio_state="suspended"), dict(audio_peak_hz=100),
                        dict(raf=20), dict(ui_position=1), dict(ui_position=float("nan"))):
            with self.subTest(changes=changes):
                opened, controlled = self.observations()
                controlled["sample"].update(changes)
                with self.assertRaises(ValueError):
                    t01.validate_functional(opened, controlled)

    def exercise_native(self, endurance):
        run = mock.Mock(observed={"native_sessions": []})
        run.case.side_effect = lambda _name, _ids, _expected, check: check() or True
        sessions = []
        opened, controlled = self.observations()
        opened["sample"]["audio_rms"] = 0.06
        def native(*_args):
            session = mock.Mock(samples=[])
            session.process.pid = 1234
            session.record = dict(exit=0, lingering=[])
            run.observed["native_sessions"].append(session.record)
            def command(op, **_kwargs):
                if op == "identity":
                    return dict(native=dict(pid=1234, label="main", tauri="2.12.5"), origin="tauri://localhost")
                if op == "open": return copy.deepcopy(opened)
                if op == "controls": return copy.deepcopy(controlled)
                return {}
            session.command.side_effect = command
            sessions.append(session)
            return session
        with mock.patch.object(t01, "native_gui_session", return_value=dict(screen_locked=False)), \
             mock.patch.object(t01, "NativeSession", side_effect=native), \
             mock.patch.object(t01, "validate_window"), mock.patch.object(t01, "validate_long"):
            value = t01.native_all(run, mock.Mock(), dict(automation=dict(path="fixture.app")), endurance=endurance)
        return run, sessions, value

    def test_development_stage_uses_one_window_and_defers_endurance_explicitly(self):
        run, sessions, value = self.exercise_native(False)
        self.assertEqual(len(sessions), 1)
        ops = [call.args[0] for call in sessions[0].command.call_args_list]
        self.assertNotIn("long", ops)
        self.assertEqual(ops.count("open"), 2)
        self.assertEqual(ops.count("controls"), 2)
        self.assertEqual(ops.count("stop"), 2)
        self.assertEqual(ops.count("layout"), 5)
        self.assertEqual(ops.count("zoom"), 4)
        sessions[0].close.assert_called_once_with(graceful=True)
        pending = run.observed["product_test_handoff"]
        self.assertEqual((pending["owner"], pending["status"], pending["seconds_per_format"], pending["full_window_cycles"]),
                         ("T0.28/A1", "NOT RUN", 1800, 20))
        self.assertEqual(value["functional_formats"], 2)

    def test_product_endurance_keeps_both_1800_second_runs_and_twenty_cycles(self):
        run, sessions, value = self.exercise_native(True)
        self.assertEqual(len(sessions), 21)
        longs = [call for call in sessions[0].command.call_args_list if call.args == ("long",)]
        self.assertEqual(len(longs), 2)
        self.assertTrue(all(call.kwargs == dict(timeout=1900, seconds=1800) for call in longs))
        self.assertEqual(value, dict(long_formats=2, full_window_cycles=20, sessions=21))
        self.assertNotIn("product_test_handoff", run.observed)

class NativeLaunchTests(unittest.TestCase):
    def test_macos_launch_uses_process_scoped_restoration_override(self):
        with mock.patch.object(t01.sys, "platform", "darwin"):
            self.assertEqual(t01.NativeSession.launch_argv(Path("/fixture/test app/native")),
                             ["/fixture/test app/native", "-ApplePersistenceIgnoreState", "YES"])

    def test_other_platform_launch_keeps_original_arguments(self):
        with mock.patch.object(t01.sys, "platform", "linux"):
            self.assertEqual(t01.NativeSession.launch_argv(Path("/fixture/native")), ["/fixture/native"])


class NativeResultTests(unittest.TestCase):
    def setUp(self):
        root = Path(self.enterContext(tempfile.TemporaryDirectory(prefix="t01-result-regression-")))
        self.enterContext(mock.patch.object(t01, "ROOT", root))
        self.session = t01.NativeSession.__new__(t01.NativeSession)
        self.session.run = mock.Mock(folder=root)
        self.session.fixture = mock.Mock(results=queue.Queue())
        self.session.record = dict(index=0, log="synthetic-native.log")
        self.session.observe = mock.Mock()
        self.session.drain_samples = mock.Mock()

    def test_wake_guard_is_bound_to_and_accounted_with_native_session(self):
        self.session.process = mock.Mock(pid=1234)
        self.session.owned = {}
        wake = mock.Mock(pid=5678)
        wake.poll.return_value = None
        with mock.patch.object(t01.subprocess, "Popen", return_value=wake) as launch, \
             mock.patch.object(t01.workflow.runtime, "identity", return_value="created:fixture"):
            self.session.start_wake_guard()
        self.assertEqual(launch.call_args.args[0], ["/usr/bin/caffeinate", "-d", "-i", "-w", "1234"])
        self.assertEqual(self.session.owned, {5678: "created:fixture"})
        self.assertEqual(self.session.record["wake_guard"]["identity"], "created:fixture")

    def test_wake_guard_start_failure_is_explicit(self):
        self.session.process = mock.Mock(pid=1234)
        self.session.owned = {}
        wake = mock.Mock(pid=5678)
        wake.poll.return_value = 1
        with mock.patch.object(t01.subprocess, "Popen", return_value=wake), \
             mock.patch.object(t01.workflow.runtime, "identity", return_value=None):
            with self.assertRaisesRegex(ValueError, "wake guard could not start"):
                self.session.start_wake_guard()
        self.assertEqual(self.session.owned, {})

    def result_log(self):
        path = t01.ROOT / self.session.record["result_log"]
        return [json.loads(line) for line in path.read_text().splitlines()]

    def test_failed_native_snapshot_is_persisted_before_raising(self):
        result = dict(id=5, ok=False, error="Error: Playback or UI stalled",
            diagnostics=dict(long=dict(previous=dict(frames=3527, raf=7103, position=0.0065),
                current=dict(paused=False, media_error=None, frames=3527, raf=7403,
                             visibility="visible", seeking=True, position=0.0065),
                events=[dict(type="seeking", time=122249)]),
                native=dict(visible=True, minimized=False)))
        self.session.fixture.results.put(result)
        with self.assertRaisesRegex(ValueError, "Playback or UI stalled; result: results-0.jsonl"):
            self.session.wait(5, 1)
        rows = self.result_log()
        self.assertEqual(rows[0]["result"], result)
        self.assertGreater(rows[0]["host_received_monotonic"], 0)
        # The final queued sample must also be drained after receiving a result.
        self.assertEqual(self.session.drain_samples.call_count, 2)

    def test_successful_native_results_are_retained_in_command_order(self):
        for identifier in (0, 1):
            self.session.fixture.results.put(dict(id=identifier, ok=True, value=dict(ready=True)))
            self.assertEqual(self.session.wait(identifier, 1), dict(ready=True))
        self.assertEqual([row["result"]["id"] for row in self.result_log()], [0, 1])

    def test_mismatched_result_is_retained_and_still_rejected(self):
        result = dict(id=99, ok=True, value={})
        self.session.fixture.results.put(result)
        with self.assertRaisesRegex(ValueError, "unexpected command/session"):
            self.session.wait(5, 1)
        self.assertEqual(self.result_log()[0]["result"], result)


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.samples = [dict(width=1920, height=1080, paused=False, media_error=None, audio_state="running",
            audio_rms=0.1, audio_peak_hz=440, time=(i + 1) * 5000, frames=(i + 1) * 150,
            raf=(i + 1) * 300, host_received_monotonic=(i + 1) * 5, max_raf_gap_ms=20,
            visibility="visible", focused=True, foreground_interruptions=0,
            native=dict(visible=True, focused=True, minimized=False),
            position=((i + 1) * 5) % 30, ui_position=((i + 1) * 5) % 30, duration=30,
            owned_rss_bytes=1000000) for i in range(360)]
        self.value = dict(duration_seconds=1800.1, media_seconds=1800, samples=360, unique_frames=10,
            foreground=dict(confirmed=dict(visible=True, focused=True, minimized=False)),
            initial=dict(frames=0, dropped_frames=0, foreground_interruptions=0), final=dict(frames=54000, dropped_frames=3))

    def test_valid_long_evidence(self):
        self.assertEqual(t01.validate_long(self.value, self.samples)["samples"], 360)

    def test_short_clock_or_missing_samples_fails(self):
        for change in (dict(duration_seconds=1799), dict(media_seconds=100), dict(samples=0), dict(unique_frames=1)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                t01.validate_long({**self.value, **change}, self.samples)
        with self.assertRaises(ValueError):
            t01.validate_long(self.value, self.samples[:-1])

    def test_silent_wrong_tone_or_stalled_media_fails(self):
        for change in (dict(audio_rms=0), dict(audio_state="suspended"), dict(audio_peak_hz=1000),
                       dict(paused=True), dict(width=1280), dict(media_error=3), dict(frames=0), dict(raf=0)):
            samples = copy.deepcopy(self.samples)
            samples[100].update(change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                t01.validate_long(self.value, samples)

    def test_host_sample_gap_fails(self):
        samples = copy.deepcopy(self.samples)
        samples[100]["host_received_monotonic"] += 20
        with self.assertRaises(ValueError):
            t01.validate_long(self.value, samples)

    def test_unconfirmed_foreground_fails(self):
        for confirmed in ({}, dict(visible=False, focused=True, minimized=False),
                          dict(visible=True, focused=False, minimized=False),
                          dict(visible=True, focused=True, minimized=True)):
            value = copy.deepcopy(self.value)
            value["foreground"]["confirmed"] = confirmed
            with self.subTest(confirmed=confirmed), self.assertRaisesRegex(ValueError, "activation confirmation"):
                t01.validate_long(value, self.samples)

    def test_foreground_loss_or_missing_evidence_fails(self):
        for change in (dict(visibility="hidden"), dict(focused=False), dict(foreground_interruptions=1),
                       dict(foreground_interruptions=None), dict(native={}),
                       dict(native=dict(visible=True, focused=False, minimized=False)),
                       dict(native=dict(visible=False, focused=True, minimized=False)),
                       dict(native=dict(visible=True, focused=True, minimized=True))):
            samples = copy.deepcopy(self.samples)
            samples[100].update(change)
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "foreground condition"):
                t01.validate_long(self.value, samples)

    def test_stalled_wrong_or_missing_progress_ui_fails(self):
        for position in (None, float("nan"), float("inf"), -1, 31, self.samples[100]["position"] + 5):
            samples = copy.deepcopy(self.samples)
            samples[100]["ui_position"] = position
            with self.subTest(position=position), self.assertRaisesRegex(ValueError, "progress UI"):
                t01.validate_long(self.value, samples)
        samples = copy.deepcopy(self.samples)
        # Adjacent near-loop readings can agree with both clocks yet still fail
        # the separate requirement that the rendered progress control advances.
        samples[99]["position"] = samples[100]["position"] = 1
        samples[99]["ui_position"] = samples[100]["ui_position"] = 1
        with self.assertRaisesRegex(ValueError, "progress UI sample sequence stalled"):
            t01.validate_long(self.value, samples)

    def test_production_rejects_test_bridge_and_source_files(self):
        with tempfile.TemporaryDirectory(prefix="t01-production-regression-") as folder:
            root = Path(folder)
            frontend = root / "frontend"
            frontend.mkdir()
            executable = root / "native"
            executable.write_bytes(b"synthetic native")
            asset = frontend / "app.js"
            asset.write_text("console.log('production')")
            t01.validate_production(frontend, executable)
            for marker in (b"T01_FIXTURE_CAP", b"/control/next", b"control/next", b"t01_control", b"_topContentInset"):
                for destination in (executable, asset):
                    executable.write_bytes(b"synthetic native")
                    asset.write_bytes(b"console.log('production')")
                    destination.write_bytes(marker)
                    with self.subTest(marker=marker, destination=destination.name), self.assertRaises(ValueError):
                        t01.validate_production(frontend, executable)
            executable.write_bytes(b"synthetic native")
            asset.write_bytes(b"console.log('production')")
            (frontend / "server.py").write_text("pass")
            with self.assertRaises(ValueError):
                t01.validate_production(frontend, executable)


class WindowGeometryTests(unittest.TestCase):
    def geometry(self, width=1200, scale=2, inset=32, zoom=1):
        return dict(native=dict(physical_width=width * scale, physical_height=800 * scale,
                                scale_factor=scale, top_content_inset=inset),
                    layout=dict(inner_width=round(width / zoom), inner_height=round((800 - inset) / zoom),
                                scroll_width=round(width / zoom)))

    def test_native_frame_and_measured_titlebar_inset_define_viewport(self):
        # Reproduces the recorded 2400x1600 / scale 2 frame and 1200x768 viewport.
        for inset in (0, 28, 32, 38):
            value = self.geometry(inset=inset)
            self.assertIs(t01.validate_window(value), value)

    def test_resized_and_zoomed_viewports_keep_native_dimensions(self):
        for width in (360, 390, 768, 960, 1200):
            for scale in (1, 2):
                for zoom in (1, 1.25, 1.5, 2):
                    with self.subTest(width=width, scale=scale, zoom=zoom):
                        t01.validate_window(self.geometry(width, scale, zoom=zoom), width, zoom)

    def test_wrong_native_size_or_unexplained_viewport_loss_fails(self):
        for section, change in (("native", dict(physical_height=1536)),
                                ("native", dict(physical_width=2398)),
                                ("native", dict(top_content_inset=0)),
                                ("layout", dict(inner_height=800)),
                                ("layout", dict(inner_width=1190)),
                                ("layout", dict(scroll_width=1210))):
            value = self.geometry()
            value[section].update(change)
            with self.subTest(section=section, change=change), self.assertRaises(ValueError):
                t01.validate_window(value)

    def test_invalid_scale_or_inset_fails(self):
        for key, numbers in (("scale_factor", (0, -1, float("nan"), float("inf"))),
                             ("top_content_inset", (-1, 800, float("nan"), float("inf")))):
            for number in numbers:
                value = self.geometry()
                value["native"][key] = number
                with self.subTest(key=key, number=number), self.assertRaises(ValueError):
                    t01.validate_window(value)


class ReportTests(unittest.TestCase):
    def setUp(self):
        self.temp = self.enterContext(tempfile.TemporaryDirectory(prefix="t01-report-regression-"))
        root = Path(self.temp)
        self.enterContext(mock.patch.object(t01, "ROOT", root))
        self.enterContext(mock.patch.object(t01.workflow, "snapshot", return_value="synthetic-source"))
        self.enterContext(mock.patch.object(t01.workflow, "protected_hash", return_value="synthetic-protected"))
        self.run = t01.Acceptance("all", root / "report.json", root / "junit.xml")

    def output(self):
        status = self.run.save()
        report = t01.read_json(self.run.report_path)
        xml = ET.parse(self.run.junit_path).getroot()
        self.assertEqual(int(xml.get("tests")), len(xml.findall("testcase")))
        self.assertEqual(int(xml.get("failures")), len(xml.findall("testcase/failure")))
        return status, report, {node.get("name"): node for node in xml.findall("testcase")}

    def test_failed_child_writes_json_and_junit_with_original_details(self):
        self.run.case("native-geometry", list(t01.IDS), "expected native geometry",
                      lambda: t01.require(False, "default window size differs: 1200x768"))
        self.run.cases[0]["evidence"] = ["synthetic-native.log"]
        status, report, nodes = self.output()
        self.assertEqual(status, 1)
        child = report["cases"][0]
        self.assertEqual(child["actual"], "ValueError: default window size differs: 1200x768")
        self.assertEqual(nodes[child["id"]].find("failure").text, child["actual"])
        for identifier in t01.IDS:
            with self.subTest(identifier=identifier):
                failure = nodes[identifier].find("failure")
                self.assertIsNotNone(failure)
                self.assertIn(child["actual"], failure.text)
                self.assertIn(child["id"], failure.text)
                self.assertIn("synthetic-native.log", failure.text)
                summary = next(s for s in report["summaries"] if s["id"] == identifier)
                self.assertEqual(summary["actual"]["failures"][0]["actual"], child["actual"])

    def test_missing_requirement_children_fail_even_with_zero_failed_cases(self):
        self.run.case("only-one-requirement", ["REQ-T01"], "synthetic pass", lambda: True)
        status, report, nodes = self.output()
        self.assertEqual((status, report["counts"]["failed"]), (1, 0))
        for identifier in t01.IDS[:-1]:
            summary = next(s for s in report["summaries"] if s["id"] == identifier)
            self.assertFalse(summary["actual"]["children_present"])
            self.assertIsNotNone(nodes[identifier].find("failure"))

    def test_required_skip_preserves_failure_reason(self):
        self.run.case("covered", list(t01.IDS), "synthetic pass", lambda: True)
        self.run.cases.append(dict(id="U01/native-missing", requirements=["U01"], required=True,
            expected="native run", actual="native run missing", result="SKIP", evidence=[]))
        status, report, nodes = self.output()
        self.assertEqual((status, report["counts"]["skipped"]), (1, 1))
        self.assertIn("native run missing", nodes["U01"].find("failure").text)
        self.assertEqual(nodes["U01/native-missing"].find("skipped").get("message"), "native run missing")

    def test_deferred_platform_skips_preserve_passing_summaries(self):
        self.run.case("covered", list(t01.IDS), "synthetic pass", lambda: True)
        t01.platform_matrix(self.run)
        status, report, nodes = self.output()
        self.assertEqual((status, report["counts"]["failed"], report["counts"]["skipped"]), (0, 0, 4))
        for identifier in t01.IDS:
            self.assertIsNone(nodes[identifier].find("failure"))

    def test_functional_suite_keeps_all_requirement_summaries(self):
        self.run.suite = "functional"
        self.run.case("covered", list(t01.IDS), "functional behavior", lambda: True)
        status, report, nodes = self.output()
        self.assertEqual(status, 0)
        self.assertEqual(report["suite"], "functional")
        self.assertEqual({s["id"] for s in report["summaries"]}, set(t01.IDS))
        for identifier in t01.IDS:
            self.assertIsNone(nodes[identifier].find("failure"))


class AutomationArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="t01-automation-regression-")
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.frontend = root / "frontend"
        self.frontend.mkdir()
        self.executable = root / "native"
        self.executable.write_bytes(b"T01_FIXTURE_CAP")

    def test_real_built_automation_artifacts_are_accepted(self):
        # The build suite precedes these regressions in the native acceptance gate.
        t01.validate_automation(t01.ROOT / "target/t01/frontend-automation",
            t01.DATA / "YingxiangVideoT01.app/Contents/MacOS/yingxiang-video")

    def test_missing_frontend_transport_or_native_bridge_fails(self):
        asset = self.frontend / "automation.js"
        for data in (b"", b"control/next", b"t01_control"):
            asset.write_bytes(data)
            with self.subTest(data=data), self.assertRaisesRegex(ValueError, "automation frontend mode"):
                t01.validate_automation(self.frontend, self.executable)

    def test_missing_frontend_chunk_fails(self):
        with self.assertRaisesRegex(ValueError, "automation frontend mode"):
            t01.validate_automation(self.frontend, self.executable)

    def test_missing_native_bootstrap_fails(self):
        (self.frontend / "automation.js").write_bytes(b"control/next t01_control")
        self.executable.write_bytes(b"production native")
        with self.assertRaisesRegex(ValueError, "automation native feature"):
            t01.validate_automation(self.frontend, self.executable)


if __name__ == "__main__":
    unittest.main()
