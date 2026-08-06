#!/usr/bin/env python3
"""Hermetic tests for the canonical Ethereum ingress-proof descriptor path."""
from __future__ import annotations

import copy
import importlib.util
import json
import os
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread

import pytest


SPEC = importlib.util.spec_from_file_location(
    "native_prover_leaf", Path(__file__).with_name("native_prover_leaf.py")
)
leaf = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(leaf)

TX = "016f9c5f9b99fc951cea7c539f7f791c2d753d35793296b2e284f96512575924"
NONCE = "fe457c66ab796d980ffcabf557ae7c13c60eefca01a276e5b023e812418b04b6"
ROUTE = "caec1d48fd3112116a96ec6fcf4a1428a190957962dfb042b811a72ff0d02d93"
DEPOSIT_ID = "e54713583c1bb46e908e8f01f1c996966dc5c82281365c91092a27ae0852d02b"
DEPOSITOR = "1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"
RECIPIENT = "pfab9b9228942e5c529633a13aa271d5297bec6353"


def _word(value: int | str) -> str:
    if isinstance(value, str):
        value = int(value, 16)
    return f"{value:064x}"


def _receipt() -> dict:
    # This is the public receipt fixture for the 10-USDC deposit.  The event
    # payload is the ABI encoding returned by the mainnet receipt.
    recipient_hex = RECIPIENT.encode().hex()
    data = "".join(
        [
            _word(0xE0),
            _word(10_000_000),
            NONCE,
            ROUTE,
            _word(1),
            _word("aaa78fda7062efce769e95cd72fc55e507bc8183"),
            _word("a0b86991c6218b36c1d19d4a2e9eb0ce3606eB48"),
            _word(len(RECIPIENT)),
            recipient_hex.ljust(64, "0"),
        ]
    )
    return {
        "status": "0x1",
        "blockNumber": "0x1882006",
        "transactionHash": "0x" + TX,
        "from": "0x" + DEPOSITOR,
        "to": leaf.EXPECTED_VAULT_ADDRESS,
        "logs": [
            {
                "address": leaf.EXPECTED_VAULT_ADDRESS,
                "topics": [
                    "0x7564437da24aa33f24442c214d7047d8bf275a86555bc57b83be448783cd6d81",
                    "0x" + DEPOSIT_ID,
                    "0x" + "0" * 24 + DEPOSITOR,
                    "0x13f03839b7fd67fe8d84d5a0d6e7bb3fef580e23d9872e0bd442d364cb0d4bb0",
                ],
                "data": "0x" + data,
            }
        ],
    }


def _report(**overrides: object) -> dict:
    report = {
        "vault_address": leaf.EXPECTED_VAULT_ADDRESS,
        "usdc_address": leaf.EXPECTED_TOKEN_ADDRESS,
        "stakehub_wallet": "0x" + DEPOSITOR,
        "pftl_recipient": RECIPIENT,
        "amount_atoms": 10_000_000,
        "nonce": "0x" + NONCE,
        "deposit_tx": TX,
    }
    report.update(overrides)
    return report


def _descriptor(report: dict) -> dict:
    return leaf._build_deployment_descriptor(report, ROUTE)


def test_stale_h390_descriptor_fails_against_current_receipt(monkeypatch: pytest.MonkeyPatch) -> None:
    stale = json.loads(Path("/tmp/krimp-gpu/h390-deployment.json").read_text())
    stale_report = _report(
        deposit_tx=stale["deposit_tx"],
        amount_atoms=stale["amount_atoms"],
        pftl_recipient=stale["recipient"],
        nonce=stale["nonce"],
        route_binding=stale["route_binding"],
    )
    descriptor = _descriptor(stale_report)
    monkeypatch.setattr(leaf, "_rpc_receipt", lambda _url, _tx: _receipt())
    with pytest.raises(RuntimeError, match="transactionHash mismatch"):
        leaf._verify_canonical_receipt(["fixture://mainnet"], descriptor, stale_report)


