"""T06 functional gate, executed only under the original resource runner.

Native HTTP fixtures exercise production Host/adapters. Cloudstream tests are
synthetic common DTO/guest-reference vectors; real provider execution is N02.
"""
import argparse
import datetime as dt
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import sys
import time
import uuid
import xml.etree.ElementTree as ET

from t00 import Acceptance as BaseAcceptance, read_json, require, workflow, write_json

ROOT = Path(__file__).resolve().parents[2]
IDS = ("R15", "R18", "R19", "R21", "R35", "R59", "R60", "REQ-T06")
RUST_TESTS = {
    "t06_archived_identity_and_home_scope_vectors": ("R18", "R35", "REQ-T06"),
    "t06_three_kind_common_dto_and_negative_relationships": ("R15", "R18", "R21", "R35", "R59", "R60", "REQ-T06"),
    "t06_guest_dynamic_multi_duplicate_titles_and_original_refs": ("R15", "R18", "R35", "REQ-T06"),
    "t06_guest_partial_retry_scope_and_empty_all_failure": ("R15", "R18", "R21", "R35", "REQ-T06"),
    "t06_guest_epoch_revision_and_transactional_negatives": ("R18", "R35", "REQ-T06"),
    "t06_provider_serial_queue_deadline_and_cancelled_publication": ("R19", "REQ-T06"),
    "t06_provider_queue_timeout_and_cancel_do_not_enter_transport": ("R19", "REQ-T06"),
    "t06_budget_selection_child_deadline_reserve_and_cleanup": ("R19", "R59", "R60", "REQ-T06"),
    "t06_maccms_dynamic_duplicate_titles_original_page_and_retry": ("R15", "R18", "R21", "R35", "R59", "REQ-T06"),
    "t06_maccms_reserve_keeps_completed_and_unstarted_positions": ("R19", "R35", "R59", "REQ-T06"),
    "t06_maccms_single_default_empty_and_all_failure_regressions": ("R15", "R18", "R21", "R59", "REQ-T06"),
    "t06_stremio_order_duplicate_titles_retry_and_original_offset": ("R15", "R18", "R21", "R35", "R60", "REQ-T06"),
    "t06_stremio_column_localization_shares_30s_deadline": ("R19", "R60", "REQ-T06"),
    "t06_stremio_reserve_cancellation_empty_and_all_failed_regressions": ("R15", "R18", "R19", "R21", "R35", "R60", "REQ-T06"),
}


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T06-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def save(self):
        current = workflow.snapshot()
        self.case("stable-tested-source", ["REQ-T06"], "source unchanged throughout A1",
                  lambda: require(current == self.source, "source changed during A1"))
        summaries = []
        for identifier in IDS:
            children = [case for case in self.cases if identifier in case["requirements"]]
            summaries.append(dict(id=identifier,
                                  result="PASS" if children and all(case["result"] == "PASS" for case in children) else "FAIL",
                                  children=[case["id"] for case in children]))
        counts = dict(required=len(self.cases), ran=len(self.cases),
                      passed=sum(case["result"] == "PASS" for case in self.cases),
                      failed=sum(case["result"] != "PASS" for case in self.cases), skipped=0)
        report = dict(card="T0.6", original_card="T06", suite="all", utc_started=self.started,
                      utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
                      tested_source_digest=current, protected=workflow.protected_hash(), platform=sys.platform,
                      arch=platform.machine(), os=platform.platform(), commands=self.commands,
                      cases=self.cases, summaries=summaries, observed=self.observed, counts=counts,
                      deployment="ordinary native process and loopback HTTP fixtures; no cluster or Android",
                      cleanup="fixture Drop stops/joins owned workers; original runner retains final descendant cleanup",
                      native_counts=self.observed.get("native_counts", dict(required=len(RUST_TESTS), ran=0, passed=0, failed=0, skipped=0, status="NOT RUN")),
                      cloudstream=dict(synthetic=True, scope="common DTO and guest reference boundary",
                                       real_providers="NOT RUN: original 7-package runtime/provider gate belongs to N02"))
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T06-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T06", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T06 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return int(counts["failed"] > 0 or any(row["result"] != "PASS" for row in summaries))


