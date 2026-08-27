use super::*;

fn copy_activation_test_dir(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create activation recovery directory");
    for entry in std::fs::read_dir(source).expect("read activation seed directory") {
        let entry = entry.expect("read activation seed entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry
            .file_type()
            .expect("read activation seed entry type")
            .is_dir()
        {
            copy_activation_test_dir(&source_path, &destination_path);
        } else {
            std::fs::copy(&source_path, &destination_path)
                .expect("copy activation seed entry");
        }
    }
}

fn activation_test_journal<T: serde::Serialize>(
    store: &NodeStore,
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    batch_kind: &str,
    batch_id: &str,
    payload: &T,
    receipts: &[Receipt],
    write_governance: bool,
) -> OrderedCommitDeltaJournal {
    let shielded = store.read_shielded().expect("read activation shielded state");
    let bridge = store.read_bridge().expect("read activation bridge state");
    let tip = store.read_chain_tip().expect("read activation chain tip");
    let block_height = tip.height.checked_add(1).expect("activation height overflow");
    let (proposed_ordered_batches, ordered_history) =
        proposed_ordered_state(store, genesis, batch_id, block_height)
            .expect("build activation ordered history");
    let mut ordered_batches = proposed_ordered_batches;
    if ordered_history.is_none() {
        ordered_batches.pop();
    } else {
        ordered_batches.clear();
    }
    let validator_keys = read_validator_key_file(&store.data_dir().join(VALIDATOR_KEYS_FILE))
        .expect("read activation validator keys");
    let certificate_validators =
        active_validator_ids(governance).expect("read activation validator ids");
    let commit = prepare_ordered_commit(OrderedCommitPlan {
        genesis,
        governance,
        ledger,
        ordered_batches: &ordered_batches,
        ordered_history: ordered_history.as_ref(),
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash: tip.block_hash,
        batch_kind,
        batch_id,
        payload,
        batch_receipts: receipts,
        archived_payload_json: None,
        validator_keys: Some(&validator_keys),
        external_certificate: None,
        external_validator_registry: None,
        external_certificate_preverified: false,
        historical_replay: None,
        certificate_validators: &certificate_validators,
        fastpay_pre_state_effects: &[],
    })
    .expect("prepare activation ordered commit");
    ordered_commit_delta_journal(OrderedCommitWrite {
        ledger: Some(ledger.clone()),
        governance: write_governance.then(|| governance.clone()),
        shielded: None,
        bridge: None,
        commit,
        validator_registry: None,
    })
    .expect("build activation ordered commit journal")
}

fn assert_activation_journal_recovers_every_prefix(
    seed_dir: &Path,
    label: &str,
    journal: &OrderedCommitDeltaJournal,
) {
    let seed_store = NodeStore::new(seed_dir);
    let pre_receipts = seed_store.read_receipts().expect("read seed receipts");
    let pre_ordered = seed_store
        .read_ordered_batches()
        .expect("read seed ordered batches");
    let pre_archive = seed_store.read_batch_archive().expect("read seed archive");
    let pre_blocks = seed_store.read_blocks().expect("read seed blocks");
    let pre_tip = seed_store.read_chain_tip().expect("read seed chain tip");

    let mut expected_receipts = pre_receipts;
    expected_receipts.extend(journal.receipt_delta.clone());
    let mut expected_ordered = pre_ordered;
    expected_ordered.push(journal.ordered_batch_id.clone());
    let mut expected_archive = pre_archive;
    expected_archive.batches.push(journal.archive_entry.clone());
    let mut expected_blocks = pre_blocks;
    expected_blocks.blocks.push(journal.block.clone());
    let expected_tip = chain_tip_after_delta(&pre_tip, journal).expect("activation expected tip");

    for write_prefix in 0..=9 {
        let data_dir = unique_test_dir(&format!(
            "postfiat-replicated-state-v2-{label}-recovery-{write_prefix}"
        ));
        copy_activation_test_dir(seed_dir, &data_dir);
        let store = NodeStore::new(&data_dir);
        store
            .write_ordered_commit_journal(journal)
            .expect("write activation recovery journal");
        if write_prefix >= 1 {
            if let Some(ledger) = &journal.ledger {
                store.write_ledger(ledger).expect("write activation ledger");
            }
        }
        if write_prefix >= 2 {
            if let Some(governance) = &journal.governance {
                store
                    .write_governance(governance)
                    .expect("write activation governance");
            }
        }
        if write_prefix >= 3 {
            if let Some(shielded) = &journal.shielded {
                store
                    .write_shielded(shielded)
                    .expect("write activation shielded state");
            }
        }
        if write_prefix >= 4 {
            if let Some(bridge) = &journal.bridge {
                store.write_bridge(bridge).expect("write activation bridge");
            }
        }
        if write_prefix >= 5 {
            for receipt in &journal.receipt_delta {
                store
                    .append_receipt_record(receipt)
                    .expect("write activation receipt");
            }
        }
        if write_prefix >= 6 {
            store
                .append_ordered_batch_record(&journal.ordered_batch_id)
                .expect("write activation ordered batch");
        }
        if write_prefix >= 7 {
            store
                .append_batch_archive_entry(journal.archive_entry.clone())
                .expect("write activation archive entry");
        }
        if write_prefix >= 8 {
            store
                .append_block_record(&journal.block)
                .expect("write activation block");
        }
        if write_prefix >= 9 {
            store
                .write_chain_tip(&expected_tip)
                .expect("write activation chain tip");
        }

        status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("recover activation journal");
        assert_eq!(
            store.read_ledger().expect("read recovered ledger"),
            journal.ledger.clone().expect("activation journal ledger")
        );
        if let Some(governance) = &journal.governance {
            assert_eq!(
                store
                    .read_governance()
                    .expect("read recovered governance"),
                *governance
            );
        }
        assert_eq!(
            store.read_receipts().expect("read recovered receipts"),
            expected_receipts
        );
        assert_eq!(
            store
                .read_ordered_batches()
                .expect("read recovered ordered batches"),
            expected_ordered
        );
        assert_eq!(
            store.read_batch_archive().expect("read recovered archive"),
            expected_archive
        );
        assert_eq!(
            store.read_blocks().expect("read recovered blocks"),
            expected_blocks
        );
        assert_eq!(
            store.read_chain_tip().expect("read recovered tip"),
            expected_tip
        );
        assert!(
            store
                .read_ordered_commit_journal_raw()
                .expect("read recovered activation journal")
                .is_none(),
            "recovery must remove the activation journal"
        );
        let verified = verify_state(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify recovered activation state");
        assert!(verified.verified);
        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify recovered activation blocks");
        std::fs::remove_dir_all(data_dir).expect("remove activation recovery directory");
    }
}

fn apply_activation_journal(store: &NodeStore, journal: &OrderedCommitDeltaJournal) {
    store
        .write_ordered_commit_journal(journal)
        .expect("write complete activation journal");
    status(NodeOptions {
        data_dir: store.data_dir().to_path_buf(),
    })
    .expect("apply complete activation journal");
}

#[test]
fn replicated_state_v2_activation_journal_recovers_every_persist_prefix() {
    let seed_dir = unique_test_dir("postfiat-replicated-state-v2-activation-seed");
    init(InitOptions {
        data_dir: seed_dir.clone(),
        chain_id: "postfiat-replicated-state-v2-activation".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize activation seed");
    let store = NodeStore::new(&seed_dir);
    let mut genesis = store.read_genesis().expect("read activation genesis");
    genesis.replicated_state_v2_activation_height = None;
    store
        .write_genesis(&genesis)
        .expect("write legacy activation genesis");

    let mut ledger = store.read_ledger().expect("read activation ledger");
    let mut genesis_tip = store.read_chain_tip().expect("read activation genesis tip");
    genesis_tip.genesis_hash = genesis_hash(&genesis);
    genesis_tip.state_root =
        current_replicated_state_root(&store, &genesis).expect("legacy activation genesis root");
    store
        .write_chain_tip(&genesis_tip)
        .expect("write legacy activation genesis tip");

    let mut governance = store.read_governance().expect("read activation governance");
    let amendment = ratify_governance(RatifyGovernanceOptions {
        data_dir: seed_dir.clone(),
        validators: vec!["validator-0".to_string()],
        support: vec!["validator-0".to_string()],
        kind: GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT.to_string(),
        value: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: seed_dir.join("replicated-state-v2-amendment.json"),
    })
    .expect("ratify activation amendment fixture");
    let amendment_batch =
        build_governance_action_batch(&genesis, vec![amendment], Vec::new())
            .expect("build activation amendment batch");
    let amendment_receipts =
        execute_governance_batch(&mut governance, Some(&mut ledger), &amendment_batch, 1);
    assert_eq!(amendment_receipts.len(), 1);
    assert!(amendment_receipts[0].accepted, "{amendment_receipts:?}");
    let amendment_journal = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_GOVERNANCE,
        &amendment_batch.batch_id,
        &amendment_batch,
        &amendment_receipts,
        true,
    );
    assert_eq!(amendment_journal.block.header.height, 1);
    assert_activation_journal_recovers_every_prefix(
        &seed_dir,
        "amendment",
        &amendment_journal,
    );
    apply_activation_journal(&store, &amendment_journal);

    let ordered_before_activation = store
        .read_ordered_batches()
        .expect("read pre-activation ordered batches");
    assert_eq!(
        replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_before_activation,
            &store.read_shielded().expect("read pre-activation shielded"),
            &store.read_bridge().expect("read pre-activation bridge"),
        )
        .expect("pre-activation state root"),
        amendment_journal.block.header.state_root,
        "height 1 must replay the historical omitted-field root"
    );

    let activation_batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build activation-height empty batch")
    .batch;
    let activation_journal = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &activation_batch.batch_id,
        &activation_batch,
        &[],
        false,
    );
    assert_eq!(activation_journal.block.header.height, 2);
    let mut ordered_at_activation = ordered_before_activation;
    ordered_at_activation.push(activation_batch.batch_id.clone());
    let shielded = store.read_shielded().expect("read activation shielded");
    let bridge = store.read_bridge().expect("read activation bridge");
    assert_eq!(
        replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_at_activation,
            &shielded,
            &bridge,
        )
        .expect("activation state root"),
        activation_journal.block.header.state_root,
        "height 2 must use the scheduled v2 commitment"
    );
    assert_activation_journal_recovers_every_prefix(
        &seed_dir,
        "first-v2-block",
        &activation_journal,
    );
    apply_activation_journal(&store, &activation_journal);
    assert_eq!(
        store.read_chain_tip().expect("read activated chain tip").state_root,
        activation_journal.block.header.state_root
    );
    verify_blocks(NodeOptions {
        data_dir: seed_dir.clone(),
    })
    .expect("replay activation boundary");
    std::fs::remove_dir_all(seed_dir).expect("remove activation seed directory");
}

