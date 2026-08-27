fn rewrite_authenticated_meta(
    store: &TransactionalStore,
    rewrite: impl FnOnce(&mut TransactionalStoreMetaV1),
) {
    let transaction = store
        .begin_durable_write()
        .expect("begin authenticated metadata tamper");
    let mut table = transaction.open_table(META).expect("open metadata table");
    let mut meta = read_meta(&table, &store.integrity_key, &store.counters)
        .expect("read metadata before tamper");
    rewrite(&mut meta);
    insert_authenticated(
        &mut table,
        META_TABLE,
        META_KEY,
        &encode_json(&meta, MAX_META_BYTES).expect("encode tampered metadata"),
        MAX_META_BYTES,
        &store.integrity_key,
        &store.counters,
    )
    .expect("write authenticated metadata tamper");
    drop(table);
    store
        .commit_durable_write(transaction)
        .expect("commit authenticated metadata tamper");
}

fn assert_failed_scan_does_not_rewrite_database(
    store: &TransactionalStore,
    expected_code: StorageErrorCode,
) {
    let before = fs::read(store.database_path()).expect("read database before rejected scan");
    let error = store
        .verify_and_mark_full_integrity()
        .expect_err("tampered database must fail closed");
    assert_eq!(error.code(), expected_code, "{error}");
    assert_eq!(
        fs::read(store.database_path()).expect("read database after rejected scan"),
        before,
        "a rejected integrity scan changed the database"
    );
}

#[test]
fn authenticated_stale_metadata_tip_count_and_accumulator_fail_without_marker() {
    let cases = [
        ("wrong-storage-domain", StorageErrorCode::UnsupportedSchema),
        (
            "wrong-chain-domain",
            StorageErrorCode::OrderedCommitmentMismatch,
        ),
        (
            "wrong-genesis-domain",
            StorageErrorCode::OrderedCommitmentMismatch,
        ),
        (
            "wrong-protocol-domain",
            StorageErrorCode::OrderedCommitmentMismatch,
        ),
        (
            "wrong-commitment-domain",
            StorageErrorCode::UnsupportedSchema,
        ),
        ("stale-metadata-height", StorageErrorCode::CountMismatch),
        ("future-full-verification", StorageErrorCode::CountMismatch),
        ("stale-tip", StorageErrorCode::CorruptRecord),
        (
            "stale-ordered-accumulator",
            StorageErrorCode::OrderedCommitmentMismatch,
        ),
        ("incorrect-ordered-count", StorageErrorCode::CountMismatch),
    ];

    for (label, expected_code) in cases {
        let (_dir, store) = committed_one_block(label);
        rewrite_authenticated_meta(&store, |meta| match label {
            "wrong-storage-domain" => meta.storage_format = "postfiat-redb-v0".to_owned(),
            "wrong-chain-domain" => meta.chain_id = "substituted-chain".to_owned(),
            "wrong-genesis-domain" => meta.genesis_hash = "substituted-genesis".to_owned(),
            "wrong-protocol-domain" => {
                meta.protocol_version = meta.protocol_version.saturating_add(1);
            }
            "wrong-commitment-domain" => {
                meta.ordered_history_schema = "postfiat-ordered-history-v0".to_owned();
            }
            "stale-metadata-height" => meta.finalized_height = 0,
            "future-full-verification" => {
                meta.last_full_verification_height = Some(meta.finalized_height + 1);
            }
            "stale-tip" => meta.finalized_block_hash = "genesis".to_owned(),
            "stale-ordered-accumulator" => {
                meta.ordered_history_accumulator = "aa".repeat(48);
            }
            "incorrect-ordered-count" => meta.ordered_batch_count = 2,
            _ => unreachable!(),
        });
        assert_failed_scan_does_not_rewrite_database(&store, expected_code);
    }
}

