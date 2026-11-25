# B24 Schwab NAV Reading

B24 is the NAVCoin backed by a Schwab brokerage account. Its NAV is updated on chain by a
reading: the account value is read inside an AWS Nitro enclave, the enclave's signed result is
proven with SP1, and three transactions record it on chain.

All the code for a reading is in this repository:

| Part | Path |
| --- | --- |
| Enclave package (runs inside Nitro) | `tools/nav-reserve-proof/schwab-tee/pftl_schwab_tee/` |
| Enclave image recipe and pins | `tools/nav-reserve-proof/schwab-tee/enclave/` |
| AWS operator runner, archive verification | `tools/nav-reserve-proof/schwab-tee/operator/` |
| Scripts run on the EC2 parent | `tools/nav-reserve-proof/schwab-tee/parent/` |
| Mint-side Schwab deposit reader (productionrpc) | `tools/nav-reserve-proof/schwab-tee/mint_reader/` |
| Reading job (schedule, lock, rehearsal, submit) | `ops/b24-live/daily.py` |
| Chain-copy rehearsal tool | `tools/b23-nav-canary/` (Rust, `postfiat-b23-nav-canary`) |
| Proof, qualification, submit helpers | `tools/b23-nav-canary/*.py`, `ops/b23-tee-nav/daily.py` |
| Timer and service | `ops/b24-live/b24-tee-nav-daily.{timer,service}` |

Run everything from the live checkout `~/repos/postfiatl1v2-live` with the ops interpreter:

```bash
python3 -m venv ~/repos/postfiatl1v2-live/.venv-ops
~/repos/postfiatl1v2-live/.venv-ops/bin/pip install -r ops/requirements-ops.txt
```

## What a reading does

`daily.py` runs these stages in order. Each failure writes `failure.json` with the stage name
in the run directory and stops; nothing retries on its own.

1. `pre-capture-runtime-check`: all six validators must run the pinned node binary
   (`DAILY_NODE_SHA`) for both the transport and the RPC service.
2. `chain-read`: the next epoch must be `finalized_epoch + 1` under the expected proof profile,
   and the asset must not be halted.
3. `collector-plan`, `aws-preflight`, `budget`: reviewed plan, no leftover instance, a cost
   reservation within $9 per run and $250 per month and lane.
4. `capture`: one EC2 parent builds the enclave image twice (PCRs must match), the Schwab
   credentials are sealed to KMS for that enclave, and the enclave reads the account. The
   instance is torn down; `final-inventory.json` must show no billable compute left.
5. `archive-verification`, `reading-qualification`, `proof`: the encrypted archive is checked
   against the source bytes, negative readings are rejected, and the SP1 proof is generated on
   the GPU prover and verified.
6. `chain-lock`, `runtime-check`, `rehearsal`: under `b23-coord/chain.lock`, the three
   transactions are replayed on a copy of current chain state. A deliberately corrupted proof
   must be rejected and the real one accepted.
7. `submission`: `nav_reserve_submit`, `nav_epoch_finalize`, `pftl_uniswap_route_epoch_advance`,
   each checked for an accepted receipt and full ledger match on all six validators.
8. `landing-check`: the new epoch must read back as finalized with this reserve packet.
   The line `READING COMPLETE … epoch N, NAV x` is appended to `b23-coord/status/now.md`.

Run directories are under `b23-coord/evidence/tee/b24-daily/` (`daily-*` scheduled, `now-*`
on demand).

## Commands

```bash
cd ~/repos/postfiatl1v2-live
PY=.venv-ops/bin/python

$PY ops/b24-live/daily.py --check      # local inputs only; no AWS, no chain writes
$PY ops/b24-live/daily.py --now        # on-demand reading (refuses while the timer's run is active)
$PY ops/b24-live/daily.py --resume <run-dir>
```

`--execute` is what the timer runs at 17:55 UTC.

`--resume` finishes a run that already has a capture and a verified proof. It never starts an
AWS instance.

- If nothing was submitted, it runs stages 6 to 8.
- If the submit already landed (accepted on all six), it runs only finalize and pricing.
- It refuses if `complete.json` exists.

## When it fails

Read `failure.json` in the run directory first.

| Stage / message | Cause seen before | Fix |
| --- | --- | --- |
| `pre-capture-runtime-check`: requires release … on all six validators | The fleet was upgraded and `DAILY_NODE_SHA` still names the old binary | Confirm the new release is built from `main` and contains the B24 code, then pin its binary hash |
| `collector-plan`: MINT reader path is unavailable | `mint_reader/observe_deposit.py` missing | It is in this repository; check the checkout |
| `rehearsal`: canary expected status did not match | The rehearsal tool was built from an older release than the fleet runs | Rebuild `tools/b23-nav-canary` from the fleet's source commit, copy it to `validator-0:/tmp/b23-tee-nav-canary-<first 16 hex of sha256>`, point `CANARY` in `ops/b24-replacement/common.py` at it, then `--resume` |
| `submission`: `rpc_finality_submit_failed … Permission denied (os error 13)` | Root-owned entries in `/var/lib/postfiat/validator-N/certified-send-outbox/` | `chown` those entries to `postfiat`. Check whether the submit landed (six-validator receipt for the tx id in `live/01-daily-submit/expected-state.json`) before anything else, then `--resume` |

A failed run costs its AWS reservation. Use `--resume` rather than a fresh `--now` whenever the
run directory has `proof/public-values.bin` and `archive-verification.json`.

## Changing the enclave

Any change to `pftl_schwab_tee/` or `enclave/` changes the measured image, so its PCRs and the
proof profile change. Before the next capture:

1. Build twice and confirm identical PCRs.
2. Register the new profile with issuer-signed `nav_asset_register` on a chain copy, and qualify
   two hardware readings on it.
3. Prepare the rollback rebind.
4. Rebind live and take one `--now` reading, checked on all six validators.

Until that is done, a capture from changed code stops at `bind_measured_profile` ("Measured
daily profile differs") before any credential is released.
