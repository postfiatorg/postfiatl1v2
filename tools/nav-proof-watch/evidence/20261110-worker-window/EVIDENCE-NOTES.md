# Evidence: worker-recorded observation window, 10 November 2026

Produced by emergency-push worker **p07-c06** between **19:58:24 UTC and
about 20:41 UTC on 2026-11-10**, on the postfiat machine, using
`tools/nav-proof-watch` from branch `tools/nav-proof-watch-20261110`.

Every sample is one read-only HTTP GET of the public wallet API
(`https://5.223.45.94:5173/api/navcoin/routes`), identical in kind to the
GET recorded in `execution/review-03-reserve-proofs-nav-mint-caps.md` §1.
No order, transaction, signature, key, service action, deployment, paid API
or third-party contact occurred. Nothing was deployed; this directory is a
worker-run record, not a production deployment.

Files:

- `observations.jsonl` — the hash-chained observation log (7 samples × 2
  routes = 14 records). Audit with
  `python3 nav_proof_watch.py verify-log --state-dir <this dir>`.
- `observe-runs.log` — raw stdout of each observe run.
- `sample-times.txt` — UTC times the background sampler fired.
- `terms-SYNTHETIC-DEMO-NOT-D-SETTLEMENT.json` — **synthetic demonstration
  terms**. D-SETTLEMENT (plan 07) is undecided: no founder terms exist, and
  these values are used only to show what the alarm machinery reports when
  terms are supplied. They authorize nothing.
- `evaluation.json`, `alarms.jsonl` — the evaluation outputs over this
  window (with the synthetic demo terms) and the alarms it raised.

What this window records (plain facts, all from the samples):

- The finalized NAV epochs did not change: A666 epoch 9, BMNRC epoch 1 —
  the same October packets cited by review-03 at 02:00 UTC.
- `operator_observation_unix_ms` is null on both routes for every sample:
  NAV age is not wall-clock verifiable (review-03 finding R3-F1).
- Chain height stayed 1170 across the whole window (idle chain).
- The sampler's own `stale` flag stayed false (sample freshness ≠ NAV
  freshness, review-03 finding R3-F4).
