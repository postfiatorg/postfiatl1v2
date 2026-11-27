"""Idempotent deployment and funding readback on Solana mainnet-beta."""

import argparse
import base64
import fcntl
import hashlib
import math
import json
import os
import secrets
import shlex
import tempfile
import runpy
import struct
import time
import requests
from decimal import Decimal
from datetime import datetime, timezone
import subprocess
from types import SimpleNamespace
from pathlib import Path
from solders.hash import Hash
from solders.transaction import Transaction
from solders.system_program import create_account
from solders.compute_budget import set_compute_unit_limit
from live import STATE, fleet, read, receipt_at, save, six
from govern import draft, govern
from profile import profile, profile_hash
from measure import RECIPIENTS, balance, deposit_ix, series_id
from bridge import (
    Cluster,
    DIRECTORY,
    IMAGES,
    Instruction,
    LOADER,
    MINTS,
    SYSTEM,
    TOKEN,
    VOTE,
    deploy,
    initialize,
    load,
    pda,
    persistent,
    pk,
    require,
    ro,
    rw,
)
from recorder import record
from route_binding import route_binding

BASE = STATE / "mainnet"
GENESIS = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
PAYER = "4JJEuaYtJvgtMVixTbXfppHJPh6tcU5g7g7UtkvxK3sB"
# Builds that carry multi-route Solana clients: 6.2 and the ancestry admission build on top of it.
MULTI_ROUTE_BUILDS = {"d18def8e", "20765df0", "3fe988e6"}
PROGRAMS = (
    ("vault", "vault"),
    ("recorder", "recorder"),
    ("egress_verifier", "egress-verifier"),
)
URL = os.environ.get("SOL_MAINNET_RPC", "https://api.mainnet-beta.solana.com")


def votes(c):
    return c.call(
        "getProgramAccounts",
        str(VOTE),
        {
            **c.level,
            "encoding": "base64",
            "dataSlice": {"offset": 0, "length": 0},
            "filters": [
                {"dataSize": 3762},
                {"memcmp": {"offset": 0, "bytes": "5Sxr3"}},
                {"memcmp": {"offset": 144, "bytes": "2"}},
            ],
        },
    )


def estimate(c, keys, count, *, include_recorder=True):
    required = 0
    transactions = 0
    for label, key in keys.items():
        if c.account(key.pubkey()):
            continue
        size = (IMAGES / f"pfusdc_{label}.so").stat().st_size
        # Loader deploy reuses buffer rent. Its funded program account coexists with the buffer.
        required += c.rent(36) + max(c.rent(45 + size), c.rent(37 + size))
        # A buffer funded by an interrupted run already holds its rent.
        staged = c.account(persistent(f"{GENESIS}-{label}-buffer").pubkey())
        required -= min(staged["lamports"], c.rent(37 + size)) if staged else 0
        transactions += math.ceil(size / 960) + 4
    epoch = c.call("getEpochInfo", c.level)["epoch"]
    vault = keys["vault"].pubkey()
    recorder = keys["recorder"].pubkey()
    rent_accounts = [
        (pda(vault, b"config"), 96),
        (pda(vault, b"custody"), 165),
        (pda(keys["egress_verifier"].pubkey(), b"egress-light"), 289),
    ]
    if include_recorder:
        for kind, size in (
            ("keys", 20 + 48 * count),
            ("next-stakes", 28 + 120 * count),
        ):
            key = persistent(f"{GENESIS}-{recorder}-{epoch}-{kind}")
            rent_accounts.append((key.pubkey(), size))
        transactions += 4 * math.ceil(count / 16)
    for address, size in rent_accounts:
        live = c.account(address)
        required += max(0, c.rent(size) - (live["lamports"] if live else 0))
    required += c.rent(48) + c.rent(132)
    transactions += 10
    return required + 10000 * transactions


