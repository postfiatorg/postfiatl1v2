pub fn required_account_reserve(address: &str) -> u64 {
    if address == FEE_COLLECTOR_ADDRESS {
        0
    } else {
        ACCOUNT_RESERVE
    }
}

pub fn transfer_weight_bytes(transfer: &SignedTransfer) -> usize {
    transfer
        .unsigned
        .signing_bytes()
        .len()
        .saturating_add(transfer.algorithm_id.len())
        .saturating_add(transfer.public_key_hex.len())
        .saturating_add(transfer.signature_hex.len())
}

pub fn minimum_transfer_fee(transfer: &SignedTransfer) -> u64 {
    let quanta = transfer_weight_bytes(transfer).div_ceil(TRANSFER_FEE_BYTE_QUANTUM);
    let byte_fee = u64::try_from(quanta)
        .unwrap_or(u64::MAX)
        .saturating_mul(TRANSFER_FEE_PER_QUANTUM);
    MIN_TRANSFER_FEE.max(byte_fee)
}

pub fn transfer_state_expansion_fee(ledger: &LedgerState, transfer: &SignedTransfer) -> u64 {
    if transfer.unsigned.to != transfer.unsigned.from
        && ledger.account(&transfer.unsigned.to).is_none()
    {
        TRANSFER_ACCOUNT_CREATION_FEE
    } else {
        0
    }
}

pub fn minimum_transfer_fee_for_ledger(ledger: &LedgerState, transfer: &SignedTransfer) -> u64 {
    minimum_transfer_fee(transfer).saturating_add(transfer_state_expansion_fee(ledger, transfer))
}

pub fn payment_v2_weight_bytes(payment: &SignedPaymentV2) -> usize {
    payment.tx_id_preimage_bytes().len()
}

pub fn minimum_payment_v2_fee(payment: &SignedPaymentV2) -> u64 {
    let quanta = payment_v2_weight_bytes(payment).div_ceil(TRANSFER_FEE_BYTE_QUANTUM);
    let byte_fee = u64::try_from(quanta)
        .unwrap_or(u64::MAX)
        .saturating_mul(TRANSFER_FEE_PER_QUANTUM);
    MIN_TRANSFER_FEE.max(byte_fee)
}

pub fn payment_v2_state_expansion_fee(ledger: &LedgerState, payment: &SignedPaymentV2) -> u64 {
    if payment.unsigned.to != payment.unsigned.from
        && ledger.account(&payment.unsigned.to).is_none()
    {
        TRANSFER_ACCOUNT_CREATION_FEE
    } else {
        0
    }
}

pub fn minimum_payment_v2_fee_for_ledger(ledger: &LedgerState, payment: &SignedPaymentV2) -> u64 {
    minimum_payment_v2_fee(payment).saturating_add(payment_v2_state_expansion_fee(ledger, payment))
}

pub fn asset_transaction_weight_bytes(transaction: &SignedAssetTransaction) -> usize {
    transaction.tx_id_preimage_bytes().len()
}

pub fn minimum_asset_transaction_fee(transaction: &SignedAssetTransaction) -> u64 {
    let quanta = asset_transaction_weight_bytes(transaction).div_ceil(TRANSFER_FEE_BYTE_QUANTUM);
    let byte_fee = u64::try_from(quanta)
        .unwrap_or(u64::MAX)
        .saturating_mul(TRANSFER_FEE_PER_QUANTUM);
    MIN_TRANSFER_FEE.max(byte_fee)
}

pub fn asset_transaction_state_expansion_fee(
    ledger: &LedgerState,
    transaction: &SignedAssetTransaction,
) -> u64 {
    match &transaction.unsigned.operation {
        AssetTransactionOperation::AssetCreate(operation) => {
            let asset_id = match postfiat_types::issued_asset_id(
                &transaction.unsigned.chain_id,
                &operation.issuer,
                &operation.code,
                operation.version,
            ) {
                Ok(asset_id) => asset_id,
                Err(_) => return 0,
            };
            if ledger.asset_definition(&asset_id).is_none() {
                ASSET_DEFINITION_STATE_EXPANSION_FEE
            } else {
                0
            }
        }
        AssetTransactionOperation::TrustSet(operation)
            if transaction.unsigned.source == operation.account
                && ledger
                    .trustline_for_account_asset(&operation.account, &operation.asset_id)
                    .is_none() =>
        {
            operation.reserve_paid.max(TRUSTLINE_STATE_EXPANSION_FEE)
        }
        AssetTransactionOperation::NavAssetRegister(operation)
            if ledger.nav_asset(&operation.asset_id).is_none() =>
        {
            ASSET_DEFINITION_STATE_EXPANSION_FEE
        }
        AssetTransactionOperation::NavReserveSubmit(operation)
            if ledger
                .nav_reserve_packet(
                    &operation.asset_id,
                    operation.epoch,
                    &operation.reserve_packet_hash,
                )
                .is_none() =>
        {
            TRUSTLINE_STATE_EXPANSION_FEE
        }
        AssetTransactionOperation::NavRedeemAtNav(_) => TRUSTLINE_STATE_EXPANSION_FEE,
        AssetTransactionOperation::VaultBridgeDepositClaim(_) => {
            TRUSTLINE_STATE_EXPANSION_FEE.saturating_mul(3)
        }
        _ => 0,
    }
}

