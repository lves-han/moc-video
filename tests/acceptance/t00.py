"""T00 environment baseline. Run via the original run.py, without nested runners.

prepare acquires pinned inputs; all validates them and builds Core. The environment
suite detects tools on deferred hosts; negatives exercises synthetic validators.
Results go to evidence; source and production Cloudstream locks stay unchanged.
"""
import argparse
import datetime as dt
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import subprocess
import sys
import time
import tomllib
import unittest
import urllib.error
import urllib.request
import uuid
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import workflow  # Reuse the original source digest, protected baseline and OS backend.

IDS = ("U02", "U06", "R02", "R08", "R09", "R33", "R50", "R57",
       "C-PATH", "C-RESOURCE", "REQ-T00")
CORE_TOOLS = {
    "rustc": r"^rustc (\d+\.\d+\.\d+)\b",
    "cargo": r"^cargo (\d+\.\d+\.\d+)\b",
    "rustup": r"^rustup (\d+\.\d+\.\d+)\b",
    "node": r"^v(\d+\.\d+\.\d+)\s*$",
    "npm": r"^(\d+\.\d+\.\d+)\s*$",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def file_hash(path):
    return sha256(Path(path).read_bytes())


def write_json(path, value):
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def validate_tool(name, row):
    require(name in CORE_TOOLS, f"unknown version tool: {name}")
    require(row.get("status") == "version_observed" and row.get("exit") == 0,
            f"{name}: successful official version command required")
    argv = row.get("argv", [])
    require(len(argv) >= 2 and Path(argv[0]).is_absolute() and argv[-1] == "--version",
            f"{name}: absolute version argv required")
    match = re.search(CORE_TOOLS[name], row.get("stdout", ""), re.M)
    require(match is not None, f"{name}: unrecognized version output")
    return match.group(1)


def validate_registry(rows, inventory):
    require(isinstance(rows, list) and len(rows) == 7, "registry must contain seven packages")
    names = [r["internalName"] for r in rows]
    expected = {p["internal_name"]: p for p in inventory["plugins"]}
    require(len(set(names)) == 7 and set(names) == set(expected), "registry package identities differ")
    for row in rows:
        original = expected[row["internalName"]]
        require(type(row["version"]) is int and row["version"] == original["version"], "registry version differs")
        require(type(row["fileSize"]) is int and row["fileSize"] == original["downloaded_bytes"], "registry length differs")
        require(row["url"] == original["url"], "registry artifact URL differs")
    return names


def validate_artifact(path, expected):
    data = Path(path).read_bytes()  # A missing package is a failure, even when six others pass.
    require(len(data) == expected["downloaded_bytes"] == expected["index_file_size"],
            f"{path.name}: artifact length differs")
    require(sha256(data) == expected["sha256"], f"{path.name}: artifact SHA256 differs")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        require(sorted(archive.namelist()) == sorted(expected["entries"]), f"{path.name}: ZIP entries differ")
        require(archive.testzip() is None, f"{path.name}: ZIP CRC failed")
        manifest = archive.read("manifest.json")
        require(json.loads(manifest) == expected["plugin_manifest"], f"{path.name}: manifest differs")
        require(bool(archive.read("classes.dex")), f"{path.name}: empty DEX")
    return dict(bytes=len(data), sha256=sha256(data), manifest=json.loads(manifest)), manifest


def validate_plan(lock):
    require(lock["schema_version"] == 2 and lock["card"] == "T0.1", "unsupported T00 environment schema")
    require(lock["status"] == "WIP", "committed environment describes the plan; results belong in current reports")
    for name in CORE_TOOLS:
        validate_tool(name, lock["observed_tools"][name])
    require(set(lock["observed_tools"]) == set(CORE_TOOLS) | {"xcode-select", "xcrun", "java"},
            "unknown tool in observed environment table")
    for name in ("tauri", "hls.js", "cloudstream"):
        candidate = lock["candidate_versions"][name]
        require(candidate["status"] == "candidate" and candidate.get("production_version") is None,
                f"{name}: untested candidate promoted to production")
        require(candidate["source"].startswith("https://"), f"{name}: candidate source missing")
        if name != "cloudstream":
            require(re.fullmatch(r"\d+\.\d+\.\d+", candidate["exact_version"]) is not None,
                    f"{name}: exact candidate version missing")
    require(lock["candidate_versions"]["cloudstream"]["commit"] ==
            "a7ac454af8fbd64574c96847a936905855911720", "Cloudstream candidate commit differs")
    require(lock["candidate_versions"]["cloudstream"].get("production_commit") is None,
            "Cloudstream ABI loading has not been verified")
    targets = lock["platforms"]
    expected_targets = {"macos-arm64", "macos-x86_64", "windows-x86_64", "linux-x11", "linux-wayland"}
    require(len(targets) == 5 and {p["id"] for p in targets} == expected_targets, "platform matrix incomplete")
    require({p["id"] for p in targets if p["required"]} == {"macos-arm64"}, "required platform differs from ADR-005/006")
    for target in targets:
        require(target["status"] == "NOT RUN" and target["reason"], "platform requires a truthful pending reason")
        require(target["machine"] or target["ci"] or target["needed_machine"], "machine/CI or blocked prerequisite missing")
        command = lock["commands"]["acceptance"] if target["required"] else lock["commands"]["environment"][target["id"]]
        require(target["reproduce_argv"] == command, "platform reproduction argv differs")
    android = lock["android"]
    require(android["status"] == "BLOCKED" and android["resume_cards"] == ["T02", "T04"], "Android branch gate missing")
    require(android["system_image"]["package"] == "system-images;android-35;default;arm64-v8a", "API35 image plan differs")
    require(android["system_image"]["status"] == "planned" and android["system_image"]["installed_verified"] is False,
            "unverified Android image marked installed")
    require(android["avd"] == {"cores": 2, "memory_mib": 3072, "status": "planned"}, "AVD starting budget differs")
    for name in ("jdk", "gradle", "emulator", "sdk_tools", "platform_tools"):
        require(android["plan"][name]["selection"] and android["plan"][name]["status"] == "pending T02/T04",
                f"Android {name} plan missing")
    require(lock["artifacts"]["expected_count"] == 7 and lock["artifacts"]["verified_count"] == 0,
            "committed plan must retain NOT RUN package verification")
    return dict(platforms=5, required_platform="macos-arm64", cloudstream="candidate, ABI NOT RUN")


def validate_npm_inventory(root, inventory):
    lock = read_json(root / "package-lock.json")
    require(inventory["package_lock_sha256"] == file_hash(root / "package-lock.json"), "license inventory lock hash differs")
    packages = {p["path"]: p for p in inventory["packages"]}
    expected = {key: row for key, row in lock["packages"].items() if key}
    require(len(packages) == len(inventory["packages"]) and set(packages) == set(expected), "license package coverage differs")
    installed, pending = 0, []
    for name, item in expected.items():
        row = packages[name]
        require(row["version"] == item["version"] and row["license"] == item["license"], f"{name}: license/version differs")
        require(row["integrity"] == item["integrity"] and row["resolved"] == item["resolved"], f"{name}: acquisition identity differs")
        metadata_path = root / name / "package.json"
        if not metadata_path.exists():
            require(item.get("optional") is True, f"{name}: required dependency missing")
            cpu = {"aarch64": "arm64", "x86_64": "x64", "AMD64": "x64"}.get(platform.machine(), platform.machine())
            def matches(choices, value):
                positives = [c for c in choices if not c.startswith("!")]
                return "!" + value not in choices and (not positives or value in positives)
            applicable = matches(item.get("os", []), sys.platform) and matches(item.get("cpu", []), cpu)
            require(not applicable, f"{name}: applicable optional dependency missing")
            pending.append(dict(package=name, reason="optional platform package absent on this host"))
            continue
        metadata = read_json(metadata_path)
        require(metadata["version"] == item["version"], f"{name}: installed version differs")
        require(metadata.get("license") == row["license"], f"{name}: installed license differs")
        installed += 1
        for material in row["materials"]:
            require(file_hash(root / material["path"]) == material["sha256"], f"{name}: license material differs")
        if not row["materials"]:
            pending.append(dict(package=name, reason="license declared; full text collection required in T23"))
    return dict(locked=len(expected), installed=installed, release_pending=pending)


class Acceptance:
    def __init__(self, suite, report, junit):
        self.suite, self.report_path, self.junit_path = suite, report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T00-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.cases, self.commands, self.observed = [], [], {}
        self.source = workflow.snapshot()

    def command(self, argv, timeout=120):
        argv = workflow.runtime.resolve_argv([str(a) for a in argv])
        path = self.folder / f"command-{len(self.commands) + 1}.log"
        # Rolldown uses separate native and Rayon pools; both follow the approved jobs=2.
        overrides = dict(ROLLDOWN_WORKER_THREADS="2", RAYON_NUM_THREADS="2")
        record = dict(argv=argv, cwd=str(ROOT), env_overrides=overrides, exit=None, log=path.relative_to(ROOT).as_posix())
        self.commands.append(record)
        started = time.monotonic()
        with path.open("w", encoding="utf-8") as log:
            log.write(json.dumps(argv, ensure_ascii=False) + "\n")
            log.flush()
            try:
                result = subprocess.run(argv, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                                        env=dict(os.environ, **overrides), timeout=timeout, check=False)
                record["exit"] = result.returncode
            except (OSError, subprocess.TimeoutExpired) as error:
                log.write(f"\n{type(error).__name__}: {error}\n")
                raise
            finally:
                record["seconds"] = time.monotonic() - started
        output = path.read_text(encoding="utf-8", errors="replace")
        require(record["exit"] == 0, f"command failed ({record['exit']}): {argv}; log={record['log']}")
        return output.split("\n", 1)[1]

    def case(self, name, requirements, expected, function, inputs=None):
        case = dict(id=f"{requirements[0]}/{name}", requirements=list(requirements),
                    required=True, input=inputs, expected=expected, actual=None, result="FAIL", evidence=[])
        before = len(self.commands)
        try:
            case["actual"] = function()
            case["result"] = "PASS"
        except Exception as error:
            case["actual"] = f"{type(error).__name__}: {error}"
        case["evidence"] = [r["log"] for r in self.commands[before:]]
        self.cases.append(case)
        print(f"{case['id']}: {case['result']} {str(case['actual'])[:260]}", flush=True)
        return case["result"] == "PASS"

    def save(self):
        same_source = workflow.snapshot() == self.source
        self.cases.append(dict(id="C-PATH/source-stability", requirements=["C-PATH", "R57"],
                               required=True,
                               input=self.source, expected="source unchanged during this run",
                               actual=same_source, result="PASS" if same_source else "FAIL", evidence=[]))
        summaries = []
        if self.suite == "all":
            for identifier in IDS:
                children = [c for c in self.cases if identifier in c["requirements"]]
                passed = bool(children) and all(c["result"] == "PASS" or
                                               (c["result"] == "SKIP" and not c["required"])
                                               for c in children)
                summaries.append(dict(id=identifier, result="PASS" if passed else "FAIL",
                                      children=[c["id"] for c in children]))
        counts = dict(required=sum(c["required"] for c in self.cases),
                      ran=sum(c["result"] != "SKIP" for c in self.cases),
                      passed=sum(c["result"] == "PASS" for c in self.cases),
                      failed=sum(c["result"] == "FAIL" for c in self.cases),
                      skipped=sum(c["result"] == "SKIP" for c in self.cases))
        report = dict(card="T0.1", original_card="T00", suite=self.suite,
                      utc_started=self.started, utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(),
                      tested_source_digest=self.source, platform=sys.platform, arch=platform.machine(),
                      os=platform.platform(), observed=self.observed, commands=self.commands,
                      cases=self.cases, summaries=summaries, counts=counts,
                      resources=dict(deployment="single process probes and sequential compiler children; no service/cluster/AVD",
                                     supervision="inherited from original run.py; see observed resource receipt",
                                     cleanup="direct children waited; original runner owns descendant cleanup and the final resource receipt; own output retained under evidence"))
        write_json(self.report_path, report)
        rows = self.cases + summaries
        failures = sum(row["result"] == "FAIL" for row in rows)
        xml = ET.Element("testsuite", name="T00-" + self.suite, tests=str(len(rows)),
                         failures=str(failures), errors="0", skipped=str(counts["skipped"]), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", name=row["id"], classname="T00")
            if row["result"] == "SKIP":
                ET.SubElement(node, "skipped", message=row["actual"])
            elif row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T00 assertion failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return 0 if counts["failed"] == 0 and counts["passed"] > 0 and failures == 0 else 1


def bound_inventory(lock):
    manifest = read_json(ROOT / "docs/SOURCE_MANIFEST.json")
    for name in ("references/plugin-artifact-inventory.json", "references/plugins.snapshot.json"):
        require(file_hash(ROOT / "replica-spec" / name) == manifest[name], f"frozen input differs: {name}")
    inventory_path = ROOT / lock["artifacts"]["inventory"]
    require(file_hash(inventory_path) == lock["artifacts"]["inventory_sha256"], "artifact inventory digest differs")
    inventory = read_json(inventory_path)
    require(len(inventory["plugins"]) == 7 and len({p["internal_name"] for p in inventory["plugins"]}) == 7,
            "artifact inventory must have seven unique packages")
    return inventory


def acquire(url, path, expected_hash=None, expected_size=None, optional=False):
    """Keep existing bytes for revalidation; never repair a corrupt cache silently."""
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        try:
            with urllib.request.urlopen(url, timeout=90) as response:
                data = response.read(16 * 1024 * 1024 + 1)
            require(len(data) <= 16 * 1024 * 1024, f"download too large: {url}")
        except urllib.error.HTTPError as error:
            if optional and error.code == 404:
                return dict(url=url, status="NOT FOUND", http_status=404, release_status="pending license review in T23")
            raise
        if expected_hash:
            require(sha256(data) == expected_hash, f"download SHA256 differs: {url}")
        if expected_size is not None:
            require(len(data) == expected_size, f"download length differs: {url}")
        path.write_bytes(data)
    data = path.read_bytes()
    if expected_hash:
        require(sha256(data) == expected_hash, f"cached SHA256 differs: {path}")
    if expected_size is not None:
        require(len(data) == expected_size, f"cached length differs: {path}")
    return dict(url=url, path=path.relative_to(ROOT).as_posix(), bytes=len(data), sha256=sha256(data), status="downloaded")


def prepare(run, lock):
    inventory = bound_inventory(lock)
    cache = ROOT / lock["artifacts"]["directory"]
    records = []

    def fetch_registry():
        row = acquire(inventory["pinned_registry_url"], cache / "plugins.json", inventory["registry_sha256"])
        records.append(row)
        validate_registry(read_json(cache / "plugins.json"), inventory)
        return row
    run.case("download-registry", ["REQ-T00"], inventory["registry_sha256"], fetch_registry,
             inputs=inventory["pinned_registry_url"])
    for item in inventory["plugins"]:
        def fetch_artifact(item=item):
            path = cache / (item["internal_name"] + ".cs3")
            row = acquire(item["pinned_url"], path, item["sha256"], item["downloaded_bytes"])
            _, manifest = validate_artifact(path, item)
            (cache / (item["internal_name"] + ".manifest.json")).write_bytes(manifest)
            records.append(row)
            return row
        run.case("download-" + item["internal_name"], ["REQ-T00"], item, fetch_artifact, inputs=item["pinned_url"])
    for item in lock["candidate_inputs"]:
        def fetch_candidate(item=item):
            row = acquire(item["url"], cache / item["file"], optional=item.get("optional", False))
            records.append(row)
            return row
        run.case("download-" + item["file"], ["REQ-T00"], "source acquired; optional missing license remains release pending",
                 fetch_candidate, inputs=item["url"])
    write_json(cache / "acquisition.json", dict(utc=dt.datetime.now(dt.timezone.utc).isoformat(),
               complete=all(c["result"] == "PASS" for c in run.cases), inputs=records))
    run.observed["acquisition"] = records


def binding(run, lock):
    declared = lock["binding"]
    require(Path.cwd().resolve() == ROOT == Path(declared["root"]), "working directory differs from authorized root")
    require(Path(sys.prefix) == ROOT / ".venv" and Path(sys.executable).absolute() == Path(declared["python"]),
            "project virtualenv interpreter required")
    require(run.command(["git", "rev-parse", "--show-toplevel"]).strip() == str(ROOT), "Git root differs")
    node = workflow.state("T0.1")
    require(node["base"] == declared["source_baseline"], "original card source baseline differs")
    require(workflow.protected_hash() == node["protected"] == declared["protected"], "protected baseline differs")
    run.command(["git", "merge-base", "--is-ancestor", declared["source_baseline"], "HEAD"])
    require(workflow.current() == "T0.1", "original workflow current card differs")
    require(Path(declared["driver_lock"]) == ROOT / "evidence/drive/driver.lock", "driver lock binding differs")
    require(Path(declared["driver_lock"]).is_file(), "original driver lock file missing")
    driver = workflow.read_json(ROOT / "evidence/drive/state.json")
    live = bool(driver.get("pid") and workflow.runtime.alive(dict(pid=driver["pid"], identity=driver["identity"])))
    return dict(root=str(ROOT), python=sys.executable, baseline=node["base"], protected=node["protected"],
                driver=dict(pid=driver.get("pid"), identity=driver.get("identity"), alive=live),
                driver_note="ADR-010 permits outer acceptance after the development driver exits")


def resources(run, lock):
    cfg = workflow.config()
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "use the original run.py resource wrapper")
    require(str(workflow.heavy_path(cfg)) == lock["binding"]["global_heavy_lock"], "original shared lock differs")
    require(cfg["memory_mib"] == 4096 and cfg["jobs"] == 2, "approved resource budget differs")
    require(os.environ.get("CARGO_BUILD_JOBS") == "2" and os.environ.get("RUST_TEST_THREADS") == "1",
            "inherited build/test concurrency differs")
    require(Path(os.environ["CARGO_TARGET_DIR"]) == ROOT / "target", "build output binding differs")
    process = workflow.runtime.psutil.Process(os.getpid())
    ancestors = {p.pid for p in process.parents()} | {process.pid}
    # The original runner writes payload ownership after launch; wait for that handshake.
    deadline = time.monotonic() + 5
    while True:
        matches = []
        for path in (ROOT / "evidence/runs").glob("*/receipt.json"):
            receipt = read_json(path)
            if (receipt.get("status") == "running" and receipt.get("payload_pid") in ancestors
                    and workflow.runtime.alive(dict(pid=receipt["payload_pid"], identity=receipt["payload_identity"]))):
                matches.append((path, receipt))
        if matches or time.monotonic() >= deadline:
            break
        time.sleep(0.05)
    require(len(matches) == 1, "exactly one current original resource receipt must own this payload")
    path, receipt = matches[0]
    require(receipt["source"] == run.source and receipt["card"] == "T0.1", "resource receipt source/card differs")
    env = receipt["environment"]
    expected_backend = workflow.runtime.backend()
    expected_memory = "cgroup-hard" if expected_backend == "systemd" else "rss-sampled"
    require(env["backend"] == expected_backend and env["memory_enforcement"] == expected_memory
            and env["memory_mib"] == 4096 and env["jobs"] == 2 and env["test_threads"] == 1,
            "actual resource backend/budget differs")
    return dict(receipt=path.relative_to(ROOT).as_posix(), environment=env,
                note=("RSS sampled; transient overage possible; CPU limited by build jobs" if expected_backend == "process-watchdog"
                      else "original systemd scope enforces memory and CPU quota"))


def core_versions(run, lock):
    require(sys.platform == "darwin" and platform.machine() == "arm64", "T00 required target is local macOS arm64 (ADR-005/006)")
    require(int(platform.mac_ver()[0].split(".")[0]) >= 14, "macOS 14+ required")
    versions = {}
    for name in CORE_TOOLS:
        tool = shutil.which(name)
        require(tool is not None, f"missing Core tool: {name}; see runtime acquisition instructions")
        output = run.command([tool, "--version"])
        row = dict(status="version_observed", exit=0, argv=[tool, "--version"], stdout=output)
        versions[name] = validate_tool(name, row)
        require(versions[name] == validate_tool(name, lock["observed_tools"][name]), f"{name}: version drift from baseline")
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]
    require(toolchain["channel"] == versions["rustc"], "Rust toolchain pin differs")
    require(set(toolchain["components"]) == {"rustfmt", "clippy"}, "Rust quality components missing")
    for name in ("rustfmt", "clippy-driver"):
        run.command([name, "--version"])
    run.observed["versions"] = versions
    return versions


