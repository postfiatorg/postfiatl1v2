# Feedback — Pillar 5 — The Task Node Local Experience

*Review round 2.*

> **Mean score:** 72/100 · Gemini 4 Neon 72 · Claude Opus 5.5 72
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/05-local-task-node.md), once, with the question below. Run 2026-11-26 20:38 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 72/100

*Model `google/gemini-4-neon` · 17754 output tokens*

#### Summary judgement

This is a notably well-grounded and candid specification. Unlike many planning documents, it is anchored in concrete repository state (open L1 PRs 232/271/280, `tasknode-apple` PR 39, exact crate and file paths), exposes real bugs in the current prototypes (such as incompatible PQ key derivation between the Mac app and `pftl_pq_walletd`, and `pftl_swapd` still defaulting to `on` in `docker-compose.wallet-public.yml`), and avoids security theater in its *Privacy model* section by stating plainly what stays visible when one operator runs both Task Node and all validators.

However, the document is not yet ready to produce a system that works cleanly in production. Its biggest weakness is **economic and DoS fragility**: pairing a flat 1-atom fee with a hard 4-proofs-per-block cap and zero validator compensation for admission/page serving creates a trivial valid-spam exhaustion attack and guarantees independent validators will leave private admission turned off. Technically, it glosses over several critical protocol and wallet seams—most notably the boundary between FastPay balances and consensus-layer shielding, fixed 2-in/2-out note consolidation and unconfirmed-change locking, stale-anchor expiry versus identical-byte resubmission, ciphertext integrity during page sync, and the undisclosed loss of outgoing transaction history due to the absence of an outgoing viewing key. Finally, in terms of founder intent, ~90% of the spec covers shielded payments while largely punting the other half of Deliverable 5 ("it can do virtually everything the task node can do") to an open question and leaving non-shielded functionality and existing encrypted-memo payments completely uncovered by Acceptance Criteria.

#### Correspondence to the founder's intention

The sub-page closely matches the founder's core privacy mandate:
- **Local proving on MacBooks:** Anchored on native Apple-silicon macOS (`tasknode-apple`) with local PFSH03 proving and a hard boundary (R8, R17) ensuring the traditional Task Node server never receives keys, notes, witnesses, proofs, or shielded actions.
- **Quantum resistance and assurance:** Correctly drops Orchard in favor of post-quantum PFSH03, incorporates the founder's request for multi-model adversarial stress testing (R18) and explicit Lean proof coverage boundaries (R19), and ties activation to Pillar 1 governance.
- **FastPay awareness:** Recognizes the founder's mandate that FastPay remain the default for Task Node payouts (R13).

