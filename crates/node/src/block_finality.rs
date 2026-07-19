const BLOCK_PROPOSAL_VOTE_LOCK_DIR: &str = "block_proposal_vote_locks";
const BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1: &str = "postfiat.block_proposal_vote_lock.v1";
const BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA: &str = "postfiat.block_proposal_vote_lock.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BlockProposalVoteLock {
    schema: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    block_height: u64,
    view: u64,
    validator: String,
    proposal_hash: String,
}

#[derive(Debug)]
struct TxFinalityTransactionNotFound {
    tx_id: String,
}

impl std::fmt::Display for TxFinalityTransactionNotFound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "transaction `{}` has no receipt", self.tx_id)
    }
}

impl std::error::Error for TxFinalityTransactionNotFound {}

fn tx_finality_transaction_not_found(tx_id: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        TxFinalityTransactionNotFound {
            tx_id: tx_id.to_string(),
        },
    )
}

#[doc(hidden)]
pub fn tx_finality_error_is_transaction_not_found(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound
        && error
            .get_ref()
            .and_then(|source| source.downcast_ref::<TxFinalityTransactionNotFound>())
            .is_some()
}

pub fn tx_finality(options: TxFinalityQueryOptions) -> io::Result<TxFinalityReport> {
    validate_finality_tx_id(&options.tx_id)?;
    let tx_id = options.tx_id;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let receipt_log = store.read_receipts()?;
    let blocks = store.read_blocks()?;

    let (receipt, block, receipt_index) = if options.audit_block_log {
        let matching_receipts = receipt_log
            .iter()
            .filter(|receipt| receipt.tx_id == tx_id)
            .cloned()
            .collect::<Vec<_>>();
        let receipt = match matching_receipts.as_slice() {
            [receipt] => receipt.clone(),
            [] => {
                return Err(tx_finality_transaction_not_found(&tx_id));
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("transaction `{tx_id}` has multiple receipts"),
                ));
            }
        };

        let mut matches = Vec::new();
        for block in &blocks.blocks {
            for (index, receipt_id) in block.receipt_ids.iter().enumerate() {
                if receipt_id == &tx_id {
                    matches.push((block.clone(), index as u64));
                }
            }
        }
        let (block, receipt_index) = match matches.as_slice() {
            [(block, index)] => (block.clone(), *index),
            [] => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("transaction `{tx_id}` receipt is not linked from any verified block"),
                ));
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("transaction `{tx_id}` receipt is linked from multiple blocks"),
                ));
            }
        };
        (receipt, block, receipt_index)
    } else {
        let receipt = receipt_log
            .iter()
            .rev()
            .find(|receipt| receipt.tx_id == tx_id)
            .cloned()
            .ok_or_else(|| tx_finality_transaction_not_found(&tx_id))?;
        let (block, receipt_index) = blocks
            .blocks
            .iter()
            .rev()
            .find_map(|block| {
                block
                    .receipt_ids
                    .iter()
                    .position(|receipt_id| receipt_id == &tx_id)
                    .map(|index| (block.clone(), index as u64))
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("transaction `{tx_id}` receipt is not linked from any block"),
                )
            })?;
        (receipt, block, receipt_index)
    };

    let verification = if options.audit_block_log {
        Some(verify_blocks(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?)
    } else {
        None
    };
    let (verification_mode, block_log_verified, block_count, tip_hash, tip_state_root) =
        if let Some(verification) = verification {
            (
                "full-block-replay".to_string(),
                verification.verified,
                verification.block_count as u64,
                verification.tip_hash,
                verification.state_root,
            )
        } else {
            let tip_hash = blocks.tip_hash();
            let tip_state_root = blocks
                .blocks
                .last()
                .map(|block| block.header.state_root.clone())
                .unwrap_or_default();
            (
                "selected-block-hot-path".to_string(),
                false,
                blocks.len() as u64,
                tip_hash,
                tip_state_root,
            )
        };

    let proof_id = tx_finality_proof_id(&genesis, &receipt, receipt_index, &block)?;
    Ok(TxFinalityReport {
        schema: "postfiat-tx-finality-v1".to_string(),
        proof_id,
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        tx_id,
        confirmed: true,
        verification_mode,
        receipt,
        receipt_index,
        receipt_count: block.receipt_ids.len() as u64,
        block,
        block_log_verified,
        block_count,
        tip_hash,
        tip_state_root,
    })
}

pub fn blocks(options: BlockQueryOptions) -> io::Result<Vec<BlockRecord>> {
    let store = NodeStore::new(options.data_dir);
    let mut blocks = store.read_blocks()?.blocks;
    let limit = bounded_read_query_limit(options.limit, "blocks")?;
    if let Some(from_height) = options.from_height {
        blocks.retain(|block| block.header.height >= from_height);
        if blocks.len() > limit {
            blocks.truncate(limit);
        }
    } else if blocks.len() > limit {
        blocks = blocks[blocks.len() - limit..].to_vec();
    }
    Ok(blocks)
}

pub fn account_tx(options: AccountTxQueryOptions) -> io::Result<AccountTxReport> {
    validate_account_tx_query(&options)?;
    let scan_limit = bounded_read_query_limit(options.limit, "account_tx")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let block_log = store.read_blocks()?;
    if let Some(rows) =
        read_usable_account_tx_disk_rows(&store, &genesis, &block_log, &options.address)
    {
        return Ok(account_tx_from_rows(options, scan_limit, &genesis, rows));
    }
    if let Some(index) = read_usable_account_tx_index(&store, &genesis, &block_log) {
        return Ok(account_tx_from_index(options, scan_limit, &genesis, &index));
    }
    account_tx_scan(options, scan_limit, &store, &genesis, &block_log)
}

