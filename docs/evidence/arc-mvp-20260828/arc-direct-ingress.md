# Arc direct-ingress contract adaptation

## Why an adaptation was required

The Tier-4 `ERC20BridgeVaultV2` inherited from the Arbitrum worktree committed deposits with `ArbSys.sendTxToL1`, and `PfUsdcIngressAnchorV1` authenticated an Ethereum Arbitrum outbox. Arc has neither Nitro's `ArbSys` transport nor an Ethereum outbox. Deploying that unchanged bytecode on Arc would make every deposit revert or would require an ungoverned transport shim.

The Arc ingress proof specified for this MVP is receipt-native: Arc consensus finalizes the EVM header, the header authenticates `receiptsRoot`, and the proof opens the vault's deposit log. No cross-chain message transport is needed to authenticate ingress.

## Versioned behavior

The existing constructor ABI is retained, with an explicit sentinel configuration:

- `arbSys != address(0)`: original Nitro behavior. The vault calls `sendTxToL1`, and the anchor authenticates the active Arbitrum outbox and L2 sender.
- `arbSys == address(0)`: Arc direct behavior. The vault calls the configured `PfUsdcIngressAnchorV1` in the same transaction.
- `bridge != address(0)` in the anchor: original Nitro outbox authentication.
- `bridge == address(0)` in the anchor: Arc direct authentication; only the exact pinned vault may call it.

Both contracts expose the immutable `directIngress` readback so a deployment can be checked without interpreting constructor history.

## Arc deposit transition

For `depositV2(amount, recipient, nonce, routeBinding)`:

1. The vault rejects pause, zero amount, invalid recipient length, zero route binding, and duplicate deposit IDs.
2. It derives `depositId` from the canonical domain, chain ID, vault, token, depositor, amount, recipient hash, nonce, and route binding.
3. It pulls exactly `amount` token atoms and rejects any non-exact balance delta.
4. It emits `ERC20BridgeDepositedV2` with all proof inputs.
5. In Arc mode it calls the local anchor with the same canonical fields.
6. The anchor authenticates `msg.sender == pinnedVault`, checks chain/vault/token/route bindings, recipient integrity, nonzero fields, independently recomputes `depositId`, consumes its replay key, and emits `Tier4DepositRecorded`.
7. Any failure in steps 3-6 reverts the complete transaction, including token balances and both replay maps.

The Arc ingress circuit will authenticate the vault's `ERC20BridgeDepositedV2` receipt directly. The local anchor is defense in depth and the governed route record required by Workstream 2; it is not substituted for the Arc finality proof.

## Circular address resolution

The anchor must pin the vault before the vault exists, while the vault constructor must pin the anchor. `ArcPfUsdcDeploymentFactory` resolves this without a mutable setter:

- a fresh factory's first `CREATE` address is the anchor;
- its second `CREATE` address is the vault;
- the factory computes both addresses from its own address and the fixed nonces 1 and 2;
- only the EOA that deployed the factory may invoke the one-shot `deploy` function;
- both observed addresses must equal their predictions or the transaction reverts.

The resulting anchor and vault have no factory-controlled upgrade or setter path. Vault ownership is assigned directly to the configured owner.

## Verification

`forge test --match-contract PFUSDCTier4Test -vv` passes 18 tests, including:

- the existing Nitro send and outbox paths;
- the Arc one-shot address prediction and deployment;
- rejection of a non-deployer factory caller and factory replay;
- a successful exact `1_000_000` atom direct deposit;
- direct-anchor vault authentication;
- wrong-route atomic rollback of wallet balance, vault balance, and replay state;
- existing proof-nullifier, committee transition, pause, fee-token, malformed-public-value, and SP1-rejection cases.

`forge build --sizes` reports runtime sizes of 5,284 bytes for `ERC20BridgeVaultV2`, 3,084 bytes for `PfUsdcIngressAnchorV1`, and 10,810 bytes for the one-shot factory, all below EVM runtime and initcode limits.
