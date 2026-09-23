# Z3 NAVCoin Round-Trip Execution Plan

**Status:** Execution plan - awaiting the operator's go; authorizes no live value movement

**Harness score:** Text Improvement Harness full gate passed on 2026-09-03 — average 90.00/100 (GPT 93.00, Fable 88.40, GLM 88.60; five runs per lane; run group `z3-navcoin-roundtrip-plan`); scored content SHA-256 `6c509701ea0c5978d3f11ef42d9e921b0f458d9426db48db990c42668626b9a1`

**Date:** 2026-09-03

**Owner:** Post Fiat operator for authorization; agent for preparation, tooling, and evidence

**Parent gate:** [L1v2 Public Testnet Path, Gate Zero Z3](l1v2-public-testnet-path-milestone.md#gate-zero-operator-preconditions-recorded-2026-08-30)

## Purpose

Z3 demands repeated end-to-end NAVCoin round trips in the order deposit, mint, swap, and redemption on one qualified release lineage without a consensus upgrade during the qualification window.
It matters because Gate Zero blocks every community-facing public-testnet step until this behavior is stable and evidenced.
The [Arc grant proposal](../../business/pfusdc-arc-grant-proposal-20260828-v3.md) says pfUSDC is the settlement leg for recurring NAVCoin subscription and redemption cycles, so Z3 must prove that recurring integrated flow rather than infer it from separate bridge and NAVCoin demonstrations.

## Controlling scope

This plan applies the precedence rule in
[NAVCoin Reserve Redemption System Specification section 2.2](../../deferred-plans/NAVCOIN-RESERVE-REDEMPTION-SYSTEM-SPEC-20260730.md#22-binding-monday-demonstration-profile).

The first route is the smallest existing primary route that can accept the selected source-labeled pfUSDC without a new consensus transaction kind.
The flow uses existing primary subscription and redemption operations.
It does not create a generic NRRS facility.
It does not deploy another settlement asset.
It does not use an AMM or the legacy offer book to substitute for primary issuance and redemption.
If the selected Arc pfUSDC cannot enter the existing A666 route through existing configuration and governance operations, work stops for an operator decision.
That incompatibility is not authority to add consensus code.

The intended integrated cycle is:

~~~text
Arc testnet USDC deposit
  -> proof-verified pfUSDC claim and mint on PFTL
  -> user-signed A666 primary subscription with pfUSDC
  -> release of the unused A666 export entitlement
  -> fresh reserve proof and governed route epoch
  -> user-signed A666 primary redemption into pfUSDC
  -> pfUSDC burn on PFTL
  -> proof-verified USDC release on Arc testnet
~~~

The same ordinary funded test wallet should own the economic cycle.
Normal proof finalization and route maintenance keep their existing authorities.
No per-customer operator approval may replace the user's signed subscription or redemption.

## Evidence baseline and reconciliation

The Arc evidence below is committed on the read-only integration lineage at
[38e626e95682f79639a83aba5cf617807e960c8e](https://github.com/postfiatorg/postfiatl1v2/commit/38e626e95682f79639a83aba5cf617807e960c8e).

An earlier observed epoch-7 pair is anchor
0x661D558a818A07002C7D5da4A3179c4672FEf124 and vault
0xe88FB9ab4890f513261F0aCA4FF13bfBa3e14862.
The [deployment record](https://github.com/postfiatorg/postfiatl1v2/blob/484a0feb524f6526b8f96f8ce4a79a74205bbf62/docs/evidence/arc-mvp-20260828/deployments.md)
says that pair is pinned to the August egress key and is not the current-v2 qualification pair.
The 2026-09-02 bridge cycle used fresh current-v2 contracts, including anchor
0x92390d3a2102cb74e4746c05b4d91f61093475d0 and vault
0x160307f3efead79b6a3629c4b8d90e8301fc250f.
A future cycle must pin full read-back identities and must not select a pair from a shortened address.

| Z3 element | What committed evidence demonstrates | Verdict and remaining work |
| --- | --- | --- |
| Deposit | The [Arc receipt](https://github.com/postfiatorg/postfiatl1v2/blob/38e626e95682f79639a83aba5cf617807e960c8e/docs/evidence/arc-mvp-20260828/devnet-20260902/ingress/deposit.receipt.json) records transaction 0xbc8a1b3875394e0132e99a5815a6a66107afa8b894cd6370440a6fbdd1c7259c, block 60,163,641, status 0x1, deposit log index 65, deposit ID fe5f47725604a0112574fac5a0726bd83bb26cdd250d0315f561de21dc5e7e60, and 1,000,000 USDC atoms into the current-v2 vault. | **Demonstrated once on Arc testnet.** Repeat it inside every integrated Z3 cycle against the operator-pinned pair. |
| Mint | The [certified operations report](https://github.com/postfiatorg/postfiatl1v2/blob/38e626e95682f79639a83aba5cf617807e960c8e/docs/evidence/arc-mvp-20260828/devnet-20260902/devnet/arc-finalize-claim.report.json) records accepted finalize transaction ab31da324d62a8163a9164e5504b5792bef9ce2f9f62cbbfaf02822f739408135cd27e88bc294b1835c5029878ea60c5 and claim transaction df9f4d163db2253fb15e84fa6313b8a22c8961e374ba66738eae97efbfc7f820348d62f3811330a5b96e2f3e1800e843 in the height-947 round for 1,000,000 atoms. | **Demonstrated once on PFTL devnet.** Bind the resulting source-labeled pfUSDC balance to the later A666 subscription in the same cycle. |
| Swap | The Arc packet stops after mint and proceeds to burn; it contains no pftl_uniswap_order_reserve, pftl_uniswap_primary_subscribe_v2, entitlement release, NAV refresh, or pftl_uniswap_primary_redeem receipt. The July primary-route demonstration and the historical native-NAV offer-book smoke prove separate paths, not an Arc-funded integrated cycle. | **Not demonstrated.** Run the existing A666 primary subscription and primary redemption path with the same-cycle pfUSDC, without AMM substitution or new consensus code. |
| Redemption | The [egress witness](https://github.com/postfiatorg/postfiatl1v2/blob/38e626e95682f79639a83aba5cf617807e960c8e/docs/evidence/arc-mvp-20260828/devnet-20260902/egress/egress-witness.json) contains accepted PFTL burn transaction 65d00f38a8caf6ca423c9fca63494882fdbe77ea77077efb224fe4b30cc2da04b2095b522f11f2506d775cc975638819 at finalized height 948 for 1,000,000 atoms. The [Arc release receipt](https://github.com/postfiatorg/postfiatl1v2/blob/38e626e95682f79639a83aba5cf617807e960c8e/docs/evidence/arc-mvp-20260828/devnet-20260902/egress/withdraw.receipt.json) records transaction 0x86036658acab859dc1ab8200140a33ddadfac603ca402fb041e504a28e77a207, block 60,170,160, status 0x1, and a 1,000,000-atom release; the retained replay record shows a second claim reverted. | **Bridge redemption demonstrated once; Z3 remains open.** The same integrated cycle still needs an accepted A666 primary redemption into pfUSDC before the proved burn and release. |
| Repeatability | The 2026-09-02 packet closes one deposit, mint, burn, and release sequence. No committed packet contains consecutive integrated NAVCoin cycles or a sustained unchanged-lineage window. | **Not demonstrated.** Complete the fixed consecutive-cycle campaign below with no reset-triggering failure. |

## Existing implementation anchors

The plan composes existing code rather than authorizing a replacement:

- crates/types/src/core_chain.rs defines the existing vault-bridge claim, burn, primary-subscribe-v2, entitlement-release, and primary-redeem transaction kinds.
- crates/execution/src/nav_vault_asset_execution.rs owns their deterministic execution, accounting, replay, and receipt behavior.
- crates/node/src/vault_bridge_workflows.rs builds the source-labeled pfUSDC burn and redemption bundle.
- scripts/a666-pfusdc-reserve-demo.py already builds and verifies the narrow reserve, subscription, entitlement-release, fresh-NAV, route-advance, and redemption sequence.
- scripts/test-a666-pfusdc-reserve-demo.py is the focused offline test for that driver.
- scripts/a666-build-live-nav-mark-ops.py and scripts/a666-build-route-epoch-advance.py build the existing maintenance operations.
- docs/specs/pfusdc-arc-mvp-testnet-spec-20260828.md and docs/specs/pfusdc-arc-tier4-spec-20260828.md define the proof-verified Arc bridge boundary.
- python/postfiat_rpc/testnet_path.py remains the user-facing status source; it must continue to report Z3 as OPEN until the final operator decision.

The current demo driver hardcodes the Ethereum route ID and production identities.
Changing it for Z3 is tooling work only if the existing protocol can express the selected Arc-backed route.
A tool must accept explicit route and asset identities, reject defaults during qualification, fail on overwrite, and write redaction-safe artifacts.
It must never embed a private key or fetch one into an evidence file.

## Meaning of “succeed repeatedly”

Z3 qualification requires **10 consecutive clean integrated cycles across at least seven elapsed UTC days**.
At least one clean cycle must finish on five distinct UTC dates within that window.
The count mirrors the grant proposal's target of at least 10 recurring cycles per week while remaining a bounded testnet campaign.
All 10 cycles must use one unchanged PFTL binary hash, protocol version, route policy hash, Arc chain ID, Arc vault and anchor pair, ingress and egress program keys, and evidence schema.
A governed NAV epoch or route epoch may advance through its existing operation, but its before and after identifiers must be recorded.
No consensus upgrade, emergency patch, manual ledger edit, hidden inventory transfer, or AMM trade is allowed during the window.

A clean cycle must start from an unambiguous state with no stale proof, unexpected reservation, export entitlement, pending withdrawal, or validator disagreement.
It must finish with every intended receipt accepted, all temporary order state terminal, exact supply and reserve deltas reconciled, six validators converged, and the Arc release replay rejected.
A rejected transaction, timeout beyond the declared bound, proof mismatch, reconciliation mismatch, validator disagreement, manual intervention after first submission, or missing artifact makes that attempt unclean.
An unclean attempt is retained honestly, pauses the campaign, and resets the consecutive count to zero after root cause and corrective work are recorded.
An environmental interruption before any transaction submission is recorded but does not count as a cycle and does not reset the count.
Retrying a published request uses its exact identity and must return the original terminal result or reject as a replay.

Each cycle packet must contain:

1. A manifest with cycle number, UTC start and end, release ID, source commit, binary hash, chain and route identities, policy hashes, proof keys, account identifiers, amount, and every artifact SHA-256.
2. A preflight proving six-validator agreement on finalized height, block ID, state root, route state, NAV state, asset state, and empty relevant queues.
3. The Arc deposit transaction, status-1 receipt, block and log position, deposit ID, exact USDC balance delta, and finality input.
4. The ingress witness and proof reports, public-values and proof hashes, PFTL propose/finalize/claim transaction IDs, accepted receipts, and exact pfUSDC mint delta.
5. The A666 reservation and pftl_uniswap_primary_subscribe_v2 transaction IDs, accepted receipts, quote inputs, and exact A666 supply, user balance, settlement reserve, and spread deltas.
6. The entitlement-release receipt proving zero remaining export entitlement without a supply or reserve change.
7. The fresh reserve packet, finalized NAV receipt, governed route-epoch receipt, and exact proof that the same-cycle reserve is counted once.
8. The pftl_uniswap_primary_redeem transaction ID and accepted receipt, with exact A666 retirement and pfUSDC return deltas.
9. The pfUSDC burn receipt, withdrawal packet and nullifier, egress proof and public-values hashes, Arc status-1 release receipt, exact wallet and vault deltas, and replay rejection.
10. A final six-validator convergence snapshot and machine verdict for NAVCoin supply, pfUSDC issued/count/redeemed totals, settlement reserve, source-vault balance, order state, and pending egress state.

## Bounded execution gates

### Gate G0 — lock the read-only baseline

- [x] Read the full deferred NRRS specification and apply section 2.2 precedence.
- [x] Reconcile the 2026-09-02 Arc deposit, mint, burn, release, and replay evidence without merging its branch.
- [x] Record that the Arc packet proves one bridge cycle but no NAVCoin swap and no repeatability.
- [x] Freeze a manifest of the exact evidence files and hashes used as the regression baseline. 2026-09-23: 133 files, 23 in-repo blobs at their cited commits and 110 server-only files, with size and SHA-256 ([manifest](../../status/z3-g0-evidence-manifest-20260923.md#manifest)).
- [x] Confirm that no evidence artifact contains a secret or forbidden field. 2026-09-23: `scripts/public-secret-scan` passed and the targeted scan found no secret or key material. The plan and NRRS name no forbidden-field list, and only path or placeholder references were found ([scan](../../status/z3-g0-evidence-manifest-20260923.md#secret-and-forbidden-field-scan)).

G0 performs no network call and no chain mutation.

### Gate G1 — operator authorizes the bounded SHADOW envelope

- [x] The operator gives an explicit go for preparation and names the qualified PFTL release lineage. 2026-09-23, decided by this lane; the other lane is informed in the 2026-09-23 handoff: preparation may proceed. The lineage is the qualified September 22 build: executable build source `1a0989ad` and node executable SHA-256 `e7bb1afa17b4c6322ac8eadabdab570595ad778a5173a5516c09ba966ed9e4b1`. Its qualification packet is `deployments/release-repair-20260922/` on `release/combined-devnet-20260915`. The deployment directory `deployments/combined-devnet-20260923/` is being prepared today; the build is not yet deployed.
- [x] The operator names the exact Arc testnet pair after full contract and route read-back. 2026-09-23, decided by this lane; the other lane is informed in the 2026-09-23 handoff. These are the values read back on 2026-09-22 in the [cycle-1 inputs](../../status/z3-cycle1-inputs-20260922.md#fixed-identities-and-remaining-envelope-inputs): chain `5042002`; anchor `0x92390d3a2102cb74e4746c05b4d91f61093475d0`; vault `0x160307f3efead79b6a3629c4b8d90e8301fc250f`; verifier `0x1d436908516d15e3c55a936899b47a885e047f27`; USDC `0x3600000000000000000000000000000000000000`; route `pfusdc-arc-testnet-tier4-epoch9`, EVM binding `0xd9e0cd409c5d1e118d65c78ee059adcbba937616353e9675e350d52ee8d498b2`. Re-read them at preflight on the cycle day.
- [x] The operator confirms the existing A666 primary route or an existing-operation-only testnet configuration that may accept the Arc source-labeled pfUSDC. 2026-09-23, decided by this lane; the other lane is informed in the 2026-09-23 handoff: use the existing A666 primary route `pftl-a666-ethereum-wA666-usdc-v1`, with no new testnet configuration. The NAV-profile compatibility blocker in the [cycle-1 inputs](../../status/z3-cycle1-inputs-20260922.md#fixed-identities-and-remaining-envelope-inputs) stays with the other lane, which owns the governed profile and the fresh proofs.
- [x] The operator sets the per-cycle faucet/test value cap and the seven-day campaign window. 2026-09-23, decided by this lane; the other lane is informed in the 2026-09-23 handoff: the per-cycle cap is 2,000,000 source atoms of testnet USDC (2 USDC), the amount already used by the dry runs and the inputs record. The seven-day campaign window opens on the UTC day of the first live cycle and is stamped in the cycle-1 packet on that day. The seed window 2026-09-18–2026-09-25 lapses because the deployment is blocked.
- [ ] The operator provides or controls the funded test wallet and signing flow without disclosing key material to evidence. 2026-09-23: the Arc side is the server wallet `0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB`, with keystore `~/.foundry/keystores/arc-testnet-server` and password file `~/.postfiat/arc-testnet-server.password` (paths only, never contents), funded with 20 testnet USDC on 2026-09-17. On the PFTL side, the holder/owner, pfUSDC issuer, A666 issuer/NAV finalizer and NAV reserve submitter keys are only on the other lane's machine ([signer table](../../status/z3-cycle1-inputs-20260922.md#signer-bindings-and-file-presence)). This item stays with the other lane (first asked 2026-09-22) and remains open.
- [x] The operator confirms that this authorization covers testnet/devnet only and does not authorize mainnet or production value. 2026-09-23, decided by this lane; the other lane is informed in the 2026-09-23 handoff: confirmed for Arc testnet chain `5042002` and `postfiat-wan-devnet-2` only. This lane authorizes no mainnet or production value.

G1 is the first required operator authorization.
Nothing after G0 begins without it.

### Gate G2 — prove route compatibility offline

- [x] Pin the integrated source commit and verify that the selected qualified lineage contains the required Arc code or an operator-approved equivalent. Candidate `15126ac3` and recorded qualified node source `1c435f4f` verified 2026-09-17; G1 lineage selection remains open ([source verdict](../../review/z3-g2-route-compatibility-20260917.md#g21-pass-candidate-source-pinned-and-required-code-present)).
- [ ] Read back or replay the selected route, asset, proof-profile, policy, NAV, and source-domain identities from frozen fixtures.
- [x] Prove that Arc source-labeled pfUSDC can fund the existing primary subscription and redemption operations without a new transaction kind or facility. Existing source selection and route maintenance supported 2026-09-17; the driver still needs G3 changes ([compatibility trace](../../review/z3-g2-route-compatibility-20260917.md#g23-pass-existing-operations-can-settle-the-governed-arc-source)).
- [x] Prove that the route counts the same-cycle reserve exactly once and that its settlement asset ID cannot be substituted. Code trace and focused offline tests passed 2026-09-17; exact Arc cycle composition remains G3 work ([accounting and binding](../../review/z3-g2-route-compatibility-20260917.md#g24-pass-reserve-counted-once-settlement-identities-cannot-be-substituted)).
- [x] Stop for the operator if compatibility requires consensus code, a generic NRRS facility, a new bridge contract, or a new settlement-price format. No prohibited expansion required by the candidate path, 2026-09-17; pair and lineage decisions remain with the operator ([stop verdict](../../review/z3-g2-route-compatibility-20260917.md#stop-verdict)).

The 2026-09-17 [identity comparison](../../review/z3-g2-route-compatibility-20260917.md#identities-read-back) records both Arc pairs without selecting one. G2's selected-state readback remains open; no G1 authorization or integrated live cycle is claimed.

### Gate G3 — complete focused tooling and local tests

- [x] Parameterize or wrap scripts/a666-pfusdc-reserve-demo.py for explicit route, asset, and account inputs while preserving fail-on-overwrite behavior. 2026-09-17, `90fbc5da` (explicit-identity checkpoint); current NAV schema and source-custody bounds verified by the focused offline driver tests.
- [x] Compose the existing Arc deposit/mint and burn/release commands around the primary-route driver. 2026-09-17, `f23f77b9`; scripts/z3-cycle-dry-run.py composes 39 individually stopped commands and refuses Arc execution without the qualified lineage.
- [x] Add one machine-readable cycle manifest and verifier covering all receipts and conservation identities. 2026-09-17, `f680d78a`, `f23f77b9`; python/postfiat_rpc/z3_cycle.py audits ten sections, artifact hashes, finality/receipts, exact deltas, reserve counting and terminal state.
- [x] Add focused success, stale-proof, wrong-route, wrong-asset, duplicate, replay, active-entitlement, insufficient-capacity, and partial-artifact tests. 2026-09-17, `f23f77b9`; python/tests/test_z3_cycle.py and python/tests/test_z3_composition.py, 32 passed.
- [x] Run the focused Python and affected Rust tests only; no Orchard suite is required because this route is transparent. 2026-09-17, `f23f77b9`; python/tests: 567 passed, 3 skipped, 103 subtests; driver: 25 passed. Final Z3 rerun: 32 passed. No Rust code affected; prior focused Rust evidence is in G2.
- [x] Produce a dry-run command sheet with hard stops before every future Arc or PFTL submission. 2026-09-17; [command sheet](../../runbooks/z3-cycle-dry-run.md) documents `f23f77b9` commands, explicit operator inputs and all stops. Offline tooling only; G1/G2 selected-state work and G4 authorization remain open.

G3 is offline and creates no live transaction.

### Gate G4 — operator authorizes one integrated testnet cycle

Read-only preflight reference: [2026-09-17 fleet, Arc pairs, wallet, and missing operator inputs](../../status/z3-preflight-20260917.md).
Dry-run reference: [2026-09-21 release wrapper: cycle 0 blocked; command interface review](../../status/z3-dry-run-20260921.md).
Cycle-1 input inventory: [2026-09-22 values, provenance, signer/prover bindings and remaining prerequisites](../../status/z3-cycle1-inputs-20260922.md).
Evidence baseline manifest: [2026-09-23 file hashes and secret/forbidden-field scan](../../status/z3-g0-evidence-manifest-20260923.md).

- [ ] The operator reviews the G2 compatibility result, G3 tests, dry-run amounts, stop conditions, and exact wallet cap.
- [ ] The operator explicitly authorizes one Arc-testnet/PFTL-devnet integrated cycle.
- [ ] Run one cycle only after fresh six-validator, route, NAV, balance, capacity, proof-key, and contract read-backs all agree.
- [ ] Stop on the first unexpected result; do not repair state manually or continue into the next value transition.
- [ ] Verify and publish the complete redaction-safe cycle packet.

One clean G4 cycle proves integration, not repeatability and not Z3 completion.

### Gate G5 — qualify failure and recovery before repetition

- [x] Rehearse stale NAV, stale Arc proof, wrong route, wrong asset, duplicate deposit, duplicate subscription nonce, active entitlement, duplicate burn, and duplicate Arc release cases with fixtures or no-value tests. 2026-09-17, `b150b14e`; nine wrapper/driver boundary cases and independent verifier negatives ([G5 scenarios](../../review/z3-g5-failure-rehearsal-20260917.md#failure-scenarios)).
- [x] Rehearse recovery after process interruption using exact request identity. 2026-09-17, `9269278b`; four confirmed-step interruptions reject exact replay without resubmission; pre-submission environmental interruption preserves the count ([recovery](../../review/z3-g5-failure-rehearsal-20260917.md#recovery-scenarios)).
- [x] Prove failures leave balances, supply, reserves, reservations, entitlements, and withdrawals unchanged or in their specified recoverable state. 2026-09-17; `b150b14e`/`9269278b` rehearsals extended with 23-field and itemized-state assertions in the [G5 record commit](../../review/z3-g5-failure-rehearsal-20260917.md#invariants-and-retained-evidence). Synthetic tooling/no-value scope only.
- [x] Record declared latency bounds and a pause threshold for each cycle stage. 2026-09-17: [nine proposed stage limits](../../review/z3-g5-failure-rehearsal-20260917.md#latency-bounds-and-pause-thresholds) recorded; no elapsed September 2 stage measurements in plan/G2. 2026-09-23, decided by this lane: the nine proposed limits are adopted as the declared bounds for cycle 1. They will be revised from measured cycle-1 timings.
- [x] Resolve every failure without a consensus change before asking to start the sustained window. 2026-09-17, `b150b14e`, `9269278b` and this record's focused assertions: [no scenario required a consensus change](../../review/z3-g5-failure-rehearsal-20260917.md#resolution-and-validation). Python tooling only; no sustained-window request or live recovery is authorized.

### Gate G6 — operator authorizes the sustained testnet window

- [ ] The operator reviews the single-cycle packet, recovery packet, unchanged-lineage manifest, value cap, and monitoring plan.
- [ ] The operator explicitly authorizes at most 10 clean cycles during the named seven-day window.
- [ ] Execute until 10 consecutive clean cycles pass or a reset-triggering failure pauses the campaign.
- [ ] Do not extend the window, raise the cap, change the route, or resume after failure without another explicit operator decision.
- [ ] Preserve every failed and successful attempt; never omit an attempt to manufacture consecutiveness.

### Gate G7 — reconcile and decide Z3

- [ ] Verify all 10 cycle packets independently from their hashes and receipts.
- [ ] Prove the unchanged binary, protocol, route policy, contract pair, proof keys, and schema across the complete window.
- [ ] Produce one summary with per-cycle latency, fees, amounts, invariant verdicts, failures, and final conservation totals.
- [ ] Confirm the campaign required no consensus upgrade, manual edit, override, or hidden inventory.
- [ ] Ask the operator to accept or reject the evidence as satisfying Z3.
- [ ] Mark Z3 complete only after that explicit operator decision; otherwise leave it OPEN with the stated gap.

## Dependencies and ownership

| Dependency or decision | Owner | Required evidence |
| --- | --- | --- |
| Permission to begin and every authorization that can cause testnet or live state mutation | Operator | Dated explicit go naming scope, route, cap, wallet, and window |
| Funded or faucet-backed wallet and custody of all signing material | Operator | Redacted account and balance preflight; no key material |
| Qualified PFTL release lineage and unchanged-window eligibility | Operator selects; agent verifies | Release ID, source commit, binary hash, deployment receipt, six-validator identity |
| Arc pair and proof route | Operator selects; agent verifies | Full addresses, code hashes, route binding, chain ID, proof keys, pause state |
| A666 primary route and Arc pfUSDC compatibility | Agent demonstrates; operator approves | Offline fixture replay, policy and asset binding, no-new-consensus verdict |
| Orchestration, focused tests, stop checks, manifests, and receipt verification | Agent | Commands, test logs, hashes, machine-readable verdicts |
| Transaction signing and submission within an authorized window | Operator-controlled signer; agent may orchestrate only within the grant | Accepted receipts and redaction-safe audit trail |
| Z3 closure | Operator | Written acceptance of the G7 evidence summary |

## Stop conditions

Stop before submission if any validator identity, height, state root, route epoch, NAV packet, proof key, contract code hash, source-domain identity, balance, capacity, nonce, or queue state is stale, missing, or inconsistent.
Stop if the exact amount and conservative output have not been calculated from current governed state.
Stop if the wallet lacks confirmed test USDC, gas, PFTL authority, or a recoverable signing path.
Stop if any step would need a consensus upgrade, emergency deployment, manual ledger edit, hidden transfer, new facility, new bridge, or NRRS activation.
Stop after any submitted step that lacks both finality and an accepted receipt.
Do not infer success from an RPC response, proof generation, block inclusion, or source-chain receipt alone.

## Explicit boundaries

- [ ] All future execution is SHADOW and Arc-testnet/PFTL-controlled-testnet only unless a later operator authorization says otherwise.
- [ ] This document itself authorizes no testnet transaction, devnet contact, mainnet action, funded-wallet use, or live value movement.
- [ ] The generic multi-asset NRRS specification remains Deferred and unauthorized for live value.
- [ ] This plan does not implement, activate, partially activate, or claim completion of NRRS sections 8 through 26.
- [ ] This plan does not merge or modify integrate/arc-tier4-current-v2-20260901.
- [ ] This plan does not change or expand the Zellic audit scope, make an audit claim, or treat Z3 evidence as an audit substitute.
- [ ] This plan does not authorize a new consensus transaction kind, smart-contract facility, Arc deployment, Uniswap pool, AMM trade, Orchard path, CCTP route, or mainnet pilot.
- [ ] Passing Z3 would remove only one Gate Zero blocker; Z1, Z2, storage qualification, release gates, and the public-testnet launch decision remain independent.
