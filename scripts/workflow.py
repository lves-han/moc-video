#!/usr/bin/env python3
"""Small, local agent workflow. TASKS.md is the only task specification."""
import argparse
import datetime as dt
import hashlib
import json
import math
import os
import re
import shlex
import shutil
import signal
import stat
import subprocess
import sys
import time
import uuid
import xml.etree.ElementTree as ET
from pathlib import Path, PurePosixPath, PureWindowsPath

sys.path.insert(0, str(Path(__file__).resolve().parent))
import platform_runtime as runtime

WorkflowError = runtime.WorkflowError


class CompletionEvidenceError(WorkflowError):
    """Successful current evidence is missing; the card can be revalidated."""


class ReviewBlocked(WorkflowError):
    """The latest independent review did not clear the current commit."""


REPO = Path(__file__).resolve().parents[1]
APPEND_ONLY = ("docs/PROGRESS.md", "docs/DECISIONS.md", "docs/BACKLOG.md")
# Review reports are committed records; like progress they never count as tested source.
DYNAMIC = (*APPEND_ONLY, "docs/TASKS.md", "docs/reviews/")


def is_dynamic(name):
    return any(name == p or (p.endswith("/") and name.startswith(p)) for p in DYNAMIC)
PROTECTED = ("AGENTS.md", "CLAUDE.md", "docs/SPEC.md", "docs/ARCHITECTURE.md", "docs/TASKS.md",
             "workflow.json", ".gitattributes", "scripts/workflow.py", "scripts/platform_runtime.py", "scripts/windows_job.py", "scripts/task.py",
             "scripts/run.py", "scripts/verify.py", "scripts/drive.py", "scripts/validate.py", "scripts/requirements.txt",
             ".github/workflows/script-portability.yml",
             "scripts/tests/", "docs/PROMPT-")


def utc():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def read_json(path, default=None):
    if not path.exists():
        return {} if default is None else default
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (ValueError, OSError) as error:
        raise WorkflowError(f"损坏或不可读的状态文件：{path}: {error}") from error


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + f".{os.getpid()}.tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    os.replace(temporary, path)


def digest(value):
    if not isinstance(value, bytes):
        value = json.dumps(value, ensure_ascii=True, sort_keys=True).encode()
    return hashlib.sha256(value).hexdigest()


def git(*args, binary=False, check=True):
    result = subprocess.run(["git", "-C", str(REPO), *args], capture_output=True)
    if check and result.returncode:
        raise WorkflowError(result.stderr.decode(errors="replace").strip() or "Git 命令失败")
    return result.stdout if binary else result.stdout.decode("utf-8", errors="surrogateescape").strip()


def relative_path(text):
    path = PurePosixPath(text)
    if not text or "\\" in text or PureWindowsPath(text).drive or path.is_absolute() or ".." in path.parts or text in (".", "./"):
        raise WorkflowError(f"需要项目内相对路径：{text}")
    return Path(path)


def validate_checks(checks, acceptance=False):
    if not isinstance(checks, list) or not checks or (acceptance and len(checks) > 6):
        raise WorkflowError("验收/验证必须是非空数组；单卡最多6项")
    ids = set()
    for index, item in enumerate(checks, 1):
        if not isinstance(item, dict):
            raise WorkflowError("每项验收必须是对象")
        name = item.get("id", "")
        if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9._-]+", name) or name in ids:
            raise WorkflowError("验收ID须唯一且只含字母、数字、点、下划线、连字符")
        if acceptance and name != f"A{index}":
            raise WorkflowError("单卡验收ID必须按 A1、A2……排列")
        argv = item.get("argv")
        if not isinstance(argv, list) or not argv or not all(isinstance(a, str) and a and "\0" not in a for a in argv):
            raise WorkflowError(f"{name}: argv 必须为非空字符串数组")
        if argv[0] in ("true", "echo", "printf"):
            raise WorkflowError(f"{name}: 不能用打印或 true 冒充验收")
        if item.get("kind") not in ("command", "test"):
            raise WorkflowError(f"{name}: kind 须为 command 或 test")
        report = item.get("report", "")
        if item["kind"] == "test":
            if not isinstance(report, str) or not (report in ("unittest", "cargo", "pytest") or report.startswith("junit:")):
                raise WorkflowError(f"{name}: 测试须声明有效 report")
            if report.startswith("junit:"):
                path = relative_path(report[6:])
                if not path.as_posix().startswith("evidence/"):
                    raise WorkflowError("JUnit 报告须位于被忽略的 evidence/ 内")
        cases, skippable = item.get("cases"), item.get("skippable")
        if cases is not None:
            # Frozen case IDs let the controller, not the script author, decide coverage.
            if not acceptance or not str(report).startswith("junit:"):
                raise WorkflowError(f"{name}: cases 只用于卡片 JUnit 验收")
            if (not isinstance(cases, list) or not cases or len(set(cases)) != len(cases)
                    or not all(isinstance(c, str) and re.fullmatch(r"[A-Za-z0-9._/:-]+", c) for c in cases)):
                raise WorkflowError(f"{name}: cases 须为唯一用例 ID 的非空数组")
            if skippable is not None and (not isinstance(skippable, list) or not set(skippable) <= set(cases)):
                raise WorkflowError(f"{name}: skippable 须为 cases 的子集")
        elif skippable is not None:
            raise WorkflowError(f"{name}: skippable 需配合 cases")
        timeout = item.get("timeout_seconds", 1)
        if type(timeout) is not int or timeout <= 0:
            raise WorkflowError(f"{name}: timeout_seconds 须为正整数")
        if acceptance and not str(item.get("description", "")).strip():
            raise WorkflowError(f"{name}: 缺少可观察验收描述")
        ids.add(name)


def config():
    cfg = read_json(REPO / "workflow.json")
    for key in ("session_seconds", "idle_seconds", "max_sessions", "poll_seconds",
                "command_seconds", "review_seconds", "memory_mib", "jobs"):
        if type(cfg.get(key)) is not int or cfg[key] <= 0:
            raise WorkflowError(f"workflow.json: {key} 须为正整数")
    if cfg.get("heavy_lock") is not None and (not isinstance(cfg["heavy_lock"], str) or not cfg["heavy_lock"]):
        raise WorkflowError("workflow.json: heavy_lock 须为空或非空字符串")
    if cfg.get("heavy_lock") and not Path(cfg["heavy_lock"]).is_absolute():
        raise WorkflowError("workflow.json: heavy_lock 须为共享锁的绝对路径")
    for key in ("agent_command", "review_command"):
        command = cfg.get(key)
        if command is not None and (not isinstance(command, list) or not command
                                    or not all(isinstance(a, str) and a and "\0" not in a for a in command)):
            raise WorkflowError(f"workflow.json: {key} 须为 null 或非空 argv 数组")
    validate_checks(cfg.get("verify"))
    if any(not re.fullmatch(r"verify-[A-Za-z0-9._-]+", item["id"]) for item in cfg["verify"]):
        raise WorkflowError("workflow.json: 全量验证 ID 必须使用 verify- 前缀，与卡片 A1、A2 分开")
    if not any(item["kind"] == "test" for item in cfg["verify"]):
        raise WorkflowError("verify 至少包含一个要求非零测试的入口")
    return cfg


def cards(text=None):
    text = (REPO / "docs/TASKS.md").read_text(encoding="utf-8") if text is None else text
    headings = list(re.finditer(r"^### (T\d+\.\d+) ([^\n]+)$", text, re.M))
    result, seen = [], set()
    for index, heading in enumerate(headings):
        body = text[heading.start():headings[index + 1].start() if index + 1 < len(headings) else len(text)].rstrip()
        cid = heading.group(1)
        if cid in seen:
            raise WorkflowError(f"重复卡号：{cid}")
        checkbox = re.findall(r"^- \[([ x])\] 完成$", body, re.M)
        dependencies = re.search(r"^- 依赖：([^\n]+)$", body, re.M)
        scope = re.search(r"^- 范围：([^\n]+)$", body, re.M)
        gate = re.search(r"^- 阶段门：([^\n]*)$", body, re.M)
        if gate and not gate.group(1).strip():
            raise WorkflowError(f"{cid}: 阶段门须写明需要的人类授权")
        environment = re.search(r"^- 环境：([^\n]*)$", body, re.M)
        if environment and not environment.group(1).strip():
            raise WorkflowError(f"{cid}: 环境须写明必需与非阻断的执行环境")
        blocks = re.findall(r"^```json\s*\n(.*?)\n```\s*$", body, re.M | re.S)
        if len(checkbox) != 1 or not dependencies or not scope or len(blocks) != 1 or "- 交付物：" not in body:
            raise WorkflowError(f"{cid}: 卡片字段或唯一 JSON 验收块不完整")
        deps = re.findall(r"T\d+\.\d+", dependencies.group(1))
        if any(dep not in seen for dep in deps):
            raise WorkflowError(f"{cid}: 依赖须是前面已定义的卡")
        paths = re.findall(r"`([^`]+)`", scope.group(1))
        if not paths:
            raise WorkflowError(f"{cid}: 范围必须包含反引号包围的相对文件/目录")
        for path in paths:
            relative_path(path)
        try:
            checks = json.loads(blocks[0])
        except ValueError as error:
            raise WorkflowError(f"{cid}: 验收 JSON 无效：{error}") from error
        validate_checks(checks, acceptance=True)
        normalized = re.sub(r"^- \[[ x]\] 完成$", "- [ ] 完成", body, flags=re.M)
        result.append(dict(id=cid, title=heading.group(2), body=body, done=checkbox[0] == "x",
                           deps=deps, scope=paths, checks=checks, spec=digest(normalized),
                           gate=gate.group(1).strip() if gate else None,
                           environment=environment.group(1).strip() if environment else None))
        seen.add(cid)
    reports = [check["report"][6:] for item in result for check in item["checks"] if check.get("report", "").startswith("junit:")]
    if len(reports) != len(set(reports)):
        # A shared path would let one card's fresh report stand in for another check.
        raise WorkflowError("JUnit 报告路径须在全部卡片验收中唯一；建议 evidence/acceptance/<卡号>-<验收ID>.xml")
    return result


