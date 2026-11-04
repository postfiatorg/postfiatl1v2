use super::*;
use postfiat_types::{
    ExternalLaneReleaseRecord, ExternalLaneState, FastAssetIdV1, FastLaneReserveBalanceV1, GenesisExternalLane,
    GenesisExternalLanePolicy, SnapshotManifest, GENESIS_NATIVE_SUPPLY_ATOMS,
};

const LANE_RESERVED_ATOMS: u64 = 300_000_000;

pub(super) fn lane_genesis(chain_id: &str, validator_count: u32) -> Genesis {
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
            committee_root: format!("0x{}", "5e".repeat(48)),
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

fn lane_release_record(nonce: u64, amount_atoms: u64, height: u64) -> ExternalLaneReleaseRecord {
    ExternalLaneReleaseRecord {
        lock_nonce: nonce,
        source_chain_id: 8453,
        lock_contract_address: format!("0x{}", "1a".repeat(20)),
        amount_atoms,
        recipient: "pftl-recipient".to_string(),
        source_block_number: 77,
        source_block_hash: format!("0x{}", "6f".repeat(32)),
        released_at_height: height,
    }
}

/// The lane after two recorded releases (nonces 3 and 7, 10 atoms) in window 2.
fn released_lane(ledger: &LedgerState) -> ExternalLaneState {
    let genesis = lane_genesis("postfiat-lane-released", 1);
    let genesis_lane = &genesis.external_lanes()[0];
    let mut lane = ledger.external_lanes[0].clone();
    for (nonce, amount) in [(3_u64, 4_u64), (7, 6)] {
        lane.apply_release(genesis_lane, lane_release_record(nonce, amount, 2 * 86_400 + nonce))
            .expect("recorded release");
    }
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

/// Recomputes the root and hash as a tamperer would, so only the lane rule
/// can refuse the tampered record.
fn rehash_checkpoint(genesis: &Genesis, checkpoint: &mut HistoryCheckpointState) {
    checkpoint.checkpoint_state_root = replicated_state_root(
        genesis,
        &checkpoint.governance,
        &checkpoint.ledger,
        &checkpoint.ordered_batches,
        &checkpoint.shielded,
        &checkpoint.bridge,
    )
    .expect("a tampered lane record still has a state root");
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
    other_nonce.external_lanes[0].releases[1].lock_nonce = 8;
    assert_ne!(commit(&released_checkpoint.ledger), commit(&other_nonce));
    assert_ne!(commit(&checkpoint.ledger), commit(&released_checkpoint.ledger));

    // The lane commitment is not gated by `commit_complete_nav_state`: the
    // fixture carries no NAV state, so the lane-bearing bytes are the same
    // with and without the gate, and still differ from the lane-less ledger's.
    let commit_without_nav = |ledger: &LedgerState| {
        let mut bytes = Vec::new();
        append_ledger_state(&mut bytes, ledger, false, true, false, false, true, false, false)
            .expect("ledger commitment without complete NAV state");
        bytes
    };
    let mut laneless = checkpoint.ledger.clone();
    laneless.external_lanes.clear();
    assert_eq!(commit_without_nav(&checkpoint.ledger), commit(&checkpoint.ledger));
    assert_ne!(commit_without_nav(&checkpoint.ledger), commit_without_nav(&laneless));
    assert_ne!(commit(&checkpoint.ledger), commit(&laneless));
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
fn snapshot_lane_tamper_is_refused_by_the_lane_rule() {
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
    let ledger: LedgerState =
        serde_json::from_slice(&fs::read(snapshot_dir.join(LEDGER_FILE)).expect("read ledger"))
            .expect("parse ledger");
    assert_eq!(ledger.external_lanes.len(), 1);
    let import_with = |edit: &dyn Fn(&mut LedgerState)| {
        let mut tampered = ledger.clone();
        edit(&mut tampered);
        rewrite_snapshot_file(
            &snapshot_dir,
            LEDGER_FILE,
            serde_json::to_vec_pretty(&tampered).expect("encode ledger"),
        );
        let error = import_snapshot(SnapshotImportOptions {
            data_dir: restored_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: None,
        })
        .unwrap_err()
        .to_string();
        assert!(!restored_dir.exists());
        error
    };

    // A dropped lane record is refused by the lane-set rule on import.
    let error = import_with(&|ledger| ledger.external_lanes.clear());
    assert!(
        error.contains("snapshot ledger is missing genesis external lane `base-erc20`"),
        "{error}"
    );

    // A changed live lane field is refused by the same rule: the window
    // fields must follow from the recorded releases, and there are none.
    let error = import_with(&|ledger| ledger.external_lanes[0].window_index += 1);
    assert!(
        error.contains("snapshot ledger external lane `base-erc20` has window 1 with 0 atoms released, but its recorded releases give window 0 with 0"),
        "{error}"
    );
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

/// Rewrites the persisted checkpoint with `edit` applied to its ledger and
/// its root and hash recomputed, then reads it back through validation.
fn reread_tampered_checkpoint(
    data_dir: &Path,
    checkpoint: &HistoryCheckpointState,
    edit: impl FnOnce(&mut LedgerState),
) -> (io::Result<Option<HistoryCheckpointState>>, HistoryCheckpointState) {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let mut tampered = checkpoint.clone();
    edit(&mut tampered.ledger);
    rehash_checkpoint(&genesis, &mut tampered);
    write_history_checkpoint_state_file(&data_dir.join(HISTORY_CHECKPOINT_FILE), &tampered)
        .expect("write tampered checkpoint");
    (read_history_checkpoint_state_optional(&store), tampered)
}

fn assert_aggregate_balances(checkpoint: &HistoryCheckpointState) {
    assert_eq!(
        native_pft_live_total(&checkpoint.ledger, &checkpoint.shielded).expect("live")
            + checkpoint.native_fee_burn_total.expect("burns"),
        u128::from(GENESIS_NATIVE_SUPPLY_ATOMS),
        "the tamper keeps live + burns = supply, so only the lane-set rule can refuse it"
    );
}

#[test]
fn checkpoint_with_lane_atoms_moved_to_an_account_is_refused_by_lane_set_check() {
    let (data_dir, checkpoint, _) = pruned_lane_node("lane-moved");
    let (result, tampered) = reread_tampered_checkpoint(&data_dir, &checkpoint, |ledger| {
        ledger.external_lanes.clear();
        ledger.accounts[0].balance += LANE_RESERVED_ATOMS;
    });
    assert_aggregate_balances(&tampered);
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("history checkpoint is missing genesis external lane `base-erc20`"),
        "{error}"
    );
    fs::remove_dir_all(data_dir).expect("cleanup");
}

#[test]
fn checkpoint_with_a_lane_genesis_never_configured_is_refused() {
    let (data_dir, checkpoint, _) = pruned_lane_node("lane-unknown");
    let (result, tampered) = reread_tampered_checkpoint(&data_dir, &checkpoint, |ledger| {
        let mut rogue = ledger.external_lanes[0].clone();
        rogue.lane_id = "rogue-lane".to_string();
        rogue.reserved_atoms = 5;
        ledger.external_lanes.push(rogue);
        ledger.accounts[0].balance -= 5;
    });
    assert_aggregate_balances(&tampered);
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("history checkpoint carries external lane `rogue-lane`, which genesis never configured"),
        "{error}"
    );

    // A changed reserve on a configured lane is refused the same way.
    let (result, tampered) = reread_tampered_checkpoint(&data_dir, &checkpoint, |ledger| {
        ledger.external_lanes[0].reserved_atoms += 5;
        ledger.accounts[0].balance -= 5;
    });
    assert_aggregate_balances(&tampered);
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains(&format!(
            "history checkpoint external lane `base-erc20` reserves {} atoms, but genesis reserves {LANE_RESERVED_ATOMS}",
            LANE_RESERVED_ATOMS + 5
        )),
        "{error}"
    );
    fs::remove_dir_all(data_dir).expect("cleanup");
}

/// Lane atoms released to an account without a recorded release: lane id
/// and reserve kept, released fields forged, the account credited by 5.
fn forge_release(ledger: &mut LedgerState) {
    let lane = &mut ledger.external_lanes[0];
    lane.released_atoms = 5;
    lane.released_lock_nonces = [99_u64].into_iter().collect();
    lane.window_released_atoms = 5;
    ledger.accounts[0].balance += 5;
}

#[test]
fn checkpoint_with_a_forged_release_is_refused() {
    let (data_dir, checkpoint, _) = pruned_lane_node("forged-release");
    let (result, tampered) = reread_tampered_checkpoint(&data_dir, &checkpoint, forge_release);
    assert_aggregate_balances(&tampered);
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains(
            "external lane `base-erc20` released Lock nonces {99} differ from its recorded releases {}"
        ),
        "{error}"
    );

    // The same state in the live ledger: replay starts from the checkpoint and refuses it.
    let store = NodeStore::new(&data_dir);
    store.write_ledger(&tampered.ledger).expect("write forged ledger");
    let error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("released Lock nonces {99} differ from its recorded releases {}"),
        "{error}"
    );
    fs::remove_dir_all(data_dir).expect("cleanup");
}

