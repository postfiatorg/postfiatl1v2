# Deterministic Sub-Scorer v3 — Weekly Shadow Rounds 2026

Each row is one counted public testnet scoring round in which the frozen v3 sub-scorer ran in shadow beside the model. Together the rows make up the four-round count in step 6 of the [Z2 advisory-flag execution plan](../plans/active/z2-advisory-flag-execution-plan-20261022.md).
A round is clean when the plan's fixed criteria hold: the internal control passes, there is no ineligible→eligible flip, every eligible→ineligible flip and seat change is explained from the scores, and UNL overlap is at least 24/25.

| Round | Evaluated | Input frozen | Flips out / in | UNL overlap | Seats changed | Consensus | Reliability | Software | Diversity | Identity | Final | Internal control | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 of 4: testnet 29 (VL seq. 27) | 2026-10-28 | 2026-10-27 16:13Z | 0 / 0 | 25/25 | 0 | 0.49 / 1 | 1.15 / 10 | 0 / 0 | 14.46 / 44 | 0.49 / 10 | 0.34 / 2 | reproduces published UNL | clean |

On 25 November 2026 this repository records only round 1 of 4; whether later
rounds ran is not recorded here.

Notes:

- Module: `subscorer_v3.py` with SHA-256 `f71f29ddf00c3825447fc19034340d4ef714b9668345c51e6872586a844c5dc5`. Any module change restarts the count.
- Harness: `benchmarks/ai-governance/dunl-subscorer-shadow-20260901/`. Each round's data is in `results-v3.json` and `results-tables-v3.md`, and its input hashes are in `rounds-manifest.json`.
- Each dimension column gives the mean and maximum absolute v3−model difference. v3 does not score diversity, so that column compares the model's advisory diversity with the pinned diversity formula that the final score uses.
- Round 29 used prompt v12, 25 seats, minimum safe version 1.0.9 and fork clone `e70515d`. All nine artifacts are pinned, and `outputs/final_scores.json` matches the pinned formula chain for all 61 validators. No validator flipped, no seat changed and no final score was within 5 of the cutoff of 40, so nothing needed explaining. Thirteen reliability scores differ; the largest gap is v3 50 against the model's 40. Three identity scores differ, each v3 55 against the model's 45. No final score moved by more than 2.
