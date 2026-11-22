#!/usr/bin/env python3
"""B23 OPS lane: read-only devnet-2 state check.

Reads only. Submits nothing, signs nothing, opens no key file. Reuses the RPC
transport in the MINT lane's .scratch-deposit/pftl.py (imported, never edited).

Usage: PYTHONDONTWRITEBYTECODE=1 python3 ops/b23/state_check.py OUTDIR
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, "/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit")
import pftl  # noqa: E402

ISSUER = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b"
HOLDER = "pfab9b9228942e5c529633a13aa271d5297bec6353"
ROUTE_ID = "pftl-pfdep-schwab-primary-pfusdc-v1"
PFUSDC_FAMILY = ("02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c2"
                 "33f6830bd5221fe2717fb6a1a7005d7b")


def call(method: str, params: dict, label: str) -> dict:
    """Return the full envelope so refusals are recorded, not raised."""
    try:
        return pftl.rpc_raw(method, params, f"ops-{label}")
    except Exception as exc:  # transport failure is data here
        return {"ok": False, "transport_error": str(exc)}


def main() -> int:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    reads = {
        "issuer_assets": ("issuer_assets", {"issuer": ISSUER, "limit": 200}),
        "routes": ("navcoin_bridge_routes", {}),
        "supply_status": ("navcoin_bridge_supply_status", {"route_id": ROUTE_ID}),
        "issuer_lines": ("account_lines", {"account": ISSUER, "limit": 100}),
        "holder_lines": ("account_lines", {"account": HOLDER, "limit": 100}),
        "issuer_account": ("account", {"address": ISSUER}),
        "holder_account": ("account", {"address": HOLDER}),
        "pfusdc_vault_bridge_status": ("vault_bridge_status", {"asset_id": PFUSDC_FAMILY}),
        "pfusdc_vault_bridge_route": ("vault_bridge_route", {"asset_id": PFUSDC_FAMILY}),
        "pfusdc_asset_info": ("asset_info", {"asset_id": PFUSDC_FAMILY}),
    }
    results = {name: call(m, p, name) for name, (m, p) in reads.items()}
    assets = (results["issuer_assets"].get("result") or {}).get("assets") or []
    b23 = [a for a in assets if a.get("code") == "B23"]
    results["b23_candidates"] = b23
    for i, a in enumerate(b23):
        aid = a["asset_id"]
        results[f"b23_{i}_nav_status"] = call("nav_reserve_proof_status", {"asset_id": aid}, "nav")
        results[f"b23_{i}_asset_info"] = call("asset_info", {"asset_id": aid}, "ai")
    (out / "04-b23-state.json").write_text(json.dumps(results, indent=1, sort_keys=True) + "\n")
    for k, v in results.items():
        ok = v.get("ok") if isinstance(v, dict) else None
        print(f"{k:32} ok={ok}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
