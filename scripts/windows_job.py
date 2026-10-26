"""Windows 10+ payload lifetime boundary using documented Win32 Job APIs.

The job is assigned during CreateProcessW, not by sampling an already running
child. Only the guardian owns its handle; guardian death kills the entire job.
See Microsoft Learn: UpdateProcThreadAttribute / PROC_THREAD_ATTRIBUTE_JOB_LIST,
Job Objects / JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, and QueryInformationJobObject.
"""
import ctypes
from ctypes import wintypes as w
import os
import subprocess
import time


class JobError(OSError):
    pass


SIZE_T = ctypes.c_size_t
ULONG_PTR = ctypes.c_size_t
LPVOID = ctypes.c_void_p


class BasicLimits(ctypes.Structure):
    _fields_ = [("PerProcessUserTimeLimit", ctypes.c_int64),
                ("PerJobUserTimeLimit", ctypes.c_int64), ("LimitFlags", w.DWORD),
                ("MinimumWorkingSetSize", SIZE_T), ("MaximumWorkingSetSize", SIZE_T),
                ("ActiveProcessLimit", w.DWORD), ("Affinity", ULONG_PTR),
                ("PriorityClass", w.DWORD), ("SchedulingClass", w.DWORD)]


class IoCounters(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint64) for name in
                ("ReadOperationCount", "WriteOperationCount", "OtherOperationCount",
                 "ReadTransferCount", "WriteTransferCount", "OtherTransferCount")]


class ExtendedLimits(ctypes.Structure):
    _fields_ = [("BasicLimitInformation", BasicLimits), ("IoInfo", IoCounters),
                ("ProcessMemoryLimit", SIZE_T), ("JobMemoryLimit", SIZE_T),
                ("PeakProcessMemoryUsed", SIZE_T), ("PeakJobMemoryUsed", SIZE_T)]


class StartupInfo(ctypes.Structure):
    _fields_ = [("cb", w.DWORD), ("lpReserved", w.LPWSTR), ("lpDesktop", w.LPWSTR),
                ("lpTitle", w.LPWSTR), ("dwX", w.DWORD), ("dwY", w.DWORD),
                ("dwXSize", w.DWORD), ("dwYSize", w.DWORD), ("dwXCountChars", w.DWORD),
                ("dwYCountChars", w.DWORD), ("dwFillAttribute", w.DWORD),
                ("dwFlags", w.DWORD), ("wShowWindow", w.WORD), ("cbReserved2", w.WORD),
                ("lpReserved2", LPVOID), ("hStdInput", w.HANDLE),
                ("hStdOutput", w.HANDLE), ("hStdError", w.HANDLE)]


class StartupInfoEx(ctypes.Structure):
    _fields_ = [("StartupInfo", StartupInfo), ("lpAttributeList", LPVOID)]


class ProcessInfo(ctypes.Structure):
    _fields_ = [("hProcess", w.HANDLE), ("hThread", w.HANDLE),
                ("dwProcessId", w.DWORD), ("dwThreadId", w.DWORD)]


def _api():
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    signatures = {
        "CreateJobObjectW": ([LPVOID, w.LPCWSTR], w.HANDLE),
        "SetInformationJobObject": ([w.HANDLE, ctypes.c_int, LPVOID, w.DWORD], w.BOOL),
        "QueryInformationJobObject": ([w.HANDLE, ctypes.c_int, LPVOID, w.DWORD, LPVOID], w.BOOL),
        "TerminateJobObject": ([w.HANDLE, w.UINT], w.BOOL),
        "InitializeProcThreadAttributeList": ([LPVOID, w.DWORD, w.DWORD, ctypes.POINTER(SIZE_T)], w.BOOL),
        "UpdateProcThreadAttribute": ([LPVOID, w.DWORD, ULONG_PTR, LPVOID, SIZE_T, LPVOID, LPVOID], w.BOOL),
        "DeleteProcThreadAttributeList": ([LPVOID], None),
        "CreateProcessW": ([w.LPCWSTR, w.LPWSTR, LPVOID, LPVOID, w.BOOL, w.DWORD,
                            LPVOID, w.LPCWSTR, LPVOID, ctypes.POINTER(ProcessInfo)], w.BOOL),
        "GetCurrentProcess": ([], w.HANDLE),
        "GetStdHandle": ([w.DWORD], w.HANDLE),
        "DuplicateHandle": ([w.HANDLE, w.HANDLE, w.HANDLE, ctypes.POINTER(w.HANDLE),
                             w.DWORD, w.BOOL, w.DWORD], w.BOOL),
        "CloseHandle": ([w.HANDLE], w.BOOL),
        "ResumeThread": ([w.HANDLE], w.DWORD),
        "WaitForSingleObject": ([w.HANDLE, w.DWORD], w.DWORD),
        "GetExitCodeProcess": ([w.HANDLE, ctypes.POINTER(w.DWORD)], w.BOOL),
        "OpenProcess": ([w.DWORD, w.BOOL, w.DWORD], w.HANDLE),
        "IsProcessInJob": ([w.HANDLE, w.HANDLE, ctypes.POINTER(w.BOOL)], w.BOOL),
    }
    for name, (args, result) in signatures.items():
        function = getattr(kernel, name)
        function.argtypes, function.restype = args, result
    return kernel


