# Feedback — Pillar 5 — The Task Node Local Experience

> **Mean score:** 60/100 · Gemini 4 Neon 54 · Claude Opus 5.5 66
>
> **Method:** each model received (1) the founder's original prompt and (2) [this pillar's spec](../pillars/05-local-task-node.md), once, with the question below. Run 2026-11-26 20:13 UTC via OpenRouter.

## The question

> On a scale from 0-100, to what extent does this sub-page correspond to the founder's intention, and how likely is it to output something that works for the founder and the project — with 'works' meaning technically robust and economically coherent?
>
> Output your reasoning at length, enumerating problems clearly so that subsequent agents can investigate and improve.

## Gemini 4 Neon — 54/100

*Model `google/gemini-4-neon` · 13326 output tokens*

#### Summary judgement

This sub-page captures the founder's core intent for Deliverable 5—a local Task Node Mac experience that keeps shielded proving and spend keys off the hosted Task Node server—and does a solid job inventorying the project's actual branches, crates, and PRs (`tasknode-apple`, PR 207, PR 232, W-1). As a high-level framing document, it is readable and grounded in reality.

However, as a specification expected to produce a **technically robust and economically coherent** system, it falls well short. It leaves the central app-architecture paradox (native UI parity cost vs. WebView security failure) completely unaddressed, extrapolates x86 server benchmarks to Apple Silicon without verifying ARM64 performance or Binius proof sizes, glosses over the fact that the existing Mac app runs Orchard while the target is an unmerged and unaudited PFSH03 branch, omits shielded transaction fees and validator admission economics entirely, and drops key founder requirements such as memo-type private payments and adversarial stress testing. Handed to an engineering team as written, this spec would stall on the PFSH03 audit/port or produce a brittle local client with major privacy leaks and unpriced DoS surfaces.

#### Correspondence to the founder's intention

**Where it aligns:**
- **Core deliverable match:** Directly implements Deliverable 5 ("logged into the task node locally... virtually everything the task node can do... shielded transactions... not processed on the traditional task node server") and links it to the founder's production-readiness gate ("privacy working - on local macbooks ideally in our task node app").
- **Correct trust-boundary framing:** Explicitly enforces that the hosted Task Node server is never the prover and never holds spend keys, notes, or viewing keys, routing shielded submission directly to validator admission.
- **Sensible interpretation of "local":** Correctly rejects the stubbed `docker-compose.dev.yml` backend in favor of a local client talking to hosted Task Node APIs for standard features and directly to PFTL validators/RPC for shielded flows.

**Where it misses or drifts:**
- **Missing memo-type private payments:** The founder explicitly lists *"works with existing memo type private payments"* as item 2 of chain performance/readiness. The sub-page only mentions Pillar 2 in a single row of the Dependencies table and never specifies how encrypted memos work inside shielded transactions (payload format, IPFS-pinned protobuf compatibility, Task Node task/payout memo UX) in the user story, requirements, or acceptance criteria.
- **Missing FastPay wallet integration:** The founder asks for FastPay wallets to be the default for task payouts; the local Task Node experience never explains how the local wallet handles FastPay alongside normal and shielded balances.
- **Hand-waved feature parity:** The founder asks that the local app do *"virtually everything the task node can do."* Given that Pillars 1, 6, 9, and 10 are rapidly adding major web capabilities (Hive Chat governance, Finance Hub, NFTs, NAVCoin SDK, Information Hive Mind), asserting that parity is automatic simply because the Mac app "talks to the same Task Node APIs" ignores how frontends actually work.
- **Silent scope expansion to multi-asset shielding (`pfUSDC`):** The augmented user story shields `100 pfUSDC`, implying multi-asset shielded pool (MASP) support, without verifying whether PFSH03 supports issued assets or only native PFTL.
- **No adversarial stress testing:** The founder explicitly requires that Post Fiat *"more thoroughly adversarially stress tested every option across multiple models"* as a migration gate. Acceptance criteria only check basic devnet happy paths.

#### Technical robustness

The architecture diagram looks clean at 30,000 feet, but breaks down under engineering scrutiny:

