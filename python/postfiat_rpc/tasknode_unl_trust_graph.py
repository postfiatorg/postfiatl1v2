"""Deterministic EigenTrust-style walk and cluster controls.

The implementation is pure: callers provide nodes, public edge evidence, the
ratified seed list, prior-window vouch penalties, and seat assignments. It
performs no file, clock, database, or network access.

Cluster discovery uses a deterministic conductance sweep. Positive-weight
connected components are separated first. Within each component, nodes are
ordered by descending stationary mass and then identifier bytes. Every prefix
cut is evaluated with exact undirected weighted conductance; the lowest
strictly-sub-threshold cut wins, with canonical member tuples breaking ties.
The procedure recurses over both sides. Golden fixtures lock this policy.
"""

from __future__ import annotations

from dataclasses import dataclass
from fractions import Fraction
from typing import Any, Iterable, Mapping, Sequence

from .tasknode_unl_schema import (
    CONDUCTANCE_CUT_THRESHOLD,
    COWORK_EDGE_CAP,
    COWORK_EDGE_WEIGHT,
    EDGE_KINDS,
    FUNDING_EDGE_WEIGHT,
    SHADOW_MODE,
    TRUST_GRAPH_INPUT_SCHEMA,
    TRUST_GRAPH_RESULT_SCHEMA,
    TRUST_WALK_DAMPING,
    TRUST_WALK_ITERATIONS,
    TRUST_WALK_SEED_DAMPING,
    VOUCH_EDGE_WEIGHT,
    TaskNodeUnlError,
    canonical_json_bytes,
    cluster_seat_limit,
    connectivity_floor,
    require_closed_keys,
    require_identifier,
    require_int,
)


@dataclass(frozen=True)
class TrustEdge:
    source: str
    target: str
    kind: str
    evidence_id: str


@dataclass(frozen=True)
class SeatAssignment:
    validator_id: str
    node: str


@dataclass(frozen=True)
class TrustGraphEvidence:
    nodes: tuple[str, ...]
    ratified_nodes: tuple[str, ...]
    foundation_bound_nodes: tuple[str, ...]
    edges: tuple[TrustEdge, ...]
    baseline_list_size: int
    seats: tuple[SeatAssignment, ...]
    penalized_vouchers: tuple[str, ...] = ()


@dataclass(frozen=True)
class ConductanceCut:
    left: tuple[str, ...]
    right: tuple[str, ...]
    conductance: Fraction

    def to_dict(self) -> dict[str, Any]:
        return {
            "left": list(self.left),
            "right": list(self.right),
            "conductance": self.conductance,
        }


@dataclass(frozen=True)
class ClusterResult:
    cluster_id: str
    members: tuple[str, ...]
    seat_count: int
    over_seat_cap: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "cluster_id": self.cluster_id,
            "members": list(self.members),
            "seat_count": self.seat_count,
            "over_seat_cap": self.over_seat_cap,
        }


@dataclass(frozen=True)
class TrustGraphResult:
    status: str
    hold_reasons: tuple[str, ...]
    seed_vector: tuple[tuple[str, Fraction], ...]
    raw_rows: tuple[tuple[str, tuple[tuple[str, Fraction], ...]], ...]
    transition_rows: tuple[
        tuple[str, tuple[tuple[str, Fraction], ...]], ...
    ]
    stationary_mass: tuple[tuple[str, Fraction], ...]
    connectivity_mass_floor: Fraction
    connectivity_holds: tuple[str, ...]
    cuts: tuple[ConductanceCut, ...]
    clusters: tuple[ClusterResult, ...]
    cluster_seat_cap: Fraction
    baseline_list_size: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": TRUST_GRAPH_RESULT_SCHEMA,
            "mode": SHADOW_MODE,
            "status": self.status,
            "hold_reasons": list(self.hold_reasons),
            "constants": {
                "vouch_weight": VOUCH_EDGE_WEIGHT,
                "cowork_weight_per_shared_unit": COWORK_EDGE_WEIGHT,
                "cowork_weight_cap": COWORK_EDGE_CAP,
                "funding_weight": FUNDING_EDGE_WEIGHT,
                "iterations": TRUST_WALK_ITERATIONS,
                "damping": TRUST_WALK_DAMPING,
                "seed_damping": TRUST_WALK_SEED_DAMPING,
                "conductance_cut_threshold": CONDUCTANCE_CUT_THRESHOLD,
                "connectivity_floor": self.connectivity_mass_floor,
                "cluster_seat_cap": self.cluster_seat_cap,
            },
            "baseline_list_size": self.baseline_list_size,
            "seed_vector": dict(self.seed_vector),
            "raw_rows": {
                node: dict(row) for node, row in self.raw_rows
            },
            "transition_rows": {
                node: dict(row) for node, row in self.transition_rows
            },
            "stationary_mass": dict(self.stationary_mass),
            "connectivity_holds": list(self.connectivity_holds),
            "cuts": [cut.to_dict() for cut in self.cuts],
            "clusters": [cluster.to_dict() for cluster in self.clusters],
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.to_dict())


