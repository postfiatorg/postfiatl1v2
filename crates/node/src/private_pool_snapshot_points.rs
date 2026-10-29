//! The last K finalized private-pool snapshot points, so a snapshot token
//! outlives new blocks. Commitments, nullifiers, retained roots and
//! ciphertexts only grow, so a point keeps their lengths and a page under an
//! older token is the current list truncated to them. Asset rows change in
//! place, so a point keeps them whole. The ring is a node-local cache in the
//! data directory, rewritten atomically after each durable commit and never
//! consensus state; a failed write never fails the commit.

use super::private_pool_reads::{
    asset_row, read_refusal, retained_anchor_rows, PrivatePoolAnchorRow, PrivatePoolAssetRow,
    PrivatePoolCounts, SnapshotToken,
};
use super::*;

pub const PRIVATE_POOL_SNAPSHOT_POINTS_FILE: &str = "private_pool_snapshot_points.json";
pub const PRIVATE_POOL_SNAPSHOT_POINTS_ENV: &str = "POSTFIAT_PRIVATE_POOL_SNAPSHOT_POINTS";
pub const PRIVATE_POOL_SNAPSHOT_POINTS_DEFAULT: usize = 16;
pub const PRIVATE_POOL_SNAPSHOT_POINTS_MAX: usize = 1024;
const SNAPSHOT_POINTS_SCHEMA: &str = "postfiat.private_pool_snapshot_points.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PrivatePoolSnapshotPoint {
    pub(super) height: u64,
    pub(super) block_hash: String,
    pub(super) state_root: String,
    /// The Orchard pool's id at this point; None before the pool exists.
    pub(super) pool_id: Option<String>,
    pub(super) counts: PrivatePoolCounts,
    pub(super) root_history: u64,
    /// The whole-tree root served while `root_history` is still empty.
    pub(super) fallback_anchor: Option<PrivatePoolAnchorRow>,
    pub(super) encrypted_outputs: u64,
    pub(super) asset_orchard_outputs: u64,
    pub(super) assets: Vec<PrivatePoolAssetRow>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SnapshotPointsFile {
    schema: String,
    points: Vec<PrivatePoolSnapshotPoint>,
}

/// K from `POSTFIAT_PRIVATE_POOL_SNAPSHOT_POINTS` (1..=1024), default 16.
pub fn private_pool_snapshot_points_limit() -> usize {
    std::env::var(PRIVATE_POOL_SNAPSHOT_POINTS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|limit| (1..=PRIVATE_POOL_SNAPSHOT_POINTS_MAX).contains(limit))
        .unwrap_or(PRIVATE_POOL_SNAPSHOT_POINTS_DEFAULT)
}

fn points_path(data_dir: &Path) -> PathBuf {
    data_dir.join(PRIVATE_POOL_SNAPSHOT_POINTS_FILE)
}

/// A missing or unreadable ring is an empty ring: older tokens are refused.
fn read_points(data_dir: &Path) -> Vec<PrivatePoolSnapshotPoint> {
    std::fs::read(points_path(data_dir))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SnapshotPointsFile>(&bytes).ok())
        .filter(|file| file.schema == SNAPSHOT_POINTS_SCHEMA)
        .map(|file| file.points)
        .unwrap_or_default()
}

fn length(len: usize) -> u64 {
    len as u64
}

pub(super) fn snapshot_point(
    tip: &ChainTipState,
    ledger: &LedgerState,
    shielded: &ShieldedState,
) -> io::Result<PrivatePoolSnapshotPoint> {
    let mut point = PrivatePoolSnapshotPoint {
        height: tip.height,
        block_hash: tip.block_hash.clone(),
        state_root: tip.state_root.clone(),
        pool_id: None,
        counts: PrivatePoolCounts {
            commitments: 0,
            nullifiers: 0,
            accepted_anchors: 0,
            outputs: 0,
            assets: 0,
        },
        root_history: 0,
        fallback_anchor: None,
        encrypted_outputs: 0,
        asset_orchard_outputs: 0,
        assets: Vec::new(),
    };
    let Some(pool) = shielded.orchard.as_ref() else {
        return Ok(point);
    };
    let fallback_anchor = if pool.root_history.is_empty() {
        retained_anchor_rows(pool)?.into_iter().next()
    } else {
        None
    };
    let mut assets = pool
        .asset_orchard_balances
        .iter()
        .map(|balance| {
            asset_row(
                ledger,
                &balance.asset_id,
                balance.ingress_total,
                balance.egress_total,
                balance.live_total,
            )
        })
        .collect::<Vec<_>>();
    assets.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    point.pool_id = Some(pool.pool_id.clone());
    point.counts = PrivatePoolCounts {
        commitments: length(pool.output_commitments.len()),
        nullifiers: length(pool.nullifiers.len()),
        accepted_anchors: if fallback_anchor.is_some() {
            1
        } else {
            length(pool.root_history.len())
        },
        outputs: length(pool.output_commitments.len()),
        assets: length(assets.len()),
    };
    point.root_history = length(pool.root_history.len());
    point.fallback_anchor = fallback_anchor;
    point.encrypted_outputs = length(pool.encrypted_outputs.len());
    point.asset_orchard_outputs = length(pool.asset_orchard_outputs.len());
    point.assets = assets;
    Ok(point)
}

