# Merged FastPay and combined lines: qualified, deployment prepared but not applied

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-25 UTC

## BLUF

What happened, and why this lane merged the two release lines:

1. **FastPay was repaired and deployed overnight.** The other lane's releases
   `fastpay-committee-20260925-r2`, `-r3` and `-r4` repaired FastPay:
   committee signers are kept after the validator-5 key rotation, pending
   FastPay effects are restored, and restore keeps the checkpoint trust basis.
   They were deployed to all six validators with a new deployment publisher key
   generated on this server. That closes the publisher-key question that had
   been open since 2026-09-21.
2. **They were built on an older line.** The releases came from
   `release/fastpay-committee-20260925(-r4)`. That line branched before this
   lane's qualified combined release `release/combined-devnet-20260915`: 279
   commits with the NAVCoin and Arc combined work, the repaired history and
   proof verification, and the burn-6 and P3 repairs.
3. **Two consequences.** First, the deployed line cannot replay the chain's
   own history from block 1011. The [r4 handoff][r4] records the failure and
   works around it by signing timeout votes against the finalized checkpoint.
   The combined line repairs exactly this defect. Second, the NAVCoin and Arc
   transactions the other lane wants next cannot run on the deployed line.
4. **So this lane merged the lines.** It created
   `release/combined-fastpay-20260925` by merging the r4 line into the combined
   line (merge `f8722092`, [merge record][merge]).
   - Nine files conflicted. The rule was "keep the repaired behaviour,
     re-apply his intent on top". His `verify_block_log: false` is kept, no
     repair was dropped, and the proof inventory was rehashed with no drift.
   - Two swapped FastPay wallet tests were restored from main.
   - The merged tip `f60e9639` was qualified ([packet][packet], final
     `18bc4419`): two identical clean builds (executable
     `d66cecc36426ce05ced8730b2439a27285c6b404688acd13dc23594b884eabd6`); 18
     full-history checks PASS at 1020/1021, while the r4 binary fails at block
     1011 on the same copy; the existing height-1036 canary backup verified
     and fully replayed with the merged build (root `ba7cc012…`);
     timeout-vote, view-recovery and FastPay tests green; governance gate and
     r4 rollback rehearsal PASS; proof inventory PASS.
   - Not covered: blocks after 1036 (no backup exists), and the full
     workspace suite on CI (no run started on draft [PR #50][pr50]; a local
     full run was chosen as the deploy gate).

**The deployment is prepared but not applied.** It was prepared up to and
including the signed canary backup. The plan gated the first apply on the full
workspace test suite. That run (793 passed, 0 failed, then a long Orchard
swap-proof test) had not finished in the time available, so the fleet still
runs r4 at height 1044. Everything needed to resume is in
[DEPLOY-SHEET.md][deploy-sheet] and under
`~/.postfiat/deployments/combined-fastpay-20260925/` on this server.

**The stall trigger is not repaired.** Validator-5 holds no FastPay effects,
so a FastPay payment followed by validator-5's turn to propose needs a view
change. The repair is consensus-affecting and belongs in its own release with
its own qualification. The other lane's two StakeHub wallet branches were not
reviewed today for lack of time; they are the first StakeHub item tomorrow.

## Current state

- **Branch `release/combined-fastpay-20260925`:**
  - `f8722092`: the merge (parents `f59dc07a` ours, `c1c81119` his).
  - `a2dfa947`: the [merge record][merge].
  - `f60e9639`: two FastPay wallet tests restored from main; the qualified
    code tip.
  - `bf9d22a7` and `99a4b2d5`: qualification checkpoints. `4da1d90b`: the
    [packet][packet].
  - `18bc4419`: addendum with the height-1036 canary backup replay PASS and a
    [fleet disk report][disk]. validator-1 is at 96 % used (13 G of logs, 10 G
    of September 5–7 dumps, 9 G of old snapshots); validator-0 is at 90 %.
    Nothing was deleted.
  - `cb84dd5c`: the [prepared deployment directory][deploy-readme], status
    `PREPARED_SIGNED_NOT_APPLIED`. To resume, first recreate the worktree
    `~/repos/postfiatl1v2-combined-fastpay-deploy` from the branch, because
    the saved rollout state pins `inventory.txt` at that path.
- **Merge verification (before qualification):**
  - cargo check, fmt and clippy clean.
  - Node library 156 passed; node binary 48 passed, including the
    failed-view-zero-proposer and `peer_round` tests; execution 57 passed;
    `atomic_swap_local_six` 1 passed (8 need a live network).
  - Proof inventory gate PASS (94 source hashes, no drift); three docs gates
    PASS.
  - Two Python wallet tests failed on the r4 line itself (swapped variables)
    and were restored from main.
- **Deployment preparation** ([README][deploy-readme], release ID
  `combined-fastpay-20260925`):
  - **Before state, 12:15Z:** all six on `fastpay-committee-20260925-r4`
    (`44b6794f…`) at height 1044, tip `136985ca…`, root `8f22d40f…`, empty
    mempools.
  - **Staged:** files generated from the live r4 layout; the six RPC units
    carry the combined line's `Restart=always`.
  - **Signed:** manifest `7a682ffe…` with the new publisher key `pfc531e0…`,
    verified for all six.
  - **Preflight:** safe-rollout preflight PASS (tunnels 27650, inventory,
    six-way agreement, signer registry).
  - **Backup:** signed canary backup from validator-1 at 1044 (root
    `8f22d40f…`, snapshot key `pf4ebb80…`), verified.
  - **No apply.** To resume: if the chain is still at 1044, run `apply-next`
    with the existing rollout state; otherwise take a fresh before reading,
    preflight and backup first.
  - **Caveats:**
    - The canary check needs one devnet transaction to certify a new block.
    - The rollout tool makes no per-host data copies. Rollback is the r4
      executable and unit files with the data left in place, verified by r4
      first ([rollback-one.sh][rollback]). The signed 1044 backup is the
      fallback. validator-1 has about 3.3 GB free.
    - The backup step left an unsigned snapshot (221 MB) and a copy of the new
      executable on validator-1. Nothing was deleted.
- **CI:** Draft [PR #50][pr50], opened to run CI on the branch, has had no
  check run since it was opened. Main's `rust-ci` and `product-security-ci`
  have been red since at least 2026-09-23 (the `check` job and the
  `open-reserve-proof-kit` job), independent of today's work.
- **Fleet and repository boundary:**
  - **Last observed fleet state:** 2026-09-25T13:05:32Z, read-only
    ([at stop][at-stop]): all six at height 1044, tip `136985ca…`, root
    `8f22d40f…`, empty mempools. [Current State][state] does not yet record
    the FastPay releases; its latest full observation is 2026-09-14T11:32:29Z.
  - **Deployed:** `fastpay-committee-20260925-r4`, executable `44b6794f…`,
    RPC build revision `943c4ca7`, from the line
    `release/fastpay-committee-20260925(-r4)`, per the other lane's
    [r4 handoff][r4].
  - **Repository:** `main` was `f2725ea4` before this handoff; the release
    branch HEAD is `cb84dd5c`.
  - **Merged but undeployed:** the merged line at `f60e9639` (executable
    `d66cecc3…`), which carries the combined release, the history and proof
    repairs, and the burn-6 and P3 repairs.
  - **Devnet access today:** read-only, except the signed canary backup from
    validator-1 at height 1044 (the rollout tool's backup step: no service
    change, no apply, no transaction). No live probe was made while writing
    this handoff.
- **Task Node:**
  - Merge: `task_81b0a8d47b9d563693e9a9fb4b3a2394`, Rewarded 4.5 PFT.
  - Qualification: `task_9b22c73ab23432ca0e6d80deb2471180`, Rewarded 3.6 PFT.
  - Deployment: `task_ccac6023e24336892d1dc2511bc91a94`, accepted and open; no
    evidence is submitted until the rollout runs.
  - No Task Node action was taken for this handoff.
- **StakeHub:** master is unchanged by this lane today (`fe21283`, his). His
  branches [`wallet/pay-transfer-fastpay-20260925`][sh-pay] (view-recovery
  transfer path, 136 tests reported) and
  [`wallet/pft-registry-20260924`][sh-registry] remain unmerged and
  unreviewed.
- **Superseded:** `deployments/combined-devnet-20260923/` and
  `deployments/combined-devnet-20260921/` were prepared for a fleet that then
  ran `a666-source-route-20260907`. They are kept as history; the deploy
  candidate is now the merged line.

## Next decision or action

### This lane's next steps

1. **Finish the deployment** of `combined-fastpay-20260925` per
   [DEPLOY-SHEET.md][deploy-sheet] once the full workspace suite is green (CI
   on [PR #50][pr50], or a local run without the 90-minute cap):
   - a fresh before reading, preflight and backup if the chain moved;
   - `apply-next` on validator-1, then 0, 2, 3, 4 and 5, with a health and
     convergence check after each;
   - one devnet transaction to certify a block on the canary;
   - the after state and a chain-state note.
2. **First Z3 integrated cycle,** once the NAVCoin signer keys and the seven
   [cycle inputs][cycle] are supplied. The "after deployment" re-reads now
   apply to the merged release.
3. **Repair the FastPay stall trigger** in its own release: validator-5
   receives and holds accepted FastPay effects, or a governed FastPay
   committee rotation. The rotation is a governance transaction and is the
   other lane's call.
4. **StakeHub:** review the two wallet branches for correctness and merge on
   the full-suite basis. The view-recovery transfer path must be proven never
   to apply a transfer twice, and the SSH path to validator RPC must be
   read-only.
5. **Remaining P3 findings** (wallet review seven, money-path ten) and the
   reserve-proof successor once approved.

### Only the other lane can provide

Each item was asked again tonight.

1. **NAVCoin signer keys and the seven [cycle inputs][cycle]** — first asked
   2026-09-22.
2. **StakeHub custody Q1 and archive Q2** — first asked 2026-09-07; one letter
   each per the [proposal][custody].
3. **Reserve-proof successor adoption, yes or no** — first asked 2026-09-24
   ([PR #49][pr49], [proposal][successor]).
4. **FastPay committee rotation for validator-5** (governance) — first asked
   2026-09-25.
5. **Height-915 archive and height-924 custodian** — first asked 2026-08-30.
6. **AI-governance decision for Gate Zero Z2** — first asked 2026-09-03.
7. **Twelve [inventory][inventory] rows** — first asked 2026-09-10.

The publisher-key item (first asked 2026-09-21) is closed by his new key.

## References

- The other lane's handoffs: [FastPay down, stale committee][fastpay-down],
  [FastPay restored][fastpay-restored], [chain stuck after
  FastPay][chain-stuck] and [r4 view recovery, chain unstuck][r4].
- This lane's [September 24 handoff][previous].
- Release branch: [merge record][merge], [qualification packet][packet],
  [deployment README][deploy-readme] and [DEPLOY-SHEET.md][deploy-sheet].
- [Current State][state] and [cycle-1 inputs][cycle].
- StakeHub branches [`wallet/pay-transfer-fastpay-20260925`][sh-pay] and
  [`wallet/pft-registry-20260924`][sh-registry].
- Draft [PR #50][pr50].

[previous]: 2026-09-24___dravlic__wallet_reviewed_and_merged_reserve_proof_gaps_and_demo_runbook.md
[fastpay-down]: 2026-09-25___postfiatchad__fastpay_down_stale_committee.md
[fastpay-restored]: 2026-09-25_fastpay_restored.md
[chain-stuck]: 2026-09-25___postfiatchad__chain_stuck_at_1037_after_fastpay.md
[r4]: 2026-09-25___postfiatchad__r4_view_recovery_chain_unstuck.md
[state]: ../status/chain-state-current.md
[cycle]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-cycle1-inputs-20260922.md
[successor]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/nav-reserve-proof-successor-proposal-20260924.md
[inventory]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/defect-inventory-20260910.md
[merge]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-fastpay-20260925/docs/status/combined-fastpay-merge-20260925.md
[packet]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-fastpay-20260925/deployments/release-repair-20260925/README.md
[disk]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-fastpay-20260925/deployments/release-repair-20260925/fleet-disk.json
[deploy-readme]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-fastpay-20260925/deployments/combined-fastpay-20260925/README.md
[deploy-sheet]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-fastpay-20260925/deployments/combined-fastpay-20260925/DEPLOY-SHEET.md
[rollback]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-fastpay-20260925/deployments/combined-fastpay-20260925/rollback-one.sh
[at-stop]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-fastpay-20260925/deployments/combined-fastpay-20260925/observed/at-stop.json
[pr50]: https://github.com/postfiatorg/postfiatl1v2/pull/50
[pr49]: https://github.com/postfiatorg/postfiatl1v2/pull/49
[custody]: https://github.com/postfiatorg/StakeHub/blob/master/docs/review/custody-and-archive-decision-proposal-20260923.md
[sh-pay]: https://github.com/postfiatorg/StakeHub/tree/wallet/pay-transfer-fastpay-20260925
[sh-registry]: https://github.com/postfiatorg/StakeHub/tree/wallet/pft-registry-20260924
