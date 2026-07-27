#!/usr/bin/env python3
"""From-zero audit of the pfUSDC Ethereum mainnet epoch-4 deployment package."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import urllib.request
from pathlib import Path
from typing import Any

from Crypto.Hash import keccak


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
PROFILE_FIELDS = (
    "schema",
    "route_id",
    "asset_id",
    "source_chain_id",
    "vault_address",
    "vault_runtime_code_hash",
    "token_address",
    "token_runtime_code_hash",
    "route_epoch",
    "verifier_kind",
    "evidence_tier",
    "verifier_policy_hash",
    "verifier_program_vkey",
    "verifier_proof_encoding",
    "max_proof_bytes",
    "max_public_values_bytes",
    "max_snapshot_age_blocks",
    "challenge_window_blocks",
    "max_epoch_gap_blocks",
    "settle_deadline_blocks",
    "min_challenge_bond",
    "min_attestations",
    "minimum_confirmations",
    "activation_height",
    "expires_at_height",
)
VAULT_IMMUTABLE_FIELDS = (
    "token_address",
    "token_runtime_code_hash",
)
VERIFIER_IMMUTABLE_FIELDS = (
    "sp1_gateway_address",
    "egress_program_vkey",
    "pftl_chain_id_hash",
    "pftl_genesis_hash_commitment",
    "pftl_protocol_version",
    "route_profile_hash_commitment",
    "route_epoch",
    "asset_id_commitment",
    "source_chain_id",
    "vault_runtime_code_hash",
    "token_address",
    "token_runtime_code_hash",
    "max_proof_bytes",
    "max_public_values_bytes",
)


class AuditError(RuntimeError):
    """Raised on any independently recomputed mismatch."""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def k256(value: bytes) -> bytes:
    digest = keccak.new(digest_bits=256)
    digest.update(value)
    return digest.digest()


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise AuditError(f"cannot read {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise AuditError(f"expected JSON object: {path}")
    return value


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AuditError(message)


def rlp_bytes(value: bytes) -> bytes:
    if len(value) == 1 and value[0] < 0x80:
        return value
    if len(value) < 56:
        return bytes([0x80 + len(value)]) + value
    length = len(value).to_bytes((len(value).bit_length() + 7) // 8, "big")
    return bytes([0xB7 + len(length)]) + length + value


def rlp_list(values: list[bytes]) -> bytes:
    payload = b"".join(values)
    if len(payload) < 56:
        return bytes([0xC0 + len(payload)]) + payload
    length = len(payload).to_bytes((len(payload).bit_length() + 7) // 8, "big")
    return bytes([0xF7 + len(length)]) + length + payload


def create_address(deployer: str, nonce: int) -> str:
    sender = bytes.fromhex(deployer.removeprefix("0x"))
    require(len(sender) == 20, "deployer is not 20 bytes")
    encoded_nonce = b"" if nonce == 0 else nonce.to_bytes((nonce.bit_length() + 7) // 8, "big")
    return "0x" + k256(rlp_list([rlp_bytes(sender), rlp_bytes(encoded_nonce)]))[12:].hex()


def word(value: object) -> bytes:
    if isinstance(value, int) and not isinstance(value, bool):
        require(0 <= value < 1 << 256, "integer immutable is outside uint256")
        return value.to_bytes(32, "big")
    require(isinstance(value, str) and value.startswith("0x"), "immutable is not hex/int")
    raw = bytes.fromhex(value[2:])
    require(len(raw) <= 32, "immutable exceeds one word")
    return raw.rjust(32, b"\x00")


def artifact(manifest_item: dict[str, Any]) -> dict[str, Any]:
    path = ROOT / manifest_item["path"]
    require(sha256(path) == manifest_item["artifact_sha256"], f"artifact digest mismatch: {path}")
    document = read_json(path)
    creation = bytes.fromhex(document["bytecode"]["object"].removeprefix("0x"))
    deployed = bytes.fromhex(document["deployedBytecode"]["object"].removeprefix("0x"))
    require(
        "0x" + k256(creation).hex() == manifest_item["creation_bytecode_keccak256"],
        f"creation bytecode hash mismatch: {path}",
    )
    require(
        "0x" + k256(deployed).hex() == manifest_item["unlinked_deployed_bytecode_keccak256"],
        f"deployed bytecode hash mismatch: {path}",
    )
    return document


def runtime_hash(
    document: dict[str, Any],
    immutable_fields: tuple[str, ...],
    values: dict[str, object],
) -> str:
    deployed = document["deployedBytecode"]
    references = deployed.get("immutableReferences", {})
    ordered_ids = sorted(references, key=int)
    require(
        len(ordered_ids) == len(immutable_fields),
        "immutable-reference count drifted",
    )
    immutable_layout = dict(zip(ordered_ids, immutable_fields, strict=True))
    code = bytearray(bytes.fromhex(deployed["object"].removeprefix("0x")))
    for identifier, locations in references.items():
        field = immutable_layout[identifier]
        require(field in values, f"no independent value for immutable {field}")
        encoded = word(values[field])
        for location in locations:
            start, length = location["start"], location["length"]
            require(length == 32, "immutable reference is not one word")
            code[start : start + length] = encoded
    return "0x" + k256(bytes(code)).hex()


def profile_hash(profile: dict[str, Any]) -> str:
    require(all(field in profile for field in PROFILE_FIELDS), "profile is missing hashed fields")
    encoded = "".join(f"{field}={profile[field]}\n" for field in PROFILE_FIELDS).encode()
    return hashlib.sha3_384(
        b"postfiat.vault_bridge.route_profile_hash.v1\x00" + encoded
    ).hexdigest()


def rpc(url: str, method: str, params: list[object]) -> Any:
    request = urllib.request.Request(
        url,
        data=json.dumps(
            {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
        ).encode(),
        headers={
            "Content-Type": "application/json",
            "User-Agent": "postfiat-mainnet-epoch4-audit/1.0",
        },
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        result = json.load(response)
    if "error" in result:
        raise AuditError(f"Ethereum RPC {method} failed: {result['error']}")
    return result["result"]


def audit(args: argparse.Namespace) -> dict[str, Any]:
    manifest_path = args.manifest.resolve()
    manifest = read_json(manifest_path)
    require(sha256(manifest_path) == args.expected_manifest_sha256, "manifest digest mismatch")
    require(
        (manifest["revision"], manifest["network"]["source_chain_id"]) == ("mainnet-epoch4", 1),
        "manifest revision/chain mismatch",
    )
    route = manifest["route"]
    require(
        (
            route["route_id"],
            route["route_epoch"],
            route["activation_height"],
            route["binding_submission_height"],
            route["registration_height"],
        )
        == ("ethereum-mainnet-usdc-v1", 4, 318, 317, 318),
        "route epoch/schedule mismatch",
    )
    artifacts = {item["contract"]: item for item in manifest["contracts"]["artifacts"]}
    require(set(artifacts) == {"PFTLFinalityVerifierV1", "ERC20BridgeVaultL1"}, "artifact set mismatch")
    verifier_item = artifacts["PFTLFinalityVerifierV1"]
    vault_item = artifacts["ERC20BridgeVaultL1"]
    require(
        (verifier_item["precomputed_create_nonce"], vault_item["precomputed_create_nonce"])
        == (157, 158),
        "CREATE nonce pair mismatch",
    )
    deployer = manifest["deployer"]["address"]
    require(create_address(deployer, 157).lower() == verifier_item["address"].lower(), "verifier CREATE address mismatch")
    require(create_address(deployer, 158).lower() == vault_item["address"].lower(), "vault CREATE address mismatch")

    verifier_artifact = artifact(verifier_item)
    vault_artifact = artifact(vault_item)
    network, programs, pftl = manifest["network"], manifest["programs"], manifest["pftl"]
    vault_runtime = runtime_hash(
        vault_artifact,
        VAULT_IMMUTABLE_FIELDS,
        {
            "token_address": network["token"]["address"],
            "token_runtime_code_hash": network["token"]["runtime_code_hash"],
        },
    )
    require(vault_runtime == route["vault_runtime_code_hash"], "vault runtime hash mismatch")

    profile = route["route_profile"]
    recomputed_profile = profile_hash(profile)
    require(recomputed_profile == route["route_profile_hash"], "route profile hash mismatch")
    commitment = "0x" + k256(bytes.fromhex(recomputed_profile)).hex()
    binding = k256(
        b"postfiat.vault_bridge.route_binding.v1\x00"
        + bytes.fromhex(recomputed_profile)
        + (4).to_bytes(4, "big")
    ).hex()
    policy = k256(
        b"postfiat.ethereum-mainnet-usdc-v1.p0\x00"
        + bytes.fromhex(vault_item["address"][2:])
        + bytes.fromhex(network["token"]["address"][2:])
        + vault_item["creation_bytecode_keccak256"].encode()
    ).hex()
    require(commitment == route["route_profile_hash_commitment"], "route commitment mismatch")
    require(binding == route["route_binding"], "route binding mismatch")
    require(policy == route["policy_hash"], "route policy mismatch")

    verifier_runtime = runtime_hash(
        verifier_artifact,
        VERIFIER_IMMUTABLE_FIELDS,
        {
            "sp1_gateway_address": network["sp1_verifier_gateway"]["address"],
            "egress_program_vkey": programs["egress"]["program_vkey"],
            "pftl_chain_id_hash": pftl["chain_id_hash"],
            "pftl_genesis_hash_commitment": pftl["genesis_hash_commitment"],
            "pftl_protocol_version": pftl["protocol_version"],
            "route_profile_hash_commitment": commitment,
            "route_epoch": pftl["route_epoch"],
            "asset_id_commitment": pftl["asset_id_commitment"],
            "source_chain_id": network["source_chain_id"],
            "vault_runtime_code_hash": vault_runtime,
            "token_address": network["token"]["address"],
            "token_runtime_code_hash": network["token"]["runtime_code_hash"],
            "max_proof_bytes": programs["max_proof_bytes"],
            "max_public_values_bytes": programs["max_public_values_bytes"],
        },
    )
    require(
        verifier_runtime == verifier_item["deployed_runtime_code_keccak256"],
        "verifier runtime hash mismatch",
    )
    checkpoint_hash = pftl["checkpoint_block_hash"]
    checkpoint_commitment = "0x" + k256(bytes.fromhex(checkpoint_hash)).hex()
    require(
        (pftl["initial_finalized_height"], checkpoint_commitment)
        == (316, pftl["initial_checkpoint_commitment"]),
        "initial H316 checkpoint mismatch",
    )

    cross_check = subprocess.run(
        [str(ROOT / "scripts/pfusdc-contract-guest-storage-cross-check.py")],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    require(cross_check.returncode == 0, "contract/guest cross-check rerun failed")
    require(json.loads(cross_check.stdout)["status"] == "PASS", "contract/guest cross-check not PASS")
    consumer_check = subprocess.run(
        [
            str(HERE / "verify_deploy_consumer_epoch4.py"),
            "--manifest",
            str(manifest_path),
            "--consumer",
            str(args.consumer.resolve()),
            "--log",
            str(args.consumer_log.resolve()),
        ],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    require(consumer_check.returncode == 0, "deploy-consumer AST check failed")

    rpc_url = network["execution_rpc_default"]
    live_chain = int(rpc(rpc_url, "eth_chainId", []), 16)
    live_nonce = int(rpc(rpc_url, "eth_getTransactionCount", [deployer, "latest"]), 16)
    pending_nonce = int(rpc(rpc_url, "eth_getTransactionCount", [deployer, "pending"]), 16)
    verifier_code = rpc(rpc_url, "eth_getCode", [verifier_item["address"], "latest"])
    vault_code = rpc(rpc_url, "eth_getCode", [vault_item["address"], "latest"])
    require((live_chain, live_nonce, pending_nonce) == (1, 157, 157), "live chain/nonce preflight mismatch")
    require(verifier_code == "0x" and vault_code == "0x", "predicted CREATE address already has code")

    payload_summary = read_json(args.payload_summary)
    require(
        payload_summary.get("verdict") == "PASS"
        and payload_summary.get("c8") == "15/15"
        and payload_summary.get("contains_key_material") is False,
        "PFTL public payload summary is not a key-free PASS",
    )
    return {
        "schema": "postfiat.pfusdc.mainnet_epoch4_from_zero_audit.v1",
        "status": "PASS",
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256(manifest_path),
        "checks": {
            "scope_chain_revision": "PASS",
            "create_nonce_address_derivation": "PASS",
            "artifact_and_bytecode_hashes": "PASS",
            "vault_runtime_with_immutables": "PASS",
            "verifier_runtime_with_immutables": "PASS",
            "profile_policy_binding_commitment": "PASS",
            "initial_checkpoint_h316": "PASS",
            "contract_guest_storage": "PASS",
            "deploy_consumer_ast": "PASS",
            "pftl_payloads_c8_key_free": "PASS",
            "live_mainnet_nonce_157_no_pending": "PASS",
            "predicted_addresses_empty": "PASS",
        },
        "planned_deployments": {
            "verifier": {"nonce": 157, "address": verifier_item["address"]},
            "vault": {"nonce": 158, "address": vault_item["address"]},
        },
        "recomputed": {
            "vault_runtime": vault_runtime,
            "verifier_runtime": verifier_runtime,
            "profile_hash": recomputed_profile,
            "route_binding": binding,
            "route_commitment": commitment,
            "route_policy": policy,
            "checkpoint_commitment": checkpoint_commitment,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--consumer", required=True, type=Path)
    parser.add_argument("--consumer-log", required=True, type=Path)
    parser.add_argument("--payload-summary", required=True, type=Path)
    parser.add_argument("--expected-manifest-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        report = audit(args)
    except (AuditError, KeyError, OSError, ValueError) as exc:
        print(f"MAINNET_EPOCH4_FROM_ZERO_AUDIT: FAIL: {exc}")
        return 1
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print("MAINNET_EPOCH4_FROM_ZERO_AUDIT: PASS")
    print(f"MANIFEST_SHA256: {report['manifest_sha256']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