const ACCOUNT_TX_DISK_INDEX_FILE: &str = "account_tx_index_meta.json";
const ACCOUNT_TX_ACCOUNT_SHARDS_DIR: &str = "account_tx_accounts";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AccountTxDiskIndex {
    schema: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    indexed_from_height: Option<u64>,
    indexed_to_height: Option<u64>,
    indexed_block_count: u64,
    indexed_row_count: u64,
    account_count: u64,
    tip_hash: String,
    shard_dir: String,
    accounts: BTreeMap<String, AccountTxDiskAccountEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AccountTxDiskAccountEntry {
    shard_file: String,
    row_count: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    tip_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AccountTxDiskAccountShard {
    schema: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    address: String,
    tip_hash: String,
    row_count: u64,
    rows: Vec<AccountTxRow>,
}

pub fn rebuild_account_tx_index(
    options: AccountTxIndexOptions,
) -> io::Result<AccountTxIndexBuildReport> {
    let store = NodeStore::new(&options.data_dir);
    let block_log = store.read_blocks()?;
    let archive = store.read_batch_archive()?;
    let receipts = store.read_receipts()?;
    refresh_account_tx_index_after_ordered_commit(&store, &block_log, &archive, &receipts)
}

fn refresh_account_tx_index_after_ordered_commit(
    store: &NodeStore,
    block_log: &BlockLog,
    archive: &BatchArchive,
    receipts: &[Receipt],
) -> io::Result<AccountTxIndexBuildReport> {
    let genesis = store.read_genesis()?;
    if let Some((index, touched_accounts)) = extend_account_tx_index_after_ordered_commit(
        store, &genesis, block_log, archive, receipts,
    )? {
        return write_account_tx_index_incremental_disk_shards(
            store,
            &genesis,
            block_log,
            index,
            &touched_accounts,
        );
    }
    let index = build_account_tx_index(&genesis, block_log, archive, receipts)?;
    write_account_tx_index(store, &genesis, block_log, index)
}

fn extend_account_tx_index_after_ordered_commit(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
    archive: &BatchArchive,
    receipts: &[Receipt],
) -> io::Result<Option<(AccountTxIndex, BTreeSet<String>)>> {
    let index_path = account_tx_index_path(store.data_dir());
    let mut index = match read_account_tx_index_file(&index_path) {
        Ok(index) => index,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::InvalidData
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    if account_tx_index_matches_domain(&index, genesis).is_err() {
        return Ok(None);
    }
    let Some(extension_start) = account_tx_index_extension_start(block_log, &index.tip_hash) else {
        return Ok(None);
    };

    let mut touched_accounts = BTreeSet::new();
    if extension_start < block_log.blocks.len() {
        let receipt_by_tx = receipt_by_tx_id(receipts)?;
        for block in &block_log.blocks[extension_start..] {
            append_account_tx_index_block(
                &mut index,
                block,
                archive,
                &receipt_by_tx,
                &mut touched_accounts,
            )?;
        }
    }
    index.account_count = index.accounts.len() as u64;
    index.tip_hash = block_log.tip_hash();
    Ok(Some((index, touched_accounts)))
}

fn account_tx_index_extension_start(block_log: &BlockLog, index_tip_hash: &str) -> Option<usize> {
    if index_tip_hash == block_log.tip_hash() {
        return Some(block_log.blocks.len());
    }
    if index_tip_hash == "genesis" {
        return Some(0);
    }
    block_log
        .blocks
        .iter()
        .position(|block| block.header.block_hash == index_tip_hash)
        .map(|index| index + 1)
}

fn append_account_tx_index_block(
    index: &mut AccountTxIndex,
    block: &BlockRecord,
    archive: &BatchArchive,
    receipt_by_tx: &HashMap<&str, &Receipt>,
    touched_accounts: &mut BTreeSet<String>,
) -> io::Result<()> {
    if block.header.batch_kind != "transparent" {
        return Ok(());
    }
    index.indexed_from_height.get_or_insert(block.header.height);
    index.indexed_to_height = Some(block.header.height);
    index.indexed_block_count = index.indexed_block_count.saturating_add(1);
    for row in account_tx_rows_for_transparent_block(block, archive, receipt_by_tx)? {
        index.indexed_row_count = index.indexed_row_count.saturating_add(1);
        touched_accounts.insert(row.from_address.clone());
        touched_accounts.insert(row.to_address.clone());
        accounts_for_account_tx_index(index, row);
    }
    Ok(())
}

fn build_account_tx_index(
    genesis: &Genesis,
    block_log: &BlockLog,
    archive: &BatchArchive,
    receipts: &[Receipt],
) -> io::Result<AccountTxIndex> {
    let receipt_by_tx = receipt_by_tx_id(receipts)?;
    let mut accounts: BTreeMap<String, Vec<AccountTxRow>> = BTreeMap::new();
    let mut indexed_row_count = 0_u64;
    let mut indexed_block_count = 0_u64;
    let mut indexed_from_height = None;
    let mut indexed_to_height = None;
    for block in &block_log.blocks {
        if block.header.batch_kind != "transparent" {
            continue;
        }
        indexed_from_height.get_or_insert(block.header.height);
        indexed_to_height = Some(block.header.height);
        indexed_block_count = indexed_block_count.saturating_add(1);
        for row in account_tx_rows_for_transparent_block(block, archive, &receipt_by_tx)? {
            indexed_row_count = indexed_row_count.saturating_add(1);
            accounts_for_account_tx_index_map(&mut accounts, row);
        }
    }
    let index = AccountTxIndex {
        schema: "postfiat-account-tx-index-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        indexed_from_height,
        indexed_to_height,
        indexed_block_count,
        indexed_row_count,
        account_count: accounts.len() as u64,
        tip_hash: block_log.tip_hash(),
        accounts,
    };
    Ok(index)
}

fn accounts_for_account_tx_index(index: &mut AccountTxIndex, row: AccountTxRow) {
    accounts_for_account_tx_index_map(&mut index.accounts, row);
}

fn accounts_for_account_tx_index_map(
    accounts: &mut BTreeMap<String, Vec<AccountTxRow>>,
    row: AccountTxRow,
) {
    accounts
        .entry(row.from_address.clone())
        .or_default()
        .push(row.clone());
    if row.to_address != row.from_address {
        accounts.entry(row.to_address.clone()).or_default().push(row);
    }
}

fn write_account_tx_index(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
    index: AccountTxIndex,
) -> io::Result<AccountTxIndexBuildReport> {
    write_account_tx_index_with_disk_update(store, genesis, block_log, index, None)
}

fn write_account_tx_index_incremental_disk_shards(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
    index: AccountTxIndex,
    touched_accounts: &BTreeSet<String>,
) -> io::Result<AccountTxIndexBuildReport> {
    write_account_tx_index_with_disk_update(
        store,
        genesis,
        block_log,
        index,
        Some(touched_accounts),
    )
}

fn write_account_tx_index_with_disk_update(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
    index: AccountTxIndex,
    touched_accounts: Option<&BTreeSet<String>>,
) -> io::Result<AccountTxIndexBuildReport> {
    let index_path = account_tx_index_path(store.data_dir());
    write_account_tx_index_file(&index_path, &index)?;
    if let Some(touched_accounts) = touched_accounts {
        write_account_tx_disk_index_incremental(store, &index, touched_accounts)?;
    } else {
        write_account_tx_disk_index(store, &index)?;
    }
    let disk_index = read_account_tx_disk_index_file(&account_tx_disk_index_path(store.data_dir()))?;
    let disk_index_usable = account_tx_disk_index_matches_chain(&disk_index, genesis, store, block_log)
        .is_ok();
    Ok(AccountTxIndexBuildReport {
        schema: "postfiat-account-tx-index-build-v1".to_string(),
        chain_id: index.chain_id.clone(),
        genesis_hash: index.genesis_hash.clone(),
        protocol_version: index.protocol_version,
        index_path: index_path.display().to_string(),
        disk_index_path: account_tx_disk_index_path(store.data_dir())
            .display()
            .to_string(),
        indexed_from_height: index.indexed_from_height,
        indexed_to_height: index.indexed_to_height,
        indexed_block_count: index.indexed_block_count,
        indexed_row_count: index.indexed_row_count,
        account_count: index.account_count,
        tip_hash: index.tip_hash.clone(),
        index_usable: account_tx_index_matches_chain(&index, genesis, store, block_log).is_ok(),
        disk_index_usable,
        disk_account_shard_count: disk_index.accounts.len() as u64,
    })
}

pub fn account_tx_index_status(
    options: AccountTxIndexOptions,
) -> io::Result<AccountTxIndexStatusReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let block_log = store.read_blocks()?;
    let index_path = account_tx_index_path(store.data_dir());
    let current_tip_hash = logical_tip_hash(&store, &block_log)?;
    let disk_status = account_tx_disk_index_status_fields(&store, &genesis, &block_log)?;
    match read_account_tx_index_file(&index_path) {
        Ok(index) => {
            let reason = account_tx_index_matches_chain(&index, &genesis, &store, &block_log).err();
            Ok(AccountTxIndexStatusReport {
                schema: "postfiat-account-tx-index-status-v1".to_string(),
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
                index_path: ACCOUNT_TX_INDEX_FILE.to_string(),
                disk_index_path: ACCOUNT_TX_DISK_INDEX_FILE.to_string(),
                index_present: true,
                index_usable: reason.is_none(),
                reason,
                disk_index_present: disk_status.present,
                disk_index_usable: disk_status.usable,
                disk_index_reason: disk_status.reason,
                indexed_from_height: index.indexed_from_height,
                indexed_to_height: index.indexed_to_height,
                indexed_block_count: index.indexed_block_count,
                indexed_row_count: index.indexed_row_count,
                account_count: index.account_count,
                disk_account_shard_count: disk_status.account_shard_count,
                tip_hash: index.tip_hash,
                current_tip_hash,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(AccountTxIndexStatusReport {
            schema: "postfiat-account-tx-index-status-v1".to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            index_path: ACCOUNT_TX_INDEX_FILE.to_string(),
            disk_index_path: ACCOUNT_TX_DISK_INDEX_FILE.to_string(),
            index_present: false,
            index_usable: false,
            reason: Some("account_tx index file is absent".to_string()),
            disk_index_present: disk_status.present,
            disk_index_usable: disk_status.usable,
            disk_index_reason: disk_status.reason,
            indexed_from_height: None,
            indexed_to_height: None,
            indexed_block_count: 0,
            indexed_row_count: 0,
            account_count: 0,
            disk_account_shard_count: disk_status.account_shard_count,
            tip_hash: String::new(),
            current_tip_hash,
        }),
        Err(error) => Err(error),
    }
}

struct AccountTxDiskIndexStatusFields {
    present: bool,
    usable: bool,
    reason: Option<String>,
    account_shard_count: u64,
}

fn account_tx_disk_index_status_fields(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
) -> io::Result<AccountTxDiskIndexStatusFields> {
    match read_account_tx_disk_index_file(&account_tx_disk_index_path(store.data_dir())) {
        Ok(index) => {
            let reason = account_tx_disk_index_matches_chain(&index, genesis, store, block_log).err();
            Ok(AccountTxDiskIndexStatusFields {
                present: true,
                usable: reason.is_none(),
                reason,
                account_shard_count: index.accounts.len() as u64,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok(AccountTxDiskIndexStatusFields {
                present: false,
                usable: false,
                reason: Some("account_tx disk index file is absent".to_string()),
                account_shard_count: 0,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            Ok(AccountTxDiskIndexStatusFields {
                present: true,
                usable: false,
                reason: Some(error.to_string()),
                account_shard_count: 0,
            })
        }
        Err(error) => Err(error),
    }
}

fn account_tx_scan(
    options: AccountTxQueryOptions,
    scan_limit: usize,
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
) -> io::Result<AccountTxReport> {
    let archive = store.read_batch_archive()?;
    let receipts = store.read_receipts()?;
    let receipt_by_tx = receipt_by_tx_id(&receipts)?;

    let mut block_indexes = block_log
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            let height = block.header.height;
            if options
                .from_height
                .is_some_and(|from_height| height < from_height)
            {
                return None;
            }
            if options
                .to_height
                .is_some_and(|to_height| height > to_height)
            {
                return None;
            }
            Some(index)
        })
        .collect::<Vec<_>>();
    let mut truncated = block_indexes.len() > scan_limit;
    if block_indexes.len() > scan_limit {
        if options.from_height.is_some() {
            block_indexes.truncate(scan_limit);
        } else {
            block_indexes = block_indexes[block_indexes.len() - scan_limit..].to_vec();
        }
    }

    let mut rows = Vec::new();
    let mut archive_lookup_count = 0_u64;
    for block_index in &block_indexes {
        let block = &block_log.blocks[*block_index];
        if block.header.batch_kind != "transparent" {
            continue;
        }
        archive_lookup_count = archive_lookup_count.saturating_add(1);
        for row in account_tx_rows_for_transparent_block(block, &archive, &receipt_by_tx)? {
            if row.from_address != options.address && row.to_address != options.address {
                continue;
            }
            rows.push(row);
            if rows.len() >= scan_limit {
                truncated = true;
                break;
            }
        }
        if rows.len() >= scan_limit {
            break;
        }
    }

    Ok(AccountTxReport {
        schema: "postfiat-account-tx-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address: options.address,
        from_height: options.from_height,
        to_height: options.to_height,
        scan_limit: scan_limit as u64,
        index_used: false,
        scanned_block_count: block_indexes.len() as u64,
        archive_lookup_count,
        truncated,
        row_count: rows.len() as u64,
        rows,
    })
}

fn account_tx_from_index(
    options: AccountTxQueryOptions,
    scan_limit: usize,
    genesis: &Genesis,
    index: &AccountTxIndex,
) -> AccountTxReport {
    let rows_for_account = index
        .accounts
        .get(&options.address)
        .cloned()
        .unwrap_or_default();
    account_tx_from_rows(options, scan_limit, genesis, rows_for_account)
}

fn account_tx_from_rows(
    options: AccountTxQueryOptions,
    scan_limit: usize,
    genesis: &Genesis,
    rows_for_account: Vec<AccountTxRow>,
) -> AccountTxReport {
    let rows_for_account = rows_for_account
        .into_iter()
        .filter(|row| {
            options
                .from_height
                .is_none_or(|from_height| row.block_height >= from_height)
                && options
                    .to_height
                    .is_none_or(|to_height| row.block_height <= to_height)
        })
        .collect::<Vec<_>>();
    let truncated = rows_for_account.len() >= scan_limit;
    let rows = if rows_for_account.len() > scan_limit {
        if options.from_height.is_some() {
            rows_for_account[..scan_limit].to_vec()
        } else {
            rows_for_account[rows_for_account.len() - scan_limit..].to_vec()
        }
    } else {
        rows_for_account
    };
    AccountTxReport {
        schema: "postfiat-account-tx-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address: options.address,
        from_height: options.from_height,
        to_height: options.to_height,
        scan_limit: scan_limit as u64,
        index_used: true,
        scanned_block_count: 0,
        archive_lookup_count: 0,
        truncated,
        row_count: rows.len() as u64,
        rows,
    }
}

fn write_account_tx_disk_index(store: &NodeStore, index: &AccountTxIndex) -> io::Result<()> {
    let shards_dir = account_tx_account_shards_dir(store.data_dir());
    std::fs::create_dir_all(&shards_dir)?;
    let mut accounts = BTreeMap::new();
    for (address, rows) in &index.accounts {
        let shard_file = account_tx_account_shard_file(address);
        let shard = AccountTxDiskAccountShard {
            schema: "postfiat-account-tx-account-shard-v1".to_string(),
            chain_id: index.chain_id.clone(),
            genesis_hash: index.genesis_hash.clone(),
            protocol_version: index.protocol_version,
            address: address.clone(),
            tip_hash: index.tip_hash.clone(),
            row_count: rows.len() as u64,
            rows: rows.clone(),
        };
        write_account_tx_account_shard_file(&shards_dir.join(&shard_file), &shard)?;
        accounts.insert(
            address.clone(),
            AccountTxDiskAccountEntry {
                shard_file,
                row_count: rows.len() as u64,
                tip_hash: index.tip_hash.clone(),
            },
        );
    }
    let disk_index = AccountTxDiskIndex {
        schema: "postfiat-account-tx-disk-index-v1".to_string(),
        chain_id: index.chain_id.clone(),
        genesis_hash: index.genesis_hash.clone(),
        protocol_version: index.protocol_version,
        indexed_from_height: index.indexed_from_height,
        indexed_to_height: index.indexed_to_height,
        indexed_block_count: index.indexed_block_count,
        indexed_row_count: index.indexed_row_count,
        account_count: index.account_count,
        tip_hash: index.tip_hash.clone(),
        shard_dir: ACCOUNT_TX_ACCOUNT_SHARDS_DIR.to_string(),
        accounts,
    };
    write_account_tx_disk_index_file(&account_tx_disk_index_path(store.data_dir()), &disk_index)
}

fn write_account_tx_disk_index_incremental(
    store: &NodeStore,
    index: &AccountTxIndex,
    touched_accounts: &BTreeSet<String>,
) -> io::Result<()> {
    if touched_accounts.is_empty() {
        return write_account_tx_disk_index(store, index);
    }
    let disk_index_path = account_tx_disk_index_path(store.data_dir());
    let previous = match read_account_tx_disk_index_file(&disk_index_path) {
        Ok(previous) => previous,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::InvalidData
            ) =>
        {
            return write_account_tx_disk_index(store, index);
        }
        Err(error) => return Err(error),
    };
    if account_tx_disk_index_matches_domain(&previous, index).is_err() {
        return write_account_tx_disk_index(store, index);
    }
    if previous
        .accounts
        .keys()
        .any(|address| !index.accounts.contains_key(address))
    {
        return write_account_tx_disk_index(store, index);
    }

    let shards_dir = account_tx_account_shards_dir(store.data_dir());
    std::fs::create_dir_all(&shards_dir)?;
    let mut accounts = previous.accounts;
    for address in touched_accounts {
        let Some(rows) = index.accounts.get(address) else {
            continue;
        };
        let shard_file = account_tx_account_shard_file(address);
        let shard = AccountTxDiskAccountShard {
            schema: "postfiat-account-tx-account-shard-v1".to_string(),
            chain_id: index.chain_id.clone(),
            genesis_hash: index.genesis_hash.clone(),
            protocol_version: index.protocol_version,
            address: address.clone(),
            tip_hash: index.tip_hash.clone(),
            row_count: rows.len() as u64,
            rows: rows.clone(),
        };
        write_account_tx_account_shard_file(&shards_dir.join(&shard_file), &shard)?;
        accounts.insert(
            address.clone(),
            AccountTxDiskAccountEntry {
                shard_file,
                row_count: rows.len() as u64,
                tip_hash: index.tip_hash.clone(),
            },
        );
    }
    if index
        .accounts
        .keys()
        .any(|address| !accounts.contains_key(address))
    {
        return write_account_tx_disk_index(store, index);
    }
    let disk_index = AccountTxDiskIndex {
        schema: "postfiat-account-tx-disk-index-v1".to_string(),
        chain_id: index.chain_id.clone(),
        genesis_hash: index.genesis_hash.clone(),
        protocol_version: index.protocol_version,
        indexed_from_height: index.indexed_from_height,
        indexed_to_height: index.indexed_to_height,
        indexed_block_count: index.indexed_block_count,
        indexed_row_count: index.indexed_row_count,
        account_count: index.account_count,
        tip_hash: index.tip_hash.clone(),
        shard_dir: ACCOUNT_TX_ACCOUNT_SHARDS_DIR.to_string(),
        accounts,
    };
    write_account_tx_disk_index_file(&disk_index_path, &disk_index)
}

fn read_usable_account_tx_disk_rows(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
    address: &str,
) -> Option<Vec<AccountTxRow>> {
    let disk_index = read_account_tx_disk_index_file(&account_tx_disk_index_path(store.data_dir()))
        .ok()?;
    account_tx_disk_index_matches_chain(&disk_index, genesis, store, block_log).ok()?;
    let Some(account) = disk_index.accounts.get(address) else {
        return Some(Vec::new());
    };
    let shard = read_account_tx_account_shard_file(
        &account_tx_account_shards_dir(store.data_dir()).join(&account.shard_file),
    )
    .ok()?;
    account_tx_account_shard_matches(&shard, &disk_index, address, account)
        .ok()
        .map(|()| shard.rows)
}

fn account_tx_account_shard_matches(
    shard: &AccountTxDiskAccountShard,
    disk_index: &AccountTxDiskIndex,
    address: &str,
    account: &AccountTxDiskAccountEntry,
) -> Result<(), String> {
    if shard.schema != "postfiat-account-tx-account-shard-v1" {
        return Err(format!(
            "unsupported account_tx account shard schema `{}`",
            shard.schema
        ));
    }
    if shard.chain_id != disk_index.chain_id
        || shard.genesis_hash != disk_index.genesis_hash
        || shard.protocol_version != disk_index.protocol_version
    {
        return Err("account_tx account shard domain does not match disk index".to_string());
    }
    if shard.address != address {
        return Err("account_tx account shard address does not match query".to_string());
    }
    let expected_tip_hash = if account.tip_hash.is_empty() {
        &disk_index.tip_hash
    } else {
        &account.tip_hash
    };
    if &shard.tip_hash != expected_tip_hash {
        return Err("account_tx account shard tip hash is stale".to_string());
    }
    if shard.row_count != account.row_count || shard.row_count != shard.rows.len() as u64 {
        return Err("account_tx account shard row count mismatch".to_string());
    }
    Ok(())
}

fn receipt_by_tx_id(receipts: &[Receipt]) -> io::Result<HashMap<&str, &Receipt>> {
    let mut receipt_by_tx = HashMap::new();
    for receipt in receipts {
        if receipt_by_tx
            .insert(receipt.tx_id.as_str(), receipt)
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("receipt log contains duplicate tx_id `{}`", receipt.tx_id),
            ));
        }
    }
    Ok(receipt_by_tx)
}

fn account_tx_rows_for_transparent_block(
    block: &BlockRecord,
    archive: &BatchArchive,
    receipt_by_tx: &HashMap<&str, &Receipt>,
) -> io::Result<Vec<AccountTxRow>> {
    let archive_entry = archive
        .find(&block.header.batch_kind, &block.header.batch_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} references missing transparent archive batch `{}`",
                    block.header.height, block.header.batch_id
                ),
            )
        })?;
    let batch: TransactionBatch = parse_archived_payload(block, archive_entry)?;
    let mut rows = Vec::new();
    for (transaction_index, transfer) in batch.transactions.iter().enumerate() {
        let unsigned = &transfer.unsigned;
        let expected_tx_id = transfer_tx_id(transfer);
        let tx_id = block.receipt_ids.get(transaction_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} has no receipt id for transparent transaction {}",
                    block.header.height, transaction_index
                ),
            )
        })?;
        if tx_id != &expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} receipt id for transaction {} does not match transfer id",
                    block.header.height, transaction_index
                ),
            ));
        }
        let receipt = receipt_by_tx.get(tx_id.as_str()).copied();
        rows.push(AccountTxRow {
            tx_id: tx_id.clone(),
            block_height: block.header.height,
            batch_kind: block.header.batch_kind.clone(),
            batch_id: block.header.batch_id.clone(),
            transaction_index: transaction_index as u64,
            transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
            from_address: unsigned.from.clone(),
            to_address: unsigned.to.clone(),
            amount: unsigned.amount,
            fee: unsigned.fee,
            sequence: unsigned.sequence,
            memo_hash: None,
            memo_count: None,
            memo_bytes: None,
            asset_id: None,
            issuer: None,
            trustline_authorized: None,
            trustline_frozen: None,
            nft_id: None,
            nft_issuer_transfer_fee: None,
            nft_collection_flags: None,
            escrow_id: None,
            offer_id: None,
            tx_role: None,
            counterparty_offer_id: None,
            fill_index: None,
            condition_hash: None,
            accepted: receipt.map(|receipt| receipt.accepted),
            receipt_code: receipt.map(|receipt| receipt.code.clone()),
        });
    }
    for (payment_index, payment) in batch.payments_v2.iter().enumerate() {
        let transaction_index = batch.transactions.len().saturating_add(payment_index);
        let unsigned = &payment.unsigned;
        let expected_tx_id = payment_v2_tx_id(payment);
        let tx_id = block.receipt_ids.get(transaction_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} has no receipt id for payment_v2 transaction {}",
                    block.header.height, transaction_index
                ),
            )
        })?;
        if tx_id != &expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} receipt id for payment_v2 transaction {} does not match payment id",
                    block.header.height, transaction_index
                ),
            ));
        }
        let receipt = receipt_by_tx.get(tx_id.as_str()).copied();
        rows.push(AccountTxRow {
            tx_id: tx_id.clone(),
            block_height: block.header.height,
            batch_kind: block.header.batch_kind.clone(),
            batch_id: block.header.batch_id.clone(),
            transaction_index: transaction_index as u64,
            transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
            from_address: unsigned.from.clone(),
            to_address: unsigned.to.clone(),
            amount: unsigned.amount,
            fee: unsigned.fee,
            sequence: unsigned.sequence,
            memo_hash: payment_v2_memo_hash(payment),
            memo_count: Some(unsigned.memos.len() as u64),
            memo_bytes: Some(unsigned.memo_bytes() as u64),
            asset_id: None,
            issuer: None,
            trustline_authorized: None,
            trustline_frozen: None,
            nft_id: None,
            nft_issuer_transfer_fee: None,
            nft_collection_flags: None,
            escrow_id: None,
            offer_id: None,
            tx_role: None,
            counterparty_offer_id: None,
            fill_index: None,
            condition_hash: None,
            accepted: receipt.map(|receipt| receipt.accepted),
            receipt_code: receipt.map(|receipt| receipt.code.clone()),
        });
    }
    for (asset_index, transaction) in batch.asset_transactions.iter().enumerate() {
        let transaction_index = batch
            .transactions
            .len()
            .saturating_add(batch.payments_v2.len())
            .saturating_add(asset_index);
        let unsigned = &transaction.unsigned;
        let expected_tx_id = asset_transaction_tx_id(transaction);
        let tx_id = block.receipt_ids.get(transaction_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} has no receipt id for asset transaction {}",
                    block.header.height, transaction_index
                ),
            )
        })?;
        if tx_id != &expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} receipt id for asset transaction {} does not match asset tx id",
                    block.header.height, transaction_index
                ),
            ));
        }
        let (
            from_address,
            to_address,
            amount,
            asset_id,
            issuer,
            trustline_authorized,
            trustline_frozen,
        ) = match &unsigned.operation {
            AssetTransactionOperation::AssetCreate(operation) => {
                let asset_id = postfiat_types::issued_asset_id(
                    &unsigned.chain_id,
                    &operation.issuer,
                    &operation.code,
                    operation.version,
                )
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                (
                    unsigned.source.clone(),
                    operation.issuer.clone(),
                    0,
                    Some(asset_id),
                    Some(operation.issuer.clone()),
                    None,
                    None,
                )
            }
            AssetTransactionOperation::TrustSet(operation) => (
                unsigned.source.clone(),
                operation.account.clone(),
                0,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                Some(operation.authorized),
                Some(operation.frozen),
            ),
            AssetTransactionOperation::IssuedPayment(operation) => (
                operation.from.clone(),
                operation.to.clone(),
                operation.amount,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::AssetBurn(operation) => (
                operation.owner.clone(),
                operation.issuer.clone(),
                operation.amount,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::AssetClawback(operation) => (
                operation.owner.clone(),
                operation.issuer.clone(),
                operation.amount,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::NavAssetRegister(operation) => (
                unsigned.source.clone(),
                operation.issuer.clone(),
                0,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::NavReserveSubmit(operation) => (
                operation.submitter.clone(),
                operation.issuer.clone(),
                operation.verified_net_assets,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::NavReserveChallenge(operation) => (
                operation.challenger.clone(),
                operation.challenger.clone(),
                0,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::NavEpochFinalize(operation) => (
                operation.issuer.clone(),
                operation.issuer.clone(),
                0,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::MarketOpsPolicyRegister(operation) => (
                operation.issuer.clone(),
                operation.issuer.clone(),
                0,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::MarketOpsFinalize(operation) => (
                operation.issuer.clone(),
                operation.issuer.clone(),
                0,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::NavMintAtNav(operation) => (
                operation.issuer.clone(),
                operation.to.clone(),
                operation.amount,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::NavRedeemAtNav(operation) => (
                operation.owner.clone(),
                operation.issuer.clone(),
                operation.amount,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::NavHalt(operation) => (
                operation.issuer.clone(),
                operation.issuer.clone(),
                0,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::NavProfileRegister(operation) => (
                operation.registrant.clone(),
                operation.registrant.clone(),
                0,
                None,
                None,
                None,
                None,
            ),
            AssetTransactionOperation::NavRedeemSettle(operation) => (
                operation.issuer.clone(),
                operation.issuer.clone(),
                0,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::NavReserveAttest(operation) => (
                operation.attestor.clone(),
                operation.attestor.clone(),
                0,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::NavAttestorRegister(operation) => (
                operation.attestor.clone(),
                operation.attestor.clone(),
                0,
                None,
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeDepositPropose(operation) => (
                operation.proposer.clone(),
                operation.evidence.vault_id(),
                operation.evidence.amount_atoms,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeDepositChallenge(operation) => (
                operation.challenger.clone(),
                operation.evidence_root.clone(),
                0,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeDepositAttest(operation) => (
                operation.attestor.clone(),
                operation.evidence_root.clone(),
                0,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeDepositFinalize(operation) => (
                operation.finalizer.clone(),
                operation.evidence_root.clone(),
                0,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeDepositClaim(operation) => (
                operation.claimer.clone(),
                operation.recipient.clone(),
                operation.amount_atoms,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeReceiptSubmit(operation) => (
                operation.operator.clone(),
                operation.operator.clone(),
                operation.amount_atoms,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeReceiptCount(operation) => (
                operation.operator.clone(),
                operation.operator.clone(),
                operation.counted_value_atoms,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeMintFromReceipts(operation) => (
                operation.issuer.clone(),
                operation.to.clone(),
                operation.amount_atoms,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeBurnToRedeem(operation) => (
                operation.owner.clone(),
                operation.issuer.clone(),
                operation.amount_atoms,
                Some(operation.asset_id.clone()),
                Some(operation.issuer.clone()),
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeRedeemSettle(operation) => (
                operation.issuer_or_redemption_account.clone(),
                operation.redemption_id.clone(),
                operation.settled_atoms,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeBucketImpair(operation) => (
                operation.operator.clone(),
                operation.bucket_id.clone(),
                operation.updated_counted_value_atoms,
                Some(operation.asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(operation) => (
                operation.operator.clone(),
                operation.nav_asset_id.clone(),
                operation.settlement_amount_atoms,
                Some(operation.settlement_asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::PftlUniswapRouteInit(operation) => (
                operation.operator.clone(),
                operation.route_id.clone(),
                0,
                Some(operation.native_nav_asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::PftlUniswapPrimarySubscribe(operation) => (
                operation.subscriber.clone(),
                operation.route_id.clone(),
                operation.settlement_value_atoms,
                Some(operation.settlement_asset_id.clone()),
                None,
                None,
                None,
            ),
            AssetTransactionOperation::PftlUniswapExportDebit(operation) => (
                operation.owner.clone(),
                operation.ethereum_recipient.clone(),
                operation.amount_atoms,
                None,
                None,
                None,
                None,
            ),
            AssetTransactionOperation::PftlUniswapDestinationConsume(operation) => (
                operation.operator.clone(),
                operation.packet_hash.clone(),
                0,
                None,
                None,
                None,
                None,
            ),
            AssetTransactionOperation::PftlUniswapRefundSource(operation) => (
                operation.operator.clone(),
                operation.packet_hash.clone(),
                0,
                None,
                None,
                None,
                None,
            ),
            AssetTransactionOperation::PftlUniswapReturnImport(operation) => (
                operation.operator.clone(),
                operation.pftl_recipient.clone(),
                operation.amount_atoms,
                Some(operation.native_nav_asset_id.clone()),
                None,
                None,
                None,
            ),
        };
        let receipt = receipt_by_tx.get(tx_id.as_str()).copied();
        rows.push(AccountTxRow {
            tx_id: tx_id.clone(),
            block_height: block.header.height,
            batch_kind: block.header.batch_kind.clone(),
            batch_id: block.header.batch_id.clone(),
            transaction_index: transaction_index as u64,
            transaction_kind: unsigned.transaction_kind.clone(),
            from_address,
            to_address,
            amount,
            fee: unsigned.fee,
            sequence: unsigned.sequence,
            memo_hash: None,
            memo_count: None,
            memo_bytes: None,
            asset_id,
            issuer,
            trustline_authorized,
            trustline_frozen,
            nft_id: None,
            nft_issuer_transfer_fee: None,
            nft_collection_flags: None,
            escrow_id: None,
            offer_id: None,
            tx_role: None,
            counterparty_offer_id: None,
            fill_index: None,
            condition_hash: None,
            accepted: receipt.map(|receipt| receipt.accepted),
            receipt_code: receipt.map(|receipt| receipt.code.clone()),
        });
    }
    for (swap_index, transaction) in batch.atomic_swap_transactions.iter().enumerate() {
        let transaction_index = batch
            .transactions
            .len()
            .saturating_add(batch.payments_v2.len())
            .saturating_add(batch.asset_transactions.len())
            .saturating_add(swap_index);
        let expected_tx_id = atomic_swap_transaction_tx_id(transaction);
        let tx_id = block.receipt_ids.get(transaction_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} has no receipt id for atomic swap transaction {}",
                    block.header.height, transaction_index
                ),
            )
        })?;
        if tx_id != &expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} receipt id for atomic swap transaction {} does not match swap tx id",
                    block.header.height, transaction_index
                ),
            ));
        }
        let receipt = receipt_by_tx.get(tx_id.as_str()).copied();
        for (role, leg) in [
            ("leg_0", &transaction.unsigned.leg_0),
            ("leg_1", &transaction.unsigned.leg_1),
        ] {
            rows.push(AccountTxRow {
                tx_id: tx_id.clone(),
                block_height: block.header.height,
                batch_kind: block.header.batch_kind.clone(),
                batch_id: block.header.batch_id.clone(),
                transaction_index: transaction_index as u64,
                transaction_kind: ATOMIC_SWAP_TRANSACTION_KIND.to_string(),
                from_address: leg.owner.clone(),
                to_address: leg.recipient.clone(),
                amount: leg.amount,
                fee: leg.fee,
                sequence: leg.sequence,
                memo_hash: None,
                memo_count: None,
                memo_bytes: None,
                asset_id: Some(leg.asset_id.clone()),
                issuer: Some(leg.issuer.clone()),
                trustline_authorized: None,
                trustline_frozen: None,
                nft_id: None,
                nft_issuer_transfer_fee: None,
                nft_collection_flags: None,
                escrow_id: None,
                offer_id: None,
                tx_role: Some(role.to_string()),
                counterparty_offer_id: None,
                fill_index: None,
                condition_hash: None,
                accepted: receipt.map(|receipt| receipt.accepted),
                receipt_code: receipt.map(|receipt| receipt.code.clone()),
            });
        }
    }
    for (escrow_index, transaction) in batch.escrow_transactions.iter().enumerate() {
        let transaction_index = batch
            .transactions
            .len()
            .saturating_add(batch.payments_v2.len())
            .saturating_add(batch.asset_transactions.len())
            .saturating_add(batch.atomic_swap_transactions.len())
            .saturating_add(escrow_index);
        let unsigned = &transaction.unsigned;
        let expected_tx_id = escrow_transaction_tx_id(transaction);
        let tx_id = block.receipt_ids.get(transaction_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} has no receipt id for escrow transaction {}",
                    block.header.height, transaction_index
                ),
            )
        })?;
        if tx_id != &expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} receipt id for escrow transaction {} does not match escrow tx id",
                    block.header.height, transaction_index
                ),
            ));
        }
        let (from_address, to_address, amount, asset_id, escrow_id, condition_hash) =
            match &unsigned.operation {
                EscrowTransactionOperation::EscrowCreate(operation) => {
                    let escrow_id = postfiat_types::escrow_id(
                        &unsigned.chain_id,
                        &operation.owner,
                        unsigned.sequence,
                    )
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    let condition_hash = if operation.condition.is_empty() {
                        None
                    } else {
                        Some(
                            postfiat_types::escrow_condition_hash(&operation.condition).map_err(
                                |error| io::Error::new(io::ErrorKind::InvalidData, error),
                            )?,
                        )
                    };
                    (
                        operation.owner.clone(),
                        operation.recipient.clone(),
                        operation.amount,
                        if operation.asset_id == postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID {
                            None
                        } else {
                            Some(operation.asset_id.clone())
                        },
                        escrow_id,
                        condition_hash,
                    )
                }
                EscrowTransactionOperation::EscrowFinish(operation) => (
                    operation.owner.clone(),
                    operation.recipient.clone(),
                    0,
                    None,
                    operation.escrow_id.clone(),
                    None,
                ),
                EscrowTransactionOperation::EscrowCancel(operation) => (
                    operation.owner.clone(),
                    operation.owner.clone(),
                    0,
                    None,
                    operation.escrow_id.clone(),
                    None,
                ),
            };
        let receipt = receipt_by_tx.get(tx_id.as_str()).copied();
        rows.push(AccountTxRow {
            tx_id: tx_id.clone(),
            block_height: block.header.height,
            batch_kind: block.header.batch_kind.clone(),
            batch_id: block.header.batch_id.clone(),
            transaction_index: transaction_index as u64,
            transaction_kind: unsigned.transaction_kind.clone(),
            from_address,
            to_address,
            amount,
            fee: unsigned.fee,
            sequence: unsigned.sequence,
            memo_hash: None,
            memo_count: None,
            memo_bytes: None,
            asset_id,
            issuer: None,
            trustline_authorized: None,
            trustline_frozen: None,
            nft_id: None,
            nft_issuer_transfer_fee: None,
            nft_collection_flags: None,
            escrow_id: Some(escrow_id),
            offer_id: None,
            tx_role: None,
            counterparty_offer_id: None,
            fill_index: None,
            condition_hash,
            accepted: receipt.map(|receipt| receipt.accepted),
            receipt_code: receipt.map(|receipt| receipt.code.clone()),
        });
    }
    for (nft_index, transaction) in batch.nft_transactions.iter().enumerate() {
        let transaction_index = batch
            .transactions
            .len()
            .saturating_add(batch.payments_v2.len())
            .saturating_add(batch.asset_transactions.len())
            .saturating_add(batch.atomic_swap_transactions.len())
            .saturating_add(batch.escrow_transactions.len())
            .saturating_add(nft_index);
        let unsigned = &transaction.unsigned;
        let expected_tx_id = nft_transaction_tx_id(transaction);
        let tx_id = block.receipt_ids.get(transaction_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} has no receipt id for nft transaction {}",
                    block.header.height, transaction_index
                ),
            )
        })?;
        if tx_id != &expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} receipt id for nft transaction {} does not match nft tx id",
                    block.header.height, transaction_index
                ),
            ));
        }
        let (
            from_address,
            to_address,
            nft_id,
            issuer,
            nft_issuer_transfer_fee,
            nft_collection_flags,
        ) =
            match &unsigned.operation {
            NftTransactionOperation::NftMint(operation) => {
                let nft_id = postfiat_types::nft_id(
                    &unsigned.chain_id,
                    &operation.issuer,
                    &operation.collection_id,
                    operation.serial,
                )
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                (
                    operation.issuer.clone(),
                    operation.owner.clone(),
                    nft_id,
                    Some(operation.issuer.clone()),
                    None,
                    (operation.collection_flags != 0).then_some(operation.collection_flags),
                )
            }
            NftTransactionOperation::NftTransfer(operation) => (
                operation.from.clone(),
                operation.to.clone(),
                operation.nft_id.clone(),
                if operation.issuer.is_empty() {
                    None
                } else {
                    Some(operation.issuer.clone())
                },
                (operation.issuer_transfer_fee != 0).then_some(operation.issuer_transfer_fee),
                None,
            ),
            NftTransactionOperation::NftBurn(operation) => (
                operation.owner.clone(),
                operation.owner.clone(),
                operation.nft_id.clone(),
                None,
                None,
                None,
            ),
        };
        let receipt = receipt_by_tx.get(tx_id.as_str()).copied();
        rows.push(AccountTxRow {
            tx_id: tx_id.clone(),
            block_height: block.header.height,
            batch_kind: block.header.batch_kind.clone(),
            batch_id: block.header.batch_id.clone(),
            transaction_index: transaction_index as u64,
            transaction_kind: unsigned.transaction_kind.clone(),
            from_address,
            to_address,
            amount: 0,
            fee: unsigned.fee,
            sequence: unsigned.sequence,
            memo_hash: None,
            memo_count: None,
            memo_bytes: None,
            asset_id: None,
            issuer,
            trustline_authorized: None,
            trustline_frozen: None,
            nft_id: Some(nft_id),
            nft_issuer_transfer_fee,
            nft_collection_flags,
            escrow_id: None,
            offer_id: None,
            tx_role: None,
            counterparty_offer_id: None,
            fill_index: None,
            condition_hash: None,
            accepted: receipt.map(|receipt| receipt.accepted),
            receipt_code: receipt.map(|receipt| receipt.code.clone()),
        });
    }
    for (offer_index, transaction) in batch.offer_transactions.iter().enumerate() {
        let transaction_index = batch
            .transactions
            .len()
            .saturating_add(batch.payments_v2.len())
            .saturating_add(batch.asset_transactions.len())
            .saturating_add(batch.atomic_swap_transactions.len())
            .saturating_add(batch.escrow_transactions.len())
            .saturating_add(batch.nft_transactions.len())
            .saturating_add(offer_index);
        let unsigned = &transaction.unsigned;
        let expected_tx_id = offer_transaction_tx_id(transaction);
        let tx_id = block.receipt_ids.get(transaction_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} has no receipt id for offer transaction {}",
                    block.header.height, transaction_index
                ),
            )
        })?;
        if tx_id != &expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} receipt id for offer transaction {} does not match offer tx id",
                    block.header.height, transaction_index
                ),
            ));
        }
        let (from_address, to_address, amount, asset_id, offer_id, tx_role) = match &unsigned.operation {
            OfferTransactionOperation::OfferCreate(operation) => {
                let offer_id =
                    postfiat_types::offer_id(&unsigned.chain_id, &operation.owner, unsigned.sequence)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                let asset_id =
                    if operation.taker_gets_asset_id != postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID
                    {
                        Some(operation.taker_gets_asset_id.clone())
                    } else if operation.taker_pays_asset_id
                        != postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID
                    {
                        Some(operation.taker_pays_asset_id.clone())
                    } else {
                        None
                    };
                (
                    operation.owner.clone(),
                    operation.owner.clone(),
                    operation.taker_gets_amount,
                    asset_id,
                    offer_id,
                    OFFER_TX_ROLE_TAKER.to_string(),
                )
            }
            OfferTransactionOperation::OfferCancel(operation) => (
                operation.owner.clone(),
                operation.owner.clone(),
                0,
                None,
                operation.offer_id.clone(),
                OFFER_TX_ROLE_CANCEL.to_string(),
            ),
        };
        let receipt = receipt_by_tx.get(tx_id.as_str()).copied();
        let taker_offer_id = offer_id.clone();
        rows.push(AccountTxRow {
            tx_id: tx_id.clone(),
            block_height: block.header.height,
            batch_kind: block.header.batch_kind.clone(),
            batch_id: block.header.batch_id.clone(),
            transaction_index: transaction_index as u64,
            transaction_kind: unsigned.transaction_kind.clone(),
            from_address,
            to_address,
            amount,
            fee: unsigned.fee,
            sequence: unsigned.sequence,
            memo_hash: None,
            memo_count: None,
            memo_bytes: None,
            asset_id,
            issuer: None,
            trustline_authorized: None,
            trustline_frozen: None,
            nft_id: None,
            nft_issuer_transfer_fee: None,
            nft_collection_flags: None,
            escrow_id: None,
            offer_id: Some(offer_id),
            tx_role: Some(tx_role),
            counterparty_offer_id: None,
            fill_index: None,
            condition_hash: None,
            accepted: receipt.map(|receipt| receipt.accepted),
            receipt_code: receipt.map(|receipt| receipt.code.clone()),
        });
        if let (OfferTransactionOperation::OfferCreate(_), Some(receipt)) =
            (&unsigned.operation, receipt)
        {
            for fill in &receipt.offer_fills {
                rows.push(AccountTxRow {
                    tx_id: tx_id.clone(),
                    block_height: block.header.height,
                    batch_kind: block.header.batch_kind.clone(),
                    batch_id: block.header.batch_id.clone(),
                    transaction_index: transaction_index as u64,
                    transaction_kind: unsigned.transaction_kind.clone(),
                    from_address: fill.maker_owner.clone(),
                    to_address: fill.taker.clone(),
                    amount: fill.maker_sends_amount,
                    fee: 0,
                    sequence: 0,
                    memo_hash: None,
                    memo_count: None,
                    memo_bytes: None,
                    asset_id: if fill.maker_sends_asset_id
                        == postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID
                    {
                        None
                    } else {
                        Some(fill.maker_sends_asset_id.clone())
                    },
                    issuer: None,
                    trustline_authorized: None,
                    trustline_frozen: None,
                    nft_id: None,
                    nft_issuer_transfer_fee: None,
                    nft_collection_flags: None,
                    escrow_id: None,
                    offer_id: Some(fill.maker_offer_id.clone()),
                    tx_role: Some(OFFER_TX_ROLE_MAKER.to_string()),
                    counterparty_offer_id: Some(taker_offer_id.clone()),
                    fill_index: Some(fill.fill_index),
                    condition_hash: None,
                    accepted: Some(receipt.accepted),
                    receipt_code: Some(receipt.code.clone()),
                });
            }
        }
    }
    Ok(rows)
}

