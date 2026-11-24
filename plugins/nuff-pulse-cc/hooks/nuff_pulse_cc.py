"""Claude Code hooks for nuff-pulse: append one Pulse Line per hook call.

Fails open: never prints, never blocks, always exits 0.
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import nuff_pulse_core as core  # noqa: E402

RUNTIME = "claude"
QUESTION_TOOL = "AskUserQuestion"
NOTIFICATIONS = {"permission_prompt": "permission_prompt", "elicitation_dialog": "elicitation"}
TOOL_ARGS = ("command", "file_path", "notebook_path", "path", "pattern", "url", "query", "description")


def tool_summary(tool, tool_input):
    """`Bash: dotnet ef ...` — the tool name and its main argument."""
    if not isinstance(tool, str) or not tool:
        return None
    if isinstance(tool_input, dict):
        for key in TOOL_ARGS:
            value = core.short(tool_input.get(key), 150)
            if value:
                return f"{tool}: {value}"
    return tool


def first_question(tool_input):
    questions = tool_input.get("questions") if isinstance(tool_input, dict) else None
    if isinstance(questions, list) and questions and isinstance(questions[0], dict):
        return questions[0].get("question")
    return None


def classify(event, payload):
    """(state, reason, detail) for a hook event; None means write no line."""
    tool, tool_input = payload.get("tool_name"), payload.get("tool_input")
    if event == "SessionStart":
        return None, payload.get("source"), None
    if event == "UserPromptSubmit":
        return "working", None, None
    if event == "PreToolUse" and tool == QUESTION_TOOL:
        return "needs_you", "question", first_question(tool_input)
    if event == "PostToolUse" and tool == QUESTION_TOOL:
        return "working", None, None
    if event == "PermissionRequest":
        summary = tool_summary(tool, tool_input)
        return "needs_you", "permission_prompt", f"Allow {summary}" if summary else None
    if event == "Notification":
        reason = NOTIFICATIONS.get(payload.get("notification_type"))
        return ("needs_you", reason, payload.get("message")) if reason else None
    if event == "Stop":
        return "done", None, None
    if event == "StopFailure":
        return "error", payload.get("error"), payload.get("error_details")
    if event == "SessionEnd":
        return "ended", payload.get("reason"), None
    return None


def main():
    started = time.time()  # before the ancestry walk, so lines keep the hooks' order
    payload = core.read_stdin()
    event = payload.get("hook_event_name")
    result = classify(event, payload)
    if result is None:
        return
    state, reason, detail = result
    line = core.build_line(
        RUNTIME, payload.get("session_id"), event, state, reason, detail,
        cwd=payload.get("cwd"), log_path=payload.get("transcript_path"),
        t=started, host=core.host_info(),
    )
    if line:
        core.append(line)


if __name__ == "__main__":
    try:
        main()
    except Exception:  # noqa: BLE001 - observational hook: never disturb the session
        pass
    sys.exit(0)
