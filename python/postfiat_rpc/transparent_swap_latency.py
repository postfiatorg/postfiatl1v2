"""Timing primitives for the transparent transfer-settled swap benchmark.

The benchmark's C4 wall clock is intentionally broader than consensus time.
This module keeps every nested client phase explicit without changing the
six-validator verification or certified-finality acceptance standards.
"""

from __future__ import annotations

from contextlib import contextmanager
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import asdict, dataclass, field
import json
from threading import Event, Lock
import time
from typing import Any, Callable, Iterator, Mapping, TypeVar

from postfiat_rpc.persistent_client import PersistentPostFiatRpcClient


T = TypeVar("T")


class FleetPropagationError(RuntimeError):
    """Base error for read-only fleet certified-point observation."""


class FleetPropagationDivergence(FleetPropagationError):
    """A validator reported a conflicting or impossible certified point."""


class FleetPropagationTimeout(FleetPropagationError):
    """Not all validators reached the certified point within the bound."""


@dataclass(frozen=True)
class CertifiedParent:
    height: int
    block_hash: str
    state_root: str


@dataclass(frozen=True)
class FleetPropagationReport:
    target: CertifiedParent
    rounds: int
    elapsed_ms: float
    validators: dict[str, dict[str, Any]]
    adaptive_waits_ms: tuple[float, ...]


