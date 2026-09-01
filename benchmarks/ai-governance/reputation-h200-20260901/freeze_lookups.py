#!/usr/bin/env python3
"""Freeze every external lookup the deterministic baseline consumes.

Plan section 7: the manifest freezes every external lookup response by
timestamp and content hash; baseline execution then uses only these snapshots.
Lookups per unique domain across the 90 packets:
  - RDAP (rdap.org bootstrap): registration event date -> domain age
  - TLS: leaf certificate notBefore/notAfter, issuer, subject org (serves as
    the organization-registration evidence surface)
Failures are recorded honestly as {"ok": false, "error": ...}.
Run BEFORE build_package.py. Never reads augmentation labels.
"""
from __future__ import annotations

import datetime as dt
import hashlib
import json
import pathlib
import socket
import ssl
import urllib.request

from build_package import anchor_packets, aug_packets, base_packets, SNAPSHOT_SRC

ROOT = pathlib.Path(__file__).resolve().parent
OUT = ROOT / "package/inputs/lookup_snapshots.json"
TIMEOUT = 15


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def rdap(domain: str) -> dict:
    ts = now()
    try:
        req = urllib.request.Request(f"https://rdap.org/domain/{domain}", headers={"User-Agent": "postfiat-benchmark/1.0"})
        with urllib.request.urlopen(req, timeout=TIMEOUT) as r:
            body = r.read()
        data = json.loads(body)
        reg = next((e.get("eventDate") for e in data.get("events", []) if e.get("eventAction") == "registration"), None)
        return {"ok": True, "queried_at": ts, "registration_date": reg,
                "response_sha256": hashlib.sha256(body).hexdigest()}
    except Exception as e:  # noqa: BLE001 - recorded, not raised
        return {"ok": False, "queried_at": ts, "error": f"{type(e).__name__}: {e}"[:200]}


def tls(domain: str) -> dict:
    ts = now()
    try:
        ctx = ssl.create_default_context()
        with socket.create_connection((domain, 443), timeout=TIMEOUT) as sock:
            with ctx.wrap_socket(sock, server_hostname=domain) as w:
                cert = w.getpeercert()
        subject = {k: v for pair in cert.get("subject", []) for (k, v) in pair}
        issuer = {k: v for pair in cert.get("issuer", []) for (k, v) in pair}
        rec = {"ok": True, "queried_at": ts,
               "not_before": cert.get("notBefore"), "not_after": cert.get("notAfter"),
               "issuer_cn": issuer.get("commonName"), "issuer_org": issuer.get("organizationName"),
               "subject_cn": subject.get("commonName"), "subject_org": subject.get("organizationName")}
        rec["response_sha256"] = hashlib.sha256(json.dumps(rec, sort_keys=True).encode()).hexdigest()
        return rec
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "queried_at": ts, "error": f"{type(e).__name__}: {e}"[:200]}


def main() -> None:
    packets = base_packets(SNAPSHOT_SRC.read_bytes()) + aug_packets() + anchor_packets()
    domains = sorted({p["domain"] for p in packets if p["domain"]})
    snap = {}
    for d in domains:
        snap[d] = {"rdap": rdap(d), "tls": tls(d)}
        print(d, "rdap" if snap[d]["rdap"]["ok"] else "rdap-FAIL", "tls" if snap[d]["tls"]["ok"] else "tls-FAIL", flush=True)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    body = json.dumps({"frozen_at": now(), "domains": snap}, sort_keys=True, separators=(",", ":"))
    OUT.write_text(body)
    print("domains", len(domains), "sha", hashlib.sha256(body.encode()).hexdigest()[:16])


if __name__ == "__main__":
    main()
