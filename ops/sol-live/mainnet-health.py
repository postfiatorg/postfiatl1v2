"""Watch the mainnet Solana observer and the prover. The wallet API closes deposits on a failure.

The observer once stood still for over an hour with its process alive: only its distance
from the public finalized slot shows that.
"""

import json
import os
import runpy
import time
from pathlib import Path

import requests

from live import STATE, read, save

BASE = STATE / "mainnet"
URL = os.environ.get("SOL_MAINNET_RPC", "https://api.mainnet-beta.solana.com")
PROVER = Path(os.environ.get("XDG_RUNTIME_DIR", "/run/user/1000")) / "pft-sol-prover/prover.sock"
# About 30 seconds of slots.
LIMIT = 150
# Measured mainnet slot time (2026-11-20), and a conservative fast bound for sizing the margin.
SLOT_SECONDS = 0.2153
FASTEST_SLOT_SECONDS = 0.1
# How often this watcher rewrites node-health.json.
HEALTH_PERIOD_SECONDS = 20
# The relay's readiness check treats node-health.json older than this as closed
# (tools/pfusdc-sol-relay/http_api.py).
READINESS_STALE_SECONDS = 120
# Task Node re-sends a deposit it has already accepted for up to 14 x 2 s (server/l1/solana-vault.js).
RESEND_SECONDS = 28
# Deposits close this many slots before the next epoch's first slot, about twelve minutes.
# A durable-nonce transaction does not expire by block height: it stays valid until its nonce
# advances. What keeps a deposit from landing between the epoch change and the re-anchor is the
# wallet's send path, which refuses while readiness is closed (Task Node #421) and keeps no signed
# copy. The close reaches it within one stale window, one health period and one re-send window
# (168 s); twelve minutes covers that even at FASTEST_SLOT_SECONDS.
CLOSE_SLOTS = round(12 * 60 / SLOT_SECONDS)
SCHEDULE = None


# A second public provider, asked only when the first fails. One slow reply from one provider
# closed deposits for a hundred seconds (2026-11-24).
FALLBACK_URL = os.environ.get("SOL_MAINNET_RPC_FALLBACK", "https://solana-rpc.publicnode.com")


def call(method, *params):
    body = dict(jsonrpc="2.0", id=1, method=method, params=list(params))
    for url in (URL, FALLBACK_URL):
        try:
            return requests.post(url, json=body, timeout=8).json()["result"]
        except (requests.RequestException, KeyError, ValueError, TypeError):
            if url == FALLBACK_URL:
                raise


# The highest finalized slot this watcher has seen, with the time it was seen. The tip it uses
# never goes below it. It is kept in a file, so a restart during a provider outage does not open
# deposits against a fallback that is behind.
HIGHEST = [None]


def highest_file():
    return BASE / "highest-finalized-slot.json"


def highest():
    """(slot, seen_at) of the highest finalized slot seen; (0, 0.0) before the first. A record that
    exists and cannot be read raises ValueError: the watcher fails closed until it is repaired."""
    if HIGHEST[0] is None:
        path = highest_file()
        if not path.exists():
            HIGHEST[0] = (0, 0.0)
        else:
            try:
                record = read(path)
                slot, seen_at = record["slot"], record["seen_at"]
                if type(slot) is not int or slot < 0 or type(seen_at) not in (int, float):
                    raise TypeError("fields")
            except (OSError, ValueError, KeyError, TypeError) as error:
                raise ValueError(f"cannot read {path.name}: {error}") from error
            HIGHEST[0] = (slot, float(seen_at))
    return HIGHEST[0]


def plausible(slot, now):
    """Whether a provider's slot can be the chain's: not further ahead of the highest seen than the
    chain can have advanced since, at its fastest. One wild answer must not be kept as the highest."""
    top, seen_at = highest()
    return top == 0 or slot <= top + max(0.0, now - seen_at) / FASTEST_SLOT_SECONDS + LIMIT


