#!/usr/bin/env python3
"""Generate the deterministic matched Cobalt/RippleD scenario contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any

SCHEMA = "postfiat-cobalt-rippled-scenario-manifest-v1"
OVERLAP_TARGETS = (100, 90, 80, 60, 40, 20, 10, 0)
SCALE_COUNTS = (6, 7, 10, 20)


def validators(count: int) -> list[str]:
    return [f"validator-{index}" for index in range(count)]


def quorum(count: int) -> int:
    return math.ceil(0.8 * count)


def correlations(count: int, live: bool) -> dict[str, list[list[str]]]:
    ids = validators(count)
    if live:
        return {
            "provider": [ids],
            "region": [[ids[0], ids[3]], [ids[1], ids[4]], [ids[2], ids[5]]],
            "operator": [ids],
            "key_custody": [[validator] for validator in ids],
        }
    regions = [ids[index::3] for index in range(3)]
    return {
        "provider": [ids[::2], ids[1::2]],
        "region": [group for group in regions if group],
        "operator": [ids[: math.ceil(count / 2)], ids[math.ceil(count / 2) :]],
        "key_custody": [[validator] for validator in ids],
    }


def canonical_unls(ids: list[str]) -> dict[str, list[str]]:
    return {validator: ids[:] for validator in ids}


def overlap_unls(ids: list[str], overlap: int) -> tuple[dict[str, list[str]], dict[str, Any]]:
    count = len(ids)
    a_only_count = (count - overlap) // 2
    b_only_count = count - a_only_count - overlap
    a_only = ids[:a_only_count]
    b_only = ids[a_only_count : a_only_count + b_only_count]
    common = ids[a_only_count + b_only_count :]
    unl_a = sorted(a_only + common)
    unl_b = sorted(b_only + common)
    union = sorted(set(unl_a) | set(unl_b))
    views: dict[str, list[str]] = {}
    for validator in a_only:
        views[validator] = unl_a
    for validator in b_only:
        views[validator] = unl_b
    for validator in common:
        views[validator] = union
    return views, {
        "a_only": a_only,
        "b_only": b_only,
        "common": common,
        "unl_a": unl_a,
        "unl_b": unl_b,
        "intersection": sorted(set(unl_a) & set(unl_b)),
        "intersection_count": len(set(unl_a) & set(unl_b)),
        "union_count": len(set(unl_a) | set(unl_b)),
    }


def local_quorums(local_unls: dict[str, list[str]]) -> dict[str, int]:
    # This is the ordinary ValidatorList::calculateQuorum branch with
    # effectiveUNL == local UNL: max(ceil(0.8*effective), ceil(0.6*local)).
    return {
        validator: max(math.ceil(0.8 * len(unl)), math.ceil(0.6 * len(unl)))
        for validator, unl in sorted(local_unls.items())
    }


def base_case(topology: dict[str, Any], case_id: str, fault_class: str) -> dict[str, Any]:
    ids = topology["validators"]
    unls = canonical_unls(ids)
    return {
        "id": case_id,
        "topology_id": topology["id"],
        "fault_class": fault_class,
        "validators": ids,
        "local_unls": unls,
        "local_quorums": local_quorums(unls),
        "essential_subset_quorum": topology["quorum"],
        "declared_byzantine_budget": topology["declared_byzantine_budget"],
        "correlation_groups": topology["correlation_groups"],
        "faults": {
            "offline": [],
            "actively_byzantine": [],
            "censored": [],
            "equivocal": [],
            "partitions": [],
            "heal_at_ms": None,
            "latency_ms": {"base": 25, "jitter": 5},
            "packet_loss_every": 0,
            "duplicate_every": 0,
            "reorder_every": 0,
            "reorder_extra_ms": 0,
        },
        "transition": {"kind": "none", "removed": [], "added": [], "rotated": []},
        "seed": int(hashlib.sha256(case_id.encode()).hexdigest()[:8], 16),
        "repetitions": 3,
        "timeout_ms": 90_000,
        "expected": {
            "model_scope": "inside",
            "pre_heal": "one_decision",
            "post_heal": "one_decision",
            "conflicting_decisions": 0,
        },
    }


def fixed_cases(topology: dict[str, Any]) -> list[dict[str, Any]]:
    tid = topology["id"]
    ids = topology["validators"]
    result: list[dict[str, Any]] = []

    result.append(base_case(topology, f"{tid}--no-fault", "no_fault"))

    case = base_case(topology, f"{tid}--one-offline", "one_fault")
    case["faults"]["offline"] = [ids[-1]]
    result.append(case)

    case = base_case(topology, f"{tid}--within-budget-byzantine", "within_budget")
    case["faults"]["actively_byzantine"] = [ids[-1]]
    case["faults"]["censored"] = [ids[-1]]
    result.append(case)

    case = base_case(topology, f"{tid}--beyond-budget-offline", "beyond_budget")
    offline_count = topology["declared_byzantine_budget"] + 1
    case["faults"]["offline"] = ids[-offline_count:]
    case["expected"].update({"model_scope": "outside", "pre_heal": "safe_halt", "post_heal": "safe_halt"})
    result.append(case)

    case = base_case(topology, f"{tid}--correlated-region-loss", "correlated_loss")
    lost = topology["correlation_groups"]["region"][0]
    case["faults"]["offline"] = lost
    case["expected"].update({"model_scope": "outside" if len(lost) > topology["declared_byzantine_budget"] else "inside", "pre_heal": "safe_halt", "post_heal": "safe_halt"})
    result.append(case)

    case = base_case(topology, f"{tid}--partition-heal", "partition_heal")
    split = len(ids) // 2
    case["faults"]["partitions"] = [ids[:split], ids[split:]]
    case["faults"]["heal_at_ms"] = 15_000
    case["expected"].update({"pre_heal": "safe_halt", "post_heal": "one_decision"})
    result.append(case)

    case = base_case(topology, f"{tid}--delay-loss-duplicate-reorder", "network_faults")
    case["faults"]["latency_ms"] = {"base": 120, "jitter": 80}
    case["faults"]["packet_loss_every"] = 11
    case["faults"]["duplicate_every"] = 7
    case["faults"]["reorder_every"] = 5
    case["faults"]["reorder_extra_ms"] = 250
    result.append(case)

    case = base_case(topology, f"{tid}--asymmetric-views", "asymmetric_views")
    views, detail = overlap_unls(ids, max(1, math.ceil(0.8 * len(ids))))
    case["local_unls"] = views
    case["local_quorums"] = local_quorums(views)
    case["view_detail"] = detail
    case["expected"].update({"model_scope": "characterize"})
    result.append(case)

    case = base_case(topology, f"{tid}--publisher-list-drift", "list_graph_drift")
    old = ids[:-1]
    views = {validator: (old[:] if index < len(ids) // 2 else ids[:]) for index, validator in enumerate(ids)}
    for validator in ids:
        if validator not in views[validator]:
            views[validator] = sorted(views[validator] + [validator])
    case["local_unls"] = views
    case["local_quorums"] = local_quorums(views)
    case["transition"] = {"kind": "view_drift", "removed": [], "added": [], "rotated": []}
    case["expected"].update({"model_scope": "characterize"})
    result.append(case)

    case = base_case(topology, f"{tid}--validator-add-remove", "validator_add_remove")
    replacement = f"candidate-{len(ids)}"
    new_ids = ids[:-1] + [replacement]
    case["validators"] = new_ids
    case["local_unls"] = canonical_unls(new_ids)
    case["local_quorums"] = local_quorums(case["local_unls"])
    case["transition"] = {"kind": "membership", "removed": [ids[-1]], "added": [replacement], "rotated": []}
    result.append(case)

    case = base_case(topology, f"{tid}--key-rotation", "key_rotation")
    case["transition"] = {"kind": "key_rotation", "removed": [], "added": [], "rotated": [ids[-1]]}
    result.append(case)

    case = base_case(topology, f"{tid}--equivocation", "equivocation")
    case["faults"]["actively_byzantine"] = [ids[0]]
    case["faults"]["equivocal"] = [ids[0]]
    case["expected"].update({"pre_heal": "one_decision_or_safe_halt", "post_heal": "one_decision_or_safe_halt"})
    result.append(case)
    return result


def overlap_cases(topology: dict[str, Any]) -> list[dict[str, Any]]:
    ids = topology["validators"]
    cases: list[dict[str, Any]] = []
    seen: dict[int, int] = {}
    for target in OVERLAP_TARGETS:
        actual = round(len(ids) * target / 100)
        ordinal = seen.get(actual, 0)
        seen[actual] = ordinal + 1
        suffix = f"-{ordinal + 1}" if ordinal else ""
        case = base_case(topology, f"{topology['id']}--overlap-{target:03d}{suffix}", "overlap_sweep")
        views, detail = overlap_unls(ids, actual)
        detail["target_percent_of_union"] = target
        detail["actual_percent_of_union"] = round(100 * actual / len(ids), 3)
        case["local_unls"] = views
        case["local_quorums"] = local_quorums(views)
        case["view_detail"] = detail
        case["expected"].update({"model_scope": "characterize"})
        cases.append(case)
    return cases


def build_manifest() -> dict[str, Any]:
    topologies = []
    cases = []
    for count in SCALE_COUNTS:
        live = count == 6
        topology = {
            "id": "live-six" if live else f"control-{count}",
            "kind": "fresh-live-fleet-receipt" if live else "deterministic-scale-control",
            "validators": validators(count),
            "quorum": quorum(count),
            "declared_byzantine_budget": count - quorum(count),
            "correlation_groups": correlations(count, live),
        }
        if live:
            topology["provenance"] = {
                "receipt": ".tih/cobalt-sibling-baseline-20260823.json",
                "captured_at": "2026-08-23T12:04:53Z",
                "provider": "vultr",
                "regions": ["ewr", "ams", "sgp"],
                "region_assignment": {"ewr": ["validator-0", "validator-3"], "ams": ["validator-1", "validator-4"], "sgp": ["validator-2", "validator-5"]},
                "operator": "postfiat-operated",
                "custody": "validator-local-key-files",
                "provider_refresh": "failed-closed-http-401-current-host-not-in-source-ip-allowlist",
            }
        topologies.append(topology)
        cases.extend(fixed_cases(topology))
        cases.extend(overlap_cases(topology))
    manifest = {
        "schema": SCHEMA,
        "description": "Simulator-to-simulator validator-governance liveness comparison; never a payment-latency or mainnet comparison.",
        "source_pins": {
            "postfiatl1v2_section4_commit": "38ddb4f667669ae191f038102566ea16e635b649",
            "rippled_version": "3.1.3",
            "rippled_commit": "46b241ace8b30d9c9775d60ffba7d24b21903896",
            "rippled_native_control": "src/test/csf and Consensus_test::testFork",
            "agti_downstream_control": "report-derived downstream overlap scenarios; not upstream XRPLF",
            "agti_report_commit": "81f6a7e8d6e0da8c2ab334209c133e85e617e6e2",
            "agti_report_path": "_posts/2026-05-26-xrpl-rippled-open-p0-freeze-audit.md",
            "agti_report_section": "The Inherited Trust Model / UNL overlap implosion",
        },
        "topologies": topologies,
        "cases": cases,
    }
    canonical = json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    manifest["manifest_sha256"] = hashlib.sha256(canonical).hexdigest()
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    manifest = build_manifest()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"schema": manifest["schema"], "cases": len(manifest["cases"]), "sha256": manifest["manifest_sha256"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
