# Arbitrum Contracts Code Review - 2026-06-19

Scope reviewed:

- `crates/ethereum-contracts/src/ERC20BridgeVault.sol`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol`
- `crates/ethereum-contracts/src/NAVGuardHook.sol`
- Related Foundry tests and deployment helper code where needed for context.

Read-only review. No contract code was changed.

Verification command:

- `cd crates/ethereum-contracts && forge test`
- Result: `49 passed; 0 failed; 0 skipped`

## Summary

The bridge has the right high-level shape for a controlled launch: deposits are event-based, withdrawal recipients are included in the signed packet digest, claims pay the packet recipient directly, local double-claim and burn-replay guards exist, and the verifier enforces threshold signatures, sorted unique signers, and low-s ECDSA.

The main unresolved security risks are liveness and replay-domain risks:

1. A zero-cost challenge can permanently freeze an otherwise valid withdrawal.
2. Withdrawal packets are not bound to a specific vault/token contract, so a verifier acceptance can be replayed across vault deployments if the same verifier/asset tuple is reused.
3. Accepted withdrawals can expire into an unrecoverable state.
4. `NAVGuardHook.sol` is a dependency-light v4-shaped adapter, not a real Uniswap v4 hook, and its observation commitment path is not yet fully self-contained for trustless PFTL replay.

## Findings

### F-01 - Critical - Any account can permanently freeze a submitted vault withdrawal

Location:

- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:271`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:301`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:306`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:315`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:321`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:334`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:344`

Issue:

`submitWithdrawal` records `pending_id_by_burn_tx[burn_commitment] = pending_id` as soon as the withdrawal is submitted. Then `challengeWithdrawal` lets any account mark the pending withdrawal as `Challenged` without a bond, evidence hash, permission check, or adjudication path. `finalizeWithdrawal` converts any challenged withdrawal into `Frozen`. `claimWithdrawal` only pays `Accepted` withdrawals.

Concrete failure mode:

1. A relayer submits a valid, verifier-accepted withdrawal.
2. An attacker or censoring operator immediately calls `challengeWithdrawal(pending_id, AnyFault)`.
3. After the delay, anyone can finalize it to `Frozen`.
4. The burn transaction commitment remains locked in `pending_id_by_burn_tx`.
5. The recipient cannot claim, and the same burn cannot be resubmitted.

This means a valid withdrawal can be permanently custody-withheld by any account for the cost of one challenge transaction. This is a bridge-fund availability failure.

Recommended fix:

- Do not let an unauthenticated challenge permanently consume the burn replay slot.
- Require a challenge bond and an objective challenge payload, or route challenges to a bounded adjudication process.
- Allow a frozen/challenged withdrawal to be superseded after the challenge is rejected or times out.
- Consider setting the burn replay lock only when a withdrawal is accepted or claimed, or track `burn_tx -> terminal outcome` with an explicit retry path for invalid challenges.

### F-02 - High - Any account can freeze a valid threshold-signed verifier proof

Location:

- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:178`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:186`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:192`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:204`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:215`

Issue:

`challengeProof` is permissionless and does not require evidence. `finalizeProof` freezes any challenged proof, and frozen proofs never satisfy `isWithdrawalAccepted`.

Concrete attack:

An attacker watches for `ProofSubmitted`, calls `challengeProof` with any enum value, and the valid threshold-signed proof freezes after the challenge delay. If the system relies on resubmission at another finality height, the attacker can repeat the same griefing attack. This is not direct theft, but it is a zero-cost bridge exit denial vector.

Recommended fix:

- Require objective challenge evidence and a bond.
- Add a way to reject invalid challenges and accept the original proof.
- If the intended controlled-launch behavior is "any dispute freezes," document that withdrawals are censorable during launch and add an operator/manual recovery path.

### F-03 - High - Withdrawal packet digest is not scoped to the vault or token

Location:

- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:141`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:144`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:362`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:365`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:415`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:418`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:262`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:270`

Issue:

`withdrawalPacketDigest` commits to the PFTL chain id, bridge asset id, burn id, withdrawal id, recipient, amount, source bucket, destination hash, finalized height, and evidence root. It does not commit to:

- `block.chainid`
- `address(this)` for the vault
- `address(token)`
- the verifier address
- a source-domain string such as `erc20_bridge_vault:<chain_id>:<vault>:<token>`

`PFTLWithdrawalVerifier.proofDigest` commits to `block.chainid` and the verifier address, but not to the vault address or token. If two funded vaults share the same verifier and `vault_bridge_asset_id`, a proof accepted once by the verifier can satisfy `isWithdrawalAccepted` for both vaults. Each vault has its own local `claimed_withdrawal_id`, so the same PFTL burn can pay out more than once across vault instances.

Concrete attack/failure mode:

1. A migration or deployment error creates two vault contracts for the same bridge asset and verifier.
2. A legitimate withdrawal proof is accepted by the shared verifier.
3. The same packet is submitted to both vaults.
4. Both vaults see the proof as accepted and both can pay the recipient because replay state is local to each vault.

Recommended fix:

- Include the source EVM chain id, vault address, token address, and verifier address directly in `withdrawalPacketDigest`.
- Alternatively, make `destination_hash` a required on-chain recomputation over `chain_id`, `vault`, `token`, and `recipient`, but direct digest binding is cleaner.
- Add a cross-vault replay Foundry test with two vaults sharing one verifier.

### F-04 - High - Accepted withdrawals can expire into an unrecoverable state

Location:

- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:271`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:301`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:321`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:340`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:353`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:354`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:391`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:192`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:209`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:226`

Issue:

The verifier and vault both have execution windows. Once a vault withdrawal reaches `Accepted`, `claimWithdrawal` reverts after `expires_at`. There is no function to reopen, cancel, supersede, or requeue an expired accepted withdrawal. The burn replay slot was already consumed at submit time, so retrying the same burn is blocked.

Concrete failure mode:

If the claimant/keeper misses the claim window, the withdrawal remains `Accepted` but unclaimable. The user's PFTL-side burn has already happened, and the vault-side burn replay guard blocks a clean retry.

Recommended fix:

- Prefer no claim expiry once a withdrawal is accepted; the packet already binds the recipient and amount.
- If expiry is required, add an explicit recovery path: expired accepted withdrawals can be revalidated, resubmitted, or force-paid to the same recipient.
- Reject `finalizeWithdrawal` and `finalizeProof` after expiry instead of creating accepted-but-unusable states.

### F-05 - Medium - Pause does not stop pending withdrawal finalization or claims

Location:

- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:214`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:259`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:321`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:344`

Issue:

`setPaused` blocks deposits and new withdrawal submissions, but `finalizeWithdrawal` and `claimWithdrawal` do not check `paused`.

Concrete failure mode:

If a bad withdrawal is already pending when operators detect a verifier/signing incident, pausing the vault does not stop finalization or claim. Operators must race to challenge before finalization. Once accepted, pause cannot prevent payout.

Recommended fix:

- Split pause controls by path: deposits, submissions, finalization, claims.
- Add an emergency freeze for pending withdrawals, or make pause block finalization/claim while preserving a governed escape hatch for known-good withdrawals.

### F-06 - Medium - Controlled-launch verifier is signer-trusted, not trustless PFTL finality

Location:

- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:139`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:146`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:262`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:281`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:283`

Issue:

The verifier proves only that the configured signer threshold signed the EVM proof digest. It does not prove PFTL consensus, ledger inclusion, or burn finality on chain. That may be acceptable for controlled launch, but it means the signer threshold can authorize an arbitrary packet if enough signers collude or are compromised.

Concrete failure mode:

A compromised signer threshold signs a packet for a burn that did not occur, or for a manipulated amount/recipient. The vault accepts it because `isWithdrawalAccepted(packet_digest, hash_commitment)` is true.

Recommended fix:

- Keep this explicitly documented as controlled-launch trust.
- For the public trust-minimized bridge, replace or supplement signer attestations with a PFTL light-client/finality proof, fraud-proof challenge game with adjudication, or cryptographic proof of the finalized burn packet.
- Include signer-set epoch and PFTL finality certificate metadata in the signed domain.

### F-07 - Medium - Signer rotations do not invalidate already-submitted proofs

Location:

- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:128`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:135`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:139`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:146`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:192`

Issue:

Signatures are checked only when `submitProof` is called. If signers are removed or threshold is raised after a proof is submitted but before it is finalized, the old proof can still finalize.

Concrete failure mode:

Operators rotate out a compromised signer after a bad proof is submitted. The proof remains pending and can finalize unless someone challenges it in time.

Recommended fix:

- Include a signer-set epoch/root in `proofDigest`.
- Store the signer-set epoch used at submission.
- On emergency signer rotation, allow owner/governance to freeze pending proofs from the old compromised epoch.

### F-08 - Medium - `NAVGuardHook.sol` is not an actual Uniswap v4 hook implementation

Location:

- `crates/ethereum-contracts/src/NAVGuardHook.sol:4`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:6`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:8`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:209`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:214`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:258`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:263`
- `crates/ethereum-contracts/test/NAVGuardHook.t.sol:183`
- `crates/ethereum-contracts/test/NAVGuardHook.t.sol:190`

Issue:

The contract says it is "Uniswap v4-shaped" and intentionally avoids external v4 dependencies. Its callback signatures are custom (`beforeSwap(bytes32)`, `afterSwap(SwapObservationInput)`, etc.), and the tests use a `MockPoolManager` wrapper. A real Uniswap v4 `PoolManager` will not call these custom functions as native hook callbacks.

Concrete failure mode:

If this contract is deployed as the NAVCoin venue hook without a real adapter, the expected observations/checkpoints will not be produced by actual v4 pool activity. If an adapter is used, the trust boundary moves to that adapter.

Recommended fix:

- Implement the actual Uniswap v4 hook interface or inherit the canonical v4 hook base.
- Make tests use the real v4 callback shapes or a faithful local PoolManager harness.
- If an adapter remains intentional, name it as a trusted component and bind its code hash into the market-ops policy.

### F-09 - Medium - NAVGuard observations trust caller-supplied market data

Location:

- `crates/ethereum-contracts/src/NAVGuardHook.sol:116`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:135`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:214`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:221`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:228`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:230`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:231`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:311`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:326`

Issue:

Only `pool_manager` can call observation functions, but the hook accepts price, volume, fee, liquidity, and deltas as calldata from that caller. It does not independently derive those values from canonical v4 pool state.

Concrete failure mode:

A malicious or faulty configured manager/adapter records false discounts, premiums, liquidity, or depth. Those observations can feed the PFTL venue evidence path and distort market-ops reserve deployment or mint caps.

Recommended fix:

- Derive price, volume, fee, and liquidity from the real v4 callback data and pool state.
- Bind `pool_id` to a canonical `PoolKey` hash at registration.
- Include adapter/pool manager code hash and deployed address in the PFTL evidence policy if any adapter remains.

### F-10 - Medium - NAVGuard commitment events are not fully self-describing for replay

Location:

- `crates/ethereum-contracts/src/NAVGuardHook.sol:85`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:89`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:95`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:239`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:241`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:243`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:333`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:335`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:357`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:389`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:408`
- `crates/ethereum-contracts/src/NAVGuardHook.sol:409`

Issue:

`_swapObservationHash` commits to `amount0_delta` and `amount1_delta`, but `SwapObservationRecorded` does not emit those deltas. The ring buffer stores full observations on chain, but old entries are overwritten. Checkpoint events emit roots/counts, not the full leaf preimages.

Concrete failure mode:

PFTL replay from headers, receipts, logs, and checkpoints cannot recompute every committed swap leaf from `NAVGuardHook` logs alone. It must either trust the emitted `observation_hash`, rely on separate v4 logs, or read non-persistent ring-buffer storage before overwrite.

Recommended fix:

- Emit every field that is part of each observation hash, including deltas and any pool config identifier needed for recomputation.
- Add first/last sequence numbers to checkpoints.
- Ensure the PFTL replay bundle contains canonical v4 pool events or full leaf preimages, not only hook-level roots.

### F-11 - Low - Deposits assume requested amount equals received amount

Location:

- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:238`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:245`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol:246`

