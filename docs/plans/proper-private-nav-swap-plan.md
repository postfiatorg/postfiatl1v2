# Proper Private NAV Swap Completion Plan

Date: 2026-06-22
Repo: `postfiatl1v2`
Scope: Asset-Orchard `pfUSDC <-> a651` private swap completion for a user-facing StakeHub flow.

## Top-Line Status

What exists today:

- Public `AssetOrchardIngressV1`: a public PFTL issued-asset balance, such as
  `pfUSDC` or `a651`, can be burned into an Asset-Orchard shielded note. This
  ingress boundary reveals the asset and amount.
- Private Asset-Orchard middle: the implemented Halo2 swap circuit can consume
  private asset-typed notes and swap `pfUSDC <-> a651` without revealing raw
  asset ids, values, owners, recipients, or bilateral price in the public swap
  action.
- Disclosed `AssetOrchardEgressV1`: a shielded typed note can be exited back to
  a public issued-asset balance, but the current egress reveals the note
  opening, asset id, amount, destination account, nullifier, and validation
  material. It is functional bridge-out/recovery plumbing, not private
  cash-out.

What private egress is:

Private egress is the missing user-facing cash-out primitive. Its goal is to
replace disclosed egress with a proof that the user owns and nullifies a valid
unspent typed note, and that the public exit output is correct, without
revealing the note opening. A direct private egress may still reveal public exit
destination and amount; batching, delay, relayers, and fixed denominations are
additional privacy layers that reduce timing and amount linkability.

Until private egress exists, the current system shape that needs to be
improved is:

```text
public ingress -> private Asset-Orchard swap middle -> disclosed egress
```

## Source Context

This plan is based on the current L1 docs and the research articles now under
`postfiatorg.github.io/content/research`:

- `content/research/heavy-zk-optimization-v2.md`
- `content/research/private-nav-otc-swaps.md`
- `docs/status/shielded-layer-map.md`
- `docs/specs/asset-orchard-swap-circuit-design-v2.md`
- `docs/specs/private-otc-shielded-scope.md`
- `docs/status/zk-prover-optimization-results.md`
- `docs/runbooks/private-nav-otc-shielded-swap-wan-devnet.md`
- `docs/runbooks/wan-devnet-full-live-end-to-end-run.md`

## Current State

The private middle exists. The current `AssetOrchardSwapCircuit` is an
Orchard/Halo2 fixed two-input/two-output circuit. It consumes two private
asset-typed notes, proves spend authorization and per-asset value conservation,
and emits two replacement notes. Consensus verifies the proof and RedPallas
spend authorization signatures before updating nullifiers, commitments,
encrypted outputs, retained roots, and receipts.

The public swap action does not reveal raw asset ids, values, owners,
recipients, or bilateral price. Public observers see the pool id, circuit/proof
ids, anchor, nullifiers, randomized verification keys, output commitments,
encrypted outputs, swap binding hash, fee, proof bytes, and signatures.

Ingress exists and is public. `AssetOrchardIngressV1` burns a transparent issued
asset balance and creates an asset-typed private note. That boundary reveals the
asset and amount being shielded.

Egress exists only as disclosed egress. `AssetOrchardEgressV1` reads a local
typed note, discloses the note opening, recomputes the commitment and nullifier,
verifies authorization, nullifies the note, and credits the public issued-asset
balance. This is functional exit, not private cash-out.

The prover optimization exists but only helps a long-lived process. The best
measured CPU hot path is:

```text
K=15 cached proving key
prove_ms      5,780
verify_ms        66
proof_bytes   6,816
```

One-shot CLI workflows can still pay about `346s` of cold proving-key setup.
StakeHub must drive a warm runner or local proving daemon if the user-facing
flow is meant to feel fast.

## Definition of Done

A proper user-facing private NAV swap is not just a demo screen. It is complete
when a user can:

1. Create or select a PFTL wallet whose keys are locally stored under StakeHub's
   wallet boundary.
2. Fund the wallet with gas and an accepted public asset balance.
3. Bridge or otherwise receive `pfUSDC` as a public PFTL issued asset.
4. Snapshot and display pre-transaction public balances for `pfUSDC`, `a651`,
   and any gas/account asset used by the transaction path.
5. Shield `pfUSDC` into an Asset-Orchard note with a receipt that names the
   public ingress boundary.
6. Keep the K=15 proving key warm before the user presses Swap.
7. Execute `pfUSDC -> a651` through the real Asset-Orchard private swap action.
8. Display only supported public proof data: batch id, transaction id, receipt,
   height, action kind, proof verification result, and public nullifier/
   commitment fields.
9. Let the user hold or transfer the resulting private `a651` note inside the
   Asset-Orchard pool once the transfer primitive exists.
10. Let the user swap back `a651 -> pfUSDC` privately.
11. Let the user exit with an explicit privacy label:
    - disclosed exit today, which reveals the note facts; or
    - private egress after the private egress circuit is built.
12. Snapshot and display post-transaction public balances and receipt hashes so
    the user can understand what moved, what stayed private, and what was
    revealed at each boundary.

## Non-Negotiable Capability Boundary

Do not describe the current system as private end-to-end cash-out. The current
implemented shape is:

```text
Public ingress -> private Asset-Orchard middle -> disclosed egress.
```

The private middle hides asset id, value, owner, recipient, and price from the
public action. The boundaries do not. Any UX, docs, runbook, or article must
state that ingress and the current egress can leak timing, asset, amount, and
recipient facts.

## Target Product Shape

The product target is a private cash hub:

```text
public pfUSDC on PFTL
  -> public ingress into Asset-Orchard
  -> private pfUSDC note
  -> private pfUSDC <-> a651 swap
  -> private a651 note
  -> private transfer or private reverse swap
  -> exit path with an explicit privacy label
```

Low slippage should come from NAV-bound primary mint/redeem or signed RFQ-style
quote policy, not from walking a public Uniswap curve. The L1 should enforce
the quote/policy that the proof binds to; StakeHub should only orchestrate user
actions and present receipts.

## L1 Work Plan

### Phase 0: Freeze the UX/Protocol Surface

Goal: remove ambiguity before adding more UX.

Work:

- Keep `docs/status/shielded-layer-map.md` as the current-state map.
- Add one operator-facing runbook for the exact supported tiers:
  - transparent-only no-Orchard demo;
  - current private-middle flow with disclosed exit;
  - future private-egress flow.
- Make all StakeHub copy consume these tiers from one JSON/spec source so the
  UI cannot present private egress before the L1 supports it.

Acceptance:

- No docs present private egress as implemented before it exists.
- No docs present the current disclosed egress as hiding `a651` cash-out.
- The demo labels each boundary as public, private, or disclosed.

### Phase 1: Wallet and Note Service

Goal: make Asset-Orchard notes usable as wallet state instead of one-off files.

Work:

- Add a wallet service boundary for Asset-Orchard viewing/spending keys.
- Store encrypted local wallet material under a user-specific StakeHub/PFTL
  data directory.
- Add note scanning for `asset-orchard-v1` encrypted outputs.
- Track spendable, pending, spent, and disclosed-exited note status.
- Add deterministic receipt correlation between local notes and chain receipts.
- Expose a narrow RPC/CLI surface for:
  - list public balances;
  - list private notes by local wallet view;
  - build ingress;
  - build swap;
  - build disclosed egress;
  - submit and wait for finality.

L1 risk:

- Wallet scanning is not consensus-critical, but any action builder that creates
  signed/proved payloads must use canonical encodings and must fail closed on
  stale anchors, duplicate nullifiers, wrong chain ids, and wrong pool ids.

Acceptance:

- A user can restart StakeHub and recover private note state from chain
  encrypted outputs plus local keys.
