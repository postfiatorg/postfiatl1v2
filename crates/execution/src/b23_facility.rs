// Explicit policy opt-in keeps historical no-recipient routes replay-compatible.
fn b23_facility_enabled(route: &PftlUniswapConsensusRouteState) -> bool {
    route
        .v2
        .as_ref()
        .is_some_and(|v2| v2.primary_market_policy.spread_recipient.is_some())
}

fn b23_facility_fund_source(
    ledger: &mut LedgerState,
    route: &PftlUniswapConsensusRouteState,
    asset_id: &str,
    amount: u64,
) -> Result<(), (&'static str, String)> {
    if asset_id == route.settlement_asset_id {
        return Ok(());
    }
    pftl_source_asset_check(ledger, route, asset_id)?;
    let row = ledger
        .pftl_uniswap_source_custody
        .iter_mut()
        .find(|row| {
            row.route_id == route.route_id && row.asset_id == asset_id && row.enabled_for_issue
        })
        .ok_or_else(|| {
            (
                "pftl_source_not_governed",
                "funding source is not enabled by the route issuer".to_string(),
            )
        })?;
    row.principal_atoms = row.principal_atoms.checked_add(amount).ok_or_else(|| {
        (
            "pftl_source_overflow",
            "funding source principal would overflow".to_string(),
        )
    })?;
    Ok(())
}

fn b23_facility_transfer_offer_inventory(
    ledger: &mut LedgerState,
    asset_id: &str,
    from: &str,
    to: &str,
    amount: u64,
) -> Result<(), (&'static str, String)> {
    if let Some(index) = b23_facility_unique_route_index(ledger, asset_id)? {
        let route = &mut ledger.pftl_uniswap_routes[index];
        // Offer custody stays attributed to its owner until filled. Cancel and
        // expiration only return that custody, so neither moves route inventory.
        pftl_uniswap_debit_native_route_balance(route, from, amount)?;
        pftl_uniswap_credit_native_route_balance(route, to, amount)?;
        route
            .validate()
            .map_err(|e| ("bad_pftl_uniswap_route", e))?;
    }
    Ok(())
}

fn b23_facility_credit_minted_inventory(
    ledger: &mut LedgerState,
    operation: &postfiat_types::NavMintAtNavOperation,
) -> Result<(), (&'static str, String)> {
    if let Some(index) = b23_facility_unique_route_index(ledger, &operation.asset_id)? {
        let route = &mut ledger.pftl_uniswap_routes[index];
        route.authorized_valid_supply_atoms = route
            .authorized_valid_supply_atoms
            .checked_add(operation.amount)
            .ok_or_else(|| {
                (
                    "pftl_uniswap_supply_overflow",
                    "minted route supply would overflow".to_string(),
                )
            })?;
        route.pftl_spendable_supply_atoms = route
            .pftl_spendable_supply_atoms
            .checked_add(operation.amount)
            .ok_or_else(|| {
                (
                    "pftl_uniswap_supply_overflow",
                    "minted native route supply would overflow".to_string(),
                )
            })?;
        pftl_uniswap_credit_native_route_balance(route, &operation.to, operation.amount)?;
        route
            .validate()
            .map_err(|e| ("bad_pftl_uniswap_route", e))?;
    }
    Ok(())
}

fn b23_facility_unique_route_index(
    ledger: &LedgerState,
    asset_id: &str,
) -> Result<Option<usize>, (&'static str, String)> {
    let mut matching = ledger
        .pftl_uniswap_routes
        .iter()
        .enumerate()
        .filter(|(_, route)| route.native_nav_asset_id == asset_id && b23_facility_enabled(route));
    let index = matching.next().map(|(index, _)| index);
    if matching.next().is_some() {
        return Err((
            "multiple_pftl_uniswap_facility_routes",
            "a native asset may have only one opted-in facility route".to_string(),
        ));
    }
    Ok(index)
}

fn b23_facility_require_unique_route(
    ledger: &LedgerState,
    asset_id: &str,
    route_id: &str,
) -> Result<(), (&'static str, String)> {
    if ledger.pftl_uniswap_routes.iter().any(|route| {
        route.native_nav_asset_id == asset_id
            && route.route_id != route_id
            && b23_facility_enabled(route)
    }) {
        return Err((
            "multiple_pftl_uniswap_facility_routes",
            "a native asset may have only one opted-in facility route".to_string(),
        ));
    }
    Ok(())
}

fn b23_facility_require_aligned_inventory(
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
) -> Result<(), (&'static str, String)> {
    // Opt-in is deliberately strict: all native custody must first be on public
    // trust lines. Cancel outstanding sell offers/escrows before activating.
    let mut positions = std::collections::BTreeMap::new();
    for line in ledger
        .trustlines
        .iter()
        .filter(|line| line.asset_id == route.native_nav_asset_id && line.balance > 0)
    {
        if positions
            .insert(line.account.clone(), line.balance)
            .is_some()
        {
            return Err((
                "pftl_uniswap_inventory_mismatch",
                "duplicate holder trust lines".to_string(),
            ));
        }
    }
    if positions != route.native_spendable_balances_atoms
        || issued_asset_supply(ledger, &route.native_nav_asset_id)?
            != route.pftl_spendable_supply_atoms
    {
        return Err(("pftl_uniswap_inventory_mismatch", "opt-in requires exact public holder/route inventory equality and no untracked native custody".to_string()));
    }
    Ok(())
}