def core_builds(run, lock):
    build = run.folder / "build"
    build.mkdir()
    rust_binary, webkit_binary = build / "rust-smoke", build / "webkit-smoke"
    run.command(["rustc", "--edition=2021", str(ROOT / "tests/fixtures/t00/core.rs"), "-o", str(rust_binary)])
    require(run.command([str(rust_binary)]).strip() == "T00 Rust toolchain smoke: 42", "Rust binary output differs")
    run.command(["xcode-select", "-p"])
    sdk_version = run.command(["xcrun", "--show-sdk-version"]).strip()
    clang_version = run.command(["xcrun", "clang", "--version"])
    sdk = run.command(["xcrun", "--sdk", "macosx", "--show-sdk-path"]).strip()
    run.command(["xcrun", "--sdk", "macosx", "clang", "-isysroot", sdk,
                 str(ROOT / "tests/fixtures/t00/webkit.m"), "-framework", "Foundation", "-framework", "WebKit",
                 "-o", str(webkit_binary)])
    require(run.command([str(webkit_binary)]).strip() == "T00 WebKit link: WKWebView available", "WebKit runtime lookup failed")
    plist_path = Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist")
    plist = plistlib.loads(plist_path.read_bytes())
    webview = dict(version=plist["CFBundleShortVersionString"], build=plist["CFBundleVersion"],
                   sdk_version=sdk_version, clang=clang_version, source=str(plist_path),
                   link="PASS", window_and_video="NOT RUN; T01")
    run.observed["webview"] = webview
    before = file_hash(ROOT / "package-lock.json")
    require(before == lock["frontend"]["lock_sha256"], "frontend lock drift before npm ci")
    run.command(["npm", "ci", "--no-audit", "--no-fund"], timeout=900)
    run.command(["npm", "run", "build"], timeout=600)
    require(file_hash(ROOT / "package-lock.json") == before, "npm changed the source lock")
    require((ROOT / "dist/index.html").is_file(), "Vite build output missing")
    return dict(rust="compiled and ran", node="npm ci, type check and Vite build", webkit=webview,
                artifacts={p.name: file_hash(p) for p in (rust_binary, webkit_binary, ROOT / "dist/index.html")},
                tauri_host="NOT RUN; T01/T05 create the app crate and its Cargo.lock")


