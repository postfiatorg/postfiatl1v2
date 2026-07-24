#!/usr/bin/env python3
"""Generate isolated six-validator evidence for the PREIMAGE-SHA-256 escrow primitive."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from typing import Any, Callable

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))

from postfiat_rpc.client import PostFiatRpcClient, RpcError  # noqa: E402
from postfiat_rpc.wallet import (  # noqa: E402
    cancel_escrow,
    create_asset_trustline,
    create_escrow,
    create_issued_asset,
    create_wallet,
    finish_escrow,
    request_faucet_pft,
    send_issued_asset,
)

CHAIN_ID = "postfiat-escrow-htlc-six-validator-evidence"
PREIMAGE = bytes(range(32))
WRONG_PREIMAGE = bytes([0xFF]) * 32
VALIDATOR_COUNT = 6


def condition_for(preimage: bytes) -> str:
    return f"a0258020{hashlib.sha256(preimage).hexdigest()}810120"


def fulfillment_for(preimage: bytes) -> str:
    return f"a0228020{preimage.hex()}"


def reserve_port() -> int:
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def run_json(*args: str) -> dict[str, Any]:
    result = subprocess.run(
        [str(ROOT / "target/debug/postfiat-node"), *args],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return json.loads(result.stdout)


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, 0o644)


def wait_ready(ready_file: Path, process: subprocess.Popen[str]) -> None:
    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"rpc-serve exited with {process.returncode}")
        if ready_file.exists():
            return
        time.sleep(0.05)
    raise RuntimeError("rpc-serve did not become ready")


def statuses(node_dirs: list[Path]) -> list[dict[str, Any]]:
    return [run_json("status", "--data-dir", str(node)) for node in node_dirs]


def convergence(node_dirs: list[Path], label: str) -> dict[str, Any]:
    rows = statuses(node_dirs)
    points = {
        (row["block_height"], row["state_root"], row["block_tip_hash"])
        for row in rows
    }
    if len(points) != 1:
        raise RuntimeError(f"six-validator divergence at {label}: {rows}")
    return {
        "label": label,
        "validator_count": len(rows),
        "converged": True,
        "validators": [
            {
                "validator": index,
                "height": row["block_height"],
                "state_root": row["state_root"],
                "block_tip_hash": row["block_tip_hash"],
            }
            for index, row in enumerate(rows)
        ],
    }


def accepted_result(result: Any, label: str, node_dirs: list[Path]) -> dict[str, Any]:
    rows = list(result.receipts_by_validator)
    if len(rows) != VALIDATOR_COUNT or any(len(row) != 1 for row in rows):
        raise RuntimeError(f"unexpected receipt shape for {label}: {rows!r}")
    receipts = [row[0] for row in rows]
    identities = {
        (row.get("tx_id"), row.get("accepted"), row.get("code")) for row in receipts
    }
    if len(identities) != 1 or receipts[0].get("accepted") is not True:
        raise RuntimeError(f"non-identical/non-accepted receipts for {label}: {receipts!r}")
    return {
        "label": label,
        "tx_id": result.tx_id,
        "receipt_code": receipts[0].get("code"),
        "accepted": True,
        "receipts_by_validator": receipts,
        "convergence": convergence(node_dirs, label),
    }


def rejection(
    label: str,
    action: Callable[[], Any],
    expected_code: str,
    node_dirs: list[Path],
) -> dict[str, Any]:
    before = convergence(node_dirs, f"{label}-before")
    try:
        action()
    except RpcError as error:
        detail = error.error
    else:
        raise RuntimeError(f"{label} unexpectedly succeeded")
    if expected_code not in str(detail):
        raise RuntimeError(f"{label} expected {expected_code}, got {detail!r}")
    after = convergence(node_dirs, f"{label}-after")
    before_point = before["validators"][0]
    after_point = after["validators"][0]
    for key in ("height", "state_root", "block_tip_hash"):
        if before_point[key] != after_point[key]:
            raise RuntimeError(f"{label} mutated state on rejection")
    return {
        "label": label,
        "expected_code": expected_code,
        "rpc_error": detail,
        "mutation_free": True,
        "state_root": before_point["state_root"],
        "height": before_point["height"],
    }


def account_balance(client: PostFiatRpcClient, address: str) -> int:
    return int(client.account(address)["balance"])


def asset_line(client: PostFiatRpcClient, address: str, asset_id: str) -> dict[str, Any]:
    rows = client.account_lines(address, asset_id=asset_id, limit=2)["lines"]
    if len(rows) != 1:
        raise RuntimeError(f"expected one asset line for {address}/{asset_id}: {rows}")
    return rows[0]


def open_locked(client: PostFiatRpcClient, owner: str, asset_id: str) -> int:
    rows = client.account_escrows(owner, role="owner", state="open", limit=100)["escrows"]
    return sum(int(row["amount"]) for row in rows if row["asset_id"] == asset_id)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="pftl-escrow-htlc-six-") as temporary:
        temp = Path(temporary)
        node_dirs = [temp / f"node{index}" for index in range(VALIDATOR_COUNT)]
        work_dir = temp / "work"
        wallets_dir = temp / "wallets"
        ready_file = temp / "rpc-ready.json"
        rpc_log = temp / "rpc.log"
        init = run_json(
            "init",
            "--data-dir",
            str(node_dirs[0]),
            "--chain-id",
            CHAIN_ID,
            "--node-id",
            "validator-0",
            "--validators",
            str(VALIDATOR_COUNT),
        )
        for index, node_dir in enumerate(node_dirs[1:], 1):
            shutil.copytree(node_dirs[0], node_dir)
            state_file = node_dir / "node_state.json"
            state = json.loads(state_file.read_text())
            state["node_id"] = f"validator-{index}"
            write_json(state_file, state)

        owner = create_wallet(chain_id=CHAIN_ID, wallet_dir=wallets_dir / "owner")
        recipient = create_wallet(chain_id=CHAIN_ID, wallet_dir=wallets_dir / "recipient")
        issuer = create_wallet(chain_id=CHAIN_ID, wallet_dir=wallets_dir / "issuer")
        fund_receipts = []
        for wallet in (owner, recipient, issuer):
            funded = request_faucet_pft(
                data_dir=node_dirs[0],
                to_address=wallet.address,
                amount=2_000_000,
                validator_data_dirs=node_dirs,
                work_dir=work_dir,
            )
            fund_receipts.append(accepted_result(funded, f"fund-{wallet.address}", node_dirs))

        port = reserve_port()
        with rpc_log.open("w") as log:
            server = subprocess.Popen(
                [
                    str(ROOT / "target/debug/postfiat-node"),
                    "rpc-serve",
                    "--unsafe-devnet-json-storage",
                    "--data-dir",
                    str(node_dirs[0]),
                    "--port",
                    str(port),
                    "--bind-host",
                    "127.0.0.1",
                    "--ready-file",
                    str(ready_file),
                    "--allow-mempool-submit",
                    "--max-mempool-submit-per-peer",
                    "1000",
                    "--max-mempool-submit-total",
                    "1000",
                    "--max-requests",
                    "1000",
                    "--keep-alive",
                ],
                cwd=ROOT,
                text=True,
                stdout=log,
                stderr=subprocess.STDOUT,
            )
            try:
                wait_ready(ready_file, server)
                client = PostFiatRpcClient(f"127.0.0.1:{port}", timeout_seconds=30)
                condition = condition_for(PREIMAGE)
                fulfillment = fulfillment_for(PREIMAGE)
                evidence: dict[str, Any] = {
                    "schema": "postfiat-preimage-sha256-escrow-six-validator-evidence-v1",
                    "scope": "fresh isolated six-validator local JSON devnet; no ce22/mainnet/value",
                    "chain_id": CHAIN_ID,
                    "genesis_hash": init["genesis_hash"],
                    "build_git_revision": init["build_git_revision"],
                    "vectors": {
                        "preimage_hex": PREIMAGE.hex(),
                        "sha256": hashlib.sha256(PREIMAGE).hexdigest(),
                        "condition": condition,
                        "fulfillment": fulfillment,
                    },
                    "funding": fund_receipts,
                    "rejections": [],
                    "accepted_transitions": [],
                }

                # Typed-condition admission failures are signed but never enter state.
                malformed_conditions = [
                    ("uppercase-condition", condition.upper(), "canonical lowercase hex"),
                    ("wrong-condition-type", "a1" + condition[2:], "unsupported escrow crypto-condition type"),
                    ("malformed-condition", condition[:-2], "invalid PREIMAGE-SHA-256 escrow condition encoding"),
                    ("oversized-condition", "a0" + "00" * 300, "must not exceed"),
                ]
                for label, malformed, expected in malformed_conditions:
                    evidence["rejections"].append(
                        rejection(
                            label,
                            lambda malformed=malformed: create_escrow(
                                client,
                                owner_wallet=owner,
                                destination=recipient.address,
                                amount=1_000,
                                condition=malformed,
                                cancel_after=int(client.status()["block_height"]) + 4,
                                work_dir=work_dir,
                            ),
                            expected,
                            node_dirs,
                        )
                    )
                for label, cancel_after, code in (
                    ("typed-missing-cancel", 0, "escrow_cancel_after_required"),
                    (
                        "typed-short-window",
                        int(client.status()["block_height"]) + 1,
                        "escrow_claim_window_too_short",
                    ),
                ):
                    evidence["rejections"].append(
                        rejection(
                            label,
                            lambda cancel_after=cancel_after: create_escrow(
                                client,
                                owner_wallet=owner,
                                destination=recipient.address,
                                amount=1_000,
                                condition=condition,
                                cancel_after=cancel_after,
                                work_dir=work_dir,
                            ),
                            code,
                            node_dirs,
                        )
                    )

                # Native finish path and public preimage disclosure.
                native_before = {
                    "owner": account_balance(client, owner.address),
                    "recipient": account_balance(client, recipient.address),
                    "locked": open_locked(client, owner.address, "PFT"),
                }
                create = create_escrow(
                    client,
                    owner_wallet=owner,
                    destination=recipient.address,
                    amount=100_000,
                    condition=condition,
                    cancel_after=int(client.status()["block_height"]) + 5,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(create, "native-create-finish-path", node_dirs)
                )
                evidence["rejections"].append(
                    rejection(
                        "duplicate-native-create",
                        lambda: client.mempool_submit_signed_escrow_transaction(
                            create.signed_escrow_transaction
                        ),
                        "bad_sequence",
                        node_dirs,
                    )
                )
                for label, bad_fulfillment, code in (
                    ("empty-fulfillment", "", "invalid_escrow_fulfillment"),
                    ("uppercase-fulfillment", fulfillment.upper(), "invalid_escrow_fulfillment"),
                    ("wrong-fulfillment-type", "a1" + fulfillment[2:], "invalid_escrow_fulfillment"),
                    ("non-32-byte-fulfillment", fulfillment[:-2], "invalid_escrow_fulfillment"),
                    ("wrong-preimage", fulfillment_for(WRONG_PREIMAGE), "escrow_condition_unsatisfied"),
                ):
                    evidence["rejections"].append(
                        rejection(
                            label,
                            lambda bad_fulfillment=bad_fulfillment: finish_escrow(
                                client,
                                recipient_wallet=recipient,
                                escrow_id=create.escrow_id,
                                owner=owner.address,
                                fulfillment=bad_fulfillment,
                                work_dir=work_dir,
                            ),
                            code,
                            node_dirs,
                        )
                    )
                finish = finish_escrow(
                    client,
                    recipient_wallet=recipient,
                    escrow_id=create.escrow_id,
                    owner=owner.address,
                    fulfillment=fulfillment,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(finish, "native-finish", node_dirs)
                )
                finish_tx = client.tx(finish.tx_id, audit_block_log=True)
                header = finish_tx["block"]["header"]
                archive = client.batch_archive(
                    batch_kind=header["batch_kind"], batch_id=header["batch_id"], limit=1
                )
                operation = json.loads(archive[0]["payload_json"])["escrow_transactions"][0]["unsigned"]
                disclosed = operation["fulfillment"][len("a0228020") :]
                if disclosed != PREIMAGE.hex():
                    raise RuntimeError("finalized block did not disclose the preimage")
                evidence["rejections"].append(
                    rejection(
                        "duplicate-native-finish",
                        lambda: client.mempool_submit_signed_escrow_transaction(
                            finish.signed_escrow_transaction
                        ),
                        "bad_sequence",
                        node_dirs,
                    )
                )
                native_after = {
                    "owner": account_balance(client, owner.address),
                    "recipient": account_balance(client, recipient.address),
                    "locked": open_locked(client, owner.address, "PFT"),
                }
                create_fee = int(create.signed_escrow_transaction["unsigned"]["fee"])
                finish_fee = int(finish.signed_escrow_transaction["unsigned"]["fee"])
                if native_after["owner"] != native_before["owner"] - 100_000 - create_fee:
                    raise RuntimeError("native create principal/fee equation failed")
                if native_after["recipient"] != native_before["recipient"] + 100_000 - finish_fee:
                    raise RuntimeError("native finish principal/fee equation failed")
                evidence["native_finish"] = {
                    "before": native_before,
                    "after": native_after,
                    "principal": 100_000,
                    "create_fee": create_fee,
                    "finish_fee": finish_fee,
                    "principal_conserved": True,
                    "publicly_disclosed_preimage_hex": disclosed,
                    "finalized_operation": operation,
                }

                # Legacy conditions remain exact-match opaque values.
                legacy = create_escrow(
                    client,
                    owner_wallet=owner,
                    destination=recipient.address,
                    amount=2_000,
                    condition="legacy-opaque-secret",
                    cancel_after=int(client.status()["block_height"]) + 4,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(legacy, "legacy-create", node_dirs)
                )
                evidence["rejections"].append(
                    rejection(
                        "legacy-nonexact",
                        lambda: finish_escrow(
                            client,
                            recipient_wallet=recipient,
                            escrow_id=legacy.escrow_id,
                            owner=owner.address,
                            fulfillment="LEGACY-OPAQUE-SECRET",
                            work_dir=work_dir,
                        ),
                        "escrow_condition_unsatisfied",
                        node_dirs,
                    )
                )
                legacy_finish = finish_escrow(
                    client,
                    recipient_wallet=recipient,
                    escrow_id=legacy.escrow_id,
                    owner=owner.address,
                    fulfillment="legacy-opaque-secret",
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(legacy_finish, "legacy-finish", node_dirs)
                )

                # Native cancel boundary: late finish loses exactly at cancel_after.
                cancel_before = account_balance(client, owner.address)
                cancel_after = int(client.status()["block_height"]) + 3
                cancel_create = create_escrow(
                    client,
                    owner_wallet=owner,
                    destination=recipient.address,
                    amount=70_000,
                    condition=condition,
                    cancel_after=cancel_after,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(cancel_create, "native-create-cancel-path", node_dirs)
                )
                cancel_locked = account_balance(client, owner.address)
                evidence["rejections"].append(
                    rejection(
                        "native-early-cancel",
                        lambda: cancel_escrow(
                            client,
                            owner_wallet=owner,
                            escrow_id=cancel_create.escrow_id,
                            work_dir=work_dir,
                        ),
                        "escrow_cancel_too_early",
                        node_dirs,
                    )
                )
                filler = request_faucet_pft(
                    data_dir=node_dirs[0],
                    to_address=recipient.address,
                    amount=1,
                    validator_data_dirs=node_dirs,
                    work_dir=work_dir,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(filler, "cancel-boundary-filler", node_dirs)
                )
                evidence["rejections"].append(
                    rejection(
                        "native-finish-at-cancel-boundary",
                        lambda: finish_escrow(
                            client,
                            recipient_wallet=recipient,
                            escrow_id=cancel_create.escrow_id,
                            owner=owner.address,
                            fulfillment=fulfillment,
                            work_dir=work_dir,
                        ),
                        "escrow_finish_expired",
                        node_dirs,
                    )
                )
                cancel = cancel_escrow(
                    client,
                    owner_wallet=owner,
                    escrow_id=cancel_create.escrow_id,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(cancel, "native-cancel-at-boundary", node_dirs)
                )
                evidence["rejections"].append(
                    rejection(
                        "duplicate-native-cancel",
                        lambda: client.mempool_submit_signed_escrow_transaction(
                            cancel.signed_escrow_transaction
                        ),
                        "bad_sequence",
                        node_dirs,
                    )
                )
                cancel_final = account_balance(client, owner.address)
                cancel_create_fee = int(cancel_create.signed_escrow_transaction["unsigned"]["fee"])
                cancel_fee = int(cancel.signed_escrow_transaction["unsigned"]["fee"])
                if cancel_locked != cancel_before - 70_000 - cancel_create_fee:
                    raise RuntimeError("native cancel lock equation failed")
                if cancel_final != cancel_locked + 70_000 - cancel_fee:
                    raise RuntimeError("native cancel refund equation failed")
                evidence["native_cancel"] = {
                    "cancel_after": cancel_after,
                    "before": cancel_before,
                    "while_locked": cancel_locked,
                    "after": cancel_final,
                    "principal": 70_000,
                    "create_fee": cancel_create_fee,
                    "cancel_fee": cancel_fee,
                    "refund_conserved": True,
                }

                # Issued-asset finish/cancel preserves global supply and trustline policy.
                asset_create = create_issued_asset(
                    client,
                    wallet=issuer,
                    code="LNSWAP",
                    precision=0,
                    max_supply=1_000_000,
                    requires_authorization=False,
                    freeze_enabled=False,
                    clawback_enabled=False,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(asset_create, "issued-asset-create", node_dirs)
                )
                asset_id = asset_create.asset_id
                for label, wallet in (("owner", owner), ("recipient", recipient)):
                    trust = create_asset_trustline(
                        client,
                        wallet=wallet,
                        issuer=issuer.address,
                        asset_id=asset_id,
                        limit=1_000_000,
                        work_dir=work_dir,
                        finalize_data_dir=node_dirs[0],
                        validator_data_dirs=node_dirs,
                    )
                    evidence["accepted_transitions"].append(
                        accepted_result(trust, f"issued-trustline-{label}", node_dirs)
                    )
                issue = send_issued_asset(
                    client,
                    wallet=issuer,
                    to_address=owner.address,
                    issuer=issuer.address,
                    asset_id=asset_id,
                    amount=50_000,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(issue, "issued-fund-owner", node_dirs)
                )

                def issued_snapshot() -> dict[str, Any]:
                    owner_line = asset_line(client, owner.address, asset_id)
                    recipient_line = asset_line(client, recipient.address, asset_id)
                    return {
                        "outstanding_supply": int(client.asset_info(asset_id)["asset"]["outstanding_supply"]),
                        "owner": {
                            key: owner_line[key]
                            for key in ("balance", "limit", "authorized", "frozen")
                        },
                        "recipient": {
                            key: recipient_line[key]
                            for key in ("balance", "limit", "authorized", "frozen")
                        },
                        "open_locked": open_locked(client, owner.address, asset_id),
                    }

                issued_before = issued_snapshot()
                issued_finish_create = create_escrow(
                    client,
                    owner_wallet=owner,
                    destination=recipient.address,
                    asset_id=asset_id,
                    amount=10_000,
                    condition=condition,
                    cancel_after=int(client.status()["block_height"]) + 5,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(issued_finish_create, "issued-create-finish-path", node_dirs)
                )
                issued_locked = issued_snapshot()
                issued_finish = finish_escrow(
                    client,
                    recipient_wallet=recipient,
                    escrow_id=issued_finish_create.escrow_id,
                    owner=owner.address,
                    fulfillment=fulfillment,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(issued_finish, "issued-finish", node_dirs)
                )
                issued_finished = issued_snapshot()
                if not (
                    issued_before["outstanding_supply"]
                    == issued_locked["outstanding_supply"]
                    == issued_finished["outstanding_supply"]
                    == 50_000
                ):
                    raise RuntimeError("issued supply changed during finish path")
                if not (
                    int(issued_before["owner"]["balance"]) == 50_000
                    and int(issued_locked["owner"]["balance"]) == 40_000
                    and issued_locked["open_locked"] == 10_000
                    and int(issued_finished["recipient"]["balance"]) == 10_000
                    and issued_finished["open_locked"] == 0
                ):
                    raise RuntimeError("issued finish trustline accounting failed")

                issued_cancel_after = int(client.status()["block_height"]) + 3
                issued_cancel_create = create_escrow(
                    client,
                    owner_wallet=owner,
                    destination=recipient.address,
                    asset_id=asset_id,
                    amount=7_000,
                    condition=condition,
                    cancel_after=issued_cancel_after,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(issued_cancel_create, "issued-create-cancel-path", node_dirs)
                )
                issued_cancel_locked = issued_snapshot()
                evidence["rejections"].append(
                    rejection(
                        "issued-early-cancel",
                        lambda: cancel_escrow(
                            client,
                            owner_wallet=owner,
                            escrow_id=issued_cancel_create.escrow_id,
                            work_dir=work_dir,
                        ),
                        "escrow_cancel_too_early",
                        node_dirs,
                    )
                )
                filler = request_faucet_pft(
                    data_dir=node_dirs[0],
                    to_address=recipient.address,
                    amount=1,
                    validator_data_dirs=node_dirs,
                    work_dir=work_dir,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(filler, "issued-cancel-boundary-filler", node_dirs)
                )
                evidence["rejections"].append(
                    rejection(
                        "issued-finish-at-cancel-boundary",
                        lambda: finish_escrow(
                            client,
                            recipient_wallet=recipient,
                            escrow_id=issued_cancel_create.escrow_id,
                            owner=owner.address,
                            fulfillment=fulfillment,
                            work_dir=work_dir,
                        ),
                        "escrow_finish_expired",
                        node_dirs,
                    )
                )
                issued_cancel = cancel_escrow(
                    client,
                    owner_wallet=owner,
                    escrow_id=issued_cancel_create.escrow_id,
                    work_dir=work_dir,
                    finalize_data_dir=node_dirs[0],
                    validator_data_dirs=node_dirs,
                )
                evidence["accepted_transitions"].append(
                    accepted_result(issued_cancel, "issued-cancel-at-boundary", node_dirs)
                )
                issued_canceled = issued_snapshot()
                if not (
                    issued_finished["outstanding_supply"]
                    == issued_cancel_locked["outstanding_supply"]
                    == issued_canceled["outstanding_supply"]
                    == 50_000
                    and int(issued_cancel_locked["owner"]["balance"]) == 33_000
                    and issued_cancel_locked["open_locked"] == 7_000
                    and int(issued_canceled["owner"]["balance"]) == 40_000
                    and int(issued_canceled["recipient"]["balance"]) == 10_000
                    and issued_canceled["open_locked"] == 0
                ):
                    raise RuntimeError("issued cancel supply/trustline accounting failed")
                policy_keys = ("limit", "authorized", "frozen")
                for party in ("owner", "recipient"):
                    baseline = {key: issued_before[party][key] for key in policy_keys}
                    if any(
                        {key: snapshot[party][key] for key in policy_keys} != baseline
                        for snapshot in (issued_locked, issued_finished, issued_cancel_locked, issued_canceled)
                    ):
                        raise RuntimeError("issued escrow mutated trustline policy")
                evidence["issued_asset"] = {
                    "asset_id": asset_id,
                    "before": issued_before,
                    "finish_locked": issued_locked,
                    "after_finish": issued_finished,
                    "cancel_locked": issued_cancel_locked,
                    "after_cancel": issued_canceled,
                    "supply_conserved": True,
                    "trustline_policy_preserved": True,
                }
                evidence["final_convergence"] = convergence(node_dirs, "final")
            finally:
                server.terminate()
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)

        # Fresh processes replay each validator block log and must reproduce one root.
        replay = []
        for index, node_dir in enumerate(node_dirs):
            verified = run_json("verify-blocks", "--data-dir", str(node_dir))
            status = run_json("status", "--data-dir", str(node_dir))
            replay.append({"validator": index, "verify_blocks": verified, "status": status})
        replay_roots = {row["status"]["state_root"] for row in replay}
        replay_tips = {row["status"]["block_tip_hash"] for row in replay}
        if len(replay_roots) != 1 or len(replay_tips) != 1:
            raise RuntimeError("restart/replay diverged")
        if any(row["verify_blocks"].get("verified") is not True for row in replay):
            raise RuntimeError("verify-blocks failed after restart")
        evidence["restart_replay"] = {
            "identical_roots": True,
            "identical_tips": True,
            "validators": replay,
        }
        evidence["result"] = "GREEN"
        write_json(args.output, evidence)

    print(json.dumps({"ok": True, "result": "GREEN", "output": str(args.output)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
