#!/usr/bin/env python3
"""Compute crates/types/testdata/external-lane-export-packet-vectors.json
independently of the Rust code. Transaction ids have the form execution gives
them (`asset_transaction_tx_id`: SHA3-384, 96 lower-case hex digits).

Usage: scripts/external-lane-export-packet-vectors.py > crates/types/testdata/external-lane-export-packet-vectors.json
"""
import hashlib
import json

DOMAINS = {
    'packet': 'postfiat.external_lane.export_packet.v1',
    'history': 'postfiat.external_lane.export_history.v1',
    'withdrawal_id': 'postfiat.external_lane.export_withdrawal_id.v1',
    'burn_tx': 'postfiat.external_lane.export_burn_tx.v1',
}


def sha3(domain, *parts):
    hasher = hashlib.sha3_384(domain.encode())
    for part in parts:
        hasher.update(b'\x00' + part)
    return hasher.digest()


def hex48(digest):
    return '0x' + digest.hex()


def vector(chain_id, packet):
    digest = sha3(DOMAINS['packet'], json.dumps(packet, separators=(',', ':')).encode())
    return {
        'chain_id': chain_id,
        'packet': packet,
        'packet_digest': hex48(digest),
        'withdrawal_id_commitment': hex48(sha3(
            DOMAINS['withdrawal_id'], chain_id.encode(), packet['lane_id'].encode(),
            packet['export_nonce'].to_bytes(8, 'big'), digest)),
        'burn_tx_commitment': hex48(sha3(
            DOMAINS['burn_tx'], chain_id.encode(), packet['lane_id'].encode(), packet['tx_id'].encode())),
        'export_history_root_from_empty': hex48(sha3(DOMAINS['history'], b'', digest)),
    }


def packet(nonce, recipient, amount, height, tx_byte):
    return {
        'lane_id': 'base-erc20', 'export_nonce': nonce, 'sender': 'pftl-sender',
        'base_recipient': recipient, 'amount_atoms': amount, 'exported_at_height': height,
        'tx_id': tx_byte * 48,
    }


def main():
    out = {
        'schema': 'postfiat-external-lane-export-packet-vectors-v1',
        'description': (
            'external_lane_export_v1 packet digest and commitments. digest = SHA3-384(packet domain || 0x00 || '
            'packet JSON). withdrawal_id = SHA3-384(withdrawal_id domain || 0x00 || chain id || 0x00 || lane id || '
            '0x00 || export nonce u64 big-endian || 0x00 || digest). burn_tx = SHA3-384(burn_tx domain || 0x00 || '
            'chain id || 0x00 || lane id || 0x00 || tx id). history root = SHA3-384(history domain || 0x00 || '
            'previous root text || 0x00 || digest). Strings are UTF-8; digests are 0x and 96 lower-case hex digits. '
            'Transaction ids are 96 lower-case hex digits, the form asset_transaction_tx_id gives.'),
        'domains': DOMAINS,
        'vectors': [
            vector('postfiat-local', packet(0, '0x' + '2c' * 20, 1000000, 42, '0a')),
            vector('postfiat-mainnet', packet(7, '0x' + '00' * 19 + 'a1', 1, 1000, '7e')),
        ],
    }
    print(json.dumps(out, indent=2))


if __name__ == '__main__':
    main()
