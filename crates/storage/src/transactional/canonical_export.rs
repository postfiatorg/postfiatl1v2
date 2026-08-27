use super::*;
use sha3::{Digest, Sha3_384};

const CANONICAL_EXPORT_RECORD_SCHEMA: &str = "postfiat-transactional-canonical-export-record-v1";
const CANONICAL_EXPORT_FOOTER_SCHEMA: &str = "postfiat-transactional-canonical-export-footer-v1";
const CANONICAL_EXPORT_RECEIPT_SCHEMA: &str = "postfiat-transactional-canonical-export-receipt-v1";
const CANONICAL_EXPORT_HASH_DOMAIN: &[u8] = b"postfiat.transactional.canonical-export.records.v1";
const MAX_CANONICAL_EXPORT_BYTES: usize = 512 * 1024 * 1024;
const MAX_CANONICAL_EXPORT_RECORDS: usize = 10_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalExportReceiptV1 {
    pub schema: String,
    pub finalized_height: u64,
    pub record_count: u64,
    pub records_sha3_384: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CanonicalExportRecordV1 {
    schema: String,
    table: String,
    key: String,
    canonical_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CanonicalExportFooterV1 {
    schema: String,
    finalized_height: u64,
    record_count: u64,
    records_sha3_384: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalCurrentStateEntry {
    domain: String,
    canonical_json: String,
}

fn export_error(code: StorageErrorCode, reason: &str, detail: impl fmt::Display) -> StorageError {
    StorageError::new(code, format!("{reason}: {detail}"))
}

fn canonical_json<T: Serialize + ?Sized>(value: &T) -> StorageResult<String> {
    serde_json::to_string(value).map_err(serialization_error)
}

fn export_record(
    table: &str,
    key: impl Into<String>,
    canonical_json: String,
) -> CanonicalExportRecordV1 {
    CanonicalExportRecordV1 {
        schema: CANONICAL_EXPORT_RECORD_SCHEMA.to_owned(),
        table: table.to_owned(),
        key: key.into(),
        canonical_json,
    }
}

fn canonical_export_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update(CANONICAL_EXPORT_HASH_DOMAIN);
    hasher.update([0_u8]);
    hasher.update(bytes);
    crate::integrity::to_hex(&hasher.finalize())
}

impl TransactionalStore {
    fn canonical_current_state_entries(&self) -> StorageResult<Vec<CanonicalCurrentStateEntry>> {
        self.counters
            .read_transactions
            .fetch_add(1, Ordering::Relaxed);
        let transaction = self.database.begin_read().map_err(database_error)?;
        let table = transaction
            .open_table(CURRENT_STATE)
            .map_err(database_error)?;
        let capacity = usize::try_from(table.len().map_err(database_error)?).map_err(|_| {
            StorageError::new(
                StorageErrorCode::SizeLimit,
                "current-state export is too large",
            )
        })?;
        let mut entries = Vec::with_capacity(capacity);
        let mut byte_count = 0_u64;
        for entry in table.iter().map_err(database_error)? {
            let (key, value) = entry.map_err(database_error)?;
            let domain = std::str::from_utf8(key.value())
                .map_err(|_| {
                    StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        "current-state export key is not UTF-8",
                    )
                })?
                .to_owned();
            validate_state_domain(&domain, false)?;
            let canonical = decode_authenticated_value(
                CURRENT_STATE_TABLE,
                key.value(),
                value.value(),
                MAX_CURRENT_STATE_BYTES,
                &self.integrity_key,
            )?;
            serde_json::from_slice::<serde_json::Value>(&canonical).map_err(serialization_error)?;
            let canonical_json = String::from_utf8(canonical).map_err(|_| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    "current-state export value is not UTF-8 JSON",
                )
            })?;
            byte_count = byte_count
                .checked_add(domain.len() as u64)
                .and_then(|total| total.checked_add(canonical_json.len() as u64))
                .ok_or_else(|| {
                    StorageError::new(
                        StorageErrorCode::SizeLimit,
                        "current-state export byte count overflow",
                    )
                })?;
            entries.push(CanonicalCurrentStateEntry {
                domain,
                canonical_json,
            });
        }
        self.counters
            .full_history_scans
            .fetch_add(1, Ordering::Relaxed);
        self.counters
            .full_history_records_read
            .fetch_add(entries.len() as u64, Ordering::Relaxed);
        self.counters
            .full_history_bytes_read
            .fetch_add(byte_count, Ordering::Relaxed);
        Ok(entries)
    }

    fn canonical_export_records(
        &self,
    ) -> StorageResult<(Vec<CanonicalExportRecordV1>, TransactionalStoreMetaV1)> {
        self.verify_logical_integrity()?;
        let meta = self.meta()?;
        let blocks = self.blocks_in_height_order()?;
        let mut records = Vec::new();
        records.push(export_record(
            META_TABLE,
            "store-meta-v1",
            canonical_json(&meta)?,
        ));

        for block in &blocks {
            let height_key = format!("{:016x}", block.header.height);
            records.push(export_record(
                BLOCKS_BY_HEIGHT_TABLE,
                height_key,
                canonical_json(block)?,
            ));
            records.push(export_record(
                BLOCK_HEIGHT_BY_HASH_TABLE,
                block.header.block_hash.clone(),
                canonical_json(&block.header.height)?,
            ));
            for (position, receipt_id) in block.receipt_ids.iter().enumerate() {
                let receipt = self
                    .receipt_at_height(receipt_id, block.header.height)?
                    .ok_or_else(|| {
                        StorageError::new(
                            StorageErrorCode::CorruptRecord,
                            format!(
                                "canonical export receipt `{receipt_id}` at height {} is missing",
                                block.header.height
                            ),
                        )
                    })?;
                records.push(export_record(
                    RECEIPTS_BY_ID_TABLE,
                    canonical_json(&(receipt_id, block.header.height, position))?,
                    canonical_json(&receipt)?,
                ));
            }
            let archive = self
                .archived_batch(&block.header.batch_kind, &block.header.batch_id)?
                .ok_or_else(|| {
                    StorageError::new(
                        StorageErrorCode::CorruptRecord,
                        format!(
                            "canonical export archive for block {} is missing",
                            block.header.height
                        ),
                    )
                })?;
            records.push(export_record(
                BATCH_ARCHIVE_TABLE,
                canonical_json(&(&block.header.batch_kind, &block.header.batch_id))?,
                canonical_json(&archive)?,
            ));
        }

        for ordinal in 1..=meta.ordered_batch_count {
            let batch_id = self.ordered_batch_by_ordinal(ordinal)?.ok_or_else(|| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    format!("canonical export ordered ordinal {ordinal} is missing"),
                )
            })?;
            records.push(export_record(
                ORDERED_BY_ORDINAL_TABLE,
                format!("{ordinal:016x}"),
                canonical_json(&batch_id)?,
            ));
        }
        for entry in self.canonical_current_state_entries()? {
            records.push(export_record(
                CURRENT_STATE_TABLE,
                entry.domain,
                entry.canonical_json,
            ));
        }
        for entry in self.canonical_history_index_entries()? {
            records.push(export_record(
                HISTORY_INDEXES_TABLE,
                entry.key_hex,
                entry.canonical_json,
            ));
        }

        records.sort_by(|left, right| (&left.table, &left.key).cmp(&(&right.table, &right.key)));
        if records
            .windows(2)
            .any(|pair| pair[0].table == pair[1].table && pair[0].key == pair[1].key)
        {
            return Err(StorageError::new(
                StorageErrorCode::CorruptRecord,
                "canonical export contains a duplicate table/key record",
            ));
        }
        if records.len() > MAX_CANONICAL_EXPORT_RECORDS {
            return Err(StorageError::new(
                StorageErrorCode::SizeLimit,
                "canonical export record count exceeds its closed bound",
            ));
        }
        Ok((records, meta))
    }

    fn render_canonical_jsonl_export(&self) -> StorageResult<(Vec<u8>, CanonicalExportReceiptV1)> {
        let (records, meta) = self.canonical_export_records()?;
        let mut bytes = Vec::new();
        for record in &records {
            let encoded = serde_json::to_vec(record).map_err(serialization_error)?;
            let next_len = bytes
                .len()
                .checked_add(encoded.len())
                .and_then(|length| length.checked_add(1))
                .ok_or_else(|| {
                    StorageError::new(
                        StorageErrorCode::SizeLimit,
                        "canonical export byte count overflow",
                    )
                })?;
            if next_len > MAX_CANONICAL_EXPORT_BYTES {
                return Err(StorageError::new(
                    StorageErrorCode::SizeLimit,
                    "canonical export exceeds its closed byte bound",
                ));
            }
            bytes.extend_from_slice(&encoded);
            bytes.push(b'\n');
        }
        let root = canonical_export_hash(&bytes);
        let record_count = records.len() as u64;
        let footer = CanonicalExportFooterV1 {
            schema: CANONICAL_EXPORT_FOOTER_SCHEMA.to_owned(),
            finalized_height: meta.finalized_height,
            record_count,
            records_sha3_384: root.clone(),
        };
        let encoded = serde_json::to_vec(&footer).map_err(serialization_error)?;
        if bytes
            .len()
            .checked_add(encoded.len())
            .and_then(|length| length.checked_add(1))
            .is_none_or(|length| length > MAX_CANONICAL_EXPORT_BYTES)
        {
            return Err(StorageError::new(
                StorageErrorCode::SizeLimit,
                "canonical export footer exceeds its closed byte bound",
            ));
        }
        bytes.extend_from_slice(&encoded);
        bytes.push(b'\n');
        Ok((
            bytes,
            CanonicalExportReceiptV1 {
                schema: CANONICAL_EXPORT_RECEIPT_SCHEMA.to_owned(),
                finalized_height: meta.finalized_height,
                record_count,
                records_sha3_384: root,
            },
        ))
    }

    /// Write a deterministic JSONL audit export after a complete authenticated
    /// logical scan. This is never used by proposal, vote, or finalized commit.
    pub fn write_canonical_jsonl_export(
        &self,
        output: impl AsRef<Path>,
    ) -> StorageResult<CanonicalExportReceiptV1> {
        let output = output.as_ref();
        let (bytes, expected) = self.render_canonical_jsonl_export()?;
        crate::atomic_write_checked(output, &bytes, |temporary| {
            verify_canonical_export_file(temporary)
                .map(|_| ())
                .map_err(io::Error::from)
        })
        .map_err(|error| {
            export_error(
                StorageErrorCode::Database,
                "storage_canonical_export_write_failed",
                error,
            )
        })?;
        let observed = verify_canonical_export_file(output)?;
        if observed != expected {
            return Err(export_error(
                StorageErrorCode::IntegrityFailure,
                "storage_canonical_export_integrity_failure",
                "written export receipt differs from the authenticated source",
            ));
        }
        Ok(observed)
    }

    /// Verify a JSONL export and require it to be the exact deterministic
    /// export of this authenticated logical store.
    pub fn verify_canonical_jsonl_export(
        &self,
        input: impl AsRef<Path>,
    ) -> StorageResult<CanonicalExportReceiptV1> {
        let observed = verify_canonical_export_file(input.as_ref())?;
        let (_, expected) = self.render_canonical_jsonl_export()?;
        if observed != expected {
            return Err(export_error(
                StorageErrorCode::IntegrityFailure,
                "storage_canonical_export_substituted",
                "export root, count, or finalized height differs from the authenticated store",
            ));
        }
        Ok(observed)
    }
}

