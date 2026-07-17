#!/usr/bin/env python3
"""Execute the controlled Gate 3 PFTL-Uniswap fork rehearsal.

The script starts an Ethereum mainnet fork, precomputes deployment addresses,
creates a route ledger whose config digest binds those addresses, deploys the
controlled bridge/adapter contracts, seeds a real Uniswap v4 pool from a PFTL
export packet, executes buy/sell swaps, and records the fork evidence through
the existing node verifier.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import rlp
from eth_utils import keccak
from web3 import Web3


ROOT = Path(__file__).resolve().parents[1]
ETH_CONTRACTS = ROOT / "crates" / "ethereum-contracts"
DEFAULT_OUT_DIR = ROOT / "docs" / "evidence" / "pftl-uniswap-gate3-2026-07-01"
OFFICIAL_UNISWAP_FILE = ROOT / "docs" / "plans" / "pftl-uniswap-official-uniswap-v4-ethereum-2026-07-01.json"
REHEARSAL_COLLECTOR = ROOT / "scripts" / "pftl-uniswap-gate3-fork-rehearsal.mjs"
NODE_BIN = ROOT / "target" / "debug" / "postfiat-node"


def env_str(name: str, default: str) -> str:
    return os.environ.get(name, default)


def env_int(name: str, default: int) -> int:
    value = os.environ.get(name)
    if value is None:
        return default
    return int(value, 0)


ROUTE_ID = env_str("PFTL_UNISWAP_ROUTE_ID", "pftl-a666-ethereum-wA666-usdc-gate3-v1")
SOURCE_WALLET = env_str("PFTL_UNISWAP_SOURCE_WALLET", "pf124071fd53a12ca4556b7aa1f5ec98b585e73468")
NATIVE_NAV_ASSET_ID = env_str("PFTL_UNISWAP_NATIVE_NAV_ASSET_ID", "aa" * 48)
SETTLEMENT_ASSET_ID = env_str("PFTL_UNISWAP_SETTLEMENT_ASSET_ID", "88" * 48)
PRICING_RESERVE_PACKET_HASH = env_str("PFTL_UNISWAP_PRICING_RESERVE_PACKET_HASH", "99" * 48)
TRUST_CLASS = env_str("PFTL_UNISWAP_TRUST_CLASS", "CONTROLLED")
VERIFIER_MODE = env_str("PFTL_UNISWAP_VERIFIER_MODE", "threshold-controlled")
WRAPPED_TOKEN_NAME = env_str("PFTL_UNISWAP_WRAPPED_TOKEN_NAME", "Wrapped A666")
WRAPPED_TOKEN_SYMBOL = env_str("PFTL_UNISWAP_WRAPPED_TOKEN_SYMBOL", "wA666")
USDC = env_str("PFTL_UNISWAP_USDC_TOKEN", "0xA0b86991c6218b36c1d19D4A2e9Eb0cE3606eB48")
POOL_MANAGER = "0x000000000004444c5dc75cB358380D2e3dE08A90"
POSITION_MANAGER = "0xbD216513d74C8cf14cf4747E6AaA6420FF64ee9e"
PERMIT2 = "0x000000000022D473030F116dDEE9F6B43aC78BA3"

SEED_USDC_ATOMS = env_int("PFTL_UNISWAP_SEED_USDC_ATOMS", 100_000_000)
SEED_WRAPPED_ATOMS = env_int("PFTL_UNISWAP_SEED_WRAPPED_ATOMS", 100_000)
EXTRA_PACKET_ATOMS = env_int("PFTL_UNISWAP_EXTRA_PACKET_ATOMS", 20)
NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM = env_int(
    "PFTL_UNISWAP_NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM",
    1_000,
)
PRIMARY_SETTLEMENT_ATOMS = (SEED_WRAPPED_ATOMS + EXTRA_PACKET_ATOMS) * NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM
MINT_ONLY_ATOMS = env_int("PFTL_UNISWAP_MINT_ONLY_ATOMS", 10)
MINT_AND_SWAP_ATOMS = env_int("PFTL_UNISWAP_MINT_AND_SWAP_ATOMS", 10)
EXTERNAL_BUY_USDC_ATOMS = env_int("PFTL_UNISWAP_EXTERNAL_BUY_USDC_ATOMS", 1_000_000)
DESTINATION_DEADLINE_SECONDS = env_int("PFTL_UNISWAP_DESTINATION_DEADLINE_SECONDS", 1_924_992_000)
FEE_PIPS = env_int("PFTL_UNISWAP_FEE_PIPS", 500)
TICK_LOWER = env_int("PFTL_UNISWAP_TICK_LOWER", -887270)
TICK_UPPER = env_int("PFTL_UNISWAP_TICK_UPPER", 887270)
RETURN_FINALITY_BLOCKS = env_int("PFTL_UNISWAP_RETURN_FINALITY_BLOCKS", 64)
LATEST_FINALIZED_NAV_EPOCH = env_int("PFTL_UNISWAP_LATEST_FINALIZED_NAV_EPOCH", 7)
ROUTE_SUPPLY_CAP_ATOMS = env_int("PFTL_UNISWAP_ROUTE_SUPPLY_CAP_ATOMS", 10_000_000)
PACKET_NOTIONAL_CAP_ATOMS = env_int("PFTL_UNISWAP_PACKET_NOTIONAL_CAP_ATOMS", 200_000_000)
SWAP_DATA = env_str("PFTL_UNISWAP_SWAP_DATA", "postfiat-gate3-wa666-usdc-v4-exact-input").encode()


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


def run(
    cmd: list[str],
    *,
    cwd: Path = ROOT,
    stdout_file: Path | None = None,
    env: dict[str, str] | None = None,
) -> str:
    result = subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, check=False, env=env)
    if stdout_file is not None:
        stdout_file.parent.mkdir(parents=True, exist_ok=True)
        stdout_file.write_text(result.stdout, encoding="utf-8")
        if result.stderr:
            stdout_file.with_suffix(stdout_file.suffix + ".stderr").write_text(result.stderr, encoding="utf-8")
    if result.returncode != 0:
        message = f"command failed ({result.returncode}): {' '.join(cmd)}\n{result.stdout}\n{result.stderr}"
        raise RuntimeError(message)
    return result.stdout


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def start_anvil(fork_rpc_url: str) -> tuple[str, subprocess.Popen[str]]:
    anvil = shutil.which("anvil") or str(Path.home() / ".foundry" / "bin" / "anvil")
    if not Path(anvil).exists() and shutil.which("anvil") is None:
        raise RuntimeError("anvil is not installed")
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        port = int(sock.getsockname()[1])
    rpc_url = f"http://127.0.0.1:{port}"
    proc = subprocess.Popen(
        [anvil, "--fork-url", fork_rpc_url, "--host", "127.0.0.1", "--port", str(port)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    return rpc_url, proc


def wait_for_rpc(web3: Web3) -> None:
    for _ in range(300):
        if web3.is_connected():
            return
        time.sleep(0.1)
    raise RuntimeError("fork Anvil did not become reachable")


def compute_create_address(deployer: str, nonce: int) -> str:
    encoded = rlp.encode([bytes.fromhex(deployer.removeprefix("0x")), nonce])
    return Web3.to_checksum_address(keccak(encoded)[12:])


def pool_id(web3: Web3, wrapped: str, usdc: str) -> str:
    wrapped_addr = Web3.to_checksum_address(wrapped)
    usdc_addr = Web3.to_checksum_address(usdc)
    currency0, currency1 = sorted([wrapped_addr, usdc_addr], key=lambda value: int(value, 16))
    encoded = web3.codec.encode(
        ["address", "address", "uint24", "int24", "address"],
        [currency0, currency1, FEE_PIPS, 10, "0x0000000000000000000000000000000000000000"],
    )
    return Web3.to_hex(Web3.keccak(encoded))


def set_usdc_balance(web3: Web3, account: str, amount: int) -> None:
    key = Web3.keccak(Web3.to_bytes(hexstr=account).rjust(32, b"\0") + (9).to_bytes(32, "big"))
    web3.provider.make_request(
        "anvil_setStorageAt",
        [Web3.to_checksum_address(USDC), Web3.to_hex(key), "0x" + amount.to_bytes(32, "big").hex()],
    )
    web3.provider.make_request("evm_mine", [])


def artifact(source: str, name: str) -> dict[str, Any]:
    return read_json(ETH_CONTRACTS / "out" / source / f"{name}.json")


def deploy(web3: Web3, source: str, name: str, sender: str, *args: Any):
    compiled = artifact(source, name)
    contract = web3.eth.contract(abi=compiled["abi"], bytecode=compiled["bytecode"]["object"])
    tx_hash = contract.constructor(*args).transact({"from": sender})
    receipt = web3.eth.wait_for_transaction_receipt(tx_hash, timeout=180)
    if receipt.status != 1:
        raise RuntimeError(f"deploy {name} reverted: {tx_hash.hex()}")
    return web3.eth.contract(address=receipt.contractAddress, abi=compiled["abi"]), receipt


def contract_at(web3: Web3, source: str, name: str, address: str):
    compiled = artifact(source, name)
    return web3.eth.contract(address=Web3.to_checksum_address(address), abi=compiled["abi"])


def transact(web3: Web3, fn: Any, sender: str, *, timeout: int = 240):
    tx_hash = fn.transact({"from": sender})
    receipt = web3.eth.wait_for_transaction_receipt(tx_hash, timeout=timeout)
    if receipt.status != 1:
        raise RuntimeError(f"transaction reverted: {tx_hash.hex()}")
    return receipt


def expect_revert(fn: Any, sender: str) -> None:
    try:
        tx_hash = fn.transact({"from": sender, "gas": 2_000_000})
        receipt = fn.w3.eth.wait_for_transaction_receipt(tx_hash, timeout=240)
        if receipt.status == 0:
            return
    except Exception:
        return
    raise RuntimeError("expected transaction to revert")


def tx_hex(value: Any) -> str:
    text = value.hex()
    return text if text.startswith("0x") else f"0x{text}"


def node_json(cmd: list[str], output_file: Path) -> dict[str, Any]:
    stdout = run(cmd, stdout_file=output_file)
    return json.loads(stdout)


def run_sidecar(out_dir: Path, addresses: dict[str, str], pool_id_hex: str) -> dict[str, Any]:
    sidecar_dir = out_dir / "sidecar"
    inputs_dir = out_dir / "inputs"
    reports_dir = out_dir / "reports"
    route_config_file = inputs_dir / "route-config.json"

    route_config = {
        "schema": "postfiat-pftl-uniswap-route-config-v1",
        "route_id": ROUTE_ID,
        "route_family": "primary_pftl_mint",
        "native_nav_asset_id": NATIVE_NAV_ASSET_ID,
        "settlement_asset_id": SETTLEMENT_ASSET_ID,
        "wrapped_navcoin_token": addresses["wrapped"].lower(),
        "handoff_controller": addresses["controller"].lower(),
        "settlement_adapter": addresses["adapter"].lower(),
        "verifier_mode": VERIFIER_MODE,
        "route_trust_class": TRUST_CLASS,
        "uniswap_pool_id_or_path": pool_id_hex.lower(),
        "router": addresses["v4_router"].lower(),
        "failure_behavior": "refund_unconsumed_pftl_packet",
        "route_supply_cap_atoms": ROUTE_SUPPLY_CAP_ATOMS,
        "packet_notional_cap_atoms": PACKET_NOTIONAL_CAP_ATOMS,
        "seed_nav_epoch": LATEST_FINALIZED_NAV_EPOCH,
        "seed_usdc_atoms": SEED_USDC_ATOMS,
        "seed_wrapped_navcoin_atoms": SEED_WRAPPED_ATOMS,
        "lp_recipient": addresses["owner"].lower(),
        "lp_custody_policy": "controlled_fork_rehearsal_lp",
    }
    write_json(route_config_file, route_config)

    route_init = node_json(
        [
            str(NODE_BIN),
            "navcoin-bridge-route-init",
            "--data-dir",
            str(sidecar_dir),
            "--config-file",
            str(route_config_file),
            "--ethereum-chain-id",
            "1",
            "--latest-finalized-nav-epoch",
            str(LATEST_FINALIZED_NAV_EPOCH),
            "--return-finality-blocks",
            str(RETURN_FINALITY_BLOCKS),
            "--replace",
        ],
        reports_dir / "01-route-init.json",
    )

    primary_request = {
        "route_id": ROUTE_ID,
        "source_wallet": SOURCE_WALLET,
        "settlement_asset_id": SETTLEMENT_ASSET_ID,
        "subscription_nonce": "71" * 32,
        "quote": {
            "settlement_value_atoms": PRIMARY_SETTLEMENT_ATOMS,
            "nav_price_settlement_atoms_per_nav_atom": NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
            "pricing_nav_epoch": LATEST_FINALIZED_NAV_EPOCH,
            "pricing_reserve_packet_hash": PRICING_RESERVE_PACKET_HASH,
        },
    }
    primary_file = inputs_dir / "primary-subscription.json"
    write_json(primary_file, primary_request)
    primary_report = node_json(
        [str(NODE_BIN), "navcoin-bridge-primary-subscribe", "--data-dir", str(sidecar_dir), "--request-file", str(primary_file)],
        reports_dir / "02-primary-subscription.json",
    )

    exports = [
        ("seed", "a1" * 48, "b1" * 32, addresses["helper"].lower(), SEED_WRAPPED_ATOMS, 10, 20),
        ("mint-only", "a2" * 48, "b2" * 32, addresses["external_user"].lower(), MINT_ONLY_ATOMS, 11, 21),
        ("mint-and-swap", "a3" * 48, "b3" * 32, addresses["external_user"].lower(), MINT_AND_SWAP_ATOMS, 12, 22),
    ]
    export_reports: dict[str, dict[str, Any]] = {}
    for index, (label, packet_hash, nonce, recipient, amount, source_height, refund_height) in enumerate(exports, start=3):
        request = {
            "route_id": ROUTE_ID,
            "packet_hash": packet_hash,
            "nonce": nonce,
            "source_wallet": SOURCE_WALLET,
            "ethereum_recipient": recipient,
            "amount_atoms": amount,
            "source_height": source_height,
            "destination_deadline_seconds": DESTINATION_DEADLINE_SECONDS,
            "refund_not_before_height": refund_height,
        }
        request_file = inputs_dir / f"export-{label}.json"
        write_json(request_file, request)
        export_reports[label] = node_json(
            [str(NODE_BIN), "navcoin-bridge-export-debit", "--data-dir", str(sidecar_dir), "--request-file", str(request_file)],
            reports_dir / f"{index:02d}-export-{label}.json",
        )

    supply_report = node_json(
        [str(NODE_BIN), "navcoin-bridge-supply-status", "--data-dir", str(sidecar_dir), "--route-id", ROUTE_ID],
        reports_dir / "06-supply-status.json",
    )
    replay_report = node_json(
        [str(NODE_BIN), "navcoin-bridge-receipt-replay", "--data-dir", str(sidecar_dir), "--route-id", ROUTE_ID],
        reports_dir / "07-receipt-replay.json",
    )

    launch_config_file = out_dir / "pftl-uniswap-launch-config.json"
    launch_report = node_json(
        [
            str(NODE_BIN),
            "navcoin-bridge-launch-config-template",
            "--route-config-file",
            str(route_config_file),
            "--official-uniswap-file",
            str(OFFICIAL_UNISWAP_FILE),
            "--usdc-token",
            USDC.lower(),
            "--receipt-verifier",
            addresses["verifier"].lower(),
            "--uniswap-pool-key-hash",
            pool_id_hex.removeprefix("0x").lower(),
            "--pricing-reserve-packet-hash",
            PRICING_RESERVE_PACKET_HASH,
            "--nav-price-settlement-atoms-per-nav-atom",
            str(NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM),
            "--tick-lower",
            str(TICK_LOWER),
            "--tick-upper",
            str(TICK_UPPER),
            "--fee-pips",
            str(FEE_PIPS),
            "--position-recipient",
            addresses["owner"].lower(),
            "--output-file",
            str(launch_config_file),
            "--overwrite",
        ],
        reports_dir / "08-launch-config-template.json",
    )
    init_report = node_json(
        [
            str(NODE_BIN),
            "navcoin-bridge-launch-config-init",
            "--data-dir",
            str(sidecar_dir),
            "--launch-config-file",
            str(launch_config_file),
            "--replace",
        ],
        reports_dir / "09-launch-config-init.json",
    )

    return {
        "route_config": route_config,
        "route_init": route_init,
        "primary_report": primary_report,
        "export_reports": export_reports,
        "supply_report": supply_report,
        "replay_report": replay_report,
        "launch_report": launch_report,
        "init_report": init_report,
        "launch_config_file": launch_config_file,
    }


def pftl_bytes(value: str) -> bytes:
    return bytes.fromhex(value.removeprefix("0x"))


def packet_tuple(
    *,
    route_digest: str,
    export_report: dict[str, Any],
    receipt_root: str,
    controller: str,
    wrapped: str,
    recipient: str,
    pool_id_hex: str,
    settlement_atoms: int,
    mint_atoms: int,
    minimum_output_atoms: int,
    swap_path_hash: bytes,
) -> tuple[Any, ...]:
    receipt = export_report["receipt"]
    return (
        pftl_bytes(route_digest),
        pftl_bytes(receipt["packet_hash"]),
        pftl_bytes(export_report["receipt_hash"]),
        pftl_bytes(receipt_root),
        1,
        Web3.to_checksum_address(controller),
        Web3.to_checksum_address(wrapped),
        Web3.keccak(text=SOURCE_WALLET),
        pftl_bytes(SETTLEMENT_ASSET_ID),
        pftl_bytes(NATIVE_NAV_ASSET_ID),
        pftl_bytes(PRICING_RESERVE_PACKET_HASH),
        bytes.fromhex(pool_id_hex.removeprefix("0x")),
        swap_path_hash,
        Web3.to_checksum_address(recipient),
        Web3.to_checksum_address(USDC),
        settlement_atoms,
        mint_atoms,
        minimum_output_atoms,
        LATEST_FINALIZED_NAV_EPOCH,
        DESTINATION_DEADLINE_SECONDS,
        bytes.fromhex(receipt["nonce"]),
    )


def collect_and_record_evidence(
    out_dir: Path,
    rpc_url: str,
    sidecar: dict[str, Any],
    txs: dict[str, str],
    deltas: dict[str, int],
    supply: dict[str, int],
    rehearsal_id: str = "gate3-controlled-fork-2026-07-01",
) -> dict[str, Any]:
    reports_dir = out_dir / "reports"
    evidence_file = out_dir / "pftl-uniswap-fork-rehearsal-evidence.json"
    collector_output = reports_dir / "10-fork-rehearsal-collector.json"
    cmd = [
        "node",
        str(REHEARSAL_COLLECTOR),
        "--launch-config-file",
        str(sidecar["launch_config_file"]),
        "--rpc-url",
        rpc_url,
        "--launch-config-digest",
        sidecar["launch_report"]["launch_config_digest"],
        "--seed-export-packet-hash",
        sidecar["export_reports"]["seed"]["receipt"]["packet_hash"],
        "--seed-receipt-root",
        sidecar["replay_report"]["receipt_root"],
        "--seed-mint-tx",
        txs["seed_mint"],
        "--seed-lp-tx",
        txs["seed_lp"],
        "--external-buy-tx",
        txs["external_buy"],
        "--external-sell-tx",
        txs["external_sell"],
        "--mint-only-packet-tx",
        txs["mint_only_packet"],
        "--mint-and-swap-packet-tx",
        txs["mint_and_swap_packet"],
        "--user-buy-usdc-spent-atoms",
        str(deltas["buy_usdc_spent"]),
        "--user-buy-wrapped-received-atoms",
        str(deltas["buy_wrapped_received"]),
        "--user-sell-wrapped-spent-atoms",
        str(deltas["sell_wrapped_spent"]),
        "--user-sell-usdc-received-atoms",
        str(deltas["sell_usdc_received"]),
        "--canonical-supply-before-external-trades-atoms",
        str(supply["before_external"]),
        "--canonical-supply-after-external-trades-atoms",
        str(supply["after_external"]),
        "--rehearsal-id",
        rehearsal_id,
        "--output-file",
        str(evidence_file),
    ]
    collector = json.loads(run(cmd, stdout_file=collector_output))
    record = node_json(
        [
            str(NODE_BIN),
            "navcoin-bridge-record-fork-rehearsal",
            "--data-dir",
            str(out_dir / "sidecar"),
            "--route-id",
            ROUTE_ID,
            "--evidence-file",
            str(evidence_file),
        ],
        reports_dir / "11-record-fork-rehearsal.json",
    )
    return {"collector": collector, "record": record, "evidence_file": evidence_file}


def generate_mvp4_beta_run_packet(
    *,
    out_dir: Path,
    addresses: dict[str, str],
    route_digest: str,
    launch_config_digest: str,
    pool_id_hex: str,
) -> dict[str, Any]:
    reports_dir = out_dir / "reports"
    swap_path_hash = Web3.to_hex(Web3.keccak(SWAP_DATA))
    request = {
        "route": "uniswap_atomic_handoff",
        "from_asset": "pfUSDC",
        "to_asset": "USDC",
        "amount": str(MINT_AND_SWAP_ATOMS),
        "recipient": addresses["external_user"].lower(),
        "minimum_output": "1",
        "deadline": str(DESTINATION_DEADLINE_SECONDS),
        "swap_path_hash": swap_path_hash,
        "failure_behavior": "refund_unconsumed_pftl_packet",
    }
    env = os.environ.copy()
    env.update(
        {
            "PFUSDC_ASSET_ID": SETTLEMENT_ASSET_ID,
            "NAVSWAP_ROUTE_ID": ROUTE_ID,
            "NAVSWAP_NATIVE_NAV_ASSET_ID": NATIVE_NAV_ASSET_ID,
            "NAVSWAP_SETTLEMENT_ASSET_ID": SETTLEMENT_ASSET_ID,
            "NAVSWAP_ROUTE_TRUST_CLASS": TRUST_CLASS,
            "NAVSWAP_WRAPPED_NAVCOIN_TOKEN": addresses["wrapped"].lower(),
            "NAVSWAP_HANDOFF_CONTROLLER": addresses["controller"].lower(),
            "NAVSWAP_SETTLEMENT_ADAPTER": addresses["adapter"].lower(),
            "NAVSWAP_VERIFIER_MODE": VERIFIER_MODE,
            "NAVSWAP_UNISWAP_POOL_ID": pool_id_hex.lower(),
            "NAVSWAP_UNISWAP_ROUTER": addresses["v4_router"].lower(),
            "NAVSWAP_UNISWAP_OUTPUT_TOKEN": USDC.lower(),
            "NAVSWAP_FAILURE_BEHAVIOR": "refund_unconsumed_pftl_packet",
            "NAVSWAP_ROUTE_SUPPLY_CAP_ATOMS": str(ROUTE_SUPPLY_CAP_ATOMS),
            "NAVSWAP_SUPPLY_CAP_REMAINING_ATOMS": str(
                ROUTE_SUPPLY_CAP_ATOMS - SEED_WRAPPED_ATOMS - MINT_ONLY_ATOMS
            ),
            "NAVSWAP_PACKET_NOTIONAL_CAP_ATOMS": str(PACKET_NOTIONAL_CAP_ATOMS),
            "NAVSWAP_SEED_NAV_EPOCH": str(LATEST_FINALIZED_NAV_EPOCH),
            "NAVSWAP_SEED_USDC_ATOMS": str(SEED_USDC_ATOMS),
            "NAVSWAP_SEED_WRAPPED_NAVCOIN_ATOMS": str(SEED_WRAPPED_ATOMS),
            "NAVSWAP_LP_RECIPIENT": addresses["owner"].lower(),
            "NAVSWAP_LP_CUSTODY_POLICY": "controlled_fork_rehearsal_lp",
            "NAVSWAP_ROUTE_CONFIG_DIGEST": route_digest,
            "NAVSWAP_LAUNCH_CONFIG_DIGEST": launch_config_digest,
            "NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE": "true",
            "NAVSWAP_ENABLE_UNISWAP_BETA_RUNS": "true",
            "NAVSWAP_UNISWAP_ROUTE_PAUSED": "false",
            "NAVSWAP_UNISWAP_PUBLIC_ROUTING_ENABLED": "false",
            "MVP4_REQUEST": json.dumps(request, sort_keys=True),
            "MVP4_GENERATED_AT": "2026-07-01T00:00:00.000Z",
        }
    )
    script = """
