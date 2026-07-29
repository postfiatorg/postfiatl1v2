# PFTL Resident Swap Limited-Availability Report

**Date:** 2026-07-29
**Scope:** PFTL-resident pfUSDC/A666 issue and redemption
**Determination:** bounded limited availability passed; scale/performance
qualification remains closed

## Outcome

The PFTL-resident service now performs governed A666 issue and redemption in
both private-default and transparent-output forms. A successful issue
increases A666 supply and consumes the governed amount of pfUSDC. A successful
redemption decreases A666 supply and returns the governed amount of pfUSDC.
The service is independent of Ethereum finality once pfUSDC is spendable on
PFTL.

Use is restricted to:

- one controlled wallet;
- one active swap;
- loopback-only access on validator-2; and
- at most 1,000,000 NAV atoms (1.000000 A666) per request.

This is a safety release, not a production-scale latency claim. The current
proof path remains materially slower than the specification.

## Deployed state

| Item | Deployed value |
|---|---|
| Fleet release | `resident-remote-proposer-5548112` |
| Fleet node revision | `5548112` |
| Fleet node binary SHA-256 | `e0927992a97590d8542e16dba998b92df597a38c4be159a56fd9278ed228dc6a` |
| Safe apply-order readiness revision | `053ac01` |
| Canary-cap revision | `df7ae7c` |
| Resident swap binary SHA-256 | `4448d80dd2af2f4e4b446f09b7259a92fbd6f08fdb5fa626b7946a22c4ed7367` |
| Maximum NAV amount | 1,000,000 atoms |
| Active-swap limit | 1 |
| Controlled wallets | 1 |
| Prover scheduling | `Nice=0`, `CPUWeight=100` |
| Prover memory ceiling | 2,048 MiB |

All three resident units are active:

- `postfiat-asset-orchard-local.service`
- `postfiat-pftl-round-driver.service`
- `postfiat-pftl-swapd.service`

Readiness requires five authenticated remote peers, persistent vote streams,
remote proposer routing, local apply before certified send, warm issue/redeem
quotes, and healthy durable stores. Stopping the round driver also stopped
swap admission through the unit dependency and readiness chain; restarting
the dependency restored a green service.

## Certified functional runs

| Height | Route | Result |
|---:|---|---|
| 483 | transparent pfUSDC -> private A666 | committed; A666 supply +1,000,000 |
| 484 | private A666 -> private pfUSDC | committed; A666 supply returned to baseline |
| 486 | transparent pfUSDC -> transparent A666 | committed; A666 supply +1,000,000 |
| 487 | transparent A666 -> transparent pfUSDC | committed through the governed transparent path |
| 488 | transparent pfUSDC -> private A666 | committed through the resident service |
| 489 | private A666 -> transparent pfUSDC | committed; A666 supply returned to baseline |

The governed 1.000000-A666 quote consumed 905,538 pfUSDC atoms on issue and
returned 900,581 pfUSDC atoms on redemption. After height 489:

- A666 supply: 31,489,197,455 atoms;
- controlled public A666 balance: 0;
- controlled public pfUSDC balance: 1,690,667 atoms; and
- all six validators: height 489, identical block hash and state root, and
  zero pending mempool entries.

The converged state root is
`9cd41dfd5fb8571d627415dc01293d4276cf861eae30384a987d6c7562b30c24fe17f6c27a8910292006452d147d40be`.

## Safety matrix

### Exact live private issue

The real height-483 private-issue batch was evaluated against an offline
reconstruction of its certified height-482 pre-state. The reconstructed root
exactly matched
`ae664ff1288aa7ede7fd565819b3be86734f5973dbca9a888ef71dd4bb73862c216e8e5389e5062f9611f3607645c6c8`.

The valid baseline accepted. All 11 negative cases rejected and preserved the
pre-state:

- invalid ingress action;
- invalid private-primary action;
- duplicate nullifier;
- reordered actions;
- missing batch-local commitment;
- stale anchor;
- changed policy;
- exhausted issue capacity;
- changed NAV;
- malformed action bounds; and
- replayed batch ID.

### Exact live private redemption

The conformance tool applied the real height-483 issue as a read-only in-memory
prefix, then evaluated the real height-484 private-redemption batch. The
computed pre-redemption root exactly matched certified height 483:
`1a19e84a97519e1d75555d86da107c09a1c9164fad61946d81320bb8715a0b193e2d49e81984326c17965ffde72f2dc2`.

