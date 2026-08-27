//! Primary transactional storage for bounded finalized-block persistence.
//!
//! redb's physical pages are a node-local implementation detail. Every
//! protocol-visible record is encoded independently, bound to its logical
//! table and canonical key with the node-local HMAC, and written through one
//! serializable transaction. Consensus code must never depend on redb page
//! layout or backend iteration order.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use postfiat_types::{
    BatchArchiveEntry, BlockRecord, BridgeState, ChainTipState, FastPayVersionFenceV1,
    GovernanceState, LedgerState, NodeState, Receipt, ShieldedState,
};
use redb::{
    Database, Durability, ReadOnlyDatabase, ReadTransaction, ReadableDatabase, ReadableTable,
    ReadableTableMetadata, TableDefinition,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::integrity::{macs_equal, IntegrityKey, MAC_BYTES};
use crate::ordered_history::ORDERED_HISTORY_COMMITMENT_SCHEMA;
use crate::{NodeStore, OrderedHistoryCommitment};

mod canonical_export;
mod export;
mod fastpay_index;
mod generation;

pub use canonical_export::CanonicalExportReceiptV1;
pub use export::CanonicalHistoryIndexEntryV1;
pub use generation::TransactionalGenerationPointerV1;

pub const TRANSACTIONAL_BACKEND: &str = "redb";
pub const TRANSACTIONAL_BACKEND_VERSION: &str = "4.2.0";
pub const TRANSACTIONAL_STORE_SCHEMA: &str = "postfiat-transactional-store-v1";
pub const TRANSACTIONAL_STORAGE_FORMAT: &str = "postfiat-redb-v1";
pub const TRANSACTIONAL_VERIFIER_VERSION: &str = "postfiat.storage_verifier.v2";
pub const TRANSACTIONAL_DATABASE_FILE: &str = "postfiat-state-v1.redb";
pub const TRANSACTIONAL_GENERATION: &str = "generation-00000001";
pub const TRANSACTIONAL_GENERATION_POINTER_FILE: &str = "transactional_generation.json";
pub const TRANSACTIONAL_GENERATION_POINTER_SCHEMA: &str =
    "postfiat-transactional-generation-pointer-v1";

const VALUE_SCHEMA_VERSION: u8 = 1;
const VALUE_MAC_DOMAIN: &[u8] = b"postfiat.transactional-store.value.v1";
const META_KEY: &[u8] = b"store-meta-v1";
const MAX_ID_BYTES: usize = 1024;
const MAX_DOMAIN_NAME_BYTES: usize = 64;
const MAX_META_BYTES: usize = 1024 * 1024;
const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
const MAX_CURRENT_STATE_BYTES: usize = 256 * 1024 * 1024;
const MAX_RANGE_LIMIT: usize = 100_000;
const MAX_RECEIPT_OCCURRENCES_PER_ID: usize = 1024;

const META: TableDefinition<&[u8], &[u8]> = TableDefinition::new("meta_v1");
const BLOCKS_BY_HEIGHT: TableDefinition<&[u8], &[u8]> = TableDefinition::new("blocks_by_height_v1");
const BLOCK_HEIGHT_BY_HASH: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("block_height_by_hash_v1");
const RECEIPTS_BY_ID: TableDefinition<&[u8], &[u8]> = TableDefinition::new("receipts_by_id_v1");
const BATCH_ARCHIVE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("batch_archive_v1");
const ORDERED_BY_ID: TableDefinition<&[u8], &[u8]> = TableDefinition::new("ordered_by_id_v1");
const ORDERED_BY_ORDINAL: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("ordered_by_ordinal_v1");
const CURRENT_STATE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("current_state_v1");
const HISTORY_INDEXES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("history_indexes_v1");

const META_TABLE: &str = "meta_v1";
const BLOCKS_BY_HEIGHT_TABLE: &str = "blocks_by_height_v1";
const BLOCK_HEIGHT_BY_HASH_TABLE: &str = "block_height_by_hash_v1";
const RECEIPTS_BY_ID_TABLE: &str = "receipts_by_id_v1";
const BATCH_ARCHIVE_TABLE: &str = "batch_archive_v1";
const ORDERED_BY_ID_TABLE: &str = "ordered_by_id_v1";
const ORDERED_BY_ORDINAL_TABLE: &str = "ordered_by_ordinal_v1";
const CURRENT_STATE_TABLE: &str = "current_state_v1";
const HISTORY_INDEXES_TABLE: &str = "history_indexes_v1";

const STATE_LEDGER: &str = "ledger";
const STATE_GOVERNANCE: &str = "governance";
const STATE_SHIELDED: &str = "shielded";
const STATE_BRIDGE: &str = "bridge";
const STATE_NODE: &str = "node_state";
const ALLOWED_ADDITIONAL_STATE_DOMAINS: &[&str] = &[
    "validator_registry",
    "storage_activation",
    "retained_history_checkpoint",
];
const FASTPAY_ANCHOR_KEY_PREFIX: &[u8] = b"fastpay_anchor_v1\0";

static SHARED_STORES: OnceLock<Mutex<HashMap<PathBuf, Arc<TransactionalStore>>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageErrorCode {
    Database,
    Serialization,
    Uninitialized,
    InitializationConflict,
    UnsupportedSchema,
    DomainMismatch,
    ExpectedTipMismatch,
    NonSequentialHeight,
    ParentHashMismatch,
    CountMismatch,
    InvalidBlock,
    ReceiptMismatch,
    ArchiveMismatch,
    OrderedCommitmentMismatch,
    DuplicateRecord,
    IdempotentConflict,
    IntegrityFailure,
    NonCanonicalKey,
    SizeLimit,
    CorruptRecord,
}

impl StorageErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Database => "storage_database_error",
            Self::Serialization => "storage_serialization_error",
            Self::Uninitialized => "storage_uninitialized",
            Self::InitializationConflict => "storage_initialization_conflict",
            Self::UnsupportedSchema => "storage_unsupported_schema",
            Self::DomainMismatch => "storage_domain_mismatch",
            Self::ExpectedTipMismatch => "storage_expected_tip_mismatch",
            Self::NonSequentialHeight => "storage_non_sequential_height",
            Self::ParentHashMismatch => "storage_parent_hash_mismatch",
            Self::CountMismatch => "storage_count_mismatch",
            Self::InvalidBlock => "storage_invalid_block",
            Self::ReceiptMismatch => "storage_receipt_mismatch",
            Self::ArchiveMismatch => "storage_archive_mismatch",
            Self::OrderedCommitmentMismatch => "storage_ordered_commitment_mismatch",
            Self::DuplicateRecord => "storage_duplicate_record",
            Self::IdempotentConflict => "storage_idempotent_conflict",
            Self::IntegrityFailure => "storage_integrity_failure",
            Self::NonCanonicalKey => "storage_non_canonical_key",
            Self::SizeLimit => "storage_size_limit",
            Self::CorruptRecord => "storage_corrupt_record",
        }
    }
}

#[derive(Debug)]
pub struct StorageError {
    code: StorageErrorCode,
    message: String,
}

impl StorageError {
    fn new(code: StorageErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub const fn code(&self) -> StorageErrorCode {
        self.code
    }

    pub const fn reason_code(&self) -> &'static str {
        self.code.as_str()
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_code(), self.message)
    }
}

impl std::error::Error for StorageError {}

impl From<StorageError> for io::Error {
    fn from(error: StorageError) -> Self {
        let kind = match error.code {
            StorageErrorCode::Database => io::ErrorKind::Other,
            StorageErrorCode::DuplicateRecord => io::ErrorKind::AlreadyExists,
            StorageErrorCode::Uninitialized => io::ErrorKind::NotFound,
            StorageErrorCode::SizeLimit | StorageErrorCode::NonCanonicalKey => {
                io::ErrorKind::InvalidInput
            }
            _ => io::ErrorKind::InvalidData,
        };
        io::Error::new(kind, error)
    }
}

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionalStoreMetaV1 {
    pub schema: String,
    pub storage_format: String,
    pub backend: String,
    pub backend_version: String,
    pub generation: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
    pub finalized_state_root: String,
    pub ordered_batch_count: u64,
    pub receipt_count: u64,
    pub history_base_height: u64,
    pub ordered_history_schema: String,
    pub ordered_history_accumulator: String,
    pub scheduled_activation_height: Option<u64>,
    pub last_full_verification_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_packet_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifier_version: Option<String>,
}

impl TransactionalStoreMetaV1 {
    fn from_tip_and_commitment(tip: &ChainTipState, commitment: &OrderedHistoryCommitment) -> Self {
        Self {
            schema: TRANSACTIONAL_STORE_SCHEMA.to_owned(),
            storage_format: TRANSACTIONAL_STORAGE_FORMAT.to_owned(),
            backend: TRANSACTIONAL_BACKEND.to_owned(),
            backend_version: TRANSACTIONAL_BACKEND_VERSION.to_owned(),
            generation: TRANSACTIONAL_GENERATION.to_owned(),
            chain_id: tip.chain_id.clone(),
            genesis_hash: tip.genesis_hash.clone(),
            protocol_version: tip.protocol_version,
            finalized_height: tip.height,
            finalized_block_hash: tip.block_hash.clone(),
            finalized_state_root: tip.state_root.clone(),
            ordered_batch_count: tip.ordered_batch_count,
            receipt_count: tip.receipt_count,
            history_base_height: tip.history_base_height,
            ordered_history_schema: commitment.schema.clone(),
            ordered_history_accumulator: commitment.accumulator.clone(),
            scheduled_activation_height: None,
            last_full_verification_height: None,
            migration_packet_root: None,
            verifier_version: None,
        }
    }

    pub fn chain_tip(&self, schema: impl Into<String>) -> ChainTipState {
        ChainTipState {
            schema: schema.into(),
            chain_id: self.chain_id.clone(),
            genesis_hash: self.genesis_hash.clone(),
            protocol_version: self.protocol_version,
            height: self.finalized_height,
            block_hash: self.finalized_block_hash.clone(),
            state_root: self.finalized_state_root.clone(),
            ordered_batch_count: self.ordered_batch_count,
            receipt_count: self.receipt_count,
            history_base_height: self.history_base_height,
        }
    }

    pub fn ordered_history_commitment(&self) -> OrderedHistoryCommitment {
        OrderedHistoryCommitment {
            schema: self.ordered_history_schema.clone(),
            chain_id: self.chain_id.clone(),
            genesis_hash: self.genesis_hash.clone(),
            protocol_version: self.protocol_version,
            count: self.ordered_batch_count,
            accumulator: self.ordered_history_accumulator.clone(),
        }
    }

    fn validate(&self) -> StorageResult<()> {
        if self.schema != TRANSACTIONAL_STORE_SCHEMA
            || self.storage_format != TRANSACTIONAL_STORAGE_FORMAT
            || self.backend != TRANSACTIONAL_BACKEND
            || self.backend_version != TRANSACTIONAL_BACKEND_VERSION
            || self.generation != TRANSACTIONAL_GENERATION
            || self.ordered_history_schema != ORDERED_HISTORY_COMMITMENT_SCHEMA
        {
            return Err(StorageError::new(
                StorageErrorCode::UnsupportedSchema,
                "transactional store metadata uses an unsupported schema or backend",
            ));
        }
        validate_identifier("chain id", &self.chain_id)?;
        validate_identifier("genesis hash", &self.genesis_hash)?;
        validate_identifier("finalized block hash", &self.finalized_block_hash)?;
        validate_identifier("finalized state root", &self.finalized_state_root)?;
        if self.finalized_height < self.history_base_height {
            return Err(StorageError::new(
                StorageErrorCode::CountMismatch,
                "finalized height is below the retained-history base",
            ));
        }
        if self
            .last_full_verification_height
            .is_some_and(|height| height > self.finalized_height)
        {
            return Err(StorageError::new(
                StorageErrorCode::CountMismatch,
                "full-verification height is ahead of the finalized tip",
            ));
        }
        let commitment = self.ordered_history_commitment();
        commitment
            .validate_domain(&self.chain_id, &self.genesis_hash, self.protocol_version)
            .map_err(|error| {
                StorageError::new(StorageErrorCode::DomainMismatch, error.to_string())
            })?;
        match (&self.migration_packet_root, &self.verifier_version) {
            (None, None) => {}
            (Some(root), Some(version))
                if root.len() == 96
                    && root
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    && version == TRANSACTIONAL_VERIFIER_VERSION => {}
            _ => {
                return Err(StorageError::new(
                    StorageErrorCode::UnsupportedSchema,
                    "transactional migration verification binding is invalid",
                ));
            }
        }
        Ok(())
    }

    fn matches_tip(&self, tip: &ChainTipState) -> bool {
        self.chain_id == tip.chain_id
            && self.genesis_hash == tip.genesis_hash
            && self.protocol_version == tip.protocol_version
            && self.finalized_height == tip.height
            && self.finalized_block_hash == tip.block_hash
            && self.finalized_state_root == tip.state_root
            && self.ordered_batch_count == tip.ordered_batch_count
            && self.receipt_count == tip.receipt_count
            && self.history_base_height == tip.history_base_height
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedStateValue {
    pub domain: String,
    pub canonical_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CurrentStateUpdate<'a> {
    pub ledger: Option<&'a LedgerState>,
    pub governance: Option<&'a GovernanceState>,
    pub shielded: Option<&'a ShieldedState>,
    pub bridge: Option<&'a BridgeState>,
    pub node_state: Option<&'a NodeState>,
    pub additional: &'a [NamedStateValue],
}

#[derive(Debug, Clone, Copy)]
pub struct CommitFinalizedBlock<'a> {
    pub expected_tip: &'a ChainTipState,
    pub new_tip: &'a ChainTipState,
    pub block: &'a BlockRecord,
    pub receipts: &'a [Receipt],
    pub archive_entry: &'a BatchArchiveEntry,
    pub batch_id: &'a str,
    pub ordered_history: &'a OrderedHistoryCommitment,
    pub current_state: CurrentStateUpdate<'a>,
    pub scheduled_activation_height: Option<u64>,
    pub allow_legacy_receipt_id_mismatch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitOutcome {
    Committed,
    AlreadyCommitted,
}

#[derive(Debug, Clone, Copy)]
pub struct PruneRetainedHistory<'a> {
    pub expected_tip: &'a ChainTipState,
    pub new_history_base_height: u64,
    pub retained_checkpoint: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneOutcome {
    pub previous_history_base_height: u64,
    pub new_history_base_height: u64,
    pub pruned_block_count: u64,
    pub pruned_archive_count: u64,
    pub pruned_receipt_count: u64,
    pub remaining_receipt_count: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionalWorkCounters {
    pub read_transactions: u64,
    pub write_transactions: u64,
    pub committed_write_transactions: u64,
    pub records_read: u64,
    pub records_written: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub page_reads: u64,
    pub page_writes: u64,
    pub full_history_scans: u64,
    pub full_history_records_read: u64,
    pub full_history_bytes_read: u64,
    pub durable_commit_micros: u64,
}

impl TransactionalWorkCounters {
    #[must_use]
    pub fn saturating_delta(self, before: Self) -> Self {
        Self {
            read_transactions: self
                .read_transactions
                .saturating_sub(before.read_transactions),
            write_transactions: self
                .write_transactions
                .saturating_sub(before.write_transactions),
            committed_write_transactions: self
                .committed_write_transactions
                .saturating_sub(before.committed_write_transactions),
            records_read: self.records_read.saturating_sub(before.records_read),
            records_written: self.records_written.saturating_sub(before.records_written),
            bytes_read: self.bytes_read.saturating_sub(before.bytes_read),
            bytes_written: self.bytes_written.saturating_sub(before.bytes_written),
            page_reads: self.page_reads.saturating_sub(before.page_reads),
            page_writes: self.page_writes.saturating_sub(before.page_writes),
            full_history_scans: self
                .full_history_scans
                .saturating_sub(before.full_history_scans),
            full_history_records_read: self
                .full_history_records_read
                .saturating_sub(before.full_history_records_read),
            full_history_bytes_read: self
                .full_history_bytes_read
                .saturating_sub(before.full_history_bytes_read),
            durable_commit_micros: self
                .durable_commit_micros
                .saturating_sub(before.durable_commit_micros),
        }
    }
}

#[derive(Debug, Default)]
struct WorkCounterState {
    read_transactions: AtomicU64,
    write_transactions: AtomicU64,
    committed_write_transactions: AtomicU64,
    records_read: AtomicU64,
    records_written: AtomicU64,
    bytes_read: AtomicU64,
    bytes_written: AtomicU64,
    page_reads: AtomicU64,
    page_writes: AtomicU64,
    full_history_scans: AtomicU64,
    full_history_records_read: AtomicU64,
    full_history_bytes_read: AtomicU64,
    durable_commit_micros: AtomicU64,
}

enum TransactionalDatabase {
    ReadWrite(Database),
    ReadOnly(ReadOnlyDatabase),
}

impl TransactionalDatabase {
    fn begin_read(&self) -> Result<ReadTransaction, redb::TransactionError> {
        match self {
            Self::ReadWrite(database) => database.begin_read(),
            Self::ReadOnly(database) => database.begin_read(),
        }
    }
}

pub struct TransactionalStore {
    database_path: PathBuf,
    database: TransactionalDatabase,
    integrity_key: IntegrityKey,
    counters: Arc<WorkCounterState>,
}

/// One backend-consistent read view bound to the authenticated finalized tip
/// captured when the redb read transaction began.
pub struct TransactionalReadSnapshot {
    transaction: ReadTransaction,
    meta: TransactionalStoreMetaV1,
    integrity_key: IntegrityKey,
    counters: Arc<WorkCounterState>,
}

impl fmt::Debug for TransactionalStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransactionalStore")
            .field("database_path", &self.database_path)
            .finish_non_exhaustive()
    }
}

impl TransactionalStore {
    pub fn open(data_dir: impl AsRef<Path>) -> StorageResult<Self> {
        let data_dir = data_dir.as_ref();
        fs::create_dir_all(data_dir).map_err(database_error)?;
        let integrity_key = IntegrityKey::load_or_create(data_dir).map_err(database_error)?;
        Self::open_with_integrity_key(data_dir, integrity_key)
    }

    fn open_with_integrity_key(
        data_dir: &Path,
        integrity_key: IntegrityKey,
    ) -> StorageResult<Self> {
        fs::create_dir_all(data_dir).map_err(database_error)?;
        let database_path = data_dir.join(TRANSACTIONAL_DATABASE_FILE);
        let database = Database::create(&database_path).map_err(database_error)?;
        Ok(Self {
            database_path,
            database: TransactionalDatabase::ReadWrite(database),
            integrity_key,
            counters: Arc::new(WorkCounterState::default()),
        })
    }

