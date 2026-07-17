fn apply_issued_payment(
    ledger: &mut LedgerState,
    operation: &IssuedPaymentOperation,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if operation.from == operation.to {
        return Err((
            "self_issued_payment",
            "issued_payment from and to must differ".to_string(),
        ));
    }
    let asset = ledger
        .asset_definition(&operation.asset_id)
        .cloned()
        .ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{}` does not exist", operation.asset_id),
            )
        })?;
    if asset.issuer != operation.issuer {
        return Err((
            "asset_issuer_mismatch",
            "issued_payment issuer does not match asset issuer".to_string(),
        ));
    }
    if operation.from == operation.issuer && operation.to == operation.issuer {
        return Err((
            "issuer_self_payment",
            "issuer cannot issue assets to itself".to_string(),
        ));
    }

    let from_index = if operation.from == operation.issuer {
        None
    } else {
        Some(
            trustline_index(ledger, &operation.from, &operation.asset_id).ok_or_else(|| {
                (
                    "missing_trustline",
                    "issued_payment sender has no trustline for asset".to_string(),
                )
            })?,
        )
    };
    let to_index = if operation.to == operation.issuer {
        None
    } else {
        Some(issued_asset_credit_recipient_line_index(
            ledger,
            &asset,
            &operation.to,
            operation.amount,
            "issued_payment",
        )?)
    };

    if let Some(index) = from_index {
        ensure_line_can_move(&asset, &ledger.trustlines[index])?;
        if ledger.trustlines[index].balance < operation.amount {
            return Err((
                "insufficient_issued_balance",
                "issued_payment amount exceeds sender trustline balance".to_string(),
            ));
        }
    }
    let recipient_credit = if let Some(index) = to_index {
        Some(prepare_issued_asset_credit(
            ledger,
            &asset,
            &operation.to,
            index,
            operation.amount,
            "issued_payment",
        )?)
    } else {
        None
    };
    if from_index.is_none() {
        if compatibility.bridge_verification_rules_active(block_height) {
            ensure_not_vault_bridge_out_of_lane_mint(
                ledger,
                &operation.asset_id,
                "issued_payment",
            )?;
        }
        let current_supply = issued_asset_supply(ledger, &operation.asset_id)?;
        let new_supply = current_supply
            .checked_add(operation.amount)
            .ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "issued asset supply would overflow".to_string(),
                )
            })?;
        if let Some(max_supply) = asset.max_supply {
            if new_supply > max_supply {
                return Err((
                    "issued_supply_cap_exceeded",
                    "issued_payment exceeds asset max_supply".to_string(),
                ));
            }
        }
    }

    if let Some(index) = from_index {
        ledger.trustlines[index].balance -= operation.amount;
    }
    if let Some(index) = to_index {
        if let Some((recipient_after, required_limit)) = recipient_credit {
            apply_prepared_issued_asset_credit(ledger, index, recipient_after, required_limit);
        }
    }
    Ok(())
}

pub fn credit_issued_asset_from_shielded_pool(
    ledger: &mut LedgerState,
    to: &str,
    asset_id: &str,
    amount: u64,
) -> Result<(), (&'static str, String)> {
    if amount == 0 {
        return Err((
            "zero_issued_credit",
            "shielded-pool issued asset credit amount must be nonzero".to_string(),
        ));
    }
    let asset = ledger.asset_definition(asset_id).cloned().ok_or_else(|| {
        (
            "missing_asset",
            format!("asset `{asset_id}` does not exist"),
        )
    })?;
    let to_index =
        issued_asset_credit_recipient_line_index(ledger, &asset, to, amount, "shielded-pool credit")?;
    let (recipient_after, required_limit) =
        prepare_issued_asset_credit(ledger, &asset, to, to_index, amount, "shielded-pool credit")?;
    apply_prepared_issued_asset_credit(ledger, to_index, recipient_after, required_limit);
    Ok(())
}

fn trustline_index(ledger: &LedgerState, account: &str, asset_id: &str) -> Option<usize> {
    ledger
        .trustlines
        .iter()
        .position(|line| line.account == account && line.asset_id == asset_id)
}

fn issued_asset_credit_recipient_line_index(
    ledger: &mut LedgerState,
    asset: &AssetDefinition,
    account: &str,
    amount: u64,
    context: &str,
) -> Result<usize, (&'static str, String)> {
    if let Some(index) = trustline_index(ledger, account, &asset.asset_id) {
        return Ok(index);
    }
    if account == asset.issuer {
        return Err((
            "issuer_credit_unsupported",
            format!("{context} cannot credit an issued asset to its issuer"),
        ));
    }
    if ledger.account(account).is_none() {
        return Err((
            "missing_recipient",
            format!("{context} recipient `{account}` does not exist"),
        ));
    }
    let mut line =
        TrustLine::new(account, asset.issuer.clone(), asset.asset_id.clone(), amount, 0).map_err(
            |message| ("invalid_implicit_issued_balance", message),
        )?;
    line.authorized = true;
    line.frozen = false;
    line.validate()
        .map_err(|message| ("invalid_implicit_issued_balance", message))?;
    ledger.trustlines.push(line);
    Ok(ledger.trustlines.len() - 1)
}

fn prepare_issued_asset_credit(
    ledger: &LedgerState,
    asset: &AssetDefinition,
    account: &str,
    line_index: usize,
    amount: u64,
    context: &str,
) -> Result<(u64, u64), (&'static str, String)> {
    ensure_line_can_move(asset, &ledger.trustlines[line_index])?;
    let recipient_after = ledger.trustlines[line_index]
        .balance
        .checked_add(amount)
        .ok_or_else(|| {
            (
                "issued_balance_overflow",
                format!("{context} recipient balance would overflow"),
            )
        })?;
    let reserved_escrows =
        issued_asset_reserved_total_for_account(ledger, account, &asset.asset_id, None, None)?;
    let required_limit = recipient_after
        .checked_add(reserved_escrows)
        .ok_or_else(|| {
            (
                "issued_balance_overflow",
                format!("{context} recipient balance and reservations would overflow"),
            )
        })?;
    Ok((recipient_after, required_limit))
}

fn apply_prepared_issued_asset_credit(
    ledger: &mut LedgerState,
    line_index: usize,
    recipient_after: u64,
    required_limit: u64,
) {
    if required_limit > ledger.trustlines[line_index].limit {
        ledger.trustlines[line_index].limit = required_limit;
    }
    ledger.trustlines[line_index].balance = recipient_after;
}

fn escrow_index(ledger: &LedgerState, escrow_id: &str) -> Option<usize> {
    ledger
        .escrows
        .iter()
        .position(|escrow| escrow.escrow_id == escrow_id)
}

fn nft_index(ledger: &LedgerState, nft_id: &str) -> Option<usize> {
    ledger.nfts.iter().position(|nft| nft.nft_id == nft_id)
}

fn offer_index(ledger: &LedgerState, offer_id: &str) -> Option<usize> {
    ledger
        .offers
        .iter()
        .position(|offer| offer.offer_id == offer_id)
}

fn lock_offer_reserve(
    ledger: &mut LedgerState,
    owner: &str,
    reserve: u64,
) -> Result<(), (&'static str, String)> {
    let account = ledger.account_mut(owner).ok_or_else(|| {
        (
            "missing_owner",
            format!("offer owner `{owner}` does not exist"),
        )
    })?;
    if account.balance < reserve {
        return Err((
            "insufficient_reserve",
            "offer owner balance is too low for reserve".to_string(),
        ));
    }
    let owner_after_reserve = account.balance - reserve;
    if let Some(message) = account_reserve_violation(owner, owner_after_reserve) {
        return Err(("below_account_reserve", message));
    }
    account.balance = owner_after_reserve;
    Ok(())
}

fn refund_offer_reserve(
    ledger: &mut LedgerState,
    owner: &str,
    reserve: u64,
) -> Result<(), (&'static str, String)> {
    if reserve == 0 {
        return Ok(());
    }
    let account = ledger.ensure_account(owner);
    account.balance = account.balance.checked_add(reserve).ok_or_else(|| {
        (
            "balance_overflow",
            "offer owner reserve refund would overflow".to_string(),
        )
    })?;
    Ok(())
}

fn lock_offer_sell_side(
    ledger: &mut LedgerState,
    owner: &str,
    asset_id: &str,
    amount: u64,
) -> Result<(), (&'static str, String)> {
    if asset_id == NATIVE_PFT_ESCROW_ASSET_ID {
        let account = ledger.account_mut(owner).ok_or_else(|| {
            (
                "missing_owner",
                format!("offer owner `{owner}` does not exist"),
            )
        })?;
        if account.balance < amount {
            return Err((
                "insufficient_funds",
                "offer owner balance is too low for sell-side PFT amount".to_string(),
            ));
        }
        let owner_after_lock = account.balance - amount;
        if let Some(message) = account_reserve_violation(owner, owner_after_lock) {
            return Err(("below_account_reserve", message));
        }
        account.balance = owner_after_lock;
        Ok(())
    } else {
        let asset = ledger.asset_definition(asset_id).cloned().ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{asset_id}` does not exist"),
            )
        })?;
        if owner == asset.issuer {
            return Err((
                "unsupported_issuer_offer",
                "issued-asset offers require holder trustlines".to_string(),
            ));
        }
        let owner_index = trustline_index(ledger, owner, asset_id).ok_or_else(|| {
            (
                "missing_trustline",
                "offer owner has no trustline for sell-side issued asset".to_string(),
            )
        })?;
        ensure_line_can_move(&asset, &ledger.trustlines[owner_index])?;
        if ledger.trustlines[owner_index].balance < amount {
            return Err((
                "insufficient_issued_balance",
                "offer sell-side amount exceeds owner trustline balance".to_string(),
            ));
        }
        ledger.trustlines[owner_index].balance -= amount;
        Ok(())
    }
}

