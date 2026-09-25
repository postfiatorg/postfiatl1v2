#!/usr/bin/env python3
"""Offline checks only for combined-devnet-20260923; no fleet preflight or rollout state.

Runs the reviewed combined-devnet-20260921 checks unchanged against this packet:
every identity is read from this directory's manifest-input.unsigned.json.
"""
from pathlib import Path
import tempfile

PACKET = Path(__file__).resolve().parent
BASE = PACKET.parent / "combined-devnet-20260921" / Path(__file__).name

namespace = {"__name__": "reused_local_preflight", "__file__": str(PACKET / BASE.name)}
exec(compile(BASE.read_text(), str(BASE), "exec"), namespace)

if __name__ == "__main__":
    # The production verifier creates temporary local bindings; retain them under ~/.cache.
    scratch = Path.home() / ".cache/deploy-prep-20260923/tmp"
    scratch.mkdir(parents=True, exist_ok=True)
    tempfile.tempdir = str(scratch)
    namespace["main"]()
