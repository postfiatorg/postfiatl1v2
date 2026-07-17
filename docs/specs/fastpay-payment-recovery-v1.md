# FastPay Payment Recovery v1

**Status:** production implementation locally integrated; deployment and WAN latency gates pending
**Scope:** single-owner FastPay transfer and unwrap payments only
**Not in scope:** multi-owner atomic swaps or FastSwap DvP
**Model:** `crates/fastpay-prototype/src/cancellation_model.rs`

## 1. Problem and non-negotiable safety rule

The live FastPay payment path durably locks each `(object_id, version,
registry_id)` before a validator signs. A complete `n-f` certificate can later
consume the object on any validator. Deleting an abandoned lock locally is
therefore unsafe: a certificate assembled before deletion could arrive after a
new spend and consume the same value twice.

Recovery must produce one canonical outcome for every locked object version:

1. consume it using the one valid certified order; or
2. cancel that order domain and atomically advance the object version.

An old certificate must never apply after the cancel/version fence. Core
FastPay remains available; this protocol replaces the deliberately fail-closed
`owned_safe_unlock` placeholder.

## 2. Fault and timing model

For committee size `n`, `f = floor((n-1)/3)` and normal quorum `q = n-f`.
PostFiat uses `q=3` for `n=4` and `q=5` for `n=6`. Honest validators:

- persist a lock before signing a vote;
- never vote for two order digests for the same object version and lock domain;
- verify owner authorization and live state before locking;
- persist the complete certificate and resulting effect before signing an
  apply acknowledgement; and
- reveal any persisted complete certificate during ordered recovery.

The base chain provides deterministic height, BFT ordering, eventual liveness
after synchrony, and atomic state persistence. A recovery window must be long
enough for the base chain to make progress despite `f` unavailable validators.
Its maximum length is governance-bound and signed into the order domain.

Two `q` certificates intersect in more than `f` validators:

```text
2q - n > f
```

Therefore two conflicting complete certificates require an honest validator to
double-vote. Recovery halts if conflicting complete certificates are ever
observed; it must not choose one.

## 3. Why partial-vote recovery is forbidden

The initial executable model tested confirmation from `q-f` revealed votes and
failed. For `n=4`, two two-vote recovery sets can intersect only in the one
Byzantine validator. Both conflicting orders could then appear recoverable.

Consequently:

- individual votes may diagnose locks but have no recovery authority;
- `q-f`, majority, or local-time unlock thresholds are forbidden; and
- only a complete, signature-verified normal `q` certificate may confirm
  recovery.

## 4. Versioned signed domain

The v3 transfer and unwrap owner authorizations and every validator vote bind:

- schema and operation kind;
- chain ID and genesis hash;
- protocol version;
- committee epoch and exact registry root;
- every input object ID and expected version;
- one canonical order digest;
- one canonical lock ID;
- `valid_from_height`;
- `expires_at_height`; and
- `recovery_closes_at_height`.

The lock ID is a domain-separated hash of the operation kind, complete canonical
order semantics excluding the lock-ID field itself, committee identity, and
validity/recovery window. The owner and validator signatures then cover the
complete order including that derived lock ID. It is not caller-selected. All
lengths and heights are bounded before hashing, signing, allocation, or
persistence.

Legacy v2 orders remain byte-identical below the governed activation height.
At and above activation, sign/apply RPCs require v3; a v2 certificate cannot
enter the v3 recovery protocol.

## 5. Durable records

All records use canonical, versioned encodings and are state-root committed when
replicated. JSON may remain an RPC presentation format but is not the signed or
hashed canonical form.

### 5.1 Lock record

For every input, one record stores `(object_id, object_version, lock_id,
order_digest, committee_epoch, registry_root, expiry, recovery_close)`. The
compare-and-set and fsync complete before a vote is signed.

### 5.2 Complete certificate record

A validator receiving `owned_apply` verifies the owner signature, distinct
validator signatures, exact domain, quorum, expiry, and current object state.
Before acknowledging, one atomic storage transaction persists:

- canonical complete certificate and digest;
- every consumed input/version;
- output or unwrap effect;
- terminal `Confirmed` fence; and
- replay/idempotency indexes.

A crash may expose either the complete old state or complete new state, never an
effect without its certificate/fence. Repeating the same certificate is
idempotent; a different certificate for a terminal version fails closed.

### 5.3 Apply acknowledgement

Each acknowledgement signs `(domain, order_digest, certificate_digest,
validator_id, terminal_state_digest)`. A wallet reports FastPay finality only
after `q` distinct valid acknowledgements. Returning after the first apply is
not finality under a protocol that permits eventual cancellation.