def test_current_descriptor_and_receipt_pass(monkeypatch: pytest.MonkeyPatch) -> None:
    report = _report()
    descriptor = _descriptor(report)
    monkeypatch.setattr(leaf, "_rpc_receipt", lambda _url, _tx: _receipt())
    assert leaf._verify_canonical_receipt(["fixture://mainnet"], descriptor, report) == (
        "0x" + DEPOSIT_ID,
        25_698_310,
    )
    assert descriptor["route_binding"] == ROUTE
    assert descriptor["creation_bytecode_hash"].startswith("0x")


def test_wrong_tx_and_nonce_are_rejected(monkeypatch: pytest.MonkeyPatch) -> None:
    report = _report()
    descriptor = _descriptor(report)
    monkeypatch.setattr(leaf, "_rpc_receipt", lambda _url, _tx: _receipt())
    wrong_tx = dict(descriptor, deposit_tx="0x" + "11" * 32)
    with pytest.raises(RuntimeError, match="transactionHash mismatch"):
        leaf._verify_canonical_receipt(["fixture://mainnet"], wrong_tx, report)
    wrong_report = _report(nonce="0x" + "22" * 32)
    with pytest.raises(RuntimeError, match="nonce mismatch"):
        leaf._verify_canonical_receipt(["fixture://mainnet"], _descriptor(wrong_report), wrong_report)


def test_receipt_status_depositor_amount_and_recipient_are_bound(monkeypatch: pytest.MonkeyPatch) -> None:
    report = _report()
    descriptor = _descriptor(report)
    for mutate, message in (
        (lambda r: r.update(status="0x0"), "successful"),
        (lambda r: r["logs"][0]["topics"].__setitem__(2, "0x" + "0" * 64), "depositor"),
        (lambda r: r["logs"][0].__setitem__("data", "0x" + _word(0xE0) + _word(1) + NONCE + ROUTE + r["logs"][0]["data"][2 + 64 * 4:]), "amount"),
    ):
        receipt = _receipt()
        mutate(receipt)
        monkeypatch.setattr(leaf, "_rpc_receipt", lambda _url, _tx, receipt=receipt: receipt)
        with pytest.raises(RuntimeError, match=message):
            leaf._verify_canonical_receipt(["fixture://mainnet"], descriptor, report)
    receipt = _receipt()
    data = receipt["logs"][0]["data"]
    recipient_start = 2 + 64 * 8
    receipt["logs"][0]["data"] = data[:recipient_start] + ("78" * len(RECIPIENT)) + data[recipient_start + 2 * len(RECIPIENT):]
    monkeypatch.setattr(leaf, "_rpc_receipt", lambda _url, _tx: receipt)
    with pytest.raises(RuntimeError, match="recipient"):
        leaf._verify_canonical_receipt(["fixture://mainnet"], descriptor, report)


def test_vault_and_token_pins_are_fail_closed() -> None:
    with pytest.raises(RuntimeError, match="vault_address"):
        _descriptor(_report(vault_address="0x" + "11" * 20))
    with pytest.raises(RuntimeError, match="usdc_address"):
        _descriptor(_report(usdc_address="0x" + "22" * 20))


