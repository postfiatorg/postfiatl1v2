use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

const COMPLETED_INDEX_SCHEMA: &str = "postfiat.certified_send_completed_index.v1";
const COMPLETED_INDEX_INTENT_SCHEMA: &str =
    "postfiat.certified_send_completed_index_intent.v1";
const COMPLETED_INDEX_FILE: &str = ".certified-send-completed-index-state.v1";
const COMPLETED_INDEX_INTENT_FILE: &str = ".certified-send-completed-index-intent.v1";
const COMPLETED_INDEX_MUTATION_LOCK_FILE: &str =
    ".certified-send-completed-index-mutation.lock";
const COMPLETED_INDEX_MAX_BYTES: u64 = 4 * 1024 * 1024;
const COMPLETED_INDEX_INTENT_MAX_BYTES: u64 = 64 * 1024;
const COMPLETED_INDEX_INTENT_BATCH_SCHEMA: &str =
    "postfiat.certified_send_completed_index_intent.v2";
const COMPLETED_INDEX_INTENT_MAX_OPERATIONS: usize = 32;
const CERTIFIED_SEND_MAX_COMPACTIONS_PER_RESUME: usize = 5;
const COMPLETED_INDEX_MAX_TRANSIENT_ENTRIES: usize =
    CERTIFIED_SEND_COMPLETED_TOMBSTONE_MAX_JOBS + CERTIFIED_SEND_OUTBOX_MAX_JOBS;
const COMPLETED_INDEX_VERIFY_REPORT_SCHEMA: &str =
    "postfiat.certified_send_completed_index_verify.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletedIndexEntryV1 {
    pub(super) block_height: u64,
    pub(super) job_id: String,
    pub(super) topology_id: String,
    pub(super) chain_id: String,
    pub(super) genesis_hash: String,
    pub(super) protocol_version: u32,
    pub(super) job_json_sha256: String,
    pub(super) batch_sha256: String,
    pub(super) certificate_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedDirectoryStampV1 {
    directory_exists: bool,
    device: u64,
    inode: u64,
    mtime_seconds: i64,
    mtime_nanoseconds: i64,
    ctime_seconds: i64,
    ctime_nanoseconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedIndexV1 {
    schema: String,
    entry_count: u64,
    entries: Vec<CompletedIndexEntryV1>,
    entries_checksum: String,
    completed_directory_stamp: CompletedDirectoryStampV1,
    completed_directory_stamp_checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedIndexIntentV1 {
    schema: String,
    operation: String,
    entry: CompletedIndexEntryV1,
    intent_checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedIndexIntentOp {
    operation: String,
    entry: CompletedIndexEntryV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedIndexBatchIntentV2 {
    schema: String,
    operations: Vec<CompletedIndexIntentOp>,
    intent_checksum: String,
}

enum CompletedIndexIntent {
    Single(CompletedIndexIntentV1),
    Batch(CompletedIndexBatchIntentV2),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct DurableCertifiedSendWorkReport {
    #[serde(default)]
    pub(super) tombstones_validated: u64,
    #[serde(default)]
    pub(super) files_read: u64,
    #[serde(default)]
    pub(super) bytes_hashed: u64,
    #[serde(default)]
    pub(super) index_files_read: u64,
    #[serde(default)]
    pub(super) index_bytes_read: u64,
    #[serde(default)]
    pub(super) completed_entries_enumerated: u64,
    #[serde(default)]
    pub(super) jobs_compacted: u64,
    #[serde(default)]
    pub(super) jobs_pruned: u64,
    #[serde(default)]
    pub(super) index_migration_performed: bool,
    #[serde(default)]
    pub(super) compaction_ms: f64,
    #[serde(default)]
    pub(super) validation_ms: f64,
    #[serde(default)]
    pub(super) prune_ms: f64,
}

impl DurableCertifiedSendWorkReport {
    fn record_tombstone_validation(
        &mut self,
        job_bytes: usize,
        batch_bytes: usize,
        certificate_bytes: usize,
    ) -> Result<(), String> {
        self.tombstones_validated = self
            .tombstones_validated
            .checked_add(1)
            .ok_or_else(|| "certified send tombstone validation counter overflow".to_string())?;
        self.files_read = self
            .files_read
            .checked_add(3)
            .ok_or_else(|| "certified send tombstone file-read counter overflow".to_string())?;
        let bytes = job_bytes
            .checked_add(batch_bytes)
            .and_then(|value| value.checked_add(certificate_bytes))
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| "certified send tombstone hashed-byte count overflow".to_string())?;
        self.bytes_hashed = self
            .bytes_hashed
            .checked_add(bytes)
            .ok_or_else(|| "certified send tombstone hashed-byte counter overflow".to_string())?;
        Ok(())
    }

    fn record_index_read(&mut self, bytes: usize) -> Result<(), String> {
        self.index_files_read = self
            .index_files_read
            .checked_add(1)
            .ok_or_else(|| "certified send index file-read counter overflow".to_string())?;
        self.index_bytes_read = self
            .index_bytes_read
            .checked_add(
                u64::try_from(bytes)
                    .map_err(|_| "certified send index byte count overflow".to_string())?,
            )
            .ok_or_else(|| "certified send index byte counter overflow".to_string())?;
        Ok(())
    }

    fn record_completed_enumeration(&mut self) -> Result<(), String> {
        self.completed_entries_enumerated = self
            .completed_entries_enumerated
            .checked_add(1)
            .ok_or_else(|| "certified send completed enumeration counter overflow".to_string())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct CompletedIndexCompactionReport {
    pub(super) compacted: usize,
    pub(super) pruned: usize,
    pub(super) work: DurableCertifiedSendWorkReport,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CompletedIndexVerifyReport {
    schema: String,
    index_file: String,
    intent_file: String,
    entry_count: usize,
    rebuilt: bool,
    work: DurableCertifiedSendWorkReport,
}

#[derive(Debug)]
struct CompletedIndexMutationGuard {
    _file: File,
}

#[derive(Debug)]
struct ValidatedCompletedJob {
    #[cfg(test)]
    directory: PathBuf,
    #[cfg(test)]
    job: DurableCertifiedSendJob,
    entry: CompletedIndexEntryV1,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn canonical_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn entry_cmp(
    left: &CompletedIndexEntryV1,
    right: &CompletedIndexEntryV1,
) -> std::cmp::Ordering {
    left.block_height
        .cmp(&right.block_height)
        .then_with(|| left.job_id.cmp(&right.job_id))
}

fn validate_entry(entry: &CompletedIndexEntryV1) -> Result<(), String> {
    if entry.block_height == 0
        || !certified_send_job_id_is_canonical(&entry.job_id)
        || entry.topology_id.trim().is_empty()
        || entry.chain_id.trim().is_empty()
        || entry.genesis_hash.trim().is_empty()
        || !canonical_hash(&entry.job_json_sha256)
        || !canonical_hash(&entry.batch_sha256)
        || !canonical_hash(&entry.certificate_sha256)
    {
        return Err(format!(
            "certified send completed index entry `{}` is invalid",
            entry.job_id
        ));
    }
    Ok(())
}

fn entries_checksum(entries: &[CompletedIndexEntryV1]) -> Result<String, String> {
    let encoded = serde_json::to_vec(entries)
        .map_err(|error| format!("certified send completed index checksum encoding failed: {error}"))?;
    Ok(sha256_hex(&encoded))
}

fn directory_stamp_checksum(stamp: &CompletedDirectoryStampV1) -> Result<String, String> {
    let encoded = serde_json::to_vec(stamp).map_err(|error| {
        format!("certified send completed directory stamp checksum encoding failed: {error}")
    })?;
    Ok(sha256_hex(&encoded))
}

fn validate_directory_stamp(stamp: &CompletedDirectoryStampV1) -> Result<(), String> {
    let nanoseconds_valid = |value: i64| (0..1_000_000_000).contains(&value);
    if stamp.directory_exists {
        if stamp.inode == 0
            || !nanoseconds_valid(stamp.mtime_nanoseconds)
            || !nanoseconds_valid(stamp.ctime_nanoseconds)
        {
            return Err("certified send completed directory stamp is invalid".to_string());
        }
    } else if stamp.device != 0
        || stamp.inode != 0
        || stamp.mtime_seconds != 0
        || stamp.mtime_nanoseconds != 0
        || stamp.ctime_seconds != 0
        || stamp.ctime_nanoseconds != 0
    {
        return Err("certified send absent-directory stamp is invalid".to_string());
    }
    Ok(())
}

fn validate_index(index: &CompletedIndexV1) -> Result<(), String> {
    if index.schema != COMPLETED_INDEX_SCHEMA {
        return Err(format!(
            "certified send completed index schema `{}` is not supported",
            index.schema
        ));
    }
    validate_directory_stamp(&index.completed_directory_stamp)?;
    if index.completed_directory_stamp_checksum
        != directory_stamp_checksum(&index.completed_directory_stamp)?
    {
        return Err("certified send completed directory stamp self-checksum mismatch".to_string());
    }
    if usize::try_from(index.entry_count).ok() != Some(index.entries.len()) {
        return Err("certified send completed index entry count mismatch".to_string());
    }
    if index.entries.len() > COMPLETED_INDEX_MAX_TRANSIENT_ENTRIES {
        return Err(format!(
            "certified send completed index contains {} entries, exceeding transient bound {COMPLETED_INDEX_MAX_TRANSIENT_ENTRIES}",
            index.entries.len()
        ));
    }
    let mut prior: Option<&CompletedIndexEntryV1> = None;
    let mut job_ids = BTreeSet::new();
    for entry in &index.entries {
        validate_entry(entry)?;
        if !job_ids.insert(entry.job_id.as_str()) {
            return Err(format!(
                "certified send completed index repeats job `{}`",
                entry.job_id
            ));
        }
        if prior.is_some_and(|value| entry_cmp(value, entry).is_ge()) {
            return Err("certified send completed index order is non-canonical".to_string());
        }
        prior = Some(entry);
    }
    let expected_checksum = entries_checksum(&index.entries)?;
    if index.entries_checksum != expected_checksum {
        return Err("certified send completed index self-checksum mismatch".to_string());
    }
    Ok(())
}

fn completed_index_path(data_dir: &Path) -> PathBuf {
    data_dir.join(COMPLETED_INDEX_FILE)
}

fn completed_index_intent_path(data_dir: &Path) -> PathBuf {
    data_dir.join(COMPLETED_INDEX_INTENT_FILE)
}

#[cfg(unix)]
fn completed_directory_stamp(data_dir: &Path) -> Result<CompletedDirectoryStampV1, String> {
    let path = certified_send_completed_dir(data_dir);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(CompletedDirectoryStampV1 {
                directory_exists: false,
                device: 0,
                inode: 0,
                mtime_seconds: 0,
                mtime_nanoseconds: 0,
                ctime_seconds: 0,
                ctime_nanoseconds: 0,
            });
        }
        Err(error) => {
            return Err(format!(
                "certified send completed directory stamp `{}` failed: {error}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "certified send completed directory `{}` must be a non-symlink directory",
            path.display()
        ));
    }
    Ok(CompletedDirectoryStampV1 {
        directory_exists: true,
        device: metadata.dev(),
        inode: metadata.ino(),
        mtime_seconds: metadata.mtime(),
        mtime_nanoseconds: metadata.mtime_nsec(),
        ctime_seconds: metadata.ctime(),
        ctime_nanoseconds: metadata.ctime_nsec(),
    })
}

#[cfg(not(unix))]
fn completed_directory_stamp(_data_dir: &Path) -> Result<CompletedDirectoryStampV1, String> {
    Err("certified send completed directory stamping requires Unix metadata".to_string())
}

fn require_completed_directory_stamp(
    data_dir: &Path,
    index: &CompletedIndexV1,
) -> Result<(), String> {
    let observed = completed_directory_stamp(data_dir)?;
    if observed != index.completed_directory_stamp {
        return Err(
            "certified send completed index/directory divergence requires explicit verify repair"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn completed_index_path_for_test(data_dir: &Path) -> PathBuf {
    completed_index_path(data_dir)
}

#[cfg(test)]
pub(super) fn completed_index_intent_path_for_test(data_dir: &Path) -> PathBuf {
    completed_index_intent_path(data_dir)
}

fn read_bounded_regular_file(path: &Path, max_bytes: u64, label: &str) -> Result<Option<Vec<u8>>, String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "certified send {label} metadata `{}` failed: {error}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "certified send {label} `{}` must be a non-symlink regular file",
            path.display()
        ));
    }
    if metadata.len() > max_bytes {
        return Err(format!(
            "certified send {label} `{}` exceeds {max_bytes} bytes",
            path.display()
        ));
    }
    std::fs::read(path)
        .map(Some)
        .map_err(|error| format!("certified send {label} read `{}` failed: {error}", path.display()))
}

fn read_index(
    data_dir: &Path,
    work: &mut DurableCertifiedSendWorkReport,
) -> Result<Option<CompletedIndexV1>, String> {
    let path = completed_index_path(data_dir);
    let Some(bytes) = read_bounded_regular_file(&path, COMPLETED_INDEX_MAX_BYTES, "completed index")?
    else {
        return Ok(None);
    };
    work.record_index_read(bytes.len())?;
    let index: CompletedIndexV1 = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "certified send completed index parse `{}` failed: {error}",
            path.display()
        )
    })?;
    validate_index(&index)?;
    Ok(Some(index))
}

fn write_index(data_dir: &Path, entries: &mut Vec<CompletedIndexEntryV1>) -> Result<(), String> {
    entries.sort_by(entry_cmp);
    let completed_directory_stamp = completed_directory_stamp(data_dir)?;
    let index = CompletedIndexV1 {
        schema: COMPLETED_INDEX_SCHEMA.to_string(),
        entry_count: u64::try_from(entries.len())
            .map_err(|_| "certified send completed index entry count overflow".to_string())?,
        entries: entries.clone(),
        entries_checksum: entries_checksum(entries)?,
        completed_directory_stamp_checksum: directory_stamp_checksum(
            &completed_directory_stamp,
        )?,
        completed_directory_stamp,
    };
    validate_index(&index)?;
    // Compact encoding: the index is machine-owned durable state read and
    // rewritten on every resume; at the 1,024-entry retention cap the pretty
    // form costs ~25% extra bytes on every parse, serialize, and fsync.
    let bytes = serde_json::to_vec(&index)
        .map_err(|error| format!("certified send completed index serialization failed: {error}"))?;
    if bytes.len() as u64 > COMPLETED_INDEX_MAX_BYTES {
        return Err(format!(
            "certified send completed index exceeds {COMPLETED_INDEX_MAX_BYTES} bytes"
        ));
    }
    postfiat_storage::atomic_write(completed_index_path(data_dir), bytes)
        .map_err(|error| format!("certified send completed index atomic write failed: {error}"))
}

fn intent_checksum(
    schema: &str,
    operation: &str,
    entry: &CompletedIndexEntryV1,
) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(schema, operation, entry))
        .map_err(|error| format!("certified send completed intent checksum encoding failed: {error}"))?;
    Ok(sha256_hex(&encoded))
}

#[cfg(test)]
fn write_intent(
    data_dir: &Path,
    operation: &str,
    entry: &CompletedIndexEntryV1,
) -> Result<(), String> {
    if !matches!(operation, "append" | "prune") {
        return Err("certified send completed intent operation is invalid".to_string());
    }
    validate_entry(entry)?;
    let intent = CompletedIndexIntentV1 {
        schema: COMPLETED_INDEX_INTENT_SCHEMA.to_string(),
        operation: operation.to_string(),
        entry: entry.clone(),
        intent_checksum: intent_checksum(COMPLETED_INDEX_INTENT_SCHEMA, operation, entry)?,
    };
    let bytes = serde_json::to_vec_pretty(&intent)
        .map_err(|error| format!("certified send completed intent serialization failed: {error}"))?;
    if bytes.len() as u64 > COMPLETED_INDEX_INTENT_MAX_BYTES {
        return Err(format!(
            "certified send completed intent exceeds {COMPLETED_INDEX_INTENT_MAX_BYTES} bytes"
        ));
    }
    postfiat_storage::atomic_write(completed_index_intent_path(data_dir), bytes)
        .map_err(|error| format!("certified send completed intent atomic write failed: {error}"))
}

fn batch_intent_checksum(
    schema: &str,
    operations: &[CompletedIndexIntentOp],
) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(schema, operations)).map_err(|error| {
        format!("certified send completed batch intent checksum encoding failed: {error}")
    })?;
    Ok(sha256_hex(&encoded))
}

