"""WAN devnet transaction-readiness preflight.

The preflight is intentionally read-only. It checks whether the wallet-facing
fleet is converged before transaction tests or browser sends are treated as
meaningful evidence.
"""

from __future__ import annotations

import argparse
import collections
import json
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterable

from .client import PostFiatRpcClient


DEFAULT_WAN_VALIDATORS: tuple[tuple[str, str, int], ...] = (
    ("validator-0", "127.0.0.1", 27650),
    ("validator-1", "127.0.0.1", 27651),
    ("validator-2", "127.0.0.1", 27652),
    ("validator-3", "127.0.0.1", 27653),
    ("validator-4", "127.0.0.1", 27654),
    ("validator-5", "127.0.0.1", 27655),
)


@dataclass(frozen=True)
class ValidatorEndpoint:
    validator_id: str
    host: str
    port: int

    @classmethod
    def parse(cls, value: str) -> "ValidatorEndpoint":
        if "=" not in value:
            raise ValueError(f"validator endpoint must be id=host:port: {value}")
        validator_id, endpoint = value.split("=", 1)
        if ":" not in endpoint:
            raise ValueError(f"validator endpoint must include host:port: {value}")
        host, port_text = endpoint.rsplit(":", 1)
        validator_id = validator_id.strip()
        host = host.strip()
        port = int(port_text)
        if not validator_id:
            raise ValueError("validator id must not be empty")
        if not host:
            raise ValueError("validator host must not be empty")
        if not (0 < port < 65536):
            raise ValueError(f"validator port out of range: {value}")
        return cls(validator_id=validator_id, host=host, port=port)

    def to_dict(self) -> dict[str, Any]:
        return {"validator_id": self.validator_id, "host": self.host, "port": self.port}


def default_validator_endpoints() -> list[ValidatorEndpoint]:
    return [
        ValidatorEndpoint(validator_id=validator_id, host=host, port=port)
        for validator_id, host, port in DEFAULT_WAN_VALIDATORS
    ]


def parse_validator_endpoints(values: str | None) -> list[ValidatorEndpoint]:
    if values is None or not values.strip():
        return default_validator_endpoints()
    return [ValidatorEndpoint.parse(part.strip()) for part in values.split(",") if part.strip()]


def _rpc_result(call: Callable[[], Any]) -> dict[str, Any]:
    started = time.monotonic()
    try:
        result = call()
    except Exception as exc:  # noqa: BLE001 - preserve exact preflight failure text.
        return {
            "ok": False,
            "duration_ms": round((time.monotonic() - started) * 1000, 3),
            "error": str(exc),
            "error_type": type(exc).__name__,
        }
    return {
        "ok": True,
        "duration_ms": round((time.monotonic() - started) * 1000, 3),
        "result": result,
    }


def _pending_summary(mempool_result: dict[str, Any]) -> dict[str, Any]:
    if not isinstance(mempool_result, dict):
        return {"total": None, "queues": {}}
    queues: dict[str, Any] = {}
    total = 0
    for key, value in sorted(mempool_result.items()):
        if isinstance(value, list):
            tx_ids = []
            for item in value[:20]:
                if isinstance(item, dict):
                    tx_ids.append(item.get("tx_id"))
                else:
                    tx_ids.append(None)
            queues[key] = {"count": len(value), "tx_ids": tx_ids}
            total += len(value)
    return {"total": total, "queues": queues}


