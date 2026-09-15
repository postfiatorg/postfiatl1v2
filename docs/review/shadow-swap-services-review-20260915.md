# Live shadow and swap services review — 2026-09-15

This is A4 of the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). The review traced focused advisory authority, peer-message queue, signature, state and history restart paths in `crates/node/src/cobalt_shadow.rs`, and FastSwap canonical refresh, voting, asset-control and checkpoint service paths in `crates/node/src/fastswap_service.rs`. `batch_snapshot.rs` was not read for A4: neither service directly uses its functions. These focused paths were read cold; this is not a whole-file audit.

## Findings

### 1. P2 — shadow queue processing can lower a peer replay watermark

**Source:** `crates/node/src/cobalt_shadow.rs:2648-2668`.

A registered peer can sign sequence 1 for round 2 and sequence 2 for round 1 after installing randomness for both rounds. The receiver accepts both into its bounded queue because neither has been processed. `process_all` orders the queue by round before sender sequence, processes sequence 2 and then sequence 1, and overwrites the peer's inbound high watermark with 1. After enough subsequent messages remove sequence 2's ID from the bounded seen-ID list, the receiver can accept and process the already processed sequence 2 again. Even before eviction, the reported watermark and the advisory governance digest are inconsistent with the highest processed peer sequence.

The minimal repair is to keep the maximum processed sequence for each sender during ordered queue draining and add a two-round, out-of-order delivery regression that checks the watermark before and after restart and rejects a stale signed sequence. This changes the hashed shadow advisory state-transition result and is conservatively **consensus-affecting** under the brief's definition. It is source-only and must not be presented as live behavior.

### 2. P2 — issuer asset-control prepare can sign round-zero after recovery advances

**Source:** `crates/node/src/fastswap_service.rs:437-505` (compare ordinary swap prepare at `:362-384`).

For a valid signed issuer command whose operation already has a reserved input and has advanced to a later precommit round, an issuer can retry `asset_control_prepare`. The method checks only a Cancel tombstone, then reuses the same reservation and signs another round-zero CONFIRM precommit vote. Ordinary swap `prepare` refuses round-zero after a later precommit round or a cancelled, decided-cancel, superseded or checkpointed status. The issuer path can therefore emit a stale vote despite an advanced local recovery state; the operation ID and digest can remain unchanged.

The minimal repair is to apply the same existing-record status and round guard to issuer asset-control prepare before validating or signing and add a regression using a signed issuer command with a later recorded round. This tightens FastSwap validator voting admissibility and is **consensus-affecting** under the brief's signed-bytes and state-transition boundary. It is source-only and must not be presented as live behavior.

### 3. P3 — canonical refresh can write local FastSwap state before rejecting a changed tip

**Source:** `crates/node/src/fastswap_service.rs:197-319`.

If the canonical chain tip advances between the first tip read and the final tip read, refresh may first persist a prepare fence, import a deposit or apply and compact a checkpoint from the ledger it read, then return `canonical chain tip changed`. The caller sees a failed refresh although its in-memory state and local WAL have already changed. A later refresh can reconcile the newer tip, but the error does not promise that local state is unchanged, and a restart replays the writes. This is a race observation about the service boundary, not evidence of a live chain or consensus divergence.

A future minimal repair would verify a matching tip before durable refresh steps and stage the controls or recover/replay if the final tip changes; test a forced tip change around those steps. This P3 is recorded without repair.

## Areas with no findings

- `crates/node/src/cobalt_shadow.rs:603-815,935-1029,1307-1549,1551-1751,1753-2020`: reviewed shadow-only authority flags on signed state load, validator-key binding, round locks before protocol signing, bounded durable history, signed catch-up verification and restart reconciliation; no additional finding in these paths.
- `crates/node/src/cobalt_shadow.rs:2017-2290,2313-2647,2684-2808,3699-3860,3953-4205`: reviewed transcript-domain and stage-signature checks, bounded beacon evidence and queue admission, pre-signature sequence persistence, rejection on bad peer signatures, private-file permissions and bounded state reads; no additional finding beyond finding 1.
- `crates/node/src/fastswap_service.rs:42-196,324-436,506-1474,1476-1665`: reviewed committee/key checks, canonical controls on open, ordinary swap prepare guards, verified certificates and locks before voting, complete CONFIRM apply and catch-up paths, cancellation, bounded queries, checkpoint signatures and bounded base files; no additional finding beyond findings 2 and 3.

## Review limits and skips

The sections named above were sampled around A4's focus, not audited line by line. The drill generator and extensive fixture/performance tests in the two service files were not reviewed as production paths. `batch_snapshot.rs` has no directly used path in these two services and was skipped. No A1–A3 file, A5 command path, B inventory, excluded crate or file, already-reviewed crate, frozen artifact, fleet, or Task Node was reviewed or acted upon. The P3 remains recorded without repair.
