#!/usr/bin/env python3
"""Generate the pfUSDC Ethereum-mainnet epoch-5 verifier recovery package.

Epoch 5 replaces the immutable epoch-4 egress verifier after the PFTL
consensus-v2 block encoding gained the Uniswap receipt root.  The old lane is
not mutated: this package predicts a fresh verifier/vault CREATE pair and
binds both ingress and egress to a new route epoch.
"""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from typing import Any

from Crypto.Hash import keccak


ROOT = Path(__file__).resolve().parents[1]
EPOCH3_GENERATOR = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/package"
    / "generate_mainnet_epoch3_package.py"
)
BASE_INPUT = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/package"
    / "input-tuple.mainnet-epoch4.json"
)
DEPLOY_DIR = Path("deployments/pfusdc-eth-mainnet-20260728-epoch5")
EVIDENCE_DIR = Path(
    "docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/"
    "pfusdc-egress/recovery-epoch5/package"
)
CHECKPOINT_EVIDENCE = (
    ROOT
    / "docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/"
    "pfusdc-egress/emergency-halt-finality-h385/consensus/round-report.json"
)

REVISION = "mainnet-epoch5"
DEPLOYMENT_ID = "pfusdc-eth-mainnet-20260728-epoch5"
SOURCE_COMMIT = "40a8c1ec1108e837b7ff9a2442045342e74f18bb"
DEPLOYER_NONCE = 216
ROUTE_EPOCH = 5
BIND_HEIGHT = 386
ACTIVATION_HEIGHT = 387
CHECKPOINT_HEIGHT = 385
CHECKPOINT_BLOCK_ID = (
    "209e66c434f2969f81384e75a327b50c4ff2377203d1e432a8a656d1d9d2a246"
    "9257459c6ed33f9c49b30daa909ce535"
)
COMMITTEE_ROOT = (
    "9f7ce761878dd29a42151dd5f94af4886344764f4dba235bdf5c231f1700b8d37"
    "20e47cbda3dc1392d99c39ea8cda39b"
)
EGRESS_ELF_SHA256 = "ea0d3ef37ade9e2413646c8051b58f8e8123516e75da0937a8d47d4d9586f2fe"
EGRESS_PROGRAM_VKEY = "0x0026a156bfd82ce1d1bf3f966c77daba8d5c266b8cc29928474747c4a02ca89b"

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