    fn open_read_only_with_integrity_key(
        data_dir: &Path,
        integrity_key: IntegrityKey,
    ) -> StorageResult<Self> {
        if !data_dir.is_dir() {
            return Err(StorageError::new(
                StorageErrorCode::Database,
                format!(
                    "storage_transactional_read_only_directory_missing: `{}` does not exist",
                    data_dir.display()
                ),
            ));
        }
        let database_path = data_dir.join(TRANSACTIONAL_DATABASE_FILE);
        let database = ReadOnlyDatabase::open(&database_path).map_err(database_error)?;
        Ok(Self {
            database_path,
            database: TransactionalDatabase::ReadOnly(database),
            integrity_key,
            counters: Arc::new(WorkCounterState::default()),
        })
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn work_counters(&self) -> TransactionalWorkCounters {
        TransactionalWorkCounters {
            read_transactions: self.counters.read_transactions.load(Ordering::Relaxed),
            write_transactions: self.counters.write_transactions.load(Ordering::Relaxed),
            committed_write_transactions: self
                .counters
                .committed_write_transactions
                .load(Ordering::Relaxed),
            records_read: self.counters.records_read.load(Ordering::Relaxed),
            records_written: self.counters.records_written.load(Ordering::Relaxed),
            bytes_read: self.counters.bytes_read.load(Ordering::Relaxed),
            bytes_written: self.counters.bytes_written.load(Ordering::Relaxed),
            page_reads: self.counters.page_reads.load(Ordering::Relaxed),
            page_writes: self.counters.page_writes.load(Ordering::Relaxed),
            full_history_scans: self.counters.full_history_scans.load(Ordering::Relaxed),
            full_history_records_read: self
                .counters
                .full_history_records_read
                .load(Ordering::Relaxed),
            full_history_bytes_read: self
                .counters
                .full_history_bytes_read
                .load(Ordering::Relaxed),
            durable_commit_micros: self.counters.durable_commit_micros.load(Ordering::Relaxed),
        }
    }

    pub fn reset_work_counters(&self) {
        self.counters.read_transactions.store(0, Ordering::Relaxed);
        self.counters.write_transactions.store(0, Ordering::Relaxed);
        self.counters
            .committed_write_transactions
            .store(0, Ordering::Relaxed);
        self.counters.records_read.store(0, Ordering::Relaxed);
        self.counters.records_written.store(0, Ordering::Relaxed);
        self.counters.bytes_read.store(0, Ordering::Relaxed);
        self.counters.bytes_written.store(0, Ordering::Relaxed);
        self.counters.page_reads.store(0, Ordering::Relaxed);
        self.counters.page_writes.store(0, Ordering::Relaxed);
        self.counters.full_history_scans.store(0, Ordering::Relaxed);
        self.counters
            .full_history_records_read
            .store(0, Ordering::Relaxed);
        self.counters
            .full_history_bytes_read
            .store(0, Ordering::Relaxed);
        self.counters
            .durable_commit_micros
            .store(0, Ordering::Relaxed);
    }

    pub fn initialize(
        &self,
        tip: &ChainTipState,
        commitment: &OrderedHistoryCommitment,
        current_state: CurrentStateUpdate<'_>,
    ) -> StorageResult<()> {
        self.initialize_with_activation(tip, commitment, current_state, None)
    }

    pub fn initialize_with_activation(
        &self,
        tip: &ChainTipState,
        commitment: &OrderedHistoryCommitment,
        current_state: CurrentStateUpdate<'_>,
        scheduled_activation_height: Option<u64>,
    ) -> StorageResult<()> {
        validate_initial_state(tip, commitment)?;
        if scheduled_activation_height == Some(0) {
            return Err(StorageError::new(
                StorageErrorCode::InitializationConflict,
                "transactional storage activation height must be positive",
            ));
        }
        let mut meta = TransactionalStoreMetaV1::from_tip_and_commitment(tip, commitment);
        meta.scheduled_activation_height = scheduled_activation_height;
        meta.validate()?;

        let transaction = self.begin_durable_write()?;
        let mut meta_table = transaction.open_table(META).map_err(database_error)?;
        if let Some(raw) = read_authenticated(
            &meta_table,
            META_TABLE,
            META_KEY,
            MAX_META_BYTES,
            &self.integrity_key,
            &self.counters,
        )? {
            let stored: TransactionalStoreMetaV1 = decode_json(&raw)?;
            stored.validate()?;
            if stored == meta {
                return Ok(());
            }
            return Err(StorageError::new(
                StorageErrorCode::InitializationConflict,
                "database is already initialized for a different state",
            ));
        }

        ensure_all_non_meta_tables_empty(&transaction)?;
        let meta_bytes = encode_json(&meta, MAX_META_BYTES)?;
        insert_authenticated(
            &mut meta_table,
            META_TABLE,
            META_KEY,
            &meta_bytes,
            MAX_META_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        drop(meta_table);
        write_current_state(
            &transaction,
            current_state,
            &self.integrity_key,
            &self.counters,
        )?;
        self.commit_durable_write(transaction)
    }

    /// Initialize a generation at an authenticated retained-history checkpoint.
    /// The checkpoint prefix has no local block, archive, or receipt rows, but
    /// its complete ordered-batch prefix is retained so duplicate detection and
    /// the append-only accumulator remain exact.
    pub fn initialize_from_retained_checkpoint(
        &self,
        tip: &ChainTipState,
        commitment: &OrderedHistoryCommitment,
        ordered_batches: &[String],
        current_state: CurrentStateUpdate<'_>,
    ) -> StorageResult<()> {
        self.initialize_from_retained_checkpoint_with_activation(
            tip,
            commitment,
            ordered_batches,
            current_state,
            None,
        )
    }

    pub fn initialize_from_retained_checkpoint_with_activation(
        &self,
        tip: &ChainTipState,
        commitment: &OrderedHistoryCommitment,
        ordered_batches: &[String],
        current_state: CurrentStateUpdate<'_>,
        scheduled_activation_height: Option<u64>,
    ) -> StorageResult<()> {
        validate_retained_initial_state(tip, commitment, ordered_batches)?;
        if scheduled_activation_height == Some(0) {
            return Err(StorageError::new(
                StorageErrorCode::InitializationConflict,
                "transactional storage activation height must be positive",
            ));
        }
        let mut meta = TransactionalStoreMetaV1::from_tip_and_commitment(tip, commitment);
        meta.scheduled_activation_height = scheduled_activation_height;
        meta.validate()?;

        let transaction = self.begin_durable_write()?;
        let mut meta_table = transaction.open_table(META).map_err(database_error)?;
        if let Some(raw) = read_authenticated(
            &meta_table,
            META_TABLE,
            META_KEY,
            MAX_META_BYTES,
            &self.integrity_key,
            &self.counters,
        )? {
            let stored: TransactionalStoreMetaV1 = decode_json(&raw)?;
            stored.validate()?;
            if stored == meta {
                drop(meta_table);
                verify_retained_ordered_prefix(
                    &transaction,
                    ordered_batches,
                    &self.integrity_key,
                    &self.counters,
                )?;
                if let Some(ledger) = current_state.ledger {
                    fastpay_index::write_checkpoint_anchors(
                        &transaction,
                        ledger,
                        tip.height,
                        &self.integrity_key,
                        &self.counters,
                    )?;
                }
                return self.commit_durable_write(transaction);
            }
            return Err(StorageError::new(
                StorageErrorCode::InitializationConflict,
                "database is already initialized for a different retained checkpoint",
            ));
        }

        ensure_all_non_meta_tables_empty(&transaction)?;
        let meta_bytes = encode_json(&meta, MAX_META_BYTES)?;
        insert_authenticated(
            &mut meta_table,
            META_TABLE,
            META_KEY,
            &meta_bytes,
            MAX_META_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        drop(meta_table);
        write_current_state(
            &transaction,
            current_state,
            &self.integrity_key,
            &self.counters,
        )?;
        if let Some(ledger) = current_state.ledger {
            fastpay_index::write_checkpoint_anchors(
                &transaction,
                ledger,
                tip.height,
                &self.integrity_key,
                &self.counters,
            )?;
        }
        write_retained_ordered_prefix(
            &transaction,
            ordered_batches,
            &self.integrity_key,
            &self.counters,
        )?;
        self.commit_durable_write(transaction)
    }

    pub fn read_snapshot(&self) -> StorageResult<TransactionalReadSnapshot> {
        self.counters
            .read_transactions
            .fetch_add(1, Ordering::Relaxed);
        let transaction = self.database.begin_read().map_err(database_error)?;
        let table = transaction.open_table(META).map_err(database_error)?;
        let meta = read_meta(&table, &self.integrity_key, &self.counters)?;
        drop(table);
        Ok(TransactionalReadSnapshot {
            transaction,
            meta,
            integrity_key: self.integrity_key.clone(),
            counters: Arc::clone(&self.counters),
        })
    }

    pub fn meta(&self) -> StorageResult<TransactionalStoreMetaV1> {
        self.counters
            .read_transactions
            .fetch_add(1, Ordering::Relaxed);
        let transaction = self.database.begin_read().map_err(database_error)?;
        let table = transaction.open_table(META).map_err(database_error)?;
        read_meta(&table, &self.integrity_key, &self.counters)
    }

    pub fn contains_ordered_batch(&self, batch_id: &str) -> StorageResult<bool> {
        validate_identifier("batch id", batch_id)?;
        self.counters
            .read_transactions
            .fetch_add(1, Ordering::Relaxed);
        let transaction = self.database.begin_read().map_err(database_error)?;
        let table = transaction
            .open_table(ORDERED_BY_ID)
            .map_err(database_error)?;
        Ok(read_authenticated(
            &table,
            ORDERED_BY_ID_TABLE,
            batch_id.as_bytes(),
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?
        .is_some())
    }

    /// Resolve duplicate membership and the next accumulator from one read
    /// transaction, so proposal construction cannot combine two finalized
    /// tips even if a writer commits concurrently.
    pub fn next_ordered_history_commitment(
        &self,
        batch_id: &str,
    ) -> StorageResult<OrderedHistoryCommitment> {
        validate_identifier("batch id", batch_id)?;
        self.counters
            .read_transactions
            .fetch_add(1, Ordering::Relaxed);
        let transaction = self.database.begin_read().map_err(database_error)?;
        let meta_table = transaction.open_table(META).map_err(database_error)?;
        let meta = read_meta(&meta_table, &self.integrity_key, &self.counters)?;
        let ordered = transaction
            .open_table(ORDERED_BY_ID)
            .map_err(database_error)?;
        if read_authenticated(
            &ordered,
            ORDERED_BY_ID_TABLE,
            batch_id.as_bytes(),
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?
        .is_some()
        {
            return Err(StorageError::new(
                StorageErrorCode::DuplicateRecord,
                format!("ordered batch `{batch_id}` already exists"),
            ));
        }
        meta.ordered_history_commitment()
            .append(batch_id)
            .map_err(|error| {
                StorageError::new(
                    StorageErrorCode::OrderedCommitmentMismatch,
                    error.to_string(),
                )
            })
    }

    pub fn block(&self, height: u64) -> StorageResult<Option<BlockRecord>> {
        let key = ordered_u64_key(height);
        self.read_json_record(BLOCKS_BY_HEIGHT, BLOCKS_BY_HEIGHT_TABLE, &key)
    }

    pub fn block_height_by_hash(&self, block_hash: &str) -> StorageResult<Option<u64>> {
        validate_identifier("block hash", block_hash)?;
        let record: Option<StoredBlockHeightV1> = self.read_json_record(
            BLOCK_HEIGHT_BY_HASH,
            BLOCK_HEIGHT_BY_HASH_TABLE,
            block_hash.as_bytes(),
        )?;
        match record {
            Some(record) if record.schema == STORED_BLOCK_HEIGHT_SCHEMA => Ok(Some(record.height)),
            Some(_) => Err(StorageError::new(
                StorageErrorCode::UnsupportedSchema,
                "block hash index has an unsupported schema",
            )),
            None => Ok(None),
        }
    }

    pub fn receipt(&self, receipt_id: &str) -> StorageResult<Option<Receipt>> {
        validate_identifier("receipt id", receipt_id)?;
        let record: Option<StoredReceiptHistoryV1> =
            self.read_json_record(RECEIPTS_BY_ID, RECEIPTS_BY_ID_TABLE, receipt_id.as_bytes())?;
        match record {
            Some(record) => {
                validate_receipt_history(receipt_id, &record)?;
                Ok(record
                    .occurrences
                    .last()
                    .map(|occurrence| occurrence.receipt.clone()))
            }
            None => Ok(None),
        }
    }

    pub fn receipt_at_height(
        &self,
        receipt_id: &str,
        finalized_height: u64,
    ) -> StorageResult<Option<Receipt>> {
        validate_identifier("receipt id", receipt_id)?;
        let record: Option<StoredReceiptHistoryV1> =
            self.read_json_record(RECEIPTS_BY_ID, RECEIPTS_BY_ID_TABLE, receipt_id.as_bytes())?;
        let Some(record) = record else {
            return Ok(None);
        };
        validate_receipt_history(receipt_id, &record)?;
        Ok(record
            .occurrences
            .iter()
            .find(|occurrence| occurrence.finalized_height == finalized_height)
            .map(|occurrence| occurrence.receipt.clone()))
    }

    pub fn archived_batch(
        &self,
        batch_kind: &str,
        batch_id: &str,
    ) -> StorageResult<Option<BatchArchiveEntry>> {
        let key = archive_key(batch_kind, batch_id)?;
        self.read_json_record(BATCH_ARCHIVE, BATCH_ARCHIVE_TABLE, &key)
    }

    pub fn anchored_fastpay_effect(
        &self,
        lock_id: &str,
    ) -> StorageResult<Option<FastPayVersionFenceV1>> {
        let key = fastpay_anchor_key(lock_id)?;
        let record: Option<StoredFastPayAnchorV1> =
            self.read_json_record(HISTORY_INDEXES, HISTORY_INDEXES_TABLE, &key)?;
        match record {
            Some(record)
                if record.schema == STORED_FASTPAY_ANCHOR_SCHEMA
                    && record.finalized_height > 0
                    && record.effect.lock_id == lock_id =>
            {
                Ok(Some(record.effect))
            }
            Some(_) => Err(StorageError::new(
                StorageErrorCode::CorruptRecord,
                "FastPay anchor index has an invalid schema, height, or lock id",
            )),
            None => Ok(None),
        }
    }

    pub fn ordered_batch_by_ordinal(&self, ordinal: u64) -> StorageResult<Option<String>> {
        if ordinal == 0 {
            return Err(StorageError::new(
                StorageErrorCode::NonCanonicalKey,
                "ordered-batch ordinal is one-based",
            ));
        }
        let key = ordered_u64_key(ordinal);
        let record: Option<StoredOrderedOrdinalV1> =
            self.read_json_record(ORDERED_BY_ORDINAL, ORDERED_BY_ORDINAL_TABLE, &key)?;
        Ok(record.map(|record| record.batch_id))
    }

    pub fn ordered_range(&self, start_ordinal: u64, limit: usize) -> StorageResult<Vec<String>> {
        if start_ordinal == 0 || limit > MAX_RANGE_LIMIT {
            return Err(StorageError::new(
                StorageErrorCode::SizeLimit,
                "ordered range must start at one and stay within the range limit",
            ));
        }
        let meta = self.meta()?;
        let remaining = meta
            .ordered_batch_count
            .saturating_sub(start_ordinal.saturating_sub(1));
        let count = remaining.min(limit as u64);
        let mut output = Vec::with_capacity(count as usize);
        for offset in 0..count {
            let ordinal = start_ordinal.checked_add(offset).ok_or_else(|| {
                StorageError::new(StorageErrorCode::SizeLimit, "ordered range overflow")
            })?;
            let Some(batch_id) = self.ordered_batch_by_ordinal(ordinal)? else {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("ordered batch ordinal {ordinal} is missing"),
                ));
            };
            output.push(batch_id);
        }
        Ok(output)
    }

    pub fn current_state_raw(&self, domain: &str) -> StorageResult<Option<Vec<u8>>> {
        validate_state_domain(domain, false)?;
        self.counters
            .read_transactions
            .fetch_add(1, Ordering::Relaxed);
        let transaction = self.database.begin_read().map_err(database_error)?;
        let table = transaction
            .open_table(CURRENT_STATE)
            .map_err(database_error)?;
        read_authenticated(
            &table,
            CURRENT_STATE_TABLE,
            domain.as_bytes(),
            MAX_CURRENT_STATE_BYTES,
            &self.integrity_key,
            &self.counters,
        )
    }

    pub fn current_state<T: DeserializeOwned>(&self, domain: &str) -> StorageResult<Option<T>> {
        self.current_state_raw(domain)?
            .map(|bytes| decode_json(&bytes))
            .transpose()
    }

    pub fn ledger(&self) -> StorageResult<Option<LedgerState>> {
        self.current_state(STATE_LEDGER)
    }

    pub fn governance(&self) -> StorageResult<Option<GovernanceState>> {
        self.current_state(STATE_GOVERNANCE)
    }

    pub fn shielded(&self) -> StorageResult<Option<ShieldedState>> {
        self.current_state(STATE_SHIELDED)
    }

    pub fn bridge(&self) -> StorageResult<Option<BridgeState>> {
        self.current_state(STATE_BRIDGE)
    }

    pub fn node_state(&self) -> StorageResult<Option<NodeState>> {
        self.current_state(STATE_NODE)
    }

    fn record_full_history_scan<T: Serialize>(&self, values: &[T]) -> StorageResult<()> {
        let bytes = values.iter().try_fold(0_u64, |total, value| {
            let encoded = serde_json::to_vec(value).map_err(serialization_error)?;
            total.checked_add(encoded.len() as u64).ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::SizeLimit,
                    "full-history instrumentation byte count overflow",
                )
            })
        })?;
        self.counters
            .full_history_scans
            .fetch_add(1, Ordering::Relaxed);
        self.counters
            .full_history_records_read
            .fetch_add(values.len() as u64, Ordering::Relaxed);
        self.counters
            .full_history_bytes_read
            .fetch_add(bytes, Ordering::Relaxed);
        Ok(())
    }

