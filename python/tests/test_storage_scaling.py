from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path

from postfiat_rpc.storage_scaling import (
    ARTIFACT_SCHEMAS,
    MANIFEST_FILE,
    MATERIAL_STAGE_PATHS,
    PERFORMANCE_RESOURCE_FIELDS,
    PERFORMANCE_STORAGE_BEHAVIORS,
    RESOURCE_SAMPLE_SCHEMA,
    RESOURCE_SAMPLE_TARGET_INTERVAL_MS,
    REQUIRED_TAMPER_CASES,
    REQUIRED_TAMPER_REASONS,
    StorageScalingVerificationError,
    serve_verified_packet,
    verify_packet,
)


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _write_checksums(packet: Path) -> None:
    files = sorted(
        path
        for path in packet.rglob("*")
        if path.is_file() and path.name != "SHA256SUMS.txt"
    )
    (packet / "SHA256SUMS.txt").write_text(
        "".join(
            f"{_sha256(path)}  {path.relative_to(packet).as_posix()}\n"
            for path in files
        ),
        encoding="utf-8",
    )


def _reference(packet: Path, path: Path) -> dict[str, str]:
    return {
        "path": path.relative_to(packet).as_posix(),
        "sha256": _sha256(path),
    }


def _constant_distribution(value: float, count: int) -> dict[str, float | int]:
    return {
        "count": count,
        "min": value,
        "p50": value,
        "p95": value,
        "p99": value,
        "max": value,
        "mean": value,
        "population_stddev": 0.0,
    }


def _signed_transfer_identity(
    transfer: dict[str, object],
) -> tuple[str, str]:
    unsigned = transfer["unsigned"]
    assert isinstance(unsigned, dict)
    canonical_unsigned = {
        field: unsigned[field]
        for field in (
            "chain_id",
            "genesis_hash",
            "protocol_version",
            "address_namespace",
            "transaction_kind",
            "signature_algorithm_id",
            "from",
            "to",
            "amount",
            "fee",
            "sequence",
        )
    }
    canonical_transfer = {
        "unsigned": canonical_unsigned,
        "algorithm_id": transfer["algorithm_id"],
        "public_key_hex": transfer["public_key_hex"],
        "signature_hex": transfer["signature_hex"],
    }
    signed_sha256 = hashlib.sha256(
        json.dumps(
            canonical_transfer,
            ensure_ascii=False,
            separators=(",", ":"),
        ).encode("utf-8")
    ).hexdigest()
    signing_bytes = (
        "postfiat.transfer.v1\n"
        f"chain_id={unsigned['chain_id']}\n"
        f"genesis_hash={unsigned['genesis_hash']}\n"
        f"protocol_version={unsigned['protocol_version']}\n"
        f"address_namespace={unsigned['address_namespace']}\n"
        f"transaction_kind={unsigned['transaction_kind']}\n"
        f"signature_algorithm_id={unsigned['signature_algorithm_id']}\n"
        f"from={unsigned['from']}\n"
        f"to={unsigned['to']}\n"
        f"amount={unsigned['amount']}\n"
        f"fee={unsigned['fee']}\n"
        f"sequence={unsigned['sequence']}\n"
        f"algorithm={transfer['algorithm_id']}\n"
        f"public_key={transfer['public_key_hex']}\n"
        f"signature={transfer['signature_hex']}\n"
    ).encode("utf-8")
    tx_id = hashlib.sha3_384(b"postfiat.tx_id.v1\x00" + signing_bytes).hexdigest()
    return tx_id, signed_sha256


