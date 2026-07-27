#!/usr/bin/env python3
"""Fail closed unless the Ethereum vault storage matches the frozen ingress guest."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CONTRACT_DIR = ROOT / "crates/ethereum-contracts"
CONTRACT_SOURCE = CONTRACT_DIR / "src/ERC20BridgeVaultL1.sol"
GUEST_SOURCE = ROOT / "programs/pfusdc-eth-mainnet-ingress/src/lib.rs"


class CrossCheckError(RuntimeError):
    """Raised when the contract and frozen guest storage models diverge."""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def forge_storage_layout() -> dict[str, Any]:
    result = subprocess.run(
        [
            "forge",
            "inspect",
            "ERC20BridgeVaultL1",
            "storage-layout",
            "--json",
            "--force",
        ],
        cwd=CONTRACT_DIR,
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        raise CrossCheckError(f"forge storage-layout failed: {result.stderr.strip()}")
    try:
        layout = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise CrossCheckError(f"forge storage-layout was not JSON: {exc}") from exc
    if not isinstance(layout, dict):
        raise CrossCheckError("forge storage-layout root is not an object")
    return layout


def require_equal(actual: object, expected: object, label: str) -> None:
    if actual != expected:
        raise CrossCheckError(f"{label} mismatch: expected {expected!r}, got {actual!r}")


def validate_layout(layout: dict[str, Any]) -> dict[str, Any]:
    storage = layout.get("storage")
    types = layout.get("types")
    if not isinstance(storage, list) or not isinstance(types, dict):
        raise CrossCheckError("forge storage-layout lacks storage/types")

    by_label = {
        item.get("label"): item
        for item in storage
        if isinstance(item, dict) and isinstance(item.get("label"), str)
    }
    obligations = by_label.get("totalObligations")
    records = by_label.get("depositRecords")
    if not isinstance(obligations, dict) or not isinstance(records, dict):
        raise CrossCheckError("vault lacks totalObligations or depositRecords storage")
    require_equal((obligations.get("slot"), obligations.get("offset")), ("0", 0), "obligations slot")
    obligations_type = types.get(obligations.get("type"))
    if not isinstance(obligations_type, dict):
        raise CrossCheckError("obligations type is absent")
    require_equal(
        (obligations_type.get("label"), obligations_type.get("numberOfBytes")),
        ("uint256", "32"),
        "obligations type",
    )

    require_equal((records.get("slot"), records.get("offset")), ("1", 0), "deposit mapping slot")
    mapping_type = types.get(records.get("type"))
    if not isinstance(mapping_type, dict):
        raise CrossCheckError("deposit mapping type is absent")
    require_equal(mapping_type.get("encoding"), "mapping", "deposit mapping encoding")
    key_type = types.get(mapping_type.get("key"))
    if not isinstance(key_type, dict):
        raise CrossCheckError("deposit mapping key type is absent")
    require_equal(
        (key_type.get("label"), key_type.get("numberOfBytes")),
        ("bytes32", "32"),
        "deposit mapping key",
    )

    record_type = types.get(mapping_type.get("value"))
    if not isinstance(record_type, dict) or not isinstance(record_type.get("members"), list):
        raise CrossCheckError("deposit record type/members are absent")
    members = []
    for member in record_type["members"]:
        if not isinstance(member, dict):
            raise CrossCheckError("deposit record member is not an object")
        member_type = types.get(member.get("type"))
        if not isinstance(member_type, dict):
            raise CrossCheckError(f"deposit record member type is absent: {member.get('label')}")
        members.append(
            (
                member.get("label"),
                member.get("slot"),
                member.get("offset"),
                member_type.get("label"),
                member_type.get("numberOfBytes"),
            )
        )
    expected_members = [
        ("depositor", "0", 0, "address", "20"),
        ("amount", "0", 20, "uint96", "12"),
        ("recipientHash", "1", 0, "bytes32", "32"),
        ("routeBinding", "2", 0, "bytes32", "32"),
        ("nonce", "3", 0, "bytes32", "32"),
    ]
    require_equal(members, expected_members, "deposit record packing")
    require_equal(record_type.get("numberOfBytes"), "128", "deposit record width")
    return {
        "obligations": {"slot": "0", "type": "uint256"},
        "deposit_mapping": {"slot": "1", "key": "bytes32"},
        "deposit_record_members": members,
        "deposit_record_bytes": 128,
    }


def validate_frozen_guest() -> None:
    source = GUEST_SOURCE.read_text(encoding="utf-8")
    required_markers = (
        "const DEPOSIT_MAPPING_SLOT: u64 = 1;",
        "if vault_slots.len() != 5 || token_slots.len() != 1",
        "let base = mapping_base(deposit_id, DEPOSIT_MAPPING_SLOT);",
        "B256::ZERO,",
        "add_slot(base, 1)?",
        "add_slot(base, 2)?",
        "add_slot(base, 3)?",
        "let packed_address: U256 = packed & address_mask;",
        "let proved_amount = packed >> 160;",
        "let recipient_hash = B256::from(w.vault_storage.storage_slots[2].value.to_be_bytes::<32>());",
        "let route_binding = B256::from(w.vault_storage.storage_slots[3].value.to_be_bytes::<32>());",
        "let nonce = B256::from(w.vault_storage.storage_slots[4].value.to_be_bytes::<32>());",
    )
    missing = [marker for marker in required_markers if marker not in source]
    if missing:
        raise CrossCheckError("frozen guest storage markers drifted: " + ", ".join(missing))

    contract = CONTRACT_SOURCE.read_text(encoding="utf-8")
    if "MAX_DEPOSIT_AMOUNT = type(uint64).max" not in contract:
        raise CrossCheckError("vault deposit domain is not bounded to the guest u64 amount")


def run_decode_simulation() -> None:
    result = subprocess.run(
        [
            "forge",
            "test",
            "--match-contract",
            "ERC20BridgeVaultL1Test",
            "--match-test",
            "testDepositStorageMatchesFrozenGuestDecode",
        ],
        cwd=CONTRACT_DIR,
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        raise CrossCheckError(
            "guest-decode simulation failed:\n"
            + result.stdout.strip()
            + "\n"
            + result.stderr.strip()
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    parser.add_argument("--skip-decode-simulation", action="store_true")
    args = parser.parse_args()

    validate_frozen_guest()
    normalized_layout = validate_layout(forge_storage_layout())
    if not args.skip_decode_simulation:
        run_decode_simulation()

    report = {
        "schema": "postfiat.pfusdc.contract_guest_storage_cross_check.v1",
        "status": "PASS",
        "contract_source": str(CONTRACT_SOURCE.relative_to(ROOT)),
        "contract_source_sha256": sha256(CONTRACT_SOURCE),
        "guest_source": str(GUEST_SOURCE.relative_to(ROOT)),
        "guest_source_sha256": sha256(GUEST_SOURCE),
        "layout": normalized_layout,
        "decode_simulation": "SKIPPED" if args.skip_decode_simulation else "PASS",
    }
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        output = args.output if args.output.is_absolute() else ROOT / args.output
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CrossCheckError as exc:
        raise SystemExit(f"contract_guest_storage_cross_check=failed: {exc}") from exc
