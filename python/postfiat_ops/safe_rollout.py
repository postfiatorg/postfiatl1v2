from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Sequence


STATE_SCHEMA = "postfiat.safe_validator_rollout.v1"
STAGE_SCHEMA = "postfiat.deployment_validator_unit_stage.v1"
INVENTORY_LINE = re.compile(
    r"^(validator-[0-5])\s+(\S+)\s+p2p=(\d+)\s+rpc=(\d+)\s+region=(\S+)\s+.*\bvultr_instance=([0-9a-f-]{36})\b"
)
IDENTIFIER = re.compile(r"^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$")
FLEET_CONVERGENCE_RETRY_ATTEMPTS = 20
FLEET_CONVERGENCE_RETRY_DELAY_SECONDS = 1.0


class SafetyError(RuntimeError):
    """A fail-closed rollout safety violation."""


@dataclass(frozen=True)
class InventoryEntry:
    validator_id: str
    host: str
    p2p_port: int
    rpc_port: int
    region: str
    instance_id: str


@dataclass(frozen=True)
class CopyEntry:
    source: Path
    target: PurePosixPath


class Runner:
    def run(
        self,
        argv: Sequence[str],
        *,
        input_text: str | None = None,
        capture: bool = True,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            list(argv),
            input=input_text,
            check=True,
            text=True,
            capture_output=capture,
        )


