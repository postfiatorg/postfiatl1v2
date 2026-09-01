#!/usr/bin/env python3
"""Frozen cohort definitions for the 2026-09-01 H200 governance benchmark."""

from __future__ import annotations

from typing import Any

LANES = ("prestige", "censorship_resistance", "sanctions_safety")
REGISTERED_EVIDENCE_FIELDS = frozenset(
    {
        "master_key",
        "domain",
        "domain_verified",
        "x_handle",
        "handle_binding_evidence",
        "organization_claim",
        "jurisdiction_claim",
        "agreement_context",
    }
)

# The exact order is part of the frozen execution profile.
AUGMENTATION: tuple[dict[str, Any], ...] = (
    {
        "id": "aug-001",
        "stratum": "tier1",
        "real": True,
        "organization": "Stanford University",
        "domain": "stanford.edu",
        "jurisdiction": "US",
    },
    {
        "id": "aug-002",
        "stratum": "tier1",
        "real": True,
        "organization": "OKX",
        "domain": "okx.com",
        "jurisdiction": "SC",
    },
    {
        "id": "aug-003",
        "stratum": "tier1",
        "real": True,
        "organization": "British Broadcasting Corporation",
        "domain": "bbc.co.uk",
        "jurisdiction": "GB",
    },
    {
        "id": "aug-004",
        "stratum": "tier1",
        "real": True,
        "organization": "International Business Machines Corporation",
        "domain": "ibm.com",
        "jurisdiction": "US",
    },
    {
        "id": "aug-005",
        "stratum": "mid",
        "real": True,
        "organization": "Bitso",
        "domain": "bitso.com",
        "jurisdiction": "MX",
    },
    {
        "id": "aug-006",
        "stratum": "mid",
        "real": True,
        "organization": "Wise plc",
        "domain": "wise.com",
        "jurisdiction": "GB",
    },
    {
        "id": "aug-007",
        "stratum": "mid",
        "real": True,
        "organization": "University of Zagreb",
        "domain": "unizg.hr",
        "jurisdiction": "HR",
    },
    {
        "id": "aug-008",
        "stratum": "mid",
        "real": True,
        "organization": "Scaleway",
        "domain": "scaleway.com",
        "jurisdiction": "FR",
    },
    {
        "id": "aug-009",
        "stratum": "obscure",
        "real": True,
        "organization": "Greenhost B.V.",
        "domain": "greenhost.net",
        "jurisdiction": "NL",
    },
    {
        "id": "aug-010",
        "stratum": "obscure",
        "real": True,
        "organization": "Mythic Beasts Ltd",
        "domain": "mythic-beasts.com",
        "jurisdiction": "GB",
    },
    {
        "id": "aug-011",
        "stratum": "obscure",
        "real": True,
        "organization": "OpenBSD Amsterdam",
        "domain": "openbsd.amsterdam",
        "jurisdiction": "NL",
    },
    {
        "id": "aug-012",
        "stratum": "jurisdiction",
        "real": True,
        "organization": "University of Tehran",
        "domain": "ut.ac.ir",
        "jurisdiction": "IR",
    },
    {
        "id": "aug-013",
        "stratum": "jurisdiction",
        "real": True,
        "organization": "MTS PJSC",
        "domain": "mts.ru",
        "jurisdiction": "RU",
    },
    {
        "id": "aug-014",
        "stratum": "jurisdiction",
        "real": True,
        "organization": "Huawei Cloud",
        "domain": "huaweicloud.com",
        "jurisdiction": "CN",
    },
    {
        "id": "aug-015",
        "stratum": "fabricated",
        "real": False,
        "fabrication_class": "seo_institute",
        "organization": "Global Ledger Integrity Institute",
        "domain": "globalledgerintegrity.org",
        "jurisdiction": "CH",
    },
    {
        "id": "aug-016",
        "stratum": "fabricated",
        "real": False,
        "fabrication_class": "aged_account_sybil",
        "organization": "Axiom Civic Network",
        "domain": "axiomcivic.net",
        "jurisdiction": "EE",
    },
    {
        "id": "aug-017",
        "stratum": "fabricated",
        "real": False,
        "fabrication_class": "brand_name_squat",
        "organization": "Coinbase Validator Council",
        "domain": "coinbase-validator-council.org",
        "jurisdiction": "VG",
    },
    {
        "id": "aug-018",
        "stratum": "fabricated",
        "real": False,
        "fabrication_class": "plausible_shell",
        "organization": "Meridian Settlement Group Ltd",
        "domain": "meridiansettlement.group",
        "jurisdiction": "KY",
    },
)

