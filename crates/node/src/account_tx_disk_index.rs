// Disk layout of the `account_tx` history index (schema v4). Included from
// lib.rs next to block_finality.rs, which computes the rows.
//
// <data-dir>/account_tx_index_meta.json      disk index metadata (counts, ranges)
// <data-dir>/account_tx_index.json           aggregate metadata (same fields)
// <data-dir>/account_tx_accounts/<h>/head.json            per-account head
// <data-dir>/account_tx_accounts/<h>/chunk-00000000.json  rows 0..63, ...
//
// `<h>` is the address hash. A chunk holds at most `chunk_rows` rows; every
// chunk but the last is full. A block commit rewrites, per touched account,
// only the last chunk (or starts the next one) and the head, then the two
// fixed-size metadata files. The head is the authority for an account's row
// count: rows a chunk holds past the head's count are left by an interrupted
// append and are ignored and overwritten.

const ACCOUNT_TX_DISK_INDEX_FILE: &str = "account_tx_index_meta.json";
const ACCOUNT_TX_ACCOUNT_SHARDS_DIR: &str = "account_tx_accounts";
const ACCOUNT_TX_ACCOUNT_HEAD_FILE: &str = "head.json";
// v3 stores rows in bounded per-account chunks, and both metadata files hold
// counts and ranges only. v2 rows carried `payment_v2` memo contents and
// shielded-withdrawal credits; v3 rows are the same. An index written with an
// older schema is unusable for reads (the archive scan answers instead) and is
// never extended: the next `account-tx-index-build` rebuilds it from the
// retained blocks.
// v4 rows attribute an `escrow_finish` to the recipient who signed it.
const ACCOUNT_TX_INDEX_SCHEMA: &str = "postfiat-account-tx-index-v4";
const ACCOUNT_TX_DISK_INDEX_SCHEMA: &str = "postfiat-account-tx-disk-index-v4";
const ACCOUNT_TX_ACCOUNT_HEAD_SCHEMA: &str = "postfiat-account-tx-account-head-v4";
const ACCOUNT_TX_ACCOUNT_CHUNK_SCHEMA: &str = "postfiat-account-tx-account-chunk-v4";
/// Rows per account chunk written by `account-tx-index-build`. Commits use the
/// value recorded in the metadata.
pub(crate) const ACCOUNT_TX_CHUNK_ROWS: u64 = 64;
const ACCOUNT_TX_MAX_CHUNK_ROWS: u64 = 4096;

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
    #[serde(default)]
    chunk_rows: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AccountTxAccountHead {
    schema: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    address: String,
    first_height: u64,
    tip_height: u64,
    tip_hash: String,
    row_count: u64,
    chunk_rows: u64,
    chunk_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AccountTxAccountChunk {
    schema: String,
    address: String,
    chunk_index: u64,
    rows: Vec<AccountTxRow>,
}

impl AccountTxDiskIndex {
    fn empty(genesis: &Genesis, chunk_rows: u64) -> Self {
        Self {
            schema: ACCOUNT_TX_DISK_INDEX_SCHEMA.to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            indexed_from_height: None,
            indexed_to_height: None,
            indexed_block_count: 0,
            indexed_row_count: 0,
            account_count: 0,
            tip_hash: "genesis".to_string(),
            shard_dir: ACCOUNT_TX_ACCOUNT_SHARDS_DIR.to_string(),
            chunk_rows,
        }
    }

    fn aggregate(&self) -> AccountTxIndex {
        AccountTxIndex {
            schema: ACCOUNT_TX_INDEX_SCHEMA.to_string(),
            chain_id: self.chain_id.clone(),
            genesis_hash: self.genesis_hash.clone(),
            protocol_version: self.protocol_version,
            indexed_from_height: self.indexed_from_height,
            indexed_to_height: self.indexed_to_height,
            indexed_block_count: self.indexed_block_count,
            indexed_row_count: self.indexed_row_count,
            account_count: self.account_count,
            tip_hash: self.tip_hash.clone(),
            shard_dir: self.shard_dir.clone(),
            chunk_rows: self.chunk_rows,
        }
    }

    fn from_aggregate(index: &AccountTxIndex) -> Self {
        Self {
            schema: ACCOUNT_TX_DISK_INDEX_SCHEMA.to_string(),
            chain_id: index.chain_id.clone(),
            genesis_hash: index.genesis_hash.clone(),
            protocol_version: index.protocol_version,
            indexed_from_height: index.indexed_from_height,
            indexed_to_height: index.indexed_to_height,
            indexed_block_count: index.indexed_block_count,
            indexed_row_count: index.indexed_row_count,
            account_count: index.account_count,
            tip_hash: index.tip_hash.clone(),
            shard_dir: index.shard_dir.clone(),
            chunk_rows: index.chunk_rows,
        }
    }

    /// Records one indexed block: its height and its rows (each row once,
    /// whichever accounts it touches).
    fn record_block(&mut self, height: u64, row_count: usize) {
        self.indexed_from_height.get_or_insert(height);
        self.indexed_to_height = Some(height);
        self.indexed_block_count = self.indexed_block_count.saturating_add(1);
        self.indexed_row_count = self.indexed_row_count.saturating_add(row_count as u64);
    }
}

pub fn rebuild_account_tx_index(
    options: AccountTxIndexOptions,
) -> io::Result<AccountTxIndexBuildReport> {
    let store = NodeStore::new(&options.data_dir);
    let block_log = store.read_blocks()?;
    let archive = store.read_batch_archive()?;
    let receipts = store.read_receipts()?;
    let genesis = store.read_genesis()?;
    let disk_index =
        match catch_up_account_tx_disk_index(&store, &genesis, &block_log, &archive, &receipts)? {
            Some(disk_index) => disk_index,
            None => {
                rebuild_account_tx_disk_index(&store, &genesis, &block_log, &archive, &receipts)?
            }
        };
    let index = disk_index.aggregate();
    let index_path = account_tx_index_path(store.data_dir());
    write_account_tx_index_file(&index_path, &index)?;
    let disk_index_usable =
        account_tx_disk_index_matches_chain(&disk_index, &genesis, &store, &block_log).is_ok();
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
        index_usable: account_tx_index_matches_chain(&index, &genesis, &store, &block_log).is_ok(),
        disk_index_usable,
        disk_account_shard_count: disk_index.account_count,
    })
}

