#!/usr/bin/env python3
"""Digest-gated Ethereum-mainnet deployment driver for pfUSDC epoch 5."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASE_DRIVER = ROOT / "scripts/pfusdc-eth-mainnet-deploy.py"
MANIFEST = (
    ROOT
    / "deployments/pfusdc-eth-mainnet-20260728-epoch5/manifest.mainnet-epoch5.json"
)
EVIDENCE_DIR = (
    ROOT
    / "docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/"
    "pfusdc-egress/recovery-epoch5/deploy"
)
AUDIT = EVIDENCE_DIR.parent / "predeploy-audit.json"
AUDIT_SCRIPT = ROOT / "scripts/pfusdc-eth-mainnet-epoch5-audit.py"
MANIFEST_SHA256 = "66abebe1cc23b5154933d2771ffe8a9407cf16d257871e76cfddf8fb46a75559"
VERIFIER_NONCE = 216
VAULT_NONCE = 217


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
    if (
        manifest.get("revision"),
        manifest["network"].get("source_chain_id"),
        manifest["route"].get("route_id"),
        manifest["route"].get("route_epoch"),
        manifest["route"].get("activation_height"),
        manifest["route"].get("binding_submission_height"),
    ) != (
        "mainnet-epoch5",
        1,
        "ethereum-mainnet-usdc-v1",
        5,
        387,
        386,
    ):
        raise D.DeploymentError("manifest is not the pinned mainnet epoch-5 scope")
    if (
        manifest["network"]["token"]["address"].lower() != D.CANONICAL_USDC
        or manifest["network"]["sp1_verifier_gateway"]["address"].lower() != D.SP1_GATEWAY
    ):
        raise D.DeploymentError("manifest dependencies are not canonical mainnet USDC/SP1")


def validate_offline_manifest(
    manifest_path: Path, manifest: dict[str, Any]
) -> dict[str, dict[str, Any]]:
    if manifest_path.resolve() != MANIFEST.resolve():
        raise D.DeploymentError("manifest path is not the pinned epoch-5 manifest")
    if D._sha256(manifest_path) != MANIFEST_SHA256:
        raise D.DeploymentError("epoch-5 manifest digest drifted")
    validate_scope(manifest)
    artifacts = D._contract_artifacts(manifest)
    verifier = artifacts["PFTLFinalityVerifierV1"]
    vault = artifacts["ERC20BridgeVaultL1"]
    if (
        verifier["precomputed_create_nonce"],
        vault["precomputed_create_nonce"],
    ) != (VERIFIER_NONCE, VAULT_NONCE):
        raise D.DeploymentError("epoch-5 CREATE nonces drifted")
    deployer = manifest["deployer"]["address"]
    if (
        D.create_address(deployer, VERIFIER_NONCE).lower() != verifier["address"].lower()
        or D.create_address(deployer, VAULT_NONCE).lower() != vault["address"].lower()
    ):
        raise D.DeploymentError("epoch-5 CREATE addresses drifted")
    if (
        manifest["route"]["verifier_address"].lower() != verifier["address"].lower()
        or manifest["route"]["vault_address"].lower() != vault["address"].lower()
    ):
        raise D.DeploymentError("epoch-5 route/contract address binding drifted")
    if (
        manifest["programs"]["egress"]["program_vkey"]
        != "0x0026a156bfd82ce1d1bf3f966c77daba8d5c266b8cc29928474747c4a02ca89b"
        or manifest["pftl"]["initial_finalized_height"] != 385
    ):
        raise D.DeploymentError("epoch-5 guest/checkpoint pin drifted")
    D.build_verifier_constructor_inputs(manifest)
    return artifacts


def require_epoch5_authorization(
    manifest_path: Path, manifest: dict[str, Any], authorization_path: Path | None
) -> dict[str, Any]:
    if authorization_path is None or authorization_path.resolve() != AUDIT.resolve():
        raise D.DeploymentError("epoch-5 deployment requires the pinned predeployment audit")
    rerun = subprocess.run(
        ["python3", str(AUDIT_SCRIPT)],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    if rerun.returncode != 0:
        detail = rerun.stderr.strip() or rerun.stdout.strip()
        raise D.DeploymentError(f"epoch-5 predeployment audit failed: {detail}")
    result = json.loads(AUDIT.read_text(encoding="utf-8"))
    checks = result.get("checks")
    if (
        result.get("schema") != "postfiat.pfusdc.mainnet_epoch5_predeploy_audit.v1"
        or result.get("status") != "PASS"
        or result.get("manifest_sha256") != MANIFEST_SHA256
        or not isinstance(checks, dict)
        or not checks
        or any(value != "PASS" for value in checks.values())
    ):
        raise D.DeploymentError("epoch-5 predeployment audit is not a digest-bound PASS")
    return {
        "manifest_sha256": MANIFEST_SHA256,
        "authorization": result,
        "authorization_kind": "single-agent-digest-bound-predeployment-audit",
    }


D._validate_scope = validate_scope
D.validate_offline_manifest = validate_offline_manifest
D.require_mainnet_authorization = require_epoch5_authorization


if __name__ == "__main__":
    if not any(
        argument in {"--deployment-authorization", "--auditor-authorization"}
        for argument in sys.argv[1:]
    ):
        sys.argv.extend(["--deployment-authorization", str(AUDIT)])
    raise SystemExit(D.main())
