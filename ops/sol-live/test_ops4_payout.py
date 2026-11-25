import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import exit_payout as p
import exit_precheck as checks
from solders.keypair import Keypair


class PayoutReviewTest(unittest.TestCase):
    def test_recipient_authorities_are_allowed_only_for_the_canonical_receiving_ata(
        self,
    ):
        owner = checks.PHANTOM
        ata = p.pda(p.ATA, bytes(owner), bytes(p.TOKEN), bytes(p.MINT))
        data = bytearray(165)
        data[:64] = bytes(p.MINT) + bytes(owner)
        data[64:72] = (20000).to_bytes(8, "little")
        data[108] = 1
        for offset in (72, 129):
            data[offset] = 1
            data[offset + 4 : offset + 36] = bytes(owner)
        account = dict(executable=False, owner=str(p.TOKEN), data=data)
        self.assertEqual(checks.recipient_atoms(ata, account, owner), 20000)
        for address, bad in [
            (p.MINT, account),
            (ata, dict(account, owner=str(p.SYSTEM))),
        ]:
            with self.assertRaises(ValueError):
                checks.recipient_atoms(address, bad, owner)
        for offset in (0, 32, 108):
            changed = bytearray(data)
            changed[offset] ^= 3
            with self.assertRaises(ValueError):
                checks.recipient_atoms(ata, dict(account, data=changed), owner)
        with self.assertRaises(ValueError):
            checks.token_atoms(account, owner)

    def test_shortfall_precedes_checkpoint_and_withdrawal_preparation(self):
        for kind in ("withdrawal", "checkpoint"):
            with (
                patch.object(p, "check_custody", side_effect=ValueError("shortfall")),
                patch.object(p, "reclaim_completed") as reclaim,
                patch.object(p, "prove") as prove,
                patch.object(p, "funded_send") as send,
            ):
                with self.assertRaisesRegex(ValueError, "shortfall"):
                    p.submit(Mock(), kind, {}, Path("unused"))
                for fn in (reclaim, prove, send):
                    fn.assert_not_called()

    def test_retry_pays_for_only_missing_buffer_chunks(self):
        values = b"x" * 1486
        signer = Keypair()
        c = Mock()
        with (
            tempfile.TemporaryDirectory() as tmp,
            patch.object(p, "load", return_value=signer),
            patch.object(p, "funded_send") as send,
        ):
            work = Path(tmp)
            (work / (hashlib.sha256(values).hexdigest() + ".key")).touch()
            for data, offsets in [
                (values, []),
                (values[:700] + bytes(700) + values[1400:], [700]),
            ]:
                c.account.return_value = dict(
                    owner=str(p.VERIFIER), data=data, executable=False
                )
                p.upload(c, values, work)
                self.assertEqual(
                    [
                        int.from_bytes(call.args[2][0].data[1:3], "little")
                        for call in send.call_args_list
                    ],
                    offsets,
                )
                send.reset_mock()
            for bad in (
                dict(owner=str(p.SYSTEM)),
                dict(data=bytes(12)),
                dict(executable=True),
            ):
                c.account.return_value = dict(
                    dict(owner=str(p.VERIFIER), data=values, executable=False), **bad
                )
                with self.assertRaises(ValueError):
                    p.upload(c, values, work)
                send.assert_not_called()

    def test_coverage_is_refreshed_if_it_changes_during_proving(self):
        c = Mock()
        with (
            patch.object(
                p, "check_custody", side_effect=[True, ValueError("shortfall")]
            ),
            patch.object(p, "reclaim_completed"),
            patch.object(p, "check_work_capacity"),
            patch.object(p, "prove", return_value=(b"val", bytes(356))),
            patch.object(p, "public_values", return_value=b"val"),
            patch.object(p, "balance"),
            patch.object(p, "upload"),
            patch.object(p, "funded_send") as send,
        ):
            # The upload now runs beside the proof. It spends only the fee payer's
            # buffer rent; the transaction that moves custody must still not be sent.
            with self.assertRaisesRegex(ValueError, "shortfall"):
                p.submit(c, "checkpoint", {}, Path("unused"))
            send.assert_not_called()
