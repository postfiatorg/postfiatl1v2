from pathlib import Path
import tempfile
import unittest

from tools.xrpl_navcoin_demo.accounting import (
    ConservationError,
    PrincipalState,
    assert_principal_conserved,
    assert_xrp_conserved_with_fees,
)
from tools.xrpl_navcoin_demo.journal import EffectJournal, IdempotencyConflict


class JournalTests(unittest.TestCase):
    def test_duplicate_returns_prior_result_without_second_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            journal = EffectJournal(Path(directory) / "journal.sqlite3")
            calls = []
            operation = lambda: calls.append("called") or {"tx": "ABC"}
            first, duplicate1 = journal.execute("finish:1", {"escrow": 1}, operation)
            second, duplicate2 = journal.execute("finish:1", {"escrow": 1}, operation)
            self.assertEqual(first, second)
            self.assertFalse(duplicate1)
            self.assertTrue(duplicate2)
            self.assertEqual(calls, ["called"])
            with self.assertRaises(IdempotencyConflict):
                journal.execute("finish:1", {"escrow": 2}, operation)
            journal.close()


class AccountingTests(unittest.TestCase):
    def test_asset_principal_includes_locked(self):
        assert_principal_conserved(
            PrincipalState(100, 200, 0), PrincipalState(90, 200, 10)
        )
        with self.assertRaises(ConservationError):
            assert_principal_conserved(
                PrincipalState(100, 200, 0), PrincipalState(90, 200, 9)
            )

    def test_xrp_fees_are_explicit(self):
        assert_xrp_conserved_with_fees(
            PrincipalState(100, 200, 10),
            PrincipalState(95, 200, 10),
            5,
        )


if __name__ == "__main__":
    unittest.main()

