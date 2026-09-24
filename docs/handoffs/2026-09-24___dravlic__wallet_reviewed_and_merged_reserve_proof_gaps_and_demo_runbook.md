# Wallet reviewed and merged, reserve-proof gaps, and demo runbook

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-24 UTC

## BLUF

Today this lane did three things. (1) The other lane's new PFT wallet
(browser and terminal) and Ethereum-mainnet pfUSDC bridge ([PR #14][pr14])
were reviewed, repaired and merged into StakeHub master (merge `1d72f4b`;
follow-up [PR #15][pr15], `48de23c`). The [wallet review][wallet-review]
found 3 P1, 5 P2 and 10 P3 findings; all P1 and P2 and three P3 are repaired
with reproduce-first regression tests, and wallet tests went from 98 to 120.
**Live mode stays unqualified** until the read-only route check and the small
live qualification in [his wallet handoff][wallet-handoff] are done; both
need his proxy configuration and real money. (2) The [NAVCoin reserve-proof
mechanics review][reserve-review] found 3 P1, 5 P2 and 3 P3. The open kit
proves balances only below a committee-signed point, accepts a NEAR reader
record of any age and proves Aave debt for one asset only. The live A666 NAV
path (`sp1-groth16`, `stakehub-six-leg-reserves-v3`) has no epoch or
freshness binding. None of it can be repaired without a new program identity
and governance registration. The four repairs that need no consensus change
(N1, A1, B1, M1) are on branch `reserve-proof/freshness-and-debt-20260924`
([PR #49][pr49], open, not merged); a one-page [adoption
proposal][proposal] asks him for yes or no. (3) A [Token2049 demo
runbook][runbook] for 2026-10-06 is on StakeHub master, with every command
verified by running it. The wallet-web question is decided. The devnet was
not touched. The publisher-key decision is now three working days old and
blocks the prepared deployment [`combined-devnet-20260923`][candidate].

## Current state

- **StakeHub master (`48de23c`):**
  - `c6280b0` is the other lane's six safety commits of 2026-09-23.
  - `1d72f4b` merges [PR #14][pr14]: his wallet `34a508b`, this lane's
    [review][wallet-review] `313fe02`, and repairs:
    - `7c2a3dd` WB-01: a deposit signed twice after a broadcast timeout.
    - `240f4be` WB-02: an earlier payout of the same amount counted as
      success.
    - `13a1d9d` WB-03: a burn signed from unchecked quote fields with an
      unbounded fee.
    - `f83450c` WB-05, `9d899bb` WB-04.
    - `c37cedf` WB-06: the durable API token printed.
    - `911984a` WB-07: RPC provider keys in errors and the operation store.
    - `d51502a` WB-08: a missing amount shown as zero.
    - `47c60e3` and `15b71e9` (rebroadcast handling; review record).
    - `ed95920`: the [demo runbook][runbook] and the wallet decision, recorded
      in item 4 of [his wallet handoff][wallet-handoff].
  - `48de23c` ([PR #15][pr15]): WB-10, a burn accepted only on its own
    receipt; WB-11, crash adoption only by the saved burn transaction id;
    WB-13, a reverted or dropped deposit goes to `needs_reconcile`.
  - Seven wallet-review P3 findings remain recorded.
- **Behaviour changes the other lane should know:**
  - `mainnet_bridge.max_fee_atoms` (default 1 PFT) refuses a higher quoted
    fee.
  - `pft gui` prints a per-session link and no longer takes `--token-file`.
  - A failed API bridge job after money moved goes to `needs_reconcile`.
  - The WB-11 burn transaction id is computed in Python from the L1 layout
    and is unconfirmed against a real burn; it fails safe. The first
    supervised live withdrawal must confirm it.
- **StakeHub merge basis:** The full suite on this server had 5,235 passes,
  89 skips and 84 failures: the same 84 browser and environment-bound
  failures as unchanged master (identical sets on 2026-09-23 and today).
  These are local test results, not live-funds evidence.
- **Reserve-proof review** (main `cda9d91c`, `4df53782`): The
  [review][reserve-review] has a plain-language assessment and the findings:
  - **P1:** N1, the NEAR record age is unbounded (reproduced with a 90-day,
    7,776,000-block-old record); A1, Aave debt completeness; G1, the live
    `sp1-groth16` path has no epoch or freshness binding.
  - **P2:** N2, N3, X1, B1, M1. **P3:** N4, L1, L2.
  - Repairs need a successor program identity and governance registration;
    the order is in [what to do next, by risk][reserve-next].
- **Repair branch** ([PR #49][pr49], open): `08532dfb` B1, `fea9fda8` M1,
  `f37b6c60` N1, `303d2c14` A1, `670fa615` candidate identity record.
  - **Tests:** `reserve-proof-types` 89 + 3 + 4 passed, including 7 new
    negative tests that each fail with their check disabled; the CLI crate
    28 + 8 passed; fmt and the three gates passed.
  - **Not built:** This host has no Docker or `cargo-prove`, so the successor
    guest ELF is not built. The candidate identity is recorded, not built.
  - **Adoption, on the other lane's side:** build the ELF with the pinned SP1
    6.3.1 Docker image; set the new A666 manifest fields (NEAR record age
    limit, proposed 30 minutes and 3,000 blocks; Aave user-configuration
    storage slot 53) and regenerate the commitments; register the identity
    through governance; re-qualify the successor epochs; and migrate A666
    off the legacy `sp1-groth16` path (G1).
  - **To confirm:** A1 fails on any Aave debt outside the listed assets but
    does not require the listed debt to be present.
  - The [proposal][proposal] is on main as `81d6cc3a`.
- **Incident:** While pushing the branch, the five branch commits reached
  `origin/main` by mistake for about one minute (`670fa615`). Main was
  restored to `4df53782` with a force push and the three CI runs were
  cancelled. The cause was a repository-wide `push.default=upstream` setting
  left by this lane's worktree script; it is removed. Anyone who fetched main
  in that minute may hold a local main ahead of origin and should reset to
  `origin/main`.
- **Demo:** The [runbook][runbook] covers setup, a five-to-eight-minute demo
  script and troubleshooting. Live mode is not used on stage. The demo port
  is 8796, and the terminal must be at least 120×36. The other lane's demo
  server on port 8795 was left running. The StakeHub wallet is the demo and
  local-custody wallet; `postfiatl1v2` `wallet-web` stays as the
  browser-custody wallet for now.
- **Task Node:** Three tasks were rewarded today:
  - Wallet review: `task_abebbf3af9120aaa4df92978ac535596`, 2.4 PFT.
  - Reserve-proof review: `task_0d9ebb1c222dbb7f15d68d3b3b815dc2`, 3.4 PFT.
  - Reserve-proof repairs branch: `task_4cf7400a9b5aa6079a3f29087a8c6f31`.

  PR #15 and the runbook were small steps without a task. No Task Node action
  was taken for this handoff.
- **Repository and fleet boundary:**
  - **Deployed:** The fleet runs `a666-source-route-20260907`, executable
    `57b0f4d1…`, build `707e006f`, per [Current State][state].
  - **Latest capture:** The latest recorded read-only chain capture is
    [2026-09-22T09:50:50Z–09:51:09Z][fleet]. [Current State][state]'s latest
    full observation is 2026-09-14T11:32:29Z.
  - **Repository:** `main` was `81d6cc3a` before this handoff; the release
    branch is `f59dc07a`, unchanged from the [September 23
    handoff][previous].
  - **Undeployed:** [`combined-devnet-20260923`][candidate] (executable
    `e7bb1afa…`) is unsigned; the combined release, the September 22 build
    and the six P3 repairs are merged but undeployed.
  - **No live changes:** The devnet was not touched today: no host access,
    deployment, signing, key or transaction, and no live probe. Yesterday's
    `needrestart` override on the six validators stands.

## Next decision or action

### This lane's next steps

1. After the publisher decision, sign or rotate per [SIGNING.md][signing] and
   deploy per [DEPLOY-SHEET.md][deploy], outside the 06:00–07:00Z update
   window.
2. After deployment and the signer and input bindings, run the first Z3
   integrated cycle from the [cycle-1 inputs][cycle].
3. After the other lane's read-only route check and small live qualification
   of the wallet bridge, confirm the WB-11 burn transaction id against the
   real burn and lift the "unattended use" hold.
4. StakeHub: work the seven remaining wallet-review P3s and the ten
   [money-path][money-path] P3s.
5. Reserve proof: after his yes on the [proposal][proposal], build the
   successor identity (needs a host with Docker and `cargo-prove`, or his
   machine), set the A666 manifest fields, and prepare the governance
   registration and re-qualification. Until then the branch waits in
   [PR #49][pr49].

### Only the other lane can provide

The other lane was asked again tonight for each item below.

1. **Publisher-key decision — first asked 2026-09-21, three working days:**
   find the key on the other host and sign, or approve a new key generated
   here.
2. **NAVCoin signer keys and the seven [cycle inputs][cycle] — first asked
   2026-09-22.**
3. **StakeHub custody Q1 and archive Q2 — first asked 2026-09-07:** one
   letter each per the [proposal][custody]. His own 2026-09-24 [wallet
   handoff][wallet-handoff] lists them as open.
4. **Reserve-proof successor: adopt yes or no — first asked 2026-09-24:**
   adoption needs his governance registration.
5. **Wallet read-only route check and small live qualification:** his proxy
   configuration and real money under the $5 cap, per [his own
   handoff][wallet-handoff].
6. **Height-915 archive and height-924 custodian — first asked 2026-08-30.**
7. **AI-governance decision for Gate Zero Z2 — first asked 2026-09-03.**
8. **Twelve [inventory][inventory] rows — first asked 2026-09-10.**

## References

- This lane's [September 23 handoff][previous].
- StakeHub: [PR #14][pr14], [PR #15][pr15], [wallet review][wallet-review],
  [his wallet handoff][wallet-handoff], [demo runbook][runbook],
  [money-path review][money-path], and [custody and archive
  proposal][custody].
- Reserve proof: [mechanics review][reserve-review], [successor
  proposal][proposal], and [PR #49][pr49].
- Release branch: [deploy candidate][candidate], [SIGNING.md][signing], and
  [DEPLOY-SHEET.md][deploy].
- [Current State][state], [Z3 plan][z3-plan], and [cycle-1 inputs][cycle].

[previous]: 2026-09-23___dravlic__stakehub_money_path_repaired_deploy_candidate_and_restart_cause.md
[state]: ../status/chain-state-current.md
[z3-plan]: ../plans/active/z3-navcoin-roundtrip-plan.md
[cycle]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-cycle1-inputs-20260922.md
[fleet]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-cycle1-inputs-20260922.md#fresh-fleet-observations
[inventory]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/defect-inventory-20260910.md
[reserve-review]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/nav-reserve-proof-mechanics-review-20260924.md
[reserve-next]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/nav-reserve-proof-mechanics-review-20260924.md#7-what-to-do-next-by-risk
[proposal]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/nav-reserve-proof-successor-proposal-20260924.md
[pr49]: https://github.com/postfiatorg/postfiatl1v2/pull/49
[pr14]: https://github.com/postfiatorg/StakeHub/pull/14
[pr15]: https://github.com/postfiatorg/StakeHub/pull/15
[wallet-review]: https://github.com/postfiatorg/StakeHub/blob/master/docs/review/wallet-bridge-review-20260924.md
[wallet-handoff]: https://github.com/postfiatorg/StakeHub/blob/master/docs/handoffs/wallet-gui-tui-20260924/README.md
[runbook]: https://github.com/postfiatorg/StakeHub/blob/master/docs/runbooks/token2049-demo-20261006.md
[money-path]: https://github.com/postfiatorg/StakeHub/blob/master/docs/review/private-funding-money-path-review-20260923.md
[custody]: https://github.com/postfiatorg/StakeHub/blob/master/docs/review/custody-and-archive-decision-proposal-20260923.md
[candidate]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260923/README.md
[signing]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260923/SIGNING.md
[deploy]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260923/DEPLOY-SHEET.md
