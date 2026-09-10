# QA campaign brief — 2026-09-10

Autonomous reset-burning QA campaign. The operator's ask: heavy code review before the Arc response; burn the available usage resets on it. Work in priority order until the surfaces are exhausted or usage stops progress. Commit and push after every completed unit; never accumulate unpushed work.

## Boundaries (absolute)

- No Task Node action of any kind.
- No fleet mutation: no deploy, restart, configuration change, or write to any validator/RPC host or live chain. The validator-0 diagnosis below is read-only.
- No pushes to StakeHub or any destination other than this repository's origin main.
- No spends, rentals, or external service signups. Network use: git, the documented read-only ledger/status endpoints, and OpenRouter for TIH scoring only.
- Frozen artifacts stay frozen: the V1 attack-simulation outputs, the V2 gate outputs, all deployment evidence directories, whitepaper_legacy.md, and the locked amendment and lock records.
- docs/whitepaper.md may change only under the promotion rule in section D.
- The strict docs build must pass before every commit. Plain single-sentence commit messages.
- When uncertain whether an action crosses a boundary: skip it and log the skip.

## Campaign log

Maintain docs/review/qa-campaign-20260910.md: per-surface status (pending, reviewing, fixing, done), finding counts by severity, fix commits, skipped items with reasons, and scores. Push an update at least once per completed surface. A reader of only this file must understand the campaign's state.

## A. Review surfaces, priority order

Per surface: an adversarial fresh-eyes review (clear or compact context between surfaces so each is seen cold), a findings document docs/review/<surface>-review-20260910.md with severity, file and line, and a concrete failure scenario per finding; then fix that surface's P1 and P2 findings with minimal changes and regression tests (P3 findings are recorded, not fixed); rerun the surface's focused tests; commit findings and fixes separately.

1. Arc-facing code: pfUSDC ingress and egress programs, proof verification, bridge contracts, round-trip tooling, gateway paths.
2. Consensus and storage code merged since 2026-09-05, focusing on the surroundings of the signing fix rather than the already-hardened fix itself.
3. Wallet, proxy, and RPC SDK shipped code.
4. The StakeHub branch fix/pr8-safety-20260907: read-only; the findings document lives in this repository; fix nothing there; push nothing there.
5. python/postfiat_rpc tasknode_unl and v2 modules, fresh eyes. Frozen fixtures stay frozen; goldens regenerate only when a code defect changes correct output, with the reason logged.

## B. Defect inventory

Create and maintain docs/review/defect-inventory-20260910.md classifying every issue from this campaign and from the existing review documents (the 2026-09-06 storage/cobalt/tasknode review, the StakeHub PR 8 review, the proof-input review, the signing-fix qualification blockers) into: reproduced defect (with reproduction), evidence gap, economic assumption, or proposed capability. Each row carries source, severity, and status (fixed, open, or dispositioned). Near campaign end, score this document with the Text Improvement Harness at the established gate (average at least 86) and record the score.

## C. Validator-0 RPC diagnosis (read-only)

Using the documented observation procedure only, determine why validator-0's RPC times out while its host and services run. Read-only commands exclusively: status, logs, sockets. No restarts, no edits. Findings go to the campaign log and to docs/status/chain-state-current.md marked as diagnosis, not repair.

## D. Whitepaper corrections (bounded)

Prepare a candidate correcting exactly two things in the current 87.13 paper: the abstract's three-versus-four question count, and a source-backed update of the outdated consensus signing description citing bbb291ce and the signing contract document. No other edits. Score it with the established gate: three judges, five reviews each, temperature 0, the exact models and scoring prompt recorded in the 2026-09-09 whitepaper handoff. Promote to docs/whitepaper.md and the Markdown download only if the average strictly exceeds 87.13. Otherwise leave docs/whitepaper.md untouched, store the candidate under /home/postfiatchad/pastedocs/, and record the scores in the campaign log.

## Ordering and stop

Order: A1, then D, then the initial B, then C, then A2, A3, A4, A5, then the final B with its TIH gate. If a usage limit pauses work, stop cleanly at a pushed state; the campaign log is the resume point. Never leave the tree dirty between units.
