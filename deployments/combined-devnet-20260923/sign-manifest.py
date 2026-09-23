#!/usr/bin/env python3
"""KEY-HOLDER ONLY: sign the reviewed combined-devnet-20260923 inputs.

Runs the reviewed combined-devnet-20260921 signer unchanged against this
packet's manifest-input.unsigned.json and local-preflight.py. Not run in preparation.
"""
from pathlib import Path

if __name__ == "__main__":
    PACKET = Path(__file__).resolve().parent
    BASE = PACKET.parent / "combined-devnet-20260921" / Path(__file__).name
    namespace = {"__name__": "__main__", "__file__": str(PACKET / BASE.name), "__doc__": __doc__}
    exec(compile(BASE.read_text(), str(BASE), "exec"), namespace)
