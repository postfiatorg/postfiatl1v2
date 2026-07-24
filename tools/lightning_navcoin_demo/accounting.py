"""Exact conservation and conditional-atomicity assertions for demo evidence."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
from typing import Mapping, Sequence


class InvariantViolation(AssertionError):
    """A claimed ledger transition does not match its exact integer deltas."""


def _amount(value: int, name: str) -> int:
    if type(value) is not int or value < 0:
        raise InvariantViolation(f"{name} must be a nonnegative integer")
    return value


@dataclass(frozen=True)
class PftlAssetState:
    """Consensus money fields only; height/root are checked separately."""

    owner_spendable_atoms: int
    recipient_spendable_atoms: int
    open_escrow_atoms: int
    issued_supply_atoms: int
    escrow_state: str | None

    def __post_init__(self) -> None:
        _amount(self.owner_spendable_atoms, "owner_spendable_atoms")
        _amount(self.recipient_spendable_atoms, "recipient_spendable_atoms")
        _amount(self.open_escrow_atoms, "open_escrow_atoms")
        _amount(self.issued_supply_atoms, "issued_supply_atoms")
        if self.escrow_state not in {None, "OPEN", "FINISHED", "CANCELED"}:
            raise InvariantViolation("unsupported escrow state")

    def money_tuple(self) -> tuple[int, int, int, int]:
        return (
            self.owner_spendable_atoms,
            self.recipient_spendable_atoms,
            self.open_escrow_atoms,
            self.issued_supply_atoms,
        )


def _delta(before: int, after: int) -> int:
    return after - before


def assert_create_delta(
    before: PftlAssetState,
    after: PftlAssetState,
    *,
    principal_atoms: int,
) -> dict[str, int]:
    principal = _amount(principal_atoms, "principal_atoms")
    expected = {
        "owner_spendable_atoms": -principal,
        "recipient_spendable_atoms": 0,
        "open_escrow_atoms": principal,
        "issued_supply_atoms": 0,
    }
    actual = {
        "owner_spendable_atoms": _delta(
            before.owner_spendable_atoms, after.owner_spendable_atoms
        ),
        "recipient_spendable_atoms": _delta(
            before.recipient_spendable_atoms, after.recipient_spendable_atoms
        ),
        "open_escrow_atoms": _delta(
            before.open_escrow_atoms, after.open_escrow_atoms
        ),
        "issued_supply_atoms": _delta(
            before.issued_supply_atoms, after.issued_supply_atoms
        ),
    }
    if actual != expected or after.escrow_state != "OPEN":
        raise InvariantViolation(
            f"escrow create delta mismatch: actual={actual}, expected={expected}, "
            f"state={after.escrow_state!r}"
        )
    if sum(before.money_tuple()[:3]) != sum(after.money_tuple()[:3]):
        raise InvariantViolation("escrow create did not conserve issued-asset atoms")
    return actual


def assert_finish_delta(
    before: PftlAssetState,
    after: PftlAssetState,
    *,
    principal_atoms: int,
) -> dict[str, int]:
    principal = _amount(principal_atoms, "principal_atoms")
    expected = {
        "owner_spendable_atoms": 0,
        "recipient_spendable_atoms": principal,
        "open_escrow_atoms": -principal,
        "issued_supply_atoms": 0,
    }
    actual = {
        "owner_spendable_atoms": _delta(
            before.owner_spendable_atoms, after.owner_spendable_atoms
        ),
        "recipient_spendable_atoms": _delta(
            before.recipient_spendable_atoms, after.recipient_spendable_atoms
        ),
        "open_escrow_atoms": _delta(
            before.open_escrow_atoms, after.open_escrow_atoms
        ),
        "issued_supply_atoms": _delta(
            before.issued_supply_atoms, after.issued_supply_atoms
        ),
    }
    if actual != expected or before.escrow_state != "OPEN" or after.escrow_state != "FINISHED":
        raise InvariantViolation(
            f"escrow finish delta mismatch: actual={actual}, expected={expected}, "
            f"states={before.escrow_state!r}->{after.escrow_state!r}"
        )
    if sum(before.money_tuple()[:3]) != sum(after.money_tuple()[:3]):
        raise InvariantViolation("escrow finish did not conserve issued-asset atoms")
    return actual


def assert_cancel_delta(
    before: PftlAssetState,
    after: PftlAssetState,
    *,
    principal_atoms: int,
) -> dict[str, int]:
    principal = _amount(principal_atoms, "principal_atoms")
    expected = {
        "owner_spendable_atoms": principal,
        "recipient_spendable_atoms": 0,
        "open_escrow_atoms": -principal,
        "issued_supply_atoms": 0,
    }
    actual = {
        "owner_spendable_atoms": _delta(
            before.owner_spendable_atoms, after.owner_spendable_atoms
        ),
        "recipient_spendable_atoms": _delta(
            before.recipient_spendable_atoms, after.recipient_spendable_atoms
        ),
        "open_escrow_atoms": _delta(
            before.open_escrow_atoms, after.open_escrow_atoms
        ),
        "issued_supply_atoms": _delta(
            before.issued_supply_atoms, after.issued_supply_atoms
        ),
    }
    if actual != expected or before.escrow_state != "OPEN" or after.escrow_state != "CANCELED":
        raise InvariantViolation(
            f"escrow cancel delta mismatch: actual={actual}, expected={expected}, "
            f"states={before.escrow_state!r}->{after.escrow_state!r}"
        )
    if sum(before.money_tuple()[:3]) != sum(after.money_tuple()[:3]):
        raise InvariantViolation("escrow cancel did not conserve issued-asset atoms")
    return actual


def assert_mutation_free_rejection(
    before: PftlAssetState, after: PftlAssetState
) -> None:
    if before != after:
        raise InvariantViolation(
            f"rejected operation mutated money or escrow state: {before!r} -> {after!r}"
        )


def assert_validator_convergence(
    views: Sequence[Mapping[str, object]],
    *,
    declared_validator_count: int = 6,
    required_available: int = 6,
) -> dict[str, object]:
    if declared_validator_count != 6:
        raise InvariantViolation("demo chain must declare exactly six validators")
    if len(views) != required_available:
        raise InvariantViolation(
            f"expected {required_available} validator views, received {len(views)}"
        )
    if required_available < 5 or required_available > 6:
        raise InvariantViolation("available validator threshold must be five or six")
    comparable = []
    for view in views:
        if view.get("validator_count") != 6:
            raise InvariantViolation("validator view does not declare six validators")
        comparable.append(
            (
                view.get("chain_id"),
                view.get("genesis_hash"),
                view.get("block_height"),
                view.get("block_tip_hash"),
                view.get("state_root"),
            )
        )
    if len(set(comparable)) != 1:
        raise InvariantViolation(f"validator views diverged: {comparable!r}")
    chain_id, genesis_hash, height, tip, root = comparable[0]
    if type(height) is not int or height < 0:
        raise InvariantViolation("converged block height is invalid")
    for value, name, expected_length in (
        (genesis_hash, "genesis_hash", 96),
        (tip, "block_tip_hash", 96),
        (root, "state_root", 96),
    ):
        if type(value) is not str or len(value) != expected_length:
            raise InvariantViolation(f"converged {name} is invalid")
    return {
        "chain_id": chain_id,
        "genesis_hash": genesis_hash,
        "block_height": height,
        "block_tip_hash": tip,
        "state_root": root,
        "declared_validators": 6,
        "available_validators": len(views),
    }


@dataclass(frozen=True)
class LightningSettlement:
    payment_hash: str
    payment_preimage: bytes
    invoice_amount_msat: int
    settled_amount_msat: int
    fee_msat: int
    status: str

    def __post_init__(self) -> None:
        _amount(self.invoice_amount_msat, "invoice_amount_msat")
        _amount(self.settled_amount_msat, "settled_amount_msat")
        _amount(self.fee_msat, "fee_msat")
        if type(self.payment_preimage) is not bytes or len(self.payment_preimage) != 32:
            raise InvariantViolation("payment preimage must be exactly 32 bytes")


def assert_lightning_settlement(
    settlement: LightningSettlement,
    *,
    expected_hash: str,
    fee_limit_msat: int,
) -> dict[str, object]:
    fee_limit = _amount(fee_limit_msat, "fee_limit_msat")
    actual_hash = hashlib.sha256(settlement.payment_preimage).hexdigest()
    if settlement.status != "SUCCEEDED":
        raise InvariantViolation(f"Lightning payment is not settled: {settlement.status}")
    if settlement.payment_hash != expected_hash or actual_hash != expected_hash:
        raise InvariantViolation("Lightning preimage/hash linkage failed")
    if settlement.settled_amount_msat != settlement.invoice_amount_msat:
        raise InvariantViolation("Lightning settled amount differs from fixed invoice amount")
    if settlement.fee_msat > fee_limit:
        raise InvariantViolation("Lightning fee exceeds signed limit")
    return {
        "payment_hash": settlement.payment_hash,
        "invoice_amount_msat": settlement.invoice_amount_msat,
        "settled_amount_msat": settlement.settled_amount_msat,
        "fee_msat": settlement.fee_msat,
        "status": settlement.status,
        "payment_preimage": "<redacted>",
    }


def assert_terminal_conditional_atomicity(
    *,
    lightning_settled: bool,
    pftl_escrow_state: str,
) -> str:
    if lightning_settled and pftl_escrow_state == "FINISHED":
        return "BOTH_SETTLED"
    if not lightning_settled and pftl_escrow_state == "CANCELED":
        return "NEITHER_SETTLED"
    raise InvariantViolation(
        "terminal cross-ledger state is not conditionally atomic: "
        f"lightning_settled={lightning_settled}, pftl={pftl_escrow_state}"
    )
