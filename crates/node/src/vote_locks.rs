use super::{
    atomic_write, consensus_v2_active_at, genesis_hash, hash_hex, read_json_file, BlockVoteTarget,
    Genesis, NodeStore, MAX_TEXT_FIELD_BYTES,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

const BLOCK_PROPOSAL_VOTE_LOCK_DIR: &str = "block_proposal_vote_locks";
const BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1: &str = "postfiat.block_proposal_vote_lock.v1";
const BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA: &str = "postfiat.block_proposal_vote_lock.v2";
const BLOCK_PROPOSAL_VOTE_LOCK_INDEX_SCHEMA: &str =
    "postfiat.block_proposal_vote_lock_index_state.v1";
const BLOCK_PROPOSAL_VOTE_LOCK_INDEX_MARKER: &str = ".vote_lock_index_state.v1";
const BLOCK_PROPOSAL_VOTE_LOCK_MUTATION_LOCK: &str = ".vote_lock_index_mutation.v1";
const MIGRATED_PATH_DOMAINS: [&str; 2] = ["v2", "v3"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BlockProposalVoteLock {
    schema: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    block_height: u64,
    view: u64,
    validator: String,
    proposal_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockProposalVoteLockIndexState {
    schema: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    migrated_path_domains: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct VoteLockWorkReport {
    pub(super) files_examined: u64,
    pub(super) bytes_decoded: u64,
    pub(super) migration_performed: bool,
}

impl VoteLockWorkReport {
    fn record_examined_path(&mut self, bytes: u64) -> io::Result<()> {
        self.files_examined = self.files_examined.checked_add(1).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "block proposal vote-lock examined-file counter overflow",
            )
        })?;
        self.bytes_decoded = self.bytes_decoded.checked_add(bytes).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "block proposal vote-lock decoded-byte counter overflow",
            )
        })?;
        Ok(())
    }
}

#[derive(Debug)]
struct VoteLockMutationGuard {
    _file: File,
}

#[derive(Debug, Clone)]
struct MigrationRecord {
    source_path: PathBuf,
    lock: BlockProposalVoteLock,
}

pub(super) fn reserve_block_proposal_vote_lock(
    store: &NodeStore,
    genesis: &Genesis,
    target: &BlockVoteTarget,
    validator: &str,
) -> io::Result<VoteLockWorkReport> {
    let Some(proposal_hash) = target.proposal_hash.as_deref() else {
        return Ok(VoteLockWorkReport::default());
    };

    let lock_dir = store.data_dir().join(BLOCK_PROPOSAL_VOTE_LOCK_DIR);
    let _mutation_guard = acquire_vote_lock_mutation_guard(&lock_dir)?;
    let mut work = VoteLockWorkReport::default();
    ensure_block_proposal_vote_lock_index(store, genesis, &mut work)?;

    let lock = BlockProposalVoteLock {
        schema: BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        block_height: target.evidence.height,
        view: target.evidence.view,
        validator: validator.to_string(),
        proposal_hash: proposal_hash.to_string(),
    };
    let lock_path = block_proposal_vote_lock_path(
        store,
        genesis,
        target.evidence.height,
        target.evidence.view,
        validator,
    );

    if let Some(existing) =
        read_counted_json_file(&lock_path, "block proposal vote lock", &mut work)?
    {
        validate_block_proposal_vote_lock(&existing, genesis, target, validator, proposal_hash)?;
    }

    if write_new_block_proposal_vote_lock(&lock_path, &lock)? {
        return Ok(work);
    }

    let existing =
        read_required_counted_json_file(&lock_path, "block proposal vote lock", &mut work)?;
    validate_block_proposal_vote_lock(&existing, genesis, target, validator, proposal_hash)?;
    Ok(work)
}

