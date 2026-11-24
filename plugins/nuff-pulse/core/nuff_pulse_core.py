"""Shared writer for the nuff-pulse hook plugins.

Each hook call appends one JSON line (a Pulse Line) to
`~/.nuff-pulse/sessions/<runtime>-<session_id>.hooks.jsonl`. The line format is
documented in the plugin README.

Maintainer note: the source of this file is `plugins/nuff-pulse/core/` in the
nuff-plugin repo; identical copies ship in each nuff-pulse plugin's `hooks/` folder.
"""

import json
import os
import re
import sys
import time
from pathlib import Path

VERSION = 1
MAX_LINE = 4000  # bytes, newline included
DETAIL_CHARS = 200
SESSION_ID = re.compile(r"[A-Za-z0-9_-]{1,128}")
TOKEN = re.compile(r"[A-Za-z0-9_.-]{1,64}")
STATES = {"working", "needs_you", "done", "error", "ended"}
MAX_PIDS = 16
LOCK_WAIT = 0.5  # seconds; after that the line is dropped (unlocked appends can overwrite)
# Launchers between the agent and this script: never the agent itself.
LAUNCHERS = {
    "py", "python", "python3", "pythonw", "bash", "sh", "dash", "zsh", "env",
    "pwsh", "powershell", "cmd", "conhost", "openconsole",
}


def token(value):
    """A short machine word (enum value), or None."""
    return value if isinstance(value, str) and TOKEN.fullmatch(value) else None


def short(value, limit=DETAIL_CHARS):
    """A single-line, length-limited string, or None."""
    if not isinstance(value, str):
        return None
    value = " ".join(value.split())
    if not value:
        return None
    return value if len(value) <= limit else value[: limit - 1] + "…"


def launcher(exe):
    name = exe.rsplit("/", 1)[-1].rsplit("\\", 1)[-1].casefold()
    if name.endswith(".exe"):
        name = name[:-4]
    return name in LAUNCHERS or re.fullmatch(r"python3(\.\d+)?", name) is not None


def agent_chain(chain):
    """Drop this script's launchers; the first remaining process is the agent."""
    for index, (_, exe, _) in enumerate(chain):
        if not launcher(exe):
            return chain[index:]
    return []


# --- process ancestry -------------------------------------------------------


