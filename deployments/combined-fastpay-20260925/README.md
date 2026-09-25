# Combined and FastPay release deployment — 2026-09-25

Release `combined-fastpay-20260925`: the merged combined and FastPay line,
qualified at `f60e9639` ([qualification](../release-repair-20260925/README.md),
PASS, plus the height-1036 canary-backup addendum). Executable
`d66cecc36426ce05ced8730b2439a27285c6b404688acd13dc23594b884eabd6` (two identical
clean builds, rechecked before staging). Rollback release:
`fastpay-committee-20260925-r4` (`44b6794f…`). Identities: [RELEASE-ID.txt](RELEASE-ID.txt).

## Status

**Prepared and signed, not applied** (`status=PREPARED_SIGNED_NOT_APPLIED`).
Steps 1–5 were done on 2026-09-25: before state at height 1044, staging, signed
manifest `7a682ffe…`, rollout preflight PASS, signed canary backup at 1044 with
root `8f22d40f…`. No apply was run. The step-6 gate (full workspace test run)
was not met in the time available. All six validators still run r4 at height
1044 ([at stop](observed/at-stop.json)).

| Step | Result | Evidence |
|---|---|---|
| 1. Before state, read-only | PASS: six on r4 (`44b6794f`, build `943c4ca7`, manifest `a05cfa35…`), height 1044, tip `136985ca…`, root `8f22d40f…`, mempools empty; apt timers next run 2026-09-26 06:02–06:56Z | [before.json](observed/before.json) |
| 2. Stage from the r4 layout, local preflight | PASS | DEPLOY-SHEET §2 |
| 3. Sign, verify locally ×6 | PASS: manifest SHA-256 `7a682ffe…`, publisher `pfc531e0…`, `deployment-manifest-verify` passes for all six | DEPLOY-SHEET §3 |
| 4. `postfiat-safe-rollout preflight` | PASS: Vultr reconciled, six-way convergence at 1044, six signer rosters valid, 0 deletions, order 1, 0, 2, 3, 4, 5 | local `rollout-state.json` (`e00b3158…`) |
| 5. Signed canary backup, validator-1 | PASS: height 1044, root `8f22d40f…`, finalized-checkpoint verified, signed manifest `9b6c885f…`, snapshot publisher `pf4ebb80…` | local `pre-rollout-backup/` |
| 6. Gate: local full workspace test run | **NOT FINISHED after 45 minutes → STOP.** 793 tests passed and none failed so far; one Orchard swap-proof test in the `postfiat_node` library was still running, with the other test binaries still to come | [gate record](observed/workspace-test-gate.json) |
| 6. Applies | not run | — |

## To resume

Only after the step-6 gate passes:

- **Chain still at 1044:** run `apply-next` with the existing rollout state
  under `~/.postfiat/deployments/combined-fastpay-20260925/` (DEPLOY-SHEET §6).
  That state reads `inventory.txt` from
  `~/repos/postfiatl1v2-combined-fastpay-deploy/deployments/combined-fastpay-20260925/`
  and checks its SHA-256 (`6c11341d…`), so first recreate that worktree from
  branch `release/combined-fastpay-20260925`.
- **Chain has moved:** a fresh before read, preflight and backup first; do not
  reuse the 1044 state.

## Caveats

1. **Canary check needs a transaction.** After the canary apply, the check
   requires the canary to certify a new block. The chain makes blocks only for
   transactions and has been idle at 1044, so one devnet transaction is needed.
2. **No per-host data copies.** The rollout tool does not copy validator data.
   Rollback is the r4 executable and unit files with the data in place, verified
   by r4 first ([rollback-one.sh](rollback-one.sh)). The signed 1044 backup is
   the fallback. validator-1 has about 3.3 GB free.
3. **Leftovers on validator-1.** The backup step left an unsigned snapshot
   (221 MB, under `/var/lib/postfiat/pre-rollout-snapshots/`) and a copy of the
   new executable on validator-1. Nothing was deleted.

## Contents

- [DEPLOY-SHEET.md](DEPLOY-SHEET.md): the exact commands, in order, and the rollback.
- `rootfs/etc/`: the 33 generated files (12 units, 12 environment files, six
  runtime bindings, topology, both circuit metadata files). Generated from the
  r4 layout read from validator-0; 27 are identical to r4 after the release-ID
  substitution, and the six RPC units differ only in `Restart=always`
  (combined-line change `15af691d`).
- `stage-report.template.json`, `validator-bindings.signing.template.json`: the
  generated stage files with the local prefix replaced by `@STAGE@`.
- [manifest-input.unsigned.json](manifest-input.unsigned.json): the reviewed
  inputs of the signed manifest. As in r4, the top-level unit and environment
  are validator-0's RPC unit and environment; there are no separate operator
  reference files.
- [local-preflight.py](local-preflight.py): offline checks, adapted from
  `../combined-devnet-20260921/`.
- [observe-fleet.py](observe-fleet.py): read-only fleet observer (RPC over the
  tunnels, unit health, running executable hashes, real manifest verification,
  apt timer, disk); `observed/` holds its outputs.
- [rollback-one.sh](rollback-one.sh): one-validator return to r4.
- [publication-gates.json](publication-gates.json).
- [inventory.txt](inventory.txt): the r4 rollout inventory (SHA-256 `6c11341d…`).

Not in Git: private keys, the signed stage, rollout state and backup under
`~/.postfiat/deployments/combined-fastpay-20260925/` on the signing workstation.
No key material is in this directory.
