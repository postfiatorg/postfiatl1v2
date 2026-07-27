#!/usr/bin/env python3
"""Digest-gated Ethereum mainnet deployment driver for pfUSDC epoch 4."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASE_DRIVER = ROOT / "scripts/pfusdc-eth-mainnet-deploy.py"
MANIFEST = ROOT / "deployments/pfusdc-eth-mainnet-20260726-epoch4/manifest.mainnet-epoch4.json"
EVIDENCE_DIR = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/deploy"
)
MANIFEST_SHA256 = "d3fad46f9c8ee5fa2c064e3a35ed8dd0b5b1f9909f9e3f155bf804ede2330d8e"
VERIFIER_NONCE = 157
VAULT_NONCE = 158
SELF_AUDIT = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/"
    "reviews/from-zero-audit.json"
)
SELF_AUDIT_SCRIPT = SELF_AUDIT.parents[1] / "audit_mainnet_epoch4_from_zero.py"
CONSUMER = (
    ROOT
    / "deployments/pfusdc-eth-mainnet-20260726-epoch4/"
    "deploy-consumer.mainnet-epoch4.json"
)
CONSUMER_LOG = SELF_AUDIT.parents[1] / "package/deploy-consumer-schema.audit.log"
PAYLOAD_SUMMARY = SELF_AUDIT.parents[1] / "pftl-payloads/payload-summary.json"
EXPECTED_SELF_AUDIT_CHECKS = {
    "scope_chain_revision",
    "create_nonce_address_derivation",
    "artifact_and_bytecode_hashes",
    "vault_runtime_with_immutables",
    "verifier_runtime_with_immutables",
    "profile_policy_binding_commitment",
    "initial_checkpoint_h316",
    "contract_guest_storage",
    "deploy_consumer_ast",
    "pftl_payloads_c8_key_free",
    "live_mainnet_nonce_157_no_pending",
    "predicted_addresses_empty",
}


def load_driver():
    spec = importlib.util.spec_from_file_location("pfusdc_mainnet_base_deployer", BASE_DRIVER)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load base deploy driver: {BASE_DRIVER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


D = load_driver()
D.DEFAULT_MANIFEST = MANIFEST
D.DEFAULT_EVIDENCE_DIR = EVIDENCE_DIR
D.REQUIRED_MANIFEST_SHA256 = MANIFEST_SHA256


def validate_scope(manifest: dict[str, Any]) -> None:
    network = manifest.get("network")
    if not isinstance(network, dict):
        raise D.DeploymentError("manifest missing network")
    if network.get("source_chain_id") != D.ETHEREUM_MAINNET_CHAIN_ID:
        raise D.DeploymentError("manifest source_chain_id is not Ethereum mainnet")
    token = network.get("token")
    gateway = network.get("sp1_verifier_gateway")
    if not isinstance(token, dict) or str(token.get("address", "")).lower() != D.CANONICAL_USDC:
        raise D.DeploymentError("manifest token is not canonical Circle mainnet USDC")
    if not isinstance(gateway, dict) or str(gateway.get("address", "")).lower() != D.SP1_GATEWAY:
        raise D.DeploymentError("manifest SP1 gateway is not the canonical mainnet gateway")
    if manifest.get("revision") != "mainnet-epoch4":
        raise D.DeploymentError("manifest revision is not mainnet-epoch4")
    route = manifest.get("route")
    if (
        not isinstance(route, dict)
        or route.get("route_id") != "ethereum-mainnet-usdc-v1"
        or route.get("route_epoch") != 4
        or route.get("activation_height") != 318
        or route.get("binding_submission_height") != 317
        or route.get("registration_height") != 318
    ):
        raise D.DeploymentError("manifest route is not the pinned H317/H318 epoch-4 route")


def validate_offline_manifest(
    manifest_path: Path, manifest: dict[str, Any]
) -> dict[str, dict[str, Any]]:
    if manifest_path.resolve() != MANIFEST.resolve():
        raise D.DeploymentError("manifest path is not the pinned epoch-4 manifest")
    if D._sha256(manifest_path) != MANIFEST_SHA256:
        raise D.DeploymentError("manifest digest is not the pinned epoch-4 digest")
    for path, expected in {
        ("deployer", "address"): str,
        ("network", "execution_rpc_default"): str,
        ("network", "beacon_rpc_default"): str,
        ("network", "token", "runtime_code_hash"): str,
        ("network", "sp1_verifier_gateway", "runtime_code_hash"): str,
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
        ("route", "vault_runtime_code_hash"): str,
    }.items():
        D._required(manifest, path, expected)
    validate_scope(manifest)
    artifacts = D._contract_artifacts(manifest)
    verifier = artifacts["PFTLFinalityVerifierV1"]
    vault = artifacts["ERC20BridgeVaultL1"]
    if (verifier["precomputed_create_nonce"], vault["precomputed_create_nonce"]) != (
        VERIFIER_NONCE,
        VAULT_NONCE,
    ):
        raise D.DeploymentError("manifest CREATE nonces are not 157/158")
    deployer = manifest["deployer"]["address"]
    if D.create_address(deployer, VERIFIER_NONCE).lower() != verifier["address"].lower():
        raise D.DeploymentError("verifier CREATE address does not match nonce 157")
    if D.create_address(deployer, VAULT_NONCE).lower() != vault["address"].lower():
        raise D.DeploymentError("vault CREATE address does not match nonce 158")
    if manifest["route"].get("verifier_address", "").lower() != verifier["address"].lower():
        raise D.DeploymentError("route verifier differs from the artifact plan")
    if manifest["route"].get("vault_address", "").lower() != vault["address"].lower():
        raise D.DeploymentError("route vault differs from the artifact plan")
    D.build_verifier_constructor_inputs(manifest)
    return artifacts


def require_epoch4_authorization(
    manifest_path: Path, manifest: dict[str, Any], authorization_path: Path | None
) -> dict[str, Any]:
    if authorization_path is None or authorization_path.resolve() != SELF_AUDIT.resolve():
        raise D.DeploymentError("epoch-4 deployment requires the pinned from-zero self-audit")
    rerun = subprocess.run(
        [
            str(SELF_AUDIT_SCRIPT),
            "--manifest",
            str(manifest_path),
            "--consumer",
            str(CONSUMER),
            "--consumer-log",
            str(CONSUMER_LOG),
            "--payload-summary",
            str(PAYLOAD_SUMMARY),
            "--expected-manifest-sha256",
            MANIFEST_SHA256,
            "--output",
            str(SELF_AUDIT),
        ],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    if rerun.returncode != 0:
        detail = rerun.stdout.strip() or rerun.stderr.strip()
        raise D.DeploymentError(f"from-zero self-audit rerun failed: {detail}")
    result = D._load_json(SELF_AUDIT)
    if (
        result.get("schema") != "postfiat.pfusdc.mainnet_epoch4_from_zero_audit.v1"
        or result.get("status") != "PASS"
        or result.get("manifest_sha256") != MANIFEST_SHA256
    ):
        raise D.DeploymentError("from-zero self-audit is not a digest-bound PASS")
    checks = result.get("checks")
    if (
        not isinstance(checks, dict)
        or set(checks) != EXPECTED_SELF_AUDIT_CHECKS
        or any(value != "PASS" for value in checks.values())
    ):
        raise D.DeploymentError("from-zero self-audit check set is incomplete")
    if result.get("planned_deployments") != {
        "verifier": {
            "nonce": VERIFIER_NONCE,
            "address": manifest["route"]["verifier_address"],
        },
        "vault": {
            "nonce": VAULT_NONCE,
            "address": manifest["route"]["vault_address"],
        },
    }:
        raise D.DeploymentError("from-zero self-audit deployment plan drifted")
    return {
        "manifest_sha256": MANIFEST_SHA256,
        "authorization": result,
        "authorization_kind": "single-agent-from-zero-self-audit",
    }


D._validate_scope = validate_scope
D.validate_offline_manifest = validate_offline_manifest
D.require_mainnet_authorization = require_epoch4_authorization


if __name__ == "__main__":
    if not any(
        argument in {"--deployment-authorization", "--auditor-authorization"}
        for argument in sys.argv[1:]
    ):
        sys.argv.extend(["--deployment-authorization", str(SELF_AUDIT)])
    raise SystemExit(D.main())
