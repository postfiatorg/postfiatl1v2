#!/usr/bin/env python3
"""Execute the controlled Gate 4 PFTL-Uniswap return-path rehearsal.

The rehearsal builds the PFTL sidecar ledger, deploys the controlled Ethereum
bridge contracts on a mainnet fork, consumes two PFTL export packets into
wA666, burns both wrapped balances for return, and imports both burns back into
PFTL through the existing node CLI. It writes checked evidence for the Gate 4
requirement: two round trips without manual ledger edits.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
GATE3_EXECUTOR = ROOT / "scripts" / "pftl-uniswap-gate3-fork-execute.py"
DEFAULT_OUT_DIR = ROOT / "docs" / "evidence" / "pftl-uniswap-gate4-2026-07-01"

ROUTE_ID = os.environ.get("PFTL_UNISWAP_RETURN_ROUTE_ID", "pftl-a666-ethereum-wA666-usdc-gate4-v1")
ROUNDTRIPS = [
    {
        "label": "roundtrip-1",
        "packet_hash": "d1" * 48,
        "export_nonce": "e1" * 32,
        "return_nonce": "f1" * 32,
        "amount_atoms": 25,
        "source_height": 31,
        "refund_not_before_height": 41,
    },
    {
        "label": "roundtrip-2",
        "packet_hash": "d2" * 48,
        "export_nonce": "e2" * 32,
        "return_nonce": "f2" * 32,
        "amount_atoms": 17,
        "source_height": 32,
        "refund_not_before_height": 42,
    },
]
TOTAL_ROUNDTRIP_ATOMS = sum(item["amount_atoms"] for item in ROUNDTRIPS)
def load_gate3_module():
    spec = importlib.util.spec_from_file_location("pftl_uniswap_gate3_executor", GATE3_EXECUTOR)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load Gate 3 executor from {GATE3_EXECUTOR}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


gate3 = load_gate3_module()
PRIMARY_SETTLEMENT_ATOMS = TOTAL_ROUNDTRIP_ATOMS * gate3.NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM


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


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def no_prefix_hex(value: Any) -> str:
    if isinstance(value, str):
        text = value
    else:
        text = Web3.to_hex(value)
    return text.removeprefix("0x").lower()


def prefixed_hex(value: Any) -> str:
    return "0x" + no_prefix_hex(value)


def node_json(cmd: list[str], output_file: Path) -> dict[str, Any]:
    return gate3.node_json(cmd, output_file)


def run_sidecar(out_dir: Path, addresses: dict[str, str], pool_id_hex: str) -> dict[str, Any]:
    sidecar_dir = out_dir / "sidecar"
    inputs_dir = out_dir / "inputs"
    reports_dir = out_dir / "reports"
    route_config_file = inputs_dir / "route-config.json"

    route_config = {
        "schema": "postfiat-pftl-uniswap-route-config-v1",
        "route_id": ROUTE_ID,
        "route_family": "primary_pftl_mint",
        "native_nav_asset_id": gate3.NATIVE_NAV_ASSET_ID,
        "settlement_asset_id": gate3.SETTLEMENT_ASSET_ID,
        "wrapped_navcoin_token": addresses["wrapped"].lower(),
        "handoff_controller": addresses["controller"].lower(),
        "settlement_adapter": addresses["adapter"].lower(),
        "verifier_mode": "threshold-controlled",
        "route_trust_class": gate3.TRUST_CLASS,
        "uniswap_pool_id_or_path": pool_id_hex.lower(),
        "router": addresses["v4_router"].lower(),
        "failure_behavior": "refund_unconsumed_pftl_packet",
        "route_supply_cap_atoms": gate3.ROUTE_SUPPLY_CAP_ATOMS,
        "packet_notional_cap_atoms": gate3.PACKET_NOTIONAL_CAP_ATOMS,
        "seed_nav_epoch": gate3.LATEST_FINALIZED_NAV_EPOCH,
        "seed_usdc_atoms": gate3.SEED_USDC_ATOMS,
        "seed_wrapped_navcoin_atoms": TOTAL_ROUNDTRIP_ATOMS,
        "lp_recipient": addresses["owner"].lower(),
        "lp_custody_policy": "controlled_gate4_return_rehearsal_no_lp",
    }
    write_json(route_config_file, route_config)

    route_init = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-route-init",
            "--data-dir",
            str(sidecar_dir),
            "--config-file",
            str(route_config_file),
            "--ethereum-chain-id",
            "1",
            "--latest-finalized-nav-epoch",
            str(gate3.LATEST_FINALIZED_NAV_EPOCH),
            "--return-finality-blocks",
            str(gate3.RETURN_FINALITY_BLOCKS),
            "--replace",
        ],
        reports_dir / "01-route-init.json",
    )

    primary_request = {
        "route_id": ROUTE_ID,
        "source_wallet": gate3.SOURCE_WALLET,
        "settlement_asset_id": gate3.SETTLEMENT_ASSET_ID,
        "subscription_nonce": "74" * 32,
        "quote": {
            "settlement_value_atoms": PRIMARY_SETTLEMENT_ATOMS,
            "nav_price_settlement_atoms_per_nav_atom": gate3.NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
            "pricing_nav_epoch": gate3.LATEST_FINALIZED_NAV_EPOCH,
            "pricing_reserve_packet_hash": gate3.PRICING_RESERVE_PACKET_HASH,
        },
    }
    primary_file = inputs_dir / "primary-subscription.json"
    write_json(primary_file, primary_request)
    primary_report = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-primary-subscribe",
            "--data-dir",
            str(sidecar_dir),
            "--request-file",
            str(primary_file),
        ],
        reports_dir / "02-primary-subscription.json",
    )

    export_reports: dict[str, dict[str, Any]] = {}
    for index, roundtrip in enumerate(ROUNDTRIPS, start=3):
        request = {
            "route_id": ROUTE_ID,
            "packet_hash": roundtrip["packet_hash"],
            "nonce": roundtrip["export_nonce"],
            "source_wallet": gate3.SOURCE_WALLET,
            "ethereum_recipient": addresses["external_user"].lower(),
            "amount_atoms": roundtrip["amount_atoms"],
            "source_height": roundtrip["source_height"],
            "destination_deadline_seconds": gate3.DESTINATION_DEADLINE_SECONDS,
            "refund_not_before_height": roundtrip["refund_not_before_height"],
        }
        request_file = inputs_dir / f"export-{roundtrip['label']}.json"
        write_json(request_file, request)
        export_reports[roundtrip["label"]] = node_json(
            [
                str(gate3.NODE_BIN),
                "navcoin-bridge-export-debit",
                "--data-dir",
                str(sidecar_dir),
                "--request-file",
                str(request_file),
            ],
            reports_dir / f"{index:02d}-export-{roundtrip['label']}.json",
        )

    supply_report = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-supply-status",
            "--data-dir",
            str(sidecar_dir),
            "--route-id",
            ROUTE_ID,
        ],
        reports_dir / "05-supply-status-after-exports.json",
    )
    replay_report = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-receipt-replay",
            "--data-dir",
            str(sidecar_dir),
            "--route-id",
            ROUTE_ID,
        ],
        reports_dir / "06-receipt-replay-after-exports.json",
    )
    return {
        "route_config": route_config,
        "route_init": route_init,
        "primary_report": primary_report,
        "export_reports": export_reports,
        "supply_report": supply_report,
        "replay_report": replay_report,
    }


def mint_packet_for_roundtrip(
    roundtrip: dict[str, Any],
    *,
    route_digest: str,
    export_report: dict[str, Any],
    receipt_root: str,
    controller: str,
    wrapped: str,
    recipient: str,
    pool_id_hex: str,
) -> tuple[Any, ...]:
    amount = roundtrip["amount_atoms"]
    return gate3.packet_tuple(
        route_digest=route_digest,
        export_report=export_report,
        receipt_root=receipt_root,
        controller=controller,
        wrapped=wrapped,
        recipient=recipient,
        pool_id_hex=pool_id_hex,
        settlement_atoms=amount * gate3.NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
        mint_atoms=amount,
        minimum_output_atoms=1,
        swap_path_hash=bytes(32),
    )


def burn_and_import_roundtrip(
    out_dir: Path,
    web3: Web3,
    controller: Any,
    wrapped: Any,
    external_user: str,
    roundtrip: dict[str, Any],
    report_index: int,
) -> dict[str, Any]:
    reports_dir = out_dir / "reports"
    inputs_dir = out_dir / "inputs"
    sidecar_dir = out_dir / "sidecar"
    label = roundtrip["label"]
    amount = roundtrip["amount_atoms"]
    return_nonce_hex = roundtrip["return_nonce"]
    return_nonce = bytes.fromhex(return_nonce_hex)
    native_asset_id = gate3.pftl_bytes(gate3.NATIVE_NAV_ASSET_ID)

    balance_before = wrapped.functions.balanceOf(external_user).call()
    total_supply_before = wrapped.functions.totalSupply().call()
    burn_receipt = gate3.transact(
        web3,
        controller.functions.burnForPftlReturn(
            amount,
            gate3.SOURCE_WALLET,
            native_asset_id,
            return_nonce,
        ),
        external_user,
    )
    events = controller.events.ReturnBurned().process_receipt(burn_receipt)
    if len(events) != 1:
        raise RuntimeError(f"{label}: expected exactly one ReturnBurned event, got {len(events)}")
    event = events[0]["args"]
    burn_id = no_prefix_hex(event["return_burn_id"])
    if Web3.to_checksum_address(event["ethereum_sender"]) != Web3.to_checksum_address(external_user):
        raise RuntimeError(f"{label}: event ethereum sender mismatch")
    if no_prefix_hex(event["return_nonce"]) != return_nonce_hex:
        raise RuntimeError(f"{label}: event return nonce mismatch")
    if event["pftl_recipient"] != gate3.SOURCE_WALLET:
        raise RuntimeError(f"{label}: event PFTL recipient mismatch")
    if no_prefix_hex(event["native_nav_asset_id"]) != gate3.NATIVE_NAV_ASSET_ID:
        raise RuntimeError(f"{label}: event native NAV asset mismatch")
    if event["amount_atoms"] != amount:
        raise RuntimeError(f"{label}: event amount mismatch")
    if event["ethereum_chain_id"] != 1:
        raise RuntimeError(f"{label}: event chain id mismatch")
    if Web3.to_checksum_address(event["bridge_controller"]) != Web3.to_checksum_address(controller.address):
        raise RuntimeError(f"{label}: event bridge controller mismatch")
    if Web3.to_checksum_address(event["wrapped_navcoin"]) != Web3.to_checksum_address(wrapped.address):
        raise RuntimeError(f"{label}: event wrapped token mismatch")
    if event["burn_height"] != burn_receipt.blockNumber:
        raise RuntimeError(f"{label}: event burn height mismatch")

    balance_after = wrapped.functions.balanceOf(external_user).call()
    total_supply_after = wrapped.functions.totalSupply().call()
    if balance_before - balance_after != amount:
        raise RuntimeError(f"{label}: Ethereum user balance did not burn exact amount")
    if total_supply_before - total_supply_after != amount:
        raise RuntimeError(f"{label}: Ethereum total supply did not burn exact amount")

    request_report = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-return-burn-request",
            "--data-dir",
            str(sidecar_dir),
            "--route-id",
            ROUTE_ID,
            "--ethereum-sender",
            external_user.lower(),
            "--pftl-recipient",
            gate3.SOURCE_WALLET,
            "--amount-atoms",
            str(amount),
            "--return-nonce",
            return_nonce_hex,
            "--burn-height",
            str(burn_receipt.blockNumber),
            "--output-file",
            str(inputs_dir / f"return-burn-{label}.json"),
            "--overwrite",
        ],
        reports_dir / f"{report_index:02d}-return-burn-request-{label}.json",
    )
    if request_report["burn_event_hash"] != burn_id:
        raise RuntimeError(f"{label}: PFTL-derived burn id does not match Ethereum event")

    record_report = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-record-return-burn",
            "--data-dir",
            str(sidecar_dir),
            "--route-id",
            ROUTE_ID,
            "--request-file",
            str(inputs_dir / f"return-burn-{label}.json"),
        ],
        reports_dir / f"{report_index + 1:02d}-record-return-burn-{label}.json",
    )
    import_report = node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-import-return",
            "--data-dir",
            str(sidecar_dir),
            "--route-id",
            ROUTE_ID,
            "--burn-event-hash",
            burn_id,
            "--pftl-recipient",
            gate3.SOURCE_WALLET,
        ],
        reports_dir / f"{report_index + 2:02d}-import-return-{label}.json",
    )

    return {
        "label": label,
        "amount_atoms": amount,
        "return_nonce": return_nonce_hex,
        "burn_event_hash": burn_id,
        "burn_tx": prefixed_hex(burn_receipt.transactionHash),
        "burn_height": burn_receipt.blockNumber,
        "evm_balance_before": balance_before,
        "evm_balance_after": balance_after,
        "evm_total_supply_before": total_supply_before,
        "evm_total_supply_after": total_supply_after,
        "return_burn_request": request_report,
        "record_report": record_report,
        "import_report": import_report,
    }


def record_destination_consume(out_dir: Path, roundtrip: dict[str, Any], report_index: int) -> dict[str, Any]:
    return node_json(
        [
            str(gate3.NODE_BIN),
            "navcoin-bridge-destination-consume",
            "--data-dir",
            str(out_dir / "sidecar"),
            "--route-id",
            ROUTE_ID,
            "--packet-hash",
            roundtrip["packet_hash"],
        ],
        out_dir / "reports" / f"{report_index:02d}-destination-consume-{roundtrip['label']}.json",
    )


def write_readme(out_dir: Path, fork_rpc_url: str, summary: dict[str, Any]) -> None:
    roundtrip_lines = "\n".join(
        f"- {item['label']}: burn tx `{item['burn_tx']}`, burn id `{item['burn_event_hash']}`, amount `{item['amount_atoms']}` atoms"
        for item in summary["roundtrips"]
    )
    invariant = str(summary["final_supply"]["invariant_holds"]).lower()
    text = f"""# PFTL-Uniswap Gate 4 Controlled Return Evidence

