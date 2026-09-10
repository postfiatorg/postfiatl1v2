# Whitepaper and consensus handoff for Dravlic

- **Operator:** Codex (`codex`)
- **Recipient:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-09 UTC

## BLUF

The repository [whitepaper](../whitepaper.md) remains the promoted **87.13 TIH** candidate. The latest GPT 6 Pro language rewrite scored **85.60**, grew from 8,144 to 8,404 words, and stays in `pastedocs`; it failed to improve the score. Separately, the consensus signing defect described in §1.9 was reproduced, repaired, tested, and pushed to `main` in [bbb291ce](https://github.com/postfiatorg/postfiatl1v2/commit/bbb291ce761673fc8df6aee874308dec0cff6da6). This work included no fleet deployment or live probe. The tighter, higher-scoring replacement remains unfinished.

## Current state

### Selected paper and latest rewrite

The current paper is candidate 04, promoted in `cb175ef3b29683d3b49922a338fb55dc0b480b68`. Its Markdown download contains identical bytes. The 87.13 score belongs to this candidate, rather than the original repository paper preserved as [whitepaper_legacy.md](../whitepaper_legacy.md).

| Judge | Selected paper | Latest language rewrite | Latest five scores |
| --- | ---: | ---: | --- |
| GPT 6 Pro | 87.80 | 87.80 | 87, 88, 88, 88, 88 |
| Fable 5.1 | 85.60 | 84.80 | 83, 85, 86, 84, 86 |
| GLM 5.3 | 88.00 | 84.20 | 82, 90, 80, 85, 84 |
| **Overall** | **87.13** | **85.60** | **15 reviews; decrease of 1.53** |

The user's latest editing instruction was a full GPT 6 Pro rewrite to improve arcane or unreadable English **without changing content**, plus correction of the abstract's three-versus-four question count. The direct OpenRouter request used `openai/gpt-6-astra-pro`; the returned model matched. It received the complete 87.13 manuscript and a strict language-only prompt.

The delivered draft is `/home/postfiatchad/pastedocs/post-fiat-whitepaper-clear-english.md`. Comparison preserved all 181 math expressions, all numerals, inline code, citation uses, the entire References section with 70 reference definitions, the Mermaid diagram, all 19 numbered subsections, and all 24 implementation claims. The abstract now says “these three questions”; §1.1 still defines four objects. One original sentence on deletion monotonicity was restored during review because the API wording blurred numerical results and authorization. All other delivered text is the API manuscript.

The published paper still contains the abstract count error. Both manuscripts preserve the old source-pinned consensus description; the language-only brief excluded a substantive status update.

SHA-256 identities:

- Current paper and download: `28f004a6a0e08a87561e62d14ace6a87659718c4845feba21eb468b3fe314202`.
- Latest delivered rewrite: `7fd56b696128888037acf439fb1b1532a7136541ba51f3620b887b9c8f85d9c2`.
- Legacy repository paper: `83fa0951a27d8278b0fa6435d49931e406941a6860adf13f28d376fea600cc98`.

### Consensus repair already landed

Before the repair, the signer checked separate prepare, precommit, and timeout counters. A delayed valid prepare certificate could obtain a first lower-view precommit after the signer had prepared in a newer view. The regression reproduced this through production node signing functions and on-disk safety state.

The repair requires every signing phase to respect the highest persisted round across all three phases. `require_consensus_v2_monotone_round` lives in [consensus_v2.rs](https://github.com/postfiatorg/postfiatl1v2/blob/bbb291ce761673fc8df6aee874308dec0cff6da6/crates/ordering_fast/src/consensus_v2.rs); node authorization persists state under its existing safety guard before emitting a signature. Same-view phase progression remains valid. Signed bytes, state schema, quorum thresholds, historical certificate verification, and legacy finality remain unchanged.

The [signing contract](../architecture/consensus-signing-rounds.md) records the invariant, compatibility boundary, and conditional fixed-height, fixed-committee safety argument. The commit also updates [finality](../architecture/finality.md), adds the supporting [private-exchange models](../business/private-exchange-models.md), and adds documentation navigation. The illustrative market derivations remain in the selected whitepaper.

Validation completed during the repair:

- The delayed-certificate regression failed on the baseline and passed after the fix.
- 20 focused library tests passed, including snapshot recovery, every phase pairing, same-view progression, and 332 bounded delayed-certificate cases for four- and six-member committees.
- Three RPC/transport tests passed, including four- and six-validator failed-proposer recovery.
- Focused all-target Clippy with warnings denied, formatting, documentation links, strict MkDocs build, and whitespace checks passed.

Principal commands already run:

```bash
cargo test -p postfiat-node --lib delayed_precommit_is_rejected_after_preparing_a_newer_view --locked -- --nocapture
cargo test -p postfiat-ordering-fast -p postfiat-node --lib consensus_v2 --locked -- --nocapture
cargo test -p postfiat-node --bin postfiat-node consensus_v2 --locked -- --nocapture
cargo clippy -p postfiat-ordering-fast -p postfiat-node --all-targets --locked -- -D warnings
```

The tests establish the named signing and recovery behavior. They establish neither a live exploit with conflicting executed chains nor arbitrary-network liveness. Full workspace, Orchard-specific, and WAN/fleet suites were outside this focused repair.

Local `main`, remote `main`, and remote `fix/consensus-whitepaper-20260909` were verified at `bbb291ce761673fc8df6aee874308dec0cff6da6` when preparing this handoff. The repair worktree is `/home/postfiatchad/repos/postfiatl1v2-consensus-whitepaper`. Task Node task `task_2fcb55fa06199dd8459491fb15ad5b9b` closed as Rewarded; verification receipt `task_evt_6f9ffd6d-c8ae-4edc-aa6c-529f5dececdf`.

**Fleet boundary:** this session performed no live probe or rollout. The latest deployment entry in [Current State](../status/chain-state-current.md) records `storage-lease-af9b83c3` with writer lease `f0013c29`, binary `383f4325…141a7a`, and all six validators at height 931; Z1 observation began `2026-08-31T04:29:41Z`. Its lower height-924 tables are historical. These are dated records, not a current fleet reading. Deployment of the new signing fix remains unverified.

### Earlier attempts and remaining criticism

The preceding repair-aware revisions all scored below the selected paper:

| Artifact | GPT | Fable | GLM | Overall |
| --- | ---: | ---: | ---: | ---: |
| Candidate 12 | 87.20 | 83.60 | 83.80 | 84.87 |
| Candidate 13 | 87.20 | 80.20 | 84.20 | 83.87 |
| Candidate 14 | 87.20 | 83.80 | 84.60 | 85.20 |
| Candidate 15 | 87.20 | 80.80 | 81.40 | 83.13 |
| Candidate 16 | 86.80 | 80.00 | 84.20 | 83.67 |
| User-supplied `t.md` | 82.80 | 77.80 | 81.40 | 80.67 |

Candidates 12–14 tested cuts, relocation of illustrative derivations, clearer consensus wording, and structural changes. Candidate 15 was a GPT rewrite; candidate 16 corrected an eligibility sentence whose meaning had reversed. That reversed sentence belongs to the rejected candidate, rather than the 87.13 paper. None was promoted. The user-supplied `t.md` is a separate manuscript, SHA-256 `c6dd8baf7a570469524574a50834e4281827c692f91359865980b3536a62b002`.

The latest judges still criticize density, undefined shorthand, competing technical and commercial narratives, dispersed dependencies, and limited customer and comparative-cost evidence. GPT's average stayed flat. Fable fell by 0.8 and GLM by 3.8. GLM again criticized the old signing obligation because the supplied manuscript retained it. TIH judges the supplied text; its reviews do not independently establish current source behavior.

The recurring substantive questions concern authenticated supply denominators, emergency redemption, eligibility across private transfers, independent operator participation, and issuer economics. Distinguish missing evidence or proposed product requirements from reproduced defects. The complete defect inventory requested earlier remains unfinished; this handoff records the established findings and review history.

## Next decision or action

The standing objective is a tighter, professional paper scoring at least 90. The latest attempt missed both the length and score objectives. Preserve the 87.13 baseline and its score provenance. A lower score fails the user's improvement criterion.

For subsequent work, the unresolved items are:

- A qualifying replacement for the selected paper, with the abstract count corrected.
- A source-backed update to the manuscript's old consensus account. This is a content change and belongs in a separately identified revision from the completed language-only edit. The code repair already exists.
- A complete defect inventory that separates reproduced bugs, evidence gaps, economic assumptions, and proposed capabilities.
- Fresh release evidence if anyone claims the signing repair runs on the fleet. Deployment is outside the work completed here.

Preserve the user's intended thesis and three-part structure: Theory, Business Case, and Implementation Details. NAVCoins, the buy-side internet of value, Cobalt, AI-assisted governance/indexing, ML-DSA authorization, and private Asset-Orchard settlement remain central. AI governance remains in §1.10.

For whitepaper prose, retain the user's style rules: active, concise English; no repetition; no standalone “not”; no “load bearing” phrase or “it is not X, it is Y” construction. Keep editorial model names, rewrite history, and TIH references out of public citations. Repair confirmed protocol defects in their code owners and describe the resulting behavior accurately. Preserve material qualifications while avoiding repeated defensive prose.

All API and scoring requests have finished. There are no pending background writer or scoring jobs. This handoff records current work; it starts no rewrite or deployment.

## References

Repository entry points:

- [Selected whitepaper](../whitepaper.md), [Markdown download](../assets/raw/whitepaper.md.txt), and [legacy paper](../whitepaper_legacy.md).
- [Signing invariant and proof](../architecture/consensus-signing-rounds.md).
- [Production signer/storage regression](https://github.com/postfiatorg/postfiatl1v2/blob/bbb291ce761673fc8df6aee874308dec0cff6da6/crates/node/src/consensus_v2_delayed_certificate_tests.rs).
- [Round and quorum regressions](https://github.com/postfiatorg/postfiatl1v2/blob/bbb291ce761673fc8df6aee874308dec0cff6da6/crates/ordering_fast/src/consensus_v2/tests/round_monotonicity.rs).
- [Canonical operational record](../status/chain-state-current.md).

Local evidence lives on the authoring workstation and has been kept outside Git:

| Location under `/home/postfiatchad/` | Contents |
| --- | --- |
| `pastedocs/.whitepaper-language-edit-8713-20260909/` | Exact source packet, `SUPERPROMPT.md`, API runner, raw GPT output and run manifest, delivered-draft validation, all 15 latest reviews in `tih-reviews.json`, score log, and experiment log. |
| `pastedocs/.whitepaper-consensus-resolution-20260909/` | Candidates 12–16, score logs, earlier API packets, investigation, before/after reproduction, focused test logs, and Task Node evidence. The older experiment log stops before the last candidates; use the database for their final scores. |
| `pastedocs/.tih-whitepaper-to-90-20260908/scores.sqlite3` | Original 87.13 reviews and all later score runs. Project: `whitepaper-to-90-20260908`. |
| `pastedocs/.tih-whitepaper-to-90-20260908/user-t-c6dd8baf-reviews.json` | All reviews of the user-supplied `t.md`. |
| `repos/text-improvement-harness-codex-plugin/` | Existing scoring CLI. Preserve its pre-existing working-tree edits. |

Every comparison above used five reviews per model, temperature 0, 8,000 maximum response tokens, and the arithmetic mean of all 15 scores. Exact OpenRouter models are `openai/gpt-6-astra-pro`, `anthropic/claude-fable-5.1`, and `z-ai/glm-5.3`. Preserve those requested models.

Scoring instruction: `Rate this document on a scale of 1-100. Output the score and your reasoning.`

The latest run group is `wp90-clear-english-7fd56b69-20260909`. All 15 requests succeeded on their first attempt. Stored prompt bodies were compared after replacing each manuscript to confirm identical scoring instructions. Reuse the existing gate and compare exact document hashes; an edited file requires its own score.
