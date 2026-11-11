"""T21: current Rust gateway regressions and the official player on owned media.

The original run.py owns every subprocess, including the existing packaged
WKWebView harness and its identity-registered XPC helpers. No cluster is used.
"""
import argparse
import copy
import json
import os
from pathlib import Path
import re
import subprocess
import sys

import t01
import t02
import t09
import t10
import t11
import t12
import t18
from t00 import file_hash, read_json, require, write_json

ROOT = Path(__file__).resolve().parents[2]
IDS = ["R05", "R12", "R19", "R26", "R27", "R28", "R29", "R34", "R37",
       "R44", "R46", "R48", "R49", "R50", "C-RESOURCE", "REQ-T21"]
IPC = ["t05_contract_commands_and_envelopes", "t05_permissions_and_actual_tauri_handler",
       "t05_configuration_limits_and_utf8_budgets", "t05_uuid_duplicates_self_cancel_and_cascade",
       "t05_deadlines_include_queue_and_discard_old_work"]
HTTP = ["n00_public_addresses_dns_rebinding_and_peer", "n00_redirect_origin_headers_proxy_and_limits",
        "t02_dns_two_servers_failover_and_both_families",
        "t02_dns_both_failed_never_falls_back_to_hosts_or_system",
        "t02_dns_fake_ip_mixed_results_and_redirect_rebinding_rejected"]


class Acceptance(t02.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.25", "T21", IDS
    DEPLOYMENT = "ordinary owned loopback HTTP fixtures, real Rust Host/gateway, Tauri MockRuntime permissions and packaged macOS arm64 WKWebView; no cluster"
    DEFERRED = [dict(target="macOS x86_64 / Windows / Linux", status="NOT RUN", reason="ADR-005/006"),
                dict(target="10-minute gateway endurance", status="NOT RUN", reason="ADR-044; T0.26/A2")]

    def __init__(self, report, junit):
        super().__init__(report, junit)
        self.sample_path = self.folder / "stream-phase.json"

    def command(self, argv, timeout=3000):
        if "t09_" in argv:
            return t09.Acceptance.command(self, argv, timeout)
        return super().command(argv, timeout)


def group(run, name, action):
    """Preserve the existing independent oracles and each native inventory."""
    result = action(run)
    run.observed.setdefault("native_groups", {})[name] = copy.deepcopy(dict(
        result=result, counts=run.observed.get("native_counts"),
        independent=run.observed.get("independent")))
    return result


def selected_native(run, target, names):
    logs = []
    for name in names:
        output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path",
            "src-tauri/Cargo.toml", *target, name, "--", "--test-threads=1"], timeout=600)
        actual = [value.split("::")[-1] for value in re.findall(r"^test (\S+) \.\.\.", output, re.M)]
        require(actual == [name] and "test result: ok. 1 passed; 0 failed; 0 ignored;" in output,
                "Required named native behavior did not execute: " + name)
        logs.append(dict(test=name, log=run.commands[-1]["log"]))
    return dict(required=len(names), passed=len(names), failed=0, skipped=0, tests=logs)


def check_trace(trace, media):
    files = {row["name"]: row for row in media["files"]}
    actual = {row["name"]: row for row in trace["inventory"]}
    require(len(actual) == len(trace["inventory"]) and actual == files,
            "Rust gateway and player must use the exact same owned media collection")
    require(len(files) == len(media["files"]) and all(row["bytes"] > 0
            and re.fullmatch(r"[0-9a-f]{64}", row["sha256"]) for row in files.values()),
            "Nonempty media files and full SHA256 digests required")
    require({"a.mp4", "b.mp4", "index.m3u8"} <= files.keys()
            and len([name for name in files if re.fullmatch(r"seg-\d+\.ts", name)]) >= 2,
            "Shared H264/AAC/HLS collection incomplete")
    require(trace["active_requests"] == 0 and trace["upstream_active"] == 0
            and trace["local_credentials"] is False, "Host/upstream leaked requests or credentials")
    bindings = {row["session_id"]: row for row in trace["bindings"]}
    require(len(bindings) == len(trace["bindings"]) >= 52, "Issued Session inventory missing or duplicated")
    for row in bindings.values():
        expected = "index.m3u8" if row["episode"] == 51 else "a.mp4" if row["episode"] % 2 == 0 else "b.mp4"
        require(row["root"] == expected, "Issued session points at another episode's media")
        require(all(row[key] for key in ("session_id", "resolution_id", "stream_id", "context")),
                "Session/Resolution/stream/context identity missing")
    requests = trace["requests"]
    require(len(requests) >= 52, "Actual upstream subrequests missing")
    successful = set()
    episodes = set()
    for row in requests:
        owner = row["owner"]
        require(bindings.get(owner["session_id"]) == owner, "Subrequest belongs to an unissued or wrong Session")
        require(row["local_credentials"] is False, "Local credentials reached upstream")
        name = row["name"]
        require(name in files, "Unregistered upstream media file")
        require(name == owner["root"] or name.endswith(".ts") and owner["root"] == "index.m3u8",
                "Subrequest belongs to another stream")
        if row["mode"] in (1, 2, 3):
            require(row["status"] == {1: 401, 2: 502, 3: 200}[row["mode"]], "Fault stimulus/status changed")
        else:
            require(row["status"] == 200 and row["bytes"] == files[name]["bytes"]
                    and row["sha256"] == files[name]["sha256"], "Owned media body changed")
            successful.add(name)
            if name == owner["root"]:
                episodes.add(owner["episode"])
    require(successful == files.keys(), "A shared media file never traversed the Rust gateway")
    require(set(range(52)) <= episodes, "A player episode is missing its associated real upstream request")
    return dict(files=len(files), sessions=len(bindings), subrequests=len(requests), episodes=len(episodes),
                local_credentials=False, active_requests=0, upstream_active=0)


