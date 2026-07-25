"""Strict parser for the persistent proven-NAV PFTL public handoff.

The handoff is treated as a pinned release manifest, not as ambient discovery.
Loading it performs no RPC calls and never opens a signer file.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import stat
from typing import Any, Mapping, Sequence
from urllib.parse import urlsplit


HANDOFF_SCHEMA = "postfiat.lightning.navcoin.orc3_handoff.v1"
DEFAULT_HANDOFF_PATH = Path(
    "/home/postfiat/tmp/pftl-proven-nav-v2-20260724/public/orc3-handoff.json"
)
PINNED_HANDOFF_SHA256 = (
    "4e348ca873efed841f7d352a187e756aaf4fd505c42feb0b80203f54939e8f3b"
)
PINNED_CERTIFICATION_HELPER_SHA256 = (
    "193deb019ce932ee0c9eb220801cb70f9b8088002b1377a5e2b8b5e5c6894bd3"
)
MAX_HANDOFF_BYTES = 128 * 1024
HEX_32 = re.compile(r"^[0-9a-f]{64}$")
HEX_48 = re.compile(r"^[0-9a-f]{96}$")
PFTL_ADDRESS = re.compile(r"^pf[0-9a-f]{40}$")


class HandoffError(ValueError):
    """A public handoff or one of its pinned artifacts is invalid."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _strict_object(
    value: Any,
    expected: frozenset[str],
    label: str,
) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise HandoffError(f"{label} must be an object")
    fields = frozenset(value)
    if fields != expected:
        raise HandoffError(
            f"{label} fields mismatch; "
            f"missing={sorted(expected - fields)}, unknown={sorted(fields - expected)}"
        )
    return value


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise HandoffError(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def _read_json(path: Path, *, maximum_bytes: int = MAX_HANDOFF_BYTES) -> Any:
    try:
        file_stat = path.lstat()
    except OSError as error:
        raise HandoffError(f"handoff is unavailable: {path}") from error
    if stat.S_ISLNK(file_stat.st_mode) or not stat.S_ISREG(file_stat.st_mode):
        raise HandoffError("handoff must be a regular, non-symlink file")
    if file_stat.st_size < 2 or file_stat.st_size > maximum_bytes:
        raise HandoffError("handoff size is outside the allowed bound")
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=lambda value: (_ for _ in ()).throw(
                HandoffError(f"non-finite JSON number: {value}")
            ),
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise HandoffError("handoff is not canonical UTF-8 JSON") from error


def _text(value: Any, label: str, *, maximum: int = 4096) -> str:
    if (
        type(value) is not str
        or not value
        or len(value) > maximum
        or not value.isascii()
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
    ):
        raise HandoffError(f"{label} must be bounded printable ASCII")
    return value


def _uint(value: Any, label: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum or value > (1 << 63) - 1:
        raise HandoffError(f"{label} must be uint63 >= {minimum}")
    return value


def _hex(value: Any, label: str, pattern: re.Pattern[str]) -> str:
    result = _text(value, label)
    if pattern.fullmatch(result) is None:
        raise HandoffError(f"{label} must be canonical lowercase hexadecimal")
    return result


def _address(value: Any, label: str) -> str:
    result = _text(value, label)
    if PFTL_ADDRESS.fullmatch(result) is None:
        raise HandoffError(f"{label} must be a canonical PFTL address")
    return result


def _absolute_path(value: Any, label: str) -> Path:
    text = _text(value, label, maximum=1024)
    path = Path(text)
    if not path.is_absolute() or ".." in path.parts:
        raise HandoffError(f"{label} must be an absolute normalized path")
    return path


def _endpoint(value: Any, label: str) -> str:
    text = _text(value, label, maximum=256)
    try:
        parsed = urlsplit(text)
        port = parsed.port
    except ValueError as error:
        raise HandoffError(f"{label} is not a valid endpoint") from error
    if (
        parsed.scheme != "tcp"
        or parsed.hostname not in {"127.0.0.1", "localhost", "::1"}
        or port is None
        or port < 1
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path
        or parsed.query
        or parsed.fragment
    ):
        raise HandoffError(f"{label} must be an explicit loopback tcp endpoint")
    return text


@dataclass(frozen=True)
class PersistentPftlHandoff:
    """Immutable pins consumed by the coordinator and read-only dry check."""

    handoff_path: Path
    handoff_sha256: str
    chain_id: str
    genesis_hash: str
    handoff_height: int
    handoff_state_root: str
    validator_count: int
    data_root: Path
    topology_file: Path
    binary_path: Path
    binary_build_git_revision: str
    binary_sha256: str
    rpc_protocol: str
    rpc_endpoints: tuple[str, ...]
    asset_id: str
    asset_code: str
    asset_precision: int
    asset_issuer: str
    circulating_supply_atoms: int
    nav_epoch: int
    verified_net_assets_usd_e8: int
    nav_per_unit_usd_e8: int
    reserve_packet_hash: str
    profile_id: str
    coordinator_address: str
    user_address: str
    certification_helper: Path
    certification_helper_sha256: str
    proof_lifecycle: tuple[str, ...]
    proof_profile: str
    proof_attestation_count: int
    proof_bytes_stored_on_chain: bool
    consensus_native_groth16_verification: bool

    def public_pins(self) -> dict[str, Any]:
        return {
            "schema": "postfiat.lightning.pftl_handoff_pins.v1",
            "handoff_sha256": self.handoff_sha256,
            "chain_id": self.chain_id,
            "genesis_hash": self.genesis_hash,
            "binary": {
                "build_git_revision": self.binary_build_git_revision,
                "sha256": self.binary_sha256,
            },
            "rpc_endpoints": list(self.rpc_endpoints),
            "asset": {
                "asset_id": self.asset_id,
                "code": self.asset_code,
                "precision": self.asset_precision,
                "issuer": self.asset_issuer,
                "circulating_supply_atoms": self.circulating_supply_atoms,
            },
            "nav": {
                "epoch": self.nav_epoch,
                "verified_net_assets_usd_e8": self.verified_net_assets_usd_e8,
                "nav_per_unit_usd_e8": self.nav_per_unit_usd_e8,
                "valuation_unit": "USD_PER_WHOLE_ASSET_UNIT",
                "valuation_scale": 100_000_000,
                "reserve_packet_hash": self.reserve_packet_hash,
                "profile_id": self.profile_id,
            },
            "coordinator_address": self.coordinator_address,
            "assurance": {
                "lifecycle": list(self.proof_lifecycle),
                "profile": self.proof_profile,
                "attestation_count": self.proof_attestation_count,
                "proof_bytes_stored_on_chain": self.proof_bytes_stored_on_chain,
                "consensus_native_groth16_verification": (
                    self.consensus_native_groth16_verification
                ),
            },
            "certification_helper_sha256": self.certification_helper_sha256,
        }

    def verify_artifacts(self) -> dict[str, Any]:
        """Re-hash executable artifacts without reading any signer material."""

        checks: dict[str, Any] = {}
        for label, path, expected in (
            ("binary", self.binary_path, self.binary_sha256),
            (
                "certification_helper",
                self.certification_helper,
                self.certification_helper_sha256,
            ),
        ):
            try:
                file_stat = path.lstat()
            except OSError as error:
                raise HandoffError(f"pinned {label} is unavailable") from error
            if stat.S_ISLNK(file_stat.st_mode) or not stat.S_ISREG(file_stat.st_mode):
                raise HandoffError(f"pinned {label} must be a regular non-symlink file")
            if not os.access(path, os.X_OK):
                raise HandoffError(f"pinned {label} is not executable")
            observed = sha256_file(path)
            if observed != expected:
                raise HandoffError(f"pinned {label} SHA-256 mismatch")
            checks[label] = {"sha256": observed, "executable": True}
        return checks

    def assert_policy_matches(self, policy: Any) -> None:
        """Bind a separately configured LND/value policy to this handoff."""

        expected = {
            "pftl_chain_id": self.chain_id,
            "pftl_genesis_hash": self.genesis_hash,
            "pftl_asset_id": self.asset_id,
            "pftl_rpc_endpoints": self.rpc_endpoints,
            "pftl_nav_epoch": self.nav_epoch,
            "pftl_nav_reserve_packet_hash": self.reserve_packet_hash,
            "pftl_nav_valuation_unit": "USD_PER_WHOLE_ASSET_UNIT",
            "pftl_nav_valuation_scale": 100_000_000,
            "pftl_nav_per_unit_usd_e8": self.nav_per_unit_usd_e8,
            "coordinator_pftl_address": self.coordinator_address,
            "minimum_pftl_validators": 6,
            "require_non_freezable": True,
        }
        mismatches = [
            field
            for field, expected_value in expected.items()
            if getattr(policy, field, None) != expected_value
        ]
        if mismatches:
            raise HandoffError(
                f"real-value policy does not match PFTL handoff pins: {mismatches}"
            )


def load_persistent_handoff(
    path: str | Path = DEFAULT_HANDOFF_PATH,
    *,
    expected_handoff_sha256: str = PINNED_HANDOFF_SHA256,
    expected_certification_helper_sha256: str = PINNED_CERTIFICATION_HELPER_SHA256,
) -> PersistentPftlHandoff:
    """Load one exact, digest-pinned public handoff."""

    handoff_path = Path(path)
    expected_handoff_sha256 = _hex(
        expected_handoff_sha256, "expected handoff SHA-256", HEX_32
    )
    observed_handoff_sha256 = sha256_file(handoff_path)
    if observed_handoff_sha256 != expected_handoff_sha256:
        raise HandoffError("public handoff SHA-256 does not match the release pin")
    document = _strict_object(
        _read_json(handoff_path),
        frozenset(
            {
                "schema",
                "status",
                "chain",
                "binary",
                "rpc",
                "navcoin",
                "accounts",
                "live_escrow",
                "hashlock_encoding",
                "proof_assurance",
            }
        ),
        "handoff",
    )
    if document["schema"] != HANDOFF_SCHEMA or document["status"] != "ready":
        raise HandoffError("handoff is not a ready supported release")

    chain = _strict_object(
        document["chain"],
        frozenset(
            {
                "chain_id",
                "genesis_hash",
                "height",
                "state_root",
                "validator_count",
                "consensus_v2_activation_height",
                "data_root",
                "topology_file",
            }
        ),
        "chain",
    )
    if _uint(chain["validator_count"], "validator_count") != 6:
        raise HandoffError("persistent handoff must contain exactly six validators")
    if _uint(
        chain["consensus_v2_activation_height"],
        "consensus_v2_activation_height",
    ) != 1:
        raise HandoffError("unexpected consensus activation height")
    data_root = _absolute_path(chain["data_root"], "chain.data_root")

    binary = _strict_object(
        document["binary"],
        frozenset({"path", "build_git_revision", "sha256"}),
        "binary",
    )
    rpc = _strict_object(
        document["rpc"],
        frozenset({"protocol", "primary", "endpoints", "status_request"}),
        "rpc",
    )
    endpoints_value = rpc["endpoints"]
    if (
        not isinstance(endpoints_value, Sequence)
        or isinstance(endpoints_value, (str, bytes))
        or len(endpoints_value) != 6
    ):
        raise HandoffError("handoff must pin exactly six RPC endpoints")
    endpoints = tuple(
        _endpoint(endpoint, f"rpc.endpoints[{index}]")
        for index, endpoint in enumerate(endpoints_value)
    )
    if len(set(endpoints)) != 6 or _endpoint(rpc["primary"], "rpc.primary") != endpoints[0]:
        raise HandoffError("RPC endpoints must be distinct and primary must be first")
    if rpc["protocol"] != "newline-delimited JSON, postfiat-local-rpc-v1":
        raise HandoffError("unsupported PFTL RPC protocol")
    status_request = _strict_object(
        rpc["status_request"],
        frozenset({"version", "id", "method", "params"}),
        "rpc.status_request",
    )
    if (
        status_request["version"] != "postfiat-local-rpc-v1"
        or status_request["method"] != "status"
        or status_request["params"] != {}
    ):
        raise HandoffError("handoff status request is not the pinned read-only shape")

    asset = _strict_object(
        document["navcoin"],
        frozenset(
            {
                "asset_id",
                "code",
                "display_name",
                "precision",
                "issuer",
                "circulating_supply_atoms",
                "finalized_epoch",
                "verified_net_assets_usd_e8",
                "nav_per_unit_usd_e8",
                "nav_per_unit_usd",
                "reserve_packet_hash",
                "profile_id",
            }
        ),
        "navcoin",
    )
    if asset["display_name"] != "Proven NAVcoin":
        raise HandoffError("unexpected NAVcoin display name")
    nav_per_unit_usd_e8 = _uint(
        asset["nav_per_unit_usd_e8"],
        "navcoin.nav_per_unit_usd_e8",
        minimum=1,
    )
    expected_nav_decimal = (
        f"{nav_per_unit_usd_e8 // 100_000_000}."
        f"{nav_per_unit_usd_e8 % 100_000_000:08d}"
    )
    if asset["nav_per_unit_usd"] != expected_nav_decimal:
        raise HandoffError(
            "NAVcoin decimal display does not match the USD-e8 valuation"
        )

    accounts = _strict_object(
        document["accounts"],
        frozenset({"coordinator", "user"}),
        "accounts",
    )
    hashlock = _strict_object(
        document["hashlock_encoding"],
        frozenset(
            {
                "condition",
                "fulfillment",
                "finish_signer",
                "cancel_signer",
                "local_certification_helper",
            }
        ),
        "hashlock_encoding",
    )
    if (
        hashlock["condition"] != "a0258020<sha256(preimage_bytes)>810120"
        or hashlock["fulfillment"] != "a0228020<32_byte_preimage_hex>"
        or hashlock["finish_signer"] != "recipient/user"
        or hashlock["cancel_signer"] != "owner/coordinator after cancel_after_height"
    ):
        raise HandoffError("handoff hashlock semantics changed")

    proof = _strict_object(
        document["proof_assurance"],
        frozenset(
            {
                "lifecycle",
                "on_chain_profile",
                "attestation_count",
                "proof_bytes_stored_on_chain",
                "consensus_native_groth16_verification",
                "note",
            }
        ),
        "proof_assurance",
    )
    lifecycle = proof["lifecycle"]
    expected_lifecycle = (
        "nav_reserve_submit",
        "nav_reserve_attest",
        "nav_epoch_finalize",
    )
    if not isinstance(lifecycle, list) or tuple(lifecycle) != expected_lifecycle:
        raise HandoffError("unexpected NAV proof lifecycle")
    if (
        proof["on_chain_profile"] != "multi-fetch-quorum"
        or _uint(proof["attestation_count"], "attestation_count", minimum=1) != 1
        or proof["proof_bytes_stored_on_chain"] is not True
        or proof["consensus_native_groth16_verification"] is not False
    ):
        raise HandoffError("handoff proof assurance does not match the reviewed lane")

    helper_sha = _hex(
        expected_certification_helper_sha256,
        "expected certification helper SHA-256",
        HEX_32,
    )
    return PersistentPftlHandoff(
        handoff_path=handoff_path.resolve(),
        handoff_sha256=observed_handoff_sha256,
        chain_id=_text(chain["chain_id"], "chain_id", maximum=128),
        genesis_hash=_hex(chain["genesis_hash"], "genesis_hash", HEX_48),
        handoff_height=_uint(chain["height"], "chain.height", minimum=1),
        handoff_state_root=_hex(chain["state_root"], "chain.state_root", HEX_48),
        validator_count=6,
        data_root=data_root,
        topology_file=_absolute_path(chain["topology_file"], "chain.topology_file"),
        binary_path=_absolute_path(binary["path"], "binary.path"),
        binary_build_git_revision=_text(
            binary["build_git_revision"], "binary.build_git_revision", maximum=64
        ),
        binary_sha256=_hex(binary["sha256"], "binary.sha256", HEX_32),
        rpc_protocol="postfiat-local-rpc-v1",
        rpc_endpoints=endpoints,
        asset_id=_hex(asset["asset_id"], "navcoin.asset_id", HEX_48),
        asset_code=_text(asset["code"], "navcoin.code", maximum=32),
        asset_precision=_uint(asset["precision"], "navcoin.precision"),
        asset_issuer=_address(asset["issuer"], "navcoin.issuer"),
        circulating_supply_atoms=_uint(
            asset["circulating_supply_atoms"],
            "navcoin.circulating_supply_atoms",
            minimum=1,
        ),
        nav_epoch=_uint(asset["finalized_epoch"], "navcoin.finalized_epoch", minimum=1),
        verified_net_assets_usd_e8=_uint(
            asset["verified_net_assets_usd_e8"],
            "navcoin.verified_net_assets_usd_e8",
            minimum=1,
        ),
        nav_per_unit_usd_e8=nav_per_unit_usd_e8,
        reserve_packet_hash=_hex(
            asset["reserve_packet_hash"], "navcoin.reserve_packet_hash", HEX_48
        ),
        profile_id=_hex(asset["profile_id"], "navcoin.profile_id", HEX_48),
        coordinator_address=_address(
            accounts["coordinator"], "accounts.coordinator"
        ),
        user_address=_address(accounts["user"], "accounts.user"),
        certification_helper=_absolute_path(
            hashlock["local_certification_helper"],
            "hashlock_encoding.local_certification_helper",
        ),
        certification_helper_sha256=helper_sha,
        proof_lifecycle=expected_lifecycle,
        proof_profile="multi-fetch-quorum",
        proof_attestation_count=1,
        proof_bytes_stored_on_chain=True,
        consensus_native_groth16_verification=False,
    )