fn payment_v2_memo_hash(payment: &SignedPaymentV2) -> Option<String> {
    if payment.unsigned.memos.is_empty() {
        return None;
    }
    let mut bytes = format!("memo_count={}\n", payment.unsigned.memos.len()).into_bytes();
    for (index, memo) in payment.unsigned.memos.iter().enumerate() {
        bytes.extend_from_slice(
            format!(
                "memo[{index}].type_bytes={}\nmemo[{index}].type={}\nmemo[{index}].format_bytes={}\nmemo[{index}].format={}\nmemo[{index}].data_bytes={}\nmemo[{index}].data={}\n",
                memo.memo_type.len() / 2,
                memo.memo_type,
                memo.memo_format.len() / 2,
                memo.memo_format,
                memo.memo_data.len() / 2,
                memo.memo_data
            )
            .as_bytes(),
        );
    }
    Some(hash_hex("postfiat.payment_v2.memo.v1", &bytes))
}

fn read_usable_account_tx_index(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
) -> Option<AccountTxIndex> {
    let index = read_account_tx_index_file(&account_tx_index_path(store.data_dir())).ok()?;
    account_tx_index_matches_chain(&index, genesis, store, block_log)
        .ok()
        .map(|()| index)
}

fn account_tx_index_matches_chain(
    index: &AccountTxIndex,
    genesis: &Genesis,
    store: &NodeStore,
    block_log: &BlockLog,
) -> Result<(), String> {
    account_tx_index_matches_domain(index, genesis)?;
    let current_tip_hash = logical_tip_hash(store, block_log).map_err(|error| error.to_string())?;
    if index.tip_hash != current_tip_hash {
        return Err("account_tx index tip hash is stale".to_string());
    }
    Ok(())
}

