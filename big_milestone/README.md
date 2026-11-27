# Post Fiat — The Big Milestone

> **What this is:** The master execution spec and milestone tracker for taking Post Fiat from zero to production. Every open founder decision across all 10 pillars has been resolved and locked in — **zero open questions remain**. Every pillar has concrete `[ ]` build milestones plus the four mandatory ship gates: **1) Executed, 2) Tested by user, 3) Added to docs, 4) Public blog post document**.

## The goal

**Post-human governance is a new primitive, and Post Fiat is actively building it in public.**

The core loop is simple: **information packets are published, acted on, and audited in public.** Anyone who encounters Post Fiat should immediately see a real, working post-human network running on a fast, quantum-safe chain with an open financial primitive anyone can build on.

That breaks down into **three outcomes** across **ten pillars**:

| Outcome | What it delivers | Pillars |
|---|---|---|
| **A. Post-human governance, in public** | On-demand public governance sessions with direct carry-over accountability, verifiable on-chain Nostr identity, and an Information Hive Mind rewarding real alpha. | [1 · Hive Governance](pillars/01-hive-governance.md) · [6 · Hive Identity](pillars/06-hive-identity.md) · [10 · Information Hive Mind](pillars/10-information-hive-mind.md) |
| **B. A production-ready new chain (`postfiatl1v2`)** | Replaces the old XRPL-fork chain with sub-second FastPay, post-quantum messaging, NFTs, outside validators, local Mac privacy (`PFSH03`), and Finance Hub. | [2 · Message Protocol](pillars/02-message-protocol.md) · [3 · Validators](pillars/03-validator-program.md) · [4 · NFTs](pillars/04-nft-system.md) · [5 · Local Task Node](pillars/05-local-task-node.md) · [9 · Finance Hub](pillars/09-finance-hub.md) |
| **C. NAVCoins anyone can build** | An open NAVCoin SDK with optional deterministic fees (zero hedge-fund clutter when off), permissionless-by-default assets, and opt-in KYC credentials. | [7 · NAVCoin Fees & SDK](pillars/07-navcoin-fees.md) · [8 · On-chain Identity & KYC](pillars/08-onchain-identity.md) |

```mermaid
flowchart LR
    P1[1 · Hive Governance] --> A
    P6[6 · Hive Identity] --> A
    P10[10 · Information Hive Mind] --> A
    P2[2 · Message Protocol] --> B
    P3[3 · Validator Program] --> B
    P4[4 · NFT System] --> B
    P5[5 · Local Task Node + PFSH03] --> B
    P9[9 · Finance Hub] --> B
    P7[7 · Optional NAVCoin Fees & SDK] --> C
    P8[8 · On-chain Identity & Optional Trust Lines] --> C
    A[A · Post-human Governance in Public] --> S((Post Fiat Succeeds))
    B[B · Production-Ready New Chain] --> S
    C[C · Open NAVCoin Primitive] --> S
```

*Naming note: Task Node's legacy code calls the **old** XRPL-fork chain "PFTL", while the new chain repo uses `pftl_*` for the **new** chain. Across all 10 specs, **old chain** means the XRPL fork and **new chain (`postfiatl1v2`)** means Post Fiat L1v2.*

---

## Master milestone checklist (all 10 pillars)

Every pillar is complete **only** when all four ship gates are checked off:

### Pillar 1 — [Hive Chat Governance Module](pillars/01-hive-governance.md)
*Founder-triggered 30-min public Hive sessions with auto-snapshot, direct carry-over accountability from the previous session (no auto PFT bounties), `postfiatl1v2` anchoring, and optional flagged deterministic rulings on our Google setup or Vast.*

- [ ] **1. Executed** — Multi-channel Hive, collector, fast facilitator, transcript/minutes anchoring on `postfiatl1v2`, public archive, explorer view, verifier.
- [ ] **2. Tested by user** — Founder runs a full live 30-min session with prior-session carry-over and verifies third-party audit script.
- [ ] **3. Added to docs** — Governance packet schema, snapshot method, and verification guide published.
- [ ] **4. Public blog post document** — Public blog post published and linked from the session archive.

