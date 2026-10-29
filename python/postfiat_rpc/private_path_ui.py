"""Read-only browser page for a validator's private-path state.

Run from the repository root against one validator's RPC::

    PYTHONPATH=python python3 -m postfiat_rpc.private_path_ui \\
      --endpoint validator-0=127.0.0.1:27650

``/api/state`` calls the node's public reads ``private_readiness``,
``private_pool_snapshot``, ``asset_orchard_pricing_claim`` (for the governed
A666/pfUSDC pair at the next height) and ``asset_orchard_private_egress_binding``
through the Python RPC client, and returns them with a rendered view.
``/api/receipt?intent_id=<96 hex>`` calls ``private_action_receipt``. Besides
the page and its two assets there is no other route, and every method but GET
is refused: the service never signs, submits or relays an action.
"""

from __future__ import annotations

import argparse
import json
import re
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Callable, Sequence
from urllib.parse import parse_qs, urlsplit

from postfiat_rpc.__main__ import parse_endpoint_ref, utc_iso
from postfiat_rpc.client import PostFiatRpcClient, RpcClientError, RpcError

STATE_SCHEMA = "postfiat.private_path_ui_state.v1"
DEFAULT_POOL_ID = "asset-orchard-v1"
INTENT_ID_RE = re.compile(r"^[0-9a-f]{96}$")
ASSET_DIR = Path(__file__).with_name("private_path_ui_assets")
STATIC_ROUTES = {
    "/": ("index.html", "text/html"),
    "/app.js": ("app.js", "text/javascript"),
    "/styles.css": ("styles.css", "text/css"),
}
SECURITY_HEADERS = {
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "referrer-policy": "no-referrer",
    "content-security-policy": "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; "
    "connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
}


def _read(call: Callable[[], dict[str, Any]]) -> dict[str, Any]:
    """One read: its result, or a refusal (the node answered ok=false) or a
    failure (transport or protocol) recorded instead of raising."""
    try:
        return {"result": call()}
    except RpcError as error:
        return {"refused": True, "error": str(error), "code": error.error.get("code", "rpc_error")}
    except (RpcClientError, OSError, ValueError) as error:
        return {"refused": False, "error": str(error)}


def read_private_path_state(
    client: PostFiatRpcClient,
    *,
    pool_id: str = DEFAULT_POOL_ID,
    base_asset_id: str | None = None,
    quote_asset_id: str | None = None,
) -> dict[str, Any]:
    """Every read the page shows, all against one validator."""
    readiness = _read(lambda: client.private_readiness(pool_id=pool_id))
    route = (readiness.get("result") or {}).get("a666_pfusdc_route") or {}
    base = base_asset_id or route.get("base_asset_id")
    quote = quote_asset_id or route.get("quote_asset_id")
    snapshot = _read(
        lambda: client.private_pool_snapshot(
            pool_id,
            base_asset_id=base if base and quote else None,
            quote_asset_id=quote if base and quote else None,
        )
    )
    finalized = (snapshot.get("result") or readiness.get("result") or {}).get("finalized_height")
    next_height = finalized + 1 if isinstance(finalized, int) else None
    pair_known = bool(base and quote and next_height)
    unknown_pair = {
        "refused": True,
        "error": "no A666/pfUSDC pair: pass --base-asset-id/--quote-asset-id or register the governed route",
        "code": "pair_unknown",
    }
    pricing = (
        _read(lambda: client.asset_orchard_pricing_claim(base, quote, next_height))
        if pair_known
        else unknown_pair
    )
    egress = (
        _read(lambda: client.asset_orchard_private_egress_binding(base, quote, next_height))
        if pair_known
        else unknown_pair
    )
    state = {
        "schema": STATE_SCHEMA,
        "pool_id": pool_id,
        "base_asset_id": base,
        "quote_asset_id": quote,
        "next_height": next_height,
        "readiness": readiness,
        "snapshot": snapshot,
        "pricing": pricing,
        "egress": egress,
    }
    state["view"] = build_view(state)
    return state


def _flat(value: Any, prefix: str = "") -> list[list[str]]:
    """Every leaf of a JSON value as [path, text] rows, in key order."""
    if isinstance(value, dict):
        rows: list[list[str]] = []
        for key in value:
            rows.extend(_flat(value[key], f"{prefix}.{key}" if prefix else str(key)))
        return rows
    if isinstance(value, list):
        if not value:
            return [[prefix, "[]"]]
        rows = []
        for index, item in enumerate(value):
            rows.extend(_flat(item, f"{prefix}[{index}]"))
        return rows
    if value is None:
        return [[prefix, "—"]]
    return [[prefix, value if isinstance(value, str) else json.dumps(value)]]


