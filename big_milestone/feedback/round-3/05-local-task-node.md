# Feedback — Pillar 5 — The Task Node Local Experience

*Review round 3.*

> **Mean score:** 70/100 · Gemini 4 Neon 68 · Claude Opus 5.5 73
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../../pillars/05-local-task-node.md), once, with the question below. Run 2026-11-26 22:31 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 68/100

*Model `google/gemini-4-neon` · 20358 output tokens*

#### Summary judgement

This is an unusually well-grounded, candid engineering spec. Unlike typical high-level roadmaps, it anchors itself in real repositories, concrete open PRs (`postfiatl1v2` PRs 232/271/280 and `tasknode-apple` PR 39), real cryptographic and operational constraints, and an honest accounting of what is unmerged, unmeasured, or still centralized under the Foundation. It nails the founder's most important security invariant—client-side proving on a local MacBook with private actions strictly kept off the traditional Task Node server—even catching and overriding an existing contradiction in the Mac repo's own spec (`item C3`, the planned Task Node relay).

However, **as written, this spec will not produce a technically robust and economically coherent launch**, for four main reasons:
1. **Broken spam, storage, and operator economics are acknowledged and then punted to open questions.** A 1-atom flat fee on ~149 KB proofs with a 4-action/block global cap and zero validator/RPC operator compensation invites trivial permanent DoS of the privacy pool, ~50 GB/day of archival storage bloat, and a 3.3× consensus slowdown, while giving independent RPC operators zero economic reason to exist.
2. **Outcome B ("a production-ready new chain") is silently gated on an unbudgeted, unscheduled external audit of a bespoke Binius proof system** whose PFSH03 proof sizes, verification times, and WAN/Cobalt consensus impacts have not yet been measured—and whose acceptance criteria have no quantitative pass/fail budgets.
3. **Several concrete architectural and security bugs remain in the spec itself**, including an RPC-to-validator DoS amplification vector (20-bit PoW + 15 ms bad-proof verification + unvalidated forwarding), a diagram that gives the sandboxed prover direct network access, deferred finality-certificate verification, an impossible/unspecified IPFS pinning path for private-payment memos, and overlooked metadata leaks in RPC address discovery.
4. **"Virtually everything the task node can do" is heavily diluted**, punting Hive Chat, Governance, NFTs, and Information Hive Mind to an open question while introducing a direct contradiction between requiring NAVCoin markets in the Mac app (AC 9) and compiling out the Orchard pool that NAVCoin fix fills currently settle in (R1 / Economics).

The engineering bones are strong (roughly 75–80% of the wallet state-machine thinking is right), which is why this scores 68 rather than lower; fixing the critical economic, admission-DoS, critical-path, and parity defects would make it an execution-grade specification.

---

#### Correspondence to the founder's intention

**Where it aligns closely:**
- **Local MacBook privacy off the Task Node server:** Directly satisfies Deliverable 5 and the production-readiness gate ("privacy working - on local macbooks ideally in our task node app"). R8's hard boundary—forbidding the Task Node server from ever seeing keys, notes, witnesses, proofs, shielded actions, or private-payment memos, and explicitly rejecting the Mac repo's planned "Task Node relay"—captures the founder's intent with precision.
- **FastPay payouts as default:** R13 preserves FastPay as the default for low-latency Task Node payouts and designs a clean, two-step *Make private* flow (unwrap -> shield) with crash recovery.
- **Existing memo-type private payments:** R14 properly distinguishes existing public-payment encrypted memos (fixing `core/src/codec.rs`, which currently refuses memos) from native memos inside PFSH03 shielded envelopes (OQ6).
- **Adversarial stress testing and formal verification transparency:** R18 incorporates the founder's requirement for multi-model adversarial review across the circuit, verifiers, wallet crate, admission, and Mac app; R19 honestly states the scope and gaps of the existing 9 Lean modules.

**Where it falls short or diverges:**
- **"Virtually everything the task node can do" is largely deferred.** The founder's user story puts local parity first ("it can do virtually everything the task node can do. But more importantly it can do shielded transactions"). Today's Mac app lacks Hive Chat, Finance Hub, NFTs, and governance screens—which represent Deliverables 1, 4, 6, 9, and 10 of the founder's prompt. The spec unilaterally moves Finance Hub to web-only (R12), punts Hive/governance/NFTs/Hive Mind to Open Question 3, and writes AC 9 so that Pillar 5 passes using essentially the current v0.2.0 feature set. Without a concrete parity commitment, the "local experience" risks shipping as a stripped-down wallet companion rather than a full local Task Node.
- **NAVCoin parity contradiction.** AC 9 requires NAVCoin markets in the notarized Mac build, yet R1 forbids Orchard in any release build and the Economics section states that NAVCoin fix fills settle in the separate Asset-Orchard pool, which is not in this app. The founder expects NAVCoin swaps to be a near-term priority; this spec leaves Mac users unable to participate in private NAVCoin settlement without clarifying how NAVCoin markets in AC 9 work.
- **Unstated schedule impact on the new-chain migration.** The founder wants a staged migration off the old chain to a performant new chain, with local privacy as a production-readiness gate. By making a brand-new, unmerged, unaudited Binius PQ system (PFSH03) the *only* privacy path—with no auditor engaged, no budget, and Gate 2 requiring production-genesis cut + mobile qualification—the spec effectively couples the entire L1 migration to an open-ended 6–12+ month cryptographic audit timeline without explicitly presenting that schedule trade-off to the founder.
- **Cobalt + Lean connection is narrow.** The founder asked for Cobalt simulations and Lean studies in the context of consensus performance. R19 covers PFSH03's Lean modules (good), but Pillar 5 never links its ~600 KB PQ blocks and 3.3× round-time overhead back to Cobalt consensus simulations.

---

#### Technical robustness

