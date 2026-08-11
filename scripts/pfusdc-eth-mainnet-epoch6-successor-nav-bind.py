#!/usr/bin/env python3
"""Build the ordered h794 NAV profile register and pfUSDC rebind request."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PROFILE_FILE = (
    ROOT
    / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6-successor/package/"
    "planned-nav-profile.mainnet-epoch6.json"
)
OUTPUT = (
    ROOT
    / "docs/evidence/a666-egress-lane-redeploy-20260809/epoch6-successor/pftl/"
    "h794-nav-bind.request.json"
)
ISSUER = "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8"
PFUSDC_ASSET = (
    "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c2"
    "33f6830bd5221fe2717fb6a1a7005d7b"
)
ISSUER_KEY = "/var/lib/postfiat/validator-2/fast-ingress-live/pfusdc-issuer-key.json"
EXPECTED_PROFILE_ID = (
    "24a1308da141d846bf2ac4f8d0900e2a5760817b2084153e16a7ffbefb8ef459"
    "bdf20e4bd54393ad239ac7549b18b490"
)


def profile_id(profile: dict[str, Any]) -> str:
    preimage = (
        f"verifier_kind={profile['verifier_kind']}\n"
        f"source_class={profile['source_class']}\n"
        f"max_snapshot_age_blocks={profile['max_snapshot_age_blocks']}\n"
        f"challenge_window_blocks={profile['challenge_window_blocks']}\n"
        f"max_epoch_gap_blocks={profile['max_epoch_gap_blocks']}\n"
        f"settle_deadline_blocks={profile['settle_deadline_blocks']}\n"
        f"min_challenge_bond={profile['min_challenge_bond']}\n"
        f"min_attestations={profile['min_attestations']}\n"
        f"tolerance_bp={profile['tolerance_bp']}\n"
        f"valuation_policy_hash={profile['valuation_policy_hash']}\n"
        f"sp1_program_vkey={profile['sp1_program_vkey']}\n"
        f"sp1_proof_encoding={profile['sp1_proof_encoding']}\n"
        f"max_proof_bytes={profile['max_proof_bytes']}\n"
        f"max_public_values_bytes={profile['max_public_values_bytes']}\n"
    )
    if profile.get("bridge_observer_min_confirmations", 0):
        preimage += (
            "bridge_observer_min_confirmations="
            f"{profile['bridge_observer_min_confirmations']}\n"
        )
    if profile.get("vault_bridge_route_policy_hash", ""):
        preimage += (
            "vault_bridge_route_policy_hash="
            f"{profile['vault_bridge_route_policy_hash']}\n"
        )
    return hashlib.sha3_384(
        b"postfiat.nav_proof_profile_id.v1\0" + preimage.encode()
    ).hexdigest()


def main() -> int:
    document = json.loads(PROFILE_FILE.read_text(encoding="utf-8"))
    profile = document["nav_proof_profile"]
    derived_profile_id = profile_id(profile)
    if derived_profile_id != EXPECTED_PROFILE_ID:
        raise SystemExit("epoch-6 successor NAV profile ID drifted")
    operation = {key: value for key, value in profile.items() if key != "schema"}
    request = {
        "schema": "postfiat-certified-asset-ops-request-v1",
        "operations": [
            {
                "label": "h794-epoch6-successor-nav-profile-register",
                "source": ISSUER,
                "key_file": ISSUER_KEY,
                "dependencies": [],
                "operation": {
                    "operation": "nav_profile_register",
                    "registrant": ISSUER,
                    **operation,
                },
            },
            {
                "label": "h794-epoch6-successor-nav-asset-bind",
                "source": ISSUER,
                "key_file": ISSUER_KEY,
                "dependencies": [
                    {
                        "label": "h794-epoch6-successor-nav-profile-register",
                        "mode": "same_round",
                        "reason": (
                            "pfUSDC binds the distinct epoch-6 successor NAV profile "
                            "registered earlier in the ordered height-794 asset batch"
                        ),
                    }
                ],
                "operation": {
                    "operation": "nav_asset_register",
                    "issuer": ISSUER,
                    "asset_id": PFUSDC_ASSET,
                    "reserve_operator": ISSUER,
                    "proof_profile": derived_profile_id,
                    "valuation_unit": "USDC",
                    "redemption_account": ISSUER,
                },
            },
        ],
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(request, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(OUTPUT.relative_to(ROOT)), "profile_id": derived_profile_id}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
