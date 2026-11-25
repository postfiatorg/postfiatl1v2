"""Fixed-upstream vsock transport with enclave-owned TLS.

The parent proxy never accepts a destination from enclave traffic.  Each proxy
port has one configured upstream, while TLS and HTTP remain inside the enclave.
"""

from __future__ import annotations

import http.client
import json
import socket
import ssl
import threading
from dataclasses import dataclass
from typing import Any, Callable, Mapping, Protocol
from urllib.parse import urlencode

from .canonical import canonical_json_bytes


MAX_HTTP_RESPONSE_BYTES = 32 * 1024 * 1024
MAX_HEADER_COUNT = 64


@dataclass(frozen=True)
class FixedUpstream:
    host: str
    port: int

    def __post_init__(self) -> None:
        if (
            not isinstance(self.host, str)
            or not self.host
            or any(character.isspace() for character in self.host)
            or "/" in self.host
        ):
            raise ValueError("fixed upstream host is invalid")
        if (
            not isinstance(self.port, int)
            or isinstance(self.port, bool)
            or not 1 <= self.port <= 65_535
        ):
            raise ValueError("fixed upstream port is invalid")


@dataclass(frozen=True)
class NitroHttpResponse:
    status_code: int
    headers: dict[str, str]
    content: bytes


class TlsSocketFactory(Protocol):
    def __call__(self) -> ssl.SSLSocket: ...


class NitroVsockTlsSocketFactory:
    """Open TLS inside the enclave over one parent-side fixed TCP proxy."""

    def __init__(
        self,
        *,
        parent_cid: int,
        proxy_port: int,
        server_hostname: str,
        ssl_context: ssl.SSLContext,
        timeout_seconds: float,
    ) -> None:
        if not hasattr(socket, "AF_VSOCK"):
            raise RuntimeError("this Python build does not support AF_VSOCK")
        if not isinstance(parent_cid, int) or parent_cid < 0:
            raise ValueError("parent_cid must be a non-negative integer")
        if not isinstance(proxy_port, int) or not 1 <= proxy_port <= 65_535:
            raise ValueError("proxy_port must be in 1..65535")
        if not isinstance(server_hostname, str) or not server_hostname:
            raise ValueError("TLS server hostname is required")
        if timeout_seconds <= 0:
            raise ValueError("timeout_seconds must be positive")
        self.parent_cid = parent_cid
        self.proxy_port = proxy_port
        self.server_hostname = server_hostname
        self.ssl_context = ssl_context
        self.timeout_seconds = timeout_seconds
        self._validate_tls_context()

    def _validate_tls_context(self) -> None:
        if (self.ssl_context.verify_mode != ssl.CERT_REQUIRED
                or not self.ssl_context.check_hostname):
            raise ValueError("enclave TLS requires certificate and hostname validation")

    def __call__(self) -> ssl.SSLSocket:
        self._validate_tls_context()
        raw = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
        raw.settimeout(self.timeout_seconds)
        try:
            raw.connect((self.parent_cid, self.proxy_port))
            return self.ssl_context.wrap_socket(
                raw,
                server_hostname=self.server_hostname,
            )
        except BaseException:
            raw.close()
            raise


def _header_name(value: str) -> bool:
    separators = set('()<>@,;:\\"/[]?={} \t')
    return bool(value) and all(
        33 <= ord(character) <= 126 and character not in separators
        for character in value
    )


