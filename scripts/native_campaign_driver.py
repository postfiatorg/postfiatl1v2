#!/usr/bin/env python3
"""Fail-closed native campaign executor.

The module is deliberately independent of application control planes.  It only
loads held packet JSON, invokes the pinned node binary or the packet's leaf
command, and records finality evidence in the native journal.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from decimal import Decimal, InvalidOperation
from pathlib import Path
import re
import subprocess
import socket
import sys
import time
import tempfile
from typing import Any, Callable, Iterable, Mapping, Sequence

SCHEMA = "postfiat.native-campaign-journal-v1"
RPC_VERSION = "postfiat-local-rpc-v1"
REQUEST_SCHEMA = "postfiat-certified-asset-ops-request-v1"
# deployed fleet binary pin (2246d257) — reference only, never overwritten
EXPECTED_NODE_SHA256 = "05330fb20a40b8a4536000ec57da1862d879bcdc4a21bc8c0657f5c56aa8e0f5"
EXPECTED_CLIENT_NODE_SHA256 = "a982f8d27a42daad39e6a7d2ad1aff69a97064b7890da654fc9aae8f47f58f95"
DEFAULT_CLIENT_NODE = "/tmp/fire-20260806-bin/postfiat-node-client-depositv2"
FLEET_ENDPOINTS = ["127.0.0.1:39660", "127.0.0.1:39651", "127.0.0.1:39652", "127.0.0.1:39653", "127.0.0.1:39654", "127.0.0.1:39655"]
CERTIFIED = {
    "2a": "pftl_uniswap_order_reserve",
    "2b": "pftl_uniswap_primary_subscribe_v2",
    "3a": "pftl_uniswap_export_debit",
    "4-import": "pftl_uniswap_return_import",
    "5a": "pftl_uniswap_primary_redeem",
}

class StopError(ValueError):
    """A state or receipt mismatch that must stop without retry."""

class ConfigError(ValueError):
    """A malformed invocation or packet."""


def _load(path: Path) -> Any:
    try:
        with path.open(encoding="utf-8") as fh:
            return json.load(fh)
    except (OSError, ValueError) as exc:
        raise ConfigError(f"invalid JSON: {path}: {exc}") from exc


def _write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=str(path.parent), text=True)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            json.dump(value, fh, indent=2, sort_keys=True)
            fh.write("\n")
            fh.flush()
            os.fsync(fh.fileno())
        os.replace(tmp_name, path)
    finally:
        try:
            os.unlink(tmp_name)
        except FileNotFoundError:
            pass


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for block in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def _binary_path() -> Path:
    return Path(os.environ.get("POSTFIAT_NODE_BIN", DEFAULT_CLIENT_NODE))


def check_binary(path: Path | None = None) -> Path:
    path = path or _binary_path()
    if not path.is_file():
        raise ConfigError(f"node binary is missing: {path}")
    digest = _sha256(path)
    if digest != EXPECTED_CLIENT_NODE_SHA256:
        raise ConfigError(f"node binary digest mismatch: {digest}")
    return path


def _result_payload(result: Any) -> Any:
    if isinstance(result, Mapping):
        return result
    if hasattr(result, "stdout"):
        text = getattr(result, "stdout", "")
        if isinstance(text, bytes):
            text = text.decode()
    else:
        text = result if isinstance(result, str) else ""
    try:
        return json.loads(text)
    except (TypeError, ValueError):
        return text


def _call(argv: Sequence[str], runner: Callable[[Sequence[str]], Any] | None = None) -> Any:
    if runner is not None:
        result = runner(list(argv))
        if isinstance(result, Mapping) and result.get("returncode", 0) not in (0, None):
            raise StopError(f"command failed ({result.get('returncode')}): {' '.join(argv)}")
        return _result_payload(result)
    try:
        completed = subprocess.run(list(argv), check=False, text=True, capture_output=True)
    except OSError as exc:
        raise StopError(f"command unavailable: {argv[0]}: {exc}") from exc
    if completed.returncode:
        raise StopError(f"command failed ({completed.returncode}): {' '.join(argv)}\n{completed.stderr[-500:]}")
    return _result_payload(completed)


def _leg_key(value: Any) -> str:
    return str(value).strip().lower()


def _packet_id(packet: Mapping[str, Any]) -> str:
    value = packet.get("packet_id") or packet.get("id")
    if not value:
        raise ConfigError("packet_id is required")
    return str(value)


def _binding_entries(binding: Any) -> list[Mapping[str, Any]]:
    if isinstance(binding, Mapping):
        for key in ("packets", "entries", "binding"):
            value = binding.get(key)
            if isinstance(value, list):
                return [x for x in value if isinstance(x, Mapping)]
        return [binding]
    if isinstance(binding, list):
        return [x for x in binding if isinstance(x, Mapping)]
    raise ConfigError("binding must be an object or list")


def _verify_binding(packet_path: Path, packet: Mapping[str, Any], binding: Any) -> None:
    actual = _sha256(packet_path)
    packet_id = _packet_id(packet)
    found = None
    for entry in _binding_entries(binding):
        if entry.get("packet_id") == packet_id or entry.get("id") == packet_id:
            found = entry
            break
        entry_path = str(entry.get("path", ""))
        if entry_path and (entry_path == str(packet_path) or Path(entry_path).name == packet_path.name):
            found = entry
            break
    if found is None:
        raise StopError(f"packet has no authorization entry: {packet_id}")
    expected = found.get("sha256") or found.get("packet_sha256")
    if expected and not str(expected).startswith("PENDING") and actual.lower() != str(expected).lower():
        raise StopError(f"packet SHA mismatch: {actual} != {expected}")
    if not expected:
        raise StopError(f"authorization entry has no SHA: {packet_id}")


def _status_endpoints(packet: Mapping[str, Any], binding: Any) -> list[str]:
    values = packet.get("fleet_endpoints")
    if not values and isinstance(packet.get("endpoints"), Mapping):
        values = packet.get("endpoints", {}).get("pftl")
    if not values and isinstance(binding, Mapping):
        values = binding.get("fleet_endpoints")
    values = values or FLEET_ENDPOINTS
    if isinstance(values, Mapping):
        values = list(values.values())
    out = [str(x) for x in values]
    if len(out) != 6:
        raise StopError(f"fleet quorum requires six endpoints, got {len(out)}")
    return out


def _rpc_call(endpoint: str, method: str, params: Mapping[str, Any], runner: Callable[[Sequence[str]], Any] | None, node: Path) -> Any:
    if runner is not None:
        argv = [str(node), "rpc", "--method", method, "--endpoint", endpoint]
        for key, value in params.items():
            argv.extend([f"--{key.replace('_', '-')}", str(value)])
        return _result_payload(_call(argv, runner))
    if ":" not in endpoint:
        raise StopError(f"invalid fleet endpoint: {endpoint}")
    host, port_text = endpoint.rsplit(":", 1)
    try:
        port = int(port_text)
    except ValueError as exc:
        raise StopError(f"invalid fleet port: {endpoint}") from exc
    request = {"version": RPC_VERSION, "id": f"native-{method}", "method": method, "params": dict(params)}
    try:
        with socket.create_connection((host, port), timeout=10) as conn:
            conn.sendall((json.dumps(request, separators=(",", ":")) + "\n").encode())
            data = b""
            while b"\n" not in data:
                chunk = conn.recv(65536)
                if not chunk:
                    break
                data += chunk
    except OSError as exc:
        raise StopError(f"fleet RPC unavailable at {endpoint}: {exc}") from exc
    try:
        response = json.loads(data.split(b"\n", 1)[0].decode())
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise StopError(f"fleet RPC malformed response at {endpoint}") from exc
    if not isinstance(response, Mapping) or response.get("error"):
        raise StopError(f"fleet RPC error at {endpoint}: {response.get('error') if isinstance(response, Mapping) else response}")
    return response.get("result", response)


def _status(endpoint: str, node: Path, runner: Callable[[Sequence[str]], Any] | None) -> Mapping[str, Any]:
    value = _rpc_call(endpoint, "status", {}, runner, node)
    if not isinstance(value, Mapping):
        raise StopError(f"status response is not JSON: {endpoint}")
    return value


def _field(value: Mapping[str, Any], *names: str) -> Any:
    for name in names:
        if name in value:
            return value[name]
    return None


def _source_rpc_url(packet: Mapping[str, Any]) -> str:
    """Return the packet-bound Ethereum RPC URL for every cast read."""
    value = packet.get("source_rpc_url") or packet.get("ethereum_rpc_url")
    if not isinstance(value, str) or not value or "PENDING" in value:
        raise ConfigError("packet source_rpc_url is required for Ethereum receipt reads")
    return value


def _cast_receipt(packet: Mapping[str, Any], tx_hash: str, runner: Callable[[Sequence[str]], Any] | None) -> Mapping[str, Any]:
    """Read and strictly validate one Ethereum receipt through the packet RPC."""
    receipt = _call(
        ["cast", "receipt", str(tx_hash), "--rpc-url", _source_rpc_url(packet), "--json"],
        runner,
    )
    if not isinstance(receipt, Mapping):
        raise StopError("cast receipt must be JSON object")
    actual_hash = receipt.get("transactionHash", receipt.get("transaction_hash"))
    if not isinstance(actual_hash, str) or actual_hash.lower() != str(tx_hash).lower():
        raise StopError("cast receipt transaction hash mismatch")
    if receipt.get("status") != "0x1":
        raise StopError("Ethereum receipt status is not 0x1")
    block_number = receipt.get("blockNumber", receipt.get("block_number"))
    block_hash = receipt.get("blockHash", receipt.get("block_hash"))
    if not block_number or not isinstance(block_hash, str) or not block_hash:
        raise StopError("cast receipt missing block number or block hash")
    return receipt


_SPEND_LEGS = {"0", "1", "3b0", "3c", "3d", "3e", "3f", "3g", "3h", "4-burn", "5b"}


def _budget_guard(packet: Mapping[str, Any], journal: Mapping[str, Any]) -> Decimal:
    leg = _leg_key(packet.get("leg"))
    budget = packet.get("budget_guard")
    if not isinstance(budget, Mapping):
        if leg in _SPEND_LEGS:
            raise StopError(f"budget_guard missing for spend leg {leg}")
        return Decimal("0")
    try:
        cap = Decimal(str(budget["cap_usdc"]))
        prior = Decimal(str(budget["prior_spend_usdc"]))
        ceiling = Decimal(str(budget.get("leg_ceiling_usdc", "0")))
    except (KeyError, InvalidOperation, ValueError) as exc:
        raise StopError("budget_guard fields are invalid") from exc
    if cap < 0 or prior < 0 or ceiling < 0:
        raise StopError("budget_guard values must be non-negative")
    projection = prior + ceiling
    if projection > cap:
        raise StopError(f"budget cap exceeded: {projection} > {cap}")
    return projection


def _preflight(packet: Mapping[str, Any], binding: Any, node: Path, runner: Callable[[Sequence[str]], Any] | None) -> dict[str, Any]:
    statuses = [_status(ep, node, runner) for ep in _status_endpoints(packet, binding)]
    genesis = [_field(s, "genesis_hash", "genesis") for s in statuses]
    roots = [_field(s, "state_root", "stateRoot", "root") for s in statuses]
    if any("block_height" not in s for s in statuses):
        raise StopError("fleet status omitted required block_height")
    heights = [_field(s, "block_height") for s in statuses]
    if any(x is None for x in genesis + roots + heights):
        raise StopError("fleet status omitted genesis, state_root, or height")
    if len(set(map(str, genesis))) != 1 or len(set(map(str, roots))) != 1:
        raise StopError("fleet status disagreement")
    try:
        height_ints = [int(x) for x in heights]
    except (TypeError, ValueError) as exc:
        raise StopError("fleet height is not integer") from exc
    if max(height_ints) - min(height_ints) > 1:
        raise StopError("fleet height spread exceeds one")
    expected = packet.get("preflight", packet.get("expected_state", {}))
    if isinstance(expected, Mapping):
        for key, actual in (("genesis_hash", genesis[0]), ("state_root", roots[0]), ("height", height_ints[0])):
            wanted = expected.get(key)
            if wanted is not None and key != "height" and str(actual) != str(wanted):
                raise StopError(f"packet precondition mismatch: {key}")
            if wanted is not None and key == "height" and height_ints[0] < int(wanted):
                raise StopError("fleet height below packet minimum")
        for key in ("route_epoch", "pricing_nav_epoch", "pricing_reserve_packet_hash", "route_id"):
            wanted = expected.get(key)
            if wanted is not None:
                actual = _field(statuses[0], key)
                if actual is not None and str(actual) != str(wanted):
                    raise StopError(f"packet precondition mismatch: {key}")
    for read in packet.get("ethereum_reads", []) if isinstance(packet.get("ethereum_reads"), list) else []:
        if isinstance(read, list):
            _call([str(x) for x in read], runner)
        elif isinstance(read, str):
            _call(read.split(), runner)
    return {"genesis_hash": str(genesis[0]), "state_root": str(roots[0]), "height": height_ints[0], "fleet": statuses}


def _validate_finality(receipt: Mapping[str, Any], expected: Mapping[str, Any]) -> bool:
    if receipt.get("accepted") is not True:
        raise ValueError("receipt not accepted")
    for key in ("height", "state_root"):
        if str(receipt.get(key)) != str(expected.get(key)):
            raise ValueError(f"finality {key} mismatch")
    return True


def _validate_receipt_identity(actual: Mapping[str, Any], expected: Mapping[str, Any]) -> bool:
    if actual.get("receipt_id") != expected.get("receipt_id"):
        raise ValueError("receipt id mismatch")
    return True


def _validate_unique(values: Iterable[Any]) -> bool:
    vals = [str(v) for v in values]
    if len(vals) != len(set(vals)):
        raise ValueError("duplicate nonce, packet, or burn identifier")
    return True


def _validate_recipient(op: Mapping[str, Any], expected: str) -> bool:
    actual = next((op.get(k) for k in ("recipient", "ethereum_recipient", "pftl_recipient", "settlement_recipient") if op.get(k) is not None), None)
    if actual != expected:
        raise ValueError("recipient mismatch")
    return True


def _validate_delta(before: Any, after: Any, expected: Any) -> bool:
    try:
        if int(after) - int(before) != int(expected):
            raise ValueError("amount delta mismatch")
    except (TypeError, ValueError) as exc:
        if isinstance(exc, ValueError) and str(exc) == "amount delta mismatch":
            raise
        raise ValueError("amount delta is not integer") from exc
    return True


def _validate_nav(actual: Mapping[str, Any], expected: Mapping[str, Any]) -> bool:
    if int(actual.get("epoch", -1)) != int(expected.get("epoch", -2)):
        raise ValueError("stale NAV epoch")
    if actual.get("packet_hash") != expected.get("packet_hash"):
        raise ValueError("stale NAV packet")
    return True


def _validate_replay(journal: Mapping[str, Any], incoming: Mapping[str, Any]) -> bool:
    existing = {str(x) for x in journal.get("receipt_ids", [])}
    existing.update(str(x) for leg in journal.get("legs", []) if isinstance(leg, Mapping) for x in leg.get("finality", []) if isinstance(x, Mapping) for x in (x.get("receipt_id"),) if x)
    incoming_ids = {str(x) for x in incoming.get("receipt_ids", [])}
    if existing & incoming_ids:
        raise ValueError("replayed campaign receipt")
    return True


def _validate_account_assets(payload: Mapping[str, Any], asset: str) -> bool:
    if "wallet_total" in payload or "total" in payload and asset not in payload:
        raise ValueError("aggregate wallet total cannot substitute account asset")
    assets = payload.get("account_assets", payload)
    if not isinstance(assets, Mapping) or asset not in assets:
        raise ValueError("account asset missing")
    return True


def _validate_eth_finality(receipt: Mapping[str, Any]) -> bool:
    status = receipt.get("status")
    if status not in ("0x1", 1, "1") or not receipt.get("blockNumber", receipt.get("block_number")):
        raise ValueError("Ethereum finality missing")
    return True


def _validate_signer(receipt: Mapping[str, Any], expected: str) -> bool:
    actual = receipt.get("signer", receipt.get("operator"))
    if actual != expected:
        raise ValueError("signer mismatch")
    return True


def _validate_swap(receipt: Mapping[str, Any]) -> bool:
    try:
        output = int(receipt["output_atoms"])
        minimum = int(receipt["min_output_atoms"])
    except (KeyError, TypeError, ValueError) as exc:
        raise ValueError("swap output evidence missing") from exc
    if output < minimum:
        raise ValueError("swap output below packet minimum")
    return True


def _validate_state_forbidden(state: Any) -> bool:
    text = json.dumps(state, sort_keys=True).lower()
    if "external-control-plane" in text or "adapter-generated" in text or "local receipt registry" in text:
        raise ValueError("non-native state reference")
    return True


def _journal_path(artifact_dir: Path) -> Path:
    return artifact_dir / "campaign-journal.json"


def _read_journal(artifact_dir: Path) -> dict[str, Any]:
    path = _journal_path(artifact_dir)
    if not path.exists():
        return {"schema": SCHEMA, "legs": [], "resume_policy": "resume only when all listed receipt_ids/finality roots match; otherwise STOP"}
    value = _load(path)
    if not isinstance(value, dict) or value.get("schema") != SCHEMA or not isinstance(value.get("legs"), list):
        raise StopError("journal schema mismatch")
    _validate_state_forbidden(value)
    return value


def _leg_order(value: Any) -> tuple[int, str]:
    text = _leg_key(value)
    m = re.match(r"(\d+)(.*)", text)
    return (int(m.group(1)) if m else 999, m.group(2) if m else text)


def resume_after_interruption(path: Path, requested: str) -> bool:
    journal = _load(path)
    if not isinstance(journal, Mapping) or not journal.get("legs"):
        raise ValueError("resume has no finalized receipt position")
    return True


def _first_unjournaled(journal: Mapping[str, Any], requested: Any) -> bool:
    done = [_leg_order(x.get("leg")) for x in journal.get("legs", []) if isinstance(x, Mapping)]
    if not done:
        return _leg_order(requested) == (0, "")
    return _leg_order(requested) > max(done)


def _operation_from_packet(packet: Mapping[str, Any], leg: str) -> tuple[str, dict[str, Any]]:
    template = packet.get("ops_file_template")
    operation = None
    if isinstance(template, Mapping) and isinstance(template.get("operations"), list) and template["operations"]:
        first = template["operations"][0]
        if isinstance(first, Mapping):
            operation = first.get("operation")
    if not isinstance(operation, Mapping):
        operation = packet.get("operation")
    if not isinstance(operation, Mapping):
        for key in ("primary_order_reserve", "primary_subscribe", "primary_redeem", "export_debit", "return_import"):
            if isinstance(packet.get(key), Mapping):
                operation = dict(packet[key])
                break
    if not isinstance(operation, Mapping):
        raise ConfigError(f"certified packet has no operation body: {leg}")
    body = dict(operation)
    tag = str(body.pop("operation", CERTIFIED.get(leg, "")))
    if not tag:
        raise ConfigError(f"unknown certified leg: {leg}")
    allowed = {
        "pftl_uniswap_order_reserve": {"subscriber", "route_id", "reservation_id", "ethereum_recipient", "route_epoch", "policy_epoch", "policy_hash", "mint_amount_atoms", "max_settlement_value_atoms", "expires_at_height"},
        "pftl_uniswap_primary_subscribe_v2": {"subscriber", "route_id", "reservation_id", "subscription_nonce", "settlement_asset_id", "settlement_value_atoms", "pricing_nav_epoch", "pricing_reserve_packet_hash"},
        "pftl_uniswap_export_debit": {"owner", "route_id", "packet_hash", "export_nonce", "ethereum_recipient", "amount_atoms", "reservation_id", "settlement_value_atoms", "destination_deadline_seconds", "refund_delay_blocks", "ethereum_packet_digest", "ethereum_packet_schema_version"},
        "pftl_uniswap_return_import": {"operator", "route_id", "burn_event_hash", "ethereum_chain_id", "bridge_controller", "wrapped_navcoin_token", "native_nav_asset_id", "ethereum_sender", "pftl_recipient", "amount_atoms", "return_nonce", "burn_height", "finalized_height", "external_event_proof"},
        "pftl_uniswap_primary_redeem": {"owner", "settlement_recipient", "route_id", "redemption_nonce", "nav_amount_atoms", "min_settlement_value_atoms", "route_epoch", "policy_epoch", "policy_hash", "pricing_nav_epoch", "pricing_reserve_packet_hash", "expires_at_height"},
    }
    if tag not in allowed:
        raise ConfigError(f"unsupported operation tag: {tag}")
    optional = {"reservation_id", "settlement_value_atoms", "ethereum_packet_digest", "ethereum_packet_schema_version", "external_event_proof"}
    for field in allowed[tag] - optional:
        value = body.get(field)
        if isinstance(value, str) and value.startswith("PENDING-FIRE-TIME"):
            raise ConfigError(f"unresolved fire-time field: {field}")
    body = {k: v for k, v in body.items() if k in allowed[tag] and not (k in optional and isinstance(v, str) and v.startswith("PENDING-FIRE-TIME"))}
    packet_amount = packet.get("amount_atoms")
    amount_field = {"pftl_uniswap_export_debit":"amount_atoms", "pftl_uniswap_return_import":"amount_atoms", "pftl_uniswap_primary_redeem":"nav_amount_atoms"}.get(tag)
    if amount_field and isinstance(packet_amount, int) and isinstance(body.get(amount_field), int) and body[amount_field] != packet_amount:
        raise ConfigError(f"amount mismatch: {amount_field}")
    missing = [k for k in allowed[tag] if k not in body and k not in optional]
    if missing:
        raise ConfigError(f"operation missing fields: {','.join(sorted(missing))}")
    return tag, body


def _render_ops(packet: Mapping[str, Any], leg_dir: Path) -> tuple[Path, str, dict[str, Any]]:
    leg = _leg_key(packet.get("leg"))
    tag, body = _operation_from_packet(packet, leg)
    source = str(packet.get("source", body.get("subscriber", body.get("owner", body.get("operator", "")))))
    key_file = str(packet.get("key_file") or packet.get("executor", {}).get("key_file", ""))
    if not source or not key_file:
        raise ConfigError("certified operation requires source and key_file location")
    deps = packet.get("dependencies", [])
    if not isinstance(deps, list):
        raise ConfigError("dependencies must be an array")
    request = {"schema": REQUEST_SCHEMA, "operations": [{"label": _packet_id(packet), "source": source, "key_file": key_file, "dependencies": deps, "operation": {"operation": tag, **body}}]}
    path = leg_dir / "ops.json"
    _write_json(path, request)
    return path, tag, request


def _command_from_packet(packet: Mapping[str, Any], node: Path, leg_dir: Path) -> list[list[str]]:
    executor = packet.get("executor") if isinstance(packet.get("executor"), Mapping) else {}
    commands = executor.get("commands")
    if not isinstance(commands, list):
        command = executor.get("args") or executor.get("command")
        commands = [command] if command else []
    out: list[list[str]] = []
    for command in commands:
        if isinstance(command, str):
            tokens = command.split()
        elif isinstance(command, list):
            tokens = [str(x) for x in command]
        else:
            raise ConfigError("executor command must be string or array")
        tokens = [str(leg_dir) if x == "{artifact_dir}" else x for x in tokens]
        out.append(tokens)
    if not out:
        template = executor.get("command_template")
        if isinstance(template, str) and "PENDING" not in template:
            out.append(template.split())
    if not out:
        raise ConfigError("packet has no executable command")
    return out


def _normalize_stage(commands: list[list[str]], node: Path) -> list[list[str]]:
    out: list[list[str]] = []
    for argv in commands:
        if not argv:
            raise ConfigError("empty stage command")
        if argv[0] in ("postfiat-node", "postfiat-node-canonical"):
            argv = [str(node), *argv[1:]]
        elif argv[0] == "nav-roundtrip-live-demo":
            argv = [str(node), *argv]
        out.append(argv)
    return out


def _phase_receipt_gate(packet: Mapping[str, Any], reports: list[Mapping[str, Any]], runner: Callable[[Sequence[str]], Any] | None) -> None:
    expected = packet.get("expected_receipt") if isinstance(packet.get("expected_receipt"), Mapping) else {}
    for report in reports:
        tx_hash = report.get("tx_hash") or report.get("transaction_hash")
        if not tx_hash:
            continue
        if report.get("status") not in (None, 1, "0x1", "1"):
            raise StopError("phase-1 EVM receipt reverted")
        if expected:
            _validate_expected_receipt(report, expected)
        receipt = _cast_receipt(packet, str(tx_hash), runner)
        _validate_eth_finality(receipt)
        if expected:
            _validate_expected_receipt(receipt, expected)


def _locate_evm_report(leg_dir: Path) -> Path | None:
    matches = sorted(leg_dir.glob("**/evm-deposit.json"))
    return matches[0] if matches else None


def _gate_deposit_stage(packet: Mapping[str, Any], leg_dir: Path, runner: Callable[[Sequence[str]], Any] | None) -> Mapping[str, Any]:
    report_path = _locate_evm_report(leg_dir)
    if report_path is None:
        raise StopError("EVM deposit report missing after stage 1")
    try:
        report = _load(report_path)
    except ConfigError as exc:
        raise StopError("EVM deposit report malformed") from exc
    if not isinstance(report, Mapping) or not report.get("deposit_tx"):
        raise StopError("EVM deposit report missing deposit_tx")
    if report.get("delta_ok") is not True:
        raise StopError("EVM deposit report delta_ok is false")
    tx_hash = str(report["deposit_tx"])
    timeout = float(packet.get("deposit_receipt_timeout_secs", 2400))
    interval = float(packet.get("deposit_receipt_poll_interval_secs", 15))
    started = time.monotonic()
    while True:
        receipt = _cast_receipt(packet, tx_hash, runner)
        if isinstance(receipt, Mapping):
            try:
                _validate_eth_finality(receipt)
                expected = packet.get("expected_receipt")
                if isinstance(expected, Mapping):
                    _validate_expected_receipt(receipt, expected)
                return report
            except (ValueError, StopError):
                pass
        if time.monotonic() - started >= timeout:
            raise StopError("EVM deposit receipt finality timeout")
        time.sleep(interval)


def _dispatch(packet: Mapping[str, Any], node: Path, leg_dir: Path, runner: Callable[[Sequence[str]], Any] | None) -> dict[str, Any]:
    leg = _leg_key(packet.get("leg"))
    if leg == "0":
        return {"commands": [], "reports": []}
    executor = packet.get("executor") if isinstance(packet.get("executor"), Mapping) else {}
    if executor.get("kind") == "phases":
        phases = executor.get("phases")
        if not isinstance(phases, list) or len(phases) != 2:
            raise ConfigError("phases executor requires two phases")
        first = phases[0]
        if not isinstance(first, Mapping) or first.get("kind") != "evm_script":
            raise ConfigError("phase 1 must be evm_script")
        report_ref = str(first.get("report", "{artifact_dir}/burn-report.json")).replace("{artifact_dir}", str(leg_dir))
        report_path = Path(report_ref)
        if report_path.exists():
            existing = _load(report_path)
            if not isinstance(existing, Mapping):
                raise StopError("existing phase-1 report is malformed")
            _phase_receipt_gate(packet, [existing], runner)
            first_result = {"commands": [], "outputs": [], "reports": [existing]}
        else:
            first_packet = dict(packet)
            first_packet["executor"] = first
            first_result = _submit_and_collect(_command_from_packet(first_packet, node, leg_dir), runner, leg_dir, "evm")
        first_reports = list(first_result.get("reports", [])) + [x for x in first_result.get("outputs", []) if isinstance(x, Mapping)]
        _phase_receipt_gate(packet, first_reports, runner)
        burn_hash = next((r.get("tx_hash") or r.get("transaction_hash") for r in first_reports if r.get("tx_hash") or r.get("transaction_hash")), None)
        if not burn_hash:
            raise StopError("phase 1 report omitted tx_hash")
        second_packet = json.loads(json.dumps(packet))
        def inject(value: Any) -> None:
            if isinstance(value, dict):
                for key, item in list(value.items()):
                    if item == "FROM-PHASE-1-RECEIPT":
                        value[key] = burn_hash
                    else:
                        inject(item)
            elif isinstance(value, list):
                for item in value:
                    inject(item)
        inject(second_packet.get("ops_file_template"))
        second_dir = leg_dir
        ops_path, tag, _ = _render_ops(second_packet, second_dir)
        second_cmd = [[str(node), "pftl-submit-certified-asset-ops", "--ops-file", str(ops_path), "--artifact-dir", str(second_dir)]]
        second_result = _submit_and_collect(second_cmd, runner, second_dir, tag)
        return {"commands": first_result.get("commands", []) + second_result.get("commands", []), "outputs": first_result.get("outputs", []) + second_result.get("outputs", []), "reports": first_result.get("reports", []) + second_result.get("reports", []), "kind": "phases"}
    if leg == "1":
        commands = _normalize_stage(_command_from_packet(packet, node, leg_dir), node)
        if len(commands) < 2:
            raise ConfigError("leg 1 requires deposit and relay commands")
        report = _locate_evm_report(leg_dir)
        first_result = {"commands": [], "outputs": [], "reports": []}
        if report is None:
            first_result = _submit_and_collect(commands[:1], runner, leg_dir, "stage")
        deposit_report = _gate_deposit_stage(packet, leg_dir, runner)
        if len(commands) == 2:
            second_result = _submit_and_collect(commands[1:], runner, leg_dir, "stage")
            return {"commands": first_result.get("commands", []) + second_result.get("commands", []), "outputs": first_result.get("outputs", []) + second_result.get("outputs", []), "reports": first_result.get("reports", []) + second_result.get("reports", []) + [deposit_report], "kind": "stage"}
        proof_result = _submit_and_collect(commands[1:2], runner, leg_dir, "prover")
        proof_path = next(iter(sorted(leg_dir.glob("**/proof-report.json"))), None)
        if proof_path is None:
            raise StopError("ingress prover report missing after proof stage")
        proof_report = _load(proof_path)
        if not isinstance(proof_report, Mapping):
            raise StopError("ingress prover report malformed")
        expected_hashes = packet.get("expected_proof_hashes", {})
        for key, value in expected_hashes.items():
            if value and proof_report.get(key) != value:
                raise StopError(f"ingress proof hash mismatch: {key}")
        relay_result = _submit_and_collect(commands[2:], runner, leg_dir, "stage")
        return {"commands": first_result.get("commands", []) + proof_result.get("commands", []) + relay_result.get("commands", []), "outputs": first_result.get("outputs", []) + proof_result.get("outputs", []) + relay_result.get("outputs", []), "reports": first_result.get("reports", []) + proof_result.get("reports", []) + relay_result.get("reports", []) + [deposit_report, proof_report], "kind": "stage"}
    elif leg in CERTIFIED:
        ops_path, tag, _ = _render_ops(packet, leg_dir)
        commands = [[str(node), "pftl-submit-certified-asset-ops", "--ops-file", str(ops_path), "--artifact-dir", str(leg_dir)]]
        if packet.get("resume"):
            commands[-1].append("--resume")
        return _submit_and_collect(commands, runner, leg_dir, tag)
    elif leg == "5b":
        commands = _normalize_stage(_command_from_packet(packet, node, leg_dir), node)
    else:
        commands = _command_from_packet(packet, node, leg_dir)
    return _submit_and_collect(commands, runner, leg_dir, "stage")


def _submit_and_collect(commands: list[list[str]], runner: Callable[[Sequence[str]], Any] | None, leg_dir: Path, kind: str) -> dict[str, Any]:
    outputs = []
    for argv in commands:
        outputs.append(_call(argv, runner))
    reports = []
    for path in sorted(leg_dir.glob("*.json")):
        if path.name == "ops.json":
            continue
        try:
            value = _load(path)
        except ConfigError:
            continue
        if isinstance(value, Mapping):
            reports.append(value)
    return {"commands": commands, "outputs": outputs, "reports": reports, "kind": kind}


def _tx_ids(result: Mapping[str, Any]) -> list[str]:
    ids: list[str] = []
    def visit(value: Any) -> None:
        if isinstance(value, Mapping):
            for key, item in value.items():
                if key in ("tx_id", "tx_ids", "transaction_hash", "transaction_hashes"):
                    if isinstance(item, list):
                        ids.extend(str(x) for x in item)
                    elif item:
                        ids.append(str(item))
                else:
                    visit(item)
        elif isinstance(value, list):
            for item in value:
                visit(item)
    visit(result)
    return list(dict.fromkeys(ids))


def _validate_expected_receipt(receipt: Mapping[str, Any], expected: Mapping[str, Any]) -> None:
    comparisons = (("from", "from"), ("to", "to"), ("value_wei", "value"))
    for expected_key, receipt_key in comparisons:
        if expected_key not in expected:
            continue
        actual = receipt.get(receipt_key, receipt.get(expected_key))
        if expected_key == "value_wei" and actual is not None:
            try:
                actual = int(actual, 16) if isinstance(actual, str) and actual.startswith("0x") else int(actual)
            except (TypeError, ValueError) as exc:
                raise StopError(f"Ethereum receipt {expected_key} is malformed") from exc
        if str(actual).lower() != str(expected[expected_key]).lower():
            raise StopError(f"Ethereum receipt {expected_key} mismatch")


def _receipt_gate(tx_ids: list[str], packet: Mapping[str, Any], node: Path, runner: Callable[[Sequence[str]], Any] | None, pre: Mapping[str, Any], dispatch: Mapping[str, Any], leg_dir: Path) -> dict[str, Any]:
    finals = []
    endpoints = _status_endpoints(packet, {})
    for tx_id in tx_ids:
        responses = []
        for endpoint in endpoints:
            value = _rpc_call(endpoint, "receipts", {"tx_id": tx_id}, runner, node)
            payload = value.get("result") if isinstance(value, Mapping) and isinstance(value.get("result"), Mapping) else value
            if not isinstance(payload, Mapping):
                raise StopError("receipt response malformed")
            responses.append(payload)
        first = responses[0]
        if any(json.dumps(r, sort_keys=True) != json.dumps(first, sort_keys=True) for r in responses[1:]):
            raise StopError("receipt disagreement across fleet")
        expected = {"height": _field(first, "height", "finalized_height"), "state_root": _field(first, "state_root", "stateRoot", "root")}
        if first.get("accepted") is not True or expected["height"] is None or expected["state_root"] is None:
            raise StopError("receipt is not finalized")
        finals.append({"tx_id": tx_id, "receipt_id": first.get("receipt_id"), "accepted": True, "height": expected["height"], "state_root": expected["state_root"], "batch_id": first.get("batch_id"), "batch_payload_hash": first.get("batch_payload_hash")})
        _rpc_call(endpoints[0], "tx", {"tx_id": tx_id, "audit_block_log": True}, runner, node)
        _rpc_call(endpoints[0], "batch_archive", {"tx_id": tx_id}, runner, node)
    evm_hashes = []
    expected_receipt = packet.get("expected_receipt") if isinstance(packet.get("expected_receipt"), Mapping) else {}
    for report in dispatch.get("reports", []):
        if expected_receipt and any(k in report for k in ("from", "to", "value_wei", "value")):
            _validate_expected_receipt(report, expected_receipt)
        for key in ("ethereum_tx_hash", "tx_hash", "transaction_hash"):
            if report.get(key):
                evm_hashes.append(str(report[key]))
    evm_receipts: list[Mapping[str, Any]] = []
    for tx_hash in evm_hashes:
        receipt = _cast_receipt(packet, tx_hash, runner)
        _validate_eth_finality(receipt)
        if expected_receipt:
            _validate_expected_receipt(receipt, expected_receipt)
        evm_receipts.append(receipt)
    if not tx_ids and not evm_hashes and not dispatch.get("reports") and packet.get("leg") not in (0, "0"):
        raise StopError("no receipt or stage report emitted")
    for report in dispatch.get("reports", []):
        if report.get("accepted") is False or report.get("status") in ("FAILED", "ERROR"):
            raise StopError("stage report indicates failure")
    return {"finality": finals, "ethereum_tx_hashes": evm_hashes, "evm_receipts": evm_receipts, "tx_ids": tx_ids}


def _actual_cost(packet: Mapping[str, Any], finality: Mapping[str, Any]) -> str:
    budget = packet.get("budget_guard") if isinstance(packet.get("budget_guard"), Mapping) else {}
    eth_usd = budget.get("eth_usd", packet.get("eth_usd"))
    receipts = finality.get("evm_receipts", [])
    if receipts and eth_usd is None:
        raise StopError("EVM receipt cost requires packet-pinned eth_usd")
    total = Decimal(str(budget.get("fee_usdc", "0"))) if budget else Decimal("0")
    if eth_usd is not None:
        for receipt in receipts:
            try:
                gas_raw = receipt.get("gasUsed", receipt.get("gas_used"))
                price_raw = receipt.get("effectiveGasPrice", receipt.get("effective_gas_price"))
                gas = Decimal(str(int(gas_raw, 16) if isinstance(gas_raw, str) and gas_raw.startswith("0x") else gas_raw))
                price = Decimal(str(int(price_raw, 16) if isinstance(price_raw, str) and price_raw.startswith("0x") else price_raw))
                total += gas * price * Decimal(str(eth_usd)) / Decimal("1e18")
            except (TypeError, ValueError, InvalidOperation) as exc:
                raise StopError("EVM receipt gas fields malformed") from exc
    return format(total, "f")


def _append_journal(artifact_dir: Path, packet: Mapping[str, Any], pre: Mapping[str, Any], post: Mapping[str, Any], submission: Mapping[str, Any], finality: Mapping[str, Any]) -> None:
    journal = _read_journal(artifact_dir)
    incoming = {"receipt_ids": [x.get("receipt_id") for x in finality.get("finality", []) if x.get("receipt_id")]}
    _validate_replay(journal, incoming)
    leg = packet.get("leg")
    if any(str(x.get("leg")) == str(leg) for x in journal.get("legs", []) if isinstance(x, Mapping)):
        raise StopError("leg already finalized in journal")
    delta = packet.get("delta_assertions", {})
    if isinstance(delta, Mapping) and {"before_atoms", "after_atoms", "amount_atoms"} <= set(delta):
        _validate_delta(delta["before_atoms"], delta["after_atoms"], delta["amount_atoms"])
    entry = {"leg": leg, "name": packet.get("leg_name", str(leg)), "source_rpc_url": packet.get("source_rpc_url") or packet.get("ethereum_rpc_url"), "pre_state_root": pre.get("state_root"), "pre_height": pre.get("height"), "submission": {"ops_file": submission.get("ops_file"), "artifact_dir": str(artifact_dir), "tx_ids": finality.get("tx_ids", []), "ethereum_tx_hashes": finality.get("ethereum_tx_hashes", [])}, "finality": finality.get("finality", []), "post_state_root": post.get("state_root"), "post_height": post.get("height"), "delta_assertions": delta, "actual_cost_usdc": _actual_cost(packet, finality), "status": "FINALIZED"}
    journal["chain_id"] = packet.get("chain_id", journal.get("chain_id"))
    journal["genesis_hash"] = packet.get("genesis_hash", pre.get("genesis_hash", journal.get("genesis_hash")))
    journal["route_id"] = packet.get("route", packet.get("route_id", journal.get("route_id")))
    campaign_nonce = str(packet.get("campaign_nonce", packet.get("packet_id", "campaign")))
    journal["campaign_id"] = hashlib.sha256((str(journal.get("route_id", "")) + "|" + campaign_nonce).encode()).hexdigest()
    journal.setdefault("legs", []).append(entry)
    _write_json(_journal_path(artifact_dir), journal)


def _verify_entry(entry: Mapping[str, Any], node: Path, runner: Callable[[Sequence[str]], Any] | None, endpoints: list[str]) -> None:
    for final in entry.get("finality", []):
        tx_id = final.get("tx_id")
        if not tx_id:
            raise StopError("journal finality entry has no tx id")
        responses = []
        for endpoint in endpoints:
            value = _rpc_call(endpoint, "receipts", {"tx_id": str(tx_id)}, runner, node)
            payload = value.get("result") if isinstance(value, Mapping) and isinstance(value.get("result"), Mapping) else value
            if not isinstance(payload, Mapping):
                raise StopError("journal receipt response malformed")
            responses.append(payload)
        for receipt in responses:
            try:
                _validate_finality(receipt, final)
            except ValueError as exc:
                raise StopError(f"journal receipt mismatch: {exc}") from exc
    for tx_hash in entry.get("submission", {}).get("ethereum_tx_hashes", []):
        _validate_eth_finality(_cast_receipt(entry, str(tx_hash), runner))


def verify_journal(artifact_dir: Path, runner: Callable[[Sequence[str]], Any] | None = None, node: Path | None = None) -> bool:
    node = node or _binary_path()
    journal = _read_journal(artifact_dir)
    endpoints = FLEET_ENDPOINTS
    for entry in journal.get("legs", []):
        _verify_entry(entry, node, runner, endpoints)
    return True


def _recover_submitted(artifact_dir: Path, journal: dict[str, Any], packet: Mapping[str, Any], node: Path, runner: Callable[[Sequence[str]], Any] | None) -> set[str]:
    recovered: set[str] = set()
    for entry in journal.get("legs", []):
        if not isinstance(entry, Mapping) or entry.get("status") != "SUBMITTED" or entry.get("finality"):
            continue
        tx_ids = entry.get("submission", {}).get("tx_ids", []) if isinstance(entry.get("submission"), Mapping) else []
        if not tx_ids:
            raise StopError("submitted leg has no receipt transaction ids")
        finality = _receipt_gate([str(x) for x in tx_ids], packet, node, runner, {}, {"reports": []}, artifact_dir)
        if not finality.get("finality"):
            raise StopError("submitted leg has no finalized receipts")
        entry["finality"] = finality["finality"]
        entry["status"] = "FINALIZED"
        entry["post_state_root"] = finality["finality"][0].get("state_root")
        entry["post_height"] = finality["finality"][0].get("height")
        recovered.add(str(entry.get("leg")))
    if recovered:
        _write_json(_journal_path(artifact_dir), journal)
    return recovered


def run_leg(packet_path: Path, binding_path: Path, artifact_dir: Path, resume: bool = False, runner: Callable[[Sequence[str]], Any] | None = None, node: Path | None = None) -> dict[str, Any]:
    node = node or check_binary()
    packet = _load(packet_path)
    binding = _load(binding_path)
    if not isinstance(packet, Mapping):
        raise ConfigError("packet must be an object")
    _verify_binding(packet_path, packet, binding)
    artifact_dir.mkdir(parents=True, exist_ok=True)
    journal = _read_journal(artifact_dir)
    requested_leg = str(packet.get("leg"))
    recovered_legs: set[str] = set()
    if resume:
        recovered_legs = _recover_submitted(artifact_dir, journal, packet, node, runner)
        journal = _read_journal(artifact_dir)
        verify_journal(artifact_dir, runner, node)
        if requested_leg in recovered_legs:
            raise StopError("crashed leg finalized from receipts; refusing duplicate submission")
    if not journal.get("legs"):
        if requested_leg not in ("0", "1"):
            raise StopError("requested leg is not first unjournaled leg")
    elif not _first_unjournaled(journal, requested_leg):
        raise StopError("requested leg is not first unjournaled leg")
    _budget_guard(packet, journal)
    pre = _preflight(packet, binding, node, runner)
    leg_dir = artifact_dir / f"leg-{_leg_key(packet.get('leg'))}"
    leg_dir.mkdir(parents=True, exist_ok=True)
    dispatch = _dispatch(packet, node, leg_dir, runner)
    tx_ids = _tx_ids(dispatch)
    finality = _receipt_gate(tx_ids, packet, node, runner, pre, dispatch, leg_dir)
    post = _preflight(packet, binding, node, runner)
    _append_journal(artifact_dir, packet, pre, post, dispatch, finality)
    return {"status": "FINALIZED", "leg": packet.get("leg"), "tx_ids": tx_ids, "post": post}


def _packet_path_from_entry(entry: Mapping[str, Any], binding_path: Path) -> Path:
    value = entry.get("path") or entry.get("packet_path")
    if not value:
        raise ConfigError("binding packet entry has no path")
    path = Path(str(value))
    if not path.is_absolute():
        path = binding_path.parent / path
        if not path.exists():
            path = Path.cwd() / str(value)
    return path


def _walk_strings(value: Any, prefix: str = "") -> Iterable[tuple[str, str]]:
    if isinstance(value, Mapping):
        for key, item in value.items():
            yield from _walk_strings(item, f"{prefix}/{key}")
    elif isinstance(value, list):
        for idx, item in enumerate(value):
            yield from _walk_strings(item, f"{prefix}/{idx}")
    elif isinstance(value, str):
        yield prefix, value


def _staged_entries(binding: Any) -> list[dict[str, Any]]:
    raw = binding.get("staged_fields", []) if isinstance(binding, Mapping) else []
    out: list[dict[str, Any]] = []
    if isinstance(raw, Mapping):
        for pointer, value in raw.items():
            if isinstance(value, Mapping):
                item = dict(value); item.setdefault("json_pointer", pointer)
            else:
                item = {"json_pointer": pointer, "source": value}
            out.append(item)
    elif isinstance(raw, list):
        out = [dict(x) for x in raw if isinstance(x, Mapping)]
    return out


def _pointer_value(root: Any, pointer: str) -> Any:
    value = root
    for part in pointer.split("/")[1:]:
        part = part.replace("~1", "/").replace("~0", "~")
        if isinstance(value, Mapping): value = value.get(part)
        elif isinstance(value, list): value = value[int(part)] if int(part) < len(value) else None
        else: return None
    return value


def validate_executable(binding_path: Path, through: str) -> list[str]:
    binding = _load(binding_path)
    entries = _binding_entries(binding)
    target = _leg_order(through)
    staged = _staged_entries(binding)
    lines: list[str] = []
    for entry in entries:
        packet_path = _packet_path_from_entry(entry, binding_path)
        packet = _load(packet_path)
        if not isinstance(packet, Mapping) or _leg_order(packet.get("leg")) > target:
            continue
        leg = str(packet.get("leg"))
        executor = packet.get("executor") if isinstance(packet.get("executor"), Mapping) else {}
        kind = executor.get("kind", "stage_sequence" if leg in ("1", "5b") else ("certified_ops" if leg in CERTIFIED else "evm_script"))
        if kind not in ("read_only", "stage_sequence", "evm_script", "certified_ops", "phases"):
            raise ConfigError(f"LEG {leg}: unsupported executor kind {kind}")
        def scan_pending(surface: Any, base_pointer: str) -> None:
            for raw_pointer, value in _walk_strings(surface):
                pointer = base_pointer + raw_pointer
                if "PENDING" not in value:
                    continue
                candidates = [
                    x for x in staged
                    if x.get("packet") in (None, packet_path.name, str(packet_path))
                    and x.get("json_pointer") == pointer
                ]
                valid = [
                    x for x in candidates
                    if isinstance(x.get("source"), str) and x.get("source")
                    and isinstance(x.get("stage"), str) and x.get("stage")
                ]
                if not valid:
                    raise ConfigError(f"LEG {leg}: unresolved executable field {pointer}: {value}")
                chosen = valid[0]
                actual_value = _pointer_value(packet, pointer)
                if "PENDING" not in str(actual_value):
                    raise ConfigError(f"LEG {leg}: staged exemption points to resolved field {pointer}")
                lines.append(f"STAGED-EXEMPT {leg} {pointer} <- {chosen['source']} @{chosen['stage']}")

        surfaces = executor
        if kind == "phases":
            # Preserve the packet's real /executor/phases/... pointer shape.
            surfaces = {"phases": executor.get("phases", [])}
        scan_pending(surfaces, "/executor")
        if kind == "certified_ops":
            # Certified templates are executable inputs too; exemptions must
            # use their real /ops_file_template/... packet pointers.
            scan_pending(packet.get("ops_file_template", {}), "/ops_file_template")
        if kind == "phases" and not executor.get("phases"):
            raise ConfigError(f"LEG {leg}: phases has no commands")
        if kind in ("stage_sequence", "evm_script") and not (executor.get("commands") or executor.get("args") or executor.get("command")):
            raise ConfigError(f"LEG {leg}: executable argv commands absent")
        if kind == "phases":
            for phase in executor.get("phases", []):
                if phase.get("kind") == "evm_script" and not (phase.get("commands") or phase.get("command")):
                    raise ConfigError(f"LEG {leg}: phase command absent")
        if kind == "certified_ops":
            try:
                _operation_from_packet(packet, leg)
            except ConfigError as exc:
                # _operation_from_packet names the unresolved field rather
                # than echoing its value. scan_pending above already enforced
                # a complete exemption for every actual PENDING pointer.
                if "PENDING" not in str(exc) and "unresolved fire-time field" not in str(exc):
                    raise ConfigError(f"LEG {leg}: {exc}")
        lines.append(f"LEG {leg} EXECUTABLE {kind} commands {len(executor.get('commands', [])) if isinstance(executor.get('commands'), list) else 0} staged-exempt")
    if not lines:
        raise ConfigError("no packet entries through requested leg")
    return lines


def _cli_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="native_campaign_driver")
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run-leg")
    run.add_argument("--packet", required=True)
    run.add_argument("--binding", required=True)
    run.add_argument("--artifact-dir", required=True)
    run.add_argument("--resume", action="store_true")
    status = sub.add_parser("status")
    status.add_argument("--artifact-dir", required=True)
    verify = sub.add_parser("verify-journal")
    verify.add_argument("--artifact-dir", required=True)
    lint = sub.add_parser("validate-executable")
    lint.add_argument("--binding", required=True)
    lint.add_argument("--through", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _cli_parser()
    try:
        args = parser.parse_args(argv)
        if args.command == "run-leg":
            result = run_leg(Path(args.packet), Path(args.binding), Path(args.artifact_dir), args.resume)
            print(json.dumps(result, sort_keys=True))
        elif args.command == "validate-executable":
            for line in validate_executable(Path(args.binding), args.through):
                print(line)
        elif args.command == "status":
            check_binary()
            journal = _read_journal(Path(args.artifact_dir))
            print(json.dumps(journal, sort_keys=True))
        else:
            check_binary()
            verify_journal(Path(args.artifact_dir))
            print("VERIFIED")
        return 0
    except StopError as exc:
        print(f"STOP-no-retry: {exc}", file=sys.stderr)
        return 2
    except (ConfigError, OSError, ValueError) as exc:
        print(f"CONFIG: {exc}", file=sys.stderr)
        return 3


if __name__ == "__main__":
    raise SystemExit(main())