    pub fn blocks_in_height_order(&self) -> StorageResult<Vec<BlockRecord>> {
        let meta = self.meta()?;
        let count = meta
            .finalized_height
            .checked_sub(meta.history_base_height)
            .ok_or_else(|| {
                StorageError::new(StorageErrorCode::CountMismatch, "invalid history range")
            })?;
        let capacity = usize::try_from(count).map_err(|_| {
            StorageError::new(StorageErrorCode::SizeLimit, "block range is too large")
        })?;
        let mut blocks = Vec::with_capacity(capacity);
        for height in meta.history_base_height.saturating_add(1)..=meta.finalized_height {
            let block = self.block(height)?.ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} is missing"),
                )
            })?;
            if block.header.height != height {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} has a mismatched embedded height"),
                ));
            }
            blocks.push(block);
        }
        self.record_full_history_scan(&blocks)?;
        Ok(blocks)
    }

    pub fn receipts_in_block_order(&self) -> StorageResult<Vec<Receipt>> {
        let meta = self.meta()?;
        let capacity = usize::try_from(meta.receipt_count).map_err(|_| {
            StorageError::new(StorageErrorCode::SizeLimit, "receipt range is too large")
        })?;
        let mut receipts = Vec::with_capacity(capacity);
        for block in self.blocks_in_height_order()? {
            for receipt_id in block.receipt_ids {
                let receipt = self
                    .receipt_at_height(&receipt_id, block.header.height)?
                    .ok_or_else(|| {
                        StorageError::new(
                            StorageErrorCode::CorruptRecord,
                            format!(
                                "canonical receipt `{receipt_id}` at height {} is missing",
                                block.header.height
                            ),
                        )
                    })?;
                receipts.push(receipt);
            }
        }
        if receipts.len() as u64 != meta.receipt_count {
            return Err(StorageError::new(
                StorageErrorCode::CountMismatch,
                "block-ordered receipt count does not match metadata",
            ));
        }
        self.record_full_history_scan(&receipts)?;
        Ok(receipts)
    }

    pub fn archived_batches_in_block_order(&self) -> StorageResult<Vec<BatchArchiveEntry>> {
        let mut archives = Vec::new();
        for block in self.blocks_in_height_order()? {
            let archive = self
                .archived_batch(&block.header.batch_kind, &block.header.batch_id)?
                .ok_or_else(|| {
                    StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!(
                            "archive for block {} batch `{}` is missing",
                            block.header.height, block.header.batch_id
                        ),
                    )
                })?;
            archives.push(archive);
        }
        self.record_full_history_scan(&archives)?;
        Ok(archives)
    }

    pub fn ordered_batches(&self) -> StorageResult<Vec<String>> {
        let meta = self.meta()?;
        let values = if meta.ordered_batch_count == 0 {
            Vec::new()
        } else {
            let count = usize::try_from(meta.ordered_batch_count).map_err(|_| {
                StorageError::new(
                    StorageErrorCode::SizeLimit,
                    "ordered-batch range is too large",
                )
            })?;
            self.ordered_range(1, count)?
        };
        self.record_full_history_scan(&values)?;
        Ok(values)
    }

    pub fn commit_finalized_block(
        &self,
        commit: CommitFinalizedBlock<'_>,
    ) -> StorageResult<CommitOutcome> {
        self.commit_finalized_block_with_precommit_hook(commit, || Ok(()))
    }

    /// Test and qualification hook invoked after every logical write is staged
    /// but before the database transaction is committed. Production callers
    /// use `commit_finalized_block`.
    #[doc(hidden)]
    pub fn commit_finalized_block_with_precommit_hook<F>(
        &self,
        commit: CommitFinalizedBlock<'_>,
        precommit_hook: F,
    ) -> StorageResult<CommitOutcome>
    where
        F: FnOnce() -> StorageResult<()>,
    {
        validate_commit_shape(&commit)?;
        let transaction = self.begin_durable_write()?;
        let mut meta_table = transaction.open_table(META).map_err(database_error)?;
        let stored_meta = read_meta(&meta_table, &self.integrity_key, &self.counters)?;

        if stored_meta.matches_tip(commit.new_tip) {
            drop(meta_table);
            verify_idempotent_commit(
                &transaction,
                &stored_meta,
                &commit,
                &self.integrity_key,
                &self.counters,
            )?;
            return Ok(CommitOutcome::AlreadyCommitted);
        }
        if !stored_meta.matches_tip(commit.expected_tip) {
            return Err(StorageError::new(
                StorageErrorCode::ExpectedTipMismatch,
                format!(
                    "stored tip {}:{} does not match expected {}:{}",
                    stored_meta.finalized_height,
                    stored_meta.finalized_block_hash,
                    commit.expected_tip.height,
                    commit.expected_tip.block_hash
                ),
            ));
        }

        validate_commit_against_meta(&stored_meta, &commit)?;
        ensure_commit_keys_absent(&transaction, &commit, &self.integrity_key, &self.counters)?;
        write_current_state(
            &transaction,
            commit.current_state,
            &self.integrity_key,
            &self.counters,
        )?;

        {
            let mut history_indexes = transaction
                .open_table(HISTORY_INDEXES)
                .map_err(database_error)?;
            for effect in &commit.block.fastpay_pre_state_effects {
                let record = StoredFastPayAnchorV1 {
                    schema: STORED_FASTPAY_ANCHOR_SCHEMA.to_owned(),
                    finalized_height: commit.new_tip.height,
                    effect: effect.clone(),
                };
                insert_authenticated(
                    &mut history_indexes,
                    HISTORY_INDEXES_TABLE,
                    &fastpay_anchor_key(&effect.lock_id)?,
                    &encode_json(&record, MAX_RECORD_BYTES)?,
                    MAX_RECORD_BYTES,
                    &self.integrity_key,
                    &self.counters,
                )?;
            }
        }

        {
            let mut receipts_table = transaction
                .open_table(RECEIPTS_BY_ID)
                .map_err(database_error)?;
            for (canonical_receipt_id, receipt) in
                commit.block.receipt_ids.iter().zip(commit.receipts)
            {
                let existing = read_authenticated(
                    &receipts_table,
                    RECEIPTS_BY_ID_TABLE,
                    canonical_receipt_id.as_bytes(),
                    MAX_RECORD_BYTES,
                    &self.integrity_key,
                    &self.counters,
                )?;
                let mut history = match existing {
                    Some(bytes) => {
                        let history: StoredReceiptHistoryV1 = decode_json(&bytes)?;
                        validate_receipt_history(canonical_receipt_id, &history)?;
                        validate_receipt_transition(&history, receipt)?;
                        history
                    }
                    None => StoredReceiptHistoryV1 {
                        schema: STORED_RECEIPT_HISTORY_SCHEMA.to_owned(),
                        occurrences: Vec::new(),
                    },
                };
                if history.occurrences.len() >= MAX_RECEIPT_OCCURRENCES_PER_ID {
                    return Err(StorageError::new(
                        StorageErrorCode::SizeLimit,
                        "receipt occurrence history exceeds its closed bound",
                    ));
                }
                history.occurrences.push(StoredReceiptOccurrenceV1 {
                    finalized_height: commit.new_tip.height,
                    receipt: receipt.clone(),
                });
                let bytes = encode_json(&history, MAX_RECORD_BYTES)?;
                insert_authenticated(
                    &mut receipts_table,
                    RECEIPTS_BY_ID_TABLE,
                    canonical_receipt_id.as_bytes(),
                    &bytes,
                    MAX_RECORD_BYTES,
                    &self.integrity_key,
                    &self.counters,
                )?;
            }
        }
        {
            let archive_key = archive_key(
                &commit.archive_entry.batch_kind,
                &commit.archive_entry.batch_id,
            )?;
            let archive_bytes = encode_json(commit.archive_entry, MAX_RECORD_BYTES)?;
            let mut archive_table = transaction
                .open_table(BATCH_ARCHIVE)
                .map_err(database_error)?;
            insert_authenticated(
                &mut archive_table,
                BATCH_ARCHIVE_TABLE,
                &archive_key,
                &archive_bytes,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?;
        }
        {
            let ordered_id = StoredOrderedIdV1 {
                schema: STORED_ORDERED_ID_SCHEMA.to_owned(),
                ordinal: commit.ordered_history.count,
                finalized_height: commit.new_tip.height,
            };
            let ordered_id_bytes = encode_json(&ordered_id, MAX_RECORD_BYTES)?;
            let mut ordered_by_id = transaction
                .open_table(ORDERED_BY_ID)
                .map_err(database_error)?;
            insert_authenticated(
                &mut ordered_by_id,
                ORDERED_BY_ID_TABLE,
                commit.batch_id.as_bytes(),
                &ordered_id_bytes,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?;

            let ordinal_key = ordered_u64_key(commit.ordered_history.count);
            let ordered_ordinal = StoredOrderedOrdinalV1 {
                schema: STORED_ORDERED_ORDINAL_SCHEMA.to_owned(),
                batch_id: commit.batch_id.to_owned(),
            };
            let ordered_ordinal_bytes = encode_json(&ordered_ordinal, MAX_RECORD_BYTES)?;
            let mut ordered_by_ordinal = transaction
                .open_table(ORDERED_BY_ORDINAL)
                .map_err(database_error)?;
            insert_authenticated(
                &mut ordered_by_ordinal,
                ORDERED_BY_ORDINAL_TABLE,
                &ordinal_key,
                &ordered_ordinal_bytes,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?;
        }
        {
            let height_key = ordered_u64_key(commit.new_tip.height);
            let block_bytes = encode_json(commit.block, MAX_RECORD_BYTES)?;
            let mut blocks = transaction
                .open_table(BLOCKS_BY_HEIGHT)
                .map_err(database_error)?;
            insert_authenticated(
                &mut blocks,
                BLOCKS_BY_HEIGHT_TABLE,
                &height_key,
                &block_bytes,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?;

            let height_value = StoredBlockHeightV1 {
                schema: STORED_BLOCK_HEIGHT_SCHEMA.to_owned(),
                height: commit.new_tip.height,
            };
            let height_bytes = encode_json(&height_value, MAX_RECORD_BYTES)?;
            let mut block_hashes = transaction
                .open_table(BLOCK_HEIGHT_BY_HASH)
                .map_err(database_error)?;
            insert_authenticated(
                &mut block_hashes,
                BLOCK_HEIGHT_BY_HASH_TABLE,
                commit.new_tip.block_hash.as_bytes(),
                &height_bytes,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?;
        }

        let mut new_meta = TransactionalStoreMetaV1::from_tip_and_commitment(
            commit.new_tip,
            commit.ordered_history,
        );
        new_meta.scheduled_activation_height = commit.scheduled_activation_height;
        new_meta.last_full_verification_height = stored_meta.last_full_verification_height;
        new_meta.migration_packet_root = stored_meta.migration_packet_root;
        new_meta.verifier_version = stored_meta.verifier_version;
        new_meta.validate()?;
        let meta_bytes = encode_json(&new_meta, MAX_META_BYTES)?;
        insert_authenticated(
            &mut meta_table,
            META_TABLE,
            META_KEY,
            &meta_bytes,
            MAX_META_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        drop(meta_table);
        precommit_hook()?;

        self.commit_durable_write(transaction)?;
        Ok(CommitOutcome::Committed)
    }

    /// Atomically discard the authenticated block/archive/receipt prefix while
    /// retaining the ordered-batch commitment and a portable checkpoint that
    /// binds the removed history. This is a maintenance transaction: callers
    /// must already have verified and archived the prefix.
    pub fn prune_retained_history(
        &self,
        prune: PruneRetainedHistory<'_>,
    ) -> StorageResult<PruneOutcome> {
        validate_tip_domain(prune.expected_tip)?;
        if prune.retained_checkpoint.len() > MAX_CURRENT_STATE_BYTES {
            return Err(StorageError::new(
                StorageErrorCode::SizeLimit,
                "retained-history checkpoint exceeds its closed size bound",
            ));
        }
        let checkpoint: RetainedCheckpointBindingV2 = decode_json(prune.retained_checkpoint)?;
        if checkpoint.schema != "postfiat-history-checkpoint-v2"
            || checkpoint.pruned_up_to_height != prune.new_history_base_height
        {
            return Err(StorageError::new(
                StorageErrorCode::DomainMismatch,
                "retained-history checkpoint does not bind the requested prune boundary",
            ));
        }
        if prune.new_history_base_height <= prune.expected_tip.history_base_height
            || prune.new_history_base_height > prune.expected_tip.height
        {
            return Err(StorageError::new(
                StorageErrorCode::CountMismatch,
                "new retained-history base must advance the current base without passing the tip",
            ));
        }

        // A prune is intentionally rare and may spend O(retained history) work.
        // Refuse to transform an already inconsistent logical store.
        self.verify_logical_integrity()?;

        let transaction = self.begin_durable_write()?;
        let mut meta_table = transaction.open_table(META).map_err(database_error)?;
        let mut meta = read_meta(&meta_table, &self.integrity_key, &self.counters)?;
        if !meta.matches_tip(prune.expected_tip) {
            return Err(StorageError::new(
                StorageErrorCode::ExpectedTipMismatch,
                "transactional store tip changed before retained-history prune",
            ));
        }
        let previous_history_base_height = meta.history_base_height;
        let mut pruned_blocks = Vec::new();
        {
            let blocks = transaction
                .open_table(BLOCKS_BY_HEIGHT)
                .map_err(database_error)?;
            let block_hashes = transaction
                .open_table(BLOCK_HEIGHT_BY_HASH)
                .map_err(database_error)?;
            let archives = transaction
                .open_table(BATCH_ARCHIVE)
                .map_err(database_error)?;
            let receipts = transaction
                .open_table(RECEIPTS_BY_ID)
                .map_err(database_error)?;
            for height in
                previous_history_base_height.saturating_add(1)..=prune.new_history_base_height
            {
                let block: BlockRecord = required_json_record(
                    &blocks,
                    BLOCKS_BY_HEIGHT_TABLE,
                    &ordered_u64_key(height),
                    &self.integrity_key,
                    &self.counters,
                )?;
                if block.header.height != height {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!("pruned block {height} has a mismatched embedded height"),
                    ));
                }
                let hash_index: StoredBlockHeightV1 = required_json_record(
                    &block_hashes,
                    BLOCK_HEIGHT_BY_HASH_TABLE,
                    block.header.block_hash.as_bytes(),
                    &self.integrity_key,
                    &self.counters,
                )?;
                if hash_index.schema != STORED_BLOCK_HEIGHT_SCHEMA || hash_index.height != height {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!("pruned block {height} has a conflicting hash index"),
                    ));
                }
                let archive: BatchArchiveEntry = required_json_record(
                    &archives,
                    BATCH_ARCHIVE_TABLE,
                    &archive_key(&block.header.batch_kind, &block.header.batch_id)?,
                    &self.integrity_key,
                    &self.counters,
                )?;
                if archive.batch_kind != block.header.batch_kind
                    || archive.batch_id != block.header.batch_id
                {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!("pruned block {height} has a conflicting archive entry"),
                    ));
                }
                for receipt_id in &block.receipt_ids {
                    let history: StoredReceiptHistoryV1 = required_json_record(
                        &receipts,
                        RECEIPTS_BY_ID_TABLE,
                        receipt_id.as_bytes(),
                        &self.integrity_key,
                        &self.counters,
                    )?;
                    validate_receipt_history(receipt_id, &history)?;
                    if history
                        .occurrences
                        .iter()
                        .filter(|occurrence| occurrence.finalized_height == height)
                        .count()
                        != 1
                    {
                        return Err(StorageError::new(
                            StorageErrorCode::CorruptRecord,
                            format!(
                                "pruned block {height} does not have one literal receipt occurrence for `{receipt_id}`"
                            ),
                        ));
                    }
                }
                pruned_blocks.push(block);
            }
        }

        {
            let mut blocks = transaction
                .open_table(BLOCKS_BY_HEIGHT)
                .map_err(database_error)?;
            for block in &pruned_blocks {
                if blocks
                    .remove(ordered_u64_key(block.header.height).as_slice())
                    .map_err(database_error)?
                    .is_none()
                {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        "authenticated block disappeared during prune transaction",
                    ));
                }
                self.counters.page_writes.fetch_add(1, Ordering::Relaxed);
                self.counters
                    .records_written
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        {
            let mut block_hashes = transaction
                .open_table(BLOCK_HEIGHT_BY_HASH)
                .map_err(database_error)?;
            for block in &pruned_blocks {
                if block_hashes
                    .remove(block.header.block_hash.as_bytes())
                    .map_err(database_error)?
                    .is_none()
                {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        "authenticated block hash index disappeared during prune transaction",
                    ));
                }
                self.counters.page_writes.fetch_add(1, Ordering::Relaxed);
                self.counters
                    .records_written
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        {
            let mut archives = transaction
                .open_table(BATCH_ARCHIVE)
                .map_err(database_error)?;
            for block in &pruned_blocks {
                let key = archive_key(&block.header.batch_kind, &block.header.batch_id)?;
                if archives
                    .remove(key.as_slice())
                    .map_err(database_error)?
                    .is_none()
                {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        "authenticated archive entry disappeared during prune transaction",
                    ));
                }
                self.counters.page_writes.fetch_add(1, Ordering::Relaxed);
                self.counters
                    .records_written
                    .fetch_add(1, Ordering::Relaxed);
            }
        }

        let receipt_ids = pruned_blocks
            .iter()
            .flat_map(|block| block.receipt_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        let mut pruned_receipt_count = 0_u64;
        {
            let mut receipts = transaction
                .open_table(RECEIPTS_BY_ID)
                .map_err(database_error)?;
            for receipt_id in receipt_ids {
                let raw = read_authenticated(
                    &receipts,
                    RECEIPTS_BY_ID_TABLE,
                    receipt_id.as_bytes(),
                    MAX_RECORD_BYTES,
                    &self.integrity_key,
                    &self.counters,
                )?
                .ok_or_else(|| {
                    StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        "authenticated receipt disappeared during prune transaction",
                    )
                })?;
                let mut history: StoredReceiptHistoryV1 = decode_json(&raw)?;
                validate_receipt_history(&receipt_id, &history)?;
                let before = history.occurrences.len();
                history.occurrences.retain(|occurrence| {
                    occurrence.finalized_height > prune.new_history_base_height
                });
                let removed = before
                    .checked_sub(history.occurrences.len())
                    .ok_or_else(|| {
                        StorageError::new(
                            StorageErrorCode::CountMismatch,
                            "receipt prune underflow",
                        )
                    })?;
                pruned_receipt_count = pruned_receipt_count
                    .checked_add(removed as u64)
                    .ok_or_else(|| {
                        StorageError::new(
                            StorageErrorCode::CountMismatch,
                            "pruned receipt count overflow",
                        )
                    })?;
                if history.occurrences.is_empty() {
                    receipts
                        .remove(receipt_id.as_bytes())
                        .map_err(database_error)?;
                    self.counters.page_writes.fetch_add(1, Ordering::Relaxed);
                    self.counters
                        .records_written
                        .fetch_add(1, Ordering::Relaxed);
                } else {
                    let bytes = encode_json(&history, MAX_RECORD_BYTES)?;
                    insert_authenticated(
                        &mut receipts,
                        RECEIPTS_BY_ID_TABLE,
                        receipt_id.as_bytes(),
                        &bytes,
                        MAX_RECORD_BYTES,
                        &self.integrity_key,
                        &self.counters,
                    )?;
                }
            }
        }

        // These indexes are derived acceleration structures. Clearing them in
        // the same transaction is safer than retaining entries into the
        // discarded prefix; online reads remain correct while they rebuild.
        {
            let mut indexes = transaction
                .open_table(HISTORY_INDEXES)
                .map_err(database_error)?;
            let keys = indexes
                .iter()
                .map_err(database_error)?
                .map(|entry| {
                    entry
                        .map(|(key, _)| key.value().to_vec())
                        .map_err(database_error)
                })
                .collect::<StorageResult<Vec<_>>>()?;
            for key in keys {
                indexes.remove(key.as_slice()).map_err(database_error)?;
                self.counters.page_writes.fetch_add(1, Ordering::Relaxed);
                self.counters
                    .records_written
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        if let Some(ledger) = &checkpoint.ledger {
            fastpay_index::write_checkpoint_anchors(
                &transaction,
                ledger,
                prune.new_history_base_height,
                &self.integrity_key,
                &self.counters,
            )?;
        }
        {
            let blocks = transaction
                .open_table(BLOCKS_BY_HEIGHT)
                .map_err(database_error)?;
            for height in prune.new_history_base_height.saturating_add(1)..=meta.finalized_height {
                let block: BlockRecord = required_json_record(
                    &blocks,
                    BLOCKS_BY_HEIGHT_TABLE,
                    &ordered_u64_key(height),
                    &self.integrity_key,
                    &self.counters,
                )?;
                fastpay_index::write_block_anchors(
                    &transaction,
                    &block,
                    &self.integrity_key,
                    &self.counters,
                )?;
            }
        }
        {
            let mut current_state = transaction
                .open_table(CURRENT_STATE)
                .map_err(database_error)?;
            insert_authenticated(
                &mut current_state,
                CURRENT_STATE_TABLE,
                b"retained_history_checkpoint",
                prune.retained_checkpoint,
                MAX_CURRENT_STATE_BYTES,
                &self.integrity_key,
                &self.counters,
            )?;
        }

        let pruned_block_count = u64::try_from(pruned_blocks.len()).map_err(|_| {
            StorageError::new(
                StorageErrorCode::CountMismatch,
                "pruned block count overflow",
            )
        })?;
        meta.history_base_height = prune.new_history_base_height;
        meta.receipt_count = meta
            .receipt_count
            .checked_sub(pruned_receipt_count)
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CountMismatch,
                    "pruned receipt count exceeds metadata",
                )
            })?;
        meta.last_full_verification_height = None;
        meta.validate()?;
        insert_authenticated(
            &mut meta_table,
            META_TABLE,
            META_KEY,
            &encode_json(&meta, MAX_META_BYTES)?,
            MAX_META_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        drop(meta_table);
        self.commit_durable_write(transaction)?;
        self.verify_and_mark_full_integrity()?;

        Ok(PruneOutcome {
            previous_history_base_height,
            new_history_base_height: prune.new_history_base_height,
            pruned_block_count,
            pruned_archive_count: pruned_block_count,
            pruned_receipt_count,
            remaining_receipt_count: meta.receipt_count,
        })
    }

    pub fn verify_logical_integrity(&self) -> StorageResult<LogicalIntegrityReport> {
        self.counters
            .read_transactions
            .fetch_add(1, Ordering::Relaxed);
        let transaction = self.database.begin_read().map_err(database_error)?;
        let meta_table = transaction.open_table(META).map_err(database_error)?;
        let meta = read_meta(&meta_table, &self.integrity_key, &self.counters)?;
        drop(meta_table);

        let blocks = transaction
            .open_table(BLOCKS_BY_HEIGHT)
            .map_err(database_error)?;
        let block_hashes = transaction
            .open_table(BLOCK_HEIGHT_BY_HASH)
            .map_err(database_error)?;
        let receipts = transaction
            .open_table(RECEIPTS_BY_ID)
            .map_err(database_error)?;
        let archives = transaction
            .open_table(BATCH_ARCHIVE)
            .map_err(database_error)?;
        let ordered_ids = transaction
            .open_table(ORDERED_BY_ID)
            .map_err(database_error)?;
        let ordered_ordinals = transaction
            .open_table(ORDERED_BY_ORDINAL)
            .map_err(database_error)?;
        let current_state = transaction
            .open_table(CURRENT_STATE)
            .map_err(database_error)?;
        let history_indexes = transaction
            .open_table(HISTORY_INDEXES)
            .map_err(database_error)?;

        verify_every_record(
            &blocks,
            BLOCKS_BY_HEIGHT_TABLE,
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        verify_every_record(
            &block_hashes,
            BLOCK_HEIGHT_BY_HASH_TABLE,
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        verify_every_record(
            &receipts,
            RECEIPTS_BY_ID_TABLE,
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        verify_every_record(
            &archives,
            BATCH_ARCHIVE_TABLE,
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        verify_every_record(
            &ordered_ids,
            ORDERED_BY_ID_TABLE,
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        verify_every_record(
            &ordered_ordinals,
            ORDERED_BY_ORDINAL_TABLE,
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        verify_every_record(
            &current_state,
            CURRENT_STATE_TABLE,
            MAX_CURRENT_STATE_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        verify_every_record(
            &history_indexes,
            HISTORY_INDEXES_TABLE,
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;

        let expected_block_count = meta
            .finalized_height
            .checked_sub(meta.history_base_height)
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CountMismatch,
                    "finalized height is below the retained-history base",
                )
            })?;
        if blocks.len().map_err(database_error)? != expected_block_count
            || block_hashes.len().map_err(database_error)? != expected_block_count
            || archives.len().map_err(database_error)? != expected_block_count
        {
            return Err(StorageError::new(
                StorageErrorCode::CountMismatch,
                "block, block-hash, or archive table length does not match the retained history range",
            ));
        }

        let mut referenced_receipts = BTreeSet::<(String, u64)>::new();
        let mut expected_fastpay_anchors = BTreeMap::<String, StoredFastPayAnchorV1>::new();
        let mut previous_block_hash: Option<String> = None;
        for height in meta.history_base_height.saturating_add(1)..=meta.finalized_height {
            let height_key = ordered_u64_key(height);
            let block_raw = read_authenticated(
                &blocks,
                BLOCKS_BY_HEIGHT_TABLE,
                &height_key,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} is missing"),
                )
            })?;
            let block: BlockRecord = decode_json(&block_raw)?;
            if block.header.height != height
                || block.header.block_hash.is_empty()
                || block.header.batch_id.is_empty()
                || block.header.receipt_count != block.receipt_ids.len() as u64
                || previous_block_hash
                    .as_ref()
                    .is_some_and(|parent| block.header.parent_hash != *parent)
            {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} has inconsistent header or receipt fields"),
                ));
            }
            validate_identifier("block hash", &block.header.block_hash)?;
            validate_identifier("batch kind", &block.header.batch_kind)?;
            validate_identifier("batch id", &block.header.batch_id)?;
            for effect in &block.fastpay_pre_state_effects {
                effect.validate_shape().map_err(|error| {
                    StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!("canonical block {height} has an invalid FastPay effect: {error}"),
                    )
                })?;
                let record = StoredFastPayAnchorV1 {
                    schema: STORED_FASTPAY_ANCHOR_SCHEMA.to_owned(),
                    finalized_height: height,
                    effect: effect.clone(),
                };
                if expected_fastpay_anchors
                    .insert(effect.lock_id.clone(), record)
                    .is_some()
                {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!("canonical block {height} duplicates a retained FastPay anchor"),
                    ));
                }
            }

            let hash_raw = read_authenticated(
                &block_hashes,
                BLOCK_HEIGHT_BY_HASH_TABLE,
                block.header.block_hash.as_bytes(),
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} has no hash index"),
                )
            })?;
            let hash_record: StoredBlockHeightV1 = decode_json(&hash_raw)?;
            if hash_record.schema != STORED_BLOCK_HEIGHT_SCHEMA || hash_record.height != height {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} has a conflicting hash index"),
                ));
            }

            let ordinal_raw = read_authenticated(
                &ordered_ordinals,
                ORDERED_BY_ORDINAL_TABLE,
                &ordered_u64_key(height),
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} has no ordered ordinal"),
                )
            })?;
            let ordinal_record: StoredOrderedOrdinalV1 = decode_json(&ordinal_raw)?;
            validate_stored_ordered_ordinal(&ordinal_record)?;
            if ordinal_record.batch_id != block.header.batch_id {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} conflicts with its ordered ordinal"),
                ));
            }
            let id_raw = read_authenticated(
                &ordered_ids,
                ORDERED_BY_ID_TABLE,
                block.header.batch_id.as_bytes(),
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} has no ordered ID index"),
                )
            })?;
            let id_record: StoredOrderedIdV1 = decode_json(&id_raw)?;
            if id_record.schema != STORED_ORDERED_ID_SCHEMA
                || id_record.ordinal != height
                || id_record.finalized_height != height
            {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} conflicts with its ordered ID index"),
                ));
            }

            let archive_key = archive_key(&block.header.batch_kind, &block.header.batch_id)?;
            let archive_raw = read_authenticated(
                &archives,
                BATCH_ARCHIVE_TABLE,
                &archive_key,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} has no archived batch"),
                )
            })?;
            let archive: BatchArchiveEntry = decode_json(&archive_raw)?;
            if archive.batch_kind != block.header.batch_kind
                || archive.batch_id != block.header.batch_id
                || archive.payload_hash.is_empty()
            {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical block {height} conflicts with its archived batch"),
                ));
            }

            let mut block_receipt_ids = BTreeSet::new();
            for receipt_id in &block.receipt_ids {
                validate_identifier("receipt id", receipt_id)?;
                if !block_receipt_ids.insert(receipt_id.as_str())
                    || !referenced_receipts.insert((receipt_id.clone(), height))
                {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!("canonical block {height} has duplicate receipt references"),
                    ));
                }
                let receipt_raw = read_authenticated(
                    &receipts,
                    RECEIPTS_BY_ID_TABLE,
                    receipt_id.as_bytes(),
                    MAX_RECORD_BYTES,
                    &self.integrity_key,
                    &self.counters,
                )?
                .ok_or_else(|| {
                    StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!("canonical block {height} receipt `{receipt_id}` is missing"),
                    )
                })?;
                let receipt_history: StoredReceiptHistoryV1 = decode_json(&receipt_raw)?;
                validate_receipt_history(receipt_id, &receipt_history)?;
                if receipt_history
                    .occurrences
                    .iter()
                    .filter(|occurrence| occurrence.finalized_height == height)
                    .count()
                    != 1
                {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!(
                            "canonical block {height} receipt `{receipt_id}` has no unique occurrence"
                        ),
                    ));
                }
            }
            previous_block_hash = Some(block.header.block_hash);
        }

        let ledger_raw = read_authenticated(
            &current_state,
            CURRENT_STATE_TABLE,
            STATE_LEDGER.as_bytes(),
            MAX_CURRENT_STATE_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        if let Some(ledger_raw) = ledger_raw {
            let ledger: LedgerState = decode_json(&ledger_raw)?;
            for effect in ledger.fastpay_version_fences.iter().filter(|effect| {
                effect.origin == postfiat_types::FastPayFenceOriginV1::Consensusless
                    && matches!(
                        effect.decision,
                        postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. }
                    )
            }) {
                effect.validate_shape().map_err(|error| {
                    StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!("current ledger has an invalid FastPay effect: {error}"),
                    )
                })?;
                if let Some(record) = expected_fastpay_anchors.get(&effect.lock_id) {
                    if record.effect != *effect {
                        return Err(StorageError::new(
                            StorageErrorCode::CorruptRecord,
                            "retained block and current ledger FastPay effects conflict",
                        ));
                    }
                    continue;
                }
                if meta.history_base_height == 0 {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        "current ledger FastPay effect has no canonical block",
                    ));
                }
                let record = StoredFastPayAnchorV1 {
                    schema: STORED_FASTPAY_ANCHOR_SCHEMA.to_owned(),
                    finalized_height: meta.history_base_height,
                    effect: effect.clone(),
                };
                if expected_fastpay_anchors
                    .insert(effect.lock_id.clone(), record)
                    .is_some()
                {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        "current ledger contains a duplicate FastPay anchor",
                    ));
                }
            }
        }
        if history_indexes.len().map_err(database_error)? != expected_fastpay_anchors.len() as u64 {
            return Err(StorageError::new(
                StorageErrorCode::CountMismatch,
                "history-index entries do not match canonical FastPay effects",
            ));
        }
        for (lock_id, expected) in &expected_fastpay_anchors {
            let raw = read_authenticated(
                &history_indexes,
                HISTORY_INDEXES_TABLE,
                &fastpay_anchor_key(lock_id)?,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("FastPay anchor `{lock_id}` is missing from the history index"),
                )
            })?;
            let observed: StoredFastPayAnchorV1 = decode_json(&raw)?;
            if observed != *expected {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("FastPay anchor `{lock_id}` conflicts with canonical history"),
                ));
            }
        }

        for entry in receipts.iter().map_err(database_error)? {
            let (key, value) = entry.map_err(database_error)?;
            let receipt_id = std::str::from_utf8(key.value()).map_err(|_| {
                StorageError::new(
                    StorageErrorCode::NonCanonicalKey,
                    "receipt table contains a non-UTF-8 key",
                )
            })?;
            let payload = decode_authenticated_value(
                RECEIPTS_BY_ID_TABLE,
                key.value(),
                value.value(),
                MAX_RECORD_BYTES,
                &self.integrity_key,
            )?;
            let history: StoredReceiptHistoryV1 = decode_json(&payload)?;
            validate_receipt_history(receipt_id, &history)?;
            for occurrence in history.occurrences {
                if !referenced_receipts
                    .contains(&(receipt_id.to_owned(), occurrence.finalized_height))
                {
                    return Err(StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!(
                            "receipt `{receipt_id}` at height {} has no canonical block reference",
                            occurrence.finalized_height
                        ),
                    ));
                }
            }
        }

        if ordered_ids.len().map_err(database_error)? != meta.ordered_batch_count
            || ordered_ordinals.len().map_err(database_error)? != meta.ordered_batch_count
        {
            return Err(StorageError::new(
                StorageErrorCode::CountMismatch,
                "logical table lengths do not match metadata counts",
            ));
        }

        let mut commitment = OrderedHistoryCommitment::genesis(
            &meta.chain_id,
            &meta.genesis_hash,
            meta.protocol_version,
        )
        .map_err(|error| StorageError::new(StorageErrorCode::CorruptRecord, error.to_string()))?;
        for ordinal in 1..=meta.ordered_batch_count {
            let key = ordered_u64_key(ordinal);
            let raw = read_authenticated(
                &ordered_ordinals,
                ORDERED_BY_ORDINAL_TABLE,
                &key,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("ordered ordinal {ordinal} is missing"),
                )
            })?;
            let record: StoredOrderedOrdinalV1 = decode_json(&raw)?;
            validate_stored_ordered_ordinal(&record)?;
            let id_raw = read_authenticated(
                &ordered_ids,
                ORDERED_BY_ID_TABLE,
                record.batch_id.as_bytes(),
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("ordered batch `{}` has no ID index", record.batch_id),
                )
            })?;
            let id_record: StoredOrderedIdV1 = decode_json(&id_raw)?;
            if id_record.schema != STORED_ORDERED_ID_SCHEMA
                || id_record.ordinal != ordinal
                || id_record.finalized_height != ordinal
            {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    "ordered ID and ordinal indexes conflict",
                ));
            }
            commitment = commitment.append(&record.batch_id).map_err(|error| {
                StorageError::new(StorageErrorCode::CorruptRecord, error.to_string())
            })?;
        }
        if commitment != meta.ordered_history_commitment() {
            return Err(StorageError::new(
                StorageErrorCode::OrderedCommitmentMismatch,
                "recomputed ordered-history commitment does not match metadata",
            ));
        }

        if meta.finalized_height > meta.history_base_height {
            let tip_key = ordered_u64_key(meta.finalized_height);
            let tip_raw = read_authenticated(
                &blocks,
                BLOCKS_BY_HEIGHT_TABLE,
                &tip_key,
                MAX_RECORD_BYTES,
                &self.integrity_key,
                &self.counters,
            )?
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    "finalized block is missing",
                )
            })?;
            let tip_block: BlockRecord = decode_json(&tip_raw)?;
            if tip_block.header.block_hash != meta.finalized_block_hash
                || tip_block.header.state_root != meta.finalized_state_root
            {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    "finalized block does not match metadata tip",
                ));
            }
        }

        let receipt_occurrence_count =
            receipt_occurrence_count(&receipts, &self.integrity_key, &self.counters)?;
        if receipt_occurrence_count != meta.receipt_count {
            return Err(StorageError::new(
                StorageErrorCode::CountMismatch,
                "literal receipt occurrence count does not match metadata",
            ));
        }

        Ok(LogicalIntegrityReport {
            schema: LOGICAL_INTEGRITY_REPORT_SCHEMA.to_owned(),
            storage_format: meta.storage_format,
            backend: meta.backend,
            finalized_height: meta.finalized_height,
            block_count: blocks.len().map_err(database_error)?,
            receipt_count: receipt_occurrence_count,
            archive_count: archives.len().map_err(database_error)?,
            ordered_batch_count: commitment.count,
            history_index_count: history_indexes.len().map_err(database_error)?,
            accumulator: commitment.accumulator,
        })
    }

    /// Persist that a complete authenticated logical scan succeeded at the
    /// exact current tip. If a writer advanced the database between the scan
    /// and this marker transaction, the operation fails instead of blessing a
    /// stale verification result.
    pub fn verify_and_mark_full_integrity(&self) -> StorageResult<LogicalIntegrityReport> {
        let report = self.verify_logical_integrity()?;
        self.write_full_verification_marker(&report, None)?;
        Ok(report)
    }

    pub fn verify_and_bind_migration(
        &self,
        migration_packet_root: &str,
    ) -> StorageResult<LogicalIntegrityReport> {
        if migration_packet_root.len() != 96
            || !migration_packet_root
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StorageError::new(
                StorageErrorCode::NonCanonicalKey,
                "migration packet root must be a lowercase SHA3-384 digest",
            ));
        }
        let report = self.verify_logical_integrity()?;
        self.write_full_verification_marker(&report, Some(migration_packet_root))?;
        Ok(report)
    }

    fn write_full_verification_marker(
        &self,
        report: &LogicalIntegrityReport,
        migration_packet_root: Option<&str>,
    ) -> StorageResult<()> {
        let transaction = self.begin_durable_write()?;
        let mut meta_table = transaction.open_table(META).map_err(database_error)?;
        let mut meta = read_meta(&meta_table, &self.integrity_key, &self.counters)?;
        if meta.finalized_height != report.finalized_height
            || meta.ordered_batch_count != report.ordered_batch_count
            || meta.ordered_history_accumulator != report.accumulator
        {
            return Err(StorageError::new(
                StorageErrorCode::ExpectedTipMismatch,
                "transactional store advanced during full logical verification",
            ));
        }
        meta.last_full_verification_height = Some(report.finalized_height);
        if let Some(root) = migration_packet_root {
            meta.migration_packet_root = Some(root.to_owned());
            meta.verifier_version = Some(TRANSACTIONAL_VERIFIER_VERSION.to_owned());
        }
        meta.validate()?;
        let meta_bytes = encode_json(&meta, MAX_META_BYTES)?;
        insert_authenticated(
            &mut meta_table,
            META_TABLE,
            META_KEY,
            &meta_bytes,
            MAX_META_BYTES,
            &self.integrity_key,
            &self.counters,
        )?;
        drop(meta_table);
        self.commit_durable_write(transaction)?;
        Ok(())
    }

    pub fn check_database_integrity(&mut self) -> StorageResult<bool> {
        match &mut self.database {
            TransactionalDatabase::ReadWrite(database) => {
                database.check_integrity().map_err(database_error)
            }
            TransactionalDatabase::ReadOnly(_) => Err(StorageError::new(
                StorageErrorCode::Database,
                "storage_transactional_read_only_write_refused: database integrity repair requires a writable store",
            )),
        }
    }

    fn read_json_record<T: DeserializeOwned>(
        &self,
        definition: TableDefinition<&[u8], &[u8]>,
        table_name: &str,
        key: &[u8],
    ) -> StorageResult<Option<T>> {
        self.counters
            .read_transactions
            .fetch_add(1, Ordering::Relaxed);
        let transaction = self.database.begin_read().map_err(database_error)?;
        let table = transaction.open_table(definition).map_err(database_error)?;
        read_authenticated(
            &table,
            table_name,
            key,
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?
        .map(|raw| decode_json(&raw))
        .transpose()
    }

    fn begin_durable_write(&self) -> StorageResult<redb::WriteTransaction> {
        self.counters
            .write_transactions
            .fetch_add(1, Ordering::Relaxed);
        let TransactionalDatabase::ReadWrite(database) = &self.database else {
            return Err(StorageError::new(
                StorageErrorCode::Database,
                "storage_transactional_read_only_write_refused: write transaction requested from a read-only store",
            ));
        };
        let mut transaction = database.begin_write().map_err(database_error)?;
        transaction
            .set_durability(Durability::Immediate)
            .map_err(database_error)?;
        transaction.set_two_phase_commit(true);
        transaction.set_quick_repair(true);
        Ok(transaction)
    }

    fn commit_durable_write(&self, transaction: redb::WriteTransaction) -> StorageResult<()> {
        let started = Instant::now();
        transaction.commit().map_err(database_error)?;
        self.counters
            .committed_write_transactions
            .fetch_add(1, Ordering::Relaxed);
        self.counters.durable_commit_micros.fetch_add(
            started.elapsed().as_micros().min(u64::MAX as u128) as u64,
            Ordering::Relaxed,
        );
        Ok(())
    }
}

