"""T10 credential gate and raw sequence-IV evidence regression tests."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest

from t10 import Acceptance, ROOT, RUST_TESTS, check_vector, redact


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


class ImplicitIvOracleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.vector = dict(kind="implicit_iv", playlist=(
            '#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:37\n#EXT-X-KEY:METHOD=AES-128,URI="key.bin"\n'
            '#EXTINF:1,\nsegment-037.ts\n#EXTINF:1,\nsegment-038.ts\n#EXT-X-ENDLIST\n'),
            key_sha256=hashlib.sha256(bytes(range(16))).hexdigest(), segments=[])
        for sequence in (37, 38):
            # Small public oracle inputs; the native test separately generates
            # real playable TS, compares all plaintext bytes and all 24 frames.
            plaintext = bytes(range(188)) * 2
            result = subprocess.run(["openssl", "enc", "-aes-128-cbc", "-K", bytes(range(16)).hex(),
                                     "-iv", sequence.to_bytes(16, "big").hex()], input=plaintext,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10, check=True)
            wrong_iv = bytearray(plaintext)
            wrong_sequence = bytearray(plaintext)
            # CBC's IV affects only the first block: derive these expected
            # counterexample hashes without using the oracle decrypt routine.
            wrong_iv[15] ^= sequence
            wrong_sequence[15] ^= sequence ^ (sequence + 1)
            cls.vector["segments"].append(dict(uri=f"segment-{sequence:03d}.ts", sequence=sequence,
                plaintext_bytes=len(plaintext), plaintext_sha256=hashlib.sha256(plaintext).hexdigest(),
                ciphertext_sha256=hashlib.sha256(result.stdout).hexdigest(), ciphertext_hex=result.stdout.hex(),
                decrypted_sha256=hashlib.sha256(plaintext).hexdigest(),
                wrong_iv_sha256=hashlib.sha256(wrong_iv).hexdigest(),
                wrong_sequence_sha256=hashlib.sha256(wrong_sequence).hexdigest()))

    def test_actual_sequence_iv_plaintext_and_wrong_iv_counterexamples(self):
        check_vector(self.vector)

    def test_rejects_explicit_iv_zero_sequence_and_incomplete_or_reordered_segments(self):
        for field, value in [
            ("playlist", self.vector["playlist"].replace('URI="key.bin"', 'URI="key.bin",IV=0x0')),
            ("playlist", self.vector["playlist"].replace("SEQUENCE:37", "SEQUENCE:0")),
            ("segments", self.vector["segments"][:1]),
            ("segments", list(reversed(self.vector["segments"]))),
        ]:
            with self.subTest(field=field, value=str(value)[:100]):
                vector = copy.deepcopy(self.vector)
                vector[field] = value
                with self.assertRaises(ValueError):
                    check_vector(vector)

    def test_rejects_wrong_sequence_and_forged_full_byte_evidence(self):
        for field, value in [("sequence", 38), ("plaintext_sha256", "0" * 64),
                             ("decrypted_sha256", "0" * 64), ("plaintext_bytes", 188),
                             ("wrong_iv_sha256", self.vector["segments"][0]["plaintext_sha256"]),
                             ("wrong_sequence_sha256", self.vector["segments"][0]["plaintext_sha256"])]:
            with self.subTest(field=field):
                vector = copy.deepcopy(self.vector)
                vector["segments"][0][field] = value
                with self.assertRaises(ValueError):
                    check_vector(vector)

    def test_rejects_changed_ciphertext_even_with_updated_cipher_hash(self):
        vector = copy.deepcopy(self.vector)
        ciphertext = bytearray.fromhex(vector["segments"][0]["ciphertext_hex"])
        ciphertext[0] ^= 1
        vector["segments"][0]["ciphertext_hex"] = ciphertext.hex()
        vector["segments"][0]["ciphertext_sha256"] = hashlib.sha256(ciphertext).hexdigest()
        with self.assertRaises(ValueError):
            check_vector(vector)


if __name__ == "__main__":
    unittest.main()
