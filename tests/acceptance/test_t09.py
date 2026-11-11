"""Regression checks for the T09 persisted-output credential gate."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

from t09 import Acceptance, redact


class CredentialOutputTests(unittest.TestCase):
    def test_schema_valid_placeholder_is_preserved(self):
        line = json.dumps({"capability": "r" * 43, "url": "http://127.0.0.1:12/images/id?cap=" + "r" * 43})
        self.assertEqual(redact(line), line)

    def test_real_capabilities_in_json_urls_and_debug_are_filtered(self):
        secret = "s" * 43
        for line in [json.dumps({"media_capability": secret}),
                     f"http://127.0.0.1:12/media/id/stream?cap={secret}",
                     f'assertion failed: left: String("{secret}")',
                     f"Authorization: Bearer {secret}"]:
            with self.subTest(context=line.split(secret)[0]):
                safe = redact(line)
                self.assertNotEqual(safe, line)
                self.assertNotIn(secret, safe)
                self.assertIn("<redacted-cap-256>", safe)

    def test_mixed_placeholder_and_real_capability_still_fails_detection(self):
        line = f'{{"capability":"{"r" * 43}","url":"http://127.0.0.1/?cap={"s" * 43}"}}'
        safe = redact(line)
        self.assertNotEqual(safe, line)
        self.assertIn("r" * 43, safe)
        self.assertNotIn("s" * 43, safe)

    def test_only_the_exact_fixed_placeholder_is_exempt(self):
        for token in ["a" * 43, "z" * 43, "r" * 42 + "s", "AZaz09_-" * 5 + "AZa"]:
            self.assertNotEqual(redact(token), token)

    def command(self, folder, text):
        run = Acceptance.__new__(Acceptance)
        run.folder = Path(folder)
        run.sample_path = run.folder / "unused-phase.json"
        run.commands, run.observed = [], {}
        return run, [sys.executable, "-c", "print(" + repr(text) + ")"]

    def test_command_accepts_placeholder_without_credential_count(self):
        # Evidence paths must stay under the project, as in the real A1 runner.
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[2] / "evidence") as folder:
            line = json.dumps({"capability": "r" * 43})
            run, argv = self.command(folder, line)
            self.assertEqual(run.command(argv).strip(), line)
            self.assertEqual(run.observed["native_credential_lines"], 0)

    def test_command_rejects_real_credential_and_only_persists_safe_output(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[2] / "evidence") as folder:
            secret = "s" * 43
            # Construct in the child so the test command itself has no secret.
            run, _ = self.command(folder, "")
            with self.assertRaisesRegex(ValueError, "native output contained a credential"):
                run.command([sys.executable, "-c", "print('http://127.0.0.1:12/images/id?cap=' + 's' * 43)"])
            self.assertEqual(run.observed["native_credential_lines"], 1)
            output = (run.folder / "command-1.log").read_text()
            self.assertNotIn(secret, output)
            self.assertIn("<redacted-cap-256>", output)


if __name__ == "__main__":
    unittest.main()
