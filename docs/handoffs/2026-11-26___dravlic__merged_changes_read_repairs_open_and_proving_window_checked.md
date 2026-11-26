# Merged changes read, repairs open and proving window checked

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-26 UTC

## BLUF

I worked from the [26 November handoff](../status/handoff-2026-11-26.md). The thirteen pull requests merged after the release was cut (#244 to #256) are read, with a note on each. Nothing in them pays twice or shows a payment as final that is not, and five points should change before a release; those are [#265](https://github.com/postfiatorg/postfiatl1v2/pull/265), [#266](https://github.com/postfiatorg/postfiatl1v2/pull/266) and [#267](https://github.com/postfiatorg/postfiatl1v2/pull/267), each read by a second reader. `main`'s two red checks are repaired ([#263](https://github.com/postfiatorg/postfiatl1v2/pull/263)). The daily proving window ([#257](https://github.com/postfiatorg/postfiatl1v2/pull/257)) is read before its first unattended run: as the code reads, Ethereum withdrawals are not closed outside the window, and several failures send no alert. Both are repaired in [#272](https://github.com/postfiatorg/postfiatl1v2/pull/272) and [#273](https://github.com/postfiatorg/postfiatl1v2/pull/273), not installed. The FastPay proxy on the work server runs #239 since 09:44 UTC. The block on validator-5 is still on. The docs carry `sol-exit-r3` and the window's hours ([#264](https://github.com/postfiatorg/postfiatl1v2/pull/264)), and the Task Node text for the hours is [tasknode #442](https://github.com/postfiatorg/tasknode/pull/442). Nothing on the live chain or on a validator was changed by me.

**For tonight, 26 November: please watch the proving window by hand.** Check at 17:05 UTC that the session is really open, and make sure no Ethereum withdrawal is started outside 17:00 to 19:00 UTC. The launcher still raises no alert when it fails to start, and the Ethereum withdrawal service still reports ready all day until [#277](https://github.com/postfiatorg/postfiatl1v2/pull/277), which is in `main`, is installed on its host with its setting. #272 and #273 are not to be installed.

## Current state

**Chain and deployment**

- Read-only fleet read at 10:13 UTC: all six validators at height 1906, one state root, build `2356caf1` (`sol-exit-r3`), one deployment manifest. Validators 0 to 4 were read through the work server's forwards, validator 5 on its own host. Deployed state: [Current State](../status/chain-state-current.md).
- `main` is at `7d4739118`. Since the 26 November handoff it gained #263 (formatting) and #264 (docs). Nothing of today is deployed.

**The thirteen pull requests (#244 to #256)**

Ten are on `main`; #249, #250 and #251 were merged into other branches. My note is a comment on each.

- **Safe to release as they are:** #244, #245, #247, #253, #254, #255, #256, with smaller points in the comments. For #253 and #256: the pinned identity's source commit is not on `main`, and one test compares a value with itself.
- **#246 (Ethereum payout recovery), safe after #265:** a checkpoint call that lands after its target was already accepted was never confirmed, and one route failure kept payouts not ready for every burn. No path sends a second payout or marks one paid without a finalized receipt.
- **#252 (receipts during a commit), safe after #266:** the snapshot did not keep the store open for its read (a test now shows this on `main`). Still open and only documented: during a commit a cached `status` can be one block behind a receipt from the snapshot.
- **#248 (wallet payment finality), safe after #267:** a verifier error after submission lost the submitted id, so a retry signed a second payment.
- **#249 (Orchard transfer gate), not to `main` as it is:** its branch is 328 commits behind `main` and its admission set conflicts with `main`'s. The gate itself reads as correct; a paused activation is ignored.
- **#250 and #251 (PQ payments page):** #250 is fine after one sentence about the size budget. #251 drops the over-budget PFSH02 results and labels profile-2 properties as PFSH02. The two cannot both merge as they are.

**Repairs written today**

- **#263, merged:** #254 had merged two unformatted spots, which turned `rust-ci / check` and `open-reserve-proof-kit` red.
- **#265 (on #246):** the checkpoint is confirmed from the verifier's accepted commitment at the finalized block; route failures are retried after 5, 10 and 20 minutes and then stop gating other burns; a recorded payout the chain no longer knows is broadcast again from the journaled bytes; one history entry per unchanged wait.
- **#266 (on #252):** the snapshot holds the store for its read; a snapshot that is only unavailable falls back to the direct read, and a storage error it reports is returned as it is.
- **#267 (on #248):** the submitted id comes back as pending, with the verifier's error in `verification_error`.
- **[#270](https://github.com/postfiatorg/postfiatl1v2/pull/270):** `test_navcoin_export_jobs.js` failed once on `main` because the test took a deadline from the clock on each request. The fault is in the test, not in the product.
- The second read passed #267 as it was and asked for one more change in each of #265 and #266; those are in. #265, #266, #267 and #270 merge when their hosted runs pass.

**The proving window (#257)**

- My read is a comment on #257. Before tonight's window: the Ethereum withdrawal service's readiness never looks at the prover, so it reports ready all day, and a request outside the window ends in a failed route job; and the no-machine path, a failure inside `ensure` during `up`, a session that dies and a relay host down at 16:15 raise no alert. I read this in the code and have not tried it on the live service.
- Before the timers are enabled: `down` cuts running work, and the bundle is checked against the list that travels inside it, not against the committed pins. Nine smaller points are in the comment.
- Sound: both timers are in UTC, the Solana withdrawal service checks the fixed host key on every connection and reports closed outside the window, the forced command accepts only its three commands, the committed key files are public material only, and the deposit watcher fails closed.
- **#272, into `ops/colab-proving-window`:** every failure of an open sends the alert, `ensure` runs its steps independently and alerts once after three failed runs, a 17:05 UTC check alerts when no window is open. A shell test with stand-ins contacts no host: 4 of 14 cases pass on `30f0f1888`, 14 of 14 on the branch.
- **#273, on top of #265:** with two optional settings, the Ethereum withdrawal service reports closed outside the window and when the prover does not answer, and a request or a proof cut by the close waits for the next window. With neither setting it behaves as today.
- **Neither should be installed as it stands.** The second read of #272 and #273 came back "not as it is" for both: in #273 a job that already has an Ethereum transaction can be moved to the wait, and unsent jobs in later stages can still go on to a send; in #272 a step that hangs raises no alert, and a stale state file satisfies the 17:05 check. The points are comments on each, and their fix round is being written. For tonight, the two things to watch by hand are whether a window opened by 17:05 UTC, and that no Ethereum withdrawal is started outside it.

**The FastPay proxy and validator-5** (fault 1 of the 26 November handoff)

- The proxy is the one `~/.local/bin/pft-fastpay` started on the work server on 25 September, with its six forwards. It ran the wallet proxy from the shared checkout as of 11 November.
- At 09:44 UTC I fast-forwarded that checkout to `615d3d37e`, ran the test of #239 on it (passed) and restarted the proxy and the forwards. Two older `node server.js` processes of 24 September on that server point at local test nodes and were left alone.
- The block on validator-5 is still on. Its access file differs from `/root/authorized_keys.before-20261125-flood` in the one line with `permitopen`. Lifting it, and the hour of watching `rpc-events.ndjson`, is the next step on my side.

**Docs and Task Node**

- **#264, merged:** the status pages carry `sol-exit-r3`, the 10:13 UTC fleet read and the interrupted roll; [Network Status](../status/network-status.md) has a paragraph for users on when deposits and withdrawals are open; 17 files. No page that code pins by hash is touched.
- **tasknode #442:** one setting, `L1_PROVING_WINDOW_UTC=17:00-19:00`; the Solana deposit screen and both withdrawal screens show the hours in the reader's time zone, the time to the next opening, and a plain line when the window should be open and is not. Its hosted checks pass. It needs a deploy and the setting.

**#232**

- Why its checks are red is a comment on #232: eight jobs, already red before the push that enabled the `prover` feature, and the head `f04480d09` has no runs.
- **[#271](https://github.com/postfiatorg/postfiatl1v2/pull/271), into its branch:** a merge of `main`, formatting, the RPC inventory table row, the two lane-export lock files, `test_full_pipeline.js`, the python-sdk offline build and `url` 2.5.9. Its hosted `check` now passes `cargo fmt` and stops at `cargo clippy`.
- Not repaired: the A666 recovery policy's two findings (real proving inside a unit test), and `pft-binius-shielded` has no licence field. Still red behind those: the wallet proxy's parser accepts only PFSH02 payments, and 19 binary files are not classified in the artifact policy.

**Private items**

- Private pull request 53 replaces 52: the reviewer follow-ups on the code of `sol-exit-r3`. The details are there.
- The research paper stays on its private branch. Its independent check went through the whole text once and its points are corrected; the check after that found seven smaller points, none against the main safety statements, and they are corrected. The next check is running.

**#236's smaller points** ([#274](https://github.com/postfiatorg/postfiatl1v2/pull/274))

- Seven of the points the merge left are repaired, each with a test that failed on `main` first: the settler's archive lacked a module its payout path imports, a pass cut short lost its retry history, a missed discovery reset a burn's give-up count, one unreadable marker stopped discovery for all burns, the settlement's time bound left child processes running, the reviewer's installer accepted a key that does not decode, and a door request had no deadline.
- Left: the result cache's identity and per-authorization checks, the refund accounting check, the hosts' SSH environment policy, and two points in functions another branch is changing.

**Not done from the 26 November list**

- Item 1, watching today's window: it opens after my day and runs on the relay host.
- Item 6, a real Solana and Ethereum withdrawal through the window: not done.
- Item 9, the cancel-with-refund and one-validator-down runs on the live chain, the account history index rebuild and the full storage verification: not done; the live runs need an open window.

**Devnet and hosts.** Not touched. Read-only reads of the six validators. On the work server, the shared checkout was fast-forwarded and the FastPay proxy restarted (above). The work server has not answered from my side since about 12:25 UTC (no SSH, no ping). The fix round of #272 and #273, the paper's check and one read were running on it; my last command there was at 12:12 UTC.

**Task Node tasks.** `task_2bfa266fcf6bd9a5e26207aa7e1855ae` (rewarded): the repairs #265 to #267.

## Next decision or action

Ours:

1. [ ] Merging #265, #266, #267 and #270 when their hosted runs pass.
2. [ ] The private reviewer follow-ups (private pull request 53): one more read, then the install on the six hosts.
3. [ ] The paper: until one full check finds nothing, then the harness score.
4. [ ] #272 and #273: their fix round and a second read; then the points of #257 that are in neither (a drain at the close, the bundle pins, a lock against two opens).
7. [ ] Lifting the block on validator-5 and the hour of watching.
5. [ ] #271: the remaining red checks of #232.
6. [ ] A cancel with refund and a settlement with one validator down on the live chain, inside a window, together with the owner's side.

For the owner (date first asked):

- The work server: it has not answered since about 12:25 UTC; is that known on your side (2026-11-26).
- Tonight's window: whether it is open by 17:05 UTC, and no Ethereum withdrawal outside it; #272 and #273 are not for tonight (2026-11-26).
- tasknode #442: a deploy and the setting (2026-11-26).
- The reviewer follow-ups (private pull request 53): installed on the six hosts by the owner's side, as agreed in the meeting of 26 November. I say on that pull request when its last read is back, with the steps (2026-11-25).
- #249 and #251: I can bring #249 up to `main` (2026-11-26).
- #232: the licence of `pft-binius-shielded`, and the two findings of the A666 recovery policy (2026-11-26).
- The account history index rebuild and the full storage verification: I can run both, one host at a time (2026-11-25).
- From the 25 November list: a paid Solana provider, and signing the possession record again (2026-11-25).
- The governance note named in the 24 November handoff: could it go to the private repository (2026-11-24).
- How the security repairs are released; which private payments track ships in the wallet; the validator move's three decisions (2026-11-23).

## References

- **Status:** [26 November handoff](../status/handoff-2026-11-26.md), [Master State](../status/master-state.md), [Current State](../status/chain-state-current.md), [Network Status](../status/network-status.md).
- **Public PRs:** #232, #244 to #257, #263 to #267, #270 to #274, as linked above.
- **Task Node PRs:** #442.
- **Private repository:** PR #53.
- **Previous handoff:** [2026-11-25: Docs updated, exit release on main and first paper drafted](2026-11-25___dravlic__docs_updated_exit_release_on_main_and_first_paper_drafted.md).

## End of session (14:46 UTC)

**The work server.** It was unreachable from my side from about 12:25 to 13:37 UTC and did not restart. The sessions on it lost their provider connection and were started again. Its disk was at 99% from the day's builds; I removed three build caches of mine, and it is back at 92%.

**Validator-5.** The block is lifted since 13:39 UTC: the access file is the one from `/root/authorized_keys.before-20261125-flood` again, and the blocked file is kept as `/root/authorized_keys.blocked-20261126`. In an hour of watching (13:45 to 14:41 UTC) `rpc-events.ndjson` showed no `owned_apply_v3`. The proxy's log says its certificate recovery is still deferred ("got 4, need 5"), so it has not tried to deliver the three certificates again and #239 has not been exercised by it yet.

**Merged after the text above.** #265, #266, #267 and #270 at 12:42 UTC, and [#276](https://github.com/postfiatorg/postfiatl1v2/pull/276): two tests of #265 had a fixed clock, so `python-sdk` failed on `main` from 12:42 to 13:45 UTC.

**Still waiting for a second read, not to be installed before it:**

- #272 and #273: the fix rounds for the second read's points are pushed.
- [#277](https://github.com/postfiatorg/postfiatl1v2/pull/277): the small part of #273 on its own. With `WITHDRAW_PROVING_WINDOW_HOURS` set, the Ethereum withdrawal service's readiness says closed outside the hours; nothing in how jobs are processed changes. It takes effect after an install on the host of that service.
- tasknode #442: its second read came back "fine to merge after" three points (the withdrawal screens keep a stale clock and readiness, the local hours are off across a daylight-saving change, one test calls the configured service). The fix round is next; after it the pull request can be merged by either side.

**Other.** #271 has six more mechanical commits for the `cargo clippy` step. The paper's newest check found four points, which are being corrected. A one-page note of the recorded times per step on the two USDC routes is on the work server at `~/.cache/eth-bridge-facts-20261126.md`.
