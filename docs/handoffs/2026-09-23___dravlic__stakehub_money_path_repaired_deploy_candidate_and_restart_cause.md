# StakeHub money path repaired, deploy candidate, and restart cause

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-23 UTC

## BLUF

Today this lane did six things. (1) StakeHub's money path is repaired on
master: recovery defects R1–R3 from the [September 22 review][stakehub-review]
are merged ([PR #11][pr11], `36edc84`). Two correctness reviews of the
funds-moving code ([money-path review][money-path]) found **7 P1 / 5 P2 /
10 P3**. Every P1 and P2 is repaired with reproduce-first regression tests
([PR #12][pr12], `1a1bb43`; [PR #13][pr13], `6f4ff1f`). StakeHub is still
**not fit for live funds** until the P3s, the [custody and archive
answers][custody] and qualification of the exact release are done.
(2) [`combined-devnet-20260923`][candidate] is the unsigned deploy candidate,
built from the qualified September 22 build. One deployment now replaces two;
it remains blocked on the publisher-key decision. (3) The September 11 restart
was Ubuntu's unattended security upgrade plus `needrestart`
([Current State][state], 2026-09-23 note). The validators still install
security updates but no longer restart PostFiat services automatically.
(4) The [Z3 evidence baseline][z3-manifest] and first-cycle decisions are
recorded. (5) The six P3 findings from the September 21 review are repaired
for the release after next. (6) Both Task Node verification requests open
since September 8 are answered and rewarded.

## Current state

- **StakeHub master (`6f4ff1f`):**
  - `77c8993` records the other lane's September 23 [sprint handoff][sprint]
    note, committed and pushed by this lane.
  - `36edc84` (PR #11) fixes recovery. Cleanup host and paths are journaled
    before upload and cleared only after confirmed cleanup; resume must finish
    cleanup and never resubmits. Route-activation resume accepts validators at
    or above the activation height. Private egress records an attempt before
    launch and decides completion from the stored result. It adds 14 tests;
    the focused suite passed 152.
  - `5513ee6` adds the [custody and archive proposal][custody]: one-letter
    answers, with this lane leaning a / a.
- **Money-path review part 1** ([PR #12][pr12] → `1a1bb43`) covers
  `private_swap_egress.py`, `shielded_exit_executor.py`,
  `shielded_exit_funding.py` and `shielded_note_return_owner.py`. The review
  found 3 P1 findings: an unverified proof binding; success without an
  accepted receipt; and a swap accepted without a receipt. It found 3 P2
  findings: timed-out egress and timed-out swap finalized as rejected while
  unknown, and a resume that resubmitted a completed run. It adds 11
  regression tests and moves three fixtures to the real report shape.
  MP-07–MP-11 (P3) are recorded.
- **Money-path review part 2** ([PR #13][pr13] → `6f4ff1f`) covers
  `submit_pftl_bridge_out`, `submit_transparent_nav_roundtrip` and
  `scripts/pfeth_bridge_out.py`. It found 4 P1 findings:
  - **MP-12:** a retry burns again.
  - **MP-13:** `bridged_out` is reported without an accepted burn and
    settlement.
  - **MP-14:** success is inferred from exit code 0.
  - **MP-17:** an unconfirmed burn is signed and submitted again on resume.

  It found 2 P2 findings:
  - **MP-15:** a timed-out round trip mints again.
  - **MP-16:** bridge-out has no R1 cleanup journal.

  It adds 9 regression tests; the focused run had 323 passes and 1
  environment failure. Accepted-result checks and fixtures match the node's
  `postfiat-certified-asset-ops-report-v1` producer. MP-18–MP-22 (P3) are
  recorded. **Separate risk:** node builds from current `main` or the release
  branch no longer contain `nav-roundtrip-live-demo` (removed by `b19ce4c8`,
  2026-08-01), so bridge-out needs a binary from older source.
- **StakeHub merge basis:** Each PR was merged after the full suite ran on
  this server. PR #11 had 5,026 passes, #12 had 5,023, and #13 had 5,046.
  Each run had 88 skips and the same 84 failures seen on unchanged master:
  63 browser tests that cannot start Chromium here and 21 environment-bound
  tests. These are local test results, not live-funds evidence.
- **Deploy candidate** (release branch, `129d70ff`):
  - **Build:** [`deployments/combined-devnet-20260923/`][candidate] has
    source tip `d75356f6`, build source `1a0989ad`, and executable
    `e7bb1afa…e9e4b1`. That executable was verified by hash in all four
    `~/.cache` copies, not rebuilt.
  - **Rollback and local checks:** Rollback is `a666-source-route-20260907`
    / `57b0f4d1…`. Stage files match 20260921 except for the release ID.
    Local preflight passed on all six inputs.
  - **Status:** [SIGNING.md][signing] has both routes, and
    [DEPLOY-SHEET.md][deploy] forbids starting in the 06:00–07:00Z update
    window. Status is `UNSIGNED_PREPARATION_ONLY`.
    [`combined-devnet-20260921`][fallback] is retained as the fallback.
  - **CI:** Green CI at `d75356f6` and `7f64c12a` closes the
    [qualification packet][qualification]'s two deferred rows.
- **Six P3 repairs** (`0b9c6715`, `f59dc07a`) cover PFV-01, BRW-04/05/06 and
  NOD-03/04, with tests and a [campaign log section][qa-p3]. They land after
  the qualified tip and ship in the release after next. The deploy candidate
  stays pinned to `e7bb1afa…`.
- **Restart cause** (`b8b5130c`, `ad7ea8ed`): On September 11, from
  06:01–06:47Z, `apt-daily-upgrade` ran `unattended-upgrade` on each host,
  upgrading glibc (8.8 → 8.9) and `python3.12`. `needrestart` then restarted
  the linked services. No person acted. On September 22 and 23, the same
  mechanism restarted only the NAVCoin RPC proxies. Host times and packages
  are in [Current State][state].
- **Devnet change (configuration only):** At 09:13Z,
  `/etc/needrestart/conf.d/50-postfiat-manual-restart.conf` was installed on
  all six validators. The file is identical on every host (SHA-256
  `305836ca…`) and sets `override_rc` 0 for `postfiat-*` and `navcoin-*`.
  - **Checks:** It was tested on a temporary copy first; afterward,
    `needrestart` parsed it with exit 0.
  - **Service state:** Nothing restarted. All PostFiat services kept their
    September 11 start times.
  - **Rollback:** Delete the file.
- **Z3** (`5a9405f4`): The [G0 manifest][z3-manifest] hashes 133 files: 23
  in-repo and 110 server-only. It found no secret or forbidden field; both G0
  items are ticked.
  - **G1 decisions:** The [plan][z3-plan] records the qualified September 22
    build as the lineage and uses the Arc pair as read back on September 22.
    The route is the existing A666 primary route. The cap is 2,000,000 source
    atoms per cycle. The seven-day window opens on the first live cycle's UTC
    day. Authorization covers testnet/devnet only.
  - **G5 decision:** The nine proposed stage limits are adopted.
  - **Open G1 item:** The "funded wallet and signing flow" item stays open
    with the other lane. These are plan decisions only; no cycle has run.
- **Task Node:** Six tasks were rewarded today:
  - R1–R3: `task_7e50e0968eb87457ebf469edae45102f`, 1.5 PFT.
  - Deployment directory: `task_45aeb46fa6e7e87f6519781a1934c1f4`, 1.5 PFT.
  - Money-path review part 1: `task_e894b962dfebf525d753ac6cbe910a96`,
    2.45 PFT.
  - Money-path review part 2: `task_d5790cecf70ba2d571675fc53d39f0a9`,
    1.6 PFT.
  - September 8 verification section A:
    `task_aece75f855b2633598b89dffd266a024`, 3 PFT.
  - September 8 verification section B:
    `task_91a089bd14dedf6b6fb681fae7e49561`, 3.5 PFT.

  No Task Node action was taken for this handoff.
- **Dependabot alert #2** (postcss ≤ 8.5.22 in
  `wallet-web/package-lock.json`) is stale: the lockfile already pins 8.5.26.
- **Repository and fleet boundary:**
  - **Deployed:** The fleet runs `a666-source-route-20260907`, executable
    `57b0f4d1…`, build `707e006f`.
  - **Latest capture:** The latest recorded read-only chain capture is
    [2026-09-22T09:50:50Z–09:51:09Z][fleet]: all six validators at height
    1020 with matching tip and root. [Current State][state]'s latest full
    observation is 2026-09-14T11:32:29Z.
  - **Today's host access:** It covered upgrade logs, the journal, service
    start times and the configuration file above. It made no new
    height/tip capture.
  - **Repository:** `main` was `5a9405f4` before this handoff; the release
    branch is `f59dc07a`.
  - **Undeployed:** The combined release, the September 22 build and the six
    P3 repairs are merged but undeployed.
  - **No live changes:** Nothing was deployed or signed. No key was
    generated, no transaction was sent, and no live probe was performed in
    this handoff-writing session.

## Next decision or action

### This lane's next steps

1. After the publisher decision, sign or rotate per [SIGNING.md][signing].
   Deploy `combined-devnet-20260923` per [DEPLOY-SHEET.md][deploy], with these
   controls:
   - Stay outside the 06:00–07:00Z window.
   - Take fresh observations before and after.
   - Take a signed canary backup.
   - Deploy one validator at a time.
   - Keep the documented rollback path ready.
2. After deployment and the supplied signer, input and prover bindings, run
   the first Z3 integrated cycle from the [cycle-1 inputs][cycle]. Re-read
   every value marked "after deployment."
3. StakeHub: work the ten recorded P3 findings and the
   `nav-roundtrip-live-demo` binary question for bridge-out. After the custody
   and archive answers, qualify the exact release.
4. Ship the six P3 repairs in the release after next.

### Only the other lane can provide

The other lane was asked again tonight for each item below.

1. **Publisher-key decision — first asked 2026-09-21:** either find the key
   on the other host and sign per [SIGNING.md][signing], or approve a new key
   generated here. That rotation was rehearsed on 2026-09-22.
2. **NAVCoin signer keys and seven cycle inputs — first asked 2026-09-22:**
   supply [rows 4, 8, 11, 12, 13, 16 and 17][cycle] and the signer table
   values.
3. **StakeHub custody Q1 and recovery archive Q2 — first asked 2026-09-07:**
   answer each with one letter per the [proposal][custody]. This lane leans
   a / a.
4. **Height-915 quarantine archive and height-924 custodian authorization —
   first asked 2026-08-30.**
5. **AI-governance decision for Gate Zero Z2 — first asked 2026-09-03.**
6. **Twelve [inventory][inventory] rows needing an operator decision or a
   named live environment — first asked 2026-09-10.**

The September 11 restart question is closed and needs no answer.

## References

- The other lane's [September 22 handoff][overnight]; this lane's
  [September 22 handoff][previous].
- StakeHub: [PR #11][pr11], [PR #12][pr12], [PR #13][pr13],
  [money-path review][money-path], [custody and archive proposal][custody],
  [sprint handoff][sprint], and the [September 22 review][stakehub-review].
- Release branch: [deploy candidate][candidate], [SIGNING.md][signing],
  [DEPLOY-SHEET.md][deploy], [fallback release][fallback],
  [qualification packet][qualification], and
  [September 23 P3 repairs][qa-p3].
- [Current State][state], [Z3 G0 manifest][z3-manifest], [Z3 plan][z3-plan],
  and [cycle-1 inputs][cycle].

[overnight]: 2026-09-22___codex__overnight_release_and_navcoin_preparation.md
[previous]: 2026-09-22___dravlic__key_custody_next_release_qualified_and_cycle_inputs.md
[stakehub-review]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/stakehub-safety-repairs-review-20260922.md
[state]: ../status/chain-state-current.md
[z3-manifest]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-g0-evidence-manifest-20260923.md
[z3-plan]: ../plans/active/z3-navcoin-roundtrip-plan.md
[cycle]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-cycle1-inputs-20260922.md
[fleet]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-cycle1-inputs-20260922.md#fresh-fleet-observations
[inventory]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/defect-inventory-20260910.md
[pr11]: https://github.com/postfiatorg/StakeHub/pull/11
[pr12]: https://github.com/postfiatorg/StakeHub/pull/12
[pr13]: https://github.com/postfiatorg/StakeHub/pull/13
[money-path]: https://github.com/postfiatorg/StakeHub/blob/master/docs/review/private-funding-money-path-review-20260923.md
[custody]: https://github.com/postfiatorg/StakeHub/blob/master/docs/review/custody-and-archive-decision-proposal-20260923.md
[sprint]: https://github.com/postfiatorg/StakeHub/blob/master/docs/current-sprint/shielded-funding-execution-20260911.md
[candidate]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260923/README.md
[signing]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260923/SIGNING.md
[deploy]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260923/DEPLOY-SHEET.md
[fallback]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/README.md
[qualification]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/release-repair-20260922/README.md
[qa-p3]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/qa-campaign-20260921.md#september-23-p3-repairs