fn write_batch_intent(
    data_dir: &Path,
    operations: &[CompletedIndexIntentOp],
) -> Result<(), String> {
    if operations.is_empty() || operations.len() > COMPLETED_INDEX_INTENT_MAX_OPERATIONS {
        return Err(format!(
            "certified send completed batch intent must contain between 1 and {COMPLETED_INDEX_INTENT_MAX_OPERATIONS} operations"
        ));
    }
    for op in operations {
        if !matches!(op.operation.as_str(), "append" | "prune") {
            return Err("certified send completed intent operation is invalid".to_string());
        }
        validate_entry(&op.entry)?;
    }
    let intent = CompletedIndexBatchIntentV2 {
        schema: COMPLETED_INDEX_INTENT_BATCH_SCHEMA.to_string(),
        operations: operations.to_vec(),
        intent_checksum: batch_intent_checksum(COMPLETED_INDEX_INTENT_BATCH_SCHEMA, operations)?,
    };
    let bytes = serde_json::to_vec_pretty(&intent).map_err(|error| {
        format!("certified send completed batch intent serialization failed: {error}")
    })?;
    if bytes.len() as u64 > COMPLETED_INDEX_INTENT_MAX_BYTES {
        return Err(format!(
            "certified send completed batch intent exceeds {COMPLETED_INDEX_INTENT_MAX_BYTES} bytes"
        ));
    }
    postfiat_storage::atomic_write(completed_index_intent_path(data_dir), bytes).map_err(
        |error| format!("certified send completed batch intent atomic write failed: {error}"),
    )
}

fn read_intent(data_dir: &Path) -> Result<Option<CompletedIndexIntent>, String> {
    let path = completed_index_intent_path(data_dir);
    let Some(bytes) = read_bounded_regular_file(
        &path,
        COMPLETED_INDEX_INTENT_MAX_BYTES,
        "completed index intent",
    )?
    else {
        return Ok(None);
    };
    let probe: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "certified send completed intent parse `{}` failed: {error}",
            path.display()
        )
    })?;
    let schema = probe
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if schema == COMPLETED_INDEX_INTENT_BATCH_SCHEMA {
        let intent: CompletedIndexBatchIntentV2 =
            serde_json::from_value(probe).map_err(|error| {
                format!(
                    "certified send completed batch intent parse `{}` failed: {error}",
                    path.display()
                )
            })?;
        if intent.operations.is_empty()
            || intent.operations.len() > COMPLETED_INDEX_INTENT_MAX_OPERATIONS
        {
            return Err(
                "certified send completed batch intent operation count is invalid".to_string(),
            );
        }
        for op in &intent.operations {
            if !matches!(op.operation.as_str(), "append" | "prune") {
                return Err(
                    "certified send completed intent operation is invalid".to_string()
                );
            }
            validate_entry(&op.entry)?;
        }
        let expected = batch_intent_checksum(&intent.schema, &intent.operations)?;
        if intent.intent_checksum != expected {
            return Err("certified send completed intent self-checksum mismatch".to_string());
        }
        return Ok(Some(CompletedIndexIntent::Batch(intent)));
    }
    let intent: CompletedIndexIntentV1 = serde_json::from_value(probe).map_err(|error| {
        format!(
            "certified send completed intent parse `{}` failed: {error}",
            path.display()
        )
    })?;
    if intent.schema != COMPLETED_INDEX_INTENT_SCHEMA
        || !matches!(intent.operation.as_str(), "append" | "prune")
    {
        return Err("certified send completed intent schema or operation is invalid".to_string());
    }
    validate_entry(&intent.entry)?;
    let expected = intent_checksum(&intent.schema, &intent.operation, &intent.entry)?;
    if intent.intent_checksum != expected {
        return Err("certified send completed intent self-checksum mismatch".to_string());
    }
    Ok(Some(CompletedIndexIntent::Single(intent)))
}