pub fn minimum_asset_transaction_fee_for_ledger(
    ledger: &LedgerState,
    transaction: &SignedAssetTransaction,
) -> u64 {
    minimum_asset_transaction_fee(transaction)
        .saturating_add(asset_transaction_state_expansion_fee(ledger, transaction))
}

pub fn atomic_swap_transaction_weight_bytes(transaction: &SignedAtomicSwapTransaction) -> usize {
    transaction.tx_id_preimage_bytes().len()
}

pub fn minimum_atomic_swap_fee(transaction: &SignedAtomicSwapTransaction) -> u64 {
    let quanta =
        atomic_swap_transaction_weight_bytes(transaction).div_ceil(TRANSFER_FEE_BYTE_QUANTUM);
    let byte_fee = u64::try_from(quanta)
        .unwrap_or(u64::MAX)
        .saturating_mul(TRANSFER_FEE_PER_QUANTUM);
    MIN_TRANSFER_FEE.max(byte_fee)
}

pub fn atomic_swap_leg_state_expansion_fee(
    ledger: &LedgerState,
    leg: &AtomicSwapLeg,
) -> u64 {
    if leg.recipient != leg.issuer
        && ledger
            .trustline_for_account_asset(&leg.recipient, &leg.asset_id)
            .is_none()
    {
        TRUSTLINE_STATE_EXPANSION_FEE
    } else {
        0
    }
}

pub fn minimum_atomic_swap_leg_fee_for_ledger(
    ledger: &LedgerState,
    transaction: &SignedAtomicSwapTransaction,
    leg: &AtomicSwapLeg,
) -> u64 {
    minimum_atomic_swap_fee(transaction)
        .saturating_add(atomic_swap_leg_state_expansion_fee(ledger, leg))
}

pub fn escrow_transaction_weight_bytes(transaction: &SignedEscrowTransaction) -> usize {
    transaction.tx_id_preimage_bytes().len()
}

pub fn minimum_escrow_transaction_fee(transaction: &SignedEscrowTransaction) -> u64 {
    let quanta = escrow_transaction_weight_bytes(transaction).div_ceil(TRANSFER_FEE_BYTE_QUANTUM);
    let byte_fee = u64::try_from(quanta)
        .unwrap_or(u64::MAX)
        .saturating_mul(TRANSFER_FEE_PER_QUANTUM);
    MIN_TRANSFER_FEE.max(byte_fee)
}

pub fn escrow_transaction_state_expansion_fee(
    ledger: &LedgerState,
    transaction: &SignedEscrowTransaction,
) -> u64 {
    match &transaction.unsigned.operation {
        EscrowTransactionOperation::EscrowCreate(operation) => {
            let escrow_id = match escrow_id(
                &transaction.unsigned.chain_id,
                &operation.owner,
                transaction.unsigned.sequence,
            ) {
                Ok(escrow_id) => escrow_id,
                Err(_) => return 0,
            };
            if ledger.escrow(&escrow_id).is_none() {
                ESCROW_STATE_EXPANSION_FEE
            } else {
                0
            }
        }
        _ => 0,
    }
}

pub fn minimum_escrow_transaction_fee_for_ledger(
    ledger: &LedgerState,
    transaction: &SignedEscrowTransaction,
) -> u64 {
    minimum_escrow_transaction_fee(transaction)
        .saturating_add(escrow_transaction_state_expansion_fee(ledger, transaction))
}

pub fn nft_transaction_weight_bytes(transaction: &SignedNftTransaction) -> usize {
    transaction.tx_id_preimage_bytes().len()
}

