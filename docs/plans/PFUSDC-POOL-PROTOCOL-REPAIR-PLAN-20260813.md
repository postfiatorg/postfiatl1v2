# pfUSDC Pool Protocol Repair Plan

**Date:** 2026-08-13

**Status:** implementation in progress; consensus changes implemented locally and under test; no live activation or fund movement performed

**Severity:** P0 — live USDC accepted by the governed Ethereum vault, corresponding pfUSDC claim not executable by the deployed validator binary

**Affected route:** `ethereum-mainnet-usdc-v1`, epoch-6 successor vault

**Affected PFTL asset:** `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b`

**Recovery deposit:** `15.000000 USDC`, Ethereum transaction `0x7d91722c2e8071827a5d06144d30b2612af319fd9b0d76687bbb014c9ebc6364`

## 1. Objective

Make pfUSDC behave exactly like the source-labeled counted-cash system described in the local PostFiat.org article, while preserving deterministic replay, supply conservation, source-bucket isolation, and the user's finalized 15-USDC deposit.

This is not a plan to:

- disable pfUSDC and call that a fix;
- bypass the global supply invariant;
- manually edit validator JSON state;
- raise `circulating_supply` by operator assertion;
- mint against an RPC response, multisig, or unproven balance;
- silently use new epoch-6 backing to make an impaired legacy bucket whole;
- delete or rewrite the user's finalized Ethereum deposit;
- create a second Ethereum deposit or request another wallet approval.

The terminal result is a working protocol:

```text
finalized external USDC deposit
  -> source-bound proof
  -> source-labeled ReserveReceipt
  -> counted value
  -> one allocation
  -> exact pfUSDC issuance
  -> source-specific redemption
```

## 2. Normative product and accounting specification

For this repair, the user has designated the following local PostFiat.org article as the normative product model:

- `/home/postfiat/repos/postfiatorg.github.io/content/blog/pfusdc.md`
  - mental model and flow: lines 54-107;
  - counted mint capacity: lines 238-259;
  - `ReserveReceipt` fields and states: lines 338-374;
  - allocation ledger: lines 376-387;
  - canonical source-bucket invariant: lines 389-406;
  - reserve packets and replay checks: lines 419 onward;
  - source-domain isolation and source-specific redemption: `Source domains in practice` and `Redemption, impairment, and loss allocation`;
  - impairment procedure and recapitalization rules: `Impairment process` and `Recapitalization`.

The public bridge behavior is additionally defined by:

- `/home/postfiat/repos/postfiatorg.github.io/content/blog/pfusdc-trustless-bridge.md`
  - ingress: PFTL verifies the finalized source proof and mints exactly the deposited amount;
  - egress: a finalized PFTL burn proof releases exactly the corresponding USDC;
  - holder claim: every circulating pfUSDC corresponds to USDC locked in an auditable vault;
  - no operator-set balance or signer downgrade path.

The first article currently labels itself a research proposal. This repair must convert its rules into a versioned consensus specification and conformance suite. The implementation must not treat prose as an excuse for unspecified state transitions.

### 2.1 Exact article-to-protocol mapping

