import contextlib
import io
import unittest
from unittest.mock import patch


class FifthReview(unittest.TestCase):
    def test_duplicate_validator_fails_all_three_checks(self):
        import live
        import exit_roll
        import exit_sandbox_check

        row = dict(
            chain_id=live.CHAIN.decode(),
            genesis_hash=live.PFTL_GENESIS.hex(),
            build_git_revision="test",
            block_height=1753,
            block_tip_hash="tip",
            state_root="root",
            node_id="validator-0",
        )
        for module, fn in [
            (live, live.fleet),
            (exit_roll, exit_roll.snapshot),
            (live, exit_sandbox_check.check_tunnels),
        ]:
            with patch.object(
                module, "rpc", return_value=row
            ), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(ValueError, "validator-1"):
                    fn()
