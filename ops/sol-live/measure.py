#!/usr/bin/env python3
"""Send a local vault deposit, verify its live credit, and append a measured row."""

import argparse
import base64
from concurrent.futures import ThreadPoolExecutor, as_completed
import fcntl
import hashlib
import json
import os
import secrets
import struct
import sys
import time
from solders.hash import Hash
from solders.transaction import Transaction
from live import (
    ASSET,
    CHAIN,
    INSTANCE,
    ROOT,
    SHARED_STATE,
    STATE,
    read,
    receipt_at,
    rpc,
    save,
    sol,
)
from bridge import DIRECTORY, Instruction, SYSTEM, TOKEN, load, pda, pk, ro, rw

sys.path.insert(0, str(ROOT / "tools/pfusdc-sol-relay"))
from jobs import job_id

RECIPIENTS = (
    "pfab9b9228942e5c529633a13aa271d5297bec6353",
    "pf5c266a1bdb18320e140961411a8c4d8383eaafb2",
    "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b",
)


def h(domain, body):
    return hashlib.sha3_384(domain.encode() + b"\0" + body.encode()).hexdigest()


def series_id(activation):
    p = activation["profile"]
    domain = (
        f"spl:{p['solana']['genesis_hash']}:{p['vault_address']}:{p['token_address']}"
    )
    bucket = h(
        "postfiat.vault_bridge_bucket_id.v1",
        f"asset_id={ASSET}\nsource_domain_bytes={len(domain)}\nsource_domain={domain}\npolicy_hash={activation['profile_hash']}\n",
    )
    return h(
        "postfiat.pfusdc.solana.source_series.v1",
        f"{CHAIN.decode()}\n{ASSET}\n{bucket}",
    )


def balance(address, asset, peer=0):
    rows = rpc(peer, "account_lines", account=address, asset_id=asset, limit=500)[
        "lines"
    ]
    return sum(int(line["balance"]) for line in rows if line["asset_id"] == asset)


def observe_balance(address, asset, expected):
    def read_peer(peer):
        amount = balance(address, asset, peer)
        return dict(validator=peer, balance=amount, time=time.time())

    pool = ThreadPoolExecutor(6)
    try:
        for future in as_completed([pool.submit(read_peer, i) for i in range(6)]):
            try:
                observed = future.result()
            except (OSError, RuntimeError):
                continue
            if observed["balance"] == expected:
                return observed
    finally:
        pool.shutdown(wait=False, cancel_futures=True)
    return None


def deposit_ix(owner, vault, source, mint, atoms, recipient, nonce):
    if (
        not 0 < atoms <= 25_000_000
        or len(nonce) != 32
        or len(recipient) != 42
        or not recipient.startswith("pf")
    ):
        raise ValueError("invalid amount, nonce or destination")
    return Instruction(
        vault,
        b"\1" + struct.pack("<Q", atoms) + bytes.fromhex(recipient[2:]) + nonce,
        [
            rw(owner, True),
            rw(source),
            rw(pda(vault, b"config")),
            rw(pda(vault, b"custody")),
            ro(mint),
            rw(pda(vault, b"receipt", nonce)),
            ro(TOKEN),
            ro(SYSTEM),
        ],
    )


