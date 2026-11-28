use super::*;

const FASTPAY_V3_TRANSFER_ORDER_HASH_DOMAIN: &str = "postfiat.fastpay.transfer-order.v3";
const FASTPAY_V3_UNWRAP_ORDER_HASH_DOMAIN: &str = "postfiat.fastpay.unwrap-order.v3";
const FASTPAY_SPECULATIVE_JOURNAL_SCHEMA_V1: &str = "postfiat.fastpay.speculative-effects.v1";
const CHAIN_TIP_GENERATION_SIDECAR_SCHEMA_V1: &str = "postfiat.chain-tip-generation.v1";
const FASTPAY_LEDGER_SNAPSHOT_SIDECAR_SCHEMA_V1: &str = "postfiat.fastpay.ledger-snapshot.v1";
const FASTPAY_SPECULATIVE_WAL_MAGIC_V2: u32 = 0x5046_5732; // "PFW2"
const FASTPAY_SPECULATIVE_WAL_HEADER_LEN: usize = 12;
const FASTPAY_SPECULATIVE_WAL_MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
pub(super) const FASTPAY_HELD_EFFECT_SCHEMA_V1: &str = "postfiat-fastpay-held-effect-v1";

#[derive(Debug, Clone, Default)]
pub struct FastPayRpcMetrics {
    pub redb_lease_wait_ms: Option<f64>,
    pub wal_fdatasync_ms: Option<f64>,
    pub ledger_cache_hit: Option<bool>,
    pub redb_opened: Option<bool>,
    pub order_lru_hit: Option<bool>,
}

thread_local! {
    static FASTPAY_RPC_METRICS: std::cell::RefCell<Option<FastPayRpcMetrics>> =
        const { std::cell::RefCell::new(None) };
}

fn begin_fastpay_rpc_metrics() {
    FASTPAY_RPC_METRICS.with(|slot| {
        *slot.borrow_mut() = Some(FastPayRpcMetrics {
            redb_lease_wait_ms: Some(0.0),
            wal_fdatasync_ms: None,
            ledger_cache_hit: Some(false),
            redb_opened: Some(false),
            order_lru_hit: None,
        });
    });
}

fn update_fastpay_rpc_metrics(f: impl FnOnce(&mut FastPayRpcMetrics)) {
    FASTPAY_RPC_METRICS.with(|slot| {
        if let Some(metrics) = slot.borrow_mut().as_mut() {
            f(metrics);
        }
    });
}

pub fn take_fastpay_rpc_metrics() -> Option<FastPayRpcMetrics> {
    FASTPAY_RPC_METRICS.with(|slot| slot.borrow_mut().take())
}

fn crc32c_checksum(bytes: &[u8]) -> u32 {
    const POLY: u32 = 0x82F6_3B78;
    let mut crc = 0xFFFF_FFFF_u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (POLY & mask);
        }
    }
    !crc
}

fn fastpay_wal_shadow_verify_enabled() -> bool {
    std::env::var("POSTFIAT_FASTPAY_WAL_SHADOW_VERIFY")
        .map(|val| matches!(val.trim(), "1" | "true" | "TRUE" | "yes"))
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChainTipGenerationSidecarV1 {
    schema: String,
    generation: String,
    finalized_height: u64,
    finalized_block_hash: String,
    finalized_state_root: String,
    ordered_history_accumulator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scheduled_activation_height: Option<u64>,
    db_path: PathBuf,
    db_len: u64,
    #[cfg(unix)]
    db_ino: u64,
}

pub(super) fn write_chain_tip_generation_sidecar(
    data_dir: &std::path::Path,
    meta: &postfiat_storage::TransactionalStoreMetaV1,
    db_path: &std::path::Path,
) -> io::Result<()> {
    let db_identity = fastpay_db_identity(db_path)?;
    let db_len = std::fs::metadata(db_path)?.len();
    let sidecar = ChainTipGenerationSidecarV1 {
        schema: CHAIN_TIP_GENERATION_SIDECAR_SCHEMA_V1.to_string(),
        generation: meta.generation.clone(),
        finalized_height: meta.finalized_height,
        finalized_block_hash: meta.finalized_block_hash.clone(),
        finalized_state_root: meta.finalized_state_root.clone(),
        ordered_history_accumulator: meta.ordered_history_accumulator.clone(),
        scheduled_activation_height: meta.scheduled_activation_height,
        db_path: db_identity.path,
        db_len,
        #[cfg(unix)]
        db_ino: db_identity.ino,
    };
    let bytes = serde_json::to_vec(&sidecar).map_err(invalid_data)?;
    atomic_write(data_dir.join(CHAIN_TIP_GENERATION_SIDECAR_FILE), bytes)
}

fn read_chain_tip_generation_sidecar(
    data_dir: &std::path::Path,
) -> Option<ChainTipGenerationSidecarV1> {
    let path = data_dir.join(CHAIN_TIP_GENERATION_SIDECAR_FILE);
    let raw = std::fs::read(&path).ok()?;
    if raw.len() > 4096 {
        return None;
    }
    let sidecar: ChainTipGenerationSidecarV1 = serde_json::from_slice(&raw).ok()?;
    if sidecar.schema != CHAIN_TIP_GENERATION_SIDECAR_SCHEMA_V1 {
        return None;
    }
    Some(sidecar)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct FastPayLedgerSnapshotSidecarV1 {
    schema: String,
    generation: String,
    finalized_height: u64,
    finalized_block_hash: String,
    finalized_state_root: String,
    ordered_history_accumulator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scheduled_activation_height: Option<u64>,
    ledger: LedgerState,
    genesis: Genesis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fastpay_fee_v1_activation_height: Option<u64>,
    base_pending_unanchored_count: usize,
}

pub(super) fn write_fastpay_ledger_snapshot_sidecar(
    data_dir: &std::path::Path,
    meta: &postfiat_storage::TransactionalStoreMetaV1,
    ledger: &LedgerState,
    genesis: &Genesis,
    fastpay_fee_v1_activation_height: Option<u64>,
    base_pending_unanchored_count: usize,
) -> io::Result<()> {
    let snapshot = FastPayLedgerSnapshotSidecarV1 {
        schema: FASTPAY_LEDGER_SNAPSHOT_SIDECAR_SCHEMA_V1.to_string(),
        generation: meta.generation.clone(),
        finalized_height: meta.finalized_height,
        finalized_block_hash: meta.finalized_block_hash.clone(),
        finalized_state_root: meta.finalized_state_root.clone(),
        ordered_history_accumulator: meta.ordered_history_accumulator.clone(),
        scheduled_activation_height: meta.scheduled_activation_height,
        ledger: ledger.clone(),
        genesis: genesis.clone(),
        fastpay_fee_v1_activation_height,
        base_pending_unanchored_count,
    };
    let bytes = serde_json::to_vec(&snapshot).map_err(invalid_data)?;
    static SNAPSHOT_TMP_COUNTER: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(1);
    let seq = SNAPSHOT_TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_path = data_dir.join(format!(
        "{FASTPAY_LEDGER_SNAPSHOT_SIDECAR_FILE}.tmp.{}.{seq}",
        std::process::id()
    ));
    let final_path = data_dir.join(FASTPAY_LEDGER_SNAPSHOT_SIDECAR_FILE);
    std::fs::write(&tmp_path, &bytes)?;
    if let Err(err) = std::fs::rename(&tmp_path, &final_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(err);
    }
    Ok(())
}

fn read_fastpay_ledger_snapshot_sidecar(
    data_dir: &std::path::Path,
    expected_sidecar: &ChainTipGenerationSidecarV1,
) -> Option<FastPayLedgerSnapshotSidecarV1> {
    let path = data_dir.join(FASTPAY_LEDGER_SNAPSHOT_SIDECAR_FILE);
    let raw = std::fs::read(&path).ok()?;
    let snapshot: FastPayLedgerSnapshotSidecarV1 = serde_json::from_slice(&raw).ok()?;
    if snapshot.schema != FASTPAY_LEDGER_SNAPSHOT_SIDECAR_SCHEMA_V1
        || snapshot.generation != expected_sidecar.generation
        || snapshot.finalized_height != expected_sidecar.finalized_height
        || snapshot.finalized_block_hash != expected_sidecar.finalized_block_hash
        || snapshot.finalized_state_root != expected_sidecar.finalized_state_root
        || snapshot.ordered_history_accumulator != expected_sidecar.ordered_history_accumulator
        || snapshot.scheduled_activation_height != expected_sidecar.scheduled_activation_height
    {
        return None;
    }
    Some(snapshot)
}

fn hydrate_fastpay_state_from_snapshot_sidecar(
    data_dir: &std::path::Path,
    sidecar: &ChainTipGenerationSidecarV1,
    key: &FastPayLedgerCacheKey,
    take_ownership: bool,
) -> io::Result<Option<LoadedFastPayState>> {
    let Some(mut snapshot) = read_fastpay_ledger_snapshot_sidecar(data_dir, sidecar) else {
        return Ok(None);
    };
    let journal = read_fastpay_speculative_journal(data_dir)?;
    let mut pending = Vec::new();
    let mut legacy_pending = 0usize;
    for record in &journal.effects {
        if snapshot
            .ledger
            .fastpay_version_fences
            .iter()
            .any(|f| f.lock_id == record.fence.lock_id)
        {
            continue;
        }
        let (height, sequence) = match &record.retained_tip {
            Some(base) => {
                if base.height == sidecar.finalized_height
                    && (base.block_hash != sidecar.finalized_block_hash
                        || base.state_root != sidecar.finalized_state_root)
                {
                    return Err(fastpay_invalid_data(
                        "FastPay journal is bound to a different finalized tip",
                    ));
                }
                (base.height, base.application_sequence)
            }
            None => (record.fence.decided_at_height, 0),
        };
        if height > sidecar.finalized_height {
            return Err(fastpay_invalid_data(
                "FastPay journal is ahead of the finalized tip",
            ));
        }
        if height < sidecar.finalized_height {
            continue;
        }
        if record.retained_tip.is_none() {
            legacy_pending += 1;
        }
        pending.push((sequence, record.fence.clone()));
    }
    if pending.len() > postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK || legacy_pending > 1
    {
        return Err(fastpay_invalid_data(
            "FastPay pending journal is oversized or has ambiguous legacy application order",
        ));
    }
    pending.sort_by_key(|(sequence, _)| *sequence);
    if pending.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(fastpay_invalid_data(
            "FastPay journal has duplicate application sequence",
        ));
    }
    let effects: Vec<_> = pending.into_iter().map(|(_, fence)| fence).collect();
    let replayed_count = effects.len();
    if !effects.is_empty() {
        let empty_shielded = ShieldedState::empty();
        replay_confirmed_fastpay_fences_dependency_ordered(
            &mut snapshot.ledger,
            &empty_shielded,
            &effects,
            snapshot.fastpay_fee_v1_activation_height,
        )?;
    }
    // Verify the finalized tip sidecar did not advance concurrently while reading WAL.
    if read_chain_tip_generation_sidecar(data_dir).as_ref() != Some(sidecar) {
        return Ok(None);
    }
    let pending_unanchored_count = snapshot.base_pending_unanchored_count + replayed_count;
    let fee_v1_active = snapshot
        .fastpay_fee_v1_activation_height
        .is_some_and(|activation| activation > 0 && key.height >= activation);
    let ledger = std::sync::Arc::new(snapshot.ledger);
    if !take_ownership {
        store_cached_fastpay_state(CachedFastPayState {
            key: key.clone(),
            ledger: std::sync::Arc::clone(&ledger),
            genesis: snapshot.genesis.clone(),
            pending_unanchored_count,
            fee_v1_active,
        });
    }
    update_fastpay_rpc_metrics(|m| {
        m.ledger_cache_hit = Some(false);
        m.redb_opened = Some(false);
        m.redb_lease_wait_ms = Some(0.0);
    });
    Ok(Some(LoadedFastPayState {
        height: key.height,
        cache_key: Some(key.clone()),
        ledger,
        genesis: snapshot.genesis,
        pending_unanchored_count,
        fee_v1_active,
    }))
}

/// The finalized tip (height, block hash, state root) last prewarmed, by data directory.
type FastPayPrewarmedTips = std::sync::Mutex<BTreeMap<std::path::PathBuf, (u64, String, String)>>;

fn fastpay_prewarmed_tip_tracker() -> &'static FastPayPrewarmedTips {
    static TRACKER: std::sync::OnceLock<FastPayPrewarmedTips> = std::sync::OnceLock::new();
    TRACKER.get_or_init(|| std::sync::Mutex::new(BTreeMap::new()))
}

fn cache_has_tip(data_dir: &std::path::Path, tip: &(u64, String, String)) -> bool {
    let dir = canonical_fastpay_cache_dir(data_dir);
    let cache = fastpay_ledger_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.iter().any(|entry| {
        entry.key.data_dir == dir
            && entry.key.height == tip.0
            && entry.key.block_hash == tip.1
            && entry.key.state_root == tip.2
    })
}