/// Extends a v3 disk index whose tip is on the local chain by the blocks after
/// it, through the same per-block append the commit uses. `None` when there is
/// no such index, so the caller rebuilds.
fn catch_up_account_tx_disk_index(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
    archive: &BatchArchive,
    receipts: &[Receipt],
) -> io::Result<Option<AccountTxDiskIndex>> {
    let meta_path = account_tx_disk_index_path(store.data_dir());
    let mut disk_index = match read_account_tx_disk_index_file(&meta_path) {
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
    if account_tx_disk_index_matches_genesis_and_tip(&disk_index, genesis, &disk_index.tip_hash)
        .is_err()
    {
        return Ok(None);
    }
    let Some(extension_start) = account_tx_index_extension_start(block_log, &disk_index.tip_hash)
    else {
        return Ok(None);
    };
    if extension_start < block_log.blocks.len() {
        let receipt_by_tx = receipt_by_tx_id(receipts, block_log)?;
        let shards_dir = account_tx_account_shards_dir(store.data_dir());
        for block in &block_log.blocks[extension_start..] {
            let rows = account_tx_rows_for_block(block, archive, &receipt_by_tx)?;
            if !append_block_to_account_tx_disk_index(&shards_dir, &mut disk_index, block, &rows)? {
                return Ok(None);
            }
        }
        write_account_tx_disk_index_file(&meta_path, &disk_index)?;
    }
    Ok(Some(disk_index))
}

/// Writes the whole disk index from the retained blocks. The metadata files
/// are removed first, so reads use the archive scan until the new metadata is
/// written last; old account files (any schema) are removed with them.
fn rebuild_account_tx_disk_index(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
    archive: &BatchArchive,
    receipts: &[Receipt],
) -> io::Result<AccountTxDiskIndex> {
    let receipt_by_tx = receipt_by_tx_id(receipts, block_log)?;
    let mut disk_index = AccountTxDiskIndex::empty(genesis, ACCOUNT_TX_CHUNK_ROWS);
    let mut accounts: BTreeMap<String, (Vec<AccountTxRow>, u64, u64, String)> = BTreeMap::new();
    for block in &block_log.blocks {
        if !account_tx_indexes_batch_kind(&block.header.batch_kind) {
            continue;
        }
        let rows = account_tx_rows_for_block(block, archive, &receipt_by_tx)?;
        disk_index.record_block(block.header.height, rows.len());
        for (address, account_rows) in account_tx_rows_by_account(&rows) {
            let entry = accounts
                .entry(address)
                .or_insert_with(|| (Vec::new(), block.header.height, 0, String::new()));
            entry.0.extend(account_rows);
            entry.2 = block.header.height;
            entry.3 = block.header.block_hash.clone();
        }
    }
    disk_index.account_count = accounts.len() as u64;
    disk_index.tip_hash = block_log.tip_hash();

    for path in [
        account_tx_index_path(store.data_dir()),
        account_tx_disk_index_path(store.data_dir()),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let shards_dir = account_tx_account_shards_dir(store.data_dir());
    match std::fs::remove_dir_all(&shards_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    for (address, (rows, first_height, tip_height, tip_hash)) in accounts {
        let head = AccountTxAccountHead {
            schema: ACCOUNT_TX_ACCOUNT_HEAD_SCHEMA.to_string(),
            chain_id: disk_index.chain_id.clone(),
            genesis_hash: disk_index.genesis_hash.clone(),
            protocol_version: disk_index.protocol_version,
            address,
            first_height,
            tip_height,
            tip_hash,
            row_count: 0,
            chunk_rows: disk_index.chunk_rows,
            chunk_count: 0,
        };
        write_account_tx_account_rows(&shards_dir, head, Vec::new(), rows)?;
    }
    write_account_tx_disk_index_file(&account_tx_disk_index_path(store.data_dir()), &disk_index)?;
    Ok(disk_index)
}

/// An index row once per account it touches, in block order.
fn account_tx_rows_by_account(rows: &[AccountTxRow]) -> BTreeMap<String, Vec<AccountTxRow>> {
    let mut accounts: BTreeMap<String, Vec<AccountTxRow>> = BTreeMap::new();
    for row in rows {
        accounts
            .entry(row.from_address.clone())
            .or_default()
            .push(row.clone());
        if row.to_address != row.from_address {
            accounts
                .entry(row.to_address.clone())
                .or_default()
                .push(row.clone());
        }
    }
    accounts
}

/// What the commit-time append did to the disk index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountTxIndexAppend {
    /// No disk index exists yet (`account-tx-index-build` has not run).
    NotIndexed,
    /// The disk index already ends at this block (a replayed commit).
    AlreadyCurrent,
    /// The disk index does not end at this block's parent, so it is left
    /// behind; `account_tx` answers from the archive scan until the build
    /// catches it up.
    Behind,
    /// The block's rows were appended and the index now ends at this block.
    Appended,
}

/// Appends one committed block to the history index: the touched accounts'
/// last chunks and heads, then `account_tx_index_meta.json`, then the
/// aggregate `account_tx_index.json`. Returns what happened to the disk index.
///
/// Ordering rule: the caller runs this only after the block, its archive entry
/// and its receipts are durable (the transactional commit returned, or the
/// ordered commit journal was applied and removed). Each index is extended only
/// from the block's parent and is otherwise left as it is. A crash or error
/// before a metadata write leaves that index at an older tip: reads see a stale
/// tip and use the next source (disk index, aggregate, archive scan), later
/// commits do not extend it, and a replay of the same block or
/// `account-tx-index-build` completes it without adding a row twice.
pub(crate) fn append_account_tx_index_after_commit(
    store: &NodeStore,
    genesis: &Genesis,
    block: &BlockRecord,
    archive_entry: &BatchArchiveEntry,
    receipts: &[Receipt],
) -> io::Result<AccountTxIndexAppend> {
    let data_dir = store.data_dir();
    let meta_path = account_tx_disk_index_path(data_dir);
    if !meta_path.exists() && !account_tx_index_path(data_dir).exists() {
        return Ok(AccountTxIndexAppend::NotIndexed);
    }
    let rows = account_tx_rows_for_committed_block(block, archive_entry, receipts)?;
    let mut disk_index = match read_account_tx_disk_index_file(&meta_path) {
        Ok(index) => index,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(AccountTxIndexAppend::NotIndexed);
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return Ok(AccountTxIndexAppend::Behind);
        }
        Err(error) => return Err(error),
    };
    let disk = if disk_index.tip_hash == block.header.block_hash {
        AccountTxIndexAppend::AlreadyCurrent
    } else if account_tx_disk_index_matches_genesis_and_tip(
        &disk_index,
        genesis,
        &block.header.parent_hash,
    )
    .is_err()
        || !append_block_to_account_tx_disk_index(
            &account_tx_account_shards_dir(data_dir),
            &mut disk_index,
            block,
            &rows,
        )?
    {
        return Ok(AccountTxIndexAppend::Behind);
    } else {
        write_account_tx_disk_index_file(&meta_path, &disk_index)?;
        AccountTxIndexAppend::Appended
    };
    append_account_tx_aggregate_index_after_commit(store, genesis, block, &disk_index)?;
    Ok(disk)
}

/// The `account_tx` rows of one committed block, from the commit's own block,
/// archive entry and receipts (never the full history).
fn account_tx_rows_for_committed_block(
    block: &BlockRecord,
    archive_entry: &BatchArchiveEntry,
    receipts: &[Receipt],
) -> io::Result<Vec<AccountTxRow>> {
    if !account_tx_indexes_batch_kind(&block.header.batch_kind) {
        return Ok(Vec::new());
    }
    if archive_entry.batch_kind != block.header.batch_kind
        || archive_entry.batch_id != block.header.batch_id
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} archive entry does not match its batch",
                block.header.height
            ),
        ));
    }
    // The block's own receipts, under the same repeated-id rule as the build:
    // only governance blocks may repeat an id, and they have no rows.
    let block_log = BlockLog {
        blocks: vec![block.clone()],
    };
    let receipt_by_tx = receipt_by_tx_id(receipts, &block_log)?;
    account_tx_rows_for_archived_block(block, archive_entry, &receipt_by_tx)
}