def _failed_section(title: str, read: dict[str, Any]) -> dict[str, Any]:
    refused = bool(read.get("refused"))
    return {
        "title": title,
        "status": "refused" if refused else "failed",
        "verdict": f"{'Refused' if refused else 'Failed'}: {read.get('error')}",
        "reasons": [read["code"]] if read.get("code") else [],
        "rows": [],
    }


def _readiness_section(read: dict[str, Any]) -> dict[str, Any]:
    if "result" not in read:
        return _failed_section("Readiness", read)
    report = read["result"]
    ready = report.get("ready") is True
    return {
        "title": "Readiness",
        "status": "ok" if ready else "not_ready",
        "verdict": "Ready for private actions" if ready else "Not ready for private actions",
        "reasons": list(report.get("reasons") or []),
        "rows": _flat({key: value for key, value in report.items() if key not in ("ready", "reasons")}),
    }


def _snapshot_section(read: dict[str, Any]) -> dict[str, Any]:
    if "result" not in read:
        return _failed_section("Pool snapshot", read)
    report = read["result"]
    completeness = report.get("completeness") or {}
    complete = completeness.get("complete") is True
    return {
        "title": "Pool snapshot",
        "status": "ok" if complete else "incomplete",
        "verdict": f"Snapshot at height {report.get('finalized_height')}: "
        + ("complete" if complete else "incomplete"),
        "reasons": [name for name, flag in completeness.items() if flag is False],
        "rows": _flat(
            {
                "snapshot_token": report.get("snapshot_token"),
                "pool_id": report.get("pool_id"),
                "pool_present": report.get("pool_present"),
                "counts": report.get("counts"),
                "page_limits": report.get("page_limits"),
                "completeness": completeness,
            }
        ),
    }


def _pricing_section(read: dict[str, Any]) -> dict[str, Any]:
    if "result" not in read:
        return _failed_section("Pricing claim", read)
    report = read["result"]
    status = str(report.get("status"))
    refusal = report.get("refusal") or {}
    return {
        "title": "Pricing claim",
        "status": "ok" if status == "ok" else "refused",
        "verdict": f"Claim at height {report.get('inclusion_height')}: {status}"
        + (f" — {refusal.get('reason')}" if refusal else ""),
        "reasons": [refusal["code"]] if refusal.get("code") else [],
        "rows": _flat(
            {
                "base_asset_id": report.get("base_asset_id"),
                "quote_asset_id": report.get("quote_asset_id"),
                "pricing_enforced": report.get("pricing_enforced"),
                "governed_route_id": report.get("governed_route_id"),
                "pricing_claim": report.get("pricing_claim"),
            }
        ),
    }


def _egress_section(read: dict[str, Any]) -> dict[str, Any]:
    if "result" not in read:
        return _failed_section("Egress binding", read)
    report = read["result"]
    status = str(report.get("status"))
    refusal = report.get("refusal") or {}
    return {
        "title": "Egress binding",
        "status": "ok" if status == "ok" else "ungoverned" if status == "ungoverned" else "refused",
        "verdict": f"Egress binding: {status}" + (f" — {refusal.get('reason')}" if refusal else ""),
        "reasons": [refusal["code"]] if refusal.get("code") else [],
        "rows": _flat({"bindings": report.get("bindings")}),
    }


def build_view(state: dict[str, Any]) -> dict[str, Any]:
    """The page's sections, each with a status, a verdict line, the reasons
    when it is not ok, and its fields as rows."""
    return {
        "sections": [
            _readiness_section(state["readiness"]),
            _snapshot_section(state["snapshot"]),
            _pricing_section(state["pricing"]),
            _egress_section(state["egress"]),
        ]
    }


def build_receipt_view(intent_id: str, read: dict[str, Any]) -> dict[str, Any]:
    title = f"Receipt {intent_id[:16]}…"
    if "result" not in read:
        return _failed_section(title, read)
    report = dict(read["result"])
    state = str(report.get("state") or "unknown")
    record = dict(report.get("record") or {})
    for key in ("action_json", "batch_json"):
        if isinstance(record.get(key), str):
            record[key] = f"<{len(record[key])} bytes>"
    if record:
        report["record"] = record
    outcome = record.get("outcome") or {}
    return {
        "title": title,
        "status": {"accepted": "ok", "rejected": "refused"}.get(state, "pending"),
        "verdict": f"Intent state: {state}",
        "reasons": [str(outcome["code"])] if state == "rejected" and outcome.get("code") else [],
        "rows": _flat(report),
    }


