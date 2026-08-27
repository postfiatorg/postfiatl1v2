#!/usr/bin/env python3
"""Run legacy, bounded-JSONL, and selected-store performance lanes locally.

The retired lanes use frozen release binaries from their owning revisions and
the selected lane uses the release binary under qualification. All three share
the exact validator keys, deterministic account derivation, semantic transfer
workload, loopback topology shape, full-vote policy, height/window cardinality,
host allocation, storage medium, and resource sampler. Lane-native snapshots
are required because the retired storage formats are not runtime modes of the
selected binary. No external network or devnet endpoint is contacted.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
BASE_RUNNER = REPO / "benchmarks" / "storage-scaling" / "run_campaign.py"
SPEC = REPO / "docs" / "architecture" / "storage-scaling-fix-spec.md"
LANE_ORDER = ("legacy-jsonl", "bounded-jsonl", "selected-indexed")
LEGACY_SOURCE_REVISION = "8cc7d15edc58b5f5a0b745143fef2d45203465ff"
BOUNDED_SOURCE_REVISION = "dfd0b9f11108b0b773d1e02bebae71685864228e"
HISTORICAL_REVISIONS = {
    "legacy-jsonl": LEGACY_SOURCE_REVISION,
    "bounded-jsonl": BOUNDED_SOURCE_REVISION,
}
STORAGE_BEHAVIORS = {
    "legacy-jsonl": "full-prefix JSON/JSONL and full ordered-history proposal path",
    "bounded-jsonl": "authenticated JSONL v2 heads and fixed-slot ordered index candidate",
    "selected-indexed": "transactional redb finality path and fixed-size accumulator",
}
SCHEMA = "postfiat-storage-scaling-paired-six-validator-campaign-v2"


def load_base_runner() -> Any:
    spec = importlib.util.spec_from_file_location(
        "postfiat_storage_scaling_selected_runner", BASE_RUNNER
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load selected-store runner: {BASE_RUNNER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BASE = load_base_runner()


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def validator_public_identities(path: Path) -> list[dict[str, str]]:
    value = BASE.read_json(path)
    records = value.get("validators")
    if not isinstance(records, list) or len(records) != BASE.VALIDATORS:
        raise ValueError("validator key file does not contain six records")
    identities: list[dict[str, str]] = []
    for record in records:
        if not isinstance(record, dict):
            raise ValueError("validator key record is malformed")
        node_id = str(record.get("node_id", ""))
        algorithm_id = str(record.get("algorithm_id", ""))
        public_key_hex = str(record.get("public_key_hex", ""))
        if (
            node_id not in {f"validator-{index}" for index in range(BASE.VALIDATORS)}
            or algorithm_id != "ML-DSA-65"
            or not public_key_hex
            or len(public_key_hex) % 2 != 0
        ):
            raise ValueError("validator public key identity is invalid")
        try:
            public_key = bytes.fromhex(public_key_hex)
        except ValueError as error:
            raise ValueError("validator public key is not hexadecimal") from error
        identities.append(
            {
                "node_id": node_id,
                "algorithm_id": algorithm_id,
                "public_key_sha256": hashlib.sha256(public_key).hexdigest(),
            }
        )
    identities.sort(key=lambda identity: identity["node_id"])
    if [identity["node_id"] for identity in identities] != [
        f"validator-{index}" for index in range(BASE.VALIDATORS)
    ]:
        raise ValueError("validator public key identities are duplicated")
    return identities


def require_revision(value: str, label: str) -> None:
    if len(value) != 40 or any(byte not in "0123456789abcdef" for byte in value):
        raise ValueError(f"{label} must be a full lowercase Git object ID")


def revision_is_ancestor(ancestor: str, descendant: str) -> bool:
    completed = subprocess.run(
        ["git", "merge-base", "--is-ancestor", ancestor, descendant],
        cwd=REPO,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return completed.returncode == 0


def release_binary(path: Path, label: str) -> Path:
    raw = path.expanduser()
    if raw.is_symlink():
        raise ValueError(f"{label} must not be a symlink")
    resolved = raw.resolve()
    if not resolved.is_file() or resolved.parent.name != "release":
        raise ValueError(f"{label} must be a regular target/release binary")
    return resolved


def aggregate_rows(rows: list[dict[str, Any]], lane_root: Path) -> None:
    resource_fields = (
        "cpu_ticks",
        "peak_rss_kib",
        "disk_growth_bytes",
        "bytes_read",
        "bytes_written",
        "sample_count",
        "duration_ms",
        "observed_pid_count",
        "foreground_process_count",
        "foreground_min_sample_count",
        "host_cpu_ticks",
        "host_total_memory_kib",
        "host_min_available_memory_kib",
        "network_received_bytes",
        "network_transmitted_bytes",
    )
    for row in rows:
        for metric in ("consensus_round_ms", "wallet_to_finality_ms"):
            samples = [
                float(iteration[metric])
                for window in row["windows"]
                for iteration in BASE.read_json(
                    lane_root / window["normalized_report"]
                )["iterations"]
            ]
            row.setdefault("aggregate", {})[metric] = BASE.distribution_summary(samples)
        row["resource_variance"] = {
            field: BASE.distribution_summary(
                [float(window["resources"][field]) for window in row["windows"]]
            )
            for field in resource_fields
        }


def prefix_report_references(
    rows: list[dict[str, Any]], lane_root: Path, campaign_root: Path
) -> None:
    prefix = lane_root.relative_to(campaign_root)
    for row in rows:
        for window in row["windows"]:
            for field in ("normalized_report", "resource_samples"):
                relative = Path(str(window[field]))
                window[field] = (prefix / relative).as_posix()


def run_lane(
    *,
    campaign_root: Path,
    lane: str,
    node_bin: Path,
    source_revision: str,
    development_smoke: bool,
    shared_validator_key_file: Path | None,
) -> dict[str, Any]:
    lane_root = campaign_root / "lanes" / lane
    lane_root.mkdir(parents=True)
    (lane_root / "snapshots").mkdir()
    (lane_root / "receipts").mkdir()
    (lane_root / "normalized").mkdir()

    storage_activation_height = None if lane == "legacy-jsonl" else 1
    base_port, rpc_base_port = BASE.SHARED.find_ports()
    seed, current_snapshot, wallet_key, wallet_address, recipient, topology = (
        BASE.setup_seed(
            node_bin,
            lane_root,
            base_port,
            rpc_base_port,
            storage_activation_height=storage_activation_height,
            validator_key_file=shared_validator_key_file,
        )
    )
    validator_identities = validator_public_identities(seed / "validator_keys.json")
    binary_build = BASE.require_release_binary_identity(
        node_bin,
        seed,
        source_revision,
    )

    heights = [2] if development_smoke else BASE.HEIGHTS
    windows_per_height = 1 if development_smoke else BASE.WINDOWS_PER_HEIGHT
    rounds_per_window = 1 if development_smoke else BASE.ROUNDS_PER_WINDOW
    current_height = 1
    rows: list[dict[str, Any]] = []
    for target_height in heights:
        if current_height < target_height:
            advance_rounds = target_height - current_height
            advance, current_snapshot = BASE.run_rounds(
                node_bin=node_bin,
                root=lane_root,
                seed=seed,
                topology=topology,
                source_snapshot=current_snapshot,
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                label=f"advance-{current_height}-to-{target_height}",
                rounds=advance_rounds,
                storage_lane=lane,
            )
            current_height = int(advance["final_height"])
        if current_height != target_height:
            raise RuntimeError(f"{lane} snapshot height drifted")

        base_snapshot = current_snapshot
        windows: list[dict[str, Any]] = []
        first_result_snapshot: Path | None = None
        for window_index in range(windows_per_height):
            result, result_snapshot = BASE.run_rounds(
                node_bin=node_bin,
                root=lane_root,
                seed=seed,
                topology=topology,
                source_snapshot=base_snapshot,
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                label=f"height-{target_height}-window-{window_index + 1}",
                rounds=rounds_per_window,
                storage_lane=lane,
            )
            windows.append(result)
            if window_index == 0:
                first_result_snapshot = result_snapshot
        if first_result_snapshot is None:
            raise RuntimeError(f"{lane} height produced no measurement window")
        current_snapshot = first_result_snapshot
        current_height = target_height + rounds_per_window
        rows.append({"height": target_height, "windows": windows})

    aggregate_rows(rows, lane_root)
    relationship_models = (
        {} if development_smoke else BASE.height_relationship_models(rows, lane_root)
    )
    no_positive_linear_relationship = (
        None
        if development_smoke
        else all(
            model["material_positive_linear_relationship"] is False
            for model in relationship_models.values()
        )
    )
    comparison_windows_pass = all(
        window["literal_receipts_exact"] is True
        and int(window["validators_converged"]) == BASE.VALIDATORS
        and int(window["resources"]["foreground_process_count"])
        == rounds_per_window
        and int(window["resources"]["foreground_min_sample_count"]) >= 2
        for row in rows
        for window in row["windows"]
    )
    selected_storage_gates_pass = (
        None
        if lane != BASE.SELECTED_STORAGE_LANE
        else all(
            window["zero_full_history_reads"] is True
            and window["bounded_index_pages"] is True
            and window["constant_accumulator_work"] is True
            for row in rows
            for window in row["windows"]
        )
    )
    prefix_report_references(rows, lane_root, campaign_root)
    return {
        "lane": lane,
        "storage_behavior": STORAGE_BEHAVIORS[lane],
        "source_revision": source_revision,
        "node_binary_sha256": sha256(node_bin),
        "node_binary": node_bin.name,
        "node_binary_build": binary_build,
        "storage_activation_height": storage_activation_height,
        "chain_id": BASE.CHAIN_ID,
        "wallet_address": wallet_address,
        "recipient_address": recipient,
        "validator_public_identities": validator_identities,
        "topology_sha256": sha256(topology),
        "environment": {
            "cpu_affinity": sorted(os.sched_getaffinity(0)),
            "filesystem_device": lane_root.stat().st_dev,
            "filesystem_block_size_bytes": os.statvfs(lane_root).f_bsize,
        },
        "height_1_snapshot_sha256": BASE.directory_digest(
            lane_root / "snapshots" / "height-1.snapshot"
        ),
        "rows": rows,
        "comparison_windows_pass": comparison_windows_pass,
        "selected_storage_gates_pass": selected_storage_gates_pass,
        "height_relationship_model": {
            "schema": "postfiat-storage-height-cost-model-v2",
            "sample_kind": "per_window_p95",
            "relative_materiality": BASE.MODEL_RELATIVE_MATERIALITY,
            "residual_sigmas": BASE.MODEL_RESIDUAL_SIGMAS,
            "stages": relationship_models,
        },
        "no_positive_linear_height_relationship": no_positive_linear_relationship,
    }


def metric_p95(lane: dict[str, Any], row_index: int, metric: str) -> float:
    return float(lane["rows"][row_index]["aggregate"][metric]["p95"])


def host_description(campaign_root: Path) -> dict[str, Any]:
    uname = platform.uname()
    filesystem = os.statvfs(campaign_root)
    rustc = subprocess.run(
        ["rustc", "-Vv"],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    return {
        "system": uname.system,
        "kernel_release": uname.release,
        "machine": uname.machine,
        "logical_cpu_count": os.cpu_count(),
        "cpu_affinity": sorted(os.sched_getaffinity(0)),
        "page_size_bytes": os.sysconf("SC_PAGE_SIZE"),
        "campaign_root_device": campaign_root.stat().st_dev,
        "filesystem_block_size_bytes": filesystem.f_bsize,
        "clock_ticks_per_second": os.sysconf("SC_CLK_TCK"),
        "python": platform.python_version(),
        "rustc": rustc,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy-node-bin", type=Path, required=True)
    parser.add_argument("--legacy-source-revision", required=True)
    parser.add_argument("--bounded-node-bin", type=Path, required=True)
    parser.add_argument("--bounded-source-revision", required=True)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--expected-source-revision", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--development-smoke",
        action="store_true",
        help="run one measured round per lane; output is never evidence eligible",
    )
    args = parser.parse_args()

    revisions = {
        "legacy-jsonl": args.legacy_source_revision,
        "bounded-jsonl": args.bounded_source_revision,
        "selected-indexed": args.expected_source_revision,
    }
    for lane, revision in revisions.items():
        require_revision(revision, f"{lane} source revision")
    for lane, expected_revision in HISTORICAL_REVISIONS.items():
        if revisions[lane] != expected_revision:
            raise ValueError(
                f"{lane} must use the frozen source revision {expected_revision}"
            )
    if len(set(revisions.values())) != len(revisions):
        raise ValueError("paired lanes must bind three distinct source revisions")
    current_revision = BASE.run_git_revision()
    if args.expected_source_revision != current_revision:
        raise ValueError("selected source revision does not match HEAD")
    for lane in ("legacy-jsonl", "bounded-jsonl"):
        if not revision_is_ancestor(revisions[lane], current_revision):
            raise ValueError(f"{lane} revision is not an ancestor of selected source")
    if not args.development_smoke and not BASE.git_is_clean():
        raise ValueError("paired release qualification requires a clean checkout")

    raw_root = args.output_dir.expanduser()
    if raw_root.is_symlink():
        raise ValueError("output path must not be a symlink")
    root = raw_root.resolve()
    if root.exists():
        raise ValueError(f"refusing to overwrite output directory: {root}")
    binaries = {
        "legacy-jsonl": release_binary(args.legacy_node_bin, "legacy binary"),
        "bounded-jsonl": release_binary(args.bounded_node_bin, "bounded binary"),
        "selected-indexed": release_binary(args.node_bin, "selected binary"),
    }
    if len({sha256(path) for path in binaries.values()}) != len(binaries):
        raise ValueError("paired lanes must bind three distinct release binaries")

    root.mkdir(parents=True)
    shared_private = root / "private"
    shared_private.mkdir(mode=0o700)
    shared_validator_key_file = shared_private / "validator_keys.json"
    lanes: dict[str, dict[str, Any]] = {}
    for lane in LANE_ORDER:
        lanes[lane] = run_lane(
            campaign_root=root,
            lane=lane,
            node_bin=binaries[lane],
            source_revision=revisions[lane],
            development_smoke=args.development_smoke,
            shared_validator_key_file=(
                shared_validator_key_file
                if shared_validator_key_file.exists()
                else None
            ),
        )
        if lane == "legacy-jsonl":
            source_keys = (
                root
                / "lanes"
                / lane
                / "private"
                / "seed"
                / "validator_keys.json"
            )
            shutil.copyfile(source_keys, shared_validator_key_file)
            shared_validator_key_file.chmod(0o600)

    validator_key_sets = {
        json.dumps(lane["validator_public_identities"], sort_keys=True)
        for lane in lanes.values()
    }
    if len(validator_key_sets) != 1:
        raise RuntimeError("paired lanes did not use the same validator keys")
    selected = lanes["selected-indexed"]
    legacy = lanes["legacy-jsonl"]
    baseline = (
        {}
        if args.development_smoke
        else {
            metric: metric_p95(legacy, 0, metric)
            for metric in ("consensus_round_ms", "wallet_to_finality_ms")
        }
    )
    ratios: dict[str, float] = {}
    if not args.development_smoke:
        for metric in ("consensus_round_ms", "wallet_to_finality_ms"):
            selected_50 = metric_p95(selected, 0, metric)
            selected_5000 = metric_p95(selected, -1, metric)
            ratios[f"{metric}_height50_vs_legacy"] = (
                selected_50 / baseline[metric]
            )
            ratios[f"{metric}_height5000_vs_height50"] = (
                selected_5000 / selected_50
            )

    comparison_windows_pass = all(
        lane["comparison_windows_pass"] is True for lane in lanes.values()
    )
    selected_window_gates_pass = selected["selected_storage_gates_pass"] is True
    source_worktree_clean = BASE.git_is_clean()
    release_gates_pass = (
        source_worktree_clean
        and comparison_windows_pass
        and selected_window_gates_pass
        and selected["no_positive_linear_height_relationship"] is True
        and all(value <= 1.10 for value in ratios.values())
    )
    status = (
        "DEVELOPMENT SMOKE PASS"
        if args.development_smoke and comparison_windows_pass and selected_window_gates_pass
        else "DEVELOPMENT SMOKE BLOCKED"
        if args.development_smoke
        else "PASS"
        if release_gates_pass
        else "PUBLIC TESTNET BLOCKED"
    )
    report = {
        "schema": SCHEMA,
        "status": status,
        "captured_at": datetime.now(timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z"),
        "campaign_mode": (
            "development-smoke"
            if args.development_smoke
            else "release-qualification"
        ),
        "evidence_eligible": (not args.development_smoke and release_gates_pass),
        "source_worktree_clean": source_worktree_clean,
        "source_revision": args.expected_source_revision,
        "node_binary_sha256": sha256(binaries["selected-indexed"]),
        "node_binary": binaries["selected-indexed"].name,
        "node_binary_build": selected["node_binary_build"],
        "spec_sha3_384": hashlib.sha3_384(SPEC.read_bytes()).hexdigest(),
        "paired_runner_sha256": sha256(Path(__file__).resolve()),
        "selected_runner_sha256": sha256(BASE_RUNNER),
        "shared_runner_sha256": sha256(BASE.SHARED_RUNNER),
        "validator_count": BASE.VALIDATORS,
        "windows_per_height": 1 if args.development_smoke else BASE.WINDOWS_PER_HEIGHT,
        "rounds_per_window": 1 if args.development_smoke else BASE.ROUNDS_PER_WINDOW,
        "timeout_ms": BASE.QUALIFICATION_TIMEOUT_MS,
        "lane_order": list(LANE_ORDER),
        "lanes": lanes,
        "legacy_height_50_baseline": baseline,
        "rows": selected["rows"],
        "ratios": ratios,
        "comparison_windows_pass": comparison_windows_pass,
        "window_gates_pass": selected_window_gates_pass,
        "height_relationship_model": selected["height_relationship_model"],
        "no_positive_linear_height_relationship": selected[
            "no_positive_linear_height_relationship"
        ],
        "pairing": {
            "same_host": True,
            "same_chain_id": len({lane["chain_id"] for lane in lanes.values()}) == 1,
            "same_validator_count": True,
            "same_validator_keys": len(validator_key_sets) == 1,
            "same_height_window_cardinality": True,
            "same_full_vote_policy": True,
            "same_timeout_policy": True,
            "same_host_allocation": len(
                {
                    tuple(lane["environment"]["cpu_affinity"])
                    for lane in lanes.values()
                }
            )
            == 1,
            "same_storage_medium": len(
                {
                    (
                        lane["environment"]["filesystem_device"],
                        lane["environment"]["filesystem_block_size_bytes"],
                    )
                    for lane in lanes.values()
                }
            )
            == 1,
            "same_wallet_and_recipient_accounts": len(
                {
                    (lane["wallet_address"], lane["recipient_address"])
                    for lane in lanes.values()
                }
            )
            == 1,
            "same_semantic_transfer_workload": True,
            "same_binary": False,
            "binary_policy": (
                "exact owning release binary for each historical storage behavior; "
                "all binaries and source revisions are hash-bound"
            ),
            "snapshot_policy": (
                "lane-native authenticated snapshots at identical starting heights; "
                "snapshot bytes differ where storage format or genesis activation differs"
            ),
            "signature_limit": (
                "ML-DSA signatures are randomized, so independently executed lane "
                "transactions and consensus hashes are not byte-identical"
            ),
        },
        "host": host_description(root),
        "claims_not_made": [
            *(
                ["release qualification or a height-50 legacy baseline"]
                if args.development_smoke
                else []
            ),
            "same binary across retired and selected storage implementations",
            "byte-identical cross-lane signatures, blocks, or state roots",
            "public WAN or devnet performance",
            "deployment authorization",
        ],
        "offline": True,
        "network_contacted": False,
        "devnet_queried_or_mutated": False,
    }
    write_json(root / "campaign-report.json", report)
    print(f"storage-scaling-paired-campaign={status}", flush=True)
    print(f"report={root / 'campaign-report.json'}", flush=True)
    return 0 if status in {"PASS", "DEVELOPMENT SMOKE PASS"} else 1


if __name__ == "__main__":
    raise SystemExit(main())
