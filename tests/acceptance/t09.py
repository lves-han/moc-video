"""T09 real Rust gateway gate; ordinary owned sockets, no cluster or Android.

Native source callback contexts are synthetic. All resource requests, HTTP
fixtures, payload/lease assertions and sampled streaming RSS are executed.
"""
import argparse
import copy
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import threading
import time
import uuid
import xml.etree.ElementTree as ET

from t00 import Acceptance as BaseAcceptance, read_json, require, workflow, write_json

ROOT = Path(__file__).resolve().parents[2]
IDS = ("R06", "R12", "R19", "R25", "R26", "R27", "R31", "R34", "R37", "R44", "R46", "R63", "REQ-T09")
RUST_TESTS = {
    "t09_capabilities_bearer_cors_csp_and_resource_isolation": ("R06", "R12", "R26", "R27", "R34", "R37", "R44", "R46", "REQ-T09"),
    "t09_native_headers_cookie_referer_interceptor_and_no_local_secrets": ("R19", "R26", "R34", "R46", "REQ-T09"),
    "t09_range_if_range_200_206_416_and_head": ("R06", "R19", "R34", "REQ-T09"),
    "t09_gzip_redirects_private_hops_and_html": ("R19", "R26", "R34", "R37", "REQ-T09"),
    "t09_slow_timeout_truncation_disconnect_and_close_cancel": ("R12", "R25", "R34", "R44", "REQ-T09"),
    "t09_bounded_stream_backpressure_and_real_payload": ("R06", "R34", "R44", "REQ-T09"),
    "t09_active_over_five_minutes_release_and_original_network_context": ("R25", "R63", "REQ-T09"),
    "t09_image_decoders_byte_pixel_limits_and_mime": ("R31", "R37", "R44", "REQ-T09"),
    "t09_image_sliding_lease_readonly_status_lru_and_epoch": ("R25", "R31", "R44", "REQ-T09"),
    "t09_diagnostics_request_sequence_and_other_resource_success": ("R25", "R31", "R34", "R63", "REQ-T09"),
    "t09_same_episode_multiple_streams_and_late_close_release": ("R25", "R63", "REQ-T09"),
    "t09_ssrf_actual_peer_dns_mixture_and_rebinding": ("R26", "R27", "R37", "R46", "REQ-T09"),
    "t09_host_session_bindings_switch_release_and_old_cleanup": ("R12", "R19", "R25", "R34", "R63", "REQ-T09"),
    "t09_host_image_maps_recursive_dedupe_and_generation": ("R31", "R63", "REQ-T09"),
    "t09_host_inflight_media_revoke_and_control_responsive": ("R25", "R44", "REQ-T09"),
    "t09_host_inflight_resolve_cancel_and_late_result_rejected": ("R25", "R63", "REQ-T09"),
}


