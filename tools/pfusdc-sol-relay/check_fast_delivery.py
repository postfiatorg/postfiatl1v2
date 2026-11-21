"""Regression checks for ambiguous delivery, proposer redirects and warm quote binding."""

from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import Mock, patch

from jobs import Relay
from ledger import Invalid, Node, NodeReplyError
from storage import load, save


def check(directory):
    peers = [["localhost", 9200 + i] for i in range(6)]
    cfg = dict(rpc_peers=peers, chain_id="unit", genesis_hash="abcd")
    node = Node(cfg)
    signed = {"envelope": "immutable"}
    tid = "1234"
    node.tx_id = lambda envelope: tid if envelope == signed else None
    receipt = dict(accepted=True, code="accepted", tx_id=tid)
    finality = dict(
        confirmed=True, chain_id="unit", genesis_hash="abcd", receipt=receipt
    )
    seen = []

    def rpc(peer, method, params, **opts):
        if method == "status":
            return {"block_height": 9}
        assert method == "mempool_submit_signed_asset_transaction_finality"
        seen.append((peer, params["signed_asset_transaction_json"]))
        if len(seen) == 1:
            raise NodeReplyError("busy", dict(code="rpc_finality_submit_busy"))
        if len(seen) == 2:
            raise NodeReplyError(
                "test",
                dict(
                    code="rpc_finality_wrong_proposer",
                    message="retry the signed request at `validator-1`",
                ),
            )
        return {"tx_id": tid, "finality": finality}

    node.rpc = rpc
    save(directory / "signed.json", signed)
    relay = Relay.__new__(Relay)
    relay.ledger = node
    node.receipt = Mock(side_effect=AssertionError("redundant receipt RPC"))
    with patch("ledger.time.sleep") as delay:
        relay.deliver(directory, fresh=True)
    delay.assert_called_once_with(1)
    assert [p for p, _ in seen] == [peers[4], peers[4], peers[1]]
    assert len({envelope for _, envelope in seen}) == 1
    assert relay.confirm(directory) == receipt
    relay.deliver(directory)
    assert len(seen) == 3
    # A second route may sign after the first route has advanced the height.
    # Use the confirmed reply even when validator 0's deferred apply is pending.
    with TemporaryDirectory() as other:
        node.cfg["signer_state"] = other
        save(Path(other) / "confirmed.json", {"height": 10})
        node.rpc = Mock(return_value={"tx_id": tid, "finality": finality})
        node.submit(signed, Path(other))
        assert node.rpc.call_count == 1 and node.rpc.call_args.args[:2] == (
            peers[5],
            "mempool_submit_signed_asset_transaction_finality",
        )
        node.cfg.pop("signer_state")
        node.rpc = rpc
    report = load(directory / "finality.json")
    report["finality"]["genesis_hash"] = "unexpected"
    save(directory / "finality.json", report)
    try:
        relay.confirm(directory)
    except Invalid:
        pass
    else:
        raise AssertionError("accepted receipt from a different genesis")
    (directory / "finality.json").unlink()
    node.receipt = Mock(return_value=receipt)
    relay.deliver(directory)
    node.receipt.assert_called_once_with(tid)
    assert len(seen) == 3
    template = dict(proof=[255] * 356, public_values=[90], proof_kind="deposit")
    try:
        node.sign(
            dict(template, public_values=[89]),
            directory,
            (None, {"operation": template}, None, None),
        )
    except Invalid:
        pass
    else:
        raise AssertionError("signed a journal that differs from the prequote")
    # A stale quote can collide across routes. Only re-sign when every validator
    # has consumed the sequence and the old envelope has no receipt.
    node.cfg.update(submitter="issuer", profile_hash="profile", warm_signer="helper")
    u = dict(
        source="issuer",
        chain_id="unit",
        genesis_hash="abcd",
        sequence=8,
        submitter="issuer",
        profile_hash="profile",
        operation="solana_bridge_proof_v1",
        proof_kind="deposit",
        proof=[1],
        public_values=[2],
    )
    stale = {"unsigned": u}
    node.tx_id = lambda _: "deadbeef"
    exhausted = NodeReplyError(
        "id",
        {
            "code": "rpc_finality_submit_failed",
            "message": "mempool admission rejected `bad_sequence`",
        },
    )
    updated = {"unsigned": dict(u, sequence=9)}
    node.sign = Mock(return_value=updated)
    for existing in (True, False):

        def read_peer(peer, method, params):
            if method == "account":
                return {"address": "issuer", "sequence": 8}
            assert method == "receipts"
            return [{"tx_id": "deadbeef"}] if existing and peer == peers[-1] else []

        node.rpc = read_peer
        if existing:
            try:
                node.replace_spent_sequence(stale, directory, exhausted)
            except RuntimeError as error:
                assert "already has" in str(error)
            else:
                raise AssertionError("re-signed an already receipted transaction")
            node.sign.assert_not_called()
        else:
            assert node.replace_spent_sequence(stale, directory, exhausted) == updated
            record = load(directory / "superseded-deadbeef.json")
            assert len(record["observations"]) == 6 and record["signed"] == stale
    # On the measured fleet the last proposer already has the committed balance.
    account = dict(
        account="alice",
        chain_id="unit",
        genesis_hash="abcd",
        lines=[dict(asset_id="series", balance="19")],
    )
    header = {"height": 10, "state_root": "root"}
    node.rpc = Mock(return_value=account)
    assert node.fast_balance("alice", "series", (3, header))["validator"] == 3
    assert node.rpc.call_count == 1 and node.rpc.call_args.args[:2] == (
        peers[3],
        "account_lines",
    )

    def balance_peer(peer, method, params):
        if peer == peers[3]:
            raise OSError("disconnected")
        if method == "status":
            return {
                "block_height": 10,
                "state_root": "root" if peer == peers[2] else "stale",
            }
        assert method == "account_lines" and peer == peers[2]
        return account

    node.rpc = balance_peer
    assert node.fast_balance("alice", "series", (3, header))["validator"] == 2
    print(
        "PASS busy retry and proposer redirect 4 -> 1 reused one envelope; correlated receipt replay, lost acknowledgement, chain, prequote and spent-sequence checks"
    )


if __name__ == "__main__":
    with TemporaryDirectory() as d:
        check(Path(d))
