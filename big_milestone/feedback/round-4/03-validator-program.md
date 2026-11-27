# Feedback — Pillar 3 — Validator Rewards Program

*Execution-hole audit · Round 4.*

> **Mean score:** 72/100 · Gemini 4 Neon 70 · Claude Opus 5.5 74
>
> **Method:** each model received (1) the founder's original prompt, (2) locked founder decisions & live state, (3) master `README.md`, and (4) [this pillar's spec](../../pillars/03-validator-program.md), once, scored across 5 explicit rubric dimensions (Execution Holes /30, Economics /20, Founder Alignment /20, Milestones & 4 Gates /15, Readability /15). Run 2026-11-26 23:35 UTC via OpenRouter.

## Gemini 4 Neon — 70/100

*Model `google/gemini-4-neon` · 14845 output tokens · `tech=15/30 econ=12/20 align=18/20 ship=12/15 read=13/15`*

#### Summary judgement

Pillar 3 is clearly written, well aligned with every locked founder decision, and properly scoped around turning existing `postfiat-node` primitives into a clean outside-validator product with concrete milestones and all four ship gates. It answers the founder's RPC question directly, keeps key custody separated by default, and rightly closes the current loophole where binding a validator once grants permanent Finance Hub access.

Before engineering starts building, however, several serious execution holes will break live decentralization or stall rollout. Three are critical: measuring 95% uptime strictly from finalized `precommit_qc.votes` will systematically punish geographically distant validators in BFT consensus; the 25-of-30 rolling window makes it mathematically impossible to recover within the 14-day grace period after any 6-day outage (and leaves Day-0 validators unserved); and enabling public RPC plus `PFSH03` private-action admission directly on consensus-signing validators exposes consensus liveness to public DoS while leaving today's 7–10s read latency undiagnosed. Closing these alongside the upgrade-rollback guard trap, single-key release SPOF, 2-observer split-brain, and per-operator Sybil/re-bind loopholes will make the spec genuinely build-ready.

#### Rubric breakdown

- **Execution Holes & Technical Robustness (15/30):** Good base architecture, but undermined by three critical BFT/state-machine/DoS bugs (`precommit_qc` quorum bias, unrecoverable grace-window math + missing Day-0 warmup, public ZK-admission on validators) and major upgrade-rollback, 1-of-1 release signing, 2-observer quorum, and key-custody gaps.
- **Economic & Incentive Coherence (12/20):** Stake-free admission with real off-chain utility perks and active-liveness enforcement is coherent, but permissionless Pillar 6 identities plus 2 seats per correlated group leave an easy double-claiming loophole, bearer API-key revocation is unaddressed, and `precommit_qc` bias incentivizes datacenter colocation.
- **Founder Decision Alignment & Zero Open Questions (18/20):** Faithfully implements every locked Pillar 3 decision with zero deferred founder questions; minor deduction for blocking Stage 1 (`G0`) on later-wave NAVCoin swaps (`Z3`) and leaving the Wave 1 vs. Wave 2 Pillar 6 identity gap unresolved.
- **Actionable Milestones & 4 Ship Gates (12/15):** Strong M1–M6 checklist and clean-host rehearsal gate; deducts for omitting hot-key rotation (required when guard state is lost), lacking a concrete RPC read-latency target in M3, and missing observer-quorum/API-key revocation checks in M5.
- **Plain-English Readability & Zero Jargon Bloat (13/15):** Crisp, scannable, plain English throughout with a helpful architecture diagram; only minor slip is dropping unexplained internal gate codes (`Z1`, `Z3`, `C1`, `C4`, `G6`, `D4`) into Step 3 and M2.

#### Execution holes