def redact(line):
    # Assertions can print an unexpected URL/token on failure. Redact before
    # persisting subprocess output as well as in successful observations.
    return re.sub(r"(?<![A-Za-z0-9_-])[A-Za-z0-9_-]{43}(?![A-Za-z0-9_-])", "<redacted-cap-256>", line)


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T09-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}
        self.sample_path = self.folder / "stream-phase.json"

    def command(self, argv, timeout=3000):
        argv = workflow.runtime.resolve_argv([str(a) for a in argv])
        path = self.folder / f"command-{len(self.commands) + 1}.log"
        record = dict(argv=argv, cwd=str(ROOT), exit=None, log=path.relative_to(ROOT).as_posix(),
                      env_overrides=dict(T09_MEMORY_SAMPLE=str(self.sample_path)))
        self.commands.append(record)
        started = time.monotonic()
        samples, sampling_errors = [], []
        credential_lines = []
        stopped = threading.Event()

        def sample():
            identity = None
            while not stopped.wait(0.02):
                try:
                    row = read_json(self.sample_path)
                    if row.get("phase") not in ("baseline", "stream"):
                        continue
                    process = workflow.runtime.psutil.Process(row["pid"])
                    current = (process.pid, process.create_time())
                    if identity is None:
                        require(os.getpid() in {p.pid for p in process.parents()}, "stream sample PID is outside this acceptance process tree")
                        identity = current
                    require(current == identity, "stream sample PID identity changed")
                    samples.append(dict(phase=row["phase"], pid=process.pid, identity=current[1],
                                        rss=process.memory_info().rss, seconds=time.monotonic() - started))
                except (FileNotFoundError, json.JSONDecodeError, workflow.runtime.psutil.NoSuchProcess):
                    continue
                except Exception as error:
                    sampling_errors.append(f"{type(error).__name__}: {error}")
                    return

        monitor = threading.Thread(target=sample)
        monitor.start()
        with path.open("w", encoding="utf-8") as log:
            log.write(json.dumps(argv) + "\n")
            log.flush()
            process = None
            drain = None
            try:
                process = subprocess.Popen(argv, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                           text=True, env=dict(os.environ, T09_MEMORY_SAMPLE=str(self.sample_path)))

                def output():
                    for line in process.stdout:
                        safe = redact(line)
                        if safe != line:
                            credential_lines.append(True)
                        log.write(safe)
                        log.flush()

                drain = threading.Thread(target=output, daemon=True)
                drain.start()
                record["exit"] = process.wait(timeout=timeout)
                drain.join(timeout=5)
                require(not drain.is_alive(), "native descendant kept output open after cargo exit")
            finally:
                if process is not None and process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
                if drain is not None:
                    drain.join(timeout=5)
                stopped.set()
                monitor.join(timeout=5)
                record["seconds"] = time.monotonic() - started
                write_json(self.folder / "stream-rss.json", dict(samples=samples, errors=sampling_errors))
        self.observed["stream_memory"] = dict(samples=samples, errors=sampling_errors,
            trace=str((self.folder / "stream-rss.json").relative_to(ROOT)), enforcement="RSS sampled; no hard memory isolation")
        self.observed["native_credential_lines"] = len(credential_lines)
        require(record["exit"] == 0, f"native command failed ({record['exit']}); log={record['log']}")
        require(not sampling_errors, f"stream RSS sampling failed: {sampling_errors}")
        require(not credential_lines, "native output contained a credential; persisted output was sanitized")
        return path.read_text(encoding="utf-8", errors="replace").split("\n", 1)[1]

    def save(self):
        current = workflow.snapshot()
        self.case("stable-tested-source", ["REQ-T09"], "source unchanged during A1",
                  lambda: require(current == self.source, "source changed during A1"))
        summaries = []
        for identifier in IDS:
            children = [case for case in self.cases if identifier in case["requirements"]]
            summaries.append(dict(id=identifier, children=[case["id"] for case in children],
                result="PASS" if children and all(case["result"] == "PASS" for case in children) else "FAIL"))
        counts = dict(required=len(self.cases), ran=len(self.cases),
                      passed=sum(case["result"] == "PASS" for case in self.cases),
                      failed=sum(case["result"] != "PASS" for case in self.cases), skipped=0)
        report = dict(card="T0.9", original_card="T09", suite="all", utc_started=self.started,
            utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
            tested_source_digest=current, protected=workflow.protected_hash(), platform=sys.platform,
            arch=platform.machine(), os=platform.platform(), commands=self.commands, cases=self.cases,
            summaries=summaries, observed=self.observed, counts=counts,
            native_counts=self.observed.get("native_counts", dict(required=len(RUST_TESTS), ran=0, passed=0, failed=0, skipped=0, status="NOT RUN")),
            deployment="ordinary native process and owned loopback TCP/UDP fixtures; no cluster or Android",
            cleanup="fixture Drop stops/joins owned sockets/workers; gateway aborts/joins requests; original run.py cleans descendants",
            image_limits=dict(decoder="image 0.25.9; PNG/JPEG/GIF/WebP", bytes=8*1024*1024,
                              pixels=16777216, dimension=8192, decoder_allocation_bytes=128*1024*1024,
                              allocation_enforcement="decoder best effort; strict preflight byte/dimension/pixel checks"),
            cloudstream=dict(synthetic=True, scope="retained original source HTTP interceptor/Cookie callback instances",
                             real_runtime="NOT RUN: original Android provider interceptor, seven packages and DELETE remain N02"),
            later_cards=dict(manifest_rewriting="T10", subtitle_conversion="T11", long_product_endurance="ADR-020 product stage"))
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T09-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T09", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T09 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
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
        receipts = [(p, read_json(p)) for p in (ROOT / "evidence/runs").glob("*/receipt.json")]
        matches = [(p, r) for p, r in receipts if r.get("status") == "running" and r.get("payload_pid") in ancestors
                   and workflow.runtime.alive(dict(pid=r["payload_pid"], identity=r["payload_identity"]))]
        if matches:
            break
        time.sleep(0.05)
    require(len(matches) == 1, "one live original resource receipt must own A1")
    path, receipt = matches[0]
    require(receipt["source"] == run.source and receipt["card"] == "T0.9", "receipt source/card differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_mib"] == 4096
            and env["jobs"] == 2 and env["test_threads"] == 1, "receipt backend/budget differs")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "local macOS arm64 gate required")
    require(env["memory_enforcement"] == "rss-sampled", "actual RSS sampled backend required")
    run.observed["resources"] = dict(receipt=str(path.relative_to(ROOT)), environment=env,
                                     note="RSS sampling permits transient overage; no cgroup hard memory or CPU quota")
    return run.observed["resources"]