/// Lock-free background pre-warmer called by `rpc-serve` once per newly committed
/// finalized tip so post-block `owned_sign_v3` RPCs hit an already-warm in-process
/// cache with zero `redb` opens. Parses the snapshot sidecar off-lock, then
/// acquires the ordered-commit lock briefly to read the WAL and install the cache
/// entry without racing concurrent `owned_apply_v3` writes or evicting newer state.
pub fn prewarm_fastpay_ledger_cache_from_sidecar(data_dir: &std::path::Path) -> bool {
    let Some(sidecar) = read_chain_tip_generation_sidecar(data_dir) else {
        return false;
    };
    let active = sidecar
        .scheduled_activation_height
        .is_some_and(|activation| sidecar.finalized_height >= activation);
    if !active {
        return false;
    }
    let Ok(db_identity) = fastpay_db_identity(&sidecar.db_path) else {
        return false;
    };
    let db_matches = db_identity.path == sidecar.db_path && {
        #[cfg(unix)]
        {
            db_identity.ino == sidecar.db_ino
        }
        #[cfg(not(unix))]
        {
            true
        }
    };
    if !db_matches {
        return false;
    }

    let dir = canonical_fastpay_cache_dir(data_dir);
    let tip_sig = (
        sidecar.finalized_height,
        sidecar.finalized_block_hash.clone(),
        sidecar.finalized_state_root.clone(),
    );
    {
        let tracker = fastpay_prewarmed_tip_tracker()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if tracker.get(&dir) == Some(&tip_sig) {
            return false;
        }
    }
    if cache_has_tip(data_dir, &tip_sig) {
        let mut tracker = fastpay_prewarmed_tip_tracker()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        tracker.insert(dir, tip_sig);
        return false;
    }

    // Parse snapshot sidecar off-lock so we never stall live FastPay RPCs.
    let Some(mut snapshot) = read_fastpay_ledger_snapshot_sidecar(data_dir, &sidecar) else {
        return false;
    };

    // Synchronize WAL reads and cache installation with `owned_apply_v3` / block commits.
    let store = NodeStore::new(data_dir);
    let Ok(_mutation_lock) = store.lock_ordered_commit() else {
        return false;
    };
    if read_chain_tip_generation_sidecar(data_dir).as_ref() != Some(&sidecar) {
        return false;
    }
    if cache_has_tip(data_dir, &tip_sig) {
        let mut tracker = fastpay_prewarmed_tip_tracker()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        tracker.insert(dir, tip_sig);
        return false;
    }
    let Ok(journal_bytes_hash) = fastpay_speculative_journal_hash(data_dir) else {
        return false;
    };
    let Ok(journal) = read_fastpay_speculative_journal(data_dir) else {
        return false;
    };
    let mut pending = Vec::new();
    let mut legacy_pending = 0usize;
    for record in &journal.effects {
        if snapshot
            .ledger
            .fastpay_version_fences
            .iter()
            .any(|f| f.lock_id == record.fence.lock_id)
        {
            continue;
        }
        let (height, sequence) = match &record.retained_tip {
            Some(base) => {
                if base.height == sidecar.finalized_height
                    && (base.block_hash != sidecar.finalized_block_hash
                        || base.state_root != sidecar.finalized_state_root)
                {
                    return false;
                }
                (base.height, base.application_sequence)
            }
            None => (record.fence.decided_at_height, 0),
        };
        if height > sidecar.finalized_height {
            return false;
        }
        if height < sidecar.finalized_height {
            continue;
        }
        if record.retained_tip.is_none() {
            legacy_pending += 1;
        }
        pending.push((sequence, record.fence.clone()));
    }
    if pending.len() > postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK || legacy_pending > 1
    {
        return false;
    }
    pending.sort_by_key(|(sequence, _)| *sequence);
    if pending.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return false;
    }
    let effects: Vec<_> = pending.into_iter().map(|(_, fence)| fence).collect();
    let replayed_count = effects.len();
    if !effects.is_empty() {
        let empty_shielded = ShieldedState::empty();
        if replay_confirmed_fastpay_fences_dependency_ordered(
            &mut snapshot.ledger,
            &empty_shielded,
            &effects,
            snapshot.fastpay_fee_v1_activation_height,
        )
        .is_err()
        {
            return false;
        }
    }
    let key = FastPayLedgerCacheKey {
        data_dir: dir.clone(),
        generation: sidecar.generation.clone(),
        height: sidecar.finalized_height,
        block_hash: sidecar.finalized_block_hash.clone(),
        state_root: sidecar.finalized_state_root.clone(),
        ordered_history_accumulator: sidecar.ordered_history_accumulator.clone(),
        db_identity,
        journal_bytes_hash,
    };
    let pending_unanchored_count = snapshot.base_pending_unanchored_count + replayed_count;
    let fee_v1_active = snapshot
        .fastpay_fee_v1_activation_height
        .is_some_and(|activation| activation > 0 && key.height >= activation);
    store_cached_fastpay_state(CachedFastPayState {
        key,
        ledger: std::sync::Arc::new(snapshot.ledger),
        genesis: snapshot.genesis,
        pending_unanchored_count,
        fee_v1_active,
    });
    let mut tracker = fastpay_prewarmed_tip_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tracker.insert(dir, tip_sig);
    true
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FastPayPriorObjectV1 {
    index: u64,
    object: postfiat_types::OwnedObject,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FastPaySpeculativeEffectV1 {
    fence: postfiat_types::FastPayVersionFenceV1,
    prior_objects: Vec<FastPayPriorObjectV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prior_unwrap_account: Option<postfiat_types::Account>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retained_tip: Option<FastPaySpeculativeTipV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FastPaySpeculativeTipV1 {
    height: u64,
    block_hash: String,
    state_root: String,
    application_sequence: u64,
}

/// Process-local FastPay ledger cache for transactional stores.
///
/// Keyed by `(canonical_data_dir, db_identity, chain_tip, store_meta_commitment,
/// journal_bytes_hash)`. Intentionally omits file `mtime`/`ctime` because `redb`
/// updates file timestamps on every read-write `Database::open` even when no
/// write transaction occurs; cryptographic tip/meta commitments plus file inode
/// and byte length detect any database write or file replacement without
/// false cache misses. Never retains a `redb` handle across RPCs so sibling
/// services (`transport-validator-serve`) are never starved.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FastPayDbIdentity {
    path: std::path::PathBuf,
    #[cfg(unix)]
    ino: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FastPayLedgerCacheKey {
    data_dir: std::path::PathBuf,
    generation: String,
    height: u64,
    block_hash: String,
    state_root: String,
    ordered_history_accumulator: String,
    db_identity: FastPayDbIdentity,
    journal_bytes_hash: [u8; 32],
}

#[derive(Clone)]
struct CachedFastPayState {
    key: FastPayLedgerCacheKey,
    ledger: std::sync::Arc<LedgerState>,
    genesis: Genesis,
    pending_unanchored_count: usize,
    fee_v1_active: bool,
}

struct LoadedFastPayState {
    cache_key: Option<FastPayLedgerCacheKey>,
    height: u64,
    ledger: std::sync::Arc<LedgerState>,
    genesis: Genesis,
    pending_unanchored_count: usize,
    fee_v1_active: bool,
}

const MAX_FASTPAY_LEDGER_CACHE_ENTRIES: usize = 64;
const MAX_FASTPAY_SIGNED_ORDER_LRU_ENTRIES: usize = 256;

#[derive(Debug, Clone)]
pub enum FastPayApplyOutcomeV3 {
    Ack(postfiat_types::FastPayApplyAckV1),
    Held {
        validator_id: String,
        lock_id: String,
        certificate_digest: String,
    },
}

impl FastPayApplyOutcomeV3 {
    pub fn to_json_string(&self) -> io::Result<String> {
        match self {
            Self::Ack(ack) => serde_json::to_string(ack).map_err(invalid_data),
            Self::Held {
                validator_id,
                lock_id,
                certificate_digest,
            } => serde_json::to_string(&serde_json::json!({
                "schema": FASTPAY_HELD_EFFECT_SCHEMA_V1,
                "validator_id": validator_id,
                "lock_id": lock_id,
                "certificate_digest": certificate_digest,
                "signed": false,
            }))
            .map_err(invalid_data),
        }
    }
}

struct FastPayOrderLruCache {
    entries: std::collections::VecDeque<(
        (PathBuf, String),
        postfiat_types::SignedOwnedTransferOrderV3,
    )>,
}

impl FastPayOrderLruCache {
    fn new() -> Self {
        Self {
            entries: std::collections::VecDeque::with_capacity(
                MAX_FASTPAY_SIGNED_ORDER_LRU_ENTRIES,
            ),
        }
    }

    fn put(
        &mut self,
        dir: PathBuf,
        lock_id: String,
        order: postfiat_types::SignedOwnedTransferOrderV3,
    ) {
        let key = (dir, lock_id);
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
        } else if self.entries.len() >= MAX_FASTPAY_SIGNED_ORDER_LRU_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back((key, order));
    }

    fn get(
        &mut self,
        dir: &std::path::Path,
        lock_id: &str,
    ) -> Option<postfiat_types::SignedOwnedTransferOrderV3> {
        let pos = self
            .entries
            .iter()
            .position(|((d, l), _)| d.as_path() == dir && l == lock_id)?;
        let (key, order) = self.entries.remove(pos)?;
        self.entries.push_back((key, order.clone()));
        Some(order)
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.entries.clear();
    }
}

fn fastpay_signed_order_lru() -> &'static std::sync::Mutex<FastPayOrderLruCache> {
    static LRU: std::sync::OnceLock<std::sync::Mutex<FastPayOrderLruCache>> =
        std::sync::OnceLock::new();
    LRU.get_or_init(|| std::sync::Mutex::new(FastPayOrderLruCache::new()))
}

pub fn cache_fastpay_signed_order_v3(
    data_dir: &std::path::Path,
    signed: &postfiat_types::SignedOwnedTransferOrderV3,
) {
    let dir = canonical_fastpay_cache_dir(data_dir);
    let mut lru = fastpay_signed_order_lru()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    lru.put(dir, signed.order.recovery.lock_id.clone(), signed.clone());
}

pub fn lookup_cached_fastpay_signed_order_v3(
    data_dir: &std::path::Path,
    lock_id: &str,
) -> Option<postfiat_types::SignedOwnedTransferOrderV3> {
    let dir = canonical_fastpay_cache_dir(data_dir);
    let mut lru = fastpay_signed_order_lru()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    lru.get(&dir, lock_id)
}

#[cfg(test)]
pub fn clear_fastpay_signed_order_lru_for_test() {
    fastpay_signed_order_lru()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

#[cfg(test)]
pub(super) static FASTPAY_LEDGER_CACHE_HITS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
pub(super) static FASTPAY_REDB_OPENS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
struct FastPayWalFileIdentity {
    len: u64,
    #[cfg(unix)]
    ino: u64,
    mtime_nanos: u128,
}

#[derive(Clone)]
struct FastPayWalHead {
    identity: FastPayWalFileIdentity,
    chain_hash: [u8; 32],
    journal: FastPaySpeculativeJournalV1,
}

fn fastpay_wal_tracker() -> &'static std::sync::Mutex<HashMap<PathBuf, FastPayWalHead>> {
    static TRACKER: std::sync::OnceLock<std::sync::Mutex<HashMap<PathBuf, FastPayWalHead>>> =
        std::sync::OnceLock::new();
    TRACKER.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn fastpay_wal_file_identity(path: &std::path::Path) -> io::Result<FastPayWalFileIdentity> {
    let meta = std::fs::metadata(path)?;
    #[cfg(unix)]
    let ino = {
        use std::os::unix::fs::MetadataExt;
        meta.ino()
    };
    let mtime_nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Ok(FastPayWalFileIdentity {
        len: meta.len(),
        #[cfg(unix)]
        ino,
        mtime_nanos,
    })
}

fn invalidate_fastpay_wal_tracker(data_dir: &std::path::Path) {
    let dir = canonical_fastpay_cache_dir(data_dir);
    let mut tracker = fastpay_wal_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tracker.remove(&dir);
}

fn fastpay_ledger_cache() -> &'static std::sync::Mutex<Vec<CachedFastPayState>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Vec<CachedFastPayState>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn canonical_fastpay_cache_dir(data_dir: &std::path::Path) -> std::path::PathBuf {
    std::fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf())
}

pub(super) fn invalidate_fastpay_ledger_cache(data_dir: &std::path::Path) {
    let dir = canonical_fastpay_cache_dir(data_dir);
    let mut cache = fastpay_ledger_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.retain(|entry| entry.key.data_dir != dir);
    drop(cache);
    let mut prewarmed = fastpay_prewarmed_tip_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prewarmed.remove(&dir);
    drop(prewarmed);
    invalidate_fastpay_wal_tracker(data_dir);
}

fn fastpay_db_identity(db_path: &std::path::Path) -> io::Result<FastPayDbIdentity> {
    let meta = std::fs::metadata(db_path)?;
    #[cfg(unix)]
    let ino = {
        use std::os::unix::fs::MetadataExt;
        meta.ino()
    };
    Ok(FastPayDbIdentity {
        path: canonical_fastpay_cache_dir(db_path),
        #[cfg(unix)]
        ino,
    })
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).into()
}

fn rolling_wal_chain_hash(prev: [u8; 32], frame_bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(prev);
    hasher.update(frame_bytes);
    hasher.finalize().into()
}

