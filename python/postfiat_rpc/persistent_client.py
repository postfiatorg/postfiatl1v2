"""Persistent stdlib TCP client for bounded multi-call RPC sessions."""

from __future__ import annotations

import json
import socket
from threading import Lock
from typing import Any

from postfiat_rpc.client import RPC_VERSION, PostFiatRpcClient, RpcProtocolError, node_placeholder_error


class PersistentPostFiatRpcClient(PostFiatRpcClient):
    """Reuse one newline-framed TCP connection until explicitly closed.

    Calls are serialized because a connection has one ordered response stream.
    A connection carries exactly one response line per request, bound to that
    request by version and id; request ids are never reused on a connection, so
    a late or stray line can never match a later request. Any deviation, and any
    exception while a request is in flight, closes the session before raising,
    so the next call starts on a new connection. Failures are never retried
    automatically; retrying a mutation without application-level idempotency is
    unsafe.
    """

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        super().__init__(*args, **kwargs)
        self._stream: socket.socket | None = None
        self._receive_buffer = bytearray()
        self._sent_request_ids: set[str] = set()
        self._stream_lock = Lock()

    def __enter__(self) -> "PersistentPostFiatRpcClient":
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    def close(self) -> None:
        with self._stream_lock:
            self._close_unlocked()

    def _close_unlocked(self) -> None:
        if self._stream is not None:
            try:
                self._stream.close()
            except OSError:
                pass
        self._stream = None
        self._receive_buffer.clear()
        self._sent_request_ids.clear()

    def _connect_unlocked(self) -> socket.socket:
        if self._stream is None:
            self._stream = socket.create_connection(
                (self.endpoint.host, self.endpoint.port),
                timeout=self.timeout_seconds,
            )
            self._stream.settimeout(self.timeout_seconds)
        return self._stream

    def _read_line_unlocked(self, stream: socket.socket) -> bytes:
        while b"\n" not in self._receive_buffer:
            chunk = stream.recv(65536)
            if not chunk:
                raise RpcProtocolError("persistent RPC peer closed before response")
            self._receive_buffer.extend(chunk)
            if len(self._receive_buffer) > self.response_byte_cap:
                raise RpcProtocolError(
                    f"response exceeded byte cap {self.response_byte_cap}"
                )
        line, remainder = bytes(self._receive_buffer).split(b"\n", 1)
        self._receive_buffer[:] = remainder
        if len(line) > self.response_byte_cap:
            raise RpcProtocolError(f"response exceeded byte cap {self.response_byte_cap}")
        return line

    def _send(self, request: dict[str, Any]) -> dict[str, Any]:
        wire_id = json.dumps(request["id"])
        wire = json.dumps(request, separators=(",", ":")).encode("utf-8") + b"\n"
        with self._stream_lock:
            try:
                stream = self._connect_unlocked()
                if wire_id in self._sent_request_ids:
                    raise RpcProtocolError("request id reused on this persistent session")
                self._sent_request_ids.add(wire_id)
                stream.sendall(wire)
                raw = self._read_line_unlocked(stream)
                if self._receive_buffer:
                    raise RpcProtocolError(
                        "persistent RPC peer sent data beyond one response"
                    )
                try:
                    response = json.loads(raw.decode("utf-8"))
                except ValueError as error:
                    raise RpcProtocolError(f"response was not valid JSON: {error}") from error
                if not isinstance(response, dict):
                    raise RpcProtocolError("response envelope must be an object")
                if response.get("version") != RPC_VERSION:
                    raise RpcProtocolError("response version mismatch")
                if json.dumps(response.get("id")) != wire_id:
                    if not node_placeholder_error(response):
                        raise RpcProtocolError("response id mismatch")
                    # The node's error for this request, reported under its id. The node may send
                    # more than one such reply for a line it could not read whole, so the
                    # session is closed: no later reply can be paired one request behind.
                    response = {**response, "id": json.loads(wire_id)}
                    self._close_unlocked()
            except BaseException:
                self._close_unlocked()
                raise
        return response

