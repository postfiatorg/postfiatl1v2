# NAV Roundtrip EVM Withdrawal Consolidation

Status: design proposal, not deployed
Date: 2026-06-21
Audience: Ethereum contracts engineer, PFTL bridge operator, StakeHub operator
Related:

- `docs/runbooks/nav-roundtrip-speedup-plan.md`
- `docs/status/arbitrum-contracts-code-review-2026-06-19.md`
- `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol`

## Purpose

Reduce the Arbitrum withdrawal segment of the full a651 <-> pfUSDC roundtrip
without weakening bridge safety.

The current full roundtrip average is about `120s`; the largest single segment
is the Arbitrum withdrawal path at about `43.6s`. The current path is:

```text
submitProof
wait verifier challenge window
finalizeProof
submitWithdrawal
wait vault challenge window
finalizeWithdrawal
claimWithdrawal
```

This proposal keeps both challenge windows visible and enforced, but removes
unnecessary transaction and receipt waits by adding combined methods to the
fixed F-01 through F-04 contract surface.

This is not a live deployment instruction. It is the contract-change spec to
implement only after an operator approves a new Arbitrum deployment and a fresh
small-value bridge battery.

## Current Security Baseline

The consolidation must preserve the source fixes from commit `59af43d9`:

| Finding | Required invariant |
| --- | --- |
| F-01/F-02 | Only owner or challenge authority can challenge proof/withdrawal records; an unauthenticated account cannot freeze a valid exit. |
| F-03 | Withdrawal packets bind source chain id, vault address, and token address so verifier acceptance cannot replay across vaults/tokens. |
| F-04 | An accepted withdrawal remains claimable after the execution window; a missed keeper does not strand funds. |

The current fixed source already exposes:

- `PFTLWithdrawalVerifier.submitProof(...)`
- `PFTLWithdrawalVerifier.finalizeProof(bytes32 pending_id)`
- `PFTLWithdrawalVerifier.isWithdrawalAccepted(bytes32 packet_digest, bytes32 hash_commitment)`
- `ERC20BridgeVault.submitWithdrawal(WithdrawalPacket, bytes pftl_withdrawal_hash)`
- `ERC20BridgeVault.finalizeWithdrawal(bytes32 pending_id)`
- `ERC20BridgeVault.claimWithdrawal(bytes32 pending_id)`

The new methods must be additive. Existing methods remain available for replay,
manual recovery, and backwards-compatible runner behavior.

## Proposed Contract Surface

### PFTLWithdrawalVerifier

Add a read helper:

```solidity
function getProofTimes(bytes32 pending_id)
    external
    view
    returns (uint64 posted_at, uint64 valid_after, uint64 expires_at);
```

This avoids runner-side event scraping when deciding whether a proof can be
finalized.

No combined verifier-only method is required. `submitProof` and
`finalizeProof` must remain separated by the verifier challenge window.

### ERC20BridgeVault

Add:

```solidity
function finalizeProofAndSubmitWithdrawal(
    bytes32 proof_pending_id,
    WithdrawalPacket calldata packet,
    bytes calldata pftl_withdrawal_hash
) external returns (bytes32 withdrawal_pending_id);
```

Semantics:

1. Call `PFTLWithdrawalVerifier.finalizeProof(proof_pending_id)`.
2. If the proof finalized to `Frozen`, revert or return no vault submission.
3. Recompute `hash_commitment = keccak256(pftl_withdrawal_hash)`.
4. Recompute `packet_digest = withdrawalPacketDigest(packet)`.
5. Require the verifier now reports `isWithdrawalAccepted(packet_digest, hash_commitment)`.
6. Execute the same internal logic as `submitWithdrawal`.
7. Emit the same `ProofAccepted` and `WithdrawalSubmitted` events as the
   separate path.

Add:

```solidity
function finalizeWithdrawalAndClaim(bytes32 withdrawal_pending_id) external;
```

Semantics:

1. Execute the same internal logic as `finalizeWithdrawal`.
2. If the withdrawal finalized to `Frozen`, revert or leave it frozen without
   attempting payment.
