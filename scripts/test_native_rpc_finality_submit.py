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
        "block_height": 779,
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
        # August h714 precedent: deferred sends make round_ok false while
        # the finality response is still accepted.
        "certified_sends_deferred": True,
        "round_ok": False,
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
                    response = outer.status if outer.requests[-1].get("method") == "status" else outer.finality
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
    mapping = kwargs.get("validator_ports")
    if mapping is None:
        port = rpc.server.server_address[1]
        mapping = ",".join(f"validator-{index}:{port}" for index in range(6))
    args.extend(["--validator-ports", mapping])
    code = mod.main(args)
    return code, output


def test_green_status_pins_and_finality_envelope(tmp_path, capsys):
    with RpcServer(status_response(), finality_response()) as rpc:
        code, output = invoke(tmp_path, rpc)
    assert code == 0
    assert output.exists()
    saved = json.loads(output.read_text())
    assert saved["ok"] is True
    assert saved["result"]["certified_sends_deferred"] is True
    assert saved["result"]["round_ok"] is False
    assert saved["attempts"]
    assert [request["method"] for request in rpc.requests] == ["status", "mempool_submit_signed_asset_transaction_finality"]
    finality = rpc.requests[1]
    assert finality["version"] == mod.RPC_VERSION
    assert finality["id"] == "campaign"
    params = finality["params"]
    assert params["proxy_required_current_height"] == 779
    assert params["proxy_required_parent_hash"] == "11" * 48
    assert params["proxy_required_state_root"] == "22" * 48
    assert params["signed_asset_transaction_json"] == json.dumps(TX, separators=(",", ":"))
    assert "campaign 333333333333" in capsys.readouterr().out


def test_h714_precedent_shape_round_ok_false_is_accepted(tmp_path):
    precedent_path = Path(__file__).parents[1] / "deployments/pnok-private-fix-20260801/source-live-route-v2/deposit-500/pftl-finality/01-propose-h714/finality.responses.json"
    precedent = json.loads(precedent_path.read_text())[0]["response"]
    assert precedent["ok"] is True
    assert precedent["result"]["finality"]["confirmed"] is True
    assert precedent["result"]["finality"]["receipt"]["accepted"] is True
    assert precedent["result"]["certified_sends_deferred"] is True
    assert precedent["result"]["round_ok"] is False
    with RpcServer(status_response(block_height=713), precedent) as rpc:
        code, output = invoke(tmp_path, rpc)
    assert code == 0
    saved = json.loads(output.read_text())
    assert saved["result"]["round_ok"] is False
    assert saved["attempts"]


@pytest.mark.parametrize(
    "response,needle",
    [
        ({"ok": False}, "not ok"),
        ({"result": {"round_ok": True, "finality": {"confirmed": False, "receipt": {"accepted": True}, "block": {"header": {"height": 1}}}}}, "confirmed"),
        ({"result": {"round_ok": True, "finality": {"confirmed": True, "receipt": {"accepted": False}, "block": {"header": {"height": 1}}}}}, "accepted"),
        ({"result": {"certified_sends_deferred": False, "round_ok": False, "finality": {"confirmed": True, "receipt": {"accepted": True}, "block": {"header": {"height": 1}}}}}, "deferred"),
    ],
)
def test_each_finality_gate_stops(tmp_path, response, needle):
    finality = finality_response()
    if "ok" in response:
        finality["ok"] = response["ok"]
    else:
        finality["result"].update(response["result"])
    with RpcServer(status_response(), finality) as rpc:
        code, output = invoke(tmp_path, rpc)
    assert code == 2
    persisted = json.loads(output.read_text())
    assert persisted["attempts"]
    for key, value in finality.items():
        assert persisted[key] == value


@pytest.mark.parametrize("missing_path", ["tx_id", "height"])
def test_missing_required_finality_field_persists_response(tmp_path, missing_path):
    finality = finality_response()
    if missing_path == "tx_id":
        del finality["result"]["tx_id"]
    else:
        del finality["result"]["finality"]["block"]["header"]["height"]
    with RpcServer(status_response(), finality) as rpc:
        code, output = invoke(tmp_path, rpc)
    assert code == 2
    persisted = json.loads(output.read_text())
    assert persisted["result"] == finality["result"]
    assert len(persisted["attempts"]) == 1


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


def test_proposer_routing_targets_validator_three(tmp_path):
    with RpcServer(status_response(block_height=776), finality_response()) as initial, RpcServer(status_response(), finality_response()) as target:
        mapping = ",".join([
            f"validator-0:{initial.server.server_address[1]}",
            f"validator-1:{initial.server.server_address[1]}",
            f"validator-2:{initial.server.server_address[1]}",
            f"validator-3:{target.server.server_address[1]}",
            f"validator-4:{initial.server.server_address[1]}",
            f"validator-5:{initial.server.server_address[1]}",
        ])
        code, _ = invoke(tmp_path, initial, validator_ports=mapping)
    assert code == 0
    assert not any(request.get("method") != "status" for request in initial.requests)
    assert [request.get("method") for request in target.requests] == ["mempool_submit_signed_asset_transaction_finality"]


def test_wrong_proposer_reroutes_once(tmp_path):
    wrong = finality_response()
    wrong["ok"] = False
    wrong["error"] = {"code": "rpc_finality_wrong_proposer", "message": "retry the signed request at `validator-5`"}
    with RpcServer(status_response(block_height=779), wrong) as initial, RpcServer(status_response(), finality_response()) as target:
        mapping = ",".join([
            f"validator-0:{initial.server.server_address[1]}",
            f"validator-1:{initial.server.server_address[1]}",
            f"validator-2:{initial.server.server_address[1]}",
            f"validator-3:{initial.server.server_address[1]}",
            f"validator-4:{initial.server.server_address[1]}",
            f"validator-5:{target.server.server_address[1]}",
        ])
        code, output = invoke(tmp_path, initial, validator_ports=mapping)
    assert code == 0
    assert json.loads(output.read_text())["attempts"][0]["outcome"] == "rpc_finality_wrong_proposer"
    assert len(json.loads(output.read_text())["attempts"]) == 2


def test_wrong_proposer_exhaustion_attempts_each_validator_once(tmp_path):
    servers = []
    try:
        for index in range(6):
            response = finality_response()
            response["ok"] = False
            response["error"] = {"code": "rpc_finality_wrong_proposer", "message": f"retry the signed request at `validator-{(index + 1) % 6}`"}
            server = RpcServer(status_response(block_height=779), response)
            server.__enter__()
            servers.append(server)
        mapping = ",".join(f"validator-{index}:{server.server.server_address[1]}" for index, server in enumerate(servers))
        code, output = invoke(tmp_path, servers[0], validator_ports=mapping)
        assert code == 2
        assert output.exists()
        assert len(json.loads(output.read_text())["attempts"]) == 6
        # stdout is intentionally not used for a failed run; inspect each server's request count.
        assert sum(1 for server in servers for request in server.requests if request.get("method") == "mempool_submit_signed_asset_transaction_finality") == 6
    finally:
        for server in servers:
            server.__exit__(None, None, None)
