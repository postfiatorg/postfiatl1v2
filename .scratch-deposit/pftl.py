#!/usr/bin/env python3
"""Minimal client for the live PFTL chain (postfiat-wan-devnet-2, genesis ce22ca8c...).

Submits ORDINARY transactions through the validators' own RPC finality path. It
never resets, re-genesises or reconfigures the chain, never touches a validator
process, unit, topology, consensus, membership or governance, and never contacts
Ethereum.

Secret hygiene: key files are passed to the signer by PATH only. Nothing in this
module prints a key, a mnemonic, a token or a broker account number.
"""

from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path

ROOT = Path("/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit")
NODE = ROOT / "bin/postfiat-node"          # byte-identical to the live release
EVIDENCE = ROOT / "evidence"

CHAIN_ID = "postfiat-wan-devnet-2"
GENESIS = ("ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
           "21aff3ed334da07e150a7233a3e90a9")

PEERS = [
    ("validator-0", "64.176.220.75", 27650),
    ("validator-1", "95.179.184.122", 27651),
    ("validator-2", "66.42.48.39", 27652),
    ("validator-3", "149.28.63.106", 27653),
    ("validator-4", "95.179.179.206", 27654),
    ("validator-5", "45.32.110.170", 27655),
]


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def ssh_rpc(host: str, port: int, method: str, params: dict, request_id: str,
            timeout: int = 90) -> dict:
    """One line-protocol RPC call to a validator's loopback-bound RPC port."""
    request = {"version": "postfiat-local-rpc-v1", "id": request_id,
               "method": method, "params": params}
    command = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", f"root@{host}",
               f"timeout {timeout} nc -w {timeout} 127.0.0.1 {port} | head -n 1"]
    completed = subprocess.run(
        command, input=json.dumps(request, separators=(",", ":")) + "\n",
        text=True, capture_output=True, timeout=timeout + 20, check=False)
    if completed.returncode != 0:
        raise RuntimeError(f"RPC transport failed {host}:{port} {method}: "
                           f"{completed.stderr.strip()[:400]}")
    lines = completed.stdout.splitlines()
    if not lines or not lines[0]:
        raise RuntimeError(f"RPC returned no response {host}:{port} {method}")
    return json.loads(lines[0])


def rpc(method: str, params: dict, label: str, peer=PEERS[0], timeout: int = 90) -> dict:
    payload = ssh_rpc(peer[1], peer[2], method, params, f"{label}-{time.time_ns()}", timeout)
    if payload.get("ok") is not True:
        raise RuntimeError(f"{label}: {payload.get('error') or payload}")
    return payload["result"]


def rpc_raw(method: str, params: dict, label: str, peer=PEERS[0], timeout: int = 90) -> dict:
    """Like rpc() but returns the envelope so a refusal can be read, not raised."""
    return ssh_rpc(peer[1], peer[2], method, params, f"{label}-{time.time_ns()}", timeout)


def fleet_snapshot(label: str) -> dict:
    rows = {}
    for name, host, port in PEERS:
        rows[name] = rpc("server_info", {}, f"{label}-{name}", (name, host, port), 60)
    return rows


def assert_identity(rows: dict) -> None:
    for name, row in rows.items():
        if row.get("chain_id") != CHAIN_ID or row.get("genesis_hash") != GENESIS:
            raise RuntimeError(f"fleet identity mismatch at {name}: not the live ce22 chain")


def converged_point(rows: dict):
    points = {(int(r["ledger"]["height"]), r["ledger"]["state_root"]) for r in rows.values()}
    return next(iter(points)) if len(points) == 1 else None


def wait_converged(min_height: int, timeout: int = 240) -> dict:
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        last = fleet_snapshot("converge")
        assert_identity(last)
        point = converged_point(last)
        if (point and point[0] >= min_height
                and all(int(r["mempool"]["pending"]) == 0 for r in last.values())):
            return last
        time.sleep(2)
    raise RuntimeError(f"fleet did not converge at height >= {min_height}")


def run_node(args: list[str]) -> dict:
    """Run the local copy of the live node binary. stdout is parsed, never echoed
    by this function, so a key-bearing subcommand cannot leak through it."""
    completed = subprocess.run([str(NODE), *args], text=True, capture_output=True,
                               timeout=180, check=False)
    if completed.returncode != 0:
        raise RuntimeError((completed.stderr or completed.stdout).strip()[:1500])
    return json.loads(completed.stdout)


