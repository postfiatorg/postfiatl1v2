"""Governed route binding written into a Solana vault at initialization."""

import hashlib

ROUTES = {f"solana-{cluster}-usdc-v1" for cluster in ("mainnet", "devnet", "local")}
PFTL_CHAIN = "postfiat-wan-devnet-2"
PFTL_GENESIS = "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9"


PUBLIC_ROUTES = {
    # Solana mainnet-beta and devnet genesis hashes, decoded from Agave RPC base58.
    "45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0": "solana-mainnet-usdc-v1",
    "ce59db5080fc2c6d3bcf7ca90712d3c2e5e6c28f27f0dfbb9953bdb0894c03ab": "solana-devnet-usdc-v1",
}


def binding(cfg):
    route = cfg["route_id"]
    if route not in ROUTES or not cfg["chain_id"]:
        raise ValueError("invalid Solana route or Post Fiat chain ID")
    genesis, solana, vault = (
        bytes.fromhex(cfg[key])
        for key in ("genesis_hash", "solana_genesis_hash", "vault_program")
    )
    if (len(genesis), len(solana), len(vault)) != (48, 32, 32):
        raise ValueError("invalid genesis hash or vault program length")
    expected = PUBLIC_ROUTES.get(solana.hex(), "solana-local-usdc-v1")
    if expected == "solana-local-usdc-v1" and cfg.get("private_devnet_genesis") == solana.hex():
        expected = "solana-devnet-usdc-v1"
    if expected != route:
        raise ValueError("Solana genesis differs from route ID")
    return hashlib.sha256(
        b"postfiat.pfusdc.solana.route_binding.v1"
        + cfg["chain_id"].encode()
        + genesis
        + route.encode()
        + solana
        + vault
    ).hexdigest()
