# PFTL Resident Swap Limited-Availability Report

> **2026-07-30 amendment:** The resident service was subsequently moved to the
> dedicated high-core prover path, its mount identity mapping was hardened,
> and the finality observer was made proposal-view-aware without weakening
> certificate verification. Four qualification issue/redeem cycles committed
> exactly once. Issue p95 was `50.365 seconds`; redeem p95 was `38.862
> seconds`. Cycle 5 correctly stopped prepublication on stale governed NAV.
> The decision remains limited availability and `NO-GO` for the 100/100
> production gate. See
> [current state](A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md) and the
> [latest evidence](../evidence/pftl-private-swap-p0-20260730/README.md).

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
| Fleet release | `resident-local-commit-777faa0` |
| Fleet node revision | `777faa0e5cf4dfce52d36ce272dff7eecea1121d` |
| Fleet node binary SHA-256 | `0d47fc2ce57b8f5cdbcda2db1a406eb90d9d7b2c1bebe974a347de2d8d104291` |
| Safe apply-order readiness revision | `053ac01` |
| Canary-cap revision | `df7ae7c` |
| Resident swap revision | `06f80d24b6269ebed88cc28fb792b74b335b49b3` |
| Resident swap binary SHA-256 | `55e3e97a4968646947bb5eddb187bb6ad55056f1817699f0d864d263f82c0206` |
| Resident round-driver revision | `777faa0e5cf4dfce52d36ce272dff7eecea1121d` |
| Resident round-driver binary SHA-256 | `0d47fc2ce57b8f5cdbcda2db1a406eb90d9d7b2c1bebe974a347de2d8d104291` |
| Round-driver unit pin revision | `1e64c71` |
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

The `resident-precommit-6ad587f` release was installed one validator at a time
using a signed deployment manifest, the frozen quorum-safe apply order, and a
signed finalized-checkpoint backup at height 494. The backup root was
`1ed3653904ccb488352445bea184c3ca0b92d6b88e8239b7dc781477d6aeb1e745e05c242691ac7968b21b45745bb55e`.
Every post-apply check found the exact release hash above, an active service,
height-494 convergence, and empty mempools. Restarting validator-2's dependent
asset service triggered a fail-closed proving-key prewarm: readiness remained
HTTP 503 until the 447.9-second prewarm completed and then returned green.

The successor `resident-qc-view0-59cd3ee` release used the same signed,
one-validator-at-a-time process. Its mandatory signed finalized-checkpoint
backup verified at height 497 against state root
`b17e9259455350942a2bd915c50dc4aebc75e18bc8b763fee7bde76e68a23909e5357b69a055f3d10677aeaa10a50779`.
All six validator services and the resident round driver now run the exact
binary hash above. The dependent asset prover again failed readiness closed
during its cold prewarm and returned the complete resident service to HTTP
200 before the qualification cycle began.

The successor `resident-local-commit-777faa0` release used a mandatory signed
height-500 finalized-checkpoint backup that was clean-imported and verified
before the frozen one-validator-at-a-time rollout. Every apply preserved
six-validator convergence. An independent post-rollout audit confirmed the
exact binary above and active validator transport and RPC units on all six
nodes. The validator-2 resident round driver was then pinned to the same
release and returned green with five authenticated persistent peers, remote
proposer routing, local apply before send, and warm shielded verifiers.

## Certified functional runs

