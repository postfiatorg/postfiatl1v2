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

## Not yet on `main`

These run in production or are in progress and are not on `main`. Each is in the archive and
must come in by pull request.

- The Solana bridge's relay, prover and observer tooling.
- The wallet proxy that serves the Task Node wallet runs a 13 November checkout, older than
  `main`.

## Keeping it true

Before a release, check that the commit being built is on `main`:

```bash
git merge-base --is-ancestor <commit> origin/main && echo on-main
```

After a release, compare every validator's `build_git_revision` with `main`, and update the
table above. If they differ, that is an incident, not a state to leave.
