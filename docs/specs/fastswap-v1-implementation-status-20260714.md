# FastSwapV1 implementation status — 2026-07-14

Protocol authority:
`orc_directives/FASTPAY-ATOMIC-SWAP-PROTOCOL-GROUND-SPEC-20260714.md`.

Repository baseline: `87500aed04017d1870978ca99e8dd9dd6f773291`.
Implementation branch: `fastswap-v1-20260714`.
Verified code head for this record:
`5be15899eecc9685c94689b8144c76d425732879`.

## Outcome

The FastSwapV1 implementation is complete through the local portion of P9.
Epoch one now has a canonical governance bootstrap and activation cycle, and an
isolated six-process test proves the real TCP RPC/wallet path, quorum-early
settlement, exact-six repair, conservation, accepted receipt semantics, clean
shutdown, and durable restart replay. No shared fleet was deployed or activated
while producing this record. Independent review may run in parallel and is not
an implementation blocker. Safety, durability, quorum, conservation, and
activation checks remain mandatory.

FastSwapV1 is a separate prefunded object lane. A swap requires both owner
authorizations, reserves all inputs atomically, forms distinct-validator quorum
certificates, reaches one durable Confirm-or-Cancel decision, and applies
both-or-neither conserved effects. It does not reuse the unsafe legacy
single-owner unlock protocol.

## Packet status

| Packet | Status | Grounded implementation |
|---|---|---|
| P0 model | Complete | `crates/fastswap_model`; exhaustive n=4/n=6 scheduling model, negative control, partitions, crash/restart, stale-QC and rotation invariants |
| P1 types | Complete | `crates/types/src/fastswap_types.rs`; typed domains/IDs, bounded canonical encodings, frozen Rust/browser vector, lossless reserve JSON |
| P2 execution | Complete | `crates/execution/src/fastswap*.rs`; deterministic DvP, exact ratio/expiry, checked per-asset conservation, holder permits and issuer controls |
| P3 storage | Complete | `crates/storage/src/fastswap_store.rs`; checksummed WAL, persist-before-sign, atomic reservations, snapshots, tombstones, replication outbox and canonical base-state files |
| P4 decision | Complete | `fastswap_decision.rs`; LockQC, Confirm/Cancel recovery, round locks, stale-QC rejection, leader selection, catch-up and equivocation evidence |
| P5 service/relay | Complete | `crates/node/src/fastswap_service.rs` and RPC SDK; bounded parallel quorum collection, persistent TCP sessions, retrievable votes and permissionless recovery |
| P6 bridge/reserves | Complete | canonical primary deposit/redeem/checkpoint/control transactions, reserve debit/credit, exact-once import/redemption and anchored fee burns |
| P7 policy/rotation | Complete | immutable policy snapshots, stop-prepare fencing, certified checkpoints, fail-closed exits and root-verified migration; v1 rotation requires a claim-free full drain |
| P8 wallet/API | Complete | Rust wallet driver, explicit `consensus_w6 | fastswap_v1` selection, dual signing, recovery, exact-six repair, issuer control signing and SDK CLI primary submission |
| P9 activation/evidence | Local code/evidence complete; shared rollout pending | governance bootstrap atomically installs committee epoch 1, rules, policies and activation height; fail-closed activation/rotation checks and no-vote preview exist; six real local RPC processes settle, repair exact-six and restart cleanly; WAN shadow, controlled dust and rolling fleet evidence require an explicitly selected deployment target |

## Safety decisions closed during implementation

- Quorum counts distinct committee validator identities; duplicate votes fail.
- Every vote is durably persisted before it is returned or signed state advances.
- Confirm-or-Cancel is a BFT decision. There is no unilateral timeout unlock.
- All input reservations and all terminal effects are one atomic durable record.
- Delayed certified decisions supersede partial local locks and cannot revive a
  terminal tombstone.
- Deposits debit canonical balances into per-asset reserves before importing a
  FastLane object. Exit claims redeem at most once.
- Pending fee burns are debited from canonical reserves exactly once when a
  certified checkpoint is anchored.
- Retired committees cannot mutate state. A successor committee is accepted
  only when its base root matches the prior anchored drained checkpoint.
- Because v1 has no bounded exit-claim membership proof, rotation is allowed
  only after every exit claim has been redeemed. Retired-committee redemption
  is rejected rather than guessed.
- Release/profile/topology and response semantics fail closed; per-swap state,
  heights, nonces, proposer routing and balances are never treated as immutable.
