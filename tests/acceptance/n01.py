"""N01: production Host/Stremio behavior plus mandatory real en/zh responses.

Run once through scripts/run.py. Native fixtures use the existing test-build-only
HTTP allowlist; live evidence is fetched by the production public HTTP boundary.
"""
import argparse
import datetime as dt
import hashlib
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
IDS = ("U08", "R06", "R19", "R39", "R46", "R49", "R58", "R60", "R61", "R62", "R64", "R65", "REQ-N01")
RUST_TESTS = {
    "n01_manifest_atomic_first_failure_refresh_and_edit": ("R39", "R58", "R60", "R64", "REQ-N01"),
    "n01_resource_filters_required_extras_order_and_same_titles": ("U08", "R49", "R60", "R61", "REQ-N01"),
    "n01_prefix_query_rejection_and_path_segment_encoding": ("R46", "R49", "R60", "REQ-N01"),
    "n01_skip_raw_count_dedup_empty_and_no_new_items": ("R60", "R61", "REQ-N01"),
    "n01_meta_owner_fallback_filters_warnings_and_all_failed": ("U08", "R60", "R65", "REQ-N01"),
    "n01_catalog_and_meta_separate_addons_and_stream_only": ("U08", "R19", "R60", "REQ-N01"),
    "n01_movie_and_original_video_order_inline_media_ignored": ("R19", "R49", "R60", "R64", "REQ-N01"),
    "n01_partial_home_failed_placeholders_retry_and_all_failure": ("R60", "REQ-N01"),
    "n01_public_http_headers_cookies_addresses_and_foreign_refs": ("R06", "R46", "R49", "REQ-N01"),
    "n01_aggregate_45s_reserve_partial_and_cancellation": ("R39", "R60", "REQ-N01"),
    "n01_sync_sequence_and_edit_disable_delete_readd_cancel": ("R39", "R58", "R64", "REQ-N01"),
    "n01_manifest_latest_sync_timeout_and_user_cancel": ("R06", "R39", "R60", "R64", "REQ-N01"),
    "n01_language_field_fallback_identity_cache_and_locale_mapping": ("R62", "R65", "REQ-N01"),
    "n01_language_request_failure_english_failure_and_fallback_cache": ("R62", "REQ-N01"),
    "n01_localized_video_matching_and_mismatched_ids": ("R62", "R64", "REQ-N01"),
    "n01_chinese_english_chinese_race_and_queued_calls": ("R39", "R62", "REQ-N01"),
    "n01_language_storage_failure_preserves_confirmed_locale": ("R39", "R62", "REQ-N01"),
    "n01_cache_bounds_revision_and_synthetic_vectors": ("R46", "R64", "R65", "REQ-N01"),
    "n01_live_selected_tmdb_en_zh_evidence": ("U08", "R06", "R60", "R62", "R65", "REQ-N01"),
}


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("N01-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def save(self):
        current = workflow.snapshot()
        self.case("stable-tested-source", ["REQ-N01"], "source stays unchanged throughout A1",
                  lambda: require(current == self.source, "source changed during A1"))
        summaries = []
        for identifier in IDS:
            children = [case for case in self.cases if identifier in case["requirements"]]
            summaries.append(dict(id=identifier, result="PASS" if children and all(c["result"] == "PASS" for c in children) else "FAIL",
                                  children=[case["id"] for case in children]))
        counts = dict(required=len(self.cases), ran=len(self.cases), passed=sum(c["result"] == "PASS" for c in self.cases),
                      failed=sum(c["result"] != "PASS" for c in self.cases), skipped=0)
        report = dict(card="T0.5", original_card="N01", suite="all", utc_started=self.started,
                      utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
                      tested_source_digest=current, protected=workflow.protected_hash(), platform=sys.platform,
                      arch=platform.machine(), os=platform.platform(), commands=self.commands,
                      cases=self.cases, summaries=summaries, observed=self.observed, counts=counts,
                      native_counts=self.observed.get("native_counts", dict(required=len(RUST_TESTS), ran=0, passed=0, failed=0, skipped=0, status="NOT RUN")),
                      deployment="native production code; ordinary loopback protocol fixtures and external public TMDB metadata; no cluster/Android",
                      cleanup="fixture Drop joins its own workers; original runner owns descendant cleanup",
                      scope="Stremio catalog/meta and Host transactions, cancellation, locale epochs; metadata-only resolve=false")
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="N01-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="N01", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="N01 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
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
    require(receipt["source"] == run.source and receipt["card"] == "T0.5", "receipt source/card differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_mib"] == 4096
            and env["jobs"] == 2 and env["test_threads"] == 1, "resource receipt backend/budget differs")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "local macOS arm64 gate required")
    require(env["memory_enforcement"] == "rss-sampled", "actual RSS sampling backend required")
    run.observed["resources"] = dict(receipt=str(path.relative_to(ROOT)), environment=env,
                                     note="RSS sampling allows transient overage; no cgroup hard memory/CPU quota")
    return run.observed["resources"]


