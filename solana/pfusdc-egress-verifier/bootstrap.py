"""Encode the one-time bootstrap from trusted route and checkpoint configuration."""
import argparse
import hashlib
import json
from pathlib import Path


def raw(value, size):
    data = bytes.fromhex(value.removeprefix("0x"))
    if len(data) != size:
        raise ValueError(f"expected {size} bytes")
    return data


def domain_hash(name, payload):
    return hashlib.sha3_384(name.encode() + b"\0" + payload).digest()


def packet_alias(key):
    return domain_hash("postfiat.solana.packet_alias.v1", key.hex().encode())[:20]


def encode(config):
    vault = raw(config["vault_program"], 32)
    mint = bytes.fromhex("c6fa7af3bedbad3a3d65f36aabc97431b1bbe4c2d2f6e0e47ca60203452f5d61")
    genesis = raw(config["solana_genesis"], 32)
    policy = raw(config["ingress_profile_hash"], 48)
    asset = raw(config["asset_id"], 48)
    chain = hashlib.sha256(config["chain_id"].encode() + raw(config["genesis_hash"], 48)
                           + config["protocol_version"].to_bytes(4, "big")).digest()
    # The route binds the custody, not one ingress revision's bucket (see src/values.rs).
    route = hashlib.sha256(raw(config["exit_profile_hash"], 48) + config["exit_epoch"].to_bytes(8, "big")
                           + asset + b"SOLANA01" + packet_alias(vault)
                           + raw(config["vault_code_hash"], 32) + packet_alias(mint)
                           + raw(config["token_code_hash"], 32)).digest()
    return b"".join([vault, policy, config["ingress_epoch"].to_bytes(4, "little"), chain, route, genesis, raw(config["pause_authority"], 32),
                     raw(config["initial_checkpoint"], 48), raw(config["initial_committee"], 48),
                     config["initial_height"].to_bytes(8, "little")])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    parser.add_argument("output", type=Path, help="raw Borsh Bootstrap arguments for instruction tag 3")
    args = parser.parse_args()
    args.output.write_bytes(encode(json.loads(args.config.read_text())))
