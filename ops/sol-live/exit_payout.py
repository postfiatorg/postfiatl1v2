"""Prove finalized Post Fiat burns and release to the owner's canonical USDC ATA."""

import base64
import hashlib
import logging
import os
from pathlib import Path
from types import SimpleNamespace

from solders.compute_budget import set_compute_unit_limit
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.hash import Hash
from solders.message import Message
from solders.system_program import create_account
from exit_setup import (
    ASSET,
    BUCKET,
    DIRECTORY,
    EXIT,
    GENESIS,
    LIGHT,
    MAINNET_PAYER,
    MINT,
    POLICY,
    SERIES,
    SYSTEM,
    TOKEN,
    VAULT,
    VERIFIER,
    Cluster,
    Instruction,
    checkpoint_pda,
    expected_light,
    pk,
    pda,
    read,
    require,
    ro,
    rpc,
    rw,
    six,
)
from exit_proof import available, prove

ATA = pk("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
PREFIX = "solana-spl:45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0:"
FLOOR = 20_000_000
CONFIG = Path(__file__).parent / "fixtures/exit-setup.json"


def marker(burn):
    return pda(VERIFIER, b"egress-burn", hashlib.sha256(bytes.fromhex(burn)).digest())


def validate_burn(row, burn):
    require(
        row["burn_tx_id"] == burn
        and row["bucket_id"] == BUCKET
        and row["amount_atoms"] > 0,
        "Burn is outside the supported Solana series.",
    )
    destination = row["destination_ref"]
    require(destination.startswith(PREFIX), "Burn has a different destination network.")
    key = Pubkey.from_bytes(bytes.fromhex(destination[len(PREFIX) :]))
    require(
        destination == PREFIX + bytes(key).hex(), "Burn destination is not canonical."
    )
    return key


def cluster():
    c = Cluster(
        SimpleNamespace(
            rpc=os.getenv("SOL_MAINNET_RPC", "https://api.mainnet-beta.solana.com"),
            payer=DIRECTORY / "mainnet-payer.json",
            log=None,
        )
    )
    c.level = {"commitment": "finalized"}
    require(
        c.genesis == GENESIS and c.owner == MAINNET_PAYER,
        "Relayer network or key mismatch.",
    )
    return c


def balance(c, debit=0):
    value = c.call("getBalance", str(c.owner), c.level)["value"]
    require(value >= FLOOR + debit, "Relayer is below the SOL balance floor.")
    return value


def funded_send(c, label, instructions, *signers, rent=0):
    block = Hash.from_string(
        c.call("getLatestBlockhash", c.level)["value"]["blockhash"]
    )
    message = Message.new_with_blockhash(instructions, c.owner, block)
    fee = c.call(
        "getFeeForMessage", base64.b64encode(bytes(message)).decode(), c.level
    )["value"]
    require(isinstance(fee, int), "Solana fee quote unavailable.")
    balance(c, rent + fee)
    return c.send(label, instructions, *signers)


def light(c):
    account = c.account(LIGHT)
    expected = expected_light(read(CONFIG))
    require(
        account
        and account["owner"] == str(VERIFIER)
        and len(account["data"]) == len(expected)
        and account["data"][:232] == expected[:232]
        and account["data"][-1:] == b"\0",
        "Solana verifier is uninitialized, paused or has a different route.",
    )
    return int.from_bytes(account["data"][-9:-1], "little"), account["data"][
        -57:-9
    ].hex()


def readiness():
    try:
        routes = six(lambda i: rpc(i, "vault_bridge_route", asset_id=SERIES))
        require(
            all(
                r["active"]
                and r["profile_hash"] == EXIT
                and r["profile"]["route_epoch"] == 8
                for r in routes
            ),
            "Waiting for the Solana series exit on all six validators.",
        )
        c = cluster()
        light(c)
        lamports = balance(c)
        available()
        s = rpc(0, "vault_bridge_status", asset_id=ASSET)
        b = next((b for b in s["buckets"] if b["bucket_id"] == BUCKET), None)
        require(
            b is not None
            and b["policy_hash"] == POLICY
            and b["source_series_id"] == SERIES
            and b["status"] == "active",
            "Solana source bucket is unavailable.",
        )
        custody = c.account(pda(VAULT, b"custody"))
        require(
            custody
            and custody["owner"] == str(TOKEN)
            and custody["data"][:32] == bytes(MINT)
            and int.from_bytes(custody["data"][64:72], "little")
            >= b["outstanding_vault_bridge_atoms"],
            "Vault custody does not cover the offered series.",
        )
        return dict(
            ok=True,
            ready=True,
            asset_id=ASSET,
            series_asset_id=SERIES,
            bucket_id=BUCKET,
            issuer=s["issuer"],
            epoch=s["finalized_epoch"],
            reserve_packet_hash=s["finalized_reserve_packet_hash"],
            bucket_outstanding_atoms=str(b["outstanding_vault_bridge_atoms"]),
            current_height=routes[0]["current_height"],
            destination_prefix=PREFIX,
            vault_address=str(VAULT),
            relayer_lamports=lamports,
            relayer_floor_lamports=FLOOR,
        )
    except Exception as error:
        return dict(
            ok=True, ready=False, message=str(error)[:300] or type(error).__name__
        )


def signature(c, burn):
    key = marker(burn)
    account = c.account(key)
    if not account or account["owner"] != str(VERIFIER):
        return None
    require(account["data"] == b"\x01", "Unexpected verifier burn marker.")
    # A prefunding transaction can also mention this PDA. Verify the successful
    # tag-2 instruction and its nullifier account position before returning a link.
    import base58

    before = None
    for _ in range(5):
        opts = dict(c.level, limit=20)
        if before:
            opts["before"] = before
        history = c.call("getSignaturesForAddress", str(key), opts)
        for row in history:
            if row["err"] is not None:
                continue
            tx = c.call(
                "getTransaction",
                row["signature"],
                dict(c.level, encoding="json", maxSupportedTransactionVersion=0),
            )
            if not tx or tx["meta"]["err"] is not None:
                continue
            msg = tx["transaction"]["message"]
            keys = msg["accountKeys"]
            for ix in msg["instructions"]:
                if (
                    keys[ix["programIdIndex"]] == str(VERIFIER)
                    and len(ix["accounts"]) >= 7
                    and keys[ix["accounts"][6]] == str(key)
                    and base58.b58decode(ix["data"])[:1] == b"\x02"
                ):
                    return row["signature"]
        if len(history) < 20:
            break
        before = history[-1]["signature"]
    raise ValueError("Burn already paid. Finalized signature history is unavailable.")


def submit(c, kind, witness, work, owner=None, on_pay=None):
    vals, proof = prove(kind, witness, work)
    require(0 < len(vals) <= 2048 and len(proof) == 356, "Invalid proof size.")
    balance(c)
    if on_pay:
        on_pay()
    # Each distinct witness owns one restart-safe upload buffer.
    path = work / (hashlib.sha256(vals).hexdigest() + ".key")
    if not path.exists():
        from live import save
        import json

        save(path, json.loads(Keypair().to_json()))
    from keys import load

    signer = load(path)
    buf = signer.pubkey()
    if not c.account(buf):
        alloc = create_account(
            dict(
                from_pubkey=c.owner,
                to_pubkey=buf,
                lamports=c.rent(len(vals)),
                space=len(vals),
                owner=VERIFIER,
            )
        )
        funded_send(c, "exit.buffer", [alloc], signer, rent=c.rent(len(vals)))
    for off in range(0, len(vals), 700):
        funded_send(
            c,
            "exit.upload",
            [
                Instruction(
                    VERIFIER,
                    b"\x05" + off.to_bytes(2, "little") + vals[off : off + 700],
                    [rw(buf, True)],
                )
            ],
            signer,
        )
    prior = checkpoint_pda(witness["prior_checkpoint_block_id"])
    result = checkpoint_pda(witness["block"]["header"]["block_hash"])
    accounts = [
        rw(c.owner, True),
        rw(LIGHT),
        ro(prior),
        rw(result),
        ro(buf),
        ro(SYSTEM),
    ]
    rent = 0 if c.account(result) else c.rent(48)
    ixs = [set_compute_unit_limit(400_000)]
    if owner is not None:
        ata = pda(ATA, bytes(owner), bytes(TOKEN), bytes(MINT))
        rent += c.rent(1) + (0 if c.account(ata) else c.rent(165))
        ixs.append(
            Instruction(
                ATA,
                b"\x01",
                [
                    rw(c.owner, True),
                    rw(ata),
                    ro(owner),
                    ro(MINT),
                    ro(SYSTEM),
                    ro(TOKEN),
                ],
            )
        )
        accounts.extend(
            [
                rw(marker(witness["receipt"]["tx_id"])),
                ro(VAULT),
                ro(pda(VERIFIER, b"egress-release", bytes(VAULT))),
                ro(pda(VAULT, b"config")),
                rw(pda(VAULT, b"custody")),
                rw(ata),
                ro(owner),
                ro(MINT),
                ro(TOKEN),
            ]
        )
    ixs.append(
        Instruction(VERIFIER, bytes([2 if owner is not None else 8]) + proof, accounts)
    )
    sig = funded_send(c, "exit." + kind, ixs, rent=rent)
    try:
        funded_send(
            c,
            "exit.reclaim",
            [Instruction(VERIFIER, b"\x06", [rw(buf, True), rw(c.owner)])],
            signer,
        )
    except (ValueError, RuntimeError, OSError) as error:
        # The payout or checkpoint is finalized. Preserve the buffer key for rent recovery.
        logging.warning("Buffer rent recovery pending for %s: %s", path, error)
    return sig


def process(burn, work, update):
    update("reading")
    c = cluster()
    red = next(
        (
            r
            for r in rpc(0, "vault_bridge_status", asset_id=ASSET)["redemptions"]
            if r["burn_tx_id"] == burn
        ),
        None,
    )
    require(red is not None, "No finalized redemption for this burn.")
    owner = validate_burn(red, burn)
    h = red["created_at_height"]
    update(
        "reading",
        amount_atoms=str(red["amount_atoms"]),
        recipient=str(owner),
        burn_height=h,
    )
    paid = signature(c, burn)
    if paid:
        update("paid", solana_tx=paid)
        return
    balance(c)
    latest, prior = light(c)
    if latest >= h:
        start = max(1, h - 65)
        blocks = rpc(0, "blocks", from_height=start, limit=h - start)
        candidates = [
            b["header"]["block_hash"]
            for b in reversed(blocks)
            if start <= b["header"]["height"] < h
        ]
        states = c.call(
            "getMultipleAccounts",
            [str(checkpoint_pda(b)) for b in candidates],
            dict(c.level, encoding="base64"),
        )["value"]
        prior = next(
            (
                b
                for b, a in zip(candidates, states)
                if a and a["owner"] == str(VERIFIER)
            ),
            None,
        )
        require(prior, "No known checkpoint precedes this burn.")
    update("proving")
    work.mkdir(parents=True, exist_ok=True)
    while h - latest > 65:
        target = latest + 65
        block = rpc(0, "blocks", from_height=target, limit=1)[0]["header"]
        require(block["height"] == target, "Checkpoint height mismatch.")
        w = rpc(
            0,
            "pfusdc_checkpoint_witness",
            prior_checkpoint=prior,
            target_block=block["block_hash"],
        )
        submit(c, "checkpoint", w, work)
        latest, prior = light(c)
    w = rpc(
        0,
        "pfusdc_egress_witness",
        withdrawal_id=red["redemption_id"],
        prior_checkpoint=prior,
    )
    require(
        w["receipt"]["tx_id"] == burn
        and w["receipt"]["accepted"]
        and w["route_profile"]["profile_hash"] == EXIT
        and w["withdrawal_packet"]["source_bucket_id"] == BUCKET,
        "Witness does not match this burn.",
    )
    sig = signature(c, burn)
    if not sig:

        def paying():
            update("paying")

        sig = submit(c, "withdrawal", w, work, owner, on_pay=paying)
    require(signature(c, burn) == sig, "Waiting for finalized payout signature.")
    update("paid", solana_tx=sig)
