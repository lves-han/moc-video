"""T05 behavioral gate. Invoke under the original run.py; never nest runners.

Rust tests call the production Host and the real Tauri IPC handler (MockRuntime,
no GUI/Android). Node tests exercise the production api.ts settings writer.
All child processes inherit the original resource scope and global lock.
"""
import argparse
import datetime as dt
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import sys
import tempfile
import time
import uuid
import xml.etree.ElementTree as ET

from t00 import Acceptance as BaseAcceptance, read_json, require, workflow, write_json
from t05_models import generate

ROOT = Path(__file__).resolve().parents[2]
IDS = ("U01", "U06", "R01", "R06", "R08", "R10", "R17", "R19", "R31",
       "R36", "R38", "R39", "R44", "R51", "R58", "R63", "R64", "REQ-T05")
RUST_TESTS = {
    "t05_contract_commands_and_envelopes": ("U01", "U06", "R08", "R17", "R31", "R51", "REQ-T05"),
    "t05_http_only_preparation_and_gateway": ("R06", "R10", "R44", "R63", "REQ-T05"),
    "t05_settings_roundtrip_registry_and_identity": ("R01", "R06", "R38", "R58", "R64", "REQ-T05"),
    "t05_settings_validation_and_failure_atomicity": ("R38", "R39", "R58", "REQ-T05"),
    "t05_migration_corruption_and_unknown_version": ("U01", "R01", "R38", "R64", "REQ-T05"),
    "t05_configuration_limits_and_utf8_budgets": ("R17", "R44", "R58", "REQ-T05"),
    "t05_same_lock_commits_and_atomic_cancel": ("R36", "R38", "R39", "REQ-T05"),
    "t05_legacy_source_preferences_restore_by_kind": ("R01", "R38", "R64", "REQ-T05"),
    "t05_generation_tombstones_and_source_isolation": ("R36", "R39", "R63", "R64", "REQ-T05"),
    "t05_sync_sequence_rejects_late_publication": ("R39", "R63", "REQ-T05"),
    "t05_uuid_duplicates_self_cancel_and_cascade": ("R19", "R36", "REQ-T05"),
    "t05_deadlines_include_queue_and_discard_old_work": ("R19", "R36", "R39", "REQ-T05"),
    "t05_cancelled_queued_settings_never_commit": ("R36", "R38", "R39", "REQ-T05"),
    "t05_unknown_sources_revisions_and_kind_rules": ("R17", "R31", "R58", "REQ-T05"),
    "t05_permissions_and_actual_tauri_handler": ("U01", "R08", "R31", "R44", "REQ-T05"),
    "t05_ready_and_field_drift_negatives": ("U06", "R10", "R17", "R31", "REQ-T05"),
}


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T05-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def save(self):
        current = workflow.snapshot()
        self.case("stable-tested-source", ["R51", "REQ-T05"], "unchanged source throughout A1",
                  lambda: require(current == self.source, "source changed during A1"))
        summaries = []
        for identifier in IDS:
            children = [case for case in self.cases if identifier in case["requirements"]]
            summaries.append(dict(id=identifier, result="PASS" if children and all(case["result"] == "PASS" for case in children) else "FAIL",
                                  children=[case["id"] for case in children]))
        counts = dict(passed=sum(case["result"] == "PASS" for case in self.cases),
                      failed=sum(case["result"] != "PASS" for case in self.cases), skipped=0)
        report = dict(card="T0.3", original_card="T05", suite="all", utc_started=self.started,
                      utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
                      tested_source_digest=current, protected=workflow.protected_hash(), platform=sys.platform,
                      arch=platform.machine(), os=platform.platform(), observed=self.observed,
                      commands=self.commands, cases=self.cases, summaries=summaries, counts=counts,
                      deployment="ordinary single-process loopback fixtures and Tauri MockRuntime; no cluster or Android",
                      deferred_platforms=[dict(platform=name, status="NOT RUN", reason="ADR-005/006 nonblocking platform",
                          reproduce_argv=["{python}", "scripts/run.py", "A1", "--", "{python}", "-B", "tests/acceptance/t05.py", "--suite", "all",
                                          "--report", "evidence/acceptance/T05-all.json", "--junit", "evidence/acceptance/T05-all.xml"])
                          for name in ("macos-x86_64", "windows-x86_64", "linux-x11", "linux-wayland")])
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T05-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T05", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T05 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return int(counts["failed"] > 0 or any(row["result"] != "PASS" for row in summaries))