class TransparentSwapFleetSession:
    """Run-scoped, identity-checked topology and persistent RPC connections."""

    def __init__(
        self,
        *,
        endpoints: Mapping[str, str],
        clients: Mapping[str, PersistentPostFiatRpcClient],
        initial_info: Mapping[str, Mapping[str, Any]],
    ) -> None:
        self.endpoints = dict(endpoints)
        self.clients = dict(clients)
        self.initial_info = {node: dict(info) for node, info in initial_info.items()}
        self.validators = tuple(sorted(self.endpoints))

    @classmethod
    def resolve(
        cls,
        endpoints: Mapping[str, str],
        *,
        client_factory: Callable[[str], PersistentPostFiatRpcClient] = (
            lambda endpoint: PersistentPostFiatRpcClient(endpoint, timeout_seconds=60)
        ),
    ) -> "TransparentSwapFleetSession":
        if len(endpoints) != 6:
            raise ValueError(f"topology requires exactly 6 validators, got {len(endpoints)}")
        clients = {node: client_factory(endpoint) for node, endpoint in endpoints.items()}
        try:
            with ThreadPoolExecutor(max_workers=6, thread_name_prefix="topology-resolve") as executor:
                futures = {executor.submit(client.server_info): node for node, client in clients.items()}
                info = {futures[future]: future.result() for future in as_completed(futures)}
            cls._validate_topology(endpoints, info)
            return cls(endpoints=endpoints, clients=clients, initial_info=info)
        except Exception:
            for client in clients.values():
                client.close()
            raise

    @staticmethod
    def _validate_topology(
        endpoints: Mapping[str, str], info: Mapping[str, Mapping[str, Any]]
    ) -> None:
        if set(info) != set(endpoints):
            raise RuntimeError("topology resolution returned an incomplete fleet")
        domains = set()
        points = set()
        for node, row in info.items():
            if row.get("node_id") != node:
                raise RuntimeError(
                    f"topology endpoint identity mismatch: expected={node} actual={row.get('node_id')}"
                )
            validators = row.get("validators")
            ledger = row.get("ledger")
            mempool = row.get("mempool")
            if not isinstance(validators, Mapping) or validators.get("active_count") != 6:
                raise RuntimeError(f"topology {node} does not report six active validators")
            if not isinstance(ledger, Mapping) or not isinstance(mempool, Mapping):
                raise RuntimeError(f"topology {node} is missing ledger or mempool state")
            if mempool.get("pending") != 0:
                raise RuntimeError(f"topology {node} has a nonempty mempool")
            domains.add((row.get("chain_id"), row.get("genesis_hash"), row.get("protocol_version")))
            points.add((ledger.get("height"), ledger.get("hash"), ledger.get("state_root")))
        if len(domains) != 1:
            raise RuntimeError(f"topology domain mismatch: {domains}")
        if len(points) != 1:
            raise RuntimeError(f"topology ledger mismatch: {points}")

    @property
    def initial_parent(self) -> CertifiedParent:
        ledger = self.initial_info[self.validators[0]]["ledger"]
        return CertifiedParent(
            height=int(ledger["height"]),
            block_hash=str(ledger["hash"]),
            state_root=str(ledger["state_root"]),
        )

    def proposer_for_height(self, height: int, *, view: int = 0) -> str:
        if height < 1 or view < 0:
            raise ValueError("height must be positive and view must be nonnegative")
        return self.validators[((height % len(self.validators)) + (view % len(self.validators))) % len(self.validators)]

    def proposer_for_parent(self, parent: CertifiedParent) -> str:
        return self.proposer_for_height(parent.height + 1)

    def client(self, node: str) -> PersistentPostFiatRpcClient:
        return self.clients[node]

    def close(self) -> None:
        for client in self.clients.values():
            client.close()

    def __enter__(self) -> "TransparentSwapFleetSession":
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    @staticmethod
    def proxy_parent_params(parent: CertifiedParent) -> dict[str, Any]:
        return {
            "proxy_required_current_height": parent.height,
            "proxy_required_parent_hash": parent.block_hash,
            "proxy_required_state_root": parent.state_root,
        }

    def submit_asset_finality(
        self,
        signed: Mapping[str, Any],
        parent: CertifiedParent,
        *,
        request_id: str,
    ) -> tuple[dict[str, Any], str, CertifiedParent]:
        proposer = self.proposer_for_parent(parent)
        params = {
            "signed_asset_transaction_json": json.dumps(signed, separators=(",", ":")),
            **self.proxy_parent_params(parent),
        }
        response = self.client(proposer).call_response(
            "mempool_submit_signed_asset_transaction_finality",
            params,
            request_id=request_id,
        )
        result = response.get("result")
        finality = result.get("finality") if isinstance(result, Mapping) else None
        block = finality.get("block") if isinstance(finality, Mapping) else None
        header = block.get("header") if isinstance(block, Mapping) else None
        if not isinstance(result, dict) or not isinstance(header, Mapping):
            raise RuntimeError("certified asset response is missing its finality header")
        if int(header.get("height", -1)) != parent.height + 1:
            raise RuntimeError("certified asset response height does not extend cached parent")
        if header.get("parent_hash") != parent.block_hash:
            raise RuntimeError("certified asset response does not bind cached parent hash")
        if header.get("proposer") != proposer:
            raise RuntimeError("certified asset response proposer does not match cached topology")
        next_parent = CertifiedParent(
            height=int(header["height"]),
            block_hash=str(header["block_hash"]),
            state_root=str(header["state_root"]),
        )
        return result, proposer, next_parent


