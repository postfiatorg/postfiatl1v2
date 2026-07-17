# pfUSDC / Vault Bridge Handoff - 2026-06-19

## Purpose

This is a handoff for the bridge work requested as:

> Build the bridge so we have USDC on PFTL that can be swapped.

The intended end state is:

1. A source-chain vault on Arbitrum holds native USDC.
2. `deposit(amount, pftlRecipient, nonce)` transfers USDC into the vault and emits a canonical event.
3. PFTL only mints/counts the PFTL-side bridge asset from verified source-chain vault deposit evidence.
4. A PFTL holder can swap that bridge asset through existing PFTL rails.
5. A holder can burn the bridge asset to create a finalized withdrawal packet.
6. The source-chain vault accepts that PFTL withdrawal packet through proof/challenge/finality.
7. The user claims USDC directly from the source-chain vault.
8. Operators can relay/propose, but cannot invent deposits, choose withdrawal recipients, or custody-withhold once the source-chain contract accepts a valid withdrawal.

## Current Branch And Worktree

- Repo: `$POSTFIAT_REPO`
- Branch at handoff: `navcoin-market-ops-envelope`
- Worktree is dirty and large. Nothing has been committed for the latest bridge changes.
- There are many unrelated or earlier-task changes in the same dirty tree. Do not assume every changed file is part of the latest bridge increment.
- No `cargo test`, `forge test`, `forge build`, `postfiat-node`, or `anvil` process was running when this handoff was written.

Important dirty/untracked files related to this bridge:

- `crates/ethereum-contracts/src/ERC20BridgeVault.sol`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol`
- `crates/ethereum-contracts/test/ERC20BridgeVault.t.sol`
- `crates/ethereum-contracts/test/PFTLWithdrawalVerifier.t.sol`
- `crates/ethereum-contracts/script/DeployERC20Bridge.s.sol`
- `crates/ethereum-contracts/script/erc20-bridge.env.example`
- `crates/ethereum-contracts/script/README.md`
- `crates/types/src/lib_parts/core_chain.rs`
- `crates/types/src/lib_parts/ledger_assets.rs`
- `crates/types/src/lib_parts/transactions_mempool_receipts.rs`
- `crates/execution/src/lib_parts/nft_escrow_asset_state.rs`
- `crates/execution/src/lib_parts/vault_bridge_policy.rs`
- `crates/node/src/lib_parts/part_01.rs`
- `crates/node/src/lib_parts/part_02.rs`
- `crates/node/src/main_parts/cli_dispatch.rs`
- `crates/node/src/main_parts/runtime_helpers.rs`
- `crates/node/src/main_parts/tests.rs`
- `crates/node/src/node_types.rs`
- `docs/specs/vault-bridge-navcoin-profile.md`

## Important Design Decision

The implementation currently uses a generic `ERC20BridgeVault`, not a compiled contract named `PfUSDCVault`.

This was intentional after the concern that hardcoding `pfUSD` / `pfUSDC` / Arbitrum / USDC into L1 or source-chain contract code was wrong. The current model is:

- The contract is generic: `ERC20BridgeVault`.
- The deploy script is generic: `DeployERC20Bridge.s.sol`.
- The PFTL asset code can be `PFUSDC` or another code at bootstrap time.
- The source token can be native Arbitrum USDC by environment configuration.
- The L1 consensus path is named `vault_bridge_*`, not `pfusd_*`.

If product requirements demand a literal `PfUSDCVault.sol` wrapper for UX or audit packaging, that should be a very thin source-chain deployment wrapper only. It should not introduce token-specific logic into PFTL consensus/state. As of this handoff, that wrapper was removed.

## What Has Been Built

### 1. Source-Chain Vault Contract

File: `crates/ethereum-contracts/src/ERC20BridgeVault.sol`

Implemented behavior:

- Holds a configured ERC-20 token.
- `deposit(uint256 amount, string pftl_recipient, bytes32 nonce)` transfers tokens from depositor into vault.
- Emits `ERC20BridgeDeposited(...)` with canonical fields:
  - `deposit_id`
  - depositor
  - `pftl_recipient_hash`
  - plaintext PFTL recipient
  - amount
  - nonce
  - source chain id
  - vault address
  - token address
- Rejects duplicate deposits by `deposit_id`.
- Withdrawal path:
  - `submitWithdrawal(packet, pftl_withdrawal_hash)` requires `PFTLWithdrawalVerifier` acceptance for the exact packet digest and PFTL hash commitment.
  - Applies a vault challenge window.
  - Challenged withdrawals freeze and cannot pay.
  - Finalized accepted withdrawals become claimable.
  - `claimWithdrawal(pending_id)` pays the ERC-20 directly to packet recipient.
  - Rejects burn replay and duplicate withdrawal ids.

### 2. Source-Chain PFTL Withdrawal Verifier

File: `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol`

Implemented behavior:

- Stores a configured signer set and threshold.
- Computes a domain-separated proof digest bound to:
  - source `chainid`
  - verifier address
  - PFTL withdrawal packet EVM digest
  - PFTL withdrawal hash commitment
  - finalized PFTL height
- Verifies raw secp256k1 signatures over that digest.
- Enforces threshold, signer membership, sorted unique signer addresses, and low-s signatures.
- Applies challenge/finality window.
- Accepted proofs authorize exact packet/hash pairs for `ERC20BridgeVault`.
- Challenged proofs freeze and never authorize.

### 3. Generic Foundry Deployment

Files:

- `crates/ethereum-contracts/script/DeployERC20Bridge.s.sol`
- `crates/ethereum-contracts/script/erc20-bridge.env.example`
- `crates/ethereum-contracts/script/README.md`

Implemented behavior:

- Deploys `PFTLWithdrawalVerifier`.
- Deploys `ERC20BridgeVault`.
- All product parameters come from environment:
  - `SOURCE_CHAIN_RPC_URL`
  - `SOURCE_CHAIN_ID`
  - `ERC20_BRIDGE_TOKEN`
  - `PFTL_BRIDGE_OWNER`
  - `PFTL_CHAIN_ID`
  - `VAULT_BRIDGE_ASSET_ID`
  - `PFTL_WITHDRAWAL_SIGNERS`
  - thresholds and challenge windows

Current status:

- No hardcoded Arbitrum-native USDC contract remains in the contract source/deploy script.
- Native Arbitrum USDC should be configured as `ERC20_BRIDGE_TOKEN` when deploying the product.

### 4. PFTL Types And Ledger State

Files:

- `crates/types/src/lib_parts/core_chain.rs`
- `crates/types/src/lib_parts/ledger_assets.rs`
- `crates/types/src/lib_parts/transactions_mempool_receipts.rs`

Implemented concepts include:

- `VaultBridgeDepositEvidence`
- `VaultBridgeDepositRecord`
- `VaultBridgeReceipt`
- `VaultBridgeBucketState`
- `VaultBridgeAllocation`
- `VaultBridgeRedemption`
- `VaultBridgeWithdrawalPacket`
- `VaultBridgeDepositPropose`
- `VaultBridgeDepositAttest`
- `VaultBridgeDepositFinalize`
- `VaultBridgeDepositClaim`
- `VaultBridgeMintFromReceipts`
- `VaultBridgeReceiptCount`
- `VaultBridgeNavSubscriptionAllocate`
- `VaultBridgeBurnToRedeem`
- `VaultBridgeRedeemSettle`
- `VaultBridgeBucketImpair`

Important invariant:

- PFTL-side bridge supply is minted/counted from finalized source-chain vault deposit evidence, not operator-controlled balances.

### 5. PFTL Execution Path

Main file: `crates/execution/src/lib_parts/nft_escrow_asset_state.rs`

Implemented behavior includes:

- Deposit proposal validation.
- Deposit attestation/finalization.
- Deposit claim mints ordinary issued bridge asset to the committed recipient.
- Claimed deposit capacity is counted into source buckets.
- Duplicate deposit claims are rejected.
- The minted bridge asset can use existing PFTL asset rails, including the offer book.
- Burn-to-redeem burns the holder's bridge asset and creates a pending redemption with a deterministic withdrawal packet.
- Bucket outstanding supply and redemption queue accounting are updated.
- NAV reserve packet integration for vault-backed assets.
- Impairment logic for buckets.

### 6. Node / Operator Tooling

Files:

- `crates/node/src/lib_parts/part_02.rs`
- `crates/node/src/main_parts/cli_dispatch.rs`
- `crates/node/src/main_parts/runtime_helpers.rs`
- `crates/node/src/node_types.rs`

Implemented CLI / helper surfaces:

- `vault-bridge-asset-id`
  - Derives deterministic issued asset id before deploy/bootstrap.
- `vault-bridge-bootstrap-bundle`
  - Writes PFTL setup operations for profile, asset creation, NAV registration, and initial trustlines.
- `vault-bridge-deposit-intent`
  - Computes the source-chain approve/deposit commands and expected deposit id.
- `vault-bridge-deposit-plan`
  - Converts a vault event log or receipt file into PFTL deposit operations.
- `vault-bridge-deposit-relay-bundle`
  - Writes propose/attest/finalize/claim operation JSON plus quote/sign/submit commands.
- `vault-bridge-deposit-relay-rpc-bundle`
  - Fetches the EVM receipt using `cast receipt --json`, validates it, writes `source-receipt.json`, and produces the PFTL relay bundle.
- `vault-bridge-status`
  - Reports buckets, receipts, bridge deposits, allocations, redemptions, and withdrawal packet fields.
- `vault-bridge-receipts`
  - Receipt-focused status output.
- `vault-bridge-export-reserve-packet`
  - Exports replay bundle for source-root/counting verification.
- `vault-bridge-replay-reserve-packet`
  - Replays bundle and checks expected source root/counts.
- `vault-bridge-withdrawal-plan`
  - Reads a pending PFTL redemption and derives:
    - Solidity packet tuple
    - PFTL withdrawal hash
    - hash commitment
    - EVM packet digest
    - verifier pending proof id
    - vault pending withdrawal id
    - cast command arguments
- `vault-bridge-withdrawal-signature-bundle`
  - Writes `plan.json`, `signature-request.json`, empty `signatures.json`, and `commands.sh`.
  - Produces exact raw digest signers must sign with `cast wallet sign --no-hash`.
  - Generates a nested withdrawal relay bundle after signatures are collected.
- `vault-bridge-withdrawal-relay-bundle`
  - Writes source-chain staged commands:
    - `submit-proof`
    - `finalize-proof`
    - `submit-withdrawal`
    - `finalize-withdrawal`
    - `claim`
- `vault-bridge-burn-to-redeem-bundle`
  - Latest addition before this handoff.
  - Reads finalized PFTL ledger state.
  - Infers issuer, finalized epoch, reserve packet hash, and bucket when unambiguous.
  - Writes `burn-to-redeem.operation.json`.
  - Writes quote/sign/submit commands using `OWNER_KEY_FILE`.
  - Refuses ambiguous bucket selection.

## What Was Removed

Removed because it hardcoded product parameters into source-chain code:

- `crates/ethereum-contracts/src/PfUSDCVault.sol`
- `crates/ethereum-contracts/test/PfUSDCVault.t.sol`
- `crates/ethereum-contracts/script/DeployPfUSDCBridge.s.sol`
- `crates/ethereum-contracts/script/pfusdc-arbitrum.env.example`

Rationale:

- The user objected to token-specific hardcoding.
- The current generic vault can still deploy a USDC-backed PFTL asset by configuring source chain/token and choosing the PFTL asset code.

## Tests And Evidence Collected

Earlier green checks before the latest burn bundle addition:

- `cargo test -p postfiat-node vault_bridge -- --nocapture`
  - 10 passed.
- `cargo test -p postfiat-types vault_bridge -- --nocapture`
  - 7 passed.
- `cargo test -p postfiat-execution vault_bridge -- --nocapture`
  - 5 passed.
- `forge clean`
- `forge build`
  - Passed.
  - Foundry reported existing lint warnings for timestamp comparisons and checked casts in verifier/vault code.
- `forge test -vv`
  - 49 passed.
- `cargo fmt --check`
  - Passed at that point.
- `forge fmt --check`
  - Passed at that point.
- `git diff --check`
  - Passed at that point.
- Hardcode scan for `pfUSD`, `pfUSDC`, `PfUSDCVault`, `DeployPfUSDC`, native Arbitrum USDC wrapper names across touched L1/contract/spec paths:
  - Clean at that point.

Latest check after adding `vault-bridge-burn-to-redeem-bundle`:

- `cargo test -p postfiat-node vault_bridge_product_e2e_receipt_to_swap_burn_and_withdrawal_plan -- --nocapture`
  - Passed.
  - This test now exercises:
    - deposit receipt -> PFTL relay plan
    - PFTL mint/count into holder trustline
    - finalized reserve packet
    - offer-book swap to buyer
    - generated burn-to-redeem bundle
    - PFTL burn transaction using generated operation
    - withdrawal plan generation
  - Warning introduced: `VaultBridgeBurnToRedeemOperation` is now an unused import in `crates/node/src/main_parts/tests.rs`.

## Known Immediate Cleanup

1. Remove unused import in `crates/node/src/main_parts/tests.rs`.
   - `VaultBridgeBurnToRedeemOperation` became unused after the e2e test switched to the generated burn bundle.

2. Rerun formatting and full focused suites after cleanup:

   ```bash
   cargo fmt
   forge fmt
   cargo test -p postfiat-types vault_bridge -- --nocapture
   cargo test -p postfiat-execution vault_bridge -- --nocapture
   cargo test -p postfiat-node vault_bridge -- --nocapture
   forge build
   forge test -vv
   cargo fmt --check
   forge fmt --check
   git diff --check
   ```

3. Rerun hardcode scan:

   ```bash
   rg -n -i "pfusd|pfusdc|DeployPfUSDC|PfUSDCVault|native Arbitrum USDC|ARBITRUM_NATIVE_USDC|ARBITRUM_CHAIN_ID" \
     crates/types/src crates/execution/src crates/node/src \
     crates/ethereum-contracts/src crates/ethereum-contracts/script crates/ethereum-contracts/test \
     docs/specs/vault-bridge-navcoin-profile.md
   ```

   Expected result right now should be no matches, except if the next agent intentionally reintroduces product-profile docs.

## What Still Needs To Be Done To Fully Complete The Goal

This is not complete until it has been deployed and proven end-to-end with real source-chain USDC. Current work is strong scaffolding plus local proof, not a live product.

### A. Decide Product Naming / Wrapper Policy

Open decision:

- Keep `ERC20BridgeVault` only and configure it for native Arbitrum USDC.
- Or add a thin `PfUSDCVault.sol` wrapper strictly for deployment packaging.

Recommendation:

- Keep generic contract and deployment.
- If adding `PfUSDCVault.sol`, it must be a wrapper only and must not alter PFTL consensus or create a `pfusd_*` L1 path.

### B. Complete Source-Chain Deployment Readiness

Needs:

- Confirm target chain.
  - User previously mentioned Arbitrum USDC and Hyperliquid usage.
  - Current env example is generic and does not include the actual Arbitrum native USDC address.
- Fill deployment `.env`:
  - `SOURCE_CHAIN_RPC_URL`
  - `SOURCE_CHAIN_ID`
  - `ERC20_BRIDGE_TOKEN`
  - `PRIVATE_KEY`
  - `PFTL_BRIDGE_OWNER`
  - `PFTL_WITHDRAWAL_SIGNERS`
  - thresholds
  - challenge windows
- Derive `VAULT_BRIDGE_ASSET_ID`.
- Deploy `PFTLWithdrawalVerifier`.
- Deploy `ERC20BridgeVault`.
- Record deployed addresses.
- Verify deployed bytecode if required.

### C. Bootstrap PFTL Asset

Needs:

- Run `vault-bridge-bootstrap-bundle` against live PFTL data dir.
- Sign and submit:
  - `nav_profile_register`
  - `asset_create`
  - `nav_asset_register`
  - trustlines for holder/market participants if needed.
- Confirm via:

  ```bash
  postfiat-node vault-bridge-status --asset-id "$VAULT_BRIDGE_ASSET_ID"
  ```

### D. Prove Real Deposit Into Vault

Needs:

- User funds source-chain wallet with native USDC.
- User approves `ERC20BridgeVault`.
- User calls:

  ```text
  ERC20BridgeVault.deposit(amount, pftlRecipient, nonce)
  ```

- Relay transaction with:

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

- Sign/submit generated PFTL operations.
- Confirm the recipient receives the PFTL bridge asset.

### E. Prove Swapability In Live PFTL State

Local tests prove the bridge asset can be offered and filled through the PFTL offer book.

Still needed live:

- Create a real offer selling the PFTL bridge asset for PFT or another asset.
- Fill it from another account.
- Confirm:
  - maker bridge asset balance decreases
  - buyer bridge asset balance increases
  - offer fill receipt is emitted
  - supply/accounting remains valid.

### F. Prove Burn-To-Redeem In Live PFTL State

Needs:

- Run:

  ```bash
  postfiat-node vault-bridge-burn-to-redeem-bundle \
    --owner "$PFTL_HOLDER" \
    --asset-id "$VAULT_BRIDGE_ASSET_ID" \
    --amount-atoms "$WITHDRAW_AMOUNT_ATOMS" \
    --destination-ref "evm-erc20:$SOURCE_CHAIN_ID:$SOURCE_CHAIN_RECIPIENT" \
    --bundle burn-to-redeem-bundle
  ```

- Sign/submit the generated burn operation.
- Confirm a pending redemption appears in `vault-bridge-status`.

### G. Prove Source-Chain Withdrawal

Needs:

- Run:

  ```bash
  postfiat-node vault-bridge-withdrawal-signature-bundle \
    --asset-id "$VAULT_BRIDGE_ASSET_ID" \
    --redemption-id "$PFTL_REDEMPTION_ID" \
    --evm-chain-id "$SOURCE_CHAIN_ID" \
    --verifier-address "$PFTL_WITHDRAWAL_VERIFIER" \
    --bundle withdrawal-signature-bundle
  ```

- Have configured PFTL withdrawal signers sign the raw digest.
- Populate `signatures.json`.
- Build relay bundle:

  ```bash
  RUN_STAGE=relay-bundle bash withdrawal-signature-bundle/commands.sh
  ```

- On source chain, run:

  ```bash
  RUN_STAGE=submit-proof bash withdrawal-signature-bundle/relay-bundle/commands.sh
  # wait verifier challenge delay
  RUN_STAGE=finalize-proof bash withdrawal-signature-bundle/relay-bundle/commands.sh
  RUN_STAGE=submit-withdrawal bash withdrawal-signature-bundle/relay-bundle/commands.sh
  # wait vault challenge delay
  RUN_STAGE=finalize-withdrawal bash withdrawal-signature-bundle/relay-bundle/commands.sh
  RUN_STAGE=claim bash withdrawal-signature-bundle/relay-bundle/commands.sh
  ```

- Confirm the recipient receives source-chain USDC directly from `ERC20BridgeVault`.

### H. Trust-Minimization / Public-Claim Gap

Current launch path is controlled-launch and challenge/finality based.

Still needed for stronger public trustless claims:

- Source-chain receipt inclusion proof or light-client/SP1 path for deposits.
- Clear challenge rules and economic bond policy for false deposit proposals.
- Public monitoring around:
  - deposit proposals
  - verifier proofs
  - vault withdrawal submissions
  - challenge windows
- Operational runbook for signer rotation, pause, and compromised signer response.

### I. Documentation Cleanup

Current docs are useful but need review:

- `docs/specs/vault-bridge-navcoin-profile.md`
  - It includes implementation notes, MVP task list, and runbook sections. It should be cleaned into final spec vs implementation burndown.
- `crates/ethereum-contracts/script/README.md`
  - It now documents generic ERC20 bridge deployment and burn/withdrawal flow.

The next agent should separate:

- Product runbook: "How to deploy native Arbitrum USDC as a PFTL bridge asset."
- Protocol spec: generic vault bridge primitive.
- Operator emergency guide: pauses, challenges, signer issues, vault liquidity issues.

## Suggested Next Agent First Steps

1. Fix the unused import from the latest burn bundle test.
2. Run the focused test suite and hardcode scan listed above.
3. Review the generic-vs-`PfUSDCVault` decision with the user before adding any product-specific wrapper back.
4. If generic deployment is accepted, create a concrete deployment env file for native Arbitrum USDC using real addresses but do not commit secrets.
5. Run a local anvil deployment and scripted deposit/withdrawal simulation if possible.
6. Only then move to live Arbitrum funding/deploy.

## Current Honest Status

The bridge is no longer just internal ledger scaffolding. It now has:

- source-chain vault contract
- source-chain verifier contract
- PFTL deposit evidence path
- PFTL mint/count path
- PFTL swapability test
- PFTL burn-to-redeem path
- source-chain withdrawal packet planning
- signer digest bundle
- source-chain relay bundle for proof/finality/claim

But the goal is not fully complete because there is no live deployed Arbitrum USDC vault, no real USDC deposit, no live PFTL mint, no live swap, no live burn, and no live USDC claim from the vault yet.

