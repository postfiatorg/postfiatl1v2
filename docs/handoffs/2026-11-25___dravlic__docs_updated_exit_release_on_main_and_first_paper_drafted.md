# Docs updated, exit release on main and first paper drafted

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-25 UTC

## BLUF

I worked from the [25 November handoff](../status/handoff-2026-11-25.md) and the two points of the 24 November meeting. The engineering docs are checked page by page against `main` and the live chain, and the corrections are on `main`. The second Solana exit release is on `main` ([#234](https://github.com/postfiatorg/postfiatl1v2/pull/234), which carries #228 and #230 with the repairs for what the go-live found), so the validators run code from `main` again. A wallet that is not one of the owner's did a full round trip through the Task Node (0.30 USDC in and out). The B24 price job has its change for tonight and a lasting change ([#235](https://github.com/postfiatorg/postfiatl1v2/pull/235)). One settlement path, a restricted way to collect the hosts' signatures and a stricter deposit watcher are [#236](https://github.com/postfiatorg/postfiatl1v2/pull/236). The first research paper, on the refund rule of Solana withdrawals, is drafted with machine checks and is in the first correction round of an independent proof check. Nothing on the live chain or on any host was changed by me; the only live transactions are my own wallet's deposit and withdrawal through the public site.

## Current state

**Chain and deployment**

- Last fleet read, read-only by me at 12:48 UTC: all six validators at height 1900, one state root, build `04600868`. Deployed state: [Current State](../status/chain-state-current.md).
- `main` is at `ecac0727e`. The running build is on `main` since the merge `26acd4639` (#234, 11:43 UTC). #228 closed as merged with it; #230 I closed with a note, because its commits are on `main` through #234.

**Docs** (the site built from `docs/`)

- Every page of the site's menu except the whitepaper was checked: 159 pages. Each statement about the present (what runs, which release, what is live, each command, path, method and number) was compared with `main` and with read-only reads of the validators, and corrected, dated or left. 100 files changed.
- The status pages carry the releases of 21, 23 and 24 November and today's fleet read; [Network Status](../status/network-status.md) has a section on the USDC bridges with their limits; the [NAVCoins](../navcoins/index.md) page opens with each coin's newest reading; the home page has "What changed this week". Pages about earlier states carry their date and say what replaced them. Nothing was deleted, and the dated records were not touched.
- One docs commit of mine changed `docs/governance/validator-evidence-field-registry.md`, which the governance agent's tests pin by hash, so the `test` run of `main` failed from 11:06 to 12:45 UTC. The page is restored (`ecac0727e`).
- Open points that need the owner are under "Next decision or action".

**Solana exit**

- #234 on top of #228 and #230: the consensus and types tests that assumed an unset release height now test the table at 1889, at 1890 and for a chain without an entry; the upgrade rehearsal compares the loaded light with the snapshot's own dump (the one failing case of 40); the host reviewer is tested with an amendment serialized by a real node and refuses a malformed one; `settler.sh` reports failures and bounds each settlement. My review of 228, 230 and 229 is a comment on #228.
- [#236](https://github.com/postfiatorg/postfiatl1v2/pull/236), not merged: `exit_govern.py paid --burn <id>` settles any withdrawal through the one reviewed path, tested against the recorded withdrawals of blocks 1891, 1894, 1896 and 1899 and the canary; `oncall/settle.py` is removed. A host can open a restricted door (one key line with a forced command that takes two requests), and the settlement loop is a service under its own user. The service is built and not switched on: drafting, assembling and submitting a settlement still need a root login on a validator, so the tmux loop stays until those have a restricted path. The deposit watcher never uses a tip below the highest seen. It carries the commit of #229. Its independent read is running.
- The round trip, by my Phantom wallet through the Task Node screen: deposit `3UcHrsNwLpRVcL7SBnHZcDrM4TBn9zpU58okZ26h89XS5wVPuibgcAW7oQL1v29miDFVv7nQu1JEVU96uy56DeUC` at 10:29 UTC, credited in block 1898; burn in block 1899; payout `4NJjoVB55o2Ar7ospthZ8Emmg1RCoJZnitziThJNE8ZeLvwisBQ7kd2XphxH171DvgZNWjF47QwaW5i2Fd5GsGtc` at 10:38 UTC; settled in block 1900. It is the first deposit from the screen since the provider change. My 0.05 pfUSDC of 24 November cannot leave: it is below the minimum, and a withdrawal spends one batch.
- A rehearsal on copies of a cancel with refund and of a settlement with one validator stopped was running when I wrote this; its result follows in the next handoff.
- Private pull request 52 holds reviewer follow-ups; the details are there.

**B24 price reading** ([#235](https://github.com/postfiatorg/postfiatl1v2/pull/235))

- No reading has landed since 23 November (epoch 9): the job pins the node binary of the release before 24 November. Create and cash-in stop after block 1943.
- The first commit is the change for tonight: the pin of the running binary (`6dd2ee56…f759b`, read from all six validators' `status` and from the running processes) and the rehearsal tool's folder. The numbered steps are the pull request's first comment.
- The rest makes the job read the release from the six validators, require a rehearsal tool that records the same source, and say the fix in each refusal; the tool's build is reproducible and a step of the release runbook. It had an independent read and a fix round; the fix round is not read again yet.
- Read from the chain today: A666's newest reading is epoch 9 at height 1108 and BMNRC's is epoch 1 at height 1099.

**Research paper** (private branch for now)

- "The refund rule of Solana withdrawals": the model, ten theorems with proofs, six diagrams, a Lean 4 model without `sorry`, an exhaustive exploration, the evidence kept apart from the proofs, and what is not proven.
- An independent check, which proved or refuted each statement before reading the paper's proofs, found that four statements were not exact as written and that the Lean model was not the paper's model. The first correction round is running. It stays a draft until a full round comes back with nothing.

**Other pull requests**

- [#213](https://github.com/postfiatorg/postfiatl1v2/pull/213) (credit alert), [#224](https://github.com/postfiatorg/postfiatl1v2/pull/224) (the node's reply over the frame limit) and [#226](https://github.com/postfiatorg/postfiatl1v2/pull/226) (one proving machine) each had an independent read and a fix round; I read the fix rounds. They merge when their hosted runs pass. #224 and #226 are merged with `main`; bringing #224 up to `main` showed that #223's handler had made part of it unreachable, which is resolved there.
- [#225](https://github.com/postfiatorg/postfiatl1v2/pull/225) was not touched today. #211 is closed: both of its commits are in #232. #210's corrections are not in #232 yet.
- Task Node [#436](https://github.com/postfiatorg/tasknode/pull/436): read, fine to merge, one small point in my comment. [#430](https://github.com/postfiatorg/tasknode/pull/430) is unchanged and green.

**Read on the live validators today (read-only)**

- `account_tx_index_status` on all six: the account history index is not usable (a v3 index that stops at height 1193; the running build uses v4), so `account_tx` answers through the bounded scan. The rebuild is the "Upgrade Backfill" of the [account history runbook](../runbooks/account-tx-index.md).
- All six report the storage's last full verification at height 927.
- The fast reads of #227 are in the running build: a `blocks` read on validator 0 answered in under half a second.

**Devnet and hosts.** Not touched. Read-only reads of the six validators, and read-only copies of one validator's data for the rehearsal.

**Task Node tasks.** Requested, accepted and submitted today:

- `task_aec7ce2b44ab493dba9a47c29e742494` (rewarded)
- `task_2af12a32503a6aafbd25c9b7a2c9f430` (rewarded)
- `task_56b7bfa3aa9ecd44f3edd026b4fba066` (rewarded)
- `task_8c0b9b61acdb02b21e80369ed1963524` (awaiting review)

The docs update and the paper ran without a task: the Task Node helper refused both times with a model and provider error.

## Next decision or action

Ours:

1. [ ] The reviewer follow-ups of private pull request 52: one more read, then the install on the six hosts.
2. [ ] The paper: the correction rounds until the independent check finds nothing, then the harness score.
3. [ ] #236: its read's points, and a restricted path for drafting, assembling and submitting, so the service can be switched on.
4. [ ] Merging #213, #224, #226 and the lasting part of #235 when they are green and read.
5. [ ] The rehearsal's result; then a cancel with refund and a settlement with one validator down on the live chain, together with the owner's side.
6. [ ] The docs: a pass every week, and the same day after a release.

For the owner (date first asked):

- The B24 job: the two values and the rebuilt tool on the main machine before 17:55 UTC today, by the steps on #235 (2026-11-25).
- The proving hosts' credit: at the rate in the 25 November handoff it runs out at about 20:20 UTC today (2026-11-24).
- Who installs the reviewer follow-ups on the six hosts, and who runs the account history index rebuild: I can do both in my window, one host at a time (2026-11-25).
- Task Node #436 and #430: merging and deploying (2026-11-24, 2026-11-25).
- From the 25 November handoff's own list: a paid Solana provider, and signing the possession record again (2026-11-25).
- Docs points only the owner can settle: whether the four validator release plans under `docs/plans/active/` are still active; about 60 governance pages that three pages name and that are not in the repository; the two rows of `docs/governance/pending-operator-decisions.md` that the releases since 21 November overtook; the validator evidence field registry page, whose text is stale and pinned by hash; whether Z3 and gate B0.2 count as met; the links in older Current State entries to deleted release branches (2026-11-25).
- The governance note named in the 24 November handoff: could it go to the private repository (2026-11-24).
- How the security repairs are released (2026-11-23).
- Which private payments track ships in the wallet (2026-11-23).
- The validator move's three decisions (2026-11-23).
- A real Ethereum deposit and withdrawal after the 21 November move (2026-11-23; not done by me).

## References

- **Status:** [25 November handoff](../status/handoff-2026-11-25.md), [Master State](../status/master-state.md), [Current State](../status/chain-state-current.md), [Network Status](../status/network-status.md).
- **Public PRs:** #213, #224, #225, #226, #228 to #230, #232, #234 to #236, as linked above.
- **Task Node PRs:** #430, #436.
- **Private repository:** PR #52.
- **Previous handoff:** [2026-11-24: Second withdrawal release prepared and main checks green](2026-11-24___dravlic__second_withdrawal_release_prepared_and_main_checks_green.md).
