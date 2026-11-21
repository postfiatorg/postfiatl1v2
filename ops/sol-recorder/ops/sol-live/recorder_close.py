"""Archive and refund expired epoch tables after their six-validator handoff."""

import argparse
import json
import sys
import time
from pathlib import Path

from solders.hash import Hash
from solders.pubkey import Pubkey
from live import CHAIN, PFTL_GENESIS, ROOT, SHARED_STATE, read, receipt_at, rpc
from bridge import DIRECTORY, Instruction, load, pda, require, rw
from profile import profile_hash
from recorder_release import FINAL, IMAGE, MAINNET, PAYER, PROGRAM, connect, finalized
from recorder import record_epoch

sys.path.insert(0, str(ROOT / "tools/pfusdc-sol-relay"))
from ledger import node_tx_id
from source import journal
from storage import digest, locked, mkdir, save

LEGACY = [
    "9pzu1VcaZSWGbn9aBx4iq39V3vnjsXFC9w4QBQoPdgeC",
    "G3YEW7FjXUoEkc1MVpXuotkDnXH69BQLcZkiAuM1sZZB",
]


def accepted(c, state, activation, epoch):
    profile = activation["profile"]
    policy = profile_hash(profile)
    pins = profile["solana"]
    require(
        policy == activation["profile_hash"]
        and pins["recorder_program"] == bytes(PROGRAM).hex()
        and pins["genesis_hash"] == bytes(Hash.from_string(c.genesis)).hex(),
        "wrong governed source",
    )
    # Mainnet relay jobs currently share SHARED_STATE/relay; local routes use their own state.
    base = (SHARED_STATE if c.genesis == MAINNET else state) / "relay" / policy
    candidates = []
    for path in base.glob("jobs/*/job.json"):
        job = read(path)
        request = job["request"]
        target_epoch = request["reference"].partition("-")[0]
        if request["kind"] != "handoff" or target_epoch != str(epoch):
            continue
        if job["status"] != "accepted":
            # An alternative capture may still be waiting for its proof or receipt.
            return None
        candidates.append((path, job))
    for path, job in candidates:
        public = (path.parent / "public.bin").read_bytes()
        values = journal(public)
        if values["epoch"] != epoch:
            continue
        signed = read(path.parent / "signed.json")
        proof = (path.parent / "proof.bin").read_bytes()
        op = signed["unsigned"]
        txid = node_tx_id(signed)
        require(
            values["kind"] == "handoff"
            and values["recorder_program"] == pins["recorder_program"]
            and values["slot"] == job["slot"]
            and job["route_id"] == profile["route_id"]
            and job["program_vkey"] == pins["handoff_program_vkey"]
            and txid == job["tx_id"]
            and op["profile_hash"] == policy
            and op["chain_id"] == CHAIN.decode()
            and op["genesis_hash"] == PFTL_GENESIS.hex()
            and op["operation"] == "solana_bridge_proof_v1"
            and op["proof_kind"] == "sp1-solana-alpenglow-handoff-v1"
            and proof
            and bytes(op["proof"]) == proof
            and bytes(op["public_values"]) == public,
            "accepted job does not bind the signed handoff",
        )
        height = rpc(0, "tx", tx_id=txid)["block"]["header"]["height"]
        peers = receipt_at(txid, height)
        require(
            len(peers) == 6
            and all(r["receipt"]["code"].lower() == "accepted" for r in peers),
            "six matching accepted receipts required",
        )
        return (
            dict(
                tx_id=txid,
                height=height,
                slot=values["slot"],
                profile=policy,
                route=profile["route_id"],
                validators=peers,
            ),
            signed,
            public,
            proof,
        )
    return None


def verify_rows(c, key, expected_epoch, tag, header, stride, plan):
    account = c.account(key)
    require(
        account and account["owner"] == str(PROGRAM) and not account["executable"],
        "missing recorder table",
    )
    raw = account["data"]
    count = int.from_bytes(raw[header - 4 : header], "little")
    tags = (tag,) if isinstance(tag, bytes) else tag
    require(
        raw[:8] in tags
        and int.from_bytes(raw[8:16], "little") == expected_epoch
        and 0 < count <= len(plan)
        and len(raw) == header + stride * count,
        "incomplete table",
    )
    for index, (vote, bls) in enumerate(plan[:count]):
        row = raw[header + index * stride : header + (index + 1) * stride]
        require(
            row[-48:] == bytes.fromhex(bls)
            and (stride == 48 or row[:32] == bytes(Pubkey.from_string(vote))),
            "on-chain rows differ from epoch plan",
        )
    return raw, account["lamports"]


