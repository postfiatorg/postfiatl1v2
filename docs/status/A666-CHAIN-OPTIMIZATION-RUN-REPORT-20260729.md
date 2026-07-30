# A666 Chain Optimization Run Report

> **Historical performance report:** This records the 2026-07-29 full-chain
> optimization run. It remains valid evidence for that run, but it is not the
> current resident-service or production-release status. See
> [A666, pfUSDC, Private Swap, Bridge, and Uniswap Current State](A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md).

Date: 2026-07-29 UTC

Repository: `postfiatorg/postfiatl1v2`

## Executive summary

This campaign attempted to make the canonical private A666 issue and redemption
flows complete in 25 minutes or less without weakening Ethereum finality,
Orchard proof verification, PFTL consensus, replay protection, or supply
conservation.

The result was mixed:

| Flow | Target | Measured result | Verdict |
|---|---:|---:|---|
| Private A666 issue | <= 1,500s | 1,776s | FAIL by 276s |
| Private A666 redemption recovery | <= 1,500s | 888s | PASS |

The private issue worked functionally from Ethereum USDC deposit through
pfUSDC, private A666 issuance, PFTL export, Ethereum mint, and destination
consume. It did not meet the latency target.

The subsequent private redemption also worked. It burned the newly minted
wrapped A666, privately redeemed the native A666, withdrew USDC through the
pfUSDC bridge, rejected replay, and returned the measured NAV/wA666 supply
values to the frozen pass-6 baseline.

The primary latency blocker is canonical Ethereum finality. Two serialized SP1
Groth16 proofs and Ethereum transaction inclusion are the important secondary
costs. PFTL consensus startup and Orchard verifier startup are no longer the
main blockers.

## Objective and acceptance criteria

The run was intended to demonstrate the following user flow:

1. Deposit Ethereum USDC into the pfUSDC bridge.
2. Wait for canonical Ethereum finality and prove the deposit.
3. Claim pfUSDC on PFTL.
4. Privately issue A666 against the verified settlement value.
5. Export A666 from PFTL and mint wrapped A666 on Ethereum.
6. Burn wrapped A666 on Ethereum.
7. Privately redeem native A666 for pfUSDC.
8. Withdraw USDC on Ethereum.

The acceptance criteria were:

- issue time no greater than 1,500 seconds;
- redemption time no greater than 1,500 seconds;
- no manual intervention after the scored deposit;
- full Ethereum finality unchanged;
- both required Orchard private-primary proofs still generated and verified;
- six-validator PFTL convergence after every committed round;
- replay rejection on the withdrawal;
- wrapped and native A666 supply conservation; and
- no private note, seed, or spending-key material copied into evidence.

The frozen comparison baseline was pass 3:

- issue: 1,824 seconds;
- redemption: 852 seconds;
- functional and reconciled, but issue latency failed.

## Optimization work implemented

### 1. USDC approval moved outside the scored window

The deposit tool gained two fail-closed modes:

- `--approval-only`, which creates the exact USDC allowance without moving
  value; and
- `--require-preapproved`, which refuses to submit the deposit unless that
  exact allowance already exists.

This prevents an ERC-20 approval transaction from consuming issue-SLO time.

### 2. Ethereum epoch-aware deposit placement

A pre-deposit watcher waits until beacon head slot 28 of 32 before submitting
the value-moving transaction. The wait is explicitly outside the scored
window.

This did not change the finality rule. The bridge still required the deposit
block to be covered by a finalized Ethereum checkpoint.

The optimization reduces avoidable epoch-position variance, but it cannot
remove the protocol-level finality floor.

### 3. Resident PFTL consensus workers

A new resident-round orchestrator starts one proposer-bound, one-shot consensus
worker for each expected future PFTL height before the Ethereum deposit.

For pass 6, six workers were frozen:

1. pfUSDC deposit proposal;
2. pfUSDC finalize and claim;
3. private pfUSDC ingress;
4. private-primary A666 issue;
5. private A666 egress; and
6. A666 export.

Each worker:

- was bound to the deterministic proposer for one height;
- used an isolated certified-send outbox;
- warmed both Orchard verifier caches before reporting readiness;
- accepted exactly one atomically published batch;
- required verified local application and verified certified delivery; and
- required six-validator convergence before the next stage.

This removed repeated process startup and verifier-cache initialization from
the critical path without changing consensus validation.

### 4. Dependency-safe pfUSDC relay

An experimental optimization attempted to put pfUSDC proposal, finalization,
and claim in one block. The execution state machine can process those
operations sequentially in one block, but mempool admission validates the
claim against pre-block state. The claim therefore could not be admitted
before the proposal had created the holder trustline.