#[test]
fn ordered_history_v2_activation_journal_recovers_every_persist_prefix() {
    let seed_dir = unique_test_dir("postfiat-ordered-history-v2-activation-seed");
    let mut genesis = Genesis::try_new_with_validator_count(
        "postfiat-ordered-history-v2-activation",
        1,
    )
    .expect("create ordered-history v2 genesis");
    genesis.ordered_history_v2_activation_height = Some(1);
    genesis.validate().expect("validate ordered-history v2 genesis");
    lifecycle_queries::init_with_genesis(
        seed_dir.clone(),
        "validator-0".to_string(),
        genesis.clone(),
    )
    .expect("initialize ordered-history v2 seed");
    let store = NodeStore::new(&seed_dir);
    assert_eq!(
        store
            .transactional_store()
            .expect("open transactional store")
            .meta()
            .expect("read genesis ordered-history commitment")
            .ordered_history_commitment()
            .count,
        0
    );
    let governance = store.read_governance().expect("read governance");
    let ledger = store.read_ledger().expect("read ledger");
    let batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build ordered-history activation batch")
    .batch;
    let journal = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &batch.batch_id,
        &batch,
        &[],
        false,
    );
    assert_eq!(journal.block.header.height, 1);
    let (proposal_ordered_batches, proposal_ordered_history) =
        proposed_ordered_state(&store, &genesis, &batch.batch_id, 1)
            .expect("build ordered-history v2 proposal state");
    assert!(proposal_ordered_batches.is_empty());
    let proposal = build_block_proposal_from_state(BlockProposalPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &proposal_ordered_batches,
        ordered_history: proposal_ordered_history.as_ref(),
        shielded: &store.read_shielded().expect("read shielded state"),
        bridge: &store.read_bridge().expect("read bridge state"),
        block_height: 1,
        parent_hash: store.read_chain_tip().expect("read genesis tip").block_hash,
        view: 0,
        batch_kind: BATCH_KIND_TRANSPARENT,
        batch_id: &batch.batch_id,
        payload: &batch,
        receipts: &[],
        fastpay_pre_state_effects: Vec::new(),
    })
    .expect("build ordered-history v2 proposal");
    assert_eq!(proposal.state_root, journal.block.header.state_root);
    assert_activation_journal_recovers_every_prefix(
        &seed_dir,
        "ordered-history-v2-first-block",
        &journal,
    );
    apply_activation_journal(&store, &journal);
    assert_eq!(
        store
            .transactional_store()
            .expect("open transactional store")
            .meta()
            .expect("read activated ordered-history commitment")
            .ordered_history_commitment()
            .count,
        1
    );
    assert_eq!(
        current_replicated_state_root(&store, &genesis)
            .expect("read activated current state root"),
        journal.block.header.state_root
    );
    verify_blocks(NodeOptions {
        data_dir: seed_dir.clone(),
    })
    .expect("replay ordered-history v2 activation boundary");
    std::fs::remove_dir_all(seed_dir).expect("remove ordered-history v2 activation seed");
}

