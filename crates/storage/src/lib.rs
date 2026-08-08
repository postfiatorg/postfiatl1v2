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
use serde::{Deserialize, Serialize};

pub mod fastswap_store;
pub mod integrity;

use integrity::{
    from_hex, macs_equal, to_hex, IntegrityKey, FILE_MAC_MARKER, JSONL_CHAIN_GENESIS,
    JSONL_ENVELOPE_KIND, MAC_BYTES,
};

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
    integrity_key: IntegrityKey,
    allow_legacy_migration: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemCapacity {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

impl NodeStore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        let integrity_key = IntegrityKey::load_or_create(&data_dir)
            .unwrap_or_else(|error| panic!("failed to load storage integrity key: {error}"));
        Self {
            data_dir,
            integrity_key,
            allow_legacy_migration: false,
        }
    }

    /// Fallible constructor: identical to [`NodeStore::new`] but returns an
    /// error instead of panicking when the node-local integrity key cannot be
    /// loaded (permissions, entropy failure, ...).
    pub fn try_new(data_dir: impl Into<PathBuf>) -> io::Result<Self> {
        let data_dir = data_dir.into();
        let integrity_key = IntegrityKey::load_or_create(&data_dir)?;
        Ok(Self {
            data_dir,
            integrity_key,
            allow_legacy_migration: false,
        })
    }

    /// Open an offline, operator-verified legacy directory for one-shot
    /// migration. Normal constructors never accept unkeyed state.
    pub fn try_new_for_legacy_migration(data_dir: impl Into<PathBuf>) -> io::Result<Self> {
        let data_dir = data_dir.into();
        let integrity_key = IntegrityKey::load_or_create(&data_dir)?;
        Ok(Self {
            data_dir,
            integrity_key,
            allow_legacy_migration: true,
        })
    }

    /// Verify and durably rewrite every storage-owned legacy file under the
    /// node-local integrity key. The operation is restartable: already keyed
    /// files are verified, while remaining legacy files are migrated.
    ///
    /// Callers must ensure the node is offline and the legacy directory was
    /// obtained from a trusted source before using this migration path.
    pub fn migrate_legacy_state(&self) -> io::Result<()> {
        if !self.allow_legacy_migration {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "legacy storage migration requires the explicit migration constructor",
            ));
        }
        self.read_genesis()?;
        self.read_node_state()?;
        self.read_governance()?;
        self.read_ledger()?;
        self.read_receipts()?;
        self.read_blocks()?;
        self.read_batch_archive()?;
        self.read_ordered_batches()?;
        self.read_mempool()?;
        self.read_shielded()?;
        self.read_bridge()?;
        match self.read_chain_tip() {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        self.read_ordered_commit_journal_raw()?;
        Ok(())
    }

    /// Open with an integrity key anchored at an operator-protected path
    /// outside the state directory.
    pub fn try_new_with_integrity_key(
        data_dir: impl Into<PathBuf>,
        key_path: impl AsRef<Path>,
    ) -> io::Result<Self> {
        Ok(Self {
            data_dir: data_dir.into(),
            integrity_key: IntegrityKey::load_or_create_at(key_path.as_ref())?,
            allow_legacy_migration: false,
        })
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
        let bytes = self
            .write_json_with_mac("genesis", json.as_bytes())
            .map_err(invalid_data)?;
        atomic_write(self.data_dir.join(GENESIS_FILE), bytes)
    }

    pub fn read_genesis(&self) -> io::Result<Genesis> {
        let raw = self.read_body_with_mac(&self.data_dir.join(GENESIS_FILE), "genesis")?;
        Genesis::from_json(&raw)
            .map_err(|error| parse_error(&self.data_dir.join(GENESIS_FILE), "genesis", error))
    }

    pub fn write_governance(&self, governance: &GovernanceState) -> io::Result<()> {
        self.write_json(self.data_dir.join(GOVERNANCE_FILE), governance)
    }

    pub fn read_governance(&self) -> io::Result<GovernanceState> {
        self.read_json(self.data_dir.join(GOVERNANCE_FILE))
    }

    pub fn write_node_state(&self, node_state: &NodeState) -> io::Result<()> {
        let json = node_state
            .to_json()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        let bytes = self
            .write_json_with_mac("node state", json.as_bytes())
            .map_err(invalid_data)?;
        atomic_write(self.data_dir.join(NODE_STATE_FILE), bytes)
    }

    pub fn read_node_state(&self) -> io::Result<NodeState> {
        let raw = self.read_body_with_mac(&self.data_dir.join(NODE_STATE_FILE), "node state")?;
        NodeState::from_json(&raw)
            .map_err(|error| parse_error(&self.data_dir.join(NODE_STATE_FILE), "node state", error))
    }

    pub fn write_ledger(&self, ledger: &LedgerState) -> io::Result<()> {
        self.write_json(self.data_dir.join(LEDGER_FILE), ledger)
    }

    pub fn read_ledger(&self) -> io::Result<LedgerState> {
        self.read_json(self.data_dir.join(LEDGER_FILE))
    }

    pub fn write_shielded(&self, shielded: &ShieldedState) -> io::Result<()> {
        self.write_json(self.data_dir.join(SHIELDED_FILE), shielded)
    }

    pub fn read_shielded(&self) -> io::Result<ShieldedState> {
        match self.read_json(self.data_dir.join(SHIELDED_FILE)) {
            Ok(shielded) => Ok(shielded),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ShieldedState::empty()),
            Err(error) => Err(error),
        }
    }

    pub fn write_bridge(&self, bridge: &BridgeState) -> io::Result<()> {
        self.write_json(self.data_dir.join(BRIDGE_FILE), bridge)
    }

    pub fn read_bridge(&self) -> io::Result<BridgeState> {
        match self.read_json(self.data_dir.join(BRIDGE_FILE)) {
            Ok(bridge) => Ok(bridge),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BridgeState::empty()),
            Err(error) => Err(error),
        }
    }

    pub fn write_receipts(&self, receipts: &[Receipt]) -> io::Result<()> {
        self.write_json(self.data_dir.join(RECEIPTS_FILE), receipts)?;
        self.remove_jsonl_log(&self.data_dir.join(RECEIPTS_APPEND_FILE))
    }

    pub fn read_receipts(&self) -> io::Result<Vec<Receipt>> {
        let mut receipts: Vec<Receipt> = self.read_json(self.data_dir.join(RECEIPTS_FILE))?;
        for receipt in
            self.read_jsonl_records(&self.data_dir.join(RECEIPTS_APPEND_FILE), "receipt append")?
        {
            merge_appended_receipt(&mut receipts, receipt)?;
        }
        Ok(receipts)
    }

    pub fn append_receipt(&self, receipt: Receipt) -> io::Result<()> {
        self.append_receipt_record(&receipt)
    }

    pub fn append_receipt_record(&self, receipt: &Receipt) -> io::Result<()> {
        self.append_jsonl_record(self.data_dir.join(RECEIPTS_APPEND_FILE), receipt)
    }

    pub fn write_mempool(&self, mempool: &MempoolState) -> io::Result<()> {
        let _lock = acquire_mutation_lock(&self.data_dir, MEMPOOL_MUTATION_LOCK_FILE)?;
        self.write_mempool_unlocked(mempool)
    }

    fn write_mempool_unlocked(&self, mempool: &MempoolState) -> io::Result<()> {
        self.write_json(self.data_dir.join(MEMPOOL_FILE), mempool)
    }

    pub fn read_mempool(&self) -> io::Result<MempoolState> {
        match self.read_json(self.data_dir.join(MEMPOOL_FILE)) {
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
        self.write_json(self.data_dir.join(BLOCKS_FILE), blocks)?;
        self.remove_jsonl_log(&self.data_dir.join(BLOCKS_APPEND_FILE))
    }

    pub fn read_blocks(&self) -> io::Result<BlockLog> {
        let mut blocks = match self.read_json(self.data_dir.join(BLOCKS_FILE)) {
            Ok(blocks) => Ok(blocks),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BlockLog::empty()),
            Err(error) => Err(error),
        }?;
        for block in
            self.read_jsonl_records(&self.data_dir.join(BLOCKS_APPEND_FILE), "block append")?
        {
            merge_appended_block(&mut blocks, block)?;
        }
        Ok(blocks)
    }

    pub fn append_block(&self, block: BlockRecord) -> io::Result<()> {
        self.append_block_record(&block)
    }

    pub fn append_block_record(&self, block: &BlockRecord) -> io::Result<()> {
        self.append_jsonl_record(self.data_dir.join(BLOCKS_APPEND_FILE), block)
    }

    pub fn write_chain_tip(&self, tip: &ChainTipState) -> io::Result<()> {
        self.write_json(self.data_dir.join(CHAIN_TIP_FILE), tip)
    }

    pub fn read_chain_tip(&self) -> io::Result<ChainTipState> {
        self.read_json(self.data_dir.join(CHAIN_TIP_FILE))
    }

    pub fn write_batch_archive(&self, archive: &BatchArchive) -> io::Result<()> {
        self.write_json(self.data_dir.join(BATCH_ARCHIVE_FILE), archive)?;
        self.remove_jsonl_log(&self.data_dir.join(BATCH_ARCHIVE_APPEND_FILE))
    }

    pub fn read_batch_archive(&self) -> io::Result<BatchArchive> {
        let mut archive = match self.read_json(self.data_dir.join(BATCH_ARCHIVE_FILE)) {
            Ok(archive) => Ok(archive),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BatchArchive::empty()),
            Err(error) => Err(error),
        }?;
        for entry in self.read_jsonl_records(
            &self.data_dir.join(BATCH_ARCHIVE_APPEND_FILE),
            "batch archive append",
        )? {
            merge_appended_archive_entry(&mut archive, entry)?;
        }
        Ok(archive)
    }

    pub fn append_batch_archive_entry(&self, entry: BatchArchiveEntry) -> io::Result<()> {
        self.append_jsonl_record(self.data_dir.join(BATCH_ARCHIVE_APPEND_FILE), &entry)
    }

    pub fn write_ordered_batches(&self, batch_ids: &[String]) -> io::Result<()> {
        self.write_json(self.data_dir.join(ORDERED_BATCHES_FILE), batch_ids)?;
        self.remove_jsonl_log(&self.data_dir.join(ORDERED_BATCHES_APPEND_FILE))
    }

    pub fn read_ordered_batches(&self) -> io::Result<Vec<String>> {
        let mut batch_ids: Vec<String> =
            self.read_json(self.data_dir.join(ORDERED_BATCHES_FILE))?;
        for batch_id in self.read_jsonl_records(
            &self.data_dir.join(ORDERED_BATCHES_APPEND_FILE),
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
        self.append_jsonl_record(self.data_dir.join(ORDERED_BATCHES_APPEND_FILE), batch_id)
    }

    pub fn write_ordered_commit_journal<T: serde::Serialize + ?Sized>(
        &self,
        journal: &T,
    ) -> io::Result<()> {
        self.write_json(self.data_dir.join(ORDERED_COMMIT_JOURNAL_FILE), journal)
    }

    pub fn read_ordered_commit_journal<T: serde::de::DeserializeOwned>(
        &self,
    ) -> io::Result<Option<T>> {
        match self.read_json(self.data_dir.join(ORDERED_COMMIT_JOURNAL_FILE)) {
            Ok(journal) => Ok(Some(journal)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn read_ordered_commit_journal_raw(&self) -> io::Result<Option<String>> {
        let path = self.data_dir.join(ORDERED_COMMIT_JOURNAL_FILE);
        match self.read_body_with_mac(&path, "state file") {
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

    fn file_mac(&self, label: &str, body: &[u8]) -> [u8; MAC_BYTES] {
        self.integrity_key
            .mac(file_mac_domain(label).as_bytes(), body)
    }

    fn jsonl_mac(&self, payload: &[u8]) -> [u8; MAC_BYTES] {
        // Single fixed domain: the chain input (previous MAC || 0x00 ||
        // canonical payload) already binds every record to its position and
        // contents, so a per-label domain is unnecessary — and keeping the
        // label out of the MAC removes any write/read label drift.
        self.integrity_key
            .mac(b"postfiat.storage.jsonl-record.v1", payload)
    }

    /// Serialize `value` and append the keyed integrity trailer; see the
    /// format note in [`integrity`].
    fn write_json_with_mac(&self, label: &str, body: &[u8]) -> io::Result<Vec<u8>> {
        enforce_serialized_size("state JSON", body.len() as u64, MAX_STATE_FILE_BYTES)?;
        // Canonical form: exactly one newline between body and trailer, and
        // the MAC always covers the body *without* that trailing newline so
        // the verify side can be unambiguous.
        let body = strip_trailing_newlines(body);
        let mac = self.file_mac(label, body);
        let mut bytes = Vec::with_capacity(body.len() + 1 + FILE_MAC_MARKER.len() + 96 + 1);
        bytes.extend_from_slice(body);
        bytes.push(b'\n');
        bytes.extend_from_slice(FILE_MAC_MARKER.as_bytes());
        bytes.extend_from_slice(to_hex(&mac).as_bytes());
        bytes.push(b'\n');
        enforce_serialized_size("state file", bytes.len() as u64, MAX_STATE_FILE_BYTES)?;
        Ok(bytes)
    }

    /// Read a whole-file state body, verifying the keyed integrity trailer.
    /// Legacy untagged files are accepted once, logged loudly, and re-written
    /// with the keyed trailer (migration window — see [`integrity`]).
    fn read_body_with_mac(&self, path: &Path, label: &str) -> io::Result<String> {
        let raw = read_text(path, label)?;
        match split_mac_trailer(&raw) {
            Some((body, tag_hex)) => {
                let tag = from_hex(tag_hex).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("integrity tag in `{}` is not valid hex", path.display()),
                    )
                })?;
                if !macs_equal(&self.file_mac(label, body.as_bytes()), &tag) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "integrity MAC mismatch in {label} `{}`; refusing to load possibly \
                             tampered state",
                            path.display()
                        ),
                    ));
                }
                Ok(body.to_owned())
            }
            None => {
                if !self.allow_legacy_migration {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "{label} `{}` is legacy untagged state; use the explicit offline migration constructor",
                            path.display()
                        ),
                    ));
                }
                eprintln!(
                    "warning: {label} `{}` has no integrity tag (legacy format); \
                     upgrading it to the keyed format",
                    path.display()
                );
                let bytes = self.write_json_with_mac(label, raw.as_bytes())?;
                atomic_write(path, bytes)?;
                Ok(raw)
            }
        }
    }

    fn write_json<T: serde::Serialize + ?Sized>(&self, path: PathBuf, value: &T) -> io::Result<()> {
        let json = serde_json::to_string_pretty(value).map_err(invalid_data)?;
        let bytes = self.write_json_with_mac("state file", json.as_bytes())?;
        atomic_write(path, bytes)
    }

    fn read_json<T: serde::de::DeserializeOwned>(&self, path: PathBuf) -> io::Result<T> {
        let raw = self.read_body_with_mac(&path, "state file")?;
        serde_json::from_str(&raw).map_err(|error| parse_error(&path, "state file", error))
    }

    /// Append one record as a keyed, hash-chained JSONL envelope and fsync
    /// the file (plus its parent directory on first create). See [`integrity`]
    /// for the envelope format.
    fn append_jsonl_record<T: serde::Serialize + ?Sized>(
        &self,
        path: PathBuf,
        value: &T,
    ) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = serde_json::to_vec(value).map_err(invalid_data)?;
        enforce_serialized_size(
            "JSONL record",
            payload.len() as u64,
            MAX_JSONL_RECORD_BYTES as u64,
        )?;
        let existing_len = match fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error),
        };
        // Fail closed on oversized logs before the repair pass can truncate
        // them (mirrors the pre-integrity behaviour).
        enforce_serialized_size("JSONL append file", existing_len, MAX_JSONL_FILE_BYTES)?;
        repair_trailing_partial_jsonl(&path)?;
        let (existing_len, chain, record_count) = self.read_jsonl_tail(&path)?;
        enforce_serialized_size("JSONL append file", existing_len, MAX_JSONL_FILE_BYTES)?;
        let envelope = self.jsonl_envelope(&chain, payload);
        let envelope_json = serde_json::to_string(&envelope).map_err(invalid_data)?;
        let new_len = existing_len
            .checked_add(envelope_json.len() as u64)
            .and_then(|len| len.checked_add(1))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "JSONL size overflow"))?;
        enforce_serialized_size("JSONL append file", new_len, MAX_JSONL_FILE_BYTES)?;
        let existed = existing_len > 0;
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(envelope_json.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        if !existed {
            // Persist the newly created directory entry, not just the data.
            sync_parent_dir(&path)?;
        }
        self.write_jsonl_head(&path, record_count.saturating_add(1), &envelope.mac)?;
        Ok(())
    }

    fn jsonl_envelope(&self, chain: &str, payload: Vec<u8>) -> JsonlEnvelope {
        // The MAC covers the canonical payload (round-tripped through
        // `serde_json::Value`) so verification does not depend on the struct
        // field order of whatever type was appended.
        let record: serde_json::Value =
            serde_json::from_slice(&payload).expect("payload is valid JSON");
        let canonical = serde_json::to_vec(&record).expect("value is valid JSON");
        let mut mac_input = Vec::with_capacity(chain.len() + 1 + canonical.len());
        mac_input.extend_from_slice(chain.as_bytes());
        mac_input.push(0);
        mac_input.extend_from_slice(&canonical);
        JsonlEnvelope {
            pftmac: JSONL_ENVELOPE_KIND.to_owned(),
            chain: chain.to_owned(),
            record,
            mac: to_hex(&self.jsonl_mac(&mac_input)),
        }
    }

    /// Verify the keyed chain of an append file and return the accepted
    /// records. A legacy (untagged) prefix is upgraded in place after its
    /// records parse cleanly; any MAC mismatch is fatal.
    fn read_jsonl_records<T: serde::de::DeserializeOwned>(
        &self,
        path: &Path,
        label: &str,
    ) -> io::Result<Vec<T>> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        enforce_serialized_size(label, file.metadata()?.len(), MAX_JSONL_FILE_BYTES)?;
        let mut reader = BufReader::new(file);
        let mut records = Vec::new();
        let mut raw_lines: Vec<Vec<u8>> = Vec::new();
        let mut chain = JSONL_CHAIN_GENESIS.to_owned();
        let mut previous_chain = JSONL_CHAIN_GENESIS.to_owned();
        let mut legacy_prefix = false;
        let mut saw_envelope = false;
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
            let (payload, verified_mac) = if let Ok(envelope) =
                serde_json::from_slice::<JsonlEnvelope>(&line)
            {
                if saw_envelope && legacy_prefix {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "{label} `{}` mixes legacy and keyed records after line {}",
                            path.display(),
                            line_index
                        ),
                    ));
                }
                saw_envelope = true;
                let payload =
                    self.verify_jsonl_envelope(path, label, line_index, &envelope, &chain)?;
                // The chain advances by the envelope's *verified* MAC field;
                // recomputing it here would bind the next record to whatever
                // canonicalization this build happens to use instead of the
                // bytes actually committed to disk.
                (payload, envelope.mac)
            } else {
                if !self.allow_legacy_migration {
                    return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "{label} `{}` line {} is legacy untagged state; use the explicit offline migration constructor",
                                path.display(),
                                line_index
                            ),
                        ));
                }
                if saw_envelope {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "{label} `{}` line {} is an untagged record inside a keyed log; \
                             refusing to load possibly tampered history",
                            path.display(),
                            line_index
                        ),
                    ));
                }
                legacy_prefix = true;
                // Upgrade re-MACs the canonical payload (see
                // `jsonl_envelope`), so store the canonical bytes here.
                let canonical = serde_json::from_slice::<serde_json::Value>(&line)
                    .and_then(|value| serde_json::to_vec(&value))
                    .map_err(|error| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "failed to parse {label} `{}` line {}: {error}",
                                path.display(),
                                line_index
                            ),
                        )
                    })?;
                (canonical, chain.clone())
            };
            let record = serde_json::from_slice(&payload).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "failed to parse {label} `{}` line {}: {error}",
                        path.display(),
                        line_index
                    ),
                )
            })?;
            previous_chain = chain;
            chain = verified_mac;
            raw_lines.push(payload);
            records.push(record);
        }
        if legacy_prefix && !records.is_empty() {
            eprintln!(
                "warning: {label} `{}` contains {} untagged legacy record(s); \
                 re-writing with keyed MAC chain",
                path.display(),
                records.len()
            );
            let mut body = Vec::new();
            let mut rewrite_chain = JSONL_CHAIN_GENESIS.to_owned();
            for payload in &raw_lines {
                let envelope = self.jsonl_envelope(&rewrite_chain, payload.clone());
                rewrite_chain = envelope.mac.clone();
                body.extend_from_slice(
                    serde_json::to_string(&envelope)
                        .map_err(invalid_data)?
                        .as_bytes(),
                );
                body.push(b'\n');
            }
            atomic_write(path, body)?;
            chain = rewrite_chain;
            self.write_jsonl_head(path, records.len() as u64, &chain)?;
        } else if !records.is_empty() {
            self.verify_jsonl_head(path, records.len() as u64, &chain, &previous_chain)?;
        }
        Ok(records)
    }

    fn verify_jsonl_envelope(
        &self,
        path: &Path,
        label: &str,
        line_index: usize,
        envelope: &JsonlEnvelope,
        expected_chain: &str,
    ) -> io::Result<Vec<u8>> {
        if envelope.pftmac != JSONL_ENVELOPE_KIND {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{label} `{}` line {} has unknown integrity envelope `{}`",
                    path.display(),
                    line_index,
                    envelope.pftmac
                ),
            ));
        }
        if envelope.chain != expected_chain {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{label} `{}` line {} breaks the MAC chain (expected `{expected_chain}`, \
                     found `{}`); history was truncated or reordered",
                    path.display(),
                    line_index,
                    envelope.chain
                ),
            ));
        }
        let payload = serde_json::to_vec(&envelope.record).map_err(invalid_data)?;
        let mut mac_input = Vec::with_capacity(expected_chain.len() + 1 + payload.len());
        mac_input.extend_from_slice(expected_chain.as_bytes());
        mac_input.push(0);
        mac_input.extend_from_slice(&payload);
        let tag = from_hex(&envelope.mac).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{label} `{}` line {} has a non-hex integrity tag",
                    path.display(),
                    line_index
                ),
            )
        })?;
        if !macs_equal(&self.jsonl_mac(&mac_input), &tag) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{label} `{}` line {} failed integrity verification; refusing to load \
                     possibly tampered history",
                    path.display(),
                    line_index
                ),
            ));
        }
        Ok(payload)
    }

    /// Return the file length and the MAC-chain head (`"genesis"` when the
    /// file is empty/missing or legacy-format, so the first keyed record
    /// always chains from genesis).
    fn read_jsonl_tail(&self, path: &Path) -> io::Result<(u64, String, u64)> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok((0, JSONL_CHAIN_GENESIS.to_owned(), 0));
            }
            Err(error) => return Err(error),
        };
        let len = file.metadata()?.len();
        if len == 0 {
            return Ok((0, JSONL_CHAIN_GENESIS.to_owned(), 0));
        }
        let mut reader = BufReader::new(file);
        let mut chain = JSONL_CHAIN_GENESIS.to_owned();
        let mut previous_chain = JSONL_CHAIN_GENESIS.to_owned();
        let mut record_count = 0_u64;
        let mut line = Vec::new();
        loop {
            line.clear();
            let read = reader
                .by_ref()
                .take(MAX_JSONL_RECORD_BYTES as u64 + 2)
                .read_until(b'\n', &mut line)?;
            if read == 0 || !line.ends_with(b"\n") {
                break;
            }
            let mut trimmed: &[u8] = &line;
            while matches!(trimmed.last(), Some(b'\n' | b'\r')) {
                trimmed = &trimmed[..trimmed.len() - 1];
            }
            if trimmed.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let envelope: JsonlEnvelope = serde_json::from_slice(trimmed).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "JSONL append `{}` contains an untagged record; migrate it before appending",
                        path.display()
                    ),
                )
            })?;
            self.verify_jsonl_envelope(
                path,
                "JSONL append",
                record_count.saturating_add(1) as usize,
                &envelope,
                &chain,
            )?;
            previous_chain = chain;
            chain = envelope.mac;
            record_count = record_count.saturating_add(1);
        }
        self.verify_jsonl_head(path, record_count, &chain, &previous_chain)?;
        Ok((len, chain, record_count))
    }

    fn jsonl_head_path(path: &Path) -> PathBuf {
        let mut name = path
            .file_name()
            .map(|name| name.to_os_string())
            .unwrap_or_default();
        name.push(".head");
        path.with_file_name(name)
    }

    fn jsonl_head_mac(&self, count: u64, chain: &str) -> io::Result<[u8; MAC_BYTES]> {
        let payload =
            serde_json::to_vec(&(JSONL_HEAD_SCHEMA, count, chain)).map_err(invalid_data)?;
        Ok(self
            .integrity_key
            .mac(b"postfiat.storage.jsonl-head.v1", &payload))
    }

    fn write_jsonl_head(&self, path: &Path, count: u64, chain: &str) -> io::Result<()> {
        let head = JsonlHead {
            schema: JSONL_HEAD_SCHEMA.to_owned(),
            record_count: count,
            chain: chain.to_owned(),
            mac: to_hex(&self.jsonl_head_mac(count, chain)?),
        };
        atomic_write(
            Self::jsonl_head_path(path),
            serde_json::to_vec(&head).map_err(invalid_data)?,
        )
    }

    fn verify_jsonl_head(
        &self,
        path: &Path,
        count: u64,
        chain: &str,
        previous_chain: &str,
    ) -> io::Result<()> {
        let head_path = Self::jsonl_head_path(path);
        let bytes = match fs::read(&head_path) {
            Ok(bytes) => bytes,
            Err(error)
                if error.kind() == io::ErrorKind::NotFound && self.allow_legacy_migration =>
            {
                self.write_jsonl_head(path, count, chain)?;
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "JSONL head `{}` is missing; possible tail rollback",
                        head_path.display()
                    ),
                ));
            }
            Err(error) => return Err(error),
        };
        let head: JsonlHead = serde_json::from_slice(&bytes).map_err(invalid_data)?;
        let expected_mac = self.jsonl_head_mac(head.record_count, &head.chain)?;
        if head.schema != JSONL_HEAD_SCHEMA
            || !macs_equal(&expected_mac, &from_hex(&head.mac).unwrap_or_default())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "JSONL head `{}` failed integrity verification",
                    head_path.display()
                ),
            ));
        }
        if head.record_count == count && head.chain == chain {
            return Ok(());
        }
        // The log is fsynced before the head. A crash in that narrow interval
        // leaves exactly one fully authenticated record beyond the old head;
        // advance the head after verifying the complete chain.
        if head.record_count.saturating_add(1) == count && head.chain == previous_chain {
            self.write_jsonl_head(path, count, chain)?;
            return Ok(());
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "JSONL head `{}` does not match the authenticated log tail; possible rollback",
                head_path.display()
            ),
        ))
    }

    fn remove_jsonl_log(&self, path: &Path) -> io::Result<()> {
        remove_optional_file(path.to_path_buf())?;
        remove_optional_file(Self::jsonl_head_path(path))
    }
}

