# G4 blocked on the measurement environment: operator decision required

- **Operator:** Post Fiat (`postfiatchad`)
- **Date:** 2026-08-29 UTC

## BLUF

Three one-run G4 campaigns executed today under the operator's standing
directive. The candidate's own behavior now passes everything it owns — every
correctness, convergence, bounded-work, migration-position, and comparison
gate across all three runs, with the once-failing height ratios brought from
2.69 (pre-remediation) to ~1.07–1.13. The remaining blockers are
environmental and marginal, and continuing to rerun the campaign would be
retry-until-pass, which the milestone forbids. An operator decision on the
measurement environment (or gate posture) is required before any fourth run.

## The three runs (all closed, none retried)

| Run | Candidate | Result | Detail |
| --- | --- | --- | --- |
| 1 (batched-index, `86929450`) | binary `d27fc062…112c` | FAIL | One isolated 108.4 ms residual (cap 100 ms) in round 43/500; diagnostic ratios from 9 completed windows: 1.0926 / 1.0869 — passing |
| 2 (hygiene-corrected, same) | same | FAIL on ratios only | First-ever full 5+5+5 completion; every gate passed except ratios: 1.1149 / 1.10091 vs ≤1.10 |
| 3 (fsync-lean, `66f30f13`) | binary `b3574429…ea40` | FAIL | One isolated 103.6 ms residual on a validator-4 round (20 ms resume, all stages normal) in height-5000-window-2 |

Private outputs: `~/repos/postfiat-storage-g4-measurement-a3c7bea9-86929450-v1`
and `-v2`, and `…-a3c7bea9-66f30f13-v1`. All receipts, reports, checkpoints,
lanes, corpora, and prepared-input manifests are preserved. To recover disk,
the reproducible content-hashed fleet/workspace **clones** inside the two
closed `86929450` run directories were deleted after their digests were
recorded in the preserved checkpoints; the original `ae658441` fleets remain
intact. Rebuildable `target/` caches and five unreferenced
`postfiat-storage-migration-smoke.*` scratch directories were also deleted.

## What three runs establish

1. **Candidate-owned defects are gone.** Vote-lock and certified-send
   migrations land on first use everywhere, including empty-state validators;
   at-cap resumes prune within the ≤5/≤5 limits; zero full-history reads;
   700-plus measured rounds of literal receipts and six-validator convergence.
   The legacy comparison shows the selected path ~2.7× faster than legacy at
   height 50.
2. **The residual-cap failures are host jitter, not hidden work.** Both
   violations were isolated single rounds (3.6 ms and 8.4 ms over a 100 ms
   cap), on different proposers, with every named stage normal, and one of
   them on a round whose storage resume took 20 ms. The host is a shared
   virtualized machine; ~0.3%/round stall probability across 750 rounds trips
   the cap in most runs. Run 2 passed all 750 rounds' coverage, showing the
   candidate hides nothing.
3. **The ratio is real but marginal.** Height-5,000 p95 exceeds height-50 by
   ~7–13% depending on run and window; the limit is 10%. The residual cost is
   validator-0's at-cap resume, now dominated by ~9 sequential fsyncs whose
   per-fsync cost on this host's virtio disk (~5–8 ms under load) dwarfs the
   algorithmic work (~6 ms on an idle host, proven by the release spot
   check).

## Decision needed (one of)

1. **Dedicated measurement host** (bare metal or pinned CPU/disk): removes
   both the stall probability and most of the fsync latency; the same frozen
   candidate and runner rerun once there. Recommended.
2. **Reviewed runner-gate amendment:** tolerate one isolated, bounded
   (<250 ms), height-uncorrelated residual round per window, and/or measure
   the ratio on a stall-robust statistic. This is a spec-level change to the
   frozen gate and needs explicit review; it must not be made merely to pass.
3. **Further candidate fsync reduction** (defer retention disposal, drop the
   intent-clear sync): honest but small (~9 → ~7), unlikely to clear the
   margin alone on this host.

No fourth campaign is authorized until one is chosen.

## 2026-08-30 closure update: fourth campaign, ratio-only miss by 0.3%/0.6%

With the operator-approved stall-tolerance amendment (runner `2e49abb2`, one
isolated 100–250 ms residual round tolerated per window, ratio thresholds
untouched) and a further fsync cut (candidate `1423770e`), a fourth one-run
campaign completed the full matrix with **every gate passing except the
ratios**: consensus `1.1028`, wallet `1.1061` versus ≤1.10. The median round
ratio is height-flat (`1.029`); the p95 statistic deterministically samples
validator-0's at-cap proposal rounds (8 of 50 per window exceed the 5% tail),
whose resume costs ~56 ms at the 1,024-tombstone cap versus ~24 ms at height
50 — now almost entirely the ~8 sequential fsyncs and ~600 KB rewrite of the
certified-send completed-index file on a virtio disk. Private output:
`~/repos/postfiat-storage-g4-measurement-2e49abb2-1423770e-v1`.

Four campaigns now bound the truth: 1.09–1.12 on this host. The remaining
honest engineering option is architectural: move the certified-send completed
index out of its standalone fsync-per-resume JSON file and into the
transactional `redb` store, so its durability rides the existing bounded
per-round commit (~1.2 ms durable-commit already measured and gated). That
removes nearly the entire at-cap delta at its source. Alternatives remain a
bare-metal measurement host or an operator-reviewed threshold change; the
threshold change is not recommended because the gate is correctly reporting a
real at-cap cost. No fifth campaign is authorized until the operator picks
one.

## Also in this session

The freeze/evidence cycle was performed twice (sources `86929450` and
`66f30f13`): zero-warning release builds, G1 manifests, G2
rollback + tamper/crash refreshes (all PASS), helper rebuild at `a3c7bea9`,
and prepared-input rebinds. Identities are recorded in the respective plan
documents. The G3 height-915 replay and height-924 custodian items are
tracked in the milestone; height-924 remains the standing external blocker.
