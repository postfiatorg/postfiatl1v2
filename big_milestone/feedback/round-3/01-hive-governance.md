# Feedback — Pillar 1 — Hive Chat Governance Module

*Review round 3.*

> **Mean score:** 70/100 · Gemini 4 Neon 68 · Claude Opus 5.5 72
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/01-hive-governance.md), once, with the question below. Run 2026-11-26 22:31 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 68/100

*Model `google/gemini-4-neon` · 22124 output tokens*

#### Summary judgement

This sub-page is unusually well-researched and grounded in the real codebase: it audits specific commits (`204d238`, `5936d4563`), surfaces real implementation constraints (the 512-byte total `payment_v2` memo cap, `HIVE_GROUP_ID` being a hardcoded constant, `board_wanted_items` lacking a draft state, the 200-event relay query limit, the 60-second bot loop, and unused `wallet_sign_payment_v2` bindings), and takes security and verifiability seriously.

However, if handed to engineers as written, **it is likely to stall in delivery, hit several hard technical blockers, and under-deliver on the founder's core narrative**. Its biggest strategic flaw is that it bundles an over-engineered multi-steward bureaucracy (35+ requirements plus every permutation of 12 open questions) into Step 1 while relegating the founder's stated core—*formalizing deterministic post-human governance where uploaded packets actually kick things into gear*—to an optional, manually-triggered, advisory multiple-choice quiz whose compute economics remain unsolved. Technically, it contains concrete bugs around `payment_v2` memo limits for rulings, unsupported off-chain wallet signing for M-of-N approvals, torn-read multi-validator RPC queries, backdated Nostr `recovered` events, and state-machine dead-ends. Economically, it lacks a unit-economics model for dual-host GPU inference, leaves community participants structurally powerless and unincentivized, and assumes governance deliverables fit inside existing board task caps without checking those caps.

---

#### Correspondence to the founder's intention

The sub-page matches the **surface UX of Deliverable 1** closely:
- Goodalexander uploads a protocol-state information packet pinned to IPFS and anchored using the message protocol.
- A fixed public governance schedule exists in Hive Chat.
- A ~30-minute structured public conversation runs with UX toggles, produces a transcript, and outputs minutes and deliverables.
- A blog post explains the schema and methodology, and deliverables feed into Task Node's existing Wanted/board machinery so follow-up is tracked across sessions.

Where it **diverges from the founder's deeper intention**:
1. **The "post-human governance" thesis is muted in Step 1.** The founder's framing is that *"formalizing our deterministic governance framework... is the core of the project"* and that *"once the information packets are acted upon, things actually kick into gear"*, so outsiders see *"an exploration of what truly post human activity looks like."* In Step 1 of this spec, a human writes the brief, chats with a standard non-deterministic LLM facilitator, edits non-deterministic LLM-drafted minutes, manually signs off, creates *unconfirmed* (`proposed`) Wanted items that still require manual confirmation, and optionally kicks off an *advisory-only* multiple-choice ruling that triggers nothing on-chain. That reads much closer to "transparent human-led meeting minutes with an AI assistant" than a post-human primitive kicking into gear.
2. **The GPU migration / deterministic runtime problem is punted.** The founder explicitly flagged: *"The problem: it requires expensive VAST rentals currently. Though we have migrated that -"*. Instead of investigating and formalizing the post-migration deterministic execution path as a first-class part of Pillar 1, the spec leaves the deterministic pipeline behind a generic script interface (`gov-inference-provider`), defers a repeatable runtime to Step 3, and asks the founder in Open Question 8 where it migrated.
3. **Migration-era protocol state is narrowly scoped.** The founder's prompt emphasizes transitioning from the Old Chain to the New Chain and launching Hyperliquid/Solana NAVCoins + PFUSDC liquidity pools. The collector spec (`R5`) looks almost exclusively at `postfiatl1v2` RPCs and Task Node status, omitting old-chain state, validator migration progress, and Solana/Uniswap NAVCoin state.
4. **Heavyweight governance machinery precedes product-market proof.** The founder's user story is lean and direct ("upload packet -> trigger 30-min convo with toggles -> output minutes and deliverables"). The spec surrounds this with M-of-N RBAC steward sets, an embargo/reveal protocol, moderation redaction, pre-read queues, multi-chain migration archives, and a standalone verification CLI—all inside Step 1.

---

#### Technical robustness

**What is strong:**
- Honest threat modeling in the Security Model (explicitly acknowledging that third-party verification proves *consistency* with steward-approved server claims, not ground truth of relay receipt order).
- Good prompt-injection hygiene: two-pass isolated minutes drafting (`R17`), excluding community text from Pass 1 and deterministic ruling inputs (`R27`), and clear provenance tagging (`human` / `agent` / `deterministic`).
- Smart identification of real Task Node bottlenecks: decoupling the facilitator from the 60-second bot loop (`R15`), canonicalizing JSON via RFC 8785 (`R14`), and bundling raw Nostr events so transcripts do not rot when relays prune them (`R13`).

**Where it breaks down technically:**
- **Memo count/byte math fails for rulings and is ambiguous on encoding (`R23`, `R27a`):** `payment_v2` allows at most 4 memos and 512 bytes total. Anchoring a session's rulings "in one payment" fails as soon as a session has more than 4 rulings (or fewer if `payment_v2`'s 256/512 limits count hex characters rather than decoded bytes, or once Step 2's Pillar 2 v5 pointers add SHA3-384 commitments).
- **Missing off-chain wallet signing primitive (`R20`, `R20a`):** M-of-N approval requires M steward *wallets* to sign an off-chain approval record (`pf.governance.approval.v1`), yet the codebase audit only confirms `wallet_sign_payment_v2` in `postfiat_wallet_wasm.d.ts` (and XRPL lacks a standard off-chain JSON signing format). Neither the signing algorithm, domain separation, nor WASM API is specified.
- **Multi-validator snapshot quorum (`R4`) will fail or produce torn reads on a live chain:** Reading `status` before and after 6+ RPC calls across 2+ validator endpoints only checks for a root mismatch *at one height*; on a live chain advancing blocks every few seconds, height will advance mid-collection, producing torn reads across heights unless every RPC method supports an explicit historical height parameter (which is not verified).
- **Backdating & DoS holes in Nostr transcript assembly (`R11`, `R12`):** Admitting `recovered` events based on client-set `created_at` lets a malicious eligible user backdate events during the sweep window—directly contradicting the Security Model's claim that spoofed `created_at` is mitigated. Appending `recovered` events at the end also scrambles Q&A ordering, and recording *every* excluded event in the manifest enables a 16 MiB IPFS manifest-bloat DoS under `members` policy.
- **State-machine dead-ends and crash-recovery gaps:** There is no `Scheduled -> Live` transition when pre-read length is 0, no timeout out of `PreRead` (especially dangerous under `snapshot_only`) or `AnchorFailed`, no idempotency guarantee for `Acting` creating `board_wanted_items`, and no reconciliation when Task Node stamps a receipt (`R10`) but relay publication fails.
- **Unbounded facilitator context (`R15a`) and brittle quote checks (`R19`):** Feeding raw RPC responses into the facilitator without tool-based retrieval risks context overflow and slow turns, while enforcing verbatim steward substring quotes breaks natural confirmations ("Yes, approve that") and post-session edits during `InReview`.

