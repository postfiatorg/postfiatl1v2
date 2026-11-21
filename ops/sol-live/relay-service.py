"""Consume rooted receipt and epoch-stake captures in Solana slot order."""

import hashlib
import os
import struct
import sys
import time
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
from threading import Event
from solders.pubkey import Pubkey
from live import (
    ASSET,
    CHAIN,
    ISSUER,
    INSTANCE,
    SHARED_STATE,
    KEY,
    NODE,
    PEERS,
    PFTL_GENESIS,
    ROOT,
    STATE,
    read,
    save,
)
from profile import CAPTURE, MANIFEST, table

sys.path.insert(0, str(ROOT / "tools/pfusdc-sol-relay"))
from jobs import Relay, job_id
from storage import locked
from source import journal
from deposit_index import deposits_at


def configure(activation, url, archive=None):
    p = activation["profile"]
    pins = p["solana"]
    programs = {k["kind"]: k for k in read(MANIFEST)["programs"]}
    base = STATE / "relay" / activation["profile_hash"]
    cfg = dict(
        state=str(base),
        verify_certified_epoch_set=True,
        ledger="node",
        prover="warm",
        handoff_prover_lock=str(SHARED_STATE / "handoff-prover.lock"),
        prover_socket=f"/run/user/{os.getuid()}/pft-sol-prover/prover.sock",
        archive=str(archive or STATE / "archive"),
        capture_root=str(base / "captures"),
        capture_binary=str(CAPTURE),
        host=str(ROOT / "target/task-l/host/release/pfusdc-sol-ingress-host"),
        node=NODE,
        key_file=KEY,
        submitter=ISSUER,
        rpc_peers=PEERS,
        chain_id=CHAIN.decode(),
        genesis_hash=PFTL_GENESIS.hex(),
        asset_id=ASSET,
        route_id=p["route_id"],
        vault_program=p["vault_address"],
        recorder_program=pins["recorder_program"],
        route_binding=pins["route_binding"],
        solana_genesis_hash=pins["genesis_hash"],
        solana_rpc=url,
        archived_transactions=str(
            (archive or STATE / "archive").parent / "finalized-transactions"
        ),
        profile_hash=activation["profile_hash"],
        current_epoch=pins["anchor"]["epoch"],
        governance_receipt=dict(
            profile=p,
            profile_hash=activation["profile_hash"],
            tx_id=activation["tx_id"],
            root=activation["validators"][0]["root"],
        ),
    )
    cfg["warm_signer"] = str(ROOT / "target/task-l/warm-signer")
    cfg["signer_state"] = str(SHARED_STATE / "signer")
    if INSTANCE == "devnet":
        from solders.hash import Hash

        cfg["private_devnet_genesis"] = bytes(
            Hash.from_string(read(STATE / "cluster.json")["genesis"])
        ).hex()
    for role, kind in [
        ("deposit", "aggregate"),
        ("keys", "key_table"),
        ("handoff", "handoff"),
    ]:
        row = programs[kind]
        elf = MANIFEST.parent / row["elf_path"]
        if hashlib.sha256(elf.read_bytes()).hexdigest() != row["elf_sha256"]:
            raise ValueError("guest ELF mismatch")
        cfg[role + "_elf"] = str(elf)
        cfg[role + "_vkey"] = row["program_vkey"]
    # A key-only revision changes the deposit key and leaves the route's handoff pin as it
    # was. Until the handoff pin is revised, the route check must compare against that pin.
    if pins["handoff_program_vkey"] != cfg["handoff_vkey"]:
        cfg["handoff_vkey_pending"] = cfg["handoff_vkey"]
        cfg["handoff_vkey"] = pins["handoff_program_vkey"]
    save(base / "config.json", cfg)
    return cfg


def handoff(cfg, directory, epoch, prior, addr):
    profile = cfg["governance_receipt"]["profile"]["solana"]
    prefix = (
        struct.pack(
            "<QQB", epoch - 1, profile["slots_per_epoch"], profile["epoch_warmup"]
        )
        + prior
        + table(prior)
    )
    state = (directory / "state").read_bytes()
    body = (
        (directory / "shreds").read_bytes()
        + state[:32]
        + bytes.fromhex(profile["recorder_program"])
        + bytes(Pubkey.from_string(addr))
        + state[96:]
    )
    dest = Path(cfg["capture_root"]) / f"handoff-{epoch}-{directory.name}"
    dest.mkdir(parents=True, exist_ok=True)
    if (dest / "source.json").exists():
        return
    (dest / "context.bin").write_bytes(
        prefix + (directory / "certificate").read_bytes()
    )
    (dest / "block.bin").write_bytes(body)
    save(
        dest / "source.json",
        dict(
            kind="handoff",
            epoch=epoch,
            slot=int(directory.name),
            recorder_program=profile["recorder_program"],
        ),
    )


