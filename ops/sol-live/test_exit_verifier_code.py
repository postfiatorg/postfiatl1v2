"""The payout service refuses before any proof work unless the pinned verifier runs."""

import unittest
from unittest.mock import Mock, patch

import exit_payout as payout
import exit_setup


class VerifierCodeTest(unittest.TestCase):
    def test_cluster_refuses_while_the_rebuilt_verifier_is_unpinned(self):
        with (
            patch.object(exit_setup, "VERIFIER_SHA256", None),
            patch.object(payout, "ExitCluster", return_value=Mock()),
            patch.object(payout, "check_payout_authorities"),
            patch.object(payout, "profile_code", return_value="0x" + "3b" * 32),
        ):
            with self.assertRaisesRegex(ValueError, "not pinned yet"):
                payout.cluster()

    def test_cluster_refuses_a_different_deployed_image(self):
        with (
            patch.object(exit_setup, "VERIFIER_SHA256", "0x" + "aa" * 32),
            patch.object(payout, "ExitCluster", return_value=Mock()),
            patch.object(payout, "check_payout_authorities"),
            patch.object(payout, "profile_code", return_value="0x" + "3b" * 32),
        ):
            with self.assertRaisesRegex(ValueError, "verifier"):
                payout.cluster()

    def test_cluster_accepts_the_pinned_image(self):
        c = Mock()
        with (
            patch.object(exit_setup, "VERIFIER_SHA256", "0x" + "aa" * 32),
            patch.object(payout, "ExitCluster", return_value=c),
            patch.object(payout, "check_payout_authorities"),
            patch.object(payout, "profile_code", return_value="0x" + "aa" * 32),
        ):
            self.assertIs(payout.cluster(), c)

    def test_an_upgrade_after_the_job_check_is_refused_before_sending(self):
        from solders.keypair import Keypair

        payer = Keypair()
        c = Mock(level={"commitment": "finalized"}, payer=payer, owner=payer.pubkey())
        sent = []
        c.call.side_effect = lambda method, *a: sent.append(method) if method == "sendTransaction" else {
            "value": {"getLatestBlockhash": {"blockhash": str(payout.Hash.default()),
                                             "lastValidBlockHeight": 9},
                      "getBalance": payout.FLOOR + 9000, "getFeeForMessage": 5000}[method]}
        with (
            patch.object(exit_setup, "VERIFIER_SHA256", "0x" + "aa" * 32),
            patch.object(payout, "check_custody"),
            patch.object(payout, "profile_code", return_value="0x" + "bb" * 32),
        ):
            for label in ("exit.buffer", "exit.upload", "exit.checkpoint", "exit.withdrawal"):
                with self.assertRaisesRegex(ValueError, "verifier"):
                    payout.funded_send(c, label, [], before_send=Mock())
        self.assertEqual(sent, [])


if __name__ == "__main__":
    unittest.main()
