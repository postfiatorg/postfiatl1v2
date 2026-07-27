#!/usr/bin/env python3
"""Generate the mainnet epoch-4 fix-forward package from frozen epoch-3 lineage."""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from typing import Any

from Crypto.Hash import keccak


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
EPOCH3_GENERATOR = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/package"
    / "generate_mainnet_epoch3_package.py"
)
DEPLOY_DIR = Path("deployments/pfusdc-eth-mainnet-20260726-epoch4")
EVIDENCE_DIR = Path(
    "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/package"
)
REVISION = "mainnet-epoch4"
SCHEMA = "postfiat.pfusdc.eth_mainnet_epoch4_input_tuple.v1"
H316_CHECKPOINT_BLOCK_HASH = (
    "f7abc6a0a4a18a261c36a28bbaf0631ec77f9dd7dfe53545b8e4ffff40c67f9"
    "238a2b37e923bc81393e5cef84c56fd0c"
)
H316_CHECKPOINT_COMMITMENT = (
    "0xc13413daa58b7beaecc2eabf873f4b507e81e457a58c37a77170cccb59cdceff"
)
H316_CHECKPOINT_EVIDENCE = Path(
    "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/"
    "preflight/h316-finalized-checkpoint-verification.validator-0.json"
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


class InputError(ValueError):
    """Raised when epoch-4 inputs are incomplete or contradictory."""


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise InputError(f"cannot load module: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


G3 = load_module(EPOCH3_GENERATOR, "pfusdc_mainnet_epoch3_generator")
G3.DEPLOY_DIR = DEPLOY_DIR
G3.EVIDENCE_DIR = EVIDENCE_DIR


def json_bytes(value: object) -> bytes:
    return G3.json_bytes(value)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(json_bytes(value))


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def k256(value: bytes) -> bytes:
    digest = keccak.new(digest_bits=256)
    digest.update(value)
    return digest.digest()


def artifact_spec(path: Path, immutable_fields: tuple[str, ...]) -> dict[str, Any]:
    raw = path.read_bytes()
    artifact = json.loads(raw)
    bytecode = bytes.fromhex(artifact["bytecode"]["object"].removeprefix("0x"))
    deployed = bytes.fromhex(artifact["deployedBytecode"]["object"].removeprefix("0x"))
    references = artifact["deployedBytecode"].get("immutableReferences", {})
    ordered_ids = sorted(references, key=int)
    if len(ordered_ids) != len(immutable_fields):
        raise InputError(
            f"{path}: expected {len(immutable_fields)} immutable fields, got {len(ordered_ids)}"
        )
    return {
        "path": str(path.relative_to(ROOT)),
        "artifact_sha256": sha256_bytes(raw),
        "creation_bytecode_keccak256": "0x" + k256(bytecode).hex(),
        "unlinked_deployed_bytecode_keccak256": "0x" + k256(deployed).hex(),
        "immutable_layout": dict(zip(ordered_ids, immutable_fields, strict=True)),
    }


def validate_cross_check(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text(encoding="utf-8"))
    if (
        document.get("schema")
        != "postfiat.pfusdc.contract_guest_storage_cross_check.v1"
        or document.get("status") != "PASS"
        or document.get("decode_simulation") != "PASS"
    ):
        raise InputError("contract/guest storage cross-check is not a complete PASS")
    for key, source in (
        ("contract_source_sha256", ROOT / "crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol"),
        ("guest_source_sha256", ROOT / "programs/pfusdc-eth-mainnet-ingress/src/lib.rs"),
    ):
        if document.get(key) != sha256_file(source):
            raise InputError(f"contract/guest storage cross-check has stale {key}")
    return document


def validate_guest(tuple_data: dict[str, Any]) -> None:
    guest = tuple_data["guest"]
    info_path = ROOT / guest["program_info_path"]
    if sha256_file(info_path) != guest["program_info_sha256"]:
        raise InputError("frozen guest program-info digest drifted")
    info = json.loads(info_path.read_text(encoding="utf-8"))
    ingress = tuple_data["programs"]["ingress"]
    expected = {
        "elf_sha256": guest["required_elf_sha256"],
        "program_vkey": guest["required_program_vkey"],
        "route_id": tuple_data["route"]["route_id"],
        "source_chain_id": tuple_data["network"]["chain_id"],
        "genesis_validators_root": tuple_data["network"]["genesis_validators_root"],
    }
    for key, value in expected.items():
        if info.get(key) != value:
            raise InputError(f"frozen guest program-info mismatch: {key}")
    if ingress["elf_sha256"] != guest["required_elf_sha256"]:
        raise InputError("ingress ELF does not match frozen guest")
    if ingress["program_vkey"] != guest["required_program_vkey"]:
        raise InputError("ingress vkey does not match frozen guest")


def validate_epoch4_sources(tuple_data: dict[str, Any]) -> None:
    validate_guest(tuple_data)
    for program in tuple_data["programs"].values():
        if isinstance(program, dict) and "elf_path" in program:
            path = ROOT / program["elf_path"]
            if not path.is_file() or sha256_file(path) != program["elf_sha256"]:
                raise InputError(f"ELF hash mismatch: {program['elf_path']}")
    route = tuple_data["route"]
    if (
        tuple_data["network"]["chain_id"],
        route["epoch"],
        route["activation_height"],
        route["schedule"]["bind_height"],
        route["schedule"]["registration_activation_height"],
        tuple_data["contracts"]["deployer_nonce"],
    ) != (1, 4, 318, 317, 318, 157):
        raise InputError("mainnet epoch-4 chain/epoch/schedule/nonce drifted")
    for key in ("max_proof_bytes", "max_public_values_bytes"):
        if route["settings"].get(key) != 4096:
            raise InputError(f"route settings {key} must remain 4096")
    history = tuple_data.get("lineage_history")
    expected_history = {
        ("ethereum-mainnet-usdc-v1", 3),
        ("ethereum-sepolia-usdc-v1", 2),
        ("pfusdc-tier4-arbitrum-one-v1", 1),
    }
    if not isinstance(history, list) or {
        (entry.get("route_id"), entry.get("route_epoch")) for entry in history
    } != expected_history:
        raise InputError("epoch-4 lineage history is incomplete")
    for entry in history:
        if entry.get("egress_status") != "pinned_resolvable":
            raise InputError("epoch-4 lineage lost pinned egress resolution")
        source = entry.get("source")
        if not isinstance(source, dict):
            raise InputError("epoch-4 lineage source is missing")
        source_path = ROOT / str(source.get("path", ""))
        if not source_path.is_file() or sha256_file(source_path) != source.get("sha256"):
            raise InputError("epoch-4 lineage source digest drifted")
    cross_check = tuple_data["source"]["contract_guest_storage_cross_check"]
    cross_check_path = ROOT / cross_check["path"]
    if sha256_file(cross_check_path) != cross_check["sha256"]:
        raise InputError("epoch-4 tuple cross-check digest drifted")
    validate_cross_check(cross_check_path)


def materialize_epoch4(
    base: dict[str, Any],
    cross_check_path: Path,
    deployer_nonce: int,
    bind_height: int,
    registration_height: int,
) -> dict[str, Any]:
    if base.get("schema") != "postfiat.pfusdc.eth_mainnet_epoch3_input_tuple.v1":
        raise InputError("base input is not the frozen epoch-3 tuple")
    if deployer_nonce != 157:
        raise InputError("epoch-4 deployer nonce must equal the live read-only nonce 157")
    if (bind_height, registration_height) != (317, 318):
        raise InputError("epoch-4 PFTL schedule must be H317 bind / H318 registration")
    cross_check = validate_cross_check(cross_check_path)

    result = copy.deepcopy(base)
    result["schema"] = SCHEMA
    result["admission_baseline"] = {
        "active_epoch": 3,
        "activation_height": 316,
        "source": (
            "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/"
            "pftl-execution/02-h316-register/h316-summary.json"
        ),
    }
    result["deployment"] = {
        "deployment_id": "pfusdc-eth-mainnet-20260726-epoch4",
        "lane": "lane-mainnet",
        "revision": REVISION,
        "status": "generated-not-deployed",
    }
    result["route"]["epoch"] = 4
    result["route"]["activation_height"] = registration_height
    result["route"]["schedule"] = {
        "bind_height": bind_height,
        "registration_activation_height": registration_height,
    }
    result["contracts"]["deployer_nonce"] = deployer_nonce
    checkpoint_evidence = json.loads((ROOT / H316_CHECKPOINT_EVIDENCE).read_text(encoding="utf-8"))
    if (
        checkpoint_evidence.get("verified") is not True
        or checkpoint_evidence.get("checkpoint_height") != 316
        or checkpoint_evidence.get("checkpoint_block_hash") != H316_CHECKPOINT_BLOCK_HASH
    ):
        raise InputError("H316 finalized-checkpoint evidence is not a verified exact match")
    result["pftl"].update(
        {
            "checkpoint_block_hash": H316_CHECKPOINT_BLOCK_HASH,
            "initial_checkpoint_commitment": H316_CHECKPOINT_COMMITMENT,
            "initial_finalized_height": 316,
        }
    )
    result["pftl"]["source"]["h316_finalized_checkpoint_verification"] = {
        "path": str(H316_CHECKPOINT_EVIDENCE),
        "sha256": sha256_file(ROOT / H316_CHECKPOINT_EVIDENCE),
    }
    result["contracts"]["artifacts"] = {
        "ERC20BridgeVaultL1": artifact_spec(
            ROOT / "crates/ethereum-contracts/out/ERC20BridgeVaultL1.sol/ERC20BridgeVaultL1.json",
            ("token_address", "token_runtime_code_hash"),
        ),
        "PFTLFinalityVerifierV1": artifact_spec(
            ROOT
            / "crates/ethereum-contracts/out/PFTLFinalityVerifierV1.sol/PFTLFinalityVerifierV1.json",
            VERIFIER_IMMUTABLE_FIELDS,
        ),
    }
    epoch3_lineage = {
        "route_id": "ethereum-mainnet-usdc-v1",
        "route_epoch": 3,
        "status": "superseded_ingress_pinned_egress_resolvable",
        "ingress_status": "superseded_storage_incompatible",
        "egress_status": "pinned_resolvable",
        "quarantine_reason": "vault did not persist the frozen ingress guest record",
        "source": {
            "path": "deployments/pfusdc-eth-mainnet-20260726/manifest-lineage.mainnet-epoch3.json",
            "sha256": sha256_file(
                ROOT
                / "deployments/pfusdc-eth-mainnet-20260726/"
                "manifest-lineage.mainnet-epoch3.json"
            ),
        },
    }
    result["lineage_history"] = [
        epoch3_lineage,
        *[
            entry
            for entry in result["lineage_history"]
            if not (
                entry.get("route_id") == epoch3_lineage["route_id"]
                and entry.get("route_epoch") == epoch3_lineage["route_epoch"]
            )
        ],
    ]
    result["egress_lineage"]["mainnet_epoch3_lineage_path"] = epoch3_lineage["source"]["path"]
    result["source"]["contract_guest_storage_cross_check"] = {
        "path": str(cross_check_path.relative_to(ROOT)),
        "sha256": sha256_file(cross_check_path),
        "contract_source_sha256": cross_check["contract_source_sha256"],
        "guest_source_sha256": cross_check["guest_source_sha256"],
    }
    result["source"]["epoch4_sources"] = {
        path: sha256_file(ROOT / path)
        for path in (
            "crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol",
            "crates/ethereum-contracts/test/ERC20BridgeVaultL1.t.sol",
            "scripts/pfusdc-contract-guest-storage-cross-check.py",
        )
    }
    validate_guest(result)
    return result


def load_tuple(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if value.get("schema") != SCHEMA:
        raise InputError("unsupported epoch-4 input tuple schema")
    if value["deployment"]["revision"] != REVISION:
        raise InputError("epoch-4 tuple revision drifted")
    if (
        value["route"]["epoch"],
        value["route"]["activation_height"],
        value["route"]["schedule"]["bind_height"],
        value["route"]["schedule"]["registration_activation_height"],
        value["contracts"]["deployer_nonce"],
        value["pftl"]["initial_finalized_height"],
        value["pftl"]["initial_checkpoint_commitment"],
        value["pftl"]["checkpoint_block_hash"],
    ) != (
        4,
        318,
        317,
        318,
        157,
        316,
        H316_CHECKPOINT_COMMITMENT,
        H316_CHECKPOINT_BLOCK_HASH,
    ):
        raise InputError("epoch-4 tuple live schedule/nonce drifted")
    validate_epoch4_sources(value)
    return value


def generate(tuple_data: dict[str, Any], repo: Path) -> dict[str, Any]:
    validate_epoch4_sources(tuple_data)
    G3.validate_sources = lambda _repo, _tuple: validate_epoch4_sources(_tuple)
    return G3.generate(tuple_data, repo)


def profile_hash(profile: dict[str, object]) -> str:
    return G3.profile_hash(profile)


def write_package(tuple_data: dict[str, Any], input_path: Path) -> dict[str, Any]:
    docs = generate(tuple_data, ROOT)
    revision = tuple_data["deployment"]["revision"]
    files = {
        "route": ROOT / DEPLOY_DIR / f"route-profile.{revision}.json",
        "manifest": ROOT / DEPLOY_DIR / f"manifest.{revision}.json",
        "lineage": ROOT / DEPLOY_DIR / f"manifest-lineage.{revision}.json",
        "consumer": ROOT / DEPLOY_DIR / f"deploy-consumer.{revision}.json",
        "nav": ROOT / EVIDENCE_DIR / f"planned-nav-profile.{revision}.json",
        "bind": ROOT / EVIDENCE_DIR / f"planned-nav-bind.{revision}.json",
        "registration": ROOT / EVIDENCE_DIR / f"route-registration.{revision}.json",
    }
    for name in ("route", "manifest", "consumer", "nav", "bind", "registration"):
        write_json(files[name], docs[name])
    docs["lineage"]["current_revision"].update(
        {
            "manifest": {
                "path": str(DEPLOY_DIR / files["manifest"].name),
                "sha256": sha256_file(files["manifest"]),
            },
            "route_profile": {
                "path": str(DEPLOY_DIR / files["route"].name),
                "sha256": sha256_file(files["route"]),
            },
        }
    )
    write_json(files["lineage"], docs["lineage"])
    summary = {
        "schema": "postfiat.pfusdc.mainnet_epoch4_package_summary.v1",
        "input_tuple_sha256": sha256_file(input_path),
        "outputs": {
            name: {"path": str(path.relative_to(ROOT)), "sha256": sha256_file(path)}
            for name, path in files.items()
        },
        "predicted_addresses": {
            "verifier": docs["manifest"]["route"]["verifier_address"],
            "vault": docs["manifest"]["route"]["vault_address"],
        },
        "hashes": {
            "policy": docs["manifest"]["route"]["policy_hash"],
            "profile": docs["manifest"]["route"]["route_profile_hash"],
            "binding": docs["manifest"]["route"]["route_binding"],
            "commitment": docs["manifest"]["route"]["route_profile_hash_commitment"],
            "vault_runtime": docs["manifest"]["route"]["vault_runtime_code_hash"],
            "verifier_runtime": docs["manifest"]["contracts"]["artifacts"][0][
                "deployed_runtime_code_keccak256"
            ],
        },
        "plan": {"bind_height": 317, "registration_activation_height": 318},
        "contract_guest_storage_cross_check": tuple_data["source"][
            "contract_guest_storage_cross_check"
        ],
        "zero_deviation": True,
    }
    write_json(ROOT / EVIDENCE_DIR / "package-summary.json", summary)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-input", required=True, type=Path)
    parser.add_argument("--cross-check", required=True, type=Path)
    parser.add_argument("--output-input", required=True, type=Path)
    parser.add_argument("--deployer-nonce", required=True, type=int)
    parser.add_argument("--bind-height", required=True, type=int)
    parser.add_argument("--registration-height", required=True, type=int)
    args = parser.parse_args()
    base = json.loads(args.base_input.read_text(encoding="utf-8"))
    tuple_data = materialize_epoch4(
        base,
        args.cross_check.resolve(),
        args.deployer_nonce,
        args.bind_height,
        args.registration_height,
    )
    output_input = args.output_input.resolve()
    write_json(output_input, tuple_data)
    summary = write_package(tuple_data, output_input)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (InputError, G3.InputError, KeyError, OSError, ValueError) as exc:
        raise SystemExit(f"mainnet_epoch4_generation=failed: {exc}") from exc