def _duplicates(values: Sequence[str]) -> tuple[str, ...]:
    seen: set[str] = set()
    duplicates: set[str] = set()
    for value in values:
        if value in seen:
            duplicates.add(value)
        seen.add(value)
    return tuple(sorted(duplicates))


def _validated_nodes(values: Sequence[str], field: str) -> tuple[str, ...]:
    checked = tuple(require_identifier(value, field) for value in values)
    duplicates = _duplicates(checked)
    if duplicates:
        raise TaskNodeUnlError("duplicate_node", f"{field}.{duplicates[0]}")
    return tuple(sorted(checked))


def _validate_evidence(
    evidence: TrustGraphEvidence,
) -> tuple[
    tuple[str, ...],
    tuple[str, ...],
    frozenset[str],
    tuple[TrustEdge, ...],
    tuple[SeatAssignment, ...],
    frozenset[str],
]:
    nodes = _validated_nodes(evidence.nodes, "nodes")
    if not nodes:
        raise TaskNodeUnlError("empty_nodes")
    node_set = frozenset(nodes)

    ratified = _validated_nodes(evidence.ratified_nodes, "ratified_nodes")
    foundation = _validated_nodes(
        evidence.foundation_bound_nodes, "foundation_bound_nodes"
    )
    if not set(ratified).issubset(node_set):
        raise TaskNodeUnlError("unknown_ratified_node")
    if not set(foundation).issubset(ratified):
        raise TaskNodeUnlError("foundation_node_not_ratified")

    list_size = require_int(
        evidence.baseline_list_size, "baseline_list_size", minimum=1
    )
    if len(ratified) != list_size:
        raise TaskNodeUnlError("ratified_list_size_mismatch")

    penalized = frozenset(
        _validated_nodes(evidence.penalized_vouchers, "penalized_vouchers")
    )
    if not penalized.issubset(node_set):
        raise TaskNodeUnlError("unknown_penalized_voucher")

    deduplicated_edges: set[TrustEdge] = set()
    for edge in evidence.edges:
        source = require_identifier(edge.source, "edges.source")
        target = require_identifier(edge.target, "edges.target")
        kind = require_identifier(edge.kind, "edges.kind")
        evidence_id = require_identifier(
            edge.evidence_id, "edges.evidence_id"
        )
        normalized = TrustEdge(source, target, kind, evidence_id)
        if source not in node_set or target not in node_set:
            raise TaskNodeUnlError("edge_unknown_node", evidence_id)
        if source == target:
            raise TaskNodeUnlError("self_edge", evidence_id)
        if kind not in EDGE_KINDS:
            raise TaskNodeUnlError("unknown_edge_kind", kind)
        deduplicated_edges.add(normalized)
    edges = tuple(
        sorted(
            deduplicated_edges,
            key=lambda edge: (
                edge.kind,
                edge.source,
                edge.target,
                edge.evidence_id,
            ),
        )
    )

    seats: list[SeatAssignment] = []
    validator_ids: set[str] = set()
    for seat in evidence.seats:
        validator_id = require_identifier(
            seat.validator_id, "seats.validator_id"
        )
        node = require_identifier(seat.node, "seats.node")
        if validator_id in validator_ids:
            raise TaskNodeUnlError(
                "duplicate_validator_seat", validator_id
            )
        validator_ids.add(validator_id)
        if node not in node_set:
            raise TaskNodeUnlError("seat_unknown_node", validator_id)
        seats.append(SeatAssignment(validator_id, node))
    seats.sort(key=lambda seat: (seat.validator_id, seat.node))

    return (
        nodes,
        ratified,
        frozenset(foundation),
        edges,
        tuple(seats),
        penalized,
    )