##### H1 — `precommit_qc.votes` uptime metric systematically evicts geographically diverse validators
- **Severity:** critical
- **Where:** Locked-in design decisions; Section 5 (*Signed multi-observer feed*)
- **Hole:** Daily participation (`>= 95%`) is computed solely from votes included in finalized `precommit_qc.votes`.
- **Why it breaks execution:** In BFT consensus, a block finalizes as soon as the proposer collects the quorum threshold (`2f + 1`) of precommit votes. The fastest `2f + 1` nodes (typically the colocated Foundation fleet plus nearby operators) close the QC every block, routinely omitting valid, slightly higher-latency votes from healthy validators in other regions. Those honest outside validators will sit well below 95% inclusion, lose Finance Hub and `$20/mo` rewards, trigger automatic removal proposals, and force the entire network to colocate in one datacenter.
- **Minimal plain-English fix:** Either record late-arriving valid precommits from block `H` inside block `H+1` (or allow a brief vote-aggregation grace window before sealing the QC) and count both in-block and next-block precommits toward participation, so any online validator within normal WAN RTT scores ~100%.

##### H2 — Rolling 25-of-30 math makes 14-day grace recovery impossible after a 6-day outage (and breaks Day-0 onboarding)
- **Severity:** critical
- **Where:** Locked-in design decisions; Section 5 (*Access state machine*); M6 & Ship Gate 2
- **Hole:** Two edge cases in the `>= 95% over 25 of 30 observed days` rule are broken: (1) a validator with a contiguous 6-day outage cannot mathematically recover during the 14-day grace window, and (2) a newly admitted validator has 0 observed days.
- **Why it breaks execution:** If an honest validator goes offline for Days 1–6, it enters `lapsed (14d grace)` on Day 6 (24/30 good days). Even if the operator fixes it on Day 7 and runs at 100% uptime for all 14 grace days (Days 7–20), Days 1–6 are still inside the trailing 30-day window on Day 20—so it still has 24/30 good days, gets automatically revoked, and triggers a removal proposal until Day 31. Meanwhile, at Day 0, a brand-new validator has 0/30 observed days, so M6 and Ship Gate 2 ("connects to Task Node, verifies Finance Hub unlocks") either fail for 25 days or rely on an undocumented bypass.
- **Minimal plain-English fix:** Add two explicit rules: (1) **Warmup:** newly admitted validators start `active` immediately upon binding and are evaluated over observed days since activation (requiring `>= 95%` on at least `N - 1` days during the first 7 days, scaling to 25 of 30 once 30 days exist); (2) **Grace recovery:** achieving `>= 95%` daily participation on 7 consecutive days during the 14-day grace window clears `lapsed` back to `active` without waiting for the old outage to roll off the 30-day tail (with a cooldown, e.g., at most one grace recovery per 60 days, to block yo-yo gaming).

##### H3 — Public RPC + `PFSH03` private-action admission on consensus validators creates a liveness DoS vector, and 7–10s read latency has no root cause or SLA
- **Severity:** critical
- **Where:** Section 3 (*Packaged RPC node*); M3
- **Hole:** Section 3 enables `--allow-private-action-admission` directly on consensus validators ("on validators, optionally enables `--allow-private-action-admission`") and commits to cutting devnet read latency "well below today's 7–10s" without diagnosing why it is 7–10s or setting an acceptance target.
- **Why it breaks execution:** Verifying public `PFSH03` ZK proofs and serving untrusted public RPC traffic directly on a consensus-signing validator lets an attacker exhaust CPU/memory on those nodes. At launch, Foundation runs 6 validators and the first 2 public RPC/admission endpoints—if those 2 endpoints are consensus validators, DDoSing them stalls `2/6 = 33.3%` of voting power and halts the entire chain. Separately, 7–10s read latency breaks Pillar 5's direct-RPC UX and Pillar 2's FastPay confirmations; without a diagnosed root cause and numeric SLA, M3 can ship still unusable.
- **Minimal plain-English fix:** Require public RPC and `--allow-private-action-admission` to run on dedicated **non-voting sentry RPC nodes** (same `postfiat-node` binary in RPC mode) that verify proofs and rate-limit traffic at the edge before forwarding valid actions to validators over private peering. In M3, add a concrete latency gate (e.g., `p95 < 500 ms` for account/tx reads under target load) and verify the bottleneck before standing up the 2 Foundation endpoints.

