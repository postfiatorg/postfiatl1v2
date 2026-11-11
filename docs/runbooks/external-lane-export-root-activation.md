# External Lane Export Root Activation

This runbook switches on `external_lane_export_root`, the block-header root over a block's lane export packets, on a running network. From its activation height every header carries the root, and the block hash, proposal hash, vote message and consensus v2 block id take their new versions (`postfiat.block.v5` and the others). Before the height `external_lane_export_v1` is refused with `external_lane_export_root_not_active`. The design is the step-2 section of `docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md`.

It is a consensus migration. Do not introduce it with an uncoordinated rolling restart.

## New chain

Set the genesis field `external_lane_export_root_activation_height` (for example `1`) and nothing else. `init` refuses a genesis whose height is one at which consensus v2 is not active.

## Preconditions on a running network

- Every validator node **and** every validator signer process runs a build that knows the root. An older signer refuses to sign a block that carries it, because its guard does not know the `.v5` vote and proposal messages. An older node fails to verify the new encodings.
- Consensus v2 is active at the chosen height. The amendment is refused at admission, batch building, execution and replay with `external_lane_export_root_activation_requires_consensus_v2` otherwise.
- No lane with `exports_enabled: true` can be reached by an old binary. Old code accepts an export without the new gate, so a node on an old build would apply an export that the new build refuses.

## Precondition: fixed-key consumers of PFTL finality

Establish this **before the amendment is proposed**. From the activation height on, every block takes the new block-id domain and signed block fields, also a block with no lane export: its header carries the empty root (`crates/ordering_fast/src/consensus_v2.rs:297-319`, `:1376-1378` at `5bff3138a`). A proof program built before this change computes only the old block ids and refuses a block whose id does not match. In the retained release `pfeth-v1-20260904`, source `c9407b1c2`, the domain choice knows three domains only (`crates/ordering_fast/src/consensus_v2.rs:264-281`) and the recomputed id must equal the proposal's (`:396-399`). A verifier contract that pins such a program's key cannot be given a new program. A route whose Ethereum-, Base- or Arc-side verifier pins a pre-change PFTL finality program, on the chain being activated, could not prove any block finalized after the activation, and its withdrawals would stop being provable. This is conditional, and it was confirmed at the encoder level (the new and the old encoding give different block ids for the committed lane witness), not by running an old program.

For every fixed-key consumer of PFTL finality on the chain being activated, record: the deployed verifier contract (Ethereum, Base or Arc), the program key it pins, the source commit of that program, and whether that source knows the block-id format with the export root. For each one that does not, one of these must hold before the amendment is proposed:

- the route is drained and retired;
- the route is moved to a verifier whose program accepts both formats; or
- a recorded run shows that the deployed program proves a block finalized after an activation on a rehearsal chain.

No activation while one such route can still owe a withdrawal.

### First inventory, from the repository (2026-11-11)

The domain was introduced by `b2747f396` (2026-11-10) and is defined at `crates/ordering_fast/src/consensus_v2.rs:29-30`. A source commit that does not descend from `b2747f396` cannot know it. Whether each verifier pins the PFTL chain being activated, and whether its route can still owe a withdrawal, is not established here.

