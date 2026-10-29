//! Finalized-state reads a user wallet needs to prove an Asset-Orchard swap or
//! private egress locally: the exact pricing claim consensus accepts at an
//! intended inclusion height, and the egress policy/disclosure binding.
//! Every field of one report comes from one committed chain-tip state.

use super::*;
use postfiat_privacy_orchard::{
    validate_asset_orchard_pricing_policy, AssetOrchardPricingClaim,
    AssetOrchardPricingClaimProvenance, AssetOrchardPricingPolicy, AssetOrchardSwapBindingHash,
    AssetTag, VerifiedAssetOrchardPricingClaim,
};

pub const ASSET_ORCHARD_PRICING_CLAIM_READ_SCHEMA: &str = "postfiat.asset_orchard_pricing_claim.v1";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_BINDING_READ_SCHEMA: &str =
    "postfiat.asset_orchard_private_egress_binding.v1";
/// Every at-NAV claim producer (circuit fixtures, local service, app) uses
/// this mode; consensus enforces the band, not the mode label.
pub const ASSET_ORCHARD_AT_NAV_PRICING_MODE: &str = "at_nav_with_band";

const SNAPSHOT_READ_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardPairReadOptions {
    pub data_dir: PathBuf,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub inclusion_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardReadRefusal {
    pub code: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardGoverningAsset {
    pub asset_id: String,
    pub asset_tag_lo: String,
    pub asset_tag_hi: String,
    pub issuer: String,
    pub code: String,
    pub version: u32,
    pub precision: u8,
    pub asset_family_id: String,
    pub source_series_id: String,
    pub nav_tracked: bool,
    pub nav_issuer: Option<String>,
    pub proof_profile_id: Option<String>,
    pub valuation_unit: Option<String>,
    pub finalized_epoch: Option<u64>,
    pub finalized_reserve_packet_hash: Option<String>,
    pub finalized_at_height: Option<u64>,
    pub nav_live_through_height: Option<u64>,
    pub halted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardPricingClaimReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
    pub finalized_state_root: String,
    pub inclusion_height: u64,
    /// `ok`, `stale`, `halted` or `mismatch`.
    pub status: String,
    pub refusal: Option<AssetOrchardReadRefusal>,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub base_asset: Option<AssetOrchardGoverningAsset>,
    pub quote_asset: Option<AssetOrchardGoverningAsset>,
    pub nav_usd_e8_activation_height: u64,
    pub ratio_denominator_unit: u64,
    /// Governance `asset_orchard_pricing_enforcement_activation_height`.
    pub pricing_enforcement_activation_height: Option<u64>,
    /// Whether the freshness, governed-pair and mode rules apply at the
    /// inclusion height.
    pub pricing_enforced: bool,
    /// The governed NAV/settlement route binding this pair, if any.
    pub governed_route_id: Option<String>,
    pub pricing_claim: Option<AssetOrchardPricingClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressBinding {
    pub asset_id: String,
    pub policy_id: Option<String>,
    pub disclosure_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressBindingReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
    pub finalized_state_root: String,
    pub inclusion_height: u64,
    /// `ok`, `stale`, `halted`, `mismatch` or `ungoverned`.
    pub status: String,
    pub refusal: Option<AssetOrchardReadRefusal>,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub base_asset: Option<AssetOrchardGoverningAsset>,
    pub quote_asset: Option<AssetOrchardGoverningAsset>,
    pub bindings: Vec<AssetOrchardPrivateEgressBinding>,
}

pub(super) struct FinalizedPairSnapshot {
    pub(super) genesis: Genesis,
    pub(super) tip: ChainTipState,
    pub(super) governance: GovernanceState,
    pub(super) ledger: LedgerState,
    pub(super) shielded: ShieldedState,
}

/// Reads governance and ledger state and proves, by recomputing the
/// replicated state root over the exact copies served, that they are the
/// committed chain-tip state. A commit racing the read is retried.
pub(super) fn read_finalized_pair_snapshot(data_dir: &Path) -> io::Result<FinalizedPairSnapshot> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    for _ in 0..SNAPSHOT_READ_ATTEMPTS {
        let tip = super::storage_commit::read_chain_tip_or_reconstruct_for_genesis_read_only(
            &store, &genesis,
        )?;
        let governance = store.read_governance()?;
        let ledger = store.read_ledger()?;
        let shielded = store.read_shielded()?;
        let bridge = store.read_bridge()?;
        let root = super::lifecycle_queries::replicated_state_root_of_parts(
            &store,
            &genesis,
            &governance,
            &ledger,
            &shielded,
            &bridge,
        )?;
        let tip_after = super::storage_commit::read_chain_tip_or_reconstruct_for_genesis_read_only(
            &store, &genesis,
        )?;
        if root == tip.state_root
            && tip_after.height == tip.height
            && tip_after.block_hash == tip.block_hash
            && tip_after.state_root == tip.state_root
        {
            return Ok(FinalizedPairSnapshot {
                genesis,
                tip,
                governance,
                ledger,
                shielded,
            });
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Interrupted,
        "state did not match one committed chain tip; retry the read",
    ))
}

fn validate_pair_options(options: &AssetOrchardPairReadOptions) -> io::Result<()> {
    super::lifecycle_queries::validate_issued_asset_query_id(
        "base_asset_id",
        &options.base_asset_id,
    )?;
    super::lifecycle_queries::validate_issued_asset_query_id(
        "quote_asset_id",
        &options.quote_asset_id,
    )?;
    if options.inclusion_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "inclusion_height must be nonzero",
        ));
    }
    Ok(())
}

