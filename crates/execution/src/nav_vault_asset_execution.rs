fn nav_subscription_consumer_id(nav_asset_id: &str) -> String {
    format!("nav_subscription:{nav_asset_id}")
}

fn nav_subscription_recipient_consumer_id(nav_asset_id: &str, recipient: &str) -> String {
    format!("nav_subscription:{nav_asset_id}:{recipient}")
}

fn nav_subscription_recipient_order_consumer_id(
    nav_asset_id: &str,
    recipient: &str,
    subscription_id: &str,
) -> String {
    format!("nav_subscription:{nav_asset_id}:{recipient}:{subscription_id}")
}

fn nav_subscription_consumer_matches(consumer_id: &str, nav_asset_id: &str) -> bool {
    consumer_id == nav_subscription_consumer_id(nav_asset_id)
        || consumer_id
            .strip_prefix(&format!("nav_subscription:{nav_asset_id}:"))
            .map_or(false, |recipient| !recipient.is_empty())
}

fn nav_subscription_consumer_matches_recipient(
    consumer_id: &str,
    nav_asset_id: &str,
    recipient: &str,
) -> bool {
    consumer_id == nav_subscription_consumer_id(nav_asset_id)
        || consumer_id == nav_subscription_recipient_consumer_id(nav_asset_id, recipient)
        || consumer_id
            .strip_prefix(&format!("nav_subscription:{nav_asset_id}:{recipient}:"))
            .map_or(false, |subscription_id| !subscription_id.is_empty())
}

fn nav_redemption_consumer_id(redemption_id: &str) -> String {
    format!("nav_redemption:{redemption_id}")
}

fn nav_redemption_supply_consumer_id(redemption_id: &str, receipt_id: &str) -> String {
    format!("nav_redemption_supply:{redemption_id}:{receipt_id}")
}

fn release_vault_bridge_supply_allocations(
    ledger: &mut LedgerState,
    asset_family_id: &str,
    bucket_id: &str,
    release_atoms: u64,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    if release_atoms == 0 {
        return Ok(());
    }
    let mut indices = ledger
        .vault_bridge_allocations
        .iter()
        .enumerate()
        .filter(|(_, allocation)| {
            allocation.asset_id == asset_family_id
                && allocation.bucket_id == bucket_id
                && allocation.purpose == VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY
                && allocation.released_atoms < allocation.amount_atoms
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        let left = &ledger.vault_bridge_allocations[*left];
        let right = &ledger.vault_bridge_allocations[*right];
        (left.created_at_height, left.allocation_id.as_str())
            .cmp(&(right.created_at_height, right.allocation_id.as_str()))
    });
    let available = indices.iter().try_fold(0_u64, |total, index| {
        let allocation = &ledger.vault_bridge_allocations[*index];
        total
            .checked_add(allocation.amount_atoms - allocation.released_atoms)
            .ok_or((
                "vault_bridge_allocation_overflow",
                "source-bucket supply allocation capacity overflowed".to_string(),
            ))
    })?;
    if available < release_atoms {
        return Err((
            "insufficient_vault_bridge_supply_allocation",
            "source-bucket settlement exceeds its remaining supply allocations".to_string(),
        ));
    }
    let mut remaining = release_atoms;
    for index in indices {
        if remaining == 0 {
            break;
        }
        let allocation = &mut ledger.vault_bridge_allocations[index];
        let capacity = allocation.amount_atoms - allocation.released_atoms;
        let take = capacity.min(remaining);
        allocation.released_atoms += take;
        if allocation.released_atoms == allocation.amount_atoms {
            allocation.retired_at_height = block_height;
        }
        allocation
            .validate()
            .map_err(|error| ("bad_vault_bridge_allocation", error))?;
        remaining -= take;
    }
    debug_assert_eq!(remaining, 0);
    Ok(())
}

fn reconcile_vault_bridge_supply_allocations(
    ledger: &mut LedgerState,
    bucket: &VaultBridgeBucketState,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let live_supply_claim = bucket
        .outstanding_vault_bridge_atoms
        .checked_add(bucket.redemption_queue_atoms)
        .ok_or((
            "vault_bridge_bucket_overflow",
            "source-bucket outstanding plus redemption queue overflowed".to_string(),
        ))?;
    let recorded_remaining = ledger
        .vault_bridge_allocations
        .iter()
        .filter(|allocation| {
            allocation.asset_id == bucket.asset_id
                && allocation.bucket_id == bucket.bucket_id
                && allocation.purpose == VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY
        })
        .try_fold(0_u64, |total, allocation| {
            total
                .checked_add(allocation.amount_atoms - allocation.released_atoms)
                .ok_or((
                    "vault_bridge_allocation_overflow",
                    "source-bucket supply allocation total overflowed".to_string(),
                ))
        })?;
    if recorded_remaining < live_supply_claim {
        return Err((
            "vault_bridge_supply_allocation_deficit",
            "source-bucket has more live supply claims than allocation capacity".to_string(),
        ));
    }
    release_vault_bridge_supply_allocations(
        ledger,
        &bucket.asset_id,
        &bucket.bucket_id,
        recorded_remaining - live_supply_claim,
        block_height,
    )
}

fn valuation_unit_scale(valuation_unit: &str, asset_precision: u8) -> Option<u128> {
    let unit = valuation_unit.trim().to_ascii_lowercase();
    if let Some(scale) = unit.strip_prefix("usd_1e") {
        return scale
            .parse::<u32>()
            .ok()
            .and_then(|exponent| 10_u128.checked_pow(exponent));
    }
    match unit.as_str() {
        "usdc" | "usd_1e6" | "micro_usd" => 10_u128.checked_pow(asset_precision.into()),
        _ => None,
    }
}

fn vault_bridge_atoms_to_nav_value(
    amount_atoms: u64,
    nav_valuation_unit: &str,
    settlement_valuation_unit: &str,
    settlement_asset_precision: u8,
) -> Result<u64, (&'static str, String)> {
    let amount_atoms = u128::from(amount_atoms);
    let value = match (
        valuation_unit_scale(nav_valuation_unit, settlement_asset_precision),
        valuation_unit_scale(settlement_valuation_unit, settlement_asset_precision),
    ) {
        (Some(nav_scale), Some(settlement_scale)) if nav_scale != settlement_scale => {
            amount_atoms.checked_mul(nav_scale).ok_or_else(|| {
                (
                    "nav_subscription_overlay_overflow",
                    "nav subscription overlay valuation-scale conversion would overflow"
                        .to_string(),
                )
            })? / settlement_scale
        }
        _ => amount_atoms,
    };
    u64::try_from(value).map_err(|_| {
        (
            "nav_subscription_overlay_overflow",
            "nav subscription overlay value exceeds u64".to_string(),
        )
    })
}

pub fn required_vault_bridge_settlement_atoms(
    amount_atoms: u64,
    nav_asset_precision: u8,
    nav_per_unit: u64,
    nav_valuation_unit: &str,
    settlement_valuation_unit: &str,
    settlement_asset_precision: u8,
) -> Result<u64, (&'static str, String)> {
    let nav_asset_scale = 10_u128
        .checked_pow(nav_asset_precision.into())
        .ok_or_else(|| {
            (
                "nav_settlement_overflow",
                "nav asset precision scale would overflow".to_string(),
            )
        })?;
    let raw = u128::from(amount_atoms)
        .checked_mul(u128::from(nav_per_unit))
        .ok_or_else(|| {
            (
                "nav_settlement_overflow",
                "nav mint amount times nav_per_unit would overflow".to_string(),
            )
        })?;
    let (numerator, denominator) = match (
        valuation_unit_scale(nav_valuation_unit, settlement_asset_precision),
        valuation_unit_scale(settlement_valuation_unit, settlement_asset_precision),
    ) {
        (Some(nav_scale), Some(settlement_scale)) if nav_scale != settlement_scale => {
            let numerator = raw.checked_mul(settlement_scale).ok_or_else(|| {
                (
                    "nav_settlement_overflow",
                    "nav settlement valuation-scale conversion would overflow".to_string(),
                )
            })?;
            let denominator = nav_scale.checked_mul(nav_asset_scale).ok_or_else(|| {
                (
                    "nav_settlement_overflow",
                    "nav settlement denominator scale would overflow".to_string(),
                )
            })?;
            (numerator, denominator)
        }
        _ => (raw, nav_asset_scale),
    };
    let required = numerator.checked_add(denominator - 1).ok_or_else(|| {
        (
            "nav_settlement_overflow",
            "nav settlement valuation-scale rounding would overflow".to_string(),
        )
    })? / denominator;
    u64::try_from(required).map_err(|_| {
        (
            "nav_settlement_overflow",
            "nav settlement amount exceeds u64".to_string(),
        )
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavSubscriptionReserveOverlay {
    pub value_nav_units: u64,
    pub source_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NavSubscriptionReserveOverlayRow {
    allocation_id: String,
    settlement_asset_id: String,
    bucket_id: String,
    receipt_id: String,
    amount_atoms: u64,
    released_atoms: u64,
    remaining_atoms: u64,
    value_nav_units: u64,
    retired_at_height: u64,
    bucket_source_domain: String,
    bucket_policy_hash: String,
    bucket_gross_receipt_atoms: u64,
    bucket_counted_value_atoms: u64,
    bucket_nav_subscription_allocations_atoms: u64,
    bucket_redemption_queue_atoms: u64,
    bucket_outstanding_vault_bridge_atoms: u64,
    bucket_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NavPrimaryMarketReserveOverlayRow {
    route_id: String,
    route_config_digest: String,
    settlement_asset_id: String,
    settlement_reserve_atoms: u64,
    value_nav_units: u64,
    active_bucket_backing_atoms: u64,
    live_value_enabled: bool,
    paused: bool,
}

fn nav_subscription_reserve_overlay(
    ledger: &LedgerState,
    nav_asset: &NavTrackedAsset,
) -> Result<Option<NavSubscriptionReserveOverlay>, (&'static str, String)> {
    let mut rows = Vec::new();
    for allocation in ledger.vault_bridge_allocations.iter().filter(|allocation| {
        allocation.purpose == VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION
            && nav_subscription_consumer_matches(&allocation.consumer_id, &nav_asset.asset_id)
            && allocation.retired_at_height != 0
    }) {
        let remaining_atoms = allocation
            .amount_atoms
            .checked_sub(allocation.released_atoms)
            .ok_or_else(|| {
                (
                    "bad_vault_bridge_allocation",
                    "nav subscription allocation released atoms exceed amount".to_string(),
                )
            })?;
        if remaining_atoms == 0 {
            continue;
        }
        let settlement_nav_asset = ledger.nav_asset(&allocation.asset_id).ok_or_else(|| {
            (
                "missing_vault_bridge_nav_asset",
                "nav subscription overlay references missing settlement NAV asset".to_string(),
            )
        })?;
        let settlement_asset = ledger
            .asset_definition(&allocation.asset_id)
            .ok_or_else(|| {
                (
                    "missing_vault_bridge_asset",
                    "nav subscription overlay references missing settlement asset definition"
                        .to_string(),
                )
            })?;
        let bucket = ledger
            .vault_bridge_bucket(&allocation.bucket_id)
            .ok_or_else(|| {
                (
                    "missing_vault_bridge_bucket",
                    "nav subscription overlay references missing settlement bucket".to_string(),
                )
            })?;
        if bucket.asset_id != allocation.asset_id {
            return Err((
                "vault_bridge_bucket_asset_mismatch",
                "nav subscription overlay bucket asset mismatch".to_string(),
            ));
        }
        if bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE {
            continue;
        }
        let receipt = ledger
            .vault_bridge_receipt(&allocation.receipt_id)
            .ok_or_else(|| {
                (
                    "missing_vault_bridge_receipt",
                    "nav subscription overlay references missing receipt".to_string(),
                )
            })?;
        if receipt.asset_id != allocation.asset_id
            || receipt.bucket_id != allocation.bucket_id
            || receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
        {
            return Err((
                "vault_bridge_receipt_not_counted",
                "nav subscription overlay allocation must reference a counted receipt in the settlement bucket"
                    .to_string(),
            ));
        }
        let value_nav_units = vault_bridge_atoms_to_nav_value(
            remaining_atoms,
            &nav_asset.valuation_unit,
            &settlement_nav_asset.valuation_unit,
            settlement_asset.precision,
        )?;
        rows.push(NavSubscriptionReserveOverlayRow {
            allocation_id: allocation.allocation_id.clone(),
            settlement_asset_id: allocation.asset_id.clone(),
            bucket_id: allocation.bucket_id.clone(),
            receipt_id: allocation.receipt_id.clone(),
            amount_atoms: allocation.amount_atoms,
            released_atoms: allocation.released_atoms,
            remaining_atoms,
            value_nav_units,
            retired_at_height: allocation.retired_at_height,
            bucket_source_domain: bucket.source_domain.clone(),
            bucket_policy_hash: bucket.policy_hash.clone(),
            bucket_gross_receipt_atoms: bucket.gross_receipt_atoms,
            bucket_counted_value_atoms: bucket.counted_value_atoms,
            bucket_nav_subscription_allocations_atoms: bucket.nav_subscription_allocations_atoms,
            bucket_redemption_queue_atoms: bucket.redemption_queue_atoms,
            bucket_outstanding_vault_bridge_atoms: bucket.outstanding_vault_bridge_atoms,
            bucket_status: bucket.status.clone(),
        });
    }
    let mut primary_market_rows = Vec::new();
    let mut reserve_by_settlement_asset = std::collections::BTreeMap::<String, u64>::new();
    for route in ledger.pftl_uniswap_routes.iter().filter(|route| {
        route.native_nav_asset_id == nav_asset.asset_id && route.settlement_reserve_atoms != 0
    }) {
        let settlement_nav_asset = ledger.nav_asset(&route.settlement_asset_id).ok_or_else(|| {
            (
                "missing_primary_market_settlement_nav_asset",
                "primary-market NAV reserve overlay references an unregistered settlement NAV asset"
                    .to_string(),
            )
        })?;
        let settlement_asset = ledger
            .asset_definition(&route.settlement_asset_id)
            .ok_or_else(|| {
                (
                    "missing_primary_market_settlement_asset",
                    "primary-market NAV reserve overlay references a missing settlement asset"
                        .to_string(),
                )
            })?;
        let active_bucket_backing_atoms = ledger
            .vault_bridge_bucket_states
            .iter()
            .filter(|bucket| {
                bucket.asset_id == route.settlement_asset_id
                    && bucket.status == VAULT_BRIDGE_BUCKET_STATUS_ACTIVE
            })
            .try_fold(0_u64, |sum, bucket| {
                sum.checked_add(bucket.outstanding_vault_bridge_atoms)
                    .ok_or_else(|| {
                        (
                            "primary_market_backing_overflow",
                            "primary-market settlement bucket backing sum overflowed".to_string(),
                        )
                    })
            })?;
        let committed = reserve_by_settlement_asset
            .entry(route.settlement_asset_id.clone())
            .or_default();
        let available_active_backing_atoms = active_bucket_backing_atoms.saturating_sub(*committed);
        let proof_backed_reserve_atoms =
            route.settlement_reserve_atoms.min(available_active_backing_atoms);
        *committed = committed
            .checked_add(proof_backed_reserve_atoms)
            .ok_or_else(|| {
                (
                    "primary_market_reserve_overflow",
                    "primary-market settlement reserve sum overflowed".to_string(),
                )
            })?;
        let value_nav_units = vault_bridge_atoms_to_nav_value(
            proof_backed_reserve_atoms,
            &nav_asset.valuation_unit,
            &settlement_nav_asset.valuation_unit,
            settlement_asset.precision,
        )?;
        primary_market_rows.push(NavPrimaryMarketReserveOverlayRow {
            route_id: route.route_id.clone(),
            route_config_digest: route.route_config_digest.clone(),
            settlement_asset_id: route.settlement_asset_id.clone(),
            settlement_reserve_atoms: proof_backed_reserve_atoms,
            value_nav_units,
            active_bucket_backing_atoms,
            live_value_enabled: route.live_value_enabled,
            paused: route.paused,
        });
    }
    if rows.is_empty() && primary_market_rows.is_empty() {
        return Ok(None);
    }
    rows.sort_by(|left, right| left.allocation_id.cmp(&right.allocation_id));
    primary_market_rows.sort_by(|left, right| left.route_id.cmp(&right.route_id));
    let mut value_nav_units = 0_u64;
    let mut preimage = format!(
        "nav_asset_id={}\nnav_valuation_unit_bytes={}\nnav_valuation_unit={}\nallocation_count={}\nprimary_market_route_count={}\n",
        nav_asset.asset_id,
        nav_asset.valuation_unit.len(),
        nav_asset.valuation_unit,
        rows.len(),
        primary_market_rows.len(),
    );
    for (index, row) in rows.iter().enumerate() {
        value_nav_units = value_nav_units
            .checked_add(row.value_nav_units)
            .ok_or_else(|| {
                (
                    "nav_subscription_overlay_overflow",
                    "nav subscription overlay value would overflow".to_string(),
                )
            })?;
        preimage.push_str(&format!(
            "allocation[{index}].allocation_id={}\nallocation[{index}].settlement_asset_id={}\nallocation[{index}].bucket_id={}\nallocation[{index}].receipt_id={}\nallocation[{index}].amount_atoms={}\nallocation[{index}].released_atoms={}\nallocation[{index}].remaining_atoms={}\nallocation[{index}].value_nav_units={}\nallocation[{index}].retired_at_height={}\nallocation[{index}].bucket_source_domain_bytes={}\nallocation[{index}].bucket_source_domain={}\nallocation[{index}].bucket_policy_hash={}\nallocation[{index}].bucket_gross_receipt_atoms={}\nallocation[{index}].bucket_counted_value_atoms={}\nallocation[{index}].bucket_nav_subscription_allocations_atoms={}\nallocation[{index}].bucket_redemption_queue_atoms={}\nallocation[{index}].bucket_outstanding_vault_bridge_atoms={}\nallocation[{index}].bucket_status={}\n",
            row.allocation_id,
            row.settlement_asset_id,
            row.bucket_id,
            row.receipt_id,
            row.amount_atoms,
            row.released_atoms,
            row.remaining_atoms,
            row.value_nav_units,
            row.retired_at_height,
            row.bucket_source_domain.len(),
            row.bucket_source_domain,
            row.bucket_policy_hash,
            row.bucket_gross_receipt_atoms,
            row.bucket_counted_value_atoms,
            row.bucket_nav_subscription_allocations_atoms,
            row.bucket_redemption_queue_atoms,
            row.bucket_outstanding_vault_bridge_atoms,
            row.bucket_status,
        ));
    }
    for (index, row) in primary_market_rows.iter().enumerate() {
        value_nav_units = value_nav_units
            .checked_add(row.value_nav_units)
            .ok_or_else(|| {
                (
                    "nav_subscription_overlay_overflow",
                    "primary-market reserve overlay value would overflow".to_string(),
                )
            })?;
        preimage.push_str(&format!(
            "primary_market[{index}].route_id_bytes={}\nprimary_market[{index}].route_id={}\nprimary_market[{index}].route_config_digest={}\nprimary_market[{index}].settlement_asset_id={}\nprimary_market[{index}].settlement_reserve_atoms={}\nprimary_market[{index}].value_nav_units={}\nprimary_market[{index}].active_bucket_backing_atoms={}\nprimary_market[{index}].live_value_enabled={}\nprimary_market[{index}].paused={}\n",
            row.route_id.len(),
            row.route_id,
            row.route_config_digest,
            row.settlement_asset_id,
            row.settlement_reserve_atoms,
            row.value_nav_units,
            row.active_bucket_backing_atoms,
            row.live_value_enabled,
            row.paused,
        ));
    }
    Ok(Some(NavSubscriptionReserveOverlay {
        value_nav_units,
        source_root: hash_hex(
            "postfiat.nav_subscription_source_root.v1",
            preimage.as_bytes(),
        ),
    }))
}

/// Derive the finalized vault/primary-market reserve overlay committed by a
/// NAV asset's next reserve packet.
///
/// Packet builders and controlled migration rehearsals use this public wrapper
/// so their root/value calculation cannot drift from validator execution.
pub fn nav_subscription_reserve_overlay_for_asset(
    ledger: &LedgerState,
    asset_id: &str,
) -> Result<Option<NavSubscriptionReserveOverlay>, String> {
    let nav_asset = ledger
        .nav_asset(asset_id)
        .ok_or_else(|| "missing NAV asset for subscription reserve overlay".to_string())?;
    nav_subscription_reserve_overlay(ledger, nav_asset)
        .map_err(|(code, detail)| format!("{code}: {detail}"))
}

/// Combine a verified reserve proof with the exact finalized subscription
/// overlay using the same domain and encoding as validator verification.
pub fn derive_nav_reserve_subscription_composite_source_root_v1(
    values: &NavReservePublicValuesV1,
    overlay_source_root: &str,
    overlay_value: u64,
) -> Result<(String, u64), String> {
    nav_reserve_subscription_composite_source_root_v1(
        values,
        overlay_source_root,
        overlay_value,
    )
}

fn apply_nav_redeem_vault_bridge_settlement(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    nav_asset: &NavTrackedAsset,
    operation: &NavRedeemSettleOperation,
    redemption: &NavRedemption,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    let settlement_nav_asset = ledger
        .nav_asset(&operation.settlement_asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_vault_bridge_nav_asset",
                "nav redeem vault bridge settlement asset is not registered as a NAV asset"
                    .to_string(),
            )
        })?;
    let settlement_family_asset = ledger
        .asset_definition(&operation.settlement_asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_vault_bridge_asset",
                "nav redeem vault bridge settlement asset definition is missing".to_string(),
            )
        })?;
    let nav_asset_definition = ledger
        .asset_definition(&nav_asset.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset_definition",
                "nav redeem asset definition is missing".to_string(),
            )
        })?;
    let required_settlement_atoms = required_vault_bridge_settlement_atoms(
        redemption.amount,
        nav_asset_definition.precision,
        redemption.nav_per_unit,
        &nav_asset.valuation_unit,
        &settlement_nav_asset.valuation_unit,
        settlement_family_asset.precision,
    )?;
    if operation.settlement_amount_atoms != required_settlement_atoms {
        return Err((
            "nav_vault_bridge_settlement_amount_mismatch",
            "nav redeem vault bridge settlement amount must equal the valuation-adjusted redemption claim".to_string(),
        ));
    }

    let allocation_index = ledger
        .vault_bridge_allocations
        .iter()
        .position(|allocation| allocation.allocation_id == operation.settlement_allocation_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_allocation",
                "nav redeem references missing vault bridge settlement allocation".to_string(),
            )
        })?;
    let allocation = ledger.vault_bridge_allocations[allocation_index].clone();
    if allocation.asset_id != operation.settlement_asset_id {
        return Err((
            "vault_bridge_allocation_asset_mismatch",
            "nav redeem vault bridge settlement allocation asset mismatch".to_string(),
        ));
    }
    if allocation.bucket_id != operation.settlement_bucket_id {
        return Err((
            "vault_bridge_allocation_bucket_mismatch",
            "nav redeem vault bridge settlement allocation bucket mismatch".to_string(),
        ));
    }
    if allocation.purpose != VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION {
        return Err((
            "vault_bridge_allocation_wrong_purpose",
            "nav redeem requires a vault bridge nav_subscription allocation".to_string(),
        ));
    }
    if !nav_subscription_consumer_matches_recipient(
        &allocation.consumer_id,
        &operation.asset_id,
        &redemption.owner,
    ) {
        return Err((
            "vault_bridge_allocation_consumer_mismatch",
            "nav redeem vault bridge allocation is not bound to this NAV asset".to_string(),
        ));
    }
    if allocation.retired_at_height == 0 {
        return Err((
            "vault_bridge_allocation_not_retired",
            "nav redeem vault bridge settlement requires a retired nav_subscription allocation"
                .to_string(),
        ));
    }
    let release_capacity = allocation
        .amount_atoms
        .checked_sub(allocation.released_atoms)
        .ok_or_else(|| {
            (
                "bad_vault_bridge_allocation",
                "nav redeem allocation released atoms exceed amount".to_string(),
            )
        })?;
    if release_capacity == 0 {
        return Err((
            "vault_bridge_allocation_fully_released",
            "nav redeem vault bridge settlement allocation is already fully released".to_string(),
        ));
    }
    let release_atoms = release_capacity.min(operation.settlement_amount_atoms);
    let top_up_atoms = operation
        .settlement_amount_atoms
        .checked_sub(release_atoms)
        .ok_or_else(|| {
            (
                "nav_settlement_underflow",
                "nav redeem settlement release exceeds requested settlement".to_string(),
            )
        })?;

    let bucket_index = ledger
        .vault_bridge_bucket_states
        .iter()
        .position(|bucket| bucket.bucket_id == operation.settlement_bucket_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_bucket",
                "nav redeem vault bridge settlement bucket is missing".to_string(),
            )
        })?;
    let bucket = ledger.vault_bridge_bucket_states[bucket_index].clone();
    if bucket.asset_id != operation.settlement_asset_id {
        return Err((
            "vault_bridge_bucket_asset_mismatch",
            "nav redeem vault bridge settlement bucket asset mismatch".to_string(),
        ));
    }
    if bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE {
        return Err((
            "vault_bridge_bucket_not_active",
            "nav redeem vault bridge settlement requires an active source bucket".to_string(),
        ));
    }
    let settlement_profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &settlement_nav_asset,
        &bucket.source_domain,
        &bucket.policy_hash,
    )?;
    ensure_vault_bridge_source_policy(
        settlement_profile,
        &bucket.source_domain,
        &bucket.policy_hash,
    )?;
    let settlement_asset = if compatibility.pfusdc_source_series_active(block_height) {
        let mut matches = ledger.asset_definitions.iter().filter(|asset| {
            asset.asset_family_id == operation.settlement_asset_id
                && asset.source_bucket_id == operation.settlement_bucket_id
                && asset.asset_id == asset.source_series_id
        });
        let asset = matches.next().cloned().ok_or_else(|| {
            (
                "pfusdc_source_series_asset_missing",
                "NAV redemption requires the source-series settlement asset for its bucket"
                    .to_string(),
            )
        })?;
        if matches.next().is_some() {
            return Err((
                "pfusdc_source_series_asset_duplicate",
                "multiple source-series assets reference the same settlement bucket".to_string(),
            ));
        }
        asset
    } else {
        settlement_family_asset.clone()
    };
    if bucket.nav_subscription_allocations_atoms < release_atoms {
        return Err((
            "vault_bridge_bucket_underflow",
            "nav redeem settlement release exceeds bucket nav_subscription allocations".to_string(),
        ));
    }

    let receipt_index = ledger
        .vault_bridge_receipts
        .iter()
        .position(|receipt| receipt.receipt_id == allocation.receipt_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_receipt",
                "nav redeem vault bridge allocation references missing receipt".to_string(),
            )
        })?;
    let receipt = ledger.vault_bridge_receipts[receipt_index].clone();
    if receipt.asset_id != operation.settlement_asset_id
        || receipt.bucket_id != operation.settlement_bucket_id
        || receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
    {
        return Err((
            "vault_bridge_receipt_not_counted",
            "nav redeem vault bridge settlement allocation must reference a counted receipt in the settlement bucket"
                .to_string(),
        ));
    }
    let mut remaining_top_up_atoms = top_up_atoms;
    let mut top_up_plan: Vec<(usize, u64, VaultBridgeAllocation)> = Vec::new();
    if remaining_top_up_atoms != 0 {
        for (candidate_index, candidate_receipt) in ledger.vault_bridge_receipts.iter().enumerate()
        {
            if candidate_receipt.asset_id != operation.settlement_asset_id
                || candidate_receipt.bucket_id != operation.settlement_bucket_id
                || candidate_receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
            {
                continue;
            }
            let available = candidate_receipt
                .available_counted_value()
                .map_err(|error| ("bad_vault_bridge_receipt", error))?;
            if available == 0 {
                continue;
            }
            let planned_atoms = remaining_top_up_atoms.min(available);
            let top_up_allocation = VaultBridgeAllocation::new(
                &genesis.chain_id,
                candidate_receipt.receipt_id.clone(),
                operation.settlement_asset_id.clone(),
                operation.settlement_bucket_id.clone(),
                planned_atoms,
                VAULT_BRIDGE_ALLOCATION_PURPOSE_REDEMPTION,
                nav_redemption_consumer_id(&redemption.redemption_id),
                block_height,
            )
            .map_err(|error| ("bad_vault_bridge_allocation", error))?;
            if ledger
                .vault_bridge_allocation(&top_up_allocation.allocation_id)
                .is_some()
                || top_up_plan
                    .iter()
                    .any(|(_, _, planned)| planned.allocation_id == top_up_allocation.allocation_id)
            {
                return Err((
                    "duplicate_vault_bridge_allocation",
                    "nav redeem settlement top-up allocation already exists".to_string(),
                ));
            }
            top_up_plan.push((candidate_index, planned_atoms, top_up_allocation));
            remaining_top_up_atoms -= planned_atoms;
            if remaining_top_up_atoms == 0 {
                break;
            }
        }
        if remaining_top_up_atoms != 0 {
            return Err((
                "insufficient_vault_bridge_receipt_capacity",
                "nav redeem settlement exceeds released subscription collateral plus unallocated counted receipt capacity"
                    .to_string(),
            ));
        }
    }

    let mut return_supply_by_receipt = std::collections::BTreeMap::<String, u64>::new();
    if release_atoms != 0 {
        return_supply_by_receipt.insert(allocation.receipt_id.clone(), release_atoms);
    }
    for (_, amount, top_up_allocation) in &top_up_plan {
        let total = return_supply_by_receipt
            .entry(top_up_allocation.receipt_id.clone())
            .or_default();
        *total = total.checked_add(*amount).ok_or_else(|| {
            (
                "vault_bridge_allocation_overflow",
                "NAV redemption return supply allocation overflowed".to_string(),
            )
        })?;
    }
    let mut return_supply_allocations = Vec::new();
    if compatibility.pfusdc_source_series_active(block_height) {
        for (receipt_id, amount) in return_supply_by_receipt {
            let supply_allocation = VaultBridgeAllocation::new(
                &genesis.chain_id,
                receipt_id.clone(),
                operation.settlement_asset_id.clone(),
                operation.settlement_bucket_id.clone(),
                amount,
                VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,
                nav_redemption_supply_consumer_id(&redemption.redemption_id, &receipt_id),
                block_height,
            )
            .map_err(|error| ("bad_vault_bridge_allocation", error))?;
            if ledger
                .vault_bridge_allocation(&supply_allocation.allocation_id)
                .is_some()
                || return_supply_allocations.iter().any(
                    |existing: &VaultBridgeAllocation| {
                        existing.allocation_id == supply_allocation.allocation_id
                    },
                )
            {
                return Err((
                    "duplicate_vault_bridge_allocation",
                    "NAV redemption return supply allocation already exists".to_string(),
                ));
            }
            return_supply_allocations.push(supply_allocation);
        }
    }

    let to_index = issued_asset_credit_recipient_line_index(
        ledger,
        &settlement_asset,
        &redemption.owner,
        operation.settlement_amount_atoms,
        "nav redeem settlement",
    )?;
    let (recipient_after, recipient_required) = prepare_issued_asset_credit(
        ledger,
        &settlement_asset,
        &redemption.owner,
        to_index,
        operation.settlement_amount_atoms,
        "nav redeem settlement",
    )?;
    let supply_after = issued_asset_family_supply(ledger, &operation.settlement_asset_id)?
        .checked_add(operation.settlement_amount_atoms)
        .ok_or_else(|| {
            (
                "issued_supply_overflow",
                "nav redeem settlement would overflow settlement asset supply".to_string(),
            )
        })?;
    if let Some(max_supply) = settlement_family_asset.max_supply {
        if supply_after > max_supply {
            return Err((
                "issued_supply_cap_exceeded",
                "nav redeem settlement exceeds settlement asset max_supply".to_string(),
            ));
        }
    }

    let mut bucket_after = bucket;
    bucket_after.nav_subscription_allocations_atoms -= release_atoms;
    bucket_after.outstanding_vault_bridge_atoms = bucket_after
        .outstanding_vault_bridge_atoms
        .checked_add(operation.settlement_amount_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_bucket_overflow",
                "nav redeem settlement bucket outstanding supply would overflow".to_string(),
            )
        })?;
    bucket_after.last_updated_height = block_height;
    bucket_after
        .validate()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;

    let released_after = ledger.vault_bridge_allocations[allocation_index]
        .released_atoms
        .checked_add(release_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_allocation_overflow",
                "nav redeem settlement allocation released atoms would overflow".to_string(),
            )
        })?;
    ledger.vault_bridge_allocations[allocation_index].released_atoms = released_after;
    ledger.vault_bridge_allocations[allocation_index]
        .validate()
        .map_err(|error| ("bad_vault_bridge_allocation", error))?;

    for (top_up_receipt_index, top_up_amount, mut top_up_allocation) in top_up_plan {
        ledger.vault_bridge_receipts[top_up_receipt_index].allocated_value_atoms = ledger
            .vault_bridge_receipts[top_up_receipt_index]
            .allocated_value_atoms
            .checked_add(top_up_amount)
            .ok_or_else(|| {
                (
                    "vault_bridge_receipt_overflow",
                    "nav redeem settlement receipt allocated value would overflow".to_string(),
                )
            })?;
        ledger.vault_bridge_receipts[top_up_receipt_index]
            .validate()
            .map_err(|error| ("bad_vault_bridge_receipt", error))?;
        if compatibility.pfusdc_source_series_active(block_height) {
            top_up_allocation.released_atoms = top_up_allocation.amount_atoms;
            top_up_allocation.retired_at_height = block_height;
            top_up_allocation
                .validate()
                .map_err(|error| ("bad_vault_bridge_allocation", error))?;
        }
        ledger.vault_bridge_allocations.push(top_up_allocation);
    }

    ledger
        .vault_bridge_allocations
        .extend(return_supply_allocations);

    ledger.vault_bridge_bucket_states[bucket_index] = bucket_after;
    apply_prepared_issued_asset_credit(ledger, to_index, recipient_after, recipient_required);
    Ok(())
}

