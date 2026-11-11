"""T07 details gate. Native HTTP fixtures call production Host/adapters.

Cloudstream observations are explicitly synthetic load/DTO boundary vectors;
the original runtime and Ddys/SuperVod execution remain in N02.
"""
import argparse
import copy
import datetime as dt
import hashlib
from html.parser import HTMLParser
import json
import math
import os
from pathlib import Path
import platform
import re
import sys
import time
import uuid
import xml.etree.ElementTree as ET

from t00 import Acceptance as BaseAcceptance, read_json, require, workflow, write_json

ROOT = Path(__file__).resolve().parents[2]
IDS = ("R05", "R17", "R18", "R22", "R31", "R59", "R60", "REQ-T07")
RUST_TESTS = {
    "t07_guest_original_opaque_movie_series_anime_groups_and_images": ("R05", "R17", "R18", "R22", "R31", "REQ-T07"),
    "t07_common_missing_fields_rewritten_refs_and_invalid_relationships": ("R17", "R18", "R22", "REQ-T07"),
    "t07_guest_late_epoch_revision_cancel_timeout_and_error": ("R05", "R18", "REQ-T07"),
    "t07_maccms_http_routes_200_order_metadata_and_registration": ("R17", "R18", "R22", "R31", "R59", "REQ-T07"),
    "t07_maccms_http_unknown_null_zero_and_empty_episodes": ("R17", "R22", "R59", "REQ-T07"),
    "t07_maccms_source_revision_canonical_and_late_details": ("R18", "R59", "REQ-T07"),
    "t07_stremio_http_movie_series_anime_seasons_200_and_images": ("R17", "R18", "R22", "R31", "R60", "REQ-T07"),
    "t07_stremio_http_null_metadata_no_season_no_images_and_empty": ("R17", "R22", "R60", "REQ-T07"),
    "t07_stremio_units_ratings_status_dates_and_identity_negatives": ("R17", "R18", "R22", "R60", "REQ-T07"),
    "t07_stremio_late_details_cancel_locale_edit_and_revision": ("R18", "R60", "REQ-T07"),
}


class Acceptance(BaseAcceptance):
    def __init__(self, report, junit):
        self.report_path, self.junit_path = report, junit
        self.started = dt.datetime.now(dt.timezone.utc).isoformat()
        self.folder = ROOT / "evidence/acceptance" / ("T07-" + uuid.uuid4().hex)
        self.folder.mkdir(parents=True)
        self.source = workflow.snapshot()
        self.cases, self.commands, self.observed = [], [], {}

    def save(self):
        current = workflow.snapshot()
        self.case("stable-tested-source", ["REQ-T07"], "source unchanged during A1",
                  lambda: require(current == self.source, "source changed during A1"))
        summaries = []
        for identifier in IDS:
            children = [case for case in self.cases if identifier in case["requirements"]]
            summaries.append(dict(id=identifier, children=[case["id"] for case in children],
                                  result="PASS" if children and all(case["result"] == "PASS" for case in children) else "FAIL"))
        counts = dict(required=len(self.cases), ran=len(self.cases),
                      passed=sum(case["result"] == "PASS" for case in self.cases),
                      failed=sum(case["result"] != "PASS" for case in self.cases), skipped=0)
        report = dict(card="T0.7", original_card="T07", suite="all", utc_started=self.started,
                      utc_finished=dt.datetime.now(dt.timezone.utc).isoformat(), source_before=self.source,
                      tested_source_digest=current, protected=workflow.protected_hash(), platform=sys.platform,
                      arch=platform.machine(), os=platform.platform(), commands=self.commands, cases=self.cases,
                      summaries=summaries, observed=self.observed, counts=counts,
                      native_counts=self.observed.get("native_counts", dict(required=len(RUST_TESTS), ran=0, passed=0, failed=0, skipped=0, status="NOT RUN")),
                      deployment="ordinary native process and owned loopback HTTP fixtures; no cluster or Android",
                      cleanup="fixture Drop stops/joins owned workers; original runner performs final descendant cleanup",
                      cloudstream=dict(synthetic=True, scope="load opaque forwarding, common normalized DTO, recursive image registration and snapshot rejection",
                                       real_providers="NOT RUN: original runtime, Ddys/SuperVod and seven packages belong to N02"))
        write_json(self.report_path, report)
        rows = self.cases + summaries
        xml = ET.Element("testsuite", name="T07-all", tests=str(len(rows)), errors="0", skipped="0",
                         failures=str(sum(row["result"] != "PASS" for row in rows)), timestamp=self.started)
        for row in rows:
            node = ET.SubElement(xml, "testcase", classname="T07", name=row["id"])
            if row["result"] != "PASS":
                ET.SubElement(node, "failure", message="T07 behavior assertion failed").text = json.dumps(row, ensure_ascii=False)
        self.junit_path.parent.mkdir(parents=True, exist_ok=True)
        ET.ElementTree(xml).write(self.junit_path, encoding="utf-8", xml_declaration=True)
        print(json.dumps(counts), flush=True)
        return int(counts["failed"] > 0 or any(row["result"] != "PASS" for row in summaries))