fn refusal(code: &str, reason: impl Into<String>) -> AssetOrchardReadRefusal {
    AssetOrchardReadRefusal {
        code: code.to_string(),
        reason: reason.into(),
    }
}

fn governing_asset(
    ledger: &LedgerState,
    asset_id: &str,
    max_nav_age_blocks: Option<u64>,
) -> io::Result<Option<AssetOrchardGoverningAsset>> {
    let Some(definition) = ledger.asset_definition(asset_id) else {
        return Ok(None);
    };
    let tag = AssetTag::derive(asset_id).map_err(invalid_data)?;
    let nav = ledger
        .nav_assets
        .iter()
        .find(|asset| asset.asset_id == asset_id);
    let live_through = nav
        .and_then(|asset| asset_orchard_nav_live_through_height(ledger, asset, max_nav_age_blocks))
        .flatten();
    Ok(Some(AssetOrchardGoverningAsset {
        asset_id: definition.asset_id.clone(),
        asset_tag_lo: format!("{:032x}", tag.lo),
        asset_tag_hi: format!("{:032x}", tag.hi),
        issuer: definition.issuer.clone(),
        code: definition.code.clone(),
        version: definition.version,
        precision: definition.precision,
        asset_family_id: definition.asset_family_id.clone(),
        source_series_id: definition.source_series_id.clone(),
        nav_tracked: nav.is_some(),
        nav_issuer: nav.map(|asset| asset.issuer.clone()),
        proof_profile_id: nav.map(|asset| asset.proof_profile.clone()),
        valuation_unit: nav.map(|asset| asset.valuation_unit.clone()),
        finalized_epoch: nav.map(|asset| asset.finalized_epoch),
        finalized_reserve_packet_hash: nav.map(|asset| asset.finalized_reserve_packet_hash.clone()),
        finalized_at_height: nav.map(|asset| asset.finalized_at_height),
        nav_live_through_height: live_through,
        halted: nav.is_some_and(|asset| asset.halted),
    }))
}

