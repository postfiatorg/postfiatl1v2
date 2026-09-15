#!/usr/bin/env python3
"""Build the digest-bound pfETH Ethereum-mainnet deployment package.

This script never broadcasts. It links the WETH vault's immutables, computes
the exact runtime hash, predicts the verifier/vault CREATE addresses, builds
both constructor payloads, derives the governed route profile, and emits a
review gate. The separate deploy driver refuses to broadcast without a signed
external-review record bound to the generated manifest digest.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
from pathlib import Path
from typing import Any

import rlp
from Crypto.Hash import keccak
from eth_abi import encode as abi_encode
from eth_utils import to_checksum_address

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INPUT = ROOT / "deployments/pfeth-eth-mainnet-20260904/input.json"
DEFAULT_OUTPUT = ROOT / "deployments/pfeth-eth-mainnet-20260904"
VAULT_ARTIFACT = (
    ROOT
    / "crates/ethereum-contracts/out/WETHBridgeVaultL1.sol/WETHBridgeVaultL1.json"
)
VERIFIER_ARTIFACT = (
    ROOT
    / "crates/ethereum-contracts/out/PFTLFinalityVerifierV1.sol/PFTLFinalityVerifierV1.json"
)
EXIT_EXECUTOR_ARTIFACT = (
    ROOT / "crates/ethereum-contracts/out/ExitExecutorV1.sol/ExitExecutorV1.json"
)
EXIT_EXECUTOR_SOURCE = ROOT / "crates/ethereum-contracts/src/ExitExecutorV1.sol"
INGRESS_ELF = (
    ROOT
    / "programs/pfeth-eth-mainnet-ingress/elf/pfeth-eth-mainnet-ingress-program"
)
EGRESS_ELF = ROOT / "programs/pfusdc-egress/elf/pfusdc-egress-program"
NODE_BIN = ROOT / "target/debug/postfiat-node"
ROUTE_HASH_DOMAIN = b"postfiat.vault_bridge.route_profile_hash.v1"
ROUTE_BINDING_DOMAIN = b"postfiat.vault_bridge.route_binding.v1"
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


class PackageError(RuntimeError):
    pass


def k256(data: bytes) -> bytes:
    digest = keccak.new(digest_bits=256)
    digest.update(data)
    return digest.digest()


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PackageError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise PackageError(f"JSON object required: {path}")
    return value


def write(path: Path, value: Any, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = (
        value
        if isinstance(value, bytes)
        else (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    )
    tmp = path.with_suffix(path.suffix + ".tmp")
    descriptor = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, mode)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(payload)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(tmp, path)


def hex_bytes(value: str, length: int, field: str) -> bytes:
    text = str(value).removeprefix("0x").lower()
    try:
        raw = bytes.fromhex(text)
    except ValueError as error:
        raise PackageError(f"{field} is not hex") from error
    if len(raw) != length:
        raise PackageError(f"{field} must be {length} bytes")
    return raw


def create_address(deployer: str, nonce: int) -> str:
    encoded = rlp.encode([hex_bytes(deployer, 20, "deployer"), nonce])
    return to_checksum_address(k256(encoded)[12:])


def artifact(path: Path) -> dict[str, Any]:
    value = read_json(path)
    for field in ("bytecode", "deployedBytecode"):
        if not value.get(field, {}).get("object"):
            raise PackageError(f"{path} has no {field}.object")
    return value


def link_vault_runtime(
    vault_artifact: dict[str, Any], token: str, token_code_hash: str
) -> tuple[bytes, dict[str, str]]:
    deployed = bytearray(
        hex_bytes(vault_artifact["deployedBytecode"]["object"], len(
            bytes.fromhex(vault_artifact["deployedBytecode"]["object"].removeprefix("0x"))
        ), "vault deployed bytecode")
    )
    references = vault_artifact["deployedBytecode"].get("immutableReferences", {})
    keys = sorted(references, key=int)
    if len(keys) != 2:
        raise PackageError("WETHBridgeVaultL1 must contain exactly token and tokenRuntimeCodeHash immutables")
    values = {
        keys[0]: bytes(12) + hex_bytes(token, 20, "token"),
        keys[1]: hex_bytes(token_code_hash, 32, "token_runtime_code_hash"),
    }
    for key, replacement in values.items():
        for reference in references[key]:
            start = int(reference["start"])
            length = int(reference["length"])
            if length != 32:
                raise PackageError("vault immutable reference is not one ABI word")
            deployed[start : start + length] = replacement
    return bytes(deployed), {
        keys[0]: "token",
        keys[1]: "tokenRuntimeCodeHash",
    }


def profile_preimage(profile: dict[str, Any]) -> bytes:
    if tuple(profile) != PROFILE_FIELDS:
        raise PackageError("route profile field order drifted")
    return "".join(f"{field}={profile[field]}\n" for field in PROFILE_FIELDS).encode()


def profile_hash(profile: dict[str, Any]) -> str:
    digest = hashlib.sha3_384()
    digest.update(ROUTE_HASH_DOMAIN)
    digest.update(b"\0")
    digest.update(profile_preimage(profile))
    return digest.hexdigest()


def route_binding(profile_hash_hex: str, epoch: int) -> str:
    return k256(
        ROUTE_BINDING_DOMAIN
        + b"\0"
        + hex_bytes(profile_hash_hex, 48, "route_profile_hash")
        + int(epoch).to_bytes(4, "big")
    ).hex()


def canonical_manifest_preimage(
    *, vault: str, token: str, creation_bytecode_hash: str
) -> bytes:
    return (
        b"postfiat.ethereum-mainnet-weth-v1.p0\0"
        + hex_bytes(vault, 20, "vault")
        + hex_bytes(token, 20, "token")
        + hex_bytes(creation_bytecode_hash, 32, "vault creation bytecode hash")
    )


def validate_input(value: dict[str, Any]) -> None:
    required = {
        "schema",
        "deployer",
        "deployer_nonce",
        "owner",
        "sp1_verifier",
        "sp1_verifier_runtime_code_hash",
        "weth",
        "weth_runtime_code_hash",
        "pftl_chain_id",
        "pftl_genesis_hash",
        "pftl_protocol_version",
        "pfeth_asset_id",
        "pftl_issuer",
        "pftl_reserve_operator",
        "pftl_redemption_account",
        "route_epoch",
        "activation_height",
        "expires_at_height",
        "initial_checkpoint_block_id",
        "initial_finalized_height",
        "initial_committee_root",
        "ingress_program_elf_sha256",
        "ingress_program_vkey",
        "egress_program_elf_sha256",
        "egress_program_vkey",
        "max_proof_bytes",
        "max_public_values_bytes",
    }
    missing = sorted(required - set(value))
    if missing:
        raise PackageError(f"input fields missing: {', '.join(missing)}")
    if value["schema"] != "postfiat.pfeth.ethereum_mainnet_deployment_input.v1":
        raise PackageError("input schema mismatch")
    for field in ("deployer", "owner", "sp1_verifier", "weth"):
        to_checksum_address(value[field])
    for field in ("pftl_issuer", "pftl_reserve_operator", "pftl_redemption_account"):
        if not isinstance(value[field], str) or not value[field].startswith("pf"):
            raise PackageError(f"{field} must be a PFTL account")
    for field in (
        "sp1_verifier_runtime_code_hash",
        "weth_runtime_code_hash",
        "ingress_program_vkey",
        "egress_program_vkey",
    ):
        hex_bytes(value[field], 32, field)
    for field in ("ingress_program_elf_sha256", "egress_program_elf_sha256"):
        hex_bytes(value[field], 32, field)
    for field in (
        "pftl_genesis_hash",
        "pfeth_asset_id",
        "initial_checkpoint_block_id",
        "initial_committee_root",
    ):
        hex_bytes(value[field], 48, field)
    if int(value["route_epoch"]) <= 0:
        raise PackageError("route_epoch must be positive")
    if int(value["expires_at_height"]) <= int(value["activation_height"]):
        raise PackageError("route expiry must follow activation")


def generate_pftl_bootstrap(
    value: dict[str, Any],
    output: Path,
    *,
    vault_address: str,
    ingress_policy_hash: str,
    route_hash: str,
) -> dict[str, Any]:
    """Generate the canonical PFETH profile/asset operations with postfiat-node."""
    if not NODE_BIN.is_file() or not os.access(NODE_BIN, os.X_OK):
        raise PackageError(
            "target/debug/postfiat-node is required to generate the PFTL bootstrap bundle"
        )
    bundle = output / "pftl-bootstrap"
    command = [
        str(NODE_BIN),
        "vault-bridge-bootstrap-bundle",
        "--pftl-chain-id",
        value["pftl_chain_id"],
        "--source-chain-id",
        "1",
        "--vault-address",
        vault_address,
        "--token-address",
        value["weth"],
        "--issuer",
        value["pftl_issuer"],
        "--reserve-operator",
        value["pftl_reserve_operator"],
        "--redemption-account",
        value["pftl_redemption_account"],
        "--asset-code",
        "PFETH",
        "--asset-version",
        "1",
        "--asset-precision",
        "9",
        "--asset-display-name",
        "PostFiat WETH Bridge Asset",
        "--max-supply",
        str((1 << 64) - 1),
        "--valuation-unit",
        "ETH",
        "--valuation-policy-hash",
        ingress_policy_hash,
        "--verifier-kind",
        "sp1-groth16",
        "--max-snapshot-age-blocks",
        str(int(value.get("max_snapshot_age_blocks", 900))),
        "--challenge-window-blocks",
        str(int(value.get("challenge_window_blocks", 64))),
        "--max-epoch-gap-blocks",
        str(int(value.get("max_epoch_gap_blocks", 128))),
        "--settle-deadline-blocks",
        str(int(value.get("settle_deadline_blocks", 256))),
        "--min-challenge-bond",
        str(int(value.get("min_challenge_bond", 1))),
        "--min-attestations",
        "0",
        "--tolerance-bp",
        "0",
        "--bridge-observer-min-confirmations",
        "0",
        "--vault-bridge-route-policy-hash",
        route_hash,
        "--sp1-program-vkey",
        value["ingress_program_vkey"],
        "--sp1-proof-encoding",
        "groth16",
        "--max-proof-bytes",
        str(int(value["max_proof_bytes"])),
        "--max-public-values-bytes",
        str(int(value["max_public_values_bytes"])),
        "--bundle",
        str(bundle),
        "--overwrite",
    ]
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "unknown error")[-2_000:]
        raise PackageError(f"PFTL bootstrap generation failed: {detail}")
    try:
        report = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise PackageError("PFTL bootstrap generator returned invalid JSON") from error
    if (
        not isinstance(report, dict)
        or report.get("schema") != "postfiat-vault-bridge-bootstrap-bundle-v1"
        or report.get("asset_id") != value["pfeth_asset_id"]
        or report.get("pftl_chain_id") != value["pftl_chain_id"]
        or int(report.get("source_chain_id", 0)) != 1
        or str(report.get("vault_address", "")).lower() != vault_address.lower()
        or str(report.get("token_address", "")).lower() != value["weth"].lower()
    ):
        raise PackageError("PFTL bootstrap report does not match the pfETH package")
    profile = report.get("profile_register_operation")
    if (
        not isinstance(profile, dict)
        or profile.get("vault_bridge_route_policy_hash") != route_hash
        or profile.get("sp1_program_vkey", "").lower()
        != value["ingress_program_vkey"].lower()
    ):
        raise PackageError("PFTL bootstrap profile does not match the route or ingress vkey")

    commands_path = bundle / "commands.sh"
    commands_text = commands_path.read_text(encoding="utf-8")
    lines = commands_text.splitlines()
    for index, line in enumerate(lines):
        if line.startswith("BUNDLE_DIR="):
            lines[index] = 'BUNDLE_DIR=${BUNDLE_DIR:-pftl-bootstrap}'
            break
    else:
        raise PackageError("PFTL bootstrap commands lack BUNDLE_DIR")
    write(commands_path, ("\n".join(lines) + "\n").encode(), mode=0o755)

    artifact_names = (
        "profile-register.operation.json",
        "asset-create.operation.json",
        "nav-asset-register.operation.json",
        "commands.sh",
    )
    artifact_hashes = {}
    for name in artifact_names:
        path = bundle / name
        if not path.is_file():
            raise PackageError(f"PFTL bootstrap artifact is absent: {path}")
        artifact_hashes[name] = sha256(path)
    report["bundle_dir"] = "pftl-bootstrap"
    for field, name in (
        ("profile_register_operation_file", "profile-register.operation.json"),
        ("asset_create_operation_file", "asset-create.operation.json"),
        ("nav_asset_register_operation_file", "nav-asset-register.operation.json"),
        ("commands_file", "commands.sh"),
    ):
        report[field] = f"pftl-bootstrap/{name}"
    write(output / "pftl-bootstrap-report.json", report)
    return {
        "schema": report["schema"],
        "asset_id": report["asset_id"],
        "profile_id": report["profile_id"],
        "artifact_sha256": artifact_hashes,
    }


def generate(value: dict[str, Any], output: Path) -> dict[str, Any]:
    validate_input(value)
    for file in (
        VAULT_ARTIFACT,
        VERIFIER_ARTIFACT,
        EXIT_EXECUTOR_ARTIFACT,
        EXIT_EXECUTOR_SOURCE,
        INGRESS_ELF,
        EGRESS_ELF,
    ):
        if not file.is_file():
            raise PackageError(f"required build artifact is absent: {file}")
    expected_program_hashes = {
        INGRESS_ELF: value["ingress_program_elf_sha256"].lower().removeprefix("0x"),
        EGRESS_ELF: value["egress_program_elf_sha256"].lower().removeprefix("0x"),
    }
    for program, expected_hash in expected_program_hashes.items():
        actual_hash = sha256(program)
        if actual_hash != expected_hash:
            raise PackageError(
                f"SP1 ELF hash mismatch for {program.relative_to(ROOT)}: "
                f"expected={expected_hash} actual={actual_hash}; rebuild the vkey"
            )

    vault_art = artifact(VAULT_ARTIFACT)
    verifier_art = artifact(VERIFIER_ARTIFACT)
    exit_executor_art = artifact(EXIT_EXECUTOR_ARTIFACT)
    vault_creation = hex_bytes(
        vault_art["bytecode"]["object"],
        len(bytes.fromhex(vault_art["bytecode"]["object"].removeprefix("0x"))),
        "vault creation bytecode",
    )
    verifier_creation = hex_bytes(
        verifier_art["bytecode"]["object"],
        len(bytes.fromhex(verifier_art["bytecode"]["object"].removeprefix("0x"))),
        "verifier creation bytecode",
    )
    exit_executor_creation = bytes.fromhex(
        exit_executor_art["bytecode"]["object"].removeprefix("0x")
    )
    exit_executor_runtime = bytes.fromhex(
        exit_executor_art["deployedBytecode"]["object"].removeprefix("0x")
    )
    vault_runtime, immutable_layout = link_vault_runtime(
        vault_art, value["weth"], value["weth_runtime_code_hash"]
    )
    vault_runtime_hash = "0x" + k256(vault_runtime).hex()
    verifier_address = create_address(value["deployer"], int(value["deployer_nonce"]))
    vault_address = create_address(value["deployer"], int(value["deployer_nonce"]) + 1)
    vault_creation_hash = "0x" + k256(vault_creation).hex()

    manifest_preimage = canonical_manifest_preimage(
        vault=vault_address,
        token=value["weth"],
        creation_bytecode_hash=vault_creation_hash,
    )
    ingress_policy_hash = k256(manifest_preimage).hex()
    balance_key = k256(
        bytes(12)
        + hex_bytes(vault_address, 20, "vault")
        + (3).to_bytes(32, "big")
    ).hex()
    ingress_policy = {
        "schema": "postfiat.pfeth.ethereum_ingress_policy.v1",
        "route_id": "ethereum-mainnet-weth-v1",
        "source_chain_id": 1,
        "genesis_validators_root": "0x4b363db94e286120d76eb905340fdd4e54bfe9f06bf33ff6cf5ad27f511bfe95",
        "vault_address": vault_address.lower(),
        "vault_runtime_code_hash": vault_runtime_hash.lower(),
        "token_address": value["weth"].lower(),
        "token_runtime_code_hash": value["weth_runtime_code_hash"].lower(),
        "token_balance_storage_key": "0x" + balance_key,
        "manifest_hash": "0x" + ingress_policy_hash,
        "manifest_preimage_hex": "0x" + manifest_preimage.hex(),
    }
    profile = {
        "schema": "postfiat.vault_bridge.route_profile.v1",
        "route_id": "ethereum-mainnet-weth-v1",
        "asset_id": value["pfeth_asset_id"].lower(),
        "source_chain_id": 1,
        "vault_address": vault_address.lower(),
        "vault_runtime_code_hash": vault_runtime_hash.lower(),
        "token_address": value["weth"].lower(),
        "token_runtime_code_hash": value["weth_runtime_code_hash"].lower(),
        "route_epoch": int(value["route_epoch"]),
        "verifier_kind": "sp1-groth16",
        "evidence_tier": "receipt-proven",
        "verifier_policy_hash": ingress_policy_hash,
        "verifier_program_vkey": value["ingress_program_vkey"].lower(),
        "verifier_proof_encoding": "groth16",
        "max_proof_bytes": int(value["max_proof_bytes"]),
        "max_public_values_bytes": int(value["max_public_values_bytes"]),
        "max_snapshot_age_blocks": int(value.get("max_snapshot_age_blocks", 900)),
        "challenge_window_blocks": int(value.get("challenge_window_blocks", 64)),
        "max_epoch_gap_blocks": int(value.get("max_epoch_gap_blocks", 128)),
        "settle_deadline_blocks": int(value.get("settle_deadline_blocks", 256)),
        "min_challenge_bond": int(value.get("min_challenge_bond", 1)),
        "min_attestations": 0,
        "minimum_confirmations": 0,
        "activation_height": int(value["activation_height"]),
        "expires_at_height": int(value["expires_at_height"]),
    }
    route_hash = profile_hash(profile)
    route_commitment = "0x" + k256(bytes.fromhex(route_hash)).hex()
    binding = route_binding(route_hash, int(value["route_epoch"]))
    pftl_bootstrap = generate_pftl_bootstrap(
        value,
        output,
        vault_address=vault_address,
        ingress_policy_hash=ingress_policy_hash,
        route_hash=route_hash,
    )

    chain_hash = "0x" + k256(value["pftl_chain_id"].encode()).hex()
    genesis_commitment = "0x" + k256(
        hex_bytes(value["pftl_genesis_hash"], 48, "genesis")
    ).hex()
    asset_commitment = "0x" + k256(
        hex_bytes(value["pfeth_asset_id"], 48, "asset")
    ).hex()
    checkpoint_commitment = "0x" + k256(
        hex_bytes(value["initial_checkpoint_block_id"], 48, "checkpoint")
    ).hex()
    committee_commitment = "0x" + k256(
        hex_bytes(value["initial_committee_root"], 48, "committee")
    ).hex()
    verifier_config = (
        to_checksum_address(value["sp1_verifier"]),
        hex_bytes(value["egress_program_vkey"], 32, "egress vkey"),
        hex_bytes(chain_hash, 32, "chain hash"),
        hex_bytes(genesis_commitment, 32, "genesis commitment"),
        int(value["pftl_protocol_version"]),
        hex_bytes(route_commitment, 32, "route commitment"),
        int(value["route_epoch"]),
        hex_bytes(asset_commitment, 32, "asset commitment"),
        1,
        hex_bytes(vault_runtime_hash, 32, "vault runtime hash"),
        to_checksum_address(value["weth"]),
        hex_bytes(value["weth_runtime_code_hash"], 32, "token runtime hash"),
        int(value["max_proof_bytes"]),
        int(value["max_public_values_bytes"]),
        hex_bytes(checkpoint_commitment, 32, "checkpoint commitment"),
        int(value["initial_finalized_height"]),
        hex_bytes(committee_commitment, 32, "committee commitment"),
    )
    verifier_init = verifier_creation + abi_encode(
        [
            "(address,bytes32,bytes32,bytes32,uint32,bytes32,uint64,bytes32,uint64,bytes32,address,bytes32,uint256,uint256,bytes32,uint64,bytes32)"
        ],
        [verifier_config],
    )
    vault_init = vault_creation + abi_encode(
        ["address", "address", "bytes32", "address"],
        [
            to_checksum_address(value["weth"]),
            verifier_address,
            hex_bytes(value["weth_runtime_code_hash"], 32, "token runtime hash"),
            to_checksum_address(value["owner"]),
        ],
    )
    route_document = {
        "schema": "postfiat.vault_bridge.route_profile_document.v1",
        "route_profile": profile,
        "route_profile_hash": route_hash,
        "route_profile_hash_commitment": route_commitment,
        "route_binding": binding,
    }
    consumer = {
        "schema": "postfiat.pfeth.deployment_consumer.v1",
        "verifier_constructor": {
            "sp1Verifier": value["sp1_verifier"],
            "programVKey": value["egress_program_vkey"],
            "pftlChainIdHash": chain_hash,
            "pftlGenesisHashCommitment": genesis_commitment,
            "pftlProtocolVersion": int(value["pftl_protocol_version"]),
            "routeProfileHashCommitment": route_commitment,
            "routeEpoch": int(value["route_epoch"]),
            "assetIdCommitment": asset_commitment,
            "arbitrumChainId": 1,
            "vaultRuntimeCodeHash": vault_runtime_hash,
            "token": value["weth"],
            "tokenRuntimeCodeHash": value["weth_runtime_code_hash"],
            "maxProofBytes": int(value["max_proof_bytes"]),
            "maxPublicValuesBytes": int(value["max_public_values_bytes"]),
            "initialCheckpointCommitment": checkpoint_commitment,
            "initialFinalizedHeight": int(value["initial_finalized_height"]),
            "initialCommitteeRootCommitment": committee_commitment,
        },
        "vault_constructor": {
            "token": value["weth"],
            "finalityVerifier": verifier_address,
            "tokenRuntimeCodeHash": value["weth_runtime_code_hash"],
            "initialOwner": value["owner"],
        },
    }
    manifest = {
        "schema": "postfiat.pfeth.ethereum_mainnet_deployment_manifest.v1",
        "status": "generated-not-reviewed-not-deployed",
        "network": "ethereum-mainnet",
        "source_chain_id": 1,
        "sp1_verifier": {
            "address": to_checksum_address(value["sp1_verifier"]),
            "runtime_code_hash": value["sp1_verifier_runtime_code_hash"].lower(),
        },
        "deployer": to_checksum_address(value["deployer"]),
        "deployer_nonce": int(value["deployer_nonce"]),
        "predicted_addresses": {
            "verifier": verifier_address,
            "vault": vault_address,
        },
        "route": route_document,
        "ingress_policy": ingress_policy,
        "pftl": {
            "chain_id": value["pftl_chain_id"],
            "genesis_hash": value["pftl_genesis_hash"],
            "asset_code": "PFETH",
            "asset_version": 1,
            "asset_precision": 9,
            "asset_id": value["pfeth_asset_id"],
            "initial_checkpoint_block_id": value["initial_checkpoint_block_id"],
            "initial_finalized_height": int(value["initial_finalized_height"]),
            "initial_committee_root": value["initial_committee_root"],
            "bootstrap": pftl_bootstrap,
        },
        "programs": {
            "ingress": {
                "path": str(INGRESS_ELF.relative_to(ROOT)),
                "sha256": value["ingress_program_elf_sha256"].lower().removeprefix("0x"),
                "vkey": value["ingress_program_vkey"],
            },
            "egress": {
                "path": str(EGRESS_ELF.relative_to(ROOT)),
                "sha256": value["egress_program_elf_sha256"].lower().removeprefix("0x"),
                "vkey": value["egress_program_vkey"],
            },
        },
        "contracts": {
            "WETHBridgeVaultL1": {
                "artifact": str(VAULT_ARTIFACT.relative_to(ROOT)),
                "artifact_sha256": sha256(VAULT_ARTIFACT),
                "creation_bytecode_keccak256": vault_creation_hash,
                "deployed_runtime_code_keccak256": vault_runtime_hash,
                "immutable_layout": immutable_layout,
                "init_code_keccak256": "0x" + k256(vault_init).hex(),
            },
            "PFTLFinalityVerifierV1": {
                "artifact": str(VERIFIER_ARTIFACT.relative_to(ROOT)),
                "artifact_sha256": sha256(VERIFIER_ARTIFACT),
                "init_code_keccak256": "0x" + k256(verifier_init).hex(),
            },
            "ExitExecutorV1": {
                "source": str(EXIT_EXECUTOR_SOURCE.relative_to(ROOT)),
                "source_sha256": sha256(EXIT_EXECUTOR_SOURCE),
                "artifact": str(EXIT_EXECUTOR_ARTIFACT.relative_to(ROOT)),
                "artifact_sha256": sha256(EXIT_EXECUTOR_ARTIFACT),
                "creation_bytecode_keccak256": "0x"
                + k256(exit_executor_creation).hex(),
                "deployed_runtime_code_keccak256": "0x"
                + k256(exit_executor_runtime).hex(),
                "deployment_status": "external-review-then-redeploy-on-both-chains",
            },
        },
        "gates": {
            "external_contract_and_circuit_review": "PENDING",
            "live_nonce_recheck": "PENDING",
            "validator_route_activation": "PENDING",
            "live_0_01_eth_round_trips": "PENDING",
        },
    }

    governance_instructions = {
        "schema": "postfiat.pfeth.vault_bridge_route_activation_instructions.v1",
        "status": "PENDING_VALIDATOR_GOVERNANCE",
        "profile_file": "governance-route-profile.json",
        "important": (
            "Use vault-bridge-route-profile-governance, not bridge_batch_domain. "
            "The latter creates a generic BridgeDomain and does not activate the governed proof route."
        ),
        "unsigned_controlled_testnet_command": [
            "postfiat-node",
            "vault-bridge-route-profile-governance",
            "--data-dir", "REPLACE_PROPOSER_DATA_DIR",
            "--validators", "REPLACE_VALIDATOR_CSV",
            "--support", "REPLACE_SUPPORT_CSV",
            "--veto-until-height", "0",
            "--profile-file", "governance-route-profile.json",
            "--amendment-file", "route-amendment.json",
            "--batch-file", "route-governance-batch.json",
        ],
        "signed_assembly_command": [
            "postfiat-node",
            "vault-bridge-route-profile-governance-assemble",
            "--data-dir", "REPLACE_PROPOSER_DATA_DIR",
            "--profile-file", "governance-route-profile.json",
            "--signed-amendment-file", "REPLACE_SIGNED_AMENDMENT.json",
            "--proposal-slot", "REPLACE_PROPOSAL_SLOT",
            "--batch-file", "route-governance-batch.json",
        ],
        "submission_gate": (
            "Submit only after contract deployment readback, fresh PFTL tip/committee verification, "
            "and validator authorization. Require an accepted governance receipt and active "
            "vault_bridge_route readback before issuing pfETH."
        ),
    }

    write(output / "route-profile.json", route_document)
    write(output / "governance-route-profile.json", profile)
    write(output / "route-activation-instructions.json", governance_instructions)
    write(output / "ingress-policy.json", ingress_policy)
    write(output / "deployment-consumer.json", consumer)
    write(output / "verifier-init-code.hex", ("0x" + verifier_init.hex() + "\n").encode())
    write(output / "vault-init-code.hex", ("0x" + vault_init.hex() + "\n").encode())
    write(output / "manifest.json", manifest)
    manifest_digest = sha256(output / "manifest.json")
    review_path = output / "external-review.json"
    existing_review = read_json(review_path) if review_path.is_file() else {}
    if (
        existing_review.get("schema") == "postfiat.pfeth.external_review_gate.v1"
        and existing_review.get("status") == "PASS"
        and existing_review.get("manifest_sha256") == manifest_digest
    ):
        review_gate = existing_review
    else:
        review_gate = {
            "schema": "postfiat.pfeth.external_review_gate.v1",
            "status": "PENDING",
            "manifest_sha256": manifest_digest,
            "reviewed_contracts": [],
            "reviewed_programs": [],
            "reviewer": None,
            "reviewed_at": None,
            "signature_or_report": None,
            "instruction": (
                "A reviewer changes status to PASS only after reviewing all three contracts, "
                "the ingress and egress guests, public-value bindings, atom scaling, "
                "constructor payloads, PFTL bootstrap operations, and predicted addresses."
            ),
        }
        write(review_path, review_gate)

    checkpoint_gate_path = output / "pftl-checkpoint-gate.json"
    existing_checkpoint_gate = (
        read_json(checkpoint_gate_path) if checkpoint_gate_path.is_file() else {}
    )
    if (
        existing_checkpoint_gate.get("schema")
        == "postfiat.pfeth.pftl_checkpoint_gate.v1"
        and existing_checkpoint_gate.get("status") == "PASS"
        and existing_checkpoint_gate.get("manifest_sha256") == manifest_digest
        and existing_checkpoint_gate.get("chain_id") == value["pftl_chain_id"]
        and existing_checkpoint_gate.get("genesis_hash") == value["pftl_genesis_hash"]
        and existing_checkpoint_gate.get("checkpoint_block_id")
        == value["initial_checkpoint_block_id"]
        and int(existing_checkpoint_gate.get("finalized_height", 0))
        == int(value["initial_finalized_height"])
        and existing_checkpoint_gate.get("committee_root")
        == value["initial_committee_root"]
    ):
        checkpoint_gate = existing_checkpoint_gate
    else:
        checkpoint_gate = {
            "schema": "postfiat.pfeth.pftl_checkpoint_gate.v1",
            "status": "PENDING",
            "manifest_sha256": manifest_digest,
            "chain_id": value["pftl_chain_id"],
            "genesis_hash": value["pftl_genesis_hash"],
            "checkpoint_block_id": value["initial_checkpoint_block_id"],
            "finalized_height": int(value["initial_finalized_height"]),
            "committee_root": value["initial_committee_root"],
            "observed_at": None,
            "source_receipt": None,
            "instruction": (
                "Immediately before deployment, query the active PFTL fleet, rebase the package "
                "to its finalized tip and consensus committee when needed, then set PASS with a "
                "redaction-safe authenticated source receipt."
            ),
        }
        write(checkpoint_gate_path, checkpoint_gate)
    summary = {
        "ok": True,
        "status": manifest["status"],
        "manifest": "manifest.json",
        "manifest_sha256": manifest_digest,
        "review_gate": "external-review.json",
        "pftl_checkpoint_gate": "pftl-checkpoint-gate.json",
        "route_activation_instructions": "route-activation-instructions.json",
        "pftl_bootstrap": "pftl-bootstrap-report.json",
        "predicted_addresses": manifest["predicted_addresses"],
        "route_profile_hash": route_hash,
        "route_binding": binding,
        "vault_runtime_code_hash": vault_runtime_hash,
        "ingress_vkey": value["ingress_program_vkey"],
        "egress_vkey": value["egress_program_vkey"],
    }
    write(output / "package-summary.json", summary)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--build-contracts",
        action="store_true",
        help="run forge build before packaging",
    )
    args = parser.parse_args()
    if args.build_contracts:
        subprocess.run(
            ["forge", "build"],
            cwd=ROOT / "crates/ethereum-contracts",
            check=True,
        )
    try:
        result = generate(read_json(args.input), args.output)
    except (PackageError, OSError, ValueError) as error:
        raise SystemExit(f"pfeth_package=failed: {error}") from error
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
