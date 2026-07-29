#!/usr/bin/env python3
"""Wait for a deterministic favorable Ethereum epoch position before deposit.

The wait occurs before the value-moving transaction and is reported
separately from the deposit-inclusion-to-mint SLO.  A head slot at position 28
normally places the next-block deposit near the end of the epoch, reducing the
full-finality critical path without changing the finality rule.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import time
from urllib.request import Request, urlopen


SLOTS_PER_EPOCH = 32


def head_slot(url: str, timeout: float) -> int:
    request = Request(url, headers={"User-Agent": "postfiat-a666-optimizer/1"})
    with urlopen(request, timeout=timeout) as response:
        value = json.load(response)
    return int(value["data"]["header"]["message"]["slot"])


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--beacon-url",
        default="https://ethereum-beacon-api.publicnode.com/eth/v1/beacon/headers/head",
    )
    parser.add_argument("--target-slot-in-epoch", type=int, default=28)
    parser.add_argument("--timeout-seconds", type=float, default=600.0)
    parser.add_argument("--poll-seconds", type=float, default=1.0)
    args = parser.parse_args()

    if args.output.exists():
        raise RuntimeError(f"refusing to overwrite timing evidence: {args.output}")
    if not 0 <= args.target_slot_in_epoch < SLOTS_PER_EPOCH:
        raise RuntimeError("target slot must be in [0, 31]")
    if args.timeout_seconds <= 0 or args.poll_seconds <= 0:
        raise RuntimeError("timeouts must be positive")

    started = time.time()
    monotonic_started = time.monotonic()
    first_slot: int | None = None
    selected_slot: int | None = None
    attempts = 0
    last_error: str | None = None
    while time.monotonic() - monotonic_started < args.timeout_seconds:
        attempts += 1
        try:
            slot = head_slot(args.beacon_url, min(10.0, args.timeout_seconds))
            if first_slot is None:
                first_slot = slot
            if slot % SLOTS_PER_EPOCH == args.target_slot_in_epoch:
                selected_slot = slot
                break
            last_error = None
        except Exception as error:  # transient public endpoint failures are retryable
            last_error = str(error)
        time.sleep(args.poll_seconds)
    if selected_slot is None:
        raise RuntimeError(
            "favorable Ethereum deposit window was not observed before timeout; "
            f"last_error={last_error}"
        )

    completed = time.time()
    report = {
        "schema": "postfiat.a666.ethereum_deposit_window.v1",
        "verdict": "PASS",
        "beacon_url": args.beacon_url,
        "slots_per_epoch": SLOTS_PER_EPOCH,
        "target_slot_in_epoch": args.target_slot_in_epoch,
        "first_observed_slot": first_slot,
        "selected_head_slot": selected_slot,
        "selected_epoch": selected_slot // SLOTS_PER_EPOCH,
        "selected_slot_in_epoch": selected_slot % SLOTS_PER_EPOCH,
        "started_unix_ms": round(started * 1000),
        "selected_unix_ms": round(completed * 1000),
        "wait_ms": round((completed - started) * 1000),
        "poll_attempts": attempts,
        "slo_clock_started": False,
        "finality_policy_changed": False,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
