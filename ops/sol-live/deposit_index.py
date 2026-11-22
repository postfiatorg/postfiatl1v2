"""Recover the signatures that created every captured receipt account in a slot."""

import sys
import time

from solders.pubkey import Pubkey
from live import ROOT, read, save, sol

sys.path.insert(0, str(ROOT / "tools/pfusdc-sol-relay"))
from source import base58, deposit_location


def deposits_at(state, url, vault, slot, events):
    receipts = {
        bytes(Pubkey.from_string(e["address"])).hex()
        for e in events
        if e["tag"] == "PFSOLRC1"
    }
    if not receipts:
        return []
    archive = state / "finalized-transactions"
    index = archive / "slots" / f"{slot}.json"

    def match(transactions):
        found = {}
        for tx in transactions:
            if (
                tx.get("slot") != slot
                or not tx.get("meta")
                or tx["meta"]["err"] is not None
            ):
                continue
            sig = tx["transaction"]["signatures"][0]
            try:
                base58(sig, 64)
                location = deposit_location(tx, sig, vault)
            except ValueError:
                continue
            accounts = {item["receipt_address"] for item in location["receipts"]}
            if accounts & receipts:
                found[sig] = (tx, accounts)
        if set().union(*(v[1] for v in found.values())) != receipts:
            return {}
        return found

    if index.exists():
        keys = read(index)
        if all(isinstance(k, str) and k.isalnum() and len(k) <= 88 for k in keys):
            cached = [archive / f"{key}.json" for key in keys]
            if all(p.is_file() for p in cached):
                found = match(read(p) for p in cached)
                if found:
                    return list(found)
    try:
        block = sol(
            url,
            "getBlock",
            slot,
            dict(
                encoding="json",
                transactionDetails="full",
                rewards=False,
                commitment="finalized",
                maxSupportedTransactionVersion=0,
            ),
        )
    except KeyError:
        block = None
    if block is None:
        # A pruned block may have individual transactions retained by an older relay.
        found = match(
            read(p)
            for p in archive.glob("*.json")
            if not p.name.endswith(".location.json")
        )
    else:
        found = match(dict(tx, slot=slot) for tx in block["transactions"])
    if not found:
        raise RuntimeError(
            f"finalized transactions do not cover every captured receipt at slot {slot}"
        )
    for sig, (tx, _) in found.items():
        path = archive / f"{sig}.json"
        if not path.exists():
            save(path, tx)
    save(index, list(found))
    return list(found)


def retain_transactions(state, url, vault):
    """Capture the full transaction while the source still serves the certified block."""
    events = state / "ledgers" / "certified-events"
    finished = set()
    retry_after = {}
    while True:
        for marker in sorted(
            events.glob("*/events.json"), key=lambda f: int(f.parent.name), reverse=True
        ):
            slot = int(marker.parent.name)
            next_try, delay = retry_after.get(slot, (0, 2))
            if slot in finished or time.monotonic() < next_try:
                continue
            try:
                deposits_at(state, url, vault, slot, read(marker)["events"])
            except (
                OSError,
                RuntimeError,
                KeyError,
                ValueError,
            ) as error:
                retry_after[slot] = (time.monotonic() + delay, min(delay * 2, 3600))
                print(
                    f"retaining finalized transactions at slot {slot}: {error}",
                    flush=True,
                )
            else:
                finished.add(slot)
                retry_after.pop(slot, None)
        time.sleep(0.5)