fn account_tx_index_matches_domain(index: &AccountTxIndex, genesis: &Genesis) -> Result<(), String> {
    if index.schema != "postfiat-account-tx-index-v1" {
        return Err(format!(
            "unsupported account_tx index schema `{}`",
            index.schema
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if index.chain_id != genesis.chain_id {
        return Err("account_tx index chain_id does not match local genesis".to_string());
    }
    if index.genesis_hash != expected_genesis_hash {
        return Err("account_tx index genesis_hash does not match local genesis".to_string());
    }
    if index.protocol_version != genesis.protocol_version {
        return Err("account_tx index protocol_version does not match local genesis".to_string());
    }
    Ok(())
}

fn account_tx_disk_index_matches_chain(
    index: &AccountTxDiskIndex,
    genesis: &Genesis,
    store: &NodeStore,
    block_log: &BlockLog,
) -> Result<(), String> {
    let current_tip_hash = logical_tip_hash(store, block_log).map_err(|error| error.to_string())?;
    account_tx_disk_index_matches_genesis_and_tip(index, genesis, &current_tip_hash)
}

fn account_tx_disk_index_matches_genesis_and_tip(
    index: &AccountTxDiskIndex,
    genesis: &Genesis,
    current_tip_hash: &str,
) -> Result<(), String> {
    if index.schema != "postfiat-account-tx-disk-index-v1" {
        return Err(format!(
            "unsupported account_tx disk index schema `{}`",
            index.schema
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if index.chain_id != genesis.chain_id {
        return Err("account_tx disk index chain_id does not match local genesis".to_string());
    }
    if index.genesis_hash != expected_genesis_hash {
        return Err("account_tx disk index genesis_hash does not match local genesis".to_string());
    }
    if index.protocol_version != genesis.protocol_version {
        return Err("account_tx disk index protocol_version does not match local genesis".to_string());
    }
    if index.shard_dir != ACCOUNT_TX_ACCOUNT_SHARDS_DIR {
        return Err("account_tx disk index shard_dir is unsupported".to_string());
    }
    if index.account_count != index.accounts.len() as u64 {
        return Err("account_tx disk index account_count mismatch".to_string());
    }
    let indexed_rows = index
        .accounts
        .values()
        .map(|account| account.row_count)
        .fold(0_u64, |total, count| total.saturating_add(count));
    if indexed_rows < index.indexed_row_count {
        return Err("account_tx disk index shard row count is below indexed rows".to_string());
    }
    if index.tip_hash != current_tip_hash {
        return Err("account_tx disk index tip hash is stale".to_string());
    }
    Ok(())
}

fn account_tx_disk_index_matches_domain(
    disk_index: &AccountTxDiskIndex,
    index: &AccountTxIndex,
) -> Result<(), String> {
    if disk_index.schema != "postfiat-account-tx-disk-index-v1" {
        return Err(format!(
            "unsupported account_tx disk index schema `{}`",
            disk_index.schema
        ));
    }
    if disk_index.chain_id != index.chain_id {
        return Err("account_tx disk index chain_id does not match index".to_string());
    }
    if disk_index.genesis_hash != index.genesis_hash {
        return Err("account_tx disk index genesis_hash does not match index".to_string());
    }
    if disk_index.protocol_version != index.protocol_version {
        return Err("account_tx disk index protocol_version does not match index".to_string());
    }
    if disk_index.shard_dir != ACCOUNT_TX_ACCOUNT_SHARDS_DIR {
        return Err("account_tx disk index shard_dir is unsupported".to_string());
    }
    if disk_index.account_count != disk_index.accounts.len() as u64 {
        return Err("account_tx disk index account_count mismatch".to_string());
    }
    Ok(())
}

fn account_tx_index_path(data_dir: &Path) -> PathBuf {
    data_dir.join(ACCOUNT_TX_INDEX_FILE)
}

fn account_tx_disk_index_path(data_dir: &Path) -> PathBuf {
    data_dir.join(ACCOUNT_TX_DISK_INDEX_FILE)
}

fn account_tx_account_shards_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(ACCOUNT_TX_ACCOUNT_SHARDS_DIR)
}

fn account_tx_account_shard_file(address: &str) -> String {
    format!(
        "{}.json",
        hash_hex("postfiat.account_tx.address_shard.v1", address.as_bytes())
    )
}

fn read_account_tx_index_file(path: &Path) -> io::Result<AccountTxIndex> {
    read_json_file(path, "account_tx index")
}

fn write_account_tx_index_file(path: &Path, index: &AccountTxIndex) -> io::Result<()> {
    let json = serde_json::to_string_pretty(index).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

fn read_account_tx_disk_index_file(path: &Path) -> io::Result<AccountTxDiskIndex> {
    read_json_file(path, "account_tx disk index")
}

fn write_account_tx_disk_index_file(path: &Path, index: &AccountTxDiskIndex) -> io::Result<()> {
    let json = serde_json::to_string_pretty(index).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

fn read_account_tx_account_shard_file(path: &Path) -> io::Result<AccountTxDiskAccountShard> {
    read_json_file(path, "account_tx account shard")
}

fn write_account_tx_account_shard_file(
    path: &Path,
    shard: &AccountTxDiskAccountShard,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(shard).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

fn validate_account_tx_query(options: &AccountTxQueryOptions) -> io::Result<()> {
    if options.address.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "account_tx address must be nonempty",
        ));
    }
    if options.address != options.address.trim() || options.address.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "account_tx address must not contain leading, trailing, or control whitespace",
        ));
    }
    if options.address.len() > MAX_TEXT_FIELD_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "account_tx address must not exceed {MAX_TEXT_FIELD_BYTES} bytes"
            ),
        ));
    }
    if options
        .from_height
        .zip(options.to_height)
        .is_some_and(|(from_height, to_height)| from_height > to_height)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "account_tx from_height must be less than or equal to to_height",
        ));
    }
    Ok(())
}

pub fn create_block_vote(options: BlockVoteOptions) -> io::Result<BlockVoteFile> {
    Ok(create_block_vote_with_timings(options)?.vote)
}

pub fn create_block_vote_with_timings(
    options: BlockVoteOptions,
) -> io::Result<BlockVoteWithTimingsReport> {
    let total_start = std::time::Instant::now();
    let mut timings = BlockVoteCreationTimingReport {
        schema: "postfiat.block_vote_creation_timing.v1".to_string(),
        ..BlockVoteCreationTimingReport::default()
    };
    if options.verify_block_log {
        let stage_start = std::time::Instant::now();
        verify_blocks(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
        timings.verify_block_log_ms = node_timing_elapsed_ms(stage_start);
    }
    let stage_start = std::time::Instant::now();
    let store = NodeStore::new(&options.data_dir);
    timings.store_init_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let genesis = store.read_genesis()?;
    timings.read_genesis_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let target = block_vote_target_with_timings(
        &store,
        &genesis,
        options.batch_file.as_deref(),
        options.proposal_file.as_deref(),
        options.timeout_certificate_file.as_deref(),
        options.block_height,
    )?;
    timings.target_ms = node_timing_elapsed_ms(stage_start);
    let target_breakdown = target.timings;
    let target = target.target;

    let vote = create_block_vote_for_target_with_timings(
        &store,
        &genesis,
        target,
        &options.key_file,
        options.validator_id.as_deref(),
        &options.vote_file,
        target_breakdown,
        timings,
        total_start,
    )?;
    Ok(vote)
}

pub fn create_block_vote_for_verified_proposal(
    options: BlockVoteForVerifiedProposalOptions,
) -> io::Result<BlockVoteFile> {
    if options.verify_block_log {
        verify_blocks(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let proposal = options.proposal;
    validate_block_proposal_file(&proposal, &genesis)?;
    verify_block_proposal_signature_if_present(&store, &proposal)?;
    if let Some(block_height) = options.block_height {
        if proposal.block_height != block_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "proposal block height {} does not match --height {block_height}",
                    proposal.block_height
                ),
            ));
        }
    }
    let governance =
        governance_with_due_validator_registry_activations(&store, &genesis, proposal.block_height)?;
    validate_bridge_exit_root_activation(&proposal, &genesis, &governance)?;
    let validators = active_validator_ids(&governance)?;
    let proposal_hash = block_proposal_hash(&proposal)?;
    let target = BlockVoteTarget {
        evidence: OwnedBlockEvidence::from_proposal(&proposal),
        validators,
        block_hash: None,
        proposal_hash: Some(proposal_hash),
    };
    create_block_vote_for_target(
        &store,
        &genesis,
        target,
        &options.key_file,
        options.validator_id.as_deref(),
        &options.vote_file,
    )
}

fn create_block_vote_for_target(
    store: &NodeStore,
    genesis: &Genesis,
    target: BlockVoteTarget,
    key_file_path: &Path,
    validator_id: Option<&str>,
    vote_file: &Path,
) -> io::Result<BlockVoteFile> {
    Ok(create_block_vote_for_target_with_timings(
        store,
        genesis,
        target,
        key_file_path,
        validator_id,
        vote_file,
        BlockVoteTargetTimingReport {
            schema: "postfiat.block_vote_target_timing.v1".to_string(),
            ..BlockVoteTargetTimingReport::default()
        },
        BlockVoteCreationTimingReport {
            schema: "postfiat.block_vote_creation_timing.v1".to_string(),
            ..BlockVoteCreationTimingReport::default()
        },
        std::time::Instant::now(),
    )?
    .vote)
}

