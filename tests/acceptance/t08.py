"""T08 matching and resolution gate through production native behavior.

Owned MacCMS/Stremio HTTP fixtures are real loopback requests. Cloudstream
interface/ownership vectors are synthetic; seven original packages remain N02.
"""
import argparse
import copy
import datetime as dt
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
IDS = ("U08", "R05", "R19", "R22", "R25", "R59", "R61", "R65", "R66", "R67", "REQ-T08")
RUST_TESTS = {
    "t08_verified_id_unique_special_decimal_and_ambiguities": ("U08", "R05", "R65", "R66", "R67", "REQ-T08"),
    "t08_queries_two_ten_twenty_deadline_errors_and_zero": ("R19", "R65", "R67", "REQ-T08"),
    "t08_match_store_binding_selection_lru_idle_revoke_and_order": ("R25", "R65", "R66", "R67", "REQ-T08"),
    "t08_dual_source_cancellation_and_child_cleanup": ("R25", "R67", "REQ-T08"),
    "t08_loadlinks_multiple_subtitles_false_unknown_quality_and_original_data": ("R05", "R19", "R22", "R61", "REQ-T08"),
    "t08_empty_false_exception_late_cancel_timeout_and_revision": ("R05", "R19", "R25", "R61", "REQ-T08"),
    "t08_unplayable_empty_links_provider_and_revision_isolation": ("R19", "R25", "R61", "REQ-T08"),
    "t08_release_retained_network_context_and_active_request_ownership": ("R05", "R22", "R25", "REQ-T08"),
    "t08_maccms_http_match_manual_resolve_probe_register_release": ("U08", "R19", "R22", "R25", "R59", "R65", "R66", "REQ-T08"),
    "t08_maccms_http_selection_forgery_video_provider_and_stremio_rejection": ("R59", "R65", "R66", "R67", "REQ-T08"),
    "t08_maccms_http_missing_search_manual_home_details_and_zero_error": ("R59", "R65", "R67", "REQ-T08"),
    "t08_maccms_http_match_expiry_order_cancel_and_configuration_change": ("R25", "R59", "R65", "R67", "REQ-T08"),
    "t08_maccms_http_english_alias_two_queries_ten_each_and_partial_error": ("R59", "R65", "R67", "REQ-T08"),
}


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T08-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def save(self):
        current = workflow.snapshot()
        self.case("stable-tested-source", ["REQ-T08"], "source unchanged during A1",
                  lambda: require(current == self.source, "source changed during A1"))
        summaries = []
        for identifier in IDS:
            children = [case for case in self.cases if identifier in case["requirements"]]
            summaries.append(dict(id=identifier, children=[case["id"] for case in children],
                                  result="PASS" if children and all(case["result"] == "PASS" for case in children) else "FAIL"))
        counts = dict(required=len(self.cases), ran=len(self.cases),
                      passed=sum(case["result"] == "PASS" for case in self.cases),
                      failed=sum(case["result"] != "PASS" for case in self.cases), skipped=0)
        report = dict(card="T0.8", original_card="T08", suite="all", utc_started=self.started,
                      utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
                      tested_source_digest=current, protected=workflow.protected_hash(), platform=sys.platform,
                      arch=platform.machine(), os=platform.platform(), commands=self.commands, cases=self.cases,
                      summaries=summaries, observed=self.observed, counts=counts,
                      native_counts=self.observed.get("native_counts", dict(required=len(RUST_TESTS), ran=0, passed=0, failed=0, skipped=0, status="NOT RUN")),
                      deployment="ordinary native process and owned loopback HTTP fixtures; no cluster or Android",
                      cleanup="fixture Drop stops/joins owned workers; original runner performs final descendant cleanup",
                      cloudstream=dict(synthetic=True, scope="loadLinks original data/isCasting=false, callbacks, native link/subtitle/context retention and snapshot rejection",
                                       real_providers="NOT RUN: original runtime, Ddys/SuperVod and seven packages belong to N02"))
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T08-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T08", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T08 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return int(counts["failed"] > 0 or any(row["result"] != "PASS" for row in summaries))