| Blog primitive | Required protocol object or transition | Acceptance rule |
|---|---|---|
| External dollar claim | Exact chain, vault, token, depositor, recipient, amount, transaction, block and route binding | All fields are proof-bound and replay-protected. |
| `ReserveReceipt` | `VaultBridgeReceipt` plus immutable source-domain and policy identity | One finalized source event creates at most one receipt. |
| Finality | `Pending -> Finalized` deposit evidence | No receipt is counted and no pfUSDC is issued before the configured finality proof verifies. |
| Haircut | Integer `counted_value_atoms = floor(amount * (10000-haircut_bps)/10000)` | No floating point; rounding always reduces issuance. Epoch-6 direct USDC is presently 0 bps only if the governed policy explicitly says so. |
| Counted value | Active source-bucket capacity | Counted value changes only through a proof-bound receipt, impairment packet, retirement, or proof-bound recapitalization receipt. |
| pfUSDC mint capacity | Unallocated counted value for the exact source bucket | Mint cannot exceed the receipt's available counted value. |
| Allocation ledger | Immutable allocation keyed by receipt, bucket, purpose and consumer | Capacity is allocated once; replay creates no second allocation. |
| Source bucket | Route/vault/token/policy-specific bucket | `allocated_atoms[bucket] <= counted_cash_atoms[bucket]`. |
| Transferable pfUSDC | A source-preserving claim series, not an unqualified pooled liability | Transfers cannot erase source identity. |
| Redemption | Burn against the same source series and bucket | A weak or impaired bucket cannot drain a stronger bucket. |
| Impairment | Versioned impairment packet and deterministic factor | No full-value wallet display or redemption promise after a bucket is impaired below par. |
| Recapitalization | New source-labeled, proof-backed receipt | Governance cannot restore par by changing a number. |
| Reserve packet | Hash-bound replay packet covering receipt, supply, allocation, policy and source roots | Independent replay reconstructs the same totals and state root. |

## 3. Incident facts that the plan must preserve

### 3.1 User deposit

```text
Ethereum depositor: 0x0c30d0a57f4f9bc035ca8e8be6bd2abae054b882
PFTL recipient:      pfab9b9228942e5c529633a13aa271d5297bec6353
Amount:              15.000000 USDC
Ethereum tx:         0x7d91722c2e8071827a5d06144d30b2612af319fd9b0d76687bbb014c9ebc6364
Deposit ID:          0xa0ce20f2cce4131b43dfa0108856e731a0be3c232f627a061d4e09ccf9f64266
Successor vault:     0x4939a45caa85da31fb26d7dbe6477b45f7f08688
Route policy:        f088876e4bc7f611fdf7199237f241a1bb91ffc1850a8b65cd50a4852cab2ec40a2fae18c6dbf0ee5dd4934b22107f1a
PFTL proposal:       finalized at height 902
pfUSDC claim:        not accepted
```

The SP1 ingress proof already exists and is bound to this exact deposit. Recovery must reuse it after independently re-verifying its hashes and bindings.

### 3.2 Live supply and pool state observed across all six validators

The initial incident query reported the following values, but omitted a live
custody lane:

```text
global pfUSDC supply, including Asset-Orchard: 287.534134
Asset-Orchard live pfUSDC:                     20.000000
transparent/FastLane/external custody supply: 267.534134
finalized circulating-supply checkpoint:      297.933789
active counted source-bucket value:            132.601562
```

The frozen height-902 snapshot replay found another `2.166461 pfUSDC` in
`PftlUniswapRouteV2State.non_nav_spread_atoms`. That field is real settlement
asset custody under the blog's complete-inventory rule. The corrected exact
inventory is therefore:

```text
complete non-Orchard custody supply:           269.700595
Asset-Orchard live pfUSDC:                      20.000000
global live pfUSDC supply:                     289.700595
finalized circulating-supply checkpoint:      297.933789
```

The successor claim would produce:

```text
289.700595 existing global supply
+15.000000 exact proof-backed claim
-----------
304.700595 correct post-claim global supply
```

### 3.3 Deployed binary defect

The live fleet inspection on 2026-08-13 found that the validators actually
run (superseding the stale deployment identity originally recorded here):

```text
deployment:    a666-nav-overlay-cbbf53e
git revision:  cbbf53ec0415754562c3cb4f4a469a95a80a8298
binary SHA-256: 983dcc11784c80bca937c55de6945bf40613aee239006779151f210401c1f95d
```

The failing path simulates a transparent bridge claim without the complete
Asset-Orchard-aware inventory and the deployed supply reporter also omits
`non_nav_spread_atoms`. It therefore undercounts the pre-claim state. The
repair must use the complete `289.700595` inventory in admission, execution,
replay, and status output; matching the old undercount is not an acceptance
criterion.

The original reduced calculation was:

```text
267.534134 non-Orchard supply
+15.000000 claim
-----------
282.534134
```

