"""Redemption and witness reads fall back to another validator when validator 0 is down."""

import unittest
from unittest.mock import patch

import exit_payout as payout
import exit_service as svc


class ReadFallbackTest(unittest.TestCase):
    def setUp(self):
        payout.reset_reads()

    def test_reads_use_the_next_validator_when_validator_0_is_down(self):
        asked = []

        def rpc(i, method, timeout=None, **params):
            asked.append(i)
            if i == 0:
                raise RuntimeError("validator-0 unreachable")
            return dict(redemptions=[], buckets=[])

        with patch.object(payout, "rpc", side_effect=rpc), patch.object(svc, "rpc", side_effect=rpc):
            self.assertEqual(payout.rpc_any("vault_bridge_status", asset_id=payout.ASSET)["redemptions"], [])
            self.assertEqual(svc.redemptions(), ([], {}))
        # Validator 0 failed once and is then asked last; validator 1 answered and is asked first.
        self.assertEqual(asked, [0, 1, 1])

    def test_all_validators_down_raises(self):
        with patch.object(payout, "rpc", side_effect=RuntimeError("down")):
            with self.assertRaisesRegex(RuntimeError, "no validator answered"):
                payout.rpc_any("blocks", from_height=1, limit=1)


if __name__ == "__main__":
    unittest.main()