def get_card(cid):
    for item in cards():
        if item["id"] == cid:
            return item
    raise WorkflowError(f"未知任务卡：{cid}")


def card_dir(cid):
    return REPO / "evidence/task-state" / cid


def current():
    path = REPO / "evidence/task-state/current"
    return path.read_text(encoding="utf-8").strip() if path.exists() else None


def state(cid):
    return read_json(card_dir(cid) / "state.json")


def progress_entries():
    return ["## " + part for part in re.split(r"^## ", (REPO / "docs/PROGRESS.md").read_text(encoding="utf-8"), flags=re.M)[1:]]


def blocked():
    pending = {}
    for entry in progress_entries():
        status = re.search(r"^- 结果：\s*(BLOCKED|RESOLVED)\s*$", entry, re.M)
        if status:
            ids = re.findall(r"T\d+\.\d+", entry.splitlines()[0]) or ["global"]
            for cid in ids:
                if status.group(1) == "BLOCKED":
                    pending[cid] = entry
                else:
                    pending.pop(cid, None)
    return pending


def global_blocks(pending, items):
    """BLOCKED without a known card ID in its heading stops every card."""
    known = {item["id"] for item in items}
    return [cid for cid in pending if cid not in known]


def authorized(cid):
    for entry in progress_entries():
        if (cid in re.findall(r"T\d+\.\d+", entry.splitlines()[0]) and re.search(r"^- 结果：AUTHORIZED\s*$", entry, re.M)
                and re.search(r"^- 授权：\S[^\n]*$", entry, re.M)):
            return True
    return False


def is_protected(name):
    return any(name == p or (p.endswith("/") and name.startswith(p)) or
               (p.endswith("-") and name.startswith(p)) for p in PROTECTED)


def protected_hash():
    values = {}
    paths = set()
    for name in PROTECTED:
        if name.endswith("/"):
            paths.update((REPO / name).rglob("*"))
        elif name.endswith("-"):
            paths.update((REPO / Path(name).parent).glob(Path(name).name + "*"))
        else:
            paths.add(REPO / name)
    for path in sorted(paths):
        name = path.relative_to(REPO).as_posix()
        if is_protected(name):
            if path.is_file() and "__pycache__" not in path.parts:
                content = path.read_bytes()
                if path.suffix.lower() in (".md", ".py", ".json", ".txt"):
                    content = content.replace(b"\r\n", b"\n")
                if name == "docs/TASKS.md":
                    content = re.sub(r"(?m)^- \[[ x]\] 完成$", "- [ ] 完成", content.decode("utf-8")).encode("utf-8")
                values[name] = digest(content)
    for name in PROTECTED:
        if not name.endswith(("/", "-")) and name not in values:
            values[name] = "MISSING"
    return digest(values)


def snapshot(commit=None):
    files = {}
    if commit:
        entries = git("ls-tree", "-r", "-z", commit, binary=True).split(b"\0")
        for entry in entries:
            if not entry:
                continue
            meta, name = entry.split(b"\t", 1)
            mode, kind, oid = meta.split()
            name = name.decode("utf-8", errors="surrogateescape")
            if is_dynamic(name):
                continue
            if kind != b"blob":
                raise WorkflowError("源码快照暂不支持 Git submodule")
            files[name] = [mode.decode(), oid.decode()]
    else:
        index_modes = {}
        for entry in git("ls-files", "--stage", "-z", binary=True).split(b"\0"):
            if entry:
                metadata, name = entry.split(b"\t", 1)
                mode, _, stage = metadata.split()
                if stage != b"0":
                    raise WorkflowError("Git index 存在未解决的冲突")
                index_modes[name.decode("utf-8", errors="surrogateescape")] = mode.decode()
        honor_mode = git("config", "--get", "core.filemode", check=False) != "false"
        regular = []
        paths = set(git("ls-files", "-z", "--cached", "--others", "--exclude-standard", binary=True).split(b"\0"))
        for raw in sorted(paths):
            if not raw:
                continue
            name = raw.decode("utf-8", errors="surrogateescape")
            if is_dynamic(name):
                continue
            path = REPO / name
            if path.is_symlink():
                content = os.fsencode(os.readlink(path))
                oid = subprocess.run(["git", "-C", str(REPO), "hash-object", "--stdin"], input=content,
                                     capture_output=True, check=True).stdout.decode().strip()
                files[name] = ["120000", oid]
            elif path.is_file():
                mode = "100755" if path.stat().st_mode & stat.S_IXUSR else "100644"
                files[name] = [mode if honor_mode else index_modes.get(name, "100644"), None]
                regular.append(name)
            elif not path.exists():
                continue
            else:
                raise WorkflowError("源码快照暂不支持目录类型 Git submodule")
        if regular:
            names = "".join(json.dumps(name, ensure_ascii=False) + "\n" for name in regular).encode("utf-8", errors="surrogateescape")
            result = subprocess.run(["git", "-C", str(REPO), "hash-object", "--stdin-paths"],
                                    input=names, capture_output=True, check=True)
            hashes = result.stdout.decode().splitlines()
            if len(hashes) != len(regular):
                raise WorkflowError("源码快照缺少 Git clean 后的文件摘要")
            for name, oid in zip(regular, hashes):
                files[name][1] = oid
    return digest(files)


def latest_verify(cid, generation):
    values = [read_json(path) for path in sorted((REPO / "evidence/verify").glob("*.json"))]
    matching = [value for value in values if value.get("card") == cid and value.get("generation", 0) == generation]
    return matching[-1] if matching else {}


def completed(item):
    receipt = read_json(card_dir(item["id"]) / "done.json")
    node = state(item["id"])
    if not item["done"] or not receipt or receipt.get("spec") != item["spec"] or receipt.get("generation", 0) != node.get("generation", 0):
        return False
    rows = git("log", "--reverse", "--format=%H %s", receipt["head"] + "..HEAD").splitlines()
    for row in rows:
        sha, subject = row.split(" ", 1)
        if subject.startswith(item["id"] + ":") and snapshot(sha) == receipt["source"]:
            committed = next((card for card in cards(git("show", sha + ":docs/TASKS.md")) if card["id"] == item["id"]), None)
            progress = git("show", sha + ":docs/PROGRESS.md", binary=True).replace(b"\r\n", b"\n")
            prefix = receipt["progress"]
            if committed and committed["done"] and committed["spec"] == receipt["spec"] and digest(progress[:prefix["size"]]) == prefix["hash"]:
                # Earlier cards retain the gates/source they actually completed under.
                # Later cards may legitimately change source or an authorized baseline.
                gates = json.loads(git("show", sha + ":workflow.json"))["verify"]
                try:
                    completion_evidence(item, receipt["source"], node.get("protected"), node.get("generation", 0), gates)
                except CompletionEvidenceError:
                    return False
                return True
    return False


def check_completion_tree(cid):
    receipt = read_json(card_dir(cid) / "done.json")
    if git("status", "--porcelain") or snapshot() != receipt.get("source"):
        raise WorkflowError("完成提交后存在未验证改动；保留工作树，停止推进并重新验收")
    if any(run["status"] != "finished" for run in receipts()):
        raise WorkflowError("存在未完成的重命令，不能报告全部完成")


def frozen_cards(items, pending, verified):
    """Cards held by an unresolved BLOCKED or a phase gate, plus everything depending on them."""
    frozen = {cid: "BLOCKED" for cid in pending}
    for item in items:
        if item["gate"] and item["id"] not in verified and not authorized(item["id"]):
            frozen[item["id"]] = "阶段门待人类授权：" + item["gate"]
    for item in items:
        held = next((dep for dep in item["deps"] if dep in frozen), None)
        if held and item["id"] not in frozen:
            frozen[item["id"]] = f"依赖 {held}"
    return frozen


def next_card():
    items = cards()
    if not items:
        raise WorkflowError("DRAFT：尚无可执行任务卡", 12)
    pending = blocked()
    hard = global_blocks(pending, items)
    if hard:
        raise WorkflowError("BLOCKED：存在未关联已知卡号的全局决定，所有卡暂停", 11)
    verified = {item["id"] for item in items if completed(item)}
    active = current()
    if active in verified:
        check_completion_tree(active)
    frozen = frozen_cards(items, pending, verified)
    if active and active not in verified and state(active):
        item = next(value for value in items if value["id"] == active)
        if active not in frozen and all(dep in verified for dep in item["deps"]):
            return item
        if active in frozen and (git("status", "--porcelain") or any(run["status"] != "finished" for run in receipts())):
            raise WorkflowError(f"{active} 已阻塞但仍有未提交改动或未完成重命令；先提交 WIP 并交接，才能转向其他卡", 11)
    for item in items:
        if item["id"] not in verified and item["id"] not in frozen and all(dep in verified for dep in item["deps"]):
            return item
    if len(verified) == len(items) and not pending:
        check_completion_tree(items[-1]["id"])
        raise WorkflowError("ALL_DONE", 10)
    held = [f"{cid}（{why}）" for cid, why in frozen.items() if cid not in verified or cid in pending]
    if held:
        raise WorkflowError("BLOCKED：没有可执行的卡；" + "；".join(held), 11)
    raise WorkflowError("没有依赖已完成的任务卡", 12)


