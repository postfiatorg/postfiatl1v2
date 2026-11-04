# Security candidate rebuilt with every repair and token amendments merged

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-04 UTC

## BLUF

Today I rebuilt the private release candidate as one exact build that carries every repair. It is reproduced byte for byte and replayed on all six validators' data copies. The final executables are node `239458a48b4b033b03acfa25579df87b9b0b1c34ffc61fcf05413fddfef9fdfb` and migration helper `9e16801cd8eca3889f910cdcefd744571acc6f4a1579705a4f30cf20c671843d`, built from source `a8036e13d` on the private candidate branch. Its records are the [packet page](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/review/l1-security-release-candidate-20261104.md) and the [deployment packet](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/tree/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/deployments/l1-security-20261104) on that branch, in [private pull request #24](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/pull/24). On the private side:

- I rehearsed the rollout tool, the three activations and the failed-apply rollback again with the new build.
- I wrote a keyed-rehearsal packet for the owner's hosts.
- The status read that used to sign a checkpoint vote no longer signs.
- The governance amendment path now requires the validators' threshold on every route. The live chain's history is fully signed throughout.

On public main, the token's pre-genesis list gained four merged pieces, each after a correctness review: the lock contract's minimum, the pruning of release records, the governed release cap and the governed committee move. The proof-program lines were corrected, the next-release plan now points at the rebuilt base, and public CI is green again.

## Current state

The private pages below are at `a54f323e9` in the private repository.

- **The candidate.** `a54f323e9` on the private candidate branch, fast-forwarded into PR #24 today. The executables are built from `a8036e13d`; the two commits after it are the packet records. The morning's builds `624763f6…` / `113e3dad…` are superseded. The replay ran 84 verifier runs on all six copies, and every OLD/NEW pair is byte-identical. The snapshot round trip exported 439,193,843 B including the 25 MB batch archive and imported at 1168, and the seven verifiers are identical on the imported copy. These are rehearsals on copies, not a deployment.
- **The rehearsals.** [Rehearsal record](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/review/l1-security-rehearsals-20261104.md): the halt tool's happy path, failures A and B, §5 on real data, and §5–§7 on the local fleet. No defect. I fixed the tool's two reporting gaps: it now records the cause of a failed apply and the restored anchors.
- **The keyed packet.** [Keyed-rehearsal runbook](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/runbooks/l1-security-keyed-rehearsal-20261104.md), with [its inputs](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/tree/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/review/l1-security-keyed-rehearsal-inputs-20261104). The deployment packet holds the README, DEPLOY-SHEET, inventory, node builds and units. The runbook gained the membership gate, the pre-activation snapshot note and the post-drain check.
- **The status read.** It no longer signs; the vote moved to a gated operator method. In the live check at 07:14 UTC, the six votes collected overnight cannot be anchored today, because a checkpoint is already anchored at block 1130. They remain signatures in the collector's cache.
- **The governance amendment threshold.** [Threshold record](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/review/l1-security-amendment-threshold-20261104.md). Live admission already required every validator; ratification, batch evidence and archived execution now do too. The live reads show all 34 past amendments signed 6 of 6.
- **The owned-lock difference on validator-0.** [Explained here](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/review/l1-security-owned-lock-divergence-20261104.md): a client retry in July. The migration never reads the lock files, and the owner of account `pf8c1d625c…` can clear it with one transaction.
- **The reproduction tests.** [Consolidated check](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/review/l1-security-consolidated-check-20261104.md): 19 of 19 unchanged.
- **The wrapped-token options paper** for the owner: [paper](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/review/l1-security-wrapped-token-path-20261104.md). Minting was still switched on at 06:47 UTC.
- **Public main today** (tip before this handoff `c43eaa27a`):
    - `2574c1cbe`: the CLI method and the contracts CI lint ([Python RPC client](../runbooks/python-rpc-client.md));
    - `61027a7fa`: the runner disk fix;
    - `d3611b9aa`: the proof-program correction;
    - `5fc1e2374`: the [release plan](../plans/active/next-validator-release-plan-20261115.md);
    - `f53e7457c`: pruning; `8351ac551`: the lock minimum; `f2dedd257`: the release cap; `be09a97f1`: the committee move ([genesis external lanes](../architecture/genesis-external-lanes.md));
    - `76d0935e5` / `264d0f6e7` / `7527df06a`: the ticks in the [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md);
    - `59b1d8a18` / `739d49527`: the W-1 S7.1 readiness custody facts (store activation, signer type, key-file custody) in the [W-1 milestone](../plans/active/user-proven-shielded-actions-milestone.md); the hardened signer stays open;
    - `db2de7c0b`: the six-validator rehearsal of both amendment rounds at five of six with the real CLI ([qualification record](../listings/coinbase/pft-erc20-release-path-qualification-20261103.md)).
