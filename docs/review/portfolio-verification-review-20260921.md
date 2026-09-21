# Portfolio verification review — 2026-09-21

This is A1 of the [burn 6 campaign](qa-campaign-20260921-burn6-brief.md),
reviewed at `6ae06356` on `burn6-work` in `/tmp/burn6-20260921` after
`git pull --rebase origin release/combined-devnet-20260915`. References below
describe that revision. The complete permitted surface was read: 1,356 lines
across the following six files, including their tests.

| File | Lines |
| --- | ---: |
| `crates/execution/src/yolo_target_verifier.rs` | 153 |
| `crates/execution/src/yolo_collection_verifier.rs` | 205 |
| `crates/execution/src/yolo_target_execution_tests.rs` | 340 |
| `crates/node/src/yolo_target_queries.rs` | 44 |
| `crates/types/src/yolo_collection_public_values.rs` | 225 |
| `crates/node/src/tests/yolo_target_receipt_tests.rs` | 389 |

Focus: binding holdings/weights/totals to proved public values, sums, stale or
replayed proofs, target arithmetic and rounding, and receipt success reporting.
The permitted validator files verify commitments and proofs; they do not
calculate holdings, weight sums or target quantities. Those guest calculations
remain an explicit review limit, not a passed arithmetic audit.

## Findings

### PFV-01. P3 — collection regression covers only one context mismatch

**Source:** `crates/execution/src/yolo_collection_verifier.rs:180-194`;
context comparisons at `:87-140`.

**Condition:** a change removes or miswires the comparison for program, epoch,
methodology, collector, source, account/application identity, commitments,
normalized-input root or snapshot count, while preserving the attestation
comparison.

**Observed behaviour:** `collection_context_binds_every_public_value` checks
one matching context and alters only `attested_collection_sha256`. Its assertions
do not exercise rejection for the other eight digests or the snapshot count.
The adjacent test checks encoded length and verifier-kind separation, not these
negative cases. This is a code-observed coverage gap; no acceptance of an
invalid collection proof was established. The current production helper visibly
compares all ten expectations.

**Expected behaviour:** a regression described as binding every public value
should fail if any individual context binding stops rejecting mismatches,
including an otherwise valid count and a stale/different epoch.

**Suggested minimal change:** extend the existing test with one validly encoded
mismatch per digest and a different in-range snapshot count, checking each
specific error and retaining the matching-context case. No production change
is suggested. **Recorded without repair** as required for P3 findings.

No P1 or P2 finding was established within the permitted surface.

## Areas with no findings

- `yolo_target_verifier.rs:1-34`: validates registration/submission before
  verification, compares registration ID, permitted submitter and replay ID,
  decodes public bytes, validates registered expectations and invokes the bounded
  Groth16 verifier with the registered key and target-specific kind. Existing
  execution tests mutate every redundant field and each listed registered
  expectation, and reject malformed/changed proof or public bytes. The called
  type validators and shared cryptographic verifier were not re-reviewed.
- `yolo_target_verifier.rs:36-153`: inactive-feature/run rejection, strictly
  future registration activation, registrant-scoped series/epoch and replay-ID
  conflicts, exact count caps, unknown/inconsistent registration rejection and
  duplicate-receipt rejection precede receipt insertion. Verification errors
  propagate without inserting a receipt. No unbounded arithmetic or portfolio
  amount calculation occurs in these functions. Outer fee/signature/activation
  dispatch semantics are exercised by existing tests but not separately reviewed.
- `yolo_collection_verifier.rs:1-143`: distinct verifier kind, nonzero configured
  proof cap, exact configured public-value length, proof verification, strict
  decode and all nine digest plus count comparisons were examined. The context
  is caller-supplied; this stateless helper does not establish trusted context
  provenance, wall-clock freshness or one-time consumption itself.
- `yolo_collection_public_values.rs:1-225`: exact 304-byte framing, magic/version,
  fixed-order nine-digest encoding, canonical lowercase hex, count in `1..=64`,
  checked offset addition, bounds-checked slicing and trailing-data rejection.
  There are no holdings, weights, monetary totals or rounding operations here.
- `yolo_target_execution_tests.rs:1-340`: real-proof field mutations, malformed
  bytes, registration conflicts, exact state caps, signed activation/replay,
  serialization and isolation from unrelated asset state were examined. A
  mathematical proof reusable under independently pinned registrations/chains
  is not itself a violation: registration/consumer trust and destination-chain
  signature binding are separate in the documented protocol.
- `yolo_target_queries.rs:1-44` and `yolo_target_receipt_tests.rs:1-389`: query ID
  syntax, lookup by registration ID, null finality when no receipt exists,
  finality lookup by the recorded transaction hash and error propagation.
  Existing local four-store tests distinguish accepted registration/valid
  submission from rejected early/duplicate submissions, retain one receipt,
  query its valid transaction hash and reopen/replay the certified history.
  A confirmed receipt records a proved calculation, including a non-target
  outcome; it does not assert an order executed or assets moved.

## Review limits and skips

Only the six listed files were reviewed as the A1 source surface. Repository
guidance, build metadata, the burn 5 review format, required gate scripts and
the [target receipt documentation](../yolo/target-receipt-v1.md) supplied
context. No other source implementation was reviewed for correctness.

In particular, the shared `crates/execution/src/nav_sp1_verifier.rs`, target
types `crates/types/src/yolo_target_public_values.rs` and `yolo_target_receipt.rs`,
outer execution dispatch, storage, state-commitment and `tx_finality`
implementations were not opened for review. Existing tests call those
dependencies; passing them is narrower than reviewing those implementations.

Holdings completeness, weight/amount sums, target overflow/rounding, quote-age
policy, attestation provenance and guest verification-key/ELF correspondence
depend on the guest/proof tooling, including
`tools/nav-reserve-proof/programs/yolo-target-guest` and
`tools/nav-reserve-proof/crates/reserve-proof-types/src/yolo_target.rs`,
`yolo_target_proof.rs`, `yolo_collection.rs` and `yolo_nitro.rs`. These are
outside A1 and were not reviewed or changed. No malformed-holdings witness was
proved. Epoch/manifest/prior-state pinning was reviewed at the permitted call
sites; no independent live-data freshness or global cross-chain proof-replay
claim is made.

No excluded crate, frozen artifact or excluded release checkout was reviewed
or changed. No Task Node, fleet, deployment, spend, signup or installation action
occurred. A2–A5 and B were not started; inventory updates/scoring are deferred.
The opt-in externally supplied proof fixture is not enabled. No live network,
full workspace test suite, long Orchard suite, archived-chain replay or CI
status query is part of this review. Local synthetic certificate/store replay
is distinct from archived-chain re-qualification.

## Repair disposition and verification

There are **0 P1, 0 P2 and 1 P3** findings. The repair unit is skipped because
there is no P1/P2 finding. PFV-01 remains recorded without repair. No source or
test change, consensus-affecting repair or new deployment qualification is
claimed. Exact focused test results, gate results and unit commit references
are recorded in the [campaign log](qa-campaign-20260921.md#verification).