#[test]
fn snapshot_with_a_forged_release_is_refused() {
    let (data_dir, _, _) = pruned_lane_node("forged-release-snapshot");
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
    forge_release(&mut ledger);
    let shielded = NodeStore::new(&data_dir).read_shielded().expect("shielded");
    let burns = read_history_checkpoint_state_optional(&NodeStore::new(&data_dir))
        .expect("checkpoint")
        .expect("checkpoint written")
        .native_fee_burn_total
        .expect("burns");
    // The forgery keeps live + burns = supply, so only the lane rule refuses it.
    assert_eq!(
        native_pft_live_total(&ledger, &shielded).expect("live") + burns,
        u128::from(GENESIS_NATIVE_SUPPLY_ATOMS)
    );
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
    .unwrap_err()
    .to_string();
    assert!(!restored_dir.exists());
    assert!(
        error.contains(
            "snapshot ledger external lane `base-erc20` released Lock nonces {99} differ from its recorded releases {}"
        ),
        "{error}"
    );
    for dir in [data_dir, snapshot_dir] {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn init_refuses_genesis_with_duplicate_lane_ids() {
    let data_dir = unique_test_dir("postfiat-external-lane-duplicate-init");
    let mut genesis = lane_genesis("postfiat-lane-duplicate-init", 1);
    let lanes = genesis.external_lanes.as_mut().expect("lanes");
    lanes.push(lanes[0].clone());
    let error = init_with_genesis(data_dir.clone(), "validator-0".to_string(), genesis)
        .expect_err("init must validate the genesis");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{error}");
    assert!(
        error
            .to_string()
            .contains("duplicate genesis external lane id `base-erc20`"),
        "{error}"
    );
    assert!(!data_dir.exists(), "nothing is written for an invalid genesis");
}

#[test]
fn init_refuses_genesis_with_two_lanes_on_one_base_source() {
    let data_dir = unique_test_dir("postfiat-external-lane-shared-source-init");
    let mut genesis = lane_genesis("postfiat-lane-shared-source-init", 1);
    let lanes = genesis.external_lanes.as_mut().expect("lanes");
    let mut second = lanes[0].clone();
    second.id = "base-erc20-b".to_string();
    lanes.push(second);
    let error = init_with_genesis(data_dir.clone(), "validator-0".to_string(), genesis)
        .expect_err("init must validate the genesis");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{error}");
    assert!(
        error.to_string().contains(
            "genesis external lanes `base-erc20` and `base-erc20-b` both name Base source 8453"
        ),
        "{error}"
    );
    assert!(!data_dir.exists(), "nothing is written for an invalid genesis");
}

const EXAMPLE_LANE_GENESIS: &str = include_str!("../../testdata/external-lane-genesis/genesis.json");
const EXAMPLE_LANE_GENESIS_HASH: &str =
    "03f36eb201cca42c932b04161ea058e3821fd1cb0948a775a2e4431d8cfd5960bf59c70ec3105f7e7dff502d1b1109b8";

/// `lane_genesis` with the Base Sepolia rehearsal lock and token as its source,
/// as `crates/pft-token-contracts/deployments/pft-lock-base-84532` records them.
fn sepolia_rehearsal_lane_genesis() -> Genesis {
    let mut genesis = lane_genesis("postfiat-lane-sepolia-example", 1);
    let policy = &mut genesis.external_lanes.as_mut().expect("lanes")[0].policy;
    policy.chain_id = 84_532;
    policy.lock_contract_address = "0xe36e7fc384564419187546c19719a5654266de0b".to_string();
    policy.token_address = "0x18fe4ca51a0ce042688d5a54917319d03b527d92".to_string();
    policy.lock_runtime_code_hash =
        "0x328a93e99cf9659bdb44807d60a9e0328e0ff867d0b584dc2eeed3b0d5442fb2".to_string();
    policy.token_runtime_code_hash =
        "0xb0939a2407cd571d158498ba0f405f4ddb3857e688d2bc5ada170dcc655903df".to_string();
    genesis.validate().expect("valid rehearsal lane genesis");
    genesis
}

#[test]
fn init_from_lane_genesis_file_matches_the_in_code_lane_genesis() {
    let expected = sepolia_rehearsal_lane_genesis();
    // The example file is canonical, pinned by hash, and names the deployment records.
    let genesis = Genesis::from_json(EXAMPLE_LANE_GENESIS).expect("parse example lane genesis");
    assert_eq!(genesis, expected);
    assert_eq!(genesis.to_json().expect("serialize"), EXAMPLE_LANE_GENESIS);
    assert_eq!(genesis_hash(&genesis), EXAMPLE_LANE_GENESIS_HASH);
    let record: serde_json::Value = serde_json::from_str(include_str!(
        "../../../pft-token-contracts/deployments/pft-lock-base-84532/deployment.json"
    ))
    .expect("lock deployment record");
    let policy = &genesis.external_lanes()[0].policy;
    let pinned = |key: &str| record[key].as_str().expect(key).to_ascii_lowercase();
    assert_eq!(record["chain_id"].as_u64(), Some(policy.chain_id));
    assert_eq!(policy.lock_contract_address, pinned("lock"));
    assert_eq!(policy.token_address, pinned("token"));
    assert_eq!(policy.lock_runtime_code_hash, pinned("lock_runtime_codehash"));
    assert_eq!(policy.token_runtime_code_hash, pinned("token_codehash"));
    assert_eq!(genesis.external_lanes()[0].release_cap_bps, 200);

    let data_dir = unique_test_dir("postfiat-external-lane-genesis-file");
    let report = init_from_genesis_file(InitFromGenesisFileOptions {
        data_dir: data_dir.clone(),
        node_id: "validator-0".to_string(),
        genesis_file: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata/external-lane-genesis/genesis.json"),
    })
    .expect("init from the example lane genesis");
    assert_eq!(report.genesis_hash, EXAMPLE_LANE_GENESIS_HASH);
    let store = NodeStore::new(&data_dir);
    assert_eq!(store.read_genesis().expect("genesis"), expected);
    let ledger = store.read_ledger().expect("ledger");
    let faucet = read_faucet_account_file(&data_dir.join(FAUCET_ACCOUNT_FILE)).expect("faucet");
    assert_eq!(faucet.balance, GENESIS_NATIVE_SUPPLY_ATOMS - LANE_RESERVED_ATOMS);
    let in_code = genesis_ledger_state(&expected, faucet).expect("in-code genesis ledger");
    assert_eq!(ledger, in_code);
    assert_eq!(ledger.external_lanes, expected.genesis_external_lane_states());
    validate_external_lanes_match_genesis(&expected, &ledger, &GovernanceState::new(4), "init").expect("lane set rule");
    let root = replicated_state_root(
        &expected,
        &GovernanceState::new(expected.validator_count),
        &in_code,
        &Vec::<String>::new(),
        &ShieldedState::empty(),
        &BridgeState::empty(),
    )
    .expect("in-code state root");
    assert_eq!(report.state_root, root);
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("height-zero replay of the file genesis");
    fs::remove_dir_all(data_dir).expect("cleanup");
}

#[test]
fn init_from_genesis_file_refuses_invalid_lanes_before_writing() {
    let file_dir = unique_test_dir("postfiat-external-lane-invalid-file");
    fs::create_dir_all(&file_dir).expect("file dir");
    // A second lane on the same Base source: with the same id, then another.
    let cases = [
        ("duplicate-id", "base-erc20", "duplicate genesis external lane id `base-erc20`"),
        (
            "shared-source",
            "base-erc20-b",
            "genesis external lanes `base-erc20` and `base-erc20-b` both name Base source 84532",
        ),
    ];
    for (label, second_id, needle) in cases {
        let mut genesis = sepolia_rehearsal_lane_genesis();
        let lanes = genesis.external_lanes.as_mut().expect("lanes");
        let mut second = lanes[0].clone();
        second.id = second_id.to_string();
        lanes.push(second);
        let genesis_file = file_dir.join(format!("{label}.json"));
        fs::write(&genesis_file, genesis.to_json().expect("serialize")).expect("write genesis");
        let data_dir = unique_test_dir(&format!("postfiat-external-lane-{label}-file-init"));
        let error = init_from_genesis_file(InitFromGenesisFileOptions {
            data_dir: data_dir.clone(),
            node_id: "validator-0".to_string(),
            genesis_file,
        })
        .expect_err("init must refuse the lane genesis");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{label}: {error}");
        assert!(error.to_string().contains(needle), "{label}: {error}");
        assert!(!data_dir.exists(), "{label}: nothing is written");
    }
    fs::remove_dir_all(file_dir).expect("cleanup");
}

type LedgerEdit = fn(&mut LedgerState);

/// A lane state with one recorded release of 5 atoms to the faucet account.
fn record_release(ledger: &mut LedgerState) {
    let genesis = lane_genesis("postfiat-lane-recorded", 1);
    ledger.external_lanes[0]
        .apply_release(&genesis.external_lanes()[0], lane_release_record(99, 5, 1))
        .expect("recorded release");
    ledger.accounts[0].balance += 5;
}

#[test]
fn checkpoint_with_recorded_releases_is_accepted_and_tampering_them_is_refused() {
    let (data_dir, checkpoint, _) = pruned_lane_node("recorded-release");
    // Genesis reserve minus the recorded release: the checkpoint is accepted.
    let (result, accepted) = reread_tampered_checkpoint(&data_dir, &checkpoint, record_release);
    assert_aggregate_balances(&accepted);
    let read = result.expect("recorded release passes").expect("checkpoint");
    assert_eq!(read.ledger.external_lanes[0].released_atoms, 5);
    assert_eq!(read.ledger.external_lanes[0].releases.len(), 1);

    // Each tamper keeps live + burns = supply, so only the lane rule refuses it.
    let cases: [(&str, LedgerEdit); 4] = [
        ("recorded releases sum to 5", |ledger| {
            record_release(ledger);
            let lane = &mut ledger.external_lanes[0];
            lane.released_atoms += 1;
            lane.window_released_atoms += 1;
            ledger.accounts[0].balance += 1;
        }),
        ("recorded releases sum to 6", |ledger| {
            record_release(ledger);
            ledger.external_lanes[0].releases[0].amount_atoms += 1;
        }),
        ("differ from its recorded releases", |ledger| {
            record_release(ledger);
            ledger.external_lanes[0].releases[0].lock_nonce = 98;
        }),
        ("from a source genesis did not configure", |ledger| {
            record_release(ledger);
            ledger.external_lanes[0].releases[0].source_chain_id = 84_532;
        }),
    ];
    for (needle, edit) in cases {
        let (result, tampered) = reread_tampered_checkpoint(&data_dir, &checkpoint, edit);
        assert_aggregate_balances(&tampered);
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("history checkpoint external lane `base-erc20`") && error.contains(needle),
            "{needle}: {error}"
        );
    }
    fs::remove_dir_all(data_dir).expect("cleanup");
}

/// Three releases of 5 atoms to the faucet account: heights 1 and 2 in
/// window 0, then one at 86,402, a window past height 2, which prunes both
/// earlier records behind the finalized height.
fn record_pruned_releases(ledger: &mut LedgerState) {
    let genesis = lane_genesis("postfiat-lane-recorded", 1);
    for (nonce, height) in [(90_u64, 1_u64), (91, 2), (92, 86_402)] {
        ledger.external_lanes[0]
            .apply_release(&genesis.external_lanes()[0], lane_release_record(nonce, 5, height))
            .expect("recorded release");
    }
    ledger.accounts[0].balance += 15;
}

#[test]
fn checkpoint_with_a_pruned_lane_is_accepted_and_tampering_it_is_refused() {
    let (data_dir, checkpoint, _) = pruned_lane_node("pruned-release-records");
    let (result, accepted) =
        reread_tampered_checkpoint(&data_dir, &checkpoint, record_pruned_releases);
    assert_aggregate_balances(&accepted);
    let read = result.expect("pruned lane passes").expect("checkpoint");
    let lane = &read.ledger.external_lanes[0];
    assert_eq!(lane.released_atoms, 15);
    assert_eq!(lane.releases.len(), 1);
    assert_eq!(lane.pruned_release_count, 2);
    assert_eq!(lane.released_lock_nonces.len(), 3);

    let cases: [(&str, LedgerEdit); 4] = [
        // A forged stored record.
        ("does not match its recorded releases", |ledger| {
            record_pruned_releases(ledger);
            ledger.external_lanes[0].releases[0].recipient = "pftl-forged".to_string();
        }),
        // A tampered root.
        ("does not match its recorded releases", |ledger| {
            record_pruned_releases(ledger);
            ledger.external_lanes[0].pruned_release_history_root =
                ledger.external_lanes[0].release_history_root.clone();
        }),
        // A paid nonce removed from the replay set.
        ("differ from its recorded releases", |ledger| {
            record_pruned_releases(ledger);
            ledger.external_lanes[0].released_lock_nonces.remove(&90);
        }),
        // Pruned atoms raised with the balance moved to an account.
        ("recorded releases sum to 15", |ledger| {
            record_pruned_releases(ledger);
            let lane = &mut ledger.external_lanes[0];
            lane.released_atoms += 1;
            lane.window_released_atoms += 1;
            ledger.accounts[0].balance += 1;
        }),
    ];
    for (needle, edit) in cases {
        let (result, tampered) = reread_tampered_checkpoint(&data_dir, &checkpoint, edit);
        assert_aggregate_balances(&tampered);
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("history checkpoint external lane `base-erc20`") && error.contains(needle),
            "{needle}: {error}"
        );
    }
    fs::remove_dir_all(data_dir).expect("cleanup");
}