class PrivatePathUiServer(ThreadingHTTPServer):
    reader: Callable[[], dict[str, Any]]
    receipt_reader: Callable[[str], dict[str, Any]]


class PrivatePathUiHandler(BaseHTTPRequestHandler):
    server: PrivatePathUiServer

    def log_message(self, format_string: str, *args: object) -> None:
        print(f"private-path-ui: {format_string % args}")

    def __getattr__(self, name: str) -> Any:
        # Every request method other than GET, including unknown ones, is 405.
        if name.startswith("do_"):
            return self._method_not_allowed
        raise AttributeError(name)

    def _send(self, status: HTTPStatus, payload: bytes, content_type: str, **extra: str) -> None:
        self.send_response(status)
        self.send_header("content-type", f"{content_type}; charset=utf-8")
        self.send_header("content-length", str(len(payload)))
        self.send_header("cache-control", "no-store")
        for name, value in {**SECURITY_HEADERS, **extra}.items():
            self.send_header(name, value)
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(payload)

    def _json(self, status: HTTPStatus, value: dict[str, Any], **extra: str) -> None:
        self._send(status, json.dumps(value, sort_keys=True).encode(), "application/json", **extra)

    def _method_not_allowed(self) -> None:
        self._json(HTTPStatus.METHOD_NOT_ALLOWED, {"error": "read-only interface: GET only"}, allow="GET")

    def do_GET(self) -> None:  # noqa: N802
        parts = urlsplit(self.path)
        fetched_at = utc_iso()
        if parts.path == "/api/state":
            self._json(HTTPStatus.OK, {**self.server.reader(), "fetched_at": fetched_at})
            return
        if parts.path == "/api/receipt":
            values = parse_qs(parts.query).get("intent_id", [])
            intent_id = values[0] if len(values) == 1 else ""
            if not INTENT_ID_RE.fullmatch(intent_id):
                body = {"error": "intent_id must be 96 lowercase hex characters", "refused": True}
                self._json(HTTPStatus.BAD_REQUEST, {**body, "fetched_at": fetched_at})
                return
            read = self.server.receipt_reader(intent_id)
            body = {"intent_id": intent_id, "read": read, "view": build_receipt_view(intent_id, read)}
            self._json(HTTPStatus.OK, {**body, "fetched_at": fetched_at})
            return
        route = STATIC_ROUTES.get(parts.path)
        if route is None:
            self._json(HTTPStatus.NOT_FOUND, {"error": "not found"})
            return
        asset, content_type = route
        self._send(HTTPStatus.OK, (ASSET_DIR / asset).read_bytes(), content_type)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python -m postfiat_rpc.private_path_ui",
        description="Serve a read-only page with a validator's private-path readiness, pool and receipt state.",
    )
    parser.add_argument("--endpoint", required=True, help="the validator RPC as label=host:port")
    parser.add_argument("--pool-id", default=DEFAULT_POOL_ID)
    parser.add_argument("--base-asset-id", help="the pair's base asset id (default: the governed A666 route)")
    parser.add_argument("--quote-asset-id", help="the pair's quote asset id (default: the governed route)")
    parser.add_argument("--timeout-seconds", type=float, default=8.0)
    parser.add_argument("--port", type=int, default=8767, help="port on 127.0.0.1 (0 picks a free one)")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if not 0 <= args.port <= 65535:
        raise SystemExit("--port must be between 0 and 65535")
    if (args.base_asset_id is None) != (args.quote_asset_id is None):
        raise SystemExit("--base-asset-id and --quote-asset-id must be given together")
    label, endpoint = parse_endpoint_ref(args.endpoint)
    client = PostFiatRpcClient(endpoint, timeout_seconds=args.timeout_seconds)

    def reader() -> dict[str, Any]:
        state = read_private_path_state(
            client,
            pool_id=args.pool_id,
            base_asset_id=args.base_asset_id,
            quote_asset_id=args.quote_asset_id,
        )
        return {**state, "endpoint_label": label}

    server = PrivatePathUiServer(("127.0.0.1", args.port), PrivatePathUiHandler)
    server.reader = reader
    server.receipt_reader = lambda intent_id: _read(lambda: client.private_action_receipt(intent_id))
    host, port = server.server_address[:2]
    print(f"Private path state: http://{host}:{port}")
    print(f"Read-only: GET only, validator {label}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
