"""Typed coordinator service façade over the durable journal."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .journal import CoordinatorJournal, SideEffectSpec, SwapState
from .protocol import SecretPreimage


@dataclass(frozen=True)
class RecoveryAction:
    swap_id: str
    state: SwapState
    action: str
    pending_effect_keys: tuple[str, ...]


RECOVERY_ACTIONS: dict[SwapState, str] = {
    SwapState.QUOTED: "submit_or_observe_pftl_lock",
    SwapState.PFTL_LOCK_SUBMITTED: "observe_pftl_lock_finality",
    SwapState.PFTL_LOCK_FINAL: "start_or_observe_lightning_payment",
    SwapState.LN_IN_FLIGHT: "reconcile_lightning_by_payment_hash",
    SwapState.LN_SETTLED: "submit_or_observe_pftl_finish",
    SwapState.REFUND_ELIGIBLE: "submit_or_observe_pftl_cancel",
}


class CoordinatorService:
    """Stable orchestration contract used by the synthetic E2E harness."""

    def __init__(self, journal: CoordinatorJournal) -> None:
        self.journal = journal

    def admit_quote(
        self,
        principal: str,
        signed_quote: Mapping[str, Any],
        *,
        expected_public_key: bytes | None = None,
        coordinator_secret: SecretPreimage | None = None,
    ) -> dict[str, Any]:
        return self.journal.create_swap(
            principal,
            signed_quote,
            expected_public_key=expected_public_key,
            secret=coordinator_secret,
        )

    def mark_lock_submitted(
        self,
        swap_id: str,
        *,
        effect_key: str,
        operation: Mapping[str, Any],
    ) -> dict[str, Any]:
        return self.journal.advance(
            swap_id,
            SwapState.PFTL_LOCK_SUBMITTED,
            f"state:{swap_id}:pftl_lock_submitted",
            evidence={"intent_durable": True},
            side_effect=SideEffectSpec(effect_key, "PFTL_ESCROW_CREATE", operation),
        )

    def mark_lock_final(
        self, swap_id: str, *, finality_evidence: Mapping[str, Any]
    ) -> dict[str, Any]:
        return self.journal.advance(
            swap_id,
            SwapState.PFTL_LOCK_FINAL,
            f"state:{swap_id}:pftl_lock_final",
            evidence=finality_evidence,
        )

    def mark_ln_in_flight(
        self,
        swap_id: str,
        *,
        payment_evidence: Mapping[str, Any],
        effect_key: str | None = None,
        payment_request: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        side_effect = None
        if effect_key is not None:
            if payment_request is None:
                raise ValueError("outgoing Lightning effect requires request fields")
            side_effect = SideEffectSpec(effect_key, "LND_SEND_PAYMENT", payment_request)
        return self.journal.advance(
            swap_id,
            SwapState.LN_IN_FLIGHT,
            f"state:{swap_id}:ln_in_flight",
            evidence=payment_evidence,
            side_effect=side_effect,
        )

    def mark_ln_settled(
        self,
        swap_id: str,
        *,
        settlement_evidence: Mapping[str, Any],
        learned_secret: SecretPreimage | None = None,
        effect_key: str | None = None,
        finish_operation: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        if (effect_key is None) != (finish_operation is None):
            raise ValueError(
                "PFTL finish intent requires both effect_key and finish_operation"
            )
        finish_effect = (
            None
            if effect_key is None
            else SideEffectSpec(
                effect_key, "PFTL_ESCROW_FINISH", finish_operation
            )
        )
        return self.journal.advance(
            swap_id,
            SwapState.LN_SETTLED,
            f"state:{swap_id}:ln_settled",
            evidence=settlement_evidence,
            side_effect=finish_effect,
            secret_write=(
                None
                if learned_secret is None
                else ("invoice_preimage", learned_secret)
            ),
        )

    def mark_finish_final(
        self, swap_id: str, *, finality_evidence: Mapping[str, Any]
    ) -> dict[str, Any]:
        return self.journal.advance(
            swap_id,
            SwapState.PFTL_FINISH_FINAL,
            f"state:{swap_id}:pftl_finish_final",
            evidence=finality_evidence,
        )

    def mark_refund_eligible(
        self,
        swap_id: str,
        *,
        reason_evidence: Mapping[str, Any],
        effect_key: str,
        cancel_operation: Mapping[str, Any],
    ) -> dict[str, Any]:
        return self.journal.advance(
            swap_id,
            SwapState.REFUND_ELIGIBLE,
            f"state:{swap_id}:refund_eligible",
            evidence=reason_evidence,
            side_effect=SideEffectSpec(
                effect_key, "PFTL_ESCROW_CANCEL", cancel_operation
            ),
        )

    def mark_cancel_final(
        self, swap_id: str, *, finality_evidence: Mapping[str, Any]
    ) -> dict[str, Any]:
        return self.journal.advance(
            swap_id,
            SwapState.PFTL_CANCEL_FINAL,
            f"state:{swap_id}:pftl_cancel_final",
            evidence=finality_evidence,
        )

    def recovery_plan(self) -> list[RecoveryAction]:
        pending_by_swap: dict[str, list[str]] = {}
        for effect in self.journal.pending_side_effects():
            pending_by_swap.setdefault(effect["swap_id"], []).append(
                effect["effect_key"]
            )
        actions: list[RecoveryAction] = []
        for swap in self.journal.recoverable_swaps():
            state = SwapState(swap["state"])
            actions.append(
                RecoveryAction(
                    swap_id=swap["swap_id"],
                    state=state,
                    action=RECOVERY_ACTIONS[state],
                    pending_effect_keys=tuple(
                        sorted(pending_by_swap.get(swap["swap_id"], ()))
                    ),
                )
            )
        return actions