#[test]
fn ordered_history_v2_active_commit_uses_one_database_transaction_without_jsonl() {
    let data_dir = unique_test_dir("postfiat-ordered-history-v2-transactional-commit");
    let mut genesis = Genesis::try_new_with_validator_count(
        "postfiat-ordered-history-v2-transactional-commit",
        1,
    )
    .expect("create ordered-history v2 genesis");
    genesis.ordered_history_v2_activation_height = Some(1);
    lifecycle_queries::init_with_genesis(
        data_dir.clone(),
        "validator-0".to_owned(),
        genesis.clone(),
    )
    .expect("initialize transactional seed");
    let store = NodeStore::new(&data_dir);
    let governance = store.read_governance().expect("read governance");
    let ledger = store.read_ledger().expect("read ledger");
    let batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build activation batch")
    .batch;
    let journal = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &batch.batch_id,
        &batch,
        &[],
        false,
    );
    let legacy_blocks_before = std::fs::read(data_dir.join(BLOCKS_FILE)).expect("read legacy blocks");
    let legacy_ordered_before =
        std::fs::read(data_dir.join(ORDERED_BATCHES_FILE)).expect("read legacy ordered batches");
    store
        .transactional_store()
        .expect("open transactional store")
        .reset_work_counters();
    let commit_lock = store.lock_ordered_commit().expect("lock ordered commit");
    write_ordered_commit_with_journal_locked(
        &store,
        &commit_lock,
        OrderedCommitWrite {
            ledger: journal.ledger.clone(),
            governance: journal.governance.clone(),
            shielded: journal.shielded.clone(),
            bridge: journal.bridge.clone(),
            commit: OrderedCommitArtifacts {
                height: journal.height,
                receipt_delta: journal.receipt_delta.clone(),
                ordered_batch_id: journal.ordered_batch_id.clone(),
                archive_entry: journal.archive_entry.clone(),
                block: journal.block.clone(),
            },
            validator_registry: journal.validator_registry.clone(),
        },
    )
    .expect("commit one transactional height");
    drop(commit_lock);

    assert_eq!(
        std::fs::read(data_dir.join(BLOCKS_FILE)).expect("reread legacy blocks"),
        legacy_blocks_before,
        "active commits must not rewrite the legacy block store"
    );
    assert_eq!(
        std::fs::read(data_dir.join(ORDERED_BATCHES_FILE))
            .expect("reread legacy ordered batches"),
        legacy_ordered_before,
        "active commits must not rewrite the legacy ordered list"
    );
    assert!(!data_dir.join(BLOCKS_APPEND_FILE).exists());
    assert!(!data_dir
        .join(postfiat_storage::ORDERED_COMMIT_JOURNAL_FILE)
        .exists());
    let transactional = store
        .transactional_store()
        .expect("open transactional store");
    let commit_work = transactional.work_counters();
    assert_eq!(
        commit_work.committed_write_transactions, 1,
        "one active height must use one durable database commit"
    );
    assert_eq!(commit_work.full_history_scans, 0);
    assert_eq!(commit_work.full_history_records_read, 0);
    assert_eq!(commit_work.full_history_bytes_read, 0);
    assert!(
        commit_work.page_reads <= 24,
        "unexpected active read work: {commit_work:?}"
    );
    assert!(
        commit_work.page_writes <= 16,
        "unexpected active write work: {commit_work:?}"
    );

    assert_eq!(store.read_chain_tip().expect("read active tip").height, 1);
    assert_eq!(store.read_blocks().expect("read active blocks").blocks.len(), 1);
    assert_eq!(
        transactional
            .verify_logical_integrity()
            .expect("verify transactional store")
            .finalized_height,
        1
    );

    let snapshot_dir = unique_test_dir("postfiat-new-chain-storage-snapshot");
    let restored_dir = unique_test_dir("postfiat-new-chain-storage-snapshot-restore");
    let snapshot = export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export new-chain transactional snapshot");
    let restored = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: Some("validator-restored".to_owned()),
    })
    .expect("import new-chain transactional snapshot");
    assert_eq!(restored.block_height, snapshot.block_height);
    assert_eq!(restored.block_tip_hash, snapshot.block_tip_hash);
    assert_eq!(restored.state_root, snapshot.state_root);
    let restored_store = NodeStore::new(&restored_dir);
    assert!(restored_store
        .transactional_storage_active()
        .expect("restored new-chain snapshot activation status"));
    assert_eq!(
        restored_store
            .transactional_store()
            .expect("open restored new-chain transactional store")
            .verify_logical_integrity()
            .expect("verify restored new-chain transactional store")
            .finalized_height,
        1
    );

    std::fs::remove_dir_all(data_dir).expect("remove transactional test directory");
    std::fs::remove_dir_all(snapshot_dir).expect("remove transactional snapshot");
    std::fs::remove_dir_all(restored_dir).expect("remove transactional snapshot restore");
}

