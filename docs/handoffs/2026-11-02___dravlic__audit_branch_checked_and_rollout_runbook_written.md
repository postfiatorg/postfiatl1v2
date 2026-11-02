# Audit branch checked and rollout runbook written

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-02 UTC

## BLUF

Today I followed the client's 2 November handoff on the audit remediation branch. That branch lives in the private archive repository [`postfiatorg/postfiatl1v2-private-archive-20260717`](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717): branch `security/integration-20261102`, tip `cb251534`. Everything I produced today is on its branch `security/integration-check-20261102`, in [pull request #23](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/pull/23) there: 22 commits on top of the client's branch, tip `0efe23ee`. By design none of it is in this repository, and I did not modify the client's branch. The work:

- one independent test for each item of the client's branch, written against today's main `7a91bb30` first and then run against the branch;
- a review of the branch;
- a full CI-equivalent run of the branch on the work server, with four small fixes;
- a local six-validator rehearsal of the upgrade with both binaries;
- read-only pre-checks on a copy of the validator-1 data at height 1161;
- a rollout runbook;
- a milestone document in the usual format.

The details, the records and the register are in the private pull request. I did not touch the devnet or the Ethereum contracts.

## Current state

- **Private pull request #23.** It holds:
  - the independent test set and its register;
  - the review record, the local rehearsal record and the real-data pre-check record;
  - the rollout runbook (`docs/runbooks/l1-security-release-rollout-20261102.md` on that branch);
  - the milestone (`docs/plans/active/l1-security-remediation-milestone-20261102.md` on that branch);
  - four fixes that keep the branch's CI green: the RPC method inventory regenerated for two new methods, five documentation anchors, one policy test, and four long tests moved to a release-mode CI job;
  - one PR comment per round.

  None of this is in this repository.
- **CI-equivalent run on the work server.** Every job passes with the fixes above, except one long single-threaded node test that was still running when I wrote this. That test is now assigned to the release-mode job. The `arc-proof-identities` job will run on a public PR, because the branch changes a program lock file. It takes about 90 minutes, and I did not run it here.
- **Local rehearsal.** Consensus keeps certifying with five of six validators in every mix of old and new binaries. A mixed fleet stalls on queued certified sends, so the runbook leans towards a coordinated stop at one height rather than an upgrade host by host.
- **Real-data pre-checks.** On the validator-1 copy at 1161, the seven verifiers give byte-identical reports with both binaries. Two issues in the new build showed up on real data. They are recorded in the pre-check record and listed in the milestone as open items.
- **Two read-only checks** that the client's handoff left open, a pfUSDC supply reconciliation and an inventory at heights 929, 1050 and 1161, are closed in the private records.
- **Branches and remotes.**
  - The remote `audit`, the private archive repository, is configured in the server checkouts under `~/repos/postfiatl1v2`.
  - `security/integration-check-20261102` is pushed to the private archive repository only. These branches exist only on the work server, with their upstreams unset on purpose:
    - `security/repro-{a,b,c}-20261102`;
    - `security/independent-check-20261102`;
    - `security/rollout-runbook-20261102`;
    - `security/extra-tests-20261102`;
    - `security/gate-fixes-20261102`.
  - `ci/slow-tests-release-job-20261102` holds an unfinished CI-time measurement.
- **Deployment record.**
  - **Last observed fleet state:** unchanged, as recorded in [Current State](../status/chain-state-current.md).
  - **Deployed binary:** still `vote-records-20261030`.
  - **Repository:** `origin/main` was at `7a91bb30` before this handoff, and main received no other commit today.
  - **Merged but undeployed work on main:** none.
  - **Live probe in this session:** none. I made no host login, no RPC write and no deployment, and on Ethereum I made only read-only calls.
- **Task Node.** I made three requests in the morning, before the private-only rule was set: one task is awaiting review with hash-only evidence, and the other two were refused. I took no Task Node action after that.

## Next decision or action

- **Mine, next:**
  1. Run the same pre-checks and the replay on final-tip copies of all six validators. Add the live reads the copy could not give: a mixed-binary block, connection counts, outbox counts and timers.
  2. Fix the two real-data issues on the check branch, with failing tests first.
  3. Run the rollout day under the runbook's option B, once the client approves that option.
- **Mine, after:**
  - one more test noted in the register;
  - the parked transport work on the client's branch;
  - the CI-time measurement on main (`ci/slow-tests-release-job-20261102`).
- **The client's, from his 2 November handoff (first asked 2026-11-02):** the owner decisions and approvals listed there, including the choice of rollout option (the runbook's lean is option B) and the signatures for the two full-fleet actions on the day. The full list is in the private pull request.
- **The client's older items, unchanged:**
  - the two issuer signatures (2026-09-25);
  - the reserve-proof successor registration (2026-09-24);
  - the `ETHEREUM_MAINNET_RPC_URL` secret (2026-09-28);
  - the W-1 and D15b items (2026-10-29);
  - the ERC-20 slice decisions (2026-10-26).

## References

- The client's 2 November handoff, in the private archive repository (`docs/review/l1-security-HANDOFF-20261102.md` on his branch).
- [Private pull request #23](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/pull/23).
- [Vote records release live and genesis external lane merged](2026-10-30___dravlic__vote_records_release_live_and_genesis_external_lane_merged.md) (2026-10-30).
- [The 29 October Task Node wallet handoff](2026-10-29___goodalexander__tasknode_wallet_findings_for_the_l1.md) (goodalexander).
- [Current State](../status/chain-state-current.md), unchanged today.