def resources(run):
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "original run.py wrapper required")
    cfg = workflow.config()
    require(cfg["memory_mib"] == 4096 and cfg["jobs"] == 2, "approved resource budget differs")
    require(os.environ.get("CARGO_BUILD_JOBS") == "2" and os.environ.get("RUST_TEST_THREADS") == "1", "build/test concurrency differs")
    require(Path(os.environ["CARGO_TARGET_DIR"]) == ROOT / "target", "build directory binding differs")
    require(Path.cwd().resolve() == ROOT and Path(sys.executable).absolute() == ROOT / ".venv/bin/python", "root/venv binding differs")
    ancestors = {process.pid for process in workflow.runtime.psutil.Process().parents()} | {os.getpid()}
    deadline, matches = time.monotonic() + 5, []
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
    require(receipt["source"] == run.source and receipt["card"] == "T0.6", "receipt source/card differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_mib"] == 4096
            and env["jobs"] == 2 and env["test_threads"] == 1, "resource receipt backend/budget differs")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "local macOS arm64 functional gate required")
    require(env["memory_enforcement"] == "rss-sampled", "actual RSS sampling backend required")
    run.observed["resources"] = dict(receipt=str(path.relative_to(ROOT)), environment=env,
                                     note="RSS sampling allows transient overage; no cgroup hard memory/CPU quota")
    return run.observed["resources"]


def independent(run, output):
    from jsonschema import Draft202012Validator, FormatChecker
    from referencing import Registry, Resource
    app, bridge = read_json(ROOT / "protocol/app.schema.json"), read_json(ROOT / "protocol/bridge.openapi.json")
    bridge_uri = "https://replica.invalid/contracts/bridge.openapi.json"
    registry = Registry().with_resource(app["$id"], Resource.from_contents(app)).with_resource(bridge_uri, Resource.from_contents({
        **bridge, "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": bridge_uri}))
    module = importlib.util.spec_from_file_location("t06_contract_oracle", ROOT / "replica-spec/validate_contracts.py")
    oracle = importlib.util.module_from_spec(module)
    module.loader.exec_module(oracle)
    observations, synthetic, identity_vectors, kinds = [], [], [], set()
    home_requests = []
    for line in output.splitlines():
        for marker in ("N00_OBSERVATION:", "N01_OBSERVATION:"):
            if marker not in line:
                continue
            item = json.loads(line.split(marker, 1)[1])
            require(item.get("capabilities_redacted") is True, "capability-safe native observation required")
            Draft202012Validator({"$ref": f'{app["$id"]}#/$defs/Result_{item["command"]}'}, registry=registry,
                                 format_checker=FormatChecker()).validate(item["response"])
            require(item["args"]["request_id"] == item["response"]["request_id"], "response request identity differs")
            if item["command"] == "get_home" and "data" in item["response"]:
                request, page = item["args"]["input"], item["response"]["data"]["page"]
                require(oracle.home_response(request, page), "native home violates independent request/index oracle")
                home_requests.append(dict(input=request, page=page, kind="maccms" if marker.startswith("N00") else "stremio"))
            observations.append(item)
        if "T06_DTO:" in line:
            item = json.loads(line.split("T06_DTO:", 1)[1])
            require(item["synthetic"] is True, "synthetic provider vector must be labeled")
            Draft202012Validator({"$ref": f'{app["$id"]}#/$defs/HomeViewData'}, registry=registry,
                                 format_checker=FormatChecker()).validate(dict(page=item["page"], images={}))
            require(oracle.home_response(item["input"], item["page"]), "synthetic common home violates independent scope oracle")
            kinds.add(item["kind"])
            synthetic.append(item)
        if "T06_IDENTITY:" in line:
            item = json.loads(line.split("T06_IDENTITY:", 1)[1])
            vector = item["input"]
            section = vector["section_input"]
            data = b"section\0"
            for value in (vector["source_id"], vector["source_revision"], section["mainpage_name"],
                          section["mainpage_data"], section["duplicate_section_ordinal"]):
                value = value.encode("utf-8")
                data += len(value).to_bytes(4, "big") + value
            require(hashlib.sha256(data).hexdigest() == item["actual"] == vector["section_id"], "section identity formula differs")
            identity_vectors.append(item)
    require(kinds == {"cloudstream", "maccms", "stremio"}, "all three synthetic common DTO vectors required")
    require(len(identity_vectors) == 1, "one original section identity vector required")
    for kind in ("maccms", "stremio"):
        rows = [row for row in home_requests if row["kind"] == kind]
        require(any([s["index"] for s in row["page"]["sections"]] == [0, 2]
                    and [s["index"] for s in row["page"]["failed_sections"]] == [1] for row in rows), f"{kind} A/B/C mixed positions missing")
        require(any(row["input"].get("section_ref") and row["input"]["page"] == 1
                    and len(row["page"]["sections"]) == 1 and row["page"]["sections"][0]["index"] == 1
                    and not row["page"]["failed_sections"] for row in rows), f"{kind} successful B-only first-screen retry missing")
        require(any(row["input"]["page"] >= 2 and len(row["page"]["sections"]) == 1 for row in rows), f"{kind} pagination missing")
    write_json(run.folder / "native-observations.json", observations)
    write_json(run.folder / "synthetic-home-vectors.json", synthetic)
    run.observed["dto"] = dict(native=len(observations), synthetic=len(synthetic), kinds=sorted(kinds),
                                section_identity_vectors=len(identity_vectors),
                                trace=str((run.folder / "native-observations.json").relative_to(ROOT)),
                                synthetic_trace=str((run.folder / "synthetic-home-vectors.json").relative_to(ROOT)))
    return run.observed["dto"]