#[test]
fn ordered_history_v2_shadow_stays_exact_then_activation_stops_legacy_appends() {
    let data_dir = unique_test_dir("postfiat-ordered-history-v2-shadow-activation");
    let mut genesis =
        Genesis::try_new_with_validator_count("postfiat-ordered-history-v2-shadow", 1)
            .expect("create ordered-history v2 genesis");
    genesis.ordered_history_v2_activation_height = Some(2);
    lifecycle_queries::init_with_genesis(
        data_dir.clone(),
        "validator-0".to_owned(),
        genesis.clone(),
    )
    .expect("initialize shadow seed");
    let store = NodeStore::new(&data_dir);
    let governance = store.read_governance().expect("read governance");
    let ledger = store.read_ledger().expect("read ledger");

    let first = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        "shadow-batch-1",
        &serde_json::json!({"height": 1}),
        &[],
        false,
    );
    let commit_lock = store.lock_ordered_commit().expect("lock first commit");
    write_ordered_commit_with_journal_locked(
        &store,
        &commit_lock,
        OrderedCommitWrite {
            ledger: first.ledger.clone(),
            governance: first.governance.clone(),
            shielded: first.shielded.clone(),
            bridge: first.bridge.clone(),
            commit: OrderedCommitArtifacts {
                height: first.height,
                receipt_delta: first.receipt_delta.clone(),
                ordered_batch_id: first.ordered_batch_id.clone(),
                archive_entry: first.archive_entry.clone(),
                block: first.block.clone(),
            },
            validator_registry: None,
        },
    )
    .expect("commit shadow height");
    drop(commit_lock);
    assert_eq!(store.read_chain_tip().expect("legacy tip").height, 1);
    assert_eq!(
        store
            .transactional_store()
            .expect("open transactional store")
            .meta()
            .expect("shadow metadata")
            .finalized_height,
        1
    );
    assert!(data_dir.join(BLOCKS_APPEND_FILE).exists());
    let legacy_append_after_shadow =
        std::fs::read(data_dir.join(BLOCKS_APPEND_FILE)).expect("read legacy append");

    let second = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        "active-batch-2",
        &serde_json::json!({"height": 2}),
        &[],
        false,
    );
    let commit_lock = store.lock_ordered_commit().expect("lock active commit");
    write_ordered_commit_with_journal_locked(
        &store,
        &commit_lock,
        OrderedCommitWrite {
            ledger: second.ledger.clone(),
            governance: second.governance.clone(),
            shielded: second.shielded.clone(),
            bridge: second.bridge.clone(),
            commit: OrderedCommitArtifacts {
                height: second.height,
                receipt_delta: second.receipt_delta.clone(),
                ordered_batch_id: second.ordered_batch_id.clone(),
                archive_entry: second.archive_entry.clone(),
                block: second.block.clone(),
            },
            validator_registry: None,
        },
    )
    .expect("commit activation height");
    drop(commit_lock);

    assert_eq!(store.read_chain_tip().expect("active tip").height, 2);
    assert_eq!(
        store.read_ordered_batches().expect("active ordered batches"),
        vec!["shadow-batch-1".to_owned(), "active-batch-2".to_owned()]
    );
    assert_eq!(
        std::fs::read(data_dir.join(BLOCKS_APPEND_FILE)).expect("reread legacy append"),
        legacy_append_after_shadow,
        "activation height must not append to the legacy block log"
    );
    assert_eq!(
        store
            .transactional_store()
            .expect("open transactional store")
            .verify_logical_integrity()
            .expect("verify activated store")
            .finalized_height,
        2
    );
    std::fs::remove_dir_all(data_dir).expect("remove shadow activation directory");
}