fn clear_intent(data_dir: &Path) -> Result<(), String> {
    let path = completed_index_intent_path(data_dir);
    match std::fs::remove_file(&path) {
        Ok(()) => sync_certified_send_directory(data_dir, "data directory after intent removal"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "certified send completed intent remove `{}` failed: {error}",
            path.display()
        )),
    }
}

/// Removes a fully-applied batch intent without forcing the removal durable.
/// Safe only after every operation in the batch is reflected in the durable
/// index: replaying a fully-applied batch intent is idempotent (appends see
/// indexed destinations, prunes see absent sources), so a crash that
/// resurrects the cleared intent recovers to the same state.
fn clear_intent_unsynced(data_dir: &Path) -> Result<(), String> {
    let path = completed_index_intent_path(data_dir);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "certified send completed intent remove `{}` failed: {error}",
            path.display()
        )),
    }
}

fn acquire_mutation_guard(data_dir: &Path) -> Result<CompletedIndexMutationGuard, String> {
    std::fs::create_dir_all(data_dir).map_err(|error| {
        format!(
            "certified send data directory create `{}` failed: {error}",
            data_dir.display()
        )
    })?;
    let path = data_dir.join(COMPLETED_INDEX_MUTATION_LOCK_FILE);
    #[cfg(unix)]
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .map_err(|error| format!("certified send mutation lock open failed: {error}"))?;
    #[cfg(not(unix))]
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| format!("certified send mutation lock open failed: {error}"))?;

    #[cfg(unix)]
    loop {
        // SAFETY: the descriptor remains open in the guard for the complete
        // migration, reconciliation, compaction, and prune critical section.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if result == 0 {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!(
                "certified send mutation lock `{}` failed: {error}",
                path.display()
            ));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        return Err(
            "cross-process certified send index mutation locking requires Unix flock".to_string(),
        );
    }

    Ok(CompletedIndexMutationGuard { _file: file })
}

fn completed_directory_names(
    data_dir: &Path,
    work: &mut DurableCertifiedSendWorkReport,
) -> Result<Vec<String>, String> {
    let directory = certified_send_completed_dir(data_dir);
    if !require_certified_send_directory(&directory, "completed directory")? {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&directory)
        .map_err(|error| format!("certified send completed index scan failed: {error}"))?
    {
        let entry = entry
            .map_err(|error| format!("certified send completed index entry failed: {error}"))?;
        work.record_completed_enumeration()?;
        let name = entry.file_name();
        let job_id = name.to_str().ok_or_else(|| {
            format!(
                "certified send completed entry `{}` is not valid UTF-8",
                entry.path().display()
            )
        })?;
        if !certified_send_job_id_is_canonical(job_id) {
            return Err(format!(
                "certified send completed entry `{}` has a non-canonical name",
                entry.path().display()
            ));
        }
        if !require_certified_send_directory(&entry.path(), "completed job directory")? {
            return Err(format!(
                "certified send completed job directory `{}` disappeared during index scan",
                entry.path().display()
            ));
        }
        names.push(job_id.to_string());
        if names.len() > COMPLETED_INDEX_MAX_TRANSIENT_ENTRIES {
            return Err(format!(
                "certified send completed directory exceeds transient bound {COMPLETED_INDEX_MAX_TRANSIENT_ENTRIES}"
            ));
        }
    }
    names.sort();
    Ok(names)
}

fn entry_from_validated_job(
    job: &DurableCertifiedSendJob,
    job_bytes: &[u8],
    batch_bytes: &[u8],
    certificate_bytes: &[u8],
) -> CompletedIndexEntryV1 {
    CompletedIndexEntryV1 {
        block_height: job.block_height,
        job_id: job.job_id.clone(),
        topology_id: job.topology_id.clone(),
        chain_id: job.chain_id.clone(),
        genesis_hash: job.genesis_hash.clone(),
        protocol_version: job.protocol_version,
        job_json_sha256: sha256_hex(job_bytes),
        batch_sha256: sha256_hex(batch_bytes),
        certificate_sha256: sha256_hex(certificate_bytes),
    }
}

fn validate_job_directory(
    directory: &Path,
    expected_job_id: &str,
    work: &mut DurableCertifiedSendWorkReport,
) -> Result<ValidatedCompletedJob, String> {
    if !require_certified_send_directory(directory, "completed job directory")? {
        return Err(format!(
            "certified send completed job directory `{}` is missing",
            directory.display()
        ));
    }
    let job_file = directory.join("job.json");
    let (job, job_bytes) = read_durable_certified_send_job_with_bytes(&job_file)?;
    if job.job_id != expected_job_id {
        return Err(format!(
            "certified send completed directory `{expected_job_id}` conflicts with job id `{}`",
            job.job_id
        ));
    }
    validate_completed_durable_certified_send_job_metadata(&job)?;
    let validation_start = Instant::now();
    let (batch_bytes, certificate_bytes) =
        read_validated_durable_certified_send_payloads(&job_file, &job)?;
    work.validation_ms += monotonic_elapsed_ms(validation_start);
    work.record_tombstone_validation(
        job_bytes.len(),
        batch_bytes.len(),
        certificate_bytes.len(),
    )?;
    let entry = entry_from_validated_job(&job, &job_bytes, &batch_bytes, &certificate_bytes);
    Ok(ValidatedCompletedJob {
        #[cfg(test)]
        directory: directory.to_path_buf(),
        #[cfg(test)]
        job,
        entry,
    })
}

fn validate_job_directory_and_quarantine(
    data_dir: &Path,
    directory: &Path,
    expected_job_id: &str,
    work: &mut DurableCertifiedSendWorkReport,
    context: &str,
) -> Result<ValidatedCompletedJob, String> {
    match validate_job_directory(directory, expected_job_id, work) {
        Ok(validated) => Ok(validated),
        Err(detail) => {
            let error = format!(
                "certified send completed job `{expected_job_id}` is invalid during {context}: {detail}"
            );
            let job_file = directory.join("job.json");
            quarantine_durable_certified_send_job(
                data_dir,
                expected_job_id,
                &job_file,
                &error,
            )?;
            Err(error)
        }
    }
}

fn fully_validate_completed_jobs(
    data_dir: &Path,
    work: &mut DurableCertifiedSendWorkReport,
    context: &str,
) -> Result<Vec<ValidatedCompletedJob>, String> {
    let names = completed_directory_names(data_dir, work)?;
    let completed_dir = certified_send_completed_dir(data_dir);
    let mut completed = Vec::with_capacity(names.len());
    for job_id in names {
        completed.push(validate_job_directory_and_quarantine(
            data_dir,
            &completed_dir.join(&job_id),
            &job_id,
            work,
            context,
        )?);
    }
    completed.sort_by(|left, right| entry_cmp(&left.entry, &right.entry));
    Ok(completed)
}

fn ensure_index(
    data_dir: &Path,
    work: &mut DurableCertifiedSendWorkReport,
) -> Result<CompletedIndexV1, String> {
    if let Some(index) = read_index(data_dir, work)? {
        return Ok(index);
    }
    if read_intent(data_dir)?.is_some() {
        return Err(
            "certified send completed index is missing while a mutation intent exists; explicit verify repair is required"
                .to_string(),
        );
    }
    let completed = fully_validate_completed_jobs(data_dir, work, "index migration")?;
    let mut entries = completed
        .into_iter()
        .map(|validated| validated.entry)
        .collect::<Vec<_>>();
    write_index(data_dir, &mut entries)?;
    work.index_migration_performed = true;
    read_index(data_dir, work)?.ok_or_else(|| {
        "certified send completed index disappeared after migration".to_string()
    })
}

fn index_entry_position(index: &CompletedIndexV1, job_id: &str) -> Option<usize> {
    index.entries.iter().position(|entry| entry.job_id == job_id)
}

fn require_validated_matches_entry(
    validated: &ValidatedCompletedJob,
    entry: &CompletedIndexEntryV1,
) -> Result<(), String> {
    if &validated.entry != entry {
        return Err(format!(
            "certified send completed intent for `{}` conflicts with validated job bytes",
            entry.job_id
        ));
    }
    Ok(())
}

fn sync_append_move(data_dir: &Path) -> Result<(), String> {
    sync_certified_send_directory(
        &certified_send_outbox_dir(data_dir),
        "outbox directory after completed move",
    )?;
    sync_certified_send_directory(
        &certified_send_completed_dir(data_dir),
        "completed directory after completed move",
    )
}

fn retention_destination(data_dir: &Path, job_id: &str) -> PathBuf {
    certified_send_completed_retention_dir(data_dir).join(job_id)
}

fn ensure_retention_directory(data_dir: &Path) -> Result<(), String> {
    let outbox_dir = certified_send_outbox_dir(data_dir);
    let resolved_dir = certified_send_resolved_quarantine_dir(data_dir);
    let retention_dir = certified_send_completed_retention_dir(data_dir);
    std::fs::create_dir_all(&retention_dir).map_err(|error| {
        format!(
            "certified send completed retention directory create `{}` failed: {error}",
            retention_dir.display()
        )
    })?;
    sync_certified_send_directory(&outbox_dir, "outbox directory after retention create")?;
    sync_certified_send_directory(
        &resolved_dir,
        "resolved quarantine directory after retention create",
    )
}

