"""Loopback-only, secret-free HTTP surface for the wallet interface."""

from __future__ import annotations

from collections import defaultdict, deque
from dataclasses import dataclass
import hmac
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import re
import time
from typing import Any, Callable, Mapping, Protocol
from urllib.parse import urlsplit

from .policy import RealValuePolicyError


API_PREFIX = "/api/lightning-navcoin/v1"
MAX_REQUEST_BYTES = 64 * 1024
MAX_RESPONSE_BYTES = 256 * 1024
SWAP_ID = re.compile(r"^[0-9a-f]{64}$")
PFTL_TX_ID = re.compile(r"^[0-9a-f]{96}$")
CLIENT_REQUEST_ID = re.compile(r"^[0-9a-f]{64}$")
CSRF_TOKEN = re.compile(r"^[0-9a-f]{64}$")
DECIMAL_INTEGER = re.compile(r"^(0|[1-9][0-9]*)$")
PFTL_ADDRESS = re.compile(r"^pf[0-9a-f]{40}$")
FORBIDDEN_PUBLIC_KEY_MARKERS = (
    "preimage",
    "secret",
    "fulfillment",
    "macaroon",
    "private_key",
    "mnemonic",
    "seed",
    "backup",
)


class ApiError(RealValuePolicyError):
    def __init__(self, status: int, code: str, message: str) -> None:
        super().__init__(message)
        self.status = status
        self.code = code


class CoordinatorApiFacade(Protocol):
    """Business logic is injected; the HTTP layer never loads signer material."""

    def public_status(self) -> Mapping[str, Any]: ...

    def create_quote(self, request: Mapping[str, Any]) -> Mapping[str, Any]: ...

    def public_swap(self, swap_id: str) -> Mapping[str, Any]: ...

    def observe_user_lock(
        self, swap_id: str, tx_id: str
    ) -> Mapping[str, Any]: ...

    def observe_user_finish(
        self, swap_id: str, tx_id: str
    ) -> Mapping[str, Any]: ...

    def observe_user_cancel(
        self, swap_id: str, tx_id: str
    ) -> Mapping[str, Any]: ...


def _reject_duplicate_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ApiError(400, "duplicate_json_field", f"duplicate JSON field: {key}")
        result[key] = value
    return result


def _assert_secret_free(value: Any, path: str = "$", depth: int = 0) -> None:
    if depth > 12:
        raise ApiError(500, "unsafe_response", "public response exceeds depth limit")
    if isinstance(value, Mapping):
        if len(value) > 512:
            raise ApiError(500, "unsafe_response", "public response object is oversized")
        for key, child in value.items():
            if type(key) is not str:
                raise ApiError(500, "unsafe_response", "public response key is not text")
            lowered = key.lower()
            if any(marker in lowered for marker in FORBIDDEN_PUBLIC_KEY_MARKERS):
                raise ApiError(
                    500,
                    "unsafe_response",
                    f"secret-bearing response field is forbidden at {path}.{key}",
                )
            _assert_secret_free(child, f"{path}.{key}", depth + 1)
    elif isinstance(value, (list, tuple)):
        if len(value) > 1024:
            raise ApiError(500, "unsafe_response", "public response list is oversized")
        for index, child in enumerate(value):
            _assert_secret_free(child, f"{path}[{index}]", depth + 1)
    elif value is None or type(value) in {bool, int, str, float}:
        if type(value) is float:
            raise ApiError(500, "unsafe_response", "floats are forbidden in public responses")
        if isinstance(value, str) and len(value) > 64 * 1024:
            raise ApiError(500, "unsafe_response", "public response string is oversized")
    else:
        raise ApiError(500, "unsafe_response", "unsupported public response type")


