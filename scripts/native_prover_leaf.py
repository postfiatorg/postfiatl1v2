#!/usr/bin/env python3
"""Fail-closed remote ingress proof leaf; no application imports."""
from __future__ import annotations
import argparse
import hashlib
import json
import shlex
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path


# Profile-pinned epoch-5 vault-claim verifier and committed ingress ELF lineage.
EXPECTED_PROGRAM_VKEY = "0x00a9f8f037da18dd1aa5a7b0f478df0c7c9fae411ee62b339baf48dc2505076e"
EXPECTED_ELF_SHA256 = "0e59a0cf7723b9028aaa4c57f9e9c0da72119a552d62a5577223ba7b2df222d3"
# Mainnet pfUSDC vault/token pins and route binding from the committed leg-1
# packet plus the Deposit event in tx 0x016f9c5f...5924.  The creation hash is
# per-vault (the same vault used by the h390 deployment descriptor).
EXPECTED_VAULT_ADDRESS = "0xaaa78fda7062efce769e95cd72fc55e507bc8183"
EXPECTED_TOKEN_ADDRESS = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
EXPECTED_ROUTE_BINDING = "caec1d48fd3112116a96ec6fcf4a1428a190957962dfb042b811a72ff0d02d93"
EXPECTED_VAULT_CREATION_BYTECODE_HASH = "c02403a4d05a2b4400d21b360e5787ad560c1fccd293c1ad937840f986fdcd38"


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for block in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def run(argv: list[str]):
    return subprocess.run(argv, check=True)


def run_output(argv: list[str]):
    return subprocess.run(argv, check=True, capture_output=True, text=True)


def _strip_0x(value: object, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"missing {field}")
    value = value[2:] if value.lower().startswith("0x") else value
    if not value or len(value) % 2 or any(ch not in "0123456789abcdefABCDEF" for ch in value):
        raise ValueError(f"invalid hex {field}")
    return value.lower()


def _bytes32(value: object, field: str) -> str:
    value = _strip_0x(value, field)
    if len(value) != 64:
        raise ValueError(f"{field} must be bytes32")
    return value


def _address(value: object, field: str) -> str:
    if not isinstance(value, str) or not value.startswith("0x") or len(value) != 42:
        raise ValueError(f"invalid {field}")
    try:
        int(value[2:], 16)
    except ValueError as exc:
        raise ValueError(f"invalid {field}") from exc
    return value.lower()


def _quantity(value: object, field: str) -> int:
    if isinstance(value, bool):
        raise ValueError(f"invalid {field}")
    if isinstance(value, int):
        result = value
    elif isinstance(value, str):
        try:
            result = int(value, 16) if value.lower().startswith("0x") else int(value)
        except ValueError as exc:
            raise ValueError(f"invalid {field}") from exc
    else:
        raise ValueError(f"invalid {field}")
    if result < 0:
        raise ValueError(f"invalid {field}")
    return result


def _build_deployment_descriptor(deposit_report: dict, route_binding: str | None = None) -> dict:
    """Build the descriptor only from typed deposit-report fields and the pinned route."""
    vault = _address(deposit_report.get("vault_address"), "vault_address")
    token = _address(deposit_report.get("usdc_address") or deposit_report.get("token_address"), "usdc_address")
    if vault != EXPECTED_VAULT_ADDRESS.lower():
        raise RuntimeError("deposit report vault_address mismatch")
    if token != EXPECTED_TOKEN_ADDRESS.lower():
        raise RuntimeError("deposit report usdc_address mismatch")
    tx = "0x" + _bytes32(deposit_report.get("deposit_tx"), "deposit_tx")
    amount = _quantity(deposit_report.get("amount_atoms"), "amount_atoms")
    if amount <= 0:
        raise ValueError("amount_atoms must be positive")
    recipient = deposit_report.get("pftl_recipient")
    if not isinstance(recipient, str) or not recipient:
        raise ValueError("missing pftl_recipient")
    nonce = _bytes32(deposit_report.get("nonce"), "nonce")
    report_route = deposit_report.get("route_binding")
    if report_route is not None and _bytes32(report_route, "route_binding") != EXPECTED_ROUTE_BINDING:
        raise RuntimeError("deposit report route_binding mismatch")
    selected_route = _bytes32(route_binding or report_route or EXPECTED_ROUTE_BINDING, "route_binding")
    if selected_route != EXPECTED_ROUTE_BINDING:
        raise RuntimeError("route_binding mismatch")
    return {
        "vault": EXPECTED_VAULT_ADDRESS,
        "deposit_tx": tx,
        "amount_atoms": amount,
        "recipient": recipient,
        "route_binding": selected_route,
        "nonce": nonce,
        "creation_bytecode_hash": "0x" + EXPECTED_VAULT_CREATION_BYTECODE_HASH,
    }


