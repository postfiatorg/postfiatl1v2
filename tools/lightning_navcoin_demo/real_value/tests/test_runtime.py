from __future__ import annotations

from dataclasses import replace
import tempfile
import threading
import unittest
from pathlib import Path

from ...coordinator.journal import CoordinatorJournal, ExposureLimits, SwapState
from ...coordinator.lnd_grpc import (
    CreatedInvoice,
    InvoiceStatus,
    LndInvoiceNotFound,
    LightningPaymentError,
    LndLiquiditySummary,
    LndNodeInfo,
    LndNodePreflight,
    PaymentReconciliation,
    PaymentReconciliationStatus,
    SettledPayment,
)
from ...coordinator.protocol import LndInvoiceFacts, SecretPreimage, payment_hash
from ..authorization import sign_value_authorization
from ..budget import RealValueBudget
from ..policy import validate_mainnet_quote
from ..pftl_quorum import PftlRouteSnapshot
from ..runtime import (
    MainnetCoordinatorRuntime,
    PftlEffect,
    PftlEscrowPlan,
    RuntimeError as CoordinatorRuntimeError,
)
from .common import AUTH_SIGNER, QUOTE_SIGNER, authorization_for, policy, price


NOW = 1_800_000_000
AMOUNT_MSAT = 100_000
USER = "pf" + "77" * 20
ESCROW_ID = "88" * 48
CREATE_TX = "91" * 48
FINISH_TX = "92" * 48
USER_FINISH_TX = "95" * 48
CANCEL_TX = "97" * 48
OFFRAMP_SECRET = SecretPreimage(bytes.fromhex("93" * 32))
REQUEST_ID = "cd" * 32


class FakeLnd:
    def __init__(self, route: object) -> None:
        self.route = route
        self.incoming_settled = False
        self.add_invoice_count = 0
        self.send_count = 0
        self.fail_payment = False
        self.preflight_calls: list[dict[str, object]] = []
        self.incoming_secret: SecretPreimage | None = None
        self.created_incoming: CreatedInvoice | None = None
        self.incoming_state = "OPEN"
        self.outgoing_facts = LndInvoiceFacts(
            payment_hash=payment_hash(OFFRAMP_SECRET),
            amount_msat=AMOUNT_MSAT,
            payee="03" + "99" * 32,
            timestamp_unix=NOW,
            expiry_seconds=900,
            min_final_cltv_delta=144,
            network="bitcoin",
            is_amp=False,
        )

    def preflight_node(self, **kwargs: object) -> LndNodePreflight:
        self.preflight_calls.append(dict(kwargs))
        return LndNodePreflight(
            node=LndNodeInfo(
                identity_pubkey=self.route.expected_lnd_pubkey,
                alias="runtime-test",
                network="bitcoin",
                block_height=900_000,
                synced_to_chain=True,
                synced_to_graph=True,
                version="0.20.1-beta commit=v0.20.1-beta",
                commit_hash="848b72ce96eb68fa90fd4336523ca4c59bddcd4c",
            ),
            liquidity=LndLiquiditySummary(
                total_channels=1,
                active_channels=1,
                inactive_channels=0,
                inbound_msat=100_000_000,
                outbound_msat=100_000_000,
            ),
        )

    def add_invoice(
        self,
        secret: SecretPreimage,
        *,
        amount_msat: int,
        expiry_seconds: int,
        min_final_cltv_delta: int,
        memo: str,
    ) -> CreatedInvoice:
        self.add_invoice_count += 1
        self.incoming_secret = secret
        facts = LndInvoiceFacts(
            payment_hash=payment_hash(secret),
            amount_msat=amount_msat,
            payee=self.route.expected_lnd_pubkey,
            timestamp_unix=NOW,
            expiry_seconds=expiry_seconds,
            min_final_cltv_delta=min_final_cltv_delta,
            network="bitcoin",
            is_amp=False,
        )
        self.created_incoming = CreatedInvoice(
            payment_request="lnbc1incomingruntime",
            payment_hash=facts.payment_hash,
            add_index=7,
            payment_addr=b"\x01" * 32,
            facts=facts,
        )
        return self.created_incoming

    def recover_created_invoice(
        self,
        payment_hash_value: bytes,
        *,
        expected_amount_msat: int,
        expected_payee: str,
        expected_min_final_cltv_delta: int,
    ) -> CreatedInvoice:
        if self.created_incoming is None:
            raise LndInvoiceNotFound("test invoice is absent")
        assert payment_hash_value == self.created_incoming.payment_hash
        assert expected_amount_msat == self.created_incoming.facts.amount_msat
        assert expected_payee == self.created_incoming.facts.payee
        assert (
            expected_min_final_cltv_delta
            == self.created_incoming.facts.min_final_cltv_delta
        )
        return self.created_incoming

    def decode_invoice(self, invoice: str) -> LndInvoiceFacts:
        if invoice != "lnbc1outgoingruntime":
            raise ValueError("unknown test invoice")
        return self.outgoing_facts

    def lookup_invoice(
        self,
        payment_hash_value: str,
        *,
        expected_amount_msat: int,
    ) -> InvoiceStatus:
        assert self.incoming_secret is not None
        assert payment_hash_value == payment_hash(self.incoming_secret).hex()
        assert expected_amount_msat == AMOUNT_MSAT
        return InvoiceStatus(
            payment_hash=payment_hash(self.incoming_secret),
            settled=self.incoming_settled,
            state="SETTLED" if self.incoming_settled else self.incoming_state,
            amount_paid_msat=AMOUNT_MSAT if self.incoming_settled else 0,
            add_index=7,
            settle_index=8 if self.incoming_settled else 0,
            is_amp=False,
            invoice_amount_msat=AMOUNT_MSAT,
            payment_request="lnbc1incomingruntime",
            payment_addr=b"\x01" * 32,
        )

    def send_payment(self, *_args: object, **_kwargs: object) -> SettledPayment:
        self.send_count += 1
        if self.fail_payment:
            raise LightningPaymentError("terminal route failure")
        return SettledPayment(
            payment_hash=payment_hash(OFFRAMP_SECRET),
            payment_preimage=OFFRAMP_SECRET,
            fee_sat=1,
            payer_htlc_expiries=(900_200,),
            fee_msat=1_001,
            amount_msat=AMOUNT_MSAT,
        )

    def track_payment(
        self, payment_hash_value: str, *, expected_amount_msat: int
    ) -> PaymentReconciliation:
        assert payment_hash_value == payment_hash(OFFRAMP_SECRET).hex()
        assert expected_amount_msat == AMOUNT_MSAT
        settled = SettledPayment(
            payment_hash=payment_hash(OFFRAMP_SECRET),
            payment_preimage=OFFRAMP_SECRET,
            fee_sat=1,
            payer_htlc_expiries=(900_200,),
            fee_msat=1_001,
            amount_msat=AMOUNT_MSAT,
        )
        return PaymentReconciliation(
            payment_hash=settled.payment_hash,
            status=PaymentReconciliationStatus.SETTLED,
            settled_payment=settled,
            failure_reason=None,
            last_lnd_status="SUCCEEDED",
            updates_seen=1,
        )