const {
  buildNavswapQuoteResponse,
  buildNavswapRunResponse,
  navswapCapabilities,
} = require('./wallet-proxy/server');

const request = JSON.parse(process.env.MVP4_REQUEST);
const generatedAt = new Date(process.env.MVP4_GENERATED_AT);
const capability = navswapCapabilities(generatedAt).routes.uniswap_atomic_handoff;
const quote = buildNavswapQuoteResponse(request);
const run = buildNavswapRunResponse(request);
process.stdout.write(JSON.stringify({
  schema: 'postfiat-pftl-uniswap-mvp4-beta-run-packet-evidence-v1',
  generated_at: generatedAt.toISOString(),
  request,
  capability,
  quote,
  run,
}, null, 2));
"""
    report_path = reports_dir / "13-mvp4-beta-run-packet.json"
    packet_report = json.loads(run(["node", "-e", script], stdout_file=report_path, env=env))
    if packet_report["capability"]["status"] != "controlled_beta_run_ready":
        raise RuntimeError(f"MVP4 beta capability not runnable: {packet_report['capability']['status']}")
    if packet_report["run"]["ok"] is not True:
        raise RuntimeError(f"MVP4 beta run packet failed: {packet_report['run']}")
    run_packet = packet_report["run"]["run_packet"]
    binding = run_packet["mint_and_swap_uniswap"]
    checks = {
        "route_config_digest": run_packet["route_config_digest"] == route_digest,
        "quote_route_config_digest": binding["route_config_digest"] == route_digest,
        "trust_class": run_packet["route_trust_class"] == TRUST_CLASS,
        "public_routing_disabled": run_packet["public_routing_enabled"] is False,
        "amount_in": int(binding["amount_in"]) == MINT_AND_SWAP_ATOMS,
        "minimum_output": int(binding["minimum_output"]) == 1,
        "recipient": binding["recipient"].lower() == addresses["external_user"].lower(),
        "deadline": int(binding["deadline"]) == DESTINATION_DEADLINE_SECONDS,
        "swap_path_hash": binding["swap_path_hash"] == swap_path_hash.removeprefix("0x"),
        "token_in": binding["token_in"].lower() == addresses["wrapped"].lower(),
        "token_out": binding["token_out"].lower() == USDC.lower(),
        "pool": binding["pool_id_or_path"].lower() == pool_id_hex.lower(),
        "router": binding["router"].lower() == addresses["v4_router"].lower(),
        "source_asset": binding["pftl_source_asset"] == SETTLEMENT_ASSET_ID,
    }
    failed = [name for name, ok in checks.items() if not ok]
    if failed:
        raise RuntimeError(f"MVP4 beta run packet mismatched Gate 3 config: {failed}")
    return packet_report


def write_readme(out_dir: Path, fork_rpc_url: str, summary: dict[str, Any]) -> None:
    text = f"""# PFTL-Uniswap Gate 3 Controlled Fork Evidence

