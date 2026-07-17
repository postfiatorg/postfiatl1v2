#!/usr/bin/env python3
"""Execute Gate 5 optimistic verifier fork evidence for PFTL-Uniswap."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
GATE3_PATH = ROOT / "scripts" / "pftl-uniswap-gate3-fork-execute.py"
DEFAULT_OUT_DIR = ROOT / "docs" / "evidence" / "pftl-uniswap-gate5-optimistic-fork-2026-07-01"

OPTIMISTIC_ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-gate5-optimistic-v1"
OPTIMISTIC_TRUST_CLASS = "OPTIMISTIC"
OPTIMISTIC_VERIFIER_MODE = "optimistic"
OPTIMISTIC_SWAP_DATA = b"postfiat-gate5-optimistic-wa666-usdc-v4-exact-input"
POSTER_BOND_WEI = 1_000_000_000_000_000_000
CHALLENGER_BOND_WEI = 1_000_000_000_000_000_000
ETHEREUM_FINALITY_BLOCKS = 64
ETHEREUM_SECONDS_PER_BLOCK = 12
PROOF_SUBMISSION_MARGIN_SECONDS = 900
CHALLENGE_WINDOW_SECONDS = ETHEREUM_FINALITY_BLOCKS * ETHEREUM_SECONDS_PER_BLOCK + PROOF_SUBMISSION_MARGIN_SECONDS
CHALLENGE_RESOLUTION_WINDOW_SECONDS = 900
CHALLENGE_EVIDENCE_HASH = Web3.keccak(text="postfiat.gate5.optimistic.challenge.evidence.v1")


def load_gate3_module():
    spec = importlib.util.spec_from_file_location("pftl_uniswap_gate3_fork_execute", GATE3_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load {GATE3_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.ROUTE_ID = OPTIMISTIC_ROUTE_ID
    module.TRUST_CLASS = OPTIMISTIC_TRUST_CLASS
    module.VERIFIER_MODE = OPTIMISTIC_VERIFIER_MODE
    module.SWAP_DATA = OPTIMISTIC_SWAP_DATA
    return module


gate3 = load_gate3_module()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", default=str(DEFAULT_OUT_DIR), help="evidence output directory")
    parser.add_argument(
        "--fork-rpc-url",
        default=None,
        help="upstream Ethereum RPC; defaults to ETHEREUM_RPC_URL, MAINNET_RPC_URL, or publicnode",
    )
    parser.add_argument("--replace", action="store_true", help="replace an existing evidence directory")
    parser.add_argument("--keep-anvil", action="store_true", help="leave the fork RPC process running after success")
    return parser.parse_args()


def transact_value(web3: Web3, fn: Any, sender: str, value: int = 0, *, timeout: int = 240):
    tx_hash = fn.transact({"from": sender, "value": value})
    receipt = web3.eth.wait_for_transaction_receipt(tx_hash, timeout=timeout)
    if receipt.status != 1:
        raise RuntimeError(f"transaction reverted: {tx_hash.hex()}")
    return receipt


def advance_time(web3: Web3, seconds: int) -> None:
    web3.provider.make_request("evm_increaseTime", [seconds])
    web3.provider.make_request("evm_mine", [])


def receipt_cost_wei(receipt: Any, fallback_gas_price: int) -> int:
    gas_price = int(receipt.get("effectiveGasPrice") or fallback_gas_price)
    return int(receipt.gasUsed) * gas_price


def canonical_json(value: Any) -> str:
    if isinstance(value, list):
        return "[" + ",".join(canonical_json(item) for item in value) + "]"
    if isinstance(value, dict):
        return "{" + ",".join(json.dumps(key) + ":" + canonical_json(value[key]) for key in sorted(value)) + "}"
    return json.dumps(value)


def digest_binding(value: dict[str, Any]) -> str:
    without_digest = json.loads(json.dumps(value))
    without_digest.pop("binding_digest", None)
    return hashlib.sha3_384(canonical_json(without_digest).encode("utf-8")).hexdigest()


def claim_is_accepted(verifier: Any, packet: tuple[Any, ...], digest: bytes) -> bool:
    return verifier.functions.isReceiptAccepted(
        packet[3],
        packet[2],
        packet[0],
        Web3.keccak(text=OPTIMISTIC_TRUST_CLASS),
        digest,
    ).call()


def optimistic_accept_packet(
    *,
    web3: Web3,
    verifier: Any,
    controller: Any,
    packet: tuple[Any, ...],
    poster: str,
    preflight_consume: Any,
    label: str,
) -> dict[str, Any]:
    digest = controller.functions.packetDigest(packet).call()
    accepted_before = claim_is_accepted(verifier, packet, digest)
    gate3.expect_revert(preflight_consume, poster)
    post_receipt = transact_value(
        web3,
        verifier.functions.postReceiptClaim(packet[3], packet[2], packet[0], digest),
        poster,
        POSTER_BOND_WEI,
    )
    claim_id = verifier.functions.receiptClaimId(packet[3], packet[2], packet[0], digest).call()
    accepted_during_window = claim_is_accepted(verifier, packet, digest)
    gate3.expect_revert(preflight_consume, poster)
    advance_time(web3, CHALLENGE_WINDOW_SECONDS + 1)
    finalize_receipt = transact_value(web3, verifier.functions.finalizeReceiptClaim(claim_id), poster)
    accepted_after = claim_is_accepted(verifier, packet, digest)
    if not accepted_after:
        raise RuntimeError(f"{label} optimistic claim did not become accepted")
    return {
        "label": label,
        "packet_digest": gate3.tx_hex(digest),
        "claim_id": gate3.tx_hex(claim_id),
        "accepted_before_post": accepted_before,
        "accepted_during_challenge_window": accepted_during_window,
        "accepted_after_finalize": accepted_after,
        "post_tx": gate3.tx_hex(post_receipt.transactionHash),
        "post_gas_used": int(post_receipt.gasUsed),
        "finalize_tx": gate3.tx_hex(finalize_receipt.transactionHash),
        "finalize_gas_used": int(finalize_receipt.gasUsed),
    }


def exercise_valid_challenge(
    *,
    web3: Web3,
    verifier: Any,
    controller: Any,
    packet: tuple[Any, ...],
    poster: str,
    challenger: str,
) -> dict[str, Any]:
    digest = controller.functions.packetDigest(packet).call()
    post_receipt = transact_value(
        web3,
        verifier.functions.postReceiptClaim(packet[3], packet[2], packet[0], digest),
        poster,
        POSTER_BOND_WEI,
    )
    claim_id = verifier.functions.receiptClaimId(packet[3], packet[2], packet[0], digest).call()
    challenge_receipt = transact_value(
        web3,
        verifier.functions.challengeReceiptClaim(
            claim_id,
            1,  # ChallengeFault.InvalidReceiptHash
            CHALLENGE_EVIDENCE_HASH,
        ),
        challenger,
        CHALLENGER_BOND_WEI,
    )
    accepted_after_challenge = claim_is_accepted(verifier, packet, digest)
    gate3.expect_revert(controller.functions.consumeMintOnly(packet), poster)
    resolve_receipt = transact_value(web3, verifier.functions.resolveReceiptChallenge(claim_id, True), poster)
    accepted_after_resolve = claim_is_accepted(verifier, packet, digest)
    gate3.expect_revert(controller.functions.consumeMintOnly(packet), poster)
    gas_price = int(web3.eth.gas_price)
    return {
        "schema": "postfiat-pftl-uniswap-gate5-optimistic-challenge-evidence-v1",
        "claim_id": gate3.tx_hex(claim_id),
        "packet_digest": gate3.tx_hex(digest),
        "challenge_fault": "InvalidReceiptHash",
        "challenge_evidence_hash": gate3.tx_hex(CHALLENGE_EVIDENCE_HASH),
        "post_tx": gate3.tx_hex(post_receipt.transactionHash),
        "challenge_tx": gate3.tx_hex(challenge_receipt.transactionHash),
        "resolve_tx": gate3.tx_hex(resolve_receipt.transactionHash),
        "accepted_after_challenge": accepted_after_challenge,
        "accepted_after_valid_challenge_resolution": accepted_after_resolve,
        "consume_after_valid_challenge_rejected": True,
        "challenge_gas_used": int(challenge_receipt.gasUsed),
        "challenge_gas_cost_wei": receipt_cost_wei(challenge_receipt, gas_price),
        "challenge_gas_cost_with_4x_margin_wei": receipt_cost_wei(challenge_receipt, gas_price) * 4,
        "resolve_gas_used": int(resolve_receipt.gasUsed),
    }


def write_readme(out_dir: Path, summary: dict[str, Any]) -> None:
    text = f"""# PFTL-Uniswap Gate 5 Optimistic Fork Evidence

