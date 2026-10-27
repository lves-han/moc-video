"""Synthetic rejection tests for T00 validators; real artifacts are checked by all."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("t00", Path(__file__).with_name("t00.py"))
t00 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t00)


class T00ValidationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="t00-synthetic-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = dict(name="Synthetic", version=1, pluginClassName="example.Synthetic", requiresResources=False)
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w") as archive:
            archive.writestr("manifest.json", json.dumps(self.manifest))
            archive.writestr("classes.dex", b"synthetic validator fixture; never an original plugin")
        self.data = buffer.getvalue()
        self.artifact = self.root / "Synthetic.cs3"
        self.artifact.write_bytes(self.data)
        self.expected = dict(downloaded_bytes=len(self.data), index_file_size=len(self.data),
                             sha256=hashlib.sha256(self.data).hexdigest(), entries=["manifest.json", "classes.dex"],
                             plugin_manifest=self.manifest)
        self.inventory = dict(plugins=[dict(internal_name=f"Synthetic{i}", version=i + 1,
                                            downloaded_bytes=100 + i, url=f"https://example.test/{i}.cs3")
                                       for i in range(7)])
        self.registry = [dict(internalName=p["internal_name"], version=p["version"], fileSize=p["downloaded_bytes"],
                              url=p["url"]) for p in self.inventory["plugins"]]

    def test_accepts_archive_and_returns_original_manifest_bytes(self):
        actual, manifest = t00.validate_artifact(self.artifact, self.expected)
        self.assertEqual(actual["sha256"], hashlib.sha256(self.data).hexdigest())
        self.assertEqual(manifest, json.dumps(self.manifest).encode())

    def test_missing_archive_fails(self):
        with self.assertRaises(FileNotFoundError):
            t00.validate_artifact(self.root / "Missing.cs3", self.expected)

    def test_same_length_corruption_fails_sha(self):
        self.artifact.write_bytes(bytes([self.data[0] ^ 1]) + self.data[1:])
        with self.assertRaisesRegex(ValueError, "SHA256 differs"):
            t00.validate_artifact(self.artifact, self.expected)

    def test_truncated_archive_fails_length(self):
        self.artifact.write_bytes(self.data[:-1])
        with self.assertRaisesRegex(ValueError, "length differs"):
            t00.validate_artifact(self.artifact, self.expected)

    def test_wrong_expected_sha_fails(self):
        self.expected["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "SHA256 differs"):
            t00.validate_artifact(self.artifact, self.expected)

    def test_wrong_index_size_fails(self):
        self.expected["index_file_size"] += 1
        with self.assertRaisesRegex(ValueError, "length differs"):
            t00.validate_artifact(self.artifact, self.expected)

    def test_wrong_manifest_fails_even_with_matching_archive_sha(self):
        self.expected["plugin_manifest"] = dict(self.manifest, pluginClassName="example.Wrong")
        with self.assertRaisesRegex(ValueError, "manifest differs"):
            t00.validate_artifact(self.artifact, self.expected)

    def test_wrong_zip_entries_fail(self):
        self.expected["entries"] = ["manifest.json"]
        with self.assertRaisesRegex(ValueError, "ZIP entries differ"):
            t00.validate_artifact(self.artifact, self.expected)

    def test_seven_registry_identities_succeed_without_optional_fields(self):
        for row in self.registry:
            row["futureField"] = {"ignored": True}
        self.assertEqual(len(t00.validate_registry(self.registry, self.inventory)), 7)

    def test_six_registry_packages_fail(self):
        with self.assertRaisesRegex(ValueError, "seven packages"):
            t00.validate_registry(self.registry[:-1], self.inventory)

    def test_duplicate_registry_identity_fails(self):
        self.registry[1] = self.registry[0]
        with self.assertRaisesRegex(ValueError, "identities differ"):
            t00.validate_registry(self.registry, self.inventory)

    def test_registry_wrong_size_fails(self):
        self.registry[0]["fileSize"] += 1
        with self.assertRaisesRegex(ValueError, "length differs"):
            t00.validate_registry(self.registry, self.inventory)

    def test_registry_numeric_string_version_fails(self):
        self.registry[0]["version"] = str(self.registry[0]["version"])
        with self.assertRaisesRegex(ValueError, "version differs"):
            t00.validate_registry(self.registry, self.inventory)

    def test_registry_mutable_url_fails(self):
        self.registry[0]["url"] = "https://example.test/latest.cs3"
        with self.assertRaisesRegex(ValueError, "URL differs"):
            t00.validate_registry(self.registry, self.inventory)

    def version_row(self):
        return dict(status="version_observed", exit=0,
                    argv=[str(self.root / "rustc"), "--version"], stdout="rustc 1.2.3 (synthetic version fixture)\n")

    def test_official_version_shape_is_parsed(self):
        self.assertEqual(t00.validate_tool("rustc", self.version_row()), "1.2.3")

    def test_unknown_tool_marked_observed_fails(self):
        with self.assertRaisesRegex(ValueError, "unknown version tool"):
            t00.validate_tool("unidentified-compiler", self.version_row())

    def test_unknown_version_marked_observed_fails(self):
        row = self.version_row()
        row["stdout"] = "rustc unknown; verified"
        with self.assertRaisesRegex(ValueError, "unrecognized version output"):
            t00.validate_tool("rustc", row)

    def test_failed_version_command_marked_observed_fails(self):
        row = self.version_row()
        row["exit"] = 1
        with self.assertRaisesRegex(ValueError, "successful official version"):
            t00.validate_tool("rustc", row)

    def test_unrun_tool_marked_observed_fails(self):
        row = self.version_row()
        row["status"] = "NOT RUN"
        with self.assertRaisesRegex(ValueError, "successful official version"):
            t00.validate_tool("rustc", row)

    def test_plan_accepts_toolchain_install_without_self_update(self):
        lock = t00.read_json(t00.ROOT / "docs/runtime.lock.json")
        self.assertEqual(t00.validate_plan(lock)["required_platform"], "macos-arm64")

    def test_plan_rejects_toolchain_install_with_default_self_update(self):
        lock = t00.read_json(t00.ROOT / "docs/runtime.lock.json")
        lock["commands"]["rust_toolchain"].remove("--no-self-update")
        with self.assertRaisesRegex(ValueError, "must disable rustup self-update"):
            t00.validate_plan(lock)

    def test_plan_rejects_self_update_flag_outside_install_payload(self):
        lock = t00.read_json(t00.ROOT / "docs/runtime.lock.json")
        preparation = lock["commands"]["rust_toolchain"]
        preparation.remove("--no-self-update")
        preparation.insert(0, "--no-self-update")
        with self.assertRaisesRegex(ValueError, "must disable rustup self-update"):
            t00.validate_plan(lock)

    def npm_fixture(self):
        package = dict(version="1.0.0", license="MIT", integrity="sha512-synthetic",
                       resolved="https://example.test/synthetic-1.0.0.tgz")
        lock = self.root / "package-lock.json"
        lock.write_text(json.dumps(dict(packages={"": {}, "node_modules/synthetic": package})))
        installed = self.root / "node_modules/synthetic"
        installed.mkdir(parents=True)
        (installed / "package.json").write_text(json.dumps(dict(version="1.0.0", license="MIT")))
        inventory = dict(package_lock_sha256=t00.file_hash(lock),
                         packages=[dict(path="node_modules/synthetic", materials=[], **package)])
        return inventory, installed

    def test_npm_license_inventory_covers_installed_dependency(self):
        inventory, _ = self.npm_fixture()
        actual = t00.validate_npm_inventory(self.root, inventory)
        self.assertEqual((actual["locked"], actual["installed"]), (1, 1))

    def test_npm_required_dependency_missing_fails(self):
        inventory, installed = self.npm_fixture()
        (installed / "package.json").unlink()
        with self.assertRaisesRegex(ValueError, "required dependency missing"):
            t00.validate_npm_inventory(self.root, inventory)

    def test_npm_applicable_optional_dependency_missing_fails(self):
        inventory, installed = self.npm_fixture()
        lock_path = self.root / "package-lock.json"
        lock = json.loads(lock_path.read_text())
        lock["packages"]["node_modules/synthetic"]["optional"] = True
        lock_path.write_text(json.dumps(lock))
        inventory["package_lock_sha256"] = t00.file_hash(lock_path)
        (installed / "package.json").unlink()
        with self.assertRaisesRegex(ValueError, "applicable optional dependency missing"):
            t00.validate_npm_inventory(self.root, inventory)

    def test_npm_omitted_license_package_fails(self):
        inventory, _ = self.npm_fixture()
        inventory["packages"] = []
        with self.assertRaisesRegex(ValueError, "coverage differs"):
            t00.validate_npm_inventory(self.root, inventory)

    def test_npm_installed_version_drift_fails(self):
        inventory, installed = self.npm_fixture()
        (installed / "package.json").write_text(json.dumps(dict(version="2.0.0", license="MIT")))
        with self.assertRaisesRegex(ValueError, "installed version differs"):
            t00.validate_npm_inventory(self.root, inventory)

    def test_npm_license_drift_fails(self):
        inventory, installed = self.npm_fixture()
        (installed / "package.json").write_text(json.dumps(dict(version="1.0.0", license="Apache-2.0")))
        with self.assertRaisesRegex(ValueError, "installed license differs"):
            t00.validate_npm_inventory(self.root, inventory)


if __name__ == "__main__":
    unittest.main()