##### H4 — `postfiat-node upgrade` data rollback can deadlock against the double-sign guard and exhaust disk space
- **Severity:** major
- **Where:** Section 2 (*Signed releases, upgrades & key safety*); M1
- **Hole:** `postfiat-node upgrade` backs up chain data and "rolls back binary + data together on failure" while never touching the double-sign guard file.
- **Why it breaks execution:** First, copying the full chain DB on every upgrade requires 2× free disk space, risking disk-full failures right before coordinated upgrades. Second, if the new binary commits or signs even one block at height `H_new` before failing a post-restart health check, rolling chain data back to `H_old` while leaving the signer guard at `H_new` causes the restarted old binary to try signing at `H_old + 1`, hit the guard's monotonicity check, and crash-loop permanently. Third, rolling back chain data across a finalized activation height `H` puts the node on the wrong fork.
- **Minimal plain-English fix:** Separate binary rollback from DB rollback: always ship consensus changes with a future activation height `H`, validate the new binary with an offline pre-flight check before stopping the node, back up only config/metadata (or use filesystem snapshots) rather than full DB copies, and ensure a restarted node catches up to the network tip in non-signing mode before asking `validator-signer-serve` to sign.

##### H5 — Single pinned ML-DSA-65 release signing key is a fleet-wide supply-chain SPOF
- **Severity:** major
- **Where:** Locked-in design decisions (*Release signing & upgrades*); Section 2; M1
- **Hole:** `postfiat-node upgrade` verifies releases using a single pinned ML-DSA-65 publisher key, even though Section 2 already requires a second independent builder for reproducible builds.
- **Why it breaks execution:** Compromise of that single key lets an attacker publish a trojanized `postfiat-node` binary that passes `upgrade` verification across every outside validator (enabling coordinated key theft or double-signing). Conversely, losing that key permanently bricks `postfiat-node upgrade` because a 1-of-1 key cannot authenticate its own replacement.
- **Minimal plain-English fix:** Require a **2-of-3** offline ML-DSA-65 manifest signature threshold from day one (e.g., primary release builder + the second reproducible builder you already mandate + one cold Foundation backup key), with rotation executed via a 2-of-3 signed key-rotation manifest.

##### H6 — Per-operator reward anti-farming, cross-chain de-duplication, and bearer API-key revocation have gaps
- **Severity:** major
- **Where:** Locked-in design decisions; Section 5; M5
- **Hole:** Three reward/access enforcement holes: (1) Admission allows up to 2 seats per correlated group and Pillar 6 identities are permissionless to mint, yet nothing binds a validator's on-chain `correlated_group_id` (or `IdentityReceiptV1` old-chain key) to a single Pillar 6 identity/Task Node account; (2) Pillar 3 M1–M4 ship in Wave 1 while Pillar 6 ships in Wave 2, leaving pre-Pillar-6 de-duplication unspecified; and (3) transitioning to `revoked` does not specify active backend invalidation of issued GoodAlexander Research API keys, re-binding cooldowns across billing cycles, or preserving Finance Hub UI access if the user independently holds a Pillar 9 Top-30 seat.
- **Why it breaks execution:** An operator running 2 nodes in one correlated group (or 1 old-chain + 1 new-chain node) can mint two Pillar 6 identities, bind each to a separate Task Node account, and farm 2× `$20/mo` credit and 2× Research API keys; or re-bind a validator to a fresh Task Node account monthly; or keep calling GoodAlexander Research APIs indefinitely with an already-issued bearer key after being revoked.
- **Minimal plain-English fix:** Enforce reward uniqueness in Task Node on `(correlated_group_id OR old_chain_key_from_IdentityReceiptV1)` in addition to `pillar6_identity` (so at most one Task Node account per operator group can claim `$20/mo` or API keys per billing cycle, with a 30-day cooldown on re-binding a validator to a new account). Wire `revoked` to immediately revoke/disable issued Research API bearer keys on the API backend while keeping Finance Hub UI access intact if the user independently qualifies via Pillar 9 Top-30.

