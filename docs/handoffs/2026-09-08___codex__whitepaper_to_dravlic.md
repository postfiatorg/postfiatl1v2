# Whitepaper update for Dravlic

> Historical handoff. The later [candidate 04 promotion](2026-09-08___codex__whitepaper_candidate_04_promotion.md) supersedes the current-paper selection below. Its score and hash are recorded separately.

- **Operator:** Codex (`codex`)
- **Recipient:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-08 UTC

## BLUF

The selected Fable revision is now the repository's [whitepaper](../whitepaper.md). It scores **86.60** in TIH, improving on the original Fable draft's **85.93**, with all three judge averages higher. The previous repository paper is preserved byte-for-byte as [whitepaper_legacy.md](../whitepaper_legacy.md). This is a documentation replacement requested by the user.

## Current state

- The selected manuscript keeps the Fable draft's three parts: Theory, Business Case, and Implementation Details. It adds the requested freshness-dilution derivation, integer-scale NAV and rounding, disjoint supply perimeter including shielded units, full Kyle derivation, and deletion-monotonic authorization rule. The cash-vault notation is distinct from net assets, and the a651/a652 lineage discussion sits beside the A666 example.
- The paper retains 46 references, 82 source URLs, and 26 implementation-table claims. It distinguishes exact-floor arithmetic from supply authentication and explains the privacy turnstile's aggregate withdrawal ceiling and depositor-loss boundary.
- `docs/whitepaper.md` and `docs/assets/raw/whitepaper.md.txt` contain the exact scored bytes. Navigation exposes the current paper, legacy paper, and this handoff. The existing whitepaper implementation-boundary check recognizes the replacement's statement that shielded batches have no account-level ML-DSA outer envelope; its source assertions remain intact.
- The documentation update starts from `main` at `d351353e57b295368450a57866ace17b5e1ce6ad`. The user authorized committing and pushing the whitepaper replacement, legacy archive, handoff, navigation, Markdown download, and wording-check update together. Pre-existing changes in CI, governance, proof inventory, reserve tooling, and other documents remain separate.
- The paper retains its pinned research sources, including L1 commit `aa8b365f5cf97be05a5222b55de63150cab275c6`. That evidence snapshot is distinct from the checkout's HEAD. Consensus G12, independent validator control, and emergency redemption remain the paper's disclosed assurance obligations. No protocol implementation or fleet operation occurred.

### TIH evidence

| Judge | Original Fable draft | Selected revision | Five selected scores |
| --- | ---: | ---: | --- |
| GPT 6 Pro | 84.60 | 85.80 | 86, 86, 85, 86, 86 |
| Fable 5.1 | 85.00 | 85.40 | 83, 86, 86, 86, 86 |
| GLM 5.3 | 88.20 | 88.60 | 90, 86, 89, 87, 91 |
| **Overall** | **85.93** | **86.60** | **15 accepted reviews** |

The same prompt and settings were used for both drafts: five runs per judge, temperature 0, maximum response length 8,000 tokens, with the arithmetic mean of all 15 scores. Exact OpenRouter model IDs: `openai/gpt-6-astra-pro`, `anthropic/claude-fable-5.1`, and `z-ai/glm-5.3`.

Prompt: `Rate this document on a scale of 1-100. Output the score and your reasoning.`

All selected responses succeeded on their first attempt. Earlier broad edits scored 85.07, 85.13, and 85.33 and were rejected. The selected revision returned to the original Fable prose and made targeted changes. The 85.93 comparison draft is a separate Fable manuscript; that score does **not** describe the repository paper archived as `whitepaper_legacy.md`.

SHA-256 identities:

- Current paper and Markdown download: `547e076fe8d2475e793c1ceb2f2f39e25e218f18062d373a978cd0d1d7ba1d23`.
- Archived repository paper: `83fa0951a27d8278b0fa6435d49931e406941a6860adf13f28d376fea600cc98`.
- Original Fable scoring baseline: `a1cac28a5593c5e4fd7d2901395d48f1e8a9946163bab309933b89bf262add52`.

The complete judge responses and SQLite scoring database remain on the authoring workstation under `/home/postfiatchad/pastedocs/.tih-fable-score-improvement-20260908/` (`reviews-4.json` and `scores.sqlite3`). The human-readable comparison is `/home/postfiatchad/pastedocs/post-fiat-whitepaper-fable-edit-report.md`.

### Validation

Passed `scripts/test-whitepaper-implementation-boundaries`, `scripts/test-public-doc-links`, `scripts/public-doc-links` (358 documents), `scripts/docs-site-build` (redaction check and strict MkDocs build), and `git diff --check`. SHA-256 and `cmp` confirm that the current paper and Markdown download match the scored draft and that the legacy paper preserves the previous repository bytes. Rust, consensus, Orchard, and live-network suites were omitted because protocol behavior is unchanged.

## Next decision or action

Dravlic: review the whitepaper replacement and use the current paper as the reference for subsequent documentation work. Preserve the selected manuscript's bytes: its TIH result belongs to the recorded hash. The legacy paper is historical reference; `docs/whitepaper.md` is the current whitepaper entry point.

## References

- [Current whitepaper](../whitepaper.md)
- [Legacy whitepaper](../whitepaper_legacy.md)
- [Copyable Markdown](../assets/raw/whitepaper.md.txt)
- [Handoff standard](README.md)
