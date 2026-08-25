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
from contextlib import contextmanager
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


@contextmanager
def prefer_ipv4_dns() -> Iterable[None]:
    """Use the provider ACL's IPv4 path without changing process-wide DNS permanently."""
    original = socket.getaddrinfo

    def ipv4_getaddrinfo(host: str, port: int, *args: Any, **kwargs: Any) -> list[Any]:
        results = original(host, port, *args, **kwargs)
        ipv4 = [result for result in results if result[0] == socket.AF_INET]
        return ipv4 or results

    socket.getaddrinfo = ipv4_getaddrinfo
    try:
        yield
    finally:
        socket.getaddrinfo = original


def query_vultr_inventory(
    inventory: Sequence[InventoryEntry], api_key_file: Path
) -> list[dict[str, str]]:
    api_key = api_key_file.read_text(encoding="utf-8").strip()
    if not api_key or any(character.isspace() for character in api_key):
        raise SafetyError("Vultr API key file must contain exactly one non-empty token")
    verified: list[dict[str, str]] = []
    with prefer_ipv4_dns():
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


def rpc_tunnel_endpoints(
    inventory: Sequence[InventoryEntry], base_port: int | None
) -> dict[str, tuple[str, int]] | None:
    if base_port is None:
        return None
    if base_port < 1024 or base_port + len(inventory) - 1 > 65535:
        raise SafetyError("RPC tunnel base port must map the fleet within 1024..65535")
    return {
        entry.validator_id: ("127.0.0.1", base_port + index)
        for index, entry in enumerate(inventory)
    }


def _fleet_convergence_once(
    inventory: Sequence[InventoryEntry],
    endpoints: dict[str, tuple[str, int]] | None = None,
) -> dict[str, Any]:
    rows: list[dict[str, Any]] = []
    for index, entry in enumerate(inventory):
        request = {
            "version": "postfiat-local-rpc-v1",
            "id": f"safe-rollout-{index}",
            "method": "status",
            "params": {},
        }
        endpoint = (
            endpoints[entry.validator_id]
            if endpoints is not None
            else (entry.host, entry.rpc_port)
        )
        with socket.create_connection(endpoint, timeout=10) as connection:
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


