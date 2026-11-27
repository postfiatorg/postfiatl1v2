"""Command-line client for the owner-local PFSH02 service and a trusted finalized node."""
import argparse
import base64
import json
import os
from pathlib import Path
import socket
import sys
import tempfile
import time

from .client import (Endpoint, PostFiatRpcClient, PQ_POOL_ID, PqCursorExpired,
                     RpcClientError, pq_public_pages, private_action_hash, private_action_intent_id)


PRIVATE_SPEND_SCOPE = "pq-private-spend"


def issued_fee(snap, asset):
    terms = snap.get("pq_issued_fee")
    if (not isinstance(terms, list) or len(terms) != 2 or terms[0] != asset
            or not isinstance(terms[1], str) or not terms[1].isascii() or not terms[1].isdigit()
            or not 0 < int(terms[1]) < 2**64 or str(int(terms[1])) != terms[1]):
        raise ValueError("No active fee schedule for this issued asset.")
    return int(terms[1])


class WalletBusy(ValueError):
    """The daemon refused the operation before starting a worker."""


def local(home, op, **fields):
    # The preceding worker can close its reply just before the supervisor reaps it.
    deadline = time.monotonic() + 1
    while True:
        try:
            return _socket_command(home, op, fields)
        except WalletBusy:
            if time.monotonic() >= deadline:
                raise
            time.sleep(0.02)


def _socket_command(home, op, fields):
    with socket.socket(socket.AF_UNIX) as s:
        s.connect(str(home / "wallet.sock"))
        s.sendall(json.dumps(dict(op=op, **fields)).encode())
        s.shutdown(socket.SHUT_WR)
        with s.makefile("rb") as f:
            raw = f.read(4 * 1024 * 1024 + 1)
        if not raw:
            raise ValueError("wallet job interrupted; inspect status before retrying")
        reply = json.loads(raw)
        if reply.get("error") == "wallet busy":
            raise WalletBusy("wallet busy")
        if "error" in reply:
            raise ValueError(reply["error"])
        return reply["result"]


def with_point(home, operation, point, **fields):
    # Asset frontiers are paginated at RPC and can exceed one local control frame.
    with tempfile.NamedTemporaryFile(mode="w", dir=home, prefix="point-") as checkpoint:
        json.dump(point, checkpoint, separators=(",", ":"))
        checkpoint.flush()
        return local(home, operation, point_file=checkpoint.name, **fields)


