"""OS operations shared by the workflow; payloads never run through a shell."""
import errno
import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path
import shutil

try:
    import psutil
except ImportError:
    psutil = None


class WorkflowError(Exception):
    def __init__(self, message, code=2):
        super().__init__(message)
        self.code = code


class PayloadCleanupError(WorkflowError):
    """The created payload boundary could not be proved fully cleaned."""


def require_process_support():
    if psutil is None:
        raise WorkflowError("缺少 psutil；使用当前 Python 执行 -m pip install -r scripts/requirements.txt")


def backend():
    if sys.platform == "linux":
        return "systemd"
    if sys.platform in ("darwin", "win32"):
        return "process-watchdog"
    raise WorkflowError(f"不支持的平台：{sys.platform}")


def user_home():
    if os.name != "nt":
        import pwd
        return Path(pwd.getpwuid(os.getuid()).pw_dir)
    return Path.home()


def lock_path(config):
    if config.get("heavy_lock"):
        return Path(config["heavy_lock"]).resolve()
    folder = {"win32": "AppData/Local", "darwin": "Library/Caches"}.get(sys.platform, ".cache")
    return (user_home() / folder / "agent-project-workflow/heavy.lock").resolve()


class FileLock:
    def __init__(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        self.stream = path.open("a+b")
        # Windows _locking can lock beyond EOF. A sentinel write is unnecessary
        # and could fail before acquire() when another process holds byte zero.
        self.locked = False

    def acquire(self):
        self.stream.seek(0)
        try:
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(self.stream.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.stream.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.locked = True
            return True
        except OSError as error:
            if error.errno in (errno.EACCES, errno.EAGAIN, errno.EDEADLK):
                return False
            raise

    def fileno(self):
        return self.stream.fileno()

    def close(self):
        if self.locked:
            self.stream.seek(0)
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(self.stream.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.stream.fileno(), fcntl.LOCK_UN)
        self.stream.close()

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.close()


def identity(pid):
    require_process_support()
    try:
        process = psutil.Process(pid)
        if not process.is_running() or process.status() == psutil.STATUS_ZOMBIE:
            return None
        return "created:" + repr(process.create_time())
    except psutil.NoSuchProcess:
        return None
    except psutil.AccessDenied as error:
        raise WorkflowError(f"无法确认进程身份：PID={pid}") from error


def process_for(pid, token):
    if not token or identity(pid) != token:
        return None
    try:
        process = psutil.Process(pid)
        if "created:" + repr(process.create_time()) == token and process.is_running():
            return process
    except psutil.NoSuchProcess:
        pass
    return None


def alive(ref):
    return bool(ref.get("pid") and ref.get("identity")) and identity(ref["pid"]) == ref["identity"]


def signal_exact(pid, token, sig):
    process = process_for(pid, token)
    if process is None:
        return False
    try:
        if sig == signal.SIGTERM:
            process.terminate()
        else:
            process.kill()
        return True
    except psutil.NoSuchProcess:
        return False
    except psutil.AccessDenied as error:
        raise WorkflowError(f"不能停止已确认的进程：PID={pid}") from error


def family(pid, token, excluded=()):
    root = process_for(pid, token)
    if root is None:
        return []
    try:
        omit = set(excluded)
        children = root.children(recursive=True)
        for child in children:
            if child.pid in omit:
                omit.update(item.pid for item in child.children(recursive=True))
        result = []
        for child in [root, *children]:
            if child.pid not in omit:
                value = identity(child.pid)
                if value:
                    result.append(dict(pid=child.pid, identity=value))
        return result
    except psutil.NoSuchProcess:
        return []
    except psutil.AccessDenied as error:
        raise WorkflowError("无法核对自己的子进程树，停止推进") from error


def terminate(refs):
    unique = {(item["pid"], item["identity"]) for item in refs if item.get("pid") and item.get("identity")}
    for pid, token in unique:
        signal_exact(pid, token, signal.SIGTERM)
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline and any(identity(pid) == token for pid, token in unique):
        time.sleep(.05)
    for pid, token in unique:
        signal_exact(pid, token, getattr(signal, "SIGKILL", 9))
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        if not any(identity(pid) == token for pid, token in unique):
            return
        time.sleep(.05)
    raise WorkflowError("自己启动的进程清理未完成；停止推进")


def resolve_argv(argv):
    args = list(argv)
    if args[0] == "{python}":
        args[0] = sys.executable
    executable = None
    if Path(args[0]).parent != Path(".") or Path(args[0]).is_absolute():
        executable = args[0]
    else:
        for folder in os.get_exec_path():
            executable = shutil.which(args[0], path=folder)
            script = Path(folder) / (args[0] + ".py")
            if executable or script.is_file():
                executable = executable or str(script)
                break
    executable = executable or args[0]
    path = Path(executable)
    if path.suffix.lower() == ".py":
        return [sys.executable, "-B", str(path), *args[1:]]
    if os.name == "nt" and path.suffix.lower() in (".cmd", ".bat"):
        # npm CLI shims are batch files; launch their JS entry with Node instead of cmd.exe.
        # npm 11 uses a variable-based shim. Resolve only the known local entry;
        # arbitrary batch files must still fail without invoking cmd.exe.
        entry = None
        if path.stem.lower() in ("npm", "npx"):
            candidate = path.parent / "node_modules" / "npm" / "bin" / f"{path.stem.lower()}-cli.js"
            if candidate.is_file():
                entry = candidate
        if entry is None:
            match = re.search(r'"%dp0%\\([^"%]+\.[cm]?js)"', path.read_text(encoding="utf-8", errors="replace")) if path.is_file() else None
            entry = path.parent / match.group(1).replace("\\", "/") if match else None
        node = path.parent / "node.exe"
        node_path = str(node) if node.is_file() else shutil.which("node")
        if entry and entry.is_file() and node_path:
            return [node_path, str(entry), *args[1:]]
        raise WorkflowError("不能隐式执行 Windows batch 文件；使用原生程序、Python 脚本或显式解释器 argv")
    return [str(path), *args[1:]]


def launch(argv, *, detached=False, inherited_fds=(), **kwargs):
    if os.name == "nt":
        kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP if detached else 0
    else:
        kwargs["start_new_session"] = detached
        kwargs["pass_fds"] = inherited_fds
    return subprocess.Popen(resolve_argv(argv), **kwargs)


def session_family(session_id, leader_token):
    """A dedicated POSIX session survives its leader; parent links do not."""
    try:
        # Keep the creation token even after the leader becomes a zombie. If
        # that numerical PID now names another process, it cannot authorize us
        # to stop a newly created session bearing the same numerical ID.
        if "created:" + repr(psutil.Process(session_id).create_time()) != leader_token:
            raise WorkflowError("payload session leader PID 已复用；拒绝清理无关进程组")
    except psutil.NoSuchProcess:
        # POSIX retains the session/process-group identity while members remain.
        pass
    except psutil.AccessDenied as error:
        raise WorkflowError("无法核对 payload session leader 身份") from error
    result = []
    for process in psutil.process_iter():
        try:
            if os.getsid(process.pid) == session_id:
                token = identity(process.pid)
                if token:
                    result.append(dict(pid=process.pid, identity=token))
        except (ProcessLookupError, psutil.NoSuchProcess):
            pass
        except PermissionError:
            # Other users' processes need not be inspectable. A process in our
            # own session that cannot be inspected is rejected by identity().
            continue
    return result


def terminate_session(session_id, leader_token):
    """Stop every process group in the payload session, including reparented children."""
    for sig, seconds in ((signal.SIGTERM, 3), (signal.SIGKILL, 2)):
        deadline = time.monotonic() + seconds
        while True:
            refs = session_family(session_id, leader_token)
            if not refs:
                return
            groups = set()
            for item in refs:
                try:
                    if alive(item) and os.getsid(item["pid"]) == session_id:
                        groups.add(os.getpgid(item["pid"]))
                except ProcessLookupError:
                    pass
            for group in groups:
                try:
                    os.killpg(group, sig)
                except ProcessLookupError:
                    pass
                except PermissionError as error:
                    raise WorkflowError("不能停止自己的 payload 进程组") from error
            if time.monotonic() >= deadline:
                break
            time.sleep(.05)
    if session_family(session_id, leader_token):
        raise WorkflowError("自己启动的进程组清理未完成；停止推进")


def run_payload(argv, *, cwd, env, seconds, memory_mib, sampled, lock_fd, observe):
    require_process_support()
    cleanup_exceptions = (WorkflowError, OSError, subprocess.TimeoutExpired)
    if os.name == "nt":
        from windows_job import WindowsPayload, JobError
        cleanup_exceptions += (JobError,)
        try:
            process = WindowsPayload(resolve_argv(argv), cwd=cwd, env=env)
        except JobError as error:
            raise WorkflowError(f"不能建立 Windows 10+ Job 生命周期边界；命令未运行：{error}") from error

        def members():
            refs = []
            for pid in process.pids():
                token = identity(pid)
                # A queried PID might already have exited and been reused.
                if token and process.contains(pid) and identity(pid) == token:
                    refs.append(dict(pid=pid, identity=token))
            return refs
    else:
        process = launch(argv, detached=True, inherited_fds=(lock_fd,), cwd=cwd, env=env,
                         stdin=subprocess.DEVNULL)
    known, peak, forced, note, token = {}, 0, None, "", None
    try:
        # A POSIX zombie still has its original birth time until poll() reaps it;
        # capture the anchor before the first poll, including quick root exits.
        token = (identity(process.pid) if os.name == "nt" else
                 "created:" + repr(psutil.Process(process.pid).create_time()))
        anchor = dict(pid=process.pid, identity=token)
        if token:
            known[(process.pid, token)] = anchor
        observe(process.pid, token, list(known.values()))
        started = time.monotonic()
        if os.name != "nt":
            members = lambda: session_family(process.pid, token)
        while True:
            # Enumerate the boundary even if the root has already exited.
            # A quick spawn-and-exit parent must not hide descendants.
            refs = members()
            known.update({(item["pid"], item["identity"]): item for item in refs})
            observe(process.pid, token, list(known.values()))
            if process.poll() is not None:
                break
            if sampled:
                total = 0
                for item in refs:
                    member = process_for(item["pid"], item["identity"])
                    if member:
                        try:
                            total += member.memory_info().rss
                        except psutil.NoSuchProcess:
                            pass
                        except psutil.AccessDenied as error:
                            raise WorkflowError("无法监控重命令内存，停止运行") from error
                peak = max(peak, total)
                if total > memory_mib * 1048576:
                    forced, note = 137, "进程树 RSS 超过采样内存上限；已停止自己的进程树"
            if time.monotonic() - started >= seconds:
                forced, note = 124, f"命令超时：{seconds}s"
            if forced is not None:
                break
            time.sleep(.1)
    finally:
        cleanup_errors = []
        try:
            if os.name == "nt":
                process.terminate()
            else:
                if token is None:
                    # Popen retains child ownership and checks exit before
                    # signaling. No unverified SID may authorize a group kill.
                    cleanup_errors.append("payload 身份未登记，无法确认其 session 边界")
                    process.kill()
                else:
                    terminate_session(process.pid, token)
        except cleanup_exceptions as error:
            cleanup_errors.append(str(error))
            if os.name != "nt" and token:
                # The unreaped Popen child anchors this initial group even when
                # enumeration is denied. Other groups remain unverified.
                try:
                    if ("created:" + repr(psutil.Process(process.pid).create_time()) == token
                            and os.getsid(process.pid) == process.pid
                            and os.getpgid(process.pid) == process.pid):
                        os.killpg(process.pid, signal.SIGKILL)
                except (psutil.NoSuchProcess, ProcessLookupError):
                    pass
                except (WorkflowError, OSError, psutil.AccessDenied) as failure:
                    cleanup_errors.append(str(failure))
        finally:
            if os.name != "nt":
                # A session enumeration error must not skip already-known
                # children, including any that changed their process group.
                try:
                    terminate(list(known.values()))
                except cleanup_exceptions as error:
                    cleanup_errors.append(str(error))
            try:
                if os.name != "nt" and process.poll() is None:
                    process.kill()
                process.wait(timeout=5)
            except cleanup_exceptions as error:
                cleanup_errors.append(str(error))
            finally:
                if os.name == "nt":
                    try:
                        process.close()
                    except cleanup_exceptions as error:
                        cleanup_errors.append(str(error))
        if os.name == "nt":
            # Job termination is asynchronous, and a just-exited process can
            # remain queryable briefly after the job reports no active PIDs.
            deadline = time.monotonic() + 5
            while any(alive(ref) for ref in known.values()) and time.monotonic() < deadline:
                time.sleep(.05)
            if any(alive(ref) for ref in known.values()):
                cleanup_errors.append("Windows Job 已终止，但已登记的进程仍可查询")
        if cleanup_errors:
            raise PayloadCleanupError("payload 边界清理未确认：" + "; ".join(cleanup_errors), 125)
    raw = process.returncode
    return dict(raw_exit=raw, exit=forced if forced is not None else raw if raw >= 0 else 128 - raw,
                note=note, peak_rss_bytes=peak if sampled else None)
