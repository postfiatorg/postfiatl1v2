#!/usr/bin/env python3
"""Run the disposable Cobalt authority-handoff rehearsal against pinned live identities."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shlex
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
TII = ROOT / ".tih"
KEY = Path("/home/postfiatchad/.ssh/postfiat-cobalt-shadow-20260823")
KNOWN_HOSTS = TII / "cobalt-live-known-hosts"
BINARY = ROOT / "target/release/postfiat-cobalt-handoff-rehearsal"
BINDING = TII / "cobalt-live-registry-binding-20260823.json"
SECTION4 = TII / "cobalt-section4-live-20260823-v1/result.json"
TARGETS = [
    ("wan-validator-0-recovery-v3-20260711", 0),
    ("wan-vultr-validator-1-repl", 1),
    ("wan-vultr-validator-2-repl", 2),
    ("wan-validator-0-pf", 3),
    ("wan-validator-1-pf", 4),
    ("wan-validator-2-pf", 5),
]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical_hash(value: Any) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def encode(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(encode(value), encoding="utf-8")


def load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ssh_base(host: str) -> list[str]:
    return [
        "ssh",
        "-i",
        str(KEY),
        "-o",
        "BatchMode=yes",
        "-o",
        "IdentitiesOnly=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        f"UserKnownHostsFile={KNOWN_HOSTS}",
        "-o",
        "ConnectTimeout=10",
        f"root@{host}",
    ]


def ssh_text(host: str, command: str, timeout: int = 60) -> str:
    for attempt in range(5):
        result = subprocess.run(
            ssh_base(host) + [command],
            capture_output=True,
            text=True,
            check=False,
            timeout=timeout,
        )
        if result.returncode == 0:
            return result.stdout
        if result.returncode != 255 or attempt == 4:
            raise RuntimeError(
                f"SSH command failed on {host} with status {result.returncode}: "
                f"{result.stderr.strip()}"
            )
        time.sleep(2**attempt)
    raise RuntimeError("unreachable SSH retry state")


def scp_to(host: str, local: Path, remote: str) -> None:
    subprocess.run(
        [
            "scp",
            "-i",
            str(KEY),
            "-o",
            "BatchMode=yes",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            f"UserKnownHostsFile={KNOWN_HOSTS}",
            "-o",
            "ConnectTimeout=10",
            str(local),
            f"root@{host}:{remote}",
        ],
        check=True,
        timeout=120,
    )


def remote_json(host: str, path: str) -> dict[str, Any]:
    return json.loads(ssh_text(host, f"cat {path}"))


def upload_text(host: str, remote_path: str, text: str) -> None:
    subprocess.run(
        ssh_base(host) + [f"umask 077; tee {remote_path} >/dev/null"],
        input=text,
        text=True,
        check=True,
        timeout=60,
    )


def remote_sign(host: str, remote_binary: str, args: list[str]) -> dict[str, Any]:
    command = shlex.join([remote_binary, *args])
    return json.loads(ssh_text(host, command, timeout=120))


def remote_shadow(host: str, args: list[str]) -> dict[str, Any]:
    executable = ssh_text(
        host,
        "readlink -f /proc/$(systemctl show --property=MainPID --value postfiat-cobalt-shadow.service)/exe",
    ).strip()
    if not executable.startswith("/"):
        raise RuntimeError("cannot resolve deployed Cobalt executable")
    return json.loads(ssh_text(host, shlex.join([executable, *args]), timeout=180))


def provider_hosts() -> dict[int, dict[str, str]]:
    discovery = load_module("cobalt_discovery", TII / "discover_cobalt_live_fleet.py")
    api_key = discovery.vault_secret("vultr")
    inventory = discovery.vultr_get(api_key, "/v2/instances?per_page=100")
    instances = {
        str(item.get("label")): item
        for item in inventory.get("instances", [])
        if isinstance(item, dict)
    }
    resolved: dict[int, dict[str, str]] = {}
    for label, index in TARGETS:
        item = instances.get(label)
        if not item:
            raise RuntimeError(f"provider instance is missing: {label}")
        resolved[index] = {
            "label": label,
            "host": str(item.get("main_ip") or ""),
            "provider_fingerprint": hashlib.sha384(
                str(item.get("id") or "").encode()
            ).hexdigest()[:24],
        }
        if not resolved[index]["host"]:
            raise RuntimeError(f"provider host is missing: {label}")
    return resolved


def live_receipt(hosts: dict[int, dict[str, str]]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for index in range(6):
        host = hosts[index]["host"]
        service = f"postfiat-validator-{index}.service"
        facts_text = ssh_text(
            host,
            f"""set -eu