def start(cid):
    item = next_card()
    if item["id"] != cid:
        raise WorkflowError(f"当前卡是 {item['id']}，不能开始 {cid}")
    previous = current()
    if previous and previous != cid and state(previous) and not completed(get_card(previous)):
        # next_card only moves away from a frozen card with a clean tree; close its scope segment at HEAD.
        old = state(previous)
        old.setdefault("segments", []).append(dict(base=old.pop("resume_base", old["base"]), end=git("rev-parse", "HEAD")))
        old["paused"] = True
        write_json(card_dir(previous) / "state.json", old)
    node = state(cid)
    if node.get("paused"):
        node.update(paused=False, resume_base=git("rev-parse", "HEAD"))
        write_json(card_dir(cid) / "state.json", node)
    if not node:
        head = git("rev-parse", "HEAD")
        prefixes = {}
        for name in APPEND_ONLY:
            value = (REPO / name).read_bytes()
            prefixes[name] = {"size": len(value), "hash": digest(value)}
        node = dict(base=head, protected=protected_hash(), append_only=prefixes,
                    sessions=0, failures=[], last_note=0, started=utc())
        write_json(card_dir(cid) / "state.json", node)
    check_frozen(cid)
    path = REPO / "evidence/task-state/current"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(cid + "\n", encoding="utf-8", newline="\n")
    return item


def check_frozen(cid):
    node = state(cid)
    if not node:
        raise WorkflowError("先使用 task.py start")
    if protected_hash() != node["protected"]:
        raise WorkflowError("任务正文、契约、项目规则或控制脚本发生未授权变动；需用户决定")
    check_append_only(node)


def check_append_only(node):
    for name, prefix in node["append_only"].items():
        data = (REPO / name).read_bytes()
        if digest(data[:prefix["size"]]) != prefix["hash"]:
            raise WorkflowError(f"只追加文档被改写：{name}")


def rebaseline(cid, frozen, include_paused=None):
    if include_paused is not None:
        return rebaseline_paused(cid, frozen, include_paused)
    node = state(cid)
    if current() != cid or not node:
        raise WorkflowError("只可为已开始的当前卡更新授权基线")
    config()
    cards()
    if frozen != protected_hash() or frozen == node["protected"]:
        raise WorkflowError("批准摘要与当前修订不一致，或冻结内容没有变化")
    check_append_only(node)
    items = cards()
    pending = blocked()
    if cid in pending or global_blocks(pending, items):
        raise WorkflowError("仍有未解除的 BLOCKED，不能更新基线", 11)
    others = [item["id"] for item in items if item["id"] != cid and state(item["id"]) and not completed(item)]
    if others:
        raise WorkflowError("其他已开始未完成的卡会因基线变化失效：" + ", ".join(others) + "；先完成它们或交用户决定")
    decisions = [entry for entry in progress_entries() if cid in re.findall(r"T\d+\.\d+", entry.splitlines()[0])
                 and re.search(r"^- 结果：(BLOCKED|RESOLVED)\s*$", entry, re.M)]
    if len(decisions) < 2 or not re.search(r"^- 结果：BLOCKED\s*$", decisions[-2], re.M):
        raise WorkflowError("需要本卡 BLOCKED 后的用户 RESOLVED 决定")
    approval = decisions[-1]
    if not re.search(r"^- 结果：RESOLVED\s*$", approval, re.M) or not re.search(r"^- 决定：\S[^\n]*$", approval, re.M) or not re.search(r"^- 批准摘要：" + re.escape(frozen) + r"\s*$", approval, re.M):
        raise WorkflowError("RESOLVED 必须包含真实用户决定与匹配的批准摘要")
    approval_hash = digest(approval)
    if any(event["approval"] == approval_hash for event in node.get("rebaselines", [])):
        raise WorkflowError("此用户决定已经使用；不能重复更新基线")
    driver = read_json(REPO / "evidence/drive/state.json")
    if driver.get("pid") and identity(driver["pid"]) == driver.get("identity"):
        raise WorkflowError("先停止驱动器，再更新授权基线")
    if any(run["status"] != "finished" for run in receipts()) or heavy_active():
        raise WorkflowError("先等待或恢复未完成重命令，再更新授权基线")
    outside = scope_problems(cid)
    if any(not is_protected(name) for name in outside):
        raise WorkflowError("存在非冻结文件的范围外修改；不能借更新基线放宽范围")
    event = dict(time=utc(), previous=node["protected"], protected=frozen, approval=approval_hash)
    node.setdefault("rebaselines", []).append(event)
    node.update(protected=frozen, generation=node.get("generation", 0) + 1,
                rebaseline_scope=sorted(set(node.get("rebaseline_scope", [])) | set(outside)))
    for name in APPEND_ONLY:
        value = (REPO / name).read_bytes()
        node["append_only"][name] = dict(size=len(value), hash=digest(value))
    write_json(card_dir(cid) / "state.json", node)
    print(f"{cid} 授权基线已更新；保留额度及失败历史，旧验收证据失效，须重新完整验收")


def rebaseline_paused(cid, frozen, include_paused):
    # One explicit approval covers this set; interrupted per-file saves can resume
    # from the existing rebaseline events without a separate transaction state.
    if not include_paused or len(set(include_paused)) != len(include_paused) or cid in include_paused:
        raise WorkflowError("迁移暂停卡集合必须非空、无重复且不含当前卡")
    cfg = config()
    with runtime.FileLock(REPO / "evidence/drive/driver.lock") as driver_lock, \
            runtime.FileLock(runtime.lock_path(cfg)) as heavy_lock:
        if not driver_lock.acquire() or not heavy_lock.acquire():
            raise WorkflowError("驱动器或重任务锁仍被占用，不能迁移基线")
        items = cards()
        targets = [cid, *sorted(include_paused)]
        unfinished = {item["id"] for item in items if state(item["id"]) and not completed(item)}
        if set(targets) != unfinished or current() != cid:
            raise WorkflowError("批准迁移集合须准确覆盖当前及全部暂停未完成卡")
        nodes = {key: state(key) for key in targets}
        if nodes[cid].get("paused") or any(not nodes[key].get("paused") for key in include_paused):
            raise WorkflowError("迁移要求当前卡活跃、其余卡保持暂停")
        if frozen != protected_hash():
            raise WorkflowError("批准摘要与当前修订不一致")
        pending = blocked()
        if cid in pending or global_blocks(pending, items):
            raise WorkflowError("当前卡仍有未解除的 BLOCKED", 11)
        decisions = [entry for entry in progress_entries() if cid in re.findall(r"T\d+\.\d+", entry.splitlines()[0])
                     and re.search(r"^- 结果：(BLOCKED|RESOLVED)\s*$", entry, re.M)]
        if len(decisions) < 2 or not re.search(r"^- 结果：BLOCKED\s*$", decisions[-2], re.M):
            raise WorkflowError("需要当前卡 BLOCKED 后的用户 RESOLVED 决定")
        approval = decisions[-1]
        included = re.search(r"^- 迁移暂停卡：([^\n]+)$", approval, re.M)
        if not re.search(r"^- 结果：RESOLVED\s*$", approval, re.M) or not re.search(r"^- 决定：\S[^\n]*$", approval, re.M) \
                or not re.search(r"^- 批准摘要：" + re.escape(frozen) + r"\s*$", approval, re.M) \
                or not included or included[1] != ",".join(sorted(include_paused)):
            raise WorkflowError("RESOLVED 必须包含真实决定、批准摘要与准确迁移暂停卡集合")
        approval_hash = digest(approval)
        driver = read_json(REPO / "evidence/drive/state.json")
        if any(runtime.alive(dict(pid=driver.get(pid), identity=driver.get(token))) for pid, token in
               (("pid", "identity"), ("agent_pid", "agent_identity"))):
            raise WorkflowError("先停止驱动器及开发进程，再迁移基线")
        runs = receipts()
        if any(run["status"] != "finished" or
               any(runtime.alive(ref) for ref in [
                   dict(pid=run.get("pid"), identity=run.get("identity")),
                   dict(pid=run.get("payload_pid"), identity=run.get("payload_identity")),
                   *run.get("payload_refs", [])]) for run in runs):
            raise WorkflowError("先等待或恢复未完成重命令，再迁移基线")
        previous, updates = set(), {}
        for key, node in nodes.items():
            check_append_only(node)
            outside = scope_problems(key)
            if any(not is_protected(name) for name in outside):
                raise WorkflowError(f"{key} 存在非冻结文件的范围外修改")
            used = [event for event in node.get("rebaselines", []) if event["approval"] == approval_hash]
            if used:
                if len(used) != 1 or node["protected"] != frozen or used[0]["protected"] != frozen \
                        or used[0].get("cards") != targets or used[0]["previous"] == frozen:
                    raise WorkflowError("已使用批准事件与本次迁移不一致")
                previous.add(used[0]["previous"])
                continue
            if node["protected"] == frozen:
                raise WorkflowError("冻结内容没有变化，且无本次已迁移事件")
            previous.add(node["protected"])
            node.setdefault("rebaselines", []).append(dict(time=utc(), previous=node["protected"],
                protected=frozen, approval=approval_hash, cards=targets))
            node.update(protected=frozen, generation=node.get("generation", 0) + 1,
                        rebaseline_scope=sorted(set(node.get("rebaseline_scope", [])) | set(outside)))
            for name in APPEND_ONLY:
                value = (REPO / name).read_bytes()
                node["append_only"][name] = dict(size=len(value), hash=digest(value))
            updates[key] = node
        if len(previous) != 1:
            raise WorkflowError("迁移集合的原冻结基线不一致")
        for key, node in updates.items():
            write_json(card_dir(key) / "state.json", node)
        print(f"{','.join(targets)} 基线迁移完成；本次更新 {len(updates)} 卡，历史与暂停状态保持")


