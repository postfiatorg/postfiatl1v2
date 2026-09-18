# Identity-only profiles: corpus regenerated, H200 replay complete

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-04 UTC
- **Supersedes:** [2026-09-03 identity-packet replay](2026-09-03___dravlic__identity_packet_replay_complete.md)

## BLUF

The operator ruled that nothing but qualitative identity may reach the model. Both prompts
were rewritten to that rule, the 55-validator corpus was regenerated with the identity-only
researcher prompt (55/55 strict PASS, packet-set `8051f392…`), and the pinned model scored
the new profiles on two distinct-owner H200-class hosts, twice each: **192/192
byte-identical**, aggregate `9d6935e2…`. Everything is `SHADOW_ONLY`.

- Corpus: `benchmarks/ai-governance/validator-identity-packets-20260904/`
- Scoring: `benchmarks/ai-governance/institution-reputation-packets-20260904/`
- Results: [identity-only H200 results](../governance/institution-reputation-packets-h200-results-20260904.md)

## What changed and why

The 09-03 run was contaminated: the packet template placed "validator-list publishers
containing the key" in every packet (present in 42 of 55), and the scoring prompt told the
model to ignore uptime and list membership rather than never showing it. Both are gone.

Researcher prompt now: network, key, claimed domain in; identity, official web presence,
incorporation/operations, activities, footprint, business summary, evidence, uncertainty
out. Scoring prompt now: "Assess the organization described in this profile." plus the
profile; recognition gate, three considerations, unchanged bands. Hash binding moved to
the request record. The "individual/pseudonym/hobby → 0" clause I had added on 09-03 was
removed; the recognition rule covers it.

## Results

36 of 55 at zero, mean 14.6, max 72 (Waterloo, UNC Kenan-Flagler, ANU). Versus 09-03:
3 raised, 12 lowered, 5 lost recognition, 0 newly recognized. The losses are stricter
research conclusions, not scorer changes: two domain-less keys previously attributed to
Berkeley Haas and Bitso from registry hints are now "not established" because no primary
source names the key. All 20 PostFiat validators are 0.

Correlation (deterministic, from profile summaries plus input domain): one strong cluster,
the three Post Fiat foundation validators; 106 of 1,485 pairs share a weak signal.

## Execution record

- Corbanu 0.1.37, `gpt-5.6-sol`, `-c model_provider=openai` (the default provider had
  become `claude-plan`; the first launch failed 55/55 on that before the flag was added),
  live search, read-only sandbox, approval never, 6 workers, ~40 minutes, 0 failures.
- Vast rentals, label `inst-rep-packets-20260904`: primary 49800113 (host 214845, H200 NVL,
  Czechia); replay 49802097 (host 445596, H200, Saudi Arabia). Three attempts discarded:
  49800115 (key refused), 49800348 (never left stopped), 49800882 (host 143636 went
  offline mid-run). All destroyed; none remain. `orchestrate_host.sh` now runs the pass
  detached on the host and polls, so an SSH drop cannot kill it.
- No OpenRouter. Frozen predecessors untouched.

## Open items

- Independent human publication review not performed (manifest records it).
- The 09-03 docs are marked superseded rather than deleted.
- Whether pseudonymous/community operators should be assessed under a different rubric is
  an operator decision; this rubric structurally yields 0 for them.
