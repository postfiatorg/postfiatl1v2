/// The covered custody bucket of a Solana-source series. First release: the revision-8 series
/// only. Second release: also a series whose source bucket belongs to the shared custody and
/// derives this series ID.
pub fn solana_exit_series_bucket(
    chain_id: &str,
    ledger: &LedgerState,
    asset_id: &str,
    second: bool,
) -> Option<String> {
    use postfiat_types::solana_exit::{matches_bucket, BUCKET, FAMILY, SERIES};
    if asset_id == SERIES {
        return Some(BUCKET.into());
    }
    if !second {
        return None;
    }
    let asset = ledger.asset_definition(asset_id)?;
    let bucket = ledger
        .vault_bridge_bucket(&asset.source_bucket_id)
        .filter(|b| matches_bucket(b, true))?;
    (asset_id == postfiat_types::solana_source_series_id(chain_id, FAMILY, &bucket.bucket_id))
        .then(|| bucket.bucket_id.clone())
}

/// `solana_exit_series_bucket` with the second release decided at `height`. The genesis is
/// hashed only for a series the second release could cover and that is not the revision-8 one.
pub fn solana_exit_series_bucket_at(
    genesis: &Genesis,
    ledger: &LedgerState,
    asset_id: &str,
    height: u64,
) -> Option<String> {
    let bucket = solana_exit_series_bucket(&genesis.chain_id, ledger, asset_id, true)?;
    (asset_id == postfiat_types::solana_exit::SERIES
        || postfiat_types::solana_exit::second_release_active(
            &genesis.chain_id,
            &genesis_hash(genesis),
            height,
        ))
    .then_some(bucket)
}

/// Live claims for one source series of the shared custody, including the Orchard turnstile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolanaSeriesClaims {
    pub bucket_id: String,
    pub series_id: String,
    pub supply: u64,
    pub outstanding: u64,
    counted: u64,
}

/// Live claims of the covered buckets: the revision-8 series under the first release (with
/// its exact receipts), every covered bucket of the shared custody under the second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolanaExitAccounting {
    pub buckets: Vec<SolanaSeriesClaims>,
    second: bool,
}