def rust(run):
    run.observed["versions"] = dict(python=sys.version, rustc=run.command(["rustc", "--version"], timeout=30).strip(),
                                    cargo=run.command(["cargo", "--version"], timeout=30).strip())
    try:
        output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
                              "--lib", "t06_", "--", "--test-threads=1", "--nocapture"], timeout=3000)
    except Exception:
        output = (ROOT / run.commands[-1]["log"]).read_text(encoding="utf-8", errors="replace")
        result = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", output)
        passed, failed, skipped = map(int, result.groups()) if result else (0, 0, 0)
        run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=passed + failed + skipped,
                                               passed=passed, failed=failed, skipped=skipped,
                                               status="FAIL" if result else "NOT RUN", log=run.commands[-1]["log"])
        raise
    names = set(re.findall(r"^test (?:\w+::)*(t06_\w+) \.\.\.", output, re.M))
    require(names == set(RUST_TESTS), f"native test inventory differs: {names ^ set(RUST_TESTS)}")
    summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", output)
    require(summary is not None and int(summary[1]) == len(RUST_TESTS), "all nonzero native tests must execute and pass")
    run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=len(names), passed=len(names), failed=0, skipped=0, status="PASS")
    for name, requirements in RUST_TESTS.items():
        run.case(name, requirements, "production Rust behavior assertions executed and passed",
                 lambda name=name: dict(test=name, exit=0, log=run.commands[-1]["log"]))
    independent(run, output)
    return dict(tests=len(names), exit=0, log=run.commands[-1]["log"], dto=run.observed["dto"])


def main():
    if not __debug__:
        raise SystemExit("Acceptance requires assertions; remove -O")
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    options = parser.parse_args()
    run = Acceptance(options.report, options.junit)
    ready = run.case("resource-ownership", ["R19", "REQ-T06"], "original scope, source and local platform", lambda: resources(run))
    run.case("native-behaviors", list(IDS), "named native behaviors and independent DTO validation pass",
             lambda: rust(run) if ready else require(False, "NOT RUN: resource ownership failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