fn nav_sp1_subscription_source_root(
    nav_asset: &NavTrackedAsset,
    profile: &NavProofProfile,
    decoded: &DecodedSp1PublicValues,
    sp1_public_values: &[u8],
    overlay: &NavSubscriptionReserveOverlay,
) -> Result<String, (&'static str, String)> {
    let total_verified_net_assets = decoded
        .verified_net_assets
        .checked_add(overlay.value_nav_units)
        .ok_or_else(|| {
            (
                "nav_subscription_overlay_overflow",
                "nav subscription overlay value would overflow SP1 base assets".to_string(),
            )
        })?;
    let sp1_public_values_hash = hash_hex("postfiat.nav_sp1_public_values.v1", sp1_public_values);
    let preimage = format!(
        "asset_id={}\nprofile_id={}\nprofile_source_class_bytes={}\nprofile_source_class={}\npolicy_hash={}\nsp1_public_values_hash={}\nsp1_verified_net_assets={}\nsubscription_overlay_source_root={}\nsubscription_overlay_value_nav_units={}\ntotal_verified_net_assets={}\n",
        nav_asset.asset_id,
        profile.profile_id,
        profile.source_class.len(),
        profile.source_class,
        decoded.policy_hash_hex,
        sp1_public_values_hash,
        decoded.verified_net_assets,
        overlay.source_root,
        overlay.value_nav_units,
        total_verified_net_assets,
    );
    Ok(hash_hex(
        "postfiat.nav_sp1_subscription_composite_source_root.v1",
        preimage.as_bytes(),
    ))
}

fn apply_nav_mint_vault_bridge_settlement(
    ledger: &mut LedgerState,
    nav_asset: &NavTrackedAsset,
    operation: &NavMintAtNavOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let settlement_nav_asset = ledger
        .nav_asset(&operation.settlement_asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_vault_bridge_nav_asset",
                "nav mint vault bridge asset settlement asset is not registered as a NAV asset"
                    .to_string(),
            )
        })?;
    let settlement_asset = ledger
        .asset_definition(&operation.settlement_asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_vault_bridge_asset",
                "nav mint vault bridge asset settlement asset definition is missing".to_string(),
            )
        })?;
    let nav_asset_definition = ledger
        .asset_definition(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset_definition",
                "nav mint asset definition is missing".to_string(),
            )
        })?;
    let required_settlement_atoms = required_vault_bridge_settlement_atoms(
        operation.amount,
        nav_asset_definition.precision,
        nav_asset.nav_per_unit,
        &nav_asset.valuation_unit,
        &settlement_nav_asset.valuation_unit,
        settlement_asset.precision,
    )?;
    if operation.settlement_amount_atoms != required_settlement_atoms {
        return Err((
            "nav_vault_bridge_settlement_amount_mismatch",
            "nav mint vault bridge asset settlement amount must equal the valuation-adjusted NAV amount".to_string(),
        ));
    }

    let allocation_index = ledger
        .vault_bridge_allocations
        .iter()
        .position(|allocation| allocation.allocation_id == operation.settlement_allocation_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_allocation",
                "nav mint references missing vault bridge asset settlement allocation".to_string(),
            )
        })?;
    let allocation = ledger.vault_bridge_allocations[allocation_index].clone();
    if allocation.asset_id != operation.settlement_asset_id {
        return Err((
            "vault_bridge_allocation_asset_mismatch",
            "nav mint vault bridge asset settlement allocation asset mismatch".to_string(),
        ));
    }
    if allocation.bucket_id != operation.settlement_bucket_id {
        return Err((
            "vault_bridge_allocation_bucket_mismatch",
            "nav mint vault bridge asset settlement allocation bucket mismatch".to_string(),
        ));
    }
    if allocation.amount_atoms != operation.settlement_amount_atoms {
        return Err((
            "vault_bridge_allocation_amount_mismatch",
            "nav mint vault bridge asset settlement allocation amount mismatch".to_string(),
        ));
    }
    if allocation.purpose != VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION {
        return Err((
            "vault_bridge_allocation_wrong_purpose",
            "nav mint requires a vault bridge asset nav_subscription allocation".to_string(),
        ));
    }
    if !nav_subscription_consumer_matches_recipient(
        &allocation.consumer_id,
        &operation.asset_id,
        &operation.to,
    ) {
        return Err((
            "vault_bridge_allocation_consumer_mismatch",
            "nav mint vault bridge asset allocation is not bound to this NAV asset".to_string(),
        ));
    }
    if allocation.retired_at_height != 0 {
        return Err((
            "vault_bridge_allocation_retired",
            "nav mint vault bridge asset settlement allocation is already retired".to_string(),
        ));
    }

    let bucket = ledger
        .vault_bridge_bucket(&operation.settlement_bucket_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_bucket",
                "nav mint vault bridge asset settlement bucket is missing".to_string(),
            )
        })?;
    if bucket.asset_id != operation.settlement_asset_id {
        return Err((
            "vault_bridge_bucket_asset_mismatch",
            "nav mint vault bridge asset settlement bucket asset mismatch".to_string(),
        ));
    }
    if bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE {
        return Err((
            "vault_bridge_bucket_not_active",
            "nav mint vault bridge asset settlement requires an active source bucket".to_string(),
        ));
    }
    let settlement_profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &settlement_nav_asset,
        &bucket.source_domain,
        &bucket.policy_hash,
    )?;
    ensure_vault_bridge_source_policy(
        settlement_profile,
        &bucket.source_domain,
        &bucket.policy_hash,
    )?;
    bucket
        .validate()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;

    let receipt = ledger
        .vault_bridge_receipt(&allocation.receipt_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_receipt",
                "nav mint vault bridge asset allocation references missing receipt".to_string(),
            )
        })?;
    if receipt.asset_id != operation.settlement_asset_id
        || receipt.bucket_id != operation.settlement_bucket_id
        || receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
    {
        return Err((
            "vault_bridge_receipt_not_counted",
            "nav mint vault bridge asset settlement allocation must reference a counted receipt in the settlement bucket"
                .to_string(),
        ));
    }

    ledger.vault_bridge_allocations[allocation_index].retired_at_height = block_height;
    ledger.vault_bridge_allocations[allocation_index]
        .validate()
        .map_err(|error| ("bad_vault_bridge_allocation", error))?;
    Ok(())
}

fn apply_vault_bridge_receipt_submit(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &VaultBridgeReceiptSubmitOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!(
                    "vault bridge asset asset `{}` is not registered as a NAV asset",
                    operation.asset_id
                ),
            )
        })?;
    ensure_vault_bridge_asset_policy(ledger, &nav_asset, &operation.operator)?;
    let profile = vault_bridge_profile_for_asset(ledger, &nav_asset)?;
    ensure_vault_bridge_source_policy(profile, &operation.source_domain, &operation.policy_hash)?;
    if operation.claim_type != VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT {
        return Err((
            "unsupported_vault_bridge_claim_type",
            "vault bridge asset receipts must be backed by a bridge_deposit vault event"
                .to_string(),
        ));
    }
    if operation.expires_at_height == 0 || operation.expires_at_height <= block_height {
        return Err((
            "vault_bridge_receipt_expired",
            "vault bridge asset receipt expiry must be greater than the current block height"
                .to_string(),
        ));
    }

    let receipt = VaultBridgeReceipt::new(
        &genesis.chain_id,
        operation.asset_id.clone(),
        operation.source_domain.clone(),
        operation.source_asset.clone(),
        operation.claim_type.clone(),
        operation.amount_atoms,
        operation.source_tx_or_attestation.clone(),
        operation.finality_ref.clone(),
        operation.vault_id.clone(),
        operation.policy_hash.clone(),
        block_height,
        operation.expires_at_height,
        operation.bridge_deposit_evidence.clone(),
    )
    .map_err(|error| ("bad_vault_bridge_receipt", error))?;
    if ledger.vault_bridge_receipt(&receipt.receipt_id).is_some() {
        return Err((
            "duplicate_vault_bridge_receipt",
            "vault bridge asset receipt already exists".to_string(),
        ));
    }
    ledger.vault_bridge_receipts.push(receipt);
    Ok(())
}

fn apply_vault_bridge_deposit_propose_with_genesis(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &VaultBridgeDepositProposeOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!(
                    "vault bridge asset bridge deposit asset `{}` is not registered as a NAV asset",
                    operation.asset_id
                ),
            )
        })?;
    let profile = vault_bridge_profile_for_asset(ledger, &nav_asset)?;
    let source_domain = operation.evidence.source_domain();
    ensure_vault_bridge_deposit_admission_hygiene(profile, &operation.evidence)?;
    ensure_vault_bridge_source_policy(profile, &source_domain, &operation.policy_hash)?;
    if operation.expires_at_height <= block_height {
        return Err((
            "vault_bridge_deposit_expired",
            "vault bridge asset bridge deposit expiry must be greater than the current block height"
                .to_string(),
        ));
    }
    let expected_root = vault_bridge_deposit_evidence_root(&operation.evidence)
        .map_err(|error| ("bad_vault_bridge_deposit_evidence", error))?;
    if operation.evidence_root != expected_root {
        return Err((
            "vault_bridge_deposit_evidence_root_mismatch",
            "vault bridge asset bridge deposit evidence_root must match deterministic vault event evidence"
                .to_string(),
        ));
    }
    if ledger.vault_bridge_deposits.iter().any(|record| {
        record.asset_id == operation.asset_id
            && record.evidence.deposit_id == operation.evidence.deposit_id
    }) {
        return Err((
            "duplicate_vault_bridge_deposit_id",
            "vault bridge asset bridge deposit deposit_id is already proposed".to_string(),
        ));
    }
    if ledger
        .vault_bridge_deposit(&operation.asset_id, &operation.evidence_root)
        .is_some()
    {
        return Err((
            "duplicate_vault_bridge_deposit",
            "vault bridge asset bridge deposit evidence is already proposed".to_string(),
        ));
    }
    let fast_ingress_verifier = ledger
        .ethereum_arbitrum_finality_states
        .iter()
        .find(|state| state.route_profile_hash == operation.policy_hash)
        .and_then(|state| state.fast_ingress_verifier.as_ref());
    let proof_public_values =
        ensure_vault_bridge_deposit_source_proof(VaultBridgeDepositSourceProof {
            genesis: Some(genesis),
            profile,
            evidence: &operation.evidence,
            evidence_root: &expected_root,
            policy_hash: &operation.policy_hash,
            source_proof_kind: &operation.source_proof_kind,
            source_proof_hash: &operation.source_proof_hash,
            source_public_values_hash: &operation.source_public_values_hash,
            source_proof_bytes: &operation.source_proof_bytes,
            source_public_values: &operation.source_public_values,
            fast_ingress_verifier,
        })?;
    let mut advanced_finality = None;
    let mut advanced_campaign = None;
    let mut source_nullifier = String::new();
    if let Some(public_values) = proof_public_values {
        match public_values {
            VerifiedIngressPublicValues::Confirmed(values) => {
                let mut state = ledger
                    .ethereum_arbitrum_finality_state_mut(
                        &operation.policy_hash,
                        values.route_epoch,
                    )
                    .ok_or_else(|| {
                        (
                            "pfusdc_finality_state_missing",
                            "proof-native pfUSDC ingress requires a governance-pinned Ethereum/Arbitrum finality state"
                                .to_string(),
                        )
                    })?
                    .clone();
                state
                    .verify_and_advance(&values)
                    .map_err(|error| ("pfusdc_finality_state_rejected", error))?;
                advanced_finality = Some(state);
            }
            VerifiedIngressPublicValues::Bonded(values) => {
                let mut state = ledger
                    .ethereum_arbitrum_finality_state_mut(
                        &operation.policy_hash,
                        values.route_epoch,
                    )
                    .ok_or_else(|| {
                        (
                            "pfusdc_finality_state_missing",
                            "proof-native pfUSDC ingress requires a governance-pinned Ethereum/Arbitrum finality state"
                                .to_string(),
                        )
                    })?
                    .clone();
                state
                    .verify_and_advance_bonded(&values)
                    .map_err(|error| ("pfusdc_bonded_finality_state_rejected", error))?;
                if ledger.fast_ingress_campaigns.iter().any(|campaign| {
                    campaign
                        .mints
                        .iter()
                        .any(|mint| mint.deposit_key == values.deposit_key)
                }) {
                    return Err((
                        "pfusdc_fast_ingress_global_replay",
                        "fast-ingress deposit replay key was consumed in another route epoch"
                            .to_string(),
                    ));
                }
                let global_exposure =
                    ledger
                        .fast_ingress_campaigns
                        .iter()
                        .try_fold(0_u64, |total, campaign| {
                            total
                                .checked_add(campaign.exposure_total_atoms)
                                .ok_or_else(|| {
                                    (
                                        "pfusdc_fast_ingress_exposure_overflow",
                                        "global fast-ingress exposure overflow".to_string(),
                                    )
                                })
                        })?;
                if global_exposure
                    .checked_add(values.amount_atoms)
                    .ok_or_else(|| {
                        (
                            "pfusdc_fast_ingress_exposure_overflow",
                            "global fast-ingress exposure overflow".to_string(),
                        )
                    })?
                    > values.cap_atoms
                {
                    return Err((
                        "pfusdc_fast_ingress_global_cap_exceeded",
                        "fast-ingress mint would exceed the global campaign cap".to_string(),
                    ));
                }
                let mut campaign = ledger
                    .fast_ingress_campaign(&operation.policy_hash, values.route_epoch)
                    .cloned()
                    .map(Ok)
                    .unwrap_or_else(|| {
                        postfiat_types::FastIngressCampaignStateV1::from_public_values(&values)
                    })
                    .map_err(|error| ("pfusdc_fast_ingress_campaign_invalid", error))?;
                campaign
                    .accept_escrowed_mint(&values, block_height)
                    .map_err(|error| ("pfusdc_fast_ingress_escrow_rejected", error))?;
                advanced_campaign = Some(campaign);
                advanced_finality = Some(state);
            }
            VerifiedIngressPublicValues::Ethereum { nullifier } => {
                source_nullifier = nullifier;
            }
        }
    }
    if !source_nullifier.is_empty()
        && ledger
            .vault_bridge_deposits
            .iter()
            .any(|record| record.source_nullifier == source_nullifier)
    {
        return Err((
            "duplicate_vault_bridge_source_nullifier",
            "Ethereum ingress source nullifier is already recorded".to_string(),
        ));
    }
    let record = VaultBridgeDepositRecord::new_with_source_nullifier(
        operation.asset_id.clone(),
        operation.evidence_root.clone(),
        operation.evidence.clone(),
        operation.policy_hash.clone(),
        operation.source_proof_kind.clone(),
        operation.source_proof_hash.clone(),
        operation.source_public_values_hash.clone(),
        source_nullifier,
        operation.proposer.clone(),
        block_height,
        operation.expires_at_height,
    )
    .map_err(|error| ("bad_vault_bridge_deposit", error))?;
    if let Some(state) = advanced_finality {
        let current = ledger
            .ethereum_arbitrum_finality_state_mut(&state.route_profile_hash, state.route_epoch)
            .ok_or_else(|| {
                (
                    "pfusdc_finality_state_missing",
                    "proof-native finality state disappeared during commit".to_string(),
                )
            })?;
        *current = state;
    }
    if let Some(campaign) = advanced_campaign {
        if let Some(current) =
            ledger.fast_ingress_campaign_mut(&campaign.route_profile_hash, campaign.route_epoch)
        {
            *current = campaign;
        } else {
            ledger.fast_ingress_campaigns.push(campaign);
        }
    }
    ledger.vault_bridge_deposits.push(record);
    Ok(())
}

#[cfg(test)]
fn apply_vault_bridge_deposit_propose(
    ledger: &mut LedgerState,
    operation: &VaultBridgeDepositProposeOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let genesis = Genesis::new("postfiat-execution-test");
    apply_vault_bridge_deposit_propose_with_genesis(&genesis, ledger, operation, block_height)
}

fn ensure_vault_bridge_deposit_admission_hygiene(
    profile: &NavProofProfile,
    evidence: &VaultBridgeDepositEvidence,
) -> Result<(), (&'static str, String)> {
    if evidence.amount_atoms == 0 {
        return Err((
            "vault_bridge_zero_amount",
            "vault bridge asset bridge deposit amount_atoms must be nonzero".to_string(),
        ));
    }
    let Some(profile_source_domain) = profile
        .source_class
        .strip_prefix(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
    else {
        return Err((
            "nav_profile_not_vault_bridge",
            "vault bridge asset operation requires a vault_bridge:<source_domain> profile"
                .to_string(),
        ));
    };
    let Some((profile_chain_id, profile_vault, profile_token)) =
        parse_vault_bridge_source_domain_parts(profile_source_domain)
    else {
        return Err((
            "bad_vault_bridge_profile",
            "vault bridge source_domain must be erc20_bridge_vault:<chain_id>:<vault>:<token>"
                .to_string(),
        ));
    };
    if evidence.source_chain_id != profile_chain_id {
        return Err((
            "vault_bridge_finality_ref_chain_id_mismatch",
            "vault bridge asset bridge deposit chain_id must match evidence finality_ref and source_domain"
                .to_string(),
        ));
    }
    if evidence.vault_address != profile_vault || evidence.token_address != profile_token {
        return Err((
            "vault_bridge_evidence_policy_mismatch",
            "vault bridge asset bridge deposit vault/token must match the bucket policy"
                .to_string(),
        ));
    }
    Ok(())
}

fn parse_vault_bridge_source_domain_parts(source_domain: &str) -> Option<(u64, String, String)> {
    let mut parts = source_domain.split(':');
    let prefix = parts.next()?;
    let chain_id = parts.next()?.parse::<u64>().ok()?;
    let vault = parts.next()?;
    let token = parts.next()?;
    if parts.next().is_some()
        || prefix != VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX
        || chain_id == 0
        || !is_lowercase_evm_address(vault)
        || !is_lowercase_evm_address(token)
    {
        return None;
    }
    Some((chain_id, vault.to_string(), token.to_string()))
}

fn is_lowercase_evm_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && !value[2..].bytes().all(|byte| byte == b'0')
        && value[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn ensure_vault_bridge_deposit_observation_matches(
    evidence: &VaultBridgeDepositEvidence,
    observation_root: &str,
    observation: &VaultBridgeDepositObservation,
    min_confirmations: u64,
) -> Result<(), (&'static str, String)> {
    let expected_root = vault_bridge_deposit_observation_root(observation)
        .map_err(|error| ("bad_vault_bridge_deposit_observation", error))?;
    if observation_root != expected_root {
        return Err((
            "vault_bridge_deposit_observation_root_mismatch",
            "vault bridge deposit observation_root must match canonical observation facts"
                .to_string(),
        ));
    }
    if !observation.tx_exists {
        return Err((
            "vault_bridge_deposit_observation_unknown",
            "vault bridge deposit observer did not confirm that the source transaction exists"
                .to_string(),
        ));
    }
    if observation.receipt_status != 1 {
        return Err((
            "vault_bridge_deposit_observation_not_successful",
            "vault bridge deposit observer did not confirm receipt status 1".to_string(),
        ));
    }
    let exact_match = observation.source_chain_id == evidence.source_chain_id
        && observation.vault_address == evidence.vault_address
        && observation.token_address == evidence.token_address
        && observation.depositor == evidence.depositor
        && observation.amount_atoms == evidence.amount_atoms
        && observation.deposit_id == evidence.deposit_id
        && observation.block_hash == evidence.block_hash
        && observation.tx_hash == evidence.tx_hash
        && observation.log_index == evidence.log_index;
    if !exact_match {
        return Err((
            "vault_bridge_deposit_observation_mismatch",
            "vault bridge deposit observation must exactly match evidence chain/vault/token/depositor/amount/block/tx/log fields"
                .to_string(),
        ));
    }
    if observation.confirmation_depth < min_confirmations {
        return Err((
            "vault_bridge_deposit_confirmation_depth_too_low",
            format!(
                "vault bridge deposit observation depth {} is below profile minimum {min_confirmations}",
                observation.confirmation_depth
            ),
        ));
    }
    Ok(())
}

fn ensure_vault_bridge_withdrawal_observation_matches(
    redemption: &VaultBridgeRedemption,
    settlement_receipt_hash: &str,
    attestation: &VaultBridgeWithdrawalExecutionAttestation,
    min_confirmations: u64,
) -> Result<(), (&'static str, String)> {
    let observation = &attestation.observation;
    let expected_root = vault_bridge_withdrawal_execution_observation_root(observation)
        .map_err(|error| ("bad_vault_bridge_withdrawal_observation", error))?;
    if attestation.observation_root != expected_root {
        return Err((
            "vault_bridge_withdrawal_observation_root_mismatch",
            "vault bridge withdrawal observation_root must match canonical observation facts"
                .to_string(),
        ));
    }
    if settlement_receipt_hash != expected_root {
        return Err((
            "vault_bridge_withdrawal_settlement_receipt_mismatch",
            "vault bridge settlement_receipt_hash must equal the observed withdrawal execution root"
                .to_string(),
        ));
    }
    if !observation.tx_exists {
        return Err((
            "vault_bridge_withdrawal_observation_unknown",
            "vault bridge withdrawal observer did not confirm that the source transaction exists"
                .to_string(),
        ));
    }
    if observation.receipt_status != 1 {
        return Err((
            "vault_bridge_withdrawal_observation_not_successful",
            "vault bridge withdrawal observer did not confirm receipt status 1".to_string(),
        ));
    }
    let exact_match = observation.source_chain_id == redemption.withdrawal_packet.source_chain_id
        && observation.vault_address == redemption.withdrawal_packet.vault_address
        && observation.token_address == redemption.withdrawal_packet.token_address
        && observation.recipient == redemption.withdrawal_packet.recipient
        && observation.amount_atoms == redemption.amount_atoms
        && observation.withdrawal_id == redemption.redemption_id
        && observation.withdrawal_packet_hash == redemption.withdrawal_packet_hash;
    if !exact_match {
        return Err((
            "vault_bridge_withdrawal_observation_mismatch",
            "vault bridge withdrawal observation must exactly match redemption packet chain/vault/token/recipient/amount/id/hash fields"
                .to_string(),
        ));
    }
    if observation.confirmation_depth < min_confirmations {
        return Err((
            "vault_bridge_withdrawal_confirmation_depth_too_low",
            format!(
                "vault bridge withdrawal observation depth {} is below profile minimum {min_confirmations}",
                observation.confirmation_depth
            ),
        ));
    }
    Ok(())
}

fn verify_vault_bridge_withdrawal_attestation_signature(
    ledger: &LedgerState,
    attestation: &VaultBridgeWithdrawalExecutionAttestation,
) -> Result<(), (&'static str, String)> {
    if ledger.nav_attestor(&attestation.attestor).is_none() {
        return Err((
            "unregistered_nav_attestor",
            "vault bridge withdrawal observation requires a registered attestor".to_string(),
        ));
    }
    let account = ledger.account(&attestation.attestor).ok_or_else(|| {
        (
            "vault_bridge_withdrawal_attestor_key_missing",
            "vault bridge withdrawal observer account is missing".to_string(),
        )
    })?;
    let public_key_hex = account.public_key_hex.as_ref().ok_or_else(|| {
        (
            "vault_bridge_withdrawal_attestor_key_missing",
            "vault bridge withdrawal observer account has no public key".to_string(),
        )
    })?;
    let public_key = hex_to_bytes(public_key_hex).map_err(|error| {
        (
            "bad_vault_bridge_withdrawal_attestor_key",
            error.to_string(),
        )
    })?;
    let signature = hex_to_bytes(&attestation.signature_hex)
        .map_err(|error| ("bad_vault_bridge_withdrawal_signature", error.to_string()))?;
    if !ml_dsa_65_verify(
        &public_key,
        &attestation.observation.signing_bytes(),
        &signature,
    ) {
        return Err((
            "vault_bridge_withdrawal_observer_signature_invalid",
            "vault bridge withdrawal observer signature does not verify".to_string(),
        ));
    }
    Ok(())
}

fn ensure_vault_bridge_withdrawal_observation_quorum(
    ledger: &LedgerState,
    profile: &NavProofProfile,
    redemption: &VaultBridgeRedemption,
    operation: &VaultBridgeRedeemSettleOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if profile.bridge_observer_min_confirmations == 0
        || !compatibility.bridge_verification_rules_active(block_height)
    {
        return Ok(());
    }
    let observation_count = operation.withdrawal_observations.len() as u64;
    if observation_count < profile.min_attestations {
        return Err((
            "vault_bridge_withdrawal_observation_quorum_not_met",
            format!(
                "vault bridge withdrawal settlement has {observation_count} observer attestation(s); profile requires {}",
                profile.min_attestations
            ),
        ));
    }
    for attestation in &operation.withdrawal_observations {
        ensure_vault_bridge_withdrawal_observation_matches(
            redemption,
            &operation.settlement_receipt_hash,
            attestation,
            profile.bridge_observer_min_confirmations,
        )?;
        verify_vault_bridge_withdrawal_attestation_signature(ledger, attestation)?;
    }
    Ok(())
}

fn apply_vault_bridge_deposit_challenge(
    ledger: &mut LedgerState,
    operation: &VaultBridgeDepositChallengeOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let record = ledger
        .vault_bridge_deposit(&operation.asset_id, &operation.evidence_root)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_vault_bridge_deposit",
                "vault bridge asset bridge deposit challenge references missing evidence"
                    .to_string(),
            )
        })?;
    if record.status == VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED {
        return Err((
            "finalized_vault_bridge_deposit",
            "finalized vault bridge asset bridge deposit evidence is not challengeable".to_string(),
        ));
    }
    if record.status == VAULT_BRIDGE_DEPOSIT_STATUS_CHALLENGED {
        return Err((
            "vault_bridge_deposit_already_challenged",
            "vault bridge asset bridge deposit evidence is already challenged".to_string(),
        ));
    }
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                "vault bridge asset bridge deposit challenge references missing NAV asset"
                    .to_string(),
            )
        })?;
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &record.evidence.source_domain(),
        &record.policy_hash,
    )?
    .clone();
    if vault_bridge_source_proof_is_consensus_verified(&record.source_proof_kind) {
        return Err((
            "vault_bridge_deposit_consensus_verified",
            "cryptographically verified vault bridge deposits are verified by consensus at proposal and are not challengeable"
                .to_string(),
        ));
    }
    if profile.challenge_window_blocks == 0 {
        return Err((
            "vault_bridge_deposit_not_challengeable",
            "vault bridge asset bridge deposit profile has no challenge window".to_string(),
        ));
    }
    let challenge_window_close = record
        .submitted_at_height
        .checked_add(profile.challenge_window_blocks)
        .ok_or_else(|| {
            (
                "vault_bridge_deposit_height_overflow",
                "vault bridge asset bridge deposit challenge window height overflowed".to_string(),
            )
        })?;
    if block_height >= challenge_window_close {
        return Err((
            "vault_bridge_deposit_challenge_window_closed",
            format!(
                "vault bridge asset bridge deposit challenge window closed at height {challenge_window_close}"
            ),
        ));
    }
    if operation.bond < profile.min_challenge_bond {
        return Err((
            "challenge_bond_too_low",
            format!(
                "challenge bond {} is below profile minimum {}",
                operation.bond, profile.min_challenge_bond
            ),
        ));
    }
    if operation.bond > 0 {
        let challenger_account = ledger.account_mut(&operation.challenger).ok_or_else(|| {
            (
                "missing_challenger_account",
                "vault bridge asset bridge deposit challenger account does not exist".to_string(),
            )
        })?;
        let balance_after = challenger_account
            .balance
            .checked_sub(operation.bond)
            .ok_or_else(|| {
                (
                    "insufficient_funds",
                    "challenger balance is too low for challenge bond".to_string(),
                )
            })?;
        if let Some(message) = account_reserve_violation(&operation.challenger, balance_after) {
            return Err(("below_account_reserve", message));
        }
        challenger_account.balance = balance_after;
    }
    let record = ledger
        .vault_bridge_deposit_mut(&operation.asset_id, &operation.evidence_root)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_deposit",
                "vault bridge asset bridge deposit disappeared during challenge".to_string(),
            )
        })?;
    record.status = VAULT_BRIDGE_DEPOSIT_STATUS_CHALLENGED.to_string();
    record.challenger = operation.challenger.clone();
    record.challenge_hash = operation.challenge_hash.clone();
    record.challenge_bond = operation.bond;
    record
        .validate()
        .map_err(|error| ("bad_vault_bridge_deposit", error))?;
    Ok(())
}

#[allow(dead_code)]
fn apply_vault_bridge_deposit_attest(
    ledger: &mut LedgerState,
    operation: &VaultBridgeDepositAttestOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    apply_vault_bridge_deposit_attest_with_compatibility(
        ledger,
        operation,
        block_height,
        AssetExecutionCompatibility::strict(),
    )
}

fn apply_vault_bridge_deposit_attest_with_compatibility(
    ledger: &mut LedgerState,
    operation: &VaultBridgeDepositAttestOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    let record = ledger
        .vault_bridge_deposit(&operation.asset_id, &operation.evidence_root)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_vault_bridge_deposit",
                "vault bridge asset bridge deposit attestation references missing evidence"
                    .to_string(),
            )
        })?;
    if record.status != VAULT_BRIDGE_DEPOSIT_STATUS_PENDING {
        return Err((
            "vault_bridge_deposit_not_pending",
            "vault bridge asset bridge deposit attestation requires pending evidence".to_string(),
        ));
    }
    if record.source_proof_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1 {
        return Err((
            "pfusdc_fast_ingress_lifecycle_proof_required",
            "bonded fast ingress cannot be finalized by the legacy height/attestation path"
                .to_string(),
        ));
    }
    if block_height > record.expires_at_height {
        return Err((
            "stale_vault_bridge_deposit",
            "vault bridge asset bridge deposit evidence is expired".to_string(),
        ));
    }
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                "vault bridge asset bridge deposit attestation references missing NAV asset"
                    .to_string(),
            )
        })?;
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &record.evidence.source_domain(),
        &record.policy_hash,
    )?;
    if profile.verifier_kind != NAV_PROFILE_VERIFIER_MULTI_FETCH {
        return Err((
            "vault_bridge_deposit_not_attestable",
            "vault bridge asset bridge deposit attestations only apply to multi-fetch-quorum profiles"
                .to_string(),
        ));
    }
    if ledger.nav_attestor(&operation.attestor).is_none() {
        return Err((
            "unregistered_nav_attestor",
            "vault bridge asset bridge deposit attestation requires a registered attestor"
                .to_string(),
        ));
    }
    if profile.bridge_observer_min_confirmations > 0
        && operation.pass
        && compatibility.bridge_verification_rules_active(block_height)
    {
        let observation = operation.observation.as_ref().ok_or_else(|| {
            (
                "vault_bridge_deposit_observation_missing",
                "vault bridge deposit Stage 2 attestation requires observed EVM receipt facts"
                    .to_string(),
            )
        })?;
        ensure_vault_bridge_deposit_observation_matches(
            &record.evidence,
            &operation.observation_root,
            observation,
            profile.bridge_observer_min_confirmations,
        )?;
    }
    let record = ledger
        .vault_bridge_deposit_mut(&operation.asset_id, &operation.evidence_root)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_deposit",
                "vault bridge asset bridge deposit disappeared during attestation".to_string(),
            )
        })?;
    if record
        .attestations
        .iter()
        .any(|attestation| attestation.attestor == operation.attestor)
    {
        return Err((
            "duplicate_vault_bridge_deposit_attestation",
            "attestor has already attested this vault bridge asset bridge deposit".to_string(),
        ));
    }
    if record.attestations.len() >= MAX_NAV_ATTESTATIONS_PER_PACKET {
        return Err((
            "vault_bridge_deposit_attestations_full",
            "vault bridge asset bridge deposit attestation list is full".to_string(),
        ));
    }
    record.attestations.push(VaultBridgeDepositAttestation {
        attestor: operation.attestor.clone(),
        pass: operation.pass,
        observation_root: operation.observation_root.clone(),
        attested_at_height: block_height,
        observation: operation.observation.clone(),
    });
    record
        .validate()
        .map_err(|error| ("bad_vault_bridge_deposit", error))?;
    Ok(())
}

#[allow(dead_code)]
fn apply_vault_bridge_deposit_finalize(
    ledger: &mut LedgerState,
    operation: &VaultBridgeDepositFinalizeOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    apply_vault_bridge_deposit_finalize_with_compatibility(
        ledger,
        operation,
        block_height,
        AssetExecutionCompatibility::strict(),
    )
}

