"""Tool-argument mapping of the two platform adapters."""

import importlib.util
import unittest
from pathlib import Path
from unittest import mock

PLUGINS = Path(__file__).resolve().parents[2]
WORKSPACE = "D:\\work\\proj"
NO_HOME = "C:\\nuff-hooks-test-home"  # has no .nuff-hooks.json, so the user's real one can't change the results
POSIX_WORKSPACE = "/nuff-hooks-test/work/proj"
POSIX_NO_HOME = "/nuff-hooks-test/home"
LOCAL = {"CLAUDE_CODE_REMOTE": ""}  # a local session, even when the tests run in a Claude Code cloud session


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
        with mock.patch.dict("os.environ", {"COPILOT_PROJECT_DIR": WORKSPACE, "USERPROFILE": NO_HOME, **LOCAL}), \
                mock.patch.object(copilot.core, "WINDOWS", True):
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
    def decide(self, tool, args, windows=True, cloud=None):
        workspace = WORKSPACE if windows else POSIX_WORKSPACE
        event = {"session_id": "s1", "cwd": workspace, "tool_name": tool, "tool_input": args}
        env = {"CLAUDE_PROJECT_DIR": workspace, "USERPROFILE": NO_HOME, "HOME": POSIX_NO_HOME, **LOCAL, **(cloud or {})}
        with mock.patch.dict("os.environ", env), mock.patch.object(cc.core, "WINDOWS", windows):
            return cc.decide(event).kind

    def test_tools(self):
        self.assertEqual(self.decide("Read", {"file_path": "C:\\Windows\\win.ini"}), "deny")
        self.assertEqual(self.decide("Write", {"file_path": "D:\\work\\other\\x"}), "deny")
        self.assertEqual(self.decide("NotebookEdit", {"notebook_path": WORKSPACE + "\\n.ipynb"}), "allow")
        self.assertEqual(self.decide("Grep", {"pattern": "x", "path": "C:\\"}), "deny")
        self.assertEqual(self.decide("Bash", {"command": "ls"}), "allow")

    def test_posix_tools(self):
        self.assertEqual(self.decide("Read", {"file_path": "/etc/passwd"}, windows=False), "deny")
        self.assertEqual(self.decide("Write", {"file_path": POSIX_WORKSPACE + "/x"}, windows=False), "allow")
        self.assertEqual(self.decide("Bash", {"command": "cat /etc/hosts"}, windows=False), "deny")
        self.assertEqual(self.decide("Bash", {"command": "ls"}, windows=False), "allow")

    def test_cloud_session(self):
        cloud = {"CLAUDE_CODE_REMOTE": "true", "CLAUDE_ADDITIONAL_DIRECTORIES": "/mnt/user-data"}
        self.assertEqual(self.decide("Read", {"file_path": "/etc/passwd"}, windows=False, cloud=cloud), "allow")
        self.assertEqual(self.decide("Write", {"file_path": "/mnt/user-data/outputs/x"}, windows=False, cloud=cloud), "allow")
        self.assertEqual(self.decide("Write", {"file_path": "/etc/x"}, windows=False, cloud=cloud), "deny")


if __name__ == "__main__":
    unittest.main()
