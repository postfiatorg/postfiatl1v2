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

fn snapshot_activation_test_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, output: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = std::fs::read_dir(directory)
            .expect("read mutation-sentinel directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read mutation-sentinel entries");
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("derive mutation-sentinel relative path")
                .to_string_lossy()
                .into_owned();
            let file_type = entry.file_type().expect("read mutation-sentinel file type");
            if file_type.is_dir() {
                output.insert(format!("{relative}/"), Vec::new());
                visit(root, &path, output);
            } else if file_type.is_file() {
                output.insert(
                    relative,
                    std::fs::read(&path).expect("read mutation-sentinel file"),
                );
            } else {
                panic!("unexpected mutation-sentinel entry `{}`", path.display());
            }
        }
    }

    let mut output = BTreeMap::new();
    if root.is_dir() {
        visit(root, root, &mut output);
    }
    output
}

fn assert_transactional_logical_equivalence(
    expected: &postfiat_storage::TransactionalStore,
    observed: &postfiat_storage::TransactionalStore,
) {
    let expected_meta = expected.meta().expect("read expected transactional metadata");
    let observed_meta = observed.meta().expect("read observed transactional metadata");
    assert_eq!(
        expected_meta.chain_tip(CHAIN_TIP_SCHEMA.to_owned()),
        observed_meta.chain_tip(CHAIN_TIP_SCHEMA.to_owned()),
        "transactional tips differ"
    );
    assert_eq!(
        expected.blocks_in_height_order().expect("export expected blocks"),
        observed.blocks_in_height_order().expect("export observed blocks"),
        "canonical blocks differ"
    );
    assert_eq!(
        expected
            .receipts_in_block_order()
            .expect("export expected receipts"),
        observed
            .receipts_in_block_order()
            .expect("export observed receipts"),
        "canonical receipts differ"
    );
    assert_eq!(
        expected
            .archived_batches_in_block_order()
            .expect("export expected archives"),
        observed
            .archived_batches_in_block_order()
            .expect("export observed archives"),
        "canonical batch archives differ"
    );
    assert_eq!(
        expected.ordered_batches().expect("export expected ordering"),
        observed.ordered_batches().expect("export observed ordering"),
        "canonical ordered-batch entries differ"
    );
    assert_eq!(
        expected
            .canonical_history_index_entries()
            .expect("export expected history indexes"),
        observed
            .canonical_history_index_entries()
            .expect("export observed history indexes"),
        "canonical rebuildable history indexes differ"
    );
    for domain in [
        "ledger",
        "governance",
        "shielded",
        "bridge",
        "validator_registry",
        "storage_activation",
        "retained_history_checkpoint",
    ] {
        assert_eq!(
            expected
                .current_state_raw(domain)
                .expect("export expected current state"),
            observed
                .current_state_raw(domain)
                .expect("export observed current state"),
            "canonical current state differs for {domain}"
        );
    }
    assert_eq!(
        expected
            .verify_logical_integrity()
            .expect("verify expected transactional store"),
        observed
            .verify_logical_integrity()
            .expect("verify observed transactional store"),
        "logical integrity reports differ"
    );
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
        status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("repeat activation recovery idempotently");
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

fn backend_equivalence_transfer_journal(
    store: &NodeStore,
    genesis: &Genesis,
) -> OrderedCommitDeltaJournal {
    let governance = store
        .read_governance()
        .expect("read backend-equivalence governance");
    let mut ledger = store
        .read_ledger()
        .expect("read backend-equivalence ledger");
    let faucet = read_transfer_key_file(store.data_dir(), None)
        .expect("read backend-equivalence faucet key");
    let transfer = build_signed_transfer(
        genesis,
        &ledger,
        store.data_dir(),
        None,
        faucet.address,
        1,
    )
    .expect("build backend-equivalence transfer");
    let batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(genesis),
        vec![transfer],
    )
    .expect("build canonical backend-equivalence batch")
    .batch;
    let height = store
        .read_chain_tip()
        .expect("read backend-equivalence tip")
        .height
        .checked_add(1)
        .expect("backend-equivalence height overflow");
    let receipts = execute_transparent_batch(
        genesis,
        &governance,
        &mut ledger,
        &batch,
        height,
        AssetExecutionCompatibility::strict(),
    );
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted, "{receipts:?}");
    activation_test_journal(
        store,
        genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &batch.batch_id,
        &batch,
        &receipts,
        false,
    )
}

