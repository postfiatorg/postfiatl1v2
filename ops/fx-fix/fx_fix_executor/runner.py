"""Scheduled FxFixPacketV1 settlement batch runner, signer boundary, and RPC client."""

from __future__ import annotations

import hashlib
import json
import subprocess
import urllib.request
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Protocol

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey

from .packet import (
    FX_FIX_ACTION_BINDING_HASH_HEX_LEN,
    FX_FIX_PACKET_SCHEMA_V1,
    FX_FIX_PACKET_VERSION_V1,
    FX_FIX_RESERVATION_NONCE_HEX_LEN,
    FX_FIX_WALLET_INTENT_HASH_HEX_LEN,
    FxFixPacketV1,
    PriceObservation,
    build_fx_fix_register_op,
    build_fx_fix_reservation_create_op,
    compute_median_ratio,
    compute_source_observation_commitment,
    validate_lower_hex_len,
)
from .schedule import (
    DuplicateWindowError,
    WindowScheduleConfig,
    WindowStateStore,
    evaluate_cutoff,
)

BATCH_SCHEMA_V1 = "postfiat.fx_fix.settlement_batch.v1"
BATCH_SIGNING_DOMAIN_V1 = "postfiat.fx_fix.batch_envelope.v1"
MAX_RPC_RESPONSE_BYTES = 2 * 1024 * 1024


@dataclass(frozen=True)
class BatchOrderIntent:
    """Pre-cutoff order reservation queued for deterministic fix settlement."""

    action_binding_hash: str
    base_atoms: int
    wallet_intent_hash: str
    reservation_nonce: str
    submitted_at_unix: int
    reservation_ttl_blocks: int = 300

    def validate(self, cutoff_timestamp_unix: int) -> None:
        validate_lower_hex_len(
            "order.action_binding_hash",
            self.action_binding_hash,
            FX_FIX_ACTION_BINDING_HASH_HEX_LEN,
        )
        validate_lower_hex_len(
            "order.wallet_intent_hash",
            self.wallet_intent_hash,
            FX_FIX_WALLET_INTENT_HASH_HEX_LEN,
        )
        validate_lower_hex_len(
            "order.reservation_nonce",
            self.reservation_nonce,
            FX_FIX_RESERVATION_NONCE_HEX_LEN,
        )
        if self.base_atoms <= 0:
            raise ValueError("order.base_atoms must be nonzero")
        if self.reservation_ttl_blocks <= 0:
            raise ValueError("order.reservation_ttl_blocks must be nonzero")
        if self.submitted_at_unix > cutoff_timestamp_unix:
            raise ValueError(
                f"order_after_cutoff: submitted_at={self.submitted_at_unix} > cutoff={cutoff_timestamp_unix}"
            )


@dataclass(frozen=True)
class ExecutorConfig:
    """Configuration for a scheduled London-style FX fix pair."""

    chain_id: str
    operator: str
    base_asset_id: str
    quote_asset_id: str
    base_precision: int
    quote_precision: int
    band_bps: int
    fee_bps: int
    minimum_base_atoms: int
    capacity_base_atoms: int
    capacity_quote_atoms: int
    max_fills: int
    source_label: str
    governance_policy_hash: str
    window_validity_blocks: int = 900
    nav_tracked: bool = True
    nav_fix_fee_cap_bps: int | None = None
    max_observation_age_seconds: int = 120
    max_venue_spread_bps: int = 200
    max_dispersion_bps: int = 150
    min_venues: int = 1
    schedule: WindowScheduleConfig = field(default_factory=WindowScheduleConfig)
    rpc_url: str | None = None


class BatchSigner(Protocol):
    """Pluggable signing boundary for FxFix settlement batches."""

    def sign_batch(self, batch_payload: dict[str, Any]) -> dict[str, Any]:
        """Sign canonical batch payload and return signature metadata."""


