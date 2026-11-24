"""Governed re-anchor of the mainnet Solana route at the current epoch's first slot.

Validator governance installs the epoch's ranked validator set, as it did at activation.
Run after an epoch change when no proved handoff is available, so deposits keep flowing.
The revision also pins the current handoff and deposit proof program keys.
"""

import fcntl
import json
import runpy
import subprocess
import sys
import time
from pathlib import Path
from types import SimpleNamespace

from govern import govern, recover
from live import STATE, fleet, read, receipt_at, rpc, save
from profile import profile, profile_hash

BASE = STATE / "mainnet"
RELAY = STATE / "relay"
# getSignaturesForAddress page size; reads continue with `before` until the old anchor.
PAGE = 1000


def custody_rows(c, custody, floor):
    """Every finalized custody transaction above floor, newest first, across all pages."""
    rows, params = [], {"limit": PAGE, "commitment": "finalized"}
    while True:
        page = c.call("getSignaturesForAddress", custody, params)
        for row in page:
            if row["slot"] <= floor:
                return rows
            rows.append(row)
        if len(page) < PAGE:
            return rows
        params = dict(params, before=page[-1]["signature"])


# The profile each service reports when it starts. The health watcher reads activation.json on
# every check and needs no restart.
RUNNING = {"relay-running.json": "pft-sol-mainnet-relay", "wallet-relay.json": "pft-sol-mainnet-wallet-api"}
# A service still stale this long after its restart for the same profile is restarted again. A
# service that fails before it writes its status file is then not restarted on every timer run.
RESTART_BACKOFF_SECONDS = 600


def restart_stale(activation):
    """Restart each service that did not load the governed profile, at most once per back-off."""
    want = activation["profile_hash"]
    restarts = BASE / "service-restarts.json"
    last = read(restarts) if restarts.exists() else {}
    now = time.time()
    stale, held = [], []
    for name, unit in RUNNING.items():
        if (BASE / name).exists() and read(BASE / name).get("profile_hash") == want:
            continue
        previous = last.get(unit)
        if previous and previous["profile_hash"] == want and now - previous["at"] < RESTART_BACKOFF_SECONDS:
            held.append(unit)
        else:
            stale.append(unit)
    for unit in held:
        print(f"ALERT {unit} still not at profile {want} after its restart; next restart after the back-off", flush=True)
    if stale:
        subprocess.run(["systemctl", "--user", "restart", *stale], check=True)
        save(restarts, dict(last, **{unit: dict(profile_hash=want, at=now) for unit in stale}))
        print(f"restarted {', '.join(stale)}: not at profile {want}", flush=True)


def filed(signature):
    return any((BASE / d / (signature + ".json")).exists() for d in ("credits", "missed", "skipped"))


def retrying():
    """Deposits the relay is retrying, oldest first. Each holds the gate until it is settled."""
    return sorted((read(path) for path in (BASE / "retrying").glob("*.json")), key=lambda r: r["slot"])


ACTIVATED = "vault bridge route profile activated"
# Blocks read per request while scanning for a successor revision. A withdrawal-era block can
# be large, and a reply over the node's 4 MB frame limit cannot be sent: 256 blocks did not fit.
BLOCK_PAGE = 8


def accepted_activation(tx_id):
    found = rpc(0, "receipts", tx_id=tx_id, limit=1)
    return bool(found) and found[-1].get("accepted") is True and found[-1].get("message") == ACTIVATED


def chain_revision(old):
    """The route's next revision if the chain already has it, as (tx_id, height, profile), else None.

    Reads every governance batch above the current revision's height, so the result does not
    depend on how many receipts came since. Each activation counts only under its own amendment:
    that amendment's receipt must be accepted and its kind must name the profile's hash.
    """
    route, current = old["profile"]["route_id"], old["profile"]["route_epoch"]
    later, height = [], old["height"] + 1
    while True:
        blocks = rpc(0, "blocks", from_height=height, limit=BLOCK_PAGE)
        for block in blocks:
            header = block["header"]
            if header["batch_kind"] != "governance":
                continue
            for entry in rpc(0, "batch_archive", batch_kind="governance", batch_id=header["batch_id"], limit=1):
                for activation in json.loads(entry["payload_json"]).get("vault_bridge_route_profile_activations", []):
                    candidate = activation["profile"]
                    if candidate["route_id"] != route or candidate["route_epoch"] <= current:
                        continue
                    amendment = activation["amendment"]
                    if not accepted_activation(amendment["amendment_id"]):
                        continue
                    if amendment["kind"].split(":")[-1] != profile_hash(candidate):
                        raise RuntimeError(f"route activation {amendment['amendment_id']} differs from its amendment")
                    later.append((amendment["amendment_id"], header["height"], candidate))
        if len(blocks) < BLOCK_PAGE:
            break
        height = blocks[-1]["header"]["height"] + 1
    if not later:
        return None
    latest = max(later, key=lambda found: found[2]["route_epoch"])
    if latest[2]["route_epoch"] != current + 1:
        raise RuntimeError(f"chain route revision {latest[2]['route_epoch']} is not the successor of {current}")
    return latest


