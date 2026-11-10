# nav-proof-watch — NAV proof cadence, freshness and exit-capacity verification

Plan 07 (`plans/07-one-click-navcoin-checkout.md`, criterion 6) and plan 10
(W4) require three operational things that nothing on this machine did
before 10 November 2026:

1. a **recorded observation window** in which recurring finalized NAV proof
   epochs can actually be observed (`07.6.2`);
2. **failure alarms** when proofs stop recurring, when a proof's NAV age
   cannot be bounded in wall-clock time, or when the chain stops advancing
   (`07.6.3`); and
3. a **buyer-visible exit capacity check**: what a buyer can actually redeem
   or cash out, measured, not promised (`07.6.1`).

`execution/review-03-reserve-proofs-nav-mint-caps.md` (10 November 2026)
established why: NAV age is counted only in PFTL blocks (R3-F1), the wallet's
`stale: false` means "the API sample is fresh", not "the NAV is fresh"
(R3-F4), and **no watchdog existed** ("I found no such watchdog among this
machine's user units", §2.4). This tool is that watchdog, prepared for
operation. It is **read-only**: one HTTP GET of the public wallet API
routes endpoint per observation, read-only `eth_call` for the vault check,
local files only otherwise. It never signs, submits, pauses, resumes,
deploys or restarts anything, so running it needs no authority beyond file
system access to its state directory.

## Files

| file | what |
|---|---|
| `nav_proof_watch.py` | `observe` / `evaluate` / `verify-log` / `terms-template` |
| `exit_capacity.py` | buyer-exit capacity verifier (routes + vault reads) |
| `test_nav_proof_watch.py` | unit tests (15) incl. live-capture fixtures |
| `fixtures/routes-live-20261110T1950Z.json` | verbatim live wallet API capture, 2026-11-10T19:50Z (sha256 `435dab6e…dba`) |
| `terms.template.json` | D-SETTLEMENT terms file, every value deliberately `null` |
| `systemd/nav-proof-watch{,-alarm}.service`, `systemd/nav-proof-watch.timer` | deployment templates — **not installed, not enabled** |

## Usage

```bash
# record one observation of every route (read-only GET; run from a timer)
python3 nav_proof_watch.py observe --state-dir /var/lib/nav-proof-watch

# audit the observation log's hash chain
python3 nav_proof_watch.py verify-log --state-dir /var/lib/nav-proof-watch

# evaluate the recorded window against founder terms (D-SETTLEMENT)
python3 nav_proof_watch.py evaluate --state-dir /var/lib/nav-proof-watch \
    --terms /etc/nav-proof-watch/terms.json

# buyer-exit capacity, measured (routes + vault eth_calls + operator lane status)
python3 exit_capacity.py --usdc-lane-status /etc/nav-proof-watch/usdc-lane.json \
    --out exit-capacity.json
```

### Exit codes (both tools)

| code | meaning |
|---|---|
| 0 | clean: observation recorded / no alarms / exit verified |
| 2 | **alarm fired** (proof/freshness/chain failure, or exit NOT verified) — this is the signal a pager or `OnFailure=` consumes |
| 3 | REFUSED — founder or operator input missing; the tool invents nothing |
| 4 | read error (wallet API / RPC unreachable or malformed) |

## The recorded observation window

Every `observe` appends one hash-chained JSONL record per route to
`observations.jsonl` (`postfiat.nav-proof-watch.observation.v1`). Each record
pins: chain height and state root, finalized proof epoch/packet hash/state,
`proof_age_blocks`, `proof_age_basis`, `operator_observation_unix_ms`,
`settlement_reserve_atoms`, `reserve_backed_redeem_atoms`, the sampler's own
`stale` flag and sample time. `prev_sha256`/`record_sha256` chain the log;
`verify-log` audits it. This file *is* the recorded observation window the
plans call for; anyone can re-derive every evaluation below from it.

## Founder terms (D-SETTLEMENT) — nothing is evaluated without them

`evaluate` requires four values, and refuses (exit 3, naming each) while any
is missing: `max_nav_age_seconds`, `proof_interval_seconds`,
`observation_window_seconds`, `decisions_ref`. Plan 07's decision box
D-SETTLEMENT ("What exact terms authorize settlement?") is their only
legitimate source; the 4-hour figure in the spec is an older specification
setting, explicitly not a founder requirement (plan 07 W2). The tool ships
`terms.template.json` with every value `null`, and `terms-template` prints
the same. **There is no default value anywhere in this code.**

## Alarms

`evaluate` judges only records inside the last `observation_window_seconds`
and emits (append-only, hash-chained `alarms.jsonl`, plus stdout, exit 2):

| code | severity | fires when |
|---|---|---|
| `proof_cadence_breach` | high | longest gap between finalized epochs in the window (incl. window start → first epoch, and last epoch → window end) exceeds `proof_interval_seconds` |
| `no_proof_finalized_in_window` | high | no finalized epoch appeared at all |
| `nav_age_not_wall_clock_verifiable` | high | the packet carries no `operator_observation_unix_ms` (age exists only as block count) — review-03 finding R3-F1; freshness against `max_nav_age_seconds` **cannot be verified**, and the alarm says so instead of guessing |
| `nav_stale` | high | observation time exists and age exceeds `max_nav_age_seconds` |
| `chain_idle_over_window` | medium | chain height never advanced across the window (block-counted age frozen: the idle-chain trap behind the October NAVs still being "fresh") |
| `sampler_reported_stale` | medium | the wallet indexer flagged its own sample stale — sampler health, deliberately NOT a NAV-freshness verdict (review-03 finding R3-F4) |
| `route_missing_from_window` | high | a route seen before has no samples in the window (sampler outage) |

## Exit capacity (`exit_capacity.py`)

Measures, from reads only: per-route sell lane state and settlement reserve
(redemption at NAV into pfUSDC); the pfUSDC vault's physical USDC balance
vs `totalObligations()` and its pause flag (`eth_call`); and the
pfUSDC→USDC lane's usability. That lane's status is not exposed by any
public endpoint (it lives in the operator node's
`navcoin-bridge-supply-status` on the validator host), so it must be
supplied as `--usdc-lane-status` JSON (`live_value_enabled`,
`operator_test_only`, `observed_at_unix_ms`, `source`). Without the file the
USDC-leg verdict is REFUSED (exit 3), never guessed. Dated reference state
(2026-11-10, `inventory/D-navcoin-checkout-schwab.md`):
`live_value_enabled: false, operator_test_only: true` — which is why
`07.6.1` could not be closed that day.

## Deployment (for whoever holds the authority)

Nothing here is installed by this branch. To operate it after authorization:

```bash
sudo install -D -m 0755 nav_proof_watch.py exit_capacity.py /usr/local/lib/nav-proof-watch/
sudo mkdir -p /var/lib/nav-proof-watch /etc/nav-proof-watch
# founder completes D-SETTLEMENT and signs off the values:
sudo cp terms.template.json /etc/nav-proof-watch/terms.json && sudoedit /etc/nav-proof-watch/terms.json
sudo cp systemd/nav-proof-watch.service systemd/nav-proof-watch.timer \
        systemd/nav-proof-watch-alarm.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now nav-proof-watch.timer   # cadence per the timer template
```

`nav-proof-watch.service` runs `observe` then `evaluate`; exit 2 (alarm)
trips `OnFailure=nav-proof-watch-alarm.service` (a stub whose ExecStart you
must point at the real paging channel — do not invent one; leave the
template's placeholder refusing until the operator names the channel).
Rollback: `systemctl disable --now nav-proof-watch.timer`, remove the three
unit files, `daemon-reload`. The tool is read-only, so disabling it can not
break anything else.

## Tests

```bash
cd tools/nav-proof-watch && python3 -m unittest -v test_nav_proof_watch
```

15 tests: hash-chain append/tamper detection, refusal without/incomplete
terms, the dated live capture alarming (no observation time, idle chain,
cadence breach), a synthetic healthy window passing, stale-NAV alarm when
observation time exists, sampler-stale alarm, exit-capacity refusal /
not-verified / measured-capacity / synthetic-verified, and selector
constants.
