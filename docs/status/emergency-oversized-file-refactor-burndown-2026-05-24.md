# Emergency Oversized File Refactor Burndown

Status: resolved repo-shape burndown.
Date: 2026-05-24

## Reason

The repo has multiple tracked files over 5,000 lines, including several
handwritten Rust modules. The validator-evidence Python verifier was
previously 13k lines and is now split into a short entrypoint plus runtime
parts under 5,000 lines.

This burndown does not call for deleting behavior. The work is to split files
into smaller modules, preserve public behavior, keep tests passing, and stop
adding new functionality to oversized files.

## Inventory Command

```bash
git ls-files -z | xargs -0 wc -l 2>/dev/null \
  | awk '$1 > 5000 && $2 != "total" {print $1 " " $2}' \
  | sort -nr
```

The enforced baseline lives at
`docs/status/oversized-file-baseline.json`. The local guard is:

```bash
scripts/check-oversized-files
```

`scripts/check` now runs this guard before the broader workspace checks.

## Handwritten Source And Control Files Over 5k Lines

These are the refactor priority. New feature work should avoid touching these
files except to split them.

| Lines | File | Emergency Disposition |
| ---: | --- | --- |
| none | No remaining tracked handwritten source/control file is over 5,000 lines. | Continue to block new oversized source/control files through `scripts/check-oversized-files`. |

## Large Tracked Evidence Or Reference Artifacts

These should not be refactored like source code. They need evidence-retention
policy, deduplication, or artifact-storage decisions.

| Lines | File Or Pattern | Emergency Disposition |
| ---: | --- | --- |
| 21,807 | `reports/testnet-cobalt-canonical-artifacts/testnet-cobalt-collusion-threshold-normalized-v1.json` | Canonical normalized generated packet body retained once; 40 former full copies now carry hash-bound reference manifests with their original timestamps and SHA-256 values. |
| 125-line manifest + shards below 5,000 | `docs/status/ambient-finding-disposition-ledger.json` and `docs/status/ambient-finding-disposition-ledger/*.json` | Row-level Ambient ledger is sharded into 9 hash-bound files; the manifest keeps counts and canonical reconstruction hash. |
| 9,005 | `docs/references/cobalt-bft-governance-in-open-networks.pdf` | Retain in Git as a small hash-pinned reference artifact for the local Cobalt Markdown extraction; `scripts/check-reference-artifacts` verifies SHA-256, byte size, and line count. |
| 5,179 and 5,317 | `reports/testnet-cobalt-canonical-artifacts/testnet-cobalt-strict-launch-expected-fail-normalized-*.json` | Canonical normalized strict-launch expected-fail bodies retained once per normalized body; the 4 oversized strict reports now carry hash-bound reference manifests. |

## P0 Stop-The-Bleed Tasks

- [x] Do not add new validator-evidence gates to
  `scripts/validator-evidence-fixtures-validate` until it is split.
- [x] Add a lightweight oversized-file check that fails on new or enlarged
  handwritten source/control files over 5,000 lines.
- [x] Define an allowlist for generated evidence, vendored files, binary
  references, and intentionally retained reports.
- [x] Update `whip.md` so validator-evidence continuation does not keep
  extending the 13k-line verifier.

## Completed Refactor Slices

- [x] Split `scripts/validator-evidence-fixtures-validate` from a 13,504-line
  monolith into an 8-line entrypoint and ordered runtime parts under
  `scripts/validator_evidence_fixtures_validate_parts/`, with each part below
  5,000 lines. The full verifier still passes.
- [x] Split `crates/rpc_sdk/src/lib.rs` from a 14,455-line monolith into an
  8-line crate-root include entrypoint, two production parts, and two test
  parts under `crates/rpc_sdk/src/lib_parts/`, with each part below 5,000
  lines. `cargo test -p postfiat-rpc-sdk` passes.
- [x] Split `crates/node/src/lib.rs` and `crates/node/src/main.rs` from
  13,035-line and 5,432-line roots into short include entrypoints plus
  `crates/node/src/lib_parts/` and `crates/node/src/main_parts/` parts, with
  each new part below 5,000 lines. `cargo test -p postfiat-node` passes.
- [x] Split `crates/node/src/lib_tests.rs` from a 12,750-line test module into
  a 7-line test wrapper and four feature-area test parts under
  `crates/node/src/lib_test_parts/`, with each part below 5,000 lines.
  `cargo test -p postfiat-node` passes.
- [x] Split `crates/consensus_cobalt/src/lib.rs` from a 10,260-line monolith
  into a 10-line crate root, five protocol parts, and one test part under
  `crates/consensus_cobalt/src/lib_parts/`, with each part below 5,000 lines.
  `cargo test -p postfiat-consensus-cobalt` passes.
- [x] Split `crates/node/src/governance_agent.rs` from a 7,596-line DGA module
  into a 1,104-line schema/options root plus gate, implementation,
  verifier/adversarial, ruleset/hash, and test parts under
  `crates/node/src/governance_agent_parts/`, with each part below 5,000 lines.
  `cargo test -p postfiat-node` passes.
- [x] Split `crates/execution/src/lib.rs` from a 7,184-line state-transition
  module into a 9-line crate root plus entrypoint, fee/offer-planning,
  NFT/escrow/asset state, hashing, and test parts under
  `crates/execution/src/lib_parts/`, with each part below 5,000 lines.
  `cargo test -p postfiat-execution` passes.