---

#### Economic coherence

The economic section lists cost lines cleanly, but the underlying **incentive and sustainability model is incomplete**:

1. **Unmodeled deterministic compute economics:** Spinning up two H100/H200 hosts per weekly session (52 sessions/year) incurs provisioning, container/model cold-start overhead, and minimum billing windows on top of inference time. Answering 1–3 advisory multiple-choice questions on two rented GPUs every week is expensive per unit of decision value, is funded out-of-pocket in fiat by an unspecified payer ("As the founder decides"), and has no token sink, protocol fee, or treasury funding mechanism tied to it.
2. **Participation cold-start / legitimacy problem:** Community members, validators, and top-30 users are asked to read complex snapshots, submit signed pre-read questions, and show up for 30-minute live sessions. Yet in Step 1, community posts cannot trigger the facilitator (`R15`), are excluded from Pass 1 minutes (`R17`), are excluded from deterministic rulings (`R27`), and have no defined reward or reputation incentive (Open Question 10). Without a clear path for high-signal community input to be formally promoted (e.g., endorsed by a steward or validator quorum) and rewarded, participation will dry up quickly.
3. **Unchecked budget collision on `board_reward_budgets`:** Routing governance deliverables into the five existing public boards means protocol-level deliverables must fit under each board's existing `per_task_cap_pft` (`R19`) and compete against daily board spend (`board_reward_spend`). The spec never checks whether existing board caps are sized for governance deliverables or whether `board_ai_l1_governance` (currently hidden) needs a dedicated treasury budget.
4. **Engineering ROI distortion:** Building every branch of 12 open questions upfront ("Every option is built; the founder picks the values") burns significant engineering capital on unused code paths (dual-chain anchoring, M-of-N approval coordination, embargo/reveal flows, unused expiry/cadence modes) before the first live session validates whether the 30-minute Hive Chat governance loop even works.

---

#### Problems

##### P1 — Massive Step 1 scope bloat and the "build every branch" anti-pattern
- **Severity:** critical
- **Where:** `Technical requirements` (R1–R25, R27–R31), `Dependencies` (Step 1), and `Open questions for the founder` (intro + Q1–Q12).
- **What is wrong:** Step 1 (MVP) includes virtually the entire specification—M-of-N RBAC steward sets (`R20a`), embargo/reveal cryptography (`R7`), public moderation/redaction (`R13a`), pre-read question queues (`R8a`), dual-chain anchoring plus archive-record migration (`R24`), multi-validator quorum collection (`R4`), standalone third-party verification CLI with `--check-relays` (`R30`), and dual-host GPU rulings (`R27/R27a`). Worse, the spec explicitly commits to building *all* branches of its 12 open questions so none block Step 1 (e.g., both old-chain and new-chain anchoring, both `snapshot_only` and `record_missed`, both `publish_unapproved` and `manifest_only`, both N=1 and M-of-N, both steward-signed and runner-signed rulings).
- **Why it matters:** This multiplies engineering, QA, and security-review effort by 2–3×, guarantees dead code paths, and delays getting a live governance session in front of users—undermining the founder's immediate priority of demonstrating public post-human governance quickly.
- **How to investigate:** Map each requirement R1–R31 and each branch of Q1–Q12 to estimated engineering days and code surface area across `tasknode` and `postfiatl1v2`. Identify which items are genuinely required for the founder's first live 30-minute session versus multi-steward/long-term hardening.
- **Suggested direction for the fix:** Split Step 1 into **Step 1a (Founder-ready MVP)** and **Step 1b (Hardening & multi-steward)**. Lock down founder defaults *before* coding (e.g., N=1 steward, single chosen anchor chain, `pre_read = 0` or simple open-at-upload, no embargo/reveal or M-of-N RBAC yet, simple verifier script), and move M-of-N sets, embargoes, moderation redaction, archive migration, and alternative schedule modes to Step 1b/Step 2.

---

##### P2 — Core intention gap: deterministic governance and "things kick into gear" are diluted and deferred
- **Severity:** critical
- **Where:** `Architecture — How authority moves from humans to machines`, `Technical requirements` (`R21`, `R27`, `R28`), `Dependencies` (Step 3), and `Open questions` (Q7, Q8).
- **What is wrong:** In Step 1, the only deterministic/post-human act in the entire governance session is an optional, manually flagged, multiple-choice Verifiable Constitution ruling that is purely **advisory** (Stage 0). Deliverables created by the session do not even open automatically—they sit as `proposed` Wanted items until manually confirmed (`R28`). Meanwhile, the founder's stated #1 bottleneck (*"formalizing our deterministic governance framework... requires expensive VAST rentals currently. Though we have migrated that"*) is deferred to Step 3 and Q8.
- **Why it matters:** First-time visitors and validators encountering a Step 1 session will see a human founder chatting with a standard LLM and manually approving board bounties—not *"an exploration of what truly post human activity looks like"* where packets kick things into gear. And without formalizing the post-migration deterministic runtime now, Step 1 risks enshrining a manual process that never transitions to Stage 1/2.
- **How to investigate:** Inspect `postfiatl1v2/docs/governance/` (especially `deterministic-governance-overview.md`, `verifiable-constitution.md`, `deterministic-governance-agent-plan.md`) and `scripts/gov-inference-provider` plus recent git history/PRs to determine what "we have migrated that" actually moved to, what deterministic profiles currently work, and what safe bounded actions (e.g., opening a capped Wanted item, posting a dUNL/registry proposal, or producing deterministic machine-verified minutes) can actually execute automatically.
- **Suggested direction for the fix:** Make deterministic execution a first-class deliverable in Step 1 rather than a bolted-on advisory step: (1) document the actual post-migration inference setup and cost benchmarks; (2) run at least one concrete **auto-executing bounded action** from the approved packet/minutes (e.g., steward-approved deliverables immediately opening within a dedicated capped governance budget, or a deterministic ruling automatically queuing a Cobalt devnet/shadow ratification proposal) so packets visibly kick things into gear from day one; and (3) define concrete, quantitative promotion criteria from Stage 0 to Stage 1/2 instead of leaving them open-ended.

---

