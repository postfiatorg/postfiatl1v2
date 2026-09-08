# Safety closeout — 2026-09-07

## Plain-English result

- **CI:** reviewed the stale proof inputs rather than blindly updating hashes;
  fixed the inventory and two downstream checks it was hiding. All public-tree
  job commands now pass locally. No check was removed or weakened.
- **StakeHub #8:** repaired the policy bypass, live-by-default scripts and
  private-file cleanup failure paths in an isolated branch. Addressed or
  explicitly dispositioned every remaining review finding. **501 offline tests
  passed, four existing native opt-ins skipped.**
- **Task Node design:** a separate amendment passed its first full 15-review
  gate at **90.53/100** and was locked immediately. The original proposal/MVP
  are unchanged. The V2 implementation journal is written; V2 itself is not
  implemented or activated.
- **PR #38:** recorded **A — carry in the next qualified routine release;
  leave activation unscheduled**. This is a composition decision, not release
  or capital-use authorization.

Nothing was committed, pushed, merged, deployed, registered or activated. No
operator wallet was opened, no live validator/chain endpoint was contacted,
and no capital-moving transaction was signed or submitted. GitHub and Task
Node were used for state/evidence; OpenRouter was used for the required text gate.

## Where the work is

| Work | Local artifact |
| --- | --- |
| Proof review, eleven original mismatches and extra repairs | [Proof-input review](../review/proof-input-review-20260907.md) |
| New UNL policy design and explicit limits | [Locked amendment V2](../governance/tasknode-unl-amendment-v2-20260907.md) |
| Exact scored hash and all model scores | [Research lock](../review/tasknode-unl-amendment-v2-lock-20260907.md) |
| Future implementation, CLI and user report | [V2 milestone journal](../plans/active/tasknode-unl-amendment-v2-milestone.md) |
| Dormant-code composition decision | [PR #38 sheet](../governance/yolo-deploy-decision-20260907.md) |
| StakeHub fixes/dispositions | `/home/postfiatchad/repos/StakeHub-safety-20260907/docs/review/pr8-safety-repairs-20260907.md` |

L1 checkout remains on `main` at `d351353e57b295368450a57866ace17b5e1ce6ad`
with the reviewed local diff. StakeHub changes are on
`fix/pr8-safety-20260907`, based on PR #8 head
`8f6f27cf66896b48f9a645e995a1288fc5f7218d`, in the isolated worktree above.
The original StakeHub checkout and its pre-existing `AGENTS.md` edit are untouched.

## Verification, not deployment evidence

### L1/proof boundary

- Full `public-tree-hygiene` command sequence passes locally, including source
  inventory, both A666 checks, secret/runtime-default scans and redaction gates.
- Reserve CLI/types: **125 passed, three existing ignored SP1 tests**.
- A666 readiness/archive Python regressions: **20 passed**.
- All ten fuzz targets pass at their unchanged ten-second default budgets;
  the nested lockfile is reviewed and now protected against silent mutation.
- Strict MkDocs, reserve formatting and `git diff --check` pass.
- Historical qualification is integrity-checked from an immutable Git archive,
  not relabeled as freshly generated proofs. A666 remains **0/6 qualified**;
  StakeHub remains **not deprecated**. No Orchard/whole-L1 suite was rerun.

### StakeHub boundary

- Focused changed-surface regressions: **119 passed**, six subtests.
- Broader shielded-exit/NAVCoin/agent gate: **501 passed, four skipped**, six
  subtests. Python network guard restricts Internet sockets to loopback;
  syscall tracing found twelve Internet socket connects, all loopback.
- Strict MkDocs, changed-module compilation and `git diff --check` pass.
- Private egress no longer accepts chain-height movement as success. Actual
  mocked upload, proposer-change, rejected-round, ambiguous-resume and cleanup
  failures are covered. Copied release/profile substitution is rejected before
  promotion/signing. Exact venue amount serialization is checked.
- Existing checked-in recovery evidence remains unchanged except its README
  correction: the archive **is in Git**. Its root manifest has sixteen external
  L1 PR #39 dependencies, not sixteen fabricated local files. Publication and
  retention approval remain a manager decision.
- Remote-key scripts still have a remote-custody boundary; cleanup cannot erase
  snapshots or defeat SIGKILL, machine loss or a compromised host. Live use is
  not qualified. The irreversible referral operation and proof-downgrade
  `pfeth_nav_par` entrypoint are disabled pending separately reviewed replacements.

The ignored `.tih/` directory holds raw logs, the full StakeHub patch, reviewed
proof pin hashes, Task Node evidence receipts, and the real harness database.
The new/changed text was scanned using existing secret rules; no findings.
This is not a new independent whole-history audit.

## What remains before merge or release

1. Review and publish the two local patches through the normal PR process.
   These changes are not yet on GitHub.
2. Require green CI on the **exact resulting commit**. At the final check,
   Rust run `34129297733` still had `test` running (`check` passed).
   Security run `34129297634` had six passing jobs and the known
   `public-tree-hygiene` failure; its source did not contain these local repairs.
   Do not describe remote CI as green.
3. Re-review StakeHub #8 at the patched head before merging. Keep archive
   publication, remote custody, canary, rollback and any capital use as separate
   approvals.
4. Implement/evaluate V2 only under future Task Node-governed work. Hidden
   same-key account sales remain undetectable by the supplied evidence; V2's
   replacement edge policy and honest-admission liveness still need experiments.
   A favorable text score is not an implementation/security result.
5. Before a routine validator release, satisfy the existing CI/canary/rollback
   gates. Do not schedule YOLO activation or register programs as part of this
   closeout.

## Task Node ledger

- Proof reconciliation: `task_bdb4280daf0d3699d7382b650b81e719`.
- StakeHub safety fixes: `task_e8a897157d362e3aab353e42fd80acc4`.
- UNL research lock: `task_c26d27a9e112a7ad1c27682b95f0790e`.
- Documentation-only V2 milestone: `task_fbdc96a15de43ad360a81c25173d8d5b`.

All four tasks reached the server-reported **Rewarded** state after initial
evidence and the requested verification response. Proof verification included
the eleven exact before/after pin identities; StakeHub verification included
the actual agent-bypass diff and fresh 46-case mock test output; research
verification matched the scored file hash; milestone verification submitted
the actual document. No initial-evidence receipt was treated as completion.
