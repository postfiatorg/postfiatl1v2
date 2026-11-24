"""Every Solana-source bucket of the shared custody that the descriptor covers is payable."""

import unittest
from unittest.mock import patch

import exit_payout as payout
from exit_payout import BUCKET, POLICY, PREFIX, SOURCE_DOMAIN

REV9 = "9a" * 48


def bucket(policy, **over):
    return dict(
        dict(
            bucket_id=payout.bucket_id(policy),
            policy_hash=policy,
            source_series_id="5e" * 48,
            source_domain=SOURCE_DOMAIN,
            status="active",
            redeemable_at_par=True,
            impairment_factor_bps=10000,
            outstanding_vault_bridge_atoms=400000,
            redemption_queue_atoms=0,
            other_allocations_atoms=0,
            nav_subscription_allocations_atoms=0,
            counted_value_atoms=400000,
        ),
        **over,
    )


def burn_row(bucket_id, burn="ab" * 48):
    return dict(
        burn_tx_id=burn,
        bucket_id=bucket_id,
        amount_atoms=300000,
        destination_ref=PREFIX + "ce" * 32,
        withdrawal_packet_hash="77" * 48,
        withdrawal_packet=dict(source_bucket_id=bucket_id, burn_tx_id=burn),
    )


class CustodyBucketsTest(unittest.TestCase):
    def test_bucket_derivation_matches_the_chain(self):
        self.assertEqual(payout.bucket_id(POLICY), BUCKET)
        # A non-revision-8 policy; the value is computed by the Rust side in
        # crates/types/src/tests.rs (solana_exit_bucket_of_another_revision_...).
        self.assertEqual(payout.bucket_id("09" * 48), "4683dbe99fcb2aef277f095e459ff541e1ef0774222aa6c1962e091d8f18c14743b82a54518b1f7eb48c451c9c5b21ce")

    def test_covered_buckets_are_the_custody_buckets_only(self):
        rev9 = bucket(REV9)
        other = bucket(REV9, source_domain="spl:other")
        forged = dict(bucket(REV9), bucket_id="ff" * 48)
        covered = payout.covered_buckets([bucket(POLICY), rev9, other, forged])
        self.assertEqual(set(covered), {BUCKET, rev9["bucket_id"]})

    def test_service_pays_only_the_buckets_the_chain_reports(self):
        rev9 = bucket(REV9)
        status = [bucket(POLICY), rev9]
        signatures = dict(open=6, close=5, paid=5, min_withdrawal=5, retire=6)
        first = dict(active=True, burns_open=True)
        second = dict(first, covered_bucket_ids=[BUCKET, rev9["bucket_id"]],
                      decision_signatures=signatures, min_withdrawal_atoms=300000)
        for route, payable in ((first, {BUCKET}), (second, {BUCKET, rev9["bucket_id"]})):
            with patch.object(payout, "rpc_any", return_value=route) as read:
                rules = payout.chain_rules()
            read.assert_called_once_with("vault_bridge_route", asset_id=payout.SERIES)
            covered = payout.covered_buckets(status, rules["covered"])
            self.assertEqual(set(covered), payable)
            # A first-release chain refuses another revision's burn, so the service does too.
            refused = payout.validate_burn
            if rev9["bucket_id"] not in payable:
                with self.assertRaises(ValueError):
                    refused(burn_row(rev9["bucket_id"]), "ab" * 48, covered)
            else:
                refused(burn_row(rev9["bucket_id"]), "ab" * 48, covered)

    def test_a_burn_of_any_covered_revision_is_payable(self):
        covered = payout.covered_buckets([bucket(POLICY), bucket(REV9)])
        for bucket_id in covered:
            payout.validate_burn(burn_row(bucket_id), "ab" * 48, covered)
        with self.assertRaises(ValueError):
            payout.validate_burn(burn_row("ff" * 48), "ab" * 48, covered)

    def test_witness_is_bound_to_its_burn_bucket_and_packet_hash(self):
        red = burn_row(payout.bucket_id(REV9))
        witness = dict(
            receipt=dict(tx_id=red["burn_tx_id"], accepted=True),
            route_profile=dict(profile_hash=payout.EXIT),
            withdrawal_packet=dict(red["withdrawal_packet"]),
            withdrawal_packet_hash=red["withdrawal_packet_hash"],
        )
        payout.bind_witness(witness, red, red["burn_tx_id"])
        for change in (
            dict(withdrawal_packet_hash="78" * 48),
            dict(withdrawal_packet=dict(witness["withdrawal_packet"], source_bucket_id=BUCKET)),
            dict(withdrawal_packet=dict(witness["withdrawal_packet"], burn_tx_id="cd" * 48)),
        ):
            with self.assertRaises(ValueError):
                payout.bind_witness(dict(witness, **change), red, red["burn_tx_id"])

    def test_custody_check_covers_every_bucket_that_holds_value(self):
        status = dict(buckets=[bucket(POLICY), bucket(REV9)])
        with (
            patch.object(payout, "rpc", return_value=status),
            patch.object(payout, "inspect_vault", return_value={"custody_atoms": 800000}),
        ):
            _, covered, required = payout.check_custody(None)
            self.assertEqual((len(covered), required), (2, 800000))
            status["buckets"][1]["impairment_factor_bps"] = 9000
            with self.assertRaisesRegex(ValueError, "unavailable"):
                payout.check_custody(None)
            # An empty earlier bucket does not block the others.
            status["buckets"][1].update(
                outstanding_vault_bridge_atoms=0, counted_value_atoms=0
            )
            payout.check_custody(None)

    def test_wallet_offers_every_solana_series_the_account_holds(self):
        status = dict(buckets=[bucket(POLICY), bucket(REV9, source_series_id="6e" * 48)])
        lines = {
            "5e" * 48: [dict(balance="500000", authorized=True, frozen=False)],
            "6e" * 48: [dict(balance="250000", authorized=True, frozen=False)],
        }

        def rpc(index, method, **args):
            if method == "vault_bridge_status":
                return status
            self.assertEqual((method, args["account"]), ("account_assets", "pf" + "0b" * 20))
            return dict(assets=lines[args["asset_id"]])

        with patch.object(payout, "rpc", side_effect=rpc):
            offer = payout.holdings("pf" + "0b" * 20)
        self.assertEqual(
            offer,
            [
                dict(bucket_id=BUCKET, series_asset_id="5e" * 48, balance_atoms="500000", redeemable_atoms="400000"),
                dict(bucket_id=payout.bucket_id(REV9), series_asset_id="6e" * 48, balance_atoms="250000", redeemable_atoms="250000"),
            ],
        )
        with self.assertRaises(ValueError):
            payout.holdings("not-an-account")


    def test_readiness_for_an_account_lists_its_series(self):
        import exit_service as svc

        offer = [dict(bucket_id=BUCKET, series_asset_id="5e" * 48, balance_atoms="1", redeemable_atoms="1")]
        with (
            patch.object(svc, "service_readiness", return_value=dict(ok=True, ready=True)),
            patch.object(svc, "holdings", return_value=offer) as held,
        ):
            self.assertEqual(
                svc.account_readiness(None, "/solana/withdrawals/readiness?account=pf" + "0b" * 20),
                dict(ok=True, ready=True, account="pf" + "0b" * 20, series=offer),
            )
            held.assert_called_once_with("pf" + "0b" * 20, None, minimum=1)
            self.assertIsNone(svc.account_readiness(None, "/solana/withdrawals/readiness"))
        with (
            patch.object(svc, "service_readiness", return_value=dict(ok=True, ready=True)),
            patch.object(svc, "holdings", side_effect=ValueError("Invalid account.")),
        ):
            value = svc.account_readiness(None, "/solana/withdrawals/readiness?account=x")
            self.assertEqual((value["ready"], value["series"]), (False, []))


    def test_canary_burns_a_covered_series_and_settles_its_own_bucket(self):
        import exit_canary

        rev9 = payout.bucket_id(REV9)
        r = dict(
            series=[
                dict(bucket_id=BUCKET, series_asset_id="5e" * 48, redeemable_atoms="20000"),
                dict(bucket_id=rev9, series_asset_id="6e" * 48, redeemable_atoms="90000"),
            ]
        )
        self.assertEqual(
            exit_canary.choose_series(r, 20000),
            dict(bucket_id=rev9, series_asset_id="6e" * 48),
        )
        with self.assertRaises(ValueError):
            exit_canary.choose_series(r, 100000)
        paid = dict(burn_tx_id="ab" * 48, withdrawal_packet_hash="77" * 48)
        red = dict(
            burn_tx_id=paid["burn_tx_id"],
            withdrawal_packet_hash=paid["withdrawal_packet_hash"],
            bucket_id=rev9,
            amount_atoms=20000,
            state="pending",
            settled_atoms=0,
            settlement_receipt_hash="",
        )
        status = dict(
            buckets=[bucket(POLICY), bucket(REV9)],
            allocations=[],
            redemptions=[red],
            receipts=[],
        )
        state = exit_canary.settlement_state(status, paid)
        self.assertEqual(state["outstanding"], 400000)
        status["buckets"] = [bucket(POLICY), dict(bucket(REV9), source_domain="spl:other")]
        with self.assertRaises(ValueError):
            exit_canary.settlement_state(status, paid)


if __name__ == "__main__":
    unittest.main()
