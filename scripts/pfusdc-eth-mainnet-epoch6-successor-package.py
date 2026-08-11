#!/usr/bin/env python3
"""Generate the distinct epoch-6 successor after the fail-closed h793 attempt."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASE_GENERATOR = ROOT / "scripts/pfusdc-eth-mainnet-epoch6-package.py"
DEPLOY_DIR = Path("deployments/pfusdc-eth-mainnet-20260809-epoch6-successor")
EVIDENCE_DIR = Path(
    "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6-successor/package"
)
DEPLOYMENT_ID = "pfusdc-eth-mainnet-20260809-epoch6-successor"
DEPLOYER_NONCE = 316
ACTIVATION_HEIGHT = 795
CHECKPOINT_HEIGHT = 793
CHECKPOINT_BLOCK_ID = (
    "ad1aaaa5e061ef8a15fcc5d8e79fed699cfa00df3c422f90d70e392d48fe54820"
    "944d24b514b2d2325973c5d5ceedee2"
)
CHECKPOINT_STATE_ROOT = (
    "574deb2403d5cffbe4628d159311f164fb7c3e9f19054df5661f1df003e79aab8"
    "fda3d5c9cf1ee0c46dc0ce2b9799592"
)
CHECKPOINT_CERTIFICATE_ID = (
    "335b88f4260de38e0199257ae50a22fcd53586abd4a33bd3dbb1cc42c8f9a64e"
    "71c4a2d5c23c985919e2a4d163246cb7"
)


def load_module(path: Path):
    spec = importlib.util.spec_from_file_location("pfusdc_epoch6_base_generator", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load module: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


E6 = load_module(BASE_GENERATOR)
E6.DEPLOY_DIR = DEPLOY_DIR
E6.EVIDENCE_DIR = EVIDENCE_DIR
E6.DEPLOYMENT_ID = DEPLOYMENT_ID
E6.DEPLOYER_NONCE = DEPLOYER_NONCE
E6.ACTIVATION_HEIGHT = ACTIVATION_HEIGHT
E6.CHECKPOINT_HEIGHT = CHECKPOINT_HEIGHT
E6.CHECKPOINT_BLOCK_ID = CHECKPOINT_BLOCK_ID
E6.CHECKPOINT_STATE_ROOT = CHECKPOINT_STATE_ROOT
E6.CHECKPOINT_CERTIFICATE_ID = CHECKPOINT_CERTIFICATE_ID
E6.G3.DEPLOY_DIR = DEPLOY_DIR
E6.G3.EVIDENCE_DIR = EVIDENCE_DIR


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(E6.G3.json_bytes(value))


def materialize() -> dict[str, Any]:
    value = E6.materialize()
    value["schema"] = "postfiat.pfusdc.eth_mainnet_epoch6_successor_input_tuple.v1"
    value["deployment"]["deployment_id"] = DEPLOYMENT_ID
    value["recovery_predecessor"] = {
        "status": "deployed_quarantined_not_governed",
        "verifier_address": "0xd2191bdfa9f2750bc8b9d3cb3146291dd1251734",
        "vault_address": "0x2604fcd968c174533e6fa6ffb034c8f3798d69ea",
        "profile_hash": (
            "21076a4df45bca0b6802e1b2ba6373c1e10b32be3ce9923343ae7e066d64f806"
            "2440031e70ed2cfe8d5d890d126fb398"
        ),
        "rejection_height": 793,
        "rejection_code": "vault_bridge_route_profile_rejected",
        "rejection_reason": (
            "vault bridge route does not exactly match the active NAV proof profile"
        ),
    }
    return value


def write_package(value: dict[str, Any]) -> dict[str, Any]:
    E6.validate_sources(ROOT, value)
    E6.G3.validate_sources = E6.validate_sources
    docs = E6.G3.generate(value, ROOT)
    files = {
        "input": ROOT / EVIDENCE_DIR / "input-tuple.mainnet-epoch6.json",
        "checkpoint": ROOT / EVIDENCE_DIR / "h793-finalized-checkpoint-verification.json",
        "profile": ROOT / EVIDENCE_DIR / "h795-route-profile.json",
        "route": ROOT / DEPLOY_DIR / "route-profile.mainnet-epoch6.json",
        "manifest": ROOT / DEPLOY_DIR / "manifest.mainnet-epoch6.json",
        "lineage": ROOT / DEPLOY_DIR / "manifest-lineage.mainnet-epoch6.json",
        "consumer": ROOT / DEPLOY_DIR / "deploy-consumer.mainnet-epoch6.json",
        "nav": ROOT / EVIDENCE_DIR / "planned-nav-profile.mainnet-epoch6.json",
        "bind": ROOT / EVIDENCE_DIR / "planned-nav-bind.mainnet-epoch6.json",
        "registration": ROOT / EVIDENCE_DIR / "route-registration.mainnet-epoch6.json",
    }
    write_json(files["input"], value)
    write_json(files["checkpoint"], E6.checkpoint_evidence())
    write_json(files["profile"], docs["route"]["route_profile"])
    for name in ("route", "manifest", "consumer", "nav", "bind", "registration"):
        write_json(files[name], docs[name])
    docs["lineage"]["current_revision"].update(
        {
            "manifest": {
                "path": str(files["manifest"].relative_to(ROOT)),
                "sha256": E6.E5.sha256_file(files["manifest"]),
            },
            "route_profile": {
                "path": str(files["route"].relative_to(ROOT)),
                "sha256": E6.E5.sha256_file(files["route"]),
            },
        }
    )
    write_json(files["lineage"], docs["lineage"])
    summary = {
        "schema": "postfiat.pfusdc.mainnet_epoch6_successor_package_summary.v1",
        "status": "PASS",
        "outputs": {
            name: {"path": str(path.relative_to(ROOT)), "sha256": E6.E5.sha256_file(path)}
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
            "certificate_id": CHECKPOINT_CERTIFICATE_ID,
            "checkpoint_commitment": value["pftl"]["initial_checkpoint_commitment"],
            "committee_root_commitment": value["pftl"][
                "initial_committee_root_commitment"
            ],
        },
        "plan": {
            "deployer_nonce": DEPLOYER_NONCE,
            "activation_height": ACTIVATION_HEIGHT,
            "nav_bind_height": 794,
        },
    }
    write_json(ROOT / EVIDENCE_DIR / "package-summary.json", summary)
    return summary


def main() -> int:
    try:
        summary = write_package(materialize())
    except (KeyError, OSError, RuntimeError, ValueError) as exc:
        raise SystemExit(f"mainnet_epoch6_successor_generation=failed: {exc}") from exc
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