def fleet_convergence(
    inventory: Sequence[InventoryEntry],
    endpoints: dict[str, tuple[str, int]] | None = None,
) -> dict[str, Any]:
    """Wait only for transient RPC reachability; never retry bad ledger data."""
    for attempt in range(FLEET_CONVERGENCE_RETRY_ATTEMPTS):
        try:
            if endpoints is None:
                return _fleet_convergence_once(inventory)
            return _fleet_convergence_once(inventory, endpoints)
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
    """Fail closed unless each split signer matches one complete active registry."""
    expected_count = len(inventory)
    expected_ids = [row.validator_id for row in inventory]
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
            f"python3 - \"$active_binary\" {shlex.quote(data_dir)} "
            f"{shlex.quote(row.validator_id)} {expected_count} <<'PY'\n"
            "import json\n"
            "import pathlib\n"
            "import subprocess\n"
            "import sys\n"
            "binary, data_dir, node_id, expected_raw = sys.argv[1:]\n"
            "expected = int(expected_raw)\n"
            "local = json.loads(subprocess.run(\n"
            "    [binary, 'validate-local-keys', '--data-dir', data_dir,\n"
            "     '--validators', str(expected), '--local-only'],\n"
            "    check=True, capture_output=True, text=True,\n"
            ").stdout)\n"
            "registry_response = json.loads(subprocess.run(\n"
            "    [binary, 'rpc', '--method', 'validators', '--data-dir', data_dir],\n"
            "    check=True, capture_output=True, text=True,\n"
            ").stdout)\n"
            "registry = registry_response.get('result', {})\n"
            "private_file = json.loads(\n"
            "    (pathlib.Path(data_dir) / 'validator_keys.json').read_text()\n"
            ")\n"
            "private_records = private_file.get('validators', [])\n"
            "registry_records = registry.get('validators', [])\n"
            "local_record = next(\n"
            "    (record for record in private_records if record.get('node_id') == node_id),\n"
            "    None,\n"
            ")\n"
            "registry_record = next(\n"
            "    (record for record in registry_records if record.get('node_id') == node_id),\n"
            "    None,\n"
            ")\n"
            "report = {\n"
            "    'schema': 'postfiat-safe-rollout-committee-roster-v1',\n"
            "    'node_id': node_id,\n"
            "    'local_signer_valid': (\n"
            "        local.get('validator_keys_valid') is True\n"
            "        and local.get('validator_key_permissions_valid') is True\n"
            "        and local.get('validator_key_count') == 1\n"
            "        and local.get('required_validator_count') == 1\n"
            "    ),\n"
            "    'local_signer_matches_registry': (\n"
            "        local_record is not None\n"
            "        and registry_record is not None\n"
            "        and local_record.get('public_key_hex')\n"
            "            == registry_record.get('public_key_hex')\n"
            "    ),\n"
            "    'registry_root': registry.get('registry_root'),\n"
            "    'registry_validator_count': registry.get('validator_count'),\n"
            "    'registry_validator_ids': [\n"
            "        record.get('node_id') for record in registry_records\n"
            "    ],\n"
            "}\n"
            "print(json.dumps(report, sort_keys=True))\n"
            "PY\n"
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
            "schema": "postfiat-safe-rollout-committee-roster-v1",
            "node_id": row.validator_id,
            "local_signer_valid": True,
            "local_signer_matches_registry": True,
            "registry_validator_count": expected_count,
            "registry_validator_ids": expected_ids,
        }
        observed = {key: report.get(key) for key in expected}
        if observed != expected:
            raise SafetyError(
                f"incomplete or invalid committee roster on {row.validator_id}: "
                f"expected {expected}, observed {observed}"
            )
        reports.append(observed)
        reports[-1]["registry_root"] = report.get("registry_root")
    roots = {report.get("registry_root") for report in reports}
    if None in roots or "" in roots or len(roots) != 1:
        raise SafetyError(f"validator registry roots are incomplete or divergent: {roots}")
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
    rpc_tunnel_base_port = getattr(args, "rpc_tunnel_base_port", None)
    endpoints = rpc_tunnel_endpoints(inventory, rpc_tunnel_base_port)
    convergence = fleet_convergence(inventory, endpoints)
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
        "rpc_tunnel_base_port": rpc_tunnel_base_port,
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