1. **The Native-vs-WebView security/parity trap:** The spec never specifies how the Mac app renders the Task Node UI. If `tasknode-apple` is a native Swift/AppKit app, maintaining parity with a fast-moving web app will fail. If it embeds a WebView loading `tasknode.postfiat.org`, then the hosted Task Node server controls the executable JS/DOM inside the app—destroying the trust boundary unless the UI bundle is locally packaged, version-pinned, signed, and isolated from signing/proving primitives.
2. **Unbridged Orchard → PFSH03 engineering gap:** `tasknode-apple` currently proves **Orchard** (Halo2), whereas PFSH03 (Binius) lives in an unmerged PR (`PR 232`) driven by a separate Rust daemon (`pftl_pq_walletd`). The spec mandates PFSH03 for release without specifying whether the Mac app links PFSH03 in-process (FFI/UniFFI) or bundles `pftl_pq_walletd` as a managed localhost sidecar, nor how the multi-month external audit gate (Gate 1) interacts with the chain-migration timeline.
3. **Unverified ARM64 performance and proof-size physics:** The claim that "2.9 s on 16 cores suggests [≤10 s on a base M-series Mac] is already met" is unsafe. Binius provers rely heavily on binary-field SIMD (`VPCLMULQDQ`/GFNI on x86_64 vs. ARM64 `PMULL`/NEON), and a base M-series Mac has only 4 performance cores (not 16 homogeneous server cores). Moreover, Binius proofs are typically tens to hundreds of kilobytes—orders of magnitude larger than Halo2—yet the spec never analyzes proof byte size, validator ingress bandwidth, or Cobalt consensus latency impact.
4. **Incomplete privacy threat model:** Ensuring no POST to `tasknode.postfiat.org` carries witness bytes is necessary but nowhere near sufficient. The spec ignores: (a) how a user looks up another Task Node user's shielded address without revealing the social graph to Task Node, (b) IP and timing correlation at validator admission (`/api/private/v1/submit`) and note-scanning RPCs, (c) deterministic linkability when shielding from and unshielding back to the user's known public Task Node address, and (d) accidental leaks via crash reporters, analytics, or client logs.
5. **State sync, recovery, and multi-device gaps:** Naive RPC note scanning does not scale past devnet without compact blocks and checkpoints. Backup/recovery omits PQ shielded key derivation, wallet birthday heights, and outgoing viewing keys (OVKs) needed to recover sent-payment history. Multi-device conflicts between the web wasm wallet and the local Mac Keychain (e.g., public account nonces) are unaddressed.
6. **Validator admission DoS and censorship:** Exposing an unauthenticated `/api/private/v1/submit` endpoint accepting large, verification-heavy PQ proofs is an immediate DoS vector. There is no spam-mitigation design, no validator endpoint discovery/failover, and no protection against a single validator censoring shielded transactions.

#### Economic coherence

Economic and incentive design is essentially absent from this sub-page:

1. **No shielded fee mechanism:** Shielded-to-shielded payments cannot pay gas from a public account without destroying anonymity. Fees must be embedded in the shielded circuit's value balance (or paid via a relayer), yet the spec defines neither the fee payment mechanism nor the fee asset—especially critical when the user story shields `pfUSDC` rather than PFTL.
2. **Unpriced verification CPU, bandwidth, and nullifier state bloat:** Large Binius proofs impose real verification CPU and bandwidth costs on every validator, and every shielded spend permanently grows the nullifier set. Without a fee schedule tied to proof size, verification cost, and permanent state growth, the shielded pool is economically unsustainable and vulnerable to state-bloat attacks.
3. **Zero validator incentive to serve privacy endpoints:** Why should a validator expose `/api/private/v1/{snapshot,submit,receipt}`, absorb proof-verification DoS risk, and serve note-scanning bandwidth to desktop clients? Unless Pillar 3 validator rewards/uptime scoring require serving shielded admission—or admitting validators/RPCs earn a fee share—rational validators will keep shielded submission off or rate-limit it into unusability.
4. **Unresolved collision with NAVCoin fees and KYC/trust-lines (Deliverables 7 & 8):** If issued assets like `pfUSDC` or NAVCoins can enter the shielded pool, can users bypass Deliverable 7's deterministic management/performance fees or Deliverable 8's optional KYC/trust-line restrictions? Conversely, if only permissionless assets can be shielded, that boundary—and any selective-disclosure viewing-key mechanism—must be explicitly defined.
5. **Poor resource-allocation economics (Mac-only native app vs. local daemon):** Committing the critical path to a Mac-only native app requires ongoing dual-frontend engineering while excluding Windows and Linux users (including most validators and power developers). Meanwhile, the cross-platform `pftl_pq_walletd` local daemon pattern already exists in PR 207 and is punted to "Later (R8)" without comparing total engineering cost, time-to-mainnet, or user reach. Likewise, the cost ($100k–$300k+) and multi-month lead time of the PFSH03 external audit gate are treated as a footnote despite blocking chain readiness.

#### Problems

##### P1 — Unspecified app architecture creates a feature-parity vs. security paradox
- **Severity:** critical
- **Where:** User story (step 1), Architecture diagram, Technical requirements (R5, R7)
- **What is wrong:** The spec claims the Mac app stays at full parity with the hosted web app because "it talks to the same Task Node APIs," while guaranteeing the Task Node server never sees secrets, keys, or proofs. It never specifies whether `tasknode-apple` is a pure native SwiftUI app or a WebView wrapper around the hosted web app.
- **Why it matters:** Both unspoken choices fail unless explicitly engineered around: a pure native app will permanently lag behind rapid web iterations across Pillars 1, 6, 9, and 10, breaking the founder's "virtually everything" requirement; a remote-loaded WebView lets the Task Node server ship arbitrary JavaScript into the local client, collapsing the privacy boundary.
- **How an agent should investigate it:** Inspect `tasknode-apple` (architecture, UI framework, WebView usage, IPC bridge between UI and Rust core) and catalog all hosted Task Node web features planned across Pillars 1–10.
- **Suggested direction for the fix:** Specify the exact UI/core split. Either (a) ship a locally bundled, signed, hash-pinned static web build (Tauri/Electron/WKWebView with strict CSP and no remote script execution) talking to a local Rust prover over a privileged IPC channel, or (b) clearly scope which screens are native vs. web and enforce a hard privilege separation so untrusted web views can never touch spend/viewing keys or raw witness data.