3. If the withdrawal finalized to `Accepted`, execute the same internal logic as
   `claimWithdrawal`.
4. Emit the same `WithdrawalAccepted` and `WithdrawalClaimed` events as the
   separate path.

The two combined methods should be thin wrappers around private internal
functions:

```solidity
function _finalizeProof(bytes32 pending_id) internal returns (ProofStatus);
function _submitWithdrawal(WithdrawalPacket calldata packet, bytes calldata pftl_hash)
    internal
    returns (bytes32);
function _finalizeWithdrawal(bytes32 pending_id) internal returns (WithdrawalStatus);
function _claimWithdrawal(bytes32 pending_id) internal;
```

The current public methods should call those same internal functions so the
separate and combined paths cannot drift.

## Challenge-Window Rule

The consolidation must not shorten either challenge window:

```text
submitProof
wait verifier challenge window
finalizeProofAndSubmitWithdrawal
wait vault challenge window
finalizeWithdrawalAndClaim
```

This removes two EVM transactions and two receipt waits:

- separate `finalizeProof` transaction;
- separate `claimWithdrawal` transaction.

It does not remove:

- verifier proof submission;
- verifier challenge delay;
- vault withdrawal submission;
- vault challenge delay;
- final claim transfer.

The runner must continue to include both waits in measured wall-clock time.

## Failure Semantics

The combined methods must fail exactly as the separate path would fail.

| Failure | Required behavior |
| --- | --- |
| Verifier challenge window still open | Revert before vault submission. |
| Proof is challenged | Proof can become frozen; vault submission must not happen. |
| Verifier acceptance missing | Revert before creating a vault pending withdrawal. |
| Packet domain mismatch | Revert with the same source-chain/vault/token/asset checks as `submitWithdrawal`. |
| Burn already submitted | Revert with the same burn replay check as `submitWithdrawal`. |
| Vault challenge window still open | Revert before claim. |
| Vault withdrawal challenged | Finalize to frozen and do not transfer tokens. |
| Insufficient vault liquidity | Leave withdrawal accepted and unclaimed, preserving the existing retry path. |
| Accepted withdrawal is past execution window | Still claimable, preserving the F-04 fix. |

No combined method may consume a replay slot unless the corresponding separate
method would have consumed it.

## Runner Integration

The runner should classify the new deployment as:

```text
bridge_class = fixed_contracts_redeployed_consolidated
```

Detection should be ABI-based and fail closed:

1. If both combined methods are present, use the consolidated path.
2. If only the F-03 fixed tuple is present, use the fixed separate path.
3. If only the old tuple is present, use the controlled-launch existing-contract
   path and label it as such.
4. If ABI detection is inconsistent, stop before source-chain withdrawal.

The EVM withdrawal report must include:

- `bridge_class`;
- verifier address;
- vault address;
- token address;
- `verifier_challenge_wait_secs`;
- `vault_challenge_wait_secs`;
- which method path was used:
  `separate`, `finalize_proof_and_submit_withdrawal`,
  `finalize_withdrawal_and_claim`;
- receipt watcher rows for each source-chain transaction;
- wallet and vault USDC deltas.

The benchmark verifier should reject Phase 3 claims unless the summary reports
`bridge_class = fixed_contracts_redeployed_consolidated` and the receipt watcher
rows prove the consolidated methods were used.

## Required Foundry Tests

Add tests without deleting the existing separate-path tests:

1. `testFinalizeProofAndSubmitWithdrawalMatchesSeparatePath`

   Execute separate path and combined path from equivalent fixtures and assert
   same withdrawal pending status, packet digest, hash commitment, recipient,
   amount, and replay guards.

2. `testFinalizeProofAndSubmitWithdrawalRespectsVerifierChallengeDelay`

   Call before `valid_after`; must revert and must not create a vault pending
   withdrawal.

3. `testFinalizeProofAndSubmitWithdrawalDoesNotSubmitFrozenProof`

   Challenge the proof, pass the challenge delay, call combined method, and
   assert no vault withdrawal can be claimed.

4. `testFinalizeWithdrawalAndClaimMatchesSeparatePath`

   Execute separate finalization plus claim and combined finalization plus claim
   from equivalent fixtures; assert recipient/vault balances and terminal
   status match.