fn encode_fastpay_wal_frame(effect: &FastPaySpeculativeEffectV1) -> io::Result<Vec<u8>> {
    let payload = serde_cbor::to_vec(effect).map_err(invalid_data)?;
    if payload.len() > FASTPAY_SPECULATIVE_WAL_MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "FastPay speculative WAL frame exceeds size limit",
        ));
    }
    let crc = crc32c_checksum(&payload);
    let mut frame = Vec::with_capacity(FASTPAY_SPECULATIVE_WAL_HEADER_LEN + payload.len());
    frame.extend_from_slice(&FASTPAY_SPECULATIVE_WAL_MAGIC_V2.to_le_bytes());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&crc.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn decode_fastpay_wal_file(
    path: &std::path::Path,
    truncate_torn_tail: bool,
) -> io::Result<(FastPaySpeculativeJournalV1, [u8; 32])> {
    let bytes = std::fs::read(path)?;
    let mut offset = 0usize;
    let mut chain_hash = [0u8; 32];
    let mut by_lock: BTreeMap<String, FastPaySpeculativeEffectV1> = BTreeMap::new();
    let mut torn_tail = false;

    while offset < bytes.len() {
        let remaining = bytes.len() - offset;
        if remaining < FASTPAY_SPECULATIVE_WAL_HEADER_LEN {
            torn_tail = true;
            break;
        }
        let magic = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        if magic != FASTPAY_SPECULATIVE_WAL_MAGIC_V2 {
            if offset == 0 {
                return Err(fastpay_invalid_data(
                    "FastPay speculative WAL has invalid frame magic",
                ));
            }
            return Err(fastpay_invalid_data(
                "FastPay speculative WAL has corrupt mid-stream frame magic",
            ));
        }
        let payload_len = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let expected_crc = u32::from_le_bytes([
            bytes[offset + 8],
            bytes[offset + 9],
            bytes[offset + 10],
            bytes[offset + 11],
        ]);
        if payload_len == 0 || payload_len > FASTPAY_SPECULATIVE_WAL_MAX_FRAME_BYTES {
            return Err(fastpay_invalid_data(
                "FastPay speculative WAL frame length is out of bounds",
            ));
        }
        let Some(frame_end) = offset
            .checked_add(FASTPAY_SPECULATIVE_WAL_HEADER_LEN)
            .and_then(|v| v.checked_add(payload_len))
        else {
            return Err(fastpay_invalid_data(
                "FastPay speculative WAL frame offset overflow",
            ));
        };
        if frame_end > bytes.len() {
            torn_tail = true;
            break;
        }
        let payload = &bytes[offset + FASTPAY_SPECULATIVE_WAL_HEADER_LEN..frame_end];
        let actual_crc = crc32c_checksum(payload);
        if actual_crc != expected_crc {
            if frame_end == bytes.len() {
                torn_tail = true;
                break;
            }
            return Err(fastpay_invalid_data(
                "FastPay speculative WAL CRC32C mismatch in non-tail frame",
            ));
        }
        let effect: FastPaySpeculativeEffectV1 = match serde_cbor::from_slice(payload) {
            Ok(effect) => effect,
            Err(_) if frame_end == bytes.len() => {
                torn_tail = true;
                break;
            }
            Err(error) => return Err(invalid_data(error)),
        };
        effect
            .fence
            .validate_shape()
            .map_err(fastpay_invalid_data)?;
        if effect.fence.origin != postfiat_types::FastPayFenceOriginV1::Consensusless {
            return Err(fastpay_invalid_data(
                "FastPay speculative WAL contains non-consensusless effect",
            ));
        }
        if let Some(existing) = by_lock.get(&effect.fence.lock_id) {
            if existing != &effect {
                return Err(fastpay_invalid_data(
                    "FastPay speculative WAL contains a conflicting duplicate lock",
                ));
            }
        } else {
            by_lock.insert(effect.fence.lock_id.clone(), effect);
        }
        chain_hash = rolling_wal_chain_hash(chain_hash, &bytes[offset..frame_end]);
        offset = frame_end;
    }

    if torn_tail && truncate_torn_tail {
        if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
            let _ = file.set_len(offset as u64);
            let _ = file.sync_data();
        }
    }

    let journal = FastPaySpeculativeJournalV1 {
        schema: FASTPAY_SPECULATIVE_JOURNAL_SCHEMA_V1.to_string(),
        effects: by_lock.into_values().collect(),
    };
    Ok((journal, chain_hash))
}

fn load_or_refresh_fastpay_wal_head(
    data_dir: &std::path::Path,
) -> io::Result<Option<FastPayWalHead>> {
    let wal_path = data_dir.join(FASTPAY_SPECULATIVE_WAL_FILE);
    if !wal_path.exists() {
        return Ok(None);
    }
    let dir_key = canonical_fastpay_cache_dir(data_dir);
    let identity = fastpay_wal_file_identity(&wal_path)?;
    {
        let tracker = fastpay_wal_tracker()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(head) = tracker.get(&dir_key) {
            if head.identity == identity {
                return Ok(Some(head.clone()));
            }
        }
    }
    let (journal, chain_hash) = decode_fastpay_wal_file(&wal_path, true)?;
    let post_identity = fastpay_wal_file_identity(&wal_path)?;
    let head = FastPayWalHead {
        identity: post_identity,
        chain_hash,
        journal,
    };
    let mut tracker = fastpay_wal_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tracker.insert(dir_key, head.clone());
    Ok(Some(head))
}

fn fastpay_speculative_journal_hash(data_dir: &std::path::Path) -> io::Result<[u8; 32]> {
    let v1_path = data_dir.join(FASTPAY_SPECULATIVE_JOURNAL_FILE);
    let v2_path = data_dir.join(FASTPAY_SPECULATIVE_WAL_FILE);
    let shadow = fastpay_wal_shadow_verify_enabled();

    if !shadow && v1_path.exists() {
        let raw = read_bounded_json_text_file(&v1_path, "FastPay speculative-effect journal")?;
        let v1_hash = sha256_bytes(raw.as_bytes());
        if v2_path.exists() {
            if let Some(head) = load_or_refresh_fastpay_wal_head(data_dir)? {
                return Ok(rolling_wal_chain_hash(head.chain_hash, &v1_hash));
            }
        }
        return Ok(v1_hash);
    }
    if let Some(head) = load_or_refresh_fastpay_wal_head(data_dir)? {
        return Ok(head.chain_hash);
    }
    if v1_path.exists() {
        let raw = read_bounded_json_text_file(&v1_path, "FastPay speculative-effect journal")?;
        return Ok(sha256_bytes(raw.as_bytes()));
    }
    Ok([0u8; 32])
}

fn take_cached_fastpay_state(key: &FastPayLedgerCacheKey) -> Option<CachedFastPayState> {
    let mut cache = fastpay_ledger_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let pos = cache.iter().position(|entry| &entry.key == key)?;
    Some(cache.remove(pos))
}

fn peek_cached_fastpay_state(key: &FastPayLedgerCacheKey) -> Option<CachedFastPayState> {
    let cache = fastpay_ledger_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.iter().find(|entry| &entry.key == key).cloned()
}

fn store_cached_fastpay_state(state: CachedFastPayState) {
    let mut cache = fastpay_ledger_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.retain(|entry| entry.key.data_dir != state.key.data_dir);
    if cache.len() >= MAX_FASTPAY_LEDGER_CACHE_ENTRIES {
        cache.remove(0);
    }
    cache.push(state);
}

fn load_fastpay_state(store: &NodeStore, take_ownership: bool) -> io::Result<LoadedFastPayState> {
    if store.storage_backend_mode()?.is_transactional() {
        // Hot path: check the <256 B tip-generation sidecar + stat(db_path) +
        // O(1) WAL head hash without opening redb or acquiring a writer lease.
        if let Some(sidecar) = read_chain_tip_generation_sidecar(store.data_dir()) {
            let active = sidecar
                .scheduled_activation_height
                .is_some_and(|activation| sidecar.finalized_height >= activation);
            if active {
                if let Ok(db_identity) = fastpay_db_identity(&sidecar.db_path) {
                    // Match on canonical path + inode rather than byte length because redb
                    // shrinks/expands unused trailing pages on open/close without changing state.
                    let db_matches = db_identity.path == sidecar.db_path && {
                        #[cfg(unix)]
                        {
                            db_identity.ino == sidecar.db_ino
                        }
                        #[cfg(not(unix))]
                        {
                            true
                        }
                    };
                    if db_matches {
                        let journal_bytes_hash =
                            fastpay_speculative_journal_hash(store.data_dir())?;
                        let key = FastPayLedgerCacheKey {
                            data_dir: canonical_fastpay_cache_dir(store.data_dir()),
                            generation: sidecar.generation.clone(),
                            height: sidecar.finalized_height,
                            block_hash: sidecar.finalized_block_hash.clone(),
                            state_root: sidecar.finalized_state_root.clone(),
                            ordered_history_accumulator: sidecar
                                .ordered_history_accumulator
                                .clone(),
                            db_identity,
                            journal_bytes_hash,
                        };
                        let hit = if take_ownership {
                            take_cached_fastpay_state(&key)
                        } else {
                            peek_cached_fastpay_state(&key)
                        };
                        if let Some(cached) = hit {
                            #[cfg(test)]
                            FASTPAY_LEDGER_CACHE_HITS
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            update_fastpay_rpc_metrics(|m| {
                                m.ledger_cache_hit = Some(true);
                                m.redb_opened = Some(false);
                                m.redb_lease_wait_ms = Some(0.0);
                            });
                            return Ok(LoadedFastPayState {
                                height: cached.key.height,
                                cache_key: Some(cached.key),
                                ledger: cached.ledger,
                                genesis: cached.genesis,
                                pending_unanchored_count: cached.pending_unanchored_count,
                                fee_v1_active: cached.fee_v1_active,
                            });
                        }
                        if let Some(hydrated) = hydrate_fastpay_state_from_snapshot_sidecar(
                            store.data_dir(),
                            &sidecar,
                            &key,
                            take_ownership,
                        )? {
                            return Ok(hydrated);
                        }
                    }
                }
            }
        }

        // Cold miss or sidecar stale/missing: open redb once under bounded lease,
        // reconcile sidecar, and warm the in-process ledger cache.
        let lease_start = std::time::Instant::now();
        let _db_lease = store.transactional_store_lease()?;
        let lease_wait_ms = lease_start.elapsed().as_secs_f64() * 1000.0;
        #[cfg(test)]
        FASTPAY_REDB_OPENS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        update_fastpay_rpc_metrics(|m| {
            m.redb_lease_wait_ms = Some(lease_wait_ms);
            m.redb_opened = Some(true);
        });

        if store.transactional_storage_active()? {
            let transactional = store.transactional_store()?;
            let meta = transactional.meta()?;
            let db_identity = fastpay_db_identity(transactional.database_path())?;
            let _ = write_chain_tip_generation_sidecar(
                store.data_dir(),
                &meta,
                transactional.database_path(),
            );
            let journal_bytes_hash = fastpay_speculative_journal_hash(store.data_dir())?;
            let key = FastPayLedgerCacheKey {
                data_dir: canonical_fastpay_cache_dir(store.data_dir()),
                generation: meta.generation.clone(),
                height: meta.finalized_height,
                block_hash: meta.finalized_block_hash.clone(),
                state_root: meta.finalized_state_root.clone(),
                ordered_history_accumulator: meta.ordered_history_accumulator.clone(),
                db_identity,
                journal_bytes_hash,
            };
            let hit = if take_ownership {
                take_cached_fastpay_state(&key)
            } else {
                peek_cached_fastpay_state(&key)
            };
            if let Some(cached) = hit {
                #[cfg(test)]
                FASTPAY_LEDGER_CACHE_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                update_fastpay_rpc_metrics(|m| {
                    m.ledger_cache_hit = Some(true);
                });
                return Ok(LoadedFastPayState {
                    height: cached.key.height,
                    cache_key: Some(cached.key),
                    ledger: cached.ledger,
                    genesis: cached.genesis,
                    pending_unanchored_count: cached.pending_unanchored_count,
                    fee_v1_active: cached.fee_v1_active,
                });
            }

            let base_ledger = store.read_ledger()?;
            let genesis = store.read_genesis()?;
            let fee_v1_activation = store.read_governance()?.fastpay_fee_v1_activation_height();
            let base_pending_unanchored_count =
                fastpay_pre_state_effects_for_next_block(store, &base_ledger)?.len();
            let _ = write_fastpay_ledger_snapshot_sidecar(
                store.data_dir(),
                &meta,
                &base_ledger,
                &genesis,
                fee_v1_activation,
                base_pending_unanchored_count,
            );
            let _ = write_chain_tip_generation_sidecar(
                store.data_dir(),
                &meta,
                transactional.database_path(),
            );
            let ledger = std::sync::Arc::new(read_fastpay_ledger(store)?);
            let pending_unanchored_count =
                fastpay_pre_state_effects_for_next_block(store, &ledger)?.len();
            let fee_v1_active = fee_v1_activation
                .is_some_and(|activation| activation > 0 && key.height >= activation);
            if !take_ownership {
                store_cached_fastpay_state(CachedFastPayState {
                    key: key.clone(),
                    ledger: std::sync::Arc::clone(&ledger),
                    genesis: genesis.clone(),
                    pending_unanchored_count,
                    fee_v1_active,
                });
            }
            return Ok(LoadedFastPayState {
                height: key.height,
                cache_key: Some(key),
                ledger,
                genesis,
                pending_unanchored_count,
                fee_v1_active,
            });
        }
    }

    let ledger = std::sync::Arc::new(read_fastpay_ledger(store)?);
    let height = fastpay_height(store)?;
    let pending_unanchored_count = fastpay_pre_state_effects_for_next_block(store, &ledger)?.len();
    let genesis = store.read_genesis()?;
    let fee_v1_active = store
        .read_governance()?
        .fastpay_fee_v1_activation_height()
        .is_some_and(|activation| activation > 0 && height >= activation);
    Ok(LoadedFastPayState {
        cache_key: None,
        height,
        ledger,
        genesis,
        pending_unanchored_count,
        fee_v1_active,
    })
}

/// Business-state view. The transactional database remains the finalized base;
/// pending, certified FastPay effects live in the existing durable journal.
pub(super) fn read_fastpay_ledger(store: &NodeStore) -> io::Result<LedgerState> {
    if !store.transactional_storage_active()? {
        return store.read_ledger();
    }
    let _db_lease = store.transactional_store_lease()?;
    for _ in 0..3 {
        let tip = store.read_chain_tip()?;
        let mut ledger = store.read_ledger()?;
        let journal = read_fastpay_speculative_journal(store.data_dir())?;
        let mut pending = Vec::new();
        let mut legacy_pending = 0;
        for record in &journal.effects {
            // Finalized ordered recovery/cancellation always wins over an old
            // local certificate retained for recovery and audit.
            if ledger
                .fastpay_version_fences
                .iter()
                .any(|f| f.lock_id == record.fence.lock_id)
            {
                continue;
            }
            let (height, sequence) = match &record.retained_tip {
                Some(base) => {
                    if base.height == tip.height
                        && (base.block_hash != tip.block_hash || base.state_root != tip.state_root)
                    {
                        return Err(fastpay_invalid_data(
                            "FastPay journal is bound to a different finalized tip",
                        ));
                    }
                    (base.height, base.application_sequence)
                }
                None => (record.fence.decided_at_height, 0),
            };
            if height > tip.height {
                return Err(fastpay_invalid_data(
                    "FastPay journal is ahead of the finalized tip",
                ));
            }
            if height < tip.height {
                // An intervening certified block omitted this minority effect.
                // Keep its certificate retrievable, but never resurrect it.
                continue;
            }
            if record.retained_tip.is_none() {
                legacy_pending += 1;
            }
            pending.push((sequence, record.fence.clone()));
        }
        if pending.len() > postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK
            || legacy_pending > 1
        {
            return Err(fastpay_invalid_data(
                "FastPay pending journal is oversized or has ambiguous legacy application order",
            ));
        }
        pending.sort_by_key(|(sequence, _)| *sequence);
        if pending.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(fastpay_invalid_data(
                "FastPay journal has duplicate application sequence",
            ));
        }
        let effects = pending
            .into_iter()
            .map(|(_, fence)| fence)
            .collect::<Vec<_>>();
        if effects.is_empty() {
            if store.read_chain_tip()? == tip {
                return Ok(ledger);
            }
            continue;
        }
        let fastpay_fee_v1_activation = store.read_governance()?.fastpay_fee_v1_activation_height();
        if store.read_chain_tip()? != tip {
            continue;
        }
        // FastPay transfers and unwraps do not mutate ShieldedState, so native
        // supply conservation (`supply_after + fee == supply_before`) is
        // invariant to the constant shielded reserve and needs no shielded read.
        let empty_shielded = ShieldedState::empty();
        replay_confirmed_fastpay_fences_dependency_ordered(
            &mut ledger,
            &empty_shielded,
            &effects,
            fastpay_fee_v1_activation,
        )?;
        if store.read_chain_tip()? == tip {
            return Ok(ledger);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "finalized tip changed while reading FastPay state",
    ))
}

