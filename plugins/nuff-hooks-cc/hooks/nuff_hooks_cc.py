"""Claude Code PreToolUse adapter for nuff-hooks.

Reads the hook event from stdin and answers with a hookSpecificOutput decision.
Any internal error denies the call (exit 2): the hooks fail closed.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # `py -I` leaves the script dir off sys.path

try:
    import nuff_hooks_core as core
except Exception as error:  # fail closed even when the core can't load
    core, IMPORT_ERROR = None, error

SHELLS = {"Bash": "bash", "PowerShell": "powershell"}


def decide(event: dict) -> core.Decision:
    tool = event.get("tool_name", "")
    args = event.get("tool_input") or {}
    cwd = event.get("cwd") or os.getcwd()
    ctx = core.Context(
        workspace=os.environ.get("CLAUDE_PROJECT_DIR") or cwd,
        cwd=cwd,
        session_id=event.get("session_id", ""),
    )
    if tool == "Read":
        return core.check_read(args.get("file_path", ""), ctx)
    if tool in ("Write", "Edit"):
        return core.check_write(args.get("file_path", ""), ctx)
    if tool == "NotebookEdit":
        return core.check_write(args.get("notebook_path", ""), ctx)
    if tool == "Glob":
        return core.check_search(args.get("path"), args.get("pattern"), ctx)
    if tool == "Grep":
        return core.check_search(args.get("path"), None, ctx)
    if tool in SHELLS:
        return core.check_shell(args.get("command", ""), SHELLS[tool], ctx)
    return core.ALLOW


def main() -> int:
    try:
        if core is None:
            raise IMPORT_ERROR
        event = json.loads(sys.stdin.buffer.read().decode("utf-8"))
        decision = decide(event)
        if decision.kind == "allow":
            return 0
        output = {"hookEventName": "PreToolUse", "permissionDecisionReason": decision.reason}
        if decision.kind == "deny":
            output["permissionDecision"] = "deny"
        else:
            output["permissionDecision"] = "allow"
            output["updatedInput"] = {**event.get("tool_input", {}), "command": decision.command}
        sys.stdout.write(json.dumps({"hookSpecificOutput": output}))
        return 0
    except Exception as error:  # fail closed
        sys.stderr.write(f"nuff-hooks: internal error, call denied: {error!r}\n")
        return 2


if __name__ == "__main__":
    sys.exit(main())
