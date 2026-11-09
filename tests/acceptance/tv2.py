"""TV2: production Host/HTTP behavior, live built-in TMDB samples, DTO oracles."""
import argparse
import hashlib
import importlib.util
import json
import os
import re
import shutil
import sys
import tempfile
import tomllib
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

from t00 import Acceptance as CommandRunner, require, read_json, write_json
from t05_models import generate
from t10 import resources
from t14 import Acceptance as BaseAcceptance

ROOT = Path(__file__).resolve().parents[2]
BEHAVIORS = {
    "TV-CATALOG-DTO": ["card-fields", "section-kind-facets", "style-fields"],
    "TV-FILTER": ["single-condition", "priority-routing", "scan-limit-progress", "cursor-isolation",
                  "placeholder-empty", "localized-values", "sort-routing", "facet-availability"],
    "TV-SOURCE-UPGRADE": ["adds-once", "respects-removal"],
}
REQUIREMENTS = list(BEHAVIORS) + [f"{parent}/{name}" for parent, names in BEHAVIORS.items() for name in names] + ["REQ-TV2"]
RUST_TESTS = {
    "tv2_card_fields_and_missing_values": ["TV-CATALOG-DTO/card-fields", "TV-CATALOG-DTO/style-fields"],
    "tv2_upgrade_adds_once_and_respects_removal": ["TV-SOURCE-UPGRADE/adds-once", "TV-SOURCE-UPGRADE/respects-removal"],
    "tv2_single_conditions_and_priority_routing": ["TV-FILTER/single-condition", "TV-FILTER/priority-routing"],
    "tv2_scan_limit_progress_and_cursor_isolation": ["TV-FILTER/scan-limit-progress", "TV-FILTER/cursor-isolation"],
    "tv2_localized_values_facets_and_locale_cursors": ["TV-FILTER/localized-values", "TV-CATALOG-DTO/section-kind-facets"],
    "tv2_sort_routing_and_disabled_dimensions": ["TV-FILTER/sort-routing", "TV-FILTER/facet-availability"],
    "tv2_placeholder_empty_and_recovers": ["TV-FILTER/placeholder-empty"],
    "tv2_live_builtin_tmdb_request_samples": ["TV-FILTER/single-condition", "TV-FILTER/localized-values", "TV-FILTER/sort-routing"],
}


class Acceptance(BaseAcceptance):
    CARD, ORIGINAL, REQUIREMENTS = "T0.29", "TV2", REQUIREMENTS
    DEPLOYMENT = "production Rust Host and loopback HTTP fixtures; live public TMDB through production Http; no cluster"
    DEFERRED = [dict(target=p, status="NOT RUN", reason="ADR-005/006") for p in
                ["macos-x86_64", "windows-x86_64", "linux-x11", "linux-wayland"]]
    command = CommandRunner.command


def requirements(children):
    return sorted(set(children + [child.split("/")[0] for child in children] + ["REQ-TV2"]))


def versions(run):
    observed = {name: run.command([name, "--version"]).strip() for name in ["rustc", "cargo", "node"]}
    package = read_json(ROOT / "package.json")
    installed = {name: read_json(ROOT / "node_modules" / name / "package.json")["version"]
                 for name in ["vue", "typescript"]}
    require(installed["vue"] == package["dependencies"]["vue"]
            and installed["typescript"] == package["devDependencies"]["typescript"], "installed dependency versions differ")
    lock = (ROOT / "src-tauri/Cargo.lock").read_bytes()
    run.observed["versions"] = dict(tools=observed, python=sys.version, frontend=installed,
        native=[dict(name=p["name"], version=p["version"]) for p in tomllib.loads(lock.decode())["package"]
                if p["name"] in ["serde", "serde_json", "reqwest", "jsonschema"]],
        cargo_lock_sha256=hashlib.sha256(lock).hexdigest(),
        webview="NOT RUN: catalog/settings gate; rendered page acceptance belongs to TV3")
    return run.observed["versions"]