def reject_unsafe_cli_tokens(tokens: Sequence[str]) -> None:
    for token in tokens:
        lowered = token.lower()
        if lowered.startswith("--delete") or lowered in {"-delete", "delete"}:
            raise SafetyError(f"delete-capable option is forbidden: {token}")
        if token == "/" or token.endswith(":/"):
            raise SafetyError(f"filesystem-root destination is forbidden: {token}")
        if lowered.startswith(("--destination", "--target-root", "--rsync")):
            raise SafetyError(f"operator-supplied deployment destination is forbidden: {token}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    with temporary.open("x", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)


def parse_inventory(path: Path) -> list[InventoryEntry]:
    rows: list[InventoryEntry] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        match = INVENTORY_LINE.match(line.strip())
        if match:
            validator_id, host, p2p_port, rpc_port, region, instance_id = match.groups()
            rows.append(
                InventoryEntry(
                    validator_id,
                    host,
                    int(p2p_port),
                    int(rpc_port),
                    region,
                    instance_id,
                )
            )
    expected = [f"validator-{index}" for index in range(6)]
    actual = [row.validator_id for row in rows]
    if actual != expected:
        raise SafetyError(
            f"inventory must contain exactly {expected} in order; found {actual}"
        )
    if len({row.host for row in rows}) != 6 or len({row.instance_id for row in rows}) != 6:
        raise SafetyError("inventory hosts and Vultr instance IDs must be unique")
    return rows


def _safe_release_id(value: str) -> str:
    if not IDENTIFIER.fullmatch(value) or value in {".", ".."}:
        raise SafetyError(f"unsafe release identifier: {value!r}")
    return value


def _require_source(rootfs: Path, target: PurePosixPath) -> Path:
    if not target.is_absolute() or target == PurePosixPath("/"):
        raise SafetyError(f"deployment target is not a contained absolute path: {target}")
    source = rootfs.joinpath(*target.parts[1:]).resolve()
    root = rootfs.resolve()
    if source != root and root not in source.parents:
        raise SafetyError(f"stage source escapes rootfs: {source}")
    if not source.is_file():
        raise SafetyError(f"required staged file is missing: {source}")
    return source


def copy_entries(stage_report: Path, validator_id: str) -> tuple[str, list[CopyEntry]]:
    report = json.loads(stage_report.read_text(encoding="utf-8"))
    if report.get("schema") != STAGE_SCHEMA:
        raise SafetyError(f"unsupported stage report schema: {report.get('schema')!r}")
    release_id = _safe_release_id(str(report.get("release_id", "")))
    rootfs = Path(report.get("rootfs_dir", ""))
    if not rootfs.is_absolute() or not rootfs.is_dir():
        raise SafetyError("stage rootfs must be an existing absolute directory")
    validators = [row.get("validator_id") for row in report.get("validators", [])]
    if validators != [f"validator-{index}" for index in range(6)]:
        raise SafetyError("stage report must bind validators 0 through 5 in order")
    if validator_id not in validators:
        raise SafetyError(f"validator is not in signed stage: {validator_id}")

    config_root = PurePosixPath("/etc/postfiat/releases") / release_id
    binary_root = PurePosixPath("/opt/postfiat/releases") / release_id
    names = [
        "deployment-manifest.json",
        "deployment.public.json",
        "topology.json",
        "swap.metadata.json",
        "private-egress.metadata.json",
        f"{validator_id}.bindings.json",
        f"{validator_id}.rpc.env",
        f"{validator_id}.transport.env",
    ]
    targets = [binary_root / "postfiat-node"]
    targets.extend(config_root / name for name in names)
    targets.extend(
        [
            PurePosixPath(f"/etc/systemd/system/postfiat-{validator_id}.service"),
            PurePosixPath(f"/etc/systemd/system/postfiat-{validator_id}-rpc.service"),
        ]
    )
    entries = [CopyEntry(_require_source(rootfs, target), target) for target in targets]
    validate_copy_entries(entries, release_id, validator_id)
    return release_id, entries


def validate_copy_entries(
    entries: Iterable[CopyEntry], release_id: str, validator_id: str
) -> None:
    allowed_prefixes = (
        PurePosixPath("/opt/postfiat/releases") / release_id,
        PurePosixPath("/etc/postfiat/releases") / release_id,
    )
    allowed_units = {
        PurePosixPath(f"/etc/systemd/system/postfiat-{validator_id}.service"),
        PurePosixPath(f"/etc/systemd/system/postfiat-{validator_id}-rpc.service"),
    }
    seen: set[PurePosixPath] = set()
    for entry in entries:
        target = entry.target
        contained = any(prefix == target or prefix in target.parents for prefix in allowed_prefixes)
        if not contained and target not in allowed_units:
            raise SafetyError(f"deployment target is not allowlisted: {target}")
        if target in seen:
            raise SafetyError(f"duplicate deployment target: {target}")
        seen.add(target)


def build_diff(
    entries: Sequence[CopyEntry], remote_hashes: dict[str, str | None]
) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    expected_paths = {str(entry.target) for entry in entries}
    unexpected = set(remote_hashes) - expected_paths
    if unexpected:
        raise SafetyError(f"remote diff returned non-allowlisted paths: {sorted(unexpected)}")
    for entry in entries:
        path = str(entry.target)
        local_hash = sha256_file(entry.source)
        remote_hash = remote_hashes.get(path)
        action = "create" if remote_hash is None else "unchanged" if remote_hash == local_hash else "update"
        rows.append(
            {
                "action": action,
                "path": path,
                "local_sha256": local_hash,
                "remote_sha256": remote_hash or "",
            }
        )
    validate_diff(rows, expected_paths)
    return rows


def validate_diff(rows: Sequence[dict[str, str]], allowed_paths: set[str]) -> None:
    for row in rows:
        if row.get("action") == "delete":
            raise SafetyError(f"deployment preflight proposed a deletion: {row.get('path')}")
        if row.get("action") not in {"create", "update", "unchanged"}:
            raise SafetyError(f"unsupported deployment action: {row.get('action')}")
        if row.get("path") not in allowed_paths:
            raise SafetyError(f"deployment diff path is not allowlisted: {row.get('path')}")


def rollout_order(canary: str) -> list[str]:
    expected = [f"validator-{index}" for index in range(6)]
    if canary not in expected:
        raise SafetyError(f"invalid canary validator: {canary}")
    return [canary, *[validator for validator in expected if validator != canary]]


def next_validator(state: dict[str, Any]) -> str:
    if state.get("schema") != STATE_SCHEMA:
        raise SafetyError("unsupported or missing rollout state schema")
    if not state.get("preflight", {}).get("verified"):
        raise SafetyError("apply-next requires a completed preflight")
    if not state.get("backup", {}).get("verified"):
        raise SafetyError("apply-next requires a verified signed backup")
    order = state.get("order")
    applied = state.get("applied", [])
    if not isinstance(order, list) or not isinstance(applied, list):
        raise SafetyError("rollout state order/applied fields are invalid")
    if applied != order[: len(applied)]:
        raise SafetyError("rollout state is not a strict canary-first prefix")
    if len(applied) >= len(order):
        raise SafetyError("rollout is already complete")
    return str(order[len(applied)])


def _ssh_target(entry: InventoryEntry, user: str) -> str:
    if not IDENTIFIER.fullmatch(user):
        raise SafetyError(f"unsafe SSH user: {user!r}")
    return f"{user}@{entry.host}"


def remote_hashes(
    runner: Runner, inventory: InventoryEntry, entries: Sequence[CopyEntry], user: str
) -> dict[str, str | None]:
    script_lines = ["set -eu"]
    for entry in entries:
        quoted = shlex.quote(str(entry.target))
        script_lines.append(
            f"if test -f {quoted}; then printf '%s\\t%s\\n' {quoted} \"$(sha256sum {quoted} | cut -d' ' -f1)\"; "
            f"else printf '%s\\tMISSING\\n' {quoted}; fi"
        )
    result = runner.run(
        ["ssh", "-o", "BatchMode=yes", _ssh_target(inventory, user), "bash", "-s"],
        input_text="\n".join(script_lines) + "\n",
    )
    hashes: dict[str, str | None] = {}
    for line in result.stdout.splitlines():
        path, value = line.split("\t", 1)
        hashes[path] = None if value == "MISSING" else value
    return hashes


def query_vultr_inventory(
    inventory: Sequence[InventoryEntry], api_key_file: Path
) -> list[dict[str, str]]:
    api_key = api_key_file.read_text(encoding="utf-8").strip()
    if not api_key or any(character.isspace() for character in api_key):
        raise SafetyError("Vultr API key file must contain exactly one non-empty token")
    verified: list[dict[str, str]] = []
    for row in inventory:
        request = urllib.request.Request(
            f"https://api.vultr.com/v2/instances/{row.instance_id}",
            headers={"Authorization": f"Bearer {api_key}"},
        )
        with urllib.request.urlopen(request, timeout=15) as response:
            instance = json.load(response)["instance"]
        observed = {
            "validator_id": row.validator_id,
            "instance_id": str(instance.get("id", "")),
            "host": str(instance.get("main_ip", "")),
            "region": str(instance.get("region", "")),
            "status": str(instance.get("status", "")),
            "power_status": str(instance.get("power_status", "")),
        }
        expected = (row.instance_id, row.host, row.region, "active", "running")
        actual = (
            observed["instance_id"],
            observed["host"],
            observed["region"],
            observed["status"],
            observed["power_status"],
        )
        if actual != expected:
            raise SafetyError(
                f"inventory mismatch for {row.validator_id}: expected {expected}, observed {actual}"
            )
        verified.append(observed)
    return verified


def _fleet_convergence_once(inventory: Sequence[InventoryEntry]) -> dict[str, Any]:
    rows: list[dict[str, Any]] = []
    for index, entry in enumerate(inventory):
        request = {
            "version": "postfiat-local-rpc-v1",
            "id": f"safe-rollout-{index}",
            "method": "status",
            "params": {},
        }
        with socket.create_connection((entry.host, entry.rpc_port), timeout=10) as connection:
            stream = connection.makefile("rwb")
            stream.write((json.dumps(request, separators=(",", ":")) + "\n").encode())
            stream.flush()
            line = stream.readline(8 * 1024 * 1024)
        if not line:
            raise SafetyError(f"empty status RPC response from {entry.validator_id}")
        response = json.loads(line)
        if response.get("ok") is not True:
            raise SafetyError(f"status RPC failed for {entry.validator_id}: {response.get('error')}")
        result = response.get("result", {})
        row = {
            "validator_id": entry.validator_id,
            "height": result.get("block_height"),
            "tip": result.get("block_tip_hash"),
            "state_root": result.get("state_root"),
            "mempool_pending": result.get("mempool_pending"),
        }
        if row["mempool_pending"] != 0:
            raise SafetyError(f"non-empty mempool on {entry.validator_id}")
        rows.append(row)
    identities = {
        (row["height"], row["tip"], row["state_root"]) for row in rows
    }
    if len(identities) != 1:
        raise SafetyError(f"fleet ledger divergence: {rows}")
    height, tip, state_root = next(iter(identities))
    return {
        "verified": True,
        "validator_count": len(rows),
        "height": height,
        "tip": tip,
        "state_root": state_root,
        "validators": rows,
    }


def fleet_convergence(inventory: Sequence[InventoryEntry]) -> dict[str, Any]:
    """Wait only for transient RPC reachability; never retry bad ledger data."""
    for attempt in range(FLEET_CONVERGENCE_RETRY_ATTEMPTS):
        try:
            return _fleet_convergence_once(inventory)
        except OSError:
            if attempt + 1 == FLEET_CONVERGENCE_RETRY_ATTEMPTS:
                raise
            time.sleep(FLEET_CONVERGENCE_RETRY_DELAY_SECONDS)
    raise AssertionError("unreachable fleet convergence retry state")


def verify_remote_committee_rosters(
    runner: Runner,
    inventory: Sequence[InventoryEntry],
    user: str,
) -> list[dict[str, Any]]:
    """Fail closed unless every running validator has the exact active key roster.

    The current devnet signer file also supplies certificate-verification material.
    A node with only its own record can sign but cannot verify a quorum certificate,
    so this check must run before a rolling deployment mutates any host.
    """
    expected_count = len(inventory)
    reports: list[dict[str, Any]] = []
    for row in inventory:
        service = f"postfiat-{row.validator_id}.service"
        data_dir = f"/var/lib/postfiat/{row.validator_id}"
        remote_script = (
            "set -eu\n"
            f"active_pid=$(systemctl show --property=MainPID --value {shlex.quote(service)})\n"
            "case \"$active_pid\" in ''|*[!0-9]*|0) exit 97;; esac\n"
            "active_binary=$(readlink -f \"/proc/$active_pid/exe\")\n"
            "case \"$active_binary\" in /opt/postfiat/releases/*/postfiat-node) ;; *) exit 98;; esac\n"
            "test -x \"$active_binary\"\n"
            f"\"$active_binary\" validate-local-keys --data-dir {shlex.quote(data_dir)} "
            f"--validators {expected_count}\n"
        )
        result = runner.run(
            ["ssh", "-o", "BatchMode=yes", _ssh_target(row, user), "bash", "-s"],
            input_text=remote_script,
        )
        try:
            report = json.loads(result.stdout)
        except json.JSONDecodeError as error:
            raise SafetyError(
                f"committee-roster validation returned invalid JSON for {row.validator_id}"
            ) from error
        expected = {
            "schema": "postfiat-local-key-validation-v1",
            "node_id": row.validator_id,
            "validator_keys_valid": True,
            "validator_key_permissions_valid": True,
            "validator_key_count": expected_count,
            "required_validator_count": expected_count,
        }
        observed = {key: report.get(key) for key in expected}
        if observed != expected:
            raise SafetyError(
                f"incomplete or invalid committee roster on {row.validator_id}: "
                f"expected {expected}, observed {observed}"
            )
        reports.append(observed)
    return reports


def _entry_by_id(inventory: Sequence[InventoryEntry], validator_id: str) -> InventoryEntry:
    for row in inventory:
        if row.validator_id == validator_id:
            return row
    raise SafetyError(f"validator is absent from inventory: {validator_id}")


def verify_local_stage(
    runner: Runner, stage_report: Path, validator_id: str
) -> None:
    release_id, entries = copy_entries(stage_report, validator_id)
    sources = {str(entry.target): entry.source for entry in entries}
    config = PurePosixPath("/etc/postfiat/releases") / release_id
    binary_target = PurePosixPath("/opt/postfiat/releases") / release_id / "postfiat-node"
    binding_target = config / f"{validator_id}.bindings.json"
    binding = json.loads(sources[str(binding_target)].read_text(encoding="utf-8"))
    for validator in binding.get("validators", []):
        for service in validator.get("services", []):
            for field in ("service_unit_file", "environment_file"):
                target = service.get(field)
                if target not in sources:
                    raise SafetyError(
                        f"{validator_id} binding names a non-staged runtime file: {target}"
                    )
                service[field] = str(sources[target])
    with tempfile.TemporaryDirectory(prefix="postfiat-safe-rollout-bindings-") as temporary:
        local_binding = Path(temporary) / f"{validator_id}.bindings.json"
        local_binding.write_text(json.dumps(binding), encoding="utf-8")
        runner.run(
            [
                str(sources[str(binary_target)]),
                "deployment-manifest-verify",
                "--manifest-file",
                str(sources[str(config / "deployment-manifest.json")]),
                "--trusted-publisher-key-file",
                str(sources[str(config / "deployment.public.json")]),
                "--validator-id",
                validator_id,
                "--validator-bindings-file",
                str(local_binding),
                "--runtime-binary-file",
                str(sources[str(binary_target)]),
                "--runtime-topology-file",
                str(sources[str(config / "topology.json")]),
                "--runtime-swap-circuit-metadata-file",
                str(sources[str(config / "swap.metadata.json")]),
                "--runtime-private-egress-circuit-metadata-file",
                str(sources[str(config / "private-egress.metadata.json")]),
            ]
        )


def verify_frozen_inputs(state: dict[str, Any]) -> None:
    stage_report = Path(state["stage_report"])
    inventory_file = Path(state["inventory_file"])
    if sha256_file(stage_report) != state.get("stage_report_sha256"):
        raise SafetyError("stage report changed after preflight")
    if sha256_file(inventory_file) != state.get("inventory_file_sha256"):
        raise SafetyError("inventory changed after preflight")


def preflight(args: argparse.Namespace, runner: Runner) -> dict[str, Any]:
    if args.state_file.exists():
        raise SafetyError(f"rollout state already exists: {args.state_file}")
    inventory = parse_inventory(args.inventory_file)
    cloud = query_vultr_inventory(inventory, args.vultr_api_key_file)
    convergence = fleet_convergence(inventory)
    committee_rosters = verify_remote_committee_rosters(
        runner, inventory, args.ssh_user
    )
    order = rollout_order(args.canary_validator_id)
    all_diffs: dict[str, list[dict[str, str]]] = {}
    release_id = ""
    for row in inventory:
        verify_local_stage(runner, args.stage_report, row.validator_id)
        current_release, entries = copy_entries(args.stage_report, row.validator_id)
        if release_id and current_release != release_id:
            raise SafetyError("stage report release changed during preflight")
        release_id = current_release
        all_diffs[row.validator_id] = build_diff(
            entries, remote_hashes(runner, row, entries, args.ssh_user)
        )
    state = {
        "schema": STATE_SCHEMA,
        "release_id": release_id,
        "stage_report": str(args.stage_report.resolve()),
        "stage_report_sha256": sha256_file(args.stage_report),
        "inventory_file": str(args.inventory_file.resolve()),
        "inventory_file_sha256": sha256_file(args.inventory_file),
        "ssh_user": args.ssh_user,
        "canary_validator_id": args.canary_validator_id,
        "order": order,
        "applied": [],
        "preflight": {
            "verified": True,
            "created_unix": int(time.time()),
            "inventory": cloud,
            "fleet_convergence": convergence,
            "committee_rosters": committee_rosters,
            "diff": all_diffs,
            "deletion_count": 0,
        },
        "backup": {"verified": False},
    }
    atomic_write_json(args.state_file, state)
    return state


def create_backup(args: argparse.Namespace, runner: Runner) -> dict[str, Any]:
    state = json.loads(args.state_file.read_text(encoding="utf-8"))
    verify_frozen_inputs(state)
    if not state.get("preflight", {}).get("verified"):
        raise SafetyError("backup requires a completed preflight")
    if state.get("applied"):
        raise SafetyError("backup must complete before any validator is deployed")
    if state.get("backup", {}).get("verified"):
        raise SafetyError("signed backup is already recorded")
    inventory = parse_inventory(Path(state["inventory_file"]))
    canary = str(state["canary_validator_id"])
    row = _entry_by_id(inventory, canary)
    release_id, entries = copy_entries(Path(state["stage_report"]), canary)
    binary = next(entry.source for entry in entries if entry.target.name == "postfiat-node")
    remote_dir = PurePosixPath("/var/lib/postfiat/pre-rollout-snapshots") / f"{release_id}-{canary}"
    if PurePosixPath("/var/lib/postfiat/pre-rollout-snapshots") not in remote_dir.parents:
        raise SafetyError("backup destination escaped the dedicated snapshot directory")
    target = _ssh_target(row, str(state["ssh_user"]))
    service_name = f"postfiat-{canary}.service"
    backup_lock = PurePosixPath("/var/lib/postfiat/pre-rollout-snapshots") / (
        f".{release_id}-{canary}.lock"
    )
    remote_script = (
        "set -eu\n"
        "install -d -o postfiat -g postfiat -m 0750 /var/lib/postfiat/pre-rollout-snapshots\n"
        f"mkdir {shlex.quote(str(backup_lock))}\n"
        f"trap 'rmdir {shlex.quote(str(backup_lock))}' EXIT\n"
        f"test ! -e {shlex.quote(str(remote_dir))}\n"
        f"active_pid=$(systemctl show --property=MainPID --value {shlex.quote(service_name)})\n"
        "case \"$active_pid\" in ''|*[!0-9]*|0) exit 97;; esac\n"
        "active_binary=$(readlink -f \"/proc/$active_pid/exe\")\n"
        "case \"$active_binary\" in /opt/postfiat/releases/*/postfiat-node) ;; *) exit 98;; esac\n"
        "test -x \"$active_binary\"\n"
        "\"$active_binary\" "
        f"snapshot-export --data-dir /var/lib/postfiat/{canary} --snapshot-dir {shlex.quote(str(remote_dir))}\n"
    )
    runner.run(["ssh", "-o", "BatchMode=yes", target, "bash", "-s"], input_text=remote_script)
    unsigned = args.evidence_dir / "backup-unsigned"
    signed = args.evidence_dir / "backup-signed"
    verify_dir = args.evidence_dir / "backup-verified-import"
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    runner.run(["scp", "-q", "-r", f"{target}:{remote_dir}/.", str(unsigned)])
    runner.run(
        [
            str(binary),
            "snapshot-export-signed",
            "--data-dir",
            str(unsigned),
            "--snapshot-dir",
            str(signed),
            "--publisher-key-file",
            str(args.snapshot_publisher_key_file),
        ]
    )
    trusted = args.snapshot_publisher_public_key_file
    runner.run(
        [
            str(binary),
            "snapshot-import-signed",
            "--data-dir",
            str(verify_dir),
            "--snapshot-dir",
            str(signed),
            "--trusted-publisher-key-file",
            str(trusted),
            "--node-id",
            canary,
        ]
    )
    verification = runner.run(
        [str(binary), "verify-state", "--data-dir", str(verify_dir)]
    )
    report = json.loads(verification.stdout)
    if report.get("verified") is not True:
        raise SafetyError("signed pre-rollout backup did not pass verify-state")
    chain_tip = json.loads((verify_dir / "chain_tip.json").read_text(encoding="utf-8"))
    state_root = str(chain_tip.get("state_root", ""))
    if not state_root:
        raise SafetyError("verified signed backup is missing its chain-tip state root")
    manifest = signed / "snapshot.signed-manifest.json"
    state["backup"] = {
        "verified": True,
        "created_unix": int(time.time()),
        "source_validator": canary,
        "remote_unsigned_snapshot": str(remote_dir),
        "signed_snapshot": str(signed.resolve()),
        "signed_manifest_sha256": sha256_file(manifest),
        "height": chain_tip.get("height"),
        "tip": chain_tip.get("block_hash", ""),
        "state_root": state_root,
    }
    atomic_write_json(args.state_file, state)
    return state


def _copy_release(
    runner: Runner,
    row: InventoryEntry,
    user: str,
    release_id: str,
    entries: Sequence[CopyEntry],
) -> None:
    target = _ssh_target(row, user)
    config_dir = f"/etc/postfiat/releases/{release_id}"
    binary_dir = f"/opt/postfiat/releases/{release_id}"
    runner.run(
        ["ssh", "-o", "BatchMode=yes", target, "install", "-d", "-o", "root", "-g", "root", "-m", "0755", binary_dir, config_dir]
    )
    promotions = ["set -eu"]
    for entry in entries:
        incoming = PurePosixPath(f"{entry.target}.incoming-safe-rollout")
        runner.run(["scp", "-q", str(entry.source), f"{target}:{incoming}"])
        expected_hash = sha256_file(entry.source)
        promotions.extend(
            [
                f"test \"$(sha256sum {shlex.quote(str(incoming))} | cut -d' ' -f1)\" = {shlex.quote(expected_hash)}",
                f"mv -T {shlex.quote(str(incoming))} {shlex.quote(str(entry.target))}",
            ]
        )
    runner.run(
        ["ssh", "-o", "BatchMode=yes", target, "bash", "-s"],
        input_text="\n".join(promotions) + "\n",
    )


def apply_next(args: argparse.Namespace, runner: Runner) -> dict[str, Any]:
    state = json.loads(args.state_file.read_text(encoding="utf-8"))
    verify_frozen_inputs(state)
    validator_id = next_validator(state)
    inventory = parse_inventory(Path(state["inventory_file"]))
    # Re-check every live roster immediately before any host mutation. The
    # preflight evidence is not permission to trust mutable data directories.
    committee_rosters = verify_remote_committee_rosters(
        runner, inventory, str(state["ssh_user"])
    )
    row = _entry_by_id(inventory, validator_id)
    release_id, entries = copy_entries(Path(state["stage_report"]), validator_id)
    if release_id != state.get("release_id"):
        raise SafetyError("release ID differs from preflight state")
    # Re-check the exact target diff immediately before mutation.
    diff = build_diff(entries, remote_hashes(runner, row, entries, str(state["ssh_user"])))
    _copy_release(runner, row, str(state["ssh_user"]), release_id, entries)
    target = _ssh_target(row, str(state["ssh_user"]))
    config = f"/etc/postfiat/releases/{release_id}"
    binary = f"/opt/postfiat/releases/{release_id}/postfiat-node"
    verify = (
        f"{binary} deployment-manifest-verify --manifest-file {config}/deployment-manifest.json "
        f"--trusted-publisher-key-file {config}/deployment.public.json --validator-id {validator_id} "
        f"--validator-bindings-file {config}/{validator_id}.bindings.json --runtime-binary-file {binary} "
        f"--runtime-topology-file {config}/topology.json --runtime-swap-circuit-metadata-file {config}/swap.metadata.json "
        f"--runtime-private-egress-circuit-metadata-file {config}/private-egress.metadata.json"
    )
    remote_script = (
        "set -eu\n"
        f"chmod 0755 {binary}\n"
        f"chmod 0644 {config}/* /etc/systemd/system/postfiat-{validator_id}.service /etc/systemd/system/postfiat-{validator_id}-rpc.service\n"
        f"{verify} >/dev/null\n"
        "systemctl daemon-reload\n"
        f"systemctl enable postfiat-{validator_id}.service postfiat-{validator_id}-rpc.service >/dev/null\n"
        f"systemctl restart postfiat-{validator_id}.service\n"
        f"systemctl restart postfiat-{validator_id}-rpc.service\n"
        f"test \"$(systemctl is-active postfiat-{validator_id}.service)\" = active\n"
        f"test \"$(systemctl is-active postfiat-{validator_id}-rpc.service)\" = active\n"
        f"{binary} status --data-dir /var/lib/postfiat/{validator_id}\n"
    )
    result = runner.run(
        ["ssh", "-o", "BatchMode=yes", target, "bash", "-s"], input_text=remote_script
    )
    status = json.loads(result.stdout)
    if status.get("mempool_pending") != 0:
        raise SafetyError(f"{validator_id} restarted with a non-empty mempool")
    convergence = fleet_convergence(inventory)
    state["applied"].append(validator_id)
    state.setdefault("apply_reports", {})[validator_id] = {
        "completed_unix": int(time.time()),
        "diff": diff,
        "height": status.get("block_height"),
        "tip": status.get("block_tip_hash"),
        "state_root": status.get("state_root"),
        "mempool_pending": status.get("mempool_pending"),
        "pre_mutation_committee_rosters": committee_rosters,
        "fleet_convergence": convergence,
    }
    atomic_write_json(args.state_file, state)
    return state


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(
        description="Fail-closed, canary-first PostFiat validator rollout"
    )
    subparsers = result.add_subparsers(dest="command", required=True)
    preflight_parser = subparsers.add_parser("preflight")
    preflight_parser.add_argument("--stage-report", type=Path, required=True)
    preflight_parser.add_argument("--inventory-file", type=Path, required=True)
    preflight_parser.add_argument("--vultr-api-key-file", type=Path, required=True)
    preflight_parser.add_argument("--state-file", type=Path, required=True)
    preflight_parser.add_argument("--canary-validator-id", default="validator-1")
    preflight_parser.add_argument("--ssh-user", default="root")
    backup_parser = subparsers.add_parser("backup")
    backup_parser.add_argument("--state-file", type=Path, required=True)
    backup_parser.add_argument("--evidence-dir", type=Path, required=True)
    backup_parser.add_argument("--snapshot-publisher-key-file", type=Path, required=True)
    backup_parser.add_argument("--snapshot-publisher-public-key-file", type=Path, required=True)
    apply_parser = subparsers.add_parser("apply-next")
    apply_parser.add_argument("--state-file", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None, runner: Runner | None = None) -> int:
    tokens = list(sys.argv[1:] if argv is None else argv)
    try:
        reject_unsafe_cli_tokens(tokens)
        args = parser().parse_args(tokens)
        active_runner = runner or Runner()
        if args.command == "preflight":
            report = preflight(args, active_runner)
        elif args.command == "backup":
            report = create_backup(args, active_runner)
        else:
            report = apply_next(args, active_runner)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0
    except (SafetyError, OSError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"safe rollout refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