def _add_weight(
    rows: dict[str, dict[str, Fraction]],
    source: str,
    target: str,
    weight: Fraction,
) -> None:
    rows[source][target] = rows[source].get(target, Fraction(0, 1)) + weight


def build_raw_rows(
    nodes: Sequence[str],
    edges: Sequence[TrustEdge],
    penalized_vouchers: Iterable[str] = (),
) -> dict[str, dict[str, Fraction]]:
    """Aggregate canonical edge weights before row normalization."""

    ordered_nodes = _validated_nodes(nodes, "nodes")
    node_set = frozenset(ordered_nodes)
    penalties = frozenset(penalized_vouchers)
    if not penalties.issubset(node_set):
        raise TaskNodeUnlError("unknown_penalized_voucher")

    rows = {node: {} for node in ordered_nodes}
    vouch_pairs: set[tuple[str, str]] = set()
    cowork_units: dict[tuple[str, str], set[str]] = {}
    funding_pairs: set[tuple[str, str]] = set()

    for edge in sorted(
        set(edges),
        key=lambda edge: (
            edge.kind,
            edge.source,
            edge.target,
            edge.evidence_id,
        ),
    ):
        if edge.source not in node_set or edge.target not in node_set:
            raise TaskNodeUnlError("edge_unknown_node", edge.evidence_id)
        if edge.source == edge.target:
            raise TaskNodeUnlError("self_edge", edge.evidence_id)
        if edge.kind == "vouch":
            vouch_pairs.add((edge.source, edge.target))
        elif edge.kind == "cowork":
            pair = tuple(sorted((edge.source, edge.target)))
            cowork_units.setdefault(pair, set()).add(edge.evidence_id)
        elif edge.kind == "funding":
            funding_pairs.add(tuple(sorted((edge.source, edge.target))))
        else:
            raise TaskNodeUnlError("unknown_edge_kind", edge.kind)

    for source, target in sorted(vouch_pairs):
        weight = VOUCH_EDGE_WEIGHT
        if source in penalties:
            weight /= 2
        _add_weight(rows, source, target, weight)

    for pair, evidence_ids in sorted(cowork_units.items()):
        source, target = pair
        units = min(len(evidence_ids), COWORK_EDGE_CAP)
        weight = COWORK_EDGE_WEIGHT * units
        _add_weight(rows, source, target, weight)
        _add_weight(rows, target, source, weight)

    for source, target in sorted(funding_pairs):
        _add_weight(rows, source, target, FUNDING_EDGE_WEIGHT)
        _add_weight(rows, target, source, FUNDING_EDGE_WEIGHT)

    return {
        source: {
            target: rows[source][target] for target in sorted(rows[source])
        }
        for source in ordered_nodes
    }


def normalize_rows(
    raw_rows: Mapping[str, Mapping[str, Fraction]],
    seed_vector: Mapping[str, Fraction],
) -> dict[str, dict[str, Fraction]]:
    """Normalize non-empty rows; redirect dangling rows to the seed vector."""

    normalized: dict[str, dict[str, Fraction]] = {}
    for source in sorted(raw_rows):
        row = raw_rows[source]
        total = sum(row.values(), Fraction(0, 1))
        if total < 0:
            raise TaskNodeUnlError("negative_row_weight", source)
        if total == 0:
            normalized[source] = {
                target: seed_vector[target]
                for target in sorted(seed_vector)
                if seed_vector[target] > 0
            }
        else:
            normalized[source] = {
                target: row[target] / total for target in sorted(row)
            }
        if sum(normalized[source].values(), Fraction(0, 1)) != 1:
            raise TaskNodeUnlError("row_normalization_failed", source)
    return normalized