#[test]
fn ordered_history_v2_state_root_is_domain_bound_and_activation_gated() {
    let data_dir = unique_test_dir("postfiat-ordered-history-v2-state-root");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-ordered-history-v2-state-root".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize ordered-history v2 fixture");
    let store = NodeStore::new(&data_dir);
    let mut genesis = store.read_genesis().expect("read genesis");
    genesis.ordered_history_v2_activation_height = Some(1);
    let genesis_hash_hex = genesis_hash(&genesis);
    let empty = postfiat_storage::OrderedHistoryCommitment::genesis(
        &genesis.chain_id,
        &genesis_hash_hex,
        genesis.protocol_version,
    )
    .expect("create empty ordered-history commitment");
    let first = empty.append("batch-1").expect("append first batch");
    let second = first.append("batch-2").expect("append second batch");
    let governance = store.read_governance().expect("read governance");
    let ledger = store.read_ledger().expect("read ledger");
    let shielded = store.read_shielded().expect("read shielded state");
    let bridge = store.read_bridge().expect("read bridge state");

    let first_root = replicated_state_root_v2(
        &genesis,
        &governance,
        &ledger,
        &first,
        &shielded,
        &bridge,
    )
    .expect("compute first ordered-history v2 root");
    assert_eq!(
        first_root,
        replicated_state_root_v2(
            &genesis,
            &governance,
            &ledger,
            &first,
            &shielded,
            &bridge,
        )
        .expect("recompute first ordered-history v2 root")
    );
    assert_ne!(
        first_root,
        replicated_state_root_v2(
            &genesis,
            &governance,
            &ledger,
            &second,
            &shielded,
            &bridge,
        )
        .expect("compute second ordered-history v2 root")
    );
    assert!(replicated_state_root_v2(
        &genesis,
        &governance,
        &ledger,
        &empty,
        &shielded,
        &bridge,
    )
    .is_err());

    let wrong_domain = postfiat_storage::OrderedHistoryCommitment::genesis(
        "postfiat-wrong-domain",
        &genesis_hash_hex,
        genesis.protocol_version,
    )
    .expect("create wrong-domain commitment")
    .append("batch-1")
    .expect("append wrong-domain batch");
    assert!(replicated_state_root_v2(
        &genesis,
        &governance,
        &ledger,
        &wrong_domain,
        &shielded,
        &bridge,
    )
    .is_err());
    std::fs::remove_dir_all(data_dir).expect("remove ordered-history v2 fixture");
}

#[test]
fn transactional_rebuild_replays_publishes_and_verifies_a_legacy_generation() {
    let data_dir = unique_test_dir("postfiat-transactional-rebuild-source");
    let output_dir = unique_test_dir("postfiat-transactional-rebuild-output");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-transactional-rebuild-test".to_owned(),
        node_id: "validator-0".to_owned(),
        validator_count: 1,
    })
    .expect("initialize rebuild source");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("read rebuild genesis");
    let governance = store.read_governance().expect("read rebuild governance");
    let ledger = store.read_ledger().expect("read rebuild ledger");
    let batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build rebuild source batch")
    .batch;
    let journal = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &batch.batch_id,
        &batch,
        &[],
        false,
    );
    apply_activation_journal(&store, &journal);
    let source_tip = store.read_chain_tip().expect("read rebuild source tip");

    let report = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: false,
    })
    .expect("rebuild transactional generation");
    assert!(report.published);
    assert_eq!(report.source_tip, source_tip);
    assert_eq!(report.logical_store_report.finalized_height, 1);
    assert!(
        data_dir
            .join(postfiat_storage::transactional::TRANSACTIONAL_GENERATION_POINTER_FILE)
            .is_file()
    );

    let verified = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect("verify rebuilt transactional generation");
    assert!(verified.verify_only);
    assert!(!verified.published);
    assert_eq!(verified.migration_packet_root, report.migration_packet_root);
    assert_eq!(
        store
            .transactional_store()
            .expect("open published transactional generation")
            .meta()
            .expect("read published metadata")
            .chain_tip(source_tip.schema.clone()),
        source_tip
    );

    let manifest_path = output_dir.join(STORAGE_MIGRATION_MANIFEST_FILE);
    let original_manifest = std::fs::read(&manifest_path).expect("read migration manifest");
    let mut tampered_manifest: StorageMigrationManifestV1 =
        serde_json::from_slice(&original_manifest).expect("decode migration manifest");
    tampered_manifest.block_count = tampered_manifest.block_count.saturating_add(1);
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&tampered_manifest).expect("encode tampered migration manifest"),
    )
    .expect("write tampered migration manifest");
    let manifest_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("tampered migration manifest must reject");
    assert!(manifest_error
        .to_string()
        .contains("storage_migration_manifest_invalid"));
    std::fs::write(&manifest_path, original_manifest).expect("restore migration manifest");

    let pointer_path = data_dir.join(
        postfiat_storage::transactional::TRANSACTIONAL_GENERATION_POINTER_FILE,
    );
    let mut pointer_bytes = std::fs::read(&pointer_path).expect("read generation pointer");
    let pointer_byte = pointer_bytes
        .iter_mut()
        .find(|byte| **byte == b'g')
        .expect("find pointer byte to tamper");
    *pointer_byte = b'G';
    std::fs::write(&pointer_path, pointer_bytes).expect("write tampered generation pointer");
    let pointer_error = NodeStore::new(&data_dir)
        .transactional_generation_pointer()
        .expect_err("tampered generation pointer must reject");
    assert_eq!(pointer_error.kind(), io::ErrorKind::InvalidData, "{pointer_error}");

    std::fs::remove_dir_all(data_dir).expect("remove rebuild source");
    std::fs::remove_dir_all(output_dir).expect("remove rebuild output");
}

