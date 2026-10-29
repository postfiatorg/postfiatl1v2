//! Finalized public reads a wallet needs to recover its shielded notes: a
//! snapshot token pinning one committed chain tip, deterministic pages of the
//! pool's public sets, and the account's asset lines. Every page names the
//! token; a page is served only while that token is the committed tip, so a
//! wallet never stitches pages from two states. The node never receives a
//! viewing key: it serves ciphertexts and the wallet scans locally.

use super::asset_orchard_pricing_reads::{
    asset_orchard_pricing_claim_from_snapshot, asset_orchard_private_egress_binding_from_snapshot,
    read_finalized_pair_snapshot, FinalizedPairSnapshot,
};
use super::*;

pub const PRIVATE_POOL_SNAPSHOT_SCHEMA: &str = "postfiat.private_pool_snapshot.v1";
pub const PRIVATE_POOL_PAGE_SCHEMA: &str = "postfiat.private_pool_page.v1";
pub const PRIVATE_ACCOUNT_LINES_SCHEMA: &str = "postfiat.private_account_lines.v1";
const SNAPSHOT_TOKEN_PREFIX: &str = "pps1";
const CURSOR_PREFIX: &str = "ppc1";
const CURSOR_DOMAIN: &str = "postfiat.private_pool_page.cursor.v1";

/// Server page maxima, below the wallet's current client ceilings (4096
/// commitments, nullifiers or anchors; 64 ciphertexts and 2 MiB of them;
/// 16 assets; a 4 MiB response).
pub const PRIVATE_POOL_PAGE_MAX_ITEMS: usize = 512;
pub const PRIVATE_POOL_PAGE_MAX_OUTPUTS: usize = 64;
pub const PRIVATE_POOL_PAGE_MAX_CIPHERTEXT_BYTES: usize = 1024 * 1024;
pub const PRIVATE_POOL_PAGE_MAX_ASSETS: usize = 16;
pub const PRIVATE_ACCOUNT_LINES_MAX: usize = 512;