#[test]
fn missing_substituted_and_one_sided_indexes_fail_without_mutation() {
    for label in [
        "missing-table",
        "substituted-table-value",
        "index-without-history",
        "history-without-index",
    ] {
        let (_dir, store) = committed_one_block(label);
        let transaction = store
            .begin_durable_write()
            .expect("begin table/index tamper");
        match label {
            "missing-table" => {
                transaction
                    .delete_table(ORDERED_BY_ID)
                    .expect("delete ordered-ID table");
            }
            "substituted-table-value" => {
                let source = transaction
                    .open_table(ORDERED_BY_ID)
                    .expect("open substitution source");
                let raw = source
                    .get(b"batch-1".as_slice())
                    .expect("read substitution source")
                    .expect("substitution source exists")
                    .value()
                    .to_vec();
                drop(source);
                let mut target = transaction
                    .open_table(ORDERED_BY_ORDINAL)
                    .expect("open substitution target");
                target
                    .insert(ordered_u64_key(1).as_slice(), raw.as_slice())
                    .expect("substitute cross-table value");
            }
            "index-without-history" => {
                let orphan = StoredOrderedIdV1 {
                    schema: STORED_ORDERED_ID_SCHEMA.to_owned(),
                    ordinal: 2,
                    finalized_height: 2,
                };
                let mut table = transaction
                    .open_table(ORDERED_BY_ID)
                    .expect("open orphan-index table");
                insert_authenticated(
                    &mut table,
                    ORDERED_BY_ID_TABLE,
                    b"orphan-batch",
                    &encode_json(&orphan, MAX_RECORD_BYTES).expect("encode orphan index"),
                    MAX_RECORD_BYTES,
                    &store.integrity_key,
                    &store.counters,
                )
                .expect("insert index without history");
            }
            "history-without-index" => {
                let mut table = transaction
                    .open_table(ORDERED_BY_ID)
                    .expect("open missing-index table");
                table
                    .remove(b"batch-1".as_slice())
                    .expect("remove index for canonical history");
            }
            _ => unreachable!(),
        }
        store
            .commit_durable_write(transaction)
            .expect("commit table/index tamper");
        let expected_code = match label {
            "missing-table" => StorageErrorCode::Database,
            "substituted-table-value" => StorageErrorCode::IntegrityFailure,
            "index-without-history" => StorageErrorCode::CountMismatch,
            "history-without-index" => StorageErrorCode::CorruptRecord,
            _ => unreachable!(),
        };
        assert_failed_scan_does_not_rewrite_database(&store, expected_code);
    }
}

#[test]
fn generation_pointer_is_bound_to_the_verified_database_manifest() {
    let node_dir = TestDir::new("pointer-binding-node");
    let generation_dir = TestDir::new("pointer-binding-generation");
    let node_store = NodeStore::new(&node_dir.0);
    let target = node_store
        .open_transactional_store_at(&generation_dir.0)
        .expect("open pointer target");
    let tip = genesis_tip();
    let commitment = OrderedHistoryCommitment::genesis(
        &tip.chain_id,
        &tip.genesis_hash,
        tip.protocol_version,
    )
    .expect("pointer genesis commitment");
    target
        .initialize(&tip, &commitment, CurrentStateUpdate::default())
        .expect("initialize pointer target");
    let original_root = "11".repeat(48);
    target
        .verify_and_bind_migration(&original_root)
        .expect("bind pointer target");
    drop(target);
    let mut pointer = node_store
        .publish_transactional_generation(&generation_dir.0, &original_root)
        .expect("publish pointer target");
    drop(node_store);

    let pointer_path = node_dir.0.join(TRANSACTIONAL_GENERATION_POINTER_FILE);
    fs::remove_file(&pointer_path).expect("remove generation pointer");
    let missing_pointer_store = NodeStore::new(&node_dir.0);
    assert!(
        !missing_pointer_store
            .transactional_storage_configured()
            .expect("missing pointer must not select the external generation"),
        "a missing pointer fell back to an unbound transactional generation"
    );
    assert!(
        !node_dir.0.join(TRANSACTIONAL_DATABASE_FILE).exists(),
        "checking a missing pointer created an empty fallback database"
    );
    missing_pointer_store
        .write_json(pointer_path.clone(), &pointer)
        .expect("restore authenticated generation pointer");
    drop(missing_pointer_store);

    pointer.migration_packet_root = "22".repeat(48);
    let writer = NodeStore::new(&node_dir.0);
    writer
        .write_json(
            node_dir.0.join(TRANSACTIONAL_GENERATION_POINTER_FILE),
            &pointer,
        )
        .expect("write authenticated substituted pointer");
    drop(writer);

    let pointer_path = node_dir.0.join(TRANSACTIONAL_GENERATION_POINTER_FILE);
    let before = fs::read(&pointer_path).expect("read substituted pointer");
    let error = NodeStore::new(&node_dir.0)
        .transactional_store()
        .expect_err("pointer/database migration-root mismatch must reject");
    assert_eq!(error.code(), StorageErrorCode::IntegrityFailure, "{error}");
    assert_eq!(
        fs::read(pointer_path).expect("read pointer after rejection"),
        before,
        "rejected pointer substitution changed durable state"
    );
}

