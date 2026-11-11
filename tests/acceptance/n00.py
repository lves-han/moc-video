"""N00 functional gate; run with the original run.py resource supervisor.

The native tests compile the production Host/adapter, with a fixed test-only
HTTP allowlist. This is one ordinary loopback fixture, with no cluster/Android.
Serialized Host responses are also checked by the independent Python validator.
"""
import argparse
import datetime as dt
import json
import os
from pathlib import Path
import platform
import re
import sys
import time
import tomllib
import uuid
import xml.etree.ElementTree as ET

from t00 import Acceptance as BaseAcceptance, read_json, require, workflow, write_json

ROOT = Path(__file__).resolve().parents[2]
IDS = ("U08", "R06", "R39", "R46", "R49", "R58", "R59", "R64", "REQ-N00")
RUST_TESTS = {
    "n00_host_sync_home_details_and_native_images": ("U08", "R06", "R49", "R58", "R59", "REQ-N00"),
    "n00_query_search_encoding_and_empty_results": ("R49", "R59", "REQ-N00"),
    "n00_protocol_numbers_closed_and_errors": ("R59", "REQ-N00"),
    "n00_archived_identity_and_protocol_vectors": ("R59", "R64", "REQ-N00"),
    "n00_group_order_url_fidelity_and_stale_fingerprints": ("R39", "R49", "R59", "R64", "REQ-N00"),
    "n00_ambiguous_groups_and_unknown_metadata": ("R59", "REQ-N00"),
    "n00_type_probe_formats_html_unknown_and_bound": ("R46", "R49", "R59", "REQ-N00"),
    "n00_http_local_addresses_and_dns_refresh": ("R46", "R49", "REQ-N00"),
    "n00_cancellation_interrupts_dns_lookup": ("R39", "R46", "REQ-N00"),
    "n00_redirect_origin_headers_proxy_and_limits": ("R46", "R49", "REQ-N00"),
    "n00_partial_home_retry_pagination_and_all_failed": ("U08", "R59", "REQ-N00"),
    "n00_config_edits_disable_delete_and_readd_cancel_io": ("R39", "R58", "R64", "REQ-N00"),
    "n00_cancel_pending_sync_and_keep_control_responsive": ("R06", "R39", "REQ-N00"),
    "n00_latest_sync_wins_without_partial_publication": ("R39", "R64", "REQ-N00"),
    "n00_deadline_queue_and_live_body_timeout": ("R39", "R59", "REQ-N00"),
    "n00_metadata_limits_and_foreign_section_refs": ("R46", "R49", "R59", "REQ-N00"),
    "n00_global_metadata_slot_bound": ("R39", "R46", "REQ-N00"),
}


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("N00-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def save(self):
        current = workflow.snapshot()
        self.case("stable-tested-source", ["REQ-N00"], "source stays unchanged throughout A1",
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
        report = dict(card="T0.4", original_card="N00", suite="all", utc_started=self.started,
                      utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
                      tested_source_digest=current, protected=workflow.protected_hash(), platform=sys.platform,
                      arch=platform.machine(), os=platform.platform(), commands=self.commands,
                      cases=self.cases, summaries=summaries, observed=self.observed, counts=counts,
                      deployment="one native test process with ordinary loopback HTTP and DNS fixtures; no Android or cluster",
                      cleanup="native fixture Drop stops and joins its owned workers; original runner owns final descendant cleanup",
                      native_counts=self.observed.get("native_counts", dict(required=len(RUST_TESTS), ran=0, passed=0, failed=0, skipped=0, status="NOT RUN")),
                      scope="MacCMS adapter, Host catalog/home/details, native reference/resolve mapping; T08 match and T09 serving retain their original cards")
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="N00-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="N00", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="N00 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
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
    require(receipt["source"] == run.source and receipt["card"] == "T0.4", "receipt source/card differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_mib"] == 4096
            and env["jobs"] == 2 and env["test_threads"] == 1, "resource receipt backend/budget differs")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "local macOS arm64 functional gate required")
    require(env["memory_enforcement"] == "rss-sampled", "actual RSS sampling backend required")
    run.observed["resources"] = dict(receipt=str(path.relative_to(ROOT)), environment=env,
                                     note="process-tree RSS sampling allows transient overage; no cgroup hard memory/CPU quota")
    return run.observed["resources"]