##### P3 — Single-payment ruling anchoring violates `payment_v2` memo limits and leaves encoding/v5 headroom unverified
- **Severity:** critical
- **Where:** `What exists today` (`New chain`), `Architecture — Flow`, and `Technical requirements` (`R23`, `R24`, `R27a`).
- **What is wrong:** `What exists today` states that `payment_v2` accepts **up to 4 memos**, 256 data bytes each and **512 bytes in total**. Yet `R27a` mandates that *"The ruling signer ... anchors a session's rulings in one payment"* and the Flow diagram shows *"one payment with the ruling memos."* Any session with 5 or more rulings immediately exceeds the 4-memo count limit (and at ~102 bytes per memo, 5 memos = ~510 bytes, leaving zero margin). Furthermore, the spec does not verify whether `payment_v2`'s 256/512 caps count **decoded bytes or hex-encoded string bytes** (if hex-encoded, 102 raw bytes = 204 hex bytes, so even the 3-memo close-out payment at 612 bytes would fail!). Finally, in Step 2, Pillar 2's v5 pointer will likely include a 48-byte SHA3-384 hash per memo, pushing 3 memos close to or past 512 bytes.
- **Why it matters:** Anchoring close-out or rulings will fail hard on-chain at runtime, forcing an emergency protocol schema change and breaking explorer indexing.
- **How to investigate:** Inspect `crates/types/src/transactions_mempool_receipts.rs` in `postfiatl1v2` to verify exact `payment_v2` memo count and byte-length enforcement (raw bytes vs hex length, whether `memo_type`/`memo_format` count toward the 256/512 cap), check `server/pftl-pointer.js` for exact `pf.ptr` v4 wire sizes, and check Pillar 2's v5 pointer byte size.
- **Suggested direction for the fix:** (1) Anchor a single `pf.governance.rulings_bundle.v1` manifest CID per session (1 memo total regardless of how many rulings ran), or anchor a single close-out bundle manifest CID referencing approval, transcript, and minutes (1 memo instead of 3); (2) include an explicit worst-case byte-budget table covering both v4 and Pillar 2 v5 pointers verified against `transactions_mempool_receipts.rs`.

---

##### P4 — Off-chain wallet signing for M-of-N approvals (`R20`) is unspecified and likely unsupported by current wallet WASM
- **Severity:** critical
- **Where:** `What exists today` (`New chain`), `Architecture — Who makes each artifact`, and `Technical requirements` (`R20`, `R20a`, `R22`, `R30`).
- **What is wrong:** `R20` specifies that **Approval** is *"a signed record over the packet CID, the manifest and minutes hashes, the transcript and minutes CIDs and the steward-set version, from M stewards with the `approve` role"*, and `Who makes each artifact` states that *"Steward wallets sign the schedule, packet and approval."* However, `What exists today` only verifies that `postfiat_wallet_wasm.d.ts` exposes `wallet_sign_payment_v2`—it does not verify whether an arbitrary-message-signing API exists for `postfiatl1v2` wallets (or old-chain XRPL wallets), nor does the spec define the signature scheme (Ed25519 vs secp256k1 vs post-quantum ML-DSA), canonical signing payload, or domain separation tag so `verify-governance-session` (`R30`) can verify it.
- **Why it matters:** Without a defined off-chain wallet signature standard and WASM API, `R20` cannot be implemented for M>1 stewards, and even for M=1 the standalone verifier cannot verify `pf.governance.approval.v1` independently of the on-chain payment transaction.
- **How to investigate:** Inspect `src/features/wallet/l1/wasm/postfiat_wallet_wasm.d.ts` and the underlying Rust wallet crate in `postfiatl1v2` to see what signing functions and key algorithms exist, and inspect `server/context-publish.js` / client wallet code to see how old-chain XRPL signing works.
- **Suggested direction for the fix:** Either (a) specify the exact wallet message-signing algorithm, domain separator, and required WASM API addition (`wallet_sign_governance_payload`) as a prerequisite, or (b) simplify Step 1 so the Nostr key (already bound to the wallet in `steward_set` / Pillar 6) signs off-chain approvals while the steward wallet signs the on-chain anchor transaction—or, for N=1 in Step 1, let the steward's on-chain close-out transaction itself serve as the approval signature over a single close-out manifest CID.

---

##### P5 — Multi-validator `chain_read` quorum and state-root consistency check (`R4`) break on a live chain; AC 3 overclaims verification
- **Severity:** major
- **Where:** `Technical requirements` (`R4`) and `Acceptance criteria` (AC 3).
- **What is wrong:** `R4` defines `chain_read` as *"the same answer from two or more validator endpoints at a recorded height"* and reads `status` (height, state root) before and after all other chain reads, flagging `inconsistent` only on *"a root mismatch at one height."* Three problems occur here: (1) on a live chain producing blocks every few seconds, `height_after` will routinely exceed `height_before`, yielding torn reads across different block heights without triggering "mismatch at one height"; (2) `R4` does not check whether `postfiatl1v2` RPCs accept a pinned `height` parameter so two validators can actually be queried at the same height; (3) two validators returning semantically equivalent JSON may differ in whitespace, field order, or node-specific metadata, yet `R4` stores only a single `raw_cid` and `raw_hash` per field. Additionally, AC 3 claims the third-party verifier *"checks trust levels"*, which is impossible from IPFS + chain alone without validator-signed responses or state-root Merkle proofs.
- **Why it matters:** The collector will either constantly mark valid snapshots as failing/divergent across validators or silently accept torn reads across heights while giving third parties a false sense of cryptographic proof on `chain_read` fields.
- **How to investigate:** Inspect `postfiatl1v2/docs/rpc/methods.md` and the RPC server handlers to check block time, whether `validators`, `ledger`, `bridge_status`, `navcoin_bridge_supply_status`, and `nav_reserve_proof_status` accept a `height` / `block_hash` parameter, and whether responses are deterministic byte-for-byte across nodes.
- **Suggested direction for the fix:** Check if height-pinned RPCs exist; if yes, query `status` first to pick a finalized height `H` and pass `H` to all endpoints. If not, define a bounded height-drift tolerance (e.g., compare parsed canonical values across two nodes within `ΔH <= 1` and store an array of `{endpoint, height, raw_cid, raw_hash}` per read). Rewrite AC 3 to state honestly that the verifier checks *re-derivation from pinned raw responses and metadata schema*, not cryptographic proof of validator quorum (unless Merkle/state proofs are added later).

---

##### P6 — Security and ordering flaws in Nostr `recovered` events and manifest-bloat DoS
- **Severity:** major
- **Where:** `Technical requirements` (`R11`, `R12`, `R13`) and `Security model`.
- **What is wrong:**
  1. **Backdating exploit:** `R11` includes relay-sweep events as `recovered` if their client-set `created_at` is between `open` and freeze cut-off `T`. Because Nostr `created_at` is self-reported by the client, an eligible user can sign and publish an event *after* `T` (during the sweep window) with a backdated `created_at < T`, injecting post-freeze messages into the canonical transcript. This directly contradicts the `Security model` row claiming *"spoofed `created_at` -> Inclusion rule and receipt order (R11, R12)."*
  2. **Scrambled dialogue:** `R12` places all `recovered` events *after* receipt-ordered events. If a brief network blip causes a steward answer or facilitator prompt to be recovered, the Q&A chronology breaks, confusing both human readers and Pass-1 minutes drafting.
  3. **Manifest-bloat DoS:** `R12` requires listing *every* seen-but-excluded event with a reason code in the manifest. Under `members` posting policy, a compromised or malicious directory member flooding relays with thousands of rate-limited events can inflate the manifest toward the 16 MiB IPFS pin cap and stall freeze/anchoring.
