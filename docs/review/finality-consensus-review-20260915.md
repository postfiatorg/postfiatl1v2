# Block finality, consensus artifacts, and signing review — 2026-09-15

This is A1 of the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). The review examined the finality query and block signing paths in `crates/node/src/block_finality.rs`, block vote, certificate, timeout, and governance authorization verification in `crates/node/src/consensus_artifacts.rs`, committee, vote, certificate, round-safety, and signing verification in `crates/ordering_fast/src/consensus_v2.rs`, the relevant verifier and canonical encoding paths in `crates/ordering_fast/src/lib.rs`, and the signing, context verification, encoding, and hashing paths in `crates/crypto_provider/src/lib.rs`. Other code within these files that does not implement the A1 focus was not audited; the exact limitations appear below. No other source file was reviewed as a burn 4 surface.

## Findings

### 1. P2 — the maximum timeout view overflows during proposal verification

**Source:** `crates/ordering_fast/src/consensus_v2.rs:437`.

A nonzero-view proposal verifies the timeout certificate signatures and then computes `timeout_certificate.round.view + 1` without checking overflow. A structurally valid, quorum-signed timeout certificate at view `u64::MAX` followed by a signed proposal at a nonzero view reaches that addition and panics in overflow-checking builds; release builds wrap to zero. A malformed peer artifact can therefore interrupt a verifier instead of producing a uniform fail-closed result.

The minimal repair is a checked successor comparison that rejects an exhausted view without arithmetic overflow, with a regression through the verified proposal and timeout-certificate path. This changes artifact admission at the numeric boundary and is conservatively classified as consensus-affecting; it is source-only and must not be described as live behavior.

### 2. P2 — block proposal signing omits the existing durable proposal lock

**Source:** `crates/node/src/block_finality.rs:4052-4122`.

`sign_block_proposal_file` checks storage ambiguity, membership, and the registry key before returning a proposal signature, but does not reserve the proposal-hash lock that `create_block_vote_for_target_with_timings` durably reserves before signing a vote at the same height and view. A deterministic proposer with its valid key can call `sign_verified_block_proposal` twice on different payloads for the same height and view, and both signatures verify even after restart. The vote interlock cannot prevent the conflicting signed proposals, and a later vote for either proposal may be blocked only after both have already been issued.

The minimal repair is to reuse the durable proposal-hash lock before returning the proposer signature, preserving exact same-proposal retries, and test two different signed proposals at one height and view plus a restart. This tightens validator signing admissibility and is conservatively classified as consensus-affecting; no live activation follows from the source repair.

### 3. P3 — the selected-block finality query can pair independently selected duplicates

**Source:** `crates/node/src/block_finality.rs:89-104`.

Without `audit_block_log`, `tx_finality` takes the last matching receipt and separately takes the last block linking the transaction ID; it does not establish that the selected receipt is the one from that block. If a partial restore or corrupted local receipt log contains an extra receipt for an older transaction while a later block repeats its ID, the hot path can issue `confirmed: true` and a proof ID for a receipt/block pair that a full audit would reject as duplicates. `block_log_verified: false` exposes the missing audit, but callers of this fast path receive an ambiguous pairing.

The minimal future repair is to fail closed on duplicate matching receipts and duplicate block links before returning a hot-path report, with a corrupt-log regression. This P3 observation is recorded and not fixed during A1.

## Areas with no findings

- `crates/node/src/consensus_artifacts.rs:301-452,590-946,1024-1129`: block certificate and timeout vote verification checks chain domain, expected committee, exact quorum, sorted distinct registered identities, accepted votes, registry root, vote target, and the ML-DSA context before an ID is accepted.
- `crates/node/src/block_finality.rs:2400-2753,2932-3145`: votes reserve a durable proposal lock before signing; aggregation rejects duplicate vote identities and verifies each registered vote before writing a certificate.
- `crates/ordering_fast/src/consensus_v2.rs:475-1020,1040-1420`: typed prepare and precommit votes, timeout votes, QCs and TCs bind round, phase, domain and block; certification sorts distinct verified identities and the durable safety-state authorization enforces a shared round floor.
- `crates/ordering_fast/src/lib.rs:329-572` and `crates/crypto_provider/src/lib.rs:45-298`: the reviewed quorum, signature-context, hash-domain, and key-length paths had no additional findings.

## Review limits and skips

Within `crates/node/src/block_finality.rs`, account-transaction index construction and query presentation (lines 163–2216) and remaining batch simulation and archive tools were not reviewed. Within `crates/node/src/consensus_artifacts.rs`, unrelated shielded, bridge, pfUSDC/Arc, owned-object, batch-action construction, operator-manifest, and snapshot helper paths were not reviewed. Within `crates/ordering_fast/src/lib.rs`, the previously reviewed simulation and legacy ordering model, admission receipt, and omission-evidence paths were not re-reviewed. No Orchard, bridge, proof, privacy, program, Python, storage, execution, Cobalt, mempool, network, or RPC SDK code was reviewed. No fleet action, Task Node action, external spend, or frozen artifact write occurred.

## Repair scope

Findings 1 and 2 require focused regressions and source-only repair. Finding 3 remains recorded as P3. Full Rust suite verdict will be pending CI on any repair.