fn finish_prune_retention(data_dir: &Path, entry: &CompletedIndexEntryV1) -> Result<(), String> {
    let destination = retention_destination(data_dir, &entry.job_id);
    if destination.exists() {
        remove_certified_send_disposable_job_dir(&destination, &entry.job_id)?;
    }
    Ok(())
}

fn reconcile_append_op(
    data_dir: &Path,
    index: &mut CompletedIndexV1,
    entry: &CompletedIndexEntryV1,
    work: &mut DurableCertifiedSendWorkReport,
) -> Result<(), String> {
    let source = certified_send_outbox_dir(data_dir).join(&entry.job_id);
    let destination = certified_send_completed_dir(data_dir).join(&entry.job_id);
    let source_exists = source.exists();
    let destination_exists = destination.exists();
    let index_position = index_entry_position(index, &entry.job_id);
    if let Some(position) = index_position {
        if &index.entries[position] != entry || source_exists || !destination_exists {
            return Err(format!(
                "certified send append intent for `{}` conflicts with indexed filesystem state",
                entry.job_id
            ));
        }
        return Ok(());
    }
    if source_exists == destination_exists {
        return Err(format!(
            "certified send append intent for `{}` has ambiguous source/destination state",
            entry.job_id
        ));
    }
    let current = if source_exists { &source } else { &destination };
    let validated = validate_job_directory_and_quarantine(
        data_dir,
        current,
        &entry.job_id,
        work,
        "append-intent recovery",
    )?;
    require_validated_matches_entry(&validated, entry)?;
    if source_exists {
        std::fs::create_dir_all(certified_send_completed_dir(data_dir)).map_err(|error| {
            format!("certified send completed directory create failed: {error}")
        })?;
        std::fs::rename(&source, &destination).map_err(|error| {
            format!(
                "certified send completed append recovery move `{}` failed: {error}",
                entry.job_id
            )
        })?;
        sync_append_move(data_dir)?;
    }
    index.entries.push(entry.clone());
    Ok(())
}

fn reconcile_prune_op(
    data_dir: &Path,
    index: &mut CompletedIndexV1,
    entry: &CompletedIndexEntryV1,
    work: &mut DurableCertifiedSendWorkReport,
) -> Result<(), String> {
    let source = certified_send_completed_dir(data_dir).join(&entry.job_id);
    let destination = retention_destination(data_dir, &entry.job_id);
    let source_exists = source.exists();
    let destination_exists = destination.exists();
    if source_exists && destination_exists {
        return Err(format!(
            "certified send prune intent for `{}` has both source and retention destination",
            entry.job_id
        ));
    }
    let index_position = index_entry_position(index, &entry.job_id);
    if let Some(position) = index_position {
        if &index.entries[position] != entry {
            return Err(format!(
                "certified send prune intent for `{}` conflicts with index entry",
                entry.job_id
            ));
        }
        if source_exists || destination_exists {
            let current = if source_exists { &source } else { &destination };
            let validated = validate_job_directory_and_quarantine(
                data_dir,
                current,
                &entry.job_id,
                work,
                "prune-intent recovery",
            )?;
            require_validated_matches_entry(&validated, entry)?;
        }
        if source_exists {
            ensure_retention_directory(data_dir)?;
            std::fs::rename(&source, &destination).map_err(|error| {
                format!(
                    "certified send completed prune recovery move `{}` failed: {error}",
                    entry.job_id
                )
            })?;
            sync_certified_send_directory(
                &certified_send_completed_dir(data_dir),
                "completed directory after prune recovery move",
            )?;
            sync_certified_send_directory(
                &certified_send_completed_retention_dir(data_dir),
                "retention directory after prune recovery move",
            )?;
        }
        index.entries.remove(position);
        return Ok(());
    }
    if source_exists {
        return Err(format!(
            "certified send prune intent for `{}` is absent from the index but remains completed",
            entry.job_id
        ));
    }
    Ok(())
}

fn reconcile_intent(
    data_dir: &Path,
    index: &mut CompletedIndexV1,
    work: &mut DurableCertifiedSendWorkReport,
) -> Result<bool, String> {
    let Some(intent) = read_intent(data_dir)? else {
        return Ok(false);
    };
    let operations: Vec<CompletedIndexIntentOp> = match intent {
        CompletedIndexIntent::Single(single) => vec![CompletedIndexIntentOp {
            operation: single.operation,
            entry: single.entry,
        }],
        CompletedIndexIntent::Batch(batch) => batch.operations,
    };
    for op in &operations {
        match op.operation.as_str() {
            "append" => reconcile_append_op(data_dir, index, &op.entry, work)?,
            "prune" => reconcile_prune_op(data_dir, index, &op.entry, work)?,
            _ => {
                return Err("certified send completed intent operation is invalid".to_string());
            }
        }
    }
    write_index(data_dir, &mut index.entries)?;
    index.entry_count = index.entries.len() as u64;
    index.entries_checksum = entries_checksum(&index.entries)?;
    for op in &operations {
        if op.operation == "prune" {
            finish_prune_retention(data_dir, &op.entry)?;
        }
    }
    clear_intent(data_dir)?;
    Ok(true)
}

fn discover_active_completed_jobs(
    data_dir: &Path,
) -> Result<Vec<(PathBuf, DurableCertifiedSendJob, Vec<u8>)>, String> {
    let outbox_dir = certified_send_outbox_dir(data_dir);
    let mut completed = Vec::new();
    let mut active_count = 0usize;
    for entry in std::fs::read_dir(&outbox_dir)
        .map_err(|error| format!("certified send outbox compaction read failed: {error}"))?
    {
        let entry = entry
            .map_err(|error| format!("certified send outbox compaction entry failed: {error}"))?;
        let name = entry.file_name();
        let Some(job_id) = name.to_str() else {
            return Err(format!(
                "certified send outbox entry `{}` is not valid UTF-8",
                entry.path().display()
            ));
        };
        if !certified_send_job_id_is_canonical(job_id) {
            continue;
        }
        active_count = active_count.saturating_add(1);
        if active_count > CERTIFIED_SEND_OUTBOX_MAX_JOBS {
            return Err(format!(
                "certified send outbox contains more than {CERTIFIED_SEND_OUTBOX_MAX_JOBS} active jobs"
            ));
        }
        let job_file = entry.path().join("job.json");
        let (job, bytes) = read_durable_certified_send_job_with_bytes(&job_file)?;
        if job.job_id != job_id {
            return Err(format!(
                "certified send active directory `{job_id}` conflicts with job id `{}`",
                job.job_id
            ));
        }
        if job.completed && !certified_send_quarantine_job_dir(data_dir, job_id).exists() {
            completed.push((entry.path(), job, bytes));
        }
    }
    completed.sort_by(|left, right| {
        left.1
            .block_height
            .cmp(&right.1.block_height)
            .then_with(|| left.1.job_id.cmp(&right.1.job_id))
    });
    Ok(completed)
}

fn validate_active_completed_job(
    data_dir: &Path,
    directory: &Path,
    job: &DurableCertifiedSendJob,
    job_bytes: &[u8],
    work: &mut DurableCertifiedSendWorkReport,
) -> Result<CompletedIndexEntryV1, String> {
    let job_file = directory.join("job.json");
    let validation: Result<CompletedIndexEntryV1, String> = (|| {
        validate_completed_durable_certified_send_job_metadata(job)?;
        let validation_start = Instant::now();
        let (batch_bytes, certificate_bytes) =
            read_validated_durable_certified_send_payloads(&job_file, job)?;
        work.validation_ms += monotonic_elapsed_ms(validation_start);
        work.record_tombstone_validation(
            job_bytes.len(),
            batch_bytes.len(),
            certificate_bytes.len(),
        )?;
        Ok(entry_from_validated_job(
            job,
            job_bytes,
            &batch_bytes,
            &certificate_bytes,
        ))
    })();
    match validation {
        Ok(entry) => Ok(entry),
        Err(detail) => {
            let error = format!(
                "certified send completed job `{}` is invalid before compaction: {detail}",
                job.job_id
            );
            quarantine_durable_certified_send_job(
                data_dir,
                &job.job_id,
                &job_file,
                &error,
            )?;
            Err(error)
        }
    }
}

struct PlannedAppend {
    source: PathBuf,
    entry: CompletedIndexEntryV1,
}

