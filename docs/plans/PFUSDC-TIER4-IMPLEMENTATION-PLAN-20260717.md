# pfUSDC Tier-4 Implementation Plan

**Date:** 2026-07-17

**Priority:** P0

**Status:** Implementation in progress on `pfusdc-tier4-20260717`; the live route remains Tier 1. See `PFUSDC-TIER4-CLOCK-CRITICAL-HANDOFF-20260718.md` for current state and execution order.

**Target:** Proof-verified USDC entry and proof-verified USDC exit on the existing PFTL and StakeHub product path

## 1. Definition of done

pfUSDC reaches Tier 4 only when both directions use cryptographic finality proofs:

1. **USDC -> pfUSDC:** PFTL verifies that the exact canonical vault deposit output is included in a confirmed Arbitrum Nitro assertion `sendRoot`, and that assertion is confirmed under finalized Ethereum state.
2. **pfUSDC -> USDC:** the Arbitrum vault verifies that the exact pfUSDC burn and withdrawal packet were accepted in a finalized PFTL block.
3. Relayers transport proof material but cannot make a false deposit or withdrawal valid.
4. A user can build and relay either proof without a bridge signer committee.
5. The existing conservation identity remains true throughout the round trip:

   ```text
   V = S + D + B - R
   ```

6. The threshold withdrawal verifier is not called by any Tier-4 withdrawal.
7. Once the Tier-4 route is active, new operations cannot silently fall back to observer or threshold verification.

Tier 4 includes the Tier-3 entry requirement. Replacing only the exit signer set is not sufficient.

Tier 4 is an evidence standard, not a frontend or demonstration standard. The
protocol implementation is complete when both proof directions work under an
activated no-fallback route. StakeHub polish, private-swap demonstrations,
multi-wallet demos, and broad launch batteries are controlled-testnet release
work after the protocol gate; they do not redefine Tier 4.

The controlled-testnet product launch then carries those frozen artifacts into
StakeHub's USDC -> pfUSDC -> a651 -> pfUSDC -> USDC flow and displays the tier,
proof state, source-finality age, receipt result, and balance changes.

## 2. Operating assumptions

- The StakeHub agent and test wallets may be unlocked while running approved implementation batteries.
- Devnet PFT, pfUSDC, a651, Arbitrum ETH, and dust USDC may be funded as needed.
- New contracts may be deployed and the controlled devnet may be upgraded or reset when a phase explicitly requires it.
- Use the current six-validator WAN topology. No validator geography change is required.
- Preserve the generic vault and governed-route model. Do not hardcode a user address, StakeHub address, current vault address, or asset ID into proof code.
- All chain IDs, genesis hashes, route epochs, contract addresses, runtime code hashes, program verification keys, proof-size limits, and activation heights come from versioned protocol configuration.
- The existing StakeHub transparent and private swap paths remain consumers of pfUSDC. This project changes pfUSDC ingress and egress verification, not a651 pricing or swap settlement.

## 3. Current ground truth

### 3.1 What already works