fn apply_vault_bridge_deposit_finalize_with_compatibility(
    ledger: &mut LedgerState,
    operation: &VaultBridgeDepositFinalizeOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    let record = ledger
        .vault_bridge_deposit(&operation.asset_id, &operation.evidence_root)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_vault_bridge_deposit",
                "vault bridge asset bridge deposit finalize references missing evidence"
                    .to_string(),
            )
        })?;
    if record.source_proof_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1 {
        return Err((
            "pfusdc_fast_ingress_lifecycle_proof_required",
            "bonded fast ingress cannot be finalized by the legacy height/attestation path"
                .to_string(),
        ));
    }
    if record.status != VAULT_BRIDGE_DEPOSIT_STATUS_PENDING {
        return Err((
            "vault_bridge_deposit_not_pending",
            "vault bridge asset bridge deposit finalize requires pending evidence".to_string(),
        ));
    }
    if block_height > record.expires_at_height {
        return Err((
            "stale_vault_bridge_deposit",
            "vault bridge asset bridge deposit evidence is expired".to_string(),
        ));
    }
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                "vault bridge asset bridge deposit finalize references missing NAV asset"
                    .to_string(),
            )
        })?;
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &record.evidence.source_domain(),
        &record.policy_hash,
    )?;
    ensure_vault_bridge_source_policy(
        profile,
        &record.evidence.source_domain(),
        &record.policy_hash,
    )?;
    ensure_vault_bridge_deposit_source_proof(VaultBridgeDepositSourceProof {
        genesis: None,
        profile,
        evidence: &record.evidence,
        evidence_root: &record.evidence_root,
        policy_hash: &record.policy_hash,
        source_proof_kind: &record.source_proof_kind,
        source_proof_hash: &record.source_proof_hash,
        source_public_values_hash: &record.source_public_values_hash,
        source_proof_bytes: &[],
        source_public_values: &[],
        fast_ingress_verifier: None,
    })?;
    match profile.verifier_kind.as_str() {
        NAV_PROFILE_VERIFIER_MULTI_FETCH => {
            let fail_count = record
                .attestations
                .iter()
                .filter(|attestation| !attestation.pass)
                .count() as u64;
            if fail_count > 0 {
                return Err((
                    "vault_bridge_deposit_failed_attestations_present",
                    format!(
                        "vault bridge asset bridge deposit has {fail_count} failing attestation(s); challenge or supersede the evidence instead of finalizing"
                    ),
                ));
            }
            let pass_count = record
                .attestations
                .iter()
                .filter(|attestation| attestation.pass)
                .count() as u64;
            if pass_count < profile.min_attestations {
                return Err((
                    "vault_bridge_deposit_attestation_quorum_not_met",
                    format!(
                        "vault bridge asset bridge deposit has {pass_count} pass attestation(s); profile requires {}",
                        profile.min_attestations
                    ),
                ));
            }
            if profile.bridge_observer_min_confirmations > 0
                && compatibility.bridge_verification_rules_active(block_height)
            {
                for attestation in record
                    .attestations
                    .iter()
                    .filter(|attestation| attestation.pass)
                {
                    let observation = attestation.observation.as_ref().ok_or_else(|| {
                        (
                            "vault_bridge_deposit_observation_missing",
                            "vault bridge deposit finalized under Stage 2 requires observed EVM receipt facts"
                                .to_string(),
                        )
                    })?;
                    ensure_vault_bridge_deposit_observation_matches(
                        &record.evidence,
                        &attestation.observation_root,
                        observation,
                        profile.bridge_observer_min_confirmations,
                    )?;
                }
            }
        }
        NAV_PROFILE_VERIFIER_SP1_GROTH16
        | NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
        | NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1 => {}
        _ => {
            return Err((
                "unsupported_vault_bridge_deposit_verifier",
                "vault bridge asset bridge deposit finalize requires a multi-fetch-quorum or sp1-groth16 profile"
                    .to_string(),
            ));
        }
    }
    if profile.challenge_window_blocks > 0
        && !vault_bridge_source_proof_is_consensus_verified(&record.source_proof_kind)
    {
        let finalizable_height = record
            .submitted_at_height
            .checked_add(profile.challenge_window_blocks)
            .ok_or_else(|| {
                (
                    "vault_bridge_deposit_height_overflow",
                    "vault bridge asset bridge deposit finalizable height overflowed".to_string(),
                )
            })?;
        if block_height < finalizable_height {
            return Err((
                "vault_bridge_deposit_challenge_window_open",
                format!(
                    "vault bridge asset bridge deposit finalize before challenge window closes at height {finalizable_height}"
                ),
            ));
        }
    }
    if record.source_proof_kind != NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1
        && profile.max_snapshot_age_blocks > 0
    {
        let stale_height = record
            .submitted_at_height
            .checked_add(profile.max_snapshot_age_blocks)
            .ok_or_else(|| {
                (
                    "vault_bridge_deposit_height_overflow",
                    "vault bridge asset bridge deposit stale height overflowed".to_string(),
                )
            })?;
        if block_height > stale_height {
            return Err((
                "stale_vault_bridge_deposit",
                "vault bridge asset bridge deposit evidence exceeds profile max snapshot age"
                    .to_string(),
            ));
        }
    }
    let record = ledger
        .vault_bridge_deposit_mut(&operation.asset_id, &operation.evidence_root)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_deposit",
                "vault bridge asset bridge deposit disappeared during finalize".to_string(),
            )
        })?;
    record.status = VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED.to_string();
    record.finalized_at_height = block_height;
    record
        .validate()
        .map_err(|error| ("bad_vault_bridge_deposit", error))?;
    Ok(())
}

#[cfg(test)]
fn apply_vault_bridge_deposit_claim(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &VaultBridgeDepositClaimOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    apply_vault_bridge_deposit_claim_with_orchard(
        genesis,
        ledger,
        operation,
        block_height,
        AssetExecutionCompatibility::strict(),
        &[],
    )
}

pub(crate) fn apply_vault_bridge_deposit_claim_with_orchard(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &VaultBridgeDepositClaimOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
    orchard_balances: &[AssetOrchardAssetBalance],
) -> Result<(), (&'static str, String)> {
    let nav_asset = ledger.nav_asset(&operation.asset_id).cloned().ok_or_else(|| {
        (
            "missing_nav_asset",
            format!(
                "vault bridge asset bridge deposit claim asset `{}` is not registered as a NAV asset",
                operation.asset_id
            ),
        )
    })?;
    let asset = ledger
        .asset_definition(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", operation.asset_id),
            )
        })?;
    if asset.issuer != nav_asset.issuer {
        return Err((
            "asset_issuer_mismatch",
            "vault bridge asset bridge deposit claim asset issuer does not match NAV asset issuer"
                .to_string(),
        ));
    }
    let record = ledger
        .vault_bridge_deposit(&operation.asset_id, &operation.evidence_root)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_vault_bridge_deposit",
                "vault bridge asset bridge deposit claim references missing evidence".to_string(),
            )
        })?;
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &record.evidence.source_domain(),
        &record.policy_hash,
    )?;
    if record.status != VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED {
        return Err((
            "vault_bridge_deposit_not_finalized",
            "vault bridge asset bridge deposit claim requires finalized vault event evidence"
                .to_string(),
        ));
    }
    if record.policy_hash != operation.policy_hash {
        return Err((
            "vault_bridge_deposit_policy_mismatch",
            "vault bridge asset bridge deposit claim policy_hash does not match finalized evidence"
                .to_string(),
        ));
    }
    ensure_vault_bridge_source_policy(
        profile,
        &record.evidence.source_domain(),
        &operation.policy_hash,
    )?;
    if block_height > record.expires_at_height {
        return Err((
            "stale_vault_bridge_deposit",
            "vault bridge asset bridge deposit evidence is expired".to_string(),
        ));
    }
    if record.source_proof_kind != NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1
        && profile.max_snapshot_age_blocks > 0
    {
        let stale_height = record
            .submitted_at_height
            .checked_add(profile.max_snapshot_age_blocks)
            .ok_or_else(|| {
                (
                    "vault_bridge_deposit_height_overflow",
                    "vault bridge asset bridge deposit stale height overflowed".to_string(),
                )
            })?;
        if block_height > stale_height {
            return Err((
                "stale_vault_bridge_deposit",
                "vault bridge asset bridge deposit evidence exceeds profile max snapshot age"
                    .to_string(),
            ));
        }
    }

    let issuer_delegated_ethereum_claim = record.source_proof_kind
        == SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1
        && record.evidence.pftl_recipient == nav_asset.issuer
        && operation.claimer == nav_asset.issuer
        && operation.recipient != nav_asset.issuer;
    if operation.recipient != record.evidence.pftl_recipient && !issuer_delegated_ethereum_claim {
        return Err((
            "vault_bridge_deposit_recipient_mismatch",
            "vault bridge asset bridge deposit claim recipient must match finalized vault evidence; only the bound issuer may delegate a consensus-verified Ethereum claim to a holder"
                .to_string(),
        ));
    }
    if operation.amount_atoms != record.evidence.amount_atoms {
        return Err((
            "vault_bridge_deposit_amount_mismatch",
            "vault bridge asset bridge deposit claim amount must match finalized vault evidence"
                .to_string(),
        ));
    }

    let mut claimed_fast_campaign = None;
    if record.source_proof_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1 {
        let mut campaign = ledger
            .fast_ingress_campaigns
            .iter()
            .find(|campaign| campaign.route_profile_hash == record.policy_hash)
            .cloned()
            .ok_or_else(|| {
                (
                    "pfusdc_fast_ingress_campaign_missing",
                    "bonded ingress claim is missing its campaign state".to_string(),
                )
            })?;
        let mint = campaign
            .mints
            .iter_mut()
            .find(|mint| mint.deposit_id == record.evidence.deposit_id)
            .ok_or_else(|| {
                (
                    "pfusdc_fast_ingress_mint_missing",
                    "bonded ingress claim is missing its escrow mint record".to_string(),
                )
            })?;
        if mint.recipient != operation.recipient
            || mint.amount_atoms != operation.amount_atoms
            || mint.claimed
            || !matches!(
                mint.status.as_str(),
                postfiat_types::FAST_INGRESS_MINT_STATUS_FINAL
                    | postfiat_types::FAST_INGRESS_MINT_STATUS_RELEASED_UNCONFIRMED
            )
        {
            return Err((
                "pfusdc_fast_ingress_escrow_not_releasable",
                "provisional pfUSDC remains escrowed until an authenticated lifecycle release"
                    .to_string(),
            ));
        }
        mint.claimed = true;
        campaign
            .validate()
            .map_err(|error| ("pfusdc_fast_ingress_campaign_invalid", error))?;
        claimed_fast_campaign = Some(campaign);
    }

    let recipient = operation.recipient.clone();
    if recipient == nav_asset.issuer {
        return Err((
            "unsupported_issuer_mint",
            "vault bridge asset bridge deposit recipient must be a holder trustline account"
                .to_string(),
        ));
    }
    let claim_amount = operation.amount_atoms;
    let source_series_active = compatibility.pfusdc_source_series_active(block_height);
    let (credit_asset, source_asset_to_create) = if source_series_active {
        if operation.route_epoch == 0 {
            return Err((
                "pfusdc_source_series_route_epoch_missing",
                "source-series pfUSDC claim requires its governed route epoch".to_string(),
            ));
        }
        let expected_route_binding =
            vault_bridge_route_binding(&operation.policy_hash, operation.route_epoch)
                .map_err(|error| ("pfusdc_source_series_route_binding_invalid", error))?;
        if record.evidence.route_binding != expected_route_binding {
            return Err((
                "pfusdc_source_series_route_epoch_mismatch",
                "source-series route epoch does not match the proof-bound route binding"
                    .to_string(),
            ));
        }
        let source_series_id = pfusdc_source_series_id(
            &genesis.chain_id,
            &operation.asset_id,
            record.evidence.source_chain_id,
            &record.evidence.vault_address,
            &record.evidence.token_address,
            operation.route_epoch,
            &operation.policy_hash,
        )
        .map_err(|error| ("pfusdc_source_series_invalid", error))?;
        let source_bucket_id = vault_bridge_bucket_id(
            &operation.asset_id,
            &record.evidence.source_domain(),
            &operation.policy_hash,
        )
        .map_err(|error| ("pfusdc_source_series_bucket_invalid", error))?;
        if let Some(existing) = ledger.asset_definition(&source_series_id) {
            if existing.asset_family_id != operation.asset_id
                || existing.source_series_id != source_series_id
                || existing.source_bucket_id != source_bucket_id
                || existing.issuer != asset.issuer
                || existing.precision != asset.precision
            {
                return Err((
                    "pfusdc_source_series_asset_mismatch",
                    "existing source-series asset does not match its immutable family and source"
                        .to_string(),
                ));
            }
            (existing.clone(), None)
        } else {
            let display_name = format!("{} · source epoch {}", asset.code, operation.route_epoch);
            let source_asset = AssetDefinition::new_source_series(
                &asset,
                source_series_id,
                source_bucket_id,
                display_name,
            )
            .map_err(|error| ("pfusdc_source_series_asset_invalid", error))?;
            (source_asset.clone(), Some(source_asset))
        }
    } else {
        if operation.route_epoch != 0 {
            return Err((
                "pfusdc_source_series_not_active",
                "route_epoch is forbidden before source-series activation".to_string(),
            ));
        }
        (asset.clone(), None)
    };

    let ledger_supply = issued_asset_family_supply(ledger, &operation.asset_id)?;
    let orchard_live = orchard_balances
        .iter()
        .filter(|balance| {
            balance.asset_id == operation.asset_id
                || ledger
                    .asset_definition(&balance.asset_id)
                    .is_some_and(|definition| definition.asset_family_id == operation.asset_id)
        })
        .try_fold(0_u64, |total, balance| {
            total.checked_add(balance.live_total).ok_or((
                "issued_supply_orchard_overflow",
                "vault bridge asset family Orchard supply overflowed".to_string(),
            ))
        })?;
    let current_supply = ledger_supply.checked_add(orchard_live).ok_or_else(|| {
        (
            "issued_supply_orchard_overflow",
            "vault bridge asset claim orchard-inclusive issued supply overflowed".to_string(),
        )
    })?;
    let supply_after_claim = current_supply.checked_add(claim_amount).ok_or_else(|| {
        (
            "issued_supply_overflow",
            "vault bridge asset bridge deposit claim would overflow issued supply".to_string(),
        )
    })?;
    if let Some(max_supply) = asset.max_supply {
        if supply_after_claim > max_supply {
            return Err((
                "issued_supply_cap_exceeded",
                "vault bridge asset bridge deposit claim exceeds issued asset max_supply"
                    .to_string(),
            ));
        }
    }
    let proof_bounded_cap_checkpoint = if record.source_proof_kind
        == SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1
        && supply_after_claim > nav_asset.circulating_supply
    {
        let route_id = match record.evidence.source_chain_id {
            ETHEREUM_MAINNET_CHAIN_ID => VAULT_BRIDGE_ROUTE_ETHEREUM_MAINNET_USDC_V1,
            ETHEREUM_SEPOLIA_CHAIN_ID => VAULT_BRIDGE_ROUTE_ETHEREUM_SEPOLIA_USDC_V1,
            _ => {
                return Err((
                    "ethereum_ingress_source_chain_unsupported",
                    "Ethereum cap growth uses an unregistered source chain".to_string(),
                ));
            }
        };
        let ethereum_backing = ledger
            .vault_bridge_route_backing(&operation.asset_id)
            .map_err(|error| ("bad_route_backing", error))?
            .into_iter()
            .find(|row| row.route_id == route_id)
            .ok_or_else(|| {
                (
                    "missing_ethereum_route_backing",
                    "Ethereum cap growth requires finalized route-indexed backing".to_string(),
                )
            })?;
        let ceiling = nav_asset
            .circulating_supply
            .checked_add(
                ethereum_backing
                    .finalized_unclaimed()
                    .map_err(|error| ("bad_route_backing", error))?,
            )
            .ok_or_else(|| {
                (
                    "nav_cap_overflow",
                    "proof-bounded NAV cap overflow".to_string(),
                )
            })?;
        if supply_after_claim > ceiling {
            return Err((
                "proof_bounded_nav_cap_exceeded",
                "claim exceeds finalized unclaimed Ethereum SP1 backing".to_string(),
            ));
        }
        Some(
            proof_bounded_nav_cap_checkpoint_hash(
                &operation.asset_id,
                route_id,
                &record.evidence_root,
                nav_asset.circulating_supply,
                supply_after_claim,
            )
            .map_err(|error| ("bad_proof_bounded_nav_checkpoint", error))?,
        )
    } else {
        None
    };

    let recipient_index = issued_asset_credit_recipient_line_index(
        ledger,
        &credit_asset,
        &recipient,
        claim_amount,
        "vault bridge asset bridge deposit claim",
    )?;
    let (recipient_after, recipient_required) = prepare_issued_asset_credit(
        ledger,
        &credit_asset,
        &recipient,
        recipient_index,
        claim_amount,
        "vault bridge asset bridge deposit claim",
    )?;

    let expected_receipt = VaultBridgeReceipt::new(
        &genesis.chain_id,
        operation.asset_id.clone(),
        record.evidence.source_domain(),
        record.evidence.source_asset_ref(),
        VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT,
        claim_amount,
        record.evidence.source_tx_or_attestation(),
        record.evidence.finality_ref(),
        record.evidence.vault_id(),
        operation.policy_hash.clone(),
        record.submitted_at_height,
        record.expires_at_height,
        Some(record.evidence.clone()),
    )
    .map_err(|error| ("bad_vault_bridge_receipt", error))?;
    let receipt_index = ledger
        .vault_bridge_receipts
        .iter()
        .position(|receipt| receipt.receipt_id == expected_receipt.receipt_id);
    let mut receipt_after = if let Some(index) = receipt_index {
        let receipt = ledger.vault_bridge_receipts[index].clone();
        if receipt.asset_id != expected_receipt.asset_id
            || receipt.source_domain != expected_receipt.source_domain
            || receipt.source_asset != expected_receipt.source_asset
            || receipt.claim_type != expected_receipt.claim_type
            || receipt.amount_atoms != expected_receipt.amount_atoms
            || receipt.source_tx_or_attestation != expected_receipt.source_tx_or_attestation
            || receipt.finality_ref != expected_receipt.finality_ref
            || receipt.vault_id != expected_receipt.vault_id
            || receipt.policy_hash != expected_receipt.policy_hash
            || receipt.bucket_id != expected_receipt.bucket_id
            || receipt.bridge_deposit_evidence != expected_receipt.bridge_deposit_evidence
        {
            return Err((
                "vault_bridge_receipt_bridge_deposit_mismatch",
                "vault bridge asset bridge deposit claim receipt does not match finalized evidence"
                    .to_string(),
            ));
        }
        receipt
    } else {
        expected_receipt
    };

    let receipt_was_counted = receipt_after.status == VAULT_BRIDGE_RECEIPT_STATUS_COUNTED;
    match receipt_after.status.as_str() {
        VAULT_BRIDGE_RECEIPT_STATUS_PENDING | VAULT_BRIDGE_RECEIPT_STATUS_FINALIZED => {
            receipt_after.haircut_bps = 0;
            receipt_after.counted_value_atoms = claim_amount;
            receipt_after.status = VAULT_BRIDGE_RECEIPT_STATUS_COUNTED.to_string();
            if receipt_after.finalized_at_height == 0 {
                receipt_after.finalized_at_height = block_height;
            }
            receipt_after.counted_at_height = block_height;
        }
        VAULT_BRIDGE_RECEIPT_STATUS_COUNTED => {
            if receipt_after.haircut_bps != 0 || receipt_after.counted_value_atoms != claim_amount {
                return Err((
                    "vault_bridge_receipt_count_mismatch",
                    "vault bridge asset bridge deposit claim requires a 1:1 counted bridge receipt"
                        .to_string(),
                ));
            }
        }
        _ => {
            return Err((
                "vault_bridge_receipt_not_claimable",
                "vault bridge asset bridge deposit claim requires a pending, finalized, or counted receipt"
                    .to_string(),
            ));
        }
    }
    if receipt_after
        .available_counted_value()
        .map_err(|error| ("bad_vault_bridge_receipt", error))?
        < claim_amount
    {
        return Err((
            "vault_bridge_deposit_already_claimed",
            "vault bridge asset bridge deposit receipt capacity is already allocated".to_string(),
        ));
    }
    let consumer_id = format!("vault_bridge_deposit_claim:{}", record.evidence_root);
    let allocation = VaultBridgeAllocation::new(
        &genesis.chain_id,
        receipt_after.receipt_id.clone(),
        operation.asset_id.clone(),
        receipt_after.bucket_id.clone(),
        claim_amount,
        VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,
        consumer_id,
        block_height,
    )
    .map_err(|error| ("bad_vault_bridge_allocation", error))?;
    if ledger
        .vault_bridge_allocation(&allocation.allocation_id)
        .is_some()
    {
        return Err((
            "duplicate_vault_bridge_allocation",
            "vault bridge asset bridge deposit claim allocation already exists".to_string(),
        ));
    }

    let bucket_index = ledger
        .vault_bridge_bucket_states
        .iter()
        .position(|bucket| bucket.bucket_id == receipt_after.bucket_id);
    let mut bucket_after = if let Some(index) = bucket_index {
        ledger.vault_bridge_bucket_states[index].clone()
    } else if receipt_was_counted {
        return Err((
            "missing_vault_bridge_bucket",
            "counted vault bridge asset bridge deposit receipt is missing its source bucket"
                .to_string(),
        ));
    } else {
        VaultBridgeBucketState::new(
            receipt_after.asset_id.clone(),
            receipt_after.source_domain.clone(),
            receipt_after.policy_hash.clone(),
            block_height,
        )
        .map_err(|error| ("bad_vault_bridge_bucket", error))?
    };
    if bucket_after.asset_id != receipt_after.asset_id
        || bucket_after.source_domain != receipt_after.source_domain
        || bucket_after.policy_hash != receipt_after.policy_hash
    {
        return Err((
            "vault_bridge_bucket_mismatch",
            "vault bridge asset bridge deposit claim bucket metadata does not match receipt"
                .to_string(),
        ));
    }
    if source_series_active {
        let mut migration_buckets = ledger
            .vault_bridge_bucket_states
            .iter()
            .filter(|bucket| bucket.asset_id == operation.asset_id)
            .cloned()
            .collect::<Vec<_>>();
        migration_buckets.sort_by(|left, right| left.bucket_id.cmp(&right.bucket_id));
        for migration_bucket in migration_buckets {
            reconcile_vault_bridge_supply_allocations(
                ledger,
                &migration_bucket,
                block_height,
            )?;
        }
    }
    if bucket_after.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE {
        return Err((
            "vault_bridge_bucket_not_active",
            "vault bridge asset bridge deposit claim requires an active source bucket".to_string(),
        ));
    }
    if !receipt_was_counted {
        bucket_after.gross_receipt_atoms = bucket_after
            .gross_receipt_atoms
            .checked_add(claim_amount)
            .ok_or_else(|| {
                (
                    "vault_bridge_bucket_overflow",
                    "vault bridge asset bucket gross receipt atoms would overflow".to_string(),
                )
            })?;
        bucket_after.counted_value_atoms = bucket_after
            .counted_value_atoms
            .checked_add(claim_amount)
            .ok_or_else(|| {
                (
                    "vault_bridge_bucket_overflow",
                    "vault bridge asset bucket counted value atoms would overflow".to_string(),
                )
            })?;
    }
    bucket_after.outstanding_vault_bridge_atoms = bucket_after
        .outstanding_vault_bridge_atoms
        .checked_add(claim_amount)
        .ok_or_else(|| {
            (
                "vault_bridge_bucket_overflow",
                "vault bridge asset bucket outstanding supply would overflow".to_string(),
            )
        })?;
    bucket_after.last_updated_height = block_height;
    bucket_after
        .validate()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;

    receipt_after.allocated_value_atoms = receipt_after
        .allocated_value_atoms
        .checked_add(claim_amount)
        .ok_or_else(|| {
            (
                "vault_bridge_receipt_overflow",
                "vault bridge asset bridge deposit receipt allocated value would overflow"
                    .to_string(),
            )
        })?;
    receipt_after
        .validate_for_chain(&genesis.chain_id)
        .map_err(|error| ("bad_vault_bridge_receipt", error))?;

    if let Some(index) = receipt_index {
        ledger.vault_bridge_receipts[index] = receipt_after;
    } else {
        ledger.vault_bridge_receipts.push(receipt_after);
    }
    if let Some(index) = bucket_index {
        ledger.vault_bridge_bucket_states[index] = bucket_after;
    } else {
        ledger.vault_bridge_bucket_states.push(bucket_after);
    }
    ledger.vault_bridge_allocations.push(allocation);
    if let Some(source_asset) = source_asset_to_create {
        // The trustline may be prepared before the definition is inserted,
        // but both become visible only in the successful outer transition.
        ledger.asset_definitions.push(source_asset);
    }
    apply_prepared_issued_asset_credit(
        ledger,
        recipient_index,
        recipient_after,
        recipient_required,
    );
    if let Some(campaign) = claimed_fast_campaign {
        let current = ledger
            .fast_ingress_campaign_mut(&campaign.route_profile_hash, campaign.route_epoch)
            .ok_or_else(|| {
                (
                    "pfusdc_fast_ingress_campaign_missing",
                    "bonded ingress campaign disappeared during claim".to_string(),
                )
            })?;
        *current = campaign;
    }
    if let Some(checkpoint_hash) = proof_bounded_cap_checkpoint {
        let nav_asset = ledger.nav_asset_mut(&operation.asset_id).ok_or_else(|| {
            (
                "missing_nav_asset",
                "NAV asset disappeared during cap growth".to_string(),
            )
        })?;
        nav_asset.circulating_supply = supply_after_claim;
        nav_asset.finalized_epoch = nav_asset.finalized_epoch.checked_add(1).ok_or_else(|| {
            (
                "nav_epoch_overflow",
                "proof-bounded NAV epoch overflow".to_string(),
            )
        })?;
        nav_asset.finalized_reserve_packet_hash = checkpoint_hash;
        nav_asset.finalized_at_height = block_height;
        nav_asset
            .validate()
            .map_err(|error| ("bad_nav_asset", error))?;
    }
    Ok(())
}

/// Execute the production bridge-claim transition against a cloned ledger.
///
/// This is the consensus-rule source for ingress quotes. Callers must insert
/// the proof-bound finalized deposit they want to quote into `ledger` first.
/// The returned ledger is never persisted by this function. Orchard custody
/// is included only when the independently governed activation is live at the
/// quoted height, preserving historical replay semantics.
pub fn simulate_vault_bridge_deposit_claim(
    genesis: &Genesis,
    ledger: &LedgerState,
    operation: &VaultBridgeDepositClaimOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
    orchard_balances: &[AssetOrchardAssetBalance],
) -> Result<LedgerState, (&'static str, String)> {
    let mut simulated = ledger.clone();
    let orchard_balances =
        orchard_balances_for_bridge_claim(compatibility, block_height, orchard_balances);
    apply_vault_bridge_deposit_claim_with_orchard(
        genesis,
        &mut simulated,
        operation,
        block_height,
        compatibility,
        orchard_balances,
    )?;
    Ok(simulated)
}

fn apply_vault_bridge_fast_ingress_lifecycle(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &VaultBridgeFastIngressLifecycleOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                "fast-ingress lifecycle asset is not registered".to_string(),
            )
        })?;
    let source_domain = ledger
        .vault_bridge_deposits
        .iter()
        .find(|record| {
            record.asset_id == operation.asset_id
                && record.policy_hash == operation.route_profile_hash
                && record.source_proof_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1
        })
        .map(|record| record.evidence.source_domain())
        .ok_or_else(|| {
            (
                "pfusdc_fast_ingress_mint_missing",
                "fast-ingress lifecycle has no bonded deposit records".to_string(),
            )
        })?;
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &source_domain,
        &operation.route_profile_hash,
    )?;
    if profile.verifier_kind != NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
        || operation.source_proof_kind != NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1
        || postfiat_types::pfusdc_ingress_proof_hash_v1(&operation.source_proof_bytes)
            != operation.source_proof_hash
        || postfiat_types::pfusdc_ingress_public_values_hash_v1(&operation.source_public_values)
            != operation.source_public_values_hash
    {
        return Err((
            "pfusdc_fast_ingress_lifecycle_commitment_mismatch",
            "fast-ingress lifecycle proof commitments do not match the active route".to_string(),
        ));
    }
    let values = postfiat_types::PfUsdcBondedLifecyclePublicValuesV1::from_canonical_bytes(
        &operation.source_public_values,
    )
    .map_err(|error| ("pfusdc_fast_ingress_lifecycle_public_values_invalid", error))?;
    if values.pftl_chain_id != genesis.chain_id
        || values.pftl_genesis_hash != genesis_hash(genesis)
        || values.pftl_protocol_version != genesis.protocol_version
        || values.route_profile_hash != operation.route_profile_hash
        || values.source_assertion_id != operation.source_assertion_id
    {
        return Err((
            "pfusdc_fast_ingress_lifecycle_public_values_mismatch",
            "fast-ingress lifecycle proof does not match chain, route, or assertion".to_string(),
        ));
    }
    let mut finality = ledger
        .ethereum_arbitrum_finality_state(&operation.route_profile_hash, values.route_epoch)
        .cloned()
        .ok_or_else(|| {
            (
                "pfusdc_finality_state_missing",
                "fast-ingress lifecycle is missing its finality state".to_string(),
            )
        })?;
    let verifier = finality.fast_ingress_verifier.as_ref().ok_or_else(|| {
        (
            "pfusdc_fast_ingress_verifier_missing",
            "fast-ingress lifecycle is missing its governed secondary verifier".to_string(),
        )
    })?;
    if values.manifest_hash != verifier.deployment_manifest_hash
        || values.verifier_policy_hash != verifier.verifier_policy_hash
        || verifier.base_route_profile_hash != operation.route_profile_hash
        || verifier.route_epoch != values.route_epoch
        || (values.update_kind == postfiat_types::PFUSDC_BONDED_LIFECYCLE_UPDATE_AGE_RELEASED
            && !verifier.age_release_enabled)
    {
        return Err((
            "pfusdc_fast_ingress_lifecycle_config_mismatch",
            "fast-ingress lifecycle public values do not match the governed secondary verifier"
                .to_string(),
        ));
    }
    verify_bounded_sp1_groth16_with_config(
        &verifier.verifier_kind,
        NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1,
        &verifier.verifier_program_vkey,
        verifier.max_proof_bytes,
        verifier.max_public_values_bytes,
        &operation.source_proof_bytes,
        &operation.source_public_values,
    )
    .map_err(|error| (error.code(), error.message()))?;
    finality
        .verify_and_advance_bonded_lifecycle(&values)
        .map_err(|error| ("pfusdc_fast_ingress_lifecycle_finality_rejected", error))?;
    let mut campaign = ledger
        .fast_ingress_campaign(&operation.route_profile_hash, values.route_epoch)
        .cloned()
        .ok_or_else(|| {
            (
                "pfusdc_fast_ingress_campaign_missing",
                "fast-ingress lifecycle is missing its campaign".to_string(),
            )
        })?;
    if campaign.asset_id != operation.asset_id {
        return Err((
            "pfusdc_fast_ingress_asset_mismatch",
            "fast-ingress lifecycle asset differs from its campaign".to_string(),
        ));
    }
    let reverted = values.update_kind == postfiat_types::PFUSDC_BONDED_LIFECYCLE_UPDATE_REVERTED;
    let age_released =
        values.update_kind == postfiat_types::PFUSDC_BONDED_LIFECYCLE_UPDATE_AGE_RELEASED;
    if reverted {
        campaign
            .apply_reversion(&values)
            .map_err(|error| ("pfusdc_fast_ingress_reversion_rejected", error))?;
    } else if age_released {
        if compatibility.allow_incremental_age_release_replay {
            campaign
                .apply_incremental_age_release_for_replay(&values)
                .map_err(|error| ("pfusdc_fast_ingress_age_release_rejected", error))?;
        } else {
            campaign
                .apply_age_release(&values)
                .map_err(|error| ("pfusdc_fast_ingress_age_release_rejected", error))?;
        }
    } else {
        campaign
            .apply_confirmation(&values)
            .map_err(|error| ("pfusdc_fast_ingress_confirmation_rejected", error))?;
    }
    let deposit_ids = campaign
        .mints
        .iter()
        .filter(|mint| mint.source_assertion_id == values.source_assertion_id)
        .map(|mint| mint.deposit_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if deposit_ids.is_empty() {
        return Err((
            "pfusdc_fast_ingress_mint_missing",
            "confirmation has no matching escrowed deposits".to_string(),
        ));
    }
    let matching_records = ledger
        .vault_bridge_deposits
        .iter()
        .enumerate()
        .filter(|(_, record)| {
            record.asset_id == operation.asset_id
                && record.policy_hash == operation.route_profile_hash
                && deposit_ids.contains(&record.evidence.deposit_id)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matching_records.len() != deposit_ids.len() {
        return Err((
            "pfusdc_fast_ingress_deposit_record_mismatch",
            "confirmation mint/deposit records are incomplete".to_string(),
        ));
    }
    let mut updated_records = Vec::with_capacity(matching_records.len());
    for index in &matching_records {
        let mut record = ledger.vault_bridge_deposits[*index].clone();
        if reverted {
            record.status = VAULT_BRIDGE_DEPOSIT_STATUS_CHALLENGED.to_string();
        } else {
            record.status = VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED.to_string();
            record.finalized_at_height = block_height;
        }
        record
            .validate()
            .map_err(|error| ("bad_vault_bridge_deposit", error))?;
        updated_records.push((*index, record));
    }
    *ledger
        .ethereum_arbitrum_finality_state_mut(&operation.route_profile_hash, values.route_epoch)
        .ok_or_else(|| {
            (
                "pfusdc_finality_state_missing",
                "fast-ingress finality state disappeared during commit".to_string(),
            )
        })? = finality;
    *ledger
        .fast_ingress_campaign_mut(&operation.route_profile_hash, values.route_epoch)
        .ok_or_else(|| {
            (
                "pfusdc_fast_ingress_campaign_missing",
                "fast-ingress campaign disappeared during commit".to_string(),
            )
        })? = campaign;
    for (index, record) in updated_records {
        ledger.vault_bridge_deposits[index] = record;
    }
    Ok(())
}

fn apply_vault_bridge_receipt_count(
    ledger: &mut LedgerState,
    operation: &VaultBridgeReceiptCountOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!(
                    "vault bridge asset asset `{}` is not registered as a NAV asset",
                    operation.asset_id
                ),
            )
        })?;
    ensure_vault_bridge_asset_policy(ledger, &nav_asset, &operation.operator)?;
    let receipt_index = ledger
        .vault_bridge_receipts
        .iter()
        .position(|receipt| receipt.receipt_id == operation.receipt_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_receipt",
                "vault bridge asset receipt count references missing receipt".to_string(),
            )
        })?;
    let receipt = ledger.vault_bridge_receipts[receipt_index].clone();
    if receipt.asset_id != operation.asset_id {
        return Err((
            "vault_bridge_receipt_asset_mismatch",
            "vault bridge asset receipt does not belong to operation asset".to_string(),
        ));
    }
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &receipt.source_domain,
        &receipt.policy_hash,
    )?;
    ensure_vault_bridge_source_policy(profile, &receipt.source_domain, &operation.policy_hash)?;
    if receipt.claim_type != VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT {
        return Err((
            "unsupported_vault_bridge_claim_type",
            "vault bridge asset count requires a bridge_deposit vault event receipt".to_string(),
        ));
    }
    if receipt.policy_hash != operation.policy_hash {
        return Err((
            "vault_bridge_policy_hash_mismatch",
            "vault bridge asset receipt policy hash does not match count operation".to_string(),
        ));
    }
    if receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_PENDING
        && receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_FINALIZED
    {
        return Err((
            "vault_bridge_receipt_not_countable",
            "vault bridge asset receipt is not pending/finalized and cannot be counted".to_string(),
        ));
    }
    ensure_vault_bridge_receipt_fresh(profile, &receipt, block_height)?;
    let bridge_evidence = receipt.bridge_deposit_evidence.as_ref().ok_or_else(|| {
        (
            "missing_vault_bridge_deposit_evidence",
            "vault bridge asset bridge_deposit receipt is missing vault event evidence".to_string(),
        )
    })?;
    let expected_evidence_root = vault_bridge_deposit_evidence_root(bridge_evidence)
        .map_err(|error| ("bad_vault_bridge_deposit_evidence", error))?;
    if operation.evidence_root != expected_evidence_root {
        return Err((
            "vault_bridge_deposit_evidence_root_mismatch",
            "vault bridge asset count evidence_root must match the stored bridge deposit evidence"
                .to_string(),
        ));
    }
    let finalized_bridge_deposit = ledger
        .vault_bridge_deposit(&operation.asset_id, &expected_evidence_root)
        .ok_or_else(|| {
            (
                "vault_bridge_deposit_not_finalized",
                "vault bridge asset count requires a finalized bridge deposit evidence record"
                    .to_string(),
            )
        })?;
    if finalized_bridge_deposit.status != VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED {
        return Err((
            "vault_bridge_deposit_not_finalized",
            "vault bridge asset count requires finalized bridge deposit evidence".to_string(),
        ));
    }
    if finalized_bridge_deposit.policy_hash != operation.policy_hash {
        return Err((
            "vault_bridge_deposit_policy_mismatch",
            "vault bridge asset bridge deposit policy hash does not match count operation"
                .to_string(),
        ));
    }
    if finalized_bridge_deposit.evidence != *bridge_evidence {
        return Err((
            "vault_bridge_deposit_evidence_mismatch",
            "vault bridge asset receipt bridge evidence does not match the finalized bridge deposit record"
                .to_string(),
        ));
    }
    if block_height > finalized_bridge_deposit.expires_at_height {
        return Err((
            "stale_vault_bridge_deposit",
            "vault bridge asset bridge deposit evidence is expired".to_string(),
        ));
    }
    let counted_value_atoms =
        vault_bridge_policy::compute_counted_value(receipt.amount_atoms, operation.haircut_bps)
            .map_err(|error| (error.code(), error.message().to_string()))?;
    if counted_value_atoms != operation.counted_value_atoms {
        return Err((
            "vault_bridge_counted_value_mismatch",
            "vault bridge asset counted value must equal floor(amount_atoms * (10000 - haircut_bps) / 10000)"
                .to_string(),
        ));
    }

    let bucket_index = if let Some(index) = ledger
        .vault_bridge_bucket_states
        .iter()
        .position(|bucket| bucket.bucket_id == receipt.bucket_id)
    {
        index
    } else {
        let bucket = VaultBridgeBucketState::new(
            receipt.asset_id.clone(),
            receipt.source_domain.clone(),
            receipt.policy_hash.clone(),
            block_height,
        )
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;
        ledger.vault_bridge_bucket_states.push(bucket);
        ledger.vault_bridge_bucket_states.len() - 1
    };

    {
        let bucket = &ledger.vault_bridge_bucket_states[bucket_index];
        if bucket.asset_id != receipt.asset_id
            || bucket.source_domain != receipt.source_domain
            || bucket.policy_hash != receipt.policy_hash
        {
            return Err((
                "vault_bridge_bucket_mismatch",
                "vault bridge asset receipt bucket metadata does not match existing bucket"
                    .to_string(),
            ));
        }
        if bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE {
            return Err((
                "vault_bridge_bucket_not_active",
                "vault bridge asset receipt counting requires an active source bucket".to_string(),
            ));
        }
    }

    let gross_after = ledger.vault_bridge_bucket_states[bucket_index]
        .gross_receipt_atoms
        .checked_add(receipt.amount_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_bucket_overflow",
                "vault bridge asset bucket gross receipt atoms would overflow".to_string(),
            )
        })?;
    let counted_after = ledger.vault_bridge_bucket_states[bucket_index]
        .counted_value_atoms
        .checked_add(counted_value_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_bucket_overflow",
                "vault bridge asset bucket counted value atoms would overflow".to_string(),
            )
        })?;

    ledger.vault_bridge_receipts[receipt_index].haircut_bps = operation.haircut_bps;
    ledger.vault_bridge_receipts[receipt_index].counted_value_atoms = counted_value_atoms;
    ledger.vault_bridge_receipts[receipt_index].status =
        VAULT_BRIDGE_RECEIPT_STATUS_COUNTED.to_string();
    if ledger.vault_bridge_receipts[receipt_index].finalized_at_height == 0 {
        ledger.vault_bridge_receipts[receipt_index].finalized_at_height = block_height;
    }
    ledger.vault_bridge_receipts[receipt_index].counted_at_height = block_height;

    ledger.vault_bridge_bucket_states[bucket_index].gross_receipt_atoms = gross_after;
    ledger.vault_bridge_bucket_states[bucket_index].counted_value_atoms = counted_after;
    ledger.vault_bridge_bucket_states[bucket_index].last_updated_height = block_height;
    ledger.vault_bridge_bucket_states[bucket_index]
        .validate()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;
    ledger.vault_bridge_receipts[receipt_index]
        .validate()
        .map_err(|error| ("bad_vault_bridge_receipt", error))?;
    Ok(())
}

