"""WAN latency evidence runner for the controlled PostFiat devnet.

The runner uses the same wallet-facing WebSocket proxy as the browser wallet.
It refuses to run mutable probes against a red fleet by default, writes raw
JSONL events for every attempt, and produces a compact summary for the Stage 8
transaction-improvement gate.
"""

from __future__ import annotations

import argparse
import json
import math
import secrets
import stat
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from .client import PostFiatWebSocketRpcClient
from .wallet import (
    TransparentWallet,
    _read_json,
    _run,
    _sdk_bin,
    _validators_list,
    _write_json,
    send_fastpay,
    unwrap_fastpay,
    wrap_fastpay,
)
from .wan_preflight import collect_rpc_preflight, default_validator_endpoints, summarize_preflight


DEFAULT_PROXY_URL = "ws://127.0.0.1:8080"
DEFAULT_REPORT_ROOT = Path("reports/transaction-improvement")
MAX_PROXY_AUTH_TOKEN_FILE_BYTES = 64 * 1024


@dataclass(frozen=True)
class LatencyCounts:
    native: int
    payment_v2: int
    fastpay: int
    fastpay_send_only: int = 0


def utc_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def monotonic_ms_since(started: float) -> float:
    return round((time.monotonic() - started) * 1000, 3)


def read_proxy_auth_token_file(path: str | Path) -> str:
    """Read a private wallet-proxy token without accepting unsafe file shapes."""

    path = Path(path)
    metadata = path.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise ValueError("proxy auth token file must be a regular non-symlink file")
    if metadata.st_mode & 0o077:
        raise ValueError("proxy auth token file must not be accessible by group or other")
    if metadata.st_size > MAX_PROXY_AUTH_TOKEN_FILE_BYTES:
        raise ValueError("proxy auth token file exceeds 64 KiB")
    token = path.read_text(encoding="utf-8").strip()
    if len(token.encode("utf-8")) < 32:
        raise ValueError("proxy auth token must contain at least 32 bytes")
    return token


def load_wallet_descriptor(path: str | Path) -> TransparentWallet:
    """Load a transparent wallet from a StakeHub descriptor or key report."""

    path = Path(path)
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"wallet descriptor must be an object: {path}")
    address = data.get("address")
    public_key_hex = data.get("public_key_hex")
    if not isinstance(address, str) or not address:
        raise ValueError(f"wallet descriptor missing address: {path}")
    if not isinstance(public_key_hex, str) or not public_key_hex:
        raise ValueError(f"wallet descriptor missing public_key_hex: {path}")

    backup_file = data.get("backup_file")
    key_file = data.get("key_file")
    if not isinstance(backup_file, str) or not backup_file:
        if path.name.endswith(".key.json"):
            backup_file = str(path.with_name(path.name.replace(".key.json", ".backup.json")))
        else:
            raise ValueError(f"wallet descriptor missing backup_file: {path}")
    if not isinstance(key_file, str) or not key_file:
        key_file = str(path)

    chain_id = data.get("chain_id")
    if not isinstance(chain_id, str) or not chain_id:
        backup = _read_json(Path(backup_file))
        chain_id = backup.get("chain_id")
    if not isinstance(chain_id, str) or not chain_id:
        raise ValueError(f"wallet descriptor missing chain_id: {path}")

    return TransparentWallet(
        chain_id=chain_id,
        account_index=int(data.get("account_index") or 0),
        address=address,
        public_key_hex=public_key_hex,
        key_file=Path(key_file),
        backup_file=Path(backup_file),
        key_report=data,
    )


def percentile(values: list[float], pct: float) -> float | None:
    if not values:
        return None
    if pct <= 0:
        return round(min(values), 3)
    if pct >= 100:
        return round(max(values), 3)
    ordered = sorted(values)
    index = math.ceil((pct / 100.0) * len(ordered)) - 1
    return round(ordered[max(0, min(index, len(ordered) - 1))], 3)


