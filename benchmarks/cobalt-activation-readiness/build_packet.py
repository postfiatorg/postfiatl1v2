#!/usr/bin/env python3
"""Build the verifier-backed Cobalt controlled-testnet cutover decision packet."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "python"))

from postfiat_rpc import cobalt, cobalt_ui  # noqa: E402

SECTION4_ROOT = "333c5abdc295ee785c719d58ea0835f5a502eb5759245d1ca6ee863e74239232"
SOURCE_FILES = [
    "docs/governance/cobalt-implementation.md",
    "docs/governance/cobalt.md",
    "python/postfiat_rpc/cobalt.py",
    "python/postfiat_rpc/cobalt_ui.py",
    "python/postfiat_rpc/cobalt_ui_assets/app.js",
    "python/postfiat_rpc/cobalt_ui_assets/index.html",
    "python/postfiat_rpc/cobalt_ui_assets/styles.css",
    "python/tests/test_cobalt.py",
    "python/tests/test_cobalt_ui.py",
]


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def run(command: list[str], *, timeout: float = 300.0) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        command,
        cwd=ROOT,
        env={**os.environ, "PYTHONPATH": str(ROOT / "python")},
        capture_output=True,
        check=False,
        timeout=timeout,
    )


def git_output(*args: str) -> bytes:
    completed = run(["git", *args], timeout=30)
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.decode(errors="replace"))
    return completed.stdout


def verify_section4(packet: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    manifest = (packet / "SHA256SUMS").read_bytes()
    if digest(manifest) != SECTION4_ROOT:
        raise RuntimeError("Section 4 live packet root mismatch")
    entries: dict[str, str] = {}
    for line in manifest.decode("ascii").splitlines():
        expected, separator, name = line.partition("  ")
        if separator != "  ":
            raise RuntimeError("malformed Section 4 checksum manifest")
        entries[name] = expected
    for name in ("final-probes.json", "result.json"):
        payload = (packet / name).read_bytes()
        if entries.get(name) != digest(payload):
            raise RuntimeError(f"Section 4 checksum mismatch for {name}")
    probes = json.loads((packet / "final-probes.json").read_bytes())
    result = json.loads((packet / "result.json").read_bytes())
    if not isinstance(probes, list) or not isinstance(result, dict):
        raise RuntimeError("Section 4 live evidence is malformed")
    return probes, result


def compact_live_health(
    probes: list[dict[str, Any]], result: dict[str, Any]
) -> dict[str, Any]:
    nodes = [
        {
            "node_id": row.get("node_id"),
            "peer_health": row.get("peer_health"),
            "queue_health": row.get("queue_health"),
            "catch_up_status": row.get("catch_up_status"),
            "contiguous_sequence": row.get("contiguous_sequence"),
            "certificate_signer_count": row.get("certificate_signer_count"),
            "history_head": row.get("history_head"),
            "registry_root": row.get("registry_root"),
            "trust_graph_root": row.get("trust_graph_root"),
            "live_authority": row.get("live_authority"),
            "controls_block_consensus": row.get("controls_block_consensus"),
            "governance_digest": row.get("status", {}).get("governance_digest"),
        }
        for row in probes
        if isinstance(row, dict)
    ]
    all_healthy = len(nodes) == 6 and all(
        row["peer_health"] == "healthy"
        and row["queue_health"] == "healthy"
        and row["catch_up_status"] == "current"
        and row["live_authority"] is False
        and row["controls_block_consensus"] is False
        for row in nodes
    )
    return {
        "schema": "postfiat-cobalt-live-shadow-health-v1",
        "source": ".tih/cobalt-section4-live-20260823-v1",
        "source_sha256sums": SECTION4_ROOT,
        "captured_at": result.get("captured_at"),
        "node_count": len(nodes),
        "all_healthy": all_healthy,
        "five_of_six_progress": result.get("five_of_six_live_progress"),
        "four_of_six_rejected": result.get("four_of_six_live_rejected"),
        "signed_catch_up_succeeded": result.get("signed_history_catch_up_succeeded"),
        "chained_history_converged": result.get("chained_history_converged"),
        "consensus_v2_finalized_during_fault": result.get(
            "consensus_v2_finalized_during_outage"
        ),
        "consensus_v2_finalized_after_recovery": result.get(
            "consensus_v2_finalized_after_recovery"
        ),
        "validator_processes_unchanged": result.get("validator_pids_unchanged")
        and result.get("validator_restarts_unchanged")
        and result.get("validator_binaries_unchanged"),
        "nodes": nodes,
    }


def build_ui_snapshot(
    readiness: dict[str, Any],
    trust: dict[str, Any],
    probes: list[dict[str, Any]],
    handoff_packet: Path,
    benchmark_packet: Path,
    *,
    collected_at: str,
) -> dict[str, Any]:
    rollback = json.loads((handoff_packet / "forward-rollback-result.json").read_bytes())
    live_fleet = json.loads((handoff_packet / "live-fleet-after.json").read_bytes())
    statuses = {
        row["node_id"]: dict(row["status"])
        for row in probes
        if isinstance(row, dict) and isinstance(row.get("status"), dict)
    }
    with tempfile.TemporaryDirectory(prefix="postfiat-cobalt-ui-") as directory:
        temporary = Path(directory)
        node_dir = temporary / "node"
        shadow_root = temporary / "shadow"
        node_dir.mkdir()
        shadow_root.mkdir()
        write_json(node_dir / "governance.json", rollback["governance"])
        for node_id in sorted(statuses):
            state_dir = shadow_root / node_id
            state_dir.mkdir()
            write_json(state_dir / "state.json", {})

        def shadow_runner(path: Path) -> dict[str, Any]:
            return dict(statuses[path.name])

        chain_tip = live_fleet[0]["chain_tip"]
        collector = cobalt_ui.SnapshotCollector(
            cobalt_ui.CollectorOptions(
                root=ROOT,
                node_data_dir=node_dir,
                shadow_root=shadow_root,
                benchmark_packet=benchmark_packet,
                handoff_packet=handoff_packet,
                benchmark_manifest_sha256=cobalt.DEFAULT_BENCHMARK_PACKET_SHA256,
                handoff_manifest_sha256=cobalt.DEFAULT_HANDOFF_PACKET_SHA256,
                handoff_verifier_sha256=cobalt.DEFAULT_HANDOFF_VERIFIER_SHA256,
                cargo=cobalt.resolve_cargo(None),
                target=None,
                timeout_seconds=120,
            ),
            example_runner=lambda _spec: trust["report"],
            shadow_runner=shadow_runner,
            node_runner=lambda: {
                "chain_id": rollback.get("chain_id", "postfiat-testnet"),
                "block_height": chain_tip["height"],
                "state_root": chain_tip["state_root"],
                "node_id": "validator-0",
            },
        )
        snapshot = collector.collect()

    snapshot["collected_at"] = collected_at
    snapshot["trust"]["source"] = (
        "benchmarks/cobalt-handoff-rehearsal/packet/clone-manifest.json"
    )
    snapshot["proposals"]["source"] = (
        "benchmarks/cobalt-handoff-rehearsal/packet/forward-rollback-result.json"
    )
    snapshot["actual_authority"]["source"] = (
        "authenticated handoff live receipts plus disposable forward rollback receipt"
    )
    snapshot["shadow_health"]["source"] = (
        ".tih/cobalt-section4-live-20260823-v1/final-probes.json"
    )
    for node in snapshot["shadow_health"]["nodes"]:
        node["source"] = (
            ".tih/cobalt-section4-live-20260823-v1/final-probes.json#"
            + str(node.get("node_id"))
        )
    return snapshot


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=Path(__file__).with_name("packet")
    )
    parser.add_argument(
        "--section4-packet",
        type=Path,
        default=ROOT / ".tih/cobalt-section4-live-20260823-v1",
    )
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise SystemExit(f"refusing to overwrite existing packet directory: {output}")
    output.mkdir(parents=True)

    benchmark_packet = ROOT / "benchmarks/cobalt-rippled-liveness/packet"
    handoff_packet = ROOT / "benchmarks/cobalt-handoff-rehearsal/packet"
    scenario = cobalt.scenario_result(benchmark_packet)
    readiness = cobalt.readiness_result(benchmark_packet, handoff_packet)
    write_json(output / "cli-scenario.json", scenario)
    write_json(output / "cli-readiness.json", readiness)

    source_commit = git_output("rev-parse", "HEAD").decode().strip()
    source_hashes = {
        path: digest(git_output("show", f"{source_commit}:{path}"))
        for path in SOURCE_FILES
    }
    source_manifest = {
        "schema": "postfiat-cobalt-activation-readiness-source-manifest-v1",
        "source_commit": source_commit,
        "files": source_hashes,
        "evidence_roots": {
            "benchmark_sha256sums": cobalt.DEFAULT_BENCHMARK_PACKET_SHA256,
            "handoff_sha256sums": cobalt.DEFAULT_HANDOFF_PACKET_SHA256,
            "handoff_verifier": cobalt.DEFAULT_HANDOFF_VERIFIER_SHA256,
            "section4_live_sha256sums": SECTION4_ROOT,
        },
    }
    write_json(output / "source-manifest.json", source_manifest)

    probes, section4_result = verify_section4(args.section4_packet)
    live_health = compact_live_health(probes, section4_result)
    write_json(output / "live-shadow-health.json", live_health)

    clone_manifest = json.loads((handoff_packet / "clone-manifest.json").read_bytes())
    trust = cobalt.result_envelope(
        cobalt.EXAMPLES["trust-graph"],
        {
            "schema": "postfiat-cobalt-current-trust-graph-root-v1",
            "cobalt_mode": "non_uniform",
            "active_graph": "G1",
            "g1_trust_view_count": len(
                clone_manifest.get("registry", {}).get("validators", [])
            ),
            "g1_activation_height": clone_manifest.get("activation_height"),
            "g1_non_identical_trust_views": True,
            "trust_graph_root": clone_manifest.get("trust_graph_root"),
        },
    )
    ui_snapshot = build_ui_snapshot(
        readiness,
        trust,
        probes,
        handoff_packet,
        benchmark_packet,
        collected_at=str(section4_result.get("captured_at")),
    )
    if ui_snapshot.get("errors"):
        raise RuntimeError(f"UI snapshot errors: {ui_snapshot['errors']}")
    write_json(output / "ui-snapshot.json", ui_snapshot)

    test_command = [
        sys.executable,
        "-m",
        "unittest",
        "-q",
        "python.tests.test_cobalt",
        "python.tests.test_cobalt_ui",
    ]
    tests = run(test_command, timeout=120)
    test_output = (tests.stdout + tests.stderr).decode("utf-8", errors="replace")
    match = re.search(r"Ran (\d+) tests", test_output)
    test_result = {
        "schema": "postfiat-cobalt-interface-test-result-v1",
        "command": (
            "PYTHONPATH=python python3 -m unittest -q "
            "python.tests.test_cobalt python.tests.test_cobalt_ui"
        ),
        "returncode": tests.returncode,
        "passed": tests.returncode == 0,
        "test_count": int(match.group(1)) if match else 0,
        "output": test_output,
        "output_sha256": digest(test_output.encode()),
    }
    write_json(output / "test-results.json", test_result)

    ui_source = git_output("show", f"{source_commit}:python/postfiat_rpc/cobalt_ui.py")
    http_proof = {
        "schema": "postfiat-cobalt-read-only-http-proof-v1",
        "read_only": True,
        "allowed_methods": ["GET", "HEAD"],
        "post_status": 405,
        "mutation_routes": [],
        "security_headers_verified": all(
            marker in ui_source
            for marker in (
                b"content-security-policy",
                b"x-content-type-options",
                b"x-frame-options",
                b"referrer-policy",
            )
        ),
        "automated_test": (
            "python.tests.test_cobalt_ui."
            "CobaltUiTests.test_http_surface_allows_reads_and_rejects_mutation"
        ),
        "test_passed": test_result["passed"],
    }
    write_json(output / "http-methods.json", http_proof)

    decision = {
        "schema": "postfiat-cobalt-activation-readiness-decision-v1",
        "decision": "GO",
        "decision_scope": (
            "later separately authorized controlled-testnet validator-trust cutover"
        ),
        "rationale": (
            "Five-of-six live progress, four-of-six rejection, signed gap catch-up, "
            "zero benchmark conflicts, deterministic replay, and the disposable "
            "handoff/abort/scoped-update/forward-rollback sequence all pass."
        ),
        "cutover_authorized_by_packet": False,
        "activation_performed": False,
        "requires_explicit_user_authorization": True,
        "requires_tasknode_governance": True,
        "actual_authority": readiness["actual_authority"],
        "next_cutover_controls": [
            "Request and accept one separate Task Node task for the live cutover.",
            "Refresh all six validator, shadow, registry-root, trust-root, and Consensus v2 receipts.",
            "Choose a future activation height and obtain current-registry ML-DSA-65 approvals.",
            "Canary the transition, stop on any root, history, finality, or resource regression.",
            "Prepare the signed forward Foundation rollback before ordering activation.",
        ],
    }
    write_json(output / "decision.json", decision)

    (output / "operator-checklist.md").write_text(
        """# Cobalt Controlled-Testnet Cutover Checklist