/// Adds `point`, replacing any point at or above its height, and keeps the
/// last `limit` heights.
pub(super) fn record_snapshot_point(
    data_dir: &Path,
    point: PrivatePoolSnapshotPoint,
    limit: usize,
) -> io::Result<()> {
    let mut points = read_points(data_dir);
    points.retain(|kept| kept.height < point.height && kept.height + limit as u64 > point.height);
    points.push(point);
    let bytes = serde_json::to_vec(&SnapshotPointsFile {
        schema: SNAPSHOT_POINTS_SCHEMA.to_string(),
        points,
    })
    .map_err(invalid_data)?;
    postfiat_storage::atomic_write(points_path(data_dir), bytes)
}

/// Records the committed tip's point. The common block that leaves the
/// shielded state untouched reuses the previous point's lengths instead of
/// reading the pool.
pub(super) fn record_snapshot_point_after_durable_commit(
    store: &NodeStore,
    genesis: &Genesis,
    journal: &OrderedCommitDeltaJournal,
) {
    let result = (|| -> io::Result<()> {
        let tip = super::storage_commit::read_chain_tip_or_reconstruct_for_genesis_read_only(
            store, genesis,
        )?;
        if tip.height != journal.height {
            return Ok(());
        }
        let previous = read_points(store.data_dir())
            .into_iter()
            .find(|point| point.height + 1 == tip.height);
        let point = match (previous, journal.shielded.as_ref()) {
            (Some(mut point), None) => {
                if let Some(ledger) = journal.ledger.as_ref() {
                    for row in &mut point.assets {
                        *row = asset_row(
                            ledger,
                            &row.asset_id,
                            row.ingress_total,
                            row.egress_total,
                            row.live_total,
                        );
                    }
                }
                point.height = tip.height;
                point.block_hash = tip.block_hash.clone();
                point.state_root = tip.state_root.clone();
                point
            }
            _ => {
                let ledger = match journal.ledger.as_ref() {
                    Some(ledger) => ledger.clone(),
                    None => store.read_ledger()?,
                };
                let shielded = match journal.shielded.as_ref() {
                    Some(shielded) => shielded.clone(),
                    None => store.read_shielded()?,
                };
                snapshot_point(&tip, &ledger, &shielded)?
            }
        };
        record_snapshot_point(
            store.data_dir(),
            point,
            private_pool_snapshot_points_limit(),
        )
    })();
    if let Err(error) = result {
        eprintln!(
            "private pool snapshot point left behind at height {}: {error}",
            journal.height
        );
    }
}

/// How a token resolves against the committed tip: the tip itself (None) or
/// a retained older point. A height above the tip, or a height the ring holds
/// under another hash or root, was never a finalized point here; a height
/// K or more blocks below the tip has left the ring.
pub(super) fn resolve_snapshot_token(
    data_dir: &Path,
    token: &SnapshotToken,
    tip: &ChainTipState,
) -> Result<Option<PrivatePoolSnapshotPoint>, AssetOrchardReadRefusal> {
    if token.height == tip.height
        && token.block_hash == tip.block_hash
        && token.state_root == tip.state_root
    {
        return Ok(None);
    }
    let limit = private_pool_snapshot_points_limit() as u64;
    if token.height < tip.height && tip.height - token.height >= limit {
        return Err(read_refusal(
            "snapshot_expired",
            format!(
                "token pins height {} but this node keeps the last {limit} finalized points below tip {}; take a new snapshot",
                token.height, tip.height
            ),
        ));
    }
    if token.height < tip.height {
        if let Some(point) = read_points(data_dir).into_iter().find(|point| {
            point.height == token.height
                && point.block_hash == token.block_hash
                && point.state_root == token.state_root
        }) {
            return Ok(Some(point));
        }
    }
    Err(read_refusal(
        "snapshot_unavailable",
        format!(
            "token pins height {} but the committed tip is height {}; take a new snapshot",
            token.height, tip.height
        ),
    ))
}
