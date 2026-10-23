"""Every plugin must ship an identical copy of the shared core."""

import unittest
from pathlib import Path

PLUGINS = Path(__file__).resolve().parents[2]
SOURCE = PLUGINS / "nuff-hooks" / "core" / "nuff_hooks_core.py"
COPIES = [
    PLUGINS / "nuff-hooks-cc" / "hooks" / "nuff_hooks_core.py",
    PLUGINS / "nuff-hooks-copilot" / "hooks" / "nuff_hooks_core.py",
]


class CoreCopies(unittest.TestCase):
    def test_copies_match_source(self):
        for copy in COPIES:
            with self.subTest(copy=str(copy)):
                self.assertTrue(copy.exists(), f"missing {copy}")
                self.assertEqual(
                    copy.read_bytes(),
                    SOURCE.read_bytes(),
                    f"{copy} differs from {SOURCE}; run: cp {SOURCE} {copy}",
                )


if __name__ == "__main__":
    unittest.main()