##### H7 — 2-observer signed uptime feed has no quorum tie-breaker and drifts across wall-clock boundaries
- **Severity:** major
- **Where:** Section 5 (*Signed multi-observer feed*); M5
- **Hole:** "At least two independent observers compute daily participation" and publish signed feeds, with no aggregation/quorum rule when observers disagree and no deterministic block-height boundary for a "day."
- **Why it breaks execution:** With 2 observers, requiring 2-of-2 agreement means one crashed or lagging observer freezes all new validator bindings and `$20/mo` payouts; accepting 1-of-2 lets one buggy or compromised observer forge uptime or wrongly push operators into grace/revocation. Computing "daily" windows by observer wall-clock time guarantees boundary mismatches between honest observers.
- **Minimal plain-English fix:** Deploy **3 independent observers** with pinned public keys in both Task Node and `postfiat-node top`, evaluate participation over deterministic block-height epochs (anchored to finalized block timestamps/heights so honest observers produce byte-identical numbers), and require a **2-of-3 quorum** before updating reward or revocation state (alerting Foundation immediately if quorum is stale > 1 hour).

##### H8 — `tasknode-bind` key custody and replay fields are unspecified, and hot-key rotation is missing from build milestones
- **Severity:** major
- **Where:** Section 2 (*Key custody default*); Section 5 (*One-command connect*); M1–M2 & M5
- **Hole:** Section 5 does not specify whether `postfiat-node tasknode-bind` signs with the offline master key or the isolated hot consensus signer (`validator-signer-serve`), nor does it define domain separation and replay-protection fields. Separately, Section 2 mandates that operators "must rotate keys" if double-sign guard state is lost, yet on-chain hot-key rotation is missing from M1–M2.
- **Why it breaks execution:** Letting `tasknode-bind` read the hot consensus key directly breaks `validator-signer-serve` isolation; asking `validator-signer-serve` to sign arbitrary server challenges without strict domain separation risks consensus-signing oracle bugs; and without explicit `(chain_id, validator_id, tasknode_account_id, nonce, expires_at)` binding, a captured signature can be replayed across accounts or networks. Meanwhile, the first operator who loses a disk/guard file has no implemented key-rotation path and must be manually removed and re-admitted.
- **Minimal plain-English fix:** Specify that `tasknode-bind` signs exclusively with the operator's **master key** (via offline copy-paste or local keyfile, never the hot signer) over a domain-separated message (`PF-TASKNODE-BIND-V1 || chain_id || validator_id || tasknode_account_id || pillar6_id || single_use_nonce || expires_at`), and add master-signed on-chain hot consensus key rotation (`rotate-consensus-key`) to M2 and the M6 rehearsal.

##### H9 — "Automatic removal proposal" lacks an execution path, and Step 3 leaks opaque codes + circular dependency on NAVCoin swaps
- **Severity:** minor
- **Where:** Locked-in design decisions (*Low participation removal*); Section 1 (*Step 3 — Genesis cohort G0*); M2
- **Hole:** (1) The spec says falling past the 14-day grace window "submits an automatic removal proposal that validators still ratify explicitly," without naming who submits the on-chain transaction, how duplicate spam proposals are blocked, or whether recovery auto-cancels a pending proposal. (2) Step 3 blocks `G0` validator onboarding on unexplained internal codes (`Z1`, `Z3`, `C1`, `C4`, `G6`, `D4`)—including `NAVCoin swaps Z3`, which contradicts `README.md` where outside validator onboarding is Stage 1 / Wave 1 and NAVCoin swaps are a separate downstream product gate.
- **Why it breaks execution:** Without a defined submitter and cooldown/dedup rule, automatic removal either never gets built or becomes a governance spam vector; and leaving `NAVCoin swaps Z3` as a hard prerequisite for `G0` stalls Stage 1 chain decentralization behind unrelated Wave 3/Solana bridge work.
- **Minimal plain-English fix:** Define removal submission concretely (e.g., any active validator or designated governance relayer may submit a `remove_validator` proposal only when backed by a 2-of-3 observer non-participation proof, rate-limited to one open proposal per target validator, and auto-cancelled if the target clears `lapsed` before ratification). Replace the opaque `Z1/Z3/C1/C4/G6/D4` codes with plain-English prerequisites and remove NAVCoin swaps from the `G0` testnet validator launch gate.