/// Applies one resume's appends and prunes with one durable batch intent and
/// one index rewrite per chunk, instead of one intent write, one full index
/// rewrite, and multiple directory syncs per touched entry. Bounded work per
/// resume becomes O(index size + touched jobs), not O(index size × touched).
fn apply_completed_index_batch(
    data_dir: &Path,
    index: &mut CompletedIndexV1,
    appends: Vec<PlannedAppend>,
    max_tombstones: usize,
    work: &mut DurableCertifiedSendWorkReport,
) -> Result<(usize, usize), String> {
    let total = index.entries.len().saturating_add(appends.len());
    let prune_count = total
        .saturating_sub(max_tombstones)
        .min(index.entries.len());
    let prune_start = Instant::now();
    let mut prunes = Vec::with_capacity(prune_count);
    for entry in index.entries.iter().take(prune_count) {
        let source = certified_send_completed_dir(data_dir).join(&entry.job_id);
        let validated = validate_job_directory_and_quarantine(
            data_dir,
            &source,
            &entry.job_id,
            work,
            "index prune",
        )?;
        if &validated.entry != entry {
            let error = format!(
                "certified send completed job `{}` conflicts with its index during prune",
                entry.job_id
            );
            quarantine_durable_certified_send_job(
                data_dir,
                &entry.job_id,
                &source.join("job.json"),
                &error,
            )?;
            return Err(error);
        }
        let destination = retention_destination(data_dir, &entry.job_id);
        if destination.exists() {
            return Err(format!(
                "certified send completed retention destination `{}` already exists",
                destination.display()
            ));
        }
        prunes.push(entry.clone());
    }
    work.prune_ms += monotonic_elapsed_ms(prune_start);

    let mut append_sources = std::collections::BTreeMap::new();
    let mut operations = Vec::with_capacity(appends.len() + prunes.len());
    for append in appends {
        append_sources.insert(append.entry.job_id.clone(), append.source);
        operations.push(CompletedIndexIntentOp {
            operation: "append".to_string(),
            entry: append.entry,
        });
    }
    for entry in prunes {
        operations.push(CompletedIndexIntentOp {
            operation: "prune".to_string(),
            entry,
        });
    }
    if operations.is_empty() {
        return Ok((0, 0));
    }

    let mut compacted = 0usize;
    let mut pruned = 0usize;
    for chunk in operations.chunks(COMPLETED_INDEX_INTENT_MAX_OPERATIONS) {
        write_batch_intent(data_dir, chunk)?;
        let mut chunk_has_append = false;
        let mut chunk_has_prune = false;
        for op in chunk {
            match op.operation.as_str() {
                "append" => {
                    chunk_has_append = true;
                    let source = append_sources.get(&op.entry.job_id).ok_or_else(|| {
                        format!(
                            "certified send batch append `{}` has no planned source",
                            op.entry.job_id
                        )
                    })?;
                    let completed_dir = certified_send_completed_dir(data_dir);
                    std::fs::create_dir_all(&completed_dir).map_err(|error| {
                        format!("certified send completed directory create failed: {error}")
                    })?;
                    let destination = completed_dir.join(&op.entry.job_id);
                    std::fs::rename(source, &destination).map_err(|error| {
                        format!(
                            "certified send completed job move `{}` failed: {error}",
                            op.entry.job_id
                        )
                    })?;
                }
                "prune" => {
                    chunk_has_prune = true;
                    ensure_retention_directory(data_dir)?;
                    let source = certified_send_completed_dir(data_dir).join(&op.entry.job_id);
                    let destination = retention_destination(data_dir, &op.entry.job_id);
                    std::fs::rename(&source, &destination).map_err(|error| {
                        format!(
                            "certified send completed retention move `{}` -> `{}` failed: {error}",
                            source.display(),
                            destination.display()
                        )
                    })?;
                }
                _ => {
                    return Err(
                        "certified send completed intent operation is invalid".to_string()
                    );
                }
            }
        }
        if chunk_has_append {
            sync_certified_send_directory(
                &certified_send_outbox_dir(data_dir),
                "outbox directory after completed move",
            )?;
        }
        sync_certified_send_directory(
            &certified_send_completed_dir(data_dir),
            "completed directory after batch moves",
        )?;
        if chunk_has_prune {
            sync_certified_send_directory(
                &certified_send_completed_retention_dir(data_dir),
                "completed retention directory after move",
            )?;
        }
        for op in chunk {
            match op.operation.as_str() {
                "append" => {
                    index.entries.push(op.entry.clone());
                    compacted = compacted.saturating_add(1);
                }
                "prune" => {
                    let position =
                        index_entry_position(index, &op.entry.job_id).ok_or_else(|| {
                            format!(
                                "certified send batch prune `{}` is absent from the index",
                                op.entry.job_id
                            )
                        })?;
                    index.entries.remove(position);
                    pruned = pruned.saturating_add(1);
                }
                _ => {}
            }
        }
        write_index(data_dir, &mut index.entries)?;
        index.entry_count = index.entries.len() as u64;
        index.entries_checksum = entries_checksum(&index.entries)?;
        // Dispose all pruned retention payloads, then make the disposals
        // durable with one retention-directory sync. A crash mid-disposal
        // leaves retention directories that the next resume's retention
        // cleanup removes, exactly as with per-entry disposal.
        let mut disposed = false;
        for op in chunk {
            if op.operation == "prune" {
                let destination = retention_destination(data_dir, &op.entry.job_id);
                if destination.exists() {
                    remove_certified_send_disposable_job_dir_unsynced(
                        &destination,
                        &op.entry.job_id,
                    )?;
                    disposed = true;
                }
            }
        }
        if disposed {
            sync_certified_send_directory(
                &certified_send_completed_retention_dir(data_dir),
                "completed retention directory after disposal",
            )?;
        }
        clear_intent_unsynced(data_dir)?;
    }
    Ok((compacted, pruned))
}

fn compact_completed_with_index_locked(
    data_dir: &Path,
    max_tombstones: usize,
) -> Result<CompletedIndexCompactionReport, String> {
    let compaction_start = Instant::now();
    let mut report = CompletedIndexCompactionReport::default();
    cleanup_orphan_certified_send_staging_dirs(data_dir)?;
    let outbox_dir = certified_send_outbox_dir(data_dir);
    if !outbox_dir.exists() {
        let index = ensure_index(data_dir, &mut report.work)?;
        if !index.entries.is_empty() {
            return Err(
                "certified send completed index exists without its outbox; explicit verify repair is required"
                    .to_string(),
            );
        }
        report.work.compaction_ms = monotonic_elapsed_ms(compaction_start);
        return Ok(report);
    }

    let mut index = ensure_index(data_dir, &mut report.work)?;
    let intent_reconciled = reconcile_intent(data_dir, &mut index, &mut report.work)?;
    cleanup_certified_send_completed_retention_dir(data_dir)?;
    let active_completed = discover_active_completed_jobs(data_dir)?;
    if active_completed.is_empty() && index.entries.len() <= max_tombstones {
        report.work.compaction_ms = monotonic_elapsed_ms(compaction_start);
        return Ok(report);
    }
    if !intent_reconciled {
        require_completed_directory_stamp(data_dir, &index)?;
    }
    // Bound compaction to one round's worth of deliveries per maintenance
    // pass. A first pass after restore may find leftover completed jobs plus
    // the current round's deliveries; sweeping the oldest five keeps the
    // per-resume work gate exact (validated == compacted + pruned, ≤5 each)
    // and defers the remainder exactly one round.
    let mut appends = Vec::with_capacity(active_completed.len());
    for (directory, job, job_bytes) in active_completed
        .into_iter()
        .take(CERTIFIED_SEND_MAX_COMPACTIONS_PER_RESUME)
    {
        let entry = validate_active_completed_job(
            data_dir,
            &directory,
            &job,
            &job_bytes,
            &mut report.work,
        )?;
        if index_entry_position(&index, &entry.job_id).is_some()
            || certified_send_completed_dir(data_dir)
                .join(&entry.job_id)
                .exists()
        {
            let error = format!(
                "certified send completed record `{}` already exists",
                entry.job_id
            );
            quarantine_durable_certified_send_job(
                data_dir,
                &entry.job_id,
                &directory.join("job.json"),
                &error,
            )?;
            return Err(error);
        }
        appends.push(PlannedAppend {
            source: directory,
            entry,
        });
    }
    let (compacted, pruned) = apply_completed_index_batch(
        data_dir,
        &mut index,
        appends,
        max_tombstones,
        &mut report.work,
    )?;
    report.compacted = compacted;
    report.pruned = pruned;
    report.work.jobs_compacted = report.compacted as u64;
    report.work.jobs_pruned = report.pruned as u64;
    report.work.compaction_ms = monotonic_elapsed_ms(compaction_start);
    Ok(report)
}

pub(super) fn compact_completed_with_index(
    data_dir: &Path,
) -> Result<CompletedIndexCompactionReport, String> {
    let _guard = acquire_mutation_guard(data_dir)?;
    compact_completed_with_index_locked(
        data_dir,
        CERTIFIED_SEND_COMPLETED_TOMBSTONE_MAX_JOBS,
    )
}

#[cfg(test)]
pub(super) fn prune_completed_with_index(
    data_dir: &Path,
    max_tombstones: usize,
) -> Result<CompletedIndexCompactionReport, String> {
    let _guard = acquire_mutation_guard(data_dir)?;
    let compaction_start = Instant::now();
    let mut report = CompletedIndexCompactionReport::default();
    cleanup_orphan_certified_send_staging_dirs(data_dir)?;
    let mut index = ensure_index(data_dir, &mut report.work)?;
    let intent_reconciled = reconcile_intent(data_dir, &mut index, &mut report.work)?;
    cleanup_certified_send_completed_retention_dir(data_dir)?;
    if index.entries.len() > max_tombstones && !intent_reconciled {
        require_completed_directory_stamp(data_dir, &index)?;
    }
    let (_, pruned) = apply_completed_index_batch(
        data_dir,
        &mut index,
        Vec::new(),
        max_tombstones,
        &mut report.work,
    )?;
    report.pruned = pruned;
    report.work.jobs_pruned = report.pruned as u64;
    report.work.compaction_ms = monotonic_elapsed_ms(compaction_start);
    Ok(report)
}

pub(super) fn verify_and_rebuild_completed_index(
    data_dir: &Path,
) -> Result<CompletedIndexVerifyReport, String> {
    let _guard = acquire_mutation_guard(data_dir)?;
    cleanup_orphan_certified_send_staging_dirs(data_dir)?;
    let mut work = DurableCertifiedSendWorkReport::default();
    let completed = fully_validate_completed_jobs(data_dir, &mut work, "explicit verify repair")?;
    let mut entries = completed
        .into_iter()
        .map(|validated| validated.entry)
        .collect::<Vec<_>>();
    write_index(data_dir, &mut entries)?;
    cleanup_certified_send_completed_retention_dir(data_dir)?;
    clear_intent(data_dir)?;
    Ok(CompletedIndexVerifyReport {
        schema: COMPLETED_INDEX_VERIFY_REPORT_SCHEMA.to_string(),
        index_file: completed_index_path(data_dir).display().to_string(),
        intent_file: completed_index_intent_path(data_dir).display().to_string(),
        entry_count: entries.len(),
        rebuilt: true,
        work,
    })
}