def _rpc_receipt(url: str, tx_hash: str) -> dict:
    payload = json.dumps({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getTransactionReceipt",
        "params": [tx_hash],
    }).encode()
    request = urllib.request.Request(url, data=payload, headers={"content-type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            body = json.loads(response.read().decode())
    except (OSError, urllib.error.URLError, json.JSONDecodeError) as exc:
        raise RuntimeError(f"receipt RPC failed for {url}") from exc
    if body.get("error") is not None or not isinstance(body.get("result"), dict):
        raise RuntimeError(f"receipt RPC returned no receipt for {tx_hash}")
    return body["result"]


def _decode_deposit_log(receipt: dict, descriptor: dict, deposit_report: dict) -> tuple[str, int]:
    status = _quantity(receipt.get("status"), "receipt status")
    if status != 1:
        raise RuntimeError("canonical deposit receipt is not successful")
    block_number = _quantity(receipt.get("blockNumber"), "receipt blockNumber")
    receipt_tx = "0x" + _bytes32(receipt.get("transactionHash"), "receipt transactionHash")
    if receipt_tx.lower() != descriptor["deposit_tx"].lower():
        raise RuntimeError("canonical receipt transactionHash mismatch")
    expected_depositor = _address(deposit_report.get("stakehub_wallet"), "stakehub_wallet")
    logs = receipt.get("logs")
    if not isinstance(logs, list):
        raise RuntimeError("canonical receipt has no logs")
    selected = None
    for log in logs:
        if not isinstance(log, dict):
            continue
        if str(log.get("address", "")).lower() != EXPECTED_VAULT_ADDRESS.lower():
            continue
        topics = log.get("topics")
        if isinstance(topics, list) and len(topics) >= 3 and isinstance(topics[1], str):
            selected = log
            break
    if selected is None:
        raise RuntimeError("canonical receipt lacks vault Deposit event")
    topics = selected["topics"]
    if "blockNumber" in selected and _quantity(selected.get("blockNumber"), "Deposit event blockNumber") != block_number:
        raise RuntimeError("Deposit event blockNumber mismatch")
    if "transactionHash" in selected and "0x" + _bytes32(selected.get("transactionHash"), "Deposit event transactionHash") != receipt_tx:
        raise RuntimeError("Deposit event transactionHash mismatch")
    deposit_id = "0x" + _bytes32(topics[1], "depositId")
    depositor = _address("0x" + _strip_0x(topics[2], "depositor")[-40:], "depositor")
    if depositor != expected_depositor:
        raise RuntimeError("Deposit event depositor mismatch")
    data_hex = _strip_0x(selected.get("data"), "Deposit event data")
    if len(data_hex) < 8 * 64:
        raise RuntimeError("Deposit event data is truncated")
    words = [data_hex[i:i + 64] for i in range(0, len(data_hex), 64)]
    amount = int(words[1], 16)
    nonce = words[2].lower()
    route_binding = words[3].lower()
    if amount != descriptor["amount_atoms"]:
        raise RuntimeError("Deposit event amount mismatch")
    if nonce != descriptor["nonce"]:
        raise RuntimeError("Deposit event nonce mismatch")
    if route_binding != EXPECTED_ROUTE_BINDING or route_binding != descriptor["route_binding"]:
        raise RuntimeError("Deposit event route_binding mismatch")
    if words[5][-40:].lower() != EXPECTED_VAULT_ADDRESS[2:].lower():
        raise RuntimeError("Deposit event vault mismatch")
    if words[6][-40:].lower() != EXPECTED_TOKEN_ADDRESS[2:].lower():
        raise RuntimeError("Deposit event token mismatch")
    recipient_offset = int(words[0], 16)
    if recipient_offset % 32 or recipient_offset // 32 >= len(words):
        raise RuntimeError("Deposit event recipient offset invalid")
    recipient_index = recipient_offset // 32
    recipient_len = int(words[recipient_index], 16)
    start = (recipient_index + 1) * 64
    encoded = data_hex[start:start + recipient_len * 2]
    if len(encoded) != recipient_len * 2:
        raise RuntimeError("Deposit event recipient truncated")
    try:
        recipient = bytes.fromhex(encoded).decode("ascii")
    except (ValueError, UnicodeDecodeError) as exc:
        raise RuntimeError("Deposit event recipient encoding invalid") from exc
    if recipient != descriptor["recipient"]:
        raise RuntimeError("Deposit event recipient mismatch")
    return deposit_id, block_number


def _verify_canonical_receipt(rpc_urls: list[str], descriptor: dict, deposit_report: dict) -> tuple[str, int]:
    if not rpc_urls:
        raise ValueError("at least one --source-rpc-url is required")
    observations = []
    for url in rpc_urls:
        receipt = _rpc_receipt(url, descriptor["deposit_tx"])
        observations.append(_decode_deposit_log(receipt, descriptor, deposit_report))
    if any(observation != observations[0] for observation in observations[1:]):
        raise RuntimeError("canonical receipt disagreement across RPCs")
    return observations[0]


def _assert_public_values(values: dict, descriptor: dict, deposit_id: str, deposit_report: dict) -> None:
    if not isinstance(values, dict):
        raise RuntimeError("capture public-values JSON is not an object")
    def eq_hash(name: str, expected: str):
        actual = "0x" + _bytes32(values.get(name), name)
        if actual.lower() != expected.lower():
            raise RuntimeError(f"capture public-values {name} mismatch")
    eq_hash("deposit_id", deposit_id)
    actual_tx = "0x" + _bytes32(values.get("tx_hash"), "tx_hash")
    if actual_tx.lower() != descriptor["deposit_tx"].lower():
        raise RuntimeError("capture public-values tx_hash mismatch")
    if _address(values.get("vault_address"), "vault_address") != EXPECTED_VAULT_ADDRESS.lower():
        raise RuntimeError("capture public-values vault_address mismatch")
    if _address(values.get("token_address"), "token_address") != EXPECTED_TOKEN_ADDRESS.lower():
        raise RuntimeError("capture public-values token_address mismatch")
    if _address(values.get("depositor"), "depositor") != _address(deposit_report.get("stakehub_wallet"), "stakehub_wallet"):
        raise RuntimeError("capture public-values depositor mismatch")
    if values.get("pftl_recipient") != descriptor["recipient"]:
        raise RuntimeError("capture public-values pftl_recipient mismatch")
    if _quantity(values.get("amount_atoms"), "amount_atoms") != descriptor["amount_atoms"]:
        raise RuntimeError("capture public-values amount_atoms mismatch")
    if _bytes32(values.get("nonce"), "nonce") != descriptor["nonce"]:
        raise RuntimeError("capture public-values nonce mismatch")
    if _bytes32(values.get("route_binding"), "route_binding") != descriptor["route_binding"]:
        raise RuntimeError("capture public-values route_binding mismatch")


def _validate_report_pins(report: dict) -> None:
    if report.get("program_vkey") != EXPECTED_PROGRAM_VKEY:
        raise RuntimeError("proof report program_vkey mismatch")
    if report.get("elf_sha256") != EXPECTED_ELF_SHA256:
        raise RuntimeError("proof report elf_sha256 mismatch")


def verify_existing(out: Path) -> dict:
    report_path = out / "proof-report.json"
    if not report_path.exists():
        raise RuntimeError("existing proof output lacks proof-report.json")
    report = json.loads(report_path.read_text())
    _validate_report_pins(report)
    descriptor_path = out / "deployment-descriptor.json"
    expected_descriptor_sha = report.get("deployment_descriptor_sha256")
    if expected_descriptor_sha:
        if not descriptor_path.exists() or digest(descriptor_path) != expected_descriptor_sha:
            raise RuntimeError("existing deployment descriptor failed hash verification")
    for name in ("proof-calldata.bin", "public-values.bin"):
        path = out / name
        expected = report.get(name.replace(".bin", "_sha256")) or report.get(name + "_sha256")
        if not path.exists() or not expected or digest(path) != expected:
            raise RuntimeError(f"existing artifact failed hash verification: {name}")
    return report


def prove(args: argparse.Namespace) -> dict:
    out = Path(args.artifact_dir)
    reports = sorted(out.glob("**/evm-deposit.json"))
    if not reports:
        raise ValueError("artifact-dir has no evm-deposit.json")
    try:
        deposit_report = json.loads(reports[0].read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError("EVM deposit report malformed") from exc
    deposit_tx = deposit_report.get("deposit_tx")
    if not isinstance(deposit_tx, str) or not deposit_tx:
        raise ValueError("EVM deposit report missing deposit_tx")
    if args.deposit_tx and _bytes32(args.deposit_tx, "--deposit-tx") != _bytes32(deposit_tx, "deposit_tx"):
        raise RuntimeError("--deposit-tx does not match evm-deposit.json")
    args.deposit_tx = "0x" + _bytes32(deposit_tx, "deposit_tx")
    # The route binding is deliberately obtained from the canonical Deposit
    # event below, not from an untrusted calldata sidecar.  Use the pinned
    # value to select the event, then assert the event carries the same value.
    descriptor = _build_deployment_descriptor(deposit_report, EXPECTED_ROUTE_BINDING)
    verified_deposit_id, verified_deposit_block = _verify_canonical_receipt(
        args.source_rpc_url, descriptor, deposit_report
    )
    out.mkdir(parents=True, exist_ok=True)
    descriptor_path = out / "deployment-descriptor.json"
    descriptor_path.write_text(json.dumps(descriptor, indent=2, sort_keys=True) + "\n")
    descriptor_sha = digest(descriptor_path)
    if (out / "proof-report.json").exists():
        existing = verify_existing(out)
        if existing.get("deployment_descriptor_sha256") != descriptor_sha:
            raise RuntimeError("existing proof report deployment descriptor mismatch")
        if existing.get("verified_deposit_id", "").lower() != verified_deposit_id.lower():
            raise RuntimeError("existing proof report verified_deposit_id mismatch")
        return existing
    remote = f"{args.prover_host}:{args.remote_workdir}"
    ssh = ["ssh"]
    witness = Path(args.witness) if args.witness else None
    remote_workdir = args.remote_workdir.rstrip("/")
    remote_witness = f"{remote_workdir}/witness.json"
    remote_binary = f"{remote_workdir}/tools/eth-l1-mainnet-fast-lane-p0/target/release/eth-l1-mainnet-fast-lane-p0"
    scp = ["scp"]
    if args.ssh_key:
        ssh += ["-i", args.ssh_key]
        scp += ["-i", args.ssh_key]
    remote_descriptor = f"{remote_workdir}/deployment.json"
    run(scp + [str(descriptor_path), f"{args.prover_host}:{remote_descriptor}"])
    remote_hash_result = run_output(ssh + [args.prover_host, f"sha256sum {shlex.quote(remote_descriptor)}"])
    remote_hash_output = (remote_hash_result.stdout or "").strip().split()
    if not remote_hash_output or remote_hash_output[0] != descriptor_sha:
        raise RuntimeError("remote deployment descriptor hash mismatch")
    if witness is not None:
        if not witness.exists():
            raise ValueError(f"witness missing: {witness}")
        run(scp + [str(witness), f"{args.prover_host}:{remote_witness}"])
    else:
        capture = (
            "SP1_PROVER=cuda " + shlex.quote(remote_binary)
            + " capture --deployment " + shlex.quote(f"{remote_workdir}/deployment.json")
            + " --output " + shlex.quote(remote_witness)
            + " --execution-rpc " + shlex.quote(args.execution_rpc)
            + " --beacon-rpc " + shlex.quote(args.beacon_rpc)
        )
        run(ssh + [args.prover_host, capture])
    command = (
        "SP1_PROVER=cuda " + shlex.quote(remote_binary)
        + " prove --witness " + shlex.quote(remote_witness)
        + " --output-dir " + shlex.quote(remote_workdir)
        + " --require-prover cuda"
    )
    run(ssh + [args.prover_host, command])
    run(scp + [f"{remote}/proof-calldata.bin", str(out / "proof-calldata.bin")])
    run(scp + [f"{remote}/public-values.bin", str(out / "public-values.bin")])
    run(scp + [f"{remote}/proof-report.json", str(out / "remote-proof-report.json")])
    remote_public_values = str(Path(remote_witness).with_suffix(".public-values.json"))
    run(scp + [f"{args.prover_host}:{remote_public_values}", str(out / "capture-public-values.json")])
    report = json.loads((out / "remote-proof-report.json").read_text())
    _validate_report_pins(report)
    capture_values = json.loads((out / "capture-public-values.json").read_text())
    _assert_public_values(capture_values, descriptor, verified_deposit_id, deposit_report)
    report["proof-calldata_sha256"] = digest(out / "proof-calldata.bin")
    report["public-values_sha256"] = digest(out / "public-values.bin")
    report["deposit_tx"] = args.deposit_tx
    report["deployment_descriptor_sha256"] = descriptor_sha
    report["verified_deposit_id"] = verified_deposit_id
    report["verified_deposit_block"] = verified_deposit_block
    (out / "proof-report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--deposit-tx")
    p.add_argument("--source-rpc-url", action="append", required=True)
    p.add_argument("--prover-host", required=True)
    p.add_argument("--remote-workdir", required=True)
    p.add_argument("--artifact-dir", required=True)
    p.add_argument("--witness")
    p.add_argument("--execution-rpc", required=True)
    p.add_argument("--beacon-rpc", required=True)
    p.add_argument("--ssh-key")
    args = p.parse_args(argv)
    try:
        report = prove(args)
    except (OSError, subprocess.CalledProcessError, ValueError, RuntimeError, json.JSONDecodeError) as exc:
        print(f"STOP-no-retry: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(report, sort_keys=True))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
