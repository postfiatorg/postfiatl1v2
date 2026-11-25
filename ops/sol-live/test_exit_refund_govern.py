"""A refund decision runs end to end like close and paid: drafted from the cancelled marker,
reviewed and signed by five of six with one validator lost, then submitted."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import exit_govern
import exit_payout as payout
from exit_precheck import parent_of
from exit_setup import read
from test_exit_lost_validator import LOST, REV, Fleet, run

PACKET = "a7" * 48
BURN = "b7" * 48
SIG = "cd" * 64
KIND = f"solana_exit_refund:{PACKET}:473300000:{SIG}"


class RefundFleet(Fleet):
    def rpc(self, i, method, **params):
        value = super().rpc(i, method, **params)
        if method == "vault_bridge_route" and "decision_signatures" in value:
            value["decision_signatures"] = dict(value["decision_signatures"], refund=5)
        return value

    def remote(self, i, args, data=None):
        args = [str(a) for a in args]
        if i != LOST and args[-1] == "/etc/postfiat/sol-exit-refund-review.json":
            return json.dumps(dict(kind=self.kind, parent=parent_of(self.head)))
        if i != LOST and len(args) > 5 and args[5] == "ratify-solana-exit-refund":
            flags = args[6:]
            value = {flags[k]: flags[k + 1] for k in range(len(flags) - 1) if flags[k].startswith("--")}
            self.kind = "solana_exit_refund:{}:{}:{}".format(
                value["--packet-hash"], value["--solana-slot"], value["--signature-hex"])
            amend = dict(kind=self.kind, activation_height=int(value["--height"]), value=1,
                         paused=False, quorum=5, validators=value["--validators"].split(","))
            self.files[(i, value["--amendment-file"])] = json.dumps(amend)
            return ""
        return super().remote(i, args, data)


def status(state="pending"):
    red = dict(burn_tx_id=BURN, bucket_id=payout.BUCKET, amount_atoms=300000, state=state,
               settled_atoms=0, withdrawal_packet_hash=PACKET)
    return dict(buckets=[], redemptions=[red], allocations=[], receipts=[])


class RefundGovernTest(unittest.TestCase):
    def test_refund_runs_end_to_end_with_one_validator_lost(self):
        fleet = RefundFleet(status())
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "refund.json"
            with (
                patch.object(exit_govern, "cluster", create=True),
                patch.object(exit_govern, "refund_kind", create=True, return_value=KIND),
            ):
                run(fleet, d, ["refund", "--revision", REV, "--release-height", "1",
                               "--burn", BURN, "--output", str(out)])
            saved = read(out)
            self.assertEqual(fleet.kind, KIND)
            self.assertEqual(len(saved["signers"]), 5)
            self.assertNotIn(f"validator-{LOST}", saved["signers"])

    def test_only_a_pending_burn_is_refunded(self):
        for state in ("settled", "refunded"):
            fleet = RefundFleet(status(state))
            with tempfile.TemporaryDirectory() as d:
                with (
                    patch.object(exit_govern, "cluster", create=True),
                    patch.object(exit_govern, "refund_kind", create=True, return_value=KIND),
                    self.assertRaisesRegex(ValueError, "pending"),
                ):
                    run(fleet, d, ["refund", "--revision", REV, "--release-height", "1",
                                   "--burn", BURN, "--output", str(Path(d) / "r.json")])
            self.assertIsNone(fleet.kind)


if __name__ == "__main__":
    unittest.main()