fn acquire_vote_lock_mutation_guard(lock_dir: &Path) -> io::Result<VoteLockMutationGuard> {
    fs::create_dir_all(lock_dir).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "failed to create block proposal vote-lock directory `{}`: {error}",
                lock_dir.display()
            ),
        )
    })?;
    let path = lock_dir.join(BLOCK_PROPOSAL_VOTE_LOCK_MUTATION_LOCK);

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
        // SAFETY: the descriptor remains valid inside VoteLockMutationGuard for
        // the complete index-check, migration, lookup, and reservation section.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if result == 0 {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "failed to lock block proposal vote-lock mutation file `{}`: {error}",
                    path.display()
                ),
            ));
        }
    }

    #[cfg(not(unix))]
    {
        let _ = file;
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "cross-process block proposal vote-lock mutation locking requires Unix flock",
        ));
    }

    Ok(VoteLockMutationGuard { _file: file })
}

fn ensure_block_proposal_vote_lock_index(
    store: &NodeStore,
    genesis: &Genesis,
    work: &mut VoteLockWorkReport,
) -> io::Result<()> {
    let marker_path = vote_lock_index_marker_path(store);
    if let Some(marker) =
        read_counted_json_file(&marker_path, "block proposal vote-lock index marker", work)?
    {
        return validate_vote_lock_index_marker(&marker, genesis);
    }

    migrate_block_proposal_vote_locks(store, genesis, work)
}

fn migrate_block_proposal_vote_locks(
    store: &NodeStore,
    genesis: &Genesis,
    work: &mut VoteLockWorkReport,
) -> io::Result<()> {
    let lock_dir = store.data_dir().join(BLOCK_PROPOSAL_VOTE_LOCK_DIR);
    let entries = fs::read_dir(&lock_dir).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "failed to read block proposal vote-lock directory `{}`: {error}",
                lock_dir.display()
            ),
        )
    })?;

    let mut json_paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to enumerate block proposal vote-lock directory `{}`: {error}",
                    lock_dir.display()
                ),
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to inspect block proposal vote-lock entry `{}`: {error}",
                    entry.path().display()
                ),
            )
        })?;
        let path = entry.path();
        if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("json")
        {
            json_paths.push(path);
        }
    }
    json_paths.sort();

    // A brand-new directory has no legacy state to migrate. Defer the marker
    // until the first durable lock exists so an immediately imported legacy
    // lock still receives the one-time preflight on the next reservation.
    if json_paths.is_empty() {
        return Ok(());
    }

    work.migration_performed = true;

    // Phase 1 is read-only. Detect every malformed, mismatched, or conflicting
    // record before creating a link or removing any path.
    let mut groups = BTreeMap::<PathBuf, Vec<MigrationRecord>>::new();
    for source_path in json_paths {
        let lock = read_required_counted_json_file(&source_path, "block proposal vote lock", work)?;
        validate_migrating_block_proposal_vote_lock(&lock, genesis, &source_path)?;
        let derived_path = block_proposal_vote_lock_path(
            store,
            genesis,
            lock.block_height,
            lock.view,
            &lock.validator,
        );
        groups
            .entry(derived_path.clone())
            .or_default()
            .push(MigrationRecord { source_path, lock });
    }

    for records in groups.values_mut() {
        records.sort_by(|left, right| left.source_path.cmp(&right.source_path));
        let proposal_hash = &records[0].lock.proposal_hash;
        if records
            .iter()
            .any(|record| record.lock.proposal_hash.as_str() != proposal_hash.as_str())
        {
            return Err(conflicting_migration_locks_error(records));
        }
    }

    // Phase 2a creates every canonical derived link before deleting anything.
    // If an old binary races us, it can only create another derived-path lock;
    // validate that collision before proceeding.
    for (derived_path, records) in &groups {
        if records
            .iter()
            .any(|record| record.source_path == *derived_path)
        {
            continue;
        }

        let source_path = &records[0].source_path;
        match fs::hard_link(source_path, derived_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let existing = read_required_counted_json_file(
                    derived_path,
                    "block proposal vote lock",
                    work,
                )?;
                validate_migrating_block_proposal_vote_lock(&existing, genesis, derived_path)?;
                let existing_derived = block_proposal_vote_lock_path(
                    store,
                    genesis,
                    existing.block_height,
                    existing.view,
                    &existing.validator,
                );
                if existing_derived != *derived_path
                    || existing.proposal_hash != records[0].lock.proposal_hash
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "conflicting block proposal vote locks recorded for one slot at `{}`; operator repair is required",
                            derived_path.display()
                        ),
                    ));
                }
            }
            Err(error) => {
                return Err(io::Error::new(
                    error.kind(),
                    format!(
                        "failed to create derived block proposal vote-lock link `{}` from `{}`: {error}",
                        derived_path.display(),
                        source_path.display()
                    ),
                ));
            }
        }
    }
    sync_directory(&lock_dir)?;

    // Re-read every canonical path before any cleanup. This closes the
    // preflight-to-link race with a concurrently running older binary.
    for (derived_path, records) in &groups {
        let derived =
            read_required_counted_json_file(derived_path, "block proposal vote lock", work)?;
        validate_migrating_block_proposal_vote_lock(&derived, genesis, derived_path)?;
        let recomputed_path = block_proposal_vote_lock_path(
            store,
            genesis,
            derived.block_height,
            derived.view,
            &derived.validator,
        );
        if recomputed_path != *derived_path
            || derived.proposal_hash != records[0].lock.proposal_hash
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "derived block proposal vote lock `{}` changed during migration; operator repair is required",
                    derived_path.display()
                ),
            ));
        }
    }

    // Phase 2b removes only source records whose exact bytes still decode to
    // the preflight value and whose safe derived copy has already been synced.
    for (derived_path, records) in &groups {
        for record in records {
            if record.source_path == *derived_path {
                continue;
            }
            let current: BlockProposalVoteLock = read_required_counted_json_file(
                &record.source_path,
                "block proposal vote lock",
                work,
            )?;
            if current != record.lock {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "block proposal vote lock `{}` changed during migration; operator repair is required",
                        record.source_path.display()
                    ),
                ));
            }
            fs::remove_file(&record.source_path).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "failed to remove migrated block proposal vote lock `{}`: {error}",
                        record.source_path.display()
                    ),
                )
            })?;
        }
    }
    sync_directory(&lock_dir)?;

    let marker = expected_vote_lock_index_marker(genesis);
    let json = serde_json::to_string_pretty(&marker).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to serialize block proposal vote-lock index marker: {error}"),
        )
    })?;
    atomic_write(vote_lock_index_marker_path(store), format!("{json}\n"))
}