fn refund_offer_sell_side(
    ledger: &mut LedgerState,
    owner: &str,
    asset_id: &str,
    amount: u64,
    excluded_offer_id: Option<&str>,
) -> Result<(), (&'static str, String)> {
    if amount == 0 {
        return Ok(());
    }
    if asset_id == NATIVE_PFT_ESCROW_ASSET_ID {
        let account = ledger.ensure_account(owner);
        account.balance = account.balance.checked_add(amount).ok_or_else(|| {
            (
                "balance_overflow",
                "offer owner PFT refund would overflow".to_string(),
            )
        })?;
        Ok(())
    } else {
        let owner_index = trustline_index(ledger, owner, asset_id).ok_or_else(|| {
            (
                "missing_trustline",
                "offer owner has no trustline for sell-side issued asset refund".to_string(),
            )
        })?;
        let owner_after = ledger.trustlines[owner_index]
            .balance
            .checked_add(amount)
            .ok_or_else(|| {
                (
                    "issued_balance_overflow",
                    "offer issued-asset refund would overflow".to_string(),
                )
            })?;
        let reserved_after_cancel = issued_asset_reserved_total_for_account(
            ledger,
            owner,
            asset_id,
            None,
            excluded_offer_id,
        )?;
        let required_limit = owner_after
            .checked_add(reserved_after_cancel)
            .ok_or_else(|| {
                (
                    "issued_balance_overflow",
                    "offer issued-asset refund reservations would overflow".to_string(),
                )
            })?;
        if required_limit > ledger.trustlines[owner_index].limit {
            return Err((
                "trustline_limit_exceeded",
                "offer issued-asset refund exceeds owner trustline limit".to_string(),
            ));
        }
        ledger.trustlines[owner_index].balance = owner_after;
        Ok(())
    }
}

