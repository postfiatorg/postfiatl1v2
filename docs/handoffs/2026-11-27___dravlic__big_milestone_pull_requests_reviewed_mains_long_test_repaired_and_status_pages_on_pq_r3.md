# Big Milestone pull requests reviewed, main's long test repaired and status pages on pq-r3

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-27 UTC

## BLUF

I worked from the [27 November handoff](2026-11-27___antigravity__pfsh03_merged_big_milestone_specs_and_wave1_to_dravlic.md) in its order. Train A ([#295](https://github.com/postfiatorg/postfiatl1v2/pull/295)) had two independent reads: it replays all 1,915 blocks of the live chain exactly, and it is not safe to merge yet; ten points are on the pull request, the first being that its three activation heights cannot be set through governance. The four contract pull requests, the five Wave 1 pull requests and tasknode #450 are read, with a note on each. None is merged, because each has at least one point that would be copied forward or would change what runs. For the points with a clear repair I wrote the repairs as pull requests into the branches they belong to ([#300](https://github.com/postfiatorg/postfiatl1v2/pull/300), [#301](https://github.com/postfiatorg/postfiatl1v2/pull/301), [#302](https://github.com/postfiatorg/postfiatl1v2/pull/302), tasknode [#452](https://github.com/postfiatorg/tasknode/pull/452), [#453](https://github.com/postfiatorg/tasknode/pull/453), [#454](https://github.com/postfiatorg/tasknode/pull/454), [#455](https://github.com/postfiatorg/tasknode/pull/455)). Two of the contracts count days in blocks, on a chain that makes blocks only when there is something to order; that needs one decision. `main`'s long test had not passed since #280; [#298](https://github.com/postfiatorg/postfiatl1v2/pull/298) splits the job so it fits its limit again, both halves passed, and it is merged. The status pages carry `pq-r3` and a fleet read of this morning ([#299](https://github.com/postfiatorg/postfiatl1v2/pull/299), merged). Nothing on the live chain or on a validator was changed by me.

**For today, 27 November: the B24 daily job will refuse at 18:15 UTC until `pq-r3` has its line.** `ops/b24-live/release_pin.py` requires the fleet's release in `ops/b24-live/known-releases.txt`, which lists `sol-exit-r2` and `sol-exit-r3`. On the machine that runs the job: `git fetch origin pull/280/head` if the commit is missing, then `ops/b24-live/rehearsal_tool.sh pq-r3 9dcb689e c563a4dcf48ad4359ab0f37fb7a626efed2b2b6808fc1ebd57e2963900326fb6`, add the line it prints to `known-releases.txt`, copy the tool to validator-0 as it says, and run `python3 ops/b24-live/daily.py --check`. The tool's hash differs between machines, so the line can only be made there.

## Current state

**Chain and deployment**

- Read-only fleet read at 09:01 UTC: all six validators at height 1915, one state root, build `9dcb689e` (`pq-r3`), one deployment manifest, verified on all six. Recorded in [Current State](../status/chain-state-current.md) with the blocks 1910 to 1915 (#299).
- The running build `9dcb689e` is a commit of #280's branch, not of `main`; `main` holds #280 as the squashed, newer `b121df606`. [Master State](../status/master-state.md) says so. Block 1915 is a governance block "vault bridge route profile activated" that the 27 November handoff does not mention.
- `main` is at `1fc8af3f5` (#298).

**Train A (#295)**

- Sound: this branch's `verify-blocks` re-executed all 1,915 live blocks on a key-free copy of one validator's data and matched every header state root and receipt-id list; with every new field empty, the encoded bytes are identical to `main`'s; no historical expected hash was edited.
- Ten serious points, in the order to fix them, each with file and line in the [review comment](https://github.com/postfiatorg/postfiatl1v2/pull/295#issuecomment-7215428830): the three activation kinds cannot be ratified; an existing data directory fails its integrity scan and cannot be downgraded; mempool admission judges NFT, offer and escrow transactions by the old rules after activation; one `nft_burn` receipt changes before activation; `RevokeHolderPermit` works before activation; FastPay picks its rule from two different heights; the 4,096-objects cap can be used to stop a user from being paid and is not applied on every path; a full Solana burn loses its refund once the asset gate is on; a FastLane deposit credits a destination the gate would refuse; switching the gate on closes the exit for value of a gated asset that is already in the shielded pool. Twelve medium points follow there.
- [#302](https://github.com/postfiatorg/postfiatl1v2/pull/302), into #295's branch: nine repairs, one commit each, each with a test that fails without it. With them the storage verifier passes on the copy of a live validator's data. With the nine commits a release build re-executed all 1,915 live blocks on the copy once more and matched every header state root and receipt-id list. Its independent read is half done, so it is not merged. Two points so far: the backfill of the FastPay order index scans all anchors at every writable open, where once would do; and mempool admission now applies the existing recipient-format precheck to NFT, offer and escrow transactions also with the three heights unset (block execution already did). The read is repeated in full.
- Left for a decision, not repaired: which height selects the FastPay fee rule for signing, applying, recovery and replay; the design of the per-account cap; what happens to value of a gated asset already in the shielded pool at activation.

**The four contract pull requests**

- [#288](https://github.com/postfiatorg/postfiatl1v2/pull/288) (message, NFT, governance schemas): not ready to freeze. The order hash and certificate digest are written as 32 bytes and the chain's are 48; the sealed mode still carries an id of the recipient's key; signing, encryption and canonical JSON are not pinned down, and the vectors hold constant signatures and ciphertexts. [#300](https://github.com/postfiatorg/postfiatl1v2/pull/300) corrects the sizes, makes the vector check read the `.proto` files and adds rules for later changes.
- [#286](https://github.com/postfiatorg/postfiatl1v2/pull/286) (uptime feed, `upgrade`, `top`, RPC profiles): not ready to freeze. Days are counted in blocks; the fields the Task Node acts on are outside every signature; the fixture's signatures do not verify; a failed `upgrade` restores the validator's vote record; `upgrade` accepts an older release.
- [#287](https://github.com/postfiatorg/postfiatl1v2/pull/287) (Train A and B contracts): not ready to freeze. Train B counts time in blocks at one a second; the supply check at NAV finalization contradicts `main`'s vault-bridge rule and lets one burned atom per window stop a NAV update.
- tasknode [#446](https://github.com/postfiatorg/tasknode/pull/446) (Finance Hub schemas, signing guard): fine after three points. tasknode [#452](https://github.com/postfiatorg/tasknode/pull/452) has them: the wallet relink in the guard's allow-list, the signed bytes of a publish pinned with a known-answer vector, a tighter leak rule.

**Days counted in blocks (#286, #287)**

- The uptime rule counts "height-days" of 14,400 blocks; Train B counts 86,400 blocks a day. The chain makes a block only when there is something to order: 18 blocks in the two days to this morning. At that pace a seven-day notice never ends and the uptime rule measures nothing for more than a year; on a busy chain the reverse holds.
- This needs one decision for both contracts: a fixed block rhythm also when idle, or real time agreed in consensus.

**Wave 1 and Wave 2**

- [#289](https://github.com/postfiatorg/postfiatl1v2/pull/289) (SDKs, x402, relay): both SDKs encode all 49 message vectors byte for byte. The x402 gate never compared a payment with its own price, asset, recipient or request, and nothing in it reads the chain. [#301](https://github.com/postfiatorg/postfiatl1v2/pull/301) repairs the gate and names the facilitator a prototype until its adapter uses the chain's own order.
- [#290](https://github.com/postfiatorg/postfiatl1v2/pull/290) (validator operations): not as it is. With today's configuration a merge already changes how every validator answers sixteen read methods (a cache keyed by height serves a balance that changed between blocks; the caps of the worker path no longer apply), and `upgrade` can never pass its own health check (it compares an 8-character revision with a 40-character one).
- tasknode [#448](https://github.com/postfiatorg/tasknode/pull/448) (signing guard, governance engine): not as it is. Once deployed, every collaboration proof and the wallet relink would be refused, and a display name grants founder authority. tasknode [#453](https://github.com/postfiatorg/tasknode/pull/453) repairs those and moves its migration to 168 (#447 has 167, #449 has 169).
- tasknode [#447](https://github.com/postfiatorg/tasknode/pull/447) (Finance Hub): not as it is. A failed blacklist or suspension lookup grants access, a feed outage creates access, grace can last for ever, a publication's pointer commits outside its transaction, and Library item restrictions are ignored. tasknode [#455](https://github.com/postfiatorg/tasknode/pull/455) repairs those: the access check refuses when it cannot decide, the pointer is written in the publication's own transaction, and item restrictions apply on every route.
- tasknode [#450](https://github.com/postfiatorg/tasknode/pull/450) (reward payout over FastPay, NFT migrator): not as it is. The order it builds is not the chain's, the fallback can pay a reward twice, and a stop between burn and record loses an NFT. tasknode [#454](https://github.com/postfiatorg/tasknode/pull/454) repairs those; with the flag unset, rewards are paid as today.
- postfiatorg.github.io [#56](https://github.com/postfiatorg/postfiatorg.github.io/pull/56): not to be published yet. Of the validator guide's 22 instructions, 3 work today; the rest need #290 and #447. A repair that keeps both guides as drafts and corrects the explorer points is written; I hold it until we have spoken, because a merge there is published at once.
- Not read yet: [#303](https://github.com/postfiatorg/postfiatl1v2/pull/303) (Wave 3, opened at 12:54 UTC today), [#293](https://github.com/postfiatorg/postfiatl1v2/pull/293), [#294](https://github.com/postfiatorg/postfiatl1v2/pull/294), tasknode #449, postfiatorg.github.io #57, tasknode-apple #40.

**`main`'s long test ([#298](https://github.com/postfiatorg/postfiatl1v2/pull/298), merged)**

- Since #280, three things were hidden behind each other: two transport timing tests failed under the load of the PQ proving tests; the job ran past its two-hour limit; and behind that, `fail_fast_dos_rejects_bad_sig_bad_pow_and_aborts_siblings` in `crates/privacy_binius` asks for under 5 ms and measured 9.4 ms in the hosted debug build.
- #298: a new job `test-pq` runs the 29 tests named `pq_`; `test` runs the others, with the two transport timing tests alone on one thread and the fail-fast test alone in a release build; a check proves that every test runs exactly once. Nothing is deleted, ignored or loosened. On the hosted run `test-pq` took 1 h 23 min and `test` 1 h 56 min, both green: the first complete pass of the long test since #280. `test` passed with four minutes to spare, so [#304](https://github.com/postfiatorg/postfiatl1v2/pull/304) splits it once more by package: `test` keeps the node crate, `test-node-orchard` takes the node crate's Orchard tests and `test-workspace` every other package, about an hour each. Its first hosted run is going.
- If `test` is a required check in the branch protection, `test-pq` should be added there.

**Docs ([#299](https://github.com/postfiatorg/postfiatl1v2/pull/299), merged)**

- Current State, Network Status and Master State carry `pq-r3`, the fleet read and the blocks 1910 to 1915; the private payments and privacy pages say that PFSH03 is on `main` and active on the controlled devnet; eleven more pages had one sentence each. 17 pages, 55 statements. No page pinned by hash was touched.

**The Task Node opening-hours screens**

- tasknode #445 was merged in the cleanup with four points open. With the setting `L1_PROVING_WINDOW_UTC` unset, the three screens are as before.
- tasknode [#451](https://github.com/postfiatorg/tasknode/pull/451): one complete pass over every step from a readiness answer to the send, then the four points and two more repaired. Its second read found two further gaps (an answer that expires during signing or a slow connection still let the burn go; the ownership check and the storage change were two steps), and those are repaired. A third read confirmed both repairs and found one more of the same kind: the answer's age is measured with the monotonic clock only, which can stand still while the machine sleeps, so an answer from just before closing still looks fresh afterwards. That repair is being written. Not merged; its four hosted checks pass on `57e2539`. The setting should stay unset until it is merged.

**The research paper (private)**

- Its full independent check of 26 November found nothing, and the harness scored that version: 87.6, 87.8 and 90.8 per judge, each over five runs. A last read of the whole text then found one claim stated too widely and twenty smaller points. All are corrected; the check of the corrections confirmed the main one and found six points in today's new text. Those six are corrected too; one more independent check of today's changes is still to run before the paper is called done.
- The harness's OpenRouter account has credit again; that item of yesterday's list is closed.

**Not done from the 27 November list**

- Item 6, the lane-export ELF: my suggestion is one rebuild and rotation after Train B, with the check left red until then; nothing was changed.
- The five Wave 2 pull requests named above.

**Devnet and hosts.** Not touched. Read-only reads of the six validators. On the work server: a key-free copy of validator 1's data (4.4 GB, `~/tmp-dravlic-tests-b/train-a/copy-validator-1`) for the replay and the storage verifier. At 12:54 UTC the work server's disk stood at 99%: a test build of mine had filled it, and two of my sessions ended at that time. I removed that build's test programs; the disk is at 93% with 43 GB free, more than this morning.

**Task Node tasks.** `task_c1ebf2c4b6c1f8ad6cf218ad6bdf8476` (rewarded): the complete pass of tasknode #451. `task_7b96f88f11d47f4999aab54591997e13`: the nine Train A repairs (#302).

## Next decision or action

Ours:

1. [ ] #304: merge when its jobs are green; `test-node-orchard` and `test-workspace` then belong beside `test` and `test-pq` if those are required checks.
2. [ ] #302: its independent read in full, a fix round for the two points so far, then the merge into #295's branch.
3. [ ] tasknode #451: the repair for the sleeping clock, one more read, then the merge.
4. [ ] #300 and #301: their second read is running. tasknode #452, #453, #454, #455: an independent read of each.
5. [ ] The five Wave 2 pull requests not read yet, and #303.
6. [ ] The paper: one more check of today's corrections, then the last read.
7. [ ] A paper on the safety of the USDC bridges as a whole (the meeting of 26 November).
8. [ ] The reviewer follow-ups (private pull request 53): its last read.

For the owner (date first asked):

- The B24 daily job: the `pq-r3` line on the machine that runs it, before 18:15 UTC (2026-11-27).
- What a day is on this chain, for #286 and #287: a fixed block rhythm, or real time agreed in consensus (2026-11-27).
- Train A: which height selects the FastPay fee rule; the per-account cap; value of a gated asset already in the shielded pool at activation (2026-11-27).
- #288: whether the sealed mode may carry an id of the recipient's key (2026-11-27).
- The Task Node setting `L1_PROVING_WINDOW_UTC`: could it stay unset until tasknode #451 is merged (2026-11-26).
- Block 1915, "vault bridge route profile activated": which route, so the status pages can name it (2026-11-27).
- [Master State](../status/master-state.md) says the running build is not a commit of `main`: is a release cut from `main` planned before Train A (2026-11-27).
- The reviewer follow-ups (private pull request 53): installed on the six hosts by the owner's side (2026-11-25).
- The account history index rebuild and the full storage verification: all six report `verification_stale_or_missing`, last full verification at height 927; I can run both, one host at a time (2026-11-25).
- From the 25 November list: a paid Solana provider, and signing the possession record again (2026-11-25).
- The governance note named in the 24 November handoff: could it go to the private repository (2026-11-24).
- How the security repairs are released; the validator move's three decisions (2026-11-23).

## References

- **Status:** [Master State](../status/master-state.md), [Current State](../status/chain-state-current.md), [Network Status](../status/network-status.md).
- **Reviewed:** #286, #287, #288, #289, #290, #295; tasknode #446, #447, #448, #450; postfiatorg.github.io #56.
- **Repairs:** #300, #301, #302, #304; tasknode #451, #452, #453, #454, #455.
- **Merged:** #298, #299.
- **Previous handoff:** [2026-11-27: PFSH03 merged, PR cleanup done, Big Milestone specs locked and Wave 1 PRs open](2026-11-27___antigravity__pfsh03_merged_big_milestone_specs_and_wave1_to_dravlic.md).

## End of session, 14:00 UTC

What changed after the text above was written (13:06 UTC):

- **The research paper is done.** The last check of today's corrections found two reference points (a sentence that named the wrong default source commit, and one line number of the old verifier) and nothing in a claim or a proof. Both are corrected. The paper is on the private branch `research/solana-withdrawal-refund-safety-20261125` and is ready for you to read, change and use. It should stay private until the repair it describes is installed.
- **tasknode [#451](https://github.com/postfiatorg/tasknode/pull/451) is merged** (13:49 UTC). After the third read came two more short rounds: an answer's age is now measured with both clocks, and a clock that reads earlier than at the request's start ends the answer. The fifth read found nothing, and the four hosted checks passed. The code side is ready for `L1_PROVING_WINDOW_UTC`; switching it on is your side, after the next deploy. Not covered by any read: a real browser with a real sleep.
- **[#301](https://github.com/postfiatorg/postfiatl1v2/pull/301)** had its second read: fine to merge into its branch as it is. Five low follow-ups are in its comment. I have not merged it, so nothing moves under work that is going on in that branch.
- **[#300](https://github.com/postfiatorg/postfiatl1v2/pull/300)** had its second read: the 48 bytes are right, and four points came out of it (the vector check did not cover enum values, nor five of the forty messages). They are in as four commits, which are not read by a second reader yet.
- **[#304](https://github.com/postfiatorg/postfiatl1v2/pull/304)**, first hosted run: `test` and `test-node-orchard` passed in 35 minutes each. `test-workspace` stopped after 7 minutes on a false alarm of the split check's new feature guard, which read `cargo tree` with colour codes. That repair is being written.
- In "Next decision or action" above, items 3 and 6 are done.