def send(entries, out):
    activated = read(STATE / "local-activation.json")
    mismatch = STATE / "relay" / activated["profile_hash"] / "client-mismatch.json"
    if mismatch.exists():
        raise RuntimeError("governed epoch client differs from certified stake set")
    p = activated["profile"]
    spec = read(STATE / "cluster.json")
    url = read(STATE / "rpc.json")[0]
    if (
        sol(url, "getGenesisHash") != spec["genesis"]
        or bytes(Hash.from_string(spec["genesis"])).hex() != p["solana"]["genesis_hash"]
    ):
        raise ValueError("local genesis mismatch")
    key_dir = DIRECTORY if INSTANCE == "local" else DIRECTORY / INSTANCE
    payer = load(key_dir / "local-payer.json")
    vault = load(key_dir / "vault.json").pubkey()
    asset = series_id(activated)
    confirmed = SHARED_STATE / "signer" / "confirmed.json"
    peer = read(confirmed)["validator"] if confirmed.is_file() else 0
    before = {who: balance(who, asset, peer) for _, who in entries}
    bh = Hash.from_string(
        sol(url, "getLatestBlockhash", {"commitment": "confirmed"})["value"][
            "blockhash"
        ]
    )
    prepared = []
    for atoms, who in entries:
        nonce = secrets.token_bytes(32)
        ix = deposit_ix(
            payer.pubkey(),
            vault,
            pk(spec["source"]),
            pk(spec["mint"]),
            atoms,
            who,
            nonce,
        )
        tx = Transaction.new_signed_with_payer([ix], payer.pubkey(), [payer], bh)
        prepared.append(
            (
                tx,
                dict(
                    signature=str(tx.signatures[0]),
                    recipient=who,
                    amount_atoms=atoms,
                    balance_before=before[who],
                    asset_id=asset,
                    nonce=nonce.hex(),
                    profile_hash=activated["profile_hash"],
                    route_id=p["route_id"],
                ),
            )
        )

    def broadcast(item):
        tx, intent = item
        sig = intent["signature"]
        intent["solana_send"] = time.time()
        save(out / f"{sig}.json", intent)
        real = sol(
            url,
            "sendTransaction",
            base64.b64encode(bytes(tx)).decode(),
            dict(encoding="base64", preflightCommitment="confirmed"),
        )
        if sig != real:
            raise ValueError("signature mismatch")
        return sig

    with ThreadPoolExecutor(len(prepared)) as pool:
        return list(pool.map(broadcast, prepared))


