#!/usr/bin/env python3
"""Fail-closed H314 residual gate for the proof-backed Ethereum route.

The gate performs no fleet or source-RPC operation until a signed Phase-2
proof validates.  After that precondition it invokes the existing
conservation checker, which owns the signed finalized-checkpoint export,
local import/verification, and source-vault audit sequence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any


SCHEMA = "postfiat.pfusdc.h314_residual_gate.v1"
DEFAULT_CONFIG = Path(
    "docs/evidence/pfusdc-eth-campaign-20260725/lane-b/conservation-h314/gate-config.json"
)
DEFAULT_PROOF = Path(
    "docs/evidence/pfusdc-eth-campaign-20260725/lane-c/rev6-execution/"
    "04-h314-register/phase-2-proof.json"
)


class GateError(RuntimeError):
    pass


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    os.chmod(temporary, 0o644)
    temporary.replace(path)


def require_file(path: Path, label: str) -> None:
    if not path.is_file():
        raise GateError(f"{label} is missing: {path}")


def load_json(path: Path, label: str) -> dict[str, Any]:
    require_file(path, label)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise GateError(f"{label} is invalid JSON: {path}") from error
    if not isinstance(value, dict):
        raise GateError(f"{label} must be a JSON object: {path}")
    return value


def string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise GateError(f"{label} must be a nonempty string")
    return value


def integer(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise GateError(f"{label} must be an integer")
    return value


def reference(value: Any, label: str) -> dict[str, str]:
    if not isinstance(value, dict):
        raise GateError(f"{label} must be an object")
    path = Path(string(value.get("path"), f"{label}.path"))
    expected = string(value.get("sha256"), f"{label}.sha256")
    if len(expected) != 64 or any(character not in "0123456789abcdef" for character in expected):
        raise GateError(f"{label}.sha256 must be lowercase SHA-256")
    require_file(path, label)
    observed = sha256_file(path)
    if observed != expected:
        raise GateError(f"{label} digest mismatch")
    return {"path": str(path), "sha256": observed}


def load_config(path: Path) -> dict[str, Any]:
    config = load_json(path, "H314 gate config")
    if config.get("schema") != SCHEMA:
        raise GateError("H314 gate config has an unsupported schema")
    if integer(config.get("target_height"), "target_height") != 314:
        raise GateError("H314 gate target height must be exactly 314")
    pins = config.get("pins")
    if not isinstance(pins, dict):
        raise GateError("H314 gate config pins must be an object")
    required_pins = {
        "checker_source",
        "checker_tests",
        "h310_identity",
        "vault_interface_lineage",
        "rev6_prebroadcast_manifest",
        "rev6_lineage",
        "rev6_deployment_readback",
        "legacy_finding",
    }
    if set(pins) != required_pins:
        raise GateError("H314 gate config has incomplete or unexpected pins")
    for label in sorted(required_pins):
        reference(pins[label], f"pins.{label}")

    baseline = config.get("h310_baseline")
    if not isinstance(baseline, dict):
        raise GateError("H314 gate config h310_baseline must be an object")
    expected_components = {"V": "6000020", "S": "1000000", "D": "0", "B": "10", "R": "0"}
    if baseline.get("components") != expected_components:
        raise GateError("H314 gate baseline components are not the accepted H310 table")
    if baseline.get("rhs") != "1000010" or baseline.get("residual_atoms") != "5000010":
        raise GateError("H314 gate baseline residual is not the accepted +5000010 atoms")
    opening = load_json(Path(pins["h310_identity"]["path"]), "H310 opening identity")
    if opening.get("components") != expected_components or opening.get("rhs") != "1000010" or opening.get("residual_atoms") != "5000010":
        raise GateError("pinned H310 identity does not match the accepted baseline table")
    if opening.get("height") != 310:
        raise GateError("pinned H310 identity has an unexpected finalized height")
    return config


def rev6_route(config: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    pins = config["pins"]
    manifest_ref = reference(pins["rev6_prebroadcast_manifest"], "pins.rev6_prebroadcast_manifest")
    lineage_ref = reference(pins["rev6_lineage"], "pins.rev6_lineage")
    readback_ref = reference(pins["rev6_deployment_readback"], "pins.rev6_deployment_readback")
    manifest = load_json(Path(manifest_ref["path"]), "rev6 immutable pre-broadcast manifest")
    lineage = load_json(Path(lineage_ref["path"]), "rev6 manifest lineage")
    current = lineage.get("current_revision")
    if not isinstance(current, dict):
        raise GateError("rev6 manifest lineage has no current revision")
    lineage_manifest = current.get("manifest")
    if not isinstance(lineage_manifest, dict):
        raise GateError("rev6 manifest lineage current revision has no manifest reference")
    if lineage_manifest.get("sha256") != manifest_ref["sha256"]:
        raise GateError("rev6 manifest lineage does not bind the pinned immutable rev6 manifest digest")
    route = manifest.get("route")
    if not isinstance(route, dict):
        raise GateError("rev6 manifest route is missing")
    for field in ("route_id", "route_epoch", "vault_address", "vault_runtime_code_hash"):
        if field not in route:
            raise GateError(f"rev6 manifest route omitted {field}")
    if current.get("route_epoch") != route["route_epoch"]:
        raise GateError("rev6 manifest lineage route epoch disagrees with rev6 manifest")
    readback = load_json(Path(readback_ref["path"]), "rev6 deployment readback")
    digests = readback.get("manifest_digests")
    if not isinstance(digests, dict) or digests.get("pre_broadcast_input_sha256") != manifest_ref["sha256"]:
        raise GateError("rev6 deployment readback does not bind the immutable pre-broadcast manifest")
    readback_values = readback.get("readback")
    if not isinstance(readback_values, dict):
        raise GateError("rev6 deployment readback omitted runtime values")
    observed_runtime = readback_values.get("vault_runtime_code_hash")
    if observed_runtime != route["vault_runtime_code_hash"]:
        raise GateError("rev6 deployment readback vault runtime hash disagrees with the immutable manifest")
    return manifest, lineage, route


def validate_phase2_proof(proof_path: Path, config: dict[str, Any], route: dict[str, Any]) -> dict[str, Any]:
    if not proof_path.is_file():
        raise GateError(f"H314_PROOF_REQUIRED: signed Phase-2 artifact is missing: {proof_path}")
    proof = load_json(proof_path, "signed Phase-2 artifact")
    if proof.get("schema") != config.get("phase2_proof_schema"):
        raise GateError("H314_PROOF_REQUIRED: signed Phase-2 artifact schema is unsupported")

    signed = proof.get("signed_artifact")
    if not isinstance(signed, dict):
        raise GateError("H314_PROOF_REQUIRED: signed Phase-2 artifact has no signed_artifact")
    string(signed.get("signer"), "signed_artifact.signer")
    reference(signed.get("certificate"), "signed_artifact.certificate")

    finalized = proof.get("finalized")
    if not isinstance(finalized, dict):
        raise GateError("H314_PROOF_REQUIRED: signed Phase-2 artifact has no finalized parent")
    if integer(finalized.get("height"), "finalized.height") != config["target_height"]:
        raise GateError("H314_PROOF_REQUIRED: signed Phase-2 artifact is not finalized at H314")
    tip = string(finalized.get("tip"), "finalized.tip")
    root = string(finalized.get("state_root"), "finalized.state_root")
    validators = finalized.get("validators")
    if not isinstance(validators, list) or len(validators) != 6:
        raise GateError("H314_PROOF_REQUIRED: signed Phase-2 artifact does not contain six validator parents")
    expected_ids = {f"validator-{index}" for index in range(6)}
    observed_ids: set[str] = set()
    for entry in validators:
        if not isinstance(entry, dict):
            raise GateError("H314_PROOF_REQUIRED: validator parent entry is malformed")
        validator_id = string(entry.get("validator_id"), "validator.validator_id")
        observed_ids.add(validator_id)
        if integer(entry.get("height"), f"{validator_id}.height") != config["target_height"]:
            raise GateError(f"H314_PROOF_REQUIRED: {validator_id} is not finalized at H314")
        if string(entry.get("tip"), f"{validator_id}.tip") != tip or string(entry.get("state_root"), f"{validator_id}.state_root") != root:
            raise GateError(f"H314_PROOF_REQUIRED: {validator_id} does not bind the common finalized parent")
        reference(entry.get("evidence"), f"{validator_id}.evidence")
    if observed_ids != expected_ids:
        raise GateError("H314_PROOF_REQUIRED: signed Phase-2 artifact validator roster is not validator-0 through validator-5")

    active_route = proof.get("active_route")
    if not isinstance(active_route, dict):
        raise GateError("H314_PROOF_REQUIRED: signed Phase-2 artifact has no active route")
    for field in ("route_id", "route_epoch", "vault_address", "vault_runtime_code_hash"):
        if active_route.get(field) != route[field]:
            raise GateError(f"H314_PROOF_REQUIRED: active route {field} does not match the rev6 manifest")
    abi_class = string(active_route.get("vault_interface_abi_class"), "active_route.vault_interface_abi_class")

    negative = proof.get("arbitrum_ingress_negative_gate")
    if not isinstance(negative, dict) or negative.get("status") != "PASS":
        raise GateError("H314_PROOF_REQUIRED: Arbitrum-ingress negative gate is not PASS")
    negative_ref = reference(negative.get("evidence"), "arbitrum_ingress_negative_gate.evidence")
    return {
        "proof_path": str(proof_path),
        "proof_sha256": sha256_file(proof_path),
        "height": config["target_height"],
        "tip": tip,
        "state_root": root,
        "route_id": route["route_id"],
        "route_epoch": route["route_epoch"],
        "vault_runtime_code_hash": route["vault_runtime_code_hash"],
        "vault_interface_abi_class": abi_class,
        "negative_gate": negative_ref,
    }


def validate_live_verified_lineage(
    config: dict[str, Any], route: dict[str, Any], proof: dict[str, Any]
) -> dict[str, str]:
    lineage_ref = reference(config["pins"]["vault_interface_lineage"], "pins.vault_interface_lineage")
    manifest_ref = reference(config["pins"]["rev6_prebroadcast_manifest"], "pins.rev6_prebroadcast_manifest")
    lineage = load_json(Path(lineage_ref["path"]), "vault interface lineage")
    entries = lineage.get("entries")
    if not isinstance(entries, list):
        raise GateError("vault interface lineage entries are missing")
    matching = [entry for entry in entries if isinstance(entry, dict) and entry.get("runtime_code_hash") == route["vault_runtime_code_hash"]]
    if len(matching) != 1:
        raise GateError("vault interface lineage must contain exactly one rev6 runtime hash mapping")
    entry = matching[0]
    if entry.get("source_manifest_path") != manifest_ref["path"] or entry.get("source_manifest_sha256") != manifest_ref["sha256"]:
        raise GateError("vault interface lineage rev6 runtime mapping is not sourced by the pinned rev6 manifest")
    if entry.get("deployment_revision_label") != "rev6-generated-nonce67-68":
        raise GateError("vault interface lineage runtime mapping is not labeled as rev6-generated-nonce67-68")
    if entry.get("verification_status") != "live_verified":
        raise GateError("vault interface lineage rev6 runtime mapping is not live_verified")
    if entry.get("abi_class") != proof["vault_interface_abi_class"]:
        raise GateError("Phase-2 proof ABI class does not match the data-driven runtime hash mapping")
    return lineage_ref


def checker_command(args: argparse.Namespace, config: dict[str, Any], lineage: Path) -> list[str]:
    def required_path(value: Path | None, label: str) -> Path:
        if value is None:
            raise GateError(f"{label} is required after H314 proof acceptance")
        require_file(value, label)
        return value

    required = {
        "inventory file": args.inventory_file,
        "snapshot publisher private key file": args.snapshot_publisher_private_key_file,
        "snapshot publisher public key file": args.snapshot_publisher_public_key_file,
        "local node binary": args.local_node_bin,
        "local cast binary": args.local_cast_bin,
    }
    resolved = {label: required_path(path, label) for label, path in required.items()}
    if not isinstance(args.expected_local_node_sha256, str) or len(args.expected_local_node_sha256) != 64:
        raise GateError("expected local node SHA-256 is required after H314 proof acceptance")
    if not isinstance(args.source_rpc_url, str) or not args.source_rpc_url.startswith("https://"):
        raise GateError("source RPC URL must be an explicit https URL")
    if args.ssh_identity_file is not None:
        require_file(args.ssh_identity_file, "SSH identity file")

    output = args.output_dir / "identity.json"
    command = [
        sys.executable,
        config["pins"]["checker_source"]["path"],
        "--inventory-file", str(resolved["inventory file"]),
        "--asset-id", config["asset_id"],
        "--output", str(output),
        "--expected-validator-count", "6",
        "--local-node-bin", str(resolved["local node binary"]),
        "--expected-local-node-sha256", args.expected_local_node_sha256,
        "--local-cast-bin", str(resolved["local cast binary"]),
        "--scratch-dir", str(args.output_dir / "scratch"),
        "--snapshot-publisher-private-key-file", str(resolved["snapshot publisher private key file"]),
        "--snapshot-publisher-public-key-file", str(resolved["snapshot publisher public key file"]),
        "--source-rpc-url", args.source_rpc_url,
        "--vault-interface-lineage-manifest", str(lineage),
        "--legacy-source-label", "deprecated-Arbitrum-legacy",
        "--legacy-finding-file", config["pins"]["legacy_finding"]["path"],
        "--opening-identity-file", config["pins"]["h310_identity"]["path"],
    ]
    if args.ssh_identity_file is not None:
        command.extend(["--ssh-identity-file", str(args.ssh_identity_file)])
    return command


def exact_h314_verdict(identity_path: Path, config: dict[str, Any], proof: dict[str, Any], lineage: dict[str, str]) -> dict[str, Any]:
    identity = load_json(identity_path, "H314 conservation identity")
    baseline = config["h310_baseline"]
    components = identity.get("components")
    if not isinstance(components, dict) or set(components) != {"V", "S", "D", "B", "R"}:
        raise GateError("H314 conservation identity omitted the full V/S/D/B/R table")
    for term in components:
        string(components[term], f"H314 component {term}")
        if not components[term].lstrip("-").isdecimal():
            raise GateError(f"H314 component {term} is not exact atoms")
    if identity.get("height") != proof["height"] or identity.get("tip") != proof["tip"] or identity.get("state_root") != proof["state_root"]:
        raise GateError("H314 conservation identity does not bind the signed Phase-2 common parent")
    raw = identity.get("source_rpc", {}).get("raw_response_paths")
    if not isinstance(raw, list) or {item.get("validator_id") for item in raw if isinstance(item, dict) and str(item.get("validator_id", "")).startswith("validator-")} != {f"validator-{index}" for index in range(6)}:
        raise GateError("H314 conservation identity omitted one or more six-host source transcripts")
    snapshot = identity.get("snapshot")
    if not isinstance(snapshot, dict) or not all(isinstance(snapshot.get(key), str) and snapshot[key] for key in ("signed_import_transcript", "verification_transcript")):
        raise GateError("H314 conservation identity omitted signed import or finalized-checkpoint verification evidence")
    bracket = identity.get("opening_bracket")
    if not isinstance(bracket, dict):
        raise GateError("H314 conservation identity omitted the H310 opening-bracket comparison")
    residual = string(identity.get("residual_atoms"), "H314 residual")
    delta = string(bracket.get("residual_delta_from_h310"), "H314 residual delta")
    verdict = {
        "schema": SCHEMA,
        "identity": {"path": str(identity_path), "sha256": sha256_file(identity_path)},
        "phase2_proof": {"path": proof["proof_path"], "sha256": proof["proof_sha256"]},
        "runtime_hash_mapping": lineage,
        "height": identity["height"],
        "tip": identity["tip"],
        "state_root": identity["state_root"],
        "components": {term: components[term] for term in ("V", "S", "D", "B", "R")},
        "lhs": identity.get("lhs"),
        "rhs": identity.get("rhs"),
        "residual_atoms": residual,
        "opening_residual_atoms": baseline["residual_atoms"],
        "residual_delta_from_h310": delta,
        "component_delta_from_h310": {
            term: str(int(components[term]) - int(baseline["components"][term]))
            for term in ("V", "S", "D", "B", "R")
        },
        "criterion": "residual_h314 == +5000010 and residual_h314 - residual_h310 == 0 exactly",
    }
    if residual == baseline["residual_atoms"] and delta == "0":
        verdict["status"] = "H315_RELEASED"
        return verdict
    verdict["status"] = "H315_BLOCKED"
    return verdict


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--phase2-proof", type=Path, default=DEFAULT_PROOF)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_CONFIG.parent)
    parser.add_argument("--inventory-file", type=Path)
    parser.add_argument("--snapshot-publisher-private-key-file", type=Path)
    parser.add_argument("--snapshot-publisher-public-key-file", type=Path)
    parser.add_argument("--local-node-bin", type=Path)
    parser.add_argument("--expected-local-node-sha256")
    parser.add_argument("--local-cast-bin", type=Path)
    parser.add_argument("--source-rpc-url")
    parser.add_argument("--ssh-identity-file", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        config = load_config(args.config)
        if not args.phase2_proof.is_file():
            raise GateError(f"H314_PROOF_REQUIRED: signed Phase-2 artifact is missing: {args.phase2_proof}")
        _, _, route = rev6_route(config)
        proof = validate_phase2_proof(args.phase2_proof, config, route)
        args.output_dir.mkdir(parents=True, exist_ok=True)
        live_lineage = validate_live_verified_lineage(config, route, proof)
        preflight = {
            "schema": SCHEMA,
            "status": "H314_PROOF_ACCEPTED",
            "phase2_proof": {"path": proof["proof_path"], "sha256": proof["proof_sha256"]},
            "height": proof["height"],
            "tip": proof["tip"],
            "state_root": proof["state_root"],
            "route_id": proof["route_id"],
            "route_epoch": proof["route_epoch"],
            "vault_runtime_code_hash": proof["vault_runtime_code_hash"],
            "vault_interface_abi_class": proof["vault_interface_abi_class"],
            "arbitrum_ingress_negative_gate": proof["negative_gate"],
            "runtime_hash_mapping": live_lineage,
            "read_only_sequence": "checker performs six-host read, finalized-checkpoint export, local signed import/verify, and local audit after this proof gate",
        }
        write_json(args.output_dir / "phase2-preflight.json", preflight)
        command = checker_command(args, config, Path(live_lineage["path"]))
        completed = subprocess.run(command, text=True, capture_output=True)
        command_record = {"command": command, "returncode": completed.returncode, "stdout": completed.stdout, "stderr": completed.stderr}
        write_json(args.output_dir / "checker-command.json", command_record)
        identity_path = args.output_dir / "identity.json"
        if completed.returncode != 0:
            print("H315_BLOCKED: conservation checker did not produce an accepted H314 audit", file=sys.stderr)
            return completed.returncode
        verdict = exact_h314_verdict(identity_path, config, proof, live_lineage)
        write_json(args.output_dir / "gate-verdict.json", verdict)
        print(f"{verdict['status']}: residual={verdict['residual_atoms']} delta={verdict['residual_delta_from_h310']}")
        return 0 if verdict["status"] == "H315_RELEASED" else 1
    except GateError as error:
        print(str(error), file=sys.stderr)
        return 2
    except OSError as error:
        print(f"H315_BLOCKED: local I/O failure: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
