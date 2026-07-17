# Code Review Remediation Handoff

Status: open work queue for follow-up agent
Date: 2026-05-22
Scope: `$POSTFIAT_REPO`
Author: Composer code review (2026-05-22 session)

This document is the actionable handoff for another agent to address the
findings from the manual code review. It complements, but does not replace,
the Ambient scan artifacts under:

- `docs/status/ambient-p0-p1-remediation-triage-2026-05-22.md`
- `docs/status/ambient-p0-remediation-status-2026-05-22.md`
- `docs/status/ambient-p1-remediation-status-2026-05-22.md`
- `milestones/ambient-code-review-20260522T010605Z-023bbb6ba6/`

## Mission

Close the remaining review findings in priority order while preserving
controlled-testnet readiness claims. Do not claim "all P0/P1 closed" until the
Ambient raw-ID disposition ledger is completed.

Safe claim target after this handoff:

> Active runtime and operator-default paths are hardened for controlled
> transparent testnet on localhost/private networks.

Unsafe claims until explicitly completed:

> Production privacy, public RPC/transport, cross-implementation consensus, or
> "all Ambient findings closed."

## Execution Order

1. **CR-01** Canonical genesis/state-root hashing
2. **CR-02** Unify atomic write helpers
3. **CR-03** RPC public-bind guard
4. **CR-04** Debug privacy/proof launch gates
5. **CR-05** Plaintext key-file follow-up
6. **CR-06** RPC temp spool hardening
7. **CR-07** Cobalt example report-path helper
8. **CR-08** Operator script hardening (separate milestone)
9. **CR-09** Generated report / evidence hygiene (separate milestone)
10. **CR-10** Ambient raw-ID disposition ledger

## Current Remediation Result

Follow-up pass status on 2026-05-22:

| ID | Status | Evidence |
| --- | --- | --- |
| CR-01 | Implemented | Canonical `genesis_hash` and `replicated_state_root` encodings plus fixed golden-vector tests. |
| CR-02 | Implemented | Node writes now use `postfiat-storage` atomic write helpers; governance replay publication uses checked atomic publish. |
| CR-03 | Implemented | `rpc-serve` bind validation reuses the controlled transport public-bind guard. |
| CR-04 | Implemented for requested gate coverage | Release-mode debug-proof fail-closed behavior is covered by pure gate tests; debug privacy remains controlled-testnet-only. |
| CR-05 | Short-term documented | Plaintext key files remain controlled-testnet compatibility files; boundary/spec added in `docs/specs/plaintext-key-file-boundary.md`. |
| CR-06 | Implemented | RPC child requests now spool through private non-predictable temp directories with focused tests. |
| CR-07 | Implemented | Cobalt examples use `emit_example_report` with `REPORT_ROOT`, traversal rejection, and absolute-path opt-in. |
| CR-08 | Milestone created | `docs/status/operator-script-hardening-burndown-2026-05-22.md`. |
| CR-09 | Milestone + gate created | `docs/status/generated-evidence-hygiene-burndown-2026-05-22.md` and `scripts/check-publish-artifact-redaction`. |
| CR-10 | Ledger created | `docs/status/ambient-finding-disposition-ledger.json` is a shard manifest covering all 875 raw P0/P1 rows. |

Do not read this table as "all raw findings are fixed." The ledger explicitly
keeps generated artifacts, archive rows, operator-script backlog, documented
plaintext-key follow-up, and downgraded controlled-testnet boundaries separate
from fixed rows.

---

## CR-01 — Canonical genesis and replicated state-root hashing

**Priority:** P0 for consensus/cross-implementation claims
**Blocks:** Any claim that state roots or genesis hashes are protocol-stable
across implementations or serde versions.

### Problem

Two consensus-critical hashes still use JSON serialization instead of explicit
canonical encodings:

1. `genesis_hash()` hashes pretty-printed JSON plus trailing newline.
2. `replicated_state_root()` hashes `serde_json::to_vec(...)` over governance,
   ledger, shielded, and bridge state.

JSON is not a protocol contract. `skip_serializing_if`, pretty vs compact
output, and future serde changes can silently fork validators.

### Files

| File | Symbol / area |
| --- | --- |
| `crates/execution/src/lib.rs` | `genesis_hash` |
| `crates/node/src/lib.rs` | `replicated_state_root` |
| `crates/types/src/lib.rs` | `Genesis::to_json` consumers, genesis validation |
| Any tx/signing code that embeds `genesis_hash` | ensure consistent derivation |