fn format_fastpay_owned_transfer_error(
    prefix: &str,
    error: postfiat_execution::OwnedTransferError,
) -> String {
    match error {
        postfiat_execution::OwnedTransferError::InsufficientFee { fee, required } => {
            format!("fastpay_insufficient_fee: {prefix}: minimum {required}, provided {fee}")
        }
        postfiat_execution::OwnedTransferError::AccountOwnedObjectCapExceeded {
            owner_pubkey_hex,
            current,
            limit,
        } => {
            format!(
                "fastpay_account_object_cap_exceeded: {prefix}: owner {owner_pubkey_hex} current {current} cap {limit}"
            )
        }
        postfiat_execution::OwnedTransferError::ResourceLimitExceeded => {
            format!("fastpay_global_object_cap_exceeded: {prefix}: resource limit exceeded")
        }
        other => format!("{prefix}: {other:?}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FastPaySpeculativeJournalV1 {
    schema: String,
    effects: Vec<FastPaySpeculativeEffectV1>,
}

impl Default for FastPaySpeculativeJournalV1 {
    fn default() -> Self {
        Self {
            schema: FASTPAY_SPECULATIVE_JOURNAL_SCHEMA_V1.to_string(),
            effects: Vec::new(),
        }
    }
}

fn fastpay_invalid_data(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

fn read_fastpay_speculative_journal_v1_file(
    path: &std::path::Path,
) -> io::Result<FastPaySpeculativeJournalV1> {
    let raw = read_bounded_json_text_file(path, "FastPay speculative-effect journal")?;
    let journal: FastPaySpeculativeJournalV1 = serde_json::from_str(&raw).map_err(invalid_data)?;
    if journal.schema != FASTPAY_SPECULATIVE_JOURNAL_SCHEMA_V1 {
        return Err(fastpay_invalid_data(
            "FastPay speculative-effect journal schema mismatch",
        ));
    }
    let mut prior_lock_id: Option<&str> = None;
    for effect in &journal.effects {
        effect
            .fence
            .validate_shape()
            .map_err(fastpay_invalid_data)?;
        if effect.fence.origin != postfiat_types::FastPayFenceOriginV1::Consensusless
            || prior_lock_id.is_some_and(|prior| prior >= effect.fence.lock_id.as_str())
        {
            return Err(fastpay_invalid_data(
                "FastPay speculative-effect journal is not canonical",
            ));
        }
        prior_lock_id = Some(&effect.fence.lock_id);
    }
    Ok(journal)
}

fn read_fastpay_speculative_journal(
    data_dir: &std::path::Path,
) -> io::Result<FastPaySpeculativeJournalV1> {
    let v1_path = data_dir.join(FASTPAY_SPECULATIVE_JOURNAL_FILE);
    let v2_path = data_dir.join(FASTPAY_SPECULATIVE_WAL_FILE);
    let shadow = fastpay_wal_shadow_verify_enabled();

    if !shadow && v1_path.exists() {
        let v1_journal = read_fastpay_speculative_journal_v1_file(&v1_path)?;
        if v2_path.exists() {
            let head = load_or_refresh_fastpay_wal_head(data_dir)?
                .ok_or_else(|| fastpay_invalid_data("FastPay WAL vanished during read"))?;
            if head.journal != v1_journal {
                return Err(fastpay_invalid_data(
                    "FastPay speculative v1 JSON and v2 WAL journals disagree",
                ));
            }
        }
        return Ok(v1_journal);
    }
    if let Some(head) = load_or_refresh_fastpay_wal_head(data_dir)? {
        return Ok(head.journal);
    }
    if v1_path.exists() {
        return read_fastpay_speculative_journal_v1_file(&v1_path);
    }
    Ok(FastPaySpeculativeJournalV1::default())
}

fn write_fastpay_speculative_journal(
    data_dir: &std::path::Path,
    journal: &FastPaySpeculativeJournalV1,
) -> io::Result<[u8; 32]> {
    let bytes = format!(
        "{}\n",
        serde_json::to_string_pretty(journal).map_err(invalid_data)?
    );
    if bytes.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "FastPay speculative-effect journal is full; settle existing recovery windows",
        ));
    }
    let hash = sha256_bytes(bytes.as_bytes());
    atomic_write(data_dir.join(FASTPAY_SPECULATIVE_JOURNAL_FILE), bytes)?;
    Ok(hash)
}

fn append_fastpay_wal_frame(
    data_dir: &std::path::Path,
    effect: FastPaySpeculativeEffectV1,
    mut journal: FastPaySpeculativeJournalV1,
    prev_chain_hash: [u8; 32],
) -> io::Result<[u8; 32]> {
    use std::io::Write;
    let frame = encode_fastpay_wal_frame(&effect)?;
    let wal_path = data_dir.join(FASTPAY_SPECULATIVE_WAL_FILE);
    let sync_start = std::time::Instant::now();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&wal_path)?;
    file.write_all(&frame)?;
    file.sync_data()?;
    let sync_ms = sync_start.elapsed().as_secs_f64() * 1000.0;
    update_fastpay_rpc_metrics(|m| {
        m.wal_fdatasync_ms = Some(sync_ms);
    });
    let new_chain_hash = rolling_wal_chain_hash(prev_chain_hash, &frame);
    journal.effects.push(effect);
    journal
        .effects
        .sort_by(|left, right| left.fence.lock_id.cmp(&right.fence.lock_id));
    let identity = fastpay_wal_file_identity(&wal_path)?;
    let dir_key = canonical_fastpay_cache_dir(data_dir);
    let mut tracker = fastpay_wal_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tracker.insert(
        dir_key,
        FastPayWalHead {
            identity,
            chain_hash: new_chain_hash,
            journal,
        },
    );
    Ok(new_chain_hash)
}

fn rewrite_fastpay_wal_compacted(
    data_dir: &std::path::Path,
    journal: &FastPaySpeculativeJournalV1,
) -> io::Result<[u8; 32]> {
    let mut buf = Vec::new();
    let mut chain_hash = [0u8; 32];
    for effect in &journal.effects {
        let frame = encode_fastpay_wal_frame(effect)?;
        chain_hash = rolling_wal_chain_hash(chain_hash, &frame);
        buf.extend_from_slice(&frame);
    }
    let wal_path = data_dir.join(FASTPAY_SPECULATIVE_WAL_FILE);
    let sync_start = std::time::Instant::now();
    atomic_write(&wal_path, buf)?;
    let sync_ms = sync_start.elapsed().as_secs_f64() * 1000.0;
    update_fastpay_rpc_metrics(|m| {
        m.wal_fdatasync_ms = Some(sync_ms);
    });
    let identity = fastpay_wal_file_identity(&wal_path)?;
    let dir_key = canonical_fastpay_cache_dir(data_dir);
    let mut tracker = fastpay_wal_tracker()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tracker.insert(
        dir_key,
        FastPayWalHead {
            identity,
            chain_hash,
            journal: journal.clone(),
        },
    );
    Ok(chain_hash)
}

pub fn fastpay_wal_export_v1(options: NodeOptions) -> io::Result<String> {
    let store = NodeStore::new(&options.data_dir);
    let _mutation_lock = store.lock_ordered_commit()?;
    let journal = read_fastpay_speculative_journal(&options.data_dir)?;
    let v1_hash = write_fastpay_speculative_journal(&options.data_dir, &journal)?;
    invalidate_fastpay_ledger_cache(&options.data_dir);
    serde_json::to_string(&serde_json::json!({
        "schema": "postfiat-fastpay-wal-export-v1",
        "exported_effects": journal.effects.len(),
        "v1_path": options.data_dir.join(FASTPAY_SPECULATIVE_JOURNAL_FILE),
        "v1_sha256": bytes_to_hex(&v1_hash),
    }))
    .map_err(invalid_data)
}

pub(super) fn fastpay_speculative_journal_snapshot_bytes(
    data_dir: &std::path::Path,
) -> io::Result<Vec<u8>> {
    let journal = read_fastpay_speculative_journal(data_dir)?;
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&journal).map_err(invalid_data)?
    )
    .into_bytes())
}

fn prior_objects_for_fastpay_inputs(
    ledger: &LedgerState,
    inputs: &[postfiat_types::OwnedObjectRef],
) -> io::Result<Vec<FastPayPriorObjectV1>> {
    let mut prior = Vec::with_capacity(inputs.len());
    for input in inputs {
        let (index, object) = ledger
            .owned_objects
            .iter()
            .enumerate()
            .find(|(_, object)| object.id == input.id && object.version == input.version)
            .ok_or_else(|| fastpay_invalid_data("FastPay speculative input is absent"))?;
        prior.push(FastPayPriorObjectV1 {
            index: u64::try_from(index).map_err(invalid_data)?,
            object: object.clone(),
        });
    }
    prior.sort_by_key(|entry| entry.index);
    if prior.windows(2).any(|pair| pair[0].index >= pair[1].index) {
        return Err(fastpay_invalid_data(
            "FastPay speculative input indexes are not unique",
        ));
    }
    Ok(prior)
}

fn retain_fastpay_speculative_effect(
    store: &NodeStore,
    ledger: &LedgerState,
    mut effect: FastPaySpeculativeEffectV1,
    tip_hint: Option<&FastPayLedgerCacheKey>,
) -> io::Result<[u8; 32]> {
    let transactional_active = match tip_hint {
        Some(_) => true,
        None => store.transactional_storage_active()?,
    };
    let tip_height = if transactional_active {
        let (height, block_hash, state_root) = match tip_hint {
            Some(key) => (key.height, key.block_hash.clone(), key.state_root.clone()),
            None => {
                let tip = store.read_chain_tip()?;
                (tip.height, tip.block_hash, tip.state_root)
            }
        };
        effect.retained_tip = Some(FastPaySpeculativeTipV1 {
            height,
            block_hash,
            state_root,
            application_sequence: ledger.fastpay_version_fences.len() as u64,
        });
        height
    } else {
        fastpay_height(store)?
    };

    if !transactional_active {
        // Legacy non-transactional JSON storage retains v1 JSON journal.
        let mut journal = read_fastpay_speculative_journal(store.data_dir())?;
        if let Some(existing) = journal
            .effects
            .iter()
            .find(|existing| existing.fence.lock_id == effect.fence.lock_id)
        {
            return if existing == &effect {
                fastpay_speculative_journal_hash(store.data_dir())
            } else {
                Err(fastpay_invalid_data(
                    "FastPay speculative journal contains a conflicting lock",
                ))
            };
        }
        let anchored = store
            .read_blocks()?
            .blocks
            .into_iter()
            .flat_map(|block| block.fastpay_pre_state_effects)
            .map(|fence| fence.lock_id)
            .collect::<BTreeSet<_>>();
        journal.effects.retain(|record| {
            let recovery = record
                .fence
                .certificate
                .as_ref()
                .map(postfiat_types::FastPayCertificateV1::recovery);
            let ordered_terminal = ledger.fastpay_version_fences.iter().any(|fence| {
                fence.lock_id == record.fence.lock_id
                    && fence.origin == postfiat_types::FastPayFenceOriginV1::OrderedRecovery
            });
            !anchored.contains(&record.fence.lock_id)
                && !ordered_terminal
                && (ledger.fastpay_version_fences.contains(&record.fence)
                    || recovery.is_some_and(|window| tip_height < window.recovery_closes_at_height))
        });
        journal.effects.push(effect);
        journal
            .effects
            .sort_by(|left, right| left.fence.lock_id.cmp(&right.fence.lock_id));
        return write_fastpay_speculative_journal(store.data_dir(), &journal);
    }

    // Transactional storage uses append-only framed CRC32C WAL (`v2.wal`).
    let shadow = fastpay_wal_shadow_verify_enabled();
    let v1_path = store.data_dir().join(FASTPAY_SPECULATIVE_JOURNAL_FILE);
    let v2_path = store.data_dir().join(FASTPAY_SPECULATIVE_WAL_FILE);
    let v1_needs_migration = v1_path.exists() && !v2_path.exists();

    let (mut journal, prev_chain_hash) =
        if let Some(head) = load_or_refresh_fastpay_wal_head(store.data_dir())? {
            if !shadow && v1_path.exists() {
                let v1 = read_fastpay_speculative_journal_v1_file(&v1_path)?;
                if v1 != head.journal {
                    return Err(fastpay_invalid_data(
                        "FastPay speculative v1 JSON and v2 WAL journals disagree",
                    ));
                }
            }
            (head.journal, head.chain_hash)
        } else if v1_path.exists() {
            (
                read_fastpay_speculative_journal_v1_file(&v1_path)?,
                [0u8; 32],
            )
        } else {
            (FastPaySpeculativeJournalV1::default(), [0u8; 32])
        };

    if let Some(existing) = journal
        .effects
        .iter()
        .find(|existing| existing.fence.lock_id == effect.fence.lock_id)
    {
        return if existing == &effect {
            fastpay_speculative_journal_hash(store.data_dir())
        } else {
            Err(fastpay_invalid_data(
                "FastPay speculative journal contains a conflicting lock",
            ))
        };
    }

    // Records appended at the current finalized tip height cannot be anchored in
    // finalized redb history yet; avoid opening redb and perform O(1) WAL append.
    let has_older_height_records = journal.effects.iter().any(|record| {
        record
            .retained_tip
            .as_ref()
            .map_or(true, |retained| retained.height != tip_height)
    });
    let has_ordered_terminal = journal.effects.iter().any(|record| {
        ledger.fastpay_version_fences.iter().any(|fence| {
            fence.lock_id == record.fence.lock_id
                && fence.origin == postfiat_types::FastPayFenceOriginV1::OrderedRecovery
        })
    });

    let new_chain_hash =
        if !v1_needs_migration && !has_older_height_records && !has_ordered_terminal {
            append_fastpay_wal_frame(store.data_dir(), effect, journal, prev_chain_hash)?
        } else {
            let anchored = if has_older_height_records {
                let _db_lease = store.transactional_store_lease()?;
                let transactional = store.transactional_store()?;
                let mut anchored = BTreeSet::new();
                for record in &journal.effects {
                    if transactional
                        .anchored_fastpay_effect(&record.fence.lock_id)?
                        .is_some()
                    {
                        anchored.insert(record.fence.lock_id.clone());
                    }
                }
                anchored
            } else {
                BTreeSet::new()
            };
            journal.effects.retain(|record| {
                let recovery = record
                    .fence
                    .certificate
                    .as_ref()
                    .map(postfiat_types::FastPayCertificateV1::recovery);
                let ordered_terminal = ledger.fastpay_version_fences.iter().any(|fence| {
                    fence.lock_id == record.fence.lock_id
                        && fence.origin == postfiat_types::FastPayFenceOriginV1::OrderedRecovery
                });
                !anchored.contains(&record.fence.lock_id)
                    && !ordered_terminal
                    && (ledger.fastpay_version_fences.contains(&record.fence)
                        || recovery
                            .is_some_and(|window| tip_height < window.recovery_closes_at_height))
            });
            journal.effects.push(effect);
            journal
                .effects
                .sort_by(|left, right| left.fence.lock_id.cmp(&right.fence.lock_id));
            rewrite_fastpay_wal_compacted(store.data_dir(), &journal)?
        };

    if shadow {
        let wal_head = load_or_refresh_fastpay_wal_head(store.data_dir())?
            .ok_or_else(|| fastpay_invalid_data("FastPay WAL missing during shadow verify"))?;
        write_fastpay_speculative_journal(store.data_dir(), &wal_head.journal)?;
        let v1_readback = read_fastpay_speculative_journal_v1_file(&v1_path)?;
        let (v2_readback, _) = decode_fastpay_wal_file(&v2_path, false)?;
        if v1_readback != v2_readback {
            return Err(fastpay_invalid_data(
                "POSTFIAT_FASTPAY_WAL_SHADOW_VERIFY mismatch between v1.json and v2.wal",
            ));
        }
    } else if v1_path.exists() {
        let _ = std::fs::remove_file(&v1_path);
    }

    Ok(new_chain_hash)
}

