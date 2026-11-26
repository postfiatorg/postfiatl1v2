//! The process-shared writer lease and the database open/close counters.
//!
//! The lease is the weak registry introduced by `f0013c29`: a handle lives
//! exactly as long as some caller holds its `Arc`, so no process keeps the
//! redb file open while idle. Every open and close is counted here because
//! redb 4.2 rewrites the allocator state on each close and reloads it on each
//! open, a cost that grows with the database size (STO-07).

use super::*;

static SHARED_STORES: OnceLock<Mutex<HashMap<PathBuf, Weak<TransactionalStore>>>> = OnceLock::new();

/// Total time a writer-lease acquisition retries against a database held by
/// another process before failing closed with `storage_writer_busy`.
const WRITER_LEASE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(20);
const WRITER_LEASE_INITIAL_BACKOFF: std::time::Duration = std::time::Duration::from_millis(50);
const WRITER_LEASE_MAX_BACKOFF: std::time::Duration = std::time::Duration::from_millis(500);
const SAME_PROCESS_CLOSE_POLL: std::time::Duration = std::time::Duration::from_millis(1);
const SAME_PROCESS_CLOSE_POLLS: u32 = 50;

/// Database opens and closes counted by this process.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionalOpenCounts {
    pub database_opens: u64,
    pub database_closes: u64,
}

impl TransactionalOpenCounts {
    #[must_use]
    pub fn saturating_delta(self, before: Self) -> Self {
        Self {
            database_opens: self.database_opens.saturating_sub(before.database_opens),
            database_closes: self.database_closes.saturating_sub(before.database_closes),
        }
    }
}

static PROCESS_OPENS: AtomicU64 = AtomicU64::new(0);
static PROCESS_CLOSES: AtomicU64 = AtomicU64::new(0);
static PATH_OPENS: OnceLock<Mutex<HashMap<PathBuf, TransactionalOpenCounts>>> = OnceLock::new();

/// Opens and closes of every transactional database in this process.
pub fn transactional_open_counts() -> TransactionalOpenCounts {
    TransactionalOpenCounts {
        database_opens: PROCESS_OPENS.load(Ordering::Relaxed),
        database_closes: PROCESS_CLOSES.load(Ordering::Relaxed),
    }
}

/// Opens and closes of the database in `database_directory` in this process.
pub fn transactional_open_counts_for(database_directory: &Path) -> TransactionalOpenCounts {
    let key = open_key(&database_directory.join(TRANSACTIONAL_DATABASE_FILE));
    with_path_counts(&key, |counts| *counts).unwrap_or_default()
}

fn open_key(database_path: &Path) -> PathBuf {
    fs::canonicalize(database_path).unwrap_or_else(|_| database_path.to_path_buf())
}

fn with_path_counts<T>(
    key: &Path,
    update: impl FnOnce(&mut TransactionalOpenCounts) -> T,
) -> Option<T> {
    let mut paths = PATH_OPENS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()?;
    Some(update(paths.entry(key.to_path_buf()).or_default()))
}

/// Last field of [`TransactionalStore`]: dropped after the database, so its
/// drop records a completed close.
pub(super) struct OpenRecord(PathBuf);

pub(super) fn record_open(database_path: &Path) -> OpenRecord {
    let key = open_key(database_path);
    PROCESS_OPENS.fetch_add(1, Ordering::Relaxed);
    with_path_counts(&key, |counts| counts.database_opens += 1);
    OpenRecord(key)
}

impl Drop for OpenRecord {
    fn drop(&mut self) {
        PROCESS_CLOSES.fetch_add(1, Ordering::Relaxed);
        with_path_counts(&self.0, |counts| counts.database_closes += 1);
    }
}

/// True while a handle this process opened on `database_path` has not
/// finished closing: the contention is our own destructor, not another
/// process, and clears within the close itself.
fn same_process_close_pending(database_path: &Path) -> bool {
    with_path_counts(&open_key(database_path), |counts| {
        counts.database_opens > counts.database_closes
    })
    .unwrap_or(false)
}