/// Refusals shared by both reads, in precedence order: an inclusion height
/// that is not after the read tip, an unknown or duplicate asset, a halted
/// pool or NAV route, and a NAV epoch past its deadman bound.
fn pair_refusal(
    snapshot: &FinalizedPairSnapshot,
    options: &AssetOrchardPairReadOptions,
    enforcement: Option<AssetOrchardPricingEnforcement>,
    base: Option<&AssetOrchardGoverningAsset>,
    quote: Option<&AssetOrchardGoverningAsset>,
) -> Option<(&'static str, AssetOrchardReadRefusal)> {
    if options.inclusion_height <= snapshot.tip.height {
        return Some((
            "stale",
            refusal(
                "asset_orchard_inclusion_height_not_after_finalized",
                format!(
                    "inclusion height {} must be after finalized height {}",
                    options.inclusion_height, snapshot.tip.height
                ),
            ),
        ));
    }
    if options.base_asset_id == options.quote_asset_id {
        return Some((
            "mismatch",
            refusal(
                "asset_orchard_pricing_pair_not_governed",
                "base and quote must be different assets",
            ),
        ));
    }
    for (label, asset_id, asset) in [
        ("base", &options.base_asset_id, base),
        ("quote", &options.quote_asset_id, quote),
    ] {
        if asset.is_none() {
            return Some((
                "mismatch",
                refusal(
                    "asset_orchard_asset_unknown",
                    format!("{label} asset {asset_id} is not a registered issued asset"),
                ),
            ));
        }
    }
    if snapshot.governance.orchard_pool_paused {
        return Some((
            "halted",
            refusal(
                "orchard_pool_paused",
                "governance has paused the Orchard pool",
            ),
        ));
    }
    for asset in [base, quote].into_iter().flatten() {
        if asset.halted {
            let reason = snapshot
                .ledger
                .nav_assets
                .iter()
                .find(|nav| nav.asset_id == asset.asset_id)
                .map(|nav| nav.halt_reason.clone())
                .unwrap_or_default();
            return Some((
                "halted",
                refusal(
                    "asset_orchard_pricing_halted",
                    format!("NAV asset {} is halted: {reason}", asset.asset_id),
                ),
            ));
        }
        let nav = snapshot
            .ledger
            .nav_assets
            .iter()
            .find(|nav| nav.asset_id == asset.asset_id);
        let live_through = nav.and_then(|nav| {
            asset_orchard_nav_live_through_height(
                &snapshot.ledger,
                nav,
                enforcement.and_then(|enforcement| enforcement.max_nav_age_blocks),
            )
        });
        // Consensus refuses an epoch of unknown age only once enforcement is
        // active; before that the read reports the profile deadman alone.
        if let Some(live_through) = live_through {
            let stale = match live_through {
                Some(live_through) => options.inclusion_height > live_through,
                None => enforcement.is_some(),
            };
            if stale {
                return Some((
                    "stale",
                    refusal(
                        "asset_orchard_pricing_nav_stale",
                        format!(
                            "NAV epoch of {} is live through height {live_through:?}, before inclusion height {}",
                            asset.asset_id, options.inclusion_height
                        ),
                    ),
                ));
            }
        }
    }
    None
}

fn pricing_enforcement(
    snapshot: &FinalizedPairSnapshot,
    inclusion_height: u64,
) -> Option<AssetOrchardPricingEnforcement> {
    asset_orchard_pricing_enforcement_at(
        &asset_execution_compatibility_for_genesis_and_governance(
            &snapshot.genesis,
            &snapshot.governance,
        ),
        inclusion_height,
    )
}

fn pricing_refusal_status(code: &str) -> &'static str {
    match code {
        "asset_orchard_pricing_not_finalized" | "asset_orchard_pricing_nav_stale" => "stale",
        "asset_orchard_pricing_halted" => "halted",
        _ => "mismatch",
    }
}

/// The exact typed claim consensus accepts for an at-NAV Asset-Orchard swap
/// of the pair included at `inclusion_height`, read from the committed tip.
/// It stays acceptable until a later NAV epoch, halt or profile change
/// finalizes before inclusion.
pub fn asset_orchard_pricing_claim(
    options: AssetOrchardPairReadOptions,
) -> io::Result<AssetOrchardPricingClaimReport> {
    validate_pair_options(&options)?;
    let snapshot = read_finalized_pair_snapshot(&options.data_dir)?;
    asset_orchard_pricing_claim_from_snapshot(&snapshot, &options)
}

