"""Each adapter run as the agent runs it: hook JSON on stdin, Pulse File in a temp home."""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

PLUGINS = Path(__file__).resolve().parents[2]
CC = PLUGINS / "nuff-pulse-cc" / "hooks" / "nuff_pulse_cc.py"
COPILOT = PLUGINS / "nuff-pulse-copilot" / "hooks" / "nuff_pulse_copilot.py"
SID = "6f1c2b9e-0000-4000-8000-000000000001"


class Adapter(unittest.TestCase):
    def setUp(self):
        self.home = Path(tempfile.mkdtemp(prefix="nuff-pulse-home-"))
        self.addCleanup(shutil.rmtree, self.home)

    def run_script(self, script, stdin, *args):
        env = {**os.environ, "HOME": str(self.home), "USERPROFILE": str(self.home)}
        data = stdin if isinstance(stdin, str) else json.dumps(stdin)
        result = subprocess.run([sys.executable, "-I", str(script), *args], input=data,
                                capture_output=True, text=True, env=env, timeout=30)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (0, "", ""))
        return result

    def lines(self, runtime, session=SID):
        path = self.home / ".nuff-pulse" / "sessions" / f"{runtime}-{session}.hooks.jsonl"
        if not path.exists():
            return []
        return [json.loads(l) for l in path.read_text(encoding="utf-8").splitlines()]


class Claude(Adapter):
    def hook(self, event, **extra):
        payload = {"session_id": SID, "hook_event_name": event, "cwd": "D:\\w",
                   "transcript_path": "D:\\t.jsonl", **extra}
        self.run_script(CC, payload)
        found = self.lines("claude")
        return found[-1] if found else None

    def test_states(self):
        cases = [
            ("SessionStart", {"source": "startup"}, (None, "startup", None)),
            ("UserPromptSubmit", {"prompt": "hi"}, ("working", None, None)),
            ("PreToolUse", {"tool_name": "AskUserQuestion",
                            "tool_input": {"questions": [{"question": "Which DB?"}]}},
             ("needs_you", "question", "Which DB?")),
            ("PostToolUse", {"tool_name": "AskUserQuestion"}, ("working", None, None)),
            ("PermissionRequest", {"tool_name": "Bash",
                                   "tool_input": {"command": "dotnet ef\n database update"}},
             ("needs_you", "permission_prompt", "Allow Bash: dotnet ef database update")),
            ("PermissionRequest", {"tool_name": "mcp__x__y", "tool_input": {}},
             ("needs_you", "permission_prompt", "Allow mcp__x__y")),
            ("Notification", {"notification_type": "permission_prompt", "message": "Claude needs permission"},
             ("needs_you", "permission_prompt", "Claude needs permission")),
            ("Notification", {"notification_type": "elicitation_dialog", "message": "MCP asks"},
             ("needs_you", "elicitation", "MCP asks")),
            ("Stop", {}, ("done", None, None)),
            ("StopFailure", {"error": "rate_limit", "error_details": "429"}, ("error", "rate_limit", "429")),
            ("SessionEnd", {"reason": "prompt_input_exit"}, ("ended", "prompt_input_exit", None)),
        ]
        for event, extra, expected in cases:
            with self.subTest(event, **{"extra": str(extra)}):
                line = self.hook(event, **extra)
                self.assertEqual((line.get("state"), line.get("reason"), line.get("detail")), expected)
                self.assertEqual(line["event"], event)
                self.assertEqual((line["runtime"], line["session_id"]), ("claude", SID))
                self.assertEqual((line["cwd"], line["log_path"]), ("D:\\w", "D:\\t.jsonl"))
                self.assertIn("os", line["host"])

    def test_ignored_events_write_nothing(self):
        for event, extra in [("Notification", {"notification_type": "idle_prompt"}),
                             ("PreToolUse", {"tool_name": "Bash"}),
                             ("PostToolUse", {"tool_name": "Read"}),
                             ("SubagentStop", {})]:
            with self.subTest(event):
                self.assertIsNone(self.hook(event, **extra))

    def test_bad_input_fails_open(self):
        for stdin in ("", "not json", "[1]", json.dumps({"hook_event_name": "Stop", "session_id": "../x"})):
            with self.subTest(stdin):
                self.run_script(CC, stdin)
        self.assertFalse((self.home / ".nuff-pulse").exists())

    def test_unwritable_home_fails_open(self):
        (self.home / ".nuff-pulse").write_text("a file, not a folder")
        self.run_script(CC, {"session_id": SID, "hook_event_name": "Stop"})


class Copilot(Adapter):
    def test_agent_stop(self):
        self.run_script(COPILOT, {"sessionId": SID, "timestamp": 1764000000123, "cwd": "/w",
                                  "transcriptPath": "/e.jsonl", "stopReason": "end_turn"}, "agentStop")
        line, = self.lines("copilot")
        self.assertEqual({k: line[k] for k in ("t", "runtime", "event", "state", "reason", "cwd", "log_path")},
                         {"t": 1764000000.123, "runtime": "copilot", "event": "agentStop", "state": "done",
                          "reason": "end_turn", "cwd": "/w", "log_path": "/e.jsonl"})

    def test_stop_reasons_and_end(self):
        self.run_script(COPILOT, {"sessionId": SID, "stopReason": "error"}, "agentStop")
        self.run_script(COPILOT, {"sessionId": SID, "stopReason": "abort"}, "agentStop")
        self.run_script(COPILOT, {"sessionId": SID, "reason": "user_exit"}, "sessionEnd")
        self.assertEqual([(l["state"], l["reason"]) for l in self.lines("copilot")],
                         [("error", "error"), ("done", "abort"), ("ended", "user_exit")])

    def test_missing_timestamp_uses_now(self):
        self.run_script(COPILOT, {"sessionId": SID, "timestamp": "soon"}, "sessionEnd")
        line, = self.lines("copilot")
        self.assertGreater(line["t"], 1.7e9)

    def test_unknown_or_missing_event_writes_nothing(self):
        for args in ([], ["preToolUse"], ["agentStop", "extra"]):
            with self.subTest(args):
                self.run_script(COPILOT, {"sessionId": SID}, *args)
        self.assertEqual(self.lines("copilot"), [])


if __name__ == "__main__":
    unittest.main()
