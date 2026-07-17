# ERC20 Bridge Deployment

This deploys one source-chain ERC20 vault plus the PFTL withdrawal verifier for a
single vault bridge asset. The source token and source chain are deployment
configuration; the contracts are asset-generic.

## Generic ERC20 vault

There is no product-specific vault contract in this package. A bridge asset is
defined by deployment parameters: source chain id, ERC20 token address, PFTL
issuer, asset code, precision, profile policy hash, verifier signer set, and
challenge windows. If operators want a USD-denominated bridge asset, they set
those values in `.env`; the compiled contracts do not know the asset name.

```bash
cd crates/ethereum-contracts
cp script/erc20-bridge.env.example .env
$EDITOR .env
set -a
. ./.env
set +a

postfiat-node vault-bridge-asset-id \
  --pftl-chain-id "$PFTL_CHAIN_ID" \
  --issuer "$PFTL_ISSUER" \
  --asset-code "$VAULT_BRIDGE_ASSET_CODE" \
  --asset-version "$VAULT_BRIDGE_ASSET_VERSION" \
  --env-file vault-bridge-asset.env \
  --overwrite
set -a
. ./vault-bridge-asset.env
set +a

forge script script/DeployERC20Bridge.s.sol:DeployERC20Bridge \
  --rpc-url "$SOURCE_CHAIN_RPC_URL" \
  --broadcast
```

Readiness checks before funding the vault:

- `ERC20_BRIDGE_TOKEN` is the intended source ERC20 contract on the selected
  chain.
- `VAULT_BRIDGE_ASSET_ID` is exactly 48 bytes and matches the PFTL issued asset
  registered under the vault bridge NAV profile.
- `PFTL_WITHDRAWAL_SIGNERS` is the configured PFTL finality signer set for
  withdrawal packets.
- `PFTL_WITHDRAWAL_THRESHOLD` cannot be met by the deployer alone unless that is
  the explicit controlled-launch policy.
- Both challenge windows are nonzero and long enough for operators/challengers
  to inspect packets before funds can move.

After deployment:

1. Register the PFTL profile with source domain
   `erc20_bridge_vault:<chain_id>:<vault_address>:<token_address>` and create
   the corresponding issued bridge asset:

   ```bash
   postfiat-node vault-bridge-bootstrap-bundle \
     --pftl-chain-id "$PFTL_CHAIN_ID" \
     --source-chain-id "$SOURCE_CHAIN_ID" \
     --vault-address "$ERC20_BRIDGE_VAULT" \
     --token-address "$ERC20_BRIDGE_TOKEN" \
     --issuer "$PFTL_ISSUER" \
     --asset-code "$VAULT_BRIDGE_ASSET_CODE" \
     --asset-version "$VAULT_BRIDGE_ASSET_VERSION" \
     --asset-precision "$VAULT_BRIDGE_ASSET_PRECISION" \
     --asset-display-name "$VAULT_BRIDGE_ASSET_DISPLAY_NAME" \
     --valuation-unit "$VAULT_BRIDGE_VALUATION_UNIT" \
     --valuation-policy-hash "$VAULT_BRIDGE_POLICY_HASH" \
     --trust-accounts "$INITIAL_TRUST_ACCOUNTS" \
     --bundle bootstrap-bundle
   ```
2. Users run `postfiat-node vault-bridge-deposit-intent` to prepare the exact
   source-chain approve/deposit calls and expected vault deposit id, then call
   `ERC20BridgeVault.deposit(amount, pftlRecipient, nonce)`.
3. Relayers fetch the source-chain transaction receipt from the configured EVM
   RPC and convert the validated `ERC20BridgeDeposited` event into PFTL
   `vault_bridge_deposit_*` operations:

   ```bash
   postfiat-node vault-bridge-deposit-relay-rpc-bundle \
     --source-rpc-url "$SOURCE_CHAIN_RPC_URL" \
     --tx-hash "$DEPOSIT_TX_HASH" \
     --vault-address "$ERC20_BRIDGE_VAULT" \
     --token-address "$ERC20_BRIDGE_TOKEN" \
     --asset-id "$VAULT_BRIDGE_ASSET_ID" \
     --policy-hash "$VAULT_BRIDGE_POLICY_HASH" \
     --proposer "$PFTL_RELAYER" \
     --attestor "$PFTL_ATTESTOR" \
     --expires-at-height "$PFTL_DEPOSIT_EXPIRES_AT_HEIGHT" \
     --bundle deposit-relay-bundle
   ```
4. Users burn the PFTL bridge asset with `vault_bridge_burn_to_redeem`. The
   burn bundle infers the finalized epoch, reserve packet, issuer, and source
   bucket from PFTL state when there is one unambiguous eligible bucket.
5. Relayers run `postfiat-node vault-bridge-withdrawal-signature-bundle` to
   publish the exact verifier digest that finality signers must sign.
6. After threshold signatures are collected in the generated `signatures.json`,
   relayers run the generated relay-bundle stage and submit the accepted packet
   through `PFTLWithdrawalVerifier` and `ERC20BridgeVault`.
7. The user claims directly from `ERC20BridgeVault` after the vault challenge
   window closes.

Example deposit intent:

```bash
postfiat-node vault-bridge-deposit-intent \
  --source-chain-id "$SOURCE_CHAIN_ID" \
  --vault-address "$ERC20_BRIDGE_VAULT" \
  --token-address "$ERC20_BRIDGE_TOKEN" \
  --depositor "$DEPOSITOR" \
  --amount-atoms 1000000 \
  --pftl-recipient "$PFTL_RECIPIENT" \
  --nonce "$DEPOSIT_NONCE" \
  --asset-id "$VAULT_BRIDGE_ASSET_ID" \
  --policy-hash "$VAULT_BRIDGE_POLICY_HASH" \
  --proposer "$PFTL_RELAYER" \
  --expires-at-height "$PFTL_DEPOSIT_EXPIRES_AT_HEIGHT" \
  --bundle deposit-relay-bundle
```

Example burn-to-redeem bundle:

```bash
postfiat-node vault-bridge-burn-to-redeem-bundle \
  --owner "$PFTL_HOLDER" \
  --asset-id "$VAULT_BRIDGE_ASSET_ID" \
  --amount-atoms "$WITHDRAW_AMOUNT_ATOMS" \
  --destination-ref "evm-erc20:$SOURCE_CHAIN_ID:$SOURCE_CHAIN_RECIPIENT" \
  --bundle burn-to-redeem-bundle

OWNER_KEY_FILE=holder.key bash burn-to-redeem-bundle/commands.sh
```

Example withdrawal signature and relay bundle:

```bash
postfiat-node vault-bridge-withdrawal-signature-bundle \
  --asset-id "$VAULT_BRIDGE_ASSET_ID" \
  --redemption-id "$PFTL_REDEMPTION_ID" \
  --evm-chain-id "$SOURCE_CHAIN_ID" \
  --verifier-address "$PFTL_WITHDRAWAL_VERIFIER" \
  --bundle withdrawal-signature-bundle

RUN_STAGE=sign \
PFTL_WITHDRAWAL_SIGNER_PRIVATE_KEY=0x... \
bash withdrawal-signature-bundle/commands.sh

# After signatures.json is populated with threshold signatures sorted by signer:
RUN_STAGE=relay-bundle bash withdrawal-signature-bundle/commands.sh
```
