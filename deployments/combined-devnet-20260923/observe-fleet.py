#!/usr/bin/env python3
"""DEPLOY DAY ONLY: read fleet RPC and systemd identity, enforce convergence.

Runs the reviewed combined-devnet-20260921 observer unchanged against this
packet (desired identity from manifest-input.unsigned.json; previous identity
from observed/previous-manifest-identity.json). Not executed during preparation.
"""
from pathlib import Path

PACKET = Path(__file__).resolve().parent
BASE = PACKET.parent / "combined-devnet-20260921" / Path(__file__).name

namespace = {"__name__": "reused_observe_fleet", "__file__": str(PACKET / BASE.name)}
exec(compile(BASE.read_text(), str(BASE), "exec"), namespace)
namespace["__doc__"] = __doc__

if __name__ == "__main__":
    namespace["main"]()