Generated by:

```bash
python3 scripts/pftl-uniswap-gate4-return-execute.py --replace
```

Upstream fork RPC: `{fork_rpc_url}`

What this evidence proves:

- PFTL sidecar primary issuance created `{TOTAL_ROUNDTRIP_ATOMS}` native NAV atoms for controlled export.
- Two PFTL export packets were consumed on the Ethereum fork and minted exactly `{TOTAL_ROUNDTRIP_ATOMS}` `{gate3.WRAPPED_TOKEN_SYMBOL}` atoms.
- Two `burnForPftlReturn` calls emitted return burn ids that matched the PFTL CLI's canonical burn-id derivation.
- Both burns were recorded and imported through `navcoin-bridge-record-return-burn` and `navcoin-bridge-import-return`, with no manual ledger edits.
- Final receipt replay returned `{summary["final_replay"]["status"]}` and final supply invariant is `{invariant}`.
- Ethereum wrapped supply returned to `{summary["evm"]["final_total_supply_atoms"]}` atoms after both burns.

Key outputs:

- route id: `{ROUTE_ID}`
- route config digest: `{summary["route_config_digest"]}`
- receipt root after exports: `{summary["receipt_root_after_exports"]}`
- final receipt root: `{summary["final_replay"]["receipt_root"]}`
- controller: `{summary["addresses"]["controller"]}`
- wrapped token: `{summary["addresses"]["wrapped"]}`

