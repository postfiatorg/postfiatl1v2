# ERC-20 release path qualification run, 2026-11-03

The qualification record for milestone item P2.5 ([milestone](../../plans/active/pft-erc20-base-listing-milestone.md)): the lane tests on main, a devnet run of the release path, and the public trace of the lock contract's review. It does not tick P2.5; the open items are listed at the end.

Source: `main` at `9ae46a531`. Binary: `cargo build --release --locked -p postfiat-node` (`build_git_revision` `9ae46a53`). Everything ran on one work server: no live devnet, no Base transaction.

## Tests on main

| Crate | File | Command | Result |
|---|---|---|---|
| `postfiat-types` | `crates/types/src/external_lane_type_tests.rs` | `cargo test --locked -p postfiat-types --lib -- <its 26 test names>` | 26 passed |
| `postfiat-execution` | `crates/execution/src/external_lane_release.rs` | `cargo test --locked -p postfiat-execution --lib external_lane` | 10 passed |
| `postfiat-node` | `crates/node/src/tests/genesis_external_lane_tests.rs` | `cargo test --locked -p postfiat-node --lib external_lane` | 22 passed |

- Types (26): `genesis_without_lanes_hashes_as_before`, `ledger_state_without_lanes_serializes_as_before`, `genesis_external_lane_reserves_the_configured_amount`, `genesis_rejects_lanes_at_or_above_native_supply`, `genesis_faucet_holds_supply_minus_lanes`, `genesis_rejects_unknown_lane_fields`, `genesis_rejects_release_cap_outside_one_to_ten_thousand_bps`, `genesis_rejects_malformed_external_lanes`, `external_lane_state_rejects_release_above_reserve`, `genesis_external_lane_policy_requires_proof_policy_pins`, `genesis_external_lane_docs_example_parses_and_validates`, `deployed_devnet_genesis_bytes_and_hash_are_unchanged`, `genesis_external_lane_source_must_be_base_mainnet_or_sepolia`, `external_lane_window_cap_rounds_down`, `external_lane_release_credits_the_lane_record_once`, `external_lane_release_refuses_amount_past_window_cap_without_mutation`, `external_lane_release_refuses_past_the_record_ceiling_without_mutation`, `external_lane_release_rejects_amount_above_lane_remainder`, `external_lane_state_requires_release_fields_to_match_recorded_releases`, `external_lane_recorded_window_above_cap_is_refused`, `unreleased_external_lane_state_encodes_without_releases`, `external_lane_release_operation_validates_its_lock_key`, `genesis_rejects_two_lanes_on_one_base_source`, `external_lane_release_below_the_recorded_window_is_refused`, `external_lane_commitment_is_independent_of_release_records`, `external_lane_release_history_root_binds_every_record`.
- Execution (10): `external_lane_release_credits_recipient_once`, `external_lane_release_refuses_unconfigured_lane_and_foreign_source`, `external_lane_release_refuses_cap_and_remainder_without_mutation`, `external_lane_release_below_reserve_for_new_account_is_refused`, `external_lane_lock_proof_of_either_base_source_verifies`, `external_lane_release_rejects_wrong_chain_contract_or_token`, `external_lane_release_requires_the_attested_confirmation_distance`, `external_lane_release_rejects_checkpoint_below_quorum`, `external_lane_release_rejects_committee_the_policy_does_not_name`, `external_lane_release_rejects_claim_that_differs_from_the_lock`.
- Node (22): `genesis_init_funds_faucet_with_supply_minus_lanes`, `native_live_total_counts_external_lane_remainder`, `external_lane_state_codec_and_commitment_cover_released_nonces`, `checkpoint_snapshot_restore_preserves_unreleased_lane_records`, `checkpoint_without_lane_record_is_refused_by_checkpoint_conservation`, `snapshot_lane_tamper_is_refused_by_the_lane_rule`, `faucet_raised_for_deleted_lane_is_refused_by_height_zero_replay`, `lane_less_snapshot_with_short_faucet_is_refused_by_height_zero_replay`, `history_prune_with_checkpoint_refuses_short_faucet`, `populated_laneless_ledger_bytes_and_root_are_unchanged`, `checkpoint_with_lane_atoms_moved_to_an_account_is_refused_by_lane_set_check`, `checkpoint_with_a_lane_genesis_never_configured_is_refused`, `checkpoint_with_a_forged_release_is_refused`, `snapshot_with_a_forged_release_is_refused`, `init_refuses_genesis_with_duplicate_lane_ids`, `init_refuses_genesis_with_two_lanes_on_one_base_source`, `checkpoint_with_recorded_releases_is_accepted_and_tampering_them_is_refused`, and in `release_path`: `external_lane_release_lifecycle_on_a_local_chain`, `external_lane_info_pages_releases_newest_first`, `full_and_checkpoint_snapshots_restore_the_released_lane`, `history_checkpoint_refuses_tampered_released_lane`, `snapshot_import_refuses_tampered_released_lane`.

