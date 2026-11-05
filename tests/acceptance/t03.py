"""T03: identity and protocol integrity of every supported JSON source, over public response fixtures.

Run only through the original resource executor. Real media stays NOT RUN here (T0.19 owns it).
"""
import argparse
import json
from pathlib import Path
import re

from t00 import read_json, require
from t10 import resources
import t02

ROOT = Path(__file__).resolve().parents[2]
IDS = ["U02", "U05", "R04", "R15", "R18", "REQ-T03"]
NATIVE = {
    "t03_every_source_identity_is_stable_across_restart_and_independent",
    "t03_endpoint_disable_delete_and_readd_invalidate_old_references",
    "t03_same_name_and_same_id_sources_stay_separate",
    "t03_repeated_groups_and_episodes_keep_order_and_stay_selectable",
    "t03_numeric_forms_empty_fields_and_long_titles_are_explicit",
    "t03_page_tail_and_partial_failures_never_become_silent_empty_lists",
}
REGRESSIONS = [
    "n00_archived_identity_and_protocol_vectors",
    "n00_config_edits_disable_delete_and_readd_cancel_io",
    "n00_group_order_url_fidelity_and_stale_fingerprints",
]
# t02.redact preserves only the runner prefixes of the tests it knows.
t02.NATIVE = t02.NATIVE | NATIVE | set(REGRESSIONS)


class Acceptance(t02.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.20", "T03", IDS


def native(run):
    argv = ["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib"]
    output = run.command(argv + ["t03_", "--", "--test-threads=1", "--nocapture"])
    names = {name.split("::")[-1] for name in re.findall(r"^test (\S+) \.\.\.", output, re.M)}
    require(names == NATIVE and f"test result: ok. {len(NATIVE)} passed; 0 failed; 0 ignored;" in output,
            "All named T03 native tests must execute with no failure or skip")
    fixture = [json.loads(line.split("T03_FIXTURE:", 1)[1]) for line in output.splitlines() if "T03_FIXTURE:" in line]
    supported = read_json(ROOT / "docs/compatibility-report.json")["supported_sources"]
    require(len(fixture) == 1 and sorted(fixture[0]["sources"]) == sorted(supported),
            "The fixtures must cover exactly the JSON sources promoted by T0.19")
    for name in REGRESSIONS:
        result = run.command(argv + [name, "--", "--test-threads=1", "--nocapture"])
        require(f"::{name} ... " in result and "test result: ok. 1 passed; 0 failed; 0 ignored;" in result,
                "Required identity/revision regression did not pass")
    run.observed["native_counts"] = dict(required=len(NATIVE) + len(REGRESSIONS), ran=len(NATIVE) + len(REGRESSIONS),
                                          passed=len(NATIVE) + len(REGRESSIONS), failed=0, skipped=0, status="PASS")
    return dict(sources=fixture[0]["sources"], regressions=REGRESSIONS, **run.observed["native_counts"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T03-all.json"
            and args.junit.resolve() == ROOT / "evidence/acceptance/T03-all.xml", "Exact T03 output paths required")
    run = Acceptance(args.report, args.junit)
    run.observed.update(online_media="NOT RUN; fixtures only (T0.19 owns the real media chain)",
                        platforms="macOS arm64 required; others NOT RUN (ADR-005/006)")
    ready = run.case("resource-ownership", ["REQ-T03"], "original stable-source receipt; 4096MiB RSS sampling; jobs=2/test_threads=1",
                     lambda: resources(run, card="T0.20"))
    run.case("identity-and-protocol-integrity", IDS, "per-source identity, revision, isolation, repeats, shapes, tail and partial failure",
             lambda: native(run) if ready else require(False, "NOT RUN: resource ownership failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
