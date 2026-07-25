"""Read-only six-validator dry check for a pinned PFTL public handoff."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import sys
from typing import Any, Callable, Mapping, Sequence

from .pftl_handoff import (
    DEFAULT_HANDOFF_PATH,
    HandoffError,
    PersistentPftlHandoff,
    load_persistent_handoff,
)


class HandoffCheckError(HandoffError):
    """The live read-only view does not match the pinned public handoff."""


def default_client_factory(endpoint: str) -> Any:
    repo_root = Path(__file__).resolve().parents[3]
    python_root = repo_root / "python"
    if str(python_root) not in sys.path:
        sys.path.insert(0, str(python_root))
    from postfiat_rpc.client import PostFiatRpcClient

    return PostFiatRpcClient(endpoint, timeout_seconds=8)


def _uint(value: Any, label: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum or value > (1 << 63) - 1:
        raise HandoffCheckError(f"{label} must be uint63 >= {minimum}")
    return value


def _text(value: Any, label: str, *, maximum: int = 4096) -> str:
    if (
        type(value) is not str
        or not value
        or len(value) > maximum
        or not value.isascii()
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
    ):
        raise HandoffCheckError(f"{label} must be bounded printable ASCII")
    return value


def _hex48(value: Any, label: str) -> str:
    text = _text(value, label, maximum=96)
    if len(text) != 96 or any(character not in "0123456789abcdef" for character in text):
        raise HandoffCheckError(f"{label} must be 48-byte lowercase hex")
    return text


def _mapping(value: Any, label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise HandoffCheckError(f"{label} must be an object")
    return value


def _single_profile(
    status: Mapping[str, Any], handoff: PersistentPftlHandoff
) -> Mapping[str, Any]:
    profiles = status.get("active_nav_profiles")
    if not isinstance(profiles, list):
        raise HandoffCheckError("status has no active NAV profile list")
    matches = [
        profile
        for profile in profiles
        if isinstance(profile, Mapping)
        and profile.get("asset_id") == handoff.asset_id
    ]
    if len(matches) != 1:
        raise HandoffCheckError("status does not contain one pinned NAV profile")
    profile = matches[0]
    expected = {
        "profile_id": handoff.profile_id,
        "finalized_epoch": handoff.nav_epoch,
        "finalized_reserve_packet_hash": handoff.reserve_packet_hash,
        "nav_per_unit": handoff.nav_per_unit_usd_e8,
        "verifier_kind": handoff.proof_profile,
        "min_attestations": handoff.proof_attestation_count,
        "source_class": "sp1-groth16-existing-proof-attested",
        "halted": False,
    }
    if any(profile.get(field) != expected_value for field, expected_value in expected.items()):
        raise HandoffCheckError("live NAV profile does not match proof-assurance pins")
    return profile


def _single_line(
    value: Any, handoff: PersistentPftlHandoff
) -> Mapping[str, Any]:
    report = _mapping(value, "account_lines")
    if (
        report.get("chain_id") != handoff.chain_id
        or report.get("genesis_hash") != handoff.genesis_hash
        or report.get("account") != handoff.coordinator_address
        or report.get("asset_id") != handoff.asset_id
    ):
        raise HandoffCheckError("coordinator trustline response identity mismatch")
    lines = report.get("lines")
    if not isinstance(lines, list):
        raise HandoffCheckError("coordinator trustlines are malformed")
    matches = [
        line
        for line in lines
        if isinstance(line, Mapping) and line.get("asset_id") == handoff.asset_id
    ]
    if len(matches) != 1:
        raise HandoffCheckError("coordinator must have one pinned NAVcoin trustline")
    line = matches[0]
    if (
        line.get("account") != handoff.coordinator_address
        or line.get("issuer") != handoff.asset_issuer
        or line.get("code") != handoff.asset_code
        or line.get("precision") != handoff.asset_precision
        or line.get("authorized") is not True
        or line.get("frozen") is not False
    ):
        raise HandoffCheckError("coordinator trustline controls or metadata mismatch")
    _uint(line.get("balance"), "coordinator NAVcoin balance", minimum=1)
    _uint(line.get("limit"), "coordinator NAVcoin trustline limit", minimum=1)
    if line["balance"] > line["limit"]:
        raise HandoffCheckError("coordinator NAVcoin balance exceeds trustline limit")
    return line


def _asset(value: Any, handoff: PersistentPftlHandoff) -> Mapping[str, Any]:
    report = _mapping(value, "asset_info")
    if (
        report.get("chain_id") != handoff.chain_id
        or report.get("genesis_hash") != handoff.genesis_hash
        or report.get("found") is not True
    ):
        raise HandoffCheckError("asset response identity mismatch")
    asset = _mapping(report.get("asset"), "asset_info.asset")
    expected = {
        "asset_id": handoff.asset_id,
        "issuer": handoff.asset_issuer,
        "code": handoff.asset_code,
        "display_name": "Proven NAVcoin",
        "precision": handoff.asset_precision,
        "outstanding_supply": handoff.circulating_supply_atoms,
        "freeze_enabled": False,
        "clawback_enabled": False,
        "requires_authorization": False,
    }
    if any(asset.get(field) != expected_value for field, expected_value in expected.items()):
        raise HandoffCheckError("live issued asset does not match handoff pins")
    return asset


def _status_anchor(
    value: Any, handoff: PersistentPftlHandoff
) -> dict[str, Any]:
    """Validate and normalize one finalized validator status identity."""

    status = _mapping(value, "status")
    if (
        status.get("chain_id") != handoff.chain_id
        or status.get("genesis_hash") != handoff.genesis_hash
        or status.get("protocol_version") != 1
        or status.get("rpc_schema") != handoff.rpc_protocol
        or status.get("validator_count") != 6
        or status.get("build_git_revision")
        != handoff.binary_build_git_revision
        or status.get("status") != "running"
    ):
        raise HandoffCheckError("validator status identity or build mismatch")
    height = _uint(status.get("block_height"), "block_height")
    if height < handoff.handoff_height:
        raise HandoffCheckError("validator height predates the signed handoff")
    mempool_pending = _uint(status.get("mempool_pending"), "mempool_pending")
    if mempool_pending != 0:
        raise HandoffCheckError("PFTL mempool is not empty")
    profile = _single_profile(status, handoff)
    return {
        "node_id": _text(status.get("node_id"), "node_id", maximum=64),
        "height": height,
        "block_tip_hash": _hex48(
            status.get("block_tip_hash"), "block_tip_hash"
        ),
        "state_root": _hex48(status.get("state_root"), "state_root"),
        "build_git_revision": status["build_git_revision"],
        "mempool_pending": mempool_pending,
        "profile": dict(profile),
    }


def _topology(handoff: PersistentPftlHandoff) -> dict[str, Any]:
    try:
        value = json.loads(handoff.topology_file.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise HandoffCheckError("pinned topology is unavailable or malformed") from error
    report = _mapping(value, "topology")
    if (
        report.get("chain_id") != handoff.chain_id
        or report.get("genesis_hash") != handoff.genesis_hash
        or report.get("protocol_version") != 1
    ):
        raise HandoffCheckError("topology identity does not match handoff")
    peers = report.get("peers")
    if not isinstance(peers, list) or len(peers) != 6:
        raise HandoffCheckError("topology must contain exactly six validators")
    node_ids = {
        peer.get("node_id")
        for peer in peers
        if isinstance(peer, Mapping)
    }
    if node_ids != {f"validator-{index}" for index in range(6)}:
        raise HandoffCheckError("topology validator identities are not canonical")
    return {"validator_count": 6, "topology_id": _hex48(report.get("topology_id"), "topology_id")}


@dataclass(frozen=True)
class PftlHandoffDryCheck:
    handoff: PersistentPftlHandoff
    client_factory: Callable[[str], Any] = default_client_factory

    def run(self) -> dict[str, Any]:
        """Read status, asset, account and trustline from all six replicas."""

        artifact_checks = self.handoff.verify_artifacts()
        topology = _topology(self.handoff)
        clients = tuple(
            self.client_factory(endpoint) for endpoint in self.handoff.rpc_endpoints
        )
        try:
            rows = [
                {
                    "status_before": client.status(),
                    "asset": client.asset_info(self.handoff.asset_id),
                    "lines": client.account_lines(
                        self.handoff.coordinator_address,
                        asset_id=self.handoff.asset_id,
                        limit=8,
                    ),
                    "account": client.account(self.handoff.coordinator_address),
                    "status_after": client.status(),
                }
                for client in clients
            ]
        except Exception as error:
            raise HandoffCheckError("one or more pinned PFTL RPC reads failed") from error
        if len(rows) != 6:
            raise HandoffCheckError("six pinned PFTL RPC views are required")

        node_ids: set[str] = set()
        canonical_views: list[str] = []
        normalized_rows: list[dict[str, Any]] = []
        for row in rows:
            status = _status_anchor(row["status_before"], self.handoff)
            status_after = _status_anchor(row["status_after"], self.handoff)
            if status != status_after:
                raise HandoffCheckError(
                    "finalized view changed during persistent-handoff RPC read"
                )
            node_id = status["node_id"]
            if node_id in node_ids:
                raise HandoffCheckError("RPC views do not come from six distinct validators")
            node_ids.add(node_id)
            height = status["height"]
            profile = status["profile"]
            asset = _asset(row["asset"], self.handoff)
            line = _single_line(row["lines"], self.handoff)
            account = _mapping(row["account"], "coordinator account")
            if account.get("address") != self.handoff.coordinator_address:
                raise HandoffCheckError("coordinator account address mismatch")
            sequence = _uint(account.get("sequence"), "coordinator sequence")
            native_balance = _uint(
                account.get("balance"), "coordinator native balance", minimum=1
            )
            normalized = {
                "height": height,
                "block_tip_hash": status["block_tip_hash"],
                "state_root": status["state_root"],
                "build_git_revision": status["build_git_revision"],
                "profile": {
                    "profile_id": profile["profile_id"],
                    "finalized_epoch": profile["finalized_epoch"],
                    "finalized_reserve_packet_hash": profile[
                        "finalized_reserve_packet_hash"
                    ],
                    "nav_per_unit": profile["nav_per_unit"],
                    "verifier_kind": profile["verifier_kind"],
                    "min_attestations": profile["min_attestations"],
                    "source_class": profile["source_class"],
                    "halted": profile["halted"],
                },
                "asset": {
                    field: asset[field]
                    for field in (
                        "asset_id",
                        "issuer",
                        "code",
                        "display_name",
                        "precision",
                        "outstanding_supply",
                        "freeze_enabled",
                        "clawback_enabled",
                        "requires_authorization",
                    )
                },
                "coordinator": {
                    "address": self.handoff.coordinator_address,
                    "sequence": sequence,
                    "native_balance": native_balance,
                    "inventory_atoms": line["balance"],
                    "trustline_limit_atoms": line["limit"],
                },
            }
            normalized_rows.append(normalized)
            canonical_views.append(
                json.dumps(
                    normalized,
                    sort_keys=True,
                    separators=(",", ":"),
                    ensure_ascii=True,
                    allow_nan=False,
                )
            )
        if node_ids != {f"validator-{index}" for index in range(6)}:
            raise HandoffCheckError("live validator identities do not match topology")
        if len(set(canonical_views)) != 1:
            raise HandoffCheckError("pinned PFTL views are not converged six-of-six")

        agreed = normalized_rows[0]
        result = {
            "schema": "postfiat.lightning.pftl_handoff_dry_check.v1",
            "ok": True,
            "mode": "READ_ONLY_NO_LND_NO_CHAIN_MUTATION",
            "handoff": self.handoff.public_pins(),
            "artifacts": artifact_checks,
            "topology": topology,
            "live": {
                "agreeing_validator_count": 6,
                "validator_count": 6,
                "height": agreed["height"],
                "block_tip_hash": agreed["block_tip_hash"],
                "state_root": agreed["state_root"],
                "build_git_revision": agreed["build_git_revision"],
                "nav_epoch": agreed["profile"]["finalized_epoch"],
                "nav_per_unit_usd_e8": agreed["profile"]["nav_per_unit"],
                "coordinator_inventory_atoms": agreed["coordinator"][
                    "inventory_atoms"
                ],
                "coordinator_sequence": agreed["coordinator"]["sequence"],
                "mempool_pending": 0,
            },
            "assurance_boundary": {
                "on_chain_profile": self.handoff.proof_profile,
                "proof_bytes_stored_on_chain": (
                    self.handoff.proof_bytes_stored_on_chain
                ),
                "consensus_native_groth16_verification": (
                    self.handoff.consensus_native_groth16_verification
                ),
            },
        }
        _assert_secret_free(result)
        return result


_SECRET_FIELD_MARKERS = (
    "private_key",
    "mnemonic",
    "seed",
    "macaroon",
    "wallet_password",
    "fulfillment",
    "preimage",
)


def _assert_secret_free(value: Any, path: str = "$") -> None:
    if isinstance(value, Mapping):
        for key, child in value.items():
            lowered = str(key).lower()
            if any(marker in lowered for marker in _SECRET_FIELD_MARKERS):
                raise HandoffCheckError(
                    f"secret-bearing field is forbidden in dry-check output: {path}.{key}"
                )
            _assert_secret_free(child, f"{path}.{key}")
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        for index, child in enumerate(value):
            _assert_secret_free(child, f"{path}[{index}]")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="read-only six-validator PFTL persistent-handoff dry check"
    )
    parser.add_argument("--handoff", type=Path, default=DEFAULT_HANDOFF_PATH)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        result = PftlHandoffDryCheck(load_persistent_handoff(args.handoff)).run()
    except (HandoffError, OSError, ValueError) as error:
        result = {
            "schema": "postfiat.lightning.pftl_handoff_dry_check.v1",
            "ok": False,
            "mode": "READ_ONLY_NO_LND_NO_CHAIN_MUTATION",
            "error": str(error),
        }
        print(json.dumps(result, sort_keys=True, separators=(",", ":")))
        return 1
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