impl TransactionalReadSnapshot {
    pub fn meta(&self) -> &TransactionalStoreMetaV1 {
        &self.meta
    }

    pub fn chain_tip(&self) -> ChainTipState {
        self.meta.chain_tip("postfiat-chain-tip-v1")
    }

    pub fn contains_ordered_batch(&self, batch_id: &str) -> StorageResult<bool> {
        validate_identifier("batch id", batch_id)?;
        let table = self
            .transaction
            .open_table(ORDERED_BY_ID)
            .map_err(database_error)?;
        Ok(read_authenticated(
            &table,
            ORDERED_BY_ID_TABLE,
            batch_id.as_bytes(),
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?
        .is_some())
    }

    pub fn block(&self, height: u64) -> StorageResult<Option<BlockRecord>> {
        self.read_json_record(
            BLOCKS_BY_HEIGHT,
            BLOCKS_BY_HEIGHT_TABLE,
            &ordered_u64_key(height),
        )
    }

    pub fn receipt(&self, receipt_id: &str) -> StorageResult<Option<Receipt>> {
        validate_identifier("receipt id", receipt_id)?;
        let record: Option<StoredReceiptHistoryV1> =
            self.read_json_record(RECEIPTS_BY_ID, RECEIPTS_BY_ID_TABLE, receipt_id.as_bytes())?;
        match record {
            Some(record) => {
                validate_receipt_history(receipt_id, &record)?;
                Ok(record
                    .occurrences
                    .last()
                    .map(|occurrence| occurrence.receipt.clone()))
            }
            None => Ok(None),
        }
    }

    pub fn archived_batch(
        &self,
        batch_kind: &str,
        batch_id: &str,
    ) -> StorageResult<Option<BatchArchiveEntry>> {
        self.read_json_record(
            BATCH_ARCHIVE,
            BATCH_ARCHIVE_TABLE,
            &archive_key(batch_kind, batch_id)?,
        )
    }

    pub fn ordered_batch_by_ordinal(&self, ordinal: u64) -> StorageResult<Option<String>> {
        if ordinal == 0 {
            return Err(StorageError::new(
                StorageErrorCode::NonCanonicalKey,
                "ordered-batch ordinal is one-based",
            ));
        }
        let record: Option<StoredOrderedOrdinalV1> = self.read_json_record(
            ORDERED_BY_ORDINAL,
            ORDERED_BY_ORDINAL_TABLE,
            &ordered_u64_key(ordinal),
        )?;
        Ok(record.map(|record| record.batch_id))
    }

    pub fn ordered_range(&self, start_ordinal: u64, limit: usize) -> StorageResult<Vec<String>> {
        if start_ordinal == 0 || limit > MAX_RANGE_LIMIT {
            return Err(StorageError::new(
                StorageErrorCode::SizeLimit,
                "ordered range must start at one and stay within the range limit",
            ));
        }
        let remaining = self
            .meta
            .ordered_batch_count
            .saturating_sub(start_ordinal.saturating_sub(1));
        let count = remaining.min(limit as u64);
        let mut output = Vec::with_capacity(count as usize);
        for offset in 0..count {
            let ordinal = start_ordinal.checked_add(offset).ok_or_else(|| {
                StorageError::new(StorageErrorCode::SizeLimit, "ordered range overflow")
            })?;
            let batch_id = self.ordered_batch_by_ordinal(ordinal)?.ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("ordered batch ordinal {ordinal} is missing"),
                )
            })?;
            output.push(batch_id);
        }
        Ok(output)
    }

    pub fn current_state_raw(&self, domain: &str) -> StorageResult<Option<Vec<u8>>> {
        validate_state_domain(domain, false)?;
        let table = self
            .transaction
            .open_table(CURRENT_STATE)
            .map_err(database_error)?;
        read_authenticated(
            &table,
            CURRENT_STATE_TABLE,
            domain.as_bytes(),
            MAX_CURRENT_STATE_BYTES,
            &self.integrity_key,
            &self.counters,
        )
    }

    pub fn current_state<T: DeserializeOwned>(&self, domain: &str) -> StorageResult<Option<T>> {
        self.current_state_raw(domain)?
            .map(|bytes| decode_json(&bytes))
            .transpose()
    }

    fn read_json_record<T: DeserializeOwned>(
        &self,
        definition: TableDefinition<&[u8], &[u8]>,
        table_name: &str,
        key: &[u8],
    ) -> StorageResult<Option<T>> {
        let table = self
            .transaction
            .open_table(definition)
            .map_err(database_error)?;
        read_authenticated(
            &table,
            table_name,
            key,
            MAX_RECORD_BYTES,
            &self.integrity_key,
            &self.counters,
        )?
        .map(|raw| decode_json(&raw))
        .transpose()
    }
}

fn shared_transactional_store(
    data_dir: &Path,
    integrity_key: IntegrityKey,
) -> StorageResult<Arc<TransactionalStore>> {
    fs::create_dir_all(data_dir).map_err(database_error)?;
    let canonical_dir = fs::canonicalize(data_dir).map_err(database_error)?;
    let database_path = canonical_dir.join(TRANSACTIONAL_DATABASE_FILE);
    let registry = SHARED_STORES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut stores = registry.lock().map_err(|_| {
        StorageError::new(
            StorageErrorCode::Database,
            "shared transactional store registry is poisoned",
        )
    })?;
    // Keep the active path strongly owned for the process lifetime. A weak-only
    // registry can observe a zero strong count while redb's destructor is still
    // releasing its file lock, then race a reopen and fail with `Database already
    // open`. Sweep inactive *other* paths whenever a new path is requested so
    // tests and one-shot tools do not accumulate handles without bound.
    stores.retain(|path, store| path == &database_path || Arc::strong_count(store) > 1);
    if let Some(store) = stores.get(&database_path) {
        return Ok(Arc::clone(store));
    }
    let store = Arc::new(TransactionalStore::open_with_integrity_key(
        &canonical_dir,
        integrity_key,
    )?);
    stores.insert(database_path, Arc::clone(&store));
    Ok(store)
}

/// Release process-cached writable handles that have no live consumer before
/// an offline read-only verifier opens a database. Active handles remain in
/// place, causing the read-only open to fail closed on redb's file lock.
pub(crate) fn release_inactive_shared_transactional_stores() -> StorageResult<()> {
    let Some(registry) = SHARED_STORES.get() else {
        return Ok(());
    };
    let mut stores = registry.lock().map_err(|_| {
        StorageError::new(
            StorageErrorCode::Database,
            "shared transactional store registry is poisoned",
        )
    })?;
    stores.retain(|_, store| Arc::strong_count(store) > 1);
    Ok(())
}

const STORED_RECEIPT_HISTORY_SCHEMA: &str = "postfiat-stored-receipt-history-v1";
const STORED_BLOCK_HEIGHT_SCHEMA: &str = "postfiat-stored-block-height-v1";
const STORED_ORDERED_ID_SCHEMA: &str = "postfiat-stored-ordered-id-v1";
const STORED_ORDERED_ORDINAL_SCHEMA: &str = "postfiat-stored-ordered-ordinal-v1";
const STORED_FASTPAY_ANCHOR_SCHEMA: &str = "postfiat-stored-fastpay-anchor-v1";
const LOGICAL_INTEGRITY_REPORT_SCHEMA: &str = "postfiat-storage-logical-integrity-v1";

