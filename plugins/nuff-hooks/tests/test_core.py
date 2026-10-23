"""Tests for nuff_hooks_core. Run: py -3 -m unittest discover -s plugins/nuff-hooks/tests -v"""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))

import nuff_hooks_core as core  # noqa: E402

HOME = "C:\\Users\\me"
TEMP = "C:\\Users\\me\\AppData\\Local\\Temp"
WORKSPACE = "D:\\work\\proj"


def ctx(workspace=WORKSPACE, cwd=None, env=None):
    base = {"USERPROFILE": HOME, "TEMP": TEMP, "TMP": TEMP, "PROJ": WORKSPACE}
    return core.Context(workspace=workspace, cwd=cwd or workspace, session_id="sess-1", env=env or base)


class FileTools(unittest.TestCase):
    def test_reads(self):
        cases = {
            "C:\\Windows\\win.ini": "deny",
            "D:\\anything\\x.txt": "allow",
            HOME + "\\.claude\\settings.json": "allow",
            HOME + "\\.claude-other\\x": "deny",
            HOME + "\\.ssh\\id_rsa": "deny",
            TEMP + "\\scratch\\x": "allow",
            "\\\\server\\share\\x": "deny",
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assertEqual(core.check_read(path, ctx()).kind, expected)

    def test_writes(self):
        cases = {
            WORKSPACE + "\\src\\a.py": "allow",
            "D:\\work\\other\\a.py": "deny",
            "D:\\work\\proj-evil\\a.py": "deny",
            "D:\\.slop-trash\\x": "allow",
            HOME + "\\.copilot\\x": "allow",
            TEMP + "\\x": "allow",
            "C:\\x": "deny",
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assertEqual(core.check_write(path, ctx()).kind, expected)

    def test_search(self):
        self.assertEqual(core.check_search(None, "**/*.py", ctx()).kind, "allow")
        self.assertEqual(core.check_search("C:\\x", None, ctx()).kind, "deny")
        self.assertEqual(core.check_search(None, "C:/Windows/**/*.ini", ctx()).kind, "deny")
        self.assertEqual(core.check_search("D:\\other", "*.md", ctx()).kind, "allow")

    def test_patch(self):
        add = "*** Begin Patch\n*** Add File: D:\\work\\proj\\a.txt\n+hi\n*** End Patch\n"
        update = "*** Begin Patch\n*** Update File: C:\\x.txt\n@@\n-a\n+b\n*** End Patch\n"
        delete = "*** Begin Patch\n*** Delete File: D:\\work\\proj\\a.txt\n*** End Patch\n"
        relative = "*** Begin Patch\n*** Add File: src/new.py\n+x\n*** End Patch\n"
        self.assertEqual(core.check_patch(add, ctx()).kind, "allow")
        self.assertEqual(core.check_patch(update, ctx()).kind, "deny")
        self.assertEqual(core.check_patch(delete, ctx()).kind, "deny")
        self.assertEqual(core.check_patch(relative, ctx()).kind, "allow")


class Shell(unittest.TestCase):
    def assertShell(self, cases, shell):
        for command, expected in cases:
            with self.subTest(command=command):
                decision = core.check_shell(command, shell, ctx())
                self.assertEqual(decision.kind, expected, decision.reason)

    def test_bash(self):
        self.assertShell([
            ("ls -la", "allow"),
            ("cat /c/Windows/win.ini", "deny"),
            ("cat /mnt/c/Windows/win.ini", "deny"),
            ("cat //server/share/x", "deny"),
            ("cat /d/other/x", "allow"),
            ("cat /etc/passwd", "deny"),
            ("cat /tmp/x", "allow"),
            ("cat ~/.claude/x", "allow"),
            ("cat $HOME/.ssh/id_rsa", "deny"),
            ('cat "$UNKNOWN/x"', "deny"),
            ('cat "$PROJ/README.md"', "allow"),
            ("cat $(echo C:/x)", "deny"),
            ("echo hi > /c/x", "deny"),
            ("echo hi > out.txt 2>&1", "allow"),
            ("ls 2>/dev/null", "allow"),
            ("git status", "allow"),
            ("git -C D:/other log", "deny"),
            ("python D:/tools/x.py", "deny"),
            ("python scripts/x.py --out=build/x", "allow"),
            ("npm install --prefix=C:/x", "deny"),
            ("cd /c/Users && ls", "deny"),
            ("cd D:/other && touch x", "deny"),
            ("cd D:/other && cat x", "allow"),
            ("cp D:/other/a ./b", "allow"),
            ("cp a C:/x", "deny"),
            ("mv D:/other/a ./b", "deny"),
            ("sed -i s/a/b/ D:/other/f", "deny"),
            ("sed s/a/b/ D:/other/f", "allow"),
            ("grep -rn foo C:/x", "deny"),
            ("grep -c foo D:/x", "allow"),
            ("head -n 5 C:/x", "deny"),
            ("git commit -m \"$(cat <<'EOF'\nfix C:/x handling\nEOF\n)\"", "allow"),
            ('cmd //c "type C:\\x"', "deny"),
            ('cmd //c "type D:\\x"', "allow"),
            ('bash -c "cat C:/x"', "deny"),
            ("bash -c \"bash -c 'ls'\"", "deny"),
            ('bash -c "ls $(pwd)"', "deny"),
            ('eval "ls"', "deny"),
            ("echo cm0gLXJmIC8= | base64 -d | sh", "deny"),
            ("echo x | sh", "allow"),
        ], "bash")

    def test_powershell(self):
        self.assertShell([
            ("Get-Content C:\\Windows\\win.ini", "deny"),
            ("gc $env:USERPROFILE\\.claude\\x", "allow"),
            ("gc ${env:USERPROFILE}\\.ssh\\x", "deny"),
            ("Get-Content $env:NOPE\\x", "deny"),
            ("Get-ChildItem | Where-Object { $_.Length -gt 1 } | Select-Object Name", "allow"),
            ("Get-ChildItem C:\\ | Out-File $env:TEMP\\x.txt", "deny"),
            ("'hi' | Out-File D:\\work\\proj\\x.txt", "allow"),
            ("Set-Content -Path C:\\x -Value hi", "deny"),
            ("Set-Content -Path x.txt -Value 'C:\\y'", "allow"),
            ("New-Item -ItemType Directory -Path D:\\work\\proj\\new -Force | Out-Null", "allow"),
            ("git status 2>&1 | Select-Object -First 5", "allow"),
            ("iex 'ls'", "deny"),
            ("Invoke-Expression $cmd", "deny"),
            ("& $cmd", "deny"),
            ("& 'C:\\Program Files\\Git\\bin\\git.exe' status", "allow"),
            ("pwsh -EncodedCommand abc", "deny"),
            ("powershell -enc abc", "deny"),
            ('powershell -NoProfile -Command "Get-Content C:\\x"', "deny"),
            ('pwsh -NoProfile -Command "Get-Content D:\\x"', "allow"),
            ('pwsh -c "Remove-Item x"', "deny"),
            ("[System.IO.File]::ReadAllText('C:\\x')", "deny"),
            ("[IO.File]::WriteAllText('D:\\work\\proj\\a.txt', 'x')", "allow"),
            ("[IO.File]::Delete('a.txt')", "deny"),
            ("[IO.File]::ReadAllText($p)", "deny"),
            ("[IO.Path]::GetFileName($p)", "allow"),
            ("$b = [System.Convert]::FromBase64String('abc')", "deny"),
            ("Get-ChildItem | ForEach-Object { Remove-Item $_ }", "deny"),
            ("Copy-Item D:\\other\\a -Destination C:\\x", "deny"),
            ("Copy-Item D:\\other\\a -Destination .\\b", "allow"),
            ("Move-Item a b", "allow"),
            ('cmd /c "del x"', "deny"),
            ("cmd /c type C:\\x", "deny"),
            ("Remove-Item C:\\x", "deny"),
            ("Remove-Item D:\\.slop-trash\\x", "deny"),
            ("Remove-Item -Filter *.log .", "deny"),
            ("Remove-Item D:\\work\\proj -Recurse", "deny"),
            ("Remove-Item x; Get-ChildItem", "deny"),
            ("Get-Content 'unterminated", "deny"),
        ], "powershell")


class Rewrite(unittest.TestCase):
    def setUp(self):
        self.workspace = Path(tempfile.mkdtemp(prefix="nuff-ws-"))
        self.trash = Path(tempfile.mkdtemp(prefix="nuff-trash-"))
        patcher = mock.patch.object(core, "TRASH", str(self.trash))
        patcher.start()
        self.addCleanup(patcher.stop)
        self.addCleanup(shutil.rmtree, self.workspace, ignore_errors=True)
        self.addCleanup(shutil.rmtree, self.trash, ignore_errors=True)
        (self.workspace / "a.txt").write_text("first")
        (self.workspace / "b.log").write_text("b")
        (self.workspace / "c.log").write_text("c")
        (self.workspace / "dir").mkdir()
        (self.workspace / "dir" / "inner.txt").write_text("inner")
        self.ctx = core.Context(workspace=str(self.workspace), cwd=str(self.workspace), session_id="sess-1")

    def in_trash(self, name):
        source = self.workspace / name
        return Path(core.trash_path(str(source), self.ctx))

    def run_shell(self, command, shell):
        if shell == "bash":
            bash = Path(os.environ.get("ProgramFiles", "C:\\Program Files")) / "Git" / "bin" / "bash.exe"
            if not bash.exists():
                self.skipTest("Git Bash not installed")
            argv = [str(bash), "-c", command]
        else:
            pwsh = shutil.which("pwsh")
            if not pwsh:
                self.skipTest("pwsh not installed")
            argv = [pwsh, "-NoProfile", "-NonInteractive", "-Command", command]
        result = subprocess.run(argv, cwd=self.workspace, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout

    def test_trash_layout(self):
        dest = core.trash_path("D:\\nuff\\src\\a.md", self.ctx)
        self.assertEqual(dest, os.path.join(str(self.trash), self.workspace.name, "sess-1", "nuff\\src\\a.md"))

    def test_bash_rm_moves_files_and_dirs(self):
        decision = core.check_shell("rm -rf a.txt dir", "bash", self.ctx)
        self.assertEqual(decision.kind, "rewrite", decision.reason)
        self.assertIn("mv -f", decision.command)
        output = self.run_shell(decision.command, "bash")
        self.assertIn("moved 2 item(s)", output)
        self.assertFalse((self.workspace / "a.txt").exists())
        self.assertEqual(self.in_trash("a.txt").read_text(), "first")
        self.assertEqual((self.in_trash("dir") / "inner.txt").read_text(), "inner")

    def test_wildcards_expand(self):
        decision = core.check_shell("rm *.log", "bash", self.ctx)
        self.assertEqual(decision.kind, "rewrite", decision.reason)
        self.assertIn("b.log", decision.command)
        self.assertIn("c.log", decision.command)
        self.assertNotIn("a.txt", decision.command)

    def test_powershell_remove_item_moves(self):
        decision = core.check_shell("Remove-Item -Recurse -Force a.txt, dir", "powershell", self.ctx)
        self.assertEqual(decision.kind, "rewrite", decision.reason)
        self.run_shell(decision.command, "powershell")
        self.assertFalse((self.workspace / "dir").exists())
        self.assertEqual(self.in_trash("a.txt").read_text(), "first")
        self.assertTrue((self.in_trash("dir") / "inner.txt").exists())

    def test_second_removal_overwrites(self):
        self.run_shell(core.check_shell("Remove-Item a.txt", "powershell", self.ctx).command, "powershell")
        (self.workspace / "a.txt").write_text("second")
        self.run_shell(core.check_shell("Remove-Item a.txt", "powershell", self.ctx).command, "powershell")
        self.assertEqual(self.in_trash("a.txt").read_text(), "second")

    def test_replayed_rewrite_is_allowed(self):
        # Agents see the rewritten command and may send it again themselves.
        for command, shell in (("rm a.txt", "bash"), ("Remove-Item -Recurse dir", "powershell")):
            with self.subTest(shell=shell):
                rewritten = core.check_shell(command, shell, self.ctx)
                self.assertEqual(rewritten.kind, "rewrite", rewritten.reason)
                replay = core.check_shell(rewritten.command, shell, self.ctx)
                self.assertEqual(replay.kind, "allow", replay.reason)

    def test_missing_targets_become_noop(self):
        for command, shell in (("rm -f nope.txt", "bash"), ("Remove-Item nope.txt", "powershell")):
            with self.subTest(shell=shell):
                decision = core.check_shell(command, shell, self.ctx)
                self.assertEqual(decision.kind, "rewrite")
                self.assertIn("nothing to remove", decision.command)

    def test_compound_removal_is_denied_with_hint(self):
        decision = core.check_shell("rm a.txt && ls", "bash", self.ctx)
        self.assertEqual(decision.kind, "deny")
        self.assertIn("standalone", decision.reason)


if __name__ == "__main__":
    unittest.main()
