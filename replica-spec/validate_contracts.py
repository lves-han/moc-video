#!/usr/bin/env python3
"""Validate the documentation's schemas, authored examples, and static evidence.

Requires: openapi-spec-validator, jsonschema (with referencing).
Run from any directory: python3 /absolute/path/replica-spec/validate_contracts.py
Optionally verify originals with --artifact-root /path/to/extracted/asar.
This does not execute the original app, plugins, or application tests.
"""
import argparse
import copy
import datetime
import hashlib
import importlib.metadata
import ipaddress
import json
import re
import sys
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker
from openapi_spec_validator import validate
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parent
BRIDGE_URI = "https://replica.invalid/contracts/bridge.openapi.json"


def read(relative):
    return json.loads((ROOT / relative).read_text())


def digest(tag, *parts):
    data = tag.encode("utf-8") + b"\0"
    for part in parts:
        value = str(part).encode("utf-8")
        data += len(value).to_bytes(4, "big") + value
    return hashlib.sha256(data).hexdigest()


def source_binding(value):
    return all(value[key] == value["episode_ref"][key]
               for key in ("source_id", "source_revision"))


def check_source_bindings(value):
    """Check peer identities that JSON Schema cannot compare dynamically."""
    if isinstance(value, dict):
        if {"source_id", "source_revision", "episode_ref"} <= value.keys():
            assert source_binding(value), "episode_ref的源/修订与所属对象不一致"
        for key, child in value.items():
            # Error diagnostics may intentionally include the rejected old reference.
            if key != "error":
                check_source_bindings(child)
    elif isinstance(value, list):
        for child in value:
            check_source_bindings(child)


def context_binding(context):
    # Provider content/episode stay together; Stremio metadata has its own identity.
    return (bool(context.get("match_id")) and bool(context.get("metadata_video_id"))
            and context["metadata_ref"]["content_ref"]["source_id"] != context["episode_ref"]["source_id"]
            and all(context["content_ref"][key] == context["episode_ref"][key]
                    for key in ("source_id", "source_revision")))


def match_input_binding(value):
    source = value["playback_source"]
    selection = value["selection"]
    return (value["metadata_ref"]["content_ref"]["source_id"] != source["source_id"]
            and (selection is None or all(selection["content_ref"][key] == source[key]
                                          for key in ("source_id", "source_revision"))))


def playback_match_binding(value):
    context = value["context"]
    if value["status"] != "matched":
        return context is None
    return (context is not None and context_binding(context)
            and context["metadata_ref"] == value["metadata_ref"]
            and context["metadata_video_id"] == value["video_id"]
            and context["selected_stream_id"] is None
            and len(value["candidates"]) == 1
            and context["content_ref"] == value["candidates"][0]["content_ref"])


def history_record(value):
    identity = value["metadata_identity"]
    namespace = identity["namespace"]
    return (value["content_id"] == digest("metadata", namespace, identity["type"], identity["canonical_id"])
            and (namespace in ("imdb", "tmdb") or namespace == "addon:" + identity["addon_key"])
            and (namespace != "imdb" or re.fullmatch(r"tt[0-9]+", identity["canonical_id"]) is not None)
            and (namespace != "tmdb" or re.fullmatch(r"[0-9]+", identity["canonical_id"]) is not None)
            and (value["duration_ms"] is None or value["position_ms"] <= value["duration_ms"]))


def session_binding(resolution, session):
    return (source_binding(resolution) and source_binding(session)
            and session["resolution_id"] == resolution["id"]
            and all(session[key] == resolution[key] for key in
                    ("source_id", "source_revision", "episode_ref"))
            and any(stream["id"] == session["stream_id"] and stream["playable"]
                    and stream["kind"] == session["stream_kind"]
                    for stream in resolution["streams"]))


def playback_resolution(state):
    session, resolution = state.get("session"), state.get("resolution")
    context = state.get("context")
    if context is not None and not context_binding(context):
        return False
    if resolution is not None:
        if context is None or context["episode_ref"] != resolution["episode_ref"]:
            return False
        selected = context["selected_stream_id"]
        if selected is not None and not any(stream["id"] == selected for stream in resolution["streams"]):
            return False
    elif context is not None and context["selected_stream_id"] is not None:
        return False
    if session is not None:
        return (resolution is not None and context is not None
                and context["selected_stream_id"] == session["stream_id"]
                and session_binding(resolution, session))
    return True


