use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use postfiat_types::{
    BatchArchive, BatchArchiveEntry, BlockLog, BlockRecord, BridgeState, ChainTipState, Genesis,
    GovernanceState, LedgerState, MempoolAssetTransactionEntry, MempoolAtomicSwapEntry,
    MempoolEntry, MempoolEscrowTransactionEntry, MempoolFastLanePrimaryEntry,
    MempoolNftTransactionEntry, MempoolOfferTransactionEntry, MempoolPaymentV2Entry, MempoolState,
    NodeState, Receipt, ShieldedState,
};

pub mod fastswap_store;

pub const GENESIS_FILE: &str = "genesis.json";
pub const GOVERNANCE_FILE: &str = "governance.json";
pub const LEDGER_FILE: &str = "ledger.json";
pub const NODE_STATE_FILE: &str = "node_state.json";
pub const CHAIN_TIP_FILE: &str = "chain_tip.json";
pub const BLOCKS_FILE: &str = "blocks.json";
pub const BLOCKS_APPEND_FILE: &str = "blocks.append.jsonl";
pub const BATCH_ARCHIVE_FILE: &str = "batch_archive.json";
pub const BATCH_ARCHIVE_APPEND_FILE: &str = "batch_archive.append.jsonl";
pub const ORDERED_BATCHES_FILE: &str = "ordered_batches.json";
pub const ORDERED_BATCHES_APPEND_FILE: &str = "ordered_batches.append.jsonl";
pub const RECEIPTS_FILE: &str = "receipts.json";
pub const RECEIPTS_APPEND_FILE: &str = "receipts.append.jsonl";
pub const MEMPOOL_FILE: &str = "mempool.json";
pub const SHIELDED_FILE: &str = "shielded.json";
pub const BRIDGE_FILE: &str = "bridge.json";
pub const ORDERED_COMMIT_JOURNAL_FILE: &str = "ordered_commit_journal.json";
const MEMPOOL_MUTATION_LOCK_FILE: &str = ".mempool.mutation.lock";
const ORDERED_COMMIT_MUTATION_LOCK_FILE: &str = ".ordered-commit.mutation.lock";
const ATOMIC_WRITE_TEMP_ATTEMPTS: u32 = 128;
const JSONL_REPAIR_SCAN_CHUNK: usize = 8192;
pub const MAX_STATE_FILE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_JSONL_FILE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_JSONL_RECORD_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_JSONL_RECORDS: usize = 1_000_000;
static ATOMIC_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct NodeStore {
    data_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemCapacity {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

impl NodeStore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn filesystem_capacity(&self) -> io::Result<FilesystemCapacity> {
        filesystem_capacity(&self.data_dir)
    }

    pub fn init(&self, genesis: &Genesis, node_state: &NodeState) -> io::Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        self.write_genesis(genesis)?;
        self.write_node_state(node_state)?;
        self.write_governance(&GovernanceState::new(genesis.validator_count))?;
        self.write_ledger(&LedgerState::empty())?;
        self.write_receipts(&[])?;
        self.write_blocks(&BlockLog::empty())?;
        self.write_batch_archive(&BatchArchive::empty())?;
        self.write_mempool(&MempoolState::empty())?;
        self.write_ordered_batches(&[])?;
        self.write_shielded(&ShieldedState::empty())?;
        self.write_bridge(&BridgeState::empty())
    }

    pub fn write_genesis(&self, genesis: &Genesis) -> io::Result<()> {
        genesis
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        let json = genesis
            .to_json()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        atomic_write(self.data_dir.join(GENESIS_FILE), json)
    }

    pub fn read_genesis(&self) -> io::Result<Genesis> {
        let path = self.data_dir.join(GENESIS_FILE);
        let raw = read_text(&path, "genesis")?;
        Genesis::from_json(&raw).map_err(|error| parse_error(&path, "genesis", error))
    }

    pub fn write_governance(&self, governance: &GovernanceState) -> io::Result<()> {
        write_json(self.data_dir.join(GOVERNANCE_FILE), governance)
    }

    pub fn read_governance(&self) -> io::Result<GovernanceState> {
        read_json(self.data_dir.join(GOVERNANCE_FILE))
    }

    pub fn write_node_state(&self, node_state: &NodeState) -> io::Result<()> {
        let json = node_state
            .to_json()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        atomic_write(self.data_dir.join(NODE_STATE_FILE), json)
    }

    pub fn read_node_state(&self) -> io::Result<NodeState> {
        let path = self.data_dir.join(NODE_STATE_FILE);
        let raw = read_text(&path, "node state")?;
        NodeState::from_json(&raw).map_err(|error| parse_error(&path, "node state", error))
    }

    pub fn write_ledger(&self, ledger: &LedgerState) -> io::Result<()> {
        write_json(self.data_dir.join(LEDGER_FILE), ledger)
    }

    pub fn read_ledger(&self) -> io::Result<LedgerState> {
        read_json(self.data_dir.join(LEDGER_FILE))
    }

    pub fn write_shielded(&self, shielded: &ShieldedState) -> io::Result<()> {
        write_json(self.data_dir.join(SHIELDED_FILE), shielded)
    }

    pub fn read_shielded(&self) -> io::Result<ShieldedState> {
        match read_json(self.data_dir.join(SHIELDED_FILE)) {
            Ok(shielded) => Ok(shielded),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ShieldedState::empty()),
            Err(error) => Err(error),
        }
    }

    pub fn write_bridge(&self, bridge: &BridgeState) -> io::Result<()> {
        write_json(self.data_dir.join(BRIDGE_FILE), bridge)
    }

    pub fn read_bridge(&self) -> io::Result<BridgeState> {
        match read_json(self.data_dir.join(BRIDGE_FILE)) {
            Ok(bridge) => Ok(bridge),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BridgeState::empty()),
            Err(error) => Err(error),
        }
    }

    pub fn write_receipts(&self, receipts: &[Receipt]) -> io::Result<()> {
        write_json(self.data_dir.join(RECEIPTS_FILE), receipts)?;
        remove_optional_file(self.data_dir.join(RECEIPTS_APPEND_FILE))
    }

    pub fn read_receipts(&self) -> io::Result<Vec<Receipt>> {
        let mut receipts: Vec<Receipt> = read_json(self.data_dir.join(RECEIPTS_FILE))?;
        for receipt in
            read_jsonl_records(self.data_dir.join(RECEIPTS_APPEND_FILE), "receipt append")?
        {
            merge_appended_receipt(&mut receipts, receipt)?;
        }
        Ok(receipts)
    }

    pub fn append_receipt(&self, receipt: Receipt) -> io::Result<()> {
        self.append_receipt_record(&receipt)
    }

    pub fn append_receipt_record(&self, receipt: &Receipt) -> io::Result<()> {
        append_jsonl_record(self.data_dir.join(RECEIPTS_APPEND_FILE), receipt)
    }

    pub fn write_mempool(&self, mempool: &MempoolState) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        self.write_mempool_unlocked(mempool)
    }

    fn write_mempool_unlocked(&self, mempool: &MempoolState) -> io::Result<()> {
        write_json(self.data_dir.join(MEMPOOL_FILE), mempool)
    }

    pub fn read_mempool(&self) -> io::Result<MempoolState> {
        match read_json(self.data_dir.join(MEMPOOL_FILE)) {
            Ok(mempool) => Ok(mempool),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(MempoolState::empty()),
            Err(error) => Err(error),
        }
    }

    pub fn append_mempool_entry(&self, entry: MempoolEntry) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        let mut mempool = self.read_mempool()?;
        mempool.pending.push(entry);
        self.write_mempool_unlocked(&mempool)
    }

    pub fn append_mempool_payment_v2_entry(&self, entry: MempoolPaymentV2Entry) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        let mut mempool = self.read_mempool()?;
        mempool.pending_payment_v2.push(entry);
        self.write_mempool_unlocked(&mempool)
    }

    pub fn append_mempool_asset_transaction_entry(
        &self,
        entry: MempoolAssetTransactionEntry,
    ) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        let mut mempool = self.read_mempool()?;
        mempool.pending_asset_transactions.push(entry);
        self.write_mempool_unlocked(&mempool)
    }

    pub fn append_mempool_atomic_swap_entry(
        &self,
        entry: MempoolAtomicSwapEntry,
    ) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        let mut mempool = self.read_mempool()?;
        mempool.pending_atomic_swaps.push(entry);
        self.write_mempool_unlocked(&mempool)
    }

    pub fn append_mempool_fastlane_primary_entry(
        &self,
        entry: MempoolFastLanePrimaryEntry,
    ) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        let mut mempool = self.read_mempool()?;
        mempool.pending_fastlane_primary.push(entry);
        self.write_mempool_unlocked(&mempool)
    }

    pub fn append_mempool_escrow_transaction_entry(
        &self,
        entry: MempoolEscrowTransactionEntry,
    ) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        let mut mempool = self.read_mempool()?;
        mempool.pending_escrow_transactions.push(entry);
        self.write_mempool_unlocked(&mempool)
    }

    pub fn append_mempool_nft_transaction_entry(
        &self,
        entry: MempoolNftTransactionEntry,
    ) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        let mut mempool = self.read_mempool()?;
        mempool.pending_nft_transactions.push(entry);
        self.write_mempool_unlocked(&mempool)
    }

    pub fn append_mempool_offer_transaction_entry(
        &self,
        entry: MempoolOfferTransactionEntry,
    ) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        let mut mempool = self.read_mempool()?;
        mempool.pending_offer_transactions.push(entry);
        self.write_mempool_unlocked(&mempool)
    }

    pub fn write_blocks(&self, blocks: &BlockLog) -> io::Result<()> {
        write_json(self.data_dir.join(BLOCKS_FILE), blocks)?;
        remove_optional_file(self.data_dir.join(BLOCKS_APPEND_FILE))
    }

    pub fn read_blocks(&self) -> io::Result<BlockLog> {
        let mut blocks = match read_json(self.data_dir.join(BLOCKS_FILE)) {
            Ok(blocks) => Ok(blocks),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BlockLog::empty()),
            Err(error) => Err(error),
        }?;
        for block in read_jsonl_records(self.data_dir.join(BLOCKS_APPEND_FILE), "block append")? {
            merge_appended_block(&mut blocks, block)?;
        }
        Ok(blocks)
    }

    pub fn append_block(&self, block: BlockRecord) -> io::Result<()> {
        self.append_block_record(&block)
    }

    pub fn append_block_record(&self, block: &BlockRecord) -> io::Result<()> {
        append_jsonl_record(self.data_dir.join(BLOCKS_APPEND_FILE), block)
    }

    pub fn write_chain_tip(&self, tip: &ChainTipState) -> io::Result<()> {
        write_json(self.data_dir.join(CHAIN_TIP_FILE), tip)
    }

    pub fn read_chain_tip(&self) -> io::Result<ChainTipState> {
        read_json(self.data_dir.join(CHAIN_TIP_FILE))
    }

    pub fn write_batch_archive(&self, archive: &BatchArchive) -> io::Result<()> {
        write_json(self.data_dir.join(BATCH_ARCHIVE_FILE), archive)?;
        remove_optional_file(self.data_dir.join(BATCH_ARCHIVE_APPEND_FILE))
    }

    pub fn read_batch_archive(&self) -> io::Result<BatchArchive> {
        let mut archive = match read_json(self.data_dir.join(BATCH_ARCHIVE_FILE)) {
            Ok(archive) => Ok(archive),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BatchArchive::empty()),
            Err(error) => Err(error),
        }?;
        for entry in read_jsonl_records(
            self.data_dir.join(BATCH_ARCHIVE_APPEND_FILE),
            "batch archive append",
        )? {
            merge_appended_archive_entry(&mut archive, entry)?;
        }
        Ok(archive)
    }

    pub fn append_batch_archive_entry(&self, entry: BatchArchiveEntry) -> io::Result<()> {
        append_jsonl_record(self.data_dir.join(BATCH_ARCHIVE_APPEND_FILE), &entry)
    }

    pub fn write_ordered_batches(&self, batch_ids: &[String]) -> io::Result<()> {
        write_json(self.data_dir.join(ORDERED_BATCHES_FILE), batch_ids)?;
        remove_optional_file(self.data_dir.join(ORDERED_BATCHES_APPEND_FILE))
    }

    pub fn read_ordered_batches(&self) -> io::Result<Vec<String>> {
        let mut batch_ids: Vec<String> = read_json(self.data_dir.join(ORDERED_BATCHES_FILE))?;
        for batch_id in read_jsonl_records(
            self.data_dir.join(ORDERED_BATCHES_APPEND_FILE),
            "ordered batch append",
        )? {
            merge_appended_ordered_batch(&mut batch_ids, batch_id)?;
        }
        Ok(batch_ids)
    }

    pub fn append_ordered_batch(&self, batch_id: String) -> io::Result<()> {
        self.append_ordered_batch_record(&batch_id)
    }

    pub fn append_ordered_batch_record(&self, batch_id: &str) -> io::Result<()> {
        append_jsonl_record(self.data_dir.join(ORDERED_BATCHES_APPEND_FILE), batch_id)
    }

    pub fn write_ordered_commit_journal<T: serde::Serialize + ?Sized>(
        &self,
        journal: &T,
    ) -> io::Result<()> {
        write_json(self.data_dir.join(ORDERED_COMMIT_JOURNAL_FILE), journal)
    }

    pub fn read_ordered_commit_journal<T: serde::de::DeserializeOwned>(
        &self,
    ) -> io::Result<Option<T>> {
        match read_json(self.data_dir.join(ORDERED_COMMIT_JOURNAL_FILE)) {
            Ok(journal) => Ok(Some(journal)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn read_ordered_commit_journal_raw(&self) -> io::Result<Option<String>> {
        let path = self.data_dir.join(ORDERED_COMMIT_JOURNAL_FILE);
        match read_text(&path, "state file") {
            Ok(raw) => Ok(Some(raw)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn remove_ordered_commit_journal(&self) -> io::Result<()> {
        let path = self.data_dir.join(ORDERED_COMMIT_JOURNAL_FILE);
        match fs::remove_file(&path) {
            Ok(()) => sync_parent_dir(&path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub fn lock_ordered_commit(&self) -> io::Result<StorageMutationLock> {
        acquire_mutation_lock(&self.data_dir, ORDERED_COMMIT_MUTATION_LOCK_FILE)
    }
}

fn invalid_data(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(unix)]
fn filesystem_capacity(path: &Path) -> io::Result<FilesystemCapacity> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("storage path `{}` contains a NUL byte", path.display()),
        )
    })?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `path` is a live NUL-terminated CString and `stats` points to
    // writable, correctly sized storage initialized by a successful statvfs.
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: statvfs returned success and therefore initialized `stats`.
    let stats = unsafe { stats.assume_init() };
    let fragment_size = if stats.f_frsize == 0 {
        stats.f_bsize
    } else {
        stats.f_frsize
    };
    let total_bytes = u128::from(stats.f_blocks)
        .checked_mul(u128::from(fragment_size))
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "disk capacity overflow"))?;
    let available_bytes = u128::from(stats.f_bavail)
        .checked_mul(u128::from(fragment_size))
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "disk capacity overflow"))?;
    if available_bytes > total_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "available disk capacity exceeds total capacity",
        ));
    }
    Ok(FilesystemCapacity {
        total_bytes,
        available_bytes,
    })
}

