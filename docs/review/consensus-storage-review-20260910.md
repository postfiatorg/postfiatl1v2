# Consensus and storage review — 2026-09-10

Status: source repairs verified; no release or deployment authorized

Reviewed checkout: `314952a9249698b9075c1ff14cb896c11b660773`

## Scope and method

The review began cold at the last commit before 2026-09-05,
`9e15181c7d176861e0081daa00bc1a6ae1f02351`, and traced validator-runtime
changes through the reviewed checkout. It covered the Consensus v2 signing
entrypoints and durable safety store around, but not a re-evaluation of, the
already-hardened monotone-round predicate; the YOLO activation, transaction,
execution, persistence, query, and state-commitment paths; and the intervening
transactional-storage and replicated-state changes. Frozen evidence was read
only.

The narrow pre-repair baselines were green:

- Consensus v2 library selection: **20 passed**.
- Consensus v2 node binary selection: **3 passed**.
- YOLO execution selection: **6 passed**.

## Findings

1. **P1 — Consensus authorization and signature emission are separated by an
   unlocked race window.**

   [`consensus_v2_finality.rs`](../../crates/node/src/consensus_v2_finality.rs)
   lines 115–200 calls the prepare, precommit, or timeout authorization helper
   and signs only after that helper returns. The helpers in
   [`consensus_v2_store.rs`](../../crates/node/src/consensus_v2_store.rs) lines
   125–176 and 291–313 hold the per-height safety guard only through the state
   read, authorization, and durable write; `with_safety_guard` releases it on
   return at lines 376–387. Transport accepts and processes connections on
   separate threads, so signing entrypoints can overlap for one validator and
   height.

   Concrete failure scenario: thread A authorizes a prepare at view 1 and
   releases the guard before ML-DSA signing. Thread B then authorizes and emits
   a timeout or vote at view 2. Thread A resumes and emits its view-1 signature
   after the durable cross-phase floor has advanced to view 2. Each individual
   authorization passed, but the emitted order violates the documented rule
   that a signature at round $r$ requires
   $r \geq \max(r_P, r_C, r_T)$. The existing delayed-certificate tests are
   sequential and do not schedule this interleaving.

2. **P2 — Activated YOLO registrations can grow consensus state without a
   bound or state-expansion charge.**

   [`yolo_target_verifier.rs`](../../crates/execution/src/yolo_target_verifier.rs)
   lines 52–84 appends every distinct registrant/series/epoch/replay tuple to
   `yolo_target_registrations`; there is no global or per-account count limit.
   [`fees_offer_planning.rs`](../../crates/execution/src/fees_offer_planning.rs)
   lines 79–129 has no YOLO arm, so both registration and receipt operations
   receive a zero state-expansion fee. Ledger validation bounds other growing
   collections but does not bound either YOLO vector. Both vectors are durable
   state and are included in the validator state commitment.

   Concrete failure scenario: after governance activation, one funded account
   repeatedly submits otherwise valid registrations with fresh digest tuples
   and future activation heights. The duplicate predicate rejects only a reused
   replay identifier or series/epoch pair for that registrant, so fresh tuples
   continue to pass. The attacker pays only the ordinary transaction byte fee
   while every validator retains, validates, serializes, and commits an
   ever-growing vector. Valid receipts can add proof-bearing rows through the
   same unbounded surface.

## Areas with no findings

- The monotone-round predicate itself covers every prepare, precommit, and
  timeout phase pairing and correctly rejects lower rounds from durable state.
- The new YOLO proof verifier binds the immutable registration, submitter,
  replay identifier, fixed public-value encoding, program key, and bounded
  Groth16 proof before a receipt is appended.
- The reviewed transactional-storage changes are formatting-only; the
  replicated-state test change retains one handle to inspect work counters.
- The genesis-registry correction validates committed golden bytes directly
  and rebuilds them only when the documented out-of-tree source archive is
  available; it does not alter the vectors.

## Repair boundary

Finding 1 requires one safety-guard critical section spanning authorization,
durable persistence, and signature construction, with a concurrent regression
that proves lower-round emission cannot follow higher-round emission. Finding
2 requires deterministic consensus bounds and state-expansion fees for newly
created YOLO rows, with exact boundary and fee regressions. Neither repair
authorizes a release, activation, or deployment.

## Repair disposition

- **Finding 1 — fixed.** The prepare and precommit persistence helpers now
  retain the per-height guard through a supplied signing callback. Timeout
  signing selects the durable high QC inside that same critical section. All
  three production entrypoints construct and verify their signatures before
  releasing the guard. A synchronized regression pauses the callback and
  proves a competing authorization cannot acquire the guard until signature
  construction returns.
- **Finding 2 — fixed.** Registration and receipt vectors now have explicit
  4,096-row consensus limits enforced both before append and during ledger
  validation. Each new row adds a 10-PFT state-expansion fee; an already-present
  row receives no positive expansion charge. The regression admits the exact
  registration boundary, rejects the next row, rejects an over-limit restored
  ledger, and checks both fee classes through signed execution.

## Post-repair verification

- Consensus v2 library selection: **20 passed**.
- Consensus v2 node binary selection: **3 passed**.
- YOLO execution selection: **7 passed**.
- YOLO node finality/replay selection: **4 passed, 1 opt-in case ignored**.
- Strict Clippy for `postfiat-ordering-fast`, `postfiat-node`, and
  `postfiat-execution`, including all targets: pass.
