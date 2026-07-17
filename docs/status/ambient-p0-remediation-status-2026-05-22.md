# Ambient P0 Remediation Status

Status: active-code P0 blocker scope cleared; raw Ambient inventory not fully closed
Date: 2026-05-22
Scope: Ambient review `20260522T010605Z-023bbb6ba6` against commit `fd80ae5157992e1d12548ff732101d1aa728c08e`
Current tree: uncommitted remediation changes on top of `fd80ae5157992e1d12548ff732101d1aa728c08e`

## Closeout Claim

The active runtime and source-code P0 blockers identified in
`docs/status/ambient-p0-p1-remediation-triage-2026-05-22.md` have been
remediated in the current working tree.

This is not a claim that every raw Ambient P0 record is fixed. The generated
Ambient report contains 198 original P0 records across generated reports,
operator scripts, docs, archives, and source files. The row-level disposition
ledger manifest now exists at
`docs/status/ambient-finding-disposition-ledger.json`, with row shards under
`docs/status/ambient-finding-disposition-ledger/`, but many rows are
intentionally marked generated-artifact, archive-only, script-backlog,
downgraded, or documented follow-up rather than fixed.

Safe claim now:

- Active-code P0 blockers from the reviewed triage are cleared.
- Controlled-testnet live-runtime P0 blockers from the reviewed triage are
  cleared.
- Remaining raw P0 records are generated-artifact, docs/archive, or
  operator-script backlog unless promoted back into active runtime scope.

Unsafe claim:

- All 198 raw Ambient P0 finding IDs are closed.

## Reviewed P0 Status

| ID | Status | Notes |
| --- | --- | --- |
| `SEC-P0-001` | Cleared for active code | ML-DSA private keys and wallet seeds now use explicit zeroizing boundaries across the active Rust and Python wallet surfaces. Secret-owning keypairs no longer derive clone/equality. |
| `SEC-P0-002` | Cleared for active runtime; fixture/report backlog remains | Runtime key and seed handling was hardened. Historical reports, fixture data, and generated artifacts still need a separate redaction/disposition pass before the raw ID set is closed. |
| `SEC-P0-003` | Cleared for active code | `DebugProofSystem` is no longer directly constructible as a unit struct and is gated behind an explicit controlled-testnet debug constructor. |
| `SEC-P0-004` | Partially cleared; raw script inventory remains open | The live `scripts/ai-job` command execution path was hardened. The broader remote-ops script inventory remains operator-hardening backlog, not closed raw Ambient inventory. |
| `SEC-P0-005` | Cleared for active defaults | Docs, systemd examples, and generated provision bundles now default sensitive services to localhost. The remaining `0.0.0.0` source hit is a detection guard, not a bind default. |
| `SEC-P0-006` | Not fully closed | Generated reports and historical artifacts still need redaction or quarantine policy before this raw P0 class is closed. No active source-code blocker remains from this class in the reviewed pass. |
| `SEC-P0-007` | Partially cleared; raw script inventory remains open | The reviewed dynamic execution issue in `scripts/ai-job` was removed. Other automation-authority findings in remote/test scripts remain backlog unless they are promoted into live controlled-testnet operation. |
| `SEC-P0-008` | Cleared for active code | Strict serde parsing replaced brittle manual JSON extraction for core types, ZIP32 Orchard derivation was wired into active code, debug proof use was gated, and verification passed. |

## Raw Ambient Inventory

The original generated report still has this P0 path distribution:

| Path class | Count | Current status |
| --- | ---: | --- |
| `reports/` | 96 | Historical/generated artifact hygiene backlog; see `docs/status/generated-evidence-hygiene-burndown-2026-05-22.md`. |
| `scripts/` | 74 | Remote-ops and smoke-script hardening backlog; see `docs/status/operator-script-hardening-burndown-2026-05-22.md`. |
| `crates/` | 13 | Active-code blocker set remediated in this pass. |
| `docs/` | 5 | Operator-doc backlog unless promoted into release docs. |
| `work_archive/` | 4 | Archive hygiene backlog. |
| `systemd/` | 2 | Active default fixed for docs service example. |
| `python/` | 2 | Active wallet argv secret handling fixed. |
| `docs/` | 2 | Active docs service guidance fixed. |

## Evidence

Completed verification from the remediation pass:

- `cargo fmt --check` passed.
- `git diff --check` passed.
- `cargo check --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace --all-targets` passed before the final zeroizing-only
  hardening slice.
- After the final zeroizing-only hardening slice, targeted affected tests
  passed:
  - `cargo test -p postfiat-node wallet --lib`
  - `cargo test -p postfiat-rpc-sdk wallet`
  - `cargo test -p postfiat-crypto-provider`

Focused source scans after remediation:

- No remaining active-source matches for the reviewed plain private-key and
  wallet-seed decode patterns:
  `let private_key = hex_to_bytes`, `let master_seed = wallet_master_seed_bytes`,
  or `private_key: Vec<u8>` in the checked active files.
- The debug proof scan only finds the gated constructor call in
  `crates/privacy/src/lib.rs`.
- The public bind scan only finds a `0.0.0.0` detection guard in
  `scripts/testnet-remote-observability`, not an active default bind.

## Remaining Work Before Saying "All P0s Fixed"

The row-level ledger now preserves each raw P0 ID with one of these
dispositions: fixed, duplicate, generated-artifact hygiene, archive-only,
script backlog, downgraded with rationale, or documented follow-up.

With that ledger in place, the accurate status is:

> Active-code P0 blockers are cleared. The raw Ambient P0 inventory is
> dispositioned, but not every raw P0 row is fixed.
