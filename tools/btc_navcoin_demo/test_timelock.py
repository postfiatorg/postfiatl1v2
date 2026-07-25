from __future__ import annotations

import unittest

from .timelock import Direction, TimingError, validate_second_lock


class TimingTests(unittest.TestCase):
    def test_btc_first_longer(self) -> None:
        result = validate_second_lock(
            direction=Direction.BTC_TO_NAV,
            bitcoin_height=100,
            bitcoin_cancel_height=112,
            pftl_height=10,
            pftl_cancel_height=30,
        )
        self.assertEqual(result["first_ledger"], "bitcoin")

    def test_nav_first_longer(self) -> None:
        result = validate_second_lock(
            direction=Direction.NAV_TO_BTC,
            bitcoin_height=100,
            bitcoin_cancel_height=108,
            pftl_height=10,
            pftl_cancel_height=2010,
        )
        self.assertEqual(result["first_ledger"], "pftl")

    def test_inverted_plan_rejected(self) -> None:
        with self.assertRaises(TimingError):
            validate_second_lock(
                direction=Direction.NAV_TO_BTC,
                bitcoin_height=100,
                bitcoin_cancel_height=108,
                pftl_height=10,
                pftl_cancel_height=90,
            )


if __name__ == "__main__":
    unittest.main()
