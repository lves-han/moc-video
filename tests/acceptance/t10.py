"""T10: real native HLS gateway and owned H.264/AAC decode fixtures.

Executed only under the original run.py resource scope. Native source callbacks
are synthetic; real Android/Yjys interceptor order belongs to N02.
"""
import argparse
import copy
import datetime as dt
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time
import uuid
import xml.etree.ElementTree as ET

from t00 import Acceptance as BaseAcceptance, read_json, require, workflow, write_json
from t09 import redact as redact_credentials

ROOT = Path(__file__).resolve().parents[2]
IDS = ("R26", "R28", "REQ-T10")
RUST_TESTS = {
    "t10_structured_independent_uri_and_timeline_fixtures": IDS,
    "t10_invalid_uri_drm_and_atomic_registry_limits": IDS,
    "t10_http_master_tracks_keys_map_and_unknown_tags": IDS,
    "t10_cross_origin_private_key_and_closed_cap": ("R26", "REQ-T10"),
    "t10_source_response_gzip_order_and_entity_headers": IDS,
    "t10_conditionals_html_sniff_and_refresh_headers": IDS,
    "t10_active_session_release_clock_refresh_key_segment_and_close": IDS,
    "t10_real_aes128_explicit_implicit_iv_and_fmp4_decode": IDS,
}
URI_COUNTS = [1, 1, 1, 1, 1, 1, 1, 2, 3, 2, 1]


def redact(output):
    # Only Cargo's fixed runner positions may contain a nonsecret test name
    # that happens to have the same shape as a capability. Sanitize the rest
    # of each line, including any JSON/URL/Debug output after the runner prefix.
    names = "(?:" + "|".join(map(re.escape, RUST_TESTS)) + ")"
    qualified = r"gateway::tests::t10::" + names
    runner = re.compile(r"^(?:test " + qualified + r" \.\.\.(?=\s|$)|    "
                        + qualified + r"(?=\r?$)|thread '" + qualified
                        + r"' \(\d+\) panicked at )")
    safe = []
    for line in output.splitlines(keepends=True):
        match = runner.match(line)
        end = match.end() if match else 0
        safe.append(line[:end] + redact_credentials(line[end:]))
    return "".join(safe)


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T10-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def command(self, argv, timeout=3000):
        argv = workflow.runtime.resolve_argv([str(a) for a in argv])
        path = self.folder / f"command-{len(self.commands) + 1}.log"
        record = dict(argv=argv, cwd=str(ROOT), exit=None, log=path.relative_to(ROOT).as_posix())
        self.commands.append(record)
        started = time.monotonic()
        process = None
        with path.open("w", encoding="utf-8") as log:
            log.write(json.dumps(argv) + "\n")
            log.flush()
            try:
                process = subprocess.Popen(argv, cwd=ROOT, stdout=subprocess.PIPE,
                                           stderr=subprocess.STDOUT, text=True)
                output, _ = process.communicate(timeout=timeout)
                record["exit"] = process.returncode
                safe = redact(output)
                log.write(safe)
                log.flush()
                require(safe == output, "native output contained a credential; persisted output was sanitized")
            finally:
                if process is not None and process.poll() is None:
                    process.kill()
                    output, _ = process.communicate(timeout=5)
                    log.write(redact(output))
                record["seconds"] = time.monotonic() - started
        require(record["exit"] == 0, f"native command failed ({record['exit']}); log={record['log']}")
        return safe

    def save(self):
        self.case("stable-tested-source", ["REQ-T10"], "source unchanged during A1",
                  lambda: require(workflow.snapshot() == self.source, "source changed during A1"))
        summaries = []
        for identifier in IDS:
            children = [c for c in self.cases if identifier in c["requirements"]]
            summaries.append(dict(id=identifier, children=[c["id"] for c in children],
                result="PASS" if children and all(c["result"] == "PASS" for c in children) else "FAIL"))
        counts = dict(required=len(self.cases), ran=len(self.cases),
                      passed=sum(c["result"] == "PASS" for c in self.cases),
                      failed=sum(c["result"] != "PASS" for c in self.cases), skipped=0)
        report = dict(card="T0.10", original_card="T10", suite="all", utc_started=self.started,
            utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
            tested_source_digest=workflow.snapshot(), protected=workflow.protected_hash(),
            platform=sys.platform, arch=platform.machine(), os=platform.platform(),
            commands=self.commands, cases=self.cases, summaries=summaries, observed=self.observed,
            counts=counts, native_counts=self.observed.get("native_counts", dict(required=len(RUST_TESTS),
                ran=0, passed=0, failed=0, skipped=0, status="NOT RUN")),
            media=dict(provenance="CC0-1.0; project-generated testsrc2 and sine tone", duration_seconds=2,
                modes=["AES-128 TS explicit IV", "AES-128 TS implicit sequence IV", "fMP4 single-file init/byterange"],
                verification="original HLS and gateway each equal all 24 source video frame hashes; nonzero decoded AAC frames in each mode",
                fmp4_client="http_seekable=0 avoids FFmpeg 9.0.2 AVIO offset reuse; explicit HLS Range requests remain asserted"),
            deployment="ordinary native process and owned loopback socket fixtures; no cluster",
            cleanup="fixture/gateway Drop joins owned workers; temporary media deleted after fixture shutdown; original run.py owns descendants",
            cloudstream=dict(synthetic=True, original_yjys="NOT RUN: real Cloudstream source/interceptor order remains N02"),
            dash=dict(status="NOT RUN", reason="this native HLS fixture set emits no DASH; inspect actual original-provider baseline in N02"),
            later_cards=dict(subtitle_conversion="T11", long_product_endurance="ADR-020 product stage"))
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T10-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(r["result"] != "PASS" for r in rows)), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T10", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T10 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return int(counts["failed"] > 0 or any(r["result"] != "PASS" for r in summaries))