def wait_for_certified_parent_six(
    session: TransparentSwapFleetSession,
    target: CertifiedParent,
    timeline: "PhaseTimeline",
    *,
    timeout_seconds: float = 12.0,
    initial_wait_seconds: float = 0.01,
    max_wait_seconds: float = 0.1,
    clock: Callable[[], float] = time.monotonic,
    wait: Callable[[float], Any] | None = None,
) -> FleetPropagationReport:
    """Wait read-only until every validator reports one exact certified point.

    Probes run concurrently. Lag is retryable only while a validator is below
    the target height. The target height with a different hash/root, or a
    height beyond the target, is a hard divergence because current-tip reads
    can no longer prove this exact certified point. Backoff grows adaptively
    from ``initial_wait_seconds`` to ``max_wait_seconds``; no mutation is ever
    retried or resubmitted.
    """

    if timeout_seconds <= 0:
        raise ValueError("propagation timeout must be positive")
    if initial_wait_seconds <= 0 or max_wait_seconds < initial_wait_seconds:
        raise ValueError("adaptive wait bounds are invalid")
    wait = wait or (lambda seconds: Event().wait(seconds))
    started = clock()
    deadline = started + timeout_seconds
    rounds = 0
    delay = initial_wait_seconds
    adaptive_waits: list[float] = []
    last_rows: dict[str, dict[str, Any]] = {}
    phase_started_ns = timeline.now_ns()
    try:
        while True:
            rounds += 1
            with ThreadPoolExecutor(
                max_workers=6, thread_name_prefix="certified-point-wait"
            ) as executor:
                futures = {
                    executor.submit(client.server_info): node
                    for node, client in session.clients.items()
                }
                rows = {futures[future]: future.result() for future in as_completed(futures)}
            if set(rows) != set(session.validators):
                raise FleetPropagationDivergence(
                    "certified-point probe returned an incomplete fleet"
                )
            pending: list[str] = []
            normalized: dict[str, dict[str, Any]] = {}
            for node in session.validators:
                row = rows[node]
                if row.get("node_id") != node:
                    raise FleetPropagationDivergence(
                        f"certified-point endpoint identity mismatch: expected={node} "
                        f"actual={row.get('node_id')}"
                    )
                ledger = row.get("ledger")
                if not isinstance(ledger, Mapping):
                    raise FleetPropagationDivergence(
                        f"certified-point response from {node} is missing ledger state"
                    )
                height_value = ledger.get("height")
                if not isinstance(height_value, int) or isinstance(height_value, bool):
                    raise FleetPropagationDivergence(
                        f"certified-point response from {node} has invalid ledger height"
                    )
                height = height_value
                block_hash = str(ledger.get("hash", ""))
                state_root = str(ledger.get("state_root", ""))
                normalized[node] = {
                    "height": height,
                    "block_hash": block_hash,
                    "state_root": state_root,
                }
                if height < target.height:
                    pending.append(node)
                    continue
                if height > target.height:
                    raise FleetPropagationDivergence(
                        f"{node} advanced beyond certified target before exact observation: "
                        f"target_height={target.height} actual_height={height}"
                    )
                if block_hash != target.block_hash or state_root != target.state_root:
                    raise FleetPropagationDivergence(
                        f"{node} conflicts at certified target height {target.height}: "
                        f"expected_hash={target.block_hash} actual_hash={block_hash} "
                        f"expected_root={target.state_root} actual_root={state_root}"
                    )
            last_rows = normalized
            if not pending:
                ended = clock()
                return FleetPropagationReport(
                    target=target,
                    rounds=rounds,
                    elapsed_ms=(ended - started) * 1000.0,
                    validators=last_rows,
                    adaptive_waits_ms=tuple(value * 1000.0 for value in adaptive_waits),
                )
            now = clock()
            remaining = deadline - now
            if remaining <= 0:
                raise FleetPropagationTimeout(
                    f"six-validator certified-point wait timed out after {rounds} rounds; "
                    f"lagging={pending} target_height={target.height} last={last_rows}"
                )
            bounded_delay = min(delay, remaining)
            adaptive_waits.append(bounded_delay)
            wait(bounded_delay)
            delay = min(delay * 2.0, max_wait_seconds)
    finally:
        timeline.record(
            "verify.six_certified_parent_wait",
            phase_started_ns,
            timeline.now_ns(),
            target_height=target.height,
            rounds=rounds,
            adaptive_waits_ms=[value * 1000.0 for value in adaptive_waits],
        )