**What is technically strong:**
- **Wallet state machine & operational realism:** Intent persistence before submission with idempotent retry, anchor-expiry release, and multi-device conflict resolution (R7, AC 6) are well engineered.
- **Honest gap identification:** Calling out unified seed derivation between the Mac app and `pftl_pq_walletd` (R4), the lack of outgoing viewing keys necessitating a local sent-payment journal (R24), the missing release binary on PR 232's head, and the legacy certification switch still left `on` in `docker-compose.wallet-public.yml` (R10) shows strong code-level due diligence.
- **Release hygiene & leak testing:** Notarization, hardened runtime, zeroisation, CI outbound-boundary checks, and release-build network captures (R11, R17, AC 2) are solid practices.

**Where technical robustness breaks down:**
- **Missing PFSH03 benchmarks and zero SLA budgets:** Proof size (~149 KB), validator verify time (24–55 ms), and 6-validator round impact (0.31 s -> 1.03 s) are all **PFSH02** numbers, not PFSH03. R3 only requires teams to "measure and publish"—there are no pass/fail limits for MacBook Air proving time/RAM/thermals, proof size, validator verification CPU, or Cobalt round latency under WAN conditions.
- **Trivial CPU/bandwidth DoS against validator admission:** PR 232 uses a 20-bit PoW (~1M hashes, milliseconds on CPU, microseconds on a consumer GPU) before a ~15 ms bad-proof verification on payloads up to 1 MiB. A single attacker with one GPU can flood valid-PoW, valid-signature, 1 MiB invalid proofs at tens to hundreds per second. R9 specifies that public RPC hosts *forward* submissions to validator admission, with no requirement for edge pre-verification or validator admission-vs-consensus CPU isolation.
- **Architecture security regressions:** The Mermaid architecture diagram routes `Sandboxed prover` directly to `Public RPC hosts`, giving a process that holds private keys/witnesses outbound network access (violating least privilege). Meanwhile, R6 defers light-client finality-certificate verification in favor of comparing two endpoints—which provides zero Byzantine/eclipse protection while Foundation runs all endpoints, and still leaves users exposed to colluding or MITM'd RPCs afterward.
- **Broken memo-pinning architecture:** R14 says encrypted IPFS memos tied to private payments never pass through Task Node, and AC 2 forbids sending private-payment memos to Task Node *or a third party*. Yet the architecture includes neither an embedded IPFS node nor an L1/validator pinning path. As written, R14 + AC 2 are mutually unsatisfiable.
- **Confusing tree-epoch / note-consolidation mechanics & rescan scaling:** The spec claims both inputs must be under one root "so notes in different tree epochs are merged by a self-payment first"—which is either a technical misdescription of append-only commitment trees or an under-engineered epoch-migration mechanism that is never tested in AC 7. Furthermore, phrase-only restore via trial decryption across all historical outputs has no compact-block design or scan-time ceiling, which will degrade severely as chain history grows.
- **Unaddressed metadata leaks:** Querying a recipient's `pfpq1` record over PFTL RPC (R15) right before sending a shielded payment leaks sender -> recipient to the RPC operator just as surely as querying Task Node would. Birthday-height sync cursors leak wallet creation time to RPC hosts.
- **Underspecified PQ key derivation and vault backup cryptography:** R4 does not specify minimum mnemonic entropy (12-word BIP-39 gives only 64-bit PQ security under Grover's algorithm vs ML-DSA-65's higher security level), and R5/R24 never specify what encrypts exported vault backups.

---

#### Economic coherence

This is the weakest section of the sub-page. To its credit, the Economics section accurately diagnoses almost every economic failure mode—**and then fails to require fixes for any of them**, deferring them to Open Question 8.

- **Guaranteed spam / congestion DoS at 1 atom:** With a hard cap of 4 actions/block (~600 KB/block), a 3.3× consensus slowdown on full blocks, permanent monotonic nullifier growth, and ~50 GB/day (~18 TB/year at 1 s blocks) of archival proof storage, a proposed **1-atom flat fee** is economically fatal. One moderately equipped user or griefer can buy 100% of global private block space indefinitely for negligible cost, locking out legitimate users, degrading public-chain/FastPay latency, and bloating archival state. Conversely, a static governance-set high flat fee prices out small Task Node payouts and note consolidations while still breaking under demand spikes. Congestion/dynamic fee pricing (or proof-size + state-rent pricing) is not an optional "open question"—it is a launch requirement.
- **Zero incentive for independent RPCs or validator admission:** PFT fees burn and pfUSDC fees go to a governance collector; validators receive **zero fee share or reward credit** for admitting, verifying, and storing private actions, and independent public RPC operators receive **zero compensation** for serving high-bandwidth full-page syncs and absorbing public submission DDoS. Because the privacy model depends on non-Foundation RPC operators (and validators must explicitly opt in via `--allow-private-action-admission`), rational third-party operators will either never run public privacy RPCs or disable admission.
- **Fragmented anonymity sets and cold-start trap:** Per-asset pools (PFT only, plus a single pinned pfUSDC development series) combined with the "hidden, small pool" warning (until 16 candidates / 128 actions are reached), a 4-action/block ceiling, no native memos, and no NAVCoins create a classic privacy chicken-and-egg problem where early users have little reason to shield, preventing the pool from ever reaching its anonymity floor.
- **pfUSDC onboarding friction:** Requiring public PFT to shield pfUSDC breaks the USDC -> pfUSDC onboarding flow for stablecoin-only / NAVCoin users.
- **Unbudgeted fixed costs on the critical path:** An independent cryptographic + code + DoS audit of a custom Binius PQ protocol, Rust wallet, admission layer, and native Mac app is a six-figure, multi-month effort with no vendor or budget identified, compounded by the ongoing engineering cost of maintaining full native SwiftUI parity with a fast-moving web app.

---

#### Problems

