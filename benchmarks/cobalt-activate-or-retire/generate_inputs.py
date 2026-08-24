#!/usr/bin/env python3
"""Generate unscored inputs for the independent Cobalt decision oracle."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

SCHEMA = "postfiat-cobalt-decisive-input-v1"


def ids(count: int) -> list[str]:
    return [f"validator-{index:02d}" for index in range(count)]


def root(label: str) -> str:
    return hashlib.sha384(f"postfiat-cobalt-decisive:{label}".encode()).hexdigest()


def subset(validators: list[str], quorum: int, max_active_byzantine: int) -> dict[str, Any]:
    return {
        "validators": sorted(validators),
        "quorum": quorum,
        "max_active_byzantine": max_active_byzantine,
    }


def event(**overrides: bool) -> dict[str, bool]:
    value = {
        "delayed": False,
        "duplicated": False,
        "reordered": False,
        "stale_replay": False,
        "recover_unavailable": False,
    }
    value.update(overrides)
    return value


def transition(kind: str = "none", *, removed: list[str] | None = None, added: list[str] | None = None, rotated: list[str] | None = None) -> dict[str, Any]:
    return {
        "kind": kind,
        "removed": sorted(removed or []),
        "added": sorted(added or []),
        "rotated": sorted(rotated or []),
    }


def canonical_views(validators: list[str], quorum: int, max_active_byzantine: int) -> dict[str, Any]:
    shared = subset(validators, quorum, max_active_byzantine)
    return {node: {"essential_subsets": [shared]} for node in validators}


def canonical_unls(validators: list[str]) -> dict[str, list[str]]:
    return {node: validators[:] for node in validators}


def local_quorums(unls: dict[str, list[str]]) -> dict[str, int]:
    return {node: (4 * len(unl) + 4) // 5 for node, unl in sorted(unls.items())}


def scenario(
    case_id: str,
    fault_class: str,
    validators: list[str],
    trust_views: dict[str, Any],
    proposals: list[dict[str, Any]],
    *,
    correct_nodes: list[str] | None = None,
    unavailable: list[str] | None = None,
    actively_byzantine: list[str] | None = None,
    local_unls: dict[str, list[str]] | None = None,
    local_quorum_override: dict[str, int] | None = None,
    event_schedule: dict[str, bool] | None = None,
    transition_input: dict[str, Any] | None = None,
) -> dict[str, Any]:
    validators = sorted(validators)
    unls = local_unls or canonical_unls(validators)
    quorums = local_quorum_override or local_quorums(unls)
    return {
        "id": case_id,
        "fault_class": fault_class,
        "validators": validators,
        "correct_nodes": sorted(correct_nodes or validators),
        "unavailable": sorted(unavailable or []),
        "actively_byzantine": sorted(actively_byzantine or []),
        "trust_views": {node: trust_views[node] for node in sorted(trust_views)},
        "local_unls": {node: sorted(unls[node]) for node in sorted(unls)},
        "local_quorums": {node: quorums[node] for node in sorted(quorums)},
        "proposals": [
            {"registry_root": proposal["registry_root"], "supporters": sorted(proposal["supporters"])}
            for proposal in proposals
        ],
        "event_schedule": event_schedule or event(),
        "transition": transition_input or transition(),
    }


def nonuniform_six_views(validators: list[str]) -> dict[str, Any]:
    core = subset(validators[:4], 3, 0)
    left = subset(validators[:5], 4, 0)
    right = subset(validators[:4] + [validators[5]], 4, 0)
    views: dict[str, Any] = {}
    for index, node in enumerate(validators):
        extra = left if index % 2 == 0 else right
        views[node] = {"essential_subsets": [core, extra]}
    return views


def split_six(validators: list[str]) -> tuple[dict[str, Any], dict[str, list[str]], dict[str, int]]:
    left = validators[:3]
    right = validators[3:]
    views = {
        node: {"essential_subsets": [subset(left if node in left else right, 3, 0)]}
        for node in validators
    }
    unls = {node: (left[:] if node in left else right[:]) for node in validators}
    quorums = {node: 3 for node in validators}
    return views, unls, quorums


def overlap_ninety_twenty(validators: list[str], *, supplemental: bool = True) -> tuple[dict[str, Any], dict[str, list[str]]]:
    core = validators[:18]
    left = sorted(core + [validators[18]])
    right = sorted(core + [validators[19]])
    common = subset(core, 15, 2)
    left_subset = subset(left, 16, 2)
    right_subset = subset(right, 16, 2)
    views: dict[str, Any] = {}
    unls: dict[str, list[str]] = {}
    for index, node in enumerate(validators):
        if index == 18:
            views[node] = {"essential_subsets": [common, left_subset] if supplemental else [common]}
            unls[node] = left
        elif index == 19:
            views[node] = {"essential_subsets": [common, right_subset] if supplemental else [common]}
            unls[node] = right
        else:
            views[node] = {"essential_subsets": [common]}
            unls[node] = validators[:]
    return views, unls


def build_cases() -> list[dict[str, Any]]:
    six = ids(6)
    root_a = root("root-a")
    root_b = root("root-b")
    canonical = canonical_views(six, 5, 1)
    nonuniform = nonuniform_six_views(six)
    cases = [
        scenario("six-identical-control", "identical_control", six, canonical, [{"registry_root": root_a, "supporters": six}]),
        scenario("six-compatible-nonuniform", "compatible_nonuniform", six, nonuniform, [{"registry_root": root_a, "supporters": six}]),
        scenario("six-strong-support-boundary", "support_boundary", six, canonical, [{"registry_root": root_a, "supporters": six[:5]}]),
        scenario("six-below-strong-support", "support_boundary", six, canonical, [{"registry_root": root_a, "supporters": six[:4]}]),
        scenario("six-one-unavailable", "one_unavailable", six, canonical, [{"registry_root": root_a, "supporters": six[:5]}], unavailable=[six[5]]),
        scenario("six-two-unavailable", "two_unavailable", six, canonical, [{"registry_root": root_a, "supporters": six[:4]}], unavailable=six[4:]),
        scenario("six-delay-duplicate-reorder", "network_schedule", six, nonuniform, [{"registry_root": root_a, "supporters": six}], event_schedule=event(delayed=True, duplicated=True, reordered=True)),
        scenario("six-stale-replay", "stale_replay", six, nonuniform, [{"registry_root": root_a, "supporters": six}], event_schedule=event(stale_replay=True)),
        scenario("six-one-equivocator", "equivocation", six, canonical, [{"registry_root": root_a, "supporters": six[:5]}, {"registry_root": root_b, "supporters": [six[5]]}], correct_nodes=six[:5], actively_byzantine=[six[5]]),
    ]

    split_views, split_unls, split_quorums = split_six(six)
    cases.extend([
        scenario("six-divergent-local-quorums", "divergent_proposals", six, split_views, [{"registry_root": root_a, "supporters": six[:3]}, {"registry_root": root_b, "supporters": six[3:]}], local_unls=split_unls, local_quorum_override=split_quorums),
        scenario("six-unlinked-common-proposal", "insufficient_linkage", six, split_views, [{"registry_root": root_a, "supporters": six}], local_unls=split_unls, local_quorum_override=split_quorums),
    ])

    twenty = ids(20)
    views_90, unls_90 = overlap_ninety_twenty(twenty)
    cases.extend([
        scenario("twenty-overlap-090-compatible", "overlap_090", twenty, views_90, [{"registry_root": root_a, "supporters": twenty}], local_unls=unls_90),
        scenario("twenty-overlap-090-support-boundary", "overlap_090_boundary", twenty, views_90, [{"registry_root": root_a, "supporters": twenty[:18]}], local_unls=unls_90),
        scenario("twenty-overlap-090-below-boundary", "overlap_090_boundary", twenty, views_90, [{"registry_root": root_a, "supporters": twenty[:14]}], local_unls=unls_90),
    ])

    cases.extend([
        scenario("six-future-view-drift", "trust_view_transition", six, nonuniform, [{"registry_root": root_a, "supporters": six}], transition_input=transition("view_drift")),
        scenario("six-validator-add-remove", "validator_add_remove", six, nonuniform, [{"registry_root": root_a, "supporters": six}], transition_input=transition("membership", removed=[six[-1]], added=["candidate-06"])),
        scenario("six-key-rotation", "key_rotation", six, nonuniform, [{"registry_root": root_a, "supporters": six}], transition_input=transition("key_rotation", rotated=[six[-1]])),
        scenario("six-missed-history-recovery", "history_recovery", six, canonical, [{"registry_root": root_a, "supporters": six}], unavailable=[six[-1]], event_schedule=event(recover_unavailable=True)),
    ])
    return cases


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    payload = {
        "schema": SCHEMA,
        "source_pins": {
            "cobalt_paper": "arxiv:1802.07240v1",
            "locked_research_spec_sha256": "c7d7b70f9991b55e93f95604d13608f42b7d1df3c6c4ab440059423052e1fb25",
            "rippled_version": "3.1.3",
            "rippled_commit": "46b241ace8b30d9c9775d60ffba7d24b21903896",
            "comparison_scope": "validator-governance decision; native RippleD CSF ledger consensus separately labeled",
        },
        "cases": build_cases(),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"schema": SCHEMA, "cases": len(payload["cases"]), "output": str(args.output)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
