"""Pinned-TLS, macaroon-authenticated mainnet LND connector.

Generated protobuf modules remain supplied by the pinned LND v0.20.1 build
image.  This module deliberately accepts them as dependencies instead of
importing whichever protobuf package happens to be installed globally.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import stat
from typing import Any, Mapping
from urllib.parse import urlsplit

from ..coordinator.lnd_grpc import LndGrpcAdapter, LndRequestFactories
from .policy import RealValuePolicyError


CONNECTION_SCHEMA = "postfiat.lightning_mainnet_lnd_connection.v1"
HEX_32 = re.compile(r"^[0-9a-f]{64}$")
REVIEWED_LND_VERSION = "0.20.1-beta commit=v0.20.1-beta"
REVIEWED_LND_COMMIT_HASH = "848b72ce96eb68fa90fd4336523ca4c59bddcd4c"


class LndConnectionError(RealValuePolicyError):
    """A mainnet LND credential or transport pin failed validation."""


def _bounded_text(value: Any, name: str, maximum: int = 4096) -> str:
    if (
        type(value) is not str
        or not value
        or len(value) > maximum
        or not value.isascii()
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
    ):
        raise LndConnectionError(f"{name} must be bounded printable ASCII")
    return value


@dataclass(frozen=True)
class MainnetLndConnection:
    endpoint: str
    tls_server_name: str
    tls_cert_path: Path
    tls_cert_sha256: str
    macaroon_path: Path
    macaroon_sha256: str
    macaroon_profile: str
    ready_timeout_seconds: int

    FIELDS = frozenset(
        {
            "schema",
            "endpoint",
            "tls_server_name",
            "tls_cert_path",
            "tls_cert_sha256",
            "macaroon_path",
            "macaroon_sha256",
            "macaroon_profile",
            "ready_timeout_seconds",
        }
    )

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "MainnetLndConnection":
        if not isinstance(value, Mapping) or frozenset(value.keys()) != cls.FIELDS:
            raise LndConnectionError("LND connection field set mismatch")
        if value["schema"] != CONNECTION_SCHEMA:
            raise LndConnectionError("unsupported LND connection schema")
        endpoint = _bounded_text(value["endpoint"], "endpoint", 512)
        try:
            parsed_endpoint = urlsplit(f"//{endpoint}")
            endpoint_port = parsed_endpoint.port
        except ValueError as error:
            raise LndConnectionError(
                "endpoint must be an explicit loopback gRPC host:port"
            ) from error
        if (
            parsed_endpoint.hostname not in {"127.0.0.1", "localhost", "::1"}
            or endpoint_port is None
            or endpoint_port < 1
            or parsed_endpoint.username is not None
            or parsed_endpoint.password is not None
            or parsed_endpoint.path
            or parsed_endpoint.query
            or parsed_endpoint.fragment
        ):
            raise LndConnectionError(
                "endpoint must be an explicit loopback gRPC host:port"
            )
        server_name = _bounded_text(
            value["tls_server_name"], "tls_server_name", 253
        )
        for field in ("tls_cert_sha256", "macaroon_sha256"):
            item = value[field]
            if type(item) is not str or HEX_32.fullmatch(item) is None:
                raise LndConnectionError(f"{field} must be lowercase SHA-256 hex")
        if value["macaroon_profile"] != "LIGHTNING_NAVCOIN_RECEIVE_ONLY_V1":
            raise LndConnectionError(
                "the first-release receive-only baked macaroon is required"
            )
        timeout = value["ready_timeout_seconds"]
        if type(timeout) is not int or timeout <= 0 or timeout > 60:
            raise LndConnectionError("ready timeout must be within [1, 60]")
        return cls(
            endpoint=endpoint,
            tls_server_name=server_name,
            tls_cert_path=Path(_bounded_text(value["tls_cert_path"], "tls_cert_path")),
            tls_cert_sha256=value["tls_cert_sha256"],
            macaroon_path=Path(_bounded_text(value["macaroon_path"], "macaroon_path")),
            macaroon_sha256=value["macaroon_sha256"],
            macaroon_profile=value["macaroon_profile"],
            ready_timeout_seconds=timeout,
        )


def _read_pinned_file(
    path: Path,
    *,
    expected_sha256: str,
    name: str,
    private: bool,
    maximum: int = 1 << 20,
) -> bytes:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise LndConnectionError(f"{name} is unavailable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise LndConnectionError(f"{name} is not a regular non-symlink file")
    if metadata.st_size <= 0 or metadata.st_size > maximum:
        raise LndConnectionError(f"{name} size is invalid")
    if private:
        if metadata.st_uid != os.geteuid():
            raise LndConnectionError(f"{name} is not owned by the coordinator user")
        if metadata.st_mode & (stat.S_IRWXG | stat.S_IRWXO):
            raise LndConnectionError(f"{name} must not grant group/other access")
    try:
        value = path.read_bytes()
    except OSError as error:
        raise LndConnectionError(f"{name} could not be read") from error
    if not hashlib.sha256(value).hexdigest() == expected_sha256:
        raise LndConnectionError(f"{name} SHA-256 pin mismatch")
    return value


@dataclass(frozen=True)
class ConnectedMainnetLnd:
    channel: Any
    adapter: LndGrpcAdapter


def connect_mainnet_lnd(
    config: MainnetLndConnection,
    *,
    lightning_pb2: Any,
    lightning_pb2_grpc: Any,
    router_pb2: Any,
    router_pb2_grpc: Any,
    grpc_module: Any | None = None,
) -> ConnectedMainnetLnd:
    """Create one TLS channel without putting a macaroon in argv or logs."""

    if grpc_module is None:
        try:
            import grpc as grpc_module
        except ImportError as error:
            raise LndConnectionError("grpcio is required for mainnet LND") from error
    tls_cert = _read_pinned_file(
        config.tls_cert_path,
        expected_sha256=config.tls_cert_sha256,
        name="LND TLS certificate",
        private=False,
    )
    macaroon = _read_pinned_file(
        config.macaroon_path,
        expected_sha256=config.macaroon_sha256,
        name="LND macaroon",
        private=True,
    )
    macaroon_hex = macaroon.hex()

    def metadata_callback(context: Any, callback: Any) -> None:
        del context
        callback((("macaroon", macaroon_hex),), None)

    ssl_credentials = grpc_module.ssl_channel_credentials(tls_cert)
    macaroon_credentials = grpc_module.metadata_call_credentials(metadata_callback)
    credentials = grpc_module.composite_channel_credentials(
        ssl_credentials, macaroon_credentials
    )
    channel = grpc_module.secure_channel(
        config.endpoint,
        credentials,
        options=(
            ("grpc.ssl_target_name_override", config.tls_server_name),
            ("grpc.default_authority", config.tls_server_name),
            ("grpc.max_receive_message_length", 16 * 1024 * 1024),
            ("grpc.max_send_message_length", 16 * 1024 * 1024),
        ),
    )
    try:
        grpc_module.channel_ready_future(channel).result(
            timeout=config.ready_timeout_seconds
        )
    except Exception as error:
        channel.close()
        raise LndConnectionError("mainnet LND gRPC endpoint did not become ready") from error
    lightning_stub = lightning_pb2_grpc.LightningStub(channel)
    router_stub = router_pb2_grpc.RouterStub(channel)
    adapter = LndGrpcAdapter(
        lightning_stub,
        router_stub,
        LndRequestFactories.from_proto_modules(lightning_pb2, router_pb2),
        network="bitcoin",
        expected_version=REVIEWED_LND_VERSION,
        expected_commit_hash=REVIEWED_LND_COMMIT_HASH,
    )
    return ConnectedMainnetLnd(channel=channel, adapter=adapter)
