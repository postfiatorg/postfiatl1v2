# Master State

Recorded: `2026-11-21`; updated `2026-11-27`. Status: **the one source of truth for what the
code is and what the network runs.**

## The rule

There is one line of code: `main` in this repository.

- The six validators run code that is on `main`.
- A release is built from a commit on `main`, and from nowhere else.
- Work happens on a short-lived branch, goes into `main` by pull request, and the branch is
  deleted when it merges.
- A branch that will not merge is tagged into the archive and deleted. It does not stay in the
  branch list.

!!! warning "2026-11-27: the running build is not on `main`"

    Since the evening of 26 November the validators run `pq-r3`, build `9dcb689e47a4` ("fix(pq):
    allow pq_pool_v1 state domain on postfiat-wan-devnet-2", 20:47:30 UTC), a commit of #280's
    branch. `main` received #280 as one squashed commit, `b121df606`, on 27 November: the merged
    form, newer than the running build, and by the 27 November handoff it also holds #280's final
    CI and hygiene commits beyond `pq-r3`. `git merge-base --is-ancestor 9dcb689e origin/main`
    fails, so the first rule above is not met until a release cut from `main` is rolled.

!!! note "2026-11-25: the running build is on `main` again"

    On 25 November the validators ran `04600868`, the head of the release branch `release/sol-exit-r2`. It is
    `main` at `e5c4f6ceb` plus the release height (block 1890), a reviewer-installer fix and a
    rehearsal-test fix. From the rollout on 24 November until the merge it was not on `main`.
    It was merged into `main` on 25 November 2026 at 11:43 UTC as `26acd4639` (#234, which carries #228 and #230);
    `git merge-base --is-ancestor 04600868 origin/main` succeeds from that commit on, so the first
    rule above is met again.

## What the validators run

Read from all six validators on 2026-11-27, 09:01:27–09:01:34 UTC, read-only `status`, each
through its local forward.

| Fact | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Release | `pq-r3`, since the evening of 2026-11-26; PFSH03 post-quantum private payments activated at block 1910 (27 November handoff) |
| Build revision | `9dcb689e`, profile `release` |
| On `main` | No: `9dcb689e47a4` is a commit of #280's branch; `main` holds #280 as the squashed, newer `b121df606` |
| Binary SHA-256 | `c563a4dcf48ad4359ab0f37fb7a626efed2b2b6808fc1ebd57e2963900326fb6` |
| Deployment manifest SHA-256 | `25edebe2ba4081a6ad5aa4bb16eca9c73dbbe784151a4f33f2130eeac576aee4`; `deployment_manifest_verified` true |
| Height | 1915, all six agreeing on tip and state root |
| `main` ahead of it | Nothing merged to `main` after the running build was cut is deployed: #257, #274, #281, #285, `b121df606` and every later commit |

Releases since the join: `sol-exit-r1` (`71dbc9b3`, rules from block 1841, 2026-11-23), the
second Solana exit release (`04600868`, `release/sol-exit-r2`, rules from block 1890,
2026-11-24), `sol-exit-r3` (`2356caf1`, 2026-11-25) and `pq-r3` (`9dcb689e`, 2026-11-26,
PFSH03 from block 1910). The first three were rolled one validator at a time
([Current State](chain-state-current.md)). The roll of `sol-exit-r3` was interrupted after three
validators; Solana withdrawals were closed from 21:03 to 21:32 UTC
([26 November handoff](handoff-2026-11-26.md)).

### 21 November 2026, after the rollout

Read from all six validators on 2026-11-21 after the rollout.

| Fact | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Source commit | `674dbb18a29931d9ae749d28270e3c6cc508795a` |
| `main` at the join | `90c02da76` |
| Code difference between the two | None. They differ in seven documentation files only |
| Release name on the hosts | `main-r64-20261121` |
| Binary SHA-256 | `95d5b237c991f57a9ea122e110f6a2d0c017bf9ca0a85315b2359ecba3b017d1` |
| Signed manifest SHA-256 | `e4f2b8ff5781d4358f88c75c380f6c05ed121445f725d507d8c9383b5b388784` |
| Height when the rollout finished | 1738, all six agreeing on tip and state root |

On 21 November the validators reported build revision `674dbb18`. `main` reaches that commit through the merge
`90c02da76`, which adds documentation and changes no code.

## How this state was reached

From 28 September to 21 November 2026 the validators ran releases cut from private release
branches, while `main` moved separately. By 21 November the validators had 112 commits `main`
lacked and `main` had about 1,360 commits the validators had never run.

On 21 November 2026 the two were joined:

1. **Build.** The joined source (`674dbb18a`) was built into the binary above.
2. **Replay.** On an offline copy of a live validator's data, the new binary verified the
   finalized checkpoint, re-verified all 1,734 blocks from genesis, and passed the full state
   verification. Tip and state root equalled those of all six live validators.
3. **Rollback check.** The previous binary read the same data after the new one had used it,
   and again after the first upgraded validator had proposed a block.
4. **Rollout.** One validator at a time, in the order 1, 0, 2, 3, 4, 5, from 19:58 to 20:19 UTC.
   After each host all six agreed on height, tip and state root. The previous binary, units and
   manifest stay installed on every host.
5. **Mixed-fleet tests.** A test transaction was finalized with six accepted receipts at one
   new and five old (height 1735, proposed by the new version), at three and three (1736), with
   an old proposer and new voters (1737), and on the fully upgraded fleet (1738).
6. **Join.** The joined source was merged into `main` (`90c02da76`).

## The archive

Before any branch was deleted, every branch tip in the public repository and in the private
repository was tagged in the private repository under `archive/20261121/`. There are 345 such
tags, each verified to point at the identical commit. Nothing was discarded.

- `archive/20261121/public/<branch>`: a branch that was in this repository.
- `archive/20261121/private/<branch>`: a branch that was in the private repository.
- `archive/20261121/INDEX`: the inventory. One row per branch with its category, what it holds,
  and the evidence for that call.

To bring a line of work back:

```bash
git fetch <private-remote> tag archive/20261121/private/<branch>
git switch -c <new-branch> archive/20261121/private/<branch>
```

Then open a pull request against `main`.

Of the 345 archived branches, 106 were already contained in `main`, 3 were superseded, 103
were backup snapshots, and 133 hold code or documents that are **not** in `main` (111 code, 22
documents only). The 111 code branches reduce to 64 distinct lines of work. Each needs a decision: merge it or leave
it archived. The inventory lists them.

## Where the bridges run

On 21 November 2026 every bridge service was moved to one checkout of `main`, at commit
`7e67fc292`, one at a time, each followed by a check.

!!! note "2026-11-25: not all services run unmodified `main`"

    At the 25 November handoff (state of 24 November, 22:30 UTC) the relay host ran local edits,
    recorded on the branch `ops/relay-host-live-edits-20261125` and in #229 (not merged) and #230 (on `main` since #234),
    and the Solana payout service ran a bundle of the second exit release plus a speed-up
    (`c033278b`). Moving them back to unmodified `main` is open work in that handoff.

!!! note "2026-11-26: proofs only during a daily session"

    The proving machines that ran all day were stopped. A proving session runs during a daily
    window, planned for 17:00 to 19:00 UTC; outside it Solana deposits and both withdrawal paths
    (Solana and Ethereum) are closed on purpose. On 26 November the daily timers were not enabled
    and the window's launcher (#257) was not merged
    ([26 November handoff](handoff-2026-11-26.md)). At 17:13 UTC that day the window was not
    open: the Task Node showed Solana deposits and withdrawals closed. #257 was merged into `main`
    on 27 November at 00:41 UTC; by the 27 November handoff it is not deployed.

| Service | Before | Check after the move |
| --- | --- | --- |
| Solana deposit relay, wallet API, health, re-anchor, prover tunnel | A private branch | A real 0.02 USDC deposit from Phantom was credited in 122 seconds, with no relay restart |
| Ethereum deposit bridge and wallet proxy | A private branch from 13 November | The route reports ready on the new process; a wallet request through the Task Node relay returns the chain height |
| Withdrawal (payout) and faucet | A private branch from 16 November | Both listen and answer; the payout service reports ready. No real withdrawal was sent on 21 November |

The source for each deployed contract, Solana program and proof key is mapped in
[Bridge Audit Map](bridge-audit-map.md).

### Found during the move

- **Ethereum deposits had not been credited since 19 November.** The bridge built its claims
  with a validator binary four releases old, and the chain rejected them, while the bridge kept
  reporting ready. Two deposits of 10 USDC each were waiting, one for two days. The bridge now
  uses the current binary; both were credited at heights 1741 to 1745 and confirmed on the
  validators.
- **The daily price-reading job was failing.** It refused to run because the validator binary
  was not the one it pins. A B24 reading (epoch 9) was submitted at height 1842 on 23 November;
  none followed up to height 1897 (read-only read, 25 November). At that time the job pinned the
  `sol-exit-r1` node binary while the fleet ran `04600868`
  ([25 November handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/handoff-2026-11-25.md)).
  Since #235 the job selects its release from the fleet; epoch 10 (NAV per unit 1.006077,
  submitted at height 1901) is finalized (read-only `nav_reserve_proof_status`, 26 November).
  `ops/b24-live/known-releases.txt` lists `sol-exit-r2` and `sol-exit-r3` only, so with `pq-r3`
  on the validators the job refuses until `ops/b24-live/rehearsal_tool.sh pq-r3 9dcb689e
  c563a4dcf48ad4359ab0f37fb7a626efed2b2b6808fc1ebd57e2963900326fb6` has run on the machine that
  runs the job and its line is added. That step is open after `pq-r3`.
- **Not checked:** a real Ethereum deposit and a real Ethereum withdrawal after the move; none is
  recorded up to 25 November. Real Solana withdrawals ran on 23 and 24 November
  ([Current State](chain-state-current.md)).

## B24 Schwab reading

Recorded 2026-11-23 from a live read of all six validators.

- **Fleet observed.** All six ran `sol-exit-r1`: binary `d46376ff131e941cd271632d79eb5cfe4a22269d31588bd24caddda962d3e39e`,
  source `71dbc9b3`, which is on `main`. This supersedes the table above for the binary. Its
  own record belongs to the Solana exit release.
- **Last reading.** B24 epoch 9, NAV 1.006419, finalized at height 1844 (submit 1842, finalize
  1843, pricing 1844, each accepted on all six).
- **Where the code is.** The TEE Schwab reader moved from the `navstrategies` repository into
  `tools/nav-reserve-proof/schwab-tee/`, and the pipeline in `ops/` uses only this repository
  ([PR #220](https://github.com/postfiatorg/postfiatl1v2/pull/220), not merged on 2026-11-23).
  Operating guide: [B24 Schwab NAV Reading](../runbooks/b24-schwab-nav-reading.md).
- **Still on the old path.** Until PR #220 is merged and a new proof profile for the moved
  enclave is qualified, the 17:55 UTC timer captures from the `navstrategies-now` checkout.
- **Validator outboxes.** Five validators had `certified-send-outbox` entries owned by `root`
  since 21 November 13:30, which blocked proposals through them. Ownership was corrected on
  2026-11-23; the cause is not established.
- Details: [handoff 2026-11-23](../handoffs/2026-11-23___claude__b24_schwab_reading_restored_and_reader_moved_into_l1.md).

## Keeping it true

Before a release, check that the commit being built is on `main`:

```bash
git merge-base --is-ancestor <commit> origin/main && echo on-main
```

After a release, compare every validator's `build_git_revision` with `main`, and update the
table above. If they differ, that is an incident, not a state to leave.