pub fn minimum_nft_transaction_fee(transaction: &SignedNftTransaction) -> u64 {
    let quanta = nft_transaction_weight_bytes(transaction).div_ceil(TRANSFER_FEE_BYTE_QUANTUM);
    let byte_fee = u64::try_from(quanta)
        .unwrap_or(u64::MAX)
        .saturating_mul(TRANSFER_FEE_PER_QUANTUM);
    MIN_TRANSFER_FEE.max(byte_fee)
}

pub fn nft_transaction_state_expansion_fee(
    ledger: &LedgerState,
    transaction: &SignedNftTransaction,
) -> u64 {
    match &transaction.unsigned.operation {
        NftTransactionOperation::NftMint(operation) => {
            let nft_id = match nft_id(
                &transaction.unsigned.chain_id,
                &operation.issuer,
                &operation.collection_id,
                operation.serial,
            ) {
                Ok(nft_id) => nft_id,
                Err(_) => return 0,
            };
            if ledger.nft(&nft_id).is_none() {
                NFT_STATE_EXPANSION_FEE
            } else {
                0
            }
        }
        _ => 0,
    }
}

pub fn minimum_nft_transaction_fee_for_ledger(
    ledger: &LedgerState,
    transaction: &SignedNftTransaction,
) -> u64 {
    minimum_nft_transaction_fee(transaction)
        .saturating_add(nft_transaction_state_expansion_fee(ledger, transaction))
}

pub fn offer_transaction_weight_bytes(transaction: &SignedOfferTransaction) -> usize {
    transaction.tx_id_preimage_bytes().len()
}

pub fn minimum_offer_transaction_fee(transaction: &SignedOfferTransaction) -> u64 {
    let quanta = offer_transaction_weight_bytes(transaction).div_ceil(TRANSFER_FEE_BYTE_QUANTUM);
    let byte_fee = u64::try_from(quanta)
        .unwrap_or(u64::MAX)
        .saturating_mul(TRANSFER_FEE_PER_QUANTUM);
    MIN_TRANSFER_FEE.max(byte_fee)
}

pub fn offer_transaction_state_expansion_fee(
    ledger: &LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
) -> u64 {
    match &transaction.unsigned.operation {
        OfferTransactionOperation::OfferCreate(operation) => {
            let offer_id = match offer_id(
                &transaction.unsigned.chain_id,
                &operation.owner,
                transaction.unsigned.sequence,
            ) {
                Ok(offer_id) => offer_id,
                Err(_) => return 0,
            };
            if ledger.offer(&offer_id).is_none() {
                match plan_offer_create_matches(ledger, operation, block_height) {
                    Ok(plan) if offer_create_has_residual(&plan) => OFFER_STATE_EXPANSION_FEE,
                    Ok(_) => 0,
                    Err(_) => OFFER_STATE_EXPANSION_FEE,
                }
            } else {
                0
            }
        }
        _ => 0,
    }
}

pub fn offer_transaction_estimated_cross_count(
    ledger: &LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
) -> usize {
    match &transaction.unsigned.operation {
        OfferTransactionOperation::OfferCreate(operation) => {
            plan_offer_create_matches(ledger, operation, block_height)
                .map(|plan| plan.fills.len())
                .unwrap_or(0)
        }
        _ => 0,
    }
}

pub fn offer_transaction_will_create_residual_offer(
    ledger: &LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
) -> bool {
    match &transaction.unsigned.operation {
        OfferTransactionOperation::OfferCreate(operation) => {
            plan_offer_create_matches(ledger, operation, block_height)
                .map(|plan| offer_create_has_residual(&plan))
                .unwrap_or(true)
        }
        _ => false,
    }
}

pub fn offer_transaction_match_fee(
    ledger: &LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
) -> u64 {
    u64::try_from(offer_transaction_estimated_cross_count(
        ledger,
        transaction,
        block_height,
    ))
    .unwrap_or(u64::MAX)
    .saturating_mul(OFFER_MATCH_CROSS_FEE)
}

pub fn minimum_offer_transaction_fee_for_ledger(
    ledger: &LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
) -> u64 {
    minimum_offer_transaction_fee(transaction)
        .saturating_add(offer_transaction_state_expansion_fee(
            ledger,
            transaction,
            block_height,
        ))
        .saturating_add(offer_transaction_match_fee(
            ledger,
            transaction,
            block_height,
        ))
}