impl SolanaExitAccounting {
    pub fn read(
        chain_id: &str,
        ledger: &LedgerState,
        orchard: &[AssetOrchardAssetBalance],
        second: bool,
    ) -> Result<Self, (&'static str, String)> {
        use postfiat_types::solana_exit::{matches_bucket, BUCKET, FAMILY, SERIES};
        let mut ids = if second {
            ledger
                .vault_bridge_bucket_states
                .iter()
                .filter(|b| matches_bucket(b, true))
                .map(|b| b.bucket_id.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if !ids.iter().any(|id| id == BUCKET) {
            ids.push(BUCKET.into());
        }
        ids.sort();
        ids.dedup();
        let mut buckets = Vec::with_capacity(ids.len());
        for bucket_id in ids {
            let series_id = if second {
                postfiat_types::solana_source_series_id(chain_id, FAMILY, &bucket_id)
            } else {
                SERIES.to_string()
            };
            let mut balances = orchard.iter().filter(|balance| balance.asset_id == series_id);
            let orchard_supply = balances.next().map_or(0, |balance| balance.live_total);
            if balances.next().is_some() {
                return Err((
                    "issued_supply_duplicate_custody",
                    if second {
                        format!("duplicate Orchard balance for Solana series {series_id}")
                    } else {
                        "duplicate revision-8 Orchard balance".into()
                    },
                ));
            }
            let supply = issued_asset_supply(ledger, &series_id)?
                .checked_add(orchard_supply)
                .ok_or_else(|| {
                    (
                        "issued_supply_overflow",
                        if second {
                            format!("Solana series {series_id} supply overflows u64")
                        } else {
                            "revision-8 supply overflows u64".into()
                        },
                    )
                })?;
            let bucket = ledger.vault_bridge_bucket(&bucket_id);
            buckets.push(SolanaSeriesClaims {
                outstanding: bucket.map_or(0, |b| b.outstanding_vault_bridge_atoms),
                counted: bucket.map_or(0, |b| b.counted_value_atoms),
                bucket_id,
                series_id,
                supply,
            });
        }
        Ok(Self { buckets, second })
    }

    pub fn bucket(&self, bucket_id: &str) -> Option<&SolanaSeriesClaims> {
        self.buckets.iter().find(|b| b.bucket_id == bucket_id)
    }

    /// Adds a paired Orchard credit of `amount` to the series `series_id`.
    pub fn credit(&mut self, series_id: &str, amount: u64) -> Result<(), (&'static str, String)> {
        if amount == 0 {
            return Ok(());
        }
        let claims = self
            .buckets
            .iter_mut()
            .find(|b| b.series_id == series_id)
            .ok_or_else(|| {
                (
                    "solana_exit_unmatched_supply_change",
                    format!("paired credit names no covered Solana series {series_id}"),
                )
            })?;
        let second = self.second;
        claims.supply = claims.supply.checked_add(amount).ok_or_else(|| {
            (
                "issued_supply_overflow",
                if second {
                    format!("Solana series {series_id} ingress supply overflows")
                } else {
                    "revision-8 ingress supply overflows".into()
                },
            )
        })?;
        Ok(())
    }

    /// Every covered bucket's series supply equals its outstanding.
    pub fn require_reconciled(&self) -> Result<(), (&'static str, String)> {
        self.buckets
            .iter()
            .try_for_each(|b| self.require_bucket_reconciled(&b.bucket_id))
    }

    pub fn require_bucket_reconciled(&self, bucket_id: &str) -> Result<(), (&'static str, String)> {
        let claims = self.bucket(bucket_id).ok_or_else(|| {
            (
                "solana_exit_supply_mismatch",
                format!("bucket {bucket_id} is not a covered Solana custody bucket"),
            )
        })?;
        if claims.supply != claims.outstanding {
            return Err((
                "solana_exit_supply_mismatch",
                if self.second {
                    format!(
                        "Solana series supply {} differs from bucket outstanding {} in bucket {}",
                        claims.supply, claims.outstanding, claims.bucket_id,
                    )
                } else {
                    format!(
                        "revision-8 series supply {} differs from bucket outstanding {}",
                        claims.supply, claims.outstanding,
                    )
                },
            ));
        }
        Ok(())
    }

    /// In every covered bucket the supply change equals the outstanding change, and an
    /// increase is matched by counted backing. A bucket new in `after` starts from zero.
    pub fn verify_change(&self, after: &Self) -> Result<(), (&'static str, String)> {
        let zero = |b: &SolanaSeriesClaims| SolanaSeriesClaims {
            supply: 0,
            outstanding: 0,
            counted: 0,
            ..b.clone()
        };
        for later in &after.buckets {
            let earlier = self
                .bucket(&later.bucket_id)
                .cloned()
                .unwrap_or_else(|| zero(later));
            let supply = i128::from(later.supply) - i128::from(earlier.supply);
            let outstanding = i128::from(later.outstanding) - i128::from(earlier.outstanding);
            let counted = i128::from(later.counted) - i128::from(earlier.counted);
            if supply != outstanding || (supply > 0 && counted < supply) {
                return Err(("solana_exit_unmatched_supply_change", if self.second { format!(
                    "Solana series supply change {supply} in bucket {} requires matching outstanding and counted backing (outstanding change {outstanding}, counted change {counted})",
                    later.bucket_id,
                ) } else { format!(
                    "revision-8 supply change {supply} requires matching outstanding and counted backing (outstanding change {outstanding}, counted change {counted})"
                ) }));
            }
        }
        if let Some(gone) = self.buckets.iter().find(|b| after.bucket(&b.bucket_id).is_none()) {
            return Err((
                "solana_exit_unmatched_supply_change",
                format!("covered Solana bucket {} disappeared", gone.bucket_id),
            ));
        }
        Ok(())
    }
}

fn apply_issued_payment(
    genesis: &Genesis,
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

    if operation.from == operation.issuer
        && postfiat_types::solana_exit::release_height(&genesis.chain_id, &genesis_hash(genesis))
            .is_some_and(|h| block_height >= h)
        && solana_exit_series_bucket_at(genesis, ledger, &operation.asset_id, block_height)
            .is_some()
    {
        let second = postfiat_types::solana_exit::second_release_active(
            &genesis.chain_id,
            &genesis_hash(genesis),
            block_height,
        );
        return Err((
            "solana_exit_issuer_payment_disabled",
            if second {
                "Solana-source issuer payments cannot create redeemable claims"
            } else {
                "revision-8 issuer payments cannot create redeemable claims"
            }
            .into(),
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

    if b23_facility_unique_route_index(ledger, &operation.asset_id)?.is_some() {
        if operation.from == operation.issuer || operation.to == operation.issuer {
            return Err(("pftl_uniswap_issuer_payment_disabled", "opted-in NAV assets require NAV mint or primary redemption for issuer-sided transfers".to_string()));
        }
        b23_facility_transfer_offer_inventory(ledger, &operation.asset_id, &operation.from, &operation.to, operation.amount)?;
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
    issued_asset_supply_with_non_nav_spread(ledger, asset_id, true)
}

/// Reproduces the pre-AR-11 archive supply calculation while retaining the
/// spread in committed route state. Production admission must use
/// [`issued_asset_supply`].
pub fn issued_asset_supply_legacy_non_nav_spread_omitted(
    ledger: &LedgerState,
    asset_id: &str,
) -> Result<u64, (&'static str, String)> {
    issued_asset_supply_with_non_nav_spread(ledger, asset_id, false)
}

fn issued_asset_supply_with_non_nav_spread(
    ledger: &LedgerState,
    asset_id: &str,
    include_non_nav_spread: bool,
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
    let pftl_uniswap_route_custody_supply = ledger
        .pftl_uniswap_routes
        .iter()
        .filter(|route| {
            route.native_nav_asset_id == asset_id || route.settlement_asset_id == asset_id
        })
        .try_fold(0_u64, |total, route| {
            if !route_ids.insert(route.route_id.as_str()) {
                return Err((
                    "issued_supply_duplicate_custody",
                    format!("duplicate PFTL-Uniswap route `{}`", route.route_id),
                ));
            }
            let native_external = if route.native_nav_asset_id == asset_id {
                route
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
                    })?
            } else {
                0
            };
            let settlement_reserve = if route.settlement_asset_id == asset_id {
                // Accumulated non-NAV spread is live settlement-asset custody
                // alongside the subscription-funded reserve (AR-11).
                let spread = if include_non_nav_spread {
                    route
                        .v2
                        .as_ref()
                        .map(|v2| v2.non_nav_spread_atoms)
                        .unwrap_or(0)
                } else {
                    0
                };
                let (source_principal, source_spread) = pftl_source_totals(ledger, &route.route_id)?;
                route
                    .settlement_reserve_atoms
                    .checked_sub(source_principal)
                    .and_then(|principal| principal.checked_add(spread))
                    .and_then(|custody| custody.checked_sub(if include_non_nav_spread { source_spread } else { 0 }))
                    .ok_or_else(|| {
                        (
                            "issued_supply_overflow",
                            format!(
                                "issued asset settlement custody overflowed for route `{}`",
                                route.route_id
                            ),
                        )
                    })?
            } else {
                0
            };
            let route_custody =
                native_external
                    .checked_add(settlement_reserve)
                    .ok_or_else(|| {
                        (
                            "issued_supply_overflow",
                            format!(
                                "issued asset route custody overflowed for route `{}`",
                                route.route_id
                            ),
                        )
                    })?;
            total.checked_add(route_custody).ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "issued asset PFTL-Uniswap route custody total overflowed".to_string(),
                )
            })
        })?;
    let source_custody = ledger.pftl_uniswap_source_custody.iter().filter(|row| row.asset_id == asset_id)
        .try_fold(0u64, |total,row| {
            let reserved = row.reservation_escrows.values().try_fold(0u64, |sum,v| sum.checked_add(*v))
                .ok_or_else(|| ("issued_supply_overflow", "source reservation custody overflow".to_string()))?;
            total.checked_add(row.principal_atoms)
                .and_then(|n| n.checked_add(if include_non_nav_spread { row.spread_atoms } else { 0 }))
                .and_then(|n| n.checked_add(reserved))
                .ok_or_else(|| ("issued_supply_overflow", "source custody overflow".to_string()))
        })?;
    trustline_supply
        .checked_add(ledger.collected_private_fees.get(asset_id).copied().unwrap_or(0))
        .and_then(|supply| supply.checked_add(source_custody))
        .and_then(|supply| supply.checked_add(open_escrow_supply))
        .and_then(|supply| supply.checked_add(open_offer_supply))
        .and_then(|supply| supply.checked_add(fast_lane_reserve_supply))
        .and_then(|supply| supply.checked_add(pftl_uniswap_route_custody_supply))
        .ok_or_else(|| {
            (
                "issued_supply_overflow",
                "issued asset supply total overflowed".to_string(),
            )
        })
}

/// Complete non-Orchard supply for a display family. Ordinary assets count
/// themselves; source-series assets count only when their immutable
/// `asset_family_id` matches. This deliberately rejects a source-series ID as
/// the family input so callers cannot accidentally omit sibling series.
pub fn issued_asset_family_supply(
    ledger: &LedgerState,
    asset_family_id: &str,
) -> Result<u64, (&'static str, String)> {
    issued_asset_family_supply_with_non_nav_spread(ledger, asset_family_id, true)
}

fn issued_asset_family_supply_with_non_nav_spread(
    ledger: &LedgerState,
    asset_family_id: &str,
    include_non_nav_spread: bool,
) -> Result<u64, (&'static str, String)> {
    let family = ledger.asset_definition(asset_family_id).ok_or_else(|| {
        (
            "missing_asset",
            format!("asset family `{asset_family_id}` does not exist"),
        )
    })?;
    if !family.asset_family_id.is_empty() || !family.source_series_id.is_empty() {
        return Err((
            "invalid_asset_family",
            "issued asset family supply requires an ordinary family asset ID".to_string(),
        ));
    }
    let mut ids = vec![asset_family_id.to_string()];
    ids.extend(
        ledger
            .asset_definitions
            .iter()
            .filter(|asset| asset.asset_family_id == asset_family_id)
            .map(|asset| asset.asset_id.clone()),
    );
    ids.sort();
    ids.dedup();
    ids.into_iter().try_fold(0_u64, |total, asset_id| {
        total
            .checked_add(issued_asset_supply_with_non_nav_spread(
                ledger,
                &asset_id,
                include_non_nav_spread,
            )?)
            .ok_or_else(|| {
                (
                    "issued_supply_overflow",
                    "issued asset family supply total overflowed".to_string(),
                )
            })
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
        // Target receipts carry no asset balances or reserve authority.
        yolo_target_registrations: _,
        yolo_target_receipts: _,
        nav_attestors: _,
        market_ops_policies: _,
        market_ops_envelopes: _,
        fx_fix_states: _,
        fx_fix_reservations: _,
        vault_bridge_receipts: _,
        vault_bridge_bucket_states: _,
        vault_bridge_allocations: _,
        vault_bridge_redemptions: _,
        vault_bridge_deposits: _,
        collected_private_fees: _,
        pftl_uniswap_source_custody: _, // counted by the source-specific custody sum
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
        fast_ingress_campaigns: _,
        arc_finality_states: _,
        external_lanes: _, // native PFT reserve, never issued-asset custody
        beacon_finality_states: _,
        solana_route: _,
        additional_solana_routes: _,
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

/// Calculate the replacement balance of an existing holder line. Keep the
/// issuer, line limit, reservations and authorization untouched on either edge.
pub fn prepare_pq_line_balance(
    ledger: &LedgerState,
    asset: &AssetDefinition,
    holder: &str,
    amount: u64,
    credit: bool,
) -> Result<(usize, u64), &'static str> {
    if holder == asset.issuer
        || ledger
            .account(holder)
            .is_none_or(|account| account.balance < required_account_reserve(holder))
    {
        return Err("pq_ineligible_holder");
    }
    let index = trustline_index(ledger, holder, &asset.asset_id).ok_or("pq_missing_trustline")?;
    let line = &ledger.trustlines[index];
    if line.issuer != asset.issuer {
        return Err("pq_line_issuer");
    }
    ensure_line_can_move(asset, line).map_err(|(code, _)| code)?;
    let balance = if credit {
        let (balance, required) =
            prepare_issued_asset_credit(ledger, asset, holder, index, amount, "PQ exit")
                .map_err(|(code, _)| code)?;
        if required > line.limit {
            return Err("pq_trustline_limit");
        }
        balance
    } else {
        line.balance
            .checked_sub(amount)
            .ok_or("pq_insufficient_issued_balance")?
    };
    Ok((index, balance))
}