Generated by:

```bash
python3 scripts/pftl-uniswap-gate5-optimistic-fork-execute.py --replace
```

What this evidence proves:

- an `OPTIMISTIC` route was initialized from the same PFTL/Uniswap handoff model;
- `OptimisticPFTLReceiptVerifier` was deployed on an Ethereum fork with bonded
  posting, bonded challenge, challenge window, and challenge-resolution window;
- seed, mint-only, and mint-and-swap packets were rejected before optimistic
  claim finalization, then accepted and consumed after finalization;
- the fork still seeded the official Uniswap v4 pool, executed external buy and
  sell transactions, and preserved canonical supply across AMM trades;
- a challenged claim stayed unaccepted, failed settlement preflight, and stayed
  rejected after valid challenge resolution;
- challenge gas was measured on the fork and recorded for Gate 5 parameter
  calibration.

Key outputs:

- route id: `{summary["route_id"]}`
- route trust class: `{summary["route_trust_class"]}`
- route config digest: `{summary["route_config_digest"]}`
- launch config digest: `{summary["launch_config_digest"]}`
- pool id: `{summary["pool_id"]}`
- seed mint tx: `{summary["txs"]["seed_mint"]}`
- external buy tx: `{summary["txs"]["external_buy"]}`
- external sell tx: `{summary["txs"]["external_sell"]}`
- mint-only packet tx: `{summary["txs"]["mint_only_packet"]}`
- mint-and-swap packet tx: `{summary["txs"]["mint_and_swap_packet"]}`
- challenge tx: `{summary["optimistic_challenge"]["challenge_tx"]}`
- challenge gas with 4x margin wei: `{summary["optimistic_challenge"]["challenge_gas_cost_with_4x_margin_wei"]}`
- optimistic launch binding digest: `{summary["optimistic_launch_binding_digest"]}`

