"""Pinned LND v0.20.1-beta Python gRPC bootstrap for the synthetic demo."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import grpc
import lightning_pb2
import lightning_pb2_grpc
import router_pb2
import router_pb2_grpc


NODE_ENDPOINTS = {
    "user": "172.30.24.11:10009",
    "coordinator": "172.30.24.12:10009",
    "router": "172.30.24.13:10009",
}


def _read_exact(path: Path, *, maximum: int) -> bytes:
    try:
        size = path.stat().st_size
    except OSError as error:
        raise RuntimeError(f"required LND credential is unavailable: {path}") from error
    if size <= 0 or size > maximum:
        raise RuntimeError(f"invalid LND credential size: {path}")
    try:
        return path.read_bytes()
    except OSError as error:
        raise RuntimeError(f"could not read LND credential: {path}") from error


def connect_lnd(
    node: str,
    *,
    state_dir: str | Path = "/state",
    ready_timeout_seconds: float = 15.0,
) -> tuple[
    grpc.Channel,
    Any,
    Any,
    Any,
    Any,
]:
    """Return channel, Lightning/Router stubs, and their generated pb2 modules.

    The node name selects an exact endpoint on the non-routed synthetic subnet;
    callers cannot use this helper to dial a public Lightning node.
    """

    if node not in NODE_ENDPOINTS:
        raise ValueError("node must be user, coordinator, or router")
    if ready_timeout_seconds <= 0 or ready_timeout_seconds > 60:
        raise ValueError("ready timeout must be within (0, 60] seconds")

    root = Path(state_dir)
    tls_cert = _read_exact(root / f"lnd-{node}" / "tls.cert", maximum=1 << 20)
    macaroon = _read_exact(
        root
        / f"lnd-{node}"
        / "data"
        / "chain"
        / "bitcoin"
        / "regtest"
        / "admin.macaroon",
        maximum=1 << 20,
    )
    macaroon_hex = macaroon.hex()

    def metadata_callback(
        context: grpc.AuthMetadataContext,
        callback: grpc.AuthMetadataPluginCallback,
    ) -> None:
        del context
        callback((("macaroon", macaroon_hex),), None)

    ssl_credentials = grpc.ssl_channel_credentials(tls_cert)
    macaroon_credentials = grpc.metadata_call_credentials(metadata_callback)
    credentials = grpc.composite_channel_credentials(
        ssl_credentials, macaroon_credentials
    )
    channel = grpc.secure_channel(
        NODE_ENDPOINTS[node],
        credentials,
        options=(
            ("grpc.max_receive_message_length", 16 * 1024 * 1024),
            ("grpc.max_send_message_length", 16 * 1024 * 1024),
        ),
    )
    try:
        grpc.channel_ready_future(channel).result(timeout=ready_timeout_seconds)
    except grpc.FutureTimeoutError as error:
        channel.close()
        raise RuntimeError(f"LND gRPC endpoint did not become ready: {node}") from error

    return (
        channel,
        lightning_pb2_grpc.LightningStub(channel),
        router_pb2_grpc.RouterStub(channel),
        lightning_pb2,
        router_pb2,
    )


__all__ = [
    "NODE_ENDPOINTS",
    "connect_lnd",
    "lightning_pb2",
    "lightning_pb2_grpc",
    "router_pb2",
    "router_pb2_grpc",
]
