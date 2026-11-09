//! The v3 account history index bounds the per-commit write: each touched
//! account gets its last chunk and head rewritten, and the metadata files
//! hold counts and ranges only. A full rebuild writes the same files as the
//! commit-time appends, a replayed commit adds nothing twice, and a v2 index
//! is reported and never read or extended until `account-tx-index-build`
//! rebuilds it.

use super::*;

const RECIPIENT_A: &str = "pf00000000000000000000000000000000000000a1";
const RECIPIENT_B: &str = "pf00000000000000000000000000000000000000b2";
const RECIPIENT_C: &str = "pf00000000000000000000000000000000000000c3";
/// Enough rows for the faucet to cross one 64-row chunk boundary.
const FOLLOWED_COMMITS: usize = 66;

fn chain(label: &str) -> PathBuf {
    let data_dir = unique_test_dir(label);
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    data_dir
}

fn commit_transfer(data_dir: &Path, to: &str) {
    let batch_file = data_dir.join("transfer.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.to_path_buf(),
        key_file: None,
        to: to.to_string(),
        amount: ACCOUNT_RESERVE,
        batch_file: batch_file.clone(),
    })
    .expect("build transfer batch");
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file: batch_file.clone(),
        certificate_file: None,
    })
    .expect("commit batch");
    assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
    fs::remove_file(batch_file).expect("remove batch file");
}

fn build(data_dir: &Path) -> AccountTxIndexBuildReport {
    rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.to_path_buf(),
    })
    .expect("account tx index build")
}

fn index_status(data_dir: &Path) -> AccountTxIndexStatusReport {
    account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.to_path_buf(),
    })
    .expect("account tx index status")
}

fn query(data_dir: &Path, address: &str) -> AccountTxQueryOptions {
    AccountTxQueryOptions {
        data_dir: data_dir.to_path_buf(),
        address: address.to_string(),
        from_height: None,
        to_height: None,
        limit: Some(500),
    }
}

fn scanned(data_dir: &Path, address: &str) -> Vec<AccountTxRow> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let block_log = store.read_blocks().expect("blocks");
    account_tx_scan(query(data_dir, address), 500, &store, &genesis, &block_log)
        .expect("archive scan")
        .rows
}

fn assert_served_from_index(data_dir: &Path, address: &str) -> AccountTxReport {
    let report = account_tx(query(data_dir, address)).expect("account tx");
    assert!(report.index_used, "{address} not served from the index: {report:?}");
    assert_eq!(report.archive_lookup_count, 0);
    assert_eq!(report.rows, scanned(data_dir, address), "{address}");
    report
}

/// A chain built once, then followed by `FOLLOWED_COMMITS` commits. Returns
/// the faucet address.
fn followed_chain(data_dir: &Path) -> String {
    commit_transfer(data_dir, RECIPIENT_A);
    build(data_dir);
    for index in 0..FOLLOWED_COMMITS {
        let to = [RECIPIENT_A, RECIPIENT_B, RECIPIENT_C][index % 3];
        commit_transfer(data_dir, to);
    }
    let report = account_tx(query(data_dir, RECIPIENT_A)).expect("account tx");
    report.rows[0].from_address.clone()
}

fn collect_files(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let entry = entry.expect("entry");
        let path = entry.path();
        if entry.file_type().expect("file type").is_dir() {
            collect_files(root, &path, files);
        } else {
            let relative = path.strip_prefix(root).expect("relative path");
            files.insert(
                relative.display().to_string(),
                fs::read(&path).expect("read index file"),
            );
        }
    }
}

/// Every index file (aggregate, disk metadata, account files) by path.
fn index_files(data_dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    for file in [ACCOUNT_TX_INDEX_FILE, ACCOUNT_TX_DISK_INDEX_FILE] {
        if let Ok(bytes) = fs::read(data_dir.join(file)) {
            files.insert(file.to_string(), bytes);
        }
    }
    collect_files(
        data_dir,
        &data_dir.join(ACCOUNT_TX_ACCOUNT_SHARDS_DIR),
        &mut files,
    );
    files
}

fn changed_files(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> BTreeMap<String, Vec<u8>> {
    after
        .iter()
        .filter(|(path, bytes)| before.get(*path) != Some(*bytes))
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect()
}

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("index file is JSON")
}

fn shard_dir_name(address: &str) -> String {
    hash_hex("postfiat.account_tx.address_shard.v1", address.as_bytes())
}

