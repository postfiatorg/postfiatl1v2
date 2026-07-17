import unittest

from postfiat_rpc.navcoin import NavInputs, build_packet_and_operations, calculate_nav


class NavcoinExampleTests(unittest.TestCase):
    def test_calculates_exact_nav_and_operations(self) -> None:
        inputs = NavInputs(
            issuer="pfissuer-example",
            ap_account="pfap-example",
            asset_code="NAV",
            epoch=1,
            circulating_supply=1_000,
            mint_amount=1_000,
            redeem_amount=10,
            cash_micro_usd=622_300_000,
            broker_positions_micro_usd=400_000_000,
            liabilities_micro_usd=40_000_000,
            pending_redemptions_micro_usd=0,
        )

        nav = calculate_nav(inputs)
        self.assertEqual(nav["verified_net_assets_micro_usd"], 982_300_000)
        self.assertEqual(nav["nav_per_unit_micro_usd"], 982_300)
        self.assertEqual(nav["over_collateralization_remainder_micro_usd"], 0)

        packet = build_packet_and_operations(inputs)
        self.assertEqual(len(packet["asset_id"]), 96)
        self.assertEqual(len(packet["reserve_packet_hash"]), 96)
        self.assertEqual(packet["post_redeem_supply"], 990)
        self.assertEqual(packet["redemption_claim_micro_usd"], 9_823_000)

        operations = packet["operations"]
        self.assertEqual(operations["nav_reserve_submit"]["operation"], "nav_reserve_submit")
        self.assertEqual(operations["nav_reserve_submit"]["nav_per_unit"], 982_300)
        self.assertEqual(operations["nav_mint_at_nav"]["operation"], "nav_mint_at_nav")
        self.assertEqual(operations["nav_redeem_at_nav"]["operation"], "nav_redeem_at_nav")

    def test_floor_nav_reports_over_collateralization_remainder(self) -> None:
        inputs = NavInputs(
            issuer="pfissuer-example",
            ap_account="pfap-example",
            asset_code="NAV",
            epoch=1,
            circulating_supply=3,
            mint_amount=3,
            redeem_amount=1,
            cash_micro_usd=10,
            broker_positions_micro_usd=0,
            liabilities_micro_usd=0,
            pending_redemptions_micro_usd=0,
        )

        nav = calculate_nav(inputs)
        self.assertEqual(nav["verified_net_assets_micro_usd"], 10)
        self.assertEqual(nav["nav_per_unit_micro_usd"], 3)
        self.assertEqual(nav["over_collateralization_remainder_micro_usd"], 1)

        packet = build_packet_and_operations(inputs)
        self.assertTrue(
            packet["reserve_packet"]["invariant"]["verified_net_assets_gte_supply_times_nav"]
        )
        self.assertEqual(
            packet["reserve_packet"]["invariant"]["over_collateralization_remainder"], 1
        )


if __name__ == "__main__":
    unittest.main()


def test_nav_proof_profile_id_matches_rust_consensus_vector():
    from postfiat_rpc.navcoin import nav_proof_profile_id

    # Same vector asserted in crates/execution/src/lib_parts/tests.rs
    assert nav_proof_profile_id("ledger-transparent", "ledger", 10, 2, 20, 5, 50, 0, 0, "", "", "", 0, 0) == (
        "2911d88adff1737c7fa758370e4bf564eb118da2e4a594b9bdf0805eb3cebcc1e24ddd36043b77c6481937efa88e1d64"
    )


def test_profile_register_and_settle_builders_shape():
    from postfiat_rpc.navcoin import (
        build_profile_register_operation,
        build_redeem_settle_operation,
    )

    register = build_profile_register_operation(
        "pfregistrant", "ledger-transparent", "ledger", 10, 2, 20, 5, 50, 0, 0, ""
    )
    assert register["operation"] == "nav_profile_register"
    assert register["verifier_kind"] == "ledger-transparent"
    assert register["min_challenge_bond"] == 50

    settle = build_redeem_settle_operation(
        "pfissuer", "ab" * 48, "cd" * 48, "0d" * 48
    )
    assert settle["operation"] == "nav_redeem_settle"
    assert settle["settlement_receipt_hash"] == "0d" * 48


def test_reserve_accounts_flow_into_submit_operation():
    from postfiat_rpc.navcoin import NavInputs, build_packet_and_operations

    inputs = NavInputs(
        issuer="pfissuer",
        ap_account="pfap",
        asset_code="NAV",
        epoch=1,
        circulating_supply=1000,
        mint_amount=1000,
        redeem_amount=100,
        cash_micro_usd=1_000_000_000,
        broker_positions_micro_usd=0,
        liabilities_micro_usd=17_700_000,
        pending_redemptions_micro_usd=0,
        reserve_accounts=("pfreserve1", "pfreserve2"),
    )
    bundle = build_packet_and_operations(inputs)
    assert bundle["operations"]["nav_reserve_submit"]["reserve_accounts"] == [
        "pfreserve1",
        "pfreserve2",
    ]