fn apply_vault_bridge_mint_from_receipts(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    operation: &VaultBridgeMintFromReceiptsOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!(
                    "vault bridge asset asset `{}` is not registered as a NAV asset",
                    operation.asset_id
                ),
            )
        })?;
    ensure_vault_bridge_asset_policy(ledger, &nav_asset, &operation.issuer)?;
    ensure_nav_asset_live_for_epoch(
        ledger,
        &nav_asset,
        operation.epoch,
        &operation.reserve_packet_hash,
        block_height,
    )?;

    let asset = ledger
        .asset_definition(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", operation.asset_id),
            )
        })?;
    let current_supply = issued_asset_supply(ledger, &operation.asset_id)?;
    let supply_after_mint = current_supply
        .checked_add(operation.amount_atoms)
        .ok_or_else(|| {
            (
                "issued_supply_overflow",
                "vault bridge asset mint would overflow issued supply".to_string(),
            )
        })?;
    if let Some(max_supply) = asset.max_supply {
        if supply_after_mint > max_supply {
            return Err((
                "issued_supply_cap_exceeded",
                "vault bridge asset mint exceeds issued asset max_supply".to_string(),
            ));
        }
    }
    let to_index = issued_asset_credit_recipient_line_index(
        ledger,
        &asset,
        &operation.to,
        operation.amount_atoms,
        "vault bridge asset mint",
    )?;
    let (recipient_after, recipient_required) = prepare_issued_asset_credit(
        ledger,
        &asset,
        &operation.to,
        to_index,
        operation.amount_atoms,
        "vault bridge asset mint",
    )?;

    let bucket_index = ledger
        .vault_bridge_bucket_states
        .iter()
        .position(|bucket| bucket.bucket_id == operation.bucket_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_bucket",
                "vault bridge asset mint references missing source bucket".to_string(),
            )
        })?;
    let bucket = ledger.vault_bridge_bucket_states[bucket_index].clone();
    if bucket.asset_id != operation.asset_id {
        return Err((
            "vault_bridge_bucket_asset_mismatch",
            "vault bridge asset mint bucket does not belong to operation asset".to_string(),
        ));
    }
    if bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE {
        return Err((
            "vault_bridge_bucket_not_active",
            "vault bridge asset mint requires an active source bucket".to_string(),
        ));
    }
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &bucket.source_domain,
        &bucket.policy_hash,
    )?;
    ensure_vault_bridge_source_policy(profile, &bucket.source_domain, &bucket.policy_hash)?;

    let mut seen_receipts = std::collections::BTreeSet::new();
    let mut remaining = operation.amount_atoms;
    let mut receipt_takes: Vec<(usize, u64, VaultBridgeAllocation)> = Vec::new();
    for (receipt_position, receipt_id) in operation.receipt_ids.iter().enumerate() {
        if !seen_receipts.insert(receipt_id.clone()) {
            return Err((
                "duplicate_vault_bridge_mint_receipt",
                "vault bridge asset mint receipt_ids must not contain duplicates".to_string(),
            ));
        }
        let receipt_index = ledger
            .vault_bridge_receipts
            .iter()
            .position(|receipt| receipt.receipt_id == *receipt_id)
            .ok_or_else(|| {
                (
                    "missing_vault_bridge_receipt",
                    "vault bridge asset mint references missing receipt".to_string(),
                )
            })?;
        let receipt = &ledger.vault_bridge_receipts[receipt_index];
        if receipt.asset_id != operation.asset_id || receipt.bucket_id != operation.bucket_id {
            return Err((
                "vault_bridge_receipt_bucket_mismatch",
                "vault bridge asset mint receipt must belong to operation asset and bucket"
                    .to_string(),
            ));
        }
        if receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_COUNTED {
            return Err((
                "vault_bridge_receipt_not_counted",
                "vault bridge asset mint requires counted receipts".to_string(),
            ));
        }
        ensure_vault_bridge_receipt_fresh(profile, receipt, block_height)?;
        let available = receipt
            .available_counted_value()
            .map_err(|error| ("bad_vault_bridge_receipt", error))?;
        if available == 0 || remaining == 0 {
            continue;
        }
        let take = available.min(remaining);
        let consumer_id = format!(
            "vault_bridge_supply:{}:{}",
            transaction.unsigned.sequence, receipt_position
        );
        let allocation = VaultBridgeAllocation::new(
            &genesis.chain_id,
            receipt.receipt_id.clone(),
            operation.asset_id.clone(),
            operation.bucket_id.clone(),
            take,
            VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,
            consumer_id,
            block_height,
        )
        .map_err(|error| ("bad_vault_bridge_allocation", error))?;
        if ledger
            .vault_bridge_allocation(&allocation.allocation_id)
            .is_some()
        {
            return Err((
                "duplicate_vault_bridge_allocation",
                "vault bridge asset allocation already exists".to_string(),
            ));
        }
        receipt_takes.push((receipt_index, take, allocation));
        remaining -= take;
    }
    if remaining != 0 {
        return Err((
            "insufficient_vault_bridge_receipt_capacity",
            "vault bridge asset mint exceeds unallocated counted receipt capacity".to_string(),
        ));
    }

    let outstanding_after = ledger.vault_bridge_bucket_states[bucket_index]
        .outstanding_vault_bridge_atoms
        .checked_add(operation.amount_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_bucket_overflow",
                "vault bridge asset bucket outstanding supply would overflow".to_string(),
            )
        })?;
    let mut bucket_after = ledger.vault_bridge_bucket_states[bucket_index].clone();
    bucket_after.outstanding_vault_bridge_atoms = outstanding_after;
    bucket_after.last_updated_height = block_height;
    bucket_after
        .validate()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;

    for (receipt_index, take, allocation) in receipt_takes {
        let receipt = &mut ledger.vault_bridge_receipts[receipt_index];
        receipt.allocated_value_atoms = receipt
            .allocated_value_atoms
            .checked_add(take)
            .ok_or_else(|| {
                (
                    "vault_bridge_receipt_overflow",
                    "vault bridge asset receipt allocated value would overflow".to_string(),
                )
            })?;
        receipt
            .validate()
            .map_err(|error| ("bad_vault_bridge_receipt", error))?;
        ledger.vault_bridge_allocations.push(allocation);
    }
    ledger.vault_bridge_bucket_states[bucket_index] = bucket_after;
    apply_prepared_issued_asset_credit(ledger, to_index, recipient_after, recipient_required);
    Ok(())
}

#[allow(dead_code)]
fn apply_vault_bridge_nav_subscription_allocate(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    signed_source: &str,
    operation: &VaultBridgeNavSubscriptionAllocateOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    apply_vault_bridge_nav_subscription_allocate_with_compatibility(
        genesis,
        ledger,
        signed_source,
        operation,
        block_height,
        AssetExecutionCompatibility::strict(),
    )
}

fn apply_vault_bridge_nav_subscription_allocate_with_compatibility(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    signed_source: &str,
    operation: &VaultBridgeNavSubscriptionAllocateOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if ledger.nav_asset(&operation.nav_asset_id).is_none() {
        return Err((
            "missing_nav_asset",
            "vault bridge asset nav subscription allocation references missing NAV asset"
                .to_string(),
        ));
    }
    let settlement_nav_asset = ledger
        .nav_asset(&operation.settlement_asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_vault_bridge_nav_asset",
                "vault bridge asset settlement asset is not registered as a NAV asset".to_string(),
            )
        })?;
    let consumes_owned_settlement = operation.consume_supply_owner.is_some();
    if consumes_owned_settlement {
        ensure_vault_bridge_asset_registration(ledger, &settlement_nav_asset)?;
    } else {
        ensure_vault_bridge_asset_policy(ledger, &settlement_nav_asset, &operation.operator)?;
    }
    let profile = vault_bridge_profile_for_asset(ledger, &settlement_nav_asset)?;

    let bucket_index = ledger
        .vault_bridge_bucket_states
        .iter()
        .position(|bucket| bucket.bucket_id == operation.settlement_bucket_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_bucket",
                "vault bridge asset nav subscription allocation references missing source bucket"
                    .to_string(),
            )
        })?;
    let bucket = ledger.vault_bridge_bucket_states[bucket_index].clone();
    if bucket.asset_id != operation.settlement_asset_id {
        return Err((
            "vault_bridge_bucket_asset_mismatch",
            "vault bridge asset nav subscription bucket does not belong to settlement asset"
                .to_string(),
        ));
    }
    if bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE {
        return Err((
            "vault_bridge_bucket_not_active",
            "vault bridge asset nav subscription allocation requires an active source bucket"
                .to_string(),
        ));
    }
    ensure_vault_bridge_source_policy(profile, &bucket.source_domain, &bucket.policy_hash)?;

    let receipt_index = ledger
        .vault_bridge_receipts
        .iter()
        .position(|receipt| receipt.receipt_id == operation.settlement_receipt_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_receipt",
                "vault bridge asset nav subscription allocation references missing receipt"
                    .to_string(),
            )
        })?;
    let receipt = ledger.vault_bridge_receipts[receipt_index].clone();
    if receipt.asset_id != operation.settlement_asset_id
        || receipt.bucket_id != operation.settlement_bucket_id
    {
        return Err((
            "vault_bridge_receipt_bucket_mismatch",
            "vault bridge asset nav subscription receipt must belong to settlement asset and bucket"
                .to_string(),
        ));
    }
    if receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_COUNTED {
        return Err((
            "vault_bridge_receipt_not_counted",
            "vault bridge asset nav subscription allocation requires a counted receipt".to_string(),
        ));
    }
    ensure_vault_bridge_receipt_fresh(profile, &receipt, block_height)?;

    if let (Some(supply_owner), Some(supply_allocation_id), Some(nav_recipient)) = (
        operation.consume_supply_owner.as_ref(),
        operation.consume_supply_allocation_id.as_ref(),
        operation.nav_recipient.as_ref(),
    ) {
        if compatibility.bridge_verification_rules_active(block_height)
            && signed_source != supply_owner
        {
            return Err((
                "vault_bridge_supply_owner_signer_mismatch",
                "vault bridge asset nav subscription consume-supply allocation must be signed by the settlement owner".to_string(),
            ));
        }
        if supply_owner != nav_recipient {
            return Err((
                "vault_bridge_supply_owner_mismatch",
                "vault bridge asset nav subscription supply owner must match NAV recipient"
                    .to_string(),
            ));
        }
        let settlement_family_asset = ledger
            .asset_definition(&operation.settlement_asset_id)
            .cloned()
            .ok_or_else(|| {
                (
                    "missing_vault_bridge_asset",
                    "vault bridge asset nav subscription settlement asset definition is missing"
                        .to_string(),
                )
            })?;
        let settlement_asset = if compatibility.pfusdc_source_series_active(block_height) {
            let mut matches = ledger.asset_definitions.iter().filter(|asset| {
                asset.asset_family_id == operation.settlement_asset_id
                    && asset.source_bucket_id == operation.settlement_bucket_id
                    && asset.asset_id == asset.source_series_id
            });
            let asset = matches.next().cloned().ok_or_else(|| {
                (
                    "pfusdc_source_series_asset_missing",
                    "NAV subscription requires the source-series settlement asset for its bucket"
                        .to_string(),
                )
            })?;
            if matches.next().is_some() {
                return Err((
                    "pfusdc_source_series_asset_duplicate",
                    "multiple source-series assets reference the same settlement bucket"
                        .to_string(),
                ));
            }
            asset
        } else {
            settlement_family_asset
        };
        let owner_index =
            trustline_index(ledger, supply_owner, &settlement_asset.asset_id).ok_or_else(
                || {
                    (
                        "missing_trustline",
                        "vault bridge asset nav subscription supply owner has no selected source-series trustline"
                            .to_string(),
                    )
                },
            )?;
        ensure_line_can_move(&settlement_asset, &ledger.trustlines[owner_index])?;
        if ledger.trustlines[owner_index].balance < operation.settlement_amount_atoms {
            return Err((
                "insufficient_issued_balance",
                "vault bridge asset nav subscription exceeds owner settlement asset balance"
                    .to_string(),
            ));
        }

        let supply_allocation_index = ledger
            .vault_bridge_allocations
            .iter()
            .position(|allocation| allocation.allocation_id == *supply_allocation_id)
            .ok_or_else(|| {
                (
                    "missing_vault_bridge_allocation",
                    "vault bridge asset nav subscription references missing supply allocation"
                        .to_string(),
                )
            })?;
        let supply_allocation = ledger.vault_bridge_allocations[supply_allocation_index].clone();
        if supply_allocation.asset_id != operation.settlement_asset_id
            || supply_allocation.bucket_id != operation.settlement_bucket_id
            || supply_allocation.receipt_id != operation.settlement_receipt_id
        {
            return Err((
                "vault_bridge_allocation_source_mismatch",
                "vault bridge asset nav subscription supply allocation must match settlement asset, bucket, and receipt"
                    .to_string(),
            ));
        }
        if supply_allocation.purpose != VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY {
            return Err((
                "vault_bridge_allocation_wrong_purpose",
                "vault bridge asset nav subscription consume-supply allocation must be vault_bridge_supply"
                    .to_string(),
            ));
        }
        if supply_allocation.retired_at_height != 0 {
            return Err((
                "vault_bridge_allocation_retired",
                "vault bridge asset nav subscription supply allocation is already retired"
                    .to_string(),
            ));
        }
        let supply_remaining = supply_allocation
            .amount_atoms
            .checked_sub(supply_allocation.released_atoms)
            .ok_or_else(|| {
                (
                    "bad_vault_bridge_allocation",
                    "vault bridge asset nav subscription supply allocation released atoms exceed amount"
                        .to_string(),
                )
            })?;
        if supply_remaining < operation.settlement_amount_atoms {
            return Err((
                "insufficient_vault_bridge_supply_allocation",
                "vault bridge asset nav subscription exceeds remaining supply allocation"
                    .to_string(),
            ));
        }
        if bucket.outstanding_vault_bridge_atoms < operation.settlement_amount_atoms {
            return Err((
                "vault_bridge_bucket_underflow",
                "vault bridge asset nav subscription exceeds bucket outstanding wrapped supply"
                    .to_string(),
            ));
        }

        let allocation = VaultBridgeAllocation::new(
            &genesis.chain_id,
            receipt.receipt_id.clone(),
            operation.settlement_asset_id.clone(),
            operation.settlement_bucket_id.clone(),
            operation.settlement_amount_atoms,
            VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,
            operation.subscription_id.as_ref().map_or_else(
                || nav_subscription_recipient_consumer_id(&operation.nav_asset_id, nav_recipient),
                |subscription_id| {
                    nav_subscription_recipient_order_consumer_id(
                        &operation.nav_asset_id,
                        nav_recipient,
                        subscription_id,
                    )
                },
            ),
            block_height,
        )
        .map_err(|error| ("bad_vault_bridge_allocation", error))?;
        if ledger
            .vault_bridge_allocation(&allocation.allocation_id)
            .is_some()
        {
            return Err((
                "duplicate_vault_bridge_allocation",
                "vault bridge asset nav subscription allocation already exists".to_string(),
            ));
        }

        let mut bucket_after = bucket;
        bucket_after.outstanding_vault_bridge_atoms = bucket_after
            .outstanding_vault_bridge_atoms
            .checked_sub(operation.settlement_amount_atoms)
            .ok_or_else(|| {
                (
                    "vault_bridge_bucket_underflow",
                    "vault bridge asset bucket outstanding wrapped supply would underflow"
                        .to_string(),
                )
            })?;
        bucket_after.nav_subscription_allocations_atoms = bucket_after
            .nav_subscription_allocations_atoms
            .checked_add(operation.settlement_amount_atoms)
            .ok_or_else(|| {
                (
                    "vault_bridge_bucket_overflow",
                    "vault bridge asset bucket nav subscription allocations would overflow"
                        .to_string(),
                )
            })?;
        bucket_after.last_updated_height = block_height;
        bucket_after
            .validate()
            .map_err(|error| ("bad_vault_bridge_bucket", error))?;

        let released_after = ledger.vault_bridge_allocations[supply_allocation_index]
            .released_atoms
            .checked_add(operation.settlement_amount_atoms)
            .ok_or_else(|| {
                (
                    "vault_bridge_allocation_overflow",
                    "vault bridge asset supply allocation released atoms would overflow"
                        .to_string(),
                )
            })?;
        ledger.vault_bridge_allocations[supply_allocation_index].released_atoms = released_after;
        if released_after == ledger.vault_bridge_allocations[supply_allocation_index].amount_atoms {
            ledger.vault_bridge_allocations[supply_allocation_index].retired_at_height =
                block_height;
        }
        ledger.vault_bridge_allocations[supply_allocation_index]
            .validate()
            .map_err(|error| ("bad_vault_bridge_allocation", error))?;
        ledger.trustlines[owner_index].balance -= operation.settlement_amount_atoms;
        ledger.vault_bridge_bucket_states[bucket_index] = bucket_after;
        ledger.vault_bridge_allocations.push(allocation);
        return Ok(());
    }

    let available = receipt
        .available_counted_value()
        .map_err(|error| ("bad_vault_bridge_receipt", error))?;
    if available < operation.settlement_amount_atoms {
        return Err((
            "insufficient_vault_bridge_receipt_capacity",
            "vault bridge asset nav subscription allocation exceeds unallocated counted receipt capacity"
                .to_string(),
        ));
    }

    let allocation = VaultBridgeAllocation::new(
        &genesis.chain_id,
        receipt.receipt_id.clone(),
        operation.settlement_asset_id.clone(),
        operation.settlement_bucket_id.clone(),
        operation.settlement_amount_atoms,
        VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,
        nav_subscription_consumer_id(&operation.nav_asset_id),
        block_height,
    )
    .map_err(|error| ("bad_vault_bridge_allocation", error))?;
    if ledger
        .vault_bridge_allocation(&allocation.allocation_id)
        .is_some()
    {
        return Err((
            "duplicate_vault_bridge_allocation",
            "vault bridge asset nav subscription allocation already exists".to_string(),
        ));
    }

    let mut bucket_after = bucket;
    bucket_after.nav_subscription_allocations_atoms = bucket_after
        .nav_subscription_allocations_atoms
        .checked_add(operation.settlement_amount_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_bucket_overflow",
                "vault bridge asset bucket nav subscription allocations would overflow".to_string(),
            )
        })?;
    bucket_after.last_updated_height = block_height;
    bucket_after
        .validate()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;

    ledger.vault_bridge_receipts[receipt_index].allocated_value_atoms = ledger
        .vault_bridge_receipts[receipt_index]
        .allocated_value_atoms
        .checked_add(operation.settlement_amount_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_receipt_overflow",
                "vault bridge asset receipt allocated value would overflow".to_string(),
            )
        })?;
    ledger.vault_bridge_receipts[receipt_index]
        .validate()
        .map_err(|error| ("bad_vault_bridge_receipt", error))?;
    ledger.vault_bridge_bucket_states[bucket_index] = bucket_after;
    ledger.vault_bridge_allocations.push(allocation);
    Ok(())
}

fn apply_pftl_uniswap_route_init(
    _genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &PftlUniswapRouteInitOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    if ledger.pftl_uniswap_route(&operation.route_id).is_some() {
        return Err((
            "duplicate_pftl_uniswap_route",
            "PFTL-Uniswap route already exists in consensus state".to_string(),
        ));
    }
    let native_nav_asset = ensure_pftl_uniswap_native_asset_policy(
        ledger,
        &operation.native_nav_asset_id,
        &operation.operator,
    )?;
    if operation.latest_finalized_nav_epoch != native_nav_asset.finalized_epoch {
        return Err((
            "pftl_uniswap_route_epoch_mismatch",
            "PFTL-Uniswap route init latest NAV epoch must match ledger finalized NAV epoch"
                .to_string(),
        ));
    }
    ensure_pftl_uniswap_route_capacity(ledger, &native_nav_asset.issuer)?;
    if ledger
        .asset_definition(&operation.settlement_asset_id)
        .is_none()
    {
        return Err((
            "missing_settlement_asset",
            "PFTL-Uniswap route settlement asset definition is missing".to_string(),
        ));
    }
    let state_before_hash = pftl_uniswap_absent_route_hash(&operation.route_id);
    let route = PftlUniswapConsensusRouteState {
        route_id: operation.route_id.clone(),
        route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
        route_config_digest: operation.route_config_digest.clone(),
        route_trust_class: operation.route_trust_class.clone(),
        native_nav_asset_id: operation.native_nav_asset_id.clone(),
        settlement_asset_id: operation.settlement_asset_id.clone(),
        handoff_controller: operation.handoff_controller.clone(),
        settlement_adapter: operation.settlement_adapter.clone(),
        wrapped_navcoin_token: operation.wrapped_navcoin_token.clone(),
        ethereum_chain_id: operation.ethereum_chain_id,
        route_supply_cap_atoms: operation.route_supply_cap_atoms,
        packet_notional_cap_atoms: operation.packet_notional_cap_atoms,
        latest_finalized_nav_epoch: native_nav_asset.finalized_epoch,
        return_finality_blocks: operation.return_finality_blocks,
        live_value_enabled: operation.live_value_enabled,
        ethereum_verification_policy: operation.ethereum_verification_policy.clone(),
        authorized_valid_supply_atoms: 0,
        pftl_spendable_supply_atoms: 0,
        native_spendable_balances_atoms: std::collections::BTreeMap::new(),
        ethereum_spendable_supply_atoms: 0,
        other_registered_venue_supply_atoms: 0,
        outstanding_bridge_claims_atoms: 0,
        pending_return_import_claims_atoms: 0,
        settlement_reserve_atoms: 0,
        primary_subscription_nonces: std::collections::BTreeMap::new(),
        export_packets: std::collections::BTreeMap::new(),
        export_nonces: std::collections::BTreeMap::new(),
        return_imports: std::collections::BTreeMap::new(),
        paused: false,
        v2: None,
    };
    route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&route);
    ledger.pftl_uniswap_routes.push(route);
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "route_init",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: None,
            burn_event_hash: None,
            wallet: Some(operation.operator.clone()),
            amount_atoms: None,
            block_height,
        },
    )?;
    Ok(())
}

fn apply_pftl_uniswap_primary_subscribe(
    _genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &PftlUniswapPrimarySubscribeOperation,
    block_height: u64,
    allow_disabled_live_value_replay: bool,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let route = ledger.pftl_uniswap_routes[route_index].clone();
    ensure_pftl_uniswap_route_live(&route, allow_disabled_live_value_replay)?;
    let native_nav_asset = pftl_uniswap_pricing_nav_asset(ledger, &route, block_height)?;
    if operation.pricing_nav_epoch != native_nav_asset.finalized_epoch {
        return Err((
            "stale_pftl_uniswap_nav_epoch",
            "primary subscription pricing epoch must equal the ledger finalized NAV epoch"
                .to_string(),
        ));
    }
    if operation.pricing_reserve_packet_hash != native_nav_asset.finalized_reserve_packet_hash {
        return Err((
            "pftl_uniswap_pricing_packet_mismatch",
            "primary subscription pricing reserve packet hash must equal the ledger finalized reserve packet hash"
                .to_string(),
        ));
    }
    let derived_price =
        pftl_uniswap_price_settlement_atoms_per_nav_atom(ledger, &route, &native_nav_asset)?;
    if operation.nav_price_settlement_atoms_per_nav_atom != derived_price {
        return Err((
            "pftl_uniswap_price_mismatch",
            "primary subscription asserted price must equal the ledger-derived finalized NAV price"
                .to_string(),
        ));
    }
    if route.settlement_asset_id != operation.settlement_asset_id {
        return Err((
            "pftl_uniswap_settlement_asset_mismatch",
            "primary subscription settlement asset does not match route".to_string(),
        ));
    }
    if route
        .primary_subscription_nonces
        .contains_key(&operation.subscription_nonce)
    {
        return Err((
            "duplicate_pftl_uniswap_subscription_nonce",
            "primary subscription nonce already exists".to_string(),
        ));
    }
    let minted_nav_atoms = operation.settlement_value_atoms / derived_price;
    if minted_nav_atoms == 0 {
        return Err((
            "subscription_mints_zero_nav",
            "primary subscription settlement value is below one NAV atom at quoted price"
                .to_string(),
        ));
    }
    let settlement_debit_atoms = minted_nav_atoms.checked_mul(derived_price).ok_or_else(|| {
        (
            "pftl_uniswap_settlement_overflow",
            "primary subscription settlement debit would overflow".to_string(),
        )
    })?;
    let supply_after = route
        .authorized_valid_supply_atoms
        .checked_add(minted_nav_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_overflow",
                "primary subscription authorized supply would overflow".to_string(),
            )
        })?;
    if supply_after > route.route_supply_cap_atoms {
        return Err((
            "pftl_uniswap_route_supply_cap_exceeded",
            "primary subscription would exceed route supply cap".to_string(),
        ));
    }
    // Pricing above is bound to the finalized pre-inflow NAV snapshot. The
    // paid fill grows its issued-supply accounting bound by exactly the atoms
    // minted in this same transition; it does not create unbacked headroom.
    let native_circulating_supply_after = native_nav_asset
        .circulating_supply
        .checked_add(minted_nav_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_nav_supply_overflow",
                "primary subscription NAV circulating supply would overflow".to_string(),
            )
        })?;
    let settlement_reserve_after = route
        .settlement_reserve_atoms
        .checked_add(settlement_debit_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_reserve_overflow",
                "settlement reserve would overflow".to_string(),
            )
        })?;
    let pftl_spendable_supply_after = route
        .pftl_spendable_supply_atoms
        .checked_add(minted_nav_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_overflow",
                "PFTL spendable route supply would overflow".to_string(),
            )
        })?;
    debit_issued_asset_balance(
        ledger,
        &operation.subscriber,
        &operation.settlement_asset_id,
        settlement_debit_atoms,
        "PFTL-Uniswap primary subscription settlement debit",
    )?;
    credit_issued_asset_balance(
        ledger,
        &operation.subscriber,
        &route.native_nav_asset_id,
        minted_nav_atoms,
        "PFTL-Uniswap primary subscription NAV credit",
    )?;

    let mut next_route = route;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    next_route.primary_subscription_nonces.insert(
        operation.subscription_nonce.clone(),
        operation.subscriber.clone(),
    );
    next_route.authorized_valid_supply_atoms = supply_after;
    next_route.pftl_spendable_supply_atoms = pftl_spendable_supply_after;
    pftl_uniswap_credit_native_route_balance(
        &mut next_route,
        &operation.subscriber,
        minted_nav_atoms,
    )?;
    next_route.settlement_reserve_atoms = settlement_reserve_after;
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger
        .nav_asset_mut(&next_route.native_nav_asset_id)
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_nav_asset",
                "primary subscription native NAV asset disappeared before commit".to_string(),
            )
        })?
        .circulating_supply = native_circulating_supply_after;
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "primary_subscription",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: None,
            burn_event_hash: None,
            wallet: Some(operation.subscriber.clone()),
            amount_atoms: Some(minted_nav_atoms),
            block_height,
        },
    )?;
    Ok(())
}

fn apply_pftl_uniswap_route_init_v2(
    _genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &PftlUniswapRouteInitV2Operation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    if ledger.pftl_uniswap_route(&operation.route_id).is_some() {
        return Err((
            "duplicate_pftl_uniswap_route",
            "PFTL-Uniswap route already exists in consensus state".to_string(),
        ));
    }
    let native_nav_asset = ensure_pftl_uniswap_native_asset_policy(
        ledger,
        &operation.native_nav_asset_id,
        &operation.operator,
    )?;
    if operation.latest_finalized_nav_epoch != native_nav_asset.finalized_epoch
        || operation.primary_market_policy.pricing_nav_epoch != native_nav_asset.finalized_epoch
        || operation.primary_market_policy.pricing_reserve_packet_hash
            != native_nav_asset.finalized_reserve_packet_hash
    {
        return Err((
            "pftl_uniswap_route_epoch_mismatch",
            "v2 route and primary policy must pin the ledger finalized NAV epoch and reserve packet"
                .to_string(),
        ));
    }
    ensure_pftl_uniswap_route_capacity(ledger, &native_nav_asset.issuer)?;
    let native_definition = ledger
        .asset_definition(&operation.native_nav_asset_id)
        .ok_or_else(|| {
            (
                "missing_native_nav_asset",
                "PFTL-Uniswap v2 native NAV asset definition is missing".to_string(),
            )
        })?;
    let settlement_definition = ledger
        .asset_definition(&operation.settlement_asset_id)
        .ok_or_else(|| {
            (
                "missing_settlement_asset",
                "PFTL-Uniswap v2 settlement asset definition is missing".to_string(),
            )
        })?;
    if native_definition.precision != 6 || settlement_definition.precision != 6 {
        return Err((
            "pftl_uniswap_v2_precision_mismatch",
            "PFTL-Uniswap v2 currently requires six-decimal native and settlement assets"
                .to_string(),
        ));
    }
    // The route is a generic NAVCoin primary market. Asset identity and
    // version are already bound by native_nav_asset_id, its finalized reserve
    // packet, and the governed route/policy hashes. Restricting this path to
    // the literal A666/v2 tuple made independent issuers impossible without
    // adding any security property. Elastic primary issuance does, however,
    // require that no permanent asset-definition ceiling can conflict with
    // the route's governed capacity accounting.
    if native_definition.max_supply.is_some() {
        return Err((
            "pftl_uniswap_v2_asset_policy_mismatch",
            "PFTL-Uniswap v2 native NAV assets must not have a permanent max supply"
                .to_string(),
        ));
    }
    let issued_supply = issued_asset_supply(ledger, &operation.native_nav_asset_id)?;
    let existing_native_route = ledger
        .pftl_uniswap_routes
        .iter()
        .any(|route| route.native_nav_asset_id == operation.native_nav_asset_id);
    let mut opening_balances = std::collections::BTreeMap::new();
    if operation.opening_inventory_atoms == 0 {
        if !existing_native_route
            && (issued_supply != 0 || native_nav_asset.circulating_supply != 0)
        {
            return Err((
                "pftl_uniswap_untracked_opening_supply",
                "a fresh v2 route must inventory all existing proof-backed NAV supply"
                    .to_string(),
            ));
        }
    } else {
        if existing_native_route {
            return Err((
                "pftl_uniswap_opening_inventory_double_count",
                "opening inventory cannot be attached when another route already tracks the native NAV asset"
                    .to_string(),
            ));
        }
        if issued_supply != operation.opening_inventory_atoms
            || native_nav_asset.circulating_supply != operation.opening_inventory_atoms
        {
            return Err((
                "pftl_uniswap_opening_inventory_supply_mismatch",
                "opening inventory must equal both issued supply and proof-backed NAV circulating supply"
                    .to_string(),
            ));
        }
        let opening_line = ledger
            .trustline_for_account_asset(
                &operation.opening_inventory_holder,
                &operation.native_nav_asset_id,
            )
            .ok_or_else(|| {
                (
                    "pftl_uniswap_opening_inventory_missing",
                    "opening inventory holder has no native NAV trustline".to_string(),
                )
            })?;
        if opening_line.balance != operation.opening_inventory_atoms
            || (native_definition.requires_authorization && !opening_line.authorized)
            || opening_line.frozen
        {
            return Err((
                "pftl_uniswap_opening_inventory_balance_mismatch",
                "opening holder must own the full movable proof-backed NAV supply".to_string(),
            ));
        }
        opening_balances.insert(
            operation.opening_inventory_holder.clone(),
            operation.opening_inventory_atoms,
        );
    }
    let state_before_hash = pftl_uniswap_absent_route_hash(&operation.route_id);
    let route = PftlUniswapConsensusRouteState {
        route_id: operation.route_id.clone(),
        route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
        route_config_digest: operation.route_config_digest.clone(),
        route_trust_class: operation.return_verification_class.clone(),
        native_nav_asset_id: operation.native_nav_asset_id.clone(),
        settlement_asset_id: operation.settlement_asset_id.clone(),
        handoff_controller: operation.handoff_controller.clone(),
        settlement_adapter: operation.settlement_adapter.clone(),
        wrapped_navcoin_token: operation.wrapped_navcoin_token.clone(),
        ethereum_chain_id: operation.ethereum_chain_id,
        route_supply_cap_atoms: operation.route_supply_cap_atoms,
        packet_notional_cap_atoms: operation.packet_notional_cap_atoms,
        latest_finalized_nav_epoch: native_nav_asset.finalized_epoch,
        return_finality_blocks: operation.return_finality_blocks,
        live_value_enabled: operation.live_value_enabled,
        ethereum_verification_policy: Some(operation.ethereum_verification_policy.clone()),
        authorized_valid_supply_atoms: operation.opening_inventory_atoms,
        pftl_spendable_supply_atoms: operation.opening_inventory_atoms,
        native_spendable_balances_atoms: opening_balances,
        ethereum_spendable_supply_atoms: 0,
        other_registered_venue_supply_atoms: 0,
        outstanding_bridge_claims_atoms: 0,
        pending_return_import_claims_atoms: 0,
        settlement_reserve_atoms: 0,
        primary_subscription_nonces: std::collections::BTreeMap::new(),
        export_packets: std::collections::BTreeMap::new(),
        export_nonces: std::collections::BTreeMap::new(),
        return_imports: std::collections::BTreeMap::new(),
        paused: false,
        v2: Some(PftlUniswapRouteV2State {
            route_schema_version: PFTL_UNISWAP_ROUTE_SCHEMA_V2,
            route_epoch: operation.route_epoch,
            outbound_verification_class: operation.outbound_verification_class.clone(),
            return_verification_class: operation.return_verification_class.clone(),
            primary_market_policy: operation.primary_market_policy.clone(),
            issue_capacity_used_atoms: 0,
            redeem_capacity_used_atoms: 0,
            non_nav_spread_atoms: 0,
            active_reservations: std::collections::BTreeMap::new(),
            export_entitlements: std::collections::BTreeMap::new(),
            terminal_reservations: std::collections::BTreeMap::new(),
            redemption_nonces: std::collections::BTreeMap::new(),
        }),
    };
    route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&route);
    ledger.pftl_uniswap_routes.push(route);
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "route_init",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: None,
            burn_event_hash: None,
            wallet: Some(operation.operator.clone()),
            amount_atoms: None,
            block_height,
        },
    )
}