#[test]
fn pruned_lane_record_round_trips_and_replays_to_the_same_state() {
    let genesis = lane_genesis("postfiat-lane-recorded", 1);
    let mut ledger = LedgerState::new(Vec::new());
    ledger.external_lanes = genesis.genesis_external_lane_states();
    let mut replayed = ledger.clone();
    let genesis_lane = &genesis.external_lanes()[0];
    for (nonce, height) in [(90_u64, 1_u64), (91, 2), (92, 86_402)] {
        ledger.external_lanes[0]
            .apply_release(genesis_lane, lane_release_record(nonce, 5, height))
            .expect("release");
        replayed.external_lanes[0]
            .apply_release(genesis_lane, lane_release_record(nonce, 5, height))
            .expect("replayed release");
    }
    // Replaying the same releases from genesis prunes to the same state.
    assert_eq!(replayed.external_lanes, ledger.external_lanes);
    let lane = &ledger.external_lanes[0];
    assert_eq!(lane.pruned_release_count, 2);
    // The stored ledger encoding restores the same lane and commitment.
    let bytes = serde_json::to_vec(&ledger).expect("encode");
    let restored: LedgerState = serde_json::from_slice(&bytes).expect("decode");
    assert_eq!(restored.external_lanes, ledger.external_lanes);
    assert_eq!(
        restored.external_lanes[0].state_commitment_bytes().expect("commit"),
        lane.state_commitment_bytes().expect("commit")
    );
    restored.external_lanes[0]
        .validate_against_genesis(genesis_lane)
        .expect("restored pruned lane");
}

// ---- P2.4: the release path on a local chain --------------------------------

mod release_path {
    use super::*;
    use postfiat_crypto_provider::{
        address_from_public_key, bytes_to_hex, hex_to_bytes, ml_dsa_65_keygen_from_seed,
        ml_dsa_65_sign, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair, ML_DSA_65_ALGORITHM,
    };
    use postfiat_types::{
        AssetTransactionOperation, EthereumCheckpointCertificateV1, EthereumCheckpointVoteV1,
        EthereumExternalEventProofV1, EthereumFinalizedCheckpointV1, EthereumReceiptProofV1,
        ExternalLaneReleaseCapChange, ExternalLaneReleaseOperationV1, FastSwapChainDomainV1, FastSwapCommitteeDomainV1,
        FastSwapCommitteeRootV1, FastSwapCommitteeV1, FastSwapOpaqueHashV1, FastSwapValidatorV1,
        SignedAssetTransaction, UnsignedAssetTransaction, ETHEREUM_CHECKPOINT_SCHEMA_V1,
        ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1, EXTERNAL_LANE_RELEASE_TRANSACTION_KIND_V1,
        FASTSWAP_SCHEMA_VERSION_V1,
    };

    struct Chain {
        data_dir: PathBuf,
        genesis: Genesis,
        keys: Vec<MlDsa65KeyPair>,
        committee: FastSwapCommitteeV1,
        recipient: String,
    }

    fn keccak(bytes: &[u8]) -> [u8; 32] {
        postfiat_bridge::ethereum_keccak256(bytes)
    }

    fn rlp(base: u8, payload: &[u8]) -> Vec<u8> {
        if base == 0x80 && payload.len() == 1 && payload[0] <= 0x7f {
            return payload.to_vec();
        }
        let mut encoded = if payload.len() < 56 {
            vec![base + payload.len() as u8]
        } else {
            let length = payload.len().to_be_bytes();
            let first = length.iter().position(|byte| *byte != 0).unwrap_or(7);
            let mut prefix = vec![base + 55 + (8 - first) as u8];
            prefix.extend_from_slice(&length[first..]);
            prefix
        };
        encoded.extend_from_slice(payload);
        encoded
    }

