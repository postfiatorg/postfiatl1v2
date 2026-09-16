# Mempool proposals review — 2026-09-16

This is A1 of the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md). The review read all 3,337 lines of `crates/node/src/mempool_proposals.rs` at `c4303717`, concentrating on admission and ordering, duplicate/conflicting transactions and proposals, size/count bounds, fees/nonces, malformed input and replay. Source references below describe that pre-repair revision. Called implementations outside this file were not reviewed.

## Findings

### MPL-01. P2 — admission omits offers from the sender pending limit

**Source:** `crates/node/src/mempool_proposals.rs:2074-2151`, particularly `:2132-2142`; offer admission at `:1075` and state-limit verification at `:2287-2300`.

`enforce_mempool_admission_limits` counts every account-debit family except `pending_offer_transactions`. A funded sender can submit a sequence of otherwise valid offers up to its per-sender quota and then submit another offer. The admission guard still sees zero sender entries, the offer dry-run can succeed, and the extra entry is persisted. The same omission also lets another family pass this guard when the quota is already occupied by offers. The global transaction cap remains enforced, but the sender can consume other senders' intended capacity. Once the sender exceeds its quota, `enforce_mempool_state_limits` rejects the pool, so `verify_mempool` and `create_mempool_batch` cannot process it normally.

The minimal repair is to use the existing `mempool_pending_count_for_sender`, which already counts offers and both atomic-swap owners, in the admission guard. Add a regression at the exact offer-only quota, just below it, and at a mixed-family quota; retain global-cap and pending-atomic-swap interlocks. This is local mempool admission policy, not a consensus validity rule, state-transition result, on-disk format or signed/hashed encoding. **Not consensus-affecting.** No excluded-file repair dependency.

### MPL-02. P3 — latest transaction reporting reverses atomic-swap and FastLane order

**Source:** `crates/node/src/mempool_proposals.rs:2305-2347`, particularly `:2322-2333`; execution order at `:1704-1881` and selection order at `:2464-2512`.

For a pool containing an atomic swap and a FastLane primary transaction, with no escrow, NFT or offer entries, `mempool_latest_tx_id` reports the atomic swap. Both verification and batch selection process FastLane after atomic swaps. An operator using the report's latest ID as the final transaction in the module's family order therefore receives the wrong entry. The field also cannot establish wall-clock admission recency across families because entries retain no common arrival order here. This is a reporting issue; it does not change execution order.

A minimal future repair for execution-order reporting is to check the FastLane tail before the atomic-swap tail and state the field's meaning clearly. A true admission-recency field would require separately designed metadata. This P3 is recorded without repair under the brief.

## Areas with no findings

- Lines 45–116, 119–1114: JSON entry points check byte limits before parsing; admissions bind chain domains, check transaction IDs and sender sequences, and use dry-run receipts before appending. Atomic swaps check activation/pause and both owners; account-debit FastLane operations check sequence conflicts. No additional confirmed finding in the local guards. FastLane and NFT paths omit later-family revalidation; no concrete executable conflict was established within the permitted file, so that observation is not claimed as a defect.
- Lines 1116–2072 and 2154–2303: transaction-family coverage in ID lookup, sender counting and state-limit verification; canonical dry-run family order; recomputed IDs; duplicate sender sequences; checked fee/amount totals; and terminal-receipt reconciliation were examined. No additional confirmed finding. Storage and execution helper semantics remain outside this review.
- Lines 2373–2706: nonzero/max batch counts, cumulative count bounds across families, rejected-entry filtering and writing the batch before removing selected entries were examined. No additional confirmed finding. File decoding, aggregate encoded-size limits and durable publication are delegated to unreviewed helpers.
- Lines 2708–3337: supported proposal kinds, required-parent checks before/after construction, checked height increments, ordered-batch replay checks, ordered-history count checks after activation, timeout-evidence and signature call ordering were examined. No additional confirmed finding at these call sites. Consensus validation, signing locks, state commitments and execution are delegated to excluded or previously reviewed modules.

## Review limits and skips

Only `crates/node/src/mempool_proposals.rs` was reviewed as source. Package target metadata and transaction documentation supplied context, not additional review surfaces. No called storage, types, execution, finality/signing, governance, privacy, proof, bridge or state-commitment implementation was reviewed. In particular, the brief's excluded release-candidate files (including node `lib.rs`, `lib_tests.rs`, `tests/`, `execution_actions.rs`, `block_finality.rs` and the listed type files) were neither read for review nor edited. Remote-ref path metadata alone established exclusions; the release branch and checkout were not mutated.

This is a single-file adversarial review, not an end-to-end safety claim. Concurrent writers, crash/restart behavior of delegated persistence, cryptographic verification, cross-family execution semantics and archived replay remain unverified. Shielded/governance/bridge proposal orchestration was read only in this file; its called domain implementations were not followed. No full Rust, Orchard/Halo2, live-chain or fleet test was run for the findings unit. A2–A5 and B were not started; inventory rows and scoring are deferred to B. MPL-02 remains unfixed. No Task Node, fleet, spend, signup, deployment or activation action occurred.

## Repair result

MPL-01 is repaired by using the existing complete sender-count helper in `enforce_mempool_admission_limits`. The global pending limit and atomic-swap-owner interlock are unchanged. Two tests in this same source file exercise every offer-only count through the quota, rejection exactly at the quota, state-verifier rejection above it, a mixed offer/transfer quota and independent sender capacity. The quota fixtures intentionally use inert unsigned payloads; they test admission accounting, not cryptographic execution. Both regressions failed against the original guard, then passed with the repair.

**Not consensus-affecting:** only local mempool admission policy changes. No consensus validity rule, execution result, storage format or signed/hashed bytes change. No activation or deployment occurred. MPL-02 remains unfixed. **Full Rust suite verdict pending** CI.

Verification (Cargo ran with `CARGO_NET_OFFLINE=true`):

- `cargo test -p postfiat-node mempool_proposals::burn5_tests --lib --locked`: pre-repair regression run, 0 passed and 2 failed as expected at the missing offer quota. Earlier fixture-construction attempts had compile errors and executed no tests.
- `cargo check -p postfiat-node --locked`: passed.
- `cargo test -p postfiat-node mempool --lib --locked`: 16 passed, 0 failed, 0 ignored; includes both new regressions and existing sender/global limits, invalid-signature, atomic-swap and transaction-family mempool flows.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

Only `crates/node/src/mempool_proposals.rs` changed as source; the focused tests compiled dependencies and executed existing tests without reviewing or editing their excluded source files. No full workspace or long Orchard/Halo2 suite was run because this admission-accounting repair crosses neither boundary. No inventory edit or scoring was performed.
