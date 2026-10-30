# Vote records release live and genesis external lane merged

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-10-30 UTC

## BLUF

The next validator release, `vote-records-20261030`, was cut, qualified locally and rolled out to all six devnet validators today. Its source is `ae70d66d`, its executable `87cfe797…` and its signed manifest `ee1df5b3…`. The applies ran 10:31–11:14Z at blocks 1162–1167. The after state at 11:21Z is 1168 / `a20e406e…` / `39771b74…`, and no rollback was needed. The release carries:

- the VR-01 vote-record fix (`5fc43961`, #175 `72475b01`, #174), shown fixed live at block 1168: validator-4's record lists all six as accepted, including validator-2, which accepted after the quorum (the case the old build wrote as `no_answer`);
- the whole W-1 private path, switched off and ungoverned: no `rpc-serve` unit passes `--allow-private-action-admission`, and neither the pricing enforcement nor the egress policy is activated;
- the secret-lifetime fix `548d4918`, the `k256` removal `6f7aae33` and the rust-ci test-race fix `ae70d66d`. That fix covers two test-only races in the node bin test binary; the commit where the red started did not cause them.

The ERC-20 milestone's P2.2 genesis external lane is written and merged (`bfe44841`, ticked in `5e28cd63`). It passed a correctness review with three fix rounds. Lanes are configured on no network. The W-1 specification is locked at 86.27 on run r4 (`8d9d6838`). The deployed release's private readiness is recorded in the W-1 manifest and milestone (`92425854`).

Main documents:

- the release directory [`deployments/vote-records-20261030/`](https://github.com/postfiatorg/postfiatl1v2/tree/release/vote-records-20261030/deployments/vote-records-20261030) on branch `release/vote-records-20261030`: README, DEPLOY-SHEET, qualification packet and `observed/` (`dcddd53d`, `6201e1ee`, `19cd2355`, `d4b344be`);
- [Current State](../status/chain-state-current.md) (`85051b2b`);
- the [deployed release plan](../plans/active/next-validator-release-plan-20261108.md) and the new skeleton [plan for the next release](../plans/active/next-validator-release-plan-20261115.md) (`4a541185`).

## Current state

- **Release.**
  - **Cut and packet** (`dcddd53d`, recorded on main in `76dd4f86`). Two clean builds were identical (`87cfe797…`). History checks ran on the 1153, 1131, 1123 and 1021 archives, and the exact replays of 924 and 1153 PASS. The old binary reads the new build's data, so a rollback needs no extra step.
  - **Before state** at 1161, with the signed canary backup `9b68d2a1…` and a PASS replay of the current head (`6201e1ee`, `19cd2355`).
  - **Rollout** (`d4b344be`). It went one validator at a time in the order 1, 0, 2, 3, 4, 5, with a 1-PFT test grant and a six-way observation after each.
    - validator-2 needed a second try: new SSH connections to its host hung for minutes several times. sshd logged nothing, and the RPC tunnel kept working.
    - A connection-reuse wrapper fixed this for every later step.
  - **After checks** passed on all six:
    - manifest verified, build `ae70d66d`, units with the standard flags;
    - the private path switched off on every host;
    - the snapshot points file present, index v3 lag 0;
    - activation heights 1153, routes epoch 1, swap epoch 2.
  - **StakeHub.** Its three settings point at the new release.
  - **CI on `ae70d66d`** is green in all three workflows: docs-build `41032153113`, rust-ci `41032153158` and product-security-ci `41032153060`.
- **Deployment record.**
  - **Last observed fleet state:** 11:21Z, six on `vote-records-20261030` at 1168 ([Current State](../status/chain-state-current.md)).
  - **Deployed source:** main `ae70d66d` on `release/vote-records-20261030`.
  - **Repository:** `origin/main` was at `5e28cd63` when this was written.
  - **Merged but not deployed:** the genesis external lane (`bfe44841`) and the readiness route fix (`ca7283ce`).
  - **Live probe:** none was made while writing this handoff.
- **SCT-08 item 4.** The live vote records are recorded as evidence (`7dcc23eb`) in the [campaign document](../review/sct08-evidence-campaign-20261023.md).
  - The `no_answer` at 1166 is the VR-01 symptom on the old executable (`dcf70236`). validator-3 did vote, 18 s after the apply, but the certificate already had five votes without it.
  - The first window run fully on the fixed build starts at 1168.
- **Genesis external lane** ([architecture page](../architecture/genesis-external-lanes.md)).
  - **Branch commits:** `token/genesis-external-lane` carries `90f44df2`, `2a84c980`, `ef2892a3`, `a3cef750`, `fb35f4d9` and `e8d56087`; the merge is `bfe44841`. Five rebased copies of the first five also sit in main's history under the real merge (`9133dc6b`, `a12f5925`, `64d3fd08`, `0abeb4c2`, `f45c845b`), because a `pull --rebase` replayed them before the merge was pushed. The trees are identical, and no history was rewritten.
  - **Genesis input:** `Genesis` takes an optional lane list. It gives the reserved amount as a parameter and pins the release policy: chain id, lock and token addresses, code hashes, committee epoch and root, and confirmation depth.
  - **Supply:** the faucet receives the supply minus the lanes, and the conservation checks count the lane remainder.
  - **Lane records:** until the release operation exists (P2.4), each must equal its genesis state exactly.
  - **Lane-less chains:** the genesis, ledger and state root are proven byte-identical. The devnet-2 genesis `ce22ca8c…` is pinned, and a populated ledger has a golden.
  - **Related docs:** the design note's merge-gate sentence was reworded: the supply was fixed 2026-10-28, and no lane comes before the mainnet genesis plan. The line references between the ERC-20 documents were corrected (`81f73481`).
- **W-1** ([milestone](../plans/active/user-proven-shielded-actions-milestone.md), [specification](../specs/user-proven-shielded-actions-spec-20261029.md)).
  - **Spec:** locked at 86.27 on r4 (`8d9d6838`), and M1 is ticked.
  - **Readiness:** read from all six validators (`92425854`). The reads were byte-identical, the verifiers are on their pins, and atomic batches are active from 483.
    - It is not ready for two reasons. The pricing activation is absent. The A666/pfUSDC route reads "absent" because two routes match its codes.
    - `ca7283ce` on main fixes the route read by matching on asset id; it ships with the next release.
    - The readiness is recorded in the contract manifest. S1.3 and the "published from the deployed release" sub-item of S7.1 are ticked.
  - **Six-validator evidence:** ticked in `681bd96b`.
- **Dependencies.**
  - **vite:** the five alerts were stale on GitHub's side and were dismissed as inaccurate. The lock has installed 8.3.5 since `56be18af`, and the dependency graph never re-read it.
  - **Solana stake reader:** its lock update is not a lock-only change, because the program is an immutable mainnet program with a pinned identity. This is recorded in the release plan row (`059776e3`).
- **Devnet.** It was changed by the release rollout above. The evidence is the packet's `observed/` files and the Current State record.
- **Task Node.** The session logs show these tasks rewarded today:
  - `task_8f13b2c3…`, the CI fix;
  - `task_6642fde2…`, the genesis lane;
  - `task_8f03d982…`, its first fix round, 1.6 PFT;
  - `task_f36033b1…`, the cut;
  - `task_8f700146…`, the rollout, 5 PFT;
  - `task_2968993e…`, the readiness manifest;
  - `task_c46b4cd3…`, the readiness fix;
  - `task_c52ea00d…`, the spec lock;
  - `task_34c2b5fe…`, the merge (its evidence discloses the rebased copies).

  The session logs have no id for a third genesis-lane task.

## Next decision or action

My next steps, in order:

1. P2.4 of the ERC-20 milestone: the inbound release path on PFTL with the Base checkpoint source. It gets its own correctness review before merge.
2. With the wallet team:
   - S1.5, S2.4 and the egress schema agreement;
   - then the governance activations on the devnet: the pricing enforcement, the egress policy and the single governed A666/pfUSDC route;
   - then the admission switch on `rpc-serve`. Each of these is its own qualified step.
   - then S8.
3. The next release, from the [20261115 skeleton](../plans/active/next-validator-release-plan-20261115.md). The genesis lane and the readiness fix ride along.
4. The page's biweekly update on 2026-11-10.

Only the other lanes can provide these (date first asked):

- the permit content validator command on the operator host (2026-10-29);
- the D15b recovery plan for the stuck deposit `0x6998…884e` (2026-10-29);
- the ERC-20 slice size (it now also fixes the genesis lane amount), the launch allocation, the issuing entity, the treasury Safe's signers and threshold, the PFT Ledger treatment, and the audit budget and vendor (2026-10-26; the supply was answered 2026-10-28);
- the two issuer signatures per the issuer signing sheet, A666 first and then BMNRC (2026-09-25 and 2026-10-01);
- the on-chain registration of the reserve-proof successor (2026-09-24);
- the `ETHEREUM_MAINNET_RPC_URL` secret (2026-09-28);
- the StakeHub acceptance file and binaries (2026-10-21);
- SCT-08 items 1–3 (2026-10-23);
- the production genesis plan and the website go-live (2026-10-23).

## References

- Release branch [`release/vote-records-20261030`](https://github.com/postfiatorg/postfiatl1v2/tree/release/vote-records-20261030) and its [release directory](https://github.com/postfiatorg/postfiatl1v2/tree/release/vote-records-20261030/deployments/vote-records-20261030) ([README](https://github.com/postfiatorg/postfiatl1v2/blob/release/vote-records-20261030/deployments/vote-records-20261030/README.md), [DEPLOY-SHEET](https://github.com/postfiatorg/postfiatl1v2/blob/release/vote-records-20261030/deployments/vote-records-20261030/DEPLOY-SHEET.md))
- [Current State](../status/chain-state-current.md)
- Release plans: [deployed (20261108)](../plans/active/next-validator-release-plan-20261108.md) and [next (20261115)](../plans/active/next-validator-release-plan-20261115.md)
- [User-proven shielded actions milestone](../plans/active/user-proven-shielded-actions-milestone.md) and [specification](../specs/user-proven-shielded-actions-spec-20261029.md)
- [PFT as an ERC-20 on Base milestone](../plans/active/pft-erc20-base-listing-milestone.md) and [Phase 2 design note](../specs/pft-erc20-base-bridge-phase2-design-20261026.md)
- [Genesis external lanes](../architecture/genesis-external-lanes.md)
- [SCT-08 evidence campaign](../review/sct08-evidence-campaign-20261023.md)
- The client's last handoff: [Task Node wallet findings for the L1](2026-10-29___goodalexander__tasknode_wallet_findings_for_the_l1.md)
- Previous handoff: [Wallet app private path built end to end and permit tool added](2026-10-29___dravlic__wallet_app_private_path_built_end_to_end_and_permit_tool_added.md)
