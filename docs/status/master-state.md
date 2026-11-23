# Master State

Recorded: `2026-11-21`. Status: **the one source of truth for what the code is and what the
network runs.**

## The rule

There is one line of code: `main` in this repository.

- The six validators run code that is on `main`.
- A release is built from a commit on `main`, and from nowhere else.
- Work happens on a short-lived branch, goes into `main` by pull request, and the branch is
  deleted when it merges.
- A branch that will not merge is tagged into the archive and deleted. It does not stay in the
  branch list.

## What the validators run

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

The validators report build revision `674dbb18`. `main` reaches that commit through the merge
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

Every bridge service runs from one checkout of `main`, at commit `7e67fc292`. They were moved
there on 21 November 2026, one at a time, each followed by a check.

| Service | Before | Check after the move |
| --- | --- | --- |
| Solana deposit relay, wallet API, health, re-anchor, prover tunnel | A private branch | A real 0.02 USDC deposit from Phantom was credited in 122 seconds, with no relay restart |
| Ethereum deposit bridge and wallet proxy | A private branch from 13 November | The route reports ready on the new process; a wallet request through the Task Node relay returns the chain height |
| Withdrawal (payout) and faucet | A private branch from 16 November | Both listen and answer; the payout service reports ready. No real withdrawal was sent |

The source for each deployed contract, Solana program and proof key is mapped in
[Bridge Audit Map](bridge-audit-map.md).

### Found during the move

- **Ethereum deposits had not been credited since 19 November.** The bridge built its claims
  with a validator binary four releases old, and the chain rejected them, while the bridge kept
  reporting ready. Two deposits of 10 USDC each were waiting, one for two days. The bridge now
  uses the current binary; both were credited at heights 1741 to 1745 and confirmed on the
  validators.
- **The daily price-reading job is failing.** It refuses to run because the validator binary
  is not the one it pins. The corrected checks are on `main`; the next scheduled run has not
  happened yet. *Update 2026-11-23:* it kept failing through 23 November; fixed and a reading
  landed the same day. See "B24 Schwab reading" below.
- **Not checked:** a real Ethereum deposit and a real withdrawal after the move.

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