- No spend builder can create an action against an unretained anchor without a
  clear error.
- The UI can show local note status without parsing ad hoc files.

### Phase 2: Warm Prover Runner

Goal: make the optimized K=15 path the default operator path.

Work:

- Add a long-lived local prover/runner process for Asset-Orchard actions.
- Preload and cache the K=15 proving and verifying keys.
- Expose readiness as an explicit machine-readable receipt:

```text
pool_id
circuit_id
K
params_hash
vk_hash
pk_cache_status
ready_at
process_id
```

- Make StakeHub call the runner for proving instead of spawning cold one-shot
  CLI commands.
- Keep one-shot CLI commands for recovery/debug, but mark them as cold path.

Acceptance:

- Warm readiness completes once per runner lifetime.
- A subsequent local swap proof uses the cached key and lands near the measured
  hot path, not the 346s cold path.
- The UI does not let the user confuse "proof pending" with "wallet warmed".

### Phase 3: Private Asset Transfer

Goal: let users move private `pfUSDC` or `a651` around inside PFTL without
requiring a swap against a counterparty note every time.

Work:

- Add an Asset-Orchard typed transfer action or generalized note spend action.
- Support one input to one recipient plus optional change, or a reviewed fixed
  shape that covers the first product need.
- Bind encrypted output bytes and recipient diversified address material to the
  action transcript.
- Preserve private asset id and amount.
- Reuse the existing nullifier, anchor, note commitment, and RedPallas
  authorization machinery where safe.

Consensus invariants:

- Input commitment exists under retained anchor.
- Nullifier is correctly derived and unused.
- Output commitment binds the same hidden asset tag and conserved value.
- Spend authorization signs the exact action transcript.
- No asset/value creation, no duplicate output commitments, no duplicate
  nullifiers.

Acceptance:

- Private `a651` can be sent to another PFTL wallet inside Asset-Orchard.
- The public action does not reveal raw asset id, amount, sender, recipient, or
  memo.
- Negative tests reject forged asset tags, forged conservation, replayed
  nullifiers, wrong chain domain, and wrong recipient binding.

### Phase 4: Quote-Bound NAV Swap

Goal: support low-slippage `pfUSDC <-> a651` primary mint/redeem or RFQ-style
swaps with policy enforcement, not just a bare conservation swap.

Work:

- Define a quote/policy envelope:

```text
asset_pair
side
amount_in_or_out
price_or_nav_band
fee_bps
issuer_or_market_maker_key
capacity
expiry_height
reserve_or_NAV_root
policy_hash
```

- Add consensus-visible commitment to that envelope.
- Add circuit constraints or verifier checks that the private values satisfy the
  quoted price, fee, capacity, and expiry rules.
- Decide which policy fields are public and which are committed privately.
- Bind the quote envelope into `H_action` / `swap_binding_hash`.

Consensus invariants:

- A stale quote cannot be replayed.
- A quote for `pfUSDC/a651` cannot authorize another asset pair.
- Capacity cannot be overfilled across multiple private actions.
- NAV/reserve roots used by the quote are finalized and canonical.
- Policy failure rejects before state mutation.

Acceptance:

- The product can support low-slippage private NAV swap under an issuer/RFQ
  policy.
- The receipt can prove which public quote/policy authorized the swap without
  revealing the hidden note details.
- Forged quote, expired quote, wrong asset pair, over-capacity, and wrong NAV
  root tests fail closed.

### Phase 5: Private Egress

Goal: replace disclosed egress with a private-egress primitive for user-facing
cash-out UX.

Current disclosed egress reveals:

- note opening;
- `asset_id`;
- amount;
- destination account;
- nullifier;
- spend/view material needed for validation.

Private egress should prove:

- the user owns a valid unspent typed note under a retained anchor;
- the note asset is eligible for the chosen exit route;
- the exited value is conserved into the public output or bridge output;
- the nullifier is correctly derived and unused;
- the public destination or bridge output is bound to the proof;
- fees and policy limits are satisfied;
- no note opening is disclosed.

Design options:

1. Direct private egress:
   - public output reveals destination and amount, but not the private note
     opening;
   - best first implementation if the goal is to stop revealing the exact note.
2. Batched private egress:
   - many private exits are aggregated before public settlement;
   - reduces timing and amount linkability, but adds coordinator and liveness
     complexity.
3. Fixed-denomination private egress:
   - exits in buckets to reduce amount fingerprinting;
   - requires change handling and user education.
4. Delayed/relayed private egress:
   - decouples submitter, owner, and final destination timing;
   - requires relayer fee policy and replay protections.

Recommended first step:

Build direct private egress as the consensus primitive, then add batching,
delay, relayers, and denominations as privacy hardening layers. Do not block
the circuit on the full mixer product, but do not overclaim its privacy.

Acceptance:

- Public validators can verify exit correctness without seeing the note opening.
- Receipts label the exit as direct private egress, batched private egress, or
  disclosed egress.
- Tests reject wrong asset, wrong amount, wrong destination binding, replayed
  nullifier, stale anchor, duplicate commitment, and forged spend authority.

### Phase 6: Canonical a651 Bridge-Out

Goal: let a user choose whether they exit to PFTL public `a651`, source-chain
`a651`, or USDC.

Work:

- Define canonical relationship between PFTL `a651` and any EVM `a651` token.
- Specify burn/mint or lock/mint semantics for bridge-out.
- Preserve global supply accounting with replayable evidence.
- If exiting to Uniswap-side `a651`, make clear that the user is leaving the
  PFTL private pool and entering a public EVM venue.
- If redeeming to USDC, bind the redemption policy, reserve route, fee, and
  source-domain/custody facts into the exit receipt.

Acceptance:

- A user can see whether they are receiving:
  - public PFTL `a651`;
  - EVM `a651`;
  - public PFTL `pfUSDC`;
  - source-chain USDC.
- Bridge receipts include source and destination chain facts, amount, asset,
  custody route, finality proof, and privacy label.
- Supply conservation is replayable across PFTL and the destination domain.

### Phase 7: StakeHub UX Contract

Goal: StakeHub becomes a transaction console that exercises the real protocol
state, not a diagram with hopeful labels.

Each step gets exactly one primary action button and one receipt panel:

1. Create PFTL wallet.
2. Fund wallet / faucet.
3. Bridge or receive `pfUSDC`.
4. Warm prover.
5. Snapshot balances before.
6. Shield `pfUSDC`.
7. Swap `pfUSDC -> a651`.
8. Private transfer or hold `a651`.
9. Swap `a651 -> pfUSDC`.
10. Exit through disclosed or private egress.
11. Snapshot balances after.

Each receipt panel must show:

- status: idle, ready, running, submitted, finalized, failed, or action needed;
- transaction or batch id;
- height;
- validator/finality receipt;
- input asset and amount if public;
- hidden/private label if not public;
- output asset and amount if public;
- proof hash or proof verification result;
- privacy label: public, private middle, disclosed egress, private egress.

Acceptance:

- No step says "manual command required" if the demo is presented as
  end-to-end.
- A user can tell where `pfUSDC` is custodied at every step.
- A user can tell whether `a651` is public, shielded, disclosed-exiting, or
  bridged out.
- Public balances and local private-note balances are checked before and after
  every transaction path that moves value.

## Test and Evidence Plan

Minimum L1 tests:

- Unit tests for canonical encoding and action hash stability.
- Positive and negative tests for ingress, swap, typed transfer, disclosed
  egress, and private egress.
- Property tests for conservation and duplicate nullifier rejection.
- Replay tests for quote expiry, quote overfill, wrong NAV root, wrong asset
  pair, and wrong pool domain.