fn create_block_vote_for_target_with_timings(
    store: &NodeStore,
    genesis: &Genesis,
    target: BlockVoteTarget,
    key_file_path: &Path,
    validator_id: Option<&str>,
    vote_file: &Path,
    target_breakdown: BlockVoteTargetTimingReport,
    mut timings: BlockVoteCreationTimingReport,
    total_start: std::time::Instant,
) -> io::Result<BlockVoteWithTimingsReport> {
    let stage_start = std::time::Instant::now();
    let key_file = read_validator_key_file(key_file_path)?;
    timings.key_read_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    validate_validator_key_file(&key_file)?;
    timings.key_validation_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let key_record = select_validator_key_record(&key_file, validator_id)?;
    if !target
        .validators
        .iter()
        .any(|validator| validator == &key_record.node_id)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "validator `{}` is not in block {} certificate set",
                key_record.node_id, target.evidence.height
            ),
        ));
    }
    timings.validator_membership_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    timings.registry_read_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let registry_record = validator_registry_record(&registry, &key_record.node_id)?;
    if key_record.algorithm_id != registry_record.algorithm_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "validator key `{}` algorithm does not match registry",
                key_record.node_id
            ),
        ));
    }
    if key_record.public_key_hex != registry_record.public_key_hex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "validator key `{}` public key does not match registry",
                key_record.node_id
            ),
        ));
    }
    timings.registry_key_check_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    reserve_block_proposal_vote_lock(store, genesis, &target, &key_record.node_id)?;
    timings.vote_lock_reservation_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let evidence_ref = target.evidence.as_evidence();
    let registry_root = validator_registry_root(&registry, &target.validators)?;
    let message = block_certificate_vote_message(
        &genesis,
        &evidence_ref,
        &key_record.node_id,
        true,
        &registry_root,
    )?;
    timings.message_build_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let private_key = Zeroizing::new(hex_to_bytes(&key_record.private_key_hex).map_err(invalid_data)?);
    let signature_seed = block_certificate_signature_seed(&message)?;
    timings.private_key_decode_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let signature = ml_dsa_65_sign_with_context_seed(
        &private_key,
        &message,
        BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
        &signature_seed,
    )
    .map_err(invalid_data)?;
    timings.mldsa_signing_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let vote = BlockVoteFile {
        schema: BLOCK_VOTE_FILE_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height: target.evidence.height,
        view: target.evidence.view,
        block_hash: target.block_hash.clone(),
        proposal_hash: target.proposal_hash.clone(),
        vote: BlockCertificateVote {
            vote_id: block_certificate_vote_id(&message),
            validator: key_record.node_id.clone(),
            accept: true,
            algorithm_id: key_record.algorithm_id.clone(),
            registry_root,
            public_key_hex: String::new(),
            signature_hex: bytes_to_hex(&signature),
        },
    };
    timings.vote_construct_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    validate_block_vote_file_for_target(
        &vote,
        genesis,
        &target.evidence,
        &target.validators,
        vote.block_hash.as_deref(),
        vote.proposal_hash.as_deref(),
    )?;
    timings.vote_validation_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let json = serde_json::to_string_pretty(&vote).map_err(invalid_data)?;
    timings.json_serde_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    atomic_write(vote_file, format!("{json}\n"))?;
    timings.vote_file_write_ms = node_timing_elapsed_ms(stage_start);

    timings.target_breakdown = target_breakdown;
    timings.total_ms = node_timing_elapsed_ms(total_start);
    Ok(BlockVoteWithTimingsReport { vote, timings })
}

fn block_proposal_vote_lock_path(
    store: &NodeStore,
    genesis: &Genesis,
    block_height: u64,
    view: u64,
    validator: &str,
) -> PathBuf {
    // Consensus v2 verifies the timeout certificate before reaching vote-lock
    // reservation. Its anti-equivocation boundary is therefore one proposal
    // per (height, view), while the legacy protocol must retain its historical
    // one-proposal-per-height lock and byte-identical path derivation.
    let (domain, material, file_name) = if consensus_v2_active_at(genesis, block_height) {
        (
            "postfiat.block_proposal_vote_lock_path.v3",
            format!("{block_height}:{view}:{validator}"),
            format!("{block_height}.{view}"),
        )
    } else {
        (
            "postfiat.block_proposal_vote_lock_path.v2",
            format!("{block_height}:{validator}"),
            block_height.to_string(),
        )
    };
    let lock_id = hash_hex(domain, material.as_bytes());
    store
        .data_dir()
        .join(BLOCK_PROPOSAL_VOTE_LOCK_DIR)
        .join(format!("{file_name}.{lock_id}.json"))
}

fn reserve_block_proposal_vote_lock(
    store: &NodeStore,
    genesis: &Genesis,
    target: &BlockVoteTarget,
    validator: &str,
) -> io::Result<()> {
    let Some(proposal_hash) = target.proposal_hash.as_deref() else {
        return Ok(());
    };
    let lock = BlockProposalVoteLock {
        schema: BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        block_height: target.evidence.height,
        view: target.evidence.view,
        validator: validator.to_string(),
        proposal_hash: proposal_hash.to_string(),
    };
    validate_prior_block_proposal_vote_locks(store, genesis, target, validator, proposal_hash)?;
    let lock_path = block_proposal_vote_lock_path(
        store,
        genesis,
        target.evidence.height,
        target.evidence.view,
        validator,
    );
    if write_new_block_proposal_vote_lock(&lock_path, &lock)? {
        return Ok(());
    }
    let existing = read_block_proposal_vote_lock_file(&lock_path)?;
    validate_block_proposal_vote_lock(&existing, genesis, target, validator, proposal_hash)
}

fn validate_prior_block_proposal_vote_locks(
    store: &NodeStore,
    genesis: &Genesis,
    target: &BlockVoteTarget,
    validator: &str,
    proposal_hash: &str,
) -> io::Result<()> {
    let lock_dir = store.data_dir().join(BLOCK_PROPOSAL_VOTE_LOCK_DIR);
    let entries = match std::fs::read_dir(&lock_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let existing = read_block_proposal_vote_lock_file(&entry.path())?;
        if existing.block_height == target.evidence.height
            && existing.validator == validator
            && block_proposal_vote_lock_binds_view(
                genesis,
                target.evidence.height,
                existing.view,
                target.evidence.view,
            )
        {
            validate_block_proposal_vote_lock(
                &existing,
                genesis,
                target,
                validator,
                proposal_hash,
            )?;
        }
    }
    Ok(())
}

fn block_proposal_vote_lock_binds_view(
    genesis: &Genesis,
    block_height: u64,
    recorded_view: u64,
    attempted_view: u64,
) -> bool {
    !consensus_v2_active_at(genesis, block_height) || recorded_view == attempted_view
}

fn write_new_block_proposal_vote_lock(
    lock_path: &Path,
    lock: &BlockProposalVoteLock,
) -> io::Result<bool> {
    let parent = lock_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "block proposal vote lock path has no parent directory",
        )
    })?;
    std::fs::create_dir_all(parent)?;
    let json = serde_json::to_string_pretty(lock).map_err(invalid_data)?;
    let temp_id = hash_hex(
        "postfiat.block_proposal_vote_lock_temp.v1",
        format!(
            "{}:{}:{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(invalid_data)?
                .as_nanos(),
            lock_path.display()
        )
        .as_bytes(),
    );
    let temp_path = parent.join(format!(".{temp_id}.tmp"));
    atomic_write(&temp_path, format!("{json}\n"))?;
    let result = std::fs::hard_link(&temp_path, lock_path);
    let _ = std::fs::remove_file(&temp_path);
    match result {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

fn read_block_proposal_vote_lock_file(path: &Path) -> io::Result<BlockProposalVoteLock> {
    read_json_file(path, "block proposal vote lock")
}

fn validate_block_proposal_vote_lock(
    lock: &BlockProposalVoteLock,
    genesis: &Genesis,
    target: &BlockVoteTarget,
    validator: &str,
    proposal_hash: &str,
) -> io::Result<()> {
    if lock.schema != BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA
        && lock.schema != BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported block proposal vote lock schema `{}`", lock.schema),
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if lock.chain_id != genesis.chain_id
        || lock.genesis_hash != expected_genesis_hash
        || lock.protocol_version != genesis.protocol_version
        || lock.block_height != target.evidence.height
        || (consensus_v2_active_at(genesis, target.evidence.height)
            && lock.view != target.evidence.view)
        || lock.validator != validator
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal vote lock target mismatch",
        ));
    }
    if lock.proposal_hash != proposal_hash {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "conflicting block proposal vote already recorded for validator `{validator}` at height {} (recorded view {}, attempted view {})",
                target.evidence.height, lock.view, target.evidence.view
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod block_proposal_vote_lock_tests {
    use super::*;

    fn target(view: u64, proposal_hash: &str) -> BlockVoteTarget {
        BlockVoteTarget {
            evidence: OwnedBlockEvidence {
                height: 14,
                view,
                parent_hash: "parent".to_string(),
                proposer: "validator-0".to_string(),
                batch_kind: "mempool".to_string(),
                batch_id: format!("batch-{view}"),
                state_root: "state".to_string(),
                bridge_exit_root: None,
                receipt_ids: Vec::new(),
                fastpay_pre_state_effects: Vec::new(),
            },
            validators: vec!["validator-0".to_string()],
            block_hash: None,
            proposal_hash: Some(proposal_hash.to_string()),
        }
    }

    fn test_store(label: &str) -> (PathBuf, NodeStore) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-block-proposal-lock-{label}-{}-{unique}",
            std::process::id()
        ));
        (data_dir.clone(), NodeStore::new(data_dir))
    }

    #[test]
    fn activated_consensus_v2_lock_rejects_same_view_equivocation_but_allows_timeout_view() {
        let (data_dir, store) = test_store("activated");
        let mut activated = Genesis::try_new_with_validator_count("lock-test".to_string(), 4)
            .expect("genesis");
        activated.consensus_v2_activation_height = Some(1);
        activated.validate().expect("activated genesis");

        reserve_block_proposal_vote_lock(
            &store,
            &activated,
            &target(0, "proposal-view-0"),
            "validator-0",
        )
        .expect("first view-0 vote lock");
        let same_view = reserve_block_proposal_vote_lock(
            &store,
            &activated,
            &target(0, "equivocating-view-0-proposal"),
            "validator-0",
        )
        .expect_err("same-view equivocation must remain locked out");
        assert_eq!(same_view.kind(), io::ErrorKind::AlreadyExists);
        reserve_block_proposal_vote_lock(
            &store,
            &activated,
            &target(2, "proposal-view-2"),
            "validator-0",
        )
        .expect("verified timeout view must receive an independent durable lock");

        let lock_count = std::fs::read_dir(data_dir.join(BLOCK_PROPOSAL_VOTE_LOCK_DIR))
            .expect("lock directory")
            .count();
        assert_eq!(lock_count, 2, "one durable lock is required per signed view");
        std::fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn legacy_lock_remains_height_wide_across_views() {
        let (data_dir, store) = test_store("legacy");
        let legacy = Genesis::try_new_with_validator_count("lock-test".to_string(), 4)
            .expect("legacy genesis");
        reserve_block_proposal_vote_lock(
            &store,
            &legacy,
            &target(0, "legacy-proposal"),
            "validator-0",
        )
        .expect("legacy lock");
        let cross_view = reserve_block_proposal_vote_lock(
            &store,
            &legacy,
            &target(2, "different-legacy-proposal"),
            "validator-0",
        )
        .expect_err("legacy behavior must remain height-wide");
        assert_eq!(cross_view.kind(), io::ErrorKind::AlreadyExists);
        std::fs::remove_dir_all(data_dir).expect("cleanup");
    }
}

pub fn aggregate_block_certificate(
    options: BlockCertificateOptions,
) -> io::Result<BlockCertificateFile> {
    if options.vote_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one block vote file is required",
        ));
    }
    if options.verify_block_log {
        verify_blocks(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let target = block_vote_target(
        &store,
        &genesis,
        options.batch_file.as_deref(),
        options.proposal_file.as_deref(),
        options.timeout_certificate_file.as_deref(),
        options.block_height,
    )?;
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;

    let mut votes_by_validator = HashMap::<String, BlockCertificateVote>::new();
    for vote_file in &options.vote_files {
        let vote = read_block_vote_file(vote_file)?;
        validate_block_vote_file_for_target(
            &vote,
            &genesis,
            &target.evidence,
            &target.validators,
            target.block_hash.as_deref(),
            target.proposal_hash.as_deref(),
        )?;
        if votes_by_validator
            .insert(vote.vote.validator.clone(), vote.vote)
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate block vote for `{}`", vote_file.display()),
            ));
        }
    }

    let quorum = block_certificate_quorum(&target.validators)?;
    let registry_root = validator_registry_root(&registry, &target.validators)?;
    if votes_by_validator.len() < quorum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "insufficient block votes: got {}, need {quorum}",
                votes_by_validator.len()
            ),
        ));
    }
    let mut votes = Vec::with_capacity(votes_by_validator.len());
    for expected_validator in &target.validators {
        if let Some(vote) = votes_by_validator.remove(expected_validator) {
            verify_block_certificate_vote_for_evidence(
                &genesis,
                &target.evidence.as_evidence(),
                &registry,
                &vote,
                expected_validator,
                &registry_root,
            )?;
            votes.push(vote);
        }
    }
    if let Some(unexpected_validator) = votes_by_validator.keys().next() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected block vote for `{unexpected_validator}`"),
        ));
    }

    let certificate = BlockCertificate {
        validators: target.validators,
        quorum,
        registry_root,
        votes,
    };
    let evidence_ref = target.evidence.as_evidence();
    let certificate_id = block_certificate_id(&genesis, &evidence_ref, &certificate)?;
    if let Some(block_hash) = target.block_hash.as_ref() {
        let blocks = store.read_blocks()?;
        let block = select_block(&blocks, options.block_height)?;
        if certificate_id != block.header.certificate_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "aggregated certificate id {certificate_id} does not match block {} certificate id {}",
                    block.header.height, block.header.certificate_id
                ),
            ));
        }
        if block_hash != &block.header.block_hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("block {} hash mismatch", block.header.height),
            ));
        }
    }
    let certificate_file = BlockCertificateFile {
        schema: BLOCK_CERTIFICATE_FILE_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height: target.evidence.height,
        view: target.evidence.view,
        proposer: target.evidence.proposer.clone(),
        block_hash: target.block_hash,
        proposal_hash: target.proposal_hash,
        certificate_id,
        certificate,
        fastpay_pre_state_effects: target.evidence.fastpay_pre_state_effects,
        consensus_v2_commit: None,
    };
    write_block_certificate_file(&options.certificate_file, &certificate_file)?;
    Ok(certificate_file)
}

pub fn reconstruct_block_certificate_from_archive(
    options: BlockCertificateFromArchiveOptions,
) -> io::Result<BlockCertificateFile> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let block: BlockRecord = read_json_file(&options.block_file, "block record")?;
    let payload_json = read_bounded_json_text_file(&options.batch_file, "archived batch payload")?;
    serde_json::from_str::<serde_json::Value>(&payload_json).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "archived batch payload `{}` is not valid JSON: {error}",
                options.batch_file.display()
            ),
        )
    })?;
    let payload_hash = batch_archive_payload_hash(
        &genesis,
        &block.header.batch_kind,
        &block.header.batch_id,
        &payload_json,
    )?;
    let evidence = BlockEvidence::from_block(&block);
    let proposal_hash = block_proposal_hash_from_evidence(&genesis, &evidence, &payload_hash)?;
    let certificate_file = BlockCertificateFile {
        schema: BLOCK_CERTIFICATE_FILE_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height: block.header.height,
        view: block.header.view,
        proposer: block.header.proposer.clone(),
        block_hash: Some(block.header.block_hash.clone()),
        proposal_hash: Some(proposal_hash.clone()),
        certificate_id: block.header.certificate_id.clone(),
        certificate: block.header.certificate.clone(),
        fastpay_pre_state_effects: block.fastpay_pre_state_effects.clone(),
        consensus_v2_commit: block.header.consensus_v2_commit.clone(),
    };
    verify_external_block_certificate(
        &genesis,
        &evidence,
        &certificate_file,
        Some(&proposal_hash),
        &registry,
        &block.header.certificate.validators,
    )?;
    write_block_certificate_file(&options.certificate_file, &certificate_file)?;
    Ok(certificate_file)
}

