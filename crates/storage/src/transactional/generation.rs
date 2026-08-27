use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionalGenerationPointerV1 {
    pub schema: String,
    pub generation: String,
    pub database_directory: PathBuf,
    pub database_file: String,
    pub migration_packet_root: String,
}

impl NodeStore {
    pub fn open_transactional_store(&self) -> StorageResult<TransactionalStore> {
        let directory = self
            .transactional_database_directory()
            .map_err(|error| StorageError::new(StorageErrorCode::Database, error.to_string()))?;
        if self.read_only {
            TransactionalStore::open_read_only_with_integrity_key(
                &directory,
                self.integrity_key.clone(),
            )
        } else {
            TransactionalStore::open_with_integrity_key(&directory, self.integrity_key.clone())
        }
    }

    pub fn open_transactional_store_at(
        &self,
        database_directory: impl AsRef<Path>,
    ) -> StorageResult<TransactionalStore> {
        self.ensure_writable()
            .map_err(|error| StorageError::new(StorageErrorCode::Database, error.to_string()))?;
        TransactionalStore::open_with_integrity_key(
            database_directory.as_ref(),
            self.integrity_key.clone(),
        )
    }

    pub fn open_transactional_store_read_only_at(
        &self,
        database_directory: impl AsRef<Path>,
    ) -> StorageResult<TransactionalStore> {
        TransactionalStore::open_read_only_with_integrity_key(
            database_directory.as_ref(),
            self.integrity_key.clone(),
        )
    }

    pub fn transactional_generation_pointer(
        &self,
    ) -> io::Result<Option<TransactionalGenerationPointerV1>> {
        let path = self.data_dir.join(TRANSACTIONAL_GENERATION_POINTER_FILE);
        let pointer = match self.read_json(path) {
            Ok(pointer) => pointer,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        validate_generation_pointer(&pointer)?;
        Ok(Some(pointer))
    }

    pub fn publish_transactional_generation(
        &self,
        database_directory: impl AsRef<Path>,
        migration_packet_root: &str,
    ) -> io::Result<TransactionalGenerationPointerV1> {
        self.ensure_writable()?;
        let canonical_directory = fs::canonicalize(database_directory.as_ref())?;
        let store = TransactionalStore::open_with_integrity_key(
            &canonical_directory,
            self.integrity_key.clone(),
        )?;
        let meta = store.meta()?;
        if meta.migration_packet_root.as_deref() != Some(migration_packet_root)
            || meta.verifier_version.as_deref() != Some(TRANSACTIONAL_VERIFIER_VERSION)
            || meta.last_full_verification_height != Some(meta.finalized_height)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "transactional generation is not fully verified for the requested migration packet",
            ));
        }
        drop(store);
        let pointer = TransactionalGenerationPointerV1 {
            schema: TRANSACTIONAL_GENERATION_POINTER_SCHEMA.to_owned(),
            generation: meta.generation,
            database_directory: canonical_directory,
            database_file: TRANSACTIONAL_DATABASE_FILE.to_owned(),
            migration_packet_root: migration_packet_root.to_owned(),
        };
        validate_generation_pointer(&pointer)?;
        self.write_json(
            self.data_dir.join(TRANSACTIONAL_GENERATION_POINTER_FILE),
            &pointer,
        )?;
        *self
            .transactional_store
            .lock()
            .map_err(|_| io::Error::other("transactional store handle lock is poisoned"))? = None;
        Ok(pointer)
    }

    /// Return the process-local shared database handle for this node store.
    /// Cloned `NodeStore` values share the same handle, allowing concurrent
    /// redb read transactions without attempting to open the file twice.
    pub fn transactional_store(&self) -> StorageResult<Arc<TransactionalStore>> {
        let mut cached = self.transactional_store.lock().map_err(|_| {
            StorageError::new(
                StorageErrorCode::Database,
                "transactional store handle lock is poisoned",
            )
        })?;
        if let Some(store) = cached.as_ref() {
            return Ok(Arc::clone(store));
        }
        let pointer = self.transactional_generation_pointer().map_err(|error| {
            StorageError::new(StorageErrorCode::IntegrityFailure, error.to_string())
        })?;
        let directory = pointer
            .as_ref()
            .map(|pointer| pointer.database_directory.clone())
            .unwrap_or_else(|| self.data_dir.clone());
        let store = if self.read_only {
            Arc::new(TransactionalStore::open_read_only_with_integrity_key(
                &directory,
                self.integrity_key.clone(),
            )?)
        } else {
            shared_transactional_store(&directory, self.integrity_key.clone())?
        };
        if let Some(pointer) = pointer.as_ref() {
            validate_generation_pointer_binding(pointer, &store.meta()?)?;
        }
        *cached = Some(Arc::clone(&store));
        Ok(store)
    }

    pub fn transactional_storage_configured(&self) -> io::Result<bool> {
        let directory = self.transactional_database_directory()?;
        if !directory.join(TRANSACTIONAL_DATABASE_FILE).exists() {
            return Ok(false);
        }
        self.transactional_store()?.meta()?;
        Ok(true)
    }

    pub fn transactional_storage_active(&self) -> io::Result<bool> {
        if !self.storage_backend_mode()?.is_transactional() {
            return Ok(false);
        }
        let directory = self.transactional_database_directory()?;
        if !directory.join(TRANSACTIONAL_DATABASE_FILE).exists() {
            return Ok(false);
        }
        let meta = self.transactional_store()?.meta()?;
        let Some(activation_height) = meta.scheduled_activation_height else {
            return Ok(false);
        };
        Ok(meta.finalized_height >= activation_height)
    }

    fn transactional_database_directory(&self) -> io::Result<PathBuf> {
        Ok(self
            .transactional_generation_pointer()?
            .map(|pointer| pointer.database_directory)
            .unwrap_or_else(|| self.data_dir.clone()))
    }
}

fn validate_generation_pointer(pointer: &TransactionalGenerationPointerV1) -> io::Result<()> {
    if pointer.schema != TRANSACTIONAL_GENERATION_POINTER_SCHEMA
        || pointer.generation != TRANSACTIONAL_GENERATION
        || pointer.database_file != TRANSACTIONAL_DATABASE_FILE
        || !pointer.database_directory.is_absolute()
        || pointer.migration_packet_root.len() != 96
        || !pointer
            .migration_packet_root
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !pointer
            .database_directory
            .join(TRANSACTIONAL_DATABASE_FILE)
            .is_file()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "transactional generation pointer is invalid or stale",
        ));
    }
    Ok(())
}

fn validate_generation_pointer_binding(
    pointer: &TransactionalGenerationPointerV1,
    meta: &TransactionalStoreMetaV1,
) -> StorageResult<()> {
    if meta.generation != pointer.generation
        || meta.migration_packet_root.as_deref() != Some(pointer.migration_packet_root.as_str())
        || meta.verifier_version.as_deref() != Some(TRANSACTIONAL_VERIFIER_VERSION)
        || meta.last_full_verification_height.is_none()
    {
        return Err(StorageError::new(
            StorageErrorCode::IntegrityFailure,
            "published transactional generation does not match its authenticated pointer binding",
        ));
    }
    Ok(())
}