#[test]
fn ordered_commit_journal_disagreement_rejects_without_durable_mutation() {
    let data_dir = unique_test_dir("postfiat-ordered-journal-disagreement");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-ordered-journal-disagreement".to_owned(),
        node_id: "validator-0".to_owned(),
        validator_count: 1,
    })
    .expect("initialize journal-disagreement store");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("read journal genesis");
    let governance = store.read_governance().expect("read journal governance");
    let ledger = store.read_ledger().expect("read journal ledger");
    let batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build journal-disagreement batch")
    .batch;
    let mut journal = activation_test_journal(
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
    journal.ordered_batch_id = "authenticated-but-conflicting-batch".to_owned();
    store
        .write_ordered_commit_journal(&journal)
        .expect("write authenticated conflicting journal");
    let tip_before = store.read_chain_tip().expect("read tip before rejection");
    let blocks_before = store.read_blocks().expect("read blocks before rejection");
    let ledger_before = store.read_ledger().expect("read ledger before rejection");
    let journal_before = store
        .read_ordered_commit_journal_raw()
        .expect("read journal before rejection");

    let error = recover_ordered_commit_journal(&store)
        .expect_err("journal/block disagreement must fail closed");
    assert!(
        error
            .to_string()
            .contains("ordered commit delta journal batch `authenticated-but-conflicting-batch` does not match block batch"),
        "{error}"
    );
    assert_eq!(store.read_chain_tip().expect("read rejected tip"), tip_before);
    assert_eq!(store.read_blocks().expect("read rejected blocks"), blocks_before);
    assert_eq!(store.read_ledger().expect("read rejected ledger"), ledger_before);
    assert_eq!(
        store
            .read_ordered_commit_journal_raw()
            .expect("read rejected journal"),
        journal_before
    );

    std::fs::remove_dir_all(data_dir).expect("remove journal-disagreement store");
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
    let restored_transactional = restored_store
        .transactional_store()
        .expect("open restored new-chain transactional store");
    assert_eq!(
        restored_transactional
            .verify_logical_integrity()
            .expect("verify restored new-chain transactional store")
            .finalized_height,
        1
    );
    assert_transactional_logical_equivalence(&transactional, &restored_transactional);

    std::fs::remove_dir_all(data_dir).expect("remove transactional test directory");
    std::fs::remove_dir_all(snapshot_dir).expect("remove transactional snapshot");
    std::fs::remove_dir_all(restored_dir).expect("remove transactional snapshot restore");
}

