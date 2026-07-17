"""WAN transaction permutation matrix for controlled-testnet evidence.

The matrix command is intentionally conservative. It records accepted evidence
that already exists, runs read-only live probes, and uses invalid-payload probes
only to confirm whether wallet-facing write surfaces are disabled. It does not
guess transaction schemas or mutate state for unproven categories.
"""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path
from typing import Any, Callable

from .client import PostFiatWebSocketRpcClient, RpcError
from .wan_preflight import default_validator_endpoints, collect_rpc_preflight, summarize_preflight


DEFAULT_PROXY_URL = "ws://127.0.0.1:8080"
DEFAULT_REPORT_ROOT = Path("reports/transaction-improvement")


def latest_fleet_repair_dir(report_root: Path = DEFAULT_REPORT_ROOT) -> Path | None:
    if not report_root.exists():
        return None
    candidates = [
        path for path in report_root.iterdir()
        if path.is_dir() and path.name.endswith("-fleet-repair")
    ]
    if not candidates:
        return None
    return sorted(candidates, key=lambda path: path.name)[-1]


def _exists(root: Path | None, relative: str) -> dict[str, Any]:
    if root is None:
        return {"path": relative, "exists": False}
    path = root / relative
    return {"path": str(path), "exists": path.exists()}


def _evidence(root: Path | None, paths: list[str]) -> list[dict[str, Any]]:
    return [_exists(root, path) for path in paths]


def evidence_complete(items: list[dict[str, Any]]) -> bool:
    return bool(items) and all(item.get("exists") is True for item in items)


def rpc_probe(name: str, call: Callable[[], Any], *, summarize: Callable[[Any], Any] | None = None) -> dict[str, Any]:
    started = time.monotonic()
    try:
        result = call()
    except RpcError as error:
        return {
            "name": name,
            "ok": False,
            "duration_ms": round((time.monotonic() - started) * 1000, 3),
            "error": error.error,
            "error_type": type(error).__name__,
        }
    except Exception as error:  # noqa: BLE001 - matrix evidence keeps exact failure text.
        return {
            "name": name,
            "ok": False,
            "duration_ms": round((time.monotonic() - started) * 1000, 3),
            "error": {"code": type(error).__name__, "message": str(error)},
            "error_type": type(error).__name__,
        }
    if summarize is not None:
        result = summarize(result)
    return {
        "name": name,
        "ok": True,
        "duration_ms": round((time.monotonic() - started) * 1000, 3),
        "result": result,
    }


def disabled_probe(name: str, call: Callable[[], Any]) -> dict[str, Any]:
    probe = rpc_probe(name, call)
    if probe["ok"]:
        probe["classification"] = "unexpected_enabled"
    else:
        code = probe.get("error", {}).get("code")
        probe["classification"] = (
            "disabled" if code == "rpc_method_not_allowed" else "rejected"
        )
    return probe


def _wallet(path: str | Path) -> dict[str, Any]:
    with Path(path).open("r", encoding="utf-8") as handle:
        return json.load(handle)