def app_playback_binding(value):
    state, route = value["playback"], value["route"]
    if not playback_resolution(state):
        return False
    context = state["context"]
    if route["name"] == "playback" and route["context"] != context:
        return False
    if context is None:
        return True
    if state["engine_epoch"] != value["runtime"]["engine_epoch"]:
        return False
    selected = value["selected_playback_source"]
    if selected is None or any(selected[key] != context["episode_ref"][key]
                               for key in ("source_id", "source_revision")):
        return False
    selected_metadata = value["selected_metadata_source"]
    metadata = context["metadata_ref"]
    if selected_metadata is None or any(selected_metadata[key] != metadata["content_ref"][key]
                                        for key in ("source_id", "source_revision")):
        return False
    providers = value["sources"].get("data", {}).get("providers", [])
    by_id = {provider["id"]: provider for provider in providers}
    if (by_id.get(selected_metadata["source_id"], {}).get("kind") != "stremio"
            or by_id.get(selected["source_id"], {}).get("kind") not in ("cloudstream", "maccms")):
        return False
    metadata_provider = by_id[selected_metadata["source_id"]]
    if metadata_provider["origin"]["config_id"] != metadata["identity"]["config_id"]:
        return False
    details = value["details"]["data"]
    if details is not None:
        if details["metadata_ref"] != metadata or details["details"]["ref"] != metadata["content_ref"]:
            return False
        videos = []
        for group in details["details"]["groups"]:
            for episode in group["episodes"]:
                try:
                    videos.append(json.loads(episode["ref"]["data"]).get("video_id"))
                except (ValueError, AttributeError):
                    return False
        if context["metadata_video_id"] not in videos:
            return False
    return True


def check_playback_resolutions(value):
    """Bind route, context, selected episode, resolution and session."""
    if isinstance(value, dict):
        if {"route", "playback", "selected_metadata_source", "selected_playback_source", "runtime", "details"} <= value.keys():
            assert app_playback_binding(value), "路由/选集/播放上下文绑定不一致"
        if {"content_ref", "episode_ref", "selected_stream_id"} <= value.keys():
            assert context_binding(value), "元数据/匹配身份或播放源内容与集数引用绑定不一致"
        if {"status", "metadata_ref", "video_id", "candidates", "context"} <= value.keys():
            assert playback_match_binding(value), "匹配结果的元数据/video/provider绑定不一致"
        if {"metadata_ref", "video_id", "playback_source", "selection"} <= value.keys():
            assert match_input_binding(value), "匹配选择必须属于指定播放源"
        if {"playback_epoch", "session", "resolution"} <= value.keys():
            assert playback_resolution(value), "PlaybackState的session不属于其resolution"
        for child in value.values():
            check_playback_resolutions(child)
    elif isinstance(value, list):
        for child in value:
            check_playback_resolutions(child)


def home_positions(page):
    entries = page["sections"] + page["failed_sections"]
    positions = [entry.get("index") for entry in entries]
    return (all(isinstance(index, int) and not isinstance(index, bool) and index >= 0
                for index in positions) and len(positions) == len(set(positions)))


def home_response(request, page):
    if not home_positions(page) or any(request[key] != page[key]
                                       for key in ("source_id", "source_revision")):
        return False
    if page["failed_sections"] and not page["sections"]:
        return False
    if any(section["page"] != request["page"] for section in page["sections"]):
        return False
    reference = request.get("section_ref")
    if reference is None:
        indices = [entry["index"] for entry in page["sections"] + page["failed_sections"]]
        return request["page"] == 1 and sorted(indices) == list(range(len(indices)))
    return (len(page["sections"]) == 1 and not page["failed_sections"]
            and page["sections"][0]["section_ref"] == reference
            and page["sections"][0]["page"] == request["page"])


def check_home_positions(value):
    if isinstance(value, dict):
        if {"sections", "failed_sections"} <= value.keys():
            assert home_positions(value), "首页成功/失败栏目index重复或缺失"
        for child in value.values():
            check_home_positions(child)
    elif isinstance(value, list):
        for child in value:
            check_home_positions(child)


