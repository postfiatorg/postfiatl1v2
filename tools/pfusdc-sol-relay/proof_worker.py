#!/usr/bin/env python3
"""Run one SP1 host inside the remote GPU lock, including the lifetime of its CUDA server."""

import json
import os
import signal
import subprocess
import sys
from pathlib import Path


def main():
    directory = Path(sys.argv[1])
    kind = sys.argv[2]
    # The blocking SDK may spawn a CUDA server and exit with its descriptors still open.
    # Only this request and its descendants join the new session.
    with (directory / "prover.log").open("w") as log:
        process = subprocess.Popen(
            [
                directory / "host",
                "prove",
                directory / "guest",
                directory / "witness",
                "groth16",
                directory,
                kind,
            ],
            start_new_session=True,
            stdout=log,
            stderr=log,
        )
        try:
            result = process.wait()
        finally:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    tail = (directory / "prover.log").read_bytes()[-1600:].decode(errors="replace")
    print(tail, flush=True)
    if result:
        sys.exit(result)
    print(
        json.dumps(
            {
                "backend": os.environ["SP1_PROVER"],
                "proof_bytes": (directory / "proof.bin").stat().st_size,
            }
        )
    )


if __name__ == "__main__":
    main()
