# Identity-packet H200 replay complete; packet-derived validator correlation published

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-03 UTC
- **Resumes:** [Identity replay paused for Arc grant](2026-09-02___dravlic__identity_replay_paused_for_arc_grant.md), section 4

## BLUF

The paused step ran. The pinned `Qwen/Qwen3.8-27B-FP8` scored all 55 frozen validators
from the exact bytes of their identity packets, with per-packet and corpus hashes bound
into every request, on two distinct-owner H200-class hosts, twice each:
**192/192 byte-identical**, aggregate `3458f72d…`, comparison `4800d68d…`. A
deterministic validator correlation was derived from the same packet bytes with no model
call. Everything is `SHADOW_ONLY`; nothing touches consensus.

Package: `benchmarks/ai-governance/institution-reputation-packets-20260903/`
([README](../../benchmarks/ai-governance/institution-reputation-packets-20260903/README.md)).
Results: [identity-packet H200 results](../governance/institution-reputation-packets-h200-results-20260903.md).

## What changed versus the name-only run

| | name-only (2026-09-01) | packets (2026-09-03) |
| --- | --- | --- |
| zero scores | 45 / 55 | 31 / 55 |
| mean score | 10.8 | 18.9 |
| newly recognized | | 14 |
| lowered (brand → actual legal entity) | | 9 |
| lost recognition | | 0 |
| PostFiat validators at 0 | 20 | 20 |

Highest: Berkeley Haas 78, Ripple Labs 72, Australian National University 72, Waterloo 62,
UNC Kenan-Flagler 62, Bitso 57, Blockdaemon 57. Post Fiat itself scores 0 with an
explanation that cites the packet's own "micro-tier, no registered legal name" finding.

## Correlation

`correlate_packets.py` → `outputs/correlation.json` / `correlation.md`. One strong
cluster: the three Post Fiat foundation validators (same entity, domain, X handle).
227 of 1,485 pairs share a weak signal; strongest non-cluster pair Ripple Labs /
Blockdaemon 0.38. Shared-hosting suffixes and platform hosts are excluded and listed.
Byte-stable across reruns.

## Execution record

- Frozen corpus re-verified first: `finalize.py` PASS 55/55, zero diff, packet-set
  `b198e232…`.
- Vast rentals, label `inst-rep-packets-20260903`: primary 49794903 (host 214845,
  H200 NVL, Czechia), replay 49795370 (host 445596, H200, Saudi Arabia). Instance
  49794901 (host 612739, Japan) never accepted the account SSH key and was destroyed
  unused. All destroyed; credit 69.58 → 66.24 USD. Receipt in
  `outputs/rental-teardown.json`.
- No OpenRouter, no live search, no Corbanu rerun, no JSONL logs as input.
- The three frozen predecessors were not modified.

## Boundaries and open items

- Independent human publication review (runbook requirement) has **not** been
  performed; recorded in `manifest.json`. Required before any public production use.
- Scores and correlation are external `SHADOW_ONLY` evidence. Connecting them to
  validator weight needs a separate operator decision and governed activation path.
- The prompt changed from the predecessor (packet-handling rules added; bands
  identical). Cross-revision score deltas therefore reflect both the richer input and
  the prompt change; `outputs/delta-vs-unl-20260901.json` records them per validator.
- No PFTL validator was queried or mutated. No Task Node action.

## Suggested next steps

1. Human review of a sample of packets and explanations before any publication.
2. Decide whether PostFiat community operators should be scored on a different rubric
   (the institution rubric structurally yields 0 for pseudonymous individuals).
3. If a correlation with model judgment is wanted, run a second packet-bound pass that
   asks the model for same-controller likelihood per strong/weak pair; keep it in a new
   dated package.