fn conflicting_migration_locks_error(records: &[MigrationRecord]) -> io::Error {
    let paths = records
        .iter()
        .map(|record| record.source_path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "conflicting block proposal vote locks recorded for one slot across [{paths}]; operator repair is required and no conflicting evidence was removed"
        ),
    )
}

fn validate_migrating_block_proposal_vote_lock(
    lock: &BlockProposalVoteLock,
    genesis: &Genesis,
    path: &Path,
) -> io::Result<()> {
    if lock.schema != BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA
        && lock.schema != BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported block proposal vote lock schema `{}` in `{}`",
                lock.schema,
                path.display()
            ),
        ));
    }
    if lock.chain_id != genesis.chain_id
        || lock.genesis_hash != genesis_hash(genesis)
        || lock.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block proposal vote lock `{}` is bound to a different chain, genesis, or protocol",
                path.display()
            ),
        ));
    }
    if lock.validator.is_empty()
        || lock.validator.len() > MAX_TEXT_FIELD_BYTES
        || lock.proposal_hash.is_empty()
        || lock.proposal_hash.len() > MAX_TEXT_FIELD_BYTES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block proposal vote lock `{}` has an invalid validator or proposal hash",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn expected_vote_lock_index_marker(genesis: &Genesis) -> BlockProposalVoteLockIndexState {
    BlockProposalVoteLockIndexState {
        schema: BLOCK_PROPOSAL_VOTE_LOCK_INDEX_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        migrated_path_domains: MIGRATED_PATH_DOMAINS
            .iter()
            .map(|domain| (*domain).to_string())
            .collect(),
    }
}

fn validate_vote_lock_index_marker(
    marker: &BlockProposalVoteLockIndexState,
    genesis: &Genesis,
) -> io::Result<()> {
    let expected = expected_vote_lock_index_marker(genesis);
    if marker != &expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal vote-lock index marker is bound to a different chain, genesis, protocol, schema, or path-domain set",
        ));
    }
    Ok(())
}

