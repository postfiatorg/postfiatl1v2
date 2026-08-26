#!/usr/bin/env python3
"""Focused regression tests for the E4 paired-lane safety comparator."""

from __future__ import annotations

import importlib.util
from pathlib import Path


SCRIPT = Path(__file__).with_name("run_consensus_v2_cobalt_integration.py")
SPEC = importlib.util.spec_from_file_location("e4_integration", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def statuses(tip: str, root: str) -> list[dict[str, object]]:
    return [
        {
            "node_id": f"validator-{index}",
            "block_height": 501,
            "block_tip_hash": tip,
            "state_root": root,
        }
        for index in range(6)
    ]


def report(tx_prefix: str, block_prefix: str) -> dict[str, object]:
    return {
        "config": {
            "validators": 6,
            "rounds": 2,
            "vote_policy": "full",
            "wallet_address": "wallet",
            "recipient": "recipient",
            "amount": 10,
        },
        "iterations": [
            {
                "iteration": iteration,
                "source_node": f"validator-{iteration - 1}",
                "tx_id": f"{tx_prefix}-{iteration}",
                "block_height": iteration + 1,
                "block_hash": f"{block_prefix}-{iteration}",
                "certificate_id": f"certificate-{tx_prefix}-{iteration}",
                "vote_policy": "full",
                "validators": 6,
                "quorum": 5,
                "vote_count": 6,
                "receipt_accepted": True,
                "finality_confirmed": True,
                "round_ok": True,
                "all_vote_requests_verified": True,
                "all_sends_verified": True,
                "wallet_to_finality_ms": 1.0 + iteration,
            }
            for iteration in (1, 2)
        ],
    }


baseline_statuses = statuses("baseline-tip", "baseline-root")
attack_statuses = statuses("attack-tip", "attack-root")
assert MODULE.fleet_converged(baseline_statuses)
assert MODULE.fleet_converged(attack_statuses)

# Randomized signatures may change every identity-bearing hash while the
# unsigned workload and all consensus outcomes remain identical.
assert MODULE.benchmark_workload_fingerprint(
    report("baseline-tx", "baseline-block")
) == MODULE.benchmark_workload_fingerprint(report("attack-tx", "attack-block"))

# A real within-lane durable divergence remains a hard failure.
divergent = statuses("attack-tip", "attack-root")
divergent[-1]["state_root"] = "forked-root"
assert not MODULE.fleet_converged(divergent)

# A changed semantic workload remains detectable.
changed = report("attack-tx", "attack-block")
changed["iterations"][1]["source_node"] = "validator-5"
assert MODULE.benchmark_workload_fingerprint(
    report("baseline-tx", "baseline-block")
) != MODULE.benchmark_workload_fingerprint(changed)

print("e4-final-state-comparator-tests-ok")
