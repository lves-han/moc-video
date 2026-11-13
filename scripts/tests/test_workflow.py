"""Behavior checks in temporary repos; fake agent/systemd never call a model."""
import contextlib
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

PROJECT = Path(__file__).resolve().parents[2]

CAP = r'''#!/usr/bin/env python3
import os, sys
args = sys.argv[1:]
while args:
    if args[0] in ('-p', '--unit'):
        args = args[2:]
    elif args[0].startswith('--'):
        args = args[1:]
    else:
        break
if os.environ.get('FIXTURE_CAP_FAIL'):
    print('fixture: resource scope unavailable', flush=True)
    sys.exit(7)
os.execvp(args[0], args)
'''

AGENT = r'''#!/usr/bin/env python3
import importlib.util, json, os, re, subprocess, sys, time
from pathlib import Path
prompt = sys.stdin.read()
cid = re.search(r'本会话只处理 (T\d+\.\d+)', prompt).group(1)
root = Path.cwd()
log = root / 'evidence/agent-argv.json'
old = json.loads(log.read_text(encoding="utf-8")) if log.exists() else []
old.append({'argv': sys.argv[1:], 'card': cid, 'prompt': prompt})
log.parent.mkdir(parents=True, exist_ok=True)
log.write_text(json.dumps(old), encoding="utf-8", newline="\n")
mode = os.environ.get('FIXTURE_AGENT_MODE', 'claim')
if mode == 'sleep':
    time.sleep(20)
if mode in ('complete', 'lean_complete'):
    spec = importlib.util.spec_from_file_location('fixture_workflow', root / 'scripts/workflow.py')
    w = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(w)
    item = w.get_card(cid)
    logs = []
    for check in item['checks']:
        result = w.run_command(check['id'], check['argv'])
        if result['exit']:
            sys.exit(result['exit'])
        logs.append(result['log'])
    if mode == 'complete' and w.verify(['all']):
        sys.exit(1)
    path = root / 'docs/TASKS.md'
    body = item['body'].replace('- [ ] 完成', '- [x] 完成')
    path.write_text(path.read_text(encoding="utf-8").replace(item['body'], body), encoding="utf-8", newline="\n")
    with (root / 'docs/PROGRESS.md').open('a', encoding="utf-8", newline="\n") as stream:
        stream.write('\n## ' + cid + ' fixture ' + w.utc() + '\n- 结果：DONE\n')
        for check, log_path in zip(item['checks'], logs):
            stream.write('- 验收 ' + check['id'] + '：PASS；日志：`' + log_path + '`；断言：真实1个测试通过\n')
    w.done(cid)
    if mode == 'complete':
        subprocess.run(['git', 'add', '.'], check=True)
        subprocess.run(['git', 'commit', '-m', cid + ': fixture'], check=True, capture_output=True)
print('fixture claims DONE for ' + cid)
'''

REVIEWER = r'''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
prompt = sys.stdin.read()
log = Path.cwd() / 'evidence/review-calls.json'
calls = json.loads(log.read_text(encoding="utf-8")) if log.exists() else []
calls.append(prompt)
log.parent.mkdir(parents=True, exist_ok=True)
log.write_text(json.dumps(calls), encoding="utf-8", newline="\n")
mode = os.environ.get('FIXTURE_REVIEW_MODE', 'clear')
print('fixture review report')
if mode == 'clear':
    print('REVIEW-VERDICT: CLEAR')
if mode == 'blocking':
    print('**REVIEW-VERDICT: BLOCKING**')
'''

BEHAVIOR = '''import unittest
from pathlib import Path
class Behavior(unittest.TestCase):
    def test_value(self):
        self.assertEqual(Path('src/value.txt').read_text(encoding="utf-8"), 'ok')
'''
ARGV = ["{python}", "-B", "-m", "unittest", "discover", "-s", "tests", "-p", "test_*.py"]


