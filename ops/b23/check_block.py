#!/usr/bin/env python3
"""After one transaction: read block HEIGHT from all six (header + every receipt) and
the given accounts' balances/lines, and require identical results everywhere.
Usage: PYTHONDONTWRITEBYTECODE=1 python3 ops/b23/check_block.py OUT.json HEIGHT [ACCOUNT ...]
Exit 0 = all six agree (height, root, block hash, receipts); 2 = disagreement."""
from __future__ import annotations

import json
import sys

from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".scratch-deposit"))
import pftl  # noqa: E402


def main() -> int:
    out, height, accounts = sys.argv[1], int(sys.argv[2]), sys.argv[3:]
    rows, keys = {}, set()
    for peer in pftl.PEERS:
        b = pftl.rpc("blocks", {"from_height": height, "limit": 1}, "cb", peer)[0]
        h = b["header"]
        receipts = [pftl.rpc("tx", {"tx_id": rid}, "cbt", peer).get("receipt") for rid in b["receipt_ids"]]
        si = pftl.rpc("server_info", {}, "cbs", peer)["ledger"]
        accts = {}
        for a in accounts:
            acc = pftl.rpc("account", {"address": a}, "cba", peer)
            lines = pftl.rpc("account_lines", {"account": a, "limit": 60}, "cbl", peer).get("lines") or []
            accts[a] = {"balance": acc.get("balance"), "sequence": acc.get("sequence"),
                        "lines": sorted((l["code"], l["asset_id"][:8], l["balance"]) for l in lines
                                        if l["code"] in ("B23", "PFUSDC"))}
        rows[peer[0]] = {"proposer": h["proposer"], "batch_kind": h["batch_kind"], "state_root": h["state_root"],
                         "block_hash": h["block_hash"], "receipts": receipts, "tip": si, "accounts": accts}
        keys.add(json.dumps([h["state_root"], h["block_hash"], receipts, accts], sort_keys=True))
        print(peer[0], "proposer", h["proposer"], "root", h["state_root"][:16], "tip", si["height"],
              "receipts", [(r.get("accepted"), r.get("code"), r.get("message")[:70]) for r in receipts])
    for a in accounts:
        print("  ", a[:12], json.dumps(rows["validator-1"]["accounts"][a]))
    json.dump(rows, open(out, "w"), indent=1)
    print("AGREE" if len(keys) == 1 else "DISAGREE")
    return 0 if len(keys) == 1 else 2


if __name__ == "__main__":
    sys.exit(main())
