# Phase 2 design: ERC-20 PFT on Base into native PFT at genesis

This is a draft research specification, written on 2026-10-26 by operator dravlic for the lead developer. It records the lead developer's four decisions of 2026-10-26 (section 10) and authorizes no code, deployment or genesis change. On 2026-10-26, after the release cap and the committee lifecycle were added, the Text Improvement Harness full gate scored it, without this paragraph, at 84.73/100 (GPT-6 Astra Pro 80.60, Claude Fable 5.1 85.60, GLM 5.3 88.00; five runs per judge; run group `pft-erc20-base-bridge-phase2-20261026-r3`; scored content SHA-256 `ce7a294f47092ca3…`; Task Node task `task_47bbfbbc2aeee1c1290f82475af551b4`). The previous score was 84.93. On 2026-10-28, with the supply invariant stated net of releases, the same gate scored it at 85.67 (GPT-6 Astra Pro 81.20, Claude Fable 5.1 85.60, GLM 5.3 90.20; run group `pft-erc20-base-bridge-phase2-20261028-r4`; scored content SHA-256 `8b19f4896f6b7eba…`), and after one wording and structure pass from the judges' points at 85.27 (81.60, 85.20, 89.00; run group `pft-erc20-base-bridge-phase2-20261028-r5`; `48dea0976825e426…`). On 2026-10-28, with the cap's raise delay and the false-release response of sections 5.1, 5.2 and 9 added as the lead developer decided that day, it scored 84.02 (78.60, 84.20, 89.25; GLM 5.3 over four recorded runs, because a malformed response aborted the first call; run group `pft-erc20-base-bridge-phase2-20261028-r6`; `5e0ef5811079192a…`). It is not locked, being below 86, and is not iterated further.

## 1. BLUF

