# Certified-send eager-index remediation complete

- **Operator:** Post Fiat (`postfiatchad`)
- **Date:** 2026-08-29 UTC

> **Historical snapshot:** the separately authorized remediated campaign later
> failed once. Resume from the
> [remediated G4 qualification failure](2026-08-29___postfiatchad__remediated_g4_qualification_failure.md).

## BLUF

The candidate-owned fix for the final G4 certified-send migration mismatch is
implemented, tested, pushed, and locally evidence-bound at source `a92bb085`.
A validator now creates and binds an empty completed-set index on its first
successful resume even when no outbox exists. The runner gate logic is
unchanged; runner `a3c7bea9` adds the missing telemetry fixture. The release
freeze, G1/G2 refresh, rebuilt helper, and prepared-input rebind all pass. This
does **not** change the failed G4 result, qualify storage, authorize another
campaign, touch the devnet, or deploy the candidate. The governing documents
are the
[completed remediation spec](../plans/active/certified-send-eager-index-remediation-spec.md)
and the [storage scaling milestone](../plans/active/storage-scaling-milestone.md).

## Current state

### Exact lineage and evidence

| Boundary | Current fact |
| --- | --- |
| Node implementation | `a92bb085ceb6a9f405e916608e6b7bb6010fcc9b` (`fix(node): eagerly bind certified-send completed index`), pushed to `origin/main` |
| Frozen release binary | SHA-256 `902773e00e5226dab9e027ebce2b932b2cf26509dba08424f6ebe46db985e182`; 51,977,656 bytes; embedded revision `a92bb085`; profile `release` |
| G1 candidate manifest | PASS; SHA-256 `ed66a6375234f64d5aab863bccb6415b07c77fc5a3a028c5a6c2f01f41af0190` |
| G2 safety manifest | PASS; SHA-256 `dd300bcb8130f91ab54e26f969fe7dca37335d99cc5bf4ca78a939a79584d170` |
| Compatible rollback | PASS; six validators converged; report SHA-256 `9c32319693df1f55a6c1ecd75449fe8341d180317e11547c604f135741c3e8a5` |
| Tamper/crash matrix | PASS; 69 cases, 37 owner tests, zero uncovered requirements; report SHA-256 `6b63fe1070a2981e5d2720bf25b3cf3b8ad95beece364d9fd76579f027b146e0` |
| Runner/verifier | Branch `postfiatchad/corrected-g4-vote-lock-gate` at pushed `a3c7bea9285ab02871fd2111038764c6174b905b`; gate-logic parent `15d059d1`; test-only successor |
| Corpus helper | SHA-256 `ad70ca685cfaf1d0a67eb80f4805438c0e4363c8957598d1d884abd03690014a`; identity smoke reports `a3c7bea9` / `release` |
| Prepared input | PASS; manifest SHA-256 `c9fb32e7c3cebcf2ef16a90843c63dd96b7ed0ebc3c20ce94d2fd21707e7da42` |
| Independent input verification | PASS; all 18 references rehashed; receipt SHA-256 `6848d49d2488cd0730efd14863c5fe446a1f31827cec98346583beee8b9cbb58` |
| Campaign state | No new measurement campaign was started or authorized |
| Qualification state | Storage remains **SELECTED, NOT OFFLINE QUALIFIED**; deployment and public testnet remain blocked |

The repository branch is `main`. Runtime source `a92bb085` is the frozen node
candidate; the documentation commit containing this handoff is a docs-only
successor and is not a new runtime candidate. The runner branch is clean and
synchronized with its remote. The main worktree retains two unrelated untracked
auditor inventories under `docs/security/`; they were not edited or staged.

### What was wrong and what changed

The failed G4 campaign observed each validator's first resume. Validator 0
already had an outbox and migrated its completed-set index on observation 1.
The other five validators had no outbox, so the old node returned without
creating an index. Deliveries then created their outboxes, and migration
appeared on observation 2. The runner correctly rejected that late migration.