Generated by:

```bash
python3 scripts/pftl-uniswap-gate3-fork-execute.py --replace
```

Upstream fork RPC: `{fork_rpc_url}`

What this evidence proves:

- deterministic fork route config and launch config were generated from precomputed deployment addresses;
- PFTL sidecar primary subscription minted `{SEED_WRAPPED_ATOMS + EXTRA_PACKET_ATOMS}` native NAV atoms from `{PRIMARY_SETTLEMENT_ATOMS}` settlement atoms;
- seed `wA666` came from the seed export packet, consumed by the fork bridge controller, not by manual EVM minting;
- official Uniswap v4 PoolManager, PositionManager, Universal Router, Permit2, and StateView bytecode were checked on the fork;
- the real v4 pool was initialized and seeded through PositionManager;
- an external USDC -> wA666 buy and wA666 -> USDC sell both produced nonzero deltas;
- canonical wrapped supply was unchanged across the external AMM buy/sell;
- the wallet proxy generated a capped `CONTROLLED` beta run packet bound to the same Gate 3 route digest;
- that beta run packet drove the submitted destination `consumeMintAndSwap` transaction and reached `destination_consumed_and_swapped`;
- a min-output failure was checked to revert without consuming the packet;
- fork evidence was recorded through `navcoin-bridge-record-fork-rehearsal`.