def summarize_durations(events: list[dict[str, Any]], category: str) -> dict[str, Any]:
    selected = [
        float(event["duration_ms"])
        for event in events
        if event.get("category") == category and event.get("ok") is True
    ]
    failures = [
        event for event in events
        if event.get("category") == category and event.get("ok") is not True
    ]
    return {
        "count": len(selected),
        "failure_count": len(failures),
        "p50_ms": percentile(selected, 50),
        "p90_ms": percentile(selected, 90),
        "p95_ms": percentile(selected, 95),
        "p99_ms": percentile(selected, 99),
        "max_ms": percentile(selected, 100),
    }


def _optional_positive_int(value: Any) -> int | None:
    if isinstance(value, bool) or value is None:
        return None
    try:
        number = int(value)
    except (TypeError, ValueError):
        return None
    return number if number > 0 else None


def account_submit_capability_gate(
    *,
    server_info: dict[str, Any],
    counts: LatencyCounts,
) -> dict[str, Any]:
    """Return the account-lane submit capability gate for a planned run."""

    rpc = server_info.get("rpc", {}) if isinstance(server_info, dict) else {}
    if not isinstance(rpc, dict):
        rpc = {}

    max_per_peer = _optional_positive_int(rpc.get("max_mempool_submit_per_peer"))
    max_total = _optional_positive_int(rpc.get("max_mempool_submit_total"))
    window_secs = _optional_positive_int(rpc.get("mempool_submit_rate_limit_window_secs"))
    planned_account_submits = counts.native + counts.payment_v2
    reasons: list[str] = []
    warnings: list[str] = []

    if planned_account_submits and not rpc.get("mempool_submit_finality_enabled"):
        reasons.append("wallet-facing finality submit RPC is disabled")
    if max_per_peer is not None and window_secs is None:
        if counts.native > max_per_peer:
            reasons.append(
                f"native-count {counts.native} exceeds per-peer submit cap {max_per_peer}"
            )
        if counts.payment_v2 > max_per_peer:
            reasons.append(
                f"payment-v2-count {counts.payment_v2} exceeds per-peer submit cap {max_per_peer}"
            )
    if max_total is not None and window_secs is None and planned_account_submits > max_total:
        reasons.append(
            f"account-lane submits {planned_account_submits} exceed total submit cap {max_total}"
        )
    if max_per_peer is not None and window_secs is not None:
        if counts.native > max_per_peer or counts.payment_v2 > max_per_peer:
            warnings.append(
                "planned account-lane sample exceeds a single rate-limit window; "
                "runner must stay sequential or paced"
            )
    if max_total is not None and window_secs is not None and planned_account_submits > max_total:
        warnings.append(
            "planned account-lane sample exceeds total submits in one rate-limit window; "
            "runner must stay sequential or paced"
        )

    return {
        "ok": not reasons,
        "reasons": reasons,
        "warnings": warnings,
        "rpc": {
            "read_only": rpc.get("read_only"),
            "mempool_submit_enabled": rpc.get("mempool_submit_enabled"),
            "mempool_submit_finality_enabled": rpc.get("mempool_submit_finality_enabled"),
            "max_mempool_submit_per_peer": max_per_peer,
            "max_mempool_submit_total": max_total,
            "mempool_submit_rate_limit_window_secs": window_secs,
        },
        "planned_account_submits": planned_account_submits,
    }