def remember(slot, now):
    if slot > highest()[0]:
        HIGHEST[0] = (slot, now)
        save(highest_file(), dict(slot=slot, seen_at=now))


def finalized_tip():
    """The public finalized slot, from both providers. A fallback can only make the watcher
    stricter: when the first provider fails, a second answer below the highest slot seen is no
    answer, and two answers further apart than LIMIT close deposits. The higher answer is kept
    before either refusal, so a later check cannot open on the lower one. Raises ValueError."""
    now = time.time()
    answers = {}
    for url in (URL, FALLBACK_URL):
        body = dict(jsonrpc="2.0", id=1, method="getSlot", params=[{"commitment": "finalized"}])
        try:
            slot = requests.post(url, json=body, timeout=8).json()["result"]
        except (requests.RequestException, KeyError, ValueError, TypeError):
            continue
        if type(slot) is int and slot >= 0 and plausible(slot, now):
            answers[url] = slot
    if not answers:
        raise ValueError("no Solana provider answered the finalized slot")
    before = highest()[0]
    remember(max(answers.values()), now)
    if len(answers) == 2 and abs(answers[URL] - answers[FALLBACK_URL]) > LIMIT:
        raise ValueError(f"the Solana providers disagree by {abs(answers[URL] - answers[FALLBACK_URL])} slots")
    if URL not in answers and answers[FALLBACK_URL] < before:
        raise ValueError(f"the fallback provider is {before - answers[FALLBACK_URL]} slots behind "
                         "the highest finalized slot seen")
    return highest()[0]


def epoch_bounds(slot):
    """The epoch containing slot and the first slot of the next epoch, from the cached schedule."""
    global SCHEDULE
    if SCHEDULE is None:
        SCHEDULE = call("getEpochSchedule")
    s = SCHEDULE
    if slot < s["firstNormalSlot"]:
        raise ValueError("slot precedes the normal epoch schedule")
    index = (slot - s["firstNormalSlot"]) // s["slotsPerEpoch"]
    return s["firstNormalEpoch"] + index, s["firstNormalSlot"] + (index + 1) * s["slotsPerEpoch"]


def check(capture):
    # Fail closed: without a public finalized slot nothing shows the epoch or the observer's lag.
    try:
        tip = finalized_tip()
        epoch, boundary = epoch_bounds(tip)
        anchored = read(BASE / "activation.json")["profile"]["solana"]["anchor"]["epoch"]
    except (requests.RequestException, KeyError, ValueError, TypeError) as error:
        return dict(ok=False, message=f"public Solana RPC unavailable: {error}")
    # The route follows one epoch's validator set until a re-anchor or handoff installs the next.
    if epoch != anchored:
        return dict(ok=False, message=f"waiting for the validator set of Solana epoch {epoch}", tip=tip)
    if tip >= boundary - CLOSE_SLOTS:
        return dict(ok=False, message=f"deposits closed for the Solana epoch change at slot {boundary}", tip=tip)
    head = capture["observer_slot"]()
    if head is None:
        return dict(ok=False, message="Solana observer is not answering", tip=tip)
    if tip - head > LIMIT:
        return dict(ok=False, message=f"Solana observer is {tip - head} slots behind", tip=tip, observer=head)
    try:
        capture["check_prover"](PROVER)
    except (OSError, ValueError, RuntimeError) as error:
        return dict(ok=False, message=f"prover unavailable: {error}", tip=tip, observer=head)
    return dict(ok=True, message="", tip=tip, observer=head)


def main():
    capture = runpy.run_path(str(Path(__file__).with_name("mainnet-capture.py")))
    last = None
    while True:
        state = check(capture)
        state["checked_at"] = time.time()
        save(BASE / "node-health.json", state)
        if (state["ok"], state["message"][:19]) != last:
            print(json.dumps(state), flush=True)
            last = (state["ok"], state["message"][:19])
        time.sleep(HEALTH_PERIOD_SECONDS)


if __name__ == "__main__":
    main()
