#!/usr/bin/env python3
"""Bounded adapter from the FastSwap demo UI to generalized-wallet funding."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys


ADDRESS_RE = re.compile(r"^pf[0-9a-f]{40}$")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--stakehub-root", type=Path, required=True)
    parser.add_argument("--profile", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--address", required=True)
    parser.add_argument("--amount-atoms", type=int, required=True)
    parser.add_argument("--status-only", action="store_true")
    args = parser.parse_args()

    if not ADDRESS_RE.fullmatch(args.address):
        raise SystemExit("invalid PostFiat address")
    if args.amount_atoms != 5_000:
        raise SystemExit("FastSwap demo faucet amount must be exactly 5000 atoms")
    if not args.profile.is_file() or not args.stakehub_root.is_dir():
        raise SystemExit("configured faucet authority is unavailable")

    sys.path.insert(0, str(args.stakehub_root))
    from stakehub.generalized_wallet import fleet_method  # noqa: PLC0415

    profile = json.loads(args.profile.read_text(encoding="utf-8"))
    receipt_file = args.run_dir / "receipts" / "native-gas" / "receipt.json"
    if args.status_only:
        if not receipt_file.is_file():
            print(json.dumps({"ok": True, "claimed": False}, sort_keys=True))
            return 0
        result = json.loads(receipt_file.read_text(encoding="utf-8"))
    else:
        from stakehub.generalized_wallet import fund_native_gas  # noqa: PLC0415

        result = fund_native_gas(profile, args.run_dir, args.address, args.amount_atoms)
    account_rows = fleet_method(profile, "account", {"address": args.address})
    account_views = {
        (int(row["balance"]), int(row["sequence"])) for row in account_rows.values()
    }
    if len(account_rows) != 6 or len(account_views) != 1:
        raise RuntimeError("faucet recipient account is not exact-six after funding")
    balance_atoms, sequence = account_views.pop()
    result = {
        **result,
        "claimed": True,
        "balance_atoms": balance_atoms,
        "sequence": sequence,
    }
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