def test_remote_descriptor_hash_mismatch_stops(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    import argparse
    from types import SimpleNamespace
    report = _report()
    out = tmp_path / "proof"
    out.mkdir()
    (out / "evm-deposit.json").write_text(json.dumps(report))
    receipt = _receipt()
    monkeypatch.setattr(leaf, "_rpc_receipt", lambda _url, _tx: receipt)
    calls = []
    monkeypatch.setattr(leaf, "run", lambda argv: calls.append(list(argv)))
    monkeypatch.setattr(leaf, "run_output", lambda argv: SimpleNamespace(stdout="00" * 32 + "  deployment.json\n"))
    args = argparse.Namespace(
        artifact_dir=str(out), witness=None, prover_host="prover.example",
        remote_workdir="/work/test", ssh_key=None,
        execution_rpc="https://execution.example", beacon_rpc="https://beacon.example",
        source_rpc_url=["fixture://mainnet"], deposit_tx=None,
    )
    with pytest.raises(RuntimeError, match="remote deployment descriptor hash"):
        leaf.prove(args)
    assert len(calls) == 1


def test_wrong_block_in_event_is_rejected(monkeypatch: pytest.MonkeyPatch) -> None:
    report = _report()
    descriptor = _descriptor(report)
    receipt = _receipt()
    receipt["logs"][0]["blockNumber"] = "0x1"
    monkeypatch.setattr(leaf, "_rpc_receipt", lambda _url, _tx: receipt)
    with pytest.raises(RuntimeError, match="block"):
        leaf._verify_canonical_receipt(["fixture://mainnet"], descriptor, report)


def test_wrong_route_binding_is_rejected(monkeypatch: pytest.MonkeyPatch) -> None:
    report = _report()
    descriptor = _descriptor(report)
    receipt = _receipt()
    data = receipt["logs"][0]["data"]
    start, end = 2 + 64 * 3, 2 + 64 * 4
    receipt["logs"][0]["data"] = data[:start] + ("00" * 32) + data[end:]
    monkeypatch.setattr(leaf, "_rpc_receipt", lambda _url, _tx: receipt)
    with pytest.raises(RuntimeError, match="route_binding"):
        leaf._verify_canonical_receipt(["fixture://mainnet"], descriptor, report)


def test_cross_rpc_disagreement_is_rejected(monkeypatch: pytest.MonkeyPatch) -> None:
    report = _report()
    descriptor = _descriptor(report)
    second = _receipt()
    second["blockNumber"] = "0x1882007"
    monkeypatch.setattr(leaf, "_rpc_receipt", lambda url, _tx: _receipt() if url == "a" else second)
    with pytest.raises(RuntimeError, match="disagreement"):
        leaf._verify_canonical_receipt(["a", "b"], descriptor, report)


def test_public_values_are_independently_bound() -> None:
    report = _report()
    descriptor = _descriptor(report)
    deposit_id, _ = leaf._decode_deposit_log(_receipt(), descriptor, report)
    values = {
        "deposit_id": deposit_id,
        "vault_address": descriptor["vault"],
        "token_address": leaf.EXPECTED_TOKEN_ADDRESS,
        "depositor": report["stakehub_wallet"],
        "pftl_recipient": descriptor["recipient"],
        "amount_atoms": descriptor["amount_atoms"],
        "nonce": descriptor["nonce"],
        "route_binding": descriptor["route_binding"],
    }
    leaf._assert_public_values(values, descriptor, deposit_id, report)
    for key, bad in (
        ("deposit_id", "0x" + "33" * 32),
        ("amount_atoms", descriptor["amount_atoms"] + 1),
        ("route_binding", "0x" + "55" * 32),
    ):
        mutated = copy.deepcopy(values)
        mutated[key] = bad
        with pytest.raises(RuntimeError, match="mismatch"):
            leaf._assert_public_values(mutated, descriptor, deposit_id, report)


def test_missing_source_rpc_is_fail_closed() -> None:
    report = _report()
    with pytest.raises(ValueError, match="source-rpc"):
        leaf._verify_canonical_receipt([], _descriptor(report), report)


def test_rpc_receipt_uses_explicit_user_agent() -> None:
    receipt = _receipt()

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):  # noqa: N802 - stdlib handler API
            agent = self.headers.get("User-Agent", "")
            if not agent or agent.startswith("Python-urllib"):
                self.send_response(403)
                self.end_headers()
                return
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"jsonrpc": "2.0", "id": 1, "result": receipt}).encode())

        def log_message(self, *_args):
            return

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        got = leaf._rpc_receipt(f"http://127.0.0.1:{server.server_port}/", "0x" + TX)
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()
    assert got["status"] == "0x1"
    assert got["blockNumber"] == "0x1882006"


@pytest.mark.skipif(
    os.environ.get("POSTFIAT_LEAF_LIVE_RPC") != "1",
    reason="set POSTFIAT_LEAF_LIVE_RPC=1 to run the public Ethereum RPC smoke test",
)
def test_live_public_rpc_receipt_smoke() -> None:
    receipt = leaf._rpc_receipt(
        "https://ethereum-rpc.publicnode.com",
        "0x" + TX,
    )
    assert receipt["status"] == "0x1"
    assert receipt["blockNumber"] == "0x1882006"
