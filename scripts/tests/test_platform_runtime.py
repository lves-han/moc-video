"""Real local processes and OS locks; no online agent calls."""
import contextlib
import errno
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import types
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import platform_runtime as runtime


class PlatformTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="moc-runtime-中文-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)

    def payload(self, code, seconds=5, memory=128):
        known = {}
        def observe(pid, token, refs):
            known.update({(item["pid"], item["identity"]): item for item in refs})
        with runtime.FileLock(self.base / "payload.lock") as lock:
            self.assertTrue(lock.acquire())
            result = runtime.run_payload(["{python}", "-B", "-c", code], cwd=self.base, env=os.environ.copy(),
                                         seconds=seconds, memory_mib=memory, sampled=True, lock_fd=lock.fileno(), observe=observe)
        self.assertTrue(all(runtime.identity(pid) != token for pid, token in known))
        return result

    def test_watchdog_preserves_nonzero_payload_exit(self):
        result = self.payload("raise SystemExit(7)")
        self.assertEqual(result["raw_exit"], 7)
        self.assertEqual(result["exit"], 7)

    @unittest.skipIf(os.name == "nt", "POSIX session enumeration failure")
    def test_initial_and_cleanup_enumeration_failure_registers_and_reaps_payload(self):
        marker = self.base / "enumeration-failure-ready.json"
        code = ("import subprocess,sys,json,time,psutil; from pathlib import Path; "
                "child=subprocess.Popen([sys.executable,'-B','-c','import time; time.sleep(20)']); "
                f"Path({str(marker)!r}).write_text(json.dumps(dict(pid=child.pid,"
                "identity='created:'+repr(psutil.Process(child.pid).create_time())))); time.sleep(20)")
        observed, launched = [], []
        original_launch = runtime.launch

        def launch(*args, **kwargs):
            kwargs.update(stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            process = original_launch(*args, **kwargs)
            launched.append((process, runtime.identity(process.pid)))
            return process

        def denied(*_args):
            deadline = time.monotonic() + 3
            while not marker.exists() and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertTrue(marker.exists(), "real child must exist before the injected error")
            raise PermissionError(errno.EPERM, "injected session enumeration denied")

        try:
            with runtime.FileLock(self.base / "enumeration-failure.lock") as lock:
                self.assertTrue(lock.acquire())
                with patch.object(runtime, "launch", side_effect=launch), \
                        patch.object(runtime, "session_family", side_effect=denied), \
                        self.assertRaisesRegex(runtime.WorkflowError, "清理"):
                    runtime.run_payload(["{python}", "-B", "-c", code], cwd=self.base,
                                        env=os.environ.copy(), seconds=5, memory_mib=128,
                                        sampled=True, lock_fd=lock.fileno(),
                                        observe=lambda pid, token, refs: observed.append((pid, token, refs)))
            process, token = launched[0]
            self.assertTrue(observed, "the first enumeration error must not hide the payload")
            self.assertEqual(observed[0][0:2], (process.pid, token))
            self.assertIn(dict(pid=process.pid, identity=token), observed[0][2])
            self.assertIsNotNone(process.returncode, "owned direct child must be reaped")
            self.assertFalse(runtime.alive(json.loads(marker.read_text())))
        finally:
            for process, token in launched:
                runtime.terminate_session(process.pid, token)
                process.wait(timeout=5)

    def test_watchdog_timeout_cleans_parent_and_child(self):
        code = "import subprocess,sys,time; subprocess.Popen([sys.executable,'-c','import time; time.sleep(20)']); time.sleep(20)"
        result = self.payload(code, seconds=.5)
        self.assertEqual(result["exit"], 124)
        self.assertNotEqual(result["raw_exit"], 0)

    def test_watchdog_stops_an_actual_memory_overrun(self):
        code = "import time; data=bytearray(80*1024*1024); data[::4096]=b'x'*len(data[::4096]); time.sleep(10)"
        result = self.payload(code, memory=32)
        self.assertEqual(result["exit"], 137)
        self.assertGreater(result["peak_rss_bytes"], 32*1048576)
        self.assertIn("RSS", result["note"])

    def test_watchdog_cleans_children_after_parent_success(self):
        code = "import subprocess,sys,time; subprocess.Popen([sys.executable,'-c','import time; time.sleep(20)']); time.sleep(.5)"
        result = self.payload(code)
        self.assertEqual(result["exit"], 0)

    def test_watchdog_cleans_unsampled_child_after_immediate_parent_exit(self):
        # No delay in the spawning root. A tree walk based on the already exited
        # root cannot discover this child; the OS job/session boundary must do so.
        for attempt in range(5):
            marker = self.base / f"quick-child-{attempt}.json"
            code = ("import subprocess,sys,json; from pathlib import Path; "
                    "p=subprocess.Popen([sys.executable,'-B','-c','import time; time.sleep(30)']); "
                    f"Path({str(marker)!r}).write_text(json.dumps(p.pid))")
            ref = None
            try:
                result = self.payload(code)
                self.assertEqual(result["exit"], 0)
                pid = json.loads(marker.read_text())
                ref = dict(pid=pid, identity=runtime.identity(pid))
                self.assertIsNone(ref["identity"], f"quick root left a live child: {pid}")
            finally:
                if ref:
                    runtime.terminate([ref])

    @unittest.skipUnless(os.name == "nt", "Windows Job lifetime boundary")
    def test_windows_guardian_kill_removes_payload_family_before_next_heavy_command(self):
        marker = self.base / "guardian-ready.json"
        child_marker = self.base / "payload-child.json"
        lock_path = self.base / "guardian-heavy.lock"
        payload = ("import subprocess,sys,json,time; from pathlib import Path; "
                   "p=subprocess.Popen([sys.executable,'-B','-c','import time; time.sleep(30)']); "
                   f"Path({str(child_marker)!r}).write_text(json.dumps(p.pid)); time.sleep(30)")
        code = ("import sys,os,json; from pathlib import Path; "
                "sys.path.insert(0,sys.argv[1]); import platform_runtime as r; "
                "lock=r.FileLock(Path(sys.argv[2])); assert lock.acquire(); "
                "r.run_payload([sys.executable,'-B','-c',sys.argv[4]],cwd=Path(sys.argv[2]).parent,"
                "env=os.environ.copy(),seconds=25,memory_mib=128,sampled=True,lock_fd=lock.fileno(),"
                "observe=lambda pid,token,refs: Path(sys.argv[3]).write_text(json.dumps(dict(pid=pid,identity=token))))")
        guardian = runtime.launch([sys.executable, "-B", "-c", code, str(SCRIPTS), str(lock_path),
                                   str(marker), payload], detached=True, stdout=subprocess.DEVNULL,
                                  stderr=subprocess.DEVNULL)
        guardian_ref = dict(pid=guardian.pid, identity=runtime.identity(guardian.pid))
        refs = []
        try:
            deadline = time.monotonic() + 5
            while (not marker.exists() or not child_marker.exists()) and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertTrue(marker.exists())
            self.assertTrue(child_marker.exists())
            refs.append(json.loads(marker.read_text()))
            pid = json.loads(child_marker.read_text())
            refs.append(dict(pid=pid, identity=runtime.identity(pid)))
            self.assertTrue(all(runtime.alive(ref) for ref in refs))
            with runtime.FileLock(lock_path) as competing:
                self.assertFalse(competing.acquire())
            guardian.kill()  # No cooperative finally/cleanup gets to run.
            guardian.wait(timeout=5)
            deadline = time.monotonic() + 5
            while any(runtime.alive(ref) for ref in refs) and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertTrue(all(not runtime.alive(ref) for ref in refs))
            with runtime.FileLock(lock_path) as competing:
                self.assertTrue(competing.acquire())
        finally:
            runtime.terminate([guardian_ref, *refs])
            guardian.wait(timeout=5)

    @unittest.skipUnless(os.name == "nt", "Win32 atomic Job creation before payload execution")
    def test_windows_guardian_death_before_resume_does_not_leak_suspended_payload(self):
        marker = self.base / "suspended-ready.json"
        payload_marker = self.base / "suspended-must-not-run"
        helper = self.base / "suspended-guardian.py"
        helper.write_text("""import ctypes,json,os,sys,time
from pathlib import Path
sys.path.insert(0,sys.argv[1])
import platform_runtime as r
import windows_job as j
kernel = j._api()
create = kernel.CreateProcessW
def remember(*args):
    result = create(*args)
    if result:
        process = ctypes.cast(args[-1],ctypes.POINTER(j.ProcessInfo)).contents
        Path(sys.argv[3]).write_text(json.dumps(dict(pid=process.dwProcessId,identity=r.identity(process.dwProcessId))))
    return result
kernel.CreateProcessW = remember
kernel.ResumeThread = lambda *_: time.sleep(30)
j._api = lambda: kernel
with r.FileLock(Path(sys.argv[2])) as lock:
    assert lock.acquire()
    r.run_payload([sys.executable,'-B','-c',"from pathlib import Path; Path("+repr(sys.argv[4])+ ").touch()"],
        cwd=Path(sys.argv[2]).parent,env=os.environ.copy(),seconds=25,memory_mib=128,
        sampled=True,lock_fd=lock.fileno(),observe=lambda *_:None)
""", encoding="utf-8")
        guardian = runtime.launch([sys.executable, "-B", str(helper), str(SCRIPTS),
                                   str(self.base / "suspended.lock"), str(marker), str(payload_marker)],
                                  detached=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        guardian_ref = dict(pid=guardian.pid, identity=runtime.identity(guardian.pid))
        refs = []
        try:
            deadline = time.monotonic() + 5
            while not marker.exists() and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertTrue(marker.exists())
            refs.append(json.loads(marker.read_text()))
            self.assertTrue(runtime.alive(refs[0]))
            self.assertFalse(payload_marker.exists())
            guardian.kill()
            guardian.wait(timeout=5)
            deadline = time.monotonic() + 5
            while runtime.alive(refs[0]) and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertFalse(runtime.alive(refs[0]))
            self.assertFalse(payload_marker.exists())
        finally:
            runtime.terminate([guardian_ref, *refs])
            guardian.wait(timeout=5)

    @unittest.skipUnless(os.name == "nt", "Win32 atomic Job creation")
    def test_windows_job_assignment_failure_does_not_run_payload(self):
        import ctypes
        import windows_job
        kernel = windows_job._api()
        update = kernel.UpdateProcThreadAttribute
        marker = self.base / "must-not-run"
        def unsupported(attributes, flags, key, *args):
            if key == 0x2000D:
                ctypes.set_last_error(87)
                return False
            return update(attributes, flags, key, *args)
        with patch.object(windows_job, "_api", return_value=kernel), \
                patch.object(kernel, "UpdateProcThreadAttribute", side_effect=unsupported), \
                patch.object(kernel, "CreateProcessW", wraps=kernel.CreateProcessW) as create:
            with self.assertRaisesRegex(runtime.WorkflowError, "Job.*命令未运行"):
                self.payload(f"from pathlib import Path; Path({str(marker)!r}).touch()")
            create.assert_not_called()
        self.assertFalse(marker.exists())

    @unittest.skipUnless(os.name == "nt", "Win32 stdio inheritance and argv quoting")
    def test_windows_job_preserves_redirected_unicode_output_and_literal_argv(self):
        output = self.base / "stdio.log"
        expected = ["中文 & %PATH%", "quote\" trailing\\", "", "x$(echo unsafe)"]
        code = ("import sys,os; from pathlib import Path; sys.path.insert(0,sys.argv[1]); "
                "import platform_runtime as r; lock=r.FileLock(Path(sys.argv[2])); assert lock.acquire(); "
                "result=r.run_payload([sys.executable,'-B','-c',"
                "\"import json,sys,os; print(json.dumps([sys.argv[1:],os.environ['MOC_ARGV_VALUE']],ensure_ascii=False)); "
                "print('错误输出',file=sys.stderr)\",*sys.argv[3:]],cwd=Path(sys.argv[2]).parent,"
                "env=os.environ.copy(),seconds=5,memory_mib=128,sampled=True,lock_fd=lock.fileno(),"
                "observe=lambda *_: None); raise SystemExit(result['exit'])")
        env = dict(os.environ, PYTHONIOENCODING="utf-8", MOC_ARGV_VALUE="环境 中文 & %PATH%")
        with output.open("wb") as stream:
            guardian = runtime.launch([sys.executable, "-B", "-c", code, str(SCRIPTS),
                                       str(self.base / "stdio.lock"), *expected], cwd=self.base, env=env,
                                      stdout=stream, stderr=stream)
            try:
                self.assertEqual(guardian.wait(timeout=10), 0)
            finally:
                if guardian.poll() is None:
                    guardian.kill()
                    guardian.wait(timeout=5)
        lines = output.read_text(encoding="utf-8").splitlines()
        values = next(json.loads(line) for line in lines if line.startswith("["))
        self.assertEqual(values, [expected, env["MOC_ARGV_VALUE"]])
        self.assertIn("错误输出", lines)

    def test_file_lock_exclusion_and_release(self):
        with runtime.FileLock(self.base / "shared.lock") as first, runtime.FileLock(self.base / "shared.lock") as second:
            self.assertTrue(first.acquire())
            self.assertFalse(second.acquire())
        with runtime.FileLock(self.base / "shared.lock") as third:
            self.assertTrue(third.acquire())

    @unittest.skipUnless(os.name == "nt", "Windows byte range locks can extend past EOF")
    def test_windows_filelock_contends_with_empty_file_without_sentinel_write(self):
        path = self.base / "empty.lock"
        marker = self.base / "empty-lock-ready"
        code = ("import msvcrt,sys,time; from pathlib import Path; "
                "stream=open(sys.argv[1],'a+b'); stream.seek(0); "
                "msvcrt.locking(stream.fileno(),msvcrt.LK_NBLCK,1); "
                "Path(sys.argv[2]).touch(); time.sleep(20)")
        owner = runtime.launch([sys.executable, "-B", "-c", code, str(path), str(marker)], detached=True,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        ref = dict(pid=owner.pid, identity=runtime.identity(owner.pid))
        try:
            deadline = time.monotonic() + 5
            while not marker.exists() and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertTrue(marker.exists())
            self.assertEqual(path.stat().st_size, 0)
            with runtime.FileLock(path) as competing:
                self.assertFalse(competing.acquire())
            self.assertEqual(path.stat().st_size, 0)
        finally:
            runtime.terminate([ref])
            owner.wait(timeout=5)
        with runtime.FileLock(path) as competing:
            self.assertTrue(competing.acquire())
        self.assertEqual(path.stat().st_size, 0)

    def test_lock_released_when_process_dies(self):
        code = "import sys,time; from pathlib import Path; sys.path.insert(0,sys.argv[1]); import platform_runtime as r; lock=r.FileLock(Path(sys.argv[2])); assert lock.acquire(); Path(sys.argv[3]).touch(); time.sleep(20)"
        path = self.base / "death.lock"
        ready = self.base / "ready"
        child = runtime.launch([sys.executable, "-B", "-c", code, str(SCRIPTS), str(path), str(ready)], detached=True)
        token = runtime.identity(child.pid)
        try:
            deadline = time.monotonic()+5
            while not ready.exists() and time.monotonic() < deadline:
                time.sleep(.05)
            self.assertTrue(ready.exists())
            runtime.terminate([dict(pid=child.pid, identity=token)])
            child.wait(timeout=5)
            with runtime.FileLock(path) as lock:
                self.assertTrue(lock.acquire())
        finally:
            runtime.terminate([dict(pid=child.pid, identity=token)])
            child.wait(timeout=5)

    def test_missing_process_dependency_fails_explicitly(self):
        with patch.object(runtime, "psutil", None):
            with self.assertRaisesRegex(runtime.WorkflowError, "psutil"):
                runtime.identity(os.getpid())

    def test_stale_identity_never_stops_a_reused_pid(self):
        self.assertFalse(runtime.signal_exact(os.getpid(), "not-my-identity", signal.SIGTERM))

    def test_reused_session_leader_does_not_authorize_group_cleanup(self):
        with patch.object(runtime.psutil, "Process", return_value=types.SimpleNamespace(create_time=lambda: 20.0)), \
                patch.object(runtime.psutil, "process_iter") as listing:
            with self.assertRaisesRegex(runtime.WorkflowError, "PID 已复用"):
                runtime.session_family(1234, "created:10.0")
            listing.assert_not_called()

    def test_python_placeholder_uses_current_interpreter(self):
        args = runtime.resolve_argv(["{python}", "-B", "-c", "pass"])
        self.assertEqual(args[0], sys.executable)

    def test_windows_npm_shim_uses_node_and_preserves_argv(self):
        shim = self.base / "npm with spaces/agent.cmd"
        entry = shim.parent / "node_modules/@scope/agent-cli/bin/cli.js"
        entry.parent.mkdir(parents=True)
        shim.write_text('@ECHO off\r\n"%_prog%"  "%dp0%\\node_modules\\@scope\\agent-cli\\bin\\cli.js" %*\r\n', encoding="utf-8")
        entry.write_text("// fixture", encoding="utf-8")
        node = shim.parent / "node.exe"
        node.touch()
        arguments = ["exec", "-C", "C:/中文 & %PATH%", "-"]
        with patch.object(runtime, "os", types.SimpleNamespace(name="nt")):
            self.assertEqual(runtime.resolve_argv([str(shim), *arguments]), [str(node), str(entry), *arguments])

    def test_windows_batch_has_no_implicit_shell_execution(self):
        shim = self.base / "unsafe.cmd"
        shim.write_text("batch", encoding="utf-8")
        with patch.object(runtime, "os", types.SimpleNamespace(name="nt")):
            with self.assertRaisesRegex(runtime.WorkflowError, "batch"):
                runtime.resolve_argv([str(shim), "& echo injected"])

    def test_platform_backends_and_default_locks_ignore_TMPDIR(self):
        with patch.object(runtime, "user_home", return_value=self.base):
            for platform, expected in (("linux", "systemd"), ("darwin", "process-watchdog"), ("win32", "process-watchdog")):
                with self.subTest(platform=platform), patch.object(runtime.sys, "platform", platform):
                    self.assertEqual(runtime.backend(), expected)
                    with patch.dict(os.environ, TMPDIR=str(self.base / "one")):
                        first = runtime.lock_path({})
                    with patch.dict(os.environ, TMPDIR=str(self.base / "two")):
                        self.assertEqual(first, runtime.lock_path({}))
                    self.assertTrue(first.is_absolute())

    def test_linux_scope_applies_real_cgroup_limits(self):
        if sys.platform != "linux":
            self.skipTest("cgroups are Linux-specific")
        env = os.environ.copy()
        env.setdefault("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
        status = subprocess.run(["systemctl", "--user", "show-environment"], env=env, capture_output=True)
        if status.returncode:
            self.skipTest("Linux runner has no systemd user bus; no cgroup success claim")
        code = "from pathlib import Path; r=Path('/sys/fs/cgroup')/Path('/proc/self/cgroup').read_text().strip().split(':',2)[2].lstrip('/'); assert (r/'memory.max').read_text().strip()=='134217728'; assert (r/'memory.swap.max').read_text().strip()=='0'; assert (r/'cpu.max').read_text().strip()=='100000 100000'"
        result = subprocess.run(["systemd-run", "--user", "--scope", "--quiet", "--collect", "-p", "MemoryMax=128M", "-p", "MemorySwapMax=0", "-p", "CPUQuota=100%", sys.executable, "-B", "-c", code], env=env, capture_output=True, text=True, encoding="utf-8", timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
