#!/usr/bin/env python3
"""Report-only bridge reserve watch.

Reads the Arbitrum USDC vault balance with one eth_call and compares it to the
local PFTL vault-bridge supply report. This script has no signing, transaction,
halt, daemon, or consensus authority.
"""

import argparse
import datetime as dt
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path


DEFAULT_RPC = "https://arb1.arbitrum.io/rpc"
DEFAULT_ASSET_ID = (
    "34ce77d07099872d5691ead3842bfb3d6cc8678ff62cc68d887dad7f8645128351"
    "e72b9ae76f88ed1854a5e8d3372c8b"
)
VAULT_ADDRESS = "0x6a700337663d7c4143e26a3a172077415d90e7d7"
TOKEN_ADDRESS = "0xaf88d065e77c8cc2239327c5edb3a432268e5831"
BALANCE_OF_SELECTOR = "70a08231"
ATOM_SCALE = 1_000_000
U64_MAX = (1 << 64) - 1
I64_MIN = -(1 << 63)
I64_MAX = (1 << 63) - 1

GATE_VAULT_ATOMS = 629_999_693
GATE_ISSUED_ATOMS = 270_019_667
GATE_COUNTED_ATOMS = 609_999_693
GATE_CIRCULATING_ATOMS = 60_019_974

NODE_BIN = Path(__file__).resolve().parents[1] / "target" / "release" / "postfiat-node"


class WatchError(Exception):
    """Expected reserve-watch failure surfaced to the artifact and CLI."""


def utc_now() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc)


def iso_utc(value: dt.datetime) -> str:
    return value.isoformat(timespec="seconds").replace("+00:00", "Z")


def stamp_utc(value: dt.datetime) -> str:
    return value.strftime("%Y%m%dT%H%M%SZ")


def usdc_float(atoms: int | None) -> float | None:
    if atoms is None:
        return None
    return round(atoms / ATOM_SCALE, 6)


def compact_error(value: object, limit: int = 1000) -> str:
    if isinstance(value, str):
        text = value
    else:
        text = json.dumps(value, sort_keys=True, separators=(",", ":"))
    text = " ".join(text.split())
    if len(text) > limit:
        return text[: limit - 3] + "..."
    return text


def parse_address(address: str, label: str) -> str:
    if not address.startswith("0x"):
        raise WatchError(f"{label} must start with 0x")
    body = address[2:]
    if len(body) != 40:
        raise WatchError(f"{label} must be 20 bytes, got {len(body) // 2} bytes")
    try:
        int(body, 16)
    except ValueError as exc:
        raise WatchError(f"{label} is not hex") from exc
    return "0x" + body.lower()


def balance_of_data(owner: str) -> str:
    owner = parse_address(owner, "vault address")
    return "0x" + BALANCE_OF_SELECTOR + owner[2:].rjust(64, "0")


def read_u64(report: dict, key: str) -> int:
    value = report.get(key)
    if not isinstance(value, int) or isinstance(value, bool):
        raise WatchError(f"VaultBridgeStatusReport.{key} is not an integer")
    if value < 0 or value > U64_MAX:
        raise WatchError(f"VaultBridgeStatusReport.{key} is outside u64 range")
    return value


def checked_i64(value: int, label: str) -> int:
    if value < I64_MIN or value > I64_MAX:
        raise WatchError(f"{label} is outside i64 range")
    return value