def home_merge(vector):
    """Evaluate the documented merge rule on authored presentation fixtures."""
    before, request, response = vector["before"], vector["request"], vector["response"]
    request = {"source_id": before["source_id"], "source_revision": before["source_revision"], **request}
    if not home_response(request, response):
        return False
    if request.get("section_ref") is None:
        merged = response
    else:
        target = next((entry for entry in before["sections"] + before["failed_sections"]
                       if entry["section_ref"] == request["section_ref"]), None)
        incoming = copy.deepcopy(response["sections"][0])
        if target is None or target["index"] != incoming["index"]:
            return False
        merged = copy.deepcopy(before)
        if request["page"] >= 2:
            previous = next((entry for entry in before["sections"]
                             if entry["section_ref"] == request["section_ref"]), None)
            if previous is None:
                return False
            seen = {item["id"] for item in previous["items"]}
            items = list(previous["items"])
            for item in incoming["items"]:
                if item["id"] not in seen:
                    items.append(item)
                    seen.add(item["id"])
            incoming["items"] = items
        merged["sections"] = [entry for entry in merged["sections"]
                              if entry["index"] != incoming["index"]] + [incoming]
        merged["failed_sections"] = [entry for entry in merged["failed_sections"]
                                     if entry["index"] != incoming["index"]]
    entries = sorted(merged["sections"] + merged["failed_sections"], key=lambda entry: entry["index"])
    if ([entry["index"] for entry in entries] != vector["expected_indices"]
            or [entry["title"] for entry in entries] != vector["expected_titles"]):
        return False
    actual_items = {entry["section_ref"]: [item["id"] for item in entry["items"]]
                    for entry in merged["sections"]}
    return all(actual_items.get(key) == value for key, value in vector.get("expected_items", {}).items())


def lifecycle(vector):
    """Contract transition model; runtime ownership is verified by task cards."""
    state = copy.deepcopy(vector["initial"])
    for action in vector["actions"]:
        operation = action["op"]
        if operation == "close":
            state["sessions"].pop(action["id"], None)
        elif operation == "release":
            state["resolutions"] = [key for key in state["resolutions"] if key != action["id"]]
        elif operation == "open":
            assert action["resolution_id"] in state["resolutions"], "候选表已释放"
            state["sessions"][action["id"]] = action["resolution_id"]
        elif operation == "plugin_commit":
            affected = set(action["affected_resolutions"])
            state["resolutions"] = [key for key in state["resolutions"] if key not in affected]
            state["sessions"] = {key: value for key, value in state["sessions"].items() if value not in affected}
        else:
            raise AssertionError(operation)
    return state == vector["expected"]


def cancel_identity(value):
    return value["request_id"] != value["input"]["target_request_id"]


def check_cancel_identities(value):
    if isinstance(value, dict):
        if ("request_id" in value and isinstance(value.get("input"), dict)
                and "target_request_id" in value["input"]):
            assert cancel_identity(value), "取消调用使用了目标任务自身的request_id"
        for child in value.values():
            check_cancel_identities(child)
    elif isinstance(value, list):
        for child in value:
            check_cancel_identities(child)