##### P2 — Missing shielded transaction fee model, multi-asset support verification, and state-bloat pricing
- **Severity:** critical
- **Where:** User story (step 3: "Shield 100 pfUSDC"), Sequence diagram ("GET snapshot (anchor, fee)"), Technical requirements
- **What is wrong:** The user story shields and spends `pfUSDC`, and the sequence diagram mentions a `fee`, but the requirements never verify whether PFSH03 supports multi-asset shielded pools (MASP) or only native PFTL, never specify how fees are paid privately during shielded-to-shielded transfers, and never price proof verification or permanent nullifier storage.
- **Why it matters:** Paying fees from a public account destroys sender anonymity; requiring shielded PFTL to move shielded `pfUSDC` breaks the user story unless handled explicitly; underpricing large Binius proofs and monotonic nullifier growth invites cheap state-bloat and consensus slowdowns.
- **How an agent should investigate it:** Inspect PR 207 and PR 232 (`crates/privacy`, PFSH03 circuit specs, Lean model) to determine whether assets are single-asset or multi-asset, how value balance / fee outputs are constrained in the circuit, what the exact proof size in bytes is, and how validator fee accounting works.
- **Suggested direction for the fix:** Add explicit requirements for (1) supported shielded assets at launch (aligning the user story with actual PFSH03 circuit capabilities), (2) in-circuit fee deduction (or shielded fee asset/relayer rules), and (3) a fee formula reflecting Binius proof size, verification CPU, and permanent nullifier storage cost.

##### P3 — Unbridged Orchard-to-PFSH03 integration gap and unmitigated audit critical-path blocker
- **Severity:** critical
- **Where:** What exists today, Technical requirements (R1), Dependencies
- **What is wrong:** `tasknode-apple` currently implements Orchard (Halo2), while PFSH03 (Binius) is an unmerged PR (`PR 232`) with a standalone daemon (`pftl_pq_walletd`) blocked by Gates 1–3 (external audit, genesis/mobile qualification, governance vote). The spec mandates PFSH03 for launch with no integration plan for `tasknode-apple` and no contingency for audit lead time.
- **Why it matters:** Pillar 5 is a chain production-readiness gate. An unbudgeted, unscheduled external cryptographic audit on novel Binius circuits plus a full prover swap in `tasknode-apple` can easily slip chain launch by 4–6+ months.
- **How an agent should investigate it:** Inspect `tasknode-apple/core/src/{prover,shielded,private_actions}.rs`, PR 232's Gate 1–3 definitions in `handoff-2026-11-25.md`, and the current audit status/vendor timeline. Check whether `pftl_pq_walletd` already exposes an interface usable by `tasknode-apple`.
- **Suggested direction for the fix:** Define the concrete prover integration interface (in-process Rust crate vs. bundled `pftl_pq_walletd` sidecar behind a trait), document the audit schedule/budget/pool-cap strategy (e.g., capped beta pool post-internal review vs. hard wait for external audit), and explicitly decide Orchard's deprecation/migration path rather than deferring it to an open question.

##### P4 — Missing validator admission DoS protection, routing/failover, and validator economic incentives
- **Severity:** critical
- **Where:** Architecture, Technical requirements (R3), Dependencies
- **What is wrong:** Direct submission to validator endpoints (`/api/private/v1/submit`) lacks spam/DoS protection, endpoint discovery/failover, anti-censorship handling, and any economic reason for validators to expose and fund these endpoints.
- **Why it matters:** Without rate limiting, proof-of-work/fee-gating, or cheap pre-checks, an attacker can flood validators with invalid Binius proofs and exhaust CPU/bandwidth. Without tying admission availability to Pillar 3 validator rewards or fee splits, validators have every economic incentive to leave shielded submission turned off.
- **How an agent should investigate it:** Read `docs/plans/active/user-proven-shielded-actions-milestone.md` (W-1), `crates/node` RPC/admission code, and Pillar 3 (`03-validator-program.md`) to see how validators are scored, rewarded, and discovered by clients.
- **Suggested direction for the fix:** Specify (1) cheap pre-verification checks and anonymous DoS controls (e.g., adaptive rate limits, lightweight client puzzles, or admission fee credits), (2) multi-endpoint discovery and automatic failover so no single validator can censor or profile a user, and (3) Pillar 3 economic alignment (validator fee share or uptime/reward requirement for serving `/api/private/v1/*`).

##### P5 — Unverified Apple Silicon (ARM64) proving performance and Binius proof-size impact
- **Severity:** major
- **Where:** In one paragraph, What exists today, Technical requirements (R2), Acceptance criteria (4)
- **What is wrong:** Claims "2.9 s on 16 cores suggests [≤10 s on a base M-series Mac] is already met," extrapolating from an unspecified (likely x86_64 AVX2/AVX-512) 16-core machine to a base M-series Mac with 4 performance cores and ARM64 SIMD. Also omits Binius proof byte size, memory ceiling on 8 GB / 16 GB MacBooks, and Cobalt block propagation impact.
- **Why it matters:** Binius binary-field proving can slow down 3–10× without x86 GFNI/AVX-512 optimizations, potentially turning a 3 s desktop flow into a 25–40 s thermal-throttled freeze. Large proofs can also degrade Cobalt consensus throughput.
- **How an agent should investigate it:** Check PR 207 / PR 232 benchmarks (`crates/privacy`, Binius backend SIMD flags), measure actual proving time, peak RAM, and proof size on a base M1/M2/M3 MacBook Air (8 GB RAM), and check Cobalt block size/bandwidth limits.
- **Suggested direction for the fix:** Replace the assumption in R2 with hard budgets verified prior to freeze: wall-clock proving time on base M1/M2 (ARM64 NEON), peak RSS memory budget (e.g., ≤1 GB), thermal/battery behavior, and maximum proof byte size coordinated with Cobalt consensus parameters.