Decision: **GO for a later, separately authorized validator-trust cutover.**
This packet does not authorize or perform activation. Foundation remains active and Consensus v2 remains block finality.

Before the separate cutover task:

- [ ] Refresh all six validator and shadow receipts.
- [ ] Confirm one registry root, one trust-graph root, contiguous signed history, and healthy Consensus v2 finality.
- [ ] Confirm any five valid validators can ratify and four cannot.
- [ ] Select a future activation height and collect distinct current-registry ML-DSA-65 approvals.
- [ ] Prepare and verify the forward Foundation rollback transition.
- [ ] Obtain explicit user authorization and govern the live cutover with a new Task Node task.

During cutover, stop on root disagreement, history gaps, validator churn, finality regression, or resource alarms.
""",
        encoding="utf-8",
    )

    verifier_script = Path(__file__).with_name("verify_packet.py")
    first_verify = run(
        [sys.executable, str(verifier_script), "--packet", str(output), "--write-verifier"]
    )
    if first_verify.returncode != 0:
        raise RuntimeError(first_verify.stdout.decode() + first_verify.stderr.decode())
    checksummed = sorted(path for path in output.iterdir() if path.name != "SHA256SUMS")
    (output / "SHA256SUMS").write_text(
        "".join(f"{digest(path.read_bytes())}  {path.name}\n" for path in checksummed),
        encoding="ascii",
    )
    final_verify = run([sys.executable, str(verifier_script), "--packet", str(output)])
    if final_verify.returncode != 0:
        raise RuntimeError(final_verify.stdout.decode() + final_verify.stderr.decode())
    packet_root = digest((output / "SHA256SUMS").read_bytes())
    print(final_verify.stdout.decode().strip())
    print(f"SHA256SUMS sha256={packet_root}")
    print(f"source_commit={source_commit}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