### Pillar 2 — [Message Protocol Spec](pillars/02-message-protocol.md)
*Canonical protobuf + post-quantum X-Wing encryption + IPFS pinning, Sui-inspired FastPay fee & new-object spam protection, Task Node default FastPay payouts (`"Task reward"` badge on explorer), and x402 (+ MCP helper) agent payments.*

- [ ] **1. Executed** — `.proto` SDKs (JS/Python/Rust), L1 FastPay fees + `account_tx` history, Task Node FastPay payout cutover, Explorer Message panel, x402 facilitator + MCP helper.
- [ ] **2. Tested by user** — Live FastPay reward payout verified on explorer, decrypted locally, legacy v4 history tested, and end-to-end x402 `402 → pay → 200` run.
- [ ] **3. Added to docs** — `docs/specs/message-protocol-v1.md` with worked byte examples published.
- [ ] **4. Public blog post document** — Public blog post explaining Post Fiat's post-quantum message protocol, FastPay, and x402.

### Pillar 3 — [Validator Rewards Program](pillars/03-validator-program.md)
*Same `postfiat-node` binary for validator + RPC, signed releases, `postfiat-node upgrade`, `postfiat-node top` SSH dashboard, one "Run a Validator" docs page, active-liveness enforcement (95% / 14d grace), and `$20/mo` per-operator credit + Finance Hub/API keys (no stake/bond).*

- [ ] **1. Executed** — Signed releases, `upgrade`, `top`, packaged RPC profiles + 2 Foundation public RPCs, signed multi-observer uptime feed, Task Node connect + grace/revocation.
- [ ] **2. Tested by user** — Fresh-host rehearsal following only the docs page, `top` dashboard check, signed upgrade, Task Node connect, and offline grace/revocation test.
- [ ] **3. Added to docs** — Single canonical **Run a Validator** guide (including RPC setup & hardware specs) live on `postfiat.org` and MkDocs.
- [ ] **4. Public blog post document** — Public blog post announcing the Post Fiat L1v2 Validator & RPC Program.

### Pillar 4 — [The NFT System](pillars/04-nft-system.md)
*Signed Pillar 2 metadata (`pf.nft.metadata.v1`), keyed storage lookups, founder inventory + burn-first migration for old Post Fiat chain NFTs (official Task Node issuer with `ISSUER_BURNABLE` off, transferable, `0` fee), new-chain minting, and Explorer NFT page.*

- [ ] **1. Executed** — Metadata schema, L1 keyed NFT lookups, `wallet_wasm` NFT signing, burn-first batch migrator, dual-chain Task Node UI, Explorer NFT view.
- [ ] **2. Tested by user** — Founder reviews inventory, runs burn-first batch migration (verifying zero duplicates on re-run), mints a fresh NFT on `postfiatl1v2`, and inspects Explorer.
- [ ] **3. Added to docs** — `docs/specs/nft-v1.md` (policy, test vectors, migration guide, post-quantum threat model) published.
- [ ] **4. Public blog post document** — Public blog post explaining Post Fiat's quantum-resistant NFT protocol and burn-first migration.

### Pillar 5 — [The Task Node Local Experience](pillars/05-local-task-node.md)
*Native SwiftUI + Rust Mac app (`tasknode-apple`) with local post-quantum `PFSH03` shielded payments (**already proved live on devnet-2 Blocks 1912–1914 on Nov 26, 2026!**), direct public validator RPC default, notarized release, native tab parity, and multi-model adversarial review.*

- [ ] **1. Executed** — Merge PRs `#280`/`#444`/`#39`, unified BIP-39 recovery across clients, direct public validator RPC + leak-capture proof, notarized Mac build, native SwiftUI parity, multi-model review closed.
- [ ] **2. Tested by user** — Clean-Mac install of notarized `TaskNode.app`, seed restore, one-click *Make private* on FastPay payout, Shield → Send → Receive → Unshield directly via public validator RPC.
- [ ] **3. Added to docs** — Mac install/privacy guide, honest privacy model, MacBook Air + M5 benchmarks, Lean scope statement, and pause/exit runbook published.
- [ ] **4. Public blog post document** — Public blog post announcing local post-quantum private payments (`PFSH03`) in the native Post Fiat Mac app.