This requirement closes the withheld-broker ambiguity: a broker that assembles
but never delivers a certificate has not finalized a payment. With `q` apply
acknowledgements, at least `q-f` honest validators durably retain the full
certificate and can reveal it during recovery.

An effect observed on fewer than `q` validators remains speculative. Each node
persists a bounded inverse journal and the complete certificate before writing
that effect. If a later quorum-certified ordered block omits a speculative
effect, the node rolls back its entire unanchored suffix in reverse application
order, then applies the block's canonical pre-state effects. The certificate is
retained for ordered recovery. This is safe because two `q` sets intersect in
honest validators: a `q`-acknowledged effect cannot be omitted by a valid block
certificate, while a sub-quorum effect was never product-final.

### 5.4 Recovery decision certificate

The ordered lane stores one terminal decision per input version:

```text
Confirmed(order_digest, certificate_digest)
Cancelled(lock_id)
```

It binds the ordered block height/root, decision transaction ID, prior version,
and next version. Multi-input decisions update every input and effect in one
ordered commit; partial fencing is invalid.

### 5.5 Ordered-block anchoring and offline catch-up

Every certified ordered block carries the exact canonical, lock-ID-sorted list
of previously unanchored consensusless effects applied before that block. The
proposal signature, every block vote, certificate ID, block hash and transaction
finality proof bind the list. An honest validator with a local unanchored effect
refuses an omission at proposal-vote time. A validator that missed the FastPay
apply verifies the retained certificate, reconstructs the effect, and atomically
commits it with the ordered block. Duplicate, reordered, substituted, oversized
or certificate-incomplete lists fail closed.

The list is capped at 64 effects. Admission backpressures new FastPay work when
the next-block window is full; it never creates an untransportable 65th effect.
History checkpoints count prior effects as anchored, so pruning does not cause
old effects to be reattached.

## 6. State machine

### 6.1 Open validity window

For `valid_from_height <= h <= expires_at_height`:

1. validate the signed owner envelope against current live state;
2. atomically reserve every input for the same lock ID;
3. persist before signing;
4. aggregate `q` distinct votes into a complete certificate;
5. submit the certificate to validators;
6. each validator atomically persists certificate, effect, and `Confirmed`;
7. return success only with `q` durable apply acknowledgements.

Votes or normal applies after expiry fail closed.

### 6.2 Recovery reveal window

For `expires_at_height < h < recovery_closes_at_height`, anyone may submit a
complete certificate to the ordered recovery lane. Validators also expose
bounded certificate retrieval by lock/certificate digest so any relayer can
recover persisted evidence. The ordered lane verifies the original committee
signatures against the historical registry root.

Partial votes do not change state. A full certificate is stored durably for the
decision transaction.

### 6.3 Decision boundary

At `h >= recovery_closes_at_height`:

- exactly one valid complete certificate revealed: atomically apply/confirm it;
- none revealed: atomically record `Cancelled`, release the lock, and advance
  every input version without changing value or owner; or
- conflicting full certificates: halt the recovery item and emit a critical
  fault because the committee fault assumption has been violated.

After either terminal decision, v3 admission rejects every vote, apply, reveal,
or replay for the prior object version except byte-identical idempotent reads.

### 6.4 Certified reconciliation of a sub-quorum effect

When an externally verified ordered-block certificate omits one or more local
unanchored effects, the node does not preserve a divergent local view. It:

1. verifies the ordered certificate before touching state;
2. loads the durable inverse record for every unanchored suffix effect;
3. rolls the suffix back in reverse application order, failing closed if any
   output, account delta, input position or certificate differs;
4. retains the complete certificates in the recovery journal;
5. replays the block-attached effects in canonical lock-ID order; and
6. commits the reconciled state and ordered block through the normal atomic
   ordered-commit journal.

Snapshots include the owned-lock state and speculative recovery journal. A
legacy snapshot that contains activated FastPay recovery state but lacks those
files is rejected.

## 7. Reconfiguration

Committee rotation cannot delete locks. The historical committee root remains
available for certificate verification until all domains from that epoch are
terminal. New orders use only the active committee. Rotation completes when:

1. admission under the old committee is fenced;
2. its maximum validity and recovery windows have elapsed;
3. every old lock is `Confirmed` or `Cancelled`; and
4. the terminal set is checkpointed in replicated state.

The next committee may spend only the resulting next object versions. Old
certificates remain cryptographically verifiable for audit but cannot mutate
the fenced state.

The signed `FastPayRecoveryGovernanceBootstrapV1` envelope is retained as the
wire-compatible governance carrier for both the initial policy bootstrap and
later committee-only updates. The replicated transition distinguishes them:

- the initial bootstrap requires an empty recovery state, exact equality
  between policy activation and committee start, and future activation;
