# PFSH03 merged, PR cleanup done, Big Milestone specs locked and Wave 1 PRs open

- **Operator:** Antigravity (`antigravity`, handoff to Domagoj Ravlić / `dravlic`)
- **Date:** 2026-11-27 UTC

## BLUF

Three things landed on `main` since the [26 November handoff](2026-11-26___dravlic__merged_changes_read_repairs_open_and_proving_window_checked.md):

1. **Live PFSH03 post-quantum private payments** were rolled to all six `postfiat-wan-devnet-2` validators (`pq-r3`), activated at Block 1910, exercised end-to-end from the signed Mac app (`Shield` 1912 -> `Send` 1913 -> `Receive` 1913 -> `Unshield` 1914), and merged in dependency order across all three repos: [postfiatl1v2#280](https://github.com/postfiatorg/postfiatl1v2/pull/280), [tasknode#444](https://github.com/postfiatorg/tasknode/pull/444), and [tasknode-apple#39](https://github.com/postfiatorg/tasknode-apple/pull/39).
2. **The Big Milestone 10-pillar execution spec** (all founder decisions locked, zero open questions, 8-lane agentic sequencing plan, no external audits in this milestone) was merged into `postfiatl1v2` in [postfiatl1v2#285](https://github.com/postfiatorg/postfiatl1v2/pull/285) (`big_milestone/README.md` + `big_milestone/pillars/01..10`).
3. **Cross-repo PR cleanup** closed 46 stale/superseded/Dependabot PRs and merged 6 clean ready PRs, after which a multi-agent **Phase 0 (Contract Freeze) + Wave 1** workflow opened 9 new PRs across `postfiatl1v2`, `tasknode`, and `postfiatorg.github.io` (with the 10th lane, L1 Consensus Train A, still wrapping up its Rust tests locally).

Read [big_milestone/README.md](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/README.md) first for the master roadmap and dependency graph, then walk the PR review order in **Next decision or action** below.

## Current state

### Live fleet vs. repository `main`

- **Last observed fleet state (2026-11-26 session probe, supersedes the 10:13 UTC `sol-exit-r3` height-1906 read in [Current State](../status/chain-state-current.md)):** All six `postfiat-wan-devnet-2` validators are running `pq-r3` (`/opt/postfiat/releases/pq-r3/postfiat-node`, recorded in `/home/postfiat/.pft/config.toml` on the work server) and agreed 6-of-6 at height **1914** after activating PFSH03 in Block 1910 and finalizing live Shield (1912), Private Send (1913, resolved via a protocol-native 6-of-6 View-1 timeout certificate), and Unshield (1914). [Current State](../status/chain-state-current.md) has not yet been updated with the height-1914 receipt.
- **No fresh fleet probe was run during this 2026-11-27 PR-cleanup and workflow pass.**
- **Repository `main` HEADs (as of 2026-11-27 02:53 UTC):**
  - `postfiatorg/postfiatl1v2`: `b121df60658a7cff541fea7cbba67698aca055a2`
  - `postfiatorg/tasknode`: `d67756f3d292c75bf33f3b820a0399bdb89637de`
  - `postfiatorg/tasknode-apple`: `afe16d90d07300810d70994cf501d8798b79fc7e`
  - `postfiatorg/postfiatorg.github.io`: `8da236f68fb1dbc050fe6463c427f6e0bb5d5d1c`
- **Merged to `main` but undeployed:**
  - `postfiatl1v2`: [#257](https://github.com/postfiatorg/postfiatl1v2/pull/257) (daily Colab proving window; `sol-exit-r3` for B24), [#274](https://github.com/postfiatorg/postfiatl1v2/pull/274) (Solana exit second-check items), [#281](https://github.com/postfiatorg/postfiatl1v2/pull/281) (validator doctor runbook fixes), [#285](https://github.com/postfiatorg/postfiatl1v2/pull/285) (`big_milestone/` specs), and the final CI/hygiene commits of [#280](https://github.com/postfiatorg/postfiatl1v2/pull/280) beyond `pq-r3`.
  - `tasknode`: [#445](https://github.com/postfiatorg/tasknode/pull/445) (stop obsolete waiting withdrawal), [#436](https://github.com/postfiatorg/tasknode/pull/436) (Solana deposit provider fallback), [#444](https://github.com/postfiatorg/tasknode/pull/444) (custody-safe PFSH03 private relay + Mac OAuth routes).
  - `tasknode-apple`: [#39](https://github.com/postfiatorg/tasknode-apple/pull/39) (PFSH03 Mac app pinned to merged `postfiatl1v2` `main` `b121df60`; all core Linux/macOS/UI/fuzz checks green; 0 open PRs remain in `tasknode-apple`).
- **Known pre-existing CI note on `postfiatl1v2` `main`:** `build-lane-export-identity` now reports expected SP1 guest ELF drift (`committed d93540...` vs. `rebuilt 18ce6f...`) because `#280` added `collected_private_fees` / `pq_payment` to `postfiat-types`, which `programs/pft-lane-export` depends on. Do **not** blindly overwrite the committed anchored SP1 ELF/vkey inside feature PRs; handle the lane-export guest bump deliberately as its own release/governance step.

### Big Milestone specs (`big_milestone/`)

Merged on `main` in [#285](https://github.com/postfiatorg/postfiatl1v2/pull/285) and also served locally on the work server via user unit `big-milestone-docs.service` (port `8844`):

- **Master spec & 8-lane agentic sequencing plan:** [`big_milestone/README.md`](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/README.md)
- **Ten locked pillar specs:**
  1. [01 · Hive Chat Governance Module](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/01-hive-governance.md)
  2. [02 · Message Protocol Spec](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/02-message-protocol.md)
  3. [03 · Validator Rewards Program](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/03-validator-program.md)
  4. [04 · The NFT System](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/04-nft-system.md)
  5. [05 · The Task Node Local Experience](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/05-local-task-node.md)
  6. [06 · Hive Chat Identity on Chain](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/06-hive-identity.md)
  7. [07 · Optional Deterministic Fees for NAVCoins](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/07-navcoin-fees.md)
  8. [08 · On-chain Identity: DID, KYC & Optional Trust Lines](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/08-onchain-identity.md)
  9. [09 · Finance Hub User Spec](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/09-finance-hub.md)
  10. [10 · Information Hive Mind](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/pillars/10-information-hive-mind.md)

Every pillar tracks four mandatory ship gates: `1. Executed`, `2. Tested by user`, `3. Added to docs`, `4. Public blog post document`. Consensus changes are batched into two coordinated live-chain upgrade trains (**Train A** and **Train B**) instead of per-pillar hard forks. External audits are explicitly out of scope for this milestone.

### Phase 0 & Wave 1 Big Milestone PRs (opened 2026-11-27)

All branches were built in isolated worktrees under `/home/postfiat/repos/_bm_worktrees/` off today's clean `main`. CI snapshots below are as of `2026-11-27 02:53 UTC`:

| Stage / Lane | Repo & PR | Branch | Scope | CI Status (02:53 UTC) |
| --- | --- | --- | --- | --- |
| **Phase 0 · C1** | [postfiatl1v2#288](https://github.com/postfiatorg/postfiatl1v2/pull/288) | `bm/p0-p2-message-proto` | Canonical `.proto` schemas (`pf.message.v1`, `pf.nft.metadata.v1`, `pf.governance.v1`) + byte vectors | 8 pass, 2 pend (`test`, `open-reserve-proof-kit`), 1 fail (pre-existing `build-lane-export-identity`) |
| **Phase 0 · C2** | [postfiatl1v2#286](https://github.com/postfiatorg/postfiatl1v2/pull/286) | `bm/p0-p3-validator-spec` | Signed `postfiat-validator-uptime-v1` schema + `upgrade`/`top`/RPC profile spec | 9 pass, 1 pend (`test`), 1 fail (pre-existing `build-lane-export-identity`) |
| **Phase 0 · C3** | [tasknode#446](https://github.com/postfiatorg/tasknode/pull/446) | `bm/p0-p9-hub-contracts` | Finance Hub JSON schemas (`coverage_manifest`, `agent_brief_public`, `publish_payload`, `top30_snapshot`) + Pillar 6 signing guard spec | 3 pass, 1 fail (`check` — inspect before merge) |
| **Phase 0 · C4** | [postfiatl1v2#287](https://github.com/postfiatorg/postfiatl1v2/pull/287) | `bm/p0-l1-train-contracts` | L1 Upgrade Train A & Train B wire/RPC contracts | 9 pass, 1 pend (`test`), 1 fail (pre-existing `build-lane-export-identity`) |
| **Wave 1 · Lane 2 (P3)** | [postfiatl1v2#290](https://github.com/postfiatorg/postfiatl1v2/pull/290) | `bm/w1-p3-validator-ops` | `postfiat-node top`/`upgrade`, read-replica RPC profiles & read-path cache, multi-observer uptime feed | 7 pass, 4 fail (`build-lane-export-identity`, `check`, `test`, `open-reserve-proof-kit` — needs CI repair) |
| **Wave 1 · Lane 3 (P2/P6)** | [postfiatl1v2#289](https://github.com/postfiatorg/postfiatl1v2/pull/289) | `bm/w1-p2-sdk-x402` | JS/Python message SDKs, x402 facilitator + MCP helper, `relay.postfiat.org` packaging | 8 pass, 2 pend (`test`, `open-reserve-proof-kit`), 1 fail (pre-existing `build-lane-export-identity`) |
| **Wave 1 · Lane 4 (P6/P1)** | [tasknode#448](https://github.com/postfiatorg/tasknode/pull/448) | `bm/w1-p6-guard-p1-hive` | Pillar 6 browser Nostr signing guard + Pillar 1 multi-channel Hive governance engine & verifier | **4/4 green** |
| **Wave 1 · Lane 5 (P9)** | [tasknode#447](https://github.com/postfiatorg/tasknode/pull/447) | `bm/w1-p9-finance-hub` | Finance Hub leak closure, `srcdoc` Goodbrief sandbox, Today desk, signed Publish API, daily Top-30 snapshot | 3 pass, 1 fail (`recovery` — inspect before merge) |
| **Wave 1 · Lane 7 (Site)** | [postfiatorg.github.io#56](https://github.com/postfiatorg/postfiatorg.github.io/pull/56) | `bm/w1-docs-explorer` | Canonical **Run a Validator** & **Finance Hub Access** guides + Explorer protocol panels | **1/1 green** |
| **Wave 1 · Lane 1A (L1 Train A)** | *In progress* (no PR yet) | `bm/w1-l1-train-a` (`/home/postfiat/repos/_bm_worktrees/w1-l1-train-a`) | Pillar 8 M1 single `asset_gate(...)` + `(asset, recipient)` bridge holds; Pillar 4 keyed NFTs & tombstones; Pillar 2 FastPay fees & `order_hash` `account_tx` history | Active agent fixing final Rust test failures across 55 files + 2 new test suites (`train_a_regression_tests.rs`, `train_a_integration_tests.rs`) |

### Pre-existing ops / review PRs intentionally kept open

None of these were closed or altered (except retargeting `postfiatl1v2#272` to `main` after `#257` merged):

- **`postfiatl1v2`:** [#273](https://github.com/postfiatorg/postfiatl1v2/pull/273) (ETH withdrawals closed outside window), [#272](https://github.com/postfiatorg/postfiatl1v2/pull/272) (daily proving-window alerts, retargeted to `main`), [#269](https://github.com/postfiatorg/postfiatl1v2/pull/269) (validator-host sandbox drop-ins), [#262](https://github.com/postfiatorg/postfiatl1v2/pull/262) (`rpc-events-report`), [#260](https://github.com/postfiatorg/postfiatl1v2/pull/260) (logrotate overlap check).
- **`tasknode`:** [#430](https://github.com/postfiatorg/tasknode/pull/430) (Solana deposit nonce assertions — hit `bridge.js` bundle budget `111,199 B` on rebase, left open for review), [#413](https://github.com/postfiatorg/tasknode/pull/413) (NAVCoin create/withdraw recovery), [#410](https://github.com/postfiatorg/tasknode/pull/410) (public explorer API), [#372](https://github.com/postfiatorg/tasknode/pull/372) (agent task sizing docs).
- **`postfiatorg.github.io`:** [#41](https://github.com/postfiatorg/postfiatorg.github.io/pull/41) (agent token expiry docs), [#29](https://github.com/postfiatorg/postfiatorg.github.io/pull/29) (Schwab NAVCoin blog).

### Closed PRs (cleanup summary)

46 stale, superseded, or noisy Dependabot PRs were closed ahead of Wave 1:
- `postfiatl1v2` (6): `#271, #232, #210, #207, #206, #216`
- `tasknode-apple` (2): `#38, #36` (Orchard retired, superseded by `#39`)
- `postfiatorg.github.io` (4): `#3, #18, #26, #27`
- `tasknode` (34): Sept/Oct stale PRs and outdated Dependabot bumps (`#218`–`#385`)

## Next decision or action

Recommended ramp-up and review order for Domagoj:

1. **Orient (10 min):** Read [`big_milestone/README.md`](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/README.md) (especially the **Agentic sequencing & parallel delegation plan** and **Train A vs. Train B** split) and skim the specific pillar specs tied to any PR you review.
2. **Audit & merge the Phase 0 contract PRs first (lowest risk, unlocks everything cleanly):**
   - [postfiatl1v2#286](https://github.com/postfiatorg/postfiatl1v2/pull/286) (C2 · Pillar 3 uptime/RPC), [postfiatl1v2#287](https://github.com/postfiatorg/postfiatl1v2/pull/287) (C4 · L1 Train A/B contracts), [postfiatl1v2#288](https://github.com/postfiatorg/postfiatl1v2/pull/288) (C1 · `.proto` + vectors).
   - Check the single `check` failure on [tasknode#446](https://github.com/postfiatorg/tasknode/pull/446) (C3 · Finance Hub + signing guard contracts) and merge once green.
3. **Review the already-green Wave 1 PRs:**
   - [tasknode#448](https://github.com/postfiatorg/tasknode/pull/448) (Pillar 6 browser Nostr signing guard + Pillar 1 Hive governance engine — 4/4 CI green).
   - [postfiatorg.github.io#56](https://github.com/postfiatorg/postfiatorg.github.io/pull/56) (Run a Validator & Finance Hub docs + Explorer panels — 1/1 CI green).
   - [postfiatl1v2#289](https://github.com/postfiatorg/postfiatl1v2/pull/289) (Message SDKs, x402/MCP facilitator, `relay.postfiat.org` — clean aside from pre-existing `build-lane-export-identity`).
4. **Triage the two Wave 1 PRs with CI failures:**
   - [tasknode#447](https://github.com/postfiatorg/tasknode/pull/447) (Pillar 9 Finance Hub isolation/Today/Publish API/Top-30 — inspect the single `recovery` failure).
   - [postfiatl1v2#290](https://github.com/postfiatorg/postfiatl1v2/pull/290) (Pillar 3 `top`/`upgrade`/read-replica RPC — fix `check`, `test`, and `open-reserve-proof-kit` lockfile/clippy failures).
5. **High-scrutiny consensus review (highest priority once opened):**
   - **Wave 1 Lane 1A (`bm/w1-l1-train-a`)** — L1 Consensus Train A. Pay special attention to: (a) byte-identical replay of historical blocks prior to activation height, (b) Pillar 8 M1 single `asset_gate(...)` coverage across all 6 asset-movement paths and `(asset, recipient)` bridge holds, (c) Pillar 4 `used_nft_serials` tombstones, and (d) Pillar 2 FastPay fee/spam bounds. Do **not** roll Train A to live `postfiat-wan-devnet-2` until replay verification and a fresh fleet status capture are recorded in [Current State](../status/chain-state-current.md).
6. **Decide on the SP1 lane-export ELF bump:**
   - Because `#280` changed `postfiat-types`, `build-lane-export-identity` is currently red on every `postfiatl1v2` PR. Decide whether to rebuild & govern-rotate the `pft-lane-export` guest ELF after Train A lands (so you only rotate once instead of twice).
7. **Keep ops safety intact:**
   - Existing ops handoff constraints from [26 November](2026-11-26___dravlic__merged_changes_read_repairs_open_and_proving_window_checked.md) still apply until deployed/verified on host (`#272` and `#273` are not installed; `#277` is on `main`).

## References

- **Master Big Milestone Spec & Sequencing:** [`big_milestone/README.md`](https://github.com/postfiatorg/postfiatl1v2/blob/main/big_milestone/README.md) (served locally on the work server via `big-milestone-docs.service`, port `8844`)
- **Operational truth & prior handoff:** [Network Status](../status/network-status.md), [Current State](../status/chain-state-current.md), [26 November Handoff](2026-11-26___dravlic__merged_changes_read_repairs_open_and_proving_window_checked.md)
- **Merged PFSH03 stack:** [postfiatl1v2#280](https://github.com/postfiatorg/postfiatl1v2/pull/280), [tasknode#444](https://github.com/postfiatorg/tasknode/pull/444), [tasknode-apple#39](https://github.com/postfiatorg/tasknode-apple/pull/39)
- **Merged specs & cleanup PRs:** [postfiatl1v2#285](https://github.com/postfiatorg/postfiatl1v2/pull/285), [#281](https://github.com/postfiatorg/postfiatl1v2/pull/281), [#274](https://github.com/postfiatorg/postfiatl1v2/pull/274), [#257](https://github.com/postfiatorg/postfiatl1v2/pull/257); [tasknode#445](https://github.com/postfiatorg/tasknode/pull/445), [#436](https://github.com/postfiatorg/tasknode/pull/436)
- **Open Phase 0 PRs:** [postfiatl1v2#286](https://github.com/postfiatorg/postfiatl1v2/pull/286), [#287](https://github.com/postfiatorg/postfiatl1v2/pull/287), [#288](https://github.com/postfiatorg/postfiatl1v2/pull/288); [tasknode#446](https://github.com/postfiatorg/tasknode/pull/446)
- **Open Wave 1 PRs:** [postfiatl1v2#289](https://github.com/postfiatorg/postfiatl1v2/pull/289), [#290](https://github.com/postfiatorg/postfiatl1v2/pull/290); [tasknode#447](https://github.com/postfiatorg/tasknode/pull/447), [#448](https://github.com/postfiatorg/tasknode/pull/448); [postfiatorg.github.io#56](https://github.com/postfiatorg/postfiatorg.github.io/pull/56)
- **Local worktrees & orchestration script (work server):** `/home/postfiat/repos/_bm_worktrees/` (including active `w1-l1-train-a`)