/// Prune anchored/terminal speculative WAL records inside a transactional block
/// commit while `transactional` (`redb`) is already open under the ordered-commit
/// lock. This keeps post-block `owned_apply_v3` on the O(1) append-only WAL path
/// without re-opening `redb` or contending for the transactional writer lease.
pub(super) fn prune_fastpay_speculative_wal_after_block_commit(
    data_dir: &std::path::Path,
    transactional: &postfiat_storage::TransactionalStore,
    tip_height: u64,
    ledger: &LedgerState,
) -> io::Result<()> {
    let Some(head) = load_or_refresh_fastpay_wal_head(data_dir)? else {
        return Ok(());
    };
    if head.journal.effects.is_empty() {
        return Ok(());
    }
    let has_older_height_records = head.journal.effects.iter().any(|record| {
        record
            .retained_tip
            .as_ref()
            .map_or(true, |retained| retained.height != tip_height)
    });
    let has_ordered_terminal = head.journal.effects.iter().any(|record| {
        ledger.fastpay_version_fences.iter().any(|fence| {
            fence.lock_id == record.fence.lock_id
                && fence.origin == postfiat_types::FastPayFenceOriginV1::OrderedRecovery
        })
    });
    if !has_older_height_records && !has_ordered_terminal {
        return Ok(());
    }

    let mut anchored = BTreeSet::new();
    if has_older_height_records {
        for record in &head.journal.effects {
            if transactional
                .anchored_fastpay_effect(&record.fence.lock_id)?
                .is_some()
            {
                anchored.insert(record.fence.lock_id.clone());
            }
        }
    }

    let mut journal = head.journal;
    let before_len = journal.effects.len();
    journal.effects.retain(|record| {
        let recovery = record
            .fence
            .certificate
            .as_ref()
            .map(postfiat_types::FastPayCertificateV1::recovery);
        let ordered_terminal = ledger.fastpay_version_fences.iter().any(|fence| {
            fence.lock_id == record.fence.lock_id
                && fence.origin == postfiat_types::FastPayFenceOriginV1::OrderedRecovery
        });
        !anchored.contains(&record.fence.lock_id)
            && !ordered_terminal
            && (ledger.fastpay_version_fences.contains(&record.fence)
                || recovery.is_some_and(|window| tip_height < window.recovery_closes_at_height))
    });
    if journal.effects.len() == before_len {
        return Ok(());
    }
    journal
        .effects
        .sort_by(|left, right| left.fence.lock_id.cmp(&right.fence.lock_id));
    rewrite_fastpay_wal_compacted(data_dir, &journal)?;
    let shadow = fastpay_wal_shadow_verify_enabled();
    let v1_path = data_dir.join(FASTPAY_SPECULATIVE_JOURNAL_FILE);
    if shadow {
        write_fastpay_speculative_journal(data_dir, &journal)?;
    } else if v1_path.exists() {
        let _ = std::fs::remove_file(&v1_path);
    }
    Ok(())
}

fn fastpay_height(store: &NodeStore) -> io::Result<u64> {
    store
        .read_chain_tip()
        .map(|tip| tip.height)
        .or_else(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                Ok(store
                    .read_blocks()?
                    .blocks
                    .last()
                    .map_or(0, |block| block.header.height))
            } else {
                Err(error)
            }
        })
}

fn fastpay_recovery_policy(
    ledger: &LedgerState,
) -> io::Result<postfiat_types::FastPayRecoveryPolicyV1> {
    let policy = ledger.fastpay_recovery_policy.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "FastPay v3 recovery policy is not active",
        )
    })?;
    policy.validate().map_err(fastpay_invalid_data)?;
    Ok(policy)
}

fn fastpay_recovery_committee(
    ledger: &LedgerState,
    committee_epoch: u64,
    registry_root: &str,
) -> io::Result<postfiat_types::FastPayRecoveryCommitteeV1> {
    let committee = ledger
        .fastpay_recovery_committees
        .iter()
        .find(|committee| {
            committee.committee_epoch == committee_epoch && committee.registry_root == registry_root
        })
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "FastPay v3 recovery committee is not active",
            )
        })?;
    committee.validate().map_err(fastpay_invalid_data)?;
    Ok(committee)
}

fn active_fastpay_recovery_committee(
    ledger: &LedgerState,
    height: u64,
) -> io::Result<postfiat_types::FastPayRecoveryCommitteeV1> {
    let committee = ledger
        .fastpay_recovery_committees
        .iter()
        .filter(|committee| {
            committee.valid_from_height <= height && height <= committee.new_orders_through_height
        })
        .max_by_key(|committee| committee.committee_epoch)
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "no FastPay v3 committee admits new orders at the current height",
            )
        })?;
    committee.validate().map_err(fastpay_invalid_data)?;
    Ok(committee)
}

fn local_fastpay_signer_public_key(
    data_dir: &std::path::Path,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
    validator_id: &str,
) -> io::Result<Vec<u8>> {
    // The replicated epoch pins certificate authority. The current registry can
    // revoke this signer, but rotating another member must not revoke this one.
    let expected = committee
        .validator_public_keys()
        .into_iter()
        .find(|(id, _)| id == validator_id)
        .ok_or_else(|| fastpay_invalid_data("local signer is not in the FastPay committee"))?;
    if !load_validator_pubkeys(data_dir)?.contains(&expected) {
        return Err(fastpay_invalid_data(
            "local signer key does not match the replicated FastPay committee",
        ));
    }
    hex_to_bytes(&expected.1).map_err(invalid_data)
}

/// Signing needs this validator's registered key in the replicated committee.
/// A registered validator without that key (rotated or not a member) is a
/// holder: it verifies and journals certified effects so its own proposal
/// anchors them, but it never signs. Unknown validator IDs fail closed.
fn fastpay_local_signer_eligible(
    data_dir: &std::path::Path,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
    validator_id: &str,
) -> io::Result<bool> {
    let registered = load_validator_pubkeys(data_dir)?;
    if !registered.iter().any(|(id, _)| id == validator_id) {
        return Err(fastpay_invalid_data(
            "validator is not in the local validator registry",
        ));
    }
    Ok(committee
        .validator_public_keys()
        .into_iter()
        .any(|member| member.0 == validator_id && registered.contains(&member)))
}

/// The apply response, built before any durable write: a signed
/// acknowledgement from a signer, or an unsigned held-effect receipt that no
/// client can count toward FastPay finality.
fn fastpay_apply_response(
    options: &NodeOptions,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
    validator_id: &str,
    signer: bool,
    domain: postfiat_types::OwnedCertificateDomain,
    fence: &postfiat_types::FastPayVersionFenceV1,
) -> io::Result<String> {
    if signer {
        let acknowledgement =
            fastpay_ack_for_fence(options, committee, validator_id, domain, fence)?;
        return serde_json::to_string(&acknowledgement).map_err(invalid_data);
    }
    let postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
        certificate_digest, ..
    } = &fence.decision
    else {
        return Err(fastpay_invalid_data(
            "cancelled FastPay fence cannot be held as an effect",
        ));
    };
    serde_json::to_string(&serde_json::json!({
        "schema": FASTPAY_HELD_EFFECT_SCHEMA_V1,
        "validator_id": validator_id,
        "lock_id": fence.lock_id,
        "certificate_digest": certificate_digest,
        "signed": false,
    }))
    .map_err(invalid_data)
}

fn sign_for_fastpay_committee(
    data_dir: &std::path::Path,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
    validator_id: &str,
    message: &[u8],
    context: &[u8],
) -> io::Result<Vec<u8>> {
    let public_key = local_fastpay_signer_public_key(data_dir, committee, validator_id)?;
    let secret_key = load_owned_validator_secret_key(data_dir, validator_id)?;
    let signature = ml_dsa_65_sign_with_context(&secret_key, message, context)
        .map_err(|error| io::Error::other(format!("FastPay committee sign failed: {error}")))?;
    // Public metadata in validator_keys.json does not prove the private key's
    // identity. Verify the actual signature before reserving locks or applying.
    if !ml_dsa_65_verify_with_context(&public_key, message, &signature, context) {
        return Err(fastpay_invalid_data(
            "local signing key cannot sign for the replicated FastPay committee",
        ));
    }
    Ok(signature)
}

fn ensure_fastpay_unanchored_capacity(store: &NodeStore, ledger: &LedgerState) -> io::Result<()> {
    let pending = fastpay_pre_state_effects_for_next_block(store, ledger)?.len();
    if pending >= postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "FastPay unanchored-effect window is full; wait for the next certified block",
        ));
    }
    Ok(())
}

pub fn owned_certificate_domain_v3(
    data_dir: &std::path::Path,
) -> io::Result<postfiat_types::OwnedCertificateDomain> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let ledger = read_fastpay_ledger(&store)?;
    let committee = active_fastpay_recovery_committee(&ledger, fastpay_height(&store)?)?;
    let domain = committee.certificate_domain();
    if domain.chain_id != genesis.chain_id
        || domain.genesis_hash != postfiat_execution::genesis_hash(&genesis)
        || domain.protocol_version != genesis.protocol_version
    {
        return Err(fastpay_invalid_data(
            "replicated FastPay committee does not match local genesis",
        ));
    }
    Ok(domain)
}

fn fastpay_domain_for_committee(
    data_dir: &std::path::Path,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
) -> io::Result<postfiat_types::OwnedCertificateDomain> {
    let genesis = NodeStore::new(data_dir).read_genesis()?;
    let domain = committee.certificate_domain();
    if domain.chain_id != genesis.chain_id
        || domain.genesis_hash != postfiat_execution::genesis_hash(&genesis)
        || domain.protocol_version != genesis.protocol_version
    {
        return Err(fastpay_invalid_data(
            "replicated FastPay committee does not match local genesis",
        ));
    }
    Ok(domain)
}

pub fn owned_recovery_capabilities_v3(options: NodeOptions) -> io::Result<String> {
    let store = NodeStore::new(&options.data_dir);
    let read_lock = store.lock_ordered_commit()?;
    let report = owned_recovery_capabilities_v3_locked(&store, &read_lock)?;
    serde_json::to_string(&report).map_err(invalid_data)
}

/// `owned_recovery_capabilities_v3` for a caller that already holds the
/// ordered-commit lock (the lock is not re-entrant).
pub(super) fn owned_recovery_capabilities_v3_locked(
    store: &NodeStore,
    _commit_lock: &StorageMutationLock,
) -> io::Result<postfiat_types::FastPayRecoveryCapabilitiesV1> {
    let ledger = read_fastpay_ledger(store)?;
    ensure_fastpay_unanchored_capacity(store, &ledger)?;
    let height = fastpay_height(store)?;
    let committee = active_fastpay_recovery_committee(&ledger, height)?;
    let report = postfiat_types::FastPayRecoveryCapabilitiesV1 {
        schema: postfiat_types::FASTPAY_RECOVERY_CAPABILITIES_SCHEMA_V1.to_string(),
        domain: owned_certificate_domain_v3(store.data_dir())?,
        committee_epoch: committee.committee_epoch,
        current_height: height,
        validator_count: committee.validators.len(),
        quorum: committee.quorum,
        policy: fastpay_recovery_policy(&ledger)?,
    };
    report.validate().map_err(fastpay_invalid_data)?;
    Ok(report)
}