This is fork evidence for the optimistic path. It is not public-route approval
and does not enable uncapped or trustless routing.
"""
    (out_dir / "README.md").write_text(text, encoding="utf-8")


def main() -> int:
    args = parse_args()
    fork_rpc_url = (
        args.fork_rpc_url
        or os.environ.get("ETHEREUM_RPC_URL")
        or os.environ.get("MAINNET_RPC_URL")
        or "https://ethereum-rpc.publicnode.com"
    )
    out_dir = Path(args.out_dir).resolve()
    if out_dir.exists():
        if not args.replace:
            raise RuntimeError(f"evidence directory already exists: {out_dir}; pass --replace")
        shutil.rmtree(out_dir)
    (out_dir / "inputs").mkdir(parents=True)
    (out_dir / "reports").mkdir(parents=True)
    (out_dir / "sidecar").mkdir(parents=True)

    gate3.run(["cargo", "build", "-p", "postfiat-node"], stdout_file=out_dir / "reports" / "00-cargo-build-postfiat-node.txt")
    gate3.run(["forge", "build"], cwd=gate3.ETH_CONTRACTS, stdout_file=out_dir / "reports" / "00-forge-build.txt")

    rpc_url, proc = gate3.start_anvil(fork_rpc_url)
    keep_anvil = args.keep_anvil
    try:
        web3 = Web3(Web3.HTTPProvider(rpc_url, request_kwargs={"timeout": 120}))
        gate3.wait_for_rpc(web3)
        if web3.eth.chain_id != 1:
            raise RuntimeError(f"fork chain id must be 1, got {web3.eth.chain_id}")
        owner = Web3.to_checksum_address(web3.eth.accounts[0])
        external_user = Web3.to_checksum_address(web3.eth.accounts[1])
        challenger = Web3.to_checksum_address(web3.eth.accounts[2])
        start_nonce = web3.eth.get_transaction_count(owner)
        addresses = {
            "owner": owner,
            "external_user": external_user,
            "challenger": challenger,
            "wrapped": gate3.compute_create_address(owner, start_nonce),
            "verifier": gate3.compute_create_address(owner, start_nonce + 1),
            "replay_registry": gate3.compute_create_address(owner, start_nonce + 2),
            "v4_router": gate3.compute_create_address(owner, start_nonce + 3),
            "adapter": gate3.compute_create_address(owner, start_nonce + 4),
            "helper": gate3.compute_create_address(owner, start_nonce + 5),
            "controller": gate3.compute_create_address(owner, start_nonce + 6),
        }
        pool_id_hex = gate3.pool_id(web3, addresses["wrapped"], gate3.USDC)
        sidecar = gate3.run_sidecar(out_dir, addresses, pool_id_hex)
        route_digest = sidecar["route_init"]["route_config_digest"]

        gate3.set_usdc_balance(web3, owner, 1_000_000_000_000)
        gate3.set_usdc_balance(web3, external_user, 1_000_000_000_000)
        usdc = gate3.contract_at(web3, "PFTLUniswapV4PoolHarness.sol", "IERC20V4Harness", gate3.USDC)

        wrapped, _ = gate3.deploy(
            web3, "PFTLUniswapHandoffController.sol", "WrappedVenueNAVCoin", owner, "Wrapped A666", "wA666", 6, owner
        )
        verifier, _ = gate3.deploy(
            web3,
            "PFTLUniswapHandoffController.sol",
            "OptimisticPFTLReceiptVerifier",
            owner,
            owner,
            POSTER_BOND_WEI,
            CHALLENGER_BOND_WEI,
            CHALLENGE_WINDOW_SECONDS,
            CHALLENGE_RESOLUTION_WINDOW_SECONDS,
        )
        replay_registry, _ = gate3.deploy(web3, "PFTLUniswapHandoffController.sol", "PacketReplayRegistry", owner, owner)
        v4_router, _ = gate3.deploy(web3, "PFTLUniswapV4PoolHarness.sol", "PFTLUniswapV4ExactInputRouter", owner, gate3.POOL_MANAGER)
        adapter, _ = gate3.deploy(
            web3,
            "PFTLUniswapHandoffController.sol",
            "UniswapSettlementAdapter",
            owner,
            v4_router.address,
            wrapped.address,
            Web3.to_checksum_address(gate3.USDC),
            bytes.fromhex(pool_id_hex.removeprefix("0x")),
            Web3.keccak(OPTIMISTIC_SWAP_DATA),
            owner,
        )
        helper, _ = gate3.deploy(
            web3,
            "PFTLUniswapV4PoolHarness.sol",
            "PFTLUniswapV4LaunchHelper",
            owner,
            owner,
            gate3.POOL_MANAGER,
            gate3.POSITION_MANAGER,
            gate3.PERMIT2,
        )
        route_config_tuple = (
            owner,
            1,
            gate3.pftl_bytes(route_digest),
            Web3.keccak(text=OPTIMISTIC_TRUST_CLASS),
            gate3.pftl_bytes(gate3.SETTLEMENT_ASSET_ID),
            gate3.pftl_bytes(gate3.NATIVE_NAV_ASSET_ID),
            gate3.pftl_bytes(gate3.PRICING_RESERVE_PACKET_HASH),
            gate3.LATEST_FINALIZED_NAV_EPOCH,
            bytes.fromhex(pool_id_hex.removeprefix("0x")),
            gate3.ROUTE_SUPPLY_CAP_ATOMS,
            gate3.PACKET_NOTIONAL_CAP_ATOMS,
            replay_registry.address,
        )
        controller, _ = gate3.deploy(
            web3,
            "PFTLUniswapHandoffController.sol",
            "PFTLUniswapHandoffController",
            owner,
            wrapped.address,
            adapter.address,
            verifier.address,
            route_config_tuple,
        )

        expected = {key: Web3.to_checksum_address(value) for key, value in addresses.items() if key not in {"owner", "external_user", "challenger"}}
        actual = {
            "wrapped": wrapped.address,
            "verifier": verifier.address,
            "replay_registry": replay_registry.address,
            "v4_router": v4_router.address,
            "adapter": adapter.address,
            "helper": helper.address,
            "controller": controller.address,
        }
        if actual != expected:
            raise RuntimeError(f"precomputed deployment addresses drifted: expected={expected}, actual={actual}")

        gate3.transact(web3, wrapped.functions.setController(controller.address), owner)
        gate3.transact(web3, wrapped.functions.lockController(), owner)
        gate3.transact(web3, replay_registry.functions.setControllerAuthorization(controller.address, True), owner)
        gate3.transact(web3, adapter.functions.setController(controller.address), owner)
        gate3.transact(web3, adapter.functions.lockController(), owner)
        gate3.transact(web3, usdc.functions.transfer(helper.address, gate3.SEED_USDC_ATOMS), owner)

        receipt_root = sidecar["replay_report"]["receipt_root"]
        optimistic_claims: list[dict[str, Any]] = []
        seed_packet = gate3.packet_tuple(
            route_digest=route_digest,
            export_report=sidecar["export_reports"]["seed"],
            receipt_root=receipt_root,
            controller=controller.address,
            wrapped=wrapped.address,
            recipient=helper.address,
            pool_id_hex=pool_id_hex,
            settlement_atoms=gate3.SEED_USDC_ATOMS,
            mint_atoms=gate3.SEED_WRAPPED_ATOMS,
            minimum_output_atoms=1,
            swap_path_hash=bytes(32),
        )
        optimistic_claims.append(
            optimistic_accept_packet(
                web3=web3,
                verifier=verifier,
                controller=controller,
                packet=seed_packet,
                poster=owner,
                preflight_consume=controller.functions.consumeMintOnly(seed_packet),
                label="seed",
            )
        )
        seed_mint_receipt = gate3.transact(web3, controller.functions.consumeMintOnly(seed_packet), owner)
        seed_lp_receipt = gate3.transact(
            web3,
            helper.functions.initializeAndSeed(wrapped.address, Web3.to_checksum_address(gate3.USDC), gate3.SEED_WRAPPED_ATOMS, gate3.SEED_USDC_ATOMS),
            owner,
            timeout=900,
        )

        supply_before_external = wrapped.functions.totalSupply().call()
        gate3.transact(web3, usdc.functions.approve(v4_router.address, gate3.EXTERNAL_BUY_USDC_ATOMS), external_user)
        buy_usdc_before = usdc.functions.balanceOf(external_user).call()
        buy_wrapped_before = wrapped.functions.balanceOf(external_user).call()
        external_buy_receipt = gate3.transact(
            web3,
            v4_router.functions.exactInput(
                Web3.to_checksum_address(gate3.USDC),
                wrapped.address,
                gate3.EXTERNAL_BUY_USDC_ATOMS,
                1,
                external_user,
                gate3.DESTINATION_DEADLINE_SECONDS,
                b"",
            ),
            external_user,
            timeout=900,
        )
        buy_usdc_after = usdc.functions.balanceOf(external_user).call()
        buy_wrapped_after = wrapped.functions.balanceOf(external_user).call()
        sell_wrapped_spent = max(1, (buy_wrapped_after - buy_wrapped_before) // 2)
        gate3.transact(web3, wrapped.functions.approve(v4_router.address, sell_wrapped_spent), external_user)
        sell_wrapped_before = wrapped.functions.balanceOf(external_user).call()
        sell_usdc_before = usdc.functions.balanceOf(external_user).call()
        external_sell_receipt = gate3.transact(
            web3,
            v4_router.functions.exactInput(
                wrapped.address,
                Web3.to_checksum_address(gate3.USDC),
                sell_wrapped_spent,
                1,
                external_user,
                gate3.DESTINATION_DEADLINE_SECONDS,
                b"",
            ),
            external_user,
            timeout=900,
        )
        sell_wrapped_after = wrapped.functions.balanceOf(external_user).call()
        sell_usdc_after = usdc.functions.balanceOf(external_user).call()
        supply_after_external = wrapped.functions.totalSupply().call()

        mint_only_packet = gate3.packet_tuple(
            route_digest=route_digest,
            export_report=sidecar["export_reports"]["mint-only"],
            receipt_root=receipt_root,
            controller=controller.address,
            wrapped=wrapped.address,
            recipient=external_user,
            pool_id_hex=pool_id_hex,
            settlement_atoms=gate3.MINT_ONLY_ATOMS * gate3.NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
            mint_atoms=gate3.MINT_ONLY_ATOMS,
            minimum_output_atoms=1,
            swap_path_hash=bytes(32),
        )
        optimistic_claims.append(
            optimistic_accept_packet(
                web3=web3,
                verifier=verifier,
                controller=controller,
                packet=mint_only_packet,
                poster=owner,
                preflight_consume=controller.functions.consumeMintOnly(mint_only_packet),
                label="mint-only",
            )
        )
        mint_only_receipt = gate3.transact(web3, controller.functions.consumeMintOnly(mint_only_packet), owner)

        mint_swap_packet = gate3.packet_tuple(
            route_digest=route_digest,
            export_report=sidecar["export_reports"]["mint-and-swap"],
            receipt_root=receipt_root,
            controller=controller.address,
            wrapped=wrapped.address,
            recipient=external_user,
            pool_id_hex=pool_id_hex,
            settlement_atoms=gate3.MINT_AND_SWAP_ATOMS * gate3.NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
            mint_atoms=gate3.MINT_AND_SWAP_ATOMS,
            minimum_output_atoms=1,
            swap_path_hash=Web3.keccak(OPTIMISTIC_SWAP_DATA),
        )
        optimistic_claims.append(
            optimistic_accept_packet(
                web3=web3,
                verifier=verifier,
                controller=controller,
                packet=mint_swap_packet,
                poster=owner,
                preflight_consume=controller.functions.consumeMintAndSwap(mint_swap_packet, OPTIMISTIC_SWAP_DATA),
                label="mint-and-swap",
            )
        )
        mint_swap_receipt = gate3.transact(web3, controller.functions.consumeMintAndSwap(mint_swap_packet, OPTIMISTIC_SWAP_DATA), owner, timeout=900)

        challenge_packet = list(mint_swap_packet)
        challenge_packet[1] = gate3.pftl_bytes("d1" * 48)
        challenge_packet[2] = gate3.pftl_bytes("d2" * 48)
        challenge_packet[20] = bytes.fromhex("d3" * 32)
        challenge_packet = tuple(challenge_packet)
        optimistic_challenge = exercise_valid_challenge(
            web3=web3,
            verifier=verifier,
            controller=controller,
            packet=challenge_packet,
            poster=owner,
            challenger=challenger,
        )

        failure_packet = list(mint_swap_packet)
        failure_packet[1] = gate3.pftl_bytes("e1" * 48)
        failure_packet[2] = gate3.pftl_bytes("e2" * 48)
        failure_packet[17] = 2**63
        failure_packet[20] = bytes.fromhex("e3" * 32)
        failure_packet = tuple(failure_packet)
        optimistic_accept_packet(
            web3=web3,
            verifier=verifier,
            controller=controller,
            packet=failure_packet,
            poster=owner,
            preflight_consume=controller.functions.consumeMintAndSwap(failure_packet, OPTIMISTIC_SWAP_DATA),
            label="min-output-failure",
        )
        failure_digest = controller.functions.packetDigest(failure_packet).call()
        gate3.expect_revert(controller.functions.consumeMintAndSwap(failure_packet, OPTIMISTIC_SWAP_DATA), owner)
        if controller.functions.consumed_packet(failure_digest).call():
            raise RuntimeError("min-output failure consumed packet")

        txs = {
            "seed_mint": gate3.tx_hex(seed_mint_receipt.transactionHash),
            "seed_lp": gate3.tx_hex(seed_lp_receipt.transactionHash),
            "external_buy": gate3.tx_hex(external_buy_receipt.transactionHash),
            "external_sell": gate3.tx_hex(external_sell_receipt.transactionHash),
            "mint_only_packet": gate3.tx_hex(mint_only_receipt.transactionHash),
            "mint_and_swap_packet": gate3.tx_hex(mint_swap_receipt.transactionHash),
        }
        deltas = {
            "buy_usdc_spent": buy_usdc_before - buy_usdc_after,
            "buy_wrapped_received": buy_wrapped_after - buy_wrapped_before,
            "sell_wrapped_spent": sell_wrapped_before - sell_wrapped_after,
            "sell_usdc_received": sell_usdc_after - sell_usdc_before,
        }
        supply = {"before_external": supply_before_external, "after_external": supply_after_external}
        if any(value <= 0 for value in deltas.values()):
            raise RuntimeError(f"external trade deltas must be nonzero: {deltas}")
        if supply_before_external != gate3.SEED_WRAPPED_ATOMS or supply_after_external != supply_before_external:
            raise RuntimeError(f"canonical supply changed across external trades: {supply}")

        gate3.write_json(out_dir / "reports" / "13-optimistic-claims.json", {
            "schema": "postfiat-pftl-uniswap-gate5-optimistic-claims-evidence-v1",
            "route_id": OPTIMISTIC_ROUTE_ID,
            "route_trust_class": OPTIMISTIC_TRUST_CLASS,
            "claims": optimistic_claims,
        })
        gate3.write_json(out_dir / "reports" / "14-optimistic-challenge.json", optimistic_challenge)
        calibration = {
            "schema": "postfiat-pftl-uniswap-gate5-optimistic-calibration-v1",
            "status": "fork_measured_parameters_not_public_launch_signoff",
            "route_id": OPTIMISTIC_ROUTE_ID,
            "packet_notional_cap": {"value": str(gate3.PACKET_NOTIONAL_CAP_ATOMS), "unit": "settlement_atoms"},
            "invalid_profit_bound": {"value": str(gate3.PACKET_NOTIONAL_CAP_ATOMS), "unit": "settlement_atoms"},
            "poster_bond_wei": str(POSTER_BOND_WEI),
            "challenger_bond_wei": str(CHALLENGER_BOND_WEI),
            "challenge_gas_cost_with_margin_wei": str(optimistic_challenge["challenge_gas_cost_with_4x_margin_wei"]),
            "griefing_cost_bound": {
                "value": str(CHALLENGER_BOND_WEI),
                "unit": "wei",
                "rationale": "Challenges must escrow the challenger bond; unresolved challenges fail closed and do not pay the challenger bond to the poster.",
            },
            "policy_floor": {"value": str(POSTER_BOND_WEI), "unit": "wei"},
            "destination_finality": {"value": str(ETHEREUM_FINALITY_BLOCKS), "unit": "ethereum_blocks"},
            "proof_submission_margin": {"value": str(PROOF_SUBMISSION_MARGIN_SECONDS), "unit": "seconds"},
            "challenge_window_seconds": CHALLENGE_WINDOW_SECONDS,
            "challenge_resolution_window_seconds": CHALLENGE_RESOLUTION_WINDOW_SECONDS,
            "watcher_liveness_slo": "detect_and_submit_valid_challenge_before_challenge_resolution_deadline",
            "fail_closed": [
                "Route remains disabled unless deployed verifier constructor values match this calibration artifact.",
                "Route remains capped and OPTIMISTIC; this evidence is not a TRUSTLESS_FINALITY claim.",
            ],
        }
        gate3.write_json(out_dir / "reports" / "15-gate5-parameters-calibration.json", calibration)

        evidence = gate3.collect_and_record_evidence(
            out_dir,
            rpc_url,
            sidecar,
            txs,
            deltas,
            supply,
            rehearsal_id="gate5-optimistic-fork-2026-07-01",
        )
        binding = {
            "schema": "postfiat-pftl-uniswap-gate5-optimistic-launch-binding-v1",
            "status": "fork_evidence_binding_public_routing_disabled",
            "route_id": OPTIMISTIC_ROUTE_ID,
            "route_config_digest": route_digest,
            "launch_config_digest": sidecar["launch_report"]["launch_config_digest"],
            "route_trust_class": OPTIMISTIC_TRUST_CLASS,
            "verifier_mode": OPTIMISTIC_VERIFIER_MODE,
            "verifier": verifier.address.lower(),
            "controller": controller.address.lower(),
            "replay_registry": replay_registry.address.lower(),
            "poster_bond_wei": str(POSTER_BOND_WEI),
            "challenger_bond_wei": str(CHALLENGER_BOND_WEI),
            "challenge_window_seconds": CHALLENGE_WINDOW_SECONDS,
            "challenge_resolution_window_seconds": CHALLENGE_RESOLUTION_WINDOW_SECONDS,
            "challenge_resolution_mode": "owner_arbitrated",
            "challenge_resolver": verifier.functions.challenge_resolver().call().lower(),
            "resolver_owner": verifier.functions.owner().call().lower(),
            "challenge_gas_cost_with_4x_margin_wei": str(
                optimistic_challenge["challenge_gas_cost_with_4x_margin_wei"]
            ),
            "parameter_calibration_report": "reports/15-gate5-parameters-calibration.json",
            "fork_summary_report": "reports/16-summary.json",
            "public_routing_enabled": False,
            "trustless_claim_allowed": False,
            "binding_digest": "",
        }
        binding["binding_digest"] = digest_binding(binding)
        gate3.write_json(out_dir / "reports" / "17-optimistic-launch-config-binding.json", binding)
        summary = {
            "route_id": OPTIMISTIC_ROUTE_ID,
            "route_trust_class": OPTIMISTIC_TRUST_CLASS,
            "route_config_digest": route_digest,
            "launch_config_digest": sidecar["launch_report"]["launch_config_digest"],
            "pool_id": pool_id_hex,
            "addresses": addresses,
            "optimistic_verifier": {
                "poster_bond_wei": str(POSTER_BOND_WEI),
                "challenger_bond_wei": str(CHALLENGER_BOND_WEI),
                "challenge_window_seconds": CHALLENGE_WINDOW_SECONDS,
                "challenge_resolution_window_seconds": CHALLENGE_RESOLUTION_WINDOW_SECONDS,
                "challenge_resolution_mode": "owner_arbitrated",
                "challenge_resolver": verifier.functions.challenge_resolver().call().lower(),
                "resolver_owner": verifier.functions.owner().call().lower(),
            },
            "txs": txs,
            "deltas": deltas,
            "supply": supply,
            "optimistic_claims_report": "reports/13-optimistic-claims.json",
            "optimistic_challenge": optimistic_challenge,
            "calibration_report": "reports/15-gate5-parameters-calibration.json",
            "optimistic_launch_binding_report": "reports/17-optimistic-launch-config-binding.json",
            "optimistic_launch_binding_digest": binding["binding_digest"],
            "collector": evidence["collector"],
            "record": evidence["record"],
        }
        gate3.write_json(out_dir / "reports" / "16-summary.json", summary)
        write_readme(out_dir, summary)
        print(json.dumps(summary, indent=2))
        return 0
    finally:
        if keep_anvil:
            print(f"kept anvil running at {rpc_url}", file=sys.stderr)
        else:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
