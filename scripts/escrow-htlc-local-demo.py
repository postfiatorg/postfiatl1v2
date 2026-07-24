#!/usr/bin/env python3
"""Run a no-value local PREIMAGE-SHA-256 escrow finish/refund transcript."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))

from postfiat_rpc.client import PostFiatRpcClient, RpcError  # noqa: E402
from postfiat_rpc.wallet import (  # noqa: E402
    cancel_escrow,
    create_escrow,
    create_wallet,
    finish_escrow,
    request_faucet_pft,
)

CHAIN_ID = "postfiat-escrow-htlc-local"
PREIMAGE = bytes(range(32))


def condition_for(preimage: bytes) -> str:
    return f"a0258020{hashlib.sha256(preimage).hexdigest()}810120"


def fulfillment_for(preimage: bytes) -> str:
    return f"a0228020{preimage.hex()}"


def reserve_port() -> int:
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def run_json(*args: str) -> dict:
    completed = subprocess.run(
        [str(ROOT / "target/debug/postfiat-node"), *args],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return json.loads(completed.stdout)


def receipt(result) -> dict:
    rows = list(result.receipts_by_validator)
    if len(rows) != 1 or not isinstance(rows[0], list) or len(rows[0]) != 1:
        raise RuntimeError(f"unexpected receipt shape: {rows!r}")
    return rows[0][0]


def wait_ready(ready_file: Path, process: subprocess.Popen[str]) -> None:
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"rpc-serve exited with {process.returncode}")
        if ready_file.exists():
            return
        time.sleep(0.05)
    raise RuntimeError("rpc-serve did not become ready")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="pftl-escrow-htlc-local-") as temporary:
        temp = Path(temporary)
        data_dir = temp / "node0"
        work_dir = temp / "work"
        wallets = temp / "wallets"
        ready_file = temp / "rpc-ready.json"
        rpc_log = temp / "rpc.log"

        init = run_json(
            "init",
            "--data-dir",
            str(data_dir),
            "--chain-id",
            CHAIN_ID,
            "--node-id",
            "validator-0",
            "--validators",
            "1",
        )
        alice = create_wallet(chain_id=CHAIN_ID, wallet_dir=wallets / "alice")
        bob = create_wallet(chain_id=CHAIN_ID, wallet_dir=wallets / "bob")
        for wallet in (alice, bob):
            request_faucet_pft(
                data_dir=data_dir,
                to_address=wallet.address,
                amount=100_000,
                validator_data_dirs=[data_dir],
                work_dir=work_dir,
            )

        port = reserve_port()
        with rpc_log.open("w") as log:
            server = subprocess.Popen(
                [
                    str(ROOT / "target/debug/postfiat-node"),
                    "rpc-serve",
                    "--unsafe-devnet-json-storage",
                    "--data-dir",
                    str(data_dir),
                    "--port",
                    str(port),
                    "--bind-host",
                    "127.0.0.1",
                    "--ready-file",
                    str(ready_file),
                    "--allow-mempool-submit",
                    "--max-requests",
                    "100",
                    "--keep-alive",
                ],
                cwd=ROOT,
                text=True,
                stdout=log,
                stderr=subprocess.STDOUT,
            )
            try:
                wait_ready(ready_file, server)
                client = PostFiatRpcClient(f"127.0.0.1:{port}", timeout_seconds=20)
                condition = condition_for(PREIMAGE)
                fulfillment = fulfillment_for(PREIMAGE)

                finish_create = create_escrow(
                    client,
                    owner_wallet=alice,
                    destination=bob.address,
                    amount=10_000,
                    condition=condition,
                    finish_after=0,
                    cancel_after=20,
                    work_dir=work_dir,
                    finalize_data_dir=data_dir,
                    validator_data_dirs=[data_dir],
                )
                finish_create_receipt = receipt(finish_create)
                finish_open = client.escrow_info(finish_create.escrow_id)

                try:
                    finish_escrow(
                        client,
                        recipient_wallet=bob,
                        escrow_id=finish_create.escrow_id,
                        owner=alice.address,
                        fulfillment=fulfillment_for(bytes([0xFF]) * 32),
                        work_dir=work_dir,
                        finalize_data_dir=data_dir,
                        validator_data_dirs=[data_dir],
                    )
                except RpcError as error:
                    wrong_finish_rejection = {
                        "method": "mempool_submit_signed_escrow_transaction",
                        "error": error.error,
                    }
                else:
                    raise RuntimeError("wrong hash preimage was not rejected")

                finish_result = finish_escrow(
                    client,
                    recipient_wallet=bob,
                    escrow_id=finish_create.escrow_id,
                    owner=alice.address,
                    fulfillment=fulfillment,
                    work_dir=work_dir,
                    finalize_data_dir=data_dir,
                    validator_data_dirs=[data_dir],
                )
                finish_receipt = receipt(finish_result)
                finish_closed = client.escrow_info(finish_create.escrow_id)
                finish_tx = client.tx(finish_result.tx_id, audit_block_log=True)
                finish_header = finish_tx["block"]["header"]
                finish_archive = client.batch_archive(
                    batch_kind=finish_header["batch_kind"],
                    batch_id=finish_header["batch_id"],
                    limit=1,
                )
                finish_batch = json.loads(finish_archive[0]["payload_json"])
                finish_operation = finish_batch["escrow_transactions"][0]["unsigned"]
                disclosed_fulfillment = finish_operation["fulfillment"]
                disclosed_preimage = disclosed_fulfillment[len("a0228020") :]
                if disclosed_preimage != PREIMAGE.hex():
                    raise RuntimeError("finalized block did not disclose the expected preimage")

                cancel_after = int(client.status()["block_height"]) + 3
                balance_before_cancel_create = int(client.account(alice.address)["balance"])
                cancel_create = create_escrow(
                    client,
                    owner_wallet=alice,
                    destination=bob.address,
                    amount=7_000,
                    condition=condition,
                    finish_after=0,
                    cancel_after=cancel_after,
                    work_dir=work_dir,
                    finalize_data_dir=data_dir,
                    validator_data_dirs=[data_dir],
                )
                cancel_create_receipt = receipt(cancel_create)
                balance_locked = int(client.account(alice.address)["balance"])

                try:
                    cancel_escrow(
                        client,
                        owner_wallet=alice,
                        escrow_id=cancel_create.escrow_id,
                        work_dir=work_dir,
                        finalize_data_dir=data_dir,
                        validator_data_dirs=[data_dir],
                    )
                except RpcError as error:
                    early_cancel_rejection = {
                        "method": "mempool_submit_signed_escrow_transaction",
                        "error": error.error,
                    }
                    if "escrow_cancel_too_early" not in str(error.error):
                        raise RuntimeError("early cancel failed for an unexpected reason") from error
                else:
                    raise RuntimeError("early cancel did not fail at the timelock")

                request_faucet_pft(
                    data_dir=data_dir,
                    to_address=bob.address,
                    amount=1,
                    validator_data_dirs=[data_dir],
                    work_dir=work_dir,
                )
                cancel_result = cancel_escrow(
                    client,
                    owner_wallet=alice,
                    escrow_id=cancel_create.escrow_id,
                    work_dir=work_dir,
                    finalize_data_dir=data_dir,
                    validator_data_dirs=[data_dir],
                )
                cancel_receipt = receipt(cancel_result)
                cancel_closed = client.escrow_info(cancel_create.escrow_id)
                balance_refunded = int(client.account(alice.address)["balance"])
                cancel_fee = int(cancel_result.signed_escrow_transaction["unsigned"]["fee"])
                if balance_refunded != balance_locked + 7_000 - cancel_fee:
                    raise RuntimeError("cancel path did not refund the locked amount net of fee")

                transcript = {
                    "schema": "postfiat-local-preimage-sha256-escrow-demo-v1",
                    "scope": "fresh one-validator local JSON devnet; no shared ce22 state",
                    "build_git_revision": init["build_git_revision"],
                    "chain_id": CHAIN_ID,
                    "genesis_hash": init["genesis_hash"],
                    "rpc_capabilities": client.server_capabilities(),
                    "participants": {"owner": alice.address, "recipient": bob.address},
                    "crypto_condition": {
                        "type": "PREIMAGE-SHA-256",
                        "preimage_hex": PREIMAGE.hex(),
                        "payment_hash_sha256": hashlib.sha256(PREIMAGE).hexdigest(),
                        "condition": condition,
                        "fulfillment": fulfillment,
                    },
                    "finish_path": {
                        "create_tx_id": finish_create.tx_id,
                        "create_submit": finish_create.submit_result,
                        "create_receipt": finish_create_receipt,
                        "open_escrow": finish_open,
                        "wrong_preimage_rejection": wrong_finish_rejection,
                        "finish_tx_id": finish_result.tx_id,
                        "finish_submit": finish_result.submit_result,
                        "finish_receipt": finish_receipt,
                        "finished_escrow": finish_closed,
                        "tx_finality": finish_tx,
                        "finalized_block_operation": finish_operation,
                        "publicly_disclosed_preimage_hex": disclosed_preimage,
                    },
                    "cancel_path": {
                        "cancel_after": cancel_after,
                        "create_tx_id": cancel_create.tx_id,
                        "create_receipt": cancel_create_receipt,
                        "balance_before_create": balance_before_cancel_create,
                        "balance_while_locked": balance_locked,
                        "early_cancel_rejection": early_cancel_rejection,
                        "cancel_tx_id": cancel_result.tx_id,
                        "cancel_receipt": cancel_receipt,
                        "canceled_escrow": cancel_closed,
                        "cancel_fee": cancel_fee,
                        "balance_after_refund": balance_refunded,
                        "refund_equation": (
                            f"{balance_locked} + 7000 - {cancel_fee} = {balance_refunded}"
                        ),
                    },
                    "final_status": client.status(),
                }
                args.output.write_text(json.dumps(transcript, indent=2) + "\n")
                os.chmod(args.output, 0o644)
            finally:
                server.terminate()
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)

    print(json.dumps({"ok": True, "transcript": str(args.output)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