def plan_for(go, c, old, epoch, first):
    """One plan per target epoch and replaced revision; every retry reuses it.

    The route is pinned at the epoch's first slot, not at the certified bank the observer
    captured later in the epoch. The ledger keeps only the slot and the epoch's stake table
    (set hash, total stake, key table), which is the same for every bank of the epoch; no bank
    hash or block id is filed under the slot. Deposits made in the new epoch before this
    revision lands then lie at or above the anchor and are credited once it does.
    """
    plan = BASE / f"reanchor-plan-epoch-{epoch}-from-{old['profile']['route_epoch']}.json"
    if plan.exists():
        saved = read(plan)
        return saved["profile"], Path(saved["archive"])
    pins = old["profile"]["solana"]
    archive = go["anchor"](c)
    if int(archive.name) <= pins["anchor_slot"]:
        sys.exit("no certified bank after the current anchor yet")
    p = profile(go["URL"], archive, fleet()["block_height"] + 1, True)
    p["route_epoch"] = old["profile"]["route_epoch"] + 1
    # A same-epoch re-anchor cannot move back to the epoch start: anchors only advance.
    if first > pins["anchor_slot"]:
        p["solana"]["anchor_slot"] = first
    for field in ("vault_address", "token_address", "asset_id", "route_id"):
        if p[field] != old["profile"][field]:
            raise RuntimeError(f"re-anchor changes {field}")
    if p["solana"]["route_binding"] != pins["route_binding"]:
        raise RuntimeError("re-anchor changes the route binding")
    save(plan, dict(profile=p, profile_hash=profile_hash(p), previous=old["profile_hash"], archive=str(archive)))
    return p, archive


def main():
    BASE.mkdir(parents=True, exist_ok=True)
    with (BASE / "reanchor.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            sys.exit("another re-anchor is running")
        run()


def run():
    go = runpy.run_path(str(Path(__file__).with_name("mainnet-go.py")))
    c = go["Cluster"](
        SimpleNamespace(
            rpc=go["URL"],
            payer=go["DIRECTORY"] / "mainnet-payer.json",
            log=BASE / "transactions.jsonl",
        )
    )
    old = read(BASE / "activation.json")
    pins = old["profile"]["solana"]
    info = c.call("getEpochInfo", {"commitment": "finalized"})
    epoch, first = info["epoch"], info["absoluteSlot"] - info["slotIndex"]
    if "--same-epoch" not in sys.argv and epoch == pins["anchor"]["epoch"]:
        print(f"route is already anchored in epoch {epoch}")
        restart_stale(old)
        return
    custody = str(go["pda"](go["load"](go["DIRECTORY"] / "vault.json").pubkey(), b"custody"))
    # The relay handles custody rows in slot order: a row below its checkpoint has been passed.
    checkpoint = RELAY / old["profile_hash"] / "custody-cursor.json"
    cursor = read(checkpoint) if checkpoint.exists() else dict(signature=None, slot=-1)
    open_rows = [
        row
        for row in custody_rows(c, custody, pins["anchor_slot"])
        if row["err"] is None
        and row["slot"] >= cursor["slot"]
        and row["signature"] != cursor["signature"]
        and not filed(row["signature"])
    ]
    # A deposit the relay is retrying is not settled, whatever the custody read shows.
    for held in retrying():
        sys.exit(f"waiting: the relay is retrying {held['signature']} ({held['error']})")
    # The ledger still holds the old epoch's set, so let the relay finish that epoch's deposits.
    waiting = [row["signature"] for row in open_rows if row["slot"] < first or "--same-epoch" in sys.argv]
    if waiting:
        sys.exit(f"waiting for the relay to finish {waiting[0]}")
    revision = chain_revision(old)
    if revision is not None:
        # An earlier run committed this revision but lost its local evidence: adopt it.
        tx_id, height, p = revision
        evidence = dict(tx_id=tx_id, height=height, validators=receipt_at(tx_id, height), profile=p, profile_hash=profile_hash(p))
    else:
        p, archive = plan_for(go, c, old, epoch, first)
        readback = go["finalized_anchor"](c, archive)
        save(BASE / f"anchor-readback-{archive.name}.json", dict(readback, anchor_slot=p["solana"]["anchor_slot"]))
        head = fleet()
        evidence = recover(p) or govern("route", p, compatible=head["build_git_revision"])
        if evidence["profile_hash"] != profile_hash(p):
            raise RuntimeError("governed profile differs from the planned re-anchor")
        if receipt_at(evidence["tx_id"], evidence["height"]) != evidence["validators"]:
            raise RuntimeError("governance readback differs")
    slot = p["solana"]["anchor_slot"]
    save(BASE / f"activation-revision-{old['profile']['route_epoch']}.json", old)
    save(BASE / "activation.json", evidence)
    # A deposit made in the new epoch before its set was installed lies below the new anchor.
    # Re-read custody: deposits finalized while this run captured and governed count too.
    for row in custody_rows(c, custody, pins["anchor_slot"]):
        if row["err"] is None and row["slot"] < slot and not filed(row["signature"]):
            save(
                BASE / "missed" / (row["signature"] + ".json"),
                dict(signature=row["signature"], slot=row["slot"], reason=f"below re-anchor slot {slot}"),
            )
    restart_stale(evidence)
    print(
        json.dumps(
            dict(
                height=evidence["height"],
                profile_hash=evidence["profile_hash"],
                route_epoch=p["route_epoch"],
                anchor_epoch=p["solana"]["anchor"]["epoch"],
                anchor_slot=slot,
                handoff_key=p["solana"]["handoff_program_vkey"],
                deposit_key=p["verifier_program_vkey"],
            )
        )
    )


if __name__ == "__main__":
    main()