fn checked_mul_div_ceil(
    value: u64,
    multiplier: u32,
    denominator: u32,
) -> Result<u64, (&'static str, String)> {
    let numerator = u128::from(value)
        .checked_mul(u128::from(multiplier))
        .ok_or_else(|| {
            (
                "pftl_uniswap_pricing_overflow",
                "primary-market multiplier would overflow".to_string(),
            )
        })?;
    let denominator = u128::from(denominator);
    let result = numerator
        .checked_add(denominator - 1)
        .ok_or_else(|| {
            (
                "pftl_uniswap_pricing_overflow",
                "primary-market rounding would overflow".to_string(),
            )
        })?
        / denominator;
    u64::try_from(result).map_err(|_| {
        (
            "pftl_uniswap_pricing_overflow",
            "primary-market result exceeds u64".to_string(),
        )
    })
}

fn checked_mul_div_floor(
    value: u64,
    multiplier: u32,
    denominator: u32,
) -> Result<u64, (&'static str, String)> {
    let result = u128::from(value)
        .checked_mul(u128::from(multiplier))
        .ok_or_else(|| {
            (
                "pftl_uniswap_pricing_overflow",
                "primary-market multiplier would overflow".to_string(),
            )
        })?
        / u128::from(denominator);
    u64::try_from(result).map_err(|_| {
        (
            "pftl_uniswap_pricing_overflow",
            "primary-market result exceeds u64".to_string(),
        )
    })
}

fn pftl_uniswap_v2_base_value(
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
    nav_asset: &NavTrackedAsset,
    nav_amount_atoms: u64,
) -> Result<u64, (&'static str, String)> {
    let native_definition = ledger
        .asset_definition(&route.native_nav_asset_id)
        .ok_or_else(|| {
            (
                "missing_native_nav_asset",
                "PFTL-Uniswap native asset definition is missing".to_string(),
            )
        })?;
    let settlement_definition = ledger
        .asset_definition(&route.settlement_asset_id)
        .ok_or_else(|| {
            (
                "missing_settlement_asset",
                "PFTL-Uniswap settlement asset definition is missing".to_string(),
            )
        })?;
    let settlement_nav_asset = ledger
        .nav_asset(&route.settlement_asset_id)
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_settlement_nav_asset",
                "PFTL-Uniswap settlement asset must be NAV-registered".to_string(),
            )
        })?;
    required_vault_bridge_settlement_atoms(
        nav_amount_atoms,
        native_definition.precision,
        nav_asset.nav_per_unit,
        &nav_asset.valuation_unit,
        &settlement_nav_asset.valuation_unit,
        settlement_definition.precision,
    )
}

fn apply_pftl_uniswap_order_reserve(
    ledger: &mut LedgerState,
    operation: &PftlUniswapOrderReserveOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let mut next_route = ledger.pftl_uniswap_routes[route_index].clone();
    ensure_pftl_uniswap_route_live(&next_route, false)?;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    let pricing_nav_asset =
        pftl_uniswap_pricing_nav_asset(ledger, &next_route, block_height)?;
    let quoted_base = pftl_uniswap_v2_base_value(
        ledger,
        &next_route,
        &pricing_nav_asset,
        operation.mint_amount_atoms,
    )?;
    let issue_multiplier_bps = next_route
        .v2
        .as_ref()
        .ok_or_else(|| {
            (
                "pftl_uniswap_v2_required",
                "order reservations require a v2 route".to_string(),
            )
        })?
        .primary_market_policy
        .issue_multiplier_bps;
    let quoted_due = checked_mul_div_ceil(
        quoted_base,
        issue_multiplier_bps,
        postfiat_types::PFTL_UNISWAP_BPS_DENOMINATOR,
    )?;
    let v2 = next_route.v2.as_mut().ok_or_else(|| {
        (
            "pftl_uniswap_v2_required",
            "order reservations require a v2 route".to_string(),
        )
    })?;
    let policy = &v2.primary_market_policy;
    if block_height < policy.valid_from_height || block_height > policy.expires_at_height {
        return Err((
            "pftl_uniswap_policy_inactive",
            "primary-market policy is not active at this height".to_string(),
        ));
    }
    if operation.route_epoch != v2.route_epoch
        || operation.policy_epoch != policy.policy_epoch
        || operation.policy_hash != policy.policy_hash
    {
        return Err((
            "pftl_uniswap_reservation_policy_mismatch",
            "reservation is not pinned to the active route and policy".to_string(),
        ));
    }
    if operation.mint_amount_atoms < policy.min_order_atoms
        || operation.mint_amount_atoms > policy.max_order_atoms
        || operation.expires_at_height <= block_height
        || operation.expires_at_height > policy.expires_at_height
        || operation.expires_at_height.saturating_sub(block_height) > 2_048
    {
        return Err((
            "pftl_uniswap_reservation_bounds",
            "reservation amount or expiry is outside active policy bounds".to_string(),
        ));
    }
    let subscriber_commitment_count = v2
        .active_reservations
        .values()
        .filter(|reservation| reservation.subscriber == operation.subscriber)
        .count()
        .saturating_add(
            v2.export_entitlements
                .values()
                .filter(|entitlement| entitlement.subscriber == operation.subscriber)
                .count(),
        );
    if subscriber_commitment_count >= 4 {
        return Err((
            "pftl_uniswap_wallet_reservation_limit",
            "subscriber already has the maximum of four active reservations or export entitlements"
                .to_string(),
        ));
    }
    if v2.active_reservations.contains_key(&operation.reservation_id)
        || v2
            .terminal_reservations
            .contains_key(&operation.reservation_id)
    {
        return Err((
            "duplicate_pftl_uniswap_reservation",
            "reservation identifier has already been used".to_string(),
        ));
    }
    let reserved_atoms = v2.active_reservations.values().try_fold(0_u64, |sum, row| {
        sum.checked_add(row.mint_amount_atoms).ok_or_else(|| {
            (
                "pftl_uniswap_capacity_overflow",
                "reserved issue capacity would overflow".to_string(),
            )
        })
    })?;
    let committed = v2
        .issue_capacity_used_atoms
        .checked_add(reserved_atoms)
        .and_then(|amount| amount.checked_add(operation.mint_amount_atoms))
        .ok_or_else(|| {
            (
                "pftl_uniswap_capacity_overflow",
                "committed issue capacity would overflow".to_string(),
            )
        })?;
    if committed > policy.issue_capacity_atoms {
        return Err((
            "pftl_uniswap_issue_capacity_exceeded",
            "reservation would exceed primary issue capacity".to_string(),
        ));
    }
    if operation.max_settlement_value_atoms < quoted_due {
        return Err((
            "pftl_uniswap_reservation_underfunded",
            "reservation maximum is below the deterministic policy quote".to_string(),
        ));
    }
    // The signed maximum is held immediately. This makes reservations
    // sybil-resistant and guarantees the later subscription can settle
    // without an operator or a second value authorization.
    debit_issued_asset_balance(
        ledger,
        &operation.subscriber,
        &next_route.settlement_asset_id,
        operation.max_settlement_value_atoms,
        "PFTL-Uniswap order reservation escrow",
    )?;
    v2.active_reservations.insert(
        operation.reservation_id.clone(),
        PftlUniswapOrderReservationV2 {
            reservation_id: operation.reservation_id.clone(),
            subscriber: operation.subscriber.clone(),
            ethereum_recipient: operation.ethereum_recipient.clone(),
            route_epoch: operation.route_epoch,
            policy_epoch: operation.policy_epoch,
            policy_hash: operation.policy_hash.clone(),
            mint_amount_atoms: operation.mint_amount_atoms,
            max_settlement_value_atoms: operation.max_settlement_value_atoms,
            created_at_height: block_height,
            expires_at_height: operation.expires_at_height,
            status: PFTL_UNISWAP_ORDER_STATUS_RESERVED.to_string(),
        },
    );
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "order_reserved",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: Some(operation.reservation_id.clone()),
            burn_event_hash: None,
            wallet: Some(operation.subscriber.clone()),
            amount_atoms: Some(operation.mint_amount_atoms),
            block_height,
        },
    )
}

fn apply_pftl_uniswap_order_release(
    ledger: &mut LedgerState,
    operation: &PftlUniswapOrderReleaseOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let mut next_route = ledger.pftl_uniswap_routes[route_index].clone();
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    let v2 = next_route.v2.as_mut().ok_or_else(|| {
        (
            "pftl_uniswap_v2_required",
            "order release requires a v2 route".to_string(),
        )
    })?;
    let (transition, owner, amount_atoms, escrow_refund) =
        if let Some(reservation) = v2.active_reservations.get(&operation.reservation_id).cloned() {
            if operation.releaser != reservation.subscriber
                && block_height <= reservation.expires_at_height
            {
                return Err((
                    "unauthorized_pftl_uniswap_reservation_release",
                    "only the subscriber may release before expiry".to_string(),
                ));
            }
            v2.active_reservations.remove(&operation.reservation_id);
            v2.terminal_reservations.insert(
                operation.reservation_id.clone(),
                PFTL_UNISWAP_ORDER_STATUS_RELEASED.to_string(),
            );
            (
                "order_released",
                reservation.subscriber,
                reservation.mint_amount_atoms,
                Some(reservation.max_settlement_value_atoms),
            )
        } else if let Some(entitlement) =
            v2.export_entitlements.get(&operation.reservation_id).cloned()
        {
            if operation.releaser != entitlement.subscriber
                && block_height <= entitlement.expires_at_height
            {
                return Err((
                    "unauthorized_pftl_uniswap_reservation_release",
                    "only the subscriber may release export capacity before expiry".to_string(),
                ));
            }
            v2.export_entitlements.remove(&operation.reservation_id);
            (
                "export_entitlement_released",
                entitlement.subscriber,
                entitlement.remaining_amount_atoms,
                None,
            )
        } else {
            return Err((
                "missing_pftl_uniswap_reservation",
                "active reservation or export entitlement does not exist".to_string(),
            ));
        };
    if let Some(refund_atoms) = escrow_refund {
        credit_issued_asset_balance_from_custody(
            ledger,
            &owner,
            &next_route.settlement_asset_id,
            refund_atoms,
            "PFTL-Uniswap released reservation refund",
        )?;
    }
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition,
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: Some(operation.reservation_id.clone()),
            burn_event_hash: None,
            wallet: Some(operation.releaser.clone()),
            amount_atoms: Some(amount_atoms),
            block_height,
        },
    )
}

fn apply_pftl_uniswap_primary_subscribe_v2(
    ledger: &mut LedgerState,
    operation: &PftlUniswapPrimarySubscribeV2Operation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let route = ledger.pftl_uniswap_routes[route_index].clone();
    ensure_pftl_uniswap_route_live(&route, false)?;
    if route.settlement_asset_id != operation.settlement_asset_id
        || route
            .primary_subscription_nonces
            .contains_key(&operation.subscription_nonce)
    {
        return Err((
            "pftl_uniswap_subscription_binding",
            "settlement asset must match and subscription nonce must be unused".to_string(),
        ));
    }
    let v2 = route.v2.as_ref().ok_or_else(|| {
        (
            "pftl_uniswap_v2_required",
            "v2 subscription requires a v2 route".to_string(),
        )
    })?;
    let reservation = v2
        .active_reservations
        .get(&operation.reservation_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_reservation",
                "v2 subscription requires an active reservation".to_string(),
            )
        })?;
    if reservation.subscriber != operation.subscriber || block_height > reservation.expires_at_height
    {
        return Err((
            "pftl_uniswap_reservation_owner_or_expiry",
            "reservation owner must match and reservation must be unexpired".to_string(),
        ));
    }
    let policy = &v2.primary_market_policy;
    let nav_asset = pftl_uniswap_pricing_nav_asset(ledger, &route, block_height)?;
    if operation.pricing_nav_epoch != nav_asset.finalized_epoch
        || operation.pricing_nav_epoch != policy.pricing_nav_epoch
        || operation.pricing_reserve_packet_hash != nav_asset.finalized_reserve_packet_hash
        || operation.pricing_reserve_packet_hash != policy.pricing_reserve_packet_hash
    {
        return Err((
            "pftl_uniswap_pricing_binding_mismatch",
            "subscription must match the policy-pinned finalized NAV packet".to_string(),
        ));
    }
    let policy_fresh_until = nav_asset
        .finalized_at_height
        .checked_add(policy.max_nav_age_blocks)
        .ok_or_else(|| {
            (
                "pftl_uniswap_pricing_height_overflow",
                "policy NAV freshness height would overflow".to_string(),
            )
        })?;
    if block_height > policy_fresh_until {
        return Err((
            "stale_pftl_uniswap_policy_pricing",
            "policy-pinned NAV is older than the policy freshness limit".to_string(),
        ));
    }
    let base_value =
        pftl_uniswap_v2_base_value(ledger, &route, &nav_asset, reservation.mint_amount_atoms)?;
    let settlement_due = checked_mul_div_ceil(
        base_value,
        policy.issue_multiplier_bps,
        postfiat_types::PFTL_UNISWAP_BPS_DENOMINATOR,
    )?;
    if operation.settlement_value_atoms != settlement_due
        || settlement_due > reservation.max_settlement_value_atoms
    {
        return Err((
            "pftl_uniswap_subscription_slippage",
            "settlement debit does not equal deterministic policy price or exceeds reservation maximum"
                .to_string(),
        ));
    }
    let spread = settlement_due.checked_sub(base_value).ok_or_else(|| {
        (
            "pftl_uniswap_pricing_underflow",
            "issue settlement is below base NAV".to_string(),
        )
    })?;
    let supply_after = route
        .authorized_valid_supply_atoms
        .checked_add(reservation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_overflow",
                "v2 subscription supply would overflow".to_string(),
            )
        })?;
    let circulating_after = nav_asset
        .circulating_supply
        .checked_add(reservation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_nav_supply_overflow",
                "v2 subscription NAV supply would overflow".to_string(),
            )
        })?;
    let reservation_refund = reservation
        .max_settlement_value_atoms
        .checked_sub(settlement_due)
        .ok_or_else(|| {
            (
                "pftl_uniswap_reservation_underflow",
                "reservation escrow is below settlement due".to_string(),
            )
        })?;
    if reservation_refund != 0 {
        credit_issued_asset_balance_from_custody(
            ledger,
            &operation.subscriber,
            &operation.settlement_asset_id,
            reservation_refund,
            "PFTL-Uniswap subscription reservation refund",
        )?;
    }
    credit_issued_asset_balance(
        ledger,
        &operation.subscriber,
        &route.native_nav_asset_id,
        reservation.mint_amount_atoms,
        "PFTL-Uniswap v2 subscription NAV credit",
    )?;
    let mut next_route = route;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    next_route.primary_subscription_nonces.insert(
        operation.subscription_nonce.clone(),
        operation.subscriber.clone(),
    );
    next_route.authorized_valid_supply_atoms = supply_after;
    next_route.pftl_spendable_supply_atoms = next_route
        .pftl_spendable_supply_atoms
        .checked_add(reservation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_overflow",
                "v2 PFTL spendable supply would overflow".to_string(),
            )
        })?;
    next_route.settlement_reserve_atoms = next_route
        .settlement_reserve_atoms
        .checked_add(base_value)
        .ok_or_else(|| {
            (
                "pftl_uniswap_reserve_overflow",
                "v2 settlement NAV reserve would overflow".to_string(),
            )
        })?;
    pftl_uniswap_credit_native_route_balance(
        &mut next_route,
        &operation.subscriber,
        reservation.mint_amount_atoms,
    )?;
    let next_v2 = next_route.v2.as_mut().expect("v2 route checked above");
    next_v2.active_reservations.remove(&operation.reservation_id);
    next_v2.terminal_reservations.insert(
        operation.reservation_id.clone(),
        PFTL_UNISWAP_ORDER_STATUS_CONSUMED.to_string(),
    );
    next_v2.export_entitlements.insert(
        operation.reservation_id.clone(),
        PftlUniswapExportEntitlementV2 {
            reservation_id: operation.reservation_id.clone(),
            subscriber: reservation.subscriber.clone(),
            ethereum_recipient: reservation.ethereum_recipient.clone(),
            route_epoch: reservation.route_epoch,
            policy_epoch: reservation.policy_epoch,
            policy_hash: reservation.policy_hash.clone(),
            remaining_amount_atoms: reservation.mint_amount_atoms,
            expires_at_height: reservation.expires_at_height,
        },
    );
    next_v2.issue_capacity_used_atoms = next_v2
        .issue_capacity_used_atoms
        .checked_add(reservation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_capacity_overflow",
                "v2 used issue capacity would overflow".to_string(),
            )
        })?;
    next_v2.non_nav_spread_atoms = next_v2
        .non_nav_spread_atoms
        .checked_add(spread)
        .ok_or_else(|| {
            (
                "pftl_uniswap_spread_overflow",
                "v2 non-NAV issue spread would overflow".to_string(),
            )
        })?;
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger
        .nav_asset_mut(&next_route.native_nav_asset_id)
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_nav_asset",
                "v2 subscription native NAV asset disappeared".to_string(),
            )
        })?
        .circulating_supply = circulating_after;
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "primary_subscription",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: Some(operation.reservation_id.clone()),
            burn_event_hash: None,
            wallet: Some(operation.subscriber.clone()),
            amount_atoms: Some(reservation.mint_amount_atoms),
            block_height,
        },
    )
}

/// Applies the governed route/supply half of a private AssetOrchard primary
/// issue. The caller must execute this against a trial ledger and atomically
/// couple it to settlement-custody debit, proof/nullifier consumption, and the
/// encrypted NAV output append.
pub fn apply_asset_orchard_private_primary_issue_route_transition(
    ledger: &mut LedgerState,
    operation: &AssetOrchardPrivatePrimaryIssueActionPayload,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let route = ledger.pftl_uniswap_routes[route_index].clone();
    ensure_pftl_uniswap_route_live(&route, false)?;
    let v2 = route.v2.as_ref().ok_or_else(|| {
        (
            "pftl_uniswap_v2_required",
            "private primary issue requires a v2 route".to_string(),
        )
    })?;
    let policy = &v2.primary_market_policy;
    if block_height > operation.expires_at_height
        || block_height < policy.valid_from_height
        || block_height > policy.expires_at_height
        || operation.route_epoch != v2.route_epoch
        || operation.policy_epoch != policy.policy_epoch
        || operation.policy_hash != policy.policy_hash
        || operation.mint_amount_atoms < policy.min_order_atoms
        || operation.mint_amount_atoms > policy.max_order_atoms
        || route
            .primary_subscription_nonces
            .contains_key(&operation.subscription_nonce)
    {
        return Err((
            "pftl_uniswap_private_primary_policy_mismatch",
            "private primary expiry, route/policy binding, amount, or nonce is invalid".to_string(),
        ));
    }
    if v2
        .active_reservations
        .contains_key(&operation.reservation_id)
        || v2
            .terminal_reservations
            .contains_key(&operation.reservation_id)
        || v2
            .export_entitlements
            .contains_key(&operation.reservation_id)
    {
        return Err((
            "pftl_uniswap_private_primary_reservation_replay",
            "private primary reservation identifier is already in use".to_string(),
        ));
    }
    let issue_capacity_after = v2
        .issue_capacity_used_atoms
        .checked_add(operation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_capacity_overflow",
                "private primary issue capacity would overflow".to_string(),
            )
        })?;
    if issue_capacity_after > policy.issue_capacity_atoms {
        return Err((
            "pftl_uniswap_issue_capacity_exceeded",
            "private primary issue exceeds active policy capacity".to_string(),
        ));
    }
    let nav_asset = pftl_uniswap_pricing_nav_asset(ledger, &route, block_height)?;
    if operation.pricing_nav_epoch != nav_asset.finalized_epoch
        || operation.pricing_nav_epoch != policy.pricing_nav_epoch
        || operation.pricing_reserve_packet_hash != nav_asset.finalized_reserve_packet_hash
        || operation.pricing_reserve_packet_hash != policy.pricing_reserve_packet_hash
    {
        return Err((
            "pftl_uniswap_pricing_binding_mismatch",
            "private primary issue must match the policy-pinned finalized NAV packet".to_string(),
        ));
    }
    let policy_fresh_until = nav_asset
        .finalized_at_height
        .checked_add(policy.max_nav_age_blocks)
        .ok_or_else(|| {
            (
                "pftl_uniswap_pricing_height_overflow",
                "policy NAV freshness height would overflow".to_string(),
            )
        })?;
    if block_height > policy_fresh_until {
        return Err((
            "stale_pftl_uniswap_policy_pricing",
            "policy-pinned NAV is older than the policy freshness limit".to_string(),
        ));
    }
    let base_value =
        pftl_uniswap_v2_base_value(ledger, &route, &nav_asset, operation.mint_amount_atoms)?;
    let settlement_due = checked_mul_div_ceil(
        base_value,
        policy.issue_multiplier_bps,
        postfiat_types::PFTL_UNISWAP_BPS_DENOMINATOR,
    )?;
    if operation.settlement_value_atoms != settlement_due {
        return Err((
            "pftl_uniswap_subscription_slippage",
            "private primary settlement value does not equal the deterministic policy price"
                .to_string(),
        ));
    }
    let spread = settlement_due.checked_sub(base_value).ok_or_else(|| {
        (
            "pftl_uniswap_pricing_underflow",
            "private primary settlement is below base NAV".to_string(),
        )
    })?;
    let supply_after = route
        .authorized_valid_supply_atoms
        .checked_add(operation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_overflow",
                "private primary authorized supply would overflow".to_string(),
            )
        })?;
    let circulating_after = nav_asset
        .circulating_supply
        .checked_add(operation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_nav_supply_overflow",
                "private primary NAV circulating supply would overflow".to_string(),
            )
        })?;

    let mut next_route = route;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    next_route.primary_subscription_nonces.insert(
        operation.subscription_nonce.clone(),
        operation.subscriber.clone(),
    );
    next_route.authorized_valid_supply_atoms = supply_after;
    next_route.pftl_spendable_supply_atoms = next_route
        .pftl_spendable_supply_atoms
        .checked_add(operation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_overflow",
                "private primary PFTL spendable supply would overflow".to_string(),
            )
        })?;
    next_route.settlement_reserve_atoms = next_route
        .settlement_reserve_atoms
        .checked_add(base_value)
        .ok_or_else(|| {
            (
                "pftl_uniswap_reserve_overflow",
                "private primary settlement NAV reserve would overflow".to_string(),
            )
        })?;
    pftl_uniswap_credit_native_route_balance(
        &mut next_route,
        &operation.subscriber,
        operation.mint_amount_atoms,
    )?;
    let next_v2 = next_route.v2.as_mut().expect("v2 route checked above");
    next_v2.terminal_reservations.insert(
        operation.reservation_id.clone(),
        PFTL_UNISWAP_ORDER_STATUS_CONSUMED.to_string(),
    );
    next_v2.export_entitlements.insert(
        operation.reservation_id.clone(),
        PftlUniswapExportEntitlementV2 {
            reservation_id: operation.reservation_id.clone(),
            subscriber: operation.subscriber.clone(),
            ethereum_recipient: operation.ethereum_recipient.clone(),
            route_epoch: operation.route_epoch,
            policy_epoch: operation.policy_epoch,
            policy_hash: operation.policy_hash.clone(),
            remaining_amount_atoms: operation.mint_amount_atoms,
            expires_at_height: operation.expires_at_height,
        },
    );
    next_v2.issue_capacity_used_atoms = issue_capacity_after;
    next_v2.non_nav_spread_atoms = next_v2
        .non_nav_spread_atoms
        .checked_add(spread)
        .ok_or_else(|| {
            (
                "pftl_uniswap_spread_overflow",
                "private primary non-NAV issue spread would overflow".to_string(),
            )
        })?;
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger
        .nav_asset_mut(&next_route.native_nav_asset_id)
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_nav_asset",
                "private primary native NAV asset disappeared".to_string(),
            )
        })?
        .circulating_supply = circulating_after;
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "private_primary_subscription",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: Some(operation.reservation_id.clone()),
            burn_event_hash: None,
            wallet: Some(operation.subscriber.clone()),
            amount_atoms: Some(operation.mint_amount_atoms),
            block_height,
        },
    )
}

pub fn apply_asset_orchard_private_primary_redeem_route_transition(
    ledger: &mut LedgerState,
    operation: &AssetOrchardPrivatePrimaryRedeemActionPayload,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let route = ledger.pftl_uniswap_routes[route_index].clone();
    // Redemption remains available while inbound issue/export is paused.
    let v2 = route.v2.as_ref().ok_or_else(|| {
        (
            "pftl_uniswap_v2_required",
            "private primary redemption requires a v2 route".to_string(),
        )
    })?;
    let policy = &v2.primary_market_policy;
    if block_height > operation.expires_at_height
        || block_height < policy.valid_from_height
        || block_height > policy.expires_at_height
        || operation.route_epoch != v2.route_epoch
        || operation.policy_epoch != policy.policy_epoch
        || operation.policy_hash != policy.policy_hash
        || operation.mint_amount_atoms < policy.min_order_atoms
        || operation.mint_amount_atoms > policy.max_order_atoms
        || v2
            .redemption_nonces
            .contains_key(&operation.subscription_nonce)
    {
        return Err((
            "pftl_uniswap_private_redemption_policy_mismatch",
            "private redemption expiry, policy binding, amount, or nonce is invalid".to_string(),
        ));
    }
    let redeemed_after = v2
        .redeem_capacity_used_atoms
        .checked_add(operation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_capacity_overflow",
                "private redemption capacity would overflow".to_string(),
            )
        })?;
    if redeemed_after > policy.redeem_capacity_atoms {
        return Err((
            "pftl_uniswap_redeem_capacity_exceeded",
            "private redemption exceeds active policy capacity".to_string(),
        ));
    }
    let nav_asset = pftl_uniswap_pricing_nav_asset(ledger, &route, block_height)?;
    if operation.pricing_nav_epoch != nav_asset.finalized_epoch
        || operation.pricing_nav_epoch != policy.pricing_nav_epoch
        || operation.pricing_reserve_packet_hash != nav_asset.finalized_reserve_packet_hash
        || operation.pricing_reserve_packet_hash != policy.pricing_reserve_packet_hash
    {
        return Err((
            "pftl_uniswap_pricing_binding_mismatch",
            "private redemption must match the policy-pinned finalized NAV packet".to_string(),
        ));
    }
    let policy_fresh_until = nav_asset
        .finalized_at_height
        .checked_add(policy.max_nav_age_blocks)
        .ok_or_else(|| {
            (
                "pftl_uniswap_pricing_height_overflow",
                "policy NAV freshness height would overflow".to_string(),
            )
        })?;
    if block_height > policy_fresh_until {
        return Err((
            "stale_pftl_uniswap_policy_pricing",
            "policy-pinned NAV is older than the policy freshness limit".to_string(),
        ));
    }
    let base_value =
        pftl_uniswap_v2_base_value(ledger, &route, &nav_asset, operation.mint_amount_atoms)?;
    let settlement_output = checked_mul_div_floor(
        base_value,
        policy.redeem_multiplier_bps,
        postfiat_types::PFTL_UNISWAP_BPS_DENOMINATOR,
    )?;
    if operation.settlement_value_atoms != settlement_output
        || route.settlement_reserve_atoms < base_value
    {
        return Err((
            "pftl_uniswap_private_redemption_unavailable",
            "private redemption output or subscription-funded reserve principal is invalid"
                .to_string(),
        ));
    }
    let spread = base_value.checked_sub(settlement_output).ok_or_else(|| {
        (
            "pftl_uniswap_pricing_underflow",
            "private redemption output exceeds base NAV".to_string(),
        )
    })?;
    let mut next_route = route;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    pftl_uniswap_debit_native_route_balance(
        &mut next_route,
        &operation.subscriber,
        operation.mint_amount_atoms,
    )?;
    next_route.authorized_valid_supply_atoms = next_route
        .authorized_valid_supply_atoms
        .checked_sub(operation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_underflow",
                "private redemption exceeds route authorized supply".to_string(),
            )
        })?;
    next_route.pftl_spendable_supply_atoms = next_route
        .pftl_spendable_supply_atoms
        .checked_sub(operation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_underflow",
                "private redemption exceeds route PFTL spendable supply".to_string(),
            )
        })?;
    next_route.settlement_reserve_atoms = next_route
        .settlement_reserve_atoms
        .checked_sub(base_value)
        .ok_or_else(|| {
            (
                "pftl_uniswap_reserve_underflow",
                "private redemption exceeds route settlement reserve".to_string(),
            )
        })?;
    let next_v2 = next_route.v2.as_mut().expect("v2 route checked above");
    next_v2.redeem_capacity_used_atoms = redeemed_after;
    next_v2.non_nav_spread_atoms = next_v2
        .non_nav_spread_atoms
        .checked_add(spread)
        .ok_or_else(|| {
            (
                "pftl_uniswap_spread_overflow",
                "private redemption spread would overflow non-NAV fee custody".to_string(),
            )
        })?;
    next_v2.redemption_nonces.insert(
        operation.subscription_nonce.clone(),
        operation.subscriber.clone(),
    );
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let circulating_after = nav_asset
        .circulating_supply
        .checked_sub(operation.mint_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_nav_supply_underflow",
                "private redemption exceeds NAV circulating supply".to_string(),
            )
        })?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger
        .nav_asset_mut(&next_route.native_nav_asset_id)
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_nav_asset",
                "private redemption native NAV asset disappeared".to_string(),
            )
        })?
        .circulating_supply = circulating_after;
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "private_primary_redeemed",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: Some(operation.reservation_id.clone()),
            burn_event_hash: None,
            wallet: Some(operation.subscriber.clone()),
            amount_atoms: Some(operation.mint_amount_atoms),
            block_height,
        },
    )
}

fn apply_pftl_uniswap_redemption_fund(
    _ledger: &mut LedgerState,
    _operation: &PftlUniswapRedemptionFundOperation,
    _block_height: u64,
) -> Result<(), (&'static str, String)> {
    Err((
        "pftl_uniswap_redemption_fund_retired",
        "separate redemption funding is retired; primary redemption releases subscription-funded NAV reserve principal"
            .to_string(),
    ))
}