def eth_call_balance_atoms(rpc: str) -> tuple[int, None]:
    token = parse_address(TOKEN_ADDRESS, "token address")
    payload = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_call",
        "params": [{"to": token, "data": balance_of_data(VAULT_ADDRESS)}, "latest"],
    }
    request = urllib.request.Request(
        rpc,
        data=json.dumps(payload).encode("utf-8"),
        headers={"content-type": "application/json", "user-agent": "reserve-watch/1.0"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            response_text = response.read().decode("utf-8")
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        raise WatchError(f"rpc_http_error:{exc.code}:{compact_error(body)}") from exc
    except urllib.error.URLError as exc:
        raise WatchError(f"rpc_url_error:{compact_error(str(exc.reason))}") from exc
    except TimeoutError as exc:
        raise WatchError("rpc_timeout") from exc

    try:
        rpc_response = json.loads(response_text)
    except json.JSONDecodeError as exc:
        raise WatchError(f"rpc_invalid_json:{compact_error(response_text)}") from exc

    if not isinstance(rpc_response, dict):
        raise WatchError("rpc_response_not_object")
    if "error" in rpc_response:
        raise WatchError(f"rpc_error_key:error:{compact_error(rpc_response['error'])}")

    result = rpc_response.get("result")
    if not isinstance(result, str):
        raise WatchError("rpc_missing_result")
    if not result.startswith("0x") or len(result) <= 2:
        raise WatchError("rpc_empty_or_non_hex_result")
    try:
        balance = int(result, 16)
    except ValueError as exc:
        raise WatchError(f"rpc_non_hex_result:{compact_error(result)}") from exc
    if balance < 0 or balance > U64_MAX:
        raise WatchError("rpc_balance_outside_u64_range")
    return balance, None


def pftl_supply_report(data_dir: str, asset_id: str) -> tuple[dict, None]:
    command = [
        str(NODE_BIN),
        "vault-bridge-status",
        "--data-dir",
        data_dir,
        "--asset-id",
        asset_id,
    ]
    try:
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except FileNotFoundError as exc:
        raise WatchError(f"node_binary_not_found:{NODE_BIN}") from exc
    except subprocess.TimeoutExpired as exc:
        raise WatchError("node_timeout") from exc

    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise WatchError(f"node_exit_{completed.returncode}:{compact_error(detail)}")
    try:
        report = json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise WatchError(f"node_invalid_json:{compact_error(completed.stdout)}") from exc
    if not isinstance(report, dict):
        raise WatchError("node_report_not_object")
    return report, None


def build_notes(
    rpc_error: str | None,
    node_error: str | None,
    gate_vault_delta: int | None,
    gate_issued_delta: int | None,
    gate_counted_delta: int | None,
    gate_circulating_delta: int | None,
) -> tuple[bool, str]:
    findings: list[str] = []
    if rpc_error is not None:
        findings.append(f"FINDING: Arbitrum vault balance UNKNOWN; {rpc_error}")
    elif gate_vault_delta != 0:
        findings.append(
            "FINDING: live Arbitrum vault balance differs from gate snapshot by "
            f"{gate_vault_delta} atoms; gate snapshot is point-in-time, no action taken"
        )

    if node_error is not None:
        findings.append(f"FINDING: PFTL supply UNKNOWN; {node_error}")
    else:
        if gate_issued_delta != 0:
            findings.append(
                "FINDING: PFTL issued_supply_atoms differs from gate map by "
                f"{gate_issued_delta} atoms"
            )
        if gate_counted_delta != 0:
            findings.append(
                "FINDING: PFTL counted_value_atoms differs from gate map by "
                f"{gate_counted_delta} atoms"
            )
        if gate_circulating_delta != 0:
            findings.append(
                "FINDING: PFTL circulating_supply differs from gate map by "
                f"{gate_circulating_delta} atoms"
            )

    reconciles = len(findings) == 0
    if reconciles:
        return True, "No findings; vault and PFTL supply match gate money map atom-for-atom."
    return False, "; ".join(findings)


def write_artifact(out_dir: Path, stamp: str, artifact: dict) -> Path:
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / f"reserve-watch-{stamp}.json"
    if path.exists():
        suffix = utc_now().strftime("%f")
        path = out_dir / f"reserve-watch-{stamp}-{suffix}.json"
    tmp_path = path.with_suffix(path.suffix + ".tmp")
    with tmp_path.open("w", encoding="utf-8") as handle:
        json.dump(artifact, handle, indent=2)
        handle.write("\n")
    os.replace(tmp_path, path)
    return path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Report-only PFTL reserve watch.")
    parser.add_argument("--data-dir", required=True, help="postfiat-node data directory")
    parser.add_argument("--asset-id", default=DEFAULT_ASSET_ID, help="PFTL asset id")
    parser.add_argument("--rpc", default=DEFAULT_RPC, help="Arbitrum JSON-RPC URL")
    parser.add_argument("--out-dir", required=True, help="directory for reserve-watch JSON artifacts")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    run_at = utc_now()
    run_iso = iso_utc(run_at)
    run_stamp = stamp_utc(run_at)

    balance_atoms = None
    rpc_error = None
    try:
        balance_atoms, _ = eth_call_balance_atoms(args.rpc)
    except WatchError as exc:
        rpc_error = str(exc)

    schema = None
    issued_atoms = None
    counted_atoms = None
    circulating_atoms = None
    node_error = None
    try:
        report, _ = pftl_supply_report(args.data_dir, args.asset_id)
        schema = report.get("schema")
        if not isinstance(schema, str):
            raise WatchError("VaultBridgeStatusReport.schema is not a string")
        issued_atoms = read_u64(report, "issued_supply_atoms")
        counted_atoms = read_u64(report, "counted_value_atoms")
        circulating_atoms = read_u64(report, "circulating_supply")
    except WatchError as exc:
        node_error = str(exc)

    can_compute = (
        balance_atoms is not None
        and issued_atoms is not None
        and counted_atoms is not None
        and circulating_atoms is not None
    )

    vault_minus_issued = checked_i64(balance_atoms - issued_atoms, "vault_minus_issued_atoms") if can_compute else None
    vault_minus_counted = checked_i64(balance_atoms - counted_atoms, "vault_minus_counted_atoms") if can_compute else None
    counted_minus_issued = checked_i64(counted_atoms - issued_atoms, "counted_minus_issued_atoms") if can_compute else None
    issued_minus_circulating = (
        checked_i64(issued_atoms - circulating_atoms, "issued_minus_circulating_atoms") if can_compute else None
    )

    gate_vault_delta = (
        checked_i64(balance_atoms - GATE_VAULT_ATOMS, "current_vs_gate_vault_delta_atoms")
        if balance_atoms is not None
        else None
    )
    gate_issued_delta = (
        checked_i64(issued_atoms - GATE_ISSUED_ATOMS, "current_vs_gate_issued_delta_atoms")
        if issued_atoms is not None
        else None
    )
    gate_counted_delta = counted_atoms - GATE_COUNTED_ATOMS if counted_atoms is not None else None
    gate_circulating_delta = circulating_atoms - GATE_CIRCULATING_ATOMS if circulating_atoms is not None else None
    reconciles, notes = build_notes(
        rpc_error,
        node_error,
        gate_vault_delta,
        gate_issued_delta,
        gate_counted_delta,
        gate_circulating_delta,
    )

    artifact = {
        "run_utc": run_iso,
        "vault_balance_check": {
            "rpc": args.rpc,
            "vault_address": VAULT_ADDRESS,
            "token_address": TOKEN_ADDRESS,
            "balance_atoms": balance_atoms,
            "balance_usdc_6dp": usdc_float(balance_atoms),
            "rpc_error_key": rpc_error,
        },
        "pftl_supply_check": {
            "data_dir": args.data_dir,
            "asset_id": args.asset_id,
            "schema": schema,
            "issued_supply_atoms": issued_atoms,
            "counted_value_atoms": counted_atoms,
            "circulating_supply_atoms": circulating_atoms,
            "node_error": node_error,
        },
        "conservation_deltas": {
            "vault_minus_issued_atoms": vault_minus_issued,
            "vault_minus_counted_atoms": vault_minus_counted,
            "counted_minus_issued_atoms": counted_minus_issued,
            "issued_minus_circulating_atoms": issued_minus_circulating,
        },
        "gate_money_map_reconciliation": {
            "gate_vault_usdc": 629.999693,
            "gate_issued_usdc": 270.019667,
            "gate_counted_usdc": 609.999693,
            "gate_circulating_usdc": 60.019974,
            "current_vs_gate_vault_delta_atoms": gate_vault_delta,
            "current_vs_gate_issued_delta_atoms": gate_issued_delta,
            "reconciles": reconciles,
            "notes": notes,
        },
    }

    artifact_path = write_artifact(Path(args.out_dir), run_stamp, artifact)
    print(f"reserve_watch_artifact={artifact_path}")
    print(f"run_utc={run_iso}")
    print(f"vault_balance_atoms={balance_atoms}")
    print(f"issued_supply_atoms={issued_atoms}")
    print(f"counted_value_atoms={counted_atoms}")
    print(f"circulating_supply_atoms={circulating_atoms}")
    print(f"vault_minus_issued_atoms={vault_minus_issued}")
    print(f"vault_minus_counted_atoms={vault_minus_counted}")
    print(f"counted_minus_issued_atoms={counted_minus_issued}")
    print(f"issued_minus_circulating_atoms={issued_minus_circulating}")
    print(f"current_vs_gate_vault_delta_atoms={gate_vault_delta}")
    print(f"current_vs_gate_issued_delta_atoms={gate_issued_delta}")
    print(f"reconciles={str(reconciles).lower()}")
    print(f"notes={notes}")

    if rpc_error or node_error:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
