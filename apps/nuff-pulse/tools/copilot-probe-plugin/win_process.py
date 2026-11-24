import ctypes as c
import json
import os
import re
import sys
from ctypes import wintypes as w
from pathlib import Path

kernel = c.WinDLL("kernel32", use_last_error=True)
user = c.WinDLL("user32", use_last_error=True)
INVALID_HANDLE = c.c_void_p(-1).value


class ProcessEntry(c.Structure):
    _fields_ = [
        ("dwSize", w.DWORD), ("cntUsage", w.DWORD), ("pid", w.DWORD),
        ("heap", c.c_size_t), ("module", w.DWORD), ("threads", w.DWORD),
        ("parent", w.DWORD), ("priority", w.LONG), ("flags", w.DWORD),
        ("exe", w.WCHAR * 260),
    ]


def bind(dll, name, result, *args):
    fn = getattr(dll, name)
    fn.restype, fn.argtypes = result, args
    return fn


snapshot_handle = bind(kernel, "CreateToolhelp32Snapshot", w.HANDLE, w.DWORD, w.DWORD)
first = bind(kernel, "Process32FirstW", w.BOOL, w.HANDLE, c.POINTER(ProcessEntry))
next_entry = bind(kernel, "Process32NextW", w.BOOL, w.HANDLE, c.POINTER(ProcessEntry))
close = bind(kernel, "CloseHandle", w.BOOL, w.HANDLE)
open_process = bind(kernel, "OpenProcess", w.HANDLE, w.DWORD, w.BOOL, w.DWORD)
get_times = bind(kernel, "GetProcessTimes", w.BOOL, w.HANDLE, *([c.POINTER(w.FILETIME)] * 4))
clock = bind(kernel, "GetSystemTimeAsFileTime", None, c.POINTER(w.FILETIME))
wait = bind(kernel, "WaitForSingleObject", w.DWORD, w.HANDLE, w.DWORD)
create_file = bind(kernel, "CreateFileW", w.HANDLE, w.LPCWSTR, w.DWORD, w.DWORD,
                   c.c_void_p, w.DWORD, w.DWORD, w.HANDLE)
window_callback = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
enum_windows = bind(user, "EnumWindows", w.BOOL, window_callback, w.LPARAM)
is_visible = bind(user, "IsWindowVisible", w.BOOL, w.HWND)
window_pid = bind(user, "GetWindowThreadProcessId", w.DWORD, w.HWND, c.POINTER(w.DWORD))
window_text = bind(user, "GetWindowTextW", c.c_int, w.HWND, w.LPWSTR, c.c_int)


def exe_name(name):
    return name if re.fullmatch(r"[A-Za-z0-9_. +()-]{1,100}", name) else "<exe>"


def filetime(value):
    return (value.dwHighDateTime << 32) | value.dwLowDateTime


def snapshot():
    before = w.FILETIME()
    clock(c.byref(before))
    handle = snapshot_handle(2, 0)  # TH32CS_SNAPPROCESS
    if handle == INVALID_HANDLE:
        raise c.WinError(c.get_last_error())
    try:
        entry = ProcessEntry()
        entry.dwSize = c.sizeof(entry)
        ok = first(handle, c.byref(entry))
        processes = {}
        while ok:
            processes[entry.pid] = (entry.parent, exe_name(entry.exe))
            ok = next_entry(handle, c.byref(entry))
        if c.get_last_error() != 18:  # ERROR_NO_MORE_FILES
            raise c.WinError(c.get_last_error())
        return processes, filetime(before)
    finally:
        close(handle)


def process_status(pid):
    handle = open_process(0x101000, False, pid)  # SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION
    if not handle:
        return False if c.get_last_error() == 87 else None, None
    try:
        status = wait(handle, 0)
        alive = {0: False, 258: True}.get(status)
        times = [w.FILETIME() for _ in range(4)]
        created = filetime(times[0]) if get_times(handle, *(c.byref(t) for t in times)) else None
        return alive, created
    finally:
        close(handle)


def process_chain(pid, processes, snapshot_time, limit=None):
    child_created = snapshot_time + 1
    seen = set()
    while pid in processes and pid not in seen and (limit is None or len(seen) < limit):
        alive, created = process_status(pid)
        if alive is not True or created is None or created >= child_created:
            break
        parent, exe = processes[pid]
        yield pid, exe
        seen.add(pid)
        pid, child_created = parent, created


def matching_windows(pids, folder):
    matches = {}
    folder = folder.casefold() if folder else None

    @window_callback
    def visit(hwnd, _):
        pid = w.DWORD()
        window_pid(hwnd, c.byref(pid))
        if pid.value in pids and is_visible(hwnd):
            matches.setdefault(pid.value, False)
            if folder and not matches[pid.value]:
                title = c.create_unicode_buffer(32768)
                window_text(hwnd, title, len(title))
                matches[pid.value] = folder in title.value.casefold()
        return True

    if not enum_windows(visit, 0):
        raise c.WinError(c.get_last_error())
    return matches


def exclusive_read(path):
    handle = create_file(str(path), 0x80000000, 0, None, 3, 0x80, None)
    if handle == INVALID_HANDLE:
        return c.get_last_error()
    close(handle)
    return 0


if __name__ == "__main__":
    # Invoked by the extension with its own process and parent IDs.
    try:
        child, parent = map(int, sys.argv[1:])
        chain = list(process_chain(child, *snapshot(), limit=9))
        names = [exe for _, exe in chain[1:]] if len(chain) > 1 and chain[1][0] == parent else []
        print(json.dumps(names))
    except (OSError, ValueError):
        sys.exit(1)
