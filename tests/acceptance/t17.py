"""T0.17 source manager acceptance: production App/NativeSourcesView against a real Host; Cloudstream states are synthetic until N02."""
import argparse
import re
from pathlib import Path

from t00 import require
from t10 import resources
import t14

ROOT = Path(__file__).resolve().parents[2]
IDS = ["R01", "R13", "R24", "R58", "REQ-T17"]
LIVE_TEST = "host::maccms::tests::t08::t09::t17::t17_host_sources_webview_bridge"


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.17", "T17", IDS


def validate_sources(e):
    f = e["first_run"]
    require(f["link"] == "#/setup" and f["opened"]["route"] == "sources" and f["opened"]["empty"] is True and f["opened"]["guidance"] == 2, "First run must lead to the source manager with guidance")
    require(e["invalid"]["saves"] == 0 and len(e["invalid"]["messages"]) == 4 and "hidden" not in str(e["invalid"]["messages"]), "Invalid drafts must be rejected before saving, without echoing credentials")
    a = e["added"]
    require(a["kinds"] == ["stremio", "maccms"] and "ready" in a["state"] and "no failure" in a["last"] and a["shown_secret"] is False, "Added sources must show registration and last request separately, without URL secrets")
    require(e["home"]["cards"] == 2 and e["home"]["setup_gone"] is True, "The real home must use the added source")
    d = e["edit"]
    require(d["kind_locked"] is True and d["same_id"] is True and d["count"] == 2 and d["provider"] is True, "Editing must keep identity and republish the source")
    i = e["independent"]
    require(i["metadata_kept"] is True and i["home_kept"] is True and i["persisted"] is True, "Playback selection must not disturb metadata")
    o = e["disabled"]
    require("disabled" in o["state"] and o["fallback"] is True and o["option"] is False and e["enabled"] is True, "Disable must withdraw the source and fall back; enable must restore it")
    require(e["deleted"]["kept_after_cancel"] is True and e["deleted"]["configs"] == 2, "Delete must confirm first")
    g = e["failing"]
    require(g["message"] and g["others_ready"] is True and g["selection_kept"] is True, "A failing source must report its error without affecting others")
    p = e["real_prepare"]
    require(p["message"] and p["host_phase"] == "ready" and p["others_ready"] is True, "A real prepare refusal must be visible and leave HTTP sources working")
    c = e["cloudstream"]
    require(c["stimulus"] == "synthetic" and c["target"] == "cloudstream" and "SuperVod" in c["plugin"] and "ready" in c["guest"] and c["host_phase"] == "ready" and c["selectable"] is True, "Cloudstream packages and guest state must display apart from the host")
    n = e["interaction"]
    require(n["stimulus"] == "synthetic" and n["after_reject"] == 0 and n["rejected_shown"] is True and "Synthetic window" in n["started"] and n["request"].get("source_revision") == "rev"
            and n["cancelled"] == "cancelled" and n["completed"] == "completed", "Interaction must confirm first, report rejection, and cancel or complete explicitly")
    r = e["diagnostics"]
    require(all(r[name] is True for name in ("copied_equals_report", "platform", "contract", "engine", "package", "full_digest_absent", "correlation", "states_separate")), "Diagnostics must carry versions, digests, platform, correlation and states: " + str(r))
    require(r["secret"] is False and r["query"] is False, "Diagnostics must be redacted")
    require(e["errors"] == [], "Source manager raised page errors: " + str(e["errors"]))


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/sources.test.ts", "tests/state.test.ts", "tests/api.test.ts"], timeout=180)
    count, fail = re.search(r"# pass (\d+)", output), re.search(r"# fail (\d+)", output)
    require("REQ-T17" in output and count and int(count[1]) > 0 and fail and int(fail[1]) == 0, "Frontend tests must run nonzero with no failures")
    return dict(tests=int(count[1]))


def webview(run):
    return t14.packaged(run, LIVE_TEST, "t17", validate_sources, "sources", lambda e: dict(interaction="synthetic; real interaction accepted in N02"))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T17-all.json" and args.junit.resolve() == ROOT / "evidence/acceptance/T17-all.xml", "Exact T17 output paths required")
    run = Acceptance(args.report, args.junit)
    if run.case("resource-ownership", ["REQ-T17"], "scope/source bound 4096MiB RSS sampled backend", lambda: resources(run, card="T0.17")):
        def oracles():
            output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t17.py"])
            count = re.search(r"Ran (\d+) tests?", output)
            require(count and int(count[1]) >= 4, "Independent T17 oracle negatives must run")
            return dict(tests=int(count[1]))
        run.case("independent-negative-oracles", ["REQ-T17"], "corrupted source manager evidence is rejected", oracles)
        run.case("frontend-behaviors", ["R01", "R24", "R58", "REQ-T17"], "nonzero validation, redaction and state tests", lambda: frontend(run))
        run.case("packaged-sources", IDS, "real HTTP config add/edit/disable/delete, independent selection, real home; synthetic Cloudstream state and interaction; redacted diagnostics", lambda: webview(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
