#!/usr/bin/env python3
"""Verify all identity packets and publish the frozen corpus index and manifest."""

from __future__ import annotations

import hashlib
import json
import pathlib
import re
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
INPUT_INDEX = ROOT / "inputs" / "index.json"
SOURCE_VALIDATORS = ROOT / "inputs" / "validators.json"
TEMPLATE = ROOT / "prompt_template.txt"
EXPECTED_SOURCE_SHA256 = (
    "7687dcd9a23638dca4e0fbe50c2dd3782c6db89fa645802cd5dd9586feb87f27"
)
HEADINGS = [
    "# Validator Identity Packet",
    "## Packet Status",
    "## Validator Coordinates",
    "## Claimed Domain and Official URLs",
    "## Public Identity",
    "## Business Summary",
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
    "business_summary",
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


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def digest_lines(lines: list[str]) -> str:
    return hashlib.sha256("".join(lines).encode()).hexdigest()


def packet_sections(packet: str) -> dict[str, str]:
    matches = list(re.finditer(r"^(#{1,2} .+)$", packet, flags=re.MULTILINE))
    sections: dict[str, str] = {}
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(packet)
        sections[match.group(1)] = packet[match.end() : end].strip()
    return sections


def machine_summary(packet: str) -> dict[str, Any]:
    match = re.search(
        r"^## Machine-Readable Summary\n\n\x60\x60\x60json\n(.*?)\n\x60\x60\x60\s*$",
        packet,
        flags=re.MULTILINE | re.DOTALL,
    )
    if not match:
        raise ValueError("missing terminal machine-readable JSON")
    return json.loads(match.group(1))


def verify_one(row: dict[str, Any]) -> dict[str, Any]:
    validator_id = row["validator_id"]
    network = row["network"]
    prompt = ROOT / row["prompt_path"]
    packet = ROOT / "packets" / network / f"{validator_id}.md"
    exec_log = ROOT / "logs" / network / f"{validator_id}.jsonl"
    stderr_log = ROOT / "logs" / network / f"{validator_id}.stderr.log"
    run_path = ROOT / "runs" / network / f"{validator_id}.json"
    required = (prompt, packet, exec_log, stderr_log, run_path)
    missing = [str(path.relative_to(ROOT)) for path in required if not path.exists()]
    if missing:
        raise ValueError(f"missing files: {missing}")

    run = json.loads(run_path.read_text())
    if run["status"] != "PASS" or run["returncode"] != 0:
        raise ValueError(f"run receipt is not PASS: {run.get('error')}")
    expected_hashes = {
        "prompt_sha256": sha256(prompt),
        "packet_sha256": sha256(packet),
        "exec_log_sha256": sha256(exec_log),
        "stderr_log_sha256": sha256(stderr_log),
    }
    for field, expected in expected_hashes.items():
        if run[field] != expected:
            raise ValueError(f"{field} receipt mismatch")
    if row["prompt_sha256"] != expected_hashes["prompt_sha256"]:
        raise ValueError("prompt differs from frozen input index")
    if stderr_log.read_bytes():
        raise ValueError("Corbanu exec stderr is not empty")

    packet_text = packet.read_text()
    headings = re.findall(r"^#{1,2} .+$", packet_text, flags=re.MULTILINE)
    if headings != HEADINGS:
        raise ValueError(f"heading contract mismatch: {headings}")
    if "SHADOW_ONLY" not in packet_text:
        raise ValueError("missing SHADOW_ONLY marker")
    if validator_id not in packet_text:
        raise ValueError("missing validator id")
    lowered = packet_text.lower()
    for forbidden in (
        "association_score",
        "legitimacy_score",
        "reputation_score",
        "sanctions_score",
        "credit_rating",
    ):
        if forbidden in lowered:
            raise ValueError(f"packet contains forbidden scoring field: {forbidden}")

    sections = packet_sections(packet_text)
    business = sections["## Business Summary"]
    if "\n" in business:
        raise ValueError("business summary must be exactly one paragraph")
    word_count = len(re.findall(r"\b[\w’&.-]+\b", business))
    if not 90 <= word_count <= 160:
        raise ValueError(f"business summary has {word_count} words")
    if re.search(r"\[[^]]+\]\([^)]+\)", business):
        raise ValueError("business summary must not contain Markdown links")
    if not re.search(r"^1\. ", sections["## Evidence"], flags=re.MULTILINE):
        raise ValueError("Evidence section is not a numbered list")

    summary = machine_summary(packet_text)
    if set(summary) != SUMMARY_KEYS:
        raise ValueError(
            f"summary key mismatch: missing={SUMMARY_KEYS - set(summary)}, "
            f"extra={set(summary) - SUMMARY_KEYS}"
        )
    if summary["validator_id"] != validator_id:
        raise ValueError("machine summary validator mismatch")
    if summary["network"] != row["network_label"]:
        raise ValueError("machine summary network mismatch")
    if summary["claimed_domain"] != row["claimed_domain"]:
        raise ValueError("machine summary claimed domain mismatch")
    if summary["domain_verification_status"] != row["domain_verification_status"]:
        raise ValueError("machine summary domain verification mismatch")
    if summary["business_summary"] != business:
        raise ValueError("machine summary business paragraph differs from prose")
    if summary["profile_size_tier"] not in PROFILE_TIERS:
        raise ValueError("unknown profile size tier")
    if not summary["evidence_urls"]:
        raise ValueError("empty evidence URL list")

    log_rows = [
        json.loads(line) for line in exec_log.read_text().splitlines() if line.strip()
    ]
    thread_rows = [item for item in log_rows if item.get("type") == "thread.started"]
    completions = [item for item in log_rows if item.get("type") == "turn.completed"]
    messages = [
        item["item"]["text"]
        for item in log_rows
        if item.get("type") == "item.completed"
        and item.get("item", {}).get("type") == "agent_message"
    ]
    if not (len(thread_rows) == len(completions) == len(messages) == 1):
        raise ValueError("incomplete Corbanu JSONL lifecycle")
    if messages[0].strip() != packet_text.strip():
        raise ValueError("logged final answer differs from packet")
    if run["thread_id"] != thread_rows[0]["thread_id"]:
        raise ValueError("thread id mismatch")
    if run["usage"] != completions[0]["usage"]:
        raise ValueError("usage mismatch")

    return {
        "validator_id": validator_id,
        "network": network,
        "network_label": row["network_label"],
        "claimed_domain": row["claimed_domain"],
        "domain_verification_status": row["domain_verification_status"],
        "canonical_entity": summary["canonical_entity"],
        "entity_type": summary["entity_type"],
        "x_handle": summary["x_handle"],
        "incorporation_region": summary["incorporation_region"],
        "operating_regions": summary["operating_regions"],
        "profile_size_tier": summary["profile_size_tier"],
        "profile_size_confidence": summary["profile_size_confidence"],
        "identity_confidence": summary["identity_confidence"],
        "business_summary": business,
        "business_summary_word_count": word_count,
        "evidence_url_count": len(summary["evidence_urls"]),
        "packet_path": str(packet.relative_to(ROOT)),
        "packet_sha256": expected_hashes["packet_sha256"],
        "prompt_path": str(prompt.relative_to(ROOT)),
        "prompt_sha256": expected_hashes["prompt_sha256"],
        "exec_log_path": str(exec_log.relative_to(ROOT)),
        "exec_log_sha256": expected_hashes["exec_log_sha256"],
        "stderr_log_path": str(stderr_log.relative_to(ROOT)),
        "stderr_log_sha256": expected_hashes["stderr_log_sha256"],
        "run_receipt_path": str(run_path.relative_to(ROOT)),
        "run_receipt_sha256": sha256(run_path),
        "thread_id": run["thread_id"],
        "started_at": run["started_at"],
        "finished_at": run["finished_at"],
        "duration_seconds": run["duration_seconds"],
        "usage": run["usage"],
    }


def markdown_index(records: list[dict[str, Any]]) -> str:
    lines = [
        "# Validator Identity Packet Index",
        "",
        "**Status:** SHADOW_ONLY",
        "",
        "Frozen public-identity research packets generated by one Corbanu Terminal",
        "exec session per validator. These are external evidence, not consensus data",
        "or scores. H200 replay must consume the packet bytes identified here.",
        "",
        "| Network | Validator | Claimed domain | Public identity | X | Region | Size | Packet SHA-256 |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in records:
        domain = row["claimed_domain"] or "—"
        entity = row["canonical_entity"] or "Not established"
        x_handle = row["x_handle"] or "—"
        region = row["incorporation_region"] or "Not established"
        lines.append(
            f"| {row['network']} | {row['validator_id']} | {domain} | "
            f"{entity} | {x_handle} | {region} | {row['profile_size_tier']} | "
            f"{row['packet_sha256']} |"
        )
    return "\n".join(lines) + "\n"


def main() -> None:
    rows = json.loads(INPUT_INDEX.read_text())
    if len(rows) != 55:
        raise SystemExit(f"expected 55 validators, got {len(rows)}")
    if sha256(SOURCE_VALIDATORS) != EXPECTED_SOURCE_SHA256:
        raise SystemExit("frozen validator corpus hash mismatch")

    records = []
    failures = []
    for row in rows:
        try:
            records.append(verify_one(row))
        except Exception as exc:
            failures.append(
                {
                    "validator_id": row["validator_id"],
                    "network": row["network"],
                    "error": f"{type(exc).__name__}: {exc}",
                }
            )
    network_order = {"xrpl": 0, "postfiat": 1}
    records.sort(key=lambda row: (network_order[row["network"]], row["validator_id"]))
    verification = {
        "verdict": "PASS" if not failures and len(records) == 55 else "FAIL",
        "validator_count": len(rows),
        "verified_count": len(records),
        "failure_count": len(failures),
        "failures": failures,
    }
    (ROOT / "verification.json").write_text(canonical(verification) + "\n")
    if failures:
        print(canonical(verification))
        raise SystemExit(1)

    (ROOT / "index.json").write_text(canonical(records) + "\n")
    (ROOT / "index.md").write_text(markdown_index(records))
    packet_lines = [
        f"{row['network']}|{row['validator_id']}|{row['packet_sha256']}\n"
        for row in records
    ]
    log_lines = [
        f"{row['network']}|{row['validator_id']}|{row['exec_log_sha256']}\n"
        for row in records
    ]
    receipt_lines = [
        f"{row['network']}|{row['validator_id']}|{row['run_receipt_sha256']}\n"
        for row in records
    ]
    manifest = {
        "artifact": "validator-identity-packets-20260901",
        "finalized_at": max(row["finished_at"] for row in records),
        "shadow_only": True,
        "consensus_input": False,
        "downstream_contract": "frozen Markdown packet bytes",
        "corbanu_exec": {
            "version": "0.1.36",
            "configured_model": "gpt-5.6-sol",
            "provider": "openai",
            "search_enabled": True,
            "sandbox": "read-only",
            "approval_policy": "never",
            "sessions_per_validator": 1,
            "codex_fallback_used": False,
            "openrouter_used": False,
        },
        "counts": {
            "validators": len(records),
            "xrpl": sum(row["network"] == "xrpl" for row in records),
            "postfiat": sum(row["network"] == "postfiat" for row in records),
            "packets": len(records),
            "exec_logs": len(records),
        },
        "hashes": {
            "source_validators_sha256": sha256(SOURCE_VALIDATORS),
            "input_index_sha256": sha256(INPUT_INDEX),
            "prompt_template_sha256": sha256(TEMPLATE),
            "packet_set_sha256": digest_lines(packet_lines),
            "exec_log_set_sha256": digest_lines(log_lines),
            "run_receipt_set_sha256": digest_lines(receipt_lines),
            "index_json_sha256": sha256(ROOT / "index.json"),
            "index_markdown_sha256": sha256(ROOT / "index.md"),
            "verification_sha256": sha256(ROOT / "verification.json"),
        },
    }
    (ROOT / "manifest.json").write_text(canonical(manifest) + "\n")
    print(
        canonical(
            {
                "verdict": "PASS",
                "validators": len(records),
                "xrpl": manifest["counts"]["xrpl"],
                "postfiat": manifest["counts"]["postfiat"],
                "packet_set_sha256": manifest["hashes"]["packet_set_sha256"],
                "exec_log_set_sha256": manifest["hashes"]["exec_log_set_sha256"],
            }
        )
    )


if __name__ == "__main__":
    main()