### Required changes

1. Define explicit length-delimited canonical encodings for:
   - `postfiat.genesis.v1`
   - `postfiat.replicated_state.v1`
2. Follow the same style already used in:
   - `crates/ordering_fast/src/lib.rs` (`hash_canonical`, `append_str_field`, etc.)
   - `crates/proofs/src/lib.rs`
   - `crates/privacy/src/lib.rs`
3. Encode ledger/governance/shielded/bridge fields in deterministic order.
   Prefer explicit field walks over `serde_json::to_vec`.
4. Decide and document whether empty collections are omitted or encoded as
   empty; match that rule everywhere.
5. Add golden-vector tests with fixed inputs and expected hash hex.
6. Migrate callers to the new functions. Keep a temporary compatibility shim
   only if needed for one release cycle; otherwise break and update tests.

### Acceptance criteria

- [ ] No consensus-critical hash uses `serde_json::to_vec` or `to_json()` as
      the hash input.
- [ ] Golden vectors committed for genesis hash and replicated state root.
- [ ] Existing multi-validator smokes still converge on state root.
- [ ] `docs/status/chain-state-current.md` updated if hash semantics changed.

### Suggested tests

```bash
cargo test -p postfiat-execution genesis
cargo test -p postfiat-node replicated_state_root
cargo test -p postfiat-node --lib state_root
scripts/testnet-p0-network-gate   # if available in tree
```

---

## CR-02 — Unify atomic write helpers (remove weak duplicate)

**Priority:** P0 for local integrity on multi-user hosts; P1 for single-user dev

### Problem

`postfiat-storage` already uses hardened atomic writes (`create_new`, collision
retry, nanosecond/counter temp names). `postfiat-node` still has a separate
weaker implementation with predictable PID-based temp paths and
`File::create` truncation semantics.

### Files

| File | Notes |
| --- | --- |
| `crates/storage/src/lib.rs` | Reference implementation (`atomic_write`, `create_atomic_temp_file`) |
| `crates/node/src/lib.rs` | Duplicate `atomic_write`, `temp_write_path`, `write_synced_file` |
| `crates/node/src/governance.rs` | Uses node-local `temp_write_path` for replay packages |
| All `atomic_write(` call sites in `crates/node/` | Audit and migrate |

### Required changes

1. Extract shared atomic-write helper into one crate (prefer `postfiat-storage`
   or a tiny shared internal module if storage dependency direction is wrong).
2. Delete the duplicate helpers from `postfiat-node`.
3. Route snapshot export/import, key files, operator manifests, governance
   replay packages, and any other node writes through the shared helper.
4. Add negative tests:
   - pre-existing temp symlink must not corrupt target
   - collision retry succeeds under concurrent temp creation

### Acceptance criteria

- [ ] Exactly one atomic-write implementation for runtime code paths.
- [ ] No PID-only predictable temp naming remains in production code.
- [ ] Symlink/preexisting-temp negative test passes.
- [ ] Snapshot export/import and governance replay package tests pass.

### Suggested tests

```bash
cargo test -p postfiat-storage atomic_write
cargo test -p postfiat-node snapshot
cargo test -p postfiat-node governance
```

---

## CR-03 — RPC public-bind guard (mirror transport policy)

**Priority:** P0 before any non-localhost RPC deployment

### Problem

Transport listeners require `POSTFIAT_ALLOW_PUBLIC_TRANSPORT_BIND=1` for public,
wildcard, or DNS bind hosts. RPC defaults to `127.0.0.1` but accepts any
`--bind-host` with no equivalent guard. RPC is plaintext, unauthenticated, and
can expose mempool submit / Orchard batch creation.

### Files

| File | Notes |
| --- | --- |
| `crates/node/src/main.rs` | `rpc-serve` bind-host parsing |
| `crates/node/src/rpc_cli.rs` | `rpc_serve`, bind handling |
| `crates/node/src/transport_cli.rs` | Reuse or share `validate_controlled_transport_bind_host` |
| `docs/runbooks/public-rpc-operator-policy.md` | Document guard + TLS requirement |
| `systemd/*.example` | Ensure localhost defaults |

### Required changes