fn remove_index(data_dir: &Path) {
    for file in [ACCOUNT_TX_INDEX_FILE, ACCOUNT_TX_DISK_INDEX_FILE] {
        let _ = fs::remove_file(data_dir.join(file));
    }
    let _ = fs::remove_dir_all(data_dir.join(ACCOUNT_TX_ACCOUNT_SHARDS_DIR));
}

#[test]
fn a_commit_rewrites_only_the_last_chunk_of_each_touched_account_and_the_metadata() {
    let data_dir = chain("account-tx-chunked-commit-write");
    let faucet = followed_chain(&data_dir);
    let before = index_files(&data_dir);
    let history_bytes: usize = before.values().map(Vec::len).sum();
    commit_transfer(&data_dir, RECIPIENT_B);
    let after = index_files(&data_dir);
    let changed = changed_files(&before, &after);
    let written: usize = changed.values().map(Vec::len).sum();
    eprintln!(
        "per-commit index write after {} rows: {written} bytes in {} files (index total {history_bytes} bytes): {:?}",
        FOLLOWED_COMMITS + 2,
        changed.len(),
        changed
            .iter()
            .map(|(path, bytes)| format!("{path}={}", bytes.len()))
            .collect::<Vec<_>>()
    );

    // The two metadata files hold counts and ranges, not rows or account lists.
    let aggregate = json(&changed[ACCOUNT_TX_INDEX_FILE]);
    let meta = json(&changed[ACCOUNT_TX_DISK_INDEX_FILE]);
    for metadata in [&aggregate, &meta] {
        assert!(metadata.get("accounts").is_none(), "{metadata}");
    }
    let chunk_rows = meta["chunk_rows"].as_u64().expect("chunk_rows");
    assert_eq!(chunk_rows, 64);

    // Per touched account (the faucet and the recipient): its head and its
    // last chunk, nothing else.
    let faucet_dir = shard_dir_name(&faucet);
    let recipient_dir = shard_dir_name(RECIPIENT_B);
    let mut account_files = changed
        .keys()
        .filter(|path| path.starts_with(ACCOUNT_TX_ACCOUNT_SHARDS_DIR))
        .cloned()
        .collect::<Vec<_>>();
    account_files.sort();
    let faucet_last_chunk =
        format!("{ACCOUNT_TX_ACCOUNT_SHARDS_DIR}/{faucet_dir}/chunk-00000001.json");
    let mut expected = vec![
        faucet_last_chunk.clone(),
        format!("{ACCOUNT_TX_ACCOUNT_SHARDS_DIR}/{faucet_dir}/head.json"),
        format!("{ACCOUNT_TX_ACCOUNT_SHARDS_DIR}/{recipient_dir}/chunk-00000000.json"),
        format!("{ACCOUNT_TX_ACCOUNT_SHARDS_DIR}/{recipient_dir}/head.json"),
    ];
    expected.sort();
    assert_eq!(account_files, expected);
    let faucet_rows = FOLLOWED_COMMITS as u64 + 2;
    let last_chunk = json(&changed[&faucet_last_chunk]);
    assert_eq!(
        last_chunk["rows"].as_array().expect("rows").len() as u64,
        faucet_rows - chunk_rows
    );
    for path in &account_files {
        let file = json(&changed[path]);
        if let Some(rows) = file.get("rows") {
            assert!(rows.as_array().expect("rows").len() as u64 <= chunk_rows);
        }
    }
    assert_eq!(
        assert_served_from_index(&data_dir, &faucet).rows.len() as u64,
        faucet_rows
    );

    fs::remove_dir_all(data_dir).expect("remove test data dir");
}

#[test]
fn a_full_rebuild_writes_the_same_chunks_as_appends_over_the_same_blocks() {
    let data_dir = chain("account-tx-chunked-rebuild-equal");
    let faucet = followed_chain(&data_dir);
    let followed = index_files(&data_dir);
    let followed_history = [faucet.as_str(), RECIPIENT_A, RECIPIENT_B, RECIPIENT_C]
        .map(|address| assert_served_from_index(&data_dir, address).rows);

    remove_index(&data_dir);
    let rebuilt_report = build(&data_dir);
    assert!(rebuilt_report.index_usable && rebuilt_report.disk_index_usable);
    let rebuilt = index_files(&data_dir);
    assert_eq!(
        followed.keys().collect::<Vec<_>>(),
        rebuilt.keys().collect::<Vec<_>>()
    );
    for (path, bytes) in &followed {
        assert!(rebuilt[path] == *bytes, "{path} differs after a full rebuild");
    }
    let rebuilt_history = [faucet.as_str(), RECIPIENT_A, RECIPIENT_B, RECIPIENT_C]
        .map(|address| assert_served_from_index(&data_dir, address).rows);
    assert_eq!(followed_history, rebuilt_history);

    fs::remove_dir_all(data_dir).expect("remove test data dir");
}