fn credit_offer_received_asset(
    ledger: &mut LedgerState,
    account: &str,
    asset_id: &str,
    amount: u64,
) -> Result<(), (&'static str, String)> {
    if amount == 0 {
        return Ok(());
    }
    if asset_id == NATIVE_PFT_ESCROW_ASSET_ID {
        let account = ledger.ensure_account(account);
        account.balance = account.balance.checked_add(amount).ok_or_else(|| {
            (
                "balance_overflow",
                "offer PFT fill credit would overflow".to_string(),
            )
        })?;
        Ok(())
    } else {
        let asset = ledger.asset_definition(asset_id).cloned().ok_or_else(|| {
            (
                "missing_asset",
                format!("asset `{asset_id}` does not exist"),
            )
        })?;
        if account == asset.issuer {
            return Err((
                "unsupported_issuer_offer",
                "issued-asset offer fills require holder trustlines".to_string(),
            ));
        }
        let line_index = trustline_index(ledger, account, asset_id).ok_or_else(|| {
            (
                "missing_trustline",
                "offer fill recipient has no trustline for issued asset".to_string(),
            )
        })?;
        ensure_line_can_move(&asset, &ledger.trustlines[line_index])?;
        let balance_after = ledger.trustlines[line_index]
            .balance
            .checked_add(amount)
            .ok_or_else(|| {
                (
                    "issued_balance_overflow",
                    "offer issued-asset fill credit would overflow".to_string(),
                )
            })?;
        let reserved =
            issued_asset_reserved_total_for_account(ledger, account, asset_id, None, None)?;
        let required_limit = balance_after.checked_add(reserved).ok_or_else(|| {
            (
                "issued_balance_overflow",
                "offer issued-asset fill reservations would overflow".to_string(),
            )
        })?;
        if required_limit > ledger.trustlines[line_index].limit {
            return Err((
                "trustline_limit_exceeded",
                "offer fill issued amount exceeds trustline limit after reservations".to_string(),
            ));
        }
        ledger.trustlines[line_index].balance = balance_after;
        Ok(())
    }
}