def _compact_status(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        return {"raw_type": type(result).__name__}
    keys = [
        "node_id",
        "status",
        "chain_id",
        "genesis_hash",
        "protocol_version",
        "block_height",
        "block_tip_hash",
        "state_root",
        "mempool_pending",
        "validator_count",
        "last_run_unix",
    ]
    return {key: result.get(key) for key in keys if key in result}


def _compact_server_info(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        return {"raw_type": type(result).__name__}
    return {
        "node_id": result.get("node_id"),
        "status": result.get("status"),
        "chain_id": result.get("chain_id"),
        "genesis_hash": result.get("genesis_hash"),
        "protocol_version": result.get("protocol_version"),
        "ledger": result.get("ledger"),
        "mempool": result.get("mempool"),
        "rpc": result.get("rpc"),
        "validators": result.get("validators"),
    }


def collect_rpc_preflight(
    endpoints: Iterable[ValidatorEndpoint],
    *,
    timeout_seconds: float,
) -> list[dict[str, Any]]:
    entries = []
    for endpoint in endpoints:
        client = PostFiatRpcClient(
            f"{endpoint.host}:{endpoint.port}",
            timeout_seconds=timeout_seconds,
        )
        status = _rpc_result(client.status)
        server_info = _rpc_result(client.server_info)
        mempool_status = _rpc_result(client.mempool_status)
        mempool_result = mempool_status.get("result") if mempool_status.get("ok") else {}
        entries.append(
            {
                **endpoint.to_dict(),
                "reachable": status["ok"] and server_info["ok"] and mempool_status["ok"],
                "status": (
                    {**status, "result": _compact_status(status.get("result"))}
                    if status.get("ok")
                    else status
                ),
                "server_info": (
                    {**server_info, "result": _compact_server_info(server_info.get("result"))}
                    if server_info.get("ok")
                    else server_info
                ),
                "mempool_status": {
                    **mempool_status,
                    "summary": _pending_summary(mempool_result),
                },
            }
        )
    return entries


def _service_names(validator_id: str) -> tuple[str, str]:
    suffix = validator_id.removeprefix("validator-")
    return (f"postfiat-validator-{suffix}.service", f"postfiat-validator-{suffix}-rpc.service")


def collect_ssh_inventory(
    endpoints: Iterable[ValidatorEndpoint],
    *,
    ssh_user: str,
    timeout_seconds: float,
) -> dict[str, Any]:
    inventory: dict[str, Any] = {}
    for endpoint in endpoints:
        validator_service, rpc_service = _service_names(endpoint.validator_id)
        remote_command = (
            "set -o pipefail; "
            "hostname; "
            "date -u +%Y-%m-%dT%H:%M:%SZ; "
            "pgrep -a postfiat-node || true; "
            "sha256sum /usr/local/bin/postfiat-node 2>/dev/null || true; "
            f"systemctl show {validator_service} {rpc_service} "
            "--property=Id,ActiveState,SubState,ExecMainPID,ExecStart --no-pager 2>/dev/null || true"
        )
        command = [
            "ssh",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=6",
            "-o",
            "StrictHostKeyChecking=no",
            f"{ssh_user}@{endpoint.host}",
            remote_command,
        ]
        started = time.monotonic()
        try:
            completed = subprocess.run(
                command,
                check=False,
                capture_output=True,
                text=True,
                timeout=timeout_seconds,
            )
            inventory[endpoint.validator_id] = {
                "ok": completed.returncode == 0,
                "duration_ms": round((time.monotonic() - started) * 1000, 3),
                "returncode": completed.returncode,
                "stdout": completed.stdout,
                "stderr": completed.stderr,
            }
        except Exception as exc:  # noqa: BLE001 - inventory failure is report evidence.
            inventory[endpoint.validator_id] = {
                "ok": False,
                "duration_ms": round((time.monotonic() - started) * 1000, 3),
                "error": str(exc),
                "error_type": type(exc).__name__,
            }
    return inventory


def _ledger_key(entry: dict[str, Any]) -> tuple[Any, Any, Any]:
    result = entry.get("status", {}).get("result", {})
    if not isinstance(result, dict):
        return (None, None, None)
    return (result.get("block_height"), result.get("block_tip_hash"), result.get("state_root"))


def _rpc_caps(entry: dict[str, Any]) -> dict[str, Any]:
    result = entry.get("server_info", {}).get("result", {})
    if not isinstance(result, dict):
        return {}
    rpc = result.get("rpc", {})
    return rpc if isinstance(rpc, dict) else {}


def _ssh_binary_hashes(ssh_inventory: dict[str, Any]) -> dict[str, list[str]]:
    hashes: dict[str, list[str]] = collections.defaultdict(list)
    for validator_id, inventory in sorted(ssh_inventory.items()):
        stdout = inventory.get("stdout", "")
        if not isinstance(stdout, str):
            continue
        for line in stdout.splitlines():
            if line.endswith("  /usr/local/bin/postfiat-node"):
                hashes[line.split()[0]].append(validator_id)
    return dict(hashes)


def summarize_preflight(
    entries: list[dict[str, Any]],
    *,
    ssh_inventory: dict[str, Any] | None = None,
    quorum_min: int = 5,
) -> dict[str, Any]:
    reasons: list[str] = []
    reachable = [entry["validator_id"] for entry in entries if entry.get("reachable")]
    if len(reachable) != len(entries):
        missing = [entry["validator_id"] for entry in entries if not entry.get("reachable")]
        reasons.append(f"unreachable validators: {', '.join(missing)}")

    ledger_groups: dict[str, list[str]] = collections.defaultdict(list)
    for entry in entries:
        height, tip, root = _ledger_key(entry)
        ledger_groups[f"{height}|{tip}|{root}"].append(entry["validator_id"])
    largest_ledger_group = max((len(group) for group in ledger_groups.values()), default=0)
    if len(ledger_groups) != 1:
        reasons.append(
            "ledger divergence: "
            + "; ".join(f"{key} -> {','.join(ids)}" for key, ids in sorted(ledger_groups.items()))
        )
    if largest_ledger_group < quorum_min:
        reasons.append(f"largest converged ledger group has {largest_ledger_group}, need {quorum_min}")

    pending = []
    for entry in entries:
        pending_total = entry.get("mempool_status", {}).get("summary", {}).get("total")
        if pending_total:
            pending.append(f"{entry['validator_id']}={pending_total}")
    if pending:
        reasons.append("non-empty mempools: " + ", ".join(pending))

    finality_disabled = []
    read_only = []
    for entry in entries:
        caps = _rpc_caps(entry)
        if caps.get("read_only"):
            read_only.append(entry["validator_id"])
        if caps.get("mempool_submit_finality_enabled") is not True:
            finality_disabled.append(entry["validator_id"])
    if read_only:
        reasons.append("read-only RPC endpoints: " + ", ".join(read_only))
    if finality_disabled:
        reasons.append("finality RPC disabled: " + ", ".join(finality_disabled))

    binary_hash_groups = _ssh_binary_hashes(ssh_inventory or {})
    if ssh_inventory is not None:
        ssh_failed = [
            validator_id
            for validator_id, inventory in sorted(ssh_inventory.items())
            if not inventory.get("ok")
        ]
        if ssh_failed:
            reasons.append("ssh inventory failed: " + ", ".join(ssh_failed))
        if len(binary_hash_groups) != 1:
            reasons.append(
                "binary hash divergence or missing hashes: "
                + json.dumps(binary_hash_groups, sort_keys=True)
            )

    return {
        "healthy": len(reasons) == 0,
        "reachable_count": len(reachable),
        "validator_count": len(entries),
        "quorum_min": quorum_min,
        "ledger_groups": dict(sorted(ledger_groups.items())),
        "largest_ledger_group": largest_ledger_group,
        "ssh_binary_hash_groups": binary_hash_groups,
        "red_reasons": reasons,
    }


def build_report(
    *,
    endpoints: list[ValidatorEndpoint],
    timeout_seconds: float,
    ssh_inventory: bool,
    ssh_user: str,
    quorum_min: int,
) -> dict[str, Any]:
    entries = collect_rpc_preflight(endpoints, timeout_seconds=timeout_seconds)
    ssh = (
        collect_ssh_inventory(endpoints, ssh_user=ssh_user, timeout_seconds=max(timeout_seconds, 12.0))
        if ssh_inventory
        else None
    )
    return {
        "schema": "postfiat-wan-transaction-preflight-v1",
        "captured_at_unix": time.time(),
        "endpoints": [endpoint.to_dict() for endpoint in endpoints],
        "summary": summarize_preflight(entries, ssh_inventory=ssh, quorum_min=quorum_min),
        "validators": entries,
        "ssh_inventory": ssh,
    }


def _print_human_summary(report: dict[str, Any]) -> None:
    summary = report["summary"]
    status = "GREEN" if summary["healthy"] else "RED"
    print(f"wan transaction preflight: {status}")
    print(
        f"reachable={summary['reachable_count']}/{summary['validator_count']} "
        f"largest_ledger_group={summary['largest_ledger_group']} quorum_min={summary['quorum_min']}"
    )
    for reason in summary["red_reasons"]:
        print(f"reason: {reason}")
    for entry in report["validators"]:
        result = entry.get("status", {}).get("result", {})
        pending = entry.get("mempool_status", {}).get("summary", {}).get("total")
        print(
            f"{entry['validator_id']} {entry['host']}:{entry['port']} "
            f"height={result.get('block_height')} root={result.get('state_root')} "
            f"mempool={pending} reachable={entry.get('reachable')}"
        )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--validators",
        help="comma-separated validator endpoints as validator-0=host:port,...",
    )
    parser.add_argument("--output", type=Path, help="write full JSON report to this path")
    parser.add_argument("--timeout-seconds", type=float, default=8.0)
    parser.add_argument("--quorum-min", type=int, default=5)
    parser.add_argument("--ssh-inventory", action="store_true")
    parser.add_argument("--ssh-user", default="root")
    parser.add_argument(
        "--strict-exit",
        action="store_true",
        help="exit nonzero when the preflight is red",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    endpoints = parse_validator_endpoints(args.validators)
    report = build_report(
        endpoints=endpoints,
        timeout_seconds=args.timeout_seconds,
        ssh_inventory=args.ssh_inventory,
        ssh_user=args.ssh_user,
        quorum_min=args.quorum_min,
    )
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    _print_human_summary(report)
    return 1 if args.strict_exit and not report["summary"]["healthy"] else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
