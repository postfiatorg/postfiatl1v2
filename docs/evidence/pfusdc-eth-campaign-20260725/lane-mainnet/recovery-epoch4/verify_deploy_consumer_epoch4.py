#!/usr/bin/env python3
"""AST-derived epoch-4 deploy-consumer and constructor cross-check."""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
BASE_CHECKER = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/package"
    / "verify_deploy_consumer.py"
)
BASE_DEPLOYER = ROOT / "scripts/pfusdc-eth-mainnet-deploy.py"
EPOCH4_DEPLOYER = ROOT / "scripts/pfusdc-eth-mainnet-epoch4-deploy.py"


def load_checker():
    spec = importlib.util.spec_from_file_location("pfusdc_deploy_consumer_base", BASE_CHECKER)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load base checker: {BASE_CHECKER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


C = load_checker()


def verify(manifest_path: Path, consumer_path: Path) -> list[str]:
    manifest = C.load_json(manifest_path)
    consumer = C.load_json(consumer_path)
    paths = C.manifest_paths_from_ast(BASE_DEPLOYER) | C.manifest_paths_from_ast(EPOCH4_DEPLOYER)
    required_types = {
        ("deployer", "address"): str,
        ("network", "source_chain_id"): int,
        ("network", "execution_rpc_default"): str,
        ("network", "beacon_rpc_default"): str,
        ("network", "sp1_verifier_gateway", "address"): str,
        ("network", "sp1_verifier_gateway", "runtime_code_hash"): str,
        ("network", "token", "address"): str,
        ("network", "token", "runtime_code_hash"): str,
        ("programs", "egress", "program_vkey"): str,
        ("programs", "max_proof_bytes"): int,
        ("programs", "max_public_values_bytes"): int,
        ("pftl", "chain_id_hash"): str,
        ("pftl", "genesis_hash_commitment"): str,
        ("pftl", "protocol_version"): int,
        ("pftl", "route_profile_hash_commitment"): str,
        ("pftl", "route_epoch"): int,
        ("pftl", "asset_id_commitment"): str,
        ("pftl", "vault_runtime_code_hash"): str,
        ("pftl", "initial_checkpoint_commitment"): str,
        ("pftl", "initial_finalized_height"): int,
        ("pftl", "initial_committee_root_commitment"): str,
        ("contracts", "artifacts"): list,
        ("status",): str,
    }
    for path in sorted(paths):
        if path:
            C.lookup(manifest, path)
    for path, expected in required_types.items():
        C.require_type(manifest, path, expected)
    artifacts = C.validate_artifacts(manifest)
    verifier, vault = C.constructor_tuples(manifest, artifacts)

    expected_verifier = tuple(
        consumer["verifier_constructor"][key]
        for key in (
            "sp1Verifier",
            "programVKey",
            "pftlChainIdHash",
            "pftlGenesisHashCommitment",
            "pftlProtocolVersion",
            "routeProfileHashCommitment",
            "routeEpoch",
            "assetIdCommitment",
            "arbitrumChainId",
            "vaultRuntimeCodeHash",
            "token",
            "tokenRuntimeCodeHash",
            "maxProofBytes",
            "maxPublicValuesBytes",
            "initialCheckpointCommitment",
            "initialFinalizedHeight",
            "initialCommitteeRootCommitment",
        )
    )
    expected_vault = tuple(
        consumer["vault_constructor"][key]
        for key in ("token", "finalityVerifier", "tokenRuntimeCodeHash", "initialOwner")
    )
    if verifier != expected_verifier:
        raise C.CheckError("verifier constructor differs from generated consumer")
    if vault != expected_vault:
        raise C.CheckError("vault constructor differs from generated consumer")
    runtime = manifest["route"]["vault_runtime_code_hash"]
    if manifest["pftl"]["vault_runtime_code_hash"] != runtime:
        raise C.CheckError("PFTL and route vault runtime hashes differ")
    if manifest["route"]["route_profile"]["vault_runtime_code_hash"] != runtime:
        raise C.CheckError("route profile and manifest vault runtime hashes differ")
    if (
        manifest["revision"],
        manifest["route"]["route_epoch"],
        manifest["pftl"]["initial_finalized_height"],
    ) != ("mainnet-epoch4", 4, 316):
        raise C.CheckError("epoch-4 revision/epoch/checkpoint tuple drifted")
    return [
        f"BASE_DEPLOY_SCRIPT: {BASE_DEPLOYER}",
        f"EPOCH4_DEPLOY_SCRIPT: {EPOCH4_DEPLOYER}",
        f"AST_MANIFEST_PATH_COUNT: {len(paths)}",
        "VERIFIER_CONSTRUCTOR: MATCH",
        "VAULT_CONSTRUCTOR: MATCH",
        "VAULT_RUNTIME: MATCH",
        "INITIAL_CHECKPOINT: H316",
        "DEPLOY_CONSUMER_SCHEMA: PASS",
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--consumer", required=True, type=Path)
    parser.add_argument("--log", required=True, type=Path)
    args = parser.parse_args()
    try:
        lines = verify(args.manifest, args.consumer)
    except (C.CheckError, KeyError, OSError, ValueError) as exc:
        args.log.parent.mkdir(parents=True, exist_ok=True)
        args.log.write_text(f"DEPLOY_CONSUMER_SCHEMA: FAIL — {exc}\n", encoding="utf-8")
        print(f"DEPLOY_CONSUMER_SCHEMA: FAIL — {exc}")
        return 1
    args.log.parent.mkdir(parents=True, exist_ok=True)
    args.log.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