#[cfg(not(unix))]
fn filesystem_capacity(_path: &Path) -> io::Result<FilesystemCapacity> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "filesystem capacity telemetry requires Unix statvfs",
    ))
}

#[derive(Debug)]
pub struct StorageMutationLock {
    _file: File,
}

fn acquire_mutation_lock(data_dir: &Path, file_name: &str) -> io::Result<StorageMutationLock> {
    fs::create_dir_all(data_dir)?;
    let path = data_dir.join(file_name);
    #[cfg(unix)]
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)?;
    #[cfg(not(unix))]
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;

    #[cfg(unix)]
    loop {
        // SAFETY: `file` remains open in `MutationLock` for the full critical
        // section and `flock` only observes its valid borrowed descriptor.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if result == 0 {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(io::Error::new(
                error.kind(),
                format!("failed to lock mutation file `{}`: {error}", path.display()),
            ));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "cross-process storage mutation locking requires Unix flock",
        ));
    }

    Ok(StorageMutationLock { _file: file })
}

fn write_json<T: serde::Serialize + ?Sized>(path: PathBuf, value: &T) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(invalid_data)?;
    enforce_serialized_size("state JSON", json.len() as u64, MAX_STATE_FILE_BYTES)?;
    atomic_write(path, format!("{json}\n"))
}

