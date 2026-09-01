# Non-Storage Release Gate Inventory

**Date:** 2026-09-01
**Purpose:** D1 of the [testnet-path milestone](../plans/active/l1v2-public-testnet-path-milestone.md) — an inventory of every non-storage gate that must pass before a public testnet, so D2–D4 work against a fixed list.
**Scope boundary:** storage gates (G0–G7) are tracked separately in the [storage scaling milestone](../plans/active/storage-scaling-milestone.md) and are deliberately absent here.

This is an inventory, not policy. Every state below is read from a committed
source; nothing is asserted beyond what those sources record. States: **DONE**
(the committed source records the gate as satisfied), **OPEN** (the committed
source records it as unsatisfied or still required), **UNKNOWN** (the true
state is not recorded inside this repository).

Root-level and site-excluded evidence files are linked through the public
repository because the docs site does not publish them.

## Gate table

| # | Gate | What it demands | State | Evidence | Feeds |
| --- | --- | --- | --- | --- | --- |
| 1 | Security intake channel | A private vulnerability-report channel is published and is the only intake path. | DONE | [`SECURITY.md`](https://github.com/postfiatorg/postfiatl1v2/blob/main/SECURITY.md) § Reporting a Vulnerability | D2, D4 |
| 2 | Threat model | A written threat model covers the adversaries a public testnet exposes. | DONE | [`SECURITY.md`](https://github.com/postfiatorg/postfiatl1v2/blob/main/SECURITY.md) § Threat Model Summary; [threat model](../security/threat-model.md) | D4 |
| 3 | Validator key custody | Production key custody (HSM/remote signer or encrypted keystore with rotation, separation, and audit logging) replaces plaintext software key files. | OPEN | [`SECURITY.md`](https://github.com/postfiatorg/postfiatl1v2/blob/main/SECURITY.md) § Current Security Limitations; [audit](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md) P1 custody closure requirement | D2, D4 |
| 4 | Public RPC hardening | Public RPC runs only behind an authenticated TLS edge; the node rejects public and wildcard plaintext binds. | DONE | [`SECURITY.md`](https://github.com/postfiatorg/postfiatl1v2/blob/main/SECURITY.md) § Current Security Limitations; [public RPC operator policy](../runbooks/public-rpc-operator-policy.md) | D2 |
| 5 | Owned-lane recovery evidence | External deployment/operations evidence for the FastPay versioned ordered-recovery path. | OPEN | [`SECURITY.md`](https://github.com/postfiatorg/postfiatl1v2/blob/main/SECURITY.md) § Current Security Limitations ("remains a real-value gate") | D4 |
| 6 | Release verification battery | The full formatting/check/test/Clippy/docs/dependency/secret-scan battery passes on the exact release candidate. | OPEN (per-release; no public-testnet candidate has run it) | [release process](../release-process.md) steps 1–2 | D4 |
| 7 | Release artifacts | CycloneDX SBOM, signed deployment manifest, checksums, and a second-builder reproduction of the node hash. | OPEN (per-release) | [release process](../release-process.md) step 3; [signed deployment manifest runbook](../runbooks/signed-deployment-manifest.md) | D4 |
| 8 | Migration, rollback, canary | Snapshot migration, rollback compatibility, and a rolling canary with explicit stop conditions are exercised for the release. | OPEN (per-release) | [release process](../release-process.md) step 4 | D4 |
| 9 | Protected branch and required CI | Branch protection and the eight named required jobs are enforced on `main` and cannot be bypassed. | UNKNOWN (job definitions are committed; GitHub enforcement state is not recorded in this repository) | [release process](../release-process.md) § Protected-branch requirements; [`.github/workflows/`](https://github.com/postfiatorg/postfiatl1v2/tree/main/.github/workflows) | D4 |
| 10 | Publication-candidate verification | The secret-backed `official-mainnet-fork` job passes on the exact release revision, plus `scripts/verify-publication-candidate` and `scripts/test-productionization-closure-table --require-closed`. | OPEN (per-release; scripts are committed) | [release process](../release-process.md); [`scripts/verify-publication-candidate`](https://github.com/postfiatorg/postfiatl1v2/blob/main/scripts/verify-publication-candidate) | D4 |
| 11 | AssetOrchard `h_action` external audit (P13) | Independent review of the swap-circuit value-binding construction. | OPEN | [audit scope](../security/orchard-h-action-binding-audit-scope.md) ("independent review required before P13 can be closed") | Z3, D4 |
| 12 | Specialist consensus/circuit/bridge review | Independent specialist review of consensus, circuits, contracts, and bridge code. | OPEN | [audit](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md) § 12; [release process](../release-process.md) production-promotion paragraph | D4 |
| 13 | SP1 guest/program reproduction | SP1 guest/program reproduction and circuit review before proof-dependent activation. | OPEN | [audit](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md) § 12 ("remain real-value activation gates") | Z3, D4 |
| 14 | Deployed privacy capture review | Wire/browser/log capture review on the deployed system, beyond source scanning. | OPEN | [audit](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md) § 12 ("remains a real-value evidence gate") | Z3, D4 |
| 15 | External bridge verifier deployment | The federated settlement-release verifier is deployed at the external EVM boundary. | OPEN | [audit](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md) § 12; [mint settlement verifier](../security/mint-settlement-verifier.md) | Z3, D4 |
| 16 | Public history hygiene | Only sanitized history is public; the contaminated development history stays private. | DONE | [audit](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md) § 12 ("Source publication is closed"); [publication gate](../security/public-history-publication.md) | D4 |
| 17 | Launch authority | An explicit launch-authorization artifact exists (whitepaper: LaunchCertificate with ≥7 ratifiers). | OPEN | [audit](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md) § 8 ("Exact artifact is not implemented") and § 12 ("launch authority") | D4 |
| 18 | Operations readiness | Production alerting, independent fault drills, multi-region operations, and independent-operator evidence. | OPEN | [operations readiness inventory](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/OPEN-SOURCE-OPERATIONS-READINESS-INVENTORY-20260716.md) § Result; [audit](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md) § 12 | D2, D3 |
| 19 | Public operator runbook | Join, ML-DSA key custody, sidecar, and monitoring documentation a stranger can follow. | OPEN | milestone [D2 unchecked](../plans/active/l1v2-public-testnet-path-milestone.md); controlled-testnet pieces exist: [operator launch](../runbooks/controlled-testnet-operator-launch.md), [day two](../runbooks/operator-day-two.md), [emergency key rotation](../runbooks/validator-emergency-key-rotation.md), [incident response](../runbooks/incident-response.md) | D2 |
| 20 | Topology/independence thresholds | Placement preflight and concentration caps are defined for launch. | OPEN | thresholds proposed in [launch-topology-thresholds.md](../architecture/launch-topology-thresholds.md) (awaiting the operator's confirmation; placement preflight and L3 verifier not built); [public launch boundary](../security/public-launch-boundary.md) (mechanics may be green "before public operator diversity exists") | D3 |
| 21 | Privacy production burndown | Production-grade privacy gates: external audits, live soak, anonymity-set and disclosure model, frozen upgrade/emergency procedures. | OPEN | [privacy production burndown](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/privacy-production-burndown.md) (production-grade goal list; PRIV rows Partial) | Z3, D4 |

State count: 4 DONE, 16 OPEN, 1 UNKNOWN.

Per-release rows (6, 7, 8, 10) are process gates: the committed process defines
them, and no committed receipt binds them to a public-testnet release
candidate yet.

## Top gaps

1. **Validator key custody (row 3)** — plaintext software key files are the
   recorded posture; every public operator story (D2) inherits this until a
   production custody path exists.
2. **Public operator runbook (row 19)** — nothing stranger-followable exists;
   only controlled-testnet operator documents are committed.
3. **Specialist external reviews (rows 11–14)** — the longest-lead-time OPEN
   items; the P13 `h_action` scope is prepared but no independent review is
   recorded.
4. **Launch authority (row 17)** — the whitepaper's LaunchCertificate artifact
   is explicitly not implemented, and no substitute authorization artifact is
   recorded.
5. **Protected-branch enforcement (row 9)** — the only UNKNOWN: cheap to
   verify against GitHub settings, and the release process treats it as
   mandatory.
