# Weekend builds checked, Solana withdrawals and main checks

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-23 UTC

## BLUF

I worked from the [23 November handoff](../status/handoff-2026-11-23.md). I checked the weekend's builds independently before they carry real money or go public (Solana withdrawals, the quantum-safe private payments, the Phantom deposit change), repaired the checks of `main` that failed since the join, started bringing archived work back onto `main`, and wrote the comparison for the private payments decision. During my day the Solana withdrawal release went onto all six validators (build `71dbc9b3`, rules from block 1841); my change that lets every Solana deposit withdraw is written against it as a following release. Nothing on the live chain was changed by me.

## Current state

**Chain and deployment**

- Last fleet read, read-only by me at 11:46 UTC: all six validators at height 1806, one state root, build `71dbc9b3`, the Solana withdrawal release (PRs [#212](https://github.com/postfiatorg/postfiatl1v2/pull/212), [#217](https://github.com/postfiatorg/postfiatl1v2/pull/217), [#218](https://github.com/postfiatorg/postfiatl1v2/pull/218)). They were on it at 10:21 UTC at height 1765.
- The verifier's one-time initialisation on Solana had not run at 11:47 UTC (public Solana read).
- `main` is at `a720cc80f` (the merge of #218); this handoff's branch `handoff/2026-11-23-dravlic` starts there. None of my PRs below is merged.
- [Current State](../status/chain-state-current.md) does not yet record the Solana withdrawal release.

**Solana withdrawals**

- My independent read of the withdrawal branch: private PR #40.
- My change, private PR #42: every Solana-source bucket of the shared custody can exit; a destination-owner check; a governed minimum of 0.30 USDC; close, paid and re-open with five of six validators; the service side. It was read independently five times over six rounds and is merged with the withdrawal branch and with `main`.
- Since the first release is live, the change is now a second release. Every rule of it sits behind its own release height, which is unset. With it unset, the build gave `main`'s verdicts and ledger digests in every compared test case: burns of the current and of another bucket, five-of-six decisions, an off-curve owner, a sub-minimum burn, the new governance kind. An independent read found no path on which it can disagree with the live build while that height is unset. Golden values at heights 1840 to 1842 are being added.
- Open in both designs: a refund for a burn that can never be paid; the validator set cannot change while Solana pfUSDC is outstanding; the changed verifier program is not built (no Solana build tools on the work server).

**Solana deposits**

- The epoch-change fix of 20 November is live: read at 04:45 UTC, the route's anchor slot 472,608,000 is exactly the first slot of Solana epoch 1094.
- A deposit whose proof arrives after a re-anchor: a failing test and a fix behind its own activation height are private PR #45. The independent read requested changes. The second round answers every point (one rule for the deposit path and the exit interlock, a governed time limit, key rotations, corrections, the operator's command); a re-read of that round is running.

**Quantum-safe private payments** (draft [#207](https://github.com/postfiatorg/postfiatl1v2/pull/207), at `a87887001`)

- Every number of the status page reproduced on a second machine except two harmless ones (table in the review).
- The branch's node replays the live chain from genesis to the live tip and state root.
- No value or double-spend problem found.
- Corrections to the status page: [#210](https://github.com/postfiatorg/postfiatl1v2/pull/210). Cheap pool checks before the proof check: [#211](https://github.com/postfiatorg/postfiatl1v2/pull/211) (read independently, approved).
- The two review documents: private PR #41.
- For the private payments decision, a one-page side-by-side of the two tracks: private PR #44.

**Task Node**

- Review comments on [#428](https://github.com/postfiatorg/tasknode/pull/428) and [#429](https://github.com/postfiatorg/tasknode/pull/429).
- [#430](https://github.com/postfiatorg/tasknode/pull/430), on top of #428 (two rounds, read independently): assertions on the nonce account are accepted on the two-signer form, the relay reports a check version, the one-signer rule is unchanged, and refusals are logged with the public layout.
- [#431](https://github.com/postfiatorg/tasknode/pull/431), on top of the draft #429: the withdrawal screen checks the payout address before the burn is built (a wallet key, and on Solana a system-owned or not yet existing account), because the first chain release does not check it; plus five smaller points.

**Checks of `main`**

- [#209](https://github.com/postfiatorg/postfiatl1v2/pull/209) repairs the jobs that failed since the 21 November join: formatting and one lint, the lane fee tests on the v2 fee kind, the reserve proof kit and fuzz lock files, the operation schema, the proof inventory pins, the `url` advisory, the height-1453 test and the shared test lock. Read independently (approved); merged with the new `main` (head `5636b07a5`).
- `main` itself still fails formatting, the `url` advisory, the reserve proof kit lock files and the schema test; #209 repairs all four.
- On the hosted run of #209 every job passes except the long test run, which was still running, and the lane export identity.
- The lane export program's identity changed with the shared types: draft [#216](https://github.com/postfiatorg/postfiatl1v2/pull/216) (lock files and a refreshed identity). It stays a draft until it has had its own review; the identity must be derived again on the new `main`.

**Archived branches**

- A verified merge-or-drop table for all 134 archived branches with unmerged work: private PR #43 (59 lines of work: 3 merge now, 11 merge after work, 5 the owner's choice, 25 keep archived, 15 drop).
- Brought back so far: the web wallet's FastPay inventory fixes ([#214](https://github.com/postfiatorg/postfiatl1v2/pull/214)) and the inventory revision ([#215](https://github.com/postfiatorg/postfiatl1v2/pull/215); the new fields are returned only on request, so the deployment order does not matter).
- The security repairs of 4 to 10 November are being ported in about six steps: the plan is private PR #47, port 1 private PR #46, port 2 private PR #48. Both ports were read independently and have a fix round.

**B24 price reading** (read-only from validator-0 at 07:44 UTC)

- The newest reading is epoch 8, finalized at height 1442; there is no later reading.
- At height 1759 it was 317 blocks old against the 128-block limit, so minting and redeeming B24 at NAV are refused until a new reading lands.

**Proving hosts.** A check that reports before the rented hosts run out of credit: [#213](https://github.com/postfiatorg/postfiatl1v2/pull/213). It runs once its timer is installed on the machine that runs the bridge services.

**Devnet.** Not touched. Read-only reads of the six validators and one read-only copy of one validator's data for the replay (key files excluded, copy deleted).

**Task Node tasks.** Requested, accepted and rewarded today:

- `task_b76fe4064e0121fffaa915006eee3cf4`
- `task_6595f824`
- `task_220e8764a3b624424b10dbe731e654b1`
- `task_1539f5460e4d907e1e3087060a9023ad`
- `task_1a80f0fb414b1f160c521ff09eb1f1ff`
- `task_12fd8d2f81c052d1a8aa0d5ab514dfb5`
- `task_5508f39c`
- `task_2e0a1dfc63482eea932af1c74c252262`
- `task_01a18770660ced658d807251548e4478`
- `task_f7bd39196787dff8d625c0d642859d87`
- `task_42f72aeeb72ce0669cfa1e127e4f0b56`
- `task_6d93efa19b3f127dd259ee47b7ea2f91`
- `task_5b5bf3d7`
- `task_9b3b59891d233721c3ed2232c6d26510`
- `task_803b527b4e5091b3c5bcfc87eaf54d5d`
- `task_1ae37faa0e54580185d4a4a81af91332`
- `task_4368e6214231583aa0a30fe712f6a907`
- `task_d913de593b92d79abc57bb24b7f351ce`

## Next decision or action

Ours:

1. [ ] The Solana withdrawal change as the release after the one that is live: a replay of the live chain with it, then the refund path for a burn that cannot be paid. Its height is not set until every validator runs it and the Solana verifier is upgraded and pinned.
2. [ ] The security repairs: ports 3 to 6, and an independent read of ports 1 and 2.
3. [ ] The lane export program identity: its own review, a fresh hosted build on the new `main`, then #209 and #216.
4. [ ] The late-deposit fix (private PR #45): the re-read's points, then the relay's submission of late proofs.
5. [ ] The validator move. My proposal: operators start as watchers with no vote and are measured, and the first outside validators come from the reliable ones. The first stage is a watcher mode in the node, a signed download and the receipt tool.

For the owner (date first asked):

- The one-time initialisation of the Solana verifier: run it now for the current bucket and upgrade the program later, or hold it until the second release. No announcement of withdrawals until every deposit can exit (2026-11-23).
- The B24 price reading job: no reading since 18 to 19 November (2026-11-23).
- How the security repairs are released: into `main` just before a release that is rolled at once, or a private release first (2026-11-23).
- Which private payments track ships in the wallet (2026-11-23, from the 23 November handoff).
- Task Node #428 with #430: the relay host first, then the web app, then the two-signer deposit with a real Phantom wallet, then the two relay settings (2026-11-23, from the 23 November handoff).
- Installing the credit alert's timer and setting the address its reports go to (2026-11-23).
- A real Ethereum deposit and withdrawal after the 21 November move (2026-11-23, from the 23 November handoff; not done by me).
- The audit firm and budget for the PFT token contracts (2026-10-26).
- Whether the three rules of block 1153 come back (2026-11-20).
- The follow-ups of 20 November that were shared privately (2026-11-20).

## References

- **Status:** [23 November handoff](../status/handoff-2026-11-23.md), [Master State](../status/master-state.md), [Current State](../status/chain-state-current.md).
- **Public PRs:** #207, #209 to #218, as linked above.
- **Task Node PRs:** #428 to #431, as linked above.
- **Private archive:** PRs #40 to #48.
- **Previous handoff:** [2026-11-20: Next release replays genesis and Solana reviews](2026-11-20___dravlic__next_release_replays_genesis_and_solana_reviews.md).
