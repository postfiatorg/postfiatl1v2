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


def migrate(config):
    """Arguments of instruction tag 9: the route preimage of a light initialised by the first
    release, whose route also bound the ingress revision's bucket (field 26)."""
    vault = raw(config["vault_program"], 32)
    mint = bytes.fromhex("c6fa7af3bedbad3a3d65f36aabc97431b1bbe4c2d2f6e0e47ca60203452f5d61")
    genesis = raw(config["solana_genesis"], 32)
    source = f"spl:{genesis.hex()}:{vault.hex()}:{mint.hex()}"
    policy = raw(config["ingress_profile_hash"], 48)
    asset = raw(config["asset_id"], 48)
    preimage = f"asset_id={asset.hex()}\nsource_domain_bytes={len(source)}\nsource_domain={source}\npolicy_hash={policy.hex()}\n"
    bucket = domain_hash("postfiat.vault_bridge_bucket_id.v1", preimage.encode())
    return b"".join([raw(config["exit_profile_hash"], 48), config["exit_epoch"].to_bytes(8, "big"), asset, bucket,
                     b"SOLANA01", raw(config["vault_code_hash"], 32), raw(config["token_code_hash"], 32)])

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    parser.add_argument("output", type=Path, help="raw Borsh arguments for instruction tag 3 (or 9)")
    parser.add_argument("--migrate", action="store_true", help="encode the tag-9 route migration")
    args = parser.parse_args()
    config = json.loads(args.config.read_text())
    args.output.write_bytes(migrate(config) if args.migrate else encode(config))