def scope_problems(cid):
    item, node = get_card(cid), state(cid)
    # A paused card owns only the commits made while it was active, not other cards' work in between.
    paths = set()
    for segment in node.get("segments", []):
        paths |= set(git("diff", "--no-renames", "--name-only", "-z", segment["base"], segment["end"], binary=True).split(b"\0"))
    working_base = "HEAD" if node.get("paused") else node.get("resume_base", node["base"])
    paths |= set(git("diff", "--no-renames", "--name-only", "-z", working_base, binary=True).split(b"\0"))
    paths |= set(git("ls-files", "--others", "--exclude-standard", "-z", binary=True).split(b"\0"))
    allowed = [*item["scope"], *DYNAMIC, *node.get("rebaseline_scope", [])]
    return sorted(p.decode("utf-8", errors="surrogateescape") for p in paths if p and not any(
        p.decode("utf-8", errors="surrogateescape") == name or (name.endswith("/") and p.decode("utf-8", errors="surrogateescape").startswith(name)) for name in allowed))


def receipts(cid=None):
    result = []
    for path in sorted((REPO / "evidence/runs").glob("*/receipt.json")):
        value = read_json(path)
        if cid is None or value.get("card") == cid:
            value["path"] = path.relative_to(REPO).as_posix()
            result.append(value)
    return sorted(result, key=lambda item: item["created"])


def verify_evidence(item):
    path = REPO / item["log"]
    if not path.is_file() or digest(path.read_bytes()) != item.get("log_hash"):
        raise WorkflowError(f"日志缺失或被改写：{path}；保留现场并重新验收")
    if item.get("status") != "finished" or item.get("exit") != 0:
        raise WorkflowError(f"运行未成功：{item['id']}")
    if item["check"]["kind"] == "test" and (not item.get("tests") or item["tests"]["passed"] < 1 or item["tests"]["failed"]):
        raise WorkflowError("缺少非零成功测试证据")
    if item.get("report_artifact"):
        artifact = REPO / item["report_artifact"]
        if not artifact.is_file() or digest(artifact.read_bytes()) != item["report_hash"]:
            raise WorkflowError("测试报告缺失或被改写；保留现场并重新验收")
    if item["check"].get("cases"):
        problems = case_problems(item["check"], (REPO / item["report_artifact"]).read_bytes()) if item.get("report_artifact") else ["缺少 JUnit 报告"]
        if problems:
            raise WorkflowError("验收用例证据不完整：" + "；".join(problems[:5]))


def completion_evidence(item, source, frozen, generation, gates):
    """Validate latest acceptance and every gate, for done and later completion checks."""
    cid, acceptance = item["id"], []

    def validate_run(run, check):
        if run.get("status") != "finished" or run.get("exit") != 0:
            raise CompletionEvidenceError(f"最近运行未成功：{check['id']}；须重新验收")
        if (run.get("card") != cid or run.get("id") != check["id"] or run.get("argv") != check["argv"]
                or run.get("check") != check or run.get("source") != source or run.get("after_source") != source
                or run.get("spec") != item["spec"] or run.get("generation", 0) != generation or run.get("protected") != frozen):
            raise CompletionEvidenceError(f"{check['id']}: 验收不是当前源码及任务定义的证据")
        verify_evidence(run)

    runs = receipts(cid)
    for check in item["checks"]:
        matching = [run for run in runs if run["id"] == check["id"]]
        if not matching:
            raise CompletionEvidenceError(f"缺少真实运行：{check['id']}")
        latest = matching[-1]
        validate_run(latest, check)
        acceptance.append(latest)
    verification = latest_verify(cid, generation)
    if (verification.get("status") != "finished" or verification.get("exit") != 0
            or verification.get("source") != source or verification.get("protected") != frozen):
        raise CompletionEvidenceError("最近一次 verify.py all 尚未成功完成，或缺少当前源码的完整收据")
    paths = verification.get("runs")
    if (not isinstance(paths, list) or not paths or not all(isinstance(path, str) for path in paths)
            or len(paths) != len(gates) or len(set(paths)) != len(paths)):
        raise CompletionEvidenceError("最近一次 verify.py all 缺少完整门槛收据；须重新完整验证")
    for path, check in zip(paths, gates):
        run = read_json(REPO / relative_path(path))
        if not run:
            raise WorkflowError(f"完整验证的运行收据缺失：{path}；保留现场并重新完整验证")
        validate_run(run, check)
    return acceptance


def done(cid):
    if next_card()["id"] != cid or current() != cid:
        raise WorkflowError("只可完成当前卡")
    check_frozen(cid)
    item = get_card(cid)
    if not item["done"]:
        raise WorkflowError("先在全部验收后勾选完成")
    outside = scope_problems(cid)
    if outside:
        raise WorkflowError("范围外修改：" + ", ".join(outside))
    runs, source = receipts(cid), snapshot()
    if any(run["status"] != "finished" for run in runs):
        raise WorkflowError("存在未完成的重命令；先恢复或等待")
    entries = progress_entries()
    entry = entries[-1] if entries else ""
    if cid not in re.findall(r"T\d+\.\d+", entry.splitlines()[0] if entry else "") or not re.search(r"^- 结果：DONE\s*$", entry, re.M):
        raise WorkflowError("PROGRESS 最后一条须为本卡 DONE")
    rows = re.findall(r"^- 验收 (A\d+)：PASS；([^\n]+)$", entry, re.M)
    if [name for name, _ in rows] != [check["id"] for check in item["checks"]]:
        raise WorkflowError("PROGRESS 须按顺序逐项记录全部 A 验收")
    accepted = completion_evidence(item, source, protected_hash(), state(cid).get("generation", 0), config()["verify"])
    for check, latest, (_, row) in zip(item["checks"], accepted, rows):
        if latest["log"] not in re.findall(r"`([^`]+)`", row):
            raise WorkflowError(f"{check['id']}: PROGRESS 未引用本项准确日志")
    progress = (REPO / "docs/PROGRESS.md").read_bytes().replace(b"\r\n", b"\n")
    receipt = dict(card=cid, spec=item["spec"], source=source, generation=state(cid).get("generation", 0), head=git("rev-parse", "HEAD"), finished=utc(),
                   progress=dict(size=len(progress), hash=digest(progress)))
    write_json(card_dir(cid) / "done.json", receipt)
    print(f"{cid} 验证完成；现在提交 git commit -m '{cid}: <摘要>'，提交前不再改源码")


def adr_digests():
    """Per-ADR digests that ignore trailing blank lines, so appending a new ADR leaves earlier ones stable."""
    path = REPO / "docs/DECISIONS.md"
    text = path.read_text(encoding="utf-8").replace("\r\n", "\n") if path.exists() else ""
    parts = re.split(r"(?m)^(?=## ADR-\d+)", text)
    return {re.match(r"## (ADR-\d+)", part).group(1): digest(part.rstrip().encode("utf-8"))
            for part in parts if re.match(r"## ADR-\d+", part)}


def budget_problems(cfg, items):
    """A card must fit its full acceptance plus every verify gate inside the configured session budget."""
    total_budget, problems, notes = cfg["session_seconds"] * cfg["max_sessions"], [], []
    gates = sum(check.get("timeout_seconds", cfg["command_seconds"]) for check in cfg["verify"])
    for item in items:
        timeouts = [check.get("timeout_seconds", cfg["command_seconds"]) for check in item["checks"]]
        if sum(timeouts) + gates > total_budget:
            problems.append(f"{item['id']}: 验收{sum(timeouts)}s+verify{gates}s 超过会话额度{total_budget}s")
        notes += [f"{item['id']}/{check['id']}: 超时{value}s 超过单会话{cfg['session_seconds']}s，须跨会话续跑"
                  for check, value in zip(item["checks"], timeouts) if value > cfg["session_seconds"]]
    return problems, notes