def _check(value, operation):
    if not value:
        error = ctypes.get_last_error()
        raise JobError(error, f"{operation}: {ctypes.FormatError(error).strip()}")
    return value


class WindowsPayload:
    """Popen-like root process plus an atomic, non-inheritable Job boundary."""
    def __init__(self, argv, *, cwd, env):
        if os.name != "nt":
            raise JobError("Windows Job runtime requires Windows 10 or newer")
        self.kernel = _api()
        self.job = self.handle = None
        self.pid, self.returncode = None, None
        info, inherited, attributes = ProcessInfo(), [], None
        attributes_ready = False
        try:
            self.job = _check(self.kernel.CreateJobObjectW(None, None), "CreateJobObjectW")
            limits = ExtendedLimits()
            # Do not set either breakaway flag; descendants must remain in the job.
            limits.BasicLimitInformation.LimitFlags = 0x2000  # KILL_ON_JOB_CLOSE
            _check(self.kernel.SetInformationJobObject(self.job, 9, ctypes.byref(limits),
                                                       ctypes.sizeof(limits)), "SetInformationJobObject")
            startup = StartupInfoEx()
            startup.StartupInfo.cb = ctypes.sizeof(startup)
            startup.StartupInfo.dwFlags = 0x100  # STARTF_USESTDHANDLES
            # Duplicate only stdio handles. The job and lock handles are never
            # inherited, so descendants cannot keep the job alive after death.
            with open(os.devnull, "rb") as empty, open(os.devnull, "wb") as discard:
                import msvcrt
                inputs = [msvcrt.get_osfhandle(empty.fileno())]
                for standard in (-11, -12):
                    handle = self.kernel.GetStdHandle(standard & 0xffffffff)
                    if handle in (None, ctypes.c_void_p(-1).value):
                        handle = msvcrt.get_osfhandle(discard.fileno())
                    inputs.append(handle)
                for handle in inputs:
                    duplicate = w.HANDLE()
                    current = self.kernel.GetCurrentProcess()
                    _check(self.kernel.DuplicateHandle(current, handle, current,
                                                       ctypes.byref(duplicate), 0, True, 2), "DuplicateHandle")
                    inherited.append(duplicate.value)
            (startup.StartupInfo.hStdInput, startup.StartupInfo.hStdOutput,
             startup.StartupInfo.hStdError) = inherited
            length = SIZE_T()
            self.kernel.InitializeProcThreadAttributeList(None, 2, 0, ctypes.byref(length))
            if not length.value:
                _check(False, "InitializeProcThreadAttributeList(size)")
            attributes = ctypes.create_string_buffer(length.value)
            _check(self.kernel.InitializeProcThreadAttributeList(attributes, 2, 0, ctypes.byref(length)),
                   "InitializeProcThreadAttributeList")
            attributes_ready = True
            startup.lpAttributeList = ctypes.cast(attributes, LPVOID)
            handles = (w.HANDLE * len(inherited))(*inherited)
            jobs = (w.HANDLE * 1)(self.job)
            _check(self.kernel.UpdateProcThreadAttribute(attributes, 0, 0x20002, handles,
                                                        ctypes.sizeof(handles), None, None), "HANDLE_LIST")
            # JOB_LIST requires Windows 10+. Failure stops before payload creation;
            # never fall back to a running process followed by AssignProcessToJobObject.
            _check(self.kernel.UpdateProcThreadAttribute(attributes, 0, 0x2000D, jobs,
                                                        ctypes.sizeof(jobs), None, None), "JOB_LIST (Windows 10+)")
            command = ctypes.create_unicode_buffer(subprocess.list2cmdline([os.fspath(arg) for arg in argv]))
            environment = ctypes.create_unicode_buffer("\0".join(
                f"{key}={value}" for key, value in sorted(env.items(), key=lambda item: item[0].upper())) + "\0\0")
            flags = 0x4 | 0x200 | 0x400 | 0x80000 | 0x8000000
            # CREATE_SUSPENDED + CREATE_NEW_PROCESS_GROUP + CREATE_UNICODE_ENVIRONMENT
            # + EXTENDED_STARTUPINFO_PRESENT + CREATE_NO_WINDOW. Job assignment is
            # atomic even if the guardian dies before ResumeThread.
            _check(self.kernel.CreateProcessW(None, command, None, None, True, flags,
                                              environment, os.fspath(cwd), ctypes.byref(startup),
                                              ctypes.byref(info)), "CreateProcessW (atomic Job assignment)")
            self.handle, self.pid = info.hProcess, info.dwProcessId
            if self.kernel.ResumeThread(info.hThread) == 0xffffffff:
                _check(False, "ResumeThread")
        except BaseException:
            if self.job:
                self.kernel.CloseHandle(self.job)
                self.job = None
            if self.handle:
                self.kernel.WaitForSingleObject(self.handle, 5000)
                self.kernel.CloseHandle(self.handle)
                self.handle = None
            raise
        finally:
            if info.hThread:
                self.kernel.CloseHandle(info.hThread)
            if attributes_ready:
                self.kernel.DeleteProcThreadAttributeList(attributes)
            for handle in inherited:
                self.kernel.CloseHandle(handle)

    def poll(self):
        if self.returncode is None:
            state = self.kernel.WaitForSingleObject(self.handle, 0)
            if state == 0:
                code = w.DWORD()
                _check(self.kernel.GetExitCodeProcess(self.handle, ctypes.byref(code)), "GetExitCodeProcess")
                self.returncode = code.value
            elif state != 0x102:  # WAIT_TIMEOUT
                _check(False, "WaitForSingleObject")
        return self.returncode

    def wait(self, timeout=5):
        state = self.kernel.WaitForSingleObject(self.handle, min(int(timeout * 1000), 0xfffffffe))
        if state == 0x102:
            raise subprocess.TimeoutExpired(self.pid, timeout)
        if state != 0:
            _check(False, "WaitForSingleObject")
        return self.poll()

    def pids(self):
        capacity = 64
        while True:
            class PidList(ctypes.Structure):
                _fields_ = [("NumberOfAssignedProcesses", w.DWORD),
                            ("NumberOfProcessIdsInList", w.DWORD),
                            ("ProcessIdList", ULONG_PTR * capacity)]
            value = PidList()
            if self.kernel.QueryInformationJobObject(self.job, 3, ctypes.byref(value), ctypes.sizeof(value), None):
                return list(value.ProcessIdList[:value.NumberOfProcessIdsInList])
            if ctypes.get_last_error() != 234:  # ERROR_MORE_DATA
                _check(False, "QueryInformationJobObject")
            capacity = max(capacity * 2, value.NumberOfAssignedProcesses)

    def contains(self, pid):
        handle = self.kernel.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
        if not handle:
            if ctypes.get_last_error() == 87:  # process exited
                return False
            _check(False, "OpenProcess (Job identity)")
        try:
            value = w.BOOL()
            _check(self.kernel.IsProcessInJob(handle, self.job, ctypes.byref(value)), "IsProcessInJob")
            return bool(value.value)
        finally:
            self.kernel.CloseHandle(handle)

    def terminate(self):
        _check(self.kernel.TerminateJobObject(self.job, 1), "TerminateJobObject")
        deadline = time.monotonic() + 5
        while self.pids():
            if time.monotonic() >= deadline:
                raise JobError("Windows Job process cleanup did not finish")
            time.sleep(.05)
        self.wait(timeout=5)

    def close(self):
        if self.job:
            self.kernel.CloseHandle(self.job)
            self.job = None
        if self.handle:
            self.kernel.CloseHandle(self.handle)
            self.handle = None
