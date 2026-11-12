"""T25: reproduce the documented delivery in a clean directory, under run.py."""
import argparse
import json
from pathlib import Path
import re
import plistlib
import stat
import zipfile

import t01
import t10
import t14
import t23
from t00 import file_hash, read_json, require, workflow

ROOT = Path(__file__).resolve().parents[2]
IDS = ["U06", "R33", "R51", "C-PATH", "REQ-T25"]
BEHAVIORS = {"resource-ownership", "final-materials", "current-build-receipt", "installer-roundtrip",
             "clean-readme-reproduction", "evidence-counterexamples"}


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.36", "T25", IDS
    DEPLOYMENT = "Fresh source extraction and sequential local builds/Host smoke; archive inspection; no VM/cluster launched"
    DEFERRED = [dict(target=target, status="NOT RUN", reason="ADR-005/006; no target run")
                for target in [*t01.DEFERRED, "macos14-arm64", "Android", "iOS"]]
    command = t23.Acceptance.command

    def save(self):
        actual = {row["id"].split("/", 1)[1] for row in self.cases}
        self.case("required-behavior-inventory", IDS, sorted(BEHAVIORS),
                  lambda: require(BEHAVIORS <= actual, "Missing delivery behavior cases"))
        result = super().save()
        immutable = self.folder / "report.json"
        immutable.write_bytes(self.report_path.read_bytes())
        print("T25_EVIDENCE_SHA256:" + file_hash(immutable), flush=True)
        return result


def recipe(text):
    match = re.search(r'<!-- t25-reproduce -->\s*```json\s*\n(.*?)\n```', text, re.S)
    require(match, "README reproducible argv block missing")
    rows = json.loads(match[1])
    require([row["id"] for row in rows] == ["npm-init", "cargo-init", "frontend-build", "native-build", "native-smoke"],
            "README must cover initialization, both builds, native execution and diagnostics")
    for row in rows:
        require(isinstance(row["argv"], list) and row["argv"] and all(isinstance(arg, str) for arg in row["argv"])
                and type(row["timeout"]) is int and 0 < row["timeout"] <= 4200, "Invalid README argv/timeout")
    return rows


def materials(root=ROOT):
    lock = read_json(root / "docs/runtime.lock.json")["final_delivery"]
    require((lock["card"], lock["version"], lock["macos_bundle_version"], lock["target"])
            == ("T0.36", "0.1.0", "2", "macos-arm64"), "Final delivery identity differs")
    for name, digest in lock["locks"].items():
        require(file_hash(root / name) == digest, "Final lock/material changed: " + name)
    require({"package-lock.json", "src-tauri/Cargo.lock", "licenses/dependencies.json",
             "licenses/THIRD_PARTY_NOTICES.txt"} <= lock["locks"].keys(), "Required delivery materials absent")
    require(lock["reports"] == {"A1": "evidence/acceptance/T25-all.json", "A2": "evidence/acceptance/T25-lifecycle.json"},
            "Delivery report paths differ")
    historical = []
    for row in lock["historical_evidence"]:
        path = root / row["path"]
        report = read_json(path)
        require(file_hash(path) == row["sha256"] and report["card"] == row["card"]
                and report["source_before"] == row["source"], "Historical evidence identity changed")
        require(report["counts"]["passed"] > 0 and report["counts"]["failed"] == 0
                and report["summaries"] and all(item["result"] == "PASS" for item in report["summaries"]),
                "Historical report lacks successful behavior evidence")
        receipt = read_json(root / report["observed"]["resources"]["receipt"])
        require(receipt["status"] == "finished" and receipt["exit"] == receipt["raw_exit"] == 0
                and receipt["card"] == row["card"] and receipt["source"] == receipt["after_source"] == row["source"]
                and file_hash(root / receipt["log"]) == receipt["log_hash"], "Historical receipt/log changed")
        historical.append(dict(row, interpretation="Historical source and stated scope only"))
    require({row["card"] for row in historical} == {"T0.35", "T0.37"}, "Both prior evidence scopes required")
    require(all(row["status"] == "NOT RUN" for row in lock["deferred"]) and len(lock["deferred"]) == 5,
            "Unrun desktop/minimum OS targets must remain NOT RUN")
    require([row["card"] for row in lock["mobile"]] == [f"T2.{i}" for i in range(1, 7)]
            and all(row["status"] == "NOT RUN" for row in lock["mobile"]), "Mobile checklist/status missing")
    readme = (root / "README.md").read_text()
    for marker in ("packaging/T23.md", "docs/runtime.lock.json", "docs/release-evidence/T25.md", "NOT RUN",
                   "e6c9b946636adb29afe1bc849f536d2244b1dc38", "T25-lifecycle", "T2.1", "诊断"):
        require(marker in readme, "README delivery reference missing: " + marker)
    require((root / "docs/release-evidence/T25.md").is_file(), "Delivery index missing")
    return dict(version_lock=dict(path="docs/runtime.lock.json", sha256=file_hash(root / "docs/runtime.lock.json")),
                materials=lock["locks"], historical=historical, deferred=lock["deferred"], mobile=lock["mobile"],
                recipe=recipe(readme))