1. Reuse transport bind classification for RPC, or move shared helper to a
   small common module.
2. Fail closed when binding publicly unless
   `POSTFIAT_ALLOW_PUBLIC_TRANSPORT_BIND=1` (or introduce a dedicated RPC env
   var if policy should differ — document the choice).
3. Add tests for:
   - default localhost bind allowed
   - `0.0.0.0` / public IP rejected without opt-in
   - opt-in env allows bind but logs a warning
4. Update operator docs: public RPC requires TLS termination or SSH tunnel.

### Acceptance criteria

- [ ] Public/wildcard RPC bind fails without explicit opt-in.
- [ ] Localhost/private binds still work unchanged.
- [ ] Unit tests cover bind classification.
- [ ] Runbook documents safe deployment pattern.

---

## CR-04 — Debug privacy/proof launch gates

**Priority:** P0 for privacy/production claims; P1 for transparent testnet

### Problem

`DebugProofSystem` and debug shielded semantics remain reachable in release
builds when `POSTFIAT_ENABLE_DEBUG_PROOFS=1`. Debug nullifiers are deterministic
from public `note_id`. This is acceptable only under explicit debug/testnet
claims.

### Files

| File | Notes |
| --- | --- |
| `crates/proofs/src/lib.rs` | `DebugProofSystem::for_controlled_testnet_debug` |
| `crates/privacy/src/lib.rs` | `debug_nullifier`, debug mint/spend paths |
| `crates/node/src/main.rs` | CLI paths that can reach debug privacy |
| `crates/rpc_sdk/src/lib.rs` | RPC validation / privacy-alpha guards |
| `docs/status/public-claims-checklist.md` | Align claims with gating |

### Required changes

1. Add fail-closed tests proving release binaries reject debug proof artifacts
   unless the env var is set.
2. Add fail-closed tests proving production/privacy-alpha RPC commands reject
   debug shielded state when debug proofs are disabled.
3. Ensure startup logs clearly state when debug privacy is active.
4. Consider compile-time feature flag (`debug-privacy`) in addition to runtime
   env gate if release artifacts must never include debug paths.
5. Document in `public-claims-checklist.md` that privacy claims require Orchard
   path + no debug env + audited proof system.

### Acceptance criteria

- [ ] Negative tests for debug proof rejection in release mode.
- [ ] Negative tests for debug shielded RPC rejection.
- [ ] No silent fallback from Orchard to debug privacy.
- [ ] Claims checklist updated.

---

## CR-05 — Plaintext key-file formats and secret struct hygiene

**Priority:** P1 before public/custodial use; acceptable for controlled local testnet after permissions enforcement

### Problem

Key backup and validator key types still serialize plaintext hex secrets.
Permissions are enforced on Unix (`0600`), but formats remain copy/leak prone.
Some secret-owning structs still derive `Clone`.

### Files

| File | Types |
| --- | --- |
| `crates/node/src/node_types.rs` | `DevKeyFile`, `WalletBackupFile`, `ValidatorKeyRecord`, Orchard key files |
| `crates/rpc_sdk/src/main.rs` | secret flag handling |
| `crates/rpc_sdk/src/lib.rs` | wallet backup read/write |
| `python/postfiat_rpc/wallet.py` | temp/output permissions |

### Required changes

1. Short term (controlled testnet):
   - Remove unnecessary `Clone` derives from secret-owning file structs where feasible.
   - Ensure every read/write path validates `0600` on Unix.
   - Ensure reports never serialize raw secret fields (redaction tests).
2. Medium term (pre-public):
   - Design encrypted-at-rest wallet/key format (KDF + AEAD).
   - Migration path from plaintext backup files.
   - Document in `docs/specs/wallet-mnemonic-design.md` or new spec.

### Acceptance criteria

- [ ] No secret-owning struct appears in public RPC responses or evidence JSON.
- [ ] Redaction tests pass for wallet reports and test vectors.
- [ ] Encrypted format spec drafted OR explicitly deferred with issue link and
      operator warning in runbooks.

---

## CR-06 — RPC temp spool file hardening

**Priority:** P1

### Problem

RPC serve writes request JSON to predictable paths under the system temp dir:
`postfiat-rpc-serve-{pid}-{request_index}.json`.

### Files

| File | Notes |
| --- | --- |
| `crates/node/src/rpc_cli.rs` | `run_rpc_request_via_child` |