def android_observations(run, lock):
    android = lock["android"]
    sdk = Path(android["sdk_root"])
    actual = []
    if sdk.is_dir():
        for path in sorted(sdk.rglob("source.properties")):
            properties = {}
            for line in path.read_text(encoding="utf-8").splitlines():
                if "=" in line and not line.startswith("#"):
                    key, value = line.split("=", 1)
                    properties[key.strip()] = value.strip()
            actual.append(dict(path=str(path), sha256=file_hash(path), properties=properties,
                               status="installed metadata; tool execution/AVD NOT RUN"))
    target = sdk / "system-images/android-35/default/arm64-v8a/source.properties"
    def observe_optional(argv):
        try:
            return dict(status="observed", output=run.command(argv))
        except (ValueError, OSError, subprocess.TimeoutExpired) as error:
            return dict(status="BLOCKED", reason=str(error), resume="T02/T04; Core remains independent")
    result = dict(status="BLOCKED", reason=android["reason"], sdk_root=str(sdk), installed_properties=actual,
                  target_image_metadata_present=target.is_file(), image_boot="NOT RUN; T04",
                  jdk=observe_optional([lock["observed_tools"]["java"]["path"], "-version"]),
                  virtualization=observe_optional(["sysctl", "-n", "kern.hv_support"]),
                  gpu=observe_optional(["system_profiler", "SPDisplaysDataType", "-json"]),
                  licenses="NOT RUN; user reviews/accepts Android SDK licenses when preparing T02/T04",
                  avd_started=False)
    run.observed["android"] = result
    return result