vpid=$(systemctl show --property=MainPID --value {service})
spid=$(systemctl show --property=MainPID --value postfiat-cobalt-shadow.service)
printf 'validator_active=%s\\n' "$(systemctl is-active {service})"
printf 'validator_pid=%s\\n' "$vpid"
printf 'validator_restarts=%s\\n' "$(systemctl show --property=NRestarts --value {service})"
printf 'validator_binary_sha256=%s\\n' "$(sha256sum /proc/$vpid/exe | cut -d' ' -f1)"
printf 'sidecar_active=%s\\n' "$(systemctl is-active postfiat-cobalt-shadow.service)"
printf 'sidecar_pid=%s\\n' "$spid"
printf 'sidecar_restarts=%s\\n' "$(systemctl show --property=NRestarts --value postfiat-cobalt-shadow.service)"
printf 'sidecar_binary_sha256=%s\\n' "$(sha256sum /proc/$spid/exe | cut -d' ' -f1)"
""",
            timeout=60,
        )
        facts = {
            key: value
            for line in facts_text.splitlines()
            if "=" in line
            for key, _, value in [line.partition("=")]
        }
        chain = remote_json(host, f"/var/lib/postfiat/validator-{index}/chain_tip.json")
        shadow = remote_json(
            host, "/var/lib/postfiat-cobalt-shadow/state.json"
        )
        rows.append(
            {
                "validator_id": f"validator-{index}",
                **facts,
                "chain_tip": {
                    "height": chain.get("height"),
                    "block_hash": chain.get("block_hash"),
                    "state_root": chain.get("state_root"),
                },
                "cobalt_shadow": {
                    "registry_root": shadow.get("registry_root"),
                    "trust_graph_root": shadow.get("trust_graph_root"),
                    "live_authority": shadow.get("live_authority"),
                    "controls_block_consensus": shadow.get("controls_block_consensus"),
                    "state_hash": shadow.get("state_hash"),
                },
            }
        )
    return rows


def assert_untouched(before: list[dict[str, Any]], after: list[dict[str, Any]]) -> None:
    if len(before) != 6 or len(after) != 6:
        raise RuntimeError("live receipt cardinality mismatch")
    for left, right in zip(before, after):
        if left["validator_id"] != right["validator_id"]:
            raise RuntimeError("live receipt order changed")
        for field in (
            "validator_active",
            "validator_pid",
            "validator_restarts",
            "validator_binary_sha256",
        ):
            if left[field] != right[field]:
                raise RuntimeError(
                    f"{left['validator_id']} changed live validator field {field}"
                )
        if left["cobalt_shadow"]["registry_root"] != right["cobalt_shadow"]["registry_root"]:
            raise RuntimeError(f"{left['validator_id']} live registry root changed")
        if left["cobalt_shadow"]["trust_graph_root"] != right["cobalt_shadow"]["trust_graph_root"]:
            raise RuntimeError(f"{left['validator_id']} live trust graph changed")
        if right["cobalt_shadow"]["live_authority"] is not False:
            raise RuntimeError(f"{left['validator_id']} live Cobalt authority enabled")
        if right["cobalt_shadow"]["controls_block_consensus"] is not False:
            raise RuntimeError(f"{left['validator_id']} Cobalt controls block consensus")


def must(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise RuntimeError(f"refusing to overwrite existing packet: {output}")
    if not BINARY.is_file():
        raise RuntimeError("build target/release/postfiat-cobalt-handoff-rehearsal first")
    output.mkdir(parents=True)

    hosts = provider_hosts()
    before = live_receipt(hosts)
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, check=True, text=True, capture_output=True
    ).stdout.strip()
    binary_hash = sha256(BINARY)
    remote_binary = f"/tmp/postfiat-cobalt-handoff-rehearsal-{binary_hash[:12]}"
    for index in range(6):
        scp_to(hosts[index]["host"], BINARY, remote_binary)
        ssh_text(hosts[index]["host"], f"chmod 0700 {remote_binary}")

    source_host = hosts[0]["host"]
    genesis = remote_json(source_host, "/var/lib/postfiat/validator-0/genesis.json")
    chain_tip = remote_json(source_host, "/var/lib/postfiat/validator-0/chain_tip.json")
    registry = remote_json(
        source_host, "/var/lib/postfiat/validator-0/validator_registry_genesis.json"
    )
    binding = json.loads(BINDING.read_text(encoding="utf-8"))
    section4 = json.loads(SECTION4.read_text(encoding="utf-8"))
    activation_height = int(chain_tip["height"]) + 100
    manifest = {
        "schema": "postfiat-cobalt-handoff-clone-manifest-v1",
        "source_commit": commit,
        "genesis": genesis,
        "registry": registry,
        "registry_root": binding["registry_root"],
        "trust_graph_root": binding["trust_graph"]["trust_graph_root"],
        "cobalt_lock_hash": section4["governance_digest"],
        "anchor_height": chain_tip["height"],
        "anchor_genesis_hash": chain_tip["genesis_hash"],
        "anchor_block_hash": chain_tip["block_hash"],
        "anchor_state_root": chain_tip["state_root"],
        "activation_height": activation_height,
    }
    manifest_path = output / "clone-manifest.json"
    write_json(manifest_path, manifest)

    unsigned_path = output / "activation-transition-unsigned.json"
    subprocess.run(
        [str(BINARY), "prepare", "--manifest", str(manifest_path), "--output", str(unsigned_path)],
        check=True,
        timeout=60,
    )

    transition_remote = f"/tmp/{unsigned_path.name}"
    approvals: list[dict[str, Any]] = []
    for index in range(5):
        host = hosts[index]["host"]
        upload_text(host, transition_remote, unsigned_path.read_text(encoding="utf-8"))
        approval = remote_sign(
            host,
            remote_binary,
            [
                "sign-transition",
                "--transition",
                transition_remote,
                "--key-file",
                f"/var/lib/postfiat/validator-{index}/validator_keys.json",
                "--validator",
                f"validator-{index}",
            ],
        )
        must(approval.get("validator") == f"validator-{index}", "remote signer identity mismatch")
        approvals.append(approval)
    approvals_path = output / "activation-approvals.json"
    write_json(approvals_path, approvals)

    abort_path = output / "pre-activation-abort.json"
    subprocess.run(
        [
            str(BINARY),
            "abort",
            "--manifest",
            str(manifest_path),
            "--transition",
            str(unsigned_path),
            "--approvals",
            str(approvals_path),
            "--output",
            str(abort_path),
        ],
        check=True,
        timeout=60,
    )
    activation_path = output / "activation-result.json"
    subprocess.run(
        [
            str(BINARY),
            "finalize-activation",
            "--manifest",
            str(manifest_path),
            "--transition",
            str(unsigned_path),
            "--approvals",
            str(approvals_path),
            "--output",
            str(activation_path),
        ],
        check=True,
        timeout=60,
    )
    activation = json.loads(activation_path.read_text(encoding="utf-8"))
    signed_transition = activation["governance"]["cobalt_authority_transitions"][0]
    signed_transition_path = output / "activation-transition-signed.json"
    write_json(signed_transition_path, {"transition": signed_transition})

    update_path = output / "validator-trust-update-unsigned.json"
    updated_registry_path = output / "updated-registry.json"
    subprocess.run(
        [
            str(BINARY),
            "prepare-update",
            "--manifest",
            str(manifest_path),
            "--activation-result",
            str(activation_path),
            "--output",
            str(update_path),
            "--registry-output",
            str(updated_registry_path),
        ],
        check=True,
        timeout=60,
    )

    payload_info = json.loads(
        subprocess.run(
            [str(BINARY), "update-payload-hash", "--update", str(update_path)],
            check=True,
            capture_output=True,
            text=True,
            timeout=60,
        ).stdout
    )
    protocol_round = int(payload_info["protocol_round"])
    payload_hash = str(payload_info["payload_hash"])
    binding_remote = f"/tmp/cobalt-handoff-binding-{binary_hash[:12]}.json"
    proposal_remote = f"/tmp/cobalt-handoff-proposal-{protocol_round}.json"
    contributions_remote = f"/tmp/cobalt-handoff-contributions-{protocol_round}.json"
    shadow_clone_dirs: dict[int, str] = {}
    for index in range(6):
        host = hosts[index]["host"]
        clone_dir = ssh_text(
            host,
            shlex.join(
                [
                    "mktemp",
                    "-d",
                    f"/tmp/postfiat-cobalt-authority-{protocol_round}-XXXXXX",
                ]
            ),
        ).strip()
        if not clone_dir.startswith("/tmp/postfiat-cobalt-authority-"):
            raise RuntimeError("unexpected remote Cobalt clone path")
        ssh_text(
            host,
            shlex.join(
                ["cp", "-a", "/var/lib/postfiat-cobalt-shadow/.", f"{clone_dir}/"]
            ),
        )
        shadow_clone_dirs[index] = clone_dir
        upload_text(host, binding_remote, BINDING.read_text(encoding="utf-8"))

    proposal = remote_shadow(
        hosts[0]["host"],
        [
            "propose",
            "--data-dir",
            shadow_clone_dirs[0],
            "--registry-binding",
            binding_remote,
            "--round",
            str(protocol_round),
            "--payload-hash",
            payload_hash,
        ],
    )
    proposal_path = output / "cobalt-update-proposal.json"
    write_json(proposal_path, proposal)
    contributions: list[dict[str, Any]] = []
    for index in range(6):
        host = hosts[index]["host"]
        upload_text(host, proposal_remote, proposal_path.read_text(encoding="utf-8"))
        contribution = remote_shadow(
            host,
            [
                "contribute",
                "--data-dir",
                shadow_clone_dirs[index],
                "--registry-binding",
                binding_remote,
                "--proposal",
                proposal_remote,
            ],
        )
        must(contribution.get("node_id") == f"validator-{index}", "Cobalt contributor mismatch")
        contributions.append(contribution)
    contributions_path = output / "cobalt-update-contributions.json"
    write_json(contributions_path, contributions)
    upload_text(
        hosts[0]["host"],
        contributions_remote,
        contributions_path.read_text(encoding="utf-8"),
    )
    transcript = remote_shadow(
        hosts[0]["host"],
        [
            "assemble",
            "--registry-binding",
            binding_remote,
            "--proposal",
            proposal_remote,
            "--contributions",
            contributions_remote,
        ],
    )
    transcript_path = output / "cobalt-update-protocol-transcript.json"
    write_json(transcript_path, transcript)
    decision_certificate_path = output / "cobalt-update-decision-certificate.json"
    write_json(
        decision_certificate_path,
        {
            "schema": "postfiat.cobalt_validator_update_decision_certificate.v1",
            "registry_binding": binding,
            "protocol_transcript": transcript,
        },
    )
    certified_update_path = output / "validator-trust-update-certified.json"
    subprocess.run(
        [
            str(BINARY),
            "attach-decision",
            "--manifest",
            str(manifest_path),
            "--activation-result",
            str(activation_path),
            "--update",
            str(update_path),
            "--certificate",
            str(decision_certificate_path),
            "--output",
            str(certified_update_path),
        ],
        check=True,
        timeout=120,
    )

    negative_path = output / "negative-cases.json"
    subprocess.run(
        [
            str(BINARY),
            "negative",
            "--manifest",
            str(manifest_path),
            "--transition",
            str(signed_transition_path),
            "--update",
            str(certified_update_path),
            "--output",
            str(negative_path),
        ],
        check=True,
        timeout=60,
    )
    update = json.loads(certified_update_path.read_text(encoding="utf-8"))
    update_remote = f"/tmp/{certified_update_path.name}"
    authorizations: list[dict[str, Any]] = []
    for index in range(5):
        host = hosts[index]["host"]
        upload_text(host, update_remote, certified_update_path.read_text(encoding="utf-8"))
        authorization = remote_sign(
            host,
            remote_binary,
            [
                "sign-update",
                "--update",
                update_remote,
                "--key-file",
                f"/var/lib/postfiat/validator-{index}/validator_keys.json",
                "--validator",
                f"validator-{index}",
                "--authority-transition-id",
                signed_transition["transition_id"],
                "--parent-lock-hash",
                manifest["cobalt_lock_hash"],
                "--amendment-sequence",
                "2",
                "--proposal-slot",
                str(update["activation_height"]),
                "--expires-at-height",
                str(update["activation_height"] + 10),
            ],
        )
        must(authorization.get("validator") == f"validator-{index}", "update signer mismatch")
        authorizations.append(authorization)
    authorizations_path = output / "validator-update-authorizations.json"
    write_json(authorizations_path, authorizations)
    update_result_path = output / "validator-update-result.json"
    subprocess.run(
        [
            str(BINARY),
            "finalize-update",
            "--manifest",
            str(manifest_path),
            "--activation-result",
            str(activation_path),
            "--update",
            str(certified_update_path),
            "--authorizations",
            str(authorizations_path),
            "--output",
            str(update_result_path),
        ],
        check=True,
        timeout=60,
    )

    rollback_unsigned_path = output / "rollback-transition-unsigned.json"
    subprocess.run(
        [
            str(BINARY),
            "prepare-rollback",
            "--manifest",
            str(manifest_path),
            "--update-result",
            str(update_result_path),
            "--updated-registry",
            str(updated_registry_path),
            "--output",
            str(rollback_unsigned_path),
        ],
        check=True,
        timeout=60,
    )
    rollback_remote = f"/tmp/{rollback_unsigned_path.name}"
    rollback_approvals: list[dict[str, Any]] = []
    for index in range(5):
        host = hosts[index]["host"]
        upload_text(host, rollback_remote, rollback_unsigned_path.read_text(encoding="utf-8"))
        approval = remote_sign(
            host,
            remote_binary,
            [
                "sign-transition",
                "--transition",
                rollback_remote,
                "--key-file",
                f"/var/lib/postfiat/validator-{index}/validator_keys.json",
                "--validator",
                f"validator-{index}",
            ],
        )
        must(approval.get("validator") == f"validator-{index}", "rollback signer mismatch")
        rollback_approvals.append(approval)
    rollback_approvals_path = output / "rollback-approvals.json"
    write_json(rollback_approvals_path, rollback_approvals)
    rollback_result_path = output / "forward-rollback-result.json"
    subprocess.run(
        [
            str(BINARY),
            "finalize-rollback",
            "--manifest",
            str(manifest_path),
            "--update-result",
            str(update_result_path),
            "--updated-registry",
            str(updated_registry_path),
            "--transition",
            str(rollback_unsigned_path),
            "--approvals",
            str(rollback_approvals_path),
            "--output",
            str(rollback_result_path),
        ],
        check=True,
        timeout=60,
    )

    after = live_receipt(hosts)
    assert_untouched(before, after)

    abort = json.loads(abort_path.read_text(encoding="utf-8"))
    negative = json.loads(negative_path.read_text(encoding="utf-8"))
    update_result = json.loads(update_result_path.read_text(encoding="utf-8"))
    rollback = json.loads(rollback_result_path.read_text(encoding="utf-8"))
    must(abort["verified_before_abort"] is True and abort["applied"] is False, "abort receipt invalid")
    must(abort["governance_commitment_before"] == abort["governance_commitment_after"], "abort mutated state")
    must(negative["all_rejected"] is True and negative["durable_state_unchanged"] is True, "negative receipt invalid")
    must(len(negative["cases"]) == 6, "negative case count mismatch")
    must(activation["accepted"] is True, "activation was not accepted")
    must(activation["governance_commitment_before"] != activation["governance_commitment_after"], "activation did not mutate clone")
    must(update_result["accepted"] is True, "validator update was not accepted")
    must(bool(update_result["unrelated_governance_rejected"]), "unrelated governance was not rejected")
    must(rollback["accepted"] is True, "rollback was not accepted")
    must(rollback["authority_mode_after"] == rollback_result_authority_foundation(), "Foundation authority was not restored")
    final_governance = rollback["governance"]
    must(len(final_governance["cobalt_authority_transitions"]) == 2, "forward authority history is incomplete")
    must(len(final_governance["validator_registry_updates"]) == 1, "validator trust history is incomplete")

    operator_sequence = f"""# Disposable Cobalt authority rehearsal