def wait_for_certified_parent_six_independent(
    session: TransparentSwapFleetSession,
    target: CertifiedParent,
    timeline: "PhaseTimeline",
    *,
    timeout_seconds: float = 12.0,
    initial_wait_seconds: float = 0.01,
    max_wait_seconds: float = 0.1,
    clock: Callable[[], float] = time.monotonic,
    wait: Callable[[float], Any] | None = None,
) -> FleetPropagationReport:
    """Watch each validator independently until all report the exact target.

    Unlike barrier-style rounds, a validator stops generating WAN reads as
    soon as it reaches the target. A shared cancellation event bounds sibling
    workers after any hard divergence. Poll waits grow adaptively and remain
    read-only; no transaction or certified request is retried.
    """

    if len(session.clients) != 6 or set(session.clients) != set(session.validators):
        raise ValueError("independent propagation wait requires exactly six topology clients")
    if timeout_seconds <= 0:
        raise ValueError("propagation timeout must be positive")
    if initial_wait_seconds <= 0 or max_wait_seconds < initial_wait_seconds:
        raise ValueError("adaptive wait bounds are invalid")
    started = clock()
    deadline = started + timeout_seconds
    cancelled = Event()
    wait_one = wait or cancelled.wait

    def watch(node: str, client: PersistentPostFiatRpcClient) -> dict[str, Any]:
        attempts = 0
        delay = initial_wait_seconds
        waits: list[float] = []
        phase_started_ns = timeline.now_ns()
        try:
            while not cancelled.is_set():
                attempts += 1
                row = client.server_info()
                if row.get("node_id") != node:
                    raise FleetPropagationDivergence(
                        f"certified-point endpoint identity mismatch: expected={node} "
                        f"actual={row.get('node_id')}"
                    )
                ledger = row.get("ledger")
                if not isinstance(ledger, Mapping):
                    raise FleetPropagationDivergence(
                        f"certified-point response from {node} is missing ledger state"
                    )
                height_value = ledger.get("height")
                if not isinstance(height_value, int) or isinstance(height_value, bool):
                    raise FleetPropagationDivergence(
                        f"certified-point response from {node} has invalid ledger height"
                    )
                block_hash = str(ledger.get("hash", ""))
                state_root = str(ledger.get("state_root", ""))
                if height_value == target.height:
                    if block_hash != target.block_hash or state_root != target.state_root:
                        raise FleetPropagationDivergence(
                            f"{node} conflicts at certified target height {target.height}: "
                            f"expected_hash={target.block_hash} actual_hash={block_hash} "
                            f"expected_root={target.state_root} actual_root={state_root}"
                        )
                    return {
                        "height": height_value,
                        "block_hash": block_hash,
                        "state_root": state_root,
                        "attempts": attempts,
                        "adaptive_waits_ms": [value * 1000.0 for value in waits],
                    }
                if height_value > target.height:
                    raise FleetPropagationDivergence(
                        f"{node} advanced beyond certified target before exact observation: "
                        f"target_height={target.height} actual_height={height_value}"
                    )
                remaining = deadline - clock()
                if remaining <= 0:
                    raise FleetPropagationTimeout(
                        f"{node} certified-point wait timed out after {attempts} attempts; "
                        f"target_height={target.height} actual_height={height_value}"
                    )
                bounded_delay = min(delay, remaining)
                waits.append(bounded_delay)
                wait_one(bounded_delay)
                delay = min(delay * 2.0, max_wait_seconds)
            raise FleetPropagationError(f"{node} certified-point watcher cancelled")
        finally:
            timeline.record(
                f"verify.propagation.{node}",
                phase_started_ns,
                timeline.now_ns(),
                attempts=attempts,
                adaptive_waits_ms=[value * 1000.0 for value in waits],
            )

    phase_started_ns = timeline.now_ns()
    rows: dict[str, dict[str, Any]] = {}
    try:
        with ThreadPoolExecutor(
            max_workers=6, thread_name_prefix="independent-certified-point"
        ) as executor:
            futures = {
                executor.submit(watch, node, session.clients[node]): node
                for node in session.validators
            }
            for future in as_completed(futures):
                try:
                    rows[futures[future]] = future.result()
                except Exception:
                    cancelled.set()
                    raise
    finally:
        timeline.record(
            "verify.six_certified_parent_wait_independent",
            phase_started_ns,
            timeline.now_ns(),
            target_height=target.height,
            completed_validator_count=len(rows),
        )
    if set(rows) != set(session.validators):
        raise FleetPropagationError("independent propagation wait returned incomplete fleet")
    ended = clock()
    ordered = {node: rows[node] for node in session.validators}
    waits = tuple(
        wait_ms
        for node in session.validators
        for wait_ms in ordered[node]["adaptive_waits_ms"]
    )
    return FleetPropagationReport(
        target=target,
        rounds=max(int(row["attempts"]) for row in ordered.values()),
        elapsed_ms=(ended - started) * 1000.0,
        validators=ordered,
        adaptive_waits_ms=waits,
    )