def semantic(case):
    kind = case["kind"]
    if kind == "home_positions":
        return home_positions(case["value"])
    if kind == "home_response":
        return home_response(case["request"], case["value"])
    if kind == "home_merge":
        return home_merge(case["value"])
    if kind == "match_input_binding":
        return match_input_binding(case["value"])
    if kind == "playback_match_binding":
        return playback_match_binding(case["value"])
    if kind == "history_record":
        return history_record(case["value"])
    if kind == "app_playback_binding":
        return app_playback_binding(case["value"])
    if kind == "stremio_resolve_capability":
        resources = case["value"]["resources"]
        names = {resource if isinstance(resource, str) else resource["name"] for resource in resources}
        return bool(names & set(read("app.schema.json")["x-stremio-capability-rule"]["resolve_resources"]))
    if kind == "cancel_identity":
        return cancel_identity({"request_id": case["request_id"],
                                "input": {"target_request_id": case["target_request_id"]}})
    if kind == "publish_guard":
        return (case["exists"] and case["enabled"]
                and case["captured"] == case["current"]
                and case["sync_sequence"] == case["latest_sync_sequence"])
    if kind == "session_context":
        # Existing sessions retain their context after resolution collection.
        return case["session_active"] and case["owns_link_context"]
    if kind == "manifest_commit":
        return (bool(case["results"]) and all(case["results"])
                and case["captured_generation"] == case["current_generation"])
    if kind == "public_connection":
        def normalized(address):
            parsed = ipaddress.ip_address(address)
            return getattr(parsed, "ipv4_mapped", None) or parsed
        addresses = [normalized(address) for address in case["addresses"]]
        peer = normalized(case["peer"])
        return bool(addresses) and all(address.is_global and not address.is_multicast
                                       for address in addresses) and peer in addresses
    if kind == "request_budget":
        budgets = read("app.schema.json")["x-request-budgets"]
        operation = budgets["operations"][case["operation"]]
        return case["transport_ms"] >= operation["total_ms"] + budgets["transport_grace_ms"]
    if kind == "resource_target":
        return all(case["response_" + key] == case["current_" + key]
                   for key in ("target", "epoch", "kind", "revision", "request_epoch"))
    if kind == "config_ids_unique":
        keys = [config["config_id"] for config in case["configs"]]
        return len(keys) == len(set(keys))
    if kind == "backend_isolation":
        before, after = case["before"], case["after"]
        return (before["runtime"]["engine_epoch"] == after["runtime"]["engine_epoch"]
                and before["playback"] == after["playback"]
                and before["selected_playback_source"] == after["selected_playback_source"])
    if kind == "stremio_filter":
        manifest = case["manifest"]
        if case["resource"] == "catalog":
            return any(catalog["type"] == case["type"] and catalog["id"] == case["id"]
                       for catalog in manifest["catalogs"])
        for resource in manifest["resources"]:
            name = resource if isinstance(resource, str) else resource["name"]
            if name != case["resource"]:
                continue
            declaration = manifest if isinstance(resource, str) else resource
            prefixes = declaration.get("idPrefixes")
            if (case["type"] in declaration["types"]
                    and (prefixes is None or any(case["id"].startswith(prefix) for prefix in prefixes))):
                return True
        return False
    if kind == "source_binding":
        return source_binding(case["value"])
    if kind == "session_binding":
        return session_binding(case["resolution"], case["session"])
    if kind == "playback_resolution":
        return playback_resolution(case["value"])
    if kind == "page_order":
        value = case["value"]
        return value["next_page"] > value["page"] if value["has_next"] else value["next_page"] is None
    if kind == "utf8_limit":
        return len((case["repeat"] * case["count"]).encode("utf-8")) <= 524288
    if kind == "identity_distinct":
        return digest("fixture", *case["left"]) != digest("fixture", *case["right"])
    if kind == "buffered_range":
        value = case["value"]
        return value["start_ms"] <= value["end_ms"]
    raise AssertionError(f"Unknown semantic case: {kind}")


