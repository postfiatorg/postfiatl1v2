"""Resume from finalized mainnet custody transactions and submit authenticated deposits."""

import os
from collections import deque
from concurrent.futures import FIRST_COMPLETED, ThreadPoolExecutor, wait
from pathlib import Path
import runpy
import time

from solders.pubkey import Pubkey
from live import STATE, read, save, sol
from bridge import pda

BASE = STATE / "mainnet"
URL = os.environ.get("SOL_MAINNET_RPC", "https://api.mainnet-beta.solana.com")

# Ledger receipt codes that the same deposit gets on every resubmission to this route revision.
# Only these file a deposit as missed; every other failure is retried.
FINAL_REJECTIONS = {
    "solana_source_mismatch": "the deposit's slot or vault lies outside the route's anchor and epoch window",
    "solana_validator_set_mismatch": "the deposit's certificate is signed under another stake table than its epoch's",
    "invalid_solana_deposit_values": "the governed deposit program's journal for this block does not decode",
}
# Not final here: solana_route_superseded and solana_route_mismatch also mean a relay that runs a
# stale profile; duplicate_solana_deposit means the deposit was credited and must not be refunded;
# a proof-verification code also means a prover key that differs from the governed one.

# Exit status after two identical non-final failures on one deposit. The unit does not restart on
# it (RestartPreventExitStatus), so the relay stays down and visible until an operator restarts it.
RELAY_STOPPED = 3

# Submitted deposits whose six-receipt wait (live.receipt_at: up to 90 s per validator) runs off
# the custody loop, which proves and submits the next deposit meanwhile. At most this many at once.
RECEIPT_WAITS = 4


def pending(custody, anchor, cursor):
    rows = []
    params = {"commitment": "finalized", "limit": 100}
    while True:
        page = sol(URL, "getSignaturesForAddress", str(custody), params)
        if not page:
            break
        for row in page:
            if row["slot"] < anchor or row["signature"] == cursor:
                return reversed(rows)
            rows.append(row)
        params["before"] = page[-1]["signature"]
        if len(page) < params["limit"]:
            break
    return reversed(rows)


def record_missed(sig, slot, reason):
    save(BASE / "missed" / (sig + ".json"), dict(signature=sig, slot=slot, reason=reason))
    print(f"missed deposit {sig} at slot {slot}: {reason}", flush=True)


def filed(sig):
    """Credited, missed or skipped: final for this loop, as for the re-anchor gate."""
    return any((BASE / d / (sig + ".json")).exists() for d in ("credits", "missed", "skipped"))


def settled(sig):
    """The deposit is credited or filed: forget any retry record."""
    (BASE / "retrying" / (sig + ".json")).unlink(missing_ok=True)
    stopped = BASE / "relay-stopped.json"
    if stopped.exists() and read(stopped)["signature"] == sig:
        stopped.unlink()


def retry_or_stop(sig, slot, error):
    """Record a non-final failure so the re-anchor gate holds; stop on the second identical one."""
    record = BASE / "retrying" / (sig + ".json")
    message = str(error) or type(error).__name__
    if record.exists() and read(record)["error"] == message:
        save(BASE / "relay-stopped.json", dict(signature=sig, slot=slot, error=message, stopped_at=time.time()))
        print(f"ALERT relay stopped: deposit {sig} at slot {slot} failed twice with: {message}", flush=True)
        raise SystemExit(RELAY_STOPPED)
    save(record, dict(signature=sig, slot=slot, error=message, failed_at=time.time()))
    print(f"retrying deposit {sig} at slot {slot}: {message}", flush=True)