/// [`asset_orchard_pricing_claim`] over an already-read finalized snapshot,
/// so a caller serving several reads keeps them on one state.
pub(super) fn asset_orchard_pricing_claim_from_snapshot(
    snapshot: &FinalizedPairSnapshot,
    options: &AssetOrchardPairReadOptions,
) -> io::Result<AssetOrchardPricingClaimReport> {
    validate_pair_options(options)?;
    let enforcement = pricing_enforcement(&snapshot, options.inclusion_height);
    let max_nav_age = enforcement.and_then(|enforcement| enforcement.max_nav_age_blocks);
    let base = governing_asset(&snapshot.ledger, &options.base_asset_id, max_nav_age)?;
    let quote = governing_asset(&snapshot.ledger, &options.quote_asset_id, max_nav_age)?;
    let mut report = AssetOrchardPricingClaimReport {
        schema: ASSET_ORCHARD_PRICING_CLAIM_READ_SCHEMA.to_string(),
        chain_id: snapshot.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&snapshot.genesis),
        protocol_version: snapshot.genesis.protocol_version,
        finalized_height: snapshot.tip.height,
        finalized_block_hash: snapshot.tip.block_hash.clone(),
        finalized_state_root: snapshot.tip.state_root.clone(),
        inclusion_height: options.inclusion_height,
        status: "ok".to_string(),
        refusal: None,
        base_asset_id: options.base_asset_id.clone(),
        quote_asset_id: options.quote_asset_id.clone(),
        base_asset: base.clone(),
        quote_asset: quote.clone(),
        nav_usd_e8_activation_height: ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT,
        ratio_denominator_unit: asset_orchard_nav_ratio_denominator(options.inclusion_height),
        pricing_enforcement_activation_height: snapshot
            .governance
            .asset_orchard_pricing_enforcement_activation_height(),
        pricing_enforced: enforcement.is_some(),
        governed_route_id: snapshot
            .ledger
            .pftl_uniswap_routes
            .iter()
            .find(|route| {
                route.native_nav_asset_id == options.base_asset_id
                    && route.settlement_asset_id == options.quote_asset_id
            })
            .map(|route| route.route_id.clone()),
        pricing_claim: None,
    };
    let refuse = |mut report: AssetOrchardPricingClaimReport,
                  status: &str,
                  refusal: AssetOrchardReadRefusal| {
        report.status = status.to_string();
        report.refusal = Some(refusal);
        report
    };
    if let Some((status, reason)) = pair_refusal(
        &snapshot,
        &options,
        enforcement,
        base.as_ref(),
        quote.as_ref(),
    ) {
        return Ok(refuse(report, status, reason));
    }
    if options.inclusion_height < ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT {
        return Ok(refuse(
            report,
            "mismatch",
            refusal(
                "asset_orchard_pricing_before_nav_usd_e8_activation",
                format!(
                    "inclusion height {} precedes the NAV USD-e8 pricing rule at height {}; \
                     the legacy denominator {} is replay-only",
                    options.inclusion_height,
                    ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT,
                    postfiat_types::VAULT_BRIDGE_UNIT
                ),
            ),
        ));
    }
    let base_tag = AssetTag::derive(&options.base_asset_id).map_err(invalid_data)?;
    let quote_tag = AssetTag::derive(&options.quote_asset_id).map_err(invalid_data)?;
    let policy = match asset_orchard_at_nav_pricing_policy(
        &snapshot.ledger,
        base_tag,
        quote_tag,
        options.inclusion_height,
    ) {
        Ok(policy) => policy,
        Err(error) => {
            let status = pricing_refusal_status(error.code());
            return Ok(refuse(
                report,
                status,
                refusal(error.code(), error.to_string()),
            ));
        }
    };
    let claim = at_nav_pricing_claim(&policy);
    if let Err(error) = claim.validate() {
        return Ok(refuse(
            report,
            "mismatch",
            refusal(error.code(), error.to_string()),
        ));
    }
    if let Some(enforcement) = enforcement {
        if let Err(error) = enforce_asset_orchard_at_nav_pricing_rules(
            &snapshot.ledger,
            &claim,
            options.inclusion_height,
            enforcement,
        ) {
            let status = pricing_refusal_status(error.code());
            return Ok(refuse(
                report,
                status,
                refusal(error.code(), error.to_string()),
            ));
        }
    }
    // Self-check against the validator policy consensus applies at the
    // inclusion height, so a returned claim is never one consensus rejects.
    let evidence = VerifiedAssetOrchardPricingClaim {
        claim: claim.clone(),
        action_binding_hash: AssetOrchardSwapBindingHash::from_bytes(
            &[0u8; postfiat_privacy_orchard::ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES],
        ),
        provenance: AssetOrchardPricingClaimProvenance::CircuitProven,
    };
    if let Err(error) = validate_asset_orchard_pricing_policy(&evidence, &policy) {
        let status = pricing_refusal_status(error.code());
        return Ok(refuse(
            report,
            status,
            refusal(error.code(), error.to_string()),
        ));
    }
    report.pricing_claim = Some(claim);
    Ok(report)
}