#[cfg(test)]
pub(super) fn validated_completed_jobs_for_legacy_callers(
    data_dir: &Path,
) -> Result<Vec<(PathBuf, DurableCertifiedSendJob)>, String> {
    let mut work = DurableCertifiedSendWorkReport::default();
    fully_validate_completed_jobs(data_dir, &mut work, "full validation")?.into_iter().map(
        |validated| Ok((validated.directory, validated.job)),
    ).collect()
}

#[cfg(test)]
pub(super) fn write_append_intent_for_test(
    data_dir: &Path,
    source_directory: &Path,
) -> Result<CompletedIndexEntryV1, String> {
    let job_id = source_directory
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "test source directory has no canonical job id".to_string())?;
    let mut work = DurableCertifiedSendWorkReport::default();
    let validated = validate_job_directory(source_directory, job_id, &mut work)?;
    write_intent(data_dir, "append", &validated.entry)?;
    Ok(validated.entry)
}

#[cfg(test)]
pub(super) fn write_batch_intent_for_test(
    data_dir: &Path,
    operations: &[(&str, CompletedIndexEntryV1)],
) -> Result<(), String> {
    let ops: Vec<CompletedIndexIntentOp> = operations
        .iter()
        .map(|(operation, entry)| CompletedIndexIntentOp {
            operation: (*operation).to_string(),
            entry: entry.clone(),
        })
        .collect();
    write_batch_intent(data_dir, &ops)
}