def anchor(c):
    # Select only a frozen-bank archive from the live observer, then verify it locally.
    ssh = [
        "ssh",
        "-oBatchMode=yes",
        "-oConnectTimeout=8",
        "-p",
        "63534",
        "root@207.216.48.74",
    ]
    epoch = c.call("getEpochInfo", {"commitment": "finalized"})
    first = epoch["absoluteSlot"] - epoch["slotIndex"]
    archive = BASE / "archive"
    archive.mkdir(parents=True, exist_ok=True)
    required_files = ("validators", "certificate", "shreds", "state", "finalized-id")
    cached = [
        p
        for p in archive.iterdir()
        if p.name.isdecimal()
        and first <= int(p.name) <= epoch["absoluteSlot"]
        and all((p / file).is_file() for file in required_files)
    ]
    if cached:
        return max(cached, key=lambda p: int(p.name))
    vault = load(DIRECTORY / "vault.json").pubkey()
    route = str(Hash.from_bytes(route_binding(GENESIS, vault)))
    command = shlex.join(
        ["python3", "-", str(first), str(epoch["absoluteSlot"]), str(vault), route]
    )
    made = subprocess.run(
        [*ssh, command],
        input=Path(__file__).with_name("observer-anchor.py").read_bytes(),
        stdout=subprocess.PIPE,
        check=True,
    )
    slot = int(made.stdout.decode().splitlines()[-1])
    latest = c.call("getEpochInfo", {"commitment": "finalized"})
    require(
        latest["epoch"] == epoch["epoch"] and first <= slot <= latest["absoluteSlot"],
        "observer anchor is outside current finalized epoch",
    )
    with tempfile.TemporaryDirectory(dir=archive) as tmp:
        stage = Path(tmp) / str(slot)
        subprocess.run(
            [
                "rsync",
                "-a",
                "-e",
                "ssh -oBatchMode=yes -p 63534",
                f"root@207.216.48.74:/work/g/sol-live/archive/{slot}/",
                str(stage) + "/",
            ],
            check=True,
        )
        require(
            all((stage / file).is_file() for file in required_files),
            "incomplete anchor archive",
        )
        final = archive / str(slot)
        if final.exists():
            # Keep partial downloads from the older launcher for diagnosis.
            final.rename(archive / f"partial-{slot}-{time.time_ns()}")
        stage.rename(final)
    return final


def finalized_anchor(c, archive):
    """Cross-check the verified bank's PoH hash and parent against the finalized public RPC."""
    slot = int(archive.name)
    rpc_block = c.call(
        "getBlock",
        slot,
        {
            "commitment": "finalized",
            "transactionDetails": "none",
            "rewards": False,
            "maxSupportedTransactionVersion": 0,
        },
    )
    with (archive / "state").open("rb") as stream:
        stream.seek(96)
        body = stream.read(2192)
    with (archive / "shreds").open("rb") as stream:
        source, parent = struct.unpack("<QQ", stream.read(16))
    poh = str(Hash.from_bytes(body[40:72]))
    parent_poh = str(Hash.from_bytes(body[2160:2192]))
    require(
        (source, parent, poh, parent_poh)
        == (
            slot,
            rpc_block["parentSlot"],
            rpc_block["blockhash"],
            rpc_block["previousBlockhash"],
        ),
        "finalized public PoH or parent differs from verified observer bank",
    )
    witness = STATE / "anchors" / GENESIS / str(slot) / "witness"
    with witness.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    ranks = (archive / "validators").read_bytes()
    return {
        "slot": slot,
        "block_id": (archive / "finalized-id").read_text().strip(),
        "poh": poh,
        "parent_poh": parent_poh,
        "parent_slot": parent,
        "block_time": rpc_block["blockTime"],
        "ranked_validators": int.from_bytes(ranks[16:20], "little"),
        "total_stake": int.from_bytes(ranks[:16], "little"),
        "witness_sha256": digest,
        "witness_bytes": witness.stat().st_size,
    }