class NitroHttpsJsonTransport:
    """Bounded HTTP/1.1 client whose socket is already enclave-owned TLS."""

    def __init__(
        self,
        *,
        socket_factory: TlsSocketFactory,
        host: str,
        max_response_bytes: int = MAX_HTTP_RESPONSE_BYTES,
    ) -> None:
        FixedUpstream(host, 443)
        if not isinstance(max_response_bytes, int) or max_response_bytes <= 0:
            raise ValueError("max_response_bytes must be positive")
        self.socket_factory = socket_factory
        self.host = host
        self.max_response_bytes = max_response_bytes

    def request(
        self,
        method: str,
        path: str,
        *,
        params: Mapping[str, str | int] | None = None,
        headers: Mapping[str, str] | None = None,
        body: bytes | None = None,
    ) -> NitroHttpResponse:
        if method not in {"GET", "POST"}:
            raise ValueError("Nitro HTTP method must be GET or POST")
        if (
            not isinstance(path, str)
            or not path.startswith("/")
            or path.startswith("//")
            or "#" in path
            or "\r" in path
            or "\n" in path
        ):
            raise ValueError("Nitro HTTP path must be an absolute origin-form path")
        query: list[tuple[str, str | int]] = []
        for key, value in (params or {}).items():
            if not isinstance(key, str) or not key or not isinstance(value, (str, int)):
                raise ValueError("Nitro HTTP query parameters must be scalar")
            if isinstance(value, bool):
                raise ValueError("Nitro HTTP query parameters must not be boolean")
            query.append((key, value))
        target = path
        if query:
            target += ("&" if "?" in target else "?") + urlencode(query)
        supplied = dict(headers or {})
        if len(supplied) > MAX_HEADER_COUNT:
            raise ValueError("Nitro HTTP header count exceeds the maximum")
        forbidden = {"host", "connection", "content-length", "transfer-encoding"}
        for name, value in supplied.items():
            if (
                not isinstance(name, str)
                or not _header_name(name)
                or name.lower() in forbidden
                or not isinstance(value, str)
                or "\r" in value
                or "\n" in value
            ):
                raise ValueError("Nitro HTTP header is invalid")
        body = b"" if body is None else body
        if not isinstance(body, bytes):
            raise TypeError("Nitro HTTP body must be bytes")
        request_headers = {
            "Host": self.host,
            "Connection": "close",
            "Accept": "application/json",
            "User-Agent": "postfiat-yolo-nitro/1",
            **supplied,
            "Content-Length": str(len(body)),
        }
        head = [f"{method} {target} HTTP/1.1"] + [
            f"{name}: {value}" for name, value in request_headers.items()
        ]
        wire = ("\r\n".join(head) + "\r\n\r\n").encode("ascii") + body

        tls_socket = self.socket_factory()
        try:
            tls_socket.sendall(wire)
            response = http.client.HTTPResponse(tls_socket)
            response.begin()
            content = response.read(self.max_response_bytes + 1)
            if len(content) > self.max_response_bytes:
                raise ValueError("Nitro HTTP response exceeds the maximum")
            response_headers: dict[str, str] = {}
            for name, value in response.getheaders():
                normalized = name.lower()
                if normalized in response_headers:
                    response_headers[normalized] += ", " + value
                else:
                    response_headers[normalized] = value
            return NitroHttpResponse(
                status_code=response.status,
                headers=response_headers,
                content=content,
            )
        finally:
            tls_socket.close()


class NitroSchwabApiClient:
    """Option-chain-only client with authorization inside enclave TLS."""

    def __init__(
        self,
        transport: NitroHttpsJsonTransport,
        *,
        access_token: Callable[[], str],
    ) -> None:
        self.transport = transport
        self.access_token = access_token

    def request(
        self,
        method: str,
        path: str,
        *,
        params: Mapping[str, str | int] | None = None,
        payload: Mapping[str, Any] | None = None,
    ) -> tuple[dict[str, Any], NitroHttpResponse]:
        if method != "GET" or path != "/marketdata/v1/chains" or payload is not None:
            raise ValueError("Nitro Schwab client supports only option-chain reads")
        token = self.access_token()
        if not isinstance(token, str) or not token:
            raise ValueError("Schwab access token is unavailable")
        body = None if payload is None else canonical_json_bytes(payload)
        headers = {"Authorization": "Bearer " + token}
        if body is not None:
            headers["Content-Type"] = "application/json"
        response = self.transport.request(
            method,
            path,
            params=params,
            headers=headers,
            body=body,
        )
        try:
            decoded = json.loads(response.content.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ValueError("Schwab response is not UTF-8 JSON") from exc
        if not isinstance(decoded, dict):
            raise ValueError("Schwab response must be a JSON object")
        return decoded, response


def relay_bidirectional(left: socket.socket, right: socket.socket) -> None:
    """Relay opaque bytes until both write directions reach EOF."""

    errors: list[BaseException] = []

    def copy(source: socket.socket, target: socket.socket) -> None:
        try:
            while data := source.recv(64 * 1024):
                target.sendall(data)
            try:
                target.shutdown(socket.SHUT_WR)
            except OSError:
                pass
        except BaseException as exc:
            errors.append(exc)
            for endpoint in (source, target):
                try:
                    endpoint.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass

    reverse = threading.Thread(target=copy, args=(right, left), daemon=True)
    reverse.start()
    copy(left, right)
    reverse.join()
    if errors:
        raise errors[0]


def serve_fixed_upstream_proxy(
    *,
    listen_port: int,
    upstream: FixedUpstream,
    connect_timeout_seconds: float,
) -> None:
    """Run the parent-side vsock proxy; one port maps to one fixed upstream."""

    if not hasattr(socket, "AF_VSOCK"):
        raise RuntimeError("this Python build does not support AF_VSOCK")
    if not isinstance(listen_port, int) or not 1 <= listen_port <= 65_535:
        raise ValueError("listen_port must be in 1..65535")
    if connect_timeout_seconds <= 0:
        raise ValueError("connect_timeout_seconds must be positive")
    listener = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
    listener.bind((socket.VMADDR_CID_ANY, listen_port))
    listener.listen()
    while True:
        enclave_socket, _ = listener.accept()
        try:
            upstream_socket = socket.create_connection(
                (upstream.host, upstream.port),
                timeout=connect_timeout_seconds,
            )
        except BaseException:
            enclave_socket.close()
            continue

        def relay(enclave_socket=enclave_socket, upstream_socket=upstream_socket) -> None:
            try:
                relay_bidirectional(enclave_socket, upstream_socket)
            finally:
                enclave_socket.close()
                upstream_socket.close()

        threading.Thread(target=relay, daemon=True).start()
