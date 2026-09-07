# GPT-6 Pro whitepaper rewrite review

The [complete rewrite candidate](../whitepaper-gpt6-rewrite.md) was written by `openai/gpt-6-astra-pro` through OpenRouter on September 7, 2026. It received the complete preceding paper and all 15 TIH reviews, including each judge's reasoning, strongest point, weakest point, and requested edit. The instructions required substantive corrections through rules, derivations, and examples; prohibited hedging or apology as substitutes; and excluded deployment history and implementation inventories from the paper.

## Evaluation and disposition

Five independent scores per model used the same generic document-scoring prompt as the preceding paper:

| Judge | Preceding paper | Rewrite candidate | Change |
| --- | ---: | ---: | ---: |
| GPT-6 Pro | 84.2 | 87.2 | +3.0 |
| Claude Fable 5.1 | 75.4 | 77.0 | +1.6 |
| GLM 5.3 | 82.4 | 80.8 | −1.6 |
| Overall | 80.67 | 81.67 | +1.00 |

The combined Fable/GLM average remains 78.9. The candidate therefore does **not** pass TIH's promotion requirement of GPT staying flat or improving and the combined Fable/GLM average improving. It remains a reviewable candidate; the canonical paper and its download retain the preceding revision. These are document-quality judgments, not protocol verification.

One GLM request exhausted three attempts with malformed or truncated JSON. Its replacement used an 8,000-token response allowance instead of 4,000. All 14 successful initial scores were retained; exactly one additional valid score completed the set. The complete individual scores are GPT [88, 87, 87, 87, 87], Fable [79, 79, 76, 74, 77], and GLM [82, 83, 83, 82, 74].

## Substantive changes

| Criticism supplied to the writer | Candidate response |
| --- | --- |
| Cross-view signing assumption substitutes for an argument | §§3.1–3.8 specify durable phase state and a cross-phase view fence, derive certificate safety, and work through delayed certificates and restart. The new fence has the source-conformance gap below. |
| Global block quorum confused with Cobalt trust thresholds | §§4.1–4.3 distinguish block finality, trust ratification, local essential subsets, and active-authority authorization. |
| Cover completeness and transition intersection too broad | §§4.4–4.6 define the bounded support model, account for input traversal, derive the intersection requirement, and include a seven-validator counterexample and key-continuity condition. |
| Economic exposure alone does not establish validator participation or deterrence | §§5.1–5.4 compare validation against passive participation, derive a break-even condition, supply a labeled hypothetical sensitivity example, and separate coalition deterrence from recruitment. |
| Admission confuses supplied evidence with trustworthy operators | §5.5 separates authenticated observations, selector policy, and authority; established mandatory policy failures outrank missing-evidence holds. |
| Turnstile accounting overstates privacy containment | §6 derives per-asset conservation and public pool accounting while distinguishing proof soundness, disclosure, and classical private-action authorization. |
| The classification example needs no model or conflates repeatability with truth | §7 uses conflicting institutional-control evidence, a closed answer schema, independently authenticated replay thresholds, and explicit admission and challenge rules. |
| Post-quantum scope and certificate costs are overstated | §8 separates account/validator signatures from classical shielded cryptography and labels signature-and-identifier arithmetic accurately. |

## Material source-conformance gap

Candidate §3 requires a durable current-view fence across prepare, precommit, and timeout. Advancing to view 1 prevents issuing a new precommit at view 0, even if its prepare certificate arrives late. The safety argument relies on this requirement.

The inspected `apply_consensus_v2_prepare_vote_to_safety` and `apply_consensus_v2_precommit_vote_to_safety` helpers in [consensus_v2.rs](https://github.com/postfiatorg/postfiatl1v2/blob/be5b03c10a20b5276b04f644a5c0c2e8f2074553/crates/ordering_fast/src/consensus_v2.rs) maintain phase-specific high-water marks. The precommit helper checks its own phase mark and lock but does not compare the proposed precommit view with the higher prepare or timeout view. Inspection of [consensus_v2_store.rs](https://github.com/postfiatorg/postfiatl1v2/blob/be5b03c10a20b5276b04f644a5c0c2e8f2074553/crates/node/src/consensus_v2_store.rs), [consensus_v2_finality.rs](https://github.com/postfiatorg/postfiatl1v2/blob/be5b03c10a20b5276b04f644a5c0c2e8f2074553/crates/node/src/consensus_v2_finality.rs), and the transport and block-vote consumers did not establish an equivalent cross-phase fence.

An analytical four-validator schedule shows why the distinction matters: prepare X at view 0; withhold its certificate; open view 1 and prepare Y with an overlapping correct signer; then deliver X's certificate to obtain a lower-view precommit; finally deliver Y's newer certificate to replace that lock. Separate phase counters alone admit those authorizations. The candidate's fence excludes the lower-view precommit after the higher-view prepare.

This is a source-based analytical trace, not an executed Rust regression or a demonstrated network exploit. The rewritten theorem applies to the stated protocol rules; it does not establish that the inspected runtime satisfies them. Runtime behavior was not changed. [G12](whitepaper-gaps.md#g12-cross-phase-consensus-view-discipline) records the required engineering review and regression coverage.

## Remaining criticisms

The new reviews still request a progress argument addressing a high lock omitted from a timeout quorum, a more explicit definition of Cobalt participants relative to the block committee, stronger cryptographic and replay-interface specifications, and formal or empirical validation. Validator recruitment, operating costs, and reproducible inference cannot be established by stronger wording or hypothetical numbers. Requests for measurements and executable proof artifacts remain engineering work. The user's instruction to keep implementation inventories out of the paper remains in force.

Some judges inferred that no implementation or testnet exists because the paper contains no implementation evidence. That inference is not supported by the separate [implementation audit](whitepaper-overview.md). It is also not evidence that the rewritten rules are implemented. The review preserves both distinctions.

## Provenance and checks

The input was the paper at `be5b03c10a20b5276b04f644a5c0c2e8f2074553`, SHA-256 `d5ba3579b3bf3c358efbaa1daeebfdff1bfb4f9e271e8bf9b3383280b99ab949`. The first maximum-reasoning request timed out after 1,500 seconds without returning a manuscript. The successful retry used the same exact model with provider-default reasoning and a 32,000-token output allowance. It returned approximately 7,400 words.

The raw model output is preserved separately with SHA-256 `d5a3b0306827eb3df9f33358d266c2cf44b3c136eab87be3692e4f5824c2a119`. Review corrected rejection precedence, replaced misleading threshold-signature terminology with quorum signatures and identifiers, and clarified that transmitting public keys adds bytes. Math delimiters were normalized for Markdown.

The scored text has SHA-256 `9d9288515faf0a0be345745ba93f9185e8e93a0638dd84b724bbee65c6199e22`. The delivered candidate has SHA-256 `b642c390cc5f5ffa68d6de7844654f33469d3e60ede333f582676e350a91b37b`; the only post-score change repairs 46 standalone math delimiter lines from single to double dollar signs. No prose or equation changed after scoring. Full instructions, raw responses, scoring records, and the analytical trace remain in the local review artifacts. The original 73-claim audit and locked research specification retain their original identities.

Focused validation passed: `git diff --check`, canonical/download byte comparison, the existing whitepaper authorization-boundary check, candidate authorization wording, display-math delimiter and Mermaid-block checks, and local review-link checks. The candidate hash and locked-specification hash were verified. No Rust test, formal model, full documentation build, deployment, or live network exploit was run for this rewrite.