fn append_jsonl_record<T: serde::Serialize + ?Sized>(path: PathBuf, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(value).map_err(invalid_data)?;
    enforce_serialized_size(
        "JSONL record",
        json.len() as u64,
        MAX_JSONL_RECORD_BYTES as u64,
    )?;
    let existing_len = match fs::metadata(&path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error),
    };
    enforce_serialized_size("JSONL append file", existing_len, MAX_JSONL_FILE_BYTES)?;
    repair_trailing_partial_jsonl(&path)?;
    let existing_len = match fs::metadata(&path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error),
    };
    let new_len = existing_len
        .checked_add(json.len() as u64)
        .and_then(|len| len.checked_add(1))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "JSONL size overflow"))?;
    enforce_serialized_size("JSONL append file", new_len, MAX_JSONL_FILE_BYTES)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(json.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    sync_parent_dir(&path)
}

fn read_json<T: serde::de::DeserializeOwned>(path: PathBuf) -> io::Result<T> {
    let raw = read_text(&path, "state file")?;
    serde_json::from_str(&raw).map_err(|error| parse_error(&path, "state file", error))
}

fn read_jsonl_records<T: serde::de::DeserializeOwned>(
    path: PathBuf,
    label: &str,
) -> io::Result<Vec<T>> {
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    enforce_serialized_size(label, file.metadata()?.len(), MAX_JSONL_FILE_BYTES)?;
    let mut reader = BufReader::new(file);
    let mut records = Vec::new();
    let mut line = Vec::new();
    let mut line_index = 0_usize;
    loop {
        line.clear();
        let read = reader
            .by_ref()
            .take(MAX_JSONL_RECORD_BYTES as u64 + 2)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        line_index = line_index.saturating_add(1);
        if line.len() > MAX_JSONL_RECORD_BYTES.saturating_add(1) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{label} `{}` line {line_index} exceeds {} bytes",
                    path.display(),
                    MAX_JSONL_RECORD_BYTES
                ),
            ));
        }
        if !line.ends_with(b"\n") {
            break;
        }
        while matches!(line.last(), Some(b'\n' | b'\r')) {
            line.pop();
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if records.len() >= MAX_JSONL_RECORDS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{label} `{}` exceeds {} records",
                    path.display(),
                    MAX_JSONL_RECORDS
                ),
            ));
        }
        let record = serde_json::from_slice(&line).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "failed to parse {label} `{}` line {}: {error}",
                    path.display(),
                    line_index
                ),
            )
        })?;
        records.push(record);
    }
    Ok(records)
}

