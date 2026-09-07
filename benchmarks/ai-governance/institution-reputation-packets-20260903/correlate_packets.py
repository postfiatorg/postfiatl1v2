#!/usr/bin/env python3
"""Deterministic validator correlation derived only from frozen identity-packet bytes.

For every pair of the 55 validators this computes auditable overlap features from the
packet's Machine-Readable Summary (canonical entity, aliases, X handle, registrable
domain, official-URL hosts, evidence hosts, incorporation and operating regions) and
publishes every pair that shares any signal, plus clusters over strong links. No model
call, no network access, no operator-supplied mapping. Re-running on the same corpus
produces byte-identical output.
"""

from __future__ import annotations

import hashlib
import itertools
import json
import pathlib
import re
from typing import Any
from urllib.parse import urlsplit

ROOT = pathlib.Path(__file__).resolve().parent
CORPUS = (ROOT.parent / "validator-identity-packets-20260901").resolve()
OUTPUTS = ROOT / "outputs"

# Hosts that many unrelated packets cite; excluded from evidence-host overlap so the
# signal reflects operator-specific sources. Listed in the output for auditability.
GENERIC_EVIDENCE_HOSTS = {
    "api.xrpscan.com", "xrpscan.com", "xrpl.org", "livenet.xrpl.org", "github.com",
    "www.linkedin.com", "linkedin.com", "x.com", "twitter.com", "en.wikipedia.org",
    "opencorporates.com", "unl.xrplf.org", "vl.ripple.com", "vl.xrplf.org",
    "xrplf.org", "foundation.xrpl.org", "scoring-testnet.postfiat.org", "postfiat.org",
    "www.postfiat.org", "docs.postfiat.org", "web.archive.org", "archive.org",
    "bithomp.com", "xrpcharts.ripple.com", "medium.com", "youtube.com", "www.youtube.com",
    "crunchbase.com", "www.crunchbase.com", "sec.gov", "www.sec.gov", "find-and-update.company-information.service.gov.uk",
}
MULTI_LABEL_SUFFIXES = {"co", "com", "org", "net", "gov", "edu", "ac"}
# Shared-hosting suffixes: two unrelated operators on the same suffix are not the same
# registrable owner, so the registrable unit is the full subdomain.
SHARED_HOSTING_SUFFIXES = {
    "github.io", "gitlab.io", "vercel.app", "netlify.app", "pages.dev", "herokuapp.com",
    "web.app", "firebaseapp.com", "wordpress.com", "blogspot.com", "substack.com",
    "notion.site", "carrd.co", "wixsite.com", "squarespace.com", "webflow.io",
}
# Platform hosts that appear in many packets' official URLs (profiles, repos, socials);
# excluded from official-host overlap so it reflects operator-owned hosts only.
GENERIC_OFFICIAL_HOSTS = {
    "github.com", "x.com", "twitter.com", "linkedin.com", "youtube.com", "medium.com",
    "discord.gg", "discord.com", "t.me", "telegram.org", "facebook.com", "instagram.com",
    "reddit.com", "xrpl.org", "livenet.xrpl.org", "xrpscan.com", "bithomp.com",
    "postfiat.org", "docs.postfiat.org", "scoring-testnet.postfiat.org",
}

WEIGHTS = {
    "canonical_entity_match": 1.00,
    "alias_overlap": 0.80,
    "x_handle_match": 0.90,
    "registrable_domain_match": 0.90,
    "official_host_overlap": 0.60,
    "evidence_host_overlap": 0.35,
    "incorporation_region_match": 0.20,
    "operating_region_overlap": 0.10,
}
STRONG = {"canonical_entity_match", "alias_overlap", "x_handle_match", "registrable_domain_match"}


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha(data: bytes | str) -> str:
    if isinstance(data, str):
        data = data.encode()
    return hashlib.sha256(data).hexdigest()


def norm(text: str | None) -> str:
    if not text:
        return ""
    text = text.lower()
    text = re.sub(r"[^a-z0-9]+", " ", text)
    return " ".join(text.split())


def entity_key(text: str | None) -> str:
    """Normalize a legal-entity string: strip corporate suffixes and punctuation."""
    words = norm(text).split()
    drop = {"ab", "ag", "inc", "llc", "ltd", "limited", "gmbh", "sa", "sas", "sarl", "bv", "corp",
            "corporation", "co", "company", "plc", "oy", "pty", "the", "foundation", "labs"}
    kept = [w for w in words if w not in drop]
    return " ".join(kept)


def host(url: str) -> str:
    try:
        h = urlsplit(url).hostname or ""
    except ValueError:
        return ""
    return h.lower().removeprefix("www.")


def registrable(domain: str | None) -> str:
    if not domain:
        return ""
    labels = domain.lower().strip(".").split(".")
    for n in (2, 3):
        if len(labels) > n and ".".join(labels[-n:]) in SHARED_HOSTING_SUFFIXES:
            return ".".join(labels[-(n + 1):])
    if len(labels) >= 3 and labels[-2] in MULTI_LABEL_SUFFIXES and len(labels[-1]) == 2:
        return ".".join(labels[-3:])
    return ".".join(labels[-2:])