- [`ERC20BridgeVault.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/ethereum-contracts/src/ERC20BridgeVault.sol) holds an ERC-20, emits recipient-bound deposit events, binds withdrawal packet fields, prevents burn/withdrawal replay, and pays the packet recipient.
- [`PFTLWithdrawalVerifier.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol) verifies a sorted ECDSA threshold signature set over a PFTL withdrawal digest. This is the current exit boundary and must be replaced for Tier 4.
- [`account_owned_asset_types.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/types/src/account_owned_asset_types.rs) defines `VaultBridgeDepositEvidence`, route binding, deposit identifiers, bucket accounting, redemptions, and withdrawal packets.
- [`transactions_mempool_receipts.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/types/src/transactions_mempool_receipts.rs) defines the existing deposit propose/attest/finalize/claim operations.
- [`nav_vault_asset_execution.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/execution/src/nav_vault_asset_execution.rs) enforces deposit, issuance, bucket, supply, and redemption transitions.
- [`state_commitment.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/node/src/state_commitment.rs) commits bridge deposits, buckets, allocations, redemptions, and withdrawal packets into the replicated state root.
- [`consensus_v2.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/ordering_fast/src/consensus_v2.rs) defines the prepare/precommit finality protocol, committee root, quorum math, canonical vote bytes, and verified commit artifact.
- [`consensus_artifacts.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/node/src/consensus_artifacts.rs) binds finalized block evidence, receipt identifiers, state root, validator registry, and consensus-v2 commit material.
- [`nav_sp1_verifier.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/execution/src/nav_sp1_verifier.rs) proves that the Rust execution layer can verify SP1 Groth16 proofs with a configured program vkey and bounded public values.
- StakeHub `docs/pfusdc-bridge-runbook.md` records a complete live bridge-in, NAV subscription, NAV exit, bridge-out, and accounting-close flow.
- StakeHub `docs/ux-surfaces.md` identifies the dashboard routes and the existing end-to-end public/private demo flow.

### 3.2 Gaps that prevent Tier 4

- The active deposit route is observer-based.
- The current `sp1-groth16` deposit fields are proof and public-value **hashes**. `ensure_vault_bridge_deposit_source_proof` checks their presence and binding but does not receive or verify the actual bridge proof.
- The current exit accepts an ECDSA threshold assertion rather than a proof of PFTL finality.
- The current PFTL block artifact does not expose a compact, versioned Merkle commitment dedicated to accepted bridge exits. Proving the entire ledger snapshot would be expensive and unnecessarily broad.
- There is no stateful PFTL light-client verifier on Arbitrum that follows PFTL committee changes from a pinned genesis/checkpoint.
- There is no stateful Ethereum/Arbitrum finality verifier in PFTL replicated state.
- The current StakeHub bridge-out runner builds threshold-signature bundles and waits through two challenge windows.

## 4. Target architecture

### 4.1 Ingress

```text
Arbitrum USDC deposit
        |
        v
vault measures exact token balance delta
        |
        v
canonical ArbSys L2-to-L1 deposit output
        |
        v
exact output path into confirmed Nitro assertion sendRoot
        |
        v
confirmed assertion rooted in finalized Ethereum state
        |
        v
SP1 program -> Groth16 proof + PfUsdcIngressPublicValuesV2
        |
        v
PFTL execution verifies proof, route, output, replay key, and recipient
        |
        v
pfUSDC credited and bridge accounting updated
```

### 4.2 Egress

```text
accepted pfUSDC burn-to-redeem on PFTL
        |
        v
bridge-exit leaf committed in a finalized PFTL block
        |
        v
consensus-v2 commit + exit Merkle path + accepted receipt
        |
        v
SP1 program -> Groth16 proof + PfUsdcEgressPublicValuesV1
        |
        v
PFTLFinalityVerifierV1 on Arbitrum verifies and consumes proof
        |
        v
ERC20BridgeVaultV2 pays exact USDC amount to exact recipient
```

### 4.3 Proof trust anchors

The proof path must be stateful and continuous:

- PFTL stores a pinned Ethereum finalized checkpoint and advances it only through a valid Ethereum/Arbitrum finality proof.
- The Arbitrum verifier stores a pinned PFTL chain/genesis checkpoint and committee root and advances it only through a valid PFTL finality proof.
- PFTL committee rotation must be proven as part of the chain segment. No operator RPC field or contract setter may substitute a new committee root.
- Program-vkey rotation uses a new route epoch and activation height. An operation is permanently bound to the route epoch under which it began.

## 5. Protocol artifacts to freeze first

### 5.1 `PfUsdcIngressPublicValuesV2`

The canonical encoding must include at least:

- schema and proof-program versions;
- PFTL chain ID, genesis hash, protocol version, route profile hash, and route epoch;
- Ethereum chain ID and finalized beacon checkpoint root/slot;
- Arbitrum One chain ID, Rollup contract address/code hash, confirmed assertion identifier, asserted L2 block hash, and assertion `sendRoot`;
- canonical output index, item hash, sender, destination, calldata hash, L2/L1 block numbers, and timestamp;
- vault and token addresses plus their pinned runtime code hashes;
- production ingress-anchor address and runtime code hash;
- depositor, PFTL recipient bytes/hash, amount atoms, nonce, route binding, and deposit ID;
- evidence root and a domain-separated public-values commitment.

### 5.2 `PfUsdcEgressPublicValuesV1`

The canonical encoding must include at least:

- schema and proof-program versions;
- PFTL chain ID, genesis hash, protocol version, route profile hash, and route epoch;
- prior and resulting proof-verified PFTL checkpoints;
- committee epoch/root and any committee transition included in the proved segment;
- finalized block height/view/id, parent, state root, and bridge-exit root;
- exit leaf index and leaf commitment;
- accepted transaction receipt identifier and literal accepted receipt code;
- asset ID, burn transaction ID, withdrawal ID, source bucket ID, amount, recipient, destination hash, evidence root, and finalized height;
- Arbitrum chain ID, vault address, token address, and their pinned runtime code hashes;
- domain-separated packet digest, withdrawal hash commitment, and proof nullifier.

PFTL uses 48-byte identifiers in several places. EVM-facing commitments must hash the entire canonical byte string; they must not truncate a PFTL identifier to 32 bytes.

### 5.3 Bridge exit commitment

Add a versioned `BridgeExitLeafV1` containing only fields required to authorize one release. Each activated block commits an ordered Merkle root of accepted bridge-exit leaves.

Required properties:

- only an `accepted` `vault_bridge_burn_to_redeem` result can create a leaf;
- rejected receipts create no leaf;
- leaf ordering is deterministic;
- empty-root behavior is fixed and tested;
- the root is bound into the consensus-v2 block reference before validators vote;
- replay recomputes the identical root;
- historical blocks retain their historical encoding and cannot be reinterpreted as V1 exit-root blocks.

## 6. Execution phases

## Phase 0 — exact-binding spike and conformance vectors

**Goal:** close every ambiguity before building either prover.

- [ ] Trace a real accepted `vault_bridge_burn_to_redeem` from signed transaction through receipt ID, block payload, state root, consensus-v2 proposal, prepare QC, precommit QC, and `BlockRecord`.
- [ ] Write a failing test demonstrating that a fabricated withdrawal packet cannot be proven from the current finality artifact alone.
- [ ] Confirm whether receipt IDs are transitively bound by the consensus-v2 payload hash and document the byte-for-byte path.
- [ ] Decide the minimal activation-version change required to add `bridge_exit_root` to the voted block reference.
- [ ] Freeze canonical Rust encoders for both public-value structs and `BridgeExitLeafV1`.
- [ ] Produce Rust, SP1 guest, Solidity, and JSON conformance vectors.
- [ ] Prove that every single-field mutation changes the relevant digest.
- [ ] Set explicit proof-size, witness-size, memory, and proving-time limits from the two required release-candidate proofs. Do not generate standalone benchmark proofs on the critical path.

**Exit gate:** the exact signed/committed bytes and proof public values are frozen. No prose-only binding is accepted.

## Phase 1 — proof-verified Arbitrum entry

### 1A. Ethereum and Arbitrum finality state

- [ ] Add versioned replicated `EthereumArbitrumFinalityStateV1` with the pinned Ethereum checkpoint, Arbitrum rollup binding, latest verified assertion, retained finalized roots, and route epoch.
- [ ] Add bounded finality-update transaction types and deterministic state transition logic.
- [ ] Require monotonic checkpoint advancement and exact ancestry from a retained checkpoint.
- [ ] Reject wrong chain, stale route epoch, wrong rollup, wrong code hash, unconfirmed assertion, unfinalized Ethereum root, conflicting root, and oversized proof input before mutation.
- [ ] Retain a bounded checkpoint window so concurrent deposit proofs do not fail merely because another valid proof advanced the tip.

### 1B. Ingress SP1 program

- [ ] Create a dedicated SP1 workspace/program for pfUSDC ingress rather than reusing the NAV proof program.
- [ ] Verify Ethereum finality from the stored checkpoint to the new finalized checkpoint.
- [ ] Verify the deployed Arbitrum One rollup contract binding and confirmed assertion/state commitment under that Ethereum state.
- [ ] Verify the exact Nitro output leaf and Merkle path against the confirmed assertion `sendRoot` using canonical Nitro encoding or official vectors.
- [ ] Prove that ArbOS binds the L2 sender to the executing Tier-4 vault contract.
- [ ] Prove the Tier-4 vault and production ingress-anchor runtime code at the asserted L2 state.
- [ ] ABI-decode the exact canonical `recordDepositV1(...)` output calldata.
- [ ] Recompute recipient hash, route binding, deposit ID, amount, nonce, vault, token, sender, destination, output item hash, and output index.
- [ ] Commit only `PfUsdcIngressPublicValuesV2`.
- [ ] Generate reproducible Groth16 artifacts and record ELF hash, program vkey, verifier hash, toolchain lock, and proof encoding.

### 1C. PFTL proof verification

- [ ] Extend the deposit operation to carry bounded proof bytes and canonical public values, not only their hashes.
- [ ] Add a dedicated verifier kind such as `sp1-arbitrum-finality-v1`; do not overload a generic string that can mean a different proof.
- [ ] Verify Groth16 in execution using the active route's exact program vkey.
- [ ] Decode and compare every public value against the proposed evidence and active route.
- [ ] Make proof-backed propose/finalize/claim permissionless while keeping transaction fees and recipient binding intact.
- [ ] Store the proof digest and public-value commitment for audit, not the full witness.
- [ ] Preserve duplicate `deposit_id` and evidence-root rejection across restart, snapshot, replay, and route rotation.
- [ ] Remove observer attestations from the proof-backed finalization rule. They may remain only on older pinned route epochs.

**Exit gate:** a real Arbitrum deposit mints pfUSDC on a controlled PFTL target
with no observer attestation, and every mutated proof fixture rejects before
balance mutation. Six-validator rollout is a later controlled-testnet launch
gate, not part of the Tier-3/Tier-4 evidence definition.

## Phase 2 — PFTL exit commitment and finality prover

### 2A. Consensus-visible exit root

- [ ] Implement `BridgeExitLeafV1` and deterministic ordered Merkle construction.
- [ ] Add `bridge_exit_root` to a versioned block reference/header and bind it into proposal, prepare, and precommit signing bytes.
- [ ] Gate the new encoding at one explicit activation height.
- [ ] Update block building, external-certificate verification, replay, snapshots, history export/import, pruning proofs, RPC responses, and state verification.
- [ ] Reject mixed old/new encoding at and after activation.
- [ ] Add inclusion-proof export RPC/CLI for an accepted redemption ID.

### 2B. Egress SP1 program

- [ ] Create a dedicated SP1 PFTL-finality guest.
- [ ] Verify chain ID, genesis hash, protocol version, committee epoch/root, height, view, parent, and activation version.
- [ ] Verify canonical committee membership and quorum math.
- [ ] Verify distinct-validator ML-DSA-65 proposal/prepare/precommit signatures with their exact contexts.
- [ ] Require a valid precommit commit QC; proposal, legacy certificate, prepare QC, status convergence, or receipt presence alone is insufficient.
- [ ] Verify view-change/timeout ancestry when the committed block is in a later view.
- [ ] Verify any committee transition from the prior proof-verified checkpoint to the target block.
- [ ] Verify the bridge-exit Merkle path and accepted receipt identifier/code.
- [ ] Recompute the complete withdrawal packet, packet digest, withdrawal hash commitment, and proof nullifier.
- [ ] Commit only `PfUsdcEgressPublicValuesV1`.
- [ ] Bound chain-segment length. Add recursive checkpoint proofs if a single segment exceeds the proven resource limit.

### 2C. Proof service and CLI

- [ ] Add `pftl-finality-proof-export` to collect canonical, self-contained witness material from any honest node.
- [ ] Add `pfusdc-egress-prove` with persistent warm-prover mode.
- [ ] Validate all node responses against chain/genesis/route/checkpoint before proving.
- [ ] Make proof generation idempotent by `(route_epoch, burn_tx_id, withdrawal_id, finalized_block_id)`.
- [ ] Support multiple RPC sources and byte-compare the immutable witness fields.
- [ ] Emit progress, elapsed time, proof hash, public-values hash, and exact checkpoint range.

**Exit gate:** a local proof accepts one real finalized burn and rejects duplicate validators, insufficient quorum, bad signature, rejected receipt, altered packet, wrong Merkle path, wrong committee, wrong chain, and replay.

## Phase 3 — Arbitrum verifier and proof-native vault

### 3A. `PFTLFinalityVerifierV1.sol`

- [ ] Verify the SP1 Groth16 proof against an immutable program vkey/verifier binding.
- [ ] Decode `PfUsdcEgressPublicValuesV1` with strict length and canonical-encoding checks.
- [ ] Pin PFTL chain/genesis/protocol and the initial checkpoint/committee root.
- [ ] Advance the stored PFTL checkpoint only when the proof starts from an accepted checkpoint and ends at a strictly newer canonical checkpoint.
- [ ] Follow committee changes only through verified proof output.
- [ ] Pin route epoch, Arbitrum chain ID, vault, token, and runtime code hashes.
- [ ] Consume each proof nullifier and withdrawal ID once.
- [ ] Reject stale-start, non-advancing, wrong-vkey, wrong-route, wrong-vault, wrong-token, wrong-amount, wrong-recipient, and replayed proofs.

### 3B. `ERC20BridgeVaultV2.sol`

- [ ] Keep the V2 deposit event and recipient-selected PFTL address.
- [ ] Replace threshold/challenge authorization on the Tier-4 path with the proof verifier.
- [ ] Make proof verification and withdrawal consumption atomic with creation of the claimable withdrawal.
- [ ] Allow the named recipient to claim, or allow any caller to trigger direct payment only to the named recipient.
- [ ] Preserve burn-ID and withdrawal-ID replay protection and non-reentrancy.
- [ ] Measure actual token balance deltas. Reject fee-on-transfer and rebasing behavior for this route.
- [ ] Ensure a pause can stop new work but cannot rewrite, redirect, or fabricate a withdrawal.
- [ ] Make verifier, token, chain, and asset bindings immutable for a deployment.

### 3C. Contract verification

- [ ] Add cross-language public-value and packet-digest vectors.
- [ ] Add Foundry fuzz tests for calldata lengths, malformed proof encodings, reentrancy, malicious tokens, replay, checkpoint races, and maximum integers.
- [ ] Add invariants: total vault outflow never exceeds unique proof-verified burns; one burn pays once; recipient and amount never differ from the PFTL leaf.
- [ ] Run a pinned Arbitrum fork test that deploys the exact release contracts and uses the real USDC behavior.
- [ ] Produce deterministic deployment manifests with compiler, optimizer, sources, constructor arguments, bytecode, runtime hashes, program vkeys, and initial checkpoints.

**Exit gate:** an Anvil/Arbitrum-fork round trip releases exact USDC from a real PFTL proof and no ECDSA withdrawal signature is requested or accepted.

## Phase 4 — protocol route activation, then product integration

### 4A. Route and migration

- [ ] Add an exact Tier-4 route profile containing ingress/egress verifier kinds, program vkeys, proof bounds, chain bindings, contract addresses/code hashes, route epoch, and activation height.
- [ ] Inventory every pending deposit, minted pfUSDC atom, bucket allocation, pending burn, released-but-unsettled redemption, and vault USDC atom.
- [ ] Complete or cancel old in-flight operations under their pinned old route.
- [ ] Require zero unresolved old-route deposits and redemptions before declaring the new pfUSDC route Tier 4.
- [ ] Deploy a fresh V2 vault/verifier pair and migrate backing through one reconciled operation with before/after balances recorded.
- [ ] Disable new deposits and burns on the old route at the Tier-4 activation boundary.
- [ ] Do not implement an automatic fallback from Tier 4 to the old verifier. A Tier-4 failure pauses the Tier-4 route and preserves state.

**Protocol boundary:** Tier 4 is implemented when Phases 1-3 and this activated,
no-fallback route are proven with the frozen ingress and egress artifacts. The
CLI and StakeHub work below is controlled-testnet product integration.

### 4B. Canonical CLI

- [ ] Replace the observer ceremony in `vault-bridge-deposit-relay-rpc-bundle` with proof build/verify/submit steps when the active profile is Tier 4.
- [ ] Replace `vault-bridge-withdrawal-signature-bundle` on the Tier-4 route with finality witness export, proof generation, proof submission, and claim.
- [ ] Keep commands parameterized by wallet, route, recipient, amount, and RPC endpoints.
- [ ] Require accepted receipt codes on both chains; block convergence or transaction inclusion alone is not success.
- [ ] Report source-finality checkpoint, proof program/vkey, proof age, route epoch, amount, recipient, and conservation deltas.
- [ ] Retain the old commands only for explicitly pinned old-route drain work.

### 4C. StakeHub integration

Update the existing StakeHub dashboard and runner rather than creating a second demo:

- [ ] `bridge_in` displays `Building receipt proof -> Proving Arbitrum finality -> Verifying on PFTL -> Credited`.
- [ ] `bridge_out` displays `Burn accepted -> Building PFTL finality proof -> Verifying on Arbitrum -> USDC claimed -> Accounting settled`.
- [ ] Show `Tier 4 - proof verified` only when both active route verifier kinds and runtime code hashes match the chain profile.
- [ ] Show the Ethereum finalized checkpoint age and PFTL finalized checkpoint height.
- [ ] Show proof generation time separately from on-chain settlement time.
- [ ] Show accepted/rejected/unknown receipt code and never render inclusion or convergence as success.
- [ ] Preserve NAV proof freshness, pfUSDC/a651 balances, transparent swap, shielded swap, and private-egress views.
- [ ] Add self-relay and retry-from-artifact controls. Retrying proof transport must not repeat a deposit, burn, or claim.
- [ ] Reuse the persistent warm prover service pattern already used by the private-swap UX.

**Exit gate:** StakeHub drives the complete route without a signer-bundle step and explains each proof boundary to a first-time user.

## Phase 5 — adversarial and recovery batteries

### 5A. Ingress battery

- [ ] Wrong Ethereum chain, checkpoint, slot, sync committee, or finality branch.
- [ ] Unconfirmed or conflicting Arbitrum assertion.
- [ ] Wrong L2 block, receipt root, transaction index, receipt status, log index, topic, emitter, token, vault, amount, recipient, nonce, route, or code hash.
- [ ] Malformed RLP/MPT nodes, oversized paths, duplicate nodes, and resource-exhaustion witnesses.
- [ ] Duplicate deposit, same event under another route, proof replay after restart, and route rotation during proof generation.
- [ ] Concurrent valid deposits referencing retained checkpoints.

### 5B. Egress battery

- [ ] Four-of-six signatures, duplicate validator IDs, mixed committee, bad ML-DSA signature/context, wrong quorum, proposal-only, prepare-only, and missing precommit QC.
- [ ] Wrong view-change ancestry, timeout certificate, parent, block ID, state root, exit root, Merkle path, receipt ID, or receipt code.
- [ ] Altered asset, burn ID, withdrawal ID, source bucket, amount, recipient, destination hash, vault, token, route, or finalized height.
- [ ] Replayed proof, stale checkpoint start, competing checkpoint updates, committee rotation, and proof built across activation.
- [ ] Contract reentrancy, malicious/no-return token, fee-on-transfer token, rebasing token, paused route, and duplicate claim.

### 5C. System and recovery battery

- [ ] Crash prover, relayer, node, and StakeHub after every durable step; resume without repeating a money transition.
- [ ] Restart all six validators from snapshots and replay to the identical state root.
- [ ] Partition and delay validators while proving finality; accept only a valid commit QC.
- [ ] Kill all relayers and complete both directions from a second machine using only public artifacts.
- [ ] Rotate the PFTL committee and prove the first withdrawal under the new committee.
- [ ] Rotate the route/program vkey through a new epoch while preserving pinned in-flight work.
- [ ] Reconcile the full conservation identity after every test and at terminal state.

## Phase 6 — devnet deployment and final proof

### 6A. Deployment batching

Use the fewest disruptive network events:

1. Finish types, proof programs, contracts, execution, replay, snapshots, RPC, CLI, StakeHub, and local/fork batteries before touching WAN devnet.
2. If the block encoding can activate by protocol height, perform one rolling binary deploy and one governed activation. Do not reset.
3. If a new genesis is unavoidable, perform one planned devnet reset only after all release artifacts and migration fixtures are green. Archive the old chain first.
4. Deploy the Arbitrum contracts once per candidate. Any bytecode change creates a new candidate and repeats fork tests before redeployment.

### 6B. Rolling release gate

- [ ] Stage the current binary and contract manifests for rollback.
- [ ] Rolling-deploy one validator at a time; require six-node rejoin and identical tip/root before advancing.
- [ ] Activate the versioned exit root and Tier-4 route at explicit future heights.
- [ ] Confirm every node reports the same release, protocol version, route profile, program vkeys, and activation heights.
- [ ] Confirm old-route creation is rejected after activation while pinned drain artifacts remain readable.

### 6C. Final end-to-end demonstrations

Run both a transparent and private flow with fresh PFTL wallets:

1. acquire dust native Arbitrum USDC;
2. deposit into `ERC20BridgeVaultV2` for the fresh PFTL address;
3. build and submit the ingress finality proof;
4. verify accepted PFTL receipt and exact pfUSDC delta on all six validators;
5. perform a transparent pfUSDC/a651 atomic swap;
6. perform shield ingress, private pfUSDC/a651 swap, and private egress with the warm prover;
7. return a651 through the existing NAV exit to pfUSDC;
8. burn pfUSDC to the chosen Arbitrum recipient;
9. build and submit the PFTL finality proof;
10. claim exact USDC and settle PFTL accounting;
11. verify receipt codes, proof nullifiers, balance deltas, and `V = S + D + B - R`;
12. repeat with a second fresh wallet and different amount/recipient.

**Final gate:** two fresh-wallet round trips complete, all money transitions are accepted, all six validators agree in the untimed audit, both directions use the configured proof verifiers, no threshold withdrawal signature is produced, and conservation is exact.

## 7. Required tests and commands

During protocol implementation, run the smallest targeted command for the code
just changed. Do not use the following workspace battery for debugging and do
not run it through GitHub Actions. Run it locally once, after the four protocol
gates are green, for the immutable controlled-testnet launch candidate:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
forge test --root crates/ethereum-contracts -vv
```

Core protocol gates require:

- SP1 guest unit tests and deterministic ELF/vkey regeneration;
- Rust/Solidity/public-values conformance vectors;
- property tests for canonical encodings and Merkle construction;
- explicitly bounded fuzz/invariant tests for the affected trie, public-value,
  certificate, and ABI decoders; no open-ended fuzzer or soak run;
- Arbitrum fork deployment and exact USDC balance-delta tests;
- one real ingress proof and one real egress proof from the frozen guests.

Controlled-testnet launch gates additionally require:

- six-validator deterministic replay and snapshot restore;
- StakeHub backend and browser regression tests;
- full round-trip evidence bundle with file hashes.

## 8. Evidence layout

Each phase writes an append-only directory:

```text
docs/evidence/pfusdc-tier4-<phase>-<UTC>/
  MANIFEST.json
  ACCEPTANCE.json
  commands.log
  test-reports/
  conformance-vectors/
  proof-artifacts/
  chain-before.json
  chain-after.json
  balances-before.json
  balances-after.json
  receipts/
  deployment-manifests/
```

`ACCEPTANCE.json` must state every gate as a machine-readable boolean and include the hashes of proofs, public values, blocks, receipts, binaries, contracts, program ELFs, vkeys, route profiles, and deployment manifests.

## 9. Critical path

The shortest safe implementation order is:

1. freeze exact PFTL and Arbitrum proof bindings;
2. add and activate the consensus-bound bridge-exit root;
3. build the PFTL-finality SP1 guest and EVM verifier;
4. build the Arbitrum/Ethereum-finality SP1 guest and PFTL verifier path;
5. deploy the proof-native vault and activate the no-fallback Tier-4 route;
6. record the Tier-4 protocol result against the frozen artifacts;
7. replace signer/observer steps in CLI and StakeHub;
8. run controlled-testnet adversarial, recovery, and two-wallet launch batteries.

Work on ingress and egress proof guests can run in parallel after Phase 0 freezes the shared encodings. Contract and StakeHub work can proceed against conformance fixtures before the proof systems finish.

## 10. Completion checklists

### Tier-4 protocol implementation

- [ ] Real Arbitrum/Ethereum finality proof verified by PFTL execution.
- [ ] Real PFTL consensus-v2 finality proof verified by Arbitrum contract.
- [ ] Accepted receipt code proven in both directions.
- [ ] PFTL committee rotation followed through proof.
- [ ] No observer required for new deposits.
- [ ] No threshold signer required for new withdrawals.
- [ ] No automatic downgrade or fallback.
- [ ] Exact conservation holds across the proof-verified ingress and egress.
- [ ] The controlled-target Tier-4 route is activated and the core evidence
      manifest records its frozen artifact hashes.

### Controlled-testnet product launch

- [ ] Route/vkey rotation is versioned and replay-safe.
- [ ] Exact conservation holds across two fresh-wallet round trips.
- [ ] StakeHub makes proof tier, age, state, result, and value movement visible.
- [ ] CLI and self-relay paths are parameterized and reproducible.
- [ ] Local, fork, six-node, restart, partition, replay, fuzz, and browser gates pass.
- [ ] The controlled-testnet launch evidence manifests are recorded.