fn apply_pftl_uniswap_primary_redeem(
    ledger: &mut LedgerState,
    operation: &PftlUniswapPrimaryRedeemOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let route = ledger.pftl_uniswap_routes[route_index].clone();
    // Inbound pause/disable is not an escape hatch from redemption. Policy
    // freshness, supply, and subscription-funded reserve principal remain
    // fail-closed while users can exit.
    let v2 = route.v2.as_ref().ok_or_else(|| {
        (
            "pftl_uniswap_v2_required",
            "primary redemption requires a v2 route".to_string(),
        )
    })?;
    let policy = &v2.primary_market_policy;
    if block_height > operation.expires_at_height
        || block_height < policy.valid_from_height
        || block_height > policy.expires_at_height
        || operation.route_epoch != v2.route_epoch
        || operation.policy_epoch != policy.policy_epoch
        || operation.policy_hash != policy.policy_hash
        || operation.nav_amount_atoms < policy.min_order_atoms
        || operation.nav_amount_atoms > policy.max_order_atoms
        || v2.redemption_nonces.contains_key(&operation.redemption_nonce)
    {
        return Err((
            "pftl_uniswap_redemption_policy_mismatch",
            "redemption expiry, policy binding, amount, or nonce is invalid".to_string(),
        ));
    }
    let redeemed_after = v2
        .redeem_capacity_used_atoms
        .checked_add(operation.nav_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_capacity_overflow",
                "redemption capacity would overflow".to_string(),
            )
        })?;
    if redeemed_after > policy.redeem_capacity_atoms {
        return Err((
            "pftl_uniswap_redeem_capacity_exceeded",
            "redemption exceeds active policy capacity".to_string(),
        ));
    }
    let nav_asset = pftl_uniswap_pricing_nav_asset(ledger, &route, block_height)?;
    if operation.pricing_nav_epoch != nav_asset.finalized_epoch
        || operation.pricing_nav_epoch != policy.pricing_nav_epoch
        || operation.pricing_reserve_packet_hash != nav_asset.finalized_reserve_packet_hash
        || operation.pricing_reserve_packet_hash != policy.pricing_reserve_packet_hash
    {
        return Err((
            "pftl_uniswap_pricing_binding_mismatch",
            "redemption must match the policy-pinned finalized NAV packet".to_string(),
        ));
    }
    let base_value =
        pftl_uniswap_v2_base_value(ledger, &route, &nav_asset, operation.nav_amount_atoms)?;
    let settlement_output = checked_mul_div_floor(
        base_value,
        policy.redeem_multiplier_bps,
        postfiat_types::PFTL_UNISWAP_BPS_DENOMINATOR,
    )?;
    if settlement_output < operation.min_settlement_value_atoms
        || route.settlement_reserve_atoms < base_value
    {
        return Err((
            "pftl_uniswap_redemption_unavailable",
            "redemption output or subscription-funded NAV reserve principal is insufficient"
                .to_string(),
        ));
    }
    let spread = base_value.checked_sub(settlement_output).ok_or_else(|| {
        (
            "pftl_uniswap_pricing_underflow",
            "redemption output exceeds base NAV".to_string(),
        )
    })?;
    debit_issued_asset_balance(
        ledger,
        &operation.owner,
        &route.native_nav_asset_id,
        operation.nav_amount_atoms,
        "PFTL-Uniswap primary redemption NAV debit",
    )?;
    credit_issued_asset_balance_from_custody(
        ledger,
        &operation.settlement_recipient,
        &route.settlement_asset_id,
        settlement_output,
        "PFTL-Uniswap primary redemption settlement credit",
    )?;
    let mut next_route = route;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    pftl_uniswap_debit_native_route_balance(
        &mut next_route,
        &operation.owner,
        operation.nav_amount_atoms,
    )?;
    next_route.authorized_valid_supply_atoms = next_route
        .authorized_valid_supply_atoms
        .checked_sub(operation.nav_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_underflow",
                "redemption exceeds route authorized supply".to_string(),
            )
        })?;
    next_route.pftl_spendable_supply_atoms = next_route
        .pftl_spendable_supply_atoms
        .checked_sub(operation.nav_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_underflow",
                "redemption exceeds route PFTL spendable supply".to_string(),
            )
        })?;
    next_route.settlement_reserve_atoms = next_route
        .settlement_reserve_atoms
        .checked_sub(base_value)
        .ok_or_else(|| {
            (
                "pftl_uniswap_reserve_underflow",
                "redemption exceeds route settlement reserve".to_string(),
            )
        })?;
    let next_v2 = next_route.v2.as_mut().expect("v2 route checked above");
    next_v2.redeem_capacity_used_atoms = redeemed_after;
    next_v2.non_nav_spread_atoms = next_v2
        .non_nav_spread_atoms
        .checked_add(spread)
        .ok_or_else(|| {
            (
                "pftl_uniswap_spread_overflow",
                "redemption spread would overflow non-NAV fee custody".to_string(),
            )
        })?;
    next_v2.redemption_nonces.insert(
        operation.redemption_nonce.clone(),
        operation.owner.clone(),
    );
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let circulating_after = nav_asset
        .circulating_supply
        .checked_sub(operation.nav_amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_nav_supply_underflow",
                "redemption exceeds NAV circulating supply".to_string(),
            )
        })?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger
        .nav_asset_mut(&next_route.native_nav_asset_id)
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_nav_asset",
                "redemption native NAV asset disappeared".to_string(),
            )
        })?
        .circulating_supply = circulating_after;
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "primary_redeemed",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: None,
            burn_event_hash: None,
            wallet: Some(operation.owner.clone()),
            amount_atoms: Some(operation.nav_amount_atoms),
            block_height,
        },
    )
}

fn apply_pftl_uniswap_route_epoch_advance(
    ledger: &mut LedgerState,
    operation: &PftlUniswapRouteEpochAdvanceOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let mut next_route = ledger.pftl_uniswap_routes[route_index].clone();
    ensure_pftl_uniswap_native_asset_policy(
        ledger,
        &next_route.native_nav_asset_id,
        &operation.operator,
    )?;
    let current_nav = ledger
        .nav_asset(&next_route.native_nav_asset_id)
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                "route epoch advance native NAV asset is missing".to_string(),
            )
        })?;
    if operation.next_primary_market_policy.pricing_nav_epoch != current_nav.finalized_epoch
        || operation
            .next_primary_market_policy
            .pricing_reserve_packet_hash
            != current_nav.finalized_reserve_packet_hash
        || block_height < operation.next_primary_market_policy.valid_from_height
        || block_height > operation.next_primary_market_policy.expires_at_height
    {
        return Err((
            "pftl_uniswap_route_epoch_nav_mismatch",
            "next route epoch must pin the current finalized NAV packet and an active policy"
                .to_string(),
        ));
    }
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    let v2 = next_route.v2.as_mut().ok_or_else(|| {
        (
            "pftl_uniswap_v2_required",
            "route epoch advance requires a v2 route".to_string(),
        )
    })?;
    if operation.prior_route_epoch != v2.route_epoch
        || operation.next_route_epoch != v2.route_epoch.saturating_add(1)
        || operation.next_primary_market_policy.policy_epoch
            != v2.primary_market_policy.policy_epoch.saturating_add(1)
        || !v2.active_reservations.is_empty()
        || !v2.export_entitlements.is_empty()
    {
        return Err((
            "pftl_uniswap_route_epoch_advance_blocked",
            "epoch must increment active route/policy and requires no active reservations or export entitlements"
                .to_string(),
        ));
    }
    if operation.next_primary_market_policy.issue_capacity_atoms
        > next_route.route_supply_cap_atoms
        || operation.next_primary_market_policy.redeem_capacity_atoms
            > next_route.route_supply_cap_atoms
        || next_route.packet_notional_cap_atoms
            > operation.next_primary_market_policy.max_order_atoms
    {
        return Err((
            "pftl_uniswap_route_epoch_policy_mismatch",
            "next policy capacities must fit the route supply cap and packet notional cap must fit max order"
                .to_string(),
        ));
    }
    v2.route_epoch = operation.next_route_epoch;
    v2.primary_market_policy = operation.next_primary_market_policy.clone();
    v2.issue_capacity_used_atoms = 0;
    v2.redeem_capacity_used_atoms = 0;
    // Reservation identifiers are epoch/policy-bound, so terminal entries can
    // be pruned at the boundary without enabling same-epoch replay.
    v2.terminal_reservations.clear();
    next_route.route_config_digest = operation.next_route_config_digest.clone();
    next_route.latest_finalized_nav_epoch = operation.next_primary_market_policy.pricing_nav_epoch;
    next_route.live_value_enabled = operation.live_value_enabled;
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "route_epoch_advanced",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: None,
            burn_event_hash: None,
            wallet: Some(operation.operator.clone()),
            amount_atoms: None,
            block_height,
        },
    )
}

fn apply_pftl_uniswap_route_pause(
    ledger: &mut LedgerState,
    operation: &PftlUniswapRoutePauseOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let mut next_route = ledger.pftl_uniswap_routes[route_index].clone();
    ensure_pftl_uniswap_native_asset_policy(
        ledger,
        &next_route.native_nav_asset_id,
        &operation.operator,
    )?;
    if next_route.paused == operation.paused {
        return Err((
            "pftl_uniswap_route_pause_noop",
            "route pause state already equals the requested value".to_string(),
        ));
    }
    if !operation.paused {
        if !next_route.live_value_enabled {
            return Err((
                "pftl_uniswap_route_resume_disabled",
                "route cannot resume while live value is disabled".to_string(),
            ));
        }
        pftl_uniswap_pricing_nav_asset(ledger, &next_route, block_height)?;
    }

    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    next_route.paused = operation.paused;
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: if operation.paused {
                "route_paused"
            } else {
                "route_resumed"
            },
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: None,
            burn_event_hash: None,
            wallet: Some(operation.operator.clone()),
            amount_atoms: None,
            block_height,
        },
    )
}

fn apply_pftl_uniswap_export_debit(
    _genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &PftlUniswapExportDebitOperation,
    block_height: u64,
    allow_disabled_live_value_replay: bool,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let route = ledger.pftl_uniswap_routes[route_index].clone();
    ensure_pftl_uniswap_route_live(&route, allow_disabled_live_value_replay)?;
    let mut effective_reservation_id = operation.reservation_id.clone();
    if let Some(v2) = &route.v2 {
        if let Some(reservation_id) = operation.reservation_id.as_ref() {
            let entitlement = v2.export_entitlements.get(reservation_id).ok_or_else(|| {
                (
                    "missing_pftl_uniswap_export_entitlement",
                    "v2 export reservation has no remaining export entitlement".to_string(),
                )
            })?;
            if entitlement.subscriber != operation.owner
                || entitlement.ethereum_recipient != operation.ethereum_recipient
                || entitlement.remaining_amount_atoms < operation.amount_atoms
                || block_height > entitlement.expires_at_height
            {
                return Err((
                    "pftl_uniswap_export_entitlement_mismatch",
                    "v2 export owner, recipient, amount, or expiry does not match its reservation"
                        .to_string(),
                ));
            }
        } else {
            let opening_inventory_export = v2.issue_capacity_used_atoms == 0
                && v2.active_reservations.is_empty()
                && v2.export_entitlements.is_empty()
                && route.native_spendable_balances_atoms.len() == 1
                && route
                    .native_spendable_balances_atoms
                    .get(&operation.owner)
                    .is_some_and(|balance| *balance == route.pftl_spendable_supply_atoms)
                && operation.amount_atoms == route.pftl_spendable_supply_atoms
                && route.pftl_spendable_supply_atoms == route.authorized_valid_supply_atoms;
            if !opening_inventory_export {
                return Err((
                    "pftl_uniswap_export_entitlement_required",
                    "v2 export must reference its reserved order; only the complete pre-issuance opening inventory may export without one"
                        .to_string(),
                ));
            }
            // The destination packet format requires a unique 48-byte
            // reservation binding. Opening inventory is not a paid
            // subscription, so bind its packet hash as a synthetic,
            // single-use reservation identifier instead.
            effective_reservation_id = Some(operation.packet_hash.clone());
        }
    }
    if route.export_packets.contains_key(&operation.packet_hash) {
        return Err((
            "duplicate_pftl_uniswap_export_packet",
            "export packet hash already exists".to_string(),
        ));
    }
    if route.export_nonces.contains_key(&operation.export_nonce) {
        return Err((
            "duplicate_pftl_uniswap_export_nonce",
            "export nonce already exists".to_string(),
        ));
    }
    if operation.amount_atoms > route.packet_notional_cap_atoms {
        return Err((
            "pftl_uniswap_packet_cap_exceeded",
            "export debit amount exceeds route packet cap".to_string(),
        ));
    }
    let wrapped_exposure_after = route
        .ethereum_spendable_supply_atoms
        .checked_add(route.outstanding_bridge_claims_atoms)
        .and_then(|amount| amount.checked_add(operation.amount_atoms))
        .ok_or_else(|| {
            (
                "pftl_uniswap_exposure_overflow",
                "net wrapped exposure would overflow".to_string(),
            )
        })?;
    if wrapped_exposure_after > route.route_supply_cap_atoms {
        return Err((
            "pftl_uniswap_route_supply_cap_exceeded",
            "export would exceed net wrapped exposure cap".to_string(),
        ));
    }
    if let Some(v2) = &route.v2 {
        let nav_asset = pftl_uniswap_pricing_nav_asset(ledger, &route, block_height)?;
        let base_value =
            pftl_uniswap_v2_base_value(ledger, &route, &nav_asset, operation.amount_atoms)?;
        let settlement_value = checked_mul_div_ceil(
            base_value,
            v2.primary_market_policy.issue_multiplier_bps,
            postfiat_types::PFTL_UNISWAP_BPS_DENOMINATOR,
        )?;
        let policy_hash_commitment = postfiat_types::pftl_uniswap_keccak_commitment48(
            "pftl_uniswap_export.policy_hash",
            &v2.primary_market_policy.policy_hash,
        )
        .map_err(|error| ("bad_pftl_uniswap_export_packet", error))?;
        let digest_packet = PftlUniswapMintPacketV2 {
            route_config_digest: route.route_config_digest.clone(),
            source_packet_hash: operation.packet_hash.clone(),
            reservation_id: effective_reservation_id.clone().ok_or_else(|| {
                (
                    "pftl_uniswap_export_v2_missing_reservation",
                    "v2 export requires a paid reservation or opening-inventory binding"
                        .to_string(),
                )
            })?,
            // Receipt fields are validated for shape but intentionally do not
            // enter the pre-receipt digest.
            source_receipt_hash: "01".repeat(48),
            source_receipt_root: "02".repeat(48),
            settlement_asset_id: route.settlement_asset_id.clone(),
            native_nav_asset_id: route.native_nav_asset_id.clone(),
            pricing_reserve_packet_hash: v2
                .primary_market_policy
                .pricing_reserve_packet_hash
                .clone(),
            policy_hash_commitment,
            route_epoch: v2.route_epoch,
            pricing_nav_epoch: v2.primary_market_policy.pricing_nav_epoch,
            deadline_seconds: operation.destination_deadline_seconds,
            nonce: operation.export_nonce.clone(),
            destination_chain_id: route.ethereum_chain_id,
            destination_controller: route.handoff_controller.clone(),
            wrapped_token: route.wrapped_navcoin_token.clone(),
            ethereum_recipient: operation.ethereum_recipient.clone(),
            mint_amount_atoms: operation.amount_atoms,
            settlement_value_atoms: settlement_value,
        };
        let expected_digest = digest_packet
            .evm_digest()
            .map_err(|error| ("bad_pftl_uniswap_export_packet", error))?;
        if operation.settlement_value_atoms != Some(settlement_value)
            || operation.ethereum_packet_schema_version
                != Some(PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V2)
            || operation.ethereum_packet_digest.as_deref() != Some(expected_digest.as_str())
        {
            return Err((
                "pftl_uniswap_export_v2_binding_mismatch",
                "v2 export must carry the consensus-derived settlement value, schema, and EVM digest"
                    .to_string(),
            ));
        }
    }
    if operation.amount_atoms > route.pftl_spendable_supply_atoms {
        return Err((
            "insufficient_pftl_uniswap_route_supply",
            "export debit exceeds route PFTL spendable supply".to_string(),
        ));
    }
    if operation.amount_atoms
        > route
            .native_spendable_balances_atoms
            .get(&operation.owner)
            .copied()
            .unwrap_or(0)
    {
        return Err((
            "insufficient_pftl_uniswap_wallet_balance",
            "export debit exceeds route wallet native spendable balance".to_string(),
        ));
    }
    debit_issued_asset_balance(
        ledger,
        &operation.owner,
        &route.native_nav_asset_id,
        operation.amount_atoms,
        "PFTL-Uniswap export NAV debit",
    )?;

    let refund_not_before_height = block_height
        .checked_add(operation.refund_delay_blocks)
        .ok_or_else(|| {
            (
                "pftl_uniswap_refund_height_overflow",
                "export refund height would overflow".to_string(),
            )
        })?;
    let packet = PftlUniswapConsensusExportPacket {
        packet_hash: operation.packet_hash.clone(),
        nonce: operation.export_nonce.clone(),
        source_wallet: operation.owner.clone(),
        ethereum_recipient: operation.ethereum_recipient.clone(),
        amount_atoms: operation.amount_atoms,
        settlement_value_atoms: operation.settlement_value_atoms,
        source_height: block_height,
        destination_deadline_seconds: operation.destination_deadline_seconds,
        refund_not_before_height,
        status: PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED.to_string(),
        ethereum_packet_digest: operation.ethereum_packet_digest.clone(),
        ethereum_packet_schema_version: operation.ethereum_packet_schema_version,
        route_epoch: route.v2.as_ref().map(|v2| v2.route_epoch),
        policy_hash: route
            .v2
            .as_ref()
            .map(|v2| v2.primary_market_policy.policy_hash.clone()),
        reservation_id: effective_reservation_id,
    };
    packet
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_export_packet", error))?;

    let mut next_route = route;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    next_route.pftl_spendable_supply_atoms = next_route
        .pftl_spendable_supply_atoms
        .checked_sub(operation.amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_underflow",
                "PFTL spendable route supply would underflow".to_string(),
            )
        })?;
    pftl_uniswap_debit_native_route_balance(
        &mut next_route,
        &operation.owner,
        operation.amount_atoms,
    )?;
    next_route.outstanding_bridge_claims_atoms = next_route
        .outstanding_bridge_claims_atoms
        .checked_add(operation.amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_claim_overflow",
                "outstanding bridge claims would overflow".to_string(),
            )
        })?;
    if let Some(reservation_id) = &operation.reservation_id {
        let next_v2 = next_route.v2.as_mut().ok_or_else(|| {
            (
                "pftl_uniswap_v2_required",
                "export reservation may only be used by a v2 route".to_string(),
            )
        })?;
        let remaining = next_v2
            .export_entitlements
            .get(reservation_id)
            .expect("v2 export entitlement checked above")
            .remaining_amount_atoms
            .checked_sub(operation.amount_atoms)
            .ok_or_else(|| {
                (
                    "pftl_uniswap_export_entitlement_underflow",
                    "export exceeds reserved entitlement".to_string(),
                )
            })?;
        if remaining == 0 {
            next_v2.export_entitlements.remove(reservation_id);
        } else {
            next_v2
                .export_entitlements
                .get_mut(reservation_id)
                .expect("entitlement exists")
                .remaining_amount_atoms = remaining;
        }
    }
    next_route.export_nonces.insert(
        operation.export_nonce.clone(),
        operation.packet_hash.clone(),
    );
    next_route
        .export_packets
        .insert(operation.packet_hash.clone(), packet);
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "export_debit",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: Some(operation.packet_hash.clone()),
            burn_event_hash: None,
            wallet: Some(operation.owner.clone()),
            amount_atoms: Some(operation.amount_atoms),
            block_height,
        },
    )?;
    Ok(())
}

fn apply_pftl_uniswap_refund_source(
    _genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &PftlUniswapRefundSourceOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let route = ledger.pftl_uniswap_routes[route_index].clone();
    let packet = route
        .export_packets
        .get(&operation.packet_hash)
        .cloned()
        .ok_or_else(|| {
            (
                "unknown_pftl_uniswap_export_packet",
                "refund references unknown export packet".to_string(),
            )
        })?;
    ensure_pftl_uniswap_native_asset_policy(
        ledger,
        &route.native_nav_asset_id,
        &operation.operator,
    )?;
    // Pause intentionally does not block refunds: source refunds shrink
    // outstanding exposure and let users exit stale source-debited packets.
    if packet.status != PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED {
        return Err((
            "pftl_uniswap_packet_not_refundable",
            "only source-debited export packets can be refunded".to_string(),
        ));
    }
    let expected_proof_hash = pftl_uniswap_non_consumption_proof_hash(
        &operation.route_id,
        &operation.packet_hash,
        packet.refund_not_before_height,
    )
    .map_err(|error| ("bad_pftl_uniswap_non_consumption_proof", error))?;
    if operation.non_consumption_proof_hash != expected_proof_hash {
        return Err((
            "pftl_uniswap_non_consumption_proof_mismatch",
            "refund non-consumption proof hash does not match canonical commitment".to_string(),
        ));
    }
    if block_height < packet.refund_not_before_height {
        return Err((
            "pftl_uniswap_refund_before_window",
            "export packet cannot be refunded before refund window".to_string(),
        ));
    }
    credit_issued_asset_balance(
        ledger,
        &packet.source_wallet,
        &route.native_nav_asset_id,
        packet.amount_atoms,
        "PFTL-Uniswap export refund NAV credit",
    )?;

    let mut next_route = route;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    let mut refunded = packet.clone();
    refunded.status = PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED.to_string();
    next_route.outstanding_bridge_claims_atoms = next_route
        .outstanding_bridge_claims_atoms
        .checked_sub(packet.amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_claim_underflow",
                "outstanding bridge claims would underflow".to_string(),
            )
        })?;
    next_route.pftl_spendable_supply_atoms = next_route
        .pftl_spendable_supply_atoms
        .checked_add(packet.amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_overflow",
                "PFTL spendable route supply would overflow".to_string(),
            )
        })?;
    pftl_uniswap_credit_native_route_balance(
        &mut next_route,
        &packet.source_wallet,
        packet.amount_atoms,
    )?;
    next_route
        .export_packets
        .insert(operation.packet_hash.clone(), refunded);
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "source_refunded",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: Some(operation.packet_hash.clone()),
            burn_event_hash: None,
            wallet: Some(packet.source_wallet),
            amount_atoms: Some(packet.amount_atoms),
            block_height,
        },
    )?;
    Ok(())
}

fn apply_pftl_uniswap_destination_consume(
    _genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &PftlUniswapDestinationConsumeOperation,
    block_height: u64,
    allow_disabled_live_value_replay: bool,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let route = ledger.pftl_uniswap_routes[route_index].clone();
    ensure_pftl_uniswap_route_live(&route, allow_disabled_live_value_replay)?;
    ensure_pftl_uniswap_native_asset_policy(
        ledger,
        &route.native_nav_asset_id,
        &operation.operator,
    )?;
    let packet = route
        .export_packets
        .get(&operation.packet_hash)
        .cloned()
        .ok_or_else(|| {
            (
                "unknown_pftl_uniswap_export_packet",
                "destination consume references unknown export packet".to_string(),
            )
        })?;
    if packet.status != PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED {
        return Err((
            "pftl_uniswap_packet_not_consumable",
            "only source-debited export packets can be destination-consumed".to_string(),
        ));
    }
    let required_finalized_height = operation
        .consumed_height
        .checked_add(route.return_finality_blocks)
        .ok_or_else(|| {
            (
                "pftl_uniswap_destination_finality_overflow",
                "destination consume finality height would overflow".to_string(),
            )
        })?;
    if operation.finalized_height < required_finalized_height {
        return Err((
            "pftl_uniswap_destination_below_finality",
            "destination consume event is below configured finality".to_string(),
        ));
    }

    let mut next_route = route;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    let mut consumed = packet.clone();
    consumed.status = PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED.to_string();
    next_route.outstanding_bridge_claims_atoms = next_route
        .outstanding_bridge_claims_atoms
        .checked_sub(packet.amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_claim_underflow",
                "outstanding bridge claims would underflow".to_string(),
            )
        })?;
    next_route.ethereum_spendable_supply_atoms = next_route
        .ethereum_spendable_supply_atoms
        .checked_add(packet.amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_ethereum_supply_overflow",
                "Ethereum spendable route supply would overflow".to_string(),
            )
        })?;
    next_route
        .export_packets
        .insert(operation.packet_hash.clone(), consumed);
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "destination_consume",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: Some(operation.packet_hash.clone()),
            burn_event_hash: None,
            wallet: Some(packet.ethereum_recipient),
            amount_atoms: Some(packet.amount_atoms),
            block_height,
        },
    )?;
    Ok(())
}

fn apply_pftl_uniswap_return_import(
    _genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &PftlUniswapReturnImportOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let route_index = pftl_uniswap_route_index(ledger, &operation.route_id)?;
    let route = ledger.pftl_uniswap_routes[route_index].clone();
    // Pause intentionally does not block return imports: importing a return
    // burn shrinks Ethereum-side exposure and credits NAV back on PFTL.
    ensure_pftl_uniswap_native_asset_policy(
        ledger,
        &route.native_nav_asset_id,
        &operation.operator,
    )?;
    if route
        .return_imports
        .contains_key(&operation.burn_event_hash)
    {
        return Err((
            "duplicate_pftl_uniswap_return_import",
            "return burn event hash already exists".to_string(),
        ));
    }
    if operation.ethereum_chain_id != route.ethereum_chain_id
        || operation.native_nav_asset_id != route.native_nav_asset_id
        || !operation
            .bridge_controller
            .eq_ignore_ascii_case(&route.handoff_controller)
        || !operation
            .wrapped_navcoin_token
            .eq_ignore_ascii_case(&route.wrapped_navcoin_token)
    {
        return Err((
            "pftl_uniswap_return_route_mismatch",
            "return import fields do not match route".to_string(),
        ));
    }
    let expected_burn_event_hash = pftl_uniswap_return_burn_id_from_fields(
        operation.ethereum_chain_id,
        &operation.bridge_controller,
        &operation.wrapped_navcoin_token,
        &operation.native_nav_asset_id,
        &operation.ethereum_sender,
        &operation.pftl_recipient,
        operation.amount_atoms,
        &operation.return_nonce,
        operation.burn_height,
    )
    .map_err(|error| ("bad_pftl_uniswap_return_burn_id", error))?;
    if operation.burn_event_hash != expected_burn_event_hash {
        return Err((
            "pftl_uniswap_return_burn_id_mismatch",
            "return import burn event hash does not match canonical preimage".to_string(),
        ));
    }
    let required_finalized_height = operation
        .burn_height
        .checked_add(route.return_finality_blocks)
        .ok_or_else(|| {
            (
                "pftl_uniswap_return_finality_overflow",
                "return finality height would overflow".to_string(),
            )
        })?;
    if operation.finalized_height < required_finalized_height {
        return Err((
            "pftl_uniswap_return_below_finality",
            "return burn event is below configured finality".to_string(),
        ));
    }
    if operation.amount_atoms > route.ethereum_spendable_supply_atoms {
        return Err((
            "insufficient_pftl_uniswap_ethereum_supply",
            "return import exceeds route Ethereum spendable supply".to_string(),
        ));
    }
    credit_issued_asset_balance(
        ledger,
        &operation.pftl_recipient,
        &route.native_nav_asset_id,
        operation.amount_atoms,
        "PFTL-Uniswap return import NAV credit",
    )?;

    let mut next_route = route;
    let state_before_hash = pftl_uniswap_route_state_hash(&next_route);
    let imported = PftlUniswapConsensusReturnImport {
        burn_event_hash: operation.burn_event_hash.clone(),
        ethereum_chain_id: operation.ethereum_chain_id,
        bridge_controller: operation.bridge_controller.clone(),
        wrapped_navcoin_token: operation.wrapped_navcoin_token.clone(),
        native_nav_asset_id: operation.native_nav_asset_id.clone(),
        ethereum_sender: operation.ethereum_sender.clone(),
        pftl_recipient: operation.pftl_recipient.clone(),
        amount_atoms: operation.amount_atoms,
        return_nonce: operation.return_nonce.clone(),
        burn_height: operation.burn_height,
        finalized_height: operation.finalized_height,
        status: PFTL_UNISWAP_RETURN_STATUS_IMPORTED.to_string(),
    };
    imported
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_return_import", error))?;
    next_route.ethereum_spendable_supply_atoms = next_route
        .ethereum_spendable_supply_atoms
        .checked_sub(operation.amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_ethereum_supply_underflow",
                "Ethereum spendable route supply would underflow".to_string(),
            )
        })?;
    next_route.pftl_spendable_supply_atoms = next_route
        .pftl_spendable_supply_atoms
        .checked_add(operation.amount_atoms)
        .ok_or_else(|| {
            (
                "pftl_uniswap_supply_overflow",
                "PFTL spendable route supply would overflow".to_string(),
            )
        })?;
    pftl_uniswap_credit_native_route_balance(
        &mut next_route,
        &operation.pftl_recipient,
        operation.amount_atoms,
    )?;
    next_route
        .return_imports
        .insert(operation.burn_event_hash.clone(), imported);
    next_route
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_route", error))?;
    let state_after_hash = pftl_uniswap_route_state_hash(&next_route);
    ledger.pftl_uniswap_routes[route_index] = next_route;
    append_pftl_uniswap_consensus_receipt(
        ledger,
        PftlUniswapReceiptPlan {
            transition: "return_imported",
            route_id: &operation.route_id,
            state_before_hash,
            state_after_hash,
            packet_hash: None,
            burn_event_hash: Some(operation.burn_event_hash.clone()),
            wallet: Some(operation.pftl_recipient.clone()),
            amount_atoms: Some(operation.amount_atoms),
            block_height,
        },
    )?;
    Ok(())
}

struct PftlUniswapReceiptPlan<'a> {
    transition: &'static str,
    route_id: &'a str,
    state_before_hash: String,
    state_after_hash: String,
    packet_hash: Option<String>,
    burn_event_hash: Option<String>,
    wallet: Option<String>,
    amount_atoms: Option<u64>,
    block_height: u64,
}

fn append_pftl_uniswap_consensus_receipt(
    ledger: &mut LedgerState,
    plan: PftlUniswapReceiptPlan<'_>,
) -> Result<(), (&'static str, String)> {
    let receipt_hash = pftl_uniswap_consensus_receipt_hash(&plan);
    if ledger
        .pftl_uniswap_receipts
        .iter()
        .any(|receipt| receipt.receipt_hash == receipt_hash)
    {
        return Err((
            "duplicate_pftl_uniswap_receipt",
            "PFTL-Uniswap consensus receipt already exists".to_string(),
        ));
    }
    let receipt = PftlUniswapConsensusReceipt {
        receipt_hash,
        transition: plan.transition.to_string(),
        route_id: plan.route_id.to_string(),
        state_before_hash: plan.state_before_hash,
        state_after_hash: plan.state_after_hash,
        packet_hash: plan.packet_hash,
        burn_event_hash: plan.burn_event_hash,
        wallet: plan.wallet,
        amount_atoms: plan.amount_atoms,
        block_height: plan.block_height,
    };
    receipt
        .validate()
        .map_err(|error| ("bad_pftl_uniswap_receipt", error))?;
    ledger.pftl_uniswap_receipts.push(receipt);
    Ok(())
}

fn pftl_uniswap_route_index(
    ledger: &LedgerState,
    route_id: &str,
) -> Result<usize, (&'static str, String)> {
    ledger
        .pftl_uniswap_routes
        .iter()
        .position(|route| route.route_id == route_id)
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_route",
                format!("PFTL-Uniswap route `{route_id}` is missing"),
            )
        })
}

fn ensure_pftl_uniswap_route_live(
    route: &PftlUniswapConsensusRouteState,
    allow_disabled_live_value_replay: bool,
) -> Result<(), (&'static str, String)> {
    if route.paused || (!route.live_value_enabled && !allow_disabled_live_value_replay) {
        return Err((
            "pftl_uniswap_route_paused",
            "PFTL-Uniswap route is paused or live value is disabled".to_string(),
        ));
    }
    Ok(())
}

fn ensure_pftl_uniswap_native_asset_policy(
    ledger: &LedgerState,
    native_nav_asset_id: &str,
    operator: &str,
) -> Result<NavTrackedAsset, (&'static str, String)> {
    let nav_asset = ledger.nav_asset(native_nav_asset_id).cloned().ok_or_else(|| {
        (
            "missing_pftl_uniswap_nav_asset",
            format!(
                "PFTL-Uniswap route native asset `{native_nav_asset_id}` is not registered as a NAV asset"
            ),
        )
    })?;
    if operator != nav_asset.issuer && operator != nav_asset.reserve_operator {
        return Err((
            "unauthorized_pftl_uniswap_operator",
            "PFTL-Uniswap route operator must be the native NAV asset issuer or reserve operator"
                .to_string(),
        ));
    }
    let asset = ledger
        .asset_definition(&nav_asset.asset_id)
        .ok_or_else(|| {
            (
                "missing_native_nav_asset",
                format!(
                    "PFTL-Uniswap route native asset `{}` is missing",
                    nav_asset.asset_id
                ),
            )
        })?;
    if asset.issuer != nav_asset.issuer {
        return Err((
            "asset_issuer_mismatch",
            "PFTL-Uniswap native NAV asset issuer does not match issued asset issuer".to_string(),
        ));
    }
    Ok(nav_asset)
}

fn ensure_pftl_uniswap_route_capacity(
    ledger: &LedgerState,
    native_nav_issuer: &str,
) -> Result<(), (&'static str, String)> {
    let route_count = ledger
        .pftl_uniswap_routes
        .iter()
        .filter(|route| {
            ledger
                .nav_asset(&route.native_nav_asset_id)
                .map_or(false, |nav_asset| nav_asset.issuer == native_nav_issuer)
        })
        .count();
    if route_count >= MAX_PFTL_UNISWAP_ROUTES_PER_NATIVE_ISSUER {
        return Err((
            "pftl_uniswap_route_cap_exceeded",
            "PFTL-Uniswap route count for the native NAV issuer exceeds the bounded consensus limit"
                .to_string(),
        ));
    }
    Ok(())
}

fn pftl_uniswap_pricing_nav_asset(
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
    block_height: u64,
) -> Result<NavTrackedAsset, (&'static str, String)> {
    let nav_asset = ledger
        .nav_asset(&route.native_nav_asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_nav_asset",
                format!(
                    "PFTL-Uniswap route native asset `{}` is not registered as a NAV asset",
                    route.native_nav_asset_id
                ),
            )
        })?;
    if nav_asset.halted {
        return Err((
            "pftl_uniswap_nav_asset_halted",
            format!(
                "PFTL-Uniswap native NAV asset is halted: {}",
                nav_asset.halt_reason
            ),
        ));
    }
    if nav_asset.finalized_epoch == 0
        || nav_asset.nav_per_unit == 0
        || nav_asset.finalized_reserve_packet_hash.is_empty()
    {
        return Err((
            "pftl_uniswap_nav_not_finalized",
            "PFTL-Uniswap primary subscription requires a finalized NAV reserve packet".to_string(),
        ));
    }
    if nav_asset.finalized_at_height == 0 {
        return Err((
            "pftl_uniswap_pricing_height_missing",
            "PFTL-Uniswap primary subscription requires height-aware finalized NAV pricing"
                .to_string(),
        ));
    }
    let expires_at_height = nav_asset
        .finalized_at_height
        .checked_add(MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS)
        .ok_or_else(|| {
            (
                "pftl_uniswap_pricing_height_overflow",
                "PFTL-Uniswap pricing freshness height would overflow".to_string(),
            )
        })?;
    if block_height > expires_at_height {
        return Err((
            "stale_pftl_uniswap_pricing",
            "PFTL-Uniswap finalized NAV pricing is older than the consensus freshness window"
                .to_string(),
        ));
    }
    Ok(nav_asset)
}