##### P6 — Privacy and metadata leakage beyond the Task Node API boundary
- **Severity:** major
- **Where:** User story (steps 3–6), Architecture, Technical requirements (R4, R7)
- **What is wrong:** The privacy model stops at "no request carrying the note goes to `tasknode.postfiat.org`." It ignores recipient shielded-address resolution (if looked up via Task Node, Task Node learns sender, recipient, and timing), IP/timing correlation at validator admission and note-scanning RPCs, public-leg timing/amount correlation when shielding/unshielding from the user's known Task Node address, and client telemetry/crash-reporter leaks. R7's CI test ("no Task Node server route accepts shielded witness data") does not detect client-side leaks into generic logging or analytics.
- **Why it matters:** A user relying on the UI's *"Proved on this device · submitted to validators directly"* badge can still be trivially deanonymized by Task Node (via address lookup or amount/timing matching) or by a single validator logging IPs.
- **How an agent should investigate it:** Review `docs/privacy/` (especially metadata bounds), check how Task Node resolves usernames/identities (Pillar 6 / NOSTR / on-chain identity), and audit `tasknode-apple` and `tasknode` for analytics, telemetry, and crash-reporting SDKs.
- **Suggested direction for the fix:** Define a formal privacy threat model covering: (1) decentralized/on-chain or offline shielded-address discovery (e.g., Pillar 6 identity records fetched from PFTL RPC, not Task Node), (2) randomized submission delays, optional Tor/proxy/relayer routing, and multi-RPC rotation, (3) UX warnings for exact-amount quick shield→unshield correlation, and (4) client-side network-egress allowlist tests plus mandatory telemetry stripping in shielded flows.

##### P7 — Missing founder requirements: memo-type private payments and FastPay integration
- **Severity:** major
- **Where:** User story, Technical requirements, Dependencies, Acceptance criteria
- **What is wrong:** The founder explicitly called out *"works with existing memo type private payments"* and FastPay default payouts for chain readiness. The sub-page omits encrypted memos entirely from the user story, technical requirements, and acceptance criteria, and never mentions FastPay.
- **Why it matters:** On Post Fiat, payments carry structured memos (task submissions, IPFS-pinned protobuf payloads, machine-native messages). A shielded transfer that cannot carry an encrypted Pillar 2 memo cannot replace existing private memo payments in Task Node workflows.
- **How an agent should investigate it:** Read Pillar 2 (`02-message-protocol.md`), inspect existing Task Node private memo payment code in `tasknode`, and check how PFSH03 notes encode encrypted memo fields (byte capacity, encryption scheme, IPFS pointer conventions) and how FastPay interacts with L1 wallets.
- **Suggested direction for the fix:** Add explicit requirements and acceptance criteria for (1) encrypted Pillar 2 memo attachment and recipient decryption inside shielded payments (including size bounds / IPFS pointer support), and (2) clear UX and key-management integration between the local wallet's FastPay balance, public L1 balance, and shielded balance.

##### P8 — Note scanning scalability, key backup/recovery (OVK & birthday height), and web/local multi-device conflicts
- **Severity:** major
- **Where:** Architecture, Technical requirements (R4, R6)
- **What is wrong:** R4 relies on scanning notes from PFTL RPC without a compact-block sync design. R6 assumes backup "matches the web wallet's seed vault" without defining PQ shielded key derivation from the root seed, wallet birthday height, outgoing viewing keys (to recover sent payment history and memos), or multi-device state sync when a user uses both web Task Node and local Mac Task Node.
- **Why it matters:** Full trial decryption over generic RPC degrades as chain volume grows; restoring a wallet without birthday height or OVKs results in slow syncs and missing transaction history; using the same public key simultaneously in web wasm and Mac Keychain causes nonce collisions and confusing balance state.
- **How an agent should investigate it:** Inspect `crates/wallet_wasm`, `tasknode-apple/core/src/private_vault.rs`, and `pftl_pq_walletd` to check key derivation paths, note-scanning endpoints, and nonce management.
- **Suggested direction for the fix:** Specify (1) a compact-block / incremental Merkle-tree sync protocol for viewing-key scanning, (2) deterministic PQ shielded key + OVK derivation with seed + birthday-height backup format, and (3) explicit multi-device key/nonce behavior between hosted web and local app.

##### P9 — Unresolved interaction with KYC/trust-line assets and fee-bearing NAVCoins (Deliverables 7 & 8)
- **Severity:** major
- **Where:** User story (pfUSDC), What exists today (`disclosure` docs), Technical requirements
- **What is wrong:** The spec introduces shielding of `pfUSDC` without defining asset eligibility rules for the shielded pool or selective-disclosure requirements.
- **Why it matters:** Deliverable 7 introduces deterministic NAVCoin performance/management fees, and Deliverable 8 introduces optional KYC/trust-line assets. Allowing restricted or fee-bearing assets into an unrestricted shielded pool breaks protocol economics and compliance guarantees; conversely, institutional/validator users may require viewing-key selective disclosure (referenced in `docs/privacy/` but omitted from requirements).
- **How an agent should investigate it:** Cross-check Pillars 7 and 8 (`07-*`, `08-*`), `docs/privacy/` disclosure documentation, and PFSH03 pool asset-whitelist logic.
- **Suggested direction for the fix:** Explicitly restrict the v1 shielded pool to permissionless, non-fee-bearing assets (e.g., PFTL and permissionless pfUSDC), enforce asset eligibility at consensus level, and add a requirement for user-initiated selective disclosure (payment/viewing proofs) if compliance verification is needed.

