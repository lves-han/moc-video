"""T0.45: approximate MacCMS search on the real Host and the search page hint."""
import argparse
import json
import re
from pathlib import Path

import t14
from t00 import require

ROOT = Path(__file__).resolve().parents[2]
IDS = ["REQ-X08"]


class Acceptance(t14.Acceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.45", "X8", IDS
    DEPLOYMENT = "real Rust Host with a substring-matching MacCMS fixture and a TMDB-shaped addon fixture; current frontend"
    DEFERRED = [dict(target="typo correction and Traditional/Simplified folding", status="NOT RUN", reason="out of scope by user-approved plan")]


def native(run):
    output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml", "--lib", "--",
                          "--test-threads=1", "--nocapture", "fuzzy::", "x8_approximate_maccms_search_relaxes_ranks_and_uses_tmdb_titles"], timeout=1800)
    require("test result: ok. 2 passed; 0 failed" in output, "fuzzy unit test and Host search test must pass")
    row = [json.loads(line.split(":", 1)[1]) for line in output.splitlines() if line.startswith("X8_SEARCH:")]
    require(row == [{"alias": "Inception→盗梦空间", "relaxed": ["复仇者联盟 4", "庆余年 第二季"]}], "Host observation differs")
    return row[0]


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/x8.test.ts"], timeout=180)
    require(re.search(r"^# pass 1$", output, re.M) and re.search(r"^# fail 0$", output, re.M), "frontend hint test must pass")
    run.command(["npx", "vue-tsc", "--noEmit"], timeout=600)
    ui = json.loads((ROOT / "evidence/manual/T0.45-search-ui.json").read_text())["observed"]
    require(ui["query_庆余年_第二季"]["hint"] and ui["query_庆余年"]["hint"] is None, "recorded hint check differs")
    return ui


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    host = {}
    run.case("relaxed-queries", ["REQ-X08"], "spaces/punctuation, season markers and shorter prefixes are tried only after the typed query finds nothing",
             lambda: host.update(native(run)) or host)
    run.case("ranking", ["REQ-X08"], "results sort by closeness to the typed title (asserted in unit and Host tests)", lambda: require(host, "Host case must pass") or "ranked")
    run.case("tmdb-alias", ["REQ-X08"], "an English or alias title finds the Chinese title through the built-in TMDB source", lambda: require(host.get("alias"), "alias missing") or host["alias"])
    run.case("approximate-hint", ["REQ-X08"], "the search page says when no result contains the typed text", lambda: frontend(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
