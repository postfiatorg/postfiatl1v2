# Big Milestone — Live Execution Progress

> **Last updated:** 2026-11-27 03:02 UTC  
> **Purpose:** Single honest source of truth for what is **merged on `main`**, what has an **open PR (with CI status)**, what is **actively running overnight**, and what is **NOT done yet** so morning review has zero ambiguity.

---

## Summary at a glance

- **Live PFSH03 post-quantum private payments stack:** **MERGED** across all 3 repos (`postfiatl1v2#280`, `tasknode#444`, `tasknode-apple#39`) and verified live on `postfiat-wan-devnet-2` (`pq-r3`, Blocks `1910–1914`).
- **PR cleanup:** **DONE** — 46 stale/superseded/Dependabot PRs closed, 6 clean PRs merged (`postfiatl1v2#257, #274, #281, #285`; `tasknode#445, #436`), `#272` retargeted to `main`, `tasknode-apple` at `0` open PRs.
- **Big Milestone Phase 0 (Contract Freeze):** **4/4 PRs OPEN** (`#286, #287, #288`, `tasknode#446`).
- **Big Milestone Wave 1:** **5/6 PRs OPEN** (`#289, #290`, `tasknode#447, #448`, `postfiatorg.github.io#56`); **Lane 1A (L1 Consensus Train A)** actively wrapping up its Rust tests in `bm/w1-l1-train-a`.
- **Big Milestone Wave 2 + CI Repairs:** **LAUNCHED** (8 parallel lanes running overnight across disjoint worktrees).
- **What is NOT done yet (requires human/founder/external action or Train B):**
  - [ ] Merge the Phase 0 & Wave 1/2 PRs after code review (especially L1 Train A consensus review by Domagoj).
  - [ ] L1 Consensus **Train B** (`identity_set`, `NavFeeScheduleV1` on-chain execution, Issuer Registry + Credentials + `did:pftl` — queued right behind Train A).
  - [ ] Live devnet-2 rollouts of Train A and Train B (nothing post-`pq-r3` is deployed on live validators yet).
  - [ ] SP1 `pft-lane-export` guest ELF rebuild & governance rotation (`build-lane-export-identity` shows expected drift after `#280` added `pq_payment` to `postfiat-types`).
  - [ ] **Gate #2 (Tested by user)** across all 10 pillars (3 batched Founder UAT checkpoints).
  - [ ] External / calendar gates: Goodbrief content-rights check (P9), legal sign-off for perp-backed equity interviews (P10), Apple Developer ID notarization (P5), recruiting outside `G0` validator cohort (P3).
  - [ ] Publishing blog post drafts (`draft: true` -> live) after Founder UAT sign-off.

---

## Merged on `main` (verified)

