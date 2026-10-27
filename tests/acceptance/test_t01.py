"""Behavioral regressions for the fixture and evidence validator, run under run.py."""
import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest

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


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.samples = [dict(width=1920, height=1080, paused=False, media_error=None, audio_state="running",
            audio_rms=0.1, audio_peak_hz=440, time=(i + 1) * 5000, frames=(i + 1) * 150,
            raf=(i + 1) * 300, host_received_monotonic=(i + 1) * 5, max_raf_gap_ms=20,
            owned_rss_bytes=1000000) for i in range(360)]
        self.value = dict(duration_seconds=1800.1, media_seconds=1800, samples=360, unique_frames=10,
            initial=dict(frames=0, dropped_frames=0), final=dict(frames=54000, dropped_frames=3))

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
            for marker in (b"T01_FIXTURE_CAP", b"/control/next", b"control/next", b"t01_control"):
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