#[test]
fn canonical_jsonl_export_rejects_missing_corrupted_and_substituted_files_without_database_mutation()
{
    let (target_dir, target_store) = committed_two_blocks("canonical-export-target");
    let target_export = target_dir.0.join("canonical-history.jsonl");
    let expected = target_store
        .write_canonical_jsonl_export(&target_export)
        .expect("write canonical target export");
    assert_eq!(
        target_store
            .verify_canonical_jsonl_export(&target_export)
            .expect("verify canonical target export"),
        expected
    );
    let expected_bytes = fs::read(&target_export).expect("read canonical target export");
    target_store
        .write_canonical_jsonl_export(&target_export)
        .expect("rewrite deterministic canonical target export");
    assert_eq!(
        fs::read(&target_export).expect("read rewritten canonical target export"),
        expected_bytes,
        "canonical export is not byte deterministic"
    );

    let database_before =
        fs::read(target_store.database_path()).expect("read target database before export tamper");
    fs::remove_file(&target_export).expect("remove canonical target export");
    let missing = target_store
        .verify_canonical_jsonl_export(&target_export)
        .expect_err("missing canonical export must reject");
    assert!(
        missing.to_string().contains("storage_canonical_export_missing"),
        "{missing}"
    );
    assert_eq!(
        fs::read(target_store.database_path()).expect("read database after missing export"),
        database_before,
        "rejecting a missing canonical export changed the database"
    );

    target_store
        .write_canonical_jsonl_export(&target_export)
        .expect("restore canonical target export");
    let mut corrupted = fs::read(&target_export).expect("read export for corruption");
    corrupted.pop();
    fs::write(&target_export, corrupted).expect("truncate canonical export footer");
    let corrupt = target_store
        .verify_canonical_jsonl_export(&target_export)
        .expect_err("corrupted canonical export must reject");
    assert!(
        corrupt
            .to_string()
            .contains("storage_canonical_export_integrity_failure"),
        "{corrupt}"
    );
    assert_eq!(
        fs::read(target_store.database_path()).expect("read database after corrupt export"),
        database_before,
        "rejecting a corrupted canonical export changed the database"
    );

    let (donor_dir, donor_store) = committed_one_block("canonical-export-donor");
    let donor_export = donor_dir.0.join("canonical-history.jsonl");
    donor_store
        .write_canonical_jsonl_export(&donor_export)
        .expect("write valid donor export");
    let substituted = target_store
        .verify_canonical_jsonl_export(&donor_export)
        .expect_err("valid foreign canonical export must reject");
    assert!(
        substituted
            .to_string()
            .contains("storage_canonical_export_substituted"),
        "{substituted}"
    );
    assert_eq!(
        fs::read(target_store.database_path()).expect("read database after substituted export"),
        database_before,
        "rejecting a substituted canonical export changed the database"
    );
}

