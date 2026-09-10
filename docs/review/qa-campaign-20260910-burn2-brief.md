# QA campaign burn 2 brief — 2026-09-10

Second autonomous burn, following the completed [burn 1](qa-campaign-20260910.md). Mission: adversarially verify burn 1's own repairs, drive the open defect-inventory rows to ground, and expand fuzz and property coverage on the repaired surfaces. Work until the units are exhausted or usage stops progress; commit and push after every completed unit.

## Boundaries

All boundaries of the [burn 1 brief](qa-campaign-20260910-brief.md) apply verbatim: no Task Node action; no fleet mutation of any kind; no pushes anywhere but this repository's origin main; no spends or signups; frozen artifacts stay frozen; docs/whitepaper.md stays untouched (no whitepaper work in this burn); strict docs build before every commit; single-sentence commit messages; when uncertain, skip and log.

## Log

Append a "Burn 2" section to docs/review/qa-campaign-20260910.md and keep it current per unit, same discipline as burn 1.

## Unit 1 — Review the repairs

Fresh-eyes adversarial review of burn 1's repair commits (dbc73fea, f9f13ead, 83488d91, 1c10f828): does each fix fully close its finding; does it introduce any regression; does the same defect class survive elsewhere? Sweep each repaired defect class across the workspace for sibling instances (the signing race class, the refund-path class, the connection-exhaustion class, the evidence-path class). Findings document docs/review/burn2-fix-review-20260910.md; fix P1/P2 with minimal changes and regression tests; P3 recorded only.

## Unit 2 — Ground the open inventory rows

For each row marked open in docs/review/defect-inventory-20260910.md: attempt a bounded reproduction. Outcome per row, recorded in the inventory: reproduced (with the reproduction committed and, if P1/P2 and in-repo, fixed with tests), needs-live-environment (state exactly what environment), needs-operator-decision (state the decision), or not-reproducible (with the evidence of the attempt). No row may remain bare "open" without one of these outcomes or a logged skip reason.

## Unit 3 — Fuzz and property expansion

Extend fuzz targets and property tests on the repaired surfaces within the existing harness conventions: bridge proof parsing, consensus round monotonicity, the RPC accept loop, UNL V2 evidence parsing. Bounded budgets (ten minutes per new target). Commit targets and results; any crash or property violation is a finding handled under the unit 1 rules.

## Unit 4 — Close

Update the inventory counts and statuses; if the inventory changed materially, rerun its Text Improvement Harness gate (average at least 86) and record the score. Final burn-2 summary in the campaign log: findings, fixes, reclassifications, new coverage, remaining risks. Stop cleanly at a pushed state.