def power_iteration(
    transition_rows: Mapping[str, Mapping[str, Fraction]],
    seed_vector: Mapping[str, Fraction],
) -> dict[str, Fraction]:
    """Run exactly the proposal's 20 personalized power-iteration steps."""

    nodes = tuple(sorted(seed_vector))
    if set(transition_rows) != set(nodes):
        raise TaskNodeUnlError("transition_node_mismatch")
    mass = {node: seed_vector[node] for node in nodes}
    if sum(mass.values(), Fraction(0, 1)) != 1:
        raise TaskNodeUnlError("seed_mass_not_one")

    for _step in range(TRUST_WALK_ITERATIONS):
        walked = {node: Fraction(0, 1) for node in nodes}
        for source in nodes:
            for target in sorted(transition_rows[source]):
                if target not in walked:
                    raise TaskNodeUnlError(
                        "transition_unknown_node", target
                    )
                walked[target] += (
                    mass[source] * transition_rows[source][target]
                )
        mass = {
            node: TRUST_WALK_DAMPING * walked[node]
            + TRUST_WALK_SEED_DAMPING * seed_vector[node]
            for node in nodes
        }
        if sum(mass.values(), Fraction(0, 1)) != 1:
            raise TaskNodeUnlError("stationary_mass_not_one")
    return mass


def _undirected_weights(
    raw_rows: Mapping[str, Mapping[str, Fraction]],
) -> dict[tuple[str, str], Fraction]:
    weights: dict[tuple[str, str], Fraction] = {}
    for source in sorted(raw_rows):
        for target in sorted(raw_rows[source]):
            pair = tuple(sorted((source, target)))
            weights[pair] = (
                weights.get(pair, Fraction(0, 1))
                + raw_rows[source][target]
            )
    return {
        pair: weight
        for pair, weight in sorted(weights.items())
        if weight > 0
    }


def weighted_conductance(
    members: Iterable[str],
    left: Iterable[str],
    weights: Mapping[tuple[str, str], Fraction],
) -> Fraction | None:
    """Compute exact undirected conductance for one partition."""

    member_set = frozenset(members)
    left_set = frozenset(left)
    right_set = member_set - left_set
    if not left_set or not right_set or not left_set.issubset(member_set):
        raise TaskNodeUnlError("invalid_conductance_partition")

    left_volume = Fraction(0, 1)
    right_volume = Fraction(0, 1)
    cut_weight = Fraction(0, 1)
    for (source, target), weight in sorted(weights.items()):
        if source not in member_set or target not in member_set:
            continue
        if source in left_set:
            left_volume += weight
        else:
            right_volume += weight
        if target in left_set:
            left_volume += weight
        else:
            right_volume += weight
        if (source in left_set) != (target in left_set):
            cut_weight += weight

    denominator = min(left_volume, right_volume)
    if denominator == 0:
        return None
    return cut_weight / denominator


def _connected_components(
    nodes: Sequence[str],
    weights: Mapping[tuple[str, str], Fraction],
) -> tuple[tuple[str, ...], ...]:
    neighbors = {node: set() for node in nodes}
    for (source, target), weight in sorted(weights.items()):
        if weight > 0:
            neighbors[source].add(target)
            neighbors[target].add(source)

    remaining = set(nodes)
    components: list[tuple[str, ...]] = []
    while remaining:
        start = min(remaining)
        stack = [start]
        component: set[str] = set()
        while stack:
            node = stack.pop()
            if node in component:
                continue
            component.add(node)
            stack.extend(
                sorted(neighbors[node] - component, reverse=True)
            )
        remaining -= component
        components.append(tuple(sorted(component)))
    return tuple(sorted(components))


def _canonical_partition(
    left: Iterable[str], right: Iterable[str]
) -> tuple[tuple[str, ...], tuple[str, ...]]:
    first = tuple(sorted(left))
    second = tuple(sorted(right))
    return (first, second) if first < second else (second, first)