def rust(run):
    packages = tomllib.loads((ROOT / "src-tauri/Cargo.lock").read_text(encoding="utf-8"))["package"]
    run.observed["versions"] = dict(python=sys.version, rustc=run.command(["rustc", "--version"], timeout=30).strip(),
                                    cargo=run.command(["cargo", "--version"], timeout=30).strip(),
                                    dependencies={name: [p["version"] for p in packages if p["name"] == name]
                                                  for name in ("tauri", "reqwest", "tokio", "url")})
    live = run.folder / "live-tmdb"
    env = {name: "http://127.0.0.1:1" for name in
           ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy")}
    env.update(N01_PROXY_TEST="1", NO_PROXY="", no_proxy="", N01_LIVE_EVIDENCE=str(live))
    previous = {name: os.environ.get(name) for name in env}
    output = ""
    try:
        os.environ.update(env)
        try:
            output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
                                  "--lib", "n01_", "--", "--test-threads=1", "--nocapture"], timeout=3000)
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
    names = set(re.findall(r"^test (?:\w+::)*(n01_\w+) \.\.\.", output, re.M))
    require(names == set(RUST_TESTS), f"native test inventory differs: {names ^ set(RUST_TESTS)}")
    summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", output)
    require(summary is not None and int(summary[1]) == len(RUST_TESTS), "all native tests must execute and pass")
    require("N01_PROXY_ENV:true" in output, "real HTTP requests with rejecting proxy variables required")
    run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=len(names), passed=len(names), failed=0, skipped=0, status="PASS")
    for name, requirements in RUST_TESTS.items():
        run.case(name, requirements, "production Rust behavior assertions executed and passed",
                 lambda name=name: dict(test=name, exit=0, log=run.commands[-1]["log"]))
    observations = [json.loads(line.split("N01_OBSERVATION:", 1)[1]) for line in output.splitlines() if "N01_OBSERVATION:" in line]
    require(observations, "serialized production Host responses required")
    from jsonschema import Draft202012Validator, FormatChecker
    from referencing import Registry, Resource
    app, bridge = read_json(ROOT / "protocol/app.schema.json"), read_json(ROOT / "protocol/bridge.openapi.json")
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
    require({"sync_catalog", "get_home", "get_details", "save_settings", "get_sources", "cancel_request", "resolve_episode"} <= seen,
            "required production command observations missing")
    write_json(run.folder / "native-observations.json", observations)
    run.observed["rust"] = dict(tests=len(names), observations=len(observations), exit=0, log=run.commands[-1]["log"],
                                trace=str((run.folder / "native-observations.json").relative_to(ROOT)))
    return run.observed["rust"]


def live_evidence(run):
    folder = run.folder / "live-tmdb"
    records = read_json(folder / "requests.json")
    require(len(records) == 4, "exact real en/zh manifest and meta evidence required")
    expected = {(language, resource) for language in ("en-US", "zh-CN") for resource in ("manifest", "meta")}
    seen, versions, titles = set(), set(), {}
    for row in records:
        require(row["method"] == "GET" and row["headers"] == {}, "anonymous public request required")
        require(row["requested_url"].startswith("https://tmdb.elfhosted.com/")
                and row["final_url"].startswith("https://tmdb.elfhosted.com/"), "selected addon origin differs")
        language = "zh-CN" if "/zh-CN/" in row["requested_url"] else "en-US"
        resource = "manifest" if row["requested_url"].endswith("manifest.json") else "meta"
        require(row["requested_url"] == f'https://tmdb.elfhosted.com/{language}/'
                + ("manifest.json" if resource == "manifest" else "meta/movie/tmdb:27205.json"), "exact language path differs")
        body = (folder / row["body"]).read_bytes()
        require(hashlib.sha256(body).hexdigest() == row["sha256"] and len(body) == row["bytes"], "live response digest differs")
        value = json.loads(body)
        if resource == "manifest":
            require(value["id"] == "tmdb-addon" and value["version"] == row["manifest_version"], "real addon version missing")
            versions.add(value["version"])
        else:
            require(value["meta"]["id"] == "tmdb:27205" and value["meta"]["type"] == "movie", "real same-ID response required")
            titles[language] = value["meta"]["name"]
        dt.datetime.fromisoformat(row["received_utc"].replace("Z", "+00:00"))
        seen.add((language, resource))
    require(seen == expected and len(versions) == 1, "one exact addon version with both languages required")
    require(titles["en-US"] != titles["zh-CN"] and any("\u4e00" <= c <= "\u9fff" for c in titles["zh-CN"]), "real Chinese localization required")
    run.observed["live_addon"] = dict(version=next(iter(versions)), titles=titles, requests=records,
                                      evidence=str(folder.relative_to(ROOT)), synthetic=False,
                                      reference="https://github.com/mrcanelas/tmdb-addon/blob/main/addon/index.js",
                                      language_rule="explicit en-US/zh-CN configuration path; anonymous headers")
    return run.observed["live_addon"]


def main():
    if not __debug__:
        raise SystemExit("Acceptance requires assertions; remove -O")
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    options = parser.parse_args()
    run = Acceptance(options.report, options.junit)
    ready = run.case("resource-ownership", ["R06", "R39", "REQ-N01"], "original scope, source and local platform", lambda: resources(run))
    native = run.case("native-behaviors", list(IDS), "all named native behaviors and independent DTO validation pass",
                      lambda: rust(run) if ready else require(False, "NOT RUN: resource ownership failed"))
    run.case("real-language-evidence", ["U08", "R60", "R62", "R65", "REQ-N01"], "real selected addon same-ID en/zh requests and responses",
             lambda: live_evidence(run) if native else require(False, "NOT RUN: native behaviors failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
