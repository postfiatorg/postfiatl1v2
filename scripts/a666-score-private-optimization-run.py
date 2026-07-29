#!/usr/bin/env python3
"""Score one live private A666 issue/redemption optimization run."""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import sys
from typing import Any

from web3 import Web3


RPC = "https://ethereum-rpc.publicnode.com"
WA666 = Web3.to_checksum_address("0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5")
WALLET = Web3.to_checksum_address("0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0")
ERC20_VIEW_ABI = [
    {
        "inputs": [{"name": "account", "type": "address"}],
        "name": "balanceOf",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function",
    },
    {
        "inputs": [],
        "name": "totalSupply",
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function",
    },
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase-dir", type=Path, required=True)
    return parser.parse_args()


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain an object")
    return value


def transaction_block(document: dict[str, Any], label: str) -> int:
    rows = document.get("transactions")
    if not isinstance(rows, list):
        raise ValueError("mint state does not contain transactions")
    matches = [
        row
        for row in rows
        if isinstance(row, dict) and row.get("label") == label
    ]
    if len(matches) != 1 or not isinstance(matches[0].get("block_number"), int):
        raise ValueError(f"mint state does not contain exactly one {label!r}")
    return matches[0]["block_number"]


def fleet_status(repo: Path) -> list[dict[str, Any]]:
    path = repo / "scripts" / "a666-ce22-finality-op.py"
    spec = importlib.util.spec_from_file_location("a666_score_fleet", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load fleet status helper")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.wait_for_fleet_status(
        [28650, 28651, 28652, 28653, 28654, 28655],
        45.0,
        45.0,
    )


def main() -> None:
    args = parse_args()
    phase = args.phase_dir.resolve()
    repo = Path(__file__).resolve().parent.parent
    manifest = load(phase / "run-manifest.json")
    baseline = load(phase / "baseline" / "supply-status.json")
    deposit = load(phase / "deposit" / "deposit-result.json")
    mint = load(phase / "ethereum" / "mint-state.json")
    burn = load(phase / "return" / "ethereum-burn" / "burn.json")
    withdrawal = load(phase / "pfusdc-egress" / "withdrawal-result.json")
    issue = load(phase / "summary.json")
    redemption = load(phase / "private-roundtrip-summary.json")
    supply_check = load(phase / "roundtrip-supply-check.json")
    final_supply = load(phase / "final-pftl-supply-status.json")
    runner = load(phase / "runner-summary.json")

    w3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 60}))
    if not w3.is_connected() or w3.eth.chain_id != 1:
        raise RuntimeError("Ethereum mainnet RPC is unavailable")

    deposit_block = deposit["deposit"]["block_number"]
    mint_block = transaction_block(mint, "consume finalized A666 mint packet")
    burn_block = burn["transaction"]["block_number"]
    withdrawal_block = withdrawal["receipt_block_number"]
    for name, value in (
        ("deposit block", deposit_block),
        ("mint block", mint_block),
        ("burn block", burn_block),
        ("withdrawal block", withdrawal_block),
    ):
        if not isinstance(value, int) or value <= 0:
            raise ValueError(f"{name} must be a positive integer")

    blocks = {
        "deposit": w3.eth.get_block(deposit_block),
        "mint": w3.eth.get_block(mint_block),
        "burn": w3.eth.get_block(burn_block),
        "withdrawal": w3.eth.get_block(withdrawal_block),
    }
    issue_seconds = blocks["mint"]["timestamp"] - blocks["deposit"]["timestamp"]
    redemption_seconds = (
        blocks["withdrawal"]["timestamp"] - blocks["burn"]["timestamp"]
    )
    issue_slo = manifest["issue_slo_seconds"]
    redemption_slo = manifest["redemption_slo_seconds"]

    rows = fleet_status(repo)
    final_fleet = {
        "schema": "postfiat.a666.optimization_final_fleet.v1",
        "validator_count": len(rows),
        "height": rows[0]["block_height"],
        "block_tip_hash": rows[0]["block_tip_hash"],
        "state_root": rows[0]["state_root"],
        "mempool_pending": 0,
        "nodes": [
            {
                "node_id": row["node_id"],
                "height": row["block_height"],
                "state_root": row["state_root"],
                "mempool_pending": row["mempool_pending"],
            }
            for row in rows
        ],
    }
    final_dir = phase / "final"
    final_dir.mkdir(mode=0o755)
    (final_dir / "fleet-status.json").write_text(
        json.dumps(final_fleet, indent=2, sort_keys=True) + "\n"
    )

    token = w3.eth.contract(address=WA666, abi=ERC20_VIEW_ABI)
    wrapped_balance = token.functions.balanceOf(WALLET).call()
    wrapped_supply = token.functions.totalSupply().call()
    amounts = manifest["amounts"]
    expected_spread_delta = (
        amounts["issue_spread_atoms"]
        + amounts["issue_base_value_atoms"]
        - amounts["redemption_output_atoms"]
    )

    intervention_free = (
        runner.get("intervention_free_after_deposit") is True
        and not (phase / "intervention-log.json").exists()
        and not (phase / "run-failure.json").exists()
    )
    functional_pass = (
        issue.get("verdict") == "PASS"
        and redemption.get("verdict") == "PASS"
        and supply_check.get("verdict") == "PASS"
        and withdrawal.get("replay_rejected") is True
        and final_supply.get("invariant_holds") is True
    )
    conservation_pass = (
        final_supply["authorized_valid_supply_atoms"]
        == baseline["authorized_valid_supply_atoms"]
        and final_supply["outstanding_bridge_claims_atoms"]
        == baseline["outstanding_bridge_claims_atoms"]
        and final_supply["ethereum_spendable_supply_atoms"]
        == baseline["ethereum_spendable_supply_atoms"]
        and final_supply["settlement_reserve_atoms"]
        == baseline["settlement_reserve_atoms"]
        and final_supply["non_nav_spread_atoms"]
        == baseline["non_nav_spread_atoms"] + expected_spread_delta
        and wrapped_balance == manifest["expected_wrapped_balance_before"]
        and wrapped_supply == manifest["expected_wrapped_supply_before"]
    )
    issue_slo_pass = 0 <= issue_seconds <= issue_slo
    redemption_slo_pass = 0 <= redemption_seconds <= redemption_slo
    overall = (
        functional_pass
        and conservation_pass
        and intervention_free
        and issue_slo_pass
        and redemption_slo_pass
    )

    timing = {
        "schema": "postfiat.a666.optimization_block_timing.v1",
        "issue": {
            "start_block": deposit_block,
            "start_timestamp": blocks["deposit"]["timestamp"],
            "end_block": mint_block,
            "end_timestamp": blocks["mint"]["timestamp"],
            "seconds": issue_seconds,
            "slo_seconds": issue_slo,
            "pass": issue_slo_pass,
        },
        "redemption": {
            "start_block": burn_block,
            "start_timestamp": blocks["burn"]["timestamp"],
            "end_block": withdrawal_block,
            "end_timestamp": blocks["withdrawal"]["timestamp"],
            "seconds": redemption_seconds,
            "slo_seconds": redemption_slo,
            "pass": redemption_slo_pass,
        },
    }
    (phase / "block-timing.json").write_text(
        json.dumps(timing, indent=2, sort_keys=True) + "\n"
    )
    score = {
        "schema": "postfiat.a666.private_optimization_score.v1",
        "verdict": "PASS" if overall else "FAIL",
        "functional_pass": functional_pass,
        "conservation_pass": conservation_pass,
        "intervention_free_after_deposit": intervention_free,
        "issue_seconds": issue_seconds,
        "issue_slo_seconds": issue_slo,
        "issue_slo_pass": issue_slo_pass,
        "redemption_seconds": redemption_seconds,
        "redemption_slo_seconds": redemption_slo,
        "redemption_slo_pass": redemption_slo_pass,
        "six_validator_convergence_pass": len(rows) == 6,
        "final_height": rows[0]["block_height"],
        "final_state_root": rows[0]["state_root"],
        "final_wrapped_balance_atoms": wrapped_balance,
        "final_wrapped_supply_atoms": wrapped_supply,
        "uniswap_liquidity_consumed_atoms": issue["uniswap_eligibility"][
            "liquidity_consumed"
        ],
    }
    (phase / "optimization-score.json").write_text(
        json.dumps(score, indent=2, sort_keys=True) + "\n"
    )
    print(json.dumps(score, indent=2, sort_keys=True))
    if not overall:
        raise RuntimeError("optimization score did not pass every gate")


if __name__ == "__main__":
    try:
        main()
    except (OSError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"a666-score-private-optimization-run: {error}", file=sys.stderr)
        raise SystemExit(1) from error