fn ensure_offer_asset_participation(
    ledger: &LedgerState,
    owner: &str,
    asset_id: &str,
    sell_side: bool,
) -> Result<(), (&'static str, String)> {
    if asset_id == NATIVE_PFT_ESCROW_ASSET_ID {
        return Ok(());
    }
    let asset = ledger.asset_definition(asset_id).ok_or_else(|| {
        (
            "missing_asset",
            format!("asset `{asset_id}` does not exist"),
        )
    })?;
    if owner == asset.issuer {
        return Err((
            "unsupported_issuer_offer",
            "issued-asset offers require holder trustlines".to_string(),
        ));
    }
    let line_index = trustline_index(ledger, owner, asset_id).ok_or_else(|| {
        (
            "missing_trustline",
            if sell_side {
                "offer owner has no trustline for sell-side issued asset"
            } else {
                "offer owner has no trustline for buy-side issued asset"
            }
            .to_string(),
        )
    })?;
    ensure_line_can_move(asset, &ledger.trustlines[line_index])
}

fn ensure_issued_offer_receive_capacity(
    ledger: &LedgerState,
    owner: &str,
    asset_id: &str,
    amount: u64,
    excluded_offer_id: Option<&str>,
) -> Result<(), (&'static str, String)> {
    let line_index = trustline_index(ledger, owner, asset_id).ok_or_else(|| {
        (
            "missing_trustline",
            "offer owner has no trustline for buy-side issued asset".to_string(),
        )
    })?;
    let reserved =
        issued_asset_reserved_total_for_account(ledger, owner, asset_id, None, excluded_offer_id)?;
    let required_limit = ledger.trustlines[line_index]
        .balance
        .checked_add(reserved)
        .and_then(|value| value.checked_add(amount))
        .ok_or_else(|| {
            (
                "issued_balance_overflow",
                "offer buy-side issued-asset reservations would overflow".to_string(),
            )
        })?;
    if required_limit > ledger.trustlines[line_index].limit {
        return Err((
            "trustline_limit_exceeded",
            "offer buy-side issued amount exceeds owner trustline limit after reservations"
                .to_string(),
        ));
    }
    Ok(())
}

fn ensure_line_can_move(
    asset: &AssetDefinition,
    line: &TrustLine,
) -> Result<(), (&'static str, String)> {
    if asset.requires_authorization && !line.authorized {
        return Err((
            "missing_issuer_authorization",
            "trustline is not authorized by issuer".to_string(),
        ));
    }
    if line.frozen {
        return Err(("frozen_trustline", "trustline is frozen".to_string()));
    }
    Ok(())
}

