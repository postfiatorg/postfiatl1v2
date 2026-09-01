"""Placement-preflight tests against the launch-topology thresholds."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from postfiat_rpc.genesis_registry import evidence_digest, template_trust_graph
from postfiat_rpc.placement_preflight import FAIL, PASS, evaluate, render_table

REPO_ROOT = Path(__file__).resolve().parents[2]
GOLDEN_REGISTRY = REPO_ROOT / "benchmarks/genesis-registry/fixtures/golden/testnet-r19.json"

SOFTWARE = "postfiatd-2.6.0+sha256:5f1e"

# Six provider families x 2 seats, four countries (4/4/2/2), six operators,
# one exact binary: passes every proposed (non-strict) launch threshold.
GOLDEN_12 = [
    {"name": "v01", "provider": "HETZNER-AS, DE", "asn": 24940, "country": "DE", "correlation_group": "op-1", "software_version": SOFTWARE},
    {"name": "v02", "provider": "Hetzner Online GmbH", "asn": 213230, "country": "DE", "correlation_group": "op-1", "software_version": SOFTWARE},
    {"name": "v03", "provider": "OVH SAS, FR", "asn": 16276, "country": "FR", "correlation_group": "op-2", "software_version": SOFTWARE},
    {"name": "v04", "provider": "OVH SAS, US", "asn": 16276, "country": "US", "correlation_group": "op-2", "software_version": SOFTWARE},
    {"name": "v05", "provider": "CONTABO-40021 - Contabo Inc., US", "asn": 40021, "country": "US", "correlation_group": "op-3", "software_version": SOFTWARE},
    {"name": "v06", "provider": "CONTABO, DE", "asn": 51167, "country": "DE", "correlation_group": "op-3", "software_version": SOFTWARE},
    {"name": "v07", "provider": "The Constant Company, LLC", "asn": 20473, "country": "US", "correlation_group": "op-4", "software_version": SOFTWARE},
    {"name": "v08", "provider": "Vultr Holdings LLC", "asn": 20473, "country": "SG", "correlation_group": "op-4", "software_version": SOFTWARE},
    {"name": "v09", "provider": "DIGITALOCEAN-ASN, US", "asn": 14061, "country": "US", "correlation_group": "op-5", "software_version": SOFTWARE},
    {"name": "v10", "provider": "DigitalOcean, LLC", "asn": 14061, "country": "SG", "correlation_group": "op-5", "software_version": SOFTWARE},
    {"name": "v11", "provider": "AMAZON-02, US", "asn": 16509, "country": "DE", "correlation_group": "op-6", "software_version": SOFTWARE},
    {"name": "v12", "provider": "AMAZON-04, DE", "asn": 16509, "country": "FR", "correlation_group": "op-6", "software_version": SOFTWARE},
]

STRICT_COUNTRIES = ("DE", "DE", "FR", "FR", "US", "US", "SG", "SG", "BR", "BR", "JP", "JP")


def verdict(report: dict, dimension_prefix: str) -> dict:
    for dim in report["dimensions"]:
        if dim["dimension"].startswith(dimension_prefix):
            return dim
    raise AssertionError(f"no dimension starting with {dimension_prefix!r}")


class PlacementPreflightTests(unittest.TestCase):
    def test_golden_12_seat_set_passes_every_dimension(self) -> None:
        report = evaluate(copy.deepcopy(GOLDEN_12))
        self.assertEqual(PASS, report["overall"])
        for dim in report["dimensions"]:
            self.assertEqual(PASS, dim["verdict"], dim)
        self.assertEqual(5, len(report["dimensions"]))
        self.assertIn("overall: PASS", render_table(report))

    def test_provider_family_cap_catches_corporate_variants(self) -> None:
        seats = copy.deepcopy(GOLDEN_12)
        seats[2]["provider"] = "HETZNER-CLOUD, FI"  # third hetzner-family seat
        report = evaluate(seats)
        dim = verdict(report, "provider/ASN")
        self.assertEqual(FAIL, dim["verdict"])
        self.assertIn("hetzner=3", dim["detail"])
        self.assertEqual(FAIL, report["overall"])

    def test_asn_cap_is_enforced_independently_of_family(self) -> None:
        seats = copy.deepcopy(GOLDEN_12)
        seats[1]["asn"] = 24940  # second AS24940 seat
        seats[4]["asn"] = 24940  # third AS24940 seat, in another family
        dim = verdict(evaluate(seats), "provider/ASN")
        self.assertEqual(FAIL, dim["verdict"])
        self.assertIn("AS24940=3", dim["detail"])

    def test_country_cap_fails_a_fifth_seat_in_one_country(self) -> None:
        seats = copy.deepcopy(GOLDEN_12)
        seats[3]["country"] = "DE"  # fifth DE seat, still 4 distinct countries
        dim = verdict(evaluate(seats), "geographic")
        self.assertEqual(FAIL, dim["verdict"])
        self.assertIn("DE=5", dim["detail"])

    def test_minimum_country_count_fails_a_three_country_set(self) -> None:
        seats = copy.deepcopy(GOLDEN_12)
        for seat in seats:
            if seat["country"] == "SG":
                seat["country"] = "US"
        dim = verdict(evaluate(seats), "geographic")
        self.assertEqual(FAIL, dim["verdict"])
        self.assertIn("3 distinct countries", dim["detail"])

    def test_correlation_group_cap_fails_a_three_seat_group(self) -> None:
        seats = copy.deepcopy(GOLDEN_12)
        seats[11]["correlation_group"] = "op-1"
        dim = verdict(evaluate(seats), "operator independence")
        self.assertEqual(FAIL, dim["verdict"])
        self.assertIn("op-1=3", dim["detail"])

    def test_software_skew_and_pinned_mismatch_fail(self) -> None:
        seats = copy.deepcopy(GOLDEN_12)
        seats[0]["software_version"] = "postfiatd-2.5.9"
        dim = verdict(evaluate(seats), "software")
        self.assertEqual(FAIL, dim["verdict"])
        self.assertIn("version skew", dim["detail"])

        pinned = verdict(evaluate(copy.deepcopy(GOLDEN_12), pinned="other"), "software")
        self.assertEqual(FAIL, pinned["verdict"])
        self.assertIn("pinned", pinned["detail"])
        self.assertEqual(
            PASS, verdict(evaluate(copy.deepcopy(GOLDEN_12), pinned=SOFTWARE), "software")["verdict"]
        )

    def test_eleven_seats_fail_the_minimum_count(self) -> None:
        dim = verdict(evaluate(copy.deepcopy(GOLDEN_12)[:11]), "minimum count")
        self.assertEqual(FAIL, dim["verdict"])
        self.assertIn("11 seats", dim["detail"])

    def test_strict_variant_rejects_the_four_country_profile(self) -> None:
        report = evaluate(copy.deepcopy(GOLDEN_12), strict=True)
        dim = verdict(report, "geographic")
        self.assertEqual(FAIL, dim["verdict"])
        self.assertTrue(report["strict_geographic_variant"])

        six_countries = copy.deepcopy(GOLDEN_12)
        for seat, country in zip(six_countries, STRICT_COUNTRIES):
            seat["country"] = country
        self.assertEqual(PASS, evaluate(six_countries, strict=True)["overall"])

    def test_missing_fields_fail_closed_loudly(self) -> None:
        seats = copy.deepcopy(GOLDEN_12)
        del seats[6]["country"]
        seats[7]["correlation_group"] = "  "
        report = evaluate(seats)
        self.assertEqual(FAIL, report["overall"])
        geo = verdict(report, "geographic")
        self.assertEqual(FAIL, geo["verdict"])
        self.assertIn("fail-closed", geo["detail"])
        self.assertIn("v07", geo["detail"])
        corr = verdict(report, "operator independence")
        self.assertEqual(FAIL, corr["verdict"])
        self.assertIn("fail-closed", corr["detail"])
        self.assertIn("v08", corr["detail"])

    def test_canonical_registry_without_evidence_fails_closed(self) -> None:
        payload = json.loads(GOLDEN_REGISTRY.read_text())
        report = evaluate(payload)
        self.assertEqual(FAIL, report["overall"])
        self.assertEqual(18, report["seats"])
        self.assertEqual(PASS, verdict(report, "minimum count")["verdict"])
        for prefix in ("provider/ASN", "geographic", "operator independence", "software"):
            dim = verdict(report, prefix)
            self.assertEqual(FAIL, dim["verdict"], dim)
            self.assertIn("--evidence", dim["detail"])

    def test_canonical_registry_with_digest_matched_evidence_passes(self) -> None:
        records = []
        for index, seat in enumerate(GOLDEN_12):
            record = {
                "fork_master_key_hex": "ed" + f"{index + 1:064x}",
                "domain": f"{seat['name']}.example",
                "domain_verified": 1,
                "provider": seat["provider"],
                "country": seat["country"],
            }
            records.append(
                dict(
                    record,
                    correlation_group=seat["correlation_group"],
                    software_version=seat["software_version"],
                    asn=seat["asn"],
                )
            )
        registry = {
            "entries": [
                {
                    "fork_master_key_hex": record["fork_master_key_hex"],
                    "identity_evidence_digest_hex": evidence_digest(record).hex(),
                }
                for record in records
            ],
            "template_trust_graph": template_trust_graph(12),
        }
        self.assertEqual(PASS, evaluate(registry, evidence=records)["overall"])

        tampered = copy.deepcopy(records)
        tampered[0]["country"] = "BR"  # declared/observed mismatch -> unresolved
        report = evaluate(registry, evidence=tampered)
        self.assertEqual(FAIL, report["overall"])
        self.assertIn("unresolved record", verdict(report, "geographic")["detail"])
        self.assertEqual(PASS, verdict(report, "minimum count")["verdict"])


if __name__ == "__main__":
    unittest.main()
