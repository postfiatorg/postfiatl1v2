#!/usr/bin/env python3
"""Release-2 final verification (expectations = pre-deploy state at h1236): read-only final verification on all six validators.
Usage: PYTHONDONTWRITEBYTECODE=1 python3 -u ops/b23/final_verify.py OUT.json
Prints PASS/FAIL per check and exits 0 only if every check passes on every node."""
from __future__ import annotations

import json
import sys

from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".scratch-deposit"))
import pftl  # noqa: E402

B23 = "ca7845134c8e95819082fcd2f1612808e20a37dfabab98dab120f99fd2c54c18ffb50d98c0664480420c881773244627"
PFUSDC = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"
ALICE = "pfab9b9228942e5c529633a13aa271d5297bec6353"
BOB = "pf5c266a1bdb18320e140961411a8c4d8383eaafb2"
ROUTE = "pftl-pfdep-schwab-primary-pfusdc-v1"


def b23_line(peer, acct):
    for l in pftl.rpc("account_lines", {"account": acct, "limit": 60}, "fl", peer).get("lines") or []:
        if l["asset_id"] == B23:
            return l["balance"]
    return 0


def main() -> int:
    out, ok, points = {}, True, set()
    for peer in pftl.PEERS:
        st = pftl.rpc("status", {}, "fs", peer)
        s = pftl.rpc("navcoin_bridge_supply_status", {"route_id": ROUTE}, "fss", peer)
        vr = pftl.rpc("vault_bridge_route", {"asset_id": PFUSDC}, "fvr", peer)
        a = pftl.rpc("asset_info", {"asset_id": B23}, "fa", peer)["asset"]
        rcpt = pftl.rpc_raw("pftl_uniswap_receipt", {"tx_id": "ab" * 48}, "fpr", peer)
        route = {r["wallet"]: r["amount_atoms"] for r in s.get("native_spendable_balances") or []}
        row = {
            "build": st["build_git_revision"], "height": st["block_height"], "root": st["state_root"],
            "supply": a["outstanding_supply"], "alice_line": b23_line(peer, ALICE),
            "alice_route": route.get(ALICE), "bob_line": b23_line(peer, BOB), "bob_route": route.get(BOB, 0),
            "reserve": s["settlement_reserve_atoms"], "route_epoch": s["route_epoch"],
            "spread_recipient_present": "spread_recipient" in s, "spread_recipient": s.get("spread_recipient"),
            "funding_sequence_present": "funding_sequence" in s, "funding_sequence": s.get("funding_sequence"),
            "invariant_holds": s["invariant_holds"],
            "anchor_present": "beacon_finality_anchor" in vr, "beacon_finality_anchor": vr.get("beacon_finality_anchor"),
            "vault_route_epoch": vr["profile"]["route_epoch"],
            "receipt_read_ok": rcpt.get("ok"), "receipt_read_error": (rcpt.get("error") or {}).get("code"),
        }
        expect = {"build": "7b4d850d", "supply": 419234350000, "alice_line": 419234350000,
                  "alice_route": 419234350000, "bob_line": 0, "bob_route": 0, "reserve": 33000000,
                  "route_epoch": 4, "spread_recipient_present": True,
                  "spread_recipient": "pfee68d0a5a89e8f554bcd942d2f38659f45bfab1b",
                  "funding_sequence_present": True, "funding_sequence": 1, "invariant_holds": True,
                  "anchor_present": True, "beacon_finality_anchor": None, "vault_route_epoch": 10,
                  "receipt_read_ok": False, "receipt_read_error": "rpc_error"}
        fails = {k: (row[k], v) for k, v in expect.items() if row[k] != v}
        row["fails"] = fails
        ok &= not fails
        out[peer[0]] = row
        points.add((row["height"], row["root"]))
        print(peer[0], "PASS" if not fails else f"FAIL {fails}", row["build"], row["height"], row["root"][:16],
              "receipt_read:", row["receipt_read_error"])
    converged = len(points) == 1
    print("CONVERGED" if converged else f"DIVERGED {points}")
    json.dump(out, open(sys.argv[1], "w"), indent=1)
    print("FINAL", "PASS" if ok and converged else "FAIL")
    return 0 if ok and converged else 2


if __name__ == "__main__":
    sys.exit(main())