def test_hyperliquid_normalization_is_deterministic_and_timestamp_free():
    from postfiat_rpc.hyperliquid import (
        comparable_view,
        normalize_observation,
        observation_root,
    )

    perp_fixture = {
        "marginSummary": {
            "accountValue": "1000000.5",
            "totalNtlPos": "500000.0",
            "totalMarginUsed": "100000.0",
        },
        "withdrawable": "900000.5",
        "assetPositions": [
            {
                "position": {
                    "coin": "ETH",
                    "szi": "100.0",
                    "entryPx": "2500.0",
                    "positionValue": "250000.0",
                    "unrealizedPnl": "1000.0",
                    "marginUsed": "50000.0",
                }
            },
            {
                "position": {
                    "coin": "BTC",
                    "szi": "2.5",
                    "entryPx": "100000.0",
                    "positionValue": "250000.0",
                    "unrealizedPnl": "-500.0",
                    "marginUsed": "50000.0",
                }
            },
        ],
    }
    spot_fixture = {"balances": [{"coin": "USDC", "total": "5000.0", "hold": "0.0"}]}

    obs_a = normalize_observation("0xABC", perp_fixture, spot_fixture, captured_at_unix=1)
    obs_b = normalize_observation("0xabc", perp_fixture, spot_fixture, captured_at_unix=999)

    # positions sorted by coin regardless of input order
    assert [p["coin"] for p in obs_a["perp"]["positions"]] == ["BTC", "ETH"]
    # address case-normalized
    assert obs_a["address"] == obs_b["address"] == "0xabc"
    # capture timestamp excluded from the comparable view and the root
    assert comparable_view(obs_a) == comparable_view(obs_b)
    assert observation_root(obs_a) == observation_root(obs_b)
    assert len(observation_root(obs_a)) == 96


def test_solana_normalization_deterministic_and_stake_parsing():
    from postfiat_rpc.solana import (
        comparable_view,
        normalize_observation,
        observation_root,
        stake_summary,
    )

    parsed = {
        "data": {
            "parsed": {
                "type": "delegated",
                "info": {
                    "meta": {"rentExemptReserve": "2282880"},
                    "stake": {
                        "delegation": {"stake": "5000000000", "voter": "Vote1111"}
                    },
                },
            }
        }
    }
    stake = stake_summary(parsed)
    assert stake["delegated_lamports"] == 5000000000
    assert stake["voter"] == "Vote1111"

    obs_a = normalize_observation("StakeAcc1", 7000000000, stake, captured_at_unix=1)
    obs_b = normalize_observation("StakeAcc1", 7000000000, stake, captured_at_unix=99)
    assert comparable_view(obs_a) == comparable_view(obs_b)
    assert observation_root(obs_a) == observation_root(obs_b)
    assert len(observation_root(obs_a)) == 96


def test_basis_policy_nav_and_invariants():
    from decimal import Decimal
    from postfiat_rpc.basis_policy import evaluate, policy_descriptor, policy_hash

    descriptor = policy_descriptor(
        "SOL",
        solana_accounts=["StakeAcc1"],
        hyperliquid_accounts=["0xabc"],
        hedge_band_bp=500,
        min_margin_ratio_bp=20000,
    )
    assert len(policy_hash(descriptor)) == 96

    sol_obs = {
        "balance_lamports": 0,
        "stake": {"delegated_lamports": 100_000_000_000},  # 100 SOL staked
    }
    hl_obs = {
        "perp": {
            "account_value": "5000.0",
            "total_margin_used": "1500.0",
            "positions": [
                {"coin": "SOL", "szi": "-99.0", "entry_px": "150.0",
                 "position_value": "14850.0", "unrealized_pnl": "0.0",
                 "margin_used": "1500.0"},
            ],
        }
    }
    result = evaluate(descriptor, [sol_obs], hl_obs, Decimal("150"))
    # NAV = 100 * 150 + 5000 = 20000
    assert result["nav"] == "20000.0"
    # hedge gap = |100 - 99| / 100 = 100 bp <= 500 -> hedged
    assert result["invariants"]["hedge_within_band"] is True
    # margin ratio = 5000/1500 = 3.33x = 33333 bp >= 20000 -> ok
    assert result["invariants"]["margin_above_floor"] is True
    assert result["strategy_pass"] is True

    # unhedged book fails strategy even when value is right
    hl_obs["perp"]["positions"][0]["szi"] = "-50.0"
    result = evaluate(descriptor, [sol_obs], hl_obs, Decimal("150"))
    assert result["invariants"]["hedge_within_band"] is False
    assert result["strategy_pass"] is False
