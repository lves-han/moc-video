"""Delivery evidence counterexamples; these fixtures never attest a real build."""
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import t25


class DeliveryTests(unittest.TestCase):
    def test_readme_recipe_runs_delivery_and_native_diagnostics(self):
        rows = t25.recipe((t25.ROOT / "README.md").read_text())
        self.assertEqual(rows[0]["argv"], ["npm", "ci", "--offline", "--no-audit", "--no-fund"])
        self.assertEqual(rows[4]["argv"][-3:], ["t25_delivery", "--", "--test-threads=1"])
        self.assertEqual(len(rows), 5)
        self.assertEqual(rows[-1]["id"], "native-smoke")

    def test_missing_incomplete_or_shell_recipe_fails(self):
        text = (t25.ROOT / "README.md").read_text()
        changes = ["no recipe", text.replace('"id":"native-smoke"', '"id":"omitted"'),
                   text.replace('"timeout":900', '"timeout":0'),
                   text.replace('"argv":["npm","ci","--offline","--no-audit","--no-fund"]', '"argv":"npm ci"')]
        for value in changes:
            with self.subTest(value=value[:40]), self.assertRaises((ValueError, KeyError)):
                t25.recipe(value)

    def test_missing_changed_or_overclaimed_materials_fail(self):
        # Work from a small independent material tree, including successful raw
        # receipt/log bindings. Saved status flags alone do not validate it.
        with tempfile.TemporaryDirectory(dir=t25.ROOT / "evidence", prefix="t25-materials-") as temporary:
            root = Path(temporary)
            original = json.loads((t25.ROOT / "docs/runtime.lock.json").read_text())["final_delivery"]
            lock = copy.deepcopy(original)
            for name in lock["locks"]:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("synthetic locked material")
                lock["locks"][name] = t25.file_hash(path)
            for row in lock["historical_evidence"]:
                report = root / row["path"]
                report.parent.mkdir(parents=True, exist_ok=True)
                receipt = root / f'evidence/runs/{row["card"]}/receipt.json'
                receipt.parent.mkdir(parents=True)
                log = receipt.with_name("output.log")
                log.write_text("synthetic original log")
                receipt.write_text(json.dumps(dict(status="finished", exit=0, raw_exit=0, card=row["card"], source=row["source"],
                    after_source=row["source"], log=log.relative_to(root).as_posix(), log_hash=t25.file_hash(log))))
                report.write_text(json.dumps(dict(card=row["card"], source_before=row["source"], counts=dict(passed=1, failed=0),
                    summaries=[dict(result="PASS")], observed=dict(resources=dict(receipt=receipt.relative_to(root).as_posix())))))
                row["sha256"] = t25.file_hash(report)
            (root / "README.md").write_bytes((t25.ROOT / "README.md").read_bytes())
            index = root / "docs/release-evidence/T25.md"
            index.parent.mkdir(parents=True)
            index.write_text("synthetic index")
            version = root / "docs/runtime.lock.json"
            def write(value):
                version.write_text(json.dumps(dict(final_delivery=value)))
            write(lock)
            self.assertEqual(t25.materials(root)["mobile"][0]["card"], "T2.1")
            for mutate in (lambda d: d["deferred"][0].update(status="PASS"),
                           lambda d: d["mobile"][0].update(status="PASS"),
                           lambda d: d["locks"].update({"package-lock.json": "stale"}),
                           lambda d: d["historical_evidence"][0].update(source="new-source")):
                wrong = copy.deepcopy(lock)
                mutate(wrong)
                write(wrong)
                with self.assertRaises(ValueError):
                    t25.materials(root)
            write(lock)
            (root / "package-lock.json").unlink()
            with self.assertRaises(FileNotFoundError):
                t25.materials(root)

    def test_source_zip_reopens_bytes_and_modes_in_an_empty_directory(self):
        with tempfile.TemporaryDirectory(dir=t25.ROOT / "evidence", prefix="t25-source-") as temporary:
            root = Path(temporary)
            names = ["README.md", "src-tauri/Cargo.lock", "package-lock.json", "docs/runtime.lock.json"]
            for name in names:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(name)
            (root / "README.md").chmod(0o755)
            output = root / "output"
            output.mkdir()
            with mock.patch.object(t25, "ROOT", root), mock.patch.object(t25.workflow, "git", return_value="\0".join(names)):
                source, record = t25.source_archive(output, root)
                self.assertEqual(record["files"], 4)
                self.assertEqual((source / "README.md").read_text(), "README.md")
                if os.name != "nt":
                    self.assertEqual((source / "README.md").stat().st_mode & 0o777, 0o755)
                else:
                    self.assertEqual((source / "README.md").stat().st_mode & 0o777, (root / "README.md").stat().st_mode & 0o777)
                self.assertFalse((source / "target").exists())
                with self.assertRaisesRegex(ValueError, "already exists"):
                    t25.source_archive(output, root)


if __name__ == "__main__":
    unittest.main()
