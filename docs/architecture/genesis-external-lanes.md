# Genesis External Lanes

A genesis external lane is native PFT that genesis reserves for a claim held outside PFTL. The first use is the ERC-20 PFT on Base. The product owner decided on 2026-10-28 that the total supply is 100 billion PFT and that the Base ERC-20 is a slice of it; the lane holds that slice as native PFT that is not in circulation until a proven Lock on Base releases it. The lane is the $s_{\text{external}}$ term of the [whitepaper's four-way partition](../whitepaper.md#13-supply-perimeter-primary-operations-and-the-cash-leg). [The PFT token on Base](pft-token-on-base.md) is the reader's page for the token, its deployments and this lane together.

The design is `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md`, sections 4, 5 and 5.1. This page describes what the code does: genesis can reserve lanes, the ledger carries their records, every supply check counts them (milestone item P2.2), and `external_lane_release_v1` releases from a lane against a proven Base Lock (P2.4), whose proof committee members build with [the Base checkpoint source](#the-base-checkpoint-source) commands.

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
| `release_cap_bps` | 1 to 10,000: the share of `reserved_atoms` one window may release. The rounded window cap, `reserved_atoms × release_cap_bps / 10,000`, must be positive: a lane that could release nothing is refused, because a zero cap is a governed halt (design section 5.1), not a genesis state. |
| `policy.chain_id` | The source chain, a parameter limited to Base mainnet (8453) and Base Sepolia (84532). |
| `policy.*_address` | `0x` and 40 lower-case hex digits. |
| `policy.*_code_hash` | `0x` and 64 lower-case hex digits. |
| `policy.committee_epoch` | The bridge committee epoch whose checkpoints prove a Lock. |
| `policy.committee_root` | That committee's root (`FastSwapCommitteeV1::computed_root`, 48 bytes): `0x` and 96 lower-case hex digits. |
| `policy.confirmation_depth` | Positive: the Base confirmations a Lock needs before it can be proven. |
| `policy.exports_enabled` | Optional, default `false`: whether `external_lane_export_v1` is applied ([Exports](#exports)). It is written only when `true`, so a genesis without it keeps its bytes and hash. |

Unknown fields are refused at every level, so the policy cannot gain a field later without a schema change. The policy values are checked for shape, and `chain_id` against the two Base sources; the release path verifies proofs against them.

## Ledger and genesis state

At genesis the faucet receives the native supply minus the sum of the lanes, and `LedgerState.external_lanes` gets one record per lane (`crates/node/src/lifecycle_queries.rs`, `genesis_ledger_state`):

- `lane_id` and `reserved_atoms`, from genesis;
- `released_atoms`, starting at 0;
- `released_lock_nonces`, the set of Lock nonces already paid, starting empty;
- `window_index` and `window_released_atoms`, the current window and what it released, starting at 0;
- `release_history_root`, a SHA3-384 hash chain over the release records in application order, omitted while there is none;
- `pruned_release_count`, `pruned_released_atoms` and `pruned_release_history_root`, the records pruned behind the finalized height (below), omitted while none is;
- `releases`, the stored release records, omitted while empty;
- `exported_atoms` and `next_export_nonce`, starting at 0, and `export_history_root`, the pruned export fields and `exports`, the export packets, each omitted while zero or empty ([Exports](#exports)).

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

The example file has one Base Sepolia lane: the rehearsal lock `0xe36e7fc384564419187546c19719a5654266de0b` and token `0x18fe4ca51a0ce042688d5a54917319d03b527d92`, with the code hashes of `crates/pft-token-contracts/deployments/pft-lock-base-84532/archived-0xE36e7Fc384564419187546c19719A5654266dE0B.json` (the lock archived on 2026-11-06), 300,000,000 atoms reserved and a 200 bps cap; its committee root is a placeholder. A test pins its bytes and hash and checks that `init` gives the same ledger and state root as the in-code lane genesis (`init_from_lane_genesis_file_matches_the_in_code_lane_genesis`, `crates/node/src/tests/genesis_external_lane_tests.rs`). `topology-consensus-v2` still derives its domain from a genesis without lanes.

## What the conservation check counts

`native_pft_supply_breakdown` (`crates/node/src/block_replay_wallet.rs`) reports two lines and their total:

- circulating: accounts, escrows, offers, owned objects, FastLane reserves and the Orchard pool;
- external lane remainder: `reserved_atoms - (released_atoms - exported_atoms)` over all lanes.

`native_pft_live_total` is their sum, so the three checks keep their form:

- height-zero replay: live supply equals the genesis supply;
- each block: live before minus fee burns equals live after;
- each history checkpoint: live supply plus cumulative fee burns equals the genesis supply.

A lane that has released more than it reserved, or a duplicated lane id, fails these checks. A history checkpoint or imported snapshot must carry each genesis lane once, with its genesis `reserved_atoms`, and with release fields that are exactly the sum of its recorded releases: `released_lock_nonces` is the set of recorded nonces, `released_atoms` the sum of their amounts, the window fields follow from the recorded heights, no window above the cap active at its heights and each stored release naming the committee active at its height (genesis plus the applied amendments, [below](#governed-lane-amendments)), and `release_history_root` is the hash chain over the records, starting for a pruned lane from its pruned aggregate and root (below) (`validate_external_lanes_match_genesis`, `crates/node/src/history.rs`). This rule, not the conservation checks, refuses atoms moved from a lane to an account without a record, since the totals still balance. Where the genesis is at hand (height-zero replay, history checkpoints) the faucet must hold exactly the native supply minus the lanes; a faucet file read without the genesis is only bounded by the supply, and height-zero replay refuses a short one.

Snapshots, restores and history checkpoints carry the lane records unchanged; tests cover lanes with nothing released and checkpoints with recorded releases, and a checkpoint, snapshot or faucet that drops a lane is refused.

## The release path

`external_lane_release_v1` is an asset transaction operation (`crates/types/src/transactions_mempool_receipts.rs`, `ExternalLaneReleaseOperationV1`). It names the lane, the Lock (`source_chain_id`, `lock_contract_address`, `lock_nonce`), `amount_atoms`, the PFTL `recipient` and the `proof`. Any account may submit it and pays the fee; it is submitted with `mempool_submit_signed_asset_transaction`, and mempool admission and batch building refuse it when execution would.

The proof is the design's option (b): an `EthereumExternalEventProofV1`, a Base block checkpoint signed by a quorum of the committee the lane's policy names, plus the receipt-inclusion proof of the lock contract's `Locked` event. For a lane the checkpoint's route fields carry the lane id, the lane's route-config digest (SHA3-384 over the genesis lane JSON), the lock contract and the token with their runtime code hashes, and the policy's `confirmation_depth` (`crates/execution/src/external_lane_release.rs`, `verify_external_lane_lock`).

Execution checks, in order, and refuses with no state change and no nonce consumed: the proof first (`verify_external_lane_lock`), then the lane rules (`apply_external_lane_release`, rules on `ExternalLaneState`). For a valid initialized ledger, in first-failure order; application repeats the lane, source and lane-rule checks defensively before it mutates, with the same codes.

| Check | Refusal code |
|---|---|
| Genesis configures the lane | `external_lane_not_configured` |
| The Lock is from the lane's chain and lock contract | `external_lane_source_mismatch` |
| The checkpoint's observed head is at least `confirmation_depth` blocks past the Lock's block | `external_lane_checkpoint_mismatch` |
| The checkpoint, votes and receipt proof pass `validate_bounds`: schema and shape, the self-declared depth, canonical vote order, signature and receipt-proof size bounds | `external_lane_proof_invalid` |
| A committee with the epoch and root the lane names at the height (genesis policy or governed move) is registered | `external_lane_committee_missing` |
| The checkpoint carries that committee's epoch and root | `external_lane_committee_inactive` |
| The certificate has a quorum of valid votes | `external_lane_proof_invalid` |
| The checkpoint binds the lane, chain, digest, contracts, code hashes and depth | `external_lane_checkpoint_mismatch` |
| The receipt and the `Locked` log verify | `external_lane_proof_invalid` |
| The `Locked` nonce, amount, recipient and recipient hash equal the claim | `external_lane_lock_mismatch` |
| The release amount is positive (the operation's own validation refuses zero earlier, as a malformed operation) | `external_lane_proof_invalid` |
| The nonce was not released before | `external_lane_nonce_already_released` |
| The amount fits `reserved_atoms - released_atoms` | `external_lane_remainder_exceeded` |
| The release's window is not below the lane's recorded window | `external_lane_window_regressed` |
| The window's released amount stays within the cap active at the height (genesis or governed) | `external_lane_window_cap_exceeded` |
| The pruning calculation recomputes its retained prefix without overflow | `external_lane_proof_invalid` |
| The lane stores at most `EXTERNAL_LANE_MAX_RELEASE_RECORDS` records after this release prunes and stores | `external_lane_release_records_full` |
| Crediting the recipient does not overflow its balance | `amount_overflow` |
| The recipient's final balance reaches its account reserve | `below_account_reserve` |

The Base checkpoint attestation adds no new schema. It reuses the existing Ethereum checkpoint (`EthereumFinalizedCheckpointV1`, `EthereumExternalEventProofV1`) and its public-values layout; the source chain id, the lock contract and its code hash, the token and its code hash, the committee and the confirmation depth are parameters taken from the lane's genesis policy. The pfUSDC SP1 ingress program is unchanged and not used by the lane; an SP1 guest for Base is the design's option (a) upgrade. Today the node returns the refusal codes of the table above: `external_lane_not_configured`, `external_lane_source_mismatch`, `external_lane_committee_missing`, `external_lane_committee_inactive`, `external_lane_proof_invalid`, `external_lane_checkpoint_mismatch`, `external_lane_lock_mismatch`, `external_lane_nonce_already_released`, `external_lane_remainder_exceeded`, `external_lane_window_regressed`, `external_lane_window_cap_exceeded`, `external_lane_release_records_full`, `amount_overflow` and `below_account_reserve`.

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

`external_lane_info` (public read RPC, and the `external-lane-info` node command; `--lane-id`, `--release-cursor` and `--release-limit` optional) returns each lane's genesis configuration, committed record, remainder, the release cap at the next block (`release_cap`: the active cap, the amendment that set it, a pending raise and its height) with its window cap, the committee the next block accepts (`committee`: epoch, root and the move that set it), what the next block's window may still release, `release_count` (every release), `pruned_release_count`, `release_history_root`, and one page of stored release records newest first, each with its `sequence` in the history; sequences below `pruned_release_count` are pruned and no page returns them. A page holds at most `EXTERNAL_LANE_RELEASE_PAGE_SIZE` (100) records, the default; `next_release_cursor`, when present, is passed as `--release-cursor` for the next older page.

## Governed lane amendments

The design changes a configured lane only by governed amendment: the window cap in basis points (section 5.1) and the committee epoch and root the policy names (section 4.1). Their kinds are `external_lane_release_cap_v1:<lane_id>` and `external_lane_committee_epoch_v1:<lane_id>:<binding>` (`external_lane_amendment_refusal`, `crates/types/src/external_lane_types.rs`), checked at ratification and signed-batch admission (`validate_amendment_kind`, `validate_amendment_value`, `crates/consensus_cobalt/src/internal_validation.rs`) and again at block execution (`governance_amendment_lifecycle_rejection`, `external_lane_committee_amendment_rejection`, `external_lane_amendment_authority_rejection`, `crates/node/src/execution_actions.rs`).

**Who signs, and the proposal slot.** Both kinds follow one rule. Design sections 4.1 and 5.1 ask for the validators' authorization; the node reads that as the registry threshold, the ⌊2n/3⌋ + 1 rule the registry uses for every governed change (`external_lane_amendment_threshold`, 5 of 6), so one offline validator cannot block a halt or a move. The amendment must declare exactly the active validator set, with a quorum and support of at least that threshold, and its signed authorizations, verified at signed-batch admission, must reach the quorum (`external_lane_amendment_authority_refusal`). Its `activation_height` must be the proposal slot it is signed for, the height of the block that commits it, and it must not be paused (`external_lane_amendment_slot_refusal`), so each request has its own amendment id and a repeated identical request is a new amendment. The Cobalt ratifier and verifier check the declared set; node ratification, signed-batch admission (`verify_external_lane_amendment_admission`, `crates/node/src/consensus_artifacts.rs`) and block execution (`external_lane_amendment_authority_rejection`) check the active set and refuse a lane genesis does not configure (`external_lane_amendment_lane_refusal`). The codes are `external_lane_release_cap_authority_invalid`, `external_lane_release_cap_slot_invalid` and `external_lane_release_cap_lane_not_configured` for a cap amendment, and `external_lane_committee_move_authority_invalid`, `external_lane_committee_move_slot_invalid` and `external_lane_not_configured` for a committee move. `governance-authorization-sign` runs the Cobalt verifier and requires the active set before signing, so it also refuses an amendment below the registry threshold (the six-validator rehearsal of 2026-11-04 saw `external_lane_release_cap_authority_invalid: … registry threshold 5 of 6` at signing); the governance-only replays without a ledger or genesis (`execute_governance_batch` in the governance-agent gates and some tests) check a cap amendment's authority and slot only, as the lane check needs the genesis, and refuse a committee move by its ledger rule.

The node's ratify commands for these two kinds (`ratify-external-lane-release-cap`, `ratify-external-lane-committee-epoch`) declare the full active set at quorum ⌊2n/3⌋ + 1 and accept a `--support` of at least that threshold, so five of six validators ratify, sign and assemble a halt or a move while one is offline (`ratify_governance`, `crates/node/src/governance.rs`); every other governance kind still needs each declared validator.

**Release cap.** The amendment's value is the new `release_cap_bps`, 0 to 10,000; a larger value, or a kind without one well-formed lane id, is refused with `invalid_external_lane_release_cap`. A halt repeated after a restore is a new amendment and cancels the pending raise. The node command is `ratify-external-lane-release-cap --lane-id ID --cap-bps N --activation-height SLOT`, then the usual signing, assembly and governance batch. The cap in force at a height (`GenesisExternalLane::release_cap_schedule_at`) follows the lane's committed cap amendments in commit order, each at the height governance applied it:

- an amendment at or below the cap in force applies from that height; a cap of zero refuses every release and moves no PFT;
- an amendment above it applies seven windows later, `release_window_blocks` × 7 blocks (`EXTERNAL_LANE_RELEASE_CAP_RAISE_DELAY_WINDOWS`);
- each committed amendment replaces a raise still in its delay, so a zero cap committed during a pending raise holds; amendments at one height apply in commit order.

Execution, mempool admission and batch building take the cap from governance (`GovernanceState::external_lane_release_caps`, carried in `AssetExecutionCompatibility`) and refuse a release above it with `external_lane_window_cap_exceeded`: the lane and the Lock nonce do not change. Once the rejected-transaction fee is active (`rejected_tx_fee_activation_height`), such a release that reaches a certified block still burns the payer's fee and consumes its sequence, with code `tec_external_lane_window_cap_exceeded` (`crates/execution/src/rejected_fee_claim.rs`). The cap table skips a cap amendment without its activation record rather than dating it at height 0, and a lane past its eighth slot, which genesis parse already refuses, would release nothing. The lane rule of history checkpoints and snapshot import checks each window against the cap active at the height of each of its releases (`validate_against_genesis_with_caps`). Genesis configures at most eight lanes (`EXTERNAL_LANE_MAX_LANES`), the size of that cap table. The lock contract's `maxLockAmount` is not changed by the amendment. Tests: `crates/types/src/external_lane_type_tests.rs`, `crates/execution/src/external_lane_release.rs`, `crates/node/src/tests/external_lane_amendment_tests.rs`, and in `crates/node/src/tests/genesis_external_lane_tests.rs` `governed_release_cap_lowers_at_once_and_raises_after_seven_windows` (governance round, admission, replay from genesis and a snapshot restore on a local chain), `release_cap_amendment_needs_the_registry_threshold_of_the_active_set`, `release_cap_amendment_on_six_validators_needs_five_signatures`, `a_fresh_halt_after_a_pending_restore_is_applied_and_cancels_the_raise` and `release_above_the_cap_in_a_block_burns_the_fee_only_after_fee_activation`.

**Committee epoch.** The amendment's value is the new epoch; the 48-byte root does not fit it, so the kind names a binding: SHA3-384 over the ASCII domain `postfiat.external_lane.committee_move.v1`, a 0x00 byte, the lane id bytes, a 0x00 byte, the epoch as a big-endian u64 and the raw 48-byte committee root, rendered as 96 lowercase hex digits (`external_lane_committee_binding`). `ratify-external-lane-committee-epoch` computes it from `--lane-id`, `--committee-epoch` and `--committee-root`, and the read-only `external-lane-committee-binding` command with the same three flags prints the kind and binding. A kind of the family without one well-formed lane id and binding, the bare prefix included, is refused with `invalid_external_lane_committee_move` at ratification, signed-batch admission and block execution. At ratification and again at block execution (`external_lane_committee_move_refusal`, `crates/types/src/external_lane_types.rs`; `external_lane_committee_amendment_rejection`, `crates/node/src/execution_actions.rs`) a move is refused:

- with `external_lane_not_configured` when the ledger has no such lane;
- with `external_lane_committee_binding_mismatch` when no registered committee of that epoch has a root the binding hashes, such as a forged root;
- with `external_lane_committee_not_current` unless that committee is the newest registered and has no prepare fence, the check the routes use;
- with `external_lane_committee_epoch_not_advanced` unless the epoch is above the lane's genesis policy epoch and every epoch it moved to before (`external_lane_committee_epoch_refusal`, read from the genesis lanes and governance, so signed-batch admission checks it too);
- with `external_lane_committee_ledger_unavailable` when execution has no ledger or genesis lanes, as the FastSwap bootstrap needs the ledger. The ledger-less certificate replay drops a move this way while the state replay keeps it; the state root check reads the state replay's copy.

Signed-batch admission has no ledger: it checks the lane, the authority, the slot and the epoch advance, not the binding or the newest-unfenced rule. Signing an externally prepared move does not re-run those ledger checks; block execution refuses such a move in the block receipt.

A move holds from the block after it commits, and the latest move in force wins (`GenesisExternalLane::committee_schedule_at`). Execution, mempool admission and batch building take the committee from governance (`GovernanceState::external_lane_committees`, carried in `AssetExecutionCompatibility`). A release checkpoint must carry that committee's epoch and root, or it is refused with `external_lane_committee_inactive` with no state change and no nonce consumed. Once the rejected-transaction fee is active (`rejected_tx_fee_activation_height`), such a release that reaches a certified block still burns the payer's fee and consumes its sequence, with code `tec_external_lane_committee_inactive`; the lane and the Lock nonce do not change. A Lock attested only by the old epoch is released when the new committee attests the same Base block. A release proven under a moved committee records its epoch (`committee_epoch` on the release record; absent under the genesis committee, so a lane that never moved records as before), and the lane rule of history checkpoints and snapshot import checks that each stored release names the committee active at its height (`validate_against_genesis_governed`). The second committee is registered by the existing lifecycle: the old committee's drained final FastLane checkpoint is anchored, then `ActivateCommittee`. The node command is `ratify-external-lane-committee-epoch --lane-id ID --committee-epoch N --committee-root HEX --activation-height SLOT`, then the usual signing, assembly and governance batch; a second move to the same epoch stays refused by the epoch rule. Tests: `crates/types/src/external_lane_type_tests.rs`, `crates/execution/src/external_lane_release.rs`, `committee_move_binds_the_newest_unfenced_registered_committee`, `committee_move_ratification_checks_the_registered_committees`, `committee_move_needs_the_registry_threshold_of_the_active_set`, `committee_move_needs_the_active_set_its_slot_and_a_configured_lane_at_admission_and_execution`, `committee_move_to_the_genesis_epoch_is_refused_at_admission_and_execution`, `committee_kind_without_a_binding_is_refused_at_execution`, `lane_amendment_ratify_command_on_six_validators_accepts_the_registry_threshold`, `old_committee_release_in_a_block_burns_the_fee_only_after_fee_activation` and `governed_committee_move_refuses_the_old_epoch_and_releases_the_lock_re_proven` in `crates/node/src/tests/genesis_external_lane_tests.rs` (`ActivateCommittee` in blocks, a governance round, the old checkpoint refused, the re-proven Lock released, replay from genesis and a snapshot restore on a local chain).

Remaining P2.5 steps:

- [x] Cap amendment: value 0 to 10,000 bps; a lower cap applies from its activation height, a higher one seven windows after it commits; the latest applied amendment of a lane wins.
- [x] Pass the active cap to `apply_external_lane_release`, mempool admission and batch building; a release above the amended cap is refused with `external_lane_window_cap_exceeded`.
- [x] Extend `validate_external_lanes_match_genesis` to check each window against the cap active at its heights, keeping the forged-release and tamper refusals; replay and snapshots stay identical.
- [x] Committee epoch move: bind the new epoch and root by hash in the kind; apply only while that committee is the newest registered and unfenced; refuse checkpoints of the old epoch from the block after.
- [x] `external_lane_info` reports the active cap with the amendment that set it, and a pending raise with its height (`release_cap`).
- [x] `external_lane_info` reports the active committee (epoch and root) with the move that set it (`committee`).

## The Base checkpoint source

Committee members observe Base and sign lane checkpoints with four node commands (`crates/node/src/external_lane_checkpoint_source.rs`). Each takes `--data-dir` and reads the lane's genesis policy and the committee the lane names at the next PFTL block: the genesis policy's, or the one the last committed committee move binds, the selector `execute_external_lane_release` uses. The Base RPC is an `http://` endpoint. What a member trusts is its own, independently operated Base node and that node's `finalized` tag: the state derived from finalized Ethereum data, as that node reports it. The commands do not verify Base finality against Ethereum. If five members use one shared RPC, the quorum's independence collapses into trust in that RPC, which the design's committee-trust model (section 4 (b)) does not cover. A refusal carries a typed code (`ExternalLaneCheckpointRefusal`).

| Command | What it does | Refusals |
|---|---|---|
| `external-lane-checkpoint-observe --lane-id ID --base-rpc URL [--block-number N] --checkpoint-file PATH` | Reads the block that this member's Base node reports under the `finalized` tag, or block N at or below it. Writes the lane checkpoint (`EthereumFinalizedCheckpointV1`): block number, hash, receipts root, observed head, lane id and route-config digest, chain id, lock contract, token, their code hashes, depth, and the active committee's epoch and root. | `base_chain_id_mismatch`, `base_block_not_finalized`, `base_confirmation_depth_not_reached` (head minus block below `confirmation_depth`), `base_lock_code_hash_mismatch`, `base_token_code_hash_mismatch` (keccak-256 of `eth_getCode` at the block), `external_lane_not_configured`, `external_lane_committee_missing` |
| `external-lane-checkpoint-sign --checkpoint-file PATH --base-rpc URL --validator ID --validator-key-file PATH --vote-file PATH` | Signs one vote with the member's FastSwap committee key after observing the same block from its own RPC, and verifies the vote under the committee public key before writing it. The signing state lives under `--data-dir`, in `external-lane-checkpoint-signing/`. It keeps one checkpoint per lane, committee epoch and block number: the same checkpoint again returns the same vote. | `external_lane_not_configured`, `external_lane_committee_missing`, `external_lane_checkpoint_mismatch` (not the lane's or not the active committee's), `base_confirmation_depth_not_reached`, `checkpoint_signer_not_member`, `checkpoint_signing_key_mismatch` (not the committee key, an unusable key file or one without the member's record, or a new vote that does not verify), the observe refusals of its own RPC (`base_chain_id_mismatch`, `base_block_not_finalized`, `base_lock_code_hash_mismatch`, `base_token_code_hash_mismatch`), `base_observation_mismatch`, `checkpoint_signing_state_invalid` (a corrupt or foreign signing state), `checkpoint_conflicting_vote` |
| `external-lane-checkpoint-assemble --checkpoint-file PATH --vote-files CSV --certificate-file PATH` | Writes the certificate with exactly the quorum of votes (5 of 6 for a six-member committee; with more valid votes, the lowest validator ids), verified with `verify_ethereum_checkpoint_certificate` first. | `external_lane_not_configured`, `external_lane_committee_missing`, `external_lane_checkpoint_mismatch`, `base_confirmation_depth_not_reached`, `checkpoint_vote_duplicate`, `checkpoint_vote_not_member`, `checkpoint_vote_other_checkpoint` (a vote that does not sign this checkpoint), `checkpoint_votes_below_quorum`, `checkpoint_certificate_invalid` |
| `external-lane-lock-proof --lane-id ID --base-rpc URL --transaction-hash 0xHASH --certificate-file PATH --submitter ADDRESS [--log-index N] --operation-file PATH` | Fetches the Lock's receipt, its block and every receipt of the block, rebuilds the receipt trie to the block's receipts root (OP Stack deposit receipts included) and builds the receipt proof. The Base RPC must serve full receipts through `eth_getBlockReceipts`, with each deposit receipt's `depositNonce` and `depositReceiptVersion`; without them every Base block's rebuild fails closed, because the rebuilt root does not match the block's receipts root. Takes the receipt's one `Locked` log of the lane's lock contract (or log N) and decodes nonce, amount and recipient. As an extra check of this command, not a consensus rule, requires the receipt to hold an ERC-20 `Transfer` of the amount from the holder to the lock contract emitted by the policy's token, and ignores same-shaped transfers of other contracts; consensus binds the lock's address and runtime code hash, and the lock's token is immutable. Writes the `external_lane_release_v1` operation. | `external_lane_not_configured`, `external_lane_committee_missing`, `external_lane_checkpoint_mismatch`, `base_confirmation_depth_not_reached`, `checkpoint_certificate_invalid`, `base_chain_id_mismatch`, `lock_receipt_block_mismatch` (not the certificate's block), `lock_receipt_log_other_contract`, `lock_receipt_log_other_event`, `lock_receipt_locked_log_missing`, `lock_receipt_locked_log_ambiguous`, `lock_receipt_locked_event_invalid`, `lock_receipt_token_mismatch`, `external_lane_release_operation_invalid` (a submitter or recipient that is not a PFTL address) |

One release, step by step:

1. One member runs `external-lane-checkpoint-observe --data-dir DIR` once the Lock's block is finalized and at least `confirmation_depth` blocks deep, and shares the checkpoint file.
2. Five of the six members run `external-lane-checkpoint-sign --data-dir DIR`, each with its own data directory and its own Base RPC, and share their vote files.
3. Anyone runs `external-lane-checkpoint-assemble --data-dir DIR` with the five votes.
4. Anyone runs `external-lane-lock-proof --data-dir DIR` with the Lock's transaction hash and the certificate. The same inputs give byte-identical operation JSON.
5. The submitter signs it: `wallet-sign-asset-transaction --key-file PATH --chain-id ID --genesis-hash HASH --protocol-version N --fee F --sequence S --operation-json "$(cat operation.json)"`.
6. The signed transaction goes to `mempool_submit_signed_asset_transaction`; execution checks it as [the release path](#the-release-path) describes.

The signing-state directory `external-lane-checkpoint-signing/` is signing-safety state: it is what stops a member from signing two checkpoints of one block. Never restore or recreate a validator's data directory without it, and keep it with the key backups for the life of the committee key.

Tests (`crates/node/src/tests/external_lane_checkpoint_source_tests.rs`, against a mock Base JSON-RPC) cover:

- **Observe:** every observe refusal.
- **Sign:** a member that observes another block, a non-member, a key that is not the committee key, a key file whose private key is another key's, a key file without the member's record, a conflicting checkpoint, a corrupt or foreign signing state, a checkpoint of the committee before a governed move, and the active committee after it.
- **Assemble:** duplicate, non-member and other-checkpoint votes, and too few votes.
- **Lock proof:** another block, contract or event; a receipt with no `Locked` log or two; a recipient that does not match its indexed hash; the wrong token, and the token's transfer accepted beside another contract's same-shaped transfer; a forged certificate; an invalid submitter or recipient; and the decoded fields at their boundaries (nonce and amount at 0, 1 and `u64::MAX`, a 65-bit word, the recipient at 0, 41, 42, 43 and 256 bytes, and one token unit per atom, unscaled).
- **Committee missing:** all four commands refuse when the lane's committee is not on the ledger.
- **End to end:** on a six-member lane chain: observe, five votes, certificate, proof, then the chain releases the Lock once and refuses it again. Four votes, and a sixth vote on another block hash, are refused at assembly.

The tests do not exercise these refusals:

- `external_lane_not_configured` and `base_confirmation_depth_not_reached` on sign, assemble or proof.
- `external_lane_checkpoint_mismatch` on assemble or proof.
- The observe refusals of sign's own RPC.
- `base_chain_id_mismatch` on proof.
- `checkpoint_certificate_invalid` on assemble.

 `live_base_sepolia_checkpoint_of_the_rehearsal_lock` (ignored; `POSTFIAT_LIVE_BASE_RPC`) runs observe, five signatures and assembly against Base Sepolia with the rehearsal lock and token. It also rebuilds that block's receipt trie; on 2026-11-05 the rebuilt root matched the block's receipts root (the run record is the section 'Base checkpoint source (2026-11-05)' of [the qualification record](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-release-path-qualification-20261103.md)). On 2026-11-06 the four commands proved the first real Lock, on the Base Sepolia rehearsal lock, and a local six-validator fleet released it ([part 2 of the rehearsal](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-release-path-qualification-20261103.md#base-sepolia-rehearsal-2026-11-06-part-2-the-release-on-a-local-fleet)).

## The watcher

`python/postfiat_rpc/external_lane_watcher.py` is the watcher of design section 5.2: it matches every release on PFTL against its Lock on Base and raises an alarm on the first release that no Lock backs. With `--exports` it also checks the outbound direction ([the export view](#the-export-view)). It only reads, on both chains, and holds no key.

Each run reads the lane's row from a PFTL node (`external_lane_info`: the lane policy and the stored release records, paged) and, on Base, the block with the `finalized` tag. Then:

1. The Base RPC's chain id must be the policy's `chain_id`; otherwise the run stops (`base_chain_id_mismatch`, exit 1).
2. The lock contract's runtime code (`eth_getCode`, at the anchor block below) must hash (keccak-256) to the policy's `lock_runtime_code_hash`.
3. Each release in range is checked, oldest first:
   - A release whose `source_block_number` is above the finalized block is not judged. It is reported as `release_block_not_finalized`, nothing else is checked for it in that run, and a later run checks it again.
   - Otherwise the Base block at `source_block_number` must have the release record's `source_block_hash`. A different hash is confirmed before `release_source_block_mismatch` is raised: read by hash, and read again by number in a second, independent read, both must name it; otherwise the run is refused.
   - `lockRecord(lock_nonce)`, read at that same source block by its hash (EIP-1898, `requireCanonical`) and not at the head, must return a holder, the release's `amount_atoms` and the keccak-256 of its UTF-8 `recipient`. A Lock made after the release therefore cannot back it. This is the lock's `LockRecord` storage entry, which carries the same three fields as the `Locked` event the design names.
   - One token base unit is compared as one atom, with no scaling.

**One Base chain for every read.** Checking `eth_chainId` once does not bind the answers that follow: a load balancer can send one read to a node of another network. So the run reads the finalized block once, after the chain id, and keeps its number N and hash H as the anchor; every Base answer it uses is bound to it:

| Read | Bound by |
|---|---|
| `eth_chainId` | equal to the policy's chain id (`base_chain_id_mismatch`) |
| finalized block | the anchor; read again by number N at the end of the run, which must return H |
| `eth_getCode`, `token()`, `totalLocked()`, `totalReleased()`, `balanceOf(lock)` | `{"blockHash": H, "requireCanonical": true}` |
| `lockRecord(nonce)` | the release's source block hash, `requireCanonical` |
| a release's source block by number | a hash other than the release's must be canonical at that number: read by hash and read again by number |
| `eth_getLogs` (range up to N) | each log not removed, with a block hash at or below N that is canonical at its number: read by hash (`eth_getBlockByHash`) and by number (`eth_getBlockByNumber`), both naming it, once per block per run, one hash per number; when the totals then disagree, the range is read again and must give the same logs |

An honest node of another network does not know H or the blocks of this chain, so it answers an error or no block, and the run is refused: `base_anchor_unknown` (a by-hash read the node does not answer) or `base_anchor_mismatch` (a log that is removed, has no hash or names a block that is not canonical at its number, two differing reads of the logs, or an anchor that changed by the end of the run). Known is not canonical: a node also serves by hash a side block it kept from before a reorg (EIP-1898; geth's `BlockByHash`), so a fork that shares this chain's history can name a block this node knows. That is why a log's block, and a differing source block, must also be the canonical block at its number. Both exit 1 and are retried under `--interval`; no `payout`, `export` or `summary` line is printed (`release` lines printed before the export view stand on bound reads) and the state file is not written. The guarantee: when any single Base read of a run is answered by an honest node of another network, or of a fork whose blocks this node keeps as side blocks, the run is refused or gives exactly the all-honest result. A log answer from another network names blocks this node does not know; an empty one hides a payout only by changing the totals, since `PFTLockV1` releases no zero amount, and the second read then differs. What remains assumed: a node that lies, rather than answering honestly for another network, is out of scope, and the anchor is "finalized" as the node reports it. The PFTL endpoint is assumed to be a node of the validated fleet: a PFTL node of a fork that shares this chain's genesis and lane identity is not detected, so an export page from such a fork can back a payout. Binding export pages to an independently confirmed PFTL snapshot is the next step. Anvil (1.8) accepts the by-hash block parameter for `eth_call` and `eth_getCode` and answers an error for an unknown hash, as geth does.

**What the Base endpoint must support.** EIP-1898 (`blockHash` with `requireCanonical`) for `eth_getCode` and `eth_call`: an endpoint without it refuses every run as `base_anchor_unknown`, by design, with no fallback to block numbers or tags. Historical state at the finalized block and at each release's source block: an archive-capable endpoint. Log validation reads block headers only and needs no historical state.

The run fails closed when the node's lane report changes under it. Every page must carry the first page's chain id, genesis hash, `current_height`, `release_count`, `pruned_release_count` and `release_history_root`. Any difference stops the run (`external_lane_info_inconsistent`, exit 1, retried under `--interval`). The sequences must run without a gap from `release_count − 1` down to the pruned count; a gap is `external_lane_info_malformed`.

| Alarm | Meaning |
|---|---|
| `release_without_lock` | `lockRecord` returns no holder: no Lock has this nonce. |
| `release_amount_mismatch` | The Lock's amount is not the release's. |
| `release_recipient_mismatch` | The Lock's `pftlRecipientHash` is not the hash of the release's recipient. |
| `release_block_not_finalized` | The release's source block is above Base's finalized block; a later run checks it again. |
| `release_source_block_mismatch` | The Base block at the release's source block number has another hash than the release names. |
| `lock_code_hash_mismatch` | The lock contract's code at the finalized block is not the pinned code. |
| `release_pruned_before_check` | Release records were pruned before a run checked them, so they cannot be matched. |

The summary line counts each of these codes (`alarm_codes`).

| Run error (exit 1) | Meaning |
|---|---|
| `input_invalid` | A bad `--base-rpc` or `--pftl-rpc` value. |
| `pftl_rpc_failed` | The PFTL `external_lane_info` read failed; retried under `--interval`. |
| `base_rpc_failed` | A Base read failed or answered an error object; retried under `--interval`. |
| `external_lane_not_reported` | The PFTL node reports no lane with `--lane-id`. |
| `external_lane_policy_malformed` | The lane's policy (chain id, lock contract, code hash) is not well formed. |
| `external_lane_info_malformed` | The lane report is not well formed: a counter, the protocol version, the history root, or a gap in the release sequences. |
| `external_lane_info_inconsistent` | The lane report changed while it was paged; retried under `--interval`. |
| `release_record_malformed` | A release record is not well formed, including a zero amount. |
| `base_chain_id_mismatch` | The Base RPC is on another chain than the policy names. |
| `base_answer_malformed` | A Base answer is malformed, names another block than asked, or the node does not serve the release's source block. |
| `state_file_invalid` | The state file is malformed, of an earlier schema, of another network or lane policy, or ahead of the node. |
| `state_file_write_failed` | The state file could not be written; nothing of the run is printed, and the run is retried under `--interval`. |

Output is JSON lines: one `release` line per release checked (`status` `backed` or `alarm`), one `alarm` line per alarm and one `summary` line per run. A one-shot run (`--once`, the default) exits 0 when clean, 2 on any alarm and 1 on a run error, reported as one `error` line on stderr. `--interval` keeps running after an alarm: the alert signal is the `alarm` line, which is printed at once, and the loop continues.

`--state-file` keeps the PFTL height checked through and the release count. It is bound to the PFTL chain id, the genesis hash and the lane policy's lock contract and chain id, and it is written before the run's lines are printed, so a clean summary always means the state advanced. The state advances only after a clean run, so an alarm is raised again on every run until an operator starts past it with `--from-height`. A lost or deleted state file means the next run starts from height 0 and re-checks every stored release. A state file of an earlier schema is refused; delete it, or pass `--from-height`, which ignores the file's release part and replaces the file after a clean run. It still carries the export view's known payouts (see the state file below).

An operator runs it beside a validator, against that validator's RPC and an independently operated Base node, either as a timer with `--once` or as a service with `--interval`, which keeps running and retries after an RPC error:

```bash
PYTHONPATH=python python -m postfiat_rpc.external_lane_watcher \
  --pftl-rpc 127.0.0.1:27650 --lane-id base-erc20 \
  --base-rpc http://127.0.0.1:8545 --state-file /var/lib/postfiat-watcher/base-erc20.json --interval 60
```

It must run at least once per release window: a release record becomes prunable one window after its height ([pruning the release records](#pruning-the-release-records)), and a release pruned before a run checked it is only reported as `release_pruned_before_check`. The Base node must also serve each release's source block. A node that has pruned it fails the run (exit 1, retried under `--interval`), so run the watcher at least once per release window or point it at an archive node.

What it does not do:

- A Lock made on Base but never released is not an alarm: its PFT waits in the lane.
- It trusts its own Base node's `finalized` tag, as each committee member does, and takes its answers as honest; an answer from another network is refused (above), a false one is not detected.
- It trusts the lane read of the PFTL node it is pointed at, and the Lock answers of its Base node. So it runs beside the operator's own validator and against an independently operated Base node.
- It answers nothing. The response of section 5.2 is governed: a zero-cap amendment (`ratify-external-lane-release-cap --cap-bps 0`, signed by the registry threshold), then a committee rotation, then the delayed restore of the cap. The watcher only prompts them.

### The export view

`--exports` adds the outbound direction to the same run, under the same rules: it reads both chains only, uses finalized Base blocks only (`Released` logs up to the anchor block, each bound to it, the totals at it by hash), and follows the same exit codes, plus exit 3 for an accepted baseline. Its options:

- `--exports` turns the view on.
- `--lock-from-block N` (default 0) sets the first Base block of the `Released` scan. It must be at or below the lock's deployment block, or `totalReleased` cannot be matched.
- `--unpaid-warning-blocks N` turns an export that has stayed unpaid for more than N PFTL blocks into a warning.
- `--accept-export-baseline`, the recovery path below.
- `--discard-export-history`, the one way to forget the verified payouts the state knows (see the state file); one-shot runs only.

**The operating rule.** Start the export view when the lane goes live and run it at least once per release window, as the release view. An export packet becomes prunable after its retention, and a payout is checked against its packet only while the packet is stored. Run this way, every payout is checked once against its stored packet and no baseline is ever needed.

It checks three things:

1. **Every payout.** Each `Released` event of the lane's lock must match the stored export packet whose EVM digest (`external_lane_export_evm_digest`) equals the event's `packetDigest`. The recipient, the amount and the withdrawal-id and burn commitments must also match. A payout that matches no stored packet is an alarm: `payout_without_export` when the lane has pruned nothing, `payout_pruned_before_check` when it has, since the watcher never verified it. The archive lookup of the proof-input command needs a data directory and is not available over RPC.
2. **Every export.** Each stored export packet in a final block is reported as `paid`, with the Base transaction, block and log index of its verified payout, taken from the state file's paid record, or `unpaid`, with its age in PFTL blocks. Unpaid is information, not an alarm; with `--unpaid-warning-blocks` set, a long-unpaid export becomes the warning `export_unpaid_too_long`.
3. **The totals,** at the finalized block. Each broken invariant is an alarm:
   - the lock's `totalReleased` equals the sum of every payout up to the finalized block, the payout sum kept in the state file plus the new events (`total_released_mismatch`);
   - the lock's `totalReleased` is at most its `totalLocked`, as `PFTLockV1` enforces (`base_released_above_locked`);
   - `token.balanceOf(lock)` is at least `totalLocked − totalReleased`, as `PFTLockV1` promises; a shortfall is `lock_balance_mismatch`, and a surplus (tokens sent to the lock by a plain transfer) is reported as `lock_surplus` in the totals, not raised. When `totalReleased` is above `totalLocked`, no balance matches the negative expectation: `lock_balance_mismatch` is raised with `base_released_above_locked` and `lock_surplus` is 0;
   - the lane's `exported_atoms` is at least the payouts' sum (`exported_below_paid`);
   - the lane's `released_atoms` is at most `totalLocked` (`released_above_locked`).

The other alarms of the view:

- `payout_burn_mismatch`: the payout's burn commitment differs from the packet's.
- `payout_duplicate`: a payout names a packet digest already paid, in the same run or in an earlier one (from the paid record or the recently pruned paid exports), or a payout other than the recorded one of a digest a rescan carried.
- `payout_unknown_above_pruned`: on a lane with pruned packets, the payouts that match no stored packet are more than the pruned packets not yet accounted for, or pay more than their atoms. Accounted for are the payouts verified before their packets were pruned (`pruned_paid_count`, `pruned_paid_atoms`) and an accepted baseline. With nothing pruned, each such payout is already `payout_without_export`.

Its errors, each one `error` line (exit 1, retried under `--interval`): `base_answer_malformed` (a log or a call answer that is not what the lock emits or returns, or a log outside the asked range or from another address), `export_record_malformed` (an export page that breaks the paging rules: nonces not contiguous down to `pruned_export_count`, a cursor that is not the page's oldest nonce or repeats, a missing or non-integer `export_count`, `pruned_export_count` or `released_atoms`, or `exported_atoms` that is not the pruned atoms plus the stored amounts), `external_lane_info_inconsistent` (the pages came from different snapshots), `export_identity_mismatch` (an export page of another network, lane or policy than the release view; see below), `base_anchor_unknown` and `base_anchor_mismatch` (a Base answer not bound to the run's anchor; these refuse the whole run, see [one Base chain for every read](#the-watcher)), `export_baseline_already_taken` (see the recovery path), `base_finalized_behind_state` (the Base node's finalized block is below the kept one; checked before a rescan or a discard drops or rewrites anything, so a lagging node cannot hide a known payout) and `export_snapshot_behind_state` (the PFTL export snapshot is below the kept one; see the state file). `Released` logs are read in chunks of 10,000 blocks, halved on an RPC error down to 16. A failure of the export view does not hide the release view: its lines are printed first, and the summary names `export_error`.

**One network for both views.** A run reads `external_lane_info` twice: the release view's pages, whose identity (lane, PFTL chain id and genesis, and the policy's source chain id, lock and lock code hash) is checked against the state file, then the export pages. Every export page, the first and each later one, must carry that same identity, checked apart from the snapshot checks between export pages and before any of its packets is read. One that does not is refused with `export_identity_mismatch` (exit 1: the run cannot judge), so an endpoint that fails over to another network between the two reads, or between two export pages, cannot back a payout with that network's exports. No `payout` or `export` line is printed and the export part of the state is unchanged; a clean release view still saves its own progress. Heights and counters are not compared across the two views, so honest progress between the reads passes.

**The state file.** Next to the release cursor, a clean export view keeps `exports`:

- `format`, the entry's format number (1);
- `lock_from_block`, `scanned_through_block` (the finalized Base block scanned through), and `payout_count` and `payout_sum` up to it;
- `paid`, the record of verified payouts of retained exports: for each, `export_nonce`, `evm_digest`, `block`, `transaction`, `log_index`, `amount` and `withdrawal_id_commitment`. A payout whose digest is in it is `payout_duplicate`, and a retained export in it stays `paid` in every later run;
- `pruned_paid_count` and `pruned_paid_atoms`: an entry is dropped only once PFTL has pruned its packet (its nonce is below `pruned_export_count`) and is folded into these two numbers, so the record stays as large as the retained paid exports. A payout verified before pruning is never counted as unknown later, and a new unknown payout still meets the bound;
- `pruned_paid_recent`, the last 4,096 folded paid exports (`RECENT_PRUNED_PAID`), each with the payout that was verified: `export_nonce`, `evm_digest`, `block`, `transaction`, `log_index` and `amount` (about 1.26 MB at u64-wide numbers, 7.5% of the size limit). What is guaranteed: a second payout of a retained paid export or of one in this list is `payout_duplicate` in every run, also under `--accept-export-baseline`, which then accepts nothing. A second payout of an older pruned paid export is not recognised as one: it is `payout_pruned_before_check` in a normal run, and the baseline step is the one place where an operator could accept it, which is why that step prints every payout it accepts and must be read;
- `snapshot`, the export snapshot the run used: its PFTL `height`, `export_count`, `exported_atoms`, `pruned_export_count` and `pruned_exported_atoms`. Each is a high-water mark: a run whose export snapshot is below any of them, or whose pruned count and atoms are below the state's verified-then-pruned payouts plus its baseline, is refused with `export_snapshot_behind_state` (exit 1, the export part kept) before any payout is looked up or any entry folded. A lagging node is not an alarm, but nothing is judged on it: an older snapshot could show a folded export as retained again and take its second payout as `backed`. No paid entry lies below the pruned mark, so each nonce is folded once. The release part keeps its own height (`checked_through_height`, the run's first lane page) and is checked against it;
- `baseline`, once one was accepted (the recovery path).

The whole file is at most 16 MiB (`MAX_STATE_BYTES`), checked when it is written as well as when it is read: a run whose new state would be larger fails with `state_file_too_large` (exit 1, retried under `--interval`) and the last good file stays. A retained paid export takes about 355 bytes at mainnet-sized numbers, so the limit holds about 43,000 retained paid exports next to a full recent list. Waiting does not recover such a run: the payouts it verified but could not save are unknown once their packets are pruned, and then alarm `payout_pruned_before_check`. Raise the constant deliberately (it bounds the load too) and rerun before the packets of the unsaved payouts are pruned. Never delete the good state, or drop entries from it, to get past the error; the baseline step is weaker evidence than the verification it would replace.

The next run reads only later blocks. Each view's part advances only after a clean run of that view; a release alarm keeps both. A state file without `exports` loads, and the view scans from `--lock-from-block`. An `exports` entry without the current `format` is never used and never upgraded in place: the run is a first run from `--lock-from-block` in which nothing is trusted, and its summary says `rescan: export_state_format_old`. There is no migration because the export view was never released: the only earlier entries are this branch's intermediate ones, and they cannot tell which exports they folded at which snapshot, so a lagging node could make a folded export look unpaid. On such a first run a payout whose packet is pruned is an alarm unless the baseline step is taken, and two payouts of one export are `payout_duplicate`. The file's identity (lane, PFTL chain id and genesis, source chain id and lock) is checked against the run's own reads before any part of it is used, on every path: a normal run, `--from-height`, a changed `--lock-from-block` and a discard. Another identity, or a file that cannot be read, is `state_file_invalid` (exit 1, the file unchanged); `--from-height` replaces only a readable file of an earlier schema. A rescan (`--lock-from-block` changed, `rescan: lock_from_block_changed`, or `--from-height`, `rescan: from_height`) rebuilds the scan position and the accounting from `--lock-from-block`, but never forgets a payment it knew: the payouts of `paid` and `pruned_paid_recent` are carried for comparison only. The recorded event (same block, transaction, log index and amount) is the original, in a normal run too: for a pruned packet its payout line says `verified_earlier` and it is accounted as verified, not judged again; for a retained one it is judged again against the packet. A known payout of a retained export that a rescan does not reach stays in `paid`. Any other payout of a known digest is `payout_duplicate`, so the baseline step never accepts one. The summary's `known_payouts_carried` gives their number. `--discard-export-history` is the only way to forget them: the run starts as a first run, prints a `history_discarded` line with the `identities`, `verified_payouts` and `verified_atoms` it drops, and saves no history after a clean run. Duplicate protection for every earlier payout ends there.

**The recovery path.** An export view that starts late, or that was not run for longer than a release window, can see payouts whose packets were pruned before any check, and each is `payout_pruned_before_check` on every run. The operating rule makes this path unnecessary. `--accept-export-baseline` (only with `--exports`, never with `--interval`) is the one explicit way past them, as `--from-height` is for releases. It checks everything that can still be checked: every payout whose packet is stored, the four totals, and the count and sum of the unknown payouts against the pruned packets not yet accounted for (see `payout_unknown_above_pruned`). If any of these fails, the alarms stand and no baseline is kept. Otherwise it prints each never-verified payout as a `baseline_payout` line (block, transaction, amount, recipient, digest), which the operator must read (a second payout of a paid export pruned before the recent list can be among them), keeps the scan position with `baseline` (`baseline_accepted: true`, `accepted_at` in UTC, the cumulative `unverified_payouts` and `unverified_atoms`, and the lane's `pruned_export_count` and `pruned_exported_atoms` at acceptance), and exits 3. Every later summary repeats `baseline`. A baseline is taken once: a later `--accept-export-baseline` is refused with `export_baseline_already_taken` (exit 1, the state kept) unless the lane's pruned count and pruned atoms have both grown since, and even then the cumulative accepted payouts must stay within the pruned packets not yet accounted for, so capacity already used is never used again. It does not prove that those payouts had export packets: only that there were at most as many as the pruned packets and that they paid no more than the pruned atoms. Checking each one needs the pruned packets from a retained archive.

**The trust.** Two assumptions, stated plainly. The state file is trusted local data: the watcher checks its form and its identity on every path, not its history, so it must be kept where only the operator can write it. The RPCs' reads are taken as honest: the view compares what a PFTL node and a Base node report, and is not a cryptographic check of either. It trusts its PFTL node as the release view does. A PFTL node behind the state's export snapshot is refused (`export_snapshot_behind_state`), so a failover to a lagging node cannot bring back a pruned export. One behind the chain but not behind the state errs on the safe side: a payout of an export it has not applied yet shows as `payout_without_export` and `exported_below_paid`, false alarms that clear once the node catches up, never a missed one. For the same reason `released_atoms` is read from the run's first lane page, before the finalized Base block is fixed, and `exported_atoms` from the export pages, after it.

`payout`, `export`, `baseline_payout`, `history_discarded` and `warning` lines join the output. A `payout` line's `status` is `backed` (it matches its stored packet), `alarm`, `verified_earlier` (the original of a known payout whose packet is pruned), `matched_incomplete_scan` or `baseline_unverified` (accepted by the recovery path); an `export` line's is `paid`, `unpaid` or `matched_incomplete_scan`. The totals are read before any payout is judged: when the scanned payouts do not add up to `totalReleased` at the finalized block (a scan start after an unseen payout, `total_released_mismatch`), every match of that run says `matched_incomplete_scan`, never `backed`, `verified_earlier` or `paid`, and the export part of the state is unchanged (a clean release view still saves its own height, so the file as a whole may differ). The summary gains `exports` (the counts, the totals, `paid_recorded`, `pruned_paid_count`, `pruned_paid_atoms`, `rescan`, `known_payouts_carried` and any `baseline`), `warnings` and `warning_codes`.

## Exports

`external_lane_export_v1` (`ExternalLaneExportOperationV1`) moves native PFT from a sender back into the lane, to be paid on Base as ERC-20 PFT. The design is `docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md`, sections 3 and 4.

An export also needs the block-header export root active at its height (`external_lane_export_root_not_active` otherwise). Switching the root on, for a new chain or a running network, is in [External Lane Export Root Activation](../runbooks/external-lane-export-root-activation.md).

**The switch.** A lane applies exports only when its genesis policy sets `exports_enabled: true`. Otherwise every export is refused as before: an unknown lane with `external_lane_not_configured`, the lane's lock contract as the Base recipient with `external_lane_export_recipient_is_lock`, and every other export with `external_lane_export_not_supported`, at mempool admission, batch building and execution; in a certified block the ordinary rejected-transaction fee rule applies (`crates/execution/src/rejected_fee_claim.rs`).

**The rules.** With the switch on, after the lane and lock-address checks (`apply_external_lane_export`, `crates/execution/src/external_lane_release.rs`):

1. The halt: while the lane's active release cap is zero, the export is refused with `external_lane_export_halted`, so the governed zero-cap amendment stops exports as it stops releases. The halt is enforced at mempool admission, batch building, certified-block execution and replay, which all take the governed caps. The bare library entry `execute_asset_transaction` knows only the genesis caps and committees, so it refuses an export that passes every rule with `external_lane_export_needs_governed_caps`, and a verified release with `external_lane_release_needs_governed_caps`, since a release had the same bypass of the cap and the committee. Callers pass them through `execute_asset_transaction_with_compatibility`: `with_external_lane_release_caps` marks the caps governed and `with_external_lane_committees` the committees (`ExternalLaneGovernedInputs`, in memory only). An export needs the caps; a verified release needs both, and with the caps alone it is refused with `external_lane_release_needs_governed_committees`.
2. `amount_atoms` must be at most `released_atoms - exported_atoms` (`external_lane_export_above_released`); `next_export_nonce` must be able to advance (`external_lane_export_nonce_exhausted`); the stored packets must stay under the record ceiling (`external_lane_export_records_full`).
3. The sender pays `amount_atoms` on top of the fee, with the ordinary `insufficient_funds` and `below_account_reserve` refusals.
4. Only then `exported_atoms` grows by the amount, the nonce advances and the packet is recorded.

The sender must be the transaction source. There is no minimum above one atom and no refund.

**The accounting.** The remainder is `reserved_atoms - (released_atoms - exported_atoms)`, in checked u64 arithmetic and in that net form, so a release after a full cycle (reserved 100, released 100, exported 100) leaves 99, not an underflow (`ExternalLaneState::remainder_atoms`). Exports above releases are a corrupt record, refused with `external_lane_state_corrupt`; a release whose `released_atoms` would overflow is refused with `external_lane_counter_overflow`. An export lowers circulating PFT and raises the lane's remainder by the same amount, and its fee is burned, so the conservation checks keep their form.

**The packet.** `ExternalLaneExportPacket` holds the lane id, the export nonce, the sender, `base_recipient`, `amount_atoms`, the PFTL height and the accepted transaction id. Each hash is SHA3-384 with its own domain tag, a zero byte between parts:

| Value | Hashed |
|---|---|
| packet digest | `postfiat.external_lane.export_packet.v1`, the packet's JSON |
| withdrawal-id commitment | `postfiat.external_lane.export_withdrawal_id.v1`, the chain id, the lane id, the export nonce (8 bytes big-endian), the packet digest |
| burn-transaction commitment | `postfiat.external_lane.export_burn_tx.v1`, the chain id, the lane id, the transaction id |
| export history root | `postfiat.external_lane.export_history.v1`, the previous root, the packet digest |

Fixed vectors are in `crates/types/testdata/external-lane-export-packet-vectors.json`. The state commitment covers the counters, the history root and the pruned fields, not the packets. Packets are pruned and bounded by the release-record rule ([Pruning the release records](#pruning-the-release-records)), and a replay base must chain its stored packets to the committed root.

**The header root.** From the activation height, every block header carries `external_lane_export_root`. It is the bridge-exit tree over the block's export leaves, in execution order across lanes. Each leaf is SHA3-384 under `postfiat.external_lane.export_root_leaf.v1` over the chain id, the lane id, the export nonce and the packet digest. A block without exports carries the tree's empty value. A block carries at most 4,096 exports (`EXTERNAL_LANE_EXPORT_ROOT_MAX_LEAVES_V1`). Batch building leaves the rest in the mempool for the next block, and a block with more is invalid (`external_lane_export_root_full`). The genesis field `external_lane_export_root_activation_height` sets the height. On a running chain, the governed amendment of the same name sets it, proposed with `postfiat-node ratify-external-lane-export-root-activation-height`. Consensus v2 must be active at the activation height; `init` refuses a genesis that breaks this rule. Before the height is proposed, every node and every validator signer process must run a build that knows the root, because an older signer refuses to sign a block that carries it. The empty value equals the bridge-exit tree's empty value. That is harmless, because the leaf domains differ and the two roots sit in different header fields. NAV egress proofs (`crates/pfusdc_proofs`, unchanged) rely on the recomputed block id, which binds this root, and do not compare it field by field; the lane's own proof must compare it. Before the height, headers and block hashes are unchanged, and `external_lane_export_v1` is refused with `external_lane_export_root_not_active` at mempool admission, batch building and execution. An export to the lane's token address is refused with `external_lane_export_recipient_is_token`. Vectors are in `crates/types/testdata/external-lane-export-root-vectors.json`.

**The read.** `external_lane_info` reports `state.exported_atoms` and `state.next_export_nonce`, and pages the packets newest first with their digest and two commitments: `export_count`, `pruned_export_count`, `export_history_root`, `exports` and `next_export_cursor`, with `--export-cursor` and `--export-limit` as the release page's flags. The Python client takes `export_cursor` and `export_limit`; `external_lane_export_coverage` summarises a row.

**The proof input.** `postfiat-node external-lane-export-proof-input --lane-id ID (--export-nonce N | --tx-id HEX) --prior-checkpoint HEX --output-dir PATH` builds the witness of one export. Its block must be the proven block, and the prior checkpoint is the Base verifier's latest accepted checkpoint. The command writes the CBOR input of `programs/pft-lane-export` and the expected public values, after it checks them with `verify_external_lane_export_witness_v1`. It rebuilds pruned packets from the batch archive. It refuses an unknown or non-final export, a lane with exports off and a chain without the export root ([design note, step 3](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md#base-half-step-3-2026-11-10)).

**Watching exports.** The watcher's export view matches every Base payout against an export packet and checks the lock and lane totals ([the export view](#the-export-view)).

**The relay.** `scripts/pft-lane-export-relay.py` takes an export from PFTL to the Base lock. `input` builds the proof input once the block is final. `prove` runs the guest in execute mode and writes a placeholder proof, or calls an external prover. Before it sends, `release` checks every revert that it can read from the lock, the verifier and the live code. It has no pre-send check for the lock's private burn-tx record, for the SP1 proof itself, or for a race with another relay. After sending, it reports the `Released` event. `advance-checkpoint` and `status` complete the set. `scripts/rehearse-lane-roundtrip.py` runs a full round trip on a local Anvil with a mock SP1 verifier ([design note, step 4](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md#base-half-step-4-2026-11-10)).

**The holder's commands.** `external-lane-export` reads the lane with `external_lane_info` and refuses before signing, with `refused before signing: CODE: message`, when the lane does not exist (`external_lane_not_configured`), exports are off (`external_lane_export_not_supported`), the active cap is zero (`external_lane_export_halted`), the amount is zero (`external_lane_export_amount_zero`), the recipient is malformed or zero (`external_lane_export_recipient_invalid`), the lane's lock (`external_lane_export_recipient_is_lock`) or its token (`external_lane_export_recipient_is_token`), or the balance cannot cover the amount, the `asset_fee_quote` fee and the account reserve (`insufficient_funds`). Last, it refuses with the quote's `export_refusal`: `asset_fee_quote` runs execution's export rules (`dry_run_external_lane_export`, `crates/execution/src/external_lane_release.rs`) on the ledger asset admission executes the export on (`apply_pending_before_new_asset_transaction`, `crates/node/src/mempool_proposals.rs`: the pending plain transfers, `payment_v2` entries and asset transactions, in that order; the later families — atomic swaps, FastLane, escrow, NFT, offers — execute after it and are not counted), so pending releases and exports count: whether the amount fits `released_atoms - exported_atoms` is decided there alone, and `external_lane_export_above_released` (naming released and exported on that ledger), `external_lane_export_nonce_exhausted`, `external_lane_export_records_full`, `external_lane_export_root_not_active` (the headers do not carry the export root at the height the export would execute at, the value admission passes) and execution's `external_lane_export_recipient_is_token` are refused before signing. A quote without `export_checked: true` comes from a node too old to run that check and is refused (`external_lane_export_not_checked`). `--amount-atoms` takes digits only. It then signs through `wallet_sign_asset_transaction` and submits through the mempool path; `--dry-run` stops after the checks and prints the quote (the unsigned fields a signature would cover): it opens no key file, signs nothing and takes the sender's address from `--sender`, without which it is refused. When signing, `--sender` is optional and must be the key file's address. The Python helper's `dry_run=True` takes the address from the wallet and reads no key either. Admission's mempool limits are refused before signing too: the mempool full (`mempool_full`), the sender at its pending limit (`mempool_sender_pending_limit`) or in a pending atomic swap (`mempool_sender_in_pending_atomic_swap`); a quote sequence held by a pending escrow, NFT, offer or FastLane entry of the sender is refused with `bad_sequence` (send the export again once that entry is in a block). A quoted minimum fee above `--max-fee-atoms` (Python `max_fee_atoms`) is refused before signing with `external_lane_export_fee_above_ceiling`, naming both numbers. The default, 92 atoms, is a safety ceiling against a node that quotes an inflated fee: 4x the largest fee an export can be charged today, 23 atoms (`EXTERNAL_LANE_EXPORT_LARGEST_FEE_ATOMS`: one atom per started 512 bytes of the tx-id preimage, which the ML-DSA-65 key and signature dominate, and no state-expansion part). Every refusal the command can know from the node's current state happens before signing; the cases that can still come after signing, all at admission and without a fee, are these. Admission re-runs the pending escrow, NFT and offer entries after the export and refuses it with `mempool admission blocked by stale pending` when one of them no longer applies; the holder sends the export again once that entry is in a block or has left the mempool. A transaction another submitter gets admitted between the check and the submission can make admission refuse the export. And a block committed between the quote and the submission could raise the minimum fee, so that admission refuses with `fee_too_low` and the holder runs the command again; under the current fee rule this cannot happen for an export, whose minimum fee does not depend on the ledger (its state-expansion part is zero), and the sentence states what the command guarantees if the fee rule changes. A refusal the node returns at admission is printed as the node gives it (`crates/node/src/external_lane_export_cli.rs`).

```bash
postfiat-node external-lane-export --data-dir DATA_DIR --sender SENDER_ADDRESS \
  --lane-id base-erc20 --amount-atoms 400 \
  --base-recipient 0x2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c --dry-run
```

`external-lane-export-status` finds one packet by `--tx-id` or `--export-nonce` in the lane's stored export pages and prints it with its digest and commitments, `in_committed_block` (the receipt is linked from a block in this node's committed log; the log itself is not replayed), the lane's `exported_atoms`, and `base_payout`: the packet pays out on Base only once a release relay proves it to the lane's lock contract and that release is confirmed there. Stdout is that one JSON document. `--audit` replays this node's block log (`tx --audit-block-log`) and adds `block_log_verified`. A transaction the node does not find gives `in_committed_block: false`; any other read error is returned. The export nonce is the lane's counter, not the sender's: nonce 0 is the lane's first export by anyone. `--sender ADDRESS` refuses with `external_lane_export_other_sender` when the packet found belongs to another sender. A pruned packet is no longer found.

```bash
postfiat-node external-lane-export-status --data-dir DATA_DIR --lane-id base-erc20 --export-nonce 0 --audit
```

The Python client has the same pair: `external_lane_export(client, wallet=..., lane_id=..., amount_atoms=..., base_recipient=..., dry_run=False)` runs the lane checks (`external_lane_export_refusal`), quotes, refuses a quote without `export_checked: true` (`external_lane_export_not_checked`), refuses `insufficient_funds` below amount + minimum fee + account reserve and refuses the quote's `export_refusal`, all before the signer runs, then signs and submits; it refuses a quote whose kind, source or operation is not the requested export (`external_lane_export_quote_mismatch`) and a quote whose echoed amount, minimum fee, balance or reserve is malformed or out of range (`external_lane_export_quote_invalid`); `dry_run=True` returns the quote and the unsigned operation, never reads the key, signs nothing and never submits. `PostFiatRpcClient.external_lane_export_status(lane_id, tx_id_or_nonce, audit=False, sender=None)` is the status read.

**Before an operator or an exchange exports:**

- [ ] Keep the key file readable by its owner only (`chmod 600`); it is the only thing that signs.
- [ ] Run the reads, the signing and the submission against one trusted node that is synced to the chain tip; the pending-export check is that node's mempool.
- [ ] Run with `--dry-run` first and check the chain id and genesis hash it prints (`quote`) and the lane policy `external-lane-info --lane-id ID` prints (exports on, lock and token contracts) against the published ones.
- [ ] Keep the printed `tx_id`; it is the handle for the status read.
- [ ] Wait for `external-lane-export-status --tx-id ID --sender ADDRESS` to show `in_committed_block: true` (`--audit` to replay the node's block log).
- [ ] Expect no ERC-20 on Base: until a release relay is operated for the lane, nothing pays the packet out.

An export that is accepted and never paid loses nothing on PFTL beyond the exported amount, which is out of circulation and counted in the lane's `exported_atoms`; the packet stays in the lane's export history and stays provable against the committed `export_history_root` for a future relay.

## Not built yet

The user-facing interface for the lane. The Python CLI reads a lane (`external_lane_info`) and watches it ([the watcher](#the-watcher)) but does not build or submit a release; the node commands of [the Base checkpoint source](#the-base-checkpoint-source) do. The design keeps the SP1 route for Base through its dispute games (option (a)) as a later upgrade.

The Base half of exports: the PFTL finality verifier deployment for PFT on Base with its route profile and asset id, the release ELF and key of the export program (`programs/pft-lane-export`, built so far only for execute-mode tests), the relay and the Base `release` of an export. No deployed genesis enables exports; tests use the switch to qualify the native-chain implementation. Whether Phase 3 ships is the product owner's decision.

## Not before

The implementation is merged and the November 6 rehearsal configured a lane on
a separate local fleet. Production activation still requires:

1. The token document and genesis plan must fix the ERC-20 slice, native
   allocation and treatment of existing PFT Ledger payments; the decided total
   is 100 billion PFT. The [launch specification](l1v2-testnet-genesis-and-launch-spec.md)
   leaves token migration to a separate document.
2. Genesis must pin the intended lock address and runtime code hash. The
   [deployment order](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md#5-proof-on-base)
   predicts the verifier and lock addresses, establishes the first finalized
   PFTL checkpoint, then deploys the verifier and lock. The rehearsal's stand-in
   verifier does not qualify that production path.
3. The token and lock audit, production treasury/deployment steps and selected
   validator release's qualification remain open in the
   [milestone](../plans/active/pft-erc20-base-listing-milestone.md) and
   [release plan](../plans/active/next-validator-release-plan-20261115.md).

Existing network genesis files are unchanged. P2.5's remaining lock audit is
one qualification item; it is not a statement that all launch gates are closed.