##### P10 — Unexamined strategic and economic tradeoff: Mac-only native app vs. cross-platform local daemon
- **Severity:** major
- **Where:** Technical requirements (R5, R8), Open questions for the founder
- **What is wrong:** Punts the already-built `pftl_pq_walletd` local daemon pattern to "Later (R8)" while placing a Mac-only native app on the chain-launch critical path, without evaluating engineering burn, maintenance cost, or exclusion of Linux/Windows users.
- **Why it matters:** Many validators, developers, and power users run Linux or Windows. Maintaining full web parity in a Mac-only native app consumes scarce engineering bandwidth while leaving non-Mac users unable to use shielded transactions at launch.
- **How an agent should investigate it:** Measure the remaining engineering effort to reach feature parity in `tasknode-apple` vs. shipping a signed desktop shell (macOS/Linux/Windows) or browser+`pftl_pq_walletd` pairing with a verified local UI bundle.
- **Suggested direction for the fix:** Add a brief architecture decision record (ADR) comparing both paths on time-to-launch, security boundary, OS coverage, and ongoing maintenance cost. Strongly consider making `pftl_pq_walletd` the shared local proving engine underneath both the signed Mac app (for turnkey UX) and Linux/Windows CLI/browser setups so the proving stack is built once.

##### P11 — Weak acceptance criteria: devnet-only, no adversarial stress tests, and missing failure/CI gates
- **Severity:** major
- **Where:** Acceptance criteria (items 1–5)
- **What is wrong:** Acceptance criteria only test devnet happy paths and omit: (a) the founder's explicit adversarial stress-testing requirement, (b) double-spend / nullifier replay / invalid-proof rejection, (c) expired-anchor / reorg / network-drop recovery, (d) seed backup and restore with private balance recovery, (e) encrypted memo verification, and (f) the CI privacy-boundary test required by R7.
- **Why it matters:** Devnet happy-path tests do not prove production readiness for a shielded financial system where bugs mean permanent fund loss or silent privacy failure.
- **How an agent should investigate it:** Review existing integration/adversarial test suites in `postfiatl1v2` (PR 207 / PR 232) and `tasknode-apple`, plus the Lean model assumptions (`handoff-2026-11-25.md`), to identify untested boundary conditions between the Lean spec, Rust prover, validator admission, and client app.
- **Suggested direction for the fix:** Expand acceptance criteria to include: (1) mainnet-like testnet soak and adversarial tests (nullifier replay, malformed/oversized proofs, anchor expiry, admission flooding), (2) full backup-and-restore test on a clean machine, (3) encrypted memo round-trip test, (4) automated CI egress/privacy test asserting zero shielded payload or metadata leakage to Task Node or third-party telemetry, and (5) audited ARM64 proving latency and memory benchmarks.

##### P12 — Non-portable local filesystem path and unverified private/anomalously dated references
- **Severity:** minor
- **Where:** What exists today, Code references
- **What is wrong:** Cites a machine-local absolute path (`/home/postfiat/var/pq-work/HANDOFF-2026-11-23.md`) that other contributors and automated agents cannot access, and relies on private-repo docs (`tasknode-apple/docs/PRIVATE-PAYMENTS-SPEC-20261125.md`) and future-looking dates (`2026-11-25`) without summarizing critical invariants inline.
- **Why it matters:** Subsequent agents or external auditors investigating this pillar cannot verify claims anchored to an unreachable local filesystem path.
- **How an agent should investigate it:** Check whether `/home/postfiat/var/pq-work/HANDOFF-2026-11-23.md` has been committed to `postfiatl1v2/docs/status/` and verify the actual filenames/commit hashes of the referenced handoffs and `tasknode-apple` specs.
- **Suggested direction for the fix:** Commit any load-bearing handoff docs into the repository (or link to the committed version with exact commit/PR references) and inline any essential constraints from the private `tasknode-apple` spec directly into Pillar 5.

## Claude Opus 5.5 — 66/100

*Model `anthropic/claude-opus-5.5` · 9169 output tokens*

#### Summary judgement

This is one of the stronger kinds of sub-page: it is grounded, concrete, and honest about status. It inventories what exists, with branch and PR state, proof timings and memory, and the network flag being off. It frames the core invariant correctly: the Task Node server is never the prover and never holds spend keys. It also gives a sensible architecture, testable acceptance criteria, and good open questions. It reads the founder's "privacy working on local MacBooks" as a chain-readiness gate, which is right.

It falls short of "will produce something that works" for five main reasons:

- **The core engineering gap is not stated as a work item.** The Mac app proves Orchard, while the release target is PFSH03 (Binius). Porting the PFSH03 prover into the Mac app is never stated as a requirement.
- **The privacy claim is over-strong.** "Task Node's server sees nothing" ignores metadata, the public legs of shield and unshield, and the fact that the same organisation likely runs the validators and RPC.
- **The submission path has no economics or safety layer.** Fees, fee-payer privacy, and anti-DoS on an anonymous proof-verification endpoint are absent.
- **Founder-stated readiness criteria are dropped.** Existing memo-type private payments and FastPay-default task payouts are not mentioned.
- **The governance and audit gates are acknowledged but not planned.** There is no Pillar 1 dependency, no audit scope, cost or timeline, and no owner.

#### Correspondence to the founder's intention

