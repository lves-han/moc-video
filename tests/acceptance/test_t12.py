"""Independent negative controls for the real T12 native evidence oracle."""
import copy
import tempfile
from pathlib import Path
import unittest
import urllib.error
import urllib.request
import json

import t12


def cycle():
    return dict(index=1, old_tracks_disabled=True, session_changed=True, new_frames=5, actual_gateway=True,
                errors=[], host=dict(live_sessions=1, requests=0, old_http_status=410),
                sample=dict(video_count=1, tracks=1, width=1920, height=1080, media_error=None,
                            audio_rms=.1, audio_peak_hz=880, pixel=[0, 0, 254, 255],
                            diagnostics=dict(hls_instances=0, listening=True, retiring=0)))


def stopped():
    return dict(source_removed=True, frames_after_stop=0, host=dict(live_sessions=0, requests=0, source_active=0),
                sample=dict(video_count=1, paused=True, ready_state=0, network_state=0, source_present=False,
                            tracks=0, audio_rms=0, diagnostics=dict(listening=False, hls_instances=0, retiring=0)))


class NativeOracleTests(unittest.TestCase):
    def test_cycle_accepts_current_audio_color_and_released_old_handles(self):
        t12.validate_cycle(cycle(), 1)
        value = cycle()
        value["index"] = 2
        value["sample"].update(pixel=[254, 0, 0, 255], audio_peak_hz=440)
        t12.validate_cycle(value, 2)

    def test_cycle_rejects_every_resource_audio_frame_and_identity_corruption(self):
        edits = [("index", 2), ("old_tracks_disabled", False), ("session_changed", False),
                 ("new_frames", 0), ("actual_gateway", False), ("errors", ["runtime failed"])]
        for key, value in edits:
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_cycle(dict(cycle(), **{key: value}), 1)
        edits = [("video_count", 2), ("tracks", 2), ("width", 0), ("height", 0), ("media_error", 3),
                 ("audio_rms", 0), ("audio_peak_hz", 440), ("pixel", [254, 0, 0, 255]), ("pixel", [0, 0, 0, 255])]
        for key, value in edits:
            row = cycle(); row["sample"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_cycle(row, 1)
        for key, value in [("hls_instances", 2), ("listening", False), ("retiring", 1)]:
            row = cycle(); row["sample"]["diagnostics"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_cycle(row, 1)
        for key, value in [("live_sessions", 2), ("requests", 1), ("old_http_status", 200)]:
            row = cycle(); row["host"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_cycle(row, 1)

    def test_stop_rejects_old_audio_frame_and_every_retained_native_resource(self):
        t12.validate_stop(stopped())
        for key, value in [("source_removed", False), ("frames_after_stop", 2)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_stop(dict(stopped(), **{key: value}))
        for key, value in [("video_count", 2), ("paused", False), ("ready_state", 1), ("network_state", 2),
                           ("source_present", True), ("tracks", 1), ("audio_rms", .1)]:
            row = stopped(); row["sample"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_stop(row)
        for key, value in [("listening", True), ("hls_instances", 1), ("retiring", 1)]:
            row = stopped(); row["sample"]["diagnostics"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_stop(row)
        for key in ("live_sessions", "requests", "source_active"):
            row = stopped(); row["host"][key] = 1
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_stop(row)

    def test_requirement_summary_requires_real_children_and_no_skipped_required_behavior(self):
        self.assertTrue(all(r["result"] == "FAIL" for r in t12.summaries([])))
        cases = [dict(id="all", requirements=t12.IDS, result="PASS")]
        self.assertTrue(all(r["result"] == "PASS" for r in t12.summaries(cases)))
        for result in ("FAIL", "SKIP", "NOT RUN"):
            self.assertTrue(all(r["result"] == "FAIL" for r in t12.summaries([dict(cases[0], result=result)])))

    def test_locked_native_desktop_stops_before_build_or_launch(self):
        from types import SimpleNamespace
        from unittest.mock import patch
        run = SimpleNamespace(observed={})
        with patch.object(t12.t01, "native_gui_session", return_value=dict(screen_locked=True)), \
             patch.object(t12.t01, "build") as build:
            with self.assertRaisesRegex(ValueError, "desktop locked"):
                t12.webview(run)
            build.assert_not_called()


class TransportTests(unittest.TestCase):
    def test_real_host_transport_exists_only_when_explicitly_supplied_and_authenticated(self):
        calls = []
        with tempfile.TemporaryDirectory() as temporary:
            for enabled in (False, True):
                with t12.t01.Fixture(Path(temporary), invoke=(lambda value: calls.append(value) or dict(ok=True)) if enabled else None) as fixture:
                    url = fixture.base + "control/invoke?cap=" + fixture.cap
                    request = urllib.request.Request(url, data=json.dumps(dict(command="runtime_status")).encode(), headers={"Content-Type": "application/json", "Origin": "tauri://localhost"})
                    if enabled:
                        with urllib.request.urlopen(request) as response:
                            self.assertEqual(json.load(response), dict(ok=True))
                    else:
                        with self.assertRaises(urllib.error.HTTPError) as error:
                            urllib.request.urlopen(request)
                        self.assertEqual(error.exception.code, 405)
                    denied = urllib.request.Request(url, data=b'{}', headers={"Origin": "https://untrusted.invalid"})
                    with self.assertRaises(urllib.error.HTTPError) as error:
                        urllib.request.urlopen(denied)
                    self.assertEqual(error.exception.code, 403)
            self.assertEqual(calls, [dict(command="runtime_status")])


if __name__ == "__main__":
    unittest.main()
