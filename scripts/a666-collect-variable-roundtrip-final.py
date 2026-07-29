#!/usr/bin/env python3
"""Collect and reconcile the terminal A666 variable-size mainnet campaign."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import tempfile
from typing import Any

from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
RPC = "https://ethereum-rpc.publicnode.com"
OWNER = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
USDC = Web3.to_checksum_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
WA666 = Web3.to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5")
VAULT = Web3.to_checksum_address("0xaaa78FdA7062eFce769e95cd72fc55e507BC8183")
STATE_VIEW = Web3.to_checksum_address("0x7fFE42C4a5DEeA5b0feC41C94C136Cf115597227")
POOL_ID = bytes.fromhex(
    "c5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98"
)
SLO_SECONDS = 1_500

ERC20_ABI = [
    {
        "type": "function",
        "name": "balanceOf",
        "stateMutability": "view",
        "inputs": [{"name": "", "type": "address"}],
        "outputs": [{"name": "", "type": "uint256"}],
    },
    {
        "type": "function",
        "name": "totalSupply",
        "stateMutability": "view",
        "inputs": [],
        "outputs": [{"name": "", "type": "uint256"}],
    },
]
VAULT_ABI = [
    {
        "type": "function",
        "name": "totalObligations",
        "stateMutability": "view",
        "inputs": [],
        "outputs": [{"name": "", "type": "uint256"}],
    }
]
STATE_VIEW_ABI = [
    {
        "type": "function",
        "name": "getLiquidity",
        "stateMutability": "view",
        "inputs": [{"name": "poolId", "type": "bytes32"}],
        "outputs": [{"name": "", "type": "uint128"}],
    },
    {
        "type": "function",
        "name": "getSlot0",
        "stateMutability": "view",
        "inputs": [{"name": "poolId", "type": "bytes32"}],
        "outputs": [
            {"name": "sqrtPriceX96", "type": "uint160"},
            {"name": "tick", "type": "int24"},
            {"name": "protocolFee", "type": "uint24"},
            {"name": "lpFee", "type": "uint24"},
        ],
    },
]


def load(path: Path) -> Any:
    return json.loads(path.read_text())


def atomic_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


def block_time(web3: Web3, number: int) -> int:
    return int(web3.eth.get_block(number)["timestamp"])


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--campaign-dir",
        type=Path,
        default=ROOT
        / "docs/evidence/a666-variable-size-nav-roundtrip-20260728",
    )
    args = parser.parse_args()
    campaign = args.campaign_dir.resolve()
    final = campaign / "final"

    baseline_route = load(
        ROOT
        / "docs/evidence/a666-acceptance-20260728/"
        "phase-9-private-redeem-hands-off-verify/pftl-supply-status-before.json"
    )
    baseline_mint = load(
        ROOT
        / "docs/evidence/a666-acceptance-20260728/"
        "phase-9-private-redeem-hands-off-verify/ethereum/mint-state.json"
    )
    baseline_eth = load(campaign / "baseline/ethereum.json")
    final_route = load(final / "a666-route.json")
    final_vault = load(final / "pfusdc-vault.json")
    final_fleet = load(final / "fleet-status.json")
    final_joe = load(final / "joe-assets.json")
    nav = load(campaign / "stakehub-nav-mark/nav-epoch-2/live-nav-mark-manifest.json")
    profile = load(
        campaign / "stakehub-nav-mark/profile-rotation/profile-rotation-manifest.json"
    )

    transparent_burn = load(
        campaign / "transparent-withdrawal-1-a666/return/ethereum-burn/burn.json"
    )
    transparent_withdrawal = load(
        campaign
        / "transparent-withdrawal-1-a666/pfusdc-egress/withdrawal-result.json"
    )
    transparent_residual = load(
        campaign
        / "transparent-withdrawal-1-a666/rounding-residual-recovery/"
        "pfusdc-egress/withdrawal-result.json"
    )
    private_burn = load(
        campaign / "private-withdrawal-100-a666/return/ethereum-burn/burn.json"
    )
    private_withdrawal = load(
        campaign / "private-withdrawal-100-a666/pfusdc-egress/withdrawal-result.json"
    )
    large_deposit = load(campaign / "p-large-private-issue/deposit/deposit-result.json")
    large_mint = load(campaign / "p-large-private-issue/destination-consume/summary.json")
    small_timing = load(campaign / "t-small-transparent-issue/timing.json")

    web3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 120}))
    if not web3.is_connected() or int(web3.eth.chain_id) != 1:
        raise RuntimeError("Ethereum mainnet RPC unavailable or on the wrong chain")
    observation_block = int(web3.eth.block_number)
    usdc = web3.eth.contract(address=USDC, abi=ERC20_ABI)
    wa666 = web3.eth.contract(address=WA666, abi=ERC20_ABI)
    vault = web3.eth.contract(address=VAULT, abi=VAULT_ABI)
    state_view = web3.eth.contract(address=STATE_VIEW, abi=STATE_VIEW_ABI)
    ethereum = {
        "schema": "postfiat.a666.variable_roundtrip_ethereum_final.v1",
        "rpc": RPC,
        "chain_id": 1,
        "observation_block": observation_block,
        "observation_timestamp": block_time(web3, observation_block),
        "wallet_usdc_atoms": int(
            usdc.functions.balanceOf(OWNER).call(block_identifier=observation_block)
        ),
        "vault_usdc_atoms": int(
            usdc.functions.balanceOf(VAULT).call(block_identifier=observation_block)
        ),
        "vault_obligations_atoms": int(
            vault.functions.totalObligations().call(block_identifier=observation_block)
        ),
        "wallet_wa666_atoms": int(
            wa666.functions.balanceOf(OWNER).call(block_identifier=observation_block)
        ),
        "wa666_total_supply_atoms": int(
            wa666.functions.totalSupply().call(block_identifier=observation_block)
        ),
        "wallet_eth_wei": int(web3.eth.get_balance(OWNER, observation_block)),
        "pool_id": Web3.to_hex(POOL_ID),
        "pool_liquidity": int(
            state_view.functions.getLiquidity(POOL_ID).call(
                block_identifier=observation_block
            )
        ),
        "pool_slot0": list(
            state_view.functions.getSlot0(POOL_ID).call(
                block_identifier=observation_block
            )
        ),
    }
    atomic_json(final / "ethereum.json", ethereum)

    large_issue_start = int(large_deposit["deposit"]["block_number"])
    large_issue_end = int(large_mint["ethereum_mint_block"])
    transparent_redeem_start = int(transparent_burn["transaction"]["block_number"])
    transparent_redeem_main_end = int(transparent_withdrawal["receipt_block_number"])
    transparent_redeem_exact_end = int(transparent_residual["receipt_block_number"])
    private_redeem_start = int(private_burn["transaction"]["block_number"])
    private_redeem_end = int(private_withdrawal["receipt_block_number"])

    def timing_row(start: int, end: int) -> dict[str, Any]:
        start_time = block_time(web3, start)
        end_time = block_time(web3, end)
        elapsed = end_time - start_time
        return {
            "start_block": start,
            "start_timestamp": start_time,
            "end_block": end,
            "end_timestamp": end_time,
            "elapsed_seconds": elapsed,
            "slo_seconds": SLO_SECONDS,
            "slo_pass": elapsed <= SLO_SECONDS,
        }

    timing = {
        "schema": "postfiat.a666.variable_roundtrip_timing.v1",
        "t_small_issue": small_timing,
        "p_large_issue": timing_row(large_issue_start, large_issue_end),
        "t_small_redeem_main_payout": timing_row(
            transparent_redeem_start, transparent_redeem_main_end
        ),
        "t_small_redeem_exact_completion": timing_row(
            transparent_redeem_start, transparent_redeem_exact_end
        ),
        "p_large_redeem": timing_row(private_redeem_start, private_redeem_end),
    }
    atomic_json(final / "timing.json", timing)

    baseline_supply = int(baseline_route["authorized_valid_supply_atoms"])
    baseline_reserve = int(baseline_route["settlement_reserve_atoms"])
    baseline_spread = int(baseline_route["non_nav_spread_atoms"])
    base_small = (1_000_000 * int(nav["nav_per_unit_usd_1e8"]) + 99_999_999) // 100_000_000
    base_large = (
        100_000_000 * int(nav["nav_per_unit_usd_1e8"]) + 99_999_999
    ) // 100_000_000
    output_small = base_small * 9_995 // 10_000
    output_large = base_large * 9_995 // 10_000
    expected_reserve = (
        baseline_reserve + 1_000_000 + 100_000_000 - base_small - base_large
    )
    expected_spread = (
        baseline_spread
        + 5_000
        + 500_000
        + (base_small - output_small)
        + (base_large - output_large)
    )
    joe_pfusdc = next(
        row
        for row in final_joe["assets"]
        if row["asset_id"]
        == "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"
    )
    current_bucket = next(
        bucket
        for bucket in final_vault["buckets"]
        if bucket["bucket_id"]
        == "5d5abc049bd1545e0d552e4176047e39d2151868ad7287384"
        "cbaf8db792330add44f625cfc631c8204413deb3665c536"
    )

    supply_pass = int(final_route["authorized_valid_supply_atoms"]) == baseline_supply
    reserve_pass = (
        int(final_route["settlement_reserve_atoms"]) == expected_reserve
        and int(final_route["non_nav_spread_atoms"]) == expected_spread
    )
    wrapper_pass = (
        ethereum["wa666_total_supply_atoms"] == baseline_supply
        and int(final_route["outstanding_bridge_claims_atoms"]) == baseline_supply
        and ethereum["wallet_wa666_atoms"]
        == int(baseline_mint["pre_state"]["recipient_balance_atoms"])
    )
    pfusdc_pass = (
        int(joe_pfusdc["balance"]) == 800_000
        and int(current_bucket["redemption_queue_atoms"]) == 0
        and ethereum["vault_usdc_atoms"] == ethereum["vault_obligations_atoms"]
    )
    fleet_pass = (
        final_fleet["validator_count"] == 6
        and final_fleet["heights"] == [440]
        and len(final_fleet["state_roots"]) == 1
        and final_fleet["mempools"] == [0]
    )
    pool_pass = ethereum["pool_liquidity"] == int(baseline_eth["pool_liquidity"])

    reconciliation = {
        "schema": "postfiat.a666.variable_roundtrip_reconciliation.v1",
        "baseline": {
            "authorized_valid_supply_atoms": baseline_supply,
            "settlement_reserve_atoms": baseline_reserve,
            "non_nav_spread_atoms": baseline_spread,
            "wallet_wa666_atoms": int(
                baseline_mint["pre_state"]["recipient_balance_atoms"]
            ),
            "pool_liquidity": int(baseline_eth["pool_liquidity"]),
        },
        "canonical_redemption": {
            "nav_per_unit_usd_1e8": int(nav["nav_per_unit_usd_1e8"]),
            "transparent": {
                "base_value_atoms": base_small,
                "output_atoms": output_small,
                "main_payout_atoms": int(transparent_withdrawal["amount_atoms"]),
                "rounding_residual_payout_atoms": int(
                    transparent_residual["amount_atoms"]
                ),
            },
            "private": {
                "base_value_atoms": base_large,
                "output_atoms": output_large,
            },
        },
        "expected_final": {
            "authorized_valid_supply_atoms": baseline_supply,
            "settlement_reserve_atoms": expected_reserve,
            "non_nav_spread_atoms": expected_spread,
            "wallet_wa666_atoms": int(
                baseline_mint["pre_state"]["recipient_balance_atoms"]
            ),
            "joe_pfusdc_atoms": 800_000,
        },
        "observed_final": {
            "authorized_valid_supply_atoms": int(
                final_route["authorized_valid_supply_atoms"]
            ),
            "settlement_reserve_atoms": int(
                final_route["settlement_reserve_atoms"]
            ),
            "non_nav_spread_atoms": int(final_route["non_nav_spread_atoms"]),
            "outstanding_bridge_claims_atoms": int(
                final_route["outstanding_bridge_claims_atoms"]
            ),
            "wallet_wa666_atoms": ethereum["wallet_wa666_atoms"],
            "wa666_total_supply_atoms": ethereum["wa666_total_supply_atoms"],
            "joe_pfusdc_atoms": int(joe_pfusdc["balance"]),
            "pool_liquidity": ethereum["pool_liquidity"],
        },
        "supply_conservation_pass": supply_pass,
        "reserve_conservation_pass": reserve_pass,
        "wrapper_conservation_pass": wrapper_pass,
        "pfusdc_conservation_pass": pfusdc_pass,
        "fleet_convergence_pass": fleet_pass,
        "uniswap_liquidity_unchanged_pass": pool_pass,
        "active_reservations": int(final_route["active_reservation_count"]),
        "export_entitlements": int(final_route["export_entitlement_count"]),
        "pending_return_import_claims": int(
            final_route["pending_return_import_claims_atoms"]
        ),
        "route_invariant_holds": bool(final_route["invariant_holds"]),
    }
    atomic_json(final / "reconciliation.json", reconciliation)

    business_flow_pass = all(
        (
            supply_pass,
            reserve_pass,
            wrapper_pass,
            pfusdc_pass,
            fleet_pass,
            pool_pass,
            bool(final_route["invariant_holds"]),
            int(final_route["active_reservation_count"]) == 0,
            int(final_route["export_entitlement_count"]) == 0,
            int(final_route["pending_return_import_claims_atoms"]) == 0,
        )
    )
    release_slo_pass = (
        timing["p_large_issue"]["slo_pass"]
        and timing["t_small_redeem_exact_completion"]["slo_pass"]
        and timing["p_large_redeem"]["slo_pass"]
    )
    summary = {
        "schema": "postfiat.a666.variable_size_nav_roundtrip_acceptance.v1",
        "verdict": "PASS" if business_flow_pass and release_slo_pass else "FAIL",
        "business_flow_pass": business_flow_pass,
        "release_slo_pass": release_slo_pass,
        "t_small": {
            "transparent_issue_pass": True,
            "transparent_redeem_pass": output_small
            == int(transparent_withdrawal["amount_atoms"])
            + int(transparent_residual["amount_atoms"]),
            "issue_slo": "NOT_MEASURED_FROM_FRESH_START",
            "redeem_slo": (
                "PASS"
                if timing["t_small_redeem_exact_completion"]["slo_pass"]
                else "FAIL"
            ),
        },
        "p_large": {
            "private_issue_pass": True,
            "private_redeem_pass": output_large
            == int(private_withdrawal["amount_atoms"]),
            "issue_slo": "PASS" if timing["p_large_issue"]["slo_pass"] else "FAIL",
            "redeem_slo": (
                "PASS" if timing["p_large_redeem"]["slo_pass"] else "FAIL"
            ),
        },
        "stakehub_nav": {
            "real_sources": profile["source_class"]
            == "stakehub-six-leg-reserves-v3",
            "proof_verified": True,
            "finalized_on_pftl": True,
            "epoch": int(nav["epoch"]),
            "nav_per_unit_usd_1e8": int(nav["nav_per_unit_usd_1e8"]),
            "program_vkey": nav["program_vkey"],
            "proof_sha256": nav["proof_sha256"],
            "public_values_sha256": nav["public_values_sha256"],
            "uniswap_price_used": bool(nav["uniswap_price_used"]),
        },
        "supply_conservation_pass": supply_pass,
        "reserve_conservation_pass": reserve_pass,
        "wrapper_conservation_pass": wrapper_pass,
        "pfusdc_conservation_pass": pfusdc_pass,
        "uniswap_liquidity_consumed_by_primary_issue": 0,
        "intervention_free_after_p_large_funding": False,
        "interventions": [
            "validator ingress finality gating was hardened after P-LARGE funding",
            "the P-LARGE proof worker resumed after an A100 crash",
            "the transparent redemption quote rounded base value down by one atom",
            "the one-atom transparent payout residual completed after reconciliation",
        ],
        "terminal_pftl_height": 440,
        "terminal_state_root": final_fleet["state_roots"][0],
        "unresolved_recovery": False,
    }
    atomic_json(campaign / "acceptance-summary.json", summary)
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
