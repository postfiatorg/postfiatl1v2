#!/usr/bin/env python3
"""Deterministic synthetic adversarial simulation for Task Node UNL policy.

The driver supplies synthetic populations and attack edges to the production
Task Node UNL engine. It does not implement the walk, edge weights,
accountability formula, connectivity floor, conductance cuts, or seat cap.
There is no clock, network, database, or model access.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import random
import sys
from contextlib import ExitStack, contextmanager
from dataclasses import dataclass, replace
from datetime import datetime, timedelta, timezone
from fractions import Fraction
from pathlib import Path
from typing import Iterable, Iterator, Mapping, Sequence
from unittest.mock import patch


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
PYTHON_ROOT = REPOSITORY_ROOT / "python"
if str(PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(PYTHON_ROOT))

from postfiat_rpc import tasknode_unl_accountability as accountability  # noqa: E402
from postfiat_rpc import tasknode_unl_edges as edge_engine  # noqa: E402
from postfiat_rpc import tasknode_unl_policy as policy  # noqa: E402
from postfiat_rpc import tasknode_unl_schema as schema  # noqa: E402
from postfiat_rpc import tasknode_unl_trust_graph as trust_graph  # noqa: E402


POPULATION_SEED = 2_026_090_701
ACCOUNTABILITY_SEED = 2_026_090_702
ATTACK_SEED = 2_026_090_703
EVALUATION_END = datetime(2026, 9, 7, tzinfo=timezone.utc)
LIST_SIZE = 20
FOUNDATION_SEATS = 3
COMMUNITIES = 20
ACCOUNTS_PER_COMMUNITY = 12
HONEST_ACCOUNT_COUNT = COMMUNITIES * ACCOUNTS_PER_COMMUNITY
WINDOW = {
    "start": schema.format_utc_timestamp(
        EVALUATION_END - timedelta(days=schema.ACCOUNTABILITY_WINDOW_DAYS)
    ),
    "end": schema.format_utc_timestamp(EVALUATION_END),
    "days": schema.ACCOUNTABILITY_WINDOW_DAYS,
}


@dataclass(frozen=True)
class EngineProfile:
    """One-at-a-time overrides of published engine constants."""

    vouch_weight: Fraction = Fraction(1, 1)
    cowork_weight: Fraction = Fraction(1, 1)
    cowork_cap: int = 3
    funding_weight: Fraction = Fraction(2, 1)
    damping: Fraction = Fraction(85, 100)
    walk_steps: int = 20
    conductance_cut: Fraction = Fraction(1, 10)
    connectivity_divisor: int = 2
    minimum_cluster_seats: int = 2
    cluster_seat_fraction: Fraction = Fraction(1, 10)

    def key(self) -> tuple[object, ...]:
        return (
            self.vouch_weight,
            self.cowork_weight,
            self.cowork_cap,
            self.funding_weight,
            self.damping,
            self.walk_steps,
            self.conductance_cut,
            self.connectivity_divisor,
            self.minimum_cluster_seats,
            self.cluster_seat_fraction,
        )


DEFAULT_PROFILE = EngineProfile()


@dataclass(frozen=True)
class Population:
    nodes: tuple[str, ...]
    ratified: tuple[str, ...]
    foundation: tuple[str, ...]
    edges: tuple[trust_graph.TrustEdge, ...]
    seats: tuple[trust_graph.SeatAssignment, ...]
    accountability_scores: Mapping[str, int]


@dataclass(frozen=True)
class Allocation:
    selected: tuple[str, ...]
    graph: trust_graph.TrustGraphResult
    projections: Mapping[str, Mapping[str, object]]


@contextmanager
def engine_profile(profile: EngineProfile) -> Iterator[None]:
    """Patch constant bindings only; all algorithms remain production code."""

    with ExitStack() as stack:
        for target, name, value in (
            (trust_graph, "VOUCH_EDGE_WEIGHT", profile.vouch_weight),
            (trust_graph, "COWORK_EDGE_WEIGHT", profile.cowork_weight),
            (trust_graph, "COWORK_EDGE_CAP", profile.cowork_cap),
            (trust_graph, "FUNDING_EDGE_WEIGHT", profile.funding_weight),
            (trust_graph, "TRUST_WALK_DAMPING", profile.damping),
            (
                trust_graph,
                "TRUST_WALK_SEED_DAMPING",
                Fraction(1, 1) - profile.damping,
            ),
            (trust_graph, "TRUST_WALK_ITERATIONS", profile.walk_steps),
            (
                trust_graph,
                "CONDUCTANCE_CUT_THRESHOLD",
                profile.conductance_cut,
            ),
            (
                schema,
                "CONNECTIVITY_FLOOR_DIVISOR",
                profile.connectivity_divisor,
            ),
            (
                schema,
                "MIN_CLUSTER_SEATS",
                profile.minimum_cluster_seats,
            ),
            (
                schema,
                "CLUSTER_SEAT_FRACTION",
                profile.cluster_seat_fraction,
            ),
        ):
            stack.enter_context(patch.object(target, name, value))
        yield


def account_id(community: int, member: int) -> str:
    return f"honest-c{community:02d}-n{member:02d}"


def edge(
    source: str,
    target: str,
    kind: str,
    evidence_id: str,
) -> trust_graph.TrustEdge:
    return trust_graph.TrustEdge(source, target, kind, evidence_id)


def canonical_edges(
    values: Iterable[trust_graph.TrustEdge],
) -> tuple[trust_graph.TrustEdge, ...]:
    return tuple(
        sorted(
            set(values),
            key=lambda item: (
                item.kind,
                item.source,
                item.target,
                item.evidence_id,
            ),
        )
    )


def build_honest_edges() -> tuple[trust_graph.TrustEdge, ...]:
    """Build sparse work communities with rare cross-community links."""

    rng = random.Random(POPULATION_SEED)
    values: list[trust_graph.TrustEdge] = []
    for community in range(COMMUNITIES):
        hub = account_id(community, 0)
        central_candidate = account_id(community, 1)
        for unit in range(3):
            values.append(
                edge(
                    hub,
                    central_candidate,
                    "cowork",
                    f"cowork:c{community:02d}:central:{unit:02d}",
                )
            )
        for member in range(2, ACCOUNTS_PER_COMMUNITY):
            successor = 2 + ((member - 1) % (ACCOUNTS_PER_COMMUNITY - 2))
            values.append(
                edge(
                    account_id(community, member),
                    account_id(community, successor),
                    "cowork",
                    f"cowork:c{community:02d}:ring:{member:02d}",
                )
            )
        values.extend(
            (
                edge(
                    hub,
                    central_candidate,
                    "vouch",
                    f"vouch:c{community:02d}:hub-to-central",
                ),
                edge(
                    central_candidate,
                    hub,
                    "vouch",
                    f"vouch:c{community:02d}:central-to-hub",
                ),
            )
        )
        for member in range(2, ACCOUNTS_PER_COMMUNITY):
            node = account_id(community, member)
            target_index = rng.randrange(2, ACCOUNTS_PER_COMMUNITY)
            if target_index == member:
                target_index = 2 + ((target_index - 1) % (ACCOUNTS_PER_COMMUNITY - 2))
            values.append(
                edge(
                    node,
                    hub,
                    "vouch",
                    f"vouch:c{community:02d}:peripheral-hub:{member:02d}",
                )
            )
            values.append(
                edge(
                    node,
                    account_id(community, target_index),
                    "vouch",
                    f"vouch:c{community:02d}:random:{member:02d}",
                )
            )
        funding_members = list(range(1, ACCOUNTS_PER_COMMUNITY))
        funding_members.remove(1)
        rng.shuffle(funding_members)
        for pair_index in range(0, 6, 2):
            source = account_id(community, funding_members[pair_index])
            target = account_id(community, funding_members[pair_index + 1])
            values.append(
                edge(
                    source,
                    target,
                    "funding",
                    f"funding:c{community:02d}:{pair_index // 2:02d}",
                )
            )
    for offset, community in enumerate(range(LIST_SIZE - FOUNDATION_SEATS - 3, LIST_SIZE - FOUNDATION_SEATS)):
        foundation_community = LIST_SIZE - FOUNDATION_SEATS + offset
        values.append(
            edge(
                account_id(community, 0),
                account_id(foundation_community, 0),
                "vouch",
                f"vouch:foundation-link:{community:02d}:{foundation_community:02d}",
            )
        )
    return canonical_edges(values)


def accountability_score(node: str, ordinal: int) -> int:
    """Create explicit evidence and invoke the production score evaluator."""

    node_seed = int.from_bytes(
        hashlib.sha256(
            f"{ACCOUNTABILITY_SEED}:{ordinal}:{node}".encode("ascii")
        ).digest()[:8],
        "big",
    )
    rng = random.Random(node_seed)
    tenure_days = rng.randint(210, 820)
    recent_tasks = rng.randint(16, 48)
    failure_rate_percent = rng.randint(0, 24)
    tasks: list[accountability.TaskEvidence] = []
    anchor = EVALUATION_END - timedelta(days=tenure_days)
    tasks.append(
        accountability.TaskEvidence(
            task_id=f"{node}-tenure-anchor",
            kind="network",
            accepted_at=anchor - timedelta(hours=2),
            verification_outcome="pass",
            verified_at=anchor - timedelta(hours=1),
            rewarded_at=anchor,
        )
    )
    for index in range(recent_tasks):
        accepted = EVALUATION_END - timedelta(
            days=1 + (index * 173 // max(1, recent_tasks)),
            hours=index % 11,
        )
        failed = ((index * 37 + ordinal) % 100) < failure_rate_percent
        tasks.append(
            accountability.TaskEvidence(
                task_id=f"{node}-task-{index:03d}",
                kind="network",
                accepted_at=accepted,
                verification_outcome="fail" if failed else "pass",
                verified_at=accepted + timedelta(minutes=20),
                rewarded_at=accepted + timedelta(minutes=40),
            )
        )
    open_disputes = 1 if rng.randrange(10) == 0 else 0
    disputes = tuple(
        accountability.DisputeEvidence(
            dispute_id=f"{node}-dispute-{index}",
            opened_at=EVALUATION_END - timedelta(days=30 + index),
            resolved_at=None,
        )
        for index in range(open_disputes)
    )
    badge_verified = rng.randrange(20) != 0
    badge = accountability.BadgeEvidence(
        verified=badge_verified,
        valid_from=EVALUATION_END - timedelta(days=300),
        expires_at=EVALUATION_END + timedelta(days=65),
        revoked_at=None,
    )
    result = accountability.evaluate_accountability(
        accountability.AccountabilityEvidence(
            window_end=EVALUATION_END,
            tasks=tuple(tasks),
            disputes=disputes,
            badge=badge,
        )
    )
    if result.calculation is None:
        raise RuntimeError(f"synthetic accountability held for {node}")
    return result.calculation.projected_score


def build_population() -> Population:
    nodes = tuple(
        account_id(community, member)
        for community in range(COMMUNITIES)
        for member in range(ACCOUNTS_PER_COMMUNITY)
    )
    ratified = tuple(account_id(community, 0) for community in range(LIST_SIZE))
    foundation = ratified[-FOUNDATION_SEATS:]
    seats = tuple(
        trust_graph.SeatAssignment(
            validator_id=f"validator-{index:02d}", node=node
        )
        for index, node in enumerate(ratified)
    )
    scores = {
        node: accountability_score(node, index)
        for index, node in enumerate(nodes)
    }
    return Population(
        nodes=nodes,
        ratified=ratified,
        foundation=foundation,
        edges=build_honest_edges(),
        seats=seats,
        accountability_scores=scores,
    )


def derive_graph(
    population: Population,
    nodes: Sequence[str],
    edges: Sequence[trust_graph.TrustEdge],
    seats: Sequence[trust_graph.SeatAssignment],
) -> trust_graph.TrustGraphResult:
    return trust_graph.derive_trust_graph(
        trust_graph.TrustGraphEvidence(
            nodes=tuple(nodes),
            ratified_nodes=population.ratified,
            foundation_bound_nodes=population.foundation,
            edges=tuple(edges),
            baseline_list_size=LIST_SIZE,
            seats=tuple(seats),
        )
    )


def increment_cluster_seat(
    graph: trust_graph.TrustGraphResult, candidate: str
) -> trust_graph.TrustGraphResult:
    """Advance fixed-window seat state after policy admits one candidate."""

    updated = []
    for cluster in graph.clusters:
        if candidate not in cluster.members:
            updated.append(cluster)
            continue
        seat_count = cluster.seat_count + 1
        updated.append(
            replace(
                cluster,
                seat_count=seat_count,
                over_seat_cap=Fraction(seat_count, 1) > graph.cluster_seat_cap,
            )
        )
    return replace(graph, clusters=tuple(updated))


def admission_decision(
    population: Population,
    candidate: str,
    accountability_score: int,
    graph_projection: Mapping[str, object],
    *,
    funding_correlated: bool,
) -> Mapping[str, object]:
    """Run Admission Policy V1 with all non-experiment gates passing."""

    validator_id = f"candidate-validator-{candidate}"
    control_group = policy.ControlGroup(
        validator_id=validator_id,
        operator_group=f"operator-{candidate}",
        release_manager_group=f"release-{candidate}",
        key_management_group=f"keys-{candidate}",
        funding_source_group=f"funding-{candidate}",
    )
    facts = policy.CandidateFacts(
        validator_id=validator_id,
        account_id=candidate,
        public_key_hash=hashlib.sha256(candidate.encode("ascii")).hexdigest(),
        reliability_bps=policy.V1_MIN_RELIABILITY_BPS,
        operator_manifest_signed=True,
        domain_control_proved=True,
        cobalt_linkedness_safe=True,
        control_group=control_group,
        model_output=policy.ModelOutput(
            classification="independent",
            cited_fields=(policy.FIELD_OPERATOR_GROUP,),
            parsed_output_root="c" * 64,
            replay_certificate_root="d" * 64,
        ),
    )
    active = tuple(
        policy.ActiveValidator(
            validator_id=seat.validator_id,
            account_id=seat.node,
            control_group=policy.ControlGroup(
                validator_id=seat.validator_id,
                operator_group=f"active-operator-{index:02d}",
                release_manager_group=f"active-release-{index:02d}",
                key_management_group=f"active-keys-{index:02d}",
                funding_source_group=f"active-funding-{index:02d}",
            ),
        )
        for index, seat in enumerate(population.seats)
    )
    evidence = policy.PolicyEvidence(
        evaluation_end=schema.format_utc_timestamp(EVALUATION_END),
        target_round=2,
        transition_budget=1,
        foundation_bound_validator_ids=tuple(
            seat.validator_id
            for seat in population.seats
            if seat.node in population.foundation
        ),
        active_validators=active,
        candidates=(facts,),
    )
    roots = {
        "work_digest_verifications": "1" * 64,
        "trust_graph": "2" * 64,
        "policy_evidence": "3" * 64,
        "public_edges": "4" * 64,
        "independence": "5" * 64,
    }
    packet = policy._policy_packet(
        facts,
        evidence,
        "a" * 64,
        accountability_score,
        1 if funding_correlated else 0,
        roots,
    )
    upstream = tuple(
        {
            "code": str(code),
            "field": "validator.trust_graph",
        }
        for code in graph_projection["reason_codes"]
    )
    return policy._evaluate_v1_projection(packet, upstream)


def allocate_seats(
    population: Population,
    *,
    nodes: Sequence[str],
    edges: Sequence[trust_graph.TrustEdge],
    candidates: Sequence[str],
    scores: Mapping[str, int],
    blocked_candidates: Iterable[str] = (),
) -> Allocation:
    """Apply production Admission Policy V1 in canonical one-seat rounds."""

    graph = derive_graph(population, nodes, edges, population.seats)
    blocked = frozenset(blocked_candidates)
    selected: list[str] = []
    projections: dict[str, Mapping[str, object]] = {}
    for candidate in sorted(candidates):
        projection = policy._graph_projection(candidate, graph)
        projections[candidate] = projection
        decision = admission_decision(
            population,
            candidate,
            scores[candidate],
            projection,
            funding_correlated=candidate in blocked,
        )
        if decision["action"] == "admit":
            selected.append(candidate)
            graph = increment_cluster_seat(graph, candidate)
    final_seats = population.seats + tuple(
        trust_graph.SeatAssignment(
            validator_id=f"simulated-add-{index:04d}-{candidate}",
            node=candidate,
        )
        for index, candidate in enumerate(selected)
    )
    final_graph = derive_graph(population, nodes, edges, final_seats)
    return Allocation(tuple(selected), final_graph, projections)


def funding_blocked_candidates(
    edges: Sequence[trust_graph.TrustEdge],
    candidates: Sequence[str],
    population: Population,
) -> tuple[str, ...]:
    extracted = edge_engine.EdgeExtractionResult(
        status="extracted",
        hold_reasons=(),
        edges=tuple(edges),
        provenance=(),
    )
    active = frozenset(population.ratified)
    return tuple(
        candidate
        for candidate in sorted(candidates)
        if policy._funding_links(extracted, candidate, active)
    )


def fraction_ppm(value: Fraction) -> int:
    return value.numerator * 1_000_000 // value.denominator


def attack_metrics(
    population: Population,
    allocation: Allocation,
    attack_nodes: Sequence[str],
    candidates: Sequence[str],
    budget: int,
    budget_detail: Mapping[str, int],
    extra: Mapping[str, object] | None = None,
) -> dict[str, object]:
    mass = dict(allocation.graph.stationary_mass)
    attack_set = frozenset(attack_nodes)
    controlled_mass = sum(
        (mass.get(node, Fraction(0, 1)) for node in attack_set),
        Fraction(0, 1),
    )
    cluster_masses = [
        sum((mass[node] for node in cluster.members), Fraction(0, 1))
        for cluster in allocation.graph.clusters
        if attack_set.intersection(cluster.members)
    ]
    selected = frozenset(allocation.selected)
    floor = allocation.graph.connectivity_mass_floor
    metrics: dict[str, object] = {
        "budget": budget,
        "budget_detail": dict(sorted(budget_detail.items())),
        "candidate_accounts": len(candidates),
        "seats_gained": len(allocation.selected),
        "selected_accounts": list(allocation.selected),
        "attack_controlled_walk_mass_ppm": fraction_ppm(controlled_mass),
        "largest_touched_cluster_mass_ppm": (
            fraction_ppm(max(cluster_masses)) if cluster_masses else 0
        ),
        "connected_attack_candidates": sum(
            mass.get(candidate, Fraction(0, 1)) >= floor
            for candidate in candidates
        ),
        "cluster_cap_held": not any(
            cluster.over_seat_cap for cluster in allocation.graph.clusters
        ),
        "connectivity_floor_held": all(
            mass.get(candidate, Fraction(0, 1)) >= floor
            for candidate in selected
        ),
    }
    if extra:
        metrics.update(extra)
    return metrics


def eligible_honest_candidates(
    population: Population,
    graph: trust_graph.TrustGraphResult,
) -> tuple[str, ...]:
    candidates = [
        node
        for node in population.nodes
        if node not in population.ratified
        and population.accountability_scores[node] >= policy.ACCOUNTABILITY_FLOOR
    ]
    blocked = frozenset(
        funding_blocked_candidates(population.edges, candidates, population)
    )
    return tuple(
        node
        for node in sorted(candidates)
        if node not in blocked
        and policy._graph_projection(node, graph)["status"] == "pass"
    )


def ranked_purchase_pool(
    population: Population,
    base_graph: trust_graph.TrustGraphResult,
) -> tuple[str, ...]:
    mass = dict(base_graph.stationary_mass)
    candidates = eligible_honest_candidates(population, base_graph)
    cluster_by_node = {
        node: cluster.cluster_id
        for cluster in base_graph.clusters
        for node in cluster.members
    }
    ranked = sorted(candidates, key=lambda node: (-mass[node], node))
    first_by_cluster: list[str] = []
    remainder: list[str] = []
    seen: set[str] = set()
    for node in ranked:
        cluster = cluster_by_node[node]
        if cluster not in seen:
            seen.add(cluster)
            first_by_cluster.append(node)
        else:
            remainder.append(node)
    return tuple(first_by_cluster + remainder)


def sponsor_pool(
    population: Population,
    base_graph: trust_graph.TrustGraphResult,
) -> tuple[str, ...]:
    mass = dict(base_graph.stationary_mass)
    cluster_by_node = {
        node: cluster.cluster_id
        for cluster in base_graph.clusters
        for node in cluster.members
    }
    seeds = [node for node in population.ratified if node not in population.foundation]
    ranked = sorted(seeds, key=lambda node: (-mass[node], node))
    selected: list[str] = []
    seen: set[str] = set()
    for node in ranked:
        cluster = cluster_by_node[node]
        if cluster not in seen:
            selected.append(node)
            seen.add(cluster)
    if len(selected) < 2:
        raise RuntimeError("synthetic graph did not yield two sponsor clusters")
    return tuple(selected)


def run_vouch_ring(
    population: Population, budget: int
) -> dict[str, object]:
    candidates = tuple(f"attack-ring-{index:03d}" for index in range(budget))
    values = list(population.edges)
    if budget > 1:
        for index, source in enumerate(candidates):
            target = candidates[(index + 1) % budget]
            values.append(edge(source, target, "vouch", f"attack:ring:{index:03d}"))
    nodes = population.nodes + candidates
    scores = {candidate: 100 for candidate in candidates}
    allocation = allocate_seats(
        population,
        nodes=nodes,
        edges=canonical_edges(values),
        candidates=candidates,
        scores=scores,
    )
    return attack_metrics(
        population,
        allocation,
        candidates,
        candidates,
        budget,
        {"ring_accounts": budget, "internal_vouches": budget if budget > 1 else 0},
    )


def run_buying_aged_accounts(
    population: Population,
    base_graph: trust_graph.TrustGraphResult,
    budget: int,
) -> dict[str, object]:
    purchased = ranked_purchase_pool(population, base_graph)[:budget]
    controller = "attack-aged-controller"
    values = list(population.edges)
    for index, candidate in enumerate(purchased):
        values.append(
            edge(
                controller,
                candidate,
                "funding",
                f"attack:aged:majority-settlement:{index:03d}",
            )
        )
    nodes = population.nodes + ((controller,) if purchased else ())
    allocation = allocate_seats(
        population,
        nodes=nodes,
        edges=canonical_edges(values),
        candidates=purchased,
        scores=population.accountability_scores,
        blocked_candidates=funding_blocked_candidates(values, purchased, population),
    )
    attack_nodes = ((controller,) + purchased) if purchased else ()
    return attack_metrics(
        population,
        allocation,
        attack_nodes,
        purchased,
        budget,
        {"aged_accounts_bought": budget, "accounts_available": len(purchased)},
    )


def run_parallel_farming(
    population: Population,
    base_graph: trust_graph.TrustGraphResult,
    budget: int,
) -> dict[str, object]:
    rng = random.Random(ATTACK_SEED + budget)
    candidates = tuple(f"attack-farm-{index:03d}" for index in range(budget))
    controller = "attack-farm-controller"
    sponsors = list(sponsor_pool(population, base_graph))
    rng.shuffle(sponsors)
    values = list(population.edges)
    for index, candidate in enumerate(candidates):
        first = sponsors[index % len(sponsors)]
        second = sponsors[(index + 1) % len(sponsors)]
        values.extend(
            (
                edge(first, candidate, "vouch", f"attack:farm:vouch-a:{index:03d}"),
                edge(second, candidate, "vouch", f"attack:farm:vouch-b:{index:03d}"),
                edge(
                    controller,
                    candidate,
                    "funding",
                    f"attack:farm:funding:{index:03d}",
                ),
            )
        )
    nodes = population.nodes + candidates + ((controller,) if candidates else ())
    scores = {candidate: 100 for candidate in candidates}
    allocation = allocate_seats(
        population,
        nodes=nodes,
        edges=canonical_edges(values),
        candidates=candidates,
        scores=scores,
        blocked_candidates=funding_blocked_candidates(values, candidates, population),
    )
    attack_nodes = ((controller,) + candidates) if candidates else ()
    return attack_metrics(
        population,
        allocation,
        attack_nodes,
        candidates,
        budget,
        {
            "fully_farmed_accounts": budget,
            "external_vouches": budget * 2,
            "distinct_sponsor_clusters_per_account": 2 if budget else 0,
        },
    )


def funding_exclusion_document() -> dict[str, object]:
    return {
        "schema": schema.FUNDING_EXCLUSION_SCHEMA,
        "mode": schema.SHADOW_MODE,
        "version": "synthetic-20260907",
        "publication_tx_hash": "f" * 64,
        "published_at": "2026-03-01T00:00:00Z",
        "valid_from": "2026-03-01T00:00:00Z",
        "valid_until": "2027-03-01T00:00:00Z",
        "addresses": [],
    }


def extract_dust_funding_edges(
    attacker: str, rival: str, dust_transfers: int
) -> edge_engine.ExtractedEdges:
    attacker_wallet = "rSyntheticMaliciousIncumbent"
    rival_wallet = "rSyntheticAlreadyFundedRival"
    transfers: list[dict[str, object]] = [
        {
            "tx_hash": hashlib.sha256(b"historical-first-funding").hexdigest(),
            "ledger_index": 100,
            "transaction_index": 0,
            "close_time": "2026-01-01T00:00:00Z",
            "source_wallet_address": "rSyntheticExternalOriginalFunder",
            "target_wallet_address": rival_wallet,
            "asset": "PFT",
            "value_units": 100,
        }
    ]
    for index in range(dust_transfers):
        transfers.append(
            {
                "tx_hash": hashlib.sha256(
                    f"dust-transfer:{index}".encode("ascii")
                ).hexdigest(),
                "ledger_index": 1_000 + index,
                "transaction_index": 0,
                "close_time": "2026-08-01T00:00:00Z",
                "source_wallet_address": attacker_wallet,
                "target_wallet_address": rival_wallet,
                "asset": "PFT",
                "value_units": 1,
            }
        )
    document = {
        "schema": schema.FUNDING_TRANSFER_INPUT_SCHEMA,
        "mode": schema.SHADOW_MODE,
        "window": WINDOW,
        "value_asset": "PFT",
        "history_complete_from_ledger_genesis": True,
        "window_complete": True,
        "wallet_accounts": [
            {"wallet_address": attacker_wallet, "account_id": attacker},
            {"wallet_address": rival_wallet, "account_id": rival},
        ],
        "transfers": transfers,
    }
    return edge_engine.extract_funding_edges(
        document, funding_exclusion_document()
    )


def run_first_funder_manipulation(
    population: Population,
    base_graph: trust_graph.TrustGraphResult,
    budget: int,
) -> dict[str, object]:
    honest_candidates = eligible_honest_candidates(population, base_graph)
    rival = (
        honest_candidates[0]
        if honest_candidates
        else account_id(0, 1)
    )
    would_pass = (
        population.accountability_scores[rival] >= policy.ACCOUNTABILITY_FLOOR
        and policy._graph_projection(rival, base_graph)["status"] == "pass"
    )
    attacker = population.ratified[0]
    extracted = extract_dust_funding_edges(attacker, rival, budget)
    values = canonical_edges(population.edges + extracted.edges)
    blocked = funding_blocked_candidates(values, (rival,), population)
    target_decision = admission_decision(
        population,
        rival,
        population.accountability_scores[rival],
        policy._graph_projection(rival, base_graph),
        funding_correlated=rival in blocked,
    )
    allocation = allocate_seats(
        population,
        nodes=population.nodes,
        edges=values,
        candidates=(),
        scores={},
    )
    return attack_metrics(
        population,
        allocation,
        (attacker,),
        (),
        budget,
        {"dust_transfers": budget, "value_units_each": 1},
        {
            "target_account": rival,
            "funding_edge_created": bool(extracted.edges),
            "target_denied_by_funding_rho": (
                would_pass and target_decision["action"] == "reject"
            ),
            "target_would_pass_graph_without_attack": would_pass,
        },
    )


def foundation_support_actions(
    population: Population, candidate: str
) -> tuple[trust_graph.TrustEdge, ...]:
    actions: list[trust_graph.TrustEdge] = []
    for index, foundation in enumerate(population.foundation):
        actions.append(
            edge(
                foundation,
                candidate,
                "vouch",
                f"attack:foundation:vouch:{index:02d}",
            )
        )
    for unit in range(3):
        for index, foundation in enumerate(population.foundation):
            actions.append(
                edge(
                    foundation,
                    candidate,
                    "cowork",
                    f"attack:foundation:cowork:{unit:02d}:{index:02d}",
                )
            )
    return tuple(actions)


def run_foundation_choice(
    population: Population, budget: int
) -> dict[str, object]:
    candidate = "attack-foundation-favorite"
    actions = foundation_support_actions(population, candidate)[:budget]
    values = canonical_edges(population.edges + actions)
    nodes = population.nodes + (candidate,)
    scores = {candidate: 100}
    allocation = allocate_seats(
        population,
        nodes=nodes,
        edges=values,
        candidates=(candidate,),
        scores=scores,
    )
    return attack_metrics(
        population,
        allocation,
        population.foundation + (candidate,),
        (candidate,),
        budget,
        {
            "foundation_support_actions": budget,
            "foundation_vouches": min(budget, 3),
            "shared_work_units": max(0, budget - 3),
        },
    )


ATTACK_BUDGETS: tuple[tuple[str, tuple[int, ...]], ...] = (
    ("vouch_ring", (0, 1, 2, 4, 8, 16, 32, 64)),
    ("buying_aged_accounts", (0, 1, 2, 3, 4, 6, 8, 12, 16)),
    ("farming_parallel_identities", (0, 1, 2, 3, 4, 6, 8, 12, 16)),
    ("first_funder_manipulation", (0, 1, 2, 4, 8, 16, 32, 64)),
    ("foundation_choosing_winner", (0, 1, 2, 3, 4, 6, 8, 10, 12)),
)


def run_attack_sweeps(
    population: Population,
    base_graph: trust_graph.TrustGraphResult,
) -> tuple[dict[str, object], ...]:
    rows: list[dict[str, object]] = []
    for attack, budgets in ATTACK_BUDGETS:
        points = []
        for budget in budgets:
            if attack == "vouch_ring":
                result = run_vouch_ring(population, budget)
            elif attack == "buying_aged_accounts":
                result = run_buying_aged_accounts(population, base_graph, budget)
            elif attack == "farming_parallel_identities":
                result = run_parallel_farming(population, base_graph, budget)
            elif attack == "first_funder_manipulation":
                result = run_first_funder_manipulation(
                    population, base_graph, budget
                )
            elif attack == "foundation_choosing_winner":
                result = run_foundation_choice(population, budget)
            else:
                raise AssertionError(attack)
            points.append(result)
        successful = [
            int(point["budget"])
            for point in points
            if int(point["seats_gained"]) > 0
        ]
        best_seats = 0
        for point in points:
            best_seats = max(best_seats, int(point["seats_gained"]))
            point["best_seats_at_or_below_budget"] = best_seats
        cap_violations = [
            int(point["budget"])
            for point in points
            if not bool(point["cluster_cap_held"])
        ]
        target_denials = [
            int(point["budget"])
            for point in points
            if bool(point.get("target_denied_by_funding_rho", False))
        ]
        rows.append(
            {
                "attack": attack,
                "cheapest_seat_gaining_budget": min(successful) if successful else None,
                "cheapest_cluster_cap_violation_budget": (
                    min(cap_violations) if cap_violations else None
                ),
                "cheapest_target_denial_budget": (
                    min(target_denials) if target_denials else None
                ),
                "points": points,
            }
        )
    return tuple(rows)


def honest_admissions(
    population: Population,
) -> tuple[int, trust_graph.TrustGraphResult]:
    candidates = tuple(
        node for node in population.nodes if node not in population.ratified
    )
    blocked = funding_blocked_candidates(population.edges, candidates, population)
    allocation = allocate_seats(
        population,
        nodes=population.nodes,
        edges=population.edges,
        candidates=candidates,
        scores=population.accountability_scores,
        blocked_candidates=blocked,
    )
    baseline_graph = derive_graph(
        population, population.nodes, population.edges, population.seats
    )
    return len(allocation.selected), baseline_graph


def sensitivity_profiles() -> tuple[tuple[str, str, EngineProfile], ...]:
    variants: list[tuple[str, str, EngineProfile]] = []

    def add(
        constant: str,
        values: Sequence[tuple[str, object]],
        field: str,
    ) -> None:
        for label, value in values:
            variants.append((constant, label, replace(DEFAULT_PROFILE, **{field: value})))

    add("vouch_weight", (("1/2", Fraction(1, 2)), ("1", Fraction(1)), ("2", Fraction(2))), "vouch_weight")
    add("cowork_weight", (("1/2", Fraction(1, 2)), ("1", Fraction(1)), ("2", Fraction(2))), "cowork_weight")
    add("cowork_cap", (("1", 1), ("3", 3), ("5", 5)), "cowork_cap")
    add("funding_weight", (("1", Fraction(1)), ("2", Fraction(2)), ("4", Fraction(4))), "funding_weight")
    add("walk_damping", (("0.75", Fraction(3, 4)), ("0.85", Fraction(17, 20)), ("0.90", Fraction(9, 10))), "damping")
    add("walk_steps", (("10", 10), ("20", 20), ("40", 40)), "walk_steps")
    add("conductance_cut", (("0.05", Fraction(1, 20)), ("0.10", Fraction(1, 10)), ("0.15", Fraction(3, 20))), "conductance_cut")
    add("connectivity_floor", (("1/N", 1), ("1/(2N)", 2), ("1/(4N)", 4)), "connectivity_divisor")
    add("minimum_cluster_seats", (("1", 1), ("2", 2), ("3", 3)), "minimum_cluster_seats")
    add("cluster_seat_fraction", (("5%", Fraction(1, 20)), ("10%", Fraction(1, 10)), ("15%", Fraction(3, 20))), "cluster_seat_fraction")
    return tuple(variants)


def profile_summary(
    population: Population,
    profile: EngineProfile,
) -> dict[str, object]:
    with engine_profile(profile):
        admission_count, graph = honest_admissions(population)
        sweeps = run_attack_sweeps(population, graph)
    return {
        "honest_account_admissions": admission_count,
        "cheapest_seat_gaining_budget": {
            str(row["attack"]): row["cheapest_seat_gaining_budget"]
            for row in sweeps
        },
        "sweeps": sweeps,
    }


def source_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def edge_counts(edges: Sequence[trust_graph.TrustEdge]) -> dict[str, int]:
    return {
        kind: sum(item.kind == kind for item in edges)
        for kind in schema.EDGE_KINDS
    }


def results_document() -> dict[str, object]:
    population = build_population()
    cache: dict[tuple[object, ...], dict[str, object]] = {}

    def cached(profile: EngineProfile) -> dict[str, object]:
        if profile.key() not in cache:
            cache[profile.key()] = profile_summary(population, profile)
        return cache[profile.key()]

    default = cached(DEFAULT_PROFILE)
    sensitivity = []
    for constant, value, profile in sensitivity_profiles():
        summary = cached(profile)
        sensitivity.append(
            {
                "constant": constant,
                "value": value,
                "honest_account_admissions": summary["honest_account_admissions"],
                "cheapest_seat_gaining_budget": summary[
                    "cheapest_seat_gaining_budget"
                ],
            }
        )
    scores = list(population.accountability_scores.values())
    engine_paths = (
        "python/postfiat_rpc/tasknode_unl_trust_graph.py",
        "python/postfiat_rpc/tasknode_unl_edges.py",
        "python/postfiat_rpc/tasknode_unl_accountability.py",
        "python/postfiat_rpc/tasknode_unl_policy.py",
    )
    proposal = Path(
        "/home/postfiatchad/repos/postfiatorg.github.io/content/research/"
        "deterministic-unl-task-node-cobalt.md"
    )
    return {
        "schema": "tasknode-unl-attack-simulation-results-v1",
        "evaluation_end": schema.format_utc_timestamp(EVALUATION_END),
        "simulation_mode": "synthetic_local_only",
        "random_seeds": {
            "population": POPULATION_SEED,
            "accountability": ACCOUNTABILITY_SEED,
            "attacks": ATTACK_SEED,
        },
        "source_hashes": {
            **{
                path: source_hash(REPOSITORY_ROOT / path)
                for path in engine_paths
            },
            "published_proposal": source_hash(proposal),
        },
        "generation_assumptions": {
            "honest_accounts": HONEST_ACCOUNT_COUNT,
            "communities": COMMUNITIES,
            "accounts_per_community": ACCOUNTS_PER_COMMUNITY,
            "ratified_list_size": LIST_SIZE,
            "foundation_bound_ratified_accounts": FOUNDATION_SEATS,
            "uniform_non_foundation_seed_accounts": LIST_SIZE - FOUNDATION_SEATS,
            "edge_counts": edge_counts(population.edges),
            "mean_edge_facts_per_account_milli": (
                len(population.edges) * 1_000 // HONEST_ACCOUNT_COUNT
            ),
            "accountability_score_min": min(scores),
            "accountability_score_median": sorted(scores)[len(scores) // 2],
            "accountability_score_max": max(scores),
            "accounts_meeting_accountability_floor": sum(
                score >= policy.ACCOUNTABILITY_FLOOR for score in scores
            ),
            "fixed_window_exposure": (
                "Each selected seat consumes one conceptual one-add round; "
                "N and the start-of-window seed set stay fixed to isolate graph effects."
            ),
        },
        "published_profile": {
            "vouch_weight": "1",
            "cowork_weight": "1",
            "cowork_cap": 3,
            "funding_weight": "2",
            "walk_damping": "0.85",
            "walk_steps": 20,
            "conductance_cut": "0.10",
            "connectivity_floor": "1/(2N)",
            "cluster_seat_cap": "max(2,10% of N)",
        },
        "published_profile_honest_account_admissions": default[
            "honest_account_admissions"
        ],
        "attack_sweeps": default["sweeps"],
        "sensitivity": sensitivity,
    }


def attack_csv(document: Mapping[str, object]) -> bytes:
    output = io.StringIO(newline="")
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(
        (
            "attack",
            "budget",
            "seats_gained_at_exact_budget",
            "best_seats_at_or_below_budget",
            "attack_controlled_walk_mass_ppm",
            "largest_touched_cluster_mass_ppm",
            "connected_attack_candidates",
            "cluster_cap_held",
            "connectivity_floor_held",
            "funding_edge_created",
            "target_denied_by_funding_rho",
        )
    )
    for sweep in document["attack_sweeps"]:
        for point in sweep["points"]:
            writer.writerow(
                (
                    sweep["attack"],
                    point["budget"],
                    point["seats_gained"],
                    point["best_seats_at_or_below_budget"],
                    point["attack_controlled_walk_mass_ppm"],
                    point["largest_touched_cluster_mass_ppm"],
                    point["connected_attack_candidates"],
                    str(point["cluster_cap_held"]).lower(),
                    str(point["connectivity_floor_held"]).lower(),
                    str(point.get("funding_edge_created", "")).lower(),
                    str(point.get("target_denied_by_funding_rho", "")).lower(),
                )
            )
    return output.getvalue().encode("utf-8")


def sensitivity_csv(document: Mapping[str, object]) -> bytes:
    attacks = tuple(attack for attack, _budgets in ATTACK_BUDGETS)
    output = io.StringIO(newline="")
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(
        ("constant", "value", "honest_account_admissions", *attacks)
    )
    for row in document["sensitivity"]:
        budgets = row["cheapest_seat_gaining_budget"]
        writer.writerow(
            (
                row["constant"],
                row["value"],
                row["honest_account_admissions"],
                *(budgets[attack] if budgets[attack] is not None else "none" for attack in attacks),
            )
        )
    return output.getvalue().encode("utf-8")


def write_outputs(output_directory: Path) -> None:
    output_directory.mkdir(parents=True, exist_ok=True)
    document = results_document()
    files = {
        "results.json": schema.canonical_json_bytes(document),
        "attack-sweep.csv": attack_csv(document),
        "sensitivity.csv": sensitivity_csv(document),
    }
    for name, content in files.items():
        (output_directory / name).write_bytes(content)
    manifest = {
        "schema": "tasknode-unl-attack-simulation-output-manifest-v1",
        "files": {
            name: {
                "bytes": len(content),
                "sha256": hashlib.sha256(content).hexdigest(),
            }
            for name, content in sorted(files.items())
        },
    }
    (output_directory / "output-manifest.json").write_bytes(
        schema.canonical_json_bytes(manifest)
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path(__file__).resolve().parent,
        help="directory for canonical JSON/CSV outputs",
    )
    args = parser.parse_args()
    write_outputs(args.output_dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
