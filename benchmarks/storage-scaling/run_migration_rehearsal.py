#!/usr/bin/env python3
"""Rehearse existing-chain storage activation on six immutable source clones.

The runner is fail closed and local-only. It never discovers a source, key,
host, or endpoint: the operator must provide six explicit stopped data
directories, an isolated split validator-key directory, and a funded workload
key. Raw output contains disposable private material and is never publishable;
only the final redaction-safe report may be copied into an evidence packet.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import stat
import subprocess
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable

VALIDATORS = 6
CONTROLLED_CHAIN_ID = "postfiat-wan-devnet-2"
CONTROLLED_GENESIS_HASH = (
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad255"
    "21aff3ed334da07e150a7233a3e90a9"
)
CONTROLLED_HEIGHT = 924
REPORT_SCHEMA = "postfiat-storage-scaling-six-clone-migration-v1"
MIGRATION_MANIFEST_SCHEMA = "postfiat-storage-migration-manifest-v2"
MIGRATION_REPORT_SCHEMA = "postfiat-storage-migration-report-v1"
MIGRATION_VERIFIER_VERSION = "postfiat.storage_verifier.v2"
HEX64 = frozenset("0123456789abcdef")
REPO = Path(__file__).resolve().parents[2]
CAMPAIGN_RUNNER = REPO / "benchmarks" / "storage-scaling" / "run_campaign.py"


def load_campaign_runner() -> Any:
    spec = importlib.util.spec_from_file_location(
        "postfiat_storage_migration_campaign_runner",
        CAMPAIGN_RUNNER,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load campaign runner: {CAMPAIGN_RUNNER}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


CAMPAIGN = load_campaign_runner()
SHARED = CAMPAIGN.SHARED


def write_json(path: Path, value: Any, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    path.chmod(mode)


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError(f"expected a JSON object: {path}")
    return value


def read_authenticated_json(path: Path) -> dict[str, Any]:
    raw = path.read_text(encoding="utf-8")
    value, _offset = json.JSONDecoder().raw_decode(raw)
    if not isinstance(value, dict):
        raise ValueError(f"expected an authenticated JSON object: {path}")
    return value


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def semantic_sha256(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def validate_regular_tree(root: Path, label: str) -> None:
    if not root.is_dir() or root.is_symlink():
        raise ValueError(f"{label} must be a regular directory")
    for path in root.rglob("*"):
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode) or not (stat.S_ISREG(mode) or stat.S_ISDIR(mode)):
            raise ValueError(f"{label} contains a symlink or special file: {path}")


def tree_sha256(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix().encode("utf-8")
        if path.is_dir():
            digest.update(b"D")
            digest.update(len(relative).to_bytes(8, "big"))
            digest.update(relative)
            continue
        digest.update(b"F")
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(path.stat().st_size.to_bytes(8, "big"))
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


def full_git_revision() -> str:
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    if len(revision) != 40 or any(character not in HEX64 for character in revision):
        raise ValueError("HEAD is not a full Git object ID")
    return revision


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


def verify_sources_stopped(sources: Iterable[Path]) -> dict[str, Any]:
    proc = Path("/proc")
    if not proc.is_dir():
        raise RuntimeError("source stop verification requires Linux procfs")
    source_paths = {source.resolve() for source in sources}
    processes_examined = 0
    unreadable_processes = 0
    matching_processes = 0
    for process in proc.iterdir():
        if not process.name.isdecimal():
            continue
        try:
            raw = (process / "cmdline").read_bytes()
        except (FileNotFoundError, ProcessLookupError):
            continue
        except PermissionError:
            unreadable_processes += 1
            continue
        if not raw:
            continue
        processes_examined += 1
        arguments = [os.fsdecode(value) for value in raw.split(b"\0") if value]
        data_dirs: list[str] = []
        for offset, argument in enumerate(arguments):
            if argument == "--data-dir" and offset + 1 < len(arguments):
                data_dirs.append(arguments[offset + 1])
            elif argument.startswith("--data-dir="):
                data_dirs.append(argument.split("=", 1)[1])
        for data_dir in data_dirs:
            if Path(data_dir).expanduser().resolve(strict=False) in source_paths:
                matching_processes += 1
    if unreadable_processes:
        raise RuntimeError("source stop verification could not inspect every user process")
    if matching_processes:
        raise RuntimeError("a source data directory is referenced by an active process")
    return {
        "schema": "postfiat-storage-source-stop-receipt-v1",
        "source_directory_count": len(source_paths),
        "processes_examined": processes_examined,
        "unreadable_process_count": unreadable_processes,
        "matching_process_count": matching_processes,
    }


class CommandRunner:
    def __init__(self, logs: Path) -> None:
        self.logs = logs
        self.logs.mkdir(parents=True)

    def run(self, command: list[str], label: str) -> subprocess.CompletedProcess[str]:
        completed = subprocess.run(
            command,
            cwd=REPO,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        (self.logs / f"{label}.stdout").write_text(completed.stdout, encoding="utf-8")
        (self.logs / f"{label}.stderr").write_text(completed.stderr, encoding="utf-8")
        if completed.returncode != 0:
            raise RuntimeError(
                f"{label} failed with exit {completed.returncode}; inspect the private logs"
            )
        return completed

    def json(self, command: list[str], label: str) -> dict[str, Any]:
        completed = self.run(command, label)
        value = json.loads(completed.stdout)
        if not isinstance(value, dict):
            raise RuntimeError(f"{label} did not emit a JSON object")
        return value

    def receipts(self, command: list[str], label: str) -> list[dict[str, Any]]:
        completed = self.run(command, label)
        value = json.loads(completed.stdout)
        if not isinstance(value, list) or not value or not all(
            isinstance(receipt, dict) for receipt in value
        ):
            raise RuntimeError(f"{label} did not emit literal receipts")
        return value

    def expected_failure(
        self,
        command: list[str],
        label: str,
        reason_code: str,
        reason_detail: str,
    ) -> dict[str, Any]:
        completed = subprocess.run(
            command,
            cwd=REPO,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        (self.logs / f"{label}.stdout").write_text(completed.stdout, encoding="utf-8")
        (self.logs / f"{label}.stderr").write_text(completed.stderr, encoding="utf-8")
        if completed.returncode == 0:
            raise RuntimeError(f"{label} unexpectedly succeeded")
        combined = f"{completed.stdout}\n{completed.stderr}"
        expected_reason = f"{reason_code}: {reason_detail}".lower()
        if expected_reason not in combined.lower():
            raise RuntimeError(f"{label} failed without the exact expected reason")
        return {
            "exit_code": completed.returncode,
            "reason_code": reason_code,
            "reason_detail": reason_detail,
            "failure_output_sha256": hashlib.sha256(combined.encode("utf-8")).hexdigest(),
            "artifact_absent": True,
        }


def status(runner: CommandRunner, node_bin: Path, data_dir: Path, label: str) -> dict[str, Any]:
    return runner.json(
        [str(node_bin), "status", "--data-dir", str(data_dir)],
        label,
    )


def status_identity(value: dict[str, Any]) -> dict[str, Any]:
    return {
        "height": int(value["block_height"]),
        "tip": str(value["block_tip_hash"]),
        "state_root": str(value["state_root"]),
    }


def require_hex(value: Any, size: int, label: str) -> str:
    parsed = str(value)
    if len(parsed) != size or any(character not in HEX64 for character in parsed):
        raise RuntimeError(f"{label} is not lowercase {size}-hex")
    return parsed


def fleet_status(
    runner: CommandRunner,
    node_bin: Path,
    nodes: list[Path],
    label: str,
) -> list[dict[str, Any]]:
    return [
        status(runner, node_bin, node, f"{label}.validator-{index}")
        for index, node in enumerate(nodes)
    ]


def converged_identity(statuses: list[dict[str, Any]], label: str) -> dict[str, Any]:
    if len(statuses) != VALIDATORS:
        raise RuntimeError(f"{label} does not contain six validators")
    identities = {
        (
            int(value["block_height"]),
            str(value["block_tip_hash"]),
            str(value["state_root"]),
        )
        for value in statuses
    }
    if len(identities) != 1:
        raise RuntimeError(f"{label} did not converge")
    height, tip, state_root = next(iter(identities))
    require_hex(tip, 96, f"{label} tip")
    require_hex(state_root, 96, f"{label} state root")
    return {"height": height, "tip": tip, "state_root": state_root}


def choose_lagging_validator(
    runner: CommandRunner,
    node_bin: Path,
    node: Path,
    height: int,
) -> int:
    report = runner.json(
        [
            str(node_bin),
            "block-proposer",
            "--data-dir",
            str(node),
            "--height",
            str(height),
        ],
        "activation-catch-up.proposer",
    )
    proposer = str(report.get("proposer"))
    if proposer not in {f"validator-{index}" for index in range(VALIDATORS)}:
        raise RuntimeError("activation proposer report did not identify a fleet validator")
    proposer_index = int(proposer.removeprefix("validator-"))
    # Validator-0 is the local round process. Hold back a remote validator that
    # is not the deterministic proposer so the five-vote certificate can still
    # be formed and delivered before that validator catches up from the exact
    # certified artifact.
    return next(
        index
        for index in reversed(range(1, VALIDATORS))
        if index != proposer_index
    )


def binary_identity(
    runner: CommandRunner,
    node_bin: Path,
    source: Path,
    revision: str,
    label: str,
) -> dict[str, str]:
    value = status(runner, node_bin, source, f"{label}.status")
    if value.get("build_git_revision") != revision[:8]:
        raise RuntimeError(f"{label} embedded revision does not match the requested source")
    if value.get("build_profile") != "release":
        raise RuntimeError(f"{label} is not a release build")
    return {
        "sha256": sha256(node_bin),
        "source_revision": revision,
        "git_revision": revision[:8],
        "profile": "release",
    }


def cobalt_boundary(data_dir: Path) -> dict[str, str]:
    registry = read_authenticated_json(data_dir / "validator_registry.json")
    governance = read_authenticated_json(data_dir / "governance.json")
    cobalt_fields = {
        key: governance.get(key)
        for key in (
            "active_validator_count",
            "active_validators",
            "authority_mode",
            "validator_registry_updates",
            "cobalt_authority_transitions",
        )
    }
    return {
        "validator_registry_semantic_sha256": semantic_sha256(registry),
        "cobalt_governance_semantic_sha256": semantic_sha256(cobalt_fields),
    }


def validate_source_statuses(
    statuses: list[dict[str, Any]],
    expected_height: int,
    expected_chain_id: str,
    expected_genesis_hash: str,
) -> dict[str, Any]:
    identity = converged_identity(statuses, "source fleet")
    if identity["height"] != expected_height:
        raise RuntimeError(
            f"source fleet height {identity['height']} does not equal {expected_height}"
        )
    node_ids = {str(value.get("node_id")) for value in statuses}
    if node_ids != {f"validator-{index}" for index in range(VALIDATORS)}:
        raise RuntimeError("source fleet does not contain validator-0 through validator-5")
    for value in statuses:
        if value.get("chain_id") != expected_chain_id:
            raise RuntimeError("source fleet chain ID mismatch")
        if value.get("genesis_hash") != expected_genesis_hash:
            raise RuntimeError("source fleet genesis mismatch")
        if value.get("validator_count") != VALIDATORS:
            raise RuntimeError("source fleet validator count mismatch")
        storage = value.get("storage")
        if not isinstance(storage, dict):
            raise RuntimeError("source status omitted storage")
        if storage.get("transactional_active") is not False:
            raise RuntimeError("source fleet is already transactionally active")
        if storage.get("commitment_version") != "postfiat.replicated_state.v1":
            raise RuntimeError("source fleet is not in legacy commitment mode")
    return identity


def key_file(key_dir: Path, validator_id: str) -> Path:
    path = key_dir / f"{validator_id}.validator_keys.json"
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"missing regular split key file for {validator_id}")
    return path


def stage_validator_keys(
    runner: CommandRunner,
    node_bin: Path,
    nodes: list[Path],
    key_dir: Path,
) -> None:
    for index, node in enumerate(nodes):
        validator_id = f"validator-{index}"
        report = runner.json(
            [
                str(node_bin),
                "validator-key-stage",
                "--data-dir",
                str(node),
                "--source-key-file",
                str(key_file(key_dir, validator_id)),
                "--source-validator-id",
                validator_id,
                "--validator-id",
                validator_id,
                "--replace",
            ],
            f"keys.{validator_id}",
        )
        if report.get("registry_public_key_matched") is not True:
            raise RuntimeError(f"{validator_id} staged key did not match the registry")


def rebuild_generation(
    runner: CommandRunner,
    node_bin: Path,
    node: Path,
    generation: Path,
    identity: dict[str, Any],
    label: str,
) -> dict[str, Any]:
    rebuilt = runner.json(
        [
            str(node_bin),
            "storage-rebuild-transactional",
            "--data-dir",
            str(node),
            "--output-dir",
            str(generation),
            "--expected-tip",
            str(identity["tip"]),
            "--expected-state-root",
            str(identity["state_root"]),
            "--offline-confirmed",
        ],
        f"{label}.rebuild",
    )
    verified = runner.json(
        [
            str(node_bin),
            "storage-rebuild-transactional",
            "--data-dir",
            str(node),
            "--output-dir",
            str(generation),
            "--expected-tip",
            str(identity["tip"]),
            "--expected-state-root",
            str(identity["state_root"]),
            "--verify-only",
            "--offline-confirmed",
        ],
        f"{label}.verify",
    )
    for name, report, verify_only in (
        ("rebuild", rebuilt, False),
        ("verify", verified, True),
    ):
        if report.get("schema") != MIGRATION_REPORT_SCHEMA:
            raise RuntimeError(f"{label} {name} report schema mismatch")
        if report.get("verify_only") is not verify_only:
            raise RuntimeError(f"{label} {name} mode mismatch")
        source_tip = report.get("source_tip")
        if not isinstance(source_tip, dict) or int(source_tip.get("height", -1)) != identity["height"]:
            raise RuntimeError(f"{label} {name} source tip mismatch")
        if source_tip.get("block_hash") != identity["tip"]:
            raise RuntimeError(f"{label} {name} source hash mismatch")
        if source_tip.get("state_root") != identity["state_root"]:
            raise RuntimeError(f"{label} {name} state root mismatch")
        if report.get("migration_packet_root") != rebuilt.get("migration_packet_root"):
            raise RuntimeError(f"{label} {name} packet root mismatch")
    if rebuilt.get("published") is not True or verified.get("published") is not False:
        raise RuntimeError(f"{label} generation publication state mismatch")
    required = int(rebuilt.get("required_disk_bytes", -1))
    available = int(rebuilt.get("available_disk_bytes", -1))
    if required < 0 or available < required:
        raise RuntimeError(f"{label} disk-capacity gate failed")
    manifest_path = generation / "storage-migration-manifest.json"
    manifest = read_json(manifest_path)
    if manifest.get("schema") != MIGRATION_MANIFEST_SCHEMA:
        raise RuntimeError(f"{label} migration manifest schema mismatch")
    if manifest.get("verifier_version") != MIGRATION_VERIFIER_VERSION:
        raise RuntimeError(f"{label} migration verifier mismatch")
    if manifest.get("migration_packet_root") != rebuilt.get("migration_packet_root"):
        raise RuntimeError(f"{label} manifest packet root mismatch")
    require_hex(manifest.get("node_state_root"), 96, f"{label} node-state root")
    require_hex(manifest.get("current_state_root"), 96, f"{label} current-state root")
    return {
        "packet_root": require_hex(
            rebuilt.get("migration_packet_root"), 96, f"{label} packet root"
        ),
        "manifest_sha256": sha256(manifest_path),
        "manifest_file_sha3_384": hashlib.sha3_384(manifest_path.read_bytes()).hexdigest(),
        "current_state_root": str(manifest["current_state_root"]),
        "node_state_root": str(manifest["node_state_root"]),
        "required_disk_bytes": required,
        "available_disk_bytes": available,
        "logical_store_report": rebuilt["logical_store_report"],
        "canonical_export_receipt": rebuilt["canonical_export_receipt"],
        "rebuild_passed": True,
        "verify_only_passed": True,
        "generation_pointer_published": True,
    }


def require_shared_packet_root(rebuilds: Iterable[dict[str, Any]], label: str) -> str:
    roots = {str(report["packet_root"]) for report in rebuilds}
    if len(roots) != 1:
        raise RuntimeError(f"{label} did not derive one shared migration packet root")
    return next(iter(roots))


def make_transfer_batch(
    runner: CommandRunner,
    node_bin: Path,
    node: Path,
    key: Path,
    recipient: str,
    batch_file: Path,
    label: str,
) -> None:
    runner.json(
        [
            str(node_bin),
            "batch-transfer",
            "--data-dir",
            str(node),
            "--key-file",
            str(key),
            "--to",
            recipient,
            "--amount",
            "1",
            "--batch-file",
            str(batch_file),
        ],
        f"{label}.batch",
    )


def certify_and_apply(
    runner: CommandRunner,
    node_bin: Path,
    nodes: list[Path],
    key_dir: Path,
    topology: Path,
    service_root: Path,
    service_logs: Path,
    artifacts: Path,
    label: str,
    height: int,
    batch_kind: str,
    batch_file: Path,
    apply_indexes: Iterable[int] = range(VALIDATORS),
) -> dict[str, Any]:
    phase = artifacts / label
    phase.mkdir(parents=True)
    certificate_file = phase / "block-certificate.json"
    applied = list(apply_indexes)
    if not applied or applied[0] != 0 or applied != sorted(set(applied)):
        raise RuntimeError(f"{label} apply indexes must be ordered, unique, and include zero")
    if any(index < 0 or index >= VALIDATORS for index in applied):
        raise RuntimeError(f"{label} apply index is outside the six-validator fleet")
    if len(applied) not in (VALIDATORS - 1, VALIDATORS):
        raise RuntimeError(f"{label} must retain either full participation or five-vote quorum")

    processes = []
    handles = []
    try:
        # Validator-0 is the local peer-certified round process. Starting a
        # second service for that same data directory would contend for the
        # staged redb generation lock; only remote validators need listeners.
        for index in applied[1:]:
            process, process_handles = SHARED.start_validator(
                node_bin,
                service_root / "nodes",
                topology,
                service_root,
                service_logs,
                f"{label}.live-round",
                index,
            )
            processes.append(process)
            handles.append(process_handles)
        command = [
            str(node_bin),
            "transport-peer-certified-batch-round",
            "--data-dir",
            str(nodes[0]),
            "--topology",
            str(topology),
            "--batch-kind",
            batch_kind,
            "--batch-file",
            str(batch_file),
            "--key-file",
            str(nodes[0] / "validator_keys.json"),
            "--proposal-key-file",
            str(nodes[0] / "validator_keys.json"),
            "--artifact-dir",
            str(phase),
            "--height",
            str(height),
            "--timeout-ms",
            "30000" if len(applied) == VALIDATORS else "5000",
            "--send-retries",
            "2" if len(applied) == VALIDATORS else "0",
            "--retry-backoff-ms",
            "100",
        ]
        if len(applied) != VALIDATORS:
            command.append("--allow-peer-failures")
        round_report = runner.json(command, f"{label}.certify-and-apply")
    finally:
        SHARED.stop_validators(processes, handles)

    if round_report.get("schema") != "postfiat-transport-peer-certified-batch-round-v1":
        raise RuntimeError(f"{label} transport round schema mismatch")
    certification = round_report.get("certification")
    if not isinstance(certification, dict):
        raise RuntimeError(f"{label} transport round omitted certification")
    if int(certification.get("block_height", -1)) != height:
        raise RuntimeError(f"{label} transport certification height mismatch")
    if int(certification.get("vote_count", -1)) != len(applied):
        raise RuntimeError(f"{label} transport certification vote count mismatch")
    if round_report.get("round_ok") is not True:
        raise RuntimeError(f"{label} transport round did not pass")
    if round_report.get("local_apply_verified") is not True:
        raise RuntimeError(f"{label} local application was not verified")
    if int(round_report.get("local_receipt_count", 0)) <= 0:
        raise RuntimeError(f"{label} emitted no literal local receipt")
    if int(round_report.get("local_accepted_count", -1)) != int(
        round_report["local_receipt_count"]
    ) or int(round_report.get("local_rejected_count", -1)) != 0:
        raise RuntimeError(f"{label} local receipt was not literally accepted")
    finality = round_report.get("local_hot_finality")
    if not isinstance(finality, list) or len(finality) != int(
        round_report["local_receipt_count"]
    ):
        raise RuntimeError(f"{label} omitted hot-finality receipts")
    literal_receipts = []
    for report in finality:
        receipt = report.get("receipt") if isinstance(report, dict) else None
        if (
            report.get("confirmed") is not True
            or not isinstance(receipt, dict)
            or receipt.get("accepted") is not True
        ):
            raise RuntimeError(f"{label} hot-finality receipt was not accepted")
        literal_receipts.append(receipt)
    failure_targets = {
        str(failure.get("to"))
        for field in ("vote_request_failures", "send_failures")
        for failure in round_report.get(field, [])
        if isinstance(failure, dict)
    }
    if len(applied) == VALIDATORS:
        if failure_targets:
            raise RuntimeError(f"{label} full-fleet round recorded a peer failure")
    else:
        expected_failures = {
            f"validator-{index}" for index in range(VALIDATORS) if index not in applied
        }
        if failure_targets != expected_failures:
            raise RuntimeError(f"{label} catch-up round did not isolate its lagging validator")

    certificate = read_json(certificate_file)
    if int(certificate.get("block_height", -1)) != height:
        raise RuntimeError(f"{label} certificate height mismatch")
    if not isinstance(certificate.get("consensus_v2_commit"), dict):
        raise RuntimeError(f"{label} did not retain Consensus v2 finality")
    certificate_body = certificate.get("certificate")
    if not isinstance(certificate_body, dict):
        raise RuntimeError(f"{label} certificate omitted its quorum body")
    validators = certificate_body.get("validators")
    votes = certificate_body.get("votes")
    quorum = certificate_body.get("quorum")
    if (
        not isinstance(validators, list)
        or validators != [f"validator-{index}" for index in range(VALIDATORS)]
        or not isinstance(votes, list)
        or len(votes) != len(applied)
        or quorum != VALIDATORS - 1
    ):
        raise RuntimeError(f"{label} certificate validator, vote, or quorum count mismatch")
    receipt_codes = {str(receipt.get("code")) for receipt in literal_receipts}
    applied_statuses = [
        status(
            runner,
            node_bin,
            nodes[index],
            f"{label}.status.validator-{index}",
        )
        for index in applied
    ]
    identities = {tuple(status_identity(value).values()) for value in applied_statuses}
    if len(identities) != 1:
        raise RuntimeError(f"{label} applied validators did not converge")
    final_identity = status_identity(applied_statuses[0])
    if final_identity["height"] != height:
        raise RuntimeError(f"{label} finalized unexpected height")
    return {
        "label": label,
        "height": height,
        "batch_kind": batch_kind,
        "initial_applied_validator_count": len(applied),
        "applied_validator_count": len(applied),
        "certificate_validator_count": len(validators),
        "certificate_vote_count": len(votes),
        "certificate_quorum": quorum,
        "receipt_accepted": True,
        "receipt_codes": sorted(receipt_codes),
        "certificate_id": require_hex(
            certificate.get("certificate_id"), 96, f"{label} certificate ID"
        ),
        "certificate_sha256": sha256(certificate_file),
        "consensus_v2_commit": True,
        "transport_round_ok": True,
        "all_vote_requests_verified": round_report.get("all_vote_requests_verified") is True,
        "all_certified_sends_verified": round_report.get("all_sends_verified") is True,
        "failed_peer_targets": sorted(failure_targets),
        "identity": final_identity,
        "batch_sha256": sha256(batch_file),
    }


def authorization_files(
    runner: CommandRunner,
    node_bin: Path,
    node: Path,
    key_dir: Path,
    amendment_file: Path,
    output: Path,
    proposal_slot: int,
    expires_at_height: int,
    label: str,
) -> list[Path]:
    files: list[Path] = []
    for index in range(VALIDATORS):
        validator_id = f"validator-{index}"
        authorization = output / f"{validator_id}.authorization.json"
        runner.json(
            [
                str(node_bin),
                "governance-authorization-sign",
                "--data-dir",
                str(node),
                "--amendment-file",
                str(amendment_file),
                "--validator",
                validator_id,
                "--validator-key-file",
                str(key_file(key_dir, validator_id)),
                "--proposal-slot",
                str(proposal_slot),
                "--expires-at-height",
                str(expires_at_height),
                "--authorization-file",
                str(authorization),
            ],
            f"{label}.sign.{validator_id}",
        )
        files.append(authorization)
    return files


def assemble_signed_amendment(
    runner: CommandRunner,
    node_bin: Path,
    node: Path,
    amendment_file: Path,
    authorizations: list[Path],
    proposal_slot: int,
    output: Path,
    label: str,
) -> None:
    runner.json(
        [
            str(node_bin),
            "governance-amendment-assemble",
            "--data-dir",
            str(node),
            "--amendment-file",
            str(amendment_file),
            "--authorization-files",
            ",".join(str(path) for path in authorizations),
            "--proposal-slot",
            str(proposal_slot),
            "--output",
            str(output),
        ],
        f"{label}.assemble",
    )


def build_activation_batch(
    runner: CommandRunner,
    node_bin: Path,
    node: Path,
    key_dir: Path,
    output: Path,
    scheduling_height: int,
    activation_height: int,
    label: str,
) -> tuple[Path, str]:
    output.mkdir(parents=True)
    validators = ",".join(f"validator-{index}" for index in range(VALIDATORS))
    record_file = output / "activation-record.json"
    template = runner.json(
        [
            str(node_bin),
            "storage-activation-template",
            "--data-dir",
            str(node),
            "--activation-height",
            str(activation_height),
            "--record-file",
            str(record_file),
        ],
        f"{label}.template",
    )
    record = template.get("record")
    if not isinstance(record, dict):
        raise RuntimeError(f"{label} activation template omitted its record")
    if int(record.get("scheduling_block_height", -1)) != scheduling_height:
        raise RuntimeError(f"{label} activation scheduling height mismatch")
    if int(record.get("activation_height", -1)) != activation_height:
        raise RuntimeError(f"{label} activation height mismatch")
    if record.get("required_verifier_version") != MIGRATION_VERIFIER_VERSION:
        raise RuntimeError(f"{label} activation did not require verifier v2")
    activation_id = require_hex(record.get("activation_id"), 96, f"{label} activation ID")
    unsigned = output / "activation-amendment-unsigned.json"
    runner.json(
        [
            str(node_bin),
            "storage-activation-ratify",
            "--data-dir",
            str(node),
            "--record-file",
            str(record_file),
            "--validators",
            validators,
            "--support",
            validators,
            "--amendment-file",
            str(unsigned),
        ],
        f"{label}.ratify",
    )
    authorizations = authorization_files(
        runner,
        node_bin,
        node,
        key_dir,
        unsigned,
        output,
        scheduling_height,
        activation_height + 32,
        label,
    )
    signed = output / "activation-amendment-signed.json"
    assemble_signed_amendment(
        runner,
        node_bin,
        node,
        unsigned,
        authorizations,
        scheduling_height,
        signed,
        label,
    )
    batch = output / "activation-batch.json"
    runner.json(
        [
            str(node_bin),
            "storage-activation-batch",
            "--data-dir",
            str(node),
            "--record-file",
            str(record_file),
            "--authorization-amendment-file",
            str(signed),
            "--batch-file",
            str(batch),
        ],
        f"{label}.batch",
    )
    return batch, activation_id


def build_cancellation_batch(
    runner: CommandRunner,
    node_bin: Path,
    node: Path,
    key_dir: Path,
    output: Path,
    cancellation_height: int,
    activation_id: str,
    label: str,
) -> tuple[Path, str]:
    output.mkdir(parents=True)
    validators = ",".join(f"validator-{index}" for index in range(VALIDATORS))
    record_file = output / "cancellation-record.json"
    template = runner.json(
        [
            str(node_bin),
            "storage-cancellation-template",
            "--data-dir",
            str(node),
            "--activation-id",
            activation_id,
            "--reason",
            "pre-activation six-clone rollback rehearsal",
            "--record-file",
            str(record_file),
        ],
        f"{label}.template",
    )
    record = template.get("record")
    if not isinstance(record, dict):
        raise RuntimeError(f"{label} cancellation template omitted its record")
    if int(record.get("cancellation_height", -1)) != cancellation_height:
        raise RuntimeError(f"{label} cancellation height mismatch")
    cancellation_id = require_hex(
        record.get("cancellation_id"), 96, f"{label} cancellation ID"
    )
    unsigned = output / "cancellation-amendment-unsigned.json"
    runner.json(
        [
            str(node_bin),
            "storage-cancellation-ratify",
            "--data-dir",
            str(node),
            "--record-file",
            str(record_file),
            "--validators",
            validators,
            "--support",
            validators,
            "--amendment-file",
            str(unsigned),
        ],
        f"{label}.ratify",
    )
    authorizations = authorization_files(
        runner,
        node_bin,
        node,
        key_dir,
        unsigned,
        output,
        cancellation_height,
        cancellation_height + 32,
        label,
    )
    signed = output / "cancellation-amendment-signed.json"
    assemble_signed_amendment(
        runner,
        node_bin,
        node,
        unsigned,
        authorizations,
        cancellation_height,
        signed,
        label,
    )
    batch = output / "cancellation-batch.json"
    runner.json(
        [
            str(node_bin),
            "storage-cancellation-batch",
            "--data-dir",
            str(node),
            "--record-file",
            str(record_file),
            "--authorization-amendment-file",
            str(signed),
            "--batch-file",
            str(batch),
        ],
        f"{label}.batch",
    )
    return batch, cancellation_id


def create_topology(
    runner: CommandRunner,
    node_bin: Path,
    node: Path,
    root: Path,
    chain_id: str,
) -> Path:
    genesis = read_authenticated_json(node / "genesis.json")
    activation_height = int(genesis.get("consensus_v2_activation_height", 0))
    if activation_height <= 0:
        raise RuntimeError("source genesis does not activate Consensus v2")
    base_port, rpc_base_port = SHARED.find_ports()
    topology = root / "private" / "topology.json"
    runner.json(
        [
            str(node_bin),
            "topology-consensus-v2",
            "--chain-id",
            chain_id,
            "--validators",
            str(VALIDATORS),
            "--base-port",
            str(base_port),
            "--rpc-base-port",
            str(rpc_base_port),
            "--activation-height",
            str(activation_height),
            "--output",
            str(topology),
        ],
        "topology",
    )
    return topology


def staggered_restart(
    node_bin: Path,
    nodes_root: Path,
    topology: Path,
    root: Path,
    logs: Path,
    label: str,
) -> list[dict[str, Any]]:
    services: dict[int, tuple[Any, tuple[Any, Any]]] = {}
    receipts: list[dict[str, Any]] = []
    try:
        for index in range(VALIDATORS):
            services[index] = SHARED.start_validator(
                node_bin,
                nodes_root,
                topology,
                root,
                logs,
                label,
                index,
            )
        for index in range(VALIDATORS):
            process, handles = services.pop(index)
            SHARED.stop_validators([process], [handles])
            services[index] = SHARED.start_validator(
                node_bin,
                nodes_root,
                topology,
                root,
                logs,
                label,
                index,
                restart=index + 1,
            )
            receipts.append(
                {
                    "validator_id": f"validator-{index}",
                    "stopped_cleanly": True,
                    "reopened_and_ready": True,
                }
            )
    finally:
        remaining = list(services.values())
        SHARED.stop_validators(
            [process for process, _handles in remaining],
            [handles for _process, handles in remaining],
        )
    return receipts


def migration_round(
    runner: CommandRunner,
    node_bin: Path,
    nodes: list[Path],
    generations_root: Path,
    identity: dict[str, Any],
    label: str,
) -> list[dict[str, Any]]:
    reports = []
    for index, node in enumerate(nodes):
        reports.append(
            rebuild_generation(
                runner,
                node_bin,
                node,
                generations_root / label / f"validator-{index}",
                identity,
                f"{label}.validator-{index}",
            )
        )
    require_shared_packet_root(reports, label)
    if len({report["current_state_root"] for report in reports}) != 1:
        raise RuntimeError(f"{label} current-state roots differ across validators")
    if len({report["node_state_root"] for report in reports}) != VALIDATORS:
        raise RuntimeError(f"{label} did not preserve six node-local state roots")
    return reports


def require_legacy_mode(statuses: list[dict[str, Any]], label: str) -> None:
    converged_identity(statuses, label)
    for value in statuses:
        storage = value.get("storage")
        if not isinstance(storage, dict):
            raise RuntimeError(f"{label} omitted storage status")
        if storage.get("transactional_active") is not False:
            raise RuntimeError(f"{label} unexpectedly activated transactional storage")
        if storage.get("commitment_version") != "postfiat.replicated_state.v1":
            raise RuntimeError(f"{label} did not retain the legacy commitment")


def require_active_mode(statuses: list[dict[str, Any]], label: str) -> None:
    converged_identity(statuses, label)
    for value in statuses:
        storage = value.get("storage")
        if not isinstance(storage, dict):
            raise RuntimeError(f"{label} omitted storage status")
        if storage.get("transactional_active") is not True:
            raise RuntimeError(f"{label} did not activate transactional storage")
        if storage.get("commitment_version") != "postfiat.replicated_state.v2":
            raise RuntimeError(f"{label} did not switch to the v2 commitment")
        if int(storage.get("full_history_scans", -1)) != 0:
            raise RuntimeError(f"{label} performed post-activation full-history work")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-bin", type=Path, required=True)
    parser.add_argument("--incompatible-node-bin", type=Path, required=True)
    parser.add_argument("--incompatible-source-revision", required=True)
    parser.add_argument(
        "--source-data-dir",
        type=Path,
        action="append",
        required=True,
        help="repeat exactly six times in validator-0 through validator-5 order",
    )
    parser.add_argument("--validator-key-dir", type=Path, required=True)
    parser.add_argument("--workload-key-file", type=Path, required=True)
    parser.add_argument("--workload-recipient", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--expected-source-revision", required=True)
    parser.add_argument("--expected-source-height", type=int, default=CONTROLLED_HEIGHT)
    parser.add_argument("--expected-chain-id", default=CONTROLLED_CHAIN_ID)
    parser.add_argument("--expected-genesis-hash", default=CONTROLLED_GENESIS_HASH)
    parser.add_argument(
        "--development-smoke",
        action="store_true",
        help="allow a dirty checkout and non-controlled source; never evidence eligible",
    )
    args = parser.parse_args()

    revision = full_git_revision()
    incompatible_revision = args.incompatible_source_revision
    if args.expected_source_revision != revision:
        raise ValueError("HEAD does not match --expected-source-revision")
    if len(incompatible_revision) != 40 or any(
        character not in HEX64 for character in incompatible_revision
    ):
        raise ValueError("--incompatible-source-revision must be a full Git object ID")
    if not is_ancestor(incompatible_revision, revision):
        raise ValueError("incompatible source is not an ancestor of current source")
    if not args.development_smoke and not git_clean():
        raise ValueError("migration evidence requires a clean checkout")
    if not args.development_smoke and (
        args.expected_source_height != CONTROLLED_HEIGHT
        or args.expected_chain_id != CONTROLLED_CHAIN_ID
        or args.expected_genesis_hash != CONTROLLED_GENESIS_HASH
    ):
        raise ValueError("evidence mode requires the exact controlled height-924 domain")

    raw_node_bin = args.node_bin.expanduser()
    raw_incompatible_bin = args.incompatible_node_bin.expanduser()
    raw_output = args.output_dir.expanduser()
    if (
        raw_node_bin.is_symlink()
        or raw_incompatible_bin.is_symlink()
        or raw_output.is_symlink()
    ):
        raise ValueError("output and release binary paths must not be symlinks")
    node_bin = raw_node_bin.resolve()
    incompatible_bin = raw_incompatible_bin.resolve()
    output = raw_output.resolve()
    key_dir = args.validator_key_dir.resolve()
    workload_key = args.workload_key_file.resolve()
    sources = [path.resolve() for path in args.source_data_dir]
    if len(sources) != VALIDATORS or len(set(sources)) != VALIDATORS:
        raise ValueError("provide exactly six distinct --source-data-dir values")
    if output.exists():
        raise ValueError(f"refusing to overwrite output directory: {output}")
    for label, path in (
        ("--node-bin", node_bin),
        ("--incompatible-node-bin", incompatible_bin),
    ):
        if not path.is_file() or path.parent.name != "release":
            raise ValueError(f"{label} must identify a regular target/release binary")
    if node_bin == incompatible_bin or sha256(node_bin) == sha256(incompatible_bin):
        raise ValueError("current and incompatible binaries must be distinct")
    if not key_dir.is_dir() or key_dir.is_symlink():
        raise ValueError("--validator-key-dir must be a regular directory")
    for index in range(VALIDATORS):
        key_file(key_dir, f"validator-{index}")
    if not workload_key.is_file() or workload_key.is_symlink():
        raise ValueError("--workload-key-file must be a regular file")
    for index, source in enumerate(sources):
        validate_regular_tree(source, f"source validator-{index}")
        if output == source or output.is_relative_to(source) or source.is_relative_to(output):
            raise ValueError("source and output directories must be disjoint")
    stop_condition_receipt = verify_sources_stopped(sources)

    output.mkdir(parents=True)
    (output / "private").mkdir(mode=0o700)
    (output / "backups").mkdir()
    (output / "nodes").mkdir()
    (output / "generations").mkdir()
    (output / "artifacts").mkdir()
    runner = CommandRunner(output / "private" / "logs")

    current_binary = binary_identity(runner, node_bin, sources[0], revision, "current-binary")
    incompatible_binary = binary_identity(
        runner,
        incompatible_bin,
        sources[0],
        incompatible_revision,
        "incompatible-binary",
    )
    source_statuses = [
        status(runner, node_bin, source, f"source.validator-{index}")
        for index, source in enumerate(sources)
    ]
    source_identity = validate_source_statuses(
        source_statuses,
        args.expected_source_height,
        args.expected_chain_id,
        args.expected_genesis_hash,
    )
    source_cobalt = cobalt_boundary(sources[0])

    source_digests: list[str] = []
    backup_digests: list[str] = []
    nodes: list[Path] = []
    for index, source in enumerate(sources):
        source_digest = tree_sha256(source)
        backup = output / "backups" / f"validator-{index}"
        node = output / "nodes" / f"validator-{index}"
        shutil.copytree(source, backup, copy_function=shutil.copy2)
        shutil.copytree(source, node, copy_function=shutil.copy2)
        backup_digest = tree_sha256(backup)
        if backup_digest != source_digest:
            raise RuntimeError(f"validator-{index} immutable backup differs from source")
        source_digests.append(source_digest)
        backup_digests.append(backup_digest)
        nodes.append(node)
    stage_validator_keys(runner, node_bin, nodes, key_dir)
    topology = create_topology(
        runner,
        node_bin,
        nodes[0],
        output,
        args.expected_chain_id,
    )

    initial_rebuilds = migration_round(
        runner,
        node_bin,
        nodes,
        output / "generations",
        source_identity,
        "initial-migration",
    )
    phases: list[dict[str, Any]] = []

    height = args.expected_source_height + 1
    batch = output / "private" / "legacy-finality.batch.json"
    make_transfer_batch(
        runner,
        node_bin,
        nodes[0],
        workload_key,
        args.workload_recipient,
        batch,
        "legacy-finality",
    )
    phases.append(
        certify_and_apply(
            runner,
            node_bin,
            nodes,
            key_dir,
            topology,
            output,
            output / "private" / "logs",
            output / "artifacts",
            "legacy-finality",
            height,
            "transparent",
            batch,
        )
    )
    legacy_statuses = fleet_status(runner, node_bin, nodes, "legacy-finality")
    require_legacy_mode(legacy_statuses, "legacy finality fleet")
    restart_receipts = staggered_restart(
        node_bin,
        output / "nodes",
        topology,
        output / "private",
        output / "private" / "logs",
        "pre-activation-restart",
    )
    post_restart_statuses = fleet_status(runner, node_bin, nodes, "post-restart")
    post_restart_identity = converged_identity(post_restart_statuses, "post-restart fleet")
    if post_restart_identity != phases[-1]["identity"]:
        raise RuntimeError("pre-activation restart did not preserve the exact certified tip")

    restart_rebuilds = migration_round(
        runner,
        node_bin,
        nodes,
        output / "generations",
        post_restart_identity,
        "post-restart-refreeze",
    )
    cancelled_activation_height = height + 3
    height += 1
    activation_batch, cancelled_activation_id = build_activation_batch(
        runner,
        node_bin,
        nodes[0],
        key_dir,
        output / "private" / "cancelled-activation",
        height,
        cancelled_activation_height,
        "cancelled-activation",
    )
    phases.append(
        certify_and_apply(
            runner,
            node_bin,
            nodes,
            key_dir,
            topology,
            output,
            output / "private" / "logs",
            output / "artifacts",
            "cancelled-activation-scheduled",
            height,
            "governance",
            activation_batch,
        )
    )
    height += 1
    cancellation_batch, cancellation_id = build_cancellation_batch(
        runner,
        node_bin,
        nodes[0],
        key_dir,
        output / "private" / "cancellation",
        height,
        cancelled_activation_id,
        "pre-activation-cancellation",
    )
    phases.append(
        certify_and_apply(
            runner,
            node_bin,
            nodes,
            key_dir,
            topology,
            output,
            output / "private" / "logs",
            output / "artifacts",
            "pre-activation-cancellation",
            height,
            "governance",
            cancellation_batch,
        )
    )
    height += 1
    batch = output / "private" / "post-cancellation-legacy.batch.json"
    make_transfer_batch(
        runner,
        node_bin,
        nodes[0],
        workload_key,
        args.workload_recipient,
        batch,
        "post-cancellation-legacy",
    )
    phases.append(
        certify_and_apply(
            runner,
            node_bin,
            nodes,
            key_dir,
            topology,
            output,
            output / "private" / "logs",
            output / "artifacts",
            "post-cancellation-legacy-finality",
            height,
            "transparent",
            batch,
        )
    )
    cancelled_statuses = fleet_status(runner, node_bin, nodes, "post-cancellation")
    require_legacy_mode(cancelled_statuses, "post-cancellation fleet")
    cancelled_identity = converged_identity(cancelled_statuses, "post-cancellation fleet")

    final_rebuilds = migration_round(
        runner,
        node_bin,
        nodes,
        output / "generations",
        cancelled_identity,
        "final-activation-refreeze",
    )
    shared_packet_root = require_shared_packet_root(final_rebuilds, "final activation refreeze")
    final_activation_height = height + 4
    height += 1
    final_activation_batch, activation_id = build_activation_batch(
        runner,
        node_bin,
        nodes[0],
        key_dir,
        output / "private" / "final-activation",
        height,
        final_activation_height,
        "final-activation",
    )
    activation_record = read_json(
        output / "private" / "final-activation" / "activation-record.json"
    )
    if activation_record.get("migration_packet_root") != shared_packet_root:
        raise RuntimeError("activation record does not bind the shared six-validator packet root")
    phases.append(
        certify_and_apply(
            runner,
            node_bin,
            nodes,
            key_dir,
            topology,
            output,
            output / "private" / "logs",
            output / "artifacts",
            "final-activation-scheduled",
            height,
            "governance",
            final_activation_batch,
        )
    )
    scheduled_restart_receipts = staggered_restart(
        node_bin,
        output / "nodes",
        topology,
        output / "private",
        output / "private" / "logs",
        "scheduled-staggered-restart",
    )

    for lane in ("pre-activation-one", "pre-activation-two"):
        height += 1
        batch = output / "private" / f"{lane}.batch.json"
        make_transfer_batch(
            runner,
            node_bin,
            nodes[0],
            workload_key,
            args.workload_recipient,
            batch,
            lane,
        )
        phases.append(
            certify_and_apply(
                runner,
                node_bin,
                nodes,
                key_dir,
                topology,
                output,
                output / "private" / "logs",
                output / "artifacts",
                lane,
                height,
                "transparent",
                batch,
            )
        )

    height += 1
    if height != final_activation_height:
        raise RuntimeError("activation phase height calculation drifted")
    lagging_index = choose_lagging_validator(
        runner,
        node_bin,
        nodes[0],
        height,
    )
    activation_participants = [
        index for index in range(VALIDATORS) if index != lagging_index
    ]
    activation_workload = output / "private" / "activation-finality.batch.json"
    make_transfer_batch(
        runner,
        node_bin,
        nodes[0],
        workload_key,
        args.workload_recipient,
        activation_workload,
        "activation-finality",
    )
    incompatible_artifact = output / "private" / "mixed-version-probe.certificate.json"
    mixed_version = runner.expected_failure(
        [
            str(incompatible_bin),
            "certify-batch",
            "--data-dir",
            str(nodes[0]),
            "--batch-kind",
            "transparent",
            "--batch-file",
            str(activation_workload),
            "--validator-key-dir",
            str(key_dir),
            "--proposal-file",
            str(output / "private" / "mixed-version-probe.proposal.json"),
            "--vote-dir",
            str(output / "private" / "mixed-version-probe-votes"),
            "--certificate-file",
            str(incompatible_artifact),
            "--height",
            str(height),
        ],
        "mixed-version-probe",
        "storage_unsupported_schema",
        "transactional migration verification binding is invalid",
    )
    if incompatible_artifact.exists():
        raise RuntimeError("incompatible binary left a certificate artifact")
    activation_phase = certify_and_apply(
        runner,
        node_bin,
        nodes,
        key_dir,
        topology,
        output,
        output / "private" / "logs",
        output / "artifacts",
        "activation-finality",
        height,
        "transparent",
        activation_workload,
        activation_participants,
    )
    lagging_validator = f"validator-{lagging_index}"
    lagging_status = status(
        runner,
        node_bin,
        nodes[lagging_index],
        f"activation-catch-up.lagging.{lagging_validator}",
    )
    if int(lagging_status["block_height"]) != height - 1:
        raise RuntimeError("catch-up validator did not remain one block behind")
    catch_up_receipts = runner.receipts(
        [
            str(node_bin),
            "apply-batch",
            "--data-dir",
            str(nodes[lagging_index]),
            "--batch-file",
            str(activation_workload),
            "--certificate-file",
            str(output / "artifacts" / "activation-finality" / "block-certificate.json"),
        ],
        f"activation-catch-up.apply.{lagging_validator}",
    )
    if not all(receipt.get("accepted") is True for receipt in catch_up_receipts):
        raise RuntimeError("catch-up validator did not accept the certified activation block")
    activation_statuses = fleet_status(runner, node_bin, nodes, "activation-converged")
    require_active_mode(activation_statuses, "activation fleet")
    activation_identity = converged_identity(activation_statuses, "activation fleet")
    if activation_identity != activation_phase["identity"]:
        raise RuntimeError("catch-up did not converge on the activation identity")
    activation_phase["applied_validator_count"] = VALIDATORS
    activation_phase["catch_up_validator"] = lagging_validator
    activation_phase["catch_up_receipt_accepted"] = True
    activation_phase["catch_up_receipt_count"] = len(catch_up_receipts)
    activation_phase["catch_up_receipt_codes"] = sorted(
        {str(receipt.get("code")) for receipt in catch_up_receipts}
    )
    phases.append(activation_phase)

    height += 1
    batch = output / "private" / "post-activation-finality.batch.json"
    make_transfer_batch(
        runner,
        node_bin,
        nodes[0],
        workload_key,
        args.workload_recipient,
        batch,
        "post-activation-finality",
    )
    phases.append(
        certify_and_apply(
            runner,
            node_bin,
            nodes,
            key_dir,
            topology,
            output,
            output / "private" / "logs",
            output / "artifacts",
            "post-activation-finality",
            height,
            "transparent",
            batch,
        )
    )
    active_statuses = fleet_status(runner, node_bin, nodes, "post-activation")
    require_active_mode(active_statuses, "post-activation fleet")
    forward_restart_receipts = staggered_restart(
        node_bin,
        output / "nodes",
        topology,
        output / "private",
        output / "private" / "logs",
        "post-activation-forward-restart",
    )
    height += 1
    batch = output / "private" / "forward-recovery.batch.json"
    make_transfer_batch(
        runner,
        node_bin,
        nodes[0],
        workload_key,
        args.workload_recipient,
        batch,
        "forward-recovery",
    )
    phases.append(
        certify_and_apply(
            runner,
            node_bin,
            nodes,
            key_dir,
            topology,
            output,
            output / "private" / "logs",
            output / "artifacts",
            "post-activation-forward-recovery",
            height,
            "transparent",
            batch,
        )
    )
    final_statuses = fleet_status(runner, node_bin, nodes, "final")
    require_active_mode(final_statuses, "final fleet")
    final_identity = converged_identity(final_statuses, "final fleet")
    final_cobalt = cobalt_boundary(nodes[0])
    if final_cobalt != source_cobalt:
        raise RuntimeError("storage workflow changed Cobalt or validator-registry authority state")

    clone_reports = []
    for index, source in enumerate(sources):
        source_after = tree_sha256(source)
        backup_after = tree_sha256(output / "backups" / f"validator-{index}")
        if source_after != source_digests[index] or backup_after != backup_digests[index]:
            raise RuntimeError(f"validator-{index} source or immutable backup changed")
        clone_reports.append(
            {
                "validator_id": f"validator-{index}",
                "source_tree_sha256": source_digests[index],
                "backup_tree_sha256": backup_digests[index],
                "backup_reverified_sha256": backup_after,
                "initial_migration": initial_rebuilds[index],
                "post_restart_refreeze": restart_rebuilds[index],
                "final_activation_refreeze": final_rebuilds[index],
                "final_identity": status_identity(final_statuses[index]),
            }
        )

    exact_rehearsal = (
        args.expected_source_height == CONTROLLED_HEIGHT
        and args.expected_chain_id == CONTROLLED_CHAIN_ID
        and args.expected_genesis_hash == CONTROLLED_GENESIS_HASH
    )
    evidence_eligible = not args.development_smoke and exact_rehearsal and git_clean()
    report = {
        "schema": REPORT_SCHEMA,
        "status": "PASS" if evidence_eligible else "DEVELOPMENT SMOKE PASS",
        "evidence_eligible": evidence_eligible,
        "source_worktree_clean": git_clean(),
        "captured_at": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "source_revision": revision,
        "node_binary_sha256": current_binary["sha256"],
        "node_binary_build": {
            "git_revision": current_binary["git_revision"],
            "profile": current_binary["profile"],
        },
        "incompatible_binary": incompatible_binary,
        "source_height": args.expected_source_height,
        "chain_id": args.expected_chain_id,
        "genesis_hash": args.expected_genesis_hash,
        "exact_existing_chain_rehearsal": exact_rehearsal,
        "clone_count": VALIDATORS,
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
        "stop_condition_receipt": stop_condition_receipt,
        "consensus_v2_unchanged": all(
            phase["consensus_v2_commit"] is True for phase in phases
        ),
        "cobalt_authority_unchanged": True,
        "literal_receipts_exact": all(
            phase["receipt_accepted"] is True for phase in phases
        ),
        "zero_post_activation_full_history_scans": True,
        "external_network_contacted": False,
        "loopback_transport_used": True,
        "devnet_queried_or_mutated": False,
        "identities": {
            "source_tip": source_identity["tip"],
            "source_state_root": source_identity["state_root"],
            "packet_root": shared_packet_root,
            "activation_id": activation_id,
            "cancelled_activation_id": cancelled_activation_id,
            "cancellation_id": cancellation_id,
            "activation_tip": activation_identity["tip"],
            "activation_state_root": activation_identity["state_root"],
            "final_tip": final_identity["tip"],
            "final_state_root": final_identity["state_root"],
        },
        "cobalt_boundary": {
            "before": source_cobalt,
            "after": final_cobalt,
        },
        "mixed_version_probe": {
            **mixed_version,
            "binary_sha256": incompatible_binary["sha256"],
            "source_revision": incompatible_binary["source_revision"],
            "verifier_boundary": "v1 binary refused v2 migration generation",
        },
        "restart_receipts": {
            "pre_activation": restart_receipts,
            "scheduled_staggered": scheduled_restart_receipts,
            "post_activation_forward": forward_restart_receipts,
        },
        "phases": phases,
        "clones": clone_reports,
    }
    report_path = output / "six-clone-migration-report.json"
    write_json(report_path, report)
    print(f"storage-six-clone-migration={report['status']}", flush=True)
    print(f"report={report_path}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
