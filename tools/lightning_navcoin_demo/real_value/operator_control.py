"""Owner-only Unix control plane for signed value authorizations.

The browser HTTP API intentionally has no authorization endpoint.  Nazgul's
already-signed, single-use permit enters through this local socket; the
coordinator never loads the authorization private key.
"""

from __future__ import annotations

import errno
import json
import os
from pathlib import Path
import re
import socket
import socketserver
import stat
import struct
import threading
from typing import Any, Mapping

from .composition import CompositionError, load_strict_json
from .policy import ExecutionMode


CONTROL_REQUEST_SCHEMA = "postfiat.lightning_operator_control_request.v1"
CONTROL_RESPONSE_SCHEMA = "postfiat.lightning_operator_control_response.v1"
MAX_CONTROL_FRAME = 128 * 1024
SWAP_ID = re.compile(r"^[0-9a-f]{64}$")


class OperatorControlError(CompositionError):
    """A local operator-control request failed closed."""


class OperatorControl:
    def __init__(self, runtime: Any) -> None:
        self.runtime = runtime

    def dispatch(self, request: Any) -> Mapping[str, Any]:
        if not isinstance(request, Mapping):
            raise OperatorControlError("operator request must be an object")
        expected = frozenset({"schema", "action", "swap_id", "authorization"})
        if frozenset(request.keys()) != expected:
            raise OperatorControlError("operator request field set mismatch")
        if request["schema"] != CONTROL_REQUEST_SCHEMA:
            raise OperatorControlError("unsupported operator request schema")
        if request["action"] != "authorize_swap":
            raise OperatorControlError("unsupported operator action")
        swap_id = request["swap_id"]
        if type(swap_id) is not str or SWAP_ID.fullmatch(swap_id) is None:
            raise OperatorControlError("swap_id must be canonical lowercase hex")
        authorization = request["authorization"]
        if not isinstance(authorization, Mapping):
            raise OperatorControlError("authorization envelope must be an object")
        if self.runtime.policy.mode is not ExecutionMode.ARMED:
            raise OperatorControlError("operator authorization is disabled in DRY_RUN")
        return self.runtime.authorize_swap(swap_id, authorization)


def _decode_request(encoded: bytes) -> Mapping[str, Any]:
    if not encoded or len(encoded) > MAX_CONTROL_FRAME:
        raise OperatorControlError("operator request frame is empty or oversized")
    try:
        value = json.loads(
            encoded.decode("ascii"),
            object_pairs_hook=lambda pairs: _unique_pairs(pairs),
            parse_constant=lambda item: (_ for _ in ()).throw(
                OperatorControlError(f"non-finite JSON value: {item}")
            ),
        )
    except (UnicodeError, json.JSONDecodeError) as error:
        raise OperatorControlError("operator request is not valid ASCII JSON") from error
    if not isinstance(value, Mapping):
        raise OperatorControlError("operator request must be an object")
    return value


def _unique_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise OperatorControlError(f"duplicate operator request field: {key}")
        result[key] = value
    return result


def _canonical_line(value: Mapping[str, Any]) -> bytes:
    try:
        encoded = json.dumps(
            dict(value),
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        ).encode("ascii")
    except (TypeError, UnicodeError, ValueError) as error:
        raise OperatorControlError("operator response is not canonical JSON") from error
    if len(encoded) > MAX_CONTROL_FRAME:
        raise OperatorControlError("operator response is oversized")
    return encoded + b"\n"


class _UnixControlServer(socketserver.UnixStreamServer):
    allow_reuse_address = False

    def __init__(
        self,
        path: str,
        control: OperatorControl,
        expected_uid: int,
    ) -> None:
        self.control = control
        self.expected_uid = expected_uid
        super().__init__(path, _ControlHandler, bind_and_activate=True)


class _ControlHandler(socketserver.StreamRequestHandler):
    def handle(self) -> None:
        response: dict[str, Any]
        try:
            credentials = self.connection.getsockopt(
                socket.SOL_SOCKET,
                socket.SO_PEERCRED,
                struct.calcsize("3i"),
            )
            _pid, uid, _gid = struct.unpack("3i", credentials)
            if uid != self.server.expected_uid:  # type: ignore[attr-defined]
                raise OperatorControlError("operator peer uid is not authorized")
            frame = self.rfile.readline(MAX_CONTROL_FRAME + 1)
            if len(frame) > MAX_CONTROL_FRAME or not frame.endswith(b"\n"):
                raise OperatorControlError(
                    "operator request must be one bounded newline frame"
                )
            request = _decode_request(frame[:-1])
            result = self.server.control.dispatch(request)  # type: ignore[attr-defined]
            response = {
                "schema": CONTROL_RESPONSE_SCHEMA,
                "ok": True,
                "result": dict(result),
            }
        except Exception as error:
            message = str(error)
            if (
                not message
                or len(message) > 512
                or not message.isascii()
                or any(ord(character) < 0x20 for character in message)
            ):
                message = "operator request failed closed"
            response = {
                "schema": CONTROL_RESPONSE_SCHEMA,
                "ok": False,
                "error": {
                    "class": type(error).__name__,
                    "message": message,
                },
            }
        self.wfile.write(_canonical_line(response))
        self.wfile.flush()