- Release ignored tests for real Halo2 proof acceptance and forged proof
  rejection.
- Cold and hot prover benchmarks with explicit runner lifecycle.

Minimum live evidence:

- Local deterministic end-to-end flow.
- WAN devnet flow with:
  - public bridge/funding receipt;
  - public ingress receipt;
  - private swap receipt;
  - reverse private swap receipt;
  - disclosed or private egress receipt;
  - public balance snapshots before and after.
- Privacy scan proving the private swap action does not contain raw asset ids,
  amounts, owners, recipients, or price.

## Implementation Map

Likely L1 files and modules:

- `crates/privacy_orchard/src/asset_orchard_circuit.rs`
- `crates/privacy_orchard/src/asset_orchard_sinsemilla.rs`
- `crates/privacy_orchard/src/verify.rs`
- `crates/privacy_orchard/src/types.rs`
- `crates/types/src/lib_parts/shielded_bridge_governance.rs`
- `crates/node/src/privacy.rs`
- `crates/node/src/lib_parts/part_02.rs`
- `crates/node/src/lib_parts/part_03.rs`
- `crates/node/src/main_parts/cli_dispatch.rs`
- `crates/node/src/main_parts/runtime_helpers.rs`

Likely new or expanded surfaces:

- Asset-Orchard typed transfer action payload.
- Private-egress action payload.
- Quote/policy envelope payload.
- Long-lived prover runner command or service.
- Wallet/note scanning RPC/CLI.
- Receipt schema shared by L1 and StakeHub.

## Risk Register

| Risk | Impact | Mitigation |
|---|---:|---|
| UX presents private egress before it exists | High | Single capability-source spec plus receipt privacy labels |
| Cold prover path leaks into normal flow | High | Warm runner required before Swap is enabled |
| Private egress reveals too much through timing/amount | High | Direct circuit first, then batching/delay/denomination/relayer hardening |
| Quote policy creates asset/value inflation bug | Critical | Circuit/verifier binding, capacity accounting, negative tests, replay corpus |
| Asset tag collision or non-canonical asset id | High | Canonical registry, tag collision rejection, pinned hash domain |
| Wallet note scanner loses spend state | High | Chain-derived nullifier status plus local encrypted wallet DB |
| Bridge-out supply mismatch | Critical | Burn/mint or lock/mint invariant with replayable evidence |
| GPU prover sees witness | Medium | Treat GPU as sensitive compute boundary; use operator-controlled or hardened environments until trust model improves |

## Immediate Next Work

1. Build the long-lived Asset-Orchard prover runner and wire StakeHub to it.
2. Add a narrow Asset-Orchard wallet/note service so the demo stops depending
   on loose action files.
3. Add private typed transfer if the user needs to move `pfUSDC` or `a651`
   inside PFTL without a swap.
4. Design and implement direct private egress as the next consensus primitive.
5. Add quote-bound swap policy so low-slippage NAV swaps are issuer/RFQ backed
   rather than implied by a public AMM path.
6. Make StakeHub consume receipt schemas and show one action button per step.

## Harness Score

Text Improvement Harness score run:

```text
Command: tih score docs/plans/proper-private-nav-swap-plan.md --project postfiatl1v2 --gate full --runs 1 --concurrency 3 --force
Run group: full-2cb42d06be
Scored content SHA-256 before this result block: 2b190486c9bb94740c9afeccc9cd465d142a7c8f494955d8fafce6549706dbf2

openai/chat-latest                         93
openrouter/deepseek-v4-pro                 91
openrouter/claude-4.8-opus                 87
Average                                  90.33
```

The lowest-score critique was that the plan is strong on boundary labeling and
implementation sequencing, but should add more detail before implementation on
private-egress unlinkability, prover-runner lifecycle/fault tolerance, and the
exact quote-policy state machine. Those are left as design-deepening tasks
rather than hidden assumptions.