Because that reduced value is below the checkpoint, the old execution path
incorrectly decides that no proof-bounded checkpoint growth is needed. The
final complete invariant then rejects the claim. With all known custody lanes,
the required terminal value is `304.700595`.

```text
269.700595 + 20.000000 + 15.000000 = 304.700595 > 297.933789
```

The source tree contains the intended Orchard-aware semantics in commit
`16621fa94a4a29c637574180800777fa4ed0e1b5` and the required non-NAV-spread
inventory correction in `83ac75d`. Both are required for complete accounting;
discarding the latter merely to reproduce the stale incident number would
violate Section 4.1.

### 3.4 Existing pool-model defect

The live asset is a pooled ticker spanning four source buckets. One legacy Ethereum vault bucket is impaired:

```text
legacy vault:                 0xaaa78fda7062efce769e95cd72fc55e507bc8183
bucket status:                impaired
impairment factor:            0 bps
counted value:                0.000000
outstanding bucket amount:    185.098533
redemption queue:               9.932863
```

The wallet nevertheless presents a single fungible `pfUSDC` balance at full face value and does not disclose source composition or impairment. This contradicts the blog's source-label, bucket-isolation, source-specific redemption, impairment, and loss-allocation model.

The reported aggregate `active counted source-bucket value` is also below global issued supply. Before any claim that the pool is fully backed, a replay must explain every atom through active backing, impaired claims, redemptions, burns, private custody, and any recoverable but not-yet-counted vault assets.

## 4. Non-negotiable invariants

The repaired implementation must enforce all of the following in consensus and in the independent replay verifier.

### 4.1 Global conservation

```text
global_live_pfusdc =
    transparent_trustline_supply
  + escrow_custody
  + FastPay_owned_supply
  + external_bridge_custody
  + AssetOrchard_live_supply
```

Every custody lane is counted exactly once. Adding a new replicated custody lane must fail compilation until classified.

### 4.2 Source-bucket conservation

For every source bucket `b`:

```text
counted_cash[b] = active_counted_receipts[b] - retired_counted_value[b]

allocated[b] =
    outstanding_source_series[b]
  + NAV_subscription_allocations[b]
  + redemption_queue[b]
  + other_protocol_allocations[b]

allocated[b] <= counted_cash[b]
```

An impaired bucket uses its impairment factor in both wallet valuation and redeemable claims. It may not silently borrow another bucket's cash.

### 4.3 Atomic ingress

The following either all commit in one deterministic transition or none commit:

1. consume the finalized, unclaimed deposit nullifier;
2. create/count the exact `ReserveReceipt`;
3. grow proof-bounded capacity if required;
4. create one supply allocation;
5. credit exactly the claim amount to the bound recipient;
6. update source-bucket totals;
7. update the reserve packet/checkpoint root.

No intermediate state may expose minted pfUSDC without allocation or counted backing.

### 4.4 Replay and route binding

- The same deposit, proof, nullifier, receipt or allocation cannot be consumed twice.
- The source chain, vault, token, route epoch, policy hash, program vkey, amount, depositor and PFTL recipient are immutable inputs.
- Historical blocks replay with historical semantics.
- The Orchard-aware rule activates only at an explicit governed activation height or protocol version.
- Six validators applying the same transition produce byte-identical state roots.

### 4.5 No manual solvency

- A direct `circulating_supply` edit is forbidden.
- A governance vote without a source-labeled receipt cannot add counted value.
- Recapitalization enters through the same proof and receipt machinery as other backing.
- A migration receipt identifies both the retired source and the replacement source; it cannot create net counted value without proven assets.

## 5. Work plan

## Phase 0 — Freeze evidence, not the protocol

Goal: establish a reproducible incident corpus before changing consensus behavior.

Tasks:

1. Export signed finalized-checkpoint snapshots from all six validators at the same height.
2. Record binary, topology, manifest, route profile, proof program and state-root hashes.
3. Export the complete pfUSDC inventory from every custody lane.
4. Export all `VaultBridgeReceipt`, bucket, allocation, redemption and deposit records for the pfUSDC asset.
5. Preserve the recovery job, Ethereum receipt, block, SP1 witness, public values and proof artifacts.
6. Verify that the six snapshots independently produce identical inventory and source roots.
7. Create a machine-readable incident manifest containing hashes only; never copy wallet seeds or signing keys into evidence.

This phase does not stop transfers, burns, withdrawals or consensus. It only creates a stable forensic input.

Gate P0:

- six matching finalized state roots;
- six matching pfUSDC inventories;
- exact recovery-deposit proof bindings verified;
- no direct state mutation performed.

## Phase 1 — Turn the blog model into an executable specification

Goal: remove ambiguity between marketing prose, pool accounting and consensus code.

Tasks:

1. Add a versioned `pfusdc-source-labeled-pool-v1` protocol specification derived line-by-line from `content/blog/pfusdc.md`.
2. Define canonical integer formulas for counted value, allocations, impairment factor, redemption and recapitalization.
3. Define source-series identity:

   ```text
   source_series_id = H(
     pftl_chain_id,
     pfusdc_asset_family,
     source_chain_id,
     vault,
     token,
     route_epoch,
     policy_hash
   )
   ```

4. Define `pfUSDC` as a wallet display family, not permission to merge consensus claims. Each transferable on-chain series preserves `source_series_id`.
5. Define the legacy pooled asset as an explicit `legacy-pooled` series until reconciled; do not mislabel it as successor-vault pfUSDC.
6. Define reserve replay input/output JSON schemas and canonical serialization.
7. Add conformance vectors for healthy, haircut, partial allocation, transfer, redemption, impairment, recapitalization, replay and overflow cases.

Gate P1:

- every row in Section 2.1 has a spec rule and a conformance vector;
- no float, wall-clock, unordered iteration or unbounded input affects state;
- spec and implementation PRs are linked and reviewed together.

## Phase 2 — Build an independent pool reconciliation report

Goal: determine the exact economic state before choosing a migration transaction.

Tasks:

1. Implement/read an offline replay tool over the Phase-0 snapshot.
2. Reconstruct, atom by atom:
   - transparent trustlines;
   - escrows and offers;
   - FastPay owned objects and reserves;
   - Asset-Orchard ingress, egress and live totals;
   - bridge receipts and allocations;
   - burns and redemption queues;
   - active, impaired and retired buckets.
3. Query each historical Ethereum vault read-only and bind its live USDC balance, deposits and withdrawals to finalized block evidence.
4. Classify every difference between issued supply and active counted backing as exactly one of:
   - proven active backing;
   - proven recoverable legacy backing awaiting migration;
   - pending finalized deposit awaiting claim;
   - redemption liability;
   - impaired claim;
   - accounting defect requiring a deterministic correction;
   - unexplained and therefore blocking.
5. Produce the equations per bucket and in aggregate.
6. Have a second implementation replay the same snapshot and compare the report hash.

Gate P2:

- every atom is classified;
- no negative or duplicate allocation;
- report hash matches across two implementations and all six snapshots;
- unresolved amount equals zero before claiming full backing.

## Phase 3 — Repair Orchard-aware consensus execution

Goal: make proof-backed cap growth use the complete pre-claim supply.

Tasks:

1. Apply the Orchard-aware bridge-claim semantics from `16621fa` with an
   explicit governed activation height on the replay-verified height-902
   fleet lineage.
2. Include the `83ac75d` non-NAV-spread custody correction because the blog's
   global inventory requires it; exclude unrelated pNOK behavior.
3. Pass the same immutable Orchard inventory into all relevant paths:
   - mempool admission;
   - pending-transaction prefix simulation;
   - transparent batch construction;
   - block application;
   - historical replay above activation;
   - snapshot/recovery validation.
4. Compute:

   ```text
   current_global_supply = issued_asset_supply(all non-Orchard lanes)
                         + AssetOrchard.live_total

   post_claim_supply = current_global_supply + exact_claim_amount
   ```