def _passing_packet(packet: Path) -> None:
    packet.mkdir()
    digest96 = "a" * 96
    revision = "b" * 40
    binary = packet / "bin" / "postfiat-node"
    binary.parent.mkdir()
    binary.write_bytes(b"release-binary-identity")
    batch_builder_binary = packet / "bin" / "postfiat-storage-corpus-batches"
    batch_builder_binary.write_bytes(b"corpus-batch-builder-identity")
    rollback_binary = packet / "bin" / "postfiat-node-rollback"
    rollback_binary.write_bytes(b"older-compatible-release-binary-identity")
    incompatible_binary = packet / "bin" / "postfiat-node-incompatible"
    incompatible_binary.write_bytes(b"older-incompatible-release-binary-identity")
    binaries = [
        {"path": "bin/postfiat-node", "sha256": _sha256(binary)},
        {
            "path": "bin/postfiat-storage-corpus-batches",
            "sha256": _sha256(batch_builder_binary),
        },
        {
            "path": "bin/postfiat-node-rollback",
            "sha256": _sha256(rollback_binary),
        },
        {
            "path": "bin/postfiat-node-incompatible",
            "sha256": _sha256(incompatible_binary),
        },
    ]

    replay_receipts = []
    for height, source_kind in (
        (915, "quarantine_archive"),
        (924, "authenticated_history"),
    ):
        path = packet / "replay" / f"height-{height}.json"
        _write_json(
            path,
            {
                "schema": "postfiat-storage-replay-receipt-v1",
                "source_height": height,
                "source_kind": source_kind,
                "block_count": height,
                "commitment_mode": "legacy_below_storage_activation",
                "exact_replay": True,
                "full_replay_passed": True,
                "logical_rebuild_identical": True,
                "canonical_export_identical": True,
                "canonical_export_receipt": {
                    "schema": (
                        "postfiat-transactional-canonical-export-receipt-v1"
                    ),
                    "finalized_height": height,
                    "record_count": height,
                    "records_sha3_384": digest96,
                },
                "canonical_export_sha256": "e" * 64,
                "tip_hash": digest96,
                "state_root": digest96,
                "ordered_history_accumulator": digest96,
                "chain_id": "postfiat-wan-devnet-2",
                "genesis_hash": (
                    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
                    "21aff3ed334da07e150a7233a3e90a9"
                ),
                "source_revision": revision,
                "node_binary_sha256": _sha256(binary),
                "node_binary_build": {
                    "git_revision": revision[:8],
                    "profile": "release",
                },
            },
        )
        replay_receipts.append(_reference(packet, path))

    performance_corpora: dict[int, dict[str, object]] = {}
    for height_index, height in enumerate([50, 5000]):
        first_sequence = 10_000 + height_index * 100
        transfers: list[dict[str, object]] = []
        identities: list[tuple[str, str]] = []
        for transfer_index in range(50):
            transfer: dict[str, object] = {
                "unsigned": {
                    "chain_id": "postfiat-storage-scaling-local-v1",
                    "genesis_hash": "c" * 96,
                    "protocol_version": 1,
                    "address_namespace": "postfiat",
                    "transaction_kind": "transfer",
                    "signature_algorithm_id": "ML-DSA-65",
                    "from": "pf-test-wallet",
                    "to": "pf-test-recipient",
                    "amount": 10,
                    "fee": 1,
                    "sequence": first_sequence + transfer_index,
                },
                "algorithm_id": "ML-DSA-65",
                "public_key_hex": f"{transfer_index + 1:04x}",
                "signature_hex": f"{transfer_index + 101:04x}",
            }
            transfers.append(transfer)
            identities.append(_signed_transfer_identity(transfer))
        corpus_path = packet / "performance" / "corpora" / f"height-{height}.json"
        _write_json(
            corpus_path,
            {
                "schema": "postfiat-tx-latency-signed-transfer-corpus-v1",
                "transfers": transfers,
            },
        )
        performance_corpora[height] = {
            "path": corpus_path.relative_to(packet).as_posix(),
            "sha256": _sha256(corpus_path),
            "identities": identities,
            "first_sequence": first_sequence,
            "last_sequence": first_sequence + 49,
        }

    def performance_fleet(height: int, identity_seed: int) -> list[dict[str, object]]:
        return [
            {
                "node_id": f"validator-{validator}",
                "height": height,
                "tip": f"{identity_seed:096x}",
                "state_root": f"{identity_seed + 1:096x}",
            }
            for validator in range(6)
        ]

    def performance_storage_work(
        lane_name: str, *, apply_stage: bool
    ) -> dict[str, object]:
        transactional = {
            "read_transactions": 0,
            "write_transactions": 0,
            "committed_write_transactions": 0,
            "records_read": 0,
            "records_written": 0,
            "bytes_read": 0,
            "bytes_written": 0,
            "page_reads": 0,
            "page_writes": 0,
            "full_history_scans": 0,
            "full_history_records_read": 0,
            "full_history_bytes_read": 0,
            "durable_commit_micros": 0,
        }
        legacy = {
            "jsonl_append_calls": 0,
            "checkpoint_bytes_read": 0,
            "crash_suffix_bytes_read": 0,
            "crash_suffix_records_verified": 0,
            "legacy_prefix_bytes_read": 0,
            "legacy_prefix_records_verified": 0,
            "ordered_history_bytes_read": 0,
            "ordered_history_records_read": 0,
            "ordered_index_bitmap_bytes_read": 0,
            "ordered_index_bitmap_bytes_written": 0,
            "ordered_index_slots_read": 0,
            "ordered_index_slots_written": 0,
        }
        if lane_name == "selected-indexed":
            transactional.update(
                {
                    "read_transactions": 1,
                    "records_read": 1,
                    "bytes_read": 10,
                    "page_reads": 1,
                }
            )
            if apply_stage:
                transactional.update(
                    {
                        "write_transactions": 1,
                        "committed_write_transactions": 1,
                        "records_written": 1,
                        "bytes_written": 10,
                        "page_writes": 1,
                        "durable_commit_micros": 1,
                    }
                )
            transactional_value: dict[str, int] | None = transactional
        else:
            if apply_stage:
                legacy.update(
                    {
                        "jsonl_append_calls": 1,
                        "legacy_prefix_bytes_read": 10,
                        "legacy_prefix_records_verified": 1,
                    }
                )
            else:
                legacy.update(
                    {
                        "ordered_history_bytes_read": 10,
                        "ordered_history_records_read": 1,
                    }
                )
            transactional_value = None
        full_history_records = (
            (transactional_value or {}).get("full_history_records_read", 0)
            + legacy["crash_suffix_records_verified"]
            + legacy["legacy_prefix_records_verified"]
            + legacy["ordered_history_records_read"]
        )
        full_history_bytes = (
            (transactional_value or {}).get("full_history_bytes_read", 0)
            + legacy["checkpoint_bytes_read"]
            + legacy["crash_suffix_bytes_read"]
            + legacy["legacy_prefix_bytes_read"]
            + legacy["ordered_history_bytes_read"]
            + legacy["ordered_index_bitmap_bytes_read"]
        )
        return {
            "transactional": transactional_value,
            "legacy": legacy,
            "full_history_records_read": full_history_records,
            "full_history_bytes_read": full_history_bytes,
        }

    def performance_round_timings(lane_name: str) -> dict[str, object]:
        proposal_work = performance_storage_work(lane_name, apply_stage=False)
        apply_work = performance_storage_work(lane_name, apply_stage=True)
        return {
            "proposal_ms": 10.0,
            "verification_ms": 10.0,
            "vote_requests_ms": 10.0,
            "local_vote_ms": 10.0,
            "certificate_ms": 10.0,
            "local_apply_ms": 10.0,
            "certified_sends_ms": 10.0,
            "post_apply_status_ms": 10.0,
            "local_commit_publish_ms": 10.0,
            "proposal_breakdown": {"storage_work": proposal_work},
            "vote_request_targets": [
                {
                    "target": f"validator-{validator}",
                    "result": "ok",
                    "vote_request_breakdown": {
                        "remote_handling": {
                            "block_vote_breakdown": {
                                "storage_work": performance_storage_work(
                                    lane_name, apply_stage=False
                                )
                            }
                        }
                    },
                }
                for validator in range(1, 6)
            ],
            "local_apply_breakdown": {
                "write_commit_ms": 10.0,
                "storage_work": apply_work,
            },
            "certified_send_targets": [
                {
                    "target": f"validator-{validator}",
                    "result": "ok",
                    "storage_work": performance_storage_work(
                        lane_name, apply_stage=True
                    ),
                }
                for validator in range(1, 6)
            ],
        }

    def performance_rows(
        lane_name: str,
        consensus_base: float,
        wallet_base: float,
    ) -> list[dict[str, object]]:
        selected = lane_name == "selected-indexed"
        rows: list[dict[str, object]] = []
        heights = [50, 5000] if selected else [50]
        for height in heights:
            offset = 4 if height == 5000 else 0
            consensus = consensus_base + offset
            wallet = wallet_base + offset
            index = 1 if height == 5000 else 0
            corpus = performance_corpora[height]
            transaction_identities = corpus["identities"]
            assert isinstance(transaction_identities, list)
            windows = []
            for window_index in range(5):
                raw_path = (
                    packet
                    / "performance"
                    / lane_name
                    / f"height-{height}-window-{window_index + 1}.json"
                )
                initial_identity_seed = (index + 1) * 100
                result_identity_seed = initial_identity_seed + 10 + window_index * 2
                _write_json(
                    raw_path,
                    {
                        "schema": "postfiat-real-transaction-latency-benchmark-v1",
                        "status": "passed",
                        "config": {
                            "mode": "wallet-to-finality",
                            "build_mode": "release",
                            "transport": (
                                "local-loopback-persistent-validator-services"
                            ),
                            "validators": 6,
                            "rounds": 50,
                            "vote_policy": "full",
                            "timeout_ms": 900_000,
                            "amount": 10,
                            "wallet_address": "pf-test-wallet",
                            "recipient": "pf-test-recipient",
                            "input_source": "signed-transfer-corpus",
                            "signed_transfer_corpus": "$SIGNED_TRANSFER_CORPUS",
                            "signed_transfer_corpus_sha256": corpus["sha256"],
                            "signed_transfer_corpus_offset": 0,
                            "resident_transactional_store": selected,
                            "expected_start_height": height if selected else None,
                        },
                        "checks": {
                            "all_receipts_accepted": True,
                            "all_rounds_ok": True,
                            "all_transactions_final": True,
                            "all_vote_policies_match": True,
                            "converged": True,
                            "final_height_matches_rounds": True,
                            "iteration_count_matches_rounds": True,
                            "no_duplicate_receipts": True,
                            "state_verified_after_run": True,
                            "exact_input_binding": True,
                        },
                        "final_state": {
                            "height": height + 50,
                            "block_tip_hash": f"{result_identity_seed:096x}",
                            "state_root": f"{result_identity_seed + 1:096x}",
                            "state_verification_count": 6,
                        },
                        "iterations": [
                            {
                                "iteration": iteration_index,
                                "round_ok": True,
                                "receipt_accepted": True,
                                "finality_confirmed": True,
                                "all_sends_verified": True,
                                "all_vote_requests_verified": True,
                                "block_height": height + iteration_index,
                                "block_hash": f"{height + iteration_index:096x}",
                                "certificate_id": f"{height + iteration_index + 1:096x}",
                                "quorum": 5,
                                "input_source": "signed-transfer-corpus",
                                "signed_transfer_corpus_index": iteration_index - 1,
                                "tx_id": transaction_identities[iteration_index - 1][0],
                                "signed_transfer_sha256": transaction_identities[
                                    iteration_index - 1
                                ][1],
                                "consensus_round_ms": consensus,
                                "wallet_to_finality_ms": wallet,
                                "round_timings": performance_round_timings(
                                    lane_name
                                ),
                            }
                            for iteration_index in range(1, 51)
                        ],
                    },
                )
                resource_path = (
                    packet
                    / "performance"
                    / "resources"
                    / lane_name
                    / f"height-{height}-window-{window_index + 1}.json"
                )
                foreground_pids = list(range(1000, 1050))
                first_processes = {
                    str(pid): {
                        "cpu_ticks": 0,
                        "rss_kib": 20 if pid == foreground_pids[0] else 0,
                        "read_bytes": 0,
                        "write_bytes": 0,
                    }
                    for pid in foreground_pids
                }
                last_processes = {
                    str(pid): {
                        "cpu_ticks": 10 if pid == foreground_pids[0] else 0,
                        "rss_kib": 20 if pid == foreground_pids[0] else 0,
                        "read_bytes": 40 if pid == foreground_pids[0] else 0,
                        "write_bytes": 50 if pid == foreground_pids[0] else 0,
                    }
                    for pid in foreground_pids
                }
                _write_json(
                    resource_path,
                    {
                        "schema": RESOURCE_SAMPLE_SCHEMA,
                        "sample_target_interval_ms": (
                            RESOURCE_SAMPLE_TARGET_INTERVAL_MS
                        ),
                        "samples": [
                            {
                                "monotonic_offset_ns": 0,
                                "host_cpu_ticks": 100,
                                "host_memory": {
                                    "total_kib": 100,
                                    "available_kib": 90,
                                },
                                "network": {
                                    "received": 1000,
                                    "transmitted": 2000,
                                },
                                "node_disk_bytes": 100,
                                "processes": first_processes,
                            },
                            {
                                "monotonic_offset_ns": 1_000_000_000,
                                "host_cpu_ticks": 160,
                                "host_memory": {
                                    "total_kib": 100,
                                    "available_kib": 70,
                                },
                                "network": {
                                    "received": 1080,
                                    "transmitted": 2090,
                                },
                                "node_disk_bytes": 130,
                                "processes": last_processes,
                            },
                        ],
                        "foreground_processes": [
                            {
                                "pid": pid,
                                "started_offset_ns": 0,
                                "ended_offset_ns": 1_000_000_000,
                            }
                            for pid in foreground_pids
                        ],
                        "foreground_sample_counts": {
                            str(pid): 2 for pid in foreground_pids
                        },
                    },
                )
                transactional = {
                    "read_transactions": 0,
                    "write_transactions": 0,
                    "committed_write_transactions": 0,
                    "records_read": 0,
                    "records_written": 0,
                    "bytes_read": 0,
                    "bytes_written": 0,
                    "page_reads": 0,
                    "page_writes": 0,
                    "full_history_scans": 0,
                    "full_history_records_read": 0,
                    "full_history_bytes_read": 0,
                    "durable_commit_micros": 0,
                }
                legacy = {
                    "jsonl_append_calls": 0,
                    "checkpoint_bytes_read": 0,
                    "crash_suffix_bytes_read": 0,
                    "crash_suffix_records_verified": 0,
                    "legacy_prefix_bytes_read": 0,
                    "legacy_prefix_records_verified": 0,
                    "ordered_history_bytes_read": 0,
                    "ordered_history_records_read": 0,
                    "ordered_index_bitmap_bytes_read": 0,
                    "ordered_index_bitmap_bytes_written": 0,
                    "ordered_index_slots_read": 0,
                    "ordered_index_slots_written": 0,
                }
                if selected:
                    transactional.update(
                        {
                            "read_transactions": 600,
                            "write_transactions": 300,
                            "committed_write_transactions": 300,
                            "records_read": 600,
                            "records_written": 300,
                            "bytes_read": 6000,
                            "bytes_written": 3000,
                            "page_reads": 600,
                            "page_writes": 300,
                            "durable_commit_micros": 300,
                        }
                    )
                    full_history_records = 0
                    full_history_bytes = 0
                    fsync_count = 300
                else:
                    legacy.update(
                        {
                            "jsonl_append_calls": 300,
                            "legacy_prefix_bytes_read": 3000,
                            "legacy_prefix_records_verified": 300,
                            "ordered_history_bytes_read": 3000,
                            "ordered_history_records_read": 300,
                        }
                    )
                    full_history_records = 600
                    full_history_bytes = 6000
                    fsync_count = 300
                storage = {
                    **transactional,
                    "transactional": transactional,
                    "legacy": legacy,
                    "full_history_records_read": full_history_records,
                    "full_history_bytes_read": full_history_bytes,
                    "fsync_count": fsync_count,
                }
                resources = {
                    "cpu_ticks": 10,
                    "peak_rss_kib": 20,
                    "disk_growth_bytes": 30,
                    "bytes_read": 40,
                    "bytes_written": 50,
                    "page_reads": transactional["page_reads"],
                    "page_writes": transactional["page_writes"],
                    "fsync_count": fsync_count,
                    "fsync_micros": transactional["durable_commit_micros"],
                    "sample_count": 2,
                    "duration_ms": 1000.0,
                    "observed_pid_count": 50,
                    "foreground_process_count": 50,
                    "foreground_min_sample_count": 2,
                    "host_cpu_ticks": 60,
                    "host_total_memory_kib": 100,
                    "host_min_available_memory_kib": 70,
                    "network_received_bytes": 80,
                    "network_transmitted_bytes": 90,
                }
                windows.append(
                    {
                        "label": f"height-{height}-window-{window_index + 1}",
                        "storage_lane": lane_name,
                        "starting_height": height,
                        "rounds": 50,
                        "validators_converged": 6,
                        "literal_receipts_exact": True,
                        "backend_work_gate_pass": True,
                        "zero_full_history_reads": full_history_records == 0
                        and full_history_bytes == 0,
                        "bounded_index_pages": lane_name != "legacy-jsonl",
                        "constant_accumulator_work": lane_name != "legacy-jsonl",
                        "source_snapshot_sha256": (
                            f"{height + 10:064x}" if height == 50 else None
                        ),
                        "node_preparation_mode": (
                            "byte-verified-prepared-fleet-clone"
                            if selected
                            else "authenticated-portable-snapshot-import"
                        ),
                        "prepared_fleet_sha256": (
                            f"{height + 20:064x}" if selected else None
                        ),
                        "signed_transfer_corpus": corpus["path"],
                        "signed_transfer_corpus_sha256": corpus["sha256"],
                        "result_snapshot_sha256": (
                            None if selected else f"{result_identity_seed:064x}"
                        ),
                        "result_prepared_fleet_sha256": (
                            f"{result_identity_seed + 2:064x}" if selected else None
                        ),
                        "initial_fleet": performance_fleet(
                            height, initial_identity_seed
                        ),
                        "final_fleet": performance_fleet(
                            height + 50, result_identity_seed
                        ),
                        "final_height": height + 50,
                        "final_tip": f"{result_identity_seed:096x}",
                        "final_state_root": f"{result_identity_seed + 1:096x}",
                        "storage": storage,
                        "resources": resources,
                        "normalized_report": raw_path.relative_to(packet).as_posix(),
                        "normalized_report_sha256": _sha256(raw_path),
                        "resource_samples": resource_path.relative_to(packet).as_posix(),
                        "resource_samples_sha256": _sha256(resource_path),
                    }
                )
            resource_variance = {
                field: _constant_distribution(
                    float(windows[0]["resources"][field]), 5
                )
                for field in PERFORMANCE_RESOURCE_FIELDS
            }
            rows.append(
                {
                    "height": height,
                    "windows": windows,
                    "resource_variance": resource_variance,
                    "aggregate": {
                        "consensus_round_ms": {
                            "count": 250,
                            "min": consensus,
                            "p50": consensus,
                            "p95": consensus,
                            "p99": consensus,
                            "max": consensus,
                            "mean": consensus,
                            "population_stddev": 0.0,
                        },
                        "wallet_to_finality_ms": {
                            "count": 250,
                            "min": wallet,
                            "p50": wallet,
                            "p95": wallet,
                            "p99": wallet,
                            "max": wallet,
                            "mean": wallet,
                            "population_stddev": 0.0,
                        },
                    },
                }
            )
        return rows

    selected_rows = performance_rows("selected-indexed", 100.0, 105.0)
    legacy_rows = performance_rows("legacy-jsonl", 110.0, 115.0)

    e3_manifest_path = packet / "tamper" / "evidence" / "current-e3-manifest.json"
    _write_json(
        e3_manifest_path,
        {
            "schema": "postfiat-cobalt-adversarial-e3-campaign-manifest-v1",
            "campaign_id": "cobalt-e3-adversarial-recovery-v1",
            "source_revision": revision,
            "live_binding": {
                "validators": [f"validator-{index}" for index in range(6)],
                "quorum": 5,
            },
            "source_files": [
                {"path": path, "sha256": "1" * 64}
                for path in (
                    "docs/governance/cobalt-adversarial-verification-research-spec.md",
                    "crates/node/src/cobalt_shadow.rs",
                    "crates/node/src/cobalt_shadow_runtime.rs",
                    "crates/node/src/bin/postfiat_cobalt_liveness_simulation.rs",
                    "crates/cobalt_e3_harness/src/main.rs",
                )
            ],
            "history_entry_count": 4,
            "tamper_cases": [
                "truncated",
                "padded",
                "reordered",
                "one_entry_modified",
            ],
            "forged_catch_up_cases": [
                "fabricated_transition",
                "wrong_root_certificate",
                "omitted_latest_update",
            ],
            "rebound_from": {
                "path": (
                    "benchmarks/cobalt-adversarial-verification/e3/"
                    "campaign-manifest.json"
                ),
                "sha256": (
                    "c23320d47d631efdd74c1e5c6c541951f452a4de9b14eb583f9d888b77167fa7"
                ),
                "policy": "same frozen cases and live binding, current source hashes",
            },
        },
    )
    e3_report_path = packet / "tamper" / "evidence" / "current-e3-report.json"
    _write_json(
        e3_report_path,
        {
            "schema": "postfiat-cobalt-adversarial-e3-campaign-v1",
            "summary": {
                "manifest_sha256": _sha256(e3_manifest_path),
                "source_revision": revision,
                "validator_count": 6,
                "tamper_case_count": 24,
                "forged_catch_up_case_count": 18,
                "recovery_case_count": 6,
                "rejected_case_count": 42,
                "durable_mutation_count": 0,
                "signed_evidence_count": 18,
                "signed_evidence_verified": True,
                "byte_identical_recovery_count": 6,
                "manual_repair_action_count": 0,
                "summary_only": False,
                "pass": True,
            },
            "cases": [
                {
                    "ok": True,
                    "detected_before_rejoin": True,
                    "durable_state_mutated": False,
                    "state_hash_before": "2" * 64,
                    "state_hash_after": "2" * 64,
                    "journal_sha256_before": "3" * 64,
                    "journal_sha256_after": "3" * 64,
                }
                for _ in range(42)
            ],
            "recoveries": [
                {
                    "ok": True,
                    "byte_identical": True,
                    "restart_succeeded": True,
                    "no_manual_repair": True,
                    "honest_history_sha256": "4" * 64,
                    "restored_history_sha256": "4" * 64,
                }
                for _ in range(6)
            ],
        },
    )

    rollback_revision = "c" * 40
    rollback_report_path = packet / "tamper" / "evidence" / "compatible-rollback.json"
    _write_json(
        rollback_report_path,
        {
            "schema": "postfiat-storage-compatible-rollback-v1",
            "status": "PASS",
            "evidence_eligible": True,
            "source_revision": revision,
            "current_binary": {
                "sha256": _sha256(binary),
                "git_revision": revision[:8],
                "source_revision": revision,
                "profile": "release",
            },
            "rollback_binary": {
                "sha256": _sha256(rollback_binary),
                "git_revision": rollback_revision[:8],
                "source_revision": rollback_revision,
                "profile": "release",
            },
            "rollback_source_is_ancestor": True,
            "chain_id": "postfiat-storage-scaling-local-v1",
            "validator_count": 6,
            "storage_activation_height": 1,
            "consensus_activation_height": 2,
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
                "current_post_activation": {
                    "height": 2,
                    "tip": digest96,
                    "state_root": digest96,
                },
                "rollback_resume_input": {
                    "height": 2,
                    "tip": digest96,
                    "state_root": digest96,
                },
                "rollback_finalized": {
                    "height": 3,
                    "tip": "c" * 96,
                    "state_root": "c" * 96,
                },
                "forward_resume_input": {
                    "height": 3,
                    "tip": "c" * 96,
                    "state_root": "c" * 96,
                },
                "forward_finalized": {
                    "height": 4,
                    "tip": "d" * 96,
                    "state_root": "d" * 96,
                },
            },
            "offline": True,
            "network_contacted": False,
            "devnet_queried_or_mutated": False,
        },
    )

    tamper_cases = []
    for name in sorted(REQUIRED_TAMPER_CASES):
        receipt_path = packet / "tamper" / f"{name}.json"
        test_receipts = [
            {
                "package": "postfiat-fixture",
                "test_filter": name,
                "executed_test_count": 1,
                "result": "passed",
                "command_sha256": "ab" * 32,
            },
            {
                "package": "postfiat-node",
                "test_filter": (
                    "ambiguous_active_transactional_state_blocks_vote_without_mutation"
                ),
                "executed_test_count": 1,
                "result": "passed",
                "command_sha256": "cd" * 32,
            },
        ]
        if name == "history_truncated":
            test_receipts.append(
                {
                    "package": "postfiat-cobalt-e3-harness",
                    "test_filter": "__full_campaign__",
                    "executed_case_count": 48,
                    "result": "passed",
                    "command_sha256": "de" * 32,
                    "verify_command_sha256": "ad" * 32,
                    "report": _reference(packet, e3_report_path),
                    "manifest": _reference(packet, e3_manifest_path),
                }
            )
        if name == "compatible_post_activation_software_rollback":
            test_receipts = [
                {
                    "package": "postfiat-storage-rollback-rehearsal",
                    "test_filter": name,
                    "executed_test_count": 1,
                    "result": "passed",
                    "command_sha256": "ef" * 32,
                    "report": _reference(packet, rollback_report_path),
                }
            ]
        terminal_state = (
            "recovered_new_tip"
            if name == "compatible_post_activation_software_rollback"
            else "rejected_voting_blocked"
        )
        receipt = {
            "schema": "postfiat-storage-tamper-receipt-v1",
            "name": name,
            "passed": True,
            "reason_code": REQUIRED_TAMPER_REASONS[name],
            "no_partial_mutation": True,
            "terminal_state": terminal_state,
            "source_revision": revision,
            "test_receipts": test_receipts,
            "offline": True,
            "network_contacted": False,
        }
        _write_json(receipt_path, receipt)
        tamper_cases.append(
            {
                **{key: value for key, value in receipt.items() if key != "schema"},
                "receipt": _reference(packet, receipt_path),
            }
        )

    height_model_observations = [
        {
            "height": height,
            "window": f"height-{height}-window-{window_index}",
            "p95_ms": 10.0,
        }
        for height in (50, 5000)
        for window_index in range(1, 6)
    ]
    height_model_predictions = [10.0 for _ in height_model_observations]
    height_model_residuals = [0.0 for _ in height_model_observations]
    constant_fit = {
        "intercept_ms": 10.0,
        "predictions_ms": height_model_predictions,
        "residuals_ms": height_model_residuals,
        "residual_rmse_ms": 0.0,
    }
    logarithmic_fit = {
        "intercept_ms": 10.0,
        "predictions_ms": height_model_predictions,
        "residuals_ms": height_model_residuals,
        "residual_rmse_ms": 0.0,
        "r_squared": 0.0,
        "slope_ms_per_log_height": 0.0,
    }
    linear_fit = {
        "intercept_ms": 10.0,
        "predictions_ms": height_model_predictions,
        "residuals_ms": height_model_residuals,
        "residual_rmse_ms": 0.0,
        "r_squared": 0.0,
        "slope_ms_per_height": 0.0,
    }
    height_relationship_stages = {
        stage: {
            "slope_ms_per_height": 0.0,
            "intercept_ms": 10.0,
            "predictions_ms": height_model_predictions,
            "residuals_ms": height_model_residuals,
            "residual_rmse_ms": 0.0,
            "r_squared": 0.0,
            "observations": height_model_observations,
            "fits": {
                "constant": constant_fit,
                "logarithmic": logarithmic_fit,
                "linear": linear_fit,
            },
            "preferred_fit_by_rmse": "constant",
            "sample_kind": "per_window_p95",
            "sample_count": 10,
            "height_50_window_p95_median_ms": 10.0,
            "max_same_height_window_range_ms": 0.0,
            "predicted_delta_50_to_5000_ms": 0.0,
            "material_threshold_ms": 1.0,
            "relative_materiality": 0.10,
            "residual_sigmas": 2.0,
            "material_positive_linear_relationship": False,
        }
        for stage in MATERIAL_STAGE_PATHS
    }

    migration_phase_contract = (
        ("legacy-finality", "transparent", ["accepted"]),
        (
            "cancelled-activation-scheduled",
            "governance",
            ["storage_commitment_activation_scheduled"],
        ),
        (
            "pre-activation-cancellation",
            "governance",
            ["storage_commitment_activation_cancelled"],
        ),
        ("post-cancellation-legacy-finality", "transparent", ["accepted"]),
        (
            "final-activation-scheduled",
            "governance",
            ["storage_commitment_activation_scheduled"],
        ),
        ("pre-activation-one", "transparent", ["accepted"]),
        ("pre-activation-two", "transparent", ["accepted"]),
        ("activation-finality", "transparent", ["accepted"]),
        ("post-activation-finality", "transparent", ["accepted"]),
        ("post-activation-forward-recovery", "transparent", ["accepted"]),
    )
    migration_phases = []
    for offset, (label, batch_kind, receipt_codes) in enumerate(
        migration_phase_contract,
        start=1,
    ):
        degraded = label == "activation-finality"
        phase = {
            "label": label,
            "height": 924 + offset,
            "batch_kind": batch_kind,
            "initial_applied_validator_count": 5 if degraded else 6,
            "applied_validator_count": 6,
            "certificate_validator_count": 6,
            "certificate_vote_count": 5 if degraded else 6,
            "certificate_quorum": 5,
            "receipt_accepted": True,
            "receipt_codes": receipt_codes,
            "certificate_id": f"{offset:096x}",
            "certificate_sha256": f"{offset + 20:064x}",
            "consensus_v2_commit": True,
            "transport_round_ok": True,
            "all_vote_requests_verified": True,
            "all_certified_sends_verified": not degraded,
            "failed_peer_targets": ["validator-4"] if degraded else [],
            "identity": {
                "height": 924 + offset,
                "tip": f"{offset + 100:096x}",
                "state_root": f"{offset + 200:096x}",
            },
            "batch_sha256": f"{offset + 40:064x}",
        }
        if degraded:
            phase.update(
                {
                    "catch_up_validator": "validator-4",
                    "catch_up_receipt_accepted": True,
                    "catch_up_receipt_count": 1,
                    "catch_up_receipt_codes": ["accepted"],
                }
            )
        migration_phases.append(phase)

    def migration_rebuild(
        height: int,
        packet_root: str,
        current_root: str,
        node_root: str,
        seed: int,
    ) -> dict[str, object]:
        return {
            "packet_root": packet_root,
            "manifest_sha256": f"{seed:064x}",
            "manifest_file_sha3_384": f"{seed:096x}",
            "current_state_root": current_root,
            "node_state_root": node_root,
            "required_disk_bytes": 1,
            "available_disk_bytes": 2,
            "logical_store_report": {
                "schema": "postfiat-storage-logical-integrity-v1",
                "backend": "redb",
                "storage_format": "postfiat-redb-v1",
                "finalized_height": height,
                "block_count": height,
                "archive_count": height,
                "ordered_batch_count": height,
                "receipt_count": height,
                "history_index_count": 0,
                "accumulator": f"{height:096x}",
            },
            "canonical_export_receipt": {
                "schema": "postfiat-transactional-canonical-export-receipt-v1",
                "finalized_height": height,
                "record_count": height,
                "records_sha3_384": f"{height + seed:096x}",
            },
            "rebuild_passed": True,
            "verify_only_passed": True,
            "generation_pointer_published": True,
        }

    packet_root = "c" * 96
    migration_clones = []
    for index in range(6):
        source_digest = f"{index + 1:064x}"
        migration_clones.append(
            {
                "validator_id": f"validator-{index}",
                "source_tree_sha256": source_digest,
                "backup_tree_sha256": source_digest,
                "backup_reverified_sha256": source_digest,
                "initial_migration": migration_rebuild(
                    924,
                    "d" * 96,
                    "e" * 96,
                    f"{1000 + index:096x}",
                    10 + index,
                ),
                "post_restart_refreeze": migration_rebuild(
                    925,
                    "f" * 96,
                    "1" * 96,
                    f"{2000 + index:096x}",
                    20 + index,
                ),
                "final_activation_refreeze": migration_rebuild(
                    928,
                    packet_root,
                    "2" * 96,
                    f"{3000 + index:096x}",
                    30 + index,
                ),
                "final_identity": migration_phases[-1]["identity"],
            }
        )
    restart_receipts = [
        {
            "validator_id": f"validator-{index}",
            "stopped_cleanly": True,
            "reopened_and_ready": True,
        }
        for index in range(6)
    ]
    incompatible_revision = "d" * 40
    cobalt_boundary = {
        "validator_registry_semantic_sha256": "3" * 64,
        "cobalt_governance_semantic_sha256": "4" * 64,
    }
    migration_report = {
        "schema": ARTIFACT_SCHEMAS["migration"],
        "status": "PASS",
        "evidence_eligible": True,
        "source_worktree_clean": True,
        "captured_at": "2026-08-26T00:00:00Z",
        "source_revision": revision,
        "node_binary_sha256": _sha256(binary),
        "node_binary_build": {
            "git_revision": revision[:8],
            "profile": "release",
        },
        "incompatible_binary": {
            "sha256": _sha256(incompatible_binary),
            "source_revision": incompatible_revision,
            "git_revision": incompatible_revision[:8],
            "profile": "release",
        },
        "source_height": 924,
        "chain_id": "postfiat-wan-devnet-2",
        "genesis_hash": (
            "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
            "21aff3ed334da07e150a7233a3e90a9"
        ),
        "exact_existing_chain_rehearsal": True,
        "clone_count": 6,
        "offline_rebuild": True,
        "second_logical_scan": True,
        "generation_pointer_published": True,
        "pre_activation_restart": True,
        "activation": True,
        "pre_activation_cancellation": True,
        "catch_up": True,
        "pre_activation_rollback": True,
        "post_activation_forward_recovery": True,
        "all_six_converged": True,
        "mixed_version_refused": True,
        "backup_verified": True,
        "disk_capacity_verified": True,
        "stop_conditions_verified": True,
        "stop_condition_receipt": {
            "schema": "postfiat-storage-source-stop-receipt-v1",
            "source_directory_count": 6,
            "processes_examined": 10,
            "unreadable_process_count": 0,
            "matching_process_count": 0,
        },
        "consensus_v2_unchanged": True,
        "cobalt_authority_unchanged": True,
        "literal_receipts_exact": True,
        "zero_post_activation_full_history_scans": True,
        "external_network_contacted": False,
        "loopback_transport_used": True,
        "devnet_queried_or_mutated": False,
        "identities": {
            "source_tip": digest96,
            "source_state_root": digest96,
            "packet_root": packet_root,
            "activation_id": "5" * 96,
            "cancelled_activation_id": "6" * 96,
            "cancellation_id": "7" * 96,
            "activation_tip": migration_phases[7]["identity"]["tip"],
            "activation_state_root": migration_phases[7]["identity"]["state_root"],
            "final_tip": migration_phases[-1]["identity"]["tip"],
            "final_state_root": migration_phases[-1]["identity"]["state_root"],
        },
        "cobalt_boundary": {
            "before": cobalt_boundary,
            "after": dict(cobalt_boundary),
        },
        "mixed_version_probe": {
            "exit_code": 1,
            "reason_code": "storage_unsupported_schema",
            "reason_detail": "transactional migration verification binding is invalid",
            "failure_output_sha256": "5" * 64,
            "artifact_absent": True,
            "binary_sha256": _sha256(incompatible_binary),
            "source_revision": incompatible_revision,
            "verifier_boundary": "v1 binary refused v2 migration generation",
        },
        "restart_receipts": {
            "pre_activation": restart_receipts,
            "scheduled_staggered": restart_receipts,
            "post_activation_forward": restart_receipts,
        },
        "phases": migration_phases,
        "clones": migration_clones,
    }

    def performance_lane(
        lane_name: str,
        rows: list[dict[str, object]],
    ) -> dict[str, object]:
        selected = lane_name == "selected-indexed"
        return {
            "lane": lane_name,
            "storage_behavior": PERFORMANCE_STORAGE_BEHAVIORS[lane_name],
            "source_revision": revision,
            "node_binary_sha256": _sha256(binary),
            "node_binary": binary.name,
            "node_binary_build": {
                "git_revision": revision[:8],
                "profile": "release",
            },
            "storage_backend_mode": {
                "legacy-jsonl": "legacy-jsonl",
                "selected-indexed": "transactional",
            }[lane_name],
            "storage_activation_height": 1,
            "chain_id": "postfiat-storage-scaling-local-v1",
            "wallet_address": "pf-test-wallet",
            "recipient_address": "pf-test-recipient",
            "validator_public_identities": [
                {
                    "node_id": f"validator-{index}",
                    "algorithm_id": "ML-DSA-65",
                    "public_key_sha256": f"{index + 10:064x}",
                }
                for index in range(6)
            ],
            "topology_sha256": "1" * 64,
            "environment": {
                "cpu_affinity": [0, 1],
                "filesystem_device": 1,
                "filesystem_block_size_bytes": 4096,
            },
            "height_1_snapshot_sha256": "2" * 64,
            "rows": rows,
            "comparison_windows_pass": True,
            "selected_storage_gates_pass": True if selected else None,
            "height_relationship_model": {
                "schema": "postfiat-storage-height-cost-model-v2",
                "sample_kind": "per_window_p95",
                "relative_materiality": 0.10,
                "residual_sigmas": 2.0,
                "stages": height_relationship_stages if selected else {},
            },
            "no_positive_linear_height_relationship": True if selected else None,
        }

    performance_lanes = {
        "selected-indexed": performance_lane("selected-indexed", selected_rows),
        "legacy-jsonl": performance_lane("legacy-jsonl", legacy_rows),
    }
    performance_ratios = {
        "consensus_round_ms_height50_vs_legacy": 100.0 / 110.0,
        "consensus_round_ms_height5000_vs_height50": 104.0 / 100.0,
        "wallet_to_finality_ms_height50_vs_legacy": 105.0 / 115.0,
        "wallet_to_finality_ms_height5000_vs_height50": 109.0 / 105.0,
    }

    reports = {
        "source": {
            "schema": ARTIFACT_SCHEMAS["source"],
            "git_revision": revision,
            "assembly_revision": revision,
            "spec_sha3_384": digest96,
            "binaries": binaries,
            "clean_checkout": True,
            "build_profile": "release",
        },
        "replay": {
            "schema": ARTIFACT_SCHEMAS["replay"],
            "quarantine_archive_blocks": 915,
            "authenticated_history_height": 924,
            "exact_pre_activation_replay": True,
            "full_replay_passed": True,
            "logical_rebuild_identical": True,
            "canonical_export_identical": True,
            "tip_hash": digest96,
            "state_root": digest96,
            "ordered_history_accumulator": digest96,
            "receipts": replay_receipts,
            "source_revision": revision,
            "node_binary_sha256": _sha256(binary),
            "node_binary_build": {
                "git_revision": revision[:8],
                "profile": "release",
            },
        },
        "performance": {
            "schema": ARTIFACT_SCHEMAS["performance"],
            "status": "PASS",
            "captured_at": "2026-08-27T00:00:00Z",
            "campaign_mode": "release-qualification",
            "qualification_profile": "time-budgeted-redb-v4",
            "evidence_eligible": True,
            "source_worktree_clean": True,
            "source_revision": revision,
            "runner_source_revision": revision,
            "node_binary_sha256": _sha256(binary),
            "batch_builder_binary_sha256": _sha256(batch_builder_binary),
            "batch_builder_binary": "postfiat-storage-corpus-batches",
            "batch_builder_build": {
                "git_revision": revision[:8],
                "profile": "release",
            },
            "node_binary_build": {
                "git_revision": revision[:8],
                "profile": "release",
            },
            "validator_count": 6,
            "windows_per_height": 5,
            "rounds_per_window": 50,
            "timeout_ms": 900_000,
            "max_wall_seconds": 14_400,
            "elapsed_wall_seconds": 120.0,
            "lane_order": ["selected-indexed", "legacy-jsonl"],
            "lane_height_matrix": [
                {"lane": "selected-indexed", "height": 50},
                {"lane": "selected-indexed", "height": 5000},
                {"lane": "legacy-jsonl", "height": 50},
            ],
            "lanes": performance_lanes,
            "materials_by_height": [
                {
                    "height": height,
                    "snapshot": (
                        f"canonical/snapshots/height-{height}.snapshot"
                        if height == 50
                        else None
                    ),
                    "snapshot_sha256": (
                        f"{height + 10:064x}" if height == 50 else None
                    ),
                    "prepared_fleet_sha256": f"{height + 20:064x}",
                    "corpus_source_mode": (
                        "authenticated-portable-snapshot-import"
                        if height == 50
                        else "disposable-canonical-prepared-fleet-clone"
                    ),
                    "corpus_source_prepared_fleet_sha256": (
                        None if height == 50 else f"{height + 20:064x}"
                    ),
                    "corpus_scratch_before_sha256": (
                        None if height == 50 else f"{height + 20:064x}"
                    ),
                    "corpus_scratch_after_sha256": (
                        None if height == 50 else f"{height + 31:064x}"
                    ),
                    "corpus_scratch_mutated": None if height == 50 else True,
                    "corpus_scratch_discarded": None if height == 50 else True,
                    "corpus_scratch_restored_sha256": (
                        None if height == 50 else f"{height + 20:064x}"
                    ),
                    "signed_transfer_corpus": performance_corpora[height]["path"],
                    "signed_transfer_corpus_sha256": performance_corpora[height][
                        "sha256"
                    ],
                    "transfer_count": 50,
                    "first_sequence": performance_corpora[height]["first_sequence"],
                    "last_sequence": performance_corpora[height]["last_sequence"],
                }
                for height in [50, 5000]
            ],
            "legacy_height_50_baseline": {
                "consensus_round_ms": 110.0,
                "wallet_to_finality_ms": 115.0,
            },
            "rows": selected_rows,
            "ratios": performance_ratios,
            "comparison_windows_pass": True,
            "window_gates_pass": True,
            "height_relationship_model": {
                "schema": "postfiat-storage-height-cost-model-v2",
                "sample_kind": "per_window_p95",
                "relative_materiality": 0.10,
                "residual_sigmas": 2.0,
                "stages": height_relationship_stages,
            },
            "no_positive_linear_height_relationship": True,
            "offline": True,
            "network_contacted": False,
            "devnet_queried_or_mutated": False,
            "host": {
                "cpu_affinity": [0, 1],
                "campaign_root_device": 1,
                "filesystem_block_size_bytes": 4096,
            },
            "pairing": {
                "shared_comparison_height": 50,
                "same_host": True,
                "same_source_revision": True,
                "same_binary": True,
                "same_chain_id": True,
                "same_validator_count": True,
                "same_validator_keys": True,
                "same_topology_file": True,
                "same_authenticated_snapshot_at_shared_height": True,
                "same_signed_transactions_at_shared_height": True,
                "same_wallet_and_recipient_accounts": True,
                "same_window_cardinality_at_shared_height": True,
                "same_full_vote_policy": True,
                "same_timeout_policy": True,
                "same_host_allocation": True,
                "same_storage_medium": True,
                "same_final_state_for_identical_inputs": True,
                "changed_input_at_shared_height": (
                    "authenticated node-local storage backend mode only"
                ),
            },
        },
        "tamper": {
            "schema": ARTIFACT_SCHEMAS["tamper"],
            "status": "PASS",
            "coverage_complete": True,
            "uncovered_requirements": [],
            "source_revision": revision,
            "cases": tamper_cases,
            "unique_test_count": len(REQUIRED_TAMPER_CASES) + 2,
            "offline": True,
            "network_contacted": False,
        },
        "migration": migration_report,
        "redaction": {
            "schema": ARTIFACT_SCHEMAS["redaction"],
            "passed": True,
            "allowed_nonlocal_ip_files": [],
        },
    }
    artifacts = {}
    for label, report in reports.items():
        path = packet / "artifacts" / f"{label}.json"
        _write_json(path, report)
        artifacts[label] = _reference(packet, path)

    manifest = {
        "schema": "postfiat-storage-scaling-evidence-packet-v1",
        "status": "PASS",
        "captured_at": "2026-08-26T00:00:00Z",
        "source": {
            "git_revision": revision,
            "assembly_revision": revision,
            "spec_sha3_384": digest96,
            "binaries": binaries,
        },
        "state_distinction": {
            label: {
                "exact_identifier": f"{label}-{digest96}",
                "observed_at": "2026-08-26T00:00:00Z",
                "freshness": "recorded evidence",
                **({"live_probe": False} if label == "live" else {}),
            }
            for label in ("live", "deployed", "repository")
        },
        "artifacts": artifacts,
    }
    _write_json(packet / MANIFEST_FILE, manifest)
    _write_checksums(packet)


