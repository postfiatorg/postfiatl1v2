use super::*;

/// Live writable handles whose generation pointer binding was validated when
/// they were acquired, keyed by node data directory. An entry is reused only
/// while its handle is open, so the binding is re-validated on the next open.
static VALIDATED_HANDLES: OnceLock<Mutex<HashMap<PathBuf, Weak<TransactionalStore>>>> =
    OnceLock::new();

fn validated_handle(data_dir: &Path) -> Option<Arc<TransactionalStore>> {
    VALIDATED_HANDLES
        .get()?
        .lock()
        .ok()?
        .get(data_dir)
        .and_then(Weak::upgrade)
}

fn remember_validated_handle(data_dir: &Path, store: &Arc<TransactionalStore>) {
    if let Ok(mut handles) = VALIDATED_HANDLES.get_or_init(Default::default).lock() {
        handles.retain(|_, handle| handle.strong_count() > 0);
        handles.insert(data_dir.to_path_buf(), Arc::downgrade(store));
    }
}

fn forget_validated_handle(data_dir: &Path) {
    if let Some(Ok(mut handles)) = VALIDATED_HANDLES.get().map(Mutex::lock) {
        handles.remove(data_dir);
    }
}

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
        forget_validated_handle(&self.data_dir);
        self.write_json(
            self.data_dir.join(TRANSACTIONAL_GENERATION_POINTER_FILE),
            &pointer,
        )?;
        Ok(pointer)
    }

    /// Rebind an authenticated generation pointer for an imminent atomic move
    /// of the entire data directory. The future directory must be absolute,
    /// and the current generation must be contained inside this store.
    ///
    /// After this returns, the current directory's pointer intentionally names
    /// the future location. The caller must either atomically rename the whole
    /// directory into that location or discard the unpublished directory.
    pub fn prepare_transactional_generation_for_data_dir_move(
        &self,
        future_data_dir: impl AsRef<Path>,
    ) -> io::Result<()> {
        let Some(pointer) = self.transactional_generation_pointer()? else {
            return Ok(());
        };
        let future_data_dir = future_data_dir.as_ref();
        if !future_data_dir.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "future transactional data directory must be absolute",
            ));
        }
        let current_data_dir = fs::canonicalize(&self.data_dir)?;
        let relative_generation = pointer
            .database_directory
            .strip_prefix(&current_data_dir)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "transactional generation is outside the data directory being moved",
                )
            })?;
        if relative_generation.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "transactional generation must be a child of the data directory being moved",
            ));
        }
        let store = self.transactional_store()?;
        validate_generation_pointer_binding(&pointer, &store.meta()?)?;
        drop(store);

        let relocated = TransactionalGenerationPointerV1 {
            database_directory: future_data_dir.join(relative_generation),
            ..pointer
        };
        validate_generation_pointer_shape(&relocated)?;
        forget_validated_handle(&self.data_dir);
        self.write_json(
            self.data_dir.join(TRANSACTIONAL_GENERATION_POINTER_FILE),
            &relocated,
        )
    }

    /// Acquire the shared database handle for this node store under the
    /// bounded writer lease.
    ///
    /// The returned `Arc` is the lease: concurrent callers inside one process
    /// share a single live handle, and when the last caller drops its `Arc`
    /// the database closes and the file lock is released for the other
    /// service. Callers must not store the handle beyond the operation that
    /// acquired it. `redb` permits no cross-process sharing at all (a writer
    /// excludes read-only opens too), so holding an idle handle starves the
    /// sibling `transport-validator-serve`/`rpc-serve` process; that is the
    /// exact defect that stopped the 2026-08-30 storage canary.
    ///
    /// While a writable handle is open, later lookups reuse it and its
    /// validated pointer binding without reading the pointer file again.
    pub fn transactional_store(&self) -> StorageResult<Arc<TransactionalStore>> {
        if let Some(store) = self.live_transactional_store() {
            return Ok(store);
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
        if !self.read_only {
            remember_validated_handle(&self.data_dir, &store);
        }
        Ok(store)
    }

    /// Hold the database open for one consensus operation (STO-07).
    ///
    /// Every store lookup made while the returned lease is alive reuses its
    /// handle and validated pointer binding; dropping it closes the database
    /// so the sibling service can open it between operations. Returns `None`
    /// for read-only or non-transactional stores and when no database exists.
    /// It adds no failure mode: a pointer or binding error is left to the
    /// lookups that already report it, and only `storage_writer_busy` fails.
    pub fn transactional_store_lease(&self) -> io::Result<Option<Arc<TransactionalStore>>> {
        if self.read_only
            || !matches!(self.storage_backend_mode(), Ok(mode) if mode.is_transactional())
        {
            return Ok(None);
        }
        if let Some(store) = self.live_transactional_store() {
            return Ok(Some(store));
        }
        match self.transactional_database_directory() {
            Ok(directory) if directory.join(TRANSACTIONAL_DATABASE_FILE).exists() => {}
            _ => return Ok(None),
        }
        match self.transactional_store() {
            Ok(store) => Ok(Some(store)),
            Err(error) if error.code() == StorageErrorCode::WriterBusy => Err(error.into()),
            Err(_) => Ok(None),
        }
    }

    /// Snapshot the already validated handle held by this process for an
    /// operation. A sibling reader must never open or retain a writer lease
    /// on an idle validator. Before activation, the legacy store is authoritative.
    pub fn held_transactional_read_snapshot(
        &self,
    ) -> StorageResult<Option<TransactionalReadSnapshot>> {
        let Some(store) = self.live_transactional_store() else {
            return Ok(None);
        };
        let snapshot = store.read_snapshot()?;
        let meta = snapshot.meta();
        Ok(meta
            .scheduled_activation_height
            .is_some_and(|height| meta.finalized_height >= height)
            .then_some(snapshot))
    }

    fn live_transactional_store(&self) -> Option<Arc<TransactionalStore>> {
        (!self.read_only)
            .then(|| validated_handle(&self.data_dir))
            .flatten()
    }

    pub fn transactional_storage_configured(&self) -> io::Result<bool> {
        if let Some(store) = self.live_transactional_store() {
            store.meta()?;
            return Ok(true);
        }
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
        let meta = match self.live_transactional_store() {
            Some(store) => store.meta()?,
            None => {
                let directory = self.transactional_database_directory()?;
                if !directory.join(TRANSACTIONAL_DATABASE_FILE).exists() {
                    return Ok(false);
                }
                self.transactional_store()?.meta()?
            }
        };
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
    validate_generation_pointer_shape(pointer)?;
    if !pointer
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

fn validate_generation_pointer_shape(pointer: &TransactionalGenerationPointerV1) -> io::Result<()> {
    if pointer.schema != TRANSACTIONAL_GENERATION_POINTER_SCHEMA
        || pointer.generation != TRANSACTIONAL_GENERATION
        || pointer.database_file != TRANSACTIONAL_DATABASE_FILE
        || !pointer.database_directory.is_absolute()
        || pointer.migration_packet_root.len() != 96
        || !pointer
            .migration_packet_root
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