def simulate(c, keys, members):
    block = Hash.from_string(
        c.call("getLatestBlockhash", c.level)["value"]["blockhash"]
    )
    results = []

    def submit(label, instructions, *signers):
        tx = Transaction.new_signed_with_payer(
            instructions, c.owner, [c.payer, *signers], block
        )
        response = c.call(
            "simulateTransaction",
            base64.b64encode(bytes(tx)).decode(),
            {"encoding": "base64", "sigVerify": True, "commitment": "confirmed"},
        )
        results.append(
            dict(
                stage=label,
                slot=response["context"]["slot"],
                err=response["value"]["err"],
                units=response["value"].get("unitsConsumed"),
            )
        )

    def create(address, size):
        return create_account(
            dict(
                from_pubkey=c.owner,
                to_pubkey=address,
                lamports=c.rent(size),
                space=size,
                owner=LOADER,
            )
        )

    for label, key in keys.items():
        image = (IMAGES / f"pfusdc_{label}.so").read_bytes()
        buf = persistent(f"{GENESIS}-{label}-buffer")
        addr = buf.pubkey()
        program = key.pubkey()
        submit(
            label + ".buffer",
            [
                create(addr, 37 + len(image)),
                Instruction(LOADER, struct.pack("<I", 0), [rw(addr), ro(c.owner)]),
            ],
            buf,
        )
        chunk = image[:960]
        submit(
            label + ".write",
            [
                Instruction(
                    LOADER,
                    struct.pack("<IIQ", 1, 0, len(chunk)) + chunk,
                    [rw(addr), ro(c.owner, True)],
                )
            ],
        )
        submit(
            label + ".deploy",
            [
                create(program, 36),
                Instruction(
                    LOADER,
                    struct.pack("<IQ", 2, len(image)),
                    [
                        rw(c.owner, True),
                        rw(pda(LOADER, bytes(program))),
                        rw(program),
                        rw(addr),
                        ro(pk("SysvarRent111111111111111111111111111111111")),
                        ro(pk("SysvarC1ock11111111111111111111111111111111")),
                        ro(SYSTEM),
                        ro(c.owner, True),
                    ],
                ),
            ],
            key,
        )
    v = keys["vault"]
    r = keys["recorder"].pubkey()
    mint = pk(MINTS[GENESIS])
    submit(
        "vault.initialize",
        [
            Instruction(
                v.pubkey(),
                b"\0"
                + route_binding(GENESIS, v.pubkey())
                + struct.pack("<QQ", 25000000, 500000000),
                [
                    rw(c.owner, True),
                    ro(v.pubkey(), True),
                    rw(pda(v.pubkey(), b"config")),
                    rw(pda(v.pubkey(), b"custody")),
                    ro(mint),
                    ro(TOKEN),
                    ro(SYSTEM),
                ],
            )
        ],
        v,
    )
    epoch = c.call("getEpochInfo", c.level)["epoch"]
    for tag, kind in enumerate(("keys", "next-stakes")):
        k = persistent(f"{GENESIS}-{r}-{epoch}-{kind}")
        submit(
            "recorder." + kind,
            [
                set_compute_unit_limit(1400000),
                Instruction(
                    r,
                    bytes([tag]),
                    [
                        rw(c.owner, True),
                        rw(k.pubkey(), True),
                        ro(SYSTEM),
                        *(ro(pk(m["pubkey"])) for m in members[:16]),
                    ],
                ),
            ],
            k,
        )
    submit(
        "recorder.publish-scheduled-epoch", [Instruction(r, b"\2", [rw(k.pubkey())])]
    )
    for name, k in keys.items():
        submit(
            name + ".freeze",
            [
                Instruction(
                    LOADER,
                    struct.pack("<I", 4),
                    [rw(pda(LOADER, bytes(k.pubkey()))), ro(c.owner, True)],
                )
            ],
        )
    return results