def check_home(row):
    """Independent assertions on serialized Host output, also exercised with mutations."""
    request, data = row["args"]["input"], row["response"]["data"]
    selected = request.get("filter") or {}
    for section in data["page"]["sections"]:
        require(section["kind"] in ["movie", "series", "anime", "other"], "section kind missing")
        require(set(section["filters"]) == {"genre", "year", "language", "region", "sort"}, "facet dimensions differ")
        require(section["filters"]["region"] == dict(available=False, options=[]), "TMDB region must be disabled")
        if "language" in selected or "year" in selected:
            require(section["filters"]["sort"] == dict(available=False, options=[]), "sort must be disabled with language/year")
        for facet in section["filters"].values():
            require(facet["available"] or not facet["options"], "disabled facet contains options")
            require(all(option.get("name", "").strip() and option.get("value", "").strip() for option in facet["options"]), "facet display names missing")
        items = section["items"]
        require(len(items) <= section["matched_count"] <= section["scanned_count"] <= request["page"] * 100, "scan/match counts violate bounded progress")
        for card in items:
            require(all(field in card for field in ["backdrop", "plot", "genres", "rating", "year"]), "card style field missing")
            require(card["plot"] is None or bool(card["plot"].strip()), "empty plot must be null")
            require(card["genres"] is None or bool(card["genres"]) and all(g.strip() for g in card["genres"]), "empty genres must be null")
            if selected.get("year"):
                require(str(card["year"]) == selected["year"], "year filter mismatch")
            if selected.get("genre"):
                aliases = {"Drama": {"Drama", "剧情"}, "剧情": {"Drama", "剧情"},
                           "Action & Adventure": {"Action & Adventure", "动作冒险"}}
                require(bool(set(card["genres"] or []) & aliases.get(selected["genre"], {selected["genre"]})), "genre filter mismatch")
            for image in [card.get("poster"), card["backdrop"]]:
                if image:
                    require(image["resource_id"] in data["images"], "image access missing")
    return dict(sections=len(data["page"]["sections"]))


def protocol(run):
    module = importlib.util.spec_from_file_location("tv2_contracts", ROOT / "scripts/check-contracts.py")
    checker = importlib.util.module_from_spec(module)
    module.loader.exec_module(checker)
    checked = checker.check(ROOT / "protocol")
    require(checked["commands"] == 23, "existing command set changed")
    require((ROOT / "src/models.ts").read_text() == generate(), "generated DTO types drifted")
    mutations = [
        ("bridge", lambda d: d["components"]["schemas"]["MediaCard"]["properties"]["title"].update(type="integer")),
        ("bridge", lambda d: d["components"]["schemas"]["MediaCard"]["properties"].pop("poster")),
        ("bridge", lambda d: d["components"]["schemas"]["MediaCard"]["properties"].update(extra={"type": "string"})),
        ("bridge", lambda d: d["components"]["schemas"]["MediaCard"]["required"].append("plot")),
        ("bridge", lambda d: d["components"]["schemas"]["HomeSection"]["properties"]["scanned_count"].update(minimum=-1)),
        ("bridge", lambda d: d["components"]["schemas"]["HomeRequest"]["properties"]["filter"]["anyOf"][0].update(additionalProperties=True)),
        ("app", lambda d: d["$defs"]["Settings"]["properties"].update(unapproved={"type": "boolean"})),
        ("app", lambda d: d["$defs"]["Settings"]["required"].append("metadata_preset_applied")),
    ]
    with tempfile.TemporaryDirectory(prefix="tv2-contracts-") as folder:
        for index, (owner, mutate) in enumerate(mutations):
            target = Path(folder) / str(index)
            shutil.copytree(ROOT / "protocol", target)
            path = target / ("bridge.openapi.json" if owner == "bridge" else "app.schema.json")
            value = read_json(path); mutate(value); write_json(path, value)
            try:
                checker.check(target)
            except AssertionError:
                pass
            else:
                raise AssertionError(f"contract mutation accepted: {index}")
    return dict(**checked, rejected_mutations=len(mutations))


def live_samples(folder):
    rows = read_json(folder / "requests.json")
    require(len(rows) == 9, "all actual built-in request samples required")
    total, routes = 0, []
    for row in rows:
        body = (folder / row["body"]).read_bytes()
        require(hashlib.sha256(body).hexdigest() == row["sha256"] and len(body) == row["bytes"], "live body digest differs")
        parsed = urlsplit(row["requested_url"])
        require(parsed.scheme == "https" and parsed.netloc == "tmdb.elfhosted.com" and urlsplit(row["final_url"]).netloc == parsed.netloc, "actual built-in origin differs")
        parts = parsed.path.split("/")
        extras = parse_qs(parts[-1].removesuffix(".json"))
        require(set(extras) == {"genre", "skip"} and extras["skip"][0] in ["0", "20"], "genre/skip not transmitted")
        metas = json.loads(body)["metas"]
        require(len(metas) == row["count"] > 0, "actual nonzero sample required")
        genre = extras["genre"][0]
        if parts[-2] in ["tmdb.top", "tmdb.latest"]:
            require(all(genre in meta.get("genres", meta.get("genre", [])) for meta in metas), "actual genre filter mismatch")
        if parts[-2] == "tmdb.year":
            require(all(str(meta.get("releaseInfo", meta.get("year", "")))[:4] == genre for meta in metas), "actual year filter mismatch")
        total += len(metas); routes.append(row["requested_url"])
    return dict(requests=len(rows), items=total, routes=routes, evidence=str(folder.relative_to(ROOT)))