pub const PRIVATE_POOL_SECTIONS: [&str; 5] = [
    "commitments",
    "nullifiers",
    "accepted_anchors",
    "outputs",
    "assets",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivatePoolSnapshotOptions {
    pub data_dir: PathBuf,
    pub pool_id: String,
    pub address: Option<String>,
    pub base_asset_id: Option<String>,
    pub quote_asset_id: Option<String>,
    /// Defaults to the block after the finalized tip.
    pub inclusion_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivatePoolPageOptions {
    pub data_dir: PathBuf,
    pub snapshot_token: String,
    pub section: String,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateAccountLinesOptions {
    pub data_dir: PathBuf,
    pub snapshot_token: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolCounts {
    pub commitments: u64,
    pub nullifiers: u64,
    pub accepted_anchors: u64,
    pub outputs: u64,
    pub assets: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolPageLimits {
    pub items: u64,
    pub outputs: u64,
    pub output_ciphertext_bytes: u64,
    pub assets: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolCompleteness {
    /// No pool or no output yet, and no ingress recorded: a legitimate empty pool.
    pub empty_pool_before_ingress: bool,
    /// Recorded ingress or spends are matched by retained commitments.
    pub history_complete: bool,
    /// Every commitment has its retained ciphertext.
    pub outputs_complete: bool,
    /// Anchors are the roots of retained tree prefixes, strictly ordered and
    /// ending at the current commitment count.
    pub anchors_from_retained_prefixes: bool,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolSnapshotReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub address: Option<String>,
    pub snapshot_token: String,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
    pub finalized_state_root: String,
    pub pool_id: String,
    pub pool_present: bool,
    pub counts: PrivatePoolCounts,
    pub page_limits: PrivatePoolPageLimits,
    pub base_asset_id: Option<String>,
    pub quote_asset_id: Option<String>,
    pub inclusion_height: u64,
    /// The `asset_orchard_pricing_claim` status for the pair at
    /// `inclusion_height`, or `unavailable` when no pair is known.
    pub pricing_status: String,
    pub pricing_refusal: Option<AssetOrchardReadRefusal>,
    pub pricing_claim: Option<AssetOrchardPricingClaim>,
    pub pricing_claim_json: Option<String>,
    /// The `asset_orchard_private_egress_binding` status for the pair.
    pub egress_status: String,
    pub policy_id: Option<String>,
    pub disclosure_hash: Option<String>,
    pub completeness: PrivatePoolCompleteness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolAnchorRow {
    pub root: String,
    pub output_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolOutputRow {
    pub position: u64,
    pub commitment: String,
    /// `asset_orchard`, `orchard`, or `missing` when no ciphertext is retained.
    pub kind: String,
    pub ciphertext: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_ciphertext: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolAssetRow {
    pub asset_id: String,
    pub ingress_total: u64,
    pub egress_total: u64,
    pub live_total: u64,
    pub asset_info_bound: bool,
    pub issuer: Option<String>,
    pub code: Option<String>,
    pub precision: Option<u8>,
    pub requires_authorization: Option<bool>,
    /// The governing NAV proof profile when the asset is NAV-tracked.
    pub proof_profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PrivatePoolPageItems {
    Strings(Vec<String>),
    Anchors(Vec<PrivatePoolAnchorRow>),
    Outputs(Vec<PrivatePoolOutputRow>),
    Assets(Vec<PrivatePoolAssetRow>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivatePoolPageReport {
    pub schema: String,
    pub snapshot_token: String,
    pub section: String,
    /// `ok` or `refused`.
    pub status: String,
    pub refusal: Option<AssetOrchardReadRefusal>,
    pub offset: u64,
    pub limit: u64,
    pub total: u64,
    pub item_count: u64,
    pub items: Option<PrivatePoolPageItems>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateAccountLineRow {
    pub trustline_id: String,
    pub asset_id: String,
    pub issuer: String,
    pub code: String,
    pub precision: u8,
    pub requires_authorization: bool,
    pub balance: u64,
    pub limit: u64,
    pub authorized: bool,
    pub frozen: bool,
    /// `ok`, `frozen`, `unauthorized` or `issuer_mismatch`.
    pub line_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateAccountLinesReport {
    pub schema: String,
    pub snapshot_token: String,
    pub address: String,
    /// `ok` or `refused`.
    pub status: String,
    pub refusal: Option<AssetOrchardReadRefusal>,
    pub line_count: u64,
    pub lines: Vec<PrivateAccountLineRow>,
    pub positive_asset_count: u64,
    /// The account's positive `account_assets` rows equal its positive lines.
    pub account_assets_consistent: bool,
    /// No line is frozen, unauthorized or bound to a different issuer.
    pub all_lines_usable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SnapshotToken {
    pub(super) pool_id: String,
    pub(super) height: u64,
    pub(super) block_hash: String,
    pub(super) state_root: String,
}

impl SnapshotToken {
    fn of(snapshot: &FinalizedPairSnapshot, pool_id: &str) -> Self {
        Self {
            pool_id: pool_id.to_string(),
            height: snapshot.tip.height,
            block_hash: snapshot.tip.block_hash.clone(),
            state_root: snapshot.tip.state_root.clone(),
        }
    }

    fn encode(&self) -> String {
        format!(
            "{SNAPSHOT_TOKEN_PREFIX}:{}:{}:{}:{}",
            self.pool_id, self.height, self.block_hash, self.state_root
        )
    }

    pub(super) fn parse(token: &str) -> Option<Self> {
        let mut parts = token.split(':');
        let (prefix, pool_id, height, block_hash, state_root) = (
            parts.next()?,
            parts.next()?,
            parts.next()?,
            parts.next()?,
            parts.next()?,
        );
        let lower_hex = |value: &str| {
            !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        let canonical_height = height.parse::<u64>().ok()?;
        (parts.next().is_none()
            && prefix == SNAPSHOT_TOKEN_PREFIX
            && valid_pool_id(pool_id)
            && canonical_height.to_string() == height
            && (lower_hex(block_hash) || block_hash == "genesis")
            && lower_hex(state_root))
        .then(|| Self {
            pool_id: pool_id.to_string(),
            height: canonical_height,
            block_hash: block_hash.to_string(),
            state_root: state_root.to_string(),
        })
    }

    pub(super) fn matches(&self, snapshot: &FinalizedPairSnapshot) -> bool {
        self.height == snapshot.tip.height
            && self.block_hash == snapshot.tip.block_hash
            && self.state_root == snapshot.tip.state_root
    }
}

fn valid_pool_id(pool_id: &str) -> bool {
    !pool_id.is_empty()
        && pool_id.len() <= 64
        && pool_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

pub(super) fn read_refusal(code: &str, reason: impl Into<String>) -> AssetOrchardReadRefusal {
    AssetOrchardReadRefusal {
        code: code.to_string(),
        reason: reason.into(),
    }
}

fn validate_address(address: &str) -> io::Result<()> {
    if postfiat_crypto_provider::is_canonical_address(address) {
        Ok(())
    } else {
        Err(invalid_input(
            "address must be a canonical lowercase PFTL address",
        ))
    }
}

fn snapshot_pool<'a>(
    snapshot: &'a FinalizedPairSnapshot,
    pool_id: &str,
) -> Option<&'a OrchardPoolState> {
    snapshot
        .shielded
        .orchard
        .as_ref()
        .filter(|pool| pool.pool_id == pool_id)
}

/// The anchors consensus accepts for a spend in `pool`: the roots of the
/// retained tree prefixes in `root_history`, or, before any root is
/// recorded, the root of the whole current tree (the empty root for an empty
/// pool). This mirrors `orchard_anchor_is_retained_for_apply`. The pool's
/// `accepted_anchors` list records anchors earlier spends used and is not
/// served.
fn retained_anchor_rows(pool: &OrchardPoolState) -> io::Result<Vec<PrivatePoolAnchorRow>> {
    if !pool.root_history.is_empty() {
        return Ok(pool
            .root_history
            .iter()
            .map(|record| PrivatePoolAnchorRow {
                root: record.root.clone(),
                output_count: record.output_count,
            })
            .collect());
    }
    let root = if pool.output_commitments.is_empty() {
        orchard_empty_root_hex()
    } else {
        orchard_pool_current_root(pool)?
    };
    Ok(vec![PrivatePoolAnchorRow {
        root,
        output_count: pool.output_commitments.len() as u64,
    }])
}

fn output_rows(pool: &OrchardPoolState) -> Vec<PrivatePoolOutputRow> {
    let asset_outputs = pool
        .asset_orchard_outputs
        .iter()
        .map(|output| (output.output_commitment.as_str(), output))
        .collect::<BTreeMap<_, _>>();
    let orchard_outputs = pool
        .encrypted_outputs
        .iter()
        .map(|output| (output.cmx.as_str(), output))
        .collect::<BTreeMap<_, _>>();
    pool.output_commitments
        .iter()
        .enumerate()
        .map(|(position, commitment)| {
            let position = position as u64;
            if let Some(output) = asset_outputs.get(commitment.as_str()) {
                PrivatePoolOutputRow {
                    position,
                    commitment: commitment.clone(),
                    kind: "asset_orchard".to_string(),
                    ciphertext: Some(output.encrypted_output.clone()),
                    epk: None,
                    out_ciphertext: None,
                }
            } else if let Some(output) = orchard_outputs.get(commitment.as_str()) {
                PrivatePoolOutputRow {
                    position,
                    commitment: commitment.clone(),
                    kind: "orchard".to_string(),
                    ciphertext: Some(output.enc_ciphertext.clone()),
                    epk: Some(output.epk.clone()),
                    out_ciphertext: Some(output.out_ciphertext.clone()),
                }
            } else {
                PrivatePoolOutputRow {
                    position,
                    commitment: commitment.clone(),
                    kind: "missing".to_string(),
                    ciphertext: None,
                    epk: None,
                    out_ciphertext: None,
                }
            }
        })
        .collect()
}

fn asset_rows(ledger: &LedgerState, pool: &OrchardPoolState) -> Vec<PrivatePoolAssetRow> {
    let mut rows = pool
        .asset_orchard_balances
        .iter()
        .map(|balance| {
            let asset = ledger.asset_definition(&balance.asset_id);
            PrivatePoolAssetRow {
                asset_id: balance.asset_id.clone(),
                ingress_total: balance.ingress_total,
                egress_total: balance.egress_total,
                live_total: balance.live_total,
                asset_info_bound: asset.is_some(),
                issuer: asset.map(|asset| asset.issuer.clone()),
                code: asset.map(|asset| asset.code.clone()),
                precision: asset.map(|asset| asset.precision),
                requires_authorization: asset.map(|asset| asset.requires_authorization),
                proof_profile_id: ledger
                    .nav_assets
                    .iter()
                    .find(|nav| nav.asset_id == balance.asset_id)
                    .map(|nav| nav.proof_profile.clone()),
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    rows
}

fn completeness(pool: Option<&OrchardPoolState>) -> io::Result<PrivatePoolCompleteness> {
    let Some(pool) = pool else {
        return Ok(PrivatePoolCompleteness {
            empty_pool_before_ingress: true,
            history_complete: true,
            outputs_complete: true,
            anchors_from_retained_prefixes: true,
            complete: true,
        });
    };
    let commitments = pool.output_commitments.len() as u64;
    let ingress_recorded = pool
        .asset_orchard_balances
        .iter()
        .any(|balance| balance.ingress_total > 0)
        || !pool.nullifiers.is_empty()
        || pool.turnstile_deposit_total > 0;
    let empty_pool_before_ingress = commitments == 0 && !ingress_recorded;
    let history_complete = commitments > 0 || !ingress_recorded;
    let outputs_complete = output_rows(pool).iter().all(|row| row.ciphertext.is_some());
    let anchors = retained_anchor_rows(pool)?;
    let anchors_from_retained_prefixes = anchors
        .windows(2)
        .all(|pair| pair[0].output_count < pair[1].output_count)
        && anchors
            .last()
            .is_some_and(|last| last.output_count == commitments);
    Ok(PrivatePoolCompleteness {
        empty_pool_before_ingress,
        history_complete,
        outputs_complete,
        anchors_from_retained_prefixes,
        complete: history_complete && outputs_complete && anchors_from_retained_prefixes,
    })
}

/// Pins one committed chain tip and returns the identity, counts, pricing
/// claim, egress binding and completeness a wallet needs before paging.
pub fn private_pool_snapshot(
    options: PrivatePoolSnapshotOptions,
) -> io::Result<PrivatePoolSnapshotReport> {
    if !valid_pool_id(&options.pool_id) {
        return Err(invalid_input(
            "pool_id must be 1-64 ASCII letters, digits, '-' or '_'",
        ));
    }
    if let Some(address) = options.address.as_deref() {
        validate_address(address)?;
    }
    let snapshot = read_finalized_pair_snapshot(&options.data_dir)?;
    let pool = snapshot_pool(&snapshot, &options.pool_id);
    let inclusion_height = options
        .inclusion_height
        .unwrap_or_else(|| snapshot.tip.height.saturating_add(1));
    let (base_asset_id, quote_asset_id) = match (&options.base_asset_id, &options.quote_asset_id) {
        (Some(base), Some(quote)) => (Some(base.clone()), Some(quote.clone())),
        (None, None) => match snapshot.ledger.pftl_uniswap_routes.as_slice() {
            [route] => (
                Some(route.native_nav_asset_id.clone()),
                Some(route.settlement_asset_id.clone()),
            ),
            _ => (None, None),
        },
        _ => {
            return Err(invalid_input(
                "base_asset_id and quote_asset_id must be given together",
            ))
        }
    };
    let (pricing_status, pricing_refusal, pricing_claim, egress_status, policy_id, disclosure_hash) =
        match (&base_asset_id, &quote_asset_id) {
            (Some(base), Some(quote)) => {
                let pair = AssetOrchardPairReadOptions {
                    data_dir: options.data_dir.clone(),
                    base_asset_id: base.clone(),
                    quote_asset_id: quote.clone(),
                    inclusion_height,
                };
                let pricing = asset_orchard_pricing_claim_from_snapshot(&snapshot, &pair)?;
                let egress = asset_orchard_private_egress_binding_from_snapshot(&snapshot, &pair)?;
                let binding = egress
                    .bindings
                    .iter()
                    .find(|binding| binding.asset_id == *base);
                (
                    pricing.status,
                    pricing.refusal,
                    pricing.pricing_claim,
                    egress.status,
                    binding.and_then(|binding| binding.policy_id.clone()),
                    binding.and_then(|binding| binding.disclosure_hash.clone()),
                )
            }
            _ => (
                "unavailable".to_string(),
                Some(read_refusal(
                    "private_pool_pair_unknown",
                    "no base/quote pair was given and finalized state has not exactly one governed NAV route",
                )),
                None,
                "unavailable".to_string(),
                None,
                None,
            ),
        };
    let pricing_claim_json = pricing_claim
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(invalid_data)?;
    let counts = match pool {
        Some(pool) => PrivatePoolCounts {
            commitments: pool.output_commitments.len() as u64,
            nullifiers: pool.nullifiers.len() as u64,
            accepted_anchors: retained_anchor_rows(pool)?.len() as u64,
            outputs: pool.output_commitments.len() as u64,
            assets: pool.asset_orchard_balances.len() as u64,
        },
        None => PrivatePoolCounts {
            commitments: 0,
            nullifiers: 0,
            accepted_anchors: 0,
            outputs: 0,
            assets: 0,
        },
    };
    Ok(PrivatePoolSnapshotReport {
        schema: PRIVATE_POOL_SNAPSHOT_SCHEMA.to_string(),
        chain_id: snapshot.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&snapshot.genesis),
        protocol_version: snapshot.genesis.protocol_version,
        address: options.address,
        snapshot_token: SnapshotToken::of(&snapshot, &options.pool_id).encode(),
        finalized_height: snapshot.tip.height,
        finalized_block_hash: snapshot.tip.block_hash.clone(),
        finalized_state_root: snapshot.tip.state_root.clone(),
        pool_id: options.pool_id,
        pool_present: pool.is_some(),
        counts,
        page_limits: PrivatePoolPageLimits {
            items: PRIVATE_POOL_PAGE_MAX_ITEMS as u64,
            outputs: PRIVATE_POOL_PAGE_MAX_OUTPUTS as u64,
            output_ciphertext_bytes: PRIVATE_POOL_PAGE_MAX_CIPHERTEXT_BYTES as u64,
            assets: PRIVATE_POOL_PAGE_MAX_ASSETS as u64,
        },
        base_asset_id,
        quote_asset_id,
        inclusion_height,
        pricing_status,
        pricing_refusal,
        pricing_claim,
        pricing_claim_json,
        egress_status,
        policy_id,
        disclosure_hash,
        completeness: completeness(pool)?,
    })
}

fn cursor_digest(token: &str, section: &str) -> String {
    hash_hex(CURSOR_DOMAIN, format!("{token}\n{section}").as_bytes())[..16].to_string()
}

pub(super) fn encode_cursor(token: &str, section: &str, offset: usize) -> String {
    format!(
        "{CURSOR_PREFIX}:{section}:{offset}:{}",
        cursor_digest(token, section)
    )
}

/// A cursor is bound to its snapshot token and section; any other cursor is
/// malformed.
pub(super) fn parse_cursor(cursor: &str, token: &str, section: &str) -> Option<usize> {
    let mut parts = cursor.split(':');
    let (prefix, cursor_section, offset, digest) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let parsed = offset.parse::<usize>().ok()?;
    (parts.next().is_none()
        && prefix == CURSOR_PREFIX
        && cursor_section == section
        && parsed.to_string() == offset
        && digest == cursor_digest(token, section))
    .then_some(parsed)
}

fn section_max(section: &str) -> usize {
    match section {
        "outputs" => PRIVATE_POOL_PAGE_MAX_OUTPUTS,
        "assets" => PRIVATE_POOL_PAGE_MAX_ASSETS,
        _ => PRIVATE_POOL_PAGE_MAX_ITEMS,
    }
}

/// One deterministic page of a pool section at the token's committed tip.
pub fn private_pool_page(options: PrivatePoolPageOptions) -> io::Result<PrivatePoolPageReport> {
    let max = section_max(&options.section);
    let limit = options.limit.unwrap_or(max).clamp(1, max);
    let mut report = PrivatePoolPageReport {
        schema: PRIVATE_POOL_PAGE_SCHEMA.to_string(),
        snapshot_token: options.snapshot_token.clone(),
        section: options.section.clone(),
        status: "ok".to_string(),
        refusal: None,
        offset: 0,
        limit: limit as u64,
        total: 0,
        item_count: 0,
        items: None,
        next_cursor: None,
    };
    let refuse = |mut report: PrivatePoolPageReport, code: &str, reason: String| {
        report.status = "refused".to_string();
        report.refusal = Some(read_refusal(code, reason));
        Ok(report)
    };
    if !PRIVATE_POOL_SECTIONS.contains(&options.section.as_str()) {
        let reason = format!(
            "section must be one of {}",
            PRIVATE_POOL_SECTIONS.join(", ")
        );
        return refuse(report, "section_unknown", reason);
    }
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
    let offset = match options.cursor.as_deref() {
        None => 0,
        Some(cursor) => match parse_cursor(cursor, &options.snapshot_token, &options.section) {
            Some(offset) => offset,
            None => {
                return refuse(
                    report,
                    "cursor_malformed",
                    "cursor was not issued for this snapshot token and section".to_string(),
                )
            }
        },
    };
    let pool = snapshot_pool(&snapshot, &token.pool_id);
    let empty = OrchardPoolState::empty(token.pool_id.clone());
    let pool = pool.unwrap_or(&empty);
    let total = match options.section.as_str() {
        "commitments" | "outputs" => pool.output_commitments.len(),
        "nullifiers" => pool.nullifiers.len(),
        "accepted_anchors" => {
            if snapshot_pool(&snapshot, &token.pool_id).is_some() {
                retained_anchor_rows(pool)?.len()
            } else {
                0
            }
        }
        _ => pool.asset_orchard_balances.len(),
    };
    report.total = total as u64;
    report.offset = offset as u64;
    if options.cursor.is_some() && offset >= total {
        let reason = format!("cursor offset {offset} is past the end of {total} items");
        return refuse(report, "cursor_out_of_range", reason);
    }
    let end = offset.saturating_add(limit).min(total);
    let (items, end) = match options.section.as_str() {
        "commitments" => (
            PrivatePoolPageItems::Strings(pool.output_commitments[offset..end].to_vec()),
            end,
        ),
        "nullifiers" => (
            PrivatePoolPageItems::Strings(pool.nullifiers[offset..end].to_vec()),
            end,
        ),
        "accepted_anchors" => {
            let rows = if total == 0 {
                Vec::new()
            } else {
                retained_anchor_rows(pool)?
            };
            (
                PrivatePoolPageItems::Anchors(rows[offset..end].to_vec()),
                end,
            )
        }
        "outputs" => {
            let rows = output_rows(pool);
            let mut page = Vec::new();
            let mut bytes = 0usize;
            for row in &rows[offset..end] {
                let row_bytes = row.ciphertext.as_ref().map_or(0, |hex| hex.len() / 2)
                    + row.out_ciphertext.as_ref().map_or(0, |hex| hex.len() / 2);
                if !page.is_empty()
                    && bytes.saturating_add(row_bytes) > PRIVATE_POOL_PAGE_MAX_CIPHERTEXT_BYTES
                {
                    break;
                }
                bytes = bytes.saturating_add(row_bytes);
                page.push(row.clone());
            }
            let end = offset + page.len();
            (PrivatePoolPageItems::Outputs(page), end)
        }
        _ => {
            let rows = asset_rows(&snapshot.ledger, pool);
            (
                PrivatePoolPageItems::Assets(rows[offset..end].to_vec()),
                end,
            )
        }
    };
    report.item_count = (end - offset) as u64;
    report.items = Some(items);
    report.next_cursor =
        (end < total).then(|| encode_cursor(&options.snapshot_token, &options.section, end));
    Ok(report)
}

/// The account's issued-asset lines at the token's committed tip, including
/// zero-balance lines, each bound to its `asset_info` row.
pub fn private_account_lines(
    options: PrivateAccountLinesOptions,
) -> io::Result<PrivateAccountLinesReport> {
    validate_address(&options.address)?;
    let mut report = PrivateAccountLinesReport {
        schema: PRIVATE_ACCOUNT_LINES_SCHEMA.to_string(),
        snapshot_token: options.snapshot_token.clone(),
        address: options.address.clone(),
        status: "ok".to_string(),
        refusal: None,
        line_count: 0,
        lines: Vec::new(),
        positive_asset_count: 0,
        account_assets_consistent: false,
        all_lines_usable: false,
    };
    let refuse = |mut report: PrivateAccountLinesReport, code: &str, reason: String| {
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
    let lines =
        issued_asset_line_reports(&snapshot.ledger, |line| line.account == options.address)?;
    if lines.len() > PRIVATE_ACCOUNT_LINES_MAX {
        let reason = format!(
            "account has {} lines, above the {PRIVATE_ACCOUNT_LINES_MAX}-line read maximum",
            lines.len()
        );
        return refuse(report, "account_lines_over_maximum", reason);
    }
    let rows = lines
        .iter()
        .map(|line| {
            let asset = snapshot
                .ledger
                .asset_definition(&line.asset_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("line asset {} is missing", line.asset_id),
                    )
                })?;
            let line_status = if line.issuer != asset.issuer {
                "issuer_mismatch"
            } else if line.frozen {
                "frozen"
            } else if asset.requires_authorization && !line.authorized {
                "unauthorized"
            } else {
                "ok"
            };
            Ok(PrivateAccountLineRow {
                trustline_id: line.trustline_id.clone(),
                asset_id: line.asset_id.clone(),
                issuer: asset.issuer.clone(),
                code: asset.code.clone(),
                precision: asset.precision,
                requires_authorization: asset.requires_authorization,
                balance: line.balance,
                limit: line.limit,
                authorized: line.authorized,
                frozen: line.frozen,
                line_status: line_status.to_string(),
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    // The account_assets read lists exactly the account's positive lines.
    let positive_assets = issued_asset_line_reports(&snapshot.ledger, |line| {
        line.account == options.address && line.balance > 0
    })?;
    let positive_lines = rows
        .iter()
        .filter(|row| row.balance > 0)
        .map(|row| (row.asset_id.as_str(), row.balance))
        .collect::<Vec<_>>();
    report.account_assets_consistent = positive_assets
        .iter()
        .map(|asset| (asset.asset_id.as_str(), asset.balance))
        .collect::<Vec<_>>()
        == positive_lines;
    report.positive_asset_count = positive_assets.len() as u64;
    report.all_lines_usable = rows.iter().all(|row| row.line_status == "ok");
    report.line_count = rows.len() as u64;
    report.lines = rows;
    Ok(report)
}
