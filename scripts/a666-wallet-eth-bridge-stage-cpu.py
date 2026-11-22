#!/usr/bin/env python3
"""Local CPU backend for the existing durable Ethereum ingress stage driver."""
import importlib.util
import json
import os
from pathlib import Path
import shlex
import time
import urllib.request

spec = importlib.util.spec_from_file_location("ingress_stages", Path(__file__).with_name("a666-wallet-eth-bridge-stage-serialized.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
QUALIFICATION = Path(os.environ["B23_INGRESS_CPU_QUALIFICATION"])


def archive(job_dir, job_key):
    destination = job_dir / f"stale-{time.time_ns()}"
    destination.mkdir(mode=0o700)
    for name in ("ingress", "proof-cuda"):
        source = job_dir / name
        if source.exists():
            source.rename(destination / name)


def capture(request, job_dir, job_key):
    deposit = base.read_json(job_dir / "deposit-result.json")
    anchor = base.live_beacon_anchor()
    ingress = job_dir / "ingress"
    public_file = ingress / "witness.public-values.json"
    if public_file.exists():
        base.require_witness_anchor(base.read_json(public_file), anchor)
    else:
        deployment = {"beacon_anchor": anchor, "vault": base.VAULT,
            "deposit_tx": request["deposit_tx_hash"], "amount_atoms": int(request["amount_atoms"]),
            "recipient": request["pftl_recipient"], "route_binding": base.ROUTE_BINDING[2:],
            "nonce": deposit["nonce"][2:], "creation_bytecode_hash": base.CREATION_BYTECODE_HASH}
        base.write_json(ingress / "capture-deployment.json", deployment)
        base.run([base.A100_CAPTURE, "capture", "--deployment", str(ingress / "capture-deployment.json"),
            "--output", str(ingress / "witness.json"), "--execution-rpc", base.EXECUTION_RPCS[0],
            "--beacon-rpc", base.BEACON_RPC, "--wait-seconds", "1800"], timeout=1900)
    public = base.read_json(public_file)
    base.require_witness_anchor(public, anchor)
    expected = {"route_id": base.ROUTE_ID, "source_chain_id": 1, "vault_address": base.VAULT,
        "token_address": base.TOKEN, "depositor": request["depositor"], "pftl_recipient": request["pftl_recipient"],
        "amount_atoms": int(request["amount_atoms"]), "deposit_id": request["deposit_id"][2:],
        "route_binding": base.ROUTE_BINDING[2:], "manifest_hash": base.MANIFEST_HASH}
    if any(public.get(key) != value for key, value in expected.items()):
        raise RuntimeError("captured Ethereum public values do not match the durable job")
    return public


def prove(request, job_dir, job_key):
    output = job_dir / "proof-cuda"  # Stable artifact path used by the relay.
    report_file = output / "proof-report.json"
    if not report_file.exists():
        env = os.environ.copy()
        env.update(SP1_PROVER="cpu", PFUSDC_INGRESS_ELF=base.A100_ELF)
        for key, value in {
            "RAYON_NUM_THREADS": "12", "SP1_WORKER_NUM_CORE_WORKERS": "1",
            "SP1_WORKER_NUM_RECURSION_PROVER_WORKERS": "1",
            "SP1_WORKER_NUM_RECURSION_EXECUTOR_WORKERS": "1",
            "SP1_WORKER_CORE_BUFFER_SIZE": "1", "SP1_WORKER_RECURSION_PROVER_BUFFER_SIZE": "1",
        }.items():
            env.setdefault(key, value)
        base.run([base.A100_PROVE, "prove", "--witness", str(job_dir / "ingress/witness.json"),
            "--output-dir", str(output), "--require-prover", "cpu"], env=env, timeout=86400)
    report = base.read_json(report_file)
    if (report.get("program_vkey") != base.PROGRAM_VKEY or report.get("prover_backend") != "cpu"
        or report.get("elf_sha256") != base.A100_HASHES[base.A100_ELF]
        or report.get("host_execute_skipped") is not False or report.get("proof_bytes") != 356):
        raise RuntimeError("CPU proof report differs from pinned program/backend")
    return report


def readiness():
    checks = {}
    job_fields = ("deposit_tx_hash", "deposit_id", "pftl_recipient", "depositor", "amount_atoms")
    result = base.common("readiness", dict.fromkeys(job_fields, ""))
    for field in job_fields:
        result.pop(field)
    result["observer_attestor_enabled"] = False
    result.update(ready=False, prover_authenticated=False, prover_healthy=False,
        beacon_anchor_authenticated=False, route_manifest_active=False, program_vkey_active=False,
        nav_cap_growth_enabled=False, vault_paused=True, vault_code_hash_matches=False,
        token_code_hash_matches=False, execution_rpc_sources_reachable=0, beacon_finality_current=False,
        relay_authenticated=False,
        trust_class="TRUSTLESS_FINALITY")
    try:
        route = base.live_route()
        result.update(route_manifest_active=True, program_vkey_active=True, nav_cap_growth_enabled=True)
        anchor = base.live_beacon_anchor()
        result.update(beacon_anchor_authenticated=True,
            beacon_anchor_route_profile_hash=anchor["route_profile_hash"], beacon_anchor_route_epoch=anchor["route_epoch"])
        checks["anchor"] = "authenticated on-ledger"
    except Exception as error:
        checks["anchor"] = str(error)
    try:
        for path, expected in base.A100_HASHES.items():
            if base.sha256_file(Path(path)) != expected:
                raise RuntimeError("local capture/prover/ELF hash mismatch")
        result["prover_authenticated"] = True
        qualification = base.read_json(QUALIFICATION)
        if (qualification.get("node_verifier_accepted") is not True
            or qualification.get("program_vkey") != base.PROGRAM_VKEY
            or qualification.get("elf_sha256") != base.A100_HASHES[base.A100_ELF]
            or base.sha256_file(Path(qualification["proof_file"])) != qualification["proof_sha256"]
            or base.sha256_file(Path(qualification["public_values_file"])) != qualification["public_values_sha256"]):
            raise RuntimeError("CPU Groth16 qualification does not match release")
        result["prover_healthy"] = True
        checks["prover"] = "real CPU Groth16 proof accepted by node verifier"
    except Exception as error:
        checks["prover"] = type(error).__name__ + ": CPU Groth16 qualification missing or invalid"
    try:
        # Read only. The private key is used by path; never print its contents.
        base.ssh(base.VALIDATOR2_HOST,
            "set -euo pipefail; "
            f'test "$(sha256sum {shlex.quote(base.PFTL_NODE)} | cut -d\' \' -f1)" = {base.PFTL_NODE_HASH}; '
            f'test "$(sha256sum {shlex.quote(base.PFTL_CAST)} | cut -d\' \' -f1)" = {base.PFTL_CAST_HASH}; '
            f'test "$(jq -r .address {shlex.quote(base.PFTL_ISSUER_KEY)})" = {base.ISSUER}; '
            f"test -r {shlex.quote(base.PFTL_TOPOLOGY)}; "
            f"{shlex.quote(base.PFTL_NODE)} status --data-dir /var/lib/postfiat/validator-2 >/dev/null",
            timeout=30)
        result["relay_authenticated"] = True
        checks["relay"] = "pinned node/cast, topology and issuer path verified"
    except Exception:
        checks["relay"] = "relay binary, topology or issuer path unavailable or mismatched"
    primary = None
    for endpoint in base.EXECUTION_RPCS:
        try:
            w3 = base.checked_web3(endpoint)
            primary = primary or w3
            result["execution_rpc_sources_reachable"] += 1
        except Exception:
            pass
    if primary is not None:
        result["vault_code_hash_matches"] = result["token_code_hash_matches"] = True
        try:
            result["vault_paused"] = primary.eth.contract(address=base.Web3.to_checksum_address(base.VAULT), abi=base.VAULT_READ_ABI).functions.paused().call()
        except Exception:
            pass
    checks["execution_rpc"] = str(result["execution_rpc_sources_reachable"]) + " verified mainnet sources"
    try:
        with urllib.request.urlopen(base.BEACON_RPC + "/eth/v1/beacon/headers/finalized", timeout=12) as response:
            slot = int(json.load(response)["data"]["header"]["message"]["slot"])
        result["beacon_finality_current"] = 0 <= int((time.time() - 1606824023) // 12) - slot <= 128
    except Exception:
        pass
    checks["beacon"] = "current" if result["beacon_finality_current"] else "unavailable or stale"
    result["ready"] = (result["beacon_anchor_authenticated"] and result["prover_authenticated"]
        and result["prover_healthy"] and result["execution_rpc_sources_reachable"] >= 2
        and result["vault_paused"] is False and result["beacon_finality_current"]
        and result["relay_authenticated"])
    result["checks"] = checks
    result["ok"] = True
    if not result["ready"]:
        result["code"] = "cpu_ingress_not_ready"
        result["message"] = "Bridge blocked until the governed anchor and CPU prover are qualified."
    return result


base.archive_stale_proof = archive
base.capture_once = capture
base.ensure_proof = prove
base.readiness = readiness
base.EXECUTION_RPCS = tuple(os.environ.get("B23_ETHEREUM_RPCS", "https://ethereum-rpc.publicnode.com,https://rpc.mevblocker.io,https://eth.drpc.org").split(","))
base.BEACON_RPC = os.environ.get("A666_ETHEREUM_BEACON_RPC", "https://lodestar-mainnet.chainsafe.io")

if __name__ == "__main__":
    base.main()