def resources(run):
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "original run.py wrapper required")
    cfg = workflow.config()
    require(cfg["memory_mib"] == 4096 and cfg["jobs"] == 2, "approved resource budget differs")
    require(os.environ.get("CARGO_BUILD_JOBS") == "2" and os.environ.get("RUST_TEST_THREADS") == "1", "build/test concurrency differs")
    require(Path(os.environ["CARGO_TARGET_DIR"]) == ROOT / "target", "target binding differs")
    require(Path.cwd().resolve() == ROOT and Path(sys.executable).absolute() == ROOT / ".venv/bin/python", "root/venv binding differs")
    ancestors = {p.pid for p in workflow.runtime.psutil.Process().parents()} | {os.getpid()}
    deadline, matches = time.monotonic() + 5, []
    while time.monotonic() < deadline:
        receipts = [(path, read_json(path)) for path in (ROOT / "evidence/runs").glob("*/receipt.json")]
        matches = [(path, row) for path, row in receipts if row.get("status") == "running" and row.get("payload_pid") in ancestors
                   and workflow.runtime.alive(dict(pid=row["payload_pid"], identity=row["payload_identity"]))]
        if matches:
            break
        time.sleep(0.05)
    require(len(matches) == 1, "one live original resource receipt must own A1")
    path, receipt = matches[0]
    require(receipt["source"] == run.source and receipt["card"] == "T0.8", "receipt source/card differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_mib"] == 4096
            and env["jobs"] == 2 and env["test_threads"] == 1, "receipt backend/budget differs")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "local macOS arm64 gate required")
    require(env["memory_enforcement"] == "rss-sampled", "actual RSS sampled backend required")
    run.observed["resources"] = dict(receipt=str(path.relative_to(ROOT)), environment=env,
                                     note="RSS sampling permits transient overage; no cgroup hard memory or CPU quota")
    return run.observed["resources"]


def check_context(input, details, context):
    require(context["metadata_ref"] == input["metadata_ref"] and context["metadata_video_id"] == input["video_id"], "metadata/video binding changed")
    require(context["content_ref"] == details["ref"], "provider content binding changed")
    require(context["selected_stream_id"] is None, "new context selected a stream before resolve")
    groups = [group for group in details["groups"] if group["id"] == context["group_id"]]
    require(len(groups) == 1, "group ownership is ambiguous")
    episodes = [ep for ep in groups[0]["episodes"] if ep["id"] == context["episode_id"]]
    require(len(episodes) == 1 and episodes[0]["ref"] == context["episode_ref"], "episode ownership or original data changed")
    for ref in (context["content_ref"], context["episode_ref"]):
        require(all(ref[field] == input["playback_source"][field] for field in ("source_id", "source_revision")), "wrong provider revision")


def check_vector(vector):
    kind = vector["kind"]
    if kind == "matching":
        metadata, provider, selected = vector["metadata"], vector["provider"], vector["selected"]
        require(selected["content_ref"] == provider["ref"], "automatic content changed")
        group = next(g for g in provider["groups"] if g["id"] == selected["group_id"])
        episode = next(ep for ep in group["episodes"] if ep["id"] == selected["episode_id"])
        expected = metadata["details"]["groups"][0]["episodes"][0]
        require(provider["kind"] == metadata["metadata_ref"]["identity"]["type"], "automatic media type changed")
        require(episode["season"] == expected["season"] and episode["number"] == expected["number"], "automatic season/decimal episode changed")
        return
    if kind in ("context", "http"):
        context = vector["context"] if kind == "context" else vector["match"]["context"]
        check_context(vector["input"], vector["details"], context)
    if kind == "http":
        require(vector["match"]["status"] == "matched" and len(vector["match"]["candidates"]) == 1, "confirmed candidate count changed")
        require(vector["resolution"]["episode_ref"] == vector["match"]["context"]["episode_ref"], "HTTP resolve lost matched episode")
        require(vector["resolution"]["streams"][0]["label"] == "线路甲 · Special 0.5", "HTTP stream name changed")
    if kind in ("resolution", "http"):
        resolution = vector["resolution"]
        reference = vector["reference"] if kind == "resolution" else vector["match"]["context"]["episode_ref"]
        require(resolution["episode_ref"] == reference, "original episode reference changed")
        require(all(resolution[field] == reference[field] for field in ("source_id", "source_revision")), "resolution provider changed")
        require(len({s["id"] for s in resolution["streams"]}) == len(resolution["streams"]), "duplicate stream IDs")
        if kind == "resolution":
            require([s["label"] for s in resolution["streams"]] == ["first 原始", "second 同名"], "callback order/name changed")
            require(resolution["streams"][0]["quality"] is None and resolution["subtitles"][0]["format"] == "ass", "unknown quality or original subtitle lost")


