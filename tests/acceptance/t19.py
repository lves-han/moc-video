"""T19: configuration and sync transactions for the supported JSON sources, through the production Host."""
import argparse
from pathlib import Path
import re

from t00 import read_json, require
from t10 import resources
import t02
import t14

ROOT = Path(__file__).resolve().parents[2]
IDS = ["R02", "R04", "R14", "R44", "REQ-T19"]
NATIVE = {
    "t19_unchanged_save_and_failed_sync_keep_the_confirmed_source_and_playback",
    "t19_editing_one_source_stops_only_its_playback_and_references",
    "t19_failed_persistence_keeps_configuration_registry_and_playback",
    "t19_invalid_configurations_are_refused_before_anything_changes",
}
LIB_REGRESSIONS = [
    "n00_latest_sync_wins_without_partial_publication",
    "n00_cancel_pending_sync_and_keep_control_responsive",
    "n00_config_edits_disable_delete_and_readd_cancel_io",
    "n01_sync_sequence_and_edit_disable_delete_readd_cancel",
    "t03_endpoint_disable_delete_and_readd_invalidate_old_references",
    "t09_capabilities_bearer_cors_csp_and_resource_isolation",
    "t09_image_decoders_byte_pixel_limits_and_mime",
]
T05_REGRESSIONS = ["t05_sync_sequence_rejects_late_publication", "t05_generation_tombstones_and_source_isolation",
                   "t05_settings_validation_and_failure_atomicity", "t05_configuration_limits_and_utf8_budgets"]
# Index download, refresh and package installation left the product (ADR-048); history stays as material.
RETIRED = re.compile(r"plugins\.json|repositories\.json|\.cs3\b|zip::|ZipArchive|unzip|install_plugin|download_plugin", re.I)


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.23", "T19", IDS
    DEPLOYMENT = "Rust unit-test Host and gateway over local fixtures; no packaged WebView, cluster or Android"

    def redact_output(self, output):
        return t02.redact(output)


def retired_paths():
    files = [path for folder in ("src-tauri/src", "src") for path in (ROOT / folder).rglob("*") if path.suffix in {".rs", ".ts", ".vue"}]
    hits = [str(path.relative_to(ROOT)) for path in files if RETIRED.search(path.read_text(encoding="utf-8"))]
    require(not hits, f"Retired plugin index/package paths remain in the product: {hits}")
    cargo = (ROOT / "src-tauri/Cargo.toml").read_text(encoding="utf-8")
    require(not re.search(r"^\s*zip\s*=", cargo, re.M), "A ZIP dependency remains")
    inventory = ROOT / "replica-spec/references/plugin-artifact-inventory.json"
    require(inventory.exists(), "Historical package inventory must stay as material")
    report = read_json(ROOT / "docs/compatibility-report.json")
    require(report["supported_sources"] and all(any(row["name"] == name and row["supported"] for row in report["sources"]) for name in report["supported_sources"]),
            "Supported sources must stay traceable to the compatibility report")
    return dict(scanned=len(files), retired_hits=0, supported=report["supported_sources"])


def native(run):
    argv = ["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml"]
    output = run.command(argv + ["--lib", "t19_", "--", "--test-threads=1", "--nocapture"], timeout=600)
    names = {name.split("::")[-1] for name in re.findall(r"^test (\S+) \.\.\.", output, re.M)}
    require(names == NATIVE and f"test result: ok. {len(NATIVE)} passed; 0 failed; 0 ignored;" in output, "All named T19 native tests must execute with no failure or skip")
    for name in LIB_REGRESSIONS:
        result = run.command(argv + ["--lib", name, "--", "--test-threads=1", "--nocapture"], timeout=600)
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in result, f"Required regression did not pass: {name}")
    for name in T05_REGRESSIONS:
        result = run.command(argv + ["--test", "t05", name, "--", "--test-threads=1", "--nocapture"], timeout=600)
        require("test result: ok. 1 passed; 0 failed; 0 ignored;" in result, f"Required regression did not pass: {name}")
    total = len(NATIVE) + len(LIB_REGRESSIONS) + len(T05_REGRESSIONS)
    run.observed["native_counts"] = dict(required=total, ran=total, passed=total, failed=0, skipped=0, status="PASS")
    return run.observed["native_counts"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T19-all.json" and args.junit.resolve() == ROOT / "evidence/acceptance/T19-all.xml", "Exact T19 output paths required")
    t02.NATIVE = t02.NATIVE | NATIVE | set(LIB_REGRESSIONS) | set(T05_REGRESSIONS)
    run = Acceptance(args.report, args.junit)
    run.observed.update(online_media="NOT RUN; fixtures only", platforms="macOS arm64 required; others NOT RUN (ADR-005/006)")
    if run.case("resource-ownership", ["REQ-T19"], "scope/source bound 4096MiB RSS sampled backend", lambda: resources(run, card="T0.23")):
        run.case("retired-package-paths", ["R02", "REQ-T19"], "no index download, refresh or ZIP install in the product; history and supported list traceable", retired_paths)
        run.case("configuration-transactions", IDS, "unchanged and failed sync keep playback; edits stop only their source; failed persistence and invalid input change nothing", lambda: native(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
