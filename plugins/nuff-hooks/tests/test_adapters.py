"""Tool-argument mapping of the two platform adapters."""

import importlib.util
import unittest
from pathlib import Path
from unittest import mock

PLUGINS = Path(__file__).resolve().parents[2]
WORKSPACE = "D:\\work\\proj"


def load(path: Path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


copilot = load(PLUGINS / "nuff-hooks-copilot" / "hooks" / "nuff_hooks_copilot.py")
cc = load(PLUGINS / "nuff-hooks-cc" / "hooks" / "nuff_hooks_cc.py")


class Copilot(unittest.TestCase):
    def decide(self, tool, args):
        event = {"sessionId": "s1", "cwd": WORKSPACE, "toolName": tool, "toolArgs": args}
        with mock.patch.dict("os.environ", {"COPILOT_PROJECT_DIR": WORKSPACE}):
            return copilot.decide(event, args).kind

    def test_search_roots(self):
        self.assertEqual(self.decide("rg", {"pattern": "x"}), "allow")
        self.assertEqual(self.decide("rg", {"pattern": "x", "path": "C:\\Windows"}), "deny")
        self.assertEqual(self.decide("rg", {"pattern": "x", "paths": ["D:\\a", "C:\\Windows"]}), "deny")
        self.assertEqual(self.decide("glob", {"pattern": "*.md", "paths": "C:\\Windows"}), "deny")
        self.assertEqual(self.decide("glob", {"pattern": "*.md", "paths": ["D:\\a"]}), "allow")

    def test_file_tools(self):
        self.assertEqual(self.decide("view", {"path": "C:\\Windows\\win.ini"}), "deny")
        self.assertEqual(self.decide("apply_patch", "*** Begin Patch\n*** Add File: C:\\x\n+x\n*** End Patch\n"), "deny")
        self.assertEqual(self.decide("powershell", {"command": "Get-ChildItem"}), "allow")


class ClaudeCode(unittest.TestCase):
    def decide(self, tool, args):
        event = {"session_id": "s1", "cwd": WORKSPACE, "tool_name": tool, "tool_input": args}
        with mock.patch.dict("os.environ", {"CLAUDE_PROJECT_DIR": WORKSPACE}):
            return cc.decide(event).kind

    def test_tools(self):
        self.assertEqual(self.decide("Read", {"file_path": "C:\\Windows\\win.ini"}), "deny")
        self.assertEqual(self.decide("Write", {"file_path": "D:\\work\\other\\x"}), "deny")
        self.assertEqual(self.decide("NotebookEdit", {"notebook_path": WORKSPACE + "\\n.ipynb"}), "allow")
        self.assertEqual(self.decide("Grep", {"pattern": "x", "path": "C:\\"}), "deny")
        self.assertEqual(self.decide("Bash", {"command": "ls"}), "allow")


if __name__ == "__main__":
    unittest.main()