pub fn create_block_timeout_vote(
    options: BlockTimeoutVoteOptions,
) -> io::Result<BlockTimeoutVoteFile> {
    if options.verify_block_log {
        verify_blocks(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
    }
    if options.block_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "timeout vote block height must be positive",
        ));
    }
    if options.high_qc_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "timeout vote high_qc_id must be nonempty",
        ));
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let validators = active_validator_ids(&governance)?;
    let key_file = read_validator_key_file(&options.key_file)?;
    validate_validator_key_file(&key_file)?;
    let key_record = select_validator_key_record(&key_file, options.validator_id.as_deref())?;
    if !validators
        .iter()
        .any(|validator| validator == &key_record.node_id)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "validator `{}` is not in timeout validator set",
                key_record.node_id
            ),
        ));
    }

    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let registry_root = validator_registry_root(&registry, &validators)?;
    let registry_record = validator_registry_record(&registry, &key_record.node_id)?;
    if key_record.algorithm_id != registry_record.algorithm_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "validator key `{}` algorithm does not match registry",
                key_record.node_id
            ),
        ));
    }
    if key_record.public_key_hex != registry_record.public_key_hex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "validator key `{}` public key does not match registry",
                key_record.node_id
            ),
        ));
    }

    let message = block_timeout_vote_message(
        &genesis,
        options.block_height,
        options.view,
        &options.high_qc_id,
        &key_record.node_id,
        &registry_root,
    )?;
    let private_key = Zeroizing::new(hex_to_bytes(&key_record.private_key_hex).map_err(invalid_data)?);
    let signature_seed = block_timeout_signature_seed(&message)?;
    let signature = ml_dsa_65_sign_with_context_seed(
        &private_key,
        &message,
        BLOCK_TIMEOUT_SIGNATURE_CONTEXT,
        &signature_seed,
    )
    .map_err(invalid_data)?;
    let consensus_v2_vote = if consensus_v2_active_at(&genesis, options.block_height) {
        Some(create_consensus_v2_timeout_vote(
            &options.data_dir,
            postfiat_types::ConsensusV2Round {
                height: options.block_height,
                view: options.view,
            },
            &options.key_file,
            &key_record.node_id,
        )?)
    } else {
        None
    };
    let vote = BlockTimeoutVoteFile {
        schema: BLOCK_TIMEOUT_VOTE_FILE_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height: options.block_height,
        view: options.view,
        vote: BlockTimeoutVote {
            vote_id: block_timeout_vote_id(&message),
            validator: key_record.node_id.clone(),
            high_qc_id: options.high_qc_id,
            algorithm_id: key_record.algorithm_id.clone(),
            registry_root,
            public_key_hex: String::new(),
            signature_hex: bytes_to_hex(&signature),
        },
        consensus_v2_vote,
    };
    validate_block_timeout_vote_file_for_target(
        &vote,
        &genesis,
        options.block_height,
        options.view,
        &validators,
    )?;
    write_block_timeout_vote_file(&options.vote_file, &vote)?;
    Ok(vote)
}

pub fn aggregate_block_timeout_certificate(
    options: BlockTimeoutCertificateOptions,
) -> io::Result<BlockTimeoutCertificateFile> {
    if options.vote_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one block timeout vote file is required",
        ));
    }
    if options.verify_block_log {
        verify_blocks(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
    }
    if options.block_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "timeout certificate block height must be positive",
        ));
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let validators = active_validator_ids(&governance)?;
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let mut votes_by_validator = HashMap::<String, BlockTimeoutVote>::new();
    let mut consensus_v2_votes_by_validator =
        HashMap::<String, postfiat_types::ConsensusV2TimeoutVote>::new();
    for vote_file in &options.vote_files {
        let vote = read_block_timeout_vote_file(vote_file)?;
        validate_block_timeout_vote_file_for_target(
            &vote,
            &genesis,
            options.block_height,
            options.view,
            &validators,
        )?;
        let validator_id = vote.vote.validator.clone();
        if consensus_v2_active_at(&genesis, options.block_height) {
            let consensus_v2_vote = vote.consensus_v2_vote.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "activated consensus v2 timeout vote `{}` omitted v2 evidence",
                        vote_file.display()
                    ),
                )
            })?;
            if consensus_v2_vote.validator != validator_id
                || consensus_v2_votes_by_validator
                    .insert(validator_id.clone(), consensus_v2_vote)
                    .is_some()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "duplicate or mismatched consensus v2 timeout vote for `{}`",
                        vote_file.display()
                    ),
                ));
            }
        } else if vote.consensus_v2_vote.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "consensus v2 timeout vote appears before activation",
            ));
        }
        if votes_by_validator
            .insert(validator_id, vote.vote)
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate block timeout vote for `{}`", vote_file.display()),
            ));
        }
    }

    let quorum = block_certificate_quorum(&validators)?;
    let registry_root = validator_registry_root(&registry, &validators)?;
    if votes_by_validator.len() < quorum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "insufficient block timeout votes: got {}, need {quorum}",
                votes_by_validator.len()
            ),
        ));
    }
    let mut votes = Vec::with_capacity(votes_by_validator.len());
    for expected_validator in &validators {
        if let Some(vote) = votes_by_validator.remove(expected_validator) {
            verify_block_timeout_vote_for_target(
                &genesis,
                options.block_height,
                options.view,
                &registry,
                &vote,
                expected_validator,
                &registry_root,
            )?;
            votes.push(vote);
        }
    }
    if let Some(unexpected_validator) = votes_by_validator.keys().next() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected block timeout vote for `{unexpected_validator}`"),
        ));
    }
    validate_block_timeout_vote_set(&votes, &validators, "block timeout certificate")?;
    let high_qc_id = votes
        .iter()
        .map(|vote| vote.high_qc_id.clone())
        .max()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "timeout certificate has no votes",
            )
        })?;
    let hotstuff_certificate_id = verify_hotstuff_timeout_certificate(
        &genesis,
        &validators,
        options.block_height,
        options.view,
        &votes,
    )?;
    let certificate = BlockTimeoutCertificate {
        validators: validators.clone(),
        quorum,
        registry_root,
        high_qc_id,
        votes,
    };
    let certificate_id = block_timeout_certificate_id(
        &genesis,
        options.block_height,
        options.view,
        &hotstuff_certificate_id,
        &certificate,
    )?;
    let consensus_v2_certificate = if consensus_v2_active_at(&genesis, options.block_height) {
        let (domain, consensus_v2_validators) =
            live_consensus_v2_context(&options.data_dir)?;
        let graph = read_consensus_v2_qc_graph(
            &options.data_dir,
            &domain,
            &consensus_v2_validators,
        )?;
        let ordered_votes = validators
            .iter()
            .filter_map(|validator| consensus_v2_votes_by_validator.remove(validator))
            .collect::<Vec<_>>();
        if !consensus_v2_votes_by_validator.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected consensus v2 timeout vote validator",
            ));
        }
        Some(
            postfiat_ordering_fast::certify_consensus_v2_timeouts(
                &domain,
                &consensus_v2_validators,
                postfiat_types::ConsensusV2Round {
                    height: options.block_height,
                    view: options.view,
                },
                postfiat_types::ConsensusV2Phase::Precommit,
                ordered_votes,
                &graph,
            )
            .map_err(invalid_data)?,
        )
    } else {
        None
    };
    let certificate_file = BlockTimeoutCertificateFile {
        schema: BLOCK_TIMEOUT_CERTIFICATE_FILE_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        block_height: options.block_height,
        view: options.view,
        hotstuff_certificate_id,
        certificate_id,
        certificate,
        consensus_v2_certificate,
    };
    verify_block_timeout_certificate_material(&genesis, &registry, &validators, &certificate_file)?;
    verify_consensus_v2_timeout_extension(&options.data_dir, &genesis, &certificate_file)?;
    write_block_timeout_certificate_file(&options.certificate_file, &certificate_file)?;
    Ok(certificate_file)
}

pub fn verify_block_timeout_certificate_file(
    options: BlockTimeoutCertificateVerifyOptions,
) -> io::Result<BlockTimeoutCertificateFile> {
    if options.verify_block_log {
        verify_blocks(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let validators = active_validator_ids(&governance)?;
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let certificate = read_block_timeout_certificate_file(&options.certificate_file)?;
    verify_block_timeout_certificate_material(&genesis, &registry, &validators, &certificate)?;
    verify_consensus_v2_timeout_extension(&options.data_dir, &genesis, &certificate)?;
    Ok(certificate)
}

fn verify_consensus_v2_timeout_extension(
    data_dir: &Path,
    genesis: &Genesis,
    certificate: &BlockTimeoutCertificateFile,
) -> io::Result<()> {
    if consensus_v2_active_at(genesis, certificate.block_height) {
        let consensus_v2_certificate =
            certificate
                .consensus_v2_certificate
                .as_ref()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "activated timeout certificate omitted consensus v2 evidence",
                    )
                })?;
        let (domain, validators) = live_consensus_v2_context(data_dir)?;
        let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
        postfiat_ordering_fast::verify_consensus_v2_timeout_certificate(
            &domain,
            &validators,
            consensus_v2_certificate,
            &graph,
        )
        .map_err(invalid_data)?;
        if consensus_v2_certificate.round.height != certificate.block_height
            || consensus_v2_certificate.round.view != certificate.view
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "consensus v2 timeout target does not match legacy transport envelope",
            ));
        }
    } else if certificate.consensus_v2_certificate.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "consensus v2 timeout evidence appears before activation",
        ));
    }
    Ok(())
}

pub fn detect_block_vote_equivocation(
    options: BlockVoteEquivocationOptions,
) -> io::Result<BlockEquivocationEvidenceFile> {
    let evidence = build_block_vote_equivocation_evidence(&options)?;
    write_block_equivocation_evidence_file(&options.evidence_file, &evidence)?;
    Ok(evidence)
}

pub fn verify_block_vote_equivocation(
    options: BlockVoteEquivocationOptions,
) -> io::Result<BlockEquivocationEvidenceFile> {
    let expected = build_block_vote_equivocation_evidence(&options)?;
    let actual = read_block_equivocation_evidence_file(&options.evidence_file)?;
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block vote equivocation evidence mismatch",
        ));
    }
    Ok(actual)
}

fn build_block_vote_equivocation_evidence(
    options: &BlockVoteEquivocationOptions,
) -> io::Result<BlockEquivocationEvidenceFile> {
    verify_blocks(NodeOptions {
        data_dir: options.data_dir.clone(),
    })?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let validators = active_validator_ids(&governance)?;
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;

    let first_proposal = read_block_proposal_file(&options.first_proposal_file)?;
    let second_proposal = read_block_proposal_file(&options.second_proposal_file)?;
    validate_block_vote_equivocation_proposal(&first_proposal, &genesis, &validators)?;
    verify_block_proposal_signature_if_present(&store, &first_proposal)?;
    validate_block_vote_equivocation_proposal(&second_proposal, &genesis, &validators)?;
    verify_block_proposal_signature_if_present(&store, &second_proposal)?;
    if first_proposal.block_height != second_proposal.block_height
        || first_proposal.view != second_proposal.view
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block vote equivocation proposals must target the same height and view",
        ));
    }

    let first_proposal_hash = block_proposal_hash(&first_proposal)?;
    let second_proposal_hash = block_proposal_hash(&second_proposal)?;
    if first_proposal_hash == second_proposal_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block vote equivocation requires conflicting proposal targets",
        ));
    }

    let first_vote = read_block_vote_file(&options.first_vote_file)?;
    let second_vote = read_block_vote_file(&options.second_vote_file)?;
    let first_evidence = OwnedBlockEvidence::from_proposal(&first_proposal);
    let second_evidence = OwnedBlockEvidence::from_proposal(&second_proposal);
    validate_block_vote_file_for_target(
        &first_vote,
        &genesis,
        &first_evidence,
        &validators,
        None,
        Some(&first_proposal_hash),
    )?;
    validate_block_vote_file_for_target(
        &second_vote,
        &genesis,
        &second_evidence,
        &validators,
        None,
        Some(&second_proposal_hash),
    )?;
    let registry_root = if !first_vote.vote.registry_root.is_empty()
        || !second_vote.vote.registry_root.is_empty()
    {
        validator_registry_root(&registry, &validators)?
    } else {
        String::new()
    };
    verify_block_certificate_vote_for_evidence(
        &genesis,
        &first_evidence.as_evidence(),
        &registry,
        &first_vote.vote,
        &first_vote.vote.validator,
        &registry_root,
    )?;
    verify_block_certificate_vote_for_evidence(
        &genesis,
        &second_evidence.as_evidence(),
        &registry,
        &second_vote.vote,
        &second_vote.vote.validator,
        &registry_root,
    )?;
    if first_vote.vote.validator != second_vote.vote.validator {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block vote equivocation requires two votes from the same validator",
        ));
    }
    if first_vote.vote.vote_id == second_vote.vote.vote_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block vote equivocation requires distinct signed votes",
        ));
    }

    let (first_evidence_id, second_evidence_id, first_target_hash, second_target_hash) =
        if first_proposal_hash <= second_proposal_hash {
            (
                first_vote.vote.vote_id.clone(),
                second_vote.vote.vote_id.clone(),
                first_proposal_hash,
                second_proposal_hash,
            )
        } else {
            (
                second_vote.vote.vote_id.clone(),
                first_vote.vote.vote_id.clone(),
                second_proposal_hash,
                first_proposal_hash,
            )
        };
    let evidence_id = block_equivocation_evidence_id(BlockEquivocationEvidenceIdInput {
        genesis: &genesis,
        kind: "block_vote",
        block_height: first_proposal.block_height,
        view: first_proposal.view,
        validator: &first_vote.vote.validator,
        first_evidence_id: &first_evidence_id,
        second_evidence_id: &second_evidence_id,
        first_target_hash: &first_target_hash,
        second_target_hash: &second_target_hash,
    })?;
    let evidence = BlockEquivocationEvidenceFile {
        schema: BLOCK_EQUIVOCATION_EVIDENCE_FILE_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        kind: "block_vote".to_string(),
        block_height: first_proposal.block_height,
        view: first_proposal.view,
        validator: first_vote.vote.validator,
        first_evidence_kind: "block_vote".to_string(),
        second_evidence_kind: "block_vote".to_string(),
        first_evidence_id,
        second_evidence_id,
        first_target_kind: "proposal".to_string(),
        second_target_kind: "proposal".to_string(),
        first_target_hash,
        second_target_hash,
        evidence_id,
    };
    Ok(evidence)
}

pub fn detect_block_proposal_equivocation(
    options: BlockProposalEquivocationOptions,
) -> io::Result<BlockEquivocationEvidenceFile> {
    let evidence = build_block_proposal_equivocation_evidence(&options)?;
    write_block_equivocation_evidence_file(&options.evidence_file, &evidence)?;
    Ok(evidence)
}

pub fn verify_block_proposal_equivocation(
    options: BlockProposalEquivocationOptions,
) -> io::Result<BlockEquivocationEvidenceFile> {
    let expected = build_block_proposal_equivocation_evidence(&options)?;
    let actual = read_block_equivocation_evidence_file(&options.evidence_file)?;
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal equivocation evidence mismatch",
        ));
    }
    Ok(actual)
}