def _finality_summary(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        return {"raw_type": type(result).__name__}
    finality = result.get("finality")
    proxy_route = result.get("proxy_route")
    block = finality.get("block") if isinstance(finality, dict) else None
    header = block.get("header") if isinstance(block, dict) else None
    receipt = finality.get("receipt") if isinstance(finality, dict) else None
    if not isinstance(header, dict):
        header = {}
    if not isinstance(receipt, dict):
        receipt = {}
    if not isinstance(proxy_route, dict):
        proxy_route = {}
    return {
        "tx_id": result.get("tx_id"),
        "block_height": header.get("height"),
        "proposer": header.get("proposer"),
        "state_root": header.get("state_root"),
        "receipt_code": receipt.get("code"),
        "accepted": receipt.get("accepted"),
        "node_total_ms": result.get("total_ms"),
        "readiness_wait_ms": result.get("readiness_wait_ms"),
        "mempool_submit_ms": result.get("mempool_submit_ms"),
        "mempool_batch_ms": result.get("mempool_batch_ms"),
        "certified_round_ms": result.get("certified_round_ms"),
        "round_report_file": result.get("round_report_file"),
        "proxy_route": proxy_route or None,
        "proxy_route_attempts": proxy_route.get("route_attempts"),
        "proxy_route_wait_ms": proxy_route.get("route_wait_ms"),
        "proxy_route_converged_count": proxy_route.get("converged_count"),
    }


def _record_event(
    events: list[dict[str, Any]],
    *,
    category: str,
    iteration: int,
    call: Callable[[], dict[str, Any]],
) -> dict[str, Any]:
    started = time.monotonic()
    event: dict[str, Any] = {
        "category": category,
        "iteration": iteration,
        "started_at": utc_now(),
    }
    try:
        event.update(call())
        event["ok"] = bool(event.get("ok", True))
    except Exception as error:  # noqa: BLE001 - latency evidence preserves exact failure class.
        event.update({
            "ok": False,
            "error_type": type(error).__name__,
            "error": str(error),
        })
    event["duration_ms"] = monotonic_ms_since(started)
    events.append(event)
    return event


def run_native_or_payment(
    *,
    client: PostFiatWebSocketRpcClient,
    wallet_a: TransparentWallet,
    wallet_b: TransparentWallet,
    amount: int,
    work_dir: Path,
    memo: bool,
    review_delay_ms: float = 0.0,
) -> dict[str, Any]:
    work_dir.mkdir(parents=True, exist_ok=True)
    attempts: list[dict[str, Any]] = []
    for attempt in range(1, 3):
        timings: dict[str, float] = {}
        memo_type = "latency".encode("utf-8").hex() if memo else None
        memo_format = "text/plain".encode("utf-8").hex() if memo else None
        memo_data = (
            f"stage8-latency-{time.time_ns()}".encode("utf-8").hex()
            if memo
            else None
        )

        try:
            started = time.monotonic()
            quote_response = client.transfer_fee_quote_response(
                wallet_a.address,
                wallet_b.address,
                amount,
                memo_type=memo_type,
                memo_format=memo_format,
                memo_data=memo_data,
                request_id=f"latency-quote-{secrets.token_hex(4)}",
            )
            timings["quote_ms"] = monotonic_ms_since(started)

            quote_file = work_dir / f"quote-{secrets.token_hex(8)}.response.json"
            signed_file = work_dir / (
                f"signed-{secrets.token_hex(8)}.payment-v2.json"
                if memo
                else f"signed-{secrets.token_hex(8)}.transfer.json"
            )
            _write_json(quote_file, quote_response)
            if review_delay_ms > 0:
                started = time.monotonic()
                time.sleep(review_delay_ms / 1000.0)
                timings["review_delay_ms"] = monotonic_ms_since(started)
            started = time.monotonic()
            if memo:
                quote_result = quote_response.get("result")
                if not isinstance(quote_result, dict):
                    raise ValueError("transfer_fee_quote response missing result object")
                sign_args = [
                    *_sdk_bin(),
                    "wallet-sign-payment-v2",
                    "--backup-file",
                    str(wallet_a.backup_file),
                    "--chain-id",
                    str(quote_result["chain_id"]),
                    "--genesis-hash",
                    str(quote_result["genesis_hash"]),
                    "--protocol-version",
                    str(quote_result["protocol_version"]),
                    "--to",
                    wallet_b.address,
                    "--amount",
                    str(amount),
                    "--fee",
                    str(quote_result["minimum_fee"]),
                    "--sequence",
                    str(quote_result["sequence"]),
                    "--memo-type",
                    str(memo_type),
                    "--memo-format",
                    str(memo_format),
                    "--memo-data",
                    str(memo_data),
                    "--output",
                    str(signed_file),
                ]
                _run(sign_args, json_output=False)
            else:
                _run(
                    [
                        *_sdk_bin(),
                        "wallet-sign-quote",
                        "--backup-file",
                        str(wallet_a.backup_file),
                        "--quote-response",
                        str(quote_file),
                        "--output",
                        str(signed_file),
                    ],
                    json_output=False,
                )
            timings["sign_ms"] = monotonic_ms_since(started)

            signed_transfer = _read_json(signed_file)
            started = time.monotonic()
            if memo:
                submit_result = client.mempool_submit_signed_payment_v2_finality(
                    signed_transfer,
                    request_id=f"latency-submit-{secrets.token_hex(4)}",
                )
            else:
                submit_result = client.mempool_submit_signed_transfer_finality(
                    signed_transfer,
                    request_id=f"latency-submit-{secrets.token_hex(4)}",
                )
            timings["submit_finality_ms"] = monotonic_ms_since(started)
            finality = _finality_summary(submit_result)
            attempts.append({"attempt": attempt, "ok": True, "timings": timings})
            return {
                "ok": finality.get("accepted") is True,
                "tx_id": submit_result.get("tx_id"),
                "attempts": attempts,
                "timings": timings,
                "finality": finality,
                "finality_timeout": False,
            }
        except Exception as error:  # noqa: BLE001 - retry evidence records exact failure.
            attempts.append({
                "attempt": attempt,
                "ok": False,
                "timings": timings,
                "error_type": type(error).__name__,
                "error": str(error),
            })
            if "bad_sequence" not in str(error) or attempt == 2:
                raise
            time.sleep(1.0)

    raise RuntimeError("unreachable account-lane latency retry state")


def run_fastpay_cycle(
    *,
    client: PostFiatWebSocketRpcClient,
    wallet_a: TransparentWallet,
    wallet_b: TransparentWallet,
    amount: int,
    fee: int,
    work_dir: Path,
    validator_records: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    substeps: dict[str, Any] = {}
    wrap_amount = amount + fee + 1

    started = time.monotonic()
    wrap_result = wrap_fastpay(
        client,
        wallet=wallet_a,
        amount=wrap_amount,
        check_capabilities=False,
        refresh_snapshot=False,
    )
    substeps["wrap_ms"] = monotonic_ms_since(started)
    substeps["wrap_result"] = wrap_result.result
    substeps["wrap_timings"] = wrap_result.timings or {}
    wrap_object_id = (
        wrap_result.result.get("object_id")
        if isinstance(wrap_result.result, dict)
        else None
    )
    if not isinstance(wrap_object_id, str) or not wrap_object_id:
        raise ValueError("signed FastPay deposit result missing object_id")

    if validator_records is None:
        started = time.monotonic()
        validator_records = _validators_list(client.validators())
        substeps["validators_lookup_ms"] = monotonic_ms_since(started)
    else:
        substeps["validators_lookup_ms"] = 0.0

    started = time.monotonic()
    send_result = send_fastpay(
        client,
        wallet=wallet_a,
        recipient_public_key_hex=wallet_b.public_key_hex,
        amount=amount,
        fee=fee,
        work_dir=work_dir,
        owned_objects=[{
            "id": wrap_object_id,
            "version": 1,
            "owner_pubkey_hex": wallet_a.public_key_hex,
            "value": wrap_amount,
            "asset": "PFT",
        }],
        validators=validator_records,
        check_capabilities=False,
    )
    substeps["send_apply_ms"] = monotonic_ms_since(started)
    substeps["send_result"] = send_result.result
    substeps["send_timings"] = send_result.timings or {}
    substeps["vote_count"] = len(send_result.votes)
    created_objects = (
        send_result.result.get("created_objects")
        if isinstance(send_result.result, dict)
        else None
    )
    recipient_outputs = [
        obj for obj in created_objects
        if isinstance(obj, dict)
        and obj.get("owner_pubkey_hex") == wallet_b.public_key_hex
        and obj.get("id")
    ] if isinstance(created_objects, list) else []
    if not recipient_outputs:
        raise ValueError("FastPay owned_apply result missing recipient created object")
    recipient_object_id = str(recipient_outputs[0]["id"])

    started = time.monotonic()
    unwrap_result = unwrap_fastpay(
        client,
        wallet=wallet_b,
        object_id=recipient_object_id,
        check_capabilities=False,
    )
    substeps["unwrap_ms"] = monotonic_ms_since(started)
    substeps["unwrap_result"] = unwrap_result.result
    substeps["unwrap_timings"] = unwrap_result.timings or {}

    return {
        "ok": True,
        "substeps": substeps,
    }


def prepare_fastpay_send_input(
    *,
    client: PostFiatWebSocketRpcClient,
    wallet: TransparentWallet,
    amount: int,
    fee: int,
) -> dict[str, Any]:
    """Create a fresh owned object for a send-only FastPay hot-path sample."""

    wrap_amount = amount + fee + 1
    started = time.monotonic()
    wrap_result = wrap_fastpay(
        client,
        wallet=wallet,
        amount=wrap_amount,
        check_capabilities=False,
        refresh_snapshot=False,
    )
    wrap_ms = monotonic_ms_since(started)
    wrap_object_id = (
        wrap_result.result.get("object_id")
        if isinstance(wrap_result.result, dict)
        else None
    )
    if not isinstance(wrap_object_id, str) or not wrap_object_id:
        raise ValueError("signed FastPay deposit result missing object_id")
    owned_object = {
        "id": wrap_object_id,
        "version": 1,
        "owner_pubkey_hex": wallet.public_key_hex,
        "value": wrap_amount,
        "asset": "PFT",
    }
    return {
        "setup_kind": "fastpay_signed_consensus_deposit",
        "setup_wrap_ms": wrap_ms,
        "setup_wrap_result": wrap_result.result,
        "setup_wrap_timings": wrap_result.timings or {},
        "owned_object": owned_object,
    }


def run_fastpay_send_only(
    *,
    client: PostFiatWebSocketRpcClient,
    wallet_a: TransparentWallet,
    wallet_b: TransparentWallet,
    amount: int,
    fee: int,
    work_dir: Path,
    owned_object: dict[str, Any],
    validator_records: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    substeps: dict[str, Any] = {}
    if validator_records is None:
        started = time.monotonic()
        validator_records = _validators_list(client.validators())
        substeps["validators_lookup_ms"] = monotonic_ms_since(started)
    else:
        substeps["validators_lookup_ms"] = 0.0

    started = time.monotonic()
    send_result = send_fastpay(
        client,
        wallet=wallet_a,
        recipient_public_key_hex=wallet_b.public_key_hex,
        amount=amount,
        fee=fee,
        work_dir=work_dir,
        owned_objects=[owned_object],
        validators=validator_records,
        check_capabilities=False,
    )
    substeps["send_apply_ms"] = monotonic_ms_since(started)
    substeps["send_result"] = send_result.result
    substeps["send_timings"] = send_result.timings or {}
    substeps["vote_count"] = len(send_result.votes)
    return {
        "ok": True,
        "substeps": substeps,
    }


def _record_fastpay_send_only_event(
    events: list[dict[str, Any]],
    *,
    iteration: int,
    client: PostFiatWebSocketRpcClient,
    wallet_a: TransparentWallet,
    wallet_b: TransparentWallet,
    amount: int,
    fee: int,
    work_dir: Path,
    validator_records: list[dict[str, Any]] | None,
) -> dict[str, Any]:
    event: dict[str, Any] = {
        "category": "fastpay_send_only",
        "iteration": iteration,
        "started_at": utc_now(),
    }
    event_started = time.monotonic()
    hot_started: float | None = None
    try:
        setup = prepare_fastpay_send_input(
            client=client,
            wallet=wallet_a,
            amount=amount,
            fee=fee,
        )
        event["setup"] = {
            key: value for key, value in setup.items() if key != "owned_object"
        }
        hot_started = time.monotonic()
        event.update(run_fastpay_send_only(
            client=client,
            wallet_a=wallet_a,
            wallet_b=wallet_b,
            amount=amount,
            fee=fee,
            work_dir=work_dir,
            owned_object=setup["owned_object"],
            validator_records=validator_records,
        ))
        event["ok"] = bool(event.get("ok", True))
    except Exception as error:  # noqa: BLE001 - latency evidence preserves exact failure class.
        event.update({
            "ok": False,
            "error_type": type(error).__name__,
            "error": str(error),
        })
    event["duration_ms"] = monotonic_ms_since(hot_started or event_started)
    events.append(event)
    return event


def build_latency_report(
    *,
    proxy_url: str,
    wallet_a_path: str | Path,
    wallet_b_path: str | Path,
    output_dir: Path,
    counts: LatencyCounts,
    amount: int,
    fastpay_fee: int,
    timeout_seconds: float,
    proxy_origin: str | None = None,
    proxy_auth_token: str | None = None,
    review_delay_ms: float = 0.0,
    allow_red_fleet: bool = False,
    allow_rate_limit_exceed: bool = False,
) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=True)
    preflight_entries = collect_rpc_preflight(
        default_validator_endpoints(),
        timeout_seconds=timeout_seconds,
    )
    preflight_summary = summarize_preflight(preflight_entries)
    if not preflight_summary["healthy"] and not allow_red_fleet:
        return {
            "schema": "postfiat-wan-latency-report-v1",
            "created_at": utc_now(),
            "proxy_url": proxy_url,
            "status": "blocked_red_fleet",
            "preflight": {
                "entries": preflight_entries,
                "summary": preflight_summary,
            },
            "events": [],
            "summary": {},
        }

    client = PostFiatWebSocketRpcClient(
        proxy_url,
        timeout_seconds=timeout_seconds,
        response_byte_cap=4 * 1024 * 1024,
        origin=proxy_origin,
        proxy_auth_token=proxy_auth_token,
    )
    server_info = client.server_info()
    capability_gate = account_submit_capability_gate(
        server_info=server_info,
        counts=counts,
    )
    if not capability_gate["ok"] and not allow_rate_limit_exceed:
        return {
            "schema": "postfiat-wan-latency-report-v1",
            "created_at": utc_now(),
            "proxy_url": proxy_url,
            "status": "blocked_rate_limit_config",
            "preflight": {
                "entries": preflight_entries,
                "summary": preflight_summary,
            },
            "capability_gate": capability_gate,
            "counts": {
                "native": counts.native,
                "payment_v2": counts.payment_v2,
                "fastpay": counts.fastpay,
                "fastpay_send_only": counts.fastpay_send_only,
            },
            "events": [],
            "summary": {
                "full_stage8_sample_size_met": (
                    counts.native >= 50
                    and counts.payment_v2 >= 50
                    and counts.fastpay + counts.fastpay_send_only >= 20
                ),
                "unsupported_breakdowns": [
                    "The requested account-lane sample exceeds current RPC submit caps",
                ],
            },
        }
    wallet_a = load_wallet_descriptor(wallet_a_path)
    wallet_b = load_wallet_descriptor(wallet_b_path)
    events: list[dict[str, Any]] = []
    fastpay_validators = (
        _validators_list(client.validators())
        if counts.fastpay > 0 or counts.fastpay_send_only > 0
        else None
    )

    for index in range(counts.native):
        _record_event(
            events,
            category="native_pft",
            iteration=index + 1,
            call=lambda: run_native_or_payment(
                client=client,
                wallet_a=wallet_a,
                wallet_b=wallet_b,
                amount=amount,
                work_dir=output_dir / "work",
                memo=False,
                review_delay_ms=review_delay_ms,
            ),
        )

    for index in range(counts.payment_v2):
        _record_event(
            events,
            category="payment_v2",
            iteration=index + 1,
            call=lambda: run_native_or_payment(
                client=client,
                wallet_a=wallet_a,
                wallet_b=wallet_b,
                amount=amount,
                work_dir=output_dir / "work",
                memo=True,
                review_delay_ms=review_delay_ms,
            ),
        )

    for index in range(counts.fastpay):
        _record_event(
            events,
            category="fastpay_cycle",
            iteration=index + 1,
            call=lambda: run_fastpay_cycle(
                client=client,
                wallet_a=wallet_a,
                wallet_b=wallet_b,
                amount=amount,
                fee=fastpay_fee,
                work_dir=output_dir / "work",
                validator_records=fastpay_validators,
            ),
        )

    for index in range(counts.fastpay_send_only):
        _record_fastpay_send_only_event(
            events,
            iteration=index + 1,
            client=client,
            wallet_a=wallet_a,
            wallet_b=wallet_b,
            amount=amount,
            fee=fastpay_fee,
            work_dir=output_dir / "work",
            validator_records=fastpay_validators,
        )

    summary = {
        "native_pft": summarize_durations(events, "native_pft"),
        "payment_v2": summarize_durations(events, "payment_v2"),
        "fastpay_cycle": summarize_durations(events, "fastpay_cycle"),
        "fastpay_send_only": summarize_durations(events, "fastpay_send_only"),
        "full_stage8_sample_size_met": (
            counts.native >= 50
            and counts.payment_v2 >= 50
            and counts.fastpay + counts.fastpay_send_only >= 20
        ),
        "unsupported_breakdowns": [
            "FastPay cycle is measured as proxy-broadcast wrap/send/apply/unwrap; send-only isolates the hot owned-object transfer",
            "trustline/asset/offer/Orchard/bridge latency is not measured until Stage 7 write paths are enabled",
        ],
    }
    return {
        "schema": "postfiat-wan-latency-report-v1",
        "created_at": utc_now(),
        "proxy_url": proxy_url,
        "status": "complete" if all(event.get("ok") for event in events) else "partial_or_failed",
        "preflight": {
            "entries": preflight_entries,
            "summary": preflight_summary,
        },
        "capability_gate": capability_gate,
        "counts": {
            "native": counts.native,
            "payment_v2": counts.payment_v2,
            "fastpay": counts.fastpay,
            "fastpay_send_only": counts.fastpay_send_only,
        },
        "amount": amount,
        "review_delay_ms": review_delay_ms,
        "events": events,
        "summary": summary,
    }


def write_latency_outputs(report: dict[str, Any], output_dir: Path) -> dict[str, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    raw_path = output_dir / "latency-raw.jsonl"
    summary_path = output_dir / "latency-summary.json"
    markdown_path = output_dir / "latency-summary.md"
    with raw_path.open("w", encoding="utf-8") as handle:
        for event in report.get("events", []):
            handle.write(json.dumps(event, sort_keys=True) + "\n")
    summary_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    markdown_path.write_text(markdown_report(report), encoding="utf-8")
    return {"raw": raw_path, "summary": summary_path, "markdown": markdown_path}


def markdown_report(report: dict[str, Any]) -> str:
    lines = [
        "# WAN Latency Evidence",
        "",
        f"Created: {report.get('created_at')}",
        f"Status: `{report.get('status')}`",
        f"Proxy: `{report.get('proxy_url')}`",
        f"Review delay ms: `{report.get('review_delay_ms', 0)}`",
        "",
    ]
    preflight = report.get("preflight", {})
    preflight_summary = preflight.get("summary", {}) if isinstance(preflight, dict) else {}
    lines.extend([
        "## Fleet Gate",
        "",
        f"- Healthy: `{preflight_summary.get('healthy')}`",
        f"- Reachable: `{preflight_summary.get('reachable_count')}/{preflight_summary.get('validator_count')}`",
        f"- Largest ledger group: `{preflight_summary.get('largest_ledger_group')}`",
        f"- Red reasons: `{preflight_summary.get('red_reasons')}`",
        "",
    ])
    capability_gate = report.get("capability_gate", {})
    if isinstance(capability_gate, dict):
        rpc = capability_gate.get("rpc", {})
        if not isinstance(rpc, dict):
            rpc = {}
        lines.extend([
            "## RPC Capability Gate",
            "",
            f"- OK: `{capability_gate.get('ok')}`",
            f"- Planned account-lane submits: `{capability_gate.get('planned_account_submits')}`",
            f"- Per-peer submit cap: `{rpc.get('max_mempool_submit_per_peer')}`",
            f"- Total submit cap: `{rpc.get('max_mempool_submit_total')}`",
            f"- Rate-limit window seconds: `{rpc.get('mempool_submit_rate_limit_window_secs')}`",
            f"- Reasons: `{capability_gate.get('reasons')}`",
            f"- Warnings: `{capability_gate.get('warnings')}`",
            "",
        ])
    lines.extend([
        "## Latency Summary",
        "",
        "| Category | Count | Failures | p50 ms | p90 ms | p95 ms | p99 ms | max ms |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ])
    summary = report.get("summary", {})
    if isinstance(summary, dict):
        for category in ("native_pft", "payment_v2", "fastpay_cycle", "fastpay_send_only"):
            item = summary.get(category, {})
            if not isinstance(item, dict):
                item = {}
            lines.append(
                f"| `{category}` | {item.get('count')} | {item.get('failure_count')} | "
                f"{item.get('p50_ms')} | {item.get('p90_ms')} | {item.get('p95_ms')} | "
                f"{item.get('p99_ms')} | {item.get('max_ms')} |"
            )
        unsupported = summary.get("unsupported_breakdowns", [])
        if unsupported:
            lines.extend(["", "## Explicit Gaps", ""])
            for item in unsupported:
                lines.append(f"- {item}")
        lines.extend([
            "",
            f"Full Stage 8 sample size met: `{summary.get('full_stage8_sample_size_met')}`",
        ])
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proxy-url", default=DEFAULT_PROXY_URL)
    parser.add_argument("--wallet-a", required=True)
    parser.add_argument("--wallet-b", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--native-count", type=int, default=2)
    parser.add_argument("--payment-v2-count", type=int, default=2)
    parser.add_argument("--fastpay-count", type=int, default=1)
    parser.add_argument("--fastpay-send-only-count", type=int, default=0)
    parser.add_argument("--amount", type=int, default=1)
    parser.add_argument("--fastpay-fee", type=int, default=1)
    parser.add_argument("--timeout-seconds", type=float, default=20.0)
    parser.add_argument("--proxy-origin")
    parser.add_argument("--proxy-auth-token-file")
    parser.add_argument("--review-delay-ms", type=float, default=0.0)
    parser.add_argument("--allow-red-fleet", action="store_true")
    parser.add_argument("--allow-rate-limit-exceed", action="store_true")
    args = parser.parse_args(argv)

    for name, value in [
        ("native-count", args.native_count),
        ("payment-v2-count", args.payment_v2_count),
        ("fastpay-count", args.fastpay_count),
        ("fastpay-send-only-count", args.fastpay_send_only_count),
        ("amount", args.amount),
    ]:
        if value < 0:
            parser.error(f"--{name} must be nonnegative")
    if args.amount < 1:
        parser.error("--amount must be positive")
    if args.fastpay_fee < 0:
        parser.error("--fastpay-fee must be nonnegative")
    if args.review_delay_ms < 0:
        parser.error("--review-delay-ms must be nonnegative")

    output_dir = Path(args.output_dir)
    proxy_auth_token = (
        read_proxy_auth_token_file(args.proxy_auth_token_file)
        if args.proxy_auth_token_file
        else None
    )
    report = build_latency_report(
        proxy_url=args.proxy_url,
        wallet_a_path=args.wallet_a,
        wallet_b_path=args.wallet_b,
        output_dir=output_dir,
        counts=LatencyCounts(
            native=args.native_count,
            payment_v2=args.payment_v2_count,
            fastpay=args.fastpay_count,
            fastpay_send_only=args.fastpay_send_only_count,
        ),
        amount=args.amount,
        fastpay_fee=args.fastpay_fee,
        timeout_seconds=args.timeout_seconds,
        proxy_origin=args.proxy_origin,
        proxy_auth_token=proxy_auth_token,
        review_delay_ms=args.review_delay_ms,
        allow_red_fleet=args.allow_red_fleet,
        allow_rate_limit_exceed=args.allow_rate_limit_exceed,
    )
    paths = write_latency_outputs(report, output_dir)
    print(f"latency raw written: {paths['raw']}")
    print(f"latency summary written: {paths['summary']}")
    print(f"latency markdown written: {paths['markdown']}")
    print(json.dumps({
        "status": report.get("status"),
        "summary": report.get("summary"),
        "red_reasons": report.get("preflight", {}).get("summary", {}).get("red_reasons"),
    }, indent=2))
    blocked_statuses = {"blocked_red_fleet", "blocked_rate_limit_config"}
    return 1 if report.get("status") in blocked_statuses else 0


if __name__ == "__main__":
    raise SystemExit(main())
