# QA campaign: eight repairs landed, burn two running

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-10 UTC

## BLUF

Per your ask, the day went to reset-burning QA ahead of the Arc response. Burn one is complete: every planned surface reviewed fresh-eyes, **eight P1/P2 defects found and repaired the same day** (Arc bridge, consensus, wallet/RPC, UNL V2), validator-0's silent RPC root-caused read-only, and your standing defect-inventory ask delivered — [61 classified rows](../review/defect-inventory-20260910.md) (26 fixed, 16 dispositioned, 19 driven to explicit outcomes), TIH-gated at 89.00. **Burn two is running as this handoff is written**: it adversarially re-reviews burn one's own fixes, sweeps for sibling instances of each repaired bug class, grounds the open inventory rows with reproduction attempts, and extends fuzz coverage. The [campaign log](../review/qa-campaign-20260910.md) is the live record; `/goals` in the Corbanu session shows the run, `/goal pause` stops it cleanly at a pushed state.

## Current state

- Burn one, complete (began 09:32, ran 3h06, ~2.08M tokens): commits `b8560de9`..`9103cf2c` on main. Per-surface findings and repairs are indexed in the campaign log with their test evidence; my own reruns confirmed the UNL selection (171 passed), bridge (38) and proofs (5) suites green after all repairs.
- Highlights you should read: the Arc-facing P1s (V2 export refund path; round-trip execution acknowledgement) found and fixed the day before Arc looks at the code; the consensus authorization/signature-emission race fixed next to your signing fix; validator-0's outage mechanism — the RPC exhausted its 10,000-connection accept budget under wallet readiness traffic, systemd stays green while requests time out; your side's restart last night cleared it but the recurrence condition remained until the wallet/RPC repair commit `83488d91`.
- Whitepaper: the bounded candidate (abstract count + source-backed consensus update only) scored 85.47 against the 87.13 incumbent and was **not promoted**, per your rule; candidate and full scores are under `pastedocs/.qa-campaign-whitepaper-20260910/`. The published abstract still carries the count error — fixing it now costs score; your call whether the rule bends for a factual correction.
- StakeHub `fix/pr8-safety-20260907`: read-only re-review filed one P1 and two P2 findings on the branch itself (docs/review, `07d0b3b4`); the branch remains local and unpushed — its findings need your eyes before PR #8 moves.
- The other lane's uncommitted 2026-09-09 whitepaper+consensus handoff was landed (`10d2f48f`).
- Devnet: not touched beyond read-only diagnosis. Task Node: no action either burn (explicitly fenced). Frozen artifacts, the locked amendment, whitepaper bytes: all verified unchanged by the campaign's boundary audit.
- Burn two boundaries are identical; it stops cleanly at a pushed state if usage pauses it. Everything it produces lands on main with the campaign log as index.

## Next decision or action

1. Read the defect inventory's 19 open rows (burn two is grounding them overnight) — several are needs-operator-decision items.
2. The #37/#39 lineage call remains the blocker for deploying your signing fix (qualification sheet from 09-09 stands).
3. StakeHub PR #8: the new branch findings plus your safety repairs need a merge decision.
4. Whitepaper abstract error vs the score rule — one line from you settles it.

## References

Campaign log and briefs: docs/review/qa-campaign-20260910.md, qa-campaign-20260910-brief.md, qa-campaign-20260910-burn2-brief.md. Inventory: docs/review/defect-inventory-20260910.md. Surface reviews and repair commits as indexed in the log. Validator-0: docs/status/chain-state-current.md#validator-0-rpc-diagnosis-20260910.
