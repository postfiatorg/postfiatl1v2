"""Command line entry point for the stdlib-only PostFiat RPC client."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .client import Endpoint, PostFiatRpcClient, RpcClientError


NONLOCAL_IPV4_RE = re.compile(
    r"(?<![0-9.])(?!127\.0\.0\.1\b)(?:[0-9]{1,3}\.){3}[0-9]{1,3}(?![0-9.])"
)
SENSITIVE_RE = re.compile(
    r"(private[-_ ]?key|secret|password|mnemonic|spending_key|full_viewing_key|"
    r"master_seed|rseed|ssh_cred)",
    re.IGNORECASE,
)


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def redact_network(text: str) -> str:
    return NONLOCAL_IPV4_RE.sub("<ip>", text)


def endpoint_hash(endpoint: Endpoint) -> str:
    return hashlib.sha256(f"{endpoint.host}:{endpoint.port}".encode("utf-8")).hexdigest()[:16]


def parse_endpoint_ref(raw: str) -> tuple[str, Endpoint]:
    label = "endpoint-0"
    value = raw.strip()
    if not value:
        raise ValueError("empty endpoint")
    if "=" in value:
        label, value = value.split("=", 1)
        label = label.strip() or "endpoint-0"
        value = value.strip()
    return label, Endpoint.parse(value)


def output_path(raw: str) -> Path:
    return Path(raw).expanduser()


def value_to_json(value: Any) -> Any:
    if hasattr(value, "to_dict"):
        return value.to_dict()
    if isinstance(value, tuple):
        return [value_to_json(item) for item in value]
    if isinstance(value, list):
        return [value_to_json(item) for item in value]
    if isinstance(value, dict):
        return {key: value_to_json(child) for key, child in value.items()}
    return value


def method_checks(method: str, result: Any, args: argparse.Namespace) -> dict[str, Any]:
    checks: dict[str, Any] = {}
    if method == "status":
        checks["status_running"] = isinstance(result, dict) and result.get("status") == "running"
    elif method == "account":
        checks["account_returned"] = isinstance(result, dict) and result.get("address") == args.address
    elif method == "account_tx":
        checks["account_tx_not_truncated"] = result.truncated is False
        checks["account_tx_index_used"] = result.index_used is True
        checks["required_row_present"] = len(result.rows) > 0 if args.require_row else True
    elif method == "account_tx_history":
        checks["account_tx_history_complete"] = result.complete is True
        checks["account_tx_history_indexed"] = result.all_index_used is True
        checks["account_tx_history_no_archive_lookups"] = result.total_archive_lookup_count == 0
        checks["account_tx_history_no_scans"] = result.total_scanned_block_count == 0
        checks["required_row_present"] = len(result.rows) > 0 if args.require_row else True
    elif method == "account_tx_index_status":
        checks["account_tx_index_usable"] = (
            result.index_usable is True or result.disk_index_usable is True
        )
    elif method == "orchard_pool_report":
        checks["orchard_pool_passed"] = isinstance(result, dict) and result.get("passed") is True
    elif method in {"blocks", "receipts", "batch_archive", "shield_scan"}:
        checks[f"{method}_is_list"] = isinstance(result, list)
    elif method in PUBLIC_READ_OBJECT_METHODS:
        checks[f"{method}_is_object"] = isinstance(result, dict)
    else:
        checks["result_present"] = result is not None
    return checks


def report_has_sensitive_text(report: dict[str, Any]) -> bool:
    text = json.dumps(report, sort_keys=True)
    if NONLOCAL_IPV4_RE.search(text):
        return True
    if SENSITIVE_RE.search(text):
        return True
    return False


# Documented public reads (docs/rpc/method-coverage.md) whose result is one
# report object; `shield_scan` is the list-shaped one and is checked above.
PUBLIC_READ_OBJECT_METHODS = frozenset(
    {
        "archive_window",
        "verify_blocks",
        "verify_state",
        "verify_bridge",
        "verify_mempool",
        "verify_shielded",
        "shield_disclose",
        "vault_bridge_route",
        "vault_bridge_status",
        "market_ops_status",
        "asset_orchard_action_status",
        "nav_reserve_proof_status",
        "fx_fix_list",
        "fx_fix_info",
        "fx_fix_reservation_info",
        "fx_fix_quote",
        "pfusdc_ingress_preflight",
        "pfusdc_egress_witness",
        "yolo_target_receipt",
    }
)

# Every `--method` value the CLI accepts, in display order. The documented
# public reads are listed after the long-standing methods.
CLI_METHODS: tuple[str, ...] = (
    "status",
    "server_info",
    "ledger",
    "fee",
    "validators",
    "manifests",
    "metrics",
    "blocks",
    "receipts",
    "tx",
    "account",
    "atomic_settlement_template",
    "escrow_info",
    "account_escrows",
    "offer_info",
    "account_offers",
    "book_offers",
    "nft_info",
    "account_nfts",
    "issuer_nfts",
    "mempool_status",
    "bridge_status",
    "navcoin_bridge_routes",
    "navcoin_bridge_packet",
    "navcoin_bridge_claims",
    "navcoin_bridge_supply_status",
    "navcoin_bridge_receipt_replay",
    "shield_turnstile",
    "orchard_pool_report",
    "account_tx_index_status",
    "batch_archive",
    "account_tx",
    "account_tx_history",
    "archive_window",
    "verify_blocks",
    "verify_state",
    "verify_bridge",
    "verify_mempool",
    "verify_shielded",
    "shield_scan",
    "shield_disclose",
    "vault_bridge_route",
    "vault_bridge_status",
    "market_ops_status",
    "asset_orchard_action_status",
    "nav_reserve_proof_status",
    "fx_fix_list",
    "fx_fix_info",
    "fx_fix_reservation_info",
    "fx_fix_quote",
    "pfusdc_ingress_preflight",
    "pfusdc_egress_witness",
    "yolo_target_receipt",
)


def require_arg(args: argparse.Namespace, name: str, method: str) -> Any:
    value = getattr(args, name)
    if value is None or value == "":
        raise SystemExit(f"--{name.replace('_', '-')} is required for {method}")
    return value


def call_method(client: PostFiatRpcClient, args: argparse.Namespace) -> Any:
    method = args.method
    if method == "status":
        return client.status()
    if method == "server_info":
        return client.server_info()
    if method == "ledger":
        return client.ledger(limit=args.limit)
    if method == "fee":
        return client.fee()
    if method == "validators":
        return client.validators()
    if method == "manifests":
        return client.manifests()
    if method == "metrics":
        return client.metrics()
    if method == "blocks":
        return client.blocks(from_height=args.from_height, limit=args.limit)
    if method == "receipts":
        return client.receipts(tx_id=args.tx_id, limit=args.limit)
    if method == "tx":
        return client.tx(require_arg(args, "tx_id", method), audit_block_log=args.audit_block_log)
    if method == "account":
        return client.account(require_arg(args, "address", method))
    if method == "atomic_settlement_template":
        return client.atomic_settlement_template(
            left_owner=require_arg(args, "left_owner", method),
            left_recipient=require_arg(args, "left_recipient", method),
            left_asset_id=require_arg(args, "left_asset_id", method),
            left_amount=require_arg(args, "left_amount", method),
            right_owner=require_arg(args, "right_owner", method),
            right_recipient=require_arg(args, "right_recipient", method),
            right_asset_id=require_arg(args, "right_asset_id", method),
            right_amount=require_arg(args, "right_amount", method),
            condition=require_arg(args, "condition", method),
            finish_after=args.finish_after or 0,
            cancel_after=require_arg(args, "cancel_after", method),
            left_sequence=args.left_sequence,
            right_sequence=args.right_sequence,
        )
    if method == "escrow_info":
        return client.escrow_info(require_arg(args, "escrow_id", method))
    if method == "account_escrows":
        return client.account_escrows(
            require_arg(args, "address", method),
            role=args.role,
            state=args.state,
            limit=args.limit,
        )
    if method == "offer_info":
        return client.offer_info(require_arg(args, "offer_id", method))
    if method == "account_offers":
        return client.account_offers(
            require_arg(args, "address", method),
            state=args.state,
            limit=args.limit,
        )
    if method == "book_offers":
        return client.book_offers(
            require_arg(args, "taker_gets_asset_id", method),
            require_arg(args, "taker_pays_asset_id", method),
            limit=args.limit,
        )
    if method == "nft_info":
        return client.nft_info(require_arg(args, "nft_id", method))
    if method == "account_nfts":
        return client.account_nfts(
            require_arg(args, "address", method),
            include_burned=args.include_burned,
            limit=args.limit,
        )
    if method == "issuer_nfts":
        return client.issuer_nfts(
            require_arg(args, "issuer", method),
            collection_id=args.collection_id,
            include_burned=args.include_burned,
            limit=args.limit,
        )
    if method == "mempool_status":
        return client.mempool_status()
    if method == "bridge_status":
        return client.bridge_status()
    if method == "navcoin_bridge_routes":
        return client.navcoin_bridge_routes()
    if method == "navcoin_bridge_packet":
        return client.navcoin_bridge_packet(
            require_arg(args, "route_id", method),
            require_arg(args, "packet_hash", method),
        )
    if method == "navcoin_bridge_claims":
        return client.navcoin_bridge_claims(
            require_arg(args, "route_id", method),
            limit=args.limit,
            include_terminal=args.include_terminal,
        )
    if method == "navcoin_bridge_supply_status":
        return client.navcoin_bridge_supply_status(require_arg(args, "route_id", method))
    if method == "navcoin_bridge_receipt_replay":
        return client.navcoin_bridge_receipt_replay(require_arg(args, "route_id", method))
    if method == "shield_turnstile":
        return client.shield_turnstile()
    if method == "orchard_pool_report":
        return client.orchard_pool_report()
    if method == "account_tx_index_status":
        return client.account_tx_index_status()
    if method == "batch_archive":
        return client.batch_archive(
            batch_kind=args.batch_kind,
            batch_id=args.batch_id,
            limit=args.limit,
        )
    if method == "account_tx":
        return client.account_tx(
            require_arg(args, "address", method),
            from_height=args.from_height,
            to_height=args.to_height,
            limit=args.limit,
        )
    if method == "account_tx_history":
        return client.account_tx_history(
            require_arg(args, "address", method),
            from_height=args.from_height or 0,
            to_height=args.to_height,
            window_size=args.window_size,
            limit_per_window=args.limit_per_window,
            max_windows=args.max_windows,
            allow_truncated=args.allow_truncated,
        )
    if method == "archive_window":
        return client.archive_window(
            require_arg(args, "from_height", method),
            require_arg(args, "to_height", method),
            archive_uri=args.archive_uri,
        )
    if method in {"verify_blocks", "verify_state", "verify_bridge", "verify_mempool", "verify_shielded"}:
        return getattr(client, method)()
    if method == "shield_scan":
        return client.shield_scan(require_arg(args, "owner", method))
    if method == "shield_disclose":
        return client.shield_disclose(require_arg(args, "note_id", method))
    if method in {"vault_bridge_route", "vault_bridge_status", "nav_reserve_proof_status"}:
        return getattr(client, method)(require_arg(args, "asset_id", method))
    if method == "market_ops_status":
        return client.market_ops_status(require_arg(args, "asset_id", method), epoch=args.epoch)
    if method == "asset_orchard_action_status":
        return client.asset_orchard_action_status(
            (require_arg(args, "nullifier_1", method), require_arg(args, "nullifier_2", method)),
            (
                require_arg(args, "output_commitment_1", method),
                require_arg(args, "output_commitment_2", method),
            ),
        )
    if method == "fx_fix_list":
        return client.fx_fix_list(
            base_asset_id=args.base_asset_id,
            quote_asset_id=args.quote_asset_id,
            active_only=args.active_only,
            limit=args.limit,
        )
    if method == "fx_fix_info":
        return client.fx_fix_info(require_arg(args, "fix_packet_hash", method))
    if method == "fx_fix_reservation_info":
        return client.fx_fix_reservation_info(require_arg(args, "reservation_id", method))
    if method == "fx_fix_quote":
        return client.fx_fix_quote(
            require_arg(args, "fix_packet_hash", method),
            require_arg(args, "base_atoms", method),
        )
    if method == "pfusdc_ingress_preflight":
        return client.pfusdc_ingress_preflight(
            require_arg(args, "asset_id", method),
            recipient=require_arg(args, "recipient", method),
            depositor=require_arg(args, "depositor", method),
            amount_atoms=require_arg(args, "amount_atoms", method),
        )
    if method == "pfusdc_egress_witness":
        return client.pfusdc_egress_witness(
            require_arg(args, "withdrawal_id", method),
            prior_checkpoint=args.prior_checkpoint,
        )
    if method == "yolo_target_receipt":
        return client.yolo_target_receipt(require_arg(args, "registration_id", method))
    raise SystemExit(f"unsupported method: {method}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--endpoint", required=True, help="Endpoint as label=host:port")
    parser.add_argument("--method", required=True, choices=list(CLI_METHODS))
    parser.add_argument("--address")
    parser.add_argument("--route-id")
    parser.add_argument("--packet-hash")
    parser.add_argument("--left-owner")
    parser.add_argument("--left-recipient")
    parser.add_argument("--left-asset-id")
    parser.add_argument("--left-amount", type=int)
    parser.add_argument("--right-owner")
    parser.add_argument("--right-recipient")
    parser.add_argument("--right-asset-id")
    parser.add_argument("--right-amount", type=int)
    parser.add_argument("--condition")
    parser.add_argument("--finish-after", type=int)
    parser.add_argument("--cancel-after", type=int)
    parser.add_argument("--left-sequence", type=int)
    parser.add_argument("--right-sequence", type=int)
    parser.add_argument("--escrow-id")
    parser.add_argument("--offer-id")
    parser.add_argument("--taker-gets-asset-id")
    parser.add_argument("--taker-pays-asset-id")
    parser.add_argument("--nft-id")
    parser.add_argument("--issuer")
    parser.add_argument("--collection-id")
    parser.add_argument("--include-burned", action="store_true")
    parser.add_argument("--include-terminal", action="store_true")
    parser.add_argument("--role", choices=["owner", "recipient"])
    parser.add_argument("--state", choices=["open", "finished", "canceled", "filled", "unfunded"])
    parser.add_argument("--tx-id")
    parser.add_argument("--batch-kind")
    parser.add_argument("--batch-id")
    parser.add_argument("--from-height", type=int)
    parser.add_argument("--to-height", type=int)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--window-size", type=int, default=100)
    parser.add_argument("--limit-per-window", type=int, default=512)
    parser.add_argument("--max-windows", type=int, default=1000)
    parser.add_argument("--allow-truncated", action="store_true")
    parser.add_argument("--require-row", action="store_true")
    parser.add_argument("--audit-block-log", action="store_true")
    # Documented public reads (docs/rpc/method-coverage.md)
    parser.add_argument("--archive-uri")
    parser.add_argument("--owner")
    parser.add_argument("--note-id")
    parser.add_argument("--asset-id")
    parser.add_argument("--epoch", type=int)
    parser.add_argument("--nullifier-1")
    parser.add_argument("--nullifier-2")
    parser.add_argument("--output-commitment-1")
    parser.add_argument("--output-commitment-2")
    parser.add_argument("--base-asset-id")
    parser.add_argument("--quote-asset-id")
    parser.add_argument("--active-only", action="store_true")
    parser.add_argument("--fix-packet-hash")
    parser.add_argument("--reservation-id")
    parser.add_argument("--base-atoms", type=int)
    parser.add_argument("--recipient")
    parser.add_argument("--depositor")
    parser.add_argument("--amount-atoms", type=int)
    parser.add_argument("--withdrawal-id")
    parser.add_argument("--prior-checkpoint")
    parser.add_argument("--registration-id")
    parser.add_argument("--timeout-seconds", type=float, default=8.0)
    parser.add_argument("--response-byte-cap", type=int, default=1_048_576)
    parser.add_argument(
        "--output",
        default=f"reports/postfiat-rpc-query/postfiat-rpc-query-{utc_stamp()}.json",
    )
    return parser


def validate_args(args: argparse.Namespace) -> None:
    if args.timeout_seconds <= 0:
        raise SystemExit("--timeout-seconds must be positive")
    if args.response_byte_cap < 1024:
        raise SystemExit("--response-byte-cap must be at least 1024")
    for name in ("from_height", "to_height"):
        value = getattr(args, name)
        if value is not None and value < 0:
            raise SystemExit(f"--{name.replace('_', '-')} must be non-negative")
    if args.from_height is not None and args.to_height is not None and args.from_height > args.to_height:
        raise SystemExit("--from-height cannot exceed --to-height")
    for name in ("limit", "window_size", "limit_per_window", "max_windows"):
        value = getattr(args, name)
        if value is not None and value < 1:
            raise SystemExit(f"--{name.replace('_', '-')} must be positive")


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    validate_args(args)
    label, endpoint = parse_endpoint_ref(args.endpoint)
    client = PostFiatRpcClient(
        endpoint,
        timeout_seconds=args.timeout_seconds,
        response_byte_cap=args.response_byte_cap,
    )
    report: dict[str, Any] = {
        "schema": "postfiat-rpc-query-v1",
        "generated_utc": utc_iso(),
        "endpoint": {
            "label": label,
            "endpoint_hash": endpoint_hash(endpoint),
            "host": endpoint.host if endpoint.host in {"127.0.0.1", "localhost"} else "<redacted-host>",
            "port": endpoint.port,
        },
        "method": args.method,
        "ok": False,
        "checks": {},
        "result": None,
        "error": None,
    }
    try:
        result = call_method(client, args)
        report["result"] = value_to_json(result)
        report["checks"] = method_checks(args.method, result, args)
        report["ok"] = all(report["checks"].values())
    except (RpcClientError, OSError, ValueError, RuntimeError) as error:
        report["error"] = redact_network(str(error))
    report["sensitive_material_redacted"] = not report_has_sensitive_text(report)
    report["passed"] = report["ok"] and report["sensitive_material_redacted"]
    if not report["passed"] and report["error"] is None:
        report["error"] = "method result failed one or more checks"

    path = output_path(args.output)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"postfiat_rpc_query={path}")
    print(f"postfiat_rpc_query_passed={str(report['passed']).lower()}")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