fn fastpay_domain_for_committee_with_genesis(
    genesis: &Genesis,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
) -> io::Result<postfiat_types::OwnedCertificateDomain> {
    let domain = committee.certificate_domain();
    if domain.chain_id != genesis.chain_id
        || domain.genesis_hash != postfiat_execution::genesis_hash(genesis)
        || domain.protocol_version != genesis.protocol_version
    {
        return Err(fastpay_invalid_data(
            "replicated FastPay committee does not match local genesis",
        ));
    }
    Ok(domain)
}

fn ensure_fastpay_unanchored_count(pending: usize) -> io::Result<()> {
    if pending >= postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "FastPay unanchored-effect window is full; wait for the next certified block",
        ));
    }
    Ok(())
}

pub fn owned_sign_v3(
    options: NodeOptions,
    signed_order_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let signed: postfiat_types::SignedOwnedTransferOrderV3 =
        serde_json::from_str(signed_order_json).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("signed FastPay v3 transfer parse failed: {error}"),
            )
        })?;
    let vote = owned_sign_v3_typed(options, signed, validator_id)?;
    serde_json::to_string(&vote).map_err(invalid_data)
}

pub fn owned_sign_v3_typed(
    options: NodeOptions,
    signed: postfiat_types::SignedOwnedTransferOrderV3,
    validator_id: &str,
) -> io::Result<postfiat_types::OwnedTransferVote> {
    begin_fastpay_rpc_metrics();
    let store = NodeStore::new(&options.data_dir);
    let _mutation_lock = store.lock_ordered_commit()?;
    let state = load_fastpay_state(&store, false)?;
    ensure_fastpay_unanchored_count(state.pending_unanchored_count)?;
    let height = state.height;
    let committee = fastpay_recovery_committee(
        &state.ledger,
        signed.order.recovery.committee_epoch,
        &signed.order.domain.registry_id,
    )?;
    if height < committee.valid_from_height
        || height > committee.new_orders_through_height
        || signed.order.recovery.valid_from_height > committee.new_orders_through_height
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FastPay v3 transfer is outside the committee admission window",
        ));
    }
    let committee_epoch = committee.committee_epoch;
    let domain = fastpay_domain_for_committee_with_genesis(&state.genesis, &committee)?;
    let policy = fastpay_recovery_policy(&state.ledger)?;
    postfiat_execution::validate_owned_transfer_v3_admission_with_fee_v1(
        &state.ledger,
        &signed,
        &domain,
        committee_epoch,
        &policy,
        height,
        state.fee_v1_active,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format_fastpay_owned_transfer_error("FastPay v3 transfer admission failed", error),
        )
    })?;
    let signing_bytes = postfiat_execution::owned_transfer_v3_signing_bytes(&signed.order);
    let order_digest = bytes_to_hex(&hash_bytes(
        FASTPAY_V3_TRANSFER_ORDER_HASH_DOMAIN,
        &signing_bytes,
    ));
    if order_digest != postfiat_execution::fastpay_transfer_order_digest_v3(&signed.order) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay v3 transfer order digest mismatch",
        ));
    }
    let signature = sign_for_fastpay_committee(
        &options.data_dir,
        &committee,
        validator_id,
        &signing_bytes,
        postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
    )?;
    reserve_owned_input_locks(
        &options.data_dir,
        &signed.order.inputs,
        &domain.registry_id,
        &signed.order.recovery.lock_id,
        "owned-sign-v3",
    )?;
    cache_fastpay_signed_order_v3(&options.data_dir, &signed);
    Ok(postfiat_types::OwnedTransferVote {
        validator_id: validator_id.to_string(),
        signature_hex: bytes_to_hex(&signature),
    })
}

pub fn owned_unwrap_sign_v3(
    options: NodeOptions,
    signed_order_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let signed: postfiat_types::SignedOwnedUnwrapOrderV3 = serde_json::from_str(signed_order_json)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("signed FastPay v3 unwrap parse failed: {error}"),
            )
        })?;
    begin_fastpay_rpc_metrics();
    let store = NodeStore::new(&options.data_dir);
    let _mutation_lock = store.lock_ordered_commit()?;
    let state = load_fastpay_state(&store, false)?;
    let height = state.height;
    let committee = fastpay_recovery_committee(
        &state.ledger,
        signed.order.recovery.committee_epoch,
        &signed.order.domain.registry_id,
    )?;
    if height < committee.valid_from_height
        || height > committee.new_orders_through_height
        || signed.order.recovery.valid_from_height > committee.new_orders_through_height
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FastPay v3 unwrap is outside the committee admission window",
        ));
    }
    let committee_epoch = committee.committee_epoch;
    let domain = fastpay_domain_for_committee_with_genesis(&state.genesis, &committee)?;
    let policy = fastpay_recovery_policy(&state.ledger)?;
    postfiat_execution::validate_owned_unwrap_v3_admission_with_fee_v1(
        &state.ledger,
        &signed,
        &domain,
        committee_epoch,
        &policy,
        height,
        state.fee_v1_active,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format_fastpay_owned_transfer_error("FastPay v3 unwrap admission failed", error),
        )
    })?;
    let signing_bytes = postfiat_execution::owned_unwrap_v3_signing_bytes(&signed.order);
    let order_digest = bytes_to_hex(&hash_bytes(
        FASTPAY_V3_UNWRAP_ORDER_HASH_DOMAIN,
        &signing_bytes,
    ));
    if order_digest != postfiat_execution::fastpay_unwrap_order_digest_v3(&signed.order) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay v3 unwrap order digest mismatch",
        ));
    }
    let signature = sign_for_fastpay_committee(
        &options.data_dir,
        &committee,
        validator_id,
        &signing_bytes,
        postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
    )?;
    reserve_owned_input_locks(
        &options.data_dir,
        &signed.order.inputs,
        &domain.registry_id,
        &signed.order.recovery.lock_id,
        "owned-unwrap-sign-v3",
    )?;
    serde_json::to_string(&postfiat_types::OwnedUnwrapVote {
        validator_id: validator_id.to_string(),
        signature_hex: bytes_to_hex(&signature),
    })
    .map_err(invalid_data)
}

fn fastpay_ack_for_fence(
    options: &NodeOptions,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
    validator_id: &str,
    domain: postfiat_types::OwnedCertificateDomain,
    fence: &postfiat_types::FastPayVersionFenceV1,
) -> io::Result<postfiat_types::FastPayApplyAckV1> {
    let (order_digest, certificate_digest) = match &fence.decision {
        postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
            order_digest,
            certificate_digest,
        } => (order_digest.clone(), certificate_digest.clone()),
        postfiat_types::FastPayRecoveryDecisionV1::Cancelled => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cancelled FastPay fence cannot produce an apply acknowledgement",
            ));
        }
    };
    let terminal_bytes = fence
        .state_commitment_bytes()
        .map_err(fastpay_invalid_data)?;
    let mut acknowledgement = postfiat_types::FastPayApplyAckV1 {
        schema: postfiat_types::FASTPAY_APPLY_ACK_SCHEMA_V1.to_string(),
        domain,
        committee_epoch: fence.committee_epoch,
        lock_id: fence.lock_id.clone(),
        order_digest,
        certificate_digest,
        terminal_state_digest: bytes_to_hex(&hash_bytes(
            "postfiat.fastpay.terminal-state.v1",
            &terminal_bytes,
        )),
        validator_id: validator_id.to_string(),
        signature_hex: String::new(),
    };
    let signing_bytes = postfiat_execution::fastpay_apply_ack_signing_bytes_v1(&acknowledgement)
        .map_err(|error| fastpay_invalid_data(format!("FastPay apply ack encoding: {error:?}")))?;
    acknowledgement.signature_hex = bytes_to_hex(&sign_for_fastpay_committee(
        &options.data_dir,
        committee,
        validator_id,
        &signing_bytes,
        postfiat_execution::FASTPAY_APPLY_ACK_CONTEXT_V1,
    )?);
    acknowledgement
        .validate_shape()
        .map_err(fastpay_invalid_data)?;
    Ok(acknowledgement)
}

fn matching_confirmed_fence<'a>(
    ledger: &'a LedgerState,
    lock_id: &str,
    certificate_digest: &str,
) -> Option<&'a postfiat_types::FastPayVersionFenceV1> {
    ledger.fastpay_version_fences.iter().find(|fence| {
        fence.lock_id == lock_id
            && matches!(
                &fence.decision,
                postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
                    certificate_digest: existing,
                    ..
                } if existing == certificate_digest
            )
    })
}

fn restore_fastpay_swap_removed_inputs(
    owned_objects: &mut Vec<postfiat_types::OwnedObject>,
    prior_objects: &[FastPayPriorObjectV1],
) -> io::Result<()> {
    for prior in prior_objects {
        let index = usize::try_from(prior.index).map_err(invalid_data)?;
        let prior_len = owned_objects
            .len()
            .checked_add(1)
            .ok_or_else(|| fastpay_invalid_data("FastPay rollback length overflow"))?;
        if index >= prior_len
            || owned_objects
                .iter()
                .any(|object| object.id == prior.object.id)
        {
            return Err(fastpay_invalid_data(
                "FastPay rollback input position or identity mismatch",
            ));
        }
        if index == owned_objects.len() {
            owned_objects.push(prior.object.clone());
        } else {
            let swapped_tail = owned_objects[index].clone();
            owned_objects.push(swapped_tail);
            owned_objects[index] = prior.object.clone();
        }
    }
    Ok(())
}

fn rollback_fastpay_speculative_effect(
    ledger: &mut LedgerState,
    record: &FastPaySpeculativeEffectV1,
) -> io::Result<()> {
    if ledger.fastpay_version_fences.last() != Some(&record.fence) {
        return Err(fastpay_invalid_data(
            "FastPay speculative rollback is not in reverse application order",
        ));
    }
    let certificate =
        record.fence.certificate.as_ref().ok_or_else(|| {
            fastpay_invalid_data("FastPay speculative rollback omitted certificate")
        })?;
    let input_refs = certificate.inputs();
    // The journal preserves ledger-position order for inverse swap-removal;
    // certificate inputs need not have that same order.
    let expected_inputs = input_refs
        .iter()
        .map(|input| (input.id.as_str(), input.version))
        .collect::<BTreeSet<_>>();
    let prior_inputs = record
        .prior_objects
        .iter()
        .map(|prior| (prior.object.id.as_str(), prior.object.version))
        .collect::<BTreeSet<_>>();
    if record.prior_objects.len() != input_refs.len()
        || expected_inputs.len() != input_refs.len()
        || prior_inputs.len() != record.prior_objects.len()
        || prior_inputs != expected_inputs
        || record
            .prior_objects
            .windows(2)
            .any(|pair| pair[0].index >= pair[1].index)
    {
        return Err(fastpay_invalid_data(
            "FastPay speculative rollback inputs do not match certificate",
        ));
    }

    let mut scratch = LedgerState::empty();
    scratch.owned_objects = record
        .prior_objects
        .iter()
        .map(|entry| entry.object.clone())
        .collect();
    match certificate {
        postfiat_types::FastPayCertificateV1::Transfer(certificate) => {
            if record.prior_unwrap_account.is_some() {
                return Err(fastpay_invalid_data(
                    "FastPay transfer rollback unexpectedly contains account state",
                ));
            }
            let order = postfiat_types::OwnedTransferOrder {
                domain: certificate.order.domain.clone(),
                inputs: certificate.order.inputs.clone(),
                outputs: certificate.order.outputs.clone(),
                fee: certificate.order.fee,
                nonce: certificate.order.nonce,
                memos: certificate.order.memos.clone(),
            };
            let outcome = postfiat_execution::apply_owned_transfer(
                &mut scratch,
                &order,
                &certificate.owner_pubkey_hex,
            )
            .map_err(|error| {
                fastpay_invalid_data(format!("FastPay transfer rollback model: {error:?}"))
            })?;
            if ledger.owned_objects.len() < outcome.created.len()
                || ledger.owned_objects[ledger.owned_objects.len() - outcome.created.len()..]
                    != outcome.created
            {
                return Err(fastpay_invalid_data(
                    "FastPay transfer rollback outputs were changed or reordered",
                ));
            }
            ledger
                .owned_objects
                .truncate(ledger.owned_objects.len() - outcome.created.len());
        }
        postfiat_types::FastPayCertificateV1::Unwrap(certificate) => {
            if let Some(account) = &record.prior_unwrap_account {
                scratch.accounts.push(account.clone());
            }
            let order = postfiat_types::OwnedUnwrapOrder {
                domain: certificate.order.domain.clone(),
                inputs: certificate.order.inputs.clone(),
                to_address: certificate.order.to_address.clone(),
                amount: certificate.order.amount,
                asset: certificate.order.asset.clone(),
                fee: certificate.order.fee,
                nonce: certificate.order.nonce,
                memos: certificate.order.memos.clone(),
            };
            let outcome = postfiat_execution::apply_owned_unwrap(
                &mut scratch,
                &order,
                &certificate.owner_pubkey_hex,
            )
            .map_err(|error| {
                fastpay_invalid_data(format!("FastPay unwrap rollback model: {error:?}"))
            })?;
            if let Some(change) = outcome.change_object {
                if ledger.owned_objects.last() != Some(&change) {
                    return Err(fastpay_invalid_data(
                        "FastPay unwrap rollback change object was changed or reordered",
                    ));
                }
                ledger.owned_objects.pop();
            }
            let expected_account = scratch
                .account(&certificate.order.to_address)
                .ok_or_else(|| fastpay_invalid_data("FastPay unwrap rollback account missing"))?;
            if ledger.account(&certificate.order.to_address) != Some(expected_account) {
                return Err(fastpay_invalid_data(
                    "FastPay unwrap rollback account was changed after speculative apply",
                ));
            }
            match &record.prior_unwrap_account {
                Some(prior) => {
                    *ledger
                        .account_mut(&certificate.order.to_address)
                        .ok_or_else(|| fastpay_invalid_data("FastPay unwrap account missing"))? =
                        prior.clone();
                }
                None => ledger
                    .accounts
                    .retain(|account| account.address != certificate.order.to_address),
            }
        }
    }
    restore_fastpay_swap_removed_inputs(&mut ledger.owned_objects, &record.prior_objects)?;
    ledger.fastpay_version_fences.pop();
    Ok(())
}

