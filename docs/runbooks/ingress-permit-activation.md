# Ingress permit activation

The Ethereum ingress driver (`wallet-proxy/eth-fast-lane-driver.js`) hashes every
`pinned_files` entry before readiness and before every job run. The deposit permit
(`ingress-authorization.json`) is one of those pins, so replacing the permit alone
fails closed: readiness exits 2 with `driver_config_invalid`, the bridge reports
`trustless_ingress_unavailable` and `POST /api/bridge/jobs` returns 503.

`wallet-proxy/ingress-permit-activate.js` changes the permit and every pin that
depends on it together. It does not relax any pin check.

| Mode | Effect |
| --- | --- |
| `--check` | Validates the candidate and prints the planned hashes. Changes nothing. |
| `--activate` | Validates, installs the permit, the driver config pin and the routes config pin, runs a fresh driver readiness, and restores everything on failure. |
| `--rollback` | Restores the files named in a record and verifies them by hash. |

## What is validated

- The candidate is a regular file, not a symlink, owned by the service user, mode `0600`.
- The permit consumer's own validator accepts it (window, wallet scope, source and
  route identity, value bound). Pass it with `--validator` and `--validator-arg`
  (`{permit_file}` is the candidate). The validator program and every absolute
  argument must already be hash-pinned in the driver config.
- The installed driver config passes its own pin checks.
- The routes config that the proxy loads (`TRUSTLESS_BRIDGE_ROUTES_JSON_FILE`)
  pins the driver config at proxy start. The tool rewrites that pin in the same
  step. An inline `TRUSTLESS_BRIDGE_ROUTES_JSON` that pins the driver config is
  refused, because the tool cannot rewrite it.
- With `--job-root` (or `TRUSTLESS_BRIDGE_JOB_ROOT`), activation refuses while an
  unfinished job's config snapshot pins the previous permit.

## Operator steps

Run every step as the service user from the worktree that holds the deployed driver.

1. **Check.**

    ```bash
    node wallet-proxy/ingress-permit-activate.js --check \
      --driver-config CONFIGS/ingress-driver.json --candidate NEW-PERMIT.json \
      --routes-config ROUTES.json --job-root JOB-ROOT \
      --validator VALIDATOR --validator-arg '{permit_file}'
    ```

2. **Rehearse on a no-money path.** Copy the driver config, permit and routes config
   into a rehearsal directory and rewrite their absolute paths to the copies. Run
   `--activate` there with `--no-routes-config` and a rehearsal record. Readiness only probes; it submits no
   deposit. Require `"readiness": {"ok": true}` in the record, then run `--rollback`
   on the rehearsal record.

3. **Activate.** Run step 1's command with `--activate --record RECORD.json`. Exit 0
   means the record shows `state: activated` and a fresh readiness result. The
   proxy's readiness cache refreshes within 30 seconds. Confirm
   `GET /api/bridge/readiness` reports ready before any deposit.

4. **Rollback.** `node wallet-proxy/ingress-permit-activate.js --rollback --record RECORD.json`
   restores the permit, the driver config and the routes config byte for byte.

Exit codes: `0` success, `1` refused with nothing changed, `2` activation failed
and was restored, `3` restoration failed, `64` usage. Backups are kept in
`RECORD.json.backups/`.

## Interrupted activation

The tool writes the record and a marker (`.ingress-permit-activation.pending.json`
next to the driver config) before the first rename. If the process dies between
renames, the next `--check` or `--activate` refuses and names the record and which
files are new. Run `--rollback` with that record, then start again.

## Existing bridge jobs

- A job's first driver run copies the driver config to `driver-config.snapshot.json`
  in its job directory. Every later run re-checks the snapshot's pins
  (`runJob` in `wallet-proxy/eth-fast-lane-driver.js`).
- A job that already has a snapshot pinning the previous permit fails that check
  before its state is updated. The bridge keeps retrying it with backoff, and it
  does not progress until the previous permit is back.
- Jobs without a snapshot pick up the new config. Finished jobs (`accepted`, `failed`) are unaffected.
- The routes config is checked only when the proxy starts, so the running proxy is
  unaffected until its next restart, which then expects the new driver config hash.