#[test]
fn existing_chain_governance_schedule_switches_only_at_the_recorded_height() {
    let data_dir = unique_test_dir("postfiat-existing-chain-storage-activation");
    let output_dir = unique_test_dir("postfiat-existing-chain-storage-generation");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-existing-chain-storage-activation".to_owned(),
        node_id: "validator-0".to_owned(),
        validator_count: 1,
    })
    .expect("initialize existing-chain activation source");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("read existing-chain genesis");
    let mut governance = store
        .read_governance()
        .expect("read existing-chain governance");
    let mut ledger = store.read_ledger().expect("read existing-chain ledger");
    let seed_amendment = ratify_governance(RatifyGovernanceOptions {
        data_dir: data_dir.clone(),
        validators: vec!["validator-0".to_owned()],
        support: vec!["validator-0".to_owned()],
        kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_owned(),
        value: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: data_dir.join("seed-amendment.json"),
    })
    .expect("ratify existing-chain seed amendment");
    let seed_batch = build_governance_action_batch(&genesis, vec![seed_amendment], Vec::new())
        .expect("build existing-chain seed batch");
    let seed_receipts =
        execute_governance_batch(&mut governance, Some(&mut ledger), &seed_batch, 1);
    assert!(seed_receipts[0].accepted, "{seed_receipts:?}");
    let seed = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_GOVERNANCE,
        &seed_batch.batch_id,
        &seed_batch,
        &seed_receipts,
        true,
    );
    apply_activation_journal(&store, &seed);
    let frozen_tip = store.read_chain_tip().expect("read frozen legacy tip");
    rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: frozen_tip.block_hash.clone(),
        expected_state_root: frozen_tip.state_root.clone(),
        verify_only: false,
    })
    .expect("build activation migration generation");
    let activation_record_file = data_dir.join("storage-activation-record.json");
    let activation_template = create_storage_activation_template(
        StorageActivationTemplateOptions {
            data_dir: data_dir.clone(),
            activation_height: 3,
            record_file: activation_record_file.clone(),
        },
    )
    .expect("create storage activation template");
    let activated_record_id = activation_template.record.activation_id.clone();
    let activation_amendment_file = data_dir.join("storage-activation-amendment.json");
    ratify_storage_activation(StorageActivationRatificationOptions {
        data_dir: data_dir.clone(),
        record_file: activation_record_file.clone(),
        validators: vec!["validator-0".to_owned()],
        support: vec!["validator-0".to_owned()],
        amendment_file: activation_amendment_file.clone(),
    })
    .expect("authorize storage activation");
    let activation_batch = create_storage_activation_batch(StorageActivationBatchOptions {
        data_dir: data_dir.clone(),
        record_file: activation_record_file,
        authorization_amendment_file: activation_amendment_file,
        batch_file: data_dir.join("storage-activation-batch.json"),
    })
    .expect("create authorized storage activation batch");
    let receipts = execute_governance_batch(
        &mut governance,
        Some(&mut ledger),
        &activation_batch,
        2,
    );
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted, "{receipts:?}");
    let scheduling = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_GOVERNANCE,
        &activation_batch.batch_id,
        &activation_batch,
        &receipts,
        true,
    );
    apply_activation_journal(&store, &scheduling);
    assert_eq!(store.read_chain_tip().expect("legacy scheduling tip").height, 2);
    assert_eq!(
        store
            .transactional_store()
            .expect("open scheduled generation")
            .meta()
            .expect("read scheduled metadata")
            .scheduled_activation_height,
        Some(3)
    );
    let legacy_blocks_before_activation =
        std::fs::read(data_dir.join(BLOCKS_APPEND_FILE)).expect("read legacy blocks before switch");

    let active_batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build first active batch")
    .batch;
    let active = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &active_batch.batch_id,
        &active_batch,
        &[],
        false,
    );
    assert_eq!(active.height, 3);
    apply_activation_journal(&store, &active);
    assert_eq!(store.read_chain_tip().expect("active transactional tip").height, 3);
    assert_eq!(
        std::fs::read(data_dir.join(BLOCKS_APPEND_FILE)).expect("read legacy blocks after switch"),
        legacy_blocks_before_activation,
        "activation height must not append to legacy history"
    );
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("replay existing-chain activation boundary");

    let archive_handoff_file = data_dir.join("active-storage-archive-handoff.json");
    create_history_archive_handoff(HistoryArchiveHandoffCreateOptions {
        data_dir: data_dir.clone(),
        from_height: 1,
        to_height: 1,
        archive_uri: Some("archive://postfiat/active-storage-test".to_owned()),
        output_file: archive_handoff_file.clone(),
        overwrite: false,
    })
    .expect("create active transactional archive handoff");
    let mut prune_options = HistoryOptions::with_defaults(data_dir.clone());
    prune_options.retain_recent_blocks = 2;
    prune_options.minimum_replay_window_blocks = 0;
    prune_options.prune_up_to_height = Some(1);
    prune_options.archive_handoff_file = Some(archive_handoff_file);
    let prune = history_prune(prune_options).expect("prune active transactional history");
    assert_eq!(prune.checkpoint.pruned_up_to_height, 1);
    let pruned_store = store
        .transactional_store()
        .expect("open pruned transactional generation");
    assert!(pruned_store.block(1).expect("read pruned height").is_none());
    assert!(pruned_store.block(2).expect("read retained height").is_some());
    assert_eq!(
        pruned_store
            .meta()
            .expect("read pruned transactional metadata")
            .history_base_height,
        1
    );
    assert!(pruned_store
        .current_state_raw("retained_history_checkpoint")
        .expect("read pruned transactional checkpoint")
        .is_some());
    assert!(
        !data_dir.join(HISTORY_CHECKPOINT_FILE).exists(),
        "active prune must not publish a parallel legacy checkpoint"
    );

    let snapshot_dir = unique_test_dir("postfiat-active-storage-snapshot");
    let restored_dir = unique_test_dir("postfiat-active-storage-snapshot-restore");
    let snapshot = export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export active transactional snapshot");
    let restored = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: Some("validator-restored".to_owned()),
    })
    .expect("import active transactional snapshot");
    assert_eq!(restored.block_height, snapshot.block_height);
    assert_eq!(restored.block_tip_hash, snapshot.block_tip_hash);
    assert_eq!(restored.state_root, snapshot.state_root);
    let restored_store = NodeStore::new(&restored_dir);
    assert!(restored_store
        .transactional_storage_active()
        .expect("restored snapshot transactional activation status"));
    let restored_transactional = restored_store
        .transactional_store()
        .expect("open restored snapshot transactional store");
    let restored_meta = restored_transactional
        .meta()
        .expect("read restored snapshot transactional metadata");
    assert_eq!(restored_meta.finalized_height, 3);
    assert_eq!(restored_meta.history_base_height, 1);
    assert!(restored_transactional
        .current_state_raw("retained_history_checkpoint")
        .expect("read restored retained checkpoint")
        .is_some());
    verify_blocks(NodeOptions {
        data_dir: restored_dir.clone(),
    })
    .expect("replay restored active transactional snapshot");

    let mut late_cancellation = postfiat_types::StorageCommitmentCancellationRecordV1 {
        schema: postfiat_types::STORAGE_COMMITMENT_CANCELLATION_SCHEMA_V1.to_owned(),
        cancellation_id: "0".repeat(96),
        activation_id: activated_record_id,
        authorization_amendment_id: "0".repeat(96),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        cancellation_height: 4,
        reason: "late rollback attempt".to_owned(),
    };
    late_cancellation.cancellation_id = late_cancellation
        .expected_cancellation_id()
        .expect("derive late cancellation id");
    let late_authorization = ratify_governance(RatifyGovernanceOptions {
        data_dir: data_dir.clone(),
        validators: vec!["validator-0".to_owned()],
        support: vec!["validator-0".to_owned()],
        kind: late_cancellation.authorization_kind(),
        value: 4,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: data_dir.join("late-storage-cancellation-amendment.json"),
    })
    .expect("ratify late cancellation authorization");
    late_cancellation.authorization_amendment_id = late_authorization.amendment_id.clone();
    let late_batch = build_storage_commitment_cancellation_batch(
        &genesis,
        late_authorization,
        late_cancellation,
    )
    .expect("build late cancellation batch");
    let late_receipts =
        execute_governance_batch(&mut governance, Some(&mut ledger), &late_batch, 4);
    assert_eq!(late_receipts.len(), 1);
    assert!(!late_receipts[0].accepted, "{late_receipts:?}");
    assert_eq!(
        late_receipts[0].code,
        "storage_commitment_cancellation_rejected"
    );
    assert!(late_receipts[0]
        .message
        .contains("cannot be cancelled at or after activation"));

    std::fs::remove_dir_all(data_dir).expect("remove existing-chain activation source");
    std::fs::remove_dir_all(output_dir).expect("remove existing-chain activation generation");
    std::fs::remove_dir_all(snapshot_dir).expect("remove active snapshot");
    std::fs::remove_dir_all(restored_dir).expect("remove active snapshot restore");
}