fn merge_appended_block(blocks: &mut BlockLog, block: BlockRecord) -> io::Result<()> {
    if let Some(existing) = blocks
        .blocks
        .iter()
        .find(|existing| existing.header.height == block.header.height)
    {
        if existing == &block {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "appended block height {} conflicts with materialized block",
                block.header.height
            ),
        ));
    }
    if let Some(tip) = blocks.blocks.last() {
        let expected_height =
            tip.header.height.checked_add(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "block height overflow")
            })?;
        if block.header.height != expected_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "appended block height {} does not extend materialized tip {}",
                    block.header.height, tip.header.height
                ),
            ));
        }
    }
    blocks.blocks.push(block);
    Ok(())
}

fn merge_appended_receipt(receipts: &mut Vec<Receipt>, receipt: Receipt) -> io::Result<()> {
    if let Some(existing) = receipts
        .iter()
        .find(|existing| existing.tx_id == receipt.tx_id)
    {
        if existing == &receipt {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "appended receipt `{}` conflicts with materialized receipt",
                receipt.tx_id
            ),
        ));
    }
    receipts.push(receipt);
    Ok(())
}

fn merge_appended_archive_entry(
    archive: &mut BatchArchive,
    entry: BatchArchiveEntry,
) -> io::Result<()> {
    if let Some(existing) = archive.find(&entry.batch_kind, &entry.batch_id) {
        if existing == &entry {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "appended archive entry `{}` conflicts with materialized archive",
                entry.batch_id
            ),
        ));
    }
    archive.batches.push(entry);
    Ok(())
}

