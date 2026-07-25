"""Mainnet coordinator orchestration over injected LND and PFTL backends.

The runtime can prepare and lock a swap, but it never exposes a payable
on-ramp invoice or initiates an off-ramp payment until a pinned nazgul-signed
authorization is durably reserved.  PFTL signing remains behind the injected
backend; wallet secrets and the coordinator signer are absent from this class.
"""

from __future__ import annotations

from dataclasses import dataclass
from functools import wraps
import hashlib
import threading
import time
from typing import Any, Callable, Mapping, Protocol

from ..coordinator.journal import (
    CoordinatorJournal,
    JournalError,
    SwapState,
)
from ..coordinator.lnd_grpc import (
    LightningPaymentError,
    LndGrpcAdapter,
    LndGrpcError,
    LndInvoiceNotFound,
    PaymentReconciliationStatus,
)
from ..coordinator.protocol import SecretPreimage, encode_condition, payment_hash
from ..coordinator.service import CoordinatorService
from ..coordinator.signing import QuoteSigner, sign_quote
from .authorization import verify_value_authorization
from .budget import RealValueBudget
from .pftl_quorum import PftlQuorumObserver, PftlRouteSnapshot
from .policy import (
    ExecutionMode,
    PriceObservation,
    RealValuePolicy,
    RealValuePolicyError,
    msat_to_usd_e8_ceil,
    validate_mainnet_quote,
)
from .preflight import run_preflight
from .pricing import FixedNavPricing, NavQuoteTerms


MIN_FINAL_CLTV_DELTA = 144
MAX_TOTAL_CLTV_DELTA = 288
QUOTE_LIFETIME_SECONDS = 120
# Dedicated-chain conservative policy window. This deliberately does not
# convert Bitcoin CLTV into PFTL blocks: there is no consensus-authenticated
# cross-chain clock. The 100-block quote/submission allowance is small relative
# to the million-block remaining-window gate and any shortfall remains HOLD.
PFTL_CANCEL_OFFSET = 1_000_100
PFTL_REQUIRED_FINISH_MARGIN_BLOCKS = 1_000_000
ESCROW_CONDITION_HASH_DOMAIN = b"postfiat.escrow_condition_hash.v1"


class RuntimeError(RealValuePolicyError):
    """A coordinator workflow gate failed closed."""


def _serialize_swap(method: Callable[..., Mapping[str, Any]]) -> Callable[..., Mapping[str, Any]]:
    """Serialize state reconciliation and value effects for one swap id."""

    @wraps(method)
    def guarded(
        self: "MainnetCoordinatorRuntime",
        swap_id: str,
        *args: Any,
        **kwargs: Any,
    ) -> Mapping[str, Any]:
        with self._lock_for_swap(swap_id):
            return method(self, swap_id, *args, **kwargs)

    return guarded


def _serialize_route_value(
    method: Callable[..., Mapping[str, Any]],
) -> Callable[..., Mapping[str, Any]]:
    """Serialize route-wide inventory/headroom observations with their effect."""

    @wraps(method)
    def guarded(
        self: "MainnetCoordinatorRuntime",
        *args: Any,
        **kwargs: Any,
    ) -> Mapping[str, Any]:
        with self._route_value_lock:
            return method(self, *args, **kwargs)

    return guarded


@dataclass(frozen=True)
class PftlEscrowPlan:
    owner: str
    owner_sequence: int
    recipient: str
    expected_escrow_id: str
    operation: Mapping[str, Any]


@dataclass(frozen=True)
class PftlEffect:
    tx_id: str
    accepted: bool
    code: str
    agreeing_validator_count: int
    validator_count: int
    finalized_height: int
    state_root: str
    block_tip_hash: str
    mutation_free: bool | None = None

    def public_evidence(self) -> dict[str, Any]:
        value = {
            "tx_id": self.tx_id,
            "accepted": self.accepted,
            "code": self.code,
            "agreeing_validator_count": self.agreeing_validator_count,
            "validator_count": self.validator_count,
            "finalized_height": self.finalized_height,
            "state_root": self.state_root,
            "block_tip_hash": self.block_tip_hash,
        }
        if self.mutation_free is not None:
            value["mutation_free"] = self.mutation_free
        return value


class PftlEscrowBackend(Protocol):
    """Signer-isolated write boundary supplied by the hardened PFTL handoff."""

    def plan_create(
        self,
        *,
        owner: str,
        recipient: str,
        asset_id: str,
        amount_atoms: int,
        condition: str,
        finish_after: int,
        cancel_after: int,
    ) -> PftlEscrowPlan: ...

    def submit_create(
        self, plan: PftlEscrowPlan, *, effect_key: str
    ) -> PftlEffect: ...

    def submit_finish(
        self,
        *,
        owner: str,
        recipient: str,
        escrow_id: str,
        secret: SecretPreimage,
        effect_key: str,
    ) -> PftlEffect: ...

    def submit_cancel(
        self,
        *,
        owner: str,
        escrow_id: str,
        effect_key: str,
    ) -> PftlEffect: ...


def escrow_condition_hash(condition: str) -> str:
    digest = hashlib.sha3_384()
    digest.update(ESCROW_CONDITION_HASH_DOMAIN)
    digest.update(b"\x00")
    digest.update(condition.encode("ascii"))
    return digest.hexdigest()