### Pillar 6 — [Hive Chat Identity on Chain](pillars/06-hive-identity.md)
*Native `identity_set` transaction tying ML-DSA account, Nostr `npub` (keeping existing v1 `npub`s intact + shipping browser signing guard first), `@tasknode.postfiat.org` or custom NIP-05 handle, validator/old-chain/agent links, Explorer Identity card, and first-party `relay.postfiat.org`.*

- [ ] **1. Executed** — Client signing guard, shared v2 seed KDF, `identity_set` consensus + `wallet_wasm`, handle reservations (`goodalexander`, `hive-board@`), Explorer card, `relay.postfiat.org`.
- [ ] **2. Tested by user** — Founder mints `goodalexander@tasknode.postfiat.org` (same `npub` & history), links validator + old-chain key, checks Explorer Identity card, tests `rotate` + SDK lookup.
- [ ] **3. Added to docs** — On-chain identity spec, v1/v2 test vectors, registrar policy, custom NIP-05 guide, and quantum boundary published.
- [ ] **4. Public blog post document** — Public blog post announcing *"Mint Your Task Node Identity"* with native Nostr & Hive integration.

### Pillar 7 — [Optional Deterministic Fees for NAVCoins](pillars/07-navcoin-fees.md)
*100% optional fee schedules with neutral non-security framing (caps: `5%/yr` annual, `50%` HWM gain, `5%` spread, `1%` fix, `7d` min hike notice), atomic unit accrual inside epoch finalization, strict `floor(V × U / S)` NAV check, Explorer view, and open NAVCoin SDK.*

- [ ] **1. Executed** — `NavFeeScheduleV1` consensus ops + 100+ exact-atom Rust/Python vectors, optional Task Node UI + Explorer fee views, packaged NAVCoin SDK.
- [ ] **2. Tested by user** — Founder launches both a zero-fee NAVCoin (zero clutter) and a fee-enabled NAVCoin on devnet via SDK, verifies exact atom recomputation, tests pending hike notice + cancel, inspects Explorer.
- [ ] **3. Added to docs** — **"Create a NAVCoin"** guide (SDK usage, neutral fee formulas, worked example, accounting docs) published on `postfiatorg.github.io`.
- [ ] **4. Public blog post document** — Public blog post announcing the open NAVCoin SDK and optional deterministic on-chain fee schedules.

### Pillar 8 — [On-chain Identity: DID, KYC & Optional Trust Lines](pillars/08-onchain-identity.md)
*Permissionless by default everywhere (no trust lines needed for NAVCoins or DEX offers), fixes the 6-path gating-bypass bug first, adds zero-PII XRPL-style Credentials + `did:pftl` with creator-chosen issuers (Return-only default exit), bridge holds, and W3C VC / ZK-passport adapters.*

- [ ] **1. Executed** — Gating-bypass fix + single `asset_gate(...)` + bridge holds, permissionless DEX/escrow, Issuer Registry + Credentials + `did_resolve`, W3C VC + ZK-passport adapters, Explorer views.
- [ ] **2. Tested by user** — Founder tests both a permissionless NAVCoin (zero `trust_set` on DEX) and a gated NAVCoin end-to-end with sandbox ZK-passport/VC credential, revocation + Return-only exit, and bridge hold claim.
- [ ] **3. Added to docs** — `did:pftl` spec, optional trust-lines guide, credential policy & issuer adapter docs published.
- [ ] **4. Public blog post document** — Public blog post explaining Post Fiat's *"permissionless by default, optional compliance"* identity & credential layer.

### Pillar 9 — [Finance Hub User Spec](pillars/09-finance-hub.md)
*Daily **Today** morning desk (~60 models + Goodbrief + GoodAlexander Library) for active Pillar 3 validators and Top-30 non-validator Task Node users (existing leaderboard, daily snapshot, 7d grace / keep-until rank 35), sandboxed Goodbrief isolation, signed Publish API, and Pillar 10 bridge.*

- [ ] **1. Executed** — Whole-bundle leak closure + Goodbrief sandbox + watermarks, Today view + search/watchlist/visit markers, signed Publish API + Goodbrief split, daily Top-30 snapshot, Pillar 10 briefs.
- [ ] **2. Tested by user** — Founder reviews live Today page on laptop + phone, tests validator + Top-30 + rank-31/grace states, verifies sandbox isolation and bad-signature publish rejection.
- [ ] **3. Added to docs** — Finance Hub access guide (validator & Top-30 rules, perks, Library/API keys) and signed-publishing runbook published.
- [ ] **4. Public blog post document** — Public blog post showcasing Post Fiat Finance Hub for validators and Top-30 contributors.