fn pftl_uniswap_price_settlement_atoms_per_nav_atom(
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
    native_nav_asset: &NavTrackedAsset,
) -> Result<u64, (&'static str, String)> {
    let native_asset = ledger
        .asset_definition(&route.native_nav_asset_id)
        .ok_or_else(|| {
            (
                "missing_native_nav_asset",
                format!(
                    "PFTL-Uniswap route native asset `{}` is missing",
                    route.native_nav_asset_id
                ),
            )
        })?;
    let settlement_asset = ledger
        .asset_definition(&route.settlement_asset_id)
        .ok_or_else(|| {
            (
                "missing_settlement_asset",
                format!(
                    "PFTL-Uniswap route settlement asset `{}` is missing",
                    route.settlement_asset_id
                ),
            )
        })?;
    let settlement_nav_asset = ledger
        .nav_asset(&route.settlement_asset_id)
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_settlement_nav_asset",
                "PFTL-Uniswap primary subscription settlement asset must be NAV-registered for valuation-unit binding"
                    .to_string(),
            )
        })?;
    let price = required_vault_bridge_settlement_atoms(
        1,
        native_asset.precision,
        native_nav_asset.nav_per_unit,
        &native_nav_asset.valuation_unit,
        &settlement_nav_asset.valuation_unit,
        settlement_asset.precision,
    )?;
    if price == 0 {
        return Err((
            "pftl_uniswap_price_zero",
            "PFTL-Uniswap ledger-derived NAV atom price must be nonzero".to_string(),
        ));
    }
    Ok(price)
}

fn pftl_uniswap_credit_native_route_balance(
    route: &mut PftlUniswapConsensusRouteState,
    wallet: &str,
    amount_atoms: u64,
) -> Result<(), (&'static str, String)> {
    let current = route
        .native_spendable_balances_atoms
        .get(wallet)
        .copied()
        .unwrap_or(0);
    let next = current.checked_add(amount_atoms).ok_or_else(|| {
        (
            "pftl_uniswap_wallet_balance_overflow",
            "route native wallet balance would overflow".to_string(),
        )
    })?;
    route
        .native_spendable_balances_atoms
        .insert(wallet.to_string(), next);
    Ok(())
}

fn pftl_uniswap_debit_native_route_balance(
    route: &mut PftlUniswapConsensusRouteState,
    wallet: &str,
    amount_atoms: u64,
) -> Result<(), (&'static str, String)> {
    let current = route
        .native_spendable_balances_atoms
        .get(wallet)
        .copied()
        .unwrap_or(0);
    if current < amount_atoms {
        return Err((
            "insufficient_pftl_uniswap_wallet_balance",
            "route native wallet balance is too low".to_string(),
        ));
    }
    let next = current - amount_atoms;
    if next == 0 {
        route.native_spendable_balances_atoms.remove(wallet);
    } else {
        route
            .native_spendable_balances_atoms
            .insert(wallet.to_string(), next);
    }
    Ok(())
}

fn credit_issued_asset_balance(
    ledger: &mut LedgerState,
    account: &str,
    asset_id: &str,
    amount_atoms: u64,
    context: &str,
) -> Result<(), (&'static str, String)> {
    ensure_not_vault_bridge_out_of_lane_mint(ledger, asset_id, context)?;
    credit_issued_asset_balance_from_custody(ledger, account, asset_id, amount_atoms, context)
}

/// Releases issued assets previously removed from spendable supply into
/// route/reservation custody. Callers must prove and atomically decrement the
/// matching custody balance; this is not an issuance lane.
fn credit_issued_asset_balance_from_custody(
    ledger: &mut LedgerState,
    account: &str,
    asset_id: &str,
    amount_atoms: u64,
    context: &str,
) -> Result<(), (&'static str, String)> {
    let asset = ledger.asset_definition(asset_id).cloned().ok_or_else(|| {
        (
            "missing_asset",
            format!("{context}: asset `{asset_id}` is missing"),
        )
    })?;
    let to_index =
        issued_asset_credit_recipient_line_index(ledger, &asset, account, amount_atoms, context)?;
    let (recipient_after, required_limit) =
        prepare_issued_asset_credit(ledger, &asset, account, to_index, amount_atoms, context)?;
    let supply_after = issued_asset_supply(ledger, asset_id)?
        .checked_add(amount_atoms)
        .ok_or_else(|| {
            (
                "issued_supply_overflow",
                format!("{context}: issued supply would overflow"),
            )
        })?;
    if let Some(max_supply) = asset.max_supply {
        if supply_after > max_supply {
            return Err((
                "issued_supply_cap_exceeded",
                format!("{context}: issued supply cap would be exceeded"),
            ));
        }
    }
    apply_prepared_issued_asset_credit(ledger, to_index, recipient_after, required_limit);
    Ok(())
}

fn debit_issued_asset_balance(
    ledger: &mut LedgerState,
    account: &str,
    asset_id: &str,
    amount_atoms: u64,
    context: &str,
) -> Result<(), (&'static str, String)> {
    let asset = ledger.asset_definition(asset_id).cloned().ok_or_else(|| {
        (
            "missing_asset",
            format!("{context}: asset `{asset_id}` is missing"),
        )
    })?;
    let index = trustline_index(ledger, account, asset_id).ok_or_else(|| {
        (
            "missing_trustline",
            format!("{context}: account has no issued-asset balance line"),
        )
    })?;
    ensure_line_can_move(&asset, &ledger.trustlines[index])?;
    if ledger.trustlines[index].balance < amount_atoms {
        return Err((
            "insufficient_issued_balance",
            format!("{context}: amount exceeds issued-asset balance"),
        ));
    }
    ledger.trustlines[index].balance -= amount_atoms;
    Ok(())
}

fn ensure_not_vault_bridge_out_of_lane_mint(
    ledger: &LedgerState,
    asset_id: &str,
    context: &str,
) -> Result<(), (&'static str, String)> {
    if is_vault_bridge_profiled_asset(ledger, asset_id) {
        return Err((
            "vault_bridge_out_of_lane_issuance",
            format!(
                "{context}: vault-bridge profiled assets may only increase supply through vault-bridge credit lanes"
            ),
        ));
    }
    Ok(())
}

fn is_vault_bridge_profiled_asset(ledger: &LedgerState, asset_id: &str) -> bool {
    let Some(nav_asset) = ledger.nav_asset(asset_id) else {
        return false;
    };
    let Some(profile) = nav_profile_for_asset(ledger, nav_asset) else {
        return false;
    };
    profile
        .source_class
        .starts_with(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
}

fn pftl_uniswap_absent_route_hash(route_id: &str) -> String {
    hash_hex(
        "postfiat.pftl_uniswap.consensus_route_absent.v1",
        format!("route_id={route_id}\n").as_bytes(),
    )
}

#[allow(dead_code)]
fn pftl_uniswap_route_state_hash_legacy_reference(
    route: &PftlUniswapConsensusRouteState,
) -> String {
    let mut preimage = format!(
        "route_id={}\nroute_family={}\nroute_config_digest={}\nroute_trust_class={}\nnative_nav_asset_id={}\nsettlement_asset_id={}\nhandoff_controller={}\nsettlement_adapter={}\nwrapped_navcoin_token={}\nethereum_chain_id={}\nroute_supply_cap_atoms={}\npacket_notional_cap_atoms={}\nlatest_finalized_nav_epoch={}\nreturn_finality_blocks={}\n",
        route.route_id,
        route.route_family,
        route.route_config_digest,
        route.route_trust_class,
        route.native_nav_asset_id,
        route.settlement_asset_id,
        route.handoff_controller,
        route.settlement_adapter,
        route.wrapped_navcoin_token,
        route.ethereum_chain_id,
        route.route_supply_cap_atoms,
        route.packet_notional_cap_atoms,
        route.latest_finalized_nav_epoch,
        route.return_finality_blocks,
    );
    if route.live_value_enabled {
        preimage.push_str("live_value_enabled=true\n");
    }
    preimage.push_str(&format!(
        "authorized_valid_supply_atoms={}\npftl_spendable_supply_atoms={}\nethereum_spendable_supply_atoms={}\nother_registered_venue_supply_atoms={}\noutstanding_bridge_claims_atoms={}\npending_return_import_claims_atoms={}\nsettlement_reserve_atoms={}\npaused={}\n",
        route.authorized_valid_supply_atoms,
        route.pftl_spendable_supply_atoms,
        route.ethereum_spendable_supply_atoms,
        route.other_registered_venue_supply_atoms,
        route.outstanding_bridge_claims_atoms,
        route.pending_return_import_claims_atoms,
        route.settlement_reserve_atoms,
        route.paused,
    ));
    if let Some(policy) = &route.ethereum_verification_policy {
        preimage.push_str(&format!(
            "ethereum_policy.authority_epoch={}\nethereum_policy.committee_root={}\nethereum_policy.minimum_confirmations={}\nethereum_policy.handoff_controller_code_hash={}\nethereum_policy.wrapped_navcoin_code_hash={}\n",
            policy.authority_epoch,
            bytes_to_hex(&policy.committee_root.0),
            policy.minimum_confirmations,
            bytes_to_hex(&policy.handoff_controller_code_hash),
            bytes_to_hex(&policy.wrapped_navcoin_code_hash),
        ));
    }
    if let Some(v2) = &route.v2 {
        let policy = &v2.primary_market_policy;
        preimage.push_str(&format!(
            "v2.route_schema_version={}\nv2.route_epoch={}\nv2.outbound_verification_class={}\nv2.return_verification_class={}\nv2.policy_hash={}\nv2.policy_epoch={}\nv2.issue_multiplier_bps={}\nv2.redeem_multiplier_bps={}\nv2.issue_capacity_atoms={}\nv2.redeem_capacity_atoms={}\nv2.max_order_atoms={}\nv2.min_order_atoms={}\nv2.valid_from_height={}\nv2.expires_at_height={}\nv2.max_nav_age_blocks={}\nv2.pricing_nav_epoch={}\nv2.pricing_reserve_packet_hash={}\nv2.issue_capacity_used_atoms={}\nv2.redeem_capacity_used_atoms={}\nv2.non_nav_spread_atoms={}\n",
            v2.route_schema_version,
            v2.route_epoch,
            v2.outbound_verification_class,
            v2.return_verification_class,
            policy.policy_hash,
            policy.policy_epoch,
            policy.issue_multiplier_bps,
            policy.redeem_multiplier_bps,
            policy.issue_capacity_atoms,
            policy.redeem_capacity_atoms,
            policy.max_order_atoms,
            policy.min_order_atoms,
            policy.valid_from_height,
            policy.expires_at_height,
            policy.max_nav_age_blocks,
            policy.pricing_nav_epoch,
            policy.pricing_reserve_packet_hash,
            v2.issue_capacity_used_atoms,
            v2.redeem_capacity_used_atoms,
            v2.non_nav_spread_atoms,
        ));
        for (reservation_id, reservation) in &v2.active_reservations {
            preimage.push_str(&format!(
                "v2.reservation.{reservation_id}.subscriber={}\nv2.reservation.{reservation_id}.ethereum_recipient={}\nv2.reservation.{reservation_id}.route_epoch={}\nv2.reservation.{reservation_id}.policy_epoch={}\nv2.reservation.{reservation_id}.policy_hash={}\nv2.reservation.{reservation_id}.mint_amount_atoms={}\nv2.reservation.{reservation_id}.max_settlement_value_atoms={}\nv2.reservation.{reservation_id}.created_at_height={}\nv2.reservation.{reservation_id}.expires_at_height={}\nv2.reservation.{reservation_id}.status={}\n",
                reservation.subscriber,
                reservation.ethereum_recipient,
                reservation.route_epoch,
                reservation.policy_epoch,
                reservation.policy_hash,
                reservation.mint_amount_atoms,
                reservation.max_settlement_value_atoms,
                reservation.created_at_height,
                reservation.expires_at_height,
                reservation.status,
            ));
        }
        for (reservation_id, status) in &v2.terminal_reservations {
            preimage.push_str(&format!(
                "v2.terminal_reservation.{reservation_id}={status}\n"
            ));
        }
        for (nonce, owner) in &v2.redemption_nonces {
            preimage.push_str(&format!("v2.redemption_nonce.{nonce}={owner}\n"));
        }
    }
    for (wallet, amount) in &route.native_spendable_balances_atoms {
        preimage.push_str(&format!("native_balance.{wallet}={amount}\n"));
    }
    for (nonce, wallet) in &route.primary_subscription_nonces {
        preimage.push_str(&format!("primary_nonce.{nonce}={wallet}\n"));
    }
    for (packet_hash, packet) in &route.export_packets {
        preimage.push_str(&format!(
            "export_packet.{packet_hash}.nonce={}\nexport_packet.{packet_hash}.source_wallet={}\nexport_packet.{packet_hash}.ethereum_recipient={}\nexport_packet.{packet_hash}.amount_atoms={}\nexport_packet.{packet_hash}.source_height={}\nexport_packet.{packet_hash}.destination_deadline_seconds={}\nexport_packet.{packet_hash}.refund_not_before_height={}\nexport_packet.{packet_hash}.status={}\n",
            packet.nonce,
            packet.source_wallet,
            packet.ethereum_recipient,
            packet.amount_atoms,
            packet.source_height,
            packet.destination_deadline_seconds,
            packet.refund_not_before_height,
            packet.status,
        ));
        if let Some(settlement_value_atoms) = packet.settlement_value_atoms {
            preimage.push_str(&format!(
                "export_packet.{packet_hash}.settlement_value_atoms={settlement_value_atoms}\n"
            ));
        }
        if let Some(packet_digest) = &packet.ethereum_packet_digest {
            preimage.push_str(&format!(
                "export_packet.{packet_hash}.ethereum_packet_digest={packet_digest}\n"
            ));
        }
        if let Some(schema_version) = packet.ethereum_packet_schema_version {
            preimage.push_str(&format!(
                "export_packet.{packet_hash}.ethereum_packet_schema_version={schema_version}\n"
            ));
        }
        if let Some(route_epoch) = packet.route_epoch {
            preimage.push_str(&format!(
                "export_packet.{packet_hash}.route_epoch={route_epoch}\n"
            ));
        }
        if let Some(policy_hash) = &packet.policy_hash {
            preimage.push_str(&format!(
                "export_packet.{packet_hash}.policy_hash={policy_hash}\n"
            ));
        }
    }
    for (nonce, packet_hash) in &route.export_nonces {
        preimage.push_str(&format!("export_nonce.{nonce}={packet_hash}\n"));
    }
    for (burn_hash, burn) in &route.return_imports {
        preimage.push_str(&format!(
            "return_import.{burn_hash}.ethereum_chain_id={}\nreturn_import.{burn_hash}.bridge_controller={}\nreturn_import.{burn_hash}.wrapped_navcoin_token={}\nreturn_import.{burn_hash}.native_nav_asset_id={}\nreturn_import.{burn_hash}.ethereum_sender={}\nreturn_import.{burn_hash}.pftl_recipient={}\nreturn_import.{burn_hash}.amount_atoms={}\nreturn_import.{burn_hash}.return_nonce={}\nreturn_import.{burn_hash}.burn_height={}\nreturn_import.{burn_hash}.finalized_height={}\nreturn_import.{burn_hash}.status={}\n",
            burn.ethereum_chain_id,
            burn.bridge_controller,
            burn.wrapped_navcoin_token,
            burn.native_nav_asset_id,
            burn.ethereum_sender,
            burn.pftl_recipient,
            burn.amount_atoms,
            burn.return_nonce,
            burn.burn_height,
            burn.finalized_height,
            burn.status,
        ));
    }
    let domain = if route.v2.is_some() {
        "postfiat.pftl_uniswap.consensus_route_state.v2"
    } else {
        "postfiat.pftl_uniswap.consensus_route_state.v1"
    };
    hash_hex(domain, preimage.as_bytes())
}

fn pftl_uniswap_consensus_receipt_hash(plan: &PftlUniswapReceiptPlan<'_>) -> String {
    let preimage = format!(
        "transition={}\nroute_id={}\nstate_before_hash={}\nstate_after_hash={}\npacket_hash={}\nburn_event_hash={}\nwallet={}\namount_atoms={}\nblock_height={}\n",
        plan.transition,
        plan.route_id,
        plan.state_before_hash,
        plan.state_after_hash,
        plan.packet_hash.as_deref().unwrap_or(""),
        plan.burn_event_hash.as_deref().unwrap_or(""),
        plan.wallet.as_deref().unwrap_or(""),
        plan.amount_atoms.map_or_else(String::new, |amount| amount.to_string()),
        plan.block_height,
    );
    hash_hex(
        "postfiat.pftl_uniswap.consensus_receipt.v1",
        preimage.as_bytes(),
    )
}

fn apply_vault_bridge_burn_to_redeem(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    operation: &VaultBridgeBurnToRedeemOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!(
                    "vault bridge asset asset `{}` is not registered as a NAV asset",
                    operation.asset_id
                ),
            )
        })?;
    if nav_asset.issuer != operation.issuer {
        return Err((
            "vault_bridge_issuer_mismatch",
            "vault bridge asset redemption issuer does not match the NAV asset issuer".to_string(),
        ));
    }
    ensure_vault_bridge_asset_policy(ledger, &nav_asset, &operation.issuer)?;
    ensure_nav_asset_live_for_epoch(
        ledger,
        &nav_asset,
        operation.epoch,
        &operation.reserve_packet_hash,
        block_height,
    )?;

    let family_asset = ledger
        .asset_definition(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", operation.asset_id),
            )
        })?;
    let bucket_index = ledger
        .vault_bridge_bucket_states
        .iter()
        .position(|bucket| bucket.bucket_id == operation.bucket_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_bucket",
                "vault bridge asset burn-to-redeem references missing source bucket".to_string(),
            )
        })?;
    let bucket = ledger.vault_bridge_bucket_states[bucket_index].clone();
    if bucket.asset_id != operation.asset_id {
        return Err((
            "vault_bridge_bucket_asset_mismatch",
            "vault bridge asset burn-to-redeem bucket does not belong to operation asset"
                .to_string(),
        ));
    }
    if bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE
        && bucket.status != VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED
    {
        return Err((
            "vault_bridge_bucket_not_active",
            "vault bridge asset burn-to-redeem requires an active or impaired source bucket"
                .to_string(),
        ));
    }
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &bucket.source_domain,
        &bucket.policy_hash,
    )?;
    ensure_vault_bridge_source_policy(profile, &bucket.source_domain, &bucket.policy_hash)?;
    let movement_asset = if compatibility.pfusdc_source_series_active(block_height) {
        let mut matches = ledger.asset_definitions.iter().filter(|asset| {
            asset.asset_family_id == operation.asset_id
                && asset.source_bucket_id == operation.bucket_id
                && asset.asset_id == asset.source_series_id
        });
        let asset = matches.next().cloned().ok_or_else(|| {
            (
                "pfusdc_source_series_asset_missing",
                "source-specific redemption requires the source-series asset for its bucket"
                    .to_string(),
            )
        })?;
        if matches.next().is_some() {
            return Err((
                "pfusdc_source_series_asset_duplicate",
                "multiple source-series assets reference the same backing bucket".to_string(),
            ));
        }
        asset
    } else {
        family_asset
    };
    let owner_index = trustline_index(ledger, &operation.owner, &movement_asset.asset_id)
        .ok_or_else(|| {
            (
                "missing_trustline",
                "vault bridge redemption owner has no balance in the selected source series"
                    .to_string(),
            )
        })?;
    ensure_line_can_move(&movement_asset, &ledger.trustlines[owner_index])?;
    if ledger.trustlines[owner_index].balance < operation.amount_atoms {
        return Err((
            "insufficient_issued_balance",
            "vault bridge burn-to-redeem exceeds the selected source-series balance".to_string(),
        ));
    }
    if bucket.outstanding_vault_bridge_atoms < operation.amount_atoms {
        return Err((
            "vault_bridge_bucket_underflow",
            "vault bridge asset burn-to-redeem exceeds bucket outstanding supply".to_string(),
        ));
    }

    let mut redemption = VaultBridgeRedemption::new(
        &genesis.chain_id,
        operation.owner.clone(),
        operation.issuer.clone(),
        operation.asset_id.clone(),
        operation.bucket_id.clone(),
        bucket.source_domain.clone(),
        transaction.unsigned.sequence,
        operation.amount_atoms,
        operation.epoch,
        operation.reserve_packet_hash.clone(),
        operation.destination_ref.clone(),
        asset_transaction_tx_id(transaction),
        block_height,
    )
    .map_err(|error| ("bad_vault_bridge_redemption", error))?;
    if compatibility.emit_legacy_domainless_withdrawal_packet {
        redemption.withdrawal_packet.source_chain_id = 0;
        redemption.withdrawal_packet.vault_address.clear();
        redemption.withdrawal_packet.token_address.clear();
        redemption.withdrawal_packet_hash =
            vault_bridge_withdrawal_packet_legacy_domainless_hash(&redemption.withdrawal_packet)
                .map_err(|error| ("bad_vault_bridge_redemption", error))?;
        redemption.withdrawal_packet_evm_digest =
            vault_bridge_withdrawal_packet_legacy_domainless_evm_digest(
                &redemption.withdrawal_packet,
            )
            .map_err(|error| ("bad_vault_bridge_redemption", error))?;
        redemption
            .validate_for_chain(&genesis.chain_id)
            .map_err(|error| ("bad_vault_bridge_redemption", error))?;
    }
    if ledger
        .vault_bridge_redemptions
        .iter()
        .any(|existing| existing.redemption_id == redemption.redemption_id)
    {
        return Err((
            "duplicate_vault_bridge_redemption",
            "vault bridge asset redemption already exists".to_string(),
        ));
    }

    let outstanding_after = bucket
        .outstanding_vault_bridge_atoms
        .checked_sub(operation.amount_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_bucket_underflow",
                "vault bridge asset bucket outstanding supply would underflow".to_string(),
            )
        })?;
    let redemption_queue_after = bucket
        .redemption_queue_atoms
        .checked_add(operation.amount_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_bucket_overflow",
                "vault bridge asset redemption queue would overflow".to_string(),
            )
        })?;
    let mut bucket_after = bucket;
    bucket_after.outstanding_vault_bridge_atoms = outstanding_after;
    bucket_after.redemption_queue_atoms = redemption_queue_after;
    bucket_after.last_updated_height = block_height;
    bucket_after
        .validate()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;

    ledger.trustlines[owner_index].balance -= operation.amount_atoms;
    ledger.vault_bridge_bucket_states[bucket_index] = bucket_after;
    ledger.vault_bridge_redemptions.push(redemption);
    Ok(())
}

#[allow(dead_code)]
fn apply_vault_bridge_redeem_settle(
    ledger: &mut LedgerState,
    operation: &VaultBridgeRedeemSettleOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    apply_vault_bridge_redeem_settle_with_compatibility(
        ledger,
        operation,
        block_height,
        AssetExecutionCompatibility::strict(),
    )
}

fn apply_vault_bridge_redeem_settle_with_compatibility(
    ledger: &mut LedgerState,
    operation: &VaultBridgeRedeemSettleOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!(
                    "vault bridge asset asset `{}` is not registered as a NAV asset",
                    operation.asset_id
                ),
            )
        })?;
    if nav_asset.issuer != operation.issuer_or_redemption_account
        && nav_asset.redemption_account != operation.issuer_or_redemption_account
    {
        return Err((
            "vault_bridge_settle_issuer_mismatch",
            "vault bridge asset redemption settlement must be signed by issuer or redemption account"
                .to_string(),
        ));
    }
    let redemption_index = ledger
        .vault_bridge_redemptions
        .iter()
        .position(|redemption| redemption.redemption_id == operation.redemption_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_redemption",
                "vault bridge asset redeem settlement references missing redemption".to_string(),
            )
        })?;
    let redemption = ledger.vault_bridge_redemptions[redemption_index].clone();
    if redemption.asset_id != operation.asset_id {
        return Err((
            "vault_bridge_redemption_asset_mismatch",
            "vault bridge asset redeem settlement redemption does not belong to asset".to_string(),
        ));
    }
    if redemption.state != VAULT_BRIDGE_REDEMPTION_STATE_PENDING {
        return Err((
            "vault_bridge_redemption_not_pending",
            "vault bridge asset redeem settlement requires a pending redemption".to_string(),
        ));
    }
    if redemption.settled_atoms > redemption.amount_atoms {
        return Err((
            "bad_vault_bridge_redemption",
            "vault bridge asset redemption settled amount exceeds requested amount".to_string(),
        ));
    }
    let remaining = redemption.amount_atoms - redemption.settled_atoms;
    if operation.settled_atoms > remaining {
        return Err((
            "vault_bridge_settle_amount_exceeds_remaining",
            "vault bridge asset redeem settlement exceeds remaining redemption amount".to_string(),
        ));
    }
    let bucket_index = ledger
        .vault_bridge_bucket_states
        .iter()
        .position(|bucket| bucket.bucket_id == redemption.bucket_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_bucket",
                "vault bridge asset redemption references missing source bucket".to_string(),
            )
        })?;
    if ledger.vault_bridge_bucket_states[bucket_index].asset_id != operation.asset_id {
        return Err((
            "vault_bridge_bucket_asset_mismatch",
            "vault bridge asset redemption bucket does not belong to operation asset".to_string(),
        ));
    }
    let bucket = &ledger.vault_bridge_bucket_states[bucket_index];
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &bucket.source_domain,
        &bucket.policy_hash,
    )?;
    ensure_vault_bridge_withdrawal_observation_quorum(
        ledger,
        profile,
        &redemption,
        operation,
        block_height,
        compatibility,
    )?;
    if ledger.vault_bridge_bucket_states[bucket_index].redemption_queue_atoms
        < operation.settled_atoms
    {
        return Err((
            "vault_bridge_bucket_underflow",
            "vault bridge asset settlement exceeds bucket redemption queue".to_string(),
        ));
    }

    if compatibility.pfusdc_source_series_active(block_height) {
        release_vault_bridge_supply_allocations(
            ledger,
            &operation.asset_id,
            &redemption.bucket_id,
            operation.settled_atoms,
            block_height,
        )?;
    }

    let counted_value_after = ledger.vault_bridge_bucket_states[bucket_index]
        .counted_value_atoms
        .checked_sub(operation.settled_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_bucket_underflow",
                "vault bridge asset settlement exceeds bucket counted value".to_string(),
            )
        })?;

    ledger.vault_bridge_bucket_states[bucket_index].redemption_queue_atoms -=
        operation.settled_atoms;
    ledger.vault_bridge_bucket_states[bucket_index].counted_value_atoms = counted_value_after;
    ledger.vault_bridge_bucket_states[bucket_index].last_updated_height = block_height;
    let claim_atoms_after = ledger.vault_bridge_bucket_states[bucket_index]
        .allocated_atoms()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;
    ledger.vault_bridge_bucket_states[bucket_index].impairment_factor_bps =
        if claim_atoms_after == 0 {
            10_000
        } else {
            vault_bridge_policy::bucket_factor_bps(
                ledger.vault_bridge_bucket_states[bucket_index].counted_value_atoms,
                claim_atoms_after,
            )
            .map_err(|error| (error.code(), error.message().to_string()))?
        };
    ledger.vault_bridge_bucket_states[bucket_index]
        .validate()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;

    let redemption = &mut ledger.vault_bridge_redemptions[redemption_index];
    redemption.settled_atoms = redemption
        .settled_atoms
        .checked_add(operation.settled_atoms)
        .ok_or_else(|| {
            (
                "vault_bridge_redemption_overflow",
                "vault bridge asset redemption settled amount would overflow".to_string(),
            )
        })?;
    redemption.settlement_receipt_hash = operation.settlement_receipt_hash.clone();
    for attestation in &operation.withdrawal_observations {
        if !redemption.withdrawal_observations.iter().any(|stored| {
            stored.attestor == attestation.attestor
                && stored.observation_root == attestation.observation_root
        }) {
            redemption.withdrawal_observations.push(attestation.clone());
        }
    }
    if redemption.settled_atoms == redemption.amount_atoms {
        redemption.state = VAULT_BRIDGE_REDEMPTION_STATE_SETTLED.to_string();
    }
    redemption
        .validate()
        .map_err(|error| ("bad_vault_bridge_redemption", error))?;
    Ok(())
}

fn apply_vault_bridge_bucket_impair(
    ledger: &mut LedgerState,
    operation: &VaultBridgeBucketImpairOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!(
                    "vault bridge asset asset `{}` is not registered as a NAV asset",
                    operation.asset_id
                ),
            )
        })?;
    ensure_vault_bridge_asset_policy(ledger, &nav_asset, &operation.operator)?;
    let bucket_index = ledger
        .vault_bridge_bucket_states
        .iter()
        .position(|bucket| bucket.bucket_id == operation.bucket_id)
        .ok_or_else(|| {
            (
                "missing_vault_bridge_bucket",
                "vault bridge asset impairment references missing source bucket".to_string(),
            )
        })?;
    let bucket = ledger.vault_bridge_bucket_states[bucket_index].clone();
    if bucket.asset_id != operation.asset_id {
        return Err((
            "vault_bridge_bucket_asset_mismatch",
            "vault bridge asset impairment bucket does not belong to operation asset".to_string(),
        ));
    }
    let profile = vault_bridge_profile_for_pinned_policy(
        ledger,
        &nav_asset,
        &bucket.source_domain,
        &bucket.policy_hash,
    )?;
    ensure_vault_bridge_source_policy(profile, &bucket.source_domain, &operation.policy_hash)?;
    if bucket.policy_hash != operation.policy_hash {
        return Err((
            "vault_bridge_policy_hash_mismatch",
            "vault bridge asset impairment policy_hash does not match bucket policy".to_string(),
        ));
    }
    if bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE
        && bucket.status != VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED
        && bucket.status != VAULT_BRIDGE_BUCKET_STATUS_PAUSED
    {
        return Err((
            "vault_bridge_bucket_not_impairable",
            "vault bridge asset impairment requires an active, impaired, or paused bucket"
                .to_string(),
        ));
    }
    if operation.updated_counted_value_atoms > bucket.counted_value_atoms {
        return Err((
            "vault_bridge_impairment_increases_counted_value",
            "vault bridge asset impairment cannot increase counted value without a counted recapitalization receipt"
                .to_string(),
        ));
    }

    let claim_atoms = bucket
        .allocated_atoms()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;
    let expected_factor_bps = if claim_atoms == 0 {
        10_000
    } else {
        vault_bridge_policy::bucket_factor_bps(operation.updated_counted_value_atoms, claim_atoms)
            .map_err(|error| (error.code(), error.message().to_string()))?
    };
    if operation.impairment_factor_bps != expected_factor_bps {
        return Err((
            "vault_bridge_impairment_factor_mismatch",
            "vault bridge asset impairment factor must equal floor(updated_counted_value_atoms * 10000 / bucket claims)"
                .to_string(),
        ));
    }

    let mut bucket_after = bucket;
    bucket_after.counted_value_atoms = operation.updated_counted_value_atoms;
    bucket_after.impairment_factor_bps = expected_factor_bps;
    bucket_after.status = if bucket_after.status == VAULT_BRIDGE_BUCKET_STATUS_PAUSED {
        VAULT_BRIDGE_BUCKET_STATUS_PAUSED.to_string()
    } else {
        VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED.to_string()
    };
    bucket_after.last_updated_height = block_height;
    bucket_after
        .validate()
        .map_err(|error| ("bad_vault_bridge_bucket", error))?;

    ledger.vault_bridge_bucket_states[bucket_index] = bucket_after;
    for receipt in ledger.vault_bridge_receipts.iter_mut().filter(|receipt| {
        receipt.asset_id == operation.asset_id && receipt.bucket_id == operation.bucket_id
    }) {
        if receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_REJECTED
            && receipt.status != VAULT_BRIDGE_RECEIPT_STATUS_RETIRED
        {
            receipt.status = VAULT_BRIDGE_RECEIPT_STATUS_IMPAIRED.to_string();
            receipt
                .validate()
                .map_err(|error| ("bad_vault_bridge_receipt", error))?;
        }
    }
    Ok(())
}

fn vault_bridge_source_proof_is_consensus_verified(source_proof_kind: &str) -> bool {
    matches!(
        source_proof_kind,
        SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1 | NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
    )
}

struct VaultBridgeDepositSourceProof<'a> {
    genesis: Option<&'a Genesis>,
    profile: &'a NavProofProfile,
    evidence: &'a VaultBridgeDepositEvidence,
    evidence_root: &'a str,
    policy_hash: &'a str,
    source_proof_kind: &'a str,
    source_proof_hash: &'a str,
    source_public_values_hash: &'a str,
    source_proof_bytes: &'a [u8],
    source_public_values: &'a [u8],
    fast_ingress_verifier: Option<&'a FastIngressVerifierConfigV1>,
}

#[derive(Debug)]
enum VerifiedIngressPublicValues {
    Confirmed(postfiat_types::PfUsdcIngressPublicValuesV3),
    Bonded(postfiat_types::PfUsdcBondedIngressPublicValuesV1),
    Ethereum { nullifier: String },
}

