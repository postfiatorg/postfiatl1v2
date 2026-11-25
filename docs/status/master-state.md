# Master State

Recorded: `2026-11-21`; updated `2026-11-25`. Status: **the one source of truth for what the
code is and what the network runs.**

## The rule

There is one line of code: `main` in this repository.

- The six validators run code that is on `main`.
- A release is built from a commit on `main`, and from nowhere else.
- Work happens on a short-lived branch, goes into `main` by pull request, and the branch is
  deleted when it merges.
- A branch that will not merge is tagged into the archive and deleted. It does not stay in the
  branch list.

!!! note "2026-11-25: the running build is on `main` again"

    The validators run `04600868`, the head of the release branch `release/sol-exit-r2`. It is
    `main` at `e5c4f6ceb` plus the release height (block 1890), a reviewer-installer fix and a
    rehearsal-test fix. From the rollout on 24 November until the merge it was not on `main`.
    It was merged into `main` on 25 November 2026 at 11:43 UTC as `26acd4639` (#234, which carries #228 and #230);
    `git merge-base --is-ancestor 04600868 origin/main` succeeds from that commit on, so the first
    rule above is met again.

## What the validators run

Read from all six validators on 2026-11-25, 10:16–10:23 UTC, read-only `status`.

| Fact | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Build revision | `04600868` (`release/sol-exit-r2`; second Solana exit release, rules from block 1890) |
| On `main` | Yes, since `26acd4639` (#234, 25 November 2026). Its base `e5c4f6ceb` is the merge of #223 |
| Binary SHA-256 | `6dd2ee56d350854dc32f6542895fd51508d12901b068ac840be428d32f0f759b` |
| Deployment manifest SHA-256 | `cd37a303d953e575727ee94141d2b897e0eff9c6cb9d91c5a09fd04b0ddb4d1b`; `deployment_manifest_verified` true |
| Height | 1897, all six agreeing on tip and state root |

Releases since the join: `sol-exit-r1` (`71dbc9b3`, rules from block 1841, 2026-11-23) and the
second Solana exit release (`04600868`, rules from block 1890, 2026-11-24). Both were rolled one
validator at a time ([Current State](chain-state-current.md)).

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
  none followed up to height 1897 (read-only read, 25 November). The job pins the `sol-exit-r1`
  node binary while the fleet runs `04600868`, so its runtime check will stop it until the pin
  moves ([25 November handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/handoff-2026-11-25.md)).
- **Not checked:** a real Ethereum deposit and a real Ethereum withdrawal after the move; none is
  recorded up to 25 November. Real Solana withdrawals ran on 23 and 24 November
  ([Current State](chain-state-current.md)).

## Keeping it true

Before a release, check that the commit being built is on `main`:

```bash
git merge-base --is-ancestor <commit> origin/main && echo on-main
```

After a release, compare every validator's `build_git_revision` with `main`, and update the
table above. If they differ, that is an incident, not a state to leave.
