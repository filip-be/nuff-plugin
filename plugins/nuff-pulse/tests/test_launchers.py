"""The hook commands in hooks.json, run in the shell each agent uses for them."""

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

PLUGINS = Path(__file__).resolve().parents[2]
CC = PLUGINS / "nuff-pulse-cc"
COPILOT = PLUGINS / "nuff-pulse-copilot"
SID = "6f1c2b9e-0000-4000-8000-000000000002"
CC_EVENTS = {"SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "PermissionRequest",
             "Notification", "Stop", "StopFailure", "SessionEnd"}


def cc_hooks():
    return json.loads((CC / "hooks" / "hooks.json").read_text())["hooks"]


def copilot_hooks():
    return json.loads((COPILOT / "hooks.json").read_text())["hooks"]


class Definitions(unittest.TestCase):
    def test_claude_hooks(self):
        hooks = cc_hooks()
        self.assertEqual(set(hooks), CC_EVENTS)
        commands = set()
        for event, entries in hooks.items():
            with self.subTest(event):
                handler, = [h for entry in entries for h in entry["hooks"]]
                commands.add(handler["command"])
                self.assertEqual(handler.get("async", False), event != "SessionEnd")
        self.assertEqual(len(commands), 1)
        self.assertEqual(hooks["PreToolUse"][0]["matcher"], "AskUserQuestion")
        self.assertEqual(hooks["PostToolUse"][0]["matcher"], "AskUserQuestion")
        self.assertEqual(hooks["Notification"][0]["matcher"], "permission_prompt|elicitation_dialog")

    def test_copilot_hooks(self):
        hooks = copilot_hooks()
        self.assertEqual(set(hooks), {"agentStop", "sessionEnd"})
        for event, (handler,) in hooks.items():
            with self.subTest(event):
                self.assertEqual(handler["timeoutSec"], 5)
                self.assertNotIn("exec", handler)
                for shell in ("bash", "powershell"):
                    self.assertTrue(handler[shell].rstrip().endswith(f"{event} *> $null }} }} catch {{}}; exit 0")
                                    if shell == "powershell" else handler[shell].endswith("exit 0"))
                    self.assertIn(f"nuff_pulse_copilot.py\" {event}", handler[shell])


class Launchers(unittest.TestCase):
    def setUp(self):
        self.home = Path(tempfile.mkdtemp(prefix="nuff-pulse-home-"))
        self.addCleanup(shutil.rmtree, self.home)

    def run_command(self, shell, command, variable, root, payload, path=None):
        exe = shutil.which(shell)
        if not exe:
            self.skipTest(f"{shell} not installed")
        command = command.replace("${" + variable + "}", root.as_posix())  # as the agent substitutes it
        argv = [exe, "-c", command] if shell == "sh" else [exe, "-NoProfile", "-NonInteractive", "-Command", command]
        env = {**os.environ, variable: str(root), "HOME": str(self.home), "USERPROFILE": str(self.home)}
        if path is not None:
            env["PATH"] = path
        return subprocess.run(argv, input=json.dumps(payload), capture_output=True, text=True, env=env,
                              timeout=60)

    def pulse(self, runtime):
        path = self.home / ".nuff-pulse" / "sessions" / f"{runtime}-{SID}.hooks.jsonl"
        return [json.loads(l) for l in path.read_text(encoding="utf-8").splitlines()] if path.exists() else []

    def cases(self):
        stop = copilot_hooks()["agentStop"][0]
        return [
            ("cc", "sh", cc_hooks()["Stop"][0]["hooks"][0]["command"], "CLAUDE_PLUGIN_ROOT", CC,
             {"session_id": SID, "hook_event_name": "Stop"}, "claude"),
            ("copilot-bash", "sh", stop["bash"], "PLUGIN_ROOT", COPILOT,
             {"sessionId": SID, "stopReason": "end_turn"}, "copilot"),
            ("copilot-powershell", "pwsh", stop["powershell"], "PLUGIN_ROOT", COPILOT,
             {"sessionId": SID, "stopReason": "end_turn"}, "copilot"),
        ]

    def test_writes_a_pulse_line(self):
        for name, shell, command, variable, root, payload, runtime in self.cases():
            with self.subTest(name):
                before = len(self.pulse(runtime))
                result = self.run_command(shell, command, variable, root, payload)
                self.assertEqual((result.returncode, result.stdout), (0, ""), result.stderr)
                lines = self.pulse(runtime)
                self.assertEqual(len(lines), before + 1, result.stderr)
                self.assertEqual(lines[-1]["state"], "done")

    def test_no_python_fails_open(self):
        empty = self.home / "empty-bin"
        empty.mkdir()
        for name, shell, command, variable, root, payload, runtime in self.cases():
            with self.subTest(name):
                exe = shutil.which(shell)
                if not exe:
                    self.skipTest(f"{shell} not installed")
                # Keep the shell itself reachable on PATH, but no Python launcher.
                path = str(empty) if shell == "sh" and os.name != "nt" else os.pathsep.join(
                    [str(empty), str(Path(exe).parent)])
                if os.name == "nt" and (shutil.which("py", path=path) or shutil.which("python3", path=path)):
                    self.skipTest("Python sits next to the shell")
                result = self.run_command(shell, command, variable, root, payload, path=path)
                self.assertEqual((result.returncode, result.stdout), (0, ""), result.stderr)
        self.assertFalse((self.home / ".nuff-pulse").exists())


if __name__ == "__main__":
    unittest.main()
