"""Authenticated inputs and finalized account readback for the one-time bootstrap."""

import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

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
    checkpoint_pda,
    LIGHT,
    pda,
    pk,
    read,
    require,
    rpc,
    save,
    six,
)
from exit_segments import committee, public_checkpoint, select_target, validate_segment
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


def token_atoms(acc, owner, *, allow_empty=False):
    if allow_empty and (acc is None or (
        not acc["executable"] and acc["owner"] == str(SYSTEM) and not acc["data"]
    )):
        return 0
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
    return int.from_bytes(d[64:72], "little")


def inspect_vault(c, recipient=None):
    config = pda(VAULT, b"config")
    keys = [
        config,
        pda(VAULT, b"custody"),
        MINT,
    ]
    if recipient is not None:
        keys.append(pda(ATA_PROGRAM, bytes(recipient), bytes(TOKEN), bytes(MINT)))
    slot, rows = accounts(c, keys)
    cfg, custody, mint = rows[:3]
    route = route_binding(GENESIS, VAULT)
    require(
        cfg
        and not cfg["executable"]
        and cfg["owner"] == str(VAULT)
        and len(cfg["data"]) == 96
        and cfg["data"][:64] == bytes(MINT) + route,
        "Vault route or mint mismatch.",
    )
    token_atoms(custody, config)
    if recipient is not None:
        token_atoms(rows[3], recipient, allow_empty=True)
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
        **({"ata_exists": bool(rows[3] and rows[3]["data"])} if recipient is not None else {}),
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


def execute_guest(w, out):
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
    return dict(**checkpoint_values(raw), execution=read(out / "execute-report.json"))


def parent_of(head):
    return {k: head[k] for k in ("block_height", "block_tip_hash", "state_root")}


def check_unproved_updates(w):
    # A registry update at the last block has no successor QC in this segment.
    # Authenticate even an apparently empty governance payload before accepting it.
    for step in [*w.get("finality_ancestry", []), w]:
        h = step["block"]["header"]
        if h["batch_kind"] != "governance" or "governance_payload_json" in step:
            continue
        (batch,) = rpc(0, "batch_archive", batch_kind="governance", batch_id=h["batch_id"], limit=1)
        payload = batch["payload_json"]
        fields = [w[k] for k in ("chain_id", "genesis_hash", "protocol_version")]
        fields.extend(["governance", h["batch_id"], payload])
        encoded = json.dumps(fields, ensure_ascii=False, separators=(",", ":")).encode()
        digest = hashlib.sha3_384(b"postfiat.batch_archive_payload.v1\0" + encoded).hexdigest()
        require(batch["batch_id"] == h["batch_id"] and
                digest == h["consensus_v2_commit"]["precommit_qc"]["block"]["payload_hash"],
                "Governance archive differs from finalized payload.")
        require(not json.loads(payload).get("validator_registry_updates"),
                "Registry rotation requires a verified transition and successor.")


def qualify_path(members, anchor, head, out, *, simulation_height=None):
    height, block, root = anchor["height"], anchor["block"], anchor["committee"]
    tip = head["block_height"]
    require(tip >= height, "Finalized tip precedes trusted checkpoint.")
    if simulation_height is None:
        simulation_height = height
    require(height <= simulation_height <= tip, "Current checkpoint is outside the finalized path.")
    segments = []
    simulation = None
    state = anchor["root"]
    with tempfile.TemporaryDirectory() as tmp:
        while height < tip:
            stop = simulation_height if height < simulation_height else tip
            if stop - height > 65:
                target = select_target(height, rpc)
            else:
                (record,) = rpc(0, "blocks", from_height=stop, limit=1)
                target = record["header"]
                require(target["height"] == stop, "Wrong checkpoint height.")
            witness = public_checkpoint(rpc, height, block, target, members, anchor)
            check_unproved_updates(witness)
            validate_segment(witness, block, target, root)
            use_for_simulation = height == simulation_height
            values = execute_guest(witness, out if use_for_simulation else Path(tmp))
            require((values["prior"], values["start"], values["result"], values["end"], values["height"])
                    == (block, root, target["block_hash"], committee(target), target["height"]),
                    "Frozen checkpoint endpoints differ from finalized path.")
            if use_for_simulation:
                simulation = values
            segments.append(values)
            height, block, root = values["height"], values["result"], values["end"]
            members, state = witness["committee"], target["state_root"]
    require((height, block, state) == (tip, head["block_tip_hash"], head["state_root"]),
            "Frozen path differs from the agreed finalized tip.")
    return dict(qualified_parent=parent_of(head), segments=segments,
                guest=dict(simulation or {}, forward=simulation is not None),
                cursor=dict(anchor, height=height, block=block, root=state, committee=root),
                members=members)


def bootstrap_state(cfg, rows):
    if all(
        a is None
        or (not a["executable"] and a["owner"] == str(SYSTEM) and not a["data"])
        for a in rows
    ):
        return "empty"
    require(len(rows) == 2 and all(a and not a["executable"] and a["owner"] == str(VERIFIER)
                                   for a in rows), "Conflicting bootstrap accounts.")
    require(rows[1]["data"] == bytes.fromhex(cfg["initial_committee"]),
            "Conflicting original checkpoint.")
    light_fields(cfg, rows[0])
    return "initialized"


def light_fields(cfg, account):
    expected = expected_light(cfg)
    require(account and not account["executable"] and account["owner"] == str(VERIFIER),
            "Unexpected light account owner.")
    raw = account["data"]
    require(len(raw) == len(expected) and raw[:-57] == expected[:-57] and raw[-1] in (0, 1),
            "Conflicting immutable light fields.")
    height = int.from_bytes(raw[-9:-1], "little")
    block = raw[-57:-9].hex()
    require(height >= cfg["initial_height"] and
            ((height == cfg["initial_height"]) == (block == cfg["initial_checkpoint"])),
            "Invalid latest checkpoint.")
    return height, block


def recover_bootstrap(c, cfg, ix, known=None):
    keys = [LIGHT, checkpoint_pda(cfg["initial_checkpoint"])]
    _, rows = accounts(c, keys)
    if bootstrap_state(cfg, rows) == "empty":
        return None
    sig = bootstrap_history(c, ix, known)
    height, block = light_fields(cfg, rows[0])
    slot, snapshot = accounts(c, [*keys, checkpoint_pda(block)])
    require(rows == snapshot[:2], "Light client changed during readback. Retry.")
    (record,) = agreement(six(lambda i: rpc(i, "blocks", from_height=height, limit=1)))
    h = record["header"]
    cp = snapshot[2]
    require((h["height"], h["block_hash"]) == (height, block)
            and cp and not cp["executable"] and cp["owner"] == str(VERIFIER)
            and cp["data"].hex() == committee(h), "Current checkpoint disagrees with finalized history.")
    return dict(signature=sig, finalized_readback=True, slot=slot, block=block, height=height,
                committee=committee(h), paused=bool(rows[0]["data"][-1]))


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
