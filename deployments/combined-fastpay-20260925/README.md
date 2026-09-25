# Combined and FastPay release deployment — 2026-09-25

Release `combined-fastpay-20260925`: the merged combined and FastPay line,
qualified at `f60e9639` ([qualification](../release-repair-20260925/README.md),
PASS, plus the height-1036 canary-backup addendum). Executable
`d66cecc36426ce05ced8730b2439a27285c6b404688acd13dc23594b884eabd6` (two identical
clean builds, rechecked before staging). Rollback release:
`fastpay-committee-20260925-r4` (`44b6794f…`). Identities: [RELEASE-ID.txt](RELEASE-ID.txt).

## Status

**Deployed** (`status=DEPLOYED`, 2026-09-25 14:18:53Z). All six validators run
`combined-fastpay-20260925` (executable `d66cecc3…`, build `f60e9639`, signed
manifest `7a682ffe…`) and agree at height 1050, tip `03a24230…`, root
`13d9e652…` ([after.json](observed/after.json)). Steps 1–5 were done in the
morning; the step-6 gate passed at 13:26Z. The rollout resumed at 13:30Z: the
fleet was still at 1044 with the same tip and root
([resume-before.json](observed/resume-before.json)), so the existing rollout
state, preflight and signed backup were used (no fresh preflight or backup).

| Step | Result | Evidence |
|---|---|---|
| 1. Before state, read-only | PASS: six on r4 (`44b6794f`, build `943c4ca7`, manifest `a05cfa35…`), height 1044, tip `136985ca…`, root `8f22d40f…`, mempools empty; apt timers next run 2026-09-26 06:02–06:56Z | [before.json](observed/before.json) |
| 2. Stage from the r4 layout, local preflight | PASS | DEPLOY-SHEET §2 |
| 3. Sign, verify locally ×6 | PASS: manifest SHA-256 `7a682ffe…`, publisher `pfc531e0…`, `deployment-manifest-verify` passes for all six | DEPLOY-SHEET §3 |
| 4. `postfiat-safe-rollout preflight` | PASS: Vultr reconciled, six-way convergence at 1044, six signer rosters valid, 0 deletions, order 1, 0, 2, 3, 4, 5 | local `rollout-state.json` (`e00b3158…`) |
| 5. Signed canary backup, validator-1 | PASS: height 1044, root `8f22d40f…`, finalized-checkpoint verified, signed manifest `9b6c885f…`, snapshot publisher `pf4ebb80…` | local `pre-rollout-backup/` |
| 6. Gate: local full workspace test run | PASS at 13:26Z: 84 groups, 1,486 passed, 0 failed, 39 ignored (the morning's 45-minute attempt had stopped the rollout) | [gate record](observed/workspace-test-gate.json), `../release-repair-20260925/logs/full-workspace-tests.summary.txt` |
| 6. Applies, one at a time | PASS: six `apply-next` runs, exit 0, each followed by one faucet grant and a full observer check | table below, [rollout-record.json](observed/rollout-record.json), local `rollout-state.json` (`25ad45b0…`) |

## Rollout, 2026-09-25

Each `apply-next` verified the signed manifest on the host, restarted the
validator and RPC units on the new executable and checked six-way convergence.
Then one faucet grant of 1 PFT (fee 32 atoms) from the faucet account
`pfcd4cc8…` to the `testing` wallet (`pf6395ef…`) made the next block, through
StakeHub's `pft faucet` (`send_pft`); all six grants landed at view 0 and were
verified on all six. After each grant, [observe-fleet.py](observe-fleet.py)
checked units, running executables, the manifest signature on every host and
six-node agreement.

| Validator | Applied (UTC) | Grant height, proposer | Certificate voters (quorum 5) | Check after grant |
|---|---|---|---|---|
| validator-1 (canary) | 13:37:58 | 1045, validator-1 | 0, 1, 3, 4, 5 | PASS, root `8dcd52dc…` ([after-validator-1](observed/after-validator-1.json)) |
| validator-0 | 13:48:32 | 1046, validator-2 | 1, 2, 3, 4, 5 | PASS, root `0cea99a5…` ([after-validator-0](observed/after-validator-0.json)) |
| validator-2 | 13:55:50 | 1047, validator-3 | 0, 1, 3, 4, 5 | PASS, root `f2db80ff…` ([after-validator-2](observed/after-validator-2.json)) |
| validator-3 | 14:01:52 | 1048, validator-4 | 0, 1, 3, 4, 5 | PASS, root `87376902…` ([after-validator-3](observed/after-validator-3.json)) |
| validator-4 | 14:07:28 | 1049, validator-5 | 0, 1, 2, 4, 5 | PASS, root `e5ca3a4b…` ([after-validator-4](observed/after-validator-4.json)) |
| validator-5 | 14:14:09 | 1050, validator-0 | 0, 1, 3, 4, 5 | PASS, root `13d9e652…` ([after.json](observed/after.json)) |

Faucet grants used as block triggers: six, 1 PFT each at heights 1045–1050
(6 PFT plus 192 atoms of fees; faucet 91.995592 → 85.995400 PFT). Nothing
else moved. The canary's health read before its grant is in
[after-validator-1-apply.json](observed/after-validator-1-apply.json).

Notes:

- StakeHub's `~/.pft/config.toml` still names the r4 executable as
  `runtime_binary`, so the proposer-side round of each grant ran the r4 CLI
  on the proposer host. At 1045 the proposer was the canary itself, so its vote
  on 1045 came from that CLI; its new services then served 1045, and they voted
  as a non-proposer on 1046–1049. Pointing StakeHub at the new release is left
  to the StakeHub lane.
- The FastPay stall trigger is unchanged (validator-5 holds no FastPay
  effects; a FastPay payment followed by validator-5's turn needs a view change).

## Caveats

1. **No per-host data copies.** The rollout tool does not copy validator data.
   Rollback is the r4 executable and unit files with the data in place, verified
   by r4 first ([rollback-one.sh](rollback-one.sh)). The signed 1044 backup is
   the fallback. Rollback was not needed. validator-1 had 3.37 GB free after
   the rollout.
2. **Leftovers on validator-1.** The backup step left an unsigned snapshot
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
