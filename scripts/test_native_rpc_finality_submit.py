from __future__ import annotations

import importlib.util
import json
import socket
import socketserver
import threading
from pathlib import Path

import pytest

SPEC = importlib.util.spec_from_file_location(
    "native_rpc_finality_submit",
    Path(__file__).with_name("native_rpc_finality_submit.py"),
)
mod = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(mod)


TX = {
    "unsigned": {"operation": "synthetic_asset_operation", "nonce": 7},
    "algorithm_id": "ML-DSA-65",
    "public_key_hex": "aa" * 32,
    "signature_hex": "bb" * 32,
}


def status_response(**overrides):
    value = {
        "chain_id": "postfiat-wan-devnet-2",
        "block_height": 776,
        "block_tip_hash": "11" * 48,
        "state_root": "22" * 48,
        "node_id": "validator-0",
    }
    value.update(overrides)
    return {"version": mod.RPC_VERSION, "id": "campaign-status", "ok": True, "result": value}


def finality_response(**overrides):
    result = {
        "schema": "postfiat-rpc-mempool-submit-signed-asset-transaction-finality-v1",
        "tx_id": "33" * 48,
        "round_ok": True,
        "finality": {
            "confirmed": True,
            "receipt": {"accepted": True},
            "block": {"header": {"height": 777}},
        },
    }
    result.update(overrides)
    return {"version": mod.RPC_VERSION, "id": "campaign", "ok": True, "result": result}


class RpcServer:
    def __init__(self, status, finality, close=False, delay=False):
        self.status = status
        self.finality = finality
        self.close = close
        self.delay = delay
        self.requests = []
        outer = self

        class Handler(socketserver.StreamRequestHandler):
            def handle(self):
                for index in range(2):
                    line = self.rfile.readline()
                    if not line:
                        return
                    outer.requests.append(json.loads(line))
                    if outer.close:
                        return
                    if outer.delay:
                        threading.Event().wait(0.2)
                    response = outer.status if index == 0 else outer.finality
                    self.wfile.write(json.dumps(response, separators=(",", ":")).encode() + b"\n")
                    self.wfile.flush()

        self.server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.thread.join(timeout=2)
        self.server.server_close()


def invoke(tmp_path, rpc, **kwargs):
    tx_path = tmp_path / "signed.json"
    tx_path.write_text(json.dumps(TX))
    output = tmp_path / "response.json"
    args = [
        "--signed-tx-file", str(tx_path),
        "--id", "campaign",
        "--output", str(output),
        "--socket-port", str(rpc.server.server_address[1]),
        "--readiness-timeout-ms", str(kwargs.pop("timeout_ms", 45000)),
    ]
    code = mod.main(args)
    return code, output


def test_green_status_pins_and_finality_envelope(tmp_path, capsys):
    with RpcServer(status_response(), finality_response()) as rpc:
        code, output = invoke(tmp_path, rpc)
    assert code == 0
    assert output.exists()
    assert json.loads(output.read_text())["ok"] is True
    assert [request["method"] for request in rpc.requests] == ["status", "mempool_submit_signed_asset_transaction_finality"]
    finality = rpc.requests[1]
    assert finality["version"] == mod.RPC_VERSION
    assert finality["id"] == "campaign"
    params = finality["params"]
    assert params["proxy_required_current_height"] == 776
    assert params["proxy_required_parent_hash"] == "11" * 48
    assert params["proxy_required_state_root"] == "22" * 48
    assert params["signed_asset_transaction_json"] == json.dumps(TX, separators=(",", ":"))
    assert "campaign 333333333333" in capsys.readouterr().out


@pytest.mark.parametrize(
    "response,needle",
    [
        ({"ok": False}, "not ok"),
        ({"result": {"round_ok": True, "finality": {"confirmed": False, "receipt": {"accepted": True}, "block": {"header": {"height": 1}}}}}, "confirmed"),
        ({"result": {"round_ok": True, "finality": {"confirmed": True, "receipt": {"accepted": False}, "block": {"header": {"height": 1}}}}}, "accepted"),
        ({"result": {"round_ok": False, "finality": {"confirmed": True, "receipt": {"accepted": True}, "block": {"header": {"height": 1}}}}}, "round_ok"),
    ],
)
def test_each_finality_gate_stops(tmp_path, response, needle):
    finality = finality_response()
    if "ok" in response:
        finality["ok"] = response["ok"]
    else:
        finality["result"].update(response["result"])
    with RpcServer(status_response(), finality) as rpc:
        code, _ = invoke(tmp_path, rpc)
    assert code == 2


@pytest.mark.parametrize("missing", ["block_height", "block_tip_hash", "state_root"])
def test_missing_status_pin_stops(tmp_path, missing):
    value = status_response()["result"]
    value.pop(missing)
    status = status_response()
    status["result"] = value
    with RpcServer(status, finality_response()) as rpc:
        code, _ = invoke(tmp_path, rpc)
    assert code == 2


def test_malformed_status_socket_closed_timeout_and_bad_tx_stop(tmp_path):
    with RpcServer({"not": "status"}, finality_response()) as rpc:
        code, _ = invoke(tmp_path, rpc)
    assert code == 2
    with RpcServer(status_response(), finality_response(), close=True) as rpc:
        code, _ = invoke(tmp_path, rpc)
    assert code == 2
    with RpcServer(status_response(), finality_response(), delay=True) as rpc:
        code, _ = invoke(tmp_path, rpc, timeout_ms=1)
    assert code == 2
    bad = tmp_path / "bad.json"
    bad.write_text(json.dumps({key: value for key, value in TX.items() if key != "signature_hex"}))
    with RpcServer(status_response(), finality_response()) as rpc:
        output = tmp_path / "bad-out.json"
        code = mod.main([
            "--signed-tx-file", str(bad), "--id", "bad", "--output", str(output),
            "--socket-port", str(rpc.server.server_address[1]),
        ])
    assert code == 2
