#!/usr/bin/env python3
"""Create a fail-closed six-host pfUSDC conservation identity verdict.

The validator fleet is read directly. A finalized-checkpoint snapshot is then
imported and verified locally with the supplied combined-head node before a
local Foundry `cast` audit. Prefer ``--signed-snapshot-dir`` for a previously
signed finalized checkpoint; no executable or credential is copied to a
validator.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import time
import uuid
from typing import Any, Iterable


SCHEMA = "postfiat.pfusdc.conservation-identity.v2"
MAX_U64 = (1 << 64) - 1
DEFAULT_ETHEREUM_SOURCE_RPC = "https://ethereum-rpc.publicnode.com"
AUDIT_MISMATCH = re.compile(r"V=(\d+) S=(\d+) D=(\d+) B=(\d+) R=(\d+) expected=(\d+) unexplained_delta=(-?\d+)")
DEPRECATED_ARBITRUM_CHAIN_IDS = frozenset((42_161, 421_614))
COMBINED_HEAD_CANDIDATE_SHA256 = "0467b6b84047e004eac4c3add673a41c8ba2f3eeed580b7b9eda60be43037b56"


class CheckerError(RuntimeError):
    pass


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def write_json(path: Path, value: Any, *, pretty: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    encoded = (
        json.dumps(value, sort_keys=True, indent=2).encode("utf-8")
        if pretty else canonical_json(value)
    )
    temporary.write_bytes(encoded + b"\n")
    os.chmod(temporary, 0o644)
    temporary.replace(path)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def parse_u64(value: Any, label: str) -> int:
    if isinstance(value, bool):
        raise CheckerError(f"{label} must be unsigned atoms")
    if isinstance(value, str):
        if not value.isdecimal():
            raise CheckerError(f"{label} is not decimal atoms")
        value = int(value)
    if not isinstance(value, int) or value < 0 or value > MAX_U64:
        raise CheckerError(f"{label} is outside u64 atoms")
    return value


def parse_i128(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, str):
        raise CheckerError(f"{label} must be signed decimal atoms")
    if not re.fullmatch(r"-?(0|[1-9][0-9]*)", value):
        raise CheckerError(f"{label} is not signed decimal atoms")
    parsed = int(value)
    if parsed < -(1 << 127) or parsed > (1 << 127) - 1:
        raise CheckerError(f"{label} is outside i128 atoms")
    return parsed


def checked_add(left: int, right: int, label: str) -> int:
    result = left + right
    if result > MAX_U64:
        raise CheckerError(f"{label} overflows u64 atoms")
    return result


def inventory_hosts(path: Path, expected: int) -> list[tuple[str, str]]:
    if expected <= 0 or not path.is_file():
        raise CheckerError("validator inventory is missing or expected count is invalid")
    hosts: dict[str, str] = {}
    for line in path.read_text().splitlines():
        fields = line.split()
        if len(fields) >= 2 and fields[0].startswith("validator-"):
            if fields[0] in hosts:
                raise CheckerError(f"duplicate inventory validator {fields[0]}")
            hosts[fields[0]] = fields[1]
    wanted = {f"validator-{index}" for index in range(expected)}
    if set(hosts) != wanted:
        raise CheckerError(f"inventory set mismatch: expected {sorted(wanted)}, got {sorted(hosts)}")
    return [(validator, hosts[validator]) for validator in sorted(hosts)]


def remote_state_script() -> str:
    return r'''import json, os, subprocess
validator = os.environ["PFUSDC_VALIDATOR_ID"]
service = "postfiat-" + validator + ".service"
pid = subprocess.check_output(["systemctl", "show", "--property=MainPID", "--value", service], text=True).strip()
if not pid or pid == "0": raise RuntimeError("service has no running main pid")
node = os.path.realpath("/proc/{}/exe".format(pid))
result = subprocess.run([node, "status", "--data-dir", "/var/lib/postfiat/" + validator], text=True, capture_output=True)
print(json.dumps({"validator_id":validator,"node_binary":node,"status_returncode":result.returncode,"status_stdout":result.stdout,"status_stderr":result.stderr}, sort_keys=True))
'''


def collect_parent(validator: str, host: str, identity: Path | None) -> dict[str, Any]:
    command = ["ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=no", "-o", "ConnectTimeout=15"]
    if identity is not None:
        command.extend(["-i", str(identity)])
    command.extend([f"root@{host}", "env", f"PFUSDC_VALIDATOR_ID={validator}", "python3", "-"])
    result = subprocess.run(command, input=remote_state_script(), text=True, capture_output=True, timeout=60)
    if result.returncode != 0:
        raise CheckerError(f"{validator} unreachable: {result.stderr.strip() or result.stdout.strip()}")
    try:
        row = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise CheckerError(f"{validator} produced invalid JSON") from error
    if row.get("validator_id") != validator or row.get("status_returncode") != 0:
        raise CheckerError(f"{validator} did not return a successful status")
    try:
        row["status"] = json.loads(row["status_stdout"])
    except (KeyError, json.JSONDecodeError) as error:
        raise CheckerError(f"{validator} status JSON is invalid") from error
    return row


def common_parent(rows: Iterable[dict[str, Any]], expected: int) -> tuple[int, str, str]:
    rows = list(rows)
    if len(rows) != expected or {row.get("validator_id") for row in rows} != {f"validator-{i}" for i in range(expected)}:
        raise CheckerError("missing or duplicate validator host response")
    parents: set[tuple[int, str, str]] = set()
    for row in rows:
        status = row["status"]
        validator = row["validator_id"]
        if status.get("status") != "running" or status.get("node_id") != validator:
            raise CheckerError(f"{validator} is not its running node")
        if status.get("validator_count") != expected or status.get("mempool_pending") != 0:
            raise CheckerError(f"{validator} has wrong validator count or a mutating mempool")
        height, tip, root = status.get("block_height"), status.get("block_tip_hash"), status.get("state_root")
        if not isinstance(height, int) or height < 0 or not isinstance(tip, str) or not tip or not isinstance(root, str) or not root:
            raise CheckerError(f"{validator} omitted finalized parent fields")
        parents.add((height, tip, root))
    if len(parents) != 1:
        raise CheckerError(f"six-host finalized parent divergence: {sorted(parents)}")
    return next(iter(parents))


def run(command: list[str], *, timeout: int = 180, input_text: str | None = None) -> dict[str, Any]:
    result = subprocess.run(
        command, text=True, input=input_text, capture_output=True, timeout=timeout
    )
    return {"command": command, "returncode": result.returncode, "stdout": result.stdout, "stderr": result.stderr}


def require_file(path: Path, label: str) -> None:
    if not path.is_file():
        raise CheckerError(f"{label} is missing: {path}")


def failed_command(label: str, result: dict[str, Any]) -> CheckerError:
    output = (result.get("stderr", "") + "\n" + result.get("stdout", "")).splitlines()
    detail = next((line.strip() for line in output if line.strip().startswith("error:")), None)
    return CheckerError(f"{label}: {detail or 'command returned nonzero'}")


def export_snapshot(
    canonical: dict[str, Any],
    local_snapshot: Path,
    transcript_path: Path,
    identity: Path | None,
) -> dict[str, Any]:
    stamp = f"pfusdc-conservation-{int(time.time())}-{uuid.uuid4().hex}"
    remote_dir = f"/var/lib/postfiat/{canonical['validator_id']}/lane-b-evidence/{stamp}"
    command = ["ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=no", "-o", "ConnectTimeout=15"]
    if identity is not None:
        command.extend(["-i", str(identity)])
    remote_script = (
        "set -euo pipefail\n"
        "umask 077\n"
        f"install -d -m 700 -- {shlex.quote(remote_dir)}\n"
        f"exec {shlex.quote(canonical['node_binary'])} snapshot-export-finalized-checkpoint "
        f"--data-dir {shlex.quote('/var/lib/postfiat/' + canonical['validator_id'])} "
        f"--snapshot-dir {shlex.quote(remote_dir)}\n"
    )
    command.extend([f"root@{canonical['host']}", "bash", "--noprofile", "--norc", "-s", "--"])
    exported = run(command, input_text=remote_script)
    write_json(transcript_path, exported)
    if exported["returncode"] != 0:
        raise failed_command("remote finalized-checkpoint snapshot export failed", exported)
    local_snapshot.parent.mkdir(parents=True, exist_ok=True)
    rsync_command = "ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o ConnectTimeout=15"
    if identity is not None:
        rsync_command += " -i " + str(identity)
    copied = run(["rsync", "-e", rsync_command, "-a", f"root@{canonical['host']}:{remote_dir}/", f"{local_snapshot}/"])
    write_json(local_snapshot.parent / "snapshot-copy.json", copied)
    if copied["returncode"] != 0:
        raise failed_command("snapshot copy failed", copied)
    manifest_path = local_snapshot / "snapshot_manifest.json"
    require_file(manifest_path, "snapshot manifest")
    try:
        manifest = json.loads(manifest_path.read_text())
    except json.JSONDecodeError as error:
        raise CheckerError("snapshot manifest is invalid JSON") from error
    return {"remote_path": remote_dir, "local_path": str(local_snapshot), "manifest_path": str(manifest_path), "manifest_sha256": sha256_file(manifest_path), "manifest": manifest}


def existing_snapshot(snapshot_dir: Path) -> dict[str, Any]:
    manifest_path = snapshot_dir / "snapshot_manifest.json"
    require_file(manifest_path, "existing unsigned snapshot manifest")
    try:
        manifest = json.loads(manifest_path.read_text())
    except json.JSONDecodeError as error:
        raise CheckerError("existing unsigned snapshot manifest is invalid JSON") from error
    return {
        "remote_path": None,
        "local_path": str(snapshot_dir),
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "manifest": manifest,
    }


def existing_signed_snapshot(snapshot_dir: Path) -> dict[str, Any]:
    """Return validated metadata for an already signed finalized checkpoint."""
    snapshot = existing_snapshot(snapshot_dir)
    signed_manifest_path = snapshot_dir / "snapshot.signed-manifest.json"
    require_file(signed_manifest_path, "existing signed snapshot manifest")
    try:
        signed_manifest = json.loads(signed_manifest_path.read_text())
    except json.JSONDecodeError as error:
        raise CheckerError("existing signed snapshot manifest is invalid JSON") from error
    snapshot.update({
        "source": "supplied_signed_finalized_checkpoint",
        "signed_snapshot_path": str(snapshot_dir),
        "signed_manifest_path": str(signed_manifest_path),
        "signed_manifest_sha256": sha256_file(signed_manifest_path),
        "signed_manifest_schema": signed_manifest.get("schema"),
        "signed_manifest_publisher": signed_manifest.get("publisher"),
    })
    return snapshot


def prior_parent_evidence(path: Path, expected: int) -> tuple[tuple[int, str, str], list[dict[str, str]]]:
    """Validate retained 6/6 parent evidence without contacting the fleet.

    Reusing a signed local import is deliberately an offline operation: the
    retained verdict must carry the exact six raw parent responses and hashes
    that bound the snapshot.  This prevents a snapshot-only audit from being
    mistaken for a fresh fleet read while a height freeze is active.
    """
    require_file(path, "prior conservation identity")
    try:
        prior = json.loads(path.read_text())
    except json.JSONDecodeError as error:
        raise CheckerError("prior conservation identity is invalid JSON") from error
    parent = (
        prior.get("height"),
        prior.get("tip"),
        prior.get("state_root"),
    )
    if not isinstance(parent[0], int) or parent[0] < 0 or not isinstance(parent[1], str) or not parent[1] or not isinstance(parent[2], str) or not parent[2]:
        raise CheckerError("prior conservation identity omitted finalized parent fields")
    raw = prior.get("source_rpc", {}).get("raw_response_paths")
    if not isinstance(raw, list):
        raise CheckerError("prior conservation identity omitted raw parent responses")
    by_validator: dict[str, dict[str, str]] = {}
    for entry in raw:
        if not isinstance(entry, dict):
            continue
        validator = entry.get("validator_id")
        if not isinstance(validator, str) or not validator.startswith("validator-"):
            continue
        evidence_path = entry.get("path")
        evidence_hash = entry.get("sha256")
        if not isinstance(evidence_path, str) or not isinstance(evidence_hash, str):
            raise CheckerError(f"prior parent response for {validator} is malformed")
        if validator in by_validator:
            raise CheckerError(f"duplicate prior parent response for {validator}")
        raw_path = Path(evidence_path)
        require_file(raw_path, f"prior parent response for {validator}")
        if sha256_file(raw_path) != evidence_hash:
            raise CheckerError(f"prior parent response hash mismatch for {validator}")
        try:
            row = json.loads(raw_path.read_text())
        except json.JSONDecodeError as error:
            raise CheckerError(f"prior parent response is invalid JSON for {validator}") from error
        if row.get("validator_id") != validator:
            raise CheckerError(f"prior parent response validator mismatch for {validator}")
        by_validator[validator] = {
            "validator_id": validator,
            "path": evidence_path,
            "sha256": evidence_hash,
            "row": row,
        }
    rows = [entry.pop("row") for _, entry in sorted(by_validator.items())]
    if common_parent(rows, expected) != parent:
        raise CheckerError("prior six-host parent does not match prior conservation identity")
    return parent, [{key: value for key, value in entry.items() if key != "row"} for _, entry in sorted(by_validator.items())]


def bind_snapshot(manifest: dict[str, Any], status: dict[str, Any], parent: tuple[int, str, str]) -> None:
    height, tip, root = parent
    for label, value in (("manifest", manifest), ("imported status", status)):
        if value.get("block_height") != height or value.get("block_tip_hash") != tip or value.get("state_root") != root:
            raise CheckerError(f"{label} does not bind to the six-host common parent")


def audit_identity(report: dict[str, Any], asset_id: str, parent: tuple[int, str, str], source_rpc_url: str, raw_paths: list[dict[str, str]], legacy_label: str | None, evidence_hash: str, legacy_finding: dict[str, str] | None = None) -> dict[str, Any]:
    height, tip, root = parent
    if report.get("asset_id") != asset_id or report.get("current_height") != height:
        raise CheckerError("local audit term source height or asset differs from snapshot parent")
    v = parse_u64(report.get("source_vault_atoms"), "V")
    s = parse_u64(report.get("live_claim_atoms"), "S")
    d = parse_u64(report.get("uncredited_deposit_atoms"), "D")
    b = parse_u64(report.get("burned_unsettled_atoms"), "B")
    r = parse_u64(report.get("released_unsettled_atoms"), "R")
    rhs = checked_add(checked_add(s, d, "S+D"), b, "S+D+B")
    if r > rhs:
        raise CheckerError("R exceeds S+D+B")
    rhs -= r
    residual = v - rhs
    if residual != report.get("unexplained_delta_atoms") or rhs != report.get("expected_source_vault_atoms"):
        raise CheckerError("local audit report arithmetic is internally inconsistent")
    status = "verified" if residual == 0 and report.get("conserved") is True else "violated"
    identity: dict[str, Any] = {
        "schema": SCHEMA, "status": status, "verified": status == "verified", "execution_blocked": False, "asset_id": asset_id,
        "height": height, "tip": tip, "state_root": root,
        "source_rpc": {"url": source_rpc_url, "query": "vault-bridge-conservation-audit", "raw_response_paths": raw_paths},
        "components": {"V": str(v), "S": str(s), "D": str(d), "B": str(b), "R": str(r)},
        "lhs": str(v), "rhs": str(rhs), "residual_atoms": str(residual),
        "lower_level": {key: report.get(key) for key in ("issued_supply_atoms", "wrapped_supply_atoms", "nav_subscription_claim_atoms", "other_claim_atoms", "recognized_but_unallocated_atoms", "observed_but_uncounted_atoms", "route_count", "deposit_count", "redemption_count", "routes", "deposits", "redemptions")},
        "node_report": {"expected_source_vault_atoms": str(report.get("expected_source_vault_atoms")), "unexplained_delta_atoms": str(report.get("unexplained_delta_atoms")), "conserved": report.get("conserved")},
    }
    legacy_routes = [
        route for route in report.get("routes", [])
        if route.get("source_chain_id") in DEPRECATED_ARBITRUM_CHAIN_IDS
    ]
    if legacy_routes:
        if legacy_label != "deprecated-Arbitrum-legacy":
            raise CheckerError("legacy Arbitrum source requires explicit --legacy-source-label deprecated-Arbitrum-legacy")
        if legacy_finding is None:
            raise CheckerError("legacy Arbitrum source requires --legacy-finding-file")
        identity["legacy_backing_migration"] = {
            "classification": "deprecated-Arbitrum-legacy", "evidence_hash": evidence_hash,
            "canonical_finding": legacy_finding,
            "pftl_finalized_height": height,
            "routes": [{
                "source_chain_id": route["source_chain_id"],
                "vault_address": route["vault_address"],
                "token_address": route["token_address"],
                "vault_balance_atoms": str(route["vault_balance_atoms"]),
                "activation_height": route["activation_height"],
                "expires_at_height": route["expires_at_height"],
                "profile_hash": route["profile_hash"],
                "route_id": route["route_id"],
                "route_epoch": route["route_epoch"],
                "current_for_new_ingress": route["current_for_new_ingress"],
            } for route in legacy_routes],
            "finding": "lineage finding only; pre-Ethereum supply must be reconciled to Ethereum backing or redeemed out",
        }
    elif legacy_label is not None:
        raise CheckerError("legacy source label supplied for a non-legacy audit")
    return identity


def parse_failed_audit(message: str, asset_id: str, parent: tuple[int, str, str]) -> dict[str, Any] | None:
    match = AUDIT_MISMATCH.search(message)
    if match is None:
        return None
    v, s, d, b, r, rhs, residual = map(int, match.groups())
    height, tip, root = parent
    return {"schema": SCHEMA, "status": "violated", "verified": False, "execution_blocked": False, "asset_id": asset_id, "height": height, "tip": tip, "state_root": root, "components": {"V": str(v), "S": str(s), "D": str(d), "B": str(b), "R": str(r)}, "lhs": str(v), "rhs": str(rhs), "residual_atoms": str(residual), "failure": message}


def attach_opening_bracket(identity: dict[str, Any], opening_path: Path) -> None:
    require_file(opening_path, "opening conservation identity")
    try:
        opening = json.loads(opening_path.read_text())
    except json.JSONDecodeError as error:
        raise CheckerError("opening conservation identity is invalid JSON") from error
    if opening.get("status") == "execution_blocked":
        raise CheckerError("opening conservation identity is execution_blocked")
    opening_residual = parse_i128(opening.get("residual_atoms"), "opening residual")
    current_residual = parse_i128(identity.get("residual_atoms"), "current residual")
    identity["opening_bracket"] = {
        "opening_identity": {"path": str(opening_path), "sha256": sha256_file(opening_path)},
        "opening_height": opening.get("height"),
        "opening_residual_atoms": str(opening_residual),
        "residual_delta_from_h310": str(current_residual - opening_residual),
        "residual_delta_zero": current_residual == opening_residual,
        "final_criterion": "residual_final - residual_opening == 0 exactly; any nonzero value blocks acceptance",
    }


def execution_blocked(
    asset_id: str,
    message: str,
    raw_paths: list[dict[str, str]],
    parent: tuple[int, str, str] | None = None,
    snapshot: dict[str, Any] | None = None,
    source_rpc_url: str | None = None,
    legacy_label: str | None = None,
    legacy_finding: dict[str, str] | None = None,
) -> dict[str, Any]:
    verdict: dict[str, Any] = {
        "schema": SCHEMA, "status": "execution_blocked", "verified": False, "execution_blocked": True,
        "asset_id": asset_id, "components": None, "lhs": None, "rhs": None,
        "residual_atoms": None, "failure": message,
        "source_rpc": {"url": source_rpc_url, "raw_response_paths": raw_paths},
    }
    if parent is not None:
        verdict.update({"height": parent[0], "tip": parent[1], "state_root": parent[2]})
    if snapshot is not None:
        verdict["snapshot"] = snapshot
    if legacy_label is not None and legacy_finding is not None:
        verdict["legacy_backing_migration"] = {
            "classification": legacy_label,
            "canonical_finding": legacy_finding,
            "finding": "lineage finding only; local audit did not produce conservation terms",
        }
    return verdict


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory-file", type=Path, required=True)
    parser.add_argument("--asset-id", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--ssh-identity-file", type=Path)
    parser.add_argument("--expected-validator-count", type=int, default=6)
    parser.add_argument("--local-node-bin", type=Path, default=Path("target/release/postfiat-node"))
    parser.add_argument("--expected-local-node-sha256", default=COMBINED_HEAD_CANDIDATE_SHA256)
    parser.add_argument("--local-cast-bin", type=Path, default=Path.home() / ".foundry/bin/cast")
    parser.add_argument("--scratch-dir", type=Path)
    snapshots = parser.add_mutually_exclusive_group()
    snapshots.add_argument("--unsigned-snapshot-dir", type=Path)
    snapshots.add_argument("--signed-snapshot-dir", type=Path)
    parser.add_argument(
        "--reuse-imported-data-dir", type=Path,
        help="read-only reuse of a previously verified signed snapshot import; requires --signed-snapshot-dir and --prior-identity-file",
    )
    parser.add_argument(
        "--prior-identity-file", type=Path,
        help="retained six-host parent evidence that binds --reuse-imported-data-dir",
    )
    parser.add_argument(
        "--opening-identity-file", type=Path,
        help="opening identity used to emit residual_delta_from_h310 for an E2E closing bracket",
    )
    parser.add_argument(
        "--snapshot-publisher-private-key-file", type=Path,
        default=Path.home() / ".postfiat/recovery-v3-snapshot-publisher.private.json",
    )
    parser.add_argument("--snapshot-publisher-public-key-file", type=Path, required=True)
    parser.add_argument("--source-rpc-url", default=DEFAULT_ETHEREUM_SOURCE_RPC)
    parser.add_argument("--vault-interface-lineage-manifest", type=Path, required=True)
    parser.add_argument("--legacy-source-label", choices=["deprecated-Arbitrum-legacy"])
    parser.add_argument("--legacy-finding-file", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    raw_paths: list[dict[str, str]] = []
    parent: tuple[int, str, str] | None = None
    snapshot: dict[str, Any] | None = None
    legacy_finding: dict[str, str] | None = None
    try:
        require_file(args.local_node_bin, "local node binary")
        require_file(args.local_cast_bin, "local cast binary")
        if sha256_file(args.local_node_bin) != args.expected_local_node_sha256:
            raise CheckerError("local node binary does not match the required combined-head candidate hash")
        require_file(args.snapshot_publisher_public_key_file, "snapshot publisher public key file")
        require_file(args.vault_interface_lineage_manifest, "vault interface lineage manifest")
        if args.reuse_imported_data_dir is not None:
            if args.signed_snapshot_dir is None:
                raise CheckerError("--reuse-imported-data-dir requires --signed-snapshot-dir")
            if args.prior_identity_file is None:
                raise CheckerError("--reuse-imported-data-dir requires --prior-identity-file")
            if not args.reuse_imported_data_dir.is_dir():
                raise CheckerError(f"reused imported data directory is missing: {args.reuse_imported_data_dir}")
        if args.signed_snapshot_dir is None:
            require_file(args.snapshot_publisher_private_key_file, "snapshot publisher private key file")
        if args.legacy_finding_file is not None:
            require_file(args.legacy_finding_file, "legacy backing finding")
            legacy_finding = {
                "path": str(args.legacy_finding_file),
                "sha256": sha256_file(args.legacy_finding_file),
            }
        if args.ssh_identity_file is not None:
            require_file(args.ssh_identity_file, "SSH identity file")
        if not args.source_rpc_url.startswith("https://"):
            raise CheckerError("source RPC must be an https URL")
        scratch_root = args.scratch_dir or args.output.parent / "conservation-scratch"
        run_dir = scratch_root / f"run-{uuid.uuid4().hex}"
        if args.reuse_imported_data_dir is not None:
            parent, raw_paths = prior_parent_evidence(args.prior_identity_file, args.expected_validator_count)
            snapshot = existing_signed_snapshot(args.signed_snapshot_dir)
            manifest = snapshot["manifest"]
            bind_snapshot(manifest, manifest, parent)
            signed_dir = args.signed_snapshot_dir
            imported_dir = args.reuse_imported_data_dir
            snapshot["reused_imported_data_dir"] = str(imported_dir)
            snapshot["prior_identity_path"] = str(args.prior_identity_file)
            snapshot["prior_identity_sha256"] = sha256_file(args.prior_identity_file)
        else:
            rows = []
            raw_dir = args.output.parent / "raw"
            for validator, host in inventory_hosts(args.inventory_file, args.expected_validator_count):
                row = collect_parent(validator, host, args.ssh_identity_file)
                row["host"] = host
                raw_path = raw_dir / f"{validator}.json"
                write_json(raw_path, row)
                raw_paths.append({"validator_id": validator, "path": str(raw_path), "sha256": sha256_file(raw_path)})
                rows.append(row)
            parent = common_parent(rows, args.expected_validator_count)
            canonical = rows[0]
            if args.signed_snapshot_dir is not None:
                snapshot = existing_signed_snapshot(args.signed_snapshot_dir)
            elif args.unsigned_snapshot_dir is None:
                snapshot = export_snapshot(
                    canonical, run_dir / "unsigned-snapshot",
                    run_dir / "unsigned-snapshot-export.json", args.ssh_identity_file,
                )
            else:
                snapshot = existing_snapshot(args.unsigned_snapshot_dir)
            manifest = snapshot["manifest"]
            bind_snapshot(manifest, manifest, parent)
            if args.signed_snapshot_dir is not None:
                signed_dir = args.signed_snapshot_dir
            else:
                signed_dir = run_dir / "signed-snapshot"
                signed = run([
                    str(args.local_node_bin), "snapshot-export-signed-finalized-checkpoint",
                    "--data-dir", snapshot["local_path"], "--snapshot-dir", str(signed_dir),
                    "--publisher-key-file", str(args.snapshot_publisher_private_key_file),
                ])
                signed_export_path = run_dir / "signed-snapshot-export.json"
                write_json(signed_export_path, signed)
                if signed["returncode"] != 0:
                    raise failed_command("local signed finalized-checkpoint snapshot export failed", signed)
                signed_manifest = signed_dir / "snapshot.signed-manifest.json"
                require_file(signed_manifest, "signed snapshot manifest")
                snapshot["signed_snapshot_path"] = str(signed_dir)
                snapshot["signed_manifest_path"] = str(signed_manifest)
                snapshot["signed_manifest_sha256"] = sha256_file(signed_manifest)
                snapshot["signed_export_transcript"] = str(signed_export_path)
            imported_dir = run_dir / "verified-import"
            snapshot["local_import_data_dir"] = str(imported_dir)
            imported = run([
                str(args.local_node_bin), "snapshot-import-signed-finalized-checkpoint",
                "--data-dir", str(imported_dir), "--snapshot-dir", str(signed_dir),
                "--trusted-publisher-key-file", str(args.snapshot_publisher_public_key_file),
                "--node-id", canonical["validator_id"],
            ])
            import_path = run_dir / "signed-snapshot-import.json"
            write_json(import_path, imported)
            snapshot["signed_import_transcript"] = str(import_path)
            if imported["returncode"] != 0:
                raise failed_command("local signed finalized-checkpoint snapshot import failed", imported)
        snapshot["local_node_sha256"] = sha256_file(args.local_node_bin)
        snapshot["local_import_data_dir"] = str(imported_dir)
        status_result = run([str(args.local_node_bin), "status", "--data-dir", str(imported_dir)])
        status_path = run_dir / "signed-snapshot-import-status.json"
        write_json(status_path, status_result)
        snapshot["signed_import_status_transcript"] = str(status_path)
        if status_result["returncode"] != 0:
            raise failed_command("local imported snapshot status failed", status_result)
        status = json.loads(status_result["stdout"])
        bind_snapshot(manifest, status, parent)
        verification = run([str(args.local_node_bin), "verify-finalized-checkpoint", "--data-dir", str(imported_dir)])
        verification_path = run_dir / "finalized-checkpoint-verification.json"
        write_json(verification_path, verification)
        snapshot["verification_transcript"] = str(verification_path)
        if verification["returncode"] != 0:
            raise failed_command("local finalized-checkpoint verification failed", verification)
        audit = run([str(args.local_node_bin), "vault-bridge-conservation-audit", "--data-dir", str(imported_dir), "--asset-id", args.asset_id, "--source-rpc-url", args.source_rpc_url, "--vault-interface-lineage-manifest", str(args.vault_interface_lineage_manifest), "--cast-bin", str(args.local_cast_bin)], timeout=300)
        audit_path = run_dir / "local-audit.json"
        write_json(audit_path, audit)
        snapshot["manifest_sha256"] = sha256_file(Path(snapshot["manifest_path"]))
        raw_paths.append({"validator_id": "local-audit", "path": str(audit_path), "sha256": sha256_file(audit_path)})
        if audit["returncode"] != 0:
            violation = parse_failed_audit(audit["stderr"] + audit["stdout"], args.asset_id, parent)
            if violation is not None:
                violation["snapshot"] = snapshot
                write_json(args.output, violation, pretty=True)
                print(json.dumps({"status": "violated", "residual_atoms": violation["residual_atoms"], "output": str(args.output)}), file=sys.stderr)
                return 1
            raise failed_command("local conservation audit did not execute", audit)
        report = json.loads(audit["stdout"])
        identity = audit_identity(report, args.asset_id, parent, args.source_rpc_url, raw_paths, args.legacy_source_label, sha256_file(audit_path), legacy_finding)
        identity["snapshot"] = snapshot
        if args.opening_identity_file is not None:
            attach_opening_bracket(identity, args.opening_identity_file)
        write_json(args.output, identity, pretty=True)
        print(json.dumps({"status": identity["status"], "residual_atoms": identity["residual_atoms"], "output": str(args.output)}))
        return 0
    except (CheckerError, subprocess.TimeoutExpired, json.JSONDecodeError, OSError) as error:
        verdict = execution_blocked(
            args.asset_id, str(error), raw_paths, parent, snapshot,
            args.source_rpc_url, args.legacy_source_label, legacy_finding,
        )
        write_json(args.output, verdict, pretty=True)
        print(json.dumps({"status": "execution_blocked", "residual_atoms": None, "output": str(args.output)}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