5. Permit checkpoint growth only up to:

   ```text
   prior_checkpoint
   + finalized_unclaimed_backing_for_the_exact_route
   ```

6. Atomically set the checkpoint to `post_claim_supply` only when the exact finalized deposit supplies the needed capacity.
7. Replace the misleading external `growing_backed_cap` no-op with one of:
   - a read-only stage that proves the atomic claim will grow the checkpoint; or
   - removal from the worker stage vocabulary.
   It must never claim success merely because a deposit record is finalized.
8. Add structured terminal and retryable error codes. A deterministic invariant rejection is not an endless retry.

Required tests:

- exact live regression: `269.700595` complete non-Orchard + `20.000000` Orchard + `15.000000` claim;
- claim grows checkpoint from `297.933789` to `304.700595`;
- claim fails if finalized unclaimed route backing is `14.999999`;
- claim succeeds with exactly `15.000000` and consumes capacity once;
- replay fails without changing state;
- wrong vault, epoch, policy, recipient, amount, proof or nullifier fails;
- six-replica application produces identical receipt, state root and checkpoint hash;
- activation-minus-one uses historical semantics and activation-height uses Orchard-aware semantics;
- full historical replay retains every pre-activation state root.

Gate P3:

- focused Rust tests pass;
- workspace tests pass;
- deterministic replay corpus passes;
- no historical state-root drift;
- candidate binary is reproducibly built, signed and hash-pinned.

## Phase 4 — Reconcile the legacy pooled asset without inventing backing

Goal: satisfy the blog's source-bucket and impairment rules before presenting pfUSDC as uniformly redeemable at par.

The Phase-2 report determines which of the following proof-backed actions applies to each legacy atom.

### 4.1 Recoverable legacy backing

If USDC still exists in a retired or dead vault and can be moved under its existing contract rules:

1. prove the old vault balance and withdrawal;
2. deposit the recovered USDC into the approved successor source;
3. prove the successor deposit;
4. create a migration/recapitalization receipt referencing both evidence chains;
5. count the replacement receipt only after the old counted value is retired;
6. prove no interval or final state double-counts both sources.

### 4.2 Missing or impaired backing

If backing is not recoverable, use one of the blog-authorized outcomes:

- a real recapitalization deposit entering through a source-labeled receipt; or
- a deterministic impairment packet and pro-rata claim factor.

Governance may choose between those economic outcomes, but it may not restore par by changing `circulating_supply` or a bucket number.

### 4.3 Source-preserving migration

1. Introduce explicit source-series identity for new issuance.
2. Keep the current fungible asset visibly labeled `legacy pooled pfUSDC` until its backing is reconciled.
3. Migrate legacy balances 1:1 only to the extent the reconciliation report proves full backing.
4. If an impairment remains, migrate at the deterministic factor from the impairment packet.
5. Update NAV markets to accept only governance-whitelisted source series and to disclose the accepted backing source.
6. Make redemption burn the same source series that owns the backing bucket.

Gate P4:

- `allocated[b] <= counted_cash[b]` for every healthy bucket;
- impaired claims carry an explicit factor and cannot redeem at undisclosed par;
- aggregate wallet valuation equals the sum of source-series valuations;
- old and successor backing are never counted simultaneously;
- a replay verifier reconstructs the post-migration root from Phase-0 evidence plus certified migration blocks.

## Phase 5 — Make wallet preflight simulate the terminal claim

Goal: the wallet must never accept an Ethereum deposit merely because RPCs, the prover and the vault are reachable.

Add a read-only, quorum-checked `pfusdc_ingress_quote/preflight` result containing:

```text
route identity and activation
connected depositor and PFTL recipient
amount atoms
current global supply by custody lane
current source-series counted and allocated values
finalized-unclaimed backing for the exact route
post-claim global supply
post-claim source-bucket allocation
checkpoint before and after
whether the exact claim transition is executable
blocking consensus code and human explanation
six-validator state-root agreement
quote height and bounded expiry height
```

Rules:

