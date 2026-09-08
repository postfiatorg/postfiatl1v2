# Whitepaper candidate 04 promotion

- **Operator:** Codex (`codex`)
- **Recipient:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-08 UTC

## BLUF

The user selected candidate 04 for publication after reviewing its **87.1333/100 TIH score**. The [whitepaper](../whitepaper.md) and [Markdown download](../assets/raw/whitepaper.md.txt) now contain its exact scored bytes. This explicit selection authorizes promotion below the earlier 90-point target; that target was never reached. The [legacy paper](../whitepaper_legacy.md) is preserved byte-for-byte.

## Current state

The revision is 8,144 words, about 19% shorter than the 10,086-word preceding repository paper. It keeps Theory, Business Case, and Implementation Details, including the Claim | Proof | Codebase reference table. It tightens repeated explanations, specifies a prospective issuer and adoption sequence, quantifies issue/redeem spread revenue and break-even volume, adds a measured swap-proving cost, and makes AI-assisted governance explicit in §1.10. The financial derivations and cryptographic and authority boundaries remain.

The source snapshot stays pinned to L1 commit `aa8b365f5cf97be05a5222b55de63150cab275c6` and website commit `f930703e32433a9eb9d74708c2423ebde17832b4`. Promotion begins from repository `main` at `e1493d62`; it changes documentation only. Current source HEAD, dated experimental evidence, and deployed software remain distinct. No protocol implementation or fleet probe occurred in this promotion.

### Scoring evidence

| Judge | Preceding repository paper | Selected candidate | Five selected scores |
| --- | ---: | ---: | --- |
| GPT 6 Pro | 86.20 | 87.80 | 88, 88, 88, 88, 87 |
| Fable 5.1 | 85.00 | 85.60 | 85, 84, 87, 86, 86 |
| GLM 5.3 | 86.40 | 88.00 | 86, 85, 92, 88, 89 |
| **Overall** | **85.8667** | **87.1333** | **15 accepted reviews** |

Exact OpenRouter model IDs: `openai/gpt-6-astra-pro`, `anthropic/claude-fable-5.1`, and `z-ai/glm-5.3`. Both gates used five accepted runs per judge, temperature 0, maximum response length 8,000 tokens, and the arithmetic mean of all fifteen scores. Prompt: `Rate this document on a scale of 1-100. Output the score and your reasoning.` No new scoring call was needed for this exact-byte promotion.

Candidate 04 was the highest completed result among eleven candidates. Its abstract introduces three questions and subsequently refers to four. That known count error remains in the selected bytes; candidate 10 corrected it and scored 86.9333. The user selected candidate 04 after disclosure of both results. Any later textual correction produces a new document hash.

SHA-256 identities:

- Selected paper and download: `28f004a6a0e08a87561e62d14ace6a87659718c4845feba21eb468b3fe314202`.
- Preceding repository paper: `79ca48397b54d6a70080881a95b4bfbf074b61ae00836b05a91078fab318392d`.
- Legacy paper: `83fa0951a27d8278b0fa6435d49931e406941a6860adf13f28d376fea600cc98`.

The fifteen full reviews, manuscript, complete score audit, and SQLite database remain on the authoring workstation under `/home/postfiatchad/pastedocs/.tih-whitepaper-to-90-20260908/`: `candidate-04-reviews.json`, `candidate-04.md`, `final-score-audit.json`, and `scores.sqlite3`. The pending-evidence handoff is `/home/postfiatchad/pastedocs/post-fiat-whitepaper-tih-handoff-20260908.md`. Historical scores in the earlier handoff apply to their recorded hashes.

### Validation

Passed `scripts/test-whitepaper-implementation-boundaries`, `scripts/test-public-doc-links`, `scripts/public-doc-links` (361 documents), `scripts/docs-site-build` (redaction check and strict MkDocs build), and `git diff --check`. Byte comparison and SHA-256 verification confirm that the paper and download match candidate 04 and that the legacy file is unchanged. Rust, consensus, Orchard, and live-network suites are outside this documentation-only change.

## Next decision or action

Dravlic: use the promoted paper as the repository reference and keep its TIH result attached to the recorded hash. The remaining consensus assurance, independent-operator evidence, commercial adoption and full-cost comparison, and holder recovery requirements remain open. This publication closes none of those engineering or commercial obligations.

## References

- [Current whitepaper](../whitepaper.md)
- [Copyable Markdown](../assets/raw/whitepaper.md.txt)
- [Legacy whitepaper](../whitepaper_legacy.md)
- [Earlier whitepaper handoff](2026-09-08___codex__whitepaper_to_dravlic.md)
- [Current chain state](../status/chain-state-current.md)
- [Handoff standard](README.md)