def windows_chain(limit=24):
    """[(pid, exe, created unix s)] from this process upward (Toolhelp32, no WMI)."""
    import ctypes as c
    from ctypes import wintypes as w

    kernel = c.WinDLL("kernel32", use_last_error=True)

    class Entry(c.Structure):
        _fields_ = [
            ("dwSize", w.DWORD), ("cntUsage", w.DWORD), ("pid", w.DWORD),
            ("heap", c.c_size_t), ("module", w.DWORD), ("threads", w.DWORD),
            ("parent", w.DWORD), ("priority", w.LONG), ("flags", w.DWORD),
            ("exe", w.WCHAR * 260),
        ]

    def bind(name, result, *args):
        fn = getattr(kernel, name)
        fn.restype, fn.argtypes = result, args
        return fn

    snapshot = bind("CreateToolhelp32Snapshot", w.HANDLE, w.DWORD, w.DWORD)
    first = bind("Process32FirstW", w.BOOL, w.HANDLE, c.POINTER(Entry))
    following = bind("Process32NextW", w.BOOL, w.HANDLE, c.POINTER(Entry))
    close = bind("CloseHandle", w.BOOL, w.HANDLE)
    open_process = bind("OpenProcess", w.HANDLE, w.DWORD, w.BOOL, w.DWORD)
    times = bind("GetProcessTimes", w.BOOL, w.HANDLE, *([c.POINTER(w.FILETIME)] * 4))

    handle = snapshot(2, 0)  # TH32CS_SNAPPROCESS
    if handle in (None, c.c_void_p(-1).value):
        return []
    processes = {}
    try:
        entry = Entry()
        entry.dwSize = c.sizeof(entry)
        ok = first(handle, c.byref(entry))
        while ok:
            processes[entry.pid] = (entry.parent, entry.exe)
            ok = following(handle, c.byref(entry))
    finally:
        close(handle)

    def created(pid):
        process = open_process(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
        if not process:
            return None
        try:
            stamps = [w.FILETIME() for _ in range(4)]
            if not times(process, *(c.byref(s) for s in stamps)):
                return None
            value = (stamps[0].dwHighDateTime << 32) | stamps[0].dwLowDateTime
            return (value - 116444736000000000) / 1e7
        finally:
            close(process)

    chain, pid, child_created = [], os.getpid(), float("inf")
    while pid in processes and len(chain) < limit and pid not in {p for p, _, _ in chain}:
        started = created(pid)
        # A parent created after its child means the pid was reused: stop.
        if started is None or started > child_created:
            break
        parent, exe = processes[pid]
        chain.append((pid, exe, started))
        pid, child_created = parent, started
    return chain


def linux_chain(limit=24, proc=Path("/proc")):
    """[(pid, comm, started unix s)] from this process upward, read from /proc."""
    try:
        ticks = os.sysconf("SC_CLK_TCK")
        boot = next(
            float(line.split()[1])
            for line in (proc / "stat").read_text().splitlines()
            if line.startswith("btime ")
        )
    except (OSError, ValueError, StopIteration):
        return []
    chain, pid = [], os.getpid()
    while pid > 0 and len(chain) < limit:
        try:
            stat = (proc / str(pid) / "stat").read_text()
        except OSError:
            break
        comm = stat[stat.index("(") + 1 : stat.rindex(")")]
        fields = stat[stat.rindex(")") + 2 :].split()
        chain.append((pid, comm, boot + int(fields[19]) / ticks))
        pid = int(fields[1])
    return chain


def host_info():
    """The `host` object: OS, the agent's pid and start time, and its ancestors."""
    if os.name == "nt":
        host, reader = {"os": "windows"}, windows_chain
    elif sys.platform.startswith("linux"):
        distro = os.environ.get("WSL_DISTRO_NAME")
        host = {"os": "wsl", "distro": short(distro, 64)} if distro else {"os": "linux"}
        reader = linux_chain
    else:
        return {"os": sys.platform}
    try:
        chain = agent_chain(reader())
    except (OSError, ValueError, IndexError, AttributeError):
        chain = []
    if chain:
        host["pids"] = [pid for pid, _, _ in chain[:MAX_PIDS]]
        host["agent_pid"] = chain[0][0]
        host["agent_started"] = round(chain[0][2], 3)
    return {k: v for k, v in host.items() if v is not None}


# --- the line ---------------------------------------------------------------


def build_line(runtime, session_id, event, state=None, reason=None, detail=None,
               cwd=None, log_path=None, t=None, host=None):
    """The Pulse Line as a dict, or None when the session id is unusable."""
    if not isinstance(session_id, str) or not SESSION_ID.fullmatch(session_id):
        return None
    line = {
        "v": VERSION,
        "t": round(time.time() if t is None else t, 3),
        "runtime": runtime,
        "session_id": session_id,
        "event": token(event),
        "state": state if state in STATES else None,
        "reason": token(reason),
        "detail": short(detail),
        "cwd": cwd if isinstance(cwd, str) and cwd else None,
        "log_path": log_path if isinstance(log_path, str) and log_path else None,
        "host": host,
    }
    return {k: v for k, v in line.items() if v is not None}


def encode(line):
    """UTF-8 JSON + newline, at most MAX_LINE bytes (drops detail, then paths)."""
    line = dict(line)
    for drop in (None, "detail", "log_path", "cwd", "host"):
        if drop:
            line.pop(drop, None)
        data = (json.dumps(line, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
        if len(data) <= MAX_LINE:
            return data
    return None


def sessions_dir():
    return Path.home() / ".nuff-pulse" / "sessions"


def _lock(fd):
    """Lock byte 0 of the file; True when held. Gives up after LOCK_WAIT."""
    deadline = time.monotonic() + LOCK_WAIT
    while True:
        try:
            if os.name == "nt":
                import msvcrt

                os.lseek(fd, 0, os.SEEK_SET)
                msvcrt.locking(fd, msvcrt.LK_NBLCK, 1)
            else:
                import fcntl

                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return True
        except OSError:
            if time.monotonic() >= deadline:
                return False
            time.sleep(0.01)


def _unlock(fd):
    if os.name == "nt":
        import msvcrt

        os.lseek(fd, 0, os.SEEK_SET)
        msvcrt.locking(fd, msvcrt.LK_UNLCK, 1)
    else:
        import fcntl

        fcntl.flock(fd, fcntl.LOCK_UN)


def append(line, folder=None):
    """Append one encoded line to the session's file. Returns the path, or None
    when nothing was written (line unusable, or the lock not taken in time)."""
    data = encode(line)
    if data is None:
        return None
    folder = Path(folder) if folder else sessions_dir()
    folder.mkdir(mode=0o700, parents=True, exist_ok=True)
    path = folder / f"{line['runtime']}-{line['session_id']}.hooks.jsonl"
    flags = os.O_WRONLY | os.O_APPEND | os.O_CREAT | getattr(os, "O_BINARY", 0)
    fd = os.open(path, flags, 0o600)
    try:
        if not _lock(fd):
            return None
        try:
            view = memoryview(data)
            while view:
                view = view[os.write(fd, view):]
        finally:
            _unlock(fd)
    finally:
        os.close(fd)
    return path


def read_stdin():
    """The hook's JSON input as a dict ({} when missing or not an object)."""
    try:
        payload = json.load(sys.stdin.buffer)
    except (ValueError, OSError):
        return {}
    return payload if isinstance(payload, dict) else {}