def _find_sweep_cut(
    members: tuple[str, ...],
    stationary_mass: Mapping[str, Fraction],
    weights: Mapping[tuple[str, str], Fraction],
) -> ConductanceCut | None:
    ranked = tuple(
        sorted(members, key=lambda node: (-stationary_mass[node], node))
    )
    member_set = frozenset(members)
    candidates: list[
        tuple[Fraction, tuple[str, ...], tuple[str, ...]]
    ] = []
    for index in range(1, len(ranked)):
        prefix = frozenset(ranked[:index])
        conductance = weighted_conductance(
            member_set, prefix, weights
        )
        if (
            conductance is not None
            and conductance < CONDUCTANCE_CUT_THRESHOLD
        ):
            left, right = _canonical_partition(
                prefix, member_set - prefix
            )
            candidates.append((conductance, left, right))
    if not candidates:
        return None
    conductance, left, right = min(candidates)
    return ConductanceCut(left, right, conductance)


def derive_clusters(
    nodes: Sequence[str],
    raw_rows: Mapping[str, Mapping[str, Fraction]],
    stationary_mass: Mapping[str, Fraction],
) -> tuple[tuple[tuple[str, ...], ...], tuple[ConductanceCut, ...]]:
    """Derive deterministic disconnected and low-conductance clusters."""

    ordered_nodes = tuple(sorted(nodes))
    weights = _undirected_weights(raw_rows)
    clusters: list[tuple[str, ...]] = []
    cuts: list[ConductanceCut] = []

    def split(members: tuple[str, ...]) -> None:
        if len(members) < 2:
            clusters.append(members)
            return
        cut = _find_sweep_cut(members, stationary_mass, weights)
        if cut is None:
            clusters.append(tuple(sorted(members)))
            return
        cuts.append(cut)
        split(cut.left)
        split(cut.right)

    for component in _connected_components(ordered_nodes, weights):
        split(component)

    return (
        tuple(sorted(clusters)),
        tuple(
            sorted(
                cuts,
                key=lambda cut: (
                    cut.left,
                    cut.right,
                    cut.conductance,
                ),
            )
        ),
    )


def meets_connectivity(stationary_mass: Fraction, list_size: int) -> bool:
    """Return whether mass meets the inclusive proposal floor of 1/(2N)."""

    if not isinstance(stationary_mass, Fraction):
        raise TaskNodeUnlError("non_rational_stationary_mass")
    return stationary_mass >= connectivity_floor(list_size)


def _row_tuples(
    rows: Mapping[str, Mapping[str, Fraction]],
) -> tuple[tuple[str, tuple[tuple[str, Fraction], ...]], ...]:
    return tuple(
        (
            source,
            tuple(
                (target, rows[source][target])
                for target in sorted(rows[source])
            ),
        )
        for source in sorted(rows)
    )


def derive_trust_graph(evidence: TrustGraphEvidence) -> TrustGraphResult:
    """Run the exact walk, connectivity check, clustering, and seat cap."""

    (
        nodes,
        ratified,
        foundation,
        edges,
        seats,
        penalties,
    ) = _validate_evidence(evidence)
    list_size = evidence.baseline_list_size
    mass_floor = connectivity_floor(list_size)
    seat_cap = cluster_seat_limit(list_size)
    seed_nodes = tuple(
        node for node in ratified if node not in foundation
    )
    if not seed_nodes:
        return TrustGraphResult(
            status="hold",
            hold_reasons=("empty_seed_set",),
            seed_vector=(),
            raw_rows=(),
            transition_rows=(),
            stationary_mass=(),
            connectivity_mass_floor=mass_floor,
            connectivity_holds=tuple(nodes),
            cuts=(),
            clusters=(),
            cluster_seat_cap=seat_cap,
            baseline_list_size=list_size,
        )

    seed_mass = Fraction(1, len(seed_nodes))
    seed_vector = {
        node: seed_mass if node in seed_nodes else Fraction(0, 1)
        for node in nodes
    }
    raw_rows = build_raw_rows(nodes, edges, penalties)
    transition_rows = normalize_rows(raw_rows, seed_vector)
    stationary_mass = power_iteration(transition_rows, seed_vector)
    connectivity_holds = tuple(
        node
        for node in nodes
        if not meets_connectivity(stationary_mass[node], list_size)
    )
    cluster_members, cuts = derive_clusters(
        nodes, raw_rows, stationary_mass
    )
    seats_by_node: dict[str, int] = {node: 0 for node in nodes}
    for seat in seats:
        seats_by_node[seat.node] += 1

    clusters = tuple(
        ClusterResult(
            cluster_id=f"cluster-{index:04d}",
            members=members,
            seat_count=sum(seats_by_node[node] for node in members),
            over_seat_cap=(
                sum(seats_by_node[node] for node in members) > seat_cap
            ),
        )
        for index, members in enumerate(cluster_members)
    )
    return TrustGraphResult(
        status="scored",
        hold_reasons=(),
        seed_vector=tuple(
            (node, seed_vector[node]) for node in nodes
        ),
        raw_rows=_row_tuples(raw_rows),
        transition_rows=_row_tuples(transition_rows),
        stationary_mass=tuple(
            (node, stationary_mass[node]) for node in nodes
        ),
        connectivity_mass_floor=mass_floor,
        connectivity_holds=connectivity_holds,
        cuts=cuts,
        clusters=clusters,
        cluster_seat_cap=seat_cap,
        baseline_list_size=list_size,
    )


