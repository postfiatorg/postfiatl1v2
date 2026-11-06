# ERC-20 release path qualification run, 2026-11-03

The qualification record for milestone item P2.5 ([milestone](../../plans/active/pft-erc20-base-listing-milestone.md)): the lane tests on main, a devnet run of the release path, and the public trace of the lock contract's review. It does not tick P2.5; the open items are listed at the end. The amendment rounds were rehearsed on 2026-11-04 ([below](#six-validator-rehearsal-of-the-amendment-rounds-2026-11-04)).

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
| `policy.lock_runtime_code_hash` | `0x328a93e9…5442fb2`, `lock_runtime_codehash` of `crates/pft-token-contracts/deployments/pft-lock-base-84532/archived-0xE36e7Fc384564419187546c19719A5654266dE0B.json` (archived 2026-11-06) |
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
- The Base Sepolia rehearsal: `crates/pft-token-contracts/deployments/pft-lock-base-84532/archived-0xE36e7Fc384564419187546c19719A5654266dE0B.json` (archived 2026-11-06; test mode, reverting stand-in verifier) and [the dry-run record](pft-token-dry-run-20261026.md), "Base Sepolia lock rehearsal".

## Six-validator rehearsal of the amendment rounds, 2026-11-04

Both lane amendment kinds through their real CLI rounds at five of six, on a local fleet. Source: `main` at `7527df06a` (`build_git_revision` `7527df06`), `cargo build --release --locked -p postfiat-node`. One work server, loopback only, local keys: no live network, no Base transaction.

