import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from subprocess import CompletedProcess


class RollTest(unittest.TestCase):
    def test_idle_chain_still_needs_margin_for_all_remaining_restarts(self):
        import exit_roll as r
        self.assertGreater(r.margin(0, 600), 7)
        self.assertGreater(r.margin(1, 600), r.margin(0, 600))
        h=1753+r.margin(0,600)+1
        r.require_margin(h,1753,0,600)
        with self.assertRaises(ValueError):
            r.require_margin(h,1755,0,600)

    def test_inherited_success_cannot_hide_watchdog_failure(self):
        import exit_roll as r
        with tempfile.TemporaryDirectory() as d:
            kit=Path(d)
            (kit/"evidence").mkdir()
            with patch.object(r.subprocess,"run",return_value=CompletedProcess([],0,"watchdog restore returned nonzero v0\nALL-DONE\n")), self.assertRaisesRegex(ValueError,"watchdog"):
                r.run_one(kit,0)