def rust(run):
    packages = tomllib.loads((ROOT / "src-tauri/Cargo.lock").read_text(encoding="utf-8"))["package"]
    direct = tomllib.loads((ROOT / "src-tauri/Cargo.toml").read_text(encoding="utf-8"))["dependencies"]
    run.observed["versions"] = dict(python=sys.version, rustc=run.command(["rustc", "--version"], timeout=30).strip(),
                                    cargo=run.command(["cargo", "--version"], timeout=30).strip(),
                                    dependencies={name: [p["version"] for p in packages if p["name"] == name] for name in direct},
                                    webview=dict(status="NOT RUN", reason="headless native protocol gate"))
    proxy_env = {name: "http://127.0.0.1:1" for name in
                 ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy")}
    proxy_env.update(N00_PROXY_TEST="1", NO_PROXY="", no_proxy="")
    previous = {name: os.environ.get(name) for name in proxy_env}
    try:
        os.environ.update(proxy_env)
        try:
            output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
                                  "--lib", "n00_", "--", "--test-threads=1", "--nocapture"], timeout=3000)
        except Exception:
            output = (ROOT / run.commands[-1]["log"]).read_text(encoding="utf-8", errors="replace")
            result = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", output)
            passed, failed, skipped = map(int, result.groups()) if result else (0, 0, 0)
            run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=passed + failed + skipped,
                                                   passed=passed, failed=failed, skipped=skipped,
                                                   status="FAIL" if result else "NOT RUN", log=run.commands[-1]["log"])
            raise
    finally:
        for name, value in previous.items():
            if value is None:
                os.environ.pop(name, None)
            else:
                os.environ[name] = value
    names = set(re.findall(r"^test (?:\w+::)*(n00_\w+) \.\.\.", output, re.M))
    require(names == set(RUST_TESTS), f"native test inventory differs: {names ^ set(RUST_TESTS)}")
    summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", output)
    require(summary is not None and int(summary[1]) == len(RUST_TESTS), "all nonzero native tests must execute and pass")
    require("N00_PROXY_ENV:true" in output, "real HTTP transport must run with rejecting proxy variables")
    run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=len(names), passed=len(names), failed=0, skipped=0, status="PASS")
    observations = []
    for line in output.splitlines():
        if "N00_OBSERVATION:" in line:
            observations.append(json.loads(line.split("N00_OBSERVATION:", 1)[1]))
    require(observations, "serialized production Host responses required")
    from jsonschema import Draft202012Validator, FormatChecker
    from referencing import Registry, Resource
    app = read_json(ROOT / "protocol/app.schema.json")
    bridge = read_json(ROOT / "protocol/bridge.openapi.json")
    bridge_uri = "https://replica.invalid/contracts/bridge.openapi.json"
    registry = Registry().with_resource(app["$id"], Resource.from_contents(app)).with_resource(bridge_uri, Resource.from_contents({
        **bridge, "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": bridge_uri}))
    seen = set()
    for item in observations:
        require(item.get("capabilities_redacted") is True, "capability-safe observation required")
        validator = Draft202012Validator({"$ref": f'{app["$id"]}#/$defs/Result_{item["command"]}'},
                                        registry=registry, format_checker=FormatChecker())
        validator.validate(item["response"])
        require(item["args"]["request_id"] == item["response"]["request_id"], "request identity changed")
        seen.add(item["command"])
    require({"sync_catalog", "get_home", "get_details", "save_settings", "cancel_request", "runtime_status"} <= seen,
            "required production command observations missing")
    write_json(run.folder / "native-observations.json", observations)
    for name, requirements in RUST_TESTS.items():
        run.case(name, requirements, "production Rust behavior assertions executed and passed",
                 lambda name=name: dict(test=name, exit=0, log=run.commands[-1]["log"]))
    run.observed["rust"] = dict(tests=len(names), observations=len(observations), exit=0,
                                proxy_env=proxy_env, log=run.commands[-1]["log"],
                                trace=str((run.folder / "native-observations.json").relative_to(ROOT)))
    return run.observed["rust"]


def main():
    if not __debug__:
        raise SystemExit("Acceptance requires assertions; remove -O")
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    options = parser.parse_args()
    run = Acceptance(options.report, options.junit)
    if run.case("resource-ownership", ["R06", "R39", "REQ-N00"], "original scope, source and local platform", lambda: resources(run)):
        run.case("native-behaviors", list(IDS), "all named native behaviors and independent DTO validation pass", lambda: rust(run))
    else:
        run.case("native-behaviors", list(IDS), "original resource owner required",
                 lambda: require(False, "NOT RUN: resource ownership failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
