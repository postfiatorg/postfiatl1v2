"""Per-host settlement reviewer for the Solana exit: check only, never sign by itself.

Runs on one validator host, reads the burn and its redemption from that host's own node,
reads Solana through the endpoint configured on that host, derives every address itself, and
writes its verdict into a local review directory. The signing step on the same host
(`exit_review.py sign`) signs a decision only from this host's own fresh verdict for exactly
that decision. One review covers a paid decision (marker byte 1, one payout instruction),
a refund (marker byte 2, one cancel instruction) or this host's rotation clearance.

Exit codes: 0 approved, 2 refused with a named reason, 3 RPC error with a named reason.
"""

import argparse
import base64
import dataclasses
import datetime
import hashlib
import hmac
import json
import os
import re
import socket
import stat
import sys
import time
import urllib.request
import uuid
from pathlib import Path

import base58
from solders.pubkey import Pubkey

VERIFIER = Pubkey.from_string("8Dxp8bfgEh6ankapfBdkTds5cebqrtPdFxpZSP6JsGAF")
VAULT = Pubkey.from_string("GFkpeTibgdRAAAndgUMYsJnSTck91tUbrpuWBWan6z1G")
MINT = Pubkey.from_string("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
TOKEN = Pubkey.from_string("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
ATA_PROGRAM = Pubkey.from_string("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
SYSTEM = Pubkey.from_string("11111111111111111111111111111111")
SOLANA_GENESIS = "45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0"
PREFIX = f"solana-spl:{SOLANA_GENESIS}:"
ASSET = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"
FAMILY = ASSET
SERIES = "e90627502bea5857aefd003d80af518f0ccb44ad2be8098ec6db2025ccaf8390e8bef31e7538b398a3c94f019d7b3b0f"
BUCKET = "4c045e289eaaa2c7ae7592b96ec0eeabe9fa0cfdd032783d359409dcc08d9dd0dfdd7cc8d483d6a88c12dc713522d2cc"
MAGIC = b"PFTL-PFUSDC-TIER4"
EGRESS = b"postfiat.pfusdc.egress_public_values.v1"
REVIEWER = "exit_review.py/1"
REVIEW_MAX_AGE = 900
EXIT_REFUSED, EXIT_RPC = 2, 3
CONFIG = Path("/etc/postfiat/sol-exit-review.json")
KEY = Path("/etc/postfiat/sol-exit-review.key")
ACTIONS = {"paid": (b"\x01", 2), "refund": (b"\x02", 10)}


class Refused(ValueError):
    """The review refuses this decision; the reason names the failed check."""


class RpcError(RuntimeError):
    """A node or Solana read failed; nothing is written."""


def require(ok, reason):
    if not ok:
        raise Refused(reason)


def pda(program, *seeds):
    return Pubkey.find_program_address(list(seeds), program)[0]


LIGHT = pda(VERIFIER, b"egress-light")


def marker(burn):
    return pda(VERIFIER, b"egress-burn", hashlib.sha256(bytes.fromhex(burn)).digest())


def destination_hash(owner):
    dest = PREFIX + bytes(owner).hex()
    return hashlib.sha3_384(b"postfiat.vault_bridge.destination_ref_hash.v1\0"
                            + f"destination_ref_bytes={len(dest)}\ndestination_ref={dest}\n".encode()).digest()


def fields(buffer):
    """The proof's public values (TLV fields 1..n) as the verifier decodes them."""
    require(buffer.startswith(MAGIC), "public values: not a withdrawal value set")
    size = int.from_bytes(buffer[len(MAGIC):len(MAGIC) + 4], "big")
    body = buffer[len(MAGIC) + 4 + size:]
    require(buffer[len(MAGIC) + 4:len(MAGIC) + 4 + size] == EGRESS, "public values: not a withdrawal value set")
    out = []
    while body:
        require(len(body) >= 6 and int.from_bytes(body[:2], "big") == len(out) + 1, "public values: malformed")
        n = int.from_bytes(body[2:6], "big")
        out.append(body[6:6 + n])
        body = body[6 + n:]
    require(len(out) == 39, "public values: wrong field count")
    return out


def canonical_owner(destination_ref):
    require(destination_ref.startswith(PREFIX), "canonical destination: not this Solana network")
    text = destination_ref[len(PREFIX):]
    require(re.fullmatch(r"[0-9a-f]{64}", text) is not None, "canonical destination: not 32 lowercase hex bytes")
    owner = Pubkey.from_bytes(bytes.fromhex(text))
    require(owner.is_on_curve(), "canonical destination: owner is not a wallet key")
    return owner


def solana_rpc(endpoint):
    def call(method, params):
        body = json.dumps(dict(jsonrpc="2.0", id=1, method=method, params=params)).encode()
        try:
            request = urllib.request.Request(endpoint, body, {"content-type": "application/json"})
            reply = json.load(urllib.request.urlopen(request, timeout=30))
        except (OSError, ValueError) as error:
            raise RpcError(f"{endpoint} {method}: {error}") from error
        if "error" in reply:
            raise RpcError(f"{endpoint} {method}: {reply['error']}")
        return reply["result"]

    call.endpoint = endpoint
    return call


def node_rpc(address):
    def call(method, **params):
        rid = uuid.uuid4().hex
        try:
            with socket.create_connection(tuple(address), 5) as s:
                s.settimeout(60)
                s.sendall(json.dumps(dict(version="postfiat-local-rpc-v1", id=rid, method=method,
                                          params=params)).encode() + b"\n")
                line = s.makefile("rb").readline(64 << 20)
            reply = json.loads(line)
        except (OSError, ValueError) as error:
            raise RpcError(f"node {method}: {error}") from error
        if reply.get("id") != rid or reply.get("ok") is not True:
            raise RpcError(f"node {method}: {reply.get('error')}")
        return reply["result"]

    return call


def transaction(sol, sig):
    tx = sol("getTransaction", [sig, dict(commitment="finalized", encoding="json", maxSupportedTransactionVersion=0)])
    if tx is None:
        raise RpcError(f"{getattr(sol, 'endpoint', 'solana')} getTransaction: {sig} not found at finalized")
    return tx


def uploaded_values(sol, buffer):
    """Rebuild the values buffer the proof was checked against from its tag-5 uploads."""
    chunks = []
    for row in reversed(sol("getSignaturesForAddress", [buffer, dict(commitment="finalized", limit=50)])):
        if row["err"] is not None:
            continue
        tx = transaction(sol, row["signature"])
        keys = tx["transaction"]["message"]["accountKeys"]
        for ix in tx["transaction"]["message"]["instructions"]:
            data = base58.b58decode(ix["data"])
            if keys[ix["programIdIndex"]] == str(VERIFIER) and data[:1] == b"\x05":
                chunks.append((int.from_bytes(data[1:3], "little"), data[3:]))
    out = bytearray()
    for offset, chunk in chunks:
        out.extend(b"\0" * max(0, offset + len(chunk) - len(out)))
        out[offset:offset + len(chunk)] = chunk
    return bytes(out)


def review_burn(action, burn, node, sol):
    """Every check behind a paid or refund signature; returns the verdict body."""
    want_marker, want_tag = ACTIONS[action]
    rows = [r for r in node("vault_bridge_status", asset_id=ASSET)["redemptions"] if r["burn_tx_id"] == burn]
    require(len(rows) == 1, "redemption state: burn not found on this node")
    red = rows[0]
    require(red["state"] == "pending" and red["settled_atoms"] == 0, "redemption state: not pending and unsettled")
    covered = node("vault_bridge_route", asset_id=SERIES).get("covered_bucket_ids") or [BUCKET]
    require(red["bucket_id"] in covered, "covered bucket: the burn's bucket is not covered")
    owner = canonical_owner(red["destination_ref"])
    key = marker(burn)
    account = sol("getAccountInfo", [str(key), dict(commitment="finalized", encoding="base64")])["value"]
    require(account is not None and account["owner"] == str(VERIFIER)
            and base64.b64decode(account["data"][0]) == want_marker,
            f"marker: {key} does not read {want_marker[0]} at finalized")
    finalized = sol("getSlot", [dict(commitment="finalized")])
    history = [r for r in sol("getSignaturesForAddress", [str(key), dict(commitment="finalized", limit=100)])]
    require(len(history) >= 1, "marker: no transaction wrote it")
    sig = history[-1]["signature"]
    tx = transaction(sol, sig)
    require(tx["meta"]["err"] is None, "failed transaction: the marker's writer did not succeed")
    require(0 < tx["slot"] <= finalized, "finalized: the transaction is not below the finalized slot")
    message = tx["transaction"]["message"]
    keys = message["accountKeys"]
    tagged = [ix for ix in message["instructions"]
              if base58.b58decode(ix["data"])[:1] == bytes([want_tag]) and len(ix["accounts"]) >= 7
              and keys[ix["accounts"][6]] == str(key)]
    require(len(tagged) == 1, "exactly one: the transaction has not exactly one such instruction for this marker")
    for group in tx["meta"].get("innerInstructions") or []:
        require(not any(keys[i["programIdIndex"]] == str(VERIFIER) for i in group["instructions"]),
                "exactly one: the verifier is also called inside the transaction")
    ix = tagged[0]
    require(keys[ix["programIdIndex"]] == str(VERIFIER), "program: the instruction is not the exit verifier's")
    used = [keys[i] for i in ix["accounts"]]
    require(used[1] == str(LIGHT), "program: the instruction names another light account")
    values = fields(uploaded_values(sol, used[4]))
    require(values[23].hex() == burn and values[20] == values[23], "public values: another burn")
    require(int.from_bytes(values[26], "big") == red["amount_atoms"], "public values: another amount")
    require(values[28] == destination_hash(owner), "public values: another destination")
    require(values[25].hex() == red["bucket_id"] and values[22].hex() == FAMILY,
            "public values: another bucket or family")
    require(values[31] == b"SOLANA01", "public values: not a Solana exit")
    if action == "paid":
        ata = pda(ATA_PROGRAM, bytes(owner), bytes(TOKEN), bytes(MINT))
        custody = pda(VAULT, b"custody")
        require(len(used) == 15 and used[7] == str(VAULT) and used[12] == str(owner)
                and used[14] == str(TOKEN), "program: the payout names other accounts")
        require(used[13] == str(MINT), "mint: the payout names another mint")
        deltas = {}
        for name, address in (("custody", custody), ("recipient", ata)):
            require(str(address) in keys, f"amount: the {name} account is missing")
            index = keys.index(str(address))
            amounts = []
            for phase in ("preTokenBalances", "postTokenBalances"):
                rows = [r for r in tx["meta"][phase] if r["accountIndex"] == index]
                if not rows and name == "recipient" and phase == "preTokenBalances":
                    amounts.append(0)
                    continue
                require(len(rows) == 1 and rows[0]["mint"] == str(MINT)
                        and rows[0]["uiTokenAmount"]["decimals"] == 6, "mint: a balance in another mint")
                amounts.append(int(rows[0]["uiTokenAmount"]["amount"]))
            deltas[name] = amounts[1] - amounts[0]
        require(deltas == {"custody": -red["amount_atoms"], "recipient": red["amount_atoms"]},
                f"amount: token deltas {deltas}")
    raw = base58.b58decode(sig)
    require(len(raw) == 64 and any(raw), "finalized: invalid transaction signature")
    packet = red["withdrawal_packet_hash"]
    decision = (f"solana_exit_r8_paid:{packet}:{tx['slot']}:0:{raw.hex()}" if action == "paid"
                else f"solana_exit_refund:{packet}:{tx['slot']}:{raw.hex()}")
    return dict(action=action, burn=burn, decision=decision, finalized_slot=finalized, transaction_slot=tx["slot"])


@dataclasses.dataclass
class Config:
    validator: str
    node: object
    solana: object
    review_dir: Path
    key: bytes


def mac(key, body):
    return hmac.new(key, json.dumps(body, sort_keys=True).encode(), hashlib.sha256).hexdigest()


def write_verdict(cfg, body):
    body = dict(body, validator=cfg.validator, reviewer=REVIEWER, verdict="approve",
                decision_sha256=hashlib.sha256(body["decision"].encode()).hexdigest(),
                solana_endpoint=getattr(cfg.solana, "endpoint", "unknown"),
                time=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
                unix_time=int(time.time()))
    doc = dict(body, mac=mac(cfg.key, body))
    cfg.review_dir.mkdir(mode=0o755, parents=True, exist_ok=True)
    path = cfg.review_dir / f"{body['decision_sha256']}.json"
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(doc, indent=1, sort_keys=True))
    os.chmod(tmp, 0o644)
    os.replace(tmp, path)
    return path


def run_review(cfg, action, burn):
    try:
        if action == "clearance":
            body = review_clearance(cfg)
        else:
            body = review_burn(action, burn, cfg.node, cfg.solana)
    except RpcError as error:
        print(f"rpc error: {error}", file=sys.stderr)
        return EXIT_RPC
    except Refused as error:
        print(f"refused: {error}", file=sys.stderr)
        return EXIT_REFUSED
    print(write_verdict(cfg, body))
    return 0


def review_clearance(cfg, data_dir=None):
    """This host's rotation clearance: its own signing history ends at or below its node's tip."""
    head = cfg.node("status")
    directory = Path(data_dir or f"/var/lib/postfiat/{cfg.validator}") / "consensus-v2-safety"
    names = sorted(p.name for p in directory.glob("*.json"))
    require(names, "clearance: no resident signing history")
    for name in names:
        match = re.fullmatch(r"height-([0-9]+)\.json", name) or re.fullmatch(
            r"epoch-[0-9]+-[a-f0-9]{96}-height-([0-9]+)(?:-view-[0-9]+\.timeout-vote)?\.json", name)
        require(match and int(match[1]) <= head["block_height"], f"clearance: unknown or uncommitted round {name}")
    parent = {k: head[k] for k in ("block_height", "block_tip_hash", "state_root")}
    text = f"{parent['block_height']}:{parent['block_tip_hash']}:{parent['state_root']}"
    decision = f"solana_exit_rotation_clearance:{cfg.validator}:{text}:{head['build_git_revision']}"
    return dict(action="clearance", decision=decision, parent=parent, revision=head["build_git_revision"],
                outstanding_signed_rotation_rounds=[], finalized_slot=0)


def load_review(cfg, decision, now=None):
    """This host's own fresh approval of exactly `decision`, or Refused."""
    digest = hashlib.sha256(decision.encode()).hexdigest()
    path = cfg.review_dir / f"{digest}.json"
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    except FileNotFoundError:
        raise Refused(f"no review of this decision on {cfg.validator}") from None
    with os.fdopen(fd, "rb") as stream:
        info = os.fstat(stream.fileno())
        require(stat.S_ISREG(info.st_mode) and not info.st_mode & 0o022 and info.st_uid in (0, os.getuid())
                and info.st_size <= 65536, "unsafe review file")
        doc = json.loads(stream.read())
    body = {k: v for k, v in doc.items() if k != "mac"}
    require(hmac.compare_digest(str(doc.get("mac", "")), mac(cfg.key, body))
            and body.get("validator") == cfg.validator and body.get("reviewer") == REVIEWER,
            "review not written by this host's reviewer")
    require(body.get("decision") == decision and body.get("decision_sha256") == digest
            and body.get("verdict") == "approve", "review is for another decision")
    age = (now or time.time()) - body["unix_time"]
    require(0 <= age <= REVIEW_MAX_AGE, f"stale review ({int(age)} s old)")
    return body


def authorize(cfg, amendment, now=None):
    """Sign only a plain exit decision this host itself reviewed and approved."""
    require(amendment.get("value") == 1 and amendment.get("paused") is False,
            "not a plain Solana exit decision")
    return load_review(cfg, amendment["kind"], now)


def load_config(path=CONFIG, key=KEY):
    raw = json.loads(Path(path).read_text())
    secret = Path(key).read_bytes()
    require(len(secret) >= 32, "review key too short")
    return Config(validator=raw["validator"], node=node_rpc(raw["node"]), solana=solana_rpc(raw["solana_rpc"]),
                  review_dir=Path(raw.get("review_dir", "/var/lib/postfiat-exit-review")), key=secret)


def pending_burns(cfg):
    """Pending covered burns whose marker reads paid or cancelled: what the timer reviews."""
    status = cfg.node("vault_bridge_status", asset_id=ASSET)
    for red in status["redemptions"]:
        if red["state"] != "pending" or red["settled_atoms"] != 0:
            continue
        account = cfg.solana("getAccountInfo", [str(marker(red["burn_tx_id"])),
                                                dict(commitment="finalized", encoding="base64")])["value"]
        if account and account["owner"] == str(VERIFIER):
            byte = base64.b64decode(account["data"][0])
            for action, (want, _) in ACTIONS.items():
                if byte == want:
                    yield action, red["burn_tx_id"]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("paid", "refund"):
        p = sub.add_parser(name)
        p.add_argument("--burn", required=True)
    sub.add_parser("clearance")
    sub.add_parser("show-clearance", help="print this host's own fresh clearance for its node's current tip")
    sub.add_parser("scan", help="review every pending burn whose marker is written (the timer's job)")
    s = sub.add_parser("sign", help="run the node's signing command only for a reviewed decision")
    s.add_argument("--amendment-file", required=True, type=Path)
    s.add_argument("command_line", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    cfg = load_config()
    if args.command == "sign":
        try:
            authorize(cfg, json.loads(args.amendment_file.read_text()))
        except Refused as error:
            print(f"refused: {error}", file=sys.stderr)
            return EXIT_REFUSED
        command = [a for a in args.command_line if a != "--"]
        os.execv(command[0], command)
    if args.command == "show-clearance":
        try:
            print(json.dumps(load_review(cfg, review_clearance(cfg)["decision"])))
        except RpcError as error:
            print(f"rpc error: {error}", file=sys.stderr)
            return EXIT_RPC
        except Refused as error:
            print(f"refused: {error}", file=sys.stderr)
            return EXIT_REFUSED
        return 0
    if args.command == "scan":
        try:
            work = list(pending_burns(cfg))
        except RpcError as error:
            print(f"rpc error: {error}", file=sys.stderr)
            return EXIT_RPC
        return max([run_review(cfg, action, burn) for action, burn in work] + [run_review(cfg, "clearance", None)])
    return run_review(cfg, args.command, getattr(args, "burn", None))


if __name__ == "__main__":
    sys.exit(main())