pub fn issued_asset_supply(
    ledger: &LedgerState,
    asset_id: &str,
) -> Result<u64, (&'static str, String)> {
    assert_issued_supply_ledger_inventory_complete(ledger);
    let mut trustline_keys = std::collections::BTreeSet::new();
    for line in ledger
        .trustlines
        .iter()
        .filter(|line| line.asset_id == asset_id)
    {
        if !trustline_keys.insert((line.account.as_str(), line.asset_id.as_str())) {
            return Err((
                "issued_supply_duplicate_custody",
                format!(
                    "duplicate issued trustline for account `{}` and asset `{}`",
                    line.account, line.asset_id
                ),
            ));
        }
    }
    let mut escrow_ids = std::collections::BTreeSet::new();
    for escrow in ledger
        .escrows
        .iter()
        .filter(|escrow| escrow.asset_id == asset_id)
    {
        if !escrow_ids.insert(escrow.escrow_id.as_str()) {
            return Err((
                "issued_supply_duplicate_custody",
                format!("duplicate issued escrow `{}`", escrow.escrow_id),
            ));
        }
    }
    let mut offer_ids = std::collections::BTreeSet::new();
    for offer in ledger
        .offers
        .iter()
        .filter(|offer| offer.taker_gets_asset_id == asset_id)
    {
        if !offer_ids.insert(offer.offer_id.as_str()) {
            return Err((
                "issued_supply_duplicate_custody",
                format!("duplicate issued offer `{}`", offer.offer_id),
            ));
        }
    }
    let trustline_supply = ledger
        .trustlines
        .iter()
        .filter(|line| line.asset_id == asset_id)
        .try_fold(0_u64, |total, line| {
            total.checked_add(line.balance).ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "issued asset supply total overflowed".to_string(),
                )
            })
        })?;
    let open_escrow_supply = issued_asset_open_escrow_total(ledger, asset_id)?;
    let open_offer_supply = issued_asset_open_offer_locked_total(ledger, asset_id)?;
    let fast_lane_asset_id = postfiat_types::FastAssetIdV1(
        postfiat_crypto_provider::hex_to_bytes(asset_id)
            .map_err(|_| {
                (
                    "issued_supply_asset_id_invalid",
                    "issued asset id is not canonical hex".to_string(),
                )
            })?
            .try_into()
            .map_err(|_| {
                (
                    "issued_supply_asset_id_invalid",
                    "issued asset id does not fit the FastLane asset domain".to_string(),
                )
            })?,
    );
    let fast_lane_reserve_supply = ledger
        .fast_lane_reserves
        .iter()
        .filter(|reserve| reserve.asset_id == fast_lane_asset_id)
        .try_fold((0_u128, false), |(total, found), reserve| {
            if found {
                return Err((
                    "issued_supply_duplicate_custody",
                    format!("duplicate FastLane reserve for issued asset `{asset_id}`"),
                ));
            }
            total.checked_add(reserve.amount_atoms).ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "issued asset FastLane reserve total overflowed".to_string(),
                )
            }).map(|total| (total, true))
        })?
        .0;
    let fast_lane_reserve_supply = u64::try_from(fast_lane_reserve_supply).map_err(|_| {
        (
            "issued_supply_overflow",
            "issued asset FastLane reserve total exceeds u64".to_string(),
        )
    })?;
    let mut route_ids = std::collections::BTreeSet::new();
    let external_bridge_supply = ledger
        .pftl_uniswap_routes
        .iter()
        .filter(|route| route.native_nav_asset_id == asset_id)
        .try_fold(0_u64, |total, route| {
            if !route_ids.insert(route.route_id.as_str()) {
                return Err((
                    "issued_supply_duplicate_custody",
                    format!("duplicate PFTL-Uniswap route `{}`", route.route_id),
                ));
            }
            let route_external = route
                .outstanding_bridge_claims_atoms
                .checked_add(route.pending_return_import_claims_atoms)
                .and_then(|value| value.checked_add(route.ethereum_spendable_supply_atoms))
                .and_then(|value| value.checked_add(route.other_registered_venue_supply_atoms))
                .ok_or_else(|| {
                    (
                        "issued_supply_overflow",
                        format!(
                            "issued asset external bridge supply overflowed for route `{}`",
                            route.route_id
                        ),
                    )
                })?;
            total.checked_add(route_external).ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "issued asset external bridge supply total overflowed".to_string(),
                )
            })
        })?;
    trustline_supply
        .checked_add(open_escrow_supply)
        .and_then(|supply| supply.checked_add(open_offer_supply))
        .and_then(|supply| supply.checked_add(fast_lane_reserve_supply))
        .and_then(|supply| supply.checked_add(external_bridge_supply))
        .ok_or_else(|| {
            (
                "issued_supply_overflow",
                "issued asset supply total overflowed".to_string(),
            )
        })
}

fn assert_issued_supply_ledger_inventory_complete(ledger: &LedgerState) {
    // Compile-time exhaustiveness: adding a ledger lane requires an explicit
    // issued-supply classification here and at the global node+Orchard boundary.
    let LedgerState {
        accounts: _,
        asset_definitions: _,
        trustlines: _,
        escrows: _,
        nfts: _,
        offers: _,
        nav_assets: _,
        nav_reserve_packets: _,
        nav_redemptions: _,
        nav_proof_profiles: _,
        nav_attestors: _,
        market_ops_policies: _,
        market_ops_envelopes: _,
        vault_bridge_receipts: _,
        vault_bridge_bucket_states: _,
        vault_bridge_allocations: _,
        vault_bridge_redemptions: _,
        vault_bridge_deposits: _,
        pftl_uniswap_routes: _,
        pftl_uniswap_receipts: _,
        owned_objects: _,
        fastpay_recovery_policy: _,
        fastpay_recovery_committees: _,
        fastpay_recovery_reveals: _,
        fastpay_version_fences: _,
        fast_lane_reserves: _,
        fast_lane_deposit_receipts: _,
        redeemed_fast_lane_exit_claims: _,
        fast_lane_asset_rules: _,
        fast_lane_holder_permits: _,
        fastswap_policy_snapshots: _,
        fastswap_committees: _,
        fast_lane_prepare_fences: _,
        fast_lane_checkpoint_anchors: _,
        fastswap_activation_height: _,
        ethereum_arbitrum_finality_states: _,
    } = ledger;
}