## Claude Opus 5.5 — 74/100

*Model `anthropic/claude-opus-5.5` · 7879 output tokens · `tech=18/30 econ=13/20 align=18/20 ship=13/15 read=12/15`*

#### Summary judgement

This is a solid, honest pillar. It correctly answers the founder's RPC question: the same `postfiat-node` binary runs in RPC mode. It implements every locked decision:

- Finance Hub, research API keys and $20/mo credit, paid per operator
- ≥95% participation over 25 of 30 days, with a 14-day grace window
- No stake and no bond
- `G0` admitted via `IdentityReceiptV1`
- Opt-in public RPC, with the Foundation running the first 2 endpoints

It also has concrete milestones and all four ship gates. The current-state table is grounded in real files, and the upgrade and guard-file rules show real operational care.

The weak spots are in the details that decide whether rewards and removal are fair and safe:

- **Uptime metric:** it is computed from inclusion in quorum certificates, which a proposer can censor.
- **Launch arithmetic:** "minimum 12 validators, max 2 per correlated group" conflicts with 6 Foundation nodes.
- **Release signing:** there is a single pinned publisher key with no client-side check of the second builder.
- **Activation upgrades:** there is no notice window or readiness rule, even though upgrades are manual-only.
- **Operator identity:** "per operator" has no defined linkage before Pillar 6 is live.
- **New validators:** there is no rule for how they reach `active`.

None of these require reopening founder decisions. All are fixable with a few sentences each.

#### Rubric breakdown

- **Tech (18/30):** Good upgrade, rollback and guard design. However, the uptime source can be griefed, activation upgrades can halt the chain, the release key is a single point of failure, the bind signature lacks domain separation, and the private-admission endpoints have no DoS limits.
- **Econ (13/20):** Per-operator dedup and no double-dipping are stated. But the operator identity linkage is undefined before Pillar 6, a new validator's path to `active` is undefined, credit during grace is ambiguous, and observer disagreement could flip paid access.
- **Align (18/20):** Fully faithful to the locked decisions, with no open-questions section. The phrase "via Pillar 6 identity *once live*" softens the locked rule that minting an identity is *required* for per-operator rewards.
- **Ship (13/15):** M1–M6 plus 4 gates are clear and testable. However, M5 bundles five deliverables, and the "well below 7–10s" latency target has no number.
- **Read (12/15):** Mostly plain English. Unexplained gate codes (`Z1`, `Z3`, `C1`, `C4`, `G6`, `D4`) and some dense lines slip into jargon.

#### Execution holes

**H1**
- **Severity:** critical
- **Where:** §5 Signed uptime feed; Locked decisions (participation from finalized `precommit_qc.votes`); Low participation removal
- **Hole:** Participation counts only the votes that made it into the finalized quorum certificate (QC). A QC needs just 2/3+ of signatures, and the proposer or aggregator chooses which votes to include.
- **Why it breaks execution:**
  - A malicious or simply fast-quorum proposer set can routinely drop one validator's votes, even though that validator voted on time.
  - Geographically distant honest validators get excluded whenever quorum forms without them.
  - Either way, they drift below 95%, lose Finance Hub and credit, and trigger an *automatic removal proposal*.
  - This is a cheap griefing and censorship vector against outside operators, who are exactly the people this pillar is trying to attract.
- **Minimal plain-English fix:**
  - Count a validator as participating if its valid signed precommit for that height reached the observers before a short timeout, whether or not it landed in the QC.
  - Alternatively, require proposers to include all valid precommits received before finalization.
  - Publish the raw vote evidence so anyone can recompute the result.
  - Never auto-propose removal based only on QC-inclusion gaps.

