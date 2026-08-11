#!/usr/bin/env python3
"""Digest-gated Ethereum deployment driver for the distinct epoch-6 successor."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
ORCHARD = ROOT.parent / "a666-orchard-fix-2246d25"
BASE_DRIVER = ORCHARD / "scripts/pfusdc-eth-mainnet-deploy.py"
MANIFEST = (
    ROOT
    / "deployments/pfusdc-eth-mainnet-20260809-epoch6-successor/manifest.mainnet-epoch6.json"
)
EVIDENCE_DIR = (
    ROOT
    / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6-successor/deploy"
)
AUDIT = EVIDENCE_DIR.parent / "predeploy-audit.json"
AUDIT_SCRIPT = ROOT / "scripts/pfusdc-eth-mainnet-epoch6-successor-audit.py"
GAS_PREFLIGHT = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/"
    "gas-preflight/funding-reconciliation.json"
)
STAKEHUB = ROOT.parent / "StakeHub-master-e6"
MANIFEST_SHA256 = "14c7c317c78c68cfaac27ba269a56c5fccc3af222a545d5d8d9fc535ccafddb4"
VERIFIER_NONCE = 316
VAULT_NONCE = 317
VERIFIER_ADDRESS = "0xa53926f0f7453ad9f8dca592a076991ec627838c"
VAULT_ADDRESS = "0x4939a45caa85da31fb26d7dbe6477b45f7f08688"
EGRESS_VKEY = "0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87"


def load_driver():
    spec = importlib.util.spec_from_file_location(
        "pfusdc_mainnet_epoch6_successor_base_deployer", BASE_DRIVER
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load base deploy driver: {BASE_DRIVER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


D = load_driver()
D.REPOSITORY_ROOT = ROOT
D.DEFAULT_MANIFEST = MANIFEST
D.DEFAULT_EVIDENCE_DIR = EVIDENCE_DIR
D.DEFAULT_FUNDING_RECONCILIATION = GAS_PREFLIGHT
D.DEFAULT_STAKEHUB_REPO = STAKEHUB
D.REQUIRED_MANIFEST_SHA256 = MANIFEST_SHA256


def validate_scope(manifest: dict[str, Any]) -> None:
    route = manifest.get("route", {})
    if (
        manifest.get("revision"),
        manifest.get("network", {}).get("source_chain_id"),
        route.get("route_id"),
        route.get("route_epoch"),
        route.get("activation_height"),
    ) != ("mainnet-epoch6", 1, "ethereum-mainnet-usdc-v1", 6, 795):
        raise D.DeploymentError("manifest is not the pinned mainnet epoch-6 successor scope")
    if (
        manifest["network"]["token"]["address"].lower() != D.CANONICAL_USDC
        or manifest["network"]["sp1_verifier_gateway"]["address"].lower() != D.SP1_GATEWAY
    ):
        raise D.DeploymentError("manifest dependencies are not canonical mainnet USDC/SP1")


def validate_offline_manifest(
    manifest_path: Path, manifest: dict[str, Any]
) -> dict[str, dict[str, Any]]:
    if manifest_path.resolve() != MANIFEST.resolve():
        raise D.DeploymentError("manifest path is not the pinned epoch-6 successor manifest")
    if D._sha256(manifest_path) != MANIFEST_SHA256:
        raise D.DeploymentError("epoch-6 successor manifest digest drifted")
    validate_scope(manifest)
    artifacts = D._contract_artifacts(manifest)
    verifier = artifacts["PFTLFinalityVerifierV1"]
    vault = artifacts["ERC20BridgeVaultL1"]
    if (verifier["precomputed_create_nonce"], vault["precomputed_create_nonce"]) != (
        VERIFIER_NONCE,
        VAULT_NONCE,
    ):
        raise D.DeploymentError("epoch-6 successor CREATE nonces drifted")
    deployer = manifest["deployer"]["address"]
    if (
        D.create_address(deployer, VERIFIER_NONCE).lower() != VERIFIER_ADDRESS
        or D.create_address(deployer, VAULT_NONCE).lower() != VAULT_ADDRESS
        or verifier["address"].lower() != VERIFIER_ADDRESS
        or vault["address"].lower() != VAULT_ADDRESS
    ):
        raise D.DeploymentError("epoch-6 successor CREATE plan drifted")
    if (
        manifest["route"]["verifier_address"].lower() != VERIFIER_ADDRESS
        or manifest["route"]["vault_address"].lower() != VAULT_ADDRESS
    ):
        raise D.DeploymentError("epoch-6 successor route/contract address binding drifted")
    if (
        manifest["programs"]["egress"]["program_vkey"] != EGRESS_VKEY
        or manifest["pftl"]["initial_finalized_height"] != 793
    ):
        raise D.DeploymentError("epoch-6 successor guest/checkpoint pin drifted")
    D.build_verifier_constructor_inputs(manifest)
    return artifacts


def require_authorization(
    manifest_path: Path,
    manifest: dict[str, Any],
    authorization_path: Path | None,
) -> dict[str, Any]:
    if authorization_path is None or authorization_path.resolve() != AUDIT.resolve():
        raise D.DeploymentError("epoch-6 successor deployment requires its pinned audit")
    rerun = subprocess.run(
        [sys.executable, str(AUDIT_SCRIPT)],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    if rerun.returncode != 0:
        detail = rerun.stderr.strip() or rerun.stdout.strip()
        raise D.DeploymentError(f"epoch-6 successor predeployment audit failed: {detail}")
    result = json.loads(AUDIT.read_text(encoding="utf-8"))
    checks = result.get("checks")
    if (
        result.get("schema")
        != "postfiat.pfusdc.mainnet_epoch6_successor_predeploy_audit.v1"
        or result.get("status") != "PASS"
        or result.get("manifest_sha256") != MANIFEST_SHA256
        or not isinstance(checks, dict)
        or not checks
        or any(value != "PASS" for value in checks.values())
    ):
        raise D.DeploymentError("epoch-6 successor predeployment audit is not a digest-bound PASS")
    return {
        "manifest_sha256": MANIFEST_SHA256,
        "authorization": result,
        "authorization_kind": "live-digest-bound-successor-predeployment-audit",
    }


D._validate_scope = validate_scope
D.validate_offline_manifest = validate_offline_manifest
D.require_mainnet_authorization = require_authorization


if __name__ == "__main__":
    if not any(
        argument in {"--deployment-authorization", "--auditor-authorization"}
        for argument in sys.argv[1:]
    ):
        sys.argv.extend(["--deployment-authorization", str(AUDIT)])
    raise SystemExit(D.main())