fn issued_asset_open_offer_locked_total(
    ledger: &LedgerState,
    asset_id: &str,
) -> Result<u64, (&'static str, String)> {
    ledger
        .offers
        .iter()
        .filter(|offer| offer.taker_gets_asset_id == asset_id && offer.state == OFFER_STATE_OPEN)
        .try_fold(0_u64, |total, offer| {
            total
                .checked_add(offer.taker_gets_amount_remaining)
                .ok_or_else(|| {
                    (
                        "issued_supply_overflow",
                        "issued asset offer locked total overflowed".to_string(),
                    )
                })
        })
}

fn issued_asset_open_escrow_total(
    ledger: &LedgerState,
    asset_id: &str,
) -> Result<u64, (&'static str, String)> {
    ledger
        .escrows
        .iter()
        .filter(|escrow| escrow.asset_id == asset_id && escrow.state == ESCROW_STATE_OPEN)
        .try_fold(0_u64, |total, escrow| {
            total.checked_add(escrow.amount).ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "issued asset escrow total overflowed".to_string(),
                )
            })
        })
}

fn issued_asset_reserved_total_for_account(
    ledger: &LedgerState,
    account: &str,
    asset_id: &str,
    excluded_escrow_id: Option<&str>,
    excluded_offer_id: Option<&str>,
) -> Result<u64, (&'static str, String)> {
    let escrow_total = issued_asset_reserved_escrow_total_for_account(
        ledger,
        account,
        asset_id,
        excluded_escrow_id,
    )?;
    let offer_total = issued_asset_reserved_offer_total_for_account(
        ledger,
        account,
        asset_id,
        excluded_offer_id,
    )?;
    escrow_total.checked_add(offer_total).ok_or_else(|| {
        (
            "issued_supply_overflow",
            "issued asset reservation total overflowed".to_string(),
        )
    })
}

fn issued_asset_reserved_escrow_total_for_account(
    ledger: &LedgerState,
    account: &str,
    asset_id: &str,
    excluded_escrow_id: Option<&str>,
) -> Result<u64, (&'static str, String)> {
    ledger
        .escrows
        .iter()
        .filter(|escrow| {
            escrow.asset_id == asset_id
                && escrow.state == ESCROW_STATE_OPEN
                && excluded_escrow_id != Some(escrow.escrow_id.as_str())
                && (escrow.owner == account || escrow.recipient == account)
        })
        .try_fold(0_u64, |total, escrow| {
            total.checked_add(escrow.amount).ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "issued asset escrow reservation total overflowed".to_string(),
                )
            })
        })
}

fn issued_asset_reserved_offer_total_for_account(
    ledger: &LedgerState,
    account: &str,
    asset_id: &str,
    excluded_offer_id: Option<&str>,
) -> Result<u64, (&'static str, String)> {
    ledger
        .offers
        .iter()
        .filter(|offer| {
            offer.state == OFFER_STATE_OPEN
                && offer.owner == account
                && excluded_offer_id != Some(offer.offer_id.as_str())
        })
        .try_fold(0_u64, |total, offer| {
            let reserved = if offer.taker_gets_asset_id == asset_id {
                offer.taker_gets_amount_remaining
            } else if offer.taker_pays_asset_id == asset_id {
                offer.taker_pays_amount_remaining
            } else {
                0
            };
            total.checked_add(reserved).ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "issued asset offer reservation total overflowed".to_string(),
                )
            })
        })
}

fn account_reserve_violation(address: &str, balance: u64) -> Option<String> {
    let reserve = required_account_reserve(address);
    if balance < reserve {
        Some(format!(
            "account `{address}` final balance {balance} is below reserve {reserve}"
        ))
    } else {
        None
    }
}