#[derive(Debug, Deserialize)]
struct RetainedCheckpointBindingV2 {
    schema: String,
    pruned_up_to_height: u64,
    #[serde(default)]
    ledger: Option<LedgerState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredReceiptHistoryV1 {
    schema: String,
    occurrences: Vec<StoredReceiptOccurrenceV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredReceiptOccurrenceV1 {
    finalized_height: u64,
    receipt: Receipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredBlockHeightV1 {
    schema: String,
    height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredOrderedIdV1 {
    schema: String,
    ordinal: u64,
    finalized_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredOrderedOrdinalV1 {
    schema: String,
    batch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredFastPayAnchorV1 {
    schema: String,
    finalized_height: u64,
    effect: FastPayVersionFenceV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalIntegrityReport {
    pub schema: String,
    pub storage_format: String,
    pub backend: String,
    pub finalized_height: u64,
    pub block_count: u64,
    pub receipt_count: u64,
    pub archive_count: u64,
    pub ordered_batch_count: u64,
    pub history_index_count: u64,
    pub accumulator: String,
}

fn validate_initial_state(
    tip: &ChainTipState,
    commitment: &OrderedHistoryCommitment,
) -> StorageResult<()> {
    validate_tip_domain(tip)?;
    commitment
        .validate_domain(&tip.chain_id, &tip.genesis_hash, tip.protocol_version)
        .map_err(|error| StorageError::new(StorageErrorCode::DomainMismatch, error.to_string()))?;
    if tip.height != 0
        || tip.history_base_height != 0
        || tip.ordered_batch_count != 0
        || tip.receipt_count != 0
        || commitment.count != 0
    {
        return Err(StorageError::new(
            StorageErrorCode::InitializationConflict,
            "new transactional stores initialize at the genesis boundary; use deterministic rebuild for existing history",
        ));
    }
    Ok(())
}

fn validate_retained_initial_state(
    tip: &ChainTipState,
    commitment: &OrderedHistoryCommitment,
    ordered_batches: &[String],
) -> StorageResult<()> {
    validate_tip_domain(tip)?;
    commitment
        .validate_domain(&tip.chain_id, &tip.genesis_hash, tip.protocol_version)
        .map_err(|error| StorageError::new(StorageErrorCode::DomainMismatch, error.to_string()))?;
    let ordered_count = u64::try_from(ordered_batches.len()).map_err(|_| {
        StorageError::new(
            StorageErrorCode::SizeLimit,
            "retained ordered-batch prefix is too large",
        )
    })?;
    if tip.height == 0
        || tip.history_base_height != tip.height
        || tip.ordered_batch_count != ordered_count
        || commitment.count != ordered_count
        || tip.receipt_count != 0
    {
        return Err(StorageError::new(
            StorageErrorCode::InitializationConflict,
            "retained checkpoint tip, history base, ordered prefix, and receipt count are inconsistent",
        ));
    }
    let mut unique = BTreeSet::new();
    for batch_id in ordered_batches {
        validate_identifier("retained ordered batch id", batch_id)?;
        if !unique.insert(batch_id.as_str()) {
            return Err(StorageError::new(
                StorageErrorCode::DuplicateRecord,
                "retained ordered-batch prefix contains a duplicate ID",
            ));
        }
    }
    let mut recomputed = OrderedHistoryCommitment::genesis(
        &tip.chain_id,
        &tip.genesis_hash,
        tip.protocol_version,
    )
    .map_err(|error| StorageError::new(StorageErrorCode::CorruptRecord, error.to_string()))?;
    for batch_id in ordered_batches {
        recomputed = recomputed.append(batch_id).map_err(|error| {
            StorageError::new(StorageErrorCode::CorruptRecord, error.to_string())
        })?;
    }
    if recomputed != *commitment {
        return Err(StorageError::new(
            StorageErrorCode::OrderedCommitmentMismatch,
            "retained ordered-batch prefix does not reproduce the checkpoint accumulator",
        ));
    }
    Ok(())
}

fn write_retained_ordered_prefix(
    transaction: &redb::WriteTransaction,
    ordered_batches: &[String],
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    let mut ordered_by_id = transaction
        .open_table(ORDERED_BY_ID)
        .map_err(database_error)?;
    let mut ordered_by_ordinal = transaction
        .open_table(ORDERED_BY_ORDINAL)
        .map_err(database_error)?;
    for (index, batch_id) in ordered_batches.iter().enumerate() {
        let ordinal = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::SizeLimit,
                    "retained ordered-batch ordinal overflow",
                )
            })?;
        let id_record = StoredOrderedIdV1 {
            schema: STORED_ORDERED_ID_SCHEMA.to_owned(),
            ordinal,
            finalized_height: ordinal,
        };
        insert_authenticated(
            &mut ordered_by_id,
            ORDERED_BY_ID_TABLE,
            batch_id.as_bytes(),
            &encode_json(&id_record, MAX_RECORD_BYTES)?,
            MAX_RECORD_BYTES,
            integrity_key,
            counters,
        )?;
        let ordinal_record = StoredOrderedOrdinalV1 {
            schema: STORED_ORDERED_ORDINAL_SCHEMA.to_owned(),
            batch_id: batch_id.clone(),
        };
        insert_authenticated(
            &mut ordered_by_ordinal,
            ORDERED_BY_ORDINAL_TABLE,
            &ordered_u64_key(ordinal),
            &encode_json(&ordinal_record, MAX_RECORD_BYTES)?,
            MAX_RECORD_BYTES,
            integrity_key,
            counters,
        )?;
    }
    Ok(())
}

fn verify_retained_ordered_prefix(
    transaction: &redb::WriteTransaction,
    ordered_batches: &[String],
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    let ordered_by_id = transaction
        .open_table(ORDERED_BY_ID)
        .map_err(database_error)?;
    let ordered_by_ordinal = transaction
        .open_table(ORDERED_BY_ORDINAL)
        .map_err(database_error)?;
    if ordered_by_id.len().map_err(database_error)? != ordered_batches.len() as u64
        || ordered_by_ordinal.len().map_err(database_error)? != ordered_batches.len() as u64
    {
        return Err(StorageError::new(
            StorageErrorCode::InitializationConflict,
            "existing retained checkpoint indexes have a different length",
        ));
    }
    for (index, batch_id) in ordered_batches.iter().enumerate() {
        let ordinal = (index as u64).checked_add(1).ok_or_else(|| {
            StorageError::new(
                StorageErrorCode::SizeLimit,
                "retained ordered-batch ordinal overflow",
            )
        })?;
        let id_raw = read_authenticated(
            &ordered_by_id,
            ORDERED_BY_ID_TABLE,
            batch_id.as_bytes(),
            MAX_RECORD_BYTES,
            integrity_key,
            counters,
        )?
        .ok_or_else(|| {
            StorageError::new(
                StorageErrorCode::InitializationConflict,
                "existing retained checkpoint is missing an ordered ID",
            )
        })?;
        let id_record: StoredOrderedIdV1 = decode_json(&id_raw)?;
        let ordinal_raw = read_authenticated(
            &ordered_by_ordinal,
            ORDERED_BY_ORDINAL_TABLE,
            &ordered_u64_key(ordinal),
            MAX_RECORD_BYTES,
            integrity_key,
            counters,
        )?
        .ok_or_else(|| {
            StorageError::new(
                StorageErrorCode::InitializationConflict,
                "existing retained checkpoint is missing an ordered ordinal",
            )
        })?;
        let ordinal_record: StoredOrderedOrdinalV1 = decode_json(&ordinal_raw)?;
        if id_record.schema != STORED_ORDERED_ID_SCHEMA
            || id_record.ordinal != ordinal
            || id_record.finalized_height != ordinal
            || ordinal_record.schema != STORED_ORDERED_ORDINAL_SCHEMA
            || ordinal_record.batch_id != *batch_id
        {
            return Err(StorageError::new(
                StorageErrorCode::InitializationConflict,
                "existing retained checkpoint ordered indexes do not match the requested prefix",
            ));
        }
    }
    Ok(())
}

fn validate_tip_domain(tip: &ChainTipState) -> StorageResult<()> {
    validate_identifier("chain tip schema", &tip.schema)?;
    validate_identifier("chain id", &tip.chain_id)?;
    validate_identifier("genesis hash", &tip.genesis_hash)?;
    validate_identifier("block hash", &tip.block_hash)?;
    validate_identifier("state root", &tip.state_root)?;
    if tip.height < tip.history_base_height {
        return Err(StorageError::new(
            StorageErrorCode::CountMismatch,
            "chain tip is below its retained-history base",
        ));
    }
    Ok(())
}

fn validate_commit_shape(commit: &CommitFinalizedBlock<'_>) -> StorageResult<()> {
    validate_tip_domain(commit.expected_tip)?;
    validate_tip_domain(commit.new_tip)?;
    validate_identifier("batch id", commit.batch_id)?;
    let expected_height = commit.expected_tip.height.checked_add(1).ok_or_else(|| {
        StorageError::new(
            StorageErrorCode::NonSequentialHeight,
            "block height overflow",
        )
    })?;
    if commit.new_tip.height != expected_height || commit.block.header.height != expected_height {
        return Err(StorageError::new(
            StorageErrorCode::NonSequentialHeight,
            "new tip and block must advance the expected tip by one height",
        ));
    }
    if commit.new_tip.chain_id != commit.expected_tip.chain_id
        || commit.new_tip.genesis_hash != commit.expected_tip.genesis_hash
        || commit.new_tip.protocol_version != commit.expected_tip.protocol_version
    {
        return Err(StorageError::new(
            StorageErrorCode::DomainMismatch,
            "new tip changes the chain, genesis, or protocol domain",
        ));
    }
    if commit.new_tip.history_base_height != commit.expected_tip.history_base_height {
        return Err(StorageError::new(
            StorageErrorCode::CountMismatch,
            "a finalized-block transaction cannot change the history base",
        ));
    }
    if commit.block.header.parent_hash != commit.expected_tip.block_hash {
        return Err(StorageError::new(
            StorageErrorCode::ParentHashMismatch,
            "block parent does not match the expected finalized tip",
        ));
    }
    if commit.block.header.block_hash != commit.new_tip.block_hash
        || commit.block.header.state_root != commit.new_tip.state_root
        || commit.block.header.batch_id != commit.batch_id
    {
        return Err(StorageError::new(
            StorageErrorCode::InvalidBlock,
            "block does not match the new tip or ordered batch",
        ));
    }
    if commit.archive_entry.batch_kind != commit.block.header.batch_kind
        || commit.archive_entry.batch_id != commit.batch_id
        || commit.archive_entry.payload_hash.is_empty()
    {
        return Err(StorageError::new(
            StorageErrorCode::ArchiveMismatch,
            "archived batch does not match the finalized block",
        ));
    }
    if commit.block.header.receipt_count != commit.receipts.len() as u64
        || commit.block.receipt_ids.len() != commit.receipts.len()
    {
        return Err(StorageError::new(
            StorageErrorCode::ReceiptMismatch,
            "literal receipt count does not match the finalized block",
        ));
    }
    let mut fastpay_lock_ids = BTreeSet::new();
    for effect in &commit.block.fastpay_pre_state_effects {
        effect.validate_shape().map_err(|error| {
            StorageError::new(
                StorageErrorCode::InvalidBlock,
                format!("FastPay pre-state effect is invalid: {error}"),
            )
        })?;
        if !fastpay_lock_ids.insert(effect.lock_id.as_str()) {
            return Err(StorageError::new(
                StorageErrorCode::InvalidBlock,
                "FastPay pre-state effect lock id is duplicated",
            ));
        }
    }
    let mut receipt_ids = BTreeSet::new();
    for (receipt_id, receipt) in commit.block.receipt_ids.iter().zip(commit.receipts) {
        validate_identifier("receipt id", receipt_id)?;
        if (!commit.allow_legacy_receipt_id_mismatch && receipt.tx_id != *receipt_id)
            || !receipt_ids.insert(receipt_id.as_str())
        {
            return Err(StorageError::new(
                StorageErrorCode::ReceiptMismatch,
                "receipt IDs are missing, reordered, or duplicated",
            ));
        }
    }
    let expected_receipt_count = commit
        .expected_tip
        .receipt_count
        .checked_add(commit.receipts.len() as u64)
        .ok_or_else(|| {
            StorageError::new(StorageErrorCode::CountMismatch, "receipt count overflow")
        })?;
    if commit.new_tip.receipt_count != expected_receipt_count
        || commit.new_tip.ordered_batch_count
            != commit
                .expected_tip
                .ordered_batch_count
                .checked_add(1)
                .ok_or_else(|| {
                    StorageError::new(StorageErrorCode::CountMismatch, "ordered count overflow")
                })?
    {
        return Err(StorageError::new(
            StorageErrorCode::CountMismatch,
            "new tip counts do not match the finalized delta",
        ));
    }
    commit
        .ordered_history
        .validate_domain(
            &commit.new_tip.chain_id,
            &commit.new_tip.genesis_hash,
            commit.new_tip.protocol_version,
        )
        .map_err(|error| StorageError::new(StorageErrorCode::DomainMismatch, error.to_string()))?;
    if commit.ordered_history.count != commit.new_tip.ordered_batch_count {
        return Err(StorageError::new(
            StorageErrorCode::OrderedCommitmentMismatch,
            "ordered-history count does not match the new tip",
        ));
    }
    validate_current_state_update(commit.current_state)
}

fn validate_commit_against_meta(
    meta: &TransactionalStoreMetaV1,
    commit: &CommitFinalizedBlock<'_>,
) -> StorageResult<()> {
    match (
        meta.scheduled_activation_height,
        commit.scheduled_activation_height,
    ) {
        (None, None) => {}
        (None, Some(height)) if commit.new_tip.height < height => {}
        (Some(previous), Some(next)) if previous == next => {}
        (Some(previous), None) if commit.new_tip.height < previous => {}
        (None, Some(_)) => {
            return Err(StorageError::new(
                StorageErrorCode::DomainMismatch,
                "transactional storage activation must be scheduled before its height",
            ));
        }
        (Some(_), Some(_)) => {
            return Err(StorageError::new(
                StorageErrorCode::DomainMismatch,
                "transactional storage activation height cannot be replaced",
            ));
        }
        (Some(_), None) => {
            return Err(StorageError::new(
                StorageErrorCode::DomainMismatch,
                "transactional storage activation cannot be cancelled at or after activation",
            ));
        }
    }
    let expected_commitment = meta
        .ordered_history_commitment()
        .append(commit.batch_id)
        .map_err(|error| {
            StorageError::new(
                StorageErrorCode::OrderedCommitmentMismatch,
                error.to_string(),
            )
        })?;
    if expected_commitment != *commit.ordered_history {
        return Err(StorageError::new(
            StorageErrorCode::OrderedCommitmentMismatch,
            "provided ordered-history commitment is not the canonical append result",
        ));
    }
    Ok(())
}

fn ensure_commit_keys_absent(
    transaction: &redb::WriteTransaction,
    commit: &CommitFinalizedBlock<'_>,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    let height_key = ordered_u64_key(commit.new_tip.height);
    ensure_absent(
        &transaction
            .open_table(BLOCKS_BY_HEIGHT)
            .map_err(database_error)?,
        BLOCKS_BY_HEIGHT_TABLE,
        &height_key,
        integrity_key,
        counters,
    )?;
    ensure_absent(
        &transaction
            .open_table(BLOCK_HEIGHT_BY_HASH)
            .map_err(database_error)?,
        BLOCK_HEIGHT_BY_HASH_TABLE,
        commit.new_tip.block_hash.as_bytes(),
        integrity_key,
        counters,
    )?;
    ensure_absent(
        &transaction
            .open_table(ORDERED_BY_ID)
            .map_err(database_error)?,
        ORDERED_BY_ID_TABLE,
        commit.batch_id.as_bytes(),
        integrity_key,
        counters,
    )?;
    let ordinal_key = ordered_u64_key(commit.ordered_history.count);
    ensure_absent(
        &transaction
            .open_table(ORDERED_BY_ORDINAL)
            .map_err(database_error)?,
        ORDERED_BY_ORDINAL_TABLE,
        &ordinal_key,
        integrity_key,
        counters,
    )?;
    let archive_key = archive_key(
        &commit.archive_entry.batch_kind,
        &commit.archive_entry.batch_id,
    )?;
    ensure_absent(
        &transaction
            .open_table(BATCH_ARCHIVE)
            .map_err(database_error)?,
        BATCH_ARCHIVE_TABLE,
        &archive_key,
        integrity_key,
        counters,
    )?;
    let history_indexes = transaction
        .open_table(HISTORY_INDEXES)
        .map_err(database_error)?;
    for effect in &commit.block.fastpay_pre_state_effects {
        ensure_absent(
            &history_indexes,
            HISTORY_INDEXES_TABLE,
            &fastpay_anchor_key(&effect.lock_id)?,
            integrity_key,
            counters,
        )?;
    }
    Ok(())
}

fn ensure_absent<T: ReadableTable<&'static [u8], &'static [u8]>>(
    table: &T,
    table_name: &str,
    key: &[u8],
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    if read_authenticated(
        table,
        table_name,
        key,
        MAX_RECORD_BYTES,
        integrity_key,
        counters,
    )?
    .is_some()
    {
        return Err(StorageError::new(
            StorageErrorCode::DuplicateRecord,
            format!("duplicate key in `{table_name}`"),
        ));
    }
    Ok(())
}

fn verify_idempotent_commit(
    transaction: &redb::WriteTransaction,
    meta: &TransactionalStoreMetaV1,
    commit: &CommitFinalizedBlock<'_>,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    validate_commit_against_previous_tip(commit)?;
    if meta.scheduled_activation_height != commit.scheduled_activation_height {
        return Err(StorageError::new(
            StorageErrorCode::IdempotentConflict,
            "stored activation schedule differs from retry",
        ));
    }
    if meta.ordered_history_commitment() != *commit.ordered_history {
        return Err(StorageError::new(
            StorageErrorCode::IdempotentConflict,
            "stored ordered-history commitment differs from retry",
        ));
    }
    let block_key = ordered_u64_key(commit.new_tip.height);
    let stored_block: BlockRecord = required_json_record(
        &transaction
            .open_table(BLOCKS_BY_HEIGHT)
            .map_err(database_error)?,
        BLOCKS_BY_HEIGHT_TABLE,
        &block_key,
        integrity_key,
        counters,
    )?;
    if stored_block != *commit.block {
        return Err(idempotent_conflict("block"));
    }
    let height_record: StoredBlockHeightV1 = required_json_record(
        &transaction
            .open_table(BLOCK_HEIGHT_BY_HASH)
            .map_err(database_error)?,
        BLOCK_HEIGHT_BY_HASH_TABLE,
        commit.new_tip.block_hash.as_bytes(),
        integrity_key,
        counters,
    )?;
    if height_record.schema != STORED_BLOCK_HEIGHT_SCHEMA
        || height_record.height != commit.new_tip.height
    {
        return Err(idempotent_conflict("block hash index"));
    }
    let ordered_id: StoredOrderedIdV1 = required_json_record(
        &transaction
            .open_table(ORDERED_BY_ID)
            .map_err(database_error)?,
        ORDERED_BY_ID_TABLE,
        commit.batch_id.as_bytes(),
        integrity_key,
        counters,
    )?;
    if ordered_id.schema != STORED_ORDERED_ID_SCHEMA
        || ordered_id.ordinal != commit.ordered_history.count
        || ordered_id.finalized_height != commit.new_tip.height
    {
        return Err(idempotent_conflict("ordered ID index"));
    }
    let ordinal_key = ordered_u64_key(commit.ordered_history.count);
    let ordered_ordinal: StoredOrderedOrdinalV1 = required_json_record(
        &transaction
            .open_table(ORDERED_BY_ORDINAL)
            .map_err(database_error)?,
        ORDERED_BY_ORDINAL_TABLE,
        &ordinal_key,
        integrity_key,
        counters,
    )?;
    if ordered_ordinal.schema != STORED_ORDERED_ORDINAL_SCHEMA
        || ordered_ordinal.batch_id != commit.batch_id
    {
        return Err(idempotent_conflict("ordered ordinal index"));
    }
    let archive_key = archive_key(
        &commit.archive_entry.batch_kind,
        &commit.archive_entry.batch_id,
    )?;
    let archive: BatchArchiveEntry = required_json_record(
        &transaction
            .open_table(BATCH_ARCHIVE)
            .map_err(database_error)?,
        BATCH_ARCHIVE_TABLE,
        &archive_key,
        integrity_key,
        counters,
    )?;
    if archive != *commit.archive_entry {
        return Err(idempotent_conflict("batch archive"));
    }
    let receipts = transaction
        .open_table(RECEIPTS_BY_ID)
        .map_err(database_error)?;
    for (canonical_receipt_id, receipt) in commit.block.receipt_ids.iter().zip(commit.receipts) {
        let stored: StoredReceiptHistoryV1 = required_json_record(
            &receipts,
            RECEIPTS_BY_ID_TABLE,
            canonical_receipt_id.as_bytes(),
            integrity_key,
            counters,
        )?;
        validate_receipt_history(canonical_receipt_id, &stored)?;
        let matching = stored.occurrences.iter().filter(|occurrence| {
            occurrence.finalized_height == commit.new_tip.height && occurrence.receipt == *receipt
        });
        if matching.count() != 1 {
            return Err(idempotent_conflict("receipt"));
        }
    }
    let history_indexes = transaction
        .open_table(HISTORY_INDEXES)
        .map_err(database_error)?;
    for effect in &commit.block.fastpay_pre_state_effects {
        let stored: StoredFastPayAnchorV1 = required_json_record(
            &history_indexes,
            HISTORY_INDEXES_TABLE,
            &fastpay_anchor_key(&effect.lock_id)?,
            integrity_key,
            counters,
        )?;
        if stored.schema != STORED_FASTPAY_ANCHOR_SCHEMA
            || stored.finalized_height != commit.new_tip.height
            || stored.effect != *effect
        {
            return Err(idempotent_conflict("FastPay anchor index"));
        }
    }
    verify_current_state(transaction, commit.current_state, integrity_key, counters)
}

fn validate_commit_against_previous_tip(commit: &CommitFinalizedBlock<'_>) -> StorageResult<()> {
    validate_commit_shape(commit)?;
    let expected = commit
        .ordered_history
        .count
        .checked_sub(1)
        .ok_or_else(|| idempotent_conflict("ordered commitment count"))?;
    if expected != commit.expected_tip.ordered_batch_count {
        return Err(idempotent_conflict("expected tip ordered count"));
    }
    Ok(())
}

fn idempotent_conflict(record: &str) -> StorageError {
    StorageError::new(
        StorageErrorCode::IdempotentConflict,
        format!("stored {record} differs from idempotent retry"),
    )
}

fn write_current_state(
    transaction: &redb::WriteTransaction,
    state: CurrentStateUpdate<'_>,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    validate_current_state_update(state)?;
    let mut table = transaction
        .open_table(CURRENT_STATE)
        .map_err(database_error)?;
    if let Some(value) = state.ledger {
        insert_state_json(&mut table, STATE_LEDGER, value, integrity_key, counters)?;
    }
    if let Some(value) = state.governance {
        insert_state_json(&mut table, STATE_GOVERNANCE, value, integrity_key, counters)?;
    }
    if let Some(value) = state.shielded {
        insert_state_json(&mut table, STATE_SHIELDED, value, integrity_key, counters)?;
    }
    if let Some(value) = state.bridge {
        insert_state_json(&mut table, STATE_BRIDGE, value, integrity_key, counters)?;
    }
    if let Some(value) = state.node_state {
        insert_state_json(&mut table, STATE_NODE, value, integrity_key, counters)?;
    }
    for value in state.additional {
        insert_authenticated(
            &mut table,
            CURRENT_STATE_TABLE,
            value.domain.as_bytes(),
            &value.canonical_bytes,
            MAX_CURRENT_STATE_BYTES,
            integrity_key,
            counters,
        )?;
    }
    Ok(())
}

fn verify_current_state(
    transaction: &redb::WriteTransaction,
    state: CurrentStateUpdate<'_>,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    validate_current_state_update(state)?;
    let table = transaction
        .open_table(CURRENT_STATE)
        .map_err(database_error)?;
    if let Some(value) = state.ledger {
        verify_state_json(&table, STATE_LEDGER, value, integrity_key, counters)?;
    }
    if let Some(value) = state.governance {
        verify_state_json(&table, STATE_GOVERNANCE, value, integrity_key, counters)?;
    }
    if let Some(value) = state.shielded {
        verify_state_json(&table, STATE_SHIELDED, value, integrity_key, counters)?;
    }
    if let Some(value) = state.bridge {
        verify_state_json(&table, STATE_BRIDGE, value, integrity_key, counters)?;
    }
    if let Some(value) = state.node_state {
        verify_state_json(&table, STATE_NODE, value, integrity_key, counters)?;
    }
    for value in state.additional {
        let stored = read_authenticated(
            &table,
            CURRENT_STATE_TABLE,
            value.domain.as_bytes(),
            MAX_CURRENT_STATE_BYTES,
            integrity_key,
            counters,
        )?
        .ok_or_else(|| idempotent_conflict("current state"))?;
        if stored != value.canonical_bytes {
            return Err(idempotent_conflict("current state"));
        }
    }
    Ok(())
}

fn insert_state_json<T: Serialize>(
    table: &mut redb::Table<'_, &[u8], &[u8]>,
    domain: &str,
    value: &T,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    let bytes = encode_json(value, MAX_CURRENT_STATE_BYTES)?;
    insert_authenticated(
        table,
        CURRENT_STATE_TABLE,
        domain.as_bytes(),
        &bytes,
        MAX_CURRENT_STATE_BYTES,
        integrity_key,
        counters,
    )
}

fn verify_state_json<T: Serialize>(
    table: &impl ReadableTable<&'static [u8], &'static [u8]>,
    domain: &str,
    value: &T,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    let expected = encode_json(value, MAX_CURRENT_STATE_BYTES)?;
    let stored = read_authenticated(
        table,
        CURRENT_STATE_TABLE,
        domain.as_bytes(),
        MAX_CURRENT_STATE_BYTES,
        integrity_key,
        counters,
    )?
    .ok_or_else(|| idempotent_conflict("current state"))?;
    if stored != expected {
        return Err(idempotent_conflict("current state"));
    }
    Ok(())
}

fn validate_current_state_update(state: CurrentStateUpdate<'_>) -> StorageResult<()> {
    let mut seen = BTreeSet::new();
    for value in state.additional {
        validate_state_domain(&value.domain, true)?;
        if value.canonical_bytes.len() > MAX_CURRENT_STATE_BYTES {
            return Err(StorageError::new(
                StorageErrorCode::SizeLimit,
                "additional current-state value exceeds its closed size bound",
            ));
        }
        if !seen.insert(value.domain.as_str()) {
            return Err(StorageError::new(
                StorageErrorCode::NonCanonicalKey,
                "additional current-state domain is duplicated",
            ));
        }
    }
    Ok(())
}

fn validate_state_domain(domain: &str, require_additional: bool) -> StorageResult<()> {
    if domain.is_empty()
        || domain.len() > MAX_DOMAIN_NAME_BYTES
        || !domain
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
    {
        return Err(StorageError::new(
            StorageErrorCode::NonCanonicalKey,
            "current-state domain is not canonical",
        ));
    }
    let typed = matches!(
        domain,
        STATE_LEDGER | STATE_GOVERNANCE | STATE_SHIELDED | STATE_BRIDGE | STATE_NODE
    );
    let additional = ALLOWED_ADDITIONAL_STATE_DOMAINS.contains(&domain);
    if !additional && (require_additional || !typed) {
        return Err(StorageError::new(
            StorageErrorCode::UnsupportedSchema,
            format!("unsupported current-state domain `{domain}`"),
        ));
    }
    Ok(())
}

fn ensure_all_non_meta_tables_empty(transaction: &redb::WriteTransaction) -> StorageResult<()> {
    let mut non_empty = false;
    non_empty |= !transaction
        .open_table(BLOCKS_BY_HEIGHT)
        .map_err(database_error)?
        .is_empty()
        .map_err(database_error)?;
    non_empty |= !transaction
        .open_table(BLOCK_HEIGHT_BY_HASH)
        .map_err(database_error)?
        .is_empty()
        .map_err(database_error)?;
    non_empty |= !transaction
        .open_table(RECEIPTS_BY_ID)
        .map_err(database_error)?
        .is_empty()
        .map_err(database_error)?;
    non_empty |= !transaction
        .open_table(BATCH_ARCHIVE)
        .map_err(database_error)?
        .is_empty()
        .map_err(database_error)?;
    non_empty |= !transaction
        .open_table(ORDERED_BY_ID)
        .map_err(database_error)?
        .is_empty()
        .map_err(database_error)?;
    non_empty |= !transaction
        .open_table(ORDERED_BY_ORDINAL)
        .map_err(database_error)?
        .is_empty()
        .map_err(database_error)?;
    non_empty |= !transaction
        .open_table(CURRENT_STATE)
        .map_err(database_error)?
        .is_empty()
        .map_err(database_error)?;
    non_empty |= !transaction
        .open_table(HISTORY_INDEXES)
        .map_err(database_error)?
        .is_empty()
        .map_err(database_error)?;
    if non_empty {
        return Err(StorageError::new(
            StorageErrorCode::InitializationConflict,
            "uninitialized database contains logical records",
        ));
    }
    Ok(())
}

fn read_meta<T: ReadableTable<&'static [u8], &'static [u8]>>(
    table: &T,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<TransactionalStoreMetaV1> {
    let raw = read_authenticated(
        table,
        META_TABLE,
        META_KEY,
        MAX_META_BYTES,
        integrity_key,
        counters,
    )?
    .ok_or_else(|| StorageError::new(StorageErrorCode::Uninitialized, "metadata is missing"))?;
    let meta: TransactionalStoreMetaV1 = decode_json(&raw)?;
    meta.validate()?;
    Ok(meta)
}

fn required_json_record<T: DeserializeOwned>(
    table: &impl ReadableTable<&'static [u8], &'static [u8]>,
    table_name: &str,
    key: &[u8],
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<T> {
    let raw = read_authenticated(
        table,
        table_name,
        key,
        MAX_RECORD_BYTES,
        integrity_key,
        counters,
    )?
    .ok_or_else(|| idempotent_conflict(table_name))?;
    decode_json(&raw)
}

fn insert_authenticated(
    table: &mut redb::Table<'_, &[u8], &[u8]>,
    table_name: &str,
    key: &[u8],
    payload: &[u8],
    max_payload: usize,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    validate_raw_key(key)?;
    if payload.len() > max_payload {
        return Err(StorageError::new(
            StorageErrorCode::SizeLimit,
            format!("value for `{table_name}` exceeds {max_payload} bytes"),
        ));
    }
    let encoded = authenticated_value(table_name, key, payload, integrity_key)?;
    table
        .insert(key, encoded.as_slice())
        .map_err(database_error)?;
    counters.page_writes.fetch_add(1, Ordering::Relaxed);
    counters.records_written.fetch_add(1, Ordering::Relaxed);
    counters
        .bytes_written
        .fetch_add(encoded.len() as u64, Ordering::Relaxed);
    Ok(())
}

fn read_authenticated<T: ReadableTable<&'static [u8], &'static [u8]>>(
    table: &T,
    table_name: &str,
    key: &[u8],
    max_payload: usize,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<Option<Vec<u8>>> {
    validate_raw_key(key)?;
    counters.page_reads.fetch_add(1, Ordering::Relaxed);
    let Some(value) = table.get(key).map_err(database_error)? else {
        return Ok(None);
    };
    let encoded = value.value();
    counters.records_read.fetch_add(1, Ordering::Relaxed);
    counters
        .bytes_read
        .fetch_add(encoded.len() as u64, Ordering::Relaxed);
    decode_authenticated_value(table_name, key, encoded, max_payload, integrity_key).map(Some)
}

fn authenticated_value(
    table_name: &str,
    key: &[u8],
    payload: &[u8],
    integrity_key: &IntegrityKey,
) -> StorageResult<Vec<u8>> {
    let binding = value_mac_payload(table_name, key, payload)?;
    let tag = integrity_key.mac(VALUE_MAC_DOMAIN, &binding);
    let mut encoded = Vec::with_capacity(1 + MAC_BYTES + payload.len());
    encoded.push(VALUE_SCHEMA_VERSION);
    encoded.extend_from_slice(&tag);
    encoded.extend_from_slice(payload);
    Ok(encoded)
}

fn decode_authenticated_value(
    table_name: &str,
    key: &[u8],
    encoded: &[u8],
    max_payload: usize,
    integrity_key: &IntegrityKey,
) -> StorageResult<Vec<u8>> {
    if encoded.len() < 1 + MAC_BYTES {
        return Err(StorageError::new(
            StorageErrorCode::CorruptRecord,
            format!("value in `{table_name}` is shorter than its envelope"),
        ));
    }
    if encoded[0] != VALUE_SCHEMA_VERSION {
        return Err(StorageError::new(
            StorageErrorCode::UnsupportedSchema,
            format!("value in `{table_name}` uses an unsupported envelope version"),
        ));
    }
    let payload = &encoded[1 + MAC_BYTES..];
    if payload.len() > max_payload {
        return Err(StorageError::new(
            StorageErrorCode::SizeLimit,
            format!("value in `{table_name}` exceeds its closed size bound"),
        ));
    }
    let binding = value_mac_payload(table_name, key, payload)?;
    let expected = integrity_key.mac(VALUE_MAC_DOMAIN, &binding);
    if !macs_equal(&expected, &encoded[1..1 + MAC_BYTES]) {
        return Err(StorageError::new(
            StorageErrorCode::IntegrityFailure,
            format!("value in `{table_name}` fails its table-and-key integrity binding"),
        ));
    }
    Ok(payload.to_vec())
}

fn value_mac_payload(table_name: &str, key: &[u8], payload: &[u8]) -> StorageResult<Vec<u8>> {
    let table_len = u16::try_from(table_name.len())
        .map_err(|_| StorageError::new(StorageErrorCode::SizeLimit, "table domain is too large"))?;
    let key_len = u32::try_from(key.len())
        .map_err(|_| StorageError::new(StorageErrorCode::SizeLimit, "key is too large"))?;
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| StorageError::new(StorageErrorCode::SizeLimit, "value is too large"))?;
    let mut binding = Vec::with_capacity(2 + table_name.len() + 4 + key.len() + 8 + payload.len());
    binding.extend_from_slice(&table_len.to_be_bytes());
    binding.extend_from_slice(table_name.as_bytes());
    binding.extend_from_slice(&key_len.to_be_bytes());
    binding.extend_from_slice(key);
    binding.extend_from_slice(&payload_len.to_be_bytes());
    binding.extend_from_slice(payload);
    Ok(binding)
}

fn verify_every_record<T: ReadableTable<&'static [u8], &'static [u8]>>(
    table: &T,
    table_name: &str,
    max_payload: usize,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    let iterator = table.iter().map_err(database_error)?;
    for entry in iterator {
        let (key, value) = entry.map_err(database_error)?;
        let key = key.value();
        let encoded = value.value();
        counters.page_reads.fetch_add(1, Ordering::Relaxed);
        counters.records_read.fetch_add(1, Ordering::Relaxed);
        counters
            .bytes_read
            .fetch_add(encoded.len() as u64, Ordering::Relaxed);
        decode_authenticated_value(table_name, key, encoded, max_payload, integrity_key)?;
    }
    Ok(())
}

fn encode_json<T: Serialize>(value: &T, max_bytes: usize) -> StorageResult<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(serialization_error)?;
    if bytes.len() > max_bytes {
        return Err(StorageError::new(
            StorageErrorCode::SizeLimit,
            format!("canonical JSON exceeds {max_bytes} bytes"),
        ));
    }
    Ok(bytes)
}

fn decode_json<T: DeserializeOwned>(bytes: &[u8]) -> StorageResult<T> {
    serde_json::from_slice(bytes).map_err(serialization_error)
}

fn validate_identifier(label: &str, value: &str) -> StorageResult<()> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || value
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(StorageError::new(
            StorageErrorCode::NonCanonicalKey,
            format!("{label} is empty, oversized, or contains control bytes"),
        ));
    }
    Ok(())
}

fn validate_raw_key(key: &[u8]) -> StorageResult<()> {
    if key.is_empty() || key.len() > MAX_ID_BYTES * 2 + 8 {
        return Err(StorageError::new(
            StorageErrorCode::NonCanonicalKey,
            "logical database key is empty or oversized",
        ));
    }
    Ok(())
}

fn archive_key(batch_kind: &str, batch_id: &str) -> StorageResult<Vec<u8>> {
    validate_identifier("batch kind", batch_kind)?;
    validate_identifier("batch id", batch_id)?;
    let kind_len = u16::try_from(batch_kind.len())
        .map_err(|_| StorageError::new(StorageErrorCode::SizeLimit, "batch kind is too large"))?;
    let mut key = Vec::with_capacity(2 + batch_kind.len() + batch_id.len());
    key.extend_from_slice(&kind_len.to_be_bytes());
    key.extend_from_slice(batch_kind.as_bytes());
    key.extend_from_slice(batch_id.as_bytes());
    Ok(key)
}

fn fastpay_anchor_key(lock_id: &str) -> StorageResult<Vec<u8>> {
    validate_identifier("FastPay lock id", lock_id)?;
    let mut key = Vec::with_capacity(FASTPAY_ANCHOR_KEY_PREFIX.len() + lock_id.len());
    key.extend_from_slice(FASTPAY_ANCHOR_KEY_PREFIX);
    key.extend_from_slice(lock_id.as_bytes());
    validate_raw_key(&key)?;
    Ok(key)
}

pub const fn ordered_u64_key(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}

fn validate_stored_ordered_ordinal(record: &StoredOrderedOrdinalV1) -> StorageResult<()> {
    if record.schema != STORED_ORDERED_ORDINAL_SCHEMA {
        return Err(StorageError::new(
            StorageErrorCode::UnsupportedSchema,
            "ordered ordinal record has an unsupported schema",
        ));
    }
    validate_identifier("batch id", &record.batch_id)
}

fn validate_receipt_history(
    _receipt_id: &str,
    history: &StoredReceiptHistoryV1,
) -> StorageResult<()> {
    if history.schema != STORED_RECEIPT_HISTORY_SCHEMA
        || history.occurrences.is_empty()
        || history.occurrences.len() > MAX_RECEIPT_OCCURRENCES_PER_ID
    {
        return Err(StorageError::new(
            StorageErrorCode::UnsupportedSchema,
            "receipt occurrence history has an unsupported schema or cardinality",
        ));
    }
    let mut previous: Option<&StoredReceiptOccurrenceV1> = None;
    for occurrence in &history.occurrences {
        if occurrence.finalized_height == 0 {
            return Err(StorageError::new(
                StorageErrorCode::CorruptRecord,
                "receipt occurrence has an invalid finalized height",
            ));
        }
        validate_identifier("literal receipt id", &occurrence.receipt.tx_id)?;
        if let Some(previous) = previous {
            if occurrence.finalized_height <= previous.finalized_height
                || (occurrence.receipt != previous.receipt
                    && (previous.receipt.accepted || !occurrence.receipt.accepted))
            {
                return Err(StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    "receipt occurrence history has a conflicting or non-monotonic transition",
                ));
            }
        }
        previous = Some(occurrence);
    }
    Ok(())
}

fn validate_receipt_transition(
    history: &StoredReceiptHistoryV1,
    receipt: &Receipt,
) -> StorageResult<()> {
    let previous = history.occurrences.last().ok_or_else(|| {
        StorageError::new(
            StorageErrorCode::CorruptRecord,
            "receipt occurrence history is empty",
        )
    })?;
    if previous.receipt == *receipt || (!previous.receipt.accepted && receipt.accepted) {
        return Ok(());
    }
    Err(StorageError::new(
        StorageErrorCode::ReceiptMismatch,
        format!(
            "receipt `{}` conflicts with its terminal stored result",
            receipt.tx_id
        ),
    ))
}

fn receipt_occurrence_count(
    table: &impl ReadableTable<&'static [u8], &'static [u8]>,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<u64> {
    let mut count = 0_u64;
    for entry in table.iter().map_err(database_error)? {
        let (key, value) = entry.map_err(database_error)?;
        let receipt_id = std::str::from_utf8(key.value()).map_err(|_| {
            StorageError::new(
                StorageErrorCode::NonCanonicalKey,
                "receipt table contains a non-UTF-8 key",
            )
        })?;
        counters.page_reads.fetch_add(1, Ordering::Relaxed);
        let payload = decode_authenticated_value(
            RECEIPTS_BY_ID_TABLE,
            key.value(),
            value.value(),
            MAX_RECORD_BYTES,
            integrity_key,
        )?;
        counters.records_read.fetch_add(1, Ordering::Relaxed);
        counters
            .bytes_read
            .fetch_add(value.value().len() as u64, Ordering::Relaxed);
        let history: StoredReceiptHistoryV1 = decode_json(&payload)?;
        validate_receipt_history(receipt_id, &history)?;
        count = count
            .checked_add(history.occurrences.len() as u64)
            .ok_or_else(|| {
                StorageError::new(StorageErrorCode::CountMismatch, "receipt count overflow")
            })?;
    }
    Ok(count)
}

fn database_error(error: impl fmt::Display) -> StorageError {
    StorageError::new(StorageErrorCode::Database, error.to_string())
}

fn serialization_error(error: impl fmt::Display) -> StorageError {
    StorageError::new(StorageErrorCode::Serialization, error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use postfiat_types::{BlockCertificate, BlockHeader};

    use super::*;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "postfiat-transactional-storage-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn genesis_tip() -> ChainTipState {
        ChainTipState {
            schema: "postfiat-chain-tip-v1".to_owned(),
            chain_id: "test-chain".to_owned(),
            genesis_hash: "genesis-hash".to_owned(),
            protocol_version: 1,
            height: 0,
            block_hash: "genesis".to_owned(),
            state_root: "state-0".to_owned(),
            ordered_batch_count: 0,
            receipt_count: 0,
            history_base_height: 0,
        }
    }

    fn next_tip(receipt_count: u64) -> ChainTipState {
        ChainTipState {
            height: 1,
            block_hash: "block-1".to_owned(),
            state_root: "state-1".to_owned(),
            ordered_batch_count: 1,
            receipt_count,
            ..genesis_tip()
        }
    }

    fn receipt(tx_id: &str, accepted: bool) -> Receipt {
        Receipt {
            tx_id: tx_id.to_owned(),
            accepted,
            code: if accepted { "accepted" } else { "rejected" }.to_owned(),
            message: "literal result".to_owned(),
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

    fn block(receipt_ids: Vec<String>) -> BlockRecord {
        BlockRecord {
            header: BlockHeader {
                height: 1,
                view: 0,
                parent_hash: "genesis".to_owned(),
                proposer: "validator-1".to_owned(),
                batch_kind: "payments".to_owned(),
                batch_id: "batch-1".to_owned(),
                state_root: "state-1".to_owned(),
                bridge_exit_root: None,
                pftl_uniswap_receipt_root: None,
                receipt_count: receipt_ids.len() as u64,
                certificate_id: "certificate-1".to_owned(),
                certificate: BlockCertificate {
                    validators: Vec::new(),
                    quorum: 0,
                    registry_root: String::new(),
                    votes: Vec::new(),
                },
                consensus_v2_commit: None,
                block_hash: "block-1".to_owned(),
            },
            receipt_ids,
            fastpay_pre_state_effects: Vec::new(),
        }
    }

    fn archive() -> BatchArchiveEntry {
        BatchArchiveEntry {
            batch_kind: "payments".to_owned(),
            batch_id: "batch-1".to_owned(),
            payload_hash: "payload-hash".to_owned(),
            payload_json: "{}".to_owned(),
        }
    }

    fn confirmed_consensusless_fastpay_effect() -> FastPayVersionFenceV1 {
        let input = postfiat_types::OwnedObjectRef {
            id: "owned-input".to_owned(),
            version: 1,
        };
        let certificate = postfiat_types::FastPayCertificateV1::Unwrap(
            postfiat_types::OwnedUnwrapCertificateV3 {
                order: postfiat_types::OwnedUnwrapOrderV3 {
                    domain: postfiat_types::OwnedCertificateDomain {
                        schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_owned(),
                        chain_id: "test-chain".to_owned(),
                        genesis_hash: "genesis-hash".to_owned(),
                        protocol_version: 1,
                        registry_id: "11".repeat(48),
                    },
                    recovery: postfiat_types::FastPayOrderRecoveryV1 {
                        schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_owned(),
                        committee_epoch: 1,
                        lock_id: "22".repeat(48),
                        valid_from_height: 1,
                        expires_at_height: 2,
                        recovery_closes_at_height: 3,
                    },
                    inputs: vec![input.clone()],
                    to_address: "pf-test-recipient".to_owned(),
                    amount: 1,
                    asset: "PFT".to_owned(),
                    fee: 0,
                    nonce: 1,
                    memos: Vec::new(),
                },
                owner_pubkey_hex: "owner".to_owned(),
                owner_signature_hex: "signature".to_owned(),
                votes: Vec::new(),
            },
        );
        FastPayVersionFenceV1 {
            schema: postfiat_types::FASTPAY_VERSION_FENCE_SCHEMA_V1.to_owned(),
            operation: postfiat_types::FastPayOperationKindV1::Unwrap,
            origin: postfiat_types::FastPayFenceOriginV1::Consensusless,
            committee_epoch: 1,
            registry_root: "11".repeat(48),
            lock_id: "22".repeat(48),
            inputs: vec![input.clone()],
            decision: postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
                order_digest: "33".repeat(48),
                certificate_digest: "44".repeat(48),
            },
            certificate: Some(certificate),
            decided_at_height: 1,
            next_versions: vec![postfiat_types::OwnedObjectRef {
                id: input.id,
                version: 2,
            }],
        }
    }

    fn committed_one_block(label: &str) -> (TestDir, TransactionalStore) {
        let dir = TestDir::new(label);
        let store = TransactionalStore::open(&dir.0).expect("open committed fixture store");
        let old_tip = genesis_tip();
        let old_commitment = OrderedHistoryCommitment::genesis(
            &old_tip.chain_id,
            &old_tip.genesis_hash,
            old_tip.protocol_version,
        )
        .expect("fixture genesis commitment");
        store
            .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
            .expect("initialize committed fixture");
        let receipt = receipt("tx-1", true);
        let new_tip = next_tip(1);
        let new_commitment = old_commitment.append("batch-1").expect("fixture append");
        let block = block(vec![receipt.tx_id.clone()]);
        let archive = archive();
        store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &old_tip,
                new_tip: &new_tip,
                block: &block,
                receipts: std::slice::from_ref(&receipt),
                archive_entry: &archive,
                batch_id: "batch-1",
                ordered_history: &new_commitment,
                current_state: CurrentStateUpdate::default(),
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect("commit fixture block");
        (dir, store)
    }

    fn committed_two_blocks(label: &str) -> (TestDir, TransactionalStore) {
        let (dir, store) = committed_one_block(label);
        let old_tip = next_tip(1);
        let old_commitment = store
            .meta()
            .expect("read one-block metadata")
            .ordered_history_commitment();
        let receipt = receipt("tx-2", true);
        let new_tip = ChainTipState {
            height: 2,
            block_hash: "block-2".to_owned(),
            state_root: "state-2".to_owned(),
            ordered_batch_count: 2,
            receipt_count: 2,
            ..old_tip.clone()
        };
        let new_commitment = old_commitment
            .append("batch-2")
            .expect("append second batch");
        let mut block = block(vec![receipt.tx_id.clone()]);
        block.header.height = 2;
        block.header.parent_hash = old_tip.block_hash.clone();
        block.header.batch_id = "batch-2".to_owned();
        block.header.state_root = new_tip.state_root.clone();
        block.header.certificate_id = "certificate-2".to_owned();
        block.header.block_hash = new_tip.block_hash.clone();
        let mut archive = archive();
        archive.batch_id = "batch-2".to_owned();
        archive.payload_hash = "payload-hash-2".to_owned();
        let ledger = LedgerState::empty();
        store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &old_tip,
                new_tip: &new_tip,
                block: &block,
                receipts: std::slice::from_ref(&receipt),
                archive_entry: &archive,
                batch_id: "batch-2",
                ordered_history: &new_commitment,
                current_state: CurrentStateUpdate {
                    ledger: Some(&ledger),
                    ..CurrentStateUpdate::default()
                },
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect("commit second fixture block");
        (dir, store)
    }

    fn replace_authenticated_block(
        store: &TransactionalStore,
        height: u64,
        replacement: Option<&BlockRecord>,
    ) {
        let transaction = store
            .begin_durable_write()
            .expect("begin history tamper transaction");
        let mut table = transaction
            .open_table(BLOCKS_BY_HEIGHT)
            .expect("open canonical block table");
        if let Some(block) = replacement {
            let bytes = encode_json(block, MAX_RECORD_BYTES).expect("encode tampered block");
            insert_authenticated(
                &mut table,
                BLOCKS_BY_HEIGHT_TABLE,
                &ordered_u64_key(height),
                &bytes,
                MAX_RECORD_BYTES,
                &store.integrity_key,
                &store.counters,
            )
            .expect("write authenticated history tamper");
        } else {
            table
                .remove(ordered_u64_key(height).as_slice())
                .expect("remove canonical history record");
        }
        drop(table);
        store
            .commit_durable_write(transaction)
            .expect("commit authenticated history tamper");
    }

    #[test]
    fn retained_history_prune_is_atomic_and_keeps_ordered_commitment() {
        let dir = TestDir::new("retained-prune");
        let store = TransactionalStore::open(&dir.0).expect("open retained prune store");
        let old_tip = genesis_tip();
        let old_commitment = OrderedHistoryCommitment::genesis(
            &old_tip.chain_id,
            &old_tip.genesis_hash,
            old_tip.protocol_version,
        )
        .expect("genesis commitment");
        store
            .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
            .expect("initialize retained prune store");
        let expected_tip = next_tip(1);
        let effect = confirmed_consensusless_fastpay_effect();
        let mut checkpoint_ledger = LedgerState::empty();
        checkpoint_ledger
            .fastpay_version_fences
            .push(effect.clone());
        let receipt = receipt("tx-1", true);
        let mut block = block(vec![receipt.tx_id.clone()]);
        block.fastpay_pre_state_effects.push(effect.clone());
        let archive = archive();
        let commitment = old_commitment
            .append("batch-1")
            .expect("append retained prune batch");
        store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &old_tip,
                new_tip: &expected_tip,
                block: &block,
                receipts: std::slice::from_ref(&receipt),
                archive_entry: &archive,
                batch_id: "batch-1",
                ordered_history: &commitment,
                current_state: CurrentStateUpdate {
                    ledger: Some(&checkpoint_ledger),
                    ..CurrentStateUpdate::default()
                },
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect("commit retained prune fixture");
        let bad_checkpoint =
            br#"{"schema":"postfiat-history-checkpoint-v2","pruned_up_to_height":0}"#;
        let original_meta = store.meta().expect("read original prune metadata");
        let error = store
            .prune_retained_history(PruneRetainedHistory {
                expected_tip: &expected_tip,
                new_history_base_height: 1,
                retained_checkpoint: bad_checkpoint,
            })
            .expect_err("mismatched checkpoint boundary must reject");
        assert_eq!(error.code(), StorageErrorCode::DomainMismatch);
        assert_eq!(
            store.meta().expect("read rejected prune metadata"),
            original_meta
        );
        assert!(store.block(1).expect("read unpruned block").is_some());

        let checkpoint = serde_json::to_vec(&serde_json::json!({
            "schema": "postfiat-history-checkpoint-v2",
            "pruned_up_to_height": 1,
            "ledger": checkpoint_ledger,
        }))
        .expect("encode retained checkpoint");
        let outcome = store
            .prune_retained_history(PruneRetainedHistory {
                expected_tip: &expected_tip,
                new_history_base_height: 1,
                retained_checkpoint: &checkpoint,
            })
            .expect("prune retained history");
        assert_eq!(outcome.previous_history_base_height, 0);
        assert_eq!(outcome.new_history_base_height, 1);
        assert_eq!(outcome.pruned_block_count, 1);
        assert_eq!(outcome.pruned_archive_count, 1);
        assert_eq!(outcome.pruned_receipt_count, 1);
        assert_eq!(outcome.remaining_receipt_count, 0);
        assert!(store.block(1).expect("read pruned block").is_none());
        assert!(store
            .block_height_by_hash("block-1")
            .expect("read pruned hash index")
            .is_none());
        assert!(store
            .archived_batch("payments", "batch-1")
            .expect("read pruned archive")
            .is_none());
        assert!(store
            .receipt("tx-1")
            .expect("read pruned receipt")
            .is_none());
        assert_eq!(
            store
                .ordered_batch_by_ordinal(1)
                .expect("read retained ordered batch"),
            Some("batch-1".to_owned())
        );
        assert_eq!(
            store
                .current_state_raw("retained_history_checkpoint")
                .expect("read retained checkpoint"),
            Some(checkpoint.to_vec())
        );
        assert_eq!(
            store
                .anchored_fastpay_effect(&effect.lock_id)
                .expect("read pruned FastPay anchor"),
            Some(effect)
        );
        let meta = store.meta().expect("read pruned metadata");
        assert_eq!(meta.history_base_height, 1);
        assert_eq!(meta.receipt_count, 0);
        assert_eq!(meta.last_full_verification_height, Some(1));
        let report = store
            .verify_logical_integrity()
            .expect("verify pruned logical store");
        assert_eq!(report.block_count, 0);
        assert_eq!(report.ordered_batch_count, 1);
    }

    #[test]
    fn retained_history_prune_rebuilds_fastpay_anchors_from_retained_suffix() {
        let (_dir, store) = committed_one_block("retained-suffix-fastpay-anchor");
        let old_tip = next_tip(1);
        let old_commitment = store
            .meta()
            .expect("read one-block metadata")
            .ordered_history_commitment();
        let effect = confirmed_consensusless_fastpay_effect();
        let mut ledger = LedgerState::empty();
        ledger.fastpay_version_fences.push(effect.clone());
        let receipt = receipt("tx-2", true);
        let new_tip = ChainTipState {
            height: 2,
            block_hash: "block-2".to_owned(),
            state_root: "state-2".to_owned(),
            ordered_batch_count: 2,
            receipt_count: 2,
            ..old_tip.clone()
        };
        let new_commitment = old_commitment
            .append("batch-2")
            .expect("append second batch");
        let mut block = block(vec![receipt.tx_id.clone()]);
        block.header.height = 2;
        block.header.parent_hash = old_tip.block_hash.clone();
        block.header.batch_id = "batch-2".to_owned();
        block.header.state_root = new_tip.state_root.clone();
        block.header.certificate_id = "certificate-2".to_owned();
        block.header.block_hash = new_tip.block_hash.clone();
        block.fastpay_pre_state_effects.push(effect.clone());
        let mut archive = archive();
        archive.batch_id = "batch-2".to_owned();
        archive.payload_hash = "payload-hash-2".to_owned();
        store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &old_tip,
                new_tip: &new_tip,
                block: &block,
                receipts: std::slice::from_ref(&receipt),
                archive_entry: &archive,
                batch_id: "batch-2",
                ordered_history: &new_commitment,
                current_state: CurrentStateUpdate {
                    ledger: Some(&ledger),
                    ..CurrentStateUpdate::default()
                },
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect("commit retained suffix FastPay effect");

        let checkpoint = serde_json::to_vec(&serde_json::json!({
            "schema": "postfiat-history-checkpoint-v2",
            "pruned_up_to_height": 1,
            "ledger": LedgerState::empty(),
        }))
        .expect("encode prefix checkpoint");
        store
            .prune_retained_history(PruneRetainedHistory {
                expected_tip: &new_tip,
                new_history_base_height: 1,
                retained_checkpoint: &checkpoint,
            })
            .expect("prune prefix while retaining suffix");

        assert!(store.block(1).expect("read pruned prefix").is_none());
        assert_eq!(store.block(2).expect("read retained suffix"), Some(block));
        assert_eq!(
            store
                .anchored_fastpay_effect(&effect.lock_id)
                .expect("read rebuilt retained-suffix FastPay anchor"),
            Some(effect)
        );
        let report = store
            .verify_logical_integrity()
            .expect("verify retained suffix after index rebuild");
        assert_eq!(report.finalized_height, 2);
        assert_eq!(
            store
                .meta()
                .expect("read pruned metadata")
                .history_base_height,
            1
        );
        assert_eq!(report.block_count, 1);
        assert_eq!(report.history_index_count, 1);
    }

    #[test]
    fn shared_store_registry_keeps_one_redb_owner_across_concurrent_reopens() {
        let dir = TestDir::new("shared-owner");
        let integrity_key = IntegrityKey::load_or_create(&dir.0).expect("load integrity key");
        let first =
            shared_transactional_store(&dir.0, integrity_key.clone()).expect("open shared store");
        let first_ptr = Arc::as_ptr(&first) as usize;
        drop(first);

        let owners = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for _ in 0..16 {
                let directory = dir.0.clone();
                let key = integrity_key.clone();
                handles.push(scope.spawn(move || {
                    shared_transactional_store(&directory, key).expect("reopen shared store")
                }));
            }
            handles
                .into_iter()
                .map(|handle| handle.join().expect("shared store thread"))
                .collect::<Vec<_>>()
        });
        assert!(owners
            .iter()
            .all(|owner| Arc::as_ptr(owner) as usize == first_ptr));
    }

    #[test]
    fn initialize_reopen_and_domain_binding() {
        let dir = TestDir::new("initialize");
        let tip = genesis_tip();
        let commitment = OrderedHistoryCommitment::genesis(
            &tip.chain_id,
            &tip.genesis_hash,
            tip.protocol_version,
        )
        .expect("genesis commitment");
        {
            let store = TransactionalStore::open(&dir.0).expect("open store");
            store
                .initialize(&tip, &commitment, CurrentStateUpdate::default())
                .expect("initialize store");
            assert_eq!(store.meta().expect("metadata").chain_id, tip.chain_id);
        }
        let store = TransactionalStore::open(&dir.0).expect("reopen store");
        assert_eq!(store.meta().expect("metadata").finalized_height, 0);

        let mut wrong_tip = tip.clone();
        wrong_tip.chain_id = "other-chain".to_owned();
        let wrong_commitment = OrderedHistoryCommitment::genesis(
            &wrong_tip.chain_id,
            &wrong_tip.genesis_hash,
            wrong_tip.protocol_version,
        )
        .expect("wrong commitment");
        let error = store
            .initialize(&wrong_tip, &wrong_commitment, CurrentStateUpdate::default())
            .expect_err("conflicting initialization must reject");
        assert_eq!(error.code(), StorageErrorCode::InitializationConflict);
    }

    #[test]
    fn read_snapshot_remains_bound_to_one_finalized_tip_across_commit() {
        let dir = TestDir::new("consistent-read-snapshot");
        let store = TransactionalStore::open(&dir.0).expect("open snapshot store");
        let old_tip = genesis_tip();
        let old_commitment = OrderedHistoryCommitment::genesis(
            &old_tip.chain_id,
            &old_tip.genesis_hash,
            old_tip.protocol_version,
        )
        .expect("snapshot genesis commitment");
        store
            .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
            .expect("initialize snapshot store");
        let snapshot = store
            .read_snapshot()
            .expect("open consistent read snapshot");

        let accepted = receipt("tx-1", true);
        let new_tip = next_tip(1);
        let new_commitment = old_commitment.append("batch-1").expect("append commitment");
        let block = block(vec![accepted.tx_id.clone()]);
        let archive = archive();
        store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &old_tip,
                new_tip: &new_tip,
                block: &block,
                receipts: std::slice::from_ref(&accepted),
                archive_entry: &archive,
                batch_id: "batch-1",
                ordered_history: &new_commitment,
                current_state: CurrentStateUpdate::default(),
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect("commit while read snapshot remains open");

        assert_eq!(snapshot.meta().finalized_height, 0);
        assert_eq!(snapshot.chain_tip(), old_tip);
        assert!(!snapshot
            .contains_ordered_batch("batch-1")
            .expect("snapshot membership"));
        assert!(snapshot.block(1).expect("snapshot block read").is_none());
        assert_eq!(store.meta().expect("live metadata").finalized_height, 1);
        assert!(store
            .contains_ordered_batch("batch-1")
            .expect("live membership"));
    }

    #[test]
    fn atomic_commit_preserves_literal_receipts_and_is_idempotent() {
        let dir = TestDir::new("commit");
        let store = TransactionalStore::open(&dir.0).expect("open store");
        let old_tip = genesis_tip();
        let old_commitment = OrderedHistoryCommitment::genesis(
            &old_tip.chain_id,
            &old_tip.genesis_hash,
            old_tip.protocol_version,
        )
        .expect("genesis commitment");
        store
            .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
            .expect("initialize");
        let receipts = vec![receipt("tx-accepted", true), receipt("tx-rejected", false)];
        let new_tip = next_tip(receipts.len() as u64);
        let new_commitment = old_commitment.append("batch-1").expect("append commitment");
        let block = block(
            receipts
                .iter()
                .map(|receipt| receipt.tx_id.clone())
                .collect(),
        );
        let archive = archive();
        let commit = CommitFinalizedBlock {
            expected_tip: &old_tip,
            new_tip: &new_tip,
            block: &block,
            receipts: &receipts,
            archive_entry: &archive,
            batch_id: "batch-1",
            ordered_history: &new_commitment,
            current_state: CurrentStateUpdate::default(),
            scheduled_activation_height: None,
            allow_legacy_receipt_id_mismatch: false,
        };
        assert_eq!(
            store.commit_finalized_block(commit).expect("commit block"),
            CommitOutcome::Committed
        );
        assert_eq!(
            store
                .commit_finalized_block(commit)
                .expect("idempotent retry"),
            CommitOutcome::AlreadyCommitted
        );
        assert!(
            store
                .receipt("tx-accepted")
                .expect("receipt")
                .unwrap()
                .accepted
        );
        assert!(
            !store
                .receipt("tx-rejected")
                .expect("receipt")
                .unwrap()
                .accepted
        );
        assert_eq!(store.block(1).expect("block"), Some(block));
        assert_eq!(
            store.ordered_batch_by_ordinal(1).expect("ordered batch"),
            Some("batch-1".to_owned())
        );
        assert_eq!(
            store
                .verify_logical_integrity()
                .expect("logical integrity")
                .finalized_height,
            1
        );
    }

    #[test]
    fn finalized_commit_indexes_fastpay_anchor_for_point_lookup() {
        let dir = TestDir::new("fastpay-anchor");
        let store = TransactionalStore::open(&dir.0).expect("open store");
        let old_tip = genesis_tip();
        let old_commitment = OrderedHistoryCommitment::genesis(
            &old_tip.chain_id,
            &old_tip.genesis_hash,
            old_tip.protocol_version,
        )
        .expect("genesis commitment");
        store
            .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
            .expect("initialize");
        let effect = FastPayVersionFenceV1 {
            schema: postfiat_types::FASTPAY_VERSION_FENCE_SCHEMA_V1.to_owned(),
            operation: postfiat_types::FastPayOperationKindV1::Transfer,
            origin: postfiat_types::FastPayFenceOriginV1::OrderedRecovery,
            committee_epoch: 1,
            registry_root: "11".repeat(48),
            lock_id: "22".repeat(48),
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "owned-input".to_owned(),
                version: 1,
            }],
            decision: postfiat_types::FastPayRecoveryDecisionV1::Cancelled,
            certificate: None,
            decided_at_height: 1,
            next_versions: vec![postfiat_types::OwnedObjectRef {
                id: "owned-input".to_owned(),
                version: 2,
            }],
        };
        let receipt = receipt("tx-1", true);
        let new_tip = next_tip(1);
        let new_commitment = old_commitment.append("batch-1").expect("append commitment");
        let mut block = block(vec![receipt.tx_id.clone()]);
        block.fastpay_pre_state_effects.push(effect.clone());
        let archive = archive();
        let commit = CommitFinalizedBlock {
            expected_tip: &old_tip,
            new_tip: &new_tip,
            block: &block,
            receipts: std::slice::from_ref(&receipt),
            archive_entry: &archive,
            batch_id: "batch-1",
            ordered_history: &new_commitment,
            current_state: CurrentStateUpdate::default(),
            scheduled_activation_height: None,
            allow_legacy_receipt_id_mismatch: false,
        };
        assert_eq!(
            store.commit_finalized_block(commit).expect("commit block"),
            CommitOutcome::Committed
        );
        assert_eq!(
            store
                .anchored_fastpay_effect(&effect.lock_id)
                .expect("read FastPay anchor"),
            Some(effect)
        );
        assert_eq!(
            store
                .commit_finalized_block(commit)
                .expect("idempotent retry"),
            CommitOutcome::AlreadyCommitted
        );
    }