fn verify_canonical_export_file(path: &Path) -> StorageResult<CanonicalExportReceiptV1> {
    let bytes = fs::read(path).map_err(|error| {
        let reason = if error.kind() == io::ErrorKind::NotFound {
            "storage_canonical_export_missing"
        } else {
            "storage_canonical_export_read_failed"
        };
        export_error(StorageErrorCode::Database, reason, error)
    })?;
    if bytes.len() > MAX_CANONICAL_EXPORT_BYTES {
        return Err(export_error(
            StorageErrorCode::SizeLimit,
            "storage_canonical_export_size_limit",
            "export exceeds its closed byte bound",
        ));
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return Err(export_error(
            StorageErrorCode::CorruptRecord,
            "storage_canonical_export_integrity_failure",
            "export is empty or lacks its terminal newline",
        ));
    }
    let mut lines = bytes[..bytes.len() - 1].split(|byte| *byte == b'\n');
    let all_lines = lines.by_ref().collect::<Vec<_>>();
    if all_lines.len() < 2 || all_lines.len() > MAX_CANONICAL_EXPORT_RECORDS.saturating_add(1) {
        return Err(export_error(
            StorageErrorCode::SizeLimit,
            "storage_canonical_export_size_limit",
            "export has an invalid record count",
        ));
    }
    let footer: CanonicalExportFooterV1 =
        serde_json::from_slice(all_lines.last().expect("checked nonempty")).map_err(|error| {
            export_error(
                StorageErrorCode::CorruptRecord,
                "storage_canonical_export_integrity_failure",
                error,
            )
        })?;
    if footer.schema != CANONICAL_EXPORT_FOOTER_SCHEMA {
        return Err(export_error(
            StorageErrorCode::UnsupportedSchema,
            "storage_canonical_export_integrity_failure",
            "footer schema is unsupported",
        ));
    }

    let record_lines = &all_lines[..all_lines.len() - 1];
    let mut authenticated_bytes = Vec::new();
    let mut records = Vec::with_capacity(record_lines.len());
    for line in record_lines {
        authenticated_bytes.extend_from_slice(line);
        authenticated_bytes.push(b'\n');
        let record: CanonicalExportRecordV1 = serde_json::from_slice(line).map_err(|error| {
            export_error(
                StorageErrorCode::CorruptRecord,
                "storage_canonical_export_integrity_failure",
                error,
            )
        })?;
        if record.schema != CANONICAL_EXPORT_RECORD_SCHEMA
            || record.table.is_empty()
            || record.key.is_empty()
            || record.canonical_json.len() > MAX_CURRENT_STATE_BYTES
            || serde_json::from_str::<serde_json::Value>(&record.canonical_json).is_err()
        {
            return Err(export_error(
                StorageErrorCode::CorruptRecord,
                "storage_canonical_export_integrity_failure",
                "record schema, key, or canonical JSON is invalid",
            ));
        }
        records.push(record);
    }
    let record_count = records.len() as u64;
    if footer.record_count != record_count
        || footer.records_sha3_384 != canonical_export_hash(&authenticated_bytes)
    {
        return Err(export_error(
            StorageErrorCode::IntegrityFailure,
            "storage_canonical_export_integrity_failure",
            "footer count or authenticated record root is invalid",
        ));
    }
    if records
        .windows(2)
        .any(|pair| (&pair[0].table, &pair[0].key) >= (&pair[1].table, &pair[1].key))
    {
        return Err(export_error(
            StorageErrorCode::CorruptRecord,
            "storage_canonical_export_integrity_failure",
            "records are duplicated or not in canonical table/key order",
        ));
    }

    let meta_records = records
        .iter()
        .filter(|record| record.table == META_TABLE)
        .collect::<Vec<_>>();
    if meta_records.len() != 1 || meta_records[0].key != "store-meta-v1" {
        return Err(export_error(
            StorageErrorCode::CorruptRecord,
            "storage_canonical_export_integrity_failure",
            "export does not contain exactly one metadata record",
        ));
    }
    let meta: TransactionalStoreMetaV1 = serde_json::from_str(&meta_records[0].canonical_json)
        .map_err(|error| {
            export_error(
                StorageErrorCode::CorruptRecord,
                "storage_canonical_export_integrity_failure",
                error,
            )
        })?;
    meta.validate().map_err(|error| {
        export_error(
            error.code(),
            "storage_canonical_export_integrity_failure",
            error,
        )
    })?;
    let retained_block_count = meta
        .finalized_height
        .checked_sub(meta.history_base_height)
        .ok_or_else(|| {
            export_error(
                StorageErrorCode::CountMismatch,
                "storage_canonical_export_integrity_failure",
                "history base exceeds finalized height",
            )
        })?;
    let count = |table: &str| -> u64 {
        records
            .iter()
            .filter(|record| record.table == table)
            .count() as u64
    };
    if footer.finalized_height != meta.finalized_height
        || count(BLOCKS_BY_HEIGHT_TABLE) != retained_block_count
        || count(BLOCK_HEIGHT_BY_HASH_TABLE) != retained_block_count
        || count(RECEIPTS_BY_ID_TABLE) != meta.receipt_count
        || count(BATCH_ARCHIVE_TABLE) != retained_block_count
        || count(ORDERED_BY_ORDINAL_TABLE) != meta.ordered_batch_count
    {
        return Err(export_error(
            StorageErrorCode::CountMismatch,
            "storage_canonical_export_integrity_failure",
            "logical table counts disagree with exported metadata",
        ));
    }
    Ok(CanonicalExportReceiptV1 {
        schema: CANONICAL_EXPORT_RECEIPT_SCHEMA.to_owned(),
        finalized_height: footer.finalized_height,
        record_count,
        records_sha3_384: footer.records_sha3_384,
    })
}