def task(args):
    command = args[0] if args else "status"
    if command == "check":
        cfg = config()
        items = cards()
        problems, notes = budget_problems(cfg, items)
        for note in notes:
            print("note: " + note)
        if problems:
            raise WorkflowError("会话预算无法覆盖完整验收：" + "；".join(problems))
        print(f"roadmap OK: {len(items)} cards" + ("; DRAFT，不能启动驱动器" if not items else ""))
    elif command == "next":
        print(next_card()["id"])
    elif command == "show":
        item = get_card(args[1]) if len(args) > 1 else next_card()
        print(item["body"])
        for check in item["checks"]:
            args = [sys.executable, "-B", "scripts/run.py", check["id"], "--", *check["argv"]]
            print("\n" + (subprocess.list2cmdline(args) if os.name == "nt" else shlex.join(args)))
    elif command == "start" and len(args) == 2:
        start(args[1])
        for name in ("handoff.md", "journal.md"):
            path = card_dir(args[1]) / name
            if path.exists():
                print(path.read_text(encoding="utf-8"))
        print(f"started: {args[1]}")
    elif command == "rebaseline" and len(args) == 4 and args[2] == "--protected":
        rebaseline(args[1], args[3])
    elif command == "rebaseline" and len(args) == 6 and args[2] == "--protected" and args[4] == "--include-paused":
        rebaseline(args[1], args[3], args[5].split(","))
    elif command in ("note", "handoff") and len(args) > 1:
        cid = current()
        if not cid:
            raise WorkflowError("没有当前卡")
        text = " ".join(args[1:])
        path = card_dir(cid) / ("journal.md" if command == "note" else "handoff.md")
        with path.open("a" if command == "note" else "w", encoding="utf-8") as stream:
            stream.write(f"{utc()} {text}\n")
        if command == "note":
            node = state(cid)
            node["last_note"] = time.time()
            write_json(card_dir(cid) / "state.json", node)
    elif command == "scope":
        cid = args[1] if len(args) > 1 else current()
        if not cid:
            raise WorkflowError("没有当前卡")
        print("\n".join(scope_problems(cid)) or "all changes inside scope")
    elif command == "done" and len(args) == 2:
        done(args[1])
    elif command == "recover":
        recover_runs()
    elif command == "status":
        cid = current()
        print(json.dumps(dict(current=cid, state=state(cid) if cid else {},
                              protected=protected_hash(), adr=adr_digests(), blocked=list(blocked()), runs=receipts(cid) if cid else []), ensure_ascii=False, indent=2))
    else:
        raise WorkflowError("task.py: check|next|show [ID]|start ID|rebaseline ID --protected SHA256 [--include-paused ID,...]|note TEXT|handoff TEXT|scope|done ID|status|recover")
    return 0


identity = runtime.identity
signal_exact = runtime.signal_exact


def heavy_path(cfg=None):
    cfg = cfg or config()
    return runtime.lock_path(cfg)


def heavy_active():
    path = heavy_path()
    if not path.exists():
        return False
    with runtime.FileLock(path) as lock:
        return not lock.acquire()


def user_bus_environment():
    env = os.environ.copy()
    env.update(PYTHONUTF8="1", PYTHONIOENCODING="utf-8")
    if sys.platform != "linux":
        return env
    runtime = Path(f"/run/user/{os.getuid()}")
    if not env.get("XDG_RUNTIME_DIR") and runtime.is_dir():
        env["XDG_RUNTIME_DIR"] = str(runtime)
    return env


def run_alive(run):
    if runtime.alive(run):
        return True
    launcher = read_json(REPO / Path(run["log"]).parent / "launcher.json")
    if runtime.alive(launcher):
        return True
    if any(runtime.alive(item) for item in run.get("payload_refs", [])):
        return True
    if os.name != "nt" and run.get("payload_pid") and run.get("payload_identity"):
        try:
            if runtime.session_family(run["payload_pid"], run["payload_identity"]):
                return True
        except (WorkflowError, OSError) as error:
            raise WorkflowError("无法确认 payload session 清理；保留未完成状态") from error
    elif run.get("status") == "cleanup_failed" or (run.get("started") and run.get("status") != "finished"):
        raise WorkflowError("无法确认未登记 payload 边界的清理；保留未完成状态")
    if sys.platform == "linux" and run.get("scope_started"):
        if run.get("boot") != Path("/proc/sys/kernel/random/boot_id").read_text(encoding="utf-8").strip():
            return False
        result = subprocess.run(["systemctl", "--user", "is-active", run["unit"]], capture_output=True, text=True, timeout=5,
                                env=user_bus_environment())
        if result.stdout.strip() in ("active", "activating", "deactivating"):
            return True
        if result.returncode not in (0, 3, 4):
            raise WorkflowError("无法确认旧 scope 状态；不把残留运行视为已结束")
    return False


def recover_runs():
    for run in receipts():
        if run["status"] == "finished":
            continue
        if run_alive(run):
            raise WorkflowError(f"旧运行仍存活：{run['id']}")
        if time.time() - run["created"] < 5:
            raise WorkflowError("运行刚启动，稍后再核对")
        run.update(status="finished", exit=125, raw_exit=None, ended=utc(),
                   note="执行被中断，缺少完整证据；须重新验证", tests=None)
        log = REPO / run["log"]
        run["log_hash"] = digest(log.read_bytes()) if log.exists() else None
        run.pop("path", None)
        write_json(REPO / Path(run["log"]).parent / "receipt.json", run)
        print(f"interrupted: {run['id']} exit=125；原日志保留")


def test_counts(log, report, started_ns, old_report):
    if report == "unittest":
        totals = re.findall(r"(?m)^Ran (\d+) tests?\b", log)
        summaries = re.findall(r"(?m)^(?:OK|FAILED)(?: \(([^()\n]*)\))?[ \t]*$", log)
        count = lambda key: sum(int(value) for row in summaries for value in re.findall(
            r"(?:^|,\s*)" + re.escape(key) + r"=(\d+)(?=,|$)", row))
        expected, unexpected = count("expected failures"), count("unexpected successes")
        failed, skipped = count("failures") + count("errors") + unexpected, count("skipped")
        return dict(passed=max(0, sum(int(total) for total in totals) - failed - skipped - expected),
                    failed=failed, skipped=skipped, expected_failures=expected, unexpected_successes=unexpected)
    if report == "cargo":
        rows = re.findall(r"(?m)^test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored", log)
        return dict(passed=sum(int(a) for a, _, _ in rows), failed=sum(int(b) for _, b, _ in rows),
                    skipped=sum(int(c) for _, _, c in rows))
    if report == "pytest":
        log = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", log)
        outcome = r"\d+ (?:passed|failed|skipped|errors?|warnings?|xfailed|xpassed|deselected)"
        summaries = [line.strip("= ") for line in log.splitlines()
                     if re.fullmatch(r"(?:" + outcome + r"(?:, " + outcome + r")*|no tests ran) in \d+(?:\.\d+)?s(?: \([^\n]*\))?", line.strip("= "))]
        summary = summaries[-1] if summaries else ""
        count = lambda word: sum(int(n) for n in re.findall(r"(\d+) " + word + r"\b", summary))
        return dict(passed=count("passed"), failed=count("failed") + count("errors?"), skipped=count("skipped"))
    path = REPO / relative_path(report[6:])
    if not path.is_file() or path.resolve().is_relative_to(REPO / "evidence") is False:
        raise WorkflowError("JUnit 报告缺失或指向项目证据目录之外")
    metadata = (path.stat().st_mtime_ns, path.stat().st_size)
    if metadata == old_report or metadata[0] < started_ns - 1000000000:
        raise WorkflowError("JUnit 报告不是本次执行生成的证据")
    root = ET.fromstring(path.read_bytes())
    cases = list(root.iter("testcase"))
    failed = sum(bool(list(case.iter("failure")) or list(case.iter("error"))) for case in cases)
    skipped = sum(bool(list(case.iter("skipped"))) for case in cases)
    return dict(passed=len(cases) - failed - skipped, failed=failed, skipped=skipped)


def case_problems(check, data):
    """Every frozen case ID must appear as a JUnit testcase name and pass, unless declared skippable."""
    outcomes = {}
    for case in ET.fromstring(data).iter("testcase"):
        outcome = ("failed" if list(case.iter("failure")) or list(case.iter("error"))
                   else "skipped" if list(case.iter("skipped")) else "passed")
        outcomes.setdefault(case.get("name", ""), set()).add(outcome)
    allowed, problems = set(check.get("skippable") or []), []
    for name in check.get("cases") or []:
        seen = outcomes.get(name)
        if not seen:
            problems.append(f"{name} 未执行")
        elif "failed" in seen:
            problems.append(f"{name} 失败")
        elif "passed" not in seen and name not in allowed:
            problems.append(f"{name} 被跳过但未声明 skippable")
    return problems


def failure_signature(log, rc, note):
    names = re.findall(r"(?m)^(?:test .* \.\.\. FAILED|FAIL: .*|ERROR: .*|FAILED .*|thread .* panicked.*)$", log)
    causes = re.findall(r"(?m)^(?:error(?:\[E\d+\])?:.*|AssertionError:.*|[A-Za-z]+Error:.*)$", log)
    text = "\n".join(sorted(set(names)) + causes[:1]) or f"exit={rc}\n{note}\n" + "\n".join(log.splitlines()[-3:])
    return digest(re.sub(r"\d+", "N", text))[:16]


def record_failure(run):
    if run["exit"] == 0 or not run.get("card"):
        return
    node = state(run["card"])
    log = (REPO / run["log"]).read_text(errors="replace", encoding="utf-8")
    sig = failure_signature(log, run["exit"], run.get("note", ""))
    node["failures"].append(dict(time=time.time(), signature=sig, session=run["session"], log=run["log"], exit=run["exit"]))
    count = sum(f["signature"] == sig and f["session"] == run["session"] for f in node["failures"])
    if count >= 5:
        node["hard_sessions"] = sorted(set(node.get("hard_sessions", []) + [run["session"]]))
    write_json(card_dir(run["card"]) / "state.json", node)
    if count >= 3:
        with (REPO / run["log"]).open("a", encoding="utf-8", newline="\n") as stream:
            stream.write(f"\nSTUCK{'-HARD' if count >= 5 else ''}: same_failure={count}; 换根本不同方案或 BLOCKED\n")