fn apply_offer_operation(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
) -> Result<OfferApplyOutcome, (&'static str, String)> {
    match &transaction.unsigned.operation {
        OfferTransactionOperation::OfferCreate(operation) => {
            if transaction.unsigned.transaction_kind != OFFER_CREATE_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "offer_create transaction kind mismatch".to_string(),
                ));
            }
            if operation.expiration_height != 0 && operation.expiration_height <= block_height {
                return Err((
                    "offer_expired",
                    "offer expiration_height must be greater than current block height".to_string(),
                ));
            }
            let offer_id = offer_id(
                &genesis.chain_id,
                &operation.owner,
                transaction.unsigned.sequence,
            )
            .map_err(|error| ("bad_offer_id", error))?;
            if ledger.offer(&offer_id).is_some() {
                return Err((
                    "duplicate_offer",
                    format!("offer `{offer_id}` already exists"),
                ));
            }
            ensure_offer_asset_participation(
                ledger,
                &operation.owner,
                &operation.taker_gets_asset_id,
                true,
            )?;
            ensure_offer_asset_participation(
                ledger,
                &operation.owner,
                &operation.taker_pays_asset_id,
                false,
            )?;
            if operation.taker_pays_asset_id != NATIVE_PFT_ESCROW_ASSET_ID {
                ensure_issued_offer_receive_capacity(
                    ledger,
                    &operation.owner,
                    &operation.taker_pays_asset_id,
                    operation.taker_pays_amount,
                    None,
                )?;
            }

            lock_offer_sell_side(
                ledger,
                &operation.owner,
                &operation.taker_gets_asset_id,
                operation.taker_gets_amount,
            )?;

            let plan = plan_offer_create_matches(ledger, operation, block_height)?;
            let fills = apply_offer_fills(ledger, operation, &plan)?;
            if offer_create_has_residual(&plan) {
                lock_offer_reserve(ledger, &operation.owner, OFFER_OBJECT_RESERVE)?;
                let mut offer = Offer::new(
                    &genesis.chain_id,
                    operation.owner.clone(),
                    transaction.unsigned.sequence,
                    operation.taker_gets_asset_id.clone(),
                    plan.taker_gets_remaining,
                    operation.taker_pays_asset_id.clone(),
                    plan.taker_pays_remaining,
                    block_height,
                    operation.expiration_height,
                )
                .map_err(|error| ("bad_offer", error))?;
                offer.original_taker_gets_amount = operation.taker_gets_amount;
                offer.original_taker_pays_amount = operation.taker_pays_amount;
                offer.reserve_paid = OFFER_OBJECT_RESERVE;
                ledger.offers.push(offer);
                Ok(OfferApplyOutcome {
                    receipt_code: if fills.is_empty() {
                        "accepted"
                    } else {
                        "partially_filled"
                    },
                    offer_id: Some(offer_id),
                    fills,
                })
            } else {
                refund_offer_sell_side(
                    ledger,
                    &operation.owner,
                    &operation.taker_gets_asset_id,
                    plan.taker_gets_remaining,
                    None,
                )?;
                Ok(OfferApplyOutcome {
                    receipt_code: if fills.is_empty() {
                        "accepted"
                    } else {
                        "filled"
                    },
                    offer_id: None,
                    fills,
                })
            }
        }
        OfferTransactionOperation::OfferCancel(operation) => {
            if transaction.unsigned.transaction_kind != OFFER_CANCEL_TRANSACTION_KIND {
                return Err((
                    "wrong_transaction_kind",
                    "offer_cancel transaction kind mismatch".to_string(),
                ));
            }
            let offer_index = offer_index(ledger, &operation.offer_id).ok_or_else(|| {
                (
                    "missing_offer",
                    format!("offer `{}` does not exist", operation.offer_id),
                )
            })?;
            let offer = ledger.offers[offer_index].clone();
            if offer.owner != operation.owner {
                return Err((
                    "offer_owner_mismatch",
                    "offer_cancel owner does not match offer".to_string(),
                ));
            }
            if offer.state != OFFER_STATE_OPEN {
                return Err(("offer_not_open", "offer is not open for cancel".to_string()));
            }
            refund_offer_sell_side(
                ledger,
                &offer.owner,
                &offer.taker_gets_asset_id,
                offer.taker_gets_amount_remaining,
                Some(&offer.offer_id),
            )?;
            refund_offer_reserve(ledger, &offer.owner, offer.reserve_paid)?;
            let offer = &mut ledger.offers[offer_index];
            offer.state = OFFER_STATE_CANCELED.to_string();
            offer.reserve_paid = 0;
            Ok(OfferApplyOutcome {
                receipt_code: "accepted",
                offer_id: Some(operation.offer_id.clone()),
                fills: Vec::new(),
            })
        }
    }
}