- **Why it matters:** Compromises transcript integrity, breaks readability during the exact outage scenario `R11` is meant to handle, and gives any Hive member a cheap way to DoS session close-out.
- **How to investigate:** Inspect `server/hive-group-worker.js` and `server/hive-group-relay.js` to see how relays handle `since`/`until`, rate limits, and event deduplication, and check typical event sizes vs the 16 MiB cap in `server/context-ipfs.js`.
- **Suggested direction for the fix:** (1) Restrict `recovered` backfill strictly to declared Task Node ingest downtime windows (recorded with server start/stop timestamps) or require steward explicit inclusion during `InReview`, rather than blindly trusting client `created_at` during a post-freeze sweep; (2) render recovered events in chronological context in the Markdown view (clearly badged `recovered`) while keeping receipt-order metadata in the manifest; (3) cap per-pubkey excluded-event entries in the manifest (e.g., record the first N excluded event IDs per pubkey plus a `truncated_count` counter).

---

##### P7 — Incomplete economic model: unfunded GPU compute, unverified board caps, and unincentivized/powerless participants
- **Severity:** major
- **Where:** `Economics`, `Technical requirements` (`R15`, `R17`, `R19`, `R27`, `R27a`, `R28`), and `Open questions` (Q8, Q9, Q10, Q12).
- **What is wrong:**
  1. **No unit-economics or funding source for GPU rulings:** The spec cites an H200 estimate ($2.20–$3.50/host-hour) and admits *"no per-ruling cost exists"*, ignores cold-start/provisioning overhead across two distinct hosts, and leaves funding as *"As the founder decides."*
  2. **Board budget mismatch:** Deliverables are validated against existing `board_reward_budgets.per_task_cap_pft` (`R19`) and draw from existing daily board budgets (`R28`), without checking what those caps currently are or whether protocol-scale governance deliverables fit within them.
  3. **Participant incentive vacuum:** Under `R15`, `R17`, and `R27`, community messages cannot trigger the facilitator, are quarantined out of Pass 1 minutes, and are never passed to deterministic rulings—while participant incentives are punted to Q10.
- **Why it matters:** Unmodeled recurring fiat GPU spend creates operational drag; undersized board caps will cause legitimate governance deliverables to be rejected by `R19`'s own validator; and structurally powerless, unrewarded community members will stop showing up, turning public governance into a solo monologue.
- **How to investigate:** Inspect `server/db/migrations/099_board_manager_v2_decisions.sql` and `165_five_boards_wanted.sql` to read actual `per_task_cap_pft` and daily budget caps across the five boards (and `board_ai_l1_governance`). Inspect `reputation-scoring-h200-results-20260901.md` and `scripts/gov-inference-provider` to measure actual cold-start time, minimum rental cost, and per-ruling cost.
- **Suggested direction for the fix:** (1) Publish a concrete per-session cost table (including two-host cold-start time) and explicitly tie governance compute funding to a treasury/validator/protocol budget line; (2) either unhide and size `board_ai_l1_governance` specifically for governance deliverables with appropriate caps or verify existing board caps before enforcing `R19`; (3) add a lightweight promotion mechanism (e.g., a steward or facilitator can explicitly promote/quote a community question into Pass 1/agenda) paired with an explicit attribution/reputation or small bounty path so high-signal participants have a reason to engage.

---

##### P8 — State-machine dead-ends, non-idempotent `Acting`, and Task Node–relay split-brain
- **Severity:** major
- **Where:** `Architecture — Session states` diagram, `Technical requirements` (`R1`, `R2`, `R8`, `R10`, `R25`, `R28`).
- **What is wrong:**
  1. **Missing `Scheduled -> Live` transition:** `R1` and User Story step 4 explicitly allow `pre_read = 0` so packet upload opens the conversation directly, yet the state machine requires passing through `PreRead` and signing a separate open event.
  2. **Stuck states:** There is no timeout transition out of `PreRead` (e.g., if `snapshot_only` auto-enters `PreRead` at the deadline per `R2` and the steward never opens the session) and no timeout/cancel transition out of `AnchorFailed`.
  3. **Non-idempotent `Acting`:** `R25` makes anchoring idempotent, but `R28` does not specify an idempotency key (e.g., `UNIQUE(governance_session_id, deliverable_index)`) on `board_wanted_items`. A server crash mid-`Acting` risks duplicate Wanted items or permanently stuck `Acting` state.
  4. **Task Node–relay split-brain (`R10`):** Task Node stamps `received_at` and a receipt number and immediately triggers the facilitator (`R15`) *without waiting for relay acknowledgement*. If relay publication fails, external Nostr clients never see the steward's turn (nor does `--check-relays`), and client retries produce duplicate signed events.
- **Why it matters:** These edge cases happen routinely in production distributed systems and will leave live governance sessions permanently wedged or produce duplicate bounties and broken transcripts.
- **How to investigate:** Inspect `server/hive-group-routes.js`, `server/hive-group-relay.js`, and `server/board-wanted.js` to see how message publishing retries and `board_wanted_items` uniqueness constraints are currently handled.
- **Suggested direction for the fix:** Add `PreRead -> Missed` (on open-window timeout), `AnchorFailed -> Expired/Cancelled`, and a `Scheduled -> Live` fast-path (or atomic packet+open UX when `pre_read = 0`); require deterministic idempotency keys on `board_wanted_items` and ruling job creation during `Acting`; and add a persistent outbound relay-publish queue with retries and delivery status tracking for events accepted under `R10`.

---

##### P9 — Snapshot collector (`R5`) assumes unverified `postfiatl1v2` RPCs and omits migration-critical protocol state
- **Severity:** major
- **Where:** `Technical requirements` (`R5`) and `What exists today`.
- **What is wrong:** `R5` specifies minimum `chain_state` sections including `bridge_status`, `navcoin_bridge_supply_status`, `nav_reserve_proof_status`, and Pillar 3 admission verdicts/ratifications, without confirming whether these RPC endpoints are live with real data today versus stubs or not-yet-built Pillar 3 outputs. Meanwhile, `R5` completely omits **old-chain (network 2025) PFTL state**, **validator migration progress (old vs new chain)**, and **NAVCoin Solana/Uniswap/PFUSDC pool state**—which are central to the founder's immediate roadmap.
- **Why it matters:** If `pf-gov-collector` fails or outputs empty/stale stubs for unbuilt RPCs while missing the live old-chain and NAVCoin state the founder actually makes decisions about, the machine snapshot and machine brief will be low-signal at launch.
- **How to investigate:** Inspect `postfiatl1v2/docs/rpc/methods.md` and the live devnet/testnet RPC server to verify which methods in `R5` exist and return non-stub data today; check Pillar 3's spec (`03-validator-program.md`) for when admission verdicts ship; check existing old-chain/NAVCoin monitoring endpoints in `tasknode`.
- **Suggested direction for the fix:** Partition `R5` collectors into **required-at-Step-1** (verified live endpoints: old-chain + new-chain `status`/`validators`/`ledger`, Task Node `/api/system/status`, dUNL round, board budgets/deliverables, Cobalt observations) and **optional/progressive plugins** (bridge/NAVCoin reserve RPCs, Pillar 3 verdicts, Solana/Uniswap NAVCoin pools) that degrade gracefully with explicit `unavailable` status rather than failing the snapshot run.

