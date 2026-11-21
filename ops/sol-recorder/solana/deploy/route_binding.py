"""Compute the Post Fiat Solana vault binding from public deployment inputs."""
import argparse
from hashlib import sha256

from solders.hash import Hash
from solders.pubkey import Pubkey


DOMAIN = b"postfiat.pfusdc.solana.route_binding.v1"
CHAIN = b"postfiat-wan-devnet-2"
PFTL_GENESIS = bytes.fromhex(
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd4"
    "9a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9"
)
PUBLIC_ROUTES = {
    "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d": b"solana-mainnet-usdc-v1",
    "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG": b"solana-devnet-usdc-v1",
}


def route_binding(genesis, vault, *, local=False, test_route="local"):
    route = PUBLIC_ROUTES.get(genesis)
    if local and route is None:
        if test_route not in ("local", "devnet"):
            raise ValueError("invalid private test cluster route")
        route = f"solana-{test_route}-usdc-v1".encode()
    elif local or route is None:
        raise ValueError("cluster genesis does not match the requested route")
    return sha256(DOMAIN + CHAIN + PFTL_GENESIS + route
                  + bytes(Hash.from_string(genesis)) + bytes(vault)).digest()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("genesis", help="base58 Solana genesis hash")
    parser.add_argument("vault", type=Pubkey.from_string, help="base58 vault program ID")
    parser.add_argument("--local", action="store_true", help="select the local test route")
    args = parser.parse_args()
    print(route_binding(args.genesis, args.vault, local=args.local).hex())
