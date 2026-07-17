# Generated Evidence Hygiene Burndown

Status: open milestone
Date: 2026-05-22
Source review: `milestones/ambient-code-review-20260522T010605Z-023bbb6ba6/`

## Scope

The raw Ambient inventory contains generated, archived, and documentation
hygiene rows that should not be counted as active runtime exploits, but also
must not be published casually:

- 473 generated evidence rows under `reports/`
- 22 archive-only rows under `work_archive/` or archived docs
- documentation and repo-metadata rows that need publication review before any
  public release bundle

The row-level source of truth is:

- `docs/status/ambient-finding-disposition-ledger.json`, a manifest whose row
  shards live under `docs/status/ambient-finding-disposition-ledger/`

## Redaction Contract

Published evidence must remove or replace:

- absolute local paths and usernames
- hostnames, IP addresses, and topology fingerprints unless intentionally public
- private-material directory names and credential locator paths
- raw key, seed, mnemonic, spending-key, and witness fields
- sudo, SSH, cloud, or deployment credentials

## Release Gate

Before publishing `reports/`, `milestones/`, or generated docs artifacts:

1. Generate redacted copies into a publication staging directory.
2. Run `scripts/check-publish-artifact-redaction <staging-dir>` over the
   staging directory only.
3. Record the scan command and hash of the staged bundle.
4. Exclude unredacted `reports/`, `milestones/`, and `work_archive/` from
   release tarballs.

## Current Status

| Bucket | Status | Notes |
| --- | --- | --- |
| `reports/` | Open | Historical generated evidence needs redaction or quarantine before publication. |
| `milestones/` | Open by policy | Review artifacts are internal unless explicitly redacted. |
| `work_archive/` | Archive-only | Excluded from active runtime claims and public bundles by default. |
| docs/repo metadata | Partial | Active operator docs were hardened where promoted; publication hygiene remains. |

Safe claim:

> Generated evidence and archive findings are dispositioned separately from
> active runtime remediation.

Unsafe claim:

> All raw Ambient report rows are closed for publication.