---

##### P10 — Dependency contradiction between Step 1 independence and the blog post (`R31` / AC 9)
- **Severity:** major
- **Where:** `Technical requirements` (`R14`, `R23`, `R31`), `Dependencies`, and `Acceptance criteria` (AC 9).
- **What is wrong:** `Dependencies` asserts that Step 1 needs *"No other pillar's code"* and puts Pillar 2 (`CANONICAL_JSON` envelopes and v5 pointers) in Step 2. Yet `R31` and AC 9 require a live blog post in Step 1 *"written with Pillar 2's message-protocol post."* Either Step 1 is actually blocked on Pillar 2's spec/blog post, or Step 1 ships a blog post documenting temporary `pf.ptr` v4 + raw JCS JSON that becomes obsolete as soon as Step 2 ships.
- **Why it matters:** Creates a hidden cross-pillar blocker on Step 1 acceptance, or causes public developer documentation to churn immediately after launch—eroding credibility on the exact public surface the founder wants users to read.
- **How to investigate:** Check `02-message-protocol.md` to see how mature Pillar 2's envelope and v5 pointer spec is and whether its blog post is already drafted or blocked on implementation.
- **Suggested direction for the fix:** Either (a) decouple `R31` in Step 1 into a standalone "Post Fiat Governance Protocol" post that clearly separates the permanent governance artifact schemas (`pf.governance.*.v1`, RFC 8785 JCS, SHA3-384) from the transport pointer version (`v4` now, `v5` upon Pillar 2 launch), or (b) pull Pillar 2's `CANONICAL_JSON` envelope header format directly into Step 1 if it is just a thin protobuf wrapper, eliminating the Step 1 -> Step 2 schema migration altogether.

---

##### P11 — Unbounded facilitator context window and missing latency target (`R15`, `R15a`)
- **Severity:** minor
- **Where:** `Technical requirements` (`R15`, `R15a`) and `Open questions` (Q11).
- **What is wrong:** `R15a` states *"The facilitator sees the packet, the snapshot with raw responses, the machine brief and the previous minutes."* Raw RPC responses can easily be hundreds of kilobytes to several megabytes (up to the 16 MiB pin cap per response). Injecting raw responses into prompt context will exceed model context windows, cause severe latency spikes during a tight 30-minute live session, and inflate inference cost. Meanwhile, latency SLA is left blank in Q11.
- **Why it matters:** A 30-minute live public session becomes unusable if the facilitator stalls for 45–90 seconds per turn or errors out on context overflow.
- **How to investigate:** Measure the byte/token sizes of current responses from `/api/system/status`, `validators`, `ledger`, `component-observations.json`, and `board_wanted_items`, and check which LLM models and context limits Task Node's inference stack currently uses.
- **Suggested direction for the fix:** Give the facilitator a compact structured context (packet + extracted snapshot values + machine brief + summary of previous minutes, strictly token-budgeted) plus a deterministic read-only tool (`get_snapshot_field(path)` / `get_raw_response_slice(cid, path)`) to fetch raw responses on demand, and set a concrete default latency budget (e.g., p95 < 10 seconds) directly in `R15`.

---

##### P12 — Rigid verbatim substring quote validator (`R19`) breaks natural dialogue and `InReview` edits
- **Severity:** minor
- **Where:** `Technical requirements` (`R18`, `R19`) and `Architecture — Session states`.
- **What is wrong:** `R19` rejects any decision or deliverable whose `quote` is not a verbatim substring of the cited steward text. In natural structured Q&A, the facilitator asks a detailed question ("Should we allocate 5,000 PFT on Board X to ship Y?") and the steward replies *"Yes, do that."* A verbatim quote of *"Yes, do that"* carries no meaning on its own, while quoting the facilitator's text fails the steward-only quote check. Worse, if the steward refines or splits a deliverable during `InReview`, `R19` either blocks the edit or forces an awkward mismatch between the revised deliverable and the old chat substring.
- **Why it matters:** Will cause frustrating validation rejections right when the founder is trying to review and sign off minutes after a live session, encouraging workarounds or abandoned reviews.
- **How to investigate:** Trace realistic 30-minute Q&A transcripts (steward agreeing to facilitator proposals, steward combining two earlier remarks into one deliverable, steward editing a deliverable during `InReview`) against the validation rules in `R18`/`R19`.
- **Suggested direction for the fix:** Allow a decision/deliverable to cite a `{facilitator_event_id, steward_event_id}` turn pair (storing verbatim quotes from both when the steward is confirming a prompt), and allow explicitly marked `steward_review_edit` provenance during `InReview` (which requires steward signature anyway) so the quote validator catches *unapproved agent hallucinations* without handcuffing *signed human edits*.

## Claude Opus 5.5 — 72/100

*Model `anthropic/claude-opus-5.5` · 12337 output tokens*

#### Summary judgement

This is a strong, well-grounded spec. It turns the founder's Deliverable 1 user story into a concrete, verifiable pipeline:

- a fixed public schedule;
- a versioned state collector with a trust level per field;
- a signed, IPFS-pinned, chain-anchored packet;
- a ~30-minute public session in its own Hive room with toggles;
- a frozen transcript and minutes whose items quote the steward's own words;
- deliverables flowing into the existing Wanted/budget machinery;
- an optional deterministic ruling path.

It is anchored in real code and commit hashes, and it is candid about what exists and what is trust-based.

The main weaknesses fall into four groups:

1. **Scope.** The "MVP" (Step 1 = R1–R25, R27–R31) includes almost everything: M-of-N steward sets, embargo/reveal, moderation, relay sweeps, a third-party verifier, and two-host GPU rulings. It also commits to building every policy branch rather than picking defaults.
2. **Narrative.** The "post-human" story is under-delivered at Stage 0. Humans author, sign, confirm and gate almost everything, and "acting on" the packet mostly produces *proposed* items and *advisory* rulings.
3. **Internal inconsistencies.** These include the snapshot-only path still needing a steward signature, missing state-machine timeouts, ruling memo capacity, and "no other pillar's code" despite Pillar 3 and Pillar 9 data dependencies.
4. **Economics.** Ruling costs and payer are undecided, and the founder's flagged pain (expensive GPU rental) is placed inside the Step 1 acceptance criteria. Reward checks ignore daily and per-user budget caps.

None of these is fatal. Together they lower confidence that this ships quickly and lands as the founder intends.

#### Correspondence to the founder's intention

**What matches well:**

