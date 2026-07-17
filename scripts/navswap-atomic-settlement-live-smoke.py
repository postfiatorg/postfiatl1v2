#!/usr/bin/env python3
"""Tiny live ESCROW-009 smoke for the NAVSwap atomic-settlement path.

The default mode is read-only. Passing ``--execute`` signs and submits a
1-atom PFT <-> a651 settlement with the configured local key files.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "python"))

from postfiat_rpc import PostFiatRpcClient, PostFiatWebSocketRpcClient  # noqa: E402
from postfiat_rpc.wallet import (  # noqa: E402
    TransparentWallet,
    build_atomic_settlement_template,
    execute_atomic_settlement,
)


A651_ASSET_ID = "dcddbf56e7e15f7893d0038e8e0e6089d5a41418dead75353aabb8c016cf626beeb93bc802929f29883c078d910f59d5"
DEFAULT_RPC_URL = "ws://127.0.0.1:8080/rpc"
MAX_LIVE_ATOMS = 100


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def json_default(value: Any) -> Any:
    if dataclasses.is_dataclass(value):
        return dataclasses.asdict(value)
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, tuple):
        return list(value)
    return str(value)


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True, default=json_default) + "\n")


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rpc-url", default=DEFAULT_RPC_URL)
    parser.add_argument(
        "--endpoint",
        help="Direct TCP RPC endpoint. Overrides --rpc-url; useful for read-only dry-runs.",
    )
    parser.add_argument("--left-key-file", type=Path, required=True)
    parser.add_argument("--right-key-file", type=Path, required=True)
    parser.add_argument("--left-asset-id", default="PFT")
    parser.add_argument("--right-asset-id", default=A651_ASSET_ID)
    parser.add_argument("--left-amount", type=positive_int, default=1)
    parser.add_argument("--right-amount", type=positive_int, default=1)
    parser.add_argument("--condition", default=f"navswap-atomic-live-smoke-{utc_stamp()}")
    parser.add_argument("--cancel-height-delta", type=positive_int, default=120)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--max-live-atoms", type=positive_int, default=MAX_LIVE_ATOMS)
    parser.add_argument("--timeout-seconds", type=float, default=90.0)
    parser.add_argument("--poll-seconds", type=float, default=1.5)
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("/tmp") / f"navswap-atomic-settlement-live-smoke-{utc_stamp()}",
    )
    return parser.parse_args()


def load_key_wallet(path: Path, chain_id: str) -> TransparentWallet:
    report = json.loads(path.read_text(encoding="utf-8"))
    address = report.get("address")
    public_key_hex = report.get("public_key_hex")
    if not isinstance(address, str) or not address.startswith("pf"):
        raise ValueError(f"{path} missing wallet address")
    if not isinstance(public_key_hex, str) or not public_key_hex:
        raise ValueError(f"{path} missing public_key_hex")
    return TransparentWallet(
        chain_id=chain_id,
        account_index=0,
        address=address,
        public_key_hex=public_key_hex,
        key_file=path,
        backup_file=path.with_name(path.name.replace(".key.json", ".backup.json")),
        key_report={"address": address, "public_key_hex": public_key_hex},
    )


def asset_balance(client: PostFiatRpcClient, address: str, asset_id: str) -> int:
    if asset_id == "PFT":
        return int(client.account(address).get("balance", 0))
    assets = client.account_assets(address, asset_id=asset_id)
    total = 0
    for item in assets.get("assets", []):
        if item.get("asset_id") == asset_id or item.get("id") == asset_id:
            total += int(item.get("balance", item.get("amount", 0)))
    return total


def wallet_snapshot(client: PostFiatRpcClient, address: str, asset_ids: list[str]) -> dict[str, Any]:
    account = client.account(address)
    assets = client.account_assets(address)
    return {
        "address": address,
        "account": {
            "balance": account.get("balance"),
            "sequence": account.get("sequence"),
        },
        "balances": {asset_id: asset_balance(client, address, asset_id) for asset_id in asset_ids},
        "assets": assets,
    }


def wait_receipt(
    client: PostFiatRpcClient,
    tx_id: str | None,
    *,
    timeout_seconds: float,
    poll_seconds: float,
) -> list[dict[str, Any]]:
    if not tx_id:
        raise ValueError("missing tx_id")
    deadline = time.monotonic() + timeout_seconds
    while True:
        receipts = client.receipts(tx_id=tx_id, limit=10)
        if receipts:
            return receipts
        if time.monotonic() >= deadline:
            raise TimeoutError(f"timed out waiting for receipt {tx_id}")
        time.sleep(min(poll_seconds, max(0.0, deadline - time.monotonic())))


def wait_escrow_terminal(
    client: PostFiatRpcClient,
    escrow_id: str,
    *,
    timeout_seconds: float,
    poll_seconds: float,
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout_seconds
    last: dict[str, Any] | None = None
    while True:
        info = client.escrow_info(escrow_id)
        last = info
        escrow = info.get("escrow")
        state = escrow.get("state") if isinstance(escrow, dict) else None
        if state in {"finished", "canceled"}:
            return info
        if time.monotonic() >= deadline:
            raise TimeoutError(f"escrow {escrow_id} did not reach terminal state; last={last}")
        time.sleep(min(poll_seconds, max(0.0, deadline - time.monotonic())))


def assert_live_limits(args: argparse.Namespace) -> None:
    if args.left_amount > args.max_live_atoms or args.right_amount > args.max_live_atoms:
        raise ValueError(
            f"live atomic smoke capped at {args.max_live_atoms} atoms per leg; "
            "raise --max-live-atoms only for an intentional larger test"
        )
    if args.left_asset_id == args.right_asset_id:
        raise ValueError("left and right assets must differ")
    if (args.left_asset_id == "PFT") == (args.right_asset_id == "PFT"):
        raise ValueError("ESCROW-009 smoke requires exactly one PFT leg")


def main() -> None:
    args = parse_args()
    assert_live_limits(args)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    if args.endpoint:
        client = PostFiatRpcClient(args.endpoint, timeout_seconds=15.0)
        rpc_target = args.endpoint
    else:
        client = PostFiatWebSocketRpcClient(args.rpc_url, timeout_seconds=15.0)
        rpc_target = args.rpc_url
    status = client.status()
    chain_id = str(status.get("chain_id") or "")
    current_height = int(status.get("block_height") or 0)
    cancel_after = current_height + args.cancel_height_delta
    left_wallet = load_key_wallet(args.left_key_file, chain_id)
    right_wallet = load_key_wallet(args.right_key_file, chain_id)
    asset_ids = sorted({args.left_asset_id, args.right_asset_id})

    request = {
        "rpc_target": rpc_target,
        "execute": args.execute,
        "chain_id": chain_id,
        "current_height": current_height,
        "cancel_after": cancel_after,
        "left_wallet": left_wallet.address,
        "right_wallet": right_wallet.address,
        "left_asset_id": args.left_asset_id,
        "left_amount": args.left_amount,
        "right_asset_id": args.right_asset_id,
        "right_amount": args.right_amount,
        "condition": args.condition,
        "max_live_atoms": args.max_live_atoms,
    }
    write_json(args.out_dir / "request.json", request)

    before = {
        "left": wallet_snapshot(client, left_wallet.address, asset_ids),
        "right": wallet_snapshot(client, right_wallet.address, asset_ids),
        "mempool": client.mempool_status(),
    }
    write_json(args.out_dir / "before.json", before)

    template = build_atomic_settlement_template(
        client,
        left_wallet=left_wallet,
        right_wallet=right_wallet,
        left_asset_id=args.left_asset_id,
        left_amount=args.left_amount,
        right_asset_id=args.right_asset_id,
        right_amount=args.right_amount,
        condition=args.condition,
        cancel_after=cancel_after,
    )
    write_json(args.out_dir / "template.json", template)

    if not args.execute:
        summary = {
            "ok": True,
            "mode": "dry-run",
            "out_dir": str(args.out_dir),
            "settlement_id": template.settlement_id,
            "left_escrow_id": template.left_escrow_id,
            "right_escrow_id": template.right_escrow_id,
            "message": "Template built only. Re-run with --execute to sign and submit tiny live escrows.",
        }
        write_json(args.out_dir / "summary.json", summary)
        print(json.dumps(summary, indent=2, sort_keys=True))
        return

    result = execute_atomic_settlement(
        client,
        left_wallet=left_wallet,
        right_wallet=right_wallet,
        left_asset_id=args.left_asset_id,
        left_amount=args.left_amount,
        right_asset_id=args.right_asset_id,
        right_amount=args.right_amount,
        condition=args.condition,
        cancel_after=cancel_after,
        submit_finality=True,
        wait_for_create_timeout_seconds=args.timeout_seconds,
        wait_for_create_poll_seconds=args.poll_seconds,
        work_dir=args.out_dir / "work",
    )
    write_json(args.out_dir / "execution-result.json", result)

    tx_ids = {
        "left_create": result.left_create.tx_id,
        "right_create": result.right_create.tx_id,
        "left_finish": result.left_finish.tx_id,
        "right_finish": result.right_finish.tx_id,
    }
    receipts = {
        label: wait_receipt(
            client,
            tx_id,
            timeout_seconds=args.timeout_seconds,
            poll_seconds=args.poll_seconds,
        )
        for label, tx_id in tx_ids.items()
    }
    write_json(args.out_dir / "receipts.json", receipts)
    escrows = {
        "left": wait_escrow_terminal(
            client,
            result.template.left_escrow_id,
            timeout_seconds=args.timeout_seconds,
            poll_seconds=args.poll_seconds,
        ),
        "right": wait_escrow_terminal(
            client,
            result.template.right_escrow_id,
            timeout_seconds=args.timeout_seconds,
            poll_seconds=args.poll_seconds,
        ),
    }
    write_json(args.out_dir / "escrows-after.json", escrows)
    after = {
        "left": wallet_snapshot(client, left_wallet.address, asset_ids),
        "right": wallet_snapshot(client, right_wallet.address, asset_ids),
        "mempool": client.mempool_status(),
    }
    write_json(args.out_dir / "after.json", after)
    summary = {
        "ok": True,
        "mode": "execute",
        "out_dir": str(args.out_dir),
        "settlement_id": result.settlement_id,
        "left_escrow_id": result.template.left_escrow_id,
        "right_escrow_id": result.template.right_escrow_id,
        "tx_ids": tx_ids,
        "left_receipt_count": len(receipts["left_create"]) + len(receipts["left_finish"]),
        "right_receipt_count": len(receipts["right_create"]) + len(receipts["right_finish"]),
        "left_terminal_state": escrows["left"].get("escrow", {}).get("state"),
        "right_terminal_state": escrows["right"].get("escrow", {}).get("state"),
        "before_balances": {
            "left": before["left"]["balances"],
            "right": before["right"]["balances"],
        },
        "after_balances": {
            "left": after["left"]["balances"],
            "right": after["right"]["balances"],
        },
    }
    write_json(args.out_dir / "summary.json", summary)
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
