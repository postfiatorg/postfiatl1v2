# Lane export header root merged and first local round trip of PFT back to Base

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-10 UTC

## BLUF

Phase 3 of the ERC-20 bridge sends native PFT back to Base as the same ERC-20. Today it went from a design note to a working round trip on local chains.

- **On main:** the Base verifier's binding of a lane export, and a block-header root over lane exports. The root sits behind an activation height that nothing sets.
- **On the branch `token/phase3-base-half` (under review, not merged):**
  - the export proof program, with a reproducible build;
  - the relay tool;
  - one full round trip on local chains: 1,000,000 units locked on a local Anvil became native PFT and came back out of the same lock in two exports;
  - the Base Sepolia dry run as one resumable command, exercised against a local fork.
- **What did not happen:** no SP1 proof was generated (a mock SP1 verifier accepted a placeholder). Nothing was sent to Base Sepolia or any public network. The devnet was not touched.
- **Also on main:** outside PR [#201] with review fixes, the Phase 3 deployment-order rehearsal, the FastPay recovery committee read and admission export for the Mac wallet, tests that pin the fix of issue [#160], and a CI repair.

Primary references: [design note](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md), [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md), [release plan](../plans/active/next-validator-release-plan-20261115.md).

## Current state

**The Base half of Phase 3, merged on main**

`5bff3138a` merges `11488db96`, the tip of `token/phase3-base-half-leftovers`. `1253764ff` records it in the milestone and the release plan.

- **The binding.**
  - A lane's PFT asset id and route profile come from the lane's genesis data by a fixed rule. This resolves decision 6 of the design note's section 9.
  - `Genesis::external_lane_verifier_pins` returns every constructor value of the Base verifier from one genesis.
  - `external_lane_export_egress_public_values` builds the exact public values `PFTLFinalityVerifierV1` decodes for one export packet ([`external_lane_egress_types.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/types/src/external_lane_egress_types.rs)).
  - Foundry tests release through a real `PFTLockV1` and a real `PFTLFinalityVerifierV1`, with a mock SP1 verifier.
- **The lane export root in the block header.**
  - **Why.** A finalized block did not commit a lane's exports where a bounded proof could reach them, because the state root is one flat hash.
  - **What.** Each block now carries a root over the export packets it accepted, built like the bridge-exit root.
  - **Activation.** The root is active only from an activation height: the genesis field `external_lane_export_root_activation_height`, or a governed amendment ratified by all validators. The amendment must be scheduled after its own block and at a height where consensus v2 is active. The root is part of the block hash only from that height and absent before it.
  - **Nothing sets it today.** No chain carries the field or the amendment. A test pins forty encodings of headers without the root to main's earlier bytes (`external_lane_block_encodings_without_the_export_root_hash_as_on_main`).
  - **Refusals and limits.**
    - Exports are refused while the root is not active (`external_lane_export_root_not_active`).
    - A recipient equal to the lane's token is refused (`external_lane_export_recipient_is_token`).
    - A block holds at most 4,096 exports; the rest wait for the next block.
    - Replay and `verify-blocks` recompute the root.
    - The validator signer guard knows the new message forms.
  - **Switching it on.** The command `ratify-external-lane-export-root-activation-height` and the [activation runbook](../runbooks/external-lane-export-root-activation.md) cover it. Every node and every signer process must run a build that knows the root before the height is proposed. An applied amendment cannot be undone.
  - **Decision taken.** Design note section 9, decision 8 is settled: the header root. The release plan lists it under "needs its own qualification".
  - The existing pfUSDC proof program's source is byte for byte unchanged.

**The Base half on `token/phase3-base-half` at `74d6e00af` (not merged; its review continues tomorrow)**

The design note's dated sections "Base half, step 3" and "step 4" are [on the branch](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-base-half/docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md).

- **The lane export proof program.**
  - It consists of `verify_external_lane_export_witness_v1` in [`crates/pft_lane_proofs`](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-base-half/crates/pft_lane_proofs/src/lib.rs), the guest [`programs/pft-lane-export`](https://github.com/postfiatorg/postfiatl1v2/tree/token/phase3-base-half/programs/pft-lane-export), and the node's proof-input command.
  - What it proves:
    - the PFTL finality of the export's block, with the existing finality check unchanged;
    - that the block id recomputed with the export root is the finalized one;
    - that the packet's leaf is under that root;
    - that the lane policy comes from the genesis whose hash the committee signed.
  - In SP1 execute mode it commits exactly the expected public values, at 33.6 to 76.7 million instructions depending on validators and ancestry.
- **The reproducible build.**
  - It ran twice on GitHub's runners, on separate machines: workflow `pft-lane-export-identity`, run 42818337185 on `439803aaa` and its re-run. Each run made two clean-tree Docker builds.
  - ELF sha256 `e92f20046c20761bb7a26ec47540000a88fad28b51874453437c3dd390f448ab`; verifying key `0x000cef5bfa4586cfb79e255a950ae8c1dbb15c4270a968ad5bfcbe32d0a81414`.
  - `03ccbf58f` commits them on the branch: the ELF, [`program-info.json`](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-base-half/programs/pft-lane-export/program-info.json) and `releases.json`.
  - An identity check runs on every push and pull request. It rebuilds the program in the pinned image and fails on any difference; run 42825026024 on `79c58030e` is green.
  - The program has an entry in the proof inventory, which now lists 8 systems.
  - **Key rule.** No verifier is to be constructed with this key before the conditions in the [program README](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-base-half/programs/pft-lane-export/README.md) hold on the commit being deployed. None is to be constructed for production before a real SP1 proof has been generated and verified with it.
- **The relay tool** [`scripts/pft-lane-export-relay.py`](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-base-half/scripts/pft-lane-export-relay.py) has the subcommands input, prove, release (with pre-send refusals), advance-checkpoint and status.
- **One full round trip on local chains** ([`scripts/rehearse-lane-roundtrip.py`](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-base-half/scripts/rehearse-lane-roundtrip.py); [record](https://github.com/postfiatorg/postfiatl1v2/tree/token/phase3-base-half/docs/review/lane-roundtrip-rehearsal-20261110)).
  - 1,000,000 units were locked on a local Anvil and released as native PFT.
  - Exports of 400,000 and 600,000 were paid out of the same lock.
  - A second release of one packet was refused before sending, and reverted when forced.
  - A further export of 1 atom was refused on PFTL.
  - Supply was conserved on both sides at every step.
  - **Real:** `PFTLockV1` and `PFTLFinalityVerifierV1`; PFTL blocks carrying consensus v2 commits of six validators; the witness check; the guest in execute mode.
  - **Not real:** there was no SP1 proof (a mock SP1 verifier accepted a placeholder). A local Anvil stood in for Base. The "fleet" is one data directory driven by a test, with no separate node processes.
- **The Base Sepolia dry run as one resumable command.**
  - The command is [`scripts/rehearse-lane-base-sepolia.py`](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-base-half/scripts/rehearse-lane-base-sepolia.py), with the [dry-run runbook](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-base-half/docs/runbooks/lane-base-sepolia-dry-run.md).
  - Today it ran only against a local fork of Base Sepolia ([record](https://github.com/postfiatorg/postfiatl1v2/tree/token/phase3-base-half/docs/review/lane-base-sepolia-fork-rehearsal-20261110)). Nothing was sent to Base Sepolia.
  - On the fork:
    - the real verifier was constructed with the program key and Succinct's versioned SP1 verifier `V6_1_0_SP1_VERIFIER_GROTH16` at `0xb69f2584CBcFf99a58C4e7002E8b89Af54a6f4e2` (no owner or upgrade path found);
    - both contracts landed at their predicted addresses;
    - it ran a Lock, its release on PFTL, an export and its proof input.
  - It stops at `prove-and-release`. A real proof needs a prover (a GPU host or a prover-network account), and we have none.
  - **Review fixes, waiting for a second reading.** A review found three gaps: the real deploy stage did not insist on the predicted addresses, the program key's provenance was not enforced, and a crash after a mined transaction could not be reconciled. The fixes are on the branch:
    - `3c93d9d88`: the keystore, nonce and address checks before and between the two deployments;
    - `acdb7fdd7`: the key taken only from the committed program info in a clean worktree;
    - `a08999ff3`: each public transaction's intent recorded before sending and reconciled on a rerun, never a second Lock;
    - `780eb889d`: real-proof modes for the checkpoint advance, and two more pre-send checks;
    - `c8e672672`: keystore names redacted, a separate relayer account, a gas-price guard;
    - `74d6e00af`: the fork run and the runbook redone.
  - No real transaction is sent before that second reading.

**The holder's export command on `token/export-command` at `3a6b550a4` (not merged)**

- It adds `external-lane-export` and `external-lane-export-status`, with the Python client functions.
- Two review rounds are in.
  - The pre-sign check now runs on exactly the ledger that admission executes a new asset transaction on. One shared helper, `apply_pending_before_new_asset_transaction`, is used by admission itself and by the export quote.
  - A quote carries an explicit `export_checked` marker and is refused without it.
- The branch is merged up to main with the header root.
- **Open:**
  - the pre-sign check does not look at the sender's sequence yet (admission then refuses with `bad_sequence`, no fee);
  - the last review.

**The watcher's export view on `token/lane-watcher-exports` at `28318e902` (not merged)**

- The branch is off `token/phase3-base-half` at `79c58030e`.
- With `--exports`, the release watcher checks the outbound direction:
  - every finalized `Released` event of the lock on Base must match a PFTL export packet (amount, recipient, withdrawal id), else it raises the alarm `payout_without_export`;
  - it reports `paid` / `unpaid` per export;
  - it checks four total invariants.
- In the round-trip rehearsal it reports both exports paid with zero alarms. It raises the alarm on a payout forged on an Anvil snapshot for a packet PFTL never recorded.

**Also merged on main**

- **Outside PR [#201]** (darkmatter-sudo; issue [#181] points 3 and 5), with review fixes:
  - `ab2150253` merges the PR; `431231fc4` merges the review fixes:
    - the validation decode no longer leaves an unwiped copy;
    - tests fail when a Drop is removed;
    - both hex helpers return zeroizing arrays;
    - further copies are wrapped, and the remaining ones are listed with reasons;
    - the secret owner is sealed to the pasta types.
  - `6b6fcd786` adds the release plan row.
  - The 10 release vector and proof pins pass unchanged.
  - Issue [#181] stays open: the upstream secret-owner request is not filed.
- **The Phase 3 deployment-order rehearsal:** `5c4fc262d` + `c29d8c623` ([`scripts/rehearse-phase3-deployment-order.py`](https://github.com/postfiatorg/postfiatl1v2/blob/main/scripts/rehearse-phase3-deployment-order.py); [record](../review/phase3-deployment-order-rehearsal-20261109/README.md)). It adds:
  - two dedicated deployers;
  - every constructor value read back;
  - the lock's cap taken from the lane;
  - a Lock;
  - a "Before a public network" section.
- **For the Mac wallet** (from the 2026-11-07 wallet handoffs):
  - The read `fastpay_recovery_committee` ([RPC methods](../rpc/methods.md)) and the command `fastpay-admission-export`, which writes the `{committee, policy}` file the wallet loads ([Web wallet, "FastPay Committee Pin"](../wallets/web-wallet.md#fastpay-committee-pin)).
  - The commits: `0d275e796`, `b7a90bc2f` (public through the gateway, not cached), `95d60136c`, `bc27c377c`, `d67153bda`, and `915c9524e` (the read and its height under one commit lock).
  - It is not live until the next release is deployed. Reviewing the file and pinning it in a signed build is the wallet lane's job.
  - I read all seventeen asks of those handoffs. Next by value for the wallet: validator-signed fence status (the two "terminal proofs"), a cursor on `owned_objects`, native-session hardening.
- **Issue [#160]** (`verify_bridge` over rpc-serve):
  - PR [#161] had fixed it on 2026-10-26.
  - `1da7bf5d5` adds a sweep of every parameterless public read over rpc-serve on a fresh node, and a Python client test.
  - `5c8f693ca` adds the plan row. The issue is closed.
- **CI repair.**
  - `product-security-ci` was red on main from `6b6fcd786` through `d67153bda`, only in the job `public-tree-hygiene`. PR [#201] had changed three source files the proof inventory pins.
  - `c2be70c5f` updates the three hashes the way `d23b14d16` did. Only PR [#201]'s commits touched those files, and the release pins pass.

**Deployment state** (see [Current State](../status/chain-state-current.md))

- **Last observed fleet state:** not re-read today. The last read-only observation is the one recorded on 2026-11-09.
- **Deployed binary and lineage:** unchanged from that record.
- **Repository HEAD:** `main` at this handoff's commit. The branches are at the tips named above.
- **Merged but undeployed:** everything on main above.
- **Live probe:** none. The devnet was not touched: no deployment, signing, transaction or read.

**Task Node** (each id read with `corbanu tasknode task show <id> --json` at 13:55Z)

- **Rewarded:**
  - `task_caf3a46790456446eda83deb48441aac` (PR [#201]);
  - `task_80ed16e149096c80a0904ccaead70d66` (the rehearsal merge);
  - `task_ae820fa48ee0963e12fc2a9391e5b488` ([#160]);
  - `task_0aed4547b17c25f5307790e67a9b963e` (the round trip);
  - `task_78a73b1ae9e942afc8096e1e49a4ed8f` (the header root merge).
- **Also shown as rewarded:** the three tasks expected to be awaiting review:
  - `task_d65943435e72ab87e00edd28a3b5cc30` (the FastPay read);
  - `task_b71105271bc4b56177e7a00b5d068d45` (the proof program);
  - `task_fbf221cfe9f2f33f1f2b74d39e4a42a8` (the dry-run command).
- **Not read here:** the export command's task is rewarded as I recorded it; its id was not at hand.
- **Ran without a task:** three steps of the morning. The long-lived session had lost its Task Node profile variable; restarting it fixed this.

**Public CI at the time of writing** (`gh run list --branch main --limit 30`, 13:57Z)

- **`docs-build`:** green on every push, including `1253764ff`.
- **`rust-ci`:** green from `6b6fcd786` through `915c9524e`; running on `1253764ff`.
- **`product-security-ci`:** red on `6b6fcd786`, `c29d8c623`, `5c8f693ca`, `95d60136c`, `bc27c377c` and `d67153bda`, only in `public-tree-hygiene`. It is green on `c2be70c5f` and `915c9524e`, and running on `1253764ff`.

## Next decision or action

Ours first:

1. **Finish the review of `token/phase3-base-half`, then merge it.** The review covers the proof program, the relay, the round trip and the dry-run command, plus today's merged range. The committed program identity must come from a build of the final source. No verifier is constructed with the key before the identity check is green on the commit being deployed.
2. **Run the Base Sepolia dry run with the one command, after that review.**
   - Fund two one-time deployer keystores from the rehearsal holder `0x6E4E…`. It holds about 0.00198 test ETH; the sheet needs about 0.00007.
   - Deploy the real verifier and a new lock for the disposable rehearsal token.
   - Run a Lock, its release on a local fleet, an export and its proof input.
   - This proves the deployment order on a public chain. It stops before the payout until a prover exists.
3. **The export command:** its second fix round (its pre-check must run on exactly the ledger admission uses), then its merge.
4. **The wallet lane's asks:** next the validator-signed fence status, then the `owned_objects` cursor.
5. **Read CI on today's last pushes.** `rust-ci` and `product-security-ci` were still running on `1253764ff`.

Then the client's, each with the date first asked:

- **The audit vendor and budget for the token and lock contracts** (2026-10-26 / 2026-11-06).
  - A shortlist of three firms, with a request text, went to you separately today.
  - If the way back to Base ships, the Base verifier contract and the export proof program join the audit's scope, because the lock is tied to its verifier for good.
- **The remaining Phase 3 decisions in the design note's section 9** (2026-11-05):
  - whether Phase 3 ships;
  - an export minimum;
  - a refund for an unpaid export;
  - who runs the prover and the relay, and how they are funded (a prover for SP1 6.3.1 is the one missing piece for a first real payout on Base Sepolia);
  - the response to a false release;
  - new today (2026-11-10), the fixed-verifier choice. The Base verifier fixes its program key, the PFTL protocol version and the SP1 verifier forever, and the lock fixes the verifier forever. So either PFTL upgrades keep the old checkpoint format provable for the lock's life, or a successor-verifier design comes before the production lock.
- **The rollout go, the keyed rehearsal on his hosts, the six migration signatures and the migration-trust descriptor** (2026-11-02/03/04).
- **The wrapped-token path, A or B** (2026-11-04).
- **The three red CI jobs on the next-release branch**, and our fix branch `security/next-release-ci-fixes-20261106` (2026-11-06).
- **The reachable host of the audit and operations machine** (2026-11-04).
- **The six cached checkpoint votes** (2026-11-04).
- **The old proving service and the faucet key** (2026-11-02).
- **The two issuer signatures** (2026-09-25 / 2026-10-01).
- **The reserve-proof successor registration** (2026-09-24).
- **The `ETHEREUM_MAINNET_RPC_URL` secret** (2026-09-28).
- **From the 2026-11-07 wallet handoffs** (listed 2026-11-10):
  - review the FastPay committee file and pin it in a signed build;
  - switch native-session admission on at the gateway;
  - private admission and pricing-route activation on the live network;
  - which "old Ethereum controller" the handoff means.

Minting on the Ethereum controller `0x9A02…` reads as paused since this morning (`mintPaused()` true at block 26383455, 08:56Z), so that item is closed.

## References

- **Pull requests and issues:** [#201], [#181], [#160], [#161].
- **Phase 3:**
  - [design note](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md): section 9 and "Base half, step 1" and "step 2" on main; steps 3 and 4 [on the branch](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-base-half/docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md);
  - [Genesis external lanes, "Exports"](../architecture/genesis-external-lanes.md#exports);
  - [export root activation runbook](../runbooks/external-lane-export-root-activation.md).
- **Plans:** [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md), [next validator release plan](../plans/active/next-validator-release-plan-20261115.md).
- **Wallet:** [RPC methods](../rpc/methods.md) (`fastpay_recovery_committee`), [Web wallet, "FastPay Committee Pin"](../wallets/web-wallet.md#fastpay-committee-pin).
- **Branches:**
  - [`token/phase3-base-half`](https://github.com/postfiatorg/postfiatl1v2/tree/token/phase3-base-half), with its records under `docs/review/` on the branch;
  - [`token/export-command`](https://github.com/postfiatorg/postfiatl1v2/tree/token/export-command);
  - [`token/lane-watcher-exports`](https://github.com/postfiatorg/postfiatl1v2/tree/token/lane-watcher-exports).
- **Status:** [Current State](../status/chain-state-current.md).
- **The client's wallet handoffs** in the private wallet repository: `docs/handoffs/2026-11-07_primary_postfiat_wallet.md` and `docs/handoffs/2026-11-07_normal_mac_private_wallet.md`.
- **Previous handoff:** [2026-11-09: quote and history bugs fixed, exchange page updated, wallet reads merged](2026-11-09___dravlic__quote_and_history_bugs_fixed_exchange_page_updated_and_wallet_reads_merged.md).

[#160]: https://github.com/postfiatorg/postfiatl1v2/issues/160
[#161]: https://github.com/postfiatorg/postfiatl1v2/pull/161
[#181]: https://github.com/postfiatorg/postfiatl1v2/issues/181
[#201]: https://github.com/postfiatorg/postfiatl1v2/pull/201