- **Founder's story, near-verbatim.** It includes the IPFS-tagged packet "the same way our existing message protocol works," the blog explanation (R31), a public conversation triggered with toggles (R9), ~30 minutes, a structured Q&A transcript, and minutes and deliverables.
- **"Fixed timeline for governance"** is covered by R1–R3, with no silent skips.
- **"Defined methodology to gather the important information about the protocol state… likely an IPFS upload or information packet"** is covered by R4–R7. The collector with a trust level per field and pinned raw responses is a genuinely good methodology.
- **"Information packets are uploaded, and acted upon… then things kick into gear"** is covered by R27–R29. Deliverables become Wanted items, rulings follow, and the next snapshot reports follow-through.
- **"It has to be public in nature"** is covered by R30. There is a public page without login, a verifier, and labels on every artifact.
- **Deterministic governance as the core, and the VAST cost concern.** R21 keeps the chat layer GPU-free. R27 plugs into the existing Verifiable Constitution pipeline behind a provider interface. Open question 8 asks directly about the half-finished "we have migrated that" sentence.
- **The human→machine authority progression** is a thoughtful extension aligned with "post-human governance is a new primitive."

**Where it drifts:**

- **The visible experience is still founder-centric.** At Stage 0 the founder writes the brief, signs the packet, signs the open, answers questions, edits the minutes, signs approval, may sign rulings, and may confirm rewards. Someone encountering this would see "a founder's structured AMA with an AI secretary," not "what truly post-human activity looks like."
  - The spec does lead the public page with machine-made artifacts (R30). But no Stage 0 element is autonomously machine-triggered and consequential.
- **"Things actually kick into gear" is weak in Stage 0.** Deliverables are `proposed` until confirmed. Rulings are advisory. There is no concrete, guaranteed automatic effect.
- **The founder's simplicity is lost.** The founder described a ~30-minute session with toggles. The founder now has 12 open questions to answer before the first production session, and the toggles are a small part of a large apparatus.
- **Validators and top-30 users** are touched only as allowlist sources. Cross-deliverable integration with Finance Hub and the Information Hive Mind is only implicit. That is acceptable for this pillar.

#### Technical robustness

**Strengths:**

- Honest separation of what is proven (hash and signature consistency) from what is a server-assembled claim (receipt order, eligibility).
- RFC 8785 canonicalisation, with the need for a canonical mode in `pinContextIpfsJson` identified.
- SHA3-384 commitments aligned with Pillar 2.
- Idempotent anchoring with fixed memo bytes.
- Relay sweep with `recovered` events.
- Isolated two-pass minutes drafting against prompt injection.
- Quote-verification validator.
- Per-artifact anchor state.
- Memo-budget arithmetic against real `payment_v2` limits.

**Weaknesses:**

- **State machine gaps.** There is no timeout from `PreRead`, `AnchorFailed` loops forever, and nothing covers a failed packet anchor.
- **Snapshot-only contradiction.** The snapshot-only path still needs the steward's signature.
- **Ruling payment capacity.** One ruling payment can't hold more than four memos, or roughly 512 bytes.
- **`chain_read` assumptions.** It assumes multi-endpoint reads at a fixed height, and validator RPC availability is itself Pillar 3's open problem.
- **Two-host replay determinism.** It is assumed rather than verified for heterogeneous rented hosts.
- **Facilitator latency.** The existing bot takes up to 180 s per reply, which could make a 30-minute session thin.
- **Undeclared cross-pillar dependencies** in Step 1 (Pillar 3 verdicts, Pillar 9 top-30 snapshot).
- **Two artifact formats forever.** v4/JCS-only now and Pillar 2 envelopes later, with "never re-pinned," so the verifier must support both.

#### Economic coherence

**Reasonable:**

- Deliverable rewards reuse the existing board budgets and caps.
- Chat-layer inference reuses an existing budget.
- Anchors are cheap self-payments.
- Rulings share one rental under a hard per-session cap.
- Costs are published per session.

**Weak:**

- **Payer undefined.** The ruling payer is "as the founder decides."
- **Per-ruling cost unknown.** There is no estimate of cold-start or model-load overhead, which likely dominates for 1–3 short questions.
- **The expensive thing gates the MVP.** Acceptance criterion 6 makes a GPU ruling mandatory for Step 1, putting exactly the cost the founder flagged on the MVP's critical path.
- **Budget checks are incomplete.** R19 checks only the per-task cap and the packet ceiling. It ignores the daily cap, per-user caps and remaining budget, so a session can mint more deliverable claims than a board can fund.
- **No participant incentives.** There is no incentive model for participants (Q10) and no account of how governance output creates value for PFT holders.
- These are acceptable as open questions, but there should be recommended defaults.

#### Problems

**P1 — Step 1 scope is too large; "every option is built"**
- **Severity:** major
- **Where:** Dependencies table ("Step 1 (MVP): R1–R25, R27–R31"); R1 ("Every option is built; the founder picks the values"); Open questions preamble.
- **What is wrong:** The MVP includes nearly every requirement:
  - M-of-N steward sets with role bindings;
  - embargo/reveal;
  - moderation;
  - relay sweep and recovery;
  - eligibility lists;
  - a third-party verifier with two modes;
  - two-host GPU rulings;
  - dual-chain anchor interfaces.

  Every policy branch (`snapshot_only` vs `record_missed`, `publish_unapproved` vs `manifest_only`, cadence variants) is implemented.
- **Why it matters:** Time to the first real session, which is the founder's core deliverable, stretches out. Branch combinations multiply test burden and bugs.
- **How to investigate:** Estimate effort per requirement. Identify the minimal path for one founder-led session that is public, pinned, anchored and has minutes.
- **Suggested fix:** Split Step 1 into:
  - **1a:** schedule, collector, packet, session room, transcript, minutes, anchor, public page.
  - **1b:** verifier, relay sweep, moderation, embargo, rulings, M-of-N.

  Pick recommended defaults (e.g. weekly `per_dunl_round`, no pre-read, `snapshot_only`, N=1, new chain) and build only those first.

**P2 — The snapshot-only path still requires a steward signature, so governance can still pause**
- **Severity:** major
- **Where:** R2 ("the steward signs its anchor at open"); Session states (`Scheduled --> PreRead: … snapshot-only packet at deadline`); Q3 ("so governance never pauses").
- **What is wrong:** The option sold as "governance never pauses" depends on the steward signing at open. Steward absence yields no anchor and no session. There is also no transition out of `PreRead` if nobody signs the open event.
- **Why it matters:** This is a contradiction in a liveness guarantee. It also undercuts the post-human story, since the machine path is still human-gated.
- **How to investigate:** Trace the state machine for "steward never shows up."
- **Suggested fix:** Either:
  - let a registered runner key (the same one as `anchor_ruling`) anchor snapshot-only packets and open an `agent_mode` session, labelled; or
  - add `PreRead --> Missed/Expired` on open timeout and drop the "never pauses" claim.

**P3 — State machine has missing exits**
- **Severity:** minor
- **Where:** Session states diagram.
- **What is wrong:**
  - No timeout from `PreRead` without an open event.
  - `AnchorFailed ↔ Anchoring` can loop indefinitely with no terminal or escalation state.
  - Packet anchor failure before `PreRead` is not modelled. `Scheduled` only goes to `PreRead` on "packet anchored."
  - `Live` with zero steward events is not handled.
