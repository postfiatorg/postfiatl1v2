#!/usr/bin/env python3
"""Verify the Cobalt controlled-testnet cutover decision packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path, PurePosixPath
from typing import Any

VERIFIER_SCHEMA = "postfiat-cobalt-activation-readiness-verifier-v1"
DECISION_SCHEMA = "postfiat-cobalt-activation-readiness-decision-v1"
SOURCE_SCHEMA = "postfiat-cobalt-activation-readiness-source-manifest-v1"
LIVE_SCHEMA = "postfiat-cobalt-live-shadow-health-v1"
HTTP_SCHEMA = "postfiat-cobalt-read-only-http-proof-v1"
EXPECTED_BENCHMARK_ROOT = "7968a085033419255b52b844edd586346a1e85561394e52c69e6683b2561c50b"
EXPECTED_HANDOFF_ROOT = "b678b3f45eb2a14299b941101bd556d61795a1033f1f6e53557442b7e315807e"
EXPECTED_SECTION4_ROOT = "333c5abdc295ee785c719d58ea0835f5a502eb5759245d1ca6ee863e74239232"
MAX_FILE_BYTES = 4 * 1024 * 1024
REQUIRED_DATA_FILES = {
    "cli-readiness.json",
    "cli-scenario.json",
    "decision.json",
    "http-methods.json",
    "live-shadow-health.json",
    "operator-checklist.md",
    "source-manifest.json",
    "test-results.json",
    "ui-snapshot.json",
}


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def read_bytes(path: Path) -> bytes:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"missing or non-regular packet file: {path.name}")
    payload = path.read_bytes()
    if len(payload) > MAX_FILE_BYTES:
        raise ValueError(f"oversized packet file: {path.name}")
    return payload


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(read_bytes(path))
    if not isinstance(value, dict):
        raise ValueError(f"packet JSON is not an object: {path.name}")
    return value


def verify_checksums(packet: Path) -> bool:
    manifest = read_bytes(packet / "SHA256SUMS").decode("ascii").splitlines()
    names: set[str] = set()
    for line in manifest:
        expected, separator, name = line.partition("  ")
        candidate = PurePosixPath(name)
        if (
            separator != "  "
            or len(expected) != 64
            or any(character not in "0123456789abcdef" for character in expected)
            or candidate.is_absolute()
            or len(candidate.parts) != 1
            or candidate.parts[0] in {".", ".."}
            or name in names
        ):
            return False
        names.add(name)
        if digest(read_bytes(packet / name)) != expected:
            return False
    return names == REQUIRED_DATA_FILES | {"verifier.json"}


def git_blob(repo: Path, commit: str, path: str) -> bytes:
    completed = subprocess.run(
        ["git", "show", f"{commit}:{path}"],
        cwd=repo,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise ValueError(f"cannot read {path} at source commit {commit}")
    return completed.stdout


def verify(packet: Path, *, require_checksums: bool) -> dict[str, Any]:
    packet = packet.resolve()
    repo = Path(__file__).resolve().parents[2]
    source = read_json(packet / "source-manifest.json")
    scenario = read_json(packet / "cli-scenario.json")
    readiness = read_json(packet / "cli-readiness.json")
    ui = read_json(packet / "ui-snapshot.json")
    live = read_json(packet / "live-shadow-health.json")
    http = read_json(packet / "http-methods.json")
    tests = read_json(packet / "test-results.json")
    decision = read_json(packet / "decision.json")

    source_files = source.get("files", {})
    source_commit = source.get("source_commit")
    source_hashes_match = (
        source.get("schema") == SOURCE_SCHEMA
        and isinstance(source_commit, str)
        and len(source_commit) == 40
        and isinstance(source_files, dict)
        and bool(source_files)
    )
    if source_hashes_match:
        try:
            source_hashes_match = all(
                digest(git_blob(repo, source_commit, path)) == expected
                for path, expected in source_files.items()
            )
        except (OSError, ValueError):
            source_hashes_match = False

    scenario_summary = scenario.get("summary", {})
    scenario_passes = (
        scenario.get("command") == "scenario"
        and scenario.get("ok") is True
        and scenario_summary.get("case_count") == 80
        and scenario_summary.get("cobalt_passed") == 80
        and scenario_summary.get("rippled_passed") == 80
        and scenario_summary.get("cobalt_conflicting_decisions") == 0
        and scenario_summary.get("rippled_conflicting_decisions") == 0
        and scenario_summary.get("cobalt_replay_equal") is True
        and scenario_summary.get("rippled_native_fork_control") is True
        and scenario_summary.get("unresolved_methodology_exception") is False
    )
    readiness_checks = readiness.get("checks", [])
    readiness_passes = (
        readiness.get("command") == "readiness"
        and readiness.get("ok") is True
        and readiness.get("status") == "GO"
        and readiness.get("activation_performed") is False
        and isinstance(readiness_checks, list)
        and len(readiness_checks) >= 6
        and all(isinstance(row, dict) and row.get("ok") is True for row in readiness_checks)
        and readiness.get("actual_authority", {}).get("validator_trust") == "foundation"
        and readiness.get("actual_authority", {}).get("cobalt_active") is False
        and readiness.get("actual_authority", {}).get("block_finality") == "consensus-v2"
    )
    packet_roots_match = (
        source.get("evidence_roots", {}).get("benchmark_sha256sums")
        == EXPECTED_BENCHMARK_ROOT
        and source.get("evidence_roots", {}).get("handoff_sha256sums")
        == EXPECTED_HANDOFF_ROOT
        and source.get("evidence_roots", {}).get("section4_live_sha256sums")
        == EXPECTED_SECTION4_ROOT
        and scenario.get("packet", {}).get("manifest_sha256")
        == EXPECTED_BENCHMARK_ROOT
        and readiness.get("packets", {}).get("handoff", {}).get("manifest_sha256")
        == EXPECTED_HANDOFF_ROOT
    )
    shadow_nodes = live.get("nodes", [])
    live_shadow_passes = (
        live.get("schema") == LIVE_SCHEMA
        and live.get("source_sha256sums") == EXPECTED_SECTION4_ROOT
        and live.get("node_count") == 6
        and live.get("all_healthy") is True
        and live.get("five_of_six_progress") is True
        and live.get("four_of_six_rejected") is True
        and live.get("signed_catch_up_succeeded") is True
        and live.get("consensus_v2_finalized_during_fault") is True
        and isinstance(shadow_nodes, list)
        and len(shadow_nodes) == 6
        and all(
            isinstance(node, dict)
            and node.get("live_authority") is False
            and node.get("controls_block_consensus") is False
            and node.get("catch_up_status") == "current"
            and node.get("peer_health") == "healthy"
            and node.get("queue_health") == "healthy"
            for node in shadow_nodes
        )
    )
    ui_states_separate = (
        ui.get("schema") == "postfiat-cobalt-governance-ui-snapshot-v2"
        and ui.get("read_only") is True
        and ui.get("shadow_health", {}).get("ok") is True
        and ui.get("rehearsal_readiness", {}).get("status") == "GO"
        and ui.get("rehearsal_readiness", {}).get("ready") is True
        and ui.get("rehearsal_readiness", {}).get("activation_performed") is False
        and ui.get("actual_authority", {}).get("foundation_active") is True
        and ui.get("actual_authority", {}).get("cobalt_active") is False
        and ui.get("actual_authority", {}).get("block_finality") == "consensus-v2"
        and ui.get("scenario", {}).get("case_count") == 80
        and ui.get("errors") == []
    )
    http_read_only = (
        http.get("schema") == HTTP_SCHEMA
        and http.get("read_only") is True
        and http.get("allowed_methods") == ["GET", "HEAD"]
        and http.get("post_status") == 405
        and http.get("mutation_routes") == []
        and http.get("security_headers_verified") is True
    )
    tests_pass = (
        tests.get("returncode") == 0
        and tests.get("passed") is True
        and tests.get("test_count", 0) >= 21
        and tests.get("command")
        == "PYTHONPATH=python python3 -m unittest -q python.tests.test_cobalt python.tests.test_cobalt_ui"
    )
    decision_is_scoped_go = (
        decision.get("schema") == DECISION_SCHEMA
        and decision.get("decision") == "GO"
        and decision.get("cutover_authorized_by_packet") is False
        and decision.get("activation_performed") is False
        and decision.get("requires_explicit_user_authorization") is True
        and decision.get("requires_tasknode_governance") is True
        and decision.get("actual_authority", {}).get("validator_trust") == "foundation"
        and decision.get("actual_authority", {}).get("block_finality") == "consensus-v2"
    )
    checksum_passes = verify_checksums(packet) if require_checksums else True

    checks = {
        "checksum_manifest_passes": checksum_passes,
        "source_commit_hashes_match": source_hashes_match,
        "prior_evidence_roots_match": packet_roots_match,
        "matched_scenario_passes": scenario_passes,
        "cutover_readiness_passes": readiness_passes,
        "six_validator_shadow_health_passes": live_shadow_passes,
        "ui_three_states_are_separate": ui_states_separate,
        "http_surface_is_read_only": http_read_only,
        "cli_ui_tests_pass": tests_pass,
        "decision_is_scoped_go_without_activation": decision_is_scoped_go,
    }
    return {
        "schema": VERIFIER_SCHEMA,
        "result": "passed" if all(checks.values()) else "failed",
        "checks": checks,
        "source_commit": source_commit,
        "decision": decision.get("decision"),
        "actual_authority": decision.get("actual_authority"),
        "activation_performed": decision.get("activation_performed"),
        "evidence_roots": source.get("evidence_roots"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packet", type=Path, default=Path(__file__).with_name("packet"))
    parser.add_argument("--write-verifier", action="store_true")
    args = parser.parse_args()
    result = verify(args.packet, require_checksums=not args.write_verifier)
    if args.write_verifier:
        (args.packet / "verifier.json").write_text(
            json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    print(
        f"cobalt-activation-readiness-packet-{result['result']} "
        f"checks={sum(value is True for value in result['checks'].values())}/"
        f"{len(result['checks'])}"
    )
    return 0 if result["result"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
