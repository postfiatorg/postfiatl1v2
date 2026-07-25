"""Runnable Lightning invoice -> PFTL asset synthetic E2E and evidence suite."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any, Mapping, Sequence

from .accounting import (
    LightningSettlement,
    PftlAssetState,
    assert_cancel_delta,
    assert_create_delta,
    assert_finish_delta,
    assert_lightning_settlement,
    assert_terminal_conditional_atomicity,
)
from .coordinator.journal import CoordinatorJournal, ExposureLimits
from .coordinator.protocol import (
    SecretPreimage,
    encode_condition,
    encode_fulfillment,
    payment_hash,
)
from .coordinator.service import CoordinatorService
from .coordinator.signing import (
    Ed25519Signer,
    sign_quote,
    verify_signed_quote,
)
from .crash_matrix import (
    crash_after_outgoing_lightning_payment,
    crash_service_transition,
    run_crash_matrix,
)
from .evidence import EvidenceBundle, REDACTED, sha256_file, verify_bundle
from .lightning import DirectLncliGrpc, LightningTransportError, PaymentResult
from .pftl.harness import PftlDevnet
from .safety import SafetyEnvelope
from .wallet.validation import (
    TimelockPolicy,
    ValidationError,
    validate_invoice_against_quote,
    validate_pftl_lock_views,
)


MAX_TOTAL_CLTV_DELTA = 288
MIN_FINAL_CLTV_DELTA = 144
INVOICE_EXPIRY_SECONDS = 900
REFUND_INVOICE_EXPIRY_SECONDS = 30
SAFE_PFTL_CANCEL_OFFSET = 400
SHORT_REFUND_CANCEL_OFFSET = 10
FLOW_A_MSAT = 10_000_000
FLOW_A_ATOMS = 10_000_000
FLOW_B_MSAT = 4_000_000
FLOW_B_ATOMS = 4_000_000
REFUND_MSAT = 1_000_000
REFUND_ATOMS = 1_000_000
ROUTE_FAILURE_MSAT = 3_000_000_000
CRASH_RECOVERY_MSAT = 2_000_000
CRASH_RECOVERY_ATOMS = 2_000_000


class DemoFailure(RuntimeError):
    """A live synthetic acceptance gate failed."""


@dataclass(frozen=True)
class QuoteContext:
    label: str
    payer_node: str
    invoice_secret_known_to_coordinator: SecretPreimage | None
    signed_quote: Mapping[str, Any]
    quote: Mapping[str, Any]
    plan: Mapping[str, Any]
    decoded_invoice: Mapping[str, Any]

    @property
    def swap_id(self) -> str:
        return str(self.quote["swap_id"])

    @property
    def escrow_id(self) -> str:
        return str(self.quote["expected_escrow_id"])


def _atomic_private_bytes(path: Path, value: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL,
        0o600,
    )
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(value)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def _load_or_create_quote_signer(path: Path) -> Ed25519Signer:
    if path.exists():
        seed = path.read_bytes()
    else:
        seed = os.urandom(32)
        _atomic_private_bytes(path, seed)
    if len(seed) != 32:
        raise DemoFailure("coordinator Ed25519 seed has invalid length")
    return Ed25519Signer.from_private_bytes(seed)


def _json_command(command: Sequence[str], *, timeout: float = 300) -> Mapping[str, Any]:
    completed = subprocess.run(
        list(command),
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )
    if completed.returncode != 0:
        raise DemoFailure(
            f"command failed rc={completed.returncode}: {completed.stderr.strip()}"
        )
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise DemoFailure("command returned non-JSON output") from error
    if not isinstance(value, Mapping):
        raise DemoFailure("command JSON is not an object")
    return value


def _issued_balance(snapshot: Mapping[str, Any], address: str) -> int:
    report = snapshot["rows"][0]["accounts"][address]
    lines = report.get("lines", [])
    matches = [
        row
        for row in lines
        if row.get("asset_id") == snapshot["asset_id"]
        and row.get("account") == address
    ]
    if len(matches) != 1:
        raise DemoFailure(f"expected one issued-asset line for {address}")
    value = matches[0].get("balance")
    if type(value) is not int or value < 0:
        raise DemoFailure("issued-asset balance is invalid")
    return value


def _accounting_state(
    snapshot: Mapping[str, Any],
    *,
    owner: str,
    recipient: str,
) -> PftlAssetState:
    escrow_report = snapshot["rows"][0].get("escrow")
    escrow = (
        escrow_report.get("escrow")
        if isinstance(escrow_report, Mapping)
        else None
    )
    state = None
    if isinstance(escrow, Mapping):
        raw_state = escrow.get("state")
        if isinstance(raw_state, str):
            state = raw_state.upper()
    return PftlAssetState(
        owner_spendable_atoms=_issued_balance(snapshot, owner),
        recipient_spendable_atoms=_issued_balance(snapshot, recipient),
        open_escrow_atoms=int(snapshot["open_escrow_total"]),
        issued_supply_atoms=int(snapshot["outstanding_supply"]),
        escrow_state=state,
    )


def _channel_state(lnd: DirectLncliGrpc) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for node in ("user", "coordinator", "router"):
        report = lnd.list_channels(node)
        channels = report.get("channels", [])
        if not isinstance(channels, list):
            raise DemoFailure("LND listchannels returned an invalid shape")
        public_channels: list[dict[str, Any]] = []
        total_local = 0
        for channel in channels:
            if not isinstance(channel, Mapping) or channel.get("active") is not True:
                raise DemoFailure(f"{node} has a non-active channel")
            local = int(channel["local_balance"])
            total_local += local
            public_channels.append(
                {
                    "chan_id": channel["chan_id"],
                    "channel_point": channel["channel_point"],
                    "capacity_sat": int(channel["capacity"]),
                    "local_balance_sat": local,
                    "remote_balance_sat": int(channel["remote_balance"]),
                    "unsettled_balance_sat": int(channel["unsettled_balance"]),
                    "total_sent_sat": int(channel["total_satoshis_sent"]),
                    "total_received_sat": int(channel["total_satoshis_received"]),
                    "active": True,
                }
            )
        result[node] = {
            "local_balance_sat": total_local,
            "channels": public_channels,
        }
    return result


def _assert_channel_delta(
    before: Mapping[str, Any],
    after: Mapping[str, Any],
    *,
    payer: str,
    receiver: str,
    payment: PaymentResult,
) -> dict[str, int]:
    if payment.value_msat % 1000 or payment.fee_msat % 1000:
        raise DemoFailure("demo requires sat-aligned Lightning amount and fee")
    amount_sat = payment.value_msat // 1000
    fee_sat = payment.fee_msat // 1000
    actual = {
        node: int(after[node]["local_balance_sat"])
        - int(before[node]["local_balance_sat"])
        for node in ("user", "coordinator", "router")
    }
    expected = {"user": 0, "coordinator": 0, "router": fee_sat}
    expected[payer] = -(amount_sat + fee_sat)
    expected[receiver] = amount_sat
    if actual != expected:
        raise DemoFailure(
            f"Lightning channel delta mismatch: actual={actual}, expected={expected}"
        )
    if sum(actual.values()) != 0:
        raise DemoFailure("Lightning channel balances did not conserve satoshis")
    return actual


class SyntheticDemo:
    def __init__(
        self,
        *,
        devnet: PftlDevnet,
        env_script: Path,
        evidence_dir: Path,
        run_id: str,
    ) -> None:
        self.devnet = devnet
        self.env_script = env_script.resolve()
        self.evidence = EvidenceBundle(evidence_dir, run_id)
        self.lnd = DirectLncliGrpc(self.env_script)
        private_run = self.devnet.root / "private" / "demo-runs" / run_id
        private_run.mkdir(parents=True, exist_ok=False)
        os.chmod(private_run, 0o700)
        self.signer = _load_or_create_quote_signer(
            self.devnet.root / "private" / "coordinator-ed25519.seed"
        )
        self.signer_public_key = self.signer.public_key_bytes()
        self.journal = CoordinatorJournal(
            private_run / "coordinator.sqlite3",
            ExposureLimits(
                per_principal_atoms=100_000_000,
                aggregate_atoms=250_000_000,
            ),
        )
        self.service = CoordinatorService(self.journal)
        self.test_vectors: list[dict[str, str]] = []
        self.results: dict[str, Any] = {}
        self.initial_pftl_supply: int | None = None
        self.initial_user_atoms: int | None = None
        self.initial_coordinator_atoms: int | None = None

    def close(self) -> None:
        self.journal.close()

    def _snapshot(self, escrow_id: str | None = None, tx_id: str | None = None):
        asset = self.devnet.manifest["asset"]["asset_id"]
        return self.devnet.consensus_snapshot(
            asset_id=asset,
            accounts=[
                self.devnet.manifest["roles"]["coordinator"]["address"],
                self.devnet.manifest["roles"]["user"]["address"],
            ],
            escrow_id=escrow_id,
            tx_id=tx_id,
        )

    def _capture_finality(
        self,
        effect: Mapping[str, Any],
        *,
        label: str,
    ) -> Mapping[str, Any]:
        """Copy one hash-checked, claim-material-free certificate bundle."""

        effect_key = effect.get("effect_key")
        if type(effect_key) is not str or not effect_key:
            raise DemoFailure(f"{label} effect has no effect_key")
        if "finality_proof_path" in effect:
            proof = self.devnet.public_finality_proof(effect_key)
            source_path = Path(str(effect["finality_proof_path"])).resolve()
            expected_hash = str(effect["finality_proof_sha256"])
        else:
            reference = effect.get("public_finality_proof")
            if not isinstance(reference, Mapping):
                raise DemoFailure(f"{label} effect has no public finality proof")
            source_path = Path(str(reference.get("path", ""))).resolve()
            expected_hash = str(reference.get("sha256", ""))
            public_root = (self.devnet.root / "evidence" / "finality").resolve()
            if (
                public_root not in source_path.parents
                or not source_path.is_file()
                or sha256_file(source_path) != expected_hash
            ):
                raise DemoFailure(f"{label} public finality reference failed")
            proof = json.loads(source_path.read_text(encoding="utf-8"))
            if (
                not isinstance(proof, Mapping)
                or proof.get("effect_key") != effect_key
            ):
                raise DemoFailure(f"{label} finality proof is not effect-bound")
        if sha256_file(source_path) != expected_hash:
            raise DemoFailure(f"{label} finality proof hash changed")
        destination = self.evidence.write_json(
            f"pftl/finality/{label}.json",
            proof,
        )
        captured = {
            "effect_key": effect_key,
            "certificate_id": proof["certificate"]["certificate_id"],
            "vote_count": proof["checks"]["vote_count"],
            "quorum": proof["checks"]["quorum"],
            "bundle_path": destination.relative_to(self.evidence.root).as_posix(),
            "bundle_sha256": sha256_file(destination),
            "source_sha256": expected_hash,
        }
        self.evidence.record("pftl_finality_captured", captured)
        return captured

    def preflight(self) -> None:
        infos = {node: self.lnd.get_info(node) for node in ("user", "coordinator", "router")}
        snapshot = self._snapshot()
        envelope = SafetyEnvelope(
            bitcoin_network="regtest",
            pftl_chain_id=self.devnet.manifest["chain_id"],
            pftl_genesis_hash=self.devnet.manifest["genesis_hash"],
            pftl_asset_symbol=self.devnet.manifest["asset"]["code"],
            pftl_asset_id=self.devnet.manifest["asset"]["asset_id"],
            run_root=self.evidence.root,
            bitcoin_rpc_endpoint="172.30.24.10:18443",
            lnd_endpoints=(
                "172.30.24.11:10009",
                "172.30.24.12:10009",
                "172.30.24.13:10009",
            ),
            pftl_rpc_endpoints=tuple(
                self.devnet.endpoint(index) for index in range(6)
            ),
        )
        envelope.validate()
        if snapshot["agreeing_validator_count"] != 6:
            raise DemoFailure("PFTL preflight is not converged 6/6")
        self.initial_pftl_supply = int(snapshot["outstanding_supply"])
        self.initial_user_atoms = _issued_balance(
            snapshot, self.devnet.manifest["roles"]["user"]["address"]
        )
        self.initial_coordinator_atoms = _issued_balance(
            snapshot, self.devnet.manifest["roles"]["coordinator"]["address"]
        )
        if snapshot["rows"][0]["asset"]["asset"].get("freeze_enabled") is not False:
            raise DemoFailure("test asset is unexpectedly freezable")
        bitcoin = _json_command(
            [str(self.env_script), "bitcoin-cli", "getblockchaininfo"]
        )
        if bitcoin.get("chain") != "regtest":
            raise DemoFailure("Bitcoin Core is not on regtest")
        network = _json_command(
            [str(self.env_script), "bitcoin-cli", "getnetworkinfo"]
        )
        if network.get("connections") != 0:
            raise DemoFailure("Bitcoin Core has an external peer")
        runtime_attestation = _json_command(
            [str(self.env_script), "attestation"],
            timeout=600,
        )
        isolation = runtime_attestation.get("runtime_isolation", {})
        source = runtime_attestation.get("harness_source", {})
        signatures = runtime_attestation.get("lnd_release_verification", {})
        if (
            runtime_attestation.get("synthetic_only") is not True
            or not isinstance(isolation, Mapping)
            or isolation.get("verified") is not True
            or isolation.get("external_connectivity") is not False
            or not isinstance(source, Mapping)
            or source.get("git_clean") is not True
            or not isinstance(signatures, Mapping)
            or int(signatures.get("valid_signature_count", 0)) < 5
        ):
            raise DemoFailure("runtime/source provenance attestation failed")
        preflight = {
            "safety_envelope": "PASS",
            "runtime_attestation": runtime_attestation,
            "bitcoin": {
                "chain": bitcoin["chain"],
                "height": bitcoin["blocks"],
                "bestblockhash": bitcoin["bestblockhash"],
                "connections": network["connections"],
            },
            "lnd": {
                node: {
                    "pubkey": info["identity_pubkey"],
                    "block_height": info["block_height"],
                    "synced_to_chain": info["synced_to_chain"],
                    "active_channels": info["num_active_channels"],
                }
                for node, info in infos.items()
            },
            "pftl": {
                "chain_id": snapshot["chain_id"],
                "height": snapshot["finalized_height"],
                "state_root": snapshot["state_root"],
                "validator_count": snapshot["validator_count"],
                "agreeing_validator_count": snapshot["agreeing_validator_count"],
                "asset_id": snapshot["asset_id"],
                "outstanding_supply": snapshot["outstanding_supply"],
                "asset_control_class": "NON_FREEZABLE_TEST",
                "binary": self.devnet.manifest["binary"],
            },
            "scope": {
                "real_btc": False,
                "ce22": False,
                "mainnet": False,
                "external_api": False,
                "gpu": False,
                "stakehub_signer": False,
            },
        }
        self.evidence.write_json("00-preflight.json", preflight)
        for source_name, destination_name in (
            ("binary-gate.json", "pftl/binary-gate.json"),
            ("bootstrap-snapshot.json", "pftl/bootstrap-snapshot.json"),
        ):
            source = self.devnet.root / "evidence" / source_name
            if not source.is_file():
                raise DemoFailure(f"missing PFTL preflight artifact: {source_name}")
            self.evidence.write_json(
                destination_name,
                json.loads(source.read_text(encoding="utf-8")),
            )
        self.evidence.record("preflight_passed", preflight)

    def _new_quote(
        self,
        label: str,
        *,
        direction: str,
        invoice_node: str,
        payer_node: str,
        owner_role: str,
        recipient_role: str,
        amount_msat: int,
        amount_atoms: int,
        cancel_offset: int,
        invoice_expiry_seconds: int = INVOICE_EXPIRY_SECONDS,
        secret: SecretPreimage | None = None,
    ) -> QuoteContext:
        coordinator_knows_secret = direction == "lightning_to_pftl"
        if coordinator_knows_secret:
            secret = secret or SecretPreimage.generate()
            invoice = self.lnd.add_invoice(
                invoice_node,
                secret,
                amount_msat=amount_msat,
                expiry_seconds=invoice_expiry_seconds,
                min_final_cltv_delta=MIN_FINAL_CLTV_DELTA,
                memo=f"synthetic-pftl-{label}",
            )
        else:
            if secret is not None:
                raise DemoFailure(
                    "off-ramp receiver secret must remain inside the receiver LND"
                )
            invoice = self.lnd.add_invoice_generated(
                invoice_node,
                amount_msat=amount_msat,
                expiry_seconds=invoice_expiry_seconds,
                min_final_cltv_delta=MIN_FINAL_CLTV_DELTA,
                memo=f"synthetic-pftl-{label}",
            )
        _, decoded = self.lnd.decode_invoice_response(
            payer_node, invoice.payment_request
        )
        current = self._snapshot()
        cancel_after = int(current["finalized_height"]) + cancel_offset
        condition = encode_condition(bytes.fromhex(invoice.payment_hash))
        plan = self.devnet.plan_create(
            owner_role=owner_role,
            recipient_role=recipient_role,
            amount=amount_atoms,
            condition=condition,
            cancel_after=cancel_after,
        )
        now = int(time.time())
        invoice_expiry = invoice.facts.expiry_unix
        quote = {
            "schema": "postfiat.lightning_submarine_quote.v1",
            "swap_id": os.urandom(32).hex(),
            "quote_expires_unix": min(now + 120, invoice_expiry - 1),
            "direction": direction,
            "payment_hash": invoice.payment_hash,
            "lightning_network": "regtest",
            "invoice": invoice.payment_request,
            "invoice_payee": invoice.facts.payee,
            "invoice_amount_msat": amount_msat,
            "invoice_expiry_unix": invoice_expiry,
            "min_final_cltv_delta": MIN_FINAL_CLTV_DELTA,
            "max_total_cltv_delta": MAX_TOTAL_CLTV_DELTA,
            "pftl_chain_id": plan["chain_id"],
            "pftl_genesis_hash": plan["genesis_hash"],
            "pftl_asset_id": plan["asset_id"],
            "pftl_amount_atoms": amount_atoms,
            "pftl_owner": plan["owner"],
            "pftl_owner_sequence": plan["owner_sequence"],
            "pftl_recipient": plan["recipient"],
            "expected_escrow_id": plan["expected_escrow_id"],
            "condition": plan["condition"],
            "finish_after": plan["finish_after"],
            "cancel_after": plan["cancel_after"],
            "latest_lightning_start_unix": min(now + 300, invoice_expiry - 1),
            "rate_numerator": amount_atoms,
            "rate_denominator": amount_msat,
            "coordinator_fee_atoms": 0,
            "nav_epoch": 0,
            "nav_reserve_packet_hash": "",
            "custody_class": "NON_CUSTODIAL_HASHLOCK",
            "atomicity_class": "CONDITIONAL_HTLC",
            "timeout_clock_class": "OFFCHAIN_CROSS_LEDGER_POLICY",
            "asset_control_class": "NON_FREEZABLE_TEST",
        }
        envelope = sign_quote(quote, self.signer)
        verified = verify_signed_quote(
            envelope, expected_public_key=self.signer_public_key
        )
        if verified != quote:
            raise DemoFailure("signed quote round trip changed a field")

        def signature_check(candidate: Mapping[str, Any]) -> bool:
            try:
                return (
                    verify_signed_quote(
                        envelope, expected_public_key=self.signer_public_key
                    )
                    == candidate
                )
            except Exception:
                return False

        validate_invoice_against_quote(
            quote,
            decoded,
            now_unix=now,
            verify_quote_signature=signature_check,
        )
        self.evidence.write_json(f"quotes/{label}-signed.json", envelope)
        self.evidence.write_json(f"lightning/{label}-decoded-invoice.json", decoded)
        self.evidence.record(
            "signed_quote_validated",
            {
                "label": label,
                "swap_id": quote["swap_id"],
                "payment_hash": quote["payment_hash"],
                "direction": direction,
                "invoice_amount_msat": amount_msat,
                "pftl_amount_atoms": amount_atoms,
                "quote_signature": "VALID",
                "invoice_binding": "VALID",
                "amp": False,
            },
        )
        return QuoteContext(
            label=label,
            payer_node=payer_node,
            invoice_secret_known_to_coordinator=secret,
            signed_quote=envelope,
            quote=quote,
            plan=plan,
            decoded_invoice=decoded,
        )

    def _lock(
        self,
        context: QuoteContext,
        *,
        owner_role: str,
        recipient_role: str,
        coordinator_controls_secret: bool,
    ) -> tuple[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any]]:
        before = self._snapshot(context.escrow_id)
        self.service.admit_quote(
            self.devnet.manifest["roles"]["user"]["address"],
            context.signed_quote,
            expected_public_key=self.signer_public_key,
            coordinator_secret=(
                context.invoice_secret_known_to_coordinator
                if coordinator_controls_secret
                else None
            ),
        )
        effect_key = f"{context.swap_id}:pftl-create"
        self.service.mark_lock_submitted(
            context.swap_id,
            effect_key=effect_key,
            operation={
                "escrow_id": context.escrow_id,
                "owner": context.quote["pftl_owner"],
                "recipient": context.quote["pftl_recipient"],
                "asset_id": context.quote["pftl_asset_id"],
                "amount_atoms": context.quote["pftl_amount_atoms"],
                "condition": context.quote["condition"],
                "cancel_after": context.quote["cancel_after"],
            },
        )
        effect = self.devnet.submit_create(
            owner_role=owner_role,
            recipient_role=recipient_role,
            amount=int(context.quote["pftl_amount_atoms"]),
            condition=str(context.quote["condition"]),
            cancel_after=int(context.quote["cancel_after"]),
            effect_key=effect_key,
            expected_escrow_id=context.escrow_id,
        )
        retry_height = int(self.devnet.statuses()[0]["block_height"])
        retried_effect = self.devnet.submit_create(
            owner_role=owner_role,
            recipient_role=recipient_role,
            amount=int(context.quote["pftl_amount_atoms"]),
            condition=str(context.quote["condition"]),
            cancel_after=int(context.quote["cancel_after"]),
            effect_key=effect_key,
            expected_escrow_id=context.escrow_id,
        )
        if (
            retried_effect != effect
            or int(self.devnet.statuses()[0]["block_height"]) != retry_height
        ):
            raise DemoFailure("PFTL create idempotent retry changed consensus")
        self.journal.record_side_effect_attempt(
            effect_key,
            f"{effect_key}:attempt:1",
            "SUCCEEDED",
            result=effect,
        )
        self.service.mark_lock_final(
            context.swap_id,
            finality_evidence=effect,
        )
        after = self._snapshot(context.escrow_id, effect["tx_id"])
        owner = str(context.quote["pftl_owner"])
        recipient = str(context.quote["pftl_recipient"])
        delta = assert_create_delta(
            _accounting_state(before, owner=owner, recipient=recipient),
            _accounting_state(after, owner=owner, recipient=recipient),
            principal_atoms=int(context.quote["pftl_amount_atoms"]),
        )
        if (
            effect.get("accepted") is not True
            or effect.get("reason") != "accepted"
            or effect.get("agreeing_validator_count") != 6
        ):
            raise DemoFailure(f"{context.label} PFTL create gate failed")
        finality = self._capture_finality(
            effect, label=f"{context.label}-lock"
        )
        artifact = {
            "effect": effect,
            "finality": finality,
            "idempotent_retry": {
                "same_effect": True,
                "no_new_height": True,
            },
            "exact_delta": delta,
            "before": before,
            "after": after,
        }
        self.evidence.write_json(f"pftl/{context.label}-lock.json", artifact)
        self.evidence.record(
            "pftl_lock_final",
            {
                "label": context.label,
                "tx_id": effect["tx_id"],
                "receipt_code": effect["reason"],
                "accepted": effect["accepted"],
                "exact_delta": delta,
                "validator_agreement": "6/6",
                "state_root": effect["state_root"],
            },
        )
        return before, after, effect

    def _validate_final_lock(
        self,
        context: QuoteContext,
        *,
        expect_safe: bool,
    ) -> Mapping[str, Any]:
        snapshot = self._snapshot(context.escrow_id)
        _, current_invoice = self.lnd.decode_invoice_response(
            context.payer_node,
            str(context.quote["invoice"]),
        )

        def signature_check(candidate: Mapping[str, Any]) -> bool:
            try:
                return (
                    verify_signed_quote(
                        context.signed_quote,
                        expected_public_key=self.signer_public_key,
                    )
                    == candidate
                )
            except Exception:
                return False

        validate_invoice_against_quote(
            context.quote,
            current_invoice,
            now_unix=int(time.time()),
            verify_quote_signature=signature_check,
        )
        try:
            view = validate_pftl_lock_views(
                context.quote,
                snapshot["rows"],
                policy=TimelockPolicy(),
            )
        except ValidationError as error:
            if expect_safe:
                raise
            result = {
                "safe_to_pay": False,
                "reason": str(error),
                "mutation": "none-wallet-read-only",
            }
            self.evidence.write_json(
                f"wallet/{context.label}-timelock-refusal.json", result
            )
            return result
        if not expect_safe:
            raise DemoFailure("short refund quote unexpectedly passed timelock policy")
        result = {
            "safe_to_pay": True,
            "available_validators": view.available_validators,
            "finalized_height": view.height,
            "state_root": view.state_root,
            "escrow_id": view.escrow_id,
            "cancel_after": view.cancel_after,
            "recipient_asset_balance": view.recipient_asset_balance,
            "recipient_asset_headroom": view.recipient_asset_headroom,
            "recipient_native_balance": view.recipient_native_balance,
            "finish_minimum_fee": view.finish_minimum_fee,
            "required_pftl_margin_blocks": TimelockPolicy().required_pftl_blocks(),
        }
        self.evidence.write_json(f"wallet/{context.label}-lock-validation.json", result)
        self.evidence.record(
            "wallet_authorized_lightning_start",
            {"label": context.label, **result},
        )
        return result

    def _pay_and_finish(
        self,
        context: QuoteContext,
        *,
        payer_node: str,
        receiver_node: str,
        owner_role: str,
        recipient_role: str,
        coordinator_outgoing: bool,
    ) -> dict[str, Any]:
        # This is the last read-only gate before the irreversible Lightning
        # send. It re-decodes the invoice, revalidates quote time, and reads
        # the finalized escrow independently from all six validators.
        self._validate_final_lock(context, expect_safe=True)
        channels_before = _channel_state(self.lnd)
        payment_effect_key = f"{context.swap_id}:lightning-payment"
        self.service.mark_ln_in_flight(
            context.swap_id,
            payment_evidence={
                "payer": payer_node,
                "payment_hash": context.quote["payment_hash"],
                "max_total_cltv_delta": MAX_TOTAL_CLTV_DELTA,
                "max_parts": 1,
            },
            effect_key=payment_effect_key if coordinator_outgoing else None,
            payment_request=(
                {
                    "payment_hash": context.quote["payment_hash"],
                    "invoice_sha256": hashlib.sha256(
                        str(context.quote["invoice"]).encode()
                    ).hexdigest(),
                    "amount_msat": context.quote["invoice_amount_msat"],
                    "max_total_cltv_delta": MAX_TOTAL_CLTV_DELTA,
                    "max_parts": 1,
                }
                if coordinator_outgoing
                else None
            ),
        )
        bitcoin_height = int(self.lnd.get_info(payer_node)["block_height"])
        payment = self.lnd.pay_invoice(
            payer_node,
            str(context.quote["invoice"]),
            fee_limit_sat=20,
            max_total_cltv_delta=MAX_TOTAL_CLTV_DELTA,
            timeout_seconds=30,
        )
        if payment.payment_preimage is None:
            raise DemoFailure(f"{context.label} payment returned no preimage")
        settlement = assert_lightning_settlement(
            LightningSettlement(
                payment_hash=payment.payment_hash,
                payment_preimage=payment.payment_preimage.reveal_for_protocol(),
                invoice_amount_msat=int(context.quote["invoice_amount_msat"]),
                settled_amount_msat=payment.value_msat,
                fee_msat=payment.fee_msat,
                status=payment.status,
            ),
            expected_hash=str(context.quote["payment_hash"]),
            fee_limit_msat=20_000,
        )
        if not payment.payer_htlc_expiries:
            raise DemoFailure("settled payment has no payer-side HTLC expiry")
        payer_first_hop_expiry = max(payment.payer_htlc_expiries)
        if not (
            bitcoin_height < payer_first_hop_expiry
            and payer_first_hop_expiry - bitcoin_height
            <= MAX_TOTAL_CLTV_DELTA
        ):
            raise DemoFailure("payer first-hop CLTV is outside signed safety limit")
        receiver_invoice = self.lnd.lookup_invoice(
            receiver_node, str(context.quote["payment_hash"])
        )
        if (
            receiver_invoice.get("settled") is not True
            or receiver_invoice.get("state") != "SETTLED"
            or int(receiver_invoice.get("amt_paid_msat", -1))
            != int(context.quote["invoice_amount_msat"])
        ):
            raise DemoFailure("receiver LND does not show exact settled invoice")
        channels_after = _channel_state(self.lnd)
        channel_delta = _assert_channel_delta(
            channels_before,
            channels_after,
            payer=payer_node,
            receiver=receiver_node,
            payment=payment,
        )
        if coordinator_outgoing:
            self.journal.record_side_effect_attempt(
                payment_effect_key,
                f"{payment_effect_key}:attempt:1",
                "SUCCEEDED",
                result={
                    "payment_hash": payment.payment_hash,
                    "status": payment.status,
                    "value_msat": payment.value_msat,
                    "fee_msat": payment.fee_msat,
                    "hash_link_verified": True,
                },
            )

        finish_effect_key = f"{context.swap_id}:pftl-finish"
        journal_settlement = {
            key: value
            for key, value in settlement.items()
            if key != "payment_preimage"
        }
        self.service.mark_ln_settled(
            context.swap_id,
            settlement_evidence={
                **journal_settlement,
                "hash_link_verified": True,
                "receiver_invoice_state": receiver_invoice["state"],
                "payer_first_hop_expiry": payer_first_hop_expiry,
                "payer_bitcoin_height": bitcoin_height,
            },
            learned_secret=payment.payment_preimage if coordinator_outgoing else None,
            effect_key=finish_effect_key,
            finish_operation={
                "escrow_id": context.escrow_id,
                "owner": context.quote["pftl_owner"],
                "recipient": context.quote["pftl_recipient"],
                "payment_hash": context.quote["payment_hash"],
            },
        )
        before_finish = self._snapshot(context.escrow_id)
        fulfillment = encode_fulfillment(payment.payment_preimage)
        effect = self.devnet.submit_finish(
            owner_role=owner_role,
            recipient_role=recipient_role,
            escrow_id=context.escrow_id,
            fulfillment=fulfillment,
            expected_condition=str(context.quote["condition"]),
            effect_key=finish_effect_key,
        )
        retry_height = int(self.devnet.statuses()[0]["block_height"])
        retried_effect = self.devnet.submit_finish(
            owner_role=owner_role,
            recipient_role=recipient_role,
            escrow_id=context.escrow_id,
            fulfillment=fulfillment,
            expected_condition=str(context.quote["condition"]),
            effect_key=finish_effect_key,
        )
        if (
            retried_effect != effect
            or int(self.devnet.statuses()[0]["block_height"]) != retry_height
        ):
            raise DemoFailure("PFTL finish idempotent retry changed consensus")
        self.journal.record_side_effect_attempt(
            finish_effect_key,
            f"{finish_effect_key}:attempt:1",
            "SUCCEEDED",
            result=effect,
        )
        self.service.mark_finish_final(
            context.swap_id, finality_evidence=effect
        )
        finality = self._capture_finality(
            effect, label=f"{context.label}-finish"
        )
        after_finish = self._snapshot(context.escrow_id, effect["tx_id"])
        owner = str(context.quote["pftl_owner"])
        recipient = str(context.quote["pftl_recipient"])
        pftl_delta = assert_finish_delta(
            _accounting_state(before_finish, owner=owner, recipient=recipient),
            _accounting_state(after_finish, owner=owner, recipient=recipient),
            principal_atoms=int(context.quote["pftl_amount_atoms"]),
        )
        terminal_replay = self.devnet.submit_expected_rejection(
            signer_role=recipient_role,
            operation={
                "operation": "escrow_finish",
                "escrow_id": context.escrow_id,
                "owner": owner,
                "recipient": recipient,
                "fulfillment": fulfillment,
            },
            expected_code="escrow_not_open",
            effect_key=f"{context.swap_id}:terminal-finish-replay",
            escrow_id=context.escrow_id,
        )
        terminal_replay_finality = self._capture_finality(
            terminal_replay,
            label=f"{context.label}-terminal-finish-replay",
        )
        terminal = assert_terminal_conditional_atomicity(
            lightning_settled=True,
            pftl_escrow_state="FINISHED",
        )
        result = {
            "lightning": {
                "settlement": settlement,
                "receiver_invoice": receiver_invoice,
                "payer_bitcoin_height": bitcoin_height,
                "payer_first_hop_htlc_expiry": payer_first_hop_expiry,
                "all_payer_htlc_expiries": list(payment.payer_htlc_expiries),
                "channel_delta_sat": channel_delta,
                "before": channels_before,
                "after": channels_after,
                "response": payment.public_response,
            },
            "pftl": {
                "effect": effect,
                "finality": finality,
                "idempotent_retry": {
                    "same_effect": True,
                    "no_new_height": True,
                },
                "exact_delta": pftl_delta,
                "terminal_state_replay": {
                    "effect": terminal_replay,
                    "finality": terminal_replay_finality,
                },
                "before": before_finish,
                "after": after_finish,
            },
            "conditional_atomicity": terminal,
        }
        self.evidence.write_json(f"flows/{context.label}-settlement.json", result)
        self.evidence.record(
            "cross_ledger_terminal",
            {
                "label": context.label,
                "payment_hash": payment.payment_hash,
                "lightning_status": payment.status,
                "lightning_value_msat": payment.value_msat,
                "lightning_fee_msat": payment.fee_msat,
                "payment_preimage": REDACTED,
                "pftl_tx_id": effect["tx_id"],
                "pftl_receipt_code": effect["reason"],
                "pftl_accepted": effect["accepted"],
                "pftl_exact_delta": pftl_delta,
                "validator_agreement": "6/6",
                "conditional_atomicity": terminal,
            },
        )
        self.test_vectors.append(
            {
                "name": context.label,
                "preimage": payment.payment_preimage.protocol_hex(),
                "payment_hash": payment.payment_hash,
                "condition": str(context.quote["condition"]),
                "fulfillment": fulfillment,
            }
        )
        return result

    def flow_a(self) -> QuoteContext:
        context = self._new_quote(
            "flow-a-onramp",
            direction="lightning_to_pftl",
            invoice_node="coordinator",
            payer_node="user",
            owner_role="coordinator",
            recipient_role="user",
            amount_msat=FLOW_A_MSAT,
            amount_atoms=FLOW_A_ATOMS,
            cancel_offset=SAFE_PFTL_CANCEL_OFFSET,
        )
        _, _, create = self._lock(
            context,
            owner_role="coordinator",
            recipient_role="user",
            coordinator_controls_secret=True,
        )
        result = self._pay_and_finish(
            context,
            payer_node="user",
            receiver_node="coordinator",
            owner_role="coordinator",
            recipient_role="user",
            coordinator_outgoing=False,
        )
        duplicate_create = self.devnet.submit_duplicate(
            original_tx_id=create["tx_id"],
            expected_code="bad_sequence",
            effect_key=f"{context.swap_id}:duplicate-create",
            escrow_id=context.escrow_id,
        )
        duplicate_finish = self.devnet.submit_duplicate(
            original_tx_id=result["pftl"]["effect"]["tx_id"],
            expected_code="bad_sequence",
            effect_key=f"{context.swap_id}:duplicate-finish",
            escrow_id=context.escrow_id,
        )
        duplicate_create_finality = self._capture_finality(
            duplicate_create, label="flow-a-duplicate-create"
        )
        duplicate_finish_finality = self._capture_finality(
            duplicate_finish, label="flow-a-duplicate-finish"
        )
        duplicates = {
            "create": {
                "effect": duplicate_create,
                "finality": duplicate_create_finality,
            },
            "finish": {
                "effect": duplicate_finish,
                "finality": duplicate_finish_finality,
            },
        }
        self.evidence.write_json("adversarial/duplicate-create-finish.json", duplicates)
        self.results["flow_a"] = result
        self.results["duplicate_create"] = duplicate_create
        self.results["duplicate_finish"] = duplicate_finish
        return context

    def flow_b(self) -> QuoteContext:
        context = self._new_quote(
            "flow-b-offramp",
            direction="pftl_to_lightning",
            invoice_node="user",
            payer_node="coordinator",
            owner_role="user",
            recipient_role="coordinator",
            amount_msat=FLOW_B_MSAT,
            amount_atoms=FLOW_B_ATOMS,
            cancel_offset=SAFE_PFTL_CANCEL_OFFSET,
        )
        self._lock(
            context,
            owner_role="user",
            recipient_role="coordinator",
            coordinator_controls_secret=False,
        )
        result = self._pay_and_finish(
            context,
            payer_node="coordinator",
            receiver_node="user",
            owner_role="user",
            recipient_role="coordinator",
            coordinator_outgoing=True,
        )
        self.results["flow_b"] = result
        return context

    def refund_and_adversarial(self) -> QuoteContext:
        context = self._new_quote(
            "refund-branch",
            direction="lightning_to_pftl",
            invoice_node="coordinator",
            payer_node="user",
            owner_role="coordinator",
            recipient_role="user",
            amount_msat=REFUND_MSAT,
            amount_atoms=REFUND_ATOMS,
            cancel_offset=SHORT_REFUND_CANCEL_OFFSET,
            invoice_expiry_seconds=REFUND_INVOICE_EXPIRY_SECONDS,
        )
        before, locked, create = self._lock(
            context,
            owner_role="coordinator",
            recipient_role="user",
            coordinator_controls_secret=True,
        )
        refusal = self._validate_final_lock(context, expect_safe=False)
        owner = str(context.quote["pftl_owner"])
        recipient = str(context.quote["pftl_recipient"])
        known_secret = context.invoice_secret_known_to_coordinator
        if known_secret is None:
            raise DemoFailure("refund branch coordinator secret is unavailable")
        wrong_secret = SecretPreimage.generate()
        if payment_hash(wrong_secret).hex() == context.quote["payment_hash"]:
            raise DemoFailure("random wrong preimage collided")
        finish_base = {
            "operation": "escrow_finish",
            "escrow_id": context.escrow_id,
            "owner": owner,
            "recipient": recipient,
        }
        wrong = self.devnet.submit_expected_rejection(
            signer_role="user",
            operation={
                **finish_base,
                "fulfillment": encode_fulfillment(wrong_secret),
            },
            expected_code="escrow_condition_unsatisfied",
            effect_key=f"{context.swap_id}:wrong-preimage",
            escrow_id=context.escrow_id,
        )
        wrong_finality = self._capture_finality(
            wrong, label="refund-wrong-hashlock"
        )
        malformed = self.devnet.submit_expected_rejection(
            signer_role="user",
            operation={
                **finish_base,
                "fulfillment": encode_fulfillment(known_secret).upper(),
            },
            expected_code="invalid_escrow_fulfillment",
            effect_key=f"{context.swap_id}:malformed-fulfillment",
            escrow_id=context.escrow_id,
        )
        malformed_finality = self._capture_finality(
            malformed, label="refund-malformed-claim"
        )
        early_cancel = self.devnet.submit_expected_rejection(
            signer_role="coordinator",
            operation={
                "operation": "escrow_cancel",
                "escrow_id": context.escrow_id,
                "owner": owner,
            },
            expected_code="escrow_cancel_too_early",
            effect_key=f"{context.swap_id}:early-cancel",
            escrow_id=context.escrow_id,
        )
        early_finality = self._capture_finality(
            early_cancel, label="refund-early-cancel"
        )
        while int(self.devnet.statuses()[0]["block_height"]) < int(
            context.quote["cancel_after"]
        ) - 1:
            height = int(self.devnet.statuses()[0]["block_height"]) + 1
            advance = self.devnet.advance_height(
                effect_key=f"{context.swap_id}:advance:{height}"
            )
            self._capture_finality(
                advance, label=f"refund-height-{height}"
            )
        late = self.devnet.submit_expected_rejection(
            signer_role="user",
            operation={
                **finish_base,
                "fulfillment": encode_fulfillment(known_secret),
            },
            expected_code="escrow_finish_expired",
            effect_key=f"{context.swap_id}:late-finish",
            escrow_id=context.escrow_id,
        )
        late_finality = self._capture_finality(
            late, label="refund-late-finish"
        )
        expiry_deadline = int(context.quote["invoice_expiry_unix"]) + 15
        while True:
            invoice = self.lnd.lookup_invoice(
                "coordinator", str(context.quote["payment_hash"])
            )
            if invoice.get("settled") is True:
                raise DemoFailure("refund invoice unexpectedly settled")
            if invoice.get("state") == "CANCELED":
                break
            if int(time.time()) > expiry_deadline:
                raise DemoFailure(
                    "refund invoice did not expire before the PFTL cancel"
                )
            time.sleep(0.5)
        self.service.mark_refund_eligible(
            context.swap_id,
            reason_evidence={
                "lightning_settled": False,
                "invoice_state": invoice.get("state"),
                "invoice_expiry_unix": context.quote["invoice_expiry_unix"],
                "invoice_expired_before_cancel": True,
                "pftl_height": self.devnet.statuses()[0]["block_height"],
                "cancel_after": context.quote["cancel_after"],
            },
            effect_key=f"{context.swap_id}:pftl-cancel",
            cancel_operation={
                "escrow_id": context.escrow_id,
                "owner": owner,
            },
        )
        before_cancel = self._snapshot(context.escrow_id)
        cancel = self.devnet.submit_cancel(
            owner_role="coordinator",
            escrow_id=context.escrow_id,
            effect_key=f"{context.swap_id}:pftl-cancel",
        )
        self.journal.record_side_effect_attempt(
            f"{context.swap_id}:pftl-cancel",
            f"{context.swap_id}:pftl-cancel:attempt:1",
            "SUCCEEDED",
            result=cancel,
        )
        self.service.mark_cancel_final(
            context.swap_id, finality_evidence=cancel
        )
        cancel_finality = self._capture_finality(
            cancel, label="refund-cancel"
        )
        after_cancel = self._snapshot(context.escrow_id, cancel["tx_id"])
        cancel_delta = assert_cancel_delta(
            _accounting_state(before_cancel, owner=owner, recipient=recipient),
            _accounting_state(after_cancel, owner=owner, recipient=recipient),
            principal_atoms=REFUND_ATOMS,
        )
        if _issued_balance(before, owner) != _issued_balance(after_cancel, owner):
            raise DemoFailure("refund branch did not restore the initial owner balance")
        terminal = assert_terminal_conditional_atomicity(
            lightning_settled=False,
            pftl_escrow_state="CANCELED",
        )
        terminal_cancel_replay = self.devnet.submit_expected_rejection(
            signer_role="coordinator",
            operation={
                "operation": "escrow_cancel",
                "escrow_id": context.escrow_id,
                "owner": owner,
            },
            expected_code="escrow_not_open",
            effect_key=f"{context.swap_id}:terminal-cancel-replay",
            escrow_id=context.escrow_id,
        )
        terminal_cancel_replay_finality = self._capture_finality(
            terminal_cancel_replay,
            label="refund-terminal-cancel-replay",
        )
        duplicate_cancel = self.devnet.submit_duplicate(
            original_tx_id=cancel["tx_id"],
            expected_code="bad_sequence",
            effect_key=f"{context.swap_id}:duplicate-cancel",
            escrow_id=context.escrow_id,
        )
        duplicate_cancel_finality = self._capture_finality(
            duplicate_cancel, label="refund-duplicate-cancel"
        )
        result = {
            "wallet_timelock_refusal": refusal,
            "invoice": invoice,
            "wrong_hashlock_rejection": {
                "effect": wrong,
                "finality": wrong_finality,
            },
            "malformed_claim_rejection": {
                "effect": malformed,
                "finality": malformed_finality,
            },
            "early_cancel": {
                "effect": early_cancel,
                "finality": early_finality,
            },
            "late_finish": {
                "effect": late,
                "finality": late_finality,
            },
            "cancel": {
                "effect": cancel,
                "finality": cancel_finality,
            },
            "duplicate_cancel": {
                "effect": duplicate_cancel,
                "finality": duplicate_cancel_finality,
            },
            "terminal_cancel_replay": {
                "effect": terminal_cancel_replay,
                "finality": terminal_cancel_replay_finality,
            },
            "cancel_exact_delta": cancel_delta,
            "initial": before,
            "locked": locked,
            "before_cancel": before_cancel,
            "after_cancel": after_cancel,
            "conditional_atomicity": terminal,
        }
        self.evidence.write_json("flows/refund-and-adversarial.json", result)
        self.evidence.record(
            "refund_terminal",
            {
                "swap_id": context.swap_id,
                "lightning_settled": False,
                "pftl_receipt_code": cancel["reason"],
                "pftl_accepted": cancel["accepted"],
                "exact_delta": cancel_delta,
                "validator_agreement": "6/6",
                "conditional_atomicity": terminal,
            },
        )
        self.results["refund"] = result
        return context

    def lightning_adversarial(self) -> None:
        amp_request = self.lnd.add_amp_invoice_for_rejection_test(
            "coordinator", amount_msat=3_000
        )
        try:
            self.lnd.decode_invoice("user", amp_request)
        except LightningTransportError as error:
            amp = {
                "rejected": True,
                "reason": str(error),
                "payment_attempted": False,
            }
        else:
            raise DemoFailure("AMP invoice unexpectedly passed ordinary-invoice decode")

        route_secret = SecretPreimage.generate()
        route_invoice = self.lnd.add_invoice(
            "coordinator",
            route_secret,
            amount_msat=ROUTE_FAILURE_MSAT,
            expiry_seconds=INVOICE_EXPIRY_SECONDS,
            min_final_cltv_delta=MIN_FINAL_CLTV_DELTA,
            memo="synthetic-no-route-test",
        )
        before = _channel_state(self.lnd)
        route_error: str | None = None
        route_payment: PaymentResult | None = None
        try:
            route_payment = self.lnd.pay_invoice(
                "user",
                route_invoice.payment_request,
                fee_limit_sat=20,
                max_total_cltv_delta=MAX_TOTAL_CLTV_DELTA,
                timeout_seconds=5,
            )
        except LightningTransportError as error:
            route_error = str(error)
        after = _channel_state(self.lnd)
        if before != after:
            raise DemoFailure("failed Lightning route changed channel balances")
        if route_payment is not None:
            if (
                route_payment.status == "SUCCEEDED"
                or route_payment.payment_preimage is not None
            ):
                raise DemoFailure("oversized route unexpectedly settled")
            route_public: Mapping[str, Any] = {
                "status": route_payment.status,
                "failure_reason": route_payment.failure_reason,
                "payment_hash": route_payment.payment_hash,
                "payment_preimage": REDACTED,
                "response": route_payment.public_response,
            }
        else:
            route_public = {
                "status": "FAILED_AT_GRPC",
                "reason": route_error,
                "payment_preimage": REDACTED,
            }
        route = {
            "invoice_amount_msat": ROUTE_FAILURE_MSAT,
            "result": route_public,
            "channels_before": before,
            "channels_after": after,
            "mutation_free": True,
        }
        self.evidence.write_json(
            "adversarial/lightning-amp-and-route-failure.json",
            {"amp": amp, "route_failure": route},
        )
        self.evidence.record(
            "lightning_adversarial_passed",
            {
                "amp_rejected_before_payment": True,
                "route_failure_status": route_public["status"],
                "route_failure_mutation_free": True,
                "payment_preimage": REDACTED,
            },
        )
        self.results["lightning_adversarial"] = {
            "amp": amp,
            "route_failure": route,
        }

    def live_crash_recovery(self) -> QuoteContext:
        """Crash every edge plus the outgoing-LN uncertainty window."""

        context = self._new_quote(
            "live-crash-recovery",
            direction="pftl_to_lightning",
            invoice_node="user",
            payer_node="coordinator",
            owner_role="user",
            recipient_role="coordinator",
            amount_msat=CRASH_RECOVERY_MSAT,
            amount_atoms=CRASH_RECOVERY_ATOMS,
            cancel_offset=SAFE_PFTL_CANCEL_OFFSET,
        )
        if context.invoice_secret_known_to_coordinator is not None:
            raise DemoFailure("off-ramp receiver secret escaped receiver LND")
        root = (
            self.devnet.root
            / "private"
            / "demo-runs"
            / f"live-crash-{context.swap_id}"
        )
        root.mkdir(parents=True, exist_ok=False)
        os.chmod(root, 0o700)
        database = root / "coordinator.sqlite3"
        envelope_path = root / "signed-quote.json"
        _atomic_private_bytes(
            envelope_path,
            (
                json.dumps(
                    context.signed_quote,
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n"
            ).encode("ascii"),
        )
        limits = ExposureLimits(100_000_000, 250_000_000)
        principal = self.devnet.manifest["roles"]["user"]["address"]
        steps: list[dict[str, Any]] = []
        ordinal = 0

        def crash(
            action: str,
            request: Mapping[str, Any],
            *,
            expected_state: str,
        ) -> dict[str, Any]:
            nonlocal ordinal
            ordinal += 1
            step = crash_service_transition(
                root=root,
                database=database,
                envelope_path=envelope_path,
                principal=principal,
                action=action,
                request=request,
                limits=limits,
                ordinal=ordinal,
            )
            with CoordinatorJournal(database, limits) as recovered:
                swap = recovered.get_swap(context.swap_id)
                if swap["state"] != expected_state:
                    raise DemoFailure(
                        f"crash recovery state mismatch after {action}: {swap['state']}"
                    )
                plan = CoordinatorService(recovered).recovery_plan()
                step.update(
                    {
                        "recovered_state": swap["state"],
                        "sqlite_quick_check": "ok",
                        "recovery_action": (
                            plan[0].action if len(plan) == 1 else "terminal"
                        ),
                        "pending_effect_keys": [
                            row["effect_key"]
                            for row in recovered.pending_side_effects()
                        ],
                    }
                )
            steps.append(step)
            return step

        crash(
            "ADMIT",
            {},
            expected_state="QUOTED",
        )
        lock_effect_key = f"{context.swap_id}:crash-pftl-create"
        lock_operation = {
            "escrow_id": context.escrow_id,
            "owner": context.quote["pftl_owner"],
            "recipient": context.quote["pftl_recipient"],
            "asset_id": context.quote["pftl_asset_id"],
            "amount_atoms": context.quote["pftl_amount_atoms"],
            "condition": context.quote["condition"],
            "cancel_after": context.quote["cancel_after"],
        }
        crash(
            "PFTL_LOCK_SUBMITTED",
            {
                "effect_key": lock_effect_key,
                "operation": lock_operation,
            },
            expected_state="PFTL_LOCK_SUBMITTED",
        )
        before_lock = self._snapshot(context.escrow_id)
        lock_effect = self.devnet.submit_create(
            owner_role="user",
            recipient_role="coordinator",
            amount=CRASH_RECOVERY_ATOMS,
            condition=str(context.quote["condition"]),
            cancel_after=int(context.quote["cancel_after"]),
            effect_key=lock_effect_key,
            expected_escrow_id=context.escrow_id,
        )
        lock_finality = self._capture_finality(
            lock_effect, label="live-crash-lock"
        )
        with CoordinatorJournal(database, limits) as recovered:
            recovered.record_side_effect_attempt(
                lock_effect_key,
                f"{lock_effect_key}:attempt:1",
                "SUCCEEDED",
                result=lock_effect,
            )
        after_lock = self._snapshot(context.escrow_id, lock_effect["tx_id"])
        owner = str(context.quote["pftl_owner"])
        recipient = str(context.quote["pftl_recipient"])
        lock_delta = assert_create_delta(
            _accounting_state(before_lock, owner=owner, recipient=recipient),
            _accounting_state(after_lock, owner=owner, recipient=recipient),
            principal_atoms=CRASH_RECOVERY_ATOMS,
        )
        crash(
            "PFTL_LOCK_FINAL",
            {"evidence": lock_effect},
            expected_state="PFTL_LOCK_FINAL",
        )
        self._validate_final_lock(context, expect_safe=True)
        payment_effect_key = f"{context.swap_id}:crash-lightning-payment"
        crash(
            "LN_IN_FLIGHT",
            {
                "evidence": {
                    "payer": "coordinator",
                    "payment_hash": context.quote["payment_hash"],
                    "max_total_cltv_delta": MAX_TOTAL_CLTV_DELTA,
                    "max_parts": 1,
                },
                "effect_key": payment_effect_key,
                "payment_request": {
                    "payment_hash": context.quote["payment_hash"],
                    "amount_msat": CRASH_RECOVERY_MSAT,
                    "max_total_cltv_delta": MAX_TOTAL_CLTV_DELTA,
                    "max_parts": 1,
                },
            },
            expected_state="LN_IN_FLIGHT",
        )

        channels_before = _channel_state(self.lnd)
        bitcoin_height = int(self.lnd.get_info("coordinator")["block_height"])
        payment_crash = crash_after_outgoing_lightning_payment(
            root=root,
            env_script=self.env_script,
            node="coordinator",
            payment_request=str(context.quote["invoice"]),
            payment_hash=str(context.quote["payment_hash"]),
            fee_limit_sat=20,
            max_total_cltv_delta=MAX_TOTAL_CLTV_DELTA,
            timeout_seconds=30,
        )
        with CoordinatorJournal(database, limits) as recovered:
            if recovered.get_swap(context.swap_id)["state"] != "LN_IN_FLIGHT":
                raise DemoFailure("outgoing payment crash changed journal state")
            pending = {
                row["effect_key"]: row
                for row in recovered.pending_side_effects()
            }
            if payment_effect_key not in pending:
                raise DemoFailure("outgoing payment intent was not durable")
        payment = self.lnd.track_payment(
            "coordinator",
            str(context.quote["payment_hash"]),
            timeout_seconds=30,
        )
        if payment.payment_preimage is None:
            raise DemoFailure("payer-side recovery returned no preimage")
        settlement = assert_lightning_settlement(
            LightningSettlement(
                payment_hash=payment.payment_hash,
                payment_preimage=payment.payment_preimage.reveal_for_protocol(),
                invoice_amount_msat=CRASH_RECOVERY_MSAT,
                settled_amount_msat=payment.value_msat,
                fee_msat=payment.fee_msat,
                status=payment.status,
            ),
            expected_hash=str(context.quote["payment_hash"]),
            fee_limit_msat=20_000,
        )
        if not payment.payer_htlc_expiries:
            raise DemoFailure("live crash payment has no payer HTLC expiry")
        payer_expiry = max(payment.payer_htlc_expiries)
        if not (
            bitcoin_height < payer_expiry
            and payer_expiry - bitcoin_height <= MAX_TOTAL_CLTV_DELTA
        ):
            raise DemoFailure("live crash payment CLTV is outside the signed limit")
        receiver_invoice = self.lnd.lookup_invoice(
            "user", str(context.quote["payment_hash"])
        )
        if (
            receiver_invoice.get("state") != "SETTLED"
            or int(receiver_invoice.get("amt_paid_msat", -1))
            != CRASH_RECOVERY_MSAT
        ):
            raise DemoFailure("live crash receiver invoice did not settle exactly")
        channels_after = _channel_state(self.lnd)
        channel_delta = _assert_channel_delta(
            channels_before,
            channels_after,
            payer="coordinator",
            receiver="user",
            payment=payment,
        )
        with CoordinatorJournal(database, limits) as recovered:
            recovered.record_side_effect_attempt(
                payment_effect_key,
                f"{payment_effect_key}:reconcile-by-hash",
                "SUCCEEDED",
                result={
                    **{
                        key: value
                        for key, value in settlement.items()
                        if key != "payment_preimage"
                    },
                    "reconciled_by_payment_hash": True,
                },
            )

        finish_effect_key = f"{context.swap_id}:crash-pftl-finish"
        journal_settlement = {
            key: value
            for key, value in settlement.items()
            if key != "payment_preimage"
        }
        crash(
            "LN_SETTLED",
            {
                "evidence": {
                    **journal_settlement,
                    "hash_link_verified": True,
                    "receiver_invoice_state": receiver_invoice["state"],
                    "payer_htlc_expiry": payer_expiry,
                    "payer_bitcoin_height": bitcoin_height,
                    "outgoing_effect_reconciled_by_payment_hash": True,
                },
                "learned_secret_hex": payment.payment_preimage.protocol_hex(),
                "effect_key": finish_effect_key,
                "finish_operation": {
                    "escrow_id": context.escrow_id,
                    "owner": owner,
                    "recipient": recipient,
                    "payment_hash": context.quote["payment_hash"],
                },
            },
            expected_state="LN_SETTLED",
        )
        with CoordinatorJournal(database, limits) as recovered:
            durable_secret = recovered.load_secret(
                context.swap_id, "invoice_preimage"
            )
        if (
            durable_secret.reveal_for_protocol()
            != payment.payment_preimage.reveal_for_protocol()
        ):
            raise DemoFailure("recovered durable invoice secret differs from LND")
        before_finish = self._snapshot(context.escrow_id)
        finish_effect = self.devnet.submit_finish(
            owner_role="user",
            recipient_role="coordinator",
            escrow_id=context.escrow_id,
            fulfillment=encode_fulfillment(durable_secret),
            expected_condition=str(context.quote["condition"]),
            effect_key=finish_effect_key,
        )
        finish_finality = self._capture_finality(
            finish_effect, label="live-crash-finish"
        )
        with CoordinatorJournal(database, limits) as recovered:
            recovered.record_side_effect_attempt(
                finish_effect_key,
                f"{finish_effect_key}:attempt:1",
                "SUCCEEDED",
                result=finish_effect,
            )
        after_finish = self._snapshot(context.escrow_id, finish_effect["tx_id"])
        finish_delta = assert_finish_delta(
            _accounting_state(before_finish, owner=owner, recipient=recipient),
            _accounting_state(after_finish, owner=owner, recipient=recipient),
            principal_atoms=CRASH_RECOVERY_ATOMS,
        )
        crash(
            "PFTL_FINISH_FINAL",
            {"evidence": finish_effect},
            expected_state="PFTL_FINISH_FINAL",
        )
        with CoordinatorJournal(database, limits) as recovered:
            if recovered.pending_side_effects():
                raise DemoFailure("live crash journal has a pending side effect")
            if recovered.exposure() != {"active_atoms": 0, "active_swaps": 0}:
                raise DemoFailure("live crash journal retained exposure")
            audit = recovered.export_public_audit()

        terminal = assert_terminal_conditional_atomicity(
            lightning_settled=True,
            pftl_escrow_state="FINISHED",
        )
        result = {
            "transition_crashes": steps,
            "actual_pftl_lock": {
                "effect": lock_effect,
                "finality": lock_finality,
                "exact_delta": lock_delta,
                "before": before_lock,
                "after": after_lock,
            },
            "actual_lightning_settlement": {
                "payment_process_crash": payment_crash,
                "reconciliation": {
                    "method": "payer_lnd_trackpayment_by_hash",
                    "payment_hash": payment.payment_hash,
                    "journal_was_ln_in_flight": True,
                    "pending_effect_marked_after_observation": True,
                },
                "settlement": settlement,
                "receiver_invoice": receiver_invoice,
                "payer_bitcoin_height": bitcoin_height,
                "payer_htlc_expiries": list(payment.payer_htlc_expiries),
                "channel_delta_sat": channel_delta,
                "before": channels_before,
                "after": channels_after,
                "response": payment.public_response,
            },
            "actual_pftl_finish": {
                "effect": finish_effect,
                "finality": finish_finality,
                "exact_delta": finish_delta,
                "before": before_finish,
                "after": after_finish,
            },
            "recovered_public_audit": audit,
            "conditional_atomicity": terminal,
        }
        self.evidence.write_json("chaos/live-coordinator-crash-recovery.json", result)
        self.evidence.record(
            "live_crash_recovery_passed",
            {
                "transition_crash_count": len(steps),
                "outgoing_payment_process_crash": True,
                "outgoing_payment_reconciled_by_hash": True,
                "real_lightning_payment_hash": payment.payment_hash,
                "real_pftl_create_tx": lock_effect["tx_id"],
                "real_pftl_finish_tx": finish_effect["tx_id"],
                "validator_agreement": "6/6",
                "conditional_atomicity": terminal,
            },
        )
        self.test_vectors.append(
            {
                "name": context.label,
                "preimage": payment.payment_preimage.protocol_hex(),
                "payment_hash": payment.payment_hash,
                "condition": str(context.quote["condition"]),
                "fulfillment": encode_fulfillment(payment.payment_preimage),
            }
        )
        self.results["live_crash_recovery"] = result
        return context

    def chaos(self) -> None:
        outage = self.devnet.advance_height(
            effect_key=f"chaos-one-validator-down-{int(time.time_ns())}",
            one_validator_down=True,
        )
        outage_finality = self._capture_finality(
            outage, label="chaos-one-validator-down"
        )
        if (
            outage.get("vote_count") != 5
            or len(outage.get("post_recovery_statuses", [])) != 6
        ):
            raise DemoFailure("one-validator-down recovery evidence is incomplete")
        restart = self.devnet.restart_rpc_proof(
            effect_key=f"chaos-rpc-crash-{int(time.time_ns())}"
        )
        if (
            restart.get("verified") is not True
            or restart.get("agreeing_validator_count") != 6
        ):
            raise DemoFailure("PFTL RPC crash/restart proof failed")
        crash_root = (
            self.devnet.root
            / "private"
            / "demo-runs"
            / f"coordinator-crash-matrix-{int(time.time_ns())}"
        )
        # The preceding live consensus/refund cases intentionally take longer
        # than the coordinator's short quote-admission window.  Generate
        # dedicated, still-bounded quotes immediately before the journal-only
        # crash matrix instead of reusing already-consumed or expired flow
        # quotes.  These quotes are never submitted to PFTL or paid on LN.
        crash_happy = self._new_quote(
            "chaos-journal-happy",
            direction="pftl_to_lightning",
            invoice_node="user",
            payer_node="coordinator",
            owner_role="user",
            recipient_role="coordinator",
            amount_msat=REFUND_MSAT,
            amount_atoms=REFUND_ATOMS,
            cancel_offset=SAFE_PFTL_CANCEL_OFFSET,
        )
        crash_refund = self._new_quote(
            "chaos-journal-refund",
            direction="lightning_to_pftl",
            invoice_node="coordinator",
            payer_node="user",
            owner_role="coordinator",
            recipient_role="user",
            amount_msat=REFUND_MSAT,
            amount_atoms=REFUND_ATOMS,
            cancel_offset=SAFE_PFTL_CANCEL_OFFSET,
        )
        crash = run_crash_matrix(
            crash_root,
            happy_signed_quote=crash_happy.signed_quote,
            refund_signed_quote=crash_refund.signed_quote,
            limits=ExposureLimits(100_000_000, 250_000_000),
        )
        public_crash = {
            "schema": crash["schema"],
            "transition_crash_count": crash["transition_crash_count"],
            "result": crash["result"],
            "happy": {
                "terminal_state": crash["happy"]["terminal_state"],
                "steps": crash["happy"]["steps"],
            },
            "refund": {
                "terminal_state": crash["refund"]["terminal_state"],
                "steps": crash["refund"]["steps"],
            },
        }
        self.evidence.write_json(
            "chaos/validator-and-process-recovery.json",
            {
                "one_validator_down": outage,
                "one_validator_down_finality": outage_finality,
                "pftl_rpc_restart": restart,
                "coordinator_process_crash_matrix": public_crash,
            },
        )
        self.evidence.record(
            "chaos_passed",
            {
                "one_validator_down_votes": outage["vote_count"],
                "post_catchup_validator_agreement": "6/6",
                "pftl_rpc_restart": "PASS",
                "coordinator_unclean_transition_crashes": crash[
                    "transition_crash_count"
                ],
            },
        )
        self.results["chaos"] = {
            "outage": outage,
            "restart": restart,
            "crash": public_crash,
        }

    def finalize(self) -> Path:
        audit = self.journal.export_public_audit()
        if audit["exposure"] != {"active_atoms": 0, "active_swaps": 0}:
            raise DemoFailure(f"coordinator exposure did not return to zero: {audit['exposure']}")
        if self.journal.pending_side_effects():
            raise DemoFailure("coordinator has pending side effects at terminal")
        self.evidence.write_json("coordinator/public-audit.json", audit)
        self.evidence.write_test_vectors(self.test_vectors)
        final_pftl = self._snapshot()
        if (
            self.initial_pftl_supply is None
            or int(final_pftl["outstanding_supply"])
            != self.initial_pftl_supply
            or int(final_pftl["open_escrow_total"]) != 0
            or final_pftl["supply_conservation_verified"] is not True
        ):
            raise DemoFailure("terminal PFTL issued-asset conservation failed")
        final_user_atoms = _issued_balance(
            final_pftl, self.devnet.manifest["roles"]["user"]["address"]
        )
        final_coordinator_atoms = _issued_balance(
            final_pftl,
            self.devnet.manifest["roles"]["coordinator"]["address"],
        )
        expected_user_delta = (
            FLOW_A_ATOMS - FLOW_B_ATOMS - CRASH_RECOVERY_ATOMS
        )
        if (
            self.initial_user_atoms is None
            or self.initial_coordinator_atoms is None
            or final_user_atoms - self.initial_user_atoms != expected_user_delta
            or final_coordinator_atoms - self.initial_coordinator_atoms
            != -expected_user_delta
        ):
            raise DemoFailure("terminal participant asset deltas are not exact")
        lightning_user_principal_delta_msat = (
            -FLOW_A_MSAT + FLOW_B_MSAT + CRASH_RECOVERY_MSAT
        )
        if expected_user_delta + lightning_user_principal_delta_msat != 0:
            raise DemoFailure("signed one-to-one quote reconciliation failed")
        final_channels = _channel_state(self.lnd)
        summary = {
            "result": "PASS",
            "claim": (
                "non-custodial, conditionally-atomic synthetic settlement under "
                "stated chain, timelock, liveness, participant-availability, "
                "and non-freezable test-asset assumptions"
            ),
            "not_claimed": [
                "coordinator-free",
                "always available",
                "private",
                "unconditionally trustless",
                "production ready",
            ],
            "flows": {
                "onramp": "BOTH_SETTLED",
                "offramp": "BOTH_SETTLED",
                "crash_recovery": "BOTH_SETTLED",
                "refund": "NEITHER_SETTLED",
            },
            "adversarial": {
                "wrong_hashlock": "REJECTED_MUTATION_FREE",
                "malformed_claim": "REJECTED_MUTATION_FREE",
                "late_finish": "REJECTED_MUTATION_FREE",
                "early_cancel": "REJECTED_MUTATION_FREE",
                "duplicate_create": "REJECTED_MUTATION_FREE",
                "duplicate_finish": (
                    "STALE_SEQUENCE_AND_TERMINAL_STATE_REJECTED_MUTATION_FREE"
                ),
                "duplicate_cancel": (
                    "STALE_SEQUENCE_AND_TERMINAL_STATE_REJECTED_MUTATION_FREE"
                ),
                "amp": "REJECTED_BEFORE_PAYMENT",
                "route_failure": "NO_VALUE_DELTA",
                "crash_every_transition": (
                    "OUTGOING_LIGHTNING_EFFECT_RECONCILED_BY_HASH"
                ),
                "one_validator_down": "5_VOTES_THEN_6_OF_6_CATCHUP",
            },
            "pftl": {
                "finalized_height": final_pftl["finalized_height"],
                "state_root": final_pftl["state_root"],
                "agreeing_validator_count": final_pftl[
                    "agreeing_validator_count"
                ],
                "validator_count": final_pftl["validator_count"],
                "outstanding_supply": final_pftl["outstanding_supply"],
                "open_escrow_total": final_pftl["open_escrow_total"],
                "binary_sha256": self.devnet.manifest["binary"]["sha256"],
                "binary_git_revision": self.devnet.manifest["binary"][
                    "git_revision"
                ],
                "participant_exact_deltas": {
                    "user_atoms": expected_user_delta,
                    "coordinator_atoms": -expected_user_delta,
                },
            },
            "lightning": {
                "network": "regtest",
                "nodes": 3,
                "channels": sum(
                    len(final_channels[node]["channels"])
                    for node in ("user", "coordinator")
                ),
                "direct_transport": "LND gRPC via pinned official lncli",
                "amp": False,
            },
            "coordinator": {
                "journal": "SQLite WAL + synchronous FULL",
                "active_exposure_atoms": 0,
                "pending_side_effects": 0,
            },
            "signed_quote_principal_reconciliation": {
                "user_pftl_delta_atoms": expected_user_delta,
                "user_lightning_delta_msat_excluding_fees": (
                    lightning_user_principal_delta_msat
                ),
                "sum_at_signed_one_atom_per_msat_rate": 0,
            },
        }
        self.evidence.write_json("99-terminal-summary.json", summary)
        manifest = self.evidence.finalize(summary)
        verify_bundle(self.evidence.root)
        return manifest

    def run(self) -> Path:
        self.preflight()
        self.flow_a()
        self.flow_b()
        self.live_crash_recovery()
        self.refund_and_adversarial()
        self.lightning_adversarial()
        self.chaos()
        return self.finalize()


def _prepare(
    *,
    env_script: Path,
    pftl_root: Path,
    node_binary: Path,
    node_revision: str,
    node_sha256: str | None,
    wallet_sdk_sha256: str | None,
) -> PftlDevnet:
    completed = subprocess.run(
        [str(env_script), "channels"],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=600,
    )
    if completed.returncode != 0:
        raise DemoFailure(f"Lightning environment setup failed: {completed.stderr}")
    if (pftl_root / "manifest.json").exists():
        devnet = PftlDevnet(pftl_root)
        observed_revision = str(
            devnet.manifest["binary"]["git_revision"]
        ).lower()
        expected_revision = node_revision.strip().lower()
        if not (
            expected_revision.startswith(observed_revision)
            or observed_revision.startswith(expected_revision)
        ):
            raise DemoFailure("existing PFTL devnet uses a different binary revision")
        if (
            node_sha256 is not None
            and devnet.manifest["binary"]["sha256"] != node_sha256.lower()
        ):
            raise DemoFailure("existing PFTL devnet uses a different node SHA-256")
        if (
            wallet_sdk_sha256 is not None
            and devnet.manifest["binary"]["wallet_sdk_sha256"]
            != wallet_sdk_sha256.lower()
        ):
            raise DemoFailure("existing PFTL devnet uses a different SDK SHA-256")
        return devnet
    return PftlDevnet.initialize(
        pftl_root,
        binary=node_binary,
        expected_revision=node_revision,
        expected_binary_sha256=node_sha256,
        expected_wallet_sdk_sha256=wallet_sdk_sha256,
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("prepare", "run", "all"):
        command = subparsers.add_parser(name)
        command.add_argument(
            "--env-script",
            type=Path,
            default=Path("scripts/lightning-navcoin-regtest-env"),
        )
        command.add_argument("--pftl-root", required=True, type=Path)
        command.add_argument(
            "--node-binary",
            type=Path,
            default=(
                Path(os.environ["POSTFIAT_NODE_BIN"])
                if os.environ.get("POSTFIAT_NODE_BIN")
                else None
            ),
        )
        command.add_argument(
            "--node-revision",
            default=os.environ.get("POSTFIAT_NODE_GIT_REV"),
        )
        command.add_argument(
            "--node-sha256",
            default=os.environ.get("POSTFIAT_NODE_SHA256"),
        )
        command.add_argument(
            "--wallet-sdk-sha256",
            default=os.environ.get("POSTFIAT_RPC_SDK_SHA256"),
        )
        if name in {"run", "all"}:
            command.add_argument("--evidence-dir", required=True, type=Path)
            command.add_argument("--run-id")
    verify = subparsers.add_parser("verify-evidence")
    verify.add_argument("evidence_dir", type=Path)
    verify.add_argument("--expected-manifest-sha256")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = build_parser().parse_args(argv)
    try:
        if arguments.command == "verify-evidence":
            manifest_path = arguments.evidence_dir.resolve() / "manifest.json"
            manifest = verify_bundle(arguments.evidence_dir)
            observed_manifest_sha256 = sha256_file(manifest_path)
            if (
                arguments.expected_manifest_sha256 is not None
                and observed_manifest_sha256
                != arguments.expected_manifest_sha256.lower()
            ):
                raise DemoFailure("evidence manifest SHA-256 mismatch")
            print(
                json.dumps(
                    {
                        "manifest_sha256": observed_manifest_sha256,
                        "manifest": manifest,
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
            return 0
        env_script = arguments.env_script.resolve()
        if arguments.command in {"prepare", "all"}:
            if arguments.node_binary is None or not arguments.node_revision:
                raise DemoFailure(
                    "--node-binary/POSTFIAT_NODE_BIN and "
                    "--node-revision/POSTFIAT_NODE_GIT_REV are required"
                )
            devnet = _prepare(
                env_script=env_script,
                pftl_root=arguments.pftl_root,
                node_binary=arguments.node_binary,
                node_revision=arguments.node_revision,
                node_sha256=arguments.node_sha256,
                wallet_sdk_sha256=arguments.wallet_sdk_sha256,
            )
            if arguments.command == "prepare":
                print(json.dumps(devnet.manifest, indent=2, sort_keys=True))
                return 0
        else:
            devnet = PftlDevnet(arguments.pftl_root)
        devnet.start_rpc()
        run_id = arguments.run_id or datetime.now(timezone.utc).strftime(
            "%Y%m%dT%H%M%SZ"
        )
        demo = SyntheticDemo(
            devnet=devnet,
            env_script=env_script,
            evidence_dir=arguments.evidence_dir,
            run_id=run_id,
        )
        try:
            manifest = demo.run()
        finally:
            demo.close()
        print(
            json.dumps(
                {
                    "result": "PASS",
                    "run_id": run_id,
                    "evidence": str(arguments.evidence_dir.resolve()),
                    "manifest": str(manifest),
                    "manifest_sha256": sha256_file(manifest),
                    "pftl_rpc_left_running_for_verification": True,
                    "lightning_left_running_for_verification": True,
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 0
    except Exception as error:
        print(f"lightning-navcoin-demo: RED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
