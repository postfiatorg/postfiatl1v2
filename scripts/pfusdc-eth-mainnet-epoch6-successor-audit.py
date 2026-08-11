#!/usr/bin/env python3
"""Fail-closed predeployment audit for the distinct epoch-6 successor pair."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BASE_AUDIT = ROOT / "scripts/pfusdc-eth-mainnet-epoch6-audit.py"
GENERATOR = ROOT / "scripts/pfusdc-eth-mainnet-epoch6-successor-package.py"
MANIFEST = (
    ROOT
    / "deployments/pfusdc-eth-mainnet-20260809-epoch6-successor/manifest.mainnet-epoch6.json"
)
SUMMARY = (
    ROOT
    / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6-successor/package/package-summary.json"
)
OUTPUT = (
    ROOT
    / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6-successor/predeploy-audit.json"
)
EXPECTED_MANIFEST_SHA256 = "14c7c317c78c68cfaac27ba269a56c5fccc3af222a545d5d8d9fc535ccafddb4"
EXPECTED_DEPLOYER = "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"
EXPECTED_NONCE = 316
EXPECTED_VERIFIER = "0xa53926f0f7453ad9f8dca592a076991ec627838c"
EXPECTED_VAULT = "0x4939a45caa85da31fb26d7dbe6477b45f7f08688"
EXPECTED_VKEY = "0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87"
EXPECTED_ELF_SHA256 = "4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0"
EXPECTED_CHECKPOINT = (
    "ad1aaaa5e061ef8a15fcc5d8e79fed699cfa00df3c422f90d70e392d48fe54820"
    "944d24b514b2d2325973c5d5ceedee2"
)


def load_base():
    spec = importlib.util.spec_from_file_location("pfusdc_epoch6_base_audit", BASE_AUDIT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {BASE_AUDIT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


A = load_base()
A.GENERATOR = GENERATOR
A.MANIFEST = MANIFEST
A.SUMMARY = SUMMARY
A.OUTPUT = OUTPUT
A.EXPECTED_MANIFEST_SHA256 = EXPECTED_MANIFEST_SHA256
A.EXPECTED_DEPLOYER = EXPECTED_DEPLOYER
A.EXPECTED_NONCE = EXPECTED_NONCE
A.EXPECTED_VERIFIER = EXPECTED_VERIFIER
A.EXPECTED_VAULT = EXPECTED_VAULT
A.EXPECTED_CHECKPOINT = EXPECTED_CHECKPOINT


def main() -> int:
    generated = subprocess.run(
        [sys.executable, str(GENERATOR)],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    A.require(generated.returncode == 0, generated.stderr.strip() or "package regeneration failed")
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    checks: dict[str, str] = {}

    A.require(A.sha256_file(MANIFEST) == EXPECTED_MANIFEST_SHA256, "manifest digest drifted")
    checks["digest_bound_regeneration"] = "PASS"
    A.require(
        (
            manifest["revision"],
            manifest["network"]["source_chain_id"],
            manifest["route"]["route_epoch"],
            manifest["route"]["activation_height"],
        )
        == ("mainnet-epoch6", 1, 6, 795),
        "epoch-6 successor scope drifted",
    )
    checks["scope_and_schedule"] = "PASS"
    A.require(
        manifest["programs"]["egress"]["program_vkey"] == EXPECTED_VKEY
        and manifest["programs"]["egress"]["elf_sha256"] == EXPECTED_ELF_SHA256,
        "fresh egress guest drifted",
    )
    checks["fresh_egress_guest"] = "PASS"
    A.require(
        manifest["pftl"]["initial_finalized_height"] == 793
        and manifest["pftl"]["checkpoint_block_hash"] == EXPECTED_CHECKPOINT,
        "height-793 checkpoint drifted",
    )
    checks["certified_checkpoint_pin"] = "PASS"

    latest_nonce = int(A.run(str(A.CAST), "nonce", EXPECTED_DEPLOYER, "--rpc-url", A.RPC))
    pending_nonce = int(
        A.run(
            str(A.CAST),
            "rpc",
            "eth_getTransactionCount",
            EXPECTED_DEPLOYER,
            "pending",
            "--rpc-url",
            A.RPC,
        ).strip('"'),
        16,
    )
    A.require(
        latest_nonce == pending_nonce == EXPECTED_NONCE,
        "live deployer nonce drifted or has pending tx",
    )
    A.require(
        A.run(str(A.CAST), "code", EXPECTED_VERIFIER, "--rpc-url", A.RPC) == "0x"
        and A.run(str(A.CAST), "code", EXPECTED_VAULT, "--rpc-url", A.RPC) == "0x",
        "predicted CREATE address is occupied",
    )
    checks["live_nonce_and_empty_addresses"] = "PASS"

    checkpoint = json.loads(
        A.run(
            "ssh",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "root@64.176.220.75",
            "/opt/postfiat/releases/pnok-private-fix-2246d25-orchard1/postfiat-node "
            "verify-finalized-checkpoint --data-dir /var/lib/postfiat/validator-0",
        )
    )
    A.require(
        checkpoint.get("verified") is True
        and checkpoint.get("checkpoint_height") == 793
        and checkpoint.get("checkpoint_block_hash") == EXPECTED_CHECKPOINT
        and checkpoint.get("validator_count") == 6,
        "live PFTL checkpoint no longer matches the package",
    )
    checks["live_six_validator_checkpoint"] = "PASS"

    stakehub = A.agent_status()
    A.require(stakehub.get("ok") is True and stakehub.get("unlocked") is True, "StakeHub is not unlocked")
    A.require(stakehub.get("spent_today_usd", 1) == 0, "StakeHub daily spend is not clean")
    checks["stakehub_unlocked_correct_lineage"] = "PASS"

    simulation = A.constructor_simulation(manifest)
    checks["constructor_simulation_and_runtime_hashes"] = "PASS"
    A.require(summary.get("status") == "PASS", "package summary is not PASS")
    checks["package_summary"] = "PASS"

    output = {
        "schema": "postfiat.pfusdc.mainnet_epoch6_successor_predeploy_audit.v1",
        "status": "PASS",
        "timestamp_utc": datetime.now(UTC).replace(microsecond=0).isoformat(),
        "manifest_path": str(MANIFEST.relative_to(ROOT)),
        "manifest_sha256": EXPECTED_MANIFEST_SHA256,
        "checks": checks,
        "live_deployer_nonce": latest_nonce,
        "predicted_addresses": {"verifier": EXPECTED_VERIFIER, "vault": EXPECTED_VAULT},
        "checkpoint": checkpoint,
        "constructor_simulation": simulation,
        "stakehub": {
            "unlocked": True,
            "unlocked_for_s": stakehub.get("unlocked_for_s"),
            "spent_today_usd": stakehub.get("spent_today_usd"),
        },
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(output, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (A.AuditError, KeyError, OSError, RuntimeError, ValueError) as exc:
        raise SystemExit(f"mainnet_epoch6_successor_audit=failed: {exc}") from exc