| Height | Route | Result |
|---:|---|---|
| 483 | transparent pfUSDC -> private A666 | committed; A666 supply +1,000,000 |
| 484 | private A666 -> private pfUSDC | committed; A666 supply returned to baseline |
| 486 | transparent pfUSDC -> transparent A666 | committed; A666 supply +1,000,000 |
| 487 | transparent A666 -> transparent pfUSDC | committed through the governed transparent path |
| 488 | transparent pfUSDC -> private A666 | committed through the resident service |
| 489 | private A666 -> transparent pfUSDC | committed; A666 supply returned to baseline |
| 490 | transparent pfUSDC -> private A666 | post-tuning canary committed; A666 supply +1,000,000 |
| 491 | private A666 -> private pfUSDC | post-tuning canary committed; A666 supply returned to baseline |
| 493 | transparent pfUSDC -> private A666 | instrumented canary committed; A666 supply +1,000,000 |
| 494 | private A666 -> private pfUSDC | instrumented canary committed; A666 supply returned to baseline |
| 495 | private pfUSDC -> transparent pfUSDC | controlled-input restoration committed |
| 496 | transparent pfUSDC -> private A666 | post-precommit-release canary committed; A666 supply +1,000,000 |
| 497 | private A666 -> private pfUSDC | post-precommit-release canary committed; A666 supply returned to baseline |
| 498 | private pfUSDC -> transparent pfUSDC | controlled-input restoration committed |
| 499 | transparent pfUSDC -> private A666 | bounded-QC canary committed; A666 supply +1,000,000 |
| 500 | private A666 -> private pfUSDC | bounded-QC canary committed; A666 supply returned to baseline |
| 501 | private pfUSDC -> transparent pfUSDC | controlled-input restoration committed under early local finality |
| 502 | transparent pfUSDC -> private A666 | early-commit canary committed; A666 supply +1,000,000 |
| 503 | private A666 -> private pfUSDC | early-commit canary committed; A666 supply returned to baseline |

The governed 1.000000-A666 quote consumed 905,538 pfUSDC atoms on issue and
returned 900,581 pfUSDC atoms on redemption. After height 489:

- A666 supply: 31,489,197,455 atoms;
- controlled public A666 balance: 0;
- controlled public pfUSDC balance: 1,690,667 atoms; and
- all six validators: height 489, identical block hash and state root, and
  zero pending mempool entries.

The converged state root is
`9cd41dfd5fb8571d627415dc01293d4276cf861eae30384a987d6c7562b30c24fe17f6c27a8910292006452d147d40be`.

After the post-tuning height-490/491 round trip, A666 live supply returned
exactly to 31,489,197,455 atoms and the route supply invariant remained true.
All six validators converged at height 491 with empty mempools, state root
`db2afdc65b01e7dca94524db61284e488dab43e547e80ce49dd87f42fbfc754910d80b844887bc1092fc47addc34bfe8`,
and block hash
`29f78fdad95ca9efe2d2046437302b5491d14699ac8126ecdfd2d41335327dc39c776e367e4a003b71eb0692acc55308`.

After the height-496/497 private round trip, A666 live supply again returned
exactly to 31,489,197,455 atoms. `invariant_holds` was true, active reservation
count and atoms were zero, and native spendable A666 balance count and atoms
were zero. All six validators converged at height 497 with empty mempools,
state root
`b17e9259455350942a2bd915c50dc4aebc75e18bc8b763fee7bde76e68a23909e5357b69a055f3d10677aeaa10a50779`,
and block hash
`325e528c79d68f68a0944357f0c216d5f21d02f025b257c8893da59380029c410cae316fcaf795b09d4828d6fcc9d5eb`.

After the height-499/500 private round trip, A666 live supply again returned
exactly to 31,489,197,455 atoms. The route invariant held, active reservation
count and atoms were zero, and native spendable A666 balance count and atoms
were zero. All six validators converged at height 500 with empty mempools,
state root
`d7f0b68fbed2473b0a8365eb67bfd601c730daee80a3393c3ee9a3a00361ec4275c669d7d284d4fb90b7a50359e24789`,
and block hash
`1a8e988c4a0c77c1353e0c01691f328e88cd2a263de59c55eab39a0cf46e9c724f141945f5e7f84a7a285eb044ea141f`.

