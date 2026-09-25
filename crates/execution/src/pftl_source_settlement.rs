// Source custody is deliberately outside the exported route witness. The
// replicated state root commits it; existing route receipt proofs stay valid.
fn pftl_source_asset_check(
    ledger: &LedgerState, route: &PftlUniswapConsensusRouteState, asset_id: &str,
) -> Result<(), (&'static str, String)> {
    let asset = ledger.asset_definition(asset_id).ok_or_else(||
        ("pftl_source_missing", "settlement source asset is missing".to_string()))?;
    if asset.asset_family_id != route.settlement_asset_id || asset.source_series_id != asset_id
        || asset.asset_id == route.settlement_asset_id || asset.precision != 6 {
        return Err(("pftl_source_family_mismatch", "settlement must be an exact source series of the route family".to_string()));
    }
    let bucket = ledger.vault_bridge_bucket_states.iter().find(|b| b.bucket_id == asset.source_bucket_id)
        .ok_or_else(|| ("pftl_source_bucket_missing", "settlement source bucket is missing".to_string()))?;
    if bucket.asset_id != route.settlement_asset_id || bucket.status != VAULT_BRIDGE_BUCKET_STATUS_ACTIVE
        || bucket.impairment_factor_bps != 10_000 || bucket.counted_value_atoms < bucket.outstanding_vault_bridge_atoms {
        return Err(("pftl_source_not_par_backed", "settlement source must be active and fully counted".to_string()));
    }
    Ok(())
}

fn pftl_source_totals(ledger: &LedgerState, route_id: &str) -> Result<(u64,u64), (&'static str,String)> {
    ledger.pftl_uniswap_source_custody.iter().filter(|s| s.route_id == route_id)
        .try_fold((0u64,0u64), |(p,s),r| Ok((
            p.checked_add(r.principal_atoms).ok_or_else(|| ("pftl_source_overflow", "source principal overflow".to_string()))?,
            s.checked_add(r.spread_atoms).ok_or_else(|| ("pftl_source_overflow", "source spread overflow".to_string()))?)))
}

fn pftl_legacy_principal(ledger: &LedgerState, route: &PftlUniswapConsensusRouteState) -> Result<u64, (&'static str,String)> {
    route.settlement_reserve_atoms.checked_sub(pftl_source_totals(ledger,&route.route_id)?.0)
        .ok_or_else(|| ("pftl_source_accounting", "source principal exceeds aggregate reserve".to_string()))
}

fn pftl_source_reservation_index(ledger: &LedgerState, route_id: &str, reservation_id: &str) -> Option<usize> {
    ledger.pftl_uniswap_source_custody.iter().position(|s| s.route_id == route_id && s.reservation_escrows.contains_key(reservation_id))
}

fn pftl_source_reserve(ledger: &mut LedgerState, route: &PftlUniswapConsensusRouteState,
    op: &PftlUniswapOrderReserveOperation) -> Result<String, (&'static str,String)> {
    let Some(asset) = &op.settlement_source_asset_id else { return Ok(route.settlement_asset_id.clone()); };
    pftl_source_asset_check(ledger,route,asset)?;
    let row = ledger.pftl_uniswap_source_custody.iter_mut().find(|s| s.route_id == route.route_id && s.asset_id == *asset && s.enabled_for_issue)
        .ok_or_else(|| ("pftl_source_not_governed", "source is not enabled by the route issuer".to_string()))?;
    if row.reservation_escrows.insert(op.reservation_id.clone(),op.max_settlement_value_atoms).is_some() {
        return Err(("pftl_source_duplicate_reservation", "source reservation already exists".to_string()));
    }
    Ok(asset.clone())
}

