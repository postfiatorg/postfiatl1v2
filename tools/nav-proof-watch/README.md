# nav-proof-watch: NAV proof cadence, freshness and exit capacity

A NAVCoin's finalized NAV age is counted in PFTL blocks. On an idle chain that
count stops moving, so an old NAV can look fresh. The wallet API's `stale` flag
describes the age of the API sample, not the age of the NAV. This tool records
a window of observations, so anyone can show whether new proofs keep arriving,
and it raises an alarm when they stop, when a NAV's age cannot be bounded in
wall-clock time, when the chain stops advancing, or when its own log stops.

It is read-only. Each observation is one HTTP GET of the wallet API routes
endpoint; the capacity check adds read-only `eth_call`. It writes only inside
its state directory and never signs, submits, pauses, deploys or restarts
anything.

| file | what |
|---|---|
| `nav_proof_watch.py` | `observe`, `evaluate`, `verify-log`, `repair-tail`, `terms-template` |
| `exit_capacity.py` | buyer-exit capacity check (routes, vault reads, operator lane status) |
| `test_nav_proof_watch.py` | unit tests, run in CI |
| `fixtures/routes-live-20261110T1950Z.json` | recorded routes answer of 2026-11-10 (no endpoint or host detail) |
| `terms.template.json` | the terms file with every value unset |
| `systemd/` | unit templates; nothing is installed by the repository |

## Usage

```bash
python3 nav_proof_watch.py observe --url "$NAV_PROOF_WATCH_URL" --state-dir /var/lib/nav-proof-watch
python3 nav_proof_watch.py verify-log --state-dir /var/lib/nav-proof-watch
python3 nav_proof_watch.py evaluate --state-dir /var/lib/nav-proof-watch --terms /etc/nav-proof-watch/terms.json
python3 exit_capacity.py --routes-url "$NAV_PROOF_WATCH_URL" \
    --usdc-lane-status /etc/nav-proof-watch/usdc-lane.json --out exit-capacity.json
```

The endpoint has no default: pass it on every call. TLS certificates are
verified with Python's default context. If the endpoint's certificate expires
or does not match, the read fails with exit 4, nothing is recorded, and once
the newest record is older than two sampling intervals `evaluate` raises
`observation_log_stale`.

| exit | meaning |
|---|---|
| 0 | observation recorded, evaluation clean, log verified, or exit verified |
| 1 | the local log is damaged (chain broken, truncated, torn final line) |
| 2 | alarm: an alarm fired, or a measured exit gate failed |
| 3 | refused: terms or operator input missing or invalid; nothing is invented |
| 4 | the wallet API or RPC could not be read, or its answer is malformed |
| 5 | `evaluate` only: no alarm, but recorded time does not yet cover the window for every route |

## Terms

`evaluate` needs five values. They come from the product owner's settlement
terms; the tool has no default and refuses (exit 3, naming each) while any is
missing: `max_nav_age_seconds`, `proof_interval_seconds`,
`observation_window_seconds`, `sample_interval_seconds` and `terms_ref` (a
dated reference to the terms). It also refuses terms whose window is not longer
than the proof interval, because no gap inside such a window could exceed the
interval. `terms.template.json` and `terms-template` give the empty file.

## Log integrity

Every `observe` appends one hash-chained record per route to
`observations.jsonl` under an exclusive lock, then atomically rewrites
`observations.head.json` (record count and head hash). `verify-log` detects an
edited, removed, reordered or truncated record and refuses non-canonical lines
and duplicate keys. It is tamper evidence, not authentication: whoever can
rewrite the whole directory can rewrite the chain. If a crash leaves a torn
final line, `observe` stops with exit 1 and names the repair:

```bash
python3 nav_proof_watch.py repair-tail --state-dir /var/lib/nav-proof-watch
```

It moves the torn bytes to `observations.jsonl.torn-<ms>` and keeps every
complete record.

## Alarms

`evaluate` judges the window that ends now. Time before a route's first record
is not recorded time and never counts as a proof gap; until the recorded time
covers the window the route's cadence is "window not yet covered" (exit 5 if
nothing else alarms).

| code | severity | fires when |
|---|---|---|
| `observation_log_stale` | high | the newest record is older than two sampling intervals (observe stopped or keeps failing) |
| `proof_cadence_breach` | high | the longest recorded stretch without a newly finalized epoch exceeds `proof_interval_seconds` |
| `no_proof_finalized_in_window` | high | no epoch was first seen inside the window, even if an older epoch is still displayed |
| `nav_age_not_wall_clock_verifiable` | high | the packet carries no operator observation time; freshness "cannot be verified", the tool does not guess |
| `nav_stale` | high | the observation time exists and the NAV is older than `max_nav_age_seconds` |
| `chain_idle_over_window` | medium | at least two samples and the chain height did not move across a covered window |
| `sampler_reported_stale` | medium | the wallet indexer flagged its own sample stale (sampler health, not NAV freshness) |
| `route_missing_from_window` | high | a route seen before has no sample inside the window |

## Exit capacity

`exit_capacity.py` measures each route's sell lane and settlement reserve
(scaled by `settlement_asset_precision`; refused as malformed if absent), the
pfUSDC vault's USDC balance against `totalObligations()` and its `paused()`
flag. The pfUSDC-to-USDC lane status is not exposed publicly; the operator
supplies it with `--usdc-lane-status`. A failed measured gate gives exit 2 even
when the lane file is missing; a missing lane file alone gives exit 3.

## Deployment

```bash
sudo useradd --system --no-create-home --shell /usr/sbin/nologin nav-proof-watch
sudo install -D -m 0755 nav_proof_watch.py exit_capacity.py -t /usr/local/lib/nav-proof-watch/
sudo install -d -m 0755 /etc/nav-proof-watch
echo 'NAV_PROOF_WATCH_URL=<routes endpoint>' | sudo tee /etc/nav-proof-watch/env
sudo cp terms.template.json /etc/nav-proof-watch/terms.json   # then fill in the terms
sudo cp systemd/nav-proof-watch.service systemd/nav-proof-watch.timer \
        systemd/nav-proof-watch-alarm.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now nav-proof-watch.timer
```

The service runs as `nav-proof-watch` with the state directory as its only
writable path. It runs `observe` and then `evaluate`; an alarm, a damaged log
or refused terms trip `OnFailure=nav-proof-watch-alarm.service`, a stub to be
pointed at the operator's paging channel. A dead timer runs nothing: any later
`evaluate` (by hand or from another scheduler) reports `observation_log_stale`.