- **Why it matters:** Sessions can get stuck, and the public schedule shows ambiguous states.
- **How to investigate:** Enumerate failure events per state.
- **Suggested fix:**
  - Add a `PreRead → Missed` timeout.
  - Add an `AnchorFailed → Abandoned` state after N retries or a window.
  - Model a packet `AnchorFailed` path.
  - Define behaviour for a session with no steward turns.

**P4 — "One payment" for a session's rulings can exceed memo limits**
- **Severity:** major
- **Where:** R27a ("anchors a session's rulings in one payment"); R23 memo arithmetic (~102 bytes, three per payment); `payment_v2` limit of 4 memos and 512 bytes total.
- **What is wrong:** More than four rulings, and in practice more than about five by bytes, do not fit one payment. The spec does not cap rulings per session or batch them.
- **Why it matters:** Anchoring fails for busy sessions, and the per-session payment counts in R22, Economics and AC2 become wrong.
- **How to investigate:** Compute worst-case ruling memo sizes, including replay-hash references if they are in the memo rather than the pinned ruling.
- **Suggested fix:** Anchor a single pinned **ruling bundle** (one CID listing all ruling CIDs and replay hashes) in one memo, or cap rulings per session at three.

**P5 — Memo-size arithmetic should be verified against actual encodings**
- **Severity:** minor
- **Where:** R23 ("CIDv0 48, session id 38, other fields 8, type and format 8").
- **What is wrong:**
  - CIDv0 text is 46 characters; the field header overhead is assumed.
  - It is unclear whether `payment_v2`'s 256 "data bytes" include memo type and format.
  - The old chain uses upper-case hex, and the new chain lower-case hex. Hex encoding in transport versus binary limits is not discussed.
- **Why it matters:** A miscalculation breaks the "three memos fit" claim and the close-out payment design.
- **How to investigate:** Read `pftl-pointer.js` and `transactions_mempool_receipts.rs` for the exact limit semantics. Build the worst-case test vector the spec mentions now.
- **Suggested fix:** Replace the prose estimate with an exact byte table derived from the encoder, and commit the test vector as part of the spec.

**P6 — The GPU ruling is on the Step 1 critical path despite the founder's cost concern**
- **Severity:** major
- **Where:** AC6; Dependencies Step 1 ("one rental through `gov-inference-provider`"); Economics (rulings row).
- **What is wrong:** The founder's first stated problem is expensive VAST rentals. The MVP acceptance requires a two-host rented-GPU ruling. Per-ruling cost, including spin-up and model load, is unknown ("no per-ruling cost exists").
- **Why it matters:** The MVP is gated on an unpriced, possibly slow and flaky external dependency. Each session could cost far more than its chat layer.
- **How to investigate:**
  - Measure cold-start time and cost for one two-host Verifiable Constitution run with 1–3 questions on RunPod H100 and Vast.
  - Clarify the founder's "we have migrated that."
- **Suggested fix:**
  - Make rulings optional in Step 1a, or batch them across sessions (e.g. weekly with the dUNL round, reusing that rental).
  - Publish a measured per-ruling estimate and default cap before AC6 applies.

**P7 — Two-host determinism on rented heterogeneous hardware is assumed**
- **Severity:** major
- **Where:** R27 ("runs… on two distinct hosts… Matching parsed-output roots give the answer; a split records `hold-no-op`").
- **What is wrong:** Floating-point and kernel nondeterminism across different GPU SKUs, drivers or batch settings can cause frequent splits. That makes rulings default to `hold-no-op` and wastes money.
- **Why it matters:** The flagship "deterministic" output could be mostly no-ops, undermining both credibility and economics.
- **How to investigate:**
  - Check the existing H200 results and replay blog for cross-host agreement rates and pinned hardware requirements.
  - Confirm whether `gov-inference-provider` pins the GPU type and image.
- **Suggested fix:**
  - Require the same SKU, driver and image digest in the ruling profile.
  - Record the hardware fingerprint in the ruling.
  - Report split rates per session.
  - Define a re-run policy on divergence, within the cap.

**P8 — Stage 0 offers no visible autonomous machine action; the "kick into gear" effect is weak**
- **Severity:** major
- **Where:** Authority progression table (Stage 0: rulings advisory); R28 (deliverables `proposed` until confirmed); founder's narrative paragraphs.
- **What is wrong:** At Stage 0, every consequential action is human: confirmation, signature and approval. The founder's narrative is that once packets are acted upon, "things actually kick into gear," and visitors should conclude this is post-human activity. Stage-progression criteria are deferred entirely to the founder (Q7).
- **Why it matters:** The pillar's core purpose (Outcome A) is perception. A human-run meeting with AI note-taking doesn't convey the primitive.
- **How to investigate:** Identify low-risk classes that could be machine-effective from day one. Examples:
  - auto-opening Wanted items under a small reward threshold;
  - auto-posting follow-up status;
  - auto-scheduling the next session from the snapshot.
- **Suggested fix:**
  - Introduce a "Stage 0 binding class": machine-opened Wanted items under X PFT with a steward veto window.
  - Propose default, measurable stage-transition criteria (e.g. N sessions where Stage 1 replay minutes match approved minutes at ≥Y%).

**P9 — Reward validation ignores daily, per-user and remaining board budgets**
- **Severity:** major
- **Where:** R19 (checks only `per_task_cap_pft` and `reward_ceiling_pft`); "What exists today" (`board_reward_budgets` holds daily, per-task and per-user caps).
- **What is wrong:** A session can generate deliverables whose total exceeds a board's daily or remaining budget. That leaves items that can never be paid, or crowds out other board work.
- **Why it matters:** Governance becomes a source of unfunded promises, the opposite of "minutes cannot become write-only."
- **How to investigate:** Read `board-wanted.js` and the budget spend logic for how caps apply at task creation versus payout.
- **Suggested fix:**
  - Validate the sum of session deliverables per board against remaining budget.
  - Make `reward_ceiling_pft` mandatory with a default.
  - Show the budget impact on the review screen.

