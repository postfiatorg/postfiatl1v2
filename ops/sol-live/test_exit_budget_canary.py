import unittest
from unittest.mock import Mock
import exit_payout as p
import exit_canary as canary


class BudgetCanaryTest(unittest.TestCase):
    def test_shared_custody_reserves_and_fee_floor(self):
        b = dict(
            source_domain="same",
            outstanding_vault_bridge_atoms=20000,
            redemption_queue_atoms=20000,
            nav_subscription_allocations_atoms=0,
            other_allocations_atoms=0,
            counted_value_atoms=40000,
        )
        self.assertEqual(
            p.shared_liabilities(
                [
                    b,
                    dict(b, redemption_queue_atoms=0, counted_value_atoms=20000),
                    dict(b, source_domain="other"),
                ],
                "same",
            ),
            60000,
        )
        c = Mock(owner=p.FEE_PAYER)
        c.rent.side_effect = lambda n: 100 + n
        c.call.side_effect = lambda method, *args: (
            {"value": {"blockhash": str(p.Hash.default())}}
            if method == "getLatestBlockhash"
            else {"value": 10000 if method == "getFeeForMessage" else p.FLOOR}
        )
        debit = p.payout_budget(c, 1)
        self.assertGreater(debit, 10000 + c.rent(2048))
        with self.assertRaises(ValueError):
            p.balance(c, debit)

    def test_complete_signed_fields_bind_fee_sequence_and_domain(self):
        plan = dict(
            chain_id="net",
            genesis_hash="00" * 48,
            source=canary.OWNER,
            fee=23,
            sequence=12,
            protocol_version=1,
            operation={
                "operation": "vault_bridge_burn_to_redeem",
                "amount_atoms": 20000,
            },
        )
        u = canary.expected_unsigned(plan)
        self.assertEqual(u["amount_atoms"], 20000)
        for k in u:
            with self.assertRaises(ValueError):
                canary.check_unsigned(dict(u, **{k: "different"}), plan)

    def test_canary_uses_transaction_local_token_deltas(self):
        recipient = str(
            p.pda(p.ATA, bytes(canary.PHANTOM), bytes(p.TOKEN), bytes(p.MINT))
        )
        custody = str(p.pda(p.VAULT, b"custody"))

        def row(i, n):
            return dict(
                accountIndex=i,
                mint=str(p.MINT),
                uiTokenAmount={"amount": str(n), "decimals": 6},
            )

        tx = {
            "transaction": {"message": {"accountKeys": [custody, recipient]}},
            "meta": {
                "err": None,
                "preTokenBalances": [row(0, 80000)],
                "postTokenBalances": [row(0, 60000), row(1, 20000)],
            },
        }
        self.assertEqual(
            canary.token_deltas(tx), {"custody": -20000, "recipient": 20000}
        )
        tx["meta"]["postTokenBalances"][0] = row(0, 59999)
        with self.assertRaises(ValueError):
            canary.token_deltas(tx)

    def test_budget_uses_selected_segments_and_peak_buffer_rent(self):
        from unittest.mock import patch
        c = Mock(owner=p.FEE_PAYER)
        c.rent.side_effect = lambda size: 100 + size
        c.call.side_effect = [{"value": {"blockhash": str(p.Hash.default())}}, {"value": 7}]
        with patch.object(p, "select_target", return_value={"height": 1818}) as select:
            count = p.proof_count(1753, 1883, 1882)
            self.assertEqual(count, 2)
            select.assert_called_once()
            self.assertEqual(p.payout_budget(c, count), 2*(6*7+148)+2148+101+265)

    def test_partial_system_accounts_need_rent_topups(self):
        c = Mock()
        c.rent.return_value = 2000
        partial = dict(owner=str(p.SYSTEM), executable=False, data=b"", lamports=800)
        for size, program in [(48, p.VERIFIER), (165, p.TOKEN), (1, p.VERIFIER)]:
            self.assertEqual(p.rent_shortfall(c, partial, size, program), 1200)
            self.assertEqual(p.rent_shortfall(c, None, size, program), 2000)
            self.assertEqual(p.rent_shortfall(c, dict(partial, lamports=3000), size, program), 0)

    def test_nonwithdrawal_sends_the_message_used_for_fee_quote(self):
        import base64
        from solders.keypair import Keypair
        from solders.transaction import Transaction
        c = Mock()
        c.payer = Keypair()
        c.owner = c.payer.pubkey()
        quoted = []
        def call(method, *args):
            if method == "getLatestBlockhash":
                return {"value": {"blockhash": str(p.Hash.default()), "lastValidBlockHeight": 50}}
            if method == "getFeeForMessage":
                quoted.append(base64.b64decode(args[0]))
                return {"value": 5000}
            if method == "getBalance":
                return {"value": p.FLOOR + 6200}
            tx = Transaction.from_bytes(base64.b64decode(args[0]))
            self.assertEqual(bytes(tx.message), quoted[0])
            return str(tx.signatures[0])
        c.call.side_effect = call
        p.funded_send(c,"exit.checkpoint",[],rent=1200)
        c.confirm.assert_called_once()