def resources(run):
    require(os.environ.get("AGENT_WORKFLOW_RUN") == "1", "original run.py wrapper required")
    cfg = workflow.config()
    require(cfg["memory_mib"] == 4096 and cfg["jobs"] == 2, "approved resource budget differs")
    require(os.environ.get("CARGO_BUILD_JOBS") == "2" and os.environ.get("RUST_TEST_THREADS") == "1", "build/test concurrency differs")
    require(Path(os.environ["CARGO_TARGET_DIR"]) == ROOT / "target", "target binding differs")
    require(Path.cwd().resolve() == ROOT and Path(sys.executable).absolute() == ROOT / ".venv/bin/python", "root/venv binding differs")
    ancestors = {p.pid for p in workflow.runtime.psutil.Process().parents()} | {os.getpid()}
    deadline, matches = time.monotonic() + 5, []
    while time.monotonic() < deadline:
        receipts = [(path, read_json(path)) for path in (ROOT / "evidence/runs").glob("*/receipt.json")]
        matches = [(path, row) for path, row in receipts if row.get("status") == "running" and row.get("payload_pid") in ancestors
                   and workflow.runtime.alive(dict(pid=row["payload_pid"], identity=row["payload_identity"]))]
        if matches:
            break
        time.sleep(0.05)
    require(len(matches) == 1, "one live original resource receipt must own A1")
    path, receipt = matches[0]
    require(receipt["source"] == run.source and receipt["card"] == "T0.7", "receipt source/card differs")
    env = receipt["environment"]
    require(env["backend"] == workflow.runtime.backend() and env["memory_mib"] == 4096
            and env["jobs"] == 2 and env["test_threads"] == 1, "receipt backend/budget differs")
    require(sys.platform == "darwin" and platform.machine() == "arm64", "local macOS arm64 gate required")
    require(env["memory_enforcement"] == "rss-sampled", "actual RSS sampled backend required")
    run.observed["resources"] = dict(receipt=str(path.relative_to(ROOT)), environment=env,
                                     note="RSS sampling permits transient overage; no cgroup hard memory or CPU quota")
    return run.observed["resources"]


def identity(tag, *fields):
    data = tag.encode() + b"\0"
    for field in fields:
        field = str(field).encode("utf-8")
        data += len(field).to_bytes(4, "big") + field
    return hashlib.sha256(data).hexdigest()