def run():
    activation = read(BASE / "activation.json")
    # The re-anchor restarts this service when the profile it loaded is not the governed one.
    save(BASE / "relay-running.json", dict(profile_hash=activation["profile_hash"], started_at=time.time()))
    module = runpy.run_path(str(Path(__file__).with_name("relay-service.py")))
    from source import deposit_location
    from storage import Invalid, Rejected

    cfg = module["configure"](activation, URL, archive=BASE / "archive")
    relay = module["Relay"](cfg)
    capture = runpy.run_path(str(Path(__file__).with_name("mainnet-capture.py")))
    credit, Missed = capture["credit"], capture["Missed"]
    vault = Pubkey.from_bytes(bytes.fromhex(cfg["vault_program"]))
    custody = pda(vault, b"custody")
    checkpoint = Path(cfg["state"]) / "custody-cursor.json"
    cursor = read(checkpoint)["signature"] if checkpoint.exists() else None
    pins = activation["profile"]["solana"]
    anchor = pins["anchor_slot"]
    # The route's client knows one epoch. Deposits of the next one wait for the re-anchor, which
    # pins the next route at that epoch's first slot and restarts this service.
    boundary = None if pins["epoch_warmup"] else (pins["anchor"]["epoch"] + 1) * pins["slots_per_epoch"]
    print(f"watching mainnet custody {custody} from slot {anchor}", flush=True)
    import requests

    pool = ThreadPoolExecutor(RECEIPT_WAITS)
    # Rows the loop has handled, in custody order, each with its receipt wait or None. The
    # checkpoint passes a row only once it and every earlier row are settled, so a restart
    # retries an unfinished wait; an accepted job then only waits for its receipts again.
    handled = deque()

    def waiting():
        return [w for _, w in handled if w is not None and not w.done()]

    def fail(row, receipts):
        # As if the wait had failed on the loop's path: count it, restart, retry the row.
        error = receipts.exception()
        retry_or_stop(row["signature"], row["slot"], error)
        raise error

    def make_room():
        # A script deposit's balance check expects no other credit while it waits.
        limit = 1 if (BASE / "deposit-intent.json").exists() else RECEIPT_WAITS
        while True:
            # A fleet that has just failed a wait gets no further deposits, as in the serial loop.
            for row, receipts in handled:
                if receipts is not None and receipts.done() and receipts.exception() is not None:
                    fail(row, receipts)
            if len(waiting()) < limit:
                return
            wait(waiting(), return_when=FIRST_COMPLETED)

    def six_receipts(sig, record):
        record()
        settled(sig)

    def advance():
        nonlocal cursor
        while handled and (handled[0][1] is None or handled[0][1].done()):
            row, receipts = handled.popleft()
            if receipts is not None and receipts.exception() is not None:
                fail(row, receipts)
            save(checkpoint, dict(signature=row["signature"], slot=row["slot"]))
            cursor = row["signature"]

    while True:
        advance()
        try:
            rows = list(pending(custody, anchor, cursor))
        except (requests.RequestException, RuntimeError, KeyError) as error:
            # The public RPC rate-limits. Back off and keep the cursor; never exit.
            print(f"source RPC unavailable, retrying: {error}", flush=True)
            time.sleep(15)
            continue
        for row in rows:
            sig = row["signature"]
            if any(r["signature"] == sig for r, _ in handled):
                continue
            if boundary is not None and row["slot"] >= boundary:
                break
            receipts = None
            if filed(sig):
                # Filed before an exit that left the checkpoint behind it; clear a stale retry record.
                settled(sig)
            elif row["err"] is None:
                observed = time.time()
                try:
                    tx = sol(
                        URL,
                        "getTransaction",
                        sig,
                        {"commitment": "finalized", "encoding": "json", "maxSupportedTransactionVersion": 0},
                    )
                except requests.RequestException as error:
                    print(f"source RPC unavailable, retrying: {error}", flush=True)
                    time.sleep(15)
                    break
                if not tx or tx["slot"] != row["slot"]:
                    raise RuntimeError(f"awaiting finalized transaction {sig}")
                try:
                    deposit_location(tx, sig, cfg["vault_program"])
                except Invalid as exc:
                    if str(exc) == "finalized transaction contains no deposit to the configured vault":
                        # A release or a plain transfer into custody. Recording it tells the
                        # epoch-change gate that the relay has passed it.
                        save(BASE / "skipped" / (sig + ".json"), dict(signature=sig, slot=row["slot"], reason=str(exc)))
                    else:
                        record_missed(sig, row["slot"], f"unreadable deposit: {exc}")
                else:
                    if cfg.get("archived_transactions"):
                        # The job's locate() uses this copy instead of fetching it again.
                        save(Path(cfg["archived_transactions"]) / (sig + ".json"), tx)
                    make_room()
                    record = None
                    try:
                        record = credit(relay, activation, sig, row["slot"], observed)
                    except Missed as missed:
                        # The funds stay in the vault. Record the deposit for recovery and
                        # keep later deposits moving.
                        record_missed(sig, row["slot"], str(missed))
                    except Rejected as rejected:
                        if rejected.code not in FINAL_REJECTIONS:
                            # A resubmission has a new transaction id: compare by code.
                            retry_or_stop(sig, row["slot"], f"ledger rejection {rejected.code}")
                            raise
                        record_missed(sig, row["slot"], f"rejected {rejected.code}: {FINAL_REJECTIONS[rejected.code]}")
                    except requests.RequestException as error:
                        # Network, not the deposit: back off and retry this row without counting.
                        print(f"ledger or source RPC unavailable, retrying {sig}: {error}", flush=True)
                        time.sleep(15)
                        break
                    except Exception as error:
                        # A crashed prover, a lagging validator, a configuration that differs from
                        # the governed route: the service restarts and retries this row.
                        retry_or_stop(sig, row["slot"], error)
                        raise
                    if record is None:
                        settled(sig)
                    else:
                        receipts = pool.submit(six_receipts, sig, record)
            handled.append((row, receipts))
            advance()
        if waiting():
            # Checkpoint a finished wait promptly; otherwise scan at the usual pace.
            wait(waiting(), timeout=4, return_when=FIRST_COMPLETED)
        else:
            time.sleep(4)


if __name__ == "__main__":
    run()
