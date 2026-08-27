#!/usr/bin/env python3
"""Run one-binary, one-snapshot storage backend comparisons locally.

Every lane uses the exact same release binary, authenticated source snapshot,
validator keys, topology, accounts, signed transaction corpus, host allocation,
storage medium, full-vote policy, and timeout policy. The only changed input is
the authenticated node-local storage backend selector. No external network or
controlled-devnet endpoint is contacted.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import platform
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
BASE_RUNNER = REPO / "benchmarks" / "storage-scaling" / "run_campaign.py"
SPEC = REPO / "docs" / "architecture" / "storage-scaling-fix-spec.md"
LANE_ORDER = ("legacy-jsonl", "bounded-jsonl", "selected-indexed")
STORAGE_BEHAVIORS = {
    "legacy-jsonl": "authenticated JSONL with full-prefix append verification and full ordered-history proposal work",
    "bounded-jsonl": "authenticated JSONL v2 heads with the fixed-slot ordered index",
    "selected-indexed": "transactional redb finality path with the fixed-size accumulator",
}
SCHEMA = "postfiat-storage-scaling-paired-six-validator-campaign-v3"


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


def prepare_runner_root(root: Path, *, seed_root: bool = False) -> None:
    root.mkdir(parents=True)
    names = ["snapshots", "receipts", "normalized"]
    if not seed_root:
        names.append("logs")
    for name in names:
        (root / name).mkdir()


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
            corpus = Path(str(window["signed_transfer_corpus"]))
            window["signed_transfer_corpus"] = corpus.relative_to(
                campaign_root
            ).as_posix()


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


def window_transaction_identities(
    lane_root: Path, window: dict[str, Any]
) -> tuple[tuple[str, str], ...]:
    report = BASE.read_json(lane_root / str(window["normalized_report"]))
    iterations = report.get("iterations")
    if not isinstance(iterations, list):
        raise RuntimeError("normalized benchmark report omitted iterations")
    identities: list[tuple[str, str]] = []
    for iteration in iterations:
        if not isinstance(iteration, dict):
            raise RuntimeError("normalized benchmark iteration is malformed")
        tx_id = str(iteration.get("tx_id", ""))
        signed_sha256 = str(iteration.get("signed_transfer_sha256", ""))
        if not tx_id or len(signed_sha256) != 64:
            raise RuntimeError("normalized benchmark iteration omitted exact input identity")
        identities.append((tx_id, signed_sha256))
    return tuple(identities)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--expected-source-revision", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--development-smoke",
        action="store_true",
        help="run one measured round per lane; output is never evidence eligible",
    )
    args = parser.parse_args()

    require_revision(args.expected_source_revision, "source revision")
    current_revision = BASE.run_git_revision()
    if args.expected_source_revision != current_revision:
        raise ValueError("source revision does not match HEAD")
    if not args.development_smoke and not BASE.git_is_clean():
        raise ValueError("paired release qualification requires a clean checkout")

    raw_root = args.output_dir.expanduser()
    if raw_root.is_symlink():
        raise ValueError("output path must not be a symlink")
    root = raw_root.resolve()
    if root.exists():
        raise ValueError(f"refusing to overwrite output directory: {root}")
    node_bin = release_binary(args.node_bin, "qualification binary")

    root.mkdir(parents=True)
    corpora_root = root / "corpora"
    corpora_root.mkdir()
    shared_root = root / "shared"
    canonical_root = root / "canonical"
    prepare_runner_root(shared_root, seed_root=True)
    prepare_runner_root(canonical_root)
    lane_roots = {lane: root / "lanes" / lane for lane in LANE_ORDER}
    for lane_root in lane_roots.values():
        prepare_runner_root(lane_root)

    base_port, rpc_base_port = BASE.SHARED.find_ports()
    seed, current_snapshot, wallet_key, wallet_address, recipient, topology = (
        BASE.setup_seed(
            node_bin,
            shared_root,
            base_port,
            rpc_base_port,
            storage_activation_height=BASE.STORAGE_ACTIVATION_HEIGHT,
        )
    )
    binary_build = BASE.require_release_binary_identity(
        node_bin,
        seed,
        args.expected_source_revision,
    )
    validator_identities = validator_public_identities(seed / "validator_keys.json")

    heights = [2] if args.development_smoke else BASE.HEIGHTS
    windows_per_height = 1 if args.development_smoke else BASE.WINDOWS_PER_HEIGHT
    rounds_per_window = 1 if args.development_smoke else BASE.ROUNDS_PER_WINDOW
    current_height = 1
    lane_rows: dict[str, list[dict[str, Any]]] = {
        lane: [] for lane in LANE_ORDER
    }
    snapshots_by_height: list[dict[str, Any]] = []
    exact_pairing_verified = True

    for target_height in heights:
        if current_height < target_height:
            advance_rounds = target_height - current_height
            advance_label = f"advance-{current_height}-to-{target_height}"
            advance_corpus = corpora_root / f"{advance_label}.json"
            BASE.create_signed_transfer_corpus(
                node_bin=node_bin,
                source_snapshot=current_snapshot,
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                count=advance_rounds,
                output_file=advance_corpus,
                logs=canonical_root / "logs",
                label=advance_label,
            )
            advance, current_snapshot = BASE.run_rounds(
                node_bin=node_bin,
                root=canonical_root,
                seed=seed,
                topology=topology,
                source_snapshot=current_snapshot,
                wallet_key=wallet_key,
                wallet_address=wallet_address,
                recipient=recipient,
                signed_transfer_corpus=advance_corpus,
                label=advance_label,
                rounds=advance_rounds,
                storage_lane=BASE.SELECTED_STORAGE_LANE,
            )
            current_height = int(advance["final_height"])
        if current_height != target_height:
            raise RuntimeError(
                f"canonical snapshot height drifted: {current_height} != {target_height}"
            )

        source_snapshot = current_snapshot
        source_snapshot_sha256 = BASE.directory_digest(source_snapshot)
        measurement_corpus = corpora_root / f"height-{target_height}.json"
        corpus_report = BASE.create_signed_transfer_corpus(
            node_bin=node_bin,
            source_snapshot=source_snapshot,
            wallet_key=wallet_key,
            wallet_address=wallet_address,
            recipient=recipient,
            count=rounds_per_window,
            output_file=measurement_corpus,
            logs=canonical_root / "logs",
            label=f"height-{target_height}",
        )
        corpus_sha256 = sha256(measurement_corpus)
        snapshots_by_height.append(
            {
                "height": target_height,
                "snapshot": source_snapshot.relative_to(root).as_posix(),
                "snapshot_sha256": source_snapshot_sha256,
                "signed_transfer_corpus": measurement_corpus.relative_to(
                    root
                ).as_posix(),
                "signed_transfer_corpus_sha256": corpus_sha256,
                "transfer_count": rounds_per_window,
                "first_sequence": corpus_report["first_sequence"],
                "last_sequence": corpus_report["last_sequence"],
            }
        )

        first_result_snapshots: dict[str, Path] = {}
        results_at_height: dict[str, list[dict[str, Any]]] = {}
        for lane in LANE_ORDER:
            lane_root = lane_roots[lane]
            windows: list[dict[str, Any]] = []
            for window_index in range(windows_per_height):
                result, result_snapshot = BASE.run_rounds(
                    node_bin=node_bin,
                    root=lane_root,
                    seed=seed,
                    topology=topology,
                    source_snapshot=source_snapshot,
                    wallet_key=wallet_key,
                    wallet_address=wallet_address,
                    recipient=recipient,
                    signed_transfer_corpus=measurement_corpus,
                    label=(
                        f"height-{target_height}-window-{window_index + 1}"
                    ),
                    rounds=rounds_per_window,
                    storage_lane=lane,
                )
                if result["source_snapshot_sha256"] != source_snapshot_sha256:
                    raise RuntimeError(f"{lane} did not use the shared source snapshot")
                if result["signed_transfer_corpus_sha256"] != corpus_sha256:
                    raise RuntimeError(f"{lane} did not use the shared signed corpus")
                windows.append(result)
                if window_index == 0:
                    first_result_snapshots[lane] = result_snapshot
            results_at_height[lane] = windows
            lane_rows[lane].append({"height": target_height, "windows": windows})

        for window_index in range(windows_per_height):
            compared = [
                results_at_height[lane][window_index] for lane in LANE_ORDER
            ]
            if len({int(window["starting_height"]) for window in compared}) != 1:
                raise RuntimeError("paired lanes used different starting heights")
            if len({int(window["final_height"]) for window in compared}) != 1:
                raise RuntimeError("paired lanes finalized different heights")
            if len({str(window["final_state_root"]) for window in compared}) != 1:
                raise RuntimeError("paired lanes produced different final state roots")
            identity_sets = {
                window_transaction_identities(
                    lane_roots[lane], results_at_height[lane][window_index]
                )
                for lane in LANE_ORDER
            }
            if len(identity_sets) != 1:
                raise RuntimeError("paired lanes did not execute identical signed transactions")

        selected_snapshot = first_result_snapshots.get(BASE.SELECTED_STORAGE_LANE)
        if selected_snapshot is None:
            raise RuntimeError("selected lane produced no continuation snapshot")
        current_snapshot = selected_snapshot
        current_height = target_height + rounds_per_window

    lanes: dict[str, dict[str, Any]] = {}
    for lane in LANE_ORDER:
        lane_root = lane_roots[lane]
        rows = lane_rows[lane]
        aggregate_rows(rows, lane_root)
        relationship_models = (
            {}
            if args.development_smoke
            else BASE.height_relationship_models(rows, lane_root)
        )
        no_positive_linear_relationship = (
            None
            if args.development_smoke
            else all(
                model["material_positive_linear_relationship"] is False
                for model in relationship_models.values()
            )
        )
        comparison_windows_pass = all(
            window["literal_receipts_exact"] is True
            and window["backend_work_gate_pass"] is True
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
        prefix_report_references(rows, lane_root, root)
        lanes[lane] = {
            "lane": lane,
            "storage_behavior": STORAGE_BEHAVIORS[lane],
            "source_revision": args.expected_source_revision,
            "node_binary_sha256": sha256(node_bin),
            "node_binary": node_bin.name,
            "node_binary_build": binary_build,
            "storage_backend_mode": BASE.STORAGE_BACKEND_MODES[lane],
            "storage_activation_height": BASE.STORAGE_ACTIVATION_HEIGHT,
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
                shared_root / "snapshots" / "height-1.snapshot"
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

    selected = lanes[BASE.SELECTED_STORAGE_LANE]
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
        and exact_pairing_verified
        and comparison_windows_pass
        and selected_window_gates_pass
        and selected["no_positive_linear_height_relationship"] is True
        and all(value <= 1.10 for value in ratios.values())
    )
    status = (
        "DEVELOPMENT SMOKE PASS"
        if args.development_smoke
        and exact_pairing_verified
        and comparison_windows_pass
        and selected_window_gates_pass
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
        "node_binary_sha256": sha256(node_bin),
        "node_binary": node_bin.name,
        "node_binary_build": binary_build,
        "spec_sha3_384": hashlib.sha3_384(SPEC.read_bytes()).hexdigest(),
        "paired_runner_sha256": sha256(Path(__file__).resolve()),
        "selected_runner_sha256": sha256(BASE_RUNNER),
        "shared_runner_sha256": sha256(BASE.SHARED_RUNNER),
        "validator_count": BASE.VALIDATORS,
        "windows_per_height": windows_per_height,
        "rounds_per_window": rounds_per_window,
        "timeout_ms": BASE.QUALIFICATION_TIMEOUT_MS,
        "lane_order": list(LANE_ORDER),
        "lanes": lanes,
        "snapshots_by_height": snapshots_by_height,
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
            "same_source_revision": True,
            "same_binary": True,
            "same_chain_id": True,
            "same_validator_count": True,
            "same_validator_keys": True,
            "same_topology_file": True,
            "same_authenticated_snapshot_at_each_height": True,
            "same_signed_transactions": True,
            "same_wallet_and_recipient_accounts": True,
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
            "same_final_state_for_identical_inputs": exact_pairing_verified,
            "changed_input": "authenticated node-local storage backend mode only",
        },
        "host": host_description(root),
        "claims_not_made": [
            *(
                ["release qualification or a height-50 legacy baseline"]
                if args.development_smoke
                else []
            ),
            "byte-identical independently generated validator signatures or certificate IDs",
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
