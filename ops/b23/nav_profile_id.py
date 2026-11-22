#!/usr/bin/env python3
"""Derive a NAV proof profile id exactly as crates/types market_nav_asset_types.rs
nav_proof_profile_id_with_route_policy (domain postfiat.nav_proof_profile_id.v1).
Usage: python3 nav_profile_id.py OPERATION_JSON (a nav_profile_register operation)"""
import hashlib, json, sys

def profile_id(op: dict) -> str:
    pre = (f"verifier_kind={op['verifier_kind']}\nsource_class={op['source_class']}\n"
           f"max_snapshot_age_blocks={op['max_snapshot_age_blocks']}\nchallenge_window_blocks={op['challenge_window_blocks']}\n"
           f"max_epoch_gap_blocks={op['max_epoch_gap_blocks']}\nsettle_deadline_blocks={op['settle_deadline_blocks']}\n"
           f"min_challenge_bond={op['min_challenge_bond']}\nmin_attestations={op['min_attestations']}\n"
           f"tolerance_bp={op['tolerance_bp']}\nvaluation_policy_hash={op['valuation_policy_hash']}\n"
           f"sp1_program_vkey={op.get('sp1_program_vkey','')}\nsp1_proof_encoding={op.get('sp1_proof_encoding','')}\n"
           f"max_proof_bytes={op.get('max_proof_bytes',0)}\nmax_public_values_bytes={op.get('max_public_values_bytes',0)}\n")
    if int(op.get('bridge_observer_min_confirmations', 0)):
        pre += f"bridge_observer_min_confirmations={op['bridge_observer_min_confirmations']}\n"
    if op.get('vault_bridge_route_policy_hash'):
        pre += f"vault_bridge_route_policy_hash={op['vault_bridge_route_policy_hash']}\n"
    h = hashlib.sha3_384(); h.update(b"postfiat.nav_proof_profile_id.v1"); h.update(b"\x00"); h.update(pre.encode())
    return h.hexdigest()

if __name__ == "__main__":
    print(profile_id(json.load(open(sys.argv[1]))))
