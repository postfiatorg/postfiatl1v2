"""Shared chain identities, public reads and persisted Task L state."""

import concurrent.futures
import json
import os
from pathlib import Path
import sys
import tempfile
import time
from uuid import uuid4
import socket
import requests

ROOT = Path(__file__).resolve().parents[2]
STATE = Path("/mnt/HC_Volume_106212907/stakehub-hyperliquid/sol-live")
INSTANCE = os.environ.get("PFT_SOL_INSTANCE", "local")
if INSTANCE not in ("local", "devnet"):
    raise ValueError("unknown Solana test cluster")
SHARED_STATE = STATE
if INSTANCE == "devnet":
    STATE = STATE / "devnet"
sys.path.insert(0, str(ROOT / "solana/deploy"))
from route_binding import CHAIN, PFTL_GENESIS

ASSET = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"
R6 = "cdc98a49"
ISSUER = "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8"
NODE = "/mnt/HC_Volume_106212907/stakehub-hyperliquid/sol-release6/bin/cdc98a49/postfiat-node"
KEY = "/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/pfusdc-issuer-key.json"
PEERS = [["127.0.0.1", 39650 + i] for i in range(6)]
HOSTS = [
    "64.176.220.75",
    "95.179.184.122",
    "66.42.48.39",
    "149.28.63.106",
    "95.179.179.206",
    "45.32.110.170",
]


def read(p):
    return json.loads(Path(p).read_bytes())


def save(p, data):
    p = Path(p)
    p.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=p.parent, mode="w", delete=False) as f:
        json.dump(data, f, indent=2)
        f.flush()
        os.fsync(f.fileno())
    os.replace(f.name, p)


def rpc(i, method, **params):
    rid = uuid4().hex
    with socket.create_connection(tuple(PEERS[i]), 5) as s:
        s.settimeout(120)
        s.sendall(
            json.dumps(
                dict(
                    version="postfiat-local-rpc-v1",
                    id=rid,
                    method=method,
                    params=params,
                )
            ).encode()
            + b"\n"
        )
        r = json.loads(s.makefile("rb").readline(64 << 20))
    if r.get("id") != rid or r.get("ok") is not True:
        raise RuntimeError(f"{i} {method}: {r.get('error')}")
    return r["result"]


def six(fn):
    with concurrent.futures.ThreadPoolExecutor(6) as pool:
        return list(pool.map(fn, range(6)))


def survey(fn, need=6, n=6):
    """Ask every validator; return the answers by index once `need` answered.
    Below that, raise and name the validators that did not answer."""
    with concurrent.futures.ThreadPoolExecutor(n) as pool:
        futures = {i: pool.submit(fn, i) for i in range(n)}
    rows, lost = {}, []
    for i, future in futures.items():
        try:
            rows[i] = future.result()
        except Exception:
            lost.append(f"validator-{i}")
    if len(rows) < need:
        raise RuntimeError(
            f"{len(rows)} of {n} validators answered, {need} required; "
            f"unreachable: {', '.join(lost) or 'none'}"
        )
    return rows


def agreeing(rows, key, need, message):
    """Indices of the largest group of answers with equal `key`, at least `need`."""
    groups = {}
    for i, row in rows.items():
        groups.setdefault(key(row), []).append(i)
    best = max(groups.values(), key=len)
    if len(best) < need:
        raise RuntimeError(message)
    return best


def verify_node_ids(rows):
    """Each answer must come from the validator its tunnel names. A full list
    needs all six; a survey's answers by index may omit unreachable ones."""
    if isinstance(rows, list):
        if len(rows) != len(PEERS):
            raise ValueError("Six validator statuses are required.")
        rows = dict(enumerate(rows))
    for i, status in rows.items():
        expected = f"validator-{i}"
        if status.get("node_id") != expected:
            raise ValueError(f"Expected {expected} on port {PEERS[i][1]}")


def fleet(need=6):
    rows = survey(lambda i: rpc(i, "status"), need)
    verify_node_ids(rows)
    keys = (
        "chain_id",
        "genesis_hash",
        "block_height",
        "state_root",
        "block_tip_hash",
        "build_git_revision",
    )
    best = agreeing(
        rows,
        lambda r: tuple(r[k] for k in keys),
        need,
        "fleet is not at one build, root and tip",
    )
    head = rows[best[0]]
    if (head["chain_id"], head["genesis_hash"]) != (
        CHAIN.decode(),
        PFTL_GENESIS.hex(),
    ):
        raise RuntimeError("unexpected fleet identity")
    return head


def sol(url, method, *params):
    r = requests.post(
        url, json=dict(jsonrpc="2.0", id=1, method=method, params=params), timeout=20
    )
    r.raise_for_status()
    return r.json()["result"]


def receipt_at(txid, height, accepted=True, need=6):
    def check(i):
        deadline = time.monotonic() + 90
        while True:
            try:
                result = rpc(i, "tx", tx_id=txid)
                break
            except RuntimeError as error:
                message = str(error)
                busy = "storage_writer_busy:" in message
                # "RPC worker failed" is a validator's read failing just after a commit; it
                # answers on the next ask (01:15 UTC, 21 November).
                if time.monotonic() >= deadline or (
                    not busy
                    and "rpc_tx_not_found" not in message
                    and "RPC worker failed" not in message
                ):
                    raise
                time.sleep(1 if busy else 0.3)
        row = result["receipt"]
        hdr = result["block"]["header"]
        if (
            result["confirmed"] is not True
            or row["tx_id"] != txid
            or row["accepted"] is not accepted
            or hdr["height"] != height
        ):
            raise RuntimeError(f"unexpected receipt {row}")
        if result.get("block_height", result.get("height", height)) != height:
            raise RuntimeError("transaction height mismatch")
        return {"validator": i, "root": hdr["state_root"], "receipt": row}

    rows = survey(check, need)
    best = agreeing(
        rows,
        lambda r: (r["root"], json.dumps(r["receipt"], sort_keys=True)),
        need,
        "validators disagree",
    )
    return [rows[i] for i in best]