The final implementation preserves two rounds:

1. proposal; then
2. finalize and claim.

It also explicitly rewrites the claim recipient to the intended holder
trustline account, matching the previously proven relay flow.

### 5. Validator-specific Orchard service

The resident Orchard service was rebuilt for validator-2's
`skylake-avx512` CPU.

The exact ignored regression test that generates and verifies both
private-primary proofs measured:

| Binary | Wall time |
|---|---:|
| Generic x86-64 | 525.88s |
| `skylake-avx512` | 507.68s |

This was a 3.5% improvement in the deterministic heavy test. The optimized
service was deployed atomically, restarted, allowed to rebuild its proving-key
cache before funding, and required to report warm readiness.

No circuit, public input, verification key, or proof invariant changed.

### 6. Redundant SP1 export execution removed

The PFTL-to-Ethereum export prover previously performed a complete SP1 host
execute pass and then generated the Groth16 proof.

The optimized mode:

- natively verifies the witness and derives canonical expected public values;
- skips only the redundant SP1 host execute pass;
- generates the Groth16 proof;
- verifies the proof locally; and
- requires the proof's public values to match the native expected values.

The removed host execute pass represented 222,133,675 SP1 cycles and measured
about 6.3 seconds on the test witness.

### 7. Evidence and fail-closed orchestration

The run tooling freezes:

- orchestration commit;
- validator release and binary hashes;
- resident Orchard binary hash;
- A100 export-prover binary hash;
- bridge deployment manifest hash;
- NAV/reserve manifest;
- starting PFTL height and state root;
- Ethereum balances, nonce, verifier height, and contract pause state;
- expected amounts and supply values; and
- hashes of every live orchestration script.

The runner refuses tracked code changes after this freeze. A failure creates a
run-failure document and disqualifies intervention-free status.

## Live pass history

### Pass 4: pre-deposit readiness failure

Pass 4 stopped before approval or deposit.

The resident workers launched, but the readiness assertion correctly rejected
them because the three opt-in verifier-prewarm environment variables had not
been exported into the worker processes.

Fix:

- export the global prewarm switch;
- export the Orchard swap-verifier prewarm switch;
- export the Orchard private-egress-verifier prewarm switch; and
- test that the ready report confirms both verifier caches are warm.

No Ethereum or PFTL state changed.

### Pass 5: invalid one-round relay experiment

Pass 5 completed USDC approval, deposit, Ethereum finality, and the SP1 ingress
proof. It then attempted the one-round proposal+finalize+claim relay.

The claim failed mempool admission because its holder trustline did not exist
in pre-block state. No block was committed by the failed batch.

Recovery:

- proposal and finalization committed at height 467;
- the corrected holder claim committed at height 468;
- the fleet converged 6/6 with empty mempools; and
- the one-round compression was removed.

The pass-5 deposit remains as 905,538 pfUSDC atoms, or 0.905538 pfUSDC, owned by
Joe and backed by the corresponding Ethereum vault obligation. It was not
converted into A666 and was not withdrawn during this campaign.

### Pass 6: clean measurement

Pass 6 started from reconciled PFTL height 468.

Frozen issue amounts:

| Quantity | Atoms |
|---|---:|
| A666 issued | 1,000,000 |
| Base settlement value | 901,032 |
| Issue spread | 4,506 |
| USDC/pfUSDC deposit | 905,538 |
| Redemption output | 900,581 |

Ethereum deposit:

- transaction:
  `0x8d9d202379cf20ba0fb71932579fcf3e380a94af95e9e78eb87f19acb2cd2d3e`;
- deposit block: 25,636,372;
- finalized covering block: 25,636,375.

PFTL issue progression:

| Height | Operation |
|---:|---|
| 469 | pfUSDC proposal |
| 470 | pfUSDC finalize and holder claim |
| 471 | private pfUSDC ingress |
| 472 | private-primary A666 issue |
| 473 | private A666 egress |
| 474 | A666 export |
| 475 | Ethereum destination consume recorded on PFTL |

Every round reported:

- `round_ok=true`;
- five validator votes, satisfying quorum;
- local apply verified;
- all certified sends verified; and
- converged post-state.

The exact block-timestamp issue result was:

- deposit timestamp: 1,785,302,579;
- Ethereum mint timestamp: 1,785,304,355;
- elapsed: **1,776 seconds**;
- target: 1,500 seconds;
- result: **FAIL by 276 seconds**.

The fail-closed timing gate stopped the automatic runner after destination
consume. The issue was functionally successful, but the run was correctly not
reported as a passing optimization run.

