import unittest
from copy import deepcopy
import base58
import exit_canary as c
import exit_payout as p


class SettlementTest(unittest.TestCase):
    def test_finalized_ordinal_accounts_and_draft_match_paid_record(self):
        sig = base58.b58encode(bytes(range(64))).decode()
        burn = "ab" * 48
        packet = "cd" * 48
        ata = p.pda(p.ATA, bytes(c.PHANTOM), bytes(p.TOKEN), bytes(p.MINT))
        accounts = [
            str(x)
            for x in [
                p.FEE_PAYER,
                p.LIGHT,
                "prior",
                "next",
                "buffer",
                p.SYSTEM,
                p.marker(burn),
                p.VAULT,
                p.pda(p.VERIFIER, b"egress-release", bytes(p.VAULT)),
                p.pda(p.VAULT, b"config"),
                p.pda(p.VAULT, b"custody"),
                ata,
                c.PHANTOM,
                p.MINT,
                p.TOKEN,
            ]
        ]
        ix = dict(
            programIdIndex=15,
            accounts=list(range(15)),
            data=base58.b58encode(b"\2" + bytes(356)).decode(),
        )

        def row(i, n):
            return dict(
                accountIndex=i,
                mint=str(p.MINT),
                uiTokenAmount=dict(amount=str(n), decimals=6),
            )

        tx = dict(
            slot=9,
            transaction=dict(
                signatures=[sig],
                message=dict(
                    accountKeys=accounts + [str(p.VERIFIER)], instructions=[ix]
                ),
            ),
            meta=dict(
                err=None,
                innerInstructions=[],
                preTokenBalances=[row(10, 20000)],
                postTokenBalances=[row(10, 0), row(11, 20000)],
            ),
        )
        paid = dict(
            burn_tx_id=burn,
            finalized_slot=9,
            solana_tx=sig,
            withdrawal_packet_hash=packet,
        )
        ref = c.finalized_reference(paid, tx)
        self.assertEqual(
            ref["kind"],
            "solana_exit_r8_paid:" + packet + ":9:0:" + bytes(range(64)).hex(),
        )
        for field, value in [("finalized_slot", 10), ("solana_tx", "different")]:
            with self.assertRaises(ValueError):
                c.finalized_reference(dict(paid, **{field: value}), tx)
        for key in ["duplicate", "inner", "wrong owner"]:
            altered = deepcopy(tx)
            if key == "duplicate":
                altered["transaction"]["message"]["instructions"].insert(0, ix)
            elif key == "inner":
                altered["meta"]["innerInstructions"] = [
                    dict(index=0, instructions=[ix])
                ]
            else:
                altered["transaction"]["message"]["instructions"][0]["accounts"][12] = 0
            with self.assertRaises(ValueError):
                c.finalized_reference(paid, altered)
        with self.assertRaises(ValueError):
            c.match_reference(dict(kind=ref["kind"].replace(":9:0:", ":9:1:")), ref)

    def test_accounting_reconciles_concurrent_burn_deposit_and_settlement(self):
        paid = dict(burn_tx_id="a", withdrawal_packet_hash="packet")
        red = dict(**paid, amount_atoms=20000, bucket_id=c.BUCKET, state="pending",
                   settled_atoms=0, settlement_receipt_hash="")
        other = dict(red, burn_tx_id="b", withdrawal_packet_hash="second", amount_atoms=10000)
        bucket = dict(bucket_id=c.BUCKET, counted_value_atoms=60000,
                      redemption_queue_atoms=20000, outstanding_vault_bridge_atoms=40000)
        alloc = dict(bucket_id=c.BUCKET, purpose="vault_bridge_supply", remaining_atoms=60000,
                     amount_atoms=60000)
        receipt = dict(bucket_id=c.BUCKET, counted_value_atoms=60000)
        status = dict(redemptions=[red], buckets=[bucket], allocations=[alloc], receipts=[receipt])
        before = c.settlement_state(status, paid)
        red.update(state="settled", settled_atoms=20000, settlement_receipt_hash="id")
        status["redemptions"].append(other)
        bucket.update(counted_value_atoms=40000, redemption_queue_atoms=10000,
                      outstanding_vault_bridge_atoms=30000)
        alloc["remaining_atoms"] = 40000
        c.check_settlement(before, c.settlement_state(status, paid), "id")
        status["allocations"].append(dict(alloc, amount_atoms=5000, remaining_atoms=5000))
        status["receipts"].append(dict(receipt, counted_value_atoms=5000))
        bucket.update(counted_value_atoms=35000, redemption_queue_atoms=0,
                      outstanding_vault_bridge_atoms=35000)
        other.update(state="settled", settled_atoms=10000, settlement_receipt_hash="id2")
        alloc["remaining_atoms"] -= 10000
        after = c.settlement_state(status, paid)
        c.check_settlement(before, after, "id")
        for k in ("counted", "supply", "outstanding", "queue"):
            with self.assertRaises(ValueError):
                c.check_settlement(before, dict(after, **{k: after[k]+1}), "id")