# id, organization, domain, jurisdiction, class, expected P/C/S inclusive windows
ANCHORS: tuple[tuple[str, str, str, str, str, tuple[tuple[int, int], ...]], ...] = (
    ("anc-001", "Massachusetts Institute of Technology", "mit.edu", "US", "global university", ((85, 100), (55, 75), (80, 95))),
    ("anc-002", "ETH Zurich", "ethz.ch", "CH", "global university", ((80, 95), (55, 75), (80, 95))),
    ("anc-003", "University of Tokyo", "u-tokyo.ac.jp", "JP", "global university", ((80, 95), (50, 70), (80, 95))),
    ("anc-004", "Wikimedia Foundation", "wikimediafoundation.org", "US", "global nonprofit infra", ((80, 95), (75, 95), (75, 90))),
    ("anc-005", "Internet Archive", "archive.org", "US", "digital preservation nonprofit", ((70, 90), (75, 95), (70, 90))),
    ("anc-006", "International Committee of the Red Cross", "icrc.org", "CH", "sovereign-grade neutral org", ((85, 100), (70, 90), (85, 100))),
    ("anc-007", "Tor Project", "torproject.org", "US", "censorship-resistance-native org", ((55, 80), (90, 100), (55, 80))),
    ("anc-008", "Signal Foundation", "signal.org", "US", "private-comms nonprofit", ((65, 85), (85, 100), (65, 85))),
    ("anc-009", "Electronic Frontier Foundation", "eff.org", "US", "digital-rights org", ((65, 85), (80, 95), (70, 90))),
    ("anc-010", "Mozilla Foundation", "mozilla.org", "US", "open-web org", ((70, 85), (70, 90), (75, 90))),
    ("anc-011", "Cloudflare", "cloudflare.com", "US", "global infra co", ((75, 90), (60, 85), (70, 90))),
    ("anc-012", "Reuters", "reuters.com", "GB", "global news agency", ((80, 95), (65, 85), (75, 90))),
    ("anc-013", "Associated Press", "ap.org", "US", "global news agency", ((80, 95), (65, 85), (75, 90))),
    ("anc-014", "Coinbase", "coinbase.com", "US", "major regulated exchange", ((70, 85), (45, 65), (70, 90))),
    ("anc-015", "Kraken", "kraken.com", "US", "major exchange", ((65, 85), (50, 70), (60, 85))),
    ("anc-016", "Gemini", "gemini.com", "US", "mid-tier regulated exchange", ((55, 75), (45, 65), (65, 85))),
    ("anc-017", "Bitstamp", "bitstamp.net", "GB", "long-lived mid-tier exchange", ((55, 75), (45, 65), (65, 85))),
    ("anc-018", "Deutsche Boerse", "deutsche-boerse.com", "DE", "national market infra", ((75, 90), (40, 60), (75, 90))),
    ("anc-019", "Ripple", "ripple.com", "US", "crypto-native co", ((60, 80), (45, 65), (55, 75))),
    ("anc-020", "XRPL Foundation", "xrplf.org", "NL", "protocol foundation", ((45, 65), (50, 70), (55, 80))),
    ("anc-021", "Hetzner", "hetzner.com", "DE", "large EU hosting co", ((50, 70), (45, 65), (65, 85))),
    ("anc-022", "OVHcloud", "ovhcloud.com", "FR", "large EU hosting co", ((55, 75), (45, 65), (65, 85))),
    ("anc-023", "DigitalOcean", "digitalocean.com", "US", "mid-tier cloud co", ((50, 70), (40, 60), (65, 85))),
    ("anc-024", "University of Ljubljana", "uni-lj.si", "SI", "national university", ((55, 75), (50, 70), (70, 90))),
    ("anc-025", "Telegram", "telegram.org", "AE", "high-reach comms co", ((55, 75), (55, 80), (40, 65))),
    ("anc-026", "Binance", "binance.com", "AE", "top exchange", ((60, 80), (35, 60), (30, 55))),
    ("anc-027", "Yandex", "yandex.com", "RU", "major co in high-pressure jurisdiction", ((50, 70), (10, 35), (25, 50))),
    ("anc-028", "VK", "vk.com", "RU", "state-influenced platform", ((35, 55), (5, 25), (20, 45))),
    ("anc-029", "Sberbank", "sberbank.ru", "RU", "SDN-listed major bank", ((45, 70), (10, 30), (0, 10))),
    ("anc-030", "Garantex", "garantex.io", "RU", "sanctioned exchange", ((10, 30), (10, 35), (0, 5))),
)


def sample_packet(item_id: str, organization: str, domain: str, jurisdiction: str) -> dict[str, Any]:
    """Build a schema-valid sample packet; domain verification is benchmark-stubbed."""

    return {
        "packet_version": "rep-v1",
        "validator_id": item_id,
        "master_key": f"SAMPLE:{item_id}",
        "domain": domain,
        "domain_verified": True,
        "x_handle": None,
        "handle_binding_evidence": "domain_verified",
        "organization_claim": organization,
        "jurisdiction_claim": jurisdiction,
        "public_footprint_fields": [
            "master_key",
            "domain",
            "domain_verified",
            "handle_binding_evidence",
            "organization_claim",
            "jurisdiction_claim",
        ],
        "agreement_context": {
            "a24h": None,
            "a30d": None,
            "totals": {"t24h": None, "t30d": None},
        },
        "sample_data": True,
    }