#[test]
fn active_storage_backends_apply_identical_commits_from_one_snapshot() {
    let seed_dir = unique_test_dir("postfiat-storage-backend-equivalence-seed");
    let snapshot_dir = unique_test_dir("postfiat-storage-backend-equivalence-snapshot");
    let mut genesis = Genesis::try_new_with_validator_count(
        "postfiat-storage-backend-equivalence",
        1,
    )
    .expect("create backend-equivalence genesis");
    genesis.ordered_history_v2_activation_height = Some(1);
    lifecycle_queries::init_with_genesis(
        seed_dir.clone(),
        "validator-0".to_owned(),
        genesis.clone(),
    )
    .expect("initialize backend-equivalence seed");
    let seed = NodeStore::new(&seed_dir);

    let first = backend_equivalence_transfer_journal(&seed, &genesis);
    apply_activation_journal(&seed, &first);

    let second = backend_equivalence_transfer_journal(&seed, &genesis);
    let source_manifest = export_snapshot(SnapshotExportOptions {
        data_dir: seed_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export shared backend snapshot");
    assert_eq!(source_manifest.block_height, 1);

    apply_activation_journal(&seed, &second);
    let third = backend_equivalence_transfer_journal(&seed, &genesis);

    let modes = [
        postfiat_storage::StorageBackendMode::LegacyJsonl,
        postfiat_storage::StorageBackendMode::BoundedJsonl,
        postfiat_storage::StorageBackendMode::Transactional,
    ];
    let mut proposed_commitments = Vec::new();
    let mut final_identities = Vec::new();
    let mut portable_file_hashes = Vec::new();
    for mode in modes {
        let data_dir = unique_test_dir(&format!(
            "postfiat-storage-backend-equivalence-{}",
            mode.as_str()
        ));
        import_snapshot(SnapshotImportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: None,
        })
        .expect("import shared backend snapshot");
        let configured = configure_storage_backend(StorageBackendConfigureOptions {
            data_dir: data_dir.clone(),
            mode,
        })
        .expect("configure comparison backend");
        assert_eq!(configured.mode, mode.as_str());
        assert_eq!(configured.finalized_height, 1);

        let store = NodeStore::new(&data_dir);
        let (_, proposed_second) = proposed_ordered_state(
            &store,
            &genesis,
            &second.ordered_batch_id,
            2,
        )
        .expect("propose second comparison commitment");
        proposed_commitments.push(proposed_second.expect("active commitment"));
        apply_activation_journal(&store, &second);
        let (_, proposed_third) =
            proposed_ordered_state(&store, &genesis, &third.ordered_batch_id, 3)
                .expect("propose third comparison commitment");
        assert_eq!(
            proposed_third.as_ref().expect("active third commitment"),
            &store
                .backend_ordered_history_commitment()
                .expect("read committed second commitment")
                .append(&third.ordered_batch_id)
                .expect("append expected third commitment")
        );
        apply_activation_journal(&store, &third);
        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify backend-equivalent blocks");

        let status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("read backend-equivalent status");
        let storage = status.storage.expect("storage status");
        assert_eq!(status.block_height, 3);
        assert_eq!(storage.ordered_batch_count, 3);
        final_identities.push((
            status.block_height,
            status.block_tip_hash,
            status.state_root,
            storage.ordered_history_accumulator,
        ));

        let portable = unique_test_dir(&format!(
            "postfiat-storage-backend-equivalence-export-{}",
            mode.as_str()
        ));
        let manifest = export_snapshot(SnapshotExportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: portable.clone(),
        })
        .expect("export backend-equivalent snapshot");
        portable_file_hashes.push(
            manifest
                .files
                .iter()
                .map(|file| (file.name.clone(), file.hash_hex.clone()))
                .collect::<Vec<_>>(),
        );
        std::fs::remove_dir_all(portable).expect("remove backend export");
        std::fs::remove_dir_all(data_dir).expect("remove backend clone");
    }

    assert!(proposed_commitments
        .windows(2)
        .all(|pair| pair[0] == pair[1]));
    assert!(final_identities.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(portable_file_hashes
        .windows(2)
        .all(|pair| pair[0] == pair[1]));

    std::fs::remove_dir_all(seed_dir).expect("remove backend seed");
    std::fs::remove_dir_all(snapshot_dir).expect("remove shared backend snapshot");
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
fn transactional_verify_only_does_not_create_missing_source_or_target() {
    let data_dir = unique_test_dir("postfiat-transactional-verify-missing-source");
    let output_dir = unique_test_dir("postfiat-transactional-verify-missing-output");
    assert!(!data_dir.exists());
    assert!(!output_dir.exists());

    let error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: "00".repeat(48),
        expected_state_root: "11".repeat(48),
        verify_only: true,
    })
    .expect_err("verify-only must not create a missing source or target");
    assert!(
        error
            .to_string()
            .contains("storage_integrity_key_missing"),
        "{error}"
    );
    assert!(!data_dir.exists(), "verify-only created the source directory");
    assert!(!output_dir.exists(), "verify-only created the target directory");
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

    let source_before_missing_target = snapshot_activation_test_tree(&data_dir);
    let missing_target_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("verify-only must refuse a missing target without creating it");
    assert!(
        missing_target_error
            .to_string()
            .contains("storage_migration_verify_output_missing"),
        "{missing_target_error}"
    );
    assert_eq!(
        snapshot_activation_test_tree(&data_dir),
        source_before_missing_target,
        "missing-target refusal mutated the source directory"
    );
    assert!(!output_dir.exists(), "verify-only created the missing target");

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

    let source_before_verify = snapshot_activation_test_tree(&data_dir);
    let output_before_verify = snapshot_activation_test_tree(&output_dir);
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
        snapshot_activation_test_tree(&data_dir),
        source_before_verify,
        "successful --verify-only mutated the source directory"
    );
    assert_eq!(
        snapshot_activation_test_tree(&output_dir),
        output_before_verify,
        "successful --verify-only mutated the target directory"
    );

    let chain_tip_path = data_dir.join(postfiat_storage::CHAIN_TIP_FILE);
    let chain_tip_bytes = std::fs::read(&chain_tip_path).expect("read chain tip before removal");
    std::fs::remove_file(&chain_tip_path).expect("remove chain tip for read-only reconstruction");
    let source_before_tip_reconstruction = snapshot_activation_test_tree(&data_dir);
    let reconstructed = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect("verify-only should reconstruct a missing chain tip in memory");
    assert_eq!(reconstructed.source_tip, source_tip);
    assert_eq!(
        snapshot_activation_test_tree(&data_dir),
        source_before_tip_reconstruction,
        "read-only chain-tip reconstruction persisted a repair"
    );
    assert!(
        !chain_tip_path.exists(),
        "read-only verification recreated the chain-tip file"
    );
    std::fs::write(&chain_tip_path, chain_tip_bytes).expect("restore chain tip after read-only test");

    store
        .write_ordered_commit_journal(&journal)
        .expect("write pending source journal for verify-only refusal");
    let source_before_pending_refusal = snapshot_activation_test_tree(&data_dir);
    let output_before_pending_refusal = snapshot_activation_test_tree(&output_dir);
    let read_only_source =
        NodeStore::try_new_read_only(&data_dir).expect("open pending source read-only");
    let recovery_error = recover_ordered_commit_journal(&read_only_source)
        .expect_err("read-only source must reject journal recovery before creating a lock");
    assert!(
        recovery_error
            .to_string()
            .contains("storage_read_only_write_refused"),
        "{recovery_error}"
    );
    drop(read_only_source);
    assert_eq!(
        snapshot_activation_test_tree(&data_dir),
        source_before_pending_refusal,
        "read-only journal recovery refusal mutated the source directory"
    );
    let pending_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("verify-only must refuse rather than recover a pending source journal");
    assert!(
        pending_error
            .to_string()
            .contains("storage_migration_verify_source_recovery_required"),
        "{pending_error}"
    );
    assert_eq!(
        snapshot_activation_test_tree(&data_dir),
        source_before_pending_refusal,
        "pending-journal refusal mutated the source directory"
    );
    assert_eq!(
        snapshot_activation_test_tree(&output_dir),
        output_before_pending_refusal,
        "pending-journal refusal mutated the target directory"
    );
    store
        .remove_ordered_commit_journal()
        .expect("remove pending source journal after refusal test");

    let manifest_path = output_dir.join(STORAGE_MIGRATION_MANIFEST_FILE);
    let original_manifest = std::fs::read(&manifest_path).expect("read migration manifest");
    let mut tampered_manifest: StorageMigrationManifestV2 =
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
    std::fs::write(&manifest_path, &original_manifest).expect("restore migration manifest");

    let pointer_path = data_dir.join(
        postfiat_storage::transactional::TRANSACTIONAL_GENERATION_POINTER_FILE,
    );
    let database_path = output_dir.join(
        postfiat_storage::transactional::TRANSACTIONAL_DATABASE_FILE,
    );
    let pointer_before = std::fs::read(&pointer_path).expect("read pointer before export tests");
    let database_before =
        std::fs::read(&database_path).expect("read database before export tests");

    std::fs::remove_file(&manifest_path).expect("remove migration manifest");
    let missing_manifest_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("missing migration manifest must reject");
    assert!(
        missing_manifest_error
            .to_string()
            .contains("storage_migration_manifest_missing"),
        "{missing_manifest_error}"
    );
    assert_eq!(
        std::fs::read(&pointer_path).expect("read pointer after missing manifest"),
        pointer_before
    );
    assert_eq!(
        std::fs::read(&database_path).expect("read database after missing manifest"),
        database_before
    );
    std::fs::write(&manifest_path, &original_manifest).expect("restore missing manifest");

    let checksum_path = output_dir.join(STORAGE_MIGRATION_MANIFEST_CHECKSUM_FILE);
    let original_checksum =
        std::fs::read(&checksum_path).expect("read migration manifest checksum");
    std::fs::remove_file(&checksum_path).expect("remove migration manifest checksum");
    let missing_checksum_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("missing migration checksum must reject");
    assert!(
        missing_checksum_error
            .to_string()
            .contains("storage_migration_manifest_checksum_missing"),
        "{missing_checksum_error}"
    );
    std::fs::write(&checksum_path, &original_checksum).expect("restore missing checksum");

    let mut substituted_checksum = original_checksum.clone();
    let checksum_byte = substituted_checksum
        .first_mut()
        .expect("migration checksum is nonempty");
    *checksum_byte = if *checksum_byte == b'0' { b'1' } else { b'0' };
    std::fs::write(&checksum_path, substituted_checksum).expect("substitute migration checksum");
    let substituted_checksum_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("substituted migration checksum must reject");
    assert!(
        substituted_checksum_error
            .to_string()
            .contains("storage_migration_manifest_checksum_mismatch"),
        "{substituted_checksum_error}"
    );
    assert_eq!(
        std::fs::read(&pointer_path).expect("read pointer after export rejection"),
        pointer_before
    );
    assert_eq!(
        std::fs::read(&database_path).expect("read database after export rejection"),
        database_before
    );
    std::fs::write(&checksum_path, original_checksum).expect("restore migration checksum");

    let canonical_export_path = output_dir.join(STORAGE_CANONICAL_EXPORT_FILE);
    let canonical_export =
        std::fs::read(&canonical_export_path).expect("read migration canonical export");
    assert_eq!(
        report.canonical_export_file,
        canonical_export_path,
        "rebuild report did not identify the canonical export"
    );
    assert_eq!(
        verified.canonical_export_receipt,
        report.canonical_export_receipt,
        "verify-only did not reproduce the canonical export receipt"
    );

    std::fs::remove_file(&canonical_export_path).expect("remove migration canonical export");
    let missing_export_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("missing canonical export must reject verify-only");
    assert!(
        missing_export_error
            .to_string()
            .contains("storage_canonical_export_missing"),
        "{missing_export_error}"
    );
    assert_eq!(
        std::fs::read(&pointer_path).expect("read pointer after missing canonical export"),
        pointer_before
    );
    assert_eq!(
        std::fs::read(&database_path).expect("read database after missing canonical export"),
        database_before
    );
    std::fs::write(&canonical_export_path, &canonical_export)
        .expect("restore missing canonical export");

    let mut corrupted_export = canonical_export.clone();
    corrupted_export.pop();
    std::fs::write(&canonical_export_path, corrupted_export)
        .expect("truncate migration canonical export");
    let corrupted_export_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("corrupted canonical export must reject verify-only");
    assert!(
        corrupted_export_error
            .to_string()
            .contains("storage_canonical_export_integrity_failure"),
        "{corrupted_export_error}"
    );
    assert_eq!(
        std::fs::read(&pointer_path).expect("read pointer after corrupted canonical export"),
        pointer_before
    );
    assert_eq!(
        std::fs::read(&database_path).expect("read database after corrupted canonical export"),
        database_before
    );
    std::fs::write(&canonical_export_path, &canonical_export)
        .expect("restore corrupted canonical export");

    let donor_dir = unique_test_dir("postfiat-transactional-rebuild-export-donor");
    let donor_store = store
        .open_transactional_store_at(&donor_dir)
        .expect("open canonical export donor store");
    let donor_tip = ChainTipState {
        schema: source_tip.schema.clone(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        height: 0,
        block_hash: "donor-genesis".to_owned(),
        state_root: "donor-state".to_owned(),
        ordered_batch_count: 0,
        receipt_count: 0,
        history_base_height: 0,
    };
    let donor_commitment = postfiat_storage::OrderedHistoryCommitment::genesis(
        &donor_tip.chain_id,
        &donor_tip.genesis_hash,
        donor_tip.protocol_version,
    )
    .expect("build canonical export donor commitment");
    donor_store
        .initialize(
            &donor_tip,
            &donor_commitment,
            postfiat_storage::CurrentStateUpdate::default(),
        )
        .expect("initialize canonical export donor store");
    let donor_export_path = donor_dir.join("donor-canonical-history.jsonl");
    donor_store
        .write_canonical_jsonl_export(&donor_export_path)
        .expect("write valid canonical export donor");
    std::fs::copy(&donor_export_path, &canonical_export_path)
        .expect("substitute valid foreign canonical export");
    let substituted_export_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("substituted canonical export must reject verify-only");
    assert!(
        substituted_export_error
            .to_string()
            .contains("storage_canonical_export_substituted"),
        "{substituted_export_error}"
    );
    assert_eq!(
        std::fs::read(&pointer_path).expect("read pointer after substituted canonical export"),
        pointer_before
    );
    assert_eq!(
        std::fs::read(&database_path).expect("read database after substituted canonical export"),
        database_before
    );
    std::fs::write(&canonical_export_path, canonical_export)
        .expect("restore substituted canonical export");

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
fn transactional_migration_packet_root_is_shared_across_node_local_state() {
    let source_dir = unique_test_dir("postfiat-shared-migration-root-source");
    let snapshot_dir = unique_test_dir("postfiat-shared-migration-root-snapshot");
    let peer_dir = unique_test_dir("postfiat-shared-migration-root-peer");
    let source_generation = unique_test_dir("postfiat-shared-migration-root-source-generation");
    let peer_generation = unique_test_dir("postfiat-shared-migration-root-peer-generation");
    init(InitOptions {
        data_dir: source_dir.clone(),
        chain_id: "postfiat-shared-migration-root-test".to_owned(),
        node_id: "validator-0".to_owned(),
        validator_count: 1,
    })
    .expect("initialize shared migration root source");
    let source_store = NodeStore::new(&source_dir);
    let genesis = source_store
        .read_genesis()
        .expect("read shared migration root genesis");
    let governance = source_store
        .read_governance()
        .expect("read shared migration root governance");
    let ledger = source_store
        .read_ledger()
        .expect("read shared migration root ledger");
    let batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build shared migration root batch")
    .batch;
    let journal = activation_test_journal(
        &source_store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &batch.batch_id,
        &batch,
        &[],
        false,
    );
    apply_activation_journal(&source_store, &journal);
    export_snapshot(SnapshotExportOptions {
        data_dir: source_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export shared migration root snapshot");
    import_snapshot(SnapshotImportOptions {
        data_dir: peer_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: Some("validator-peer".to_owned()),
    })
    .expect("import shared migration root peer");

    let source_tip = source_store
        .read_chain_tip()
        .expect("read shared migration root source tip");
    let peer_tip = NodeStore::new(&peer_dir)
        .read_chain_tip()
        .expect("read shared migration root peer tip");
    assert_eq!(peer_tip, source_tip, "peer did not preserve the certified tip");
    let source_report = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: source_dir.clone(),
        output_dir: source_generation.clone(),
        expected_tip: source_tip.block_hash.clone(),
        expected_state_root: source_tip.state_root.clone(),
        verify_only: false,
    })
    .expect("rebuild shared migration root source");
    let peer_report = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: peer_dir.clone(),
        output_dir: peer_generation.clone(),
        expected_tip: peer_tip.block_hash.clone(),
        expected_state_root: peer_tip.state_root.clone(),
        verify_only: false,
    })
    .expect("rebuild shared migration root peer");
    assert_eq!(
        peer_report.migration_packet_root, source_report.migration_packet_root,
        "activation-bound migration packet root must exclude node-local state"
    );

    let source_manifest: StorageMigrationManifestV2 = serde_json::from_slice(
        &std::fs::read(source_generation.join(STORAGE_MIGRATION_MANIFEST_FILE))
            .expect("read shared source manifest"),
    )
    .expect("decode shared source manifest");
    let peer_manifest: StorageMigrationManifestV2 = serde_json::from_slice(
        &std::fs::read(peer_generation.join(STORAGE_MIGRATION_MANIFEST_FILE))
            .expect("read shared peer manifest"),
    )
    .expect("decode shared peer manifest");
    assert_eq!(
        peer_manifest.current_state_root, source_manifest.current_state_root,
        "consensus current-state migration root must be node independent"
    );
    assert_ne!(
        peer_manifest.node_state_root, source_manifest.node_state_root,
        "local node state must remain independently integrity-bound"
    );
    let source_activation = create_storage_activation_template(
        StorageActivationTemplateOptions {
            data_dir: source_dir.clone(),
            activation_height: 3,
            record_file: source_dir.join("shared-root-activation.json"),
        },
    )
    .expect("create source activation from shared migration root");
    let peer_activation = create_storage_activation_template(StorageActivationTemplateOptions {
        data_dir: peer_dir.clone(),
        activation_height: 3,
        record_file: peer_dir.join("shared-root-activation.json"),
    })
    .expect("create peer activation from shared migration root");
    assert_eq!(
        peer_activation.record, source_activation.record,
        "all validators must derive one consensus-orderable activation record"
    );
    let mut tampered_local_manifest = peer_manifest;
    tampered_local_manifest.node_state_root = "ab".repeat(48);
    std::fs::write(
        peer_generation.join(STORAGE_MIGRATION_MANIFEST_FILE),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&tampered_local_manifest)
                .expect("encode locally tampered migration manifest")
        ),
    )
    .expect("write locally tampered migration manifest");
    let local_tamper_error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: peer_dir.clone(),
        output_dir: peer_generation.clone(),
        expected_tip: peer_tip.block_hash,
        expected_state_root: peer_tip.state_root,
        verify_only: true,
    })
    .expect_err("node-local manifest tamper must fail the exact-file checksum");
    assert!(
        local_tamper_error
            .to_string()
            .contains("storage_migration_manifest_checksum_mismatch"),
        "{local_tamper_error}"
    );

    std::fs::remove_dir_all(source_dir).expect("remove shared migration source");
    std::fs::remove_dir_all(snapshot_dir).expect("remove shared migration snapshot");
    std::fs::remove_dir_all(peer_dir).expect("remove shared migration peer");
    std::fs::remove_dir_all(source_generation).expect("remove shared source generation");
    std::fs::remove_dir_all(peer_generation).expect("remove shared peer generation");
}