### Required changes

1. Use private temp dir (`0700`) or `tempfile` crate with restrictive permissions.
2. Avoid predictable filenames; use random suffix.
3. Ensure cleanup on all error paths (including child timeout).
4. Add test asserting temp dir mode and non-predictable name.

### Acceptance criteria

- [ ] Temp spool files created with private permissions.
- [ ] Filenames not predictable from pid/request index alone.
- [ ] Cleanup verified on success, failure, and timeout.

---

## CR-07 — Cobalt example safe report-path helper

**Priority:** P1 (harness hygiene)

### Problem

Cobalt examples write to environment-selected `REPORT` paths without a shared
safe-output helper. Acceptable only when the harness controls the environment.

### Files

| Path pattern | Notes |
| --- | --- |
| `crates/consensus_cobalt/examples/*.rs` | `REPORT` writes |
| Any example using env-selected output paths | audit all |

### Required changes

1. Add shared helper (likely in a small test/support module or example util):
   - reject `..` traversal
   - require output under caller-approved root
   - reject absolute paths unless explicitly allowed
2. Migrate all Cobalt examples to the helper.
3. Add tests for traversal rejection.

### Acceptance criteria

- [ ] No example writes directly to unchecked `REPORT` env path.
- [ ] Shared helper has unit tests.
- [ ] CI examples cannot escape output root.

---

## CR-08 — Operator script hardening milestone

**Priority:** P1 for live remote ops; separate large milestone

### Problem

Ambient inventory includes ~200 script findings (SSH, sudo, credential files,
generated shell, evidence redaction). Not all are live-runtime blockers, but
scripts used for controlled testnet operations must be hardened first.

### Scope

Promote and fix scripts in this order:

1. Private key / master seed / SSH credential handling
2. Remote command execution and systemd packaging
3. Release artifact / evidence publication
4. Public readiness claim scripts

### Files

| Path | Notes |
| --- | --- |
| `scripts/` | primary queue |
| `docs/runbooks/controlled-testnet-operator-launch.md` | align with hardened scripts |
| `docs/runbooks/controlled-write-edge-policy.md` | edge policy |

### Required changes

1. Build shared helpers for remote command construction and env allowlists.
2. Remove or quarantine `StrictHostKeyChecking=accept-new`, sudo-password stdin,
   and unbounded dynamic execution in live-deploy scripts.
3. Add injection-negative tests where feasible.
4. Track progress in a script-specific burndown doc.

### Acceptance criteria

- [ ] Live-deploy script list identified and hardened.
- [ ] No committed credentials or raw host fingerprints in active scripts.
- [ ] Script burndown doc with per-script status.

---

## CR-09 — Generated report and evidence hygiene

**Priority:** P1 for publication; not a runtime blocker

### Problem

Historical `reports/` contain host fingerprints, absolute paths, and topology
mappings. Many raw Ambient P0 IDs live here, not in active source.

### Required changes

1. Define redaction schema (host/user fingerprints, absolute paths, credential locators).
2. Regenerate or quarantine historical reports before publication.
3. Add release gate scan that fails if sensitive patterns appear in publish artifacts.
4. Exclude `reports/`, `milestones/`, `work_archive/` from release tarballs unless redacted.

### Acceptance criteria

- [ ] Redaction contract documented.
- [ ] Release gate scan implemented.
- [ ] Published evidence uses redacted reports only.

---

## CR-10 — Ambient raw-ID disposition ledger

**Priority:** Required before claiming "all P0/P1 closed"

### Problem

Raw Ambient counts (198 P0, 677 P1) are evidence inventory, not independent
exploits. No machine-readable ledger maps each ID to final disposition.

### Required changes

Create `docs/status/ambient-finding-disposition-ledger.json` (or equivalent) with:

```json
{
  "id": "SEC-P0-001",
  "file": "crates/crypto_provider/src/lib.rs",
  "bucket": "active-code",
  "status": "fixed",
  "final_severity": "P0",
  "rationale": "Zeroizing key wrapper added",
  "evidence": "cargo test -p postfiat-crypto-provider"
}
```

Allowed `status` values:

- `fixed`
- `duplicate`
- `downgraded`
- `false_positive`
- `generated_artifact`
- `archive_only`
- `script_backlog`
- `wont_fix_documented`