1. Phase 2 builds an ownerless lock contract on Base, an "external" lane created at PFTL's mainnet genesis that holds native PFT equal to the whole ERC-20 supply, and a PFTL operation that releases native PFT from that lane one for one against a proven Lock on Base (`docs/whitepaper.md:66-70`, `docs/listings/coinbase/pft-erc20-representation-options-20261026.md:89-96`).
2. The proof is a BFT checkpoint, as the lead developer decided on 2026-10-26: the bridge committee signs a Base block and PFTL checks the Lock event's receipt against it. Consensus already runs exactly this check for Ethereum events (`crates/execution/src/pftl_uniswap_ethereum_verification.rs:340-378`). The trustless route through Base's dispute games has no code here and waits days for each lock, so it stays a later upgrade.
3. The bridge is therefore committee-trusted, not trustless. PFTL checks the quorum's signatures and the receipt's inclusion, not Base itself, so a colluding quorum could release native PFT with no Lock behind it. A release cap set at genesis bounds that to a fixed share of the lane per window (lean 2 % per window of one day's blocks), and the committee changes only by a governed epoch activation and halts, never releases, below its 5-of-6 quorum (sections 4.1, 5.1 and 9).
4. Genesis changes from giving the faucet the whole supply to giving it the supply minus the lane (`crates/node/src/lifecycle_queries.rs:3`), and the lane counts as native supply that is not in circulation, so the per-block conservation check keeps its form (`crates/node/src/block_replay_wallet.rs:2841-2864`).
5. My optimistic estimate is one to two weeks of engineering once the mainnet genesis plan exists, without audit time (`pft-erc20-representation-options-20261026.md:9`, `:89`).
6. The product owner still decides the supply number and the audit budget. The lead developer also decided on 2026-10-26 that the lock contract ships the dormant Phase 3 release path, bound at construction to an immutable PFTL finality verifier (section 10).

## 2. Terms

- **PFTL** is Post Fiat L1, the chain in this repository (`docs/start/glossary.md:12-14`).
- **Native PFT** is PFTL's base asset; genesis creates it once and no later record can create more (`docs/status/OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:12-13`, `:69-73`).
- **The external lane** is a genesis-created state entry holding the native PFT whose claim currently lives outside PFTL as ERC-20 PFT, the $s_{\text{external}}$ term of the four-way partition (`docs/whitepaper.md:66-70`).
- **The lock contract** is the Phase 2 contract on Base that takes ERC-20 PFT and records each Lock (section 3).
- **Base** is Coinbase's Ethereum layer-2 network, where the ERC-20 PFT lives (`crates/pft-token-contracts/src/PFTToken.sol:7-8`); its chain id is 8453, and its testnet Base Sepolia, 84532, is the id the token's dry run uses (`crates/pft-token-contracts/README.md:39`).
- **OP Stack** is the rollup software Base runs: Base posts commitments to its state on Ethereum, and Ethereum contracts settle disputes about them (`pft-erc20-representation-options-20261026.md:24`, docs.base.org).
- **An output root** is a proposer's hash commitment to one Base block, covering its state root and block hash; **a dispute game** is the Ethereum contract in which that claim can be challenged until it resolves (docs.base.org, withdrawals page, 2026-10-26).
- **A BFT checkpoint** is an EVM chain's block number, block hash and receipts or state root, signed by a quorum of a committee registered on PFTL (`crates/types/src/ethereum_bridge_types.rs:46-63`, `tools/nav-reserve-proof/README.md:31-40`).
- **The SP1 ingress program** is the zero-knowledge program that proves a pfUSDC deposit on Ethereum Sepolia under beacon-chain finality; PFTL verifies it as a bounded SP1 Groth16 proof (`programs/pfusdc-eth-ingress/src/lib.rs:84-122`, `crates/execution/src/nav_vault_asset_execution.rs:7576-7628`).

## 3. The lock contract on Base

The lock contract does one thing. It pulls ERC-20 PFT from the holder with `transferFrom`, assigns the next nonce, writes a permanent record, emits one `Locked` event and keeps the tokens. It has no owner, no pause and no setter. Its one other function is the dormant Phase 3 release path of section 6, bound at construction to an immutable PFTL finality verifier. The pfUSDC vault it borrows from does have an owner and a pause switch (`crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol:77-78`, `:116-125`), and the lock contract drops both.

It writes a storage record as well as emitting an event, as the vault does (`ERC20BridgeVaultL1.sol:157-166`, `:177-188`), because the two proof styles read different things. The SP1 ingress program proves storage slots and treats permanent storage as the admission authority (`programs/pfusdc-eth-ingress/src/lib.rs:124-175`, `:177-180`), while the in-consensus checkpoint route proves a receipt log (`pftl_uniswap_ethereum_verification.rs:374-377`). Keeping both leaves either proof open.

Four details come from the vault.

- The token address and its runtime code hash are fixed at construction (`ERC20BridgeVaultL1.sol:106-108`).
- The token balance delta is checked on every pull (`:153-156`).
- The recipient text is bounded at 256 bytes (`:55`, `:134-136`).
- The amount is capped at `uint64` (`:56`) because native balances on PFTL are `u64` (`crates/types/src/account_owned_asset_types.rs:10`).

One detail differs. The vault takes a caller-chosen nonce and hashes it into a deposit id (`ERC20BridgeVaultL1.sol:127`, `:139-152`); the lock contract counts nonces itself from zero, so each Lock is unique by construction and PFTL can check that a nonce is not reused.

The lock contract also refuses a Lock above `maxLockAmount`, fixed at construction to the per-window release cap of section 5.1, so every Lock fits into one window (lean of the lead developer, 2026-10-26).

```solidity
interface IPFTLockV1 {
    event Locked(uint64 indexed nonce, address indexed holder,
                 bytes32 indexed pftlRecipientHash, bytes pftlRecipient, uint64 amount);
    function token() external view returns (address);          // immutable PFTToken
    function finalityVerifier() external view returns (address); // immutable, no setter
    function maxLockAmount() external view returns (uint64);    // immutable: the genesis per-window cap
    function nextNonce() external view returns (uint64);
    function totalLocked() external view returns (uint256);    // sum of all Lock amounts
    function totalReleased() external view returns (uint256);  // Phase 3 only; zero in Phase 2
    function lockRecord(uint64 nonce) external view
        returns (address holder, uint64 amount, bytes32 pftlRecipientHash);
    function lock(uint64 amount, bytes calldata pftlRecipient) external returns (uint64 nonce);
    function lockWithPermit(uint64 amount, bytes calldata pftlRecipient, uint256 deadline,
                            uint8 v, bytes32 r, bytes32 s) external returns (uint64 nonce);
    // plus the dormant Phase 3 release function of section 6, checked by finalityVerifier()
}
```

`lockWithPermit` exists because the token supports EIP-2612 permits (`crates/pft-token-contracts/src/PFTToken.sol:15-16`), which saves the holder a separate approval transaction.

The contract keeps four invariants.

1. `totalLocked - totalReleased` equals the sum of `Locked` amounts minus Phase 3 releases, and `token.balanceOf(lock)` is at least that. The two differ only by tokens sent with a plain transfer, which no Lock records and nothing can ever release.
2. Nonces run from 0 to `nextNonce - 1`, each with exactly one record and one event.
3. No function lowers the contract's token balance except the Phase 3 release path of section 6.
4. `totalLocked - totalReleased` cannot exceed the token's fixed supply (`PFTToken.sol:9-10`, `:34`), so releases on PFTL cannot exceed the lane. Released tokens can be locked again in Phase 3, so `totalLocked` alone can pass the supply; in Phase 2, with no releases, the two bounds are the same.

## 4. Proving a Base Lock on PFTL

The repository has two working proof styles for EVM events and one signature pattern for the other direction. None of them reads Base today. The SP1 program is pinned to Sepolia's chain id, genesis validators root and fork schedule (`programs/pfusdc-eth-ingress/src/lib.rs:18-23`, `:90-97`, `:286-299`). A search for output roots, dispute games or chain id 8453 finds no code; the only Base reference is the token's local dry run on 84532 (`crates/pft-token-contracts/README.md:39`). The repository therefore holds no OP Stack facts of its own, and the latency figures below come from Base's documentation.

**(a) Extend the SP1 ingress program.** Today the guest verifies Ethereum beacon finality with Helios and then proves storage slots against the finalized execution state root (`lib.rs:108-122`, `:232-284`), and PFTL verifies the result through `verify_bounded_sp1_groth16` and consumes a deposit nullifier (`nav_vault_asset_execution.rs:7608-7627`). For Base the guest would add three steps after L1 finality: a storage proof on Ethereum that a dispute game for a Base output root has resolved in the claim's favour and passed its finalization window, the opening of that output root to Base's state root, and a storage proof of the lock record in Base's state. The trust base is Ethereum consensus plus Base's proof system. The latency is the dispute game's finalization window, which Base documents as five days for a single-proof game and one day on the dual-proof fast path (docs.base.org, withdrawals page, 2026-10-26); Base's earlier withdrawal guidance said seven days (github.com/base/withdrawer). The work is a new guest, mainnet chain parameters and parsing of OP Stack contract storage, and it does not fit the estimate.

**(b) BFT-checkpoint attestation of Base.**

**Existing verification path.** Consensus already accepts an `EthereumExternalEventProofV1`, which is a checkpoint certificate plus a receipt proof and a log index (`crates/types/src/ethereum_bridge_types.rs:239-243`). Execution checks the certificate against a committee registered on PFTL, then checks the log against the checkpoint's receipts root (`pftl_uniswap_ethereum_verification.rs:357-377`, `crates/bridge/src/ethereum_checkpoint.rs:23`, `crates/bridge/src/ethereum_receipt.rs:133`). The checkpoint already carries a `u64` chain id and a confirmation depth (`ethereum_bridge_types.rs:51-56`), but its other fields bind the PFTL-Uniswap route's controller and wrapped token (`:59-62`, `pftl_uniswap_ethereum_verification.rs:423-440`). The reserve-proof adapter `evm-erc20-bft-checkpoint-mpt-v1` does the same for balances under ML-DSA-65 quorums and names its chain as `eip155:<chain id>` (`tools/nav-reserve-proof/crates/reserve-proof-types/src/evm_checkpoint.rs:153-170`, `:291-294`). Its README says plainly that this is not trustless consensus finality (`tools/nav-reserve-proof/README.md:37-40`).

**Trust and latency.** The trust base is a quorum of the PFTL committee, which is already the trust base of any later export (`docs/listings/coinbase/integration-plan.md:733-734`). The latency is the confirmation policy plus one signing round, minutes rather than days, provided the policy signs only Base blocks derived from finalized Ethereum data.

**New work.** The work is a lock-route checkpoint schema in place of the Uniswap-specific fields, a Base observer for committee members, and the release operation.

**(c) Threshold-signed statement.** `PFTLWithdrawalVerifier` lets anyone relay a packet signed by a threshold of approved signers and lets a challenge authority freeze it before a delay closes (`crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol:4-7`, `:159-198`). An owner manages the signers and the threshold (`:78-87`, `:140-149`). Mirrored onto PFTL, the committee would sign "Lock n of amount x to recipient r happened" and PFTL would check only the signatures. The trust base is the committee and whoever manages it, with no Merkle link to Base state. Latency is minutes plus any challenge delay. The work is small, but option (b) costs little more and binds every Lock to a signed Base block.

| Option | Trust base | Latency per Lock | Exists today | New work |
| --- | --- | --- | --- | --- |
| (a) SP1 through dispute games | Ethereum consensus and Base proofs | 1–5 days | Helios guest, storage proofs, Groth16 verifier | OP Stack steps in a new guest |
| (b) BFT checkpoint | PFTL committee quorum | minutes | Checkpoint certificate, receipt-log verifier | Lock-route schema, Base observer |
| (c) Threshold statement | Committee and its manager | minutes | Pattern on the EVM side only | Signer set and verifier on PFTL |

The lead developer chose (b) on 2026-10-26. It reuses code that consensus already runs, its trust base is the committee that any later export already depends on, and it is the only option that fits the estimate. Its cost is that trust: PFTL checks the committee's signatures and the receipt's inclusion, not Base's own proofs (section 9). Option (a) stays open as a later upgrade, because the lock contract keeps the permanent storage record that a later SP1 guest would prove.

### 4.1 The bridge committee and its lifecycle

**Committee model.** The committee that signs checkpoints is a `FastSwapCommitteeV1` registered on PFTL, whose domain carries its epoch, root, member count and quorum (`crates/types/src/fastswap_types.rs:154-162`). A route names its committee by authority epoch and root, and a checkpoint carries both (`pftl_uniswap_ethereum_verification.rs:380-410`, `ethereum_bridge_types.rs:46-63`).

**Current committee.** Today that is epoch 2, activated by `ActivateCommittee` at height 1131 on 2026-10-15: six members, one per validator with validator-5's current key, and quorum 5, after the old committee's drained final checkpoint (`docs/status/chain-state-current.md:145`, `:182-189`).

**Existing route migration.** The A666 and BMNRC routes still name epoch 1, so validators 0–4 still sign their Ethereum checkpoints (`:193-194`, `:592`). The issuer signing sheet moves both routes to epoch 2 with issuer-signed policy updates, only while epoch 2 is the newest committee with no fence; after that the next checkpoint is signed 5 of 6 (`deployments/fastlane-cap-20261014/ISSUER-SIGNING-SHEET-20261020.md:11-13`, `:27-28`, `:141-146`, `:209-210`, on branch `release/fastlane-cap-20261014`).

The threshold is fixed by code, not chosen per route: a committee of n members, at least four, must have quorum exactly ⌊2n/3⌋+1 (`fastswap_types.rs:232-236`), which is 5 of 6. A certificate with fewer votes, a vote from a non-member or a bad ML-DSA-65 signature is refused (`crates/bridge/src/ethereum_checkpoint.rs:37-54`).

Rotation is a new epoch. `ActivateCommittee` registers epoch e+1 only after epoch e, and only with the previous committee's final drained checkpoint (`crates/execution/src/fastswap_control.rs:167-172`, `:360-388`). A route moves to it only while it is the newest committee and not fenced (`nav_vault_asset_execution.rs:5695-5720`), and from then on a checkpoint naming the old epoch fails the route binding (`pftl_uniswap_ethereum_verification.rs:434`). Native PFT has no issuer to sign such an update. Section 10 records the lead developer's 2026-10-26 decision: the lane moves to a new epoch by a governed amendment, under the same newest-and-unfenced check. From the block after it, checkpoints of the old epoch are refused. A release whose Lock was signed only by the old epoch is re-proven by the new committee from the same Base block, because the Lock record is permanent.

**One lost key.** A lost key is replaced by rotation, as epoch 2 replaced validator-5's key (`chain-state-current.md:187`, `docs/runbooks/fastpay-committee-recovery.md:3-5`). With 5 of 6, one lost key leaves releases running; two put the committee below quorum, every certificate is refused, and releases halt while the lane stays whole.

**Sub-quorum case.** Rotation then needs the old committee's final checkpoint, which a committee below quorum cannot sign. The repository has no in-chain rule for that case; the lead developer's lean (2026-10-26) follows the launch spec's rule for a registry that loses its quorum: nobody lowers the quorum or edits membership, and permanent loss is recovered outside the chain (`docs/architecture/l1v2-testnet-genesis-and-launch-spec.md:291-297`).

**Outage.** An outage works the same way. Without a checkpoint there is no proof and no release; the Lock stays recorded on Base and is released later, so an outage delays releases but never takes PFT from the lane.

## 5. The genesis external lane and the release path on PFTL

`Genesis` rejects unknown fields and accepts a native supply only if it equals `GENESIS_NATIVE_SUPPLY_ATOMS` (`crates/types/src/core_chain.rs:10`, `:300-306`, `:379-385`). The faucet receives that whole amount (`crates/node/src/lifecycle_queries.rs:3`), and height-zero replay requires live native supply to equal it (`crates/node/src/block_replay_wallet.rs:1838-1857`). Phase 2 adds an optional `external_lanes` list to `Genesis`. Each entry names a lane id, the source chain id (8453), the lock contract and token addresses with their code hashes, the proof policy (committee epoch and root, confirmation depth), the release cap (window length in blocks and share of the lane in basis points, section 5.1) and `reserved_atoms`. Validation requires `reserved_atoms` to equal the ERC-20 total supply fixed in the token document, and the faucet and allocations to hold the rest. Because the type validator cannot know the ERC-20 slice, which the token document has not fixed yet, that equality is deferred to the genesis plan's build check (milestone P2.5), which compares the lane against the token document and the deployed token's total supply before any lane-bearing genesis is produced. The lanes' sum must stay strictly below the native supply, so that the genesis allocation keeps a positive balance. The constant becomes the mainnet supply. Section 6 states the deployment order.

`LedgerState` (`crates/types/src/market_nav_asset_types.rs:3644`) gains the lane's live record: `reserved_atoms`, `released_atoms`, the set of released Lock nonces, and the current window's index and released amount. The new operation is `external_lane_release_v1`, carrying the lane id, the Lock nonce, the amount, the recipient and the proof. Execution does six things in order.

1. It verifies the proof under the lane's genesis policy and checks the chain id, lock contract, token and code hashes.
2. It rejects a nonce already in the released set with `external_lane_nonce_already_released`.
3. It rejects an amount above `reserved_atoms - released_atoms`.
4. It rejects an amount that would take the window's released amount past the cap with `external_lane_window_cap_exceeded` (section 5.1).
5. It credits the recipient named in the proven record, creating the account if needed.
6. It adds the nonce to the set and the amount to `released_atoms` and to the window's released amount.

A rejected release leaves every lane and balance unchanged, as the inventory requires of every rejected transaction (`OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:71-73`). A repeated submission is rejected, not credited twice, so the operation is idempotent. Anyone may submit it, because the recipient comes from the proven record and a submitter cannot redirect it. A new recipient holds no native PFT for the fee, so any relayer, such as the exchange or the CLI's operator, may pay it. No governance or operator path writes to the lane; only a valid proof moves PFT out of it, and a governed amendment can change the cap but moves no PFT.

**Conservation check.** The invariant changes in what it counts, not in its form. Today every block satisfies "live before minus fee burns equals live after", and a checkpoint satisfies "live plus cumulative burns equals genesis supply" (`OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:16-24`, `block_replay_wallet.rs:2841-2864`, `crates/node/src/history.rs:1337-1357`). Phase 2 counts `reserved_atoms - released_atoms` inside `native_pft_live_total`, whose exhaustive destructure of `LedgerState` fails the build until a new field is classified (`block_replay_wallet.rs:2664-2721`, `OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:31-34`). A release then moves atoms from the lane to an account inside live custody, and both checks hold unchanged.

**Four-way partition.** The four terms stay consistent: native custody (accounts, escrows, offers, owned objects, FastLane reserves) plus shielded (the Orchard pool) plus external (the lane's remainder) plus in-flight (zero until Phase 3) equals genesis supply minus cumulative burns (`docs/whitepaper.md:66-70`, `OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:36-43`).

**Cross-chain backing.** Across chains, `released_atoms` never exceeds the lock contract's `totalLocked` as long as the committee quorum signs only real Base blocks, because every release consumes one proven Lock. Native supply conservation holds either way; the cross-chain backing depends on the quorum (section 9).

The consensus code touched is in `crates/types` (`core_chain.rs` for `Genesis`, `market_nav_asset_types.rs` for `LedgerState`, `ethereum_bridge_types.rs` for the checkpoint schema, and the operation and receipt types in `transactions_mempool_receipts.rs`), a new execution module dispatched from `crates/execution/src/entrypoints.rs`, and in `crates/node` the live-supply total, the faucet and checkpoint replay (`block_replay_wallet.rs:2664`, `lifecycle_queries.rs:3`, `history.rs:1337-1357`). A search of `crates/execution` and `crates/types` for "external" finds only NAV and PFTL-Uniswap uses (`crates/execution/src/issued_asset_ledger_helpers.rs:692-741`, `ethereum_bridge_types.rs:239`); no native external lane exists today.

### 5.1 The per-window release cap

The lead developer decided on 2026-10-26 that releases are capped per window; the numbers are the lean until the genesis plan fixes them.

**Parameters.** Each lane carries two genesis parameters: `release_window_blocks`, the window length, and `release_cap_bps`, the share of `reserved_atoms` that one window may release. The lean is 200 basis points, 2 % of the lane, per window of one day's blocks at the mainnet cadence.

**Window accounting.** The window is counted in blocks because block headers carry no time (`crates/types/src/core_chain.rs:623-643`): the window index is the block height divided by `release_window_blocks`, and the cap is `reserved_atoms × release_cap_bps / 10_000`, rounded down. The released amount resets to zero when a release lands in a new window.

**Rejection and retry.** A release that would exceed the cap is refused without any state change, like every rejected release (`OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:71-73`). Its nonce is not consumed, so anyone retries it in a later window with the same proof.

**Security effect under the lean.** Under the lean, a colluding quorum takes at most 2 % of the lane in one window, 20 million PFT of a hypothetical 1-billion-PFT lane. 50 consecutive windows, about 50 days, reach the whole of it. Over a year the cap does not protect the lane. What the cap buys is time, about 50 days between the first false release and an empty lane, in which validators can see the releases and stop them.

**Honest traffic.** The cap also limits honest traffic: no window releases more than 2 % of the lane, so a large conversion spreads over several windows.

A cap change is a governed amendment, as the three activation heights of 2026-10-23 were: one amendment each, signed by the six validators (`docs/status/chain-state-current.md:41-46`). An amendment names its kind and a `u32` value with the validators and quorum that signed it (`crates/types/src/shielded_bridge_governance.rs:1722-1735`), and the latest committed amendment of a kind wins (`:1548-1556`). A cap of zero stops releases without touching the lane or any Lock.

**Lowering and raising the cap.** The lead developer decided on 2026-10-28 that the two directions differ.

- An amendment that lowers the cap takes effect immediately. A cap of zero halts releases at once and moves no PFT.
- An amendment that raises the cap takes effect only seven windows after it commits, so a quorum that could sign false checkpoints cannot also lift the cap in the same week.
- The lock contract's `maxLockAmount` is fixed at construction and is not changed by the amendment: a larger cap on PFTL does not enlarge single Locks on Base.

### 5.2 Detecting and answering a false release

The lead developer decided on 2026-10-28 how a false release is detected and answered.

**Detection.** Every release is a receipt in a block and names its Lock nonce. A watcher, the Python CLI of this milestone extended, compares the released nonces on PFTL with the `Locked` events of the lock contract on Base. It raises an alarm for any release whose nonce has no matching Lock, whose amount differs from the Lock's amount, or whose recipient hash differs from the Lock's `pftlRecipientHash`.

**Response.** The response is three governed steps, in this order.

1. One governed amendment sets the cap to zero: six signatures, with immediate effect (section 5.1).
2. A new epoch rotates the committee (section 4.1).
3. The amendment that restores the cap takes effect after the seven-window delay of section 5.1.

**The lane during the response.** The lane loses at most one window's cap, 2 % under the lean, before the zero-cap amendment lands, and it cannot lose more than that per window, because a release never exceeds the cap and the false release is bounded by the window in which it lands.

**What the cap buys.** At 2 % of the lane per window, a quorum that nobody stops needs 100 % ÷ 2 % = 50 windows to empty the lane, about 50 days at one window per day. The arithmetic assumes no cap raise: a raise takes effect only seven windows after its amendment commits, so the buffer holds for at least seven windows after any raise is proposed. That buffer of about 50 days is what the cap buys: the watcher raises its alarm on the first false release, and the zero-cap amendment halts the next one at once.

## 6. Phase 3 hooks kept open

Phase 3 exports native PFT back to Base by releasing ERC-20 PFT that was bridged in, from this same lock contract (`pft-erc20-representation-options-20261026.md:98-104`).

**Deployment order.** The lock contract has no admin, so a release path added later could never reach tokens locked before it. Phase 2 therefore deploys the release function now, dormant and bound at construction to a PFTL finality verifier on Base, as the lead developer decided on 2026-10-26. The contract ships it as `release(bytes publicValues, bytes proofBytes) returns (bytes32 withdrawalIdCommitment)`, which emits `Released(bytes32 indexed withdrawalIdCommitment, bytes32 indexed burnTxIdCommitment, bytes32 indexed packetDigest, address recipient, uint256 amount)`. The verifier's address is immutable, with no setter and no admin, so Phase 3 stays possible without any privileged role, and the verifier is deployed on Base before the lock contract. Because genesis names the lock contract's address, the lock contract is deployed before genesis.

**Existing pattern.** The vault shows the pattern: it releases against `PFTLFinalityVerifierV1`, fixed at construction, and rejects replayed withdrawal and burn commitments (`ERC20BridgeVaultL1.sol:13-16`, `:81-83`, `:110`, `:191-203`). The verifier and relay run on Ethereum mainnet today, so Base needs its own deployment (`deployments/a666-export-relay-mainnet-20260731/README.md:7-12`).

On PFTL, Phase 2 must not preclude three things. The lane must be able to grow again when an export moves PFT from native through in-flight to external (`docs/whitepaper.md:70`), so it records released and exported atoms separately. Release nonces on Base must stay distinct from Lock nonces. And the six-decimal unit must hold on both sides (`PFTToken.sol:11-12`, `:37-40`). The PFTL export path must also refuse the lock contract's own address and the zero address as the Base recipient, because such a release can never be paid out (`release` reverts on both).

## 7. Operator and user flows

A holder takes five steps.

1. Check the PFTL recipient address in the Python CLI before locking, since the contract checks only its length.
2. Call `lockWithPermit`, or approve and then call `lock`, on Base.
3. Wait for a committee checkpoint covering that block.
4. Submit `external_lane_release_v1`, or let a relayer submit it.
5. See native PFT on PFTL under an accepted receipt.

An exchange holding ERC-20 PFT for customers locks the customers' total in one transaction (one per window when the total exceeds the cap of section 5.1) to its own PFTL address, submits one release, and credits each customer natively one for one. Coinbase converted INJ balances 1:1 the same way when Injective moved to its native chain (`pft-erc20-representation-options-20261026.md:44`). The exchange may end ERC-20 deposits on its own schedule, while the bridge itself stays open with no closing date (`:96`). Committee members run Base observers and sign checkpoints. No single member can release PFT; a release needs a quorum checkpoint and a receipt proof against it.

## 8. Tests, qualification and "not before" rules

The tests are written before the code.

- The Foundry tests for the lock contract are `test_lock_pulls_tokens_and_records_nonce`, `test_lock_nonces_are_sequential`, `test_lock_rejects_zero_amount`, `test_lock_rejects_amount_above_uint64`, `test_lock_rejects_empty_or_oversized_recipient`, `test_lock_with_permit_matches_lock`, `test_total_locked_equals_sum_of_lock_events`, `test_plain_transfer_is_not_a_lock`, `test_release_is_bound_to_immutable_finality_verifier`, `test_lock_rejects_amount_above_max_lock_amount` and `test_contract_has_no_owner_pause_or_withdraw`.
- The type tests are `genesis_external_lane_requires_reserved_equal_to_erc20_supply` (deferred to P2.5 with the genesis plan), `genesis_rejects_lanes_at_or_above_native_supply`, `genesis_faucet_holds_supply_minus_lanes`, `genesis_rejects_unknown_lane_fields` and `genesis_rejects_release_cap_outside_one_to_ten_thousand_bps`.
- The execution tests are `external_lane_release_credits_recipient_once`, `external_lane_release_rejects_replayed_nonce_without_mutation`, `external_lane_release_rejects_wrong_chain_contract_or_token`, `external_lane_release_rejects_checkpoint_below_quorum`, `external_lane_release_rejects_amount_above_lane_remainder`, `external_lane_release_creates_missing_recipient_account` and `rejected_release_in_accepted_block_leaves_supply_unchanged`.
- The cap tests are `external_lane_release_refuses_amount_past_window_cap_without_mutation`, `external_lane_window_released_amount_resets_in_next_window`, `external_lane_release_refused_by_cap_succeeds_in_later_window_with_same_proof`, `external_lane_cap_amendment_latest_value_wins`, `external_lane_cap_lowering_takes_effect_immediately`, `external_lane_cap_raise_waits_seven_windows`, `watcher_flags_release_without_matching_lock` and `watcher_flags_release_with_mismatched_amount_or_recipient`.
- The committee tests are `external_lane_release_rejects_checkpoint_of_superseded_epoch`, `external_lane_epoch_move_requires_newest_unfenced_committee`, `external_lane_release_halts_below_quorum_and_leaves_lane_unchanged` and `external_lane_release_reproven_by_new_epoch_pays_the_nonce_once`.
- The node tests are `native_live_total_counts_external_lane_remainder`, `replay_from_genesis_preserves_supply_across_releases` and `checkpoint_snapshot_restore_preserves_unreleased_lane_records` (the released-lane lifecycle test comes with P2.4).

The release changes a state result, so it needs its own qualification, as a consensus change did on 2026-10-08 (`docs/handoffs/2026-10-08___dravlic__conference_day_two_friday_ready.md:68`). That qualification repeats the native inventory's gates: genesis-to-tip replay, checkpoint, prune, and snapshot and restore (`OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:6-8`, `:77-80`).

Three "not before" rules apply. No genesis code merges before the token document fixes the supply number (`docs/listings/coinbase/integration-plan.md:99-105`). No lane is configured before a mainnet genesis plan exists; the launch spec leaves token migration to a separate document that does not exist yet (`docs/architecture/l1v2-testnet-genesis-and-launch-spec.md:27`, `:402`). And the lock contract is not deployed on Base mainnet before its audit is complete (`pft-erc20-representation-options-20261026.md:87`).

## 9. Risks

- A colluding committee quorum could sign a false Base block and release native PFT with no Lock behind it. Two mitigations decided on 2026-10-28 answer it. First, the per-window cap of section 5.1 bounds that to 2 % of the lane per window under the lean, and a raise of the cap takes effect only seven windows after its amendment commits, so the quorum that signs false checkpoints cannot also lift the cap in the same week. Second, the watcher of section 5.2 matches every release on PFTL against a Lock on Base and raises an alarm on the first false one; one zero-cap amendment halts releases at once, a new epoch rotates the committee, and the cap returns only after the seven-window delay. Left unchecked for 50 windows, the false releases still reach the whole lane.
- Base blocks can reorganise before their data is final on Ethereum. A checkpoint policy that signs only blocks derived from finalized Ethereum data avoids this.
- A Lock to a mistyped PFTL address is lost for good, because the lock contract has no admin. The CLI must validate the address first.
- ERC-20 PFT sent to the lock contract by plain transfer is also lost for good.
- If the lane's `reserved_atoms` differs from the token's constructor supply (`crates/pft-token-contracts/README.md:23-26`), the one-for-one claim breaks.
- The dormant Phase 3 release path enlarges the Phase 2 audit.
- Generalising the checkpoint schema touches the PFTL-Uniswap verification path (`pftl_uniswap_ethereum_verification.rs:411-440`), so that route's tests must stay green.

The residual risk after the cap, its raise delay, the watcher and the committee lifecycle is plain. The six committee members are today's six validators (`docs/status/chain-state-current.md:186-188`), so the quorum that could sign false checkpoints is the same quorum that votes on blocks and signs governance amendments. The cap and the rotation rules protect the lane against a few stolen or lost checkpoint keys, a faulty Base observer, and an outage: none of these releases anything. They do not protect it against five of six validators acting together, who could also lift the cap seven windows after the amendment commits, without whom the six-signature zero-cap amendment cannot pass, and who are already trusted with every rule of the chain. With all six validators operated by one organisation today, the cap bounds the damage per window, but the committee-collusion risk is fully mitigated only by independent validator operators, which the readiness work tracks separately. Against that quorum the cap only slows the loss to 2 % of the lane per window, which gives holders and the product owner about 50 days to see it, and the watcher shows each false release as it lands.

## 10. Decisions

The product owner decides the supply number, which the token document fixes (`integration-plan.md:99-105`), and the audit budget for the lock contract and its release path.

The lead developer decided four things on 2026-10-26 and two on 2026-10-28. All are revisited only if the code shows a reason.

1. The proof style is (b), a BFT checkpoint of a Base block signed by the bridge committee, plus a receipt-log proof of the Lock event (section 4). The SP1 path through Base's dispute games, option (a), stays a later upgrade.
2. The lock contract ships the dormant Phase 3 release path in Phase 2 (section 6). The PFTL finality verifier's address is fixed at construction, immutable, with no setter and no admin, so Phase 3 stays possible without any privileged role.
3. Releases are capped per window by genesis parameters, changed only by a governed amendment; the lean is 2 % of the lane per window of one day's blocks, and the lock contract caps each Lock at the same amount (section 5.1).
4. The lane follows the bridge committee's lifecycle: a new epoch by governed amendment, old-epoch checkpoints refused after it, and a halt rather than any release below quorum (section 4.1).
5. On 2026-10-28: an amendment that lowers the cap takes effect immediately, and a cap of zero halts releases at once and moves no PFT; an amendment that raises it takes effect only seven windows after it commits; the lock contract's `maxLockAmount` stays fixed at construction (section 5.1).
6. On 2026-10-28: a watcher, the Python CLI of this milestone extended, matches every release on PFTL against a `Locked` event on Base, and a false release is answered by a zero-cap amendment, then a committee rotation by a new epoch, then the amendment that restores the cap after the seven-window delay (section 5.2).

## 11. Sources

- `docs/listings/coinbase/pft-erc20-representation-options-20261026.md`, `docs/listings/coinbase/integration-plan.md`, `docs/whitepaper.md`, `docs/start/glossary.md`
- `docs/status/OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md`, `docs/architecture/l1v2-testnet-genesis-and-launch-spec.md`, `docs/handoffs/2026-10-08___dravlic__conference_day_two_friday_ready.md`
- `crates/pft-token-contracts/src/PFTToken.sol`, `crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol`, `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol`, `crates/pft-token-contracts/README.md`
- `programs/pfusdc-eth-ingress/src/lib.rs`, `tools/nav-reserve-proof/README.md`, `tools/nav-reserve-proof/crates/reserve-proof-types/src/evm_checkpoint.rs`
- `crates/types/src/core_chain.rs`, `crates/types/src/ethereum_bridge_types.rs`, `crates/types/src/market_nav_asset_types.rs`, `crates/types/src/account_owned_asset_types.rs`
- `crates/execution/src/nav_vault_asset_execution.rs`, `crates/execution/src/pftl_uniswap_ethereum_verification.rs`, `crates/execution/src/issued_asset_ledger_helpers.rs`, `crates/bridge/src/ethereum_checkpoint.rs`, `crates/bridge/src/ethereum_receipt.rs`
- `crates/node/src/block_replay_wallet.rs`, `crates/node/src/history.rs`, `crates/node/src/lifecycle_queries.rs`, `deployments/a666-export-relay-mainnet-20260731/README.md`
- `crates/types/src/fastswap_types.rs`, `crates/types/src/shielded_bridge_governance.rs`, `crates/execution/src/fastswap_control.rs`, `docs/status/chain-state-current.md`, `docs/runbooks/fastpay-committee-recovery.md`, and `deployments/fastlane-cap-20261014/ISSUER-SIGNING-SHEET-20261020.md` on branch `release/fastlane-cap-20261014`
- Base documentation, "Withdrawals", <https://docs.base.org/specifications/base-protocol/bridging/withdrawals>, and the `base/withdrawer` README, <https://github.com/base/withdrawer>, both read on 2026-10-26.
