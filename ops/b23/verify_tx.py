#!/usr/bin/env python3
"""B23 OPS lane: read-only receipt + balance verification on all six validators.

Usage: PYTHONDONTWRITEBYTECODE=1 python3 ops/b23/verify_tx.py OUT.json TX_ID ACCOUNT [ACCOUNT ...]
Prints, per validator: height, state root, receipt accepted/code, and each
account's PFT balance and trust lines. Submits nothing.
"""
from __future__ import annotations

import json
import sys

sys.path.insert(0, "/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit")
import pftl  # noqa: E402


def main() -> int:
    out, tx_id, accounts = sys.argv[1], sys.argv[2], sys.argv[3:]
    rows = {}
    for peer in pftl.PEERS:
        name = peer[0]
        si = pftl.rpc_raw("server_info", {}, "v-si", peer)
        tx = pftl.rpc_raw("tx", {"tx_id": tx_id}, "v-tx", peer)
        accts = {a: {"account": pftl.rpc_raw("account", {"address": a}, "v-a", peer),
                     "lines": pftl.rpc_raw("account_lines", {"account": a, "limit": 60}, "v-l", peer)}
                 for a in accounts}
        rows[name] = {"server_info": si, "tx": tx, "accounts": accts}
        led = (si.get("result") or {}).get("ledger") or {}
        rc = ((tx.get("result") or {}).get("receipt") or {})
        line = f"{name} h={led.get('height')} root={str(led.get('state_root'))[:16]} " \
               f"receipt accepted={rc.get('accepted')} code={rc.get('code')}"
        for a, v in accts.items():
            ar = v["account"].get("result") or {}
            ls = [(l.get("code"), l.get("asset_id", "")[:8], l.get("balance"))
                  for l in (v["lines"].get("result") or {}).get("lines") or []]
            line += f" | {a[:10]} bal={ar.get('balance')} seq={ar.get('sequence')} lines={ls}"
        print(line)
    with open(out, "w") as fh:
        json.dump(rows, fh, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