Round trips:

{roundtrip_lines}
"""
    (out_dir / "README.md").write_text(text, encoding="utf-8")


def main() -> int:
    args = parse_args()
    fork_rpc_url = args.fork_rpc_url or os.environ.get("ETHEREUM_RPC_URL") or os.environ.get("MAINNET_RPC_URL") or "https://ethereum-rpc.publicnode.com"
    out_dir = Path(args.out_dir).resolve()
    if out_dir.exists():
        if not args.replace:
            raise RuntimeError(f"evidence directory already exists: {out_dir}; pass --replace")
        shutil.rmtree(out_dir)
    (out_dir / "inputs").mkdir(parents=True)
    (out_dir / "reports").mkdir(parents=True)
    (out_dir / "sidecar").mkdir(parents=True)

    gate3.run(
        ["cargo", "build", "-p", "postfiat-node"],
        stdout_file=out_dir / "reports" / "00-cargo-build-postfiat-node.txt",
    )
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
        start_nonce = web3.eth.get_transaction_count(owner)
        addresses = {
            "owner": owner,
            "external_user": external_user,
            "wrapped": gate3.compute_create_address(owner, start_nonce),
            "verifier": gate3.compute_create_address(owner, start_nonce + 1),
            "replay_registry": gate3.compute_create_address(owner, start_nonce + 2),
            "v4_router": gate3.compute_create_address(owner, start_nonce + 3),
            "adapter": gate3.compute_create_address(owner, start_nonce + 4),
            "controller": gate3.compute_create_address(owner, start_nonce + 5),
        }
        pool_id_hex = gate3.pool_id(web3, addresses["wrapped"], gate3.USDC)
        sidecar = run_sidecar(out_dir, addresses, pool_id_hex)
        route_digest = sidecar["route_init"]["route_config_digest"]

        wrapped, _ = gate3.deploy(
            web3,
            "PFTLUniswapHandoffController.sol",
            "WrappedVenueNAVCoin",
            owner,
            gate3.WRAPPED_TOKEN_NAME,
            gate3.WRAPPED_TOKEN_SYMBOL,
            6,
            owner,
        )
        verifier, _ = gate3.deploy(
            web3,
            "PFTLUniswapHandoffController.sol",
            "ControlledPFTLReceiptVerifier",
            owner,
            owner,
            Web3.keccak(text=gate3.TRUST_CLASS),
        )
        replay_registry, _ = gate3.deploy(web3, "PFTLUniswapHandoffController.sol", "PacketReplayRegistry", owner, owner)
        v4_router, _ = gate3.deploy(
            web3,
            "PFTLUniswapV4PoolHarness.sol",
            "PFTLUniswapV4ExactInputRouter",
            owner,
            gate3.POOL_MANAGER,
        )
        adapter, _ = gate3.deploy(
            web3,
            "PFTLUniswapHandoffController.sol",
            "UniswapSettlementAdapter",
            owner,
            v4_router.address,
            wrapped.address,
            Web3.to_checksum_address(gate3.USDC),
            bytes.fromhex(pool_id_hex.removeprefix("0x")),
            Web3.keccak(gate3.SWAP_DATA),
            owner,
        )
        route_config_tuple = (
            owner,
            1,
            gate3.pftl_bytes(route_digest),
            Web3.keccak(text=gate3.TRUST_CLASS),
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

        expected = {key: Web3.to_checksum_address(value) for key, value in addresses.items() if key not in {"owner", "external_user"}}
        actual = {
            "wrapped": wrapped.address,
            "verifier": verifier.address,
            "replay_registry": replay_registry.address,
            "v4_router": v4_router.address,
            "adapter": adapter.address,
            "controller": controller.address,
        }
        if actual != expected:
            raise RuntimeError(f"precomputed deployment addresses drifted: expected={expected}, actual={actual}")

        gate3.transact(web3, wrapped.functions.setController(controller.address), owner)
        gate3.transact(web3, wrapped.functions.lockController(), owner)
        gate3.transact(web3, replay_registry.functions.setControllerAuthorization(controller.address, True), owner)
        gate3.transact(web3, adapter.functions.setController(controller.address), owner)
        gate3.transact(web3, adapter.functions.lockController(), owner)

        receipt_root = sidecar["replay_report"]["receipt_root"]
        consume_txs: dict[str, str] = {}
        destination_consume_reports: dict[str, dict[str, Any]] = {}
        for index, roundtrip in enumerate(ROUNDTRIPS, start=7):
            export_report = sidecar["export_reports"][roundtrip["label"]]
            packet = mint_packet_for_roundtrip(
                roundtrip,
                route_digest=route_digest,
                export_report=export_report,
                receipt_root=receipt_root,
                controller=controller.address,
                wrapped=wrapped.address,
                recipient=external_user,
                pool_id_hex=pool_id_hex,
            )
            digest = controller.functions.packetDigest(packet).call()
            gate3.transact(
                web3,
                verifier.functions.setReceiptAcceptance(packet[3], packet[2], packet[0], digest, True),
                owner,
            )
            consume_receipt = gate3.transact(web3, controller.functions.consumeMintOnly(packet), owner)
            consume_txs[roundtrip["label"]] = prefixed_hex(consume_receipt.transactionHash)
            destination_consume_reports[roundtrip["label"]] = record_destination_consume(out_dir, roundtrip, index)

        minted_balance = wrapped.functions.balanceOf(external_user).call()
        total_supply_after_consumes = wrapped.functions.totalSupply().call()
        if minted_balance != TOTAL_ROUNDTRIP_ATOMS or total_supply_after_consumes != TOTAL_ROUNDTRIP_ATOMS:
            raise RuntimeError(
                f"expected {TOTAL_ROUNDTRIP_ATOMS} wrapped atoms after consumes; "
                f"user={minted_balance} total={total_supply_after_consumes}"
            )

        roundtrip_results = []
        for report_index, roundtrip in zip([9, 12], ROUNDTRIPS):
            roundtrip_results.append(
                burn_and_import_roundtrip(
                    out_dir,
                    web3,
                    controller,
                    wrapped,
                    external_user,
                    roundtrip,
                    report_index,
                )
            )

        final_supply = node_json(
            [
                str(gate3.NODE_BIN),
                "navcoin-bridge-supply-status",
                "--data-dir",
                str(out_dir / "sidecar"),
                "--route-id",
                ROUTE_ID,
            ],
            out_dir / "reports" / "15-final-supply-status.json",
        )
        final_replay = node_json(
            [
                str(gate3.NODE_BIN),
                "navcoin-bridge-receipt-replay",
                "--data-dir",
                str(out_dir / "sidecar"),
                "--route-id",
                ROUTE_ID,
            ],
            out_dir / "reports" / "16-final-receipt-replay.json",
        )

        final_user_balance = wrapped.functions.balanceOf(external_user).call()
        final_total_supply = wrapped.functions.totalSupply().call()
        total_burned = controller.functions.total_return_burned_atoms().call()
        if final_user_balance != 0 or final_total_supply != 0:
            raise RuntimeError(f"wrapped supply should return to zero; user={final_user_balance} total={final_total_supply}")
        if total_burned != TOTAL_ROUNDTRIP_ATOMS:
            raise RuntimeError(f"controller total return burned mismatch: {total_burned}")
        if final_replay["status"] != "verified":
            raise RuntimeError(f"final receipt replay did not verify: {final_replay}")
        if not final_supply["invariant_holds"]:
            raise RuntimeError(f"final supply invariant failed: {final_supply}")
        if final_supply["pftl_spendable_supply_atoms"] != TOTAL_ROUNDTRIP_ATOMS:
            raise RuntimeError(f"PFTL spendable supply was not restored: {final_supply}")
        if final_supply["ethereum_spendable_supply_atoms"] != 0:
            raise RuntimeError(f"Ethereum spendable supply should be zero after returns: {final_supply}")
        if final_supply["pending_return_import_claims_atoms"] != 0:
            raise RuntimeError(f"pending return import claims should be zero: {final_supply}")

        summary = {
            "route_id": ROUTE_ID,
            "route_config_digest": route_digest,
            "pool_id": pool_id_hex,
            "addresses": addresses,
            "receipt_root_after_exports": receipt_root,
            "consume_txs": consume_txs,
            "destination_consume_reports": destination_consume_reports,
            "roundtrips": roundtrip_results,
            "final_supply": final_supply,
            "final_replay": final_replay,
            "evm": {
                "balance_after_consumes_atoms": minted_balance,
                "total_supply_after_consumes_atoms": total_supply_after_consumes,
                "final_user_balance_atoms": final_user_balance,
                "final_total_supply_atoms": final_total_supply,
                "total_return_burned_atoms": total_burned,
            },
        }
        write_json(out_dir / "reports" / "17-summary.json", summary)
        write_readme(out_dir, fork_rpc_url, summary)
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