def resources(run):
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "original run.py wrapper required")
    cfg = workflow.config()
    require((cfg["memory_mib"], cfg["jobs"]) == (4096, 2), "approved resource budget differs")
    require(os.environ.get("CARGO_BUILD_JOBS") == "2" and os.environ.get("RUST_TEST_THREADS") == "1", "concurrency differs")
    require(Path(os.environ["CARGO_TARGET_DIR"]) == ROOT / "target", "target binding differs")
    require(Path.cwd().resolve() == ROOT and Path(sys.executable).absolute() == ROOT / ".venv/bin/python", "root/venv binding differs")
    ancestors = {p.pid for p in workflow.runtime.psutil.Process().parents()} | {os.getpid()}
    deadline, matches = time.monotonic() + 5, []
    while time.monotonic() < deadline:
        receipts = [(p, read_json(p)) for p in (ROOT / "evidence/runs").glob("*/receipt.json")]
        matches = [(p, r) for p, r in receipts if r.get("status") == "running" and r.get("payload_pid") in ancestors
                   and workflow.runtime.alive(dict(pid=r["payload_pid"], identity=r["payload_identity"]))]
        if matches:
            break
        time.sleep(0.05)
    require(len(matches) == 1, "one live original resource receipt must own A1")
    path, receipt = matches[0]
    require(receipt["source"] == run.source and receipt["card"] == "T0.10", "receipt source/card differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_mib"] == 4096
            and env["jobs"] == 2 and env["test_threads"] == 1, "actual receipt backend/budget differs")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "local macOS arm64 gate required")
    require(env["memory_enforcement"] == "rss-sampled", "actual RSS sampled backend required")
    run.observed["resources"] = dict(receipt=str(path.relative_to(ROOT)), environment=env,
        note="RSS sampling permits transient overage; no cgroup hard memory or CPU quota")
    return run.observed["resources"]


def check_vector(v):
    if v["kind"] == "parser":
        require(v["fixture"] in range(len(URI_COUNTS)) and v["uri_count"] == URI_COUNTS[v["fixture"]]
                and v["preserved"] is True, "URI span count or byte preservation changed")
    elif v["kind"] == "http":
        require(v["same_cap"] is True and v["refreshed_stable"] is True
                and v["references"] == 16 and v["registry_count"] == 13, "resource identity/context observation changed")
        require({"/final/master.m3u8?sig=redirect", "/final/audio.aac?sig=a%2Fb+x", "/subs/caption.vtt",
                 "/final/key?cap=upstream&sig=a%2Fb+x", "/final/blob.mp4?sig=range", "/final/future.bin"}
                <= set(v["requests"]), "key/map/audio/subtitle/signature requests missing")
    elif v["kind"] == "response_order":
        require(v["source_calls"] == 3 and v["decoded_before_source"] is True and v["source_before_rewrite"] is True,
                "HTTP/source/rewrite order changed")
        require(v["conditional_status"] == 304 and int(v["head_length"]) == v["body_bytes"] > 0
                and v["etag"] == '"' + v["sha256"] + '"', "transformed entity headers changed")
    elif v["kind"] == "lifetime":
        require(v["active_at_ms"] > 300000 and v["resolution_released"] is True
                and v["continued_bytes"] == 131072 and v["idle_expired_at_ms"] == 600001
                and v["late_close_cancelled"] is True, "session lifetime/cancellation changed")
        require(set(v["post_release_targets"]) == {"/live.m3u8", "/key", "/segment.ts"}, "post-release source context lost")
    elif v["kind"] == "decode":
        require(v["mode"] in ("aes-explicit", "aes-implicit", "fmp4") and v["video_frames"] == 24
                and v["audio_frames"] > 0 and v["source_sha256"] == v["decoded_sha256"]
                and re.fullmatch("[a-f0-9]{64}", v["source_sha256"]) is not None,
                "real decoded frame equality/audio missing")
        require(v["resolution_released"] is True and v["same_cap"] is True, "decode lost session ownership")
        requests = v["requests"]
        require(any("/index.m3u8?sig=A%2FB+x&cap=upstream" in r["target"] for r in requests), "signed media playlist missing")
        if v["mode"].startswith("aes"):
            require(any("/key.bin?sig=A%2FB+x&cap=upstream" in r["target"] for r in requests), "AES key bypassed gateway")
            require(any("/segment-" in r["target"] for r in requests), "encrypted segments missing")
        else:
            require(sum("stream.mp4" in r["target"] and r["range"] is not None for r in requests) >= 2,
                    "fMP4 init/segment byterange requests missing")
    else:
        raise ValueError("unknown T10 observation kind")