fn plan_offer_create_matches(
    ledger: &LedgerState,
    operation: &OfferCreateOperation,
    block_height: u64,
) -> Result<OfferMatchPlan, (&'static str, String)> {
    let mut candidate_indexes: Vec<usize> = ledger
        .offers
        .iter()
        .enumerate()
        .filter_map(|(index, offer)| {
            if offer.state == OFFER_STATE_OPEN
                && !offer_is_expired(offer, block_height)
                && offer.taker_gets_asset_id == operation.taker_pays_asset_id
                && offer.taker_pays_asset_id == operation.taker_gets_asset_id
                && offer_is_crossable_at_taker_limit(offer, operation)
            {
                Some(index)
            } else {
                None
            }
        })
        .collect();
    candidate_indexes.sort_by(|left, right| {
        let left_offer = &ledger.offers[*left];
        let right_offer = &ledger.offers[*right];
        compare_offer_price_for_taker(left_offer, right_offer)
            .then_with(|| left_offer.created_height.cmp(&right_offer.created_height))
            .then_with(|| left_offer.owner_sequence.cmp(&right_offer.owner_sequence))
            .then_with(|| left_offer.offer_id.cmp(&right_offer.offer_id))
    });

    let mut taker_gets_remaining = operation.taker_gets_amount;
    let mut taker_pays_remaining = operation.taker_pays_amount;
    let mut fills = Vec::new();
    let mut crossed = 0usize;
    for offer_index in candidate_indexes {
        if taker_gets_remaining == 0
            || taker_pays_remaining == 0
            || crossed >= MAX_DEX_CROSSES_PER_TRANSACTION
        {
            break;
        }
        crossed = crossed.saturating_add(1);
        let offer = &ledger.offers[offer_index];
        let Some((maker_sends_amount, taker_sends_amount)) =
            integer_offer_fill_amounts(offer, taker_gets_remaining, taker_pays_remaining)
        else {
            continue;
        };
        taker_gets_remaining = taker_gets_remaining
            .checked_sub(taker_sends_amount)
            .ok_or_else(|| {
                (
                    "offer_fill_underflow",
                    "taker send-side remaining underflowed".to_string(),
                )
            })?;
        taker_pays_remaining = taker_pays_remaining
            .checked_sub(maker_sends_amount)
            .ok_or_else(|| {
                (
                    "offer_fill_underflow",
                    "taker receive-side remaining underflowed".to_string(),
                )
            })?;
        fills.push(OfferFillPlan {
            maker_offer_index: offer_index,
            maker_sends_amount,
            taker_sends_amount,
        });
    }

    Ok(OfferMatchPlan {
        fills,
        taker_gets_remaining,
        taker_pays_remaining,
    })
}

fn offer_create_has_residual(plan: &OfferMatchPlan) -> bool {
    plan.taker_gets_remaining > 0 && plan.taker_pays_remaining > 0
}

