use super::*;
use postfiat_types::{
    ExternalLaneState, GenesisExternalLane, GenesisExternalLanePolicy, GENESIS_NATIVE_SUPPLY_ATOMS,
};

const LANE_RESERVED_ATOMS: u64 = 300_000_000;

fn lane_genesis(chain_id: &str, validator_count: u32) -> Genesis {
    let mut genesis = Genesis::new_with_validator_count(chain_id, validator_count);
    genesis.external_lanes = Some(vec![GenesisExternalLane {
        id: "base-erc20".to_string(),
        reserved_atoms: LANE_RESERVED_ATOMS,
        release_window_blocks: 86_400,
        release_cap_bps: 200,
        policy: GenesisExternalLanePolicy {
            chain_id: 8453,
            lock_contract_address: format!("0x{}", "1a".repeat(20)),
            token_address: format!("0x{}", "2b".repeat(20)),
            token_runtime_code_hash: format!("0x{}", "3c".repeat(32)),
            lock_runtime_code_hash: format!("0x{}", "4d".repeat(32)),
        },
    }]);
    genesis.validate().expect("valid lane genesis");
    genesis
}

fn apply_one_transfer(data_dir: &Path, to: &str) -> Receipt {
    let batch_file = data_dir.join(format!("{to}.batch.json"));
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.to_path_buf(),
        key_file: None,
        to: to.to_string(),
        amount: ACCOUNT_RESERVE + 15,
        batch_file: batch_file.clone(),
    })
    .expect("create transfer batch");
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply transfer batch");
    assert!(receipts[0].accepted, "{receipts:?}");
    receipts.into_iter().next().expect("one receipt")
}

fn released_lane(ledger: &LedgerState) -> ExternalLaneState {
    let mut lane = ledger.external_lanes[0].clone();
    lane.released_atoms = 10;
    lane.released_lock_nonces = [3_u64, 7].into_iter().collect();
    lane.window_index = 2;
    lane.window_released_atoms = 10;
    lane
}

#[test]
fn genesis_init_funds_faucet_with_supply_minus_lanes() {
    let data_dir = unique_test_dir("postfiat-external-lane-faucet");
    let genesis = lane_genesis("postfiat-lane-faucet", 1);
    init_with_genesis(data_dir.clone(), "validator-0".to_string(), genesis.clone())
        .expect("init lane genesis");
    let store = NodeStore::new(&data_dir);
    assert_eq!(store.read_genesis().expect("genesis"), genesis);
    let ledger = store.read_ledger().expect("ledger");
    let faucet = read_faucet_account_file(&data_dir.join(FAUCET_ACCOUNT_FILE)).expect("faucet");
    let expected = GENESIS_NATIVE_SUPPLY_ATOMS - LANE_RESERVED_ATOMS;
    assert_eq!(faucet.balance, expected);
    assert_eq!(ledger.accounts, vec![faucet]);
    assert_eq!(ledger.external_lanes, genesis.genesis_external_lane_states());
    assert_eq!(ledger.external_lanes[0].released_atoms, 0);
    fs::remove_dir_all(data_dir).expect("cleanup");
}

#[test]
fn native_live_total_counts_external_lane_remainder() {
    let data_dir = unique_test_dir("postfiat-external-lane-live-total");
    init_with_genesis(
        data_dir.clone(),
        "validator-0".to_string(),
        lane_genesis("postfiat-lane-live-total", 1),
    )
    .expect("init lane genesis");
    let store = NodeStore::new(&data_dir);
    let ledger = store.read_ledger().expect("ledger");
    let shielded = store.read_shielded().expect("shielded");
    let supply = native_pft_supply_breakdown(&ledger, &shielded).expect("supply");
    assert_eq!(
        supply,
        NativePftSupplyBreakdown {
            circulating_atoms: u128::from(GENESIS_NATIVE_SUPPLY_ATOMS - LANE_RESERVED_ATOMS),
            external_lane_remainder_atoms: u128::from(LANE_RESERVED_ATOMS),
            live_total_atoms: u128::from(GENESIS_NATIVE_SUPPLY_ATOMS),
        }
    );
    assert_eq!(
        native_pft_live_total(&ledger, &shielded).expect("live total"),
        u128::from(GENESIS_NATIVE_SUPPLY_ATOMS)
    );
    // Height-zero replay check with lanes configured.
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("height-zero replay counts the lane");

    // Per-block check: a fee-burning block keeps live before - burn = after.
    let receipt = apply_one_transfer(&data_dir, "pfexternallane000000000000000000000001");
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("per-block conservation counts the lane");
    let after = store.read_ledger().expect("ledger after");
    let supply_after = native_pft_supply_breakdown(&after, &shielded).expect("supply after");
    assert_eq!(
        supply_after.external_lane_remainder_atoms,
        u128::from(LANE_RESERVED_ATOMS)
    );
    assert_eq!(
        supply_after.live_total_atoms + u128::from(receipt.fee_burned),
        u128::from(GENESIS_NATIVE_SUPPLY_ATOMS)
    );

    // A released amount leaves the lane's remainder; over-release fails closed.
    let mut released = after.clone();
    released.external_lanes[0] = released_lane(&after);
    assert_eq!(
        native_pft_supply_breakdown(&released, &shielded)
            .expect("released supply")
            .external_lane_remainder_atoms,
        u128::from(LANE_RESERVED_ATOMS - 10)
    );
    released.external_lanes[0].released_atoms = LANE_RESERVED_ATOMS + 1;
    assert!(native_pft_live_total(&released, &shielded).is_err());
    let mut duplicate = after.clone();
    duplicate.external_lanes.push(after.external_lanes[0].clone());
    assert!(native_pft_live_total(&duplicate, &shielded)
        .unwrap_err()
        .to_string()
        .contains("duplicate native custody external lane"));

    // A faucet that also holds the lane's atoms is refused at height zero.
    let faucet_path = data_dir.join(FAUCET_ACCOUNT_FILE);
    let mut faucet = read_faucet_account_file(&faucet_path).expect("faucet");
    faucet.balance = GENESIS_NATIVE_SUPPLY_ATOMS;
    write_faucet_account_file(&faucet_path, &faucet).expect("write full-supply faucet");
    let error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("faucet holding the lane must fail genesis replay");
    assert!(error.to_string().contains("genesis native supply"), "{error}");
    fs::remove_dir_all(data_dir).expect("cleanup");
}

