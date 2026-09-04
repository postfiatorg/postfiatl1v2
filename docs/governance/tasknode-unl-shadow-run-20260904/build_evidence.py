#!/usr/bin/env python3
"""Build deterministic local shadow inputs from the frozen public responses.

This evidence helper performs no network access. It reads only the committed
archive response, the committed round-19 list, and the repository's frozen
round-20 list and identity coordinates.
"""

from __future__ import annotations

import hashlib
import json
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any


HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parents[2]
BASELINE_PATH = REPO_ROOT / (
    "benchmarks/ai-governance/institution-reputation-unl-20260901/"
    "sources/postfiat-current-unl.json"
)
IDENTITIES_PATH = REPO_ROOT / (
    "benchmarks/ai-governance/institution-reputation-unl-20260901/"
    "inputs/validators.json"
)
ARCHIVE_PATH = HERE / "archive-rpc-response.json"
ROUND_19_PATH = HERE / "round-19-selected-unl.json"

TASKNODE_WALLET = "rpHvzMCKZ7JrzGfRseXohC3RsMWqcnEKkA"
SOURCE_REVISION = "deaaa5a280765869af0d5a472921710711b9a37f"
ENDPOINT = "wss://ws-archive.testnet.postfiat.org"
RIPPLE_EPOCH = datetime(2000, 1, 1, tzinfo=timezone.utc)
EVALUATION_END = "2026-09-04T11:57:22Z"
WINDOW_START = "2026-03-08T11:57:22Z"


def _load(path: Path) -> Any:
    return json.loads(path.read_text())


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _write(name: str, value: Any) -> None:
    (HERE / name).write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n"
    )


def _text_from_hex(value: str | None) -> str:
    if not value:
        return ""
    return bytes.fromhex(value).decode("utf-8")