#[test]
fn transactional_verify_only_rejects_a_valid_but_stale_generation_without_mutation() {
    let data_dir = unique_test_dir("postfiat-stale-generation-source");
    let output_dir = unique_test_dir("postfiat-stale-generation-output");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-stale-generation-test".to_owned(),
        node_id: "validator-0".to_owned(),
        validator_count: 1,
    })
    .expect("initialize stale-generation source");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("read stale-generation genesis");
    let mut governance = store
        .read_governance()
        .expect("read stale-generation governance");
    let mut ledger = store.read_ledger().expect("read stale-generation ledger");
    let first_amendment = ratify_governance(RatifyGovernanceOptions {
        data_dir: data_dir.clone(),
        validators: vec!["validator-0".to_owned()],
        support: vec!["validator-0".to_owned()],
        kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_owned(),
        value: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: data_dir.join("stale-generation-amendment.json"),
    })
    .expect("ratify first stale-generation amendment");
    let first_batch = build_governance_action_batch(&genesis, vec![first_amendment], Vec::new())
        .expect("build first stale-generation batch");
    let first_receipts =
        execute_governance_batch(&mut governance, Some(&mut ledger), &first_batch, 1);
    assert!(first_receipts[0].accepted, "{first_receipts:?}");
    let first = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_GOVERNANCE,
        &first_batch.batch_id,
        &first_batch,
        &first_receipts,
        true,
    );
    apply_activation_journal(&store, &first);
    let frozen_tip = store
        .read_chain_tip()
        .expect("read stale-generation frozen tip");
    rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: frozen_tip.block_hash.clone(),
        expected_state_root: frozen_tip.state_root.clone(),
        verify_only: false,
    })
    .expect("build generation at first tip");

    let pointer_path = data_dir.join(
        postfiat_storage::transactional::TRANSACTIONAL_GENERATION_POINTER_FILE,
    );
    let stale_pointer = std::fs::read(&pointer_path).expect("read valid stale pointer");
    std::fs::remove_file(&pointer_path).expect("temporarily detach transactional generation");
    let legacy_only = NodeStore::new(&data_dir);
    let second_batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build second stale-generation batch")
    .batch;
    let second = activation_test_journal(
        &legacy_only,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &second_batch.batch_id,
        &second_batch,
        &[],
        false,
    );
    assert_eq!(second.height, 2);
    apply_activation_journal(&legacy_only, &second);
    std::fs::write(&pointer_path, &stale_pointer).expect("restore valid stale pointer");
    drop(legacy_only);
    drop(store);

    let current = NodeStore::try_new_read_only(&data_dir)
        .expect("open current legacy source read-only before verification");
    let current_tip = current
        .read_chain_tip()
        .expect("read current legacy source tip");
    assert_eq!(current_tip.height, 2);
    let blocks_before = current.read_blocks().expect("read blocks before stale rejection");
    drop(current);
    let source_before = snapshot_activation_test_tree(&data_dir);
    let target_before = snapshot_activation_test_tree(&output_dir);
    let error = rebuild_transactional_storage(StorageMigrationOptions {
        data_dir: data_dir.clone(),
        output_dir: output_dir.clone(),
        expected_tip: current_tip.block_hash.clone(),
        expected_state_root: current_tip.state_root.clone(),
        verify_only: true,
    })
    .expect_err("valid generation at an older certified tip must reject");
    assert!(
        error
            .to_string()
            .contains("storage_migration_manifest_domain_mismatch"),
        "{error}"
    );
    assert_eq!(
        snapshot_activation_test_tree(&data_dir),
        source_before,
        "stale-generation rejection mutated the authenticated source"
    );
    assert_eq!(
        snapshot_activation_test_tree(&output_dir),
        target_before,
        "stale-generation rejection mutated the verification target"
    );
    let current = NodeStore::try_new_read_only(&data_dir)
        .expect("reopen current legacy source read-only after verification");
    assert_eq!(
        current.read_blocks().expect("read blocks after stale rejection"),
        blocks_before
    );
    assert_eq!(
        std::fs::read(&pointer_path).expect("read pointer after stale rejection"),
        stale_pointer
    );
    let stale_store = current
        .transactional_store()
        .expect("open still-bound stale generation");
    assert_eq!(
        stale_store
            .meta()
            .expect("read stale generation metadata")
            .finalized_height,
        1
    );

    std::fs::remove_dir_all(data_dir).expect("remove stale-generation source");
    std::fs::remove_dir_all(output_dir).expect("remove stale-generation output");
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
    assert_transactional_logical_equivalence(&pruned_store, &restored_transactional);
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