**P10 — Cross-pillar dependencies are understated in Step 1**
- **Severity:** minor
- **Where:** Dependencies ("No other pillar's code"); R5 (Pillar 3 admission verdicts and ratifications); R9 (top-30 lists from Pillar 9's daily snapshot; validator bindings); R14 and R23 (kind 12 and schema ids agreed with Pillar 2).
- **What is wrong:** Step 1 reads data produced by Pillars 3 and 9 that may not exist yet. Kind and schema registration needs Pillar 2 coordination.
- **Why it matters:** This creates hidden blockers and schedule risk.
- **How to investigate:** Check whether the Pillar 3 verdict publication and Pillar 9 top-30 snapshot exist today.
- **Suggested fix:**
  - Mark those fields and allowlist sources as optional (`absent` with a reason) in Step 1.
  - List the Pillar 2 registry agreement as an explicit Step 1 dependency.

**P11 — Facilitator latency may make a 30-minute session thin**
- **Severity:** major
- **Where:** R15 (latency "published against the founder's target"); "What exists today" (bot replies take up to 180 s).
- **What is wrong:** If replies take 60–180 s, a 30-minute session yields roughly 10–20 exchanges. There is no target, and no design (streaming, pre-drafted agenda answers) to keep it conversational.
- **Why it matters:** The founder's experience of the live session is the product. A sluggish facilitator kills it.
- **How to investigate:**
  - Measure current inference latency for snapshot-grounded answers.
  - Prototype pre-computing agenda questions and candidate answers during pre-read or at packet time.
- **Suggested fix:**
  - Set a default latency target (e.g. under 20 s p50).
  - Pre-draft agenda turns.
  - Stream partial replies, and finalise the signed event at the end.

**P12 — `chain_read` trust level assumes multi-endpoint, height-pinned RPC reads**
- **Severity:** minor
- **Where:** R4 (`chain_read`: "the same answer from two or more validator endpoints at a recorded height").
- **What is wrong:** It is unclear whether `postfiatl1v2` RPC methods accept a height parameter, or whether multiple public endpoints exist. Pillar 3 is still asking "Do we have RPC software? Where is it?" Reads at "latest" across endpoints will race. The before/after `status` check only detects the race; it doesn't solve it.
- **Why it matters:** Snapshots may be frequently marked `inconsistent` or downgraded, weakening the methodology.
- **How to investigate:** Check `docs/rpc/methods.md` for height or state-root parameters, and count publicly reachable endpoints.
- **Suggested fix:**
  - Specify retry-until-consistent within a window.
  - Allow `chain_read` with a single endpoint plus a state-root proof if available.
  - Otherwise, document the degraded level.

**P13 — The Cobalt field will almost always be stale**
- **Severity:** minor
- **Where:** R5 (`cobalt`: from `component-observations.json`, `stale` if observed before the previous snapshot); "What exists today" (Cobalt last observed at height 924 vs fleet at 1906).
- **What is wrong:** It relies on operator-entered records that are already far behind. Cobalt is a headline item for the founder (simulations, Lean proofs).
- **Why it matters:** The snapshot's most interesting section is perpetually stale or operator-attested.
- **How to investigate:** Check whether Cobalt status is exposed via RPC or devnet logs.
- **Suggested fix:** Add a machine collector for Cobalt devnet state, and link Cobalt research artifacts (blog posts, Lean proofs) as cited sources.

**P14 — The anchor chain is left open, though the founder wants the migration**
- **Severity:** minor
- **Where:** R24; Q5.
- **What is wrong:** Old-chain anchoring creates migration debt (the archive record, which the spec does not say who signs or how it is verified) and is invisible to the explorer. The founder explicitly prioritises moving to the new chain and wants explorer visibility across deliverables.
- **Why it matters:** This risks building twice and weakens the public story.
- **How to investigate:** Estimate the effort of the `wallet_sign_payment_v2` flow. It exists in WASM but is uncalled.
- **Suggested fix:**
  - Recommend the new chain as the default.
  - If the old chain is used, specify who signs the archive record and how the verifier checks it against the old-chain hashes.

**P15 — Payment and signature counts are inconsistent across sections**
- **Severity:** minor
- **Where:** R22 ("one packet payment, one open event and one close-out payment, plus the ruling payment"); Economics ("Two payments per session plus one for rulings"); AC2 ("two wallet signatures per anchor chain, plus the ruling payment" while also listing schedule and steward-set anchors).
- **What is wrong:** Schedule and steward-set anchors are separate, occasional payments. Under `snapshot_only` the packet signature moves to open. With M-of-N, approval needs M signatures but only one anchoring payment. The counts are not stated consistently.
- **Why it matters:** This produces acceptance-test ambiguity.
- **Suggested fix:** Provide one table listing, per artifact: signer, key type (wallet or Nostr), payment or event, and frequency.

**P16 — The embargo has no maximum duration, in a deliverable that must be public**
- **Severity:** minor
- **Where:** R7.
- **What is wrong:** Embargoed items publish only a hash and label until a reveal, which may never come. There is also no rule on whether facilitators or minutes may use embargoed content.
- **Why it matters:** It could quietly erode the "public in nature" requirement, and minutes might cite unverifiable content.
- **Suggested fix:**
  - Add a maximum embargo period or a mandatory reveal session.
  - Forbid minutes from citing embargoed bytes, or mark those items unverifiable until reveal.
  - Show the embargo count on the public page.

**P17 — The steward can moderate community input before approval**
- **Severity:** minor
- **Where:** R13a.
- **What is wrong:** This is a public action, but the same party who authors the decisions can remove dissenting community content from the bundle.
- **Why it matters:** It is a legitimacy risk for "governance in public."
- **Suggested fix:**
  - Retain the moderated content's hash (already done) plus an encrypted or escrowed copy.
  - Limit moderation to listed reason codes.
  - Show moderation counts prominently.
  - Consider requiring a second party once M>1.

**P18 — The verifier must support two formats indefinitely**
- **Severity:** minor
- **Where:** R14 ("Past sessions are never re-pinned"); R23 (v4 → v5).
- **What is wrong:** Step 1 artifacts are raw JCS JSON with v4 pointers, and Step 2 artifacts are Pillar 2 envelopes with v5 pointers.
- **Why it matters:** Maintenance burden and verifier complexity.
- **Suggested fix:** Define the verifier's format-dispatch rule now, or wait for the Pillar 2 envelope if it lands soon, to avoid the dual format.

**P19 — The founder must make too many decisions without recommended defaults**
- **Severity:** minor
- **Where:** Open questions 1–12.
- **What is wrong:** Twelve multi-part questions, all required "before the first production session," with no recommendations.
- **Why it matters:** This creates founder bottleneck and delay, and conflicts with the desire for a simple user story.
- **Suggested fix:** Give a recommended default and rationale per question. The founder then only overrides.

**P20 — The non-deterministic chat layer is not stated in the public narrative**
- **Severity:** minor
- **Where:** R21 (minutes on Task Node's existing inference stack).
- **What is wrong:** The facilitator and minutes drafts likely come from non-replayable hosted models. They are labelled "agent," but the public may conflate them with "deterministic governance."
- **Why it matters:** Credibility of the post-human primitive.
- **Suggested fix:** State in R30 and R31 that agent artifacts are non-replayable until Stage 1, and record the model id and version per agent artifact.

**P21 — Board ownership and incentives are unresolved**
- **Severity:** minor
- **Where:** Q10; Q12; Economics.
- **What is wrong:** There is no default owning board (`board_ai_l1_governance` is hidden) and no participant incentive. Community participation, which is part of making this public and alive, has no reason to happen.
- **Why it matters:** Empty rooms make for a weak public demonstration.
- **Suggested fix:**
  - Recommend routing each deliverable to the board for its subject.
  - Recommend small Wanted rewards for pre-read questions the minutes cite, capped per session.
