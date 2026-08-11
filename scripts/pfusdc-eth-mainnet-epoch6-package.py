#!/usr/bin/env python3
"""Generate the A666 pfUSDC Ethereum-mainnet epoch-6 egress redeploy package."""

from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
from typing import Any

from Crypto.Hash import keccak


ROOT = Path(__file__).resolve().parents[1]
EPOCH5_GENERATOR = ROOT / "scripts/pfusdc-eth-mainnet-epoch5-package.py"
BASE_INPUT = (
    ROOT
    / "docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/"
    "pfusdc-egress/recovery-epoch5/package/input-tuple.mainnet-epoch5.json"
)
DEPLOY_DIR = Path("deployments/pfusdc-eth-mainnet-20260809-epoch6")
EVIDENCE_DIR = Path("docs/evidence/a666-egress-lane-redeploy-20260809/epoch6/package")

REVISION = "mainnet-epoch6"
DEPLOYMENT_ID = "pfusdc-eth-mainnet-20260809-epoch6"
SOURCE_COMMIT = "9d14fdcc9e58ebcd240b71ff5c26714d7701d530"
DEPLOYER_NONCE = 314
ROUTE_EPOCH = 6
ACTIVATION_HEIGHT = 793
CHECKPOINT_HEIGHT = 792
CHECKPOINT_BLOCK_ID = (
    "e7a9c178fd108620a5c195aee74292489898d4aaebe4dfc6ff4548393b434c6f"
    "e883b2cabeb5609ad8ce312fddc14040"
)
CHECKPOINT_STATE_ROOT = (
    "6d643c143a2168431a94597fe1b7172e6d7073c2104d9a697b6957ac3f90f1d"
    "5de993ed389adf6a079f8dd1434192452"
)
CHECKPOINT_CERTIFICATE_ID = (
    "5103002493b1e31a763b3ea12016a7b6cac886d631a07228623a63cbad1be313e"
    "e6fe2ddba79d20f3dbd2fa1993b719e"
)
COMMITTEE_ROOT = (
    "9f7ce761878dd29a42151dd5f94af4886344764f4dba235bdf5c231f1700b8d37"
    "20e47cbda3dc1392d99c39ea8cda39b"
)
EGRESS_ELF_PATH = (
    "programs/pfusdc-egress/target/elf-compilation/"
    "riscv64im-succinct-zkvm-elf/release/pfusdc-egress-program"
)
EGRESS_ELF_SHA256 = "4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0"
EGRESS_PROGRAM_VKEY = "0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87"


