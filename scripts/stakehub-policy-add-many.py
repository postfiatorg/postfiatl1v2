#!/usr/bin/env python3
"""Add multiple EVM destinations through an already-unlocked StakeHub agent."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from web3 import Web3

from stakehub import agentd


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("addresses", nargs="+")
    parser.add_argument("--home", type=Path, default=None)
    args = parser.parse_args()

    addresses: list[str] = []
    seen: set[str] = set()
    for raw in args.addresses:
        try:
            address = Web3.to_checksum_address(raw)
        except ValueError as error:
            parser.error(f"invalid EVM address {raw!r}: {error}")
        normalized = address.lower()
        if normalized not in seen:
            seen.add(normalized)
            addresses.append(address)

    status = agentd.call({"op": "status"}, args.home)
    if not status or not status.get("ok"):
        print(json.dumps({"ok": False, "error": "StakeHub agent is unavailable"}))
        return 1
    if not status.get("unlocked"):
        print(json.dumps({"ok": False, "error": "StakeHub agent is locked"}))
        return 1

    policy = json.loads(json.dumps(status["policy"]))
    existing = {str(value).lower() for value in policy.get("whitelist", [])}
    added = [address for address in addresses if address.lower() not in existing]
    if not added:
        print(json.dumps({"ok": True, "added": [], "already_present": addresses}))
        return 0
    policy.setdefault("whitelist", []).extend(added)

    response = agentd.call({"op": "set_policy", "policy": policy}, args.home)
    if not response or not response.get("ok"):
        error = response.get("error") if response else "StakeHub agent is unavailable"
        print(json.dumps({"ok": False, "error": error}))
        return 1

    verified = agentd.call({"op": "status"}, args.home)
    final = {
        str(value).lower()
        for value in (verified or {}).get("policy", {}).get("whitelist", [])
    }
    missing = [address for address in addresses if address.lower() not in final]
    if missing:
        print(json.dumps({"ok": False, "error": "post-write verification failed", "missing": missing}))
        return 1
    print(
        json.dumps(
            {
                "ok": True,
                "added": added,
                "whitelist_size": len(final),
                "post_write_verified": True,
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