After height 501 restored the controlled input, the height-502/503 private
round trip increased supply by exactly 1,000,000 atoms and returned it exactly
to the 31,489,197,455-atom baseline. The route invariant held, active
reservation count and atoms were zero, and all six validators converged at
height 503 with empty mempools. The exact private outputs remain restricted
operational state and are not included in this report.

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
- A later prepublication failure exposed a reservation-liveness bug: a
  terminal failed or interrupted intent still prevented a newly signed intent
  from using the same unspent input. Revision `06f80d2` now atomically marks
  that old lineage rejected when the replacement is journaled, while
  published and otherwise live lineages remain exclusive. The focused
  supersession test passed, the service was rolled forward, and subsequent
  h493/h494 and h496/h497 round trips left zero active reservations.
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
| tuned private issue | 490 | 165.7s | 88.3s | n/a |
| tuned private redemption | 491 | 141.8s | 81.0s | n/a |
| instrumented private issue | 493 | 161.2s | 83.9s | n/a |
| instrumented private redemption | 494 | 146.1s | 86.0s | n/a |
| post-precommit private issue | 496 | 156.2s | 82.4s | n/a |
| post-precommit private redemption | 497 | 145.9s | 84.6s | n/a |
| bounded-QC private issue | 499 | 127.3s | 81.9s | n/a |
| bounded-QC private redemption | 500 | 115.6s | 85.4s | n/a |
| early-commit private issue | 502 | 123.2s | 87.3s | n/a |
| early-commit private redemption | 503 | 109.0s | 83.4s | n/a |

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

The fresh height-490/491 canaries confirm the effect on the live critical
path. The issue's nested output-validity and outer proofs took 45.3s and
42.9s; the redemption's took 41.5s and 39.4s. There were zero cgroup
memory-limit or OOM events. Resource tuning reduced the observed private issue
proof DAG from 132.6s to 88.3s and the observed private redemption proof DAG
from 112.6s to 81.0s, but the two serial proofs still make the 35-second gate
unreachable on the current two-vCPU shape.

A release-mode synthetic issue benchmark then prewarmed the same
private-egress proving key and ran the unchanged two-proof builder with 32
Rayon threads on a 32-logical-CPU AMD EPYC KVM host. The nested proof took
6.593s, the outer proof took 6.128s, and the complete hot proof DAG took
12.751s. Both proofs verified and peak process RSS was 2,060,780 KiB. This is
not live-fleet qualification and does not open the scale gate, but it proves
that a one-proof circuit rewrite is not required to meet the 35-second proof
target. A qualified higher-core resident prover is a viable path.

Proof acceleration alone is insufficient for the end-to-end SLO. The two
tuned live samples retained approximately 60.8-77.4 seconds outside the
reported proof DAG. Even substituting the 12.751-second synthetic proof result
would leave an estimated 73.5-90.2 seconds accepted-to-committed if every
other stage remained unchanged. The round-driver admission/publish/certificate
timing must therefore be measured and reduced independently before the
100/100 campaign.

The instrumented height-494 round localized its 52.652-second certified round
to 5.333 seconds of proposal work, 6.334 seconds of prepare-vote collection,
27.992 seconds of certificate work, 5.551 seconds of local apply, and 7.049
seconds of certified sends. The certificate segment included 11.328 seconds
collecting consensus-v2 precommit votes.

Revision `6ad587f` removes a redundant full legacy proposal revalidation from
the remote precommit path without weakening finality. A validator may reuse
only its durable prepare vote for the exact proposal after cryptographically
verifying that vote against the live registry and state root. Missing, stale,
different-proposal, and signature-tampered votes fail closed. The targeted
safety test and the ignored six-validator TCP finality/catch-up integration
test passed. A broader node-library run was stopped while one unrelated
long-running Orchard debug proof was still CPU-bound; every test that had
completed was green, but that stopped run is not represented as a full-suite
pass.

