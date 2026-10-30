#!/usr/bin/env python3
"""Local, read-only: the three governed activation heights and both routes' authority epoch in a
signed canary import (vote-records-20261030 sheet §3/§4), and the two W-1 activation kinds, which must be
absent on the devnet. Same rules as
GovernanceState: fee claim and require-memo take the latest amendment; recipient format takes
the earliest unpaused one. No value means inactive."""
import json, sys
from pathlib import Path

def first_json(path):
    return json.JSONDecoder().raw_decode(Path(path).read_text())[0]

data = Path(sys.argv[1])
amendments = first_json(data / "governance.json")["amendments"]
def latest(kind):
    values = [a["value"] for a in amendments if a["kind"] == kind]
    return values[-1] if values else None
recipient = [a["value"] for a in amendments if a["kind"] == "recipient_format_activation_height" and not a.get("paused")]
routes = {r["route_id"]: r.get("ethereum_verification_policy", {}).get("authority_epoch")
          for r in first_json(data / "ledger.json")["pftl_uniswap_routes"]
          if r["route_id"] in ("pftl-a666-ethereum-wA666-usdc-v1", "pftl-bmnrc-ethereum-wBMNRC-usdc-v1")}
out = {
    "recipient_format_activation_height": min(recipient) if recipient else None,
    "rejected_tx_fee_activation_height": latest("rejected_tx_fee_activation_height"),
    "require_memo_activation_height": latest("require_memo_activation_height"),
    "route_authority_epoch": routes,
    "asset_orchard_pricing_enforcement_activation_height": latest("asset_orchard_pricing_enforcement_activation_height"),
    "asset_orchard_egress_policy_activation_height": latest("asset_orchard_egress_policy_activation_height"),
}
out["result"] = "PASS" if all(out[k] is None for k in list(out)[:3]) and set(routes.values()) == {1} and len(routes) == 2 else "ATTENTION"
out["w1_inert"] = out["asset_orchard_pricing_enforcement_activation_height"] is None and out["asset_orchard_egress_policy_activation_height"] is None
print(json.dumps(out, indent=2))
