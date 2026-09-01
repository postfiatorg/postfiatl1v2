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

## RESOLVED — v3 and v4 completed after this handoff was first written

The in-flight work below was finished the same session: v3 completed
(determinism PASS, validity FAIL to reasoning truncation) and a v4 profile
(8192-token cap) was frozen and run — **determinism PASS 864/864 and the
first measured lift** (prestige AUC 0.9732 > baseline 0.9643, all four
fabrications in the 0–5 bands). Residuals: 16/270 truncations, obscure-real
operators misbanded. See the
[results note](../governance/reputation-scoring-h200-results-20260901.md)
v3/v4 section and `determinism-artifact-v4.json` (commit `a1699ac2`). All
instances destroyed; cumulative spend $77.32 of $150. The original in-flight
section is retained below for the record.

## In flight — needs an operator or agent to finish (historical)

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

## End of session (Domagoj, 2026-09-01)

The sections above were written by the operator's overnight agent session
under this operator name; this section is the human-directed day session.

### Delivered today

- **Vast housekeeping** — idle A100 `49076806`
  (`arc-grant-demo-archive-prover`) STOPPED at 07:20Z after ~78 h idle
  (≈$108 burned); disk kept, billing now storage-only; destroy or restart
  is the operator's call.
- **Testnet-path status CLI + generated page** — `69667346`, `fdc679c5`;
  `python -m postfiat_rpc.testnet_path` →
  [docs/status/testnet-path.md](../status/testnet-path.md).
- **D1 release-gate inventory** — `36a7848c`;
  [docs/status/release-gate-inventory.md](../status/release-gate-inventory.md)
  — 21 gates, 4 DONE / 16 OPEN / 1 UNKNOWN.
- **C2 deterministic sub-scorer shadow evaluation** — v1 `a1161ff9`,
  `0d152b09`, `e5f46fa1`, `ce93fc45`; v2 `63286b35`, `4dbcdd78`;
  [note](../governance/dunl-subscorer-shadow-eval-20260901.md). v2
  headline: one cutoff flip across rounds 12–19, UNL overlap 19–20/20,
  internal control reproduced all eight published UNLs.
- **D2 public operator runbook** — `fcbf99c1`;
  [docs/runbooks/public-operator-runbook.md](../runbooks/public-operator-runbook.md),
  seven journeys, seven boxed gap notes.
- **C3 genesis-registry proposal path design** — `9dba8682`;
  [docs/architecture/genesis-registry-proposal-path.md](../architecture/genesis-registry-proposal-path.md),
  TIH 88.93.
- **Pending-decisions sheet** — `3a2256d5` adds the model-authority
  (Gate Zero Z2) row citing both evidence sets neutrally
  ([sheet](../governance/pending-operator-decisions.md)).
- **Evidence-tooling hardening** — `faebbe16`, `d9378c79`, `97f7c41e` —
  five confirmed silent-wrong-answer defects fixed with
  executed-failing-case proof; recomputed evidence byte-identical; guard
  tests now run under every interpreter (38 tests, 0 skipped).
- **Genesis-registry work-sequence steps 1–2** — `fa7e67ff`, `83546dcb`,
  `6290175f` — canonical Rust types + 8 golden vectors + 25 mutation
  fixtures; independent Python builder/verifier; two-implementation hash
  agreement on all eight rounds.
- **D3 launch topology thresholds proposal** — `02f9c17b`;
  [docs/architecture/launch-topology-thresholds.md](../architecture/launch-topology-thresholds.md)
  — all five dimensions with concrete numbers, explicitly awaiting the
  operator's confirmation.
- **Genesis-registry work-sequence step 3** — `9135649c` — Cobalt checker
  integration tests
  (`crates/consensus_cobalt/tests/genesis_registry_checker.rs`): 8 golden
  pairs accepted, full n_S = 12–20 range, all 25 mutation fixtures
  rejected with named errors, plus hash-mismatch/threshold/tamper
  rejection cases; one pinned finding — the template's t_S is
  liveness-degraded at some sizes under full linkage.
- **D3 placement preflight tool** — `be16121a` —
  `python/postfiat_rpc/placement_preflight.py` + tests; PASS/FAIL per
  threshold dimension with `--strict` and fail-closed missing-field
  behavior; the thresholds document's placement-preflight marker updated
  (independence verifier stays `new:`).

### Boundaries

No Task Node action; devnet not queried or mutated; the operator's
campaign workspace `postfiatl1v2-dravlic` untouched (its uncommitted
`run_host_v3.py` patch left as found); fork clones read-only; all work
from the `postfiatl1v2-e1` worktree pushed via `HEAD:main`; the only Vast
write action was the single agreed stop call.

### Decisions for the operator

1. Answer the
   [pending-decisions sheet](../governance/pending-operator-decisions.md)
   — now including the model-authority row with both evidence sets.
2. Confirm or adjust the
   [D3 launch thresholds](../architecture/launch-topology-thresholds.md).
3. Name the two data inputs (height-915 archive, height-924 custodian).
4. Destroy or restart the stopped A100.
5. Operator-naming collision: overnight agent sessions filing handoffs
   under the `dravlic` slot collide with the one-file-per-operator
   convention — propose a distinct operator name for that agent.

### References

- [Pending operator decisions](../governance/pending-operator-decisions.md)
- [Sub-scorer shadow-eval note](../governance/dunl-subscorer-shadow-eval-20260901.md)
- [Launch topology thresholds proposal](../architecture/launch-topology-thresholds.md)
- [Genesis-registry proposal path design](../architecture/genesis-registry-proposal-path.md)
- [Testnet-path status page](../status/testnet-path.md)

## Operator correction — institution legitimacy is the product requirement

The operator clarified that the intended reputation score is simple: ask
`qwen/qwen3.8-27b` whether it recognizes the institution claimed by a provider,
request a 0–100 legitimacy score and explanation, and assign 0 when the model
does not recognize it. The deterministic validator sub-scorer work is not an
alternative to this institution-legitimacy judgment, and the prior
real-but-obscure positive floor does not apply.

Implemented in `python/postfiat_rpc/institution_reputation.py`, documented in
[Institution Legitimacy Scoring](../governance/institution-legitimacy-scoring.md),
and recorded as the answered model-authority row in the
[pending-decisions sheet](../governance/pending-operator-decisions.md).