def candidate_evidence(run, lock):
    cache = ROOT / lock["artifacts"]["directory"]
    acquisition = read_json(cache / "acquisition.json")
    require(acquisition["complete"] is True, "input preparation did not finish successfully")
    records = {row["url"]: row for row in acquisition["inputs"]}
    sources = {}
    for item in lock["candidate_inputs"]:
        row = records[item["url"]]
        if row["status"] == "NOT FOUND":
            require(item.get("optional") is True and row["http_status"] == 404, "required candidate source missing")
            sources[item["file"]] = row
            continue
        path = cache / item["file"]
        require(row["sha256"] == file_hash(path) and row["bytes"] == path.stat().st_size, "candidate evidence changed after acquisition")
        sources[item["file"]] = row
    license_text = (cache / "cloudstream-LICENSE").read_text(encoding="utf-8")
    require("GNU GENERAL PUBLIC LICENSE" in license_text and "Version 3" in license_text,
            "fixed Cloudstream GPLv3 text missing")
    wrapper = (cache / "cloudstream-gradle-wrapper.properties").read_text(encoding="utf-8")
    match = re.search(r"gradle-([0-9][0-9.]*)-(?:all|bin)\.zip", wrapper)
    require(match is not None, "fixed upstream Gradle distribution version missing")
    hls = read_json(cache / "hls-package.json")
    hls_release = read_json(cache / "hls-release.json")
    require(hls["name"] == "hls.js" and hls["license"] == "Apache-2.0"
            and hls_release["version"] == lock["candidate_versions"]["hls.js"]["exact_version"]
            and hls_release["license"] == hls["license"],
            "hls.js candidate source/version/license differs")
    require("Apache License" in (cache / "hls-LICENSE").read_text(encoding="utf-8"), "hls.js Apache license text missing")
    tauri = tomllib.loads((cache / "tauri-Cargo.toml").read_text())
    require(tauri["package"]["version"] == lock["candidate_versions"]["tauri"]["exact_version"], "Tauri candidate version differs")
    tauri_workspace = tomllib.loads((cache / "tauri-workspace.toml").read_text())
    require(tauri["package"]["license"] == {"workspace": True}
            and tauri_workspace["workspace"]["package"]["license"] == "Apache-2.0 OR MIT",
            "Tauri candidate inherited workspace license differs")
    pom = ET.fromstring((cache / "nanohttpd.pom").read_bytes())
    pom_version = pom.find("{*}version")
    # This release inherits its version and BSD license from the project POM.
    if pom_version is None:
        pom_version = pom.find("{*}parent/{*}version")
    require(pom_version is not None and pom_version.text == "2.3.1", "NanoHTTPD candidate version differs")
    require("BSD 3-Clause" in (pom.findtext("{*}licenses/{*}license/{*}name") or ""),
            "NanoHTTPD BSD source license declaration missing")
    android_lock = read_json(ROOT / lock["android"]["dependency_plan"])
    require(android_lock["runtime"]["candidate_commit"] == lock["candidate_versions"]["cloudstream"]["commit"]
            and android_lock["runtime"]["production_commit"] is None, "Android dependency plan promoted or changed runtime")
    require(android_lock["bridge_http"]["coordinate"] == "org.nanohttpd:nanohttpd:2.3.1", "NanoHTTPD dependency plan differs")
    repo = ET.fromstring((cache / "android-images.xml").read_bytes())
    for element in repo.iter():
        element.tag = element.tag.rsplit("}", 1)[-1]
    images = []
    for package in repo.iter("remotePackage"):
        if package.attrib.get("path") in ("system-images;android-35;default;arm64-v8a", "system-images;android-35;default;x86_64"):
            revision = package.find("revision")
            images.append(dict(package=package.attrib["path"], revision={c.tag: c.text for c in revision},
                               archive_xml=ET.tostring(package.find("archives"), encoding="unicode"),
                               status="repository candidate; download, SHA256 and boot NOT RUN; T04"))
    require({i["package"] for i in images} == {"system-images;android-35;default;arm64-v8a", "system-images;android-35;default;x86_64"},
            "API35 candidate image metadata missing")
    result = dict(sources=sources, gradle_candidate=match.group(1), api35_candidates=images,
                  production_commit=None, abi_loading="NOT RUN; T02", licenses="source texts recorded; release obligations in licenses/release-materials.json")
    run.observed["candidates"] = result
    return result