##### P1 — Unviable spam, storage-bloat, and congestion economics left unblocked
- **Severity:** `critical`
- **Where it occurs:** *What PR 232's code fixes in place* ("Compiled-in limits", "Fees"); *Economics* ("Capacity", "Spam", "Cost per action"); *Open questions for the founder* (#8).
- **What is wrong:** The spec proposes a 1-atom flat fee for a scarce global resource (4 PQ actions/block, ~149 KB each, 24–55 ms validator CPU, permanent nullifiers, 3.3× round slowdown, up to ~50 GB/day archival growth at full blocks). It explicitly admits a single sender with a handful of cores can fill every slot and crowd out all users, yet leaves congestion pricing as an open question rather than a technical requirement and launch gate.
- **Why it matters:** On mainnet, a 1-atom flat fee guarantees trivial griefing/DoS of all private transactions, sustained degradation of public-chain/Cobalt block latency, and unsustainable archival storage growth.
- **How an agent should investigate:** Inspect `crates/node/src/pq_payment_actions.rs` and `crates/types/src/shielded_bridge_governance.rs` on PR 232 to see how fees and block limits are enforced, calculate actual archival bytes/day at target block cadence using real PFSH03 proof sizes, and check whether existing L1 fee mechanics (e.g., EIP-1559-style base fee per block or PQ-slot base fee) already exist in `postfiatl1v2`.
- **Suggested direction for the fix:** Make anti-spam / congestion-aware fee pricing (e.g., a dynamic PQ-slot base fee targeting 50% of the per-block PQ cap, with a non-trivial floor pricing verification CPU + archival bytes + permanent nullifier state) a hard requirement before mainnet activation (Gate 3), rather than an optional OQ8 toggle.

