"""Unit tests for the deterministic sub-scorers (v1 and v2).

``unittest.TestCase`` classes so the repository-standard
``python3 -m unittest`` collects them (run from this directory); also
runnable directly: ``python3 test_subscorer.py``. No fork imports, no
network: only ``subscorer`` (v1) and ``subscorer_v2``.

The shared fixture is a small synthetic round shaped like the frozen
testnet entries (rounds 17+ shape with ``provider_family``; one
legacy-shape entry without it exercises the ASN-keyed provider fallback).
The v1 assertions preserve the originally recorded v1 semantics unchanged.
"""

from __future__ import annotations

import unittest

import subscorer
import subscorer_v2


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


class TestSubscorerV1(unittest.TestCase):
    """The originally recorded v1 semantics, unchanged."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.scores = subscorer.score_round(FIXTURE)

    def test_consensus_worst_window_ceiling(self) -> None:
        s = self.scores
        self.assertEqual(s["v001"]["consensus"], 100)
        self.assertEqual(s["v002"]["consensus"], 99)  # floor(99.336)
        self.assertEqual(s["v003"]["consensus"], 0)  # dead 1h window collapses
        self.assertEqual(s["v004"]["consensus"], 98)  # floor(98.21), worst is 24h
        self.assertEqual(s["v007"]["consensus"], 83)  # exact decimal floor

    def test_reliability_bands(self) -> None:
        s = self.scores
        self.assertEqual(s["v001"]["reliability"], 100)  # fully stable
        self.assertEqual(s["v002"]["reliability"], 85)  # old resolved incident
        self.assertEqual(s["v003"]["reliability"], 10)  # currently broken
        self.assertEqual(s["v004"]["reliability"], 55)  # recent instability

    def test_software_version_ordering(self) -> None:
        s = self.scores
        self.assertEqual(s["v001"]["software"], 100)  # newest in set
        self.assertEqual(s["v004"]["software"], 80)  # patch behind
        missing = subscorer.score_software({"server_version": None}, (1, 0, 4))
        self.assertEqual(missing, 50)  # documented neutral

    def test_diversity_counts_and_missing_endpoint(self) -> None:
        s = self.scores
        # v001: unique family (n_p=1) and unique country (n_c=1) -> 20+50+30.
        self.assertEqual(s["v001"]["diversity"], 100)
        # v002: hetzner family n_p=4 (v002-v004, v007; v006 is ASN-keyed),
        # Germany n_c=5 -> 20 + (50-15) + (30-12) = 73.
        self.assertEqual(s["v002"]["diversity"], 73)
        self.assertEqual(s["v005"]["diversity"], 30)  # missing-endpoint policy
        # v006 (legacy ASN key, n_p=1; Germany n_c=5) -> 20 + 50 + 18 = 88.
        self.assertEqual(s["v006"]["diversity"], 88)

    def test_identity_domain_bands(self) -> None:
        s = self.scores
        self.assertEqual(s["v001"]["identity"], 80)  # verified domain
        self.assertEqual(s["v004"]["identity"], 55)  # unverified domain
        self.assertEqual(s["v005"]["identity"], 45)  # no domain

    def test_outputs_are_bounded_integers(self) -> None:
        for dims in self.scores.values():
            self.assertEqual(set(dims), set(subscorer.DIMENSIONS))
            for value in dims.values():
                self.assertIsInstance(value, int)
                self.assertTrue(0 <= value <= 100)

    def test_determinism(self) -> None:
        self.assertEqual(subscorer.score_round(FIXTURE), subscorer.score_round(FIXTURE))


class TestSubscorerV2EraAwareConsensus(unittest.TestCase):
    def test_version_constant(self) -> None:
        self.assertEqual(subscorer_v2.SUBSCORER_VERSION, 2)

    def test_prompt_era_parsing(self) -> None:
        self.assertEqual(subscorer_v2.prompt_era("v5"), 5)
        self.assertEqual(subscorer_v2.prompt_era("v10"), 10)
        with self.assertRaises(ValueError):
            subscorer_v2.prompt_era("prompt-5")

    def test_new_era_keeps_the_worst_window_ceiling(self) -> None:
        entry = _entry("x", s1h=0.6, s24h=0.7, s30d=0.99)
        for era in (8, 9, 10):
            self.assertEqual(subscorer_v2.score_consensus(entry, era), 60)

    def test_old_era_reads_the_30_day_record_when_participating(self) -> None:
        entry = _entry("x", s1h=0.6, s24h=0.7, s30d=0.99)
        for era in (5, 6):
            self.assertEqual(subscorer_v2.score_consensus(entry, era), 99)

    def test_old_era_still_penalizes_current_non_participation(self) -> None:
        # The v5/v6 penalty policy: near-zero agreement is penalized
        # heavily; the era's model pinned offline validators at ~10.
        offline = _entry("x", s1h=0.0, s24h=0.09976, s30d=0.97037)
        self.assertEqual(subscorer_v2.score_consensus(offline, 5), 10)
        self.assertEqual(subscorer_v2.score_consensus(offline, 9), 0)

    def test_exact_decimal_floor_in_both_eras(self) -> None:
        entry = _entry("x", s30d=0.83)
        self.assertEqual(subscorer_v2.score_consensus(entry, 5), 83)
        self.assertEqual(subscorer_v2.score_consensus(entry, 9), 83)

    def test_null_window_counts_as_zero(self) -> None:
        entry = _entry("x")
        entry["agreement_30d"] = None
        self.assertEqual(subscorer_v2.score_consensus(entry, 9), 0)


class TestSubscorerV2Reliability(unittest.TestCase):
    def test_offline_now_is_broken(self) -> None:
        entry = _entry("x", s1h=0.0, s24h=0.09976, s30d=0.97037)
        self.assertEqual(subscorer_v2.score_reliability(entry), 10)

    def test_chronic_24h_degradation_with_clean_1h_is_the_band_floor(self) -> None:
        # The round-16 divergence class: clean 1h over a deeply degraded
        # 24h window is the rubric's recent/chronic band (40-70), not
        # "currently broken". v1 scored this 10; v2 scores the band floor.
        entry = _entry("x", s1h=1.0, s24h=0.165, s30d=0.773)
        self.assertEqual(subscorer.score_reliability(entry), 10)  # v1, recorded
        self.assertEqual(subscorer_v2.score_reliability(entry), 40)  # v2, in band

    def test_recent_instability_ladder_unchanged_from_v1(self) -> None:
        for s24h, expected in ((0.9821, 55), (0.995, 65), (0.93, 50), (0.7, 40)):
            entry = _entry("x", s24h=s24h)
            self.assertEqual(subscorer_v2.score_reliability(entry), expected)
            self.assertEqual(subscorer.score_reliability(entry), expected)

    def test_clean_ladders_unchanged_from_v1(self) -> None:
        for s30d, expected in ((1.0, 100), (0.9992, 95), (0.99336, 85), (0.9, 75)):
            entry = _entry("x", s30d=s30d)
            self.assertEqual(subscorer_v2.score_reliability(entry), expected)
            self.assertEqual(subscorer.score_reliability(entry), expected)

    def test_all_reliability_outputs_sit_in_rubric_bands(self) -> None:
        # broken < 40 <= chronic <= 70 < resolved 75-90 < stable 95-100.
        grid = [x / 1000 for x in range(0, 1001, 53)] + [0.999, 0.9995, 1.0]
        for s1h in grid:
            for s24h in grid:
                entry = _entry("x", s1h=s1h, s24h=s24h, s30d=0.999)
                value = subscorer_v2.score_reliability(entry)
                if s1h < 0.5:
                    self.assertLess(value, 40)
                elif s1h < 0.999 or s24h < 0.999:
                    self.assertTrue(40 <= value <= 70)
                else:
                    self.assertTrue(value >= 75)


class TestSubscorerV2Diversity(unittest.TestCase):
    def _score(self, n_p: int, n_c: int) -> int:
        entry = _entry("x")
        return subscorer_v2.score_diversity(
            entry, {"family:hetzner": n_p}, {"Germany": n_c}
        )

    def test_band_matrix_corners(self) -> None:
        self.assertEqual(self._score(1, 1), 95)  # unique/rare
        self.assertEqual(self._score(1, 17), 90)  # unique in a crowded country
        self.assertEqual(self._score(2, 17), 75)  # small/common
        self.assertEqual(self._score(8, 17), 55)  # medium/common
        self.assertEqual(self._score(8, 2), 80)  # medium/rare
        self.assertEqual(self._score(22, 17), 40)  # dominant floor
        self.assertEqual(self._score(22, 5), 55)  # dominant, rarer country

    def test_matrix_is_monotone_in_both_axes(self) -> None:
        counts = (1, 2, 3, 9, 10, 22)
        for i, n_p in enumerate(counts):
            for j, n_c in enumerate(counts):
                if i + 1 < len(counts):
                    self.assertGreaterEqual(
                        self._score(n_p, n_c), self._score(counts[i + 1], n_c)
                    )
                if j + 1 < len(counts):
                    self.assertGreaterEqual(
                        self._score(n_p, n_c), self._score(n_p, counts[j + 1])
                    )

    def test_all_bands_are_multiples_of_five(self) -> None:
        for row in subscorer_v2._DIVERSITY_BANDS.values():
            for value in row.values():
                self.assertEqual(value % 5, 0)

    def test_missing_endpoint_policy(self) -> None:
        entry = _entry("x", domain=None, asn=None, country=None)
        self.assertEqual(subscorer_v2.score_diversity(entry, {}, {}), 30)

    def test_legacy_asn_key_fallback(self) -> None:
        entry = _entry("x", legacy=True)
        self.assertEqual(subscorer_v2._provider_key(entry), "asn:24940")


class TestSubscorerV2Round(unittest.TestCase):
    def test_round_scoring_new_era(self) -> None:
        s = subscorer_v2.score_round(FIXTURE, "v9")
        self.assertEqual(s["v003"]["consensus"], 0)  # worst-window era
        self.assertEqual(s["v003"]["reliability"], 10)
        # v001: unique family, unique country -> unique/rare band.
        self.assertEqual(s["v001"]["diversity"], 95)
        # v002: hetzner n_p=4 (medium), Germany n_c=5 (moderate) -> 65.
        self.assertEqual(s["v002"]["diversity"], 65)
        self.assertEqual(s["v005"]["diversity"], 30)
        # v006: legacy ASN key n_p=1 (unique), Germany n_c=5 (moderate) -> 95.
        self.assertEqual(s["v006"]["diversity"], 95)
        # Software and identity rules are unchanged from v1.
        v1 = subscorer.score_round(FIXTURE)
        for vid in s:
            self.assertEqual(s[vid]["software"], v1[vid]["software"])
            self.assertEqual(s[vid]["identity"], v1[vid]["identity"])

    def test_round_scoring_old_era_differs_only_in_consensus(self) -> None:
        old = subscorer_v2.score_round(FIXTURE, "v5")
        new = subscorer_v2.score_round(FIXTURE, "v9")
        self.assertEqual(old["v003"]["consensus"], 10)  # offline penalty, old era
        self.assertEqual(new["v003"]["consensus"], 0)
        self.assertEqual(old["v004"]["consensus"], 98)  # floor(98.959), 30d read
        self.assertEqual(new["v004"]["consensus"], 98)  # floor(98.21), worst 24h
        for vid in old:
            for dim in ("reliability", "software", "diversity", "identity"):
                self.assertEqual(old[vid][dim], new[vid][dim])

    def test_unknown_prompt_version_fails_closed(self) -> None:
        with self.assertRaises(ValueError):
            subscorer_v2.score_round(FIXTURE, "unversioned")

    def test_outputs_are_bounded_integers(self) -> None:
        for dims in subscorer_v2.score_round(FIXTURE, "v10").values():
            self.assertEqual(set(dims), set(subscorer_v2.DIMENSIONS))
            for value in dims.values():
                self.assertIsInstance(value, int)
                self.assertTrue(0 <= value <= 100)

    def test_determinism(self) -> None:
        self.assertEqual(
            subscorer_v2.score_round(FIXTURE, "v5"),
            subscorer_v2.score_round(FIXTURE, "v5"),
        )


if __name__ == "__main__":
    unittest.main()