fn pftl_source_subscription(ledger: &mut LedgerState, route: &PftlUniswapConsensusRouteState,
    reservation_id: &str, base: u64, spread: u64) -> Result<String, (&'static str,String)> {
    let Some(index) = pftl_source_reservation_index(ledger,&route.route_id,reservation_id) else { return Ok(route.settlement_asset_id.clone()); };
    let asset = ledger.pftl_uniswap_source_custody[index].asset_id.clone();
    pftl_source_asset_check(ledger,route,&asset)?;
    let row = &mut ledger.pftl_uniswap_source_custody[index];
    let held = row.reservation_escrows.remove(reservation_id).expect("source escrow checked");
    if base.checked_add(spread).filter(|due| *due <= held).is_none() {
        return Err(("pftl_source_escrow_underfunded", "source escrow is below the issue quote".to_string()));
    }
    row.principal_atoms = row.principal_atoms.checked_add(base).ok_or_else(|| ("pftl_source_overflow", "source principal overflow".to_string()))?;
    row.spread_atoms = row.spread_atoms.checked_add(spread).ok_or_else(|| ("pftl_source_overflow", "source spread overflow".to_string()))?;
    Ok(asset)
}

fn pftl_source_release(ledger: &mut LedgerState, route: &PftlUniswapConsensusRouteState,
    reservation_id: &str, amount: u64) -> Result<String, (&'static str,String)> {
    let Some(index) = pftl_source_reservation_index(ledger,&route.route_id,reservation_id) else { return Ok(route.settlement_asset_id.clone()); };
    let row = &mut ledger.pftl_uniswap_source_custody[index];
    if row.reservation_escrows.remove(reservation_id) != Some(amount) {
        return Err(("pftl_source_escrow_mismatch", "source release amount does not match escrow".to_string()));
    }
    Ok(row.asset_id.clone())
}

fn pftl_source_redeem(ledger: &mut LedgerState, route: &PftlUniswapConsensusRouteState,
    asset: Option<&str>, base: u64, spread: u64) -> Result<String, (&'static str,String)> {
    let Some(asset) = asset else {
        if pftl_legacy_principal(ledger,route)? < base {
            return Err(("pftl_legacy_reserve_unavailable", "pooled settlement cannot draw source-specific reserves".to_string()));
        }
        return Ok(route.settlement_asset_id.clone());
    };
    pftl_source_asset_check(ledger,route,asset)?;
    let row = ledger.pftl_uniswap_source_custody.iter_mut().find(|s| s.route_id == route.route_id && s.asset_id == asset)
        .ok_or_else(|| ("pftl_source_reserve_missing", "requested source has no route reserve".to_string()))?;
    row.principal_atoms = row.principal_atoms.checked_sub(base).ok_or_else(|| ("pftl_source_reserve_unavailable", "requested source cannot draw another source or pooled reserves".to_string()))?;
    row.spread_atoms = row.spread_atoms.checked_add(spread).ok_or_else(|| ("pftl_source_overflow", "source spread overflow".to_string()))?;
    Ok(asset.to_string())
}

fn pftl_source_govern(ledger: &mut LedgerState, route: &PftlUniswapConsensusRouteState,
    assets: Option<&Vec<String>>) -> Result<(), (&'static str,String)> {
    let Some(assets) = assets else { return Ok(()); };
    let mut unique = std::collections::BTreeSet::new();
    for asset in assets {
        if !unique.insert(asset) { return Err(("pftl_source_duplicate", "governed source set contains duplicates".to_string())); }
        pftl_source_asset_check(ledger,route,asset)?;
    }
    for row in ledger.pftl_uniswap_source_custody.iter_mut().filter(|s| s.route_id == route.route_id) {
        row.enabled_for_issue = unique.contains(&row.asset_id);
    }
    for asset in assets {
        if !ledger.pftl_uniswap_source_custody.iter().any(|s| s.route_id == route.route_id && s.asset_id == *asset) {
            if ledger.pftl_uniswap_source_custody.len() >= postfiat_types::MAX_PFTL_UNISWAP_ROUTE_ENTRIES {
                return Err(("pftl_source_capacity", "source custody exceeds the bounded entry limit".to_string()));
            }
            ledger.pftl_uniswap_source_custody.push(postfiat_types::PftlUniswapSourceCustody {
                route_id:route.route_id.clone(),asset_id:asset.clone(),enabled_for_issue:true,
                principal_atoms:0,spread_atoms:0,reservation_escrows:std::collections::BTreeMap::new(),
            });
        }
    }
    Ok(())
}