def independent(run, output):
    from jsonschema import Draft202012Validator, FormatChecker
    from referencing import Registry, Resource
    app, bridge = read_json(ROOT / "protocol/app.schema.json"), read_json(ROOT / "protocol/bridge.openapi.json")
    uri = "https://replica.invalid/contracts/bridge.openapi.json"
    registry = Registry().with_resource(app["$id"], Resource.from_contents(app)).with_resource(uri, Resource.from_contents({
        **bridge, "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": uri}))
    validators = {name: Draft202012Validator({"$ref": app["$id"] + "#/$defs/" + name}, registry=registry, format_checker=FormatChecker())
                  for name in ("PlaybackContext", "PlaybackMatchResult", "Result_resolve_episode")}
    vectors = [json.loads(line.split("T08_VECTOR:", 1)[1]) for line in output.splitlines() if "T08_VECTOR:" in line]
    require(len(vectors) == 5, f"all five independent observations required, got {len(vectors)}")
    require({v["kind"] for v in vectors} == {"matching", "context", "resolution", "http"}, "observation kinds incomplete")
    mutations = 0
    for vector in vectors:
        check_vector(vector)
        kind = vector["kind"]
        if kind == "context":
            validators["PlaybackContext"].validate(vector["context"])
            edits = [lambda v: v["context"]["episode_ref"].update(data="rewritten"),
                     lambda v: v["context"].update(metadata_video_id="foreign"),
                     lambda v: v["context"].update(group_id="foreign"),
                     lambda v: v["context"].update(selected_stream_id="premature")]
        elif kind == "matching":
            edits = [lambda v: v["provider"]["groups"][0]["episodes"][0].update(season=1),
                     lambda v: v["provider"]["groups"][0]["episodes"][0].update(number=0),
                     lambda v: v["provider"].update(kind="movie")]
        else:
            validators["Result_resolve_episode"].validate(dict(request_id=str(uuid.uuid4()), engine_epoch=str(uuid.uuid4()), data=vector["resolution"]))
            if kind == "http":
                validators["PlaybackMatchResult"].validate(vector["match"])
            edits = [lambda v: v["resolution"]["episode_ref"].update(data="rewritten"),
                     lambda v: v["resolution"].update(source_revision="foreign"),
                     lambda v: v["resolution"]["streams"][0].update(label="rewritten")]
        for edit in edits:
            invalid = copy.deepcopy(vector)
            edit(invalid)
            try:
                check_vector(invalid)
            except (ValueError, KeyError, IndexError, StopIteration):
                mutations += 1
            else:
                raise ValueError(f"independent oracle accepted corrupted {kind}")
    write_json(run.folder / "matching-resolution-vectors.json", vectors)
    run.observed["dto"] = dict(vectors=len(vectors), corruptions_rejected=mutations,
                               trace=str((run.folder / "matching-resolution-vectors.json").relative_to(ROOT)))
    return run.observed["dto"]


def rust(run):
    try:
        output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
                              "--lib", "t08_", "--", "--test-threads=1", "--nocapture"], timeout=3000)
    except Exception:
        output = (ROOT / run.commands[-1]["log"]).read_text(encoding="utf-8", errors="replace")
        result = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", output)
        passed, failed, skipped = map(int, result.groups()) if result else (0, 0, 0)
        run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=passed + failed + skipped, passed=passed,
                                              failed=failed, skipped=skipped, status="FAIL" if result else "NOT RUN", log=run.commands[-1]["log"])
        raise
    names = set(re.findall(r"^test (?:\w+::)*(t08_\w+) \.\.\.", output, re.M))
    require(names == set(RUST_TESTS), f"native inventory differs: {names ^ set(RUST_TESTS)}")
    summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", output)
    require(summary is not None and int(summary[1]) == len(RUST_TESTS), "all nonzero named native tests must pass")
    run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=len(names), passed=len(names), failed=0, skipped=0, status="PASS")
    for name, requirements in RUST_TESTS.items():
        run.case(name, requirements, "production behavior assertions executed and passed",
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
    ready = run.case("resource-ownership", ["REQ-T08"], "original resource scope and local platform", lambda: resources(run))
    run.case("native-behaviors", list(IDS), "named native tests and independent matching/resolution checks",
             lambda: rust(run) if ready else require(False, "NOT RUN: resource ownership failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())

