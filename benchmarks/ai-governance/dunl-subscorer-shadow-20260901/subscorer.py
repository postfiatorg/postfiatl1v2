"""Deterministic sub-scorers v1 for the Dynamic UNL shadow evaluation.

Computes the five dimensional sub-scores (consensus, reliability, software,
diversity, identity) for every validator of a frozen testnet round purely
from that round's frozen inputs (``inputs/model_request.json``). Integer
0-100 outputs, deterministic, no network, no model. This is the candidate
replacement for the model's per-dimension judgment under prompt v8+, where
the model's overall score is already advisory and authority means the five
sub-scores.

Every rule below reads only fields present in the frozen validator entries.
The rules are grounded in the published scoring rubric (prompt v9/v10
system message) so the deterministic scorer and the model are graded
against the same published expectations, but they are this module's own
contract: transparent, recomputable by anyone from the frozen round.

CONSENSUS - from the three frozen agreement windows (1h, 24h, 30d):
    consensus = floor(100 * min(agreement scores across the three windows))
    A null/missing window counts as 0.0 (no evidence of participation).
    This is the rubric's worst-window ceiling, applied literally: the
    validation record itself, at full resolution, with recent non-participation
    (1h/24h near zero) automatically collapsing the score. Monotone: strictly
    better windows can never score lower.

RELIABILITY - the stability pattern behind the record (where the losses sit
in time), from the same frozen windows (null window counts as 0.0):
    m = min(1h, 24h) is the recent read; s30 the long-term read.
    - m < 0.5                         -> 10  (currently broken/offline)
    - recent clean (m >= 0.999):            (judge the 30d residue)
        s30 >= 0.9995 -> 100    s30 >= 0.999 -> 95    s30 >= 0.995 -> 90
        s30 >= 0.99   -> 85     s30 >= 0.95  -> 80    else         -> 75
    - recent instability (0.5 <= m < 0.999):
        m >= 0.99 -> 65    m >= 0.95 -> 55    m >= 0.9 -> 50    else -> 40
    Bands mirror the rubric's guide (fully stable 95-100, old resolved
    incident 75-90, recent/chronic instability 40-70, broken < 40).
    Domain/identity evidence never touches reliability.

SOFTWARE - from ``server_version`` ordered against the round's own set
(the frozen inputs carry no explicit current-release field, so the newest
version present in the set is the reference, per the rubric):
    latest in set -> 100; same major.minor (patch behind) -> 80;
    same major -> 60; older major -> 40; missing/unparsable -> 50
    (documented neutral: no version evidence, no invented signal).
    Fee votes are ignored in v1: ``base_fee`` is uniformly 10 across every
    validator of all eight frozen rounds 12-19, so the frozen evidence
    carries no fee-vote signal to score.

DIVERSITY - counts-based concentration over the round's own candidate set,
isolated from all other evidence:
    provider key = ``provider_family`` where the round supplies it (rounds
    17-19); earlier rounds carry no family-grouping evidence, so the raw
    ASN number is the provider key there (documented limitation: corporate
    variants of one operator count as separate providers in rounds 12-16).
    n_p = validators sharing the provider key, n_c = validators sharing the
    country, both counted over resolved entries of the frozen set.
    - unresolved endpoint (null asn or null geolocation, or provider_family
      "unknown") -> 30 flat: the missing-endpoint policy - unknown
      concentration is a risk, but not proof of poor operation.
    - resolved: diversity = 20 + P + C with
        P = max(0, 50 - 5 * (n_p - 1))   (provider-family axis)
        C = max(0, 30 - 3 * (n_c - 1))   (country axis)
      Strictly decreasing in each count until the penalty saturates at 0,
      equal count-pairs score identically; range 20-100. Like consensus,
      diversity is computed at full resolution, not banded to multiples
      of 5.

IDENTITY - accountability evidence only:
    domain_verified true -> 80; domain present but unverified -> 55;
    no domain -> 45  (the rubric's verified-domain 75-85 / no-domain 45-55
    bands, pinned to single deterministic values).
    The formal ``identity`` field is null for every validator in all eight
    frozen rounds (identity verification is not deployed on this network),
    so v1 treats it as neutral and scores from domain evidence alone - the
    frozen evidence genuinely cannot support a formal-identity rule.
"""

from __future__ import annotations

import json
import re
from fractions import Fraction
from pathlib import Path

DIMENSIONS = ("consensus", "reliability", "software", "diversity", "identity")

VALIDATOR_DATA_MARKER = "VALIDATOR DATA:"

_VERSION_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


# ---------------------------------------------------------------------------
# Frozen-input extraction
# ---------------------------------------------------------------------------