    #[test]
    fn rejected_parent_or_commitment_leaves_genesis_tip() {
        let dir = TestDir::new("abort");
        let store = TransactionalStore::open(&dir.0).expect("open store");
        let old_tip = genesis_tip();
        let old_commitment = OrderedHistoryCommitment::genesis(
            &old_tip.chain_id,
            &old_tip.genesis_hash,
            old_tip.protocol_version,
        )
        .expect("genesis commitment");
        store
            .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
            .expect("initialize");
        let receipts = vec![receipt("tx-1", true)];
        let new_tip = next_tip(1);
        let wrong_commitment = old_commitment
            .append("different-batch")
            .expect("wrong commitment");
        let block = block(vec!["tx-1".to_owned()]);
        let archive = archive();
        let error = store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &old_tip,
                new_tip: &new_tip,
                block: &block,
                receipts: &receipts,
                archive_entry: &archive,
                batch_id: "batch-1",
                ordered_history: &wrong_commitment,
                current_state: CurrentStateUpdate::default(),
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect_err("wrong commitment must reject");
        assert_eq!(error.code(), StorageErrorCode::OrderedCommitmentMismatch);
        assert_eq!(store.meta().expect("metadata").finalized_height, 0);
        assert_eq!(store.block(1).expect("block"), None);
        assert_eq!(store.receipt("tx-1").expect("receipt"), None);
    }

