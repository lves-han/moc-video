"""T0.42: local video library (mp4/mov/m4v) on the real Host/gateway and the current frontend."""
import argparse
import json
from pathlib import Path

import t14
from t00 import require

ROOT = Path(__file__).resolve().parents[2]
IDS = ["REQ-X01"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.42", "X5", IDS
    DEPLOYMENT = "real Rust Host and loopback gateway over controlled local files; recorded browser check of the production Local video page"
    DEFERRED = [dict(target="native macOS open panel and packaged WKWebView decoding", status="NOT RUN", reason="ADR-062 necessary checks")]


def native(run):
    output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib",
                          "--", "--test-threads=1", "--nocapture", "host::local::tests", "x5_local_library_range_scope_history_and_restart"], timeout=1800)
    require("test result: ok. 2 passed; 0 failed" in output, "local scan unit test and Host local test must pass")
    rows = [json.loads(line.split("X5_LOCAL:", 1)[1]) for line in output.splitlines() if line.startswith("X5_LOCAL:")]
    require(rows == [dict(cache_used=0, items=3, range="206", resume_ms=42000)], "Host local observation differs")
    run.observed["host"] = rows[0]
    return rows[0]


def manual():
    return json.loads((ROOT / "evidence/manual/T0.42-local-ui.json").read_text())["observed"]


def checks(run):
    run.command(["{python}", "-B", "scripts/check-contracts.py"], timeout=300)
    run.command(["npx", "vue-tsc", "--noEmit"], timeout=600)
    caps = json.loads((ROOT / "src-tauri/capabilities/main.json").read_text())["permissions"]
    require({"allow-list-local-videos", "allow-open-local", "dialog:allow-open"} <= set(caps), "local commands and the open dialog need permissions")
    require('tauri-plugin-dialog = "=2.8.4"' in (ROOT / "src-tauri/Cargo.toml").read_text(), "dialog plugin must be pinned")
    return "contracts, types, permissions"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    host = {}
    def host_case():
        host.update(native(run)); return host
    run.case("play-and-seek", ["REQ-X01"], "full body, byte ranges and suffix ranges from disk; 416 past the end; no media cache; UI plays with seekable controls",
             lambda: [host_case(), require(manual()["playback"]["native_controls_with_seek"], "UI playback differs"), checks(run)])
    run.case("persist", ["REQ-X01"], "library paths and resume position survive a Host restart; UI resumes from history and from the Library page",
             lambda: require(host.get("resume_ms") == 42000 and manual()["replay_resumes_from_saved_position"]["current_time_after_1_5s_s"] > 2 and manual()["library_continue_button"]["resumed_playing"], "resume differs") or "resumed")
    run.case("missing-file", ["REQ-X01"], "a moved file reports SOURCE_NOT_FOUND; removing a folder stops its session (asserted in the Host test and UI)",
             lambda: require(host and manual()["remove_folder"]["session_closed"], "missing/removed handling differs") or "explicit")
    run.case("path-scope", ["REQ-X01"], "only files inside library roots open; symlink escape and outside paths refused; relative roots rejected",
             lambda: require(host, "Host case must pass") or "scoped")
    run.case("unsupported-format", ["REQ-X01"], "mkv is listed, shown as unsupported and refused with MEDIA_UNSUPPORTED",
             lambda: require(host and manual()["items"][1]["disabled"], "unsupported handling differs") or "refused")
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