#[test]
fn existing_chain_storage_activation_can_cancel_only_before_cutover() {
    let data_dir = unique_test_dir("postfiat-existing-chain-storage-cancellation");
    let output_dir = unique_test_dir("postfiat-existing-chain-cancel-generation");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-existing-chain-storage-cancellation".to_owned(),
        node_id: "validator-0".to_owned(),
        validator_count: 1,
    })
    .expect("initialize existing-chain cancellation source");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("read cancellation genesis");
    let mut governance = store.read_governance().expect("read cancellation governance");
    let mut ledger = store.read_ledger().expect("read cancellation ledger");

    let seed_amendment = ratify_governance(RatifyGovernanceOptions {
        data_dir: data_dir.clone(),
        validators: vec!["validator-0".to_owned()],
        support: vec!["validator-0".to_owned()],
        kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_owned(),
        value: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: data_dir.join("cancellation-seed-amendment.json"),
    })
    .expect("ratify cancellation seed amendment");
    let seed_batch = build_governance_action_batch(&genesis, vec![seed_amendment], Vec::new())
        .expect("build cancellation seed batch");
    let seed_receipts =
        execute_governance_batch(&mut governance, Some(&mut ledger), &seed_batch, 1);
    assert!(seed_receipts[0].accepted, "{seed_receipts:?}");
    let seed = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_GOVERNANCE,
        &seed_batch.batch_id,
        &seed_batch,
        &seed_receipts,
        true,
    );
    apply_activation_journal(&store, &seed);

    let frozen_tip = store.read_chain_tip().expect("read cancellation frozen tip");
    rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: frozen_tip.block_hash.clone(),
        expected_state_root: frozen_tip.state_root.clone(),
        verify_only: false,
    })
    .expect("build cancellation migration generation");
    let activation_record_file = data_dir.join("cancellable-storage-activation-record.json");
    let activation_template = create_storage_activation_template(
        StorageActivationTemplateOptions {
            data_dir: data_dir.clone(),
            activation_height: 4,
            record_file: activation_record_file.clone(),
        },
    )
    .expect("create cancellable storage activation template");
    let activation_id = activation_template.record.activation_id.clone();
    let activation_amendment_file =
        data_dir.join("cancellable-storage-activation-amendment.json");
    ratify_storage_activation(StorageActivationRatificationOptions {
        data_dir: data_dir.clone(),
        record_file: activation_record_file.clone(),
        validators: vec!["validator-0".to_owned()],
        support: vec!["validator-0".to_owned()],
        amendment_file: activation_amendment_file.clone(),
    })
    .expect("authorize cancellable storage activation");
    let activation_batch = create_storage_activation_batch(StorageActivationBatchOptions {
        data_dir: data_dir.clone(),
        record_file: activation_record_file,
        authorization_amendment_file: activation_amendment_file,
        batch_file: data_dir.join("cancellable-storage-activation-batch.json"),
    })
    .expect("create cancellable storage activation batch");
    let activation_receipts = execute_governance_batch(
        &mut governance,
        Some(&mut ledger),
        &activation_batch,
        2,
    );
    assert!(activation_receipts[0].accepted, "{activation_receipts:?}");
    let scheduling = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_GOVERNANCE,
        &activation_batch.batch_id,
        &activation_batch,
        &activation_receipts,
        true,
    );
    apply_activation_journal(&store, &scheduling);
    assert_eq!(
        store
            .transactional_store()
            .expect("open scheduled cancellation generation")
            .meta()
            .expect("read scheduled cancellation metadata")
            .scheduled_activation_height,
        Some(4)
    );

    let cancellation_record_file = data_dir.join("storage-cancellation-record.json");
    create_storage_cancellation_template(StorageCancellationTemplateOptions {
        data_dir: data_dir.clone(),
        activation_id,
        reason: "pre-activation operator rollback rehearsal".to_owned(),
        record_file: cancellation_record_file.clone(),
    })
    .expect("create storage cancellation template");
    let cancellation_amendment_file = data_dir.join("storage-cancellation-amendment.json");
    ratify_storage_cancellation(StorageCancellationRatificationOptions {
        data_dir: data_dir.clone(),
        record_file: cancellation_record_file.clone(),
        validators: vec!["validator-0".to_owned()],
        support: vec!["validator-0".to_owned()],
        amendment_file: cancellation_amendment_file.clone(),
    })
    .expect("authorize storage cancellation");
    let cancellation_batch = create_storage_cancellation_batch(StorageCancellationBatchOptions {
        data_dir: data_dir.clone(),
        record_file: cancellation_record_file,
        authorization_amendment_file: cancellation_amendment_file,
        batch_file: data_dir.join("storage-cancellation-batch.json"),
    })
    .expect("create authorized storage cancellation batch");
    let cancellation_receipts = execute_governance_batch(
        &mut governance,
        Some(&mut ledger),
        &cancellation_batch,
        3,
    );
    assert_eq!(cancellation_receipts.len(), 1);
    assert!(cancellation_receipts[0].accepted, "{cancellation_receipts:?}");
    assert_eq!(
        cancellation_receipts[0].code,
        "storage_commitment_activation_cancelled"
    );
    let cancellation_journal = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_GOVERNANCE,
        &cancellation_batch.batch_id,
        &cancellation_batch,
        &cancellation_receipts,
        true,
    );
    apply_activation_journal(&store, &cancellation_journal);
    assert_eq!(
        store
            .transactional_store()
            .expect("open cancelled generation")
            .meta()
            .expect("read cancelled metadata")
            .scheduled_activation_height,
        None
    );
    assert!(!store
        .transactional_storage_active()
        .expect("read cancelled activation status"));

    let legacy_bytes_before_height_four =
        std::fs::metadata(data_dir.join(BLOCKS_APPEND_FILE))
            .expect("read pre-height-four legacy block metadata")
            .len();
    let former_activation_batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build former activation-height batch")
    .batch;
    let former_activation_journal = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &former_activation_batch.batch_id,
        &former_activation_batch,
        &[],
        false,
    );
    assert_eq!(former_activation_journal.height, 4);
    apply_activation_journal(&store, &former_activation_journal);
    assert!(
        std::fs::metadata(data_dir.join(BLOCKS_APPEND_FILE))
            .expect("read post-height-four legacy block metadata")
            .len()
            > legacy_bytes_before_height_four,
        "a cancelled activation must keep the former cutover height on legacy storage"
    );
    assert!(!store
        .transactional_storage_active()
        .expect("read former activation status"));
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("replay cancelled storage activation");

    std::fs::remove_dir_all(data_dir).expect("remove cancellation source");
    std::fs::remove_dir_all(output_dir).expect("remove cancellation generation");
}
