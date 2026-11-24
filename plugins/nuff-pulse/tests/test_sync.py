"""The shipped copies of the core match the source, and the plugin metadata agrees."""

import json
import unittest
from pathlib import Path

PLUGINS = Path(__file__).resolve().parents[2]
SOURCE = PLUGINS / "nuff-pulse" / "core" / "nuff_pulse_core.py"
NAMES = ("nuff-pulse-cc", "nuff-pulse-copilot")


class Sync(unittest.TestCase):
    def test_core_copies_match(self):
        source = SOURCE.read_bytes()
        for name in NAMES:
            with self.subTest(name):
                self.assertEqual((PLUGINS / name / "hooks" / "nuff_pulse_core.py").read_bytes(), source,
                                 "copy plugins/nuff-pulse/core/nuff_pulse_core.py into the plugin")

    def test_plugin_definitions_match(self):
        marketplace = json.loads((PLUGINS.parent / ".claude-plugin" / "marketplace.json").read_text())
        listed = {p["name"]: p for p in marketplace["plugins"]}
        for name in NAMES:
            with self.subTest(name):
                claude = json.loads((PLUGINS / name / ".claude-plugin" / "plugin.json").read_text())
                copilot = json.loads((PLUGINS / name / "plugin.json").read_text())
                self.assertEqual(claude, copilot)
                self.assertEqual(claude["name"], name)
                self.assertEqual(listed[name]["description"], claude["description"])
                self.assertEqual(listed[name]["source"], f"./plugins/{name}")


if __name__ == "__main__":
    unittest.main()