fn build_block_proposal_equivocation_evidence(
    options: &BlockProposalEquivocationOptions,
) -> io::Result<BlockEquivocationEvidenceFile> {
    verify_blocks(NodeOptions {
        data_dir: options.data_dir.clone(),
    })?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let validators = active_validator_ids(&governance)?;

    let first_proposal = read_block_proposal_file(&options.first_proposal_file)?;
    let second_proposal = read_block_proposal_file(&options.second_proposal_file)?;
    validate_block_vote_equivocation_proposal(&first_proposal, &genesis, &validators)?;
    validate_block_vote_equivocation_proposal(&second_proposal, &genesis, &validators)?;
    require_signed_block_proposal(&first_proposal)?;
    require_signed_block_proposal(&second_proposal)?;
    verify_block_proposal_signature_if_present(&store, &first_proposal)?;
    verify_block_proposal_signature_if_present(&store, &second_proposal)?;
    if first_proposal.block_height != second_proposal.block_height
        || first_proposal.view != second_proposal.view
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal equivocation requires proposals for the same height and view",
        ));
    }
    if first_proposal.proposer != second_proposal.proposer {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal equivocation requires proposals from the same proposer",
        ));
    }

    let first_proposal_hash = block_proposal_hash(&first_proposal)?;
    let second_proposal_hash = block_proposal_hash(&second_proposal)?;
    if first_proposal_hash == second_proposal_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal equivocation requires conflicting proposal targets",
        ));
    }

    let (first_evidence_id, second_evidence_id, first_target_hash, second_target_hash) =
        if first_proposal_hash <= second_proposal_hash {
            (
                first_proposal_hash.clone(),
                second_proposal_hash.clone(),
                first_proposal_hash,
                second_proposal_hash,
            )
        } else {
            (
                second_proposal_hash.clone(),
                first_proposal_hash.clone(),
                second_proposal_hash,
                first_proposal_hash,
            )
        };
    let evidence_id = block_equivocation_evidence_id(BlockEquivocationEvidenceIdInput {
        genesis: &genesis,
        kind: "block_proposal",
        block_height: first_proposal.block_height,
        view: first_proposal.view,
        validator: &first_proposal.proposer,
        first_evidence_id: &first_evidence_id,
        second_evidence_id: &second_evidence_id,
        first_target_hash: &first_target_hash,
        second_target_hash: &second_target_hash,
    })?;
    let evidence = BlockEquivocationEvidenceFile {
        schema: BLOCK_EQUIVOCATION_EVIDENCE_FILE_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        kind: "block_proposal".to_string(),
        block_height: first_proposal.block_height,
        view: first_proposal.view,
        validator: first_proposal.proposer,
        first_evidence_kind: "block_proposal".to_string(),
        second_evidence_kind: "block_proposal".to_string(),
        first_evidence_id,
        second_evidence_id,
        first_target_kind: "proposal".to_string(),
        second_target_kind: "proposal".to_string(),
        first_target_hash,
        second_target_hash,
        evidence_id,
    };
    Ok(evidence)
}

pub fn certify_batch_round(
    options: BatchCertificateRoundOptions,
) -> io::Result<BatchCertificateRoundReport> {
    if !options.validator_key_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "validator key directory `{}` not found",
                options.validator_key_dir.display()
            ),
        ));
    }
    std::fs::create_dir_all(&options.vote_dir)?;
    if let Some(parent) = options
        .proposal_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(parent) = options
        .certificate_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    let mut proposal = propose_batch(BatchProposalOptions {
        data_dir: options.data_dir.clone(),
        verify_block_log: !options.skip_block_log_verify,
        batch_kind: options.batch_kind.clone(),
        batch_file: options.batch_file.clone(),
        proposal_file: options.proposal_file.clone(),
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        key_file: None,
        validator_id: None,
    })?;
    let store = NodeStore::new(&options.data_dir);
    let proposer = proposal.proposer.clone();
    let proposer_key_file = options
        .validator_key_dir
        .join(format!("{proposer}.validator_keys.json"));
    sign_block_proposal_file(
        &store,
        &mut proposal,
        &proposer_key_file,
        Some(proposer.as_str()),
    )?;
    write_block_proposal_file(&options.proposal_file, &proposal)?;
    if let Some(block_height) = options.block_height {
        if proposal.block_height != block_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "proposed block height {} does not match --height {block_height}",
                    proposal.block_height
                ),
            ));
        }
    }

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let target = block_vote_target(
        &store,
        &genesis,
        Some(&options.batch_file),
        Some(&options.proposal_file),
        options.timeout_certificate_file.as_deref(),
        Some(proposal.block_height),
    )?;
    let proposal_hash = target.proposal_hash.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "certified batch round did not produce proposal hash",
        )
    })?;

    let mut vote_files = Vec::with_capacity(target.validators.len());
    for validator_id in &target.validators {
        let key_file = options
            .validator_key_dir
            .join(format!("{validator_id}.validator_keys.json"));
        if !key_file.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "split validator key file `{}` not found",
                    key_file.display()
                ),
            ));
        }
        let vote_file = options
            .vote_dir
            .join(format!("{validator_id}.block_vote.json"));
        let vote = create_block_vote(BlockVoteOptions {
            data_dir: options.data_dir.clone(),
            verify_block_log: !options.skip_block_log_verify,
            key_file,
            validator_id: Some(validator_id.clone()),
            batch_file: Some(options.batch_file.clone()),
            proposal_file: Some(options.proposal_file.clone()),
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            block_height: Some(proposal.block_height),
            vote_file: vote_file.clone(),
        })?;
        if vote.vote.validator != *validator_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "split vote validator `{}` did not match expected `{validator_id}`",
                    vote.vote.validator
                ),
            ));
        }
        vote_files.push(vote_file);
    }

    let certificate = aggregate_block_certificate(BlockCertificateOptions {
        data_dir: options.data_dir.clone(),
        verify_block_log: !options.skip_block_log_verify,
        batch_file: Some(options.batch_file.clone()),
        proposal_file: Some(options.proposal_file.clone()),
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        block_height: Some(proposal.block_height),
        vote_files: vote_files.clone(),
        certificate_file: options.certificate_file.clone(),
    })?;
    if certificate.certificate.validators != target.validators {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "certified batch round validator set mismatch",
        ));
    }
    let expected_quorum = block_certificate_quorum(&target.validators)?;
    if certificate.certificate.quorum != expected_quorum
        || certificate.certificate.votes.len() < expected_quorum
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "certified batch round quorum mismatch",
        ));
    }

    Ok(BatchCertificateRoundReport {
        schema: "postfiat.batch_certificate_round.v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        batch_kind: proposal.batch_kind,
        batch_id: proposal.batch_id,
        block_height: proposal.block_height,
        view: proposal.view,
        proposal_hash,
        certificate_id: certificate.certificate_id,
        validators: target.validators,
        vote_count: certificate.certificate.votes.len(),
        proposal_file: options.proposal_file.display().to_string(),
        certificate_file: options.certificate_file.display().to_string(),
        vote_dir: options.vote_dir.display().to_string(),
        vote_files: vote_files
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        private_key_policy: CertificateRoundPrivateKeyPolicy {
            split_key_files: true,
            private_key_material_redacted: true,
        },
        round_ok: true,
    })
}

fn select_block(blocks: &BlockLog, block_height: Option<u64>) -> io::Result<&BlockRecord> {
    match block_height {
        Some(height) => blocks
            .blocks
            .iter()
            .find(|block| block.header.height == height)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, format!("block {height} not found"))
            }),
        None => blocks.blocks.last().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "block log has no committed blocks")
        }),
    }
}

fn select_validator_key_record<'a>(
    key_file: &'a ValidatorKeyFile,
    validator_id: Option<&str>,
) -> io::Result<&'a ValidatorKeyRecord> {
    if let Some(validator_id) = validator_id.filter(|validator_id| !validator_id.is_empty()) {
        return validator_key_record(key_file, validator_id);
    }
    match key_file.validators.as_slice() {
        [record] => Ok(record),
        [] => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validator key file has no validators",
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validator key file has multiple validators; pass --validator",
        )),
    }
}

struct BlockVoteTargetWithTimings {
    target: BlockVoteTarget,
    timings: BlockVoteTargetTimingReport,
}

fn block_vote_target(
    store: &NodeStore,
    genesis: &Genesis,
    batch_file: Option<&Path>,
    proposal_file: Option<&Path>,
    timeout_certificate_file: Option<&Path>,
    block_height: Option<u64>,
) -> io::Result<BlockVoteTarget> {
    Ok(block_vote_target_with_timings(
        store,
        genesis,
        batch_file,
        proposal_file,
        timeout_certificate_file,
        block_height,
    )?
        .target)
}

fn block_vote_target_with_timings(
    store: &NodeStore,
    genesis: &Genesis,
    batch_file: Option<&Path>,
    proposal_file: Option<&Path>,
    timeout_certificate_file: Option<&Path>,
    block_height: Option<u64>,
) -> io::Result<BlockVoteTargetWithTimings> {
    let total_start = std::time::Instant::now();
    let mut timings = BlockVoteTargetTimingReport {
        schema: "postfiat.block_vote_target_timing.v1".to_string(),
        ..BlockVoteTargetTimingReport::default()
    };
    if let Some(proposal_file) = proposal_file {
        let batch_file = batch_file.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "--batch-file is required with --proposal-file",
            )
        })?;
        let stage_start = std::time::Instant::now();
        let proposal = read_block_proposal_file(proposal_file)?;
        timings.proposal_read_ms = node_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        validate_block_proposal_file(&proposal, genesis)?;
        timings.proposal_validate_ms = node_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        verify_block_proposal_signature_if_present(store, &proposal)?;
        timings.proposal_signature_verify_ms = node_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        if let Some(block_height) = block_height {
            if proposal.block_height != block_height {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "proposal block height {} does not match --height {block_height}",
                        proposal.block_height
                    ),
                ));
            }
        }
        timings.proposal_height_check_ms = node_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        validate_block_proposal_timeout_evidence(
            store,
            &proposal,
            timeout_certificate_file,
            false,
        )?;
        timings.timeout_evidence_ms = node_timing_elapsed_ms(stage_start);

        reset_asset_orchard_private_egress_timings();
        reset_asset_orchard_swap_timings();
        reset_asset_orchard_private_egress_node_timings();
        let stage_start = std::time::Instant::now();
        let (expected, shielded_breakdown) = build_ordered_batch_proposal_with_timings(
            store,
            &proposal.batch_kind,
            batch_file,
            Some(proposal.view),
            Some(&proposal.fastpay_pre_state_effects),
        )?;
        let mut comparable_proposal = proposal.clone();
        comparable_proposal.signature = None;
        if expected != comparable_proposal {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "block proposal does not match local batch and state",
            ));
        }
        timings.proposal_rebuild_compare_ms = node_timing_elapsed_ms(stage_start);
        let mut private_egress_verifier_breakdown = take_asset_orchard_private_egress_timings();
        let asset_orchard_swap_verifier_breakdown = take_asset_orchard_swap_timings();
        let mut private_egress_state_breakdown = take_asset_orchard_private_egress_node_timings();
        if let Some(shielded_breakdown) = shielded_breakdown {
            if private_egress_verifier_breakdown.is_empty() {
                private_egress_verifier_breakdown =
                    shielded_breakdown.private_egress_verifier_breakdown.clone();
            }
            if private_egress_state_breakdown.is_empty() {
                private_egress_state_breakdown =
                    shielded_breakdown.private_egress_state_breakdown.clone();
            }
        }
        // The shielded proposal report clones the same thread-local collectors
        // returned above. Select one source, then sum once; adding both would
        // report every proof verification twice.
        timings.local_proof_verify_ms = private_egress_verifier_breakdown
            .proof_verifications
            .iter()
            .map(|timing| timing.halo2_verify_proof_ms)
            .sum::<f64>()
            + asset_orchard_swap_verifier_breakdown
                .proof_verifications
                .iter()
                .map(|timing| timing.halo2_verify_proof_ms)
                .sum::<f64>();
        timings.private_egress_verifier_breakdown = private_egress_verifier_breakdown;
        timings.private_egress_state_breakdown = private_egress_state_breakdown;
        timings.asset_orchard_swap_verifier_breakdown =
            asset_orchard_swap_verifier_breakdown;

        let stage_start = std::time::Instant::now();
        let governance = governance_with_due_validator_registry_activations(
            store,
            genesis,
            proposal.block_height,
        )?;
        validate_bridge_exit_root_activation(&proposal, genesis, &governance)?;
        timings.governance_ms = node_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        let validators = active_validator_ids(&governance)?;
        timings.active_validators_ms = node_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        let proposal_hash = block_proposal_hash(&proposal)?;
        timings.proposal_hash_ms = node_timing_elapsed_ms(stage_start);
        timings.total_ms = node_timing_elapsed_ms(total_start);
        if timings.local_proof_verify_ms > 0.0 {
            if let Err(error) = record_local_proof_verify_latency(store, timings.local_proof_verify_ms)
            {
                eprintln!("WARN failed to persist non-gating proof latency telemetry: {error}");
            }
        }
        return Ok(BlockVoteTargetWithTimings {
            target: BlockVoteTarget {
            evidence: OwnedBlockEvidence::from_proposal(&proposal),
            validators,
            block_hash: None,
            proposal_hash: Some(proposal_hash),
            },
            timings,
        });
    }
    if batch_file.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--batch-file requires --proposal-file",
        ));
    }
    let stage_start = std::time::Instant::now();
    let blocks = store.read_blocks()?;
    let block = select_block(&blocks, block_height)?;
    timings.block_read_ms = node_timing_elapsed_ms(stage_start);
    timings.total_ms = node_timing_elapsed_ms(total_start);
    Ok(BlockVoteTargetWithTimings {
        target: BlockVoteTarget {
            evidence: OwnedBlockEvidence::from_block(block),
            validators: block.header.certificate.validators.clone(),
            block_hash: Some(block.header.block_hash.clone()),
            proposal_hash: None,
        },
        timings,
    })
}

fn validate_block_proposal_file(proposal: &BlockProposalFile, genesis: &Genesis) -> io::Result<()> {
    if proposal.schema != BLOCK_PROPOSAL_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported block proposal schema `{}`", proposal.schema),
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if proposal.chain_id != genesis.chain_id
        || proposal.genesis_hash != expected_genesis_hash
        || proposal.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal chain domain mismatch",
        ));
    }
    if proposal.block_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal height must be nonzero",
        ));
    }
    normalize_block_proposal_batch_kind(Some(&proposal.batch_kind))?;
    if proposal.parent_hash.is_empty()
        || proposal.proposer.is_empty()
        || proposal.batch_id.is_empty()
        || proposal.payload_hash.is_empty()
        || proposal.state_root.is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal contains empty evidence fields",
        ));
    }
    if proposal.receipt_count != proposal.receipt_ids.len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal receipt count mismatch",
        ));
    }
    validate_fastpay_pre_state_effects(&proposal.fastpay_pre_state_effects)?;
    if let Some(signature) = proposal.signature.as_ref() {
        if signature.signer.is_empty()
            || signature.algorithm_id.is_empty()
            || signature.public_key_hex.is_empty()
            || signature.signature_hex.is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "block proposal signature contains empty fields",
            ));
        }
    }
    Ok(())
}

fn validate_bridge_exit_root_activation(
    proposal: &BlockProposalFile,
    genesis: &Genesis,
    governance: &GovernanceState,
) -> io::Result<()> {
    let active = bridge_exit_root_activation_height_for_chain(governance)
        .is_some_and(|height| proposal.block_height >= height);
    match (active, proposal.bridge_exit_root.as_deref()) {
        (false, None) => Ok(()),
        (false, Some(_)) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal supplies bridge_exit_root before activation",
        )),
        (true, None) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "activated block proposal omits bridge_exit_root",
        )),
        (true, Some(root)) => {
            if !consensus_v2_active_at(genesis, proposal.block_height) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bridge-exit-root activation requires consensus v2 finality",
                ));
            }
            validate_lower_hex_len("block_proposal.bridge_exit_root", root, 96)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }
    }
}

#[cfg(test)]
mod bridge_exit_root_activation_tests {
    use super::*;

    fn proposal(height: u64, root: Option<String>) -> BlockProposalFile {
        let genesis = Genesis::new("postfiat-tier4-test");
        BlockProposalFile {
            schema: BLOCK_PROPOSAL_FILE_SCHEMA.to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            block_height: height,
            view: 0,
            parent_hash: "11".repeat(48),
            proposer: "validator-0".to_string(),
            batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
            batch_id: "22".repeat(48),
            payload_hash: "33".repeat(48),
            state_root: "44".repeat(48),
            bridge_exit_root: root,
            receipt_count: 0,
            receipt_ids: Vec::new(),
            fastpay_pre_state_effects: Vec::new(),
            signature: None,
        }
    }

