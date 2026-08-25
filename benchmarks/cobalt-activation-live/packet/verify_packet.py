#!/usr/bin/env python3
"""Verify the compact controlled-testnet Cobalt activation packet."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

PACKET = Path(__file__).resolve().parent
REPO = PACKET.parents[2]
EXPECTED = {
    "height": 919,
    "tip": "3a8a117af9ed40728717005d03edf032719a3ca3d696365415a2d5b0d9aeef1c509d06d54029e6c34660e29aab43d0fb",
    "state": "ffa16323555800df7a4ff7cd336b9b151b0edfcf60954c207b704749133ff4b31ebd24444696d67e652f6e94510f7e60",
    "transition": "8846d8f06b3ebf81b5695e11bfc69a0d228fcfc17cf25e82143a1ba4097209e83aa93be2e67b590e299a59183ecd9e3f",
    "update": "518f543ea7a136a4fee5b6e8f969d47919f440f5040b9c8a0d0dfd97cd48a1f4ddc895bb3003a9042906bfab5610d749",
    "registry": "945768d593497541f59961d1ba3920560cfde7bf5037e40eb89dd5466637f221709bff05b69d2d40a36d5cff8505c37e",
    "trust": "9221316ae7f0f0e7e58d734700167f73f29aa1240377a8d61c637e7f36c5deb728203fcbb283c9f8f3398fc41d6b8b13",
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def object_file(name: str) -> dict:
    value = json.loads((PACKET / name).read_text(encoding="utf-8"))
    assert isinstance(value, dict), name
    return value


activation = object_file("activation-status.json")
fleet = object_file("fleet-status.json")
recovery = object_file("liveness-recovery.json")
negative = object_file("negative-cases.json")
state = object_file("state-verification.json")
ui = object_file("ui-snapshot.json")
prior = object_file("prior-evidence.json")
pins = object_file("source-pins.json")
verifier = object_file("verifier.json")

checks = {
    "activation_terminal": activation["ok"] is True
    and activation["status"] == "ACTIVATED"
    and activation["terminal_decision"] == "ACTIVATE",
    "authority_transition_live": activation["latest_transition"]["transition_id"]
    == EXPECTED["transition"]
    and activation["latest_transition"]["activation_height"] == 916,
    "registry_update_live": activation["latest_registry_update"]["update_id"]
    == EXPECTED["update"]
    and activation["latest_registry_update"]["activation_height"] == 917
    and activation["registry_root"] == EXPECTED["registry"]
    and activation["trust_graph_root"] == EXPECTED["trust"],
    "nonuniform_governance_verified": activation["verifier"]["verified"] is True
    and activation["verifier"]["cobalt_mode"] == "non_uniform"
    and activation["verifier"]["authority_mode"] == 1,
    "six_validator_convergence": fleet["all_equal"] is True
    and fleet["validator_count"] == 6
    and fleet["height"] == EXPECTED["height"]
    and fleet["tip_hash"] == EXPECTED["tip"]
    and fleet["state_root"] == EXPECTED["state"],
    "view_change_recovery": recovery["round_ok"] is True
    and recovery["abandoned_views"] == [0, 1]
    and recovery["committed_view"] == 2
    and recovery["vote_count"] == 6
    and recovery["all_six_converged"] is True,
    "negative_cases_rejected": negative["all_rejected"] is True
    and negative["durable_state_unchanged"] is True
    and set(negative["cases"])
    == {"early", "stale", "replayed", "wrong_root", "mixed_authority", "self_authorized"},
    "full_state_replay_verified": state["verified"] is True
    and state["block_log"]["verified"] is True
    and state["block_log"]["block_count"] == EXPECTED["height"]
    and state["block_log"]["tip_hash"] == EXPECTED["tip"]
    and state["block_log"]["state_root"] == EXPECTED["state"]
    and state["governance"]["latest_validator_registry_update_id"] == EXPECTED["update"],
    "consensus_v2_block_scope": activation["block_finality"] == "consensus-v2"
    and activation["authority"]["controls_block_consensus"] is False,
    "cli_activated": "Authority state: ACTIVATED" in (PACKET / "cli-output.txt").read_text(encoding="utf-8"),
    "ui_activated_read_only": ui["read_only"] is True
    and ui["rehearsal_readiness"]["status"] == "ACTIVATED"
    and ui["rehearsal_readiness"]["activation_performed"] is True
    and ui["actual_authority"]["cobalt_active"] is True
    and ui["actual_authority"]["block_finality"] == "consensus-v2"
    and ui["mutation_probe_http_status"] == 405,
}

prior_paths = {
    "frozen_oracle_and_decisive_corpus": "benchmarks/cobalt-activate-or-retire/section2-packet/SHA256SUMS.txt",
    "isolated_validator_simulation": "benchmarks/cobalt-activate-or-retire/section3-packet/SHA256SUMS.txt",
    "release_qualification": "benchmarks/cobalt-handoff-rehearsal/packet-release-qualified-v1/SHA256SUMS",
}
checks["prior_packets_bound"] = all(
    digest(REPO / path) == prior[key]["sha256sums_sha256"]
    for key, path in prior_paths.items()
)
checks["source_pins_match"] = all(
    digest(REPO / path) == expected for path, expected in pins["sources"].items()
)
scan_names = [
    "activation-status.json",
    "fleet-status.json",
    "liveness-recovery.json",
    "negative-cases.json",
    "state-verification.json",
    "source-pins.json",
    "ui-snapshot.json",
]
scan = b"\n".join((PACKET / name).read_bytes().lower() for name in scan_names)
checks["redaction_safe"] = all(
    marker not in scan
    for marker in (b"private_key_hex", b"secret_key", b"signature_hex", b"api_key")
)

manifest = (PACKET / "SHA256SUMS.txt").read_text(encoding="ascii").splitlines()
manifest_ok = bool(manifest)
for line in manifest:
    expected, separator, name = line.partition("  ")
    path = PACKET / name
    manifest_ok = manifest_ok and separator == "  " and path.is_file() and digest(path) == expected
checks["checksums_match"] = manifest_ok

assert checks == verifier["checks"], (checks, verifier["checks"])
assert verifier["schema"] == "postfiat-cobalt-activation-live-verifier-v1"
assert verifier["result"] == "passed"
assert all(checks.values())
print("packet-ok")
print(f"sha256sums_sha256={digest(PACKET / 'SHA256SUMS.txt')}")