def _string_array(value: object, field: str) -> tuple[str, ...]:
    if not isinstance(value, list):
        raise TaskNodeUnlError("invalid_array", field)
    return tuple(
        require_identifier(item, f"{field}[{index}]")
        for index, item in enumerate(value)
    )


def _edge_from_dict(value: object, index: int) -> TrustEdge:
    field = f"edges[{index}]"
    row = require_closed_keys(
        value,
        required=("source", "target", "kind", "evidence_id"),
        field=field,
    )
    return TrustEdge(
        source=require_identifier(row["source"], f"{field}.source"),
        target=require_identifier(row["target"], f"{field}.target"),
        kind=require_identifier(row["kind"], f"{field}.kind"),
        evidence_id=require_identifier(
            row["evidence_id"], f"{field}.evidence_id"
        ),
    )


def _seat_from_dict(value: object, index: int) -> SeatAssignment:
    field = f"seats[{index}]"
    row = require_closed_keys(
        value, required=("validator_id", "node"), field=field
    )
    return SeatAssignment(
        validator_id=require_identifier(
            row["validator_id"], f"{field}.validator_id"
        ),
        node=require_identifier(row["node"], f"{field}.node"),
    )


def trust_graph_evidence_from_dict(document: object) -> TrustGraphEvidence:
    """Parse the closed fixture/input schema into immutable graph evidence."""

    row = require_closed_keys(
        document,
        required=(
            "schema",
            "nodes",
            "ratified_nodes",
            "foundation_bound_nodes",
            "edges",
            "baseline_list_size",
            "seats",
            "penalized_vouchers",
        ),
        field="trust_graph",
    )
    if row["schema"] != TRUST_GRAPH_INPUT_SCHEMA:
        raise TaskNodeUnlError("unknown_schema", str(row["schema"]))
    edges = row["edges"]
    seats = row["seats"]
    if not isinstance(edges, list):
        raise TaskNodeUnlError("invalid_array", "edges")
    if not isinstance(seats, list):
        raise TaskNodeUnlError("invalid_array", "seats")
    return TrustGraphEvidence(
        nodes=_string_array(row["nodes"], "nodes"),
        ratified_nodes=_string_array(
            row["ratified_nodes"], "ratified_nodes"
        ),
        foundation_bound_nodes=_string_array(
            row["foundation_bound_nodes"],
            "foundation_bound_nodes",
        ),
        edges=tuple(
            _edge_from_dict(edge, index)
            for index, edge in enumerate(edges)
        ),
        baseline_list_size=require_int(
            row["baseline_list_size"],
            "baseline_list_size",
            minimum=1,
        ),
        seats=tuple(
            _seat_from_dict(seat, index)
            for index, seat in enumerate(seats)
        ),
        penalized_vouchers=_string_array(
            row["penalized_vouchers"], "penalized_vouchers"
        ),
    )


def derive_trust_graph_document(document: object) -> TrustGraphResult:
    """Parse and derive one explicit JSON-shaped trust-graph document."""

    return derive_trust_graph(trust_graph_evidence_from_dict(document))