The valid baseline accepted. All 8 negative cases rejected and preserved the
pre-state:

- invalid private-primary action;
- duplicate nullifier;
- stale anchor;
- changed policy;
- exhausted redemption capacity;
- changed NAV;
- malformed action bounds; and
- replayed batch ID.

The new `--prefix-batch-files` conformance option performs only in-memory
simulation. It does not bypass certificates or write state.

### Runtime recovery and rejection

- Replaying a committed signed intent returned the original committed result
  and left height 489 unchanged.
- A tampered signed intent returned HTTP 403 and left height unchanged.
- A 1,000,001-atom quote returned HTTP 400; a 1,000,000-atom quote returned
  HTTP 200; height remained 489.
- The height-483 private issue preserved one idempotency lineage across
  interrupted and failed prepublication attempts before committing.
- A forced round-driver outage made swap admission unavailable and a clean
  restart restored all required readiness capabilities.
- Focused Rust tests passed for atomic rollback, stale/wrong anchors and
  domains, public-only private-primary requests, and rejection of note
  openings in service requests.

## Private-material scan

The scan covered the durable swap journal, quote store, selected public canary
stdout/stderr, and logs for the three resident services.

Results:

- forbidden JSON keys: 0;
- exact matches across 38 sensitive values extracted from restricted source
  records: 0;
- journal permissions: `0600`.

The only broad-label matches were CLI help text containing the literal option
name `--spending-key-hex HEX`; no key value was present. Raw signed intents,
note records, consensus spools, seeds, openings, and output references remain
restricted operational state and were not copied into this report.

## Performance result

| Route | Height | Accepted -> committed | Proof DAG | Optional transparent egress |
|---|---:|---:|---:|---:|
| private redemption | 484 | 354.3s | 112.6s | n/a |
| transparent issue | 486 | 275.3s | 129.3s | 60.1s |
| private issue | 488 | 202.5s | 132.6s | n/a |
| transparent redemption | 489 | 263.3s | 127.7s | 60.1s |

The required targets are accepted-to-committed p50 <=20 seconds, p95 <=45
seconds, and proof-DAG p95 <=35 seconds. These samples fail those targets.
Most of the wall time is proof construction, not Ethereum finality. Remote
proposer rotation and six-validator consensus are functioning.

The resident private-primary builder currently creates two
`asset-orchard-private-egress-v1` proofs serially: one proves the encrypted
output's asset and value and the second proves the input spend bound to that
output proof. Recent resident issue/redeem timing records show about 86-91
seconds for those two proofs alone on the validator's two available Rayon
threads. All six validators have the same 2-vCPU/4-GB host shape, so moving the
service between existing validators does not remove this bound. The prover
cgroup had reached its former 1,280-MiB memory ceiling. The live service is now
running at normal priority with a 2,048-MiB ceiling; its post-change key
prewarm completed in 441.5 seconds with zero memory-limit or OOM events, and
resident readiness returned green at unchanged height 489. This removes
avoidable resource pressure, but cannot make two serial 40-45-second proofs
fit a 35-second proof-DAG target.

## Rollback rehearsal

The certified height-482 governance block was replayed from the verified
height-481 backup using its archived batch, block record, and original round
certificate. The replay:

- accepted the governed amendment;
- advanced the isolated state from height 481 to 482;
- reproduced certified state root
  `ae664ff1288aa7ede7fd565819b3be86734f5973dbca9a888ef71dd4bb73862c216e8e5389e5062f9611f3607645c6c8`;
  and
- passed the post-replay state verification commands.

An earlier rehearsal failure was not a protocol replay defect. The temporary
fixture had selected a stale certificate for a different height-482 proposal.
Its proposal hash began `c8e4`, while the original governance round
certificate and reconstructed proposal both bind `cf8a`. Strict proposal-hash
verification correctly rejected the stale certificate; no verifier bypass or
compatibility exception was added.

## Gate decision

The limited-availability gate is open only under the explicit 1.000000 NAV,
single-wallet, single-active-swap cap.

The following remain blocked:

- any higher value or concurrency limit;
- a production latency claim;
- the required 100-issue and 100-redemption qualification campaign;
- bounded-burst qualification; and
- public or non-custodial service claims.

Before raising limits, the proof path must be brought within the SLO, the
100/100 campaign and remaining fault matrix must pass.