pub(super) fn rollback_unanchored_fastpay_effects_for_certified_block(
    store: &NodeStore,
    ledger: &mut LedgerState,
    local_unanchored: &[postfiat_types::FastPayVersionFenceV1],
) -> io::Result<()> {
    if local_unanchored.is_empty() {
        return Ok(());
    }
    let journal = read_fastpay_speculative_journal(store.data_dir())?;
    let local_locks = local_unanchored
        .iter()
        .map(|fence| fence.lock_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut records = ledger
        .fastpay_version_fences
        .iter()
        .rev()
        .filter(|fence| local_locks.contains(fence.lock_id.as_str()))
        .map(|fence| {
            journal
                .effects
                .iter()
                .find(|record| record.fence == *fence)
                .cloned()
                .ok_or_else(|| {
                    fastpay_invalid_data(
                        "certified FastPay rollback is missing its durable inverse journal",
                    )
                })
        })
        .collect::<io::Result<Vec<_>>>()?;
    if records.len() != local_unanchored.len() {
        return Err(fastpay_invalid_data(
            "certified FastPay rollback did not resolve every unanchored effect",
        ));
    }
    for record in records.drain(..) {
        rollback_fastpay_speculative_effect(ledger, &record)?;
    }
    invalidate_fastpay_ledger_cache(store.data_dir());
    Ok(())
}

fn fastpay_apply_outcome(
    options: &NodeOptions,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
    validator_id: &str,
    signer: bool,
    domain: postfiat_types::OwnedCertificateDomain,
    fence: &postfiat_types::FastPayVersionFenceV1,
) -> io::Result<FastPayApplyOutcomeV3> {
    if signer {
        let acknowledgement =
            fastpay_ack_for_fence(options, committee, validator_id, domain, fence)?;
        return Ok(FastPayApplyOutcomeV3::Ack(acknowledgement));
    }
    let postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
        certificate_digest, ..
    } = &fence.decision
    else {
        return Err(fastpay_invalid_data(
            "cancelled FastPay fence cannot be held as an effect",
        ));
    };
    Ok(FastPayApplyOutcomeV3::Held {
        validator_id: validator_id.to_string(),
        lock_id: fence.lock_id.clone(),
        certificate_digest: certificate_digest.clone(),
    })
}

pub fn owned_apply_v3(
    options: NodeOptions,
    cert_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let certificate: postfiat_types::OwnedTransferCertificateV3 = serde_json::from_str(cert_json)
        .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("FastPay v3 transfer certificate parse failed: {error}"),
        )
    })?;
    owned_apply_v3_typed(options, certificate, validator_id)?.to_json_string()
}

pub fn owned_apply_v3_fast_typed(
    options: NodeOptions,
    lock_id: &str,
    votes: Vec<postfiat_types::OwnedTransferVote>,
    validator_id: &str,
) -> io::Result<FastPayApplyOutcomeV3> {
    let Some(signed) = lookup_cached_fastpay_signed_order_v3(&options.data_dir, lock_id) else {
        begin_fastpay_rpc_metrics();
        update_fastpay_rpc_metrics(|m| {
            m.order_lru_hit = Some(false);
        });
        return Err(io::Error::new(io::ErrorKind::NotFound, "OrderNotCached"));
    };
    let certificate = postfiat_types::OwnedTransferCertificateV3 {
        order: signed.order,
        owner_pubkey_hex: signed.owner_pubkey_hex,
        owner_signature_hex: signed.owner_signature_hex,
        votes,
    };
    let outcome = owned_apply_v3_typed(options, certificate, validator_id)?;
    update_fastpay_rpc_metrics(|m| {
        m.order_lru_hit = Some(true);
    });
    Ok(outcome)
}

pub fn owned_apply_v3_typed(
    options: NodeOptions,
    certificate: postfiat_types::OwnedTransferCertificateV3,
    validator_id: &str,
) -> io::Result<FastPayApplyOutcomeV3> {
    let certificate_digest = postfiat_execution::fastpay_transfer_certificate_digest_v3(
        &certificate,
    )
    .map_err(|error| fastpay_invalid_data(format!("FastPay v3 certificate digest: {error:?}")))?;
    begin_fastpay_rpc_metrics();
    let store = NodeStore::new(&options.data_dir);
    let _mutation_lock = store.lock_ordered_commit()?;
    let state = load_fastpay_state(&store, true)?;
    let height = state.height;
    let committee = fastpay_recovery_committee(
        &state.ledger,
        certificate.order.recovery.committee_epoch,
        &certificate.order.domain.registry_id,
    )?;
    let signer = fastpay_local_signer_eligible(&options.data_dir, &committee, validator_id)?;
    if signer {
        local_fastpay_signer_public_key(&options.data_dir, &committee, validator_id)?;
    }
    let domain = fastpay_domain_for_committee_with_genesis(&state.genesis, &committee)?;
    let policy = fastpay_recovery_policy(&state.ledger)?;
    let response = if let Some(existing) = matching_confirmed_fence(
        &state.ledger,
        &certificate.order.recovery.lock_id,
        &certificate_digest,
    ) {
        let response =
            fastpay_apply_outcome(&options, &committee, validator_id, signer, domain, existing)?;
        if let Some(cache_key) = state.cache_key {
            store_cached_fastpay_state(CachedFastPayState {
                key: cache_key,
                ledger: state.ledger,
                genesis: state.genesis,
                pending_unanchored_count: state.pending_unanchored_count,
                fee_v1_active: state.fee_v1_active,
            });
        }
        response
    } else {
        ensure_fastpay_unanchored_count(state.pending_unanchored_count)?;
        let validator_pks = committee.validator_public_keys();
        let context = postfiat_execution::FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &domain,
            committee_epoch: committee.committee_epoch,
            policy: &policy,
            quorum: committee.quorum,
        };
        let mut next_ledger = std::sync::Arc::unwrap_or_clone(state.ledger);
        let prior_objects_opt =
            prior_objects_for_fastpay_inputs(&next_ledger, &certificate.order.inputs).ok();
        postfiat_execution::apply_owned_transfer_certificate_v3_with_fee_v1(
            &mut next_ledger,
            &certificate,
            context,
            height,
            state.fee_v1_active,
        )
        .map_err(|error| {
            io::Error::other(format_fastpay_owned_transfer_error(
                "FastPay v3 transfer apply failed",
                error,
            ))
        })?;
        let prior_objects = prior_objects_opt
            .ok_or_else(|| fastpay_invalid_data("FastPay speculative input is absent"))?;
        let fence = next_ledger
            .fastpay_version_fences
            .last()
            .cloned()
            .ok_or_else(|| fastpay_invalid_data("FastPay v3 transfer apply omitted its fence"))?;
        // Construct and verify the response before any durable writes.
        let response =
            fastpay_apply_outcome(&options, &committee, validator_id, signer, domain, &fence)?;
        let new_journal_hash = retain_fastpay_speculative_effect(
            &store,
            &next_ledger,
            FastPaySpeculativeEffectV1 {
                fence: fence.clone(),
                prior_objects,
                prior_unwrap_account: None,
                retained_tip: None,
            },
            state.cache_key.as_ref(),
        )?;
        if state.cache_key.is_none() && !store.transactional_storage_active()? {
            store.write_ledger(&next_ledger)?;
        } else if let Some(mut cache_key) = state.cache_key {
            cache_key.journal_bytes_hash = new_journal_hash;
            store_cached_fastpay_state(CachedFastPayState {
                key: cache_key,
                ledger: std::sync::Arc::new(next_ledger),
                genesis: state.genesis,
                pending_unanchored_count: state.pending_unanchored_count.saturating_add(1),
                fee_v1_active: state.fee_v1_active,
            });
        }
        response
    };
    Ok(response)
}

pub fn owned_unwrap_apply_v3(
    options: NodeOptions,
    cert_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let certificate: postfiat_types::OwnedUnwrapCertificateV3 = serde_json::from_str(cert_json)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("FastPay v3 unwrap certificate parse failed: {error}"),
            )
        })?;
    let certificate_digest = postfiat_execution::fastpay_unwrap_certificate_digest_v3(&certificate)
        .map_err(|error| {
            fastpay_invalid_data(format!("FastPay v3 certificate digest: {error:?}"))
        })?;
    begin_fastpay_rpc_metrics();
    let store = NodeStore::new(&options.data_dir);
    let _mutation_lock = store.lock_ordered_commit()?;
    let state = load_fastpay_state(&store, true)?;
    let height = state.height;
    let committee = fastpay_recovery_committee(
        &state.ledger,
        certificate.order.recovery.committee_epoch,
        &certificate.order.domain.registry_id,
    )?;
    let signer = fastpay_local_signer_eligible(&options.data_dir, &committee, validator_id)?;
    if signer {
        local_fastpay_signer_public_key(&options.data_dir, &committee, validator_id)?;
    }
    let domain = fastpay_domain_for_committee_with_genesis(&state.genesis, &committee)?;
    let policy = fastpay_recovery_policy(&state.ledger)?;
    let response = if let Some(existing) = matching_confirmed_fence(
        &state.ledger,
        &certificate.order.recovery.lock_id,
        &certificate_digest,
    ) {
        let response =
            fastpay_apply_response(&options, &committee, validator_id, signer, domain, existing)?;
        if let Some(cache_key) = state.cache_key {
            store_cached_fastpay_state(CachedFastPayState {
                key: cache_key,
                ledger: state.ledger,
                genesis: state.genesis,
                pending_unanchored_count: state.pending_unanchored_count,
                fee_v1_active: state.fee_v1_active,
            });
        }
        response
    } else {
        ensure_fastpay_unanchored_count(state.pending_unanchored_count)?;
        let validator_pks = committee.validator_public_keys();
        let context = postfiat_execution::FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &domain,
            committee_epoch: committee.committee_epoch,
            policy: &policy,
            quorum: committee.quorum,
        };
        let mut next_ledger = std::sync::Arc::unwrap_or_clone(state.ledger);
        let prior_objects_opt =
            prior_objects_for_fastpay_inputs(&next_ledger, &certificate.order.inputs).ok();
        let prior_unwrap_account = next_ledger.account(&certificate.order.to_address).cloned();
        postfiat_execution::apply_owned_unwrap_certificate_v3_with_fee_v1(
            &mut next_ledger,
            &certificate,
            context,
            height,
            state.fee_v1_active,
        )
        .map_err(|error| {
            io::Error::other(format_fastpay_owned_transfer_error(
                "FastPay v3 unwrap apply failed",
                error,
            ))
        })?;
        let prior_objects = prior_objects_opt
            .ok_or_else(|| fastpay_invalid_data("FastPay speculative input is absent"))?;
        let fence = next_ledger
            .fastpay_version_fences
            .last()
            .cloned()
            .ok_or_else(|| fastpay_invalid_data("FastPay v3 unwrap apply omitted its fence"))?;
        // Construct and verify the response before any durable writes.
        let response =
            fastpay_apply_response(&options, &committee, validator_id, signer, domain, &fence)?;
        let new_journal_hash = retain_fastpay_speculative_effect(
            &store,
            &next_ledger,
            FastPaySpeculativeEffectV1 {
                fence: fence.clone(),
                prior_objects,
                prior_unwrap_account,
                retained_tip: None,
            },
            state.cache_key.as_ref(),
        )?;
        if state.cache_key.is_none() && !store.transactional_storage_active()? {
            store.write_ledger(&next_ledger)?;
        } else if let Some(mut cache_key) = state.cache_key {
            cache_key.journal_bytes_hash = new_journal_hash;
            store_cached_fastpay_state(CachedFastPayState {
                key: cache_key,
                ledger: std::sync::Arc::new(next_ledger),
                genesis: state.genesis,
                pending_unanchored_count: state.pending_unanchored_count.saturating_add(1),
                fee_v1_active: state.fee_v1_active,
            });
        }
        response
    };
    Ok(response)
}

pub fn owned_certificate_v3(options: NodeOptions, selector: &str) -> io::Result<String> {
    validate_hex_string("FastPay certificate selector", selector, Some(96))?;
    let ledger = NodeStore::new(&options.data_dir).read_ledger()?;
    let mut certificate = ledger
        .fastpay_version_fences
        .iter()
        .find(|fence| {
            fence.lock_id == selector
                || matches!(
                    &fence.decision,
                    postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
                        certificate_digest,
                        ..
                    } if certificate_digest == selector
                )
        })
        .and_then(|fence| fence.certificate.clone())
        .or_else(|| {
            ledger
                .fastpay_recovery_reveals
                .iter()
                .find(|reveal| reveal.lock_id == selector || reveal.certificate_digest == selector)
                .map(|reveal| reveal.certificate.clone())
        });
    if certificate.is_none() {
        certificate = read_fastpay_speculative_journal(&options.data_dir)?
            .effects
            .into_iter()
            .find(|record| {
                record.fence.lock_id == selector
                    || matches!(
                        &record.fence.decision,
                        postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
                            certificate_digest,
                            ..
                        } if certificate_digest == selector
                    )
            })
            .and_then(|record| record.fence.certificate);
    }
    let certificate = certificate
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "FastPay certificate not found"))?;
    serde_json::to_string(&certificate).map_err(invalid_data)
}

pub fn owned_recovery_status_v3(options: NodeOptions, lock_id: &str) -> io::Result<String> {
    validate_hex_string("FastPay lock ID", lock_id, Some(96))?;
    let ledger = read_fastpay_ledger(&NodeStore::new(&options.data_dir))?;
    let fence = ledger
        .fastpay_version_fences
        .iter()
        .find(|fence| fence.lock_id == lock_id);
    let reveals = ledger
        .fastpay_recovery_reveals
        .iter()
        .filter(|reveal| reveal.lock_id == lock_id)
        .count();
    serde_json::to_string(&serde_json::json!({
        "schema": "postfiat-fastpay-recovery-status-v1",
        "lock_id": lock_id,
        "status": match fence.map(|value| &value.decision) {
            Some(postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. }) => "confirmed",
            Some(postfiat_types::FastPayRecoveryDecisionV1::Cancelled) => "cancelled",
            None if reveals > 0 => "certificate_revealed",
            None => "open_or_unknown",
        },
        "reveal_count": reveals,
        "fence": fence,
    }))
    .map_err(invalid_data)
}