def independent(run, output):
    vectors = [json.loads(line.split("T10_VECTOR:", 1)[1]) for line in output.splitlines() if "T10_VECTOR:" in line]
    require(len(vectors) == 17, f"17 actual independent observations required, got {len(vectors)}")
    require({v["fixture"] for v in vectors if v["kind"] == "parser"} == set(range(11)), "independent fixtures incomplete")
    require({v["mode"] for v in vectors if v["kind"] == "decode"} == {"aes-explicit", "aes-implicit", "fmp4"}, "decode modes incomplete")
    require({v["kind"] for v in vectors} == {"parser", "http", "response_order", "lifetime", "decode"}, "observation kinds missing")
    rejected = 0
    for vector in vectors:
        check_vector(vector)
        key = dict(parser="preserved", http="same_cap", response_order="source_before_rewrite",
                   lifetime="resolution_released", decode="same_cap")[vector["kind"]]
        edits = [(key, False)]
        if vector["kind"] == "decode":
            edits += [("video_frames", 0), ("audio_frames", 0), ("decoded_sha256", "corrupt"), ("requests", [])]
        elif vector["kind"] == "response_order":
            edits += [("etag", '"upstream"'), ("head_length", "1"), ("conditional_status", 200)]
        elif vector["kind"] == "lifetime":
            edits += [("active_at_ms", 300000), ("post_release_targets", [])]
        elif vector["kind"] == "http":
            edits += [("requests", []), ("refreshed_stable", False)]
        else:
            edits += [("uri_count", 999)]
        for key, value in edits:
            corrupt = copy.deepcopy(vector)
            corrupt[key] = value
            try:
                check_vector(corrupt)
            except (ValueError, KeyError, IndexError):
                rejected += 1
            else:
                raise ValueError(f"oracle accepted corrupted {vector['kind']}/{key}")
    path = run.folder / "hls-observations.json"
    write_json(path, vectors)
    run.observed["independent"] = dict(vectors=len(vectors), corruptions_rejected=rejected, trace=str(path.relative_to(ROOT)))
    return run.observed["independent"]


def rust(run):
    try:
        output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
                              "--lib", "t10_", "--", "--test-threads=1", "--nocapture"])
    except Exception:
        output = (ROOT / run.commands[-1]["log"]).read_text(encoding="utf-8", errors="replace")
        result = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", output)
        passed, failed, skipped = map(int, result.groups()) if result else (0, 0, 0)
        run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=passed + failed + skipped,
            passed=passed, failed=failed, skipped=skipped, status="FAIL" if result else "NOT RUN", log=run.commands[-1]["log"])
        raise
    names = set(re.findall(r"^test (?:\w+::)*(t10_\w+) \.\.\.", output, re.M))
    require(names == set(RUST_TESTS), f"native inventory differs: {names ^ set(RUST_TESTS)}")
    summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", output)
    require(summary is not None and int(summary[1]) == len(RUST_TESTS), "all nonzero named native tests must pass")
    run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=len(names), passed=len(names), failed=0, skipped=0, status="PASS")
    for name, requirements in RUST_TESTS.items():
        run.case(name, requirements, "production assertions and real requests/decode passed",
                 lambda name=name: dict(test=name, exit=0, log=run.commands[-1]["log"]))
    return independent(run, output)


def main():
    if not __debug__:
        raise SystemExit("Acceptance requires assertions; remove -O")
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    options = parser.parse_args()
    require(options.report.resolve() == ROOT / "evidence/acceptance/T10-all.json"
            and options.junit.resolve() == ROOT / "evidence/acceptance/T10-all.xml", "T10 destinations differ from A1")
    run = Acceptance(options.report, options.junit)
    ready = run.case("resource-ownership", ["REQ-T10"], "original resource scope and local platform", lambda: resources(run))
    run.case("native-behaviors", list(IDS), "eight named native tests and 17 independent HLS observations",
             lambda: rust(run) if ready else require(False, "NOT RUN: resource ownership failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