Issue:

`deposit` emits and commits the requested `amount` after calling `transferFrom`, but it does not check the vault balance delta. Native Arbitrum USDC should not be fee-on-transfer, so this is low severity for the current asset. The generic vault would be unsafe for fee-on-transfer or deflationary ERC20s.

Concrete failure mode:

A token transfers less than `amount` but returns success. PFTL relays the event amount and mints more bridge asset than the vault actually received.

Recommended fix:

- For generic ERC20 support, check `balanceAfter - balanceBefore == amount`.
- If only exact-transfer tokens are supported, enforce and document that token admission rule.

## Positive Confirmations

- Deposit ids include `block.chainid`, vault address, token address, depositor, amount, recipient hash, and nonce: `ERC20BridgeVault.sol:396`.
- Duplicate deposits are rejected before event replay can mint twice: `ERC20BridgeVault.sol:240`.
- Withdrawal recipient and amount are included in the packet digest: `ERC20BridgeVault.sol:415`.
- Claim pays `packet.recipient`; the caller cannot substitute a recipient at claim time: `ERC20BridgeVault.sol:362`.
- Claim sets `Claimed` and marks `claimed_withdrawal_id` before the ERC20 transfer, with a non-reentrancy guard: `ERC20BridgeVault.sol:344`, `ERC20BridgeVault.sol:363`, `ERC20BridgeVault.sol:365`.
- Local burn replay is blocked by `pending_id_by_burn_tx`: `ERC20BridgeVault.sol:271`, `ERC20BridgeVault.sol:273`, `ERC20BridgeVault.sol:301`.
- Verifier proof digest includes EVM chain id and verifier address: `PFTLWithdrawalVerifier.sol:262`.
- Verifier enforces signer membership, sorted unique signers, and low-s ECDSA: `PFTLWithdrawalVerifier.sol:321`, `PFTLWithdrawalVerifier.sol:331`, `PFTLWithdrawalVerifier.sol:353`.
- Rust-side withdrawal construction validates that `destination_ref` maps to the packet recipient and destination hash before producing the packet: `crates/types/src/lib_parts/ledger_assets.rs:1693`, `crates/types/src/lib_parts/ledger_assets.rs:1823`, `crates/types/src/lib_parts/ledger_assets.rs:1827`.
- `NAVGuardHook` does tie into the market-ops concept through pool registration, `pool_config_hash`, `pftl_state_hash`, observation roots, and checkpoint events, but the real v4 integration and replay completeness issues above remain unresolved.

## Recommended Triage Order

1. Fix F-01 and F-02 before increasing bridge value. The current challenge path is a zero-cost permanent withdrawal freeze.
2. Fix F-03 before deploying additional vaults, migrations, or parallel bridge assets that might share a verifier.
3. Fix F-04 before relying on unattended bridge exits.
4. Decide whether F-08/F-09/F-10 are acceptable as controlled-launch adapter trust, or replace the hook with a real v4 implementation before using venue evidence for automatic caps.
5. Add regression tests for challenge griefing, cross-vault replay, expired accepted withdrawal recovery, pause behavior, real v4 hook callback compatibility, and hook-log replay completeness.
