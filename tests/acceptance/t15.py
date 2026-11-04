"""T0.15 details acceptance: production App/DetailsView against a real Host in a packaged WKWebView."""
import argparse
import re
from pathlib import Path

from t00 import require
from t10 import resources
import t14

ROOT = Path(__file__).resolve().parents[2]
IDS = ["U03", "R22", "REQ-T15"]
LIVE_TEST = "host::maccms::tests::t08::t09::t15::t15_host_details_webview_bridge"


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.15", "T15", IDS


def validate_details(e):
    m = e["movie"]
    require(m["kind"] == "movie" and m["play"] is True and m["episode_grid"] is False and m["poster"] is True and "2021" in m["meta"],
            "A movie must offer Play with its title metadata and no episode grid")
    require(m["plain_text"] is True and m["collapsed_chars"] < 300 < m["full_chars"], "Long plots must be plain text, collapsed by default and expandable")
    s = e["series"]
    require(s["kind"] == "series" and s["buttons"] == 200 and s["unique"] == 200 and s["videos"] == 200 and s["duplicate_titles"] is True,
            "200 identically named episodes must stay individually selectable")
    require(len(e["layouts"]) == 2 and not any(row["overflow"] for row in e["layouts"]), "Details must not overflow at 960 and 390")
    p = e["sparse"]
    require(p["groups"] >= 2 and p["poster_placeholder"] is True and p["plot"] is False and "2" not in (p["meta"] or ""), "Missing poster/year/plot must degrade naturally")
    require(all(row["pressed"] is True and row["others"] == 1 and row["shown"] == row["expected"] and row["shown"] for row in p["switched"]),
            "Changing group must show exactly that group and mark it alone")
    q = e["empty"]
    require(q["message"] is True and q["play"] is False and q["buttons"] == 0, "A title without episodes must say so without play controls")
    t = e["stale"]
    require(t["title"] == "Sparse series" and t["data_title"] == "Sparse series", "A late details response must not overwrite the newer page")
    r = e["related"]
    require(r["reopened_plot_chars"] < 300, "A newly opened title must start with its plot collapsed")
    require(r["cards"] == 1 and "Related title" in r["text"] and r["opened"] == "Sparse series", "Related content must render and open")
    f = e["flow"]
    require(f["sent_video"] == f["video"] and f["first_selection"] is None and f["pending"]["loading"] is True and f["pending"]["disabled"] is True,
            "The chosen metadata video must be the one matched, with an accurate pending state")
    require(f["cancelled"]["route"] == "details" and f["cancelled"]["title"] and f["cancelled"]["context"] is None, "Cancelling confirmation must stay on details")
    x, sel = f["expected"], f["selection"]
    require(sel == dict(content_ref=x["content"], group_id=x["group"], episode_id=x["id"]) and f["confirmed_video"] == f["video"], "Confirmed selection differs from the clicked candidate episode")
    require(f["context_episode_ref"] == x["ref"] and f["context_group"] == x["group"] and f["context_episode"] == x["id"] and f["context_video"] == f["video"] and f["hash"] == "#/playback",
            "Playback context must be the clicked episode's real reference")
    require(e["errors"] == [], "Details raised page errors: " + str(e["errors"]))


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/details.test.ts", "tests/home.test.ts", "tests/state.test.ts"], timeout=180)
    count, fail = re.search(r"# pass (\d+)", output), re.search(r"# fail (\d+)", output)
    require("REQ-T15" in output and count and int(count[1]) > 0 and fail and int(fail[1]) == 0, "Frontend tests must run nonzero with no failures")
    return dict(tests=int(count[1]))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    require(args.report.resolve() == ROOT / "evidence/acceptance/T15-all.json" and args.junit.resolve() == ROOT / "evidence/acceptance/T15-all.xml", "Exact T15 output paths required")
    run = Acceptance(args.report, args.junit)
    if run.case("resource-ownership", ["REQ-T15"], "scope/source bound 4096MiB RSS sampled backend", lambda: resources(run, card="T0.15")):
        def oracles():
            output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_t15.py"])
            count = re.search(r"Ran (\d+) tests?", output)
            require(count and int(count[1]) >= 4, "Independent T15 oracle negatives must run")
            return dict(tests=int(count[1]))
        run.case("independent-negative-oracles", ["REQ-T15"], "corrupted details evidence is rejected", oracles)
        run.case("frontend-behaviors", ["R22", "REQ-T15"], "nonzero state and image recovery tests", lambda: frontend(run))
        run.case("packaged-details", IDS, "real Stremio details via invoke: kinds, 200 episodes, groups, sparse fields, stale response, confirmation to the real episode reference",
                 lambda: t14.packaged(run, LIVE_TEST, "t15", validate_details, "details", lambda e: dict(episodes=e["series"]["buttons"])))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