def card_text(cid="T0.1", deps="无", check=None):
    check = check or dict(id="A1", description="固定输入输出", argv=ARGV, kind="test", report="unittest")
    return f"### {cid} 交付\n- [ ] 完成\n- 依赖：{deps}\n- 范围：`src/`、`tests/`\n- 交付物：固定输入输出。\n- 验收：\n```json\n{json.dumps([check], ensure_ascii=False)}\n```\n"


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="agent-workflow-test-")
        self.addCleanup(self.temporary.cleanup)
        base = Path(self.temporary.name).resolve()
        self.repo = base / "project with spaces 中文"
        self.repo.mkdir()
        # A populated project must not leak its source, gates or history into fixtures.
        paths = [*PROJECT.glob("scripts/*.py"), *PROJECT.glob("scripts/tests/*.py"),
                 *PROJECT.glob("docs/PROMPT-*.md")]
        paths += [PROJECT / name for name in ("workflow.json", ".gitattributes", ".gitignore", "AGENTS.md", "CLAUDE.md", ".github/workflows/script-portability.yml",
                                              "scripts/requirements.txt", "docs/ARCHITECTURE.md")]
        for path in paths:
            if path.is_file():
                copied = self.repo / path.relative_to(PROJECT)
                copied.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, copied)
        for name in ("PROGRESS", "DECISIONS", "BACKLOG"):
            (self.repo / f"docs/{name}.md").write_text(f"# {name}\n", encoding="utf-8", newline="\n")
        (self.repo / "scripts/check-project.py").write_text("import sys\nprint('not configured', file=sys.stderr)\nsys.exit(2)\n", encoding="utf-8", newline="\n")
        subprocess.run(["git", "init", "-b", "main", str(self.repo)], check=True, capture_output=True)
        self.git("config", "user.name", "Workflow fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "core.autocrlf", "false")
        tool_dir = base / "tools"
        tool_dir.mkdir()
        for name, source in (("systemd-run", CAP), ("agent", AGENT), ("reviewer", REVIEWER)):
            path = tool_dir / (name + ".py")
            path.write_text(source, encoding="utf-8", newline="\n")
            path.chmod(0o755)
        self.env = os.environ.copy()
        self.env.pop("AGENT_WORKFLOW_RUN", None)
        self.env.update(PATH=str(tool_dir) + os.pathsep + self.env["PATH"], FIXTURE_AGENT_MODE="claim", PYTHONUTF8="1", PYTHONIOENCODING="utf-8")
        self.environment = patch.dict(os.environ, self.env, clear=True)
        self.environment.start()
        self.addCleanup(self.environment.stop)
        spec = importlib.util.spec_from_file_location("workflow_under_test", self.repo / "scripts/workflow.py")
        self.w = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.w)
        cfg_path = self.repo / "workflow.json"
        cfg = json.loads(cfg_path.read_text(encoding="utf-8"))
        cfg.update(project="Fixture", poll_seconds=1, idle_seconds=30, heavy_lock=str(base / "heavy.lock"), lean_workflow=False,
                   agent_command=[sys.executable, str(tool_dir / "agent.py"), "--prompt-file", "{prompt_file}"],
                   review_command=None,
                   verify=[dict(id="verify-project", argv=ARGV, kind="test", report="unittest")])
        cfg_path.write_text(json.dumps(cfg), encoding="utf-8", newline="\n")
        (self.repo / "src").mkdir()
        (self.repo / "src/value.txt").write_text("ok", encoding="utf-8", newline="\n")
        (self.repo / "src/extra.txt").write_text("A", encoding="utf-8", newline="\n")
        (self.repo / "tests").mkdir()
        (self.repo / "tests/test_behavior.py").write_text(BEHAVIOR, encoding="utf-8", newline="\n")
        (self.repo / "docs/TASKS.md").write_text(card_text(), encoding="utf-8", newline="\n")
        self.commit("initial fixture")
        self.addCleanup(self.clean_processes)

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.repo), *args], check=True, capture_output=True, text=True).stdout.strip()

    def commit(self, subject):
        self.git("add", ".")
        self.git("commit", "-m", subject)

    def command(self, *args, timeout=30):
        return subprocess.run([sys.executable, "-B", str(self.repo / "scripts/workflow.py"), *args],
                              cwd=self.repo, env=os.environ.copy(), capture_output=True, text=True, encoding="utf-8", timeout=timeout)

    def clean_processes(self):
        for run in self.w.receipts():
            launcher = self.w.read_json(self.repo / Path(run["log"]).parent / "launcher.json")
            if launcher.get("pid"):
                self.w.terminate_session(launcher["pid"], launcher.get("identity"))
        driver = self.w.read_json(self.repo / "evidence/drive/state.json")
        self.w.terminate_session(driver.get("agent_pid"), driver.get("agent_identity"))

    def accept(self):
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
        self.assertEqual(result["exit"], 0, result)
        self.assertEqual(result["tests"]["passed"], 1)
        return result

    def record(self, result):
        path = self.repo / "docs/TASKS.md"
        path.write_text(path.read_text(encoding="utf-8").replace("- [ ] 完成", "- [x] 完成", 1), encoding="utf-8", newline="\n")
        with (self.repo / "docs/PROGRESS.md").open("a", encoding="utf-8", newline="\n") as stream:
            stream.write(f"\n## T0.1 fixture\n- 结果：DONE\n- 验收 A1：PASS；日志：`{result['log']}`；断言：真实1测试\n")

    def finish(self):
        result = self.accept()
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(self.w.verify(["all"]), 0)
        self.record(result)
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.done("T0.1")
        return result

    def enable_lean(self):
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["lean_workflow"] = True
        cfg["lean_from"] = "T0.1"
        path.write_text(json.dumps(cfg), encoding="utf-8")

    def test_lean_completion_needs_only_card_acceptance_and_no_commit(self):
        self.enable_lean()
        tasks = self.repo / "docs/TASKS.md"
        tasks.write_text(card_text() + card_text("T0.2", "T0.1"), encoding="utf-8")
        self.w.start("T0.1")
        (self.repo / "src/extra.txt").write_text("changed by first card", encoding="utf-8")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
        self.assertEqual(result["exit"], 0)
        self.record(result)
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.done("T0.1")
        self.assertTrue(self.w.completed(self.w.get_card("T0.1")))
        self.assertFalse(list((self.repo / "evidence/verify").glob("*.json")))
        self.assertTrue(self.git("status", "--porcelain"))
        self.assertEqual(self.w.next_card()["id"], "T0.2")
        self.w.start("T0.2")
        self.assertEqual(self.w.scope_problems("T0.2"), [])
        (self.repo / "unrelated.txt").write_text("outside scope", encoding="utf-8")
        self.assertEqual(self.w.scope_problems("T0.2"), ["unrelated.txt"])

    def test_lean_document_change_needs_no_manual_rebaseline_or_repeat(self):
        self.enable_lean()
        result = self.accept()
        architecture = self.repo / "docs/ARCHITECTURE.md"
        architecture.write_text(architecture.read_text(encoding="utf-8") + "\nAuthorized documentation update.\n", encoding="utf-8")
        self.record(result)
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.done("T0.1")
        self.assertTrue(self.w.completed(self.w.get_card("T0.1")))
        self.assertEqual(self.w.state("T0.1")["protected"], self.w.protected_hash())

    def test_lean_change_to_other_check_keeps_unaffected_acceptance(self):
        self.enable_lean()
        tasks = self.repo / "docs/TASKS.md"
        text = card_text()
        start, end = text.index("```json\n") + len("```json\n"), text.index("\n```", text.index("```json\n"))
        checks = json.loads(text[start:end])
        checks.append(dict(id="A2", description="另一项检查", argv=["{python}", "-B", "tests/old_delivery.py"], kind="command"))
        tasks.write_text(text[:start] + json.dumps(checks, ensure_ascii=False) + text[end:], encoding="utf-8")
        (self.repo / "tests/old_delivery.py").write_text("print('old')\n", encoding="utf-8")
        self.commit("plan two separate checks")
        first = self.accept()
        checks[1]["argv"][-1] = "tests/new_delivery.py"
        (self.repo / "tests/new_delivery.py").write_text("print('new')\n", encoding="utf-8")
        text = tasks.read_text(encoding="utf-8")
        start, end = text.index("```json\n") + len("```json\n"), text.index("\n```", text.index("```json\n"))
        tasks.write_text(text[:start] + json.dumps(checks, ensure_ascii=False) + text[end:], encoding="utf-8")
        controller = self.repo / "scripts/workflow.py"
        controller.write_text(controller.read_text(encoding="utf-8") + "\n# Unrelated workflow update.\n", encoding="utf-8")
        with contextlib.redirect_stdout(io.StringIO()):
            second = self.w.run_command("A2", checks[1]["argv"])
        self.assertEqual(second["exit"], 0)
        self.record(first)
        with (self.repo / "docs/PROGRESS.md").open("a", encoding="utf-8") as stream:
            stream.write(f"- 验收 A2：PASS；日志：`{second['log']}`；断言：第二项通过\n")
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.done("T0.1")
        self.assertTrue(self.w.completed(self.w.get_card("T0.1")))

    def test_lean_rejects_acceptance_for_changed_product_code(self):
        self.enable_lean()
        result = self.accept()
        (self.repo / "src/extra.txt").write_text("changed after acceptance", encoding="utf-8")
        self.record(result)
        with self.assertRaisesRegex(self.w.CompletionEvidenceError, "不是当前源码"):
            self.w.done("T0.1")

    def test_lean_rejects_acceptance_for_changed_workflow_code(self):
        self.enable_lean()
        path = self.repo / "docs/TASKS.md"
        path.write_text(path.read_text(encoding="utf-8").replace("`src/`、`tests/`", "`src/`、`tests/`、`scripts/`"), encoding="utf-8")
        result = self.accept()
        path = self.repo / "scripts/workflow.py"
        path.write_text(path.read_text(encoding="utf-8") + "\n# Changed after acceptance.\n", encoding="utf-8")
        self.record(result)
        with self.assertRaisesRegex(self.w.CompletionEvidenceError, "不是当前源码"):
            self.w.done("T0.1")

    def test_lean_keeps_legacy_completion_evidence(self):
        self.finish()
        self.commit("T0.1: fixture")
        self.assertTrue(self.w.completed(self.w.get_card("T0.1")))
        self.enable_lean()
        old_protected = self.w.state("T0.1")["protected"]
        architecture = self.repo / "docs/ARCHITECTURE.md"
        architecture.write_text(architecture.read_text(encoding="utf-8") + "\nLater documentation.\n", encoding="utf-8")
        self.w.check_frozen("T0.1")
        self.assertEqual(self.w.state("T0.1")["protected"], old_protected)
        self.assertTrue(self.w.completed(self.w.get_card("T0.1")))

    def test_lean_policy_requires_boolean(self):
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["lean_workflow"] = "yes"
        path.write_text(json.dumps(cfg), encoding="utf-8")
        with self.assertRaisesRegex(self.w.WorkflowError, "须为布尔值"):
            self.w.config()

    def test_lean_policy_starts_at_requested_card(self):
        self.enable_lean()
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["lean_from"] = "T0.34"
        path.write_text(json.dumps(cfg), encoding="utf-8")
        self.assertFalse(self.w.lean_enabled("T0.33"))
        self.assertTrue(self.w.lean_enabled("T0.34"))
        self.assertTrue(self.w.lean_enabled("T0.37"))
        self.assertTrue(self.w.lean_enabled("T1.1"))

    def test_lean_policy_does_not_relax_earlier_card(self):
        self.enable_lean()
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["lean_from"] = "T0.34"
        path.write_text(json.dumps(cfg), encoding="utf-8")
        self.commit("configure later lean policy")
        result = self.accept()
        self.record(result)
        with self.assertRaisesRegex(self.w.CompletionEvidenceError, "verify.py all"):
            self.w.done("T0.1")

    def test_lean_driver_does_not_require_review_verify_or_commit(self):
        self.enable_review("blocking")
        self.enable_lean()
        os.environ["FIXTURE_AGENT_MODE"] = "lean_complete"
        result = self.command("drive", "--once", timeout=40)
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertIn("T0.1 DONE", result.stdout)
        self.assertEqual(self.review_calls(), [])
        self.assertFalse(list((self.repo / "evidence/verify").glob("*.json")))
        self.assertTrue(self.git("status", "--porcelain"))
        self.assertEqual(self.w.next_card()["id"], "T0.2")

    def test_lean_review_is_optional_and_can_review_uncommitted_result(self):
        self.enable_review("clear")
        self.enable_lean()
        os.environ["FIXTURE_AGENT_MODE"] = "lean_complete"
        result = self.command("drive", "--once", timeout=40)
        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        review = self.command("drive", "--review-only", timeout=40)
        self.assertEqual(review.returncode, 0, review.stderr + review.stdout)
        self.assertEqual(len(self.review_calls()), 1)
        self.assertIn("当前工作树源码摘要", self.review_calls()[0])

    def test_draft_refuses_development(self):
        (self.repo / "docs/TASKS.md").write_text("# DRAFT\n", encoding="utf-8", newline="\n")
        with self.assertRaises(self.w.WorkflowError) as caught:
            self.w.next_card()
        self.assertEqual(caught.exception.code, 12)

    def test_dependencies_reject_forward_reference(self):
        (self.repo / "docs/TASKS.md").write_text(card_text(deps="T0.2") + card_text("T0.2"), encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "依赖"):
            self.w.cards()

    def test_unsafe_scope_and_missing_test_report_rejected(self):
        path = self.repo / "docs/TASKS.md"
        path.write_text(card_text().replace("`src/`", "`../other/`"), encoding="utf-8", newline="\n")
        with self.assertRaises(self.w.WorkflowError):
            self.w.cards()
        check = dict(id="A1", argv=ARGV, description="test", kind="test")
        path.write_text(card_text(check=check), encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "report"):
            self.w.cards()

    def test_checkbox_alone_does_not_complete(self):
        path = self.repo / "docs/TASKS.md"
        path.write_text(path.read_text(encoding="utf-8").replace("[ ]", "[x]"), encoding="utf-8", newline="\n")
        self.assertEqual(self.w.next_card()["id"], "T0.1")

    def test_wrong_card_cannot_start(self):
        with self.assertRaises(self.w.WorkflowError):
            self.w.start("T0.2")

    def test_done_requires_complete_verify(self):
        result = self.accept()
        self.record(result)
        with self.assertRaisesRegex(self.w.WorkflowError, "verify"):
            self.w.done("T0.1")

    def test_latest_failed_verify_invalidates_previous_success(self):
        path = self.repo / "tests/test_behavior.py"
        path.write_text(path.read_text(encoding="utf-8").replace("self.assertEqual", "self.assertFalse(Path('evidence/fail-now').exists())\n        self.assertEqual"), encoding="utf-8", newline="\n")
        self.commit("test external runtime failure")
        self.finish()
        source = self.w.snapshot()
        (self.repo / "evidence/fail-now").touch()
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(self.w.verify(["all"]), 1)
        self.assertEqual(self.w.snapshot(), source)
        with self.assertRaisesRegex(self.w.WorkflowError, "verify"):
            self.w.done("T0.1")

    def test_interrupted_verify_cannot_reuse_previous_success(self):
        self.finish()
        with patch.object(self.w, "run_command", side_effect=KeyboardInterrupt), contextlib.redirect_stdout(io.StringIO()) as output:
            with self.assertRaises(KeyboardInterrupt):
                self.w.verify(["all"])
        self.assertIn("verify all FAIL: exit=130", output.getvalue())
        with self.assertRaisesRegex(self.w.WorkflowError, "verify"):
            self.w.done("T0.1")

    def test_completion_waits_for_matching_source_commit(self):
        self.finish()
        self.assertEqual(self.w.next_card()["id"], "T0.1")
        self.commit("T0.1: complete")
        with self.assertRaises(self.w.WorkflowError) as caught:
            self.w.next_card()
        self.assertEqual(caught.exception.code, 10)

    def test_latest_failed_acceptance_after_commit_reopens_card(self):
        path = self.repo / "tests/test_behavior.py"
        path.write_text(path.read_text(encoding="utf-8").replace("self.assertEqual", "self.assertFalse(Path('evidence/fail-now').exists())\n        self.assertEqual"), encoding="utf-8", newline="\n")
        self.commit("test external acceptance failure")
        self.finish()
        self.commit("T0.1: validated before runtime change")
        source = self.w.snapshot()
        (self.repo / "evidence/fail-now").touch()
        with contextlib.redirect_stdout(io.StringIO()):
            failed = self.w.run_command("A1", ARGV)
        self.assertEqual(failed["exit"], 1)
        self.assertEqual(self.w.snapshot(), source)
        self.assertEqual(self.w.latest_verify("T0.1", 0)["exit"], 0)
        self.assertFalse(self.w.completed(self.w.get_card("T0.1")))
        self.assertEqual(self.w.next_card()["id"], "T0.1")

    def test_completed_card_rechecks_acceptance_logs(self):
        result = self.finish()
        self.commit("T0.1: validated before evidence cleanup")
        (self.repo / result["log"]).unlink()
        with self.assertRaisesRegex(self.w.WorkflowError, "日志.*重新验收"):
            self.w.next_card()

    def test_completed_card_rechecks_verify_logs(self):
        self.finish()
        self.commit("T0.1: validated before verify log edit")
        verification = self.w.latest_verify("T0.1", 0)
        run = self.w.read_json(self.repo / verification["runs"][0])
        with (self.repo / run["log"]).open("a", encoding="utf-8", newline="\n") as stream:
            stream.write("edited")
        with self.assertRaisesRegex(self.w.WorkflowError, "日志.*重新验收"):
            self.w.next_card()

    def test_completed_card_requires_all_verify_receipts(self):
        self.finish()
        self.commit("T0.1: validated before verify receipt loss")
        verification = self.w.latest_verify("T0.1", 0)
        (self.repo / verification["runs"][0]).unlink()
        with self.assertRaisesRegex(self.w.WorkflowError, "运行收据缺失.*重新完整验证"):
            self.w.next_card()

    def test_completed_card_rejects_truncated_complete_verify(self):
        self.finish()
        self.commit("T0.1: validated before verify receipt truncation")
        path = sorted((self.repo / "evidence/verify").glob("*.json"))[-1]
        verification = self.w.read_json(path)
        verification["runs"] = []
        self.w.write_json(path, verification)
        self.assertFalse(self.w.completed(self.w.get_card("T0.1")))
        self.assertEqual(self.w.next_card()["id"], "T0.1")

    def test_completed_card_rechecks_junit_artifacts(self):
        code = "from pathlib import Path; Path('evidence/result.xml').write_text('<testsuite><testcase name=\"behavior\"/></testsuite>', encoding='utf-8')"
        check = dict(id="A1", description="生成真实非零测试报告", argv=["{python}", "-c", code], kind="test", report="junit:evidence/result.xml")
        (self.repo / "docs/TASKS.md").write_text(card_text(check=check), encoding="utf-8", newline="\n")
        self.commit("configure JUnit acceptance")
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", check["argv"])
            self.assertEqual(result["exit"], 0, result)
            self.assertEqual(self.w.verify(["all"]), 0)
        self.record(result)
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.done("T0.1")
        self.commit("T0.1: complete with JUnit artifact")
        (self.repo / result["report_artifact"]).unlink()
        with self.assertRaisesRegex(self.w.WorkflowError, "测试报告.*重新验收"):
            self.w.next_card()

    def test_earlier_completion_survives_later_source_and_authorized_gate_changes(self):
        (self.repo / "docs/TASKS.md").write_text(card_text() + "\n" + card_text("T0.2", "T0.1"), encoding="utf-8", newline="\n")
        self.commit("plan two cards with later baseline revision")
        self.finish()
        self.commit("T0.1: complete original gates")
        first_source = self.w.read_json(self.w.card_dir("T0.1") / "done.json")["source"]
        self.w.start("T0.2")
        path = self.repo / "docs/PROGRESS.md"
        with path.open("a", encoding="utf-8", newline="\n") as stream:
            stream.write("\n## T0.2 blocked\n- 结果：BLOCKED\n- 原因：需要新增验证门槛\n")
        cfg_path = self.repo / "workflow.json"
        cfg = self.w.read_json(cfg_path)
        cfg["verify"].append(dict(id="verify-additional", argv=["{python}", "-c", "assert 2 + 2 == 4"], kind="command"))
        self.w.write_json(cfg_path, cfg)
        frozen = self.w.protected_hash()
        with path.open("a", encoding="utf-8", newline="\n") as stream:
            stream.write(f"\n## T0.2 resolved\n- 结果：RESOLVED\n- 决定：用户批准新增门槛。\n- 批准摘要：{frozen}\n")
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.rebaseline("T0.2", frozen)
        (self.repo / "src/extra.txt").write_text("second card source", encoding="utf-8", newline="\n")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
            self.assertEqual(result["exit"], 0, result)
            self.assertEqual(self.w.verify(["all"]), 0)
        tasks = self.repo / "docs/TASKS.md"
        tasks.write_text(tasks.read_text(encoding="utf-8").replace("- [ ] 完成", "- [x] 完成"), encoding="utf-8", newline="\n")
        with path.open("a", encoding="utf-8", newline="\n") as stream:
            stream.write(f"\n## T0.2 fixture\n- 结果：DONE\n- 验收 A1：PASS；日志：`{result['log']}`；断言：真实1测试\n")
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.done("T0.2")
        self.commit("T0.2: complete revised gates and source")
        self.assertNotEqual(self.w.snapshot(), first_source)
        self.assertTrue(self.w.completed(self.w.get_card("T0.1")))
        with self.assertRaises(self.w.WorkflowError) as caught:
            self.w.next_card()
        self.assertEqual(caught.exception.code, 10)

    def test_completion_commit_must_include_task_and_progress(self):
        self.finish()
        self.git("commit", "--allow-empty", "-m", "T0.1: missing completion documents")
        self.assertEqual(self.w.next_card()["id"], "T0.1")
        self.commit("T0.1: complete documents")
        with self.assertRaises(self.w.WorkflowError) as caught:
            self.w.next_card()
        self.assertEqual(caught.exception.code, 10)

    def test_commit_with_unverified_source_is_rejected(self):
        self.finish()
        (self.repo / "src/extra.txt").write_text("B", encoding="utf-8", newline="\n")
        self.commit("T0.1: changed after validation")
        self.assertEqual(self.w.next_card()["id"], "T0.1")

    def test_acceptance_invalidated_even_with_preserved_mtime(self):
        result = self.accept()
        path = self.repo / "src/extra.txt"
        timestamp = path.stat().st_mtime_ns
        path.write_text("B", encoding="utf-8", newline="\n")
        os.utime(path, ns=(timestamp, timestamp))
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(self.w.verify(["all"]), 0)
        self.record(result)
        with self.assertRaisesRegex(self.w.WorkflowError, "当前源码"):
            self.w.done("T0.1")

    def test_task_contract_and_controller_are_frozen(self):
        self.w.start("T0.1")
        path = self.repo / "docs/TASKS.md"
        path.write_text(path.read_text(encoding="utf-8").replace("固定输入输出", "新验收"), encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "未授权"):
            self.w.check_frozen("T0.1")

    def approve_revision(self):
        path = self.repo / "docs/PROGRESS.md"
        with path.open("a", encoding="utf-8", newline="\n") as stream:
            stream.write("\n## T0.1 blocked\n- 结果：BLOCKED\n- 原因：契约需要修订\n")
        architecture = self.repo / "docs/ARCHITECTURE.md"
        architecture.write_text(architecture.read_text(encoding="utf-8") + "\n用户批准的契约修订。\n", encoding="utf-8", newline="\n")
        frozen = self.w.protected_hash()
        with path.open("a", encoding="utf-8", newline="\n") as stream:
            stream.write(f"\n## T0.1 resolved\n- 结果：RESOLVED\n- 决定：用户批准上述契约修订。\n- 批准摘要：{frozen}\n")
        return frozen

    def prepare_controlled_recovery(self, marker=True):
        self.w.start("T0.1")
        node = self.w.state("T0.1")
        node.update(sessions=1, hard_sessions=[0, 1], failures=[dict(time=1, signature="retained", session=1, exit=1, log="retained.log")], last_note=2)
        self.w.write_json(self.w.card_dir("T0.1") / "state.json", node)
        frozen = self.approve_revision()
        if marker:
            with (self.repo / "docs/PROGRESS.md").open("a", encoding="utf-8") as stream:
                stream.write("- 受控恢复：1->2\n")
        self.w.rebaseline("T0.1", frozen)
        return frozen, self.w.state("T0.1")

    def test_controlled_recovery_preserves_history_and_refuses_repeat(self):
        frozen, before = self.prepare_controlled_recovery()
        with (self.repo / "docs/PROGRESS.md").open("a", encoding="utf-8") as stream:
            stream.write("\n\n## T0.1 文档独立审查\n- 文档结论：CLEAR\n")
        self.w.task(["resume-hard", "T0.1", "--protected", frozen])
        after = self.w.state("T0.1")
        self.assertEqual(after["sessions"], 2)
        self.assertEqual(after["failures"], before["failures"])
        self.assertEqual(after["hard_sessions"], before["hard_sessions"])
        self.assertEqual(after["generation"], before["generation"])
        with self.assertRaisesRegex(self.w.WorkflowError, "已经使用"):
            self.w.resume_hard("T0.1", frozen)
        self.assertEqual(self.w.state("T0.1"), after)

    def test_controlled_recovery_requires_explicit_bound_approval(self):
        frozen, before = self.prepare_controlled_recovery(marker=False)
        with self.assertRaisesRegex(self.w.WorkflowError, "明确单次恢复"):
            self.w.resume_hard("T0.1", frozen)
        self.assertEqual(self.w.state("T0.1"), before)

    def test_controlled_recovery_rejects_digest_budget_and_unfinished_run(self):
        frozen, before = self.prepare_controlled_recovery()
        with self.assertRaisesRegex(self.w.WorkflowError, "摘要"):
            self.w.resume_hard("T0.1", "0" * 64)
        with patch.object(self.w, "config", return_value={**self.w.config(), "max_sessions": 1}):
            with self.assertRaisesRegex(self.w.WorkflowError, "额度"):
                self.w.resume_hard("T0.1", frozen)
        with patch.object(self.w, "receipts", return_value=[dict(status="running")]):
            with self.assertRaisesRegex(self.w.WorkflowError, "重命令"):
                self.w.resume_hard("T0.1", frozen)
        self.assertEqual(self.w.state("T0.1"), before)

    def test_authorized_rebaseline_resumes_without_resetting_history(self):
        self.w.start("T0.1")
        node = self.w.state("T0.1")
        node.update(sessions=1, failures=[dict(time=1, signature="retained", session=1, exit=1, log="retained.log")], last_note=2)
        self.w.write_json(self.w.card_dir("T0.1") / "state.json", node)
        frozen = self.approve_revision()
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.task(["rebaseline", "T0.1", "--protected", frozen])
        revised = self.w.state("T0.1")
        self.assertEqual(revised["sessions"], node["sessions"])
        self.assertEqual(revised["failures"], node["failures"])
        self.assertEqual(revised["base"], node["base"])
        self.assertEqual(self.w.start("T0.1")["id"], "T0.1")
        result = self.accept()
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(self.w.verify(["all"]), 0)
        self.record(result)
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.done("T0.1")
        self.commit("T0.1: revised contract complete")
        with self.assertRaises(self.w.WorkflowError) as caught:
            self.w.next_card()
        self.assertEqual(caught.exception.code, 10)

    def prepare_paused_migration(self):
        tasks = self.repo / "docs/TASKS.md"
        tasks.write_text(card_text() + card_text("T0.2").replace("`src/`", "`src/extra.txt`") +
                         card_text("T0.3").replace("`src/`", "`src/extra.txt`"), encoding="utf-8")
        self.commit("three card fixture")
        self.w.start("T0.1")
        original = self.w.state("T0.1")
        original.update(sessions=2, failures=[dict(signature="retained", exit=1)], last_note=123)
        self.w.write_json(self.w.card_dir("T0.1") / "state.json", original)
        for cid in ("T0.2", "T0.3"):
            node = json.loads(json.dumps(original))
            node.update(paused=True, sessions=1, failures=[], segments=[dict(base=node["base"], end=node["base"])])
            self.w.write_json(self.w.card_dir(cid) / "state.json", node)
        frozen = self.approve_revision()
        with (self.repo / "docs/PROGRESS.md").open("a", encoding="utf-8") as f:
            f.write("- 迁移暂停卡：T0.2,T0.3\n\n## T0.2 ordering\n- 结果：BLOCKED\n")
        return frozen, {cid: self.w.state(cid) for cid in ("T0.1", "T0.2", "T0.3")}

    def migrate_paused(self, frozen):
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.task(["rebaseline", "T0.1", "--protected", frozen, "--include-paused", "T0.2,T0.3"])

    def test_paused_migration_preserves_history_and_holds_both_locks(self):
        frozen, before = self.prepare_paused_migration()
        save = self.w.write_json
        def guarded_save(path, value):
            for lock_path in (self.repo / "evidence/drive/driver.lock", self.w.heavy_path()):
                with self.w.runtime.FileLock(lock_path) as lock:
                    self.assertFalse(lock.acquire())
            save(path, value)
        with patch.object(self.w, "write_json", side_effect=guarded_save):
            self.migrate_paused(frozen)
        for cid, old in before.items():
            node = self.w.state(cid)
            self.assertEqual(node["protected"], frozen)
            self.assertEqual(node["generation"], old.get("generation", 0) + 1)
            for field in ("base", "sessions", "failures", "last_note", "segments", "paused", "resume_base"):
                self.assertEqual(node.get(field), old.get(field))
        self.assertEqual(self.w.current(), "T0.1")
        self.assertIn("T0.2", self.w.blocked())
        after = {cid: self.w.state(cid) for cid in before}
        self.migrate_paused(frozen)
        self.assertEqual(after, {cid: self.w.state(cid) for cid in before})

    def test_paused_migration_recovers_partial_save_once(self):
        frozen, before = self.prepare_paused_migration()
        save = self.w.write_json
        def interrupted_save(path, value):
            if path == self.w.card_dir("T0.2") / "state.json":
                raise OSError("simulated interrupted save")
            save(path, value)
        with patch.object(self.w, "write_json", side_effect=interrupted_save):
            with self.assertRaisesRegex(OSError, "interrupted"):
                self.migrate_paused(frozen)
        self.assertEqual(self.w.state("T0.1")["protected"], frozen)
        self.assertEqual(self.w.state("T0.2")["protected"], before["T0.2"]["protected"])
        self.migrate_paused(frozen)
        for cid, old in before.items():
            node = self.w.state(cid)
            self.assertEqual(node["generation"], old.get("generation", 0) + 1)
            self.assertEqual(len(node["rebaselines"]), 1)

    def test_default_rebaseline_still_refuses_other_unfinished_cards(self):
        frozen, before = self.prepare_paused_migration()
        with self.assertRaisesRegex(self.w.WorkflowError, "其他已开始"):
            self.w.rebaseline("T0.1", frozen)
        self.assertEqual(before, {cid: self.w.state(cid) for cid in before})

    def test_paused_migration_rejects_invalid_set_or_approval_before_writing(self):
        frozen, before = self.prepare_paused_migration()
        for included, digest in ((["T0.2"], frozen), (["T0.2", "T0.2"], frozen),
                                 (["T0.1", "T0.2", "T0.3"], frozen),
                                 (["T0.2", "T0.3"], "0" * 64)):
            with self.subTest(included=included, digest=digest), self.assertRaises(self.w.WorkflowError):
                self.w.rebaseline("T0.1", digest, included)
            self.assertEqual(before, {cid: self.w.state(cid) for cid in before})
        path = self.repo / "docs/PROGRESS.md"
        path.write_text(path.read_text().replace("- 迁移暂停卡：T0.2,T0.3", "- 迁移暂停卡：T0.2"))
        with self.assertRaisesRegex(self.w.WorkflowError, "集合"):
            self.migrate_paused(frozen)
        self.assertEqual(before, {cid: self.w.state(cid) for cid in before})

    def test_paused_migration_rejects_active_card_and_modified_prefix(self):
        frozen, before = self.prepare_paused_migration()
        node = self.w.state("T0.2");node["paused"] = False
        self.w.write_json(self.w.card_dir("T0.2") / "state.json", node)
        with self.assertRaisesRegex(self.w.WorkflowError, "暂停"):
            self.migrate_paused(frozen)
        self.w.write_json(self.w.card_dir("T0.2") / "state.json", before["T0.2"])
        path = self.repo / "docs/PROGRESS.md"
        path.write_text(path.read_text().replace("# PROGRESS", "# altered", 1))
        with self.assertRaisesRegex(self.w.WorkflowError, "只追加"):
            self.migrate_paused(frozen)
        self.assertEqual(before, {cid: self.w.state(cid) for cid in before})

    def test_paused_migration_rejects_driver_agent_unfinished_run_and_locks(self):
        frozen, before = self.prepare_paused_migration()
        for state in (dict(pid=os.getpid(), identity=self.w.identity(os.getpid())),
                      dict(agent_pid=os.getpid(), agent_identity=self.w.identity(os.getpid()))):
            self.w.write_json(self.repo / "evidence/drive/state.json", state)
            with self.assertRaisesRegex(self.w.WorkflowError, "进程"):
                self.migrate_paused(frozen)
        self.w.write_json(self.repo / "evidence/drive/state.json", {})
        receipt = self.repo / "evidence/runs/pending/receipt.json"
        self.w.write_json(receipt, dict(card="T0.1", status="queued", created=1, log="unused"))
        with self.assertRaisesRegex(self.w.WorkflowError, "重命令"):
            self.migrate_paused(frozen)
        receipt.unlink()
        for path in (self.repo / "evidence/drive/driver.lock", self.w.heavy_path()):
            with self.w.runtime.FileLock(path) as lock:
                self.assertTrue(lock.acquire())
                with self.assertRaisesRegex(self.w.WorkflowError, "锁"):
                    self.migrate_paused(frozen)
        self.assertEqual(before, {cid: self.w.state(cid) for cid in before})

    def test_paused_scope_excludes_other_cards_commits_but_checks_uncommitted_changes(self):
        frozen, before = self.prepare_paused_migration()
        (self.repo / "src/value.txt").write_text("other card work")
        self.commit("T0.1: later work")
        self.assertFalse(self.w.scope_problems("T0.2"))
        (self.repo / "src/value.txt").write_text("uncommitted outside paused scope")
        with self.assertRaisesRegex(self.w.WorkflowError, "范围外"):
            self.migrate_paused(frozen)
        self.assertEqual(before, {cid: self.w.state(cid) for cid in before})

    def test_paused_migration_rejects_mixed_original_baselines(self):
        frozen, before = self.prepare_paused_migration()
        node = self.w.state("T0.2");node["protected"] = "0" * 64
        self.w.write_json(self.w.card_dir("T0.2") / "state.json", node)
        before["T0.2"] = node
        with self.assertRaisesRegex(self.w.WorkflowError, "原冻结基线"):
            self.migrate_paused(frozen)
        self.assertEqual(before, {cid: self.w.state(cid) for cid in before})

    def test_paused_migration_rejects_live_payload_from_finished_receipt(self):
        frozen, before = self.prepare_paused_migration()
        receipt = self.repo / "evidence/runs/finished/receipt.json"
        self.w.write_json(receipt, dict(card="T0.1", status="finished", created=1, log="unused",
            payload_refs=[dict(pid=os.getpid(), identity=self.w.identity(os.getpid()))]))
        with self.assertRaisesRegex(self.w.WorkflowError, "重命令"):
            self.migrate_paused(frozen)
        self.assertEqual(before, {cid: self.w.state(cid) for cid in before})

    def test_rebaseline_requires_matching_approval_and_stopped_driver(self):
        self.w.start("T0.1")
        frozen = self.approve_revision()
        before = self.w.state("T0.1")
        for value in ("0" * 64, frozen):
            with self.subTest(protected=value):
                if value == frozen:
                    path = self.repo / "docs/PROGRESS.md"
                    path.write_text(path.read_text(encoding="utf-8").replace("- 批准摘要：", "- 无效摘要："), encoding="utf-8", newline="\n")
                with self.assertRaises(self.w.WorkflowError):
                    self.w.task(["rebaseline", "T0.1", "--protected", value])
                self.assertEqual(self.w.state("T0.1"), before)
        path.write_text(path.read_text(encoding="utf-8").replace("- 无效摘要：", "- 批准摘要："), encoding="utf-8", newline="\n")
        self.w.write_json(self.repo / "evidence/drive/state.json", dict(pid=os.getpid(), identity=self.w.identity(os.getpid())))
        with self.assertRaisesRegex(self.w.WorkflowError, "驱动器"):
            self.w.task(["rebaseline", "T0.1", "--protected", frozen])
        self.w.write_json(self.repo / "evidence/drive/state.json", {})

    def test_rebaseline_invalidates_old_proofs_without_deleting_evidence(self):
        old = self.finish()
        frozen = self.approve_revision()
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.task(["rebaseline", "T0.1", "--protected", frozen])
        self.assertTrue((self.repo / old["log"]).is_file())
        self.record(old)
        with self.assertRaisesRegex(self.w.WorkflowError, "当前源码"):
            self.w.done("T0.1")
        with self.assertRaises(self.w.WorkflowError):
            self.w.task(["rebaseline", "T0.1", "--protected", frozen])

    def test_rebaseline_does_not_authorize_unrelated_scope_changes(self):
        self.w.start("T0.1")
        frozen = self.approve_revision()
        (self.repo / "unrelated.txt").write_text("outside the approved revision", encoding="utf-8", newline="\n")
        before = self.w.state("T0.1")
        with self.assertRaisesRegex(self.w.WorkflowError, "范围外"):
            self.w.task(["rebaseline", "T0.1", "--protected", frozen])
        self.assertEqual(self.w.state("T0.1"), before)

    def test_rebaseline_refuses_unfinished_commands(self):
        self.w.start("T0.1")
        frozen = self.approve_revision()
        folder = self.repo / "evidence/runs/unfinished"
        folder.mkdir(parents=True)
        log = folder / "output.log"
        log.touch()
        run = dict(card="T0.1", created=time.time(), status="queued", log=str(log.relative_to(self.repo)))
        self.w.write_json(folder / "receipt.json", run)
        with self.assertRaisesRegex(self.w.WorkflowError, "重命令"):
            self.w.task(["rebaseline", "T0.1", "--protected", frozen])

    def test_rebaseline_cannot_reuse_old_full_verify_after_new_acceptance(self):
        self.finish()
        frozen = self.approve_revision()
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.task(["rebaseline", "T0.1", "--protected", frozen])
        fresh = self.accept()
        self.record(fresh)
        with self.assertRaisesRegex(self.w.WorkflowError, "verify"):
            self.w.done("T0.1")

    def test_append_only_history_rewrite_is_rejected(self):
        self.w.start("T0.1")
        (self.repo / "docs/PROGRESS.md").write_text("erased", encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "只追加"):
            self.w.check_frozen("T0.1")

    def test_scope_matches_directory_boundary(self):
        self.w.start("T0.1")
        (self.repo / "src-evil").mkdir()
        (self.repo / "src-evil/file.txt").write_text("unexpected", encoding="utf-8", newline="\n")
        self.assertEqual(self.w.scope_problems("T0.1"), ["src-evil/file.txt"])

    def test_blocked_requires_matching_resolution(self):
        path = self.repo / "docs/PROGRESS.md"
        with path.open("a", encoding="utf-8", newline="\n") as stream:
            stream.write("\n## T0.1 blocked\n- 结果：BLOCKED\n原因\n## unrelated\n- 结果：DONE\n")
        with self.assertRaises(self.w.WorkflowError) as caught:
            self.w.next_card()
        self.assertEqual(caught.exception.code, 11)
        with path.open("a", encoding="utf-8", newline="\n") as stream:
            stream.write("\n## T0.1 resolved\n- 结果：RESOLVED\n用户决定\n")
        self.assertEqual(self.w.next_card()["id"], "T0.1")

    def append_progress(self, text):
        with (self.repo / "docs/PROGRESS.md").open("a", encoding="utf-8", newline="\n") as stream:
            stream.write(text)

    def test_blocked_card_only_freezes_its_branch(self):
        (self.repo / "docs/TASKS.md").write_text(card_text() + card_text("T0.2", "T0.1") + card_text("T0.3"), encoding="utf-8", newline="\n")
        self.append_progress("\n## T0.1 blocked\n- 结果：BLOCKED\n原因\n")
        self.assertEqual(self.w.next_card()["id"], "T0.3")
        self.append_progress("\n## T0.1 resolved\n- 结果：RESOLVED\n用户决定\n")
        self.assertEqual(self.w.next_card()["id"], "T0.1")

    def test_block_without_known_card_stops_everything(self):
        (self.repo / "docs/TASKS.md").write_text(card_text() + card_text("T0.3"), encoding="utf-8", newline="\n")
        for heading in ("全局决定", "T9.9 typo"):
            self.append_progress(f"\n## {heading}\n- 结果：BLOCKED\n原因\n")
            with self.assertRaises(self.w.WorkflowError) as caught:
                self.w.next_card()
            self.assertEqual(caught.exception.code, 11)
            self.assertIn("全局", str(caught.exception))
            self.append_progress(f"\n## {heading}\n- 结果：RESOLVED\n用户决定\n")

    def test_blocked_active_card_moves_on_only_with_clean_tree_and_keeps_own_scope(self):
        other = card_text("T0.3").replace("`src/`、`tests/`", "`other/`")
        (self.repo / "docs/TASKS.md").write_text(card_text() + other, encoding="utf-8", newline="\n")
        self.commit("plan independent cards")
        self.w.start("T0.1")
        (self.repo / "src/extra.txt").write_text("B", encoding="utf-8", newline="\n")
        self.append_progress("\n## T0.1 blocked\n- 结果：BLOCKED\n需要用户决定\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "未提交") as caught:
            self.w.next_card()
        self.assertEqual(caught.exception.code, 11)
        self.commit("WIP T0.1: handoff")
        self.assertEqual(self.w.next_card()["id"], "T0.3")
        self.w.start("T0.3")
        self.assertTrue(self.w.state("T0.1")["paused"])
        (self.repo / "other").mkdir()
        (self.repo / "other/x.txt").write_text("x", encoding="utf-8", newline="\n")
        self.commit("WIP T0.3: other work")
        self.append_progress("\n## T0.1 resolved\n- 结果：RESOLVED\n用户决定\n")
        self.assertEqual(self.w.next_card()["id"], "T0.3", "an active unblocked card keeps priority")
        self.append_progress("\n## T0.3 blocked\n- 结果：BLOCKED\n需要用户决定\n")
        self.commit("docs: record decisions")
        self.assertEqual(self.w.next_card()["id"], "T0.1")
        self.w.start("T0.1")
        self.assertEqual(self.w.scope_problems("T0.1"), [])
        (self.repo / "other/y.txt").write_text("y", encoding="utf-8", newline="\n")
        self.assertEqual(self.w.scope_problems("T0.1"), ["other/y.txt"])
        self.assertEqual(self.w.state("T0.3")["segments"][0]["end"], self.git("rev-parse", "HEAD"))

    def test_phase_gate_waits_for_recorded_human_authorization(self):
        gated = card_text().replace("- 依赖：无\n", "- 依赖：无\n- 阶段门：移动阶段需人类授权\n")
        path = self.repo / "docs/TASKS.md"
        path.write_text(gated + card_text("T0.2"), encoding="utf-8", newline="\n")
        self.assertEqual(self.w.get_card("T0.1")["gate"], "移动阶段需人类授权")
        self.assertEqual(self.w.next_card()["id"], "T0.2")
        path.write_text(gated, encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "阶段门") as caught:
            self.w.next_card()
        self.assertEqual(caught.exception.code, 11)
        self.append_progress("\n## T0.1 移动授权\n- 结果：AUTHORIZED\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "阶段门"):
            self.w.next_card()
        self.append_progress("\n## T0.1 移动授权\n- 结果：AUTHORIZED\n- 授权：用户原话“开始移动阶段”\n")
        self.assertEqual(self.w.next_card()["id"], "T0.1")
        path.write_text(gated.replace("移动阶段需人类授权", " "), encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "阶段门"):
            self.w.cards()

    def junit_check(self, **extra):
        check = dict(id="A1", description="冻结用例逐项通过", argv=["{python}", "-B", "tests/write_junit.py"], kind="test",
                     report="junit:evidence/acceptance/A1.xml", cases=["R1/a", "R2/b"], skippable=["R2/b"])
        check.update(extra)
        return {key: value for key, value in check.items() if value is not None}

    def test_cases_definition_is_validated(self):
        path = self.repo / "docs/TASKS.md"
        for check, message in ((dict(id="A1", description="x", argv=ARGV, kind="test", report="unittest", cases=["R1/a"]), "JUnit"),
                               (self.junit_check(cases=["R1/a", "R1/a"], skippable=None), "唯一"),
                               (self.junit_check(cases=[], skippable=None), "唯一"),
                               (self.junit_check(skippable=["R9/z"]), "子集"),
                               (self.junit_check(cases=None), "skippable")):
            path.write_text(card_text(check=check), encoding="utf-8", newline="\n")
            with self.assertRaisesRegex(self.w.WorkflowError, message):
                self.w.cards()

    def test_frozen_cases_must_each_pass_unless_skippable(self):
        (self.repo / "tests/write_junit.py").write_text(
            "from pathlib import Path\nout = Path('evidence/acceptance/A1.xml')\nout.parent.mkdir(parents=True, exist_ok=True)\n"
            "out.write_text(Path('tests/cases.xml').read_text(encoding='utf-8'), encoding='utf-8')\n", encoding="utf-8", newline="\n")
        cases = self.repo / "tests/cases.xml"
        (self.repo / "docs/TASKS.md").write_text(card_text(check=self.junit_check()), encoding="utf-8", newline="\n")
        cases.write_text('<testsuite><testcase name="R1/a"/><testcase name="R2/b"><skipped/></testcase></testsuite>', encoding="utf-8", newline="\n")
        self.commit("junit cases card")
        self.w.start("T0.1")
        check = self.w.get_card("T0.1")["checks"][0]
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", check["argv"])
        self.assertEqual(result["exit"], 0, result)
        self.w.verify_evidence(result)
        for xml, problem in (('<testsuite><testcase name="R1/a"/><testcase name="extra"/></testsuite>', "R2/b 未执行"),
                             ('<testsuite><testcase name="R1/a"><skipped/></testcase><testcase name="R2/b"/></testsuite>', "R1/a 被跳过")):
            cases.write_text(xml, encoding="utf-8", newline="\n")
            self.w.task(["note", "调整用例报告"])
            with contextlib.redirect_stdout(io.StringIO()):
                result = self.w.run_command("A1", check["argv"])
            self.assertEqual(result["exit"], 3, result)
            self.assertIn(problem, result["note"])
        artifact = self.repo / result["report_artifact"]
        self.assertEqual(self.w.case_problems(check, artifact.read_bytes()), ["R1/a 被跳过但未声明 skippable"])

    def test_environment_field_and_unique_junit_reports(self):
        path = self.repo / "docs/TASKS.md"
        with_env = card_text().replace("- 依赖：无\n", "- 依赖：无\n- 环境：macos-arm64 必需；linux 记录\n")
        path.write_text(with_env, encoding="utf-8", newline="\n")
        self.assertEqual(self.w.get_card("T0.1")["environment"], "macos-arm64 必需；linux 记录")
        path.write_text(with_env.replace("macos-arm64 必需；linux 记录", ""), encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "环境"):
            self.w.cards()
        shared = self.junit_check(cases=None, skippable=None)
        path.write_text(card_text(check=shared) + card_text("T0.2", check=shared), encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "唯一"):
            self.w.cards()

    def test_adr_digests_stay_stable_when_later_adr_is_appended(self):
        path = self.repo / "docs/DECISIONS.md"
        path.write_text("# 决策\n\n## ADR-001 决定\n内容一\n", encoding="utf-8", newline="\n")
        first = self.w.adr_digests()
        path.write_text("# 决策\r\n\r\n## ADR-001 决定\r\n内容一\r\n\r\n## ADR-002 新决定\r\n内容二\r\n", encoding="utf-8", newline="")
        second = self.w.adr_digests()
        self.assertEqual(first["ADR-001"], second["ADR-001"])
        self.assertEqual(sorted(second), ["ADR-001", "ADR-002"])
        result = self.command("task", "status")
        self.assertEqual(json.loads(result.stdout)["adr"], second)

    def test_check_rejects_acceptance_beyond_session_budget(self):
        cfg_path = self.repo / "workflow.json"
        cfg = json.loads(cfg_path.read_text(encoding="utf-8"))
        cfg.update(session_seconds=600, max_sessions=2)
        cfg["verify"][0]["timeout_seconds"] = 100
        cfg_path.write_text(json.dumps(cfg), encoding="utf-8", newline="\n")
        check = dict(id="A1", description="长测", argv=ARGV, kind="test", report="unittest", timeout_seconds=900)
        (self.repo / "docs/TASKS.md").write_text(card_text(check=check), encoding="utf-8", newline="\n")
        result = self.command("task", "check")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("须跨会话续跑", result.stdout)
        check["timeout_seconds"] = 1200
        (self.repo / "docs/TASKS.md").write_text(card_text(check=check), encoding="utf-8", newline="\n")
        result = self.command("task", "check")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("会话预算", result.stderr)

    def test_committed_review_reports_are_records_outside_source_and_scope(self):
        self.w.start("T0.1")
        before = self.w.snapshot()
        report = self.repo / "docs/reviews/doc-review/round-1.md"
        report.parent.mkdir(parents=True)
        report.write_text("DOC-REVIEW-VERDICT: CLEAR\n", encoding="utf-8", newline="\n")
        self.assertEqual(self.w.snapshot(), before)
        self.assertEqual(self.w.scope_problems("T0.1"), [])
        self.commit("docs: record review")
        self.assertEqual(self.w.snapshot(self.git("rev-parse", "HEAD")), before)

    def test_exact_acceptance_argv_is_required(self):
        self.w.start("T0.1")
        with self.assertRaisesRegex(self.w.WorkflowError, "argv"):
            self.w.run_command("A1", ["python3", "-c", "print('fake')"])

    def test_verify_ids_cannot_override_card_test_requirements(self):
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        argv = ["python3", "-c", "assert 1 + 1 == 2"]
        cfg["verify"] = [dict(id="A1", argv=argv, kind="test", report="unittest")]
        path.write_text(json.dumps(cfg), encoding="utf-8", newline="\n")
        (self.repo / "docs/TASKS.md").write_text(card_text(check=dict(id="A1", description="CLI断言", argv=argv, kind="command")), encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "verify-"):
            self.w.verify(["all"])
        self.assertEqual(self.w.receipts(), [])

    def test_zero_and_all_skipped_tests_do_not_pass(self):
        (self.repo / "tests/test_behavior.py").write_text("import unittest\nclass Skipped(unittest.TestCase):\n    @unittest.skip('fixture')\n    def test_skipped(self):\n        pass\n", encoding="utf-8", newline="\n")
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
        self.assertEqual(result["raw_exit"], 0)
        self.assertEqual(result["exit"], 3)
        counts = self.w.test_counts("Ran 3 tests in 0.01s\nOK (skipped=3)\n", "unittest", 0, None)
        self.assertEqual(counts["passed"], 0)

    def test_empty_discovery_is_not_success(self):
        (self.repo / "tests/test_behavior.py").write_text("import unittest\n", encoding="utf-8", newline="\n")
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
        self.assertNotEqual(result["exit"], 0)
        self.assertEqual(result["tests"]["passed"], 0)

    def test_unittest_expected_failures_do_not_hide_real_passes(self):
        source = BEHAVIOR + "\n    @unittest.expectedFailure\n    def test_known_issue(self):\n        self.assertEqual(1, 2)\n"
        (self.repo / "tests/test_behavior.py").write_text(source, encoding="utf-8", newline="\n")
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
        self.assertEqual(result["raw_exit"], 0)
        self.assertEqual(result["exit"], 0, result)
        self.assertEqual(result["tests"], dict(passed=1, failed=0, skipped=0, expected_failures=1, unexpected_successes=0))

    def test_unittest_only_expected_failures_has_zero_real_passes(self):
        source = "import unittest\nclass KnownIssue(unittest.TestCase):\n    @unittest.expectedFailure\n    def test_known_issue(self):\n        self.assertEqual(1, 2)\n"
        (self.repo / "tests/test_behavior.py").write_text(source, encoding="utf-8", newline="\n")
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
        self.assertEqual(result["raw_exit"], 0)
        self.assertEqual(result["exit"], 3)
        self.assertEqual(result["tests"]["passed"], 0)
        self.assertEqual(result["tests"]["failed"], 0)
        self.assertEqual(result["tests"]["expected_failures"], 1)

    def test_unittest_unexpected_success_is_a_real_failure(self):
        source = BEHAVIOR + "\n    @unittest.expectedFailure\n    def test_unexpected_pass(self):\n        self.assertEqual(1, 1)\n"
        (self.repo / "tests/test_behavior.py").write_text(source, encoding="utf-8", newline="\n")
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
        self.assertEqual(result["raw_exit"], 1)
        self.assertEqual(result["exit"], 1)
        self.assertEqual(result["tests"], dict(passed=1, failed=1, skipped=0, expected_failures=0, unexpected_successes=1))

    def test_unittest_summary_distinguishes_all_outcome_types(self):
        log = "Ran 7 tests in 0.01s\nFAILED (failures=1, errors=1, skipped=1, expected failures=1, unexpected successes=1)\n"
        self.assertEqual(self.w.test_counts(log, "unittest", 0, None),
                         dict(passed=2, failed=3, skipped=1, expected_failures=1, unexpected_successes=1))

    def test_failed_tests_keep_real_exit_and_require_note(self):
        (self.repo / "src/value.txt").write_text("bad", encoding="utf-8", newline="\n")
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
        self.assertEqual(result["exit"], 1)
        self.assertEqual(result["tests"]["failed"], 1)
        with self.assertRaisesRegex(self.w.WorkflowError, "note"):
            self.w.run_command("A1", ARGV)
        self.assertTrue((self.repo / result["log"]).is_file())

    @unittest.skipUnless(sys.platform == "linux", "systemd is Linux-specific; other platforms test the watchdog")
    def test_scope_failure_never_runs_unbounded(self):
        self.w.start("T0.1")
        with patch.dict(os.environ, FIXTURE_CAP_FAIL="1"), contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", ARGV)
        self.assertEqual(result["exit"], 7)
        self.assertIsNone(result["tests"])

    def test_log_changes_invalidate_done(self):
        result = self.finish()
        with (self.repo / result["log"]).open("a", encoding="utf-8", newline="\n") as stream:
            stream.write("edited")
        with self.assertRaisesRegex(self.w.WorkflowError, "日志"):
            self.w.done("T0.1")

    def test_junit_requires_fresh_nonzero_testcases(self):
        path = self.repo / "evidence/test-results/result.xml"
        path.parent.mkdir(parents=True)
        path.write_text('<testsuite><testcase name="a"/><testcase name="b"><skipped/></testcase></testsuite>', encoding="utf-8", newline="\n")
        metadata = (path.stat().st_mtime_ns, path.stat().st_size)
        with self.assertRaisesRegex(self.w.WorkflowError, "本次"):
            self.w.test_counts("", "junit:evidence/test-results/result.xml", time.time_ns(), metadata)
        counts = self.w.test_counts("", "junit:evidence/test-results/result.xml", time.time_ns(), None)
        self.assertEqual(counts, dict(passed=1, failed=0, skipped=1))

    def test_pytest_quiet_and_colored_summaries_count_real_outcomes(self):
        samples = [
            ("1 passed in 0.03s\n", dict(passed=1, failed=0, skipped=0)),
            ("=== 2 passed, 1 skipped in 0.03s ===\n", dict(passed=2, failed=0, skipped=1)),
            ("\x1b[31m1 failed, 2 passed, 1 error in 0.03s\x1b[0m\n", dict(passed=2, failed=2, skipped=0)),
            ("1 skipped in 0.03s\n", dict(passed=0, failed=0, skipped=1)),
            ("no tests ran in 0.03s\n", dict(passed=0, failed=0, skipped=0)),
            ("tests print: 99 passed\n1 passed in 0.03s\n", dict(passed=1, failed=0, skipped=0)),
        ]
        for log, expected in samples:
            with self.subTest(log=log):
                self.assertEqual(self.w.test_counts(log, "pytest", 0, None), expected)

    def test_corrupt_state_fails_explicitly(self):
        path = self.repo / "evidence/drive/state.json"
        path.parent.mkdir(parents=True)
        path.write_text("{broken", encoding="utf-8", newline="\n")
        result = self.command("drive", "status")
        self.assertEqual(result.returncode, 2)
        self.assertIn("损坏", result.stderr)
        path.unlink()

    def test_nested_wrapper_is_rejected(self):
        with patch.dict(os.environ, AGENT_WORKFLOW_RUN="1"):
            with self.assertRaisesRegex(self.w.WorkflowError, "嵌套"):
                self.w.run_command("A1", ARGV)

    def test_project_gate_failure_prevents_verify_receipt(self):
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["verify"].append(dict(id="verify-unconfigured", argv=["{python}", "scripts/check-project.py"], kind="command"))
        path.write_text(json.dumps(cfg), encoding="utf-8", newline="\n")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.verify(["all"])
        self.assertEqual(result, 2)
        attempts = [self.w.read_json(path) for path in (self.repo / "evidence/verify").glob("*.json")]
        self.assertEqual(len(attempts), 1)
        self.assertEqual(attempts[0]["status"], "finished")
        self.assertEqual(attempts[0]["exit"], 2)
        self.assertFalse(any(attempt["exit"] == 0 for attempt in attempts))

    def test_completed_card_hard_stop_does_not_block_later_unrelated_work(self):
        self.w.start("T0.1")
        node = self.w.state("T0.1")
        node.update(sessions=0, hard_sessions=[0], failures=[{"session":0,"signature":"retained"}])
        self.w.write_json(self.w.card_dir("T0.1") / "state.json", node)
        before = self.w.state("T0.1")
        with patch.object(self.w, "completed", return_value=True):
            with patch.object(self.w.runtime, "require_process_support", side_effect=RuntimeError("reached execution")):
                with self.assertRaisesRegex(RuntimeError, "reached execution"):
                    self.w.run_command("later-check", ["python3", "-c", "pass"])
            with self.assertRaises(self.w.WorkflowError) as caught:
                self.w.run_command("A1", ARGV)
            self.assertEqual(caught.exception.code, 4)
        self.assertEqual(self.w.state("T0.1"), before)

    def test_stuck_hard_preserves_history_and_limits_recovery(self):
        (self.repo / "src/value.txt").write_text("bad", encoding="utf-8", newline="\n")
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            for attempt in range(5):
                if attempt:
                    self.w.task(["note", "固定失败反证：保持同一失败以检查计数器"])
                result = self.w.run_command("A1", ARGV)
                self.assertEqual(result["exit"], 1)
        node = self.w.state("T0.1")
        self.assertEqual(len(node["failures"]), 5)
        self.assertEqual(node["hard_sessions"], [0])
        with self.assertRaises(self.w.WorkflowError) as caught:
            self.w.run_command("A1", ARGV)
        self.assertEqual(caught.exception.code, 4)
        self.assertEqual(self.command("drive", "--once").returncode, 1)
        self.assertEqual(self.command("drive", "--once").returncode, 13)
        self.assertEqual(len(self.w.state("T0.1")["failures"]), 5)

    def test_global_lock_serializes_command_intervals(self):
        code = "import json, time, sys; from pathlib import Path; start=time.time(); time.sleep(.25); Path(sys.argv[1]).write_text(json.dumps([start,time.time()]))"
        output = self.repo / "evidence/intervals"
        output.mkdir(parents=True)
        commands = [[sys.executable, "-B", str(self.repo / "scripts/workflow.py"), "run", "interval-" + str(i),
                     "--", sys.executable, "-c", code, str(output / str(i))] for i in range(2)]
        children, tokens, logs = [], [], []
        try:
            for index, cmd in enumerate(commands):
                log = output / f"caller-{index}.log"
                logs.append(log)
                with log.open("wb") as stream:
                    child = subprocess.Popen(cmd, cwd=self.repo, stdout=stream, stderr=subprocess.STDOUT,
                                             start_new_session=True)
                children.append(child)
                tokens.append(self.w.identity(child.pid))
            for child, log in zip(children, logs):
                self.assertEqual(child.wait(timeout=10), 0, log.read_text(encoding="utf-8", errors="replace"))
            intervals = sorted(json.loads((output / str(i)).read_text(encoding="utf-8")) for i in range(2))
            self.assertLessEqual(intervals[0][1], intervals[1][0])
        finally:
            for child, token in zip(children, tokens):
                self.w.terminate_session(child.pid, token)
                child.wait(timeout=5)

    def test_default_lock_serializes_with_different_TMPDIR(self):
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["heavy_lock"] = None
        path.write_text(json.dumps(cfg), encoding="utf-8", newline="\n")
        output = self.repo / "evidence/default-lock-intervals"
        output.mkdir(parents=True)
        code = '''import json, os, sys, time
from pathlib import Path
sys.path.insert(0, 'scripts')
import workflow as w
w.runtime.user_home = lambda: Path(os.environ['FIXTURE_USER_HOME'])
with w.runtime.FileLock(w.heavy_path()) as lock:
    while not lock.acquire():
        time.sleep(.05)
    started = time.time()
    time.sleep(.6)
    Path(sys.argv[1]).write_text(json.dumps([started, time.time()]), encoding='utf-8')
'''
        children, tokens = [], []
        try:
            for index in range(2):
                temporary = self.repo.parent / f"tmp-{index}"
                temporary.mkdir()
                env = os.environ.copy()
                env["TMPDIR"] = str(temporary)
                env["FIXTURE_USER_HOME"] = str(self.repo.parent / "fixture-user")
                cmd = [sys.executable, "-B", "-c", code, str(output / str(index))]
                child = subprocess.Popen(cmd, cwd=self.repo, env=env, stdout=subprocess.DEVNULL,
                                         stderr=subprocess.DEVNULL, start_new_session=True)
                children.append(child)
                tokens.append(self.w.identity(child.pid))
            for child in children:
                self.assertEqual(child.wait(timeout=15), 0)
            intervals = sorted(json.loads((output / str(index)).read_text(encoding="utf-8")) for index in range(2))
            self.assertLessEqual(intervals[0][1], intervals[1][0])
        finally:
            for child, token in zip(children, tokens):
                self.w.terminate_session(child.pid, token)
                child.wait(timeout=5)

    def test_relative_heavy_lock_is_rejected(self):
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["heavy_lock"] = "evidence/local.lock"
        path.write_text(json.dumps(cfg), encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "绝对路径"):
            self.w.config()

    def test_command_finishes_after_caller_session_is_stopped(self):
        code = "import time; time.sleep(.5); print('finished independently', flush=True)"
        child = subprocess.Popen([sys.executable, "-B", str(self.repo / "scripts/workflow.py"), "run", "surviving",
                                  "--", sys.executable, "-c", code], cwd=self.repo, start_new_session=True,
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        token = self.w.identity(child.pid)
        try:
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                runs = self.w.receipts()
                if runs and runs[0]["status"] == "running":
                    break
                time.sleep(.02)
            else:
                self.fail("command did not start")
            self.w.terminate_session(child.pid, token)
            child.wait(timeout=5)
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                run = self.w.receipts()[0]
                if run["status"] == "finished":
                    break
                time.sleep(.02)
            else:
                self.fail("independent command did not finish")
            self.assertEqual(run["exit"], 0)
            self.w.verify_evidence(run)
        finally:
            self.w.terminate_session(child.pid, token)
            child.wait(timeout=5)

    def test_model_done_claim_and_restart_do_not_reset_budget(self):
        for expected in (1, 2):
            result = self.command("drive", "--once")
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertEqual(self.w.state("T0.1")["sessions"], expected)
            self.assertEqual(self.w.next_card()["id"], "T0.1")
        result = self.command("drive", "--once")
        self.assertEqual(result.returncode, 13, result.stderr)
        self.assertEqual(self.w.state("T0.1")["sessions"], 2)

    def test_driver_requires_configured_agent_command(self):
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["agent_command"] = None
        path.write_text(json.dumps(cfg), encoding="utf-8", newline="\n")
        (self.repo / "docs/TASKS.md").write_text(card_text(), encoding="utf-8", newline="\n")
        self.commit("plan without agent")
        result = self.command("drive", "--once")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("agent_command", result.stderr)
        self.assertFalse((self.repo / "evidence/agent-argv.json").exists())

    def enable_review(self, mode):
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["review_command"] = [sys.executable, str(self.repo.parent / "tools/reviewer.py")]
        path.write_text(json.dumps(cfg), encoding="utf-8", newline="\n")
        (self.repo / "docs/TASKS.md").write_text(card_text() + "\n" + card_text("T0.2", "T0.1"), encoding="utf-8", newline="\n")
        self.commit("plan two reviewed cards")
        os.environ.update(FIXTURE_AGENT_MODE="complete", FIXTURE_REVIEW_MODE=mode)

    def review_calls(self):
        path = self.repo / "evidence/review-calls.json"
        return json.loads(path.read_text(encoding="utf-8")) if path.exists() else []

    def agent_cards(self):
        path = self.repo / "evidence/agent-argv.json"
        return [call["card"] for call in json.loads(path.read_text(encoding="utf-8"))] if path.exists() else []

    def test_review_only_does_not_resolve_developer_or_start_next_card(self):
        self.enable_review("clear")
        path = self.repo / "workflow.json"
        cfg = json.loads(path.read_text(encoding="utf-8"))
        cfg["agent_command"] = [str(self.repo.parent / "missing-developer")]
        path.write_text(json.dumps(cfg), encoding="utf-8", newline="\n")
        self.commit("configure independent review without developer")
        self.finish()
        self.commit("T0.1: completed before independent review")
        before = self.w.state("T0.1")
        head = self.git("rev-parse", "HEAD")
        result = self.command("drive", "--review-only")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.agent_cards(), [])
        self.assertEqual(self.w.state("T0.1"), before)
        self.assertEqual(self.w.state("T0.2"), {})
        self.assertEqual(len(self.review_calls()), 1)
        self.assertIn(f"审查卡 T0.1，固定提交 {head}", self.review_calls()[0])
        status = self.w.read_json(self.repo / "evidence/drive/state.json")
        self.assertEqual((status["status"], status["reason"]), ("stopped", "review_done"))
        with (self.repo / "docs/PROGRESS.md").open("a", encoding="utf-8", newline="\n") as stream:
            stream.write("\nIndependent review follow-up.\n")
        self.commit("document follow-up after clear review")
        result = self.command("drive", "--review-only")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(len(self.review_calls()), 2)
        self.assertEqual(self.w.latest_review()["commit"], self.git("rev-parse", "HEAD"))
        self.assertNotEqual(self.w.latest_review()["commit"], head)
        self.assertEqual(self.agent_cards(), [])
        self.assertEqual(self.w.state("T0.1"), before)
        self.assertEqual(self.w.state("T0.2"), {})

    def test_review_only_requires_configured_review_and_completion_commit(self):
        result = self.command("drive", "--review-only")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("review_command", result.stderr)
        self.enable_review("clear")
        result = self.command("drive", "--review-only")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("完成提交", result.stderr)
        self.finish()
        result = self.command("drive", "--review-only")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("完成提交", result.stderr)
        self.assertEqual(self.review_calls(), [])
        self.assertEqual(self.agent_cards(), [])
        self.assertEqual(self.w.state("T0.1")["sessions"], 0)

    def test_review_only_rejects_dirty_and_committed_unverified_source(self):
        self.enable_review("clear")
        self.finish()
        self.commit("T0.1: completed before review")
        (self.repo / "src/extra.txt").write_text("unverified fix", encoding="utf-8", newline="\n")
        for commit in (False, True):
            if commit:
                self.commit("unverified source fix")
            result = self.command("drive", "--review-only")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("未验证", result.stderr)
        self.assertEqual(self.review_calls(), [])
        self.assertEqual(self.agent_cards(), [])

    def test_review_only_refuses_unfinished_runs_and_global_lock(self):
        self.enable_review("clear")
        self.finish()
        self.commit("T0.1: completed before review")
        with self.w.runtime.FileLock(self.w.heavy_path()) as lock:
            self.assertTrue(lock.acquire())
            result = self.command("drive", "--review-only")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("全局锁", result.stderr)
        self.w.write_json(self.repo / "evidence/runs/interrupted/receipt.json",
                          dict(id="interrupted", created=time.time(), status="running", log="evidence/runs/interrupted/output.log"))
        result = self.command("drive", "--review-only")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("未完成", result.stderr)
        self.assertEqual(self.review_calls(), [])
        self.assertEqual(self.agent_cards(), [])
        (self.repo / "evidence/runs/interrupted/receipt.json").unlink()

    def test_review_only_retries_failed_review_without_development(self):
        self.enable_review("silent")
        self.finish()
        self.commit("T0.1: completed before failed review")
        result = self.command("drive", "--review-only")
        self.assertEqual(result.returncode, 13, result.stdout + result.stderr)
        self.assertEqual(self.w.latest_review()["reason"], "missing_verdict")
        os.environ["FIXTURE_REVIEW_MODE"] = "clear"
        result = self.command("drive", "--review-only")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(len(self.review_calls()), 2)
        self.assertEqual(self.agent_cards(), [])
        self.assertEqual(self.w.state("T0.1")["sessions"], 0)
        self.assertEqual(self.w.state("T0.2"), {})

    def test_review_only_preserves_unchanged_blocking_review(self):
        self.enable_review("blocking")
        self.finish()
        self.commit("T0.1: completed before blocking review")
        for _ in range(2):
            result = self.command("drive", "--review-only")
            self.assertEqual(result.returncode, 13, result.stdout + result.stderr)
        self.assertEqual(len(self.review_calls()), 1)
        self.assertEqual(self.agent_cards(), [])
        self.assertNotIn("accepted", self.w.latest_review())

    def test_review_only_shares_driver_lock(self):
        self.enable_review("clear")
        with self.w.runtime.FileLock(self.repo / "evidence/drive/driver.lock") as lock:
            self.assertTrue(lock.acquire())
            result = self.command("drive", "--review-only")
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("已有驱动器", result.stderr)
        self.assertEqual(self.w.state("T0.1"), {})
        self.assertEqual(self.review_calls(), [])

    def test_review_only_preserves_live_developer_and_previous_driver_state(self):
        self.enable_review("clear")
        self.finish()
        self.commit("T0.1: completed before orphaned developer check")
        child = subprocess.Popen([sys.executable, "-B", "-c", "import time; time.sleep(20)"], cwd=self.repo)
        token = self.w.identity(child.pid)
        path = self.repo / "evidence/drive/state.json"
        self.w.write_json(path, dict(pid=999999999, identity="stale-driver", status="session_ended",
                                    agent_pid=child.pid, agent_identity=token))
        previous = path.read_bytes()
        try:
            result = self.command("drive", "--review-only")
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("旧开发会话仍存活", result.stderr)
            self.assertEqual(path.read_bytes(), previous)
            self.assertEqual(self.review_calls(), [])
            self.assertEqual(self.agent_cards(), [])
            self.assertEqual(self.w.identity(child.pid), token)
            self.assertIsNone(child.poll())
        finally:
            self.w.runtime.terminate([dict(pid=child.pid, identity=token)])
            child.wait(timeout=5)

    def test_review_only_invalidates_clear_after_reviewer_mutations(self):
        self.enable_review("clear")
        self.finish()
        self.commit("T0.1: completed before reviewer mutation")
        reviewer = self.repo.parent / "tools/reviewer.py"
        source = self.repo / "src/extra.txt"
        original = source.read_bytes()
        mutations = (
            "Path('src/extra.txt').write_text('reviewer changed source', encoding='utf-8')\n",
            "import subprocess\nsubprocess.run(['git', 'commit', '--allow-empty', '-m', 'reviewer changed HEAD'], check=True, capture_output=True)\n",
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                reviewer.write_text(REVIEWER + "\n" + mutation, encoding="utf-8", newline="\n")
                result = self.command("drive", "--review-only")
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                latest = self.w.latest_review()
                self.assertEqual(latest["verdict"], "FAILED")
                self.assertEqual(latest["reported_verdict"], "CLEAR")
                self.assertEqual(latest["reason"], "post_review_validation_failed")
                self.assertTrue(latest["validation_error"])
                report = self.repo / "evidence/reviews" / latest["id"] / "output.log"
                self.assertIn("REVIEW-VERDICT: CLEAR", report.read_text(encoding="utf-8"))
                self.assertEqual(self.agent_cards(), [])
                self.assertEqual(self.w.state("T0.2"), {})
                source.write_bytes(original)

    def test_new_review_agent_clears_each_completed_card(self):
        self.enable_review("clear")
        result = self.command("drive")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.review_calls()
        self.assertEqual(len(calls), 2)
        self.assertIn("审查卡 T0.1", calls[0])
        self.assertIn("审查卡 T0.2", calls[1])
        self.assertEqual(self.w.latest_review()["verdict"], "CLEAR")

    def test_blocking_review_pauses_until_user_accepts(self):
        self.enable_review("blocking")
        result = self.command("drive")
        self.assertEqual(result.returncode, 13, result.stdout + result.stderr)
        self.assertEqual(self.w.read_json(self.repo / "evidence/drive/state.json")["status"], "review_blocked")
        self.assertEqual(self.agent_cards(), ["T0.1"])
        blocked = self.w.latest_review()
        self.assertEqual(blocked["verdict"], "BLOCKING")
        restarted = self.command("drive")
        self.assertEqual(restarted.returncode, 13)
        self.assertEqual(len(self.review_calls()), 1, "unchanged blocking commit must not be re-reviewed")
        self.assertEqual(self.agent_cards(), ["T0.1"])
        self.assertEqual(self.command("drive", "accept-review", blocked["id"]).returncode, 0)
        os.environ["FIXTURE_REVIEW_MODE"] = "clear"
        resumed = self.command("drive")
        self.assertEqual(resumed.returncode, 0, resumed.stdout + resumed.stderr)
        self.assertEqual(self.agent_cards(), ["T0.1", "T0.2"])
        self.assertEqual(len(self.review_calls()), 2)

    def test_missing_verdict_pauses_and_is_reviewed_again(self):
        self.enable_review("silent")
        result = self.command("drive")
        self.assertEqual(result.returncode, 13, result.stdout + result.stderr)
        self.assertEqual(self.w.latest_review()["reason"], "missing_verdict")
        self.assertNotEqual(self.command("drive", "accept-review", self.w.latest_review()["id"]).returncode, 0)
        os.environ["FIXTURE_REVIEW_MODE"] = "clear"
        resumed = self.command("drive")
        self.assertEqual(resumed.returncode, 0, resumed.stdout + resumed.stderr)
        calls = self.review_calls()
        self.assertEqual(len(calls), 3)
        self.assertIn("审查卡 T0.1", calls[1])
        self.assertEqual(self.agent_cards(), ["T0.1", "T0.2"])

    def test_card_completed_without_review_is_reviewed_on_restart(self):
        self.enable_review("clear")
        self.assertEqual(self.command("drive", "--once").returncode, 0)
        # Simulate the driver dying after the completion commit but before its review started.
        shutil.rmtree(self.repo / "evidence/reviews")
        (self.repo / "evidence/review-calls.json").unlink()
        result = self.command("drive")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = self.review_calls()
        self.assertEqual(len(calls), 2)
        self.assertIn("审查卡 T0.1", calls[0])
        self.assertIn("审查卡 T0.2", calls[1])

    def test_driver_advances_with_separate_agent_sessions(self):
        (self.repo / "docs/TASKS.md").write_text(card_text() + "\n" + card_text("T0.2", "T0.1"), encoding="utf-8", newline="\n")
        self.commit("plan two cards")
        with patch.dict(os.environ, FIXTURE_AGENT_MODE="complete"):
            result = self.command("drive")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        calls = json.loads((self.repo / "evidence/agent-argv.json").read_text(encoding="utf-8"))
        self.assertEqual([call["card"] for call in calls], ["T0.1", "T0.2"])
        for call in calls:
            self.assertEqual(call["argv"][0], "--prompt-file")
            prompt_file = Path(call["argv"][1])
            self.assertEqual(prompt_file.read_text(encoding="utf-8"), call["prompt"])
            self.assertIn(call["card"], prompt_file.parent.parent.name)
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_driver_develops_independent_card_around_blocked_branch(self):
        (self.repo / "docs/TASKS.md").write_text(card_text() + card_text("T0.2", "T0.1") + card_text("T0.3"), encoding="utf-8", newline="\n")
        self.append_progress("\n## T0.1 blocked\n- 结果：BLOCKED\n需要用户决定\n")
        self.commit("plan with blocked branch")
        with patch.dict(os.environ, FIXTURE_AGENT_MODE="complete"):
            result = self.command("drive")
        self.assertEqual(result.returncode, 11, result.stdout + result.stderr)
        calls = json.loads((self.repo / "evidence/agent-argv.json").read_text(encoding="utf-8"))
        self.assertEqual([call["card"] for call in calls], ["T0.3"])
        self.assertIn("T0.1（BLOCKED）", self.w.read_json(self.repo / "evidence/drive/state.json")["reason"])

    def test_driver_stops_on_unverified_changes_after_completion_commit(self):
        (self.repo / "docs/TASKS.md").write_text(card_text() + "\n" + card_text("T0.2", "T0.1"), encoding="utf-8", newline="\n")
        self.commit("plan two cards before dirty completion")
        path = self.repo.parent / "tools/agent.py"
        marker = "print('fixture claims DONE for ' + cid)"
        path.write_text(path.read_text(encoding="utf-8").replace(marker, "if mode == 'complete':\n    (root / 'src/extra.txt').write_text('unverified after commit')\n" + marker), encoding="utf-8", newline="\n")
        with patch.dict(os.environ, FIXTURE_AGENT_MODE="complete"):
            result = self.command("drive")
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("未验证", result.stderr)
        driver = self.w.read_json(self.repo / "evidence/drive/state.json")
        self.assertNotEqual(driver["status"], "all_done")
        self.assertTrue(self.git("status", "--porcelain"))
        self.assertEqual(len(json.loads((self.repo / "evidence/agent-argv.json").read_text(encoding="utf-8"))), 1)
        restarted = self.command("drive")
        self.assertNotEqual(restarted.returncode, 0)
        self.assertNotEqual(self.w.read_json(self.repo / "evidence/drive/state.json")["status"], "all_done")
        self.assertEqual(len(json.loads((self.repo / "evidence/agent-argv.json").read_text(encoding="utf-8"))), 1)

    def test_next_card_rejects_unverified_changes_after_previous_commit(self):
        (self.repo / "docs/TASKS.md").write_text(card_text() + "\n" + card_text("T0.2", "T0.1"), encoding="utf-8", newline="\n")
        self.commit("plan two cards")
        self.finish()
        self.commit("T0.1: complete")
        (self.repo / "src/extra.txt").write_text("unverified between cards", encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(self.w.WorkflowError, "未验证"):
            self.w.next_card()

    def test_all_done_rejects_clean_commit_with_unverified_source(self):
        self.finish()
        self.commit("T0.1: complete")
        (self.repo / "src/extra.txt").write_text("unverified subsequent commit", encoding="utf-8", newline="\n")
        self.commit("unverified change")
        with self.assertRaisesRegex(self.w.WorkflowError, "未验证"):
            self.w.next_card()

    def test_driver_restart_of_completed_clean_project_reports_success(self):
        self.finish()
        self.commit("T0.1: complete")
        result = self.command("drive")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.w.read_json(self.repo / "evidence/drive/state.json")["status"], "all_done")
        self.assertFalse((self.repo / "evidence/agent-argv.json").exists())

    def test_driver_timeout_cleans_agent(self):
        with patch.dict(os.environ, FIXTURE_AGENT_MODE="sleep"):
            result = self.command("drive", "--once", "--session-hours", "0.0003")
        self.assertEqual(result.returncode, 1, result.stderr)
        driver = self.w.read_json(self.repo / "evidence/drive/state.json")
        self.assertEqual(driver["reason"], "once_wip")
        session = self.w.read_json(self.repo / "evidence/task-state/T0.1/session-1/result.json")
        self.assertEqual(session["reason"], "session_timeout")
        self.assertNotEqual(self.w.identity(session["agent_pid"]), session["agent_identity"])

    def test_stale_process_identity_is_not_signalled(self):
        self.assertFalse(self.w.signal_exact(os.getpid(), "not-the-current-process", 15))

    def test_git_snapshot_matches_commit_with_CRLF_and_unicode_paths(self):
        self.git("config", "core.autocrlf", "true")
        path = self.repo / "src/中文 文件.txt"
        path.write_bytes("第一行\r\n第二行\r\n".encode("utf-8"))
        before = self.w.snapshot()
        self.commit("CRLF fixture")
        self.assertEqual(before, self.w.snapshot(self.git("rev-parse", "HEAD")))

    def test_git_snapshot_honors_disabled_filemode(self):
        self.git("config", "core.filemode", "false")
        self.git("update-index", "--chmod=+x", "scripts/workflow.py")
        self.git("commit", "-m", "filemode fixture")
        self.assertEqual(self.w.snapshot(), self.w.snapshot(self.git("rev-parse", "HEAD")))

    def test_frozen_markdown_can_change_line_endings_and_checkbox(self):
        self.w.start("T0.1")
        path = self.repo / "docs/TASKS.md"
        path.write_bytes(path.read_bytes().replace(b"[ ]", b"[x]").replace(b"\n", b"\r\n"))
        self.w.check_frozen("T0.1")

    def test_completion_commit_accepts_CRLF_appended_progress(self):
        result = self.accept()
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(self.w.verify(["all"]), 0)
        self.record(result)
        path = self.repo / "docs/PROGRESS.md"
        data = path.read_bytes()
        size = self.w.state("T0.1")["append_only"]["docs/PROGRESS.md"]["size"]
        path.write_bytes(data[:size] + data[size:].replace(b"\n", b"\r\n"))
        with contextlib.redirect_stdout(io.StringIO()):
            self.w.done("T0.1")
        self.commit("T0.1: CRLF progress fixture")
        with self.assertRaises(self.w.WorkflowError) as caught:
            self.w.next_card()
        self.assertEqual(caught.exception.code, 10)

    def test_windows_paths_cannot_escape_project_scope(self):
        for value in ("C:/outside", "C:outside", "\\\\server\\share", "..\\outside", "nested\\..\\outside"):
            with self.subTest(value=value), self.assertRaises(self.w.WorkflowError):
                self.w.relative_path(value)

    def test_python_entrypoints_return_real_exit_codes(self):
        for name, args, expected in (("task", ["unknown"], 2), ("run", [], 2), ("verify", ["unknown"], 2), ("drive", ["status"], 0), ("check-project", [], 2)):
            with self.subTest(entry=name):
                result = subprocess.run([sys.executable, "-B", "scripts/" + name + ".py", *args], cwd=self.repo,
                                        env=os.environ.copy(), capture_output=True, text=True, encoding="utf-8")
                self.assertEqual(result.returncode, expected, result.stdout + result.stderr)

    def test_payload_preserves_shell_metacharacters_and_unicode(self):
        argument = '中文 & echo injected %PATH% $(touch injected) "quoted" !value!'
        code = "import sys; from pathlib import Path; assert sys.argv[1] == " + repr(argument) + "; assert not Path('injected').exists()"
        check = dict(id="A1", description="argv无shell求值", argv=["{python}", "-c", code, argument], kind="command")
        (self.repo / "docs/TASKS.md").write_text(card_text(check=check), encoding="utf-8", newline="\n")
        self.w.start("T0.1")
        with contextlib.redirect_stdout(io.StringIO()):
            result = self.w.run_command("A1", check["argv"])
        self.assertEqual(result["exit"], 0, result)
        self.assertFalse((self.repo / "injected").exists())

    def sleeping_driver(self):
        env = os.environ.copy()
        env["FIXTURE_AGENT_MODE"] = "sleep"
        child = self.w.runtime.launch([sys.executable, "-B", "scripts/drive.py"], cwd=self.repo, env=env,
                                      detached=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        token = self.w.identity(child.pid)
        # Cleanups run last-in first-out: stop, terminate, then reap so no Popen is left unwaited.
        self.addCleanup(lambda: child.wait(timeout=10))
        self.addCleanup(lambda: self.w.runtime.terminate([dict(pid=child.pid, identity=token)]))
        self.addCleanup(lambda: self.command("drive", "stop"))
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            driver = self.w.read_json(self.repo / "evidence/drive/state.json")
            if driver.get("status") == "running":
                return child, driver
            if child.poll() is not None:
                self.fail("sleeping driver exited before ready")
            time.sleep(.05)
        self.fail("sleeping driver was not ready")

    def test_driver_stop_request_cleans_its_agent_on_every_platform(self):
        child, driver = self.sleeping_driver()
        result = self.command("drive", "stop")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(child.wait(timeout=15), 143)
        self.assertNotEqual(self.w.identity(driver["agent_pid"]), driver["agent_identity"])
        final = self.w.read_json(self.repo / "evidence/drive/state.json")
        self.assertEqual(final["reason"], "user_stop")
        self.assertEqual(self.w.state("T0.1")["sessions"], 1)

    def test_driver_lock_prevents_a_second_session(self):
        child, _ = self.sleeping_driver()
        result = self.command("drive", "--once")
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("已有驱动器", result.stderr)
        self.assertEqual(self.w.state("T0.1")["sessions"], 1)
        self.assertIsNone(child.poll())

    def test_recovery_does_not_ignore_a_live_payload_after_executor_exit(self):
        path = self.repo / "evidence/runs/orphan/receipt.json"
        self.w.write_json(path, dict(id="orphan", card=None, created=time.time()-10, status="running",
                                    log="evidence/runs/orphan/output.log", payload_refs=[dict(pid=os.getpid(), identity=self.w.identity(os.getpid()))]))
        self.assertTrue(self.w.run_alive(self.w.read_json(path)))
        with self.assertRaisesRegex(self.w.WorkflowError, "仍存活"):
            self.w.recover_runs()

    def test_dead_launcher_without_identity_is_not_marked_alive(self):
        folder = self.repo / "evidence/runs/missing-identity"
        self.w.write_json(folder / "launcher.json", dict(pid=999999999, identity=None))
        run = dict(log="evidence/runs/missing-identity/output.log", payload_refs=[dict(pid=999999999, identity=None)])
        self.assertFalse(self.w.run_alive(run))

    @unittest.skipIf(os.name == "nt", "POSIX session survives its leader")
    def test_recovery_checks_unobserved_payload_session_after_leader_exit(self):
        marker = self.repo / "evidence/unobserved-child.json"
        marker.parent.mkdir(parents=True, exist_ok=True)
        code = ("import subprocess,sys,json; from pathlib import Path; "
                "child=subprocess.Popen([sys.executable,'-B','-c','import time; time.sleep(20)'],process_group=0); "
                f"Path({str(marker)!r}).write_text(json.dumps(child.pid))")
        process = self.w.runtime.launch([sys.executable, "-B", "-c", code], cwd=self.repo,
                                        detached=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        token = "created:" + repr(self.w.runtime.psutil.Process(process.pid).create_time())
        folder = self.repo / "evidence/runs/unobserved-child"
        folder.mkdir(parents=True)
        (folder / "output.log").write_text("partial output\n")
        run = dict(id="unobserved-child", card=None, created=time.time()-10,
                   status="cleanup_failed", log="evidence/runs/unobserved-child/output.log",
                   payload_pid=process.pid, payload_identity=token,
                   payload_refs=[dict(pid=process.pid, identity=token)])
        self.w.write_json(folder / "receipt.json", run)
        try:
            self.assertEqual(process.wait(timeout=5), 0)
            self.assertTrue(marker.exists())
            self.assertTrue(self.w.run_alive(run), "unknown child in the original session must block recovery")
            with self.assertRaisesRegex(self.w.WorkflowError, "仍存活"):
                self.w.recover_runs()
            self.assertNotIn("log_hash", self.w.read_json(folder / "receipt.json"))
            self.w.runtime.terminate_session(process.pid, token)
            with contextlib.redirect_stdout(io.StringIO()):
                self.w.recover_runs()
            recovered = self.w.read_json(folder / "receipt.json")
            self.assertEqual(recovered["status"], "finished")
            self.assertEqual(recovered["exit"], 125)
            self.assertEqual(recovered["log_hash"], self.w.digest((folder / "output.log").read_bytes()))
        finally:
            self.w.runtime.terminate_session(process.pid, token)
            process.wait(timeout=5)

    def test_executor_failure_keeps_live_payload_unfinished(self):
        self.w.start("T0.1")
        child = subprocess.Popen([sys.executable, "-B", "-c", "import time; time.sleep(20)"], cwd=self.repo)
        token = self.w.identity(child.pid)
        def interrupted_wait():
            path = self.repo / self.w.receipts()[-1]["path"]
            record = self.w.read_json(path)
            record.update(status="running", payload_refs=[dict(pid=child.pid, identity=token)])
            self.w.write_json(path, record)
            return 9
        try:
            from types import SimpleNamespace
            launcher = SimpleNamespace(pid=999999999, wait=interrupted_wait)
            with patch.object(self.w.runtime, "launch", return_value=launcher), contextlib.redirect_stdout(io.StringIO()):
                result = self.w.run_command("orphaned", ["{python}", "-c", "assert True"])
            self.assertEqual(result["exit"], 9)
            self.assertEqual(result["status"], "cleanup_failed")
            self.assertTrue(self.w.run_alive(result))
            with self.assertRaisesRegex(self.w.WorkflowError, "仍存活"):
                self.w.recover_runs()
        finally:
            self.w.runtime.terminate([dict(pid=child.pid, identity=token)])
            child.wait(timeout=5)

    @unittest.skipIf(os.name == "nt", "POSIX session enumeration failure")
    def test_enumeration_failure_blocks_reentry_and_seals_only_after_recovery(self):
        self.w.start("T0.1")
        marker = self.repo / "evidence/fault-payload.json"
        payload = ("import os,json,time,psutil; from pathlib import Path; "
                   f"Path({str(marker)!r}).write_text(json.dumps(dict(pid=os.getpid(),"
                   "identity='created:'+repr(psutil.Process().create_time())))); "
                   "print('payload ready',flush=True); time.sleep(.3); "
                   "print('payload must stop before sealing',flush=True); time.sleep(20)")
        bootstrap = '''import errno,sys,time
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,sys.argv[1])
import workflow as w
def denied(*_):
    deadline=time.monotonic()+3
    while not Path(sys.argv[3]).exists() and time.monotonic()<deadline:
        time.sleep(.01)
    raise PermissionError(errno.EPERM,"injected session enumeration denied")
with patch.object(w.runtime,"session_family",side_effect=denied):
    raise SystemExit(w.execute_receipt(Path(sys.argv[2])))
'''
        original_launch = self.w.runtime.launch

        def launch(argv, **kwargs):
            self.assertIn("_execute", argv)
            return original_launch([sys.executable, "-B", "-c", bootstrap,
                                    str(self.repo / "scripts"), str(argv[-1]), str(marker)], **kwargs)

        try:
            with patch.object(self.w.runtime, "launch", side_effect=launch), \
                    contextlib.redirect_stdout(io.StringIO()):
                run = self.w.run_command("enumeration-failure", ["{python}", "-B", "-c", payload])
            self.assertEqual(run["status"], "cleanup_failed")
            self.assertEqual(run["exit"], 125)
            self.assertIn("payload_identity", run)
            self.assertTrue(run["payload_refs"])
            self.assertNotIn("log_hash", run, "unfinished output must remain unsealed")
            self.assertIsNone(self.w.identity(run["payload_pid"]))
            count = len(self.w.receipts())
            with self.assertRaisesRegex(self.w.WorkflowError, "清理"):
                self.w.run_command("must-not-start", ["{python}", "-B", "-c", "assert False"])
            self.assertEqual(len(self.w.receipts()), count)
            path = self.repo / Path(run["log"]).parent / "receipt.json"
            run["created"] -= 10
            self.w.write_json(path, run)
            with patch.object(self.w.runtime, "session_family", side_effect=PermissionError("still denied")), \
                    self.assertRaisesRegex(self.w.WorkflowError, "无法确认"):
                self.w.recover_runs()
            self.assertEqual(self.w.read_json(path)["status"], "cleanup_failed")
            with contextlib.redirect_stdout(io.StringIO()):
                self.w.recover_runs()
            recovered = self.w.read_json(path)
            self.assertEqual(recovered["status"], "finished")
            self.assertEqual(recovered["exit"], 125)
            log = self.repo / run["log"]
            sealed = recovered["log_hash"]
            time.sleep(.4)
            self.assertEqual(self.w.digest(log.read_bytes()), sealed)
            self.assertNotIn("payload must stop before sealing", log.read_text())
        finally:
            if marker.exists():
                ref = json.loads(marker.read_text())
                self.w.runtime.terminate_session(ref["pid"], ref["identity"])

    def test_agent_cleanup_preserves_previously_observed_detached_payload(self):
        child = subprocess.Popen([sys.executable, "-B", "-c", "import time; time.sleep(20)"], cwd=self.repo)
        token = self.w.identity(child.pid)
        root_pid = 999999999
        root_token = "old-agent"
        try:
            self.w.write_json(self.repo / "evidence/runs/detached/receipt.json", dict(id="detached", card=None,
                status="running", created=time.time(), log="evidence/runs/detached/output.log", payload_refs=[dict(pid=child.pid, identity=token)]))
            self.w.write_json(self.repo / "evidence/processes" / f"{root_pid}-{self.w.digest(root_token)[:16]}.json", [dict(pid=child.pid, identity=token)])
            self.w.terminate_session(root_pid, root_token)
            self.assertEqual(self.w.identity(child.pid), token)
        finally:
            self.w.runtime.terminate([dict(pid=child.pid, identity=token)])
            child.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