#[test]
fn a_restart_replays_the_last_block_without_duplicating_rows() {
    let data_dir = chain("account-tx-chunked-restart-replay");
    let faucet = followed_chain(&data_dir);
    let metadata = [ACCOUNT_TX_INDEX_FILE, ACCOUNT_TX_DISK_INDEX_FILE]
        .map(|file| fs::read(data_dir.join(file)).expect("metadata"));

    // The commit wrote the account files, then the node stopped before the
    // metadata writes.
    commit_transfer(&data_dir, RECIPIENT_B);
    for (file, bytes) in [ACCOUNT_TX_INDEX_FILE, ACCOUNT_TX_DISK_INDEX_FILE]
        .into_iter()
        .zip(&metadata)
    {
        fs::write(data_dir.join(file), bytes).expect("restore metadata");
    }
    assert_eq!(index_status(&data_dir).disk_index_lag_blocks, Some(1));

    // Journal recovery at restart replays the append of the tip block.
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let tip = store.read_blocks().expect("blocks").blocks.last().cloned().expect("tip");
    let archive = store.read_batch_archive().expect("archive");
    let archive_entry = archive
        .find(&tip.header.batch_kind, &tip.header.batch_id)
        .cloned()
        .expect("tip archive entry");
    let receipts = store
        .read_receipts()
        .expect("receipts")
        .into_iter()
        .filter(|receipt| tip.receipt_ids.contains(&receipt.tx_id))
        .collect::<Vec<_>>();
    let replayed =
        append_account_tx_index_after_commit(&store, &genesis, &tip, &archive_entry, &receipts)
            .expect("replay append");
    assert_eq!(replayed, AccountTxIndexAppend::Appended);
    let after_replay = index_files(&data_dir);
    let again =
        append_account_tx_index_after_commit(&store, &genesis, &tip, &archive_entry, &receipts)
            .expect("second replay");
    assert_eq!(again, AccountTxIndexAppend::AlreadyCurrent);
    assert_eq!(index_files(&data_dir), after_replay);

    let status = index_status(&data_dir);
    assert!(status.index_usable && status.disk_index_usable, "{status:?}");
    for address in [faucet.as_str(), RECIPIENT_A, RECIPIENT_B, RECIPIENT_C] {
        assert_served_from_index(&data_dir, address);
    }

    // The replayed index is the one a full rebuild writes.
    remove_index(&data_dir);
    build(&data_dir);
    assert_eq!(index_files(&data_dir), after_replay);

    fs::remove_dir_all(data_dir).expect("remove test data dir");
}

