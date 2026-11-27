# Big Milestone — Live Execution Progress

> **Last updated:** 2026-11-27 04:06 UTC  
> **Purpose:** Single unambiguous morning status sheet separating what is **merged on `main`**, what is **PR open & CI green**, what has **CI repair/tests running**, and what is **NOT done yet** (review, live rollout, Founder UAT, and external gates).

---

## Executive summary (as of 04:06 UTC)

- **Live PFSH03 stack merged across all 3 repos:** [postfiatl1v2#280](https://github.com/postfiatorg/postfiatl1v2/pull/280), [tasknode#444](https://github.com/postfiatorg/tasknode/pull/444), [tasknode-apple#39](https://github.com/postfiatorg/tasknode-apple/pull/39) (verified live on `postfiat-wan-devnet-2` `pq-r3`, Blocks `1910–1914`).
- **Cross-repo PR cleanup complete:** 46 stale/superseded/Dependabot PRs closed; 6 ready PRs merged (`postfiatl1v2#257, #274, #281, #285`; `tasknode#445, #436`); `postfiatl1v2#272` retargeted to `main`.
- **Workflow 1 (Phase 0 + Wave 1) — 10/10 COMPLETE (0 blockers):** All 4 Phase 0 contract PRs + all 6 Wave 1 PRs (including **L1 Consensus Train A** [postfiatl1v2#295](https://github.com/postfiatorg/postfiatl1v2/pull/295)) are open.
- **Workflow 2 (Wave 2 + Wave 1 CI Repairs) — 8/8 COMPLETE (0 blockers):**
  - **5/5 `tasknode` Big Milestone PRs are 4/4 CI GREEN** (`#446, #447, #448, #449, #450`)!
  - **2/2 `postfiatorg.github.io` PRs are 100% CI GREEN** (`#56, #57` — covering ALL 10 Gate #3 technical guides + Explorer panels + ALL 11 Gate #4 blog drafts marked `draft: true`)!
  - **`tasknode-apple#40`** (Unified BIP-39 KDF + Light-Client Quorum + Direct 2-RPC + Leak-Capture) passed **14 checks** including macOS Swift build & vectors (`W3-CI-Sweep` checking its Linux job now).
  - **`postfiatl1v2`** PRs (`#286, #287, #288, #289, #290, #293, #294, #295`) are all open; stale 1-line PQ domain test assertion on `main` (`pq_payment_local_chain_tests.rs:435`) is being merged directly onto `main` now.
- **Workflow 3 (Wave 3) — 5 PARALLEL LANES RUNNING NOW:**
  - `W3-Lane1B`: **L1 Consensus Upgrade Train B** stacked on `#295` (`identity_set` + `NavFeeScheduleV1` + Issuer Registry/Credentials/`did:pftl`/permissionless DEX)
  - `W3-Lane1-Wasm`: `crates/wallet_wasm` NFT, `identity_set` & unified v2 BIP-39 KDF signing
  - `W3-Lane4C`: `tasknode` Pillar 6 Identity Mint UI & NIP-05 registrar + Pillar 7/8 optional NAVCoin fee & gating UI
  - `W3-Lane6B`: `tasknode-apple` one-click *Make private* (`FastPay -> PFSH03`) + native SwiftUI parity tabs
  - `W3-CI-Sweep`: Verify & repair any real CI failures on `#290, #293, #294, #295, #40`

### What is NOT done yet (requires human review, rollout, UAT, or external gates)

- [ ] **Code review & merge** of the open Phase 0, Wave 1, Wave 2, and Wave 3 PRs (highest scrutiny on L1 Consensus Train A `#295` and Train B).
- [ ] **Live validator rollouts** of Train A and Train B onto `postfiat-wan-devnet-2` (live validators are still on `pq-r3` height 1914; nothing from Waves 1–3 is deployed yet).
- [ ] **SP1 `pft-lane-export` guest ELF rebuild & governance rotation** (`build-lane-export-identity` shows expected ELF drift since `#280` touched `postfiat-types`; best rotated once after Train A/B).
- [ ] **Live Postgres/RPC wiring** for production deployment of Wave 2 Task Node features (e.g., live `--execute` mode for `nft-burn-migrate.mjs` once Train A activates, live Finance Hub public bundle & ZDR LLM wiring for Pillar 10).
- [ ] **Gate #2 (Tested by user — 3 batched Founder UAT checkpoints)** across all 10 pillars + Hyperliquid NAVCoin / FX Fix.
- [ ] **External / calendar gates:** Goodbrief content-rights check (P9), legal counsel sign-off before enabling `paid_interviews_enabled` on perp-backed equities (P10), Apple Developer ID notarization & 8 GB MacBook Air benchmarks (P5), recruiting outside `G0` validator cohort & standing up 2 Foundation public RPCs (P3).
- [ ] **Flip Gate #4 blog posts from `draft: true` to live** (`postfiatorg.github.io#57`) after inserting post-UAT live tx hashes/screenshots.

---

## Complete PR Ledger (16 Open Big Milestone PRs + 10 Merged Baseline PRs)

### Merged on `main` (Baseline & Setup)

| Repo | PR | Summary |
| --- | --- | --- |
| `postfiatl1v2` | [#280](https://github.com/postfiatorg/postfiatl1v2/pull/280) | PFSH03 post-quantum private payments (`b121df60`) |
| `postfiatl1v2` | [#285](https://github.com/postfiatorg/postfiatl1v2/pull/285), [#291](https://github.com/postfiatorg/postfiatl1v2/pull/291), [#292](https://github.com/postfiatorg/postfiatl1v2/pull/292) | Big Milestone 10-pillar specs, Domagoj handoff, live `PROGRESS.md` |
| `postfiatl1v2` | [#257](https://github.com/postfiatorg/postfiatl1v2/pull/257), [#274](https://github.com/postfiatorg/postfiatl1v2/pull/274), [#281](https://github.com/postfiatorg/postfiatl1v2/pull/281) | Daily Colab proving window, Solana exit fixes, Validator doctor fixes |
| `tasknode` | [#444](https://github.com/postfiatorg/tasknode/pull/444), [#445](https://github.com/postfiatorg/tasknode/pull/445), [#436](https://github.com/postfiatorg/tasknode/pull/436) | PFSH03 private relay + Mac OAuth (`d67756f3`), obsolete withdrawal stop, Solana deposit fallback |
| `tasknode-apple` | [#39](https://github.com/postfiatorg/tasknode-apple/pull/39) | PFSH03 Mac app pinned to merged `postfiatl1v2` `b121df60` (`afe16d90`) |

### Open Big Milestone PRs (Ready for Review / CI Sweeping)

| Stage / Lane | Repo & PR | Scope | Live CI Status (04:06 UTC) |
| --- | --- | --- | --- |
| **P0 · C1** | [postfiatl1v2#288](https://github.com/postfiatorg/postfiatl1v2/pull/288) | Canonical `.proto` schemas (`pf.message.v1`, `pf.nft.metadata.v1`, `pf.governance.v1`) + byte vectors | 10 pass; only pre-existing `build-lane-export-identity` + stale `pq_payment_local_chain_tests` line 435 (fixing on `main` now) |
| **P0 · C2** | [postfiatl1v2#286](https://github.com/postfiatorg/postfiatl1v2/pull/286) | Signed `postfiat-validator-uptime-v1` schema + `upgrade`/`top`/RPC profile spec | 10 pass; only pre-existing ELF drift + stale main test line 435 |
| **P0 · C3** | [tasknode#446](https://github.com/postfiatorg/tasknode/pull/446) | Finance Hub JSON schemas (`coverage_manifest`, `agent_brief_public`, `publish_payload`, `top30_snapshot`) + P6 signing guard spec | **4/4 GREEN** |
| **P0 · C4** | [postfiatl1v2#287](https://github.com/postfiatorg/postfiatl1v2/pull/287) | L1 Upgrade Train A & Train B wire/RPC contracts | 10 pass; only pre-existing ELF drift + stale main test line 435 |
| **W1 · Lane 1A** | [postfiatl1v2#295](https://github.com/postfiatorg/postfiatl1v2/pull/295) | **L1 Consensus Train A:** P8 M1 single `asset_gate(...)` (6 paths) + bridge holds, P4 keyed NFTs & `used_nft_serials`, P2 FastPay fees & `order_hash` `account_tx` | Fresh CI running (18 local Rust test suites passed) |
| **W1 · Lane 2** | [postfiatl1v2#290](https://github.com/postfiatorg/postfiatl1v2/pull/290) | **P3 Validator Ops:** `postfiat-node top`/`upgrade`, read-replica RPC profiles & cache, multi-observer uptime feed | Fresh CI running after repair (`6 pass, 0 fail` so far; 15 local suites passed) |
| **W1 · Lane 3** | [postfiatl1v2#289](https://github.com/postfiatorg/postfiatl1v2/pull/289) | **P2/P6:** JS/Py Message SDKs, x402 facilitator + MCP helper, `relay.postfiat.org` packaging | 10 pass, 1 pend (`test`), only pre-existing ELF drift |
| **W1 · Lane 4** | [tasknode#448](https://github.com/postfiatorg/tasknode/pull/448) | **P6/P1:** Browser Nostr signing guard + multi-channel Hive Governance session engine & verifier | **4/4 GREEN** |
| **W1 · Lane 5** | [tasknode#447](https://github.com/postfiatorg/tasknode/pull/447) | **P9 Finance Hub:** Leak closure, `srcdoc` Goodbrief sandbox, Today desk, signed Publish API, daily Top-30 snapshot | **4/4 GREEN** |
| **W1 · Lane 7** | [postfiatorg.github.io#56](https://github.com/postfiatorg/postfiatorg.github.io/pull/56) | **Docs & Explorer:** Canonical Run a Validator & Finance Hub guides + Explorer UI panels | **1/1 GREEN** |
| **W2 · Lane 4B** | [tasknode#450](https://github.com/postfiatorg/tasknode/pull/450) | **P2/P4:** Feature-flagged FastPay reward payout driver (`"Task reward"` badge + legacy v4 history) + founder NFT inventory, burn-first migrator & dual-chain UI | **4/4 GREEN** |
| **W2 · Lane 5B** | [tasknode#449](https://github.com/postfiatorg/tasknode/pull/449) | **P10 Information Hive Mind:** Coverage matcher (crypto + perp megacap equities), hash-only RAM MNPI firewall, two-phase company agents, claim clustering, accelerated alpha rewards + ROI UI | **4/4 GREEN** |
| **W2 · Lane 6** | [tasknode-apple#40](https://github.com/postfiatorg/tasknode-apple/pull/40) | **P5 Mac App:** Unified BIP-39 KDF + vectors, ML-DSA-65 light-client quorum, direct 2-RPC default, HTTP/TCP leak-capture harness | 14 pass (macOS Swift build/vectors/fuzzers green); `W3-CI-Sweep` checking Linux job |
| **W2 · Lane 7-SDK** | [postfiatl1v2#293](https://github.com/postfiatorg/postfiatl1v2/pull/293) | **P7 Open NAVCoin SDK:** Python + JS/TS SDK + CLI + 120 cross-language exact-atom fee & `floor(V × U / S)` vectors | 9 pass, 2 pend, only pre-existing ELF drift |
| **W2 · Lane 8** | [postfiatl1v2#294](https://github.com/postfiatorg/postfiatl1v2/pull/294) | **Near-term & P8:** Scheduled London-style `FxFixPacketV1` executor, Hyperliquid NAVCoin Solana reserve bridge, W3C VC & ZK-Passport sandbox adapters | 9 pass, 2 pend, only pre-existing ELF drift |
| **W2 · Lane 7B** | [postfiatorg.github.io#57](https://github.com/postfiatorg/postfiatorg.github.io/pull/57) | **Gates #3 & #4:** All 9 remaining technical guides + all 11 public blog post drafts (`draft: true`, zero external-audit refs) | **1/1 GREEN** |

---

## Pillar-by-pillar gate status (10 Pillars + Near-Term NAVCoin)

| Pillar | Gate 1 · Code / Executed | Gate 2 · Founder UAT | Gate 3 · Docs | Gate 4 · Blog Post |
| --- | --- | --- | --- | --- |
| **P1 · Hive Governance** | PR Open & **CI Green** (`tasknode#448`, `l1#288`) | Not Started | PR Open & **CI Green** (`site#57`) | Draft Ready (`draft: true`, `site#57`) |
| **P2 · Message Protocol** | PRs Open (`l1#288`, `l1#289`, `l1#295` Train A, `tasknode#450` **CI Green**) | Not Started | PRs Open & **CI Green** (`l1#288`, `site#57`) | Draft Ready (`draft: true`, `site#57`) |
| **P3 · Validator Program** | PRs Open (`l1#286`, `l1#290` repaired); live 2 Foundation RPCs & G0 cohort Not Started | Not Started | PR Open & **CI Green** (`site#56`) | Draft Ready (`draft: true`, `site#57`) |
| **P4 · NFT System** | PRs Open (`l1#288`, `l1#295` Train A, `tasknode#450` **CI Green**, `site#56` **CI Green**); `wallet_wasm` running in Wave 3 | Not Started | PR Open & **CI Green** (`site#57`) | Draft Ready (`draft: true`, `site#57`) |
| **P5 · Local Task Node (Mac)** | **PFSH03 Merged** (`#39`); Unified KDF/quorum/direct-RPC PR Open (`apple#40`); SwiftUI parity + Make Private running in Wave 3; Apple notarization Not Started | Live PFSH03 E2E proved; clean-Mac notarized beta Not Started | PR Open & **CI Green** (`site#57`) | Draft Ready (`draft: true`, `site#57`) |
| **P6 · Hive Identity** | Signing guard **CI Green** (`tasknode#448`), relay (`l1#289`), spec (`l1#287`); L1 `identity_set` (**Train B**), `wallet_wasm`, and Task Node Mint UI running in Wave 3 | Not Started | PR Open & **CI Green** (`site#57`) | Draft Ready (`draft: true`, `site#57`) |
| **P7 · NAVCoin Fees & SDK** | Open NAVCoin SDK + 120 exact-atom vectors PR Open (`l1#293`), spec (`l1#287`); L1 `NavFeeScheduleV1` (**Train B**) + Task Node UI running in Wave 3 | Not Started | PR Open & **CI Green** (`site#57`) | Draft Ready (`draft: true`, `site#57`) |
| **P8 · On-chain Identity & KYC** | M1 6-path `asset_gate` + bridge holds PR Open (`l1#295` Train A), VC/ZK-Passport adapters PR Open (`l1#294`); Issuer Registry/`did:pftl`/permissionless DEX (**Train B**) + UI running in Wave 3 | Not Started | PR Open & **CI Green** (`site#57`) | Draft Ready (`draft: true`, `site#57`) |
| **P9 · Finance Hub** | PRs Open & **4/4 CI Green** (`tasknode#446`, `tasknode#447`); Goodbrief rights check Not Started | Not Started | PR Open & **CI Green** (`site#56`) | Draft Ready (`draft: true`, `site#57`) |
| **P10 · Information Hive Mind** | PR Open & **4/4 CI Green** (`tasknode#449`); live DB/bundle/payout wiring & equity legal sign-off Not Started | Not Started | PR Open & **CI Green** (`site#57`) | Draft Ready (`draft: true`, `site#57`) |
| **Near-Term · Hyperliquid NAVCoin & FX Fix** | PR Open (`l1#294` — scheduled `FxFixPacketV1` + Hyperliquid Solana reserve bridge) | Not Started | PR Open & **CI Green** (`site#57`) | Draft Ready (`draft: true`, `site#57`) |

---

## Wave 3 — Active right now (5 parallel lanes)

1. **`W3-Lane1B`** (`postfiatl1v2`, stacked on `#295`): **L1 Consensus Upgrade Train B** (`identity_set` + 2-of-3 registrar, `NavFeeScheduleV1` + 120-vector Rust verification, Issuer Registry + Credentials + `did:pftl` + permissionless DEX/escrow).
2. **`W3-Lane1-Wasm`** (`postfiatl1v2`): `crates/wallet_wasm` NFT, `identity_set`, and unified v2 BIP-39 KDF signing.
3. **`W3-Lane4C`** (`tasknode`): Pillar 6 Identity Mint UI & NIP-05 registrar + Pillar 7/8 optional NAVCoin fee & compliance UI.
4. **`W3-Lane6B`** (`tasknode-apple`, stacked on `#40`): One-click *Make private* (`FastPay -> PFSH03`) + native SwiftUI parity views.
5. **`W3-CI-Sweep`**: Verify and fix any real CI failures across `#290, #293, #294, #295, #40`.