fn vote_lock_index_marker_path(store: &NodeStore) -> PathBuf {
    store
        .data_dir()
        .join(BLOCK_PROPOSAL_VOTE_LOCK_DIR)
        .join(BLOCK_PROPOSAL_VOTE_LOCK_INDEX_MARKER)
}

fn block_proposal_vote_lock_path(
    store: &NodeStore,
    genesis: &Genesis,
    block_height: u64,
    view: u64,
    validator: &str,
) -> PathBuf {
    // Consensus v2 verifies the timeout certificate before reaching vote-lock
    // reservation. Its anti-equivocation boundary is therefore one proposal
    // per (height, view), while the legacy protocol retains one proposal per
    // (height, validator) across every view.
    let (domain, material, file_name) = if consensus_v2_active_at(genesis, block_height) {
        (
            "postfiat.block_proposal_vote_lock_path.v3",
            format!("{block_height}:{view}:{validator}"),
            format!("{block_height}.{view}"),
        )
    } else {
        (
            "postfiat.block_proposal_vote_lock_path.v2",
            format!("{block_height}:{validator}"),
            block_height.to_string(),
        )
    };
    let lock_id = hash_hex(domain, material.as_bytes());
    store
        .data_dir()
        .join(BLOCK_PROPOSAL_VOTE_LOCK_DIR)
        .join(format!("{file_name}.{lock_id}.json"))
}

fn write_new_block_proposal_vote_lock(
    lock_path: &Path,
    lock: &BlockProposalVoteLock,
) -> io::Result<bool> {
    let parent = lock_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "block proposal vote lock path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;
    let json = serde_json::to_string_pretty(lock).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to serialize block proposal vote lock: {error}"),
        )
    })?;
    let temp_id = hash_hex(
        "postfiat.block_proposal_vote_lock_temp.v1",
        format!(
            "{}:{}:{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
                .as_nanos(),
            lock_path.display()
        )
        .as_bytes(),
    );
    let temp_path = parent.join(format!(".{temp_id}.tmp"));
    atomic_write(&temp_path, format!("{json}\n"))?;
    let result = fs::hard_link(&temp_path, lock_path);
    let _ = fs::remove_file(&temp_path);
    match result {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!(
                "failed to reserve block proposal vote lock `{}`: {error}",
                lock_path.display()
            ),
        )),
    }
}

fn validate_block_proposal_vote_lock(
    lock: &BlockProposalVoteLock,
    genesis: &Genesis,
    target: &BlockVoteTarget,
    validator: &str,
    proposal_hash: &str,
) -> io::Result<()> {
    if lock.schema != BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA
        && lock.schema != BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported block proposal vote lock schema `{}`",
                lock.schema
            ),
        ));
    }
    if lock.chain_id != genesis.chain_id
        || lock.genesis_hash != genesis_hash(genesis)
        || lock.protocol_version != genesis.protocol_version
        || lock.block_height != target.evidence.height
        || (consensus_v2_active_at(genesis, target.evidence.height)
            && lock.view != target.evidence.view)
        || lock.validator != validator
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal vote lock target mismatch",
        ));
    }
    if lock.proposal_hash != proposal_hash {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "conflicting block proposal vote already recorded for validator `{validator}` at height {} (recorded view {}, attempted view {})",
                target.evidence.height, lock.view, target.evidence.view
            ),
        ));
    }
    Ok(())
}

