#!/usr/bin/env python3
"""Fail-closed live predeployment audit for the pfUSDC mainnet epoch-6 lane."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
ORCHARD = ROOT.parent / "a666-orchard-fix-2246d25"
GENERATOR = ROOT / "scripts/pfusdc-eth-mainnet-epoch6-package.py"
BASE_DRIVER = ORCHARD / "scripts/pfusdc-eth-mainnet-deploy.py"
MANIFEST = ROOT / "deployments/pfusdc-eth-mainnet-20260809-epoch6/manifest.mainnet-epoch6.json"
SUMMARY = ROOT / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6/package/package-summary.json"
OUTPUT = ROOT / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6/predeploy-audit.json"
RPC = "https://ethereum-rpc.publicnode.com"
CAST = Path("/home/postfiat/.foundry/bin/cast")
STAKEHUB = ROOT.parent / "StakeHub-master-e6"
STAKEHUB_PYTHON = ROOT.parent / "StakeHub/.venv/bin/python3"
EXPECTED_MANIFEST_SHA256 = "f1fdc96aa33c45428fffe1af871dfe684528067cd04043fb73f6719f8ccfebce"
EXPECTED_DEPLOYER = "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"
EXPECTED_NONCE = 314
EXPECTED_VERIFIER = "0xd2191bdfa9f2750bc8b9d3cb3146291dd1251734"
EXPECTED_VAULT = "0x2604fcd968c174533e6fa6ffb034c8f3798d69ea"
EXPECTED_VKEY = "0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87"
EXPECTED_ELF_SHA256 = "4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0"
EXPECTED_CHECKPOINT = "e7a9c178fd108620a5c195aee74292489898d4aaebe4dfc6ff4548393b434c6fe883b2cabeb5609ad8ce312fddc14040"


class AuditError(RuntimeError):
    """Epoch-6 audit failed."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AuditError(message)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise AuditError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def required_env(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise RuntimeError(f"required environment variable {name} is unset")
    return value


def run(*arguments: str) -> str:
    result = subprocess.run(arguments, cwd=ROOT, check=False, text=True, capture_output=True)
    if result.returncode != 0:
        raise AuditError(result.stderr.strip() or result.stdout.strip() or "command failed")
    return result.stdout.strip()


def agent_status() -> dict[str, Any]:
    program = (
        "import json; from stakehub.agentd import call; "
        "print(json.dumps(call({'op':'status'}, timeout=10.0), sort_keys=True))"
    )
    raw = subprocess.run(
        [str(STAKEHUB_PYTHON), "-c", program],
        cwd=ROOT,
        env={**dict(__import__("os").environ), "PYTHONPATH": str(STAKEHUB)},
        check=False,
        text=True,
        capture_output=True,
    )
    if raw.returncode != 0:
        raise AuditError(raw.stderr.strip() or "StakeHub status failed")
    return json.loads(raw.stdout)


def constructor_simulation(manifest: dict[str, Any]) -> dict[str, Any]:
    from web3 import Web3

    driver = load_module(BASE_DRIVER, "pfusdc_epoch6_base_driver_audit")
    web3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 30}))
    require(web3.is_connected() and web3.eth.chain_id == 1, "Ethereum mainnet RPC unavailable")
    artifacts = {item["contract"]: item for item in manifest["contracts"]["artifacts"]}

    verifier_artifact = json.loads((ROOT / artifacts["PFTLFinalityVerifierV1"]["path"]).read_text())
    config = list(driver.build_verifier_constructor_inputs(manifest))
    config[0] = Web3.to_checksum_address(config[0])
    config[10] = Web3.to_checksum_address(config[10])
    verifier_contract = web3.eth.contract(
        abi=verifier_artifact["abi"], bytecode=verifier_artifact["bytecode"]["object"]
    )
    verifier_data = verifier_contract.constructor(tuple(config)).data_in_transaction
    verifier_runtime = web3.eth.call(
        {"from": Web3.to_checksum_address(EXPECTED_DEPLOYER), "data": verifier_data}
    )
    verifier_hash = Web3.to_hex(Web3.keccak(verifier_runtime)).lower()
    require(
        verifier_hash == artifacts["PFTLFinalityVerifierV1"]["deployed_runtime_code_keccak256"],
        "verifier constructor runtime differs from manifest",
    )

    vault_artifact = json.loads((ROOT / artifacts["ERC20BridgeVaultL1"]["path"]).read_text())
    vault_contract = web3.eth.contract(
        abi=vault_artifact["abi"], bytecode=vault_artifact["bytecode"]["object"]
    )
    vault_data = vault_contract.constructor(
        Web3.to_checksum_address(manifest["network"]["token"]["address"]),
        Web3.to_checksum_address(EXPECTED_VERIFIER),
        manifest["network"]["token"]["runtime_code_hash"],
        Web3.to_checksum_address(EXPECTED_DEPLOYER),
    ).data_in_transaction
    vault_runtime = web3.eth.call(
        {"from": Web3.to_checksum_address(EXPECTED_DEPLOYER), "data": vault_data}
    )
    vault_hash = Web3.to_hex(Web3.keccak(vault_runtime)).lower()
    require(
        vault_hash == artifacts["ERC20BridgeVaultL1"]["deployed_runtime_code_keccak256"],
        "vault constructor runtime differs from manifest",
    )
    return {
        "verifier_runtime_code_hash": verifier_hash,
        "vault_runtime_code_hash": vault_hash,
        "verifier_creation_bytes": (len(bytes.fromhex(verifier_data.removeprefix("0x")))),
        "vault_creation_bytes": (len(bytes.fromhex(vault_data.removeprefix("0x")))),
        "verifier_estimated_gas": int(
            web3.eth.estimate_gas(
                {"from": Web3.to_checksum_address(EXPECTED_DEPLOYER), "data": verifier_data}
            )
        ),
        "vault_estimated_gas": int(
            web3.eth.estimate_gas(
                {"from": Web3.to_checksum_address(EXPECTED_DEPLOYER), "data": vault_data}
            )
        ),
    }