### Acceptance criteria

- [ ] Every raw Ambient P0/P1 ID has exactly one disposition row.
- [ ] Status docs updated to reference the ledger.
- [ ] No "all findings closed" claim until ledger is complete.

---

## Secondary findings (track, lower urgency)

These were noted in review but are not standalone blockers for controlled
transparent testnet on localhost:

| ID | Topic | Notes |
| --- | --- | --- |
| CR-S1 | Bridge replay cache O(n) scan | Bounded at 4096; consider `BTreeSet` if bridge traffic grows |
| CR-S2 | Ledger `Vec` insertion order in state root | Becomes moot after CR-01 canonical encoding; until then, preserve deterministic batch apply order |
| CR-S3 | JSON file store without cross-process file locks | Document single-writer assumption; consider advisory lock if RPC child + validator concurrent writes observed |
| CR-S4 | Transport pre-auth TCP surface | Acceptable on private nets until CR-03-style auth added to transport |
| CR-S5 | Orchard authorizing hash spec | Add published golden vector spec before cross-implementation Orchard claims |
| CR-S6 | `types/src/lib.rs` monolith | Long-term maintainability; split only if touching heavily |
| CR-S7 | Python plaintext RPC client | OK for local testnet; document TLS requirement for remote use |

---

## Verification gate (run after each remediation slice)

Minimum commands:

```bash
git diff --check
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Targeted scans:

```bash
# No accidental secret literals in active source
! rg -n 'private_key_hex|master_seed_hex|spending_key_hex|BEGIN [A-Z ]*PRIVATE KEY' \
  crates scripts docs docs python \
  --glob '!target/**'

# Debug proof surfaces
rg -n 'DebugProofSystem|debug_nullifier|POSTFIAT_ENABLE_DEBUG_PROOFS' crates

# JSON hashing debt (should trend to zero after CR-01)
rg -n 'serde_json::to_vec|genesis\.to_json\(\)\.as_bytes' crates/execution crates/node

# Duplicate atomic write debt (should trend to zero after CR-02)
rg -n 'fn atomic_write|fn temp_write_path' crates/

# Public bind guards
rg -n 'POSTFIAT_ALLOW_PUBLIC_TRANSPORT_BIND|validate_controlled_transport_bind_host|rpc-serve' \
  crates/node/src
```

Focused tests after privacy/RPC/storage work:

```bash
PYTHONPATH=python python3 -m unittest python.tests.test_wallet
cargo test -p postfiat-node --lib snapshot governance state_root
cargo test -p postfiat-storage atomic_write
cargo test -p postfiat-ordering-fast
cargo test -p postfiat-proofs
cargo test -p postfiat-privacy
cargo test -p postfiat-privacy-orchard
cargo test -p postfiat-rpc-sdk wallet
```

---

## Definition of done (whole handoff)

The handoff is complete when:

1. CR-01 through CR-07 are implemented with tests and docs updates.
2. CR-08 and CR-09 have explicit burndown docs and first tranche merged.
3. CR-10 ledger exists and covers all raw Ambient IDs.
4. Verification gate passes on the working tree.
5. `docs/status/chain-state-current.md` and
   `docs/status/public-claims-checklist.md` reflect the new boundaries.

Do **not** update public claims to production privacy, public decentralization,
or cross-implementation consensus until CR-01, CR-03, CR-04, and CR-05 medium-term
items are complete and evidenced.

---

## Reference: review positives (do not regress)

Preserve these while fixing findings:

- No `unsafe` in crate code
- Mempool and batch size caps
- Execution overflow checks with explicit reject receipts
- Block finality quorum / stale vote / equivocation paths
- ML-DSA keys in `Zeroizing` boundaries in crypto provider and finality signing
- RPC SDK private-key field rejection on responses
- Transport public-bind opt-in guard (extend same policy to RPC)
- Storage crate hardened atomic writes (extend to all writers)

---

## Agent instructions

When picking up this handoff:

1. Read this file and the three Ambient status docs listed at the top.
2. Start with **CR-01** unless the user directs otherwise; it is the highest
   protocol-risk item.
3. Keep diffs narrow; match existing code style and test patterns.
4. Update this file's checkboxes and status as items close.
5. Do not commit unless the user asks.
6. After each CR item, run the verification gate and record commands run in the
   PR/commit description or a short status note.
