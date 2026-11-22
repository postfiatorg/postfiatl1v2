"""Deploy the pinned recorder and account for its loader buffer at finality."""

import argparse
import hashlib
import json
import os
import time
from types import SimpleNamespace

from live import ROOT, SHARED_STATE, read, save
import bridge
from bridge import Cluster, DIRECTORY, LOADER, deploy, load, pda, pk, require

PROGRAM = pk("2fWk8a86FXqGAWCuypnEuQnhuKBUUaw4vxv7FX8Haem6")
MAINNET = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
PAYER = "4JJEuaYtJvgtMVixTbXfppHJPh6tcU5g7g7UtkvxK3sB"
IMAGE = "aca8bae3ea1056f90a6c336722c887885cdf9ed53a4a6e610e324e9ce3d6d9c0"
PREVIOUS = "d6400718d5e329052d72c0ee686f471427db346d32ca0124b7b35ec26d39577c"
RELEASE = ROOT / "target/recorder-close/release"
FINAL = {"commitment": "finalized"}


def connect(instance):
    state = SHARED_STATE if instance == "local" else SHARED_STATE / instance
    out = state / "recorder-close"
    out.mkdir(mode=0o700, exist_ok=True)
    if instance == "mainnet":
        url = os.environ.get("SOL_MAINNET_RPC", "https://api.mainnet-beta.solana.com")
        payer = DIRECTORY / "mainnet-payer.json"
        genesis = MAINNET
    else:
        url = read(state / "rpc.json")[0]
        payer = (
            DIRECTORY if instance == "local" else DIRECTORY / instance
        ) / "local-payer.json"
        genesis = read(state / "cluster.json")["genesis"]
    c = Cluster(SimpleNamespace(rpc=url, payer=payer, log=out / "transactions.jsonl"))
    require(
        c.genesis == genesis and (instance != "mainnet" or str(c.owner) == PAYER),
        "unexpected genesis or mainnet payer",
    )
    return c, state


def finalized(c, signature):
    deadline = time.monotonic() + 180
    while time.monotonic() < deadline:
        tx = c.call(
            "getTransaction", signature, {**FINAL, "maxSupportedTransactionVersion": 0}
        )
        if tx:
            require(tx["meta"]["err"] is None, "finalized transaction failed")
            return tx
        time.sleep(1)
    raise RuntimeError(f"awaiting finalized transaction {signature}")


def upgrade(instance):
    c, state = connect(instance)
    out = state / "recorder-close"
    bridge.IMAGES = RELEASE
    image = (RELEASE / "pfusdc_recorder.so").read_bytes()
    require(hashlib.sha256(image).hexdigest() == IMAGE, "unexpected recorder build")
    key = load(DIRECTORY / "recorder.json")
    require(key.pubkey() == PROGRAM, "wrong recorder")
    data = pda(LOADER, bytes(PROGRAM))
    deployed = c.account(data)
    require(
        deployed
        and deployed["owner"] == str(LOADER)
        and deployed["data"][12:45] == b"\1" + bytes(c.owner),
        "wrong upgrade authority",
    )
    old = deployed["data"][45:]
    if old != image:
        require(hashlib.sha256(old).hexdigest() == PREVIOUS, "unrecognized predecessor")
        path = DIRECTORY / f"{c.genesis}-recorder-buffer-{IMAGE[:16]}.json"
        buffer = c.account(load(path).pubkey()) if path.exists() else None
        funded = buffer["lamports"] if buffer else 0
        needed = (
            max(0, c.rent(len(image) + 37) - funded)
            + (3 + (len(image) + 959) // 960) * 10_000
        )
        require(
            c.call("getBalance", str(c.owner), FINAL)["value"] >= needed,
            f"recorder upload requires {needed} liquid lamports",
        )
    start = time.monotonic()
    deploy(c, "recorder", key, previous_image=old)
    log = [json.loads(line) for line in c.log.read_text().splitlines()]
    signature = next(
        row["signature"]
        for row in reversed(log)
        if row.get("action") == "recorder.upgrade"
    )
    tx = finalized(c, signature)
    keys = tx["transaction"]["message"]["accountKeys"]
    buffer = load(DIRECTORY / f"{c.genesis}-recorder-buffer-{IMAGE[:16]}.json").pubkey()
    i, j = keys.index(str(buffer)), keys.index(str(data))
    pre, post = tx["meta"]["preBalances"], tx["meta"]["postBalances"]
    fee = tx["meta"]["fee"]
    refund = post[0] - pre[0] + fee
    require(
        keys[0] == str(c.owner)
        and post[i] == 0
        and pre[i] > 0
        and refund == pre[i] - (post[j] - pre[j]),
        "buffer refund mismatch",
    )
    c.level = FINAL
    require(
        not c.account(buffer) and c.account(data)["data"][45:] == image,
        "finalized loader state mismatch",
    )
    fees = {
        r["signature"]: r["fee_lamports"]
        for r in log
        if "fee_lamports" in r
        and r.get("action", "").startswith(
            ("recorder.write.", "recorder.buffer", "recorder.upgrade")
        )
    }
    result = dict(
        genesis=c.genesis,
        program=str(PROGRAM),
        image=IMAGE,
        slot=tx["slot"],
        first_epoch=c.call("getEpochSchedule")["firstNormalEpoch"],
        signature=signature,
        authority=str(c.owner),
        buffer=str(buffer),
        buffer_rent=pre[i],
        buffer_after=post[i],
        refund=refund,
        programdata_increase=post[j] - pre[j],
        upgrade_fee=fee,
        upload_fees=sum(fees.values()),
        elapsed_seconds=round(time.monotonic() - start, 3),
    )
    schedule = c.call("getEpochSchedule")
    if tx["slot"] >= schedule["firstNormalSlot"]:
        result["first_epoch"] += (tx["slot"] - schedule["firstNormalSlot"]) // schedule[
            "slotsPerEpoch"
        ]
    else:
        result["first_epoch"] = ((tx["slot"] + 32) // 32).bit_length() - 1
    save(out / "upgrade.json", result)
    print(result, flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("instance", choices=("devnet", "local", "mainnet"))
    upgrade(parser.parse_args().instance)
