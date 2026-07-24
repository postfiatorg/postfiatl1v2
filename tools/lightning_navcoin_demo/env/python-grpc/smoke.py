"""Direct authenticated gRPC smoke check for all synthetic LND nodes."""

from __future__ import annotations

import json
import os

from pftl_lnd_grpc import connect_lnd


def main() -> None:
    state_dir = os.environ.get("PFTL_LN_STATE_DIR", "/state")
    result: dict[str, object] = {
        "schema": "postfiat.lightning_regtest.grpc_smoke.v1",
        "network": "regtest",
        "nodes": {},
    }
    for node in ("user", "coordinator", "router"):
        channel, lightning, _router, lightning_pb2, _router_pb2 = connect_lnd(
            node, state_dir=state_dir
        )
        try:
            info = lightning.GetInfo(lightning_pb2.GetInfoRequest(), timeout=10)
            chains = [
                {"chain": chain.chain, "network": chain.network}
                for chain in info.chains
            ]
            if chains != [{"chain": "bitcoin", "network": "regtest"}]:
                raise RuntimeError(f"LND network mismatch for {node}: {chains}")
            result["nodes"][node] = {
                "identity_pubkey": info.identity_pubkey,
                "block_height": info.block_height,
                "synced_to_chain": info.synced_to_chain,
                "chains": chains,
            }
        finally:
            channel.close()
    print(json.dumps(result, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
