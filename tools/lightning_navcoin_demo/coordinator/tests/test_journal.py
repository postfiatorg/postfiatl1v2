from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest

from tools.lightning_navcoin_demo.coordinator.journal import (
    CoordinatorJournal,
    ExposureLimitExceeded,
    ExposureLimits,
    IdempotencyConflict,
    InvalidTransition,
    JournalError,
    SecretMaterialRejected,
    SideEffectSpec,
    SwapState,
    redact_for_log,
)
from tools.lightning_navcoin_demo.coordinator.protocol import SecretPreimage
from tools.lightning_navcoin_demo.coordinator.service import CoordinatorService
from tools.lightning_navcoin_demo.coordinator.signing import (
    Ed25519Signer,
    sign_quote,
)
from tools.lightning_navcoin_demo.coordinator.tests.common import (
    TEST_SIGNING_SEED,
    envelope_for,
    quote_for,
    secret_for,
)


class FakeClock:
    def __init__(self) -> None:
        self.value = 1_000_000

    def __call__(self) -> int:
        self.value += 1
        return self.value


def advance_happy_path(journal: CoordinatorJournal, swap_id: str) -> None:
    for ordinal, state in enumerate(
        (
            SwapState.PFTL_LOCK_SUBMITTED,
            SwapState.PFTL_LOCK_FINAL,
            SwapState.LN_IN_FLIGHT,
            SwapState.LN_SETTLED,
            SwapState.PFTL_FINISH_FINAL,
        ),
        start=1,
    ):
        journal.advance(
            swap_id,
            state,
            f"event:{swap_id}:{ordinal}",
            evidence={"accepted": True, "ordinal": ordinal},
        )


class JournalTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.path = Path(self.temporary.name) / "coordinator.sqlite3"
        self.clock = FakeClock()
        self.limits = ExposureLimits(
            per_principal_atoms=1_000_000,
            aggregate_atoms=2_000_000,
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def open(self, limits: ExposureLimits | None = None) -> CoordinatorJournal:
        return CoordinatorJournal(
            self.path, self.limits if limits is None else limits, clock_ns=self.clock
        )

    def test_secret_storage_permissions_are_owner_only(self) -> None:
        self.path.parent.chmod(0o777)
        with self.open() as journal:
            journal.create_swap(
                "user-a",
                envelope_for(0),
                secret=secret_for(0),
            )
            self.assertEqual(
                os.stat(self.path.parent).st_mode & 0o777,
                0o700,
            )
            for path in (
                self.path,
                Path(f"{self.path}-wal"),
                Path(f"{self.path}-shm"),
            ):
                if path.exists():
                    self.assertEqual(os.stat(path).st_mode & 0o777, 0o600)

    def test_crash_restart_at_every_happy_path_transition(self) -> None:
        envelope = envelope_for(0)
        swap_id = envelope["quote"]["swap_id"]
        with self.open() as journal:
            journal.create_swap("user-a", envelope, secret=secret_for(0))
        transitions = (
            SwapState.PFTL_LOCK_SUBMITTED,
            SwapState.PFTL_LOCK_FINAL,
            SwapState.LN_IN_FLIGHT,
            SwapState.LN_SETTLED,
            SwapState.PFTL_FINISH_FINAL,
        )
        for ordinal, state in enumerate(transitions, start=1):
            with self.open() as restarted:
                self.assertEqual(
                    restarted.get_swap(swap_id)["state"],
                    (
                        SwapState.QUOTED.value
                        if ordinal == 1
                        else transitions[ordinal - 2].value
                    ),
                )
                restarted.advance(
                    swap_id,
                    state,
                    f"state:{ordinal}",
                    evidence={"receipt": "accepted", "ordinal": ordinal},
                )
            with self.open() as verified:
                self.assertEqual(verified.get_swap(swap_id)["state"], state.value)
                self.assertEqual(len(verified.events(swap_id)), ordinal + 1)
                if state not in {
                    SwapState.PFTL_FINISH_FINAL,
                    SwapState.PFTL_CANCEL_FINAL,
                }:
                    self.assertEqual(len(verified.recoverable_swaps()), 1)
        with self.open() as terminal:
            self.assertEqual(terminal.exposure(), {"active_atoms": 0, "active_swaps": 0})
            # Repeating the same event after further progress/terminal release is safe.
            same = terminal.advance(
                swap_id,
                SwapState.PFTL_FINISH_FINAL,
                "state:5",
                evidence={"receipt": "accepted", "ordinal": 5},
            )
            self.assertEqual(same["state"], SwapState.PFTL_FINISH_FINAL.value)
            self.assertEqual(terminal.exposure()["active_atoms"], 0)

    def test_illegal_skip_and_conflicting_event_are_mutation_free(self) -> None:
        envelope = envelope_for(1)
        swap_id = envelope["quote"]["swap_id"]
        with self.open() as journal:
            journal.create_swap("user-a", envelope)
            before = journal.export_public_audit()
            with self.assertRaises(InvalidTransition):
                journal.advance(
                    swap_id,
                    SwapState.LN_SETTLED,
                    "skip",
                    evidence={"accepted": True},
                )
            self.assertEqual(journal.export_public_audit(), before)
            journal.advance(
                swap_id,
                SwapState.PFTL_LOCK_SUBMITTED,
                "lock",
                evidence={"tx": "one"},
            )
            with self.assertRaises(IdempotencyConflict):
                journal.advance(
                    swap_id,
                    SwapState.PFTL_LOCK_SUBMITTED,
                    "lock",
                    evidence={"tx": "different"},
                )
            self.assertEqual(
                journal.get_swap(swap_id)["state"],
                SwapState.PFTL_LOCK_SUBMITTED.value,
            )

    def test_side_effect_intent_and_retries_are_transactionally_idempotent(self) -> None:
        envelope = envelope_for(2)
        swap_id = envelope["quote"]["swap_id"]
        with self.open() as journal:
            journal.create_swap("user-a", envelope)
            operation = {"escrow_id": "aa" * 32, "secret_ref": "invoice_preimage"}
            journal.advance(
                swap_id,
                SwapState.PFTL_LOCK_SUBMITTED,
                "lock-event",
                evidence={"intent_durable": True},
                side_effect=SideEffectSpec("pftl:create:2", "PFTL_CREATE", operation),
            )
            pending = journal.pending_side_effects()
            self.assertEqual(len(pending), 1)
            self.assertEqual(pending[0]["payload"], operation)
        with self.open() as restarted:
            self.assertEqual(
                restarted.pending_side_effects()[0]["effect_key"], "pftl:create:2"
            )
            retry = restarted.record_side_effect_attempt(
                "pftl:create:2",
                "attempt:1",
                "RETRYABLE_FAILURE",
                result={"code": "unavailable"},
            )
            self.assertEqual(retry["status"], "PENDING")
            success = restarted.record_side_effect_attempt(
                "pftl:create:2",
                "attempt:2",
                "SUCCEEDED",
                result={"tx_id": "bb" * 32},
            )
            self.assertEqual(success["status"], "SUCCEEDED")
            duplicate = restarted.record_side_effect_attempt(
                "pftl:create:2",
                "attempt:2",
                "SUCCEEDED",
                result={"tx_id": "bb" * 32},
            )
            self.assertEqual(duplicate["attempt_count"], 2)
            with self.assertRaises(InvalidTransition):
                restarted.record_side_effect_attempt(
                    "pftl:create:2",
                    "attempt:3",
                    "SUCCEEDED",
                    result={"tx_id": "bb" * 32},
                )

    def test_side_effect_key_collision_rolls_back_state_edge(self) -> None:
        first = envelope_for(3)
        second = envelope_for(4)
        with self.open() as journal:
            journal.create_swap("user-a", first)
            journal.create_swap("user-b", second)
            first_id = first["quote"]["swap_id"]
            second_id = second["quote"]["swap_id"]
            journal.advance(
                first_id,
                SwapState.PFTL_LOCK_SUBMITTED,
                "first-event",
                side_effect=SideEffectSpec("shared-effect", "PFTL_CREATE", {"n": 1}),
            )
            with self.assertRaises(IdempotencyConflict):
                journal.advance(
                    second_id,
                    SwapState.PFTL_LOCK_SUBMITTED,
                    "second-event",
                    side_effect=SideEffectSpec(
                        "shared-effect", "PFTL_CREATE", {"n": 2}
                    ),
                )
            self.assertEqual(
                journal.get_swap(second_id)["state"], SwapState.QUOTED.value
            )
            self.assertEqual(len(journal.events(second_id)), 1)

    def test_exposure_caps_reserve_on_quote_and_release_only_at_terminal(self) -> None:
        limits = ExposureLimits(per_principal_atoms=100, aggregate_atoms=150)
        with self.open(limits) as journal:
            first = envelope_for(10, amount_atoms=60)
            journal.create_swap("principal-a", first)
            with self.assertRaises(ExposureLimitExceeded):
                journal.create_swap(
                    "principal-a", envelope_for(11, amount_atoms=50)
                )
            second = envelope_for(12, amount_atoms=80)
            journal.create_swap("principal-b", second)
            self.assertEqual(journal.exposure()["active_atoms"], 140)
            with self.assertRaises(ExposureLimitExceeded):
                journal.create_swap(
                    "principal-c", envelope_for(13, amount_atoms=20)
                )
            advance_happy_path(journal, first["quote"]["swap_id"])
            self.assertEqual(journal.exposure()["active_atoms"], 80)
            journal.create_swap("principal-a", envelope_for(13, amount_atoms=20))
            self.assertEqual(journal.exposure()["active_atoms"], 100)

    def test_concurrent_admission_cannot_overbook_aggregate_cap(self) -> None:
        limits = ExposureLimits(per_principal_atoms=100, aggregate_atoms=100)
        journals = [self.open(limits), self.open(limits)]
        barrier = threading.Barrier(2)
        outcomes: list[str] = []
        outcome_lock = threading.Lock()

        def admit(slot: int) -> None:
            barrier.wait()
            try:
                journals[slot].create_swap(
                    f"principal-{slot}", envelope_for(20 + slot, amount_atoms=60)
                )
                outcome = "accepted"
            except ExposureLimitExceeded:
                outcome = "capped"
            with outcome_lock:
                outcomes.append(outcome)

        threads = [threading.Thread(target=admit, args=(slot,)) for slot in range(2)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join(timeout=10)
        for journal in journals:
            journal.close()
        self.assertEqual(sorted(outcomes), ["accepted", "capped"])
        with self.open(limits) as journal:
            self.assertEqual(journal.exposure()["active_atoms"], 60)

    def test_payment_hash_registry_is_durable_and_global(self) -> None:
        first = envelope_for(30)
        quote = quote_for(31)
        quote["payment_hash"] = first["quote"]["payment_hash"]
        quote["condition"] = first["quote"]["condition"]
        second = sign_quote(
            quote, Ed25519Signer.from_private_bytes(TEST_SIGNING_SEED)
        )
        with self.open() as journal:
            journal.create_swap("principal-a", first)
            with self.assertRaises(IdempotencyConflict):
                journal.create_swap("principal-b", second)

    def test_expired_new_quote_rejects_without_mutation(self) -> None:
        expired = quote_for(32)
        expired["quote_expires_unix"] = 1
        expired["latest_lightning_start_unix"] = 1
        expired["invoice_expiry_unix"] = 2
        envelope = sign_quote(
            expired, Ed25519Signer.from_private_bytes(TEST_SIGNING_SEED)
        )
        clock = lambda: 2_000_000_000
        with CoordinatorJournal(
            self.path,
            self.limits,
            clock_ns=clock,
        ) as journal:
            before = journal.export_public_audit()
            with self.assertRaisesRegex(JournalError, "expired quote"):
                journal.create_swap("principal-a", envelope)
            self.assertEqual(journal.export_public_audit(), before)

    def test_pre_value_terminal_states_release_exposure_fail_closed(self) -> None:
        for index, target in enumerate(
            (SwapState.QUOTE_EXPIRED, SwapState.ABORTED_NO_VALUE), start=33
        ):
            path = Path(self.temporary.name) / f"pre-value-{index}.sqlite3"
            with CoordinatorJournal(
                path, self.limits, clock_ns=self.clock
            ) as journal:
                envelope = envelope_for(index)
                swap_id = envelope["quote"]["swap_id"]
                journal.create_swap("principal-a", envelope)
                journal.advance(
                    swap_id,
                    target,
                    f"terminal:{index}",
                    evidence={"value_moved": False},
                )
                self.assertEqual(journal.exposure()["active_atoms"], 0)
                self.assertEqual(journal.recoverable_swaps(), [])

        path = Path(self.temporary.name) / "lock-failed.sqlite3"
        with CoordinatorJournal(path, self.limits, clock_ns=self.clock) as journal:
            envelope = envelope_for(35)
            swap_id = envelope["quote"]["swap_id"]
            journal.create_swap("principal-a", envelope)
            journal.advance(
                swap_id,
                SwapState.PFTL_LOCK_SUBMITTED,
                "lock-submitted",
                evidence={"intent_durable": True},
            )
            journal.advance(
                swap_id,
                SwapState.LOCK_FAILED,
                "lock-failed",
                evidence={
                    "accepted": False,
                    "mutation_free": True,
                    "code": "rejected",
                },
            )
            self.assertEqual(journal.exposure()["active_atoms"], 0)
            self.assertEqual(journal.recoverable_swaps(), [])

    def test_refund_branch_from_in_flight_is_durable_and_releases_exposure(self) -> None:
        envelope = envelope_for(40)
        swap_id = envelope["quote"]["swap_id"]
        with self.open() as journal:
            journal.create_swap("principal-a", envelope)
            for ordinal, state in enumerate(
                (
                    SwapState.PFTL_LOCK_SUBMITTED,
                    SwapState.PFTL_LOCK_FINAL,
                    SwapState.LN_IN_FLIGHT,
                    SwapState.REFUND_ELIGIBLE,
                    SwapState.PFTL_CANCEL_FINAL,
                ),
                start=1,
            ):
                journal.advance(swap_id, state, f"refund:{ordinal}")
            self.assertEqual(journal.exposure()["active_atoms"], 0)
            with self.assertRaises(InvalidTransition):
                journal.advance(
                    swap_id, SwapState.LN_SETTLED, "settle-after-refund"
                )

    def test_secret_is_separate_from_public_audit_and_logs(self) -> None:
        envelope = envelope_for(50)
        swap_id = envelope["quote"]["swap_id"]
        secret = secret_for(50)
        with self.open() as journal:
            journal.create_swap("principal-a", envelope, secret=secret)
            self.assertEqual(
                journal.load_secret(swap_id, "invoice_preimage").reveal_for_protocol(),
                secret.reveal_for_protocol(),
            )
            audit = json.dumps(journal.export_public_audit(), sort_keys=True)
            self.assertNotIn(secret.protocol_hex(), audit)
            before = journal.get_swap(swap_id)
            with self.assertRaises(SecretMaterialRejected):
                journal.advance(
                    swap_id,
                    SwapState.PFTL_LOCK_SUBMITTED,
                    "secret-leak",
                    evidence={"preimage": secret.protocol_hex()},
                )
            with self.assertRaises(SecretMaterialRejected):
                journal.advance(
                    swap_id,
                    SwapState.PFTL_LOCK_SUBMITTED,
                    "secret-leak-free-text",
                    evidence={"message": f"leaked={secret.protocol_hex()}"},
                )
            self.assertEqual(journal.get_swap(swap_id), before)
        redacted = redact_for_log(
            {
                "preimage": secret.protocol_hex(),
                "message": f"do not leak {secret.protocol_hex()}",
                "safe": envelope["quote"]["payment_hash"],
            },
            known_secrets=(secret,),
        )
        self.assertEqual(redacted["preimage"], "<redacted>")
        self.assertNotIn(secret.protocol_hex(), redacted["message"])
        self.assertEqual(redacted["safe"], envelope["quote"]["payment_hash"])


class ServiceTests(unittest.TestCase):
    def test_service_recovery_plan_tracks_state_and_pending_effect(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "service.sqlite3"
            with CoordinatorJournal(
                path,
                ExposureLimits(1_000_000, 1_000_000),
                clock_ns=lambda: 1_000_000,
            ) as journal:
                service = CoordinatorService(journal)
                envelope = envelope_for(60)
                swap_id = envelope["quote"]["swap_id"]
                service.admit_quote("principal", envelope)
                service.mark_lock_submitted(
                    swap_id,
                    effect_key="create:60",
                    operation={"escrow_id": envelope["quote"]["expected_escrow_id"]},
                )
                actions = service.recovery_plan()
                self.assertEqual(len(actions), 1)
                self.assertEqual(
                    actions[0].state, SwapState.PFTL_LOCK_SUBMITTED
                )
                self.assertEqual(
                    actions[0].action, "observe_pftl_lock_finality"
                )
                self.assertEqual(actions[0].pending_effect_keys, ("create:60",))

    def test_learned_preimage_state_and_finish_intent_commit_together(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "service-secret.sqlite3"
            envelope = envelope_for(61, direction="pftl_to_lightning")
            swap_id = envelope["quote"]["swap_id"]
            secret = secret_for(61)
            finish_operation = {
                "escrow_id": envelope["quote"]["expected_escrow_id"],
                "secret_ref": "invoice_preimage",
            }
            settlement_evidence = {
                "payment_hash": envelope["quote"]["payment_hash"],
                "status": "SUCCEEDED",
            }
            with CoordinatorJournal(
                path,
                ExposureLimits(1_000_000, 1_000_000),
                clock_ns=lambda: 1_000_000,
            ) as journal:
                service = CoordinatorService(journal)
                service.admit_quote("principal", envelope)
                service.mark_lock_submitted(
                    swap_id,
                    effect_key="create:61",
                    operation={"escrow_id": envelope["quote"]["expected_escrow_id"]},
                )
                journal.record_side_effect_attempt(
                    "create:61",
                    "create:61:accepted",
                    "SUCCEEDED",
                    result={"accepted": True},
                )
                service.mark_lock_final(
                    swap_id, finality_evidence={"accepted": True}
                )
                service.mark_ln_in_flight(
                    swap_id,
                    payment_evidence={
                        "payment_hash": envelope["quote"]["payment_hash"]
                    },
                    effect_key="ln-pay:61",
                    payment_request={"invoice": envelope["quote"]["invoice"]},
                )
                journal.record_side_effect_attempt(
                    "ln-pay:61",
                    "ln-pay:61:settled",
                    "SUCCEEDED",
                    result={"status": "SUCCEEDED"},
                )
                service.mark_ln_settled(
                    swap_id,
                    settlement_evidence=settlement_evidence,
                    learned_secret=secret,
                    effect_key="finish:61",
                    finish_operation=finish_operation,
                )
            with CoordinatorJournal(
                path,
                ExposureLimits(1_000_000, 1_000_000),
                clock_ns=lambda: 1_000_000,
            ) as restarted:
                service = CoordinatorService(restarted)
                self.assertEqual(
                    restarted.get_swap(swap_id)["state"], SwapState.LN_SETTLED.value
                )
                self.assertEqual(
                    restarted.load_secret(
                        swap_id, "invoice_preimage"
                    ).reveal_for_protocol(),
                    secret.reveal_for_protocol(),
                )
                pending = restarted.pending_side_effects()
                self.assertEqual(
                    [(effect["effect_key"], effect["kind"]) for effect in pending],
                    [("finish:61", "PFTL_ESCROW_FINISH")],
                )
                self.assertEqual(pending[0]["payload"], finish_operation)

                # Crash/restart replay cannot duplicate state, secret, or intent.
                replayed = service.mark_ln_settled(
                    swap_id,
                    settlement_evidence=settlement_evidence,
                    learned_secret=secret,
                    effect_key="finish:61",
                    finish_operation=finish_operation,
                )
                self.assertEqual(replayed["state_version"], 4)
                self.assertEqual(len(restarted.pending_side_effects()), 1)
                self.assertNotIn(
                    secret.protocol_hex(),
                    json.dumps(restarted.export_public_audit(), sort_keys=True),
                )

    def test_finish_intent_collision_rolls_back_secret_and_ln_settled(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "service-finish-rollback.sqlite3"
            envelope = envelope_for(62, direction="pftl_to_lightning")
            swap_id = envelope["quote"]["swap_id"]
            secret = secret_for(62)
            with CoordinatorJournal(
                path,
                ExposureLimits(1_000_000, 1_000_000),
                clock_ns=lambda: 1_000_000,
            ) as journal:
                service = CoordinatorService(journal)
                service.admit_quote("principal", envelope)
                service.mark_lock_submitted(
                    swap_id,
                    effect_key="create:62",
                    operation={"escrow_id": envelope["quote"]["expected_escrow_id"]},
                )
                service.mark_lock_final(
                    swap_id, finality_evidence={"accepted": True}
                )
                service.mark_ln_in_flight(
                    swap_id,
                    payment_evidence={
                        "payment_hash": envelope["quote"]["payment_hash"]
                    },
                    effect_key="collision:62",
                    payment_request={"invoice": envelope["quote"]["invoice"]},
                )
                with self.assertRaises(IdempotencyConflict):
                    service.mark_ln_settled(
                        swap_id,
                        settlement_evidence={"status": "SUCCEEDED"},
                        learned_secret=secret,
                        effect_key="collision:62",
                        finish_operation={
                            "escrow_id": envelope["quote"]["expected_escrow_id"],
                            "secret_ref": "invoice_preimage",
                        },
                    )
                self.assertEqual(
                    journal.get_swap(swap_id)["state"],
                    SwapState.LN_IN_FLIGHT.value,
                )
                with self.assertRaises(JournalError):
                    journal.load_secret(swap_id, "invoice_preimage")
                self.assertEqual(len(journal.events(swap_id)), 4)

    def test_finish_intent_arguments_are_all_or_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "service-finish-args.sqlite3"
            with CoordinatorJournal(
                path, ExposureLimits(1_000_000, 1_000_000)
            ) as journal:
                service = CoordinatorService(journal)
                with self.assertRaises(ValueError):
                    service.mark_ln_settled(
                        "unused",
                        settlement_evidence={},
                        effect_key="finish:unused",
                    )
