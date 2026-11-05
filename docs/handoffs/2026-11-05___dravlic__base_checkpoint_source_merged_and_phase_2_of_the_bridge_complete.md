# Base checkpoint source merged and Phase 2 of the bridge complete

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-05 UTC

## BLUF

Today the token's bridge into native PFT got its last engineering piece: the Base checkpoint source. These are the node commands with which committee members observe a finalized Base block, sign and assemble their votes, and build the receipt proof of a Lock into a release operation (merge `08d6e03f6`; [record section](../listings/coinbase/pft-erc20-release-path-qualification-20261103.md#base-checkpoint-source-2026-11-05); [page section](../architecture/genesis-external-lanes.md#the-base-checkpoint-source)). In its live run the commands observed finalized Base Sepolia block 54,766,144, five validators signed it, the certificate assembled, and the block's receipt trie rebuilt to its receipts root; no transaction was sent. Phase 2 of the [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md) is now built; P2.5 stays open only on the outside audit of the lock contract.

Beside it:

- the watcher of design section 5.2 (`41bde8b8a`, [page section](../architecture/genesis-external-lanes.md#the-watcher));
- the [Phase 3 export design note](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md) and the refused export operation (`7a968007e`);
- the validator signer process for rpc-serve (`b610061fc`, W-1 S7.1);
- the outside pull request #185, merged with its review fixes (`9251541ef`, `b319b4d2c`);
- the reader's page [The PFT token on Base](../architecture/pft-token-on-base.md) (`0efe752a5`);
- on the private side, a re-check of the rebuilt security candidate and an owned-history check on all six validators' data copies.

## Current state

**The Base checkpoint source.**

- Four node commands, `external-lane-checkpoint-observe`, `-sign`, `-assemble` and `external-lane-lock-proof` ([`external_lane_checkpoint_source.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/557b6bf52efac175cc4b944ae9ca06f2154c138b/crates/node/src/external_lane_checkpoint_source.rs)). The receipt-proof builder now encodes OP Stack deposit receipts; every Base block starts with one.
- 16 tests, plus an ignored live test.
- A correctness review found no code defect and asked for fixes, all applied before the merge:
    - typed refusals;
    - the `Locked` topic taken from the bridge crate;
    - a signature self-check before writing;
    - tests for four refusals;
    - a relaxed token check for unrelated transfers;
    - boundary tests;
    - an honest page: what is trusted, `--data-dir` in every step, the signing-state directory as safety state, and every refusal code.
- Merge `08d6e03f6`; records `a7347213f` and `6d9aa57cd`.
- The live run: finalized block 54,766,144 (hash `0x2611dc7e…08cc1`, head 54,772,391), signers `validator-0` to `validator-4`, a certificate of 5 votes, and the block's 3 receipts rebuilt.
- Limit: no Lock exists on the rehearsal lock yet, so `external-lane-lock-proof` has run only against the mock RPC. A real Lock needs Base Sepolia test ETH and is the next rehearsal.

**The milestone.**

- P2.5's sub-line is ticked with the merge. P2.5 stays open only on the outside audit, which P1.4's vendor delivers.
- The [Phase 2 design note](../specs/pft-erc20-base-bridge-phase2-design-20261026.md) carries a status sentence in section 4. The options paper stays locked.
- The Phase 3 note supersedes one Phase 2 sentence: the lock contract cannot be deployed before genesis. The verifier needs a finalized PFTL checkpoint and the lock needs the verifier, so both addresses are predicted, genesis pins the lock's address and code hash, and both contracts are deployed after the first finalized checkpoint (the eight operator steps in the Phase 3 note, section 5).

**The watcher** (design section 5.2, [`external_lane_watcher.py`](https://github.com/postfiatorg/postfiatl1v2/blob/557b6bf52efac175cc4b944ae9ca06f2154c138b/python/postfiat_rpc/external_lane_watcher.py)).

- It matches every release against its Lock at the release's own source block, with the block hash verified. It pages the lane read bound to one snapshot, keeps a state file bound to the chain, the genesis and the lane policy, and alarms with typed codes.
- 32 watcher tests.
- The live check, a clean run at 13:02 UTC:
    - against a local `rpc-serve` on the repository's external-lane test genesis and `https://sepolia.base.org`;
    - zero releases checked, because the lane has none;
    - the lock's runtime code hash at finalized Base Sepolia block 54,803,494 equal to the genesis pin;
    - exit 0.
- The match of a release against its Lock has run only against mocks, because no release or Lock exists yet.
- Merge `41bde8b8a`; record `557b6bf52`: the [record section](../listings/coinbase/pft-erc20-release-path-qualification-20261103.md#the-watcher-2026-11-05), the P2.5 sub-line and the release plan.

**Phase 3** (optional, P3.1).

- The [design note](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md) covers the export operation, the accounting with checked u64 arithmetic, the proof on Base, the deployment order, the risks and the decisions.
- The skeleton refuses `external_lane_export_v1` at admission, batch building and execution (`external_lane_export_not_supported`, `external_lane_export_recipient_is_lock`). In a certified block the ordinary rejected-transaction fee rule applies; this is tested.
- A correctness review found and fixed a design gap: an export's payability on Base holds only while every PFTL release is backed by a real Lock. The note now states this conditionally, with the response left as a decision for the product owner; our lean is that the governed zero-cap amendment also halts exports.
- Merge `7a968007e`; records `3a1978cdf`, which also corrects the lock section of the [contracts README](https://github.com/postfiatorg/postfiatl1v2/blob/557b6bf52efac175cc4b944ae9ca06f2154c138b/crates/pft-token-contracts/README.md).

**The validator signer process** (W-1 S7.1).

- `postfiat-node validator-signer-serve` holds the validator key behind a `0660` Unix socket with a peer-uid allow-list.
- rpc-serve signs through `--finality-signer-socket` (votes, timeouts, proposals and transport authentication) and refuses every key-file read in socket mode.
- Readiness reports the signer, its key match and the same-user condition.
- A correctness review found one serious defect before the merge, and six smaller ones; all are fixed. The serious one: the socket was `0600`, so a two-user deployment could not connect.
- Merge `b610061fc`, records `e7749f414`; the S7.1 sub-line is ticked.
- Open:
    - a double-sign guard inside the signer;
    - the two deployment checks;
    - a two-user deployment with new unit files, since the release unit still passes `--finality-key-file`.
- The crypto call-site policy table now records the moved signing sites (`313cf3926`).
- The [next validator release plan](../plans/active/next-validator-release-plan-20261115.md) lists every commit since the cut. The governed cap amendment and committee move are classified as needing their own qualification.

**Pull request #185** (an outside contributor, Ulysseus).

- A genesis lane whose window cap rounds down to zero is now refused.
- Merged unchanged in `9251541ef`, with the review fixes in `b319b4d2c`: the wording, the asserted refusal code, the boundary kept as a comment and the fixtures.
- A thank-you comment is on the pull request.

**The reader's page** ([The PFT token on Base](../architecture/pft-token-on-base.md), `0efe752a5`): the token, what exists on Base Sepolia, the lane, the path from a Lock to native PFT, and what is open.

**Public CI.** Read the latest runs before relying on them.

- rust-ci was red on main from 2026-11-04 12:27 UTC: a doc comment named a devnet flag that the verification-switch scan refuses. Fixed in `1599fb4b1`.
- product-security-ci was red on `e7749f414`, on the call-site policy. Fixed in `313cf3926`.

**The exchange guidance page.** Its 10 November update is drafted on branch `update-20261110` of the private repository postfiatorg/exchange-guidance-review (commit `3679886`).

- The pinned commit moved to `e7749f414`.
- 13 sentence changes; nothing is published.
- A patch for the generated code map and the pre-publish checks are in the note beside the draft.

**Private side** ([postfiatl1v2-private-archive-20260717](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717) PR #24, branch `security/our-check-20261105` = `05af5e1d0`).

- The rebuilt candidate (runtime `c68b3b68f`) was re-checked:
    - the transport reader change is correct: no incoming frame gets a longer budget;
    - 18 of 19 reproduction tests are unchanged. The 19th's fixture used a builder that now refuses below-threshold quorums, so it was repaired (`8c9b21e43`, test file only); live admission still refuses the one-signature batch;
    - the packet's evidence checker passes (53 checks, 16 artifacts).
- The owned-history reconstruction ran on all six validators' copies with the retained executables:
    - identical results and commitment on every validator;
    - both legacy checkpoint logs absent everywhere;
    - complete history still unproved.

    See [`docs/review/l1-security-owned-history-six-copies-20261105.md`](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/05af5e1d041d373af417db1cfdc7ddbbd8238d88/docs/review/l1-security-owned-history-six-copies-20261105.md) and two PR #24 comments.
- A registry lineage for pruned snapshots past a key rotation is built for the release after the candidate (branch `security/rotated-pruned-lineage-20261105` = `e9b2aa144`). It is not in the candidate.

**Devnet and deployment.**

- Not touched: no deployment, no signing, no transaction; only read-only reads of Base Sepolia and Ethereum.
- No fleet probe this session. The last recorded fleet state is in [Current State](../status/chain-state-current.md) (updated 2026-10-30, release `vote-records-20261030` on all six), unchanged today.
- Merged on main but undeployed: everything above, carried by the [next validator release plan](../plans/active/next-validator-release-plan-20261115.md) and the private candidate.
- Repository: `origin/main` at `557b6bf52` before this handoff.
- Minting on the Ethereum controller `0x9A02…` was still switched on at 07:25 UTC (block 26347115).

**Task Node.** Nine tasks went through the lifecycle today and are rewarded:

- `task_503f50e690c46c9865df01c457c97444`: the Base checkpoint source;
- `task_8fc5f1c8b6492b5f7ad9ccd0c5826426`: the validator signer;
- `task_1b125373fb2872b4eff7aadbaedddeac`: pull request #185;
- `task_ad8e9090a9625e812a004e3c116a7484`: the checkpoint source's qualification and merge;
- `task_3dbb55514240be1a15e3dfc7b9fd1dce`: the signer merge;
- `task_ec383eb946a4a92fcd81b6a3acc4ae94`: the Phase 3 design;
- `task_7ae4d6961d79196ee5777717a3d6d77d`: the watcher;
- `task_600e814af06eff55db1ad37414e92c2c`: the Phase 3 merge;
- `task_657270d114e74e37d1548e814cfd7a2d`: the watcher's check and merge.

The private-side steps took no Task Node action.

## Next decision or action

Ours, first:

1. The rollout day of the security release under the runbook, once the keyed rehearsal on the client's hosts is done.
2. A real Lock on Base Sepolia through the four commands, and a release on a local fleet. This needs test ETH on the lock deployer `0x6E4E…`.
3. The exchange page's update on 10 November: review the draft, apply the code-map patch, re-run the checks, publish.
4. The two-user deployment of the validator signer (unit files) and the double-sign guard.
5. Phase 3's implementation, only after the product owner's decisions.
6. Read CI on `313cf3926` and `41bde8b8a`.

The client's, each with the date it was first asked:

- Pause minting on the Ethereum controller `0x9A02…` (2026-11-02). It was still on at 07:25 UTC today, the fourth working day.
- The keyed rehearsal on his hosts, the six migration signatures, the migration-trust descriptor and the rollout go (2026-11-02/03/04). His own 2026-11-05 handoff lists the same gates.
- The wrapped-token path, A or B, with its inputs (2026-11-03/04).
- The Phase 3 decisions in the export design note, section 9 (2026-11-05).
- The reachable host of the audit and operations machine (2026-11-04).
- Destroy the six cached checkpoint votes, or anchor them deliberately (2026-11-04).
- Stop the old proving service, and the faucet-key decision (2026-11-02).
- The two issuer signatures (2026-09-25 / 2026-10-01).
- The reserve-proof successor registration (2026-09-24).
- The `ETHEREUM_MAINNET_RPC_URL` secret (2026-09-28).

## References

- [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md) and [the release path qualification record](../listings/coinbase/pft-erc20-release-path-qualification-20261103.md).
- [Genesis external lanes](../architecture/genesis-external-lanes.md): the Base checkpoint source, the watcher, Phase 3.
- [Phase 2 design note](../specs/pft-erc20-base-bridge-phase2-design-20261026.md) and [Phase 3 export design note](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md).
- [The PFT token on Base](../architecture/pft-token-on-base.md).
- [W-1 milestone](../plans/active/user-proven-shielded-actions-milestone.md) and the [public RPC operator policy](../runbooks/public-rpc-operator-policy.md) (the validator signer process).
- [Next validator release plan](../plans/active/next-validator-release-plan-20261115.md).
- The client's handoff [`2026-11-05___codex__security_release_gate_engineering.md`](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/ba1b8c53686256a360abd0b36be8150715c9e1fc/docs/handoffs/2026-11-05___codex__security_release_gate_engineering.md) on the private branch `security/consolidated-20261103` (`ba1b8c536`).
- Previous handoff: [Security candidate rebuilt with every repair and token amendments merged](2026-11-04___dravlic__security_candidate_rebuilt_with_every_repair_and_token_amendments_merged.md).
- [Current State](../status/chain-state-current.md), unchanged.
