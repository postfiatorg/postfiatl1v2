use super::*;

/// Canonical logical representation of one rebuildable history-index entry.
///
/// Physical redb pages are deliberately excluded. Offline rebuild, snapshot,
/// and migration checks compare these authenticated logical keys and exact
/// canonical JSON bytes instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalHistoryIndexEntryV1 {
    pub key_hex: String,
    pub canonical_json: String,
}

impl TransactionalStore {
    /// Export every rebuildable history index in redb's ordered key order.
    ///
    /// This is an explicit audit operation and therefore records one
    /// full-history scan. Consensus and finalized-commit callers must not use
    /// it.
    pub fn canonical_history_index_entries(
        &self,
    ) -> StorageResult<Vec<CanonicalHistoryIndexEntryV1>> {
        self.counters
            .read_transactions
            .fetch_add(1, Ordering::Relaxed);
        let transaction = self.database.begin_read().map_err(database_error)?;
        let table = transaction
            .open_table(HISTORY_INDEXES)
            .map_err(database_error)?;
        let capacity = usize::try_from(table.len().map_err(database_error)?).map_err(|_| {
            StorageError::new(
                StorageErrorCode::SizeLimit,
                "history-index export is too large",
            )
        })?;
        let mut entries = Vec::with_capacity(capacity);
        let mut byte_count = 0_u64;
        for entry in table.iter().map_err(database_error)? {
            let (key, value) = entry.map_err(database_error)?;
            let canonical = decode_authenticated_value(
                HISTORY_INDEXES_TABLE,
                key.value(),
                value.value(),
                MAX_RECORD_BYTES,
                &self.integrity_key,
            )?;
            let canonical_json = String::from_utf8(canonical).map_err(|_| {
                StorageError::new(
                    StorageErrorCode::CorruptRecord,
                    "history-index value is not canonical UTF-8 JSON",
                )
            })?;
            byte_count = byte_count
                .checked_add(key.value().len() as u64)
                .and_then(|count| count.checked_add(canonical_json.len() as u64))
                .ok_or_else(|| {
                    StorageError::new(
                        StorageErrorCode::SizeLimit,
                        "history-index export byte count overflow",
                    )
                })?;
            entries.push(CanonicalHistoryIndexEntryV1 {
                key_hex: crate::integrity::to_hex(key.value()),
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
}