/// Acquire the process-shared transactional store under the bounded writer
/// lease.
///
/// `redb` grants exactly one process access to the database file: a writable
/// [`Database`] excludes every other open, and [`ReadOnlyDatabase`] cannot
/// coexist with a writer either. The deployed topology runs two services
/// (`transport-validator-serve` and `rpc-serve`) against one data directory,
/// so no process may hold the database while idle. This function therefore
/// hands out a shared handle whose lifetime is exactly the set of in-flight
/// operations:
///
/// - Callers inside one process share a single live handle through a weak
///   registry entry, so concurrent redb read transactions never reopen the
///   file.
/// - When the last caller drops its `Arc`, the database closes and the file
///   lock is released for the other process.
/// - When the file is held by the other process, acquisition retries with
///   backoff until [`WRITER_LEASE_DEADLINE`], then fails closed with
///   `storage_writer_busy` without mutating anything. The drop race, in
///   which this process's own redb destructor is still releasing the lock of
///   a handle whose strong count already reached zero, is polled every
///   millisecond instead of backing off.
///
/// The devnet is operator-driven: blocks exist only during explicit certified
/// rounds and rounds are serialized by proposer election, so contention
/// windows are short and bounded.
pub(super) fn shared_transactional_store(
    data_dir: &Path,
    integrity_key: IntegrityKey,
) -> StorageResult<Arc<TransactionalStore>> {
    acquire_transactional_store(data_dir, integrity_key, true)
}

pub(super) fn acquire_transactional_store(
    data_dir: &Path,
    integrity_key: IntegrityKey,
    create: bool,
) -> StorageResult<Arc<TransactionalStore>> {
    if create {
        fs::create_dir_all(data_dir).map_err(database_error)?;
    }
    let canonical_dir = fs::canonicalize(data_dir).map_err(database_error)?;
    let database_path = canonical_dir.join(TRANSACTIONAL_DATABASE_FILE);
    let registry = SHARED_STORES.get_or_init(|| Mutex::new(HashMap::new()));
    let deadline = std::time::Instant::now() + writer_lease_deadline();
    let mut backoff = WRITER_LEASE_INITIAL_BACKOFF;
    let mut same_process_polls = 0;
    loop {
        {
            let mut stores = registry.lock().map_err(|_| {
                StorageError::new(
                    StorageErrorCode::Database,
                    "shared transactional store registry is poisoned",
                )
            })?;
            stores.retain(|_, store| store.strong_count() > 0);
            if let Some(store) = stores.get(&database_path).and_then(Weak::upgrade) {
                return Ok(store);
            }
            match TransactionalStore::open_writable(&canonical_dir, integrity_key.clone(), create) {
                Ok(store) => {
                    let store = Arc::new(store);
                    stores.insert(database_path, Arc::downgrade(&store));
                    return Ok(store);
                }
                Err(error) if !error.is_database_contention() => return Err(error),
                Err(_) => {}
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(StorageError::new(
                StorageErrorCode::WriterBusy,
                format!(
                    "transactional database `{}` is held by another process; \
                     writer lease deadline exhausted",
                    database_path.display()
                ),
            ));
        }
        // A same-process close is a short race with our own destructor; poll
        // it briefly. Only another process holding the file earns the backoff.
        if same_process_polls < SAME_PROCESS_CLOSE_POLLS
            && same_process_close_pending(&database_path)
        {
            same_process_polls += 1;
            std::thread::sleep(SAME_PROCESS_CLOSE_POLL);
            continue;
        }
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(WRITER_LEASE_MAX_BACKOFF);
    }
}

thread_local! {
    static WRITER_LEASE_DEADLINE_OVERRIDE: std::cell::Cell<Option<std::time::Duration>> =
        const { std::cell::Cell::new(None) };
}

/// Run `operation` with a shorter wait for the database file on this thread.
///
/// A caller that has another answer ready (a recent cached report) uses this
/// so that it does not wait out the full lease deadline behind the sibling
/// process. The override never lengthens the configured deadline and is
/// removed when `operation` returns or unwinds.
pub fn with_writer_lease_deadline<T>(
    deadline: std::time::Duration,
    operation: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<std::time::Duration>);
    impl Drop for Restore {
        fn drop(&mut self) {
            WRITER_LEASE_DEADLINE_OVERRIDE.with(|cell| cell.set(self.0));
        }
    }
    // A nested call never lengthens the wait an outer caller asked for.
    let _restore = Restore(WRITER_LEASE_DEADLINE_OVERRIDE.with(|cell| {
        let outer = cell.get();
        cell.set(Some(outer.map_or(deadline, |outer| outer.min(deadline))));
        outer
    }));
    operation()
}

fn writer_lease_deadline() -> std::time::Duration {
    let configured = configured_writer_lease_deadline();
    match WRITER_LEASE_DEADLINE_OVERRIDE.with(std::cell::Cell::get) {
        Some(shorter) => shorter.min(configured),
        None => configured,
    }
}