def resources(run):
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "original run.py wrapper required")
    cfg = workflow.config()
    require(cfg["memory_mib"] == 4096 and cfg["jobs"] == 2, "approved resource budget differs")
    require(os.environ.get("CARGO_BUILD_JOBS") == "2" and os.environ.get("RUST_TEST_THREADS") == "1", "build/test parallelism differs")
    require(Path(os.environ["CARGO_TARGET_DIR"]) == ROOT / "target", "target directory binding differs")
    require(Path.cwd().resolve() == ROOT and Path(sys.executable).absolute() == ROOT / ".venv/bin/python", "project/venv binding differs")
    ancestors = {process.pid for process in workflow.runtime.psutil.Process().parents()} | {os.getpid()}
    deadline = time.monotonic() + 5
    matches = []
    while time.monotonic() < deadline:
        receipts = [(path, read_json(path)) for path in (ROOT / "evidence/runs").glob("*/receipt.json")]
        matches = [(path, receipt) for path, receipt in receipts if receipt.get("status") == "running"
                   and receipt.get("payload_pid") in ancestors
                   and workflow.runtime.alive(dict(pid=receipt["payload_pid"], identity=receipt["payload_identity"]))]
        if matches:
            break
        time.sleep(0.05)
    require(len(matches) == 1, "one original resource receipt must own this A1")
    path, receipt = matches[0]
    require(receipt["source"] == run.source and receipt["card"] == "T0.3", "receipt source/card differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_mib"] == 4096
            and env["jobs"] == 2 and env["test_threads"] == 1, "resource receipt backend/budget differs")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "this A1 requires the local macOS arm64 host")
    require(env["memory_enforcement"] == "rss-sampled", "record actual macOS RSS sampling backend")
    run.observed["resources"] = dict(receipt=str(path.relative_to(ROOT)), environment=env,
        note="process-tree RSS sampled; transient overage possible; no cgroup hard memory/CPU quota")
    return run.observed["resources"]


def contracts(run):
    module = importlib.util.spec_from_file_location("working_contract_checker", ROOT / "scripts/check-contracts.py")
    checker = importlib.util.module_from_spec(module)
    module.loader.exec_module(checker)
    observed = checker.check(ROOT / "protocol")
    require(observed["commands"] == 17 and observed["structural_cases"] > 0 and observed["semantic_cases"] > 0,
            "complete existing protocol coverage required")
    negatives = 0
    with tempfile.TemporaryDirectory(prefix="t05-contract-") as temp:
        for missing in ("bridge.openapi.json", "app.schema.json", "examples/contract-cases.json"):
            directory = Path(temp) / str(negatives)
            shutil.copytree(ROOT / "protocol", directory)
            (directory / missing).unlink()
            try:
                checker.check(directory)
            except FileNotFoundError:
                negatives += 1
            else:
                raise AssertionError("missing working protocol accepted: " + missing)
        directory = Path(temp) / "drift"
        shutil.copytree(ROOT / "protocol", directory)
        app = read_json(directory / "app.schema.json")
        app["$defs"]["Args_get_sources"]["properties"]["requestId"] = app["$defs"]["Args_get_sources"]["properties"].pop("request_id")
        write_json(directory / "app.schema.json", app)
        try:
            checker.check(directory)
        except AssertionError:
            negatives += 1
        else:
            raise AssertionError("working protocol drift accepted")
    require((ROOT / "src/models.ts").read_text(encoding="utf-8") == generate(), "generated frontend DTOs drifted from working protocol")
    run.observed["contracts"] = {**observed, "negative_probes": negatives}
    return run.observed["contracts"]


