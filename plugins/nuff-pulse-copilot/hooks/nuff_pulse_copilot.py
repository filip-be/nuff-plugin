"""GitHub Copilot hooks for nuff-pulse: append one Pulse Line per hook call.

Usage: nuff_pulse_copilot.py <agentStop|sessionEnd>, hook input JSON on stdin.
Fails open: never prints, never blocks, always exits 0.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import nuff_pulse_core as core  # noqa: E402

RUNTIME = "copilot"


def classify(event, payload):
    """(state, reason) for a hook event; None means write no line."""
    if event == "agentStop":
        reason = payload.get("stopReason")
        return ("error" if reason == "error" else "done"), reason
    if event == "sessionEnd":
        return "ended", payload.get("reason")
    return None


def event_time(payload):
    """The hook's `timestamp` (int ms) in seconds, else now."""
    stamp = payload.get("timestamp")
    return stamp / 1000 if type(stamp) is int and stamp > 0 else None


def main(argv):
    event = argv[1] if len(argv) == 2 else None
    payload = core.read_stdin()
    result = classify(event, payload)
    if result is None:
        return
    state, reason = result
    line = core.build_line(
        RUNTIME, payload.get("sessionId"), event, state, reason,
        cwd=payload.get("cwd"), log_path=payload.get("transcriptPath"),
        t=event_time(payload), host=core.host_info(),
    )
    if line:
        core.append(line)


if __name__ == "__main__":
    try:
        main(sys.argv)
    except Exception:  # noqa: BLE001 - observational hook: never disturb the session
        pass
    sys.exit(0)
