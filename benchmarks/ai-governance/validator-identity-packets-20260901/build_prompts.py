#!/usr/bin/env python3
"""Freeze validator coordinates and render one Corbanu-exec prompt per validator."""

from __future__ import annotations

import hashlib
import json
import pathlib
import shutil
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
REPO = ROOT.parents[2]
SOURCE = (
    REPO
    / "benchmarks"
    / "ai-governance"
    / "institution-reputation-unl-20260901"
    / "inputs"
    / "validators.json"
)
INPUTS = ROOT / "inputs"
PROMPTS = ROOT / "prompts"
TEMPLATE = ROOT / "prompt_template.txt"
EXPECTED_SOURCE_SHA256 = (
    "7687dcd9a23638dca4e0fbe50c2dd3782c6db89fa645802cd5dd9586feb87f27"
)
NETWORK_LABELS = {
    "xrpl": "XRP Ledger mainnet",
    "postfiat": "PostFiat testnet current published UNL (completed round 20)",
}


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def render(template: str, values: dict[str, str]) -> str:
    result = template
    for key, value in values.items():
        result = result.replace("{{" + key + "}}", value)
    if "{{" in result or "}}" in result:
        raise ValueError("unresolved prompt placeholder")
    return result


def main() -> None:
    if sha256(SOURCE) != EXPECTED_SOURCE_SHA256:
        raise SystemExit("frozen validator source hash mismatch")
    validators = json.loads(SOURCE.read_text())
    if len(validators) != 55:
        raise SystemExit(f"expected 55 validators, got {len(validators)}")

    INPUTS.mkdir(parents=True, exist_ok=True)
    PROMPTS.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(SOURCE, INPUTS / "validators.json")
    template = TEMPLATE.read_text()

    index = []
    seen = set()
    for row in validators:
        validator_id = row["validator_id"]
        network = row["network"]
        if validator_id in seen:
            raise SystemExit(f"duplicate validator id: {validator_id}")
        seen.add(validator_id)
        if network not in NETWORK_LABELS:
            raise SystemExit(f"unsupported network: {network}")

        coordinate = {
            "validator_id": validator_id,
            "network": network,
            "network_label": NETWORK_LABELS[network],
            "claimed_domain": row.get("domain"),
            "domain_verification_status": row.get("domain_verified"),
            "list_publishers": row.get("list_publishers", []),
            "metadata_source": row.get("metadata_source"),
        }
        network_inputs = INPUTS / network
        network_prompts = PROMPTS / network
        network_inputs.mkdir(exist_ok=True)
        network_prompts.mkdir(exist_ok=True)
        coordinate_path = network_inputs / f"{validator_id}.json"
        prompt_path = network_prompts / f"{validator_id}.txt"
        coordinate_path.write_text(canonical(coordinate) + "\n")

        domain = coordinate["claimed_domain"]
        verification = coordinate["domain_verification_status"]
        prompt = render(
            template,
            {
                "NETWORK_LABEL": coordinate["network_label"],
                "VALIDATOR_ID": validator_id,
                "CLAIMED_DOMAIN": (
                    f"{domain} (claimed)" if domain is not None else "null (no domain supplied)"
                ),
                "DOMAIN_VERIFICATION_STATUS": (
                    "null (not independently established in the frozen input)"
                    if verification is None
                    else (
                        "true in the frozen upstream input; not independently re-verified here"
                        if verification
                        else "false in the frozen upstream input"
                    )
                ),
                "CLAIMED_DOMAIN_JSON": canonical(domain),
                "DOMAIN_VERIFICATION_JSON": canonical(verification),
                "LIST_PUBLISHERS": ", ".join(coordinate["list_publishers"]) or "None supplied",
                "METADATA_SOURCE": coordinate["metadata_source"] or "None supplied",
            },
        )
        prompt_path.write_text(prompt)
        index.append(
            {
                **coordinate,
                "coordinate_path": str(coordinate_path.relative_to(ROOT)),
                "coordinate_sha256": sha256(coordinate_path),
                "prompt_path": str(prompt_path.relative_to(ROOT)),
                "prompt_sha256": sha256(prompt_path),
            }
        )

    (INPUTS / "index.json").write_text(canonical(index) + "\n")
    print(
        canonical(
            {
                "validator_count": len(index),
                "xrpl": sum(row["network"] == "xrpl" for row in index),
                "postfiat": sum(row["network"] == "postfiat" for row in index),
                "input_index_sha256": sha256(INPUTS / "index.json"),
            }
        )
    )


if __name__ == "__main__":
    main()