## Devnet run

Six loopback validators on one fresh genesis (`transport-validator-serve --unsafe-devnet-file-signer --unsafe-devnet-json-storage`), the six-validator harness pattern of the earlier mixed-fleet rehearsal; the harness is local and not in this repository. Chain `postfiat-p25-lane-devnet`, `init-consensus-v2 --validators 6 --activation-height 2`; every block went through `transport-peer-certified-batch-round` with a certificate of 5 of 6 votes.

### The lane

| Field | Value |
|---|---|
| `id` | `base-erc20` |
| `reserved_atoms` | 300,000,000 of the 1,000,000,000-atom devnet supply; the faucet starts with 700,000,000 |
| `release_window_blocks`, `release_cap_bps` | 86,400 and 200 (window cap 6,000,000 atoms), the design note's example |
| `policy.chain_id` | 84532 (Base Sepolia) |
| `policy.lock_contract_address` | `0xe36e7fc384564419187546c19719a5654266de0b`, the rehearsal lock |
| `policy.lock_runtime_code_hash` | `0x328a93e9…5442fb2`, `lock_runtime_codehash` of `crates/pft-token-contracts/deployments/pft-lock-base-84532/deployment.json` |
| `policy.token_address` | `0x18fe4ca51a0ce042688d5a54917319d03b527d92`, the Sepolia token |
| `policy.token_runtime_code_hash` | `0xb0939a24…c655903df`, `token_codehash` of the same record |
| `policy.committee_epoch`, `committee_root` | 1 and `0x71cbb891…74da260a`: the six validators' ML-DSA-65 keys, quorum 5 |
| `policy.confirmation_depth` | 64 |

Genesis hash `c25be766…5e3a86d31`, height-zero state root `f73d0c04…e3dd45bc`.

### Steps