1. Resolve the six current provider identities, capture live service/registry/authority facts, and read the public genesis, tip, and validator registry from validator-0.
2. Build clone manifest `{manifest_path.name}` at source commit `{commit[:12]}`, anchored to height {manifest['anchor_height']} and future activation height {activation_height}.
3. Request five current-registry ML-DSA-65 transition approvals on the validators; keys never leave the validators.
4. Verify the valid transition, then discard it and record `{abort_path.name}` with no clone mutation.
5. Verify and apply the transition to the disposable clone, then run all six negative cases against the signed transition.
6. Build a validator-5 key-rotation update, clone each live Cobalt signer state into a disposable directory, and run a six-validator signed RBC -> ABBA -> MVBA -> DABC decision over the exact update payload.
7. Attach and verify that protocol decision before requesting five scoped validator authorizations; apply the certified update and reject an unrelated crypto-policy amendment.
8. Build a rollback that binds the update lock and new trust root, request five approvals under the updated registry, verify it, and apply it as a second forward transition.
9. Capture live facts again and require validator process/binary/registry/trust/authority fields to be unchanged.

The live fleet is never restarted or written. Consensus v2 remains the only block-finality protocol; this packet does not authorize activation.
"""
    (output / "operator-sequence.md").write_text(operator_sequence, encoding="utf-8")

    packet_files = [
        manifest_path,
        unsigned_path,
        approvals_path,
        abort_path,
        activation_path,
        signed_transition_path,
        negative_path,
        update_path,
        proposal_path,
        contributions_path,
        transcript_path,
        decision_certificate_path,
        certified_update_path,
        updated_registry_path,
        authorizations_path,
        update_result_path,
        rollback_unsigned_path,
        rollback_approvals_path,
        rollback_result_path,
    ]
    live_before = output / "live-fleet-before.json"
    live_after = output / "live-fleet-after.json"
    write_json(live_before, before)
    write_json(live_after, after)
    packet_files.extend([live_before, live_after, output / "operator-sequence.md"])
    checksum_rows = []
    for path in packet_files:
        checksum_rows.append(f"{sha256(path)}  {path.name}")
    checksums = output / "SHA256SUMS"
    checksums.write_text("\n".join(sorted(checksum_rows)) + "\n", encoding="utf-8")

    verifier = {
        "schema": "postfiat-cobalt-handoff-rehearsal-verifier-v1",
        "result": "passed",
        "source_commit": commit,
        "binary_sha256": binary_hash,
        "clone_manifest_sha256": sha256(manifest_path),
        "checks": {
            "current_registry_and_validator_count": len(registry["validators"]) == 6,
            "activation_future_height": activation_height > int(chain_tip["height"]),
            "pre_activation_abort_without_mutation": True,
            "activation_clone_state_changed": True,
            "all_six_negative_cases_rejected": True,
            "signed_rbc_abba_mvba_dabc_decision_verified": True,
            "decision_payload_bound_to_exact_validator_update": True,
            "scoped_validator_update_accepted": True,
            "unrelated_governance_rejected": True,
            "forward_rollback_restored_foundation": True,
            "forward_history_two_transitions_one_update": True,
            "live_validator_processes_unchanged": True,
            "live_registry_and_trust_roots_unchanged": True,
            "live_authority_and_block_control_disabled": True,
            "validator_private_keys_never_left_validators": True,
        },
        "provider_identities": {
            f"validator-{index}": hosts[index]["provider_fingerprint"] for index in range(6)
        },
        "packet_files": {path.name: sha256(path) for path in packet_files},
        "sha256sums_sha256": sha256(checksums),
        "captured_at": datetime.now(timezone.utc).isoformat(),
    }
    write_json(output / "verifier.json", verifier)

    for index in range(6):
        host = hosts[index]["host"]
        subprocess.run(
            ssh_base(host)
            + [
                shlex.join(
                    [
                        "rm",
                        "-f",
                        "--",
                        remote_binary,
                        transition_remote,
                        update_remote,
                        rollback_remote,
                        binding_remote,
                        proposal_remote,
                        contributions_remote,
                    ]
                )
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=30,
            check=False,
        )
        subprocess.run(
            ssh_base(host)
            + [shlex.join(["rm", "-rf", "--", shadow_clone_dirs[index]])],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=30,
            check=False,
        )

    print(
        f"COBALT_HANDOFF_REHEARSAL_OK activation={activation_height} "
        f"packet={output} sha256sums={verifier['sha256sums_sha256']}"
    )
    return 0


def rollback_result_authority_foundation() -> int:
    # Matches postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION.
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