**H2**
- **Severity:** major
- **Where:** Locked decisions → Max validator set size; §1 Step 3
- **Hole:** The launch minimum is 12 validators with at most 2 seats per correlated group, but the Foundation runs 6 correlated nodes today. The spec does not say:
  - whether the Foundation is capped at 2 (which means 10 outside `G0` validators must exist and be ready before launch), or
  - whether the Foundation is exempt (which leaves one party with roughly 50% of seats and control over liveness and the safety threshold).

  It also never defines what counts as a "correlated group" (same operator, same cloud, same ASN).
- **Why it breaks execution:** Step 3 can deadlock if fewer than 10 old-chain Dynamic UNL operators sign receipts. Or the launch quietly violates its own decentralization rule.
- **Minimal plain-English fix:**
  - State the Foundation seat count at launch and a stepdown schedule. For example: "Foundation holds at most 4 of the first 12 seats and drops to 2 once 14 or more validators are active."
  - Define a correlated group as same operator identity, same hosting provider, or same network (ASN).
  - Add a fallback: if `G0` is short, launch public testnet with fewer seats while the mainnet threshold stays at 12.

**H3**
- **Severity:** major
- **Where:** §2 `postfiat-node upgrade`; Locked decisions (upgrades always manual)
- **Hole:** "Activation at height H" upgrades are manual-only, yet there is:
  - no minimum notice period,
  - no readiness signal, and
  - no rule for how un-upgraded validators are scored.

  Separately, rolling back "binary + data together" after a node has already processed blocks past H leaves the old binary unable to follow the chain.
- **Why it breaks execution:**
  - If more than 1/3 of operators miss the window, the chain halts at H.
  - Operators who upgrade late lose participation days and drift toward grace or removal for something that was a coordination failure, not their fault.
  - A failed post-activation health check rolls a node back into a permanently stuck state.
- **Minimal plain-English fix:**
  - Require at least 14 days' notice for activation releases.
  - Activate only after 2/3+ of validators report the new version in the signed feed; otherwise slip H.
  - Show a countdown in `top`.
  - Allow rollback only before height H. After H, `upgrade` should stop and tell the operator to fix forward.

**H4**
- **Severity:** major
- **Where:** §2 Signed release channel; Locked decisions (one pinned ML-DSA-65 publisher key)
- **Hole:** The manifest is "reproduced by a second builder", but `upgrade` only verifies the single publisher signature. There is also no defined path to rotate the pinned key if it is lost or compromised.
- **Why it breaks execution:**
  - Anyone who compromises the one key can ship a malicious binary that every operator installs through the official command. Manual upgrades do not prevent this.
  - If the key is lost instead, clients have no trusted way to learn a new key, so the release channel is bricked.
- **Minimal plain-English fix:**
  - Make `upgrade` require both the publisher signature and the second builder's matching-hash signature. Both keys are pinned in the binary.
  - Ship one offline backup rotation key from day one. A new publisher key is accepted only if signed by the current key or the backup key.

**H5**
- **Severity:** major
- **Where:** Locked decisions (per operator "via Pillar 6 identity once live"); §5 access state machine
- **Hole:** "One grant per operator" has no enforcement mechanism before Pillar 6 ships, and none is specified for how multiple validator keys map to one operator. Pillar 3 M5 can go live before Pillar 6, since Wave 1 includes Pillar 3 but Pillar 6 is Wave 2. The founder's lock says identity minting is *required* for per-operator rewards.
- **Why it breaks execution:**
  - One person with two admitted seats (allowed under the "2 per group" rule) collects two credits and two API key sets.
  - Credit dedup across old and new chains is unenforceable without a common operator ID.
- **Minimal plain-English fix:** State that `$20/mo` credit and API keys turn on only after the operator has minted a Pillar 6 identity and linked every validator key to it. Until then, only Finance Hub viewing is granted, with each validator key mapped to its `IdentityReceiptV1` old-chain account as the interim operator ID.

