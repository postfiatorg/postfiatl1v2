# Security

This page summarizes the security position of Post Fiat L1 as recorded in the
repository: audit status, open release gates, known defects, how to report a
vulnerability, and where the threat model and audit scopes live.

## Audit status

**No external security audit is recorded in this repository.** The
[security policy](https://github.com/postfiatorg/postfiatl1v2/blob/main/SECURITY.md)
states that the code has internal review, replay tests and controlled-network
evidence, but "should not be treated as externally audited production
infrastructure." The whitepaper uses the same boundary: its "demonstrated"
results are records the project produced itself, and it states: "Neither word
means independently audited" ([Whitepaper](../whitepaper.md)).

The only running network is a controlled devnet run by six
Foundation-administered validators
([Network Status](../status/network-status.md)).

## Planned and requested reviews

These documents define review scope. None records a completed review.

| Scope | Status | Document |
| --- | --- | --- |
| AssetOrchard swap circuit `h_action` value binding (item P13) | Scope prepared; independent review required before P13 can close | [h_action binding audit scope](orchard-h-action-binding-audit-scope.md) |
| pfUSDC ↔ Arc bridge contracts and SP1 ingress program | Request for quotation (2026-09-02); the auditor "has not reviewed, audited, endorsed, or co-sponsored anything" | [Arc audit scope request](../business/arc-audit-scope-20260902.md) |
| Core protocol line count for audit sizing | 225,070 nonblank source lines at `1f478e0c` (2026-08-28) | [Core feature LOC inventory](core-feature-loc-audit-inventory.md) |
| Specialist consensus, circuit, contract and bridge review | Open release gate (row 12) | [Release Gate Inventory](../status/release-gate-inventory.md) |
| SP1 guest/program reproduction and circuit review | Open release gate (row 13) | [Release Gate Inventory](../status/release-gate-inventory.md) |

## Release gates

The [Release Gate Inventory](../status/release-gate-inventory.md) (dated
2026-09-01) recorded 21 non-storage gates before a public testnet:
**4 DONE, 16 OPEN, 1 UNKNOWN.** Storage gates (G0–G7) are tracked separately in
the [storage scaling milestone](../plans/completed/storage-scaling-milestone.md).
The table below preserves that dated inventory; it is not a recomputed total
for today's source or release candidate. Current milestones are linked from
[Testnet Path](../status/testnet-path.md).

| State | Gates |
| --- | --- |
| DONE | Security intake channel (1), threat model (2), public RPC behind an authenticated TLS edge (4), public history hygiene (16) |
| OPEN | Validator key custody (3), FastPay recovery evidence (5), release verification battery (6), release artifacts (7), migration/rollback/canary (8), publication-candidate verification (10), `h_action` external audit (11), specialist review (12), SP1 reproduction (13), deployed privacy capture review (14), external bridge verifier (15), launch authority (17), operations readiness (18), public operator runbook (19), topology/independence thresholds (20), privacy production burndown (21) |
| UNKNOWN | Protected branch and required CI enforcement (9) |

## Known defects

The [QA defect inventory](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/defect-inventory-20260910.md)
(`docs/review/defect-inventory-20260910.md`) records internal review findings:
**142 rows: 27 P1, 77 P2, 38 P3.** Its final disposition counts 109 rows as
fixed, 16 dispositioned, 5 reproduced and retained from earlier campaigns, and
4 closed on `postfiat-wan-devnet-2` on 2026-10-02, among others. The inventory
notes that "fixed" is scoped to each row: some fixes are in source but not
deployed.

## Current limitations

From the [security policy](https://github.com/postfiatorg/postfiatl1v2/blob/main/SECURITY.md)
("Current Security Limitations"):

- Validator keys remain software key files. Main implements a separate signer
  process with Unix-socket peer-UID checks and no key-file fallback in socket
  mode, and a durable double-sign guard. Its deployment under two users
  remains open; HSM custody is not implemented. The recorded fleet still uses file signers.
  See the [signer policy](../runbooks/public-rpc-operator-policy.md#validator-signer-process).
- File-signer mode needs `--unsafe-devnet-file-signer`. Long-running validator
  and RPC services on JSON/JSONL or not-yet-activated storage need
  `--unsafe-devnet-json-storage`; that flag does not select the storage backend.
  The devnet uses transactional `redb`. G5 offline qualification passed for
  the October 21 binary on October 23; G6 remains open
  ([State And Storage](../architecture/state-and-storage.md)).
- Public RPC requires an authenticated TLS edge; the node rejects public and
  wildcard plaintext binds.
- The policy "is not an external audit report or a mainnet-readiness claim."

Account-level limits that matter to integrators (no recipient check, no
checksum, no key rotation) are on
[Addresses, Keys And Signatures](../integrate/addresses-keys-signatures.md).

## Reporting a vulnerability

Do not open a public GitHub issue. Use the repository's GitHub **Report a
vulnerability** (private security advisory) form; it is the only published
intake channel. The project publishes no fallback email address or PGP key. The
policy commits to an acknowledgment within 72 hours. Only the latest `main`
branch receives security fixes. Full text:
[SECURITY.md](https://github.com/postfiatorg/postfiatl1v2/blob/main/SECURITY.md).

One incident is on record as closed:
[SEC-20260930-01](SEC-20260930-01-closure.md) (unauthorized invocation of
operator-funded services; closed 2026-10-01).

## Threat model and boundaries

- [Threat Model](threat-model.md): adversaries covered by the controlled-testnet
  work. `SECURITY.md` adds a summary table (Byzantine validator, network,
  post-quantum and privacy adversaries).
- [Public Launch Boundary](public-launch-boundary.md): what public launch adds
  beyond the controlled testnet.
- [Halo2 Dependency](halo2-dependency.md): the pinned upstream `halo2_proofs`
  and its local patch.
- [Redaction Policy](redaction-policy.md) and
  [Public History Publication](public-history-publication.md).