def source_archive(folder, root=ROOT):
    # Include human documentation as well as snapshot inputs. Evidence, caches,
    # dependencies and app data are ignored by Git and never enter the archive.
    names = sorted(set(workflow.git("ls-files", "-z", "--cached", "--others", "--exclude-standard").split("\0")) - {""})
    archive, extracted = folder / "MocVideo-0.1.0-build2-source.zip", folder / "clean-source"
    require(not extracted.exists(), "Clean reproduction directory already exists")
    files = {}
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as stream:
        for name in names:
            path = root / name
            require(not path.is_symlink(), "Source archive symlink requires explicit review: " + name)
            if not path.is_file():
                continue
            require(not Path(name).is_absolute() and ".." not in Path(name).parts, "Unsafe source path")
            files[name] = dict(sha256=file_hash(path), bytes=path.stat().st_size, mode=stat.S_IMODE(path.stat().st_mode))
            stream.write(path, name)
    require({"README.md", "src-tauri/Cargo.lock", "package-lock.json", "docs/runtime.lock.json"} <= files.keys(),
            "Source archive incomplete")
    extracted.mkdir()
    with zipfile.ZipFile(archive) as stream:
        require(stream.testzip() is None and set(stream.namelist()) == files.keys(), "Source ZIP CRC/inventory differs")
        stream.extractall(extracted)
    for name, row in files.items():
        path = extracted / name
        path.chmod(row["mode"])
        require(file_hash(path) == row["sha256"] and path.stat().st_size == row["bytes"], "Extracted source differs: " + name)
    require(not any((extracted / name).exists() for name in ("target", "dist", "node_modules", ".venv", ".git")),
            "Reproduction directory contains pre-existing dependencies/builds")
    index = folder / "source-files.json"
    index.write_text(json.dumps(files, ensure_ascii=False, indent=2) + "\n")
    return extracted, dict(archive=dict(path=archive.relative_to(ROOT).as_posix(), sha256=file_hash(archive), bytes=archive.stat().st_size),
        index=dict(path=index.relative_to(ROOT).as_posix(), sha256=file_hash(index)), files=len(files), directory=extracted.relative_to(ROOT).as_posix())


def reproduce(run):
    source, record = source_archive(run.folder)
    run.observed["clean_source"] = record
    env = dict(CARGO_TARGET_DIR=str(source / "target"), CARGO_BUILD_JOBS="2", RUST_TEST_THREADS="1",
               ROLLDOWN_WORKER_THREADS="2", RAYON_NUM_THREADS="2", RUSTFLAGS=f"--remap-path-prefix={Path.home()}=/build-user")
    record["environment"] = env
    rows = recipe((source / "README.md").read_text())
    for row in rows:
        output = run.command(row["argv"], timeout=row["timeout"], cwd=source, env=env)
        if row["id"] == "native-smoke":
            require("test result: ok. 1 passed; 0 failed" in output, "Fresh Host execution did not run its smoke assertion")
    for name in ("package-lock.json", "src-tauri/Cargo.lock"):
        require(file_hash(source / name) == file_hash(ROOT / name), "Reproduction changed version lock: " + name)
    app = source / "target/release/bundle/macos/墨客视频.app"
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    require((info["CFBundleShortVersionString"], info["CFBundleVersion"], info["CFBundleIdentifier"])
            == ("0.1.0", "2", "app.moc.video"), "Clean build application identity differs")
    record["payload"] = t01.validate_production(source / "dist", app / "Contents/MacOS/moc-video")
    record["built_app"] = app.relative_to(ROOT).as_posix()
    record["scope"] = "README dependency init, both builds, real native Host smoke and diagnostics; GUI lifecycle is A2"
    return record


def installer(run, manifest, path):
    payload = t23.payload(manifest, path)
    archives = t23.archives(run, manifest, path)
    signing = t23.signing(run, manifest, path)
    run.command(["{python}", "-B", "packaging/release.py", "--verify-licenses"])
    return dict(payload=payload, archives=archives, signing=signing)


def oracles(run):
    output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t25.py", "-v"])
    require(re.search(r"Ran [1-9]\d* tests?", output) and re.search(r"(?m)^OK\s*$", output), "Delivery counterexamples did not pass")
    return dict(delivery="nonzero evidence counterexamples", installer=t23.oracles(run,
        names=("test_final_installer_and_original_build_receipt_belong_to_t036",)))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report.resolve(), args.junit.resolve())
    owned = run.case("resource-ownership", IDS, "Original T0.36 receipt, root/venv and approved sampled resource backend", lambda: t10.resources(run, card=run.CARD))
    bound = []
    def bind():
        bound.extend(t23.artifact(run))
        return run.observed["release"]
    def document():
        run.observed["delivery"] = materials()
        return run.observed["delivery"]
    def missing():
        require(False, "NOT RUN: original resource owner/current installer missing")
    run.case("final-materials", IDS, "Traceable exact locks/notices, historical scope, platform status and mobile checklist", document if owned else missing)
    run.case("current-build-receipt", IDS, "Current T0.36 source/protected manifest and original build receipt", bind if owned else missing)
    run.case("installer-roundtrip", ["R33", "R51", "REQ-T25"], "Current payload, ZIP/DMG trees, signatures, notices and upgrade material", lambda: installer(run, *bound) if bound else missing())
    run.case("evidence-counterexamples", IDS, "Missing/stale/overclaimed delivery evidence rejected", lambda: oracles(run) if owned else missing())
    run.case("clean-readme-reproduction", IDS, "Fresh source ZIP extraction executes exact README init/build/run/diagnostics argv", lambda: reproduce(run) if owned and bound else missing())
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