- Epoch-one bootstrap is an exact governance action bound to the committee root,
  ordered rule/policy hashes, amendment validator set and activation height.
  Existing or partially initialized FastSwap state makes bootstrap fail closed.
- Before activation, preview is read-only and prepare/commit/apply cannot vote.
  After activation, every mutating phase rechecks canonical committee and height.
- The wallet signs and self-verifies both ML-DSA-65 owner authorizations in
  parallel over one immutable preimage. Per-validator/per-phase TCP lanes are
  serialized independently so quorum-early background workers cannot multiply
  connections; an idled connection is replayed at most once only for the
  already-certified, idempotent catch-up RPC. Mutation waves still enter
  unknown-result recovery instead of being automatically replayed.

## Verification evidence

Focused head verification:

- types: 4 FastSwap tests passed;
- execution: 21 FastSwap tests passed, including a 10,000-split conservation
  property and W6 differential economics;
- storage: 9 FastSwap tests passed, including torn-write, corruption,
  pre/post-sync crash, snapshot/WAL truncation and nonempty base-state startup;
- SDK/CLI: 51 library and 2 CLI tests passed;
- six-validator service: 22 passed, 1 explicit performance test ignored in the
  normal suite; includes Byzantine/below-quorum, recovery, replay, exit,
  migration, canonical epoch-one provisioning, issuer-control and
  persistent-wallet cases;
- canonical governance bootstrap: node commit/replay/`verify_state` test passed,
  including payload substitution rejection and exact ledger mutation;
- six real OS processes: two explicit TCP RPC gates passed. Preview left no
  record; settlement formed three valid 5-of-6 certificates; catch-up produced
  byte-identical exact-six status/effects; both DvP legs and per-asset totals
  conserved atom-for-atom; graceful stop removed the store lock; validator-0
  restart replayed the accepted terminal tombstone. The release-profile soak
  then completed one cold plus 100 warm real-wallet swaps with 101/101 accepted
  EffectsQCs, exact-six audits and conservation checks;
- executable model: 3 tests passed, including the required bounded state-space
  checks and unsafe negative control;
- parser fuzzing: 256 canonical-codec mutations and 2,560 auxiliary-codec cases
  completed with zero invariant failures;
- node and RPC SDK all-target builds compile.
- browser wallet: all 198 JavaScript tests passed, including the frozen
  FastSwap conformance vector and malleable-order rejection. A Vite production
  build was not asserted because this checkout does not have the Vite binary
  installed; the dependency-free Node test suite is the recorded browser gate.

Explicit performance gates run separately:

```text
100 warm operations at b271bbcd: p50=1727ms p95=1782ms p99=1809ms
four-second-late sixth validator at b271bbcd: quorum settlement=2088ms
focused in-process warm settlement at b271bbcd: total=2085ms
six real TCP RPC processes, optimized head 5be15899: preview=20ms
  settlement=114ms; prepare=28ms decision=37ms effects=34ms; exact-six repair
  and restart green
real persistent wallet, optimized head 5be15899, 100 warm plus one cold:
  cold=210ms p50=125ms p95=133ms p99=134ms; p50 stages sign=4ms preview=7ms
  prepare=27ms decision=37ms effects=33ms; 101/101 accepted, exact-six,
  conserved
```

These are local six-validator service measurements, not WAN claims. They prove
the critical path returns at a verified 5-of-6 quorum and does not wait for the
slow sixth replica; the durable outbox repairs exact-six state afterward.

## Rollout boundary

This branch has not been pushed, deployed, or activated by this record. The
local implementation claim includes canonical activation, six-process startup
from nonempty state, read-only shadow preview, quorum settlement, exact-six
repair, conservation and restart. A live claim still requires a reproducible
release, one-node-at-a-time target-fleet rollout with state-aware rollback,
WAN no-money shadow measurements, controlled dust deposit/swap/cancel/exit/
redemption, and post-rollout W6/FastPay regression evidence. Until those gates
pass, W6 remains the default settlement path and `fastswap_v1` requires explicit
opt-in.

Reproduction command for the real-process gate:

```text
cargo test -p postfiat-node --test fastswap_local_six -- --ignored --nocapture
cargo test --release -p postfiat-node --test fastswap_local_six \
  fastswap_local_six_process_hundred_warm_wallet_operations_meet_gate \
  -- --ignored --nocapture
```

At this head, `crates/node/tests/fastswap_local_six.rs` has SHA-256
`d888696ec759d82ae87e42b3587fb53b3c288fe44fa8c73fd475b40b632c6c84`.