def extract_validator_entries(model_request: dict) -> list[dict]:
    """Extract the frozen validator entries from ``inputs/model_request.json``.

    The entries are the JSON array embedded after the ``VALIDATOR DATA:``
    marker in the round's frozen user message.
    """
    content = model_request["messages"][-1]["content"]
    index = content.find(VALIDATOR_DATA_MARKER)
    if index < 0:
        raise ValueError("frozen request carries no VALIDATOR DATA block")
    payload = content[index + len(VALIDATOR_DATA_MARKER):].strip()
    entries, _ = json.JSONDecoder().raw_decode(payload)
    return entries


def load_round_entries(round_dir: Path) -> list[dict]:
    """Load the frozen validator entries of a fetched round directory."""
    request = json.loads((round_dir / "inputs" / "model_request.json").read_text())
    return extract_validator_entries(request)


# ---------------------------------------------------------------------------
# Per-dimension rules (see module docstring for the rule statements)
# ---------------------------------------------------------------------------

def _window_score(entry: dict, window: str) -> float:
    data = entry.get(f"agreement_{window}")
    if not data or data.get("score") is None:
        return 0.0
    return float(data["score"])


def score_consensus(entry: dict) -> int:
    worst = min(_window_score(entry, w) for w in ("1h", "24h", "30d"))
    # Exact decimal floor: the frozen scores are short decimals, so going
    # through the round-trip string repr avoids float artifacts such as
    # 0.83 * 100 == 82.999... truncating to 82.
    return int(Fraction(str(worst)) * 100)


def score_reliability(entry: dict) -> int:
    recent = min(_window_score(entry, "1h"), _window_score(entry, "24h"))
    s30 = _window_score(entry, "30d")
    if recent < 0.5:
        return 10
    if recent >= 0.999:
        for threshold, score in (
            (0.9995, 100), (0.999, 95), (0.995, 90), (0.99, 85), (0.95, 80),
        ):
            if s30 >= threshold:
                return score
        return 75
    for threshold, score in ((0.99, 65), (0.95, 55), (0.9, 50)):
        if recent >= threshold:
            return score
    return 40


def _parse_version(version: object) -> tuple[int, int, int] | None:
    if not isinstance(version, str):
        return None
    match = _VERSION_RE.match(version.strip())
    if match is None:
        return None
    return tuple(int(part) for part in match.groups())


def score_software(entry: dict, newest: tuple[int, int, int] | None) -> int:
    version = _parse_version(entry.get("server_version"))
    if version is None or newest is None:
        return 50
    if version >= newest:
        return 100
    if version[:2] == newest[:2]:
        return 80
    if version[0] == newest[0]:
        return 60
    return 40


def _provider_key(entry: dict) -> str | None:
    family = entry.get("provider_family")
    if family is not None:
        return None if family == "unknown" else f"family:{family}"
    asn = entry.get("asn")
    if not asn or asn.get("asn") is None:
        return None
    return f"asn:{asn['asn']}"


def _country(entry: dict) -> str | None:
    geo = entry.get("geolocation")
    if not geo or not geo.get("country"):
        return None
    return geo["country"]


def score_diversity(entry: dict, provider_counts: dict, country_counts: dict) -> int:
    provider = _provider_key(entry)
    country = _country(entry)
    if provider is None or country is None:
        return 30
    provider_points = max(0, 50 - 5 * (provider_counts[provider] - 1))
    country_points = max(0, 30 - 3 * (country_counts[country] - 1))
    return 20 + provider_points + country_points


def score_identity(entry: dict) -> int:
    if entry.get("domain_verified"):
        return 80
    if entry.get("domain"):
        return 55
    return 45


# ---------------------------------------------------------------------------
# Round-level driver
# ---------------------------------------------------------------------------

def score_round(entries: list[dict]) -> dict[str, dict[str, int]]:
    """Score every validator of a round; returns {validator_id: {dim: int}}."""
    newest = max(
        (v for v in (_parse_version(e.get("server_version")) for e in entries) if v),
        default=None,
    )
    provider_counts: dict[str, int] = {}
    country_counts: dict[str, int] = {}
    for entry in entries:
        provider = _provider_key(entry)
        country = _country(entry)
        if provider is not None:
            provider_counts[provider] = provider_counts.get(provider, 0) + 1
        if country is not None:
            country_counts[country] = country_counts.get(country, 0) + 1

    scores: dict[str, dict[str, int]] = {}
    for entry in entries:
        scores[entry["validator_id"]] = {
            "consensus": score_consensus(entry),
            "reliability": score_reliability(entry),
            "software": score_software(entry, newest),
            "diversity": score_diversity(entry, provider_counts, country_counts),
            "identity": score_identity(entry),
        }
    return scores
