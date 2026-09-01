# H200 reputation run: v2 published, v3 reasoning rerun in flight

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-01 UTC

## BLUF

The pre-registered H200 reputation-scoring determinism run (v2) is complete
and published: **determinism PASS, 864/864 byte-identical comparisons across
two distinct-owner Vast H200 hosts × two runs, $5.34 spent of the $150 cap**;
its pre-registered semantic criteria failed honestly (no lift over the
deterministic baseline, two lanes lost validity to 1024-token truncations)
and are published as a negative result. Task Node evidence for
`task_c1ed026…` is submitted, `awaiting_verification`. A **v3 reasoning
profile** (fixed prompt, thinking enabled, truncation-proof schema) is frozen,
committed, and **currently executing on rented GPUs** — see "In flight" for
what must be finished or torn down.

## Published v2 record

- Plan `docs/governance/reputation-scoring-h200-run-plan.md` (TIH-gated
  90.33/100, run group `reputation-scoring-h200-run-plan-solpro-candidate`);
  pre-registered at `06a23b6b`, frozen package + erratum at `6b3ce76f`,
  results at `3b1e92c2`.
- Results note: `docs/governance/reputation-scoring-h200-results-20260901.md`.
- Artifact: `benchmarks/ai-governance/reputation-h200-20260901/determinism-artifact.json`
  (aggregate SHA `79a34c1c…`, per-slot hashes, unsealed labels, cost).
- Semantic outcome: model prestige AUC 0.9464 vs baseline 0.9643; zero gross
  inversions; anchor misses 42/88; two `finish_reason: length` truncations
  (`anc-002` C-lane, `anc-028` S-lane) that replay byte-identically.
- v2 results are immutable; nothing about v3 changes them.

## v3 profile (frozen at `bf050466`)

`qwen3.8-27b-fp8-h200-sglang-strict-fixed32-schema-v3-thinking`:
reasoning enabled (`enable_thinking: true`, reasoning-parser qwen3),
identity-verification prompt rules (absence-of-knowledge is fabrication
evidence; name-squat check; obscure-operator floor), `max_tokens` 4096,
schema-bounded `citations` (≤8) and `weights_prior_claims` (≤12, ≤240 chars)
so the v2 repetition spiral is structurally impossible. Same frozen packets,
lookups, and sealed-label commitment as v2. Builder
`build_package_v3.py` → `package_v3/`; runner `run_host_v3.py` additionally
records `reasoning_sha256` per slot. **Not re-scored through TIH** — the plan
header's TIH record covers the pre-rewrite plan text only.

## In flight — needs an operator or agent to finish

Two Vast instances are **live and billing** (account credit 240.34 at
writing; v2+v3 spend so far ≈ $13):

| Role | Instance | Host/machine | SSH | State at writing |
| --- | --- | --- | --- | --- |
| primary | `49466621` | 445596 / 131697 | `ssh2.vast.ai:26620` | server healthy; `primary-run1` executing, ~batch 3/9 at t≈18 min; `primary-run2` queued after it |
| replay | `49467875` | 634408 / 148426 | `ssh6.vast.ai:27874` | still `loading` (image pull); nothing uploaded yet |

To finish (all scripts in `benchmarks/ai-governance/reputation-h200-20260901/`):

1. When replay boots: rsync `package_v3/inputs`, `package_v3/manifest.json`,
   `bootstrap_host.sh`, and `run_host_v3.py` (as `run_host.py`) to
   `/root/rep-run/`, run `./bootstrap_host.sh`, then
   `python3 run_host.py replay-run1 && python3 run_host.py replay-run2`.
2. Pull all four `outputs/*.json` from both hosts into
   `package_v3/outputs/runs/`, plus `host_identity.txt` per host.
3. Compare exactly as v2 (`comparison.json` builder in session history /
   determinism artifact): gate on `content` bytes per slot; report
   `reasoning_sha256` agreement separately.
4. Evaluate AUC vs the same baseline (`baseline.py` outputs are unchanged),
   write a v3 results note, extend `determinism-artifact.json` or add a v3
   artifact, commit, push.
5. **Destroy both instances** (`DELETE /api/v0/instances/{id}/`). If nobody
   finishes the run, destroy them anyway — do not leave them billing.
6. Update the Task Node card only if verification asks; v2 evidence already
   satisfies the accepted task.

Operational notes: SSH keys must be injected via `onstart`
(`echo <pub> >> /root/.ssh/authorized_keys`) — the account/instance key APIs
do not propagate reliably. Host `214845` is blacklisted (two rentals never
accepted SSH). The Vast API key is in the vault under label `vast`.

## References

- [v2 results note](../governance/reputation-scoring-h200-results-20260901.md)
- [Run plan](../governance/reputation-scoring-h200-run-plan.md)
- [Prior session handoff](2026-08-31___dravlic__bug_fixes_and_pending_decisions.md)
- [Pending operator decisions](../governance/pending-operator-decisions.md) — still unanswered
