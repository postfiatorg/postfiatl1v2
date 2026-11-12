#!/usr/bin/env python3
"""Qualified remote GPU backend for the durable Ethereum ingress stages."""
import importlib.util
import json
import os
from pathlib import Path
import shlex

spec = importlib.util.spec_from_file_location("gpu_stages", Path(__file__).with_name("a666-wallet-eth-bridge-stage-serialized.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
base.EXECUTION_RPCS = tuple(os.environ.get("B23_ETHEREUM_RPCS", "https://ethereum-rpc.publicnode.com,https://rpc.mevblocker.io,https://eth.drpc.org").split(","))
base.BEACON_RPC = os.environ.get("A666_ETHEREUM_BEACON_RPC", "https://lodestar-mainnet.chainsafe.io")
full_readiness = base.readiness


def gpu_preflight():
    checks = " && ".join(
        f'test "$(sha256sum {shlex.quote(path)} | cut -d\' \' -f1)" = {digest}'
        for path, digest in base.A100_HASHES.items())
    base.ssh(base.A100_HOST,
        f"set -euo pipefail; {checks}; nvidia-smi -L | grep -q '^GPU '; "
        "test -x /root/.sp1/bin/sp1-gpu-server; command -v docker >/dev/null; "
        "test -s /root/.sp1/circuits/groth16/v6.1.0/groth16_pk.bin; "
        "test -s /root/.sp1/circuits/groth16/v6.1.0/groth16_circuit.bin",
        port=base.A100_PORT, timeout=40)
    verifier = Path(os.environ["B23_INGRESS_NODE_VERIFY_BIN"])
    if base.sha256_file(verifier) != os.environ["B23_INGRESS_NODE_VERIFY_SHA256"]:
        raise RuntimeError("node verifier hash mismatch")
    report = base.read_json(Path(os.environ["B23_INGRESS_GPU_PROOF_REPORT"]))
    if (report.get("program_vkey") != base.PROGRAM_VKEY or report.get("prover_backend") != "cuda"
        or report.get("elf_sha256") != base.A100_HASHES[base.A100_ELF]
        or report.get("proof_bytes") != 356):
        raise RuntimeError("GPU qualification report differs from pinned release")
    verified = json.loads(base.run([str(verifier), os.environ["B23_INGRESS_GPU_PROOF"],
        os.environ["B23_INGRESS_GPU_PUBLIC_VALUES"], base.PROGRAM_VKEY], timeout=30).stdout)
    if verified.get("verified") is not True or verified.get("program_vkey") != base.PROGRAM_VKEY:
        raise RuntimeError("GPU qualification proof rejected by node verifier")


def readiness():
    fields = ("deposit_tx_hash", "deposit_id", "pftl_recipient", "depositor", "amount_atoms")
    result = base.common("readiness", dict.fromkeys(fields, ""))
    for field in fields:
        result.pop(field)
    result.update(ok=True, ready=False, observer_attestor_enabled=False,
        prover_authenticated=False, prover_healthy=False, beacon_anchor_authenticated=False,
        route_manifest_active=False, program_vkey_active=False, nav_cap_growth_enabled=False,
        vault_paused=None, vault_code_hash_matches=None, token_code_hash_matches=None,
        execution_rpc_sources_reachable=None, beacon_finality_current=None,
        trust_class="TRUSTLESS_FINALITY")
    checks = {"ethereum": "not checked until the exact governed anchor is available"}
    try:
        gpu_preflight()
        result.update(prover_authenticated=True, prover_healthy=True)
        checks["prover"] = "live GPU/ELF/binary/circuit preflight and real GPU proof verified by node"
    except Exception as error:
        checks["prover"] = str(error)
    try:
        anchor = base.live_beacon_anchor()
        result.update(beacon_anchor_authenticated=True,
            beacon_anchor_route_profile_hash=anchor["route_profile_hash"],
            beacon_anchor_route_epoch=anchor["route_epoch"])
        checks["anchor"] = "authenticated on-ledger"
        if result["prover_healthy"]:
            result.update(full_readiness())
            checks["ethereum"] = "two execution sources, code hashes, unpaused vault and current beacon verified"
            checks["route_and_relay"] = "active route, Ethereum RPCs/beacon/contracts and validator relay preflight passed"
    except Exception as error:
        checks["route_and_relay"] = str(error)
    result["checks"] = checks
    if not result["ready"]:
        result.update(code="gpu_ingress_not_ready", message="Bridge blocked until the governed route and GPU preflight pass.")
    return result


base.readiness = readiness

if __name__ == "__main__":
    base.main()