| Step | Commands | Result |
|---|---|---|
| Lane genesis | `init-consensus-v2` on validator-0; lane written into `genesis.json` and the height-zero ledger (below); `verify-state`, `verify-blocks`; `snapshot-export`, then `snapshot-import --node-id validator-N` on validators 1–5 | Both verifiers pass at height 0; all five imports accepted; six identical state roots; `external-lane-info` shows the lane with 300,000,000 remaining |
| Height 1: committee | `fastswap-governance-bootstrap`, `governance-authorization-sign` by each of the six, `governance-amendment-assemble`, `fastswap-governance-bootstrap-assemble`; governance round | Accepted; every ledger holds committee epoch 1 (6 validators, quorum 5) |
| Height 2: release | `wallet-sign-asset-transaction` (faucet, fee 1,000) with an `external_lane_release_v1` operation, nonce 1, 1,000,000 atoms to the fresh account `pfda42e6…03e1975b`; `mempool-submit-signed-asset-transaction`, `mempool-batch`; transparent round | Receipt accepted, fee burned 1,000; recipient balance 1,000,000 on all six; lane `released_atoms` 1,000,000, remainder 299,000,000, nonces `[1]`, window 0 released 1,000,000 of 6,000,000, one release record (Base block 1001, PFTL height 2) |
| Refusals | For each case, on all six validators: `mempool-submit-signed-asset-transaction` and `signed-asset-batch` (the batch builder's dry run) | Refused by all six at both entry points with the codes below; `external-lane-info` byte-identical before and after; every mempool empty |
| Snapshot | validator-4 stopped; `snapshot-export`; `snapshot-import --node-id validator-4` into a fresh data directory; validator key, registry and vote records copied over; restarted | Same height-2 tip and state root, byte-identical `external-lane-info`; validator-4 signed the height-3 certificate |
| Height 3 | `batch-transfer`; transparent round | Accepted; six identical tips |
| Verifiers | all validators stopped; `verify-state`, `verify-blocks`, `verify-governance`, `verify-bridge`, `verify-mempool`, `verify-shielded`, `verify-finalized-checkpoint` on each | 42 of 42 exit 0; each verifier's report is byte-identical across the six; tip `b57cbe2b…`, state root `e5a9175d…` everywhere |
| Lane read | `external-lane-info` on each validator | Byte-identical on all six (SHA-256 `3c91b7f2…`) |

| Refusal | Transaction | Code and message |
|---|---|---|
| Same Lock again | nonce 1, 1,000,000 atoms | `external_lane_nonce_already_released`: already released Lock nonce 1 |
| Above the window cap | nonce 2, 6,000,001 atoms | `external_lane_window_cap_exceeded`: window 0 has released 1000000 of its cap 6000000 |
| Unconfigured lane | lane `other-lane`, nonce 3 | `external_lane_not_configured`: genesis configures no external lane `other-lane` |

Mempool admission reports ``mempool admission rejected `<code>` ``; the dry run reports `rejected in batch dry-run at height 3: <code>`.

### What was real and what was a fixture

- Real: the `main` binary, six validators, their consensus rounds and certificates, the genesis lane's accounting, the committee registered by signed governance, the release and refusals in execution, mempool admission and batch building, the snapshot round trip and the seven verifiers. The lane's addresses and code hashes are the Base Sepolia rehearsal records.
- Fixture: the Base block and its `Locked` event. The receipt proof is a one-receipt trie built as `lock_proof` in `crates/node/src/tests/genesis_external_lane_tests.rs` builds it (block 1001, block hash `0x6f…6f`, observed head 1065), with the rehearsal lock as emitter. Nothing was read from Base Sepolia: the rehearsal lock has no transactions and no logs (Blockscout counters, 2026-11-03), and the test helpers build synthetic receipts.
- Harness-signed: the checkpoint certificate. A local helper built from `postfiat-types` and `postfiat-crypto-provider` signed it with five of the six validator keys. `ethereum-checkpoint-observe`, `ethereum-checkpoint-vote-sign` and `ethereum-receipt-proof-build` serve `pftl-uniswap` routes only.
- Harness-written: the lane genesis. No command initialises a node from a genesis with lanes (`init_with_genesis` is crate-internal), so the helper wrote the lane into `genesis.json` and the height-zero ledger with the node's file MAC, and removed the height-zero chain tip so that the node rebuilt it. `topology-consensus-v2` derives its domain from a genesis without lanes, so the topology was bound to the lane genesis hash and its `topology_id` recomputed.
- Two harness corrections: the first start stopped at `local node validator-0 does not match topology domain` (fixed by binding the topology as above), and the first release submission did not start because the pretty-printed signed transaction (342 KB with five ML-DSA votes) exceeds one 128 KiB command-line argument; the compact form (75 KB) was submitted. Neither reached the chain.

## The lock contract's review trace

- `forge test --offline` in `crates/pft-token-contracts`: 188 tests in 9 suites pass. Lock suites: `PFTLockV1Test` 24, `PFTLockV1AccountingTest` 14, `PFTLockV1InvariantTest` 1, `PFTLockV1ReleaseInvariantTest` 1, `DeployPFTLockTest` 59. Token suites: 89.
- The deploy-script checks merged in `1e2cc55f8` (anchor and binding pins, re-read after deployment; `crates/pft-token-contracts/README.md`, "Deploy the lock contract") added 22 of the 59 `DeployPFTLockTest` tests; CI also runs `test/deploy-lock-cli.sh`.
- The Base Sepolia rehearsal: `crates/pft-token-contracts/deployments/pft-lock-base-84532/deployment.json` (test mode, reverting stand-in verifier) and [the dry-run record](pft-token-dry-run-20261026.md), "Base Sepolia lock rehearsal".

## Open before genesis

- Corrected 2026-11-04: no proof program is needed. The lane's proof is the committee-signed Base checkpoint plus the receipt proof (design option (b)), so it has no SP1 guest, vkey pin or proof inventory entry; an SP1 guest for Base is the design's option (a) upgrade.
- The real Base source attestation path: checkpoint observation and votes for a lane, a lane receipt proof built from a Base block, and that source's own qualification. This run used committee-signed fixtures.
- A minimum lock amount in `PFTLockV1` at or above the PFTL account reserve (10 atoms).
- Pruning the release records behind the finalized checkpoint, keeping the root.
- A six-validator operational rehearsal of the release cap amendment round with the real CLI (`ratify-external-lane-release-cap`, signing, assembly and the governance batch) with one validator absent, since the command accepts the registry threshold of five, before P2.5 closes.
- A topology bound to a lane genesis: `topology-consensus-v2` derives its domain from a genesis without lanes (`init --genesis-file` now initialises from one).