def rust(run):
    output = run.command(["cargo", "test", "--locked", "--manifest-path", "src-tauri/Cargo.toml", "--test", "t05", "--", "--test-threads=1", "--nocapture"], timeout=2700)
    names = set(re.findall(r"^test (t05_\w+) \.\.\.", output, re.M))
    require(names == set(RUST_TESTS), f"Rust behavior inventory differs: {names ^ set(RUST_TESTS)}")
    summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", output)
    require(summary is not None and int(summary[1]) == len(RUST_TESTS), "all nonzero Rust behaviors must execute")
    observations = []
    for line in output.splitlines():
        if "T05_OBSERVATION:" in line:
            observations.append(json.loads(line.split("T05_OBSERVATION:", 1)[1]))
    require({item["command"] for item in observations} == set(read_json(ROOT / "protocol/app.schema.json")["x-command-contracts"]), "missing real command observations")
    # Cross-language validation checks the serialized native output with the
    # independent existing Python validator, rather than trusting Rust assertions.
    from jsonschema import Draft202012Validator, FormatChecker
    from referencing import Registry, Resource
    app = read_json(ROOT / "protocol/app.schema.json")
    bridge = read_json(ROOT / "protocol/bridge.openapi.json")
    bridge_uri = "https://replica.invalid/contracts/bridge.openapi.json"
    registry = Registry().with_resource(app["$id"], Resource.from_contents(app)).with_resource(bridge_uri, Resource.from_contents({
        **bridge, "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": bridge_uri}))
    for item in observations:
        validator = Draft202012Validator({"$ref": f'{app["$id"]}#/$defs/Result_{item["command"]}'}, registry=registry, format_checker=FormatChecker())
        validator.validate(item["response"])
        require(item["args"]["request_id"] == item["response"]["request_id"], "response request identity changed")
    write_json(run.folder / "native-observations.json", observations)
    for name, requirements in RUST_TESTS.items():
        run.case(name, requirements, "production Rust assertions executed and passed",
                 lambda name=name: dict(test=name, exit=0, log=run.commands[-1]["log"]))
    run.observed["rust"] = dict(tests=len(names), observations=len(observations), exit=0,
                                log=run.commands[-1]["log"], trace=str((run.folder / "native-observations.json").relative_to(ROOT)))
    return run.observed["rust"]


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/api.test.ts"], timeout=180)
    count = re.search(r"^# tests (\d+)\s*$", output, re.M)
    passed = re.search(r"^# pass (\d+)\s*$", output, re.M)
    require(count and passed and int(count[1]) >= 8 and count[1] == passed[1], "nonzero settings queue tests must all pass")
    require(re.search(r"^# fail 0\s*$", output, re.M) is not None and re.search(r"^# skipped 0\s*$", output, re.M) is not None,
            "frontend behavior failures/skips")
    run.observed["frontend"] = dict(tests=int(count[1]), exit=0, log=run.commands[-1]["log"])
    run.command(["npm", "run", "check"], timeout=180)
    return run.observed["frontend"]


def main():
    if not __debug__:
        raise SystemExit("Acceptance requires assertions; remove -O")
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    options = parser.parse_args()
    run = Acceptance(options.report, options.junit)
    owned = run.case("resource-ownership", ["U06", "R51", "REQ-T05"], "original scope, source and local platform", lambda: resources(run))
    if owned:
        run.case("working-protocol-and-generated-types", ["U06", "R08", "R17", "R31", "R51", "REQ-T05"],
                 "archived compatibility, refs, structural/semantic/lifecycle cases, missing/drift rejection", lambda: contracts(run))
        run.case("native-behaviors", list(IDS), "all named production Rust tests and serialized DTOs pass", lambda: rust(run))
        run.case("settings-writer-behaviors", ["R19", "R31", "R36", "R38", "R39", "REQ-T05"],
                 "single writer, failure preservation, unknown-commit recovery, drain and stale-result rejection", lambda: frontend(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
