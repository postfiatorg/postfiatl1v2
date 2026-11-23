"""Read-only self-test and separate proof rehearsal for the installed user unit."""

import os
from pathlib import Path
import sys
from tempfile import TemporaryDirectory


def check_tunnels():
    from live import CHAIN, PFTL_GENESIS, PEERS, rpc, six

    for peer, status in zip(PEERS, six(lambda i: rpc(i, "status")), strict=True):
        if (status["chain_id"], status["genesis_hash"]) != (
            CHAIN.decode(),
            PFTL_GENESIS.hex(),
        ):
            raise ValueError(f"Unexpected validator network on port {peer[1]}")
        print(
            f"Validator tunnel {peer[1]} verified at height {status['block_height']}",
            flush=True,
        )


def self_test():
    import exit_service
    from exit_setup import ExitCluster, FEE_PAYER
    from exit_proof import available

    # Check the original home and the replacement HOME. Require ENOENT: an
    # empty replacement, permission error, or readable parent cannot hide a leak.
    for home in {Path(os.environ["EXIT_OPERATOR_HOME"]), Path.home()}:
        for name in (
            ".config/postfiat/sol-bridge",
            ".postfiat",
            ".pft",
            ".ssh",
            ".config/tasknode",
        ):
            path = home / name
            try:
                path.lstat()
            except FileNotFoundError:
                print(f"absent {path}", flush=True)
            else:
                raise RuntimeError(f"Unexpected sandbox path {path}")
    credentials = Path("/credentials")
    if {p.name for p in credentials.iterdir()} != {"fee.json", "prover", "known_hosts"}:
        raise RuntimeError("Unexpected credential mounts")
    for path in (*credentials.iterdir(), Path("/app"), Path("/")):
        if not os.statvfs(path).f_flag & os.ST_RDONLY:
            raise RuntimeError(f"Expected read-only mount {path}")
    if not os.access(credentials / "fee.json", os.R_OK):
        raise RuntimeError("Fee signer is unavailable")
    with TemporaryDirectory(dir="/state"):
        pass
    print(
        f"{exit_service.__name__} imported; credential mounts read-only; state writable",
        flush=True,
    )
    check_tunnels()
    c = ExitCluster(FEE_PAYER)
    print(
        f"Solana mainnet identity verified at finalized slot {c.call('getSlot', c.level)}",
        flush=True,
    )
    available()
    print("Prover SSH and remote proof files verified", flush=True)


def proof():
    from exit_proof import prove
    from live import read

    with TemporaryDirectory() as directory:
        values, result = prove(
            "checkpoint", read(os.environ["EXIT_ANCHOR_WITNESS"]), Path(directory)
        )
        if not values or len(result) != 356:
            raise RuntimeError("Invalid checkpoint proof")
    print("service-proof verified")


if __name__ == "__main__":
    {"self-test": self_test, "proof": proof}[sys.argv[1]]()