def _rewrite_migration(
    packet: Path,
    mutation: Callable[[dict[str, object]], None],
) -> None:
    migration_path = packet / "artifacts" / "migration.json"
    migration = json.loads(migration_path.read_text(encoding="utf-8"))
    mutation(migration)
    _write_json(migration_path, migration)
    manifest_path = packet / MANIFEST_FILE
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["artifacts"]["migration"]["sha256"] = _sha256(migration_path)
    _write_json(manifest_path, manifest)
    _write_checksums(packet)


def _rewrite_performance(
    packet: Path,
    mutation: Callable[[dict[str, object]], None],
) -> None:
    performance_path = packet / "artifacts" / "performance.json"
    performance = json.loads(performance_path.read_text(encoding="utf-8"))
    mutation(performance)
    _write_json(performance_path, performance)
    manifest_path = packet / MANIFEST_FILE
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["artifacts"]["performance"]["sha256"] = _sha256(performance_path)
    _write_json(manifest_path, manifest)
    _write_checksums(packet)


def _rewrite_first_selected_resource_samples(
    packet: Path,
    mutation: Callable[[dict[str, object]], None],
) -> None:
    performance_path = packet / "artifacts" / "performance.json"
    performance = json.loads(performance_path.read_text(encoding="utf-8"))
    selected = performance["lanes"]["selected-indexed"]
    window = selected["rows"][0]["windows"][0]
    resource_path = packet / window["resource_samples"]
    resource_report = json.loads(resource_path.read_text(encoding="utf-8"))
    mutation(resource_report)
    _write_json(resource_path, resource_report)
    window["resource_samples_sha256"] = _sha256(resource_path)
    performance["rows"] = selected["rows"]
    _write_json(performance_path, performance)
    manifest_path = packet / MANIFEST_FILE
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["artifacts"]["performance"]["sha256"] = _sha256(performance_path)
    _write_json(manifest_path, manifest)
    _write_checksums(packet)