fn apply_offer_fills(
    ledger: &mut LedgerState,
    operation: &OfferCreateOperation,
    plan: &OfferMatchPlan,
) -> Result<Vec<OfferFillReceipt>, (&'static str, String)> {
    let mut receipts = Vec::new();
    for (fill_index, fill) in plan.fills.iter().enumerate() {
        let maker = ledger
            .offers
            .get(fill.maker_offer_index)
            .cloned()
            .ok_or_else(|| {
                (
                    "missing_offer",
                    "planned maker offer no longer exists".to_string(),
                )
            })?;
        if maker.state != OFFER_STATE_OPEN {
            return Err((
                "offer_not_open",
                "planned maker offer is not open".to_string(),
            ));
        }
        let maker_offer_id = maker.offer_id.clone();
        {
            let maker_offer = ledger
                .offers
                .get_mut(fill.maker_offer_index)
                .ok_or_else(|| {
                    (
                        "missing_offer",
                        "planned maker offer no longer exists".to_string(),
                    )
                })?;
            maker_offer.taker_gets_amount_remaining = maker_offer
                .taker_gets_amount_remaining
                .checked_sub(fill.maker_sends_amount)
                .ok_or_else(|| {
                    (
                        "offer_fill_underflow",
                        "maker send-side remaining underflowed".to_string(),
                    )
                })?;
            maker_offer.taker_pays_amount_remaining = maker_offer
                .taker_pays_amount_remaining
                .checked_sub(fill.taker_sends_amount)
                .ok_or_else(|| {
                    (
                        "offer_fill_underflow",
                        "maker receive-side remaining underflowed".to_string(),
                    )
                })?;
            if maker_offer.taker_gets_amount_remaining == 0
                || maker_offer.taker_pays_amount_remaining == 0
            {
                maker_offer.taker_gets_amount_remaining = 0;
                maker_offer.taker_pays_amount_remaining = 0;
                maker_offer.state = OFFER_STATE_FILLED.to_string();
            }
        }

        credit_offer_received_asset(
            ledger,
            &operation.owner,
            &maker.taker_gets_asset_id,
            fill.maker_sends_amount,
        )?;
        credit_offer_received_asset(
            ledger,
            &maker.owner,
            &maker.taker_pays_asset_id,
            fill.taker_sends_amount,
        )?;

        let maker_offer = ledger
            .offers
            .get(fill.maker_offer_index)
            .cloned()
            .ok_or_else(|| {
                (
                    "missing_offer",
                    "planned maker offer no longer exists".to_string(),
                )
            })?;
        let terminal_maker_state = if maker_offer.state == OFFER_STATE_FILLED {
            refund_offer_reserve(ledger, &maker_offer.owner, maker_offer.reserve_paid)?;
            let maker_offer = ledger
                .offers
                .get_mut(fill.maker_offer_index)
                .ok_or_else(|| {
                    (
                        "missing_offer",
                        "planned maker offer no longer exists".to_string(),
                    )
                })?;
            maker_offer.reserve_paid = 0;
            Some(OFFER_STATE_FILLED.to_string())
        } else {
            None
        };
        let maker_offer = ledger.offers.get(fill.maker_offer_index).ok_or_else(|| {
            (
                "missing_offer",
                "planned maker offer no longer exists".to_string(),
            )
        })?;
        receipts.push(OfferFillReceipt {
            fill_index: u64::try_from(fill_index).unwrap_or(u64::MAX),
            maker_offer_id,
            maker_owner: maker.owner,
            taker: operation.owner.clone(),
            maker_sends_asset_id: maker.taker_gets_asset_id,
            maker_sends_amount: fill.maker_sends_amount,
            taker_sends_asset_id: maker.taker_pays_asset_id,
            taker_sends_amount: fill.taker_sends_amount,
            maker_taker_gets_remaining: maker_offer.taker_gets_amount_remaining,
            maker_taker_pays_remaining: maker_offer.taker_pays_amount_remaining,
            terminal_maker_state,
        });
    }
    Ok(receipts)
}

fn offer_is_expired(offer: &Offer, block_height: u64) -> bool {
    offer.expiration_height != 0 && offer.expiration_height <= block_height
}

fn offer_is_crossable_at_taker_limit(offer: &Offer, operation: &OfferCreateOperation) -> bool {
    u128::from(offer.taker_pays_amount_remaining)
        .saturating_mul(u128::from(operation.taker_pays_amount))
        <= u128::from(operation.taker_gets_amount)
            .saturating_mul(u128::from(offer.taker_gets_amount_remaining))
}

fn compare_offer_price_for_taker(left: &Offer, right: &Offer) -> std::cmp::Ordering {
    u128::from(left.taker_pays_amount_remaining)
        .saturating_mul(u128::from(right.taker_gets_amount_remaining))
        .cmp(
            &u128::from(right.taker_pays_amount_remaining)
                .saturating_mul(u128::from(left.taker_gets_amount_remaining)),
        )
}

fn integer_offer_fill_amounts(
    offer: &Offer,
    taker_gets_remaining: u64,
    taker_pays_remaining: u64,
) -> Option<(u64, u64)> {
    let divisor = gcd_u64(
        offer.taker_gets_amount_remaining,
        offer.taker_pays_amount_remaining,
    );
    if divisor == 0 {
        return None;
    }
    let unit_gets = offer.taker_gets_amount_remaining / divisor;
    let unit_pays = offer.taker_pays_amount_remaining / divisor;
    if unit_gets == 0 || unit_pays == 0 {
        return None;
    }
    let units = (offer.taker_gets_amount_remaining / unit_gets)
        .min(offer.taker_pays_amount_remaining / unit_pays)
        .min(taker_pays_remaining / unit_gets)
        .min(taker_gets_remaining / unit_pays);
    if units == 0 {
        return None;
    }
    Some((units.checked_mul(unit_gets)?, units.checked_mul(unit_pays)?))
}

fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
