"""The hook commands in hooks.json: pick `py`, else `python3`, else deny (exit 2)."""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

PLUGINS = Path(__file__).resolve().parents[2]
CC = PLUGINS / "nuff-hooks-cc"
COPILOT = PLUGINS / "nuff-hooks-copilot"
WORKSPACE = str(PLUGINS.parent)

CC_EVENTS = {
    "allow": {"tool_name": "Bash", "tool_input": {"command": "ls"}, "cwd": WORKSPACE, "session_id": "s"},
    "deny": {"tool_name": "Write", "tool_input": {"file_path": "/etc/nuff-x"}, "cwd": WORKSPACE, "session_id": "s"},
}
COPILOT_EVENTS = {
    "allow": {"toolName": "bash", "toolArgs": {"command": "ls"}, "cwd": WORKSPACE, "sessionId": "s"},
    "deny": {"toolName": "create", "toolArgs": {"path": "/etc/nuff-x"}, "cwd": WORKSPACE, "sessionId": "s"},
}


def commands():
    """(name, shell, command, plugin root variable, plugin root, events)."""
    cc = json.loads((CC / "hooks" / "hooks.json").read_text())["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
    copilot = json.loads((COPILOT / "hooks.json").read_text())["hooks"]["preToolUse"][0]
    return [
        ("cc", "sh", cc, "CLAUDE_PLUGIN_ROOT", CC, CC_EVENTS),
        ("copilot-bash", "sh", copilot["bash"], "PLUGIN_ROOT", COPILOT, COPILOT_EVENTS),
        ("copilot-powershell", "pwsh", copilot["powershell"], "PLUGIN_ROOT", COPILOT, COPILOT_EVENTS),
    ]


@unittest.skipIf(os.name == "nt", "the deny cases use POSIX paths")
class Launchers(unittest.TestCase):
    def setUp(self):
        root = Path(tempfile.mkdtemp(prefix="nuff-bin-"))
        self.addCleanup(shutil.rmtree, root)
        self.empty, self.python, self.both = root / "empty", root / "python", root / "both"
        for folder in (self.empty, self.python, self.both):
            folder.mkdir()
        for folder in (self.python, self.both):
            (folder / "python3").symlink_to(sys.executable)
        fake = self.both / "py"
        fake.write_text('#!/bin/sh\necho "py-ran $*"\n')
        fake.chmod(0o755)

    def run_hook(self, shell, command, variable, root, event, path):
        exe = shutil.which(shell)
        if not exe:
            self.skipTest(f"{shell} not installed")
        command = command.replace("${" + variable + "}", str(root))  # as the agent substitutes it
        argv = [exe, "-c", command] if shell == "sh" else [exe, "-NoProfile", "-NonInteractive", "-Command", command]
        env = {**os.environ, "PATH": str(path), variable: str(root), "HOME": str(self.empty)}
        return subprocess.run(argv, input=json.dumps(event), capture_output=True, text=True, env=env, timeout=60)

    def test_python3_runs_the_hook(self):
        for name, shell, command, variable, root, events in commands():
            with self.subTest(name):
                allowed = self.run_hook(shell, command, variable, root, events["allow"], self.python)
                self.assertEqual((allowed.returncode, allowed.stdout.strip()), (0, ""), allowed.stderr)
                denied = self.run_hook(shell, command, variable, root, events["deny"], self.python)
                self.assertEqual(denied.returncode, 0, denied.stderr)
                self.assertIn('"deny"', denied.stdout)

    def test_py_is_preferred(self):
        for name, shell, command, variable, root, events in commands():
            with self.subTest(name):
                result = self.run_hook(shell, command, variable, root, events["allow"], self.both)
                self.assertIn("py-ran -3 -I", result.stdout, result.stderr)

    def test_no_python_denies(self):
        for name, shell, command, variable, root, events in commands():
            with self.subTest(name):
                result = self.run_hook(shell, command, variable, root, events["allow"], self.empty)
                self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                self.assertIn("Python 3 not found", result.stderr)


if __name__ == "__main__":
    unittest.main()