class FakeObserver:
    def __init__(self, route: object) -> None:
        self.route = route
        self.height = 42
        self.inventory = 100_000_000
        self.user_balance = 10_000_000
        self.escrow_state = "open"
        self.offramp_lock_applied = False
        self.onramp_finish_applied = False
        self.offramp_cancel_applied = False

    def route_snapshot(self) -> PftlRouteSnapshot:
        return PftlRouteSnapshot(
            height=self.height,
            block_tip_hash="aa" * 48,
            state_root="bb" * 48,
            agreeing_validator_count=6,
            validator_count=6,
            build_git_revision="test-hardened",
            asset_id=self.route.pftl_asset_id,
            asset_precision=self.route.pftl_asset_precision,
            nav_epoch=self.route.pftl_nav_epoch,
            nav_per_unit=100_000_000,
            nav_reserve_packet_hash=self.route.pftl_nav_reserve_packet_hash,
            coordinator_inventory_atoms=self.inventory,
            coordinator_trustline_limit_atoms=200_000_000,
            coordinator_receive_headroom_atoms=100_000_000,
            coordinator_native_balance=1_000_000,
            user_balance_atoms=self.user_balance,
            user_trustline_limit_atoms=200_000_000,
            user_receive_headroom_atoms=190_000_000,
            user_native_balance=1_000_000,
            asset_freeze_enabled=False,
            asset_clawback_enabled=False,
            asset_requires_authorization=False,
        )

    def open_escrow(
        self, escrow_id: str, *, expected: object
    ) -> dict[str, object]:
        assert escrow_id == ESCROW_ID
        if (
            expected["owner"] == USER
            and not self.offramp_lock_applied
        ):
            self.user_balance -= expected["amount"]
            self.offramp_lock_applied = True
        return {
            "height": self.height,
            "block_tip_hash": "aa" * 48,
            "state_root": "bb" * 48,
            "agreeing_validator_count": 6,
            "validator_count": 6,
            "escrow": {**expected, "state": "open"},
        }

    def user_finish_capacity(
        self, escrow_id: str, *, expected: object
    ) -> dict[str, object]:
        view = self.open_escrow(escrow_id, expected=expected)
        return {
            **view,
            "recipient_asset_headroom": 190_000_000,
            "recipient_native_balance": 1_000_000,
            "finish_minimum_fee": 23,
            "account_reserve": 10,
        }

    def receipt(self, tx_id: str) -> dict[str, object]:
        assert tx_id in {
            CREATE_TX,
            FINISH_TX,
            USER_FINISH_TX,
            CANCEL_TX,
            "94" * 48,
        }
        return {
            "tx_id": tx_id,
            "accepted": True,
            "code": "accepted",
            "height": self.height,
            "block_tip_hash": "aa" * 48,
            "state_root": "bb" * 48,
            "agreeing_validator_count": 6,
            "validator_count": 6,
        }

    def finished_escrow(
        self, escrow_id: str, *, expected: object
    ) -> dict[str, object]:
        assert escrow_id == ESCROW_ID
        if (
            expected["recipient"] == USER
            and not self.onramp_finish_applied
        ):
            self.user_balance += expected["amount"]
            self.onramp_finish_applied = True
        self.escrow_state = "finished"
        return {
            "height": self.height,
            "block_tip_hash": "aa" * 48,
            "state_root": "bb" * 48,
            "agreeing_validator_count": 6,
            "validator_count": 6,
            "escrow": {**expected, "state": "finished"},
        }

    def canceled_escrow(
        self, escrow_id: str, *, expected: object
    ) -> dict[str, object]:
        assert escrow_id == ESCROW_ID
        assert self.escrow_state == "canceled"
        if (
            expected["owner"] == USER
            and not self.offramp_cancel_applied
        ):
            self.user_balance += expected["amount"]
            self.offramp_cancel_applied = True
        return {
            "height": self.height,
            "block_tip_hash": "aa" * 48,
            "state_root": "bb" * 48,
            "agreeing_validator_count": 6,
            "validator_count": 6,
            "escrow": {**expected, "state": "canceled"},
        }