def jaccard(a: set[str], b: set[str]) -> float:
    if not a or not b:
        return 0.0
    return round(len(a & b) / len(a | b), 4)


def load_packets() -> tuple[list[dict[str, Any]], dict[str, Any]]:
    index = json.loads((CORPUS / "index.json").read_text())
    manifest = json.loads((CORPUS / "manifest.json").read_text())
    rows = []
    for entry in sorted(index, key=lambda r: ({"xrpl": 0, "postfiat": 1}[r["network"]], r["validator_id"])):
        data = (CORPUS / entry["packet_path"]).read_bytes()
        if sha(data) != entry["packet_sha256"]:
            raise SystemExit(f"packet hash mismatch: {entry['packet_path']}")
        match = re.search(r"## Machine-Readable Summary\s*```json\s*(\{.*?\})\s*```", data.decode("utf-8"), re.S)
        if not match:
            raise SystemExit(f"no machine-readable summary: {entry['packet_path']}")
        summary = json.loads(match.group(1))
        names = {entity_key(summary.get("canonical_entity"))} | {entity_key(a) for a in summary.get("aliases") or []}
        names.discard("")
        rows.append(
            {
                "validator_id": entry["validator_id"],
                "network": entry["network"],
                "packet_sha256": entry["packet_sha256"],
                "canonical_entity": summary.get("canonical_entity"),
                "entity_key": entity_key(summary.get("canonical_entity")),
                "name_keys": sorted(names),
                "x_handle": (summary.get("x_handle") or "").lower().lstrip("@") or None,
                "claimed_domain": summary.get("claimed_domain"),
                "registrable_domain": registrable(summary.get("claimed_domain")),
                "official_hosts": sorted({host(u) for u in summary.get("official_urls") or [] if host(u)}),
                "evidence_hosts": sorted({host(u) for u in summary.get("evidence_urls") or [] if host(u)}),
                "incorporation_region": norm(summary.get("incorporation_region")) or None,
                "operating_regions": sorted({norm(r) for r in summary.get("operating_regions") or [] if norm(r)}),
                "identity_confidence": summary.get("identity_confidence"),
                "profile_size_tier": summary.get("profile_size_tier"),
            }
        )
    return rows, manifest


def pair_features(a: dict[str, Any], b: dict[str, Any]) -> dict[str, Any]:
    f: dict[str, Any] = {}
    f["canonical_entity_match"] = bool(a["entity_key"] and a["entity_key"] == b["entity_key"])
    f["alias_overlap"] = bool(set(a["name_keys"]) & set(b["name_keys"])) and not f["canonical_entity_match"]
    f["x_handle_match"] = bool(a["x_handle"] and a["x_handle"] == b["x_handle"])
    f["registrable_domain_match"] = bool(a["registrable_domain"] and a["registrable_domain"] == b["registrable_domain"])
    oa = set(a["official_hosts"]) - GENERIC_OFFICIAL_HOSTS
    ob = set(b["official_hosts"]) - GENERIC_OFFICIAL_HOSTS
    f["official_host_overlap"] = jaccard(oa, ob)
    f["official_hosts_shared"] = sorted(oa & ob)
    ea = set(a["evidence_hosts"]) - GENERIC_EVIDENCE_HOSTS
    eb = set(b["evidence_hosts"]) - GENERIC_EVIDENCE_HOSTS
    f["evidence_host_overlap"] = jaccard(ea, eb)
    f["evidence_hosts_shared"] = sorted(ea & eb)
    f["incorporation_region_match"] = bool(
        a["incorporation_region"] and a["incorporation_region"] == b["incorporation_region"]
    )
    ra, rb = set(a["operating_regions"]), set(b["operating_regions"])
    f["operating_region_overlap"] = jaccard(ra, rb)
    f["cross_network"] = a["network"] != b["network"]
    strength = 0.0
    for key, weight in WEIGHTS.items():
        value = f[key]
        strength += weight * (1.0 if value is True else (value if isinstance(value, float) else 0.0))
    f["strength"] = round(min(strength, 1.0), 4)
    f["strong_link"] = any(f[k] for k in STRONG)
    f["signals"] = sorted(
        k for k in WEIGHTS if (f[k] is True) or (isinstance(f[k], float) and f[k] > 0)
    )
    return f


