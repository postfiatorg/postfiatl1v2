#!/usr/bin/env python3
"""Regenerate the canonical combined-devnet-20260923 stage locally; never contacts validators.

Runs the reviewed combined-devnet-20260921 logic unchanged, with this packet's
release ID and qualified executable hash from manifest-input.unsigned.json.
"""
import json
from pathlib import Path

PACKET = Path(__file__).resolve().parent
BASE = PACKET.parent / "combined-devnet-20260921" / Path(__file__).name
INPUTS = json.loads((PACKET / "manifest-input.unsigned.json").read_text())

namespace = {"__name__": "reused_prepare_stage", "__file__": str(PACKET / BASE.name)}
exec(compile(BASE.read_text(), str(BASE), "exec"), namespace)
namespace.update(RELEASE=INPUTS["deployment_id"], BINARY_SHA256=INPUTS["binary_sha256"])

if __name__ == "__main__":
    namespace["main"]()