def execute_receipt(path):
    run = read_json(path)
    cfg = config()
    run.update(pid=os.getpid(), identity=identity(os.getpid()), scope_started=runtime.backend() == "systemd")
    write_json(path, run)
    lock_path = heavy_path(cfg)
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    rc, raw_rc, note, counts, cleanup_failed = 2, None, "", None, False
    try:
        with runtime.FileLock(lock_path) as lock:
            deadline = time.monotonic() + 1800
            while True:
                if lock.acquire():
                    break
                if time.monotonic() >= deadline:
                    raise WorkflowError("等待全局重任务锁超时")
                time.sleep(0.2)
            if any(item.get("status") == "cleanup_failed" for item in receipts()):
                raise WorkflowError("存在未完成清理的重命令；先核对并 recover")
            if run.get("card"):
                check_frozen(run["card"])
            run.update(status="running", source=snapshot(), started=utc())
            write_json(path, run)
            env = user_bus_environment()
            env.update(AGENT_WORKFLOW_RUN="1", CARGO_BUILD_JOBS=str(cfg["jobs"]),
                       RUST_TEST_THREADS="1", CARGO_TARGET_DIR=str(REPO / "target"),
                       TMPDIR=str(REPO / "evidence/tmp"), TEMP=str(REPO / "evidence/tmp"), TMP=str(REPO / "evidence/tmp"))
            Path(env["TMPDIR"]).mkdir(parents=True, exist_ok=True)
            report = run["check"].get("report", "")
            old_report = None
            if report.startswith("junit:"):
                report_path = REPO / relative_path(report[6:])
                if report_path.exists():
                    old_report = (report_path.stat().st_mtime_ns, report_path.stat().st_size)
            started_ns = time.time_ns()
            timeout = run["check"].get("timeout_seconds", cfg["command_seconds"])
            def observe(pid, token, refs):
                if run.get("payload_refs") != refs:
                    run.update(payload_pid=pid, payload_identity=token, payload_refs=refs)
                    write_json(path, run)

            try:
                result = runtime.run_payload(run["argv"], cwd=REPO, env=env, seconds=timeout, memory_mib=cfg["memory_mib"],
                                             sampled=runtime.backend() == "process-watchdog", lock_fd=lock.fileno(), observe=observe)
            except runtime.PayloadCleanupError as error:
                # Publish the barrier while still holding the heavy lock. A
                # queued executor must see it before it can launch a payload.
                cleanup_failed = True
                run.update(status="cleanup_failed", exit=125, note=str(error))
                write_json(path, run)
                raise
            raw_rc, rc, note = result["raw_exit"], result["exit"], result["note"]
            run["peak_rss_bytes"] = result["peak_rss_bytes"]
            run["after_source"] = snapshot()
            if run["after_source"] != run["source"]:
                rc, note = 6, "命令执行期间源码被修改，证据不适用于稳定源码"
            log = (REPO / run["log"]).read_text(errors="replace", encoding="utf-8")
            if run["check"]["kind"] == "test":
                counts = test_counts(log, report, started_ns, old_report)
                if rc == 0 and (counts["passed"] == 0 or counts["failed"]):
                    rc, note = 3, "零个通过测试或存在失败；不能视为成功"
                if rc == 0 and run["check"].get("cases"):
                    problems = case_problems(run["check"], (REPO / relative_path(report[6:])).read_bytes())
                    if problems:
                        rc, note = 3, "验收用例不完整：" + "；".join(problems[:5])
            if report.startswith("junit:") and (REPO / report[6:]).is_file():
                report_copy = path.parent / "test-report.xml"
                shutil.copyfile(REPO / report[6:], report_copy)
                run.update(report_artifact=report_copy.relative_to(REPO).as_posix(), report_hash=digest(report_copy.read_bytes()))
            if rc == 124:
                note = f"命令超时：{timeout}s"
            elif rc == 137 and not note:
                note = "SIGKILL：核查内存上限；拆分命令，不提高上限"
    except (WorkflowError, OSError, ValueError, ET.ParseError) as error:
        note = str(error)
        print(f"run: {error}", flush=True)
        if raw_rc == 0:
            rc = 3
    try:
        remaining = any(runtime.alive(item) for item in run.get("payload_refs", []))
    except (WorkflowError, OSError) as error:
        remaining = True
        note = str(error)
    if remaining or cleanup_failed:
        detail = run.get("note", "") if cleanup_failed else note
        rc, note = 125, "payload 边界清理未确认；保留未完成状态，不允许重复启动或验收"
        if detail:
            note += "；" + detail
    run.update(status="cleanup_failed" if remaining or cleanup_failed else "finished", raw_exit=raw_rc, exit=rc, tests=counts, note=note, ended=utc())
    print(f"run: id={run['id']} exit={rc} tests={counts} log={run['log']} note={note}", flush=True)
    record_failure(run)
    if run["status"] == "finished":
        run["log_hash"] = digest((REPO / run["log"]).read_bytes())
    write_json(path, run)
    return rc