The fix is node-owned. In
`compact_completed_with_index_locked`, the no-outbox branch now calls the
existing `ensure_index` path. A fresh validator writes a deterministic empty
index and reports migration on its first successful resume. The following
guards remain fail closed:

- an intent without an index still requires explicit repair;
- a non-empty index without its outbox still requires explicit repair;
- pending jobs, completion and acknowledgement checks, quarantine, retention,
  atomic writes, fsync ordering, certificates, signing bytes, consensus bytes,
  Consensus v2, and Cobalt authority are unchanged.

Five node fixtures cover first resume, repeated resume, the exact
no-outbox→deliveries→resume sequence, and both tamper guards. The runner fixture
models the same telemetry sequence without changing any campaign gate.

### Verification completed

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo test -p postfiat-node completed_index_tests --locked` | PASS: 15 passed, one intentional manual release check ignored |
| `cargo test -p postfiat-node certified_send --locked` | PASS: 35 passed, one intentional manual release check ignored |
| Release 1,024-tombstone proposer rotation | PASS: 2.064 ms resume, zero retained payload reads/hashes, one bounded index read, 2.020 ms proposer/peer delta |
| Runner, packager, independent verifier | PASS: 96 tests on `a3c7bea9` |
| Compatible rollback | PASS: current→compatible ancestor→current, all six converged, literal receipts exact, zero full-history reads |
| Tamper/crash | PASS: 69 cases and complete requirement coverage |
| `cargo test -p postfiat-node transactional_verify_only --locked` | PASS: 2 passed |
| Prepared-input derivation and independent rehash | PASS: unchanged 5,000-block input, 18 references verified |

The accepted spec required proportional focused tests. The full workspace and
Orchard suites were not run because this change does not cross an Orchard
boundary.

### Boundaries preserved

- No performance campaign was started; the prior G4 output remains closed
  failed evidence and was not resumed, retried, or relabeled.
- No Task Node, agents, devnet, fleet, service, deployment, validator-directory,
  or height-924 action occurred.
- No live probe was performed. This session makes no new claim about what is
  running on the controlled devnet; use
  [Current State](../status/chain-state-current.md) for the latest recorded
  operational observation.
- The new G1/G2 and prepared-input directories are private local evidence.
  They may contain disposable keys or references to private material. Do not
  commit, publish, delete, or describe them as a redaction-safe G5 packet.
- No vote-lock, `redb`, timing-gate, matrix, consensus-byte, or governance
  behavior changed.
- The two unrelated untracked auditor inventories were preserved untouched.

## Next decision or action

There is no automatic next run.

1. Any remediated height-915 replay must be separately bounded and use binary
   `902773…e182`. Height 924 still requires a named custodian and separate
   authorization for a read-only copy; do not wait idle for it.
2. Before another G4 measurement, write and review a new campaign plan that
   binds source `a92bb085`, runner `a3c7bea9`, helper `ad70ca…014a`, prepared
   input `c9fb32…da42`, the unchanged 5+5+5 matrix, the time budget, and exactly
   one explicit run authorization.
3. G5 packaging remains blocked until remediated G3 and a future authorized G4
   pass exist. G6, deployment, and public-testnet eligibility remain later,
   separately authorized gates.

## References

- [Certified-send eager-index remediation spec](../plans/active/certified-send-eager-index-remediation-spec.md)
- [Storage scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Final G4 qualification failure](2026-08-29___postfiatchad__final_g4_qualification_failure.md)
- [Node implementation](https://github.com/postfiatorg/postfiatl1v2/blob/a92bb085ceb6a9f405e916608e6b7bb6010fcc9b/crates/node/src/certified_send_completed_index.rs)
- [Runner fixture](https://github.com/postfiatorg/postfiatl1v2/blob/a3c7bea9285ab02871fd2111038764c6174b905b/python/tests/test_storage_scaling_paired_runner.py)
- [Current State](../status/chain-state-current.md)
