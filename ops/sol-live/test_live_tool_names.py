"""Live tools are never named like tests (F15): pytest collecting from the repository root would
run a `test_*.py` at import, and the on-call deposit script sends a real mainnet deposit."""

import unittest
from pathlib import Path

HERE = Path(__file__).parent


class LiveToolNames(unittest.TestCase):
    def test_no_live_tool_directory_holds_a_test_named_file(self):
        for directory in ("oncall", "operator"):
            with self.subTest(directory):
                self.assertEqual(sorted(p.name for p in (HERE / directory).glob("test_*.py")), [])


if __name__ == "__main__":
    unittest.main()