class PackageError(RuntimeError):
    """Fail-closed epoch-5 package generation error."""


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise PackageError(f"cannot load module: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


G3 = load_module(EPOCH3_GENERATOR, "pfusdc_mainnet_epoch3_generator")
G3.DEPLOY_DIR = DEPLOY_DIR
G3.EVIDENCE_DIR = EVIDENCE_DIR


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def k256(value: bytes) -> bytes:
    digest = keccak.new(digest_bits=256)
    digest.update(value)
    return digest.digest()


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(G3.json_bytes(value))


def artifact_spec(path: Path, immutable_fields: tuple[str, ...]) -> dict[str, Any]:
    raw = path.read_bytes()
    artifact = json.loads(raw)
    creation = bytes.fromhex(artifact["bytecode"]["object"].removeprefix("0x"))
    deployed = bytes.fromhex(artifact["deployedBytecode"]["object"].removeprefix("0x"))
    references = artifact["deployedBytecode"].get("immutableReferences", {})
    ordered_ids = sorted(references, key=int)
    if len(ordered_ids) != len(immutable_fields):
        raise PackageError(
            f"{path}: expected {len(immutable_fields)} immutable fields, got {len(ordered_ids)}"
        )
    return {
        "path": str(path.relative_to(ROOT)),
        "artifact_sha256": hashlib.sha256(raw).hexdigest(),
        "creation_bytecode_keccak256": "0x" + k256(creation).hex(),
        "unlinked_deployed_bytecode_keccak256": "0x" + k256(deployed).hex(),
        "immutable_layout": dict(zip(ordered_ids, immutable_fields, strict=True)),
    }


def find_block(document: Any) -> dict[str, Any] | None:
    if isinstance(document, dict):
        if (
            document.get("height") == CHECKPOINT_HEIGHT
            and document.get("block_id") == CHECKPOINT_BLOCK_ID
        ):
            return document
        for value in document.values():
            match = find_block(value)
            if match is not None:
                return match
    elif isinstance(document, list):
        for value in document:
            match = find_block(value)
            if match is not None:
                return match
    return None


def contains_field(document: Any, field: str, expected: Any) -> bool:
    if isinstance(document, dict):
        if document.get(field) == expected:
            return True
        return any(contains_field(value, field, expected) for value in document.values())
    if isinstance(document, list):
        return any(contains_field(value, field, expected) for value in document)
    return False


def checkpoint_values() -> tuple[str, str]:
    evidence = json.loads(CHECKPOINT_EVIDENCE.read_text(encoding="utf-8"))
    if find_block(evidence) is None or not contains_field(
        evidence, "committee_root", COMMITTEE_ROOT
    ):
        raise PackageError("height-385 checkpoint evidence does not contain the pinned certified block")
    return (
        "0x" + k256(bytes.fromhex(CHECKPOINT_BLOCK_ID)).hex(),
        "0x" + k256(bytes.fromhex(COMMITTEE_ROOT)).hex(),
    )


def materialize() -> dict[str, Any]:
    base = json.loads(BASE_INPUT.read_text(encoding="utf-8"))
    if base.get("schema") != "postfiat.pfusdc.eth_mainnet_epoch4_input_tuple.v1":
        raise PackageError("base tuple is not the frozen epoch-4 input")

    checkpoint_commitment, committee_commitment = checkpoint_values()
    result = copy.deepcopy(base)
    result["schema"] = "postfiat.pfusdc.eth_mainnet_epoch5_input_tuple.v1"
    result["admission_baseline"] = {
        "active_epoch": 4,
        "activation_height": 318,
        "source": (
            "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/"
            "recovery-epoch4/pftl-execution/02-h318-register/h318-summary.json"
        ),
    }
    result["deployment"] = {
        "deployment_id": DEPLOYMENT_ID,
        "lane": "lane-mainnet",
        "revision": REVISION,
        "status": "generated-not-deployed",
    }
    result["route"]["epoch"] = ROUTE_EPOCH
    result["route"]["activation_height"] = ACTIVATION_HEIGHT
    result["route"]["schedule"] = {
        "bind_height": BIND_HEIGHT,
        "registration_activation_height": ACTIVATION_HEIGHT,
    }
    result["contracts"]["deployer_nonce"] = DEPLOYER_NONCE
    result["pftl"].update(
        {
            "checkpoint_block_hash": CHECKPOINT_BLOCK_ID,
            "committee_root": COMMITTEE_ROOT,
            "initial_checkpoint_commitment": checkpoint_commitment,
            "initial_finalized_height": CHECKPOINT_HEIGHT,
            "initial_committee_root_commitment": committee_commitment,
        }
    )
    result["pftl"]["source"]["h385_finalized_checkpoint"] = {
        "path": str(CHECKPOINT_EVIDENCE.relative_to(ROOT)),
        "sha256": sha256_file(CHECKPOINT_EVIDENCE),
    }
    result["programs"]["egress"].update(
        {
            "elf_sha256": EGRESS_ELF_SHA256,
            "program_vkey": EGRESS_PROGRAM_VKEY,
        }
    )
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
    epoch4_lineage = {
        "route_id": "ethereum-mainnet-usdc-v1",
        "route_epoch": 4,
        "status": "superseded_ingress_paused_egress_incompatible",
        "ingress_status": "superseded_paused",
        "egress_status": "quarantined_consensus_encoding_incompatible",
        "quarantine_reason": (
            "immutable egress verifier pins a guest predating the consensus-v2 "
            "pftl_uniswap_receipt_root field"
        ),
        "source": {
            "path": (
                "deployments/pfusdc-eth-mainnet-20260726-epoch4/"
                "manifest-lineage.mainnet-epoch4.json"
            ),
            "sha256": sha256_file(
                ROOT
                / "deployments/pfusdc-eth-mainnet-20260726-epoch4/"
                "manifest-lineage.mainnet-epoch4.json"
            ),
        },
    }
    result["lineage_history"] = [
        epoch4_lineage,
        *[
            entry
            for entry in result["lineage_history"]
            if not (
                entry.get("route_id") == epoch4_lineage["route_id"]
                and entry.get("route_epoch") == epoch4_lineage["route_epoch"]
            )
        ],
    ]
    result["egress_lineage"]["description"] = (
        "Historical lanes remain recorded, but epoch 4 is quarantined because its "
        "immutable guest cannot decode current consensus-v2 blocks."
    )
    result["source"]["commit"] = SOURCE_COMMIT
    result["source"]["epoch5_sources"] = {
        path: sha256_file(ROOT / path)
        for path in (
            "crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol",
            "crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol",
            "programs/pfusdc-egress/elf/pfusdc-egress-program",
        )
    }
    return result


def validate_sources(repo: Path, value: dict[str, Any]) -> None:
    expected = (
        value["route"]["epoch"],
        value["route"]["activation_height"],
        value["route"]["schedule"]["bind_height"],
        value["route"]["schedule"]["registration_activation_height"],
        value["contracts"]["deployer_nonce"],
        value["pftl"]["initial_finalized_height"],
    )
    if expected != (
        ROUTE_EPOCH,
        ACTIVATION_HEIGHT,
        BIND_HEIGHT,
        ACTIVATION_HEIGHT,
        DEPLOYER_NONCE,
        CHECKPOINT_HEIGHT,
    ):
        raise PackageError("epoch-5 route, nonce, or checkpoint drifted")
    if sha256_file(repo / value["programs"]["egress"]["elf_path"]) != EGRESS_ELF_SHA256:
        raise PackageError("current egress ELF does not match the epoch-5 pin")
    ingress = value["programs"]["ingress"]
    if sha256_file(repo / ingress["elf_path"]) != ingress["elf_sha256"]:
        raise PackageError("frozen Ethereum-mainnet ingress ELF drifted")
    checkpoint_commitment, committee_commitment = checkpoint_values()
    if (
        value["pftl"]["initial_checkpoint_commitment"] != checkpoint_commitment
        or value["pftl"]["initial_committee_root_commitment"] != committee_commitment
    ):
        raise PackageError("epoch-5 checkpoint commitments drifted")
    for name, spec in value["contracts"]["artifacts"].items():
        if sha256_file(repo / spec["path"]) != spec["artifact_sha256"]:
            raise PackageError(f"{name} artifact drifted")


def write_package(value: dict[str, Any]) -> dict[str, Any]:
    validate_sources(ROOT, value)
    G3.validate_sources = validate_sources
    docs = G3.generate(value, ROOT)
    files = {
        "input": ROOT / EVIDENCE_DIR / "input-tuple.mainnet-epoch5.json",
        "route": ROOT / DEPLOY_DIR / "route-profile.mainnet-epoch5.json",
        "manifest": ROOT / DEPLOY_DIR / "manifest.mainnet-epoch5.json",
        "lineage": ROOT / DEPLOY_DIR / "manifest-lineage.mainnet-epoch5.json",
        "consumer": ROOT / DEPLOY_DIR / "deploy-consumer.mainnet-epoch5.json",
        "nav": ROOT / EVIDENCE_DIR / "planned-nav-profile.mainnet-epoch5.json",
        "bind": ROOT / EVIDENCE_DIR / "planned-nav-bind.mainnet-epoch5.json",
        "registration": ROOT / EVIDENCE_DIR / "route-registration.mainnet-epoch5.json",
    }
    write_json(files["input"], value)
    for name in ("route", "manifest", "consumer", "nav", "bind", "registration"):
        write_json(files[name], docs[name])
    docs["lineage"]["current_revision"].update(
        {
            "manifest": {
                "path": str(files["manifest"].relative_to(ROOT)),
                "sha256": sha256_file(files["manifest"]),
            },
            "route_profile": {
                "path": str(files["route"].relative_to(ROOT)),
                "sha256": sha256_file(files["route"]),
            },
        }
    )
    write_json(files["lineage"], docs["lineage"])
    summary = {
        "schema": "postfiat.pfusdc.mainnet_epoch5_package_summary.v1",
        "status": "PASS",
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
        "checkpoint": {
            "height": CHECKPOINT_HEIGHT,
            "block_id": CHECKPOINT_BLOCK_ID,
            "checkpoint_commitment": value["pftl"]["initial_checkpoint_commitment"],
            "committee_root_commitment": value["pftl"][
                "initial_committee_root_commitment"
            ],
        },
        "plan": {
            "deployer_nonce": DEPLOYER_NONCE,
            "bind_height": BIND_HEIGHT,
            "registration_activation_height": ACTIVATION_HEIGHT,
        },
    }
    summary_path = ROOT / EVIDENCE_DIR / "package-summary.json"
    write_json(summary_path, summary)
    return summary


def main() -> int:
    try:
        summary = write_package(materialize())
    except (KeyError, OSError, PackageError, G3.InputError, ValueError) as exc:
        raise SystemExit(f"mainnet_epoch5_generation=failed: {exc}") from exc
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