The deployed h497 evidence confirms that the intended optimization is active:
remote precommit legacy-vote recovery took 4-7 milliseconds, versus roughly
2.6-2.7 seconds for full prepare vote construction on the same round. The
whole precommit collection stage nevertheless took 8.164 seconds because the
consensus-v2 proposal/QC verification, signing, and transport response path
remain. The full h497 certified round took 54.678 seconds, including 10.351
seconds of proposal work, 5.634 seconds of prepare-vote collection, 24.735
seconds of certificate work, 5.791 seconds of local apply, and 7.742 seconds
of certified sends. Accepted-to-committed remained 145.873 seconds because
the 84.570-second proof DAG and 57.453-second published-to-committed path both
remain above budget.

Substituting the 12.751-second 32-core proof benchmark into h497 without any
other improvement projects approximately 74.1 seconds
accepted-to-committed. Higher-core proving is therefore necessary but still
insufficient. The next consensus optimization must reduce proposal/QC
verification and transport/finality latency while retaining exact proposal
binding and full quorum verification.

Revision `59cd3ee` removes the height-linear component of that remaining
path. Each validator had accumulated 577 persisted consensus-v2 QCs (about
36 MiB). The view-zero hot path was rebuilding and cryptographically
re-verifying that entire unrelated history one or more times per vote. The
new path preserves complete QC signature, committee, quorum, target, and
canonical-ID verification for the QC being persisted. It uses an empty
dependency graph only for view zero, where timeout evidence and valid-round
QC references are forbidden by protocol; nonzero views still reconstruct and
verify the historical dependency graph. Signature-tampered QCs fail before
write, and immutable existing QC IDs still cannot be replaced.

Qualification included clippy with warnings denied, 29 ordering/adversarial
tests, the four-node consensus-v2 finality test, two store tests including
tampered-QC and view-bound dependency coverage, 11 snapshot/deployment
integrity tests, and the ignored six-validator TCP finality/catch-up test.
All passed.

The deployed height-499 issue certified round fell to 36.247 seconds, with
prepare-QC, precommit-QC, and commit assembly each below 0.5 seconds.
Published-to-committed fell from 67.581 seconds at h496 to 39.187 seconds at
h499. The height-500 redemption certified round fell further to 23.257
seconds, with 17.916-second client-visible finality; its
published-to-committed time was 26.206 seconds. The h500 stage totals were
2.934 seconds proposal, 4.012 seconds prepare-vote collection, 6.778 seconds
certificate work, 3.665 seconds local apply, and 5.339 seconds certified
sends.

Revision `777faa0` removes redundant legacy certificate state re-execution
only after exact local proposal execution and matching. It retains signature,
committee, quorum, registry, target, and certificate-ID verification, while
state apply independently recomputes the proposal. After apply, an atomic
local certified-batch marker is published before durable fleet sends; the
send outbox remains restart-recoverable.

The live height-502 issue measured 123.202 seconds accepted-to-committed,
87.326 seconds proof DAG, 29.242 seconds published-to-committed, 32.196
seconds for the certified round, and 25.157 seconds client-visible local
finality. Height-503 redemption measured 108.957, 83.372, 21.844, 24.695,
and 18.975 seconds respectively. Redundant legacy certificate work fell to
0.023 seconds for issue and 0.013 seconds for redemption. The exact round
trip restored supply and all six nodes converged, so early local publication
did not weaken fleet finality.

The remaining gate is narrower but not closed. Replacing each live two-core
proof DAG with the measured 12.751-second 32-core result projects height-502
issue at approximately 48.6 seconds and height-503 redemption at
approximately 38.3 seconds accepted-to-committed. This is a projection, not
production qualification. An always-on higher-core prover is still required,
and issue must lose approximately another 3.7 seconds or obtain a
correspondingly faster live proof result before the 100/100 scale campaign
can credibly target p95 <=45 seconds.

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

Before raising limits, an always-on higher-core prover must bring the live
proof path within the SLO, the measured issue path must close its remaining
approximately 3.7-second projected gap (or achieve an equivalently faster
proof), and the 100/100 campaign and remaining fault matrix must pass.
