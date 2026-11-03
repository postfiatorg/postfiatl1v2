# Security release candidate qualified on real data and token release path merged

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-03 UTC

## BLUF

Today I took the client's 3 November developer action plan for the next validator release through its steps 2–5, as far as this server allows. That work is the release candidate's qualification. It lives in [private pull request #23](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/pull/23) on `postfiatorg/postfiatl1v2-private-archive-20260717`, and none of it is in this repository. The candidate's packet draft is `docs/review/l1-security-release-candidate-20261103.md` on that branch. On the private branch:

- **Tests fit the runner again.** The branch's tests fit the runner's time limit again after two changes: a disk step, and a split of the slow node tests into parallel release-mode jobs.
- **One exact candidate build.** It is recorded and reproduced byte for byte. Source `7cb736759`, node `ec59fc53…`, migration helper `54ae6817…`.
- **Real-data replay.** The candidate replays all six validators' real data, using key-free copies at height 1168. Its reports are identical to the running build's, and the snapshot restore works on real data.
- **Coordinated-halt upgrade tool.** I rehearsed it on the six copies with real processes and injected failures. I fixed four defects in it and re-ran the pass clean.
- **The three governance activations.** I rehearsed all three:
  - the ingress activation works as written;
  - the owned-history migration's preparation needed one node fix on real data, and it now gives identical results on all six copies.
- **Ethereum successor contracts.** I dry-ran them on a local fork. The dry run found that the existing wrapped token is locked to its current controller, which changes the plan on that side.
- **Wallet enrollment.** I built the wallet's enrollment with the new proving service, then hardened it after a review.

On public main:

- **ERC-20 inbound release path (P2.4):** built, reviewed in two rounds and merged (`8f29aaccb`).
- **Options paper:** locked at the harness gate (`49aeca2be`).
- **Lock deployment script:** now binds the finality verifier's anchor and configuration (`1e2cc55f8`).
- **Intermittent test failures:** I fixed main's two, one of them a real race in the wallet's round service (`5eb305627`).
- **Next release plan:** points at the separately qualified release (`ccf86448a`).
- **Wallet proxy certification:** goes through the validator-owned path by default (`85340e84a`).

## Current state

- **Private branch `security/integration-check-20261102` (PR #23).**
  - **Tip:** `9cf33b301` at 13:10 UTC, from `git ls-remote audit security/integration-check-20261102`.
  - **Today's commits, in order:**
    - CI disk step: `2839716dd`;
    - rollout tool fixes: `3d201aabf`, `7345fd4ab`, `b83018a6e`, `3964de9a3`;
    - packet draft `2d239a0bf`, milestone `fb50b2c60`, runbook `4147af30b`;
    - merges: the pruned-snapshot tests `d6e60f725`, the wallet enrollment `daba9dbb9`, the successor fixes `38fb5a29b`;
    - first test split: `afdadb31c`;
    - migration preparation fix `dcace3a7b`, runbook commands `6f467b634`, rehearsal record `7cb736759`;
    - final candidate record: `03fd34153`;
    - second test split: `13bc85323`;
    - wallet hardening: `c69f7c7ee`, `369b68415`;
    - archive backfill record: `047023fb4`, `8bd350bd9`;
    - test harness guard: `8f94097f2`;
    - wallet signers: `a330cba80`, `c53930dea`;
    - one-run proxy flow test: `9cf33b301`.
  - **Not yet pushed:** the packet update, and any fix for the intermittent node tests.
  - **Evidence pages on that branch:**
    - `docs/review/l1-security-release-candidate-20261103.md`;
    - `docs/review/l1-security-activation-rehearsals-20261103.md`;
    - `docs/review/l1-security-ethereum-successor-inputs-20261103.json`.
- **The candidate.**
  - **Source:** `7cb736759`.
  - **Node:** `ec59fc53bbe936a1c15ba25c5cf2ee686af55064e94435af0e8eab4d079ab751`.
  - **Migration helper:** `54ae6817f9de43dd055812c26b43ab5868e80e2a44cff707fa9b083f24b2631c`.
  - **Reproduced:** by a second clean build.
  - **Replay on the six copies:** byte-identical with the running build `87cfe797…`.
  - **Full snapshot round trip:** on validator-1's copy, 439 MB including the 25 MB archive.

  These are rehearsals on copies, not a deployment.
- **CI on the branch at 13:10 UTC.**
  - `13bc85323`: fully green (`rust-ci` 12:15, `product-security-ci` 12:16 UTC).
  - `8bd350bd9`: `product-security-ci` green. `rust-ci` was red on the node job, through 16 intermittent node tests that passed on the commit before.
  - `8f94097f2`, `c53930dea`, `9cf33b301`: still running.
  - Before the two splits, the node test job was cancelled at its 120-minute limit.
- **Open items of the candidate.**
  - The keyed rehearsal of the signer checks, and the six signing runs of the migration, on the client's hosts.
  - An independent migration-trust descriptor.
  - For the Ethereum successors:
    - the wrapped-token decision (`controller_locked = true` on `0xeE4C92eD…`);
    - the controller type;
    - a new route id;
    - an approved receipt-v2 verifying key.
  - The wallet app's compiled signing module lacks the new ingress signer.
  - One failing node test on the branch, from before today. A stale test binary causes it, not the product, and a harness guard (`8f94097f2`) now refuses such a binary.
  - The client's own transport work on his branch.
- **Repair branches.** Five repair branches on the private repository are waiting for the client's review, not merged. Their tips are `4c4d3821e`, `5b7316a32`, `a6748ab3a`, `1c4004231` and `d893e9f83`. Their names, and the two policy questions that come with them, are on the private repository.
- **Public main.**
  - **Tip before this handoff:** `b26de17a9`.
  - **Test fixes:** `f03dd41a8`, `094f180df`, `5eb305627`, in [private action admission](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/node/src/private_action_admission.rs). Tests are green on `5eb305627`.
  - **Lock deployment checks:** `b340fd5dd`, `1e2cc55f8`, `8ec313e19`, in [`DeployPFTLock.s.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/script/DeployPFTLock.s.sol).
  - **Options paper lock:** `49aeca2be`, `fc6f9a061`, in [the options paper](../listings/coinbase/pft-erc20-representation-options-20261026.md).
  - **Certification path:** `85340e84a` ([web wallet](../wallets/web-wallet.md)).
  - **Release plan:** `ccf86448a` ([next validator release plan](../plans/active/next-validator-release-plan-20261115.md)).
  - **Release path:** merge `8f29aaccb`, milestone tick `9ae46a531`. See [the ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md) and [genesis external lanes](../architecture/genesis-external-lanes.md).
  - **Release path qualification run record:** `4f5aeb763` ([record](../listings/coinbase/pft-erc20-release-path-qualification-20261103.md)). It was a local run on one work server, not the live devnet, and it does not tick P2.5.
  - **Genesis-with-lanes init:** `e80ee8f9f`.
  - **Lane amendment refusals:** `b26de17a9`.
- **Deployment record.**
  - **Last observed fleet state:** unchanged, as recorded in [Current State](../status/chain-state-current.md) (2026-10-30T11:21Z, tip 1168).
  - **Deployed binary:** still `vote-records-20261030` (`87cfe797…`, source `ae70d66d`).
  - **Repository:** `origin/main` was at `b26de17a9` before this handoff. The private branch tip is given above.
  - **Merged but undeployed work on main:** everything listed under public main above. The [next validator release plan](../plans/active/next-validator-release-plan-20261115.md) carries it.
  - **Live probe in this session:** none beyond read-only RPC tip reads. I worked from read-only copies of the six validators' data. Nothing was deployed or activated, and I sent no Ethereum transaction; the dry run used a local fork.
- **Task Node.** 31 tasks were rewarded today:
  `task_48b0be5`, `task_c8c40fa`, `task_fedb924`, `task_830a8b0`, `task_2cd75ee`, `task_867d527`, `task_450d679`, `task_226a865`, `task_e5eaf10`, `task_ba58ccf`, `task_8c15b90`, `task_49c53d9`, `task_5e9e69b`, `task_21c9b84`, `task_44c12e6`, `task_8251347`, `task_08c234b`, `task_3df1607`, `task_0476789`, `task_5c202a3`, `task_60ae76d`, `task_b23131b`, `task_5755b30`, `task_f950e6a`, `task_1b8dda4`, `task_a963951`, `task_2590a24`, `task_349c1be`, `task_3833bab`, `task_1c059ad`, `task_438e1b9`.

## Next decision or action

- **Ours, first:**
  1. Finish the branch's full green run. Read the runs after the second split.
  2. Close the P2.5 qualification items for the release path:
     - the guest build and verifying-key pin;
     - the proof inventory entry;
     - the Base source attestation path;
     - the lock minimum;
     - the record pruning.
  3. Fix the wallet app's compiled signing module.
  4. Prepare the keyed rehearsal.
- **The client's, with the date first asked:**
  - pause minting on the Ethereum controller `0x9A02…` (2026-11-02; it was still unpaused at 07:35 UTC today);
  - the wrapped-token decision and the successor controller type (2026-11-03);
  - the rollout approval, with the keyed rehearsal and the six signing runs (2026-11-02/03);
  - the independent migration-trust descriptor (2026-11-03);
  - the review of the repair branches and the two policy questions (2026-11-03);
  - stop the old proving service on his host, and the faucet-key decision (2026-11-02);
  - the two issuer signatures (2026-09-25 and 2026-10-01);
  - the reserve-proof successor registration (2026-09-24);
  - the `ETHEREUM_MAINNET_RPC_URL` secret (2026-09-28).

## References

- [Private pull request #23](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/pull/23), branch `security/integration-check-20261102`. On that branch:
  - `docs/review/l1-security-release-candidate-20261103.md`;
  - `docs/runbooks/l1-security-release-rollout-20261102.md`;
  - `docs/plans/active/l1-security-remediation-milestone-20261102.md`;
  - `docs/handoffs/2026-11-03___codex__p0_developer_action_plan.md`.
- The client's 3 November gist.
- [ERC-20 on Base milestone](../plans/active/pft-erc20-base-listing-milestone.md).
- [Genesis external lanes](../architecture/genesis-external-lanes.md).
- [ERC-20 representation options paper](../listings/coinbase/pft-erc20-representation-options-20261026.md).
- [Next validator release plan](../plans/active/next-validator-release-plan-20261115.md).
- [Audit branch checked and rollout runbook written](2026-11-02___dravlic__audit_branch_checked_and_rollout_runbook_written.md) (2026-11-02).
- [Current State](../status/chain-state-current.md), unchanged today.