- a rotation must preserve the byte-identical active policy and chain domain;
- its committee epoch must be exactly the prior epoch plus one;
- its admission start must be exactly one height after the prior committee's
  `new_orders_through_height`, and must still be in the future; and
- duplicate roots/epochs, overlaps, gaps, policy changes, and backdated updates
  reject without mutation.

The old committee is never removed by rotation. Ordered recovery looks up the
order's exact historical epoch/root, while new signing selects only the
height-active committee. This permits old locks to drain across the rotation
without allowing new admission under the retired epoch.

## 8. RPC and wallet contract

Required bounded RPCs:

- `owned_sign_v3` / `owned_unwrap_sign_v3`;
- `owned_apply_v3` / `owned_unwrap_apply_v3`, returning a signed durable ack;
- `owned_certificate(lock_id | certificate_digest)` for persisted retrieval;
- `owned_recovery_status(lock_id)`;
- `owned_recovery_reveal(certificate)` through normal ordered submission; and
- `owned_cancel(lock_id)` through normal ordered submission after the reveal
  boundary.

Wallets display `collecting votes`, `applying`, `finalized`, `recovery pending`,
`confirmed by recovery`, or `cancelled`. They must not display success from a
certificate alone or from fewer than `q` apply acknowledgements. Cancellation
refreshes the advanced object version and permits a new payment.

## 9. Production code map

Implemented real boundaries:

- types: `crates/types/src/account_owned_asset_types.rs`;
- owner/vote canonical bytes and deterministic effects:
  `crates/execution/src/owned_transfer.rs`;
- lock WAL and legacy fail-closed unlock:
  `crates/node/src/consensus_artifacts.rs`;
- v3 sign/apply, durable acknowledgement, certificate retrieval, recovery
  status and speculative rollback journal:
  `crates/node/src/fastpay_recovery_node.rs`;
- ordered recovery transaction admission/execution and state-root commitment:
  node execution, `crates/types/src/market_nav_asset_types.rs`, and
  `crates/node/src/state_commitment.rs`;
- block anchoring, replay and certified catch-up:
  `crates/node/src/block_replay_wallet.rs`, `crates/node/src/block_finality.rs`,
  `crates/node/src/consensus_artifacts.rs` and ordered batch apply paths;
- snapshot/replay/migration: `crates/node/src/batch_snapshot.rs` and storage;
- routing and durable certificate replication: `wallet-proxy/`;
- local signing, quorum acknowledgements, recovery UX: `wallet-web/src/lib/`
  and wallet components.

The executable model remains a design oracle; production closure additionally
requires the real node, wallet, replay, snapshot and six-validator boundaries.

## 10. Acceptance tests

Every test runs for `n=4` and `n=6` where meaningful:

1. normal transfer and unwrap finalize with `q` distinct votes and `q` durable
   apply acknowledgements;
2. duplicate validator IDs, wrong domain, stale version, bad owner signature,
   and under-quorum evidence produce no mutation;
3. two through `q-1` partial locks cancel at the bounded decision height;
4. a complete certificate revealed during recovery confirms;
5. delayed votes, applies, and reveals reject after their exact boundaries;
6. an old certificate cannot apply after cancel/version advance;
7. withheld broker without apply quorum is never displayed as final; a minority
   apply rolls back on a quorum-certified omission while retaining its recovery
   certificate, and bounded recovery then confirms or cancels safely;
8. partition and restart retain votes, full certificates, effects, and fences;
9. crash injection at every lock/certificate/effect/fence persistence boundary
   produces only the prior or next atomic state;
10. committee rotation drains all old domains and rejects cross-epoch replay;
11. Byzantine equivocation cannot form two full certificates within `f`;
12. conflicting full certificates halt rather than selecting an outcome;
13. multi-input cancellation and confirmation are atomic;
14. snapshot restore, catch-up, block replay, and state-root comparison match;
15. wallet funding, send, and unwrap recover from abandoned locks without
    exporting custody or disabling FastPay; and
16. the six-node WAN warm-latency envelope is measured and does not regress.

## 11. Safety verdict

**GO for continued production hardening, not for deployment yet.**
The full-certificate consume-or-cancel fence closes the late-certificate race
under the stated BFT and eventual-synchrony assumptions. Partial-vote recovery
is a **NO-GO**. The production atomic storage, ordered decision path, block
anchoring, transfer and unwrap minority rollback, snapshot/catch-up behavior,
and wallet quorum semantics are locally implemented. Governed committee
rotation also preserves old-epoch recovery and rejects overlap atomically.
The persistence-boundary crash matrix is green for lock WAL, journal-before-
ledger, effect-before-acknowledgement, every ordered rollback write prefix,
snapshot, and replay. Deployment remains blocked until the immutable-candidate
suites and six-node WAN correctness/latency gates are evidence-green.
