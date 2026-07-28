#!/usr/bin/env python3
"""Fail-closed predeployment audit for the pfUSDC mainnet epoch-5 lane."""

from __future__ import annotations

import hashlib
import json
import subprocess
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from Crypto.Hash import keccak


ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / "scripts/pfusdc-eth-mainnet-epoch5-package.py"
MANIFEST = (
    ROOT
    / "deployments/pfusdc-eth-mainnet-20260728-epoch5/manifest.mainnet-epoch5.json"
)
SUMMARY = (
    ROOT
    / "docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/"
    "pfusdc-egress/recovery-epoch5/package/package-summary.json"
)
OUTPUT = (
    ROOT
    / "docs/evidence/a666-acceptance-20260728/phase-5-transparent-redeem-verify/"
    "pfusdc-egress/recovery-epoch5/predeploy-audit.json"
)
RPC = "https://ethereum-rpc.publicnode.com"
EXPECTED_MANIFEST_SHA256 = "66abebe1cc23b5154933d2771ffe8a9407cf16d257871e76cfddf8fb46a75559"
EXPECTED_DEPLOYER = "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"
EXPECTED_NONCE = 216
EXPECTED_VERIFIER = "0x9a45d6f1dc9da443a88b1c336b3188fa7924d1ae"
EXPECTED_VAULT = "0xaaa78fda7062efce769e95cd72fc55e507bc8183"
OLD_VAULT = "0x8583409ddbac984ec195dfa06a21103d92403c1e"


class AuditError(RuntimeError):
    """Epoch-5 audit failed."""


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*arguments: str) -> str:
    result = subprocess.run(
        list(arguments),
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise AuditError(f"{' '.join(arguments)} failed: {detail}")
    return result.stdout.strip()


def code_hash(address: str) -> str:
    code = run("cast", "code", address, "--rpc-url", RPC)
    raw = bytes.fromhex(code.removeprefix("0x"))
    digest = keccak.new(digest_bits=256)
    digest.update(raw)
    return "0x" + digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AuditError(message)


def main() -> int:
    generated = subprocess.run(
        ["python3", str(GENERATOR)],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    require(generated.returncode == 0, generated.stderr.strip() or "package regeneration failed")
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    checks: dict[str, str] = {}

    require(sha256_file(MANIFEST) == EXPECTED_MANIFEST_SHA256, "manifest digest drifted")
    checks["digest_bound_regeneration"] = "PASS"
    require(
        (
            manifest.get("revision"),
            manifest["network"]["source_chain_id"],
            manifest["route"]["route_epoch"],
            manifest["route"]["binding_submission_height"],
            manifest["route"]["registration_height"],
        )
        == ("mainnet-epoch5", 1, 5, 386, 387),
        "epoch-5 chain or schedule drifted",
    )
    checks["scope_and_schedule"] = "PASS"
    require(
        manifest["deployer"]["address"].lower() == EXPECTED_DEPLOYER,
        "deployer drifted",
    )
    artifacts = {item["contract"]: item for item in manifest["contracts"]["artifacts"]}
    require(
        artifacts["PFTLFinalityVerifierV1"]["precomputed_create_nonce"] == EXPECTED_NONCE
        and artifacts["PFTLFinalityVerifierV1"]["address"].lower() == EXPECTED_VERIFIER
        and artifacts["ERC20BridgeVaultL1"]["precomputed_create_nonce"] == EXPECTED_NONCE + 1
        and artifacts["ERC20BridgeVaultL1"]["address"].lower() == EXPECTED_VAULT,
        "CREATE plan drifted",
    )
    checks["create_plan"] = "PASS"
    require(
        manifest["programs"]["egress"]["program_vkey"]
        == "0x0026a156bfd82ce1d1bf3f966c77daba8d5c266b8cc29928474747c4a02ca89b"
        and manifest["programs"]["egress"]["elf_sha256"]
        == "ea0d3ef37ade9e2413646c8051b58f8e8123516e75da0937a8d47d4d9586f2fe",
        "current egress guest pin drifted",
    )
    checks["current_egress_guest"] = "PASS"
    require(
        manifest["pftl"]["initial_finalized_height"] == 385
        and manifest["pftl"]["initial_checkpoint_commitment"]
        == "0x253b5648f83a0e1f519f7d27957cca28fd33296fd1572d24d246afec4b52cbc0",
        "height-385 checkpoint drifted",
    )
    checks["certified_checkpoint"] = "PASS"
    require(summary.get("status") == "PASS", "package summary is not PASS")
    checks["package_summary"] = "PASS"

    live_nonce = int(run("cast", "nonce", EXPECTED_DEPLOYER, "--rpc-url", RPC))
    require(live_nonce == EXPECTED_NONCE, f"live deployer nonce is {live_nonce}")
    require(
        run("cast", "code", EXPECTED_VERIFIER, "--rpc-url", RPC) == "0x"
        and run("cast", "code", EXPECTED_VAULT, "--rpc-url", RPC) == "0x",
        "predicted deployment address is occupied",
    )
    checks["live_nonce_and_empty_addresses"] = "PASS"
    require(
        code_hash(manifest["network"]["token"]["address"])
        == manifest["network"]["token"]["runtime_code_hash"],
        "live USDC runtime hash drifted",
    )
    require(
        code_hash(manifest["network"]["sp1_verifier_gateway"]["address"])
        == manifest["network"]["sp1_verifier_gateway"]["runtime_code_hash"],
        "live SP1 gateway runtime hash drifted",
    )
    checks["live_dependency_code"] = "PASS"
    require(
        run("cast", "call", OLD_VAULT, "paused()(bool)", "--rpc-url", RPC) == "true",
        "legacy vault is not paused",
    )
    checks["legacy_lane_paused"] = "PASS"

    output: dict[str, Any] = {
        "schema": "postfiat.pfusdc.mainnet_epoch5_predeploy_audit.v1",
        "status": "PASS",
        "timestamp_utc": datetime.now(UTC).replace(microsecond=0).isoformat(),
        "manifest_path": str(MANIFEST.relative_to(ROOT)),
        "manifest_sha256": EXPECTED_MANIFEST_SHA256,
        "checks": checks,
        "live_deployer_nonce": live_nonce,
        "predicted_addresses": {
            "verifier": EXPECTED_VERIFIER,
            "vault": EXPECTED_VAULT,
        },
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(output, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AuditError, KeyError, OSError, ValueError) as exc:
        raise SystemExit(f"mainnet_epoch5_audit=failed: {exc}") from exc
