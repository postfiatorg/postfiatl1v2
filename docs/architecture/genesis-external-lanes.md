# Genesis External Lanes

A genesis external lane is native PFT that genesis reserves for a claim held outside PFTL. The first use is the ERC-20 PFT on Base. The product owner decided on 2026-10-28 that the total supply is 100 billion PFT and that the Base ERC-20 is a slice of it; the lane holds that slice as native PFT that is not in circulation until a proven Lock on Base releases it. The lane is the $s_{\text{external}}$ term of the [whitepaper's four-way partition](../whitepaper.md#13-supply-perimeter-primary-operations-and-the-cash-leg).

The design is `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md`, sections 4, 5 and 5.1. This page describes what the code does: genesis can reserve lanes, the ledger carries their records, every supply check counts them (milestone item P2.2), and `external_lane_release_v1` releases from a lane against a proven Base Lock (P2.4).

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
- `release_history_root`, a SHA3-384 hash chain over the release records in application order, omitted while there is none;
- `pruned_release_count`, `pruned_released_atoms` and `pruned_release_history_root`, the records pruned behind the finalized height (below), omitted while none is;
- `releases`, the stored release records, omitted while empty.

The state commitment covers every field but `releases` (`ExternalLaneState::state_commitment_bytes`), so a block commits the lane in bytes that grow by one nonce per release, not by the records. The stored records stay in the ledger for reads and replay, and loading a replay base checks them against the committed root, refusing a mismatch:

- an unpruned lane's `release_history_root` is recomputed from all its records;
- a pruned lane's verifier takes the committed `pruned_release_history_root` as the anchor and recomputes only the stored tail from it to `release_history_root`. It cannot prove the pruned records' contents from the lane state alone: the block receipts, a full replay from genesis or an independently verified state root attest the pruned prefix ([Pruning the release records](#pruning-the-release-records)).

The lane list is omitted from the ledger JSON and from the state commitment when there are no lanes, so a chain without lanes keeps its state root. When a lane exists, its committed fields are committed at every height, `release_history_root` included but never the release records: unlike the NAV fields beside them they do not wait for the complete-NAV commitment, because they are native supply.
Tests pin this: the deployed `postfiat-wan-devnet-2` genesis re-serialises byte for byte with its hash, and a populated lane-less ledger keeps the state root it had before lanes existed.

## Starting a chain with a lane

`init` takes the whole genesis from a file:

```sh
postfiat-node init --genesis-file crates/node/testdata/external-lane-genesis/genesis.json \
  --data-dir ./data/validator-0 --node-id validator-0
```

The file sets the chain id, validator count, activation heights and lanes, so `--chain-id` and `--validators` are refused beside it. Unknown fields are refused, and the genesis is validated, lane rules included (one id and one Base source per lane), before anything is written; a refusal exits with `init failed: <rule>` and leaves no data directory. The data directory gets the lane records in the height-zero ledger and the faucet at the native supply minus the reserves; the report's `genesis_hash` and `state_root` are those of the file's genesis.

The example file has one Base Sepolia lane: the rehearsal lock `0xe36e7fc384564419187546c19719a5654266de0b` and token `0x18fe4ca51a0ce042688d5a54917319d03b527d92`, with the code hashes of `crates/pft-token-contracts/deployments/pft-lock-base-84532/deployment.json`, 300,000,000 atoms reserved and a 200 bps cap; its committee root is a placeholder. A test pins its bytes and hash and checks that `init` gives the same ledger and state root as the in-code lane genesis (`init_from_lane_genesis_file_matches_the_in_code_lane_genesis`, `crates/node/src/tests/genesis_external_lane_tests.rs`). `topology-consensus-v2` still derives its domain from a genesis without lanes.

## What the conservation check counts

`native_pft_supply_breakdown` (`crates/node/src/block_replay_wallet.rs`) reports two lines and their total:

- circulating: accounts, escrows, offers, owned objects, FastLane reserves and the Orchard pool;
- external lane remainder: `reserved_atoms - released_atoms` over all lanes.

`native_pft_live_total` is their sum, so the three checks keep their form:

- height-zero replay: live supply equals the genesis supply;
- each block: live before minus fee burns equals live after;
- each history checkpoint: live supply plus cumulative fee burns equals the genesis supply.

A lane that has released more than it reserved, or a duplicated lane id, fails these checks. A history checkpoint or imported snapshot must carry each genesis lane once, with its genesis `reserved_atoms`, and with release fields that are exactly the sum of its recorded releases: `released_lock_nonces` is the set of recorded nonces, `released_atoms` the sum of their amounts, the window fields follow from the recorded heights, no window above the cap, and `release_history_root` is the hash chain over the records, starting for a pruned lane from its pruned aggregate and root (below) (`validate_external_lanes_match_genesis`, `crates/node/src/history.rs`). This rule, not the conservation checks, refuses atoms moved from a lane to an account without a record, since the totals still balance. Where the genesis is at hand (height-zero replay, history checkpoints) the faucet must hold exactly the native supply minus the lanes; a faucet file read without the genesis is only bounded by the supply, and height-zero replay refuses a short one.

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
| The checkpoint binds the lane, chain, contracts, code hashes and depth, and its observed head is at least `confirmation_depth` blocks past the Lock's block | `external_lane_checkpoint_mismatch` |
| The `Locked` nonce, amount, recipient and recipient hash equal the claim | `external_lane_lock_mismatch` |
| The nonce was not released before | `external_lane_nonce_already_released` |
| The amount fits `reserved_atoms - released_atoms` | `external_lane_remainder_exceeded` |
| The release's window is not below the lane's recorded window | `external_lane_window_regressed` |
| The window's released amount stays within the cap | `external_lane_window_cap_exceeded` |
| The lane stores at most `EXTERNAL_LANE_MAX_RELEASE_RECORDS` records after this release prunes and stores | `external_lane_release_records_full` |
| A new recipient account reaches the account reserve | `below_account_reserve` |

The Base checkpoint attestation adds no new schema. It reuses the existing Ethereum checkpoint (`EthereumFinalizedCheckpointV1`, `EthereumExternalEventProofV1`) and its public-values layout; the source chain id, the lock contract and its code hash, the token and its code hash, the committee and the confirmation depth are parameters taken from the lane's genesis policy. The pfUSDC SP1 ingress program is therefore unchanged and still pinned to Sepolia; a guest that proves a Base Lock, its vkey pin and its proof inventory entry are P2.5 work. Today the node returns the refusal codes of the table above: `external_lane_not_configured`, `external_lane_source_mismatch`, `external_lane_committee_missing`, `external_lane_proof_invalid`, `external_lane_checkpoint_mismatch`, `external_lane_lock_mismatch`, `external_lane_nonce_already_released`, `external_lane_remainder_exceeded`, `external_lane_window_regressed`, `external_lane_window_cap_exceeded`, `external_lane_release_records_full` and `below_account_reserve`.

Genesis allows one lane per Base source: two lanes naming the same `chain_id` and `lock_contract_address` are refused at genesis validation and at init, so a Lock can be released by one lane only.

A release to a new PFTL account below the account reserve (`ACCOUNT_RESERVE`, 10 atoms, `crates/execution/src/entrypoints.rs`) is refused with `below_account_reserve` while its Base Lock is already final; the nonce stays unused, and a later retry passes once the recipient account exists. The lock contract refuses amounts below its `minimumLockAmount`, and its constructor refuses a minimum below its constant `PFTL_ACCOUNT_RESERVE` (10); the lock deployment script checks the minimum against the protocol constant `PFTL_ACCOUNT_RESERVE_UNITS` and refuses any other value, and a test in `crates/execution` (`lock_minimum_binding_tests`) keeps both constants equal to `ACCOUNT_RESERVE`, so no lock can be unreleasable; this release-side refusal stays as the second line of defence.

The accepted bound on the stored release records is the lane's reserve divided by the minimum lock amount, since each release takes at least the minimum from the remainder. It does not bound storage: at the planned 10-atom minimum, a slice strictly below the 100-billion-PFT supply (10^17 atoms) allows up to 10^16 records, and the design's hypothetical 1-billion-PFT lane (10^15 atoms) 10^14. `EXTERNAL_LANE_MAX_RELEASE_RECORDS` (1,000,000, `crates/types/src/external_lane_types.rs`) is therefore a hard ceiling: a record is under 400 bytes of JSON, so a lane's list stays under 400 MB and its replay-base check at most a million SHA3-384 steps, far above any expected volume since each record costs a Base Lock and a PFTL fee. A release past it is refused with `external_lane_release_records_full` and changes nothing; pruning (below) keeps a lane within it.

An accepted release credits the recipient, creating the account if needed, adds the nonce and the amount, moves the window when the height is in a new one, and appends a record (nonce, chain, lock contract, amount, recipient, Base block number and hash, PFTL height) and advances `release_history_root`. The atoms move from the lane remainder into circulation, so every conservation check above holds unchanged. The recorded Base block number and hash are the committee's attestation in the checkpoint; only the checkpoint's receipts root is bound to the `Locked` log by the receipt proof, which is what the watcher of design section 5.2 reads them as.

### Pruning the release records

A release also prunes, in the same state transition, so every validator and every replay prunes identically (`ExternalLaneState::apply_release`, `crates/types/src/external_lane_types.rs`). The node has no rollback path for a committed PFTL block, so the release's height is the finalized height. The records released at or below that height minus `EXTERNAL_LANE_RELEASE_RETENTION_WINDOWS` (1) windows of `release_window_blocks`, and in a window before the lane's open one, leave `releases`.

- One window of retention is this change's decision, not a rule of design section 5.2. It follows from 5.2's loss bound of one window's cap, which assumes the watcher sees each release and the zero-cap amendment lands before the next window releases: a record becomes prunable one window after its height and is removed by the next qualifying release, so it stays readable while the watcher can still act on it. This transition removes no block receipt, but later history pruning (`crates/node/src/history.rs`) can remove blocks, receipts and batch-archive entries, so reconstructing pruned releases needs a retained archive or the watcher's own persisted history.
- `release_history_root`, `released_atoms` and the release count keep advancing; `pruned_release_history_root` is the root after the last pruned record, and the stored records chain from it to `release_history_root`.
- `released_lock_nonces` is never pruned, so a pruned nonce stays paid: 8 bytes per nonce in memory (a `BTreeSet<u64>`, about 8 MB per million plus the tree's overhead) and up to 20 bytes of JSON each.
- The open window's records stay, so the window fields still follow from the stored heights.
- The pruned fields are in the state commitment, omitted while nothing is pruned, so an unpruned lane commits as before. They first appear in the commitment at a lane's first pruning transition, so this applies to chains started from a genesis with this code: there is no activation gate, and no live lane exists today.
- The history and snapshot lane rule holds on a pruned lane: the stored records must chain from the pruned root to `release_history_root`, `released_atoms` must equal the pruned atoms plus the stored amounts, and the nonce set must hold the stored nonces and exactly one nonce per release. A forged record, a tampered root and a removed nonce are refused; a nonce swapped for another changes the committed state root.

A lane stores at most `EXTERNAL_LANE_MAX_RELEASE_RECORDS` records, and reaches that ceiling only when that many releases land within the retention window and the open window.

`external_lane_info` (public read RPC, and the `external-lane-info` node command; `--lane-id`, `--release-cursor` and `--release-limit` optional) returns each lane's genesis configuration, committed record, remainder, window cap, what the next block's window may still release, `release_count` (every release), `pruned_release_count`, `release_history_root`, and one page of stored release records newest first, each with its `sequence` in the history; sequences below `pruned_release_count` are pruned and no page returns them. A page holds at most `EXTERNAL_LANE_RELEASE_PAGE_SIZE` (100) records, the default; `next_release_cursor`, when present, is passed as `--release-cursor` for the next older page.

## Governed lane amendments

The design changes a configured lane only by governed amendment: the window cap in basis points (section 5.1) and the committee epoch and root the policy names (section 4.1). Their kinds are `external_lane_release_cap_v1:<lane_id>` and `external_lane_committee_epoch_v1:<lane_id>` (`external_lane_amendment_refusal`, `crates/types/src/external_lane_types.rs`). Neither is built: governance refuses both with `external_lane_amendment_not_supported`, signed or not, at ratification and signed-batch admission (`validate_amendment_kind`, `crates/consensus_cobalt/src/internal_validation.rs`) and again at block execution (`governance_amendment_lifecycle_rejection`, `crates/node/src/execution_actions.rs`). No lane record, cap or policy can change; tests: `crates/node/src/tests/external_lane_amendment_tests.rs`.

Both need more than an amendment kind. Execution, mempool admission and the lane equality rule read only the genesis and the ledger, so the active cap or committee at a height must reach them from governance, and the equality rule must become "genesis plus the applied amendments". The committee root is 48 bytes and does not fit the amendment's `u32` value.

Remaining P2.5 steps:

- [ ] Cap amendment: value 0 to 10,000 bps; a lower cap applies from its activation height, a higher one seven windows after it commits; the latest applied amendment of a lane wins.
- [ ] Pass the active cap to `apply_external_lane_release`, mempool admission and batch building; a release above the amended cap is refused with `external_lane_window_cap_exceeded`.
- [ ] Extend `validate_external_lanes_match_genesis` to check each window against the cap active at its heights, keeping the forged-release and tamper refusals; replay and snapshots stay identical.
- [ ] Committee epoch move: bind the new epoch and root by hash in the kind; apply only while that committee is the newest registered and unfenced; refuse checkpoints of the old epoch from the block after.
- [ ] `external_lane_info` reports the active cap and committee with the amendment that set them.

## Not built yet

The cap amendment and the committee epoch move (above), the watcher (section 5.2), and the Python CLI and interface. The design keeps the SP1 route for Base through its dispute games (option (a)) as a later upgrade.

## Not before

The design note sets three "not before" rules:

1. No genesis code merges before the token document fixes the supply number.
2. No lane is configured before a mainnet genesis plan exists. The [launch specification](l1v2-testnet-genesis-and-launch-spec.md) leaves token migration to a separate document that does not exist yet.
3. The lock contract is not deployed on Base mainnet before its audit is complete.

This work therefore lives on a branch, and no running network's genesis changes.
