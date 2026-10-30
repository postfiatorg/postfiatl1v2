# Genesis External Lanes

A genesis external lane is native PFT that genesis reserves for a claim held outside PFTL. The first use is the ERC-20 PFT on Base. The product owner decided on 2026-10-28 that the total supply is 100 billion PFT and that the Base ERC-20 is a slice of it; the lane holds that slice as native PFT that is not in circulation until a proven Lock on Base releases it. The lane is the $s_{\text{external}}$ term of the [whitepaper's four-way partition](../whitepaper.md#13-supply-perimeter-primary-operations-and-the-cash-leg).

The design is `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md`, sections 5 and 5.1. This page describes what the code does today (milestone item P2.2): genesis can reserve lanes, the ledger carries their records, and every supply check counts them. Nothing releases from a lane yet.

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
        "lock_runtime_code_hash": "0x4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d"
      }
    }
  ]
}
```

The amounts are placeholders sized to today's devnet supply constant (`GENESIS_NATIVE_SUPPLY_ATOMS`, 1,000,000,000 atoms). The slice size is a genesis parameter, not a constant, and stays open until the token document fixes it.

| Field | Rule |
|---|---|
| `external_lanes` | Absent, or a non-empty list. A present empty list is refused. |
| `id` | 1–32 bytes of `a-z`, `0-9` and `-`, starting with a letter; unique. |
| `reserved_atoms` | Positive. The sum over all lanes must be below the native supply. |
| `release_window_blocks` | Positive. The window index is the block height divided by it. |
| `release_cap_bps` | 1 to 10,000: the share of `reserved_atoms` one window may release. |
| `policy.chain_id` | Positive EVM chain id (Base mainnet is 8453). |
| `policy.*_address` | `0x` and 40 lower-case hex digits. |
| `policy.*_code_hash` | `0x` and 64 lower-case hex digits. |

Unknown fields are refused at every level. The policy values are checked for shape only; P2.4 verifies proofs against them.

## Ledger and genesis state

At genesis the faucet receives the native supply minus the sum of the lanes, and `LedgerState.external_lanes` gets one record per lane (`crates/node/src/lifecycle_queries.rs`, `genesis_ledger_state`):

- `lane_id` and `reserved_atoms`, from genesis;
- `released_atoms`, starting at 0;
- `released_lock_nonces`, the set of Lock nonces already paid, starting empty;
- `window_index` and `window_released_atoms`, the current window and what it released, starting at 0.

The records are omitted from the ledger JSON and from the state commitment when there are none, so a chain without lanes keeps its state root.

## What the conservation check counts

`native_pft_supply_breakdown` (`crates/node/src/block_replay_wallet.rs`) reports two lines and their total:

- circulating: accounts, escrows, offers, owned objects, FastLane reserves and the Orchard pool;
- external lane remainder: `reserved_atoms - released_atoms` over all lanes.

`native_pft_live_total` is their sum, so the three checks keep their form:

- height-zero replay: live supply equals the genesis supply;
- each block: live before minus fee burns equals live after;
- each history checkpoint: live supply plus cumulative fee burns equals the genesis supply.

A lane that has released more than it reserved, or a duplicated lane id, fails these checks. Snapshots, restores and history checkpoints carry the lane records unchanged.

## What P2.4 adds

Milestone item P2.4 adds `external_lane_release_v1`: it verifies a proven Lock on Base against the lane's genesis policy, rejects a nonce already in `released_lock_nonces`, rejects amounts above the lane's remainder or past the window cap (`reserved_atoms × release_cap_bps / 10,000`, rounded down), credits the recipient, and updates the release fields. A release moves atoms from the lane remainder into circulation, so the checks above hold unchanged.

## Not before

The design note sets three "not before" rules:

1. No genesis code merges before the token document fixes the supply number.
2. No lane is configured before a mainnet genesis plan exists. The [launch specification](l1v2-testnet-genesis-and-launch-spec.md) leaves token migration to a separate document that does not exist yet.
3. The lock contract is not deployed on Base mainnet before its audit is complete.

This work therefore lives on a branch, and no running network's genesis changes.
