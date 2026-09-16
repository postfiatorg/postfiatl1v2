# Swap and recovery services review — 2026-09-16

This is A5 of the [burn 5 campaign](qa-campaign-20260916-burn5-brief.md), reviewed cold on clean `main` at `3bfcbfed` after `git pull --rebase origin main`. All 4,171 lines of the five allowed files were read, including in-file tests: `pftl_swap_service.rs` (2,028), `atomic_swap_rpc.rs` (453), `atomic_swap_rpc_server.rs` (320), `fastpay_recovery_node.rs` (986), and `operator_attestations.rs` (384), under `crates/node/src/`. Focus: amount arithmetic and rounding, swap/recovery replay, attestation verification, state-change interlocks, and malformed input. Source references below name the pre-repair revision.

## Findings

### SWP-01 — P2 — FastPay rollback compares two different input orderings

**Source:** `crates/node/src/fastpay_recovery_node.rs:103-126,609-621`.

The inverse journal sorts prior objects by their original ledger positions. Rollback instead zips those records with certificate inputs in certificate order. For ledger objects `[B, A]` and a transfer consuming `[A, B]`, the ordinary transfer model succeeds and the saved inverse is `[B, A]`; rollback returns `FastPay speculative rollback inputs do not match certificate`. A certified block omitting that local speculative effect cannot finish the inverse operation through this path. This is a recovery availability failure, not evidence of a conflicting certificate being accepted.

Minimal repair: compare exact input identity/version membership independently of order, retain unique ascending ledger positions for restoration, and reject duplicate or substituted identities. Add a regression applying the real owned-transfer model and restoring the exact original ledger when certificate and ledger orders differ, plus invalid identity cases. **Consensus-affecting: yes**, conservatively: this changes recovery reconciliation results. Keep the journal format and certificate/signature bytes unchanged; no activation or deployment.

An exploratory in-file regression reproduced this rejection before the findings commit. It used a synthetic certificate/fence around the real transfer model; it did not exercise certificate quorum verification or historical replay. The exploratory edit was removed before this findings-only unit.

### SWP-02 — P2 — forward journal transitions can replace the batch identity

**Source:** `crates/node/src/pftl_swap_service.rs:1068-1082,1117-1145`.

Same-state retries reject a different batch hash, but forward transitions overwrite it. A prepared entry for batch A can be published as B, or a published entry for A can be marked committed with B's height/reference. The durable service record then associates the signed intent with a different batch, defeating its publication identity interlock and making recovery/reporting ambiguous. This finding concerns the journal API; it does not demonstrate that the delegated consensus verifier accepts a different batch.

Minimal repair: reject batch substitution when publishing a prepared entry and when resolving an already published entry. Keep prepublication retry/reproof possible. Test conflicting publish, commit and reject requests, unchanged durable bytes on refusal, and the valid same-batch completion. **Consensus-affecting: no**: only local service journal admission changes; no replicated transition, signed bytes or storage schema changes.

### SWP-03 — P2 — journal persistence can exceed its own reload limit

**Source:** `crates/node/src/pftl_swap_service.rs:863-878,1296-1355,1374-1391`.

The writer validates at most 4,096 entries and 64 transitions per entry but does not bound serialized bytes. Enough otherwise valid histories with bounded reasons exceed 32 MiB. Persistence succeeds, after which the reader refuses the entire file as over capacity. Subsequent replay checks, transitions and crash recovery are blocked, including unrelated active swaps in the same journal.

Minimal repair: check the actual serialized journal byte length, including its trailing newline, against the reader's limit before replacement; preserve the previous readable file on refusal. Add a regression with a structurally valid over-limit journal and a previously persisted entry. **Consensus-affecting: no**: this is local service resource admission; no schema or replicated state change.

### SWP-04 — P3 — timing replay counts existing stages against capacity again

**Source:** `crates/node/src/pftl_swap_service.rs:1199-1222`.