def _verify_and_record_backup(
    *,
    args: argparse.Namespace,
    runner: Runner,
    state: dict[str, Any],
    binary: Path,
    binary_sha256: str,
    remote_dir: PurePosixPath,
) -> dict[str, Any]:
    signed = args.evidence_dir / "backup-signed"
    verify_dir = args.evidence_dir / "backup-verified-import"
    manifest = signed / "snapshot.signed-manifest.json"
    if not manifest.is_file():
        raise SafetyError("signed pre-rollout backup manifest is missing")
    if verify_dir.exists():
        raise SafetyError(
            "backup verification destination already exists; refusing to overwrite it"
        )
    trusted = args.snapshot_publisher_public_key_file
    if not trusted.is_file():
        raise SafetyError("trusted snapshot publisher public key is missing")
    canary = str(state["canary_validator_id"])
    runner.run(
        [
            str(binary),
            "snapshot-import-signed-finalized-checkpoint",
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
        [str(binary), "verify-finalized-checkpoint", "--data-dir", str(verify_dir)]
    )
    report = json.loads(verification.stdout)
    if report.get("verified") is not True:
        raise SafetyError(
            "signed pre-rollout backup did not pass finalized-checkpoint verification"
        )
    verification_report_file = args.evidence_dir / "finalized-checkpoint-verification.json"
    atomic_write_json(verification_report_file, report)
    status_result = runner.run([str(binary), "status", "--data-dir", str(verify_dir)])
    chain_tip = json.loads(status_result.stdout)
    state_root = str(chain_tip.get("state_root", ""))
    if not state_root:
        raise SafetyError("verified signed backup is missing its chain-tip state root")
    state["backup"] = {
        "verified": True,
        "created_unix": int(time.time()),
        "source_validator": canary,
        "remote_unsigned_snapshot": str(remote_dir),
        "signed_snapshot": str(signed.resolve()),
        "signed_manifest_sha256": sha256_file(manifest),
        "candidate_binary_sha256": binary_sha256,
        "verification_basis": report.get("verification_basis"),
        "consensus_v2_activation_height": report.get(
            "consensus_v2_activation_height"
        ),
        "certificate_id": report.get("certificate_id"),
        "finalized_checkpoint_verification_sha256": sha256_file(
            verification_report_file
        ),
        "height": chain_tip.get("block_height"),
        "tip": chain_tip.get("block_tip_hash", ""),
        "state_root": state_root,
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
    binary_sha256 = sha256_file(binary)
    snapshot_root = PurePosixPath("/var/lib/postfiat/pre-rollout-snapshots")
    remote_dir = snapshot_root / f"{release_id}-{canary}-finalized-checkpoint"
    remote_candidate = snapshot_root / f".{release_id}-{canary}.candidate-postfiat-node"
    remote_candidate_incoming = snapshot_root / (
        f".{release_id}-{canary}.candidate-postfiat-node.incoming"
    )
    if PurePosixPath("/var/lib/postfiat/pre-rollout-snapshots") not in remote_dir.parents:
        raise SafetyError("backup destination escaped the dedicated snapshot directory")
    target = _ssh_target(row, str(state["ssh_user"]))
    service_name = f"postfiat-{canary}.service"
    backup_lock = PurePosixPath("/var/lib/postfiat/pre-rollout-snapshots") / (
        f".{release_id}-{canary}.lock"
    )
    runner.run(
        [
            "ssh",
            "-o",
            "BatchMode=yes",
            target,
            "install",
            "-d",
            "-o",
            "postfiat",
            "-g",
            "postfiat",
            "-m",
            "0750",
            str(snapshot_root),
        ]
    )
    runner.run(["scp", "-q", str(binary), f"{target}:{remote_candidate_incoming}"])
    remote_script = (
        "set -eu\n"
        "install -d -o postfiat -g postfiat -m 0750 /var/lib/postfiat/pre-rollout-snapshots\n"
        f"mkdir {shlex.quote(str(backup_lock))}\n"
        f"trap 'rmdir {shlex.quote(str(backup_lock))}' EXIT\n"
        f"test ! -e {shlex.quote(str(remote_dir))}\n"
        f"test \"$(sha256sum {shlex.quote(str(remote_candidate_incoming))} | cut -d' ' -f1)\" = {shlex.quote(binary_sha256)}\n"
        f"chmod 0755 {shlex.quote(str(remote_candidate_incoming))}\n"
        f"mv -T {shlex.quote(str(remote_candidate_incoming))} {shlex.quote(str(remote_candidate))}\n"
        f"test \"$(sha256sum {shlex.quote(str(remote_candidate))} | cut -d' ' -f1)\" = {shlex.quote(binary_sha256)}\n"
        f"active_pid=$(systemctl show --property=MainPID --value {shlex.quote(service_name)})\n"
        "case \"$active_pid\" in ''|*[!0-9]*|0) exit 97;; esac\n"
        "active_binary=$(readlink -f \"/proc/$active_pid/exe\")\n"
        "case \"$active_binary\" in /opt/postfiat/releases/*/postfiat-node) ;; *) exit 98;; esac\n"
        "test -x \"$active_binary\"\n"
        f"\"$active_binary\" snapshot-export-finalized-checkpoint "
        f"--data-dir /var/lib/postfiat/{canary} "
        f"--snapshot-dir {shlex.quote(str(remote_dir))}\n"
    )
    runner.run(["ssh", "-o", "BatchMode=yes", target, "bash", "-s"], input_text=remote_script)
    unsigned = args.evidence_dir / "backup-unsigned"
    migrated = args.evidence_dir / "backup-migrated-source"
    signed = args.evidence_dir / "backup-signed"
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    runner.run(["scp", "-q", "-r", f"{target}:{remote_dir}/.", str(unsigned)])
    runner.run(
        [
            str(binary),
            "snapshot-import-finalized-checkpoint",
            "--data-dir",
            str(migrated),
            "--snapshot-dir",
            str(unsigned),
            "--node-id",
            canary,
        ]
    )
    runner.run(
        [
            str(binary),
            "snapshot-export-signed-finalized-checkpoint",
            "--data-dir",
            str(migrated),
            "--snapshot-dir",
            str(signed),
            "--publisher-key-file",
            str(args.snapshot_publisher_key_file),
        ]
    )
    return _verify_and_record_backup(
        args=args,
        runner=runner,
        state=state,
        binary=binary,
        binary_sha256=binary_sha256,
        remote_dir=remote_dir,
    )


def resume_backup_verification(
    args: argparse.Namespace, runner: Runner
) -> dict[str, Any]:
    state = json.loads(args.state_file.read_text(encoding="utf-8"))
    verify_frozen_inputs(state)
    if not state.get("preflight", {}).get("verified"):
        raise SafetyError("backup recovery requires a completed preflight")
    if state.get("applied"):
        raise SafetyError("backup recovery must complete before any validator is deployed")
    if state.get("backup", {}).get("verified"):
        raise SafetyError("signed backup is already recorded")
    canary = str(state["canary_validator_id"])
    release_id, entries = copy_entries(Path(state["stage_report"]), canary)
    binary = next(entry.source for entry in entries if entry.target.name == "postfiat-node")
    remote_dir = (
        PurePosixPath("/var/lib/postfiat/pre-rollout-snapshots")
        / f"{release_id}-{canary}-finalized-checkpoint"
    )
    return _verify_and_record_backup(
        args=args,
        runner=runner,
        state=state,
        binary=binary,
        binary_sha256=sha256_file(binary),
        remote_dir=remote_dir,
    )


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
    migration_report = (
        f"/var/lib/postfiat/pre-rollout-snapshots/{release_id}-{validator_id}-storage-migration.json"
    )
    checkpoint_report = (
        f"/var/lib/postfiat/pre-rollout-snapshots/{release_id}-{validator_id}-checkpoint.json"
    )
    remote_script = (
        "set -eu\n"
        f"chmod 0755 {binary}\n"
        f"chmod 0644 {config}/* /etc/systemd/system/postfiat-{validator_id}.service /etc/systemd/system/postfiat-{validator_id}-rpc.service\n"
        f"{verify} >/dev/null\n"
        f"systemctl stop postfiat-{validator_id}-rpc.service postfiat-{validator_id}.service\n"
        f"! systemctl is-active --quiet postfiat-{validator_id}-rpc.service\n"
        f"! systemctl is-active --quiet postfiat-{validator_id}.service\n"
        "install -d -o root -g root -m 0700 /var/lib/postfiat/pre-rollout-snapshots\n"
        f"runuser -u postfiat -- {binary} storage-integrity-migrate-legacy --data-dir /var/lib/postfiat/{validator_id} --offline-confirmed > {migration_report}\n"
        f"test \"$(stat -c '%U:%G:%a' /var/lib/postfiat/{validator_id}/.integrity.key)\" = postfiat:postfiat:600\n"
        f"runuser -u postfiat -- {binary} verify-finalized-checkpoint --data-dir /var/lib/postfiat/{validator_id} > {checkpoint_report}\n"
        "systemctl daemon-reload\n"
        f"systemctl enable postfiat-{validator_id}.service postfiat-{validator_id}-rpc.service >/dev/null\n"
        f"systemctl start postfiat-{validator_id}.service\n"
        f"systemctl start postfiat-{validator_id}-rpc.service\n"
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
    endpoints = rpc_tunnel_endpoints(
        inventory, state.get("rpc_tunnel_base_port")
    )
    convergence = fleet_convergence(inventory, endpoints)
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
    preflight_parser.add_argument(
        "--rpc-tunnel-base-port",
        type=int,
        help=(
            "use existing localhost RPC tunnels base..base+5 for convergence "
            "while retaining canonical inventory hosts for cloud and SSH checks"
        ),
    )
    backup_parser = subparsers.add_parser("backup")
    backup_parser.add_argument("--state-file", type=Path, required=True)
    backup_parser.add_argument("--evidence-dir", type=Path, required=True)
    backup_parser.add_argument("--snapshot-publisher-key-file", type=Path, required=True)
    backup_parser.add_argument("--snapshot-publisher-public-key-file", type=Path, required=True)
    resume_backup_parser = subparsers.add_parser("resume-backup-verification")
    resume_backup_parser.add_argument("--state-file", type=Path, required=True)
    resume_backup_parser.add_argument("--evidence-dir", type=Path, required=True)
    resume_backup_parser.add_argument(
        "--snapshot-publisher-public-key-file", type=Path, required=True
    )
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
        elif args.command == "resume-backup-verification":
            report = resume_backup_verification(args, active_runner)
        else:
            report = apply_next(args, active_runner)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0
    except (SafetyError, OSError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"safe rollout refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
