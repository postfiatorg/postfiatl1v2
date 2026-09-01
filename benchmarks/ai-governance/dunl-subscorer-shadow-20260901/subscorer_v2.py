"""Deterministic sub-scorers v2 for the Dynamic UNL shadow evaluation.

Versioned FORMULA-style: ``SUBSCORER_VERSION = 2``. Version 1 lives
unchanged in ``subscorer.py`` and stays reproducible; this module is
self-contained (its own copies of every rule, including the ones v1
already got right) so that v2's complete semantics are one file. The
shared frozen-input extraction stays in ``subscorer.py`` and is not a
scoring rule.

v2 addresses the three divergence classes documented in
``docs/governance/dunl-subscorer-shadow-eval-20260901.md`` (v1 findings):

1. **Era-aware consensus.** The consensus rule now follows the round's own
   prompt-version era instead of applying one rule to every round.
   Prompt v8+ introduced the worst-window ceiling in the published rubric
   ("the consensus sub-score must never exceed the worst of the three
   agreement windows"); the v5/v6 prompts contain no such ceiling — their
   text reads the whole record ("look at all three time windows",
   "a validator with perfect 1h but poor 30d may have recently recovered
   from an outage") and the era's model scored the long-term record.
   Scoring old rounds with the new era's rule measured the era gap, not
   sub-scorer quality (the round-12 "flip" was exactly this).
2. **Rubric-text reliability banding.** v1 classified ``min(1h, 24h) <
   0.5`` as "currently broken" (10), which put a validator with a clean 1h
   window over a chronically degraded 24h window below the published
   rubric's own band. The rubric locates the failure class by *which
   window carries the losses*: 1h losses mean the problem is happening
   now (broken, below 40); 24h losses with a clean 1h mean recent or
   chronic instability (40-70); 30d-only losses mean an old resolved
   incident (75-90); no meaningful losses mean fully stable (95-100).
   v2 implements that classification literally (the round-16 divergence
   class).
3. **Rubric-banded diversity.** v1's linear count penalties ordered
   concentration like the model but curved differently (mean delta 15-23
   in every round). v2 replaces the linear curve with a band matrix shaped
   like the model's observed banding over the same concentration counts
   (unique provider scores high regardless of country, dominant-provider
   members floor at 40, country rarity lifts the middle), emitting
   multiples of 5 per the v8+ scoring rules. The matrix is monotone in
   both axes, which the published rubric requires of the model itself.

CONSENSUS (era-aware):
    era >= 8 (prompt v8+): floor(100 * min(1h, 24h, 30d)) — the rubric's
        worst-window ceiling at full resolution, identical to v1.
    era < 8 (prompt v5/v6): 10 if 1h < 0.5, else floor(100 * 30d).
        The era's whole-record reading with its own written penalty
        policy. The v5/v6 prompt has no worst-window ceiling — its text
        reads the long-term record and treats degraded recent windows as
        recovery context — but it does contain the same "agreement scores
        near zero ... penalize the consensus sub-score heavily" policy as
        later prompts, and the era's model in fact pinned currently
        offline validators at consensus ~10 (seven of the eight offline
        cases in rounds 12-15; the eighth, scored 80, is the round-12
        anomaly that violated the era's own penalty policy and motivated
        the score formula — v2 deliberately does not reproduce it). For
        participating validators the 30-day and worst-window readings are
        empirically near-identical in rounds 12-15 (mean |delta| to the
        model within ~0.1 of each other); the 30-day reading is chosen
        because it is the era's documented semantics. A pure 30-day
        reading without the offline penalty was evaluated and rejected:
        it manufactured seven ineligible->eligible flips for validators
        that even the era's model had scored ineligible.
    A null/missing window counts as 0.0 in both eras. Exact decimal floor
    via Fraction (no float artifacts). Monotone within each era.

RELIABILITY (window-located rubric bands, all eras; null window -> 0.0):
    c1 < 0.5                      -> 10   currently offline/broken (<40)
    else if c1 < 0.999 or c24 < 0.999 -> recent/chronic instability
        m = min(c1, c24):  m >= 0.99 -> 65;  m >= 0.95 -> 55;
        m >= 0.9 -> 50;  else -> 40   (band floor: chronic deep 24h
        degradation with a working 1h lands at 40, not below the band)
    else (recent windows clean)   -> judge the 30d residue:
        c30 >= 0.9995 -> 100;  c30 >= 0.999 -> 95   (fully stable)
        c30 >= 0.995 -> 90;  c30 >= 0.99 -> 85;  c30 >= 0.95 -> 80;
        else -> 75                                  (old resolved incident)
    The only behavioral change from v1 is the classification boundary:
    "broken" now requires the 1h window itself to be dead. The in-band
    ladders are v1's, which were already rubric-shaped.

SOFTWARE (unchanged from v1, restated):
    version ordering against the round's own set — latest -> 100; same
    major.minor -> 80; same major -> 60; older major -> 40;
    missing/unparsable -> 50 (documented neutral).

DIVERSITY (rubric band matrix over the same concentration counts):
    provider key = ``provider_family`` where the round supplies it
    (rounds 17+); raw ASN otherwise (v1's documented limitation stands:
    no family-grouping evidence exists for rounds 12-16).
    unresolved endpoint -> 30 flat (missing-endpoint policy, as v1).
    resolved: with n_p validators sharing the provider key and n_c the
    country, band each axis - provider: unique (1), small (2),
    medium (3-9), dominant (>= 10); country: rare (<= 2), moderate (3-9),
    common (>= 10) - and read the matrix:

                      rare   moderate   common
        unique         95       95        90
        small          85       80        75
        medium         80       65        55
        dominant       60       55        40

    Non-increasing along both axes; equal count-pairs score identically;
    all values multiples of 5.

IDENTITY (unchanged from v1, restated):
    domain_verified -> 80; domain unverified -> 55; no domain -> 45.

Explicitly read but neutral, with revisit triggers (the v1 note's open
requirements, pinned here so silence is not ambiguity):

- ``incomplete`` window flags (present only in round 19's frozen
  evidence, four 30d windows): scored at face value. In the only round
  carrying the flag the model itself scored those windows at face value
  (consensus 99 = the worst-window floor), so a discount rule would be
  invented signal. Revisit when a round shows the flag on a recent
  window or with a materially short record.
- fee votes: ``base_fee`` is uniformly 10 across every validator of all
  eight frozen rounds; there is no fee-vote signal to score and v2
  defines none. Revisit when a frozen round carries a non-uniform fee
  vote set.
- formal identity: the ``identity`` field is null for every validator in
  all eight frozen rounds; v2 scores accountability from domain evidence
  only. Revisit when the network deploys identity verification.

Integer 0-100 outputs, deterministic, no network, no model.
"""

