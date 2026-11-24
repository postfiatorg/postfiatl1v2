# Second withdrawal release prepared and main checks green

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-24 UTC

## BLUF

I worked from the [24 November handoff](../status/handoff-2026-11-24.md). The second Solana withdrawal release is prepared as [#223](https://github.com/postfiatorg/postfiatl1v2/pull/223): every Solana deposit can be withdrawn, a burn that cannot be paid can be cancelled and refunded, each validator reviews a settlement itself, and the upgrade of the already initialised Solana verifier is rehearsed from the dumped mainnet state. A read-only replay of the live chain with it matches all six validators at height 1854. The checks of `main` are green again ([#209](https://github.com/postfiatorg/postfiatl1v2/pull/209) is merged). Each of the nine open items of the 24 November handoff has a pull request or a result, and a real Phantom deposit from the Task Node screen was credited. Nothing on the live chain, on Solana mainnet or on any host was changed by me.

## Current state

**Chain and deployment**

- Last fleet read, read-only by me at 12:17 UTC: all six validators at height 1854, one state root, build `71dbc9b3`. The Solana exit verifier's light account exists (initialised on 23 November for the revision-8 bucket). Deployed state: [Current State](../status/chain-state-current.md).
- `main` is at `fec6ff729`, the newest `origin/main` when I wrote this; this handoff's branch `handoff/2026-11-24-dravlic` starts there. Merged: [#209](https://github.com/postfiatorg/postfiatl1v2/pull/209) (the checks that failed since the 21 November join; merge `f82ed6986`) and [#214](https://github.com/postfiatorg/postfiatl1v2/pull/214) (the web wallet's FastPay inventory fixes). Neither was deployed by me.
- On `f82ed6986`, `rust-ci`, `product-security-ci` and `docs-build` pass. `pft-lane-export-identity` fails until the identity refresh (draft [#216](https://github.com/postfiatorg/postfiatl1v2/pull/216)) has had its own review; that waits until the withdrawal code of #223 is on `main`, because that code changes the identity again.

**Second withdrawal release** ([#223](https://github.com/postfiatorg/postfiatl1v2/pull/223), head `a74f1e896`, not merged; every rule behind its own unset release height)

- Exits: every Solana-source bucket of the shared custody can exit; a payout owner that is not a wallet is refused; a governed minimum (0.30 USDC by default); close, paid, re-open and refund with five of six validators.
- Refund for a burn that cannot be paid (design: `docs/specs/solana-exit-refund-design-20261124.md` on #223): a `cancel` on the verifier writes the burn's own marker as cancelled, so a burn is paid or cancelled, never both. Only the destination owner or the verifier's pause authority may cancel. `ops/sol-live/exit_cancel.py` prepares a pause-authority cancel for offline signing. On our chain a five-of-six refund decision credits the burner in the same series and bucket.
- Upgrade path for the verifier that is already initialised: the deployed program equals the build of `main`'s source byte for byte (116,928 bytes, sha256 `3b16fc309501814e0bba718ecadb80f02993f6f355153a13f37ae9d47d29354c`). The new build is 123,152 bytes, sha256 `4091ee16db29154e7aae642b637320c0a0d0cdee110da88d07416ab7b1c15b21`, and fits a 10,240-byte extension. A one-time migration signed by the upgrade authority changes only the light account's stored route.
- Rehearsals, not mainnet changes: a local validator loaded with the dumped mainnet program and accounts passes 40 of 40 cases; a separate local-only rehearsal of the pause-authority cancel passes 8 of 8. A revision-9 payout is shown only with a test verifier, because no real revision-9 proof exists yet.
- Per-validator settlement reviews (`ops/sol-live/exit_review.py`, its unit, timer and installer): each host reviews a paid or refund decision from its own node and its own Solana endpoint and signs only from its own fresh review. The first release's six-of-six path is unchanged while the height is unset.
- Payout worker: the paged witness of [#221](https://github.com/postfiatorg/postfiatl1v2/pull/221) is merged in; a failing validator is skipped; pages are checked to link; an oversized page shrinks to one block; temporary failures wait with a named status instead of ending the withdrawal.
- Evidence (offline, not a deployment): a read-only replay of the live chain with the branch's node code (release binary sha256 `b7015cda92dd94aa510e25a948c43c9700f10b80d0ea0b45ac815cdefb293ac8`; no file under `crates` changed after it) verified all 1854 blocks and matches all six validators' tip and state root. The golden values at heights 1840 to 1842 match. The workspace tests pass (2,469 passed, 0 failed at the merge of `main`). Independent reads of the upgrade path, the refund and the settlement reviews were done and their points closed on the branch.
- Open: paying a burn across a validator-set rotation (it waits with a named status); the operators' side of independent reviews (named in the runbook). The one page for the owner is at the top of "Before the second release height is set" in `ops/sol-live/RUNBOOK-LIVE.md` on #223.

**The nine open items of the 24 November handoff**

- **(1) One proving machine:** [#226](https://github.com/postfiatorg/postfiatl1v2/pull/226), a device option for the withdrawal proof host; not run on a GPU by me.
- **(2) #221:** read; my comment is on it. Its paged witness is merged into #223.
- **(3) and (9) The payout installer and the service user's own id:** draft [#225](https://github.com/postfiatorg/postfiatl1v2/pull/225) on top of #221. Two independent reads each found a fault; it stays a draft until its fix round has passed a read. The validator-0 timeout drop-in is left out on purpose.
- **(4) Independent settlement reviews:** in #223.
- **(5) Fast validator reads:** [#227](https://github.com/postfiatorg/postfiatl1v2/pull/227): measurements on a work server against a read-only copy, a design note (`docs/specs/fast-validator-reads-20261124.md` on #227), and `blocks`, `receipts` and `tx` answered by key instead of decoding the whole history: 2 to 4 s each became 0.02 to 0.03 s with identical replies. The `blocks` step was read independently and approved.
- **(6) The second release:** #223.
- **(7) The credit alert:** [#213](https://github.com/postfiatorg/postfiatl1v2/pull/213). An independent read found that an unset alert address counted as sent; the fix round is pushed.
- **(8) A real Phantom deposit from the Task Node screen:** done by me at 08:36 UTC: 0.05 USDC, one signer, Phantom's six Lighthouse instructions accepted, credited at block 1854 in about two minutes. Solana signature `52JAS15acywtLBPNqtfW3aJb65n75C74xyXpMoEMvCBekWXgMKGA8RZ7fND75frr7opoRiKPV5RV6mQjgyzX3kjc`.

**Node RPC.** [#224](https://github.com/postfiatorg/postfiatl1v2/pull/224) answers a reply over the 4 MB frame limit with `rpc_response_too_large` instead of silence, bounds request ids, closes a half-written connection, and makes the re-anchor job read and check eight-block pages. Read independently twice; one point (a waiting withdrawal must be visible) is in work.

**B24 reading.** My read of [#220](https://github.com/postfiatorg/postfiatl1v2/pull/220) is a comment on it (details sent privately). The reading of 23 November (epoch 9) keeps create and cash-in usable until block 1943.

**Task Node.** [#430](https://github.com/postfiatorg/tasknode/pull/430) is repaired (the file size check), based on `main`, and its hosted checks pass; not merged.

**Private repository**

- Private PR #41 has my final read of the private-fee branch as a comment.
- Private PR #45 has the tool side of the late-deposit fix and its fix round after an independent read.
- Private PR #49 is a rule change behind its own unset height, described there.
- Private PR #50 holds today's full review reports.
- Private PR #42 has a comment with two operational points.

**Devnet and hosts.** Not touched. Read-only reads of the six validators and read-only copies of one validator's data for the replays (deleted after each).

**Task Node tasks.** Requested, accepted and submitted today (all rewarded):

- `task_ad4f08116d4b66a019c843370a4b5d12`
- `task_62e39593aab46046bcef73cf6421572c`
- `task_1a2e845313245f2a0f3011623c71a16f`
- `task_fb93f69837f43edbd7c567697c5e5b03`
- `task_a6f78d396c881fa798a873519d6a7afa`
- `task_3702de508e7438b60b1ebe593cd6a4a2`
- `task_5f24be10a38b47a8ccde98274f7bc39c`
- `task_4e20e955a1c204e3fc889aabbc9f639d`
- `task_5c5e3351e396a0a142d5fb53fba76524`
- `task_73e5d60f1b3ee4aea684082bc17fd11e`
- `task_bdd15bb299eee0691ae971aa43a44455`
- `task_0b0b523e2af53b11f8816eaffd222fbd`

## Next decision or action

Ours:

1. [ ] The second release: one full rehearsal of the order on copies (the verifier upgrade and migration, the release on six validators, the height), and paying across a validator-set rotation.
2. [ ] The last slow history read of #227 (the egress witness), and paging for `vault_bridge_status`.
3. [ ] The reads still owed on today's work: #226 and #227, and the fix rounds of #213, #224 and #225.
4. [ ] The lane export program identity (draft #216) once #223 is on `main`; then security ports 3 to 6.

For the owner (date first asked):

- Three signatures with the verifier's upgrade authority for the second release (the extension, the upgrade, the migration), by the runbook's steps on #223 (2026-11-24).
- The proving hosts' credit: at the rate in the 24 November handoff it runs out at about 02:40 UTC on 25 November (2026-11-24).
- The B24 reading job: where it runs from now on and who operates it; and the live checkout should not move to #220 before the new enclave build's profile is registered (2026-11-24).
- The operators' side of independent settlement reviews, and the installs for #225, #226 and #213 (2026-11-24).
- The governance note named in the 24 November handoff: I could not read it where it is; could it go to the private repository (2026-11-24).
- How the security repairs are released (2026-11-23).
- Which private payments track ships in the wallet (2026-11-23).
- The validator move's three decisions (2026-11-23).
- A real Ethereum deposit and withdrawal after the 21 November move (2026-11-23; not done by me).
- The audit firm and budget for the PFT token contracts (2026-10-26).

## References

- **Status:** [24 November handoff](../status/handoff-2026-11-24.md), [Master State](../status/master-state.md), [Current State](../status/chain-state-current.md).
- **Public PRs:** #209, #213, #214, #216, #220, #221, #223 to #227, as linked above.
- **Task Node PR:** #430, as linked above.
- **Private repository:** PRs #41, #42, #45, #49, #50.
- **Previous handoff:** [2026-11-23: Weekend builds checked, Solana withdrawals and main checks](2026-11-23___dravlic__weekend_builds_checked_solana_withdrawals_and_main_checks.md).