def _validate_quote_request(value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ApiError(400, "invalid_request", "quote request must be an object")
    allowed = frozenset(
        {
            "direction",
            "amount_msat",
            "wallet_address",
            "invoice",
            "client_request_id",
        }
    )
    fields = frozenset(value.keys())
    if not fields.issubset(allowed):
        raise ApiError(400, "invalid_request", "quote request has unknown fields")
    required = {
        "direction",
        "amount_msat",
        "wallet_address",
        "client_request_id",
    }
    if not required.issubset(fields):
        raise ApiError(400, "invalid_request", "quote request is incomplete")
    direction = value["direction"]
    if direction not in {"lightning_to_pftl", "pftl_to_lightning"}:
        raise ApiError(400, "invalid_direction", "unsupported swap direction")
    amount = value["amount_msat"]
    if (
        type(amount) is not str
        or DECIMAL_INTEGER.fullmatch(amount) is None
        or int(amount) <= 0
        or int(amount) > (1 << 63) - 1
    ):
        raise ApiError(
            400,
            "invalid_amount",
            "amount_msat must be a canonical positive uint63 decimal string",
        )
    client_request_id = value["client_request_id"]
    if (
        type(client_request_id) is not str
        or CLIENT_REQUEST_ID.fullmatch(client_request_id) is None
    ):
        raise ApiError(
            400,
            "invalid_client_request_id",
            "client_request_id must be canonical lowercase 32-byte hex",
        )
    wallet = value["wallet_address"]
    if type(wallet) is not str or PFTL_ADDRESS.fullmatch(wallet) is None:
        raise ApiError(400, "invalid_wallet", "wallet_address is not canonical")
    invoice = value.get("invoice")
    if direction == "pftl_to_lightning":
        if (
            type(invoice) is not str
            or not invoice
            or invoice != invoice.lower()
            or not invoice.startswith("lnbc")
            or len(invoice) > 8192
        ):
            raise ApiError(
                400,
                "invalid_invoice",
                "off-ramp requires a canonical lowercase fixed-amount lnbc invoice",
            )
    elif invoice is not None:
        raise ApiError(400, "invalid_request", "on-ramp must not supply an invoice")
    return {
        "direction": direction,
        "amount_msat": int(amount),
        "wallet_address": wallet,
        "client_request_id": client_request_id,
        **({"invoice": invoice} if invoice is not None else {}),
    }


def _validate_pftl_receipt_notice(body: bytes) -> str:
    if not body:
        raise ApiError(400, "empty_body", "request body is required")
    try:
        request = json.loads(
            body.decode("utf-8"), object_pairs_hook=_reject_duplicate_object
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ApiError(400, "invalid_json", "request body is invalid JSON") from error
    if (
        not isinstance(request, Mapping)
        or frozenset(request.keys()) != frozenset({"tx_id"})
        or type(request["tx_id"]) is not str
        or PFTL_TX_ID.fullmatch(request["tx_id"]) is None
    ):
        raise ApiError(
            400,
            "invalid_pftl_tx_id",
            "receipt notice requires one canonical 48-byte PFTL tx_id",
        )
    return request["tx_id"]


@dataclass
class _RateLimiter:
    limit: int = 60
    window_seconds: int = 60

    def __post_init__(self) -> None:
        self._requests: dict[str, deque[float]] = defaultdict(deque)

    def admit(self, principal: str, now: float) -> None:
        bucket = self._requests[principal]
        threshold = now - self.window_seconds
        while bucket and bucket[0] <= threshold:
            bucket.popleft()
        if len(bucket) >= self.limit:
            raise ApiError(429, "rate_limited", "request rate limit exceeded")
        bucket.append(now)


class LightningNavcoinApi:
    def __init__(
        self,
        facade: CoordinatorApiFacade,
        *,
        session_token: bytes,
        allowed_origin: str = "http://127.0.0.1:5173",
        clock: Callable[[], float] = time.time,
    ) -> None:
        if type(session_token) is not bytes or len(session_token) < 32:
            raise ValueError("API session token must be at least 32 bytes")
        parsed = urlsplit(allowed_origin)
        if (
            parsed.scheme not in {"http", "https"}
            or not parsed.hostname
            or parsed.path
            or parsed.query
            or parsed.fragment
            or parsed.username is not None
            or parsed.password is not None
        ):
            raise ValueError("allowed_origin must be an exact origin")
        self.facade = facade
        self._session_token = session_token
        self.allowed_origin = allowed_origin
        self.clock = clock
        self.limiter = _RateLimiter()

    def _authenticated(self, headers: Mapping[str, str]) -> bool:
        candidate = headers.get("authorization", "")
        prefix = "Bearer "
        if not candidate.startswith(prefix):
            return False
        candidate = candidate.removeprefix(prefix)
        try:
            encoded = candidate.encode("ascii")
        except UnicodeEncodeError:
            return False
        return hmac.compare_digest(encoded, self._session_token.hex().encode("ascii"))

    @staticmethod
    def _browser_request_headers_valid(headers: Mapping[str, str]) -> bool:
        csrf = headers.get("x-postfiat-csrf", "")
        return (
            type(csrf) is str
            and CSRF_TOKEN.fullmatch(csrf) is not None
            and headers.get("x-requested-with") == "postfiat-wallet"
        )

    def dispatch(
        self,
        method: str,
        path: str,
        headers: Mapping[str, str],
        body: bytes,
        *,
        principal: str,
    ) -> tuple[int, dict[str, Any]]:
        self.limiter.admit(principal, self.clock())
        if path == f"{API_PREFIX}/status" and method == "GET":
            response = dict(self.facade.public_status())
            _assert_secret_free(response)
            return 200, {"ok": True, "result": response}
        if not self._authenticated(headers):
            raise ApiError(401, "unauthorized", "missing or invalid session")
        origin = headers.get("origin")
        if method != "GET" and origin != self.allowed_origin:
            raise ApiError(403, "origin_rejected", "request origin is not allowed")
        if method != "GET" and not self._browser_request_headers_valid(headers):
            raise ApiError(
                403,
                "browser_request_rejected",
                "required browser request headers are absent or invalid",
            )
        if path == f"{API_PREFIX}/quotes" and method == "POST":
            if not body:
                raise ApiError(400, "empty_body", "request body is required")
            try:
                request = json.loads(
                    body.decode("utf-8"), object_pairs_hook=_reject_duplicate_object
                )
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise ApiError(400, "invalid_json", "request body is invalid JSON") from error
            response = dict(self.facade.create_quote(_validate_quote_request(request)))
            _assert_secret_free(response)
            return 200, {"ok": True, "result": response}
        prefix = f"{API_PREFIX}/swaps/"
        action_match = re.fullmatch(
            rf"{re.escape(prefix)}([0-9a-f]{{64}})/(pftl-lock|pftl-finish|pftl-cancel)",
            path,
        )
        if method == "POST" and action_match is not None:
            swap_id, action = action_match.groups()
            tx_id = _validate_pftl_receipt_notice(body)
            if action == "pftl-lock":
                response = dict(self.facade.observe_user_lock(swap_id, tx_id))
            elif action == "pftl-finish":
                response = dict(self.facade.observe_user_finish(swap_id, tx_id))
            else:
                response = dict(self.facade.observe_user_cancel(swap_id, tx_id))
            _assert_secret_free(response)
            return 200, {"ok": True, "result": response}
        if method == "GET" and path.startswith(prefix):
            swap_id = path.removeprefix(prefix)
            if SWAP_ID.fullmatch(swap_id) is None:
                raise ApiError(400, "invalid_swap_id", "swap id is not canonical")
            response = dict(self.facade.public_swap(swap_id))
            _assert_secret_free(response)
            return 200, {"ok": True, "result": response}
        raise ApiError(404, "not_found", "API route not found")


def make_handler(api: LightningNavcoinApi) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        server_version = "PostFiatLightningNavcoin/1"
        sys_version = ""

        def _run(self) -> None:
            transfer_encoding = self.headers.get("Transfer-Encoding")
            lengths = self.headers.get_all("Content-Length", failobj=[]) or []
            if transfer_encoding is not None or len(lengths) > 1:
                self._write_error(
                    ApiError(
                        400,
                        "ambiguous_body_framing",
                        "request body framing is unsupported or ambiguous",
                    )
                )
                return
            content_length = self.headers.get("Content-Length", "0")
            try:
                size = int(content_length)
            except ValueError:
                self._write_error(ApiError(400, "invalid_length", "invalid Content-Length"))
                return
            if size < 0 or size > MAX_REQUEST_BYTES:
                self._write_error(ApiError(413, "body_too_large", "request body is oversized"))
                return
            body = self.rfile.read(size) if size else b""
            path = urlsplit(self.path).path
            try:
                status, response = api.dispatch(
                    self.command,
                    path,
                    {key.lower(): value for key, value in self.headers.items()},
                    body,
                    principal=self.client_address[0],
                )
                self._write_json(status, response)
            except ApiError as error:
                self._write_error(error)
            except Exception:
                self._write_error(
                    ApiError(500, "internal_error", "request failed closed")
                )

        def _write_error(self, error: ApiError) -> None:
            self._write_json(
                error.status,
                {
                    "ok": False,
                    "error": {"code": error.code, "message": str(error)},
                },
            )

        def _write_json(self, status: int, value: Mapping[str, Any]) -> None:
            encoded = json.dumps(
                dict(value),
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
                allow_nan=False,
            ).encode("ascii")
            if len(encoded) > MAX_RESPONSE_BYTES:
                status = 500
                encoded = b'{"error":{"code":"response_too_large","message":"response failed closed"},"ok":false}'
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=ascii")
            self.send_header("Content-Length", str(len(encoded)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Referrer-Policy", "no-referrer")
            self.end_headers()
            self.wfile.write(encoded)

        def do_GET(self) -> None:
            self._run()

        def do_POST(self) -> None:
            self._run()

        def log_message(self, _format: str, *_args: object) -> None:
            # Access logging belongs in a structured, secret-redacting wrapper.
            return

    return Handler


def serve_loopback(
    api: LightningNavcoinApi,
    *,
    host: str = "127.0.0.1",
    port: int = 18831,
) -> ThreadingHTTPServer:
    if host not in {"127.0.0.1", "::1"}:
        raise ValueError("real-value API must bind loopback only")
    if type(port) is not int or port <= 0 or port >= 65536:
        raise ValueError("API port is invalid")
    return ThreadingHTTPServer((host, port), make_handler(api))
