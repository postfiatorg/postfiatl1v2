#!/usr/bin/env python3
"""Rehearse compatible post-activation software rollback on six local validators.

The runner never contacts the controlled devnet. It creates a disposable chain
whose transactional commitment is active at height 1, finalizes with the
current release, resumes the same certified tip with a distinct older release,
then returns to the current release and finalizes again.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
CAMPAIGN_RUNNER = REPO / "benchmarks" / "storage-scaling" / "run_campaign.py"
REPORT_SCHEMA = "postfiat-storage-compatible-rollback-v1"


def load_campaign_runner() -> Any:
    spec = importlib.util.spec_from_file_location(
        "postfiat_storage_scaling_campaign_runner",
        CAMPAIGN_RUNNER,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load campaign runner: {CAMPAIGN_RUNNER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


CAMPAIGN = load_campaign_runner()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def git_revision() -> str:
    return subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()


def git_clean() -> bool:
    return (
        subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=REPO,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout
        == ""
    )


def is_ancestor(ancestor: str, descendant: str) -> bool:
    return (
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", ancestor, descendant],
            cwd=REPO,
            check=False,
        ).returncode
        == 0
    )


def binary_identity(
    node_bin: Path,
    data_dir: Path,
    revision: str,
) -> dict[str, str]:
    build = CAMPAIGN.require_release_binary_identity(
        node_bin,
        data_dir,
        revision,
    )
    return {
        "sha256": sha256(node_bin),
        "git_revision": build["git_revision"],
        "source_revision": revision,
        "profile": build["profile"],
    }


def converged_identity(fleet: Any, label: str) -> dict[str, Any]:
    if not isinstance(fleet, list) or len(fleet) != CAMPAIGN.VALIDATORS:
        raise RuntimeError(f"{label} did not contain six validators")
    if not all(isinstance(node, dict) for node in fleet):
        raise RuntimeError(f"{label} contains a malformed validator identity")
    identities = {
        (
            int(node["height"]),
            str(node["tip"]),
            str(node["state_root"]),
        )
        for node in fleet
    }
    if len(identities) != 1:
        raise RuntimeError(f"{label} did not converge on one certified identity")
    height, tip, state_root = next(iter(identities))
    return {
        "height": height,
        "tip": tip,
        "state_root": state_root,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--rollback-node-bin", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--expected-source-revision", required=True)
    parser.add_argument("--rollback-source-revision", required=True)
    parser.add_argument(
        "--development-smoke",
        action="store_true",
        help="allow a dirty current checkout; output is not evidence eligible",
    )
    args = parser.parse_args()

    node_bin = args.node_bin.resolve()
    rollback_bin = args.rollback_node_bin.resolve()
    root = args.output_dir.resolve()
    current_revision = git_revision()
    rollback_revision = args.rollback_source_revision

    if root.exists():
        raise ValueError(f"refusing to overwrite output directory: {root}")
    for label, path in (
        ("--node-bin", node_bin),
        ("--rollback-node-bin", rollback_bin),
    ):
        if not path.is_file() or path.is_symlink() or path.parent.name != "release":
            raise ValueError(f"{label} must identify a regular target/release binary")
    if node_bin == rollback_bin or sha256(node_bin) == sha256(rollback_bin):
        raise ValueError("rollback rehearsal requires two distinct release binaries")
    if (
        args.expected_source_revision != current_revision
        or len(current_revision) != 40
        or len(rollback_revision) != 40
    ):
        raise ValueError("source revisions must be full object IDs and current must match HEAD")
    if rollback_revision == current_revision:
        raise ValueError("rollback source must be older than the current source")
    if not is_ancestor(rollback_revision, current_revision):
        raise ValueError("rollback source is not an ancestor of the current source")
    if not args.development_smoke and not git_clean():
        raise ValueError("rollback evidence requires a clean checkout")

    root.mkdir(parents=True)
    (root / "snapshots").mkdir()
    (root / "receipts").mkdir()
    (root / "normalized").mkdir()
    base_port, rpc_base_port = CAMPAIGN.SHARED.find_ports()
    seed, height_one, wallet_key, wallet_address, recipient, topology = (
        CAMPAIGN.setup_seed(node_bin, root, base_port, rpc_base_port)
    )

    current_binary = binary_identity(node_bin, seed, current_revision)
    rollback_binary = binary_identity(rollback_bin, seed, rollback_revision)

    current_result, height_two = CAMPAIGN.run_rounds(
        node_bin=node_bin,
        root=root,
        seed=seed,
        topology=topology,
        source_snapshot=height_one,
        wallet_key=wallet_key,
        wallet_address=wallet_address,
        recipient=recipient,
        label="current-post-activation",
        rounds=1,
    )
    rollback_result, height_three = CAMPAIGN.run_rounds(
        node_bin=rollback_bin,
        root=root,
        seed=seed,
        topology=topology,
        source_snapshot=height_two,
        wallet_key=wallet_key,
        wallet_address=wallet_address,
        recipient=recipient,
        label="compatible-rollback",
        rounds=1,
    )
    forward_result, _height_four = CAMPAIGN.run_rounds(
        node_bin=node_bin,
        root=root,
        seed=seed,
        topology=topology,
        source_snapshot=height_three,
        wallet_key=wallet_key,
        wallet_address=wallet_address,
        recipient=recipient,
        label="current-forward-recovery",
        rounds=1,
    )

    current_final = converged_identity(
        current_result["final_fleet"],
        "current post-activation final fleet",
    )
    rollback_initial = converged_identity(
        rollback_result["initial_fleet"],
        "rollback initial fleet",
    )
    rollback_final = converged_identity(
        rollback_result["final_fleet"],
        "rollback final fleet",
    )
    forward_initial = converged_identity(
        forward_result["initial_fleet"],
        "forward-recovery initial fleet",
    )
    forward_final = converged_identity(
        forward_result["final_fleet"],
        "forward-recovery final fleet",
    )
    if rollback_initial != current_final:
        raise RuntimeError("rollback binary did not resume the exact current certified tip")
    if forward_initial != rollback_final:
        raise RuntimeError("current binary did not resume the rollback release's certified tip")
    if (
        current_final["height"] != 2
        or rollback_final["height"] != 3
        or forward_final["height"] != 4
    ):
        raise RuntimeError("rollback rehearsal crossed unexpected heights")
    for label, result in (
        ("current", current_result),
        ("rollback", rollback_result),
        ("forward", forward_result),
    ):
        if (
            result.get("literal_receipts_exact") is not True
            or result.get("zero_full_history_reads") is not True
            or result.get("bounded_index_pages") is not True
            or result.get("constant_accumulator_work") is not True
            or result.get("validators_converged") != CAMPAIGN.VALIDATORS
        ):
            raise RuntimeError(f"{label} phase did not pass its finality/storage gates")

    evidence_eligible = not args.development_smoke
    status = "PASS" if evidence_eligible else "DEVELOPMENT SMOKE PASS"
    report = {
        "schema": REPORT_SCHEMA,
        "status": status,
        "evidence_eligible": evidence_eligible,
        "source_revision": current_revision,
        "current_binary": current_binary,
        "rollback_binary": rollback_binary,
        "rollback_source_is_ancestor": True,
        "chain_id": CAMPAIGN.CHAIN_ID,
        "validator_count": CAMPAIGN.VALIDATORS,
        "storage_activation_height": CAMPAIGN.STORAGE_ACTIVATION_HEIGHT,
        "consensus_activation_height": CAMPAIGN.CONSENSUS_ACTIVATION_HEIGHT,
        "activated_commitment_understood": True,
        "resumed_same_certified_tip": True,
        "post_activation_finality_with_rollback_binary": True,
        "forward_recovery_with_current_binary": True,
        "literal_receipts_exact": True,
        "zero_full_history_reads": True,
        "bounded_index_pages": True,
        "constant_accumulator_work": True,
        "all_six_converged": True,
        "identities": {
            "current_post_activation": current_final,
            "rollback_resume_input": rollback_initial,
            "rollback_finalized": rollback_final,
            "forward_resume_input": forward_initial,
            "forward_finalized": forward_final,
        },
        "offline": True,
        "network_contacted": False,
        "devnet_queried_or_mutated": False,
    }
    report_path = root / "compatible-rollback-report.json"
    write_json(report_path, report)
    print(f"storage-compatible-rollback={status}", flush=True)
    print(f"report={report_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