/// Moves the aggregate metadata to the disk index's tip when the disk index
/// ends at this block and the aggregate at its parent (or already here).
fn append_account_tx_aggregate_index_after_commit(
    store: &NodeStore,
    genesis: &Genesis,
    block: &BlockRecord,
    disk_index: &AccountTxDiskIndex,
) -> io::Result<()> {
    let index_path = account_tx_index_path(store.data_dir());
    let index = match read_account_tx_index_file(&index_path) {
        Ok(index) => index,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::InvalidData
            ) =>
        {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if disk_index.tip_hash != block.header.block_hash
        || account_tx_index_matches_domain(&index, genesis).is_err()
        || index.shard_dir != disk_index.shard_dir
        || index.chunk_rows != disk_index.chunk_rows
    {
        return Ok(());
    }
    let next = disk_index.aggregate();
    if index == next || index.tip_hash != block.header.parent_hash {
        return Ok(());
    }
    write_account_tx_index_file(&index_path, &next)
}

/// Adds one block to the disk index in memory and its rows to the account
/// files on disk. Returns `false` when an account's files do not match the
/// index, which leaves the index behind for the build to repair.
fn append_block_to_account_tx_disk_index(
    shards_dir: &Path,
    disk_index: &mut AccountTxDiskIndex,
    block: &BlockRecord,
    rows: &[AccountTxRow],
) -> io::Result<bool> {
    if account_tx_indexes_batch_kind(&block.header.batch_kind) {
        let height = block.header.height;
        for (address, account_rows) in account_tx_rows_by_account(rows) {
            let head_path = shards_dir
                .join(account_tx_account_dir_name(&address))
                .join(ACCOUNT_TX_ACCOUNT_HEAD_FILE);
            let head = match read_account_tx_account_head_file(&head_path) {
                Ok(head) => Some(head),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) if error.kind() == io::ErrorKind::InvalidData => return Ok(false),
                Err(error) => return Err(error),
            };
            match head {
                Some(head) => {
                    if account_tx_account_head_matches(&head, disk_index, &address).is_err() {
                        return Ok(false);
                    }
                    if head.tip_height >= height {
                        // A replay: this block's rows are already in the files.
                        if head.first_height == height {
                            disk_index.account_count = disk_index.account_count.saturating_add(1);
                        }
                        continue;
                    }
                    let Some(last) = read_account_tx_last_chunk_rows(shards_dir, &head)? else {
                        return Ok(false);
                    };
                    let mut head = head;
                    head.tip_height = height;
                    head.tip_hash = block.header.block_hash.clone();
                    write_account_tx_account_rows(shards_dir, head, last, account_rows)?;
                }
                None => {
                    let head = AccountTxAccountHead {
                        schema: ACCOUNT_TX_ACCOUNT_HEAD_SCHEMA.to_string(),
                        chain_id: disk_index.chain_id.clone(),
                        genesis_hash: disk_index.genesis_hash.clone(),
                        protocol_version: disk_index.protocol_version,
                        address,
                        first_height: height,
                        tip_height: height,
                        tip_hash: block.header.block_hash.clone(),
                        row_count: 0,
                        chunk_rows: disk_index.chunk_rows,
                        chunk_count: 0,
                    };
                    write_account_tx_account_rows(shards_dir, head, Vec::new(), account_rows)?;
                    disk_index.account_count = disk_index.account_count.saturating_add(1);
                }
            }
        }
        disk_index.record_block(height, rows.len());
    }
    disk_index.tip_hash = block.header.block_hash.clone();
    Ok(true)
}