- [x] Split `crates/types/src/lib.rs` from a 5,987-line shared type root into
  a 9-line crate root plus core chain, ledger/asset, shielded/bridge/governance,
  transaction/mempool/receipt, and test parts under
  `crates/types/src/lib_parts/`, with each part below 5,000 lines.
  `cargo test -p postfiat-types` passes, and `cargo test -p postfiat-node
  --no-run` confirms the broad downstream type consumers still compile.
- [x] Replaced 40 full-size `testnet-cobalt-collusion-threshold.json` packet
  copies with hash-bound reference manifests and one canonical normalized
  generated packet body under `reports/testnet-cobalt-canonical-artifacts/`.
- [x] Replaced the 4 oversized
  `testnet-cobalt-strict-launch-expected-fail-self-test.json` generated report
  copies with hash-bound reference manifests and 2 canonical normalized bodies
  under `reports/testnet-cobalt-canonical-artifacts/`.
- [x] Added `scripts/check-cobalt-report-references` to verify each reference
  manifest's canonical artifact hash and reconstructed original SHA-256.
- [x] Replaced the 10,549-line
  `docs/status/ambient-finding-disposition-ledger.json` root file with a
  125-line manifest and 9 shard files under
  `docs/status/ambient-finding-disposition-ledger/`, all below 5,000 lines.
- [x] Added `scripts/check-ambient-disposition-ledger` to reconstruct the
  Ambient row-level ledger from shards and verify counts, shard hashes, and the
  canonical ledger SHA-256.
- [x] Retained `docs/references/cobalt-bft-governance-in-open-networks.pdf` in
  Git as a hash-pinned 500,545-byte local reference source for the Cobalt
  Markdown extraction, with the policy recorded in
  `docs/status/reference-artifact-retention-policy-2026-05-24.md`.
- [x] Added `scripts/check-reference-artifacts` to verify hash-pinned reference
  artifacts listed in `docs/status/oversized-file-baseline.json`.

## Emergency Exit Criteria

Do not remove the emergency override from `whip.md` until all of these are
true:

- [x] `scripts/validator-evidence-fixtures-validate` is split below 5,000 lines
  or has a narrow, reviewed exception that does not allow new gates to be added
  to the root script.
- [x] The other handwritten source/control files over 5,000 lines are either
  split below 5,000 lines or have reviewed, file-specific exceptions recorded
  in this burndown.
- [x] Generated evidence and reference artifacts over 5,000 lines have an
  explicit retention policy: deduplicate, shard, move to artifact storage, or
  allowlist with reason.
- [x] `scripts/check-oversized-files` passes without expanding
  `docs/status/oversized-file-baseline.json`.
- [x] The emergency override block is removed from `whip.md`, this burndown is
  marked resolved, and the cleanup is committed.

After those criteria are satisfied, normal validator-evidence design work may
resume from `whip.md`.

## P1 Refactor Slices

- [x] Split `scripts/validator-evidence-fixtures-validate` first, because it is
  actively blocking governance-doc iteration.
- [x] Split `crates/rpc_sdk/src/lib.rs` into a module tree with no public API
  break.
- [x] Split `crates/node/src/lib.rs` and `crates/node/src/main.rs` together so
  runtime wiring and CLI dispatch stop accumulating in root files.
- [x] Split `crates/node/src/lib_tests.rs` by feature area after the node
  module split lands.
- [x] Split `crates/consensus_cobalt/src/lib.rs` with protocol invariants and
  serialization boundaries preserved.
- [x] Split `crates/node/src/governance_agent.rs` before any further DGA or
  Qwen gate implementation.
- [x] Split `crates/execution/src/lib.rs` around transaction application and
  state-transition boundaries.
- [x] Split `crates/types/src/lib.rs` only after consumers are mapped, because
  shared type moves can create wide churn.

## P2 Evidence Hygiene

- [x] Replace repeated Cobalt adversarial packet JSON copies with canonical
  packet files plus per-report hash references.
- [x] Shard or regenerate `docs/status/ambient-finding-disposition-ledger.json`
  from smaller source files.
- [x] Decide whether `docs/references/cobalt-bft-governance-in-open-networks.pdf`
  remains in Git, moves to LFS, or moves to hosted docs storage.

## Acceptance Criteria

- [x] No handwritten source/control file remains over 5,000 lines without a
  documented exception.
- [x] The validator-evidence verifier is split before any new validator-evidence
  design gates are added.
- [x] Refactors preserve public behavior and file-format compatibility.
- [x] Focused tests pass for each affected crate or script.
- [x] The oversized-file check is part of local verification.
- [x] Generated evidence over 5,000 lines is either deduplicated or explicitly
  allowlisted with a retention reason.

## Verification Per Slice

Use the narrowest verification that matches the touched surface:

- Python verifier split: `python3 -m py_compile` on split scripts/modules and
  `scripts/validator-evidence-fixtures-validate`.
- Rust crate splits: `cargo test -p <crate>` plus any existing focused command
  for the moved behavior.
- Docs/status-only updates: `git diff --check` and docs build if the file is in
  the MkDocs tree.
- Generated Cobalt report deduplication:
  `scripts/check-cobalt-report-references`, `scripts/check-oversized-files`,
  and `python3 -m json.tool docs/status/oversized-file-baseline.json`.
- Ambient disposition ledger sharding:
  `scripts/check-ambient-disposition-ledger`, `scripts/check-oversized-files`,
  and `python3 -m json.tool docs/status/ambient-finding-disposition-ledger.json`.
- Reference artifact retention:
  `scripts/check-reference-artifacts`, `scripts/check-oversized-files`, and
  `python3 -m json.tool docs/status/oversized-file-baseline.json`.
