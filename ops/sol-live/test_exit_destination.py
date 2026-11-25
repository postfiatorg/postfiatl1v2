"""A destination that is not a wallet is refused before the burn and named after it."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import exit_payout as payout
import exit_canary
import exit_service as svc
from bridge import SYSTEM, TOKEN
from solders.pubkey import Pubkey

WALLET = Pubkey.from_string("Ce5Yv528pKVrVq4JbQchZ8zasCAzxT3f9uCfatTZcfo8")
TOKEN_ACCOUNT = dict(owner=str(TOKEN), executable=False, data=bytes(165))


class Accounts:
    def __init__(self, account):
        self.value = account

    def account(self, key):
        return self.value


class DestinationTest(unittest.TestCase):
    def test_refusal_names_why_the_owner_is_not_a_wallet(self):
        self.assertIsNone(payout.destination_refusal(Accounts(None), WALLET))
        reason = payout.destination_refusal(Accounts(TOKEN_ACCOUNT), WALLET)
        self.assertTrue(reason.startswith("refused: destination not a wallet"), reason)
        with self.assertRaises(payout.DestinationNotWallet):
            payout.check_owner(Accounts(TOKEN_ACCOUNT), WALLET)

    def test_readiness_checks_the_destination_before_the_burn(self):
        base = dict(ok=True, ready=True)
        path = "/solana/withdrawals/readiness?account=pf" + "0b" * 20 + "&destination=" + str(WALLET)
        with (
            patch.object(svc, "service_readiness", return_value=base),
            patch.object(svc, "holdings", return_value=[]),
            patch.object(svc, "destination_cluster", return_value=Accounts(TOKEN_ACCOUNT)),
        ):
            value = svc.account_readiness(None, path)
        self.assertFalse(value["ready"])
        self.assertTrue(value["destination_refusal"].startswith("refused: destination not a wallet"))
        with (
            patch.object(svc, "service_readiness", return_value=base),
            patch.object(svc, "holdings", return_value=[]),
            patch.object(svc, "destination_cluster", return_value=Accounts(None)),
        ):
            value = svc.account_readiness(None, path)
        self.assertTrue(value["ready"])
        self.assertIsNone(value["destination_refusal"])

    def test_canary_checks_its_destination_before_building_the_burn(self):
        exit_canary.require_wallet_destination(Accounts(dict(owner=str(SYSTEM), executable=False)))
        with self.assertRaises(payout.DestinationNotWallet):
            exit_canary.require_wallet_destination(Accounts(TOKEN_ACCOUNT))

    def test_service_records_a_refused_destination_by_name(self):
        burn = "5d" * 48
        with tempfile.TemporaryDirectory() as d:
            journal = svc.Journal(Path(d) / "jobs.sqlite")
            journal.enqueue(burn)
            refusal = payout.DestinationNotWallet(
                "refused: destination not a wallet (program-owned account)"
            )
            with patch.object(svc, "process", side_effect=refusal):
                svc.process_next(journal, Path(d))
            row = journal.get(burn)
        self.assertEqual(row["stage"], "failed")
        self.assertTrue(row["error"].startswith("refused: destination not a wallet"))


if __name__ == "__main__":
    unittest.main()
