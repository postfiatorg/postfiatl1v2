#!/usr/bin/env python3
"""Deterministic feature baseline (plan section 7). Higher score = better.

Inputs: frozen packets, frozen lookup snapshots, frozen sanctions table,
agreement context. NO live lookups, NO augmentation labels, NO real/stratum
fields. Published formula; every feature value is emitted alongside the score.
"""
from __future__ import annotations

import datetime as dt
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent
PKG = ROOT / "package"
AS_OF = dt.date(2026, 9, 1)


def domain_age_years(rdap: dict) -> float | None:
    if not rdap.get("ok") or not rdap.get("registration_date"):
        return None
    try:
        d = dt.date.fromisoformat(rdap["registration_date"][:10])
        return max(0.0, (AS_OF - d).days / 365.25)
    except ValueError:
        return None


def clamp(x: float) -> int:
    return max(0, min(100, round(x)))


def score(p: dict, look: dict, tab: dict) -> dict:
    d = p.get("domain")
    lk = look.get(d, {}) if d else {}
    age = domain_age_years(lk.get("rdap", {}))
    tls_ok = bool(lk.get("tls", {}).get("ok"))
    tls_org = lk.get("tls", {}).get("subject_org") if tls_ok else None
    j = p.get("jurisdiction_claim")
    ac = p.get("agreement_context") or {}
    a30 = ac.get("a30d")
    agree = float(a30) if a30 is not None else None

    # -- prestige: identity surface + longevity + org registration evidence
    prestige = 10.0
    if d:
        prestige += 8
    if p.get("domain_verified"):
        prestige += 10
    if age is not None:
        prestige += min(22.0, 22.0 * age / 25.0)  # saturates at 25y
    if tls_ok:
        prestige += 4
    if tls_org:
        prestige += 8  # OV/EV cert: independently attested organization
    if p.get("organization_claim"):
        prestige += 6
    # unknown-structure cap: claim without any independent surface
    if not d:
        prestige = min(prestige, 20.0)

    # -- censorship resistance: jurisdiction pressure table, longevity bonus
    cens = 45.0
    if j in tab["censorship_high_pressure"]:
        cens = 15.0
    elif j in tab["censorship_moderate_pressure"]:
        cens = 30.0
    elif j is None:
        cens = 35.0  # unknown jurisdiction: no redundancy evidence
    if age is not None and age >= 10:
        cens += 5

    # -- sanctions safety (inverted risk); unknown structure caps at 50
    sanc = 50.0
    if j in tab["comprehensive"]:
        sanc = 12.0
    elif j in tab["elevated"]:
        sanc = 30.0
    elif j is not None:
        sanc = 50.0
    if p.get("domain_verified") and tls_org and j is not None and sanc >= 50:
        sanc = 50.0  # affirmative-compliance evidence is out of scope: hold cap
    if agree is not None and agree < 0.5:
        sanc -= 5  # dormant/limbo operator: weaker accountability surface

    return {
        "validator_id": p["validator_id"],
        "features": {
            "domain": d, "domain_verified": p.get("domain_verified"),
            "domain_age_years": None if age is None else round(age, 2),
            "tls_ok": tls_ok, "tls_subject_org": tls_org,
            "jurisdiction": j, "agreement_30d": agree,
        },
        "prestige": clamp(prestige),
        "censorship_resistance": clamp(cens),
        "sanctions_safety": clamp(sanc),
        "composite": clamp((prestige + cens + sanc) / 3.0),
    }


def main() -> None:
    packets = json.loads((PKG / "inputs/packets.json").read_text())
    look = json.loads((PKG / "inputs/lookup_snapshots.json").read_text())["domains"]
    tab = json.loads((ROOT / "sanctions_jurisdictions.json").read_text())
    out = [score(p, look, tab) for p in packets]
    (PKG / "outputs").mkdir(exist_ok=True)
    body = json.dumps(out, sort_keys=True, separators=(",", ":"))
    (PKG / "outputs/baseline_scores.json").write_text(body)
    import hashlib
    print("baseline packets", len(out), "sha", hashlib.sha256(body.encode()).hexdigest()[:16])


if __name__ == "__main__":
    main()