fn read_counted_json_file<T: DeserializeOwned>(
    path: &Path,
    label: &str,
    work: &mut VoteLockWorkReport,
) -> io::Result<Option<T>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            work.record_examined_path(0)?;
            return Ok(None);
        }
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!("failed to inspect {label} `{}`: {error}", path.display()),
            ));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` is not a regular file", path.display()),
        ));
    }
    work.record_examined_path(metadata.len())?;
    read_json_file(path, label).map(Some)
}

fn read_required_counted_json_file<T: DeserializeOwned>(
    path: &Path,
    label: &str,
    work: &mut VoteLockWorkReport,
) -> io::Result<T> {
    read_counted_json_file(path, label, work)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("required {label} `{}` does not exist", path.display()),
        )
    })
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OwnedBlockEvidence;
    use std::sync::{Arc, Barrier};

    fn target(height: u64, view: u64, proposal_hash: &str) -> BlockVoteTarget {
        BlockVoteTarget {
            evidence: OwnedBlockEvidence {
                height,
                view,
                parent_hash: "parent".to_string(),
                proposer: "validator-0".to_string(),
                batch_kind: "mempool".to_string(),
                batch_id: format!("batch-{height}-{view}"),
                state_root: "state".to_string(),
                bridge_exit_root: None,
                pftl_uniswap_receipt_root: None,
                receipt_ids: Vec::new(),
                fastpay_pre_state_effects: Vec::new(),
            },
            validators: vec!["validator-0".to_string()],
            block_hash: None,
            proposal_hash: Some(proposal_hash.to_string()),
        }
    }

    fn test_store(label: &str) -> (PathBuf, NodeStore) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-block-proposal-lock-{label}-{}-{unique}",
            std::process::id()
        ));
        (data_dir.clone(), NodeStore::new(data_dir))
    }

    fn legacy_genesis() -> Genesis {
        Genesis::try_new_with_validator_count("lock-test".to_string(), 4).expect("legacy genesis")
    }

    fn activated_genesis() -> Genesis {
        let mut genesis = legacy_genesis();
        genesis.consensus_v2_activation_height = Some(1);
        genesis.validate().expect("activated genesis");
        genesis
    }

    fn lock_for(
        genesis: &Genesis,
        height: u64,
        view: u64,
        proposal_hash: &str,
        schema: &str,
    ) -> BlockProposalVoteLock {
        BlockProposalVoteLock {
            schema: schema.to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            block_height: height,
            view,
            validator: "validator-0".to_string(),
            proposal_hash: proposal_hash.to_string(),
        }
    }

    fn write_lock(path: &Path, lock: &BlockProposalVoteLock) {
        let json = serde_json::to_string_pretty(lock).expect("serialize vote lock");
        atomic_write(path, format!("{json}\n")).expect("write vote lock");
    }

    fn write_marker(store: &NodeStore, genesis: &Genesis) {
        let marker = expected_vote_lock_index_marker(genesis);
        let json = serde_json::to_string_pretty(&marker).expect("serialize marker");
        atomic_write(vote_lock_index_marker_path(store), format!("{json}\n"))
            .expect("write marker");
    }

    fn json_lock_count(data_dir: &Path) -> usize {
        fs::read_dir(data_dir.join(BLOCK_PROPOSAL_VOTE_LOCK_DIR))
            .expect("read lock directory")
            .map(|entry| entry.expect("lock entry").path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .count()
    }

    #[test]
    fn activated_consensus_v2_lock_rejects_same_view_equivocation_but_allows_timeout_view() {
        let (data_dir, store) = test_store("activated");
        let activated = activated_genesis();

        reserve_block_proposal_vote_lock(
            &store,
            &activated,
            &target(14, 0, "proposal-view-0"),
            "validator-0",
        )
        .expect("first view-0 vote lock");
        let same_view = reserve_block_proposal_vote_lock(
            &store,
            &activated,
            &target(14, 0, "equivocating-view-0-proposal"),
            "validator-0",
        )
        .expect_err("same-view equivocation must remain locked out");
        assert_eq!(same_view.kind(), io::ErrorKind::AlreadyExists);
        reserve_block_proposal_vote_lock(
            &store,
            &activated,
            &target(14, 2, "proposal-view-2"),
            "validator-0",
        )
        .expect("verified timeout view must receive an independent durable lock");

        assert_eq!(
            json_lock_count(&data_dir),
            2,
            "one durable lock is required per signed view"
        );
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn legacy_lock_remains_height_wide_across_views() {
        let (data_dir, store) = test_store("legacy");
        let legacy = legacy_genesis();
        reserve_block_proposal_vote_lock(
            &store,
            &legacy,
            &target(14, 0, "legacy-proposal"),
            "validator-0",
        )
        .expect("legacy lock");
        let cross_view = reserve_block_proposal_vote_lock(
            &store,
            &legacy,
            &target(14, 2, "different-legacy-proposal"),
            "validator-0",
        )
        .expect_err("legacy behavior must remain height-wide");
        assert_eq!(cross_view.kind(), io::ErrorKind::AlreadyExists);
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn same_slot_same_proposal_reservation_is_idempotent() {
        let (data_dir, store) = test_store("idempotent");
        let genesis = activated_genesis();
        let first = target(14, 0, "same-proposal");
        reserve_block_proposal_vote_lock(&store, &genesis, &first, "validator-0")
            .expect("first reservation");
        let second = target(14, 0, "same-proposal");
        reserve_block_proposal_vote_lock(&store, &genesis, &second, "validator-0")
            .expect("idempotent reservation");
        assert_eq!(json_lock_count(&data_dir), 1);
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn conflicting_lock_still_binds_after_restart() {
        let (data_dir, store) = test_store("restart");
        let genesis = activated_genesis();
        reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 0, "first-proposal"),
            "validator-0",
        )
        .expect("first reservation");
        drop(store);

        let restarted = NodeStore::new(&data_dir);
        let error = reserve_block_proposal_vote_lock(
            &restarted,
            &genesis,
            &target(14, 0, "conflict"),
            "validator-0",
        )
        .expect_err("conflict after restart");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn legacy_arbitrary_filename_lock_is_migrated_and_binds() {
        let (data_dir, store) = test_store("legacy-arbitrary");
        let genesis = legacy_genesis();
        let lock_dir = data_dir.join(BLOCK_PROPOSAL_VOTE_LOCK_DIR);
        fs::create_dir_all(&lock_dir).expect("create lock directory");
        let arbitrary = lock_dir.join("14.0.legacy-v1.json");
        write_lock(
            &arbitrary,
            &lock_for(
                &genesis,
                14,
                0,
                "recorded-proposal",
                BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1,
            ),
        );

        let error = reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 2, "conflicting-proposal"),
            "validator-0",
        )
        .expect_err("migrated legacy lock must bind");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(!arbitrary.exists());
        assert!(block_proposal_vote_lock_path(&store, &genesis, 14, 0, "validator-0").exists());
        assert!(vote_lock_index_marker_path(&store).exists());
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn migration_conflicting_locks_for_same_slot_fail_closed() {
        let (data_dir, store) = test_store("migration-conflict");
        let genesis = legacy_genesis();
        let lock_dir = data_dir.join(BLOCK_PROPOSAL_VOTE_LOCK_DIR);
        fs::create_dir_all(&lock_dir).expect("create lock directory");
        let first_path = lock_dir.join("first.json");
        let second_path = lock_dir.join("second.json");
        write_lock(
            &first_path,
            &lock_for(
                &genesis,
                14,
                0,
                "first-proposal",
                BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1,
            ),
        );
        write_lock(
            &second_path,
            &lock_for(
                &genesis,
                14,
                2,
                "second-proposal",
                BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA,
            ),
        );

        let error = reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 0, "attempt"),
            "validator-0",
        )
        .expect_err("conflicting migration must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("operator repair is required"));
        assert!(first_path.exists(), "first conflict evidence must remain");
        assert!(second_path.exists(), "second conflict evidence must remain");
        assert!(!vote_lock_index_marker_path(&store).exists());
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn interrupted_migration_resumes() {
        let (data_dir, store) = test_store("interrupted");
        let genesis = activated_genesis();
        let lock_dir = data_dir.join(BLOCK_PROPOSAL_VOTE_LOCK_DIR);
        fs::create_dir_all(&lock_dir).expect("create lock directory");
        let misplaced = lock_dir.join("interrupted-copy.json");
        let lock = lock_for(
            &genesis,
            14,
            0,
            "same-proposal",
            BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA,
        );
        write_lock(&misplaced, &lock);
        let derived = block_proposal_vote_lock_path(&store, &genesis, 14, 0, "validator-0");
        fs::hard_link(&misplaced, &derived).expect("simulate linked migration copy");

        let work = reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 0, "same-proposal"),
            "validator-0",
        )
        .expect("resume migration");
        assert!(work.migration_performed);
        assert!(!misplaced.exists());
        assert!(derived.exists());
        assert!(vote_lock_index_marker_path(&store).exists());
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn truncated_lock_file_fails_closed() {
        let (data_dir, store) = test_store("truncated");
        let genesis = activated_genesis();
        let lock_path = block_proposal_vote_lock_path(&store, &genesis, 14, 0, "validator-0");
        atomic_write(&lock_path, "{").expect("write truncated lock");

        let error = reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 0, "proposal"),
            "validator-0",
        )
        .expect_err("truncated lock must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read_to_string(&lock_path).expect("read truncated lock"),
            "{"
        );
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn marker_bound_to_other_chain_fails_closed() {
        let (data_dir, store) = test_store("wrong-marker");
        let genesis = activated_genesis();
        let mut marker = expected_vote_lock_index_marker(&genesis);
        marker.genesis_hash = "other-genesis".to_string();
        let json = serde_json::to_string_pretty(&marker).expect("serialize marker");
        atomic_write(vote_lock_index_marker_path(&store), format!("{json}\n"))
            .expect("write marker");

        let error = reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 0, "proposal"),
            "validator-0",
        )
        .expect_err("wrong marker binding must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn malformed_marker_fails_closed_without_remigration() {
        let (data_dir, store) = test_store("malformed-marker");
        let genesis = activated_genesis();
        atomic_write(vote_lock_index_marker_path(&store), "{").expect("write malformed marker");

        let error = reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 0, "proposal"),
            "validator-0",
        )
        .expect_err("malformed marker must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read_to_string(vote_lock_index_marker_path(&store)).expect("read malformed marker"),
            "{"
        );
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn vote_lock_work_is_bounded_with_large_history() {
        let (data_dir, store) = test_store("bounded");
        let genesis = activated_genesis();
        write_marker(&store, &genesis);
        for height in 1_000..3_000 {
            let path = block_proposal_vote_lock_path(&store, &genesis, height, 0, "validator-0");
            write_lock(
                &path,
                &lock_for(
                    &genesis,
                    height,
                    0,
                    &format!("proposal-{height}"),
                    BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA,
                ),
            );
        }

        let work = reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 0, "new-proposal"),
            "validator-0",
        )
        .expect("bounded reservation");
        assert!(
            work.files_examined <= 3,
            "normal reservation examined {} files",
            work.files_examined
        );
        assert!(!work.migration_performed);
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    #[ignore = "manual release-mode bounded-work spot check"]
    fn release_spot_check_emits_bounded_work_with_5000_lock_history() {
        let (data_dir, _) = test_store("release-spot");
        crate::init(crate::InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-vote-lock-release-spot".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
        })
        .expect("initialize release spot-check chain");

        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("read spot-check genesis");
        write_marker(&store, &genesis);
        for height in 10_000..15_000 {
            let path = block_proposal_vote_lock_path(&store, &genesis, height, 0, "validator-0");
            write_lock(
                &path,
                &lock_for(
                    &genesis,
                    height,
                    0,
                    &format!("historical-proposal-{height}"),
                    BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA,
                ),
            );
        }

        let batch_file = data_dir.join("release-spot.batch.json");
        crate::create_transfer_batch(crate::BatchTransferOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            to: "pfvotelockrelease000000000000000000000".to_string(),
            amount: 1,
            batch_file: batch_file.clone(),
        })
        .expect("create release spot-check batch");
        let proposal_file = data_dir.join("release-spot.block_proposal.json");
        let proposal = crate::propose_batch(crate::BatchProposalOptions {
            data_dir: data_dir.clone(),
            verify_block_log: true,
            batch_kind: Some(crate::BATCH_KIND_TRANSPARENT.to_string()),
            batch_file: batch_file.clone(),
            proposal_file: proposal_file.clone(),
            view: None,
            timeout_certificate_file: None,
            key_file: Some(data_dir.join(crate::VALIDATOR_KEYS_FILE)),
            validator_id: None,
        })
        .expect("create signed release spot-check proposal");

        let report = crate::create_block_vote_with_timings(crate::BlockVoteOptions {
            data_dir: data_dir.clone(),
            verify_block_log: true,
            key_file: data_dir.join(crate::VALIDATOR_KEYS_FILE),
            validator_id: Some("validator-0".to_string()),
            batch_file: Some(batch_file),
            proposal_file: Some(proposal_file),
            timeout_certificate_file: None,
            block_height: Some(proposal.block_height),
            vote_file: data_dir.join("release-spot.block_vote.json"),
        })
        .expect("create release spot-check vote");

        println!(
            "{}",
            serde_json::to_string_pretty(&report.timings).expect("serialize timing report")
        );
        assert!(
            report.timings.vote_lock_files_examined <= 3,
            "normal reservation examined {} paths",
            report.timings.vote_lock_files_examined
        );
        assert!(
            !report.timings.vote_lock_migration_performed,
            "pre-marked history must not trigger migration"
        );
        assert_eq!(
            json_lock_count(&data_dir),
            5_001,
            "5,000 historical locks plus the new vote lock must remain"
        );
        fs::remove_dir_all(data_dir).expect("cleanup release spot-check data");
    }

    #[test]
    fn concurrent_first_reservations_serialize_and_preserve_one_lock() {
        let (data_dir, _store) = test_store("concurrent");
        let genesis = Arc::new(activated_genesis());
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for proposal in ["proposal-a", "proposal-b"] {
            let thread_data_dir = data_dir.clone();
            let thread_genesis = Arc::clone(&genesis);
            let thread_barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let store = NodeStore::new(thread_data_dir);
                thread_barrier.wait();
                reserve_block_proposal_vote_lock(
                    &store,
                    &thread_genesis,
                    &target(14, 0, proposal),
                    "validator-0",
                )
            }));
        }
        barrier.wait();

        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("reservation thread"))
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| {
                    result
                        .as_ref()
                        .is_err_and(|error| error.kind() == io::ErrorKind::AlreadyExists)
                })
                .count(),
            1
        );
        assert_eq!(json_lock_count(&data_dir), 1);
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn equivalent_v1_and_v2_legacy_duplicates_canonicalize() {
        let (data_dir, store) = test_store("schema-equivalence");
        let genesis = legacy_genesis();
        let lock_dir = data_dir.join(BLOCK_PROPOSAL_VOTE_LOCK_DIR);
        fs::create_dir_all(&lock_dir).expect("create lock directory");
        let v1_path = lock_dir.join("a-v1.json");
        let v2_path = lock_dir.join("b-v2.json");
        write_lock(
            &v1_path,
            &lock_for(
                &genesis,
                14,
                0,
                "same-proposal",
                BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1,
            ),
        );
        write_lock(
            &v2_path,
            &lock_for(
                &genesis,
                14,
                2,
                "same-proposal",
                BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA,
            ),
        );

        reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 3, "same-proposal"),
            "validator-0",
        )
        .expect("equivalent schemas migrate");
        assert!(!v1_path.exists());
        assert!(!v2_path.exists());
        assert_eq!(json_lock_count(&data_dir), 1);
        fs::remove_dir_all(data_dir).expect("cleanup");
    }

    #[test]
    fn mixed_binary_marker_is_ignored_and_v1_derived_lock_binds() {
        let (data_dir, store) = test_store("mixed-binary");
        let genesis = legacy_genesis();
        write_marker(&store, &genesis);
        let derived = block_proposal_vote_lock_path(&store, &genesis, 14, 0, "validator-0");
        write_lock(
            &derived,
            &lock_for(
                &genesis,
                14,
                0,
                "old-binary-proposal",
                BLOCK_PROPOSAL_VOTE_LOCK_SCHEMA_V1,
            ),
        );

        assert_ne!(
            vote_lock_index_marker_path(&store)
                .extension()
                .and_then(|extension| extension.to_str()),
            Some("json")
        );
        let error = reserve_block_proposal_vote_lock(
            &store,
            &genesis,
            &target(14, 2, "conflicting-proposal"),
            "validator-0",
        )
        .expect_err("derived v1 lock remains binding");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        fs::remove_dir_all(data_dir).expect("cleanup");
    }
}