- **Deployment record.**
    - Last observed fleet state: unchanged, as in [Current State](../status/chain-state-current.md) (2026-10-30T11:21Z, tip 1168).
    - Deployed binary: still `vote-records-20261030` (`87cfe797…`, source `ae70d66d`).
    - Repository: `origin/main` at `c43eaa27a` before this handoff; the private candidate branch at `a54f323e9`.
    - Merged but undeployed: everything on public main above, carried by the [next validator release plan](../plans/active/next-validator-release-plan-20261115.md), and the private candidate.
    - Live probe: unsigned status reads only, including the 07:14 UTC check. Devnet was not touched: read-only data copies, no deployment, no signing.
- **Task Node.** 20 tasks went through the lifecycle today (request, accept, evidence, verification). The full ids are in the private pages' records for the private work, and in the milestone lines for the public work.
    - Rewarded: `task_053287ca…`, `task_064a837b…` (2.25 of 3.5), `task_13cfdee9…`, `task_1422a896…`, `task_17b1a47e…`, `task_1c11afe2…`, `task_247a98f8…`, `task_3c78d9f7…` (2.25 of 3), `task_415370ce…` (2.25 of 3.5), `task_5b77fb91…`, `task_7cdf6f47…`, `task_bc05055d…`, `task_ca92f268…`, `task_f78f76d3…`.
    - Awaiting review at the close: `task_23632372…`, `task_2d63cec2…`, `task_cfe65aa8…`, `task_e03fa992…`, `task_fa60ddd9…`.

## Next decision or action

- **Ours, first:**
    1. The rollout day, under the [rollout runbook](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/runbooks/l1-security-release-rollout-20261102.md) with the [deploy sheet](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/deployments/l1-security-20261104/DEPLOY-SHEET.md), once the keyed rehearsal is done.
    2. The Base checkpoint source's own qualification (P2.5's last line).
    3. The W-1 hardened validator signer (S7.1's open item).
    4. The outside [pull request #185](https://github.com/postfiatorg/postfiatl1v2/pull/185), which refuses a genesis lane whose window cap rounds to zero. I read it today and approved it with one wording fix; it is 26 commits behind main without conflicts. Next: its correctness review and merge, and the approval of its CI run.
    5. The stale wallet bundle hash table in `wallet-web/README.md` on the private branches.
- **The client's, with the date first asked:**
    - the keyed rehearsal on his hosts, the six migration signatures, the migration-trust descriptor and the rollout go (2026-11-02/03/04);
    - pause minting on the Ethereum controller `0x9A02…` (2026-11-02; still on at 06:47 UTC today);
    - destroy the six signed checkpoint votes in the collector's cache, or anchor deliberately (2026-11-04);
    - the wrapped-token path A or B, with its inputs (2026-11-03/04);
    - the reachable host of the audit/ops machine with the old prover (2026-11-04);
    - the review of the repair branches' two policy questions (2026-11-03);
    - stop the old proving service, and the faucet-key decision (2026-11-02);
    - the two issuer signatures (2026-09-25 / 2026-10-01);
    - the reserve-proof successor registration (2026-09-24);
    - the `ETHEREUM_MAINNET_RPC_URL` secret (2026-09-28).

## References

- [Private pull request #24](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/pull/24) and the private pages linked above.
- The client's overnight handoffs on the private candidate branch: [security consolidation](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/handoffs/2026-11-03___codex__security_consolidation.md) and [five-hour security verification](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/a54f323e97ca638e0c1ac142a8571b79f9d08ef4/docs/handoffs/2026-11-04___codex__five_hour_security_verification.md).
- [ERC-20 on Base milestone](../plans/active/pft-erc20-base-listing-milestone.md) and [genesis external lanes](../architecture/genesis-external-lanes.md).
- [W-1 milestone](../plans/active/user-proven-shielded-actions-milestone.md).
- [Next validator release plan](../plans/active/next-validator-release-plan-20261115.md).
- [Security release candidate qualified on real data and token release path merged](2026-11-03___dravlic__security_release_candidate_qualified_on_real_data_and_token_release_path_merged.md) (2026-11-03).
- [Current State](../status/chain-state-current.md), unchanged today.
