"""Placement preflight for the proposed launch validator set.

Implements the two ``new:`` placement checks of
``docs/architecture/launch-topology-thresholds.md`` (provider/ASN
concentration §2.1 and geographic concentration §2.2) together with the
declared-field checks the same document states for the remaining
dimensions (correlation-group cap §2.3, exact-binary software rule §2.4,
minimum count and quorum margin §2.5). The connected-component L3
independence verifier over the published correlation dataset remains a
separate, still-unbuilt tool; this preflight only enforces the declared
correlation-group cap.

Inputs (positional path, ``-`` for stdin):

- a plain JSON list of seat entries carrying ``provider`` (ASN name),
  ``asn``, ``country``, ``correlation_group`` and ``software_version``
  fields; or
- a canonical proposed-genesis-registry payload from
  ``postfiat_rpc.genesis_registry`` (the ``build`` output or the bare
  registry object). Canonical entries commit only evidence digests, so
  placement fields must arrive via ``--evidence`` (a JSON list of genesis
  evidence records, §2.3 of the proposal-path document); every record is
  re-digested with ``genesis_registry.evidence_digest`` and must match the
  entry's committed ``identity_evidence_digest_hex``.

Every dimension fails closed: a missing, empty, unresolved, or mismatched
field is a loud FAIL for that dimension, never a silent pass. The tool is
deterministic and offline. ``--strict`` selects the document's stricter
geographic variant; ``--json`` emits the machine-readable report; exit code
is 0 only when every dimension passes.

Run from the repository root:

    PYTHONPATH=python python3 -m postfiat_rpc.placement_preflight seats.json
    PYTHONPATH=python python3 -m postfiat_rpc.placement_preflight seats.json --strict --json
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Mapping, Sequence

from postfiat_rpc.genesis_registry import (
    GenesisRegistryError,
    evidence_digest,
    template_trust_graph,
)

# ---------------------------------------------------------------------------
# Launch thresholds, quoted from docs/architecture/launch-topology-thresholds.md.
# Every value is that document's *proposed* launch threshold at the minimum
# registry size n_S = 12; none is operator-confirmed.
# ---------------------------------------------------------------------------

#: §2.1 — "at most 2 of 12 registry seats per hosting-provider family
#: (general form: family weight ≤ n_S - q_S)".
PROVIDER_FAMILY_CAP = 2
#: §2.1 — "the same 2-seat cap per individual ASN".
ASN_CAP = 2
#: §2.2 — "at most 4 of 12 seats per country (one-third)".
COUNTRY_CAP = 4
#: §2.2 — "at least 4 distinct countries across the registry".
MIN_COUNTRIES = 4
#: §2.2 caveat (strict variant) — "the cap is 2 of 12 and at least 6 countries".
STRICT_COUNTRY_CAP = 2
STRICT_MIN_COUNTRIES = 6
#: §2.3 — "every connected component … holds fewer than 3 of 12 seats,
#: i.e. at most 2"; enforced here on declared correlation-group labels.
CORRELATION_GROUP_CAP = 2
#: §2.5 — "n_S ≥ 12 seats with q_S = ceil(4·n_S/5) = 10, an unavailability
#: margin of 2 seats".
MIN_SEATS = 12
MIN_UNAVAILABILITY_MARGIN = 2

PASS = "PASS"
FAIL = "FAIL"

# ---------------------------------------------------------------------------
# Provider-family normalization, ported from the read-only fork clone
# dynamic-unl-scoring/scoring_service/services/provider_families.py so
# corporate variants of one operator count together (§2.1 "using the fork's
# family normalization").
# ---------------------------------------------------------------------------

_NOISE_TOKENS = {
    "as", "asn", "inc", "llc", "ltd", "gmbh", "ag", "sa", "srl", "bv",
    "co", "corp", "corporation", "company", "holdings",
}
_TRAILING_COUNTRY_RE = re.compile(r",\s*[A-Za-z]{2}$")
_NON_WORD_RE = re.compile(r"[\W_]+")
_TRAILING_DIGITS_RE = re.compile(r"\d+$")
_FAMILY_RULES: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("hetzner", ("hetzner",)),
    ("vultr", ("vultr", "constant")),
    ("akamai", ("akamai", "linode")),
)


def normalize_provider_name(as_name: str) -> str:
    """Reduce a raw ASN name to its provider-identifying tokens."""
    name = _TRAILING_COUNTRY_RE.sub("", as_name.strip())
    if " - " in name:
        name = name.split(" - ", 1)[1]
    name = _NON_WORD_RE.sub(" ", name.lower())
    tokens = []
    for token in name.split():
        token = _TRAILING_DIGITS_RE.sub("", token)
        if token and token not in _NOISE_TOKENS:
            tokens.append(token)
    return " ".join(tokens)


def family_for(as_name: str) -> str:
    """Provider family for a resolved ASN name (empty input is the caller's
    fail-closed problem, never mapped to a shared 'unknown' family here)."""
    normalized = normalize_provider_name(as_name)
    if not normalized:
        return as_name.strip().lower()
    tokens = set(normalized.split())
    for family, needles in _FAMILY_RULES:
        if any(needle in tokens for needle in needles):
            return family
    return normalized


# ---------------------------------------------------------------------------
# Seat model and input loading
# ---------------------------------------------------------------------------


@dataclass
class Seat:
    """One proposed registry seat with its declared placement fields.

    ``None`` means the field is missing or unresolved; the dimension that
    needs it fails closed.
    """

    label: str
    provider: str | None
    asn: str | None
    country: str | None
    correlation_group: str | None
    software_version: str | None


@dataclass
class DimensionVerdict:
    dimension: str
    threshold: str
    verdict: str
    detail: str


def _clean(value: Any) -> str | None:
    if isinstance(value, str) and value.strip():
        return value.strip()
    return None


def _first(entry: Mapping[str, Any], *keys: str) -> Any:
    for key in keys:
        if key in entry:
            return entry[key]
    return None


def _asn_text(value: Any) -> str | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return f"AS{value}"
    if isinstance(value, str) and value.strip():
        text = value.strip().upper()
        return text if text.startswith("AS") else f"AS{text}"
    if isinstance(value, Mapping):
        return _asn_text(_first(value, "asn", "number"))
    return None


def seat_from_entry(entry: Mapping[str, Any], index: int) -> Seat:
    """Normalize one plain-JSON entry (or evidence record) into a Seat."""
    label = (
        _clean(_first(entry, "name", "validator_id", "domain"))
        or _clean(_first(entry, "fork_master_key_hex"))
        or f"seat-{index}"
    )
    if isinstance(label, str) and len(label) > 16:
        label = label[:16] + "…"
    return Seat(
        label=label,
        provider=_clean(_first(entry, "provider", "as_name", "provider_family")),
        asn=_asn_text(_first(entry, "asn", "as_number")),
        country=_clean(_first(entry, "country")),
        correlation_group=_clean(
            _first(entry, "correlation_group", "correlation-group")
        ),
        software_version=_clean(
            _first(entry, "software_version", "software-version", "binary_sha256")
        ),
    )


def _registry_object(document: Any) -> Mapping[str, Any] | None:
    """The canonical registry object, if the document is one."""
    if not isinstance(document, Mapping):
        return None
    if isinstance(document.get("registry"), Mapping):
        document = document["registry"]
    if isinstance(document.get("entries"), list) and "template_trust_graph" in document:
        return document
    return None


def load_seats(
    document: Any, evidence: Sequence[Mapping[str, Any]] | None
) -> tuple[list[Seat], Mapping[str, int] | None, list[str]]:
    """Seats, the declared trust graph (canonical mode), and loud problems.

    Problems returned here poison every placement dimension (fail-closed);
    they are unresolved records, not per-field gaps.
    """
    registry = _registry_object(document)
    if registry is None:
        if not isinstance(document, list) or not all(
            isinstance(item, Mapping) for item in document
        ):
            raise ValueError(
                "input is neither a canonical proposed-genesis-registry payload "
                "nor a plain JSON list of seat entries"
            )
        seats = [seat_from_entry(entry, index) for index, entry in enumerate(document)]
        return seats, None, []

    problems: list[str] = []
    by_key: dict[str, Mapping[str, Any]] = {}
    for record in evidence or []:
        key = record.get("fork_master_key_hex")
        if isinstance(key, str):
            by_key[key.lower()] = record
    if evidence is None:
        problems.append(
            "canonical registry entries commit evidence digests only; "
            "pass --evidence with the genesis evidence records"
        )

    seats: list[Seat] = []
    for index, entry in enumerate(registry["entries"]):
        key = str(entry.get("fork_master_key_hex", "")).lower()
        record = by_key.get(key)
        if evidence is not None and record is None:
            problems.append(f"no evidence record for entry {key[:16]}…")
        elif record is not None:
            try:
                digest = evidence_digest(record).hex()
            except (GenesisRegistryError, KeyError, ValueError) as error:
                problems.append(f"evidence record {key[:16]}… undigestable: {error}")
            else:
                if digest != entry.get("identity_evidence_digest_hex"):
                    problems.append(
                        f"evidence record {key[:16]}… does not match the "
                        "committed identity_evidence_digest_hex (unresolved record)"
                    )
        seats.append(seat_from_entry(dict(record or {}, fork_master_key_hex=key), index))
    graph = registry.get("template_trust_graph")
    return seats, graph if isinstance(graph, Mapping) else None, problems


# ---------------------------------------------------------------------------
# Dimension checks (each fails closed on missing data)
# ---------------------------------------------------------------------------


def _missing(seats: Sequence[Seat], field: str) -> list[str]:
    return [seat.label for seat in seats if getattr(seat, field) is None]


def _cap_violations(counts: Counter[str], cap: int) -> list[str]:
    return [
        f"{name}={count}"
        for name, count in sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))
        if count > cap
    ]


def _fail_closed(dimension: str, threshold: str, reasons: list[str]) -> DimensionVerdict:
    return DimensionVerdict(dimension, threshold, FAIL, "fail-closed: " + "; ".join(reasons))


def check_provider_asn(seats: Sequence[Seat]) -> DimensionVerdict:
    threshold = f"≤ {PROVIDER_FAMILY_CAP} seats per provider family and per ASN (§2.1)"
    reasons = []
    if missing := _missing(seats, "provider"):
        reasons.append(f"unresolved provider for {', '.join(missing)}")
    if missing := _missing(seats, "asn"):
        reasons.append(f"unresolved ASN for {', '.join(missing)}")
    if reasons:
        return _fail_closed("provider/ASN concentration", threshold, reasons)
    families = Counter(family_for(seat.provider) for seat in seats)  # type: ignore[arg-type]
    asns = Counter(seat.asn for seat in seats)  # type: ignore[arg-type]
    over = [f"family {item}" for item in _cap_violations(families, PROVIDER_FAMILY_CAP)]
    over += [f"ASN {item}" for item in _cap_violations(asns, ASN_CAP)]
    if over:
        return DimensionVerdict(
            "provider/ASN concentration", threshold, FAIL, "over cap: " + "; ".join(over)
        )
    return DimensionVerdict(
        "provider/ASN concentration",
        threshold,
        PASS,
        f"{len(families)} families, {len(asns)} ASNs, largest family "
        f"{max(families.values())} seat(s)",
    )


def check_country(seats: Sequence[Seat], strict: bool) -> DimensionVerdict:
    cap = STRICT_COUNTRY_CAP if strict else COUNTRY_CAP
    floor = STRICT_MIN_COUNTRIES if strict else MIN_COUNTRIES
    variant = "strict §2.2 caveat" if strict else "§2.2"
    threshold = f"≤ {cap} seats per country, ≥ {floor} countries ({variant})"
    if missing := _missing(seats, "country"):
        return _fail_closed(
            "geographic concentration",
            threshold,
            [f"unresolved country for {', '.join(missing)}"],
        )
    countries = Counter(seat.country for seat in seats)  # type: ignore[arg-type]
    failures = [f"country {item}" for item in _cap_violations(countries, cap)]
    if len(countries) < floor:
        failures.append(f"only {len(countries)} distinct countries (need ≥ {floor})")
    if failures:
        return DimensionVerdict(
            "geographic concentration", threshold, FAIL, "; ".join(failures)
        )
    return DimensionVerdict(
        "geographic concentration",
        threshold,
        PASS,
        f"{len(countries)} countries, largest {max(countries.values())} seat(s)",
    )


def check_correlation(seats: Sequence[Seat]) -> DimensionVerdict:
    threshold = f"≤ {CORRELATION_GROUP_CAP} seats per declared correlation group (§2.3)"
    if missing := _missing(seats, "correlation_group"):
        return _fail_closed(
            "operator independence (declared groups)",
            threshold,
            [f"unresolved correlation group for {', '.join(missing)}"],
        )
    groups = Counter(seat.correlation_group for seat in seats)  # type: ignore[arg-type]
    if over := _cap_violations(groups, CORRELATION_GROUP_CAP):
        return DimensionVerdict(
            "operator independence (declared groups)",
            threshold,
            FAIL,
            "over cap: " + "; ".join(f"group {item}" for item in over),
        )
    return DimensionVerdict(
        "operator independence (declared groups)",
        threshold,
        PASS,
        f"{len(groups)} groups, largest {max(groups.values())} seat(s); "
        "connected-component L3 verification is a separate tool",
    )


def check_software(seats: Sequence[Seat], pinned: str | None) -> DimensionVerdict:
    threshold = "all seats on one exact pinned release binary, zero skew (§2.4)"
    if missing := _missing(seats, "software_version"):
        return _fail_closed(
            "software diversity (exact binary)",
            threshold,
            [f"unresolved software version for {', '.join(missing)}"],
        )
    versions = Counter(seat.software_version for seat in seats)  # type: ignore[arg-type]
    if len(versions) != 1:
        return DimensionVerdict(
            "software diversity (exact binary)",
            threshold,
            FAIL,
            "version skew: " + ", ".join(f"{v}={c}" for v, c in sorted(versions.items())),
        )
    (version,) = versions
    if pinned is not None and version != pinned:
        return DimensionVerdict(
            "software diversity (exact binary)",
            threshold,
            FAIL,
            f"fleet on {version}, pinned binary is {pinned}",
        )
    return DimensionVerdict(
        "software diversity (exact binary)", threshold, PASS, f"all seats on {version}"
    )


def check_count_quorum(
    seats: Sequence[Seat], declared_graph: Mapping[str, int] | None
) -> DimensionVerdict:
    threshold = (
        f"n_S ≥ {MIN_SEATS}, q_S = ceil(4·n_S/5), "
        f"margin n_S − q_S ≥ {MIN_UNAVAILABILITY_MARGIN} (§2.5)"
    )
    n = len(seats)
    failures = []
    if n < MIN_SEATS:
        failures.append(f"{n} seats (need ≥ {MIN_SEATS})")
    q = math.ceil(4 * n / 5)
    if n - q < MIN_UNAVAILABILITY_MARGIN:
        failures.append(
            f"unavailability margin {n - q} (need ≥ {MIN_UNAVAILABILITY_MARGIN})"
        )
    if declared_graph is not None:
        try:
            expected = template_trust_graph(n)
        except GenesisRegistryError as error:
            failures.append(f"trust graph unsafe at n={n}: {error.code}")
        else:
            if dict(declared_graph) != expected:
                failures.append(
                    f"declared trust graph {dict(declared_graph)} != template {expected}"
                )
    if failures:
        return DimensionVerdict(
            "minimum count and quorum margin", threshold, FAIL, "; ".join(failures)
        )
    return DimensionVerdict(
        "minimum count and quorum margin",
        threshold,
        PASS,
        f"n_S={n}, q_S={q}, margin={n - q}",
    )


# ---------------------------------------------------------------------------
# Evaluation and CLI
# ---------------------------------------------------------------------------


def evaluate(
    document: Any,
    *,
    strict: bool = False,
    pinned: str | None = None,
    evidence: Sequence[Mapping[str, Any]] | None = None,
) -> dict[str, Any]:
    """Full placement-preflight report for one proposed validator set."""
    seats, declared_graph, problems = load_seats(document, evidence)
    verdicts = [
        check_provider_asn(seats),
        check_country(seats, strict),
        check_correlation(seats),
        check_software(seats, pinned),
        check_count_quorum(seats, declared_graph),
    ]
    if problems:
        # Unresolved records poison every placement dimension, never the
        # count arithmetic, which is computed from the committed entries.
        verdicts = [
            _fail_closed(v.dimension, v.threshold, problems)
            if v.dimension != "minimum count and quorum margin"
            else v
            for v in verdicts
        ]
    overall = PASS if all(v.verdict == PASS for v in verdicts) else FAIL
    return {
        "thresholds_document": "docs/architecture/launch-topology-thresholds.md",
        "strict_geographic_variant": strict,
        "seats": len(seats),
        "dimensions": [asdict(v) for v in verdicts],
        "overall": overall,
    }


def render_table(report: Mapping[str, Any]) -> str:
    rows = [("Dimension", "Verdict", "Detail")]
    for dim in report["dimensions"]:
        rows.append((dim["dimension"], dim["verdict"], dim["detail"]))
    widths = [max(len(row[i]) for row in rows) for i in range(2)]
    lines = [
        f"placement preflight — {report['seats']} seats"
        + (" (strict geographic variant)" if report["strict_geographic_variant"] else "")
    ]
    for row in rows:
        lines.append(f"{row[0]:<{widths[0]}}  {row[1]:<{widths[1]}}  {row[2]}")
    lines.append(f"overall: {report['overall']}")
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="placement_preflight", description=__doc__.split("\n\n")[0]
    )
    parser.add_argument("path", help="proposed validator set JSON ('-' for stdin)")
    parser.add_argument(
        "--evidence",
        help="genesis evidence records JSON for canonical-registry inputs",
    )
    parser.add_argument(
        "--pinned",
        help="pinned release binary identifier every seat must match exactly",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="use the stricter §2.2 geographic variant (≤ 2 per country, ≥ 6 countries)",
    )
    parser.add_argument("--json", action="store_true", help="emit the JSON report")
    return parser


def _load(path: str) -> Any:
    if path == "-":
        return json.load(sys.stdin)
    return json.loads(Path(path).read_text())


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        document = _load(args.path)
        evidence = None
        if args.evidence is not None:
            loaded = _load(args.evidence)
            evidence = loaded.get("records") if isinstance(loaded, Mapping) else loaded
        report = evaluate(
            document, strict=args.strict, pinned=args.pinned, evidence=evidence
        )
    except (OSError, ValueError) as error:
        print(f"placement_preflight: {error}", file=sys.stderr)
        return 2
    print(json.dumps(report, indent=2) if args.json else render_table(report))
    return 0 if report["overall"] == PASS else 1


if __name__ == "__main__":
    sys.exit(main())
