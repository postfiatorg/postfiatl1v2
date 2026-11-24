"""The pause-key cancel tool: checks first, writes an unsigned transaction and a summary,
verifies the offline signature, sends only under the second release, never reads the key."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from solders.hash import Hash
from solders.keypair import Keypair
from solders.transaction import Transaction

import exit_cancel as xc
import exit_payout as payout
import test_exit_review as tr

PAUSE = Keypair()
BURN = tr.BURN


def red(state="pending"):
    return dict(burn_tx_id=BURN, bucket_id=payout.BUCKET, amount_atoms=tr.AMOUNT, state=state, settled_atoms=0,
                destination_ref=payout.PREFIX + bytes(tr.OWNER).hex(), withdrawal_packet_hash=tr.PACKET)


def proof_dir(d):
    p = Path(d) / "proof"
    p.mkdir()
    (p / "public-values.bin").write_bytes(tr.values())
    (p / "proof-calldata.bin").write_bytes(b"\x01" * 356)
    (p / "witness.json").write_text(json.dumps(dict(prior_checkpoint_block_id="aa" * 48,
                                                    block=dict(header=dict(block_hash="bb" * 48)))))
    return p


class CancelToolTest(unittest.TestCase):
    def prepare(self, d, marker=None, row=None, pending=()):
        with patch.object(xc, "PAUSE", PAUSE.pubkey()):
            return xc.prepare(row or red(), marker, list(pending), proof_dir(d), Path(d) / "out",
                              Hash.default(), 1000)

    def test_prepare_writes_an_unsigned_cancel_and_a_summary(self):
        with tempfile.TemporaryDirectory() as d:
            summary = self.prepare(d)
            tx = Transaction.from_bytes(bytes.fromhex((Path(d) / "out/unsigned.hex").read_text()))
            self.assertEqual(tx.message.header.num_required_signatures, 1)
            self.assertEqual(tx.message.account_keys[0], PAUSE.pubkey())
            ix = tx.message.instructions[-1]
            self.assertEqual(tx.message.account_keys[ix.program_id_index], payout.VERIFIER)
            self.assertEqual(bytes(ix.data)[:1], b"\x0a")
            self.assertEqual(summary["marker"], str(payout.marker(BURN)))
            self.assertEqual((summary["burn"], summary["amount_atoms"], summary["destination_owner"]),
                             (BURN, tr.AMOUNT, str(tr.OWNER)))
            self.assertEqual((summary["program"], summary["signer"]), (str(payout.VERIFIER), str(PAUSE.pubkey())))
            self.assertIn("summary.txt", {p.name for p in (Path(d) / "out").iterdir()})

    def test_prepare_refuses_a_paid_or_pending_or_settled_burn(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaisesRegex(ValueError, "marker"):
                self.prepare(d, marker=dict(owner=str(payout.VERIFIER), data=b"\x01"))
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaisesRegex(ValueError, "in flight"):
                self.prepare(d, pending=[dict(stage="paying")])
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaisesRegex(ValueError, "pending"):
                self.prepare(d, row=red("settled"))

    def test_verify_accepts_only_the_pause_signature_on_the_same_message(self):
        with tempfile.TemporaryDirectory() as d:
            self.prepare(d)
            out = Path(d) / "out"
            unsigned = Transaction.from_bytes(bytes.fromhex((out / "unsigned.hex").read_text()))
            signed = Transaction([PAUSE], unsigned.message, unsigned.message.recent_blockhash)
            with patch.object(xc, "PAUSE", PAUSE.pubkey()):
                xc.verify(out, bytes(signed))
                other = Keypair()
                with self.assertRaises(ValueError):
                    xc.verify(out, bytes(Transaction.new_signed_with_payer(
                        [], other.pubkey(), [other], unsigned.message.recent_blockhash)))

    def test_send_refuses_before_the_second_release(self):
        with tempfile.TemporaryDirectory() as d:
            self.prepare(d)
            with patch.object(xc, "PAUSE", PAUSE.pubkey()):
                with self.assertRaisesRegex(ValueError, "second release"):
                    xc.require_second_release(dict(release=1))
                xc.require_second_release(dict(release=2))

    def test_the_tool_never_reads_the_pause_key(self):
        source = Path(xc.__file__).read_text()
        self.assertNotIn("exit-pause-authority", source)
        self.assertNotIn("Keypair", source)


if __name__ == "__main__":
    unittest.main()