#[cfg(test)]
mod completed_index_tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "postfiat-certified-send-index-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn test_topology() -> NetworkTopology {
        NetworkTopology {
            topology_id: "certified-send-index-test".to_string(),
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 1,
            peers: vec![
                postfiat_network::PeerInfo {
                    node_id: "validator-0".to_string(),
                    host: "127.0.0.1".to_string(),
                    p2p_port: 26_650,
                    rpc_port: 27_650,
                    p2p_address: "127.0.0.1:26650".to_string(),
                },
                postfiat_network::PeerInfo {
                    node_id: "validator-1".to_string(),
                    host: "127.0.0.1".to_string(),
                    p2p_port: 26_651,
                    rpc_port: 27_651,
                    p2p_address: "127.0.0.1:26651".to_string(),
                },
            ],
        }
    }

    fn tombstone_job(
        data_dir: &Path,
        topology: &NetworkTopology,
        height: u64,
        completed_root: bool,
    ) -> (String, PathBuf) {
        let certificate_id = format!("certificate-{height}");
        let block_hash = format!("{:096x}", height);
        let state_root = format!("{:096x}", height.saturating_add(10_000));
        let job_id = durable_certified_send_job_id(
            &topology.topology_id,
            "validator-0",
            "validator-1",
            height,
            &certificate_id,
            &block_hash,
        );
        let root = if completed_root {
            certified_send_completed_dir(data_dir)
        } else {
            certified_send_outbox_dir(data_dir)
        };
        let directory = root.join(&job_id);
        std::fs::create_dir_all(&directory).expect("create seeded tombstone directory");
        let batch_file = directory.join("batch.json");
        let certificate_file = directory.join("certificate.json");
        let batch_bytes = format!("{{\"batch\":{height}}}\n").into_bytes();
        let certificate_bytes = format!("{{\"certificate\":{height}}}\n").into_bytes();
        std::fs::write(&batch_file, &batch_bytes).expect("write seeded batch");
        std::fs::write(&certificate_file, &certificate_bytes)
            .expect("write seeded certificate");
        let job = DurableCertifiedSendJob {
            schema: CERTIFIED_SEND_JOB_SCHEMA.to_string(),
            job_id: job_id.clone(),
            topology_id: topology.topology_id.clone(),
            chain_id: topology.chain_id.clone(),
            genesis_hash: topology.genesis_hash.clone(),
            protocol_version: topology.protocol_version,
            source: "validator-0".to_string(),
            target: "validator-1".to_string(),
            batch_kind: "transparent".to_string(),
            block_height: height,
            certificate_id,
            block_hash: block_hash.clone(),
            expected_state_root: state_root.clone(),
            batch_file: batch_file.display().to_string(),
            batch_hash: postfiat_crypto_provider::hash_hex(
                "postfiat.certified_send_job.batch.v1",
                &batch_bytes,
            ),
            certificate_file: certificate_file.display().to_string(),
            certificate_hash: postfiat_crypto_provider::hash_hex(
                "postfiat.certified_send_job.certificate.v1",
                &certificate_bytes,
            ),
            timeout_ms: 1_000,
            send_retries: 1,
            retry_backoff_ms: 10,
            attempt_count: 1,
            completed: true,
            last_error: None,
            ack: Some(DurableCertifiedSendAck {
                already_applied: false,
                block_height: height,
                block_tip_hash: block_hash,
                state_root,
            }),
        };
        write_durable_certified_send_job(&directory.join("job.json"), &job)
            .expect("write seeded tombstone job");
        (job_id, directory)
    }

    fn seed_completed(
        data_dir: &Path,
        topology: &NetworkTopology,
        count: usize,
    ) -> Vec<(String, PathBuf)> {
        (1..=count)
            .map(|height| tombstone_job(data_dir, topology, height as u64, true))
            .collect()
    }

    fn enqueue_active_job(
        root: &Path,
        topology: &NetworkTopology,
        height: u64,
    ) -> PathBuf {
        let input = root.join("input");
        std::fs::create_dir_all(&input).expect("create enqueue input");
        let batch = input.join("batch.json");
        let certificate = input.join("certificate.json");
        std::fs::write(&batch, format!("{{\"batch\":{height}}}\n"))
            .expect("write enqueue batch");
        std::fs::write(
            &certificate,
            format!("{{\"certificate\":{height}}}\n"),
        )
        .expect("write enqueue certificate");
        enqueue_durable_certified_send_job(
            root,
            topology,
            "validator-0",
            "validator-1",
            "transparent",
            height,
            &format!("certificate-{height}"),
            &format!("{:096x}", height),
            &format!("{:096x}", height.saturating_add(10_000)),
            &batch,
            &certificate,
            1_000,
            1,
            10,
        )
        .expect("enqueue active test job")
    }

    #[test]
    fn no_outbox_first_resume_migrates_empty_index() {
        let root = test_root("no-outbox-first-resume");
        std::fs::create_dir_all(&root).expect("create fresh data directory");

        let report = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("eagerly create empty completed index");

        assert!(completed_index_path_for_test(&root).is_file());
        assert!(report.work.index_migration_performed);
        assert_eq!(report.work.tombstones_validated, 0);
        assert_eq!(report.work.files_read, 0);
        assert_eq!(report.work.bytes_hashed, 0);
        assert_eq!(report.work.index_files_read, 1);
        assert!(report.work.index_bytes_read > 0);
        assert_eq!(report.work.completed_entries_enumerated, 0);
        assert_eq!(report.work.jobs_compacted, 0);
        assert_eq!(report.work.jobs_pruned, 0);
        assert!(report.all_completed);
        std::fs::remove_dir_all(root).expect("cleanup first-resume test");
    }

    #[test]
    fn no_outbox_second_resume_reads_index_without_migration() {
        let root = test_root("no-outbox-second-resume");
        std::fs::create_dir_all(&root).expect("create fresh data directory");
        let first = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("eagerly create empty completed index");
        assert!(first.work.index_migration_performed);

        let second = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("reuse eager empty completed index");

        assert!(!second.work.index_migration_performed);
        assert_eq!(second.work.index_files_read, 1);
        assert_eq!(second.work.tombstones_validated, 0);
        assert_eq!(second.work.files_read, 0);
        assert_eq!(second.work.bytes_hashed, 0);
        assert!(second.all_completed);
        std::fs::remove_dir_all(root).expect("cleanup second-resume test");
    }

    #[test]
    fn campaign_replay_no_outbox_then_deliveries_then_resume() {
        let root = test_root("campaign-no-outbox-deliveries-resume");
        let topology = test_topology();
        std::fs::create_dir_all(&root).expect("create fresh data directory");
        let first = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("eagerly create empty completed index");
        assert!(first.work.index_migration_performed);

        for height in 1..=5 {
            tombstone_job(&root, &topology, height, false);
        }
        let second = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("compact deliveries after eager migration");

        assert!(!second.work.index_migration_performed);
        assert_eq!(second.work.jobs_compacted, 5);
        assert_eq!(second.work.jobs_pruned, 0);
        assert_eq!(second.work.tombstones_validated, 5);
        assert_eq!(second.work.files_read, 15);
        assert_eq!(second.work.completed_entries_enumerated, 0);
        assert_eq!(
            second.work.tombstones_validated,
            second.work.jobs_compacted + second.work.jobs_pruned
        );
        assert!(second.all_completed);
        std::fs::remove_dir_all(root).expect("cleanup campaign replay test");
    }

    #[test]
    fn no_outbox_intent_without_index_fails_closed() {
        let root = test_root("no-outbox-intent-without-index");
        let topology = test_topology();
        let (_, source) = tombstone_job(&root, &topology, 1, false);
        write_append_intent_for_test(&root, &source).expect("write append intent");
        std::fs::remove_dir_all(certified_send_outbox_dir(&root))
            .expect("remove outbox after intent");

        let error = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect_err("intent without index must fail closed");

        assert!(error.contains("mutation intent exists"), "{error}");
        assert!(error.contains("explicit verify repair"), "{error}");
        assert!(!completed_index_path_for_test(&root).exists());
        std::fs::remove_dir_all(root).expect("cleanup intent-without-index test");
    }

    #[test]
    fn non_empty_index_with_deleted_outbox_still_fails_closed() {
        let root = test_root("non-empty-index-deleted-outbox");
        let topology = test_topology();
        seed_completed(&root, &topology, 1);
        verify_and_rebuild_completed_index(&root).expect("build non-empty index");
        std::fs::remove_dir_all(certified_send_outbox_dir(&root))
            .expect("delete outbox behind non-empty index");

        let error = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect_err("non-empty index without outbox must fail closed");

        assert!(error.contains("index exists without its outbox"), "{error}");
        assert!(error.contains("explicit verify repair"), "{error}");
        std::fs::remove_dir_all(root).expect("cleanup deleted-outbox test");
    }

    #[test]
    fn resume_with_zero_tombstones_is_bounded() {
        let root = test_root("zero");
        std::fs::create_dir_all(certified_send_outbox_dir(&root)).expect("create empty outbox");
        verify_and_rebuild_completed_index(&root).expect("create empty index");
        let report = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("resume empty indexed outbox");
        assert_eq!(report.work.tombstones_validated, 0);
        assert_eq!(report.work.files_read, 0);
        assert_eq!(report.work.bytes_hashed, 0);
        assert_eq!(report.work.index_files_read, 1);
        assert!(!report.work.index_migration_performed);
        assert!(report.all_completed);
        std::fs::remove_dir_all(root).expect("cleanup zero test");
    }

    #[test]
    fn resume_with_240_and_1024_tombstones_examines_no_retained_payloads() {
        let topology = test_topology();
        let mut observed = Vec::new();
        for count in [240usize, 1_024] {
            let root = test_root(&format!("flat-{count}"));
            seed_completed(&root, &topology, count);
            verify_and_rebuild_completed_index(&root).expect("build seeded index");
            let report = resume_durable_certified_send_outbox(
                &root,
                &root.join("unused-topology.json"),
                CERTIFIED_SEND_OUTBOX_MAX_JOBS,
            )
            .expect("resume indexed retained set");
            observed.push((
                report.work.tombstones_validated,
                report.work.files_read,
                report.work.bytes_hashed,
                report.work.index_files_read,
                report.work.completed_entries_enumerated,
            ));
            tombstone_job(&root, &topology, count as u64 + 1, false);
            let mutation = compact_completed_with_index(&root)
                .expect("compact one newly completed active job");
            assert_eq!(mutation.compacted, 1);
            assert_eq!(mutation.work.completed_entries_enumerated, 0);
            assert_eq!(
                mutation.work.tombstones_validated,
                if count == 1_024 { 2 } else { 1 }
            );
            std::fs::remove_dir_all(root).expect("cleanup flat retained-set test");
        }
        assert_eq!(observed[0], observed[1]);
        assert_eq!(observed[0], (0, 0, 0, 1, 0));
    }

    #[test]
    fn index_migration_runs_once_and_validates_everything() {
        let root = test_root("migration-once");
        let topology = test_topology();
        seed_completed(&root, &topology, 3);
        let first = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("migrate completed set");
        assert!(first.work.index_migration_performed);
        assert_eq!(first.work.tombstones_validated, 3);
        assert_eq!(first.work.files_read, 9);
        assert!(first.work.bytes_hashed > 0);
        assert!(completed_index_path_for_test(&root).is_file());

        let second = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("reuse migrated index");
        assert!(!second.work.index_migration_performed);
        assert_eq!(second.work.tombstones_validated, 0);
        assert_eq!(second.work.files_read, 0);
        std::fs::remove_dir_all(root).expect("cleanup migration-once test");
    }

    #[test]
    fn tampered_tombstone_fails_migration_and_quarantines() {
        let root = test_root("tampered-migration");
        let topology = test_topology();
        let seeded = seed_completed(&root, &topology, 1);
        std::fs::write(seeded[0].1.join("batch.json"), b"tampered\n")
            .expect("tamper migration batch");
        let error = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect_err("tampered migration must fail");
        assert!(error.contains("index migration"), "{error}");
        assert!(certified_send_quarantine_record_file(&root, &seeded[0].0).is_file());
        assert!(!completed_index_path_for_test(&root).exists());
        std::fs::remove_dir_all(root).expect("cleanup tampered migration test");
    }

    #[test]
    fn tampered_tombstone_after_index_is_caught_by_repair_and_by_prune() {
        let root = test_root("tampered-indexed");
        let topology = test_topology();
        let seeded = seed_completed(&root, &topology, 2);
        verify_and_rebuild_completed_index(&root).expect("build pre-tamper index");
        std::fs::write(seeded[0].1.join("batch.json"), b"tampered\n")
            .expect("tamper indexed batch");

        let ordinary = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("ordinary resume deliberately does not revalidate retained payloads");
        assert_eq!(ordinary.work.tombstones_validated, 0);
        assert!(verify_and_rebuild_completed_index(&root).is_err());
        let prune_error = prune_completed_with_index(&root, 1)
            .expect_err("prune touching tampered oldest entry must fail");
        assert!(prune_error.contains("index prune"), "{prune_error}");
        assert!(certified_send_quarantine_record_file(&root, &seeded[0].0).is_file());
        std::fs::remove_dir_all(root).expect("cleanup tampered indexed test");
    }

    #[test]
    fn index_directory_divergence_fails_closed() {
        let topology = test_topology();

        let deleted_root = test_root("divergence-delete");
        let deleted = seed_completed(&deleted_root, &topology, 2);
        verify_and_rebuild_completed_index(&deleted_root).expect("build delete index");
        std::fs::remove_dir_all(&deleted[0].1).expect("delete indexed directory behind index");
        let delete_error = prune_completed_with_index(&deleted_root, 1)
            .expect_err("deleted directory must fail closed on the next mutation");
        assert!(delete_error.contains("divergence"), "{delete_error}");
        std::fs::remove_dir_all(deleted_root).expect("cleanup delete divergence test");

        let added_root = test_root("divergence-add");
        seed_completed(&added_root, &topology, 1);
        verify_and_rebuild_completed_index(&added_root).expect("build add index");
        std::fs::create_dir_all(
            certified_send_completed_dir(&added_root).join("ab".repeat(48)),
        )
        .expect("add canonical directory behind index");
        let add_error = prune_completed_with_index(&added_root, 0)
            .expect_err("added directory must fail closed on the next mutation");
        assert!(add_error.contains("divergence"), "{add_error}");
        std::fs::remove_dir_all(added_root).expect("cleanup add divergence test");
    }

    #[test]
    fn at_cap_resume_batches_appends_and_prunes_with_one_index_write() {
        let root = test_root("at-cap-batch");
        let topology = test_topology();
        let seeded = seed_completed(
            &root,
            &topology,
            CERTIFIED_SEND_COMPLETED_TOMBSTONE_MAX_JOBS,
        );
        verify_and_rebuild_completed_index(&root).expect("build at-cap index");
        for height in 20_000..20_005 {
            tombstone_job(&root, &topology, height, false);
        }

        let report = resume_durable_certified_send_outbox(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("at-cap steady-state resume");

        assert!(!report.work.index_migration_performed);
        assert_eq!(report.work.jobs_compacted, 5);
        assert_eq!(report.work.jobs_pruned, 5);
        assert_eq!(
            report.work.tombstones_validated,
            report.work.jobs_compacted + report.work.jobs_pruned
        );
        assert_eq!(report.work.files_read, 30);
        assert_eq!(report.work.index_files_read, 1);
        assert!(!completed_index_intent_path_for_test(&root).exists());
        for (_, path) in seeded.iter().take(5) {
            assert!(!path.exists(), "oldest tombstones must be pruned");
        }
        let retention = certified_send_completed_retention_dir(&root);
        assert_eq!(
            std::fs::read_dir(&retention)
                .expect("read retention directory")
                .count(),
            0,
            "retention payloads must be disposed after the batch"
        );
        let verified =
            verify_and_rebuild_completed_index(&root).expect("verify at-cap index");
        assert_eq!(
            verified.entry_count,
            CERTIFIED_SEND_COMPLETED_TOMBSTONE_MAX_JOBS
        );
        std::fs::remove_dir_all(root).expect("cleanup at-cap batch test");
    }

    #[test]
    fn maintenance_sweeps_at_most_five_jobs_per_pass() {
        let root = test_root("sweep-cap");
        let topology = test_topology();
        std::fs::create_dir_all(&root).expect("create fresh data directory");
        for height in 1..=8 {
            tombstone_job(&root, &topology, height, false);
        }

        let first = compact_completed_with_index(&root).expect("first bounded sweep");
        assert_eq!(first.compacted, 5, "first pass sweeps the oldest five");
        assert_eq!(first.work.tombstones_validated, 5);
        let second = compact_completed_with_index(&root).expect("second bounded sweep");
        assert_eq!(second.compacted, 3, "remainder compacts on the next pass");
        assert!(!second.work.index_migration_performed);
        let verified =
            verify_and_rebuild_completed_index(&root).expect("verify swept index");
        assert_eq!(verified.entry_count, 8);
        std::fs::remove_dir_all(root).expect("cleanup sweep-cap test");
    }

    #[test]
    fn pending_only_resume_defers_all_maintenance() {
        let root = test_root("pending-only");
        let topology = test_topology();
        std::fs::create_dir_all(&root).expect("create fresh data directory");
        for height in 1..=3 {
            tombstone_job(&root, &topology, height, false);
        }

        let scan = resume_durable_certified_send_outbox_pending_only(
            &root,
            &root.join("unused-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("pending-only scan");
        assert!(scan.all_completed);
        assert_eq!(scan.pending, 0);
        assert!(!scan.work.index_migration_performed, "maintenance must be deferred");
        assert_eq!(scan.work.jobs_compacted, 0);
        assert!(!completed_index_path_for_test(&root).exists());

        let maintenance = compact_completed_with_index(&root).expect("deferred maintenance");
        assert!(maintenance.work.index_migration_performed);
        assert_eq!(maintenance.compacted, 3);
        assert!(completed_index_path_for_test(&root).exists());
        std::fs::remove_dir_all(root).expect("cleanup pending-only test");
    }

    #[test]
    fn crash_mid_batch_intent_recovers_all_operations() {
        let root = test_root("batch-crash");
        let topology = test_topology();
        let seeded = seed_completed(&root, &topology, 8);
        verify_and_rebuild_completed_index(&root).expect("build base index");

        let (_, append_source) = tombstone_job(&root, &topology, 99, false);
        let append_job_id = append_source
            .file_name()
            .and_then(|value| value.to_str())
            .expect("append job id")
            .to_string();
        let mut work = DurableCertifiedSendWorkReport::default();
        let append_entry = validate_job_directory(&append_source, &append_job_id, &mut work)
            .expect("validate planned append")
            .entry;
        let prune_job_id = &seeded[0].0;
        let prune_entry = validate_job_directory(&seeded[0].1, prune_job_id, &mut work)
            .expect("validate planned prune")
            .entry;
        write_batch_intent_for_test(
            &root,
            &[("append", append_entry.clone()), ("prune", prune_entry)],
        )
        .expect("write batch intent");
        let destination = certified_send_completed_dir(&root).join(&append_entry.job_id);
        std::fs::rename(&append_source, &destination)
            .expect("simulate crash after first batch move");

        let report = compact_completed_with_index(&root).expect("recover batch intent");
        assert_eq!(report.compacted, 0);
        assert!(!completed_index_intent_path_for_test(&root).exists());
        assert!(destination.exists(), "append must be reconciled into completed");
        assert!(!seeded[0].1.exists(), "prune must be reconciled out of completed");
        let verified =
            verify_and_rebuild_completed_index(&root).expect("verify recovered index");
        assert_eq!(verified.entry_count, 8);
        std::fs::remove_dir_all(root).expect("cleanup batch crash test");
    }

    #[test]
    fn crash_between_move_and_index_rewrite_recovers() {
        let root = test_root("append-crash");
        let topology = test_topology();
        seed_completed(&root, &topology, 8);
        verify_and_rebuild_completed_index(&root).expect("build base index");
        let (_, source) = tombstone_job(&root, &topology, 99, false);
        let entry = write_append_intent_for_test(&root, &source).expect("write append intent");
        let destination = certified_send_completed_dir(&root).join(&entry.job_id);
        std::fs::rename(&source, &destination).expect("simulate crash after completed move");

        let report = compact_completed_with_index(&root).expect("recover append intent");
        assert_eq!(report.compacted, 0);
        assert_eq!(report.work.tombstones_validated, 1);
        assert!(!completed_index_intent_path_for_test(&root).exists());
        let repaired = verify_and_rebuild_completed_index(&root).expect("verify recovered index");
        assert_eq!(repaired.entry_count, 9);
        std::fs::remove_dir_all(root).expect("cleanup append crash test");
    }

    #[test]
    fn pending_jobs_still_block_proposals() {
        let root = test_root("pending-blocks");
        let topology = test_topology();
        enqueue_active_job(&root, &topology, 1);
        let report = resume_durable_certified_send_outbox(
            &root,
            &root.join("missing-topology.json"),
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("pending resume report");
        assert_eq!(report.pending, 1);
        assert!(!report.all_completed);
        std::fs::remove_dir_all(root).expect("cleanup pending-blocks test");
    }

    #[test]
    fn prune_preserves_retention_and_fsync_flow() {
        let root = test_root("prune-flow");
        let topology = test_topology();
        let seeded = seed_completed(&root, &topology, 3);
        verify_and_rebuild_completed_index(&root).expect("build prune index");
        let report = prune_completed_with_index(&root, 2).expect("prune oldest indexed entry");
        assert_eq!(report.pruned, 1);
        assert_eq!(report.work.tombstones_validated, 1);
        assert!(!seeded[0].1.exists());
        assert!(seeded[1].1.exists());
        assert!(seeded[2].1.exists());
        assert!(!completed_index_intent_path_for_test(&root).exists());
        let retention = certified_send_completed_retention_dir(&root);
        assert_eq!(
            std::fs::read_dir(&retention)
                .expect("read empty retention directory")
                .count(),
            0
        );
        let verified = verify_and_rebuild_completed_index(&root).expect("verify pruned index");
        assert_eq!(verified.entry_count, 2);
        std::fs::remove_dir_all(root).expect("cleanup prune flow test");
    }

    #[test]
    fn enqueue_duplicate_job_id_behavior_unchanged() {
        let root = test_root("duplicate-behavior");
        let topology = test_topology();
        let first = enqueue_active_job(&root, &topology, 7);
        assert_eq!(enqueue_active_job(&root, &topology, 7), first);
        let mut job = read_durable_certified_send_job(&first).expect("read duplicate test job");
        let ack = DurableCertifiedSendAck {
            already_applied: false,
            block_height: job.block_height,
            block_tip_hash: job.block_hash.clone(),
            state_root: job.expected_state_root.clone(),
        };
        complete_durable_certified_send_job(&root, &first, &mut job, ack)
        .expect("complete duplicate test job");
        compact_completed_with_index(&root).expect("compact duplicate test job");
        let completed = certified_send_completed_dir(&root)
            .join(&job.job_id)
            .join("job.json");
        assert_eq!(enqueue_active_job(&root, &topology, 7), completed);

        prune_completed_with_index(&root, 0).expect("prune duplicate tombstone");
        let recreated = enqueue_active_job(&root, &topology, 7);
        assert_ne!(recreated, completed);
        assert!(recreated.is_file());
        std::fs::remove_dir_all(root).expect("cleanup duplicate behavior test");
    }

    #[test]
    #[ignore = "release-mode manual spot check; the runner carries the same gate"]
    fn release_proposer_rotation_with_1024_tombstones_is_bounded() {
        let topology = test_topology();
        let mut timings = Vec::new();
        for validator in 0..6 {
            let root = test_root(&format!("rotation-{validator}"));
            if validator == 0 {
                seed_completed(&root, &topology, 1_024);
            } else {
                std::fs::create_dir_all(certified_send_outbox_dir(&root))
                    .expect("create peer outbox");
            }
            verify_and_rebuild_completed_index(&root).expect("build rotation index");
            let report = resume_durable_certified_send_outbox(
                &root,
                &root.join("unused-topology.json"),
                CERTIFIED_SEND_OUTBOX_MAX_JOBS,
            )
            .expect("resume rotation validator");
            assert_eq!(report.work.tombstones_validated, 0);
            assert_eq!(report.work.files_read, 0);
            assert_eq!(report.work.bytes_hashed, 0);
            assert_eq!(report.work.completed_entries_enumerated, 0);
            eprintln!(
                "validator={validator} retained_tombstones={} outbox_resume_ms={:.3} files_read={} bytes_hashed={} index_files_read={} index_bytes_read={} completed_entries_enumerated={}",
                if validator == 0 { 1_024 } else { 0 },
                report.outbox_resume_ms,
                report.work.files_read,
                report.work.bytes_hashed,
                report.work.index_files_read,
                report.work.index_bytes_read,
                report.work.completed_entries_enumerated,
            );
            timings.push(report.outbox_resume_ms);
            std::fs::remove_dir_all(root).expect("cleanup rotation validator");
        }
        let slowest = timings.iter().copied().fold(0.0_f64, f64::max);
        let fastest = timings.iter().copied().fold(f64::INFINITY, f64::min);
        eprintln!(
            "resume_delta_ms={:.3} fastest_ms={fastest:.3} slowest_ms={slowest:.3}",
            slowest - fastest
        );
        assert!(
            slowest - fastest <= 50.0,
            "indexed proposer resume delta was {:.3} ms: {timings:?}",
            slowest - fastest
        );
    }

    #[test]
    #[ignore = "release-mode manual spot check of the at-cap steady-state campaign round"]
    fn release_at_cap_steady_state_rounds_are_bounded() {
        // Mirrors the remediated-G4 failure shape: validator-0 at the
        // 1,024-tombstone retention cap completing five certified sends per
        // proposal round. Before batching, every such round cost ~205 ms in
        // per-entry index rewrites and syncs; the gate here is per-round.
        let topology = test_topology();
        let root = test_root("release-at-cap-steady");
        seed_completed(
            &root,
            &topology,
            CERTIFIED_SEND_COMPLETED_TOMBSTONE_MAX_JOBS,
        );
        verify_and_rebuild_completed_index(&root).expect("build at-cap index");
        let mut round_ms = Vec::new();
        for round in 0..8u64 {
            for height in (30_000 + round * 5)..(30_000 + round * 5 + 5) {
                tombstone_job(&root, &topology, height, false);
            }
            let report = resume_durable_certified_send_outbox(
                &root,
                &root.join("unused-topology.json"),
                CERTIFIED_SEND_OUTBOX_MAX_JOBS,
            )
            .expect("at-cap steady-state release resume");
            assert_eq!(report.work.jobs_compacted, 5);
            assert_eq!(report.work.jobs_pruned, 5);
            assert_eq!(
                report.work.tombstones_validated,
                report.work.jobs_compacted + report.work.jobs_pruned
            );
            eprintln!(
                "round={round} outbox_resume_ms={:.3} compaction_ms={:.3} prune_ms={:.3} validation_ms={:.3} index_bytes_read={}",
                report.outbox_resume_ms,
                report.work.compaction_ms,
                report.work.prune_ms,
                report.work.validation_ms,
                report.work.index_bytes_read,
            );
            round_ms.push(report.outbox_resume_ms);
        }
        let slowest = round_ms.iter().copied().fold(0.0_f64, f64::max);
        assert!(
            slowest <= 60.0,
            "at-cap steady-state resume must stay bounded; observed {round_ms:?}"
        );
        std::fs::remove_dir_all(root).expect("cleanup release at-cap steady test");
    }
}