def collect(sig, out, timeout, recovery=None, *, defer_peer_checks=False):
    intent = read(out / f"{sig}.json")
    if recovery:
        intent.update(
            original_profile_hash=intent["profile_hash"],
            profile_hash=recovery["profile_hash"],
            asset_id=recovery["asset_id"],
            balance_before=recovery["balance_before"],
        )
    url = read(STATE / "rpc.json")[0]
    end = time.monotonic() + timeout
    jid = job_id("deposit", sig, intent["route_id"])[2:]
    job = STATE / "relay" / intent["profile_hash"] / "jobs" / jid
    observation = out / "observations" / f"{sig}.json"
    config_path = job.parents[1] / "config.json"
    config = read(config_path) if config_path.is_file() else {}
    relay_observes_balance = bool(
        config.get("warm_signer") and config.get("governance_receipt")
    )
    slot = recovery["slot"] if recovery else None
    while time.monotonic() < end:
        if slot is None:
            status = sol(
                url, "getSignatureStatuses", [sig], {"searchTransactionHistory": True}
            )["value"][0]
            if status and status["err"]:
                raise RuntimeError(str(status))
            if status and status.get("confirmationStatus") == "finalized":
                slot = status["slot"]
            elif status is None and (job / "source.json").is_file():
                # A resumed measurement can outlive the source RPC history.
                # The relay retained this location after its finalized read.
                location = read(job / "source.json")
                if (
                    location.get("kind") == "deposit"
                    and location.get("signature") == sig
                ):
                    slot = location["slot"]
        if slot and (job / "job.json").exists():
            jobdata = read(job / "job.json")
            if (
                not relay_observes_balance
                and not observation.exists()
                and "outbox_id" in jobdata
            ):
                broadcast = (
                    job.parents[1]
                    / "outbox"
                    / jobdata["outbox_id"]
                    / "broadcast-time.json"
                )
                if broadcast.exists():
                    observed = observe_balance(
                        intent["recipient"],
                        intent["asset_id"],
                        intent["balance_before"] + intent["amount_atoms"],
                    )
                    if observed:
                        save(observation, observed)
            if jobdata["status"] == "accepted":
                break
            if (job.parents[1] / "client-mismatch.json").exists():
                raise RuntimeError(
                    f"credit held by certified epoch mismatch, signature={sig}"
                )
            if jobdata["status"] == "failed" and not jobdata["retryable"]:
                raise RuntimeError(str(jobdata.get("error")))
        time.sleep(0.2)
    else:
        raise TimeoutError(f"credit pending, signature={sig} slot={slot}")
    source = read(STATE / "archive" / str(slot) / "events.json")
    receipt = read(job / "receipt.json")
    timing = read(job / "timings.json")
    proof = read(job / "prover-timing.json")
    txid = jobdata["tx_id"]
    asset = intent["asset_id"]
    if jobdata["credited_asset_id"] != asset or not any(
        r["nonce"] == intent["nonce"]
        and int(r["amount_atoms"]) == intent["amount_atoms"]
        for r in receipt["credits"]
    ):
        raise RuntimeError(
            "receipt does not credit the expected source series and deposit"
        )
    expected = intent["balance_before"] + intent["amount_atoms"]
    observed = read(observation) if observation.exists() else None
    for credit in receipt["credits"]:
        measured = credit.get("balance_observation")
        if (
            credit["nonce"] == intent["nonce"]
            and credit["pftl_recipient"] == intent["recipient"]
            and measured
            and measured["balance"] == expected
            and (observed is None or measured["time"] < observed["time"])
        ):
            observed = measured
    if observed is None:
        observed = observe_balance(intent["recipient"], asset, expected)
    if observed is None or observed["balance"] != expected:
        raise RuntimeError("recipient balance delta mismatch")
    save(observation, observed)
    amount, visible = observed["balance"], observed["time"]
    outbox = job.parents[1] / "outbox" / jobdata["outbox_id"]
    ledger = read(outbox / "finality.json")["finality"]
    if not (
        ledger["confirmed"]
        and ledger["receipt"]["tx_id"] == txid
        and ledger["receipt"]["accepted"]
    ):
        raise RuntimeError("finality does not identify this accepted transaction")
    height = ledger["block"]["header"]["height"]
    if defer_peer_checks:
        pending = dict(
            signature=sig,
            amount_atoms=intent["amount_atoms"],
            height=height,
            ledger_tx_id=txid,
            balance_visible=visible,
            solana_send=intent["solana_send"],
            peer_checks_pending=True,
        )
        print(json.dumps(pending), flush=True)
        return pending
    six = receipt_at(txid, height)
    sent = read(outbox / "broadcast-time.json")
    row = dict(
        **intent,
        slot=slot,
        block_id=(STATE / "archive" / str(slot) / "finalized-id").read_text().strip(),
        epoch=source["epoch"],
        proof_sha256=hashlib.sha256((job / "proof.bin").read_bytes()).hexdigest(),
        ledger_tx_id=txid,
        height=height,
        accepted=receipt["accepted"],
        balance_after=amount,
        six_roots=[r["root"] for r in six],
        validators=six,
        finality_rpc={
            k: v for k, v in read(outbox / "finality.json").items() if k.endswith("_ms")
        },
        fee_charged=receipt["fee_charged"],
        times=dict(
            solana_send=intent["solana_send"],
            solana_finality=source["finalized_at"],
            capture_complete=source["capture_complete_at"],
            witness_built=proof.get("witness_built_at", timing["witness_built"]),
            leaves=proof["leaves_at"],
            aggregate_groth16=proof["aggregate_at"],
            proof_done=timing["proof_done"],
            signed=timing["signed"],
            ledger_submit=sent["submit"],
            ledger_finality=sent["finality"],
            balance_visible=visible,
        ),
        prover_seconds=proof["seconds"],
        leaves=proof["shards"],
    )
    if "witness_timestamp_source" in proof:
        row["witness_timestamp_source"] = proof["witness_timestamp_source"]
    output = STATE / "analytics.jsonl"
    with output.open("a+") as f:
        fcntl.flock(f, fcntl.LOCK_EX)
        f.seek(0)
        if not any(json.loads(line)["signature"] == sig for line in f):
            f.seek(0, 2)
            print(json.dumps(row), file=f, flush=True)
            os.fsync(f.fileno())
    print(json.dumps(row), flush=True)
    return row


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--amount", type=int, default=1)
    p.add_argument("--recipient", default=RECIPIENTS[0])
    p.add_argument("--pair-amount", type=int)
    p.add_argument("--pair-recipient", default=RECIPIENTS[1])
    p.add_argument("--resume", nargs="+")
    p.add_argument("--timeout", type=int, default=360)
    p.add_argument(
        "--defer-peer-checks",
        action="store_true",
        help="record proposer finality now and collect the six receipts on resume",
    )
    a = p.parse_args()
    entries = [(a.amount, a.recipient)]
    if a.pair_amount:
        if a.pair_recipient == a.recipient:
            p.error("use distinct recipients for independent balance deltas")
        entries.append((a.pair_amount, a.pair_recipient))
    out = STATE / "intents"
    out.mkdir(exist_ok=True)
    sigs = a.resume or send(entries, out)
    with ThreadPoolExecutor(len(sigs)) as pool:
        list(
            pool.map(
                lambda s: collect(
                    s, out, a.timeout, defer_peer_checks=a.defer_peer_checks
                ),
                sigs,
            )
        )