    #[test]
    fn injected_disk_permission_write_and_sync_failures_preserve_the_old_tip() {
        for fault in [
            "disk_full",
            "permission_loss",
            "write_failure",
            "sync_failure",
        ] {
            let dir = TestDir::new(fault);
            let store = TransactionalStore::open(&dir.0).expect("open fault store");
            let old_tip = genesis_tip();
            let old_commitment = OrderedHistoryCommitment::genesis(
                &old_tip.chain_id,
                &old_tip.genesis_hash,
                old_tip.protocol_version,
            )
            .expect("fault genesis commitment");
            store
                .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
                .expect("initialize fault store");
            let accepted = receipt("tx-1", true);
            let new_tip = next_tip(1);
            let new_commitment = old_commitment
                .append("batch-1")
                .expect("fault next commitment");
            let block = block(vec![accepted.tx_id.clone()]);
            let archive = archive();

            let error = store
                .commit_finalized_block_with_precommit_hook(
                    CommitFinalizedBlock {
                        expected_tip: &old_tip,
                        new_tip: &new_tip,
                        block: &block,
                        receipts: std::slice::from_ref(&accepted),
                        archive_entry: &archive,
                        batch_id: "batch-1",
                        ordered_history: &new_commitment,
                        current_state: CurrentStateUpdate::default(),
                        scheduled_activation_height: None,
                        allow_legacy_receipt_id_mismatch: false,
                    },
                    || {
                        Err(StorageError::new(
                            StorageErrorCode::Database,
                            format!("injected_{fault}"),
                        ))
                    },
                )
                .expect_err("injected durable-write fault must reject");
            assert_eq!(error.reason_code(), "storage_database_error");
            assert!(error.to_string().contains(&format!("injected_{fault}")));
            assert_eq!(store.meta().expect("fault metadata").finalized_height, 0);
            assert_eq!(store.block(1).expect("fault block"), None);
            assert_eq!(store.receipt("tx-1").expect("fault receipt"), None);
            assert_eq!(
                store
                    .archived_batch("payments", "batch-1")
                    .expect("fault archive"),
                None
            );
            assert!(!store
                .contains_ordered_batch("batch-1")
                .expect("fault ordered ID"));
            assert_eq!(
                store
                    .verify_logical_integrity()
                    .expect("fault logical scan")
                    .finalized_height,
                0
            );
        }
    }

    #[test]
    fn dropped_write_transaction_exposes_only_the_old_tip_after_reopen() {
        let dir = TestDir::new("dropped-write");
        let old_tip = genesis_tip();
        let old_commitment = OrderedHistoryCommitment::genesis(
            &old_tip.chain_id,
            &old_tip.genesis_hash,
            old_tip.protocol_version,
        )
        .expect("genesis commitment");
        {
            let store = TransactionalStore::open(&dir.0).expect("open store");
            store
                .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
                .expect("initialize");

            let transaction = store.begin_durable_write().expect("begin write");
            let mut table = transaction
                .open_table(BLOCKS_BY_HEIGHT)
                .expect("open blocks table");
            let block = block(Vec::new());
            let block_bytes = encode_json(&block, MAX_RECORD_BYTES).expect("encode block");
            insert_authenticated(
                &mut table,
                BLOCKS_BY_HEIGHT_TABLE,
                &ordered_u64_key(1),
                &block_bytes,
                MAX_RECORD_BYTES,
                &store.integrity_key,
                &store.counters,
            )
            .expect("stage block");
            drop(table);
            drop(transaction);
        }

        let reopened = TransactionalStore::open(&dir.0).expect("reopen store");
        assert_eq!(reopened.meta().expect("metadata").finalized_height, 0);
        assert_eq!(reopened.block(1).expect("block lookup"), None);
    }

