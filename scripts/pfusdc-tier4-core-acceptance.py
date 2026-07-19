#!/usr/bin/env python3
"""Assemble and fail-closed verify the live pfUSDC Tier-4 Core Gates 1-4 record."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Any


REPO = Path(__file__).resolve().parents[1]
PYTHON = "/home/postfiat/repos/StakeHub/.venv/bin/python"
MANIFEST = REPO / "deployments/pfusdc-tier4-v3-accel-short-epoch6-20260719/manifest.json"
ROUTE_PROFILE = REPO / "deployments/pfusdc-tier4-v3-accel-short-epoch6-20260719/route-profile.json"
GATE1 = REPO / "docs/evidence/pfusdc-tier4-gate1-20260718T013235Z/ACCEPTANCE.json"
GATE4_BOUNDED = REPO / "docs/evidence/pfusdc-tier4-gate4-bounded-contract-20260718/summary.json"
ANCHOR_DEPLOY_STATE = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-live-route-deployment-20260719/state.json"
VAULT_DEPLOY_STATE = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-live-route-deployment-20260719/state.json"
VERIFIER_DEPLOY_STATE = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-final-verifier-deployment-20260719/state.json"
DEPOSIT = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-ingress-live-20260719/deposit-state.json"
FINALITY = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-finality-bootstrap-20260719/bootstrap.json"
ACTIVATION = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-route-activation-20260719/route-activation-summary.json"
INGRESS_WITNESS = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-ingress-live-20260719/witness.json"
INGRESS_AUDIT = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-ingress-live-20260719/audit.json"
INGRESS_PROOF_DIR = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-ingress-live-20260719/proof"
INGRESS_SUMMARY = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-ingress-pftl-20260719/summary.json"
EGRESS_WITNESS = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-egress-live-20260719/witness.json"
EGRESS_AUDIT = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-egress-live-20260719/audit.json"
EGRESS_PROOF_DIR = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-egress-live-20260719/proof"
EGRESS_STATE = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-egress-live-20260719/withdrawal-state.json"
EGRESS_SUMMARY = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-egress-live-20260719/summary.json"
HEADLINE = REPO / "docs/evidence/pfusdc-tier4-v3-short-segment-benchmark-20260719/summary.json"
GATEWAY_PREFLIGHT = REPO / "docs/evidence/pfusdc-tier4-v3-short-segment-benchmark-20260719/gateway-preflight.json"
TARGET = Path("/home/postfiat/tmp/pfusdc-tier4-target-exit-root-final-20260718")
OUTPUT = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-core-acceptance-20260719"
FINAL_FREEZE = REPO / "docs/evidence/pfusdc-tier4-v3-epoch6-height37-verifier-freeze-20260719/summary.json"
EXPECTED_INGRESS_VKEY = "0x0033bd140207b97fb2442eb279cc2ce55714be6fbcd66beb325fe7c3786d4dfc"
EXPECTED_EGRESS_VKEY = "0x00c8d744e19bc828d1b3fb19709d36863d8c5aba14af0ca939eb85fc806f868f"
EXPECTED_INGRESS_ELF = "7c581aa42a196bd5df5a1efc2c4569663744d9b597cc1cd2253e839f9ba2f921"
EXPECTED_EGRESS_ELF = "8e2464227d7428d9928871c4a655fd73f6a87879c2e8eae6c0228a5db367f7bd"
AMOUNT = 1_000_000


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise RuntimeError(f"expected JSON object: {path}")
    return value


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def deployment_readback() -> dict[str, Any]:
    from web3 import Web3

    manifest = read_json(MANIFEST)
    freeze = read_json(FINAL_FREEZE)
    verifier_address = next(
        item["address"]
        for item in manifest["deployment_sequence"]["arbitrum"]
        if item["contract"] == "PFTLFinalityVerifierV1"
    )
    verifier_artifact = read_json(
        REPO
        / "crates/ethereum-contracts/out/PFTLFinalityVerifierV1.sol/PFTLFinalityVerifierV1.json"
    )
    arbitrum = Web3(
        Web3.HTTPProvider(
            "https://sepolia-rollup.arbitrum.io/rpc", request_kwargs={"timeout": 30}
        )
    )
    verifier = arbitrum.eth.contract(
        address=Web3.to_checksum_address(verifier_address), abi=verifier_artifact["abi"]
    )
    decoded = verifier.functions.decodePublicValues(
        (EGRESS_PROOF_DIR / "public-values.bin").read_bytes()
    ).call()
    resulting_checkpoint = Web3.to_hex(decoded[7])
    committee_root = Web3.to_hex(decoded[8])
    finalized_height = int(decoded[10])
    completed = subprocess.run(
        [
            PYTHON,
            str(REPO / "scripts/pfusdc-tier4-deploy.py"),
            "readback",
            "--deployment-dir",
            str(MANIFEST.parent),
            "--expected-manifest-sha256",
            freeze["manifest_sha256"],
            "--expected-input-sha256",
            freeze["input_sha256"],
            "--evidence-dir",
            str(VERIFIER_DEPLOY_STATE.parent),
            "--expected-latest-checkpoint-commitment",
            resulting_checkpoint,
            "--expected-latest-finalized-height",
            str(finalized_height),
            "--expected-latest-committee-root-commitment",
            committee_root,
        ],
        cwd=REPO,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    return json.loads(completed.stdout)


def git_output(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=REPO, check=True, text=True, stdout=subprocess.PIPE
    ).stdout.strip()


def main() -> None:
    required_files = [
        MANIFEST,
        ROUTE_PROFILE,
        GATE1,
        GATE4_BOUNDED,
        ANCHOR_DEPLOY_STATE,
        VAULT_DEPLOY_STATE,
        VERIFIER_DEPLOY_STATE,
        DEPOSIT,
        FINALITY,
        ACTIVATION,
        INGRESS_WITNESS,
        INGRESS_AUDIT,
        INGRESS_PROOF_DIR / "proof-calldata.bin",
        INGRESS_PROOF_DIR / "public-values.bin",
        INGRESS_PROOF_DIR / "proof-report.json",
        INGRESS_SUMMARY,
        EGRESS_WITNESS,
        EGRESS_AUDIT,
        EGRESS_PROOF_DIR / "proof-calldata.bin",
        EGRESS_PROOF_DIR / "public-values.bin",
        EGRESS_PROOF_DIR / "proof-report.json",
        EGRESS_STATE,
        EGRESS_SUMMARY,
        HEADLINE,
        GATEWAY_PREFLIGHT,
        FINAL_FREEZE,
    ]
    for path in required_files:
        require(path.is_file(), f"required Core Gate evidence is missing: {path}")

    freeze = read_json(FINAL_FREEZE)
    expected_manifest_sha256 = freeze["manifest_sha256"]
    expected_input_sha256 = freeze["input_sha256"]
    require(sha256(MANIFEST) == expected_manifest_sha256, "frozen manifest hash mismatch")
    require(sha256(MANIFEST.parent / "input.json") == expected_input_sha256, "frozen input hash mismatch")
    manifest = read_json(MANIFEST)
    route = read_json(ROUTE_PROFILE)
    gate1 = read_json(GATE1)
    gate4_bounded = read_json(GATE4_BOUNDED)
    anchor_deployment = read_json(ANCHOR_DEPLOY_STATE)
    vault_deployment = read_json(VAULT_DEPLOY_STATE)
    verifier_deployment = read_json(VERIFIER_DEPLOY_STATE)
    deposit = read_json(DEPOSIT)
    activation = read_json(ACTIVATION)
    ingress_audit = read_json(INGRESS_AUDIT)
    ingress_proof = read_json(INGRESS_PROOF_DIR / "proof-report.json")
    ingress = read_json(INGRESS_SUMMARY)
    egress_audit = read_json(EGRESS_AUDIT)
    egress_proof = read_json(EGRESS_PROOF_DIR / "proof-report.json")
    egress_state = read_json(EGRESS_STATE)
    egress = read_json(EGRESS_SUMMARY)
    headline = read_json(HEADLINE)
    gateway_preflight = read_json(GATEWAY_PREFLIGHT)
    readback = deployment_readback()

    require(gate1.get("gate") == 1 and gate1.get("result") is True, "Core Gate 1 is not green")
    require(gate4_bounded.get("status") == "passed", "bounded Gate 4 contract evidence is not green")
    require(
        anchor_deployment.get("manifest_sha256")
        == "b621b912c776c303c044e43cf05c732210e53d3e01ab8787542322a13bd1b141",
        "anchor deployment manifest lineage differs",
    )
    require(
        vault_deployment.get("manifest_sha256")
        == "b621b912c776c303c044e43cf05c732210e53d3e01ab8787542322a13bd1b141",
        "vault deployment manifest lineage differs",
    )
    require(
        verifier_deployment.get("manifest_sha256") == expected_manifest_sha256,
        "final verifier deployment manifest differs",
    )
    require(readback.get("system_contracts_verified") is True, "system contract readback failed")
    require(readback["ethereum"]["remaining"] == [], "Ethereum deployment is incomplete")
    require(readback["arbitrum"]["remaining"] == [], "Arbitrum deployment is incomplete")
    require(readback["ethereum"]["deployed"] == ["ingress_anchor"], "Ethereum deployment readback differs")
    require(
        set(readback["arbitrum"]["deployed"]) == {"finality_verifier", "vault"},
        "Arbitrum deployment readback differs",
    )

    require(deposit.get("phase") == "complete", "live ingress deposit is incomplete")
    require(deposit.get("amount_atoms") == AMOUNT, "live ingress deposit amount differs")
    require(deposit.get("wallet_delta_atoms") == AMOUNT, "ingress wallet delta differs")
    require(deposit.get("vault_delta_atoms") == AMOUNT, "ingress vault delta differs")
    require(deposit.get("receipt_status") == 1, "ingress EVM receipt failed")
    require(deposit.get("event_exact") is True, "ingress event binding is not exact")
    require(deposit.get("replay_rejected") is True, "ingress replay was not rejected")
    require(ingress_audit.get("passed") == 21 and ingress_audit.get("failed") == 0, "ingress audit is not 21/21")
    require(activation.get("activation_height") == 35, "route was not activated at height 35")
    require(activation.get("converged") is True, "route activation did not converge")
    require(activation.get("route_readback_exact") is True, "route activation readback differs")
    require(activation.get("finality_readback_exact") is True, "finality activation readback differs")
    require(activation.get("profile_sha256") == sha256(ROUTE_PROFILE), "activated profile file hash differs")
    require(activation.get("finality_sha256") == sha256(FINALITY), "activated finality file hash differs")
    require(manifest["route_profile"]["profile"] == route, "route profile file differs from frozen manifest")
    require(route.get("route_epoch") == 6, "accelerated route epoch is not six")
    require(manifest["programs"]["ingress_program_vkey"] == EXPECTED_INGRESS_VKEY, "manifest ingress vkey differs")
    require(manifest["programs"]["egress_program_vkey"] == EXPECTED_EGRESS_VKEY, "manifest egress vkey differs")
    require(ingress_proof.get("program_vkey") == EXPECTED_INGRESS_VKEY, "ingress proof vkey differs")
    require(ingress_proof.get("proof_mode") == "groth16", "ingress proof is not Groth16")
    require(ingress.get("converged") is True, "ingress PFTL state did not converge")
    require(ingress.get("height") == 37, "ingress PFTL terminal height differs")
    require(ingress.get("source_proof_kind") == "sp1-arbitrum-finality-v1", "ingress used a non-Tier-4 proof kind")
    require(ingress.get("credited_pfusdc_atoms") == AMOUNT, "ingress credited amount differs")
    require(ingress.get("deposit_status") == "finalized", "ingress deposit is not finalized")

    require(egress_audit.get("passed") == 20 and egress_audit.get("failed") == 0, "egress audit is not 20/20")
    require(egress_proof.get("program_vkey") == EXPECTED_EGRESS_VKEY, "egress proof vkey differs")
    require(egress_proof.get("proof_mode") == "plonk", "egress proof report is not PLONK")
    require((EGRESS_PROOF_DIR / "proof-calldata.bin").read_bytes()[:4] == bytes.fromhex("5a093a2f"), "egress PLONK selector differs")
    require(egress_state.get("phase") == "verified", "egress EVM state is not verified")
    require(egress.get("converged") is True, "egress PFTL state did not converge")
    require(egress.get("pftl_height") == 40, "egress PFTL terminal height differs")
    require(egress.get("checkpoint_height") == 37, "egress checkpoint height differs")
    require(egress.get("segment_block_count") == 3, "egress segment is not exactly three blocks")
    require(egress.get("proof_mode") == "plonk", "egress proof is not PLONK")
    require(egress.get("amount_atoms") == AMOUNT, "egress amount differs")
    require(egress.get("vault_balance_before") - egress.get("vault_balance_after") == AMOUNT, "egress vault delta differs")
    require(egress.get("wallet_balance_after") - egress.get("wallet_balance_before") == AMOUNT, "egress wallet delta differs")
    require(egress.get("replay_rejected") is True, "egress replay was not rejected")
    require(egress.get("withdrawal_tx") == egress_state.get("withdrawal_tx"), "egress transaction state differs")
    require(egress.get("recipient", "").lower() == deposit.get("wallet", "").lower(), "round-trip recipient differs")
    require(egress.get("wallet_balance_before") == deposit.get("wallet_balance_after"), "round-trip wallet continuity differs")
    require(egress.get("vault_balance_before") == deposit.get("vault_balance_after"), "round-trip vault continuity differs")
    require(egress.get("wallet_balance_after") == deposit.get("wallet_balance_before"), "round-trip wallet did not conserve")
    require(egress.get("vault_balance_after") == deposit.get("vault_balance_before"), "round-trip vault did not conserve")
    require(deposit.get("vault", "").lower() == route.get("vault_address", "").lower(), "deposit vault differs from route")
    require(deposit.get("token", "").lower() == route.get("token_address", "").lower(), "deposit token differs from route")
    require(
        egress.get("proof_calldata_sha256") == sha256(EGRESS_PROOF_DIR / "proof-calldata.bin"),
        "egress proof hash differs",
    )
    require(
        egress.get("public_values_sha256") == sha256(EGRESS_PROOF_DIR / "public-values.bin"),
        "egress public-values hash differs",
    )
    require(headline.get("segment_block_count") == 3, "headline benchmark is not three blocks")
    require(headline.get("proof_mode") == "plonk", "headline benchmark is not PLONK")
    require(headline.get("program_vkey") == EXPECTED_EGRESS_VKEY, "headline benchmark vkey differs")
    require(headline.get("host_verified") is True, "headline benchmark proof was not host-verified")
    require(gateway_preflight.get("eth_call_accepted") is True, "deployed PLONK gateway preflight failed")
    require(gateway_preflight.get("proof_selector") == "0x5a093a2f", "PLONK route selector differs")

    ledger_hashes = {sha256(TARGET / f"validator-{index}/ledger.json") for index in range(6)}
    require(len(ledger_hashes) == 1, "terminal validator ledgers differ")
    require(next(iter(ledger_hashes)) == egress.get("pftl_ledger_sha256"), "terminal ledger hash differs from egress evidence")

    deployment_txs = [
        event["tx"]
        for deployment in (anchor_deployment, vault_deployment, verifier_deployment)
        for event in deployment.get("events", [])
        if event.get("phase") == "accepted" and event.get("tx")
    ]
    require(len(set(deployment_txs)) == 3, "expected exactly three deployment transactions")
    addresses = {
        item["contract"]: item["address"]
        for chain in manifest["deployment_sequence"].values()
        for item in chain
    }
    proof_identifiers = {
        "ingress_proof_calldata_sha256": sha256(INGRESS_PROOF_DIR / "proof-calldata.bin"),
        "ingress_public_values_sha256": sha256(INGRESS_PROOF_DIR / "public-values.bin"),
        "ingress_source_proof_hash": ingress["source_proof_hash"],
        "deposit_id": deposit["deposit_id"],
        "deposit_tx": deposit["deposit_tx"],
        "egress_proof_calldata_sha256": sha256(EGRESS_PROOF_DIR / "proof-calldata.bin"),
        "egress_public_values_sha256": sha256(EGRESS_PROOF_DIR / "public-values.bin"),
        "proof_nullifier": egress["proof_nullifier"],
        "withdrawal_id": egress["withdrawal_id"],
        "withdrawal_tx": egress["withdrawal_tx"],
    }
    hashes = {
        "manifest_sha256": sha256(MANIFEST),
        "route_profile_sha256": sha256(ROUTE_PROFILE),
        "finality_bootstrap_sha256": sha256(FINALITY),
        "ingress_witness_sha256": sha256(INGRESS_WITNESS),
        "ingress_audit_sha256": sha256(INGRESS_AUDIT),
        "egress_witness_sha256": sha256(EGRESS_WITNESS),
        "egress_audit_sha256": sha256(EGRESS_AUDIT),
        "terminal_ledger_sha256": next(iter(ledger_hashes)),
        **proof_identifiers,
    }
    acceptance = {
        "schema": "postfiat.pfusdc.tier4_v3_accelerated.acceptance.v1",
        "gate": 4,
        "gate_kind": "core-terminal",
        "result": True,
        "core_gates_passed": 4,
        "core_gates_total": 4,
        "launch_gates_passed": 0,
        "launch_gates_total": 3,
        "candidate_commit_sha": git_output("rev-parse", "HEAD"),
        "dirty_worktree_at_evidence_generation": bool(git_output("status", "--porcelain")),
        "artifact_hashes": {
            "ingress_elf_sha256": EXPECTED_INGRESS_ELF,
            "egress_elf_sha256": EXPECTED_EGRESS_ELF,
            "ingress_program_vkey": EXPECTED_INGRESS_VKEY,
            "egress_program_vkey": EXPECTED_EGRESS_VKEY,
            "route_profile_hash": manifest["route_profile"]["profile_hash"],
            "deployment_manifest_sha256": expected_manifest_sha256,
            "contract_runtime_hashes": manifest["contracts"]["configuration"],
        },
        "acceleration": {
            "mldsa_ntt_precompile": True,
            "checkpoint_pinned": True,
            "checkpoint_height": egress["checkpoint_height"],
            "segment_block_count": egress["segment_block_count"],
            "fresh_egress_gpu_proof_mode": egress["proof_mode"],
            "fresh_egress_gpu_setup_and_prove_ms": egress["setup_and_prove_ms"],
            "headline_gpu_setup_and_prove_ms": headline["setup_and_prove_ms"],
            "headline_end_to_end_wall_seconds": headline["end_to_end_wall_seconds"],
            "headline_proof_sha256": headline["proof_sha256"],
        },
        "chain_bindings": {
            "pftl_chain_id": manifest["pftl"]["chain_id"],
            "pftl_genesis_hash": manifest["pftl"]["genesis_hash"],
            "pftl_protocol_version": manifest["pftl"]["protocol_version"],
            "route_epoch": route["route_epoch"],
            "activation_height": activation["activation_height"],
            "ethereum_chain_id": manifest["network"]["ethereum_chain_id"],
            "arbitrum_chain_id": manifest["network"]["arbitrum_chain_id"],
            "addresses": addresses,
        },
        "proof_identifiers": proof_identifiers,
        "accepted_receipts": {
            "ingress_evm_receipt_status": deposit["receipt_status"],
            "route_activation_code": activation["activation_receipt"]["code"],
            "ingress_pftl_status": ingress["deposit_status"],
            "egress_evm_receipt_status": 1,
        },
        "balances": {
            "ingress_wallet_before": deposit["wallet_balance_before"],
            "ingress_wallet_after": deposit["wallet_balance_after"],
            "ingress_vault_before": deposit["vault_balance_before"],
            "ingress_vault_after": deposit["vault_balance_after"],
            "egress_wallet_before": egress["wallet_balance_before"],
            "egress_wallet_after": egress["wallet_balance_after"],
            "egress_vault_before": egress["vault_balance_before"],
            "egress_vault_after": egress["vault_balance_after"],
            "exact_amount_atoms": AMOUNT,
            "round_trip_wallet_net_atoms": 0,
            "round_trip_vault_net_atoms": 0,
        },
        "deployment_transactions": sorted(set(deployment_txs)),
        "bounded_negative_matrices": {"ingress": "21/21", "egress": "20/20"},
        "terminal_pftl": {
            "height": egress["pftl_height"],
            "block_hash": egress["pftl_block_hash"],
            "state_root": egress["pftl_state_root"],
            "validator_count": egress["validator_count"],
            "ledger_sha256": egress["pftl_ledger_sha256"],
        },
        "unresolved_core_findings": [],
        "controlled_testnet_launch_status": "0/3 launch gates; explicitly separate from the 4/4 core result",
    }
    OUTPUT.mkdir(parents=True, exist_ok=True)
    atomic_json(OUTPUT / "hashes.json", hashes)
    atomic_json(OUTPUT / "ACCEPTANCE.json", acceptance)
    (OUTPUT / "commands.log").write_text(
        "PASS  scripts/pfusdc-tier4-deploy.py readback\n"
        "PASS  scripts/pfusdc-tier4-core-acceptance.py internal Core Gates 1-4 audit\n"
        "No additional SP1 proof or EVM transaction was generated by terminal acceptance.\n"
    )
    print(json.dumps(acceptance, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