    fn word(value: u64) -> [u8; 32] {
        let mut word = [0_u8; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }

    /// A four-validator local chain whose genesis lane names the epoch-one
    /// committee (validator ids `validator-0..3`, deterministic checkpoint
    /// keys). A signed FastSwap governance bootstrap registers it, so replay,
    /// prune and snapshots reproduce it.
    fn lane_chain(label: &str) -> Chain {
        lane_chain_with_window(label, 86_400)
    }

    fn lane_chain_with_window(label: &str, release_window_blocks: u64) -> Chain {
        lane_chain_with(label, release_window_blocks, None, 4)
    }

    /// `validator_count` sizes the validator set and the lane committee,
    /// which the FastSwap bootstrap binds to it, at quorum ⌊2n/3⌋ + 1.
    fn lane_chain_with(
        label: &str,
        release_window_blocks: u64,
        consensus_v2_activation_height: Option<u64>,
        validator_count: u32,
    ) -> Chain {
        let keys: Vec<_> = (0..u8::try_from(validator_count).expect("count"))
            .map(|index| ml_dsa_65_keygen_from_seed(&[0x81 + index; 32]))
            .collect();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: String::new(),
                    genesis_hash: FastSwapOpaqueHashV1([0; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: u16::try_from(validator_count).expect("count"),
                quorum: u16::try_from(2 * validator_count / 3 + 1).expect("quorum"),
            },
            validators: keys
                .iter()
                .enumerate()
                .map(|(index, key)| FastSwapValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        committee.domain.committee_root = committee.computed_root().expect("root");
        let mut genesis = lane_genesis(&format!("postfiat-lane-{label}"), validator_count);
        genesis.external_lanes.as_mut().expect("lanes")[0].release_window_blocks =
            release_window_blocks;
        genesis.consensus_v2_activation_height = consensus_v2_activation_height;
        let policy = &mut genesis.external_lanes.as_mut().expect("lanes")[0].policy;
        policy.committee_epoch = 1;
        policy.committee_root = format!("0x{}", bytes_to_hex(&committee.domain.committee_root.0));
        genesis.validate().expect("lane genesis");
        committee.domain.chain = FastSwapChainDomainV1 {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: FastSwapOpaqueHashV1(
                hex_to_bytes(&genesis_hash(&genesis))
                    .expect("hash hex")
                    .try_into()
                    .expect("48-byte hash"),
            ),
            protocol_version: genesis.protocol_version,
        };
        let data_dir = unique_test_dir(&format!("postfiat-external-lane-{label}"));
        init_with_genesis(data_dir.clone(), "validator-0".to_string(), genesis.clone())
            .expect("init lane genesis");
        bootstrap_committee(&data_dir, &committee, validator_count);
        let recipient_key = ml_dsa_65_keygen_from_seed(&[0x99; 32]);
        Chain {
            data_dir,
            genesis,
            keys,
            committee,
            recipient: address_from_public_key(&recipient_key.public_key),
        }
    }

    /// Registers `committee` through a governance batch signed by all
    /// `validator_count` validators, as `fastswap_epoch_one_bootstrap_is_governance_bound_and_canonically_committed` does.
    fn bootstrap_committee(data_dir: &Path, committee: &FastSwapCommitteeV1, validator_count: u32) {
        let validator_keys =
            read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
        let payload = postfiat_types::FastSwapGovernanceBootstrapPayloadV1 {
            committee: committee.clone(),
            asset_rules: Vec::new(),
            policies: Vec::new(),
            activation_height: 10,
        };
        let payload_file = data_dir.join("fastswap-bootstrap-payload.json");
        let amendment_file = data_dir.join("fastswap-bootstrap-amendment.unsigned.json");
        let signed_amendment_file = data_dir.join("fastswap-bootstrap-amendment.signed.json");
        let batch_file = data_dir.join("fastswap-bootstrap-batch.json");
        fs::write(&payload_file, serde_json::to_vec_pretty(&payload).expect("payload json"))
            .expect("write payload");
        let validators = local_validator_ids(validator_count).expect("validator ids");
        create_fastswap_governance_bootstrap(FastSwapGovernanceBootstrapOptions {
            data_dir: data_dir.to_path_buf(),
            validators: validators.clone(),
            support: validators,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            payload_file: payload_file.clone(),
            amendment_file: amendment_file.clone(),
            batch_file: data_dir.join("fastswap-bootstrap-batch.unsigned.json"),
        })
        .expect("bootstrap proposal");
        let authorization_files = write_split_validator_key_files(data_dir, &validator_keys)
            .iter()
            .map(|(validator, key_file)| {
                let authorization_file =
                    data_dir.join(format!("{validator}.fastswap-authorization.json"));
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: data_dir.to_path_buf(),
                    amendment_file: amendment_file.clone(),
                    validator: validator.clone(),
                    validator_key_file: key_file.clone(),
                    proposal_slot: 1,
                    expires_at_height: 8,
                    authorization_file: authorization_file.clone(),
                })
                .expect("sign bootstrap");
                authorization_file
            })
            .collect::<Vec<_>>();
        assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
            data_dir: data_dir.to_path_buf(),
            amendment_file,
            authorization_files,
            proposal_slot: 1,
            output_file: signed_amendment_file.clone(),
        })
        .expect("assemble amendment");
        assemble_signed_fastswap_governance_bootstrap(SignedFastSwapGovernanceBootstrapOptions {
            data_dir: data_dir.to_path_buf(),
            payload_file,
            signed_amendment_file,
            proposal_slot: 1,
            batch_file: batch_file.clone(),
        })
        .expect("assemble bootstrap");
        let receipts = apply_governance_batch(ApplyBatchOptions {
            data_dir: data_dir.to_path_buf(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply bootstrap");
        assert!(receipts[0].accepted, "{receipts:?}");
        let ledger = NodeStore::new(data_dir).read_ledger().expect("ledger");
        assert_eq!(ledger.fastswap_committees, vec![committee.clone()]);
    }

    /// A committee checkpoint of a Base block whose one receipt holds
    /// `Locked(nonce, holder, keccak(recipient), recipient, amount)`.
    fn lock_proof(chain: &Chain, nonce: u64, amount: u64) -> EthereumExternalEventProofV1 {
        let lane = &chain.genesis.external_lanes()[0];
        let lock_contract = [0x1a_u8; 20];
        let mut holder = [0_u8; 32];
        holder[12..].copy_from_slice(&[0x11; 20]);
        let topics = [
            keccak(b"Locked(uint64,address,bytes32,bytes,uint64)"),
            word(nonce),
            holder,
            keccak(chain.recipient.as_bytes()),
        ];
        let mut data = word(0x40).to_vec();
        data.extend_from_slice(&word(amount));
        data.extend_from_slice(&word(chain.recipient.len() as u64));
        data.extend_from_slice(chain.recipient.as_bytes());
        data.resize(data.len().div_ceil(32) * 32, 0);
        let topic_items: Vec<u8> = topics.iter().flat_map(|topic| rlp(0x80, topic)).collect();
        let log = rlp(
            0xc0,
            &[rlp(0x80, &lock_contract), rlp(0xc0, &topic_items), rlp(0x80, &data)].concat(),
        );
        let receipt = rlp(
            0xc0,
            &[rlp(0x80, &[1]), rlp(0x80, &[1]), rlp(0x80, &[0; 256]), rlp(0xc0, &log)].concat(),
        );
        let leaf = rlp(0xc0, &[rlp(0x80, &[0x20, 0x80]), rlp(0x80, &receipt)].concat());
        let checkpoint = EthereumFinalizedCheckpointV1 {
            schema_version: ETHEREUM_CHECKPOINT_SCHEMA_V1,
            pftl_domain: chain.committee.domain.chain.clone(),
            route_id: lane.id.clone(),
            route_config_digest: FastSwapOpaqueHashV1(lane.route_config_digest().expect("digest")),
            ethereum_chain_id: lane.policy.chain_id,
            block_number: 1_000 + nonce,
            block_hash: [0x6f; 32],
            receipts_root: keccak(&leaf),
            observed_head_number: 2_000 + nonce,
            minimum_confirmations: 64,
            authority_epoch: chain.committee.domain.committee_epoch,
            committee_root: chain.committee.domain.committee_root,
            handoff_controller: lock_contract,
            wrapped_navcoin_token: [0x2b; 20],
            handoff_controller_code_hash: [0x4d; 32],
            wrapped_navcoin_code_hash: [0x3c; 32],
        };
        let votes = chain
            .keys
            .iter()
            .enumerate()
            .take(usize::from(chain.committee.domain.quorum))
            .map(|(index, key)| {
                let mut vote = EthereumCheckpointVoteV1 {
                    validator_id: format!("validator-{index}"),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context_seed(
                    &key.private_key,
                    &vote.signing_bytes(&checkpoint).expect("vote bytes"),
                    ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
                    &[0xc0 + index as u8; 32],
                )
                .expect("vote");
                vote
            })
            .collect();
        EthereumExternalEventProofV1 {
            checkpoint_certificate: EthereumCheckpointCertificateV1 { checkpoint, votes },
            receipt_proof: EthereumReceiptProofV1 {
                transaction_index: 0,
                receipt_rlp: receipt,
                proof_nodes_rlp: vec![leaf],
            },
            log_index: 0,
        }
    }

    fn release_tx(
        chain: &Chain,
        lane_id: &str,
        nonce: u64,
        amount: u64,
        sequence: u64,
    ) -> SignedAssetTransaction {
        let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
        let unsigned = UnsignedAssetTransaction {
            chain_id: chain.genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&chain.genesis),
            protocol_version: chain.genesis.protocol_version,
            address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
            transaction_kind: EXTERNAL_LANE_RELEASE_TRANSACTION_KIND_V1.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            source: faucet.address.clone(),
            fee: 1_000,
            sequence,
            operation: AssetTransactionOperation::ExternalLaneReleaseV1(
                ExternalLaneReleaseOperationV1 {
                    submitter: faucet.address.clone(),
                    lane_id: lane_id.to_string(),
                    source_chain_id: 8453,
                    lock_contract_address: format!("0x{}", "1a".repeat(20)),
                    lock_nonce: nonce,
                    amount_atoms: amount,
                    recipient: chain.recipient.clone(),
                    proof: lock_proof(chain, nonce, amount),
                },
            ),
        };
        let private_key = hex_to_bytes(&faucet.private_key_hex).expect("key bytes");
        let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign");
        SignedAssetTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: faucet.public_key_hex.clone(),
            signature_hex: bytes_to_hex(&signature),
        }
    }

    /// Admits the transaction through the mempool, proposes a batch from the
    /// mempool and applies it: the path a submitted release takes.
    fn admit_and_apply(chain: &Chain, tx: &SignedAssetTransaction, label: &str) -> Receipt {
        admit_and_apply_with(chain, tx, label, false)
    }

    /// With `consensus_v2`, the block carries a block certificate and a
    /// consensus v2 commit signed by all four validators, so it can be a
    /// finalized checkpoint.
    fn admit_and_apply_with(
        chain: &Chain,
        tx: &SignedAssetTransaction,
        label: &str,
        consensus_v2: bool,
    ) -> Receipt {
        submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
            data_dir: chain.data_dir.clone(),
            signed_asset_transaction_json: serde_json::to_string(tx).expect("tx json"),
        })
        .expect("mempool admits the release");
        let batch_file = chain.data_dir.join(format!("{label}.batch.json"));
        create_mempool_batch(MempoolBatchOptions {
            data_dir: chain.data_dir.clone(),
            batch_file: batch_file.clone(),
            max_transactions: 8,
        })
        .expect("mempool batch");
        let certificate_file =
            consensus_v2.then(|| consensus_v2_certificate(&chain.data_dir, &batch_file, label));
        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: chain.data_dir.clone(),
            batch_file,
            certificate_file,
        })
        .expect("apply batch");
        assert_eq!(receipts.len(), 1, "{receipts:?}");
        receipts.into_iter().next().expect("receipt")
    }

    /// A block certificate with a consensus v2 commit for the next block, as
    /// `active_fastpay_certify` builds it: the fixture holds every validator
    /// key, so it signs every vote directly.
    fn consensus_v2_certificate(data_dir: &Path, batch_file: &Path, label: &str) -> PathBuf {
        let data_dir = data_dir.to_path_buf();
        let key_file = data_dir.join(VALIDATOR_KEYS_FILE);
        let height = NodeStore::new(&data_dir).read_chain_tip().expect("tip").height + 1;
        let proposer = block_proposer(BlockProposerOptions {
            data_dir: data_dir.clone(),
            block_height: height,
            view: 0,
        })
        .expect("proposer")
        .proposer;
        let proposal_file = data_dir.join(format!("{label}.proposal.json"));
        let proposal = propose_batch(BatchProposalOptions {
            data_dir: data_dir.clone(),
            verify_block_log: false,
            batch_kind: None,
            batch_file: batch_file.to_path_buf(),
            proposal_file: proposal_file.clone(),
            view: Some(0),
            timeout_certificate_file: None,
            key_file: Some(key_file.clone()),
            validator_id: Some(proposer),
        })
        .expect("proposal");
        let vote_files = local_validator_ids(4)
            .expect("validator ids")
            .into_iter()
            .map(|validator| {
                let vote_file = data_dir.join(format!("{label}.{validator}.vote.json"));
                create_block_vote(BlockVoteOptions {
                    data_dir: data_dir.clone(),
                    verify_block_log: false,
                    key_file: key_file.clone(),
                    validator_id: Some(validator),
                    batch_file: Some(batch_file.to_path_buf()),
                    proposal_file: Some(proposal_file.clone()),
                    timeout_certificate_file: None,
                    block_height: Some(proposal.block_height),
                    vote_file: vote_file.clone(),
                })
                .expect("block vote");
                vote_file
            })
            .collect();
        let certificate_file = data_dir.join(format!("{label}.certificate.json"));
        let mut certificate = aggregate_block_certificate(BlockCertificateOptions {
            data_dir: data_dir.clone(),
            verify_block_log: false,
            batch_file: Some(batch_file.to_path_buf()),
            proposal_file: Some(proposal_file),
            timeout_certificate_file: None,
            block_height: Some(proposal.block_height),
            vote_files,
            certificate_file: certificate_file.clone(),
        })
        .expect("block certificate");
        let consensus_proposal =
            create_consensus_v2_proposal_for_block(&data_dir, &proposal, None, &key_file)
                .expect("consensus v2 proposal");
        let keys = read_validator_key_file(&key_file).expect("validator keys");
        let sign_votes = |phase| {
            keys.validators
                .iter()
                .map(|key| {
                    let mut vote = postfiat_types::ConsensusV2Vote {
                        schema: postfiat_types::CONSENSUS_V2_VOTE_SCHEMA.into(),
                        domain: consensus_proposal.domain.clone(),
                        round: consensus_proposal.round,
                        phase,
                        block: Some(consensus_proposal.block.clone()),
                        validator: key.node_id.clone(),
                        signature: postfiat_types::ConsensusV2Signature {
                            algorithm_id: ML_DSA_65_ALGORITHM.into(),
                            signer: key.node_id.clone(),
                            public_key_hex: key.public_key_hex.clone(),
                            signature_hex: "00".into(),
                        },
                    };
                    vote.signature.signature_hex = bytes_to_hex(
                        &postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                            &hex_to_bytes(&key.private_key_hex).expect("key hex"),
                            &postfiat_ordering_fast::consensus_v2_vote_signing_bytes(&vote)
                                .expect("vote bytes"),
                            postfiat_ordering_fast::CONSENSUS_V2_VOTE_CONTEXT,
                        )
                        .expect("vote signature"),
                    );
                    vote
                })
                .collect()
        };
        let qc = |phase| {
            certify_and_persist_consensus_v2_votes(
                &data_dir,
                consensus_proposal.round,
                phase,
                Some(consensus_proposal.block.clone()),
                sign_votes(phase),
            )
            .expect("consensus v2 QC")
        };
        let prepare_qc = qc(postfiat_types::ConsensusV2Phase::Prepare);
        let precommit_qc = qc(postfiat_types::ConsensusV2Phase::Precommit);
        certificate.consensus_v2_commit = Some(
            assemble_consensus_v2_commit(
                &data_dir,
                &proposal,
                consensus_proposal,
                None,
                prepare_qc,
                precommit_qc,
            )
            .expect("consensus v2 commit"),
        );
        write_block_certificate_file(&certificate_file, &certificate).expect("write certificate");
        certificate_file
    }

    /// A refused release: mempool admission refuses it with `code`, and so
    /// does the batch builder's dry run when it is handed the transaction
    /// directly, so no block proposed by this node carries it.
    fn refused_everywhere(chain: &Chain, tx: &SignedAssetTransaction, code: &str, label: &str) {
        let error = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: chain.data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(tx).expect("tx json"),
            },
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(&format!("mempool admission rejected `{code}`")), "{error}");
        let tx_file = chain.data_dir.join(format!("{label}.tx.json"));
        fs::write(&tx_file, serde_json::to_vec(tx).expect("tx json")).expect("write tx");
        let error = create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: chain.data_dir.clone(),
            batch_file: chain.data_dir.join(format!("{label}.batch.json")),
            signed_asset_transaction_files: vec![tx_file],
        })
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("rejected in batch dry-run at height") && error.contains(&format!(": {code} (")),
            "{error}"
        );
    }

    fn lane_page(data_dir: &Path, cursor: Option<u64>, limit: Option<usize>) -> io::Result<ExternalLaneInfoRow> {
        let report = external_lane_info(ExternalLaneInfoOptions {
            data_dir: data_dir.to_path_buf(),
            lane_id: Some("base-erc20".to_string()),
            release_cursor: cursor,
            release_limit: limit,
        })?;
        assert_eq!(report.lane_count, 1);
        Ok(report.lanes.into_iter().next().expect("lane"))
    }

    fn lane_info(data_dir: &Path) -> ExternalLaneInfoRow {
        let row = lane_page(data_dir, None, None).expect("lane info");
        assert!(row.state.releases.is_empty(), "records are paged, not in the state");
        row
    }

    #[test]
    fn external_lane_release_lifecycle_on_a_local_chain() {
        let chain = lane_chain("release-lifecycle");
        let store = NodeStore::new(&chain.data_dir);
        let cap = chain.genesis.external_lanes()[0].window_cap_atoms();
        let supply = |store: &NodeStore| {
            native_pft_live_total(
                &store.read_ledger().expect("ledger"),
                &store.read_shielded().expect("shielded"),
            )
            .expect("live")
        };
        let live_before = supply(&store);

        // 1. A proven Lock is released once, to the recipient it names.
        let amount = ACCOUNT_RESERVE + 1_000;
        let tx = release_tx(&chain, "base-erc20", 1, amount, 1);
        // The recipient is a destination address and the amount is counted.
        assert_eq!(
            asset_transaction_destination_addresses(&tx),
            vec![("external_lane_release_v1.recipient", chain.recipient.as_str())]
        );
        assert_eq!(crate::mempool_proposals::asset_transaction_amount(&tx), amount);
        let receipt = admit_and_apply(&chain, &tx, "release");
        assert!(receipt.accepted, "{receipt:?}");
        let ledger = store.read_ledger().expect("ledger");
        assert_eq!(ledger.account(&chain.recipient).expect("recipient").balance, amount);
        let info = lane_info(&chain.data_dir);
        assert_eq!(info.state.released_atoms, amount);
        assert_eq!(info.remainder_atoms, LANE_RESERVED_ATOMS - amount);
        assert_eq!(info.release_count, 1);
        assert_eq!(info.releases[0].sequence, 0);
        assert_eq!(info.releases[0].record.lock_nonce, 1);
        assert_eq!(info.releases[0].record.source_block_number, 1_001);
        assert_eq!(info.next_release_cursor, None);
        assert_eq!(info.next_window_available_atoms, cap - amount);
        // Only the fee left the live supply: the release moved lane atoms to an account.
        assert_eq!(supply(&store) + u128::from(receipt.fee_burned), live_before);

        // 2. The same Lock again, 3. a Lock above the window cap and 4. a
        // lane genesis never configured are refused without state change.
        let refusals = [
            (release_tx(&chain, "base-erc20", 1, amount, 2), "external_lane_nonce_already_released"),
            (release_tx(&chain, "base-erc20", 2, cap, 2), "external_lane_window_cap_exceeded"),
            (release_tx(&chain, "other-lane", 3, amount, 2), "external_lane_not_configured"),
        ];
        for (index, (tx, code)) in refusals.iter().enumerate() {
            let lane_before = lane_info(&chain.data_dir).state;
            refused_everywhere(&chain, tx, code, &format!("refused-{index}"));
            assert_eq!(lane_info(&chain.data_dir).state, lane_before);
        }

        // The governed committee replays, so the released chain verifies from genesis.
        verify_blocks(NodeOptions {
            data_dir: chain.data_dir.clone(),
        })
        .expect("replay from genesis through the releases");
        let _ = fs::remove_dir_all(&chain.data_dir);
    }

    /// Five releases in two-block windows on a local chain, one block each,
    /// so later releases prune the earlier records. A full snapshot export
    /// and import into a fresh directory restores the same lane, pruned
    /// fields and state root; the block and state verifiers pass on both
    /// nodes, and the replay from genesis reproduces every committed state
    /// root, the pruned fields among them.
    #[test]
    fn pruned_lane_survives_a_snapshot_round_trip_and_a_replay() {
        let chain = lane_chain_with_window("release-prune", 2);
        let amounts: Vec<u64> = (1..=5_u64).map(|nonce| ACCOUNT_RESERVE + 100 * nonce).collect();
        for (index, amount) in amounts.iter().enumerate() {
            let nonce = index as u64 + 1;
            let tx = release_tx(&chain, "base-erc20", nonce, *amount, nonce);
            let receipt = admit_and_apply(&chain, &tx, &format!("prune-{nonce}"));
            assert!(receipt.accepted, "{receipt:?}");
        }
        let ledger = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger");
        let lane = ledger.external_lanes[0].clone();
        assert!(lane.pruned_release_count > 0, "{lane:?}");
        assert_eq!(lane.release_count(), 5);
        assert_eq!(lane.released_lock_nonces, (1..=5_u64).collect());
        assert_eq!(lane.released_atoms, amounts.iter().sum::<u64>());
        assert_eq!(
            lane.pruned_released_atoms,
            amounts[..lane.pruned_release_count as usize].iter().sum::<u64>()
        );
        lane.validate_against_genesis(&chain.genesis.external_lanes()[0])
            .expect("pruned lane");
        let info = lane_info(&chain.data_dir);
        assert_eq!(info.release_count, 5);
        assert_eq!(info.pruned_release_count, lane.pruned_release_count);
        assert_eq!(info.release_history_root, lane.release_history_root);
        assert_eq!(info.releases.len() as u64, 5 - lane.pruned_release_count);
        assert_eq!(
            info.releases.last().expect("oldest stored").sequence,
            lane.pruned_release_count
        );
        assert_eq!(info.next_release_cursor, None);

        let source = status(NodeOptions {
            data_dir: chain.data_dir.clone(),
        })
        .expect("source status");
        let snapshot_dir = sibling_dir(&chain.data_dir, "prune-snapshot");
        let restored_dir = sibling_dir(&chain.data_dir, "prune-restored");
        let manifest = export_snapshot(SnapshotExportOptions {
            data_dir: chain.data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect("export snapshot of the pruned lane");
        assert_eq!(manifest.state_root, source.state_root);
        let restored = import_snapshot(SnapshotImportOptions {
            data_dir: restored_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: Some("validator-restored".to_string()),
        })
        .expect("import snapshot of the pruned lane");
        assert_eq!(restored.state_root, source.state_root);
        let restored_ledger = NodeStore::new(&restored_dir).read_ledger().expect("ledger");
        assert_eq!(restored_ledger.external_lanes, ledger.external_lanes);
        assert_eq!(lane_info(&restored_dir), info);
        for data_dir in [&chain.data_dir, &restored_dir] {
            let blocks = verify_blocks(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("replay from genesis through the pruning releases");
            assert_eq!(blocks.state_root, source.state_root);
            let state = verify_state(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("state verifiers");
            assert!(state.verified, "{state:?}");
        }
        for dir in [chain.data_dir, snapshot_dir, restored_dir] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    /// The same pruned lane exported from its finalized checkpoint rather
    /// than its full history: consensus v2 activates at the last release, so
    /// that block is a certified checkpoint. The import into a fresh
    /// directory restores the same lane, pruned fields included, the same
    /// state root and the same read report, and the checkpoint and state
    /// verifiers pass on the restored node.
    #[test]
    fn pruned_lane_restores_from_a_finalized_checkpoint_snapshot() {
        // The committee bootstrap is block 1, the releases blocks 2 to 6.
        let chain = lane_chain_with("checkpoint-prune", 2, Some(6), 4);
        for nonce in 1..=5_u64 {
            let tx = release_tx(&chain, "base-erc20", nonce, ACCOUNT_RESERVE + 100 * nonce, nonce);
            let receipt = admit_and_apply_with(&chain, &tx, &format!("prune-{nonce}"), nonce == 5);
            assert!(receipt.accepted, "{receipt:?}");
        }
        let ledger = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger");
        let lane = ledger.external_lanes[0].clone();
        assert!(lane.pruned_release_count > 0, "{lane:?}");
        assert!(lane.pruned_release_history_root.is_some(), "{lane:?}");
        assert_eq!(lane.release_count(), 5);
        let info = lane_info(&chain.data_dir);
        assert_eq!(info.pruned_release_count, lane.pruned_release_count);

        let source = status(NodeOptions {
            data_dir: chain.data_dir.clone(),
        })
        .expect("source status");
        assert_eq!(source.block_height, 6);
        let checkpoint = verify_finalized_checkpoint(NodeOptions {
            data_dir: chain.data_dir.clone(),
        })
        .expect("source checkpoint");
        assert!(checkpoint.verified);
        assert_eq!(checkpoint.checkpoint_state_root, source.state_root);
        let snapshot_dir = sibling_dir(&chain.data_dir, "checkpoint-snapshot");
        let restored_dir = sibling_dir(&chain.data_dir, "checkpoint-restored");
        let manifest = export_snapshot_from_finalized_checkpoint(SnapshotExportOptions {
            data_dir: chain.data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect("export checkpoint snapshot of the pruned lane");
        assert_eq!(manifest.state_root, source.state_root);
        let restored = import_snapshot_from_finalized_checkpoint(SnapshotImportOptions {
            data_dir: restored_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: Some("validator-restored".to_string()),
        })
        .expect("import checkpoint snapshot of the pruned lane");
        assert_eq!(restored.state_root, source.state_root);
        let restored_ledger = NodeStore::new(&restored_dir).read_ledger().expect("ledger");
        assert_eq!(restored_ledger.external_lanes, ledger.external_lanes);
        restored_ledger.external_lanes[0]
            .validate_against_genesis(&chain.genesis.external_lanes()[0])
            .expect("restored pruned lane");
        assert_eq!(lane_info(&restored_dir), info);
        let restored_checkpoint = verify_finalized_checkpoint(NodeOptions {
            data_dir: restored_dir.clone(),
        })
        .expect("restored checkpoint");
        assert!(restored_checkpoint.verified);
        assert_eq!(restored_checkpoint.checkpoint_state_root, source.state_root);
        let state = verify_state(NodeOptions {
            data_dir: restored_dir.clone(),
        })
        .expect("restored state verifiers");
        assert!(state.verified, "{state:?}");
        for dir in [chain.data_dir, snapshot_dir, restored_dir] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    /// A lane chain with two applied releases (nonces 1 and 2).
    fn released_chain(label: &str) -> (Chain, u64) {
        let chain = lane_chain(label);
        for (sequence, nonce) in [(1_u64, 1_u64), (2, 2)] {
            let tx = release_tx(&chain, "base-erc20", nonce, ACCOUNT_RESERVE + 100 * nonce, sequence);
            let receipt = admit_and_apply(&chain, &tx, &format!("release-{nonce}"));
            assert!(receipt.accepted, "{receipt:?}");
        }
        let info = lane_info(&chain.data_dir);
        assert_eq!(info.state.released_lock_nonces, [1_u64, 2].into_iter().collect());
        assert_eq!(info.release_count, 2);
        let height = read_chain_tip_or_reconstruct_for_genesis(
            &NodeStore::new(&chain.data_dir),
            &chain.genesis,
        )
        .expect("tip")
        .height;
        (chain, height)
    }

    /// Exports `data_dir`, imports into a fresh directory, and checks that
    /// the restored node has the same lane record and state root.
    fn assert_snapshot_restores_lane(data_dir: &Path, label: &str) -> PathBuf {
        let source = status(NodeOptions {
            data_dir: data_dir.to_path_buf(),
        })
        .expect("source status");
        let snapshot_dir = sibling_dir(data_dir, &format!("{label}-snapshot"));
        let restored_dir = sibling_dir(data_dir, &format!("{label}-restored"));
        let manifest = export_snapshot(SnapshotExportOptions {
            data_dir: data_dir.to_path_buf(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect("export snapshot after releases");
        assert_eq!(manifest.state_root, source.state_root);
        let restored = import_snapshot(SnapshotImportOptions {
            data_dir: restored_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: Some("validator-restored".to_string()),
        })
        .expect("import snapshot after releases");
        assert_eq!(restored.state_root, source.state_root);
        let source_lane = lane_info(data_dir);
        let restored_lane = lane_info(&restored_dir);
        assert_eq!(restored_lane.state.reserved_atoms, LANE_RESERVED_ATOMS);
        assert_eq!(
            restored_lane.state.released_lock_nonces,
            [1_u64, 2].into_iter().collect()
        );
        assert_eq!(restored_lane, source_lane);
        verify_blocks(NodeOptions {
            data_dir: restored_dir.clone(),
        })
        .expect("restored node replays its blocks");
        let _ = fs::remove_dir_all(&snapshot_dir);
        restored_dir
    }

    #[test]
    fn external_lane_info_pages_releases_newest_first() {
        let (chain, _) = released_chain("release-paging");
        let all = lane_info(&chain.data_dir);
        let nonces = |row: &ExternalLaneInfoRow| {
            row.releases
                .iter()
                .map(|entry| (entry.sequence, entry.record.lock_nonce))
                .collect::<Vec<_>>()
        };
        assert_eq!(nonces(&all), vec![(1, 2), (0, 1)]);
        assert_eq!(all.next_release_cursor, None);
        let first = lane_page(&chain.data_dir, None, Some(1)).expect("first page");
        assert_eq!(nonces(&first), vec![(1, 2)]);
        assert_eq!(first.next_release_cursor, Some(1));
        let second = lane_page(&chain.data_dir, first.next_release_cursor, Some(1)).expect("second page");
        assert_eq!(nonces(&second), vec![(0, 1)]);
        assert_eq!(second.next_release_cursor, None);
        assert_eq!(first.state, second.state);
        for limit in [0, EXTERNAL_LANE_RELEASE_PAGE_SIZE + 1] {
            let error = lane_page(&chain.data_dir, None, Some(limit)).unwrap_err().to_string();
            assert!(error.contains("release limit must be between 1 and"), "{error}");
        }
        let _ = fs::remove_dir_all(&chain.data_dir);
    }

    #[test]
    fn full_and_checkpoint_snapshots_restore_the_released_lane() {
        let (chain, height) = released_chain("release-snapshot");
        // A full snapshot replays from genesis through both releases.
        let full = assert_snapshot_restores_lane(&chain.data_dir, "full");
        // A checkpoint snapshot carries the released lane in its checkpoint.
        let checkpoint = prune_through(&chain.data_dir, 1, height).expect("prune");
        assert_eq!(checkpoint.ledger.external_lanes[0].releases.len(), 2);
        let restored = assert_snapshot_restores_lane(&chain.data_dir, "checkpoint");
        let restored_checkpoint = read_history_checkpoint_state_optional(&NodeStore::new(&restored))
            .expect("restored checkpoint")
            .expect("snapshot carries the checkpoint");
        assert_eq!(restored_checkpoint.ledger.external_lanes, checkpoint.ledger.external_lanes);
        for dir in [chain.data_dir, full, restored] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    /// Tampers of a released lane that keep live + burns = supply: the reserve
    /// raised against genesis, released atoms with no record behind them, and
    /// the id set changed. Each pair is the edit and the refusal it must meet.
    fn released_lane_tampers() -> [(&'static str, LedgerEdit); 5] {
        [
            // A record changed behind unchanged totals: only the root binds it.
            ("release history root", |ledger| {
                ledger.external_lanes[0].releases[0].recipient =
                    "pfexternallane000000000000000000000009".to_string();
            }),
            ("reserves 300000005 atoms, but genesis reserves 300000000", |ledger| {
                ledger.external_lanes[0].reserved_atoms += 5;
                ledger.accounts[0].balance -= 5;
            }),
            ("but its recorded releases sum to", |ledger| {
                let lane = &mut ledger.external_lanes[0];
                lane.released_atoms += 5;
                lane.window_released_atoms += 5;
                ledger.accounts[0].balance += 5;
            }),
            ("differ from its recorded releases", |ledger| {
                ledger.external_lanes[0].released_lock_nonces.insert(9);
            }),
            ("differ from its recorded releases", |ledger| {
                let lane = &mut ledger.external_lanes[0];
                lane.released_lock_nonces.remove(&2);
                lane.released_lock_nonces.insert(3);
            }),
        ]
    }

    #[test]
    fn history_checkpoint_refuses_tampered_released_lane() {
        let (chain, height) = released_chain("release-tamper-checkpoint");
        let checkpoint = prune_through(&chain.data_dir, 1, height).expect("prune");
        for (needle, edit) in released_lane_tampers() {
            let (result, tampered) = reread_tampered_checkpoint(&chain.data_dir, &checkpoint, edit);
            assert_aggregate_balances(&tampered);
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains("external lane `base-erc20`") && error.contains(needle),
                "{needle}: {error}"
            );
        }
        let _ = fs::remove_dir_all(&chain.data_dir);
    }

    #[test]
    fn snapshot_import_refuses_tampered_released_lane() {
        let (chain, height) = released_chain("release-tamper-snapshot");
        prune_through(&chain.data_dir, 1, height).expect("prune");
        let snapshot_dir = sibling_dir(&chain.data_dir, "snapshot");
        let restored_dir = sibling_dir(&chain.data_dir, "restored");
        export_snapshot(SnapshotExportOptions {
            data_dir: chain.data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect("export snapshot");
        let ledger: LedgerState =
            serde_json::from_slice(&fs::read(snapshot_dir.join(LEDGER_FILE)).expect("ledger"))
                .expect("parse ledger");
        for (needle, edit) in released_lane_tampers() {
            let mut tampered = ledger.clone();
            edit(&mut tampered);
            rewrite_snapshot_file(
                &snapshot_dir,
                LEDGER_FILE,
                serde_json::to_vec_pretty(&tampered).expect("encode ledger"),
            );
            let error = import_snapshot(SnapshotImportOptions {
                data_dir: restored_dir.clone(),
                snapshot_dir: snapshot_dir.clone(),
                node_id: None,
            })
            .unwrap_err()
            .to_string();
            assert!(!restored_dir.exists());
            assert!(
                error.contains("snapshot ledger external lane `base-erc20`") && error.contains(needle),
                "{needle}: {error}"
            );
        }
        for dir in [chain.data_dir, snapshot_dir] {
            let _ = fs::remove_dir_all(dir);
        }
    }

    // ---- P2.5: the governed release cap on a local chain -------------------

    fn tip_height(chain: &Chain) -> u64 {
        read_chain_tip_or_reconstruct_for_genesis(&NodeStore::new(&chain.data_dir), &chain.genesis)
            .expect("tip")
            .height
    }

    fn faucet_sequence(chain: &Chain) -> u64 {
        let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
        let ledger = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger");
        ledger.account(&faucet.address).expect("faucet").sequence + 1
    }

    /// Ratifies, signs by all four validators, and applies a cap amendment
    /// through the governance round in the next block; returns its id and
    /// commit height.
    fn govern_release_cap(chain: &Chain, cap_bps: u32) -> (String, u64) {
        let validators = local_validator_ids(4).expect("validator ids");
        let (amendment, receipts) =
            try_govern_release_cap(chain, cap_bps, &validators, validators.len(), &validators)
                .expect("cap amendment admitted");
        assert!(receipts[0].accepted, "{receipts:?}");
        (amendment.amendment_id, tip_height(chain))
    }

    /// Ratifies a cap amendment declared by `validators` at `quorum`,
    /// supported and signed by `signers`, with the next block's slot as its
    /// activation height, and submits it through that block's governance
    /// round: the ratification, signing, assembly or admission error, or the
    /// block's receipts.
    fn try_govern_release_cap(
        chain: &Chain,
        cap_bps: u32,
        validators: &[String],
        quorum: usize,
        signers: &[String],
    ) -> io::Result<(GovernanceAmendment, Vec<Receipt>)> {
        let data_dir = &chain.data_dir;
        let slot = tip_height(chain) + 1;
        let tag = format!("{slot}-{cap_bps}-{quorum}-{}", signers.len());
        let amendment_file = data_dir.join(format!("cap-{tag}.unsigned.json"));
        let amendment = postfiat_consensus_cobalt::ratify_governance_amendment_with_lifecycle(
            &cobalt_domain(&chain.genesis),
            &EssentialSubsetConfig {
                validators: validators.to_vec(),
                quorum,
            },
            &postfiat_types::external_lane_release_cap_amendment_kind("base-erc20"),
            cap_bps,
            signers.to_vec(),
            GovernanceAmendmentLifecycle {
                activation_height: slot,
                veto_until_height: 0,
                paused: false,
            },
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        fs::write(&amendment_file, serde_json::to_vec_pretty(&amendment)?)?;
        let validator_keys =
            read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
        let expires_at_height = tip_height(chain) + 20;
        let mut authorization_files = Vec::new();
        for (validator, key_file) in write_split_validator_key_files(data_dir, &validator_keys) {
            if !signers.contains(&validator) {
                continue;
            }
            let authorization_file = data_dir.join(format!("{validator}.cap-{tag}.json"));
            sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                data_dir: data_dir.clone(),
                amendment_file: amendment_file.clone(),
                validator: validator.clone(),
                validator_key_file: key_file.clone(),
                proposal_slot: slot,
                expires_at_height,
                authorization_file: authorization_file.clone(),
            })?;
            authorization_files.push(authorization_file);
        }
        let signed_file = data_dir.join(format!("cap-{tag}.signed.json"));
        assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
            data_dir: data_dir.clone(),
            amendment_file,
            authorization_files,
            proposal_slot: slot,
            output_file: signed_file.clone(),
        })?;
        let batch_file = data_dir.join(format!("cap-{tag}.batch.json"));
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(signed_file),
            registry_update_file: None,
            batch_file: batch_file.clone(),
        })?;
        let receipts = apply_governance_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })?;
        Ok((amendment, receipts))
    }

    #[test]
    fn release_cap_amendment_needs_the_registry_threshold_of_the_active_set() {
        let chain = lane_chain_with_window("release-cap-threshold", 4);
        let validators = local_validator_ids(4).expect("validator ids");
        let threshold = postfiat_types::external_lane_release_cap_threshold(validators.len());
        assert_eq!(threshold, 3, "⌊2 × 4 / 3⌋ + 1");
        let without_last = validators[..3].to_vec();
        for (label, declared, quorum, signers) in [
            ("quorum 1, one signature", validators.clone(), 1, validators[..1].to_vec()),
            ("quorum below the threshold", validators.clone(), 2, validators[..2].to_vec()),
            ("a set without an active validator", without_last.clone(), 3, without_last.clone()),
        ] {
            let outcome = try_govern_release_cap(&chain, 0, &declared, quorum, &signers);
            match &outcome {
                Err(error) => assert!(
                    error.to_string().contains("external_lane_release_cap_authority_invalid")
                        || error.to_string().contains("does not match the old active registry"),
                    "{label}: {error}"
                ),
                Ok((_, receipts)) => panic!("{label}: cap amendment reached the block: {receipts:?}"),
            }
            let info = lane_info(&chain.data_dir);
            assert_eq!(info.release_cap.active_cap_bps, 200, "{label}: cap changed");
            assert_eq!(info.release_cap.active, None, "{label}");
        }
        let (amendment, receipts) =
            try_govern_release_cap(&chain, 0, &validators, threshold, &validators[..3])
                .expect("threshold admitted");
        assert!(receipts[0].accepted, "{receipts:?}");
        assert_eq!((amendment.quorum, amendment.support.len()), (3, 3));
        assert_eq!(lane_info(&chain.data_dir).release_cap.active_cap_bps, 0);
        let _ = fs::remove_dir_all(chain.data_dir);
    }

    /// The devnet's size: six validators, threshold five. Four signatures at
    /// quorum 4 are refused, five at quorum 5 lower the cap, and a zero cap
    /// signed by five of six, one validator offline, halts the next block's
    /// release.
    #[test]
    fn release_cap_amendment_on_six_validators_needs_five_signatures() {
        const THRESHOLD: usize = 5;
        let chain = lane_chain_with("release-cap-six", 4, None, 6);
        let validators = local_validator_ids(6).expect("validator ids");
        let lane = chain.genesis.external_lanes()[0].clone();

        // Quorum 4, four signatures: refused, the cap does not change.
        let below = THRESHOLD - 1;
        match try_govern_release_cap(&chain, 100, &validators, below, &validators[..below]) {
            Err(error) => {
                assert!(error.to_string().contains("external_lane_release_cap_authority_invalid"), "{error}")
            }
            Ok((_, receipts)) => panic!("quorum {below} reached the block: {receipts:?}"),
        }
        let info = lane_info(&chain.data_dir);
        assert_eq!((info.release_cap.active_cap_bps, info.release_cap.active), (200, None));

        // Quorum 5, five signatures: accepted, the lower cap applies at once.
        let (amendment, receipts) =
            try_govern_release_cap(&chain, 100, &validators, THRESHOLD, &validators[..THRESHOLD])
                .expect("five of six admitted");
        assert!(receipts[0].accepted, "{receipts:?}");
        assert_eq!((amendment.quorum, amendment.support.len()), (5, 5));
        let info = lane_info(&chain.data_dir);
        assert_eq!(
            info.release_cap.active,
            Some(ExternalLaneReleaseCapChange {
                amendment_id: amendment.amendment_id,
                cap_bps: 100,
                effective_height: tip_height(&chain),
            })
        );
        assert_eq!(info.window_cap_atoms, lane.window_cap_atoms_for_bps(100));

        // Emergency halt by five of six, validator-0 offline: the next block
        // refuses a release, and the lane does not change.
        let (halt, receipts) =
            try_govern_release_cap(&chain, 0, &validators, THRESHOLD, &validators[1..]).expect("halt admitted");
        assert!(receipts[0].accepted, "{receipts:?}");
        assert!(!halt.support.contains(&validators[0]));
        let info = lane_info(&chain.data_dir);
        assert_eq!((info.release_cap.active_cap_bps, info.window_cap_atoms), (0, 0));
        let release = release_tx(&chain, "base-erc20", 1, ACCOUNT_RESERVE + 100, faucet_sequence(&chain));
        refused_everywhere(&chain, &release, "external_lane_window_cap_exceeded", "six-halt");
        assert_eq!(lane_info(&chain.data_dir).state, info.state);
        assert_eq!(postfiat_types::external_lane_release_cap_threshold(validators.len()), THRESHOLD);
        let _ = fs::remove_dir_all(chain.data_dir);
    }

    /// A release above the cap that reaches a block: before the rejected
    /// transaction fee activates it changes nothing; after it, the lane and
    /// the Lock nonce still do not change, but the payer's fee is burned and
    /// its sequence consumed, under the `tec_` code.
    #[test]
    fn release_above_the_cap_in_a_block_burns_the_fee_only_after_fee_activation() {
        let chain = lane_chain("release-cap-fee");
        let lane = chain.genesis.external_lanes()[0].clone();
        let store = NodeStore::new(&chain.data_dir);
        let ledger = store.read_ledger().expect("ledger");
        let height = tip_height(&chain) + 1;
        let over = release_tx(&chain, "base-erc20", 1, lane.window_cap_atoms() + 1, faucet_sequence(&chain));
        let payer = over.unsigned.source.clone();
        let mut batch = TransactionBatch::new("release-cap-fee", Vec::new());
        batch.asset_transactions.push(over);
        let unactivated = store.read_governance().expect("governance");
        let mut activated = unactivated.clone();
        let template = activated.amendments[0].clone();
        activated.apply(GovernanceAmendment {
            amendment_id: "rejected-fee-activation".to_string(),
            kind: GOVERNANCE_KIND_REJECTED_TX_FEE_ACTIVATION_HEIGHT.to_string(),
            value: u32::try_from(height).expect("height"),
            ..template
        });
        for (governance, code, burned) in [
            (unactivated, "external_lane_window_cap_exceeded", 0),
            (activated, "tec_external_lane_window_cap_exceeded", 1_000),
        ] {
            let compatibility =
                asset_execution_compatibility_for_genesis_and_governance(&chain.genesis, &governance);
            let mut after = ledger.clone();
            let receipts = execute_transparent_batch(
                &chain.genesis,
                &governance,
                &mut after,
                &batch,
                height,
                compatibility,
            );
            assert_eq!((receipts[0].code.as_str(), receipts[0].fee_burned), (code, burned), "{receipts:?}");
            assert_eq!(after.external_lanes, ledger.external_lanes, "{code}: lane or Lock nonce changed");
            let (was, now) = (ledger.account(&payer).expect("payer"), after.account(&payer).expect("payer"));
            assert_eq!(now.balance, was.balance - burned, "{code}");
            assert_eq!(now.sequence, was.sequence + u64::from(burned > 0), "{code}");
        }
        let _ = fs::remove_dir_all(chain.data_dir);
    }

    #[test]
    fn a_fresh_halt_after_a_pending_restore_is_applied_and_cancels_the_raise() {
        // Windows of 4 blocks: a raise waits 7 × 4 = 28 blocks.
        let chain = lane_chain_with_window("release-cap-halt", 4);
        let (halt_id, _) = govern_release_cap(&chain, 0);
        assert_eq!(lane_info(&chain.data_dir).release_cap.active_cap_bps, 0);
        let (restore_id, restore_height) = govern_release_cap(&chain, 200);
        let info = lane_info(&chain.data_dir);
        assert_eq!(info.release_cap.active_cap_bps, 0);
        assert_eq!(info.release_cap.pending.map(|change| change.amendment_id), Some(restore_id));

        // The same halt again, in a later slot: a new amendment.
        let (fresh_id, fresh_height) = govern_release_cap(&chain, 0);
        assert_ne!(fresh_id, halt_id);
        let info = lane_info(&chain.data_dir);
        assert_eq!(
            info.release_cap.active,
            Some(ExternalLaneReleaseCapChange {
                amendment_id: fresh_id,
                cap_bps: 0,
                effective_height: fresh_height,
            })
        );
        assert_eq!(info.release_cap.pending, None);

        // Past the cancelled raise's effective height the lane stays halted.
        let mut filler = 0;
        while tip_height(&chain) < restore_height + 7 * 4 {
            filler += 1;
            apply_one_transfer(&chain.data_dir, &format!("pfexternallanehalt{filler:020}"));
        }
        assert_eq!(lane_info(&chain.data_dir).release_cap.active_cap_bps, 0);
        let release = release_tx(&chain, "base-erc20", 1, ACCOUNT_RESERVE + 100, faucet_sequence(&chain));
        refused_everywhere(&chain, &release, "external_lane_window_cap_exceeded", "halt-held");
        let _ = fs::remove_dir_all(chain.data_dir);
    }

    #[test]
    fn governed_release_cap_lowers_at_once_and_raises_after_seven_windows() {
        // Windows of 4 blocks: a raise waits 7 × 4 = 28 blocks.
        let chain = lane_chain_with_window("release-cap", 4);
        let lane = chain.genesis.external_lanes()[0].clone();
        let lowered = lane.window_cap_atoms_for_bps(100);
        let raised = lane.window_cap_atoms_for_bps(200);
        assert_eq!(lane.window_cap_atoms(), raised);

        // 1. Lowered to 100 bps: above it is refused, below it released.
        let (lower_id, lower_height) = govern_release_cap(&chain, 100);
        let info = lane_info(&chain.data_dir);
        assert_eq!(info.release_cap.active_cap_bps, 100);
        assert_eq!(
            info.release_cap.active,
            Some(ExternalLaneReleaseCapChange {
                amendment_id: lower_id.clone(),
                cap_bps: 100,
                effective_height: lower_height,
            })
        );
        assert_eq!(info.release_cap.pending, None);
        assert_eq!(info.window_cap_atoms, lowered);
        let lane_before = info.state;
        let over = release_tx(&chain, "base-erc20", 1, lowered + 1, faucet_sequence(&chain));
        refused_everywhere(&chain, &over, "external_lane_window_cap_exceeded", "cap-over");
        assert_eq!(lane_info(&chain.data_dir).state, lane_before);
        let below = release_tx(&chain, "base-erc20", 1, ACCOUNT_RESERVE + 100, faucet_sequence(&chain));
        let receipt = admit_and_apply(&chain, &below, "cap-below");
        assert!(receipt.accepted, "{receipt:?}");

        // 2. Raised back to 200 bps: a release at the raised cap waits.
        let (raise_id, raise_height) = govern_release_cap(&chain, 200);
        let effective = raise_height + 7 * 4;
        let info = lane_info(&chain.data_dir);
        assert_eq!(info.release_cap.active_cap_bps, 100);
        assert_eq!(info.release_cap.active.as_ref().map(|c| c.amendment_id.as_str()), Some(lower_id.as_str()));
        assert_eq!(
            info.release_cap.pending,
            Some(ExternalLaneReleaseCapChange {
                amendment_id: raise_id.clone(),
                cap_bps: 200,
                effective_height: effective,
            })
        );
        let at_raised = |chain: &Chain| release_tx(chain, "base-erc20", 2, raised, faucet_sequence(chain));
        refused_everywhere(&chain, &at_raised(&chain), "external_lane_window_cap_exceeded", "cap-early");
        let mut filler = 0;
        while tip_height(&chain) + 1 < effective {
            if tip_height(&chain) + 2 == effective {
                // The last block before the delay ends holds the lowered cap.
                refused_everywhere(&chain, &at_raised(&chain), "external_lane_window_cap_exceeded", "cap-last");
            }
            filler += 1;
            apply_one_transfer(&chain.data_dir, &format!("pfexternallanecap{filler:020}"));
        }
        // The next block is the first under the raised cap.
        assert_eq!(tip_height(&chain) + 1, effective);
        let info = lane_info(&chain.data_dir);
        assert_eq!(info.release_cap.active_cap_bps, 200);
        assert_eq!(info.release_cap.active.map(|c| c.amendment_id), Some(raise_id));
        assert_eq!(info.release_cap.pending, None);
        let receipt = admit_and_apply(&chain, &at_raised(&chain), "cap-after");
        assert!(receipt.accepted, "{receipt:?}");

        // Replay from genesis and snapshot export/import reproduce the lane.
        verify_blocks(NodeOptions {
            data_dir: chain.data_dir.clone(),
        })
        .expect("replay from genesis through the amended caps");
        let restored = assert_snapshot_restores_lane(&chain.data_dir, "release-cap");
        assert_eq!(lane_info(&restored).release_cap, lane_info(&chain.data_dir).release_cap);

        // The lane rule reads the amendments: without the raise, the last
        // window is above the lowered cap.
        let store = NodeStore::new(&chain.data_dir);
        let ledger = store.read_ledger().expect("ledger");
        let mut governance = store.read_governance().expect("governance");
        validate_external_lanes_match_genesis(&chain.genesis, &ledger, &governance, "governed")
            .expect("windows within the governed caps");
        governance.amendments.retain(|amendment| amendment.value != 200);
        let error =
            validate_external_lanes_match_genesis(&chain.genesis, &ledger, &governance, "unraised")
                .unwrap_err()
                .to_string();
        assert!(error.contains(&format!("above the cap {lowered}")), "{error}");
        for dir in [chain.data_dir, restored] {
            let _ = fs::remove_dir_all(dir);
        }
    }
}