#[test]
fn checkpoint_snapshot_restore_preserves_lane_and_nonce_set() {
    let data_dir = unique_test_dir("postfiat-external-lane-checkpoint");
    init_with_genesis(
        data_dir.clone(),
        "validator-0".to_string(),
        lane_genesis("postfiat-lane-checkpoint", 3),
    )
    .expect("init lane genesis");
    let store = NodeStore::new(&data_dir);
    let genesis_lanes = store.read_ledger().expect("ledger").external_lanes;
    assert_eq!(genesis_lanes.len(), 1);
    let receipt = apply_one_transfer(&data_dir, "pfexternallane000000000000000000000002");

    // Checkpoint: prune height 1 and check the carried lane and accounting.
    let proof_file = data_dir.join("archive-handoff.json");
    create_history_archive_handoff(HistoryArchiveHandoffCreateOptions {
        data_dir: data_dir.clone(),
        from_height: 1,
        to_height: 1,
        archive_uri: Some("archive://controlled-testnet/external-lane".to_string()),
        output_file: proof_file.clone(),
        overwrite: false,
    })
    .expect("archive handoff");
    let mut prune_options = HistoryOptions::with_defaults(data_dir.clone());
    prune_options.retain_recent_blocks = 0;
    prune_options.minimum_replay_window_blocks = 0;
    prune_options.archive_handoff_file = Some(proof_file);
    let prune = history_prune(prune_options).expect("history prune");
    assert!(prune.pruned && prune.verify_after_prune.verified);
    let checkpoint = read_history_checkpoint_state_optional(&store)
        .expect("read checkpoint")
        .expect("checkpoint written");
    assert_eq!(checkpoint.ledger.external_lanes, genesis_lanes);
    assert_eq!(
        native_pft_live_total(&checkpoint.ledger, &checkpoint.shielded).expect("live")
            + checkpoint.native_fee_burn_total.expect("burns"),
        u128::from(GENESIS_NATIVE_SUPPLY_ATOMS)
    );
    assert_eq!(checkpoint.native_fee_burn_total, Some(u128::from(receipt.fee_burned)));

    // The checkpoint record carries a released lane's nonce set and window.
    let mut released_checkpoint = checkpoint.clone();
    released_checkpoint.ledger.external_lanes[0] = released_lane(&checkpoint.ledger);
    let encoded = serde_json::to_vec(&released_checkpoint).expect("encode checkpoint");
    let decoded: HistoryCheckpointState =
        serde_json::from_slice(&encoded).expect("decode checkpoint");
    assert_eq!(decoded.ledger.external_lanes, released_checkpoint.ledger.external_lanes);
    assert_eq!(
        decoded.ledger.external_lanes[0].released_lock_nonces,
        [3_u64, 7].into_iter().collect()
    );

    // The state commitment covers the nonce set.
    let commit = |ledger: &LedgerState| {
        let mut bytes = Vec::new();
        append_ledger_state(&mut bytes, ledger, true, true, false, false, true, false, false)
            .expect("ledger commitment");
        bytes
    };
    let mut other_nonce = released_checkpoint.ledger.clone();
    other_nonce.external_lanes[0].released_lock_nonces = [3_u64, 8].into_iter().collect();
    assert_ne!(commit(&released_checkpoint.ledger), commit(&other_nonce));
    assert_ne!(commit(&checkpoint.ledger), commit(&released_checkpoint.ledger));

    // Snapshot and restore carry the lane and replay from the checkpoint.
    let source_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("source status");
    let snapshot_dir = data_dir.with_file_name(format!(
        "{}-snapshot",
        data_dir.file_name().unwrap().to_string_lossy()
    ));
    let restored_dir = data_dir.with_file_name(format!(
        "{}-restored",
        data_dir.file_name().unwrap().to_string_lossy()
    ));
    let manifest = export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export snapshot");
    assert_eq!(manifest.state_root, source_status.state_root);
    let restored = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: Some("validator-restored".to_string()),
    })
    .expect("import snapshot");
    assert_eq!(restored.state_root, source_status.state_root);
    let restored_store = NodeStore::new(&restored_dir);
    assert_eq!(
        restored_store.read_ledger().expect("restored ledger").external_lanes,
        genesis_lanes
    );
    let restored_checkpoint = read_history_checkpoint_state_optional(&restored_store)
        .expect("restored checkpoint");
    if let Some(restored_checkpoint) = restored_checkpoint {
        assert_eq!(restored_checkpoint.ledger.external_lanes, genesis_lanes);
    }
    verify_blocks(NodeOptions {
        data_dir: restored_dir.clone(),
    })
    .expect("restored snapshot keeps native conservation with the lane");
    for dir in [data_dir, snapshot_dir, restored_dir] {
        let _ = fs::remove_dir_all(dir);
    }
}