pub const FASTPAY_FENCE_STATUS_SIGNER_UNAVAILABLE: &str = "fastpay_fence_status_signer_unavailable";
pub const FASTPAY_FENCE_STATUS_NOT_A_SIGNER: &str = "fastpay_fence_status_not_a_signer";
/// Distinct (data dir, validator, lock) answers kept per process. An entry is
/// served only while the certified tip it was signed at is still the tip and
/// the history base is unchanged: a prune or checkpoint restore at the same
/// tip changes the height this validator can show an effect anchored at.
const FASTPAY_FENCE_STATUS_CACHE_ENTRIES: usize = 4096;

type FastPayFenceStatusCache =
    BTreeMap<(PathBuf, String, String), (FenceStatusTipMark, postfiat_types::FastPayFenceStatusV2)>;

/// Certified tip height, its block hash and the retained-history base.
type FenceStatusTipMark = (u64, String, u64);

fn fastpay_fence_status_cache() -> &'static std::sync::Mutex<FastPayFenceStatusCache> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<FastPayFenceStatusCache>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

#[cfg(test)]
pub(super) fn clear_fastpay_fence_status_cache() {
    fastpay_fence_status_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

pub fn fastpay_fence_status_error_code(message: &str) -> Option<&'static str> {
    [
        FASTPAY_FENCE_STATUS_SIGNER_UNAVAILABLE,
        FASTPAY_FENCE_STATUS_NOT_A_SIGNER,
    ]
    .into_iter()
    .find(|code| message.contains(&format!("{code}: ")))
}

/// The certified fence for `lock_id` and the height of the certified block
/// that made it terminal. An ordered recovery (a cancellation, or a
/// confirmation of a revealed certificate) exists only by executing a
/// certified block, at its `decided_at_height`. A consensusless effect counts
/// only once a certified block's `fastpay_pre_state_effects` anchored it; a
/// speculative effect, applied or rolled back, never does.
fn certified_fastpay_fence(
    store: &NodeStore,
    ledger: &LedgerState,
    lock_id: &str,
) -> io::Result<Option<(postfiat_types::FastPayVersionFenceV1, u64)>> {
    let Some(fence) = ledger
        .fastpay_version_fences
        .iter()
        .find(|fence| fence.lock_id == lock_id)
    else {
        return Ok(None);
    };
    if fence.origin == postfiat_types::FastPayFenceOriginV1::OrderedRecovery {
        return Ok(Some((fence.clone(), fence.decided_at_height)));
    }
    let anchored = if store.transactional_storage_active()? {
        store
            .transactional_store()?
            .anchored_fastpay_effect_at(lock_id)?
    } else {
        // Legacy storage keeps speculative effects in the ledger file, so
        // the block log is the only certified evidence.
        let mut found = store.read_blocks()?.blocks.into_iter().find_map(|block| {
            let height = block.header.height;
            block
                .fastpay_pre_state_effects
                .into_iter()
                .find(|effect| effect.lock_id == lock_id)
                .map(|effect| (height, effect))
        });
        if found.is_none() {
            found = read_history_checkpoint_state_optional(store)?.and_then(|checkpoint| {
                let height = checkpoint.pruned_up_to_height;
                checkpoint
                    .ledger
                    .fastpay_version_fences
                    .into_iter()
                    .find(|effect| {
                        effect.lock_id == lock_id
                            && effect.origin == postfiat_types::FastPayFenceOriginV1::Consensusless
                    })
                    .map(|effect| (height, effect))
            });
        }
        found
    };
    match anchored {
        Some((height, effect)) if effect == *fence => Ok(Some((effect, height))),
        Some(_) => Err(fastpay_invalid_data(
            "anchored FastPay effect conflicts with the certified ledger",
        )),
        None => Ok(None),
    }
}

/// A fence status this validator signs from its certified state only: the
/// finalized ledger and the certified blocks at the certified tip, never the
/// speculative journal or the mempool. The caller chooses only the lock; the
/// validator is the serving node and every signed byte comes from that state.
///
/// The FastPay committee key is the validator key in
/// `<data-dir>/validator_keys.json`, read by `sign_for_fastpay_committee` in
/// this process. In socket mode (`rpc-serve --finality-signer-socket`, the
/// two-user shape) the key belongs to the signer process, which signs only
/// consensus and transport contexts, so this read refuses with
/// `fastpay_fence_status_signer_unavailable` instead of reading a key file.
///
/// Cost: under the ordered-commit lock, one ledger read and one anchor lookup
/// (one index read; a block-log scan on legacy storage); then, outside it,
/// one ML-DSA-65 signature and its self-check. A repeat for the same lock at
/// the same certified tip is served from a bounded per-process cache
/// (4,096 answers) without signing.
pub fn owned_fence_status_v3(
    options: NodeOptions,
    validator_id: &str,
    lock_id: &str,
) -> io::Result<postfiat_types::FastPayFenceStatusV2> {
    validate_hex_string("FastPay lock ID", lock_id, Some(96))?;
    crate::validator_signer::refuse_key_file_in_socket_mode(
        &options.data_dir.join(VALIDATOR_KEYS_FILE),
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{FASTPAY_FENCE_STATUS_SIGNER_UNAVAILABLE}: {error}"),
        )
    })?;
    let store = NodeStore::new(&options.data_dir);
    let commit_lock = store.lock_ordered_commit()?;
    let genesis = store.read_genesis()?;
    let tip = read_chain_tip_or_reconstruct_for_genesis_read_only(&store, &genesis)?;
    let cache_key = (
        std::fs::canonicalize(&options.data_dir).unwrap_or_else(|_| options.data_dir.clone()),
        validator_id.to_string(),
        lock_id.to_string(),
    );
    let tip_mark: FenceStatusTipMark =
        (tip.height, tip.block_hash.clone(), tip.history_base_height);
    if let Some((mark, status)) = fastpay_fence_status_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&cache_key)
    {
        if *mark == tip_mark {
            return Ok(status.clone());
        }
    }
    let ledger = store.read_ledger()?;
    let certified = certified_fastpay_fence(&store, &ledger, lock_id)?;
    let committee = match &certified {
        Some((fence, _)) => {
            fastpay_recovery_committee(&ledger, fence.committee_epoch, &fence.registry_root)?
        }
        None => active_fastpay_recovery_committee(&ledger, tip.height)?,
    };
    let domain = fastpay_domain_for_committee(&options.data_dir, &committee)?;
    if !fastpay_local_signer_eligible(&options.data_dir, &committee, validator_id)? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{FASTPAY_FENCE_STATUS_NOT_A_SIGNER}: validator {validator_id} holds no key of FastPay committee epoch {}",
                committee.committee_epoch
            ),
        ));
    }
    let mut status = postfiat_types::FastPayFenceStatusV2 {
        schema: postfiat_types::FASTPAY_FENCE_STATUS_SCHEMA_V2.to_string(),
        chain_id: domain.chain_id,
        genesis_hash: domain.genesis_hash,
        protocol_version: domain.protocol_version,
        committee_epoch: committee.committee_epoch,
        registry_root: committee.registry_root.clone(),
        lock_id: lock_id.to_string(),
        decision: None,
        operation: None,
        fence_digest: None,
        anchored_at_or_before_height: None,
        observed_height: tip.height,
        validator_id: validator_id.to_string(),
        signature_hex: String::new(),
    };
    if let Some((fence, height)) = certified {
        if height == 0 || height > tip.height {
            return Err(fastpay_invalid_data(
                "certified FastPay fence height is outside the certified chain",
            ));
        }
        let terminal_bytes = fence
            .state_commitment_bytes()
            .map_err(fastpay_invalid_data)?;
        status.fence_digest = Some(bytes_to_hex(&hash_bytes(
            "postfiat.fastpay.terminal-state.v1",
            &terminal_bytes,
        )));
        status.decision = Some(fence.decision);
        status.operation = Some(fence.operation);
        status.anchored_at_or_before_height = Some(height);
    }
    // Every signed byte is fixed; sign outside the lock so a flood of reads
    // does not hold up block commits.
    drop(commit_lock);
    let signing_bytes = status.signing_bytes().map_err(fastpay_invalid_data)?;
    status.signature_hex = bytes_to_hex(&sign_for_fastpay_committee(
        &options.data_dir,
        &committee,
        validator_id,
        &signing_bytes,
        postfiat_types::FASTPAY_FENCE_STATUS_CONTEXT_V2,
    )?);
    status.validate_shape().map_err(fastpay_invalid_data)?;
    let mut cache = fastpay_fence_status_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if cache.len() >= FASTPAY_FENCE_STATUS_CACHE_ENTRIES {
        cache.retain(|_, (mark, _)| *mark == tip_mark);
        if cache.len() >= FASTPAY_FENCE_STATUS_CACHE_ENTRIES {
            cache.clear();
        }
    }
    cache.insert(cache_key, (tip_mark, status.clone()));
    Ok(status)
}

#[cfg(test)]
mod burn5_recovery_tests {
    use super::*;

    #[test]
    fn burn5_multi_input_rollback_restores_ledger_order() {
        let keypair = ml_dsa_65_keygen_from_seed(&[71_u8; 32]);
        let owner = bytes_to_hex(&keypair.public_key);
        let object = postfiat_types::OwnedObject {
            id: "11".repeat(48),
            version: 0,
            owner_pubkey_hex: owner.clone(),
            asset: "PFT".to_string(),
            value: 1_000_000,
        };
        let mut ledger = LedgerState::empty();
        let mut other = object.clone();
        other.id = "22".repeat(48);
        ledger.owned_objects = vec![other.clone(), object.clone()];
        let inputs = vec![
            postfiat_types::OwnedObjectRef {
                id: object.id.clone(),
                version: 0,
            },
            postfiat_types::OwnedObjectRef {
                id: other.id.clone(),
                version: 0,
            },
        ];
        let mut spare_1 = object.clone();
        spare_1.id = "88".repeat(48);
        let mut spare_2 = object.clone();
        spare_2.id = "99".repeat(48);
        for objects in [
            vec![
                other.clone(),
                spare_1.clone(),
                object.clone(),
                spare_2.clone(),
            ],
            vec![
                object.clone(),
                spare_1.clone(),
                other.clone(),
                spare_2.clone(),
            ],
        ] {
            ledger.owned_objects = objects;
            let certificate = postfiat_types::OwnedTransferCertificateV3 {
                order: postfiat_types::OwnedTransferOrderV3 {
                    domain: postfiat_types::OwnedCertificateDomain {
                        schema: "postfiat-owned-certificate-domain-v1".to_string(),
                        chain_id: "burn5".to_string(),
                        genesis_hash: "33".repeat(48),
                        protocol_version: 1,
                        registry_id: "44".repeat(48),
                    },
                    inputs: inputs.clone(),
                    outputs: vec![serde_json::from_value(serde_json::json!({
                        "owner_pubkey_hex": owner, "asset": "PFT", "value": 1_999_999,
                    }))
                    .expect("output")],
                    fee: 1,
                    nonce: 1,
                    memos: Vec::new(),
                    recovery: postfiat_types::FastPayOrderRecoveryV1 {
                        committee_epoch: 1,
                        expires_at_height: 10,
                        lock_id: "55".repeat(48),
                        recovery_closes_at_height: 20,
                        valid_from_height: 1,
                        schema: "postfiat.fastpay.order-recovery.v1".to_string(),
                    },
                },
                owner_pubkey_hex: owner.clone(),
                owner_signature_hex: String::new(),
                votes: Vec::new(),
            };
            let fence = postfiat_types::FastPayVersionFenceV1 {
                certificate: Some(postfiat_types::FastPayCertificateV1::Transfer(
                    certificate.clone(),
                )),
                committee_epoch: 1,
                decided_at_height: 1,
                decision: postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
                    order_digest: "66".repeat(48),
                    certificate_digest: "77".repeat(48),
                },
                inputs: inputs.clone(),
                lock_id: "55".repeat(48),
                schema: "postfiat.fastpay.version-fence.v1".to_string(),
                registry_root: "44".repeat(48),
                next_versions: Vec::new(),
                operation: postfiat_types::FastPayOperationKindV1::Transfer,
                origin: postfiat_types::FastPayFenceOriginV1::Consensusless,
            };
            let record = FastPaySpeculativeEffectV1 {
                fence,
                prior_objects: prior_objects_for_fastpay_inputs(&ledger, &inputs).unwrap(),
                prior_unwrap_account: None,
                retained_tip: None,
            };
            let original = ledger.clone();
            let legacy = postfiat_types::OwnedTransferOrder {
                domain: certificate.order.domain.clone(),
                inputs: certificate.order.inputs.clone(),
                outputs: certificate.order.outputs.clone(),
                fee: certificate.order.fee,
                nonce: certificate.order.nonce,
                memos: certificate.order.memos.clone(),
            };
            postfiat_execution::apply_owned_transfer(&mut ledger, &legacy, &owner)
                .expect("apply model");
            ledger.fastpay_version_fences.push(record.fence.clone());
            for case in 0..4 {
                let mut invalid = record.clone();
                match case {
                    0 => invalid.prior_objects[0].object.version += 1,
                    1 => invalid.prior_objects[1].object = invalid.prior_objects[0].object.clone(),
                    2 => invalid.prior_objects.swap(0, 1),
                    _ => invalid.prior_objects[1].index = invalid.prior_objects[0].index,
                }
                let mut unchanged = ledger.clone();
                assert!(rollback_fastpay_speculative_effect(&mut unchanged, &invalid).is_err());
                assert_eq!(unchanged, ledger);
            }
            rollback_fastpay_speculative_effect(&mut ledger, &record).expect("rollback");
            assert_eq!(ledger, original);
        }
    }
}