| Repo | PR | Summary | `main` SHA / Status |
| --- | --- | --- | --- |
| `postfiatl1v2` | [#280](https://github.com/postfiatorg/postfiatl1v2/pull/280) | PFSH03 post-quantum private payments, devnet-2 readiness, in-memory wallet sync, Apple Silicon prover fixes | Merged (`b121df60`) |
| `postfiatl1v2` | [#285](https://github.com/postfiatorg/postfiatl1v2/pull/285) | Big Milestone master spec & 10 locked pillar specs | Merged (`864e8822`) |
| `postfiatl1v2` | [#291](https://github.com/postfiatorg/postfiatl1v2/pull/291) | 2026-11-27 Domagoj (`dravlic`) ramp-up handoff doc & regenerated index | Merged |
| `postfiatl1v2` | [#257](https://github.com/postfiatorg/postfiatl1v2/pull/257), [#274](https://github.com/postfiatorg/postfiatl1v2/pull/274), [#281](https://github.com/postfiatorg/postfiatl1v2/pull/281) | Daily Colab proving window, Solana exit follow-ups, Validator doctor fixes | Merged |
| `tasknode` | [#444](https://github.com/postfiatorg/tasknode/pull/444) | Custody-safe PFSH03 private payment relay + Mac OAuth routes | Merged (`d67756f3`) |
| `tasknode` | [#445](https://github.com/postfiatorg/tasknode/pull/445), [#436](https://github.com/postfiatorg/tasknode/pull/436) | Obsolete withdrawal cancellation, Solana deposit provider fallback | Merged |
| `tasknode-apple` | [#39](https://github.com/postfiatorg/tasknode-apple/pull/39) | PFSH03 post-quantum private payments in signed Mac app (pinned to merged `postfiatl1v2` `b121df60`) | Merged (`afe16d90`) |

---

## Pillar-by-pillar execution tracker

Legend: **Done (Merged)** · **PR Open (CI Green)** · **PR Open (CI Repair Running)** · **In Progress (Agent Running)** · **Not Started**

| Pillar | Gate 1 · Executed | Gate 2 · Founder UAT | Gate 3 · Docs | Gate 4 · Blog Draft | Active PRs / Branches |
| --- | --- | --- | --- | --- | --- |
| **P1 · Hive Governance** | PR Open (CI Green) — multi-channel engine, 30-min sessions, carry-over, verifier; L1 anchoring uses P0-C1 proto | Not Started | In Progress (Wave 2) | In Progress (Wave 2) | [tasknode#448](https://github.com/postfiatorg/tasknode/pull/448), [l1#288](https://github.com/postfiatorg/postfiatl1v2/pull/288) |
| **P2 · Message Protocol** | PRs Open — `.proto` + vectors (`#288`), JS/Py SDKs + x402 + MCP (`#289`); L1 FastPay fees/history in `W1-Lane1A`; Task Node FastPay payout cutover in `W2-Lane4B` | Not Started | PR Open (`#288` spec) + In Progress (`W2-Lane7B`) | In Progress (`W2-Lane7B`) | [l1#288](https://github.com/postfiatorg/postfiatl1v2/pull/288), [l1#289](https://github.com/postfiatorg/postfiatl1v2/pull/289), `bm/w1-l1-train-a`, `bm/w2-p2-p4-fastpay-nft` |
| **P3 · Validator Program** | PRs Open — uptime schema & RPC spec (`#286`), `top`/`upgrade`/read-replica RPC (`#290`, CI fmt repair running); live 2 Foundation RPCs + G0 cohort Not Started | Not Started | PR Open (CI Green) — Run a Validator guide (`site#56`) | In Progress (`W2-Lane7B`) | [l1#286](https://github.com/postfiatorg/postfiatl1v2/pull/286), [l1#290](https://github.com/postfiatorg/postfiatl1v2/pull/290), [site#56](https://github.com/postfiatorg/postfiatorg.github.io/pull/56) |
| **P4 · NFT System** | In Progress — `.proto` (`#288`), L1 keyed NFTs & tombstones (`W1-Lane1A`), Task Node burn-first migrator & UI (`W2-Lane4B`), Explorer NFT panel (`site#56`) | Not Started | In Progress (`W2-Lane7B`) | In Progress (`W2-Lane7B`) | [l1#288](https://github.com/postfiatorg/postfiatl1v2/pull/288), [site#56](https://github.com/postfiatorg/postfiatorg.github.io/pull/56), `bm/w1-l1-train-a`, `bm/w2-p2-p4-fastpay-nft` |
| **P5 · Local Task Node (Mac)** | **PFSH03 base MERGED** (`#280`/`#444`/`#39`); unified BIP-39 KDF + light-client quorum + direct public RPC default running in `W2-Lane6`; Apple notarization Not Started | Live PFSH03 E2E proved on devnet-2; clean-Mac notarized beta UAT Not Started | In Progress (`W2-Lane7B`) | In Progress (`W2-Lane7B`) | Merged `#39`, `bm/w1-p5-mac-direct-rpc` |
| **P6 · Hive Identity** | Partial — Browser Nostr signing guard **PR Open (CI Green)** (`tasknode#448`), `relay.postfiat.org` packaging (`#289`), L1 `identity_set` spec (`#287`); L1 `identity_set` consensus queued in **Train B** | Not Started | In Progress (`W2-Lane7B`) | In Progress (`W2-Lane7B`) | [tasknode#448](https://github.com/postfiatorg/tasknode/pull/448), [l1#287](https://github.com/postfiatorg/postfiatl1v2/pull/287), [l1#289](https://github.com/postfiatorg/postfiatl1v2/pull/289) |
| **P7 · NAVCoin Fees & SDK** | Partial — L1 `NavFeeScheduleV1` wire contract (`#287`), Open NAVCoin SDK + 100+ exact-atom vectors running in `W2-Lane7-NAVCoin-SDK`; L1 consensus ops queued in **Train B** | Not Started | In Progress (`W2-Lane7B`) | In Progress (`W2-Lane7B`) | [l1#287](https://github.com/postfiatorg/postfiatl1v2/pull/287), `bm/w2-p7-navcoin-sdk` |
| **P8 · On-chain Identity & KYC** | In Progress — L1 M1 6-path `asset_gate(...)` fix + `(asset, recipient)` bridge holds in `W1-Lane1A`; W3C VC & ZK-passport sandbox adapters in `W2-Lane8`; Issuer Registry/`did:pftl` queued in **Train B** | Not Started | In Progress (`W2-Lane7B`) | In Progress (`W2-Lane7B`) | [l1#287](https://github.com/postfiatorg/postfiatl1v2/pull/287), `bm/w1-l1-train-a`, `bm/w2-navcoin-fx-adapters` |
| **P9 · Finance Hub** | PRs Open — contracts (`tasknode#446`) + leak closure, `srcdoc` Goodbrief sandbox, Today desk, signed Publish API, daily Top-30 (`tasknode#447`, CI repair running); rights review Not Started | Not Started | PR Open (CI Green) — Finance Hub Access Guide (`site#56`) | In Progress (`W2-Lane7B`) | [tasknode#446](https://github.com/postfiatorg/tasknode/pull/446), [tasknode#447](https://github.com/postfiatorg/tasknode/pull/447), [site#56](https://github.com/postfiatorg/postfiatorg.github.io/pull/56) |
| **P10 · Information Hive Mind** | In Progress — coverage matcher, two-phase company agents, in-memory hash-only MNPI refusal firewall, claim clustering, accelerated alpha payouts running in `W2-Lane5B`; equity legal sign-off Not Started | Not Started | In Progress (`W2-Lane7B`) | In Progress (`W2-Lane7B`) | [tasknode#446](https://github.com/postfiatorg/tasknode/pull/446), `bm/w2-p10-hive-mind` |
| **Near-Term · Hyperliquid NAVCoin & FX Fix** | In Progress — scheduled `FxFixPacketV1` settlement + `HyperCoreReserveReader` non-issuer attestation wiring running in `W2-Lane8` | Not Started | In Progress (`W2-Lane7B`) | In Progress (`W2-Lane7B`) | `bm/w2-navcoin-fx-adapters` |

---

## Active overnight workflows

1. **Workflow 1 (`big_milestone_wave1.py`):** 9/10 agents complete with zero blockers; `W1-Lane1A` (`bm/w1-l1-train-a`, L1 Consensus Train A) is finishing its Rust test suite across 55 files + 2 new integration/regression test files.
2. **Workflow 2 (`big_milestone_overnight_wave2.py`):** 8 parallel lanes running now:
   - `Repair-L1-290` — fix `cargo fmt` + CI on [postfiatl1v2#290](https://github.com/postfiatorg/postfiatl1v2/pull/290)
   - `Repair-TN-446-447` — fix `check` on [tasknode#446](https://github.com/postfiatorg/tasknode/pull/446) and `recovery` on [tasknode#447](https://github.com/postfiatorg/tasknode/pull/447)
   - `W2-Lane4B` — Task Node Pillar 2 FastPay payout cutover + Pillar 4 burn-first NFT migrator & dual-chain UI
   - `W2-Lane5B` — Task Node Pillar 10 Information Hive Mind (coverage matcher, two-phase agents, hash-only MNPI firewall, accelerated alpha rewards)
   - `W2-Lane8` — Hyperliquid NAVCoin reserve wiring, scheduled `FxFixPacketV1`, Pillar 8 W3C VC & ZK-passport sandbox adapters
   - `W2-Lane7-NAVCoin-SDK` — Pillar 7 Open NAVCoin SDK + 100+ exact-atom fee & NAV test vectors
   - `W2-Lane7B` — Remaining Gate #3 technical docs + Gate #4 public blog post drafts (`draft: true`) in `postfiatorg.github.io`
   - `W2-Lane6` — Pillar 5 Mac app (`tasknode-apple`) unified BIP-39 KDF, light-client quorum verification, direct public validator RPC default, leak-capture test harness

*This page is updated automatically after each workflow phase completes and mirrored to `postfiatl1v2` `main`.*