Key outputs:

- route id: `{ROUTE_ID}`
- route config digest: `{summary["route_config_digest"]}`
- launch config digest: `{summary["launch_config_digest"]}`
- pool id: `{summary["pool_id"]}`
- seed mint tx: `{summary["txs"]["seed_mint"]}`
- seed LP tx: `{summary["txs"]["seed_lp"]}`
- external buy tx: `{summary["txs"]["external_buy"]}`
- external sell tx: `{summary["txs"]["external_sell"]}`
- mint-only packet tx: `{summary["txs"]["mint_only_packet"]}`
- mint-and-swap packet tx: `{summary["txs"]["mint_and_swap_packet"]}`
- MVP4 beta run packet report: `{summary["mvp4_beta"]["run_packet_report"]}`
- MVP4 beta consume evidence report: `{summary["mvp4_beta"]["consume_evidence_report"]}`
- MVP4 beta USDC output atoms: `{summary["mvp4_beta"]["usdc_output_atoms"]}`
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

    run(["cargo", "build", "-p", "postfiat-node"], stdout_file=out_dir / "reports" / "00-cargo-build-postfiat-node.txt")
    run(["forge", "build"], cwd=ETH_CONTRACTS, stdout_file=out_dir / "reports" / "00-forge-build.txt")

    rpc_url, proc = start_anvil(fork_rpc_url)
    keep_anvil = args.keep_anvil
    try:
        web3 = Web3(Web3.HTTPProvider(rpc_url, request_kwargs={"timeout": 120}))
        wait_for_rpc(web3)
        if web3.eth.chain_id != 1:
            raise RuntimeError(f"fork chain id must be 1, got {web3.eth.chain_id}")
        owner = Web3.to_checksum_address(web3.eth.accounts[0])
        external_user = Web3.to_checksum_address(web3.eth.accounts[1])
        start_nonce = web3.eth.get_transaction_count(owner)
        addresses = {
            "owner": owner,
            "external_user": external_user,
            "wrapped": compute_create_address(owner, start_nonce),
            "verifier": compute_create_address(owner, start_nonce + 1),
            "replay_registry": compute_create_address(owner, start_nonce + 2),
            "v4_router": compute_create_address(owner, start_nonce + 3),
            "adapter": compute_create_address(owner, start_nonce + 4),
            "helper": compute_create_address(owner, start_nonce + 5),
            "controller": compute_create_address(owner, start_nonce + 6),
        }
        pool_id_hex = pool_id(web3, addresses["wrapped"], USDC)
        sidecar = run_sidecar(out_dir, addresses, pool_id_hex)
        route_digest = sidecar["route_init"]["route_config_digest"]

        set_usdc_balance(web3, owner, 1_000_000_000_000)
        set_usdc_balance(web3, external_user, 1_000_000_000_000)
        usdc = contract_at(web3, "PFTLUniswapV4PoolHarness.sol", "IERC20V4Harness", USDC)

        wrapped, _ = deploy(
            web3,
            "PFTLUniswapHandoffController.sol",
            "WrappedVenueNAVCoin",
            owner,
            WRAPPED_TOKEN_NAME,
            WRAPPED_TOKEN_SYMBOL,
            6,
            owner,
        )
        verifier, _ = deploy(
            web3,
            "PFTLUniswapHandoffController.sol",
            "ControlledPFTLReceiptVerifier",
            owner,
            owner,
            Web3.keccak(text=TRUST_CLASS),
        )
        replay_registry, _ = deploy(web3, "PFTLUniswapHandoffController.sol", "PacketReplayRegistry", owner, owner)
        v4_router, _ = deploy(web3, "PFTLUniswapV4PoolHarness.sol", "PFTLUniswapV4ExactInputRouter", owner, POOL_MANAGER)
        adapter, _ = deploy(
            web3,
            "PFTLUniswapHandoffController.sol",
            "UniswapSettlementAdapter",
            owner,
            v4_router.address,
            wrapped.address,
            Web3.to_checksum_address(USDC),
            bytes.fromhex(pool_id_hex.removeprefix("0x")),
            Web3.keccak(SWAP_DATA),
            owner,
        )
        helper, _ = deploy(
            web3,
            "PFTLUniswapV4PoolHarness.sol",
            "PFTLUniswapV4LaunchHelper",
            owner,
            owner,
            POOL_MANAGER,
            POSITION_MANAGER,
            PERMIT2,
        )
        route_config_tuple = (
            owner,
            1,
            pftl_bytes(route_digest),
            Web3.keccak(text=TRUST_CLASS),
            pftl_bytes(SETTLEMENT_ASSET_ID),
            pftl_bytes(NATIVE_NAV_ASSET_ID),
            pftl_bytes(PRICING_RESERVE_PACKET_HASH),
            LATEST_FINALIZED_NAV_EPOCH,
            bytes.fromhex(pool_id_hex.removeprefix("0x")),
            ROUTE_SUPPLY_CAP_ATOMS,
            PACKET_NOTIONAL_CAP_ATOMS,
            replay_registry.address,
        )
        controller, _ = deploy(
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
            "helper": helper.address,
            "controller": controller.address,
        }
        if actual != expected:
            raise RuntimeError(f"precomputed deployment addresses drifted: expected={expected}, actual={actual}")

        transact(web3, wrapped.functions.setController(controller.address), owner)
        transact(web3, wrapped.functions.lockController(), owner)
        transact(web3, replay_registry.functions.setControllerAuthorization(controller.address, True), owner)
        transact(web3, adapter.functions.setController(controller.address), owner)
        transact(web3, adapter.functions.lockController(), owner)
        transact(web3, usdc.functions.transfer(helper.address, SEED_USDC_ATOMS), owner)

        receipt_root = sidecar["replay_report"]["receipt_root"]
        seed_packet = packet_tuple(
            route_digest=route_digest,
            export_report=sidecar["export_reports"]["seed"],
            receipt_root=receipt_root,
            controller=controller.address,
            wrapped=wrapped.address,
            recipient=helper.address,
            pool_id_hex=pool_id_hex,
            settlement_atoms=SEED_USDC_ATOMS,
            mint_atoms=SEED_WRAPPED_ATOMS,
            minimum_output_atoms=1,
            swap_path_hash=bytes(32),
        )
        seed_digest = controller.functions.packetDigest(seed_packet).call()
        transact(
            web3,
            verifier.functions.setReceiptAcceptance(seed_packet[3], seed_packet[2], seed_packet[0], seed_digest, True),
            owner,
        )
        seed_mint_receipt = transact(web3, controller.functions.consumeMintOnly(seed_packet), owner)
        seed_lp_receipt = transact(
            web3,
            helper.functions.initializeAndSeed(wrapped.address, Web3.to_checksum_address(USDC), SEED_WRAPPED_ATOMS, SEED_USDC_ATOMS),
            owner,
            timeout=900,
        )

        supply_before_external = wrapped.functions.totalSupply().call()
        transact(web3, usdc.functions.approve(v4_router.address, EXTERNAL_BUY_USDC_ATOMS), external_user)
        buy_usdc_before = usdc.functions.balanceOf(external_user).call()
        buy_wrapped_before = wrapped.functions.balanceOf(external_user).call()
        external_buy_receipt = transact(
            web3,
            v4_router.functions.exactInput(
                Web3.to_checksum_address(USDC),
                wrapped.address,
                EXTERNAL_BUY_USDC_ATOMS,
                1,
                external_user,
                DESTINATION_DEADLINE_SECONDS,
                b"",
            ),
            external_user,
            timeout=900,
        )
        buy_usdc_after = usdc.functions.balanceOf(external_user).call()
        buy_wrapped_after = wrapped.functions.balanceOf(external_user).call()
        sell_wrapped_spent = max(1, (buy_wrapped_after - buy_wrapped_before) // 2)
        transact(web3, wrapped.functions.approve(v4_router.address, sell_wrapped_spent), external_user)
        sell_wrapped_before = wrapped.functions.balanceOf(external_user).call()
        sell_usdc_before = usdc.functions.balanceOf(external_user).call()
        external_sell_receipt = transact(
            web3,
            v4_router.functions.exactInput(
                wrapped.address,
                Web3.to_checksum_address(USDC),
                sell_wrapped_spent,
                1,
                external_user,
                DESTINATION_DEADLINE_SECONDS,
                b"",
            ),
            external_user,
            timeout=900,
        )
        sell_wrapped_after = wrapped.functions.balanceOf(external_user).call()
        sell_usdc_after = usdc.functions.balanceOf(external_user).call()
        supply_after_external = wrapped.functions.totalSupply().call()

        mint_only_packet = packet_tuple(
            route_digest=route_digest,
            export_report=sidecar["export_reports"]["mint-only"],
            receipt_root=receipt_root,
            controller=controller.address,
            wrapped=wrapped.address,
            recipient=external_user,
            pool_id_hex=pool_id_hex,
            settlement_atoms=MINT_ONLY_ATOMS * NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
            mint_atoms=MINT_ONLY_ATOMS,
            minimum_output_atoms=1,
            swap_path_hash=bytes(32),
        )
        mint_only_digest = controller.functions.packetDigest(mint_only_packet).call()
        transact(
            web3,
            verifier.functions.setReceiptAcceptance(mint_only_packet[3], mint_only_packet[2], mint_only_packet[0], mint_only_digest, True),
            owner,
        )
        mint_only_receipt = transact(web3, controller.functions.consumeMintOnly(mint_only_packet), owner)

        mvp4_beta_packet = generate_mvp4_beta_run_packet(
            out_dir=out_dir,
            addresses=addresses,
            route_digest=route_digest,
            launch_config_digest=sidecar["launch_report"]["launch_config_digest"],
            pool_id_hex=pool_id_hex,
        )
        mvp4_binding = mvp4_beta_packet["run"]["run_packet"]["mint_and_swap_uniswap"]
        mvp4_mint_atoms = int(mvp4_binding["amount_in"])
        mvp4_minimum_output_atoms = int(mvp4_binding["minimum_output"])
        mint_swap_packet = packet_tuple(
            route_digest=route_digest,
            export_report=sidecar["export_reports"]["mint-and-swap"],
            receipt_root=receipt_root,
            controller=controller.address,
            wrapped=wrapped.address,
            recipient=mvp4_binding["recipient"],
            pool_id_hex=pool_id_hex,
            settlement_atoms=mvp4_mint_atoms * NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
            mint_atoms=mvp4_mint_atoms,
            minimum_output_atoms=mvp4_minimum_output_atoms,
            swap_path_hash=bytes.fromhex(mvp4_binding["swap_path_hash"]),
        )
        mint_swap_digest = controller.functions.packetDigest(mint_swap_packet).call()
        transact(
            web3,
            verifier.functions.setReceiptAcceptance(mint_swap_packet[3], mint_swap_packet[2], mint_swap_packet[0], mint_swap_digest, True),
            owner,
        )
        mvp4_usdc_before = usdc.functions.balanceOf(external_user).call()
        mvp4_wrapped_before = wrapped.functions.balanceOf(external_user).call()
        expected_packet_digest, expected_mvp4_amount_out = controller.functions.consumeMintAndSwap(
            mint_swap_packet, SWAP_DATA
        ).call({"from": owner})
        if expected_packet_digest != mint_swap_digest:
            raise RuntimeError(
                "MVP4 beta exact-input preflight returned a different packet digest: "
                f"expected={tx_hex(mint_swap_digest)} actual={tx_hex(expected_packet_digest)}"
            )
        if expected_mvp4_amount_out <= 0:
            raise RuntimeError("MVP4 beta exact-input preflight produced no USDC output")
        mint_swap_receipt = transact(web3, controller.functions.consumeMintAndSwap(mint_swap_packet, SWAP_DATA), owner, timeout=900)
        mvp4_usdc_after = usdc.functions.balanceOf(external_user).call()
        mvp4_wrapped_after = wrapped.functions.balanceOf(external_user).call()
        mvp4_amount_out = mvp4_usdc_after - mvp4_usdc_before
        if mvp4_amount_out <= 0:
            raise RuntimeError("MVP4 beta mint-and-swap produced no USDC output")
        if mvp4_amount_out != expected_mvp4_amount_out:
            raise RuntimeError(
                "MVP4 beta mint-and-swap output did not match exact-input preflight: "
                f"expected={expected_mvp4_amount_out} actual={mvp4_amount_out}"
            )
        if not controller.functions.consumed_packet(mint_swap_digest).call():
            raise RuntimeError("MVP4 beta mint-and-swap did not mark the packet consumed")
        source_packet_commitment = Web3.keccak(mint_swap_packet[1])
        if not controller.functions.consumed_source_packet(source_packet_commitment).call():
            raise RuntimeError("MVP4 beta mint-and-swap did not mark the source packet consumed")
        mvp4_consume_evidence = {
            "schema": "postfiat-pftl-uniswap-mvp4-beta-consume-evidence-v1",
            "route_id": ROUTE_ID,
            "route_config_digest": route_digest,
            "launch_config_digest": sidecar["launch_report"]["launch_config_digest"],
            "source_reports": {
                "run_packet": "reports/13-mvp4-beta-run-packet.json",
                "export_receipt": "reports/05-export-mint-and-swap.json",
                "receipt_replay": "reports/07-receipt-replay.json",
            },
            "controlled_beta": {
                "trust_class": mvp4_beta_packet["run"]["run_packet"]["route_trust_class"],
                "public_routing_enabled": mvp4_beta_packet["run"]["run_packet"]["public_routing_enabled"],
                "route_supply_cap_atoms": mvp4_beta_packet["run"]["run_packet"]["route_supply_cap_atoms"],
                "packet_notional_cap_atoms": mvp4_beta_packet["run"]["run_packet"]["packet_notional_cap_atoms"],
                "quote_binding_hash": mvp4_beta_packet["run"]["run_packet"]["quote_binding_hash"],
            },
            "packet": {
                "packet_digest": tx_hex(mint_swap_digest),
                "source_packet_hash": sidecar["export_reports"]["mint-and-swap"]["receipt"]["packet_hash"],
                "source_packet_commitment": tx_hex(source_packet_commitment),
                "source_receipt_hash": sidecar["export_reports"]["mint-and-swap"]["receipt_hash"],
                "source_receipt_root": receipt_root,
                "mint_amount_atoms": mvp4_mint_atoms,
                "settlement_amount_atoms": mvp4_mint_atoms * NAV_PRICE_SETTLEMENT_ATOMS_PER_NAV_ATOM,
                "minimum_output_atoms": mvp4_minimum_output_atoms,
                "swap_path_hash": mvp4_binding["swap_path_hash"],
                "recipient": mvp4_binding["recipient"],
                "deadline": mvp4_binding["deadline"],
            },
            "destination_consume": {
                "tx": tx_hex(mint_swap_receipt.transactionHash),
                "status": int(mint_swap_receipt.status),
                "terminal_state": "destination_consumed_and_swapped",
                "exact_input_preflight_usdc_output_atoms": expected_mvp4_amount_out,
                "packet_consumed": True,
                "source_packet_consumed": True,
                "usdc_before_atoms": mvp4_usdc_before,
                "usdc_after_atoms": mvp4_usdc_after,
                "usdc_output_atoms": mvp4_amount_out,
                "exact_input_output_assertion": mvp4_amount_out == expected_mvp4_amount_out,
                "wrapped_before_atoms": mvp4_wrapped_before,
                "wrapped_after_atoms": mvp4_wrapped_after,
            },
        }
        write_json(out_dir / "reports" / "14-mvp4-beta-consume-evidence.json", mvp4_consume_evidence)

        failure_packet = list(mint_swap_packet)
        failure_packet[1] = pftl_bytes("a4" * 48)
        failure_packet[2] = pftl_bytes("b4" * 48)
        failure_packet[16] = MINT_AND_SWAP_ATOMS
        failure_packet[17] = 2**63
        failure_packet[20] = bytes.fromhex("c4" * 32)
        failure_packet = tuple(failure_packet)
        failure_digest = controller.functions.packetDigest(failure_packet).call()
        transact(
            web3,
            verifier.functions.setReceiptAcceptance(failure_packet[3], failure_packet[2], failure_packet[0], failure_digest, True),
            owner,
        )
        expect_revert(controller.functions.consumeMintAndSwap(failure_packet, SWAP_DATA), owner)
        if controller.functions.consumed_packet(failure_digest).call():
            raise RuntimeError("min-output failure consumed packet")

        txs = {
            "seed_mint": tx_hex(seed_mint_receipt.transactionHash),
            "seed_lp": tx_hex(seed_lp_receipt.transactionHash),
            "external_buy": tx_hex(external_buy_receipt.transactionHash),
            "external_sell": tx_hex(external_sell_receipt.transactionHash),
            "mint_only_packet": tx_hex(mint_only_receipt.transactionHash),
            "mint_and_swap_packet": tx_hex(mint_swap_receipt.transactionHash),
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
        if supply_before_external != SEED_WRAPPED_ATOMS or supply_after_external != supply_before_external:
            raise RuntimeError(f"canonical supply changed across external trades: {supply}")

        evidence = collect_and_record_evidence(out_dir, rpc_url, sidecar, txs, deltas, supply)
        summary = {
            "route_id": ROUTE_ID,
            "route_config_digest": route_digest,
            "launch_config_digest": sidecar["launch_report"]["launch_config_digest"],
            "pool_id": pool_id_hex,
            "addresses": addresses,
            "txs": txs,
            "deltas": deltas,
            "supply": supply,
            "mvp4_beta": {
                "run_packet_report": "reports/13-mvp4-beta-run-packet.json",
                "consume_evidence_report": "reports/14-mvp4-beta-consume-evidence.json",
                "destination_consume_tx": txs["mint_and_swap_packet"],
                "terminal_state": "destination_consumed_and_swapped",
                "quote_binding_hash": mvp4_beta_packet["run"]["run_packet"]["quote_binding_hash"],
                "exact_input_preflight_usdc_output_atoms": expected_mvp4_amount_out,
                "usdc_output_atoms": mvp4_amount_out,
                "exact_input_output_assertion": mvp4_amount_out == expected_mvp4_amount_out,
            },
            "collector": evidence["collector"],
            "record": evidence["record"],
        }
        write_json(out_dir / "reports" / "12-summary.json", summary)
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