### Pillar 10 — [Information Hive Mind](pillars/10-information-hive-mind.md)
*Automatic coverage matching for any eligible Expert/verified user across covered crypto protocols and perp-backed liquid megacap equities, two-phase triangulating AI company agents, strict hash-only MNPI refusal firewall, human model-owner approval, and accelerated rewards for verified alpha.*

- [ ] **1. Executed** — Coverage manifest matching, MNPI attestation + hash-only screening + legal sign-off for perp equities, two-phase company agents, claim store + reviewer clustering, model-host bridge, accelerated alpha payouts + ROI dashboard.
- [ ] **2. Tested by user** — Founder tests member + non-member interviews (confirming zero leaked model targets), intentional MNPI refusal (zero text stored), two-user corroboration, `CHANGELOG` citation, and accelerated alpha reward payout.
- [ ] **3. Added to docs** — Contributor interview guide, MNPI/compliance policy, attribution rules, and accelerated alpha reward docs published.
- [ ] **4. Public blog post document** — Public blog post introducing the Post Fiat Information Hive Mind and accelerated alpha rewards.

---

## Near-term NAVCoin product & chain readiness gates

Two high-priority items from the master brief sit alongside the ten pillars and have their own checklists so nothing falls through the cracks:

### Near-term product: Hyperliquid NAVCoin on Solana & Scheduled FX Fix
Every NAVCoin builds on five core parts: (1) Uniswap ERC-20 wrapper + NAV guard (live on B24), (2) on-chain `postfiatl1v2` asset + offer book (built), (3) USDC → pfUSDC Solana/Ethereum bridges (live), (4) Uniswap → actual asset exit routes (built), and (5) **Scheduled London-style FX Fix** (`FxFixPacketV1` exists; scheduled execution still needed) plus **tracking a Hyperliquid NAVCoin on Solana**.

- [ ] **1. Executed** — Wire Hyperliquid reserve attestation (`HyperCoreReserveReader` + non-issuer attestor) to Solana NAVCoin swaps and ship scheduled `FxFixPacketV1` settlement.
- [ ] **2. Tested by user** — End-to-end live dust swap of Hyperliquid NAVCoin on Solana and one live scheduled FX Fix fill.
- [ ] **3. Added to docs** — Operator & user guide for Hyperliquid NAVCoin swaps and scheduled FX Fix windows published.
- [ ] **4. Public blog post document** — Public blog post announcing live Hyperliquid NAVCoin swaps and the scheduled Post Fiat FX Fix.

### Chain readiness & migration gates (Old Chain → `postfiatl1v2`)

- [ ] **Cobalt formal studies:** Publish iterative Lean formal-verification studies and deep-dive blog posts on Cobalt (building on the completed E1–E6 442,368-schedule adversarial campaign).
- [ ] **Cross-model adversarial stress-testing:** Complete multi-model adversarial reviews across NFT migration (Pillar 4), PFSH03 privacy (Pillar 5), `identity_set` (Pillar 6), and NAVCoin fee/gating execution (Pillars 7–8).
- [ ] **Staged migration execution:**
  - [ ] Stage 1 — Outside validators onboard onto `postfiatl1v2` (Pillar 3).
  - [ ] Stage 2 — Task Node switches default payouts (FastPay), messages (Pillar 2), and new NFT mints (Pillar 4) to `postfiatl1v2`.
  - [ ] Stage 3 — Users migrate NFTs (Pillar 4), mint identities (Pillar 6), and execute token/balance migration plan.
  - [ ] Stage 4 — Old chain freezes to read-only archive with a published final ledger hash (closing classical-key NFT claims).

---

## Agentic sequencing & parallel delegation plan

Assuming **heavy parallel agentic workflows and delegated coding agents**, raw coding speed is **not** the bottleneck. The four real bottlenecks that dictate sequencing are:

1. **External & human calendar gates (start on Day 0):** Coding agents move in hours; legal checks, notarization, and outside operators take days to weeks. The real critical path is the **Goodbrief content-rights check** (Pillar 9), **legal counsel sign-off for perp-backed equity interviews** (Pillar 10), **Apple Developer ID notarization** (Pillar 5), **recruiting the outside `G0` validator cohort** (Pillar 3), and **batched Founder UAT sessions** (Gate #2 across all 10 pillars).
2. **L1 "hot-file" consensus contention:** Pillars 2, 4, 5, 6, 7, and 8 all touch the same core Rust files in `postfiatl1v2` (`transactions_mempool_receipts.rs`, `account_owned_asset_types.rs`, `nft_escrow_asset_execution.rs`, `fees_offer_planning.rs`, `rpc_dispatch.rs`, `wallet_wasm/src/lib.rs`). Unleashing 6 agents on those files simultaneously causes constant rebase thrashing and replay-hash breaks. **Rule:** Assign **one L1 Consensus Integrator lane** and batch consensus changes into **just 2 governed live-chain upgrade trains** (Train A and Train B) instead of 6 separate hard forks.
3. **Contract-first unblocking (Phase 0):** Freeze wire contracts and shared test vectors in the first 24–48 hours (`.proto` schemas, `coverage_manifest.json`, `postfiat-validator-uptime-v1`, `NavFeeScheduleV1`, `identity_set` + `asset_gate` JSON/RPC shapes). Once frozen, **8 independent agent lanes execute in parallel** across separate repos/worktrees using shared vectors and mocks.
4. **Batched Founder UAT (3 checkpoints, not 40 interruptions):** Group Gate #2 (*Tested by user*) into 3 crisp demo checkpoints aligned with Ship Wave 1, L1 Train A, and L1 Train B.

```mermaid
flowchart LR
    subgraph D0["Day 0 · Start External & Human Gates Immediately"]
      E2["Goodbrief Rights + Equity Legal Review (P9/P10)"]
      E3["Apple Developer ID Notarization (P5)"]
      E4["Recruit G0 Outside Validators (P3)"]
    end

    subgraph P0["Phase 0 · Contract Freeze (First 24–48h)"]
      C1["P2 .proto + Vectors"]
      C2["P3 Uptime Feed + RPC Spec"]
      C3["P9 Coverage Manifest + Public Brief"]
      C4["P6/P7/P8 L1 Type & RPC Contracts"]
    end

    subgraph W1["Parallel Execution · 8 Independent Agent Lanes"]
      L1A["Lane 1 · L1 Train A<br/>Merge PFSH03 #280 + P2 FastPay fees/history<br/>+ P4 keyed NFTs + P8 R0 gating fix & holds"]
      L1B["Lane 1 · L1 Train B<br/>P6 identity_set + P7 NavFeeScheduleV1<br/>+ P8 Credentials & did:pftl"]
      L2["Lane 2 · Validator Ops & RPC (P3)<br/>upgrade, top, 3 observers, <250ms RPC, 2 public RPCs"]
      L3["Lane 3 · SDKs, x402 & Relay (P2/P6)<br/>JS/Py/Rust SDKs, x402+MCP, relay.postfiat.org"]
      L4["Lane 4 · Task Node Core (P1/P2/P4/P6)<br/>Signing guard, Governance UI, FastPay pool, NFTs"]
      L5["Lane 5 · Finance Hub & Hive Mind (P9/P10)<br/>Isolated viewer, Today, Publish API, Top-30, Agents"]
      L6["Lane 6 · Mac App tasknode-apple (P5)<br/>Merge #39, unified KDF, direct RPC, SwiftUI parity"]
      L7["Lane 7 · Explorer, Docs & Blogs<br/>All 6 Explorer panels + 10 Docs + 10 Blog drafts"]
      L8["Lane 8 · Near-term NAVCoin & Adapters<br/>Hyperliquid Solana NAVCoin, FX Fix, VC/ZK-Passport"]
    end

    subgraph UAT["Batched Founder Testing (Gate #2)"]
      U1["Checkpoint 1 (Fastest Wins)<br/>P3 · P5 Beta · P9"]
      U2["Checkpoint 2 (Post Train A)<br/>P1 · P2 · P4 · HL NAVCoin"]
      U3["Checkpoint 3 (Post Train B)<br/>P6 · P7 · P8 · P10"]
    end

    P0 --> L1A --> L1B
    P0 --> L2 & L3 & L4 & L5 & L6 & L7 & L8
    L2 & L5 & L6 --> U1
    L1A & L3 & L4 & L7 & L8 --> U2
    L1B & L5 & L7 & L8 --> U3
    E2 -.-> L5
    E3 -.-> L6
    E4 -.-> L2
```

### Parallel agent delegation matrix (8 zero-conflict lanes)

Every lane below operates in its own repo or disjoint file boundary so you can fan out agents simultaneously in isolated worktrees without merge-conflict gridlock:

| Agent Lane | Owned Repo / Boundary | Pillars | Can Start Immediately? | Serializes On |
|---|---|---|---|---|
| **1 · L1 Consensus Integrator** | `postfiatl1v2` (`crates/types`, `crates/execution`, `crates/node`, `wallet_wasm`) | P5 merge, P2, P4, P6, P7, P8 | **Yes** (Train A first, then Train B) | Byte-identical replay + 6-validator rollout per train |
| **2 · Validator Ops & Public RPC** | `postfiatl1v2` (`cli`, `systemd/`, observers, read-replica RPC) | **P3** (unblocks P5 direct RPC) | **Yes** (Day 1) | Foundation 2 public RPCs before P5 direct-RPC cutover |
| **3 · Message SDKs, x402 & Relay** | `postfiatl1v2/proto/`, standalone SDKs, x402 facilitator, `relay.postfiat.org` | **P2**, **P6** relay | **Yes** (after Phase 0 `.proto` freeze) | Train A for live FastPay x402 test |
| **4 · Task Node Core & Governance** | `tasknode` (Hive, wallet, payouts, NFTs, identity UI) | **P1**, **P2**, **P4**, **P6** | **Yes** (start with P6 signing guard + P1 multi-channel Hive) | Train A (FastPay/NFTs/P1 anchors), Train B (`identity_set`) |
| **5 · Finance Hub & Hive Mind** | `tasknode` (research routes) + `navstrategies-company-models` | **P9** → **P10** | **Yes** (100% independent of L1 consensus!) | P9 rights review (before Top-30); P10 equity legal sign-off |
| **6 · Native Mac App** | `tasknode-apple` (SwiftUI + Rust `pf_app_core`) | **P5** | **Yes** (merge `#39` + unified KDF + SwiftUI parity) | Lane 2 public RPCs; Day-0 Apple notarization |
| **7 · Explorer, Docs & Blogs** | `postfiatorg.github.io` | **All 10** (Gates #3 & #4) | **Yes** (build UI cards & draft docs/blogs from Phase 0 contracts) | Final screenshots & live tx hashes after UAT Checkpoints 1–3 |
| **8 · Near-term NAVCoin & Adapters** | `crates/ethereum-contracts`, `solana/`, standalone VC/ZK-passport services, Cobalt Lean | **Near-term NAVCoin**, **P8** adapters, Cobalt | **Yes** (Day 1) | Train A/B for live credential + NAVCoin SDK integration |

### Optimal execution sequence (fast wins first, zero wasted waiting)

- **Day 0 (Kick off slow external gates — takes 1 hour):**
  - Kick off Goodbrief content-rights check (Pillar 9) + perp-equity legal sign-off (Pillar 10), verify Apple Developer ID notarization credentials (Pillar 5), and open `G0` validator outreach (Pillar 3).
- **Phase 0 (Contract Freeze — 24–48h):**
  - Merge `postfiatl1v2#280`, `tasknode#444`, and `tasknode-apple#39` (today's live `PFSH03` stack) so `main` is clean.
  - Freeze and merge the tiny schema/contract PRs: Pillar 2 `.proto` files + test vectors, Pillar 3 `postfiat-validator-uptime-v1` schema, Pillar 9 `coverage_manifest.json` + `agent_brief_public.json`, and Pillar 6/7/8 RPC/type specs.
- **Wave 1 → Founder UAT Checkpoint 1 (Zero L1 hard-fork dependency — ships fastest!):**
  - **Lane 5 (Pillar 9):** Ship isolated `hub-viewer.postfiat.org`, `srcdoc` Goodbrief sandbox, Today desk, signed Publish API, and Top-30 daily snapshot.
  - **Lane 2 (Pillar 3):** Ship 2-of-3 signed releases, `upgrade`, `top`, `<250ms` RPC read fix, 3-observer uptime feed, and the first 2 Foundation public RPC + private-admission endpoints.
  - **Lane 6 (Pillar 5):** Ship unified BIP-39 KDF, light-client quorum verification, direct 2-RPC default, and notarized `TaskNode.app` *"Shielded Beta"*.
  - **Lane 3 & 4 (Quick security wins):** Ship `relay.postfiat.org` and the Pillar 6 browser signing guard immediately.
  - *Founder UAT Checkpoint 1:* Test Finance Hub Today desk (laptop + phone), validator `top`/`upgrade`/connect, and notarized Mac `TaskNode.app` direct-RPC beta.
- **Wave 2 → L1 Upgrade Train A → Founder UAT Checkpoint 2:**
  - **Lane 1 (L1 Train A — one coordinated activation + 6-validator rollout):** Pillar 2 FastPay Sui-inspired fees + `order_hash` `account_tx` history + Pillar 4 activation-gated keyed NFT storage & `used_nft_serials` tombstones + Pillar 8 M1 gating-bypass fix & aggregated `(asset, recipient)` bridge holds.
  - **Lanes 3, 4, 7, 8 (in parallel during Train A build, live right after rollout):** Task Node FastPay payout cutover + x402/MCP facilitator (Pillar 2), founder-triggered Hive Governance sessions anchored on `postfiatl1v2` (Pillar 1), founder NFT inventory + burn-first migration (Pillar 4), Hyperliquid NAVCoin on Solana + Scheduled FX Fix, and Explorer cards (Messages, Governance, NFTs).
  - *Founder UAT Checkpoint 2:* Verify live FastPay payout + x402 call, run a live 30-min Hive Governance session with carry-over, run burn-first NFT batch migration, and test Hyperliquid NAVCoin / FX Fix.
- **Wave 3 → L1 Upgrade Train B → Founder UAT Checkpoint 3 (Full Open Ecosystem):**
  - **Lane 1 (L1 Train B — second & final coordinated activation):** Pillar 6 native `identity_set` + 2-of-3 registrar, Pillar 7 `NavFeeScheduleV1` + universal `floor(V × U / S)` check + Ethereum `NAVGuardHook` sync, and Pillar 8 Issuer Registry + delegated `credential_create_and_accept` + `did:pftl` + permissionless DEX/escrow.
  - **Lanes 4, 5, 6, 7, 8 (in parallel):** Identity mint UX (`goodalexander@tasknode.postfiat.org` + `hive-board@`), open NAVCoin SDK, W3C VC & ZK-passport adapters, Information Hive Mind two-phase agents + in-memory MNPI firewall + accelerated alpha rewards (Pillar 10), native Mac SwiftUI parity tabs, and remaining Explorer views + all 10 Docs & Blog Posts (Gates #3 & #4).
  - *Founder UAT Checkpoint 3:* Mint on-chain identity, launch zero-fee + fee-enabled NAVCoins via SDK, run gated ZK-passport NAVCoin + Return-only exit, test Hive Mind crypto & perp-equity interviews + alpha payout, and sign off on raising/lifting the `PFSH03` TVL cap by governance vote.

### Dependency graph (logical order)

```mermaid
flowchart LR
    P2[2 · Message Protocol] --> P1[1 · Hive Governance]
    P2 --> P4[4 · NFTs]
    P2 --> P6[6 · Hive Identity]
    P3[3 · Validators] --> P5[5 · Local Task Node]
    P3 --> P6
    P3 --> P9[9 · Finance Hub]
    P4 --> P6
    P6 --> P8[8 · DID & KYC]
    P8 --> P7[7 · NAVCoin Fees & SDK]
    P9 --> P10[10 · Hive Mind]
    P6 --> P10
```

---

## Serving these pages locally

```bash
python3 big_milestone/serve.py --host 100.89.219.44 --port 8844
```
Pages render live from Markdown on every request (with interactive-styled `- [ ]` milestone checkboxes and Mermaid diagrams).