5. `testFinalizeWithdrawalAndClaimRespectsVaultChallengeDelay`

   Call before `valid_after`; must revert and must not transfer tokens.

6. `testFinalizeWithdrawalAndClaimDoesNotPayFrozenWithdrawal`

   Challenge the withdrawal, pass the delay, call combined method, and assert no
   token transfer occurs.

7. `testConsolidatedPathCannotReplayAcrossVaults`

   Two vaults share a verifier. A packet accepted for vault A must fail on vault
   B because the packet domain binds source chain id, vault address, and token
   address.

8. `testConsolidatedPathRejectsRecipientSubstitution`

   A packet signed for recipient A cannot be submitted or claimed to recipient B.

9. `testConsolidatedPathDoubleClaimFails`

   Combined claim followed by separate or combined claim must fail.

10. `testConsolidatedPathAcceptedWithdrawalCanBeClaimedAfterExecutionWindow`

    Warped past execution window after acceptance remains claimable, preserving
    the F-04 behavior.

The existing tests for `testUnauthorizedChallengeCannotFreezeValidWithdrawal`,
`testUnauthorizedChallengeCannotFreezeValidProof`,
`testWithdrawalCannotReplayAcrossVaults`, and
`testAcceptedWithdrawalCanBeClaimedAfterExecutionWindow` remain mandatory.

## Required Rust Tests

If the ABI changes only add methods and do not change packet encoding, Rust
tests are runner tests:

- ABI detection identifies `fixed_contracts_redeployed_consolidated`;
- old controlled-launch tuple still works;
- fixed F-03 tuple still works;
- inconsistent ABI detection fails before submitting source-chain withdrawal;
- EVM withdrawal report records consolidated method names and receipt watcher
  rows;
- Phase 3 benchmark verification rejects summaries that do not report the
  consolidated bridge class or the consolidated EVM withdrawal receipt labels.

Status: the benchmark-verifier gate is implemented in `postfiat-node
nav-roundtrip-benchmark-verify --phase phase3`, and the acceptance battery can
be generated with `postfiat-node nav-roundtrip-benchmark-plan --phase phase3`.
Contract implementation and runner execution of the consolidated methods are
still future work gated by operator approval.

If packet encoding changes, the existing Rust packet-binding vector tests must
be updated and kept green.

## Deployment Gate

Do not deploy this until all are true:

1. Foundry suite green.
2. Rust runner/packet-binding suites green.
3. Contract addresses are new; do not mutate the old live proof.
4. StakeHub points at the new vault/verifier only after explicit operator
   approval.
5. One small-dollar bridge-in and bridge-out battery passes against the new
   addresses.
6. A ten-run Phase 3 benchmark passes with median under `55s`.

## Expected Runtime Effect

The expected saving is not from hiding challenge windows. It is from reducing
source-chain transaction count and receipt waits inside the withdrawal segment.

Current EVM withdrawal hot path:

```text
submitProof tx
wait verifier challenge
finalizeProof tx
submitWithdrawal tx
wait vault challenge
finalizeWithdrawal tx
claimWithdrawal tx
```

Consolidated hot path:

```text
submitProof tx
wait verifier challenge
finalizeProofAndSubmitWithdrawal tx
wait vault challenge
finalizeWithdrawalAndClaim tx
```

Expected controlled-launch improvement:

| Segment | Current | Target |
| --- | ---: | ---: |
| EVM withdrawal path | ~43.6s | ~20-30s |
| Full roundtrip | ~120s | ~35-55s after Phase 2 plus this change |

The actual result depends on Arbitrum RPC latency, configured challenge delays,
wallet signing latency, and whether Phase 2 PFTL round compression is already
live.

## Non-Goals

- This does not replace signer-threshold controlled-launch verification with a
  PFTL light client or fraud-proof game.
- This does not justify a public trustless bridge runtime claim by itself.
- This does not shorten production challenge windows.
- This does not change NAV accounting, pfUSDC issuance, or PFTL redemption
  settlement semantics.
- This does not require redeploying the existing live demo contracts until the
  operator approves.
