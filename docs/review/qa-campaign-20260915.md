# QA campaign log — 2026-09-15

This is the progress record for the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). Work began on clean `main` at `d4037d14`. This campaign reviews source and makes no deployment or live activation.

**Status:** A1 findings recorded; repairs pending. A2–A5 and B pending.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Block finality, consensus artifacts, and signing | fixing | 0 | 2 | 1 | [Review](finality-consensus-review-20260915.md); findings commit pending; repair pending |
| A2 | Canonical types and state commitment | pending | — | — | — | — |
| A3 | Node startup, release verification, and RPC serving | pending | — | — | — | — |
| A4 | Live shadow and swap services | pending | — | — | — | — |
| A5 | Node command tools and governance agent | pending | — | — | — | — |
| B | Defect inventory and TIH gate | pending | — | — | — | — |

Current finding totals: **0 P1, 2 P2, 1 P3** (A1 only).

## Block finality, consensus artifacts, and signing review result

The [A1 findings review](finality-consensus-review-20260915.md) records two P2 findings: maximum-view timeout successor overflow and unsigned durable proposal-lock omission before a proposer returns its signature. Both proposed repairs change signing or artifact admission and are conservatively labeled consensus-affecting and source-only. One P3 records independently selected duplicate receipts and block links in the unaudited finality query. Repairs have not started. The reviewed portions and skipped portions of the five A1 source files are itemized in the findings document; the remaining sections were not audited.

## Skips and boundary decisions

No Task Node or fleet action. No other surface or B work started. The account-transaction index and unrelated action, shielded, bridge, pfUSDC/Arc, owned-object, and manifest helpers within the A1 files were not audited; already reviewed legacy ordering and other crates were not re-reviewed.

## Verification

Focused Rust repairs and tests pending. The strict documentation build, public documentation link check, and public secret scan are required before each commit.

## Scores

The B inventory scoring gate has not started.

## Final summary

A1 findings are recorded (0 P1, 2 P2, 1 P3); source-only repairs and CI remain pending. A2–A5 and B have not been reviewed.