def retire(c, state, activation, epoch):
    out = state / "recorder-close" / str(epoch)
    if (out / "refund.json").exists():
        return
    prefix = DIRECTORY / f"{c.genesis}-{PROGRAM}-{epoch}"
    marker = out / "archive.json"
    legacy = c.genesis == MAINNET and epoch == 1090
    records = {
        "keys": ("keys.bin", epoch, (b"PFSOLKY1",), 20, 48),
        "next-stakes": ("stakes.bin", epoch + 1, (b"PFSOLNX1", b"PFSOLST1"), 28, 120),
    }
    kinds, keys, metadata = [], [], []
    archived_kinds = read(marker)["kinds"] if marker.exists() else None
    for index, kind in enumerate(records):
        path = Path(f"{prefix}-{kind}.json")
        if (
            archived_kinds is not None
            and kind not in archived_kinds
            or not path.is_file()
        ):
            continue
        key = load(path)
        addr = key.pubkey()
        if legacy:
            require(
                str(addr) == LEGACY[index] and str(c.owner) == PAYER,
                "unexpected legacy table",
            )
        else:
            meta = pda(PROGRAM, b"funder", bytes(addr))
            if archived_kinds is None and c.account(meta) is None:
                # A pre-upgrade local table has no verifiable funding record.
                continue
            metadata.append(meta)
        kinds.append(kind)
        keys.append(key)
    if not keys:
        return
    addresses = [k.pubkey() for k in keys]
    handoff = accepted(c, state, activation, epoch + 1)
    if handoff is None:
        return
    receipt, signed, public, proof = handoff
    mkdir(out)
    if not marker.exists():
        # The guest journal binds the source program, route profile and successor
        # epoch. Mainnet observer exports do not always include a local events index.
        plan = read(Path(f"{prefix}.votes.json"))
        files = {
            "proof.bin": proof,
            "public.bin": public,
            "signed.json": json.dumps(signed).encode(),
            "handoff.json": json.dumps(receipt).encode(),
        }
        rents = {}
        counts = {}
        for kind, addr in zip(kinds, addresses):
            name, e, tags, header, width = records[kind]
            files[name], rents[str(addr)] = verify_rows(
                c, addr, e, tags, header, width, plan
            )
            counts[kind] = (len(files[name]) - header) // width
        for addr in metadata:
            meta = c.account(addr)
            require(
                meta["owner"] == str(PROGRAM) and meta["data"] == bytes(c.owner),
                "wrong original funder",
            )
            rents[str(addr)] = meta["lamports"]
        for name, data in files.items():
            save(out / name, data)
        save(
            marker,
            dict(
                genesis=c.genesis,
                epoch=epoch,
                program=str(PROGRAM),
                payer=str(c.owner),
                tables=list(map(str, addresses)),
                rents=rents,
                kinds=kinds,
                rows=counts,
                handoff=receipt["tx_id"],
                hashes={name: digest(data) for name, data in files.items()},
            ),
        )
    archived = read(marker)
    require(
        archived["handoff"] == receipt["tx_id"]
        and archived["tables"] == list(map(str, addresses))
        and archived["genesis"] == c.genesis
        and archived["payer"] == str(c.owner)
        and all(
            digest((out / name).read_bytes()) == hashed
            for name, hashed in archived["hashes"].items()
        ),
        "archive readback differs",
    )
    tag = f"close.{epoch}"
    if c.account(addresses[0]):
        instructions = []
        for i, addr in enumerate(addresses):
            current = c.account(addr)
            filename = records[kinds[i]][0]
            require(
                current and digest(current["data"]) == archived["hashes"][filename],
                "table bytes changed",
            )
            accounts = [rw(addr, True), rw(c.owner)] + (
                [rw(metadata[i])] if metadata else []
            )
            instructions.append(Instruction(PROGRAM, bytes([3]), accounts))
        signature = c.send(tag, instructions, *keys)
    else:
        records = (json.loads(line) for line in c.log.read_text().splitlines())
        signatures = [r["signature"] for r in records if r.get("action") == tag]
        require(signatures, "table missing without signed close")
        signature = signatures[-1]
    tx = finalized(c, signature)
    indices = tx["transaction"]["message"]["accountKeys"]
    meta = tx["meta"]
    pre, post = meta["preBalances"], meta["postBalances"]
    refunded = {str(a): pre[indices.index(str(a))] for a in addresses + metadata}
    require(
        indices[0] == str(c.owner)
        and all(post[indices.index(a)] == 0 for a in refunded)
        and post[0] - pre[0] + meta["fee"] == sum(refunded.values())
        and all(not c.account(a) for a in addresses + metadata),
        "finalized refund mismatch",
    )
    result = dict(
        signature=signature,
        slot=tx["slot"],
        payer=str(c.owner),
        payer_before=pre[0],
        payer_after=post[0],
        refunded=refunded,
        fee=meta["fee"],
        handoff=receipt["tx_id"],
    )
    save(out / "refund.json", result)
    print(json.dumps(result), flush=True)


def cycle(instance):
    c, state = connect(instance)
    upgrade = read(state / "recorder-close/upgrade.json")
    require(
        upgrade["genesis"] == c.genesis and upgrade["image"] == IMAGE,
        "wrong upgrade manifest",
    )
    c.level = FINAL
    epoch = c.call("getEpochInfo", FINAL)["epoch"]
    activation = state / (
        "activation.json" if instance == "mainnet" else "local-activation.json"
    )
    if activation.exists():
        start = 1090 if instance == "mainnet" else upgrade["first_epoch"]
        governed = read(activation)
        for recorded in range(start, epoch - 1):
            try:
                retire(c, state, governed, recorded)
            except (OSError, ValueError, KeyError, RuntimeError) as error:
                # Keep the current epoch writer running during an older proof or RPC outage.
                print(f"retained epoch {recorded}: {error}", flush=True)
    if instance == "mainnet":
        c.level = {"commitment": "confirmed"}
        # Retire eligible rent before reserving the next overlapping pair. The
        # writer independently reconciles each table and publishes the prior one.
        record_epoch(c, PROGRAM, c.call("getEpochInfo", c.level)["epoch"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("instance", choices=("devnet", "local", "mainnet"))
    parser.add_argument("--once", action="store_true")
    args = parser.parse_args()
    state = SHARED_STATE if args.instance == "local" else SHARED_STATE / args.instance
    mkdir(state / "recorder-close")
    with locked(state / "recorder-close/worker.lock"):
        while True:
            cycle(args.instance)
            if args.once:
                break
            time.sleep(10 if args.instance != "mainnet" else 60)
