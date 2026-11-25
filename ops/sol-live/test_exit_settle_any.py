"""One reviewed settlement path for every withdrawal (exit_canary.reviewed_payout and
exit_govern.settle_paid), tested on the recorded withdrawals of 24 and 25 November."""

import base64
import json
import unittest
from copy import deepcopy
from pathlib import Path
from unittest.mock import patch

import base58

import exit_canary as c
import exit_govern as g
import exit_rules
from exit_payout import PREFIX, VERIFIER, marker

FIX = json.loads((Path(__file__).parent / "fixtures/r2-withdrawals.json").read_text())
RULES = exit_rules.of_route(FIX["route"])
BY_HEIGHT = {v["height"]: burn for burn, v in FIX["solana"].items()}
WITHDRAWALS = [BY_HEIGHT[h] for h in (1891, 1894, 1896, 1899)]
CANARY = BY_HEIGHT[1846]


def row(status, burn):
    (r,) = [r for r in status["redemptions"] if r["burn_tx_id"] == burn]
    return r


def pending(burn):
    """The recorded status before the burn's settlement: the row pending and unsettled, and its
    amount back in the bucket's counted value, queue and remaining supply."""
    status = deepcopy(FIX["status"])
    red = row(status, burn)
    amount = red["amount_atoms"]
    red.update(state="pending", settled_atoms=0, settlement_receipt_hash="")
    (bucket,) = [b for b in status["buckets"] if b["bucket_id"] == red["bucket_id"]]
    bucket["counted_value_atoms"] += amount
    bucket["redemption_queue_atoms"] += amount
    alloc = next(a for a in status["allocations"]
                 if a["bucket_id"] == red["bucket_id"] and a["purpose"] == "vault_bridge_supply")
    alloc["remaining_atoms"] += amount
    return status


class Bank:
    """Public Solana RPC as recorded: the burn's marker, its signatures and finalized transactions."""

    level = {"commitment": "finalized"}

    def __init__(self, burn, tx=None):
        self.sol = deepcopy(FIX["solana"][burn])
        if tx is not None:
            (sig,) = self.sol["transactions"]
            self.sol["transactions"][sig] = tx

    def account(self, key):
        if str(key) != self.sol["marker"]:
            return None
        acc = self.sol["marker_account"]
        return dict(acc, data=base64.b64decode(acc["data"][0]))

    def call(self, method, *args):
        if method == "getSignaturesForAddress":
            return self.sol["signatures"]
        assert method == "getTransaction", method
        return deepcopy(self.sol["transactions"].get(args[0]))


def recorded_tx(burn):
    (tx,) = FIX["solana"][burn]["transactions"].values()
    return deepcopy(tx)