fn merge_appended_ordered_batch(batch_ids: &mut Vec<String>, batch_id: String) -> io::Result<()> {
    if batch_ids.last().map(String::as_str) == Some(batch_id.as_str()) {
        return Ok(());
    }
    if batch_ids.iter().any(|existing| existing == &batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("appended ordered batch `{batch_id}` is already present out of order"),
        ));
    }
    batch_ids.push(batch_id);
    Ok(())
}

fn repair_trailing_partial_jsonl(path: &Path) -> io::Result<()> {
    let mut file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(());
    }

    let mut last = [0_u8; 1];
    file.seek(SeekFrom::End(-1))?;
    file.read_exact(&mut last)?;
    if last[0] == b'\n' {
        return Ok(());
    }

    let retain_len = last_jsonl_line_boundary(&mut file, len)?;
    file.set_len(retain_len as u64)?;
    file.sync_all()?;
    sync_parent_dir(path)
}

fn last_jsonl_line_boundary(file: &mut File, len: u64) -> io::Result<u64> {
    let mut end = len;
    let mut buffer = [0_u8; JSONL_REPAIR_SCAN_CHUNK];
    while end > 0 {
        let chunk_len = (end as usize).min(JSONL_REPAIR_SCAN_CHUNK);
        let start = end - chunk_len as u64;
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut buffer[..chunk_len])?;
        if let Some(index) = buffer[..chunk_len].iter().rposition(|byte| *byte == b'\n') {
            return Ok(start + index as u64 + 1);
        }
        end = start;
    }
    Ok(0)
}

fn remove_optional_file(path: PathBuf) -> io::Result<()> {
    match fs::remove_file(&path) {
        Ok(()) => sync_parent_dir(&path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn read_text(path: &Path, label: &str) -> io::Result<String> {
    let file = File::open(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to open {label} `{}`: {error}", path.display()),
        )
    })?;
    enforce_serialized_size(label, file.metadata()?.len(), MAX_STATE_FILE_BYTES)?;
    let mut raw = String::new();
    BufReader::new(file)
        .read_to_string(&mut raw)
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to read {label} `{}`: {error}", path.display()),
            )
        })?;
    Ok(raw)
}

fn enforce_serialized_size(label: &str, actual: u64, limit: u64) -> io::Result<()> {
    if actual <= limit {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{label} is {actual} bytes; limit is {limit} bytes"),
    ))
}

fn parse_error(
    path: &Path,
    label: &str,
    error: impl std::error::Error + Send + Sync + 'static,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("failed to parse {label} `{}`: {error}", path.display()),
    )
}

pub fn atomic_write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    atomic_write_checked(path, contents, |_| Ok(()))
}

pub fn atomic_write_checked(
    path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
    check: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let (tmp_path, file) = create_atomic_temp_file(path)?;
    if let Err(error) = write_synced_file(file, contents) {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    if let Err(error) = check(&tmp_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    if let Err(error) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(error);
    }
    sync_parent_dir(path)?;
    Ok(())
}

fn create_atomic_temp_file(path: &Path) -> io::Result<(PathBuf, File)> {
    let mut last_exists_error = None;
    for attempt in 0..ATOMIC_WRITE_TEMP_ATTEMPTS {
        let tmp_path = temp_write_path(path, attempt);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
        {
            Ok(file) => return Ok((tmp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                last_exists_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_exists_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "failed to create unique atomic write temp file",
        )
    }))
}

fn temp_write_path(path: &Path, attempt: u32) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    let counter = ATOMIC_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    path.with_file_name(format!(
        ".{file_name}.{}.{}.{}.{}.tmp",
        std::process::id(),
        counter,
        nanos,
        attempt
    ))
}

fn write_synced_file(mut file: File, contents: impl AsRef<[u8]>) -> io::Result<()> {
    file.write_all(contents.as_ref())?;
    file.sync_all()
}

