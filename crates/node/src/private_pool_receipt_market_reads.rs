//! Paged admission receipts and the market view under a private-pool
//! snapshot token. Receipts are this node's admission log (the gateway
//! reconciles across nodes); only records whose outcome is final at or
//! before the token's height are served, in a deterministic order, and a
//! cursor is bound to the token, the filters and the exact eligible set.

use super::asset_orchard_pricing_reads::{
    asset_orchard_pricing_claim_from_snapshot, asset_orchard_private_egress_binding_from_snapshot,
    read_finalized_pair_snapshot,
};
use super::private_pool_reads::{encode_cursor, parse_cursor, read_refusal, SnapshotToken};
use super::*;

pub const PRIVATE_POOL_RECEIPTS_SCHEMA: &str = "postfiat.private_pool_receipts.v1";
pub const PRIVATE_POOL_MARKET_SCHEMA: &str = "postfiat.private_pool_market.v1";
pub const PRIVATE_POOL_RECEIPTS_MAX_ITEMS: usize = 256;
const RECEIPT_SET_DOMAIN: &str = "postfiat.private_pool_receipts.set.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivatePoolReceiptsOptions {
    pub data_dir: PathBuf,
    pub snapshot_token: String,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
    pub address: Option<String>,
    pub session_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivatePoolMarketOptions {
    pub data_dir: PathBuf,
    pub snapshot_token: String,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub inclusion_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolReceiptRow {
    pub intent_id: String,
    pub action_hash: String,
    pub action_kind: String,
    pub batch_id: String,
    /// `accepted`, `rejected` or `abandoned` (by the operator; the same
    /// bytes may be resubmitted under the same intent).
    pub state: String,
    /// The inclusion height, or the received height for an atomic batch
    /// refused before inclusion; the page's sort key.
    pub final_height: u64,
    /// None for an atomic batch refused before inclusion.
    pub inclusion_height: Option<u64>,
    pub block_hash: Option<String>,
    pub transaction_ids: Vec<String>,
    pub code: String,
    pub fee_charged: u64,
    pub state_effects: String,
    /// Only for `abandoned`: the operator's reason and Unix time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abandon_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolReceiptsReport {
    pub schema: String,
    pub snapshot_token: String,
    /// `ok` or `refused`.
    pub status: String,
    pub refusal: Option<AssetOrchardReadRefusal>,
    pub address: Option<String>,
    pub session_scope: Option<String>,
    pub offset: u64,
    pub limit: u64,
    pub total: u64,
    /// Matching records not final at or before the token's height.
    pub pending: u64,
    pub item_count: u64,
    pub items: Option<Vec<PrivatePoolReceiptRow>>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolMarketRoute {
    pub route_id: String,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolMarketNav {
    pub nav_epoch: u64,
    pub reserve_packet_hash: String,
    pub finalized_at_height: u64,
    /// Blocks from the epoch's finalization to the inclusion height; None when
    /// the epoch carries no finalization height.
    pub age_blocks_at_inclusion: Option<u64>,
    pub halted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolMarketEnforcement {
    pub activation_height: Option<u64>,
    /// The stale, governed-pair and mode rules apply at the inclusion height.
    pub active_at_inclusion: bool,
    pub max_nav_age_blocks: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolMarketReport {
    pub schema: String,
    pub snapshot_token: String,
    /// `ok` or `refused`.
    pub status: String,
    pub refusal: Option<AssetOrchardReadRefusal>,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub inclusion_height: u64,
    pub governed_route: Option<PrivatePoolMarketRoute>,
    pub nav: Option<PrivatePoolMarketNav>,
    pub pricing_enforcement: Option<PrivatePoolMarketEnforcement>,
    pub pricing: Option<AssetOrchardPricingClaimReport>,
    pub egress_bindings: Option<Vec<AssetOrchardPrivateEgressBinding>>,
    pub egress_status: Option<String>,
}

/// The public address an admitted action names: the egress destination or
/// the ingress burn source. A swap names none.
fn record_address(record: &PrivateActionRecord) -> Option<String> {
    let action = serde_json::from_str::<serde_json::Value>(&record.action_json).ok()?;
    action
        .get("to")
        .or_else(|| action.pointer("/burn_transaction/unsigned/source"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn final_row(record: &PrivateActionRecord, token_height: u64) -> Option<PrivatePoolReceiptRow> {
    let outcome = record.outcome.as_ref()?;
    let state = match record.state {
        PrivateActionState::Accepted => "accepted",
        PrivateActionState::Rejected => "rejected",
        PrivateActionState::Abandoned => "abandoned",
        _ => return None,
    };
    let finalized_at = outcome
        .inclusion
        .as_ref()
        .map_or(record.received_height, |inclusion| inclusion.height);
    (finalized_at <= token_height).then(|| PrivatePoolReceiptRow {
        intent_id: record.intent_id.clone(),
        action_hash: record.action_hash.clone(),
        action_kind: record.action_kind.clone(),
        batch_id: record.batch_id.clone(),
        state: state.to_string(),
        final_height: finalized_at,
        inclusion_height: outcome.inclusion.as_ref().map(|inclusion| inclusion.height),
        block_hash: outcome
            .inclusion
            .as_ref()
            .map(|inclusion| inclusion.block_hash.clone()),
        transaction_ids: outcome
            .inclusion
            .as_ref()
            .map(|inclusion| inclusion.receipt_ids.clone())
            .unwrap_or_default(),
        code: outcome.code.clone(),
        fee_charged: outcome.fee_charged,
        state_effects: outcome.state_effects.clone(),
        abandon_reason: (record.state == PrivateActionState::Abandoned)
            .then(|| outcome.message.clone()),
    })
}

/// The admission records this node holds whose outcome is final at or before
/// the token's height, ordered by inclusion height (the received height for
/// a batch refused before inclusion) and then intent id.
pub fn private_pool_receipts(
    options: PrivatePoolReceiptsOptions,
) -> io::Result<PrivatePoolReceiptsReport> {
    if let Some(address) = options.address.as_deref() {
        if !postfiat_crypto_provider::is_canonical_address(address) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "address must be a canonical lowercase PFTL address",
            ));
        }
    }
    if let Some(scope) = options.session_scope.as_deref() {
        if scope.is_empty() || scope.len() > MAX_TEXT_FIELD_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "session_scope must be 1..=256 bytes",
            ));
        }
    }
    let limit = options
        .limit
        .unwrap_or(PRIVATE_POOL_RECEIPTS_MAX_ITEMS)
        .clamp(1, PRIVATE_POOL_RECEIPTS_MAX_ITEMS);
    let mut report = PrivatePoolReceiptsReport {
        schema: PRIVATE_POOL_RECEIPTS_SCHEMA.to_string(),
        snapshot_token: options.snapshot_token.clone(),
        status: "ok".to_string(),
        refusal: None,
        address: options.address.clone(),
        session_scope: options.session_scope.clone(),
        offset: 0,
        limit: limit as u64,
        total: 0,
        pending: 0,
        item_count: 0,
        items: None,
        next_cursor: None,
    };
    let refuse = |mut report: PrivatePoolReceiptsReport, code: &str, reason: String| {
        report.status = "refused".to_string();
        report.refusal = Some(read_refusal(code, reason));
        Ok(report)
    };
    let Some(token) = SnapshotToken::parse(&options.snapshot_token) else {
        return refuse(
            report,
            "snapshot_token_malformed",
            "snapshot token is not a private_pool_snapshot token".to_string(),
        );
    };
    let snapshot = read_finalized_pair_snapshot(&options.data_dir)?;
    if !token.matches(&snapshot) {
        let reason = format!(
            "token pins height {} but the committed tip is height {}; take a new snapshot",
            token.height, snapshot.tip.height
        );
        return refuse(report, "snapshot_unavailable", reason);
    }

    let store = PrivateActionAdmissionStore::open(&options.data_dir)?;
    let mut rows = Vec::new();
    let mut pending = 0u64;
    for record in store.records() {
        if options
            .session_scope
            .as_ref()
            .is_some_and(|scope| record.domain.session_scope != *scope)
            || options
                .address
                .as_ref()
                .is_some_and(|address| record_address(record).as_ref() != Some(address))
        {
            continue;
        }
        match final_row(record, token.height) {
            Some(row) => rows.push(row),
            None => pending += 1,
        }
    }
    drop(store);
    rows.sort_by(|left, right| {
        (left.final_height, &left.intent_id).cmp(&(right.final_height, &right.intent_id))
    });
    // The cursor binds the filters and the exact eligible set, so a page
    // never joins a different set even if the node records an outcome later.
    let set_digest = hash_hex(
        RECEIPT_SET_DOMAIN,
        format!(
            "{:?}\n{:?}\n{}",
            options.address,
            options.session_scope,
            rows.iter()
                .map(|row| row.intent_id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        )
        .as_bytes(),
    );
    let section = format!("receipts-{}", &set_digest[..16]);
    report.total = rows.len() as u64;
    report.pending = pending;
    let offset = match options.cursor.as_deref() {
        None => 0,
        Some(cursor) => match parse_cursor(cursor, &options.snapshot_token, &section) {
            Some(offset) => offset,
            None if cursor.starts_with("ppc1:receipts-") => {
                return refuse(
                    report,
                    "snapshot_unavailable",
                    "the receipt set or filters changed since this cursor was issued; take a new snapshot"
                        .to_string(),
                )
            }
            None => {
                return refuse(
                    report,
                    "cursor_malformed",
                    "cursor was not issued for this snapshot token and read".to_string(),
                )
            }
        },
    };
    report.offset = offset as u64;
    if options.cursor.is_some() && offset >= rows.len() {
        let reason = format!(
            "cursor offset {offset} is past the end of {} receipts",
            rows.len()
        );
        return refuse(report, "cursor_out_of_range", reason);
    }
    let end = offset.saturating_add(limit).min(rows.len());
    report.item_count = (end - offset) as u64;
    report.items = Some(rows[offset..end].to_vec());
    report.next_cursor =
        (end < rows.len()).then(|| encode_cursor(&options.snapshot_token, &section, end));
    Ok(report)
}

/// The pair's market state at the inclusion height, from the token's state.
pub fn private_pool_market(
    options: PrivatePoolMarketOptions,
) -> io::Result<PrivatePoolMarketReport> {
    let mut report = PrivatePoolMarketReport {
        schema: PRIVATE_POOL_MARKET_SCHEMA.to_string(),
        snapshot_token: options.snapshot_token.clone(),
        status: "ok".to_string(),
        refusal: None,
        base_asset_id: options.base_asset_id.clone(),
        quote_asset_id: options.quote_asset_id.clone(),
        inclusion_height: 0,
        governed_route: None,
        nav: None,
        pricing_enforcement: None,
        pricing: None,
        egress_bindings: None,
        egress_status: None,
    };
    let refuse = |mut report: PrivatePoolMarketReport, code: &str, reason: String| {
        report.status = "refused".to_string();
        report.refusal = Some(read_refusal(code, reason));
        Ok(report)
    };
    let Some(token) = SnapshotToken::parse(&options.snapshot_token) else {
        return refuse(
            report,
            "snapshot_token_malformed",
            "snapshot token is not a private_pool_snapshot token".to_string(),
        );
    };
    let snapshot = read_finalized_pair_snapshot(&options.data_dir)?;
    if !token.matches(&snapshot) {
        let reason = format!(
            "token pins height {} but the committed tip is height {}; take a new snapshot",
            token.height, snapshot.tip.height
        );
        return refuse(report, "snapshot_unavailable", reason);
    }
    let inclusion_height = options
        .inclusion_height
        .unwrap_or_else(|| token.height.saturating_add(1));
    report.inclusion_height = inclusion_height;
    let pair = AssetOrchardPairReadOptions {
        data_dir: options.data_dir.clone(),
        base_asset_id: options.base_asset_id.clone(),
        quote_asset_id: options.quote_asset_id.clone(),
        inclusion_height,
    };
    let pricing = asset_orchard_pricing_claim_from_snapshot(&snapshot, &pair)?;
    let egress = asset_orchard_private_egress_binding_from_snapshot(&snapshot, &pair)?;
    let compatibility = asset_execution_compatibility_for_genesis_and_governance(
        &snapshot.genesis,
        &snapshot.governance,
    );
    report.governed_route = snapshot
        .ledger
        .pftl_uniswap_routes
        .iter()
        .find(|route| {
            route.native_nav_asset_id == options.base_asset_id
                && route.settlement_asset_id == options.quote_asset_id
        })
        .map(|route| PrivatePoolMarketRoute {
            route_id: route.route_id.clone(),
            paused: route.paused,
        });
    report.nav = snapshot
        .ledger
        .nav_assets
        .iter()
        .find(|asset| asset.asset_id == options.base_asset_id)
        .map(|asset| PrivatePoolMarketNav {
            nav_epoch: asset.finalized_epoch,
            reserve_packet_hash: asset.finalized_reserve_packet_hash.clone(),
            finalized_at_height: asset.finalized_at_height,
            age_blocks_at_inclusion: (asset.finalized_at_height != 0)
                .then(|| inclusion_height.saturating_sub(asset.finalized_at_height)),
            halted: asset.halted,
        });
    report.pricing_enforcement = Some(PrivatePoolMarketEnforcement {
        activation_height: compatibility.asset_orchard_pricing_enforcement_activation_height,
        active_at_inclusion: compatibility.asset_orchard_pricing_enforced(inclusion_height),
        max_nav_age_blocks: compatibility.asset_orchard_pricing_max_nav_age_blocks,
    });
    report.egress_status = Some(egress.status);
    report.egress_bindings = Some(egress.bindings);
    report.pricing = Some(pricing);
    Ok(report)
}