def main() -> int:
    generated = subprocess.run([sys.executable, str(GENERATOR)], cwd=ROOT, check=False, text=True, capture_output=True)
    require(generated.returncode == 0, generated.stderr.strip() or "package regeneration failed")
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    checks: dict[str, str] = {}

    require(sha256_file(MANIFEST) == EXPECTED_MANIFEST_SHA256, "manifest digest drifted")
    checks["digest_bound_regeneration"] = "PASS"
    require(
        (manifest["revision"], manifest["network"]["source_chain_id"], manifest["route"]["route_epoch"], manifest["route"]["activation_height"])
        == ("mainnet-epoch6", 1, 6, 793),
        "epoch-6 scope drifted",
    )
    checks["scope_and_schedule"] = "PASS"
    require(
        manifest["programs"]["egress"]["program_vkey"] == EXPECTED_VKEY
        and manifest["programs"]["egress"]["elf_sha256"] == EXPECTED_ELF_SHA256,
        "fresh egress guest drifted",
    )
    checks["fresh_egress_guest"] = "PASS"
    require(
        manifest["pftl"]["initial_finalized_height"] == 792
        and manifest["pftl"]["checkpoint_block_hash"] == EXPECTED_CHECKPOINT,
        "height-792 checkpoint drifted",
    )
    checks["certified_checkpoint_pin"] = "PASS"

    latest_nonce = int(run(str(CAST), "nonce", EXPECTED_DEPLOYER, "--rpc-url", RPC))
    pending_nonce = int(run(str(CAST), "rpc", "eth_getTransactionCount", EXPECTED_DEPLOYER, "pending", "--rpc-url", RPC).strip('"'), 16)
    require(latest_nonce == pending_nonce == EXPECTED_NONCE, "live deployer nonce drifted or has pending tx")
    require(
        run(str(CAST), "code", EXPECTED_VERIFIER, "--rpc-url", RPC) == "0x"
        and run(str(CAST), "code", EXPECTED_VAULT, "--rpc-url", RPC) == "0x",
        "predicted CREATE address is occupied",
    )
    checks["live_nonce_and_empty_addresses"] = "PASS"

    checkpoint = json.loads(
        run(
            "ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8",
            required_env("PFTL_VALIDATOR_SSH_TARGET"),
            "/opt/postfiat/releases/pnok-private-fix-2246d25-orchard1/postfiat-node verify-finalized-checkpoint --data-dir /var/lib/postfiat/validator-0",
        )
    )
    require(
        checkpoint.get("verified") is True
        and checkpoint.get("checkpoint_height") == 792
        and checkpoint.get("checkpoint_block_hash") == EXPECTED_CHECKPOINT
        and checkpoint.get("validator_count") == 6,
        "live PFTL checkpoint no longer matches the package",
    )
    checks["live_six_validator_checkpoint"] = "PASS"

    stakehub = agent_status()
    require(stakehub.get("ok") is True and stakehub.get("unlocked") is True, "StakeHub is not unlocked")
    require(stakehub.get("spent_today_usd", 1) == 0, "StakeHub daily spend is not clean")
    checks["stakehub_unlocked_correct_lineage"] = "PASS"

    simulation = constructor_simulation(manifest)
    checks["constructor_simulation_and_runtime_hashes"] = "PASS"
    require(summary.get("status") == "PASS", "package summary is not PASS")
    checks["package_summary"] = "PASS"

    output = {
        "schema": "postfiat.pfusdc.mainnet_epoch6_predeploy_audit.v1",
        "status": "PASS",
        "timestamp_utc": datetime.now(UTC).replace(microsecond=0).isoformat(),
        "manifest_path": str(MANIFEST.relative_to(ROOT)),
        "manifest_sha256": EXPECTED_MANIFEST_SHA256,
        "checks": checks,
        "live_deployer_nonce": latest_nonce,
        "predicted_addresses": {"verifier": EXPECTED_VERIFIER, "vault": EXPECTED_VAULT},
        "checkpoint": checkpoint,
        "constructor_simulation": simulation,
        "stakehub": {"unlocked": True, "unlocked_for_s": stakehub.get("unlocked_for_s"), "spent_today_usd": stakehub.get("spent_today_usd")},
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(output, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(output, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AuditError, KeyError, OSError, ValueError) as exc:
        raise SystemExit(f"mainnet_epoch6_audit=failed: {exc}") from exc