##### P2 — Zero economic incentive for validators and independent public RPC operators
- **Severity:** `critical`
- **Where it occurs:** *Economics* ("Who earns"); *Technical requirements* (R9); *Open questions* (#1, #8); *Privacy model*.
- **What is wrong:** PFT shielded fees are burned and pfUSDC fees go to a governance collector; validators earn no fee share or Pillar 3 reward credit for running `--allow-private-action-admission`, and public RPC operators earn nothing for serving full-page historical syncs and exposing public submission endpoints. Yet the privacy architecture requires at least two independent, non-Foundation RPC endpoints.
- **Why it matters:** Without compensation or protocol-level obligation, independent validators have a rational incentive to keep private admission disabled (to avoid CPU overhead and DoS risk), and independent third parties have no incentive to operate high-egress public RPCs—leaving privacy permanently centralized on Foundation servers.
- **How an agent should investigate:** Cross-reference `03-validator-program.md` (Pillar 3) to see how validator uptime, RPC operation, and reward eligibility are measured and compensated, and inspect `private_action_rpc.rs` to see what admission/relay telemetry is already logged.
- **Suggested direction for the fix:** Coordinate with Pillar 3 to require verifiable public-RPC + PQ-admission participation as a scored criterion in the Validator Rewards Program (or split PQ fees between validator/proposer burn and RPC/admission operators, or integrate X402/micropayment RPC funding from Deliverable 2), and require >=2 independent non-Foundation operators as a hard mainnet privacy readiness gate.

##### P3 — RPC-to-validator DoS amplification and missing admission CPU isolation
- **Severity:** `critical`
- **Where it occurs:** *What exists today* ("PFSH03 numbers"); *Technical requirements* (R9, R10); *Acceptance criteria* (AC 5).
- **What is wrong:** A 20-bit PoW is trivial on modern CPUs/GPUs, while verifying a valid-PoW invalid proof takes ~15 ms on payloads up to 1 MiB. R9 specifies that public RPC hosts simply forward `private_action_submit` to validator admission without requiring edge verification, and neither R10 nor AC 5 requires isolating validator admission CPU/queues from Cobalt consensus threads or testing a valid-PoW, max-size invalid-proof flood across multiple RPCs.
- **Why it matters:** An attacker with modest hardware can flood public RPC hosts with 1 MiB valid-PoW invalid proofs, saturating validator admission CPU/bandwidth and potentially stalling block consensus.
- **How an agent should investigate:** Inspect `crates/node/src/private_action_rpc.rs`, `wallet-proxy/private-actions.js`, and PR 232's admission path to check: (a) where cheap checks and full proof verification run, (b) whether admission shares a Tokio runtime/thread pool with consensus, and (c) whether RPC followers already have access to finalized anchors to verify proofs at the edge.
- **Suggested direction for the fix:** Require public RPC hosts to run cheap checks *and* full proof verification (or strict per-sender/PoW adaptive difficulty) at the edge before forwarding to validators; require validator admission to run on a bounded, low-priority thread pool isolated from consensus; and strengthen AC 5 to test a distributed valid-PoW, valid-signature, 1 MiB bad-proof flood with quantitative consensus-latency bounds.

##### P4 — Unmeasured PFSH03 costs, missing quantitative SLAs, and an unbudgeted audit blocking Outcome B
- **Severity:** `critical`
- **Where it occurs:** *Status*; *What exists today* ("PFSH03 numbers"); *Economics* ("Fixed costs"); *Technical requirements* (R1, R3); *Dependencies* ("Build order"); *Acceptance criteria* (AC 5, AC 11); *Open questions* (#5, #10).
- **What is wrong:** Every verifier/size/block-latency figure in the spec is from superseded **PFSH02**, not PFSH03. R3 and AC 11 only require "measure and publish" with no pass/fail engineering budgets (e.g., max proof time/RAM on base 8 GB MacBook Air, max Cobalt round latency impact over a WAN of 20+ validators, max archival growth). Simultaneously, Gate 1 requires an external audit of a bespoke Binius PQ stack with no vendor, budget, or timeline—silently blocking Outcome B (production-ready new chain).
- **Why it matters:** Without hard performance budgets, degraded consensus or unusable Mac performance can technically pass AC 5/AC 11. Without explicit staging/budgeting for the audit, the entire L1 migration can stall indefinitely waiting for Gate 1.
- **How an agent should investigate:** Check `postfiatl1v2` PRs 232 and 280 for actual PFSH03 proof sizes and verifier benchmarks; review Cobalt simulation docs/specs in `postfiatl1v2` for block-size and round-time budgets; and check `README.md` (Outcome B) to see whether initial L1 validator migration strictly depends on mainnet PFSH03 activation.
- **Suggested direction for the fix:** Replace PFSH02 numbers with measured PFSH03 benchmarks before finalizing the spec; add explicit pass/fail SLA thresholds to R3 and AC 5 (including WAN/Cobalt simulation with 600 KB PQ blocks); and explicitly add a founder decision/staging plan separating "new-chain L1 launch with PFSH03 active on devnet + audit underway" from "mainnet PFSH03 activation post-Gate 1", alongside realistic audit budget/timeline estimates.

##### P5 — Weak "virtually everything" parity and unresolved Orchard/NAVCoin contradiction
- **Severity:** `major`
- **Where it occurs:** *What exists today* ("Native Mac app"); *Economics* ("NAVCoins"); *Technical requirements* (R1, R12); *Acceptance criteria* (AC 9); *Open questions* (#3, #11).
- **What is wrong:** Two related scope defects: (1) Core Task Node features from Pillars 1, 4, 6, 9, and 10 (Hive Chat, governance sessions, NFTs, Finance Hub, Information Hive Mind) are either pushed to web-only or deferred to OQ3, leaving AC 9 testing only today's v0.2.0 surface; (2) AC 9 explicitly requires **NAVCoin markets** in the Mac app, while R1 forbids Orchard in any release build and *Economics* states that NAVCoin fix fills settle in the Asset-Orchard pool, which is not in this app.
- **Why it matters:** Shipping without a concrete parity plan fails the founder's "virtually everything the task node can do" requirement, and shipping NAVCoin markets without resolving Asset-Orchard settlement either breaks fix fills in the Mac app or silently falls back to server-side/custodial settlement.
- **How an agent should investigate:** Inspect `tasknode-apple` (`mac/Sources/`) to see what "NAVCoin markets" currently does in v0.2.0 (whether it only shows public swaps/prices or triggers fix fills via hosted APIs), and check `07-navcoin-fees.md` (Pillar 7 R6) on how NAVCoin fix fills interact with Asset-Orchard vs PFSH03.
- **Suggested direction for the fix:** Explicitly reconcile NAVCoin fix fills (either clarify that Mac app NAVCoin markets cover public Uniswap/PFUSDC/PFTL flows until fix fills migrate to PFSH03, or define a clean migration plan off Asset-Orchard in Pillar 7); and define a concrete, phased parity matrix in R12 (identifying which of Hive Chat, Governance, NFTs, and Hive Mind are required for v1 launch vs v1.1, with a recommended architecture in OQ3 rather than leaving AC 9 at v0.2.0 baseline).

##### P6 — Security flaws in prover sandbox boundary and deferred finality-certificate verification
- **Severity:** `major`
- **Where it occurs:** *Architecture* (Mermaid flowchart); *Technical requirements* (R6, R11); *Acceptance criteria* (AC 3).
- **What is wrong:** (1) The Mermaid flowchart shows an arrow directly from `Sandboxed prover: PFSH03, CPU` to `Public RPC hosts`, implying the prover helper has network access. (2) R6 defers cryptographic finality-certificate verification in the wallet, relying instead on two RPC endpoints returning matching state roots—which fails entirely while Foundation runs all endpoints and remains vulnerable to colluding RPCs or network MITM afterward.
- **Why it matters:** A sandboxed prover holding raw private keys/witnesses must have **zero network entitlements** so a memory-safety bug or compromised proving crate cannot exfiltrate secrets. Without finality-certificate verification, malicious or compromised RPC endpoints can spoof deposits (fake balances) to local Mac users.
- **How an agent should investigate:** Inspect `tasknode-apple` PR 39 (`core/src/prover/`, `mac/Sources/V2/V2Gateway.swift`) to verify whether the prover helper currently performs networking or communicates only over IPC/stdin-stdout; inspect `postfiatl1v2` finalized-snapshot RPC responses to see what validator quorum certificate / finality proof is already attached to finalized state roots.
- **Suggested direction for the fix:** Fix the Architecture diagram and add an explicit requirement in R11 that the sandboxed prover runs with **no network access** and returns proof bytes over IPC to `pf_app_core` for submission; promote light-client finality-certificate verification against the pinned validator set from "until the app verifies" to a hard R6/AC 3 launch requirement.

##### P7 — Unspecified IPFS pinning path for private-payment encrypted memos contradicts AC 2 / R8 / R14
- **Severity:** `major`
- **Where it occurs:** *Technical requirements* (R8, R14); *Acceptance criteria* (AC 2, AC 9).
- **What is wrong:** R14 requires encrypted-memo payments ("a pointer memo plus an encrypted IPFS payload") to work from the Mac app, and states that a memo tied to a private payment never passes through Task Node. AC 2 states that a network capture shows no private-payment memo going to Task Node *or a third party*. However, the spec never specifies **how or where the Mac app pins that encrypted IPFS payload** (no embedded IPFS node, no L1/RPC pinning endpoint, and third-party pinners are barred by AC 2).
- **Why it matters:** As written, R14 and AC 2 cannot both be satisfied simultaneously; engineers implementing R14 will either route through a third-party pinning API (failing AC 2 and leaking metadata) or block on an undesigned local IPFS daemon.
- **How an agent should investigate:** Inspect `tasknode/server/context-publish.js`, `tasknode/docs/wiki/architecture/encryption.md`, and `02-message-protocol.md` (Pillar 2) to see how IPFS pinning and retrieval work for non-Task-Node clients on Post Fiat.
- **Suggested direction for the fix:** Explicitly specify the memo pinning transport in Architecture and R14 (e.g., pinning through the same non-Task-Node public RPC / L1 IPFS pinning gateway used for Pillar 2, or an embedded lightweight IPFS/Helia/Iroh publisher, or restricting AC 2's "third party" wording to unapproved endpoints while documenting the exact metadata exposure of the chosen pinning service).

##### P8 — Overlooked metadata leaks in RPC address discovery, sync cursors, and immediate payout shielding
- **Severity:** `major`
- **Where it occurs:** *Privacy model*; *User story* (step 3); *Technical requirements* (R6, R13, R15).
- **What is wrong:** (1) R15 avoids Task Node address lookup by reading a recipient's `pfpq1` on-chain record over PFTL RPC—but querying a specific user's record on an RPC host immediately before submitting a shielded payment to that same host (or another host run by the same operator) leaks sender -> recipient directly to the RPC operator. (2) R6 fetches pages from a wallet's cursor, leaking wallet birthday/creation height and linking repeat sync sessions to the same IP. (3) The default UX in Step 3 / R13 encourages immediately shielding a fresh 100 PFT Task Node payout, creating a 1:1 amount+timing link between Task Node payout identity and pool entry.
- **Why it matters:** These leaks undermine the practical privacy of the local Mac app even when the Task Node server never touches the proof, especially while Foundation runs most RPCs and early pools are small.
- **How an agent should investigate:** Check `06-hive-identity.md` (Pillar 6) to see how `pfpq1` identity records are stored and queried on PFTL RPC, and review `docs/privacy/metadata-anonymity-bounds.md` in `postfiatl1v2` to see which network-layer leaks are already modeled.
- **Suggested direction for the fix:** Document all three leaks in the *Privacy model* table; require R15 to either cache/sync identity records in bulk or query a *different* independent RPC endpoint (and decouple timing) from submission; require R6 to quantize/round initial sync cursors (e.g., to coarse epoch boundaries); and consider suggesting delayed or randomized/denominated shielding guidance in the R13 UX.

##### P9 — Confusing tree-epoch note merging, dust-griefing gaps, and unbounded phrase-restore scan times
- **Severity:** `major`
- **Where it occurs:** *What PR 232's code fixes in place* ("Anchors and notes"); *Technical requirements* (R3, R5, R23); *Acceptance criteria* (AC 7).
- **What is wrong:** (1) The spec states "Every action spends two notes and creates two, both inputs under one root, so notes in different tree epochs are merged by a self-payment first." If an action requires both inputs under one root, two notes in genuinely separate tree epochs *cannot* be spent in a single merge action—one must first be migrated via a single-note + dummy-input self-payment into the current epoch, then spent with the second note. Conversely, if PR 232 uses a single append-only tree with a 256-root anchor window, all historical notes are already under the latest root and no epoch merge exists. Either the spec misstates PR 232's design or leaves a multi-step epoch-migration UX untested (AC 7 only tests 5 notes, not cross-epoch notes). (2) Dust notes (`<= fee`) sent by a griefer cannot be profitably spent or consolidated and accumulate indefinitely. (3) Phrase-only restore (R5) linearly trial-decrypts every output since activation with PQ crypto, with no compact-block format or scan-time ceiling.
- **Why it matters:** Misunderstanding note/epoch constraints leads to broken coin-selection logic; dust spam degrades wallet UX; and unbounded linear PQ scanning makes phrase-only recovery unusable after months of chain activity.
- **How an agent should investigate:** Read `crates/types/src/pq_payment.rs` and `crates/privacy_binius_wallet/` on PR 232 to verify: (a) whether commitment trees rotate into epochs or are single append-only trees, (b) whether dummy zero-value input notes are supported, (c) what symmetric/PQ KEM encryption scheme is used for trial decryption, and (d) how many ms per 10,000 outputs trial decryption actually takes on Apple Silicon.
- **Suggested direction for the fix:** Clarify the exact tree-epoch and dummy-input rules in *What PR 232's code fixes in place* and R23; add a cross-epoch test to AC 7 (if epochs exist); add a dust-filtering/hide-dust UI rule in R23; and add a maximum acceptable scan-time budget (and compact-output endpoint if needed) in R3/R5.

##### P10 — Post-quantum seed entropy, vault backup encryption, and multi-device sent-journal sync are underspecified
- **Severity:** `minor`
- **Where it occurs:** *Technical requirements* (R4, R5, R11, R24); *Acceptance criteria* (AC 4).
- **What is wrong:** R4 does not specify minimum mnemonic entropy (a standard 12-word / 128-bit BIP-39 phrase provides only ~64-bit post-quantum security against Grover's algorithm, undercutting ML-DSA-65's PQ strength) or the exact KDF/hash primitive. R5 and R24 describe an "encrypted vault backup" without specifying how its encryption key is derived or managed on restore (phrase-derived vs separate user passphrase), and R11/R24 leave multi-device users with permanently diverged sent-payment journals.
- **Why it matters:** Weak seed entropy undermines the entire post-quantum thesis; ambiguous backup encryption causes lockouts or insecure ad-hoc implementations during recovery.
- **How an agent should investigate:** Check `tasknode-apple` PR 39 (`core/src/pq_payments.rs`, `core/src/session/pq.rs`) and PR 232 (`crates/privacy_binius_wallet/`) for existing mnemonic length, derivation primitives, and backup file formats.
- **Suggested direction for the fix:** Specify 24-word (256-bit) minimum recovery-phrase entropy for full PQ security (with an explicit warning if restoring a legacy 12-word phrase), specify the KDF (e.g., HKDF-SHA-512 / BLAKE3 domain separation) in R4, and define the vault-backup encryption scheme (e.g., AEAD keyed via Argon2id user passphrase or phrase-derived backup key) in R5.

##### P11 — macOS prover sandboxing/pinning implementation traps and Gate 2 mobile-qualification mismatch
- **Severity:** `minor`
- **Where it occurs:** *What exists today* ("Native Mac app", "PFSH03 on the chain"); *Technical requirements* (R11); *Open questions* (#4).
- **What is wrong:** (1) R11 mandates a "sandboxed prover" with "prover's code hash pinned" inside a Developer ID-notarized app, without specifying macOS-compliant mechanisms (`SecStaticCodeCheckValidity` / designated requirement + CDHash via an App-Sandboxed XPC service rather than Apple-deprecated `sandbox-exec` or racy file-hash checks). (2) Gate 2 explicitly requires "mobile qualification", which conflicts with launching Mac-only—noted in OQ4, but not reflected in Dependencies/Build order (Step 4) or R1.
- **Why it matters:** Naive macOS sandboxing or file-hash pinning fails Apple notarization or introduces TOCTOU vulnerabilities, and an unresolved Gate 2 mobile requirement could block governance activation even after Mac tests pass.
- **How an agent should investigate:** Inspect `tasknode-apple/.github/workflows/mac.yml` and `core/src/prover/` to see how the helper binary is currently packaged, sandboxed, and verified; inspect PR 232's `docs/quantum/private-payments.md` for the exact Gate 2 wording.
- **Suggested direction for the fix:** Specify App-Sandboxed XPC helper + Apple Security framework designated-requirement/CDHash verification in R11, and explicitly add "amend Gate 2 scope to Apple-silicon macOS if mobile is deferred" to Step 4 of the Build order and AC 12.

## Claude Opus 5.5 — 73/100

*Model `anthropic/claude-opus-5.5` · 9891 output tokens*

#### Summary judgement

This is a strong sub-page. It is technically literate and candid about status: nothing is merged, no audit has started, submission is off, and Orchard is not the target. It is organised around the founder's central demand, which R8 states as a hard rule: the Task Node server never touches keys, notes, witnesses, proofs or shielded actions. It also folds in several cross-cutting asks from the prompt:

- FastPay as the default payout path;
- existing memo-type private payments;
- adversarial review across several models;
- a Lean coverage statement.

The privacy model table is honest about what local proving does *not* hide. The intent and sync rules show real engineering care, and the dependencies and build order are realistic.

The score is held down by four weaknesses:

- **The "virtually everything" half of the story is deferred.** The app is a thin client with no Hive, governance, NFT or Finance Hub screens, and parity is pushed to an open question.
- **Economic and capacity parameters are not production-coherent**, and the page acknowledges this without resolving it:
  - a 1-atom fee;
  - four actions per block;
  - no validator compensation;
  - no pool-value cap;
  - numbers taken from PFSH02, not PFSH03.
- **Some security claims need verification** before they can be stated as fact:
  - that note encryption is post-quantum;
  - that the recovery phrase has enough entropy to justify "post-quantum";
  - that "independent endpoint" checks mean anything while the Foundation runs every endpoint.
- **The critical path runs through an auditor who is not engaged and has no budget.** As written, the pillar's timeline is unbounded.

#### Correspondence to the founder's intention

**What matches well:**

- **Local app with shielded transactions, never processed on the Task Node server.** This is directly captured by R8, R15 and R17, AC 2, and the receipt wording in story step 6. The spec even flags a conflicting plan in the Mac spec (the "Task Node relay", item C3) and forbids it. This is the strongest alignment point.
- **"Privacy working on local MacBooks, ideally in our Task Node app."** The page correctly names this as a readiness gate for Outcome B and targets Apple silicon.
- **FastPay as the default for task payouts.** R13 and AC 8 handle the FastPay-to-shield path.
- **"Works with existing memo type private payments."** R14 keeps encrypted-memo payments working as public payments with private content.
- **Multi-model adversarial testing and Lean.** R18 and R19 cover these.
- **Quantum resistance.** The choice of PFSH03 (Binius with ML-DSA-65) is consistent with the founder's quantum-resistant theme elsewhere, for example in Deliverable 4.

**Where it drifts:**

- **"It can do virtually everything the Task Node can do."** The spec builds a thin client over the hosted APIs:
  - there are no Hive, governance, NFT or Finance Hub screens;
  - Finance Hub is web-only;
  - the parity decision is pushed to open question 3.

  Asking the founder is defensible, but the spec proposes no default parity set. As a result, AC 9 tests whatever happens to be native today.
- **The public, legible story.** The founder wants Post Fiat to read publicly as an exploration (blog posts, Lean studies). R19 is an internal coverage statement, and no public write-up of the PFSH03 privacy model is a deliverable.
- **Mac-only scope.** "Local macbooks" fits the founder's words, but Gate 2's "mobile qualification" is left as an awkward mismatch.
- **Tone versus urgency.** The founder frames this as near-term production readiness. The spec's honest answer is that the audit, with no vendor yet, gates everything, and it offers no interim path or estimated date.

#### Technical robustness

**Good:**

- **Cheap-checks-first denial-of-service ordering:** signature, transcript, proof-of-work, then the proof.
- **Page-based sync pinned to a finalized state root,** with output records and ciphertext digests committed.
- **Intent journaling with identical-bytes retry,** and conflict resolution through a shared real input.
- **A sent-payment journal** to compensate for the missing outgoing viewing key.
- **Hardened runtime and a sandboxed prover** with a pinned code hash.
- **A pause switch that keeps exits open.**
- **The legacy certification switch caught** in `docker-compose.wallet-public.yml`.

**Weak or unverified:**

- **Numbers are from the wrong system.** Proof size, verify cost and block-round time are PFSH02 figures, while PFSH03 is the target. Capacity and spam reasoning therefore rest on stale data.
- **The note and epoch model is underspecified.** The spec says "both inputs under one root" and "notes in different tree epochs are merged by a self-payment first". But a self-payment is itself a two-input action under one root. The spec never explains:
  - how notes from two epochs are combined;
  - how a wallet holding a single note spends (does it use a dummy note?).
- **Post-quantum confidentiality of note ciphertexts is asserted, not shown.** The page says a future quantum attacker learns "nothing extra", but never names the key-encapsulation or encryption scheme used for notes.
- **Seed entropy.** A 12-word phrase is 128 bits, which gives about 64-bit security against Grover's search. That undermines the post-quantum framing if 12-word phrases are allowed.
- **State-point checks against a "second, independent endpoint" are hollow today,** because the Foundation runs all of them. Verifying finality certificates in the app is deferred.
- **Undecryptable ciphertexts.** The proof binds the ciphertext but not that it decrypts. A buggy or malicious sender can therefore create unrecoverable notes, and the spec does not address this.
- **Thousands of Mac users cannot be bounded by "public RPC endpoints" that do not exist yet.** The submission path is undecided (open question 1).

#### Economic coherence

The page lays out the economics honestly but leaves them incoherent for production:

- **Fee versus cost.** A 1-atom fee on roughly 149 KB and 24–55 ms of validator CPU per action means anyone can buy all four slots per block. Nullifiers also grow permanently.
- **Validator incentives.** Validators get no fee share or credit for admission and serving pages.
- **Getting into the pool.** A shield's fee must be paid in public PFT, so pfUSDC-only holders face a barrier.
- **Unbounded loss.** There is no pool-value cap, so a soundness bug exposes every depositor's funds.
- **Asymmetric fee treatment.** PFT fees burn, while pfUSDC fees go to an unspecified "governance collector".
- **Fragmented pfUSDC.** The pool pins a development pfUSDC series, which risks fragmenting from the NAVCoin and Solana bridge pfUSDC.

Each of these is surfaced as an open question rather than given a recommended default, so the spec cannot yet be called economically coherent.

#### Problems

**P1 — major — R12, AC 9, open question 3: "virtually everything" is unresolved**
- **What is wrong:** The founder's core claim is that the local app does nearly everything the hosted one does. The spec ships a thin client with no Hive, governance, NFT or Finance Hub screens and offers no default parity set.
- **Why it matters:** The governance pillar (Pillar 1), Hive Chat and identity are central to the founder's narrative. A local app without them is weakly aligned with the founder's story.
- **How to investigate:** Inventory hosted Task Node features and the Pillars 1, 4, 6 and 10 UX surfaces. Estimate the porting cost of each.
- **Fix direction:**
  - Propose a concrete launch parity set, for example Hive Chat read/post plus governance-session viewing, with NFTs deferred.
  - Recommend one approach, likely a signed bundled web build for non-private screens.
  - Make AC 9 test that set.

**P2 — critical — Economics (capacity, spam) and open question 8: a 1-atom fee with 4 slots per block is a trivial denial of service**
- **What is wrong:** As the page itself admits, one sender can fill every private slot indefinitely. The proposed levers do not price congestion.
- **Why it matters:** The privacy pool becomes unusable at negligible cost to an attacker, which fails the "production ready" gate.
- **How to investigate:** Using PFSH03 numbers, compute the attacker cost per hour to saturate the slots at 1 atom versus candidate fees. Model nullifier-set growth.
- **Fix direction:**
  - Recommend a default: a byte- and CPU-priced minimum fee set by amendment, plus a congestion multiplier or priority ordering by fee.
  - State a mainnet minimum.
  - Require that the fee is not "1 atom" at activation.

**P3 — major — Economics and R3: capacity and cost use PFSH02 figures**
- **What is wrong:** The 149 KB size, the 24–55 ms verify time and the 1.03 s round time are all PFSH02 figures, while the target is PFSH03.
- **Why it matters:** The four-actions-per-block cap within 640,000 bytes, the throughput estimate and the spam cost could all be wrong.
- **How to investigate:** Measure PFSH03 proof size and verifier cost on PR 232 and PR 280.
- **Fix direction:**
  - Make PFSH03 sizing a Step 1 deliverable.
  - Re-derive the cap from it, with headroom.

**P4 — critical — "Absent: … a pool-value cap", open question 5: unbounded pool exposure**
- **What is wrong:** There is no consensus-enforced ceiling on pool value. A soundness bug could drain all deposits, and the spec only discusses a cap for a beta.
- **Why it matters:** New ZK systems have historically had soundness bugs; Zcash had a counterfeiting bug. Mainnet should have a turnstile or cap regardless of audit status.
- **How to investigate:** Review whether PR 232's state can enforce a per-asset cap and turnstile accounting cheaply.
- **Fix direction:**
  - Add an R item for a per-asset pool cap and turnstile at activation, raised by amendment.
  - Require it in the AC 12 activation packet.

**P5 — major — Privacy model: post-quantum confidentiality of note encryption is not specified**
- **What is wrong:** The page claims a quantum attacker learns "nothing extra from PFSH03", but never names the note-encryption key-encapsulation scheme. If it is classical (for example X25519), harvest-now-decrypt-later breaks confidentiality.
- **Why it matters:** The pillar's headline is "post-quantum".
- **How to investigate:** Read `crates/types/src/pq_payment.rs` and the `privacy_binius_wallet` README for the encryption suite.
- **Fix direction:**
  - State the scheme, for example ML-KEM-768 with an AEAD.
  - If the scheme is classical, change it or qualify the claim.

**P6 — major — R4 recovery phrase: seed entropy versus the post-quantum claim**
- **What is wrong:** The PQ seed is derived from the recovery phrase. A 128-bit (12-word) phrase gives about 64-bit security under Grover's search.
- **Why it matters:** Everything in the pool would hang on the phrase. That weakens the post-quantum guarantee, and it contradicts the daemon's current 48-byte random seed.
- **How to investigate:** Check the phrase length the Mac app and Task Node generate.
- **Fix direction:**
  - Require 24-word (256-bit) phrases for PQ derivation, or warn on 12-word phrases.
  - Document this in the R4 test vectors.

**P7 — major — R6 and open question 1: "independent endpoint" checks are not independent**
- **What is wrong:** The Foundation runs all validators and endpoints, so matching the state point on two endpoints gives no protection against a malicious or compromised operator. Finality-certificate verification is deferred.
- **Why it matters:** A lying endpoint can feed the app a false anchor or false pages, causing denial of service or deanonymisation through crafted state.
- **How to investigate:** Assess the cost of verifying validator-quorum certificates in `pf_app_core`.
- **Fix direction:** Make light-client certificate verification a launch requirement, or explicitly state that the trust assumption is the Foundation until it ships.

**P8 — major — User story step 3, R13, R16: shielding exact payout amounts links identity**
- **What is wrong:** The canonical flow shields a 100 PFT payout at once. Task Node knows who earned what, and the shield shows the account and amount publicly. The leak warnings cover only shield/unshield pairs.
- **Why it matters:** The pool's anonymity set is built from users whose identities are already linked by Task Node.
- **How to investigate:** Simulate the anonymity set given the observable payout and shield patterns.
- **Fix direction:**
  - Default to batching or delaying shields and rounding amounts.
  - Add a warning that the payout operator knows the amount.
  - Revisit open question 7 with a recommendation.

**P9 — major — Build order and open question 10: an unbounded critical path**
- **What is wrong:** There is no auditor, vendor, budget or timeline, yet the audit gates everything.
- **Why it matters:** The founder wants near-term production readiness. Without an estimate this pillar can block Outcome B indefinitely.
- **How to investigate:** Get quotes for a Binius-familiar ZK audit.
- **Fix direction:**
  - Add a budget range and timeline to the spec.
  - Propose an interim, capped devnet or testnet milestone with a date.

**P10 — major — "Anchors and notes", R23: epoch semantics and the two-in-two-out model are unclear**
- **What is wrong:** The spec does not explain how a self-payment merges notes from different epochs when both inputs must share one root. It also does not say how a single-note wallet spends.
- **Why it matters:** Wallets may hold unspendable or stuck funds.
- **How to investigate:** Read `pq_payment_actions.rs` for tree rotation and dummy-note support.
- **Fix direction:** Specify the epoch rotation, migration and dummy-input rules, and add an AC for them.

**P11 — minor — "Integrity" note: the proof does not prove the ciphertext decrypts**
- **What is wrong:** A sender can create notes the recipient cannot decrypt, and the spec does not state the consequence.
- **Why it matters:** This enables griefing and lost funds, and it creates disputes over whether a payment was received.
- **How to investigate:** Check whether the sender's wallet self-verifies decryption before submitting.
- **Fix direction:**
  - Require sender-side decryption self-checks.
  - Keep the plaintext in the sent journal so a disputed payment can be checked.

**P12 — major — "Who earns" and open question 8: no validator compensation for admission and pages**
- **What is wrong:** Validators bear CPU, bandwidth and storage costs for private actions with no reward.
- **Why it matters:** Once independent operators exist, their incentive to support privacy is weak. This ties into the economics of the Pillar 3 rewards program.
- **How to investigate:** Estimate per-validator costs at full capacity.
- **Fix direction:** Recommend a fee split or Pillar 3 credit, attributed through the endpoint recorded on each receipt.

**P13 — major — "Assets" and open question 9: the pinned pfUSDC is a rehearsal series**
- **What is wrong:** The production pfUSDC series is undecided, and fragmenting it from the NAVCoin and Solana bridge pfUSDC would split liquidity and the anonymity set.
- **Why it matters:** The founder's NAVCoin plan centres on pfUSDC.
- **How to investigate:** Align with the bridge and NAVCoin pillars on canonical pfUSDC issuance.
- **Fix direction:** Recommend that the pool pin the canonical bridged pfUSDC series.

**P14 — minor — "Fees": the pfUSDC governance collector is unspecified**
- **What is wrong:** The spec does not say who controls the collector or how it is custodied, and gives no reason for the asymmetry with PFT, whose fees burn.
- **Why it matters:** The collector is a custody and governance risk.
- **How to investigate:** Find the collector definition in `shielded_bridge_governance.rs`.
- **Fix direction:** Specify custody and spending rules, or burn or convert pfUSDC fees as well.

**P15 — minor — Economics "Getting in" and open question 8: shield fee only in PFT**
- **What is wrong:** pfUSDC-only users need PFT before they can shield.
- **Why it matters:** It adds onboarding friction for the NAVCoin and pfUSDC user base.
- **How to investigate:** Look at the protocol change needed to let a shield pay its fee in the shielded asset.
- **Fix direction:** Recommend a default, or have the app auto-acquire a small amount of PFT.

**P16 — minor — Acceptance criteria and R3: no pass/fail thresholds**
- **What is wrong:** The measurements are "published", with no budgets for proving time, memory or sync time on the minimum Mac.
- **Why it matters:** Without thresholds, AC 11 passes trivially.
- **How to investigate:** Derive budgets from the UX target ("proves in a few seconds").
- **Fix direction:** Set numeric budgets, for example under 6 s proving and under 1 GB peak memory on an 8 GB Air.

**P17 — minor — R19: Lean work is internal only**
- **What is wrong:** The founder asked for blog posts and iterative Lean studies, and R19 is only a coverage statement.
- **Why it matters:** It misses the public narrative of "an exploration" the founder wants.
- **How to investigate:** Review `formal/lean/README.md`.
- **Fix direction:** Add a public blog post on the PFSH03 privacy model and Lean coverage as a deliverable.

**P18 — minor — Status line versus the PR 232 numbers: inconsistent timings**
- **What is wrong:** The page gives "about 2.2 s on M5" and "2.66 s on 16 threads" without saying which hardware each refers to.
- **Why it matters:** The numbers are confusing for readers making capacity and UX decisions.
- **How to investigate:** Confirm the hardware for each figure.
- **Fix direction:** Label each figure with its machine and thread count.

**P19 — minor — Open question 4 and Gate 2: "mobile qualification" versus a Mac-only launch**
- **What is wrong:** The gate is not reconciled with the Mac-only scope.
- **Why it matters:** Activation could be blocked on a requirement the launch never intends to meet.
- **How to investigate:** Read the Gate 2 definition in the milestone document.
- **Fix direction:** Propose that the Mac-only scope be formally recorded in the Gate 2 text.

**P20 — minor — R21 and open question 11: Orchard dependency for NAVCoin fix fills**
- **What is wrong:** Pillar 7's private fix fills settle in a pool that is not quantum-safe, and the spec offers no plan for moving them.
- **Why it matters:** The quantum-safety story is inconsistent across pillars.
- **How to investigate:** Coordinate with Pillar 7 R6.
- **Fix direction:** Add a roadmap item for moving fix fills to a PQ asset pool, or label them explicitly as not quantum-safe in the app and explorer.
