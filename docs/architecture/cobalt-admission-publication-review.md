# Cobalt and admission publication review

This September 7, 2026 follow-up explains the implemented Cobalt work and the evidence boundary of validator admission. Task Node `task_985c5879a962e3287703b3879b0553a4` governs the delivery under the already locked whitepaper-audit specification. The prior 73-claim inventory remains an audit of its original baseline.

## Source and network identities

- L1 runtime: `d351353e57b295368450a57866ace17b5e1ce6ad`; this follow-up changes no runtime code.
- L1 documentation before this follow-up: `adba4743b5cc9181c122a9496d7024d16be2ac4e`, draft PR 40.
- Website reviewed: `postfiatorg.github.io` at `0a37a4029d4afa03c734e70b3415e734156b1152`; changes are on a separate review branch.
- The website's May whitepaper describes signed-list publication for the XRPL-derived public PFT Ledger. The Rust L1 v2 whitepaper describes a separate controlled-devnet protocol. Cobalt activation on the latter does not transfer publisher authority on the former.

The method separates implemented source, dated deployment observations, research targets and unresolved evidence. Neither the baseline nor this publication review is a fresh fleet probe.

## The corrected explanation

The [whitepaper](../whitepaper.md#53-admission-policy-and-the-evidence-boundary) now separates three obligations:

1. **Establish facts.** Authenticate operator identity, observations and control relationships, and expose disputed or missing evidence.
2. **Evaluate the supplied packet.** `evaluate_validator_admission` checks its configured scores, labels, flags and evidence references. It does not independently perform the first obligation or implement the full economic-exposure/attack-risk formula.
3. **Authorize and apply a change.** `verify_cobalt_validator_trust_update` checks the exact signed decision and current-registry Cobalt authorizations. Consensus v2 orders an update whose execution must accept before registry state changes.

The controlled selector requires reliability at least 9,950 basis points, accountability at least 70, correlation at most zero, no prohibited shared nonempty control-group label, and true manifest/domain/linkedness flags. A supplied false flag or explicit shared-control failure rejects; missing/stale/conflicting evidence creates hold reasons; rejection takes precedence. Different labels do not establish different real controllers. Source-hash and replay-root shape checks do not authenticate facts or independent replay signatures.

The [expanded Cobalt section](../whitepaper.md#6-implemented-cobalt-governance-and-remaining-targets) explains the signed RBC → ABBA → MVBA → DABC certificate, active-registry authorization, payload/root/parent/round/slot bindings, scope exclusivity and Consensus v2 ordering. It cites retained activation at 916, first rotation at 917, final rollback/return at 922/923, and rotation/convergence at 924. The earlier 920/921 trust-binding remediation remains visible. E6 still requires independent operators; the complete target manifest and proof composition remain separate requirements.

## Blog corpus disposition

All **30 blog source files** were screened for Cobalt, validator admission, governance authority, economic exposure and operator-independence claims. The tree has 23 non-draft posts and seven drafts. This was a review of those subjects, not a new audit of every financial, legal, cryptographic or performance claim in the corpus.

Five posts required changes:

| Source under `content/blog/` | Correction |
| --- | --- |
| `cobalt-implementation-evidence.md` | Date the June adoption discussion; add the later scoped activation and admission boundary. Correct graph-alone linkedness and the suggestion that undeclared dependencies are visible. Treat historical entry points as source inventory, not blanket live amendment authority. Preserve the June gates, seven-node topology and manifest. |
| `cobalt-further-evaluation.md` | Describe the final August observation and intervening rollback/return accurately; explain supplied-packet admission separately from signed ratification. Preserve the campaign counts, roots and E6 limitation. |
| `postfiat-canton-xrp.md` | Update the Cobalt scope, economic-screen limits and shared-control example (reject rather than hold); distinguish the two whitepapers. Keep original NAV source and reproduction dates. |
| `community-update-august-2026.md` | Date the later authority clarification, distinguish public validator setup from controlled L1 v2 admission, and identify Consensus v2 as block finality. Preserve the original measurement dates. |
| `postfiat-l1v2-fastpay-latency.md` | Correct Cobalt/account-ordering/checkpoint attribution in prose and the reader diagram. Preserve the original diagram and benchmark figures; the revised role label does not make the June experiment a Consensus v2 benchmark. Keep owned-lane handoff obligations separate from Cobalt authorization. |

The closely related `content/research/deterministic-unl-task-node-cobalt.md` now states the supplied-flag boundary and links the subsequent shadow-only replay. Wallet/funding/vouch graphs support investigation rather than proving separate human control. `content/whitepaper.md` gains a dated reading guide while retaining its May signed-list publication scope.

The remaining 25 blog files required no Cobalt/admission edit in this review:

- Replay and governance judgment already have explicit limits: `llm-governance-replay.md`, `sglang-cross-hardware-replay.md`.
- Financial-index or market-admission terminology concerns a different policy surface: `agentic-indexing.md`, `deterministic-financial-indices.md`, `prediction-market-replayable-oracles.md`, `trustless-single-stock-option-indices.md`.
- Settlement, privacy, reserve and lane evidence uses validator terminology without supplying an independent validator-admission guarantee: `canonical-navcoin-transaction.md`, `heavy-zk-optimization.md`, `navcoin-collateralization.md`, `navcoin-counterparty-risk.md`, `navcoin-ethereum.md`, `navcoin-otc-mvp-proven.md`, `navcoin-proposal.md`, `orchard-halo2-vulnerability-response.md`, `orchard-privacy-research.md`, `origination-replay.md`, `pfusdc-trustless-bridge.md`, `pfusdc.md`, `postfiat-l1v2-private-xrpl-latency-benchmark.md`, `private-fx-executed-pnok.md`, `private-fx-settlement.md`, `proof-of-leverage.md`, `proving-on-apple-silicon.md`, `trustless-ultrashort-tokens.md`.
- `introducing-pf-terminal.md` does not make the touched protocol/admission claim.

## Additional historical artifacts found through the blog

The website's [June Cobalt bundle](https://github.com/postfiatorg/postfiatorg.github.io/tree/0a37a4029d4afa03c734e70b3415e734156b1152/static/benchmarks/cobalt-devnet-evidence-20260609) retains 15 files. Every listed file matched the manifest's byte count and SHA-256 in this review. This is an archive-integrity check, not a rerun or independent attestation of the experiments.

Its cover-sizing report records 12/22 covered subsets for the 35/100-validator examples, with SHA-256 `53318e9a8ec00b61c667fb63d6c54b111f22706f68b41348090e908ea7f42282`. Its extraction report hashes to `1b9e515dc2a79daa94eea6546a96061bd5ee85bba9488aa453547adf9df519c0`. Apple and SGLang machine summaries hash to `0abfcacd31f889d88d86fcaf38a3a1904c95dee52753cb1e7d0686e6a1e82253` and `6886337b9ede4719a1239fd2c38a41743fb7b38cd839fe68844d863f4ec3e10a`.

These add provenance beyond the original L1 checkout, especially for E3/E7. The curated manifest explicitly excludes raw logs and a full source mirror; it does not establish that every original input/output packet is recovered. The prior audit's absence finding is scoped to the pinned L1 tree, not a claim that no public website archive exists. Historical reports cannot establish today's operator independence or replay authority.

## Validation and delivery

The revised canonical paper and download share SHA-256 `8424dab97149a4d038f8bf0512c38b16eb4f92dff1fda39c0037ec94970212c5`. The locked research specification remains byte-identical at its recorded hash. The [retired implementation journal](../plans/completed/cobalt-admission-publication-milestone.md) tracks delivery and Task Node closeout.

- `PYTHONPATH=python python3 -m postfiat_rpc.whitepaper_audit --check`: passed all 73 original claim references, coverage, generated table and synchronized download checks.
- `scripts/test-whitepaper-implementation-boundaries`: passed.
- `scripts/public-doc-links`: passed, 359 Markdown files, using the existing docs virtual environment.
- `scripts/docs-site-build`: passed evidence-index generation, redaction and strict MkDocs. Existing excluded-document INFO notices remain.
- Hugo **0.148.1 extended**, the website CI version: production `--gc --minify` build passed with 110 pages, 34 aliases and 8,034 static files. The existing `_build` deprecation warning belongs to unchanged `content/introducing-post-fiat-terminal.md`.
- `node scripts/validate_market_symbols.mjs`: passed eight public profile cards. `node scripts/check_relay_wallet_guidance.mjs`: passed three files.
- A local HTML reader check passed all seven revised website routes, the L1 whitepaper and review page, local fragment targets, the updated blog-index summary and the revised SVG label. The original benchmark SVG remains byte-for-byte unchanged.
- Chromium inspected the June article on desktop, the August admission explanation on mobile, the whitepaper's admission/Cobalt sections and the revised FastPay diagram. A command-line capture with a numeric fragment was blank; DevTools navigation and `getElementById` scrolling produced the inspected captures. The initial local capture helper's numeric CSS selector was corrected; these were harness issues, not passing image checks. Logs and images stay outside Git.
- `git diff --check`: passed in both worktrees. No Rust/Python runtime, workflow, live validator-list or archived benchmark file changed in this follow-up. The prior audit's focused admission and Cobalt tests use the same unchanged runtime pin; they were not rerun or claimed as fresh tests.

The existing CLI and MkDocs interface supply the L1 reader workflow; Hugo supplies the blog interface. The two review branches preserve the passed specification, original experiment dates and audit inventory. Website merge/deployment and protocol changes are outside this delivery.
