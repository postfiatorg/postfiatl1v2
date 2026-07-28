#!/usr/bin/env python3
"""Build one exact transparent A666 reservation/subscription/export sequence."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import time
from pathlib import Path

from web3 import Web3


ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
JOE_PFTL = "pfab9b9228942e5c529633a13aa271d5297bec6353"
JOE_EVM = "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"
ZERO_HASH48 = "00" * 48


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--supply-status", type=Path, required=True)
    parser.add_argument("--holder-key-file", type=Path, required=True)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--mint-amount-atoms", type=int, required=True)
    parser.add_argument("--reservation-expires-at-height", type=int, default=2000)
    parser.add_argument("--refund-delay-blocks", type=int, default=100)
    parser.add_argument("--deadline-seconds", type=int)
    return parser.parse_args()


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def random_hex(byte_count: int) -> str:
    return os.urandom(byte_count).hex()


def ops(label: str, key_file: Path, operation: dict[str, object]) -> dict[str, object]:
    return {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": label,
                "source": JOE_PFTL,
                "key_file": str(key_file.resolve()),
                "operation": operation,
            }
        ],
    }


def main() -> None:
    args = parse_args()
    if args.output_dir.exists():
        raise RuntimeError(f"refusing to overwrite operation packet: {args.output_dir}")
    if not args.holder_key_file.is_file():
        raise RuntimeError("Joe's PFTL signing key is unavailable")
    if not args.node_bin.is_file():
        raise RuntimeError("digest-capable node binary is unavailable")

    status = json.loads(args.supply_status.read_text())
    required = {
        "schema": "postfiat-pftl-uniswap-supply-status-v2",
        "route_id": ROUTE_ID,
        "live_value_enabled": True,
        "paused": False,
        "invariant_holds": True,
        "route_schema_version": 2,
        "outbound_verification_class": "TRUSTLESS_FINALITY",
        "return_verification_class": "BFT_CHECKPOINT",
        "ethereum_chain_id": 1,
    }
    for field, expected in required.items():
        if status.get(field) != expected:
            raise RuntimeError(f"route status {field} differs from {expected!r}")

    amount = args.mint_amount_atoms
    if amount <= 0:
        raise RuntimeError("--mint-amount-atoms must be positive")
    if not status["min_order_atoms"] <= amount <= status["max_order_atoms"]:
        raise RuntimeError("mint amount is outside the governed order bounds")
    if amount > status["available_issue_atoms"]:
        raise RuntimeError("mint amount exceeds available issue capacity")
    multiplier = int(status["issue_multiplier_bps"])
    settlement = (amount * multiplier + 9_999) // 10_000
    deadline = args.deadline_seconds or int(time.time()) + 86_400
    if deadline <= int(time.time()) + 3_600:
        raise RuntimeError("destination deadline must leave at least one hour")
    if args.refund_delay_blocks <= 0:
        raise RuntimeError("--refund-delay-blocks must be positive")

    reservation_id = random_hex(48)
    subscription_nonce = random_hex(32)
    packet_hash = random_hex(48)
    export_nonce = random_hex(32)
    policy_hash = status["policy_hash"]
    policy_commitment = Web3.keccak(hexstr=policy_hash).hex()

    reserve_operation = {
        "operation": "pftl_uniswap_order_reserve",
        "subscriber": JOE_PFTL,
        "route_id": ROUTE_ID,
        "reservation_id": reservation_id,
        "ethereum_recipient": JOE_EVM,
        "route_epoch": status["route_epoch"],
        "policy_epoch": status["policy_epoch"],
        "policy_hash": policy_hash,
        "mint_amount_atoms": amount,
        "max_settlement_value_atoms": settlement,
        "expires_at_height": args.reservation_expires_at_height,
    }
    subscribe_operation = {
        "operation": "pftl_uniswap_primary_subscribe_v2",
        "subscriber": JOE_PFTL,
        "route_id": ROUTE_ID,
        "reservation_id": reservation_id,
        "subscription_nonce": subscription_nonce,
        "settlement_asset_id": status["settlement_asset_id"],
        "settlement_value_atoms": settlement,
        "pricing_nav_epoch": status["pricing_nav_epoch"],
        "pricing_reserve_packet_hash": status["pricing_reserve_packet_hash"],
    }
    mint_packet = {
        "route_config_digest": status["route_config_digest"],
        "source_packet_hash": packet_hash,
        "reservation_id": reservation_id,
        "source_receipt_hash": ZERO_HASH48,
        "source_receipt_root": ZERO_HASH48,
        "settlement_asset_id": status["settlement_asset_id"],
        "native_nav_asset_id": status["native_nav_asset_id"],
        "pricing_reserve_packet_hash": status["pricing_reserve_packet_hash"],
        "policy_hash_commitment": policy_commitment,
        "route_epoch": status["route_epoch"],
        "pricing_nav_epoch": status["pricing_nav_epoch"],
        "deadline_seconds": deadline,
        "nonce": export_nonce,
        "destination_chain_id": status["ethereum_chain_id"],
        "destination_controller": status["handoff_controller"],
        "wrapped_token": status["wrapped_navcoin_token"],
        "ethereum_recipient": JOE_EVM,
        "mint_amount_atoms": amount,
        "settlement_value_atoms": settlement,
    }

    args.output_dir.mkdir(parents=True, mode=0o700)
    reserve_file = args.output_dir / "01-reserve.ops.json"
    subscribe_file = args.output_dir / "02-subscribe.ops.json"
    packet_file = args.output_dir / "03-mint-packet.json"
    digest_file = args.output_dir / "03-mint-packet-digest.json"
    export_file = args.output_dir / "03-export.ops.json"
    write_json(
        reserve_file,
        ops("joe-a666-reserve", args.holder_key_file, reserve_operation),
    )
    write_json(
        subscribe_file,
        ops("joe-a666-subscribe", args.holder_key_file, subscribe_operation),
    )
    write_json(packet_file, mint_packet)
    digest_process = subprocess.run(
        [
            str(args.node_bin),
            "pftl-uniswap-mint-packet-digest",
            "--packet-file",
            str(packet_file),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    digest_report = json.loads(digest_process.stdout)
    if digest_report.get("packet") != mint_packet:
        raise RuntimeError("digest tool did not round-trip the exact mint packet")
    write_json(digest_file, digest_report)
    packet_digest = digest_report["packet_digest"]
    export_operation = {
        "operation": "pftl_uniswap_export_debit",
        "owner": JOE_PFTL,
        "route_id": ROUTE_ID,
        "packet_hash": packet_hash,
        "export_nonce": export_nonce,
        "ethereum_recipient": JOE_EVM,
        "amount_atoms": amount,
        "reservation_id": reservation_id,
        "settlement_value_atoms": settlement,
        "destination_deadline_seconds": deadline,
        "refund_delay_blocks": args.refund_delay_blocks,
        "ethereum_packet_digest": packet_digest,
        "ethereum_packet_schema_version": 2,
    }
    write_json(
        export_file,
        ops("joe-a666-export", args.holder_key_file, export_operation),
    )
    manifest = {
        "schema": "postfiat.a666.transparent_primary_issue_ops.v1",
        "route_id": ROUTE_ID,
        "subscriber": JOE_PFTL,
        "ethereum_recipient": JOE_EVM,
        "mint_amount_atoms": amount,
        "settlement_value_atoms": settlement,
        "reservation_id": reservation_id,
        "subscription_nonce": subscription_nonce,
        "packet_hash": packet_hash,
        "export_nonce": export_nonce,
        "ethereum_packet_digest": packet_digest,
        "destination_deadline_seconds": deadline,
        "files": {
            "reserve": str(reserve_file),
            "subscribe": str(subscribe_file),
            "mint_packet": str(packet_file),
            "mint_packet_digest": str(digest_file),
            "export": str(export_file),
        },
    }
    write_json(args.output_dir / "manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