With 64 recorded stages, retrying even one identical existing stage returns `StorageFull`: the capacity check adds the incoming map length before checking overlap. An otherwise idempotent timing retry can therefore fail near the bound. Minimal future repair: count only new keys before applying the existing conflict checks, and test exact replay at capacity. Recorded without repair as required.

### SWP-05 — P3 — attestation timestamp validation accepts impossible dates

**Source:** `crates/node/src/operator_attestations.rs:234-250`.

The timestamp validator checks length, punctuation and decimal digits only. A signer can produce an otherwise valid attestation with `2026-99-99T99:99:99Z`, and this check accepts it despite promising a UTC RFC3339 timestamp. Downstream chronology or display consumers cannot rely on that promise. Minimal future repair: validate calendar and time ranges while retaining the existing second-resolution UTC encoding, with invalid-date and leap-year cases. No freshness or external identity authorization is inferred from this standalone signature verifier. Recorded without repair as required.

## Areas with no findings

- `atomic_swap_rpc.rs:1-453`: checked next-height/sequence/fee arithmetic, bounded fee convergence, pending-owner exclusion, quote activation/expiry checks, typed market-binding errors, signed JSON byte limit and targeted-batch dry-run refusal. No additional finding in this file; execution, admission, store atomicity and snapshot coherence implementations remain delegated.
- `atomic_swap_rpc_server.rs:1-320`: typed string/u64 parsing, busy/poisoned mutation and finality locks, required-parent/proposer arguments, and matching confirmed plus accepted receipt before success. No additional finding in this wrapper; transport and certificate verification were not reviewed.
- `pftl_swap_service.rs`: reviewed u128 multiply/divide rounding with the fixed nonzero basis-point denominator, checked capacity/height/spread arithmetic, route/policy/NAV checks, quote identity and economics rebinding, principal-bound signatures, replay hashes, active-input reservation, bounded transition counts, restart handling, and bounded file reads. No additional finding beyond SWP-02/03/04. The delegated base-settlement rounding rule was not reviewed.
- `fastpay_recovery_node.rs`: reviewed committee/genesis binding, ordered-commit locks, reserve-before-sign ordering, apply-to-clone and inverse-journal-before-ledger ordering, matching confirmed-fence replay, acknowledgement construction, recovery status and certificate lookup. No additional finding beyond SWP-01; delegated cryptography, admission, storage durability and outer input limits remain unverified.
- `operator_attestations.rs`: reviewed strict deserialization, body/exclusive-control checks, signed field coverage and signature context, recomputed attestation hash, file-reader redaction gate, and master-key permission check. No additional finding beyond SWP-05; trusted expected-key/challenge matching and freshness belong to consumers outside A5.

## Review limits and skips

Only the five named source files were reviewed. Remote-ref comparisons read filenames only to establish exclusions; none of the five overlaps the release candidate. No excluded release-candidate source was opened for review or edited. In particular, `crates/execution/src/owned_transfer_recovery.rs`, `crates/types/src/fastpay_recovery_types.rs`, node `lib.rs`, `lib_tests.rs`, `tests/`, `main_parts/tests/`, `fastswap_service.rs`, `block_finality.rs`, `block_replay_wallet.rs`, transport/RPC dispatch and the other release files were excluded. Prior burns' source surfaces and the other A surfaces were not re-reviewed. Frozen artifacts and excluded crates were not reviewed or edited.

Delegated execution/types, cryptographic verification, storage and atomic-write internals, historical replay, RPC limits/routing, caller serialization, quote snapshot coherence, proof construction and verification, and external attestation trust/freshness were not audited. Compiler diagnostics supplied fixture fields without reading excluded type/test files. The FastPay regression models local inverse recovery, not a full ordered-block or quorum-certificate test. No network/socket exercise, physical crash, fleet action, Task Node, release branch/checkout action, deployment, inventory edit or scoring is part of A5. P3 findings remain unfixed. The full Rust suite is CI's verdict; repair verification will be recorded separately.