def check_vector(vector):
    kind = vector["kind"]
    if kind in ("binding", "host_binding"):
        view, ref = vector["session"], vector["reference"]
        require(view["episode_ref"] == ref, "session lost original episode reference")
        require(view["resolution_id"] == vector["resolution_id"] and view["stream_id"] == vector["stream_id"], "session belongs to another candidate")
        require(all(view[k] == ref[k] for k in ("source_id", "source_revision")), "session source/revision binding changed")
        require(view["stream_kind"] == "progressive" and view["mime_type"] == "video/mp4", "session media kind changed")
        url = view["media_url"]
        require(url["scheme"] == "http" and url["host"] == "127.0.0.1", "resource escaped loopback")
        require(url["path"] == f"/media/{view['session_id']}/{view['stream_id']}", "resource path belongs to another session")
        require(url["query_keys"] == ["cap"] and url["cap_bytes"] == 32, "invalid resource capability shape")
        uuid.UUID(view["session_id"])
        if kind == "binding":
            require(url["cap_sha256"] != vector["other_cap_sha256"], "sessions share a capability")
            require(vector["status"] == 200 and vector["cors"] == "tauri://localhost" and vector["unauthorized"] == 401, "CORS/auth isolation changed")
        else:
            uuid.UUID(vector["epoch"])
            require(vector["old_state"] == "expired" and vector["current_state"] == "ready", "old cleanup changed current session")
    elif kind == "range":
        value = vector["requested"].removeprefix("bytes=")
        start, end = value.split("-")
        start = int(start) if start else 4096 - int(end)
        end = min(int(end), 4095) if end and not value.startswith("-") else 4095
        expected = list(n % 256 for n in range(start, end + 1))
        require(vector["status"] == 206 and vector["content_range"] == f"bytes {start}-{end}/4096", "partial response status/range changed")
        require(vector["bytes"] == expected and int(vector["content_length"]) == len(expected), "Range payload/length changed")
    elif kind == "stream":
        require(vector["bytes"] == 256*1024*1024 and vector["first_buffer_limit"] == 65536, "stream size/buffer limit changed")
        require(0 < vector["produced_before_read"] < vector["bytes"] // 2, "upstream lacked backpressure")
        expected = hashlib.sha256()
        for _ in range(4096):
            expected.update(bytes([0x5a])*65536)
        require(vector["sha256"] == expected.hexdigest(), "large payload hash changed")
    elif kind == "lifetime":
        require(vector["active_at_ms"] > 300000 and vector["resolution_released"] is True, "active session did not outlive candidate lease")
        require(vector["continued_bytes"] == 131072 and vector["post_release_context"] == "renewed-after-resolution-release", "session lost original source context")
        require(vector["idle_expired_at_ms"] == 600001, "idle session lease changed")
    elif kind == "image_limits":
        require((vector["bytes"], vector["pixels"], vector["dimension"]) == (8388608, 16777216, 8192), "image limits changed")
        require(set(vector["formats"]) == {"png", "jpeg", "gif", "webp"} and vector["oversized_http"] == 413, "image decoder/limit behavior changed")
    else:
        raise ValueError(f"unknown observation kind: {kind}")


def independent(run, output):
    vectors = [json.loads(line.split("T09_VECTOR:", 1)[1]) for line in output.splitlines() if "T09_VECTOR:" in line]
    require(len(vectors) == 8, f"all eight independent observations required, got {len(vectors)}")
    require({v["kind"] for v in vectors} == {"binding", "host_binding", "range", "stream", "lifetime", "image_limits"}, "observation kinds incomplete")
    rejected = 0
    for vector in vectors:
        check_vector(vector)
        kind = vector["kind"]
        if kind in ("binding", "host_binding"):
            edits = [lambda v: v["session"]["episode_ref"].update(data="foreign"),
                     lambda v: v["session"].update(source_revision="foreign"),
                     lambda v: v["session"]["media_url"].update(path="/media/foreign/unknown"),
                     lambda v: v["session"]["media_url"].update(cap_bytes=16),
                     lambda v: v["session"].update(stream_kind="hls")]
        elif kind == "range":
            edits = [lambda v: v.update(status=200), lambda v: v.update(bytes=[]), lambda v: v.update(content_length="0")]
        elif kind == "stream":
            edits = [lambda v: v.update(bytes=0), lambda v: v.update(produced_before_read=v["bytes"]), lambda v: v.update(sha256="corrupt")]
        elif kind == "lifetime":
            edits = [lambda v: v.update(active_at_ms=300000), lambda v: v.update(resolution_released=False), lambda v: v.update(post_release_context="lost")]
        else:
            edits = [lambda v: v.update(pixels=16777217), lambda v: v.update(oversized_http=200), lambda v: v.update(formats=[])]
        for edit in edits:
            corrupt = copy.deepcopy(vector)
            edit(corrupt)
            try:
                check_vector(corrupt)
            except (ValueError, KeyError, IndexError):
                rejected += 1
            else:
                raise ValueError(f"oracle accepted corrupted {kind}")
    path = run.folder / "gateway-observations.json"
    write_json(path, vectors)
    memory = run.observed["stream_memory"]
    require(not memory["errors"], "stream RSS sampler had errors")
    baseline = [s["rss"] for s in memory["samples"] if s["phase"] == "baseline"]
    stream = [s["rss"] for s in memory["samples"] if s["phase"] == "stream"]
    require(len(baseline) >= 2 and len(stream) >= 2, "actual baseline/stream RSS samples missing")
    growth = max(stream) - min(baseline)
    require(growth <= 64*1024*1024, f"256 MiB streaming body grew RSS by {growth} bytes; limit=64 MiB")
    memory.update(baseline_rss=min(baseline), peak_stream_rss=max(stream), growth_bytes=growth,
                  functional_growth_limit=64*1024*1024)
    run.observed["independent"] = dict(vectors=len(vectors), corruptions_rejected=rejected,
                                       trace=str(path.relative_to(ROOT)))
    return run.observed["independent"]


def rust(run):
    try:
        output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
                              "--lib", "t09_", "--", "--test-threads=1", "--nocapture"])
    except Exception:
        output = (ROOT / run.commands[-1]["log"]).read_text(encoding="utf-8", errors="replace")
        result = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", output)
        passed, failed, skipped = map(int, result.groups()) if result else (0, 0, 0)
        run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=passed + failed + skipped, passed=passed,
            failed=failed, skipped=skipped, status="FAIL" if result else "NOT RUN", log=run.commands[-1]["log"])
        raise
    names = set(re.findall(r"^test (?:\w+::)*(t09_\w+) \.\.\.", output, re.M))
    require(names == set(RUST_TESTS), f"native inventory differs: {names ^ set(RUST_TESTS)}")
    summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", output)
    require(summary is not None and int(summary[1]) == len(RUST_TESTS), "all nonzero named native tests must pass")
    run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=len(names), passed=len(names), failed=0, skipped=0, status="PASS")
    for name, requirements in RUST_TESTS.items():
        run.case(name, requirements, "production behavior assertions executed and passed",
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
    require(options.report.resolve() == ROOT / "evidence/acceptance/T09-all.json"
            and options.junit.resolve() == ROOT / "evidence/acceptance/T09-all.xml", "T09 report destinations differ from A1")
    run = Acceptance(options.report, options.junit)
    ready = run.case("resource-ownership", ["REQ-T09"], "original resource scope and local platform", lambda: resources(run))
    run.case("native-behaviors", list(IDS), "named native tests and independent gateway observations with RSS samples",
             lambda: rust(run) if ready else require(False, "NOT RUN: resource ownership failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