def successor_matches(archive, values):
    ranks = (archive / "validators").read_bytes()
    digest = hashlib.sha256(
        b"postfiat.pfusdc.solana.validator_set.v1" + ranks
    ).hexdigest()
    return values["next_set"] == digest and values["total_stake"] == int.from_bytes(
        ranks[:16], "little"
    )


def check_client(cfg, cursor, pins):
    if cursor["epoch"] == pins["anchor"]["epoch"]:
        return True
    base = Path(cfg["state"])
    job = (
        base
        / "jobs"
        / job_id("handoff", f"{cursor['epoch']}-{cursor['ranks']}", cfg["route_id"])[2:]
    )
    values = journal((job / "public.bin").read_bytes())
    if successor_matches(Path(cfg["archive"]) / str(cursor["ranks"]), values):
        return True
    status = base / "client-mismatch.json"
    if not status.exists():
        save(
            status,
            dict(
                epoch=cursor["epoch"],
                slot=cursor["ranks"],
                tx_id=read(job / "job.json")["tx_id"],
                signed_total=values["total_stake"],
                certified_total=int.from_bytes(
                    (
                        Path(cfg["archive"]) / str(cursor["ranks"]) / "validators"
                    ).read_bytes()[:16],
                    "little",
                ),
                reason="accepted successor differs from the certified epoch stake table",
            ),
        )
    return False


def table_for_epoch(root, epoch):
    for f in sorted(root.glob("*/events.json"), key=lambda f: int(f.parent.name)):
        data = read(f)
        if data["epoch"] == epoch:
            for event in data["events"]:
                if event["tag"] == "PFSOLST1":
                    return f.parent, event
    return None


def handoff_job(relay, epoch, ranks, candidate):
    cfg = relay.cfg
    directory, event = candidate
    job = relay.create("handoff", f"{epoch}-{directory.name}")[0]
    with locked(job / "capture.lock"):
        handoff(
            cfg,
            directory,
            epoch,
            (Path(cfg["archive"]) / str(ranks) / "validators").read_bytes(),
            event["address"],
        )
    return job


def preprove(relay, cursor, stop):
    # This worker only builds proofs. The slot-ordered consumer alone advances
    # the ledger client, after delivering all known deposits in the old epoch.
    epoch, ranks = cursor["epoch"], cursor["ranks"]
    while not stop.is_set():
        candidate = table_for_epoch(Path(relay.cfg["archive"]), epoch + 1)
        if candidate is None:
            stop.wait(0.25)
            continue
        relay.run(handoff_job(relay, epoch + 1, ranks, candidate), proof_only=True)
        epoch, ranks = epoch + 1, int(candidate[0].name)


def run():
    activated = STATE / "local-activation.json"
    while not activated.exists():
        time.sleep(0.5)
    activation = read(activated)
    url = read(STATE / "rpc.json")[0]
    cfg = configure(activation, url)
    relay = Relay(cfg)
    pins = activation["profile"]["solana"]
    root = STATE / "archive"
    checkpoint = Path(cfg["state"]) / "cursor.json"
    cursor = (
        read(checkpoint)
        if checkpoint.exists()
        else dict(
            slot=pins["anchor_slot"] - 1,
            epoch=pins["anchor"]["epoch"],
            ranks=pins["anchor_slot"],
        )
    )
    stop = Event()
    with ThreadPoolExecutor(1) as pool:
        preparing = pool.submit(preprove, relay, cursor.copy(), stop)
        try:
            while True:
                if preparing.done():
                    preparing.result()
                if url != read(STATE / "rpc.json")[0]:
                    return
                if not check_client(cfg, cursor, pins):
                    time.sleep(5)
                    continue
                files = sorted(
                    root.glob("*/events.json"), key=lambda f: int(f.parent.name)
                )
                for f in files:
                    d = f.parent
                    slot = int(d.name)
                    if slot <= cursor["slot"]:
                        continue
                    record = read(f)
                    epoch = record["epoch"]
                    events = record["events"]
                    if epoch > cursor["epoch"]:
                        # Acquire a proved stake update before handling this epoch's credits.
                        next_epoch = cursor["epoch"] + 1
                        candidate = table_for_epoch(root, next_epoch)
                        if candidate is None:
                            break
                        directory, _ = candidate
                        job = handoff_job(relay, next_epoch, cursor["ranks"], candidate)
                        reply = relay.run(job)
                        if reply["status"] != "accepted":
                            raise RuntimeError("handoff pending")
                        cursor.update(epoch=next_epoch, ranks=int(directory.name))
                        save(checkpoint, cursor)
                        if epoch != next_epoch:
                            break
                    if any(e["tag"] == "PFSOLRC1" for e in events):
                        for sig in deposits_at(STATE, url, cfg["vault_program"], slot, events):
                            job = relay.create("deposit", sig)[0]
                            relay.run(job)
                    cursor["slot"] = slot
                    save(checkpoint, cursor)
                time.sleep(0.2)
        finally:
            stop.set()


if __name__ == "__main__":
    run()