    #[test]
    fn authenticated_values_reject_cross_table_and_cross_key_substitution() {
        let dir = TestDir::new("substitution");
        let key = IntegrityKey::load_or_create(&dir.0).expect("integrity key");
        let encoded =
            authenticated_value(RECEIPTS_BY_ID_TABLE, b"tx-1", br#"{"accepted":true}"#, &key)
                .expect("encode authenticated value");

        let table_error = decode_authenticated_value(
            BATCH_ARCHIVE_TABLE,
            b"tx-1",
            &encoded,
            MAX_RECORD_BYTES,
            &key,
        )
        .expect_err("cross-table substitution must fail");
        assert_eq!(table_error.code(), StorageErrorCode::IntegrityFailure);

        let key_error = decode_authenticated_value(
            RECEIPTS_BY_ID_TABLE,
            b"tx-2",
            &encoded,
            MAX_RECORD_BYTES,
            &key,
        )
        .expect_err("cross-key substitution must fail");
        assert_eq!(key_error.code(), StorageErrorCode::IntegrityFailure);
    }

    #[test]
    fn logical_scan_rejects_forged_receipt_archive_and_state_values() {
        fn corrupt_value(
            store: &TransactionalStore,
            definition: TableDefinition<&[u8], &[u8]>,
            key: &[u8],
        ) {
            let transaction = store
                .begin_durable_write()
                .expect("begin forged-value transaction");
            let mut table = transaction
                .open_table(definition)
                .expect("open forged-value table");
            let mut raw = {
                let value = table
                    .get(key)
                    .expect("read forged-value target")
                    .expect("forged-value target exists");
                value.value().to_vec()
            };
            let byte = raw.last_mut().expect("authenticated value is nonempty");
            *byte ^= 1;
            table
                .insert(key, raw.as_slice())
                .expect("write forged value");
            drop(table);
            store
                .commit_durable_write(transaction)
                .expect("commit forged value");
        }

        for label in ["receipt", "archive", "state"] {
            let (_dir, store) = committed_one_block(&format!("forged-{label}"));
            if label == "state" {
                let transaction = store
                    .begin_durable_write()
                    .expect("begin current-state fixture");
                let mut table = transaction
                    .open_table(CURRENT_STATE)
                    .expect("open current-state fixture");
                insert_authenticated(
                    &mut table,
                    CURRENT_STATE_TABLE,
                    STATE_LEDGER.as_bytes(),
                    b"{}",
                    MAX_CURRENT_STATE_BYTES,
                    &store.integrity_key,
                    &store.counters,
                )
                .expect("write current-state fixture");
                drop(table);
                store
                    .commit_durable_write(transaction)
                    .expect("commit current-state fixture");
            }
            let original_meta = store.meta().expect("read forged-value metadata");
            match label {
                "receipt" => corrupt_value(&store, RECEIPTS_BY_ID, b"tx-1"),
                "archive" => corrupt_value(
                    &store,
                    BATCH_ARCHIVE,
                    &archive_key("payments", "batch-1").expect("archive key"),
                ),
                "state" => corrupt_value(&store, CURRENT_STATE, STATE_LEDGER.as_bytes()),
                _ => unreachable!(),
            }
            let error = store
                .verify_and_mark_full_integrity()
                .expect_err("forged authenticated value must fail closed");
            assert_eq!(
                error.code(),
                StorageErrorCode::IntegrityFailure,
                "{label} returned unexpected reason code: {error}"
            );
            assert_eq!(
                store.meta().expect("read rejected forged-value metadata"),
                original_meta,
                "{label} forgery received a verification marker"
            );
        }
    }

    #[test]
    fn durable_commit_survives_close_and_restart() {
        let dir = TestDir::new("restart");
        let old_tip = genesis_tip();
        let old_commitment = OrderedHistoryCommitment::genesis(
            &old_tip.chain_id,
            &old_tip.genesis_hash,
            old_tip.protocol_version,
        )
        .expect("genesis commitment");
        let receipts = vec![receipt("tx-1", true)];
        let new_tip = next_tip(1);
        let new_commitment = old_commitment.append("batch-1").expect("append commitment");
        let block = block(vec!["tx-1".to_owned()]);
        let archive = archive();
        {
            let store = TransactionalStore::open(&dir.0).expect("open store");
            store
                .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
                .expect("initialize");
            store
                .commit_finalized_block(CommitFinalizedBlock {
                    expected_tip: &old_tip,
                    new_tip: &new_tip,
                    block: &block,
                    receipts: &receipts,
                    archive_entry: &archive,
                    batch_id: "batch-1",
                    ordered_history: &new_commitment,
                    current_state: CurrentStateUpdate::default(),
                    scheduled_activation_height: None,
                    allow_legacy_receipt_id_mismatch: false,
                })
                .expect("commit block");
        }

        let reopened = TransactionalStore::open(&dir.0).expect("reopen store");
        assert_eq!(reopened.meta().expect("metadata").finalized_height, 1);
        assert_eq!(reopened.block(1).expect("block lookup"), Some(block));
        assert_eq!(
            reopened
                .verify_logical_integrity()
                .expect("logical integrity")
                .ordered_batch_count,
            1
        );
    }

    #[test]
    fn rejected_receipt_retry_preserves_both_literal_occurrences() {
        let dir = TestDir::new("receipt-retry");
        let store = TransactionalStore::open(&dir.0).expect("open store");
        let tip0 = genesis_tip();
        let commitment0 = OrderedHistoryCommitment::genesis(
            &tip0.chain_id,
            &tip0.genesis_hash,
            tip0.protocol_version,
        )
        .expect("genesis commitment");
        store
            .initialize(&tip0, &commitment0, CurrentStateUpdate::default())
            .expect("initialize");

        let rejected = receipt("stable-operation-id", false);
        let tip1 = next_tip(1);
        let commitment1 = commitment0.append("batch-1").expect("first append");
        let block1 = block(vec![rejected.tx_id.clone()]);
        let archive1 = archive();
        store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &tip0,
                new_tip: &tip1,
                block: &block1,
                receipts: std::slice::from_ref(&rejected),
                archive_entry: &archive1,
                batch_id: "batch-1",
                ordered_history: &commitment1,
                current_state: CurrentStateUpdate::default(),
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect("commit rejected occurrence");

        let accepted = receipt("stable-operation-id", true);
        let mut tip2 = tip1.clone();
        tip2.height = 2;
        tip2.block_hash = "block-2".to_owned();
        tip2.state_root = "state-2".to_owned();
        tip2.ordered_batch_count = 2;
        tip2.receipt_count = 2;
        let commitment2 = commitment1.append("batch-2").expect("second append");
        let mut block2 = block(vec![accepted.tx_id.clone()]);
        block2.header.height = 2;
        block2.header.parent_hash = "block-1".to_owned();
        block2.header.batch_id = "batch-2".to_owned();
        block2.header.state_root = "state-2".to_owned();
        block2.header.block_hash = "block-2".to_owned();
        let mut archive2 = archive();
        archive2.batch_id = "batch-2".to_owned();
        archive2.payload_hash = "payload-hash-2".to_owned();
        store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &tip1,
                new_tip: &tip2,
                block: &block2,
                receipts: std::slice::from_ref(&accepted),
                archive_entry: &archive2,
                batch_id: "batch-2",
                ordered_history: &commitment2,
                current_state: CurrentStateUpdate::default(),
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect("commit accepted retry");

        assert_eq!(
            store
                .receipt_at_height("stable-operation-id", 1)
                .expect("height-one receipt"),
            Some(rejected.clone())
        );
        assert_eq!(
            store
                .receipt_at_height("stable-operation-id", 2)
                .expect("height-two receipt"),
            Some(accepted.clone())
        );
        assert_eq!(
            store
                .receipt("stable-operation-id")
                .expect("terminal receipt"),
            Some(accepted)
        );
        assert_eq!(
            store
                .receipts_in_block_order()
                .expect("literal receipt history"),
            vec![rejected, receipt("stable-operation-id", true)]
        );
        assert_eq!(
            store
                .verify_logical_integrity()
                .expect("logical integrity")
                .receipt_count,
            2
        );
    }

    #[test]
    fn dropped_transaction_at_every_logical_write_cut_exposes_only_old_tip() {
        for cut in 0..=8 {
            let dir = TestDir::new(&format!("atomic-cut-{cut}"));
            let old_tip = genesis_tip();
            let old_commitment = OrderedHistoryCommitment::genesis(
                &old_tip.chain_id,
                &old_tip.genesis_hash,
                old_tip.protocol_version,
            )
            .expect("cut genesis commitment");
            {
                let store = TransactionalStore::open(&dir.0).expect("open cut store");
                store
                    .initialize(&old_tip, &old_commitment, CurrentStateUpdate::default())
                    .expect("initialize cut store");
                let transaction = store.begin_durable_write().expect("begin cut transaction");
                if cut >= 1 {
                    let mut table = transaction
                        .open_table(CURRENT_STATE)
                        .expect("open cut current state");
                    insert_authenticated(
                        &mut table,
                        CURRENT_STATE_TABLE,
                        STATE_LEDGER.as_bytes(),
                        b"{}",
                        MAX_CURRENT_STATE_BYTES,
                        &store.integrity_key,
                        &store.counters,
                    )
                    .expect("stage cut current state");
                }
                if cut >= 2 {
                    let history = StoredReceiptHistoryV1 {
                        schema: STORED_RECEIPT_HISTORY_SCHEMA.to_owned(),
                        occurrences: vec![StoredReceiptOccurrenceV1 {
                            finalized_height: 1,
                            receipt: receipt("tx-1", true),
                        }],
                    };
                    let mut table = transaction
                        .open_table(RECEIPTS_BY_ID)
                        .expect("open cut receipts");
                    insert_authenticated(
                        &mut table,
                        RECEIPTS_BY_ID_TABLE,
                        b"tx-1",
                        &encode_json(&history, MAX_RECORD_BYTES).expect("encode cut receipt"),
                        MAX_RECORD_BYTES,
                        &store.integrity_key,
                        &store.counters,
                    )
                    .expect("stage cut receipt");
                }
                if cut >= 3 {
                    let mut table = transaction
                        .open_table(BATCH_ARCHIVE)
                        .expect("open cut archive");
                    let archive = archive();
                    insert_authenticated(
                        &mut table,
                        BATCH_ARCHIVE_TABLE,
                        &archive_key("payments", "batch-1").expect("cut archive key"),
                        &encode_json(&archive, MAX_RECORD_BYTES).expect("encode cut archive"),
                        MAX_RECORD_BYTES,
                        &store.integrity_key,
                        &store.counters,
                    )
                    .expect("stage cut archive");
                }
                if cut >= 4 {
                    let record = StoredOrderedIdV1 {
                        schema: STORED_ORDERED_ID_SCHEMA.to_owned(),
                        ordinal: 1,
                        finalized_height: 1,
                    };
                    let mut table = transaction
                        .open_table(ORDERED_BY_ID)
                        .expect("open cut ordered ID");
                    insert_authenticated(
                        &mut table,
                        ORDERED_BY_ID_TABLE,
                        b"batch-1",
                        &encode_json(&record, MAX_RECORD_BYTES).expect("encode cut ordered ID"),
                        MAX_RECORD_BYTES,
                        &store.integrity_key,
                        &store.counters,
                    )
                    .expect("stage cut ordered ID");
                }
                if cut >= 5 {
                    let record = StoredOrderedOrdinalV1 {
                        schema: STORED_ORDERED_ORDINAL_SCHEMA.to_owned(),
                        batch_id: "batch-1".to_owned(),
                    };
                    let mut table = transaction
                        .open_table(ORDERED_BY_ORDINAL)
                        .expect("open cut ordered ordinal");
                    insert_authenticated(
                        &mut table,
                        ORDERED_BY_ORDINAL_TABLE,
                        &ordered_u64_key(1),
                        &encode_json(&record, MAX_RECORD_BYTES)
                            .expect("encode cut ordered ordinal"),
                        MAX_RECORD_BYTES,
                        &store.integrity_key,
                        &store.counters,
                    )
                    .expect("stage cut ordered ordinal");
                }
                if cut >= 6 {
                    let mut table = transaction
                        .open_table(BLOCKS_BY_HEIGHT)
                        .expect("open cut blocks");
                    insert_authenticated(
                        &mut table,
                        BLOCKS_BY_HEIGHT_TABLE,
                        &ordered_u64_key(1),
                        &encode_json(&block(vec!["tx-1".to_owned()]), MAX_RECORD_BYTES)
                            .expect("encode cut block"),
                        MAX_RECORD_BYTES,
                        &store.integrity_key,
                        &store.counters,
                    )
                    .expect("stage cut block");
                }
                if cut >= 7 {
                    let record = StoredBlockHeightV1 {
                        schema: STORED_BLOCK_HEIGHT_SCHEMA.to_owned(),
                        height: 1,
                    };
                    let mut table = transaction
                        .open_table(BLOCK_HEIGHT_BY_HASH)
                        .expect("open cut block hash");
                    insert_authenticated(
                        &mut table,
                        BLOCK_HEIGHT_BY_HASH_TABLE,
                        b"block-1",
                        &encode_json(&record, MAX_RECORD_BYTES).expect("encode cut block hash"),
                        MAX_RECORD_BYTES,
                        &store.integrity_key,
                        &store.counters,
                    )
                    .expect("stage cut block hash");
                }
                if cut >= 8 {
                    let commitment = old_commitment.append("batch-1").expect("cut commitment");
                    let meta = TransactionalStoreMetaV1::from_tip_and_commitment(
                        &next_tip(1),
                        &commitment,
                    );
                    let mut table = transaction.open_table(META).expect("open cut metadata");
                    insert_authenticated(
                        &mut table,
                        META_TABLE,
                        META_KEY,
                        &encode_json(&meta, MAX_META_BYTES).expect("encode cut metadata"),
                        MAX_META_BYTES,
                        &store.integrity_key,
                        &store.counters,
                    )
                    .expect("stage cut metadata");
                }
                drop(transaction);
            }

            let reopened = TransactionalStore::open(&dir.0).expect("reopen cut store");
            assert_eq!(reopened.meta().expect("cut metadata"), {
                let mut meta =
                    TransactionalStoreMetaV1::from_tip_and_commitment(&old_tip, &old_commitment);
                meta.scheduled_activation_height = None;
                meta
            });
            assert_eq!(reopened.block(1).expect("cut block"), None);
            assert_eq!(reopened.receipt("tx-1").expect("cut receipt"), None);
            assert_eq!(
                reopened
                    .archived_batch("payments", "batch-1")
                    .expect("cut archive"),
                None
            );
            assert!(!reopened
                .contains_ordered_batch("batch-1")
                .expect("cut ordered ID"));
            assert_eq!(
                reopened.current_state_raw(STATE_LEDGER).expect("cut state"),
                None
            );
        }
    }

    #[test]
    fn logical_scan_rejects_padded_reordered_duplicated_omitted_and_modified_history() {
        type TamperCase = (
            &'static str,
            StorageErrorCode,
            Box<dyn FnOnce(&TransactionalStore)>,
        );
        let cases: [TamperCase; 5] = [
            (
                "padded",
                StorageErrorCode::CountMismatch,
                Box::new(|store| {
                    let mut padded = block(Vec::new());
                    padded.header.height = 2;
                    padded.header.parent_hash = "block-1".to_owned();
                    padded.header.batch_id = "batch-2".to_owned();
                    padded.header.state_root = "state-2".to_owned();
                    padded.header.certificate_id = "certificate-2".to_owned();
                    padded.header.block_hash = "block-2".to_owned();
                    replace_authenticated_block(store, 2, Some(&padded));
                }),
            ),
            (
                "duplicated",
                StorageErrorCode::CountMismatch,
                Box::new(|store| {
                    let duplicated = store.block(1).expect("read block").expect("block one");
                    replace_authenticated_block(store, 2, Some(&duplicated));
                }),
            ),
            (
                "omitted",
                StorageErrorCode::CountMismatch,
                Box::new(|store| replace_authenticated_block(store, 1, None)),
            ),
            (
                "modified",
                StorageErrorCode::CorruptRecord,
                Box::new(|store| {
                    let mut modified = store.block(1).expect("read block").expect("block one");
                    modified.header.state_root = "substituted-state".to_owned();
                    replace_authenticated_block(store, 1, Some(&modified));
                }),
            ),
            (
                "reordered",
                StorageErrorCode::CorruptRecord,
                Box::new(|store| {
                    let first = store.block(1).expect("read first").expect("block one");
                    let second = store.block(2).expect("read second").expect("block two");
                    replace_authenticated_block(store, 1, Some(&second));
                    replace_authenticated_block(store, 2, Some(&first));
                }),
            ),
        ];

        for (label, expected_code, tamper) in cases {
            let (_dir, store) = if label == "reordered" {
                committed_two_blocks(label)
            } else {
                committed_one_block(label)
            };
            let original_meta = store.meta().expect("read pre-tamper metadata");
            tamper(&store);
            let error = store
                .verify_and_mark_full_integrity()
                .expect_err("authenticated history mutation must fail closed");
            assert_eq!(
                error.code(),
                expected_code,
                "{label} returned unexpected reason code: {error}"
            );
            assert_eq!(
                store.meta().expect("read rejected metadata"),
                original_meta,
                "{label} tamper received a verification marker"
            );
        }
    }

    #[test]
    fn logical_scan_rejects_authenticated_conflicting_ordered_index() {
        let (_dir, store) = committed_one_block("tampered-ordered-index");
        let transaction = store
            .begin_durable_write()
            .expect("begin tamper transaction");
        let mut table = transaction
            .open_table(ORDERED_BY_ORDINAL)
            .expect("open ordered ordinal table");
        let conflicting = StoredOrderedOrdinalV1 {
            schema: STORED_ORDERED_ORDINAL_SCHEMA.to_owned(),
            batch_id: "substituted-batch".to_owned(),
        };
        let bytes = encode_json(&conflicting, MAX_RECORD_BYTES).expect("encode conflicting index");
        insert_authenticated(
            &mut table,
            ORDERED_BY_ORDINAL_TABLE,
            &ordered_u64_key(1),
            &bytes,
            MAX_RECORD_BYTES,
            &store.integrity_key,
            &store.counters,
        )
        .expect("write authenticated conflicting index");
        drop(table);
        store
            .commit_durable_write(transaction)
            .expect("commit authenticated logical tamper");

        let error = store
            .verify_logical_integrity()
            .expect_err("conflicting authenticated index must fail full scan");
        assert_eq!(error.code(), StorageErrorCode::CorruptRecord);
        assert!(error
            .to_string()
            .contains("conflicts with its ordered ordinal"));
    }

    #[test]
    fn logical_scan_rejects_deleted_hash_index_without_blessing_metadata() {
        let (_dir, store) = committed_one_block("missing-hash-index");
        let original_meta = store.meta().expect("read pre-tamper metadata");
        let transaction = store
            .begin_durable_write()
            .expect("begin deletion transaction");
        let mut table = transaction
            .open_table(BLOCK_HEIGHT_BY_HASH)
            .expect("open block hash table");
        table
            .remove(b"block-1".as_slice())
            .expect("delete hash index");
        drop(table);
        store
            .commit_durable_write(transaction)
            .expect("commit index deletion");

        let error = store
            .verify_and_mark_full_integrity()
            .expect_err("missing index must not receive a verification marker");
        assert_eq!(error.code(), StorageErrorCode::CountMismatch);
        assert_eq!(
            store.meta().expect("read post-rejection metadata"),
            original_meta,
            "a rejected integrity scan must not mutate metadata"
        );
    }

    #[test]
    fn big_endian_height_and_ordinal_keys_preserve_numeric_order() {
        assert!(ordered_u64_key(1) < ordered_u64_key(2));
        assert!(ordered_u64_key(255) < ordered_u64_key(256));
        assert!(ordered_u64_key(u32::MAX as u64) < ordered_u64_key(u64::MAX));
    }

    #[test]
    fn retained_checkpoint_initialization_preserves_prefix_and_suffix_relations() {
        let dir = TestDir::new("retained-checkpoint");
        let store = TransactionalStore::open(&dir.0).expect("open retained store");
        let mut commitment = OrderedHistoryCommitment::genesis("test-chain", "genesis-hash", 1)
            .expect("retained genesis commitment");
        commitment = commitment
            .append("batch-prefix-1")
            .expect("append prefix one");
        commitment = commitment
            .append("batch-prefix-2")
            .expect("append prefix two");
        let checkpoint_tip = ChainTipState {
            height: 2,
            block_hash: "checkpoint-block-2".to_owned(),
            state_root: "checkpoint-state-2".to_owned(),
            ordered_batch_count: 2,
            receipt_count: 0,
            history_base_height: 2,
            ..genesis_tip()
        };
        let effect = confirmed_consensusless_fastpay_effect();
        let mut checkpoint_ledger = LedgerState::empty();
        checkpoint_ledger
            .fastpay_version_fences
            .push(effect.clone());
        let checkpoint_state = CurrentStateUpdate {
            ledger: Some(&checkpoint_ledger),
            ..CurrentStateUpdate::default()
        };
        store
            .initialize_from_retained_checkpoint(
                &checkpoint_tip,
                &commitment,
                &["batch-prefix-1".to_owned(), "batch-prefix-2".to_owned()],
                checkpoint_state,
            )
            .expect("initialize retained checkpoint");
        store
            .initialize_from_retained_checkpoint(
                &checkpoint_tip,
                &commitment,
                &["batch-prefix-1".to_owned(), "batch-prefix-2".to_owned()],
                checkpoint_state,
            )
            .expect("repeat retained checkpoint initialization");
        assert_eq!(
            store
                .anchored_fastpay_effect(&effect.lock_id)
                .expect("read retained FastPay anchor"),
            Some(effect)
        );
        let base_report = store
            .verify_logical_integrity()
            .expect("verify retained checkpoint base");
        assert_eq!(base_report.block_count, 0);
        assert_eq!(base_report.ordered_batch_count, 2);

        let receipt = receipt("tx-3", true);
        let mut suffix_block = block(vec![receipt.tx_id.clone()]);
        suffix_block.header.height = 3;
        suffix_block.header.parent_hash = checkpoint_tip.block_hash.clone();
        suffix_block.header.batch_id = "batch-3".to_owned();
        suffix_block.header.state_root = "state-3".to_owned();
        suffix_block.header.block_hash = "block-3".to_owned();
        let mut suffix_archive = archive();
        suffix_archive.batch_id = "batch-3".to_owned();
        let suffix_commitment = commitment.append("batch-3").expect("append suffix");
        let suffix_tip = ChainTipState {
            height: 3,
            block_hash: "block-3".to_owned(),
            state_root: "state-3".to_owned(),
            ordered_batch_count: 3,
            receipt_count: 1,
            history_base_height: 2,
            ..genesis_tip()
        };
        store
            .commit_finalized_block(CommitFinalizedBlock {
                expected_tip: &checkpoint_tip,
                new_tip: &suffix_tip,
                block: &suffix_block,
                receipts: std::slice::from_ref(&receipt),
                archive_entry: &suffix_archive,
                batch_id: "batch-3",
                ordered_history: &suffix_commitment,
                current_state: CurrentStateUpdate::default(),
                scheduled_activation_height: None,
                allow_legacy_receipt_id_mismatch: false,
            })
            .expect("commit retained suffix");
        let suffix_report = store
            .verify_logical_integrity()
            .expect("verify retained suffix");
        assert_eq!(suffix_report.block_count, 1);
        assert_eq!(suffix_report.ordered_batch_count, 3);
        assert_eq!(
            store
                .blocks_in_height_order()
                .expect("retained blocks")
                .len(),
            1
        );
    }

    include!("transactional/tamper_tests.rs");
}
