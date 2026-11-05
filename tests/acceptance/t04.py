"""T04: Rust source request context, fault isolation and cleanup without any Android tool.

Run only through the original resource executor. Fixtures only; real media stays with T0.19.
"""
import argparse
import os
from pathlib import Path
import re
import socket
import subprocess
import sys

from t00 import read_json, require
from t10 import resources
import t02

ROOT = Path(__file__).resolve().parents[2]
IDS = ["U04", "R05", "R09", "R10", "R11", "R12", "R13", "R36", "R45", "R63", "REQ-T04"]
NATIVE = {
    "t04_host_and_playback_work_without_android_tools",
    "t04_one_failing_source_keeps_other_sources_and_stremio_working",
    "t04_access_refusal_is_reported_and_stops_the_source",
    "t04_refused_media_fails_once_and_never_reaches_a_local_secret_upstream",
    "t04_certificate_errors_stay_failures_and_are_named",
    "t04_stale_work_and_exit_release_only_this_host",
}
REGRESSIONS = [
    "n00_redirect_origin_headers_proxy_and_limits",
    "n00_deadline_queue_and_live_body_timeout",
    "n00_config_edits_disable_delete_and_readd_cancel_io",
    "t09_native_headers_cookie_referer_interceptor_and_no_local_secrets",
    "t09_capabilities_bearer_cors_csp_and_resource_isolation",
    "t09_host_inflight_media_revoke_and_control_responsive",
    "t09_host_inflight_resolve_cancel_and_late_result_rejected",
]
ANDROID_TOOLS = ("adb", "emulator", "avdmanager", "sdkmanager")
t02.NATIVE = t02.NATIVE | NATIVE | set(REGRESSIONS)


class Acceptance(t02.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.21", "T04", IDS
    DEPLOYMENT = "Rust unit-test Host and gateway over local fixtures, Android tools removed from the environment; no packaged WebView, cluster or Android"


def without_android():
    """Removes every Android tool from this run; the native test asserts the same from inside."""
    for name in [name for name in os.environ if name.startswith("ANDROID")]:
        del os.environ[name]
    folders = os.environ["PATH"].split(os.pathsep)
    kept = [folder for folder in folders if not any((Path(folder) / tool).exists() for tool in ANDROID_TOOLS)]
    os.environ["PATH"] = os.pathsep.join(kept)
    os.environ["T04_NO_ANDROID"] = "1"
    return dict(removed_path_entries=len(folders) - len(kept))


def static_scope():
    sources = list((ROOT / "src-tauri/src").glob("*.rs"))
    spawning = [path.name for path in sources if "Command::new" in path.read_text(encoding="utf-8")]
    require(not spawning, f"Production Rust starts a process: {spawning}")
    config = read_json(ROOT / "src-tauri/tauri.conf.json")
    require(config["bundle"].get("resources") == [], "Bundle resources must exclude historical Android inputs")
    return dict(spawning_sources=spawning, bundle_resources=[])


def native(run):
    argv = ["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib"]
    output = run.command(argv + ["t04_", "--", "--test-threads=1", "--nocapture"])
    names = {name.split("::")[-1] for name in re.findall(r"^test (\S+) \.\.\.", output, re.M)}
    require(names == NATIVE and f"test result: ok. {len(NATIVE)} passed; 0 failed; 0 ignored;" in output,
            "All named T04 native tests must execute with no failure or skip")
    require("T04_NO_ANDROID:" in output, "The native test did not check the Android-free environment")
    for name in REGRESSIONS:
        result = run.command(argv + [name, "--", "--test-threads=1", "--nocapture"])
        require(f"::{name} ... " in result and "test result: ok. 1 passed; 0 failed; 0 ignored;" in result,
                "Required context/isolation regression did not pass")
    run.observed["native_counts"] = dict(required=len(NATIVE) + len(REGRESSIONS), ran=len(NATIVE) + len(REGRESSIONS),
                                          passed=len(NATIVE) + len(REGRESSIONS), failed=0, skipped=0, status="PASS")
    return dict(regressions=REGRESSIONS, **run.observed["native_counts"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T04-all.json"
            and args.junit.resolve() == ROOT / "evidence/acceptance/T04-all.xml", "Exact T04 output paths required")
    run = Acceptance(args.report, args.junit)
    run.observed.update(online_media="NOT RUN; fixtures only (T0.19 owns the real media chain)",
                        platforms="macOS arm64 required; others NOT RUN (ADR-005/006)")
    ready = run.case("resource-ownership", ["REQ-T04"], "original stable-source receipt; 4096MiB RSS sampling; jobs=2/test_threads=1",
                     lambda: resources(run, card="T0.21"))
    run.observed["android_free"] = without_android()
    run.case("no-process-or-android-input", ["U04", "R45", "R09"], "production Rust starts no process and bundles no Android input", static_scope)
    # External resources this run does not own must outlive the whole native suite.
    outsider = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(3600)"])
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen()
    try:
        def checked():
            require(ready, "NOT RUN: resource ownership failed")
            value = native(run)
            require(outsider.poll() is None, "An external process was stopped")
            with socket.create_connection(listener.getsockname(), timeout=2):
                pass
            return dict(external_process_alive=True, external_listener_alive=True, **value)
        run.case("context-isolation-and-cleanup", IDS, "named Host fixtures for isolation, access control, TLS, capabilities, stale work and exit", checked)
    finally:
        listener.close()
        outsider.kill()
        outsider.wait()
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
