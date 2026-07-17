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
    let ordered_batches = store
        .read_ordered_batches()
        .expect("read activation ordered batches");
    let shielded = store.read_shielded().expect("read activation shielded state");
    let bridge = store.read_bridge().expect("read activation bridge state");
    let tip = store.read_chain_tip().expect("read activation chain tip");
    let validator_keys = read_validator_key_file(&store.data_dir().join(VALIDATOR_KEYS_FILE))
        .expect("read activation validator keys");
    let certificate_validators =
        active_validator_ids(governance).expect("read activation validator ids");
    let commit = prepare_ordered_commit(OrderedCommitPlan {
        genesis,
        governance,
        ledger,
        ordered_batches: &ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height: tip.height.checked_add(1).expect("activation height overflow"),
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
