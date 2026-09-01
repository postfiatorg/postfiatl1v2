"""Unit tests for the deterministic sub-scorers v1.

Runs under pytest or directly: ``.venv/bin/python test_subscorer.py``.
The fixture is a small synthetic round shaped like the frozen testnet
entries (rounds 17+ shape with ``provider_family``; one legacy-shape entry
without it exercises the ASN-keyed provider fallback).
"""

from __future__ import annotations

import subscorer


def _entry(
    vid: str,
    s1h: float = 1.0,
    s24h: float = 1.0,
    s30d: float = 1.0,
    version: str = "1.0.4",
    domain: str | None = "example.com",
    verified: bool = True,
    asn: int | None = 24940,
    country: str | None = "Germany",
    family: str | None = "hetzner",
    legacy: bool = False,
) -> dict:
    entry = {
        "validator_id": vid,
        "domain": domain,
        "domain_verified": verified if domain else None,
        "agreement_1h": {"score": s1h, "total": 1192, "missed": 0},
        "agreement_24h": {"score": s24h, "total": 28610, "missed": 0},
        "agreement_30d": {"score": s30d, "total": 853615, "missed": 0},
        "server_version": version,
        "base_fee": 10,
        "asn": {"asn": asn, "as_name": "X"} if asn is not None else None,
        "geolocation": {"country": country} if country else None,
        "identity": None,
    }
    if not legacy:
        entry["provider_family"] = family if asn is not None else "unknown"
    return entry


FIXTURE = [
    # Perfect validator, unique provider+country.
    _entry("v001", s30d=1.0, asn=852, country="Canada", family="telus"),
    # Old resolved 30d incident, megabloc member (hetzner x3, Germany x3).
    _entry("v002", s30d=0.99336),
    # Currently offline (1h dead), long history clean.
    _entry("v003", s1h=0.0, s24h=0.09976, s30d=0.97037),
    # Recent instability, outdated patch version, unverified domain.
    _entry("v004", s24h=0.9821, s30d=0.98959, version="1.0.0", verified=False),
    # Unresolved endpoint, no domain.
    _entry("v005", domain=None, asn=None, country=None),
    # Legacy shape (rounds 12-16): no provider_family, ASN is the key.
    _entry("v006", legacy=True),
    # Exact float-floor regression case: 0.83 must floor to 83, not 82.
    _entry("v007", s30d=0.83),
]


def scores() -> dict[str, dict[str, int]]:
    return subscorer.score_round(FIXTURE)


def test_consensus_worst_window_ceiling() -> None:
    s = scores()
    assert s["v001"]["consensus"] == 100
    assert s["v002"]["consensus"] == 99  # floor(99.336)
    assert s["v003"]["consensus"] == 0  # dead 1h window collapses the score
    assert s["v004"]["consensus"] == 98  # floor(98.21), worst window is 24h
    assert s["v007"]["consensus"] == 83  # exact decimal floor, no float slip


def test_reliability_bands() -> None:
    s = scores()
    assert s["v001"]["reliability"] == 100  # fully stable
    assert s["v002"]["reliability"] == 85  # old resolved incident
    assert s["v003"]["reliability"] == 10  # currently broken
    assert s["v004"]["reliability"] == 55  # recent instability (24h 0.9821)


def test_software_version_ordering() -> None:
    s = scores()
    assert s["v001"]["software"] == 100  # newest in set
    assert s["v004"]["software"] == 80  # patch behind (1.0.0 vs 1.0.4)
    missing = subscorer.score_software({"server_version": None}, (1, 0, 4))
    assert missing == 50  # documented neutral


def test_diversity_counts_and_missing_endpoint() -> None:
    s = scores()
    # v001: unique family (n_p=1) and unique country (n_c=1) -> 20+50+30.
    assert s["v001"]["diversity"] == 100
    # v002: hetzner family n_p=4 (v002-v004, v007; v006 is ASN-keyed),
    # Germany n_c=5 -> 20 + (50-15) + (30-12) = 73.
    assert s["v002"]["diversity"] == 73
    assert s["v005"]["diversity"] == 30  # missing-endpoint policy
    # v006 (legacy ASN key, n_p=1; Germany n_c=5) -> 20 + 50 + 18 = 88.
    assert s["v006"]["diversity"] == 88


def test_identity_domain_bands() -> None:
    s = scores()
    assert s["v001"]["identity"] == 80  # verified domain
    assert s["v004"]["identity"] == 55  # unverified domain
    assert s["v005"]["identity"] == 45  # no domain


def test_outputs_are_bounded_integers() -> None:
    for dims in scores().values():
        assert set(dims) == set(subscorer.DIMENSIONS)
        for value in dims.values():
            assert isinstance(value, int)
            assert 0 <= value <= 100


def test_determinism() -> None:
    assert scores() == scores()


if __name__ == "__main__":
    failures = 0
    for name, test in sorted(globals().items()):
        if name.startswith("test_") and callable(test):
            try:
                test()
                print(f"PASS {name}")
            except AssertionError as exc:
                failures += 1
                print(f"FAIL {name}: {exc}")
    raise SystemExit(1 if failures else 0)