def check_resources(observed):
    sessions = observed["native_sessions"]
    require(len(sessions) == 1, "One packaged player session must be accounted for")
    session = sessions[0]
    require(session["exit"] == 0 and session["lingering"] == [] and session["forced_cleanup"] == [],
            "Packaged application/WebKit cleanup failed")
    require(0 < session["peak_combined_rss_bytes"] <= 4096 * 1024 * 1024,
            "Combined host/WebKit sampled budget missing or exceeded")
    require(session["wake_guard"]["exit"] == 0, "Owned wake guard did not exit normally")
    samples = observed["process_samples"]
    require(len(samples) == 51 and all(sample["owned_rss_bytes"] > 0 for sample in samples),
            "All 51 real process-tree samples required")
    registered = {(row["pid"], row["identity"]) for row in session["owned_processes"]}
    require(len(registered) == len(session["owned_processes"])
            and all(pid > 0 and identity for pid, identity in registered), "Owned process identities missing or duplicated")
    require(set(session["webkit_pids"]) == {"renderer", "gpu", "network"}, "WebKit role inventory incomplete")
    for sample in samples:
        rows = sample["processes"]
        require({"application", "renderer"} <= {row["role"] for row in rows if row["alive"]},
                "Application or WebKit renderer missing from RSS accounting")
        require(all((row["pid"], row["identity"]) in registered for row in rows), "Unregistered process identity")
        require(all(row["rss_bytes"] is not None and row["rss_bytes"] > 0 and row["identity"]
                    for row in rows + sample["support_processes"] if row["alive"]),
                "Live process RSS or creation-time identity missing")
        require(sample["detail_rss_bytes"] == sum(row["rss_bytes"] or 0 for row in rows),
                "Detailed process RSS total omits a measured member")
        require({"python_observer", "real_host"} <= {row["role"] for row in sample["support_processes"] if row["alive"]},
                "Observer/real Host RSS missing")
    for role, pid in session["webkit_pids"].items():
        if pid > 0:
            require(any(row["pid"] == pid for row in session["owned_processes"]), "WebKit helper omitted: " + role)
            require(any(row["pid"] == pid and row["role"] == role and row["alive"]
                        for sample in samples for row in sample["processes"]), "WebKit helper RSS never sampled: " + role)
    return dict(samples=len(samples), registered_processes=len(registered),
                peak_combined_rss_bytes=session["peak_combined_rss_bytes"], enforcement="rss-sampled")


def dash_baseline(run):
    # Inspect actual public-source observations, keeping synthetic DASH probes
    # separate. A newly observed DASH baseline requires its complete C8 gate.
    fixture = read_json(ROOT / "tests/fixtures/t02/public-responses.json")
    path = ROOT / fixture["provenance"]["capture"]
    supported = set(read_json(ROOT / "docs/compatibility-report.json")["supported_sources"])
    samples, baselines = [], []
    # The projection's acquisition and the latest completed public acceptance
    # can contain different samples. Both must agree about conditional DASH.
    latest = ROOT / "evidence/acceptance/T02-all.json"
    for source_path, sources in [(path, read_json(path)["sources"]),
            (latest, read_json(latest)["observed"]["public_sources"])]:
        require(supported <= {row["name"] for row in sources}, "Actual source baseline missing")
        samples.extend(record for row in sources if row["name"] in supported
                       for record in row.get("records", []) if record["stage"] == "direct_media_prefix")
        baselines.append(dict(path=str(source_path.relative_to(ROOT)), sha256=file_hash(source_path)))
    run.observed["dash"] = dict(status="REQUIRES TESTING",
        baselines=baselines, samples=len(samples),
        reason="C8 conditional DASH baseline; complete coverage required if DASH is observed")
    check_dash_samples(samples)
    run.observed["dash"].update(status="NOT RUN",
        reason="No DASH observed in the supported public-source sample; synthetic DASH probes are separate")
    return run.observed["dash"]


def check_dash_samples(samples):
    require(samples and any(row.get("observed_kind", row.get("kind")) == "hls" for row in samples),
            "Actual media format observations missing")
    require(not any(row.get("kind") == "dash" or row.get("observed_kind") == "dash" for row in samples),
            "Actual DASH baseline requires BaseURL/List/Base/SIDX/Template, seek and real player coverage")


