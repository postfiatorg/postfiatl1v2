from __future__ import annotations

import json
import socket
import ssl
import threading

import pytest

from pftl_schwab_tee import (
    FixedUpstream,
    NitroHttpResponse,
    NitroSchwabApiClient,
    NitroVsockTlsSocketFactory,
    relay_bidirectional,
)


def test_fixed_upstream_rejects_destination_injection() -> None:
    assert FixedUpstream("api.schwabapi.com", 443).host == "api.schwabapi.com"
    for host in ("", "api.schwabapi.com/other", "evil host"):
        with pytest.raises(ValueError, match="host"):
            FixedUpstream(host, 443)


def test_schwab_client_supplies_token_only_inside_transport() -> None:
    class Transport:
        def request(self, method, path, *, params, headers, body):
            self.request_value = (method, path, params, headers, body)
            return NitroHttpResponse(
                status_code=200,
                headers={"content-type": "application/json"},
                content=json.dumps({"symbol": "HOOD"}).encode(),
            )

    transport = Transport()
    client = NitroSchwabApiClient(transport, access_token=lambda: "secret-token")

    payload, response = client.request(
        "GET",
        "/marketdata/v1/chains",
        params={"symbol": "HOOD"},
    )

    assert payload == {"symbol": "HOOD"}
    assert response.status_code == 200
    assert transport.request_value == (
        "GET",
        "/marketdata/v1/chains",
        {"symbol": "HOOD"},
        {"Authorization": "Bearer secret-token"},
        None,
    )


def test_opaque_relay_moves_bytes_both_directions() -> None:
    enclave, proxy_left = socket.socketpair()
    proxy_right, upstream = socket.socketpair()
    for endpoint in (enclave, proxy_left, proxy_right, upstream):
        endpoint.settimeout(2)
    worker = threading.Thread(
        target=relay_bidirectional,
        args=(proxy_left, proxy_right),
    )
    worker.start()

    enclave.sendall(b"tls-client-ciphertext")
    assert upstream.recv(64) == b"tls-client-ciphertext"
    upstream.sendall(b"tls-server-ciphertext")
    assert enclave.recv(64) == b"tls-server-ciphertext"
    enclave.shutdown(socket.SHUT_WR)
    upstream.shutdown(socket.SHUT_WR)
    worker.join(timeout=2)

    assert not worker.is_alive()
    for endpoint in (enclave, proxy_left, proxy_right, upstream):
        endpoint.close()


@pytest.mark.parametrize("method,path,payload", [
    ("POST", "/trader/v1/accounts/example/orders", {}),
    ("GET", "/trader/v1/accounts", None),
    ("POST", "/marketdata/v1/chains", None),
    ("GET", "/marketdata/v1/chains?redirect=elsewhere", None),
    ("GET", "/marketdata/v1/chains", {}),
])
def test_schwab_boundary_has_no_write_or_arbitrary_route(method, path, payload):
    def token():
        pytest.fail("unauthorized route requested a bearer token")
    client = NitroSchwabApiClient(None, access_token=token)
    with pytest.raises(ValueError, match="only option-chain reads"):
        client.request(method, path, payload=payload)


def test_enclave_transport_requires_verified_tls_before_socket_creation(monkeypatch):
    context = ssl.create_default_context()
    settings = dict(parent_cid=3, proxy_port=8000, server_hostname="api.schwabapi.com",
                    ssl_context=context, timeout_seconds=1)
    factory = NitroVsockTlsSocketFactory(**settings)
    context.check_hostname = False
    with pytest.raises(ValueError, match="certificate and hostname"):
        NitroVsockTlsSocketFactory(**settings)
    context.verify_mode = ssl.CERT_NONE
    def socket_must_not_open(*args, **kwargs):
        pytest.fail("unverified TLS attempted a connection")
    monkeypatch.setattr(socket, "socket", socket_must_not_open)
    with pytest.raises(ValueError, match="certificate and hostname"):
        factory()


def test_parent_relays_retain_each_accepted_connection(monkeypatch):
    from pftl_schwab_tee import nitro_transport as transport

    class Endpoint:
        closed = False
        def close(self): self.closed = True
    class StopAccepting(Exception): pass
    incoming, outgoing = [Endpoint(), Endpoint()], [Endpoint(), Endpoint()]
    class Listener:
        count = 0
        def bind(self, address): pass
        def listen(self): pass
        def accept(self):
            if self.count == 2: raise StopAccepting()
            self.count += 1
            return incoming[self.count - 1], None
    pending, relayed = [], []
    class DeferredThread:
        def __init__(self, *, target, daemon): self.target = target
        def start(self): pending.append(self.target)
    destinations = iter(outgoing)
    monkeypatch.setattr(transport.socket, "socket", lambda *args: Listener())
    monkeypatch.setattr(transport.socket, "create_connection", lambda *args, **kwargs: next(destinations))
    monkeypatch.setattr(transport.threading, "Thread", DeferredThread)
    monkeypatch.setattr(transport, "relay_bidirectional", lambda left, right: relayed.append((left, right)))
    with pytest.raises(StopAccepting):
        transport.serve_fixed_upstream_proxy(
            listen_port=8000, upstream=FixedUpstream("api.schwabapi.com", 443),
            connect_timeout_seconds=1,
        )
    for target in pending: target()
    assert relayed == list(zip(incoming, outgoing))
    assert all(endpoint.closed for endpoint in incoming + outgoing)
