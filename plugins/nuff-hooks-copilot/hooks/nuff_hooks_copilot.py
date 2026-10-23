"""GitHub Copilot CLI preToolUse adapter for nuff-hooks.

Reads the hook event from stdin and answers with a top-level permissionDecision.
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

SHELLS = {"bash": "bash", "powershell": "powershell"}
SHELL_INPUTS = {"write_bash": "bash", "write_powershell": "powershell"}  # text sent to a running shell


def _args(event: dict):
    args = event.get("toolArgs")
    if isinstance(args, str) and event.get("toolName") != "apply_patch":
        try:
            return json.loads(args)
        except ValueError:
            return {}
    return args if args is not None else {}


def decide(event: dict, args) -> core.Decision:
    tool = event.get("toolName", "")
    cwd = event.get("cwd") or os.getcwd()
    ctx = core.Context(
        workspace=os.environ.get("COPILOT_PROJECT_DIR") or cwd,
        cwd=cwd,
        session_id=event.get("sessionId", ""),
    )
    if tool == "apply_patch":
        patch = args if isinstance(args, str) else next((v for v in args.values() if isinstance(v, str)), "")
        return core.check_patch(patch, ctx)
    if tool == "view":
        return core.check_read(args.get("path", ""), ctx)
    if tool in ("create", "edit", "str_replace_editor"):
        return core.check_write(args.get("path", ""), ctx)
    if tool == "glob":
        return core.check_search(args.get("path"), args.get("pattern"), ctx)
    if tool in ("grep", "rg"):
        return core.check_search(args.get("path"), None, ctx)
    if tool in SHELLS:
        return core.check_shell(args.get("command", ""), SHELLS[tool], ctx)
    if tool in SHELL_INPUTS:
        text = args.get("input") or args.get("text") or ""
        return core.check_shell(text, SHELL_INPUTS[tool], ctx)
    return core.ALLOW


def main() -> int:
    try:
        if core is None:
            raise IMPORT_ERROR
        event = json.loads(sys.stdin.buffer.read().decode("utf-8"))
        args = _args(event)
        decision = decide(event, args)
        if decision.kind == "allow":
            return 0
        output = {"permissionDecisionReason": decision.reason}
        if decision.kind == "deny":
            output["permissionDecision"] = "deny"
        else:
            output["permissionDecision"] = "allow"
            output["modifiedArgs"] = {**args, "command": decision.command}
        sys.stdout.write(json.dumps(output))
        return 0
    except Exception as error:  # fail closed
        sys.stderr.write(f"nuff-hooks: internal error, call denied: {error!r}\n")
        return 2


if __name__ == "__main__":
    sys.exit(main())