def webview(run):
    previous_data, previous_build = t01.DATA, t01.BUILD
    t01.DATA = run.folder / "packaged"
    t01.DATA.mkdir()
    t01.BUILD = t01.DATA / "build.json"
    previous_media = t12.owned_media
    previous_environment = os.environ.get("T21_MEDIA")

    def owned_media(run, folder):
        previous_media(run, folder)
        os.environ["T21_MEDIA"] = str(folder)

    t12.owned_media = owned_media
    try:
        os.environ.pop("T21_MEDIA", None)
        result = t12.webview(run, check_security=True)
        path = run.folder / "real-host.log"
        vectors = [json.loads(line.split("T21_VECTOR:", 1)[1]) for line in path.read_text().splitlines()
                   if "T21_VECTOR:" in line]
        require(len(vectors) == 1, "Exact actual Rust upstream Session trace required")
        trace = vectors[0]
        run.observed["gateway_trace"] = trace
        result["trace"] = check_trace(trace, run.observed["media"])
        result["resources"] = check_resources(run.observed)
        return result
    finally:
        if previous_environment is None:
            os.environ.pop("T21_MEDIA", None)
        else:
            os.environ["T21_MEDIA"] = previous_environment
        t12.owned_media = previous_media
        t01.DATA, t01.BUILD = previous_data, previous_build


def oracles(run):
    output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t21.py"], timeout=120)
    count = re.search(r"Ran (\d+) tests?", output)
    require(count and int(count[1]) >= 8 and re.search(r"^OK$", output, re.M), "T21 independent oracle negatives must execute")
    return dict(tests=int(count[1]))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(__debug__, "Acceptance requires assertions; remove -O")
    require(args.report.resolve() == ROOT / "evidence/acceptance/T21-all.json"
            and args.junit.resolve() == ROOT / "evidence/acceptance/T21-all.xml", "Exact T21 report paths required")
    t02.NATIVE |= set(t09.RUST_TESTS) | set(t10.RUST_TESTS) | {name.split("::")[-1] for name in t11.RUST_TESTS} | set(IPC + HTTP) | {t12.LIVE_TEST.split("::")[-1]}
    run = Acceptance(args.report, args.junit)
    if run.case("resource-ownership", ["C-RESOURCE", "REQ-T21"], "original receipt and 4096MiB macOS RSS sampling", lambda: t10.resources(run, card="T0.25")):
        run.case("oracle-negatives", ["R28", "R49", "C-RESOURCE", "REQ-T21"], "wrong subrequests and omitted resources fail", lambda: oracles(run))
        run.case("wire-contracts", ["R12", "R19", "R44", "REQ-T21"], "23 command contracts including invalid envelopes", lambda: t18.contracts(run))
        run.case("tauri-permissions-and-inputs", ["R12", "R19", "R44", "REQ-T21"], "actual Tauri handler, foreign origins/windows, arbitrary inputs, body and UTF8 limits, deadlines/cancel", lambda: selected_native(run, ["--test", "t05"], IPC))
        run.case("http-boundary", ["R05", "R19", "R46", "REQ-T21"], "real public DNS/peer, redirects, sensitive headers, proxy and limits", lambda: selected_native(run, ["--lib"], HTTP))
        run.case("gateway-regressions", ["R05", "R12", "R19", "R26", "R27", "R34", "R37", "R44", "R46", "C-RESOURCE", "REQ-T21"], "all 16 gateway/host tests, byte-exact Range, cancel, image leases/LRU/limits, backpressure and measured RSS", lambda: group(run, "gateway", t09.rust))
        run.case("hls-regressions", ["R05", "R26", "R27", "R28", "R44", "R49", "REQ-T21"], "all 8 HLS tests, every URI/key/map/segment, body transforms, invalid templates and real AES/fMP4 decode", lambda: group(run, "hls", t10.rust))
        run.case("subtitle-regressions", ["R26", "R29", "R34", "R44", "REQ-T21"], "all 9 subtitle tests, structured VTT/SRT/ASS, auth, close/cancel, DRM and limits", lambda: group(run, "subtitles", t11.rust))
        run.case("conditional-dash-baseline", ["R29", "REQ-T21"], "actual supported-source format baseline rechecked", lambda: dash_baseline(run))
        run.case("packaged-player-and-session-trace", ["R12", "R26", "R28", "R34", "R37", "R44", "R48", "R49", "R50", "C-RESOURCE", "REQ-T21"], "official player 50 switches/shared H264AAC/HLS, image DTO refresh, lease recovery, native CSP/CORS/IPC negatives and all subprocess cleanup", lambda: webview(run))
    return run.save()


if __name__ == "__main__":
    awake = subprocess.Popen(["/usr/bin/caffeinate", "-di", "-w", str(os.getpid())]) if sys.platform == "darwin" else None
    try:
        code = main()
    finally:
        if awake is not None:
            awake.terminate()
            awake.wait(timeout=10)
    raise SystemExit(code)