class LocalEd25519BatchSigner:
    """Deterministic Ed25519 envelope signer for tests and dry-run verification."""

    def __init__(self, key_seed_32: bytes) -> None:
        if len(key_seed_32) != 32:
            raise ValueError("Ed25519 seed must be 32 bytes")
        self._key = Ed25519PrivateKey.from_private_bytes(key_seed_32)
        self.public_key_hex = self._key.public_key().public_bytes_raw().hex()

    @staticmethod
    def signing_digest(batch_payload: dict[str, Any]) -> bytes:
        canonical = json.dumps(batch_payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
        hasher = hashlib.sha3_384()
        hasher.update(BATCH_SIGNING_DOMAIN_V1.encode("utf-8"))
        hasher.update(b"\x00")
        hasher.update(canonical)
        return hasher.digest()

    def sign_batch(self, batch_payload: dict[str, Any]) -> dict[str, Any]:
        digest = self.signing_digest(batch_payload)
        sig = self._key.sign(digest)
        return {
            "scheme": "ed25519-sha3-384-v1",
            "public_key_hex": self.public_key_hex,
            "digest_hex": digest.hex(),
            "signature_hex": sig.hex(),
        }

    @classmethod
    def verify_batch(cls, batch_payload: dict[str, Any], signature_meta: dict[str, Any]) -> bool:
        try:
            digest = cls.signing_digest(batch_payload)
            if signature_meta.get("digest_hex") != digest.hex():
                return False
            pub = Ed25519PublicKey.from_public_bytes(bytes.fromhex(signature_meta["public_key_hex"]))
            pub.verify(bytes.fromhex(signature_meta["signature_hex"]), digest)
            return True
        except Exception:
            return False


class SubprocessSigner:
    """External signer process boundary (e.g. ML-DSA-65 hardware/HSM/wallet-cli wrapper).

    Passes canonical JSON on stdin and expects JSON signature envelope on stdout.
    Private key material never enters the executor process.
    """

    def __init__(self, command: list[str], timeout_seconds: float = 30.0) -> None:
        if not command:
            raise ValueError("signer command must not be empty")
        self.command = list(command)
        self.timeout_seconds = timeout_seconds

    def sign_batch(self, batch_payload: dict[str, Any]) -> dict[str, Any]:
        payload_bytes = json.dumps(batch_payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
        proc = subprocess.run(
            self.command,
            input=payload_bytes,
            capture_output=True,
            timeout=self.timeout_seconds,
            check=False,
        )
        if proc.returncode != 0:
            raise RuntimeError(
                f"external signer failed (exit={proc.returncode}): {proc.stderr.decode('utf-8', 'replace').strip()}"
            )
        result = json.loads(proc.stdout.decode("utf-8"))
        if not isinstance(result, dict) or not result.get("signature_hex"):
            raise RuntimeError("external signer output missing signature_hex")
        return result


class FxFixRpcClient:
    """Minimal fail-closed JSON-RPC client for Post Fiat L1 FX Fix queries & submission."""

    def __init__(self, rpc_url: str, timeout: float = 15.0) -> None:
        self.rpc_url = rpc_url
        self.timeout = timeout

    def call(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        body = json.dumps(
            {"id": f"fx-fix-{method}", "method": method, "params": params or {}}
        ).encode("utf-8")
        req = urllib.request.Request(
            self.rpc_url,
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=self.timeout) as resp:
            raw = resp.read(MAX_RPC_RESPONSE_BYTES + 1)
        if len(raw) > MAX_RPC_RESPONSE_BYTES:
            raise ValueError("rpc response exceeded size cap")
        decoded = json.loads(raw.decode("utf-8"))
        if not isinstance(decoded, dict) or not decoded.get("ok"):
            raise RuntimeError(f"rpc {method} failed: {decoded}")
        return decoded["result"]

    def current_height(self) -> int:
        status = self.call("chain_status")
        return int(status["block_height"])

    def latest_fix_for_pair(self, base_asset_id: str, quote_asset_id: str) -> dict[str, Any] | None:
        listing = self.call(
            "fx_fix_list",
            {
                "base_asset_id": base_asset_id,
                "quote_asset_id": quote_asset_id,
                "active_only": False,
                "limit": 64,
            },
        )
        fixes = listing.get("fixes") or []
        if not fixes:
            return None
        latest = max(fixes, key=lambda row: int(row["state"]["packet"]["epoch"]))
        return latest["state"]["packet"]

    def submit_signed_operation(self, signed_tx: dict[str, Any]) -> dict[str, Any]:
        return self.call("mempool_submit_signed_asset_transaction", signed_tx)


class FxFixBatchRunner:
    """Builds, validates, signs, and submits deterministic London-style FX Fix batches."""

    def __init__(
        self,
        config: ExecutorConfig,
        state_store: WindowStateStore,
        *,
        signer: BatchSigner | None = None,
        rpc_client: FxFixRpcClient | None = None,
    ) -> None:
        self.config = config
        self.state_store = state_store
        self.signer = signer
        self.rpc_client = rpc_client or (FxFixRpcClient(config.rpc_url) if config.rpc_url else None)

    def execute_window(
        self,
        *,
        now_utc: datetime,
        observations: list[PriceObservation],
        current_block_height: int | None = None,
        order_intents: list[BatchOrderIntent] | None = None,
        prior_epoch: int | None = None,
        prior_packet_hash: str | None = None,
        target_local_date: str | None = None,
        dry_run: bool = False,
        allow_before_fix: bool = False,
    ) -> dict[str, Any]:
        """Run one London-style fixing window end-to-end with fail-closed checks."""
        cutoff = evaluate_cutoff(
            now_utc,
            self.config.schedule,
            target_local_date=target_local_date,
            allow_before_fix=allow_before_fix,
        )
        if not cutoff.allowed:
            raise ValueError(f"window_cutoff_rejected:{cutoff.reason}")

        window = cutoff.window
        if self.state_store.has_window(
            window.window_id,
            self.config.base_asset_id,
            self.config.quote_asset_id,
        ):
            raise DuplicateWindowError(
                f"duplicate fix window blocked before build: {window.window_id}"
            )

        # Resolve block height and previous fix chain continuity.
        height = current_block_height
        if height is None:
            if self.rpc_client is None:
                raise ValueError("current_block_height is required when rpc_client is not configured")
            height = self.rpc_client.current_height()
        if height < 0:
            raise ValueError("current_block_height must be non-negative")

        local_head = self.state_store.get_pair_head(
            self.config.base_asset_id,
            self.config.quote_asset_id,
        )
        if prior_epoch is None and prior_packet_hash is None:
            if local_head is not None:
                prior_epoch = int(local_head["epoch"])
                prior_packet_hash = str(local_head["packet_hash"])
            elif self.rpc_client is not None and not (dry_run and current_block_height is not None):
                onchain = self.rpc_client.latest_fix_for_pair(
                    self.config.base_asset_id,
                    self.config.quote_asset_id,
                )
                if onchain is not None:
                    prior_epoch = int(onchain["epoch"])
                    prior_packet_hash = str(onchain["packet_hash"])

        if prior_epoch is None:
            next_epoch = 1
            previous_fix_hash = None
        else:
            if not prior_packet_hash:
                raise ValueError("prior_packet_hash is required when prior_epoch is set")
            next_epoch = prior_epoch + 1
            previous_fix_hash = prior_packet_hash

        ratio_num, ratio_den = compute_median_ratio(
            observations,
            base_precision=self.config.base_precision,
            quote_precision=self.config.quote_precision,
            fix_timestamp_unix=window.fix_timestamp_unix,
            max_observation_age_seconds=self.config.max_observation_age_seconds,
            max_venue_spread_bps=self.config.max_venue_spread_bps,
            max_dispersion_bps=self.config.max_dispersion_bps,
            min_venues=self.config.min_venues,
        )
        obs_commitment = compute_source_observation_commitment(window.window_id, observations)

        valid_from_height = height
        expires_at_height = height + self.config.window_validity_blocks
        packet = FxFixPacketV1(
            version=FX_FIX_PACKET_VERSION_V1,
            schema=FX_FIX_PACKET_SCHEMA_V1,
            operator=self.config.operator,
            base_asset_id=self.config.base_asset_id,
            quote_asset_id=self.config.quote_asset_id,
            epoch=next_epoch,
            ratio_numerator=ratio_num,
            ratio_denominator=ratio_den,
            band_bps=self.config.band_bps,
            fee_bps=self.config.fee_bps,
            valid_from_height=valid_from_height,
            expires_at_height=expires_at_height,
            minimum_base_atoms=self.config.minimum_base_atoms,
            capacity_base_atoms=self.config.capacity_base_atoms,
            capacity_quote_atoms=self.config.capacity_quote_atoms,
            max_fills=self.config.max_fills,
            source_label=self.config.source_label,
            source_observation_commitment=obs_commitment,
            governance_policy_hash=self.config.governance_policy_hash,
            previous_fix_hash=previous_fix_hash,
        ).with_computed_hash()
        packet.validate(
            nav_tracked=self.config.nav_tracked,
            nav_fix_fee_cap_bps=self.config.nav_fix_fee_cap_bps,
        )

        register_op = build_fx_fix_register_op(
            packet,
            nav_tracked=self.config.nav_tracked,
            nav_fix_fee_cap_bps=self.config.nav_fix_fee_cap_bps,
        )

        # Deterministic settlement reservations for pre-cutoff queued orders.
        reservation_ops: list[dict[str, Any]] = []
        reservation_ids: list[str] = []
        cumulative_base = 0
        cumulative_quote = 0
        sorted_intents = sorted(
            order_intents or [],
            key=lambda item: (item.submitted_at_unix, item.wallet_intent_hash, item.reservation_nonce),
        )
        if len(sorted_intents) > packet.max_fills:
            raise ValueError(
                f"batch order count {len(sorted_intents)} exceeds packet.max_fills {packet.max_fills}"
            )
        for intent in sorted_intents:
            intent.validate(window.cutoff_timestamp_unix)
            quote_atoms, exact = packet.quote_atoms_for_base(intent.base_atoms)
            cumulative_base += intent.base_atoms
            cumulative_quote += quote_atoms
            if cumulative_base > packet.capacity_base_atoms or cumulative_quote > packet.capacity_quote_atoms:
                raise ValueError("settlement batch exceeds packet base or quote capacity")
            res_expiry = min(expires_at_height, height + intent.reservation_ttl_blocks)
            op, res_id = build_fx_fix_reservation_create_op(
                operator=packet.operator,
                fix_packet_hash=packet.packet_hash,
                action_binding_hash=intent.action_binding_hash,
                base_atoms=intent.base_atoms,
                quote_atoms=quote_atoms,
                wallet_intent_hash=intent.wallet_intent_hash,
                reservation_nonce=intent.reservation_nonce,
                expires_at_height=res_expiry,
            )
            op_entry = {
                "reservation_id": res_id,
                "exact_division": exact,
                "operation": op,
            }
            reservation_ops.append(op_entry)
            reservation_ids.append(res_id)

        batch_payload: dict[str, Any] = {
            "schema": BATCH_SCHEMA_V1,
            "chain_id": self.config.chain_id,
            "window_id": window.window_id,
            "fix_time_local_iso": window.fix_time_local_iso,
            "fix_time_utc_iso": window.fix_time_utc_iso,
            "cutoff_timestamp_unix": window.cutoff_timestamp_unix,
            "dry_run": dry_run,
            "packet_hash": packet.packet_hash,
            "epoch": packet.epoch,
            "register_operation": register_op,
            "reservation_operations": reservation_ops,
            "summary": {
                "reservation_count": len(reservation_ops),
                "total_base_atoms": cumulative_base,
                "total_quote_atoms": cumulative_quote,
                "observation_count": len(observations),
            },
        }

        signature_meta: dict[str, Any] | None = None
        if self.signer is not None:
            signature_meta = self.signer.sign_batch(batch_payload)

        submission_receipt: dict[str, Any] | None = None
        tx_hash: str | None = None
        status = "dry_run_validated" if dry_run else "prepared"

        if not dry_run and self.rpc_client is not None:
            if signature_meta is None:
                raise ValueError("live RPC submission requires a configured signer")
            submission_receipt = self.rpc_client.submit_signed_operation(
                {
                    "chain_id": self.config.chain_id,
                    "operation": register_op,
                    "batch_signature": signature_meta,
                }
            )
            tx_hash = str(submission_receipt.get("tx_hash", "")) or None
            status = "submitted"

        executed_at_utc = now_utc.astimezone(timezone.utc).isoformat()
        state_record = self.state_store.check_and_record(
            window_id=window.window_id,
            base_asset_id=packet.base_asset_id,
            quote_asset_id=packet.quote_asset_id,
            epoch=packet.epoch,
            packet_hash=packet.packet_hash,
            status=status,
            executed_at_utc=executed_at_utc,
            tx_hash=tx_hash,
            dry_run=dry_run,
        )

        return {
            "batch": batch_payload,
            "signature": signature_meta,
            "submission": submission_receipt,
            "state_record": state_record,
            "reservation_ids": reservation_ids,
        }
