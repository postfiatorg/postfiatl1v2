#!/usr/bin/env python3
"""Read-only: on all six validators, Alice's open offers, the B23/PFUSDC book (both
directions) and Alice's B23 trust line vs route balance. Exit 2 if nodes disagree.
Usage: PYTHONDONTWRITEBYTECODE=1 python3 -u ops/b23/offer_state.py OUT.json"""
from __future__ import annotations

import json
import sys

from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".scratch-deposit"))
import pftl  # noqa: E402

B23 = "ca7845134c8e95819082fcd2f1612808e20a37dfabab98dab120f99fd2c54c18ffb50d98c0664480420c881773244627"
PF = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"
ALICE = "pfab9b9228942e5c529633a13aa271d5297bec6353"
ROUTE = "pftl-pfdep-schwab-primary-pfusdc-v1"


def main() -> int:
    rows, keys = {}, set()
    for peer in pftl.PEERS:
        ao = pftl.rpc("account_offers", {"account": ALICE, "limit": 50}, "ao", peer)
        b1 = pftl.rpc("book_offers", {"taker_gets_asset_id": B23, "taker_pays_asset_id": PF, "limit": 50}, "b1", peer)
        b2 = pftl.rpc("book_offers", {"taker_gets_asset_id": PF, "taker_pays_asset_id": B23, "limit": 50}, "b2", peer)
        lines = pftl.rpc("account_lines", {"account": ALICE, "limit": 60}, "al", peer).get("lines") or []
        line = next((l for l in lines if l["asset_id"] == B23), {})
        s = pftl.rpc("navcoin_bridge_supply_status", {"route_id": ROUTE}, "ss", peer)
        route = {r["wallet"]: r["amount_atoms"] for r in s.get("native_spendable_balances") or []}
        si = pftl.rpc("server_info", {}, "si", peer)["ledger"]
        row = {"height": si["height"], "root": si["state_root"],
               "alice_offers": ao.get("offers", []),
               "book_b23_for_pfusdc": b1.get("offers", []),
               "book_pfusdc_for_b23": b2.get("offers", []),
               "alice_b23_line": line.get("balance"), "alice_b23_line_locked": {k: v for k, v in line.items()
                                                                                  if "lock" in k or "reserv" in k},
               "alice_route": route.get(ALICE), "supply": s.get("authorized_valid_supply_atoms"),
               "reserve": s["settlement_reserve_atoms"], "invariant_holds": s["invariant_holds"]}
        rows[peer[0]] = row
        keys.add(json.dumps(row, sort_keys=True))
        print(peer[0], row["height"], row["root"][:16], "alice_offers", json.dumps(row["alice_offers"])[:160],
              "| line", row["alice_b23_line"], "route", row["alice_route"], "reserve", row["reserve"],
              "inv", row["invariant_holds"])
    json.dump(rows, open(sys.argv[1], "w"), indent=1)
    print("AGREE" if len(keys) == 1 else "DISAGREE")
    return 0 if len(keys) == 1 else 2


if __name__ == "__main__":
    sys.exit(main())
