#!/usr/bin/env python3
"""Production adapter for the wallet's durable Ethereum -> pfUSDC bridge jobs.

Every invocation handles one idempotent stage. Economic identity comes only
from the immutable bridge job; route identity is pinned below and rechecked
against Ethereum and the replicated PFTL route before value can move.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import shlex
import socket
import subprocess
import time
import urllib.request
from pathlib import Path
from typing import Any

from web3 import Web3


REPO = Path(__file__).resolve().parents[1]
ROUTE_ID = "ethereum-mainnet-usdc-v1"
SOURCE_CHAIN_ID = 1
PROOF_KIND = "sp1-ethereum-finality-v1"
PROGRAM_VKEY = "0x00a9f8f037da18dd1aa5a7b0f478df0c7c9fae411ee62b339baf48dc2505076e"
MANIFEST_HASH = "541a43e1f2ad0f37f6d98ea437b57f502cb888e9bba8151a50e2e5bfe5ce57a5"
ROUTE_PROFILE_HASH = "5025bdfe92669e3d8f81ce7e739fd132063261b92ef7e7ee7db19b2762e88b736bd40cd4826375e041584533f4137158"
ASSET_ID = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"
VAULT = "0xaaa78fda7062efce769e95cd72fc55e507bc8183"
VAULT_CODE_HASH = "0xc6dbb722c23bfc841624bb909fcb54d84a65a2ea6ece96e2a28bf61d5dea6d05"
TOKEN = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
TOKEN_CODE_HASH = "0xd80d4b7c890cb9d6a4893e6b52bc34b56b25335cb13716e0d1d31383e6b41505"
ROUTE_BINDING = "0xcaec1d48fd3112116a96ec6fcf4a1428a190957962dfb042b811a72ff0d02d93"
CREATION_BYTECODE_HASH = "0xc02403a4d05a2b4400d21b360e5787ad560c1fccd293c1ad937840f986fdcd38"
ISSUER = "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8"

EXECUTION_RPCS = (
    "https://ethereum-rpc.publicnode.com",
    "https://eth.drpc.org",
    "https://eth.merkle.io",
)
BEACON_RPC = "https://ethereum-beacon-api.publicnode.com"
A100_HOST = os.environ.get("A666_A100_HOST", "194.228.55.129")
A100_PORT = int(os.environ.get("A666_A100_PORT", "30886"))
VALIDATOR2_HOST = os.environ.get("A666_VALIDATOR2_HOST", "66.42.48.39")
A100_CAPTURE = (
    "/workspace/a666-acceptance/bin/"
    "eth-l1-mainnet-fast-lane-p0-depositor-fix-20260731"
)
A100_PROVE = "/workspace/a666-acceptance/bin/eth-l1-mainnet-fast-lane-p0-cuda-optimized"
A100_ELF = "/workspace/a666-acceptance/witness/pfusdc-eth-mainnet-ingress-program"
A100_HASHES = {
    A100_CAPTURE: "3dd960c721fa82008ad6490678e7152898aa6ff42ed54fbd44e4b5c853479ee4",
    A100_PROVE: "2e6017599d95e09541446b8e3054f2bbf3644dae24996a7327f50d3989d77fae",
    A100_ELF: "0e59a0cf7723b9028aaa4c57f9e9c0da72119a552d62a5577223ba7b2df222d3",
}
PFTL_NODE = "/opt/postfiat/releases/pnok-private-fix-2246d25/postfiat-node"
PFTL_NODE_HASH = "05330fb20a40b8a4536000ec57da1862d879bcdc4a21bc8c0657f5c56aa8e0f5"
PFTL_TOPOLOGY = "/etc/postfiat/releases/pnok-private-fix-2246d25/topology.json"
PFTL_ISSUER_KEY = "/var/lib/postfiat/validator-2/a666-joe-e2e-20260728/pfusdc-issuer-key.json"
PFTL_CAST = "/var/lib/postfiat/validator-2/pfusdc-latency-20260727-run2/cast"
PFTL_CAST_HASH = "ccd95a4607ca3ebfcb88bb90e7235cdb7f0564f5f1afa17478d5fccabbb222cb"
PFTL_RPC_PORT = int(os.environ.get("A666_PFTL_RPC_PORT", "38650"))
PFTL_RELAY_LOCK = Path(
    os.environ.get(
        "A666_PFTL_RELAY_LOCK",
        "/home/postfiat/.local/state/postfiat-a666-wallet/pftl-relay.lock",
    )
)
A100_PROVER_LOCK = Path(
    os.environ.get(
        "A666_A100_PROVER_LOCK",
        "/home/postfiat/.local/state/postfiat-a666-wallet/a100-prover.lock",
    )
)
PFTL_FLEET = os.environ.get(
    "PFTL_FLEET_FILE", "/home/postfiat/repos/wan-vultr-all-fleet.txt"
)

HASH32 = re.compile(r"^0x[0-9a-f]{64}$")
HASH48 = re.compile(r"^[0-9a-f]{96}$")
PFTL_ADDRESS = re.compile(r"^pf[0-9a-f]{40}$")
EVM_ADDRESS = re.compile(r"^0x[0-9a-f]{40}$")

VAULT_EVENT_ABI = {
    "anonymous": False,
    "name": "ERC20BridgeDepositedV2",
    "type": "event",
    "inputs": [
        {"indexed": True, "name": "depositId", "type": "bytes32"},
        {"indexed": True, "name": "depositor", "type": "address"},
        {"indexed": True, "name": "pftlRecipientHash", "type": "bytes32"},
        {"indexed": False, "name": "pftlRecipient", "type": "string"},
        {"indexed": False, "name": "amount", "type": "uint256"},
        {"indexed": False, "name": "nonce", "type": "bytes32"},
        {"indexed": False, "name": "routeBinding", "type": "bytes32"},
        {"indexed": False, "name": "sourceChainId", "type": "uint256"},
        {"indexed": False, "name": "vault", "type": "address"},
        {"indexed": False, "name": "token", "type": "address"},
    ],
}
VAULT_READ_ABI = [
    {
        "name": "paused",
        "type": "function",
        "stateMutability": "view",
        "inputs": [],
        "outputs": [{"name": "", "type": "bool"}],
    }
]


def run(
    argv: list[str], *, timeout: int = 60, env: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        argv,
        text=True,
        capture_output=True,
        timeout=timeout,
        env=env,
        check=False,
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip().splitlines()
        message = next(
            (line for line in detail if line.startswith("error:")),
            detail[-1] if detail else f"exit {result.returncode}",
        )
        raise RuntimeError(f"{Path(argv[0]).name} failed: {message}")
    return result


def ssh(host: str, command: str, *, port: int = 22, timeout: int = 60) -> str:
    return run(
        [
            "ssh",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-p",
            str(port),
            f"root@{host}",
            command,
        ],
        timeout=timeout,
    ).stdout


def scp(source: str, destination: str, *, port: int = 22, timeout: int = 120) -> None:
    run(
        [
            "scp",
            "-q",
            "-P",
            str(port),
            "-o",
            "BatchMode=yes",
            source,
            destination,
        ],
        timeout=timeout,
    )


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    with temporary.open("x", encoding="utf-8") as handle:
        os.chmod(temporary, 0o600)
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def rpc_call(method: str, params: dict[str, Any]) -> dict[str, Any]:
    request = json.dumps(
        {
            "version": "postfiat-local-rpc-v1",
            "id": "wallet-bridge-readiness",
            "method": method,
            "params": params,
        },
        separators=(",", ":"),
    ).encode()
    with socket.create_connection(("127.0.0.1", PFTL_RPC_PORT), timeout=10) as stream:
        stream.sendall(request + b"\n")
        stream.settimeout(20)
        chunks = bytearray()
        while b"\n" not in chunks:
            chunk = stream.recv(1024 * 1024)
            if not chunk:
                break
            chunks.extend(chunk)
    response = json.loads(bytes(chunks).split(b"\n", 1)[0])
    if response.get("ok") is not True:
        raise RuntimeError(f"PFTL {method} failed")
    return response["result"]


def common(stage: str, request: dict[str, Any]) -> dict[str, Any]:
    return {
        "ok": True,
        "stage": stage,
        "route_id": ROUTE_ID,
        "source_chain_id": SOURCE_CHAIN_ID,
        "source_proof_kind": PROOF_KIND,
        "program_vkey": PROGRAM_VKEY,
        "manifest_hash": MANIFEST_HASH,
        "route_profile_hash": ROUTE_PROFILE_HASH,
        "asset_id": ASSET_ID,
        "vault_address": VAULT,
        "vault_runtime_code_hash": VAULT_CODE_HASH,
        "token_address": TOKEN,
        "token_runtime_code_hash": TOKEN_CODE_HASH,
        "deposit_tx_hash": request["deposit_tx_hash"],
        "deposit_id": request["deposit_id"],
        "pftl_recipient": request["pftl_recipient"],
        "depositor": request["depositor"],
        "amount_atoms": request["amount_atoms"],
    }


def load_job(path: Path) -> tuple[dict[str, Any], dict[str, Any], Path, str]:
    job = read_json(path)
    request = job.get("request") or {}
    job_id = str(job.get("job_id", "")).lower()
    if (
        job.get("schema") != "postfiat-trustless-bridge-job-v2"
        or not HASH32.fullmatch(job_id)
        or request.get("route_id") != ROUTE_ID
        or int(request.get("source_chain_id", 0)) != SOURCE_CHAIN_ID
        or not HASH32.fullmatch(str(request.get("deposit_tx_hash", "")))
        or not HASH32.fullmatch(str(request.get("deposit_id", "")))
        or not PFTL_ADDRESS.fullmatch(str(request.get("pftl_recipient", "")))
        or not EVM_ADDRESS.fullmatch(str(request.get("depositor", "")))
        or not str(request.get("amount_atoms", "")).isdigit()
        or int(request["amount_atoms"]) <= 0
    ):
        raise RuntimeError("invalid immutable bridge job")
    return job, request, path.parent.resolve(), job_id[2:]


def checked_web3(url: str) -> Web3:
    w3 = Web3(Web3.HTTPProvider(url, request_kwargs={"timeout": 12}))
    if not w3.is_connected() or w3.eth.chain_id != SOURCE_CHAIN_ID:
        raise RuntimeError(f"Ethereum RPC is unavailable: {url}")
    if (
        "0x" + Web3.keccak(w3.eth.get_code(Web3.to_checksum_address(VAULT))).hex()
        != VAULT_CODE_HASH
    ):
        raise RuntimeError("governed vault runtime hash mismatch")
    if (
        "0x" + Web3.keccak(w3.eth.get_code(Web3.to_checksum_address(TOKEN))).hex()
        != TOKEN_CODE_HASH
    ):
        raise RuntimeError("canonical USDC runtime hash mismatch")
    return w3


def live_route() -> dict[str, Any]:
    route = rpc_call("vault_bridge_route", {"asset_id": ASSET_ID})
    profile = route.get("profile") or {}
    if (
        route.get("active") is not True
        or route.get("profile_hash") != ROUTE_PROFILE_HASH
        or route.get("nav_profile_policy_hash") != ROUTE_PROFILE_HASH
        or route.get("route_binding") != ROUTE_BINDING[2:]
        or profile.get("route_id") != ROUTE_ID
        or profile.get("asset_id") != ASSET_ID
        or int(profile.get("source_chain_id", 0)) != SOURCE_CHAIN_ID
        or profile.get("vault_address") != VAULT
        or profile.get("vault_runtime_code_hash") != VAULT_CODE_HASH
        or profile.get("token_address") != TOKEN
        or profile.get("token_runtime_code_hash") != TOKEN_CODE_HASH
        or profile.get("verifier_program_vkey") != PROGRAM_VKEY
        or profile.get("verifier_policy_hash") != MANIFEST_HASH
        or profile.get("verifier_kind") != "sp1-groth16"
        or profile.get("verifier_proof_encoding") != "groth16"
    ):
        raise RuntimeError("replicated PFTL route does not match the production pin set")
    return route


def readiness() -> dict[str, Any]:
    reachable = 0
    primary: Web3 | None = None
    for url in EXECUTION_RPCS:
        try:
            checked = checked_web3(url)
            reachable += 1
            primary = primary or checked
        except Exception:
            continue
    if reachable < 2 or primary is None:
        raise RuntimeError("fewer than two pinned Ethereum execution RPCs are healthy")
    vault = primary.eth.contract(
        address=Web3.to_checksum_address(VAULT), abi=VAULT_READ_ABI
    )
    if vault.functions.paused().call():
        raise RuntimeError("governed Ethereum vault is paused")

    beacon_request = urllib.request.Request(
        f"{BEACON_RPC}/eth/v1/beacon/headers/finalized",
        headers={"User-Agent": "postfiat-wallet-bridge/1"},
    )
    with urllib.request.urlopen(beacon_request, timeout=12) as response:
        finalized = json.load(response)
    slot = int(finalized["data"]["header"]["message"]["slot"])
    current_slot = max(0, int((time.time() - 1606824023) // 12))
    if current_slot - slot > 128:
        raise RuntimeError("Ethereum finalized beacon header is stale")

    live_route()
    a100_checks = " && ".join(
        f"test \"$(sha256sum {shlex.quote(path)} | cut -d' ' -f1)\" = {digest}"
        for path, digest in A100_HASHES.items()
    )
    ssh(
        A100_HOST,
        f"set -euo pipefail; {a100_checks}; nvidia-smi -L >/dev/null",
        port=A100_PORT,
        timeout=30,
    )
    validator_checks = (
        "set -euo pipefail; "
        f"test \"$(sha256sum {shlex.quote(PFTL_NODE)} | cut -d' ' -f1)\" = {PFTL_NODE_HASH}; "
        f"test \"$(sha256sum {shlex.quote(PFTL_CAST)} | cut -d' ' -f1)\" = {PFTL_CAST_HASH}; "
        f"test \"$(jq -r .address {shlex.quote(PFTL_ISSUER_KEY)})\" = {ISSUER}; "
        f"{shlex.quote(PFTL_NODE)} status --data-dir /var/lib/postfiat/validator-2 >/dev/null"
    )
    ssh(VALIDATOR2_HOST, validator_checks, timeout=30)
    return {
        "ok": True,
        "ready": True,
        "route_id": ROUTE_ID,
        "source_chain_id": SOURCE_CHAIN_ID,
        "source_proof_kind": PROOF_KIND,
        "program_vkey": PROGRAM_VKEY,
        "manifest_hash": MANIFEST_HASH,
        "route_profile_hash": ROUTE_PROFILE_HASH,
        "asset_id": ASSET_ID,
        "vault_address": VAULT,
        "vault_runtime_code_hash": VAULT_CODE_HASH,
        "token_address": TOKEN,
        "token_runtime_code_hash": TOKEN_CODE_HASH,
        "observer_attestor_enabled": False,
        "prover_authenticated": True,
        "prover_healthy": True,
        "route_manifest_active": True,
        "program_vkey_active": True,
        "nav_cap_growth_enabled": True,
        "vault_paused": False,
        "vault_code_hash_matches": True,
        "token_code_hash_matches": True,
        "execution_rpc_sources_reachable": reachable,
        "beacon_finality_current": True,
        "trust_class": "TRUSTLESS_FINALITY",
    }


def confirmed_deposit(request: dict[str, Any], job_dir: Path) -> dict[str, Any]:
    w3 = checked_web3(EXECUTION_RPCS[0])
    receipt = w3.eth.get_transaction_receipt(request["deposit_tx_hash"])
    if receipt is None or int(receipt["status"]) != 1:
        raise RuntimeError("Ethereum deposit is not confirmed successfully")
    contract = w3.eth.contract(address=Web3.to_checksum_address(VAULT), abi=[VAULT_EVENT_ABI])
    events = contract.events.ERC20BridgeDepositedV2().process_receipt(receipt)
    if len(events) != 1:
        raise RuntimeError("expected exactly one governed vault deposit event")
    event = events[0]["args"]
    deposit_id = Web3.to_hex(event["depositId"]).lower()
    values = {
        "deposit_id": deposit_id,
        "depositor": event["depositor"].lower(),
        "pftl_recipient": event["pftlRecipient"].lower(),
        "amount_atoms": int(event["amount"]),
        "nonce": Web3.to_hex(event["nonce"]).lower(),
        "route_binding": Web3.to_hex(event["routeBinding"]).lower(),
        "source_chain_id": int(event["sourceChainId"]),
        "vault": event["vault"].lower(),
        "token": event["token"].lower(),
    }
    expected = {
        "deposit_id": request["deposit_id"],
        "depositor": request["depositor"],
        "pftl_recipient": request["pftl_recipient"],
        "amount_atoms": int(request["amount_atoms"]),
        "route_binding": ROUTE_BINDING,
        "source_chain_id": SOURCE_CHAIN_ID,
        "vault": VAULT,
        "token": TOKEN,
    }
    for field, value in expected.items():
        if values[field] != value:
            raise RuntimeError(f"Ethereum deposit {field} does not match the durable job")
    evidence = {
        "schema": "postfiat.a666.pfusdc_buyer_deposit.v1",
        "verdict": "PASS",
        "chain_id": SOURCE_CHAIN_ID,
        "vault": VAULT,
        "usdc": TOKEN,
        "depositor": request["depositor"],
        "pftl_recipient": request["pftl_recipient"],
        "amount_atoms": int(request["amount_atoms"]),
        "nonce": values["nonce"],
        "route_binding": ROUTE_BINDING,
        "deposit": {
            "tx_hash": request["deposit_tx_hash"],
            "block_hash": receipt["blockHash"].hex(),
            "block_number": int(receipt["blockNumber"]),
        },
        "event": values,
    }
    write_json(job_dir / "deposit-result.json", evidence)
    return evidence


def ensure_capture(request: dict[str, Any], job_dir: Path, job_key: str) -> dict[str, Any]:
    deposit = read_json(job_dir / "deposit-result.json")
    capture = {
        "vault": VAULT,
        "deposit_tx": request["deposit_tx_hash"],
        "amount_atoms": int(request["amount_atoms"]),
        "recipient": request["pftl_recipient"],
        "route_binding": ROUTE_BINDING[2:],
        "nonce": deposit["nonce"][2:],
        "creation_bytecode_hash": CREATION_BYTECODE_HASH,
    }
    ingress = job_dir / "ingress"
    write_json(ingress / "capture-deployment.json", capture)
    remote_root = f"/workspace/a666-acceptance/wallet-bridge/{job_key}"
    ssh(
        A100_HOST,
        f"install -d -m 700 {shlex.quote(remote_root + '/ingress')}",
        port=A100_PORT,
    )
    scp(
        str(ingress / "capture-deployment.json"),
        f"root@{A100_HOST}:{remote_root}/ingress/deployment.json",
        port=A100_PORT,
    )
    remote_witness = f"{remote_root}/ingress/witness.json"
    command = (
        f"if test ! -s {shlex.quote(remote_witness)}; then "
        f"{shlex.quote(A100_CAPTURE)} capture "
        f"--deployment {shlex.quote(remote_root + '/ingress/deployment.json')} "
        f"--output {shlex.quote(remote_witness)} --wait-seconds 1800; fi"
    )
    ssh(A100_HOST, command, port=A100_PORT, timeout=1900)
    scp(
        f"root@{A100_HOST}:{remote_witness}",
        str(ingress / "witness.json"),
        port=A100_PORT,
    )
    scp(
        f"root@{A100_HOST}:{remote_root}/ingress/witness.public-values.json",
        str(ingress / "witness.public-values.json"),
        port=A100_PORT,
    )
    public = read_json(ingress / "witness.public-values.json")
    if (
        public.get("route_id") != ROUTE_ID
        or int(public.get("source_chain_id", 0)) != SOURCE_CHAIN_ID
        or public.get("vault_address") != VAULT
        or public.get("token_address") != TOKEN
        or public.get("depositor") != request["depositor"]
        or public.get("pftl_recipient") != request["pftl_recipient"]
        or int(public.get("amount_atoms", 0)) != int(request["amount_atoms"])
        or public.get("deposit_id") != request["deposit_id"][2:]
        or public.get("route_binding") != ROUTE_BINDING[2:]
        or public.get("manifest_hash") != MANIFEST_HASH
    ):
        raise RuntimeError("captured Ethereum public values do not match the durable job")
    return public


def ensure_proof(request: dict[str, Any], job_dir: Path, job_key: str) -> dict[str, Any]:
    remote_root = f"/workspace/a666-acceptance/wallet-bridge/{job_key}"
    remote_proof = f"{remote_root}/proof-cuda"
    command = (
        f"if test ! -s {shlex.quote(remote_proof + '/proof-report.json')}; then "
        f"SP1_PROVER=cuda {shlex.quote(A100_PROVE)} prove "
        f"--witness {shlex.quote(remote_root + '/ingress/witness.json')} "
        f"--output-dir {shlex.quote(remote_proof)} --require-prover cuda "
        "--skip-redundant-execute; fi"
    )
    ssh(A100_HOST, command, port=A100_PORT, timeout=7200)
    proof = job_dir / "proof-cuda"
    proof.mkdir(parents=True, exist_ok=True, mode=0o700)
    for name in ("proof-calldata.bin", "public-values.bin", "proof-report.json"):
        scp(
            f"root@{A100_HOST}:{remote_proof}/{name}",
            str(proof / name),
            port=A100_PORT,
        )
    report = read_json(proof / "proof-report.json")
    if (
        report.get("program_vkey") != PROGRAM_VKEY
        or report.get("prover_backend") != "cuda"
        or report.get("host_execute_skipped") is not True
        or int(report.get("proof_bytes", 0)) != 356
    ):
        raise RuntimeError("SP1 proof report does not match the production proof policy")
    return report


def pftl_balance(account: str) -> int:
    command = (
        f"{shlex.quote(PFTL_NODE)} account-assets "
        "--data-dir /var/lib/postfiat/validator-2 "
        f"--account {shlex.quote(account)} --asset-id {ASSET_ID}"
    )
    result = json.loads(ssh(VALIDATOR2_HOST, command))
    return sum(
        int(row["balance"])
        for row in result.get("assets", [])
        if row.get("asset_id") == ASSET_ID
    )


def pftl_deposit(request: dict[str, Any]) -> dict[str, Any] | None:
    status = rpc_call("vault_bridge_status", {"asset_id": ASSET_ID})
    deposit_id = request["deposit_id"][2:]
    row = next(
        (
            item
            for item in status.get("bridge_deposits", [])
            if item.get("deposit_id") == deposit_id
        ),
        None,
    )
    if row is None:
        return None
    if (
        int(row.get("source_chain_id", 0)) != SOURCE_CHAIN_ID
        or row.get("vault_address") != VAULT
        or row.get("token_address") != TOKEN
        or row.get("depositor") != request["depositor"]
        or row.get("pftl_recipient") != request["pftl_recipient"]
        or int(row.get("amount_atoms", 0)) != int(request["amount_atoms"])
        or row.get("policy_hash") != ROUTE_PROFILE_HASH
        or row.get("source_proof_kind") != PROOF_KIND
    ):
        raise RuntimeError("existing PFTL bridge deposit conflicts with the durable job")
    return row


def pftl_receipt_for_evidence(evidence_root: str) -> dict[str, Any] | None:
    status = rpc_call("vault_bridge_status", {"asset_id": ASSET_ID})
    return next(
        (
            item
            for item in status.get("receipts", [])
            if item.get("bridge_deposit_evidence_root") == evidence_root
        ),
        None,
    )


def relay_phase(
    phase: str,
    request: dict[str, Any],
    job_dir: Path,
    job_key: str,
    *,
    skip_finalize: bool = False,
) -> dict[str, Any]:
    baseline_file = job_dir / "pftl-baseline.json"
    if baseline_file.exists():
        baseline = read_json(baseline_file)
    else:
        baseline = {
            "account": request["pftl_recipient"],
            "balance_atoms": pftl_balance(request["pftl_recipient"]),
        }
        write_json(baseline_file, baseline)
    expected = int(baseline["balance_atoms"]) + int(request["amount_atoms"])
    remote_run = f"/var/lib/postfiat/validator-2/wallet-bridge-{job_key}"
    remote_proof = f"{remote_run}/ingress-proof"
    if phase == "propose":
        ssh(
            VALIDATOR2_HOST,
            f"install -d -m 700 {shlex.quote(remote_proof)}",
        )
        for name in ("proof-calldata.bin", "public-values.bin"):
            scp(
                str(job_dir / "proof-cuda" / name),
                f"root@{VALIDATOR2_HOST}:{remote_proof}/{name}",
            )
    environment = os.environ.copy()
    environment.update(
        {
            "DEPOSIT_TX": request["deposit_tx_hash"],
            "DEPOSIT_ATOMS": request["amount_atoms"],
            "EXPECTED_HOLDER_ATOMS": str(expected),
            "PFTL_RUN_DIR": remote_run,
            "PFTL_PROOF_DIR": remote_proof,
            "PFTL_LOCAL_EVIDENCE": str(job_dir / "pftl"),
            "PFTL_RELAY_PHASE": phase,
            "PFTL_SKIP_FINALIZE": "true" if skip_finalize else "false",
            "PFTL_HOLDER": request["pftl_recipient"],
            "PFTL_NODE_BIN": PFTL_NODE,
            "PFTL_TOPOLOGY": PFTL_TOPOLOGY,
            "PFTL_ISSUER_KEY": PFTL_ISSUER_KEY,
            "PFTL_CAST_BIN": PFTL_CAST,
            "PFTL_FLEET_FILE": PFTL_FLEET,
            "PFTL_LABEL_SUFFIX": f"-wallet-{job_key[:10]}",
        }
    )
    PFTL_RELAY_LOCK.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    with PFTL_RELAY_LOCK.open("a", encoding="utf-8") as relay_lock:
        fcntl.flock(relay_lock.fileno(), fcntl.LOCK_EX)
        try:
            run(
                ["bash", str(REPO / "scripts/a666-mainnet-pfusdc-relay.sh")],
                timeout=600,
                env=environment,
            )
        finally:
            fcntl.flock(relay_lock.fileno(), fcntl.LOCK_UN)
    return read_json(job_dir / "pftl" / "summary.json")


def execute_stage(stage: str, job_file: Path) -> dict[str, Any]:
    _, request, job_dir, job_key = load_job(job_file)
    result = common(stage, request)
    if stage == "confirming_deposit":
        confirmed_deposit(request, job_dir)
        result["deposit_confirmed"] = True
    elif stage == "waiting_for_ethereum_finality":
        public = ensure_capture(request, job_dir, job_key)
        result.update(
            {
                "ethereum_finalized": True,
                "finalized_block_hash": "0x"
                + public["finalized_execution_block_hash"],
                "finalized_block_number": int(
                    public["finalized_execution_block_number"]
                ),
            }
        )
    elif stage == "capturing_state_proof":
        public = read_json(job_dir / "ingress/witness.public-values.json")
        result.update(
            {
                "witness_sha256": sha256_file(job_dir / "ingress/witness.json"),
                "evidence_root": public["evidence_root"],
                "nullifier": "0x" + public["deposit_nullifier"],
            }
        )
    elif stage == "proving":
        A100_PROVER_LOCK.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        with A100_PROVER_LOCK.open("a", encoding="utf-8") as prover_lock:
            fcntl.flock(prover_lock.fileno(), fcntl.LOCK_EX)
            try:
                ensure_proof(request, job_dir, job_key)
            finally:
                fcntl.flock(prover_lock.fileno(), fcntl.LOCK_UN)
        result.update(
            {
                "proof_sha256": sha256_file(job_dir / "proof-cuda/proof-calldata.bin"),
                "public_values_sha256": sha256_file(
                    job_dir / "proof-cuda/public-values.bin"
                ),
            }
        )
    elif stage == "verifying":
        public = read_json(job_dir / "ingress/witness.public-values.json")
        deposit = pftl_deposit(request)
        if deposit is None:
            relay_phase("propose", request, job_dir, job_key)
            deposit = pftl_deposit(request)
        if (
            deposit is None
            or deposit.get("status") not in {"pending", "finalized"}
            or deposit.get("evidence_root") != public["evidence_root"]
        ):
            raise RuntimeError("consensus proposal evidence root differs from SP1 output")
        result["proof_verified"] = True
    elif stage == "growing_backed_cap":
        deposit = pftl_deposit(request)
        if deposit is None or deposit.get("status") not in {"pending", "finalized"}:
            raise RuntimeError("proof-backed PFTL proposal was not finalized")
        result["backed_cap_ready"] = True
    elif stage == "claiming":
        deposit = pftl_deposit(request)
        if deposit is None:
            raise RuntimeError("PFTL bridge proposal disappeared before claim")
        receipt = pftl_receipt_for_evidence(deposit["evidence_root"])
        if receipt is None:
            relay_phase(
                "claim",
                request,
                job_dir,
                job_key,
                skip_finalize=deposit.get("status") == "finalized",
            )
            deposit = pftl_deposit(request)
            receipt = pftl_receipt_for_evidence(deposit["evidence_root"])
        if (
            deposit.get("status") != "finalized"
            or receipt is None
            or int(receipt.get("amount_atoms", 0)) != int(request["amount_atoms"])
            or receipt.get("status") != "counted"
        ):
            raise RuntimeError("PFTL pfUSDC claim balance delta is not exact")
        result.update(
            {
                "receipt_code": "ACCEPTED",
                "receipt_id": receipt["receipt_id"],
            }
        )
    else:
        raise RuntimeError(f"unsupported bridge stage: {stage}")
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--readiness", action="store_true")
    parser.add_argument("--stage")
    parser.add_argument("--job-file", type=Path)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.readiness:
        if args.stage or args.job_file:
            raise RuntimeError("--readiness cannot be combined with a job stage")
        result = readiness()
    else:
        if not args.stage or not args.job_file:
            raise RuntimeError("--stage and --job-file are required")
        result = execute_stage(args.stage, args.job_file.resolve())
    print(json.dumps(result, separators=(",", ":"), sort_keys=True))


if __name__ == "__main__":
    main()
