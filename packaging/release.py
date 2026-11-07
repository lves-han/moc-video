"""Collect exact installed dependency notices; invoke through scripts/run.py.

The inventory deliberately includes build tools as well as runtime dependencies.
No dependency is downloaded or upgraded by this command.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TARGET = "aarch64-apple-darwin"
INVENTORY = ROOT / "licenses/dependencies.json"
NOTICES = ROOT / "licenses/THIRD_PARTY_NOTICES.txt"
LICENSE_NAME = re.compile(r"^(?:third[-_ ]?party[-_ ]?)?(licen[sc]e|notices?|copying|copyright|unlicen[sc]e)([._-].*|$)", re.I)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(value, message):
    if not value:
        raise ValueError(message)


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def license_texts(folder, extra=(), package_key=None):
    paths = {path for path in folder.rglob("*") if path.is_file() and LICENSE_NAME.match(path.name)
             and "node_modules" not in path.relative_to(folder).parts}
    paths.update(folder / name for name in extra if name)
    if not paths and package_key:
        supplemental = read(ROOT / "licenses/supplemental/sources.json").get(package_key, [])
        require(supplemental, f"No license text or attributed supplement in {folder.name}")
        rows = []
        for row in supplemental:
            path = ROOT / row["path"]
            require(path.resolve().is_relative_to((ROOT / "licenses/supplemental").resolve()), "Supplement outside license directory")
            data = path.read_bytes()
            require(digest(data) == row["sha256"], f"Supplement hash changed: {package_key}")
            rows.append({**row, "text": data.decode("utf-8-sig")})
        return rows
    require(paths, f"No license text in {folder.name}")
    rows = []
    for path in sorted(paths):
        require(path.resolve().is_relative_to(folder.resolve()), f"License outside package: {folder.name}")
        data = path.read_bytes()
        text = data.decode("utf-8-sig")
        require(text.strip(), f"Empty license: {folder.name}/{path.name}")
        rows.append(dict(path=path.relative_to(folder).as_posix(), sha256=digest(data), text=text))
    return rows


def collect():
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "Original resource runner required")
    npm_lock = read(ROOT / "package-lock.json")
    installed = read(ROOT / "node_modules/.package-lock.json")["packages"]
    packages = []
    for name, locked in sorted(npm_lock["packages"].items()):
        if not name:
            continue
        folder = ROOT / name
        if not folder.exists():
            require(locked.get("optional") and (locked.get("os") or locked.get("cpu")),
                    f"Required npm package missing: {name}")
            continue
        actual = read(folder / "package.json")
        require(actual["version"] == locked["version"] == installed[name]["version"], f"npm version mismatch: {name}")
        require(locked.get("integrity") and locked["integrity"] == installed[name].get("integrity"), f"npm integrity missing/mismatched: {name}")
        expression = actual.get("license") or locked.get("license")
        require(expression, f"npm license expression missing: {name}")
        packages.append(dict(ecosystem="npm", name=actual["name"], version=actual["version"],
            package_path=name, source=locked.get("resolved"), integrity=locked["integrity"], license=expression,
            scope="build" if locked.get("dev") else "runtime", texts=license_texts(folder, package_key=f"{actual['name']}@{actual['version']}")))
    command = ["cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", TARGET,
               "--manifest-path", "src-tauri/Cargo.toml"]
    result = subprocess.run(command, cwd=ROOT, check=True, capture_output=True, text=True, timeout=120)
    metadata = json.loads(result.stdout)
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    reached, pending = set(), [metadata["resolve"]["root"]]
    while pending:
        key = pending.pop()
        if key in reached:
            continue
        reached.add(key)
        pending.extend(dep["pkg"] for dep in nodes[key]["deps"]
                       if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]))
    lock = tomllib.loads((ROOT / "src-tauri/Cargo.lock").read_text())
    locked_crates = {(p["name"], p["version"], p.get("source")): p for p in lock["package"]}
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["id"] not in reached or package["id"] == metadata["resolve"]["root"]:
            continue
        locked = locked_crates[(package["name"], package["version"], package["source"])]
        folder = Path(package["manifest_path"]).parent
        archive = folder.parents[2] / "cache" / folder.parent.name / (folder.name + ".crate")
        require(archive.is_file() and digest(archive.read_bytes()) == locked["checksum"], f"Crate checksum mismatch/missing: {folder.name}")
        require(package["license"], f"Crate license expression missing: {folder.name}")
        packages.append(dict(ecosystem="cargo", name=package["name"], version=package["version"],
            source=package["source"], checksum=locked["checksum"], license=package["license"],
            scope="runtime-and-build-closure", texts=license_texts(folder, [package.get("license_file")],
                package_key=f"cargo:{package['name']}@{package['version']}")))
        if package["name"] == "libsqlite3-sys":
            amalgamation = folder / "sqlite3/sqlite3.c"
            data = amalgamation.read_bytes()
            content = data.decode("utf-8")
            position = content.index("The author disclaims copyright")
            notice = content[content.rfind("/*", 0, position):content.index("*/", position) + 2]
            version = re.search(r'#define SQLITE_VERSION\s+"([^"]+)"', content).group(1)
            packages.append(dict(ecosystem="bundled-source", name="SQLite", version=version,
                source="https://www.sqlite.org/copyright.html", license="LicenseRef-SQLite-Public-Domain",
                source_crate=f"libsqlite3-sys@{package['version']}", source_file_sha256=digest(data),
                scope="runtime-bundled-c", texts=[dict(path="sqlite3/sqlite3.c copyright header",
                    sha256=digest(notice.encode()), text=notice)]))
    locks = {name: digest((ROOT / name).read_bytes()) for name in ("package-lock.json", "src-tauri/Cargo.lock")}
    return dict(schema_version=1, target=TARGET, locks=locks, cargo_metadata_argv=command,
        scope="Installed locked npm runtime/build packages and target Cargo non-dev closure including build dependencies; optional foreign-target npm binaries excluded. Alternatives are preserved. Packages omitting notices use hash-bound, individually attributed supplements. vdirs publishes an MIT declaration without a notice file; its declaration and standard MIT reference are distinctly labelled, not represented as an upstream copyright notice.",
        packages=packages)


def notices(inventory):
    parts = ["映象视频 — third-party license and copyright notices\n",
             inventory["scope"] + "\n"]
    for package in inventory["packages"]:
        parts.append(f"\n{'=' * 72}\n{package['ecosystem']}: {package['name']} {package['version']}\nLicense: {package['license']}\nSource: {package['source']}\n")
        for row in package["texts"]:
            parts.append(f"\n--- {row['path']} (SHA256 {row['sha256']}) ---\n")
            if row.get("source_url"):
                parts.append(f"Provenance: {row['source_url']}\nMaterial kind: {row['kind']}\n")
            parts.append(row["text"] + "\n")
    return "".join(parts)


def refresh(output_dir):
    inventory = collect()
    output_dir.mkdir(parents=True, exist_ok=True)
    inventory_path = output_dir / INVENTORY.name
    notices_path = output_dir / NOTICES.name
    inventory_path.write_text(json.dumps(inventory, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    notices_path.write_text(notices(inventory), encoding="utf-8")
    print(json.dumps(dict(packages=len(inventory["packages"]), inventory_sha256=digest(inventory_path.read_bytes()),
                         notices_sha256=digest(notices_path.read_bytes()), output=str(output_dir.relative_to(ROOT)))))


def verify():
    inventory = collect()
    require(read(INVENTORY) == inventory, "Dependency inventory differs from current locks/materials")
    require(NOTICES.read_bytes() == notices(inventory).encode("utf-8"), "Notices differ from verified inventory")
    print(json.dumps(dict(result="PASS", packages=len(inventory["packages"]),
        inventory_sha256=digest(INVENTORY.read_bytes()), notices_sha256=digest(NOTICES.read_bytes()))))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--refresh-licenses", action="store_true")
    mode.add_argument("--verify-licenses", action="store_true")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "evidence/release-materials")
    args = parser.parse_args()
    refresh(args.output_dir.resolve()) if args.refresh_licenses else verify()