#[test]
fn corrupted_transactional_data_pages_reject_without_logical_state() {
    let dir = TestDir::new("raw-page-corruption");
    let marker = format!("ordered-index-page-corruption-{}", "c".repeat(700));
    {
        let store = TransactionalStore::open(&dir.0).expect("open page-corruption store");
        let old_tip = genesis_tip();
        let old_commitment = OrderedHistoryCommitment::genesis(
            &old_tip.chain_id,
            &old_tip.genesis_hash,
            old_tip.protocol_version,
        )
        .expect("page-corruption genesis commitment");
        store
            .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
            .expect("initialize page-corruption store");
        let accepted = receipt("page-corruption-receipt", true);
        let new_tip = next_tip(1);
        let new_commitment = old_commitment
            .append(&marker)
            .expect("append page-corruption marker");
        let mut block = block(vec![accepted.tx_id.clone()]);
        block.header.batch_id = marker.clone();
        let mut archived = archive();
        archived.batch_id = marker.clone();
        store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &old_tip,
                new_tip: &new_tip,
                block: &block,
                receipts: std::slice::from_ref(&accepted),
                archive_entry: &archived,
                batch_id: &marker,
                ordered_history: &new_commitment,
                current_state: CurrentStateUpdate::default(),
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect("commit page-corruption marker");
    }

    let database_path = dir.0.join(TRANSACTIONAL_DATABASE_FILE);
    let mut bytes = fs::read(&database_path).expect("read database for page corruption");
    let positions = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == marker.as_bytes()).then_some(offset))
        .collect::<Vec<_>>();
    assert!(
        positions.len() >= 4,
        "expected marker in block, archive, and ordered-index pages, found {} copies",
        positions.len()
    );
    for position in positions {
        bytes[position + marker.len() / 2] ^= 1;
    }
    fs::write(&database_path, bytes).expect("corrupt transactional data pages");

    let reopened = match TransactionalStore::open(&dir.0) {
        Ok(store) => store,
        Err(error) => {
            assert_eq!(error.code(), StorageErrorCode::Database, "{error}");
            return;
        }
    };
    let mut reopened = reopened;
    match reopened.check_database_integrity() {
        Err(error) => assert_eq!(error.code(), StorageErrorCode::Database, "{error}"),
        Ok(false) => {}
        Ok(true) => match reopened.verify_logical_integrity() {
            Err(error) => assert!(
                matches!(
                    error.code(),
                    StorageErrorCode::Database
                        | StorageErrorCode::IntegrityFailure
                        | StorageErrorCode::CorruptRecord
                        | StorageErrorCode::CountMismatch
                        | StorageErrorCode::OrderedCommitmentMismatch
                ),
                "{error}"
            ),
            Ok(report) => panic!(
                "raw page corruption must reject instead of opening a logical tip: {report:?}"
            ),
        },
    }
}

fn committed_page_damage_fixture(label: &str, marker: &str) -> TestDir {
    let dir = TestDir::new(label);
    let store = TransactionalStore::open(&dir.0).expect("open page-damage store");
    let old_tip = genesis_tip();
    let old_commitment = OrderedHistoryCommitment::genesis(
        &old_tip.chain_id,
        &old_tip.genesis_hash,
        old_tip.protocol_version,
    )
    .expect("page-damage genesis commitment");
    store
        .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
        .expect("initialize page-damage store");
    let accepted = receipt("page-damage-receipt", true);
    let new_tip = next_tip(1);
    let new_commitment = old_commitment
        .append(marker)
        .expect("append page-damage marker");
    let mut block = block(vec![accepted.tx_id.clone()]);
    block.header.batch_id = marker.to_owned();
    let mut archived = archive();
    archived.batch_id = marker.to_owned();
    store
        .commit_finalized_block(CommitFinalizedBlock {
            expected_tip: &old_tip,
            new_tip: &new_tip,
            block: &block,
            receipts: std::slice::from_ref(&accepted),
            archive_entry: &archived,
            batch_id: marker,
            ordered_history: &new_commitment,
            current_state: CurrentStateUpdate::default(),
            scheduled_activation_height: None,
            allow_legacy_receipt_id_mismatch: false,
        })
        .expect("commit page-damage marker");
    drop(store);
    dir
}

