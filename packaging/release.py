"""Build the personal macOS archive and verify exact dependency notices.

The inventory deliberately includes build tools as well as runtime dependencies.
No dependency is downloaded or upgraded by this command.
"""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import stat
import subprocess
import sys
import tomllib
import uuid

ROOT = Path(__file__).resolve().parents[1]
TARGET = "aarch64-apple-darwin"
INVENTORY = ROOT / "licenses/dependencies.json"
NOTICES = ROOT / "licenses/THIRD_PARTY_NOTICES.txt"
CURRENT = ROOT / "evidence/releases/T23-current.json"
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
    # Preserve each source hash/text in the JSON inventory; use portable LF in
    # the compiled notice so a normal Git checkout does not change its bytes.
    return "".join(parts).replace("\r\n", "\n")


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


def file_hash(path):
    return digest(path.read_bytes())


def tree(folder):
    """Bind executable modes too; reject links/special files inside our bundle."""
    result = {}
    for path in sorted(folder.rglob("*")):
        require(not path.is_symlink(), f"Unexpected bundle link: {path.relative_to(folder)}")
        if path.is_dir():
            continue
        require(path.is_file(), f"Unexpected bundle entry: {path.relative_to(folder)}")
        result[path.relative_to(folder).as_posix()] = dict(sha256=file_hash(path), bytes=path.stat().st_size,
            mode=stat.S_IMODE(path.stat().st_mode))
    require(result, "Empty application tree")
    return result


def check_payload(app, frontend):
    """Check actual shipping bytes, not names of source dependencies."""
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    config = read(ROOT / "src-tauri/tauri.conf.json")
    require(info["CFBundleIdentifier"] == config["identifier"] == "app.yingxiang.video", "Application identity differs")
    require(info["CFBundleShortVersionString"] == config["version"] == "0.1.0"
            and info["CFBundleVersion"] == config["bundle"]["macOS"]["bundleVersion"] == "2", "Display/build version differs")
    require(info["LSMinimumSystemVersion"] == "14.0" and info["NSHighResolutionCapable"], "macOS/DPI metadata differs")
    require(info["NSAppTransportSecurity"] == {"NSAllowsLocalNetworking": True}, "ATS exceptions changed")
    executable = app / "Contents/MacOS" / info["CFBundleExecutable"]
    require(executable.name == "yingxiang-video" and os.access(executable, os.X_OK), "Shipping executable missing")
    allowed = {"Contents/Info.plist", "Contents/MacOS/yingxiang-video", "Contents/_CodeSignature/CodeResources",
               "Contents/Resources/licenses/dependencies.json", "Contents/Resources/licenses/THIRD_PARTY_NOTICES.txt"}
    files = tree(app)
    require(set(files) == allowed, "Unexpected or missing shipping files: " + str(sorted(set(files) ^ allowed)))
    for path in (INVENTORY, NOTICES):
        require((app / "Contents/Resources/licenses" / path.name).read_bytes() == path.read_bytes(), "Bundled license bytes differ")
    forbidden = (str(ROOT).encode(), str(Path.home()).encode() + b"/", b"/Users/", b"/home/", b"C:\\Users\\",
                 b"http://127.0.0.1:5173", b"T01_FIXTURE_CAP", b"T01_FIXTURE_BASE", b"t01_control",
                 b"control/next", b"/probe/cors-denied", b"_webProcessIdentifier", b"_gpuProcessIdentifier",
                 b"_networkProcessIdentifier", b"_topContentInset")
    # Notices contain upstream source paths/URLs as attribution; scan runnable
    # code and frontend separately from those immutable license quotations.
    assets = [path for path in frontend.rglob("*") if path.is_file()]
    require((frontend / "index.html").is_file() and any(path.suffix == ".js" for path in assets), "Local frontend missing")
    for path in [executable, *assets]:
        data = path.read_bytes()
        require(not any(marker in data for marker in forbidden), f"Development path/test bridge in {path.name}")
    for path in assets:
        require(path.suffix in {".html", ".js", ".css", ".svg", ".png", ".webp", ".avif", ".jpg", ".jpeg", ".gif", ".woff", ".woff2", ".ico", ".mp4", ".vtt"}, "Unexpected frontend material")
        if path.suffix in {".html", ".js", ".css"}:
            text = path.read_text(encoding="utf-8")
            if path.suffix == ".html":
                require(not re.search(r'<(?:script|link|img|source)\b[^>]*\b(?:src|href)\s*=\s*[\"\'](?:https?:)?//', text, re.I),
                        f"Remote HTML runtime resource in {path.name}")
            require(not re.search(r'(?:\bsrc\s*=\s*[\"\']|@import\s*[\"\']|\bfrom\s*[\"\']|\bimport\s*\(\s*[\"\'])\s*(?:https?:)?//', text, re.I),
                    f"Remote executable/style dependency in {path.name}")
            require(not re.search(r'url\(\s*[\"\']?(?:https?:)?//', text, re.I), f"Remote CSS asset in {path.name}")
    return dict(version=info["CFBundleShortVersionString"], bundle_version=info["CFBundleVersion"],
                identifier=info["CFBundleIdentifier"], files=len(files), frontend_files=len(assets))