def _rewrite_first_selected_normalized_report(
    packet: Path,
    mutation: Callable[[dict[str, object]], None],
) -> None:
    performance_path = packet / "artifacts" / "performance.json"
    performance = json.loads(performance_path.read_text(encoding="utf-8"))
    selected = performance["lanes"]["selected-indexed"]
    window = selected["rows"][0]["windows"][0]
    normalized_path = packet / window["normalized_report"]
    normalized = json.loads(normalized_path.read_text(encoding="utf-8"))
    mutation(normalized)
    _write_json(normalized_path, normalized)
    window["normalized_report_sha256"] = _sha256(normalized_path)
    performance["rows"] = selected["rows"]
    _write_json(performance_path, performance)
    manifest_path = packet / MANIFEST_FILE
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["artifacts"]["performance"]["sha256"] = _sha256(performance_path)
    _write_json(manifest_path, manifest)
    _write_checksums(packet)


class StorageScalingVerifierTests(unittest.TestCase):
    def packet_dir(self, temporary: str) -> Path:
        return Path(temporary) / "packet"

    def test_storage_scaling_packet_verifies_offline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            verified = verify_packet(packet)
            self.assertIs(verified.report["verified"], True)
            self.assertIs(verified.report["offline"], True)
            self.assertIs(verified.report["live_probe_performed"], False)
            self.assertEqual(
                verified.report["tamper_case_count"], len(REQUIRED_TAMPER_CASES)
            )

    def test_storage_scaling_packet_rejects_nonoffline_performance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def claim_network_contact(performance: dict[str, object]) -> None:
                performance["network_contacted"] = True

            _rewrite_performance(packet, claim_network_contact)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "execution mode is not offline-only",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_missing_performance_lane(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def remove_lane(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                lanes.pop("legacy-jsonl")

            _rewrite_performance(packet, remove_lane)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "exactly two lanes",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_hard_coded_legacy_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_baseline(performance: dict[str, object]) -> None:
                baseline = performance["legacy_height_50_baseline"]
                assert isinstance(baseline, dict)
                baseline["consensus_round_ms"] = 999.0

            _rewrite_performance(packet, change_baseline)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "does not derive from the raw lane",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_historical_telemetry_overclaim(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def invent_gate(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                legacy = lanes["legacy-jsonl"]
                assert isinstance(legacy, dict)
                rows = legacy["rows"]
                assert isinstance(rows, list)
                rows[0]["windows"][0]["zero_full_history_reads"] = True

            _rewrite_performance(packet, invent_gate)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "full-history summary differs",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unbound_performance_lane_binary(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def swap_binary(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                legacy = lanes["legacy-jsonl"]
                assert isinstance(legacy, dict)
                legacy["node_binary_sha256"] = "f" * 64

            _rewrite_performance(packet, swap_binary)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "legacy-jsonl binary identity is unbound",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unfrozen_historical_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_source(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                legacy = lanes["legacy-jsonl"]
                assert isinstance(legacy, dict)
                legacy["source_revision"] = "0" * 40

            _rewrite_performance(packet, change_source)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "source revision differs",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_mismatched_validator_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_key(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                legacy = lanes["legacy-jsonl"]
                assert isinstance(legacy, dict)
                identities = legacy["validator_public_identities"]
                assert isinstance(identities, list)
                identities[0]["public_key_sha256"] = "f" * 64

            _rewrite_performance(packet, change_key)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "same validator keys",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unpaired_window_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_snapshot(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                selected = lanes["selected-indexed"]
                assert isinstance(selected, dict)
                rows = selected["rows"]
                assert isinstance(rows, list)
                rows[0]["windows"][1]["source_snapshot_sha256"] = "f" * 64

            _rewrite_performance(packet, change_snapshot)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "did not use the shared frozen input",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unpaired_prepared_fleet(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_prepared_fleet(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                selected = lanes["selected-indexed"]
                assert isinstance(selected, dict)
                rows = selected["rows"]
                assert isinstance(rows, list)
                rows[0]["windows"][1]["prepared_fleet_sha256"] = "f" * 64

            _rewrite_performance(packet, change_prepared_fleet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "did not use the frozen prepared fleet",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_high_height_snapshot_dependency(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def add_snapshot(performance: dict[str, object]) -> None:
                materials = performance["materials_by_height"]
                assert isinstance(materials, list)
                high = materials[1]
                assert isinstance(high, dict)
                high["snapshot"] = "canonical/snapshots/height-5000.snapshot"
                high["snapshot_sha256"] = "f" * 64

            _rewrite_performance(packet, add_snapshot)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "snapshot/corpus binding is invalid",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unbound_corpus_source_fleet(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_corpus_source(performance: dict[str, object]) -> None:
                materials = performance["materials_by_height"]
                assert isinstance(materials, list)
                high = materials[1]
                assert isinstance(high, dict)
                high["corpus_source_prepared_fleet_sha256"] = "f" * 64

            _rewrite_performance(packet, change_corpus_source)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "snapshot/corpus binding is invalid",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_forged_scratch_discard(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def retain_scratch(performance: dict[str, object]) -> None:
                materials = performance["materials_by_height"]
                assert isinstance(materials, list)
                high = materials[1]
                assert isinstance(high, dict)
                high["corpus_scratch_discarded"] = False

            _rewrite_performance(packet, retain_scratch)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "snapshot/corpus binding is invalid",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_scratch_not_cloned_from_source(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_scratch_source(performance: dict[str, object]) -> None:
                materials = performance["materials_by_height"]
                assert isinstance(materials, list)
                high = materials[1]
                assert isinstance(high, dict)
                high["corpus_scratch_before_sha256"] = "f" * 64

            _rewrite_performance(packet, change_scratch_source)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "snapshot/corpus binding is invalid",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_selected_result_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def add_result_snapshot(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                selected = lanes["selected-indexed"]
                assert isinstance(selected, dict)
                rows = selected["rows"]
                assert isinstance(rows, list)
                rows[1]["windows"][0]["result_snapshot_sha256"] = "f" * 64

            _rewrite_performance(packet, add_result_snapshot)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "did not use the frozen prepared fleet",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unbound_signed_corpus(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_corpus(performance: dict[str, object]) -> None:
                snapshots = performance["materials_by_height"]
                assert isinstance(snapshots, list)
                snapshots[0]["signed_transfer_corpus_sha256"] = "f" * 64

            _rewrite_performance(packet, change_corpus)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "reference is not bound by the packet checksums",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_changed_signed_transaction(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_input(report: dict[str, object]) -> None:
                iterations = report["iterations"]
                assert isinstance(iterations, list)
                iterations[0]["signed_transfer_sha256"] = "f" * 64

            _rewrite_first_selected_normalized_report(packet, change_input)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "did not consume the bound signed corpus",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_raw_storage_work_summary_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_storage_work(report: dict[str, object]) -> None:
                iterations = report["iterations"]
                assert isinstance(iterations, list)
                round_timings = iterations[0]["round_timings"]
                storage_work = round_timings["proposal_breakdown"]["storage_work"]
                transactional = storage_work["transactional"]
                transactional["page_reads"] += 1

            _rewrite_first_selected_normalized_report(packet, change_storage_work)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "transactional storage summary differs from raw stage telemetry",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_false_same_binary_claim(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_pairing(performance: dict[str, object]) -> None:
                pairing = performance["pairing"]
                assert isinstance(pairing, dict)
                pairing["same_binary"] = False

            _rewrite_performance(packet, change_pairing)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "performance pairing same_binary must be true",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_timeout_policy_change(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_timeout(performance: dict[str, object]) -> None:
                performance["timeout_ms"] = 90_000

            _rewrite_performance(packet, change_timeout)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "timeout policy differs",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unpaired_host_allocation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_affinity(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                legacy = lanes["legacy-jsonl"]
                assert isinstance(legacy, dict)
                environment = legacy["environment"]
                assert isinstance(environment, dict)
                environment["cpu_affinity"] = [0]

            _rewrite_performance(packet, change_affinity)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "did not share one host allocation",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_cost_model_residual_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_residual(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                selected = lanes["selected-indexed"]
                assert isinstance(selected, dict)
                model = selected["height_relationship_model"]
                assert isinstance(model, dict)
                model["stages"]["proposal_ms"]["residuals_ms"][0] = 1.0

            _rewrite_performance(packet, change_residual)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "height relationship stage proposal_ms",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_resource_variance_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_variance(performance: dict[str, object]) -> None:
                lanes = performance["lanes"]
                assert isinstance(lanes, dict)
                selected = lanes["selected-indexed"]
                assert isinstance(selected, dict)
                rows = selected["rows"]
                assert isinstance(rows, list)
                rows[0]["resource_variance"]["cpu_ticks"]["mean"] = 999.0

            _rewrite_performance(packet, change_variance)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "resource variance cpu_ticks",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_raw_resource_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_cpu_samples(resource_report: dict[str, object]) -> None:
                samples = resource_report["samples"]
                assert isinstance(samples, list)
                processes = samples[-1]["processes"]
                assert isinstance(processes, dict)
                processes["1000"]["cpu_ticks"] = 999

            _rewrite_first_selected_resource_samples(packet, change_cpu_samples)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "sampled resource cpu_ticks",
            ):
                verify_packet(packet)

    def test_storage_scaling_redaction_allows_timing_field_name(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            _write_json(packet / "timing.json", {"private_key_decode_ms": 1.0})
            _write_checksums(packet)
            self.assertIs(verify_packet(packet).report["verified"], True)

    def test_storage_scaling_redaction_rejects_actual_private_key(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            _write_json(packet / "leak.json", {"private_key": "not-safe"})
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "sensitive material marker",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_development_migration_smoke(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def mark_development(migration: dict[str, object]) -> None:
                migration["status"] = "DEVELOPMENT SMOKE PASS"
                migration["evidence_eligible"] = False

            _rewrite_migration(packet, mark_development)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "not an evidence-eligible PASS",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_false_degraded_activation_claim(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def claim_full_delivery(migration: dict[str, object]) -> None:
                phases = migration["phases"]
                assert isinstance(phases, list)
                activation = phases[7]
                assert isinstance(activation, dict)
                activation["all_certified_sends_verified"] = True

            _rewrite_migration(packet, claim_full_delivery)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "activation-finality participation policy",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unbound_migration_binary(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def substitute_binary(migration: dict[str, object]) -> None:
                incompatible = migration["incompatible_binary"]
                assert isinstance(incompatible, dict)
                incompatible["sha256"] = "f" * 64

            _rewrite_migration(packet, substitute_binary)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "incompatible binary identity",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_conflated_binary_roles(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            rollback_digest = _sha256(packet / "bin" / "postfiat-node-rollback")

            def use_rollback_as_incompatible(migration: dict[str, object]) -> None:
                incompatible = migration["incompatible_binary"]
                assert isinstance(incompatible, dict)
                incompatible["sha256"] = rollback_digest

            _rewrite_migration(packet, use_rollback_as_incompatible)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "incompatible binary identity",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_mutable_migration_backup(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def change_backup(migration: dict[str, object]) -> None:
                clones = migration["clones"]
                assert isinstance(clones, list)
                clone = clones[0]
                assert isinstance(clone, dict)
                clone["backup_reverified_sha256"] = "f" * 64

            _rewrite_migration(packet, change_backup)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "immutable backup binding",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_active_migration_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def mark_source_active(migration: dict[str, object]) -> None:
                receipt = migration["stop_condition_receipt"]
                assert isinstance(receipt, dict)
                receipt["matching_process_count"] = 1

            _rewrite_migration(packet, mark_source_active)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "source stop receipt is invalid",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_migration_replay_identity_split(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)

            def split_source_tip(migration: dict[str, object]) -> None:
                identities = migration["identities"]
                assert isinstance(identities, dict)
                identities["source_tip"] = "f" * 96

            _rewrite_migration(packet, split_source_tip)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "disagrees with exact height-924 replay",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_checksum_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            manifest = packet / MANIFEST_FILE
            manifest.write_text(
                manifest.read_text(encoding="utf-8") + " ", encoding="utf-8"
            )
            with self.assertRaisesRegex(
                StorageScalingVerificationError, "checksum mismatch"
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_nonpassing_claim(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["status"] = "PUBLIC TESTNET BLOCKED"
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError, "status is not PASS"
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_raw_window_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            raw = (
                packet
                / "performance"
                / "selected-indexed"
                / "height-50-window-1.json"
            )
            report = json.loads(raw.read_text(encoding="utf-8"))
            report["iterations"][0]["round_ok"] = False
            _write_json(raw, report)
            performance_path = packet / "artifacts" / "performance.json"
            performance = json.loads(performance_path.read_text(encoding="utf-8"))
            performance["rows"][0]["windows"][0][
                "normalized_report_sha256"
            ] = _sha256(raw)
            performance["lanes"]["selected-indexed"]["rows"][0]["windows"][0][
                "normalized_report_sha256"
            ] = _sha256(raw)
            _write_json(performance_path, performance)
            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["artifacts"]["performance"]["sha256"] = _sha256(
                performance_path
            )
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "performance iteration round_ok",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_manifest_only_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest.pop("artifacts")
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError, "artifacts"
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unbound_binary_build(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            performance_path = packet / "artifacts" / "performance.json"
            performance = json.loads(performance_path.read_text(encoding="utf-8"))
            performance["node_binary_build"]["git_revision"] = "0" * 8
            _write_json(performance_path, performance)
            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["artifacts"]["performance"]["sha256"] = _sha256(
                performance_path
            )
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "embedded binary revision",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_invalid_canonical_export(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            receipt_path = packet / "replay" / "height-915.json"
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            receipt["canonical_export_receipt"]["record_count"] = 0
            _write_json(receipt_path, receipt)

            replay_path = packet / "artifacts" / "replay.json"
            replay = json.loads(replay_path.read_text(encoding="utf-8"))
            replay["receipts"][0]["sha256"] = _sha256(receipt_path)
            _write_json(replay_path, replay)

            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["artifacts"]["replay"]["sha256"] = _sha256(replay_path)
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "canonical export is invalid",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_missing_original_e3_campaign(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            name = "history_truncated"
            receipt_path = packet / "tamper" / f"{name}.json"
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            receipt["test_receipts"] = [
                test
                for test in receipt["test_receipts"]
                if test.get("test_filter") != "__full_campaign__"
            ]
            _write_json(receipt_path, receipt)

            tamper_path = packet / "artifacts" / "tamper.json"
            tamper = json.loads(tamper_path.read_text(encoding="utf-8"))
            case = next(value for value in tamper["cases"] if value["name"] == name)
            case["receipt"]["sha256"] = _sha256(receipt_path)
            _write_json(tamper_path, tamper)

            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["artifacts"]["tamper"]["sha256"] = _sha256(tamper_path)
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "omitted a required full-campaign",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_unbound_rollback_binary(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            report_path = (
                packet / "tamper" / "evidence" / "compatible-rollback.json"
            )
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["rollback_binary"]["sha256"] = "f" * 64
            _write_json(report_path, report)

            name = "compatible_post_activation_software_rollback"
            receipt_path = packet / "tamper" / f"{name}.json"
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            receipt["test_receipts"][0]["report"]["sha256"] = _sha256(
                report_path
            )
            _write_json(receipt_path, receipt)

            tamper_path = packet / "artifacts" / "tamper.json"
            tamper = json.loads(tamper_path.read_text(encoding="utf-8"))
            case = next(value for value in tamper["cases"] if value["name"] == name)
            case["receipt"]["sha256"] = _sha256(receipt_path)
            _write_json(tamper_path, tamper)

            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["artifacts"]["tamper"]["sha256"] = _sha256(tamper_path)
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "older rollback binary identity",
            ):
                verify_packet(packet)

    def test_storage_scaling_packet_rejects_zero_test_filter(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            name = sorted(REQUIRED_TAMPER_CASES)[0]
            receipt_path = packet / "tamper" / f"{name}.json"
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            receipt["test_receipts"][0]["executed_test_count"] = 0
            _write_json(receipt_path, receipt)

            tamper_path = packet / "artifacts" / "tamper.json"
            tamper = json.loads(tamper_path.read_text(encoding="utf-8"))
            case = next(value for value in tamper["cases"] if value["name"] == name)
            case["receipt"]["sha256"] = _sha256(receipt_path)
            _write_json(tamper_path, tamper)

            manifest_path = packet / MANIFEST_FILE
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["artifacts"]["tamper"]["sha256"] = _sha256(tamper_path)
            _write_json(manifest_path, manifest)
            _write_checksums(packet)
            with self.assertRaisesRegex(
                StorageScalingVerificationError,
                "executed zero tests",
            ):
                verify_packet(packet)

    def test_storage_scaling_browser_refuses_non_loopback_bind(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            packet = self.packet_dir(temporary)
            _passing_packet(packet)
            verified = verify_packet(packet)
            with self.assertRaisesRegex(StorageScalingVerificationError, "loopback"):
                serve_verified_packet(verified, "0.0.0.0", 0)


if __name__ == "__main__":
    unittest.main()