from __future__ import annotations

import re
from fractions import Fraction

SUBSCORER_VERSION = 2

DIMENSIONS = ("consensus", "reliability", "software", "diversity", "identity")

# The worst-window consensus ceiling entered the published rubric at
# prompt v8, the same version that made the model's overall score advisory.
WORST_WINDOW_ERA_FIRST_PROMPT = 8

_VERSION_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
_PROMPT_VERSION_RE = re.compile(r"^v(\d+)$")

_CLEAN = 0.999

# Diversity band matrix: rows = provider band, columns = country band.
_DIVERSITY_BANDS = {
    "unique": {"rare": 95, "moderate": 95, "common": 90},
    "small": {"rare": 85, "moderate": 80, "common": 75},
    "medium": {"rare": 80, "moderate": 65, "common": 55},
    "dominant": {"rare": 60, "moderate": 55, "common": 40},
}
_UNRESOLVED_DIVERSITY = 30


def prompt_era(prompt_version: str) -> int:
    """Parse a round's prompt version (``"v5"`` .. ``"v10"``) to its era."""
    match = _PROMPT_VERSION_RE.match(prompt_version.strip())
    if match is None:
        raise ValueError(f"unrecognized prompt version {prompt_version!r}")
    return int(match.group(1))


def _window_score(entry: dict, window: str) -> float:
    data = entry.get(f"agreement_{window}")
    if not data or data.get("score") is None:
        return 0.0
    return float(data["score"])


def _floor_100(score: float) -> int:
    # Exact decimal floor via the round-trip string repr, as in v1: the
    # frozen scores are short decimals, and 0.83 * 100 must floor to 83.
    return int(Fraction(str(score)) * 100)


def score_consensus(entry: dict, era: int) -> int:
    if era >= WORST_WINDOW_ERA_FIRST_PROMPT:
        return _floor_100(min(_window_score(entry, w) for w in ("1h", "24h", "30d")))
    if _window_score(entry, "1h") < 0.5:
        return 10
    return _floor_100(_window_score(entry, "30d"))


def score_reliability(entry: dict) -> int:
    c1 = _window_score(entry, "1h")
    c24 = _window_score(entry, "24h")
    c30 = _window_score(entry, "30d")
    if c1 < 0.5:
        return 10
    if c1 < _CLEAN or c24 < _CLEAN:
        m = min(c1, c24)
        for threshold, score in ((0.99, 65), (0.95, 55), (0.9, 50)):
            if m >= threshold:
                return score
        return 40
    for threshold, score in (
        (0.9995, 100), (0.999, 95), (0.995, 90), (0.99, 85), (0.95, 80),
    ):
        if c30 >= threshold:
            return score
    return 75


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


def _provider_band(n_p: int) -> str:
    if n_p <= 1:
        return "unique"
    if n_p == 2:
        return "small"
    if n_p <= 9:
        return "medium"
    return "dominant"


def _country_band(n_c: int) -> str:
    if n_c <= 2:
        return "rare"
    if n_c <= 9:
        return "moderate"
    return "common"


def score_diversity(entry: dict, provider_counts: dict, country_counts: dict) -> int:
    provider = _provider_key(entry)
    country = _country(entry)
    if provider is None or country is None:
        return _UNRESOLVED_DIVERSITY
    return _DIVERSITY_BANDS[_provider_band(provider_counts[provider])][
        _country_band(country_counts[country])
    ]


def score_identity(entry: dict) -> int:
    if entry.get("domain_verified"):
        return 80
    if entry.get("domain"):
        return 55
    return 45


def score_round(entries: list[dict], prompt_version: str) -> dict[str, dict[str, int]]:
    """Score every validator of a round under the round's own prompt era.

    Returns ``{validator_id: {dimension: int}}``. ``prompt_version`` is the
    round's frozen ``code.prompt.version`` (for example ``"v5"``); it
    selects the consensus era and nothing else.
    """
    era = prompt_era(prompt_version)
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
            "consensus": score_consensus(entry, era),
            "reliability": score_reliability(entry),
            "software": score_software(entry, newest),
            "diversity": score_diversity(entry, provider_counts, country_counts),
            "identity": score_identity(entry),
        }
    return scores