def main(argv=None):
    if not __debug__:
        sys.exit("validate_contracts.py依赖assert执行检查，禁止使用python -O运行")
    parser = argparse.ArgumentParser(description="离线检查文档契约、样例和任务依赖；原样本字节复核可选。")
    parser.add_argument("--artifact-root", type=Path,
                        help="提取后的ASAR根目录，包含dist/assets；提供时完整复核原样本摘要和片段")
    parser.add_argument("--report", type=Path, default=ROOT / "references/validation-report.json",
                        help="报告输出路径，默认references/validation-report.json")
    args = parser.parse_args(argv)
    bridge, app = read("bridge.openapi.json"), read("app.schema.json")
    validate(bridge)
    Draft202012Validator.check_schema(app)
    bridge_resource = Resource.from_contents({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": BRIDGE_URI, **bridge,
    })
    registry = Registry().with_resource(BRIDGE_URI, bridge_resource).with_resource(
        app["$id"], Resource.from_contents(app)
    )

    def validator(document, schema):
        pointer = f"{BRIDGE_URI}#/components/schemas/{schema}" if document == "bridge" else f"{app['$id']}#/$defs/{schema}"
        return Draft202012Validator({"$ref": pointer}, registry=registry, format_checker=FormatChecker())

    # Resolve every local/cross-file schema reference without fetching any network URL.
    docs = {"bridge.openapi.json": bridge, "app.schema.json": app}
    ref_count = 0

    def refs(value, owner):
        nonlocal ref_count
        if isinstance(value, dict):
            if "$ref" in value:
                target, pointer = value["$ref"].split("#", 1)
                selected = docs[target or owner]
                for part in pointer.lstrip("/").split("/"):
                    selected = selected[part.replace("~1", "/").replace("~0", "~")]
                ref_count += 1
            for child in value.values():
                refs(child, owner)
        elif isinstance(value, list):
            for child in value:
                refs(child, owner)

    for name, doc in docs.items():
        refs(doc, name)
    embedded_examples = 0
    for name, schema in bridge["components"]["schemas"].items():
        for example in schema.get("examples", []):
            validator("bridge", name).validate(example)
            check_source_bindings(example)
            check_home_positions(example)
            embedded_examples += 1
    fixture = read("examples/contract-cases.json")
    assert fixture["schema_version"] == bridge["info"]["version"]
    assert app["$defs"]["HostHealth"]["properties"]["contract_version"]["const"] == bridge["info"]["version"]
    names = set()
    for case in fixture["cases"]:
        assert case["name"] not in names, case["name"]
        names.add(case["name"])
        errors = list(validator(case["document"], case["schema"]).iter_errors(case["value"]))
        actual = not errors
        assert actual == case["expected_valid"], (case["name"], [e.message for e in errors][:3])
        if actual:
            check_source_bindings(case["value"])
            check_playback_resolutions(case["value"])
            check_cancel_identities(case["value"])
            check_home_positions(case["value"])
            if case["schema"] == "PlaybackMatchResult":
                assert playback_match_binding(case["value"]), case["name"]
            if case["schema"] == "WatchHistoryRecord":
                assert history_record(case["value"]), case["name"]
    for case in fixture["semantic_cases"]:
        assert semantic(case) == case["expected_valid"], case["name"]
    for vector in fixture["lifecycle_vectors"]:
        assert lifecycle(vector), vector["name"]
    for command in app["x-command-contracts"]:
        assert "args_" + command in names and "result_" + command in names, command
    assert "match_playback" in app["x-command-contracts"]
    assert "match_id" in app["$defs"]["ResolveEpisodeInput"]["required"]
    assert {"metadata_ref", "metadata_video_id", "match_id"} <= set(app["$defs"]["PlaybackContext"]["required"])
    assert {"selected_metadata_source", "selected_playback_source"} <= set(app["$defs"]["AppState"]["required"])
    assert app["x-stremio-capability-rule"]["resolve_resources"] == []
    stremio_branch = next(branch for branch in app["$defs"]["AppSource"]["oneOf"]
                         if branch["properties"]["kind"].get("const") == "stremio")
    assert stremio_branch["properties"]["capabilities"]["allOf"][1]["properties"]["resolve"]["const"] is False
    budgets = app["x-request-budgets"]
    assert budgets["transport_grace_ms"] >= budgets["aggregate_finalize_ms"] > 0
    for operation in budgets["operations"].values():
        assert 0 < operation["child_ms"] <= operation["total_ms"]
        assert 1 <= operation["max_concurrency"] <= budgets["public_metadata_slots"]

    operations = []
    for path, item in bridge["paths"].items():
        for method, operation in item.items():
            if method not in ("get", "head", "post", "delete", "options", "put", "patch"):
                continue
            operations.append(operation["operationId"])
            expected = [] if method == "options" else ([{"sessionBearer": []}, {"resourceCapability": []}]
                       if path.startswith(("/media/", "/images/")) else [{"sessionBearer": []}])
            assert operation.get("security", bridge["security"]) == expected
    assert len(operations) == len(set(operations))
    source_ids = set(re.findall(r"^\| (S\d+) \|", (ROOT / "references/SOURCES.md").read_text(), re.M))
    markdown_files = list(ROOT.rglob("*.md"))
    for path in markdown_files:
        text = path.read_text()
        assert text.count("```") % 2 == 0, path
        assert not re.search(r"GPUI|gpui", text), path
        for _, target in re.findall(r"\[([^\]]+)\]\(([^)]+)\)", text):
            if re.match(r"\w+://|#", target):
                continue
            assert (path.parent / target.split("#")[0]).exists(), (path, target)
        for source_id in re.findall(r"\[(S\d+)\](?!:)", text):
            assert source_id in source_ids, (path, source_id)

    task_text = (ROOT / "AI_TASKS.md").read_text()
    sections = re.split(r"(?m)^## ([TMN]\d{2})[^\n]*\n", task_text)
    dependencies = {}
    for index in range(1, len(sections), 2):
        task, body = sections[index:index + 2]
        match = re.search(r"^- 依赖：([^\n]+)", body, re.M)
        dependencies[task] = set(re.findall(r"[TMN]\d{2}", match.group(1))) if match else set()
    assert set(dependencies) == ({f"T{i:02}" for i in range(26)} | {f"N{i:02}" for i in range(3)}
                                 | {f"M{i:02}" for i in range(6)})
    edges = set(re.findall(r"([TMN]\d{2}) --> ([TMN]\d{2})", task_text))
    assert edges == {(parent, task) for task, parents in dependencies.items() for parent in parents}
    visiting, complete = set(), set()

    def visit(task):
        assert task not in visiting, task
        if task in complete:
            return
        visiting.add(task)
        for parent in dependencies[task]:
            visit(parent)
        visiting.remove(task)
        complete.add(task)

    for task in dependencies:
        visit(task)
    assert "T02" in dependencies["T04"], "T04需要T02提供APK与最小health"
    def ancestors(task):
        parents = set(dependencies[task])
        for parent in dependencies[task]:
            parents.update(ancestors(parent))
        return parents

    for task in ("T05", "N00", "N01", "T06", "T07", "T08", "T09", "T13", "T14", "T15", "T16", "T17"):
        assert not ancestors(task) & {"T02", "T03", "T04", "N02"}, (task, "HTTP路径依赖Android任务")
    bodies = dict(zip(sections[1::2], sections[2::2]))
    for task in ("T06", "T07", "T08", "T09", "T10", "T11"):
        passed = re.search(r"^- 通过：([^\n]+)", bodies[task], re.M).group(1)
        assert not re.search(r"SuperVod|Ddys|Yjys|7包|原interceptor", passed), (task, "HTTP验收包含后续原包要求")
    assert "真实Ddys/SuperVod" in task_text and "在N02验收" in task_text
    assert "T18" in ancestors("T20"), "历史端到端验收需要T18的SQLite实现"
    assert "N02" in ancestors("T20"), "最终验收必须包含Cloudstream真实集成"
    assert {"T06", "T14"} <= ancestors("T17"), "T17真实首页验收需要首页数据及界面"
    assert {"N02", "T12"} <= ancestors("T21"), "T21需要Android接入及正式播放器"
    assert "真实人机验证响应的采样与判定在N02验收" in bodies["T17"]
    assert "采集真实源站人机验证响应" in bodies["N02"]
    spec = (ROOT / "DEVELOPMENT_SPEC.md").read_text()
    budget_rows = {
        "默认元数据/详情/搜索/HTTP单栏请求": "default_metadata",
        "Cloudstream首页第1页": "cloudstream_home",
        "Cloudstream单栏请求（含首屏重试）": "cloudstream_home_page",
        "MacCMS首页第1页": "maccms_home",
        "Stremio首页第1页": "stremio_home",
        "HTTP来源同步（含全部manifest）": "http_source_sync",
        "resolve（MacCMS/Cloudstream）": "resolve",
        "match_playback": "match_playback",
        "health / get_resource_status": "health",
        "runtime_prepare": "runtime_prepare",
    }
    for label, key in budget_rows.items():
        row = re.search(r"\| " + re.escape(label) + r" \| (\d+) \| (\d+) \|", spec)
        assert row, (key, "缺少请求预算文档行")
        assert tuple(map(int, row.groups())) == tuple(budgets["operations"][key][field]
                                                     for field in ("total_ms", "child_ms")), key
    assert budgets["operations"]["health"] == budgets["operations"]["get_resource_status"]
    setup = spec.split("### 5.2 第一次启动", 1)[1].split("### 5.3", 1)[0]
    assert setup.index("host_epoch") < setup.index("Cloudstream按需准备")
    assert (setup.index("sys.boot_completed=1") < setup.index("安装固定引擎 APK")
            < setup.index("BootstrapActivity") < setup.index("转发") < setup.index("健康检查")), "首启步骤顺序错误"
    fields = re.search(r"AppState拥有：([^。]+)", spec).group(1).split("、")
    assert fields == list(app["$defs"]["AppState"]["properties"]), "AppState文档字段与schema不一致"
    evidence = read("references/app-data-evidence.json")
    artifact_root = args.artifact_root.resolve(strict=True) if args.artifact_root is not None else None
    if artifact_root is not None:
        assert artifact_root.is_dir(), "--artifact-root必须指向提取后的ASAR目录"
    original_verified = 0
    for record in evidence["records"]:
        relative = Path(record["artifact_path"])
        assert not relative.is_absolute() and ".." not in relative.parts, record["id"]
        assert re.fullmatch(r"[a-f0-9]{64}", record["sha256"]), record["id"]
        assert 0 <= record["byte_start"] <= record["byte_end"], record["id"]
        assert record["byte_end"] - record["byte_start"] == len(record["snippet"].encode("utf-8")), record["id"]
        if artifact_root is not None:
            source = (artifact_root / relative).resolve(strict=True)
            assert source.is_relative_to(artifact_root), record["id"]
            raw = source.read_bytes()
            assert hashlib.sha256(raw).hexdigest() == record["sha256"], record["id"]
            assert raw[record["byte_start"]:record["byte_end"]].decode("utf-8") == record["snippet"], record["id"]
            original_verified += 1
    inventory = read("references/plugin-artifact-inventory.json")
    assert len(inventory["plugins"]) == 7
    assert hashlib.sha256((ROOT / "references/plugins.snapshot.json").read_bytes()).hexdigest() == inventory["registry_sha256"]
    example = fixture["identity_example"]
    registration = example["registration_input"]
    assert example["registration_key"] == digest("registration", registration["qualified_provider_class"],
                                                  registration["initial_main_url"], registration["duplicate_ordinal"])
    assert example["source_id"] == digest("source", example["plugin_internal_name"], example["registration_key"])
    assert example["source_revision"] == digest("revision", example["plugin_sha256"],
                                                 example["cloudstream_commit"], example["mapping_version"])
    assert example["content_id"] == digest("content", example["source_id"], example["original_content_data"])
    group, episode, section = example["group_input"], example["episode_input"], example["section_input"]
    assert example["group_id"] == digest("group", example["content_id"], example["source_revision"], group["group_index"],
                                         group["title"], group["kind"], group["season_number_or_empty"], group["dub_status"])
    assert example["episode_id"] == digest("episode", example["content_id"], example["source_revision"], example["group_id"],
                                           episode["episode_index"], episode["original_episode_data"])
    assert example["section_id"] == digest("section", example["source_id"], example["source_revision"], section["mainpage_name"],
                                           section["mainpage_data"], section["duplicate_section_ordinal"])
    for http in fixture["http_identity_examples"]:
        assert http["source_id"] == digest("http_source", http["kind"], http["config_id"])
        assert http["source_revision"] == digest(http["kind"] + "_revision", *http["revision_parts"])
        assert http["section_id"] == digest(http["kind"] + "_section", *http["section_parts"])
    for vector in fixture["metadata_identity_vectors"]:
        assert vector["addon_key"] == digest("addon", vector["manifest_url"])
        assert vector["content_id"] == digest("metadata", vector["namespace"], vector["type"], vector["canonical_id"])
    assert digest("metadata", "imdb", "movie", "tt1234567") != digest("metadata", "imdb", "series", "tt1234567")
    assert digest("metadata", "addon:" + "a" * 64, "movie", "42") != digest("metadata", "addon:" + "b" * 64, "movie", "42")
    stremio = fixture["protocol_vectors"]["stremio"]
    for manifest, payload in zip(stremio["manifests"], stremio["manifest_payloads"]):
        assert json.loads(payload) == manifest
    inline = stremio["inline_only"]
    resources = inline["manifest"]["resources"]
    assert "stream" not in resources and "meta" in resources
    assert semantic({"kind": "stremio_resolve_capability", "value": inline["manifest"]}) == inline["expected_resolve_capability"]
    video = inline["metadata"]["meta"]["videos"][0]
    assert video["id"] == inline["expected_video_id"]
    assert video["streams"][0]["url"] == inline["expected_stream_url"]
    stremio_identity = next(item for item in fixture["http_identity_examples"] if item["kind"] == "stremio")
    assert stremio_identity["revision_parts"][2::2] == [hashlib.sha256(payload.encode("utf-8")).hexdigest()
                                                       for payload in stremio["manifest_payloads"]]
    not_run = ["original application execution", "application builds", "Android plugin execution",
               "live provider requests", "desktop and mobile playback/packaging"]
    if artifact_root is None:
        not_run.append("original artifact byte verification (enable with --artifact-root)")
    report = {
        "date": datetime.date.today().isoformat(), "spec_version": bridge["info"]["version"], "stack": "Tauri 2 + Rust; Vue 3 + TypeScript + Vite",
        "openapi": "OpenAPI 3.1 validator passed; schema refs, operation IDs, auth, and examples checked",
        "paths": len(bridge["paths"]), "operations": len(operations), "bridge_schemas": len(bridge["components"]["schemas"]),
        "app_definitions": len(app["$defs"]), "tauri_commands": len(app["x-command-contracts"]), "schema_refs_checked": ref_count,
        "embedded_examples_validated": embedded_examples,
        "embedded_source_bindings": "all embedded examples and positive fixtures checked recursively",
        "structural_cases": len(fixture["cases"]), "structural_positive": sum(c["expected_valid"] for c in fixture["cases"]),
        "structural_negative": sum(not c["expected_valid"] for c in fixture["cases"]), "semantic_cases": len(fixture["semantic_cases"]),
        "lifecycle_vectors": len(fixture["lifecycle_vectors"]),
        "task_cards": {"desktop": 29, "new_source_tasks": 3, "mobile_follow_up": 6},
        "task_dependency_graph": "acyclic; card/diagram edges match; HTTP milestone independent of Android tasks",
        "markdown_files": len(markdown_files), "static_app_evidence_snippets": len(evidence["records"]),
        "original_artifact_verification": {
            "status": "verified" if artifact_root is not None else "not_requested",
            "snippets_verified": original_verified,
        },
        "document_invariants": "lazy Android startup, T02->T04 and native independence, AppState names, seven Cloudstream and six HTTP identity hashes checked",
        "architecture_contract_checks": {
            "cancel_identity": "all positive command examples recursively checked; self-target negative vector checked",
            "request_budgets": "machine values match operation table; parent/child/grace bounds checked",
            "http_task_acceptance": "T06-T11 pass conditions exclude deferred original-provider checks",
            "policy_vectors": "publication generation/sequence, session ownership, manifest commit, public peer binding, resource response identity",
            "scope": "authored contracts and policy predicates; runtime fault-injection remains in task cards",
            "playback_binding": "metadata/playback source roles, cross-source match and provider resolution/session binding checked",
            "history": "canonical identity, metadata origin and progress bounds checked; SQLite runtime deferred to T18",
            "home_sections": "original index uniqueness, scoped response and retry merge vectors checked",
            "lifecycle": "line switch, late close/release, explicit release and stop-before-plugin-commit vectors checked",
            "integration_prerequisites": "T20 includes SQLite T18; T17 includes homepage; T21 includes N02/T12; challenge sampling assigned to N02",
            "inline_streams": "Stremio media ignored; metadata role resolve=false; original protocol fixture retained",
        },
        "plugins": "7 artifact inventories retained; pinned registry snapshot hash checked",
        "validation_dependencies": {name: importlib.metadata.version(name) for name in ("openapi-spec-validator", "jsonschema")},
        "not_run": not_run,
    }
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
