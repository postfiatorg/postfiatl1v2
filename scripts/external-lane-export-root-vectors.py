#!/usr/bin/env python3
"""Compute crates/types/testdata/external-lane-export-root-vectors.json
independently of the Rust code: the export-root leaf of a lane export packet
and the root of a block with three exports (two of one lane, one of another).

Usage: scripts/external-lane-export-root-vectors.py > crates/types/testdata/external-lane-export-root-vectors.json
"""
import hashlib
import json

PACKET_DOMAIN = 'postfiat.external_lane.export_packet.v1'
LEAF_DOMAIN = 'postfiat.external_lane.export_root_leaf.v1'
NODE_DOMAIN = 'postfiat.bridge_exit_tree.node.v1'
EMPTY_DOMAIN = 'postfiat.bridge_exit_tree.empty.v1'
CHAIN_ID = 'postfiat-local'


def sha3(domain, *parts):
    hasher = hashlib.sha3_384(domain.encode())
    for part in parts:
        hasher.update(b'\x00' + part)
    return hasher.digest()


def node(left, right):
    return hashlib.sha3_384(NODE_DOMAIN.encode() + b'\x00' + left + right).digest()


def leaf(packet):
    digest = sha3(PACKET_DOMAIN, json.dumps(packet, separators=(',', ':')).encode())
    return sha3(LEAF_DOMAIN, CHAIN_ID.encode(), packet['lane_id'].encode(),
                packet['export_nonce'].to_bytes(8, 'big'), digest)


def packet(lane, nonce, recipient_byte, amount, tx_byte):
    return {'lane_id': lane, 'export_nonce': nonce, 'sender': 'pftl-sender',
            'base_recipient': '0x' + recipient_byte * 20, 'amount_atoms': amount,
            'exported_at_height': 42, 'tx_id': tx_byte * 48}


def main():
    packets = [packet('base-erc20', 0, '2c', 1000000, '0a'), packet('base-other', 0, '3d', 5, '0b'),
               packet('base-erc20', 1, '2c', 7, '0c')]
    leaves = [leaf(p) for p in packets]
    left, right = node(leaves[0], leaves[1]), node(leaves[2], leaves[2])
    root = node(left, right)
    out = {
        'schema': 'postfiat-external-lane-export-root-vectors-v1',
        'description': (
            'leaf = SHA3-384(leaf domain || 0x00 || chain id || 0x00 || lane id || 0x00 || export nonce u64 '
            'big-endian || 0x00 || packet digest). The root is the bridge-exit tree over the leaves in execution '
            'order: node = SHA3-384(node domain || 0x00 || left || right), an odd node paired with itself, and '
            'SHA3-384(empty domain || 0x00) for a block without exports. Values are 96 lower-case hex digits; '
            'siblings run from the leaf up.'),
        'domains': {'leaf': LEAF_DOMAIN, 'node': NODE_DOMAIN, 'empty': EMPTY_DOMAIN},
        'chain_id': CHAIN_ID,
        'packets': packets,
        'leaves': [value.hex() for value in leaves],
        'root': root.hex(),
        'proofs': [
            {'leaf_index': 0, 'siblings': [leaves[1].hex(), right.hex()]},
            {'leaf_index': 1, 'siblings': [leaves[0].hex(), right.hex()]},
            {'leaf_index': 2, 'siblings': [leaves[2].hex(), left.hex()]},
        ],
        'empty_root': hashlib.sha3_384(EMPTY_DOMAIN.encode() + b'\x00').hexdigest(),
    }
    print(json.dumps(out, indent=2))


if __name__ == '__main__':
    main()