**Fleet.** Chain `postfiat-p25-amendment-devnet`, six validators served by `transport-validator-serve --unsafe-devnet-file-signer --unsafe-devnet-json-storage`. The genesis is the `init-consensus-v2 --validators 6 --activation-height 2` genesis plus the lane of the 2026-11-03 run, with `release_window_blocks` 1 so seven windows are seven blocks (`release_cap_bps` 200, window cap 6,000,000 atoms; committee epoch 1, root `0x7b36042f…947a6f7d`, the six validators' keys, quorum 5). validator-0 ran `init --genesis-file` on it with the six validator keys in place; validators 1–5 ran `snapshot-import`. Genesis hash `cfb1026d…1ed9df69`, height-zero state root `e73fbae2…4a600c81af`; `verify-state` and `verify-blocks` pass at height 0. Every block went through `transport-peer-certified-batch-round` with 5 of 6 votes, and all six reported the same tip and state root after each round.

| Height | Round | Commands | Receipt and lane read |
|---|---|---|---|
| 1 | Committee | `fastswap-governance-bootstrap`, six `governance-authorization-sign`, `governance-amendment-assemble`, `fastswap-governance-bootstrap-assemble` | `accepted`, committee epoch 1 (6 validators, quorum 5) on all six |
| 2 | Release | `wallet-sign-asset-transaction` with `external_lane_release_v1` (nonce 1, 1,000,000), `mempool-submit-signed-asset-transaction`, `mempool-batch` | `accepted`, `released_atoms` 1,000,000 |
| 3 | Cap 200 → 100 bps | `ratify-external-lane-release-cap --lane-id base-erc20 --cap-bps 100 --activation-height 3 --validators <all six> --support <five>`, five `governance-authorization-sign`, `governance-amendment-assemble`, `governance-batch` | `governance amendment applied` (`eeac1e2b…`); quorum 5, support 5, 6 declared, 5 signatures, validator-4 absent; `release_cap`: `active_cap_bps` 100, effective 3 |
| 3 → 4 | Above the new cap | nonce 2, 3,000,001 atoms, on all six | refused (below) |
| 4 | Below it | nonce 2, 2,000,000 atoms | `accepted`, `released_atoms` 3,000,000 |
| 5 | Halt, cap 0 | as at height 3, validator-0 absent | `governance amendment applied` (`f7024e4b…`); `active_cap_bps` 0 |
| 5 → 6 | Release while halted | nonce 3, 1,000,000 atoms, on all six | refused: `window 6 has released 0 of its cap 0` |
| 6 | Raise to 100 bps | as at height 3, validator-1 absent | `governance amendment applied` (`dca91e52…`); `active_cap_bps` 0, `pending` cap 100 at `effective_height` 13 (6 + 7 × 1) |
| 6 → 7 | Release in the delay | nonce 3, 1,000,000 atoms, on all six | refused: `window 7 has released 0 of its cap 0` |
| 7–13 | Filler | `batch-transfer` | at height 13 `active_cap_bps` 100, `pending` null; at tip 12 every validator admitted the nonce-3 release for height 13 |
| 14 | Release after the delay | nonce 3, 1,000,000 atoms | `accepted`, `released_atoms` 4,000,000 |
| 15 | Final checkpoint | epoch 1's drained final FastLane checkpoint, 5 of 6 votes, as `AnchorCheckpoint`; `rpc --method mempool_submit_fastlane_primary`, `mempool-batch` | `fastlane_checkpoint_anchored` |
| 16 | Epoch 2 registered | `fastswap-control-prepare --kind activate-committee --epoch 2`, five `fastswap-control-vote-sign` (validator-5 absent), `fastswap-control-assemble`; the certificate as a `control` primary transaction through the same RPC | `fastlane_control_applied`; committees 1 and 2 registered; the lane still names epoch 1 |
| 17 | Committee move | `external-lane-committee-binding` (binding `6a43ccaa…473e8f52`), `ratify-external-lane-committee-epoch --committee-epoch 2 --committee-root 0x7b36042f… --activation-height 17` with five supporters, five signatures (validator-0 absent), `governance-amendment-assemble`, `governance-batch` | `governance amendment applied` (`18f9308a…`); `committee`: epoch 2, `effective_height` 18 |
| 17 → 18 | Epoch-1 checkpoint | nonce 4, 1,000,000 atoms, Base block 1004, on all six | refused (below) |
| 18 | Same Lock, epoch 2 | nonce 4, same Base block and hash, checkpoint `authority_epoch` 2 | `accepted`, nonces `[1, 2, 3, 4]`, `released_atoms` 5,000,000 |

Refusals, each on all six validators at mempool admission and in the batch builder's dry run (`signed-asset-batch`):

| Case | Code and message |
|---|---|
| Above the new cap | ``mempool admission rejected `external_lane_window_cap_exceeded`: external lane `base-erc20` window 4 has released 0 of its cap 3000000; 3000001 more would exceed it`` |
| Epoch-1 checkpoint after the move | ``mempool admission rejected `external_lane_committee_inactive`: Base checkpoint of committee epoch 1 is not from the committee epoch 2 external lane `base-erc20` names at this height``; dry run: `rejected in batch dry-run at height 18: external_lane_committee_inactive` |
| Cap ratify with four supporters | `ratify-external-lane-release-cap failed: insufficient support: got 4, need 5`; no amendment file written |
| Move ratify with four supporters | `ratify-external-lane-committee-epoch failed: insufficient support: got 4, need 5` |
| A second move to epoch 2 | ``ratify-external-lane-committee-epoch failed: external_lane_committee_epoch_not_advanced: external lane `base-erc20` already names committee epoch 2; a move must advance it`` |
| Quorum 1, one supporter | `governance-authorization-sign failed: external_lane_release_cap_authority_invalid: release cap amendment needs quorum and support of at least the registry threshold 5 of 6`. The batch was then assembled by hand (a valid five-of-six batch with its amendment replaced by the quorum-1 one, unsigned since no signer would sign it) and proposed for height 15: `peer certified batch round proposal failed: external_lane_release_cap_authority_invalid: release cap amendment needs quorum and support of at least the registry threshold 5 of 6`; no block, tip unchanged |

**End state.** All six stopped at height 18, tip `5e7e155d…61f5c332b5`, state root `e1794693…2c36a987`. `verify-state`, `verify-blocks`, `verify-governance`, `verify-bridge`, `verify-mempool`, `verify-shielded` and `verify-finalized-checkpoint`: 42 of 42 exit 0, each report byte-identical across the six. `external-lane-info` byte-identical on all six (SHA-256 `f9420674…45b7292d`): remainder 295,000,000, cap 100 bps from height 13, committee epoch 2 from height 18, 4 releases of which 3 pruned behind the finalized height.

**Real and harness.** Real: the binary, the fleet and its certified rounds, every node command named above, admission, the batch dry runs, execution and the verifiers. Harness (local, not in this repository, the 2026-11-03 one extended): the lane added to the genesis body before `init --genesis-file`; the topology bound to the lane genesis hash; the Base checkpoint certificates and receipt proofs (fixture block, signed with five validator keys under epoch 1 or epoch 2); and the drained final FastLane checkpoint and its five votes. Not as the docs say, or not covered by a command:

- `topology-consensus-v2` still writes the lane-less genesis hash (`e9264ea4…` against `cfb1026d…`; `write_consensus_v2_topology`, `crates/node/src/lifecycle_queries.rs:624`), so the harness rebinds it.
- No command builds or signs the drained final FastLane checkpoint that `fastswap-control-prepare --kind activate-committee` requires (`crates/node/src/fastswap_control_signing.rs:183`), and `fastswap-control-assemble` writes a certificate, not the primary transaction; both went in through `rpc --method mempool_submit_fastlane_primary` (`crates/node/src/rpc_dispatch.rs:2042`).
- Epoch 2 is the registry's committee (`registry_committee`, `crates/node/src/fastswap_control_signing.rs:252`): with no key rotation it has epoch 1's keys and the same root, which does not cover the epoch. The move changed only the epoch, and the old checkpoint was refused by its epoch alone; a new root needs a registry key rotation first.
- `governance-authorization-sign` refuses an under-threshold lane amendment. The architecture page lists ratification, signed-batch admission and execution only.
- A harness slip, not the chain's: the release probed at tip 12 was admitted on all six, then a filler used the faucet sequence, and admission was blocked by ``mempool admission blocked by stale pending `3ff77890…`: expected sequence 10, got 9`` until `mempool-batch` dropped the stale entry on each validator.

## Base checkpoint source (2026-11-05)

The committee's Base observer and the lane's receipt proof, merged in `08d6e03f6` ([the page](../../architecture/genesis-external-lanes.md#the-base-checkpoint-source), `crates/node/src/external_lane_checkpoint_source.rs`). Four node commands:

- `external-lane-checkpoint-observe` writes the lane checkpoint of the Base block the member's node reports as `finalized`, bound to the lane's genesis policy and its active committee.
- `external-lane-checkpoint-sign` re-observes the same block from the member's own Base RPC and signs one vote with its committee key, never two for one block.
- `external-lane-checkpoint-assemble` writes the certificate of exactly the committee quorum (5 of 6) from distinct members' votes on that checkpoint.
- `external-lane-lock-proof` rebuilds the Lock block's receipt trie to its receipts root and writes the `external_lane_release_v1` operation of the receipt's `Locked` event.

**Tests.** `cargo test --locked -p postfiat-node --lib checkpoint_source` on the merge: 16 passed, 1 ignored (`crates/node/src/tests/external_lane_checkpoint_source_tests.rs`, against a mock Base JSON-RPC): `observe_reads_the_finalized_block_and_names_the_active_committee`, `observe_refuses_chain_finality_depth_code_and_lane_mismatches`, `sign_re_observes_refuses_non_members_and_never_signs_two_checkpoints_of_one_block`, `sign_refuses_a_corrupt_or_foreign_signing_state_as_a_typed_refusal`, `sign_refuses_a_key_file_whose_private_key_is_not_its_public_key`, `sign_refuses_a_key_file_without_the_members_record_as_a_key_mismatch`, `assemble_keeps_exactly_the_quorum_of_distinct_member_votes`, `observe_and_sign_follow_a_governed_committee_move`, `observe_sign_assemble_and_proof_refuse_a_lane_whose_committee_is_not_registered`, `lock_proof_refuses_another_block_contract_event_or_token`, `lock_proof_refuses_two_locked_logs_a_mismatched_recipient_hash_or_a_forged_certificate`, `lock_proof_refuses_an_invalid_submitter_or_recipient_as_a_typed_refusal`, `lock_proof_accepts_an_unrelated_same_shaped_transfer_beside_the_policy_token_transfer`, `lock_proof_decodes_nonce_amount_and_recipient_at_their_boundaries`, `the_node_selects_locked_logs_by_the_bridge_decoders_topic` and `checkpoint_source_releases_a_base_lock_end_to_end_at_the_five_of_six_quorum` (observe, five votes, certificate, proof, then a six-validator lane chain releases the Lock once and refuses it again). The ignored one is the live run below.

**Live Base Sepolia run.** `live_base_sepolia_checkpoint_of_the_rehearsal_lock`, once, 2026-11-05 10:58:53–10:59:12Z, on the branch tip `4da5f4d61` (merged unchanged as the second parent of `08d6e03f6`), against `https://sepolia.base.org` through a local forwarder that passed only read methods. The lane policy named chain 84532, the rehearsal lock `0xE36e7Fc384564419187546c19719A5654266dE0B` and the token `0x18fE4ca51A0cE042688D5A54917319d03B527D92` with their runtime code hashes, confirmation depth 64, and a local six-member committee (epoch 1, quorum 5).

| Fact | Value |
|---|---|
| Finalized block observed | 54,766,144 |
| Block hash | `0x2611dc7efc546bdf2b3f4468e504ebcbc64fdfe1348534cb4bea56a098a08cc1` |
| Head at observation | 54,772,391 (6,247 blocks past the block) |
| Code hashes at the block | both equal to the policy's pins |
| Votes | five, by `validator-0` to `validator-4`, each after its own re-observation of the block |
| Certificate | 5 votes, the quorum of 6, verified by `verify_ethereum_checkpoint_certificate` before it was written |
| Receipt trie | the block's 3 receipts, the first an OP Stack deposit receipt, rebuilt to the block's `receiptsRoot` for its first and last transaction |

Requests forwarded: `eth_chainId` 8, `eth_blockNumber` 6, `eth_getBlockByNumber` 12, `eth_getCode` 12, `eth_getTransactionReceipt` 2, `eth_getBlockByHash` 2, `eth_getBlockReceipts` 2; no other method, and no transaction of any kind.

**Limit.** No Lock exists on the rehearsal lock yet, so `external-lane-lock-proof` has run only against the mock RPC. A real Lock needs Base Sepolia test ETH on the deployer, and it is the next rehearsal.

## The watcher (2026-11-05)

`python/postfiat_rpc/external_lane_watcher.py`, merged in `41bde8b8a`, matches each release on PFTL against its Lock on Base and raises a typed alarm on a release that no Lock backs ([the watcher](../../architecture/genesis-external-lanes.md#the-watcher), design section 5.2).

**Tests.** `python/tests/test_external_lane_watcher.py`, against mocked PFTL and Base RPCs: 32 passed, 45 subtests. With the CLI method and documented-command tests (`test_cli_public_reads.py`, `test_cli_doc_examples.py`): 41 passed, 104 subtests (`PYTHONPATH=python python3 -m pytest -q`).

**Live check.** Once, 2026-11-05 13:02:12Z, read-only: a local `rpc-serve` with no finality flags on `init --genesis-file crates/node/testdata/external-lane-genesis/genesis.json`, which names the Base Sepolia rehearsal lock `0xE36e7Fc384564419187546c19719A5654266dE0B` and its runtime code hash, and the watcher with `--lane-id base-erc20 --base-rpc https://sepolia.base.org --once`. The node binary was built from the branch, whose node code is `e7749f414`.

| Fact | Value |
|---|---|
| Exit | 0, one `summary` line, no alarm |
| Releases checked | 0 (`release_count` 0: the lane has no release) |
| PFTL height | 0 |
| Base chain id | 84532 |
| Finalized block | 54,803,494 |
| Lock code hash at that block | `0x328a93e99cf9659bdb44807d60a9e0328e0ff867d0b584dc2eeed3b0d5442fb2`, equal to the genesis pin |

It shows that the watcher reads a real node's `external_lane_info` (chain id, genesis hash, the lane row and its policy) and, over Base's real JSON-RPC, the finalized block and the lock contract's code. It sent only read methods. Matching a release against `lockRecord` has run only against the mocked RPC: no Lock and no release exist yet.

## Open before genesis

- Corrected 2026-11-04: no proof program is needed. The lane's proof is the committee-signed Base checkpoint plus the receipt proof (design option (b)), so it has no SP1 guest, vkey pin or proof inventory entry; an SP1 guest for Base is the design's option (a) upgrade.
- [x] Done 2026-11-05 ([the Base checkpoint source](#base-checkpoint-source-2026-11-05)): the real Base source attestation path, checkpoint observation and votes for a lane and a lane receipt proof built from a Base block, with its own qualification. A real Lock's proof waits for the next rehearsal.
- A minimum lock amount in `PFTLockV1` at or above the PFTL account reserve (10 atoms).
- Pruning the release records behind the finalized checkpoint, keeping the root.
- [x] Done 2026-11-04, for both kinds ([the rehearsal](#six-validator-rehearsal-of-the-amendment-rounds-2026-11-04)): a six-validator operational rehearsal of the release cap amendment round with the real CLI (`ratify-external-lane-release-cap`, signing, assembly and the governance batch) with one validator absent, since the command accepts the registry threshold of five, before P2.5 closes.
- A topology bound to a lane genesis: `topology-consensus-v2` derives its domain from a genesis without lanes (`init --genesis-file` now initialises from one).