fn ensure_vault_bridge_deposit_source_proof(
    proof: VaultBridgeDepositSourceProof<'_>,
) -> Result<Option<VerifiedIngressPublicValues>, (&'static str, String)> {
    let VaultBridgeDepositSourceProof {
        genesis,
        profile,
        evidence,
        evidence_root,
        policy_hash,
        source_proof_kind,
        source_proof_hash,
        source_public_values_hash,
        source_proof_bytes,
        source_public_values,
        fast_ingress_verifier,
    } = proof;
    let ethereum_finality_route = profile
        .source_class
        .strip_prefix(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
        .and_then(parse_vault_bridge_source_domain_parts)
        .map(|(chain_id, _, _)| {
            matches!(
                chain_id,
                ETHEREUM_MAINNET_CHAIN_ID | ETHEREUM_SEPOLIA_CHAIN_ID
            )
        })
        .unwrap_or(false);
    if profile.verifier_kind == NAV_PROFILE_VERIFIER_SP1_GROTH16
        && ethereum_finality_route
        && source_proof_kind != SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1
    {
        return Err((
            "ethereum_ingress_source_proof_kind_required",
            "Ethereum ingress routes require sp1-ethereum-finality-v1 proof material".to_string(),
        ));
    }
    if source_proof_kind == SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1 {
        if profile.verifier_kind != NAV_PROFILE_VERIFIER_SP1_GROTH16
            || source_proof_hash.is_empty()
            || source_public_values_hash.is_empty()
        {
            return Err((
                "missing_ethereum_ingress_source_proof",
                "Ethereum-finality deposits require a sp1-groth16 profile and committed proof material".to_string(),
            ));
        }
        // Proposal execution verifies the complete proof and public values,
        // then consensus retains only their commitments in the deposit record.
        // Finalization must validate those committed fields without demanding
        // the discarded proof bytes again.
        let Some(_genesis) = genesis else {
            return Ok(None);
        };
        if source_proof_bytes.is_empty() || source_public_values.is_empty() {
            return Err((
                "missing_ethereum_ingress_source_proof",
                "Ethereum-finality deposit proposals require complete proof material".to_string(),
            ));
        }
        if postfiat_types::pfusdc_ingress_proof_hash_v1(source_proof_bytes) != source_proof_hash
            || postfiat_types::pfusdc_ingress_public_values_hash_v1(source_public_values)
                != source_public_values_hash
        {
            return Err((
                "ethereum_ingress_source_commitment_mismatch",
                "Ethereum ingress proof commitments do not match supplied bytes".to_string(),
            ));
        }
        verify_bounded_sp1_groth16(
            profile,
            NAV_PROFILE_VERIFIER_SP1_GROTH16,
            source_proof_bytes,
            source_public_values,
        )
        .map_err(|error| (error.code(), error.message()))?;
        let values = postfiat_types::PfUsdcEthereumIngressPublicValuesV1::from_canonical_bytes(
            source_public_values,
        )
        .map_err(|error| ("ethereum_ingress_public_values_invalid", error))?;
        ensure_ethereum_ingress_public_values_match(
            profile,
            evidence,
            evidence_root,
            policy_hash,
            &values,
        )?;
        return Ok(Some(VerifiedIngressPublicValues::Ethereum {
            nullifier: values.deposit_nullifier,
        }));
    }
    if source_proof_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1 {
        let config = fast_ingress_verifier.ok_or_else(|| {
            (
                "pfusdc_fast_ingress_verifier_missing",
                "bonded ingress requires a governance-pinned secondary verifier configuration"
                    .to_string(),
            )
        })?;
        if source_proof_hash.is_empty() || source_public_values_hash.is_empty() {
            return Err((
                "missing_vault_bridge_bonded_source_proof",
                "bonded pfUSDC ingress requires its dedicated proof kind and commitments"
                    .to_string(),
            ));
        }
        let Some(genesis) = genesis else {
            return Ok(None);
        };
        if postfiat_types::pfusdc_ingress_proof_hash_v1(source_proof_bytes) != source_proof_hash
            || postfiat_types::pfusdc_ingress_public_values_hash_v1(source_public_values)
                != source_public_values_hash
        {
            return Err((
                "vault_bridge_bonded_source_commitment_mismatch",
                "bonded ingress proof/public-values commitment mismatch".to_string(),
            ));
        }
        verify_bounded_sp1_groth16_with_config(
            &config.verifier_kind,
            NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1,
            &config.verifier_program_vkey,
            config.max_proof_bytes,
            config.max_public_values_bytes,
            source_proof_bytes,
            source_public_values,
        )
        .map_err(|error| (error.code(), error.message()))?;
        let public_values =
            postfiat_types::PfUsdcBondedIngressPublicValuesV1::from_canonical_bytes(
                source_public_values,
            )
            .map_err(|error| ("pfusdc_bonded_public_values_invalid", error))?;
        if public_values.manifest_hash != config.deployment_manifest_hash
            || public_values.verifier_policy_hash != config.verifier_policy_hash
            || public_values.cap_atoms != config.cap_atoms
            || public_values.age_margin_blocks != config.age_margin_blocks
            || public_values.asset_id != config.asset_id
            || public_values.route_profile_hash != config.base_route_profile_hash
            || public_values.route_epoch != config.route_epoch
        {
            return Err((
                "pfusdc_fast_ingress_config_mismatch",
                "bonded ingress public values do not match the governed secondary verifier configuration"
                    .to_string(),
            ));
        }
        ensure_pfusdc_bonded_public_values_match(
            genesis,
            evidence,
            evidence_root,
            policy_hash,
            &public_values,
        )?;
        return Ok(Some(VerifiedIngressPublicValues::Bonded(public_values)));
    }
    match profile.verifier_kind.as_str() {
        NAV_PROFILE_VERIFIER_MULTI_FETCH => {
            if !source_proof_kind.is_empty()
                || !source_proof_hash.is_empty()
                || !source_public_values_hash.is_empty()
                || !source_proof_bytes.is_empty()
                || !source_public_values.is_empty()
            {
                return Err((
                    "unexpected_vault_bridge_deposit_source_proof",
                    "multi-fetch vault bridge asset bridge deposits must not carry SP1 source-proof fields"
                        .to_string(),
                ));
            }
            Ok(None)
        }
        NAV_PROFILE_VERIFIER_SP1_GROTH16 => {
            if source_proof_kind != NAV_PROFILE_VERIFIER_SP1_GROTH16
                || source_proof_hash.is_empty()
                || source_public_values_hash.is_empty()
            {
                return Err((
                    "missing_vault_bridge_deposit_source_proof",
                    "sp1-groth16 vault bridge asset bridge deposits require source_proof_kind, source_proof_hash, and source_public_values_hash"
                        .to_string(),
                ));
            }
            let expected_public_values_hash =
                vault_bridge_deposit_public_values_hash(evidence, evidence_root, policy_hash)
                    .map_err(|error| ("bad_vault_bridge_deposit_source_proof", error))?;
            if source_public_values_hash != expected_public_values_hash {
                return Err((
                    "vault_bridge_deposit_source_public_values_mismatch",
                    "vault bridge asset bridge deposit source_public_values_hash must bind the exact vault event evidence"
                        .to_string(),
                ));
            }
            Ok(None)
        }
        NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1 => {
            if source_proof_kind != NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
                || source_proof_hash.is_empty()
                || source_public_values_hash.is_empty()
            {
                return Err((
                    "missing_vault_bridge_deposit_source_proof",
                    "proof-native vault bridge deposit requires its dedicated proof kind and proof commitments"
                        .to_string(),
                ));
            }
            // A committed deposit record stores only digests. Replay verifies
            // the material at proposal execution; later finalization trusts
            // that consensus-committed record rather than retaining megabytes.
            let Some(genesis) = genesis else {
                return Ok(None);
            };
            if postfiat_types::pfusdc_ingress_proof_hash_v1(source_proof_bytes)
                != source_proof_hash
            {
                return Err((
                    "vault_bridge_deposit_source_proof_hash_mismatch",
                    "source_proof_hash does not commit the supplied proof bytes".to_string(),
                ));
            }
            if postfiat_types::pfusdc_ingress_public_values_hash_v1(source_public_values)
                != source_public_values_hash
            {
                return Err((
                    "vault_bridge_deposit_source_public_values_hash_mismatch",
                    "source_public_values_hash does not commit the supplied public values"
                        .to_string(),
                ));
            }
            verify_bounded_sp1_groth16(
                profile,
                NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1,
                source_proof_bytes,
                source_public_values,
            )
            .map_err(|error| (error.code(), error.message()))?;
            let public_values =
                postfiat_types::PfUsdcIngressPublicValuesV3::from_canonical_bytes(
                    source_public_values,
                )
                .map_err(|error| ("pfusdc_ingress_public_values_invalid", error))?;
            ensure_pfusdc_ingress_public_values_match(
                genesis,
                evidence,
                evidence_root,
                policy_hash,
                &public_values,
            )?;
            Ok(Some(VerifiedIngressPublicValues::Confirmed(public_values)))
        }
        _ => Err((
            "unsupported_vault_bridge_deposit_verifier",
            "vault bridge asset bridge deposits require a multi-fetch-quorum or sp1-groth16 profile"
                .to_string(),
        )),
    }
}

fn ensure_ethereum_ingress_public_values_match(
    profile: &NavProofProfile,
    evidence: &VaultBridgeDepositEvidence,
    evidence_root: &str,
    policy_hash: &str,
    values: &postfiat_types::PfUsdcEthereumIngressPublicValuesV1,
) -> Result<(), (&'static str, String)> {
    let expected_source_class = format!(
        "{VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX}erc20_bridge_vault:{}:{}:{}",
        evidence.source_chain_id, evidence.vault_address, evidence.token_address
    );
    let expected_route_id = match evidence.source_chain_id {
        ETHEREUM_MAINNET_CHAIN_ID => VAULT_BRIDGE_ROUTE_ETHEREUM_MAINNET_USDC_V1,
        ETHEREUM_SEPOLIA_CHAIN_ID => VAULT_BRIDGE_ROUTE_ETHEREUM_SEPOLIA_USDC_V1,
        _ => {
            return Err((
                "ethereum_ingress_source_chain_unsupported",
                "Ethereum ingress proof uses an unregistered source chain".to_string(),
            ));
        }
    };
    let mismatch = profile.source_class != expected_source_class
        || values.route_id != expected_route_id
        || profile.valuation_policy_hash != values.manifest_hash
        || (!profile.vault_bridge_route_policy_hash.is_empty()
            && profile.vault_bridge_route_policy_hash != policy_hash)
        || values.source_chain_id != evidence.source_chain_id
        || values.vault_address != evidence.vault_address
        || values.token_address != evidence.token_address
        || values.depositor != evidence.depositor
        || values.pftl_recipient != evidence.pftl_recipient
        || values.pftl_recipient_hash != evidence.pftl_recipient_hash
        || values.amount_atoms != evidence.amount_atoms
        || values.nonce != evidence.nonce
        || values.route_binding != evidence.route_binding
        || values.deposit_id != evidence.deposit_id
        || values.finalized_execution_block_hash != evidence.block_hash
        || values.evidence_root != evidence_root;
    if mismatch {
        return Err((
            "ethereum_ingress_public_values_mismatch",
            "proof-verified Ethereum ingress values do not match the governed route and deposit evidence".to_string(),
        ));
    }
    Ok(())
}

fn ensure_pfusdc_bonded_public_values_match(
    genesis: &Genesis,
    evidence: &VaultBridgeDepositEvidence,
    evidence_root: &str,
    policy_hash: &str,
    values: &postfiat_types::PfUsdcBondedIngressPublicValuesV1,
) -> Result<(), (&'static str, String)> {
    let route_epoch = u32::try_from(values.route_epoch).map_err(|_| {
        (
            "pfusdc_bonded_route_epoch_invalid",
            "bonded ingress route epoch exceeds u32".to_string(),
        )
    })?;
    let expected_route_binding =
        postfiat_types::vault_bridge_route_binding(policy_hash, route_epoch)
            .map_err(|error| ("pfusdc_bonded_route_binding_invalid", error))?;
    let mismatch = values.proof_program_version != 1
        || values.pftl_chain_id != genesis.chain_id
        || values.pftl_genesis_hash != genesis_hash(genesis)
        || values.pftl_protocol_version != genesis.protocol_version
        || values.route_profile_hash != policy_hash
        || values.arbitrum_chain_id != evidence.source_chain_id
        || values.vault_address != evidence.vault_address
        || values.token_address != evidence.token_address
        || values.depositor != evidence.depositor
        || values.pftl_recipient != evidence.pftl_recipient
        || values.pftl_recipient_hash != evidence.pftl_recipient_hash
        || values.amount_atoms != evidence.amount_atoms
        || values.deposit_nonce != evidence.nonce
        || values.route_binding != evidence.route_binding
        || values.route_binding != expected_route_binding
        || values.deposit_id != evidence.deposit_id
        || values.evidence_root != evidence_root;
    if mismatch {
        return Err((
            "pfusdc_bonded_public_values_mismatch",
            "bonded pfUSDC public values do not exactly match chain, route, and deposit"
                .to_string(),
        ));
    }
    Ok(())
}

fn ensure_pfusdc_ingress_public_values_match(
    genesis: &Genesis,
    evidence: &VaultBridgeDepositEvidence,
    evidence_root: &str,
    policy_hash: &str,
    values: &postfiat_types::PfUsdcIngressPublicValuesV3,
) -> Result<(), (&'static str, String)> {
    let route_epoch = u32::try_from(values.route_epoch).map_err(|_| {
        (
            "pfusdc_ingress_route_epoch_invalid",
            "pfUSDC ingress route epoch exceeds the governed u32 range".to_string(),
        )
    })?;
    let expected_route_binding =
        postfiat_types::vault_bridge_route_binding(policy_hash, route_epoch)
            .map_err(|error| ("pfusdc_ingress_route_binding_invalid", error))?;
    let expected_genesis_hash = genesis_hash(genesis);
    let mismatch = values.proof_program_version != 3
        || values.pftl_chain_id != genesis.chain_id
        || values.pftl_genesis_hash != expected_genesis_hash
        || values.pftl_protocol_version != genesis.protocol_version
        || values.route_profile_hash != policy_hash
        || values.arbitrum_chain_id != evidence.source_chain_id
        || values.assertion_l2_block_hash != evidence.block_hash
        || values.vault_address != evidence.vault_address
        || values.token_address != evidence.token_address
        || values.output_item_hash != evidence.tx_hash
        || values.output_index != evidence.log_index
        || values.depositor != evidence.depositor
        || values.pftl_recipient != evidence.pftl_recipient
        || values.pftl_recipient_hash != evidence.pftl_recipient_hash
        || values.amount_atoms != evidence.amount_atoms
        || values.nonce != evidence.nonce
        || values.route_binding != evidence.route_binding
        || values.route_binding != expected_route_binding
        || values.deposit_id != evidence.deposit_id
        || values.evidence_root != evidence_root;
    if mismatch {
        return Err((
            "pfusdc_ingress_public_values_mismatch",
            "proof-verified pfUSDC ingress public values do not exactly match the active chain, route, and deposit evidence"
                .to_string(),
        ));
    }
    Ok(())
}

fn ensure_vault_bridge_asset_policy(
    ledger: &LedgerState,
    nav_asset: &NavTrackedAsset,
    operator: &str,
) -> Result<(), (&'static str, String)> {
    if operator != nav_asset.issuer && operator != nav_asset.reserve_operator {
        return Err((
            "unauthorized_vault_bridge_operator",
            "vault bridge asset operator must be the asset issuer or reserve operator".to_string(),
        ));
    }
    ensure_vault_bridge_asset_registration(ledger, nav_asset)
}

fn ensure_vault_bridge_asset_registration(
    ledger: &LedgerState,
    nav_asset: &NavTrackedAsset,
) -> Result<(), (&'static str, String)> {
    let asset = ledger
        .asset_definition(&nav_asset.asset_id)
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", nav_asset.asset_id),
            )
        })?;
    if asset.issuer != nav_asset.issuer {
        return Err((
            "asset_issuer_mismatch",
            "vault bridge asset NAV asset issuer does not match issued asset issuer".to_string(),
        ));
    }
    Ok(())
}

fn ensure_vault_bridge_receipt_fresh(
    profile: &NavProofProfile,
    receipt: &VaultBridgeReceipt,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    if receipt.expires_at_height == 0 || block_height > receipt.expires_at_height {
        return Err((
            "stale_vault_bridge_receipt",
            "vault bridge asset receipt is expired".to_string(),
        ));
    }
    if profile.max_snapshot_age_blocks > 0
        && receipt.created_at_height > 0
        && block_height
            > receipt
                .created_at_height
                .saturating_add(profile.max_snapshot_age_blocks)
    {
        return Err((
            "stale_vault_bridge_receipt",
            "vault bridge asset receipt exceeds the profile max snapshot age".to_string(),
        ));
    }
    Ok(())
}

fn validate_vault_bridge_reserve_packet_fields(
    ledger: &LedgerState,
    nav_asset: &NavTrackedAsset,
    profile: &NavProofProfile,
    operation: &NavReserveSubmitOperation,
) -> Result<(), (&'static str, String)> {
    ensure_vault_bridge_asset_policy(ledger, nav_asset, &operation.submitter)?;
    if operation.nav_per_unit != VAULT_BRIDGE_UNIT {
        return Err((
            "vault_bridge_nav_per_unit_mismatch",
            format!("vault bridge asset nav_per_unit must equal {VAULT_BRIDGE_UNIT}"),
        ));
    }
    let source_domain = profile
        .source_class
        .strip_prefix(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
        .unwrap_or_default();
    ensure_vault_bridge_source_policy(
        profile,
        source_domain,
        vault_bridge_route_policy_hash(profile),
    )?;

    let counted_value = vault_bridge_counted_value_for_asset(
        &ledger.vault_bridge_bucket_states,
        &operation.asset_id,
    )
    .map_err(|error| ("bad_vault_bridge_buckets", error))?;
    let finalized_unclaimed_sp1_backing =
        finalized_unclaimed_sp1_backing_for_asset(ledger, profile, &operation.asset_id)?;
    let expected_verified_net_assets = counted_value
        .checked_add(finalized_unclaimed_sp1_backing)
        .ok_or_else(|| {
        (
            "vault_bridge_verified_net_assets_overflow",
            "vault bridge counted value plus finalized SP1 backing overflowed".to_string(),
        )
    })?;
    if operation.verified_net_assets != expected_verified_net_assets {
        return Err((
            "vault_bridge_verified_net_assets_mismatch",
            "vault bridge asset reserve packet verified_net_assets must equal active bucket counted value"
                .to_string(),
        ));
    }
    let current_supply = issued_asset_supply(ledger, &operation.asset_id)?;
    let maximum_proof_backed_supply = current_supply
        .checked_add(finalized_unclaimed_sp1_backing)
        .ok_or_else(|| {
            (
                "vault_bridge_circulating_supply_overflow",
                "issued supply plus finalized SP1 backing overflowed".to_string(),
            )
        })?;
    if operation.circulating_supply < current_supply
        || operation.circulating_supply > maximum_proof_backed_supply
    {
        return Err((
            "vault_bridge_circulating_supply_mismatch",
            "vault bridge reserve packet circulating_supply must be at least current issued supply and may lead it only by finalized, unclaimed SP1-backed deposits".to_string(),
        ));
    }
    let expected_source_root =
        vault_bridge_source_root_for_asset(&ledger.vault_bridge_bucket_states, &operation.asset_id)
            .map_err(|error| ("bad_vault_bridge_source_root", error))?;
    if operation.source_root != expected_source_root {
        return Err((
            "vault_bridge_source_root_mismatch",
            "vault bridge asset reserve packet source_root must match deterministic source bucket summaries"
                .to_string(),
        ));
    }
    Ok(())
}

fn finalized_unclaimed_sp1_backing_for_asset(
    ledger: &LedgerState,
    profile: &NavProofProfile,
    asset_id: &str,
) -> Result<u64, (&'static str, String)> {
    let source_domain = profile
        .source_class
        .strip_prefix(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
        .unwrap_or_default();
    let route_policy_hash = vault_bridge_route_policy_hash(profile);
    let mut total = 0_u64;
    for record in ledger.vault_bridge_deposits.iter().filter(|record| {
        record.asset_id == asset_id
            && record.status == VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED
            && record.source_proof_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1
            && record.policy_hash == route_policy_hash
            && record.evidence.source_domain() == source_domain
    }) {
        let consumer_id = format!("vault_bridge_deposit_claim:{}", record.evidence_root);
        if ledger
            .vault_bridge_allocations
            .iter()
            .any(|allocation| allocation.consumer_id == consumer_id)
        {
            continue;
        }
        let releasable = ledger.fast_ingress_campaigns.iter().any(|campaign| {
            campaign.route_profile_hash == record.policy_hash
                && campaign.mints.iter().any(|mint| {
                    mint.deposit_id == record.evidence.deposit_id
                        && mint.recipient == record.evidence.pftl_recipient
                        && mint.amount_atoms == record.evidence.amount_atoms
                        && !mint.claimed
                        && matches!(
                            mint.status.as_str(),
                            postfiat_types::FAST_INGRESS_MINT_STATUS_FINAL
                                | postfiat_types::FAST_INGRESS_MINT_STATUS_RELEASED_UNCONFIRMED
                        )
                })
        });
        if !releasable {
            continue;
        }
        total = total
            .checked_add(record.evidence.amount_atoms)
            .ok_or_else(|| {
                (
                    "vault_bridge_sp1_backing_overflow",
                    "finalized unclaimed SP1 backing overflowed".to_string(),
                )
            })?;
    }
    Ok(total)
}

/// Deterministic challenge-bond resolution at finalization. For every
/// challenged packet of this asset at or below the finalized epoch:
/// same-epoch challenges (the issuer replaced the challenged packet)
/// refund the challenger; lower-epoch challenges (the issuer abandoned
/// the epoch entirely) forfeit the bond to the issuer.
fn resolve_nav_challenge_bonds(
    ledger: &mut LedgerState,
    asset_id: &str,
    finalized_epoch: u64,
    finalized_packet_hash: &str,
) -> Result<(), (&'static str, String)> {
    let mut payouts: Vec<(usize, String, u64)> = Vec::new();
    for (index, packet) in ledger.nav_reserve_packets.iter().enumerate() {
        if packet.asset_id != asset_id
            || packet.state != NAV_RESERVE_STATE_CHALLENGED
            || packet.challenge_bond == 0
            || packet.epoch > finalized_epoch
        {
            continue;
        }
        let challenger_was_right =
            packet.epoch == finalized_epoch && packet.reserve_packet_hash != finalized_packet_hash;
        let recipient = if challenger_was_right {
            packet.challenger.clone()
        } else {
            packet.issuer.clone()
        };
        payouts.push((index, recipient, packet.challenge_bond));
    }
    for (index, recipient, amount) in payouts {
        let account = ledger.account_mut(&recipient).ok_or_else(|| {
            (
                "missing_bond_recipient",
                format!("nav challenge bond recipient `{recipient}` does not exist"),
            )
        })?;
        account.balance = account.balance.checked_add(amount).ok_or_else(|| {
            (
                "bond_payout_overflow",
                "nav challenge bond payout would overflow recipient balance".to_string(),
            )
        })?;
        ledger.nav_reserve_packets[index].challenge_bond = 0;
    }
    Ok(())
}

fn ensure_nav_asset_live_for_epoch(
    ledger: &LedgerState,
    nav_asset: &NavTrackedAsset,
    epoch: u64,
    reserve_packet_hash: &str,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    if let Some(profile) = nav_profile_for_asset(ledger, nav_asset) {
        if profile.max_epoch_gap_blocks > 0
            && nav_asset.finalized_at_height > 0
            && block_height
                > nav_asset
                    .finalized_at_height
                    .saturating_add(profile.max_epoch_gap_blocks)
        {
            return Err((
                "nav_reserve_stale_deadman",
                "nav asset's finalized reserve packet has exceeded the profile's max epoch gap; mint and redeem fail closed".to_string(),
            ));
        }
    }
    if nav_asset.halted {
        return Err((
            "nav_asset_halted",
            format!("nav asset is halted: {}", nav_asset.halt_reason),
        ));
    }
    if nav_asset.finalized_epoch != epoch {
        return Err((
            "nav_epoch_mismatch",
            "nav operation epoch does not match finalized epoch".to_string(),
        ));
    }
    if nav_asset.finalized_reserve_packet_hash != reserve_packet_hash {
        return Err((
            "nav_reserve_packet_mismatch",
            "nav operation reserve packet hash does not match finalized packet".to_string(),
        ));
    }
    if nav_asset.nav_per_unit == 0 {
        return Err((
            "nav_not_finalized",
            "nav asset has no finalized NAV per unit".to_string(),
        ));
    }
    Ok(())
}

fn finalize_market_ops_envelope(
    ledger: &mut LedgerState,
    operation: &MarketOpsFinalizeOperation,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    operation
        .envelope
        .validate_basic()
        .map_err(|error| ("bad_market_ops_envelope", error))?;
    operation
        .policy_inputs
        .validate()
        .map_err(|error| ("bad_market_ops_policy_inputs", error))?;

    let computed_envelope_hash = bytes_to_hex(&operation.envelope.envelope_hash());
    if computed_envelope_hash != operation.envelope_hash {
        return Err((
            "market_ops_envelope_hash_mismatch",
            "submitted market ops envelope_hash does not match envelope fields".to_string(),
        ));
    }

    let nav_asset = ledger
        .nav_asset(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_nav_asset",
                format!("nav asset `{}` does not exist", operation.asset_id),
            )
        })?;
    if nav_asset.issuer != operation.issuer {
        return Err((
            "nav_issuer_mismatch",
            "market_ops_finalize issuer does not match nav asset issuer".to_string(),
        ));
    }
    if operation.envelope.epoch != nav_asset.finalized_epoch || nav_asset.finalized_epoch == 0 {
        return Err((
            "market_ops_epoch_not_finalized",
            "market ops envelope epoch must equal the current finalized nav epoch".to_string(),
        ));
    }
    if operation.envelope.asset_id
        != market_ops_asset_id(&operation.asset_id).map_err(|error| {
            (
                "bad_market_ops_asset_id",
                format!("could not derive market ops asset id: {error}"),
            )
        })?
    {
        return Err((
            "market_ops_asset_id_mismatch",
            "market ops envelope asset_id does not match ledger asset_id".to_string(),
        ));
    }
    if ledger
        .market_ops_envelope(&operation.asset_id, operation.envelope.epoch)
        .is_some()
    {
        return Err((
            "duplicate_market_ops_envelope",
            "market ops envelope already finalized for asset and epoch".to_string(),
        ));
    }
    if ledger
        .market_ops_policy_for_envelope(&operation.envelope)
        .is_none()
    {
        return Err((
            "unregistered_market_ops_policy",
            "market ops envelope references an unregistered program_id/policy_hash tuple"
                .to_string(),
        ));
    }

    let packet = ledger
        .nav_reserve_packet(
            &operation.asset_id,
            operation.envelope.epoch,
            &nav_asset.finalized_reserve_packet_hash,
        )
        .cloned()
        .ok_or_else(|| {
            (
                "missing_finalized_nav_reserve_packet",
                "market ops finalize references missing finalized reserve packet".to_string(),
            )
        })?;
    ensure_market_ops_packets_fresh(ledger, &nav_asset, &packet, block_height)?;

    let expected_envelope = recompute_market_ops_envelope(&operation.asset_id, &packet, operation)?;
    if expected_envelope != operation.envelope {
        return Err((
            "market_ops_envelope_mismatch",
            "submitted market ops envelope does not match deterministic policy replay".to_string(),
        ));
    }

    ledger
        .market_ops_envelopes
        .push(FinalizedMarketOpsEnvelope {
            asset_id: operation.asset_id.clone(),
            epoch: operation.envelope.epoch,
            envelope_hash: operation.envelope_hash.clone(),
            envelope: operation.envelope.clone(),
            policy_inputs: Some(operation.policy_inputs.clone()),
            finalized_at_height: block_height,
        });
    Ok(())
}

fn ensure_market_ops_packets_fresh(
    ledger: &LedgerState,
    nav_asset: &NavTrackedAsset,
    packet: &NavReservePacket,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    if packet.state != NAV_RESERVE_STATE_FINALIZED {
        return Err((
            "nav_packet_not_finalized",
            "market ops finalize requires a finalized reserve packet".to_string(),
        ));
    }
    if packet.reserve_packet_hash != nav_asset.finalized_reserve_packet_hash {
        return Err((
            "market_ops_reserve_packet_mismatch",
            "market ops reserve packet does not match current finalized nav packet".to_string(),
        ));
    }
    if let Some(profile) = nav_profile_for_asset(ledger, nav_asset) {
        if packet.submitted_at_height > 0
            && profile.max_snapshot_age_blocks > 0
            && block_height
                > packet
                    .submitted_at_height
                    .saturating_add(profile.max_snapshot_age_blocks)
        {
            return Err((
                "stale_market_ops_reserve_packet",
                "market ops reserve packet is older than the profile's max snapshot age"
                    .to_string(),
            ));
        }
        if nav_asset.finalized_at_height > 0
            && profile.max_epoch_gap_blocks > 0
            && block_height
                > nav_asset
                    .finalized_at_height
                    .saturating_add(profile.max_epoch_gap_blocks)
        {
            return Err((
                "stale_market_ops_supply_packet",
                "market ops supply packet is older than the profile's max epoch gap".to_string(),
            ));
        }
    }
    Ok(())
}

fn recompute_market_ops_envelope(
    asset_id: &str,
    packet: &NavReservePacket,
    operation: &MarketOpsFinalizeOperation,
) -> Result<MarketOpsEnvelope, (&'static str, String)> {
    let inputs = &operation.policy_inputs;
    let valid_global_supply_atoms = u128::from(packet.circulating_supply);
    let verified_net_assets_usd_e8 = u128::from(packet.verified_net_assets);
    let nav_floor = crate::market_policy::compute_nav_floor_with_unit_scale(
        verified_net_assets_usd_e8,
        valid_global_supply_atoms,
        u128::from(inputs.floor_factor_bps),
        inputs.unit_scale,
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;
    let backing_capacity = crate::market_policy::compute_backing_capacity_with_unit_scale(
        verified_net_assets_usd_e8,
        valid_global_supply_atoms,
        nav_floor.nav_floor_usd_e8,
        inputs.unit_scale,
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;
    let portfolio_floor_value_usd_e8 = crate::market_policy::mul_div_floor(
        valid_global_supply_atoms,
        nav_floor.nav_floor_usd_e8,
        inputs.unit_scale,
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;
    let alignment = crate::market_policy::compute_alignment_reserve_requirement(
        portfolio_floor_value_usd_e8,
        &inputs.cost_to_restore_14d_usd_e8,
        &inputs.cost_to_restore_90d_usd_e8,
        to_alignment_params(&inputs.alignment_params),
        inputs.previous_required_alignment_reserve_usd_e8,
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;

    let evidence_root =
        market_ops_evidence_root(&inputs.discount_observations, &inputs.premium_observations)
            .map_err(|error| ("bad_market_ops_evidence", error))?;
    if evidence_root != operation.envelope.evidence_root {
        return Err((
            "invalid_market_ops_evidence_root",
            "market ops evidence root does not match submitted observations".to_string(),
        ));
    }

    let window_seconds = operation
        .envelope
        .data_window_end
        .checked_sub(operation.envelope.data_window_start)
        .ok_or_else(|| {
            (
                "bad_market_ops_window",
                "market ops data window underflows".to_string(),
            )
        })?;
    let discount_metrics = crate::market_policy::compute_discount_metrics(
        &to_venue_observations(&inputs.discount_observations),
        nav_floor.nav_floor_usd_e8,
        u128::from(operation.envelope.discount_trigger_bps),
        u128::from(window_seconds),
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;
    let discount_response_bps = crate::market_policy::compute_discount_response_bps(
        discount_metrics.response_curve_metrics(),
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;
    let reserve_cap = crate::market_policy::compute_reserve_deploy_cap(
        operation.envelope.funded_alignment_reserve_usd_e8,
        discount_response_bps,
        to_reserve_deploy_limits(&inputs.reserve_limits),
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;

    let premium_metrics = crate::market_policy::compute_premium_metrics(
        &to_venue_observations(&inputs.premium_observations),
        nav_floor.nav_floor_usd_e8,
        u128::from(operation.envelope.premium_trigger_bps),
        u128::from(window_seconds),
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;
    let premium_response_bps = crate::market_policy::compute_premium_response_bps(
        premium_metrics.response_curve_metrics(),
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;
    let mint_cap = crate::market_policy::compute_mint_cap(
        valid_global_supply_atoms,
        premium_response_bps,
        backing_capacity.verified_capacity_remaining_atoms,
        to_mint_limits(&inputs.mint_limits),
    )
    .map_err(|error| ("market_ops_policy_error", format!("{error}")))?;

    let mut expected = operation.envelope.clone();
    expected.asset_id =
        market_ops_asset_id(asset_id).map_err(|error| ("bad_market_ops_asset_id", error))?;
    expected.epoch = packet.epoch;
    expected.reserve_packet_hash = market_ops_reserve_packet_hash(&packet.reserve_packet_hash)
        .map_err(|error| ("bad_market_ops_reserve_packet_hash", error))?;
    expected.supply_packet_hash =
        market_ops_supply_packet_hash(asset_id, packet.epoch, valid_global_supply_atoms)
            .map_err(|error| ("bad_market_ops_supply_packet_hash", error))?;
    expected.evidence_root = evidence_root;
    expected.nav_floor_usd_e8 = nav_floor.nav_floor_usd_e8;
    expected.valid_global_supply_atoms = valid_global_supply_atoms;
    expected.verified_net_assets_usd_e8 = verified_net_assets_usd_e8;
    expected.required_alignment_reserve_usd_e8 = alignment.required_alignment_reserve_next_usd_e8;
    expected.max_reserve_deploy_usd_e8 = reserve_cap.reserve_deploy_cap_usd_e8;
    expected.max_mint_atoms = mint_cap.mint_cap_atoms;
    Ok(expected)
}

fn to_alignment_params(
    params: &MarketOpsAlignmentParams,
) -> crate::market_policy::AlignmentReserveParams {
    crate::market_policy::AlignmentReserveParams {
        policy_min_usd_e8: params.policy_min_usd_e8,
        min_alignment_bps: u128::from(params.min_alignment_bps),
        stress_repeat_factor_14d: params.stress_repeat_factor_14d,
        stress_repeat_factor_90d: params.stress_repeat_factor_90d,
        stale_epochs_allowed: params.stale_epochs_allowed,
        max_decay_per_epoch_bps: u128::from(params.max_decay_per_epoch_bps),
    }
}

fn to_reserve_deploy_limits(
    limits: &MarketOpsReserveDeployLimits,
) -> crate::market_policy::ReserveDeployLimits {
    crate::market_policy::ReserveDeployLimits {
        available_alignment_reserve_usd_e8: limits.available_alignment_reserve_usd_e8,
        venue_policy_cap_usd_e8: limits.venue_policy_cap_usd_e8,
        depth_limited_cap_usd_e8: limits.depth_limited_cap_usd_e8,
        cooldown_limited_cap_usd_e8: limits.cooldown_limited_cap_usd_e8,
    }
}

fn to_mint_limits(limits: &MarketOpsMintLimits) -> crate::market_policy::MintCapLimits {
    crate::market_policy::MintCapLimits {
        policy_max_mint_atoms: limits.policy_max_mint_atoms,
        venue_bid_depth_atoms: limits.venue_bid_depth_atoms,
        cooldown_mint_atoms: limits.cooldown_mint_atoms,
    }
}

fn to_venue_observations(
    observations: &[MarketOpsVenueObservation],
) -> Vec<crate::market_policy::VenueObservation> {
    observations
        .iter()
        .map(|observation| crate::market_policy::VenueObservation {
            dt_seconds: u128::from(observation.dt_seconds),
            price_usd_e8: observation.price_usd_e8,
            volume_usd_e8: observation.volume_usd_e8,
        })
        .collect()
}
