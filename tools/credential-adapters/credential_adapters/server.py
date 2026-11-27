"""Zero-PII HTTP webhook & sandbox server for W3C VC and ZK-Passport adapters."""

from __future__ import annotations

import json
import time
from datetime import datetime, timezone
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

from .types_c4 import SubjectDelegatedAcceptance, validate_hex96
from .w3c_vc_adapter import W3cVcIssuerAdapter, verify_webhook_hmac
from .zero_pii_store import ZeroPiiCredentialStore, assert_zero_pii_structure
from .zk_passport_adapter import ZkPassportSandboxAdapter

MAX_REQUEST_BODY_BYTES = 64 * 1024


class CredentialAdapterService:
    """In-process service facade backing the HTTP webhook & sandbox endpoints."""

    def __init__(
        self,
        *,
        store: ZeroPiiCredentialStore,
        w3c_adapter: W3cVcIssuerAdapter | None = None,
        zkp_adapter: ZkPassportSandboxAdapter | None = None,
    ) -> None:
        self.store = store
        self.w3c_adapter = w3c_adapter
        self.zkp_adapter = zkp_adapter

    def handle_post(
        self,
        path: str,
        raw_body: bytes,
        headers: dict[str, str],
    ) -> tuple[int, dict[str, Any]]:
        if len(raw_body) > MAX_REQUEST_BODY_BYTES:
            return HTTPStatus.REQUEST_ENTITY_TOO_LARGE, {"ok": False, "error": "payload_too_large"}
        try:
            body = json.loads(raw_body.decode("utf-8"))
            if not isinstance(body, dict):
                raise ValueError("json_object_required")
        except Exception:
            return HTTPStatus.BAD_REQUEST, {"ok": False, "error": "invalid_json"}

        try:
            if path == "/v1/w3c-vc/issue":
                if self.w3c_adapter is None:
                    return HTTPStatus.NOT_FOUND, {"ok": False, "error": "w3c_adapter_not_enabled"}
                if self.w3c_adapter.config.webhook_hmac_bytes is not None:
                    sig = headers.get("x-postfiat-webhook-signature", "")
                    if not verify_webhook_hmac(raw_body, sig, self.w3c_adapter.config.webhook_hmac_bytes):
                        return HTTPStatus.UNAUTHORIZED, {"ok": False, "error": "invalid_webhook_hmac"}
                acc_raw = body["subject_acceptance"]
                acceptance = SubjectDelegatedAcceptance(
                    subject=str(acc_raw["subject"]),
                    subject_pubkey_hex=str(acc_raw["subject_pubkey_hex"]),
                    subject_accept_signature_hex=str(acc_raw["subject_accept_signature_hex"]),
                    subject_accept_nonce=int(acc_raw["subject_accept_nonce"]),
                )
                now_utc = (
                    datetime.fromisoformat(str(body["now_utc"]).replace("Z", "+00:00"))
                    if body.get("now_utc")
                    else datetime.now(timezone.utc)
                )
                salt_bytes = bytes.fromhex(body["salt_hex"]) if body.get("salt_hex") else None
                result = self.w3c_adapter.issue_from_vc(
                    vc=body["verifiable_credential"],
                    subject_acceptance=acceptance,
                    current_block_height=int(body["current_block_height"]),
                    now_utc=now_utc,
                    expires_at_height=(
                        int(body["expires_at_height"]) if body.get("expires_at_height") is not None else None
                    ),
                    salt_bytes=salt_bytes,
                    request_nonce=body.get("request_nonce"),
                )
                assert_zero_pii_structure(result)
                return HTTPStatus.OK, {"ok": True, "result": result}

            if path == "/v1/w3c-vc/revoke":
                if self.w3c_adapter is None:
                    return HTTPStatus.NOT_FOUND, {"ok": False, "error": "w3c_adapter_not_enabled"}
                if self.w3c_adapter.config.webhook_hmac_bytes is not None:
                    sig = headers.get("x-postfiat-webhook-signature", "")
                    if not verify_webhook_hmac(raw_body, sig, self.w3c_adapter.config.webhook_hmac_bytes):
                        return HTTPStatus.UNAUTHORIZED, {"ok": False, "error": "invalid_webhook_hmac"}
                result = self.w3c_adapter.revoke(
                    credential_id=body.get("credential_id"),
                    external_vc_id=body.get("external_vc_id"),
                    revoked_at_height=int(body["revoked_at_height"]),
                    reason_code=str(body["reason_code"]),
                )
                assert_zero_pii_structure(result)
                return HTTPStatus.OK, {"ok": True, "result": result}

            if path == "/v1/zk-passport/verify-and-issue":
                if self.zkp_adapter is None:
                    return HTTPStatus.NOT_FOUND, {"ok": False, "error": "zkp_adapter_not_enabled"}
                acc_raw = body["subject_acceptance"]
                acceptance = SubjectDelegatedAcceptance(
                    subject=str(acc_raw["subject"]),
                    subject_pubkey_hex=str(acc_raw["subject_pubkey_hex"]),
                    subject_accept_signature_hex=str(acc_raw["subject_accept_signature_hex"]),
                    subject_accept_nonce=int(acc_raw["subject_accept_nonce"]),
                )
                now_unix = int(body.get("now_unix", int(time.time())))
                salt_bytes = bytes.fromhex(body["salt_hex"]) if body.get("salt_hex") else None
                result = self.zkp_adapter.verify_and_issue(
                    proof_envelope=body["proof_envelope"],
                    subject_acceptance=acceptance,
                    current_block_height=int(body["current_block_height"]),
                    now_unix=now_unix,
                    expires_at_height=(
                        int(body["expires_at_height"]) if body.get("expires_at_height") is not None else None
                    ),
                    salt_bytes=salt_bytes,
                    request_nonce=body.get("request_nonce"),
                )
                assert_zero_pii_structure(result)
                return HTTPStatus.OK, {"ok": True, "result": result}

            if path == "/v1/zk-passport/revoke":
                if self.zkp_adapter is None:
                    return HTTPStatus.NOT_FOUND, {"ok": False, "error": "zkp_adapter_not_enabled"}
                result = self.zkp_adapter.revoke(
                    credential_id=str(body["credential_id"]),
                    revoked_at_height=int(body["revoked_at_height"]),
                    reason_code=str(body["reason_code"]),
                )
                assert_zero_pii_structure(result)
                return HTTPStatus.OK, {"ok": True, "result": result}

            return HTTPStatus.NOT_FOUND, {"ok": False, "error": "route_not_found"}
        except KeyError as exc:
            # Never echo arbitrary request values in error responses.
            return HTTPStatus.BAD_REQUEST, {"ok": False, "error": f"missing_or_unknown:{exc.args[0]}"}
        except ValueError as exc:
            code = str(exc).split(":", 1)[0]
            return HTTPStatus.BAD_REQUEST, {"ok": False, "error": code}

    def handle_get(self, path: str) -> tuple[int, dict[str, Any]]:
        if path == "/health":
            return HTTPStatus.OK, {
                "ok": True,
                "schema": "postfiat.credential_adapter.health.v1",
                "w3c_vc_enabled": self.w3c_adapter is not None,
                "zk_passport_enabled": self.zkp_adapter is not None,
                "zero_pii_enforced": True,
            }
        if path.startswith("/v1/credentials/"):
            cred_id = path[len("/v1/credentials/") :]
            try:
                validate_hex96("credential_id", cred_id)
            except ValueError:
                return HTTPStatus.BAD_REQUEST, {"ok": False, "error": "invalid_credential_id"}
            record = self.store.get(cred_id)
            if record is None:
                return HTTPStatus.NOT_FOUND, {"ok": False, "error": "credential_not_found"}
            assert_zero_pii_structure(record)
            return HTTPStatus.OK, {"ok": True, "result": record}
        return HTTPStatus.NOT_FOUND, {"ok": False, "error": "route_not_found"}


