import copy
import unittest

import t02


def evidence():
    return dict(actual_gateway=True, error_count=0, native=dict(audio_playing=True),
                first=dict(frames=3, position=.3, time=1000),
                final=dict(frames=130, position=5.3, time=6000, paused=False, media_error=None, width=1280, height=720))


def sources():
    media = lambda passed, attempted: dict(attempted=attempted, passed=passed, failed_hosts=[], verdict="x")
    return [dict(name="A", compatibility="COMPATIBLE", supported=True, media_samples=media(3, 8)),
            dict(name="B", compatibility="COMPATIBLE", supported=True, media_samples=media(0, 8)),
            dict(name="C", compatibility="SOURCE_UNAVAILABLE", supported=False, media_samples=media(0, 0))]


class SourceCompatibilityEvidence(unittest.TestCase):
    def test_unavailable_media_or_source_is_not_a_program_failure(self):
        self.assertEqual(t02.validate_sources(sources(), "A"), ["A", "B"])

    def test_program_defects_and_inconsistent_results_fail(self):
        for change in (lambda s: s[0].update(compatibility="PROGRAM_INCOMPATIBLE"), lambda s: s[1].update(supported=False),
                       lambda s: s[2].update(supported=True), lambda s: s[0].pop("compatibility"),
                       lambda s: s[0]["media_samples"].update(attempted=9)):
            value = sources()
            change(value)
            with self.assertRaises(ValueError):
                t02.validate_sources(value, "A")
        with self.assertRaises(ValueError):
            t02.validate_sources(sources(), "B")


class PublicPlaybackEvidence(unittest.TestCase):
    def rejects(self, change):
        value = copy.deepcopy(evidence())
        change(value)
        with self.assertRaises(ValueError):
            t02.validate_short_play(value)

    def test_complete_short_play_evidence(self):
        t02.validate_short_play(evidence())

    def test_stalled_frames_and_media_clock_fail(self):
        self.rejects(lambda value: value["final"].update(frames=3))
        self.rejects(lambda value: value["final"].update(position=.3))
        self.rejects(lambda value: value["final"].update(time=2000))

    def test_audio_decode_and_webview_errors_fail(self):
        self.rejects(lambda value: value["native"].update(audio_playing=False))
        self.rejects(lambda value: value["final"].update(width=0))
        self.rejects(lambda value: value["final"].update(media_error=4))
        self.rejects(lambda value: value.update(error_count=1))

    def test_media_must_use_actual_gateway(self):
        self.rejects(lambda value: value.update(actual_gateway=False))

    def test_native_runner_names_preserved_credentials_removed(self):
        name = next(name for name in t02.NATIVE if len(name) > 48)
        runner = f"test host::maccms::tests::t02::{name} ... ok\n"
        self.assertEqual(t02.redact(runner), runner)
        secret = "A" * 43
        self.assertNotIn(secret, t02.redact("unexpected capability=" + secret))
        self.assertNotIn("PRIVATE", t02.redact("https://media.example/a.m3u8?signature=PRIVATE"))