/// The rows of the account's last chunk when it is not full, or no rows when
/// the next row starts a new chunk. `None` when the chunk does not match.
fn read_account_tx_last_chunk_rows(
    shards_dir: &Path,
    head: &AccountTxAccountHead,
) -> io::Result<Option<Vec<AccountTxRow>>> {
    let in_last = account_tx_last_chunk_len(head);
    if in_last == 0 || in_last == head.chunk_rows {
        return Ok(Some(Vec::new()));
    }
    let chunk_index = head.chunk_count - 1;
    let chunk = match read_account_tx_account_chunk_file(&account_tx_account_chunk_path(
        shards_dir,
        &head.address,
        chunk_index,
    )) {
        Ok(chunk) => chunk,
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
    Ok(account_tx_chunk_rows(
        &chunk,
        &head.address,
        chunk_index,
        in_last,
    ))
}

/// Rows in the account's last chunk (0 when the account has none).
fn account_tx_last_chunk_len(head: &AccountTxAccountHead) -> u64 {
    if head.chunk_count == 0 {
        return 0;
    }
    head.row_count - (head.chunk_count - 1) * head.chunk_rows
}

/// The first `expected` rows of a chunk that matches the account and index.
fn account_tx_chunk_rows(
    chunk: &AccountTxAccountChunk,
    address: &str,
    chunk_index: u64,
    expected: u64,
) -> Option<Vec<AccountTxRow>> {
    if chunk.schema != ACCOUNT_TX_ACCOUNT_CHUNK_SCHEMA
        || chunk.address != address
        || chunk.chunk_index != chunk_index
        || (chunk.rows.len() as u64) < expected
    {
        return None;
    }
    Some(chunk.rows[..expected as usize].to_vec())
}

/// Appends `new_rows` after `last` (the rows already in the account's last,
/// unfilled chunk) and writes the chunks they fill, then the head.
fn write_account_tx_account_rows(
    shards_dir: &Path,
    mut head: AccountTxAccountHead,
    last: Vec<AccountTxRow>,
    new_rows: Vec<AccountTxRow>,
) -> io::Result<()> {
    let account_dir = shards_dir.join(account_tx_account_dir_name(&head.address));
    std::fs::create_dir_all(&account_dir)?;
    let full_chunks = head.row_count / head.chunk_rows;
    let mut chunk_index = full_chunks;
    let mut rows = last;
    head.row_count = full_chunks * head.chunk_rows + rows.len() as u64;
    for row in new_rows {
        rows.push(row);
        head.row_count += 1;
        if rows.len() as u64 == head.chunk_rows {
            write_account_tx_account_chunk(shards_dir, &head.address, chunk_index, rows)?;
            chunk_index += 1;
            rows = Vec::new();
        }
    }
    if !rows.is_empty() {
        write_account_tx_account_chunk(shards_dir, &head.address, chunk_index, rows)?;
    }
    head.chunk_count = head.row_count.div_ceil(head.chunk_rows);
    write_account_tx_account_head_file(&account_dir.join(ACCOUNT_TX_ACCOUNT_HEAD_FILE), &head)
}

fn write_account_tx_account_chunk(
    shards_dir: &Path,
    address: &str,
    chunk_index: u64,
    rows: Vec<AccountTxRow>,
) -> io::Result<()> {
    let chunk = AccountTxAccountChunk {
        schema: ACCOUNT_TX_ACCOUNT_CHUNK_SCHEMA.to_string(),
        address: address.to_string(),
        chunk_index,
        rows,
    };
    let json = serde_json::to_string(&chunk).map_err(invalid_data)?;
    atomic_write(
        account_tx_account_chunk_path(shards_dir, address, chunk_index),
        format!("{json}\n"),
    )
}

/// The rows of `address` from a usable index: the disk metadata when it is
/// current, otherwise the aggregate metadata when it is. An account the index
/// does not list has no rows. `None` sends the read to the archive scan.
fn read_usable_account_tx_disk_rows(
    store: &NodeStore,
    genesis: &Genesis,
    block_log: &BlockLog,
    address: &str,
) -> Option<Vec<AccountTxRow>> {
    let disk_index = read_account_tx_disk_index_file(&account_tx_disk_index_path(store.data_dir()))
        .ok()
        .filter(|index| {
            account_tx_disk_index_matches_chain(index, genesis, store, block_log).is_ok()
        })
        .or_else(|| {
            read_usable_account_tx_index(store, genesis, block_log)
                .map(|index| AccountTxDiskIndex::from_aggregate(&index))
                .filter(|index| {
                    account_tx_disk_index_matches_chain(index, genesis, store, block_log).is_ok()
                })
        })?;
    read_account_tx_account_rows(
        &account_tx_account_shards_dir(store.data_dir()),
        &disk_index,
        address,
    )
}

fn read_account_tx_account_rows(
    shards_dir: &Path,
    disk_index: &AccountTxDiskIndex,
    address: &str,
) -> Option<Vec<AccountTxRow>> {
    let head_path = shards_dir
        .join(account_tx_account_dir_name(address))
        .join(ACCOUNT_TX_ACCOUNT_HEAD_FILE);
    let head = match read_account_tx_account_head_file(&head_path) {
        Ok(head) => head,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Some(Vec::new()),
        Err(_) => return None,
    };
    account_tx_account_head_matches(&head, disk_index, address).ok()?;
    if disk_index
        .indexed_to_height
        .is_none_or(|indexed_to| head.tip_height > indexed_to)
    {
        return None;
    }
    let mut rows = Vec::with_capacity(head.row_count as usize);
    for chunk_index in 0..head.chunk_count {
        let expected = if chunk_index + 1 == head.chunk_count {
            account_tx_last_chunk_len(&head)
        } else {
            head.chunk_rows
        };
        let chunk = read_account_tx_account_chunk_file(&account_tx_account_chunk_path(
            shards_dir,
            address,
            chunk_index,
        ))
        .ok()?;
        rows.extend(account_tx_chunk_rows(
            &chunk,
            address,
            chunk_index,
            expected,
        )?);
    }
    Some(rows)
}

fn account_tx_account_head_matches(
    head: &AccountTxAccountHead,
    disk_index: &AccountTxDiskIndex,
    address: &str,
) -> Result<(), String> {
    if head.schema != ACCOUNT_TX_ACCOUNT_HEAD_SCHEMA {
        return Err(format!(
            "unsupported account_tx account head schema `{}`",
            head.schema
        ));
    }
    if head.chain_id != disk_index.chain_id
        || head.genesis_hash != disk_index.genesis_hash
        || head.protocol_version != disk_index.protocol_version
    {
        return Err("account_tx account head domain does not match disk index".to_string());
    }
    if head.address != address {
        return Err("account_tx account head address does not match".to_string());
    }
    if head.chunk_rows != disk_index.chunk_rows
        || head.row_count == 0
        || head.chunk_count != head.row_count.div_ceil(head.chunk_rows)
        || head.first_height > head.tip_height
    {
        return Err("account_tx account head row or chunk count is inconsistent".to_string());
    }
    Ok(())
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

fn account_tx_index_matches_domain(
    index: &AccountTxIndex,
    genesis: &Genesis,
) -> Result<(), String> {
    if index.schema != ACCOUNT_TX_INDEX_SCHEMA {
        return Err(format!(
            "unsupported account_tx index schema `{}`",
            index.schema
        ));
    }
    if index.chain_id != genesis.chain_id {
        return Err("account_tx index chain_id does not match local genesis".to_string());
    }
    if index.genesis_hash != genesis_hash(genesis) {
        return Err("account_tx index genesis_hash does not match local genesis".to_string());
    }
    if index.protocol_version != genesis.protocol_version {
        return Err("account_tx index protocol_version does not match local genesis".to_string());
    }
    if index.shard_dir != ACCOUNT_TX_ACCOUNT_SHARDS_DIR
        || !(1..=ACCOUNT_TX_MAX_CHUNK_ROWS).contains(&index.chunk_rows)
    {
        return Err("account_tx index shard layout is unsupported".to_string());
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
    if index.schema != ACCOUNT_TX_DISK_INDEX_SCHEMA {
        return Err(format!(
            "unsupported account_tx disk index schema `{}`",
            index.schema
        ));
    }
    if index.chain_id != genesis.chain_id {
        return Err("account_tx disk index chain_id does not match local genesis".to_string());
    }
    if index.genesis_hash != genesis_hash(genesis) {
        return Err("account_tx disk index genesis_hash does not match local genesis".to_string());
    }
    if index.protocol_version != genesis.protocol_version {
        return Err(
            "account_tx disk index protocol_version does not match local genesis".to_string(),
        );
    }
    if index.shard_dir != ACCOUNT_TX_ACCOUNT_SHARDS_DIR
        || !(1..=ACCOUNT_TX_MAX_CHUNK_ROWS).contains(&index.chunk_rows)
    {
        return Err("account_tx disk index shard layout is unsupported".to_string());
    }
    if index.tip_hash != current_tip_hash {
        return Err("account_tx disk index tip hash is stale".to_string());
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

fn account_tx_account_dir_name(address: &str) -> String {
    hash_hex("postfiat.account_tx.address_shard.v1", address.as_bytes())
}

fn account_tx_account_chunk_path(shards_dir: &Path, address: &str, chunk_index: u64) -> PathBuf {
    shards_dir
        .join(account_tx_account_dir_name(address))
        .join(format!("chunk-{chunk_index:08}.json"))
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

fn read_account_tx_account_head_file(path: &Path) -> io::Result<AccountTxAccountHead> {
    read_json_file(path, "account_tx account head")
}

fn write_account_tx_account_head_file(path: &Path, head: &AccountTxAccountHead) -> io::Result<()> {
    let json = serde_json::to_string_pretty(head).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

fn read_account_tx_account_chunk_file(path: &Path) -> io::Result<AccountTxAccountChunk> {
    read_json_file(path, "account_tx account chunk")
}
