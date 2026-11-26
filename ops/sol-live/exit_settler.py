"""One pass of the installed settlement loop (pft-sol-exit-settler.timer, every 90 s).

It finds the pending burns of covered buckets whose Solana marker reads paid, reading through any
answering validator, and proposes each one's settlement through the one reviewed path:
`exit_govern.py paid --burn B --restricted`. It only proposes. Each host's own reviewer decides
whether that host signs, through the host's restricted door, and with fewer than five signatures
nothing settles. It never signs and never logs in as root to sign.

Every pass holds a lock, so two loops never run. Each settlement has a time bound. A burn that
keeps failing waits longer each time (90 s doubling to an hour). It raises one ALERT when it
starts failing and one when the loop gives up on it after GIVE_UP attempts. Every failure is a
line on stdout, which is the journal.
"""

import fcntl
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
STATE = Path(os.environ.get("SETTLER_STATE", "/var/lib/pft-sol-exit-settler"))
REVISION = os.environ.get("SETTLER_REVISION", "04600868")
RELEASE_HEIGHT = os.environ.get("SETTLER_RELEASE_HEIGHT", "1890")
ATTEMPT_SECONDS = int(os.environ.get("SETTLER_ATTEMPT_SECONDS", "900"))
FIRST_RETRY, MAX_RETRY, GIVE_UP = 90, 3600, 12
# A burn one discovery did not list keeps its count this long: one scan is not a settlement.
ABSENT_KEEP = 7 * 86400


def log(message):
    print(f"{time.strftime('%H:%M:%SZ', time.gmtime())} {message}", flush=True)


def discover():
    """Burns to settle: pending, unsettled, in a bucket the chain covers, marker paid at finalized."""
    from exit_payout import ASSET, BurnCancelled, chain_rules, cluster, covered_buckets, rpc_any, signature

    c = cluster()
    status = rpc_any("vault_bridge_status", asset_id=ASSET)
    covered = covered_buckets(status["buckets"], chain_rules()["covered"])
    found = []
    for r in status["redemptions"]:
        if (r["bucket_id"] in covered and r.get("state") == "pending" and r.get("settled_atoms") == 0
                and not r.get("settlement_receipt_hash")):
            try:
                if signature(c, r["burn_tx_id"], None):
                    found.append(r["burn_tx_id"])
            except BurnCancelled:
                continue  # a refund, not a settlement
            except Exception as error:  # one unreadable marker must not hide the other burns
                log(f"ALERT discovery of {r['burn_tx_id']} failed: {type(error).__name__}: {error}")
    return found


def settle(burn):
    """Propose one settlement through the reviewed path, bounded; returns (ok, last output line)."""
    out = STATE / "settled" / f"{burn}.json"
    out.parent.mkdir(mode=0o750, parents=True, exist_ok=True)
    command = [sys.executable, str(HERE / "exit_govern.py"), "paid", "--burn", burn, "--restricted",
               "--revision", REVISION, "--release-height", RELEASE_HEIGHT, "--output", str(out)]
    try:
        done = subprocess.run(command, capture_output=True, text=True, timeout=ATTEMPT_SECONDS, cwd=HERE)
    except subprocess.TimeoutExpired:
        return False, f"stopped after {ATTEMPT_SECONDS} s"
    text = ((done.stdout or "") + (done.stderr or "")).strip().splitlines()
    return done.returncode == 0, (text[-1] if text else f"exit {done.returncode}")


def delay(failures):
    return min(FIRST_RETRY * 2 ** (failures - 1), MAX_RETRY)


def save_backoff(path, backoff):
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(backoff, sort_keys=True))
    os.replace(tmp, path)


def run_pass(now=None, find=discover, propose=settle, state=None):
    """One pass; returns 0 when nothing failed, 1 otherwise. Each outcome is saved when it is known,
    so a pass the unit's time limit ends keeps what it finished; the burns whose last attempt ended
    longest ago (or never ended) go first, and a burn's backoff starts when its attempt ends."""
    state = Path(state or STATE)
    state.mkdir(mode=0o750, parents=True, exist_ok=True)
    with open(state / "lock", "a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            log("another settlement pass holds the lock; this one does nothing")
            return 0
        now = time.time() if now is None else now
        started = time.monotonic()
        path = state / "backoff.json"
        backoff = json.loads(path.read_text()) if path.exists() else {}
        try:
            burns = find()
        except Exception as error:
            log(f"ALERT discovery failed: {type(error).__name__}: {error}")
            return 1
        failed = False
        for burn in sorted(burns, key=lambda b: backoff.get(b, {}).get("ended", -1)):
            entry = backoff.get(burn, dict(failures=0, next=0, gave_up=False))
            if burn in backoff:
                entry["seen"] = now
            if entry["gave_up"]:
                log(f"not settling {burn}: given up after {entry['failures']} attempts")
                continue
            if now < entry["next"]:
                continue
            log(f"settling {burn}")
            ok, last = propose(burn)
            ended = now + int(time.monotonic() - started)  # whole seconds since the pass began
            if ok:
                log(f"settled {burn}: {last}")
                backoff.pop(burn, None)
                save_backoff(path, backoff)
                continue
            failed = True
            entry["failures"] += 1
            entry["seen"] = now
            entry["ended"] = ended
            entry["next"] = ended + delay(entry["failures"])
            if entry["failures"] >= GIVE_UP:
                entry["gave_up"] = True
                log(f"ALERT settle {burn} gave up after {entry['failures']} attempts: {last}; "
                    f"settle it by hand with exit_govern.py paid --burn {burn}")
            elif entry["failures"] == 1:
                log(f"ALERT settle {burn} failing: {last}; retrying with backoff")
            else:
                log(f"settle {burn} failed (attempt {entry['failures']}): {last}; "
                    f"next in {delay(entry['failures'])} s")
            backoff[burn] = entry
            save_backoff(path, backoff)
        # A burn no longer pending (settled or refunded elsewhere) leaves the backoff, once no
        # discovery has listed it for ABSENT_KEEP: a scan that missed it keeps its count and give-up.
        backoff = {b: e for b, e in backoff.items() if b in burns or now - e.get("seen", now) < ABSENT_KEEP}
        save_backoff(path, backoff)
        return 1 if failed else 0


if __name__ == "__main__":
    sys.path.insert(0, str(HERE))
    sys.exit(run_pass())
