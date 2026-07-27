#!/usr/bin/env python3
"""Fail-closed latency gates for the Ethereum-mainnet pfUSDC round trip."""

from __future__ import annotations

import argparse
import json
import sys
import urllib.request
from pathlib import Path
from typing import Any


MAX_ROUNDTRIP_SECONDS = 25 * 60
MAX_CHECKPOINT_LAG_BLOCKS = 1
REPORT_SCHEMA = "postfiat.pfusdc.ethereum_mainnet_latency_gate.v1"
SUMMARY_SCHEMA = "postfiat.pfusdc.ethereum_mainnet_roundtrip_summary.v1"
LATEST_FINALIZED_HEIGHT_SELECTOR = "0xa09d3879"


class GateError(RuntimeError):
    pass


def load_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise GateError(f"{label} is missing: {path}") from error
    except json.JSONDecodeError as error:
        raise GateError(f"{label} is invalid JSON: {path}") from error
    if not isinstance(value, dict):
        raise GateError(f"{label} must be a JSON object")
    return value


def integer(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise GateError(f"{label} must be an integer")
    return value


def rpc_call(rpc_url: str, method: str, params: list[Any]) -> Any:
    request = urllib.request.Request(
        rpc_url,
        data=json.dumps(
            {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
        ).encode(),
        headers={
            "Content-Type": "application/json",
            "User-Agent": "postfiat-pfusdc-latency-gate/1",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            payload = json.load(response)
    except Exception as error:
        raise GateError(f"Ethereum RPC request failed: {error}") from error
    if not isinstance(payload, dict) or payload.get("error") is not None:
        raise GateError(f"Ethereum RPC returned an error for {method}")
    if "result" not in payload:
        raise GateError(f"Ethereum RPC omitted the result for {method}")
    return payload["result"]


def block_timestamp(rpc_url: str, height: int) -> int:
    block = rpc_call(rpc_url, "eth_getBlockByNumber", [hex(height), False])
    if not isinstance(block, dict) or not isinstance(block.get("timestamp"), str):
        raise GateError(f"Ethereum block {height} is unavailable")
    return int(block["timestamp"], 16)


def verifier_height(rpc_url: str, verifier: str) -> int:
    result = rpc_call(
        rpc_url,
        "eth_call",
        [{"to": verifier, "data": LATEST_FINALIZED_HEIGHT_SELECTOR}, "latest"],
    )
    if not isinstance(result, str) or not result.startswith("0x"):
        raise GateError("latestFinalizedHeight returned malformed data")
    return int(result, 16)


def validate_functional_acceptance(summary: dict[str, Any]) -> None:
    if summary.get("schema") != SUMMARY_SCHEMA:
        raise GateError("round-trip summary has an unsupported schema")
    legs = summary.get("legs")
    if not isinstance(legs, dict) or not legs:
        raise GateError("round-trip summary has no legs")
    failed = [
        name
        for name, leg in legs.items()
        if not isinstance(leg, dict) or leg.get("status") != "PASS"
    ]
    if failed:
        raise GateError(f"round-trip functional legs are not PASS: {', '.join(failed)}")
    withdrawal = legs.get("ethereum_withdrawal")
    if not isinstance(withdrawal, dict) or withdrawal.get("replay_rejected") is not True:
        raise GateError("withdrawal replay rejection is not proven")
    terminal = summary.get("terminal_state")
    if not isinstance(terminal, dict):
        raise GateError("round-trip terminal state is missing")
    if integer(
        terminal.get("campaign_conservation_residual_delta_atoms"),
        "campaign_conservation_residual_delta_atoms",
    ) != 0:
        raise GateError("round-trip conservation residual delta is nonzero")


def measure_report(
    summary: dict[str, Any],
    deposit_timestamp: int,
    withdrawal_timestamp: int,
) -> dict[str, Any]:
    validate_functional_acceptance(summary)
    if withdrawal_timestamp < deposit_timestamp:
        raise GateError("withdrawal predates the deposit")
    elapsed = withdrawal_timestamp - deposit_timestamp
    return {
        "schema": REPORT_SCHEMA,
        "gate": "full_roundtrip_latency",
        "verdict": "PASS" if elapsed <= MAX_ROUNDTRIP_SECONDS else "FAIL",
        "start_event": "ethereum_deposit_inclusion",
        "end_event": "ethereum_withdrawal_inclusion",
        "deposit_timestamp": deposit_timestamp,
        "withdrawal_timestamp": withdrawal_timestamp,
        "elapsed_seconds": elapsed,
        "maximum_seconds": MAX_ROUNDTRIP_SECONDS,
        "functional_acceptance": "PASS",
        "conservation_acceptance": "PASS",
        "replay_acceptance": "PASS",
    }


def preflight_report(pftl_tip_height: int, ethereum_verifier_height: int) -> dict[str, Any]:
    if pftl_tip_height < 0 or ethereum_verifier_height < 0:
        raise GateError("checkpoint heights must be nonnegative")
    if ethereum_verifier_height > pftl_tip_height:
        raise GateError("Ethereum verifier height is ahead of the supplied PFTL tip")
    lag = pftl_tip_height - ethereum_verifier_height
    return {
        "schema": REPORT_SCHEMA,
        "gate": "predeposit_checkpoint_freshness",
        "verdict": "PASS" if lag <= MAX_CHECKPOINT_LAG_BLOCKS else "FAIL",
        "pftl_tip_height": pftl_tip_height,
        "ethereum_verifier_height": ethereum_verifier_height,
        "checkpoint_lag_blocks": lag,
        "maximum_checkpoint_lag_blocks": MAX_CHECKPOINT_LAG_BLOCKS,
    }


def write_report(report: dict[str, Any], output: Path | None) -> None:
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if output is None:
        sys.stdout.write(rendered)
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(output.suffix + ".tmp")
    temporary.write_text(rendered, encoding="utf-8")
    temporary.replace(output)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    preflight = subparsers.add_parser("preflight")
    preflight.add_argument("--rpc-url", required=True)
    preflight.add_argument("--verifier", required=True)
    preflight.add_argument("--pftl-tip-height", required=True, type=int)
    preflight.add_argument("--output", type=Path)

    measure = subparsers.add_parser("measure")
    measure.add_argument("--summary", required=True, type=Path)
    measure.add_argument("--rpc-url")
    measure.add_argument("--deposit-timestamp", type=int)
    measure.add_argument("--withdrawal-timestamp", type=int)
    measure.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "preflight":
            report = preflight_report(
                args.pftl_tip_height, verifier_height(args.rpc_url, args.verifier)
            )
        else:
            summary = load_object(args.summary, "round-trip summary")
            legs = summary.get("legs")
            if not isinstance(legs, dict):
                raise GateError("round-trip summary has no legs")
            deposit = legs.get("ethereum_deposit")
            withdrawal = legs.get("ethereum_withdrawal")
            if not isinstance(deposit, dict) or not isinstance(withdrawal, dict):
                raise GateError("round-trip summary omits Ethereum boundary legs")
            if args.deposit_timestamp is None or args.withdrawal_timestamp is None:
                if not args.rpc_url:
                    raise GateError(
                        "--rpc-url is required unless both timestamps are supplied"
                    )
            deposit_time = (
                args.deposit_timestamp
                if args.deposit_timestamp is not None
                else block_timestamp(
                    args.rpc_url,
                    integer(deposit.get("deposit_block"), "deposit_block"),
                )
            )
            withdrawal_time = (
                args.withdrawal_timestamp
                if args.withdrawal_timestamp is not None
                else block_timestamp(
                    args.rpc_url,
                    integer(withdrawal.get("ethereum_block"), "ethereum_block"),
                )
            )
            report = measure_report(summary, deposit_time, withdrawal_time)
        write_report(report, args.output)
        return 0 if report["verdict"] == "PASS" else 1
    except GateError as error:
        print(f"pfUSDC mainnet latency gate failed closed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
