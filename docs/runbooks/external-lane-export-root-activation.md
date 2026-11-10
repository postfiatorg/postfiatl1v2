# External Lane Export Root Activation

This runbook switches on `external_lane_export_root`, the block-header root over a block's lane export packets, on a running network. From its activation height every header carries the root, and the block hash, proposal hash, vote message and consensus v2 block id take their new versions (`postfiat.block.v5` and the others). Before the height `external_lane_export_v1` is refused with `external_lane_export_root_not_active`. The design is the step-2 section of `docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md`.

It is a consensus migration. Do not introduce it with an uncoordinated rolling restart.

## New chain

Set the genesis field `external_lane_export_root_activation_height` (for example `1`) and nothing else. `init` refuses a genesis whose height is one at which consensus v2 is not active.

## Preconditions on a running network

- Every validator node **and** every validator signer process runs a build that knows the root. An older signer refuses to sign a block that carries it, because its guard does not know the `.v5` vote and proposal messages. An older node fails to verify the new encodings.
- Consensus v2 is active at the chosen height. The amendment is refused at admission, batch building, execution and replay with `external_lane_export_root_activation_requires_consensus_v2` otherwise.
- No lane with `exports_enabled: true` can be reached by an old binary. Old code accepts an export without the new gate, so a node on an old build would apply an export that the new build refuses.

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
