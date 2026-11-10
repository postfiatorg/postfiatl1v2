#!/usr/bin/env python3
"""Compute crates/types/testdata/external-lane-route-profile-vectors.json
independently of the Rust code: the PFT asset id and route profile hash of an
external lane, from its genesis policy (design note, "Base half, step 1").

Usage: scripts/external-lane-route-profile-vectors.py > crates/types/testdata/external-lane-route-profile-vectors.json
"""
import hashlib
import json
from pathlib import Path

from Crypto.Hash import keccak

ROOT = Path(__file__).resolve().parents[1]
REHEARSAL = ROOT / 'docs/review/base-sepolia-lock-rehearsal-20261106/genesis.json'
ASSET_ID_DOMAIN = 'postfiat.external_lane.asset_id.v1'
PROFILE_HASH_DOMAIN = 'postfiat.external_lane.route_profile_hash.v1'
PROFILE_SCHEMA = 'postfiat.external_lane.route_profile.v1'
CODE, VERSION, ROUTE_EPOCH = 'PFT', 1, 1


def sha3_384(domain, text):
    return hashlib.sha3_384(domain.encode() + b'\x00' + text.encode()).hexdigest()


def keccak256(data):
    return '0x' + keccak.new(digest_bits=256, data=data).hexdigest()


def vector(name, pftl_chain_id, lane):
    policy = lane['policy']
    asset_id = sha3_384(ASSET_ID_DOMAIN, (
        f"chain_id={pftl_chain_id}\nlane_id={lane['id']}\nsource_chain_id={policy['chain_id']}\n"
        f"lock_contract_address={policy['lock_contract_address']}\ntoken_address={policy['token_address']}\n"
        f"code_bytes={len(CODE)}\ncode={CODE}\nversion={VERSION}\n"))
    preimage = (
        f"schema={PROFILE_SCHEMA}\nroute_id={lane['id']}\nasset_id={asset_id}\n"
        f"source_chain_id={policy['chain_id']}\nvault_address={policy['lock_contract_address']}\n"
        f"vault_runtime_code_hash={policy['lock_runtime_code_hash']}\ntoken_address={policy['token_address']}\n"
        f"token_runtime_code_hash={policy['token_runtime_code_hash']}\nroute_epoch={ROUTE_EPOCH}\n")
    profile_hash = sha3_384(PROFILE_HASH_DOMAIN, preimage)
    return {
        'name': name,
        'pftl_chain_id': pftl_chain_id,
        'lane': lane,
        'asset_id': asset_id,
        'route_profile_preimage': preimage,
        'route_profile_hash': profile_hash,
        'route_epoch': ROUTE_EPOCH,
        'verifier_asset_id_commitment': keccak256(bytes.fromhex(asset_id)),
        'verifier_route_profile_hash_commitment': keccak256(bytes.fromhex(profile_hash)),
    }


def main():
    genesis = json.loads(REHEARSAL.read_text())
    (rehearsal,) = genesis['external_lanes']
    example = {
        'id': 'base-erc20',
        'reserved_atoms': 300000000,
        'release_window_blocks': 86400,
        'release_cap_bps': 200,
        'policy': {
            'chain_id': 8453,
            'lock_contract_address': '0x' + '1a' * 20,
            'token_address': '0x' + '2b' * 20,
            'token_runtime_code_hash': '0x' + '3c' * 32,
            'lock_runtime_code_hash': '0x' + '4d' * 32,
            'committee_epoch': 1,
            'committee_root': '0x' + '5e' * 48,
            'confirmation_depth': 64,
            'exports_enabled': True,
        },
    }
    out = {
        'schema': 'postfiat-external-lane-route-profile-vectors-v1',
        'description': (
            'PFT asset id and route profile hash of an external lane, derived from its genesis policy. '
            'asset_id = SHA3-384(asset_id domain || 0x00 || "chain_id=..\\nlane_id=..\\nsource_chain_id=..\\n'
            'lock_contract_address=..\\ntoken_address=..\\ncode_bytes=3\\ncode=PFT\\nversion=1\\n"). '
            'route_profile_hash = SHA3-384(profile hash domain || 0x00 || route_profile_preimage). '
            'Both are 96 lower-case hex digits; the verifier commitments are Keccak-256 of their 48 bytes.'),
        'domains': {'asset_id': ASSET_ID_DOMAIN, 'route_profile_hash': PROFILE_HASH_DOMAIN,
                    'route_profile_schema': PROFILE_SCHEMA},
        'vectors': [
            vector('base-sepolia-rehearsal-20261106 (docs/review/base-sepolia-lock-rehearsal-20261106/genesis.json)',
                   genesis['chain_id'], rehearsal),
            vector('example: Base mainnet 8453 with synthetic addresses, not a deployment', 'postfiat-mainnet-example',
                   example),
        ],
    }
    print(json.dumps(out, indent=2))


if __name__ == '__main__':
    main()