def create_http_server(
    host: str,
    port: int,
    service: CredentialAdapterService,
) -> ThreadingHTTPServer:
    """Create a bounded zero-PII HTTP server (bind to 127.0.0.1 by default)."""

    class Handler(BaseHTTPRequestHandler):
        def _send_json(self, status: int, payload: dict[str, Any]) -> None:
            encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(encoded)

        def do_GET(self) -> None:  # noqa: N802
            status, payload = service.handle_get(self.path.split("?", 1)[0])
            self._send_json(status, payload)

        def do_POST(self) -> None:  # noqa: N802
            try:
                length = int(self.headers.get("Content-Length", "0"))
            except ValueError:
                self._send_json(HTTPStatus.BAD_REQUEST, {"ok": False, "error": "invalid_content_length"})
                return
            if length < 0 or length > MAX_REQUEST_BODY_BYTES:
                self._send_json(HTTPStatus.REQUEST_ENTITY_TOO_LARGE, {"ok": False, "error": "payload_too_large"})
                return
            raw_body = self.rfile.read(length)
            headers = {k.lower(): v for k, v in self.headers.items()}
            status, payload = service.handle_post(self.path.split("?", 1)[0], raw_body, headers)
            self._send_json(status, payload)

        def log_message(self, format: str, *args: Any) -> None:  # noqa: A002
            # Suppress default request logging to avoid leaking URL/header metadata.
            return

    return ThreadingHTTPServer((host, port), Handler)
