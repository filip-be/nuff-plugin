import json
import os
import re
import runpy
import subprocess
import sys
import time
from pathlib import Path

HOOKS = {"sessionStart", "agentStop", "sessionEnd"}
SESSION_ID = re.compile(r"[0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}")
ENUMS = {
    "source": {"new", "resume", "startup", "clear", "compact", "fork"},
    "stopReason": {"end_turn", "abort", "error", "cancelled", "canceled", "stop", "max_turns"},
    "reason": {"user_exit", "complete", "error", "abort", "timeout", "other", "clear",
               "logout", "prompt_input_exit", "cancelled", "canceled"},
}


def parents():
    if os.name == "nt":
        win = runpy.run_path(str(Path(__file__).with_name("win_process.py")))
        chain = list(win["process_chain"](os.getpid(), *win["snapshot"](), limit=9))
        return [exe for _, exe in chain[1:]]
    names = []
    pid = os.getppid()
    for _ in range(8):
        if pid <= 1:
            break
        row = subprocess.check_output(
            ["ps", "-p", str(pid), "-o", "ppid=", "-o", "comm="],
            text=True, stderr=subprocess.DEVNULL, timeout=0.3,
        ).strip().split(maxsplit=1)
        if len(row) != 2:
            break
        pid, exe = int(row[0]), Path(row[1]).name
        names.append(exe if re.fullmatch(r"[A-Za-z0-9_. +()-]{1,100}", exe) else "<exe>")
    return names


def main():
    hook, = sys.argv[1:]
    if hook not in HOOKS:
        return
    payload = json.load(sys.stdin.buffer)
    if not isinstance(payload, dict):
        return
    session = payload.get("sessionId")
    if not isinstance(session, str) or not SESSION_ID.fullmatch(session):
        return
    selected = {"sessionId": session}
    stamp = payload.get("timestamp")
    if type(stamp) is int:
        selected["timestamp"] = stamp
    for key, allowed in ENUMS.items():
        value = payload.get(key)
        if isinstance(value, str) and value in allowed:
            selected[key] = value
    try:
        chain = parents()
    except (OSError, ValueError, subprocess.SubprocessError):
        chain = []
    record = {
        "hook": hook, "writeTimeMs": time.time_ns() // 1_000_000,
        "stdinKeys": sorted({k if re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]{0,63}", k)
                             else "<key>" for k in payload}),
        "stdinValues": selected, "parentExes": chain,
    }
    root = Path.home() / ".nuff-pulse-probe"
    root.mkdir(mode=0o700, exist_ok=True)
    flags = os.O_WRONLY | os.O_APPEND | os.O_CREAT | getattr(os, "O_BINARY", 0)
    fd = os.open(root / f"hooks-{session}.jsonl", flags, 0o600)
    try:
        os.write(fd, (json.dumps(record, separators=(",", ":")) + "\n").encode("ascii"))
    finally:
        os.close(fd)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError):
        pass  # Observational hook: never block or forward input in an error.