def clusters(rows: list[dict[str, Any]], pairs: list[dict[str, Any]]) -> list[dict[str, Any]]:
    parent = {r["validator_id"]: r["validator_id"] for r in rows}

    def find(x: str) -> str:
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for p in pairs:
        if p["features"]["strong_link"]:
            ra, rb = find(p["a"]), find(p["b"])
            if ra != rb:
                parent[max(ra, rb)] = min(ra, rb)
    groups: dict[str, list[str]] = {}
    for r in rows:
        groups.setdefault(find(r["validator_id"]), []).append(r["validator_id"])
    by_id = {r["validator_id"]: r for r in rows}
    out = []
    for members in groups.values():
        if len(members) < 2:
            continue
        members = sorted(members)
        out.append(
            {
                "members": members,
                "networks": sorted({by_id[m]["network"] for m in members}),
                "entities": sorted({by_id[m]["canonical_entity"] or "" for m in members}),
                "reasons": sorted(
                    {
                        s
                        for p in pairs
                        if p["a"] in members and p["b"] in members and p["features"]["strong_link"]
                        for s in p["features"]["signals"]
                        if s in STRONG
                    }
                ),
            }
        )
    return sorted(out, key=lambda g: (-len(g["members"]), g["members"][0]))


def main() -> None:
    rows, corpus_manifest = load_packets()
    pairs = []
    for a, b in itertools.combinations(rows, 2):
        f = pair_features(a, b)
        if f["signals"]:
            pairs.append({"a": a["validator_id"], "b": b["validator_id"], "a_network": a["network"], "b_network": b["network"], "features": f})
    pairs.sort(key=lambda p: (-p["features"]["strength"], p["a"], p["b"]))
    groups = clusters(rows, pairs)
    total_pairs = len(rows) * (len(rows) - 1) // 2
    report = {
        "artifact": "institution-reputation-packets-20260903/correlation",
        "shadow_only": True,
        "method": "deterministic pairwise overlap of packet Machine-Readable Summary fields; no model, no network",
        "identity_corpus": {
            "name": "validator-identity-packets-20260901",
            "packet_set_sha256": corpus_manifest["hashes"]["packet_set_sha256"],
            "index_json_sha256": corpus_manifest["hashes"]["index_json_sha256"],
        },
        "weights": WEIGHTS,
        "strong_link_features": sorted(STRONG),
        "generic_evidence_hosts_excluded": sorted(GENERIC_EVIDENCE_HOSTS),
        "generic_official_hosts_excluded": sorted(GENERIC_OFFICIAL_HOSTS),
        "shared_hosting_suffixes": sorted(SHARED_HOSTING_SUFFIXES),
        "counts": {
            "validators": len(rows),
            "pairs_total": total_pairs,
            "pairs_with_any_signal": len(pairs),
            "pairs_strong": sum(p["features"]["strong_link"] for p in pairs),
            "clusters": len(groups),
            "clustered_validators": sum(len(g["members"]) for g in groups),
        },
        "validators": [
            {k: r[k] for k in (
                "validator_id", "network", "packet_sha256", "canonical_entity", "x_handle", "claimed_domain",
                "registrable_domain", "incorporation_region", "identity_confidence", "profile_size_tier",
                "official_hosts", "evidence_hosts",
            )}
            for r in rows
        ],
        "clusters": groups,
        "pairs": pairs,
        "correlator_sha256": sha(pathlib.Path(__file__).read_bytes()),
    }
    OUTPUTS.mkdir(exist_ok=True)
    (OUTPUTS / "correlation.json").write_text(canonical(report) + "\n")

    by_id = {r["validator_id"]: r for r in rows}
    lines = [
        "# Validator correlation from frozen identity packets",
        "",
        "SHADOW_ONLY. Computed deterministically from the Machine-Readable Summary of each",
        f"frozen packet in `validator-identity-packets-20260901` (packet-set SHA-256 `{report['identity_corpus']['packet_set_sha256']}`).",
        "No model call, no network access. A link means two packets share public-identity",
        "signals; it does not prove common key control.",
        "",
        f"- validators: {len(rows)}; pairs with any signal: {len(pairs)} of {total_pairs}; strong pairs: {report['counts']['pairs_strong']}; clusters: {len(groups)}",
        "",
        "## Clusters (strong links: same entity, alias, X handle, or registrable domain)",
        "",
    ]
    if not groups:
        lines.append("_none_")
    for g in groups:
        lines.append(f"- **{' / '.join(e for e in g['entities'] if e) or 'unnamed'}** ({', '.join(g['networks'])}; {', '.join(g['reasons'])})")
        for m in g["members"]:
            r = by_id[m]
            lines.append(f"  - `{m}` — {r['network']} · {r['claimed_domain'] or 'no domain'}")
    lines += ["", "## Top pairs by strength", "", "| strength | a | b | signals |", "| --- | --- | --- | --- |"]
    for p in pairs[:40]:
        ra, rb = by_id[p["a"]], by_id[p["b"]]
        la = f"{ra['canonical_entity'] or ra['claimed_domain'] or p['a'][:12]} ({ra['network']})"
        lb = f"{rb['canonical_entity'] or rb['claimed_domain'] or p['b'][:12]} ({rb['network']})"
        lines.append(f"| {p['features']['strength']:.2f} | {la} | {lb} | {', '.join(p['features']['signals'])} |")
    (OUTPUTS / "correlation.md").write_text("\n".join(lines) + "\n")
    print(canonical(report["counts"]))


if __name__ == "__main__":
    main()