@dataclass(frozen=True)
class PhaseTiming:
    name: str
    started_ns: int
    ended_ns: int
    elapsed_ms: float
    metadata: dict[str, Any] = field(default_factory=dict)


class PhaseTimeline:
    """Monotonic, artifact-ready timing recorder.

    The recorder never uses wall time for duration calculations. Records are
    append-only and preserve execution order so failures can retain a partial
    but truthful timeline.
    """

    def __init__(self, *, clock_ns: Callable[[], int] = time.monotonic_ns) -> None:
        self._clock_ns = clock_ns
        self._records: list[PhaseTiming] = []
        self._records_lock = Lock()

    @contextmanager
    def phase(self, name: str, **metadata: Any) -> Iterator[None]:
        if not name:
            raise ValueError("phase name is required")
        started_ns = self._clock_ns()
        try:
            yield
        finally:
            ended_ns = self._clock_ns()
            self.record(name, started_ns, ended_ns, **metadata)

    def call(self, name: str, operation: Callable[[], T], **metadata: Any) -> T:
        with self.phase(name, **metadata):
            return operation()

    def record(self, name: str, started_ns: int, ended_ns: int, **metadata: Any) -> None:
        if not name:
            raise ValueError("phase name is required")
        if ended_ns < started_ns:
            raise ValueError("phase end precedes phase start")
        with self._records_lock:
            self._records.append(
                PhaseTiming(
                    name=name,
                    started_ns=started_ns,
                    ended_ns=ended_ns,
                    elapsed_ms=(ended_ns - started_ns) / 1_000_000,
                    metadata=dict(metadata),
                )
            )

    @property
    def records(self) -> tuple[PhaseTiming, ...]:
        with self._records_lock:
            return tuple(self._records)

    def now_ns(self) -> int:
        """Read the recorder's monotonic clock for overlapping milestones."""

        return self._clock_ns()

    def artifact(self) -> dict[str, Any]:
        records = self.records
        return {
            "schema": "postfiat-transparent-swap-phase-timings-v1",
            "clock": "monotonic_ns",
            "records": [asdict(record) for record in records],
        }


def instrument_certified_leg(
    *,
    label: str,
    timeline: PhaseTimeline,
    discover: Callable[[], T],
    quote: Callable[[T], Any],
    sign: Callable[[Any], Any],
    submit: Callable[[Any, T], Any],
    response_is_certified: Callable[[Any], bool],
) -> Any:
    """Run one baseline leg while exposing each client ceremony phase.

    Current finality RPC returns only after certification, so first response
    and certified finality are the same milestone. Both measurements are
    emitted explicitly and marked as overlapping; consumers must not add them.
    """

    discovery = timeline.call(f"{label}.discovery", discover)
    quoted = timeline.call(f"{label}.quote", lambda: quote(discovery))
    signed = timeline.call(f"{label}.sign", lambda: sign(quoted))
    started_ns = timeline.now_ns()
    response = submit(signed, discovery)
    ended_ns = timeline.now_ns()
    certified = response_is_certified(response)
    timeline.record(
        f"{label}.submit_to_first_response",
        started_ns,
        ended_ns,
        certified_in_response=certified,
        overlaps_certified_finality=True,
    )
    if not certified:
        raise RuntimeError(f"{label} first response did not contain certified finality")
    timeline.record(
        f"{label}.submit_to_certified_finality",
        started_ns,
        ended_ns,
        finality_in_first_response=True,
        overlaps_first_response=True,
    )
    return response