**H6**
- **Severity:** major
- **Where:** §5 access state machine; Locked decisions (uptime rule)
- **Hole:** The transition from `pending` to `active` is undefined.
  - A newly admitted validator cannot have "25 of 30 observed days" until about day 25. It is unclear whether access starts at bind, after 1 day, or after 25 days.
  - "Observed day" is undefined: UTC boundaries are not stated, and days when the chain halted or had no finalized blocks are not handled.
  - It is unclear whether the $20 credit accrues during grace.
- **Why it breaks execution:**
  - Engineers must invent the rules, and Task Node and `top` may disagree.
  - A network-wide halt would push every validator into grace at once.
- **Minimal plain-English fix:**
  - New validators become `active` after 3 consecutive UTC days at ≥95%, then the 25-of-30 rule applies to days since admission.
  - Days with no finalized blocks network-wide are excluded.
  - Credit keeps paying during grace and stops at `revoked`.
  - Re-activation after revocation requires 7 consecutive days at ≥95%.

**H7**
- **Severity:** major
- **Where:** §5 Signed multi-observer feed
- **Hole:** The spec does not say:
  - who runs the "2+ independent" observers (if both are Foundation, they are not independent),
  - what happens when observers disagree, or
  - how long a stale feed can freeze revocations. A stale feed "never revokes", so a dead feed fails open forever.
- **Why it breaks execution:**
  - Disagreeing observers make access flip between states.
  - A down feed blocks all new bindings, which stalls Step 3 onboarding.
  - Because revocation is frozen, lapsed operators keep perks indefinitely.
- **Minimal plain-English fix:**
  - Run at least 1 non-Foundation observer by Step 3.
  - Task Node uses the *more favourable* of the observer readings for the operator, and the recomputable vote data is the tie-breaker.
  - If the feed is stale for more than 72h, pause new grants, alert, and resume normal evaluation on recovery.

**H8**
- **Severity:** minor
- **Where:** Overview item 4; §5 `tasknode-bind`
- **Hole:** The spec does not say which key signs the bind challenge (hot consensus key or master key). It also does not require a domain-separation tag or specify what the challenge is bound to (Task Node account, chain ID, expiry). Nothing limits a validator key to one active binding.
- **Why it breaks execution:**
  - A consensus-key signature over attacker-shaped bytes risks signing something a parser could treat as a vote.
  - The same validator key could be bound to several Task Node accounts to resell access.
- **Minimal plain-English fix:**
  - Sign `"postfiat-tasknode-bind-v1" || chain_id || tasknode_account || nonce || expiry` with the master key, or with a dedicated binding key derived from it.
  - Allow one active binding per validator key; a new binding replaces the old one.

**H9**
- **Severity:** minor
- **Where:** §3 Public profile with `--allow-private-action-admission`; Locked decisions (Foundation's 2 endpoints)
- **Hole:** There are no concrete limits on the private-action admission endpoints:
  - rate limits,
  - proof-size caps, or
  - a budget for verification compute.

  The read-latency target is also unspecified.
- **Why it breaks execution:**
  - Shielded proof verification is expensive.
  - Flooding the only 2 Foundation endpoints with junk proofs takes down Pillar 5's default submission path.
- **Minimal plain-English fix:**
  - Set defaults such as: per-IP 10 private submissions per minute, a hard proof-size cap, and cheap format checks before full verification.
  - Run the 2 endpoints on separate hosting providers.
  - Set a published read-latency target, for example p95 under 1s.

**H10**
- **Severity:** minor
- **Where:** §1 Step 3 launch gates
- **Hole:** Validator `G0` launch is blocked on unexplained codes, including "NAVCoin swaps `Z3`", which is owned by another product track.
- **Why it breaks execution:** It creates a cross-track deadlock: outside validator onboarding (Migration Stage 1) waits on NAVCoin swap delivery for no stated reason.
- **Minimal plain-English fix:** Replace each code with a one-line plain description. Drop `Z3` as a validator-onboarding blocker, or state explicitly why NAVCoin swaps must precede outside validators.