**What it gets right:**
- **The founder's literal story is preserved and well augmented.** The augmented story covers install, local wallet, shield, pay privately, unshield, and a trust indicator. The "do not want those processed on the traditional Task Node server" requirement becomes a measurable invariant (acceptance criterion 2, network capture).
- **It links up to the production-ready chain goal.** It cites the post-quantum stance consistent with Deliverable 4's "quantum resistant" direction.
- **The open questions are the right ones.** It asks about Orchard migration, platforms, and browser plus daemon.

**Where it diverges or under-delivers:**
- **"It can do virtually everything the task node can do" is reinterpreted.** The sub-page delivers a thin native client over hosted APIs plus local keys and prover. That is defensible, but the founder may have meant a genuinely local Task Node: local context, local agent, reduced server dependence. The page never surfaces this interpretation choice to the founder.
- **Founder performance criterion #2 is unaddressed.** "Works with existing memo type private payments" is a chain-readiness criterion that belongs squarely in the privacy pillar. The page never mentions the existing memo-based private payments, or how they coexist with or migrate to shielded notes.
- **Founder criterion #3 is unaddressed.** "FastPay wallets default for task payouts" is not reconciled with shielded flows. Can task payouts land shielded? Does FastPay apply to shielded actions?
- **Founder criterion #5 is only partly addressed.** "Adversarially stress tested … across multiple models" is reduced to "external review or audit". The founder's Lean-proof interest is cited (9 modules, 0 `sorry`) without saying what is proven.
- **Cross-deliverable interactions are missing.** NAVCoin KYC and permissioned assets (D7/D8) versus a shielded pool holding pfUSDC are not considered. Neither is identity linkage (D6).

#### Technical robustness

**Strengths:**
- The architecture correctly separates the hosted API path (chat, tasks, Hive) from the proof path (validator admission).
- The snapshot → prove → submit → receipt sequence is sound.
- The page clarifies that the GPU window is for bridge proofs, not shielded proving.

**Weaknesses:**
1. **Prover-to-app integration is missing.** `tasknode-apple` has Orchard proving. PFSH02/03 exist as a Linux daemon (`pftl_pq_walletd`) and a web component. No requirement says "integrate the PFSH03 prover into the Mac app's Rust core, on arm64". That is the main build task of this pillar.
2. **The benchmark extrapolation is unsafe.**
   - The 2.9 s figure is PFSH02 on 16 cores, probably x86. The target is PFSH03 on a base M-series chip, which has 4 performance and 4 efficiency cores.
   - Binius performance depends heavily on carry-less multiply and SIMD paths (x86 PCLMUL/GFNI versus ARM PMULL).
   - The single-core figure of 26.7 s implies roughly 6–10 s on a base M1 or M2. R2's "suggests this is already met" is optimistic.
   - The 562 MiB memory figure is fine for Mac but relevant to the "mobile qualification" gate.
3. **The privacy model is under-specified.**
   - The same IP talks to the Task Node server and to validator/RPC endpoints, which allows timing correlation.
   - Validators and RPC are likely operated by the same organisation (e.g. validator 2 runs `pftl_swapd`).
   - Shield and unshield legs are public and tied to the user's known Task Node account.
   - Note fetching and scanning patterns may leak information if not done as full-block trial decryption.
   - Neither the "privacy receipt" nor "Task Node's server sees nothing" acknowledges any of this. There is no threat model.
4. **Admission DoS and fees are absent.** An unauthenticated endpoint that verifies proofs is a cheap DoS vector. The snapshot returns a "fee", but the page never says:
   - who pays it;
   - in what asset;
   - from which account (a fee from a public account links identity);
   - how mempool and rate limits work.
5. **Key derivation and recovery are vague.** R6 says only "match the web wallet's seed vault". It does not say whether PQ spend and viewing keys derive deterministically from the existing PFTL seed. It does not cover how notes are recovered on a new device, or how viewing-key export for disclosure works (the privacy docs mention disclosure).
6. **Multi-asset support is unverified.** The user story shields pfUSDC, but the page does not confirm PFSH03 supports multiple assets or bridged assets.
7. **R7's CI test is weak.** "No Task Node server route accepts shielded witness data" is hard to assert statically. The realistic assurance is an end-to-end network-capture test plus code ownership boundaries.
8. **The acceptance criteria stop at devnet.** The founder's gate is production readiness. Testnet or mainnet enablement, audit closure and a rollback plan are not in the acceptance criteria.
9. **References are not verifiable.** Private repo paths, a local filesystem path (`/home/postfiat/var/...`) and late-2026 dated docs cannot be checked by reviewers. Some claims may be stale or hallucinated.

#### Economic coherence

- **Audit cost and timeline are unaddressed.** Gate 1 (external audit of a novel Binius shielded pool) is likely the dominant cost and schedule item for the whole chain-readiness gate. The page does not scope it, price it, or propose staged alternatives, such as an audited Orchard-first launch with PQ later.
- **Maintaining a native client is a recurring cost.** Two clients (web plus native) means a parity burden. There are no options listed (Tauri or webview wrapping, or a shared Rust core with the web UI) and no statement of what the hosted team must keep compatible.
- **Fee economics are absent.** Shielded fees must cover validator verification cost. Fee-payer privacy affects whether anyone uses the pool. Treatment of custodial `pftl_swapd` (privacy theatre if left in place) is not resolved.
- **Regulatory and economic fit with NAVCoins is unexamined.** KYC-gated assets in a shielded pool affect whether NAVCoin issuers can or will use privacy. This could make the readiness gate commercially irrelevant if NAVCoin assets cannot be shielded.
- **Validator incentives are not linked to Pillar 3.** Validators would bear proof-verification load and run admission endpoints, but compensation is not connected to the validator program.