def negative_tests(run):
    spec = importlib.util.spec_from_file_location("t00_negative_tests", ROOT / "tests/acceptance/test_t00.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    suite = unittest.defaultTestLoader.loadTestsFromModule(module)
    stream = io.StringIO()
    result = unittest.TextTestRunner(stream=stream, verbosity=2).run(suite)
    log = run.folder / "negative-tests.log"
    log.write_text(stream.getvalue(), encoding="utf-8")
    require(result.testsRun >= 12 and result.wasSuccessful(), f"negative regression tests failed; log={log.relative_to(ROOT)}\n{stream.getvalue()}")
    return dict(tests=result.testsRun, failures=len(result.failures), errors=len(result.errors),
                log=log.relative_to(ROOT).as_posix(), kind="synthetic validation fixtures, separate from real seven-package checks")


def environment_probe(run):
    versions = {}
    for name in CORE_TOOLS:
        output = run.command([name, "--version"])
        row = dict(status="version_observed", exit=0,
                   argv=workflow.runtime.resolve_argv([name, "--version"]), stdout=output)
        versions[name] = validate_tool(name, row)
    result = dict(platform=sys.platform, architecture=platform.machine(), os=platform.platform(), versions=versions,
                  display={k: os.environ.get(k) for k in ("DISPLAY", "WAYLAND_DISPLAY")},
                  webview="NOT RUN; requires target-specific WebView inspection and T01 runtime evidence",
                  gpu="NOT RUN", virtualization="NOT RUN",
                  scope="Environment detection only; Core builds, media and Android boot NOT RUN")
    if sys.platform == "darwin":
        result["sdk_version"] = run.command(["xcrun", "--show-sdk-version"]).strip()
    elif sys.platform == "linux":
        result["webview"] = run.command(["pkg-config", "--modversion", "webkit2gtk-4.1"]).strip()
    run.observed["environment_probe"] = result
    return result


def all_cases(run, lock):
    run.case("environment-schema", ["U06", "R09", "R57", "REQ-T00"], "complete truthful plan and candidate/observed separation", lambda: validate_plan(lock))
    run.case("workflow-binding", ["C-PATH", "U06"], "authorized project, venv, source and original driver lock", lambda: binding(run, lock))
    supervised = run.case("resource-owner", ["C-RESOURCE", "C-PATH"], "current original receipt and inherited 4096MiB/jobs2/thread1", lambda: resources(run, lock))
    if supervised:
        run.observed["resources"] = run.cases[-1]["actual"]
    if supervised:
        run.case("validation-negative-cases", ["REQ-T00", "U06", "R02", "R57"], "all validators reject invalid inputs", lambda: negative_tests(run))
    else:
        run.case("validation-negative-cases", ["REQ-T00", "U06", "R02", "R57"], "original runner required",
                 lambda: require(False, "NOT RUN: resource ownership was not established"))
    run.case("frozen-original-inputs", ["R02", "U02", "R57"], "frozen artifact inventory and registry unchanged", lambda: dict(count=len(bound_inventory(lock)["plugins"])))
    inventory = bound_inventory(lock)
    cache = ROOT / lock["artifacts"]["directory"]

    def registry():
        require(file_hash(cache / "plugins.json") == inventory["registry_sha256"], "downloaded registry SHA256 differs")
        return validate_registry(read_json(cache / "plugins.json"), inventory)

    run.case("pinned-registry", ["U02", "R02"], "fixed registry SHA and seven identities", registry)
    for item in inventory["plugins"]:
        def check(item=item):
            actual, manifest = validate_artifact(cache / (item["internal_name"] + ".cs3"), item)
            retained = cache / (item["internal_name"] + ".manifest.json")
            require(retained.read_bytes() == manifest, "retained original manifest differs")
            actual["retained_manifest"] = retained.relative_to(ROOT).as_posix()
            return actual
        run.case("original-" + item["internal_name"], ["R02", "U02", "REQ-T00"], item, check,
                 inputs=(cache / (item["internal_name"] + ".cs3")).relative_to(ROOT).as_posix())
    run.case("candidate-sources", ["R57", "R09", "R50", "REQ-T00"], "fixed source/license configs; Android planned separately", lambda: candidate_evidence(run, lock))
    run.case("platform-execution-matrix", ["R09", "R50", "U06"], "local macOS arm64 only; deferred platforms have exact commands", lambda: lock["platforms"])
    for target in lock["platforms"]:
        if not target["required"]:
            run.cases.append(dict(id="R09/platform-" + target["id"], requirements=["R09", "REQ-T00"],
                                  required=False, input=target["target"], expected="optional execution per ADR-005/006",
                                  actual="NOT RUN: " + target["reason"], result="SKIP", evidence=[]))
    core_passed = False
    if supervised:
        versions_passed = run.case("core-official-versions", ["R33", "R57", "R50", "REQ-T00"], "live official versions equal declared baseline", lambda: core_versions(run, lock))
        if versions_passed:
            core_passed = run.case("core-minimum-builds", ["R33", "R08", "R50", "REQ-T00"], "Rust binary, npm ci/type/build, SDK/WebKit link and class lookup", lambda: core_builds(run, lock))
        else:
            run.case("core-minimum-builds", ["R33", "R08", "R50", "REQ-T00"], "Core version prerequisites required",
                     lambda: require(False, "NOT RUN: Core tool version gate failed"))
        run.case("android-prerequisites", ["R09", "R50", "REQ-T00"], "read tools/SDK metadata; record CS branch blocked; preserve HTTP progress", lambda: android_observations(run, lock))
    else:
        for name in ("core-official-versions", "core-minimum-builds", "android-prerequisites"):
            run.case(name, ["R33", "R08", "R50", "R09", "REQ-T00"], "original resource wrapper required",
                     lambda: require(False, "NOT RUN: original resource ownership was not established"))
    run.observed["platforms"] = [dict(target, status=("Core build PASS; media/window NOT RUN T01" if core_passed else "Core build FAIL or NOT RUN"))
                                  if target["required"] else target for target in lock["platforms"]]

    def dependencies():
        package = read_json(ROOT / "package.json")
        pinned = read_json(ROOT / "package-lock.json")["packages"][""]
        for section in ("dependencies", "devDependencies"):
            require(package[section] == pinned[section], "package manifest and lock differ")
        versions = {}
        for name, row in lock["frontend"]["packages"].items():
            versions[name] = read_json(ROOT / "node_modules" / name / "package.json")["version"]
            require(versions[name] == row["version"], f"{name} version differs")
        run.observed["frontend_dependencies"] = versions
        result = validate_npm_inventory(ROOT, read_json(ROOT / lock["licenses"]["npm_inventory"]))
        release = read_json(ROOT / lock["licenses"]["release_materials"])
        require(release["card"] == "T0.1" and len(release["materials"]) >= 6, "release materials checklist missing")
        require(all(r["status"] == "pending T23" and r["source"] and r["required_material"] for r in release["materials"]),
                "release material evidence/obligation missing or falsely marked complete")
        return dict(npm=result, release=release)

    run.case("dependency-licenses-and-locks", ["R33", "R57", "R08", "REQ-T00"], "every locked npm dependency licensed and version-checked; release checklist", dependencies)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", choices=("all", "prepare", "negatives", "environment"), required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.suite, args.report, args.junit)
    try:
        lock = read_json(ROOT / "docs/runtime.lock.json")
        if args.suite == "prepare":
            run.case("original-resource-wrapper", ["C-RESOURCE"], "prepare runs under the original resource wrapper", lambda: resources(run, lock))
            if run.cases[-1]["result"] == "PASS":
                prepare(run, lock)
        elif args.suite in ("negatives", "environment"):
            run.case("original-resource-wrapper", ["C-RESOURCE"], "probes/tests run under original wrapper", lambda: resources(run, lock))
            if run.cases[-1]["result"] == "PASS":
                if args.suite == "negatives":
                    run.case("validation-negative-cases", ["REQ-T00"], "all regression assertions pass", lambda: negative_tests(run))
                else:
                    run.case("environment-detection", ["R09"], "current target official tools version output", lambda: environment_probe(run))
        else:
            all_cases(run, lock)
    except Exception as error:
        run.case("unhandled-prerequisite", ["REQ-T00"], "complete required inputs",
                 lambda: require(False, f"{type(error).__name__}: {error}"))
    return run.save()


if __name__ == "__main__":
    sys.exit(main())