Where it diverges or falls short of the founder's intention:
1. **"Virtually everything the task node can do" is under-scoped and untested.** The Mac app today lacks Hive, Finance Hub, NFTs, and governance—four of the core pillars in the founder's prompt. R12 merely asks for a parity *classification list*, Open Question 3 defers the architectural choice, and Acceptance Criteria 1–11 test **zero** non-shielded Task Node workflows. A reader executing strictly to this spec could ship a notarized shielded wallet whose chat, tasks, context, rewards, and NAVCoin screens are broken or severely lagging the hosted app.
2. **Existing memo-type private payments (readiness gate #2) lack verification.** R14 asserts that today's encrypted-memo public payments keep working, but *What exists today* never confirms whether `tasknode-apple` actually implements them or how they align with Pillar 2's protobuf+IPFS message spec, and AC 9 only tests memos on *shielded* payments (Open Question 6), not existing public-with-encrypted-memo payments.
3. **Single-operator reality vs. user expectation.** The spec is commendably honest in *Privacy model* that Foundation currently operates both Task Node and all six validators. Yet Step 6 of the User Story displays *"not sent to Task Node"* on the receipt without clarifying who operates validator V, and R9 requires "at least two independent endpoints" without defining how launch proceeds if Pillar 3 has not yet onboarded independent operators.

#### Technical robustness

The cryptographic architecture, process isolation (sandboxed prover helper), and failure-mode thinking (intent persistence, replay/double-spend/invalid-proof acceptance tests, kill-switch rehearsal) are strong foundations. Still, six concrete technical gaps could break reliability or security in real use:

1. **FastPay-to-Shield seam:** User Story Step 3 says a payout "reaches my public balance over FastPay" and AC 8 requires making a FastPay payout private "in one action", whereas R13 says public and FastPay balances are *separate* and shielded actions settle through consensus, not FastPay. It is unverified whether PFSH03 can consume a FastPay balance directly, whether FastPay even carries `pfUSDC`, or whether *Make private* is actually a two-step settlement + shield flow with partial-failure states.
2. **2-in, 2-out UTXO mechanics:** PR 232 fixes every action to two notes in, two out, with fees paid from notes. Spending more than two notes requires chained multi-block consolidations; spending change requires waiting for block finality and page sync; dust smaller than the fee becomes unspendable unless paired. None of this wallet state-machine logic is specified or tested.
3. **Stale-anchor race vs. R7 resubmission:** R7 mandates resending identical bytes on retry. If proving on a base 8 GB MacBook Air takes significantly longer than an M5, or if 4-proofs/block congestion or failover delays inclusion past the anchor window, resending identical bytes will fail forever with `stale anchor`. Re-proving safely without creating confusing conflicting intents is unspecified.
4. **Sync integrity and scale:** R6 verifies rebuilt commitment tree roots against a second endpoint, which does *not* prevent a malicious endpoint from serving valid commitments while withholding or corrupting encrypted note ciphertexts unless ciphertexts are themselves committed in the certified header/page hash. Additionally, R3 specifies no chain-history scale for sync benchmarking, and R5 assumes a birthday-checkpoint backup even though Step 1 promises restore from a bare recovery phrase.
5. **Undisclosed loss of outgoing history:** The Revision notes admit PFSH03 has no outgoing viewing key (OVK), yet the spec body omits this entirely. Clean restores and multi-device use will silently lose all outgoing payment amounts, recipients, and receipts.
6. **Valid-load Cobalt impact & macOS hardening:** AC 5 tests a flood of *invalid* proofs, but never tests sustained *valid* full-capacity blocks (4 × 149 KB ≈ 600 KB/block plus 100–220 ms verification/block) on Cobalt consensus finality and validator gossip. Meanwhile, the Mac client lacks explicit requirements for Hardened Runtime / library validation, disabling iCloud Keychain sync, memory zeroization, Time Machine exclusion, and signed auto-updates/version gating.

#### Economic coherence

This is the weakest dimension of the sub-page. To its credit, the author added an *Economics* section and surfaced proof size (~149 KB), validator CPU (24–55 ms), pruning (~95% body reclamation, permanent nullifier growth), and the lack of validator rewards. However, the economic design as written is **not coherent**:

1. **Zero-cost valid-spam congestion attack:** At 1 atom per action (the proposed starting fee) and a hard cap of 4 PQ proofs per block, private block space is radically underpriced. A 20-bit PoW (~1 ms on a laptop) only slows malformed spam; an attacker with a few dollars can continuously submit *valid* 1-atom self-spends, crowding out 100% of legitimate users while imposing ~600 KB/block of consensus gossip, terabytes/year of archival storage, and permanent unprunable nullifier growth on every validator.
2. **Unfunded validator/RPC mandate:** Validators receive no share of PFT fees (burned) or pfUSDC fees (governance collector) and no Pillar 3 reward credit for running admission or serving historical sync pages. Rational third-party validators will keep `--allow-private-action-admission` off, trapping privacy on Foundation-run servers permanently.
3. **Cross-asset fee mispricing:** Paying fees in the transacted asset (PFT vs. pfUSDC) via slow governance-amended flat integers ignores decimal differences and PFT/USD price volatility, creating fee arbitrage or sudden usability breakage.
4. **Native-app maintenance economics:** Maintaining a 100% native SwiftUI parity client against five rapidly evolving web pillars (Governance, NFTs, Hive Chat, Finance Hub, Hive Mind) creates an unsustainable recurring engineering cost that the spec flags in one sentence without resolving.

---

#### Problems

##### P1 — Fixed 1-atom fee + 4-proof block cap enables trivial valid-spam DoS and unfunded validator load
- **Severity:** Critical
- **Where it occurs:** *What exists today* ("What PR 232's code fixes in place"), *Economics*, *Technical requirements* (R10), *Acceptance criteria* (AC 5), *Open questions* (OQ 8).
- **What is wrong:** The spec pairs a hard throughput ceiling (4 PQ proofs/block, ~600 KB/block) with a proposed flat fee of 1 atom set by slow governance amendment, while giving validators zero fee share or reward credit for serving admission or historical note pages. Meanwhile, AC 5 only stress-tests *invalid* proof floods, not sustained *valid* 4-proof/block load on Cobalt consensus.
- **Why it matters:** A valid self-spend takes ~2.2 s on a laptop and costs 1 atom (negligible fractions of a cent). Any adversary can cheaply monopolize 100% of private block space indefinitely, bloat the permanent nullifier set, and stress Cobalt block propagation (~600 KB/block every block). Simultaneously, third-party validators have a strictly negative economic incentive to enable `--allow-private-action-admission` or serve page sync traffic, keeping privacy centralized on Foundation infrastructure forever.
- **How an agent should investigate:**
  1. Inspect `crates/node/src/pq_payment_actions.rs` and `crates/types/src/shielded_bridge_governance.rs` in PR 232 to check exact fee denomination (atoms per PFT vs. pfUSDC decimals), whether dynamic/priority fees or base-fee adjustments exist, and what byte cap accompanies `PQ_REHEARSAL_PROOFS`.
  2. Check Cobalt block-time and gossip-size parameters in `postfiatl1v2` simulations to quantify the latency/bandwidth impact of sustained 600 KB blocks and archival growth rate at 4 proofs/block.
  3. Cross-check `03-validator-program.md` to see how validator uptime/RPC service is measured and rewarded.
- **Suggested direction for the fix:** Replace "1 atom" with a cost-grounded fee model (pricing 149 KB bandwidth + permanent nullifier bytes + verification CPU, denominated consistently across PFT and pfUSDC or dynamically adjusted in the finalized snapshot). Add an explicit validator/RPC compensation mechanism (e.g., fee split for the admitting/proposing validator and/or Pillar 3 reward weight for audited public RPC + admission availability). Add an Acceptance Criterion testing sustained *valid* 4-proof/block saturation against Cobalt finality latency and pruned/archival disk growth.

##### P2 — Contradictory and unspecified FastPay-to-Shield settlement seam
- **Severity:** Major
- **Where it occurs:** *User story* (Step 3), *Architecture* (missing FastPay), *Technical requirements* (R13), *Acceptance criteria* (AC 8).
- **What is wrong:** Step 3 states *"A task payout reaches my public balance over FastPay. I choose Make private on 100 pfUSDC"* and AC 8 requires *"A FastPay payout is made private in one action."* Yet R13 explicitly says *"Shielded actions settle through consensus, not FastPay. The wallet shows public, FastPay and private balances separately."* The spec never explains how a FastPay balance enters a consensus-layer PFSH03 shield action, nor whether FastPay supports `pfUSDC` at all.
- **Why it matters:** If FastPay balances must first be settled/withdrawn to the consensus public balance before shielding, "one action" is actually a two-transaction pipeline with separate latency, fees, and mid-flight failure states (e.g., settled to public, shield fails/rejected). If FastPay only carries PFT and not `pfUSDC`, User Story Step 3 is factually impossible as written.
- **How an agent should investigate:**
  1. Inspect FastPay implementation and specs in `postfiatl1v2` and `tasknode` to determine: (a) which assets FastPay supports (PFT only, or issued assets like `pfUSDC`?), and (b) how funds move from a FastPay balance to a consensus L1 account usable by `pq_payment_actions.rs`.
  2. Check `tasknode-apple` PR 39 (`core/src/pq_payments.rs`, `mac/Sources/V2/PqPrivateView.swift`) to see what balance source its shield flow currently consumes.
- **Suggested direction for the fix:** Align User Story Step 3, Architecture, R13, and AC 8 with the real protocol mechanics. Explicitly diagram FastPay in the architecture, confirm supported payout assets, and if shielding a FastPay payout requires a two-step FastPay-settle -> PQ-shield pipeline, specify the orchestrated state machine, error recovery, and user-facing fee/latency disclosure rather than pretending it is one atomic chain action.

##### P3 — Fixed 2-in, 2-out note fragmentation, consolidation, and unconfirmed-change locking are unspecified
- **Severity:** Major
- **Where it occurs:** *What exists today* ("Out of scope: every action is two notes in, two out"), *Technical requirements* (R4–R7), *Acceptance criteria* (AC 1, AC 6).
- **What is wrong:** The spec notes that PFSH03 actions are strictly 2-in, 2-out and pay fees from notes, and AC 1 mentions "with dust", yet no requirement defines how the Mac wallet handles spending amounts requiring >2 notes, spending unconfirmed change, or handling dust notes smaller than the fee.
- **Why it matters:** A Task Node user who shields multiple task payouts will quickly accumulate >2 notes. Without automatic note consolidation (and clear UX for multiple proofs, fees, and block-finality waits), users will be unable to spend their full displayed private balance. Likewise, sending two payments back-to-back from a single note's change will fail until block finality + page sync completes, and uncoordinated consolidation creates a glaring timing/metadata fingerprint.
- **How an agent should investigate:**
  1. Read `crates/privacy_binius_wallet/` in PR 232 and `core/src/pq_payments.rs` in `tasknode-apple` PR 39 to check how note selection, change locking, multi-note spends, and dust below fee are currently implemented.
- **Suggested direction for the fix:** Add a requirement covering deterministic note selection, pending-change locking (showing "spendable now" vs. "pending change"), automated or guided multi-step consolidation with fee/time preview, and dust handling (either pairing dust with larger notes or warning when unspendable). Add an Acceptance Criterion proving a user with 5 shielded notes can pay an amount requiring 4 of them cleanly.

##### P4 — "Virtually everything the task node can do" and existing encrypted-memo payments are unverified
- **Severity:** Major
- **Where it occurs:** *User story* (Step 1), *What exists today*, *Technical requirements* (R12, R14), *Acceptance criteria* (AC 1–11), *Open questions* (OQ 3).
- **What is wrong:** Deliverable 5's user story requires a local Task Node that *"can do virtually everything the task node can do"*, and the founder's readiness gate #2 requires compatibility with existing encrypted-memo private payments. Yet R12 only requires writing a feature-status list, OQ 3 leaves the parity architecture unresolved without evaluating the maintenance burden of five rapidly evolving hosted pillars, and AC 1–11 contain zero tests for non-shielded Task Node features or for existing public encrypted-memo payments.
- **Why it matters:** Shipping to these Acceptance Criteria could deliver a working shielded tab inside a broken or obsolete Task Node client, failing half the founder's deliverable and forcing users back to the web app. Additionally, choosing a "bundled static web build" (OQ 3 option 2) requires notarizing and shipping a new Mac release for every minor web UI update to Finance Hub or Hive Chat.
- **How an agent should investigate:**
  1. Audit `postfiatorg/tasknode-apple` (`mac/Sources/`) against `postfiatorg/tasknode` to produce the concrete current feature delta, including whether existing encrypted-memo IPFS payments work natively today.
  2. Check `docs/wiki/architecture/encryption.md` in `tasknode` to see why the Task Node service key is currently a reader on encrypted memos (e.g., server-side task verification/indexing) and whether removing it breaks existing workflows.
- **Suggested direction for the fix:** Define a concrete v1.0 parity bar in R12 (which hosted workflows *must* work natively at launch vs. explicit Phase 2 deferrals) and add Acceptance Criteria verifying core Task Node workflows (auth, chat, tasks, requests, context, rewards, NAVCoin markets) and existing encrypted-memo public payments in the notarized build. Expand OQ 3 with a realistic hybrid option (native wallet/prover/signer core + strictly isolated non-privileged WebView or local-signer bridge for fast-moving read/social surfaces like Finance Hub and Hive).

##### P5 — Undisclosed loss of outgoing transaction history (no OVK) and phrase-only restore gap
- **Severity:** Major
- **Where it occurs:** *User story* (Steps 1–2), *Technical requirements* (R4–R5), *Acceptance criteria* (AC 4), *Revision notes* (Gemini P8).
- **What is wrong:** The revision table notes *"PFSH03 has no outgoing viewing key"*, but this critical limitation is completely omitted from the spec body. Furthermore, User Story Step 1 promises full restore *"from my recovery phrase"*, while R5 assumes restoring from a backup that *"records its birthday checkpoint"*, and R3 sets no chain-size benchmark for rescanning.
- **Why it matters:** Without an OVK (or encrypted outgoing-metadata backup), any user who restores on a new Mac or uses two devices loses all record of who they paid and why—seeing only unexplained balance deductions when old notes are spent and change returns. Separately, a user who loses their Mac and only has their written 12/24-word phrase has no birthday checkpoint; if scanning from genesis is slow or unbenchmarked at realistic note volumes (e.g., 100k–1M notes), recovery becomes unusable.
- **How an agent should investigate:**
  1. Inspect `crates/privacy_binius_wallet/` in PR 232 to verify whether outgoing payment records are stored only in local SQLite/vault state and how fast trial decryption runs per 10,000 outputs on ARM64.
  2. Check how phrase-only recovery without a birthday checkpoint currently behaves in `pftl_pq_walletd` and `tasknode-apple` PR 39.
- **Suggested direction for the fix:** Surface the lack of OVK explicitly in *What PR 232's code fixes in place*, *User story*, and R5, and decide whether to add an encrypted local/exportable history journal or track OVK as a protocol upgrade / OQ 9 item. Distinguish phrase-only restore (with optional user-supplied approximate creation date or genesis scan) from full vault backup, and add a realistic output-count scale target (e.g., 100k and 1M historical outputs) to R3's sync benchmark.

##### P6 — Stale-anchor expiry breaks R7's identical-byte resubmission rule
- **Severity:** Major
- **Where it occurs:** *Architecture* (sequence diagram), *Technical requirements* (R7, R9), *Acceptance criteria* (AC 5, AC 6).
- **What is wrong:** R7 specifies that *"A resubmission sends identical bytes."* However, if a proof is built on a slower Mac (e.g., base 8 GB M1 Air) or delayed by endpoint failover (R9) or 4-proof/block congestion, the finalized anchor embedded in the proof can expire (`stale anchor`, tested in AC 5), making identical-byte retries permanently fail.
- **Why it matters:** Without distinguishing retryable transport/timeout errors from terminal `stale anchor` or `fee changed` rejections—and without defining safe re-proving semantics after anchor expiry—users will hit stuck pending intents that either spin forever or risk double-intent UX confusion if re-proved while the first proof is still valid in another validator's mempool.
- **How an agent should investigate:**
  1. Check `crates/node/src/pq_payment_actions.rs` in PR 232 for the exact anchor validity window (number of finalized blocks or seconds) and typed error codes (`stale_anchor`, `fee_mismatch`, etc.).
  2. Inspect intent-state handling in `tasknode-apple/docs/PRIVATE-PAYMENTS-SPEC-20261125.md` and PR 39.
- **Suggested direction for the fix:** Specify the minimum anchor validity window relative to worst-case base-Mac proving time plus failover/congestion margin. Update R7 to separate transport retries (resend identical bytes while anchor is live) from terminal anchor-expiry recovery (automatically mark old intent expired once chain height passes the anchor window, unlock notes, and allow clean re-proving with a fresh anchor).

##### P7 — Page sync lacks ciphertext-integrity verification, and pool floor ignores per-asset fragmentation
- **Severity:** Major
- **Where it occurs:** *What exists today* (Privacy docs), *Privacy model*, *Technical requirements* (R6, R16), *Acceptance criteria* (AC 3).
- **What is wrong:**
  1. R6 verifies scanned pages by rebuilding note commitment tree roots against a second endpoint. Unless the encrypted note *ciphertexts* are themselves authenticated by the certified block/page commitment (not just the note commitments in the Merkle tree), a malicious endpoint can serve valid commitments (passing the root check) while dropping or corrupting ciphertexts, causing a recipient to silently miss incoming funds.
  2. R16 applies a single anonymity floor (16 candidates, 128 actions from `floor-calibration.md`) before showing the word "private", even though *Privacy model* acknowledges that every shielded payment exposes **its asset** on-chain.
- **Why it matters:** Ciphertext withholding is a silent fund-loss/DoS vector against light scanners. Meanwhile, applying a combined pool floor across PFT and `pfUSDC` overstates anonymity when one asset (e.g., `pfUSDC` or PFT) has high volume and the other has only 3 active users.
- **How an agent should investigate:**
  1. Inspect `crates/node/src/private_action_rpc.rs`, PR 232's page-certification format, and `crates/privacy_binius_wallet/` to check whether certified headers/pages commit to ciphertext hashes or only note commitments.
  2. Review `docs/privacy/floor-calibration.md` to confirm whether its model accounts for multi-asset visibility and 2-in/2-out fixed arity.
- **Suggested direction for the fix:** Update R6 and AC 3 to verify a certified commitment covering *both* note commitments and encrypted note ciphertexts (and add a tampered-ciphertext test to AC 3). Update R16 so the pool-floor check and "Shielded" vs. "Private" label are evaluated **per asset** (separately for PFT and `pfUSDC`), and recalibrate `floor-calibration.md` for PFSH03.

##### P8 — Missing macOS local security hardening, signed update path, and `pftl_swapd` compose cleanup
- **Severity:** Major
- **Where it occurs:** *What exists today* (Validator admission W-1), *Technical requirements* (R4, R11, R17, R21), *Acceptance criteria* (AC 1–11).
- **What is wrong:**
  1. For a local wallet holding spend keys on macOS, the spec omits key runtime/storage protections: Hardened Runtime with library validation (preventing dylib injection into the app or prover), disabling iCloud Keychain sync (`kSecAttrSynchronizable = false`), explicit biometry/passcode Keychain ACLs, memory zeroization of keys/witnesses, and excluding the local vault/intent DB from Time Machine/cloud backups.
  2. There is no requirement for a signed auto-update mechanism or minimum-protocol-version check—critical if a PFSH03 security patch, fee amendment, or kill-switch requires rapid client updates.
  3. *What exists today* flags that `docker-compose.wallet-public.yml` still defaults custodial `pftl_swapd` to `on`, yet no requirement or AC actually disables/removes it from deployment configs (R21 only restricts the Mac app).
- **Why it matters:** A notarized Mac app that accidentally syncs Keychain items to iCloud or allows library injection compromises the entire local-custody premise. Stale desktop binaries cause stuck transactions after governance amendments or kill-switches. Leaving `pftl_swapd` defaulted to `on` in public compose files invites accidental custodial deployment.
- **How an agent should investigate:**
  1. Inspect `tasknode-apple` entitlements, signing scripts, Keychain wrapper, and `pf_app_core` key handling/zeroization.
  2. Inspect `docker-compose.wallet-public.yml` in `postfiatl1v2` to verify where `pftl_swapd` is configured and whether any active flow still depends on it.
- **Suggested direction for the fix:** Expand R11/R4 with concrete macOS security controls (Hardened Runtime + library validation, non-synchronizable Keychain with biometry/passcode ACL, `zeroize` on key/witness buffers, backup exclusion) and a signed update + protocol-version compatibility check. Add a requirement/AC to flip `pftl_swapd` to `off` (or remove it) in `docker-compose.wallet-public.yml` before launch.

##### P9 — Capped-beta risk understatement and unresolved independent-endpoint launch gate
- **Severity:** Minor
- **Where it occurs:** *User story* (Step 6), *Privacy model*, *Technical requirements* (R9), *Open questions* (OQ 1, OQ 5).
- **What is wrong:**
  1. OQ 5 floats a "capped beta after the multi-model review" (before Gate 1 external audit) with a "governance-set limit on pool value", without stating that a Binius circuit soundness bug lets an attacker forge proofs to steal **100% of the cap** (not merely disrupt the beta) and that the cap must be enforced at consensus exit accounting (`total_unshielded <= total_shielded` + pool ceiling), not merely in client UI.
  2. R9 requires submitting across *"at least two independent endpoints"*, yet *What exists today* confirms all 6 validators are Foundation-run with no public RPC. Meanwhile, User Story Step 6 displays *"not sent to Task Node"* on the receipt, which is misleading if validator V is operated by the same Foundation entity.
- **Why it matters:** Decision-makers reading OQ 5 may underestimate the blast radius of an unaudited ZK circuit. And engineers cannot satisfy R9 at launch unless the dependency on non-Foundation Pillar 3 validators is either made an explicit mainnet readiness gate or scoped for devnet vs. mainnet with honest operator labeling in the receipt UI.
- **How an agent should investigate:**
  1. Check PR 232 (`crates/node/src/pq_payment_actions.rs`) to see if consensus tracks cumulative shielded/unshielded pool value caps per asset.
  2. Check `03-validator-program.md` for the timeline/gating of non-Foundation public RPC operators.
- **Suggested direction for the fix:** Clarify in OQ 5 that any pre-audit capped beta requires an on-chain value cap and still risks total loss up to that cap under a soundness bug. Update R9 and Step 6's receipt copy to explicitly name the endpoint operator (e.g., *"sent to Foundation validator V"* vs. *"sent to independent RPC R"*) and state whether ≥2 non-Foundation endpoints is a hard mainnet gate.

##### P10 — IPFS metadata leakage and Pillar 2 alignment gap for encrypted-memo payments
- **Severity:** Minor
- **Where it occurs:** *Privacy model*, *Technical requirements* (R14), *Dependencies* (Pillar 2).
- **What is wrong:** R14 requires existing encrypted-memo payments (pointer memo + encrypted IPFS payload) to keep working and prohibits listing the Task Node service key as a default reader on a private payment's memo. However, the spec does not address *how* the Mac app pins and fetches IPFS payloads (via Task Node? via a third-party IPFS gateway? via a local node?), nor whether Pillar 2's standardized protobuf+IPFS message format is implemented in the Mac app.
- **Why it matters:** If the Mac app fetches or pins encrypted memo CIDs through Task Node or a centralized IPFS gateway, that server learns the user's IP, timing, and exact CIDs being read/written—reintroducing the exact correlation metadata R14 and *Privacy model* aim to control.
- **How an agent should investigate:**
  1. Check how `tasknode` and `tasknode-apple` currently pin/fetch IPFS memo payloads (`docs/wiki/architecture/encryption.md` and `TaskNodeClient.swift`).
  2. Cross-reference `02-message-protocol.md` for the exact protobuf schema and IPFS transport expectations.
- **Suggested direction for the fix:** Specify the IPFS transport path for the Mac app in R14, document the resulting metadata observer in *Privacy model*, and clarify whether existing encrypted-memo payments in the Mac app must adopt Pillar 2's protobuf format at launch.

## Claude Opus 5.5 — 72/100

*Model `anthropic/claude-opus-5.5` · 8138 output tokens*

#### Summary judgement

This is a strong, evidence-grounded sub-page. It identifies the real core of Deliverable 5: shielded transactions proven on the user's Mac, never touching the Task Node server. It ties that core to concrete artefacts:

- tasknode-apple PR 39;
- L1 PRs 232, 271 and 280;
- W-1 admission;
- the gate structure.

It is honest about status: nothing is merged, there is no auditor, and public shielded submission is off. It states the privacy model clearly, including the awkward fact that one Foundation operator runs both validators and Task Node. The requirements on key derivation, idempotent intents, page verification and leak tests are exactly what a team building this needs.

The weaknesses fall into four groups:

1. **Parity is underweighted.** The second half of the founder's story is "it can do virtually everything the task node can do". That is reduced to a parity list (R12) and an open question. There is no acceptance criterion for it.
2. **The economics are thin.** A four-proof-per-block cap, a 1-atom fee, 149 KB proofs and zero validator compensation are listed, not reconciled. Throughput, fee discovery and storage cost are never computed.
3. **Readiness gates are left as open questions.** Several items the founder named as gates have no recommendation: memos ("works with existing memo type private payments"), the submission path, and the audit scope or vendor.
4. **There is no sequencing.** The external audit is the obvious critical path, and the page gives no ordering or critical-path view. That weakens its use in a plan whose stated goal is to maximise the likelihood of success.

#### Correspondence to the founder's intention

**What matches well**

- **Server exclusion.** "I do not want those processed on the traditional task node server" becomes a hard rule (R8), a leak test (R17) and an acceptance criterion (AC 2). This is the best possible reading of the founder's words.
- **Readiness gate.** "Privacy working on local macbooks ideally in our task node app" is treated as a gate for Outcome B, which is correct.
- **Cross-cutting asks carried over.** The page carries the founder's other asks into this pillar:
  - FastPay stays the default for payouts (R13).
  - Memo-type private payments are addressed (R14 and open question 6).
  - The multi-model adversarial review is required (R18).
  - A Lean coverage statement is required (R19).
- **Quantum resistance.** Choosing PFSH03 over Orchard matches the founder's quantum-resistant direction, which appears explicitly in the NFT deliverable.

**What is weaker**

- **"Virtually everything."** The app today lacks Hive, Finance Hub, NFT and governance screens. Hive is where governance (Deliverable 1) is meant to happen in public. Finance Hub (Deliverable 9) is the main validator benefit. The page defers all of this to R12 and open question 3. It has no recommendation, no minimum parity bar and no acceptance criterion.
- **"Works with existing memo type private payments."** This is one of the founder's explicit performance criteria. PFSH03 has no memos, and the page leaves the design open. R14 preserves today's public-payment-plus-encrypted-memo behaviour, which is reasonable as a floor. But the gate is not closed for private payments.
- **"Maximize likelihood of success."** The page says it deliberately carries no timelines. It also gives no ordering. Meanwhile the critical path is long and unstarted: auditor engagement, then the audit, then genesis and hardware qualification, then governance. A planning page should at least name the critical path and what can run in parallel.
- **Agent-native framing.** The founder's framing of the project is "post human" and machine-native. The page treats private payments only as human, Touch-ID-reviewed actions. It does not consider whether agents or Task Node automation can use the local prover.

#### Technical robustness

**Strengths**

- Cheap checks run first: signature, transcript, then proof-of-work.
- The block cap and byte cap are in code.
- A pause keeps exits open, and the exit freeze expires on its own.
- Exact bytes are persisted with an intent state machine (R7).
- The scanner verifies pages against a second, independent endpoint (R6).
- One shared wallet crate serves the app and the daemon (R2).
- A sandboxed prover runs with a pinned code hash.
- The key-derivation mismatch between the app and the daemon was caught (R4).

**Gaps**

- **Verifier cost.** The figure used in Economics (24–55 ms inside a validator) is a PFSH02 number, not a PFSH03 one.
- **Key derivation.** R4 says one derivation must exist but does not choose between the two. The choice matters for the "restore from phrase" story.
- **Launch platform versus Gate 2.** Gate 2 includes "mobile qualification". Platforms are left open (open question 4), so the gate could block a Mac-only launch, or mean something else. The page does not reconcile the two.
- **Rehearsal constants.** `PQ_REHEARSAL_ISSUE` and `PQ_REHEARSAL_PROOFS` are named as rehearsal constants. The page never says whether production will use different parameters, or how they are set.
- **Pool floor.** With a four-proof cap, few users and links between shields and unshields, the pool may never clear the floor of 16 candidates and 128 actions. The UX would then permanently say "Shielded, small pool".
- **Metadata leakage.** The page describes metadata leaks well but proposes no mitigation beyond warnings. Candidates include randomised delay before unshield, amount-splitting nudges, Tor or OHTTP submission, and a relay run by an independent operator.

#### Economic coherence

The Economics section lists facts but does not test them against each other:

- **Throughput.** Four proofs per block is a hard ceiling on private actions. The block time is not stated, so the ceiling in actions per second is never computed. It is never compared with expected payout volume, even though "Make private" is offered on every payout.
- **Fees.** A 1-atom fee under a hard cap gives no price signal. Congestion becomes a queue or a refusal, not a market. Open question 8 asks the founder for a fee level without giving a model.
- **Validator cost.** Each payment costs 149 KB of block space, 24–55 ms of CPU and permanent nullifier growth. Validators get no fee share or reward credit. Their only incentive is Pillar 3, which is circular while all validators are run by the Foundation. Once validators are independent, admission may simply be switched off.
- **Archival storage.** The page cites "pruned nodes reclaim 95%" but leaves archival storage cost unaddressed.
- **Audit cost.** The largest cost is the audit of a novel Binius circuit. It has no budget range, even though it is the gate to everything else.
- **Reach.** Privacy covers PFT and pfUSDC only, which is coherent for launch. But NAVCoins are a near-term priority, and the page does not say whether the pfUSDC series in the pool is the same PFUSDC used in NAVCoin fixes.
- **Funding a shield.** A shield of pfUSDC needs a public PFT fee. A user paid only in pfUSDC therefore needs PFT first. The page does not account for this onboarding friction.

#### Problems

**P1 — Parity ("virtually everything") has no plan or acceptance criterion**
- **Severity:** major
- **Where:** R12, open question 3, Acceptance criteria, and the "What exists today" row for the native Mac app.
- **What is wrong:** The app lacks Hive, Finance Hub, NFT and governance screens. Parity is reduced to a marking exercise, and no acceptance criterion covers it.
- **Why it matters:** This is half of the founder's user story. Hive (Deliverable 1) and Finance Hub (Deliverable 9) are central to the governance narrative and to validator value.
- **How to investigate:** Enumerate the hosted Task Node routes and features. Map each to the native app's state.
- **Suggested fix:**
  - Define a minimum launch parity set, for example Hive read and post plus governance session participation, and Finance Hub read.
  - Recommend an approach for open question 3. A likely option is a bundled signed web build for the non-private screens, with no remote scripts.
  - Add an acceptance criterion: "parity list published, launch set native or bundled."

**P2 — Private throughput and fee market not modelled**
- **Severity:** major
- **Where:** Economics, and "What PR 232's code fixes in place" (Capacity).
- **What is wrong:** Four proofs per block and a 1-atom fee are stated with no computed private throughput and no congestion behaviour.
- **Why it matters:** If payouts plus "Make private" exceed the cap, users see refusals or queues. The anonymity set also grows slowly.
- **How to investigate:** Find the block interval on devnet-2. Compute the maximum private actions per hour. Compare with Task Node payout counts per day.
- **Suggested fix:**
  - Add a capacity table.
  - Propose a fee rule that rises with demand, or a priority rule.
  - State the conditions under which the cap is raised.

**P3 — Validator compensation for admission is absent**
- **Severity:** major
- **Where:** Economics ("Who earns") and open question 8.
- **What is wrong:** Validators bear the CPU, bandwidth, 149 KB-per-action storage and nullifier growth, but earn nothing for it.
- **Why it matters:** Once validators are independent, admission may be switched off. The whole design depends on admission being on.
- **How to investigate:** Estimate the per-action cost to a validator. Coordinate with the Pillar 3 reward design.
- **Suggested fix:** Recommend a default, such as a share of the pfUSDC collector or a Pillar 3 reward credit for admission uptime. Cross-link the two pillars explicitly.

**P4 — Memo gate left unresolved**
- **Severity:** major
- **Where:** R14 and open question 6.
- **What is wrong:** The founder named "works with existing memo type private payments" as a readiness gate. PFSH03 has no memos, and the page gives no recommendation.
- **Why it matters:** The readiness gate cannot close without a decision. Two of the three options (a circuit change, or a timing-linkable side channel) have large consequences.
- **How to investigate:** Check what fraction of current task payments carry memos and what those memos contain.
- **Suggested fix:**
  - Recommend launching with R14 (public payment, encrypted memo) plus a Pillar 2 message carrying a randomised delay.
  - Schedule an in-envelope memo for a post-audit circuit revision.

**P5 — Metadata exposure to a single operator, with no mitigation**
- **Severity:** major
- **Where:** Privacy model and open question 1.
- **What is wrong:** The Foundation runs all validators, the gateway and Task Node. Both submission options leak IP and timing to the same entity. Only warnings are proposed.
- **Why it matters:** "Not processed on the Task Node server" is met literally, but the operator can still correlate activity. This undermines the credibility of the claim.
- **How to investigate:** Review `wallet-proxy/private-actions.js` and the admission routes for support for Tor, OHTTP or a third-party relay.
- **Suggested fix:**
  - Recommend OHTTP or Tor-capable submission.
  - Seek at least one non-Foundation admission endpoint before the app uses the word "private".
  - Add an optional randomised delay before unshield.

**P6 — No critical path or sequencing; audit unstarted**
- **Severity:** major
- **Where:** Status block, Dependencies, and open question 10.
- **What is wrong:** There is no ordering. The audit has no vendor, scope or budget, yet it gates everything else.
- **Why it matters:** The founder's question is about maximising the likelihood of success. The audit is the long pole.
- **How to investigate:** Get audit lead-time and cost quotes from Binius- or ZK-capable firms.
- **Suggested fix:**
  - Add a phase ordering: merge PRs → multi-model review → engage auditor in parallel → W-1 on devnet → capped beta (open question 5) → audit closes → activation.
  - Recommend starting auditor engagement immediately.

**P7 — Economics uses PFSH02 verifier numbers**
- **Severity:** minor
- **Where:** "PFSH03 numbers" row, and Economics ("24–55 ms").
- **What is wrong:** Verifier timings are PFSH02 figures, applied to PFSH03.
- **Why it matters:** Validator cost and the flood-resistance claims may be off.
- **How to investigate:** Benchmark the PFSH03 verifier on the PR 232 and PR 280 branches, standalone and inside a validator.
- **Suggested fix:** Replace the figures with PFSH03 measurements and add them to R3.

**P8 — Key derivation choice not made**
- **Severity:** minor
- **Where:** R4.
- **What is wrong:** The page notes that the app hashes the phrase seed while the daemon uses a random seed plus a wrapping key. It does not choose between them.
- **Why it matters:** The user story promises "restore from my recovery phrase". A random seed breaks that promise unless it is wrapped by a phrase-derived key.
- **How to investigate:** Compare both schemes for security against a compromised phrase and for portability.
- **Suggested fix:** Recommend phrase-derived keys with domain separation for all clients, and change the daemon to match.

**P9 — Gate 2 "mobile qualification" versus a Mac-only launch**
- **Severity:** minor
- **Where:** The "PFSH03 on the chain" row, and open question 4.
- **What is wrong:** It is unclear whether activation requires iOS qualification.
- **Why it matters:** The gate could silently block activation.
- **How to investigate:** Read the gate definitions in PR 232's docs.
- **Suggested fix:** State whether a Mac-only activation is allowed, or scope Gate 2 explicitly.

**P10 — Rehearsal constants in a production path**
- **Severity:** minor
- **Where:** "What PR 232's code fixes in place" (`PQ_REHEARSAL_ISSUE`, `PQ_REHEARSAL_PROOFS`).
- **What is wrong:** Production values and the mechanism for setting them are undefined.
- **Why it matters:** There is a risk of launching on rehearsal parameters, or of the pfUSDC series in the pool being inconsistent with NAVCoin and PFUSDC liquidity.
- **How to investigate:** Read `shielded_bridge_governance.rs` and check how these values change under amendment.
- **Suggested fix:**
  - Add a requirement that production parameters are set by amendment in the activation packet.
  - Confirm the pfUSDC series is the one NAVCoin fixes use.

**P11 — Anonymity-set viability unaddressed**
- **Severity:** minor
- **Where:** R16 and Privacy model.
- **What is wrong:** With few users, a four-proof cap and links between shields and unshields, the pool floor may never be reached.
- **Why it matters:** The app could say "small pool" indefinitely, and real privacy would be weak.
- **How to investigate:** Simulate pool growth from current user counts.
- **Suggested fix:** Consider Foundation-seeded cover traffic, or incentives for holding balances in the pool. State the target user count.

**P12 — Shield requires public PFT**
- **Severity:** minor
- **Where:** Fees.
- **What is wrong:** Users paid in pfUSDC need PFT to shield.
- **Why it matters:** Onboarding friction, and the funding source creates a public link.
- **How to investigate:** Check payout assets.
- **Suggested fix:** Pay a small PFT float with payouts, or allow a shield fee in the same asset.

**P13 — Agent and automated use of the local prover not considered**
- **Severity:** minor
- **Where:** User story and Architecture.
- **What is wrong:** Only Touch-ID human flows are designed.
- **Why it matters:** The founder's "post human", machine-native framing suggests agents should be able to pay privately.
- **How to investigate:** Check whether `pftl_pq_walletd` could serve local agents.
- **Suggested fix:** Note the daemon as a future path for agents, with spend policies.

**P14 — Revision notes and editorial clutter**
- **Severity:** minor
- **Where:** The "Bubbles up to" line, which repeats the outcome title in italics, and the long revision table.
- **What is wrong:** Founder-facing noise.
- **Why it matters:** It lowers readability.
- **How to investigate:** None needed.
- **Suggested fix:** Move the revision notes to an appendix and remove the duplication.

**P15 — Open questions mostly lack recommendations**
- **Severity:** major
- **Where:** Open questions 1–11.
- **What is wrong:** Eleven decisions are presented with options but no default. Questions 2, 6 and 7 (thin client versus local-first, memos, private payouts) give no lean at all. Questions 1, 3 and 5 give partial leans: question 1 notes that direct submission matches the founder's words, and question 3 calls native SwiftUI "safest". None of them is turned into an actual recommendation.
- **Why it matters:** This slows the founder's decisions and stalls progress on the gates.
- **How to investigate:** None needed.
- **Suggested fix:** Give a recommended default and a short rationale for each question, so the founder can approve or override.