#### Problems

**P1 — Missing requirement: port the PFSH03 prover into the Mac app**
- **Severity:** critical
- **Where:** "What exists today" rows for the Mac app and PFSH02/03; R1, R2, R5.
- **What is wrong:** The Mac app proves Orchard only. The PQ prover exists only as a Linux daemon and web flow on unmerged branches. No requirement or acceptance step covers integrating PFSH03 proving, note scanning and key management into `tasknode-apple`'s Rust core on arm64.
- **Why it matters:** This is the actual build work of the pillar. Without it, the plan says "pick PFSH03" while the only shipping-shaped app runs Orchard.
- **Investigate:** Check whether the PFSH03 crates compile for `aarch64-apple-darwin`. Check whether `tasknode-apple/core` can depend on them, and what the FFI surface is.
- **Fix:** Add explicit requirements:
  - "R2a: PFSH03 prover integrated in the Mac app core."
  - "R2b: viewing-key scanner for the PFSH03 note format."
  - An acceptance criterion that the release build uses PFSH03.

**P2 — Unsafe extrapolation of proving performance**
- **Severity:** major
- **Where:** R2; "In one paragraph"; user story step 3.
- **What is wrong:** PFSH02 timings on 16 cores (likely x86) are used to claim the PFSH03 ≤10 s target on a base M-series Mac is "already met".
- **Why it matters:** UX and mobile-qualification gates depend on it. Binius performance is architecture-sensitive.
- **Investigate:** Benchmark PFSH03 on base M1, M2 and M3 (8-core) and Intel Macs, for every action type (shield, transfer, unshield), recording cold and warm runs and peak RSS.
- **Fix:** Make the target per action and per machine class. Add a fallback plan if it is missed, such as a progress UI, background proving or precomputation.

**P3 — No threat model; privacy claims overstated**
- **Severity:** major
- **Where:** user story steps 3 and 6; R4; R7; architecture ("never sees notes, keys or proofs").
- **What is wrong:** The page ignores four leaks:
  - IP and timing correlation between Task Node API calls and validator submissions;
  - shared operation of validators, RPC and Task Node;
  - public shield and unshield legs tied to known accounts;
  - possible leakage from note-fetch patterns.
- **Why it matters:** The founder's intent is real privacy. A UI "privacy receipt" that implies more than is true is a reputational risk.
- **Investigate:** Find who operates the public RPC and validators. Check whether scanning is full trial decryption or targeted queries. Check whether shield funding comes from the identified Task Node account.
- **Fix:** Add a threat-model section and an honest receipt wording. Consider:
  - full-block scanning;
  - optional Tor or relay submission;
  - multiple independent RPC endpoints;
  - user guidance on unlinkability (amounts, timing).

**P4 — Admission endpoint has no DoS protection or fee design**
- **Severity:** major
- **Where:** R3; sequence diagram (`GET snapshot (anchor, fee)`).
- **What is wrong:** There is no spec for:
  - fee asset or payer;
  - how fees are paid without linking identity;
  - rate limiting or anti-spam for anonymous proof-verification requests;
  - mempool behaviour or nullifier races.
- **Why it matters:** Proof verification is costly, so an open endpoint invites DoS. A fee paid from a public account undermines privacy.
- **Investigate:** Read the W-1 milestone plan for its fee and anti-spam design. Check whether PFSH03 supports in-pool fee payment.
- **Fix:** Specify:
  - fees paid from shielded value;
  - cheap pre-verification checks;
  - per-IP and proof-of-work or stake-based rate limits;
  - receipts with failure codes.

**P5 — Existing memo-type private payments not addressed**
- **Severity:** major
- **Where:** absent; it should be in "What exists today" and in the requirements.
- **What is wrong:** The founder lists "works with existing memo type private payments" as a chain-performance criterion. The page never mentions these payments or their relation to the shielded pool.
- **Why it matters:** It is a stated readiness gate. Existing Task Node users depend on memo-based private payments.
- **Investigate:** Locate the current memo-encryption payment flow in Task Node and on the old chain, and the Pillar 2 message-protocol encryption.
- **Fix:** Add a requirement defining coexistence: memo privacy (content) versus shielded privacy (amounts and graph). Add a migration path and the UX distinction.

**P6 — FastPay and task-payout integration missing**
- **Severity:** major
- **Where:** absent.
- **What is wrong:** The founder wants FastPay as the default for task payouts. The page does not say whether payouts can be received shielded, whether FastPay applies to shielded actions, or how latency trade-offs are handled.
- **Why it matters:** Task payouts are the main money flow in Task Node. Privacy that cannot touch payouts has limited value.
- **Investigate:** Read the FastPay design in `postfiatl1v2` for compatibility with shielded notes or a shield-on-receipt flow.
- **Fix:** Add an optional "auto-shield payouts" flow, or document why FastPay stays public with a manual shield step.

