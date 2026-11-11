"""T0.38: settings reorganization and built-in TMDB (Stremio mode) on the real Host and current frontend."""
import argparse
import json
import re
from pathlib import Path

import t14
from t00 import require

ROOT = Path(__file__).resolve().parents[2]
IDS = ["REQ-X03", "REQ-X04", "REQ-X05", "REQ-X06"]
NATIVE = "x1_builtin_tmdb_language_instance_and_identity"


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.38", "X1", IDS
    DEPLOYMENT = "real Rust Host with a controlled TMDB-shaped addon; current frontend state tests; recorded browser check of the production Settings page"
    DEFERRED = [dict(target="packaged WKWebView settings page", status="NOT RUN", reason="ADR-062 necessary checks; UI observed in browser over a fake host")]


def native(run):
    output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib",
                          "--", "--test-threads=1", "--nocapture", NATIVE, "settings::"], timeout=1800)
    require(re.search(r"test result: ok\. ([4-9]|\d\d+) passed; 0 failed", output), "Host TMDB test and settings tests must pass")
    rows = [json.loads(line.split("X1_TMDB:", 1)[1]) for line in output.splitlines() if line.startswith("X1_TMDB:")]
    require(len(rows) == 1, "one Host observation expected")
    row = rows[0]
    require(row["follow"] == "盗梦空间（合成）" and row["chosen_en"] == "Synthetic same title" and row["chosen_zh"] == "盗梦空间（合成）", "language choice differs")
    run.observed["host"] = row
    return row


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/x1.test.ts"], timeout=180)
    require(re.search(r"^# pass 3$", output, re.M) and re.search(r"^# fail 0$", output, re.M), "three frontend X1 tests must pass")
    return dict(tests=3)


def contracts(run):
    run.command(["{python}", "-B", "scripts/check-contracts.py"], timeout=300)
    run.command(["npx", "vue-tsc", "--noEmit"], timeout=600)
    return "contracts and type check pass"


def structure():
    app = (ROOT / "src/App.vue").read_text()
    require('"#/sources"' not in app and "#/sources" not in app and "NativeSourcesView" not in app and "SourcesView" not in app, "old source manager entry remains")
    require(not (ROOT / "src/views/NativeSourcesView.vue").exists() and not (ROOT / "src/views/SourcesView.vue").exists(), "old source manager pages remain")
    settings = (ROOT / "src/views/SettingsView.vue").read_text()
    require('section="metadata"' in settings and 'section="playback"' in settings, "settings groups must host the source settings")
    defaults = json.loads((ROOT / "src/data/defaultSources.json").read_text())
    require(all(row["kind"] == "maccms" for row in defaults) and len(defaults) == 8, "bundled defaults must hold only playback presets")
    ui = json.loads((ROOT / "evidence/manual/T0.38-settings-ui.json").read_text())["observed"]
    require(ui["sources_link_present"] is False and ui["page_errors"] == 0 and ui["add_preset"]["unadded_presets"] == 5, "recorded browser check differs")
    return dict(ui=ui["playback_section_rows_initial"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    host = {}
    def host_case():
        host.update(native(run)); return host
    run.case("stremio-mode", ["REQ-X06", "REQ-X05"], "real Host syncs the fixed TMDB source from a self-hosted instance; disabled state; identity clash and bad URL rejected", host_case)
    run.case("tmdb-language", ["REQ-X05"], "follow uses app language; chosen language ignores app language; frontend metadata locale", lambda: [require(host, "Host case must pass first"), frontend(run)])
    run.case("history-stable", ["REQ-X05"], "language changes keep source id, revision, refs and metadata_ref (asserted in the Host test)", lambda: require(host.get("source_id"), "Host identity observation missing") or host["source_id"])
    run.case("merged-list", ["REQ-X04"], "presets and added sources form one list without duplicates; add/edit/disable/delete", lambda: [frontend(run), structure()])
    run.case("playback-settings", ["REQ-X03"], "source manager lives in Settings; no #/sources entry; contracts and types pass", lambda: [structure(), contracts(run)])
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
