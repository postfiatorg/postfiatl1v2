"""Read-only self-test and separate proof rehearsal for the installed user unit."""

import hashlib
import os
import pwd
import socket
import subprocess
from pathlib import Path
import sys
from tempfile import TemporaryDirectory


def check_tunnels():
    from live import CHAIN, PFTL_GENESIS, PEERS, rpc, six, verify_node_ids

    rows = six(lambda i: rpc(i, "status"))
    verify_node_ids(rows)
    for peer, status in zip(PEERS, rows, strict=True):
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


def proof(*, check_anchor=False):
    from exit_proof import LOCAL, prove
    from live import read

    with TemporaryDirectory() as directory:
        values, result = prove(
            "checkpoint", read(os.environ["EXIT_ANCHOR_WITNESS"]), Path(directory)
        )
        if check_anchor:
            from exit_precheck import checkpoint_values
            from exit_setup import CONFIG, require
            cfg = read(CONFIG)
            fields = checkpoint_values(values)
            require((fields["height"], fields["result"], fields["end"]) ==
                    (cfg["initial_height"], cfg["initial_checkpoint"], cfg["initial_committee"]),
                    "Prover did not verify the trusted archive checkpoint.")
            subprocess.run([LOCAL / "bin/exit-verify-proof"], input=result + values,
                           check=True, timeout=30)
        if not values or len(result) != 356:
            raise RuntimeError("Invalid checkpoint proof")
    print("service-proof verified")


def system_self_test():
    import exit_service
    from exit_setup import ExitCluster, FEE_PAYER, load, require, verify_role_record, CONFIG
    from exit_proof import available
    from live import read

    require(pwd.getpwuid(os.getuid()).pw_name == "pf-sol-exit", "Wrong self-test identity.")
    home = Path("/var/lib/pf-sol-exit")
    require(Path.home() == home and os.environ["EXIT_PROVER_SSH"] == "system", "Wrong service environment.")
    require(not os.access("/home/postfiat/.ssh", os.R_OK), "Operator SSH directory is accessible.")
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 18089))
        listener.listen(1)
    print("Loopback port 18089 available", flush=True)
    with TemporaryDirectory(dir=home / "jobs"):
        pass
    require(load(os.environ["EXIT_FEE_KEY"]).pubkey() == FEE_PAYER, "Wrong installed payer.")
    roles = Path("/opt/postfiat/sol-exit/roles.json")
    verify_role_record(read(CONFIG), read(roles))
    check_tunnels()
    c = ExitCluster(FEE_PAYER)
    print(f"{exit_service.__name__} imported; finalized Solana slot {c.call('getSlot', c.level)}", flush=True)
    available()
    proof(check_anchor=True)
    print("service-proof verified", hashlib.sha256(roles.read_bytes()).hexdigest(), flush=True)


if __name__ == "__main__":
    {"self-test": self_test, "proof": proof, "system-self-test": system_self_test}[sys.argv[1]]()