def accepted_codes(value: object) -> list[str]:
    """Collect every receipt accept/reject code in a response tree. A REJECTED
    receipt also commits, so the literal code is what matters."""
    codes = []
    if isinstance(value, dict):
        if "accepted" in value and "code" in value:
            codes.append(str(value["code"]) if value.get("accepted") is True
                         else f"REJECTED:{value.get('code')}")
        for child in value.values():
            codes.extend(accepted_codes(child))
    elif isinstance(value, list):
        for child in value:
            codes.extend(accepted_codes(child))
    return codes


def submit_finality(method: str, params_key: str, signed: dict, label: str) -> dict:
    before = rpc("server_info", {}, f"{label}-before")
    params = {
        params_key: json.dumps(signed, separators=(",", ":")),
        "proxy_required_current_height": int(before["ledger"]["height"]),
        "proxy_required_parent_hash": before["ledger"]["hash"],
        "proxy_required_state_root": before["ledger"]["state_root"],
    }
    errors, result, proposer = {}, None, None
    for peer in PEERS:
        payload = ssh_rpc(peer[1], peer[2], method, params,
                          f"{label}-{peer[0]}-{time.time_ns()}", 150)
        if payload.get("ok") is True:
            result, proposer = payload["result"], peer[0]
            break
        errors[peer[0]] = payload.get("error") or payload
        if "proposer" not in json.dumps(errors[peer[0]]).lower():
            raise RuntimeError(f"{label} refused at {peer[0]}: "
                               f"{json.dumps(errors[peer[0]])[:800]}")
    if result is None:
        raise RuntimeError(f"{label}: no proposer accepted: {json.dumps(errors)[:800]}")
    codes = accepted_codes(result)
    if not codes or any(code != "accepted" for code in codes):
        raise RuntimeError(f"{label}: receipt is not literally accepted: {codes}")
    height = int(result["finality"]["block"]["header"]["height"])
    fleet = wait_converged(height)
    record = {"label": label, "proposer": proposer, "height": height,
              "receipt_codes": codes, "result": result, "fleet": fleet}
    write_json(EVIDENCE / label / "finality.json", record)
    return record


def asset_fee_quote(source: str, operation: dict, label: str) -> dict:
    return rpc("asset_fee_quote",
               {"source": source,
                "operation_json": json.dumps(operation, separators=(",", ":"))},
               f"{label}-quote")


def asset_fee_quote_raw(source: str, operation: dict, label: str) -> dict:
    """Read-only shape/admission probe. Returns the envelope; a refusal is data."""
    return rpc_raw("asset_fee_quote",
                   {"source": source,
                    "operation_json": json.dumps(operation, separators=(",", ":"))},
                   f"{label}-quote-probe")


def asset_op(label: str, source: str, key_file: Path, operation: dict) -> dict:
    write_json(EVIDENCE / label / "operation.json", operation)
    quote = asset_fee_quote(source, operation, label)
    write_json(EVIDENCE / label / "quote.json", quote)
    signed = run_node(["wallet-sign-asset-transaction", "--key-file", str(key_file),
                       "--quote-file", str(EVIDENCE / label / "quote.json")])
    write_json(EVIDENCE / label / "signed.json", signed)
    return submit_finality("mempool_submit_signed_asset_transaction_finality",
                           "signed_asset_transaction_json", signed, label)


def asset_op_expect_refusal(label: str, source: str, operation: dict) -> dict:
    """Probe an operation that is expected to be refused, without signing or
    submitting anything."""
    write_json(EVIDENCE / label / "operation.json", operation)
    envelope = asset_fee_quote_raw(source, operation, label)
    write_json(EVIDENCE / label / "refusal.json", envelope)
    return envelope


def transfer(label: str, source: str, key_file: Path, destination: str, amount: int) -> dict:
    account = rpc("account", {"address": source}, f"{label}-account")
    quote = rpc("transfer_fee_quote",
                {"from": source, "to": destination, "amount": amount,
                 "sequence": int(account["sequence"]) + 1}, f"{label}-quote")
    write_json(EVIDENCE / label / "quote.json", quote)
    signed = run_node(["wallet-sign-transfer", "--key-file", str(key_file),
                       "--quote-file", str(EVIDENCE / label / "quote.json")])
    write_json(EVIDENCE / label / "signed.json", signed)
    return submit_finality("mempool_submit_signed_transfer_finality",
                           "signed_transfer_json", signed, label)