def _prepare_socket_path(path: Path) -> None:
    try:
        parent = path.parent.lstat()
    except OSError as error:
        raise OperatorControlError("operator socket directory is unavailable") from error
    if (
        stat.S_ISLNK(parent.st_mode)
        or not stat.S_ISDIR(parent.st_mode)
        or parent.st_uid != os.geteuid()
        or parent.st_mode & (stat.S_IRWXG | stat.S_IRWXO)
    ):
        raise OperatorControlError(
            "operator socket directory must be owner-only and non-symlink"
        )
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return
    except OSError as error:
        raise OperatorControlError("operator socket path could not be inspected") from error
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISSOCK(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
    ):
        raise OperatorControlError("refusing to replace an unsafe operator socket path")
    probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    probe.settimeout(0.2)
    try:
        probe.connect(str(path))
    except OSError as error:
        if error.errno not in {
            errno.ECONNREFUSED,
            errno.ENOENT,
            errno.ECONNRESET,
        }:
            raise OperatorControlError(
                "operator socket liveness could not be classified"
            ) from error
    else:
        raise OperatorControlError("another operator control socket is active")
    finally:
        probe.close()
    path.unlink()


class OperatorControlServer:
    """One owner-only local control server, normally paired with a process lock."""

    def __init__(self, path: str | Path, runtime: Any) -> None:
        self.path = Path(path)
        if not self.path.is_absolute():
            raise OperatorControlError("operator socket path must be absolute")
        _prepare_socket_path(self.path)
        self._server = _UnixControlServer(
            str(self.path),
            OperatorControl(runtime),
            os.geteuid(),
        )
        os.chmod(self.path, 0o600)
        metadata = self.path.lstat()
        if (
            not stat.S_ISSOCK(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or metadata.st_mode & (stat.S_IRWXG | stat.S_IRWXO)
        ):
            self._server.server_close()
            raise OperatorControlError("operator socket failed mode/ownership checks")
        self._socket_inode = metadata.st_ino
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        if self._thread is not None:
            return
        thread = threading.Thread(
            target=self._server.serve_forever,
            kwargs={"poll_interval": 0.25},
            name="lightning-navcoin-operator-control",
            daemon=False,
        )
        thread.start()
        self._thread = thread

    def close(self) -> None:
        if self._thread is not None:
            self._server.shutdown()
            self._thread.join()
            if self._thread.is_alive():  # defensive: join() has no timeout
                raise OperatorControlError(
                    "operator control server did not terminate"
                )
            self._thread = None
        self._server.server_close()
        try:
            metadata = self.path.lstat()
        except FileNotFoundError:
            return
        if stat.S_ISSOCK(metadata.st_mode) and metadata.st_ino == self._socket_inode:
            self.path.unlink()

    def __enter__(self) -> "OperatorControlServer":
        self.start()
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()


def send_authorization(
    *,
    socket_path: str | Path,
    swap_id: str,
    authorization_path: str | Path,
    timeout_seconds: float = 10.0,
) -> Mapping[str, Any]:
    """Send one public signed envelope; no private signing key is accepted."""

    if type(swap_id) is not str or SWAP_ID.fullmatch(swap_id) is None:
        raise OperatorControlError("swap_id must be canonical lowercase hex")
    if (
        type(timeout_seconds) not in {int, float}
        or timeout_seconds <= 0
        or timeout_seconds > 60
    ):
        raise OperatorControlError("operator control timeout is invalid")
    envelope = load_strict_json(authorization_path, "value authorization envelope")
    request = {
        "schema": CONTROL_REQUEST_SCHEMA,
        "action": "authorize_swap",
        "swap_id": swap_id,
        "authorization": dict(envelope),
    }
    encoded = _canonical_line(request)
    if len(encoded) > MAX_CONTROL_FRAME:
        raise OperatorControlError("operator authorization request is oversized")
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.settimeout(float(timeout_seconds))
    try:
        client.connect(str(socket_path))
        client.sendall(encoded)
        buffer = bytearray()
        while not buffer.endswith(b"\n"):
            chunk = client.recv(min(4096, MAX_CONTROL_FRAME + 1 - len(buffer)))
            if not chunk:
                raise OperatorControlError("operator control closed without a response")
            buffer.extend(chunk)
            if len(buffer) > MAX_CONTROL_FRAME:
                raise OperatorControlError("operator control response is oversized")
    except OSError as error:
        raise OperatorControlError("operator control socket request failed") from error
    finally:
        client.close()
    response = _decode_request(bytes(buffer[:-1]))
    if response.get("schema") != CONTROL_RESPONSE_SCHEMA:
        raise OperatorControlError("operator control response schema mismatch")
    if response.get("ok") is not True:
        error = response.get("error")
        message = (
            error.get("message")
            if isinstance(error, Mapping)
            else "operator authorization failed closed"
        )
        raise OperatorControlError(str(message))
    result = response.get("result")
    if not isinstance(result, Mapping):
        raise OperatorControlError("operator control result is malformed")
    return result
