use super::*;
use postfiat_types::{
    ExternalLaneState, FastAssetIdV1, FastLaneReserveBalanceV1, GenesisExternalLane,
    GenesisExternalLanePolicy, SnapshotManifest, GENESIS_NATIVE_SUPPLY_ATOMS,
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
            committee_epoch: 2,
            committee_root: format!("0x{}", "5e".repeat(32)),
            confirmation_depth: 64,
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

fn sibling_dir(data_dir: &Path, suffix: &str) -> PathBuf {
    data_dir.with_file_name(format!(
        "{}-{suffix}",
        data_dir.file_name().unwrap().to_string_lossy()
    ))
}

/// Archives and prunes heights `from..=to` and returns the written checkpoint.
fn prune_through(data_dir: &Path, from: u64, to: u64) -> io::Result<HistoryCheckpointState> {
    let proof_file = data_dir.join(format!("archive-handoff-{to}.json"));
    create_history_archive_handoff(HistoryArchiveHandoffCreateOptions {
        data_dir: data_dir.to_path_buf(),
        from_height: from,
        to_height: to,
        archive_uri: Some("archive://controlled-testnet/external-lane".to_string()),
        output_file: proof_file.clone(),
        overwrite: false,
    })?;
    let mut prune_options = HistoryOptions::with_defaults(data_dir.to_path_buf());
    prune_options.retain_recent_blocks = 0;
    prune_options.minimum_replay_window_blocks = 0;
    prune_options.archive_handoff_file = Some(proof_file);
    let prune = history_prune(prune_options)?;
    assert!(prune.pruned && prune.verify_after_prune.verified);
    let store = NodeStore::new(data_dir);
    assert!(!store.transactional_storage_active()?, "checkpoint is a file");
    Ok(read_history_checkpoint_state_optional(&store)?.expect("checkpoint written"))
}

/// A lane node with one fee-burning block, pruned through height 1.
fn pruned_lane_node(label: &str) -> (PathBuf, HistoryCheckpointState, Receipt) {
    let data_dir = unique_test_dir(&format!("postfiat-external-lane-{label}"));
    init_with_genesis(
        data_dir.clone(),
        "validator-0".to_string(),
        lane_genesis(&format!("postfiat-lane-{label}"), 3),
    )
    .expect("init lane genesis");
    let receipt = apply_one_transfer(&data_dir, "pfexternallane000000000000000000000002");
    let checkpoint = prune_through(&data_dir, 1, 1).expect("history prune");
    (data_dir, checkpoint, receipt)
}

fn rehash_checkpoint(genesis: &Genesis, checkpoint: &mut HistoryCheckpointState) {
    checkpoint.checkpoint_state_root = replicated_state_root(
        genesis,
        &checkpoint.governance,
        &checkpoint.ledger,
        &checkpoint.ordered_batches,
        &checkpoint.shielded,
        &checkpoint.bridge,
    )
    .expect("checkpoint state root");
    let mut canonical = checkpoint.clone();
    canonical.checkpoint_hash.clear();
    checkpoint.checkpoint_hash = hash_hex(
        "postfiat.history_checkpoint.v2",
        &serde_json::to_vec(&canonical).expect("encode checkpoint"),
    );
}

/// Rewrites one snapshot file and its manifest entry, as a tamperer would.
fn rewrite_snapshot_file(snapshot_dir: &Path, name: &str, bytes: Vec<u8>) {
    let manifest_path = snapshot_dir.join(SNAPSHOT_MANIFEST_FILE);
    let mut manifest: SnapshotManifest =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    let entry = manifest
        .files
        .iter_mut()
        .find(|file| file.name == name)
        .expect("snapshot file listed");
    entry.hash_hex = hash_hex("postfiat.snapshot.file.v1", &bytes);
    entry.bytes = bytes.len() as u64;
    fs::write(snapshot_dir.join(name), bytes).expect("write tampered file");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");
}

#[test]
fn external_lane_state_codec_and_commitment_cover_released_nonces() {
    let (data_dir, checkpoint, _) = pruned_lane_node("codec");
    // A checkpoint record carries a released lane's nonce set and window.
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
    fs::remove_dir_all(data_dir).expect("cleanup");
}

#[test]
fn checkpoint_snapshot_restore_preserves_unreleased_lane_records() {
    let (data_dir, checkpoint, receipt) = pruned_lane_node("checkpoint");
    let store = NodeStore::new(&data_dir);
    let genesis_lanes = store.read_genesis().expect("genesis").genesis_external_lane_states();
    assert_eq!(genesis_lanes.len(), 1);
    assert!(genesis_lanes[0].released_lock_nonces.is_empty());
    assert_eq!(store.read_ledger().expect("ledger").external_lanes, genesis_lanes);
    assert_eq!(checkpoint.ledger.external_lanes, genesis_lanes);
    assert_eq!(
        native_pft_live_total(&checkpoint.ledger, &checkpoint.shielded).expect("live")
            + checkpoint.native_fee_burn_total.expect("burns"),
        u128::from(GENESIS_NATIVE_SUPPLY_ATOMS)
    );
    assert_eq!(checkpoint.native_fee_burn_total, Some(u128::from(receipt.fee_burned)));

    // Snapshot and restore carry the lane and replay from the checkpoint.
    let source_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("source status");
    let snapshot_dir = sibling_dir(&data_dir, "snapshot");
    let restored_dir = sibling_dir(&data_dir, "restored");
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
        .expect("restored checkpoint")
        .expect("snapshot carries the checkpoint");
    assert_eq!(restored_checkpoint.ledger.external_lanes, genesis_lanes);
    verify_blocks(NodeOptions {
        data_dir: restored_dir.clone(),
    })
    .expect("restored snapshot keeps native conservation with the lane");
    for dir in [data_dir, snapshot_dir, restored_dir] {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn checkpoint_without_lane_record_is_refused_by_checkpoint_conservation() {
    let (data_dir, checkpoint, _) = pruned_lane_node("tamper-checkpoint");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let checkpoint_path = data_dir.join(HISTORY_CHECKPOINT_FILE);
    let mut tampered = checkpoint.clone();
    tampered.ledger.external_lanes.clear();

    // Deleting the record alone breaks the checkpoint hash.
    write_history_checkpoint_state_file(&checkpoint_path, &tampered).expect("write");
    let error = read_history_checkpoint_state_optional(&store).unwrap_err();
    assert!(
        error.to_string().contains("history checkpoint hash mismatch"),
        "{error}"
    );

    // With root and hash recomputed, the conservation check refuses it: the
    // faucet is left as is, so live supply plus burns falls short by the lane.
    rehash_checkpoint(&genesis, &mut tampered);
    write_history_checkpoint_state_file(&checkpoint_path, &tampered).expect("write");
    let error = read_history_checkpoint_state_optional(&store).unwrap_err();
    let shortfall = native_pft_live_total(&tampered.ledger, &tampered.shielded).expect("live");
    assert_eq!(
        shortfall + u128::from(LANE_RESERVED_ATOMS) + tampered.native_fee_burn_total.unwrap(),
        u128::from(GENESIS_NATIVE_SUPPLY_ATOMS)
    );
    assert!(
        error
            .to_string()
            .contains(&format!("history checkpoint live native supply {shortfall}")),
        "{error}"
    );
    fs::remove_dir_all(data_dir).expect("cleanup");
}

#[test]
fn snapshot_without_lane_record_is_refused_by_state_root() {
    let data_dir = unique_test_dir("postfiat-external-lane-tamper-snapshot");
    init_with_genesis(
        data_dir.clone(),
        "validator-0".to_string(),
        lane_genesis("postfiat-lane-tamper-snapshot", 1),
    )
    .expect("init lane genesis");
    apply_one_transfer(&data_dir, "pfexternallane000000000000000000000003");
    let snapshot_dir = sibling_dir(&data_dir, "snapshot");
    let restored_dir = sibling_dir(&data_dir, "restored");
    export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export snapshot");
    let mut ledger: LedgerState =
        serde_json::from_slice(&fs::read(snapshot_dir.join(LEDGER_FILE)).expect("read ledger"))
            .expect("parse ledger");
    assert_eq!(ledger.external_lanes.len(), 1);
    ledger.external_lanes.clear();
    rewrite_snapshot_file(
        &snapshot_dir,
        LEDGER_FILE,
        serde_json::to_vec_pretty(&ledger).expect("encode ledger"),
    );
    let error = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: None,
    })
    .unwrap_err();
    // Import's block verification recomputes the root of the lane-less ledger
    // and refuses it against the block tip, which replay reproduces with the lane.
    let error = error.to_string();
    assert!(error.contains("snapshot block verification failed"), "{error}");
    assert!(error.contains("does not match current state root"), "{error}");
    assert!(!restored_dir.exists());
    for dir in [data_dir, snapshot_dir] {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn faucet_raised_for_deleted_lane_is_refused_by_height_zero_replay() {
    let data_dir = unique_test_dir("postfiat-external-lane-tamper-faucet");
    init_with_genesis(
        data_dir.clone(),
        "validator-0".to_string(),
        lane_genesis("postfiat-lane-tamper-faucet", 1),
    )
    .expect("init lane genesis");
    apply_one_transfer(&data_dir, "pfexternallane000000000000000000000004");
    let store = NodeStore::new(&data_dir);
    let mut ledger = store.read_ledger().expect("ledger");
    ledger.external_lanes.clear();
    store.write_ledger(&ledger).expect("write ledger without lane");
    let faucet_path = data_dir.join(FAUCET_ACCOUNT_FILE);
    let mut faucet = read_faucet_account_file(&faucet_path).expect("faucet");
    faucet.balance += LANE_RESERVED_ATOMS;
    assert_eq!(faucet.balance, GENESIS_NATIVE_SUPPLY_ATOMS);
    write_faucet_account_file(&faucet_path, &faucet).expect("faucet within the bound");
    // The height-zero replay's exact faucet check (`read_genesis_faucet_account_file`)
    // fires before any block replays or any root is compared.
    let error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .unwrap_err();
    assert!(
        error.to_string().contains(&format!(
            "genesis faucet balance {GENESIS_NATIVE_SUPPLY_ATOMS} does not equal genesis native supply"
        )),
        "{error}"
    );
    fs::remove_dir_all(data_dir).expect("cleanup");
}

#[test]
fn lane_less_snapshot_with_short_faucet_is_refused_by_height_zero_replay() {
    let data_dir = unique_test_dir("postfiat-laneless-short-faucet");
    init_with_genesis(
        data_dir.clone(),
        "validator-0".to_string(),
        Genesis::new_with_validator_count("postfiat-laneless-short-faucet", 1),
    )
    .expect("init lane-less genesis");
    apply_one_transfer(&data_dir, "pfexternallane000000000000000000000005");
    let snapshot_dir = sibling_dir(&data_dir, "snapshot");
    let restored_dir = sibling_dir(&data_dir, "restored");
    export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export snapshot");
    let mut faucet = read_faucet_account_file(&snapshot_dir.join(FAUCET_ACCOUNT_FILE))
        .expect("snapshot faucet");
    faucet.balance -= 1;
    // A short faucet passes the genesis-free bound of `validate_faucet_account`.
    validate_faucet_account(&faucet).expect("short faucet within the bound");
    rewrite_snapshot_file(
        &snapshot_dir,
        FAUCET_ACCOUNT_FILE,
        serde_json::to_vec_pretty(&faucet).expect("encode faucet"),
    );
    // Refused by the height-zero replay of `verify_replayed_blocks`: its exact
    // faucet check (`read_genesis_faucet_account_file`) compares the faucet
    // with the genesis supply.
    let error = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: None,
    })
    .unwrap_err();
    assert!(
        error.to_string().contains(&format!(
            "genesis faucet balance {} does not equal genesis native supply",
            GENESIS_NATIVE_SUPPLY_ATOMS - 1
        )),
        "{error}"
    );
    for dir in [data_dir, snapshot_dir] {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn history_prune_with_checkpoint_refuses_short_faucet() {
    let (data_dir, _, _) = pruned_lane_node("prune-short-faucet");
    apply_one_transfer(&data_dir, "pfexternallane000000000000000000000006");
    let faucet_path = data_dir.join(FAUCET_ACCOUNT_FILE);
    let mut faucet = read_faucet_account_file(&faucet_path).expect("faucet");
    faucet.balance -= 1;
    write_faucet_account_file(&faucet_path, &faucet).expect("faucet within the bound");
    // With a checkpoint the prune never rebuilds the genesis ledger, so only
    // its exact faucet read refuses the short faucet.
    let error = prune_through(&data_dir, 2, 2).unwrap_err();
    assert!(
        error.to_string().contains(&format!(
            "genesis faucet balance {} does not equal genesis native supply",
            GENESIS_NATIVE_SUPPLY_ATOMS - LANE_RESERVED_ATOMS - 1
        )),
        "{error}"
    );
    fs::remove_dir_all(data_dir).expect("cleanup");
}

/// A populated lane-less ledger from the fixture of
/// `replicated_state_root_commits_every_fastlane_ledger_field`: accounts, an
/// asset definition and a FastLane reserve.
fn populated_laneless_ledger(genesis: &Genesis) -> LedgerState {
    let asset = AssetDefinition::new(&genesis.chain_id, "pfissuer0", "ROOTA", 1, 0)
        .expect("root asset A");
    let asset_id = FastAssetIdV1(
        hex_to_bytes(&asset.asset_id)
            .expect("asset id hex")
            .try_into()
            .expect("asset id length"),
    );
    let mut ledger = LedgerState::new(vec![
        Account::new(
            "pfexternallanefaucet000000000000000001",
            GENESIS_NATIVE_SUPPLY_ATOMS - 1_000,
            Some("ab".repeat(32)),
        ),
        Account::new("pfexternallaneholder000000000000000002", 1_000, None),
    ]);
    ledger.asset_definitions = vec![asset];
    ledger.fast_lane_reserves.push(FastLaneReserveBalanceV1 {
        asset_id,
        amount_atoms: 7,
    });
    ledger
}

#[test]
fn populated_laneless_ledger_bytes_and_root_are_unchanged() {
    let genesis = Genesis::new("postfiat-fastlane-root-test");
    let ledger = populated_laneless_ledger(&genesis);
    let json = serde_json::to_vec(&ledger).expect("ledger json");
    assert!(!String::from_utf8_lossy(&json).contains("external_lanes"));
    let root = replicated_state_root(
        &genesis,
        &GovernanceState::new(4),
        &ledger,
        &Vec::<String>::new(),
        &ShieldedState::empty(),
        &BridgeState::empty(),
    )
    .expect("state root");
    // Frozen at base commit 417c0d51 (before genesis external lanes existed)
    // by running this fixture in a scratch worktree of that commit.
    let json_hash: String = <sha3::Sha3_384 as sha3::Digest>::digest(&json)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(json.len(), 817);
    assert_eq!(
        json_hash,
        "00b223ef12d3685374dabac985d346c25a288c2318cd293be5e3af7636f7332147ed8fd644187d5416ab7dea8c3c81c9"
    );
    assert_eq!(
        root,
        "c1edff6b30f85947c014054c19fb1bf91a2304dd22a17000cee0bd9ead6edf7a4a1ac2c37be163d6e6bc4794c0a3bc54"
    );
}