const JSONL_HEAD_SCHEMA: &str = "postfiat-storage-jsonl-head-v1";

#[derive(Debug, Serialize, Deserialize)]
struct JsonlEnvelope {
    pftmac: String,
    chain: String,
    record: serde_json::Value,
    mac: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonlHead {
    schema: String,
    record_count: u64,
    chain: String,
    mac: String,
}

fn file_mac_domain(label: &str) -> String {
    format!("postfiat.storage.state-file.v1:{label}")
}

/// Split a whole-file state document into its JSON body and trailing
/// `pftmac1:<hex>` integrity tag, if present.
fn split_mac_trailer(raw: &str) -> Option<(&str, &str)> {
    let trimmed = raw.trim_end_matches(['\n', '\r']);
    let last_line = trimmed.lines().last()?;
    let tag_hex = last_line.strip_prefix(FILE_MAC_MARKER)?;
    let body_end = trimmed.len() - last_line.len();
    let body = trimmed[..body_end].trim_end_matches(['\n', '\r']);
    Some((body, tag_hex))
}

fn strip_trailing_newlines(body: &[u8]) -> &[u8] {
    let mut end = body.len();
    while end > 0 && matches!(body[end - 1], b'\n' | b'\r') {
        end -= 1;
    }
    &body[..end]
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
    // Zeroed storage is valid for the plain-old-data `statvfs` struct, so no
    // `assume_init` is required: statvfs overwrites the fields on success and
    // we never read them on failure.
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is a live NUL-terminated CString and `stats` points to
    // writable, correctly sized storage.
    if unsafe { libc::statvfs(path.as_ptr(), &mut stats) } != 0 {
        return Err(io::Error::last_os_error());
    }
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
        .iter_mut()
        .find(|existing| existing.tx_id == receipt.tx_id)
    {
        if existing == &receipt {
            return Ok(());
        }
        // A rejected governance operation can be retried with the same stable
        // amendment id after its fail-closed precondition is satisfied.  The
        // later accepted receipt is the terminal result for that operation.
        // Never permit an accepted result to be replaced or two conflicting
        // results with the same acceptance status.
        if !existing.accepted && receipt.accepted {
            *existing = receipt;
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
    fn receipt_append_log_allows_rejected_operation_to_reach_terminal_acceptance() {
        let dir = unique_test_dir("postfiat-storage-receipt-retry-test");
        let store = NodeStore::new(&dir);
        let rejected = sample_receipt("amendment-1", "tecPRECONDITION");
        let accepted = sample_receipt("amendment-1", "tesSUCCESS");
        store
            .write_receipts(&[rejected])
            .expect("write rejected receipt");
        store
            .append_receipt_record(&accepted)
            .expect("append accepted retry");

        assert_eq!(
            store.read_receipts().expect("read terminal acceptance"),
            vec![accepted]
        );

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
        OpenOptions::new()
            .append(true)
            .open(dir.join(BLOCKS_APPEND_FILE))
            .and_then(|mut file| file.write_all(b"{\"partial\":"))
            .expect("write partial append tail");

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

        let store = NodeStore::new(&dir);
        let error = store
            .append_jsonl_record(path.clone(), &sample_receipt("tx-size", "tesSUCCESS"))
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
                bridge_exit_root: None,
                pftl_uniswap_receipt_root: None,
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

    #[test]
    fn tampered_state_file_is_rejected() {
        let dir = unique_test_dir("postfiat-storage-tampered-state-test");
        let store = NodeStore::new(&dir);
        store
            .write_ledger(&LedgerState::empty())
            .expect("write ledger");
        assert!(store.read_ledger().is_ok(), "baseline read must pass");

        let path = dir.join(LEDGER_FILE);
        let raw = fs::read_to_string(&path).expect("read ledger file");
        assert!(raw.contains(FILE_MAC_MARKER), "file must carry a MAC");
        // Flip a byte inside the JSON body without touching the MAC trailer.
        let tampered = raw.replacen("\"accounts\"", "\"accountz\"", 1);
        assert_ne!(tampered, raw);
        fs::write(&path, tampered).expect("tamper ledger");

        let error = store.read_ledger().expect_err("tampered ledger must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("MAC mismatch"), "{error}");

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn legacy_state_file_loads_and_upgrades() {
        let dir = unique_test_dir("postfiat-storage-legacy-state-test");
        let normal_store = NodeStore::new(&dir);
        let ledger = LedgerState::empty();
        let legacy_json = format!("{}\n", serde_json::to_string_pretty(&ledger).expect("json"));
        let path = dir.join(LEDGER_FILE);
        fs::write(&path, &legacy_json).expect("write legacy ledger");
        let error = normal_store
            .read_ledger()
            .expect_err("normal open must reject legacy ledger");
        assert!(error.to_string().contains("explicit offline migration"));
        let store = NodeStore::try_new_for_legacy_migration(&dir)
            .expect("open explicit legacy migration store");

        assert_eq!(
            store.read_ledger().expect("legacy ledger must load"),
            ledger
        );
        let upgraded = fs::read_to_string(&path).expect("read upgraded file");
        assert!(
            upgraded.contains(FILE_MAC_MARKER),
            "legacy file must be re-written with an integrity trailer"
        );
        assert_eq!(
            store.read_ledger().expect("upgraded ledger must load"),
            ledger
        );

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn tampered_jsonl_record_is_rejected() {
        let dir = unique_test_dir("postfiat-storage-tampered-jsonl-test");
        let store = NodeStore::new(&dir);
        store.write_receipts(&[]).expect("write empty receipts");
        store
            .append_receipt_record(&sample_receipt("tx-1", "tesSUCCESS"))
            .expect("append receipt");
        assert_eq!(store.read_receipts().expect("baseline").len(), 1);

        let path = dir.join(RECEIPTS_APPEND_FILE);
        let raw = fs::read_to_string(&path).expect("read append log");
        let tampered = raw.replacen("tesSUCCESS", "tesFAILED!", 1);
        assert_ne!(tampered, raw);
        fs::write(&path, tampered).expect("tamper append log");

        let error = store
            .read_receipts()
            .expect_err("tampered receipt log must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains("integrity verification"),
            "{error}"
        );

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn truncated_jsonl_chain_is_rejected() {
        let dir = unique_test_dir("postfiat-storage-truncated-jsonl-test");
        let store = NodeStore::new(&dir);
        store.write_receipts(&[]).expect("write empty receipts");
        store
            .append_receipt_record(&sample_receipt("tx-1", "tesSUCCESS"))
            .expect("append first");
        store
            .append_receipt_record(&sample_receipt("tx-2", "tesSUCCESS"))
            .expect("append second");

        // Delete the first record: the second record's chain pointer no
        // longer starts at genesis, so the gap must be detected.
        let path = dir.join(RECEIPTS_APPEND_FILE);
        let raw = fs::read_to_string(&path).expect("read append log");
        let second_line = raw.lines().nth(1).expect("two records").to_string();
        fs::write(&path, format!("{second_line}\n")).expect("drop first record");

        let error = store
            .read_receipts()
            .expect_err("truncated receipt log must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("MAC chain"), "{error}");

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn deleted_jsonl_tail_is_rejected_by_authenticated_head() {
        let dir = unique_test_dir("postfiat-storage-deleted-jsonl-tail-test");
        let store = NodeStore::new(&dir);
        store.write_receipts(&[]).expect("write empty receipts");
        store
            .append_receipt_record(&sample_receipt("tx-1", "tesSUCCESS"))
            .expect("append first");
        store
            .append_receipt_record(&sample_receipt("tx-2", "tesSUCCESS"))
            .expect("append second");

        let path = dir.join(RECEIPTS_APPEND_FILE);
        let raw = fs::read_to_string(&path).expect("read append log");
        let first_line = raw.lines().next().expect("first record");
        fs::write(&path, format!("{first_line}\n")).expect("delete authenticated tail");

        let error = store
            .read_receipts()
            .expect_err("deleted final record must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("possible rollback"), "{error}");

        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn legacy_jsonl_log_loads_and_upgrades() {
        let dir = unique_test_dir("postfiat-storage-legacy-jsonl-test");
        let normal_store = NodeStore::new(&dir);
        normal_store
            .write_receipts(&[])
            .expect("write empty receipts");
        let receipt = sample_receipt("tx-legacy", "tesSUCCESS");
        // Write a pre-integrity (bare JSON record) append log.
        let path = dir.join(RECEIPTS_APPEND_FILE);
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&receipt).expect("json")),
        )
        .expect("write legacy append log");
        let error = normal_store
            .read_receipts()
            .expect_err("normal open must reject legacy append log");
        assert!(error.to_string().contains("explicit offline migration"));
        let store = NodeStore::try_new_for_legacy_migration(&dir)
            .expect("open explicit legacy migration store");

        assert_eq!(
            store.read_receipts().expect("legacy log must load"),
            vec![receipt.clone()]
        );
        let upgraded = fs::read_to_string(&path).expect("read upgraded log");
        assert!(
            upgraded.contains("\"pftmac\""),
            "legacy log must be re-written with MAC envelopes"
        );
        assert_eq!(
            store.read_receipts().expect("upgraded log must load"),
            vec![receipt]
        );

        fs::remove_dir_all(dir).expect("cleanup");
    }
}