def verify_six_sequential(
    validators: Mapping[str, T],
    verify_one: Callable[[str, T], Any],
    timeline: PhaseTimeline,
) -> dict[str, Any]:
    """Baseline six-validator verification with per-validator timers."""

    if len(validators) != 6:
        raise ValueError(f"verification requires exactly 6 validators, got {len(validators)}")
    rows: dict[str, Any] = {}
    for validator_id, endpoint in validators.items():
        rows[validator_id] = timeline.call(
            f"verify.{validator_id}",
            lambda validator_id=validator_id, endpoint=endpoint: verify_one(validator_id, endpoint),
            validator_id=validator_id,
        )
    return rows


def verify_six_concurrent(
    validators: Mapping[str, T],
    verify_one: Callable[[str, T], Any],
    timeline: PhaseTimeline,
) -> dict[str, Any]:
    """Query all six validators concurrently and require every exact result.

    This changes only client scheduling. A failure or missing response from any
    validator fails the whole verification; successful peers never dilute it.
    """

    if len(validators) != 6:
        raise ValueError(f"verification requires exactly 6 validators, got {len(validators)}")
    started_ns = timeline.now_ns()
    rows: dict[str, Any] = {}
    with ThreadPoolExecutor(max_workers=6, thread_name_prefix="validator-verify") as executor:
        futures = {
            executor.submit(
                timeline.call,
                f"verify.{validator_id}",
                lambda validator_id=validator_id, endpoint=endpoint: verify_one(
                    validator_id, endpoint
                ),
                validator_id=validator_id,
            ): validator_id
            for validator_id, endpoint in validators.items()
        }
        for future in as_completed(futures):
            rows[futures[future]] = future.result()
    ended_ns = timeline.now_ns()
    if set(rows) != set(validators):
        raise RuntimeError("six-validator verification returned an incomplete fleet")
    timeline.record(
        "verify.six_concurrent_total",
        started_ns,
        ended_ns,
        required_validator_count=6,
        returned_validator_count=len(rows),
    )
    return {validator_id: rows[validator_id] for validator_id in validators}


def prepare_two_settlement_legs_concurrent(
    legs: Mapping[str, T],
    prepare_one: Callable[[str, T], Any],
    timeline: PhaseTimeline,
) -> dict[str, Any]:
    """Prepare two independent signer legs concurrently, without submitting.

    The function returns only after both preparations succeed. A quote/sign
    failure raises before the caller can begin either certified submission.
    Certified block production intentionally remains outside this helper and
    serialized by parent height.
    """

    if len(legs) != 2:
        raise ValueError(f"settlement preparation requires exactly 2 legs, got {len(legs)}")
    started_ns = timeline.now_ns()
    prepared: dict[str, Any] = {}
    try:
        with ThreadPoolExecutor(max_workers=2, thread_name_prefix="settlement-prepare") as executor:
            futures = {
                executor.submit(
                    timeline.call,
                    f"{label}.prepare_quote_and_sign",
                    lambda label=label, leg=leg: prepare_one(label, leg),
                    settlement_leg=label,
                ): label
                for label, leg in legs.items()
            }
            for future in as_completed(futures):
                prepared[futures[future]] = future.result()
    finally:
        timeline.record(
            "settlement.prepare_two_concurrent_total",
            started_ns,
            timeline.now_ns(),
            required_leg_count=2,
            prepared_leg_count=len(prepared),
        )
    if set(prepared) != set(legs):
        raise RuntimeError("concurrent settlement preparation returned incomplete legs")
    return {label: prepared[label] for label in legs}


def timing_names(artifact: Mapping[str, Any]) -> set[str]:
    """Return names from a serialized timing artifact for gates/tests."""

    records = artifact.get("records")
    if not isinstance(records, list):
        return set()
    return {
        str(record.get("name"))
        for record in records
        if isinstance(record, dict) and isinstance(record.get("name"), str)
    }
