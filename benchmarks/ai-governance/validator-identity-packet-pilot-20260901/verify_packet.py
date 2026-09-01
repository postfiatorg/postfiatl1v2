#!/usr/bin/env python3
"""Verify the frozen Corbanu-exec identity-packet pilot."""

from __future__ import annotations

import hashlib
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent
VALIDATOR_ID = "nHU4bLE3EmSqNwfL4AP1UZeTNPrSPPP6FXLKXo2uqfHuvBQxDVKd"
INPUT = ROOT / "inputs" / "validator.json"
PROMPT = ROOT / "prompts" / f"{VALIDATOR_ID}.txt"
PACKET = ROOT / "packets" / "xrpl" / f"{VALIDATOR_ID}.md"
LOG = ROOT / "logs" / "xrpl" / f"{VALIDATOR_ID}.jsonl"
STDERR_LOG = ROOT / "logs" / "xrpl" / f"{VALIDATOR_ID}.stderr.log"
MANIFEST = ROOT / "manifest.json"

HEADINGS = [
    "# Validator Identity Packet",
    "## Packet Status",
    "## Validator Coordinates",
    "## Claimed Domain and Official URLs",
    "## Public Identity",
    "## Public X Handle",
    "## Region of Incorporation and Operations",
    "## Activities",
    "## Estimated Public-Profile Size",
    "## Evidence",
    "## Uncertainty and Conflicts",
    "## Machine-Readable Summary",
]
SUMMARY_KEYS = {
    "validator_id",
    "network",
    "claimed_domain",
    "domain_verification_status",
    "canonical_entity",
    "entity_type",
    "aliases",
    "official_urls",
    "x_handle",
    "incorporation_region",
    "operating_regions",
    "profile_size_tier",
    "profile_size_confidence",
    "identity_confidence",
    "unresolved_fields",
    "evidence_urls",
}
PROFILE_TIERS = {
    "Unknown",
    "Individual",
    "Micro",
    "Small",
    "Medium",
    "Large",
    "Very large",
}


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    manifest = json.loads(MANIFEST.read_text())
    paths = {
        "input_sha256": INPUT,
        "prompt_sha256": PROMPT,
        "packet_sha256": PACKET,
        "exec_log_sha256": LOG,
        "stderr_log_sha256": STDERR_LOG,
    }
    for field, path in paths.items():
        assert sha256(path) == manifest["hashes"][field], f"{field} mismatch"

    packet = PACKET.read_text()
    found_headings = re.findall(r"^#{1,2} .+$", packet, flags=re.MULTILINE)
    assert found_headings == HEADINGS, "packet heading contract mismatch"
    assert "**SHADOW_ONLY**" in packet
    assert VALIDATOR_ID in packet
    assert "not independently established" in packet
    assert "legitimacy score" not in packet.lower()

    match = re.search(
        r"^## Machine-Readable Summary\n\n\x60\x60\x60json\n(.*?)\n\x60\x60\x60\s*$",
        packet,
        flags=re.MULTILINE | re.DOTALL,
    )
    assert match, "missing terminal machine-readable summary"
    summary = json.loads(match.group(1))
    assert set(summary) == SUMMARY_KEYS
    source = json.loads(INPUT.read_text())
    assert summary["validator_id"] == source["validator_id"] == VALIDATOR_ID
    assert summary["claimed_domain"] == source["domain"]
    assert summary["domain_verification_status"] is source["domain_verified"] is None
    assert summary["profile_size_tier"] in PROFILE_TIERS
    assert summary["evidence_urls"]

    rows = [json.loads(line) for line in LOG.read_text().splitlines()]
    assert rows and all(isinstance(row, dict) for row in rows)
    thread_rows = [row for row in rows if row.get("type") == "thread.started"]
    completion_rows = [row for row in rows if row.get("type") == "turn.completed"]
    messages = [
        row["item"]["text"]
        for row in rows
        if row.get("type") == "item.completed"
        and row.get("item", {}).get("type") == "agent_message"
    ]
    assert len(thread_rows) == len(completion_rows) == len(messages) == 1
    assert thread_rows[0]["thread_id"] == manifest["exec"]["thread_id"]
    assert messages[0].strip() == packet.strip(), "logged final answer differs from packet"
    assert STDERR_LOG.read_bytes() == b"", "Corbanu exec wrote to stderr"

    print(
        json.dumps(
            {
                "verdict": "PASS",
                "validator_id": VALIDATOR_ID,
                "packet_sha256": sha256(PACKET),
                "exec_log_sha256": sha256(LOG),
                "evidence_url_count": len(summary["evidence_urls"]),
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )


if __name__ == "__main__":
    main()
