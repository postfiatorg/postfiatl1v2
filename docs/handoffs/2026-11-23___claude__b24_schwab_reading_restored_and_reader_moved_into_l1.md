# B24 Schwab reading restored, and the reader moved into this repository

- **Operator:** Claude, working for the founder on the main machine (`claude`)
- **Date:** 2026-11-23 UTC

## BLUF

The daily B24 (Schwab) NAV reading had failed every day from 19 to 23 November. On
23 November a reading was taken and landed: **epoch 9, NAV 1.006419**, finalized on all six
validators at height 1844. Four separate faults were found and fixed along the way. On the
founder's order, the TEE Schwab reader and the whole B24 reading pipeline were also moved out of
the `navstrategies` trading repository into this one, under
[`tools/nav-reserve-proof/schwab-tee/`](https://github.com/postfiatorg/postfiatl1v2/tree/nav-tee/move-into-l1/tools/nav-reserve-proof/schwab-tee).
That move is in [PR #220](https://github.com/postfiatorg/postfiatl1v2/pull/220), not yet merged.
The runbook is [B24 Schwab NAV Reading](../runbooks/b24-schwab-nav-reading.md).

## Current state

### The reading that landed

Live probe performed in this session, 2026-11-23 about 20:54 UTC, all six validators read
directly.

| Fact | Value |
| --- | --- |
| Asset | B24, `52285e4d…f8d7af` |
| Epoch | 9, state `finalized` on all six at height 1844 |
| NAV per B24 | 1.006419 |
| Schwab account value (attested, TEE + SP1) | $423,033.09 |
| On-chain reserve the chain counts for B24 | $5,020.61 |
| Total backing | $428,053.70 |
| B24 in circulation | 425,323.38 |
| Proof profile | `d5f81a8d…b42a2` (unchanged) |
| Reserve packet | `71b4e86c…b68c63` |

Previous finalized reading: epoch 8, NAV 1.000394, 18 November.

| Step | Height | Transaction | Receipt |
| --- | --- | --- | --- |
| `nav_reserve_submit` | 1842 | `c3905ed7…0023c0` | accepted on all six |
| `nav_epoch_finalize` | 1843 | `8c8fcd57…424a62` | accepted on all six |
| `pftl_uniswap_route_epoch_advance` (pricing) | 1844 | `0cb48eaa…0e2194` | accepted on all six |

One AWS capture was paid for ($8.04 reserved). Month to date $72.36 of $250; lane cumulative
$112.54 of $250. Evidence:
`/home/postfiat/repos/b23-coord/evidence/tee/b24-daily/now-20261123t202819026534/` on the main
machine (not a git repository).

### What was broken, and the fix for each

1. **Stale validator pin.** `ops/b24-live/daily.py` refused to run unless every validator ran
   the release-5 binary (`69de5ce0…`). The fleet had moved on (on 23 November all six ran
   `sol-exit-r1`, binary `d46376ff…`, source `71dbc9b3`, which contains the whole B24 line).
   Every scheduled run since 19 November stopped at `pre-capture-runtime-check`. Fix: pin
   `d46376ff…`.
2. **A pipeline input that existed only in a deleted worktree.** The AWS runner refuses to start
   unless the mint-side Schwab reader `observe_deposit.py` exists at a path inside the worktree
   `postfiatl1v2-navcoin-proof-b2.2`. That worktree had been deleted, and the file had never
   been committed. It was recovered from the deployed copy on productionrpc
   (`/home/pfrpc/pftl-deposit-readonly/`) and is now committed in
   `tools/nav-reserve-proof/schwab-tee/mint_reader/`.
3. **Rehearsal on an old chain engine.** Before any live write, the pipeline replays the
   transactions on a copy of the chain using `postfiat-b23-nav-canary`. That tool was a release-4
   build, so it disagreed with the live engine on today's state (`invalid_asset_state:
   vault_bridge_receipt bridge_deposit claim requires bridge_deposit_evidence`). Fix: rebuild the
   tool from the fleet's source `71dbc9b3`, stored at
   `b23-coord/artifacts/sol-exit-r1/postfiat-b23-nav-canary` (`86c26a02…`). The exporter copy
   on validator-0 is `/tmp/b23-tee-nav-canary-86c26a02cbaa64a1`.
4. **Root-owned outbox jobs on five validators.** Validators 0, 2, 3, 4 and 5 had job
   directories in `/var/lib/postfiat/validator-N/certified-send-outbox/` owned by `root`, all
   created on 21 November at 13:30. The services run as `postfiat`, so the outbox maintenance
   could not move them, and a proposal through validator-0 returned
   `rpc_finality_submit_failed … Permission denied (os error 13)` right after the submit had
   committed. Fix: `chown -R postfiat:postfiat` on those entries only. Before-lists are on each
   host at `/root/outbox-owner-before-20261123.txt` and in the run's
   `outbox-ownership-fix/` folder. **Cause not established**: most likely a node command run as
   root during the 21 November rollout or replay.

A fifth, smaller fault: `ops/b24-live/operator.py` shadowed Python's standard `operator`
module. It only worked under the old navstrategies virtualenv, whose startup hook imports `enum`
early. It is a standalone script and is renamed `create_operator.py`.

### The move into this repository (PR #220)

| Was | Now |
| --- | --- |
| `navstrategies/yolo_indices/*` (enclave package) | `tools/nav-reserve-proof/schwab-tee/pftl_schwab_tee/` |
| `containers/yolo-nitro/` (image recipe, pins) | `…/schwab-tee/enclave/` |
| `containers/yolo-nitro/aws/` (operator runner) | `…/schwab-tee/operator/` |
| `tests/hardware/yolo_*_parent.py` | `…/schwab-tee/parent/` |
| `observe_deposit.py` (untracked, deleted worktree) and its two siblings | `…/schwab-tee/mint_reader/`, on a stdlib Schwab client (`schwab_client.py`) |
| navstrategies tests | `…/schwab-tee/tests/` (310 pass) |
| navstrategies virtualenv | `.venv-ops`, from `ops/requirements-ops.txt` |

`ops/b24-live`, `ops/b23-tee-nav`, `ops/b24-replacement`, `ops/b23`, `ops/withdraw` and
`tools/b23-nav-canary` now reference only this repository. A grep of those paths for
`navstrategies` finds only the Schwab token file's location on productionrpc
(`~/.config/navstrategies/schwab_tokens.json`), which is data on that host, not code.

Also new: `daily.py --resume <run-dir>` finishes a reading that was captured, proven or partly
submitted, with no new AWS run. It uses the same chain lock, validator check, rehearsal and
landing check as a normal run.

### Deployed state versus branch state

- **Live checkout** `~/repos/postfiatl1v2-live` (on `main`, local HEAD `a437eb5c`) has
  uncommitted edits: the pin, the rehearsal-tool path, `--resume` and its continuation, and the
  rename. They match the branch commits. They are uncommitted because the checkout must stay on
  `main`; they go away when PR #220 merges and the checkout moves to it.
- **The timer** `b24-tee-nav-daily.timer` (17:55 UTC) still runs `daily.py --execute` with the
  navstrategies interpreter and still captures from `~/repos/navstrategies-now`, because
  captures from the moved code need a new proof profile (next section). With fixes 1, 3 and 4
  in place it should pass; the next scheduled run (24 November) has not happened yet.
- **Branch** `nav-tee/move-into-l1`, HEAD `f2834a15`, pushed, PR #220 open.
- **The validators** were not restarted and no binary, unit or config changed. Only file
  ownership inside the outbox directories changed.

## Next decision or action

1. **Review and merge PR #220**, then move `~/repos/postfiatl1v2-live` to it, install the unit
   from `ops/b24-live/b24-tee-nav-daily.service` (now on `.venv-ops`) and run
   `daily.py --check`.
2. **Switch captures to the moved enclave code.** The image built from the new layout contains
   only the reader package, so its PCRs differ and the proof profile changes. Steps: build twice
   and compare PCRs; register the new profile with issuer-signed `nav_asset_register` on a chain
   copy (precedent `b23-coord/evidence/now/step1-profile-replacement.json`); qualify two
   hardware readings on the copy; rebind live with the rollback rebind prepared; one `--now`
   reading checked on all six. Each AWS run is about $8; the lane has $137 left.
3. **Make the pins follow the fleet.** Both the validator pin and the rehearsal tool went stale
   on a validator release. They should be derived from the deployed release manifest, and the
   rehearsal tool rebuilt as part of each release.
4. **Find who created root-owned files** in the validator outboxes on 21 November, and add an
   ownership check to the rollout procedure.
5. **Deploy the stdlib mint reader to productionrpc** (`pftl-deposit-readonly/`). The deployed
   copy still imports the navstrategies broker client. `.scratch-deposit/record_deposit.py`
   on the branch already calls it with plain `python3`.

## References

- PR: [#220](https://github.com/postfiatorg/postfiatl1v2/pull/220), branch `nav-tee/move-into-l1`
- Runbook: [B24 Schwab NAV Reading](../runbooks/b24-schwab-nav-reading.md)
- Code: `tools/nav-reserve-proof/schwab-tee/README.md`, `ops/b24-live/daily.py`,
  `ops/b24-replacement/common.py`
- State: [Master State](../status/master-state.md)
- Earlier B24 handoff: [2026-11-17 NAVCoin B24 live](2026-11-17___claude__navcoin_b24_live_and_solana_route_to_dravlic.md)
- Evidence (main machine): `b23-coord/evidence/tee/b24-daily/now-20261123t202819026534/`,
  failed scheduled runs `b23-coord/evidence/tee/b24-daily/daily-202611{19..23}t*/failure.json`