fn at_nav_pricing_claim(policy: &AssetOrchardPricingPolicy) -> AssetOrchardPricingClaim {
    AssetOrchardPricingClaim {
        nav_epoch: policy.nav_epoch,
        reserve_packet_hash: policy.reserve_packet_hash.clone(),
        ratio_numerator: policy.nav_ratio_numerator,
        ratio_denominator: policy.nav_ratio_denominator,
        mode: ASSET_ORCHARD_AT_NAV_PRICING_MODE.to_string(),
        band_bps: policy.band_bps,
        base_asset_tag_lo: policy.base_asset_tag.lo,
        base_asset_tag_hi: policy.base_asset_tag.hi,
        quote_asset_tag_lo: policy.quote_asset_tag.lo,
        quote_asset_tag_hi: policy.quote_asset_tag.hi,
    }
}

/// The policy id and disclosure hash a private egress of each asset must bind
/// on this chain, from governed state. Finalized state has no governed
/// Asset-Orchard egress policy: consensus accepts any canonical `policy_id`
/// and `disclosure_hash` the proof's exit binding commits to. This read
/// therefore returns `ungoverned` rather than a proxy or fixture default.
pub fn asset_orchard_private_egress_binding(
    options: AssetOrchardPairReadOptions,
) -> io::Result<AssetOrchardPrivateEgressBindingReport> {
    validate_pair_options(&options)?;
    let snapshot = read_finalized_pair_snapshot(&options.data_dir)?;
    asset_orchard_private_egress_binding_from_snapshot(&snapshot, &options)
}

pub(super) fn asset_orchard_private_egress_binding_from_snapshot(
    snapshot: &FinalizedPairSnapshot,
    options: &AssetOrchardPairReadOptions,
) -> io::Result<AssetOrchardPrivateEgressBindingReport> {
    validate_pair_options(options)?;
    let enforcement = pricing_enforcement(&snapshot, options.inclusion_height);
    let max_nav_age = enforcement.and_then(|enforcement| enforcement.max_nav_age_blocks);
    let base = governing_asset(&snapshot.ledger, &options.base_asset_id, max_nav_age)?;
    let quote = governing_asset(&snapshot.ledger, &options.quote_asset_id, max_nav_age)?;
    let (status, refusal) =
        match pair_refusal(&snapshot, &options, enforcement, base.as_ref(), quote.as_ref()) {
            Some((status, reason)) => (status, reason),
            None => (
                "ungoverned",
                refusal(
                    "asset_orchard_private_egress_policy_ungoverned",
                    "finalized state registers no private-egress policy or disclosure binding \
                     for these assets; consensus only binds caller-chosen values into the exit proof",
                ),
            ),
        };
    let bindings = [&options.base_asset_id, &options.quote_asset_id]
        .into_iter()
        .map(|asset_id| AssetOrchardPrivateEgressBinding {
            asset_id: asset_id.clone(),
            policy_id: None,
            disclosure_hash: None,
        })
        .collect();
    Ok(AssetOrchardPrivateEgressBindingReport {
        schema: ASSET_ORCHARD_PRIVATE_EGRESS_BINDING_READ_SCHEMA.to_string(),
        chain_id: snapshot.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&snapshot.genesis),
        protocol_version: snapshot.genesis.protocol_version,
        finalized_height: snapshot.tip.height,
        finalized_block_hash: snapshot.tip.block_hash.clone(),
        finalized_state_root: snapshot.tip.state_root.clone(),
        inclusion_height: options.inclusion_height,
        status: status.to_string(),
        refusal: Some(refusal),
        base_asset_id: options.base_asset_id.clone(),
        quote_asset_id: options.quote_asset_id.clone(),
        base_asset: base,
        quote_asset: quote,
        bindings,
    })
}