fn sync_parent_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::File::open(parent)?.sync_all()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_types::{
        BlockCertificate, BlockHeader, SignedTransfer, UnsignedTransfer, ADDRESS_NAMESPACE,
        TRANSFER_TRANSACTION_KIND,
    };
    use std::sync::{Arc, Barrier};

    #[test]
    fn init_and_read_back() {
        let dir = std::env::temp_dir().join(format!(
            "postfiat-storage-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let store = NodeStore::new(&dir);
        let genesis = Genesis::new("postfiat-local");
        let state = NodeState::initialized("validator-0");

        store.init(&genesis, &state).expect("init store");

        assert_eq!(store.read_genesis().expect("read genesis"), genesis);
        assert_eq!(store.read_node_state().expect("read state"), state);
        assert_eq!(
            store.read_governance().expect("read governance"),
            GovernanceState::new(1)
        );
        assert_eq!(
            store.read_ledger().expect("read ledger"),
            LedgerState::empty()
        );
        assert_eq!(
            store.read_shielded().expect("read shielded"),
            ShieldedState::empty()
        );
        assert_eq!(
            store.read_bridge().expect("read bridge"),
            BridgeState::empty()
        );
        assert_eq!(
            store.read_mempool().expect("read mempool"),
            MempoolState::empty()
        );
        assert_eq!(store.read_blocks().expect("read blocks"), BlockLog::empty());
        assert_eq!(
            store.read_batch_archive().expect("read batch archive"),
            BatchArchive::empty()
        );
        assert_eq!(store.read_receipts().expect("read receipts"), Vec::new());
        assert_eq!(
            store.read_ordered_batches().expect("read ordered batches"),
            Vec::<String>::new()
        );

        let mut invalid_genesis = genesis.clone();
        invalid_genesis.chain_id = " ".to_string();
        assert_eq!(
            store
                .write_genesis(&invalid_genesis)
                .expect_err("invalid genesis write must fail")
                .kind(),
            io::ErrorKind::InvalidInput
        );

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn block_append_log_is_visible_and_full_write_compacts_it() {
        let dir = unique_test_dir("postfiat-storage-block-append-test");
        let store = NodeStore::new(&dir);
        store
            .write_blocks(&BlockLog::empty())
            .expect("write empty blocks");
        let block = sample_block(1, "genesis", "batch-1", "block-1");

        store
            .append_block_record(&block)
            .expect("append block record");

        assert_eq!(
            store.read_blocks().expect("read appended blocks"),
            BlockLog {
                blocks: vec![block.clone()]
            }
        );
        assert!(dir.join(BLOCKS_APPEND_FILE).exists());

        store
            .write_blocks(&BlockLog {
                blocks: vec![block.clone()],
            })
            .expect("compact blocks");

        assert!(!dir.join(BLOCKS_APPEND_FILE).exists());
        assert_eq!(
            store.read_blocks().expect("read compacted blocks"),
            BlockLog {
                blocks: vec![block]
            }
        );

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn batch_archive_append_log_is_visible_and_full_write_compacts_it() {
        let dir = unique_test_dir("postfiat-storage-archive-append-test");
        let store = NodeStore::new(&dir);
        store
            .write_batch_archive(&BatchArchive::empty())
            .expect("write empty archive");
        let entry = sample_archive_entry("batch-1");

        store
            .append_batch_archive_entry(entry.clone())
            .expect("append archive entry");

        assert_eq!(
            store.read_batch_archive().expect("read appended archive"),
            BatchArchive {
                batches: vec![entry.clone()]
            }
        );
        assert!(dir.join(BATCH_ARCHIVE_APPEND_FILE).exists());

        store
            .write_batch_archive(&BatchArchive {
                batches: vec![entry.clone()],
            })
            .expect("compact archive");

        assert!(!dir.join(BATCH_ARCHIVE_APPEND_FILE).exists());
        assert_eq!(
            store.read_batch_archive().expect("read compacted archive"),
            BatchArchive {
                batches: vec![entry]
            }
        );

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn receipt_append_log_is_idempotent_and_conflicts_fail_closed() {
        let dir = unique_test_dir("postfiat-storage-receipt-append-test");
        let store = NodeStore::new(&dir);
        store.write_receipts(&[]).expect("write empty receipts");
        let receipt = sample_receipt("tx-1", "tesSUCCESS");

        store
            .append_receipt_record(&receipt)
            .expect("append receipt");
        store
            .append_receipt_record(&receipt)
            .expect("append duplicate same receipt");

        assert_eq!(
            store.read_receipts().expect("read appended receipts"),
            vec![receipt.clone()]
        );
        assert!(dir.join(RECEIPTS_APPEND_FILE).exists());

        let conflicting = sample_receipt("tx-1", "tecCONFLICT");
        store
            .append_receipt_record(&conflicting)
            .expect("append conflicting receipt record");
        let error = store
            .read_receipts()
            .expect_err("conflicting receipt append must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn ordered_batch_append_log_is_idempotent_and_compacts() {
        let dir = unique_test_dir("postfiat-storage-ordered-batch-append-test");
        let store = NodeStore::new(&dir);
        store
            .write_ordered_batches(&[])
            .expect("write empty ordered batches");

        store
            .append_ordered_batch_record("batch-1")
            .expect("append first ordered batch");
        store
            .append_ordered_batch_record("batch-1")
            .expect("append duplicate same ordered batch");
        store
            .append_ordered_batch_record("batch-2")
            .expect("append second ordered batch");

        assert_eq!(
            store
                .read_ordered_batches()
                .expect("read appended ordered batches"),
            vec!["batch-1".to_string(), "batch-2".to_string()]
        );
        assert!(dir.join(ORDERED_BATCHES_APPEND_FILE).exists());

        store
            .write_ordered_batches(&["batch-1".to_string(), "batch-2".to_string()])
            .expect("compact ordered batches");
        assert!(!dir.join(ORDERED_BATCHES_APPEND_FILE).exists());

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn chain_tip_round_trips() {
        let dir = unique_test_dir("postfiat-storage-chain-tip-test");
        let store = NodeStore::new(&dir);
        let tip = ChainTipState {
            schema: "postfiat-chain-tip-v1".to_string(),
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "genesis-hash".to_string(),
            protocol_version: 1,
            height: 7,
            block_hash: "block-7".to_string(),
            state_root: "state-root-7".to_string(),
            ordered_batch_count: 7,
            receipt_count: 7,
            history_base_height: 0,
        };

        store.write_chain_tip(&tip).expect("write chain tip");
        assert_eq!(store.read_chain_tip().expect("read chain tip"), tip);

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn block_append_repairs_trailing_partial_line_before_appending() {
        let dir = unique_test_dir("postfiat-storage-block-partial-append-test");
        let store = NodeStore::new(&dir);
        store
            .write_blocks(&BlockLog::empty())
            .expect("write empty blocks");
        let first = sample_block(1, "genesis", "batch-1", "block-1");
        let second = sample_block(2, "block-1", "batch-2", "block-2");
        store
            .append_block_record(&first)
            .expect("append first block");
        fs::write(
            dir.join(BLOCKS_APPEND_FILE),
            format!(
                "{}\n{{\"partial\":",
                serde_json::to_string(&first).expect("serialize first block")
            ),
        )
        .expect("write partial append file");

        store
            .append_block_record(&second)
            .expect("append second block after partial");

        assert_eq!(
            store.read_blocks().expect("read repaired blocks"),
            BlockLog {
                blocks: vec![first, second]
            }
        );

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn conflicting_appended_block_fails_closed() {
        let dir = unique_test_dir("postfiat-storage-block-conflict-test");
        let store = NodeStore::new(&dir);
        let original = sample_block(1, "genesis", "batch-1", "block-1");
        let conflicting = sample_block(1, "genesis", "batch-2", "block-1-conflict");
        store
            .write_blocks(&BlockLog {
                blocks: vec![original],
            })
            .expect("write materialized block");
        store
            .append_block_record(&conflicting)
            .expect("append conflicting block");

        let error = store
            .read_blocks()
            .expect_err("conflicting append must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn atomic_write_does_not_use_predictable_pid_temp_path() {
        let dir = std::env::temp_dir().join(format!(
            "postfiat-storage-atomic-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create dir");
        let target = dir.join("state.json");
        let legacy_temp = dir.join(format!(".state.json.{}.tmp", std::process::id()));
        fs::write(&legacy_temp, b"do-not-overwrite").expect("write legacy temp");

        atomic_write(target.clone(), b"{\"ok\":true}\n").expect("atomic write");

        assert_eq!(
            fs::read_to_string(&target).expect("target"),
            "{\"ok\":true}\n"
        );
        assert_eq!(
            fs::read_to_string(&legacy_temp).expect("legacy temp"),
            "do-not-overwrite"
        );

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn oversized_state_file_fails_before_allocation() {
        let dir = unique_test_dir("postfiat-storage-state-size-cap-test");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join(LEDGER_FILE);
        let file = File::create(&path).expect("create sparse state file");
        file.set_len(MAX_STATE_FILE_BYTES + 1)
            .expect("extend sparse state file");

        let error = read_text(&path, "ledger")
            .expect_err("oversized state must fail before reading its contents");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("limit"), "{error}");

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn oversized_jsonl_append_fails_without_mutation() {
        let dir = unique_test_dir("postfiat-storage-jsonl-size-cap-test");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join(RECEIPTS_APPEND_FILE);
        let file = File::create(&path).expect("create sparse append file");
        file.set_len(MAX_JSONL_FILE_BYTES + 1)
            .expect("extend sparse append file");
        let before = fs::metadata(&path).expect("metadata before").len();

        let error = append_jsonl_record(path.clone(), &sample_receipt("tx-size", "tesSUCCESS"))
            .expect_err("oversized append log must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::metadata(&path).expect("metadata after").len(), before);

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn concurrent_mempool_appends_do_not_lose_successful_writes() {
        const WRITERS: usize = 24;
        let dir = unique_test_dir("postfiat-storage-concurrent-mempool-append-test");
        fs::create_dir_all(&dir).expect("create test dir");
        let store = NodeStore::new(&dir);
        store
            .write_mempool(&MempoolState::empty())
            .expect("write empty mempool");
        let barrier = Arc::new(Barrier::new(WRITERS));
        let mut threads = Vec::with_capacity(WRITERS);
        for index in 0..WRITERS {
            let data_dir = dir.clone();
            let barrier = Arc::clone(&barrier);
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                NodeStore::new(data_dir)
                    .append_mempool_entry(sample_mempool_entry(index as u64))
                    .expect("append must report success");
            }));
        }
        for thread in threads {
            thread.join().expect("writer thread");
        }

        let mempool = store.read_mempool().expect("read final mempool");
        assert_eq!(
            mempool.pending.len(),
            WRITERS,
            "every append that reported success must remain durable"
        );

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn ordered_commit_lock_serializes_independent_store_handles() {
        let dir = unique_test_dir("postfiat-storage-ordered-commit-lock-test");
        fs::create_dir_all(&dir).expect("create test dir");
        let first = NodeStore::new(&dir)
            .lock_ordered_commit()
            .expect("first commit lock");
        let (sender, receiver) = std::sync::mpsc::channel();
        let second_dir = dir.clone();
        let thread = std::thread::spawn(move || {
            let lock = NodeStore::new(second_dir)
                .lock_ordered_commit()
                .expect("second commit lock");
            sender.send(()).expect("signal second lock");
            drop(lock);
        });

        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err(),
            "a second process-equivalent store handle must not enter the commit boundary"
        );
        drop(first);
        receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("second lock must proceed after release");
        thread.join().expect("lock thread");

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn filesystem_capacity_reports_checked_available_bytes() {
        let dir = unique_test_dir("postfiat-storage-capacity-test");
        fs::create_dir_all(&dir).expect("create test dir");

        let capacity = NodeStore::new(&dir)
            .filesystem_capacity()
            .expect("filesystem capacity");
        assert!(capacity.total_bytes > 0);
        assert!(capacity.available_bytes <= capacity.total_bytes);

        fs::remove_dir_all(dir).expect("cleanup");
    }

    fn unique_test_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{}-{}",
            prefix,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn sample_archive_entry(batch_id: &str) -> BatchArchiveEntry {
        BatchArchiveEntry {
            batch_kind: "transparent".to_string(),
            batch_id: batch_id.to_string(),
            payload_hash: format!("{batch_id}-payload-hash"),
            payload_json: "{}".to_string(),
        }
    }

    fn sample_receipt(tx_id: &str, code: &str) -> Receipt {
        Receipt {
            tx_id: tx_id.to_string(),
            accepted: code == "tesSUCCESS",
            code: code.to_string(),
            message: code.to_string(),
            fee_charged: 0,
            fee_burned: 0,
            minimum_fee: 0,
            account_reserve: 0,
            state_expansion_fee: 0,
            nft_issuer_transfer_fee: 0,
            nft_issuer_transfer_fee_recipient: None,
            nft_collection_flags: 0,
            offer_id: None,
            offer_fills: Vec::new(),
            atomic_swap_legs: None,
        }
    }

    fn sample_mempool_entry(index: u64) -> MempoolEntry {
        MempoolEntry::new(
            format!("tx-{index}"),
            SignedTransfer {
                unsigned: UnsignedTransfer {
                    chain_id: "postfiat-local".to_string(),
                    genesis_hash: "a".repeat(96),
                    protocol_version: 1,
                    address_namespace: ADDRESS_NAMESPACE.to_string(),
                    transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
                    signature_algorithm_id: "ML-DSA-65".to_string(),
                    from: format!("pfsender{index:032}"),
                    to: format!("pfrecipient{index:029}"),
                    amount: 1,
                    fee: 1,
                    sequence: 1,
                },
                algorithm_id: "ML-DSA-65".to_string(),
                public_key_hex: "00".to_string(),
                signature_hex: "11".to_string(),
            },
        )
    }

    fn sample_block(
        height: u64,
        parent_hash: &str,
        batch_id: &str,
        block_hash: &str,
    ) -> BlockRecord {
        BlockRecord {
            header: BlockHeader {
                height,
                view: 0,
                parent_hash: parent_hash.to_string(),
                proposer: "validator-0".to_string(),
                batch_kind: "transparent".to_string(),
                batch_id: batch_id.to_string(),
                state_root: format!("state-root-{height}"),
                receipt_count: 0,
                certificate_id: format!("certificate-{height}"),
                certificate: BlockCertificate {
                    validators: Vec::new(),
                    quorum: 0,
                    registry_root: String::new(),
                    votes: Vec::new(),
                },
                consensus_v2_commit: None,
                block_hash: block_hash.to_string(),
            },
            receipt_ids: Vec::new(),
            fastpay_pre_state_effects: Vec::new(),
        }
    }

    #[test]
    fn atomic_write_checked_removes_temp_and_preserves_target_on_check_failure() {
        let dir = std::env::temp_dir().join(format!(
            "postfiat-storage-atomic-check-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create dir");
        let target = dir.join("state.json");
        fs::write(&target, b"old\n").expect("write old target");

        let error = atomic_write_checked(&target, b"new\n", |_path| {
            Err(io::Error::new(io::ErrorKind::InvalidData, "check failed"))
        })
        .expect_err("check failure must abort publish");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read_to_string(&target).expect("target"), "old\n");
        let leftovers = fs::read_dir(&dir)
            .expect("read test dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect entries");
        assert_eq!(leftovers.len(), 1);

        fs::remove_dir_all(dir).expect("cleanup");
    }
}