def run_command(name, argv, check=None):
    if os.environ.get("AGENT_WORKFLOW_RUN"):
        raise WorkflowError("run.py/verify.py 不可嵌套")
    cfg, cid = config(), current()
    if any(item.get("status") == "cleanup_failed" for item in receipts()):
        raise WorkflowError("存在未完成清理的重命令；先核对并 recover")
    if not re.fullmatch(r"[A-Za-z0-9._-]+", name) or not argv:
        raise WorkflowError("运行名或命令无效")
    if cid:
        check_frozen(cid)
        node = state(cid)
        if node["sessions"] in node.get("hard_sessions", []):
            raise WorkflowError("STUCK-HARD：当前会话禁止继续重运行", 4)
        if node["failures"] and node["last_note"] < node["failures"][-1]["time"]:
            raise WorkflowError("上次运行失败；重跑前先 task.py note 新假设和改动")
        declared = next((item for item in get_card(cid)["checks"] if item["id"] == name), None)
        if declared:
            if argv != declared["argv"]:
                raise WorkflowError(f"{name}: 命令与卡片验收 argv 不一致")
            if check is not None and check != declared:
                raise WorkflowError(f"{name}: 显式验证定义不能被卡片验收覆盖")
            check = declared
    check = check or dict(id=name, argv=argv, kind="command")
    runtime.require_process_support()
    selected = runtime.backend()
    if selected == "systemd" and not shutil.which("systemd-run") and not shutil.which("systemd-run.py"):
        raise WorkflowError("需要 systemd-run；Linux 不退化为无限制运行")
    folder = REPO / "evidence/runs" / (dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S%f") + "-" + name)
    folder.mkdir(parents=True)
    path, log = folder / "receipt.json", folder / "output.log"
    record = dict(id=name, argv=argv, check=check, card=cid, session=state(cid).get("sessions", 0) if cid else 0,
                  protected=protected_hash(), generation=state(cid).get("generation", 0) if cid else 0,
                  spec=get_card(cid)["spec"] if cid else None, created=time.time(), status="queued",
                  log=log.relative_to(REPO).as_posix(), unit="agent-workflow-" + uuid.uuid4().hex + ".scope",
                  boot=Path("/proc/sys/kernel/random/boot_id").read_text(encoding="utf-8").strip() if sys.platform == "linux" else None,
                  environment=dict(python=sys.version.split()[0], platform=sys.platform, backend=selected,
                                   memory_enforcement="cgroup-hard" if selected == "systemd" else "rss-sampled",
                                   cpu_enforcement="cgroup-quota" if selected == "systemd" else "build-jobs",
                                   memory_mib=cfg["memory_mib"], jobs=cfg["jobs"], test_threads=1))
    write_json(path, record)
    log.touch()
    command = [sys.executable, "-B", str(REPO / "scripts/workflow.py"), "_execute", str(path)]
    if selected == "systemd":
        command = ["systemd-run", "--user", "--scope", "--quiet", "--collect", "--unit", record["unit"],
                   "-p", f"MemoryMax={cfg['memory_mib']}M", "-p", "MemorySwapMax=0", "-p", f"CPUQuota={cfg['jobs'] * 100}%", *command]
    with log.open("ab", buffering=0) as stream:
        process = runtime.launch(command, cwd=REPO, stdout=stream, stderr=stream,
                                 stdin=subprocess.DEVNULL, detached=True, env=user_bus_environment())
        write_json(folder / "launcher.json", dict(pid=process.pid, identity=identity(process.pid)))
        rc = process.wait()
    record = read_json(path)
    if record["status"] not in ("finished", "cleanup_failed"):
        try:
            alive = run_alive(record)
            cleanup_note = ""
        except (WorkflowError, OSError) as error:
            alive, cleanup_note = True, "；" + str(error)
        record.update(status="cleanup_failed" if alive else "finished", exit=(rc if rc > 0 else 128 - rc if rc < 0 else 125), raw_exit=rc, ended=utc(),
                      tests=None, note="执行器失败或被中断；没有完整验收证据" + ("；旧边界清理未确认，禁止重复启动" if alive else "") + cleanup_note)
        record_failure(record)
        if not alive:
            record["log_hash"] = digest(log.read_bytes())
        write_json(path, record)
    print("\n".join(log.read_text(errors="replace", encoding="utf-8").splitlines()[-30:]))
    return record


def run(args):
    if len(args) < 3 or args[1] != "--":
        raise WorkflowError("run.py <验收ID或运行名> -- <准确命令argv>")
    return run_command(args[0], args[2:])["exit"]


def verify(args):
    if args and args != ["all"]:
        raise WorkflowError("verify.py [all]；执行配置列出的全部门槛")
    if os.environ.get("AGENT_WORKFLOW_RUN"):
        raise WorkflowError("verify.py 不能套进 run.py")
    cfg, source, frozen = config(), snapshot(), protected_hash()
    cards()
    cid = current()
    receipt = dict(card=cid, source=source, protected=frozen, runs=[], created=utc(), status="running", exit=None,
                   generation=state(cid).get("generation", 0) if cid else 0)
    path = REPO / "evidence/verify" / (dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S%f") + ".json")
    write_json(path, receipt)
    try:
        for check in cfg["verify"]:
            result = run_command(check["id"], check["argv"], check)
            receipt["runs"].append((Path(result["log"]).parent / "receipt.json").as_posix())
            if result["exit"]:
                receipt.update(exit=result["exit"], note=result.get("note", ""))
                return result["exit"]
            if result.get("source") != source or result.get("after_source") != source:
                raise WorkflowError("verify 阶段源码变化；不能生成完整成功收据")
        receipt["exit"] = 0
        print(f"verify all PASS: {path.relative_to(REPO)}")
        return 0
    except BaseException as error:
        receipt.update(exit=error.code if isinstance(error, WorkflowError) else 130 if isinstance(error, KeyboardInterrupt) else 2,
                       note=f"{type(error).__name__}: {error}")
        raise
    finally:
        receipt.update(status="finished", finished=utc())
        write_json(path, receipt)
        if receipt["exit"]:
            print(f"verify all FAIL: exit={receipt['exit']} receipt={path.relative_to(REPO)}")


def terminate_session(pid, token):
    if not pid or not token:
        return
    excluded = detached_ids() - {pid}
    path = REPO / "evidence/processes" / f"{pid}-{digest(token)[:16]}.json"
    refs = read_json(path, [])
    refs.extend(runtime.family(pid, token, excluded))
    runtime.terminate([item for item in refs if item["pid"] not in excluded])


def activity(cid):
    values = [snapshot(), git("rev-parse", "HEAD")]
    for name in ("journal.md", "handoff.md"):
        path = card_dir(cid) / name
        values.append(digest(path.read_bytes()) if path.exists() else "")
    values.extend((run["path"], run["status"]) for run in receipts(cid))
    return digest(values)


def detached_ids():
    result = set()
    for run in receipts():
        launcher = read_json(REPO / Path(run["log"]).parent / "launcher.json")
        for item in (run, launcher, *run.get("payload_refs", [])):
            if runtime.alive(item):
                result.add(item["pid"])
                result.update(ref["pid"] for ref in runtime.family(item["pid"], item["identity"]))
    return result


def agent_executor(directory):
    while not (directory / "ready").exists():
        time.sleep(.02)
    command = read_json(directory / "launch.json")["argv"]
    pid, token = os.getpid(), identity(os.getpid())
    path = REPO / "evidence/processes" / f"{pid}-{digest(token)[:16]}.json"
    child = runtime.launch(command, cwd=REPO, env=user_bus_environment())
    known = {}
    while child.poll() is None:
        refs = runtime.family(pid, token, detached_ids())
        fresh = {(item["pid"], item["identity"]): item for item in refs}
        if any(key not in known for key in fresh):
            known.update(fresh)
            write_json(path, list(known.values()))
        time.sleep(.1)
    return child.returncode


def latest_review():
    runs = sorted((REPO / "evidence/reviews").glob("*/review.json"))
    return read_json(runs[-1]) if runs else {}


def unreviewed_card():
    """The newest completed card needs a review even if the driver died before starting one."""
    done = [item for item in cards() if completed(item)]
    if not done:
        return None
    newest = max(done, key=lambda item: read_json(card_dir(item["id"]) / "done.json")["finished"])["id"]
    reviewed = {read_json(path).get("card") for path in (REPO / "evidence/reviews").glob("*/review.json")}
    return None if newest in reviewed else newest


def review(cfg, cid, stop_requested):
    """Run one read-only review agent on the current commit; only CLEAR lets the driver advance."""
    head = git("rev-parse", "HEAD")
    rid = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%S%f") + "-" + cid
    directory = REPO / "evidence/reviews" / rid
    directory.mkdir(parents=True)
    prompt_path = directory / "prompt.md"
    prompt = (REPO / "docs/PROMPT-REVIEW.md").read_text(encoding="utf-8")
    prompt_path.write_text(prompt + f"\n本轮由驱动器启动：审查卡 {cid}，固定提交 {head}。\n", encoding="utf-8", newline="\n")
    command = [arg.replace("{prompt_file}", str(prompt_path)) for arg in runtime.resolve_argv(cfg["review_command"])]
    result = dict(id=rid, card=cid, commit=head, started=utc(), argv=command, verdict="FAILED")
    write_json(directory / "review.json", result)
    print(f"{utc()} review {cid} {head[:12]}", flush=True)
    output = directory / "output.log"
    with prompt_path.open("rb") as stdin, output.open("wb") as stdout, (directory / "stderr.log").open("wb") as stderr:
        child = runtime.launch(command, cwd=REPO, stdin=stdin, stdout=stdout, stderr=stderr, detached=True, env=user_bus_environment())
    token = identity(child.pid)
    deadline = time.monotonic() + cfg["review_seconds"]
    while child.poll() is None and time.monotonic() < deadline and not stop_requested():
        time.sleep(0.5)
    if child.poll() is None:
        terminate_session(child.pid, token)
        child.wait(timeout=5)
        result["reason"] = "stopped" if stop_requested() else "timeout"
    else:
        result["exit"] = child.returncode
        # The agent's final report ends with the verdict line; a missing verdict is a failed review.
        verdicts = re.findall(r"REVIEW-VERDICT:\s*(CLEAR|BLOCKING)\b", output.read_text(encoding="utf-8", errors="replace"))
        if child.returncode == 0 and verdicts:
            result["verdict"] = verdicts[-1]
        else:
            result["reason"] = "nonzero_exit" if child.returncode else "missing_verdict"
    result["ended"] = utc()
    write_json(directory / "review.json", result)
    print(f"{utc()} review {cid} {result['verdict']}: {output.relative_to(REPO).as_posix()}", flush=True)
    return result


def require_review(result):
    if result["verdict"] != "CLEAR":
        raise ReviewBlocked(f"巡检 {result['id']} 结果 {result['verdict']}：处理后重启驱动器；"
                            f"用户明确接受时执行 drive.py accept-review {result['id']}", 13)


def accept_review(rid):
    path = REPO / "evidence/reviews" / rid / "review.json"
    result = read_json(path)
    if not result or result.get("verdict") != "BLOCKING" or "ended" not in result:
        raise WorkflowError("只能接受已结束且结论为 BLOCKING 的巡检")
    result["accepted"] = utc()
    write_json(path, result)
    print(f"review {rid} accepted")
    return 0


def drive(args):
    state_path = REPO / "evidence/drive/state.json"
    stop_path = REPO / "evidence/drive/stop.json"
    old = read_json(state_path)
    if args and args[0] == "status":
        print(json.dumps(old, ensure_ascii=False, indent=2))
        return 0
    if args and args[0] == "accept-review" and len(args) == 2:
        return accept_review(args[1])
    if args and args[0] == "stop":
        pid, token = old.get("pid"), old.get("identity")
        if not pid or not token or identity(pid) != token:
            print("driver not running (missing or stale identity)")
            return 0
        write_json(stop_path, dict(pid=pid, identity=token))
        if os.name != "nt":
            signal_exact(pid, token, signal.SIGTERM)
        until = time.monotonic() + 12
        while identity(pid) == token and time.monotonic() < until:
            time.sleep(0.2)
        if identity(pid) == token:
            terminate_session(old.get("agent_pid"), old.get("agent_identity"))
            signal_exact(pid, token, getattr(signal, "SIGKILL", 9))
            old.update(status="stopped", reason="forced_stop", ended=utc())
            write_json(state_path, old)
        return 0
    parser = argparse.ArgumentParser(prog="drive.py")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--once", action="store_true")
    mode.add_argument("--review-only", action="store_true")
    parser.add_argument("--session-hours", type=float)
    parser.add_argument("--max-sessions", type=int)
    options = parser.parse_args(args)
    cfg = config()
    seconds = cfg["session_seconds"]
    maximum = cfg["max_sessions"]
    if options.session_hours is not None:
        if not math.isfinite(options.session_hours) or options.session_hours <= 0:
            raise WorkflowError("session-hours 须为有限正数")
        seconds = math.ceil(options.session_hours * 3600)
    if options.max_sessions is not None:
        if options.max_sessions <= 0:
            raise WorkflowError("max-sessions 须为正整数")
        maximum = options.max_sessions
    cards()
    git("rev-parse", "HEAD")
    runtime.require_process_support()
    if options.review_only and not cfg.get("review_command"):
        raise WorkflowError("workflow.json: review_command 未配置；独立巡检需要只读 agent argv")
    if not options.review_only and not cfg.get("agent_command"):
        raise WorkflowError("workflow.json: agent_command 未配置；写入所用 agent CLI 的非交互 argv")
    agent_argv = None if options.review_only else runtime.resolve_argv(cfg["agent_command"])
    if options.review_only and runtime.alive(dict(pid=old.get("agent_pid"), identity=old.get("agent_identity"))):
        raise WorkflowError("旧开发会话仍存活；先停止或等待结束，不启动独立巡检")
    lock_path = REPO / "evidence/drive/driver.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    lock = runtime.FileLock(lock_path)
    if not lock.acquire():
        lock.close()
        raise WorkflowError("已有驱动器运行")
    stopped, agent, token = False, None, None
    status = dict(pid=os.getpid(), identity=identity(os.getpid()), started=utc(), status="starting", agent_command=agent_argv)

    def request_stop(_signum, _frame):
        nonlocal stopped
        stopped = True

    def stop_requested():
        nonlocal stopped
        request = read_json(stop_path)
        stopped = stopped or (request.get("pid") == os.getpid() and request.get("identity") == status["identity"])
        return stopped

    previous_handlers = {sig: signal.signal(sig, request_stop) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        if not options.review_only:
            terminate_session(old.get("agent_pid"), old.get("agent_identity"))
        write_json(state_path, status)
        last = latest_review() if cfg.get("review_command") else {}
        if options.review_only:
            pending = last and last["verdict"] != "CLEAR" and not last.get("accepted")
            if pending:
                item = get_card(last["card"])
            else:
                finished = [item for item in cards() if completed(item)]
                if not finished:
                    raise WorkflowError("没有具备已验证完成提交的任务卡，不能独立巡检")
                item = max(finished, key=lambda value: read_json(card_dir(value["id"]) / "done.json")["finished"])
            cid = item["id"]
            if not completed(item):
                raise WorkflowError(f"{cid} 缺少已验证完成提交，不能独立巡检")
            check_completion_tree(cid)
            if heavy_active():
                raise WorkflowError("重命令仍占用全局锁，等待结束后再独立巡检")
            decisions = blocked()
            if cid in decisions or global_blocks(decisions, cards()):
                raise WorkflowError("BLOCKED：存在未被同卡 RESOLVED 解除的决定", 11)
            if pending and last["verdict"] == "BLOCKING" and last["commit"] == git("rev-parse", "HEAD"):
                require_review(last)
            status.update(status="reviewing", card=cid)
            write_json(state_path, status)
            result = review(cfg, cid, stop_requested)
            try:
                if git("rev-parse", "HEAD") != result["commit"]:
                    raise WorkflowError("巡检期间提交发生变化；保留现场并重新核验")
                check_completion_tree(cid)
            except (WorkflowError, OSError, subprocess.SubprocessError) as error:
                result.update(reported_verdict=result["verdict"], verdict="FAILED",
                              reason="post_review_validation_failed", validation_error=str(error))
                write_json(REPO / "evidence/reviews" / result["id"] / "review.json", result)
                raise
            require_review(result)
            status.update(status="stopped", reason="review_done")
            return 0
        if last and last["verdict"] != "CLEAR" and not last.get("accepted"):
            # An unchanged BLOCKING commit stays blocked; interrupted reviews or new fixes are reviewed again.
            if last["verdict"] == "BLOCKING" and last["commit"] == git("rev-parse", "HEAD"):
                require_review(last)
            status.update(status="reviewing", card=last["card"])
            write_json(state_path, status)
            require_review(review(cfg, last["card"], stop_requested))
        missing = unreviewed_card() if cfg.get("review_command") else None
        if missing:
            status.update(status="reviewing", card=missing)
            write_json(state_path, status)
            require_review(review(cfg, missing, stop_requested))
        while not stop_requested():
            try:
                item = next_card()
            except WorkflowError as error:
                status.update(status="all_done" if error.code == 10 else "blocked", reason=str(error))
                return 0 if error.code == 10 else error.code
            cid = item["id"]
            status.update(card=cid, status="waiting_for_runs")
            write_json(state_path, status)
            while not stop_requested():
                unfinished = [run for run in receipts() if run["status"] != "finished"]
                if not unfinished and not heavy_active():
                    break
                if any(not run_alive(run) and time.time() - run["created"] >= 5 for run in unfinished):
                    recover_runs()
                time.sleep(min(cfg["poll_seconds"], 5))
            if stopped:
                break
            start(cid)
            node = state(cid)
            hard = node.get("hard_sessions", [])
            if hard and node["sessions"] > hard[0]:
                raise WorkflowError("STUCK-HARD 的一次恢复会话已用完，等待用户决定", 13)
            if node["sessions"] >= maximum:
                raise WorkflowError("本卡累计会话额度耗尽，等待用户决定", 13)
            node["sessions"] += 1
            session = node["sessions"]
            write_json(card_dir(cid) / "state.json", node)
            directory = card_dir(cid) / f"session-{session}"
            directory.mkdir()
            prompt = (REPO / "docs/PROMPT-CARD.md").read_text(encoding="utf-8")
            handoff = []
            for filename in ("handoff.md", "journal.md"):
                path = card_dir(cid) / filename
                if path.exists():
                    handoff.append(filename + "\n" + "\n".join(path.read_text(encoding="utf-8").splitlines()[-30:]))
            deadline = (dt.datetime.now(dt.timezone.utc) + dt.timedelta(seconds=seconds)).isoformat()
            substitutions = {"PROJECT_NAME": cfg["project"], "CARD": cid, "SESSION": str(session),
                             "PLATFORM": sys.platform, "PYTHON": json.dumps(sys.executable, ensure_ascii=False),
                             "MAX_SESSIONS": str(maximum), "DEADLINE": deadline,
                             "CARD_TEXT": item["body"], "HANDOFF": "\n\n".join(handoff)}
            prompt = re.sub(r"\{\{([A-Z_]+)\}\}", lambda match: substitutions[match.group(1)], prompt)
            prompt_path = directory / "prompt.md"
            prompt_path.write_text(prompt, encoding="utf-8", newline="\n")
            # The prompt goes to stdin; tools that take a file instead use {prompt_file}.
            command = [arg.replace("{prompt_file}", str(prompt_path)) for arg in agent_argv]
            write_json(directory / "launch.json", dict(argv=command))
            with prompt_path.open("rb") as stdin, (directory / "agent-output.log").open("wb") as stdout, (directory / "stderr.log").open("wb") as stderr:
                agent = runtime.launch([sys.executable, "-B", str(REPO / "scripts/workflow.py"), "_agent", str(directory)],
                                       cwd=REPO, stdin=stdin, stdout=stdout, stderr=stderr, detached=True, env=user_bus_environment())
            token = identity(agent.pid)
            status.update(status="running", session=session, agent_pid=agent.pid, agent_identity=token, deadline=deadline,
                          prompt=prompt_path.relative_to(REPO).as_posix(), output=(directory / "agent-output.log").relative_to(REPO).as_posix())
            write_json(state_path, status)
            (directory / "ready").touch()
            print(f"{utc()} {cid} session={session} pid={agent.pid}", flush=True)
            started, changed = time.monotonic(), time.monotonic()
            fingerprint = activity(cid)
            hard_since = None
            reason = "exited"
            while agent.poll() is None and not stop_requested():
                time.sleep(min(cfg["poll_seconds"], 5))
                now, fresh = time.monotonic(), activity(cid)
                if fresh != fingerprint or heavy_active():
                    changed, fingerprint = now, fresh
                hard_now = session in state(cid).get("hard_sessions", [])
                if hard_now and hard_since is None:
                    hard_since = now
                if now - started >= seconds:
                    reason = "session_timeout"
                    break
                if now - changed >= cfg["idle_seconds"]:
                    reason = "idle_timeout"
                    break
                if hard_since is not None and now - hard_since >= 60:
                    reason = "stuck_hard"
                    break
                status.update(elapsed_seconds=int(now - started), idle_seconds=int(now - changed), heavy_running=heavy_active())
                write_json(state_path, status)
            terminate_session(agent.pid, token)
            exit_code = agent.wait(timeout=5)
            status.update(status="session_ended", reason="user_stop" if stopped else reason, agent_exit=exit_code)
            write_json(directory / "result.json", status)
            agent, token = None, None
            status.pop("agent_pid", None)
            status.pop("agent_identity", None)
            if stopped:
                break
            if completed(get_card(cid)):
                check_completion_tree(cid)
                print(f"{cid} DONE (verified commit)", flush=True)
                if cfg.get("review_command"):
                    status.update(status="reviewing")
                    write_json(state_path, status)
                    require_review(review(cfg, cid, stop_requested))
                if options.once:
                    status.update(status="stopped", reason="once_done")
                    return 0
            else:
                output = directory / "agent-output.log"
                handoff_path = card_dir(cid) / "handoff.md"
                if output.exists() and not handoff_path.exists():
                    tail = output.read_text(encoding="utf-8", errors="replace").splitlines()[-30:]
                    handoff_path.write_text("会话输出末尾（未经验证，不是完成证据）：\n" + "\n".join(tail) + "\n", encoding="utf-8", newline="\n")
                if options.once:
                    status.update(status="stopped", reason="once_wip")
                    return 1
        status.update(status="stopped", reason="user_stop")
        return 143
    except (WorkflowError, OSError, subprocess.SubprocessError) as error:
        status.update(status="review_blocked" if isinstance(error, ReviewBlocked) else
                      "blocked" if isinstance(error, WorkflowError) and error.code == 11 else "error", reason=str(error))
        raise
    finally:
        try:
            if agent:
                terminate_session(agent.pid, token)
                agent.wait(timeout=5)
        finally:
            for sig, handler in previous_handlers.items():
                signal.signal(sig, handler)
            status.update(ended=utc())
            write_json(state_path, status)
            lock.close()


def main(argv=None):
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")
    argv = sys.argv[1:] if argv is None else argv
    if not argv:
        print("workflow.py task|run|verify|drive")
        return 2
    try:
        if argv[0] == "_execute" and len(argv) == 2:
            return execute_receipt(Path(argv[1]))
        if argv[0] == "_agent" and len(argv) == 2:
            return agent_executor(Path(argv[1]))
        functions = dict(task=task, run=run, verify=verify, drive=drive)
        if argv[0] not in functions:
            raise WorkflowError("未知工作流命令")
        return functions[argv[0]](argv[1:])
    except WorkflowError as error:
        print(f"workflow: {error}", file=sys.stderr)
        return error.code
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"workflow: {type(error).__name__}: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
