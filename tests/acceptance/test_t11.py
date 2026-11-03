"""Negative controls for T11's independent evidence and conditional DASH gate."""
import copy
import datetime as dt
import hashlib
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import t11

BOUNDARY_VTT = (
    "WEBVTT\n\n00:00:04.250 --> 00:00:05.750\n前中文，2 &lt; 3！后\n第二行\n\n"
    "00:00:06.250 --> 00:00:07.750\n<c></c>\n<c></c>\n前\n<c></c>\n<c></c>\n后\n<c></c>\n<c></c>\n\n"
    "00:00:08.250 --> 00:00:09.750\n<c></c>\n前\n<c></c>\n后\u00a0尾\n<c></c>\n\n"
    "00:00:10.250 --> 00:00:11.750\n<c></c>\n前\n<c></c>\n后\n<c></c>\n\n"
)


class VttOracleTests(unittest.TestCase):
    def test_ass_boundaries_parse_actual_wire_and_reject_comments_truncation_and_collapsed_lines(self):
        vector = dict(kind="ass-boundaries", input=t11.ASS_BOUNDARY_INPUT, output=BOUNDARY_VTT)
        t11.check_vector(vector)
        self.assertEqual(t11.parse_vtt(BOUNDARY_VTT), [
            (4.25, 5.75, "前中文，2 < 3！后\n第二行"),
            (6.25, 7.75, "\n\n前\n\n\n后\n\n"),
            (8.25, 9.75, "\n前\n\n后\u00a0尾\n"),
            (10.25, 11.75, "\n前\n\n后\n"),
        ])
        for output in [BOUNDARY_VTT.replace("前中文", "前{literal}中文"),
                       BOUNDARY_VTT.replace("！后", "！注释后"),
                       BOUNDARY_VTT.replace("<c></c>", ""),  # original cue-terminating bug
                       BOUNDARY_VTT.replace("<c></c>\n", ""),
                       BOUNDARY_VTT.replace("后\u00a0尾", ""),
                       BOUNDARY_VTT.replace("04.250", "04.251"),
                       BOUNDARY_VTT.replace("2 &lt; 3", "2  3"),
                       BOUNDARY_VTT.replace("前\n<c></c>\n后", "前m 0 0 l 1 1\n<c></c>\n后")]:
            with self.subTest(output=output), self.assertRaises(ValueError):
                t11.check_vector(dict(vector, output=output))
        with self.assertRaises(ValueError):
            t11.check_vector(dict(vector, input=t11.ASS_BOUNDARY_INPUT.replace("\\N\\N", "\\N")))

    def test_independent_plain_text_preserves_markup_entities_and_line_breaks(self):
        text = "WEBVTT\n\ncue-id\n00:01.250 --> 00:03.750 line:90%\n<v 中文><b>你好</b> &amp; world\n第二行\n\n"
        self.assertEqual(t11.parse_vtt(text), t11.EXPECTED["vtt"])
        vector = dict(kind="http", format="vtt", output=text, head_length=str(len(text.encode())),
                      etag='"' + hashlib.sha256(text.encode()).hexdigest() + '"', same_cap=True, conditional_status=304)
        t11.check_vector(vector)
        for field, value in [("output", text.replace("01.250", "01.251")), ("output", text.replace("第二行", "changed")),
                             ("output", text.replace("\n第二行", "第二行")), ("head_length", "1"), ("etag", '"source"'),
                             ("same_cap", False), ("conditional_status", 200)]:
            bad = dict(vector, **{field: value})
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                t11.check_vector(bad)

    def test_invalid_timestamps_empty_results_and_forged_resource_observations(self):
        for text in ["", "WEBVTT\n\n00:03.750 --> 00:01.250\ntext", "WEBVTT\n\nmissing arrow"]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                t11.parse_vtt(text)
        for vector in [dict(kind="host", tracks=0), dict(kind="drm", type="none"),
                       dict(kind="cancel", late_bytes=1, active_requests=0),
                       dict(kind="isolation", cross_origin_stripped=True, direct_origin_stripped=True, private_target_contacted=True)]:
            with self.subTest(vector=vector), self.assertRaises((ValueError, KeyError)):
                t11.check_vector(vector)

    def test_lifetime_requires_unauthorized_cap_and_all_cleanup_observations(self):
        vector = dict(kind="lifetime", at_ms=300001, resolution_expired=True, authenticated_after_expiry=True,
                      continued_status=200, old_status=410, wrong_cap=401, revoked_status=410, active_requests=0)
        t11.check_vector(vector)
        for key, value in [("wrong_cap", 200), ("wrong_cap", 403), ("old_status", 200),
                           ("revoked_status", 200), ("active_requests", 1), ("authenticated_after_expiry", False)]:
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                t11.check_vector(dict(vector, **{key: value}))

    def test_locked_desktop_fails_before_build_and_native_launch(self):
        run = SimpleNamespace(observed={})
        with patch.object(t11.t01, "native_gui_session", return_value=dict(screen_locked=True)), \
             patch.object(t11.t01, "build") as build, patch.object(t11.t01, "NativeSession") as native:
            with self.assertRaisesRegex(ValueError, "unlock the desktop"):
                t11.webview(run)
            build.assert_not_called()
            native.assert_not_called()
        self.assertEqual(run.observed["gui_session"], dict(screen_locked=True))

    def test_webview_oracle_rejects_wrong_time_text_missing_tracks_and_no_frames(self):
        vector = dict(episode="first", disabled=True, cue_ended=True, actual_gateway=True, new_frames=3, old_tracks_removed=0,
            observations=[dict(format=f, start=1.25, end=3.75, text=f"first {f.upper()}\n第二行", mode="showing", cues=5 if f == "ass" else 1)
                          for f in ("vtt", "srt", "ass")],
            ass_boundaries=[dict(start=start, end=end, text=text, wire_text=block.split("\n", 1)[1], cue_ended=True)
                for (start, end, text), block in zip(t11.ASS_BOUNDARIES, BOUNDARY_VTT[8:].strip().split("\n\n"))],
            sample=dict(media_error=None, width=1920, height=1080, frames=3, diagnostics=dict(tracks=3)))
        t11.validate_webview(vector, "first")
        edits = [("disabled", False), ("cue_ended", False), ("actual_gateway", False), ("new_frames", 0), ("old_tracks_removed", 3),
                 ("observations", vector["observations"][:2])]
        for key in ("start", "end", "text", "mode"):
            rows = copy.deepcopy(vector["observations"])
            rows[0][key] = 0 if key in ("start", "end") else "corrupt"
            edits.append(("observations", rows))
        for key in ("frames", "width", "height"):
            sample = copy.deepcopy(vector["sample"])
            sample[key] = 0
            edits.append(("sample", sample))
        edits.append(("ass_boundaries", vector["ass_boundaries"][:3]))
        for i, row in enumerate(vector["ass_boundaries"]):
            for key, value in [("start", row["start"] + .001), ("end", row["end"] + .001),
                               ("text", row["text"].replace("\n\n", "\n") + "{literal}"),
                               ("text", row["wire_text"]), ("text", row["text"].split("\n")[0]),
                               ("wire_text", row["wire_text"].replace("<c></c>", "") + "corrupt"),
                               ("cue_ended", False)]:
                rows = copy.deepcopy(vector["ass_boundaries"])
                rows[i][key] = value
                edits.append(("ass_boundaries", rows))
        for key, value in edits:
            with self.subTest(key=key), self.assertRaises(ValueError):
                t11.validate_webview(dict(vector, **{key:value}), "first")


