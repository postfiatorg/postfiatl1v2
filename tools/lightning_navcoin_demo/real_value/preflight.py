"""Composed, secret-free mainnet and PFTL readiness report."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from ..coordinator.lnd_grpc import LndGrpcAdapter, LndNodePreflight
from .budget import RealValueBudget
from .pftl_quorum import PftlQuorumObserver, PftlRouteSnapshot
from .policy import ExecutionMode, RealValuePolicy, RealValuePolicyError


class MainnetPreflightError(RealValuePolicyError):
    """One or more mandatory no-spend gates are not green."""


@dataclass(frozen=True)
class MainnetPreflight:
    policy: RealValuePolicy
    lnd: LndNodePreflight
    pftl: PftlRouteSnapshot
    budget: dict[str, Any]
    direction: str
    amount_msat: int
    execution_enabled: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "postfiat.lightning_mainnet_preflight.v1",
            "status": "GREEN" if self.execution_enabled else "HOLD",
            "execution_enabled": self.execution_enabled,
            "direction": self.direction,
            "amount_msat": self.amount_msat,
            "claim": self.policy.atomicity_claim,
            "trust_class": self.policy.trust_class,
            "policy": self.policy.public_status(),
            "lnd": {
                "identity_pubkey": self.lnd.node.identity_pubkey,
                "alias": self.lnd.node.alias,
                "network": self.lnd.node.network,
                "block_height": self.lnd.node.block_height,
                "synced_to_chain": self.lnd.node.synced_to_chain,
                "synced_to_graph": self.lnd.node.synced_to_graph,
                "version": self.lnd.node.version,
                "commit_hash": self.lnd.node.commit_hash,
                "total_channels": self.lnd.liquidity.total_channels,
                "active_channels": self.lnd.liquidity.active_channels,
                "unconfirmed_active_channels": (
                    self.lnd.liquidity.unconfirmed_active_channels
                ),
                "inbound_msat": self.lnd.liquidity.inbound_msat,
                "outbound_msat": self.lnd.liquidity.outbound_msat,
            },
            "pftl": self.pftl.to_dict(),
            "budget": self.budget,
            "hold_reasons": (
                []
                if self.execution_enabled
                else ["real_value_policy_mode_is_dry_run"]
            ),
        }


def run_preflight(
    policy: RealValuePolicy,
    *,
    lnd: LndGrpcAdapter,
    pftl: PftlQuorumObserver,
    budget: RealValueBudget,
    direction: str,
    amount_msat: int,
) -> MainnetPreflight:
    """Read both ledgers and prove capacity; performs no external mutation."""

    if direction not in {"lightning_to_pftl", "pftl_to_lightning"}:
        raise MainnetPreflightError("unsupported swap direction")
    if type(amount_msat) is not int or amount_msat <= 0:
        raise MainnetPreflightError("amount_msat must be positive")
    all_in = amount_msat + policy.max_fee_msat
    if all_in > (1 << 63) - 1:
        raise MainnetPreflightError("all-in amount exceeds uint63")
    lnd_preflight = lnd.preflight_node(
        expected_identity_pubkey=policy.expected_lnd_pubkey,
        min_active_channels=1,
        min_inbound_msat=(
            amount_msat if direction == "lightning_to_pftl" else 0
        ),
        min_outbound_msat=(
            all_in if direction == "pftl_to_lightning" else 0
        ),
    )
    pftl_snapshot = pftl.route_snapshot()
    if (
        direction == "lightning_to_pftl"
        and pftl_snapshot.coordinator_inventory_atoms <= 0
    ):
        raise MainnetPreflightError("coordinator has no escrowable NAVcoin inventory")
    budget_summary = budget.summary()
    if budget_summary["remaining_usd_e8"] <= 0:
        raise MainnetPreflightError("real-value lifetime budget is exhausted")
    return MainnetPreflight(
        policy=policy,
        lnd=lnd_preflight,
        pftl=pftl_snapshot,
        budget=budget_summary,
        direction=direction,
        amount_msat=amount_msat,
        execution_enabled=policy.mode is ExecutionMode.ARMED,
    )