def snapshot(client, validator=None):
    validator = validator or client
    snap = validator.private_pool_snapshot(PQ_POOL_ID)
    token = snap["snapshot_token"]
    point = dict(height=snap["finalized_height"], block_hash=snap["finalized_block_hash"],
                 state_root=snap["finalized_state_root"])
    if (token != f"pps1:{PQ_POOL_ID}:{point['height']}:{point['block_hash']}:{point['state_root']}"
            or snap["pool_id"] != PQ_POOL_ID or snap["counts"]["outputs"] % 2):
        raise ValueError("snapshot identity mismatch")
    if point["height"] > 0:
        blocks = validator.blocks(from_height=point["height"], limit=1)
        if len(blocks) != 1:
            raise ValueError("missing finalized block")
        header = blocks[0]["header"]
        cert = header["certificate"]
        votes = {v["validator"] for v in cert["votes"] if v["accept"]}
        if (header["height"] != point["height"] or header["state_root"] != point["state_root"]
                or header["block_hash"] != point["block_hash"] or
                not 0 < cert["quorum"] <= len(votes.intersection(cert["validators"]))):
            raise ValueError("checkpoint disagrees with the validator's certified block")
    point.update(network={k: snap[k] for k in ("chain_id", "genesis_hash", "protocol_version")},
                 pairs=snap["counts"]["outputs"] // 2, last_page=snap["pq_last_page"], assets={})
    for page in pq_public_pages(validator, token, "pq_assets", snap["counts"]["assets"]):
        for asset, state in page:
            if asset in point["assets"]:
                raise ValueError("duplicate finalized asset")
            point["assets"][asset] = state
    return token, point


def sync(home, client, validator=None):
    for attempt in range(3):
        try:
            checkpoint = local(home, "checkpoint")
            token, point = snapshot(client, validator)
            if point["network"] != checkpoint["network"]:
                raise ValueError("wallet network mismatch")
            with tempfile.NamedTemporaryFile(mode="w", dir=home, prefix="pages-") as pages:
                for rows in pq_public_pages(client, token, "pq_pairs", point["pairs"], checkpoint["pairs"]):
                    pages.write(json.dumps(rows, separators=(",", ":")) + "\n")
                pages.flush()
                result = with_point(home, "sync", point, rows=pages.name)
            # The tip may advance while trial decryption is running.
            try:
                tip = (validator or client).private_pool_snapshot(PQ_POOL_ID)
                lag = tip["finalized_height"] - result["height"]
                if any(tip[k] != v for k, v in point["network"].items()) or lag < 0:
                    raise ValueError("finalized tip changed identity")
                result["sync_lag_blocks"] = lag
            except (OSError, RpcClientError):
                result["sync_lag_blocks"] = None
            return result
        except PqCursorExpired:
            if attempt == 2:
                raise


def submit(home, client, scope, expected_hash=None):
    net = local(home, "checkpoint")["network"]
    action = local(home, "prepared", action_hash=expected_hash)
    # ShieldedAction serializes its discriminator first.
    wire = json.dumps({"kind": action["kind"], "body": action["body"]}, separators=(",", ":"))
    intent = private_action_intent_id(**net, action_json=wire)
    action_hash = private_action_hash(wire)
    if expected_hash is not None and action_hash != expected_hash:
        raise ValueError("pending payment changed")
    if base64.b64decode(action["body"], validate=True)[71] == 0:
        scope = PRIVATE_SPEND_SCOPE
    local(home, "submitted", action_hash=action_hash)
    try:
        report = client.private_action_submit(**net, pool_id=PQ_POOL_ID, session_scope=scope,
                                              intent_id=intent, action_json=wire, action_hash=action_hash)
    except (OSError, RpcClientError) as error:
        raise ValueError(f"Payment remains saved. Poll {intent} or retry action {action_hash}. {error}") from error
    record = report["record"]
    if record["intent_id"] != intent or record["action_hash"] != private_action_hash(wire):
        raise ValueError("receipt does not match submitted payment")
    return dict(intent_id=intent, state=record["state"])


def parser():
    p = argparse.ArgumentParser(prog="postfiat_rpc pq-wallet", description=__doc__)
    p.add_argument("--wallet", required=True, type=Path, help="owner-only wallet directory")
    p.add_argument("--endpoint", help="trusted validator host:port")
    p.add_argument("--validator-endpoint", help="trusted validator for checkpoint and certificate reads when using a separate page server")
    p.add_argument("--scope", default="pq-wallet", help="public admission session scope")
    cmds = p.add_subparsers(dest="command", required=True)
    d = cmds.add_parser("daemon", help="run the local scanner and prover in the foreground")
    d.add_argument("--key-file", required=True, type=Path, help="external 32-byte wrapping key")
    d.add_argument("--binary", default="pftl-pq-walletd")
    b = cmds.add_parser("browser", help="serve the owner-local wallet-web private payment interface")
    b.add_argument("--dist", type=Path, required=True, help="built wallet-web/dist directory")
    b.add_argument("--port", type=int, default=8790)
    b.add_argument("--pair-file", type=Path, required=True, help="new owner-only file for the local session link")
    b.add_argument("--gateway", required=True, help="wallet-proxy HTTP(S) origin")
    b.add_argument("--gateway-token-file", type=Path, required=True)
    b.add_argument("--payer", help="public deposit account, only needed for shielding")
    b.add_argument("--account-key", type=Path, help="local deposit signing key")
    b.add_argument("--issue", required=True, help="exact governed pfUSDC source-series ID")
    b.add_argument("--orchard-exit-batch", type=Path, help="locally prepared, single private-egress public batch")
    k = cmds.add_parser("keygen", help="create a new owner-only wrapping key outside the wallet")
    k.add_argument("--key-file", required=True, type=Path)
    i = cmds.add_parser("init", help="create a wallet at the current finalized birthday")
    for name, default in (("account", 0), ("first", 0), ("count", 1)):
        i.add_argument(f"--{name}", default=default, type=int)
    for name in ("balance", "status", "sync", "cancel"):
        cmds.add_parser(name)
    cmds.add_parser("submit-pending").add_argument("--action-hash", help="retry these exact saved bytes (default latest)")
    cmds.add_parser("receipt").add_argument("intent_id")
    cmds.add_parser("address").add_argument("--index", type=int)
    for name in ("backup", "restore"):
        cmds.add_parser(name).add_argument("--file", required=True, type=Path)
    for name in ("shield", "pay", "unshield"):
        c = cmds.add_parser(name)
        c.add_argument("--asset", required=True, help="PFT or the exact pfUSDC issue ID")
        c.add_argument("--amount", type=int, required=True, help="integer asset atoms")
        c.add_argument("--fee", type=int, required=name == "shield", help="PFT fee in atoms. Issued spends use the governed fee.")
        if name != "shield":
            c.add_argument("--to", required=True)
            c.add_argument("--also-replace", metavar="ACTION_HASH", help="also use an input from a second unresolved payment")
            c.add_argument("--different-payment", action="store_true", help="confirm this is a different payment from the imported unresolved spend")
            c.add_argument("--replace", metavar="ACTION_HASH", help="use an input from this submitted payment")
        else:
            c.add_argument("--payer", required=True, help="public deposit account")
            c.add_argument("--account-key", type=Path, required=True)
            c.add_argument("--sequence", type=int, required=True)
    return p


def run(args):
    home = args.wallet.absolute()
    cmd = args.command
    if cmd == "browser":
        from .pq_browser import serve
        return serve(args)
    if cmd == "keygen":
        path = args.key_file.absolute()
        if path.resolve().is_relative_to(home.resolve()):
            raise ValueError("keep the wrapping key outside the wallet directory")
        with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as f:
            f.write(os.urandom(32))
            f.flush()
            os.fsync(f.fileno())
        return {"created": True}
    if cmd == "daemon":
        home.mkdir(mode=0o700, exist_ok=True)
        os.execvp(args.binary, [args.binary, str(home), str(args.key_file.absolute())])
    if cmd in ("address", "balance", "cancel", "backup", "restore"):
        fields = {}
        if cmd == "address":
            fields["index"] = args.index
        elif cmd in ("backup", "restore"):
            fields["backup"] = str(args.file.absolute())
        return local(home, cmd, **fields)
    if cmd == "status":
        progress = local(home, "progress")
        if progress["active"] is None:
            progress.update(local(home, "balance"))
        return progress
    if not args.endpoint:
        raise ValueError("a trusted validator --endpoint is required")
    client = PostFiatRpcClient(Endpoint.parse(args.endpoint), timeout_seconds=90,
                               response_byte_cap=4*1024*1024)
    validator = (PostFiatRpcClient(Endpoint.parse(args.validator_endpoint), timeout_seconds=90,
                                  response_byte_cap=4*1024*1024) if args.validator_endpoint else client)
    if cmd == "init":
        _, point = snapshot(client, validator)
        return with_point(home, "init", point, account=args.account, first=args.first, count=args.count)
    if cmd == "sync":
        return sync(home, client, validator)
    if cmd == "receipt":
        r = client.private_action_receipt(args.intent_id)
        if r["intent_id"] != args.intent_id:
            raise ValueError("receipt identity mismatch")
        outcome = (r.get("record") or {}).get("outcome")
        if outcome:
            inclusion = outcome.get("inclusion") or {}
            outcome = {**{k: outcome[k] for k in ("accepted", "code", "fee_charged") if k in outcome},
                       **{k: inclusion[k] for k in ("height", "block_hash", "state_root", "certificate_id") if k in inclusion}}
        return {"intent_id": r["intent_id"], "state": r["state"], "outcome": outcome}
    if cmd == "submit-pending":
        return submit(home, client, args.scope, args.action_hash)
    issued_spend = cmd != "shield" and args.asset not in ("PFT", "00"*48)
    if issued_spend and args.fee is not None:
        raise ValueError("Issued payments use the governed fee. Omit --fee.")
    if not issued_spend and args.fee is None:
        raise ValueError("Supply --fee in PFT atoms.")
    sync(home, client, validator)
    fee = issued_fee(validator.private_pool_snapshot(PQ_POOL_ID), args.asset) if issued_spend else args.fee
    mode = {"shield": "Shield", "pay": "Internal", "unshield": "Exit"}[cmd]
    order = dict(mode=mode, asset="00"*48 if args.asset == "PFT" else args.asset,
                 amount=args.amount, fee=fee,
                 also_replace=getattr(args, "also_replace", None), different_payment=getattr(args, "different_payment", False),
                 recipient=getattr(args, "to", None), replace=getattr(args, "replace", None))
    if mode == "Shield":
        order.update(payer=args.payer, sequence=args.sequence, account_key=str(args.account_key.absolute()))
    unit = "PFT" if mode == "Shield" else args.asset
    print(f"{cmd} {args.amount} {args.asset} atoms, fee {fee} {unit} atoms", file=sys.stderr, flush=True)
    metrics = local(home, "prove", order=order)
    return {**metrics, **submit(home, client, args.scope, metrics["action_hash"])}


def main(argv=None):
    try:
        result = run(parser().parse_args(argv))
    except (OSError, ValueError, KeyError, RuntimeError, RpcClientError) as e:
        print(f"pq-wallet: {e}", file=sys.stderr)
        return 1
    print(json.dumps(result, separators=(",", ":")))
    return 0