class DashEvidenceTests(unittest.TestCase):
    def fixture(self, root):
        plugins = []
        for i in range(7):
            path = root / "evidence/t00/inputs" / f"plugin-{i}.cs3"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(f"synthetic artifact {i}".encode())
            plugins.append(dict(internal_name=f"plugin-{i}", sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
        inventory = root / "replica-spec/references/plugin-artifact-inventory.json"
        inventory.parent.mkdir(parents=True)
        inventory.write_text(json.dumps(dict(plugins=plugins)))
        folder = root / "evidence/acceptance/T11-fixture"
        folder.mkdir(parents=True)
        (root / "evidence/t01").mkdir()
        return SimpleNamespace(folder=folder, observed={}, cases=[])

    def test_absence_is_an_explicit_conditional_not_run_with_pending_original_baseline(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(t11, "ROOT", Path(folder)):
            run = self.fixture(Path(folder))
            value = t11.dash_baseline(run)
            self.assertEqual(value["dash"], "NOT RUN")
            self.assertEqual(run.observed["dash"]["status"], "NOT RUN")
            self.assertIn("pending N02", run.observed["dash"]["reason"])
            self.assertEqual(len(run.observed["dash"]["artifacts"]), 7)
            self.assertEqual(run.cases[0]["result"], "SKIP")
            self.assertFalse(run.cases[0]["required"])

    def test_any_actual_mpd_or_observed_dash_link_blocks_the_conditional_skip(self):
        for mode in ("file", "url", "stream_kind", "kind"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as folder, patch.object(t11, "ROOT", Path(folder)):
                root = Path(folder)
                run = self.fixture(root)
                baseline = root / "evidence/acceptance/N02-baseline.json"
                if mode == "file":
                    (root / "evidence/t00/actual.mpd").write_text('<MPD xmlns="urn:mpeg:dash:schema:mpd:2011"/>')
                else:
                    link = dict(url="https://provider.invalid/actual.mpd?signature=secret") if mode == "url" else {mode:"dash"}
                    baseline.write_text(json.dumps(dict(observed=dict(provider_outputs=[link]))))
                with self.assertRaisesRegex(ValueError, "DASH artifacts were observed"):
                    t11.dash_baseline(run)
                self.assertEqual(run.observed["dash"]["status"], "REQUIRES TESTING")
                self.assertEqual(run.cases, [])
                self.assertNotIn("signature=secret", json.dumps(run.observed))


class CredentialAndReportTests(unittest.TestCase):
    def test_only_fixed_runner_names_escape_redaction(self):
        for name in t11.RUST_TESTS:
            line = f"test {name} ... ok\n"
            self.assertEqual(t11.redact(line), line)
            leaked = f'test {name} ... {{"cap":"' + "s"*43 + '"}\n'
            self.assertNotIn("s"*43, t11.redact(leaked))
        self.assertNotEqual(t11.redact("test foreign::" + "s"*43 + " ... ok"), "test foreign::" + "s"*43 + " ... ok")

    def test_command_keeps_nonzero_result_and_sanitizes_sensitive_output(self):
        with tempfile.TemporaryDirectory(dir=t11.ROOT / "evidence") as folder:
            run = t11.Acceptance.__new__(t11.Acceptance)
            run.folder, run.commands = Path(folder), []
            with self.assertRaisesRegex(ValueError, r"command failed \(7\)"):
                run.command([sys.executable, "-c", "print('native failure');raise SystemExit(7)"])
            self.assertEqual(run.commands[0]["exit"], 7)
            with self.assertRaisesRegex(ValueError, "contained credentials"):
                run.command([sys.executable, "-c", "print('http://127.0.0.1/?cap='+'s'*43)"])
            self.assertNotIn("s"*43, (Path(folder) / "command-2.log").read_text())

    def test_unapproved_skip_or_failed_behavior_fails_requirement_summary(self):
        for result in ("FAIL", "SKIP"):
            with self.subTest(result=result), tempfile.TemporaryDirectory() as folder:
                run = t11.Acceptance.__new__(t11.Acceptance)
                run.source, run.started = "stable", dt.datetime.now(dt.timezone.utc).isoformat()
                run.report_path, run.junit_path = Path(folder)/"report.json", Path(folder)/"report.xml"
                run.commands, run.observed = [], {}
                run.cases = [dict(id="R29/required-behavior", requirements=list(t11.IDS), required=False,
                    result=result, actual="failure retained", expected="behavior passes", evidence=[])]
                with patch.object(t11.workflow, "snapshot", return_value="stable"), patch.object(t11.workflow, "protected_hash", return_value="protected"):
                    self.assertEqual(run.save(), 1)
                self.assertTrue(all(row["result"] == "FAIL" for row in json.loads(run.report_path.read_text())["summaries"]))


if __name__ == "__main__":
    unittest.main()