#[test]
fn ambiguous_active_transactional_state_blocks_vote_without_mutation() {
    let data_dir = unique_test_dir("postfiat-active-storage-vote-block");
    let mut genesis = Genesis::try_new_with_validator_count(
        "postfiat-active-storage-vote-block",
        1,
    )
    .expect("create active-storage vote-block genesis");
    genesis.ordered_history_v2_activation_height = Some(1);
    genesis.consensus_v2_activation_height = Some(2);
    lifecycle_queries::init_with_genesis(
        data_dir.clone(),
        "validator-0".to_owned(),
        genesis.clone(),
    )
    .expect("initialize active-storage vote-block fixture");
    let store = NodeStore::new(&data_dir);
    let governance = store.read_governance().expect("read vote-block governance");
    let ledger = store.read_ledger().expect("read vote-block ledger");
    let first_batch = postfiat_mempool_dag::build_transaction_batch(
        &mempool_batch_domain(&genesis),
        Vec::new(),
    )
    .expect("build vote-block activation batch")
    .batch;
    let first_journal = activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &first_batch.batch_id,
        &first_batch,
        &[],
        false,
    );
    apply_activation_journal(&store, &first_journal);
    assert!(store
        .transactional_storage_active()
        .expect("read active-storage status"));

    let second_batch_file = data_dir.join("vote-block-second-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfstoragevoteblock00000000000000000000".to_owned(),
        amount: 1,
        batch_file: second_batch_file.clone(),
    })
    .expect("create vote-block second batch");
    let proposal_file = data_dir.join("vote-block-second-proposal.json");
    let proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_owned()),
        batch_file: second_batch_file.clone(),
        proposal_file: proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: Some(data_dir.join(VALIDATOR_KEYS_FILE)),
        validator_id: None,
    })
    .expect("build healthy proposal before storage ambiguity");
    let consensus_proposal = create_consensus_v2_proposal_for_block(
        &data_dir,
        &proposal,
        None,
        &data_dir.join(VALIDATOR_KEYS_FILE),
    )
    .expect("build healthy consensus-v2 proposal before storage ambiguity");
    let prepare_votes = ["validator-0"]
        .into_iter()
        .map(|validator_id| {
            create_consensus_v2_prepare_vote(
                &data_dir,
                &consensus_proposal,
                None,
                &data_dir.join(VALIDATOR_KEYS_FILE),
                validator_id,
            )
            .expect("build healthy prepare vote before storage ambiguity")
        })
        .collect::<Vec<_>>();
    let prepare_qc = certify_and_persist_consensus_v2_votes(
        &data_dir,
        consensus_proposal.round,
        postfiat_types::ConsensusV2Phase::Prepare,
        Some(consensus_proposal.block.clone()),
        prepare_votes,
    )
    .expect("build healthy prepare QC before storage ambiguity");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("read validator keys");
    let validator = validator_keys
        .validators
        .iter()
        .find(|record| record.node_id == "validator-0")
        .expect("find validator-0 key")
        .clone();
    let split_key_file = data_dir.join("validator-0.vote-block.private.json");
    write_validator_key_file(
        &split_key_file,
        &ValidatorKeyFile {
            validators: vec![validator],
        },
    )
    .expect("write split vote-block key");
    drop(store);
    let sweep_dir = unique_test_dir("postfiat-active-storage-vote-block-sweep");
    let sweep_store = NodeStore::new(&sweep_dir);
    drop(
        sweep_store
            .transactional_store()
            .expect("sweep active database handle before raw corruption"),
    );
    drop(sweep_store);

    let database_path = data_dir.join(
        postfiat_storage::transactional::TRANSACTIONAL_DATABASE_FILE,
    );
    let mut corrupted = std::fs::read(&database_path).expect("read active database");
    let positions = corrupted
        .windows(first_batch.batch_id.len())
        .enumerate()
        .filter_map(|(offset, candidate)| {
            (candidate == first_batch.batch_id.as_bytes()).then_some(offset)
        })
        .collect::<Vec<_>>();
    assert!(
        positions.len() >= 3,
        "expected active batch ID in multiple authenticated tables"
    );
    for position in &positions {
        corrupted[position + first_batch.batch_id.len() / 2] ^= 1;
    }
    std::fs::write(&database_path, &corrupted).expect("corrupt active transactional state");

    let vote_file = data_dir.join("ambiguous-storage.block-vote.json");
    let error = create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: split_key_file.clone(),
        validator_id: None,
        batch_file: Some(second_batch_file.clone()),
        proposal_file: Some(proposal_file.clone()),
        timeout_certificate_file: None,
        block_height: Some(proposal.block_height),
        vote_file: vote_file.clone(),
    })
    .expect_err("ambiguous active storage must block voting");
    assert!(
        error.to_string().contains("storage_database_error")
            || error.to_string().contains("storage_integrity_failure")
            || error
                .to_string()
                .contains("storage_vote_blocked_ambiguous_local_state"),
        "{error}"
    );
    assert!(!vote_file.exists(), "a vote was durably emitted from ambiguous storage");

    let verified_vote_file = data_dir.join("ambiguous-storage-verified.block-vote.json");
    let verified_error = create_block_vote_for_verified_proposal(
        BlockVoteForVerifiedProposalOptions {
            data_dir: data_dir.clone(),
            verify_block_log: false,
            key_file: split_key_file.clone(),
            validator_id: Some("validator-0".to_owned()),
            proposal: proposal.clone(),
            block_height: Some(proposal.block_height),
            vote_file: verified_vote_file.clone(),
        },
    )
    .expect_err("preverified proposal must not bypass the storage vote guard");
    assert!(
        verified_error
            .to_string()
            .contains("storage_vote_blocked_ambiguous_local_state"),
        "{verified_error}"
    );
    assert!(
        !verified_vote_file.exists(),
        "the preverified-proposal path emitted a vote from ambiguous storage"
    );

    let mut unsigned_proposal = proposal.clone();
    unsigned_proposal.signature = None;
    let proposal_sign_error = sign_verified_block_proposal(
        &data_dir,
        unsigned_proposal,
        &split_key_file,
        "validator-0",
    )
    .expect_err("block proposal signing must not bypass the storage vote guard");
    assert!(
        proposal_sign_error
            .to_string()
            .contains("storage_vote_blocked_ambiguous_local_state"),
        "{proposal_sign_error}"
    );

    let timeout_vote_file = data_dir.join("ambiguous-storage.timeout-vote.json");
    let timeout_error = create_block_timeout_vote(BlockTimeoutVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: false,
        key_file: split_key_file.clone(),
        validator_id: Some("validator-0".to_owned()),
        block_height: proposal.block_height,
        view: proposal.view,
        high_qc_id: "ambiguous-storage-high-qc".to_owned(),
        vote_file: timeout_vote_file.clone(),
    })
    .expect_err("timeout vote signing must not bypass the storage vote guard");
    assert!(
        timeout_error
            .to_string()
            .contains("storage_vote_blocked_ambiguous_local_state"),
        "{timeout_error}"
    );
    assert!(
        !timeout_vote_file.exists(),
        "the timeout path emitted a vote from ambiguous storage"
    );

    let consensus_error = create_consensus_v2_proposal_for_block(
        &data_dir,
        &proposal,
        None,
        &split_key_file,
    )
    .expect_err("consensus-v2 proposal signing must block on ambiguous storage");
    assert!(
        consensus_error
            .to_string()
            .contains("storage_vote_blocked_ambiguous_local_state"),
        "{consensus_error}"
    );

    let prepare_error = create_consensus_v2_prepare_vote(
        &data_dir,
        &consensus_proposal,
        None,
        &split_key_file,
        "validator-0",
    )
    .expect_err("consensus-v2 prepare voting must block on ambiguous storage");
    assert!(
        prepare_error
            .to_string()
            .contains("storage_vote_blocked_ambiguous_local_state"),
        "{prepare_error}"
    );

    let precommit_error = create_consensus_v2_precommit_vote(
        &data_dir,
        &prepare_qc,
        &split_key_file,
        "validator-0",
    )
    .expect_err("consensus-v2 precommit voting must block on ambiguous storage");
    assert!(
        precommit_error
            .to_string()
            .contains("storage_vote_blocked_ambiguous_local_state"),
        "{precommit_error}"
    );

    let consensus_timeout_error = create_consensus_v2_timeout_vote(
        &data_dir,
        consensus_proposal.round,
        &split_key_file,
        "validator-0",
    )
    .expect_err("consensus-v2 timeout voting must block on ambiguous storage");
    assert!(
        consensus_timeout_error
            .to_string()
            .contains("storage_vote_blocked_ambiguous_local_state"),
        "{consensus_timeout_error}"
    );

    // The bounded writer lease opens and closes the database around every
    // attempt, and redb rewrites shutdown and allocator bookkeeping on each
    // cycle, so whole-file byte identity no longer holds. The settled
    // recovery state is instead pinned directly: every deliberately
    // corrupted byte must remain corrupted (no repair, no healing, no
    // content mutation) across repeated blocked votes.
    let assert_corruption_settled = |label: &str| {
        let current = std::fs::read(&database_path).expect(label);
        for position in &positions {
            assert_eq!(
                current[position + first_batch.batch_id.len() / 2],
                corrupted[position + first_batch.batch_id.len() / 2],
                "blocked voting mutated the ambiguous corrupted state at byte {position}"
            );
        }
    };
    assert_corruption_settled("read database after first blocked vote");
    let second_vote_file = data_dir.join("ambiguous-storage-second.block-vote.json");
    let second_error = create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: split_key_file,
        validator_id: None,
        batch_file: Some(second_batch_file),
        proposal_file: Some(proposal_file),
        timeout_certificate_file: None,
        block_height: Some(proposal.block_height),
        vote_file: second_vote_file.clone(),
    })
    .expect_err("repeated vote attempt from ambiguous storage must remain blocked");
    assert!(
        second_error.to_string().contains("storage_database_error")
            || second_error.to_string().contains("storage_integrity_failure")
            || second_error
                .to_string()
                .contains("storage_vote_blocked_ambiguous_local_state"),
        "{second_error}"
    );
    assert!(
        !second_vote_file.exists(),
        "a repeated attempt emitted a vote from ambiguous storage"
    );
    assert_corruption_settled("read database after repeated blocked vote");

    std::fs::remove_dir_all(data_dir).expect("remove active-storage vote-block fixture");
    std::fs::remove_dir_all(sweep_dir).expect("remove active-storage sweep fixture");
}