**P7 — Governance dependency not linked to Pillar 1**
- **Severity:** major
- **Where:** Dependencies table; acceptance criterion 5; R1 (governance vote).
- **What is wrong:** Enabling shielded submission requires a governance vote. The dependency on the Hive Chat governance module and the deterministic governance framework is not listed.
- **Why it matters:** The founder calls governance the core of the project. This pillar is blocked on it with no sequencing.
- **Investigate:** Check the Pillar 1 spec for how protocol-flag votes are run and recorded.
- **Fix:** Add Pillar 1 as a dependency. Define the governance packet contents for the PFSH03 enablement vote: audit report, benchmarks, Lean status.

**P8 — Audit gate unscoped and uncosted**
- **Severity:** major
- **Where:** R1; Dependencies ("PFSH03 merge plus Gates 1–3").
- **What is wrong:** There is no audit scope, vendor, cost, timeline or staging. "External review or audit" is ambiguous. The Lean model's proven properties are not stated.
- **Why it matters:** This likely gates the entire production-readiness milestone. The founder asked for multi-model adversarial stress testing and Lean work.
- **Investigate:** Read the PR 232 gate definitions and the handoff docs. List the theorems in the 9 Lean modules and what is out of scope (e.g. circuit soundness versus model).
- **Fix:** Add an assurance plan:
  - multi-model adversarial review, as the founder requested;
  - a Lean coverage statement;
  - a scoped external audit with budget and date;
  - a possible staged launch (caps or limits on pool size).

**P9 — Interpretation of "local Task Node" not surfaced**
- **Severity:** minor
- **Where:** "In one paragraph"; R5 ("uses the existing Task Node APIs").
- **What is wrong:** The page delivers a thin client with local keys and prover. The founder's phrasing could mean broader local-first operation.
- **Why it matters:** Possible mismatch in scope expectations.
- **Investigate:** Compare with the founder's other statements on local operation; the `docker-compose.dev.yml` notes say it does not reproduce production.
- **Fix:** Add an open question: "thin client with local privacy, or local-first Task Node?" State the recommended choice and its cost.

**P10 — Multi-asset and pfUSDC support unverified; NAVCoin and KYC interaction ignored**
- **Severity:** major
- **Where:** user story step 3; requirements.
- **What is wrong:** The page assumes shielding pfUSDC works. It does not address bridged assets, NAVCoins, or KYC- and trust-line-gated assets (D7/D8) in the pool.
- **Why it matters:** Economic relevance. If NAVCoin assets cannot be shielded, or issuers object, privacy usage may be minimal.
- **Investigate:** Check the PFSH03 asset model, the bridge design for pfUSDC, and the D8 trust-line design.
- **Fix:** Specify supported assets, the policy for permissioned assets, and disclosure and viewing-key options for compliance.

**P11 — Custodial private swap left unresolved**
- **Severity:** minor
- **Where:** "What exists today" (`pftl_swapd` on validator 2).
- **What is wrong:** The page notes the live private swap is custodial but sets no requirement to deprecate it or label it.
- **Why it matters:** It contradicts the pillar's invariant and may confuse users.
- **Investigate:** Find current usage and dependencies of `pftl_swapd`.
- **Fix:** Add a deprecation or relabel requirement once W-1 ships.

**P12 — Key derivation and recovery unspecified**
- **Severity:** minor
- **Where:** R6.
- **What is wrong:** The page does not cover:
  - PQ spend and viewing key derivation from the existing seed;
  - note recovery on a new device;
  - viewing-key export for disclosure.
- **Why it matters:** This is the main source of user fund-loss incidents.
- **Investigate:** Check the key schemes in the PFSH03 and Orchard crates and the web seed vault.
- **Fix:** Define a deterministic derivation path, a rescan-from-height recovery flow, and a disclosure export.

**P13 — Acceptance stops at devnet; no production criteria**
- **Severity:** minor
- **Where:** Acceptance criteria 1–5.
- **What is wrong:** The criteria end at devnet plus a governance enablement step. There is no testnet or mainnet soak, pool caps, incident or rollback plan, or release distribution metric.
- **Why it matters:** The pillar is framed as a production-readiness gate.
- **Investigate:** Compare with the chain migration phases in other pillars.
- **Fix:** Add mainnet enablement criteria, a soak duration, a value cap, and a kill-switch procedure.

**P14 — Unverifiable references**
- **Severity:** minor
- **Where:** "Code references"; private-repo rows.
- **What is wrong:** There are local filesystem paths, private repo files, and dated documents that reviewers cannot check.
- **Why it matters:** Downstream agents may act on stale or incorrect status.
- **Investigate:** Verify PR 207 and PR 232 state, the network-status flag, and the W-1 plan contents directly.
- **Fix:** Replace local paths with committed docs. Mark private references with an owner who can confirm them.

**P15 — Weak test for the server-never-sees invariant**
- **Severity:** minor
- **Where:** R7.
- **What is wrong:** The test "no server route accepts witness data" is hard to enforce in CI.
- **Why it matters:** False assurance.
- **Investigate:** Look at the existing Task Node test harness.
- **Fix:** Replace it with an E2E network-capture test against the release build, plus an architectural boundary: the prover crate is never linked into server builds.

**P16 — Native-app maintenance cost unexamined**
- **Severity:** minor
- **Where:** R5; open questions.
- **What is wrong:** There is no plan for keeping parity between the web and native clients, and no evaluation of a wrapper approach versus native UI.
- **Why it matters:** Recurring engineering cost and drift in "virtually everything" parity.
- **Investigate:** Check the current `tasknode-apple` UI stack.
- **Fix:** Propose a shared web UI in a native shell with a Rust core, plus a parity checklist.