class MainnetCoordinatorRuntime:
    def __init__(
        self,
        *,
        policy: RealValuePolicy,
        price: PriceObservation,
        lnd: LndGrpcAdapter,
        pftl_observer: PftlQuorumObserver,
        pftl_backend: PftlEscrowBackend,
        journal: CoordinatorJournal,
        budget: RealValueBudget,
        quote_signer: QuoteSigner,
        fee_bps: int = 0,
        clock: Callable[[], float] = time.time,
    ) -> None:
        self.policy = policy
        self.price = price
        self.lnd = lnd
        self.pftl_observer = pftl_observer
        self.pftl_backend = pftl_backend
        self.journal = journal
        self.service = CoordinatorService(journal)
        self.budget = budget
        self.quote_signer = quote_signer
        self.pricing = FixedNavPricing(price, fee_bps=fee_bps)
        self.clock = clock
        self._quote_lock = threading.RLock()
        self._route_value_lock = threading.RLock()
        self._swap_locks_guard = threading.Lock()
        self._swap_locks: dict[str, threading.RLock] = {}

    def _lock_for_swap(self, swap_id: str) -> threading.RLock:
        if type(swap_id) is not str or not swap_id or len(swap_id) > 256:
            raise RuntimeError("swap_id is invalid")
        with self._swap_locks_guard:
            lock = self._swap_locks.get(swap_id)
            if lock is None:
                lock = threading.RLock()
                self._swap_locks[swap_id] = lock
            return lock

    def _now(self) -> int:
        now = int(self.clock())
        if now < 0:
            raise RuntimeError("coordinator clock is invalid")
        return now

    def _maximum_amount_msat(self) -> int:
        cap = self.policy.max_per_run_usd_e8
        numerator = cap * 100_000_000_000
        maximum_all_in = numerator // self.price.btc_usd_e8
        return max(0, maximum_all_in - self.policy.max_fee_msat)

    def _require_fresh_price(self) -> None:
        now = self._now()
        if self.price.observed_at_unix > now:
            raise RuntimeError("BTC price observation is from the future")
        if (
            now - self.price.observed_at_unix
            > self.policy.max_price_age_seconds
        ):
            raise RuntimeError("BTC price observation is stale")

    def _incoming_invoice_lifetime(self, now: int) -> int:
        """Bound BOLT11 payability to the executable price/quote window."""

        price_valid_until = (
            self.price.observed_at_unix + self.policy.max_price_age_seconds
        )
        remaining_price_seconds = price_valid_until - now
        lifetime = min(
            QUOTE_LIFETIME_SECONDS,
            self.policy.max_quote_lifetime_seconds,
            remaining_price_seconds,
        )
        if lifetime < 1:
            raise RuntimeError(
                "BTC price validity cannot cover a payable invoice window"
            )
        return lifetime

    def public_status(self) -> Mapping[str, Any]:
        blockers: list[str] = []
        lnd_status: dict[str, Any] = {
            "network": "mainnet",
            "identity_pubkey": self.policy.expected_lnd_pubkey,
            "synced_to_chain": False,
            "synced_to_graph": False,
            "active_channels": 0,
            "inbound_msat": 0,
            "outbound_msat": 0,
        }
        pftl_status: dict[str, Any] = {
            "chain_id": self.policy.pftl_chain_id,
            "genesis_hash": self.policy.pftl_genesis_hash,
            "asset_id": self.policy.pftl_asset_id,
            "asset_precision": self.policy.pftl_asset_precision,
            "user_address": self.policy.pftl_user_address,
            "nav_epoch": self.policy.pftl_nav_epoch,
            "nav_reserve_packet_hash": self.policy.pftl_nav_reserve_packet_hash,
            "quorum": {
                "observed": 0,
                "required": 6,
                "validator_count": 6,
                "converged": False,
            },
        }
        try:
            node = self.lnd.preflight_node(
                expected_identity_pubkey=self.policy.expected_lnd_pubkey,
                min_active_channels=1,
                min_inbound_msat=1,
                min_outbound_msat=0,
            )
            lnd_status.update(
                {
                    "identity_pubkey": node.node.identity_pubkey,
                    "synced_to_chain": node.node.synced_to_chain,
                    "synced_to_graph": node.node.synced_to_graph,
                    "block_height": node.node.block_height,
                    "version": node.node.version,
                    "commit_hash": node.node.commit_hash,
                    "active_channels": node.liquidity.active_channels,
                    "unconfirmed_active_channels": (
                        node.liquidity.unconfirmed_active_channels
                    ),
                    "inbound_msat": node.liquidity.inbound_msat,
                    "outbound_msat": node.liquidity.outbound_msat,
                }
            )
        except Exception:
            blockers.append("mainnet_lnd_identity_sync_or_liquidity_not_green")
        try:
            snapshot = self.pftl_observer.route_snapshot()
            pftl_status.update(
                {
                    "height": snapshot.height,
                    "block_tip_hash": snapshot.block_tip_hash,
                    "state_root": snapshot.state_root,
                    "build_git_revision": snapshot.build_git_revision,
                    "nav_per_unit": snapshot.nav_per_unit,
                    "asset_precision": snapshot.asset_precision,
                    "coordinator_inventory_atoms": snapshot.coordinator_inventory_atoms,
                    "coordinator_receive_headroom_atoms": (
                        snapshot.coordinator_receive_headroom_atoms
                    ),
                    "quorum": {
                        "observed": snapshot.agreeing_validator_count,
                        "required": 6,
                        "validator_count": snapshot.validator_count,
                        "converged": True,
                    },
                }
            )
        except Exception:
            blockers.append("persistent_hardened_pftl_handoff_not_green")
        try:
            self._require_fresh_price()
        except Exception:
            blockers.append("operator_reviewed_btc_price_not_fresh")
        if self.policy.mode is not ExecutionMode.ARMED:
            blockers.append("real_value_policy_mode_is_dry_run")
        budget = self.budget.summary()
        if budget["remaining_usd_e8"] <= 0:
            blockers.append("real_value_lifetime_budget_exhausted")
        return {
            "schema": "postfiat.lightning_navcoin.status.v1",
            "lightning_network": "bitcoin",
            "mode": "ARMED" if not blockers else "HOLD",
            "configured_mode": self.policy.mode.value,
            "can_execute": not blockers,
            "hold_reasons": sorted(set(blockers)),
            "trust_class": self.policy.trust_class,
            "atomicity_claim": self.policy.atomicity_claim,
            "quote_signer_public_key_hex": (
                self.policy.quote_signer_public_key_hex
            ),
            "pricing": {
                "btc_usd_e8": self.price.btc_usd_e8,
                "source": self.price.source,
                "observed_at_unix": self.price.observed_at_unix,
                "max_age_seconds": self.policy.max_price_age_seconds,
            },
            "lnd": lnd_status,
            "pftl": pftl_status,
            "limits": {
                "per_run_usd_e8": self.policy.max_per_run_usd_e8,
                "total_usd_e8": self.policy.max_lifetime_usd_e8,
                "remaining_usd_e8": budget["remaining_usd_e8"],
                "max_amount_msat": self._maximum_amount_msat(),
                "max_fee_msat": self.policy.max_fee_msat,
            },
        }

    def _route_and_terms(
        self, direction: str, amount_msat: int
    ) -> tuple[PftlRouteSnapshot, NavQuoteTerms]:
        self._require_fresh_price()
        preflight = run_preflight(
            self.policy,
            lnd=self.lnd,
            pftl=self.pftl_observer,
            budget=self.budget,
            direction=direction,
            amount_msat=amount_msat,
        )
        snapshot = preflight.pftl
        terms = self.pricing.terms(
            direction=direction,
            invoice_amount_msat=amount_msat,
            nav_per_unit_e8=snapshot.nav_per_unit,
            asset_precision=snapshot.asset_precision,
        )
        if (
            direction == "lightning_to_pftl"
            and terms.pftl_amount_atoms > snapshot.coordinator_inventory_atoms
        ):
            raise RuntimeError("coordinator NAVcoin inventory is insufficient")
        if (
            direction == "lightning_to_pftl"
            and terms.pftl_amount_atoms > snapshot.user_receive_headroom_atoms
        ):
            raise RuntimeError("pinned user NAVcoin receive headroom is insufficient")
        if (
            direction == "pftl_to_lightning"
            and terms.pftl_amount_atoms
            > snapshot.coordinator_receive_headroom_atoms
        ):
            raise RuntimeError("coordinator NAVcoin receive headroom is insufficient")
        if (
            direction == "pftl_to_lightning"
            and terms.pftl_amount_atoms > snapshot.user_balance_atoms
        ):
            raise RuntimeError("pinned user NAVcoin balance is insufficient")
        all_in_usd = msat_to_usd_e8_ceil(
            amount_msat + self.policy.max_fee_msat,
            self.price.btc_usd_e8,
        )
        if all_in_usd > self.policy.max_per_run_usd_e8:
            raise RuntimeError("quote exceeds real-value per-run cap")
        return snapshot, terms

    def create_quote(self, request: Mapping[str, Any]) -> Mapping[str, Any]:
        """Create one quote per canonical client request id, durably idempotent."""

        client_request_id = request.get("client_request_id")
        if (
            type(client_request_id) is not str
            or len(client_request_id) != 64
            or any(character not in "0123456789abcdef" for character in client_request_id)
        ):
            raise RuntimeError(
                "client_request_id must be canonical lowercase 32-byte hex"
            )
        recover_existing = False
        with self._quote_lock:
            try:
                existing = self.journal.get_swap(client_request_id)
            except JournalError as error:
                if str(error) != "unknown swap":
                    raise RuntimeError("quote idempotency lookup failed") from error
            else:
                # Repair a crash after swap admission but before the additive
                # pre-quote intent was marked complete.
                self.journal.complete_quote_intent(
                    client_request_id, client_request_id
                )
                quote = existing["signed_quote"]["quote"]
                wallet_address = (
                    quote["pftl_recipient"]
                    if quote["direction"] == "lightning_to_pftl"
                    else quote["pftl_owner"]
                )
                expected = {
                    "direction": quote["direction"],
                    "amount_msat": quote["invoice_amount_msat"],
                    "wallet_address": wallet_address,
                    "client_request_id": quote["swap_id"],
                }
                if quote["direction"] == "pftl_to_lightning":
                    expected["invoice"] = quote["invoice"]
                normalized = {
                    key: request.get(key)
                    for key in expected
                }
                if normalized != expected or frozenset(request.keys()) != frozenset(
                    expected.keys()
                ):
                    raise RuntimeError(
                        "client_request_id was reused for a different quote request"
                    )
                if (
                    existing["state"] == SwapState.PFTL_LOCK_SUBMITTED.value
                    and quote["quote_expires_unix"] <= self._now()
                ):
                    # Keep the global/swap lock order consistent. recover_swap
                    # takes the per-swap lock before the quote lock, so never
                    # invoke it while this global quote lock is held.
                    recover_existing = True
                else:
                    return self.public_swap(client_request_id)
            if not recover_existing:
                return self._create_quote_once(request, client_request_id)
        return self.recover_swap(client_request_id)

    def _create_quote_once(
        self,
        request: Mapping[str, Any],
        client_request_id: str,
    ) -> Mapping[str, Any]:
        direction = request["direction"]
        amount_msat = request["amount_msat"]
        wallet_address = request["wallet_address"]
        if wallet_address != self.policy.pftl_user_address:
            raise RuntimeError(
                "wallet address does not match the pinned real-value demo user"
            )
        snapshot, terms = self._route_and_terms(direction, amount_msat)
        if self.policy.mode is not ExecutionMode.ARMED:
            return {
                "schema": "postfiat.lightning_navcoin.quote_preview.v1",
                "state": "DRY_RUN",
                "can_execute": False,
                "hold_reasons": ["real_value_policy_mode_is_dry_run"],
                "direction": direction,
                "amount_msat": amount_msat,
                "pftl_amount_atoms": terms.pftl_amount_atoms,
                "coordinator_fee_atoms": terms.coordinator_fee_atoms,
                "nav_epoch": snapshot.nav_epoch,
                "nav_per_unit": snapshot.nav_per_unit,
                "asset_precision": snapshot.asset_precision,
                "nav_reserve_packet_hash": snapshot.nav_reserve_packet_hash,
                "pricing": {
                    "btc_usd_e8": terms.btc_usd_e8,
                    "price_source": self.price.source,
                    "price_observed_at_unix": self.price.observed_at_unix,
                    "rounding": terms.rounding,
                },
            }

        now = self._now()
        intent_request = {
            "direction": direction,
            "amount_msat": amount_msat,
            "wallet_address": wallet_address,
            "client_request_id": client_request_id,
        }
        if direction == "pftl_to_lightning":
            intent_request["invoice"] = request.get("invoice")
        try:
            quote_intent = self.journal.reserve_quote_intent(
                client_request_id,
                intent_request,
                create_invoice_preimage=direction == "lightning_to_pftl",
            )
        except JournalError as error:
            raise RuntimeError("durable quote intent conflicts with request") from error
        if direction == "lightning_to_pftl":
            secret = quote_intent.invoice_preimage
            if secret is None:
                raise RuntimeError("durable on-ramp invoice secret is absent")
            invoice_lifetime = self._incoming_invoice_lifetime(now)
            if quote_intent.created:
                created = self.lnd.add_invoice(
                    secret,
                    amount_msat=amount_msat,
                    expiry_seconds=invoice_lifetime,
                    min_final_cltv_delta=MIN_FINAL_CLTV_DELTA,
                    memo="PostFiat CONTROLLED NAVcoin",
                )
            else:
                try:
                    created = self.lnd.recover_created_invoice(
                        payment_hash(secret),
                        expected_amount_msat=amount_msat,
                        expected_payee=self.policy.expected_lnd_pubkey,
                        expected_min_final_cltv_delta=MIN_FINAL_CLTV_DELTA,
                    )
                except LndInvoiceNotFound:
                    # NOT_FOUND is the only conclusive proof that the earlier
                    # AddInvoice did not commit. Reuse the same persisted
                    # preimage/hash; every transport ambiguity remains HOLD.
                    created = self.lnd.add_invoice(
                        secret,
                        amount_msat=amount_msat,
                        expiry_seconds=invoice_lifetime,
                        min_final_cltv_delta=MIN_FINAL_CLTV_DELTA,
                        memo="PostFiat CONTROLLED NAVcoin",
                    )
            facts = created.facts
            if (
                facts.timestamp_unix > now
                or facts.expiry_seconds
                > min(
                    QUOTE_LIFETIME_SECONDS,
                    self.policy.max_quote_lifetime_seconds,
                )
                or facts.expiry_unix
                > self.price.observed_at_unix
                + self.policy.max_price_age_seconds
            ):
                raise RuntimeError(
                    "incoming invoice payability outlasts executable quote validity"
                )
            add_index = created.add_index
            owner = self.policy.coordinator_pftl_address
            recipient = wallet_address
        else:
            invoice = request.get("invoice")
            facts = self.lnd.decode_invoice(invoice)
            if facts.is_amp:
                raise RuntimeError("AMP invoices are unsupported")
            if facts.amount_msat != amount_msat:
                raise RuntimeError("off-ramp invoice amount does not match request")
            if facts.expiry_unix <= now:
                raise RuntimeError("off-ramp invoice is expired")
            if facts.min_final_cltv_delta != MIN_FINAL_CLTV_DELTA:
                raise RuntimeError("off-ramp invoice final CLTV delta is unsupported")
            created = None
            add_index = 0
            owner = wallet_address
            recipient = self.policy.coordinator_pftl_address

        condition = encode_condition(facts.payment_hash)
        cancel_after = snapshot.height + PFTL_CANCEL_OFFSET
        plan = self.pftl_backend.plan_create(
            owner=owner,
            recipient=recipient,
            asset_id=self.policy.pftl_asset_id,
            amount_atoms=terms.pftl_amount_atoms,
            condition=condition,
            finish_after=0,
            cancel_after=cancel_after,
        )
        if plan.owner != owner or plan.recipient != recipient:
            raise RuntimeError("PFTL backend returned different escrow parties")
        quote_expiry = min(
            now + QUOTE_LIFETIME_SECONDS,
            now + self.policy.max_quote_lifetime_seconds,
            self.price.observed_at_unix + self.policy.max_price_age_seconds,
            facts.expiry_unix,
        )
        if quote_expiry <= now:
            raise RuntimeError("quote has no executable lifetime")
        quote = {
            "schema": "postfiat.lightning_submarine_quote.v1",
            "swap_id": client_request_id,
            "quote_expires_unix": quote_expiry,
            "direction": direction,
            "payment_hash": facts.payment_hash.hex(),
            "lightning_network": "bitcoin",
            "invoice": (
                created.payment_request
                if created is not None
                else request["invoice"]
            ),
            "invoice_payee": facts.payee,
            "invoice_amount_msat": amount_msat,
            "invoice_expiry_unix": facts.expiry_unix,
            "min_final_cltv_delta": facts.min_final_cltv_delta,
            "max_total_cltv_delta": MAX_TOTAL_CLTV_DELTA,
            "pftl_chain_id": self.policy.pftl_chain_id,
            "pftl_genesis_hash": self.policy.pftl_genesis_hash,
            "pftl_asset_id": self.policy.pftl_asset_id,
            "pftl_amount_atoms": terms.pftl_amount_atoms,
            "pftl_owner": plan.owner,
            "pftl_owner_sequence": plan.owner_sequence,
            "pftl_recipient": plan.recipient,
            "expected_escrow_id": plan.expected_escrow_id,
            "condition": condition,
            "finish_after": 0,
            "cancel_after": cancel_after,
            "latest_lightning_start_unix": quote_expiry,
            "rate_numerator": terms.rate_numerator,
            "rate_denominator": terms.rate_denominator,
            "coordinator_fee_atoms": terms.coordinator_fee_atoms,
            "nav_epoch": self.policy.pftl_nav_epoch,
            "nav_reserve_packet_hash": self.policy.pftl_nav_reserve_packet_hash,
            "custody_class": "NON_CUSTODIAL_HASHLOCK",
            "atomicity_class": "CONDITIONAL_HTLC",
            "timeout_clock_class": "OFFCHAIN_CROSS_LEDGER_POLICY",
            "asset_control_class": "CONTROLLED_ISSUED_ASSET",
        }
        signed = sign_quote(quote, self.quote_signer)
        view = validate_mainnet_quote(
            signed, self.policy, self.price, now_unix=now
        )
        principal = f"wallet:{wallet_address}"
        self.service.admit_quote(principal, signed)
        self.journal.complete_quote_intent(client_request_id, view.swap_id)
        effect_key = f"{view.swap_id}:pftl-create"
        intent = {
            "escrow_id": plan.expected_escrow_id,
            "owner": plan.owner,
            "recipient": plan.recipient,
            "operation": dict(plan.operation),
            "condition_hash": escrow_condition_hash(condition),
            "lnd_add_index": add_index,
        }
        self.service.mark_lock_submitted(
            view.swap_id,
            effect_key=effect_key,
            operation=intent,
        )
        # The on-ramp lock is a real PFTL inventory mutation.  It remains a
        # durable intent until a one-use value authorization is reserved.
        return self.public_swap(view.swap_id)

    @_serialize_route_value
    def _submit_onramp_lock(self, swap_id: str) -> Mapping[str, Any]:
        """Execute or reconcile an authorized coordinator-owned PFTL lock."""

        swap = self.journal.get_swap(swap_id)
        if swap["direction"] != "lightning_to_pftl":
            raise RuntimeError("coordinator lock submission applies only to on-ramp")
        if swap["state"] == SwapState.PFTL_LOCK_FINAL.value:
            return self.public_swap(swap_id)
        if swap["state"] == SwapState.LOCK_FAILED.value:
            raise RuntimeError("PFTL on-ramp lock was terminally rejected")
        if swap["state"] != SwapState.PFTL_LOCK_SUBMITTED.value:
            raise RuntimeError("on-ramp has no durable PFTL lock intent")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None or authorization["state"] != "RESERVED":
            raise RuntimeError("PFTL on-ramp lock lacks reserved value authorization")
        quote = swap["signed_quote"]["quote"]
        view = validate_mainnet_quote(
            swap["signed_quote"],
            self.policy,
            self.price,
            now_unix=self._now(),
        )
        if view.swap_id != swap_id:
            raise RuntimeError("durable quote identity is inconsistent")
        # Re-read all no-spend gates immediately before the first inventory
        # mutation. The quote validation performed by authorize_swap separately
        # pins the price, NAV epoch, route, amount, and expiry.
        self.lnd.preflight_node(
            expected_identity_pubkey=self.policy.expected_lnd_pubkey,
            min_active_channels=1,
            min_inbound_msat=quote["invoice_amount_msat"],
            min_outbound_msat=0,
        )
        snapshot = self.pftl_observer.route_snapshot()
        effect_key = f"{swap_id}:pftl-create"
        effect_row = self._swap_side_effect(swap_id, "PFTL_ESCROW_CREATE")
        if effect_row is None or effect_row["effect_key"] != effect_key:
            raise RuntimeError("durable PFTL create intent is absent or inconsistent")
        if effect_row["status"] == "FAILED_TERMINAL":
            raise RuntimeError("PFTL on-ramp lock was terminally rejected")
        if effect_row["status"] not in {"PENDING", "SUCCEEDED"}:
            raise RuntimeError("PFTL create effect has an unknown durable status")
        checkpoint_key = f"{effect_key}:route-baseline"
        checkpoint = self.journal.side_effect_checkpoint(checkpoint_key)
        if checkpoint is None:
            if (
                effect_row["status"] != "PENDING"
                or effect_row["attempt_count"] != 0
            ):
                raise RuntimeError(
                    "durable PFTL create route baseline is absent after submission"
                )
            if snapshot.coordinator_inventory_atoms < quote["pftl_amount_atoms"]:
                raise RuntimeError("coordinator NAVcoin inventory is insufficient")
            if snapshot.user_receive_headroom_atoms < quote["pftl_amount_atoms"]:
                raise RuntimeError(
                    "pinned user NAVcoin receive headroom is insufficient"
                )
            checkpoint = self.journal.record_side_effect_checkpoint(
                effect_key,
                checkpoint_key,
                evidence={
                    "route_before": snapshot.to_dict(),
                    "coordinator_inventory_before_atoms": (
                        snapshot.coordinator_inventory_atoms
                    ),
                    "user_balance_before_atoms": snapshot.user_balance_atoms,
                    "agreeing_validator_count": 6,
                    "validator_count": 6,
                },
            )
        route_before = checkpoint["evidence"].get("route_before")
        inventory_before = checkpoint["evidence"].get(
            "coordinator_inventory_before_atoms"
        )
        wallet_before = checkpoint["evidence"].get(
            "user_balance_before_atoms"
        )
        if (
            not isinstance(route_before, Mapping)
            or type(inventory_before) is not int
            or inventory_before < 0
            or type(wallet_before) is not int
            or wallet_before < 0
        ):
            raise RuntimeError("durable pre-create route evidence is malformed")
        if (
            effect_row["status"] == "PENDING"
            and effect_row["attempt_count"] == 0
            and (
                snapshot.coordinator_inventory_atoms != inventory_before
                or snapshot.user_balance_atoms != wallet_before
            )
        ):
            raise RuntimeError("PFTL route changed after the create baseline")
        payload = effect_row["payload"]
        operation = payload.get("operation")
        if not isinstance(operation, Mapping):
            raise RuntimeError("durable PFTL create operation is malformed")
        plan = PftlEscrowPlan(
            owner=quote["pftl_owner"],
            owner_sequence=quote["pftl_owner_sequence"],
            recipient=quote["pftl_recipient"],
            expected_escrow_id=quote["expected_escrow_id"],
            operation=dict(operation),
        )
        try:
            effect = self.pftl_backend.submit_create(plan, effect_key=effect_key)
        except Exception as error:
            if effect_row["status"] == "PENDING":
                self.journal.record_side_effect_attempt(
                    effect_key,
                    f"{effect_key}:uncertain:{self._now()}",
                    "RETRYABLE_FAILURE",
                    result={"outcome": "uncertain"},
                )
            raise RuntimeError(
                "PFTL create outcome is uncertain; idempotent recovery required"
            ) from error
        if not effect.accepted or effect.code != "accepted":
            if effect.mutation_free is not True:
                raise RuntimeError(
                    "PFTL lock rejected without mutation-free evidence"
                )
            self.service.mark_lock_failed(
                swap_id,
                rejection_evidence=effect.public_evidence(),
            )
            self._release_failed_onramp_budget(swap_id)
            raise RuntimeError(f"PFTL lock rejected: {effect.code}")
        if effect_row["status"] == "PENDING":
            self.journal.record_side_effect_attempt(
                effect_key,
                f"{effect_key}:accepted:{effect.tx_id}",
                "SUCCEEDED",
                result=effect.public_evidence(),
            )
        receipt = self.pftl_observer.receipt(effect.tx_id)
        if (
            receipt["accepted"] is not True
            or receipt["code"] != "accepted"
            or receipt["agreeing_validator_count"] != 6
            or receipt["validator_count"] != 6
        ):
            raise RuntimeError("PFTL lock lacks literal six-of-six ACCEPTED receipt")
        expected = {
            "owner": quote["pftl_owner"],
            "recipient": quote["pftl_recipient"],
            "asset_id": quote["pftl_asset_id"],
            "amount": quote["pftl_amount_atoms"],
            "condition_hash": escrow_condition_hash(quote["condition"]),
            "finish_after": quote["finish_after"],
            "cancel_after": quote["cancel_after"],
        }
        lock = self.pftl_observer.user_finish_capacity(
            quote["expected_escrow_id"], expected=expected
        )
        snapshot_after = self.pftl_observer.route_snapshot()
        expected_inventory_after = (
            inventory_before - quote["pftl_amount_atoms"]
        )
        if (
            expected_inventory_after < 0
            or snapshot_after.coordinator_inventory_atoms
            != expected_inventory_after
            or snapshot_after.user_balance_atoms != wallet_before
            or snapshot_after.state_root != lock["state_root"]
            or snapshot_after.block_tip_hash != lock["block_tip_hash"]
            or snapshot_after.height < lock["height"]
        ):
            raise RuntimeError(
                "PFTL lock does not have the exact route-wide inventory delta"
            )
        if (
            quote["cancel_after"] - lock["height"]
            < PFTL_REQUIRED_FINISH_MARGIN_BLOCKS
        ):
            raise RuntimeError(
                "PFTL refund boundary lacks the conservative release margin"
            )
        self.service.mark_lock_final(
            swap_id,
            finality_evidence={
                "tx_id": effect.tx_id,
                "accepted": True,
                "code": "accepted",
                "agreeing_validator_count": lock["agreeing_validator_count"],
                "validator_count": lock["validator_count"],
                "height": lock["height"],
                "state_root": lock["state_root"],
                "block_tip_hash": lock["block_tip_hash"],
                "escrow": dict(lock["escrow"]),
                "recipient_asset_headroom": lock["recipient_asset_headroom"],
                "recipient_native_balance": lock["recipient_native_balance"],
                "finish_minimum_fee": lock["finish_minimum_fee"],
                "account_reserve": lock["account_reserve"],
                "route_before": dict(route_before),
                "route_after": snapshot_after.to_dict(),
                "coordinator_inventory_delta_atoms": (
                    -quote["pftl_amount_atoms"]
                ),
                "user_balance_delta_atoms": 0,
            },
        )
        return self.public_swap(swap_id)

    def _release_failed_onramp_budget(self, swap_id: str) -> Mapping[str, Any]:
        swap = self.journal.get_swap(swap_id)
        if (
            swap["direction"] != "lightning_to_pftl"
            or swap["state"] != SwapState.LOCK_FAILED.value
        ):
            raise RuntimeError("only a mutation-free failed on-ramp can be released")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None:
            raise RuntimeError("failed on-ramp has no value authorization")
        if authorization["state"] == "RELEASED":
            return self.public_swap(swap_id)
        if authorization["state"] != "RESERVED":
            raise RuntimeError("failed on-ramp authorization is not releasable")
        failure_events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.LOCK_FAILED.value
        ]
        if len(failure_events) != 1:
            raise RuntimeError("failed on-ramp rejection evidence is absent")
        evidence = failure_events[0]["evidence"]
        if (
            evidence.get("accepted") is not False
            or evidence.get("mutation_free") is not True
        ):
            raise RuntimeError("failed on-ramp lacks mutation-free rejection evidence")
        effect = self._swap_side_effect(swap_id, "PFTL_ESCROW_CREATE")
        if effect is None:
            raise RuntimeError("failed on-ramp has no durable PFTL create intent")
        if effect["status"] == "PENDING":
            self.journal.record_side_effect_attempt(
                effect["effect_key"],
                f"{effect['effect_key']}:rejected:{evidence['tx_id']}",
                "TERMINAL_FAILURE",
                result=evidence,
            )
        elif effect["status"] != "FAILED_TERMINAL":
            raise RuntimeError("failed on-ramp side-effect status is inconsistent")
        self.budget.release_unspent(
            authorization["authorization_id"],
            no_value_evidence={
                "value_moved": False,
                "pftl_tx_id": evidence["tx_id"],
                "accepted": False,
                "mutation_free": True,
                "reason": evidence["code"],
            },
            now_unix=self._now(),
        )
        return self.public_swap(swap_id)

    def _release_aborted_onramp_budget(self, swap_id: str) -> Mapping[str, Any]:
        """Close a crash gap after a zero-attempt abort committed first."""

        swap = self.journal.get_swap(swap_id)
        if (
            swap["direction"] != "lightning_to_pftl"
            or swap["state"] != SwapState.ABORTED_NO_VALUE.value
        ):
            raise RuntimeError("only a no-value aborted on-ramp can be released")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None:
            return self.public_swap(swap_id)
        if authorization["state"] == "RELEASED":
            return self.public_swap(swap_id)
        if authorization["state"] != "RESERVED":
            raise RuntimeError("aborted on-ramp authorization is not releasable")
        events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.ABORTED_NO_VALUE.value
        ]
        if len(events) != 1:
            raise RuntimeError("aborted on-ramp no-value evidence is absent")
        evidence = events[0]["evidence"]
        effect = self._swap_side_effect(swap_id, "PFTL_ESCROW_CREATE")
        if (
            evidence.get("value_moved") is not False
            or evidence.get("pftl_submission_attempts") != 0
            or evidence.get("invoice_publicly_released") is not False
            or effect is None
            or effect["status"] != "FAILED_TERMINAL"
            or effect["attempt_count"] != 0
        ):
            raise RuntimeError(
                "aborted on-ramp lacks zero-attempt no-value evidence"
            )
        self.budget.release_unspent(
            authorization["authorization_id"],
            no_value_evidence={
                "value_moved": False,
                "reason": "authorized_onramp_expired_before_unattempted_lock",
                "pftl_submission_attempts": 0,
                "invoice_publicly_released": False,
            },
            now_unix=self._now(),
        )
        return self.public_swap(swap_id)

    def _swap_side_effect(self, swap_id: str, kind: str) -> Mapping[str, Any] | None:
        try:
            return self.journal.side_effect(swap_id, kind)
        except JournalError as error:
            raise RuntimeError("durable side-effect read failed") from error

    def _execution_expiry_reasons(
        self,
        swap: Mapping[str, Any],
        authorization: Mapping[str, Any] | None,
    ) -> list[str]:
        quote = swap["signed_quote"]["quote"]
        now = self._now()
        reasons = [
            reason
            for boundary, reason in (
                (quote["quote_expires_unix"], "quote_expired"),
                (
                    quote["latest_lightning_start_unix"],
                    "safe_lightning_start_cutoff_passed",
                ),
                (quote["invoice_expiry_unix"], "lightning_invoice_expired"),
            )
            if boundary <= now
        ]
        if (
            authorization is not None
            and authorization["state"] == "RESERVED"
            and authorization["expires_unix"] <= now
        ):
            reasons.append("value_authorization_expired")
        return reasons

    def _mark_offramp_not_started_expired(
        self,
        swap_id: str,
        *,
        reasons: list[str],
    ) -> Mapping[str, Any]:
        """Make a late user lock refundable without ever calling LND."""

        if not reasons:
            raise RuntimeError("expired off-ramp transition requires an expiry reason")
        if self._swap_side_effect(swap_id, "LND_SEND_PAYMENT") is not None:
            raise RuntimeError(
                "off-ramp has a Lightning intent and requires hash reconciliation"
            )
        swap = self.journal.get_swap(swap_id)
        quote = swap["signed_quote"]["quote"]
        self.service.mark_refund_eligible(
            swap_id,
            reason_evidence={
                "payment_hash": quote["payment_hash"],
                "lightning_terminal_status": "NOT_STARTED_EXPIRED",
                "pftl_principal_moved": False,
                "expiry_reasons": sorted(set(reasons)),
            },
            effect_key=f"{swap_id}:user-pftl-cancel",
            cancel_operation={
                "operation": "escrow_cancel",
                "owner": quote["pftl_owner"],
                "escrow_id": quote["expected_escrow_id"],
                "cancel_after": quote["cancel_after"],
                "requires_user_signature": True,
            },
        )
        return self.public_swap(swap_id)

    @_serialize_swap
    def recover_swap(self, swap_id: str) -> Mapping[str, Any]:
        """Resume one durable state without repeating an unclassified effect."""

        with self._quote_lock:
            if self._expire_unfunded_quote(swap_id):
                return self.public_swap(swap_id)
        swap = self.journal.get_swap(swap_id)
        state = SwapState(swap["state"])
        if state is SwapState.QUOTED:
            quote = swap["signed_quote"]["quote"]
            plan = self.pftl_backend.plan_create(
                owner=quote["pftl_owner"],
                recipient=quote["pftl_recipient"],
                asset_id=quote["pftl_asset_id"],
                amount_atoms=quote["pftl_amount_atoms"],
                condition=quote["condition"],
                finish_after=quote["finish_after"],
                cancel_after=quote["cancel_after"],
            )
            if (
                plan.owner != quote["pftl_owner"]
                or plan.owner_sequence != quote["pftl_owner_sequence"]
                or plan.recipient != quote["pftl_recipient"]
                or plan.expected_escrow_id != quote["expected_escrow_id"]
            ):
                raise RuntimeError(
                    "PFTL lock plan changed after crash; no value was submitted"
                )
            self.service.mark_lock_submitted(
                swap_id,
                effect_key=f"{swap_id}:pftl-create",
                operation={
                    "escrow_id": plan.expected_escrow_id,
                    "owner": plan.owner,
                    "recipient": plan.recipient,
                    "operation": dict(plan.operation),
                    "condition_hash": escrow_condition_hash(quote["condition"]),
                    "lnd_add_index": 0,
                },
            )
            swap = self.journal.get_swap(swap_id)
            state = SwapState(swap["state"])
        if (
            state is SwapState.PFTL_LOCK_SUBMITTED
            and swap["direction"] == "lightning_to_pftl"
        ):
            authorization = self.budget.authorization_for_swap(swap_id)
            if authorization is not None and authorization["state"] == "RESERVED":
                return self._submit_onramp_lock(swap_id)
        if (
            state is SwapState.LOCK_FAILED
            and swap["direction"] == "lightning_to_pftl"
        ):
            return self._release_failed_onramp_budget(swap_id)
        if (
            state is SwapState.ABORTED_NO_VALUE
            and swap["direction"] == "lightning_to_pftl"
        ):
            return self._release_aborted_onramp_budget(swap_id)
        if (
            state is SwapState.PFTL_LOCK_FINAL
            and swap["direction"] == "pftl_to_lightning"
        ):
            return self.execute_offramp(swap_id)
        if (
            state is SwapState.LN_IN_FLIGHT
            and swap["direction"] == "pftl_to_lightning"
        ):
            return self.reconcile_offramp(swap_id)
        if (
            state is SwapState.LN_SETTLED
            and swap["direction"] == "pftl_to_lightning"
        ):
            return self.recover_offramp_finish(swap_id)
        if (
            state is SwapState.REFUND_ELIGIBLE
        ):
            if swap["direction"] == "lightning_to_pftl":
                return self.recover_onramp_refund(swap_id)
            refund_events = [
                event
                for event in self.journal.events(swap_id)
                if event["to_state"] == SwapState.REFUND_ELIGIBLE.value
            ]
            if (
                len(refund_events) == 1
                and refund_events[0]["evidence"].get(
                    "lightning_terminal_status"
                )
                == "FAILED"
            ):
                return self._finalize_failed_offramp(swap_id)
            return self.public_swap(swap_id)
        if state is SwapState.PFTL_FINISH_FINAL:
            if swap["direction"] == "pftl_to_lightning":
                return self._mark_offramp_budget_spent(swap_id)
            return self.public_swap(swap_id)
        if (
            state is SwapState.PFTL_CANCEL_FINAL
        ):
            if swap["direction"] == "lightning_to_pftl":
                return self._release_refunded_onramp_budget(swap_id)
            return self._release_refunded_offramp_budget(swap_id)
        if (
            state
            in {
                SwapState.PFTL_LOCK_FINAL,
                SwapState.LN_IN_FLIGHT,
                SwapState.LN_SETTLED,
            }
            and swap["direction"] == "lightning_to_pftl"
        ):
            return self.refresh_onramp(swap_id)
        return self.public_swap(swap_id)

    def recovery_swap_ids(self, *, limit: int = 256) -> tuple[str, ...]:
        """Include terminal journal rows whose separate budget close may lag."""

        if type(limit) is not int or limit < 1 or limit > 4096:
            raise RuntimeError("recovery scan limit is invalid")
        ordered: list[str] = []
        seen: set[str] = set()
        for action in self.service.recovery_plan():
            if action.swap_id not in seen:
                ordered.append(action.swap_id)
                seen.add(action.swap_id)
            if len(ordered) >= limit:
                return tuple(ordered)
        budget_terminal = {
            SwapState.ABORTED_NO_VALUE.value,
            SwapState.LOCK_FAILED.value,
            SwapState.PFTL_FINISH_FINAL.value,
            SwapState.PFTL_CANCEL_FINAL.value,
        }
        for swap_id in self.budget.reserved_swap_ids(limit=limit):
            if swap_id in seen:
                continue
            swap = self.journal.get_swap(swap_id)
            if swap["state"] in budget_terminal:
                ordered.append(swap_id)
                seen.add(swap_id)
            if len(ordered) >= limit:
                break
        return tuple(ordered)

    def _expire_unfunded_quote(self, swap_id: str) -> bool:
        """Release admission exposure only when no value effect was attempted."""

        swap = self.journal.get_swap(swap_id)
        if swap["state"] != SwapState.PFTL_LOCK_SUBMITTED.value:
            return False
        quote = swap["signed_quote"]["quote"]
        now = self._now()
        authorization = self.budget.authorization_for_swap(swap_id)
        boundaries = (
            quote["quote_expires_unix"],
            quote["latest_lightning_start_unix"],
            quote["invoice_expiry_unix"],
            (
                authorization["expires_unix"]
                if authorization is not None
                else (1 << 63) - 1
            ),
        )
        if min(boundaries) > now:
            return False
        if (
            authorization is not None
            and (
                swap["direction"] != "lightning_to_pftl"
                or authorization["state"] != "RESERVED"
            )
        ):
            return False
        effect = self._swap_side_effect(swap_id, "PFTL_ESCROW_CREATE")
        if effect is None:
            raise RuntimeError("expired quote has no durable PFTL create intent")
        if effect["status"] != "PENDING" or effect["attempt_count"] != 0:
            # Any external-call ambiguity remains durably held for an operator;
            # expiry is not evidence that no PFTL mutation occurred.
            return False
        self.service.abort_unattempted_lock(
            swap_id,
            effect_key=effect["effect_key"],
            abort_evidence={
                "value_moved": False,
                "reason": "quote_expired_without_value_authorization",
                "quote_expires_unix": quote["quote_expires_unix"],
                "pftl_submission_attempts": 0,
                "invoice_publicly_released": False,
            },
        )
        if authorization is not None:
            self.budget.release_unspent(
                authorization["authorization_id"],
                no_value_evidence={
                    "value_moved": False,
                    "reason": "authorized_onramp_expired_before_unattempted_lock",
                    "pftl_submission_attempts": 0,
                    "invoice_publicly_released": False,
                },
                now_unix=now,
            )
        return True

    def public_swap(self, swap_id: str) -> Mapping[str, Any]:
        swap = self.journal.get_swap(swap_id)
        authorization = self.budget.authorization_for_swap(swap_id)
        authorized = authorization is not None and authorization["state"] in {
            "RESERVED",
            "SPENT",
        }
        create_effect = self._swap_side_effect(swap_id, "PFTL_ESCROW_CREATE")
        quote = swap["signed_quote"]["quote"]
        executable_states = {
            SwapState.PFTL_LOCK_FINAL.value,
            SwapState.LN_IN_FLIGHT.value,
            SwapState.LN_SETTLED.value,
        }
        if swap["direction"] == "pftl_to_lightning":
            executable_states.add(SwapState.PFTL_LOCK_SUBMITTED.value)
        can_execute = authorized and swap["state"] in executable_states
        now = self._now()
        expired_reasons = self._execution_expiry_reasons(swap, authorization)
        if expired_reasons:
            can_execute = False
        incoming_expired_unconfirmed = (
            swap["direction"] == "lightning_to_pftl"
            and swap["state"]
            in {
                SwapState.PFTL_LOCK_FINAL.value,
                SwapState.LN_IN_FLIGHT.value,
            }
            and quote["invoice_expiry_unix"] <= now
        )
        if incoming_expired_unconfirmed:
            can_execute = False
        hold_reasons: list[str] = []
        if not authorized:
            hold_reasons.append("nazgul_value_authorization_required")
        if incoming_expired_unconfirmed:
            hold_reasons.append(
                "incoming_invoice_expired_terminal_status_unconfirmed"
            )
        hold_reasons.extend(expired_reasons)
        if not can_execute and not expired_reasons and not incoming_expired_unconfirmed:
            hold_reasons.append("swap_state_not_executable")
        wallet_address = (
            quote["pftl_recipient"]
            if swap["direction"] == "lightning_to_pftl"
            else quote["pftl_owner"]
        )
        response: dict[str, Any] = {
            "schema": "postfiat.lightning_navcoin.swap.v1",
            "swap_id": swap_id,
            "state": swap["state"],
            "direction": swap["direction"],
            "payment_hash": swap["payment_hash"],
            "invoice_amount_msat": quote["invoice_amount_msat"],
            "wallet_address": wallet_address,
            "pftl_amount_atoms": quote["pftl_amount_atoms"],
            "can_execute": can_execute,
            "hold_reasons": hold_reasons,
            "trust_class": self.policy.trust_class,
            "atomicity_claim": self.policy.atomicity_claim,
            "pftl": {
                "chain_id": self.policy.pftl_chain_id,
                "genesis_hash": self.policy.pftl_genesis_hash,
                "asset_id": self.policy.pftl_asset_id,
                "asset_precision": self.policy.pftl_asset_precision,
                "nav_epoch": self.policy.pftl_nav_epoch,
                "nav_reserve_packet_hash": self.policy.pftl_nav_reserve_packet_hash,
                "expected_escrow_id": quote["expected_escrow_id"],
                "create_operation": (
                    None if create_effect is None else create_effect["payload"]["operation"]
                ),
            },
        }
        lock_events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.PFTL_LOCK_FINAL.value
        ]
        if len(lock_events) > 1:
            raise RuntimeError("swap has duplicate PFTL lock finality events")
        if lock_events:
            lock_evidence = lock_events[0]["evidence"]
            observed = lock_evidence.get("agreeing_validator_count")
            validator_count = lock_evidence.get("validator_count")
            accepted = lock_evidence.get("accepted")
            code = lock_evidence.get("code")
            tx_id = lock_evidence.get("tx_id")
            escrow = lock_evidence.get("escrow")
            if (
                observed != 6
                or validator_count != 6
                or accepted is not True
                or code != "accepted"
                or type(tx_id) is not str
                or not isinstance(escrow, Mapping)
                or escrow.get("state") != "open"
            ):
                raise RuntimeError(
                    "durable PFTL lock finality evidence is incomplete"
                )
            response["pftl"].update(
                {
                    "height": lock_evidence["height"],
                    "state_root": lock_evidence["state_root"],
                    "block_tip_hash": lock_evidence["block_tip_hash"],
                    "quorum": {
                        "observed": 6,
                        "required": 6,
                        "validator_count": 6,
                        "converged": True,
                    },
                    "receipt": {
                        "tx_id": tx_id,
                        "accepted": True,
                        "code": "accepted",
                    },
                    "escrow": dict(escrow),
                    "recipient_asset_headroom": lock_evidence.get(
                        "recipient_asset_headroom"
                    ),
                    "recipient_native_balance": lock_evidence.get(
                        "recipient_native_balance"
                    ),
                    "finish_minimum_fee": lock_evidence.get(
                        "finish_minimum_fee"
                    ),
                    "account_reserve": lock_evidence.get("account_reserve"),
                }
            )
        terminal_events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"]
            in {
                SwapState.PFTL_FINISH_FINAL.value,
                SwapState.PFTL_CANCEL_FINAL.value,
            }
        ]
        if len(terminal_events) > 1:
            raise RuntimeError("swap has duplicate terminal PFTL finality events")
        if terminal_events:
            terminal_event = terminal_events[0]
            evidence = terminal_event["evidence"]
            if (
                evidence.get("accepted") is not True
                or evidence.get("code") != "accepted"
                or evidence.get("agreeing_validator_count") != 6
                or evidence.get("validator_count") != 6
            ):
                raise RuntimeError("terminal PFTL finality evidence is incomplete")
            terminal_state = (
                "canceled"
                if terminal_event["to_state"] == SwapState.PFTL_CANCEL_FINAL.value
                else "finished"
            )
            response["pftl"].update(
                {
                    "height": evidence["height"],
                    "state_root": evidence["state_root"],
                    "block_tip_hash": evidence["block_tip_hash"],
                    "quorum": {
                        "observed": 6,
                        "required": 6,
                        "validator_count": 6,
                        "converged": True,
                    },
                    "receipt": {
                        "tx_id": evidence["tx_id"],
                        "accepted": True,
                        "code": "accepted",
                    },
                    "escrow": {
                        **dict(response["pftl"].get("escrow", {})),
                        "state": terminal_state,
                    },
                }
            )
            if type(evidence.get("wallet_balance_atoms")) is int:
                response["pftl"]["wallet_balance_atoms"] = evidence[
                    "wallet_balance_atoms"
                ]
        onramp_lock_final = swap["state"] in {
            SwapState.PFTL_LOCK_FINAL.value,
            SwapState.LN_IN_FLIGHT.value,
            SwapState.LN_SETTLED.value,
        }
        if (
            swap["direction"] == "pftl_to_lightning"
            or (authorized and onramp_lock_final)
        ):
            response["signed_quote"] = swap["signed_quote"]
            response["lightning"] = {
                "invoice": quote["invoice"],
                "payment_hash": quote["payment_hash"],
                "amount_msat": quote["invoice_amount_msat"],
                "invoice_expiry_unix": quote["invoice_expiry_unix"],
                "min_final_cltv_delta": quote["min_final_cltv_delta"],
                "max_total_cltv_delta": quote["max_total_cltv_delta"],
            }
        return response

    @_serialize_swap
    def authorize_swap(
        self, swap_id: str, authorization_envelope: Mapping[str, Any]
    ) -> Mapping[str, Any]:
        with self._quote_lock:
            return self._authorize_swap_locked(swap_id, authorization_envelope)

    def _authorize_swap_locked(
        self, swap_id: str, authorization_envelope: Mapping[str, Any]
    ) -> Mapping[str, Any]:
        swap = self.journal.get_swap(swap_id)
        if swap["direction"] == "lightning_to_pftl":
            if swap["state"] not in {
                SwapState.PFTL_LOCK_SUBMITTED.value,
                SwapState.PFTL_LOCK_FINAL.value,
            }:
                raise RuntimeError(
                    "on-ramp authorization requires a durable PFTL lock intent"
                )
        elif swap["state"] not in {
            SwapState.PFTL_LOCK_SUBMITTED.value,
            SwapState.PFTL_LOCK_FINAL.value,
        }:
            raise RuntimeError(
                "off-ramp authorization requires a durable PFTL lock intent"
            )
        view = validate_mainnet_quote(
            swap["signed_quote"],
            self.policy,
            self.price,
            now_unix=self._now(),
        )
        authorization = verify_value_authorization(
            authorization_envelope,
            self.policy,
            quote=view,
            now_unix=self._now(),
        )
        if (
            swap["direction"] == "lightning_to_pftl"
            and authorization.expires_unix
            < swap["signed_quote"]["quote"]["invoice_expiry_unix"]
        ):
            raise RuntimeError(
                "on-ramp value authorization must cover the full BOLT11 expiry"
            )
        self.budget.reserve(
            authorization_envelope, quote=view, now_unix=self._now()
        )
        if (
            swap["direction"] == "lightning_to_pftl"
            and swap["state"] == SwapState.PFTL_LOCK_SUBMITTED.value
        ):
            return self._submit_onramp_lock(swap_id)
        return self.public_swap(swap_id)

    @_serialize_swap
    @_serialize_route_value
    def observe_user_lock(self, swap_id: str, tx_id: str) -> Mapping[str, Any]:
        """Advance an off-ramp only after six validators report literal ACCEPTED."""

        swap = self.journal.get_swap(swap_id)
        if swap["direction"] != "pftl_to_lightning":
            raise RuntimeError("user lock observation applies only to off-ramp")
        if swap["state"] != SwapState.PFTL_LOCK_SUBMITTED.value:
            final_events = [
                event
                for event in self.journal.events(swap_id)
                if event["to_state"] == SwapState.PFTL_LOCK_FINAL.value
            ]
            if (
                len(final_events) == 1
                and final_events[0]["evidence"].get("tx_id") == tx_id
            ):
                return self.public_swap(swap_id)
            raise RuntimeError("swap is not awaiting this user PFTL lock")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None or authorization["state"] != "RESERVED":
            raise RuntimeError("user PFTL lock lacks reserved value authorization")
        receipt = self.pftl_observer.receipt(tx_id)
        if (
            receipt["accepted"] is not True
            or receipt["code"] != "accepted"
            or receipt["agreeing_validator_count"] != 6
            or receipt["validator_count"] != 6
        ):
            raise RuntimeError("user PFTL lock is not literally accepted")
        quote = swap["signed_quote"]["quote"]
        expected = {
            "owner": quote["pftl_owner"],
            "recipient": quote["pftl_recipient"],
            "asset_id": quote["pftl_asset_id"],
            "amount": quote["pftl_amount_atoms"],
            "condition_hash": escrow_condition_hash(quote["condition"]),
            "finish_after": quote["finish_after"],
            "cancel_after": quote["cancel_after"],
        }
        lock = self.pftl_observer.open_escrow(
            quote["expected_escrow_id"], expected=expected
        )
        route_after_lock = self.pftl_observer.route_snapshot()
        if (
            route_after_lock.height < lock["height"]
            or route_after_lock.state_root != lock["state_root"]
            or route_after_lock.block_tip_hash != lock["block_tip_hash"]
            or receipt["state_root"] != lock["state_root"]
            or receipt["block_tip_hash"] != lock["block_tip_hash"]
        ):
            raise RuntimeError(
                "user PFTL lock route view is not tied to its accepted receipt"
            )
        effect = self._swap_side_effect(swap_id, "PFTL_ESCROW_CREATE")
        if effect is None:
            raise RuntimeError("durable PFTL create intent is absent")
        if effect["status"] == "PENDING":
            self.journal.record_side_effect_attempt(
                effect["effect_key"],
                f"{effect['effect_key']}:accepted:{tx_id}",
                "SUCCEEDED",
                result={
                    "tx_id": tx_id,
                    "accepted": True,
                    "code": receipt["code"],
                    "state_root": lock["state_root"],
                    "agreeing_validator_count": 6,
                },
            )
        elif effect["status"] != "SUCCEEDED":
            raise RuntimeError("durable user PFTL lock intent is terminally failed")
        self.service.mark_lock_final(
            swap_id,
            finality_evidence={
                "tx_id": tx_id,
                "accepted": True,
                "code": receipt["code"],
                "height": lock["height"],
                "state_root": lock["state_root"],
                "block_tip_hash": lock["block_tip_hash"],
                "agreeing_validator_count": 6,
                "validator_count": 6,
                "escrow": dict(lock["escrow"]),
                "route_after": route_after_lock.to_dict(),
            },
        )
        expiry_reasons = self._execution_expiry_reasons(
            self.journal.get_swap(swap_id),
            authorization,
        )
        if expiry_reasons:
            return self._mark_offramp_not_started_expired(
                swap_id, reasons=expiry_reasons
            )
        return self.public_swap(swap_id)

    @_serialize_swap
    @_serialize_route_value
    def observe_user_finish(
        self,
        swap_id: str,
        tx_id: str,
    ) -> Mapping[str, Any]:
        """Finalize an on-ramp only from six-view receipt and terminal state."""

        swap = self.journal.get_swap(swap_id)
        if swap["direction"] != "lightning_to_pftl":
            raise RuntimeError("user finish observation applies only to on-ramp")
        if swap["state"] == SwapState.PFTL_FINISH_FINAL.value:
            final_events = [
                event
                for event in self.journal.events(swap_id)
                if event["to_state"] == SwapState.PFTL_FINISH_FINAL.value
            ]
            if (
                len(final_events) == 1
                and final_events[0]["evidence"].get("tx_id") == tx_id
            ):
                return self.public_swap(swap_id)
            raise RuntimeError("on-ramp was finalized by a different PFTL finish")
        if swap["state"] != SwapState.LN_SETTLED.value:
            raise RuntimeError("on-ramp Lightning invoice is not durably settled")
        # Close a possible crash between LN_SETTLED and the budget charge before
        # accepting terminal PFTL evidence.
        self.refresh_onramp(swap_id)
        receipt = self.pftl_observer.receipt(tx_id)
        if (
            receipt["accepted"] is not True
            or receipt["code"] != "accepted"
            or receipt["agreeing_validator_count"] != 6
            or receipt["validator_count"] != 6
        ):
            raise RuntimeError("user PFTL finish is not literally six-of-six accepted")
        quote = swap["signed_quote"]["quote"]
        expected = {
            "owner": quote["pftl_owner"],
            "recipient": quote["pftl_recipient"],
            "asset_id": quote["pftl_asset_id"],
            "amount": quote["pftl_amount_atoms"],
            "condition_hash": escrow_condition_hash(quote["condition"]),
            "finish_after": quote["finish_after"],
            "cancel_after": quote["cancel_after"],
        }
        terminal = self.pftl_observer.finished_escrow(
            quote["expected_escrow_id"],
            expected=expected,
        )
        lock_events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.PFTL_LOCK_FINAL.value
        ]
        if len(lock_events) != 1:
            raise RuntimeError("on-ramp lock finality baseline is absent")
        route_before = lock_events[0]["evidence"].get("route_after")
        if not isinstance(route_before, Mapping):
            raise RuntimeError("on-ramp lock route baseline is absent")
        inventory_before = route_before.get("coordinator_inventory_atoms")
        wallet_before = route_before.get("user_balance_atoms")
        if (
            type(inventory_before) is not int
            or inventory_before < 0
            or type(wallet_before) is not int
            or wallet_before < 0
        ):
            raise RuntimeError("on-ramp lock route baseline is malformed")
        route_after = self.pftl_observer.route_snapshot()
        expected_wallet_after = wallet_before + quote["pftl_amount_atoms"]
        if (
            expected_wallet_after > (1 << 63) - 1
            or route_after.height < terminal["height"]
            or route_after.state_root != terminal["state_root"]
            or route_after.block_tip_hash != terminal["block_tip_hash"]
            or receipt["state_root"] != terminal["state_root"]
            or receipt["block_tip_hash"] != terminal["block_tip_hash"]
            or route_after.coordinator_inventory_atoms != inventory_before
            or route_after.user_balance_atoms != expected_wallet_after
        ):
            raise RuntimeError(
                "PFTL finish does not have the exact receipt-bound route delta"
            )
        self.service.mark_finish_final(
            swap_id,
            finality_evidence={
                "tx_id": tx_id,
                "accepted": True,
                "code": receipt["code"],
                "height": terminal["height"],
                "state_root": terminal["state_root"],
                "block_tip_hash": terminal["block_tip_hash"],
                "agreeing_validator_count": 6,
                "validator_count": 6,
                "escrow_state": "finished",
                "wallet_balance_atoms": route_after.user_balance_atoms,
                "route_before": dict(route_before),
                "route_after": route_after.to_dict(),
                "coordinator_inventory_delta_atoms": 0,
                "user_balance_delta_atoms": quote["pftl_amount_atoms"],
            },
        )
        return self.public_swap(swap_id)

    @_serialize_swap
    def refresh_onramp(self, swap_id: str) -> Mapping[str, Any]:
        """Observe incoming Lightning settlement; never obtains user PFTL keys."""

        swap = self.journal.get_swap(swap_id)
        if swap["direction"] != "lightning_to_pftl":
            raise RuntimeError("incoming invoice refresh applies only to on-ramp")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None or authorization["state"] not in {
            "RESERVED",
            "SPENT",
        }:
            raise RuntimeError("incoming invoice must not be published before authorization")
        quote = swap["signed_quote"]["quote"]
        if swap["state"] in {
            SwapState.PFTL_LOCK_FINAL.value,
            SwapState.LN_IN_FLIGHT.value,
        }:
            status = self.lnd.lookup_invoice(
                quote["payment_hash"],
                expected_amount_msat=quote["invoice_amount_msat"],
            )
            if status.settled:
                if swap["state"] == SwapState.PFTL_LOCK_FINAL.value:
                    self.service.mark_ln_in_flight(
                        swap_id,
                        payment_evidence={
                            "payment_hash": quote["payment_hash"],
                            "observation": "incoming_invoice_settled",
                        },
                    )
                self.service.mark_ln_settled(
                    swap_id,
                    settlement_evidence={
                        "payment_hash": quote["payment_hash"],
                        "status": "SETTLED",
                        "amount_paid_msat": status.amount_paid_msat,
                        "settle_index": status.settle_index,
                    },
                )
                swap = self.journal.get_swap(swap_id)
            elif status.terminal_unpaid:
                self.service.mark_refund_eligible(
                    swap_id,
                    reason_evidence={
                        "payment_hash": quote["payment_hash"],
                        "lightning_terminal_status": "CANCELED_UNPAID",
                        "amount_paid_msat": 0,
                        "settle_index": 0,
                        "pftl_principal_moved": False,
                    },
                    effect_key=f"{swap_id}:pftl-cancel",
                    cancel_operation={
                        "operation": "escrow_cancel",
                        "owner": quote["pftl_owner"],
                        "escrow_id": quote["expected_escrow_id"],
                        "cancel_after": quote["cancel_after"],
                    },
                )
                return self.recover_onramp_refund(swap_id)
        if swap["state"] == SwapState.LN_SETTLED.value:
            authorization = self.budget.authorization_for_swap(swap_id)
            if authorization is None:
                raise RuntimeError("settled on-ramp has no value authorization")
            if authorization["state"] == "RESERVED":
                self.budget.mark_spent(
                    authorization["authorization_id"],
                    terminal_evidence={
                        "value_moved": True,
                        "payment_hash": quote["payment_hash"],
                        "lightning_status": "SETTLED",
                        "pftl_escrow_id": quote["expected_escrow_id"],
                    },
                    now_unix=self._now(),
                )
            elif authorization["state"] != "SPENT":
                raise RuntimeError(
                    "settled on-ramp authorization is not chargeable"
                )
        return self.public_swap(swap_id)

    @_serialize_swap
    @_serialize_route_value
    def recover_onramp_refund(self, swap_id: str) -> Mapping[str, Any]:
        """Cancel an unpaid terminal invoice's PFTL lock at its refund bound."""

        swap = self.journal.get_swap(swap_id)
        if (
            swap["direction"] != "lightning_to_pftl"
            or swap["state"] != SwapState.REFUND_ELIGIBLE.value
        ):
            raise RuntimeError("on-ramp is not refund eligible")
        quote = swap["signed_quote"]["quote"]
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None or authorization["state"] != "RESERVED":
            raise RuntimeError("on-ramp refund lacks reserved value authorization")
        status = self.lnd.lookup_invoice(
            quote["payment_hash"],
            expected_amount_msat=quote["invoice_amount_msat"],
        )
        if not status.terminal_unpaid:
            raise RuntimeError(
                "incoming invoice is not conclusively canceled and unpaid"
            )
        route_before = self.pftl_observer.route_snapshot()
        if route_before.height < quote["cancel_after"]:
            return self.public_swap(swap_id)
        effect_key = f"{swap_id}:pftl-cancel"
        effect_row = self._swap_side_effect(swap_id, "PFTL_ESCROW_CANCEL")
        if effect_row is None or effect_row["effect_key"] != effect_key:
            raise RuntimeError("durable on-ramp cancel intent is absent")
        if effect_row["status"] == "FAILED_TERMINAL":
            raise RuntimeError("PFTL on-ramp cancel was terminally rejected")
        if effect_row["status"] not in {"PENDING", "SUCCEEDED"}:
            raise RuntimeError("PFTL on-ramp cancel effect status is invalid")
        expected = {
            "owner": quote["pftl_owner"],
            "recipient": quote["pftl_recipient"],
            "asset_id": quote["pftl_asset_id"],
            "amount": quote["pftl_amount_atoms"],
            "condition_hash": escrow_condition_hash(quote["condition"]),
            "finish_after": quote["finish_after"],
            "cancel_after": quote["cancel_after"],
        }
        checkpoint_key = f"{effect_key}:inventory-baseline"
        checkpoint = self.journal.side_effect_checkpoint(checkpoint_key)
        if checkpoint is None:
            # This observation and the cancel are route-serialized. Persist it
            # immediately before the first possible submission so another swap
            # cannot make a stale aggregate inventory delta appear exact.
            self.pftl_observer.open_escrow(
                quote["expected_escrow_id"], expected=expected
            )
            checkpoint = self.journal.record_side_effect_checkpoint(
                effect_key,
                checkpoint_key,
                evidence={
                    "inventory_before_cancel_atoms": (
                        route_before.coordinator_inventory_atoms
                    ),
                    "user_balance_before_cancel_atoms": (
                        route_before.user_balance_atoms
                    ),
                    "route_before": route_before.to_dict(),
                    "height": route_before.height,
                    "state_root": route_before.state_root,
                    "block_tip_hash": route_before.block_tip_hash,
                    "agreeing_validator_count": 6,
                    "validator_count": 6,
                },
            )
        inventory_before = checkpoint["evidence"].get(
            "inventory_before_cancel_atoms"
        )
        if type(inventory_before) is not int or inventory_before < 0:
            raise RuntimeError("durable pre-cancel inventory evidence is invalid")
        user_balance_before = checkpoint["evidence"].get(
            "user_balance_before_cancel_atoms"
        )
        route_baseline = checkpoint["evidence"].get("route_before")
        if (
            type(user_balance_before) is not int
            or user_balance_before < 0
            or not isinstance(route_baseline, Mapping)
        ):
            raise RuntimeError("durable pre-cancel route evidence is invalid")
        if (
            effect_row["status"] == "PENDING"
            and effect_row["attempt_count"] == 0
            and (
                route_before.coordinator_inventory_atoms != inventory_before
                or route_before.user_balance_atoms != user_balance_before
            )
        ):
            raise RuntimeError(
                "PFTL route changed after the durable cancel baseline"
            )
        try:
            canceled = self.pftl_backend.submit_cancel(
                owner=quote["pftl_owner"],
                escrow_id=quote["expected_escrow_id"],
                effect_key=effect_key,
            )
        except Exception as error:
            if effect_row["status"] == "PENDING":
                self.journal.record_side_effect_attempt(
                    effect_key,
                    f"{effect_key}:uncertain:{self._now()}",
                    "RETRYABLE_FAILURE",
                    result={"outcome": "uncertain"},
                )
            raise RuntimeError(
                "PFTL cancel outcome is uncertain; idempotent recovery required"
            ) from error
        if not canceled.accepted or canceled.code != "accepted":
            if canceled.mutation_free is not True:
                raise RuntimeError(
                    "PFTL cancel rejected without mutation-free evidence"
                )
            if effect_row["status"] == "PENDING":
                self.journal.record_side_effect_attempt(
                    effect_key,
                    f"{effect_key}:rejected:{canceled.tx_id}",
                    "RETRYABLE_FAILURE",
                    result=canceled.public_evidence(),
                )
            raise RuntimeError(
                f"PFTL cancel remains unfinalized: {canceled.code}"
            )
        if effect_row["status"] == "PENDING":
            self.journal.record_side_effect_attempt(
                effect_key,
                f"{effect_key}:accepted:{canceled.tx_id}",
                "SUCCEEDED",
                result=canceled.public_evidence(),
            )
        receipt = self.pftl_observer.receipt(canceled.tx_id)
        if (
            receipt["accepted"] is not True
            or receipt["code"] != "accepted"
            or receipt["agreeing_validator_count"] != 6
            or receipt["validator_count"] != 6
        ):
            raise RuntimeError("PFTL cancel lacks literal six-of-six ACCEPTED receipt")
        terminal = self.pftl_observer.canceled_escrow(
            quote["expected_escrow_id"], expected=expected
        )
        route_after = self.pftl_observer.route_snapshot()
        if route_after.height < terminal["height"]:
            raise RuntimeError("post-cancel inventory view predates finality")
        expected_inventory = inventory_before + quote["pftl_amount_atoms"]
        if (
            expected_inventory > (1 << 63) - 1
            or route_after.coordinator_inventory_atoms != expected_inventory
            or route_after.user_balance_atoms != user_balance_before
            or route_after.state_root != terminal["state_root"]
            or route_after.block_tip_hash != terminal["block_tip_hash"]
            or receipt["state_root"] != terminal["state_root"]
            or receipt["block_tip_hash"] != terminal["block_tip_hash"]
        ):
            raise RuntimeError(
                "PFTL cancel does not have the exact receipt-bound route delta"
            )
        self.service.mark_cancel_final(
            swap_id,
            finality_evidence={
                "tx_id": canceled.tx_id,
                "accepted": True,
                "code": "accepted",
                "height": terminal["height"],
                "state_root": terminal["state_root"],
                "block_tip_hash": terminal["block_tip_hash"],
                "agreeing_validator_count": 6,
                "validator_count": 6,
                "escrow_state": "canceled",
                "inventory_before_cancel_atoms": inventory_before,
                "inventory_after_cancel_atoms": (
                    route_after.coordinator_inventory_atoms
                ),
                "inventory_delta_atoms": quote["pftl_amount_atoms"],
                "wallet_balance_atoms": route_after.user_balance_atoms,
                "route_before": dict(route_baseline),
                "route_after": route_after.to_dict(),
                "coordinator_inventory_delta_atoms": (
                    quote["pftl_amount_atoms"]
                ),
                "user_balance_delta_atoms": 0,
            },
        )
        return self._release_refunded_onramp_budget(swap_id)

    def _release_refunded_onramp_budget(
        self, swap_id: str
    ) -> Mapping[str, Any]:
        """Release a no-BTC authorization only after exact cancel finality."""

        swap = self.journal.get_swap(swap_id)
        if (
            swap["direction"] != "lightning_to_pftl"
            or swap["state"] != SwapState.PFTL_CANCEL_FINAL.value
        ):
            raise RuntimeError("on-ramp refund is not terminal")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None:
            raise RuntimeError("refunded on-ramp has no value authorization")
        if authorization["state"] == "RELEASED":
            return self.public_swap(swap_id)
        if authorization["state"] != "RESERVED":
            raise RuntimeError("refunded on-ramp authorization is not releasable")
        events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.PFTL_CANCEL_FINAL.value
        ]
        if len(events) != 1:
            raise RuntimeError("PFTL cancel finality evidence is absent")
        evidence = events[0]["evidence"]
        quote = swap["signed_quote"]["quote"]
        if (
            evidence.get("accepted") is not True
            or evidence.get("code") != "accepted"
            or evidence.get("escrow_state") != "canceled"
            or evidence.get("inventory_delta_atoms")
            != quote["pftl_amount_atoms"]
        ):
            raise RuntimeError("PFTL cancel finality evidence is incomplete")
        self.budget.release_unspent(
            authorization["authorization_id"],
            no_value_evidence={
                "value_moved": False,
                "payment_hash": quote["payment_hash"],
                "lightning_status": "CANCELED_UNPAID",
                "amount_paid_msat": 0,
                "pftl_cancel_tx_id": evidence["tx_id"],
                "pftl_inventory_return_atoms": quote["pftl_amount_atoms"],
            },
            now_unix=self._now(),
        )
        return self.public_swap(swap_id)

    @_serialize_swap
    @_serialize_route_value
    def observe_user_cancel(
        self, swap_id: str, tx_id: str
    ) -> Mapping[str, Any]:
        """Observe the wallet-signed off-ramp refund; never signs for the user."""

        swap = self.journal.get_swap(swap_id)
        if swap["direction"] != "pftl_to_lightning":
            raise RuntimeError("user cancel observation applies only to off-ramp")
        if swap["state"] == SwapState.PFTL_CANCEL_FINAL.value:
            events = [
                event
                for event in self.journal.events(swap_id)
                if event["to_state"] == SwapState.PFTL_CANCEL_FINAL.value
            ]
            if len(events) == 1 and events[0]["evidence"].get("tx_id") == tx_id:
                return self._release_refunded_offramp_budget(swap_id)
            raise RuntimeError("off-ramp was canceled by a different PFTL transaction")
        if swap["state"] != SwapState.REFUND_ELIGIBLE.value:
            raise RuntimeError("off-ramp is not refund eligible")
        quote = swap["signed_quote"]["quote"]
        refund_events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.REFUND_ELIGIBLE.value
        ]
        if len(refund_events) != 1:
            raise RuntimeError("off-ramp refund eligibility evidence is absent")
        terminal_status = refund_events[0]["evidence"].get(
            "lightning_terminal_status"
        )
        lightning_effect = self._swap_side_effect(swap_id, "LND_SEND_PAYMENT")
        if terminal_status == "FAILED":
            reconciliation = self.lnd.track_payment(
                quote["payment_hash"],
                expected_amount_msat=quote["invoice_amount_msat"],
            )
            if reconciliation.status is not PaymentReconciliationStatus.FAILED:
                raise RuntimeError(
                    "Lightning payment is not durably terminal FAILED"
                )
            if lightning_effect is None:
                raise RuntimeError("failed Lightning payment has no durable intent")
        elif terminal_status == "NOT_STARTED_EXPIRED":
            if lightning_effect is not None:
                raise RuntimeError(
                    "expired no-start refund unexpectedly has a Lightning intent"
                )
        else:
            raise RuntimeError("off-ramp refund reason is not terminal")

        effect_key = f"{swap_id}:user-pftl-cancel"
        cancel_effect = self._swap_side_effect(swap_id, "PFTL_ESCROW_CANCEL")
        if cancel_effect is None or cancel_effect["effect_key"] != effect_key:
            raise RuntimeError("durable user PFTL cancel intent is absent")
        receipt = self.pftl_observer.receipt(tx_id)
        if (
            receipt["accepted"] is not True
            or receipt["code"] != "accepted"
            or receipt["agreeing_validator_count"] != 6
            or receipt["validator_count"] != 6
        ):
            raise RuntimeError(
                "user PFTL cancel is not literally six-of-six accepted"
            )
        expected = {
            "owner": quote["pftl_owner"],
            "recipient": quote["pftl_recipient"],
            "asset_id": quote["pftl_asset_id"],
            "amount": quote["pftl_amount_atoms"],
            "condition_hash": escrow_condition_hash(quote["condition"]),
            "finish_after": quote["finish_after"],
            "cancel_after": quote["cancel_after"],
        }
        terminal = self.pftl_observer.canceled_escrow(
            quote["expected_escrow_id"], expected=expected
        )
        lock_events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.PFTL_LOCK_FINAL.value
        ]
        if len(lock_events) != 1:
            raise RuntimeError("off-ramp lock finality baseline is absent")
        route_before = lock_events[0]["evidence"].get("route_after")
        if not isinstance(route_before, Mapping):
            raise RuntimeError("off-ramp lock route baseline is absent")
        inventory_before = route_before.get("coordinator_inventory_atoms")
        wallet_before = route_before.get("user_balance_atoms")
        if (
            type(inventory_before) is not int
            or inventory_before < 0
            or type(wallet_before) is not int
            or wallet_before < 0
        ):
            raise RuntimeError("off-ramp lock route baseline is malformed")
        route_after = self.pftl_observer.route_snapshot()
        expected_wallet_after = wallet_before + quote["pftl_amount_atoms"]
        if (
            expected_wallet_after > (1 << 63) - 1
            or route_after.height < terminal["height"]
            or route_after.state_root != terminal["state_root"]
            or route_after.block_tip_hash != terminal["block_tip_hash"]
            or receipt["state_root"] != terminal["state_root"]
            or receipt["block_tip_hash"] != terminal["block_tip_hash"]
            or route_after.coordinator_inventory_atoms != inventory_before
            or route_after.user_balance_atoms != expected_wallet_after
        ):
            raise RuntimeError(
                "user PFTL cancel does not have the exact receipt-bound route delta"
            )
        if cancel_effect["status"] == "PENDING":
            self.journal.record_side_effect_attempt(
                effect_key,
                f"{effect_key}:accepted:{tx_id}",
                "SUCCEEDED",
                result={
                    "tx_id": tx_id,
                    "accepted": True,
                    "code": "accepted",
                    "state_root": terminal["state_root"],
                    "agreeing_validator_count": 6,
                    "validator_count": 6,
                },
            )
        elif cancel_effect["status"] != "SUCCEEDED":
            raise RuntimeError("durable user cancel intent is terminally failed")
        self.service.mark_cancel_final(
            swap_id,
            finality_evidence={
                "tx_id": tx_id,
                "accepted": True,
                "code": "accepted",
                "height": terminal["height"],
                "state_root": terminal["state_root"],
                "block_tip_hash": terminal["block_tip_hash"],
                "agreeing_validator_count": 6,
                "validator_count": 6,
                "escrow_state": "canceled",
                "lightning_terminal_status": terminal_status,
                "wallet_balance_atoms": route_after.user_balance_atoms,
                "route_before": dict(route_before),
                "route_after": route_after.to_dict(),
                "coordinator_inventory_delta_atoms": 0,
                "user_balance_delta_atoms": quote["pftl_amount_atoms"],
            },
        )
        return self._release_refunded_offramp_budget(swap_id)

    def _release_refunded_offramp_budget(
        self, swap_id: str
    ) -> Mapping[str, Any]:
        swap = self.journal.get_swap(swap_id)
        if (
            swap["direction"] != "pftl_to_lightning"
            or swap["state"] != SwapState.PFTL_CANCEL_FINAL.value
        ):
            raise RuntimeError("off-ramp refund is not terminal")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None:
            raise RuntimeError("refunded off-ramp has no value authorization")
        if authorization["state"] == "RELEASED":
            return self.public_swap(swap_id)
        if authorization["state"] != "RESERVED":
            raise RuntimeError("refunded off-ramp authorization is not releasable")
        events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.PFTL_CANCEL_FINAL.value
        ]
        if len(events) != 1:
            raise RuntimeError("off-ramp cancel finality evidence is absent")
        evidence = events[0]["evidence"]
        if (
            evidence.get("accepted") is not True
            or evidence.get("code") != "accepted"
            or evidence.get("escrow_state") != "canceled"
            or evidence.get("lightning_terminal_status")
            not in {"FAILED", "NOT_STARTED_EXPIRED"}
        ):
            raise RuntimeError("off-ramp cancel finality evidence is incomplete")
        quote = swap["signed_quote"]["quote"]
        self.budget.release_unspent(
            authorization["authorization_id"],
            no_value_evidence={
                "value_moved": False,
                "payment_hash": quote["payment_hash"],
                "lightning_status": evidence["lightning_terminal_status"],
                "pftl_cancel_tx_id": evidence["tx_id"],
                "user_principal_return_atoms": quote["pftl_amount_atoms"],
            },
            now_unix=self._now(),
        )
        return self.public_swap(swap_id)

    @_serialize_swap
    @_serialize_route_value
    def execute_offramp(self, swap_id: str) -> Mapping[str, Any]:
        """Pay only after PFTL lock finality and a durable one-use value permit."""

        swap = self.journal.get_swap(swap_id)
        if swap["direction"] != "pftl_to_lightning":
            raise RuntimeError("outgoing payment applies only to off-ramp")
        if swap["state"] == SwapState.REFUND_ELIGIBLE.value:
            return self.recover_swap(swap_id)
        authorization = self.budget.authorization_for_swap(swap_id)
        if swap["state"] == SwapState.PFTL_FINISH_FINAL.value:
            return self._mark_offramp_budget_spent(swap_id)
        if authorization is None or authorization["state"] != "RESERVED":
            raise RuntimeError("off-ramp lacks a reserved value authorization")
        if swap["state"] == SwapState.LN_SETTLED.value:
            return self.recover_offramp_finish(swap_id)
        if swap["state"] == SwapState.LN_IN_FLIGHT.value:
            return self.reconcile_offramp(swap_id)
        if swap["state"] != SwapState.PFTL_LOCK_FINAL.value:
            raise RuntimeError("off-ramp is not ready for a Lightning payment")
        quote = swap["signed_quote"]["quote"]
        expiry_reasons = self._execution_expiry_reasons(swap, authorization)
        if expiry_reasons:
            return self._mark_offramp_not_started_expired(
                swap_id, reasons=expiry_reasons
            )
        self._revalidate_offramp_start(swap, authorization)
        effect_key = f"{swap_id}:ln-payment"
        self.service.mark_ln_in_flight(
            swap_id,
            payment_evidence={
                "payment_hash": quote["payment_hash"],
                "intent_durable": True,
            },
            effect_key=effect_key,
            payment_request={
                "invoice": quote["invoice"],
                "fee_limit_msat": authorization["max_fee_msat"],
                "max_total_cltv_delta": quote["max_total_cltv_delta"],
            },
        )
        try:
            settled = self.lnd.send_payment(
                quote["invoice"],
                fee_limit_msat=authorization["max_fee_msat"],
                max_total_cltv_delta=quote["max_total_cltv_delta"],
                timeout_seconds=60,
            )
        except LightningPaymentError as error:
            self.service.mark_refund_eligible(
                swap_id,
                reason_evidence={
                    "payment_hash": quote["payment_hash"],
                    "lightning_terminal_status": "FAILED",
                    "pftl_principal_moved": False,
                },
                effect_key=f"{swap_id}:user-pftl-cancel",
                cancel_operation={
                    "operation": "escrow_cancel",
                    "owner": quote["pftl_owner"],
                    "escrow_id": quote["expected_escrow_id"],
                    "cancel_after": quote["cancel_after"],
                    "requires_user_signature": True,
                },
            )
            self._finalize_failed_offramp(swap_id)
            raise RuntimeError("Lightning payment failed terminally; user refund remains") from error
        except LndGrpcError as error:
            self.journal.record_side_effect_attempt(
                effect_key,
                f"{effect_key}:uncertain:{self._now()}",
                "RETRYABLE_FAILURE",
                result={
                    "payment_hash": quote["payment_hash"],
                    "status": "UNCERTAIN",
                },
            )
            raise RuntimeError(
                "Lightning outcome is uncertain; TrackPaymentV2 reconciliation required"
            ) from error
        self._validate_settled_payment(
            settled,
            expected_amount_msat=quote["invoice_amount_msat"],
            maximum_fee_msat=authorization["max_fee_msat"],
        )
        self.journal.record_side_effect_attempt(
            effect_key,
            f"{effect_key}:settled:{quote['payment_hash']}",
            "SUCCEEDED",
            result={
                "payment_hash": quote["payment_hash"],
                "status": "SETTLED",
                "fee_sat": settled.fee_sat,
                "fee_msat": settled.fee_msat,
                "amount_msat": settled.amount_msat,
            },
        )
        return self._finish_offramp(swap_id, settled)

    def _revalidate_offramp_start(
        self,
        swap: Mapping[str, Any],
        authorization: Mapping[str, Any],
    ) -> None:
        """Re-read every mutable capacity/timelock gate before SendPaymentV2."""

        now = self._now()
        quote = swap["signed_quote"]["quote"]
        validate_mainnet_quote(
            swap["signed_quote"],
            self.policy,
            self.price,
            now_unix=now,
        )
        if authorization["expires_unix"] <= now:
            raise RuntimeError("off-ramp value authorization expired before payment")
        all_in_msat = (
            quote["invoice_amount_msat"] + authorization["max_fee_msat"]
        )
        self.lnd.preflight_node(
            expected_identity_pubkey=self.policy.expected_lnd_pubkey,
            min_active_channels=1,
            min_inbound_msat=0,
            min_outbound_msat=all_in_msat,
        )
        route = self.pftl_observer.route_snapshot()
        if (
            route.coordinator_receive_headroom_atoms
            < quote["pftl_amount_atoms"]
        ):
            raise RuntimeError("coordinator NAVcoin receive headroom is insufficient")
        expected = {
            "owner": quote["pftl_owner"],
            "recipient": quote["pftl_recipient"],
            "asset_id": quote["pftl_asset_id"],
            "amount": quote["pftl_amount_atoms"],
            "condition_hash": escrow_condition_hash(quote["condition"]),
            "finish_after": quote["finish_after"],
            "cancel_after": quote["cancel_after"],
        }
        lock = self.pftl_observer.open_escrow(
            quote["expected_escrow_id"], expected=expected
        )
        remaining = quote["cancel_after"] - lock["height"]
        if remaining < PFTL_REQUIRED_FINISH_MARGIN_BLOCKS:
            raise RuntimeError(
                "PFTL refund boundary lacks the required Lightning CLTV margin"
            )

    @staticmethod
    def _validate_settled_payment(
        settled: Any,
        *,
        expected_amount_msat: int,
        maximum_fee_msat: int,
    ) -> None:
        """Independently enforce the exact principal and authorized fee ceiling."""

        if settled.amount_msat != expected_amount_msat:
            raise RuntimeError(
                "settled Lightning payment amount is absent or mismatched"
            )
        if (
            type(settled.fee_msat) is not int
            or settled.fee_msat < 0
            or settled.fee_msat > (1 << 63) - 1
        ):
            raise RuntimeError(
                "settled Lightning payment exact fee_msat is absent or invalid"
            )
        if (
            settled.fee_sat is not None
            and (
                type(settled.fee_sat) is not int
                or settled.fee_sat < 0
                or settled.fee_sat != settled.fee_msat // 1000
            )
        ):
            raise RuntimeError("settled Lightning fee fields are inconsistent")
        if settled.fee_msat > maximum_fee_msat:
            raise RuntimeError("settled Lightning payment exceeded authorized fee cap")

    def _finish_offramp(
        self, swap_id: str, settled: Any
    ) -> Mapping[str, Any]:
        swap = self.journal.get_swap(swap_id)
        quote = swap["signed_quote"]["quote"]
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None or authorization["state"] != "RESERVED":
            raise RuntimeError("off-ramp finish lacks reserved value authorization")
        self._validate_settled_payment(
            settled,
            expected_amount_msat=quote["invoice_amount_msat"],
            maximum_fee_msat=authorization["max_fee_msat"],
        )
        finish_effect_key = f"{swap_id}:pftl-finish"
        self.service.mark_ln_settled(
            swap_id,
            settlement_evidence={
                "payment_hash": quote["payment_hash"],
                "status": "SETTLED",
                "fee_sat": settled.fee_sat,
                "fee_msat": settled.fee_msat,
                "payer_htlc_expiries": list(settled.payer_htlc_expiries),
            },
            learned_secret=settled.payment_preimage,
            effect_key=finish_effect_key,
            finish_operation={
                "operation": "escrow_finish",
                "owner": quote["pftl_owner"],
                "recipient": quote["pftl_recipient"],
                "escrow_id": quote["expected_escrow_id"],
                "payment_hash": quote["payment_hash"],
            },
        )
        return self.recover_offramp_finish(swap_id)

    @_serialize_swap
    @_serialize_route_value
    def recover_offramp_finish(self, swap_id: str) -> Mapping[str, Any]:
        """Idempotently finish PFTL after a durably recorded LN settlement."""

        swap = self.journal.get_swap(swap_id)
        if swap["direction"] != "pftl_to_lightning":
            raise RuntimeError("PFTL finish recovery applies only to off-ramp")
        if swap["state"] == SwapState.PFTL_FINISH_FINAL.value:
            return self._mark_offramp_budget_spent(swap_id)
        if swap["state"] != SwapState.LN_SETTLED.value:
            raise RuntimeError("off-ramp has no durable Lightning settlement")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None or authorization["state"] != "RESERVED":
            raise RuntimeError("off-ramp finish lacks reserved value authorization")
        quote = swap["signed_quote"]["quote"]
        secret = self.journal.load_secret(swap_id, "invoice_preimage")
        finish_effect_key = f"{swap_id}:pftl-finish"
        effect_row = self._swap_side_effect(swap_id, "PFTL_ESCROW_FINISH")
        if effect_row is None:
            raise RuntimeError("durable PFTL finish intent is absent")
        if effect_row["effect_key"] != finish_effect_key:
            raise RuntimeError("durable PFTL finish effect key is inconsistent")
        if effect_row["status"] == "FAILED_TERMINAL":
            raise RuntimeError("PFTL finish was terminally rejected after LN settlement")
        if effect_row["status"] not in {"PENDING", "SUCCEEDED"}:
            raise RuntimeError("PFTL finish effect has an unknown durable status")
        expected = {
            "owner": quote["pftl_owner"],
            "recipient": quote["pftl_recipient"],
            "asset_id": quote["pftl_asset_id"],
            "amount": quote["pftl_amount_atoms"],
            "condition_hash": escrow_condition_hash(quote["condition"]),
            "finish_after": quote["finish_after"],
            "cancel_after": quote["cancel_after"],
        }
        route_before_now = self.pftl_observer.route_snapshot()
        checkpoint_key = f"{finish_effect_key}:route-baseline"
        checkpoint = self.journal.side_effect_checkpoint(checkpoint_key)
        if checkpoint is None:
            if (
                effect_row["status"] != "PENDING"
                or effect_row["attempt_count"] != 0
            ):
                raise RuntimeError(
                    "durable PFTL finish route baseline is absent after submission"
                )
            open_escrow = self.pftl_observer.open_escrow(
                quote["expected_escrow_id"], expected=expected
            )
            if (
                route_before_now.height < open_escrow["height"]
                or route_before_now.state_root != open_escrow["state_root"]
                or route_before_now.block_tip_hash
                != open_escrow["block_tip_hash"]
            ):
                raise RuntimeError(
                    "pre-finish route view is not tied to the open escrow"
                )
            checkpoint = self.journal.record_side_effect_checkpoint(
                finish_effect_key,
                checkpoint_key,
                evidence={
                    "route_before": route_before_now.to_dict(),
                    "coordinator_inventory_before_atoms": (
                        route_before_now.coordinator_inventory_atoms
                    ),
                    "user_balance_before_atoms": (
                        route_before_now.user_balance_atoms
                    ),
                    "agreeing_validator_count": 6,
                    "validator_count": 6,
                },
            )
        route_before = checkpoint["evidence"].get("route_before")
        inventory_before = checkpoint["evidence"].get(
            "coordinator_inventory_before_atoms"
        )
        wallet_before = checkpoint["evidence"].get(
            "user_balance_before_atoms"
        )
        if (
            not isinstance(route_before, Mapping)
            or type(inventory_before) is not int
            or inventory_before < 0
            or type(wallet_before) is not int
            or wallet_before < 0
        ):
            raise RuntimeError("durable pre-finish route evidence is malformed")
        if (
            effect_row["status"] == "PENDING"
            and effect_row["attempt_count"] == 0
            and (
                route_before_now.coordinator_inventory_atoms
                != inventory_before
                or route_before_now.user_balance_atoms != wallet_before
            )
        ):
            raise RuntimeError("PFTL route changed after the finish baseline")
        try:
            finish = self.pftl_backend.submit_finish(
                owner=quote["pftl_owner"],
                recipient=quote["pftl_recipient"],
                escrow_id=quote["expected_escrow_id"],
                secret=secret,
                effect_key=finish_effect_key,
            )
        except Exception as error:
            if effect_row["status"] == "PENDING":
                self.journal.record_side_effect_attempt(
                    finish_effect_key,
                    f"{finish_effect_key}:uncertain:{self._now()}",
                    "RETRYABLE_FAILURE",
                    result={
                        "payment_hash": quote["payment_hash"],
                        "status": "UNCERTAIN",
                    },
                )
            raise RuntimeError(
                "PFTL finish outcome is uncertain; idempotent recovery required"
            ) from error
        if not finish.accepted or finish.code != "accepted":
            if effect_row["status"] == "PENDING":
                self.journal.record_side_effect_attempt(
                    finish_effect_key,
                    f"{finish_effect_key}:rejected:{finish.tx_id}",
                    "TERMINAL_FAILURE",
                    result=finish.public_evidence(),
                )
            raise RuntimeError(
                "PFTL finish failed after Lightning settlement; "
                "terminal manual recovery required"
            )
        if effect_row["status"] == "PENDING":
            self.journal.record_side_effect_attempt(
                finish_effect_key,
                f"{finish_effect_key}:accepted:{finish.tx_id}",
                "SUCCEEDED",
                result=finish.public_evidence(),
            )
        receipt = self.pftl_observer.receipt(finish.tx_id)
        if (
            receipt["accepted"] is not True
            or receipt["code"] != "accepted"
            or receipt["agreeing_validator_count"] != 6
            or receipt["validator_count"] != 6
        ):
            raise RuntimeError("PFTL finish lacks literal six-of-six ACCEPTED receipt")
        terminal = self.pftl_observer.finished_escrow(
            quote["expected_escrow_id"], expected=expected
        )
        route_after = self.pftl_observer.route_snapshot()
        expected_inventory_after = (
            inventory_before + quote["pftl_amount_atoms"]
        )
        if (
            expected_inventory_after > (1 << 63) - 1
            or route_after.height < terminal["height"]
            or route_after.state_root != terminal["state_root"]
            or route_after.block_tip_hash != terminal["block_tip_hash"]
            or receipt["state_root"] != terminal["state_root"]
            or receipt["block_tip_hash"] != terminal["block_tip_hash"]
            or route_after.coordinator_inventory_atoms
            != expected_inventory_after
            or route_after.user_balance_atoms != wallet_before
        ):
            raise RuntimeError(
                "PFTL finish does not have the exact receipt-bound route delta"
            )
        self.service.mark_finish_final(
            swap_id,
            finality_evidence={
                "tx_id": finish.tx_id,
                "accepted": True,
                "code": "accepted",
                "height": receipt["height"],
                "state_root": receipt["state_root"],
                "block_tip_hash": receipt["block_tip_hash"],
                "agreeing_validator_count": 6,
                "validator_count": 6,
                "escrow_state": "finished",
                "wallet_balance_atoms": route_after.user_balance_atoms,
                "route_before": dict(route_before),
                "route_after": route_after.to_dict(),
                "coordinator_inventory_delta_atoms": (
                    quote["pftl_amount_atoms"]
                ),
                "user_balance_delta_atoms": 0,
            },
        )
        return self._mark_offramp_budget_spent(swap_id)

    def _mark_offramp_budget_spent(self, swap_id: str) -> Mapping[str, Any]:
        """Close the second durable journal after terminal PFTL finality."""

        swap = self.journal.get_swap(swap_id)
        if (
            swap["direction"] != "pftl_to_lightning"
            or swap["state"] != SwapState.PFTL_FINISH_FINAL.value
        ):
            raise RuntimeError("off-ramp value cannot be charged before PFTL finish")
        quote = swap["signed_quote"]["quote"]
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None:
            raise RuntimeError("terminal off-ramp has no value authorization")
        if authorization["state"] == "SPENT":
            return self.public_swap(swap_id)
        if authorization["state"] != "RESERVED":
            raise RuntimeError("terminal off-ramp authorization is not chargeable")
        finish_events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.PFTL_FINISH_FINAL.value
        ]
        if len(finish_events) != 1:
            raise RuntimeError("terminal off-ramp finality evidence is absent")
        evidence = finish_events[0]["evidence"]
        if (
            evidence.get("accepted") is not True
            or evidence.get("code") != "accepted"
            or evidence.get("escrow_state") != "finished"
            or evidence.get("coordinator_inventory_delta_atoms")
            != quote["pftl_amount_atoms"]
            or evidence.get("user_balance_delta_atoms") != 0
        ):
            raise RuntimeError(
                "terminal off-ramp exact route evidence is incomplete"
            )
        self.budget.mark_spent(
            authorization["authorization_id"],
            terminal_evidence={
                "value_moved": True,
                "payment_hash": quote["payment_hash"],
                "lightning_status": "SETTLED",
                "pftl_tx_id": evidence["tx_id"],
                "pftl_receipt_code": evidence["code"],
            },
            now_unix=self._now(),
        )
        return self.public_swap(swap_id)

    @_serialize_swap
    def reconcile_offramp(self, swap_id: str) -> Mapping[str, Any]:
        """Resolve an uncertain SendPaymentV2 solely through TrackPaymentV2."""

        swap = self.journal.get_swap(swap_id)
        if (
            swap["direction"] != "pftl_to_lightning"
            or swap["state"] != SwapState.LN_IN_FLIGHT.value
        ):
            raise RuntimeError("swap is not an uncertain off-ramp payment")
        quote = swap["signed_quote"]["quote"]
        result = self.lnd.track_payment(
            quote["payment_hash"],
            expected_amount_msat=quote["invoice_amount_msat"],
        )
        if result.status is PaymentReconciliationStatus.UNCERTAIN:
            raise RuntimeError("Lightning payment remains uncertain")
        if result.status is PaymentReconciliationStatus.FAILED:
            self.service.mark_refund_eligible(
                swap_id,
                reason_evidence={
                    "payment_hash": quote["payment_hash"],
                    "lightning_terminal_status": "FAILED",
                    "pftl_principal_moved": False,
                },
                effect_key=f"{swap_id}:user-pftl-cancel",
                cancel_operation={
                    "operation": "escrow_cancel",
                    "owner": quote["pftl_owner"],
                    "escrow_id": quote["expected_escrow_id"],
                    "cancel_after": quote["cancel_after"],
                    "requires_user_signature": True,
                },
            )
            self._finalize_failed_offramp(swap_id)
            raise RuntimeError(
                "Lightning payment terminally failed; PFTL refund becomes eligible at cancel_after"
            )
        if result.settled_payment is None:
            raise RuntimeError("settled reconciliation has no payment proof")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None or authorization["state"] != "RESERVED":
            raise RuntimeError("settled off-ramp lacks reserved value authorization")
        self._validate_settled_payment(
            result.settled_payment,
            expected_amount_msat=quote["invoice_amount_msat"],
            maximum_fee_msat=authorization["max_fee_msat"],
        )
        effect_key = f"{swap_id}:ln-payment"
        self.journal.record_side_effect_attempt(
            effect_key,
            f"{effect_key}:reconciled-settled:{quote['payment_hash']}",
            "SUCCEEDED",
            result={
                "payment_hash": quote["payment_hash"],
                "status": "SETTLED",
                "fee_sat": result.settled_payment.fee_sat,
                "fee_msat": result.settled_payment.fee_msat,
                "amount_msat": result.settled_payment.amount_msat,
            },
        )
        return self._finish_offramp(swap_id, result.settled_payment)

    def _finalize_failed_offramp(self, swap_id: str) -> Mapping[str, Any]:
        """Close a conclusively failed Lightning intent without charging value."""

        swap = self.journal.get_swap(swap_id)
        if (
            swap["direction"] != "pftl_to_lightning"
            or swap["state"] != SwapState.REFUND_ELIGIBLE.value
        ):
            raise RuntimeError("off-ramp is not terminally refund eligible")
        failure_events = [
            event
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.REFUND_ELIGIBLE.value
        ]
        if len(failure_events) != 1:
            raise RuntimeError("off-ramp failure evidence is absent")
        evidence = failure_events[0]["evidence"]
        if (
            evidence.get("lightning_terminal_status") != "FAILED"
            or evidence.get("pftl_principal_moved") is not False
        ):
            raise RuntimeError("off-ramp failure is not proven terminal and no-value")
        effect = self._swap_side_effect(swap_id, "LND_SEND_PAYMENT")
        if effect is None:
            raise RuntimeError("failed off-ramp has no durable Lightning intent")
        if effect["status"] == "PENDING":
            self.journal.record_side_effect_attempt(
                effect["effect_key"],
                f"{effect['effect_key']}:terminal-failed",
                "TERMINAL_FAILURE",
                result={
                    "payment_hash": swap["payment_hash"],
                    "status": "FAILED",
                },
            )
        elif effect["status"] != "FAILED_TERMINAL":
            raise RuntimeError("failed off-ramp Lightning effect is inconsistent")
        authorization = self.budget.authorization_for_swap(swap_id)
        if authorization is None or authorization["state"] != "RESERVED":
            raise RuntimeError("failed off-ramp has no value authorization")
        return self.public_swap(swap_id)
