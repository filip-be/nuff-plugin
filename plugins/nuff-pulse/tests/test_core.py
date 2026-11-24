import json
import os
import shutil
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
import nuff_pulse_core as core  # noqa: E402


class BuildLine(unittest.TestCase):
    def test_full_line(self):
        line = core.build_line("claude", "abc-123", "Stop", "done", "end_turn", "  two\n lines ",
                               cwd="D:\\w", log_path="D:\\w\\l.jsonl", t=12.34567,
                               host={"os": "windows"})
        self.assertEqual(line, {
            "v": 1, "t": 12.346, "runtime": "claude", "session_id": "abc-123", "event": "Stop",
            "state": "done", "reason": "end_turn", "detail": "two lines", "cwd": "D:\\w",
            "log_path": "D:\\w\\l.jsonl", "host": {"os": "windows"},
        })

    def test_unusable_session_id(self):
        for bad in (None, "", "../x", "a/b", "a" * 129, 5):
            with self.subTest(bad):
                self.assertIsNone(core.build_line("claude", bad, "Stop", "done"))

    def test_drops_invalid_fields(self):
        line = core.build_line("copilot", "s", "agent Stop", "sleeping", "has space", 7, cwd=3)
        self.assertEqual(set(line), {"v", "t", "runtime", "session_id"})

    def test_detail_is_truncated(self):
        line = core.build_line("claude", "s", "Stop", detail="x" * 500)
        self.assertEqual(len(line["detail"]), core.DETAIL_CHARS)
        self.assertTrue(line["detail"].endswith("\u2026"))


class Encode(unittest.TestCase):
    def test_one_line_utf8(self):
        data = core.encode({"v": 1, "detail": "zażółć\u2028"})
        self.assertTrue(data.endswith(b"\n"))
        self.assertEqual(data.count(b"\n"), 1)
        self.assertEqual(json.loads(data)["detail"], "zażółć\u2028")

    def test_drops_fields_until_it_fits(self):
        big = "\\" * 3000
        line = {"v": 1, "t": 1, "runtime": "claude", "session_id": "s", "detail": "d",
                "cwd": big, "log_path": big, "host": {"os": "windows"}}
        data = core.encode(line)
        self.assertLessEqual(len(data), core.MAX_LINE)
        self.assertEqual(set(json.loads(data)), {"v", "t", "runtime", "session_id", "host"})


class Append(unittest.TestCase):
    def setUp(self):
        self.folder = Path(tempfile.mkdtemp(prefix="nuff-pulse-"))
        self.addCleanup(shutil.rmtree, self.folder)

    def test_appends_to_the_session_file(self):
        for state in ("working", "done"):
            path = core.append(core.build_line("claude", "s1", "Stop", state), self.folder / "sessions")
        self.assertEqual(path.name, "claude-s1.hooks.jsonl")
        states = [json.loads(l)["state"] for l in path.read_text(encoding="utf-8").splitlines()]
        self.assertEqual(states, ["working", "done"])

    def test_default_folder_is_in_home(self):
        with mock.patch.object(Path, "home", return_value=self.folder):
            path = core.append(core.build_line("copilot", "s2", "agentStop", "done"))
        self.assertEqual(path, self.folder / ".nuff-pulse" / "sessions" / "copilot-s2.hooks.jsonl")

    def test_concurrent_writers_never_mix_lines(self):
        detail = "y" * 180

        def write(n):
            for i in range(25):
                core.append(core.build_line("claude", "s3", "Stop", "done", detail=f"{n}-{i}-{detail}"),
                            self.folder)

        threads = [threading.Thread(target=write, args=(n,)) for n in range(8)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        lines = (self.folder / "claude-s3.hooks.jsonl").read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 200)
        self.assertEqual(len({json.loads(l)["detail"] for l in lines}), 200)

    def test_drops_the_line_when_the_lock_times_out(self):
        with mock.patch.object(core, "_lock", return_value=False), \
                mock.patch.object(core, "_unlock") as unlock:
            self.assertIsNone(core.append(core.build_line("claude", "s4", "Stop", "done"), self.folder))
        unlock.assert_not_called()
        self.assertEqual((self.folder / "claude-s4.hooks.jsonl").read_bytes(), b"")


class Host(unittest.TestCase):
    def test_agent_chain_skips_launchers(self):
        chain = [(1, "python.exe", 0), (2, "py.exe", 0), (3, "bash.exe", 0), (4, "claude.exe", 0),
                 (5, "Code.exe", 0)]
        self.assertEqual([p for p, _, _ in core.agent_chain(chain)], [4, 5])
        linux = [(1, "python3.12", 0), (2, "sh", 0), (3, "node", 0)]
        self.assertEqual([p for p, _, _ in core.agent_chain(linux)], [3])
        self.assertEqual(core.agent_chain([(1, "pwsh.exe", 0)]), [])

    def test_host_info_reports_the_agent(self):
        chain = [(10, "python.exe", 1.0), (11, "pwsh.exe", 1.0), (12, "copilot.exe", 0.5),
                 (13, "WindowsTerminal.exe", 0.25)]
        reader = "windows_chain" if os.name == "nt" else "linux_chain"
        with mock.patch.object(core, reader, return_value=chain):
            host = core.host_info()
        if sys.platform == "darwin":
            self.skipTest("no process facts on macOS")
        self.assertEqual(host["pids"], [12, 13])
        self.assertEqual(host["agent_pid"], 12)
        self.assertEqual(host["agent_started"], 0.5)

    def test_host_info_survives_errors(self):
        reader = "windows_chain" if os.name == "nt" else "linux_chain"
        with mock.patch.object(core, reader, side_effect=OSError):
            host = core.host_info()
        self.assertNotIn("pids", host)
        self.assertIn("os", host)

    def test_wsl_distro(self):
        if not sys.platform.startswith("linux"):
            self.skipTest("Linux only")
        with mock.patch.dict(os.environ, {"WSL_DISTRO_NAME": "Ubuntu"}), \
                mock.patch.object(core, "linux_chain", return_value=[]):
            self.assertEqual(core.host_info(), {"os": "wsl", "distro": "Ubuntu"})

    @unittest.skipUnless(os.name == "nt", "Windows only")
    def test_windows_chain_is_real(self):
        chain = core.windows_chain()
        self.assertEqual(chain[0][0], os.getpid())
        starts = [started for _, _, started in chain]
        self.assertEqual(starts, sorted(starts, reverse=True))

    @unittest.skipUnless(sys.platform.startswith("linux"), "Linux only")
    def test_linux_chain_is_real(self):
        chain = core.linux_chain()
        self.assertEqual(chain[0][0], os.getpid())
        self.assertEqual(chain[1][0], os.getppid())


if __name__ == "__main__":
    unittest.main()