def native(run):
    folder = run.folder / "live-tmdb"
    previous = os.environ.get("TV2_LIVE_EVIDENCE")
    os.environ["TV2_LIVE_EVIDENCE"] = str(folder)
    try:
        output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
                              "--lib", "tv2_", "--", "--test-threads=1", "--nocapture"], timeout=2400)
    finally:
        if previous is None: os.environ.pop("TV2_LIVE_EVIDENCE", None)
        else: os.environ["TV2_LIVE_EVIDENCE"] = previous
    names = set(re.findall(r"^test (?:\w+::)*(tv2_\w+) \.\.\.", output, re.M))
    require(names == set(RUST_TESTS) and f"test result: ok. {len(names)} passed; 0 failed; 0 ignored;" in output, "complete named nonzero native inventory must pass")
    observations = [json.loads(line.split("N01_OBSERVATION:", 1)[1]) for line in output.splitlines() if "N01_OBSERVATION:" in line]
    from jsonschema import Draft202012Validator, FormatChecker
    from referencing import Registry, Resource
    app, bridge = read_json(ROOT / "protocol/app.schema.json"), read_json(ROOT / "protocol/bridge.openapi.json")
    uri = "https://replica.invalid/contracts/bridge.openapi.json"
    registry = Registry().with_resource(app["$id"], Resource.from_contents(app)).with_resource(uri, Resource.from_contents({**bridge, "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": uri}))
    homes = []
    for row in observations:
        Draft202012Validator({"$ref": f'{app["$id"]}#/$defs/Result_{row["command"]}'}, registry=registry, format_checker=FormatChecker()).validate(row["response"])
        if row["command"] == "get_home" and "data" in row["response"]:
            check_home(row); homes.append(row)
    require(len(homes) >= 30, "real Host home observation coverage missing")
    write_json(run.folder / "host-observations.json", observations)
    run.observed["native"] = dict(tests=len(names), passed=len(names), failed=0, home_observations=len(homes),
        cleanup="owned fixture Drop joins workers; original resource runner owns descendant cleanup")
    run.observed["live"] = live_samples(folder)
    for name, children in RUST_TESTS.items():
        run.case(name, requirements(children), "production Host/HTTP assertions passed", lambda name=name: dict(test=name, log=run.commands[-1]["log"]))
    return run.observed["native"]


def frontend(run):
    output = run.command(["node", "--experimental-strip-types", "--test", "--test-concurrency=1", "--test-reporter=tap", "tests/tv2.test.ts"])
    require(re.search(r"# pass 3\b", output) and "# fail 0" in output, "three frontend behavior tests must pass")
    return dict(tests=3)


def budget_regression(run):
    output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
        "--lib", "t06_stremio_column_localization_shares_30s_deadline", "--", "--test-threads=1"], timeout=180)
    require("test result: ok. 1 passed; 0 failed; 0 ignored;" in output, "original shared 30s localization budget regression must pass")
    return dict(tests=1)


def main():
    if not __debug__: raise SystemExit("Assertions required")
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    args = parser.parse_args()
    run = Acceptance(args.report, args.junit)
    owned = run.case("resources", ["REQ-TV2"], "original resource receipt and macOS arm64", lambda: resources(run, card="T0.29"))
    if owned:
        run.case("versions", ["REQ-TV2"], "actual tool and installed dependency versions recorded", lambda: versions(run))
        def negatives():
            output = run.command(["{python}", "-B", "-m", "unittest", "discover", "-s", "tests/acceptance", "-p", "test_tv2.py"])
            require("Ran 7 tests" in output and "OK" in output, "all oracle mutation probes must run")
            return dict(tests=7)
        run.case("oracle-negatives", ["REQ-TV2"], "wrong filters/counts/fields/names/availability rejected", negatives)
        run.case("contracts", ["TV-CATALOG-DTO", "REQ-TV2"], "archived fields equal, exact optional allowlist, eight drift rejections", lambda: protocol(run))
        run.case("frontend", ["TV-FILTER/cursor-isolation", "TV-FILTER", "TV-CATALOG-DTO", "REQ-TV2"], "filter transmission/paging/race and backdrop image retention", lambda: frontend(run))
        run.case("native", REQUIREMENTS, "all eight native behaviors, independently validated Host output and nine live TMDB samples", lambda: native(run))
        run.case("shared-budget-regression", ["REQ-TV2"], "original t06 30s deadline preserves completed English result", lambda: budget_regression(run))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