    fn governance(activation_height: u32) -> GovernanceState {
        let mut governance = GovernanceState::new(1);
        governance.apply(GovernanceAmendment {
            amendment_id: "bridge-exit-root-activation".to_string(),
            chain_id: "postfiat-tier4-test".to_string(),
            genesis_hash: "aa".repeat(48),
            protocol_version: 1,
            instance_id: "tier4".to_string(),
            proposal_id: "tier4-proposal".to_string(),
            certificate_id: "tier4-certificate".to_string(),
            proposer: "validator-0".to_string(),
            validators: vec!["validator-0".to_string()],
            quorum: 1,
            kind: GOVERNANCE_KIND_BRIDGE_EXIT_ROOT_ACTIVATION_HEIGHT.to_string(),
            value: activation_height,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            support: vec!["validator-0".to_string()],
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
        });
        governance
    }

    #[test]
    fn bridge_exit_root_encoding_is_exactly_activation_gated() {
        let mut genesis = Genesis::new("postfiat-tier4-test");
        genesis.consensus_v2_activation_height = Some(1);
        let governance = governance(10);
        let root = postfiat_types::bridge_exit_empty_root_v1();

        validate_bridge_exit_root_activation(&proposal(9, None), &genesis, &governance)
            .expect("legacy encoding before activation");
        assert!(validate_bridge_exit_root_activation(
            &proposal(9, Some(root.clone())),
            &genesis,
            &governance,
        )
        .is_err());
        assert!(validate_bridge_exit_root_activation(
            &proposal(10, None),
            &genesis,
            &governance,
        )
        .is_err());
        validate_bridge_exit_root_activation(
            &proposal(10, Some(root)),
            &genesis,
            &governance,
        )
        .expect("v1 exit root at activation");
    }
}

fn sign_block_proposal_file(
    store: &NodeStore,
    proposal: &mut BlockProposalFile,
    key_file: &Path,
    validator_id: Option<&str>,
) -> io::Result<()> {
    if proposal.signature.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "block proposal is already signed",
        ));
    }
    let key_file = read_validator_key_file(key_file)?;
    validate_validator_key_file(&key_file)?;
    let key_record = select_validator_key_record(&key_file, validator_id)?;
    if key_record.node_id != proposal.proposer {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "proposal signer `{}` does not match proposer `{}`",
                key_record.node_id, proposal.proposer
            ),
        ));
    }
    let genesis = store.read_genesis()?;
    let governance =
        governance_with_due_validator_registry_activations(store, &genesis, proposal.block_height)?;
    validate_bridge_exit_root_activation(proposal, &genesis, &governance)?;
    let validators = active_validator_ids(&governance)?;
    if !validators
        .iter()
        .any(|validator| validator == &key_record.node_id)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "proposal signer `{}` is not in validator set",
                key_record.node_id
            ),
        ));
    }
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let registry_record = validator_registry_record(&registry, &key_record.node_id)?;
    if key_record.algorithm_id != registry_record.algorithm_id
        || key_record.public_key_hex != registry_record.public_key_hex
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "proposal signer `{}` key does not match registry",
                key_record.node_id
            ),
        ));
    }

    let message = block_proposal_signature_message(proposal)?;
    let private_key = Zeroizing::new(hex_to_bytes(&key_record.private_key_hex).map_err(invalid_data)?);
    let signature_seed = block_proposal_signature_seed(&message)?;
    let signature = ml_dsa_65_sign_with_context_seed(
        &private_key,
        &message,
        BLOCK_PROPOSAL_SIGNATURE_CONTEXT,
        &signature_seed,
    )
    .map_err(invalid_data)?;
    proposal.signature = Some(BlockProposalSignature {
        signer: key_record.node_id.clone(),
        algorithm_id: key_record.algorithm_id.clone(),
        public_key_hex: key_record.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    });
    Ok(())
}

fn verify_block_proposal_signature_if_present(
    store: &NodeStore,
    proposal: &BlockProposalFile,
) -> io::Result<()> {
    let Some(signature) = proposal.signature.as_ref() else {
        return Ok(());
    };
    if signature.signer != proposal.proposer {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal signature signer does not match proposer",
        ));
    }
    let genesis = store.read_genesis()?;
    let governance =
        governance_with_due_validator_registry_activations(store, &genesis, proposal.block_height)?;
    validate_bridge_exit_root_activation(proposal, &genesis, &governance)?;
    let validators = active_validator_ids(&governance)?;
    if !validators
        .iter()
        .any(|validator| validator == &signature.signer)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block proposal signature signer `{}` is not in validator set",
                signature.signer
            ),
        ));
    }
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let registry_record = validator_registry_record(&registry, &signature.signer)?;
    if signature.algorithm_id != registry_record.algorithm_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal signature algorithm mismatch",
        ));
    }
    if signature.public_key_hex != registry_record.public_key_hex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal signature public key mismatch",
        ));
    }
    let message = block_proposal_signature_message(proposal)?;
    let public_key = hex_to_bytes(&signature.public_key_hex).map_err(invalid_data)?;
    let signature_bytes = hex_to_bytes(&signature.signature_hex).map_err(invalid_data)?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &message,
        &signature_bytes,
        BLOCK_PROPOSAL_SIGNATURE_CONTEXT,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal signature mismatch",
        ));
    }
    Ok(())
}

fn require_signed_block_proposal(proposal: &BlockProposalFile) -> io::Result<()> {
    if proposal.signature.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal equivocation evidence requires signed proposals",
        ));
    }
    Ok(())
}

fn block_proposal_hash(proposal: &BlockProposalFile) -> io::Result<String> {
    block_proposal_hash_fields(BlockProposalHashInput {
        chain_id: proposal.chain_id.as_str(),
        genesis_hash: proposal.genesis_hash.as_str(),
        protocol_version: proposal.protocol_version,
        block_height: proposal.block_height,
        view: proposal.view,
        parent_hash: proposal.parent_hash.as_str(),
        proposer: proposal.proposer.as_str(),
        batch_kind: proposal.batch_kind.as_str(),
        batch_id: proposal.batch_id.as_str(),
        payload_hash: proposal.payload_hash.as_str(),
        state_root: proposal.state_root.as_str(),
        bridge_exit_root: proposal.bridge_exit_root.as_deref(),
        receipt_ids: &proposal.receipt_ids,
        fastpay_pre_state_effects: &proposal.fastpay_pre_state_effects,
    })
}

fn validate_block_proposal_timeout_evidence(
    store: &NodeStore,
    proposal: &BlockProposalFile,
    timeout_certificate_file: Option<&Path>,
    _verify_block_log: bool,
) -> io::Result<()> {
    if proposal.view == 0 {
        if timeout_certificate_file.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "view 0 proposal must not include timeout certificate evidence",
            ));
        }
        return Ok(());
    }
    let genesis = store.read_genesis()?;
    if !consensus_v2_active_at(&genesis, proposal.block_height) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "nonzero-view block proposals require activated consensus v2",
        ));
    }
    let timeout_certificate_file = timeout_certificate_file.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "nonzero-view block proposal omitted timeout certificate",
        )
    })?;
    let certificate = read_block_timeout_certificate_file(timeout_certificate_file)?;
    let governance = store.read_governance()?;
    let validators = active_validator_ids(&governance)?;
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    verify_block_timeout_certificate_material(&genesis, &registry, &validators, &certificate)?;
    verify_consensus_v2_timeout_extension(store.data_dir(), &genesis, &certificate)?;
    if certificate.block_height != proposal.block_height
        || certificate
            .view
            .checked_add(1)
            .is_none_or(|next_view| next_view != proposal.view)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "timeout certificate does not authorize the proposed height/view",
        ));
    }
    Ok(())
}

fn validate_block_vote_equivocation_proposal(
    proposal: &BlockProposalFile,
    genesis: &Genesis,
    validators: &[String],
) -> io::Result<()> {
    validate_block_proposal_file(proposal, genesis)?;
    let expected_proposer =
        leader_for_view(validators, proposal.block_height, proposal.view).map_err(invalid_data)?;
    if proposal.proposer != expected_proposer {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block vote equivocation proposal proposer mismatch: expected {expected_proposer}, got {}",
                proposal.proposer
            ),
        ));
    }
    Ok(())
}

struct BlockEquivocationEvidenceIdInput<'a> {
    genesis: &'a Genesis,
    kind: &'a str,
    block_height: u64,
    view: u64,
    validator: &'a str,
    first_evidence_id: &'a str,
    second_evidence_id: &'a str,
    first_target_hash: &'a str,
    second_target_hash: &'a str,
}

fn block_equivocation_evidence_id(
    fields: BlockEquivocationEvidenceIdInput<'_>,
) -> io::Result<String> {
    let encoded = serde_json::to_vec(&(
        fields.genesis.chain_id.as_str(),
        genesis_hash(fields.genesis),
        fields.genesis.protocol_version,
        fields.kind,
        fields.block_height,
        fields.view,
        fields.validator,
        fields.first_evidence_id,
        fields.second_evidence_id,
        fields.first_target_hash,
        fields.second_target_hash,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex(
        "postfiat.block_equivocation_evidence.v1",
        &encoded,
    ))
}

fn block_proposal_hash_from_evidence(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    payload_hash: &str,
) -> io::Result<String> {
    block_proposal_hash_fields(BlockProposalHashInput {
        chain_id: genesis.chain_id.as_str(),
        genesis_hash: &genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        block_height: evidence.height,
        view: evidence.view,
        parent_hash: evidence.parent_hash,
        proposer: evidence.proposer,
        batch_kind: evidence.batch_kind,
        batch_id: evidence.batch_id,
        payload_hash,
        state_root: evidence.state_root,
        bridge_exit_root: evidence.bridge_exit_root,
        receipt_ids: evidence.receipt_ids,
        fastpay_pre_state_effects: evidence.fastpay_pre_state_effects,
    })
}

struct BlockProposalHashInput<'a> {
    chain_id: &'a str,
    genesis_hash: &'a str,
    protocol_version: u32,
    block_height: u64,
    view: u64,
    parent_hash: &'a str,
    proposer: &'a str,
    batch_kind: &'a str,
    batch_id: &'a str,
    payload_hash: &'a str,
    state_root: &'a str,
    bridge_exit_root: Option<&'a str>,
    receipt_ids: &'a [String],
    fastpay_pre_state_effects: &'a [postfiat_types::FastPayVersionFenceV1],
}

fn block_proposal_hash_fields(fields: BlockProposalHashInput<'_>) -> io::Result<String> {
    if let Some(bridge_exit_root) = fields.bridge_exit_root {
        let encoded = serde_json::to_vec(&(
            "postfiat.block_proposal.v3",
            fields.chain_id,
            fields.genesis_hash,
            fields.protocol_version,
            fields.block_height,
            fields.view,
            fields.parent_hash,
            fields.proposer,
            fields.batch_kind,
            fields.batch_id,
            fields.payload_hash,
            fields.state_root,
            bridge_exit_root,
            fields.receipt_ids,
            fields.fastpay_pre_state_effects,
        ))
        .map_err(invalid_data)?;
        return Ok(hash_hex("postfiat.block_proposal.v3", &encoded));
    }
    if fields.fastpay_pre_state_effects.is_empty() {
        let encoded = serde_json::to_vec(&(
            fields.chain_id,
            fields.genesis_hash,
            fields.protocol_version,
            fields.block_height,
            fields.view,
            fields.parent_hash,
            fields.proposer,
            fields.batch_kind,
            fields.batch_id,
            fields.payload_hash,
            fields.state_root,
            fields.receipt_ids,
        ))
        .map_err(invalid_data)?;
        return Ok(hash_hex("postfiat.block_proposal.v1", &encoded));
    }
    let encoded = serde_json::to_vec(&(
        fields.chain_id,
        fields.genesis_hash,
        fields.protocol_version,
        fields.block_height,
        fields.view,
        fields.parent_hash,
        fields.proposer,
        fields.batch_kind,
        fields.batch_id,
        fields.payload_hash,
        fields.state_root,
        fields.receipt_ids,
        fields.fastpay_pre_state_effects,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex("postfiat.block_proposal.v2", &encoded))
}

fn validate_block_vote_file_for_target(
    vote_file: &BlockVoteFile,
    genesis: &Genesis,
    evidence: &OwnedBlockEvidence,
    expected_validators: &[String],
    expected_block_hash: Option<&str>,
    expected_proposal_hash: Option<&str>,
) -> io::Result<()> {
    if vote_file.schema != BLOCK_VOTE_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported block vote schema `{}`", vote_file.schema),
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if vote_file.chain_id != genesis.chain_id
        || vote_file.genesis_hash != expected_genesis_hash
        || vote_file.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block vote chain domain mismatch",
        ));
    }
    if vote_file.block_height != evidence.height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block vote height {} does not match block {}",
                vote_file.block_height, evidence.height
            ),
        ));
    }
    if vote_file.view != evidence.view {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block vote view {} does not match block view {}",
                vote_file.view, evidence.view
            ),
        ));
    }
    if vote_file.block_hash.as_deref() != expected_block_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block vote hash mismatch",
        ));
    }
    if vote_file.proposal_hash.as_deref() != expected_proposal_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block vote proposal hash mismatch",
        ));
    }
    if !expected_validators
        .iter()
        .any(|validator| validator == &vote_file.vote.validator)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block vote validator `{}` is not in certificate set",
                vote_file.vote.validator
            ),
        ));
    }
    Ok(())
}

fn validate_block_timeout_vote_file_for_target(
    vote_file: &BlockTimeoutVoteFile,
    genesis: &Genesis,
    block_height: u64,
    view: u64,
    expected_validators: &[String],
) -> io::Result<()> {
    if vote_file.schema != BLOCK_TIMEOUT_VOTE_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported block timeout vote schema `{}`",
                vote_file.schema
            ),
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if vote_file.chain_id != genesis.chain_id
        || vote_file.genesis_hash != expected_genesis_hash
        || vote_file.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout vote chain domain mismatch",
        ));
    }
    if vote_file.block_height != block_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block timeout vote height {} does not match block {block_height}",
                vote_file.block_height
            ),
        ));
    }
    if vote_file.view != view {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block timeout vote view {} does not match view {view}",
                vote_file.view
            ),
        ));
    }
    if vote_file.vote.high_qc_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout vote high_qc_id must be nonempty",
        ));
    }
    if !expected_validators
        .iter()
        .any(|validator| validator == &vote_file.vote.validator)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block timeout vote validator `{}` is not in validator set",
                vote_file.vote.validator
            ),
        ));
    }
    Ok(())
}

pub fn batch_archive(options: BatchArchiveQueryOptions) -> io::Result<Vec<BatchArchiveEntry>> {
    let store = NodeStore::new(options.data_dir);
    let mut batches = store.read_batch_archive()?.batches;
    if let Some(batch_kind) = options.batch_kind {
        batches.retain(|entry| entry.batch_kind == batch_kind);
    }
    if let Some(batch_id) = options.batch_id {
        batches.retain(|entry| entry.batch_id == batch_id);
    }
    let limit = bounded_read_query_limit(options.limit, "batch archive")?;
    if batches.len() > limit {
        batches = batches[batches.len() - limit..].to_vec();
    }
    Ok(batches)
}

fn bounded_read_query_limit(limit: Option<usize>, label: &str) -> io::Result<usize> {
    bounded_read_query_limit_with_max(limit, label, MAX_READ_QUERY_LIMIT)
}

fn bounded_read_query_limit_with_max(
    limit: Option<usize>,
    label: &str,
    max_limit: usize,
) -> io::Result<usize> {
    match limit {
        Some(0) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} query limit must be greater than zero"),
        )),
        Some(limit) if limit > max_limit => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} query limit must not exceed {max_limit}"),
        )),
        Some(limit) => Ok(limit),
        None => Ok(max_limit),
    }
}