def _read_varint(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, offset
        shift += 7


def _decode_pointer(data: bytes) -> dict[str, Any]:
    string_fields = {1: "cid", 5: "task_id", 6: "thread_id", 7: "context_id"}
    integer_fields = {2: "target", 3: "kind_id", 4: "schema", 8: "flags"}
    kinds = {
        1: "TASK",
        2: "TASK_UPDATE",
        3: "TASK_SUBMISSION",
        4: "CHAT",
        5: "CONTEXT",
        6: "REWARD",
        7: "POLICY",
        8: "IDENTITY",
        9: "ASSET",
        10: "DOCUMENT",
        11: "SYSTEM",
        99: "TEST",
    }
    result: dict[str, Any] = {}
    offset = 0
    while offset < len(data):
        key, offset = _read_varint(data, offset)
        field_number = key >> 3
        wire_type = key & 7
        if wire_type == 0:
            value, offset = _read_varint(data, offset)
            if field_number in integer_fields:
                result[integer_fields[field_number]] = value
        elif wire_type == 2:
            length, offset = _read_varint(data, offset)
            value = data[offset : offset + length].decode("utf-8")
            offset += length
            if field_number in string_fields:
                result[string_fields[field_number]] = value
        else:
            raise ValueError(f"unsupported pointer wire type {wire_type}")
    if "kind_id" in result:
        result["kind"] = kinds.get(result["kind_id"], "UNKNOWN")
    return result


def _close_time(ripple_seconds: int) -> str:
    value = RIPPLE_EPOCH + timedelta(seconds=ripple_seconds)
    return value.isoformat().replace("+00:00", "Z")


def _control_group(identifier: str, *, foundation: bool) -> dict[str, str]:
    if foundation:
        return {
            "operator_group": "postfiat-foundation",
            "release_manager_group": "postfiat-foundation",
            "key_management_group": "postfiat-foundation",
            "funding_source_group": "postfiat-foundation",
        }
    return {
        "operator_group": f"unresolved-operator-{identifier}",
        "release_manager_group": f"unresolved-release-{identifier}",
        "key_management_group": f"unresolved-key-management-{identifier}",
        "funding_source_group": f"unresolved-funding-{identifier}",
    }


def main() -> None:
    archive_bytes = ARCHIVE_PATH.read_bytes()
    archive = json.loads(archive_bytes)
    account_result = archive["account_tx"]["result"]
    ledger = archive["ledger"]["result"]["ledger"]
    rows = account_result["transactions"]

    transactions: list[dict[str, Any]] = []
    pointers: list[dict[str, Any]] = []
    wallets = {TASKNODE_WALLET}
    wallet_signing_keys: dict[str, str] = {}
    for row in rows:
        tx = row["tx"]
        meta = row["meta"]
        source = tx["Account"]
        target = tx["Destination"]
        wallets.update((source, target))
        signing_key = tx["SigningPubKey"].lower()
        previous_key = wallet_signing_keys.setdefault(source, signing_key)
        if previous_key != signing_key:
            raise ValueError(f"multiple signing keys observed for {source}")
        memos: list[dict[str, Any]] = []
        for memo_index, wrapper in enumerate(tx.get("Memos", [])):
            memo = wrapper["Memo"]
            memo_type = _text_from_hex(memo.get("MemoType"))
            memo_format = _text_from_hex(memo.get("MemoFormat"))
            normalized_memo: dict[str, Any] = {
                "memo_index": memo_index,
                "memo_type": memo_type,
                "memo_format": memo_format,
                "memo_data_hex": memo.get("MemoData", "").lower(),
            }
            if memo_type == "pf.ptr" and memo_format == "v4":
                memo_data = bytes.fromhex(memo["MemoData"])
                pointer = {
                    "pointer_id": f"{tx['hash'].lower()}:{memo_index}",
                    "tx_hash": tx["hash"].lower(),
                    "ledger_index": tx["ledger_index"],
                    "transaction_index": meta["TransactionIndex"],
                    "close_time": _close_time(tx["date"]),
                    "sender_wallet_address": source,
                    "memo_data_sha256": _sha256_bytes(memo_data),
                    **_decode_pointer(memo_data),
                }
                pointers.append(pointer)
                normalized_memo["pointer"] = pointer
            memos.append(normalized_memo)
        transactions.append(
            {
                "tx_hash": tx["hash"].lower(),
                "ledger_index": tx["ledger_index"],
                "transaction_index": meta["TransactionIndex"],
                "close_time": _close_time(tx["date"]),
                "transaction_type": tx["TransactionType"],
                "transaction_result": meta["TransactionResult"],
                "source_wallet_address": source,
                "target_wallet_address": target,
                "value_asset": "PFT",
                "value_units": int(tx["Amount"]),
                "memos": memos,
                "validated": row["validated"],
            }
        )
    transactions.sort(
        key=lambda item: (
            item["ledger_index"],
            item["transaction_index"],
            item["tx_hash"],
        )
    )
    pointers.sort(key=lambda item: item["pointer_id"])

    snapshot = {
        "schema": "tasknode-unl-real-ledger-snapshot-v1",
        "mode": "SHADOW_ONLY",
        "endpoint": ENDPOINT,
        "account": TASKNODE_WALLET,
        "ledger_anchor": {
            "ledger_index": int(ledger["ledger_index"]),
            "ledger_hash": ledger["ledger_hash"].lower(),
            "close_time": "2026-09-04T11:57:22Z",
        },
        "account_tx_query": {
            "ledger_index_min": 1,
            "ledger_index_max": 6012256,
            "forward": True,
            "binary": False,
            "limit": 200,
            "pages": 1,
            "final_marker": None,
            "validated": account_result["validated"],
        },
        "transactions": transactions,
    }
    _write("ledger-snapshot.json", snapshot)
    _write(
        "pointer-memos.json",
        {
            "schema": "tasknode-unl-real-pointer-catalog-v1",
            "mode": "SHADOW_ONLY",
            "pointer_schema": "pf.ptr/v4",
            "pointers": pointers,
        },
    )

    wallet_rows = [
        {
            "wallet_address": wallet,
            "account_id": f"observed-wallet-{wallet}",
        }
        for wallet in sorted(wallets)
    ]
    transfer_rows = [
        {
            "tx_hash": item["tx_hash"],
            "ledger_index": item["ledger_index"],
            "transaction_index": item["transaction_index"],
            "close_time": item["close_time"],
            "source_wallet_address": item["source_wallet_address"],
            "target_wallet_address": item["target_wallet_address"],
            "asset": item["value_asset"],
            "value_units": item["value_units"],
        }
        for item in transactions
        if item["transaction_type"] == "Payment"
        and item["transaction_result"] == "tesSUCCESS"
    ]
    window = {
        "start": WINDOW_START,
        "end": EVALUATION_END,
        "days": 180,
    }
    _write(
        "shadow-funding-transfers.json",
        {
            "schema": "tasknode-unl-funding-transfer-input-v1",
            "mode": "SHADOW_ONLY",
            "window": window,
            "value_asset": "PFT",
            "history_complete_from_ledger_genesis": False,
            "window_complete": False,
            "wallet_accounts": wallet_rows,
            "transfers": transfer_rows,
        },
    )
    _write(
        "shadow-vouch-memos.json",
        {
            "schema": "tasknode-unl-vouch-ledger-input-v1",
            "mode": "SHADOW_ONLY",
            "window": window,
            "memos": [],
        },
    )
    _write(
        "shadow-cowork-pointers.json",
        {
            "schema": "tasknode-unl-cowork-pointer-input-v1",
            "mode": "SHADOW_ONLY",
            "window": window,
            "pointers": [],
        },
    )
    _write(
        "shadow-bindings.json",
        {
            "schema": "tasknode-unl-binding-replay-input-v1",
            "mode": "SHADOW_ONLY",
            "evaluation_end": EVALUATION_END,
            "records": [],
            "rotations": [],
            "reattachments": [],
        },
    )
    _write(
        "shadow-work-digests.json",
        {
            "schema": "tasknode-unl-work-digest-bundle-v1",
            "mode": "SHADOW_ONLY",
            "digests": [],
        },
    )
    _write(
        "shadow-ledger-snapshots.json",
        {
            "schema": "tasknode-unl-ledger-snapshot-bundle-v1",
            "mode": "SHADOW_ONLY",
            "snapshots": [],
        },
    )
    _write(
        "shadow-publishing-keys.json",
        {
            "schema": "tasknode-unl-work-digest-publishing-keys-v1",
            "mode": "SHADOW_ONLY",
            "keys": [],
        },
    )
    (HERE / "funding-exclusions.json").write_text("null\n")

    baseline_bytes = BASELINE_PATH.read_bytes()
    baseline = json.loads(baseline_bytes)
    baseline_ids = baseline["unl"]
    current_root = _sha256_bytes(baseline_bytes)
    round_19_bytes = ROUND_19_PATH.read_bytes()
    round_19 = json.loads(round_19_bytes)
    round_19_root = _sha256_bytes(round_19_bytes)
    identities = _load(IDENTITIES_PATH)
    foundation_ids = sorted(
        row["validator_id"]
        for row in identities
        if row.get("network") == "postfiat"
        and row.get("institutional_affiliation") == "Post Fiat"
    )

    _write(
        "baseline-list.json",
        {
            "schema": "tasknode-unl-churn-baseline-v1",
            "mode": "SHADOW_ONLY",
            "registry_round": 20,
            "registry_root": current_root,
            "validator_ids": baseline_ids,
        },
    )
    _write(
        "registry-rounds.json",
        {
            "schema": "tasknode-unl-registry-history-v1",
            "mode": "SHADOW_ONLY",
            "current_round": 20,
            "current_root": current_root,
            "rounds": [
                {
                    "round": 19,
                    "root": round_19_root,
                    "validator_ids": round_19["unl"],
                },
                {
                    "round": 20,
                    "root": current_root,
                    "validator_ids": baseline_ids,
                },
            ],
        },
    )
    active = [
        {
            "validator_id": validator_id,
            "account_id": validator_id,
            "control_group": _control_group(
                validator_id,
                foundation=validator_id in foundation_ids,
            ),
        }
        for validator_id in sorted(baseline_ids)
    ]
    candidates = []
    for wallet in sorted(wallets):
        candidate_id = f"unbound-wallet-{wallet}"
        candidates.append(
            {
                "validator_id": candidate_id,
                "account_id": f"observed-wallet-{wallet}",
                "public_key_hash": _sha256_bytes(
                    bytes.fromhex(wallet_signing_keys[wallet])
                ),
                "reliability_bps": None,
                "operator_manifest_signed": None,
                "domain_control_proved": None,
                "cobalt_linkedness_safe": None,
                "control_group": _control_group(
                    candidate_id, foundation=False
                ),
                "model_output": None,
            }
        )
    _write(
        "shadow-policy-evidence.json",
        {
            "schema": "tasknode-unl-shadow-policy-evidence-v1",
            "mode": "SHADOW_ONLY",
            "evaluation_end": EVALUATION_END,
            "target_round": 21,
            "transition_budget": 1,
            "foundation_bound_validator_ids": foundation_ids,
            "active_validators": active,
            "candidates": candidates,
        },
    )
    _write(
        "shadow-input.json",
        {
            "schema": "tasknode-unl-shadow-input-manifest-v1",
            "mode": "SHADOW_ONLY",
            "files": {
                "binding_replay": "shadow-bindings.json",
                "work_digests": "shadow-work-digests.json",
                "ledger_snapshots": "shadow-ledger-snapshots.json",
                "publishing_keys": "shadow-publishing-keys.json",
                "vouch_ledger": "shadow-vouch-memos.json",
                "cowork_pointers": "shadow-cowork-pointers.json",
                "funding_transfers": "shadow-funding-transfers.json",
                "funding_exclusions": "funding-exclusions.json",
                "policy_evidence": "shadow-policy-evidence.json",
                "baseline_list": "baseline-list.json",
                "registry_history": "registry-rounds.json",
            },
        },
    )

    _write(
        "source-manifest.json",
        {
            "schema": "tasknode-unl-real-shadow-source-manifest-v1",
            "mode": "SHADOW_ONLY",
            "capture_date": "2026-09-04",
            "tasknode_wallet": TASKNODE_WALLET,
            "pull_cap_transactions": 2000,
            "endpoint_used": ENDPOINT,
            "read_methods": ["server_info", "ledger", "account_tx"],
            "authentication_used": False,
            "archive_complete_ledgers_at_capture": archive["server_info"]
            ["result"]["info"]["complete_ledgers"],
            "ledger_window": {"minimum": 1, "maximum": 6012256},
            "bounded_account_tx_pull": {
                "aggregate_record_cap": 2000,
                "tasknode_wallet": {
                    "records": 49,
                    "pages": 1,
                    "complete": True,
                    "ledger_span": [5620845, 6003432],
                    "final_marker": None,
                },
                "counterparties": [
                    {
                        "wallet_address": "rPo8GkCA9YMKzuJGTHbj11kdVfPqSJHxNx",
                        "records": 730,
                        "pages": 4,
                        "complete": True,
                        "ledger_span": [3415, 5770533],
                        "final_marker": None,
                    },
                    {
                        "wallet_address": "rwdm72S9YVKkZjeADKU2bbUMuY4vPnSfH7",
                        "records": 1221,
                        "pages": 7,
                        "complete": False,
                        "ledger_span": [3387, 479122],
                        "final_marker": {"ledger": 482263, "seq": 0},
                    },
                ],
                "aggregate_records": 2000,
                "note": (
                    "Counterparty responses are summarized, not retained as "
                    "bulk dumps; the rwdm query stopped at the cap."
                ),
            },
            "transaction_window": {
                "minimum_ledger": transactions[0]["ledger_index"],
                "maximum_ledger": transactions[-1]["ledger_index"],
                "first_close_time": transactions[0]["close_time"],
                "last_close_time": transactions[-1]["close_time"],
            },
            "counts": {
                "account_transactions": len(transactions),
                "successful_payments": len(transfer_rows),
                "wallets": len(wallets),
                "counterparty_wallets": len(wallets - {TASKNODE_WALLET}),
                "pf_ptr_v4_memos": len(pointers),
                "binding_memos": 0,
                "signed_work_digests": 0,
                "vouch_memos": 0,
            },
            "baseline": {
                "repository_revision": SOURCE_REVISION,
                "path": str(BASELINE_PATH.relative_to(REPO_ROOT)),
                "round": baseline["round_number"],
                "status": baseline["status"],
                "validator_count": len(baseline_ids),
                "source_sha256": current_root,
            },
            "round_19_history": {
                "url": (
                    "https://scoring-testnet.postfiat.org/api/scoring/"
                    "rounds/19/outputs/selected_unl.json"
                ),
                "validator_count": len(round_19["unl"]),
                "source_sha256": round_19_root,
            },
            "probes": [
                {
                    "endpoint": "https://rpc.testnet.postfiat.org",
                    "method": "server_info",
                    "result": "HTTP 200; history 5950054-6012252",
                },
                {
                    "endpoint": "https://rpc.testnet.postfiat.org:5006/",
                    "method": "server_info",
                    "result": "connection refused",
                },
                {
                    "endpoint": "http://178.156.143.199:5005",
                    "method": "server_info",
                    "result": "HTTP 200; history 6011631-6012252",
                },
                {
                    "endpoint": ENDPOINT,
                    "method": "server_info",
                    "result": "success; history 1-6012268",
                },
            ],
            "coverage_limits": [
                "No validator-key-to-wallet binding evidence exists.",
                "Observed wallets are coverage rows, not validator identities.",
                (
                    "Candidate public_key_hash values hash observed wallet "
                    "transaction signing keys, not validator keys; unresolved "
                    "control-group values only satisfy non-null input syntax "
                    "and are not evidence."
                ),
                "The bounded counterparty pull hit its aggregate cap before the rwdm history completed, so funding history and window completeness are false.",
                "No published funding exclusion list was available; the input is null.",
                "Encrypted pointer payloads were not fetched or decrypted.",
            ],
            "file_sha256": {
                "archive-rpc-response.json": _sha256_bytes(archive_bytes),
                "round-19-selected-unl.json": round_19_root,
                "repository-round-20-source": current_root,
            },
        },
    )


if __name__ == "__main__":
    main()
