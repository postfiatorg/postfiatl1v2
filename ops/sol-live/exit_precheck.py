"""Authenticated inputs and finalized account readback for the one-time bootstrap."""

import base64
import json
import os
from pathlib import Path
import subprocess

import base58
from exit_setup import (
    ASSET,
    BUCKET,
    GENESIS,
    MINT,
    POLICY,
    SYSTEM,
    TOKEN,
    VAULT,
    VERIFIER,
    expected_light,
    pda,
    pk,
    read,
    require,
    rpc,
    save,
    six,
)
from exit_segments import committee, public_checkpoint, validate_segment
from exit_proof import LOCAL, verify_local_host
from route_binding import route_binding

ATA_PROGRAM = pk("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
PHANTOM = pk("Ce5Yv528pKVrVq4JbQchZ8zasCAzxT3f9uCfatTZcfo8")
MINT_AUTH = pk("BJE5MMbqXjVwjAF7oxwPYXnTXDyspzZyt4vwenNw5ruG")
FREEZE_AUTH = pk("7dGbd2QZcCKcTndnHcTL8q7SMVXAkp688NTQYwrRCrar")
INGRESS_TX = "691f599fc249c911d2ccd1c4ddc0b0eb5d1918668b855bec6ec0ace749ff7dc3723388df21c6eb6e55a9f3579d10caee"


def accounts(c, addresses):
    reply = c.call(
        "getMultipleAccounts",
        list(map(str, addresses)),
        dict(commitment="finalized", encoding="base64"),
    )
    rows = [
        dict(a, data=base64.b64decode(a["data"][0], validate=True)) if a else None
        for a in reply["value"]
    ]
    require(len(rows) == len(addresses), "Incomplete account snapshot.")
    return reply["context"]["slot"], rows


def inspect_vault(c):
    config = pda(VAULT, b"config")
    keys = [
        config,
        pda(VAULT, b"custody"),
        MINT,
        pda(ATA_PROGRAM, bytes(PHANTOM), bytes(TOKEN), bytes(MINT)),
    ]
    slot, (cfg, custody, mint, ata) = accounts(c, keys)
    route = route_binding(GENESIS, VAULT)
    require(
        cfg
        and not cfg["executable"]
        and cfg["owner"] == str(VAULT)
        and len(cfg["data"]) == 96
        and cfg["data"][:64] == bytes(MINT) + route,
        "Vault route or mint mismatch.",
    )
    for acc, owner in [(custody, config), (ata, PHANTOM)]:
        if acc is None and owner == PHANTOM:
            continue  # The payout creates the canonical ATA idempotently.
        require(
            acc
            and not acc["executable"]
            and acc["owner"] == str(TOKEN)
            and len(acc["data"]) == 165,
            "Token account owner or layout mismatch.",
        )
        d = acc["data"]
        require(
            d[:64] == bytes(MINT) + bytes(owner)
            and d[108] == 1
            and all(d[start : start + 4] == bytes(4) for start in (72, 109, 129)),
            "Token authority, delegate, close authority or state mismatch.",
        )
    require(
        mint
        and not mint["executable"]
        and mint["owner"] == str(TOKEN)
        and len(mint["data"]) == 82
        and mint["data"][:36] == bytes([1, 0, 0, 0]) + bytes(MINT_AUTH)
        and mint["data"][44:] == bytes([6, 1, 1, 0, 0, 0]) + bytes(FREEZE_AUTH),
        "USDC authority, state or decimals mismatch.",
    )
    return dict(
        slot=slot,
        addresses=list(map(str, keys)),
        route=route.hex(),
        decimals=6,
        custody_atoms=int.from_bytes(custody["data"][64:72], "little"),
        ata_exists=bool(ata),
        mint_authority=str(MINT_AUTH),
        freeze_authority=str(FREEZE_AUTH),
    )


def agreement(rows):
    require(
        len(rows) == 6 and all(r == rows[0] for r in rows),
        "Six validator records differ.",
    )
    return rows[0]


def records(i):
    from profile import profile_hash

    t = rpc(i, "tx", tx_id=INGRESS_TX)
    require(
        t["confirmed"]
        and t["receipt"]["accepted"]
        and t["receipt"]["tx_id"] == INGRESS_TX
        and t["receipt"]["message"] == "vault bridge route profile activated",
        "Ingress activation missing.",
    )
    h = t["block"]["header"]
    (batch,) = rpc(
        i, "batch_archive", batch_kind="governance", batch_id=h["batch_id"], limit=1
    )
    require(
        batch["batch_id"] == h["batch_id"] and batch["batch_kind"] == "governance",
        "Wrong ingress batch.",
    )
    (activation,) = [
        a
        for a in json.loads(batch["payload_json"])[
            "vault_bridge_route_profile_activations"
        ]
        if a["amendment"]["amendment_id"] == INGRESS_TX
    ]
    profile = activation["profile"]
    require(
        profile_hash(profile) == POLICY
        and activation["amendment"]["kind"]
        == f"vault_bridge_route_v1:{ASSET}:{POLICY}",
        "Wrong ingress policy.",
    )
    (bucket,) = [
        b
        for b in rpc(i, "vault_bridge_status", asset_id=ASSET)["buckets"]
        if b["bucket_id"] == BUCKET
    ]
    return dict(activation=dict(profile=profile, profile_hash=POLICY), bucket=bucket)


def archive_cmd(args, store, *command):
    # The verifier needs a writable scratch database. Keep the store integrity file on its original inode.
    mount = "/var/lib/postfiat/validator-3"
    cmd = [
        "docker",
        "run",
        "--rm",
        "--network",
        "none",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "-u",
        f"{os.getuid()}:{os.getgid()}",
        "-v",
        f"{store.resolve()}:{mount}",
        "-v",
        f"{args.binary.resolve()}:/opt/node:ro",
    ]
    if args.integrity_file:
        cmd += ["-v", f"{args.integrity_file.resolve()}:{mount}/.integrity.key:ro"]
    return json.loads(
        subprocess.check_output(
            cmd + ["ubuntu:24.04", "/opt/node", *command, "--data-dir", mount]
        )
    )


def check_certificate(cert, anchor, archive_committee):
    require(
        cert == read(Path(__file__).parent / "fixtures/exit-checkpoint.json")
        and cert["verified"]
        and (
            cert["checkpoint_height"],
            cert["checkpoint_block_hash"],
            cert["checkpoint_state_root"],
            cert["certificate_id"],
        )
        == (anchor["height"], anchor["block"], anchor["root"], anchor["certificate_id"])
        and archive_committee == anchor["committee"],
        "Fresh archive certificate differs from validator anchor.",
    )


def certified_witness(args, cert, parent):
    witness = archive_cmd(
        args,
        args.archive,
        "pfusdc-checkpoint-witness",
        "--prior-checkpoint",
        parent,
        "--target-block",
        cert["checkpoint_block_hash"],
    )
    header = witness["block"]["header"]
    require(
        (
            header["block_hash"],
            header["state_root"],
            header["certificate_id"],
            header["height"],
        )
        == (
            cert["checkpoint_block_hash"],
            cert["checkpoint_state_root"],
            cert["certificate_id"],
            cert["checkpoint_height"],
        ),
        "Archive header disagrees with verified certificate.",
    )
    return witness


def checkpoint_values(raw):
    schema = b"postfiat.pfusdc.checkpoint_public_values.v1"
    prefix = b"PFTL-PFUSDC-TIER4" + len(schema).to_bytes(4, "big") + schema
    require(raw.startswith(prefix), "Wrong checkpoint proof schema.")
    data = raw[len(prefix) :]
    fields = {}
    for tag in range(1, 16):
        require(
            len(data) >= 6 and int.from_bytes(data[:2], "big") == tag,
            "Malformed guest values.",
        )
        size = int.from_bytes(data[2:6], "big")
        require(len(data) >= 6 + size, "Truncated guest values.")
        fields[tag], data = data[6 : 6 + size], data[6 + size :]
    require(
        not data and fields[1] == schema and fields[7] == fields[13],
        "Malformed guest endpoints.",
    )
    return dict(
        prior=fields[6].hex(),
        result=fields[7].hex(),
        height=int.from_bytes(fields[11], "big"),
        start=(fields[10] or fields[9]).hex(),
        end=fields[9].hex(),
    )


def guest_segment(members, anchor, tip, out):
    h = anchor["height"]
    forward = tip > h
    target_height = h + 1 if forward else h
    blocks = agreement(
        six(lambda i: rpc(i, "blocks", from_height=target_height - 1, limit=2))
    )
    prior, target = [b["header"] for b in blocks]
    require(
        (prior if forward else target)["block_hash"] == anchor["block"]
        and target["height"] == target_height,
        "Witness does not touch the trusted block.",
    )
    try:
        w = rpc(
            0,
            "pfusdc_checkpoint_witness",
            prior_checkpoint=prior["block_hash"],
            target_block=target["block_hash"],
        )
    except RuntimeError as err:
        if "rpc_method_not_allowed" not in str(err):
            raise
        w = public_checkpoint(
            rpc, prior["height"], prior["block_hash"], target, members, anchor
        )
    validate_segment(w, prior["block_hash"], target, committee(target))
    out.mkdir(parents=True, exist_ok=True)
    save(out / "witness.json", w)
    verify_local_host()
    subprocess.run(
        [
            LOCAL / "bin/proof-host",
            "checkpoint",
            LOCAL / "historical/egress.elf",
            out / "witness.json",
            out,
            "execute",
        ],
        check=True,
        timeout=300,
    )
    raw = (out / "public-values.bin").read_bytes()
    require(
        raw == (out / "native-public-values.bin").read_bytes(),
        "Frozen guest disagrees with native verifier.",
    )
    values = checkpoint_values(raw)
    require(
        (values["prior"], values["result"], values["height"])
        == (prior["block_hash"], target["block_hash"], target_height)
        and values["start" if forward else "end"] == anchor["committee"],
        "Guest committee differs from tag 3.",
    )
    return dict(forward=forward, **values, execution=read(out / "execute-report.json"))


def bootstrap_state(cfg, rows):
    if all(
        a is None
        or (not a["executable"] and a["owner"] == str(SYSTEM) and not a["data"])
        for a in rows
    ):
        return "empty"
    expected = [expected_light(cfg), bytes.fromhex(cfg["initial_committee"])]
    require(
        len(rows) == 2
        and all(
            a and not a["executable"] and a["owner"] == str(VERIFIER) and a["data"] == d
            for a, d in zip(rows, expected)
        ),
        "Conflicting bootstrap accounts.",
    )
    return "initialized"


def bootstrap_history(c, ix, known=None):
    anchor = ix.accounts[3].pubkey
    recent = (
        []
        if known
        else c.call("getSignaturesForAddress", str(anchor), dict(c.level, limit=100))
    )
    for sig in (
        [known] if known else [r["signature"] for r in recent if r["err"] is None]
    ):
        tx = c.call(
            "getTransaction",
            sig,
            dict(c.level, encoding="json", maxSupportedTransactionVersion=0),
        )
        if (
            not tx
            or tx["meta"]["err"] is not None
            or tx["transaction"]["signatures"][0] != sig
        ):
            continue
        msg = tx["transaction"]["message"]
        keys = msg["accountKeys"]
        for i in msg["instructions"]:
            if (
                keys[i["programIdIndex"]] == str(VERIFIER)
                and base58.b58decode(i["data"]) == ix.data
                and [keys[n] for n in i["accounts"]]
                == [str(a.pubkey) for a in ix.accounts]
            ):
                return sig
    raise ValueError(
        "Finalized bootstrap history unavailable. Supply the initialization signature."
    )