/// Writes the index exactly as the v2 binary left it: an aggregate holding
/// every row, a disk metadata file listing every account, and one whole
/// shard file per account. Returns the v2 shard rows by account.
pub(super) fn write_v2_account_tx_index(data_dir: &Path) -> BTreeMap<String, Vec<AccountTxRow>> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let block_log = store.read_blocks().expect("blocks");
    let archive = store.read_batch_archive().expect("archive");
    let receipts = store.read_receipts().expect("receipts");
    let receipt_by_tx = receipt_by_tx_id(&receipts, &block_log).expect("receipts by tx");
    let mut accounts: BTreeMap<String, Vec<AccountTxRow>> = BTreeMap::new();
    let (mut from, mut to, mut blocks, mut rows) = (None, None, 0_u64, 0_u64);
    for block in &block_log.blocks {
        if !matches!(
            block.header.batch_kind.as_str(),
            BATCH_KIND_TRANSPARENT | BATCH_KIND_SHIELDED
        ) {
            continue;
        }
        from.get_or_insert(block.header.height);
        to = Some(block.header.height);
        blocks += 1;
        for row in account_tx_rows_for_block(block, &archive, &receipt_by_tx).expect("rows") {
            rows += 1;
            accounts
                .entry(row.from_address.clone())
                .or_default()
                .push(row.clone());
            if row.to_address != row.from_address {
                accounts.entry(row.to_address.clone()).or_default().push(row);
            }
        }
    }
    let tip_hash = block_log.tip_hash();
    let domain = serde_json::json!({
        "chain_id": genesis.chain_id,
        "genesis_hash": genesis_hash(&genesis),
        "protocol_version": genesis.protocol_version,
    });
    let with_domain = |mut value: serde_json::Value| {
        for (key, field) in domain.as_object().expect("domain") {
            value[key] = field.clone();
        }
        value
    };
    let shards = data_dir.join(ACCOUNT_TX_ACCOUNT_SHARDS_DIR);
    fs::create_dir_all(&shards).expect("shards dir");
    let mut entries = serde_json::Map::new();
    for (address, account_rows) in &accounts {
        let shard_file = format!("{}.json", shard_dir_name(address));
        let shard = with_domain(serde_json::json!({
            "schema": "postfiat-account-tx-account-shard-v2",
            "address": address,
            "tip_hash": tip_hash,
            "row_count": account_rows.len(),
            "rows": account_rows,
        }));
        fs::write(shards.join(&shard_file), serde_json::to_vec_pretty(&shard).expect("shard"))
            .expect("write v2 shard");
        entries.insert(
            address.clone(),
            serde_json::json!({
                "shard_file": shard_file,
                "row_count": account_rows.len(),
                "tip_hash": tip_hash,
            }),
        );
    }
    let counts = serde_json::json!({
        "indexed_from_height": from,
        "indexed_to_height": to,
        "indexed_block_count": blocks,
        "indexed_row_count": rows,
        "account_count": accounts.len(),
        "tip_hash": tip_hash,
    });
    let mut aggregate = with_domain(counts.clone());
    aggregate["schema"] = "postfiat-account-tx-index-v2".into();
    aggregate["accounts"] = serde_json::to_value(&accounts).expect("accounts");
    let mut meta = with_domain(counts);
    meta["schema"] = "postfiat-account-tx-disk-index-v2".into();
    meta["shard_dir"] = ACCOUNT_TX_ACCOUNT_SHARDS_DIR.into();
    meta["accounts"] = serde_json::Value::Object(entries);
    fs::write(
        data_dir.join(ACCOUNT_TX_INDEX_FILE),
        serde_json::to_vec_pretty(&aggregate).expect("aggregate"),
    )
    .expect("write v2 aggregate");
    fs::write(
        data_dir.join(ACCOUNT_TX_DISK_INDEX_FILE),
        serde_json::to_vec_pretty(&meta).expect("meta"),
    )
    .expect("write v2 meta");
    accounts
}

#[test]
fn an_older_schema_index_is_reported_and_not_read_until_rebuilt() {
    let data_dir = chain("account-tx-chunked-v2-upgrade");
    commit_transfer(&data_dir, RECIPIENT_A);
    commit_transfer(&data_dir, RECIPIENT_B);
    remove_index(&data_dir);
    write_v2_account_tx_index(&data_dir);
    let v2_files = index_files(&data_dir);

    let status = index_status(&data_dir);
    assert!(status.index_present && !status.index_usable, "{status:?}");
    assert_eq!(
        status.reason.as_deref(),
        Some("unsupported account_tx index schema `postfiat-account-tx-index-v2`")
    );
    assert!(status.disk_index_present && !status.disk_index_usable, "{status:?}");
    assert_eq!(
        status.disk_index_reason.as_deref(),
        Some("unsupported account_tx disk index schema `postfiat-account-tx-disk-index-v2`")
    );
    let before = account_tx(query(&data_dir, RECIPIENT_A)).expect("account tx");
    assert!(!before.index_used, "a v2 index answered: {before:?}");
    assert_eq!(before.rows, scanned(&data_dir, RECIPIENT_A));

    // A commit does not extend the v2 index.
    commit_transfer(&data_dir, RECIPIENT_C);
    assert_eq!(index_files(&data_dir), v2_files);
    assert!(!account_tx(query(&data_dir, RECIPIENT_C)).expect("account tx").index_used);

    // The build rebuilds it as v4, and the v2 shard files are gone.
    let rebuilt = build(&data_dir);
    assert!(rebuilt.index_usable && rebuilt.disk_index_usable, "{rebuilt:?}");
    let meta = json(&fs::read(data_dir.join(ACCOUNT_TX_DISK_INDEX_FILE)).expect("meta"));
    assert_eq!(meta["schema"], "postfiat-account-tx-disk-index-v4");
    assert!(index_files(&data_dir).keys().all(|path| !path.ends_with(".json")
        || !path.starts_with(ACCOUNT_TX_ACCOUNT_SHARDS_DIR)
        || path.matches('/').count() == 2));
    for address in [RECIPIENT_A, RECIPIENT_B, RECIPIENT_C] {
        assert_served_from_index(&data_dir, address);
    }

    fs::remove_dir_all(data_dir).expect("remove test data dir");
}
