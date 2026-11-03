# Genesis External Lanes

A genesis external lane is native PFT that genesis reserves for a claim held outside PFTL. The first use is the ERC-20 PFT on Base. The product owner decided on 2026-10-28 that the total supply is 100 billion PFT and that the Base ERC-20 is a slice of it; the lane holds that slice as native PFT that is not in circulation until a proven Lock on Base releases it. The lane is the $s_{\text{external}}$ term of the [whitepaper's four-way partition](../whitepaper.md#13-supply-perimeter-primary-operations-and-the-cash-leg).

The design is `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md`, sections 4, 5 and 5.1. This page describes what the code does: genesis can reserve lanes, the ledger carries their records, every supply check counts them (milestone item P2.2), and `external_lane_release_v1` releases from a lane against a proven Base Lock (P2.4, on review).

## Genesis fields

`Genesis` takes an optional `external_lanes` list (`crates/types/src/external_lane_types.rs`). When the field is absent, the genesis serialises byte for byte as before, so existing genesis hashes and state roots do not change. Example:

```json
{
  "chain_id": "postfiat-example",
  "protocol_version": 1,
  "validator_count": 6,
  "native_supply_atoms": 1000000000,
  "external_lanes": [
    {
      "id": "base-erc20",
      "reserved_atoms": 300000000,
      "release_window_blocks": 86400,
      "release_cap_bps": 200,
      "policy": {
        "chain_id": 8453,
        "lock_contract_address": "0x1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a",
        "token_address": "0x2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b",
        "token_runtime_code_hash": "0x3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c",
        "lock_runtime_code_hash": "0x4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d",
        "committee_epoch": 2,
        "committee_root": "0x5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e",
        "confirmation_depth": 64
      }
    }
  ]
}
```

The amounts are placeholders sized to today's devnet supply constant (`GENESIS_NATIVE_SUPPLY_ATOMS`, 1,000,000,000 atoms). The slice size is a genesis parameter, not a constant, and stays open until the token document fixes it. A type test parses and validates this example.

`reserved_atoms` must equal the ERC-20 slice fixed by the token document; the type validator cannot know that number, so the genesis plan's build check (milestone P2.5) compares the lane against the token document and the deployed token's total supply before any lane-bearing genesis is produced.

| Field | Rule |
|---|---|
| `external_lanes` | Absent, or a non-empty list. A present empty list is refused. |
| `id` | 1–32 bytes of `a-z`, `0-9` and `-`, starting with a letter; unique. |
| `reserved_atoms` | Positive. The sum over all lanes must stay strictly below the native supply, so that the genesis allocation keeps a positive balance. |
| `release_window_blocks` | Positive. The window index is the block height divided by it. |
| `release_cap_bps` | 1 to 10,000: the share of `reserved_atoms` one window may release. |
| `policy.chain_id` | The source chain, a parameter limited to Base mainnet (8453) and Base Sepolia (84532). |
| `policy.*_address` | `0x` and 40 lower-case hex digits. |
| `policy.*_code_hash` | `0x` and 64 lower-case hex digits. |
| `policy.committee_epoch` | The bridge committee epoch whose checkpoints prove a Lock. |
| `policy.committee_root` | That committee's root (`FastSwapCommitteeV1::computed_root`, 48 bytes): `0x` and 96 lower-case hex digits. |
| `policy.confirmation_depth` | Positive: the Base confirmations a Lock needs before it can be proven. |

Unknown fields are refused at every level, so the policy cannot gain a field later without a schema change. The policy values are checked for shape, and `chain_id` against the two Base sources; the release path verifies proofs against them.

## Ledger and genesis state

At genesis the faucet receives the native supply minus the sum of the lanes, and `LedgerState.external_lanes` gets one record per lane (`crates/node/src/lifecycle_queries.rs`, `genesis_ledger_state`):

- `lane_id` and `reserved_atoms`, from genesis;
- `released_atoms`, starting at 0;
- `released_lock_nonces`, the set of Lock nonces already paid, starting empty;
- `window_index` and `window_released_atoms`, the current window and what it released, starting at 0;
- `releases`, the release history, omitted while empty.

The records are omitted from the ledger JSON and from the state commitment when there are none, so a chain without lanes keeps its state root. When there are records, they are committed at every height: unlike the NAV fields beside them they do not wait for the complete-NAV commitment, because they are native supply.
Tests pin this: the deployed `postfiat-wan-devnet-2` genesis re-serialises byte for byte with its hash, and a populated lane-less ledger keeps the state root it had before lanes existed.

## What the conservation check counts

`native_pft_supply_breakdown` (`crates/node/src/block_replay_wallet.rs`) reports two lines and their total:

- circulating: accounts, escrows, offers, owned objects, FastLane reserves and the Orchard pool;
- external lane remainder: `reserved_atoms - released_atoms` over all lanes.

`native_pft_live_total` is their sum, so the three checks keep their form:

- height-zero replay: live supply equals the genesis supply;
- each block: live before minus fee burns equals live after;
- each history checkpoint: live supply plus cumulative fee burns equals the genesis supply.

A lane that has released more than it reserved, or a duplicated lane id, fails these checks. A history checkpoint or imported snapshot must carry each genesis lane once, with its genesis `reserved_atoms`, and with release fields that are exactly the sum of its recorded releases: `released_lock_nonces` is the set of recorded nonces, `released_atoms` the sum of their amounts, and the window fields follow from the recorded heights, no window above the cap. Release fields without a record behind them have no state commitment and no live total, so atoms cannot leave a lane for an account unrecorded even when the totals still balance. Where the genesis is at hand (height-zero replay, history checkpoints) the faucet must hold exactly the native supply minus the lanes; a faucet file read without the genesis is only bounded by the supply, and height-zero replay refuses a short one.

Snapshots, restores and history checkpoints carry the lane records unchanged; tests cover lanes with nothing released and checkpoints with recorded releases, and a checkpoint, snapshot or faucet that drops a lane is refused.

## The release path

`external_lane_release_v1` is an asset transaction operation (`crates/types/src/transactions_mempool_receipts.rs`, `ExternalLaneReleaseOperationV1`). It names the lane, the Lock (`source_chain_id`, `lock_contract_address`, `lock_nonce`), `amount_atoms`, the PFTL `recipient` and the `proof`. Any account may submit it and pays the fee; it is submitted with `mempool_submit_signed_asset_transaction`, and mempool admission and batch building refuse it when execution would.

The proof is the design's option (b): an `EthereumExternalEventProofV1`, a Base block checkpoint signed by a quorum of the committee the lane's policy names, plus the receipt-inclusion proof of the lock contract's `Locked` event. For a lane the checkpoint's route fields carry the lane id, the lane's route-config digest (SHA3-384 over the genesis lane JSON), the lock contract and the token with their runtime code hashes, and the policy's `confirmation_depth` (`crates/execution/src/external_lane_release.rs`, `verify_external_lane_lock`).

Execution (`apply_external_lane_release`, rules on `ExternalLaneState`) checks, in order, and refuses with no state change and no nonce consumed:

| Check | Refusal code |
|---|---|
| Genesis configures the lane | `external_lane_not_configured` |
| The Lock is from the lane's chain and lock contract | `external_lane_source_mismatch` |
| A committee with the policy's epoch and root is registered | `external_lane_committee_missing` |
| The certificate has a quorum of valid votes; the receipt and log verify | `external_lane_proof_invalid` |
| The checkpoint binds the lane, chain, contracts, code hashes and depth | `external_lane_checkpoint_mismatch` |
| The `Locked` nonce, amount, recipient and recipient hash equal the claim | `external_lane_lock_mismatch` |
| The nonce was not released before | `external_lane_nonce_already_released` |
| The amount fits `reserved_atoms - released_atoms` | `external_lane_remainder_exceeded` |
| The window's released amount stays within the cap | `external_lane_window_cap_exceeded` |
| A new recipient account reaches the account reserve | `below_account_reserve` |

The Base checkpoint attestation adds no new schema. It reuses the existing Ethereum checkpoint (`EthereumFinalizedCheckpointV1`, `EthereumExternalEventProofV1`) and its public-values layout; the source chain id, the lock contract and its code hash, the token and its code hash, the committee and the confirmation depth are parameters taken from the lane's genesis policy. The pfUSDC SP1 ingress program is therefore unchanged and still pinned to Sepolia; a guest that proves a Base Lock, its vkey pin and its proof inventory entry are P2.5 work. Today the node returns the refusal codes of the table above: `external_lane_not_configured`, `external_lane_source_mismatch`, `external_lane_committee_missing`, `external_lane_proof_invalid`, `external_lane_checkpoint_mismatch`, `external_lane_lock_mismatch`, `external_lane_nonce_already_released`, `external_lane_remainder_exceeded`, `external_lane_window_cap_exceeded` and `below_account_reserve`.

An accepted release credits the recipient, creating the account if needed, adds the nonce and the amount, moves the window when the height is in a new one, and appends a record (nonce, chain, lock contract, amount, recipient, Base block number and hash, PFTL height). The atoms move from the lane remainder into circulation, so every conservation check above holds unchanged.

`external_lane_info` (public read RPC, and the `external-lane-info` node command, `--lane-id` optional) returns each lane's genesis configuration, live record with its release history, remainder, window cap, and what the next block's window may still release.

Not built yet: cap amendments and their seven-window raise delay (section 5.1), the committee epoch move by governed amendment (section 4.1), the watcher (section 5.2), and the Python CLI and interface. The design keeps the SP1 route for Base through its dispute games (option (a)) as a later upgrade.

## Not before

The design note sets three "not before" rules:

1. No genesis code merges before the token document fixes the supply number.
2. No lane is configured before a mainnet genesis plan exists. The [launch specification](l1v2-testnet-genesis-and-launch-spec.md) leaves token migration to a separate document that does not exist yet.
3. The lock contract is not deployed on Base mainnet before its audit is complete.

This work therefore lives on a branch, and no running network's genesis changes.