| Program (key) | Deployed verifier | Knows the export-root block id | Shown by |
|---|---|---|---|
| pfusdc-egress `pfeth-v1-20260904` (`0x00140f09…`) | Ethereum mainnet pfETH route: `finalityVerifier` `0xBCF1Aab5…` (`deployments/pfeth-eth-mainnet-20260904/deployment-consumer.json:4`, key `:20`); whether it is live is not established | no | source `c9407b1c2` (`programs/pfusdc-egress/releases.json:21`) does not descend from `b2747f396`; its `consensus_v2.rs:264-281`, `:396-399` |
| pfusdc-egress `arc-v2-20260902` (`0x0036cbe7…`) | none recorded: the current-v2 Arc pair was still to be deployed (`docs/evidence/arc-mvp-20260828/deployments.md:19-21`) | no | source `b945bbb32` (`programs/pfusdc-egress/releases.json:11`) does not descend from `b2747f396`; its `consensus_v2.rs:264`, `:396` |
| pfusdc-egress, committed ELF (`programs/pfusdc-egress/elf/pfusdc-egress-program`) | — | no | its strings name the `v2`, `bridge-exit-root.v1` and `pftl-uniswap-receipt-root.v1` block-id domains, not `external-lane-export-root.v1` |
| pfUSDC Tier-4 egress, August key (`0x00c8d744…`) | Arc testnet `0xC59EBED2…`, `routeEpoch` 7 (`docs/evidence/arc-mvp-20260828/deployments.md:13-14`) | no | the key was deployed in August 2026, before `b2747f396`; source commit not recorded |
| pfUSDC egress epochs 3 and 4 (`0x007a22f1…`) | Ethereum mainnet `0x31a89b52…` and `0xa77d5af4…` (`deployments/pfusdc-eth-mainnet-20260726/manifest.mainnet-epoch3.json:5`, `deployments/pfusdc-eth-mainnet-20260726-epoch4/manifest.mainnet-epoch4.json:5`; key `deploy-consumer.mainnet-epoch3.json:21`); PFTL chain `postfiat-wan-devnet-2` (`manifest.mainnet-epoch3.json:59`) | no | deployed 2026-07-26, before `b2747f396`; source commit not recorded |
| pfUSDC egress epoch 5 (`0x0026a156…`) | Ethereum mainnet `0x9a45d6f1…` (`deployments/pfusdc-eth-mainnet-20260728-epoch5/manifest.mainnet-epoch5.json:5`) | no | deployed 2026-07-28, before `b2747f396`; source commit not recorded |
| pfUSDC egress epoch 6 (`0x0015b046…`) | Ethereum mainnet `0xd2191bdf…` and the successor `0xa53926f0…` (`deployments/pfusdc-eth-mainnet-20260809-epoch6/manifest.mainnet-epoch6.json:5`, `deployments/pfusdc-eth-mainnet-20260809-epoch6-successor/manifest.mainnet-epoch6.json:5`) | no | deployed 2026-08-09, before `b2747f396`; source commit not recorded |
| A666 export (`0x004e44ac…`) | Ethereum mainnet `0xb79FF97E…` (`deployments/a666-mainnet-20260727/ethereum/deployment-state.json:6`, key `:88`) | not established | deployed 2026-07-27, before `b2747f396`; whether this program verifies PFTL block ids is not recorded |
| pftl-uniswap-receipt (`0x003af40e…`, `programs/pftl-uniswap-receipt/program-info.json:8`) | none recorded | no | the committed ELF's strings name the three older block-id domains only |
| Sepolia pfUSDC rehearsals (`deployments/pfusdc-eth-sepolia-20260725/`, `deployments/pfusdc-tier4-*-202607*/`) | Ethereum Sepolia | no | deployed July 2026, before `b2747f396` |
| pft-lane-export | not on main; on its branch only rehearsed on a local fork | not established | — |

The proof public-input inventory (`docs/status/OPEN-SOURCE-PROOF-PUBLIC-INPUT-INVENTORY-20260716.json`) lists one PFTL finality consumer, the bridge finality egress (`:750`); its other entries verify on PFTL or are not production proofs. Every "no" row whose verifier pins the chain being activated needs one of the three outcomes above.

**The general rule.** Any future change to what a block id covers needs the same inventory before its activation. A new long-lived verifier should pin a program that accepts the block-id formats of every activation that is already scheduled.

## Order

1. Deploy the new build and restart all validator nodes and all signer processes ([Safe Validator Rollout](safe-validator-rollout.md)).
2. Check the build version on every validator and signer. Check that every validator reports the same tip height, tip hash and state root, and that `verify-blocks` passes.
3. Propose and ratify the amendment with a height well after the ratification block. Leave enough blocks to stop it if a check fails:

   ```bash
   postfiat-node ratify-external-lane-export-root-activation-height \
     --validators CSV --height H --amendment-file amendment.json
   ```

   Sign it with `governance-authorization-sign` on each validator, assemble it with `governance-amendment-assemble`, and submit it through the governance round. A height at or before the amendment's own block is refused with `invalid_external_lane_export_root_activation_height`.
4. Confirm that the governed activation height is `H` on every validator.
5. Watch the block at `H - 1`: no root in the header, and an export is still refused with `external_lane_export_root_not_active`.
6. Watch the first block at `H`: the header carries `external_lane_export_root`, the same value on every validator. A block without exports carries the empty value.

## What to watch

- Signer refusals naming an `unsupported … tuple`. This means a signer runs an old build.
- Proposal or vote mismatches around `H`.
- A root that differs between validators at the same height.
- Tip divergence between validators.
- `verify-blocks` on each validator after `H`. It recomputes the root from the replayed exports.

## The way back

- **Before the amendment's block:** do not submit it, or ratify it with `--paused` or a `--veto-until-height`. A paused amendment is refused (`governance_amendment_paused`) and is ignored by the activation height.
- **After the amendment's block, before `H`:** the governed activation height is the lowest unpaused amendment value (`GovernanceState::external_lane_export_root_activation_height`). A later amendment cannot postpone or cancel it. Stop before `H` only with a release.
- **At or after `H`:** there is no way back short of a release. The headers already carry the root and their hashes depend on it.
