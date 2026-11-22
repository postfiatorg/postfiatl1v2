#!/usr/bin/env python3
"""Compare public authority and registry records. Never read a key file."""
import argparse
import hashlib
import json
from pathlib import Path

ROUTE = 'pftl-b24-ethereum-wB24-usdc-v1'


def diagnose(snapshot, registry):
    route = next(r for r in snapshot['ledger']['pftl_uniswap_routes'] if r['route_id'] == ROUTE)
    policy = route['ethereum_verification_policy']
    committee = next(c for c in snapshot['ledger']['fastswap_committees']
                     if c['domain']['committee_epoch'] == policy['authority_epoch']
                     and c['domain']['committee_root'] == policy['committee_root'])
    rows = []
    for authority in committee['validators']:
        current = next(v for v in registry['validators'] if v['node_id'] == authority['validator_id'])
        governed = bytes(authority['public_key'])
        registered = bytes.fromhex(current['public_key_hex'])
        rows.append(dict(validator=authority['validator_id'], matches=governed == registered,
                         governed_public_key_hex=governed.hex(), registered_public_key_hex=registered.hex(),
                         governed_public_key_sha256=hashlib.sha256(governed).hexdigest(),
                         registered_public_key_sha256=hashlib.sha256(registered).hexdigest()))
    return dict(native_height=snapshot['tip']['height'], route=ROUTE,
                authority_epoch=policy['authority_epoch'], committee_root=bytes(policy['committee_root']).hex(),
                quorum=committee['domain']['quorum'], public_comparisons=rows,
                matching_registry_entries=sum(r['matches'] for r in rows),
                private_key_files_read=False, repair_executed=False,
                fresh_signature_availability_tested=False)


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--snapshot', type=Path, required=True)
    ap.add_argument('--registry', type=Path, required=True)
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    result = diagnose(json.loads(args.snapshot.read_text()), json.loads(args.registry.read_text()))
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: v for k, v in result.items() if k != 'public_comparisons'}))