def verify_budget(estimated_lamports):
    # Rent is recoverable only in some accounts, so reserve the full debit.
    q = requests.get(
        "https://api.exchange.coinbase.com/products/SOL-USD/ticker", timeout=10
    )
    q.raise_for_status()
    quote = q.json()
    timestamp = datetime.fromisoformat(quote["time"].replace("Z", "+00:00"))
    require(
        0 <= (datetime.now(timezone.utc) - timestamp).total_seconds() < 600,
        "stale SOL price",
    )
    ask = Decimal(quote["ask"])
    require(ask > 0, "invalid SOL ask")
    bill = json.loads(
        subprocess.check_output(
            [
                Path.home() / ".local/bin/vastai",
                "show",
                "invoices",
                "--raw",
                "-s",
                "2026-11-19",
            ]
        )
    )
    instances = {61511840, 62617699, 62235343, read(STATE / "gpu.json")["new_contract"]}
    billed = {
        str(instance): sum(
            (
                Decimal(str(row["amount"]))
                for row in bill
                if row.get("instance_id") == instance
            ),
            Decimal(0),
        )
        for instance in instances
    }
    paid = 0
    if (BASE / "transactions.jsonl").exists():
        charges = {}
        for line in (BASE / "transactions.jsonl").read_text().splitlines():
            event = json.loads(line)
            if "payer_debit_lamports" in event:
                charges[event["signature"]] = event["payer_debit_lamports"]
        paid = max(0, sum(charges.values()))
    # Bill readback is accrued spend. Both existing rentals continue to accrue separately.
    usd = sum(billed.values()) + ask * Decimal(estimated_lamports + paid) / Decimal(
        10**9
    )
    require(usd < 2000, "planned costs exceed the authorized USD budget")
    save(
        BASE / "cost-check.json",
        {
            "sol_usd_ask": str(ask),
            "as_of": quote["time"],
            "planned_total_usd": str(usd),
            "billed_vast_usd": {k: str(v) for k, v in billed.items()},
            "confirmed_payer_debit_lamports": paid,
            "reserved_lamports": estimated_lamports,
        },
    )


def run(dry):
    BASE.mkdir(exist_ok=True)
    c = Cluster(
        SimpleNamespace(
            rpc=URL,
            payer=DIRECTORY / "mainnet-payer.json",
            log=BASE / "transactions.jsonl",
        )
    )
    require(
        c.genesis == GENESIS and str(c.owner) == PAYER,
        "unexpected mainnet genesis or payer",
    )
    payer = c.call("getBalance", PAYER, {"commitment": "finalized"})
    save(BASE / "balance.json", payer)
    complete = (BASE / "credit.json").exists() and (BASE / "complete.json").exists()
    if not dry and (complete or payer["value"] == 0):
        print(
            {
                "payer": PAYER,
                "lamports": payer["value"],
                "slot": payer["context"]["slot"],
            }
        )
        return
    keys = {label: load(DIRECTORY / (file + ".json")) for label, file in PROGRAMS}
    accepted = BASE / "activation.json"
    if dry or not accepted.exists():
        activate(c, payer, keys, dry)
        if dry or not accepted.exists():
            return
    deposit_once(c, keys["vault"], pk(MINTS[GENESIS]), read(accepted))


