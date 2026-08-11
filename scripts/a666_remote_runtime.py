#!/usr/bin/env python3
"""Fail-closed runtime identity checks for remote consensus rounds."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import json
import re
import shlex
import subprocess
from typing import Any


EXPECTED_VALIDATORS = 6
RELEASE_ID_PATTERN = r"[a-z0-9][a-z0-9.-]{0,127}"


def validated_release_id(remote_binary: str, remote_topology: str) -> str:
    binary_match = re.fullmatch(
        rf"/opt/postfiat/releases/({RELEASE_ID_PATTERN})/postfiat-node",
        remote_binary,
    )
    topology_match = re.fullmatch(
        rf"/etc/postfiat/releases/({RELEASE_ID_PATTERN})/topology\.json",
        remote_topology,
    )
    if binary_match is None or topology_match is None:
        raise RuntimeError(
            "remote binary and topology must be absolute signed-release paths"
        )
    if binary_match.group(1) != topology_match.group(1):
        raise RuntimeError("remote binary and topology name different release IDs")
    return binary_match.group(1)


def load_proposer_hosts(path: Any) -> dict[str, str]:
    value = json.loads(path.read_text())
    expected = {f"validator-{index}" for index in range(EXPECTED_VALIDATORS)}
    if not isinstance(value, dict) or set(value) != expected:
        raise RuntimeError(
            "proposer hosts file must map exactly validator-0 through validator-5"
        )
    if any(
        not isinstance(host, str)
        or not host
        or any(character.isspace() for character in host)
        or "@" in host
        for host in value.values()
    ):
        raise RuntimeError("proposer hosts file contains an invalid SSH host")
    return value


def _probe_command(node_id: str, remote_binary: str, remote_topology: str) -> str:
    index = node_id.rsplit("-", 1)[1]
    services = (
        f"postfiat-validator-{index}.service",
        f"postfiat-validator-{index}-rpc.service",
    )
    quoted_services = " ".join(shlex.quote(service) for service in services)
    return "\n".join(
        [
            "set -euo pipefail",
            f"expected_binary=$(readlink -f -- {shlex.quote(remote_binary)})",
            'expected_binary_sha=$(sha256sum -- "$expected_binary" | awk \'{print $1}\')',
            f"expected_topology={shlex.quote(remote_topology)}",
            'expected_topology_sha=$(sha256sum -- "$expected_topology" | awk \'{print $1}\')',
            f"for service in {quoted_services}; do",
            '  pid=$(systemctl show -p MainPID --value "$service")',
            '  test "$pid" -gt 0',
            '  active_binary=$(readlink -f -- "/proc/$pid/exe")',
            '  active_binary_sha=$(sha256sum -- "$active_binary" | awk \'{print $1}\')',
            '  topology_present=false',
            '  if tr \'\\0\' \'\\n\' <"/proc/$pid/cmdline" | grep -Fxq -- "$expected_topology"; then',
            '    topology_present=true',
            "  fi",
            '  printf \'%s\\t%s\\t%s\\t%s\\t%s\\t%s\\t%s\\t%s\\t%s\\n\' '
            f"{shlex.quote(node_id)} "
            '"$service" "$pid" "$expected_binary" "$expected_binary_sha" '
            '"$active_binary" "$active_binary_sha" "$expected_topology_sha" '
            '"$topology_present"',
            "done",
        ]
    )


def parse_runtime_probe(output: str) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line in output.splitlines():
        fields = line.split("\t")
        if len(fields) != 9:
            raise RuntimeError("remote runtime probe returned malformed output")
        (
            node_id,
            service,
            pid,
            expected_binary,
            expected_binary_sha256,
            active_binary,
            active_binary_sha256,
            topology_sha256,
            topology_present,
        ) = fields
        if not pid.isdigit() or int(pid) <= 0:
            raise RuntimeError(f"remote runtime probe returned invalid PID for {service}")
        rows.append(
            {
                "node_id": node_id,
                "service": service,
                "pid": int(pid),
                "expected_binary": expected_binary,
                "expected_binary_sha256": expected_binary_sha256,
                "active_binary": active_binary,
                "active_binary_sha256": active_binary_sha256,
                "topology_sha256": topology_sha256,
                "topology_argument_present": topology_present == "true",
            }
        )
    return rows


def validate_runtime_rows(rows: list[dict[str, Any]]) -> None:
    expected_services = {
        service
        for index in range(EXPECTED_VALIDATORS)
        for service in (
            f"postfiat-validator-{index}.service",
            f"postfiat-validator-{index}-rpc.service",
        )
    }
    observed_services = {row.get("service") for row in rows}
    if len(rows) != EXPECTED_VALIDATORS * 2 or observed_services != expected_services:
        raise RuntimeError("runtime preflight did not identify all 12 validator services")
    for row in rows:
        if row.get("active_binary") != row.get("expected_binary"):
            raise RuntimeError(
                f"{row.get('service')} runs {row.get('active_binary')}, not "
                f"{row.get('expected_binary')}"
            )
        if row.get("active_binary_sha256") != row.get("expected_binary_sha256"):
            raise RuntimeError(f"{row.get('service')} binary SHA-256 mismatch")
        if row.get("topology_argument_present") is not True:
            raise RuntimeError(
                f"{row.get('service')} does not use the requested topology path"
            )
    binary_hashes = {row["active_binary_sha256"] for row in rows}
    topology_hashes = {row["topology_sha256"] for row in rows}
    if len(binary_hashes) != 1:
        raise RuntimeError("validator fleet does not run one identical binary")
    if len(topology_hashes) != 1:
        raise RuntimeError("validator fleet does not use one identical topology")


def probe_remote_runtime(
    proposer_hosts: dict[str, str],
    remote_binary: str,
    remote_topology: str,
    *,
    timeout_seconds: float,
) -> dict[str, Any]:
    release_id = validated_release_id(remote_binary, remote_topology)

    def probe(item: tuple[str, str]) -> list[dict[str, Any]]:
        node_id, host = item
        command = _probe_command(node_id, remote_binary, remote_topology)
        try:
            result = subprocess.run(
                [
                    "ssh",
                    "-o",
                    "BatchMode=yes",
                    f"root@{host}",
                    "/bin/bash -c " + shlex.quote(command),
                ],
                check=True,
                text=True,
                capture_output=True,
                timeout=timeout_seconds,
            )
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            raise RuntimeError(f"runtime identity probe failed for {node_id}") from error
        rows = parse_runtime_probe(result.stdout)
        if any(row["node_id"] != node_id for row in rows):
            raise RuntimeError(f"runtime identity probe mislabeled {node_id}")
        return rows

    ordered_hosts = sorted(proposer_hosts.items())
    with ThreadPoolExecutor(max_workers=EXPECTED_VALIDATORS) as executor:
        nested_rows = list(executor.map(probe, ordered_hosts))
    rows = [row for node_rows in nested_rows for row in node_rows]
    validate_runtime_rows(rows)
    return {
        "schema": "postfiat-a666-remote-runtime-identity-v1",
        "release_id": release_id,
        "remote_binary": remote_binary,
        "remote_topology": remote_topology,
        "binary_sha256": rows[0]["active_binary_sha256"],
        "topology_sha256": rows[0]["topology_sha256"],
        "services": rows,
    }