class FakeBackend:
    def __init__(self, observer: FakeObserver) -> None:
        self.observer = observer
        self.create_calls = 0
        self.finish_calls = 0
        self.fail_create_once = False
        self.commit_create_then_fail_once = False
        self.reject_create = False
        self.fail_finish_once = False
        self.commit_finish_then_fail_once = False
        self.cancel_calls = 0
        self.create_applied = False
        self.cancel_applied = False
        self.finish_applied = False
        self.planned_amount_atoms = 0

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
    ) -> PftlEscrowPlan:
        self.planned_amount_atoms = amount_atoms
        return PftlEscrowPlan(
            owner=owner,
            owner_sequence=8,
            recipient=recipient,
            expected_escrow_id=ESCROW_ID,
            operation={
                "operation": "escrow_create",
                "owner": owner,
                "recipient": recipient,
                "asset_id": asset_id,
                "amount": amount_atoms,
                "condition": condition,
                "finish_after": finish_after,
                "cancel_after": cancel_after,
            },
        )

    def submit_create(
        self, _plan: PftlEscrowPlan, *, effect_key: str
    ) -> PftlEffect:
        assert effect_key.endswith(":pftl-create")
        self.create_calls += 1
        if self.fail_create_once and self.create_calls == 1:
            raise OSError("simulated uncertain PFTL create")
        if self.reject_create:
            return PftlEffect(
                tx_id=CREATE_TX,
                accepted=False,
                code="insufficient_balance",
                agreeing_validator_count=6,
                validator_count=6,
                finalized_height=42,
                state_root="bb" * 48,
                block_tip_hash="aa" * 48,
                mutation_free=True,
            )
        if not self.create_applied:
            self.observer.inventory -= self.planned_amount_atoms
            self.create_applied = True
        if self.commit_create_then_fail_once and self.create_calls == 1:
            raise OSError("simulated lost create response after consensus commit")
        return PftlEffect(
            tx_id=CREATE_TX,
            accepted=True,
            code="accepted",
            agreeing_validator_count=6,
            validator_count=6,
            finalized_height=42,
            state_root="bb" * 48,
            block_tip_hash="aa" * 48,
        )

    def submit_cancel(
        self,
        *,
        owner: str,
        escrow_id: str,
        effect_key: str,
    ) -> PftlEffect:
        assert owner.startswith("pf")
        assert escrow_id == ESCROW_ID
        assert effect_key.endswith(":pftl-cancel")
        self.cancel_calls += 1
        if not self.cancel_applied:
            self.observer.inventory += self.planned_amount_atoms
            self.observer.escrow_state = "canceled"
            self.cancel_applied = True
        return PftlEffect(
            tx_id=CANCEL_TX,
            accepted=True,
            code="accepted",
            agreeing_validator_count=6,
            validator_count=6,
            finalized_height=self.observer.height,
            state_root="bb" * 48,
            block_tip_hash="aa" * 48,
        )

    def submit_finish(
        self,
        *,
        owner: str,
        recipient: str,
        escrow_id: str,
        secret: SecretPreimage,
        effect_key: str,
    ) -> PftlEffect:
        assert owner == USER
        assert recipient.startswith("pf")
        assert escrow_id == ESCROW_ID
        assert secret.reveal_for_protocol() == OFFRAMP_SECRET.reveal_for_protocol()
        assert effect_key.endswith(":pftl-finish")
        self.finish_calls += 1
        if self.fail_finish_once and self.finish_calls == 1:
            raise OSError("simulated crash boundary")
        if not self.finish_applied:
            self.observer.inventory += self.planned_amount_atoms
            self.observer.escrow_state = "finished"
            self.finish_applied = True
        if self.commit_finish_then_fail_once and self.finish_calls == 1:
            raise OSError("simulated lost finish response after consensus commit")
        return PftlEffect(
            tx_id=FINISH_TX,
            accepted=True,
            code="accepted",
            agreeing_validator_count=6,
            validator_count=6,
            finalized_height=42,
            state_root="bb" * 48,
            block_tip_hash="aa" * 48,
        )


class RuntimeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.route = policy(mode="ARMED")
        self.lnd = FakeLnd(self.route)
        self.observer = FakeObserver(self.route)
        self.backend = FakeBackend(self.observer)
        root = Path(self.temp.name)
        self.journal = CoordinatorJournal(
            root / "journal.sqlite3",
            ExposureLimits(
                per_principal_atoms=500_000_000,
                aggregate_atoms=1_000_000_000,
            ),
            clock_ns=lambda: NOW * 1_000_000_000,
        )
        self.budget = RealValueBudget(root / "budget.sqlite3", self.route)
        self.addCleanup(self.journal.close)
        self.addCleanup(self.budget.close)
        self.runtime = self._runtime()

    def _runtime(self, *, now: int = NOW) -> MainnetCoordinatorRuntime:
        return MainnetCoordinatorRuntime(
            policy=self.route,
            price=price(),
            lnd=self.lnd,
            pftl_observer=self.observer,
            pftl_backend=self.backend,
            journal=self.journal,
            budget=self.budget,
            quote_signer=QUOTE_SIGNER,
            clock=lambda: now,
        )

    def _authorize(self, swap_id: str) -> dict[str, object]:
        swap = self.journal.get_swap(swap_id)
        view = validate_mainnet_quote(
            swap["signed_quote"], self.route, price(), now_unix=NOW
        )
        return authorization_for(view, self.route)

    def _prepared_offramp(self) -> str:
        created = self.runtime.create_quote(
            {
                "direction": "pftl_to_lightning",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "invoice": "lnbc1outgoingruntime",
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        authorized = self.runtime.authorize_swap(
            swap_id, self._authorize(swap_id)
        )
        self.assertEqual(
            authorized["state"], SwapState.PFTL_LOCK_SUBMITTED.value
        )
        self.assertIs(authorized["can_execute"], True)
        self.runtime.observe_user_lock(swap_id, "94" * 48)
        return swap_id

    def test_onramp_authorizes_before_pftl_inventory_mutation_and_charges_settlement(
        self,
    ) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        self.assertEqual(created["state"], SwapState.PFTL_LOCK_SUBMITTED.value)
        self.assertNotIn("lightning", created)
        self.assertEqual(created["invoice_amount_msat"], AMOUNT_MSAT)
        self.assertEqual(created["wallet_address"], USER)
        self.assertEqual(self.backend.create_calls, 0)
        self.assertEqual(self.lnd.preflight_calls[-1]["min_outbound_msat"], 0)

        authorized = self.runtime.authorize_swap(
            swap_id, self._authorize(swap_id)
        )
        self.assertEqual(authorized["state"], SwapState.PFTL_LOCK_FINAL.value)
        self.assertEqual(self.backend.create_calls, 1)
        self.assertIn("lightning", authorized)
        self.assertEqual(
            authorized["pftl"]["quorum"],
            {
                "observed": 6,
                "required": 6,
                "validator_count": 6,
                "converged": True,
            },
        )
        self.assertEqual(
            authorized["pftl"]["receipt"],
            {
                "tx_id": CREATE_TX,
                "accepted": True,
                "code": "accepted",
            },
        )
        self.assertEqual(authorized["pftl"]["escrow"]["state"], "open")
        self.assertNotIn("condition", authorized["pftl"]["receipt"])
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RESERVED"
        )

        self.lnd.incoming_settled = True
        settled = self.runtime.refresh_onramp(swap_id)
        self.assertEqual(settled["state"], SwapState.LN_SETTLED.value)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "SPENT"
        )
        # A restart/retry is idempotent and cannot charge twice.
        self.runtime.refresh_onramp(swap_id)
        self.assertEqual(self.budget.summary()["spent_count"], 1)
        finished = self.runtime.observe_user_finish(swap_id, USER_FINISH_TX)
        self.assertEqual(
            finished["state"], SwapState.PFTL_FINISH_FINAL.value
        )
        terminal = next(
            event["evidence"]
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.PFTL_FINISH_FINAL.value
        )
        self.assertEqual(terminal["coordinator_inventory_delta_atoms"], 0)
        self.assertEqual(
            terminal["user_balance_delta_atoms"],
            finished["pftl_amount_atoms"],
        )
        self.assertEqual(
            terminal["route_after"]["user_balance_atoms"]
            - terminal["route_before"]["user_balance_atoms"],
            finished["pftl_amount_atoms"],
        )
        self.assertEqual(
            terminal["route_after"]["state_root"], terminal["state_root"]
        )
        self.runtime.observe_user_finish(swap_id, USER_FINISH_TX)
        with self.assertRaisesRegex(Exception, "different PFTL finish"):
            self.runtime.observe_user_finish(swap_id, "96" * 48)

    def test_client_request_id_retry_returns_same_quote_without_new_invoice(self) -> None:
        request = {
            "direction": "lightning_to_pftl",
            "amount_msat": AMOUNT_MSAT,
            "wallet_address": USER,
            "client_request_id": REQUEST_ID,
        }
        first = self.runtime.create_quote(request)
        second = self.runtime.create_quote(dict(request))
        self.assertEqual(first, second)
        self.assertEqual(first["swap_id"], REQUEST_ID)
        self.assertEqual(self.lnd.add_invoice_count, 1)
        changed = dict(request)
        changed["amount_msat"] = AMOUNT_MSAT + 1
        with self.assertRaisesRegex(Exception, "reused for a different"):
            self.runtime.create_quote(changed)
        self.assertEqual(self.lnd.add_invoice_count, 1)

    def test_expired_quote_retry_releases_global_lock_before_swap_recovery(
        self,
    ) -> None:
        request = {
            "direction": "lightning_to_pftl",
            "amount_msat": AMOUNT_MSAT,
            "wallet_address": USER,
            "client_request_id": REQUEST_ID,
        }
        self.runtime.create_quote(request)
        expired_runtime = self._runtime(now=NOW + 61)
        original_recover = expired_runtime.recover_swap
        quote_lock_owned: list[bool] = []

        def checked_recover(swap_id: str) -> dict[str, object]:
            quote_lock_owned.append(
                bool(expired_runtime._quote_lock._is_owned())  # type: ignore[attr-defined]
            )
            return dict(original_recover(swap_id))

        expired_runtime.recover_swap = checked_recover  # type: ignore[method-assign]
        result = expired_runtime.create_quote(request)
        self.assertEqual(result["state"], SwapState.ABORTED_NO_VALUE.value)
        self.assertEqual(quote_lock_owned, [False])

    def test_add_invoice_crash_recovers_same_invoice_without_orphan(self) -> None:
        request = {
            "direction": "lightning_to_pftl",
            "amount_msat": AMOUNT_MSAT,
            "wallet_address": USER,
            "client_request_id": REQUEST_ID,
        }
        admit = self.runtime.service.admit_quote

        def crash_before_swap(*_args: object, **_kwargs: object) -> object:
            raise OSError("simulated crash after AddInvoice")

        self.runtime.service.admit_quote = crash_before_swap  # type: ignore[method-assign]
        with self.assertRaisesRegex(OSError, "after AddInvoice"):
            self.runtime.create_quote(request)
        self.assertEqual(self.lnd.add_invoice_count, 1)
        assert self.lnd.incoming_secret is not None
        secret_hex = self.lnd.incoming_secret.protocol_hex()
        self.assertNotIn(
            secret_hex,
            str(self.journal.export_public_audit()),
        )

        self.runtime.service.admit_quote = admit  # type: ignore[method-assign]
        recovered = self.runtime.create_quote(request)
        self.assertEqual(recovered["swap_id"], REQUEST_ID)
        self.assertEqual(self.lnd.add_invoice_count, 1)

    def test_expired_unauthorized_unattempted_quote_releases_exposure(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        self.assertEqual(self.journal.exposure()["active_swaps"], 1)
        expired = self._runtime(now=NOW + 121).recover_swap(
            created["swap_id"]
        )
        self.assertEqual(expired["state"], SwapState.ABORTED_NO_VALUE.value)
        self.assertNotIn("lightning", expired)
        self.assertEqual(self.journal.exposure()["active_swaps"], 0)
        effect = next(
            row
            for row in self.journal.export_public_audit()["side_effects"]
            if row["swap_id"] == created["swap_id"]
        )
        self.assertEqual(effect["status"], "FAILED_TERMINAL")
        self.assertEqual(effect["attempt_count"], 0)
        self.assertEqual(self.backend.create_calls, 0)

    def test_expiry_never_classifies_an_attempted_create_as_no_value(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        effect = next(
            row
            for row in self.journal.pending_side_effects()
            if row["swap_id"] == created["swap_id"]
        )
        self.journal.record_side_effect_attempt(
            effect["effect_key"],
            f"{effect['effect_key']}:transport-uncertain",
            "RETRYABLE_FAILURE",
            result={"outcome": "uncertain"},
        )
        held = self._runtime(now=NOW + 121).recover_swap(
            created["swap_id"]
        )
        self.assertEqual(held["state"], SwapState.PFTL_LOCK_SUBMITTED.value)
        self.assertEqual(self.journal.exposure()["active_swaps"], 1)

    def test_stale_price_blocks_status_and_precedes_invoice_creation(self) -> None:
        stale = MainnetCoordinatorRuntime(
            policy=self.route,
            price=price(observed_at_unix=NOW - 61),
            lnd=self.lnd,
            pftl_observer=self.observer,
            pftl_backend=self.backend,
            journal=self.journal,
            budget=self.budget,
            quote_signer=QUOTE_SIGNER,
            clock=lambda: NOW,
        )
        status = stale.public_status()
        self.assertIs(status["can_execute"], False)
        self.assertIn(
            "operator_reviewed_btc_price_not_fresh", status["hold_reasons"]
        )
        with self.assertRaisesRegex(Exception, "stale"):
            stale.create_quote(
                {
                    "direction": "lightning_to_pftl",
                    "amount_msat": AMOUNT_MSAT,
                    "wallet_address": USER,
                    "client_request_id": REQUEST_ID,
                }
            )
        self.assertIsNone(self.lnd.incoming_secret)

    def test_quote_rejects_any_wallet_except_pinned_demo_user(self) -> None:
        with self.assertRaisesRegex(Exception, "pinned real-value demo user"):
            self.runtime.create_quote(
                {
                    "direction": "lightning_to_pftl",
                    "amount_msat": AMOUNT_MSAT,
                    "wallet_address": "pf" + "78" * 20,
                    "client_request_id": REQUEST_ID,
                }
            )
        self.assertEqual(self.lnd.add_invoice_count, 0)
        self.assertEqual(self.journal.exposure()["active_swaps"], 0)

    def test_offramp_happy_path_is_terminal_and_single_charged(self) -> None:
        swap_id = self._prepared_offramp()
        self.assertEqual(self.lnd.preflight_calls[-1]["min_inbound_msat"], 0)
        result = self.runtime.execute_offramp(swap_id)
        self.assertEqual(result["state"], SwapState.PFTL_FINISH_FINAL.value)
        self.assertEqual(self.lnd.send_count, 1)
        self.assertEqual(self.backend.finish_calls, 1)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "SPENT"
        )
        terminal = next(
            event["evidence"]
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.PFTL_FINISH_FINAL.value
        )
        quote = self.journal.get_swap(swap_id)["signed_quote"]["quote"]
        self.assertEqual(
            terminal["coordinator_inventory_delta_atoms"],
            quote["pftl_amount_atoms"],
        )
        self.assertEqual(terminal["user_balance_delta_atoms"], 0)
        self.assertEqual(
            terminal["route_after"]["coordinator_inventory_atoms"]
            - terminal["route_before"]["coordinator_inventory_atoms"],
            quote["pftl_amount_atoms"],
        )
        self.runtime.execute_offramp(swap_id)
        self.assertEqual(self.lnd.send_count, 1)
        self.assertEqual(self.budget.summary()["spent_count"], 1)

    def test_concurrent_offramp_recovery_serializes_one_lightning_payment(self) -> None:
        swap_id = self._prepared_offramp()
        barrier = threading.Barrier(3)
        results: list[object] = []
        errors: list[BaseException] = []

        def execute() -> None:
            barrier.wait()
            try:
                results.append(self.runtime.execute_offramp(swap_id))
            except BaseException as error:
                errors.append(error)

        workers = [threading.Thread(target=execute) for _ in range(2)]
        for worker in workers:
            worker.start()
        barrier.wait()
        for worker in workers:
            worker.join()
        self.assertEqual(errors, [])
        self.assertEqual(len(results), 2)
        self.assertEqual(self.lnd.send_count, 1)
        self.assertEqual(self.backend.finish_calls, 1)
        self.assertEqual(self.budget.summary()["spent_count"], 1)

    def test_offramp_user_lock_requires_prior_value_authorization(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "pftl_to_lightning",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "invoice": "lnbc1outgoingruntime",
                "client_request_id": REQUEST_ID,
            }
        )
        self.assertIs(created["can_execute"], False)
        self.assertEqual(created["wallet_address"], USER)
        with self.assertRaisesRegex(Exception, "reserved value authorization"):
            self.runtime.observe_user_lock(created["swap_id"], "94" * 48)
        self.assertEqual(
            self.journal.get_swap(created["swap_id"])["state"],
            SwapState.PFTL_LOCK_SUBMITTED.value,
        )

    def test_offramp_rechecks_pftl_timelock_before_any_lightning_send(self) -> None:
        swap_id = self._prepared_offramp()
        self.observer.height += 101
        with self.assertRaisesRegex(Exception, "CLTV margin"):
            self.runtime.execute_offramp(swap_id)
        self.assertEqual(self.lnd.send_count, 0)
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"],
            SwapState.PFTL_LOCK_FINAL.value,
        )

    def test_authorized_onramp_lock_recovers_without_new_authorization(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        permit = self._authorize(swap_id)
        self.backend.fail_create_once = True
        with self.assertRaisesRegex(Exception, "idempotent recovery"):
            self.runtime.authorize_swap(swap_id, permit)
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"],
            SwapState.PFTL_LOCK_SUBMITTED.value,
        )
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RESERVED"
        )
        pending = self.runtime.public_swap(swap_id)
        self.assertNotIn("lightning", pending)
        self.assertIs(pending["can_execute"], False)

        restarted = self._runtime()
        result = restarted.recover_swap(swap_id)
        self.assertEqual(result["state"], SwapState.PFTL_LOCK_FINAL.value)
        self.assertEqual(self.backend.create_calls, 2)
        self.assertEqual(self.budget.summary()["reserved_count"], 1)

    def test_onramp_create_reconciles_commit_after_response_loss_against_baseline(
        self,
    ) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        inventory_before = self.observer.inventory
        self.backend.commit_create_then_fail_once = True
        with self.assertRaisesRegex(Exception, "idempotent recovery"):
            self.runtime.authorize_swap(swap_id, self._authorize(swap_id))
        amount = self.journal.get_swap(swap_id)["signed_quote"]["quote"][
            "pftl_amount_atoms"
        ]
        self.assertEqual(self.observer.inventory, inventory_before - amount)

        recovered = self._runtime().recover_swap(swap_id)
        self.assertEqual(recovered["state"], SwapState.PFTL_LOCK_FINAL.value)
        self.assertEqual(self.backend.create_calls, 2)
        self.assertEqual(self.observer.inventory, inventory_before - amount)
        lock = next(
            event["evidence"]
            for event in self.journal.events(swap_id)
            if event["to_state"] == SwapState.PFTL_LOCK_FINAL.value
        )
        self.assertEqual(
            lock["route_after"]["coordinator_inventory_atoms"]
            - lock["route_before"]["coordinator_inventory_atoms"],
            -amount,
        )

    def test_canceled_unpaid_onramp_refunds_exact_inventory_and_budget(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        authorized = self.runtime.authorize_swap(
            swap_id, self._authorize(swap_id)
        )
        amount_atoms = authorized["pftl_amount_atoms"]
        inventory_after_lock = self.observer.inventory
        self.runtime.refresh_onramp(swap_id)
        self.lnd.incoming_state = "CANCELED"
        quote = self.journal.get_swap(swap_id)["signed_quote"]["quote"]
        self.observer.height = quote["cancel_after"]

        refunded = self.runtime.refresh_onramp(swap_id)
        self.assertEqual(
            refunded["state"], SwapState.PFTL_CANCEL_FINAL.value
        )
        self.assertEqual(self.backend.cancel_calls, 1)
        self.assertEqual(
            self.observer.inventory, inventory_after_lock + amount_atoms
        )
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RELEASED"
        )
        self.assertEqual(self.journal.exposure()["active_swaps"], 0)
        self.runtime.recover_swap(swap_id)
        self.assertEqual(self.backend.cancel_calls, 1)

    def test_expired_but_nonterminal_invoice_holds_without_refund_guess(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        self.runtime.authorize_swap(swap_id, self._authorize(swap_id))
        self.runtime.refresh_onramp(swap_id)
        held = self._runtime(now=NOW + 901).recover_swap(swap_id)
        self.assertEqual(held["state"], SwapState.PFTL_LOCK_FINAL.value)
        self.assertIs(held["can_execute"], False)
        self.assertIn(
            "incoming_invoice_expired_terminal_status_unconfirmed",
            held["hold_reasons"],
        )
        self.assertEqual(self.backend.cancel_calls, 0)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RESERVED"
        )

    def test_open_onramp_stays_lock_final_and_invoice_expiry_bounds_quote(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        quote = self.journal.get_swap(created["swap_id"])["signed_quote"]["quote"]
        self.assertEqual(quote["invoice_expiry_unix"], NOW + 60)
        self.assertEqual(quote["quote_expires_unix"], quote["invoice_expiry_unix"])
        self.assertEqual(
            quote["latest_lightning_start_unix"], quote["invoice_expiry_unix"]
        )
        authorized = self.runtime.authorize_swap(
            created["swap_id"], self._authorize(created["swap_id"])
        )
        self.assertEqual(authorized["state"], SwapState.PFTL_LOCK_FINAL.value)
        refreshed = self.runtime.refresh_onramp(created["swap_id"])
        self.assertEqual(refreshed["state"], SwapState.PFTL_LOCK_FINAL.value)
        self.assertIn("lightning", refreshed)

    def test_onramp_uses_full_policy_window_but_never_outlives_price(self) -> None:
        route = replace(
            self.route,
            max_price_age_seconds=300,
            max_quote_lifetime_seconds=300,
        )
        lnd = FakeLnd(route)
        observer = FakeObserver(route)
        runtime = MainnetCoordinatorRuntime(
            policy=route,
            price=price(observed_at_unix=NOW - 250),
            lnd=lnd,
            pftl_observer=observer,
            pftl_backend=FakeBackend(observer),
            journal=self.journal,
            budget=self.budget,
            quote_signer=QUOTE_SIGNER,
            clock=lambda: NOW,
        )
        created = runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": "de" * 32,
            }
        )
        quote = self.journal.get_swap(created["swap_id"])["signed_quote"]["quote"]
        self.assertEqual(quote["invoice_expiry_unix"], NOW + 50)
        self.assertEqual(quote["quote_expires_unix"], NOW + 50)
        self.assertEqual(quote["latest_lightning_start_unix"], NOW + 50)

    def test_onramp_authorization_must_cover_full_invoice_payability(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        permit = self._authorize(created["swap_id"])
        authorization = dict(permit["authorization"])
        authorization["expires_unix"] = NOW + 59
        short_permit = sign_value_authorization(authorization, AUTH_SIGNER)
        with self.assertRaisesRegex(CoordinatorRuntimeError, "full BOLT11 expiry"):
            self.runtime.authorize_swap(created["swap_id"], short_permit)
        self.assertIsNone(self.budget.authorization_for_swap(created["swap_id"]))
        self.assertEqual(self.backend.create_calls, 0)

    def test_authorized_unattempted_onramp_expiry_releases_both_ledgers(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        original = self.runtime._submit_onramp_lock

        def crash_before_effect(_swap_id: str) -> dict[str, object]:
            raise OSError("crash after budget reserve")

        self.runtime._submit_onramp_lock = crash_before_effect  # type: ignore[method-assign]
        with self.assertRaisesRegex(OSError, "budget reserve"):
            self.runtime.authorize_swap(
                created["swap_id"], self._authorize(created["swap_id"])
            )
        self.runtime._submit_onramp_lock = original  # type: ignore[method-assign]
        self.assertEqual(
            self.budget.authorization_for_swap(created["swap_id"])["state"],
            "RESERVED",
        )
        recovered = self._runtime(now=NOW + 61).recover_swap(created["swap_id"])
        self.assertEqual(recovered["state"], SwapState.ABORTED_NO_VALUE.value)
        self.assertEqual(
            self.budget.authorization_for_swap(created["swap_id"])["state"],
            "RELEASED",
        )
        self.assertEqual(self.backend.create_calls, 0)
        self.assertEqual(self.journal.exposure()["active_swaps"], 0)

    def test_late_offramp_lock_never_pays_and_user_cancel_releases_budget(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "pftl_to_lightning",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "invoice": "lnbc1outgoingruntime",
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        self.runtime.authorize_swap(swap_id, self._authorize(swap_id))
        late = self._runtime(now=NOW + 61).observe_user_lock(
            swap_id, "94" * 48
        )
        self.assertEqual(late["state"], SwapState.REFUND_ELIGIBLE.value)
        self.assertFalse(late["can_execute"])
        self.assertEqual(self.lnd.send_count, 0)
        self.assertIsNone(
            self.journal.side_effect(swap_id, "LND_SEND_PAYMENT")
        )
        quote = self.journal.get_swap(swap_id)["signed_quote"]["quote"]
        self.observer.height = quote["cancel_after"]
        self.observer.escrow_state = "canceled"
        refunded = self._runtime(now=NOW + 61).observe_user_cancel(
            swap_id, CANCEL_TX
        )
        self.assertEqual(refunded["state"], SwapState.PFTL_CANCEL_FINAL.value)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RELEASED"
        )
        self.assertEqual(self.journal.exposure()["active_swaps"], 0)
        # Exact duplicate receipt observation is idempotent.
        self._runtime(now=NOW + 61).observe_user_cancel(swap_id, CANCEL_TX)

    def test_refund_inventory_baseline_is_captured_at_cancel_not_eligibility(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        self.runtime.authorize_swap(swap_id, self._authorize(swap_id))
        self.lnd.incoming_state = "CANCELED"
        quote = self.journal.get_swap(swap_id)["signed_quote"]["quote"]
        self.observer.height = quote["cancel_after"] - 1
        waiting = self.runtime.refresh_onramp(swap_id)
        self.assertEqual(waiting["state"], SwapState.REFUND_ELIGIBLE.value)
        # Simulate an unrelated finalized route inventory change while waiting.
        self.observer.inventory += 17
        inventory_before_cancel = self.observer.inventory
        self.observer.height = quote["cancel_after"]
        refunded = self.runtime.recover_onramp_refund(swap_id)
        self.assertEqual(refunded["state"], SwapState.PFTL_CANCEL_FINAL.value)
        self.assertEqual(
            self.observer.inventory,
            inventory_before_cancel + quote["pftl_amount_atoms"],
        )

    def test_exact_millisatoshi_fee_is_required_and_capped(self) -> None:
        with self.assertRaisesRegex(Exception, "exact fee_msat"):
            self.runtime._validate_settled_payment(
                SettledPayment(
                    payment_hash=payment_hash(OFFRAMP_SECRET),
                    payment_preimage=OFFRAMP_SECRET,
                    fee_sat=1,
                    payer_htlc_expiries=(900_200,),
                    fee_msat=None,
                    amount_msat=AMOUNT_MSAT,
                ),
                expected_amount_msat=AMOUNT_MSAT,
                maximum_fee_msat=10_000,
            )
        with self.assertRaisesRegex(Exception, "exceeded"):
            self.runtime._validate_settled_payment(
                SettledPayment(
                    payment_hash=payment_hash(OFFRAMP_SECRET),
                    payment_preimage=OFFRAMP_SECRET,
                    fee_sat=10,
                    payer_htlc_expiries=(900_200,),
                    fee_msat=10_001,
                    amount_msat=AMOUNT_MSAT,
                ),
                expected_amount_msat=AMOUNT_MSAT,
                maximum_fee_msat=10_000,
            )

    def test_mutation_free_onramp_rejection_releases_reserved_budget(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        self.backend.reject_create = True
        with self.assertRaisesRegex(Exception, "insufficient_balance"):
            self.runtime.authorize_swap(swap_id, self._authorize(swap_id))
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"], SwapState.LOCK_FAILED.value
        )
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RELEASED"
        )
        self.runtime.recover_swap(swap_id)
        self.assertEqual(self.budget.summary()["reserved_count"], 0)

    def test_recovery_scan_closes_lock_failed_budget_after_crash(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        self.backend.reject_create = True
        release = self.budget.release_unspent

        def crash_before_budget_close(*_args: object, **_kwargs: object) -> object:
            raise OSError("simulated crash before rejected-lock budget close")

        self.budget.release_unspent = crash_before_budget_close  # type: ignore[method-assign]
        with self.assertRaisesRegex(OSError, "rejected-lock budget close"):
            self.runtime.authorize_swap(swap_id, self._authorize(swap_id))
        self.budget.release_unspent = release  # type: ignore[method-assign]
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"], SwapState.LOCK_FAILED.value
        )
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RESERVED"
        )
        restarted = self._runtime()
        self.assertIn(swap_id, restarted.recovery_swap_ids())
        restarted.recover_swap(swap_id)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RELEASED"
        )
        self.assertNotIn(swap_id, restarted.recovery_swap_ids())

    def test_recovery_scan_closes_finish_final_budget_after_crash(self) -> None:
        swap_id = self._prepared_offramp()
        mark_spent = self.budget.mark_spent

        def crash_before_budget_close(*_args: object, **_kwargs: object) -> object:
            raise OSError("simulated crash before terminal spend close")

        self.budget.mark_spent = crash_before_budget_close  # type: ignore[method-assign]
        with self.assertRaisesRegex(OSError, "terminal spend close"):
            self.runtime.execute_offramp(swap_id)
        self.budget.mark_spent = mark_spent  # type: ignore[method-assign]
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"],
            SwapState.PFTL_FINISH_FINAL.value,
        )
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RESERVED"
        )
        restarted = self._runtime()
        self.assertIn(swap_id, restarted.recovery_swap_ids())
        restarted.recover_swap(swap_id)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "SPENT"
        )
        self.assertNotIn(swap_id, restarted.recovery_swap_ids())

    def test_recovery_scan_closes_cancel_final_budget_after_crash(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        self.runtime.authorize_swap(swap_id, self._authorize(swap_id))
        self.lnd.incoming_state = "CANCELED"
        quote = self.journal.get_swap(swap_id)["signed_quote"]["quote"]
        self.observer.height = quote["cancel_after"]
        release = self.budget.release_unspent

        def crash_before_budget_close(*_args: object, **_kwargs: object) -> object:
            raise OSError("simulated crash before terminal refund close")

        self.budget.release_unspent = crash_before_budget_close  # type: ignore[method-assign]
        with self.assertRaisesRegex(OSError, "terminal refund close"):
            self.runtime.refresh_onramp(swap_id)
        self.budget.release_unspent = release  # type: ignore[method-assign]
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"],
            SwapState.PFTL_CANCEL_FINAL.value,
        )
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RESERVED"
        )
        restarted = self._runtime()
        self.assertIn(swap_id, restarted.recovery_swap_ids())
        restarted.recover_swap(swap_id)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RELEASED"
        )
        self.assertNotIn(swap_id, restarted.recovery_swap_ids())

    def test_recovery_scan_closes_zero_attempt_abort_budget_after_crash(self) -> None:
        created = self.runtime.create_quote(
            {
                "direction": "lightning_to_pftl",
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": REQUEST_ID,
            }
        )
        swap_id = created["swap_id"]
        submit = self.runtime._submit_onramp_lock

        def crash_before_effect(_swap_id: str) -> dict[str, object]:
            raise OSError("simulated crash after authorization reserve")

        self.runtime._submit_onramp_lock = crash_before_effect  # type: ignore[method-assign]
        with self.assertRaisesRegex(OSError, "authorization reserve"):
            self.runtime.authorize_swap(swap_id, self._authorize(swap_id))
        self.runtime._submit_onramp_lock = submit  # type: ignore[method-assign]
        release = self.budget.release_unspent

        def crash_before_budget_close(*_args: object, **_kwargs: object) -> object:
            raise OSError("simulated crash after no-value abort commit")

        self.budget.release_unspent = crash_before_budget_close  # type: ignore[method-assign]
        with self.assertRaisesRegex(OSError, "no-value abort commit"):
            self._runtime(now=NOW + 61).recover_swap(swap_id)
        self.budget.release_unspent = release  # type: ignore[method-assign]
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"],
            SwapState.ABORTED_NO_VALUE.value,
        )
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RESERVED"
        )
        restarted = self._runtime(now=NOW + 61)
        self.assertIn(swap_id, restarted.recovery_swap_ids())
        restarted.recover_swap(swap_id)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RELEASED"
        )
        self.assertNotIn(swap_id, restarted.recovery_swap_ids())

    def test_offramp_finish_recovers_after_ln_settlement_without_repaying(self) -> None:
        swap_id = self._prepared_offramp()
        self.backend.fail_finish_once = True
        with self.assertRaisesRegex(Exception, "idempotent recovery"):
            self.runtime.execute_offramp(swap_id)
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"], SwapState.LN_SETTLED.value
        )
        self.assertEqual(self.lnd.send_count, 1)
        self.assertEqual(
            self.journal.load_secret(
                swap_id, "invoice_preimage"
            ).reveal_for_protocol(),
            OFFRAMP_SECRET.reveal_for_protocol(),
        )

        restarted = self._runtime()
        result = restarted.recover_swap(swap_id)
        self.assertEqual(result["state"], SwapState.PFTL_FINISH_FINAL.value)
        self.assertEqual(self.lnd.send_count, 1)
        self.assertEqual(self.backend.finish_calls, 2)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "SPENT"
        )

    def test_offramp_finish_reconciles_commit_after_response_loss_against_baseline(
        self,
    ) -> None:
        swap_id = self._prepared_offramp()
        inventory_before = self.observer.inventory
        self.backend.commit_finish_then_fail_once = True
        with self.assertRaisesRegex(Exception, "idempotent recovery"):
            self.runtime.execute_offramp(swap_id)
        amount = self.journal.get_swap(swap_id)["signed_quote"]["quote"][
            "pftl_amount_atoms"
        ]
        self.assertEqual(self.observer.inventory, inventory_before + amount)
        recovered = self._runtime().recover_swap(swap_id)
        self.assertEqual(recovered["state"], SwapState.PFTL_FINISH_FINAL.value)
        self.assertEqual(self.lnd.send_count, 1)
        self.assertEqual(self.backend.finish_calls, 2)
        self.assertEqual(self.observer.inventory, inventory_before + amount)

    def test_dry_run_full_negative_value_call_trace(self) -> None:
        dry_route = policy(mode="DRY_RUN")
        dry_lnd = FakeLnd(dry_route)
        dry_observer = FakeObserver(dry_route)
        dry_backend = FakeBackend(dry_observer)
        root = Path(self.temp.name)
        dry_journal = CoordinatorJournal(
            root / "dry-journal.sqlite3",
            ExposureLimits(
                per_principal_atoms=500_000_000,
                aggregate_atoms=1_000_000_000,
            ),
            clock_ns=lambda: NOW * 1_000_000_000,
        )
        dry_budget = RealValueBudget(root / "dry-budget.sqlite3", dry_route)
        self.addCleanup(dry_journal.close)
        self.addCleanup(dry_budget.close)
        dry_runtime = MainnetCoordinatorRuntime(
            policy=dry_route,
            price=price(),
            lnd=dry_lnd,
            pftl_observer=dry_observer,
            pftl_backend=dry_backend,
            journal=dry_journal,
            budget=dry_budget,
            quote_signer=QUOTE_SIGNER,
            clock=lambda: NOW,
        )

        status = dry_runtime.public_status()
        self.assertFalse(status["can_execute"])
        for direction, invoice, request_id in (
            ("lightning_to_pftl", None, "da" * 32),
            ("pftl_to_lightning", "lnbc1unusedindryrun", "db" * 32),
        ):
            request = {
                "direction": direction,
                "amount_msat": AMOUNT_MSAT,
                "wallet_address": USER,
                "client_request_id": request_id,
            }
            if invoice is not None:
                request["invoice"] = invoice
            preview = dry_runtime.create_quote(request)
            self.assertEqual(preview["state"], "DRY_RUN")
            self.assertFalse(preview["can_execute"])

        self.assertEqual(dry_lnd.add_invoice_count, 0)
        self.assertEqual(dry_lnd.send_count, 0)
        self.assertEqual(dry_backend.create_calls, 0)
        self.assertEqual(dry_backend.finish_calls, 0)
        self.assertEqual(dry_backend.cancel_calls, 0)
        self.assertEqual(dry_journal.exposure()["active_swaps"], 0)
        self.assertEqual(dry_budget.summary()["reserved_count"], 0)

    def test_terminal_lightning_failure_never_finishes_pftl(self) -> None:
        swap_id = self._prepared_offramp()
        self.lnd.fail_payment = True
        with self.assertRaisesRegex(Exception, "user refund remains"):
            self.runtime.execute_offramp(swap_id)
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"],
            SwapState.REFUND_ELIGIBLE.value,
        )
        self.assertEqual(self.backend.finish_calls, 0)
        self.assertEqual(
            self.budget.authorization_for_swap(swap_id)["state"], "RESERVED"
        )

    def test_terminal_lightning_failure_recovers_after_state_commit(self) -> None:
        swap_id = self._prepared_offramp()
        self.lnd.fail_payment = True

        def crash_after_refund_state(_swap_id: str) -> dict[str, object]:
            raise OSError("simulated post-state crash")

        self.runtime._finalize_failed_offramp = crash_after_refund_state  # type: ignore[method-assign]
        with self.assertRaisesRegex(OSError, "post-state crash"):
            self.runtime.execute_offramp(swap_id)
        self.assertEqual(
            self.journal.get_swap(swap_id)["state"],
            SwapState.REFUND_ELIGIBLE.value,
        )
        effect = next(
            item
            for item in self.journal.pending_side_effects()
            if item["kind"] == "LND_SEND_PAYMENT"
        )
        self.assertEqual(effect["status"], "PENDING")

        restarted = self._runtime()
        result = restarted.recover_swap(swap_id)
        self.assertEqual(result["state"], SwapState.REFUND_ELIGIBLE.value)
        self.assertFalse(
            any(
                item["kind"] == "LND_SEND_PAYMENT"
                for item in self.journal.pending_side_effects()
            )
        )


if __name__ == "__main__":
    unittest.main()