## Redemption recovery and final accounting

Because pass 6 had already minted wrapped A666, the standard private redemption
path was run to restore NAV/wA666 supply.

PFTL recovery progression:

| Height | Operation |
|---:|---|
| 476 | Ethereum A666 return import |
| 477 | private Orchard ingress |
| 478 | private-primary A666 redemption |
| 479 | private pfUSDC egress |
| 480 | pfUSDC burn-to-redeem |
| 481 | pfUSDC redemption settlement |

Measured redemption:

- burn block: 25,636,541;
- withdrawal block: 25,636,613;
- elapsed: **888 seconds**;
- target: 1,500 seconds;
- result: **PASS**.

Final checks:

| Check | Result |
|---|---|
| Private issue functionality | PASS |
| Private redemption functionality | PASS |
| Ethereum withdrawal | PASS |
| Withdrawal replay rejected | PASS |
| Final native supply invariant | PASS |
| Six-validator convergence | PASS |
| Fleet mempools empty | PASS |

Supply comparison:

| Quantity | Pass-6 baseline | Final |
|---|---:|---:|
| Authorized valid A666 supply | 31,489,197,455 | 31,489,197,455 |
| Outstanding bridge claims | 31,489,197,455 | 31,489,197,455 |
| Ethereum-spendable native supply | 0 | 0 |

Final PFTL state:

- height: 481;
- state root:
  `e47faa4a5458ff6587914e48df8086b1eabcbffb156f38ec25652caf5dce51aeca8fdfcebbd6f24fee80c035e9b0cb48`;
- validators converged: 6/6;
- total mempool pending: 0;
- resident Orchard service warm and mirrored at height 481.

The conservation statement above is relative to the frozen pass-6 baseline at
height 468. That baseline includes the separate pass-5 pfUSDC inventory.

## What the measurements show

Pass 6 improved issue time from 1,824 seconds to 1,776 seconds: a 48-second
improvement.

The result disproved the assumption that process startup and PFTL wrapper
overhead alone could produce enough margin. Those optimizations were valid,
but the canonical path remains dominated by:

1. Ethereum finalized-checkpoint latency;
2. the SP1 Ethereum-ingress Groth16 proof;
3. the SP1 PFTL-export Groth16 proof; and
4. Ethereum mint transaction inclusion.

PFTL consensus and Orchard verification are meaningful costs, but they are not
the primary blocker after the resident-worker work.

## Required path to 25 minutes or less

Another identical full-finality orchestration run is not justified. A reliable
passing result requires at least one structural change.

### Canonical full-finality path

Preserve full finality and overlap work with the finality interval:

- continuously construct checkpoint and witness scaffolding as Ethereum
  advances;
- precompute every proof component not dependent on the final finalized root;
- keep SP1 setup/proving resources resident;
- aggregate ingress and export statements where possible; and
- benchmark a materially faster SP1 release or proving topology.

This is the preferred path if the 25-minute target must apply to the canonical
trustless lane.

### Separate governed fast lane

If product latency matters more than waiting for canonical settlement, add a
separate, explicit policy:

- capped trade size;
- governed confirmation rule;
- priced spread or relayer fee;
- relayer/fronted liquidity;
- canonical proof-based reconciliation afterward; and
- clear user disclosure that the fast lane has a different risk model.

This must not silently replace or weaken the canonical full-finality path.

## Code and evidence

Principal implementation commits:

- `d431547` — critical-path optimization stack;
- `1ccc915` — resident verifier-prewarm fix;
- `d8d3557` — dependency-safe two-round pfUSDC relay;
- `f58ec90` — pass-6 evidence and recovery;
- `dcb1cae` — explicit retained-pfUSDC accounting.

Primary evidence:

- `docs/evidence/a666-optimization-run-20260729/private-1-a666-roundtrip-pass4/`
- `docs/evidence/a666-optimization-run-20260729/private-1-a666-roundtrip-pass5/`
- `docs/evidence/a666-optimization-run-20260729/private-1-a666-roundtrip-pass6/`
- `docs/evidence/a666-optimization-run-20260729/private-1-a666-roundtrip-pass6/optimization-score.json`
- `docs/plans/CHAIN-OPTIMIZATION-STACKED-RESEARCH-20260729.md`

## Final status

The canonical private A666 round trip is functionally working.

Private redemption is within the 25-minute target.

Private issue is not yet within the target under the current serialized,
full-Ethereum-finality architecture. The measured miss is 276 seconds, and the
next work should target overlap or a clearly separate governed fast lane—not
additional cosmetic orchestration changes.