def activate(c, payer, keys, dry):
    members = votes(c)
    require(members, "no current BLS validators")
    needed = estimate(c, keys, len(members))
    head = fleet()
    archive = anchor(c)
    p = profile(URL, archive, head["block_height"] + 1, True)
    save(BASE / "anchor-readback.json", finalized_anchor(c, archive))
    save(BASE / "draft.json", p)
    status = dict(
        balance=payer,
        needed_lamports=needed,
        vote_accounts=len(members),
        route_binding=p["solana"]["route_binding"],
        six_validator_build=head["build_git_revision"],
    )
    if dry:
        amendment = draft(p)
        require(
            amendment["kind"].split(":")[-1] == profile_hash(p),
            "governance and local profile hash disagree",
        )
        save(BASE / "draft-amendment.json", amendment)
        status["unsigned_governance_profile_hash"] = profile_hash(p)
        status["simulations"] = simulate(c, keys, members)
        status["payer_usdc_accounts"] = len(
            c.call(
                "getTokenAccountsByOwner",
                PAYER,
                {"mint": MINTS[GENESIS]},
                {"encoding": "jsonParsed", **c.level},
            )["value"]
        )
        try:
            from mainnet_egress import descriptor

            descriptor(keys["vault"].pubkey(), pk(MINTS[GENESIS]))
        except RuntimeError as error:
            status["egress_bootstrap_pending"] = str(error)
        save(BASE / "dry-run.json", status)
        print(status)
        return
    ingress_needed = estimate(c, keys, len(members), include_recorder=False)
    if payer["value"] < ingress_needed:
        save(BASE / "waiting.json", status)
        print(status)
        return
    verify_budget(needed if payer["value"] >= needed else ingress_needed)
    for label, key in keys.items():
        deploy(c, label, key)
    vault = keys["vault"]
    mint = pk(MINTS[GENESIS])
    initialize(c, vault, mint, route_binding(GENESIS, vault.pubkey()))
    if payer["value"] >= needed:
        record(c, keys["recorder"].pubkey())
    else:
        save(BASE / "recorder-funding.json", status)
    if head["build_git_revision"] not in MULTI_ROUTE_BUILDS:
        status["waiting_for"] = (
            "validator support for concurrent local and mainnet clients"
        )
        save(BASE / "waiting.json", status)
        return
    from mainnet_egress import MissingSolanaDescriptor, bootstrap, descriptor

    try:
        exit_descriptor = descriptor(vault.pubkey(), mint)
    except MissingSolanaDescriptor as error:
        exit_descriptor = None
        save(BASE / "egress.json", dict(ready=False, reason=str(error)))
    plan = BASE / "activation-plan.json"
    if not plan.exists():
        p["activation_height"] = fleet()["block_height"] + 1
        save(plan, dict(profile=p, profile_hash=profile_hash(p)))
    planned = read(plan)
    p = planned["profile"]
    require(
        planned["profile_hash"] == profile_hash(p), "mainnet activation plan changed"
    )
    if exit_descriptor is not None:
        bootstrap(
            c,
            keys["egress_verifier"],
            planned,
            exit_descriptor,
            read(STATE / "local-activation.json"),
        )
        for label, key in keys.items():
            data = pda(LOADER, bytes(key.pubkey()))
            if c.account(data)["data"][12]:
                c.send(
                    label + ".freeze",
                    [
                        Instruction(
                            LOADER, struct.pack("<I", 4), [rw(data), ro(c.owner, True)]
                        )
                    ],
                )
            require(c.account(data)["data"][12] == 0, "program authority still present")
        save(
            BASE / "egress.json",
            dict(ready=True, profile_hash=exit_descriptor["profile_hash"]),
        )
    from govern import recover

    evidence = recover(p) or govern("route", p, compatible=head["build_git_revision"])
    require(
        evidence["profile_hash"] == planned["profile_hash"],
        "governed profile differs from activation plan",
    )
    save(BASE / "activation.json", evidence)