class Review(unittest.TestCase):
    def review(self, burn, statuses, bank=None, rules=RULES, **expect):
        statuses = statuses if isinstance(statuses, list) else [statuses] * 6

        def rpc(i, method, **params):
            self.assertEqual(method, "vault_bridge_status")
            if statuses[i] is None:
                raise OSError(f"validator-{i} unreachable")
            return deepcopy(statuses[i])

        with patch.object(c, "rpc", rpc), patch.object(c, "cluster", return_value=bank or Bank(burn)):
            return c.reviewed_payout(dict(burn_tx_id=burn), 5, rules=rules, **expect)

    def expected_kind(self, burn):
        tx = recorded_tx(burn)
        (sig,) = tx["transaction"]["signatures"]
        packet = row(FIX["status"], burn)["withdrawal_packet_hash"]
        return f"solana_exit_r8_paid:{packet}:{tx['slot']}:0:{base58.b58decode(sig).hex()}"

    def test_the_recorded_withdrawals_and_the_canary_pass(self):
        for burn in WITHDRAWALS + [CANARY]:
            with self.subTest(height=FIX["solana"][burn]["height"]):
                ref, before = self.review(burn, pending(burn))
                self.assertEqual(ref["kind"], self.expected_kind(burn))
                self.assertEqual(before["state"], "pending")
        # The canary's own flow keeps its pinned owner and wallet.
        ref, _ = self.review(CANARY, pending(CANARY), expect_owner=c.OWNER, expect_destination=c.DEST)
        self.assertEqual(ref["kind"], self.expected_kind(CANARY))

    def test_the_canary_pin_refuses_a_payout_to_another_destination(self):
        burn = BY_HEIGHT[1896]
        self.assertNotEqual(row(FIX["status"], burn)["destination_ref"], c.DEST)
        with self.assertRaisesRegex(ValueError, "owner or destination"):
            self.review(burn, pending(burn), expect_owner=c.OWNER, expect_destination=c.DEST)

    def test_a_record_naming_another_destination_than_the_payout_is_refused(self):
        burn, other = BY_HEIGHT[1896], BY_HEIGHT[1899]
        status = pending(burn)
        row(status, burn)["destination_ref"] = row(FIX["status"], other)["destination_ref"]
        with self.assertRaises(ValueError):
            self.review(burn, status)

    def test_a_wrong_amount_is_refused(self):
        burn = BY_HEIGHT[1894]
        status = pending(burn)
        row(status, burn)["amount_atoms"] += 1
        with self.assertRaises(ValueError):
            self.review(burn, status)

    def test_two_payouts_in_one_transaction_are_refused(self):
        burn = BY_HEIGHT[1891]
        tx = recorded_tx(burn)
        keys = tx["transaction"]["message"]["accountKeys"]
        instructions = tx["transaction"]["message"]["instructions"]
        payout = next(ix for ix in instructions if keys[ix["programIdIndex"]] == str(VERIFIER)
                      and base58.b58decode(ix["data"])[:1] == b"\x02")
        for case in ("top level", "inner"):
            altered = deepcopy(tx)
            if case == "top level":
                altered["transaction"]["message"]["instructions"].append(payout)
            else:
                altered["meta"]["innerInstructions"] = [dict(index=0, instructions=[payout])]
            with self.subTest(case=case), self.assertRaisesRegex(ValueError, "payout"):
                self.review(burn, pending(burn), Bank(burn, altered))

    def test_a_settled_redemption_is_refused(self):
        burn = BY_HEIGHT[1899]
        with self.assertRaisesRegex(ValueError, "pending, unsettled"):
            self.review(burn, FIX["status"])
        status = pending(burn)
        row(status, burn)["settlement_receipt_hash"] = "ab" * 48
        with self.assertRaisesRegex(ValueError, "pending, unsettled"):
            self.review(burn, status)

    def test_an_uncovered_bucket_is_refused(self):
        burn = BY_HEIGHT[1896]
        bucket = row(FIX["status"], burn)["bucket_id"]
        # Validator policy alone would cover it; the chain's covered list does not.
        rules = dict(RULES, covered=tuple(b for b in RULES["covered"] if b != bucket))
        with self.assertRaisesRegex(ValueError, "covered"):
            self.review(burn, pending(burn), rules=rules)

    def test_a_destination_off_the_curve_or_not_canonical_is_refused(self):
        burn = BY_HEIGHT[1891]
        for name, destination in (
            ("off curve", PREFIX + bytes(marker(burn)).hex()),
            ("upper case", row(FIX["status"], burn)["destination_ref"].upper().replace(PREFIX.upper(), PREFIX)),
        ):
            status = pending(burn)
            row(status, burn)["destination_ref"] = destination
            with self.subTest(name), self.assertRaises(ValueError):
                self.review(burn, status)

    def test_validators_that_disagree_settle_nothing_below_five(self):
        burn = BY_HEIGHT[1894]
        other = pending(burn)
        row(other, burn)["amount_atoms"] = 1
        # Five agree and one differs: the five decide.
        ref, _ = self.review(burn, [pending(burn)] * 5 + [other])
        self.assertEqual(ref["kind"], self.expected_kind(burn))
        # Four agree, two differ, or one is lost and one differs: nothing passes.
        for statuses in ([pending(burn)] * 4 + [other] * 2, [pending(burn)] * 4 + [other, None]):
            with self.assertRaisesRegex(RuntimeError, "differ"):
                self.review(burn, statuses)


class SettlePaid(unittest.TestCase):
    def run_settlement(self, burn, after):
        statuses = {"now": pending(burn)}
        receipt = row(FIX["status"], burn)["settlement_receipt_hash"]

        def govern(kind, **kwargs):
            self.assertEqual(kind, "exit-paid")
            self.assertEqual(kwargs["payout"]["kind"], Review.expected_kind(None, burn))
            statuses["now"] = after
            return dict(tx_id=receipt, height=FIX["height"], signers=[f"validator-{i}" for i in range(5)])

        def rpc(i, method, **params):
            return deepcopy(statuses["now"])

        with (patch.object(c, "rpc", rpc), patch.object(c, "cluster", return_value=Bank(burn)),
              patch.object(g, "govern", side_effect=govern) as proposed, patch.object(g, "SETTLED_PAUSE", 0)):
            return g.settle_paid(dict(burn_tx_id=burn), 5, RULES, "04600868", "parent"), proposed

    def test_the_recorded_settlements_reconcile(self):
        for burn in WITHDRAWALS:
            with self.subTest(height=FIX["solana"][burn]["height"]):
                result, proposed = self.run_settlement(burn, deepcopy(FIX["status"]))
                proposed.assert_called_once()
                self.assertEqual(result["after"]["state"], "settled")

    def test_a_settlement_whose_accounting_differs_is_reported(self):
        burn = BY_HEIGHT[1891]
        after = deepcopy(FIX["status"])
        (bucket,) = [b for b in after["buckets"] if b["bucket_id"] == row(after, burn)["bucket_id"]]
        bucket["counted_value_atoms"] += 1
        with self.assertRaisesRegex(ValueError, "accounting"):
            self.run_settlement(burn, after)

    def test_a_refused_review_proposes_nothing(self):
        burn = BY_HEIGHT[1891]
        with (patch.object(c, "rpc", lambda i, m, **p: deepcopy(FIX["status"])),
              patch.object(c, "cluster", return_value=Bank(burn)), patch.object(g, "govern") as proposed):
            with self.assertRaises(ValueError):
                g.settle_paid(dict(burn_tx_id=burn), 5, RULES, "04600868", "parent")
        proposed.assert_not_called()


if __name__ == "__main__":
    unittest.main()