class Plain(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts, self.hidden = [], 0

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self.hidden += 1

    def handle_endtag(self, tag):
        if tag in ("script", "style"):
            self.hidden -= 1

    def handle_data(self, data):
        if not self.hidden:
            self.parts.append(data)


def plain(value):
    parser = Plain()
    parser.feed(value)
    return "".join(parser.parts).strip()


def words(value):
    return [plain(word.strip()) for word in re.split(r"[,，/、]", value or "") if word.strip()]


def score(value):
    try:
        number = float(value)
        return dict(value=number, scale=10) if math.isfinite(number) and 0 <= number <= 10 else None
    except (TypeError, ValueError):
        return None


def image_refs(value):
    if isinstance(value, dict):
        if "resource_id" in value and "capability" in value:
            yield value
        else:
            for child in value.values():
                yield from image_refs(child)
    elif isinstance(value, list):
        for child in value:
            yield from image_refs(child)


def image_url(vector, image):
    return vector["image_urls"].get(image["resource_id"]) if image else None


def check_vector(vector):
    """Independent comparison to pre-mapping HTTP input or synthetic guest DTO."""
    details, reference = vector["details"], vector["reference"]
    require(details["ref"] == reference, "rewritten content reference")
    groups = details["groups"]
    require(len({group["id"] for group in groups}) == len(groups), "duplicate group ID")
    episodes = [episode for group in groups for episode in group["episodes"]]
    require(len({episode["id"] for episode in episodes}) == len(episodes), "duplicate episode ID")
    for group in groups:
        require([episode["index"] for episode in group["episodes"]] == list(range(len(group["episodes"]))), "episode indices differ from source positions")
    for row in episodes + details["related"]:
        require(all(row["ref"][key] == reference[key] for key in ("source_id", "source_revision")), "foreign detail reference")
    for image in image_refs(details):
        require(vector["registered"].get(image["resource_id"]) == image, "unregistered or rewritten image")
    if vector["adapter"].startswith("guest"):
        require(details == vector["expected"], "synthetic normalized guest DTO changed")
        return
    raw = vector["raw"]
    require(details.get("episode_count") is None, "downloaded entries invented an explicit total")
    require(details.get("rating") == score(raw.get("vod_score" if vector["adapter"] == "maccms" else "imdbRating")), "rating or unknown/zero semantics changed")
    if vector["adapter"] == "maccms":
        require(details["title"] == raw["vod_name"] and details["kind"] == "other", "MacCMS title/type invented")
        require(details.get("plot") == (plain(raw["vod_content"]) if raw.get("vod_content") is not None else None), "MacCMS plot changed")
        for target, source in (("actors", "vod_actor"), ("directors", "vod_director")):
            expected = [dict(name=name, role=None, image=None) for name in words(raw.get(source))]
            require(details.get(target, []) == expected, f"MacCMS {target} differ")
        for target, source in (("regions", "vod_area"), ("languages", "vod_lang"), ("tags", "vod_class")):
            require(details.get(target, []) == words(raw.get(source)), f"MacCMS {target} differ")
        require(details.get("status") == {"0": "ongoing", "1": "completed"}.get(str(raw.get("vod_isend")), "unknown"), "invented MacCMS status")
        require(details.get("update_label") == (raw.get("vod_remarks") or None), "MacCMS update label changed")
        year = str(raw.get("vod_year", ""))
        require(details.get("year") == (int(year) if re.fullmatch(r"[1-9][0-9]{3}", year) else None), "MacCMS year changed")
        poster_url = image_url(vector, details.get("poster"))
        require((poster_url is not None and poster_url.endswith(raw["vod_pic"])) if raw.get("vod_pic") else poster_url is None, "MacCMS poster dropped or changed")
        names, urls = raw.get("vod_play_from", ""), raw.get("vod_play_url", "")
        expected_groups = list(zip(names.split("$$$"), urls.split("$$$"))) if names or urls else []
        require(len(groups) == len(expected_groups), "MacCMS route lost")
        require(details["id"] == identity("content", reference["source_id"], reference["data"]), "MacCMS content identity changed")
        for ordinal, (group, (title, entries)) in enumerate(zip(groups, expected_groups)):
            require(group["title"] == title and group["kind"] == "route" and group["dub_status"] == "unspecified" and group.get("season_number") is None, "MacCMS route metadata changed")
            rows = entries.split("#")
            group_id = identity("group", details["id"], reference["source_revision"], ordinal, title, "route", "", "unspecified")
            require(group["id"] == group_id, "MacCMS group identity changed")
            require(len(group["episodes"]) == len(rows), "MacCMS episodes lost")
            for index, (episode, entry) in enumerate(zip(group["episodes"], rows)):
                label = entry.split("$", 1)[0] if not entry.startswith(("http://", "https://")) and "$" in entry else str(index + 1)
                fields = ["maccms_episode", str(raw["vod_id"]), str(ordinal), str(index), identity("maccms_entry", title, entry)]
                require(episode["label"] == (label or str(index + 1)) and episode["ref"]["data"] == json.dumps(fields, ensure_ascii=False, separators=(",", ":")), "MacCMS original entry/ref changed")
                require(episode["id"] == identity("episode", details["id"], reference["source_revision"], group_id, index, episode["ref"]["data"]), "MacCMS episode identity changed")
    else:
        require(details["title"] == plain(raw["name"]) and details["kind"] == raw["type"], "Stremio type/title changed")
        require(details.get("plot") == (plain(raw["description"]) if raw.get("description") else None), "Stremio plot changed")
        year = str(raw.get("releaseInfo", raw.get("year", "")))[:4]
        require(details.get("year") == (int(year) if re.fullmatch(r"[1-9][0-9]{3}", year) else None), "Stremio year changed")
        for target, source in (("poster", "poster"), ("backdrop", "background")):
            require(image_url(vector, details.get(target)) == raw.get(source), f"Stremio {target} dropped or changed")
        require(details.get("tags", []) == [plain(value) for value in raw.get("genres", [])], "Stremio tags changed")
        for target, source in (("regions", "country"), ("languages", "language")):
            require(details.get(target, []) == [plain(v) for v in re.split("[,，]", raw.get(source, "")) if v.strip()], f"Stremio {target} changed")
        require([p["name"] for p in details.get("actors", [])] == [plain(v) for v in raw.get("cast", [])], "Stremio actors changed")
        require([p["name"] for p in details.get("directors", [])] == [plain(v) for v in raw.get("director", [])], "Stremio directors changed")
        for person in details.get("actors", []):
            matches = [p for p in raw.get("app_extras", {}).get("cast", []) if plain(p["name"]) == person["name"]]
            match = matches[0] if len(matches) == 1 else {}
            require(person.get("role") == (plain(match["character"]) if match.get("character") else None), "actor role attached to a different name")
            expected_url = match.get("photo")
            require(image_url(vector, person.get("image")) == expected_url, "actor photo attached to a different name")
        require(details.get("duration_ms") == (8_880_000 if raw.get("runtime") == "2h28min" else None), "duration units or unknown value changed")
        running = raw.get("releaseInfo") == "2024-" and raw["type"] in ("series", "anime")
        require(details.get("status") == ("ongoing" if running else "unknown") and details.get("update_label") is None, "invented Stremio status/update")
        raw_videos = raw.get("videos") or ([dict(id=raw["id"], title=raw["name"])] if raw["type"] == "movie" else [])
        runs = []
        for video in raw_videos:
            if not runs or runs[-1][0] != video.get("season"):
                runs.append((video.get("season"), []))
            runs[-1][1].append(video)
        require(len(groups) == len(runs), "Stremio season segment lost")
        content_fields = json.loads(reference["data"])
        require(vector["metadata_ref"]["content_ref"] == reference, "Stremio metadata origin lost")
        require(vector["metadata_ref"]["identity"]["addon_key"] == content_fields[1], "Stremio addon ownership changed")
        metadata = vector["metadata_ref"]["identity"]
        require(metadata["type"] == raw["type"] and metadata["meta_id"] == raw["id"], "Stremio metadata identity changed")
        require(details["id"] == identity("metadata", metadata["namespace"], metadata["type"], metadata["canonical_id"]), "Stremio content identity changed")
        for ordinal, (group, (season, rows)) in enumerate(zip(groups, runs)):
            require(group.get("season_number") == season and group["kind"] == ("season" if season is not None else "other") and group["dub_status"] == "unspecified", "Stremio grouping metadata changed")
            require(len(group["episodes"]) == len(rows), "Stremio episodes lost")
            title = f"Season {season}" if season is not None else "Videos"
            group_id = identity("group", details["id"], reference["source_revision"], ordinal, title, group["kind"], season if season is not None else "", "unspecified")
            require(group["id"] == group_id, "Stremio group identity changed")
            for index, (episode, video) in enumerate(zip(group["episodes"], rows)):
                fields = ["stremio_video", content_fields[1], raw["type"], raw["id"], video["id"]]
                require(episode["ref"]["data"] == json.dumps(fields, ensure_ascii=False, separators=(",", ":")), "Stremio original video ID/order changed")
                require(episode["id"] == identity("episode", details["id"], reference["source_revision"], group_id, index, episode["ref"]["data"]), "Stremio episode identity changed")
                require(episode["label"] == plain(video["title"]) and episode.get("title") == plain(video["title"]), "Stremio episode title changed")
                require(image_url(vector, episode.get("poster")) == video.get("thumbnail"), "Stremio episode image dropped or changed")
                require(episode.get("season") == video.get("season") and episode.get("number") == video.get("episode"), "Stremio season/episode changed")
                require(episode.get("duration_ms") == (2_700_000 if video.get("runtime") == "45 min" else None), "episode runtime units changed")
                require(episode.get("released_at") == ("2026-10-29T00:00:00Z" if video.get("released") else None), "episode UTC conversion changed")


def independent(run, output):
    from jsonschema import Draft202012Validator, FormatChecker
    from referencing import Registry, Resource
    app, bridge = read_json(ROOT / "protocol/app.schema.json"), read_json(ROOT / "protocol/bridge.openapi.json")
    bridge_uri = "https://replica.invalid/contracts/bridge.openapi.json"
    registry = Registry().with_resource(app["$id"], Resource.from_contents(app)).with_resource(bridge_uri, Resource.from_contents({
        **bridge, "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": bridge_uri}))
    validator = Draft202012Validator({"$ref": app["$id"] + "#/$defs/DetailsViewData"}, registry=registry, format_checker=FormatChecker())
    vectors = [json.loads(line.split("T07_VECTOR:", 1)[1]) for line in output.splitlines() if "T07_VECTOR:" in line]
    require(len(vectors) == 19, f"all detail observations required, got {len(vectors)}")
    mutations = 0
    for vector in vectors:
        validator.validate(dict(details=vector["details"], images={}, metadata_ref=vector.get("metadata_ref")))
        check_vector(vector)
        edits = [lambda v: v["details"].pop("groups"), lambda v: v["details"]["ref"].update(data="rewritten"),
                 lambda v: v["details"].update(rating=dict(value=9, scale=10)), lambda v: v["details"].update(year=1900)]
        if vector["details"]["groups"]:
            edits += [lambda v: v["details"]["groups"].pop(), lambda v: v["details"]["groups"][0]["episodes"][0].update(index=99),
                      lambda v: v["details"]["groups"][0]["episodes"][0]["ref"].update(source_id="foreign")]
        if list(image_refs(vector["details"])):
            edits += [lambda v: v.update(registered={})]
        for edit in edits:
            invalid = copy.deepcopy(vector)
            edit(invalid)
            try:
                check_vector(invalid)
            except (ValueError, KeyError, IndexError):
                mutations += 1
            else:
                raise ValueError(f"oracle accepted corrupted {vector['adapter']} projection")
    require({v["details"]["kind"] for v in vectors if v["adapter"] == "stremio"} == {"movie", "series", "anime"}, "three HTTP content kinds required")
    require(any(len(g["episodes"]) == 200 for v in vectors for g in v["details"]["groups"]), "200 episode source order missing")
    write_json(run.folder / "details-vectors.json", vectors)
    run.observed["dto"] = dict(vectors=len(vectors), corrupted_projections_rejected=mutations,
                               trace=str((run.folder / "details-vectors.json").relative_to(ROOT)))
    return run.observed["dto"]


def rust(run):
    try:
        output = run.command(["cargo", "test", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
                              "--lib", "t07_", "--", "--test-threads=1", "--nocapture"], timeout=3000)
    except Exception:
        output = (ROOT / run.commands[-1]["log"]).read_text(encoding="utf-8", errors="replace")
        result = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;", output)
        passed, failed, skipped = map(int, result.groups()) if result else (0, 0, 0)
        run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=passed + failed + skipped, passed=passed,
                                               failed=failed, skipped=skipped, status="FAIL" if result else "NOT RUN", log=run.commands[-1]["log"])
        raise
    names = set(re.findall(r"^test (?:\w+::)*(t07_\w+) \.\.\.", output, re.M))
    require(names == set(RUST_TESTS), f"native inventory differs: {names ^ set(RUST_TESTS)}")
    summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; 0 ignored;", output)
    require(summary is not None and int(summary[1]) == len(RUST_TESTS), "all nonzero named native tests must pass")
    run.observed["native_counts"] = dict(required=len(RUST_TESTS), ran=len(names), passed=len(names), failed=0, skipped=0, status="PASS")
    for name, requirements in RUST_TESTS.items():
        run.case(name, requirements, "production behavior assertions executed and passed",
                 lambda name=name: dict(test=name, exit=0, log=run.commands[-1]["log"]))
    independent(run, output)
    return dict(tests=len(names), exit=0, log=run.commands[-1]["log"], dto=run.observed["dto"])


def main():
    if not __debug__:
        raise SystemExit("Acceptance requires assertions; remove -O")
    parser = argparse.ArgumentParser()
    parser.add_argument("--suite", choices=["all"], required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--junit", type=Path, required=True)
    options = parser.parse_args()
    run = Acceptance(options.report, options.junit)
    ready = run.case("resource-ownership", ["REQ-T07"], "original resource scope and local platform", lambda: resources(run))
    run.case("native-behaviors", list(IDS), "named native tests and independent detail projection checks",
             lambda: rust(run) if ready else require(False, "NOT RUN: resource ownership failed"))
    return run.save()


if __name__ == "__main__":
    raise SystemExit(main())
