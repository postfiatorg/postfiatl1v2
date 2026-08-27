use super::*;

pub const STORAGE_BACKEND_MODE_FILE: &str = "storage_backend_mode.json";
pub const STORAGE_BACKEND_MODE_SCHEMA: &str = "postfiat-storage-backend-mode-v1";

/// Node-local durable backend selection.
///
/// This value never enters snapshots, proposals, state roots, blocks, receipts,
/// certificates, or any other consensus artifact. Missing configuration means
/// the selected transactional backend so existing nodes preserve their current
/// behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StorageBackendMode {
    LegacyJsonl,
    BoundedJsonl,
    #[default]
    Transactional,
}

impl StorageBackendMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LegacyJsonl => "legacy-jsonl",
            Self::BoundedJsonl => "bounded-jsonl",
            Self::Transactional => "transactional",
        }
    }

    pub const fn is_transactional(self) -> bool {
        matches!(self, Self::Transactional)
    }

    pub const fn is_comparison_only(self) -> bool {
        !self.is_transactional()
    }
}

impl std::str::FromStr for StorageBackendMode {
    type Err = io::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "legacy-jsonl" => Ok(Self::LegacyJsonl),
            "bounded-jsonl" => Ok(Self::BoundedJsonl),
            "transactional" => Ok(Self::Transactional),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unsupported storage backend mode `{other}`; expected legacy-jsonl, bounded-jsonl, or transactional"
                ),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StorageBackendModeRecordV1 {
    schema: String,
    mode: StorageBackendMode,
    comparison_only: bool,
}

impl NodeStore {
    pub fn storage_backend_mode(&self) -> io::Result<StorageBackendMode> {
        let path = self.data_dir.join(STORAGE_BACKEND_MODE_FILE);
        let record: StorageBackendModeRecordV1 = match self.read_json(path) {
            Ok(record) => record,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(StorageBackendMode::default());
            }
            Err(error) => return Err(error),
        };
        if record.schema != STORAGE_BACKEND_MODE_SCHEMA
            || record.comparison_only != record.mode.is_comparison_only()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "storage_backend_mode_invalid: authenticated backend-mode record is inconsistent",
            ));
        }
        Ok(record.mode)
    }

    /// Persist an authenticated node-local backend choice.
    ///
    /// The caller owns the offline/operator acknowledgement and must validate
    /// the selected backend against the exact certified tip before invoking
    /// this low-level storage operation.
    pub fn write_storage_backend_mode(&self, mode: StorageBackendMode) -> io::Result<()> {
        self.ensure_writable()?;
        if self.read_ordered_commit_journal_raw()?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "storage_backend_mode_pending_commit: recover the ordered-commit journal before changing backend mode",
            ));
        }
        self.write_json(
            self.data_dir.join(STORAGE_BACKEND_MODE_FILE),
            &StorageBackendModeRecordV1 {
                schema: STORAGE_BACKEND_MODE_SCHEMA.to_owned(),
                mode,
                comparison_only: mode.is_comparison_only(),
            },
        )
    }

    /// Recompute the versioned commitment from canonical ordered history.
    ///
    /// This deliberately materializes and hashes the full ordered list. It is
    /// the controlling comparison behavior for the legacy lane and remains
    /// suitable for offline audit/replay.
    pub fn legacy_ordered_history_commitment(&self) -> io::Result<OrderedHistoryCommitment> {
        let context = self.jsonl_checkpoint_context()?;
        if context.chain_id.is_empty() || context.genesis_hash.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history commitment requires initialized genesis",
            ));
        }
        let batches = self.read_ordered_batches()?;
        let mut commitment = OrderedHistoryCommitment::genesis(
            &context.chain_id,
            &context.genesis_hash,
            context.protocol_version,
        )?;
        for batch_id in batches {
            commitment = commitment.append(&batch_id)?;
        }
        let expected_count = match self.read_chain_tip() {
            Ok(tip) => tip.ordered_batch_count,
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error),
        };
        if commitment.count != expected_count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ordered-history commitment count does not match the certified chain tip",
            ));
        }
        Ok(commitment)
    }

    /// Read the backend-independent consensus commitment through the selected
    /// local storage implementation.
    pub fn backend_ordered_history_commitment(&self) -> io::Result<OrderedHistoryCommitment> {
        let commitment = match self.storage_backend_mode()? {
            StorageBackendMode::LegacyJsonl => self.legacy_ordered_history_commitment()?,
            StorageBackendMode::BoundedJsonl => self.ordered_history_commitment()?,
            StorageBackendMode::Transactional => self
                .transactional_store()?
                .meta()?
                .ordered_history_commitment(),
        };
        let genesis = self.read_genesis()?;
        let genesis_json = genesis.to_json().map_err(invalid_data)?;
        let genesis_hash = to_hex(&legacy_checksum(
            b"postfiat.genesis.v1",
            genesis_json.as_bytes(),
        ));
        commitment.validate_domain(&genesis.chain_id, &genesis_hash, genesis.protocol_version)?;
        let expected_count = match self.read_chain_tip() {
            Ok(tip) => tip.ordered_batch_count,
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error),
        };
        if commitment.count != expected_count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "selected backend ordered-history count does not match the certified chain tip",
            ));
        }
        Ok(commitment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn missing_mode_defaults_to_transactional_and_records_are_authenticated() {
        let dir = test_dir("postfiat-storage-backend-mode");
        let store = NodeStore::new(&dir);
        assert_eq!(
            store.storage_backend_mode().expect("default mode"),
            StorageBackendMode::Transactional
        );
        store
            .write_storage_backend_mode(StorageBackendMode::LegacyJsonl)
            .expect("write comparison mode");
        assert_eq!(
            store.storage_backend_mode().expect("configured mode"),
            StorageBackendMode::LegacyJsonl
        );

        let path = dir.join(STORAGE_BACKEND_MODE_FILE);
        let mut bytes = fs::read(&path).expect("read mode record");
        let offset = bytes
            .iter()
            .position(|byte| *byte == b'l')
            .expect("legacy marker");
        bytes[offset] = b'x';
        fs::write(&path, bytes).expect("tamper mode record");
        assert!(store.storage_backend_mode().is_err());
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