fn raw_marker_positions(bytes: &[u8], marker: &str) -> Vec<usize> {
    bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == marker.as_bytes()).then_some(offset))
        .collect()
}

fn assert_damaged_database_rejects_or_recovers_old_tip(dir: &TestDir) {
    let reopened = match TransactionalStore::open(&dir.0) {
        Ok(store) => store,
        Err(error) => {
            assert_eq!(error.code(), StorageErrorCode::Database, "{error}");
            return;
        }
    };
    let mut reopened = reopened;
    match reopened.check_database_integrity() {
        Err(error) => assert_eq!(error.code(), StorageErrorCode::Database, "{error}"),
        Ok(false) => {}
        Ok(true) => match reopened.verify_logical_integrity() {
            Err(error) => assert!(
                matches!(
                    error.code(),
                    StorageErrorCode::Database
                        | StorageErrorCode::IntegrityFailure
                        | StorageErrorCode::CorruptRecord
                        | StorageErrorCode::CountMismatch
                        | StorageErrorCode::OrderedCommitmentMismatch
                ),
                "{error}"
            ),
            Ok(report) => panic!(
                "raw page damage must reject instead of opening a logical tip: {report:?}"
            ),
        },
    }
}

#[test]
fn missing_transactional_data_page_rejects_without_logical_state() {
    const PAGE_BYTES: usize = 4096;
    let marker = format!("missing-page-marker-{}", "m".repeat(700));
    let dir = committed_page_damage_fixture("missing-page", &marker);
    let database_path = dir.0.join(TRANSACTIONAL_DATABASE_FILE);
    let mut bytes = fs::read(&database_path).expect("read database before page removal");
    let positions = raw_marker_positions(&bytes, &marker);
    assert!(positions.len() >= 4, "expected marker across logical tables");
    let removal_start = positions
        .iter()
        .map(|position| position / PAGE_BYTES * PAGE_BYTES)
        .max()
        .expect("find marker page");
    assert!(removal_start > PAGE_BYTES, "refuse to truncate the database header");
    bytes.truncate(removal_start);
    fs::write(&database_path, bytes).expect("remove transactional data page");
    assert_damaged_database_rejects_or_recovers_old_tip(&dir);
}

#[test]
fn substituted_transactional_data_page_rejects_without_logical_state() {
    const PAGE_BYTES: usize = 4096;
    let target_marker = format!("target-page-marker-{}", "t".repeat(700));
    let donor_marker = format!("donorx-page-marker-{}", "d".repeat(700));
    assert_eq!(target_marker.len(), donor_marker.len());
    let target = committed_page_damage_fixture("substituted-page-target", &target_marker);
    let donor = committed_page_damage_fixture("substituted-page-donor", &donor_marker);
    let target_path = target.0.join(TRANSACTIONAL_DATABASE_FILE);
    let donor_path = donor.0.join(TRANSACTIONAL_DATABASE_FILE);
    let mut target_bytes = fs::read(&target_path).expect("read target database");
    let donor_bytes = fs::read(&donor_path).expect("read donor database");
    let target_position = *raw_marker_positions(&target_bytes, &target_marker)
        .first()
        .expect("find target marker page");
    let donor_position = *raw_marker_positions(&donor_bytes, &donor_marker)
        .first()
        .expect("find donor marker page");
    let target_start = target_position / PAGE_BYTES * PAGE_BYTES;
    let donor_start = donor_position / PAGE_BYTES * PAGE_BYTES;
    let target_end = target_start + PAGE_BYTES;
    let donor_end = donor_start + PAGE_BYTES;
    assert!(target_end <= target_bytes.len());
    assert!(donor_end <= donor_bytes.len());
    assert_ne!(
        &target_bytes[target_start..target_end],
        &donor_bytes[donor_start..donor_end],
        "foreign page unexpectedly matches target page"
    );
    target_bytes[target_start..target_end]
        .copy_from_slice(&donor_bytes[donor_start..donor_end]);
    fs::write(&target_path, target_bytes).expect("substitute transactional data page");
    assert_damaged_database_rejects_or_recovers_old_tip(&target);
}