def _account_summary(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        return {"raw_type": type(result).__name__}
    return {
        "address": result.get("address"),
        "balance": result.get("balance"),
        "sequence": result.get("sequence"),
        "public_key_hex_present": bool(result.get("public_key_hex")),
    }


def _owned_summary(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        return {"raw_type": type(result).__name__}
    return {
        "object_count": result.get("object_count"),
        "total_value": result.get("total_value"),
        "objects": [
            {
                "id": obj.get("id"),
                "value": obj.get("value"),
                "asset": obj.get("asset"),
                "version": obj.get("version"),
            }
            for obj in result.get("objects", [])
            if isinstance(obj, dict)
        ],
    }


def _list_count_summary(key: str) -> Callable[[Any], dict[str, Any]]:
    def summarize(result: Any) -> dict[str, Any]:
        if isinstance(result, list):
            return {"count": len(result)}
        if isinstance(result, dict):
            value = result.get(key)
            if isinstance(value, list):
                return {"count": len(value), "schema": result.get("schema")}
            return {"keys": sorted(result.keys())[:16], "schema": result.get("schema")}
        return {"raw_type": type(result).__name__}

    return summarize


def _bridge_summary(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        return {"raw_type": type(result).__name__}
    return {
        "domain_count": len(result.get("domains", [])) if isinstance(result.get("domains"), list) else None,
        "transfer_count": len(result.get("transfers", [])) if isinstance(result.get("transfers"), list) else None,
        "replay_cache_count": len(result.get("replay_cache", [])) if isinstance(result.get("replay_cache"), list) else None,
    }


def _orchard_summary(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        return {"raw_type": type(result).__name__}
    counters = result.get("counters", {})
    if not isinstance(counters, dict):
        counters = {}
    return {
        "passed": result.get("passed"),
        "pool_id": result.get("pool_id"),
        "output_count": counters.get("output_count"),
        "nullifier_count": counters.get("nullifier_count"),
        "retained_root_count": counters.get("retained_root_count"),
        "latest_retained_root": counters.get("latest_retained_root"),
    }


def _turnstile_summary(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        return {"raw_type": type(result).__name__}
    return {
        "event_count": result.get("event_count"),
        "orchard_deposit_total": result.get("orchard_deposit_total"),
        "withdraw_total": result.get("withdraw_total"),
        "migration_total": result.get("migration_total"),
    }


def _category(
    category: str,
    *,
    status: str,
    operations: list[dict[str, Any]],
    evidence: list[dict[str, Any]] | None = None,
    notes: list[str] | None = None,
) -> dict[str, Any]:
    return {
        "category": category,
        "status": status,
        "operations": operations,
        "evidence": evidence or [],
        "notes": notes or [],
    }


def build_matrix(
    *,
    proxy_url: str,
    evidence_root: Path | None,
    wallet_a_path: str | Path,
    wallet_b_path: str | Path,
    timeout_seconds: float,
    include_preflight: bool = True,
) -> dict[str, Any]:
    client = PostFiatWebSocketRpcClient(
        proxy_url,
        timeout_seconds=timeout_seconds,
        response_byte_cap=4 * 1024 * 1024,
    )
    wallet_a = _wallet(wallet_a_path)
    wallet_b = _wallet(wallet_b_path)

    server_info = rpc_probe("server_info", client.server_info, summarize=lambda result: {
        "node_id": result.get("node_id") if isinstance(result, dict) else None,
        "chain_id": result.get("chain_id") if isinstance(result, dict) else None,
        "protocol_version": result.get("protocol_version") if isinstance(result, dict) else None,
        "ledger": result.get("ledger") if isinstance(result, dict) else None,
        "rpc": result.get("rpc") if isinstance(result, dict) else None,
    })
    status = rpc_probe("status", client.status)
    capabilities = rpc_probe("server_capabilities", client.server_capabilities)

    read_probes = [
        rpc_probe("wallet_a_account", lambda: client.account(wallet_a["address"]), summarize=_account_summary),
        rpc_probe("wallet_b_account", lambda: client.account(wallet_b["address"]), summarize=_account_summary),
        rpc_probe(
            "wallet_a_owned_objects",
            lambda: client.owned_objects(wallet_a["public_key_hex"], asset="PFT", limit=32),
            summarize=_owned_summary,
        ),
        rpc_probe(
            "wallet_b_owned_objects",
            lambda: client.owned_objects(wallet_b["public_key_hex"], asset="PFT", limit=32),
            summarize=_owned_summary,
        ),
        rpc_probe("wallet_a_account_assets", lambda: client.account_assets(wallet_a["address"]), summarize=_list_count_summary("assets")),
        rpc_probe("wallet_a_account_lines", lambda: client.account_lines(wallet_a["address"]), summarize=_list_count_summary("lines")),
        rpc_probe("wallet_a_account_offers", lambda: client.account_offers(wallet_a["address"]), summarize=_list_count_summary("offers")),
        rpc_probe("bridge_status", client.bridge_status, summarize=_bridge_summary),
        rpc_probe("orchard_pool_report", client.orchard_pool_report, summarize=_orchard_summary),
        rpc_probe("shield_turnstile", client.shield_turnstile, summarize=_turnstile_summary),
    ]

    disabled_probes = [
        disabled_probe(
            "mempool_submit_signed_asset_transaction",
            lambda: client._call("mempool_submit_signed_asset_transaction", {"signed_asset_json": "{}"}),
        ),
        disabled_probe(
            "mempool_submit_signed_offer_transaction",
            lambda: client._call("mempool_submit_signed_offer_transaction", {"signed_offer_json": "{}"}),
        ),
        disabled_probe(
            "shield_batch_orchard_deposit",
            lambda: client._call("shield_batch_orchard_deposit", {"deposit_json": "{}"}),
        ),
        disabled_probe(
            "shield_batch_orchard_withdraw",
            lambda: client._call("shield_batch_orchard_withdraw", {"action_json": "{}"}),
        ),
        disabled_probe(
            "apply_bridge_batch",
            lambda: client._call("apply_bridge_batch", {"batch_json": "{}"}),
        ),
    ]

    native_evidence = _evidence(
        evidence_root,
        [
            "proxy-finality/response-1.json",
            "proxy-finality/response-2.json",
            "proxy-finality/response-3.json",
            "proxy-finality/response-4.json",
            "proxy-finality/response-5.json",
            "proxy-finality/response-6.json",
            "proxy-payment-v2-current-source/response.json",
        ],
    )
    fastpay_evidence = _evidence(
        evidence_root,
        [
            "fastpay-live/fastpay-fresh-proxy-cycle-20260628T0323Z.json",
            "fastpay-live/python-websocket-fastpay-cycle-20260628T0330Z.json",
            "post-python-fastpay-preflight.json",
        ],
    )

    disabled_by_name = {probe["name"]: probe for probe in disabled_probes}
    read_by_name = {probe["name"]: probe for probe in read_probes}

    categories = [
        _category(
            "Native account",
            status="accepted_partial",
            operations=[
                {"operation": "PFT transfer", "status": "accepted", "evidence": native_evidence[:6]},
                {"operation": "memo payment_v2", "status": "accepted", "evidence": native_evidence[6:]},
                {"operation": "sequence conflict", "status": "unproven"},
                {"operation": "insufficient funds", "status": "unproven"},
            ],
            evidence=native_evidence,
            notes=["Wallet-facing proxy routes native/payment_v2 finality to the deterministic proposer."],
        ),
        _category(
            "Trustlines/assets",
            status="disabled_current_wallet_endpoint",
            operations=[
                {"operation": "account_assets", "status": "read_only_probe", "probe": read_by_name["wallet_a_account_assets"]},
                {"operation": "account_lines", "status": "read_only_probe", "probe": read_by_name["wallet_a_account_lines"]},
                {
                    "operation": "mempool_submit_signed_asset_transaction",
                    "status": disabled_by_name["mempool_submit_signed_asset_transaction"]["classification"],
                    "probe": disabled_by_name["mempool_submit_signed_asset_transaction"],
                },
            ],
            notes=["Asset write finality is not exposed through the current wallet-facing proxy."],
        ),
        _category(
            "Offers/atomic",
            status="disabled_current_wallet_endpoint",
            operations=[
                {"operation": "account_offers", "status": "read_only_probe", "probe": read_by_name["wallet_a_account_offers"]},
                {
                    "operation": "mempool_submit_signed_offer_transaction",
                    "status": disabled_by_name["mempool_submit_signed_offer_transaction"]["classification"],
                    "probe": disabled_by_name["mempool_submit_signed_offer_transaction"],
                },
                {"operation": "atomic settlement template", "status": "unproven_live"},
            ],
            notes=["Offer write finality is not exposed through the current wallet-facing proxy."],
        ),
        _category(
            "FastPay",
            status="accepted_controlled_devnet_proxy",
            operations=[
                {"operation": "wrap", "status": "accepted_proxy_broadcast"},
                {"operation": "owned object lookup", "status": "read_only_probe", "probe": read_by_name["wallet_a_owned_objects"]},
                {"operation": "object send", "status": "accepted_proxy_broadcast"},
                {"operation": "unwrap", "status": "accepted_proxy_broadcast"},
                {"operation": "duplicate/replay rejection", "status": "unproven_live"},
            ],
            evidence=fastpay_evidence,
            notes=["Current FastPay evidence is proxy-broadcast controlled-devnet evidence, not block-finality evidence."],
        ),
        _category(
            "Orchard",
            status="read_only_only_on_wallet_endpoint",
            operations=[
                {"operation": "orchard_pool_report", "status": "read_only_probe", "probe": read_by_name["orchard_pool_report"]},
                {"operation": "shield_turnstile", "status": "read_only_probe", "probe": read_by_name["shield_turnstile"]},
                {
                    "operation": "shield_batch_orchard_deposit",
                    "status": disabled_by_name["shield_batch_orchard_deposit"]["classification"],
                    "probe": disabled_by_name["shield_batch_orchard_deposit"],
                },
                {
                    "operation": "shield_batch_orchard_withdraw",
                    "status": disabled_by_name["shield_batch_orchard_withdraw"]["classification"],
                    "probe": disabled_by_name["shield_batch_orchard_withdraw"],
                },
                {"operation": "nullifier replay rejection", "status": "unproven_live"},
            ],
        ),
        _category(
            "Asset-Orchard",
            status="unproven_live",
            operations=[
                {"operation": "ingress", "status": "unproven_live"},
                {"operation": "private swap", "status": "unproven_live"},
                {"operation": "egress/private egress", "status": "unproven_live"},
                {"operation": "invalid proof rejection", "status": "unproven_live"},
            ],
            notes=["Do not infer readiness from historical shielded-swap demos; this matrix needs current transaction-layer evidence."],
        ),
        _category(
            "Bridge batches",
            status="read_only_only_on_wallet_endpoint",
            operations=[
                {"operation": "bridge_status", "status": "read_only_probe", "probe": read_by_name["bridge_status"]},
                {
                    "operation": "apply_bridge_batch",
                    "status": disabled_by_name["apply_bridge_batch"]["classification"],
                    "probe": disabled_by_name["apply_bridge_batch"],
                },
            ],
            notes=["Current bridge_status shows no configured bridge domains on this wallet-facing endpoint."],
        ),
        _category(
            "Governance-safe no-op",
            status="unproven_live",
            operations=[
                {"operation": "governance batch rejected/accepted under fixture", "status": "unproven_live"},
            ],
        ),
    ]

    preflight: dict[str, Any] | None = None
    if include_preflight:
        entries = collect_rpc_preflight(default_validator_endpoints(), timeout_seconds=timeout_seconds)
        preflight = {
            "entries": entries,
            "summary": summarize_preflight(entries),
        }

    return {
        "schema": "postfiat-transaction-permutation-matrix-v1",
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "proxy_url": proxy_url,
        "evidence_root": str(evidence_root) if evidence_root is not None else None,
        "preflight": preflight,
        "live": {
            "server_info": server_info,
            "status": status,
            "capabilities": capabilities,
            "read_probes": read_probes,
            "disabled_probes": disabled_probes,
        },
        "categories": categories,
        "summary": summarize_matrix(categories),
    }


def summarize_matrix(categories: list[dict[str, Any]]) -> dict[str, Any]:
    counts: dict[str, int] = {}
    for category in categories:
        status = str(category.get("status", "unknown"))
        counts[status] = counts.get(status, 0) + 1
    unproven = [
        category["category"] for category in categories
        if "unproven" in str(category.get("status", "")) or any(
            "unproven" in str(operation.get("status", ""))
            for operation in category.get("operations", [])
            if isinstance(operation, dict)
        )
    ]
    disabled = [
        category["category"] for category in categories
        if "disabled" in str(category.get("status", ""))
        or "read_only" in str(category.get("status", ""))
    ]
    open_work = sorted(set(unproven + disabled))
    accepted = [
        category["category"] for category in categories
        if str(category.get("status", "")).startswith("accepted")
    ]
    return {
        "category_count": len(categories),
        "status_counts": counts,
        "accepted_categories": accepted,
        "disabled_or_read_only_categories": disabled,
        "unproven_categories": unproven,
        "open_work_categories": open_work,
    }


def markdown_report(report: dict[str, Any]) -> str:
    lines = [
        "# Transaction Permutation Matrix",
        "",
        f"Created: {report.get('created_at')}",
        f"Proxy: `{report.get('proxy_url')}`",
        f"Evidence root: `{report.get('evidence_root')}`",
        "",
    ]
    preflight = report.get("preflight")
    if isinstance(preflight, dict):
        summary = preflight.get("summary", {})
        lines.extend([
            "## Fleet Preflight",
            "",
            f"- Healthy: `{summary.get('healthy')}`",
            f"- Reachable: `{summary.get('reachable_count')}/{summary.get('validator_count')}`",
            f"- Largest ledger group: `{summary.get('largest_ledger_group')}`",
            f"- Red reasons: `{summary.get('red_reasons')}`",
            "",
        ])
    lines.extend([
        "## Categories",
        "",
        "| Category | Status | Key Notes |",
        "| --- | --- | --- |",
    ])
    for category in report.get("categories", []):
        notes = " ".join(category.get("notes", []))
        lines.append(f"| {category.get('category')} | `{category.get('status')}` | {notes} |")
    lines.extend(["", "## Open Work", ""])
    for item in report.get("summary", {}).get("open_work_categories", []):
        lines.append(f"- {item}")
    lines.append("")
    return "\n".join(lines)


def write_report(report: dict[str, Any], output: Path, markdown_output: Path | None) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    if markdown_output is not None:
        markdown_output.parent.mkdir(parents=True, exist_ok=True)
        markdown_output.write_text(markdown_report(report), encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proxy-url", default=DEFAULT_PROXY_URL)
    parser.add_argument("--evidence-root", default=None)
    parser.add_argument("--wallet-a", required=True)
    parser.add_argument("--wallet-b", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--markdown-output", default=None)
    parser.add_argument("--timeout-seconds", type=float, default=10.0)
    parser.add_argument("--no-preflight", action="store_true")
    args = parser.parse_args(argv)

    evidence_root = Path(args.evidence_root) if args.evidence_root else latest_fleet_repair_dir()
    report = build_matrix(
        proxy_url=args.proxy_url,
        evidence_root=evidence_root,
        wallet_a_path=args.wallet_a,
        wallet_b_path=args.wallet_b,
        timeout_seconds=args.timeout_seconds,
        include_preflight=not args.no_preflight,
    )
    output = Path(args.output)
    markdown_output = Path(args.markdown_output) if args.markdown_output else output.with_suffix(".md")
    write_report(report, output, markdown_output)
    print(f"transaction matrix written: {output}")
    print(f"transaction matrix markdown written: {markdown_output}")
    print(json.dumps(report["summary"], indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
