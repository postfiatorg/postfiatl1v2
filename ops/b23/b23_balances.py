#!/usr/bin/env python3
"""Read-only: on all six validators, B23 supply, route state and the B23 trust-line
and route (native spendable) balances of Alice and Bob. Exit 2 if nodes disagree.
Usage: PYTHONDONTWRITEBYTECODE=1 python3 -u ops/b23/b23_balances.py OUT.json"""
from __future__ import annotations

import json
import sys

sys.path.insert(0, "/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit")
import pftl  # noqa: E402

B23 = "ca7845134c8e95819082fcd2f1612808e20a37dfabab98dab120f99fd2c54c18ffb50d98c0664480420c881773244627"
ALICE = "pfab9b9228942e5c529633a13aa271d5297bec6353"
BOB = "pf5c266a1bdb18320e140961411a8c4d8383eaafb2"
ROUTE = "pftl-pfdep-schwab-primary-pfusdc-v1"


def line(peer, acct):
    for l in pftl.rpc("account_lines", {"account": acct, "limit": 60}, "bl", peer).get("lines") or []:
        if l["asset_id"] == B23:
            return l["balance"]
    return None


def main() -> int:
    rows, keys = {}, set()
    for peer in pftl.PEERS:
        s = pftl.rpc("navcoin_bridge_supply_status", {"route_id": ROUTE}, "bs", peer)
        a = pftl.rpc("asset_info", {"asset_id": B23}, "ba", peer)["asset"]
        route = {r["wallet"]: r["amount_atoms"] for r in s.get("native_spendable_balances") or []}
        row = {"supply": a["outstanding_supply"], "reserve": s["settlement_reserve_atoms"],
               "route_epoch": s["route_epoch"], "policy_epoch": s["policy_epoch"],
               "invariant_holds": s["invariant_holds"],
               "spread_recipient": s.get("spread_recipient", "<absent>"),
               "funding_sequence": s.get("funding_sequence", "<absent>"),
               "alice_line": line(peer, ALICE), "alice_route": route.get(ALICE),
               "bob_line": line(peer, BOB), "bob_route": route.get(BOB), "route_balances": route}
        rows[peer[0]] = row
        keys.add(json.dumps({k: v for k, v in row.items() if k not in ("spread_recipient", "funding_sequence")},
                            sort_keys=True))
        print(peer[0], json.dumps(row, sort_keys=True))
    json.dump(rows, open(sys.argv[1], "w"), indent=1)
    print("AGREE" if len(keys) == 1 else "DISAGREE")
    return 0 if len(keys) == 1 else 2


if __name__ == "__main__":
    sys.exit(main())