def deposit_once(c, vault, mint, activation):
    require(
        receipt_at(activation["tx_id"], activation["height"])
        == activation["validators"],
        "mainnet governance readback differs",
    )
    subprocess.run(
        ["systemctl", "--user", "start", "pft-sol-mainnet-relay.service"], check=True
    )
    owners = c.call(
        "getTokenAccountsByOwner",
        PAYER,
        {"mint": str(mint)},
        {"encoding": "jsonParsed", **c.level},
    )
    source = next(
        (
            row
            for row in owners["value"]
            if int(row["account"]["data"]["parsed"]["info"]["tokenAmount"]["amount"])
            >= 1
        ),
        None,
    )
    intent = BASE / "deposit-intent.json"
    if source or intent.exists():
        if not intent.exists():
            asset = series_id(activation)
            before = six(lambda i: balance(RECIPIENTS[0], asset, i))
            require(len(set(before)) == 1, "opening recipient balances disagree")
            available = int(
                source["account"]["data"]["parsed"]["info"]["tokenAmount"]["amount"]
            )
            save(
                intent,
                dict(
                    nonce=secrets.token_hex(32),
                    source=source["pubkey"],
                    amount_atoms=min(100_000, available),
                    recipient=RECIPIENTS[0],
                    asset_id=asset,
                    opening_balances=before,
                ),
            )
        request = read(intent)
        nonce = bytes.fromhex(request["nonce"])
        receipt = pda(vault.pubkey(), b"receipt", nonce)
        capture = runpy.run_path(str(Path(__file__).with_name("mainnet-capture.py")))
        if not c.account(receipt):
            source_epoch = c.call("getEpochInfo", {"commitment": "finalized"})["epoch"]
            require(
                source_epoch == activation["profile"]["solana"]["anchor"]["epoch"],
                "verify the successor handoff before a new operator deposit",
            )
            # The receipt's unique nonce makes retransmission safe after a restart.
            # Require a primed proof backend and a caught-up capture before custody.
            sock = Path(f"/run/user/{os.getuid()}/pft-sol-prover/prover.sock")
            capture["check_prover"](sock)
            watcher = capture["arm"](c)
            ix = deposit_ix(
                c.owner,
                vault.pubkey(),
                pk(request["source"]),
                mint,
                request.get("amount_atoms", 1),
                RECIPIENTS[0],
                nonce,
            )
            try:
                request["solana_send"] = time.time()
                save(intent, request)
                request["signature"] = c.send("mainnet.deposit", [ix])
                save(intent, request)
            finally:
                watcher.wait(timeout=65)
        account = c.account(receipt)
        require(
            account["owner"] == str(vault.pubkey())
            and account["data"]
            == b"PFSOLRC1"
            + route_binding(GENESIS, vault.pubkey())
            + bytes.fromhex(RECIPIENTS[0][2:])
            + struct.pack("<Q", request.get("amount_atoms", 1))
            + nonce
            + bytes(c.owner),
            "deposit receipt mismatch",
        )
        if "signature" not in request:
            landed = c.call(
                "getSignaturesForAddress",
                str(receipt),
                {"limit": 10, "commitment": "finalized"},
            )
            sigs = [row["signature"] for row in landed if row["err"] is None]
            require(len(sigs) == 1, "cannot identify the confirmed receipt creation")
            request["signature"] = sigs[0]
            save(intent, request)
        result = BASE / "credits" / (request["signature"] + ".json")
        if not result.exists():
            print("Awaiting relay credit", request["signature"], flush=True)
            return
        credit = read(result)
        require(
            credit["signature"] == request["signature"]
            and any(
                row["nonce"] == request["nonce"]
                and int(row["amount_atoms"]) == request.get("amount_atoms", 1)
                and row["pftl_recipient"] == RECIPIENTS[0]
                for row in credit["credits"]
            ),
            "credit does not identify the deposit",
        )
        if "opening_balances" in request:
            closing = six(
                lambda i: balance(request["recipient"], request["asset_id"], i)
            )
            require(
                closing
                == [x + request["amount_atoms"] for x in request["opening_balances"]],
                "six recipient balance deltas differ",
            )
            require(
                credit["credited_asset_id"] == request["asset_id"],
                "wrong credited source series",
            )
            credit.update(
                opening_balances=request["opening_balances"], closing_balances=closing
            )
            credit["times"].setdefault("six_balances_seen", time.time())
        save(BASE / "credit.json", credit)
        save(
            BASE / "complete.json",
            dict(activation=activation["tx_id"], credit=credit["ledger_tx_id"]),
        )
    else:
        print("Deposits active; awaiting USDC at", PAYER, flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    BASE.mkdir(parents=True, exist_ok=True)
    with (BASE / "launch.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        run(args.dry_run)