class PackageError(RuntimeError):
    """Fail-closed epoch-6 package generation error."""


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise PackageError(f"cannot load module: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


E5 = load_module(EPOCH5_GENERATOR, "pfusdc_mainnet_epoch5_generator")
G3 = E5.G3
G3.DEPLOY_DIR = DEPLOY_DIR
G3.EVIDENCE_DIR = EVIDENCE_DIR


def k256(value: bytes) -> str:
    digest = keccak.new(digest_bits=256)
    digest.update(value)
    return "0x" + digest.hexdigest()


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(G3.json_bytes(value))


def checkpoint_evidence() -> dict[str, Any]:
    return {
        "schema": "postfiat.finalized_checkpoint_verification.v1",
        "verified": True,
        "verification_basis": "consensus-v2-finalized-checkpoint",
        "chain_id": "postfiat-wan-devnet-2",
        "protocol_version": 1,
        "checkpoint_height": CHECKPOINT_HEIGHT,
        "checkpoint_block_hash": CHECKPOINT_BLOCK_ID,
        "checkpoint_state_root": CHECKPOINT_STATE_ROOT,
        "certificate_id": CHECKPOINT_CERTIFICATE_ID,
        "committee_epoch": 1,
        "committee_root": COMMITTEE_ROOT,
        "validator_count": 6,
        "quorum": 5,
        "source": "validator-0 verify-finalized-checkpoint after old-bucket impairment finality",
    }


def materialize() -> dict[str, Any]:
    base = json.loads(BASE_INPUT.read_text(encoding="utf-8"))
    result = copy.deepcopy(base)
    result["schema"] = "postfiat.pfusdc.eth_mainnet_epoch6_input_tuple.v1"
    result["admission_baseline"] = {
        "active_epoch": 5,
        "activation_height": 387,
        "source": "deployments/pfusdc-eth-mainnet-20260728-epoch5/manifest.mainnet-epoch5.json",
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
        "bind_height": ACTIVATION_HEIGHT,
        "registration_activation_height": ACTIVATION_HEIGHT,
    }
    result["contracts"]["deployer_nonce"] = DEPLOYER_NONCE
    result["pftl"].update(
        {
            "checkpoint_block_hash": CHECKPOINT_BLOCK_ID,
            "committee_root": COMMITTEE_ROOT,
            "initial_checkpoint_commitment": k256(bytes.fromhex(CHECKPOINT_BLOCK_ID)),
            "initial_finalized_height": CHECKPOINT_HEIGHT,
            "initial_committee_root_commitment": k256(bytes.fromhex(COMMITTEE_ROOT)),
        }
    )
    result["pftl"]["source"]["h792_finalized_checkpoint"] = {
        "path": str(EVIDENCE_DIR / "h792-finalized-checkpoint-verification.json"),
        "certificate_id": CHECKPOINT_CERTIFICATE_ID,
    }
    result["programs"]["egress"].update(
        {
            "elf_path": EGRESS_ELF_PATH,
            "elf_sha256": EGRESS_ELF_SHA256,
            "program_vkey": EGRESS_PROGRAM_VKEY,
        }
    )
    result["contracts"]["artifacts"] = {
        "ERC20BridgeVaultL1": E5.artifact_spec(
            ROOT / "crates/ethereum-contracts/out/ERC20BridgeVaultL1.sol/ERC20BridgeVaultL1.json",
            ("token_address", "token_runtime_code_hash"),
        ),
        "PFTLFinalityVerifierV1": E5.artifact_spec(
            ROOT / "crates/ethereum-contracts/out/PFTLFinalityVerifierV1.sol/PFTLFinalityVerifierV1.json",
            E5.VERIFIER_IMMUTABLE_FIELDS,
        ),
    }
    epoch5_lineage = {
        "route_id": "ethereum-mainnet-usdc-v1",
        "route_epoch": 5,
        "status": "superseded_egress_accounting_impaired",
        "ingress_status": "superseded",
        "egress_status": "retired_accounting_impaired",
        "quarantine_reason": "old-vault bucket counted factor finalized to zero at PFTL height 792",
        "source": {
            "path": "deployments/pfusdc-eth-mainnet-20260728-epoch5/manifest-lineage.mainnet-epoch5.json",
            "sha256": E5.sha256_file(
                ROOT
                / "deployments/pfusdc-eth-mainnet-20260728-epoch5/"
                "manifest-lineage.mainnet-epoch5.json"
            ),
        },
    }
    result["lineage_history"] = [
        epoch5_lineage,
        *[
            item
            for item in result["lineage_history"]
            if not (item.get("route_id") == epoch5_lineage["route_id"] and item.get("route_epoch") == 5)
        ],
    ]
    result["egress_lineage"]["description"] = (
        "Epoch 5 remains archived with its PFTL accounting bucket impaired to zero; "
        "epoch 6 binds the fresh egress ELF and a current certified checkpoint."
    )
    result["source"]["commit"] = SOURCE_COMMIT
    result["source"]["epoch6_sources"] = {
        path: E5.sha256_file(ROOT / path)
        for path in (
            "crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol",
            "crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol",
            EGRESS_ELF_PATH,
        )
    }
    return result


def validate_sources(repo: Path, value: dict[str, Any]) -> None:
    if (
        value["route"]["epoch"],
        value["route"]["activation_height"],
        value["contracts"]["deployer_nonce"],
        value["pftl"]["initial_finalized_height"],
    ) != (ROUTE_EPOCH, ACTIVATION_HEIGHT, DEPLOYER_NONCE, CHECKPOINT_HEIGHT):
        raise PackageError("epoch-6 route, nonce, or checkpoint drifted")
    if E5.sha256_file(repo / EGRESS_ELF_PATH) != EGRESS_ELF_SHA256:
        raise PackageError("fresh egress ELF drifted")
    if value["programs"]["egress"]["program_vkey"] != EGRESS_PROGRAM_VKEY:
        raise PackageError("fresh egress vkey drifted")
    for name, spec in value["contracts"]["artifacts"].items():
        if E5.sha256_file(repo / spec["path"]) != spec["artifact_sha256"]:
            raise PackageError(f"{name} artifact drifted")


def write_package(value: dict[str, Any]) -> dict[str, Any]:
    validate_sources(ROOT, value)
    G3.validate_sources = validate_sources
    docs = G3.generate(value, ROOT)
    files = {
        "input": ROOT / EVIDENCE_DIR / "input-tuple.mainnet-epoch6.json",
        "checkpoint": ROOT / EVIDENCE_DIR / "h792-finalized-checkpoint-verification.json",
        "profile": ROOT / EVIDENCE_DIR / "h793-route-profile.json",
        "route": ROOT / DEPLOY_DIR / "route-profile.mainnet-epoch6.json",
        "manifest": ROOT / DEPLOY_DIR / "manifest.mainnet-epoch6.json",
        "lineage": ROOT / DEPLOY_DIR / "manifest-lineage.mainnet-epoch6.json",
        "consumer": ROOT / DEPLOY_DIR / "deploy-consumer.mainnet-epoch6.json",
        "nav": ROOT / EVIDENCE_DIR / "planned-nav-profile.mainnet-epoch6.json",
        "bind": ROOT / EVIDENCE_DIR / "planned-nav-bind.mainnet-epoch6.json",
        "registration": ROOT / EVIDENCE_DIR / "route-registration.mainnet-epoch6.json",
    }
    write_json(files["input"], value)
    write_json(files["checkpoint"], checkpoint_evidence())
    write_json(files["profile"], docs["route"]["route_profile"])
    for name in ("route", "manifest", "consumer", "nav", "bind", "registration"):
        write_json(files[name], docs[name])
    docs["lineage"]["current_revision"].update(
        {
            "manifest": {"path": str(files["manifest"].relative_to(ROOT)), "sha256": E5.sha256_file(files["manifest"])},
            "route_profile": {"path": str(files["route"].relative_to(ROOT)), "sha256": E5.sha256_file(files["route"])},
        }
    )
    write_json(files["lineage"], docs["lineage"])
    summary = {
        "schema": "postfiat.pfusdc.mainnet_epoch6_package_summary.v1",
        "status": "PASS",
        "outputs": {name: {"path": str(path.relative_to(ROOT)), "sha256": E5.sha256_file(path)} for name, path in files.items()},
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
            "verifier_runtime": docs["manifest"]["contracts"]["artifacts"][0]["deployed_runtime_code_keccak256"],
        },
        "checkpoint": {
            "height": CHECKPOINT_HEIGHT,
            "block_id": CHECKPOINT_BLOCK_ID,
            "checkpoint_commitment": value["pftl"]["initial_checkpoint_commitment"],
            "committee_root_commitment": value["pftl"]["initial_committee_root_commitment"],
        },
        "plan": {"deployer_nonce": DEPLOYER_NONCE, "activation_height": ACTIVATION_HEIGHT},
    }
    write_json(ROOT / EVIDENCE_DIR / "package-summary.json", summary)
    return summary


def main() -> int:
    try:
        summary = write_package(materialize())
    except (KeyError, OSError, PackageError, ValueError) as exc:
        raise SystemExit(f"mainnet_epoch6_generation=failed: {exc}") from exc
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