fn configured_writer_lease_deadline() -> std::time::Duration {
    std::env::var("POSTFIAT_STORAGE_WRITER_LEASE_DEADLINE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(std::time::Duration::from_millis)
        .unwrap_or(WRITER_LEASE_DEADLINE)
}

/// Drop process-global transactional handles that have no live caller.
///
/// The registry holds only weak references, so this is now a sweep of dead
/// entries. It is retained because command-line binaries call it before
/// returning to the operating system and because it documents the invariant:
/// no process may keep the database open while idle.
pub fn release_inactive_shared_transactional_stores() -> StorageResult<()> {
    let Some(registry) = SHARED_STORES.get() else {
        return Ok(());
    };
    let mut stores = registry.lock().map_err(|_| {
        StorageError::new(
            StorageErrorCode::Database,
            "shared transactional store registry is poisoned",
        )
    })?;
    stores.retain(|_, store| store.strong_count() > 0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "postfiat-store-lease-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
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

    /// A node directory whose authenticated pointer names a verified
    /// generation in a separate directory, as on the deployed validators.
    fn published_generation(node_dir: &TestDir, generation_dir: &TestDir) {
        let node_store = NodeStore::new(&node_dir.0);
        let target = node_store
            .open_transactional_store_at(&generation_dir.0)
            .expect("open generation");
        let tip = ChainTipState {
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
        };
        let commitment = OrderedHistoryCommitment::genesis(
            &tip.chain_id,
            &tip.genesis_hash,
            tip.protocol_version,
        )
        .expect("genesis commitment");
        target
            .initialize(&tip, &commitment, Default::default())
            .expect("initialize generation");
        let root = "11".repeat(48);
        target
            .verify_and_bind_migration(&root)
            .expect("bind generation");
        drop(target);
        node_store
            .publish_transactional_generation(&generation_dir.0, &root)
            .expect("publish generation");
    }

    /// STO-07: while an operation holds the store, lookups reuse the handle
    /// and its validated pointer binding instead of re-reading the pointer
    /// file and reopening the database; the next open re-validates.
    #[test]
    fn held_store_reuses_its_validated_pointer_binding_until_it_closes() {
        let node_dir = TestDir::new("binding-node");
        let generation_dir = TestDir::new("binding-generation");
        published_generation(&node_dir, &generation_dir);
        let store = NodeStore::new(&node_dir.0);
        let before = transactional_open_counts_for(&generation_dir.0);

        let held = store.transactional_store().expect("open for one operation");
        let pointer = node_dir.0.join(TRANSACTIONAL_GENERATION_POINTER_FILE);
        fs::write(&pointer, b"{}").expect("invalidate the pointer file");
        for _ in 0..100 {
            let lookup = store
                .transactional_store()
                .expect("lookup inside the operation");
            assert!(Arc::ptr_eq(&held, &lookup));
            assert!(store
                .transactional_storage_configured()
                .expect("configured"));
        }
        let during = transactional_open_counts_for(&generation_dir.0).saturating_delta(before);
        assert_eq!(during.database_opens, 1, "{during:?}");
        assert_eq!(during.database_closes, 0, "{during:?}");

        drop(held);
        let after = transactional_open_counts_for(&generation_dir.0).saturating_delta(before);
        assert_eq!(after.database_closes, 1, "{after:?}");
        assert!(
            store.transactional_store().is_err(),
            "the next open must re-validate the pointer and fail closed"
        );
    }

    /// STO-07: an rpc-serve-style reader (a second handle on another thread,
    /// standing in for the sibling process) opens the database between two
    /// operations, and is excluded only while one is in flight.
    #[test]
    fn rpc_reader_opens_the_database_between_two_operations() {
        let node_dir = TestDir::new("reader-node");
        let generation_dir = TestDir::new("reader-generation");
        published_generation(&node_dir, &generation_dir);
        let store = NodeStore::new(&node_dir.0);
        let reader = |expect_open: bool| {
            let node_dir = node_dir.0.clone();
            let generation_dir = generation_dir.0.clone();
            std::thread::spawn(move || {
                let result = NodeStore::new(&node_dir)
                    .open_transactional_store_read_only_at(&generation_dir)
                    .and_then(|reader| reader.meta());
                assert_eq!(result.is_ok(), expect_open, "{:?}", result.err());
            })
            .join()
            .expect("reader thread");
        };

        for _operation in 0..2 {
            let lease = store
                .transactional_store_lease()
                .expect("lease")
                .expect("generation exists");
            store.transactional_storage_active().expect("lookup");
            reader(false);
            drop(lease);
            reader(true);
        }
    }
}