- The preflight executes the real transition against a read-only snapshot; it does not duplicate the rules in JavaScript.
- The quote is hash-bound into the deposit request and rechecked immediately before MetaMask approval and deposit.
- A changed state invalidates the quote and requests a refresh before funds move.
- The wallet cannot show `Ready` unless the terminal claim simulation passes on a quorum and all route/proof bindings match.
- The wallet distinguishes `Ethereum finalized`, `proof complete`, `PFTL deposit finalized`, `claim rejected`, and `pfUSDC issued`.
- The UI displays exact transaction hash, amount, elapsed time, retry count and error code.
- A deterministic rejection stops retries and gives a recovery action; only transient infrastructure failures retry.
- The connected Ethereum depositor must match automatic job recovery.

Gate P5:

- the current broken deployed binary produces `BLOCKED` before approval;
- the repaired candidate produces `READY` for the same 15-USDC evidence on a snapshot;
- no fake `verifying` spinner after deterministic rejection;
- browser refresh/restart resumes the correct job without cross-account history.

## Phase 6 — Activation-safe fleet rollout

Goal: deploy the repair without mixed execution rules or historical replay drift.

Tasks:

1. Cut a minimal release from the verified deployed `cbbf53ec` lineage.
2. Add an explicit activation height/protocol feature gate for Orchard-aware bridge claims and source-series migration operations.
3. Build reproducibly and publish binary, source revision, build inputs and SHA-256.
4. Export and verify signed pre-rollout snapshots from every validator.
5. Rehearse the upgrade on six cloned validator data directories.
6. Replay from genesis/checkpoint through the activation boundary and compare state roots at every height.
7. Inject one bad validator binary and prove honest validators reject its divergent proposal.
8. Stage the same candidate on all six validators without activating new semantics.
9. Verify manifests and runtime hashes on all six hosts.
10. Activate at the governed height only after all six report readiness.
11. Observe multiple empty and ordinary transaction rounds before submitting migration or recovery operations.

Rollback:

- Before activation: return to the old binary using verified snapshots and manifests; no state transition has changed.
- After activation but before a new-rule block: coordinated binary rollback remains possible.
- After any new-rule block finalizes: do not restore an old snapshot or rewrite history. Roll forward with another versioned transition.
- Never run mixed old/new semantics at the same activation height.

Gate P6:

- all six nodes run the identical signed binary;
- all six remain height/root converged;
- activation-boundary replay is byte-identical;
- no custody or supply mutation occurs during binary staging.

## Phase 7 — Recover the finalized 15-USDC deposit through the repaired protocol

Goal: complete the user's existing deposit without a second Ethereum transaction.

Preconditions:

- P0-P6 gates pass;
- the pool reconciliation/migration state satisfies the source-bucket invariants;
- the existing proof is reverified against the activated route and candidate program;
- the claim dry-run succeeds identically on all six validators;
- expected recipient balance delta is exactly `+15.000000 pfUSDC` in the correct source series.

Execution:

1. Reuse the existing deposit ID and SP1 proof.
2. Submit the idempotent claim once through certified consensus.
3. Confirm one `ReserveReceipt`, one supply allocation and one recipient credit.
4. Confirm the deposit nullifier/consumer ID cannot be replayed.
5. Confirm all six validators finalize the same height and state root.
6. Confirm the source bucket gained exactly 15 counted atoms-per-USDC units and 15 allocated units, subject only to an explicitly governed haircut.
7. Confirm no other holder, bucket, NAV asset, vault or wallet balance changed.

Expected user result if the epoch-6 policy remains a proven 0-bps haircut:

```text
legacy pooled pfUSDC before/after: 73.097570 / 73.097570
epoch-6 source-series claim delta:  15.000000
wallet family total after:          88.097570
```

The `15.000000` must be a separate epoch-6 source-series balance. Combining it
into the legacy asset identifier would erase the backing source and recreate
the defect this repair is intended to remove.

Gate P7:

- accepted consensus receipt;
- exact balance delta;
- exact receipt/allocation delta;
- replay rejected;
- six-validator convergence;
- no new MetaMask approval or Ethereum deposit.

## Phase 8 — End-to-end acceptance

Run a new, bounded live round trip only after the stuck deposit is recovered and independently reconciled:

```text
1.000000 Ethereum USDC
  -> exact source-series pfUSDC
  -> PFTL transfer, including one private/Orchard custody transition
  -> same source-series redemption burn
  -> 1.000000 Ethereum USDC before gas
```

Acceptance evidence must prove:

- exact ingress receipt and allocation;
- Orchard custody changes total location, not global supply;
- source identity survives transparent/private transitions;
- exact burn and source-specific redemption;
- terminal conservation to the atom;
- no committee or operator balance assertion;
- wallet displays the actual source, backing, impairment status and transaction stage.

## 6. Required code surfaces

At minimum, the implementation review must cover:

- `crates/execution/src/nav_vault_asset_execution.rs`
  - complete global-supply calculation;
  - proof-bounded cap checkpoint;
  - receipt, bucket and allocation atomicity.
- `crates/execution/src/entrypoints.rs`
  - Orchard-aware execution entry points and activation semantics.
- `crates/execution/src/nft_escrow_asset_execution.rs`
  - propagation of immutable Orchard inventory.
- `crates/node/src/mempool_proposals.rs`
  - admission and batch simulation using complete custody state.
- `crates/node/src/batch_snapshot.rs`
  - snapshot-bound execution input.
- `crates/node/src/execution_actions.rs`
  - block execution/replay parity.
- `crates/node/src/state_commitment.rs`
  - exhaustive global supply inventory and deterministic invariant.
- `crates/node/src/market_bridge.rs`
  - source-bucket and aggregate status accuracy.
- `crates/node/src/vault_bridge_workflows.rs`
  - bundle construction, replay reports and source-specific redemption.
- `scripts/a666-wallet-eth-bridge-stage-serialized.py`
  - truthful stages and exact route binding.
- `scripts/a666-mainnet-pfusdc-relay.sh`
  - no historical route defaults in a current wallet job.
- `wallet-proxy/trustless-bridge-jobs.js`
  - durable retry classification and terminal error exposure.
- `wallet-web/src/components/Bridge.jsx`
  - terminal preflight, state display and account-scoped recovery.

## 7. Test and evidence matrix

| Layer | Required evidence |
|---|---|
| Unit | All formulas, overflow edges, source binding, allocation replay, impairment and exact Orchard-aware regression. |
| Property | For generated deposits/transfers/private moves/burns: global supply conservation and per-bucket allocation never exceeds counted value. |
| State transition corpus | Versioned input state + operation -> exact output state/receipt/root, including activation boundary. |
| Historical replay | Every existing block replays to the archived state root under pre-activation rules. |
| Snapshot migration | Six copies of the live finalized snapshot produce the same migration report and post-transition root. |
| Adversarial | Wrong proof, amount, route, vault, token, policy, epoch, depositor, recipient, nullifier, bucket and impairment factor all fail closed. |
| Crash recovery | Kill worker/node before and after each durable boundary; resume without duplicate receipt, allocation, claim or signature. |
| Six-validator | One proposal, quorum certification, full convergence and laggard catch-up. |
| Wallet | No deposit approval unless terminal claim simulation passes; precise progress and terminal errors. |
| Live funds | Existing 15-USDC recovery, then one bounded 1-USDC round trip with atom conservation. |

## 8. Definition of done

The repair is complete only when all of the following are true:

- The blog's `ReserveReceipt -> counted value -> allocation -> pfUSDC` ordering is enforced in consensus.
- Every live pfUSDC atom is included in global supply exactly once.
- Every healthy source bucket satisfies `allocated <= counted`.
- Every impaired claim has an explicit, enforced loss factor or verified recapitalization.
- New issuance preserves source identity and redemption uses the same source.
- The deployed fleet uses Orchard-aware claim accounting at a versioned activation height.
- Historical replay is unchanged below activation.
- The wallet preflight simulates the terminal claim before asking MetaMask to transfer USDC.
- The user's existing 15-USDC deposit mints exactly 15 source-correct pfUSDC without another Ethereum transaction.
- The claim cannot replay.
- All six validators converge on the same final state root.
- An independent reserve replay report explains every atom and matches the wallet's displayed backing and valuation.
- The bridge can complete a fresh bounded ingress and egress round trip without operator balance edits, manual cap changes, or hidden source socialization.

Anything less is partial remediation, not a working pfUSDC pool.

## 9. Implementation record (2026-08-13)

The deployed release now implements the following parts of this plan:

- explicit governed activation heights for Orchard-aware bridge claims and
  pfUSDC source-series enforcement;
- canonical source identity derived from chain, asset family, source chain,
  vault, token, route epoch, and policy hash;
- atomic ingress into a newly created/validated source-series asset, with the
  claim bound to the governed route epoch;
- family-wide supply accounting across legacy transparent custody,
  source-series custody, Asset Orchard, FastPay, escrows, and PFTL-Uniswap
  custody lanes;
- deterministic reconciliation of stale historical supply allocations to
  bucket outstanding supply plus redemption queues;
- source-specific NAV subscription, NAV redemption return, and bridge
  burn-to-redeem transitions;
- node-native exact ingress simulation and quorum-bound wallet preflight;
- terminal job error classification and removal of the false
  `growing_backed_cap` stage;
- wallet valuation and labels that exclude legacy pooled pfUSDC from a false
  one-dollar value and disclose source-backed versus legacy balances;
- a multi-chain conservation audit and the height-902 classification report.

Current test evidence:

- `postfiat-types`: 122 passed;
- `postfiat-execution`: 189 passed;
- focused six-replica source-series claim/burn test: passed;
- wallet proxy regression files: 35 passed;
- wallet browser library tests: 262 passed;
- wallet production build: passed;
- full node library suite: 267 passed, zero failed, two ignored;
- the exact ingress preflight is exposed through the public read-only RPC
  allowlist, so the wallet can query it without validator administration
  credentials;
- the durable epoch-6 relay rebuilds only the PFTL operations of an old queued
  job with route epoch 6. It reuses the preserved finalized Ethereum proof and
  never repeats the Ethereum deposit;
- wallet family balances aggregate the legacy asset and all source-series
  balances while retaining their distinct backing and valuation labels.

No new Ethereum contract is required for the active epoch-6 route or the
existing finalized 15-USDC deposit. The live requirements are a new validator
binary plus the two governed future-height activations. The immutable epoch-5
vault cannot be repaired or drained by deploying a replacement contract; its
claims remain explicitly impaired unless a valid old-vault proof path or real
recapitalization is supplied.

Live deployment and recovery completed on 2026-08-13:

- all six validators run release `pfusdc-pool-repair-8a62cf9` with binary
  SHA-256 `e6b31e715a025170747b4222f4afd703e0d9a4e7fe7f6ac998715848905d0ec5`;
- Orchard-aware claims and source-series issuance activated at height `906`;
- the existing 15.000000-USDC deposit was claimed at height `906` without a
  second Ethereum transaction;
- the recipient received exactly 15.000000 source-series pfUSDC while its
  73.097570 legacy pooled balance remained unchanged;
- all six validators converged at height `906` on state root
  `d62ec7003ed36a90c767cbf6cf306184258104b5b0d2a72fe2bdb78f90e7f0c018042398cbfd319824ddb40d724d2cce`;
- a replay admission attempt was rejected and left height, root, and mempool
  unchanged;
- the wallet relay is healthy and reports the durable job as accepted.

The complete recovery record is in
`docs/evidence/pfusdc-pool-repair-20260813/LIVE-RECOVERY-REPORT.md`. The fresh
Phase-8 1.000000-USDC user-authorized ingress/private-custody/egress loop is a
separate remaining acceptance item and is not falsely claimed by the recovered
15-USDC ingress.
