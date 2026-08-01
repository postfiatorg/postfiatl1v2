#!/usr/bin/env python3
"""Durable controlled pNOK private-FIX execution and recovery driver.

The driver deliberately separates private run state from redacted public
evidence. It is idempotent by intent ID, an Asset-Orchard request ID, a
consensus wallet-intent hash, and an action-bound reservation ID.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
from typing import Any
from urllib.error import HTTPError
from urllib.parse import urljoin
from urllib.request import Request, urlopen


SCHEMA = "postfiat-pnok-private-fix-demo-intent-v1"
PUBLIC_SCHEMA = "postfiat-pnok-private-fix-demo-public-status-v1"
EXPECTED_CHAIN_ID = "postfiat-wan-devnet-2"
EXPECTED_GENESIS_HASH = (
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
    "21aff3ed334da07e150a7233a3e90a9"
)
DEFAULT_PFUSDC_ASSET_ID = (
    "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c2"
    "33f6830bd5221fe2717fb6a1a7005d7b"
)
STAGES = (
    "created",
    "quote_verified",
    "action_built",
    "reservation_finalized",
    "batch_built",
    "submitted",
    "finalized",
    "local_finalized",
    "complete",
)
RESPONSE_CAP = 32 * 1024 * 1024


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def domain_hash_384(domain: str, payload: bytes) -> str:
    digest = hashlib.sha3_384()
    digest.update(domain.encode())
    digest.update(b"\0")
    digest.update(payload)
    return digest.hexdigest()


def domain_hash_256(domain: str, payload: bytes) -> str:
    digest = hashlib.sha3_256()
    digest.update(domain.encode())
    digest.update(b"\0")
    digest.update(payload)
    return digest.hexdigest()


def validate_hex(field: str, value: str, length: int) -> str:
    if not isinstance(value, str) or not re.fullmatch(rf"[0-9a-f]{{{length}}}", value):
        raise RuntimeError(f"{field} must be {length} lowercase hex characters")
    return value


def validate_address(field: str, value: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"pf[0-9a-f]{40}", value):
        raise RuntimeError(f"{field} must be a canonical PostFiat address")
    return value


def validate_intent_id(value: str) -> str:
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", value or ""):
        raise RuntimeError("intent ID must match [a-z0-9][a-z0-9-]{0,63}")
    return value


def atomic_write_json(path: Path, value: Any, mode: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(path.parent, 0o700)
    temporary = path.with_name(f".{path.name}.tmp.{os.getpid()}")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    descriptor = os.open(temporary, flags, mode)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        os.chmod(path, mode)
    except BaseException:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass
        raise


def load_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def load_rpc_helpers(script_dir: Path) -> Any:
    path = script_dir / "a666-ce22-finality-op.py"
    spec = importlib.util.spec_from_file_location("pnok_fix_rpc_helpers", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import RPC helpers from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def service_json(
    service_url: str,
    method: str,
    path: str,
    body: Any | None,
    timeout_seconds: float,
) -> dict[str, Any]:
    payload = None if body is None else canonical_json(body).encode()
    request = Request(
        urljoin(service_url.rstrip("/") + "/", path.lstrip("/")),
        data=payload,
        method=method,
        headers={"Accept": "application/json", "Content-Type": "application/json"},
    )
    try:
        with urlopen(request, timeout=timeout_seconds) as response:
            raw = response.read(RESPONSE_CAP + 1)
    except HTTPError as error:
        raw = error.read(RESPONSE_CAP + 1)
        try:
            detail = json.loads(raw)
        except json.JSONDecodeError:
            detail = {"error": raw.decode(errors="replace")[:500]}
        raise RuntimeError(f"resident service {path} failed with HTTP {error.code}: {detail}") from error
    if len(raw) > RESPONSE_CAP:
        raise RuntimeError(f"resident service {path} response exceeded byte cap")
    response = json.loads(raw)
    if not isinstance(response, dict) or response.get("ok") is not True:
        raise RuntimeError(f"resident service {path} rejected request: {response}")
    return response


def fleet_rpc_identical(
    rpc: Any,
    ports: list[int],
    method: str,
    params: dict[str, Any],
    timeout_seconds: float,
) -> dict[str, Any]:
    def one(port: int) -> dict[str, Any]:
        request = rpc.request(f"pnok-{method}-{port}", method, params)
        response = rpc.rpc_call(port, request, timeout_seconds)
        if response.get("ok") is not True:
            raise RuntimeError(f"{method} failed on port {port}: {response.get('error')}")
        result = response.get("result")
        if not isinstance(result, dict):
            raise RuntimeError(f"{method} on port {port} returned a non-object result")
        return result

    with ThreadPoolExecutor(max_workers=len(ports)) as executor:
        results = list(executor.map(one, ports))
    encodings = {canonical_json(result) for result in results}
    if len(encodings) != 1:
        raise RuntimeError(f"six-validator {method} results disagree")
    return results[0]


def reservation_id(operation: dict[str, Any]) -> str:
    preimage = (
        f"fix_packet_hash={operation['fix_packet_hash']}\n"
        f"operator={operation['operator']}\n"
        f"action_binding_hash={operation['action_binding_hash']}\n"
        f"base_atoms={operation['base_atoms']}\n"
        f"quote_atoms={operation['quote_atoms']}\n"
        f"wallet_intent_hash={operation['wallet_intent_hash']}\n"
        f"reservation_nonce={operation['reservation_nonce']}\n"
    )
    return domain_hash_384("postfiat.fx_fix.reservation_id.v1", preimage.encode())


def stage_at_least(state: dict[str, Any], stage: str) -> bool:
    return STAGES.index(state["stage"]) >= STAGES.index(stage)


def immutable_config(args: argparse.Namespace) -> dict[str, Any]:
    wallet_input_note_path = args.wallet_input_note_path
    liquidity_input_note_path = args.liquidity_input_note_path
    if (wallet_input_note_path is None) != (liquidity_input_note_path is None):
        raise RuntimeError(
            "--wallet-input-note-path and --liquidity-input-note-path must be provided together"
        )
    return {
        "chain_id": EXPECTED_CHAIN_ID,
        "genesis_hash": EXPECTED_GENESIS_HASH,
        "intent_id": validate_intent_id(args.intent_id),
        "wallet_address": validate_address("wallet_address", args.wallet_address),
        "facility_operator": validate_address("facility_operator", args.facility_operator),
        "facility_key_file": str(args.facility_key_file.resolve()),
        "base_asset_id": validate_hex("base_asset_id", args.base_asset_id, 96),
        "quote_asset_id": validate_hex("quote_asset_id", args.quote_asset_id, 96),
        "base_atoms": args.base_atoms,
        "expected_quote_atoms": args.expected_quote_atoms,
        "wallet_note_commitment": validate_hex(
            "wallet_note_commitment", args.wallet_note_commitment, 64
        ),
        "liquidity_commitment": validate_hex(
            "liquidity_commitment", args.liquidity_commitment, 64
        ),
        # These are paths on the resident service host. They are optional
        # because notes created by the service are selected from its vault by
        # commitment. Imported notes use the service's existing strict
        # schema/asset/value/commitment validation instead.
        "wallet_input_note_path": wallet_input_note_path,
        "liquidity_input_note_path": liquidity_input_note_path,
        "fix_packet_hash": (
            validate_hex("fix_packet_hash", args.fix_packet_hash, 96)
            if args.fix_packet_hash
            else None
        ),
        "expected_source_label": args.expected_source_label,
        "expected_ratio_numerator": args.expected_ratio_numerator,
        "expected_ratio_denominator": args.expected_ratio_denominator,
        "min_expiry_blocks": args.min_expiry_blocks,
    }


def initialize_or_load_state(args: argparse.Namespace) -> tuple[Path, dict[str, Any]]:
    root = args.intent_dir.resolve()
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(root, 0o700)
    private = root / "private"
    public = root / "public"
    private.mkdir(exist_ok=True, mode=0o700)
    public.mkdir(exist_ok=True, mode=0o700)
    os.chmod(private, 0o700)
    os.chmod(public, 0o700)
    path = private / "intent.json"
    immutable = immutable_config(args)
    if path.exists():
        state = load_json(path)
        if state.get("schema") != SCHEMA or state.get("immutable") != immutable:
            raise RuntimeError("existing intent is not byte-for-byte bound to these immutable inputs")
        if state.get("stage") not in STAGES:
            raise RuntimeError("existing intent has an invalid stage")
        return root, state
    if args.command != "run":
        raise RuntimeError("intent does not exist; run it before status or abort")
    state = {
        "schema": SCHEMA,
        "stage": "created",
        "immutable": immutable,
        "derived": {},
        "artifacts": {},
        "attempts": {"reservation": 0, "submission": 0, "replay": 0},
        "last_error": None,
        "created_at_unix_ms": int(time.time() * 1000),
        "updated_at_unix_ms": int(time.time() * 1000),
    }
    atomic_write_json(path, state, 0o600)
    return root, state


def persist_state(root: Path, state: dict[str, Any], stage: str | None = None) -> None:
    if stage is not None:
        if STAGES.index(stage) < STAGES.index(state["stage"]):
            raise RuntimeError("durable intent stage cannot move backwards")
        state["stage"] = stage
    state["updated_at_unix_ms"] = int(time.time() * 1000)
    state["last_error"] = None
    atomic_write_json(root / "private/intent.json", state, 0o600)
    publish_redacted_status(root, state)


def persist_progress(root: Path, state: dict[str, Any], stage: str) -> None:
    """Persist recovered work without ever regressing the durable stage."""
    if STAGES.index(stage) > STAGES.index(state["stage"]):
        persist_state(root, state, stage)
    else:
        persist_state(root, state)


def publish_redacted_status(root: Path, state: dict[str, Any]) -> None:
    derived = state.get("derived", {})
    public = {
        "schema": PUBLIC_SCHEMA,
        "intent_id": state["immutable"]["intent_id"],
        "stage": state["stage"],
        "chain_id": state["immutable"]["chain_id"],
        "genesis_hash": state["immutable"]["genesis_hash"],
        "fix_packet_hash": derived.get("fix_packet_hash"),
        "fix_source_label": derived.get("fix_source_label"),
        "reservation_id": derived.get("reservation_id"),
        "action_binding_hash": derived.get("action_binding_hash"),
        "nullifier_occurrence_counts": derived.get("nullifier_occurrence_counts"),
        "output_occurrence_counts": derived.get("output_occurrence_counts"),
        "supply_unchanged": derived.get("supply_unchanged"),
        "replay_rejected_without_effect": derived.get("replay_rejected_without_effect"),
        "trust_class": "CONTROLLED",
        "execution_privacy": "private_on_pftl",
        "source_boundary": "controlled_sandbox_checkpoint",
        "updated_at_unix_ms": state.get("updated_at_unix_ms"),
    }
    atomic_write_json(root / "public/status.json", public, 0o644)


def record_failure(root: Path, state: dict[str, Any], error: BaseException) -> None:
    state["last_error"] = {
        "type": type(error).__name__,
        "message": str(error)[:2000],
        "at_unix_ms": int(time.time() * 1000),
    }
    state["updated_at_unix_ms"] = int(time.time() * 1000)
    atomic_write_json(root / "private/intent.json", state, 0o600)
    publish_redacted_status(root, state)


def note_by_commitment(notes: list[Any], commitment: str) -> dict[str, Any]:
    matches = [note for note in notes if isinstance(note, dict) and note.get("id") == commitment]
    if len(matches) != 1:
        raise RuntimeError(f"resident note commitment {commitment} did not resolve exactly once")
    return matches[0]


def require_note(
    note: dict[str, Any], *, owner: str, asset_id: str, amount_atoms: int, state: str
) -> None:
    expected = {
        "wallet_address": owner,
        "asset_id": asset_id,
        "amount_atoms": amount_atoms,
        "state": state,
    }
    observed = {key: note.get(key) for key in expected}
    if observed != expected:
        raise RuntimeError(f"resident note does not match exact expected owner/asset/amount/state: {observed}")


def verify_quote(
    args: argparse.Namespace,
    rpc: Any,
    ports: list[int],
    immutable: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    rpc.wait_for_fleet_status(ports, args.rpc_timeout_seconds, args.convergence_timeout_seconds)
    fix_hash = immutable.get("fix_packet_hash")
    if fix_hash:
        info = fleet_rpc_identical(
            rpc, ports, "fx_fix_info", {"fix_packet_hash": fix_hash}, args.rpc_timeout_seconds
        )
        if info.get("found") is not True or not isinstance(info.get("fix"), dict):
            raise RuntimeError("configured FX fix is not registered")
        row = info["fix"]
    else:
        listing = fleet_rpc_identical(
            rpc,
            ports,
            "fx_fix_list",
            {
                "base_asset_id": immutable["base_asset_id"],
                "quote_asset_id": immutable["quote_asset_id"],
                "active_only": True,
                "limit": 2,
            },
            args.rpc_timeout_seconds,
        )
        fixes = listing.get("fixes")
        if not isinstance(fixes, list) or len(fixes) != 1:
            raise RuntimeError("asset pair must resolve to exactly one active FX fix")
        row = fixes[0]
        fix_hash = row.get("state", {}).get("packet", {}).get("packet_hash")
    validate_hex("discovered fix_packet_hash", fix_hash, 96)
    packet = row.get("state", {}).get("packet")
    if not isinstance(packet, dict):
        raise RuntimeError("FX fix query omitted packet state")
    required_packet = {
        "packet_hash": fix_hash,
        "operator": immutable["facility_operator"],
        "base_asset_id": immutable["base_asset_id"],
        "quote_asset_id": immutable["quote_asset_id"],
        "ratio_numerator": immutable["expected_ratio_numerator"],
        "ratio_denominator": immutable["expected_ratio_denominator"],
        "band_bps": 0,
        "fee_bps": 0,
        "source_label": immutable["expected_source_label"],
    }
    for field, expected in required_packet.items():
        if packet.get(field) != expected:
            raise RuntimeError(f"FX fix {field} mismatch: expected {expected!r}, got {packet.get(field)!r}")
    if row.get("status") != "active" or int(row.get("remaining_fill_slots", 0)) < 1:
        raise RuntimeError("FX fix has no active fill slot")
    quote = fleet_rpc_identical(
        rpc,
        ports,
        "fx_fix_quote",
        {"fix_packet_hash": fix_hash, "base_atoms": str(immutable["base_atoms"])},
        args.rpc_timeout_seconds,
    )
    quote_expectations = {
        "chain_id": EXPECTED_CHAIN_ID,
        "genesis_hash": EXPECTED_GENESIS_HASH,
        "fix_packet_hash": fix_hash,
        "source_label": immutable["expected_source_label"],
        "base_atoms": immutable["base_atoms"],
        "quote_atoms": immutable["expected_quote_atoms"],
        "exact_division": True,
        "fee_atoms": 0,
        "price_impact_bps": 0,
    }
    for field, expected in quote_expectations.items():
        if quote.get(field) != expected:
            raise RuntimeError(f"FX quote {field} mismatch: expected {expected!r}, got {quote.get(field)!r}")
    expiry = int(packet.get("expires_at_height", 0))
    current_height = int(quote.get("current_height", 0))
    if expiry - current_height < immutable["min_expiry_blocks"]:
        raise RuntimeError("FX fix does not have enough remaining PFTL-height validity")
    if quote.get("base_asset", {}).get("asset_id") != immutable["base_asset_id"]:
        raise RuntimeError("FX quote base asset mismatch")
    if quote.get("quote_asset", {}).get("asset_id") != immutable["quote_asset_id"]:
        raise RuntimeError("FX quote quote asset mismatch")
    if quote.get("base_asset", {}).get("precision") != 6:
        raise RuntimeError("pfUSDC precision must be six")
    if quote.get("quote_asset", {}).get("precision") != 0:
        raise RuntimeError("pNOK precision must be zero")
    return row, quote


def recover_bound_quote(
    args: argparse,
    rpc: Any,
    ports: list[int],
    root: Path,
    state: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    immutable = state["immutable"]
    fix_hash = validate_hex(
        "durable fix_packet_hash", state["derived"].get("fix_packet_hash"), 96
    )
    info = fleet_rpc_identical(
        rpc,
        ports,
        "fx_fix_info",
        {"fix_packet_hash": fix_hash},
        args.rpc_timeout_seconds,
    )
    if info.get("found") is not True or not isinstance(info.get("fix"), dict):
        raise RuntimeError("durably bound FX fix is no longer queryable")
    row = info["fix"]
    packet = row.get("state", {}).get("packet", {})
    expected_packet = {
        "packet_hash": fix_hash,
        "operator": immutable["facility_operator"],
        "base_asset_id": immutable["base_asset_id"],
        "quote_asset_id": immutable["quote_asset_id"],
        "ratio_numerator": immutable["expected_ratio_numerator"],
        "ratio_denominator": immutable["expected_ratio_denominator"],
        "band_bps": 0,
        "fee_bps": 0,
        "source_label": immutable["expected_source_label"],
    }
    for field, expected in expected_packet.items():
        if packet.get(field) != expected:
            raise RuntimeError(f"durably bound FX fix {field} mismatch")
    quote = load_json(root / "private/fix-quote.json")
    expected_quote = (
        immutable["base_atoms"] * immutable["expected_ratio_numerator"]
    ) // immutable["expected_ratio_denominator"]
    if (
        quote.get("fix_packet_hash") != fix_hash
        or quote.get("base_atoms") != immutable["base_atoms"]
        or quote.get("quote_atoms") != immutable["expected_quote_atoms"]
        or expected_quote != immutable["expected_quote_atoms"]
        or quote.get("pricing_claim") != state["derived"].get("pricing_claim")
    ):
        raise RuntimeError("stored six-validator quote no longer matches the immutable intent")
    return row, quote


def action_elements(action: dict[str, Any]) -> tuple[list[str], list[str], str]:
    nullifiers = action.get("nullifiers")
    outputs = action.get("output_commitments")
    binding = action.get("swap_binding_hash")
    if not isinstance(nullifiers, list) or len(nullifiers) != 2 or len(set(nullifiers)) != 2:
        raise RuntimeError("private swap action must contain two distinct nullifiers")
    if not isinstance(outputs, list) or len(outputs) != 2 or len(set(outputs)) != 2:
        raise RuntimeError("private swap action must contain two distinct output commitments")
    for index, value in enumerate(nullifiers):
        validate_hex(f"nullifier[{index}]", value, 64)
    for index, value in enumerate(outputs):
        validate_hex(f"output_commitment[{index}]", value, 64)
    validate_hex("swap_binding_hash", binding, 128)
    return nullifiers, outputs, binding


def create_or_recover_action(
    args: argparse.Namespace,
    root: Path,
    state: dict[str, Any],
    quote: dict[str, Any],
) -> dict[str, Any]:
    immutable = state["immutable"]
    quote_binding_hash = domain_hash_256(
        "postfiat.pnok.private_fix.quote_binding.v1",
        canonical_json({"intent_id": immutable["intent_id"], "quote": quote}).encode(),
    )
    local_expiry = state["derived"].setdefault(
        "local_quote_expires_at_ms", int(time.time() * 1000) + args.local_quote_lifetime_ms
    )
    request = {
        "request_id": immutable["intent_id"],
        "wallet_address": immutable["wallet_address"],
        "liquidity_wallet_address": immutable["facility_operator"],
        "from_asset_id": immutable["base_asset_id"],
        "to_asset_id": immutable["quote_asset_id"],
        "amount_atoms": immutable["base_atoms"],
        "wallet_commitment": immutable["wallet_note_commitment"],
        "liquidity_amount_atoms": immutable["expected_quote_atoms"],
        "liquidity_commitment": immutable["liquidity_commitment"],
        "quote_binding_hash": quote_binding_hash,
        "quote_expires_at_ms": str(local_expiry),
        "pricing_claim": quote["pricing_claim"],
    }
    wallet_input_note_path = immutable.get("wallet_input_note_path")
    liquidity_input_note_path = immutable.get("liquidity_input_note_path")
    if wallet_input_note_path is not None and liquidity_input_note_path is not None:
        request["input_note_path_a"] = wallet_input_note_path
        request["input_note_path_b"] = liquidity_input_note_path
    response = service_json(
        args.service_url,
        "POST",
        "/asset-orchard/swap-actions",
        request,
        args.prover_timeout_seconds,
    )
    if response.get("schema") != "postfiat-asset-orchard-local-swap-action-v1":
        raise RuntimeError("resident service returned an unsupported swap action schema")
    if response.get("request_id") != immutable["intent_id"]:
        raise RuntimeError("resident service did not bind the durable request ID")
    if response.get("verification", {}).get("verified") is not True:
        raise RuntimeError("resident service did not locally verify the private proof")
    action = json.loads(response.get("action_json", ""))
    nullifiers, outputs, binding = action_elements(action)
    if action.get("pricing_claim") != quote["pricing_claim"] or action.get("fee") != 0:
        raise RuntimeError("private action pricing claim or fee differs from the finalized quote")
    atomic_write_json(root / "private/swap-action-response.json", response, 0o600)
    atomic_write_json(
        root / "public/swap-public-action.json",
        {
            "schema": action.get("schema"),
            "pool_id": action.get("pool_id"),
            "proof_system_id": action.get("proof_system_id"),
            "circuit_id": action.get("circuit_id"),
            "nullifiers": nullifiers,
            "output_commitments": outputs,
            "pricing_claim": action.get("pricing_claim"),
            "swap_binding_hash": binding,
            "fee": action.get("fee"),
        },
        0o644,
    )
    state["derived"].update(
        {
            "quote_binding_hash": quote_binding_hash,
            "swap_id": response["swap_id"],
            "action_binding_hash": binding,
            "nullifiers": nullifiers,
            "output_commitments": outputs,
            "wallet_output_commitment": response["vault_update"]["wallet_output_commitment"],
            "facility_output_commitment": response["vault_update"]["pool_output_commitment"],
        }
    )
    return response


def build_reservation_operation(state: dict[str, Any], row: dict[str, Any]) -> dict[str, Any]:
    immutable = state["immutable"]
    derived = state["derived"]
    packet = row["state"]["packet"]
    wallet_intent = {
        "schema": "postfiat-pnok-private-fix-wallet-intent-v1",
        "chain_id": immutable["chain_id"],
        "genesis_hash": immutable["genesis_hash"],
        "intent_id": immutable["intent_id"],
        "wallet_address": immutable["wallet_address"],
        "fix_packet_hash": derived["fix_packet_hash"],
        "base_asset_id": immutable["base_asset_id"],
        "quote_asset_id": immutable["quote_asset_id"],
        "base_atoms": immutable["base_atoms"],
        "quote_atoms": immutable["expected_quote_atoms"],
        "action_binding_hash": derived["action_binding_hash"],
        "expires_at_height": packet["expires_at_height"],
    }
    wallet_intent_hash = domain_hash_384(
        "postfiat.pnok.private_fix.wallet_intent.v1", canonical_json(wallet_intent).encode()
    )
    reservation_nonce = domain_hash_384(
        "postfiat.pnok.private_fix.reservation_nonce.v1",
        immutable["intent_id"].encode(),
    )
    operation = {
        "operation": "fx_fix_reservation_create_v1",
        "operator": immutable["facility_operator"],
        "fix_packet_hash": derived["fix_packet_hash"],
        "action_binding_hash": derived["action_binding_hash"],
        "base_atoms": immutable["base_atoms"],
        "quote_atoms": immutable["expected_quote_atoms"],
        "wallet_intent_hash": wallet_intent_hash,
        "reservation_nonce": reservation_nonce,
        "expires_at_height": packet["expires_at_height"],
    }
    derived.update(
        {
            "wallet_intent_hash": wallet_intent_hash,
            "reservation_nonce": reservation_nonce,
            "reservation_id": reservation_id(operation),
            "reservation_expires_at_height": packet["expires_at_height"],
        }
    )
    return operation


def run_command(command: list[str], private_dir: Path, label: str) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, text=True, capture_output=True)
    atomic_write_json(
        private_dir / f"{label}.process.json",
        {
            "argv": command,
            "returncode": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
        },
        0o600,
    )
    return result


def finality_common_args(args: argparse.Namespace) -> list[str]:
    values = [
        "--proposer-hosts-file",
        str(args.proposer_hosts_file),
        "--remote-binary",
        args.remote_binary,
        "--remote-topology",
        args.remote_topology,
        "--ports",
        args.ports,
        "--timeout-seconds",
        str(args.rpc_timeout_seconds),
        "--preflight-seconds",
        str(args.convergence_timeout_seconds),
        "--postflight-seconds",
        str(args.convergence_timeout_seconds),
    ]
    if args.resident_manifest:
        values.extend(["--resident-manifest", str(args.resident_manifest)])
    return values


def reservation_status(
    args: argparse.Namespace, rpc: Any, ports: list[int], reservation: str
) -> dict[str, Any]:
    return fleet_rpc_identical(
        rpc,
        ports,
        "fx_fix_reservation_info",
        {"reservation_id": reservation},
        args.rpc_timeout_seconds,
    )


def ensure_reservation(
    args: argparse.Namespace,
    root: Path,
    state: dict[str, Any],
    rpc: Any,
    ports: list[int],
    operation: dict[str, Any],
) -> None:
    reservation = state["derived"]["reservation_id"]
    current = reservation_status(args, rpc, ports, reservation)
    if current.get("found") is True:
        observed = current.get("reservation") or {}
        for field in (
            "reservation_id",
            "fix_packet_hash",
            "operator",
            "action_binding_hash",
            "base_atoms",
            "quote_atoms",
            "wallet_intent_hash",
            "reservation_nonce",
        ):
            expected = reservation if field == "reservation_id" else operation[field]
            if observed.get(field) != expected:
                raise RuntimeError(f"existing reservation {field} mismatch")
        if observed.get("state") not in ("active", "filled"):
            raise RuntimeError(f"existing reservation is terminal in state {observed.get('state')}")
        return

    state["attempts"]["reservation"] += 1
    attempt = state["attempts"]["reservation"]
    ops_file = root / f"private/reservation-attempt-{attempt}.ops.json"
    artifact_dir = root / f"private/reservation-attempt-{attempt}"
    payload = {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": f"pnok-fix-reserve-{attempt}",
                "source": state["immutable"]["facility_operator"],
                "key_file": state["immutable"]["facility_key_file"],
                "operation": operation,
            }
        ],
    }
    atomic_write_json(ops_file, payload, 0o600)
    command = [
        sys.executable,
        str(args.remote_op_finality_script),
        "--ops-file",
        str(ops_file),
        "--artifact-dir",
        str(artifact_dir),
        "--node-bin",
        str(args.node_bin),
        "--remote-runner",
        str(args.remote_op_runner),
        *finality_common_args(args),
    ]
    result = run_command(command, root / "private", f"reservation-attempt-{attempt}")
    recovered = reservation_status(args, rpc, ports, reservation)
    if recovered.get("found") is not True:
        raise RuntimeError(
            f"reservation finality did not produce the deterministic reservation; runner exit={result.returncode}"
        )
    if recovered.get("reservation", {}).get("state") != "active":
        raise RuntimeError("new reservation did not finalize active")


def build_batch(args: argparse.Namespace, root: Path, response: dict[str, Any]) -> Path:
    batch_response = service_json(
        args.service_url,
        "POST",
        "/asset-orchard/swap-batch",
        {"route": "pnok_private_fix", "swap_action_json": response["action_json"]},
        args.rpc_timeout_seconds,
    )
    if batch_response.get("schema") != "postfiat-asset-orchard-local-swap-batch-v1":
        raise RuntimeError("resident service returned an unsupported swap batch schema")
    batch = batch_response.get("batch")
    if not isinstance(batch, dict) or batch.get("batch_kind") not in (None, "shielded"):
        raise RuntimeError("resident service returned an invalid shielded batch")
    path = root / "private/swap-batch.json"
    atomic_write_json(path, batch, 0o600)
    return path


def action_status(
    args: argparse, rpc: Any, ports: list[int], state: dict[str, Any]
) -> dict[str, Any]:
    derived = state["derived"]
    return fleet_rpc_identical(
        rpc,
        ports,
        "asset_orchard_action_status",
        {
            "nullifier_1": derived["nullifiers"][0],
            "nullifier_2": derived["nullifiers"][1],
            "output_commitment_1": derived["output_commitments"][0],
            "output_commitment_2": derived["output_commitments"][1],
        },
        args.rpc_timeout_seconds,
    )


def submit_batch(
    args: argparse,
    root: Path,
    state: dict[str, Any],
    rpc: Any,
    ports: list[int],
    batch_file: Path,
    *,
    replay: bool = False,
) -> subprocess.CompletedProcess[str]:
    counter = "replay" if replay else "submission"
    state["attempts"][counter] += 1
    attempt = state["attempts"][counter]
    label = f"pnok-fix-{'replay' if replay else 'swap'}-{attempt}"
    artifact_dir = root / f"private/{counter}-attempt-{attempt}"
    command = [
        sys.executable,
        str(args.remote_batch_finality_script),
        "--batch-file",
        str(batch_file),
        "--batch-kind",
        "shielded",
        "--label",
        label,
        "--artifact-dir",
        str(artifact_dir),
        "--remote-runner",
        str(args.remote_batch_runner),
        *finality_common_args(args),
    ]
    return run_command(command, root / "private", f"{counter}-attempt-{attempt}")


def snapshot_supplies(
    args: argparse, rpc: Any, ports: list[int], state: dict[str, Any]
) -> dict[str, int]:
    supplies: dict[str, int] = {}
    for role, asset_id in (
        ("base", state["immutable"]["base_asset_id"]),
        ("quote", state["immutable"]["quote_asset_id"]),
    ):
        report = fleet_rpc_identical(
            rpc, ports, "asset_info", {"asset_id": asset_id}, args.rpc_timeout_seconds
        )
        if report.get("found") is not True or not isinstance(report.get("asset"), dict):
            raise RuntimeError(f"{role} issued asset is missing")
        supplies[role] = int(report["asset"]["outstanding_supply"])
    return supplies


def finalize_local_notes(args: argparse.Namespace, root: Path, state: dict[str, Any]) -> None:
    response = service_json(
        args.service_url,
        "POST",
        "/asset-orchard/swap-finalize",
        {"swap_id": state["derived"]["swap_id"], "accepted": True},
        args.rpc_timeout_seconds,
    )
    atomic_write_json(root / "private/local-finalize-response.json", response, 0o600)
    notes_response = service_json(
        args.service_url, "GET", "/asset-orchard/notes", None, args.rpc_timeout_seconds
    )
    notes = notes_response.get("notes")
    if not isinstance(notes, list):
        raise RuntimeError("resident service note scan response is malformed")
    immutable = state["immutable"]
    wallet_output = note_by_commitment(notes, state["derived"]["wallet_output_commitment"])
    facility_output = note_by_commitment(notes, state["derived"]["facility_output_commitment"])
    require_note(
        wallet_output,
        owner=immutable["wallet_address"],
        asset_id=immutable["quote_asset_id"],
        amount_atoms=immutable["expected_quote_atoms"],
        state="spendable",
    )
    require_note(
        facility_output,
        owner=immutable["facility_operator"],
        asset_id=immutable["base_asset_id"],
        amount_atoms=immutable["base_atoms"],
        state="spendable",
    )
    atomic_write_json(
        root / "public/private-output-scan.redacted.json",
        {
            "schema": "postfiat-pnok-private-fix-output-scan-redacted-v1",
            "wallet_output_found": True,
            "facility_output_found": True,
            "wallet_output_state": "spendable",
            "facility_output_state": "spendable",
            "owners_redacted": True,
            "assets_redacted": True,
            "amounts_redacted": True,
        },
        0o644,
    )


def run_demo(args: argparse.Namespace, root: Path, state: dict[str, Any]) -> None:
    script_dir = Path(__file__).resolve().parent
    rpc = load_rpc_helpers(script_dir)
    ports = [int(value) for value in args.ports.split(",")]
    if len(ports) != 6 or len(set(ports)) != 6:
        raise RuntimeError("exactly six distinct PFTL RPC ports are required")
    immutable = state["immutable"]

    rpc.wait_for_fleet_status(ports, args.rpc_timeout_seconds, args.convergence_timeout_seconds)
    if stage_at_least(state, "action_built"):
        row, quote = recover_bound_quote(args, rpc, ports, root, state)
    else:
        row, quote = verify_quote(args, rpc, ports, immutable)
    fix_hash = row["state"]["packet"]["packet_hash"]
    if state["derived"].get("fix_packet_hash") not in (None, fix_hash):
        raise RuntimeError("durable intent is already bound to a different FX fix")
    state["derived"].update(
        {
            "fix_packet_hash": fix_hash,
            "fix_source_label": quote["source_label"],
            "pricing_claim": quote["pricing_claim"],
        }
    )
    state["derived"].setdefault(
        "local_quote_expires_at_ms", int(time.time() * 1000) + args.local_quote_lifetime_ms
    )
    if "supplies_before" not in state["derived"]:
        state["derived"]["supplies_before"] = snapshot_supplies(args, rpc, ports, state)
    atomic_write_json(root / "public/fix-packet.json", row["state"]["packet"], 0o644)
    atomic_write_json(root / "private/fix-quote.json", quote, 0o600)
    persist_progress(root, state, "quote_verified")
    if args.stop_after == "quote_verified":
        return

    if not stage_at_least(state, "action_built") and not state["derived"].get(
        "action_request_attempted"
    ):
        if immutable.get("wallet_input_note_path") is None:
            notes_response = service_json(
                args.service_url, "GET", "/asset-orchard/notes", None, args.rpc_timeout_seconds
            )
            notes = notes_response.get("notes")
            if not isinstance(notes, list):
                raise RuntimeError("resident service note list is malformed")
            wallet_input = note_by_commitment(notes, immutable["wallet_note_commitment"])
            facility_input = note_by_commitment(notes, immutable["liquidity_commitment"])
            require_note(
                wallet_input,
                owner=immutable["wallet_address"],
                asset_id=immutable["base_asset_id"],
                amount_atoms=immutable["base_atoms"],
                state="spendable",
            )
            require_note(
                facility_input,
                owner=immutable["facility_operator"],
                asset_id=immutable["quote_asset_id"],
                amount_atoms=immutable["expected_quote_atoms"],
                state="spendable",
            )
        state["derived"]["action_request_attempted"] = True
        persist_progress(root, state, "quote_verified")
    response = create_or_recover_action(args, root, state, quote)
    persist_progress(root, state, "action_built")
    if args.stop_after == "action_built":
        return

    operation = build_reservation_operation(state, row)
    ensure_reservation(args, root, state, rpc, ports, operation)
    persist_progress(root, state, "reservation_finalized")
    if args.stop_after == "reservation_finalized":
        return

    batch_file = root / "private/swap-batch.json"
    if not batch_file.exists():
        batch_file = build_batch(args, root, response)
    persist_progress(root, state, "batch_built")
    if args.stop_after == "batch_built":
        return

    status = action_status(args, rpc, ports, state)
    if status.get("finalized_exactly_once") is not True:
        result = submit_batch(args, root, state, rpc, ports, batch_file)
        persist_progress(root, state, "submitted")
        status = action_status(args, rpc, ports, state)
        if status.get("finalized_exactly_once") is not True:
            raise RuntimeError(f"private swap did not finalize exactly once; runner exit={result.returncode}")
    elif not stage_at_least(state, "submitted"):
        persist_progress(root, state, "submitted")
    reservation = reservation_status(args, rpc, ports, state["derived"]["reservation_id"])
    if reservation.get("reservation", {}).get("state") != "filled":
        raise RuntimeError("private swap finalized without atomically filling its reservation")
    state["derived"]["nullifier_occurrence_counts"] = [
        entry["occurrence_count"] for entry in status["nullifiers"]
    ]
    state["derived"]["output_occurrence_counts"] = [
        entry["occurrence_count"] for entry in status["output_commitments"]
    ]
    atomic_write_json(root / "public/swap-finality.json", status, 0o644)
    persist_progress(root, state, "finalized")
    if args.stop_after == "finalized":
        return

    finalize_local_notes(args, root, state)
    persist_progress(root, state, "local_finalized")

    supplies_after = snapshot_supplies(args, rpc, ports, state)
    state["derived"]["supplies_after"] = supplies_after
    state["derived"]["supply_unchanged"] = supplies_after == state["derived"]["supplies_before"]
    if not state["derived"]["supply_unchanged"]:
        raise RuntimeError("private swap changed issued-asset supply")

    if args.verify_replay and not state["derived"].get("replay_rejected_without_effect"):
        replay = submit_batch(args, root, state, rpc, ports, batch_file, replay=True)
        after_replay = action_status(args, rpc, ports, state)
        supplies_after_replay = snapshot_supplies(args, rpc, ports, state)
        unchanged = (
            replay.returncode != 0
            and after_replay.get("finalized_exactly_once") is True
            and all(item.get("occurrence_count") == 1 for item in after_replay["nullifiers"])
            and all(item.get("occurrence_count") == 1 for item in after_replay["output_commitments"])
            and supplies_after_replay == supplies_after
        )
        if not unchanged:
            raise RuntimeError("exact private-swap replay was not rejected without economic effect")
        state["derived"]["replay_rejected_without_effect"] = True
        atomic_write_json(
            root / "public/replay-report.json",
            {
                "schema": "postfiat-pnok-private-fix-replay-report-v1",
                "runner_rejected": True,
                "nullifiers_still_exactly_once": True,
                "outputs_still_exactly_once": True,
                "supply_unchanged": True,
            },
            0o644,
        )
    persist_progress(root, state, "complete")


def abort_demo(args: argparse.Namespace, root: Path, state: dict[str, Any]) -> None:
    if stage_at_least(state, "finalized"):
        raise RuntimeError("cannot abort: the private swap is already finalized")
    if not stage_at_least(state, "action_built"):
        raise RuntimeError("nothing is locked: no private action was built")
    rpc = load_rpc_helpers(Path(__file__).resolve().parent)
    ports = [int(value) for value in args.ports.split(",")]
    status = action_status(args, rpc, ports, state)
    if status.get("finalized_exactly_once") is True:
        raise RuntimeError("cannot abort: chain state proves the private action finalized")
    reservation = state["derived"].get("reservation_id")
    if reservation:
        report = reservation_status(args, rpc, ports, reservation)
        if report.get("active") is True:
            raise RuntimeError(
                "reservation remains active; release it through a certified operator operation before local abort"
            )
    response = service_json(
        args.service_url,
        "POST",
        "/asset-orchard/swap-finalize",
        {"swap_id": state["derived"]["swap_id"], "accepted": False},
        args.rpc_timeout_seconds,
    )
    atomic_write_json(root / "private/local-abort-response.json", response, 0o600)
    state["derived"]["aborted_without_chain_effect"] = True
    persist_state(root, state)


def parse_args() -> argparse.Namespace:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("run", "status", "abort"), nargs="?", default="run")
    parser.add_argument("--intent-dir", type=Path, required=True)
    parser.add_argument("--intent-id", required=True)
    parser.add_argument("--wallet-address", required=True)
    parser.add_argument("--facility-operator", required=True)
    parser.add_argument("--facility-key-file", type=Path, required=True)
    parser.add_argument("--base-asset-id", default=DEFAULT_PFUSDC_ASSET_ID)
    parser.add_argument("--quote-asset-id", required=True)
    parser.add_argument("--base-atoms", type=int, default=20_000_000)
    parser.add_argument("--expected-quote-atoms", type=int, default=210)
    parser.add_argument("--wallet-note-commitment", required=True)
    parser.add_argument("--liquidity-commitment", required=True)
    parser.add_argument(
        "--wallet-input-note-path",
        help="absolute wallet-note path readable by the resident service",
    )
    parser.add_argument(
        "--liquidity-input-note-path",
        help="absolute facility-note path readable by the resident service",
    )
    parser.add_argument("--fix-packet-hash")
    parser.add_argument("--expected-source-label", default="pnok_demo_fix")
    parser.add_argument("--expected-ratio-numerator", type=int, default=21)
    parser.add_argument("--expected-ratio-denominator", type=int, default=2_000_000)
    parser.add_argument("--min-expiry-blocks", type=int, default=8)
    parser.add_argument("--local-quote-lifetime-ms", type=int, default=900_000)
    parser.add_argument("--service-url", default="http://127.0.0.1:18799")
    parser.add_argument("--ports", default="28650,28651,28652,28653,28654,28655")
    parser.add_argument("--rpc-timeout-seconds", type=float, default=45.0)
    parser.add_argument("--convergence-timeout-seconds", type=float, default=60.0)
    parser.add_argument("--prover-timeout-seconds", type=float, default=900.0)
    parser.add_argument("--node-bin", type=Path, default=root / "target/release/postfiat-node")
    parser.add_argument(
        "--remote-op-finality-script",
        type=Path,
        default=root / "scripts/a666-ce22-remote-finality-op.py",
    )
    parser.add_argument(
        "--remote-batch-finality-script",
        type=Path,
        default=root / "scripts/a666-ce22-remote-finality-batch.py",
    )
    parser.add_argument(
        "--remote-op-runner",
        type=Path,
        default=root / "scripts/a666-remote-sync-round.py",
    )
    parser.add_argument(
        "--remote-batch-runner",
        type=Path,
        default=root / "scripts/a666-remote-sync-batch-round.py",
    )
    parser.add_argument(
        "--proposer-hosts-file",
        type=Path,
        default=root / "docs/evidence/a666-joe-mainnet-e2e-20260728/proposer-hosts.json",
    )
    parser.add_argument(
        "--remote-binary",
        default="/opt/postfiat/releases/pnok-private-fix-2246d25/postfiat-node",
    )
    parser.add_argument(
        "--remote-topology",
        default="/etc/postfiat/releases/pnok-private-fix-2246d25/topology.json",
    )
    parser.add_argument("--resident-manifest", type=Path)
    parser.add_argument("--stop-after", choices=STAGES[1:-1])
    parser.add_argument("--verify-replay", action=argparse.BooleanOptionalAction, default=True)
    args = parser.parse_args()
    for field in (
        "base_atoms",
        "expected_quote_atoms",
        "expected_ratio_numerator",
        "expected_ratio_denominator",
        "min_expiry_blocks",
        "local_quote_lifetime_ms",
    ):
        if getattr(args, field) <= 0:
            parser.error(f"--{field.replace('_', '-')} must be positive")
    if not args.facility_key_file.is_file():
        parser.error("--facility-key-file must exist")
    return args


def main() -> None:
    args = parse_args()
    root, state = initialize_or_load_state(args)
    publish_redacted_status(root, state)
    if args.command == "status":
        print(canonical_json(load_json(root / "public/status.json")))
        return
    try:
        if args.command == "abort":
            abort_demo(args, root, state)
        else:
            run_demo(args, root, state)
    except BaseException as error:
        record_failure(root, state, error)
        raise
    print(canonical_json(load_json(root / "public/status.json")))


if __name__ == "__main__":
    main()