def check_signing(detail, verify_exit, gatekeeper_exit, staple_exit):
    require(all(type(value) is int for value in (verify_exit, gatekeeper_exit, staple_exit)), "Actual signing command exit codes required")
    require(verify_exit == 0 and "Signature=adhoc" in detail and "TeamIdentifier=not set" in detail
            and re.search(r"Sealed Resources version=\d+ rules=\d+ files=\d+", detail), "Complete ad-hoc bundle seal required")
    require("Authority=" not in detail and "Sealed Resources=none" not in detail, "Unexpected signing identity/resource seal")
    require(gatekeeper_exit != 0 and staple_exit != 0, "Ad-hoc package must not claim Developer ID/notarization acceptance")
    return dict(kind="ad-hoc", developer_id_signed=False, notarized=False, sealed_resources=True,
                gatekeeper_accepted=False, notarization_submission="NOT RUN: no Apple credentials used",
                pending=["Apple Developer ID signing and notarization/stapling before a separately authorized public release",
                         "Clean macOS lifecycle under default security settings"])


def build():
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "Original resource runner required")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "macOS arm64 build host required")
    require(Path.cwd().resolve() == ROOT, "Project root differs")
    require("TAURI_CONFIG" not in os.environ and not any(key.startswith("VITE_") for key in os.environ),
            "Production build requires unmodified configuration")
    sys.path.insert(0, str(ROOT / "scripts"))
    import workflow
    source, protected = workflow.snapshot(), workflow.protected_hash()
    folder = ROOT / "evidence/releases" / ("T23-" + dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S") + "-" + uuid.uuid4().hex[:8])
    folder.mkdir(parents=True)
    commands = []
    require("CARGO_ENCODED_RUSTFLAGS" not in os.environ, "Encoded Rust flags would override the required path mapping")
    overrides = dict(ROLLDOWN_WORKER_THREADS="2", RAYON_NUM_THREADS="2",
                     RUSTFLAGS=(os.environ.get("RUSTFLAGS", "") + f" --remap-path-prefix={Path.home()}=/build-user").strip())

    def command(argv, timeout=600, required=True):
        argv = workflow.runtime.resolve_argv([str(arg) for arg in argv])
        log = folder / f"command-{len(commands) + 1}.log"
        row = dict(argv=argv, log=log.relative_to(ROOT).as_posix(), exit=None)
        commands.append(row)
        print(json.dumps(dict(argv=argv, log=row["log"]), ensure_ascii=False), flush=True)
        with log.open("w", encoding="utf-8") as stream:
            # No new process session: the original runner owns this whole tree.
            result = subprocess.run(argv, cwd=ROOT, env=dict(os.environ, **overrides), stdout=stream,
                                    stderr=subprocess.STDOUT, timeout=timeout, check=False)
        row.update(exit=result.returncode, sha256=file_hash(log))
        require(not required or result.returncode == 0, f"Command exit {result.returncode}; {row['log']}")
        return log.read_text(encoding="utf-8", errors="replace")

    verify()
    command(["npm", "run", "build"], timeout=600)
    # Exclude the development server URL from the embedded production context.
    command(["cargo", "tauri", "build", "--ci", "--config", '{"build":{"devUrl":null}}', "--bundles", "app", "--no-sign", "--", "--locked", "--offline"], timeout=6000)
    image = folder / "image-root"
    image.mkdir()
    app = image / "映象视频.app"
    shutil.copytree(ROOT / "target/release/bundle/macos/映象视频.app", app, symlinks=True)
    frontend = folder / "frontend"
    shutil.copytree(ROOT / "dist", frontend, symlinks=True)
    command(["/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", app])
    command(["/usr/bin/codesign", "--verify", "--deep", "--strict", app])
    verification = commands[-1]["exit"]
    detail = command(["/usr/bin/codesign", "--display", "--verbose=4", app])
    gatekeeper = command(["/usr/sbin/spctl", "--assess", "--type", "execute", "--verbose=4", app], required=False)
    gatekeeper_exit = commands[-1]["exit"]
    stapler = command(["/usr/bin/xcrun", "stapler", "validate", app], required=False)
    staple_exit = commands[-1]["exit"]
    signing = check_signing(detail, verification, gatekeeper_exit, staple_exit)
    signing.update(detail=detail, gatekeeper_exit=gatekeeper_exit, gatekeeper_output=gatekeeper,
                   staple_exit=staple_exit, staple_output=stapler)
    payload = check_payload(app, frontend)
    # Store an isolated copy; later Cargo/T01 builds can replace target freely.
    (image / "Applications").symlink_to("/Applications", target_is_directory=True)
    stem = "YingxiangVideo-0.1.0-build2-macos-arm64-personal"
    archive = folder / (stem + ".zip")
    dmg = folder / (stem + ".dmg")
    command(["/usr/bin/ditto", "-c", "-k", "--norsrc", "--keepParent", app, archive])
    command(["/usr/bin/hdiutil", "create", "-volname", "YingxiangVideo", "-srcfolder", image, "-format", "UDZO", dmg])
    command(["/usr/bin/hdiutil", "verify", dmg])
    # Preserve the actually tested predecessor, not a second copy of build 2.
    prior_report = read(ROOT / "evidence/acceptance/T22-startup-ui.json")
    require(prior_report["counts"]["passed"] > 0 and prior_report["counts"]["failed"] == 0
            and all(row["result"] == "PASS" for row in prior_report["summaries"]), "Successful T22 predecessor report required")
    prior = prior_report["observed"]["build"]["production"]
    previous_app = ROOT / prior["path"]
    previous_files = tree(previous_app)
    require({name: row["sha256"] for name, row in previous_files.items()} == prior["files"], "T22 predecessor bytes changed")
    previous_info = plistlib.loads((previous_app / "Contents/Info.plist").read_bytes())
    require(previous_info["CFBundleShortVersionString"] == "0.1.0" and previous_info["CFBundleVersion"] == "1"
            and previous_info["CFBundleIdentifier"] == payload["identifier"], "Real build 1 predecessor required")
    previous_zip = folder / "T22-upgrade-fixture-0.1.0-build1.zip"
    command(["/usr/bin/ditto", "-c", "-k", "--norsrc", "--keepParent", previous_app, previous_zip])
    require(workflow.snapshot() == source and workflow.protected_hash() == protected, "Source/baseline changed during build")
    files = tree(app)
    manifest = dict(schema_version=2, card="T0.27", utc=dt.datetime.now(dt.timezone.utc).isoformat(),
        target="macos-arm64", scope="personal use only; not public distribution", source=source, protected=protected,
        app=app.relative_to(ROOT).as_posix(), frontend=frontend.relative_to(ROOT).as_posix(),
        files=files, frontend_files=tree(frontend), logical_bytes=sum(row["bytes"] for row in files.values()),
        allocated_bytes=sum(path.stat().st_blocks * 512 for path in app.rglob("*") if path.is_file()),
        disk_measurement="Logical file bytes and OS-reported file st_blocks*512; directory/filesystem metadata adds overhead",
        archives=[dict(path=path.relative_to(ROOT).as_posix(), sha256=file_hash(path), bytes=path.stat().st_size) for path in (archive, dmg)],
        locks={name: file_hash(ROOT / name) for name in read(INVENTORY)["locks"]},
        inventory_sha256=file_hash(INVENTORY), notices_sha256=file_hash(NOTICES),
        upgrade_from=dict(version="0.1.0", bundle_version="1", files=previous_files,
            app=prior["path"], source=prior_report["source_before"],
            report="evidence/acceptance/T22-startup-ui.json", report_sha256=file_hash(ROOT / "evidence/acceptance/T22-startup-ui.json"),
            archive=dict(path=previous_zip.relative_to(ROOT).as_posix(), sha256=file_hash(previous_zip), bytes=previous_zip.stat().st_size)),
        payload=payload, signing=signing, commands=commands, build_environment=overrides,
        lifecycle="NOT RUN: clean macOS target pending", archive_reproduction="Same argv and payload; ZIP timestamps/DMG filesystem UUID may differ")
    path = folder / "manifest.json"
    path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    CURRENT.write_text(json.dumps(dict(manifest=path.relative_to(ROOT).as_posix(), sha256=file_hash(path)), indent=2) + "\n", encoding="utf-8")
    print("T23_MANIFEST_SHA256:" + file_hash(path), flush=True)
    print(json.dumps(dict(manifest=path.relative_to(ROOT).as_posix(), logical_bytes=manifest["logical_bytes"], archives=manifest["archives"])), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--refresh-licenses", action="store_true")
    mode.add_argument("--verify-licenses", action="store_true")
    mode.add_argument("--build", action="store_true")
    parser.add_argument("--output-dir", type=Path)
    args = parser.parse_args()
    if args.build:
        require(args.output_dir is None, "--output-dir applies only to --refresh-licenses")
        build()
    elif args.refresh_licenses:
        refresh((args.output_dir or ROOT / "evidence/release-materials").resolve())
    else:
        require(args.output_dir is None, "--output-dir applies only to --refresh-licenses")
        verify()
