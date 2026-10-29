"""T10 Cargo runner names and persisted credential gate regression tests."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

from t10 import Acceptance, ROOT, RUST_TESTS, redact


class CredentialOutputTests(unittest.TestCase):
    def test_only_fixed_native_runner_positions_preserve_inventory_names(self):
        for name in RUST_TESTS:
            for line in [f"test gateway::tests::t10::{name} ... ok\n",
                         f"    gateway::tests::t10::{name}\n",
                         f"thread 'gateway::tests::t10::{name}' (123) panicked at fixture.rs:1:1:\n"]:
                with self.subTest(name=name, context=line.split(name)[0]):
                    self.assertEqual(redact(line), line)

    def test_json_url_debug_and_unknown_runner_names_still_filter(self):
        name = "t10_cross_origin_private_key_and_closed_cap"
        for token in [name, "s" * 43]:
            for line in [json.dumps({"capability": token}),
                         f"http://127.0.0.1:12/media/id/stream?cap={token}",
                         f'assertion failed: left: String("{token}")',
                         f"Authorization: Bearer {token}",
                         f"test other::tests::t10::{token} ... ok\n",
                         f'test gateway::tests::t10::{name} ... {{"capability":"{token}"}}\n']:
                with self.subTest(context=line.split(token)[0]):
                    safe = redact(line)
                    self.assertNotEqual(safe, line)
                    self.assertIn("<redacted-cap-256>", safe)
                    self.assertNotIn(f'"{token}"', safe)
        self.assertNotEqual(redact("test gateway::tests::t10::" + "z" * 43 + " ... ok\n"),
                            "test gateway::tests::t10::" + "z" * 43 + " ... ok\n")

    def command(self, folder, script):
        run = Acceptance.__new__(Acceptance)
        run.folder = Path(folder)
        run.commands = []
        return run, [sys.executable, "-c", script]

    def test_runner_name_preserves_actual_native_failure(self):
        line = "test gateway::tests::t10::t10_cross_origin_private_key_and_closed_cap ... FAILED"
        with tempfile.TemporaryDirectory(dir=ROOT / "evidence") as folder:
            run, argv = self.command(folder, f"print({line!r});raise SystemExit(101)")
            with self.assertRaisesRegex(ValueError, r"native command failed \(101\)"):
                run.command(argv)
            self.assertEqual(run.commands[0]["exit"], 101)
            self.assertIn(line, (Path(folder) / "command-1.log").read_text())

    def test_command_rejects_and_sanitizes_secret_after_known_runner_prefix(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "evidence") as folder:
            prefix = "test gateway::tests::t10::t10_cross_origin_private_key_and_closed_cap ... "
            run, argv = self.command(folder, f"print({prefix!r} + 'http://127.0.0.1/?cap=' + 's' * 43)")
            with self.assertRaisesRegex(ValueError, "native output contained a credential"):
                run.command(argv)
            output = (Path(folder) / "command-1.log").read_text()
            self.assertIn(prefix, output)
            self.assertNotIn("s" * 43, output)
            self.assertIn("<redacted-cap-256>", output)


if __name__ == "__main__":
    unittest.main()
