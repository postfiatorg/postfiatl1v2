#!/usr/bin/env python3
"""Verify the completed Cobalt adversarial campaign packet."""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path


PACKET = Path(__file__).resolve().parent
REPO = PACKET.parents[2]
sys.path.insert(0, str(REPO / "python"))

from postfiat_rpc import cobalt  # noqa: E402


manifest_sha256 = hashlib.sha256((PACKET / "SHA256SUMS.txt").read_bytes()).hexdigest()
result = cobalt.adversarial_result(
    PACKET,
    expected_manifest_sha256=manifest_sha256,
)
assert result["ok"] is True
assert result["status"] == "KEEP_ACTIVE"
assert result["campaign_complete"] is True
assert result["block_finality"] == "consensus-v2"
assert result["authority"]["controls_block_consensus"] is False
assert len(result["experiments"]) == 6
assert len(result["rejected_cases"]) == 9
assert all(row["ok"] is True for row in result["checks"])

print("adversarial-packet-ok")
print(f"sha256sums_sha256={manifest_sha256}")
