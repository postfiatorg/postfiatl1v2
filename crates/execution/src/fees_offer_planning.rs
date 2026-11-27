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
        AssetTransactionOperation::SolanaBridgeProof(op)
            if op.proof_kind == postfiat_types::SOLANA_DEPOSIT_PROOF
                && op.public_values.len() <= postfiat_types::DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES as usize => {
            let count = postfiat_types::SolanaDepositPublicValues::decode(&op.public_values)
                .map_or(0, |values| values.receipts.len() as u64);
            TRUSTLINE_STATE_EXPANSION_FEE.saturating_mul(3).saturating_mul(count)
        }
        AssetTransactionOperation::VaultBridgeDepositClaim(_) => {
            TRUSTLINE_STATE_EXPANSION_FEE.saturating_mul(3)
        }
        AssetTransactionOperation::YoloTargetRegisterV1(operation)
            if !ledger.yolo_target_registrations.iter().any(|row| {
                row.registration_id == operation.registration_id()
            }) =>
        {
            YOLO_TARGET_REGISTRATION_STATE_EXPANSION_FEE
        }
        AssetTransactionOperation::YoloTargetSubmitV1(operation)
            if !ledger
                .yolo_target_receipts
                .iter()
                .any(|row| row.registration_id == operation.registration_id) =>
        {
            YOLO_TARGET_RECEIPT_STATE_EXPANSION_FEE
        }
        _ => 0,
    }
}

pub fn asset_transaction_state_expansion_fee_with_compatibility(
    ledger: &LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> u64 {
    let base = asset_transaction_state_expansion_fee(ledger, transaction);
    let mut extra = 0_u64;
    if compatibility.asset_gate_v1_active(block_height) {
        extra = extra.saturating_add(match &transaction.unsigned.operation {
            AssetTransactionOperation::IssuedPayment(operation)
                if operation.to != operation.issuer
                    && ledger
                        .trustline_for_account_asset(&operation.to, &operation.asset_id)
                        .is_none() =>
            {
                TRUSTLINE_STATE_EXPANSION_FEE
            }
            AssetTransactionOperation::IssuedPaymentV2(operation)
                if operation.to != operation.issuer
                    && ledger
                        .trustline_for_account_asset(&operation.to, &operation.asset_id)
                        .is_none() =>
            {
                TRUSTLINE_STATE_EXPANSION_FEE
            }
            AssetTransactionOperation::NavMintAtNav(operation)
                if operation.to != operation.issuer
                    && ledger
                        .trustline_for_account_asset(&operation.to, &operation.asset_id)
                        .is_none() =>
            {
                TRUSTLINE_STATE_EXPANSION_FEE
            }
            AssetTransactionOperation::BridgeHoldClaim(operation)
                if ledger
                    .trustline_for_account_asset(&operation.recipient, &operation.asset_id)
                    .is_none() =>
            {
                TRUSTLINE_STATE_EXPANSION_FEE
            }
            _ => 0,
        });
    }
    if compatibility.hive_identity_v1_active(block_height) {
        extra = extra.saturating_add(match &transaction.unsigned.operation {
            AssetTransactionOperation::IdentitySet(op) => match &op.payload {
                postfiat_types::IdentitySetActionV1::Mint { .. } => {
                    postfiat_types::IDENTITY_MINT_STATE_EXPANSION_FEE
                }
                postfiat_types::IdentitySetActionV1::Link { kind, target, .. } => {
                    let is_reconfirm = ledger
                        .identities_by_account
                        .get(&op.account)
                        .is_some_and(|id| {
                            id.links.iter().any(|l| {
                                l.kind == *kind
                                    && l.target == *target
                                    && l.status
                                        == postfiat_types::IdentityLinkStatus::PendingReconfirmation
                            })
                        });
                    if is_reconfirm {
                        0
                    } else {
                        postfiat_types::IDENTITY_LINK_STATE_EXPANSION_FEE
                    }
                }
                _ => 0,
            },
            AssetTransactionOperation::IdentityRegistrarRegister(op)
                if !ledger
                    .handle_namespace_registrars
                    .contains_key(&op.namespace) =>
            {
                ASSET_DEFINITION_STATE_EXPANSION_FEE
            }
            AssetTransactionOperation::IdentityHandleReserve(_) => TRUSTLINE_STATE_EXPANSION_FEE,
            _ => 0,
        });
    }
    if compatibility.nav_fee_v1_active(block_height) {
        if let AssetTransactionOperation::NavFeeClaim(op) = &transaction.unsigned.operation {
            if ledger
                .trustline_for_account_asset(&op.recipient, &op.asset_id)
                .is_none()
            {
                extra = extra.saturating_add(TRUSTLINE_STATE_EXPANSION_FEE);
            }
        }
    }
    if compatibility.credential_did_v1_active(block_height) {
        if let AssetTransactionOperation::IssuerRegister(_) = &transaction.unsigned.operation {
            extra = extra.saturating_add(ASSET_DEFINITION_STATE_EXPANSION_FEE);
        }
    }
    base.saturating_add(extra)
}

pub fn minimum_asset_transaction_fee_for_ledger(
    ledger: &LedgerState,
    transaction: &SignedAssetTransaction,
) -> u64 {
    minimum_asset_transaction_fee(transaction)
        .saturating_add(asset_transaction_state_expansion_fee(ledger, transaction))
}

pub fn minimum_asset_transaction_fee_for_ledger_with_compatibility(
    ledger: &LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> u64 {
    minimum_asset_transaction_fee(transaction).saturating_add(
        asset_transaction_state_expansion_fee_with_compatibility(
            ledger,
            transaction,
            block_height,
            compatibility,
        ),
    )
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

pub fn escrow_transaction_state_expansion_fee_with_compatibility(
    ledger: &LedgerState,
    transaction: &SignedEscrowTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> u64 {
    let base = escrow_transaction_state_expansion_fee(ledger, transaction);
    if !compatibility.credential_did_v1_active(block_height) {
        return base;
    }
    let extra = match &transaction.unsigned.operation {
        EscrowTransactionOperation::EscrowCreate(operation)
            if operation.asset_id != NATIVE_PFT_ESCROW_ASSET_ID
                && ledger
                    .asset_definition(&operation.asset_id)
                    .is_some_and(|asset| operation.recipient != asset.issuer)
                && ledger
                    .trustline_for_account_asset(&operation.recipient, &operation.asset_id)
                    .is_none() =>
        {
            TRUSTLINE_STATE_EXPANSION_FEE
        }
        EscrowTransactionOperation::EscrowFinish(operation) => ledger
            .escrow(&operation.escrow_id)
            .filter(|escrow| escrow.asset_id != NATIVE_PFT_ESCROW_ASSET_ID)
            .map_or(0, |escrow| {
                if ledger
                    .asset_definition(&escrow.asset_id)
                    .is_some_and(|asset| escrow.recipient != asset.issuer)
                    && ledger
                        .trustline_for_account_asset(&escrow.recipient, &escrow.asset_id)
                        .is_none()
                {
                    TRUSTLINE_STATE_EXPANSION_FEE
                } else {
                    0
                }
            }),
        _ => 0,
    };
    base.saturating_add(extra)
}

pub fn minimum_escrow_transaction_fee_for_ledger(
    ledger: &LedgerState,
    transaction: &SignedEscrowTransaction,
) -> u64 {
    minimum_escrow_transaction_fee(transaction)
        .saturating_add(escrow_transaction_state_expansion_fee(ledger, transaction))
}

pub fn minimum_escrow_transaction_fee_for_ledger_with_compatibility(
    ledger: &LedgerState,
    transaction: &SignedEscrowTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> u64 {
    minimum_escrow_transaction_fee(transaction).saturating_add(
        escrow_transaction_state_expansion_fee_with_compatibility(
            ledger,
            transaction,
            block_height,
            compatibility,
        ),
    )
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

pub fn offer_transaction_state_expansion_fee_with_compatibility(
    ledger: &LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> u64 {
    let base = offer_transaction_state_expansion_fee(ledger, transaction, block_height);
    if !compatibility.credential_did_v1_active(block_height) {
        return base;
    }
    let extra = match &transaction.unsigned.operation {
        OfferTransactionOperation::OfferCreate(operation)
            if operation.taker_pays_asset_id != NATIVE_PFT_ESCROW_ASSET_ID
                && ledger
                    .asset_definition(&operation.taker_pays_asset_id)
                    .is_some_and(|asset| operation.owner != asset.issuer)
                && ledger
                    .trustline_for_account_asset(&operation.owner, &operation.taker_pays_asset_id)
                    .is_none() =>
        {
            TRUSTLINE_STATE_EXPANSION_FEE
        }
        _ => 0,
    };
    base.saturating_add(extra)
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

pub fn minimum_offer_transaction_fee_for_ledger_with_compatibility(
    ledger: &LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> u64 {
    minimum_offer_transaction_fee(transaction)
        .saturating_add(offer_transaction_state_expansion_fee_with_compatibility(
            ledger,
            transaction,
            block_height,
            compatibility,
        ))
        .saturating_add(offer_transaction_match_fee(
            ledger,
            transaction,
            block_height,
        ))
}

fn offer_maker_is_eligible_at_height(
    ledger: &LedgerState,
    offer: &Offer,
    block_height: u64,
) -> bool {
    if offer_is_expired(offer, block_height) {
        return false;
    }
    if offer.taker_gets_asset_id != NATIVE_PFT_ESCROW_ASSET_ID {
        let Some(asset) = ledger.asset_definition(&offer.taker_gets_asset_id) else {
            return false;
        };
        if !matches!(
            asset_gate(
                ledger,
                asset,
                Some(&offer.owner),
                None,
                AssetMovementOpKind::OfferFill,
                block_height,
            ),
            AssetGateOutcome::Allow
        ) {
            return false;
        }
    }
    if offer.taker_pays_asset_id != NATIVE_PFT_ESCROW_ASSET_ID {
        let Some(asset) = ledger.asset_definition(&offer.taker_pays_asset_id) else {
            return false;
        };
        if !matches!(
            asset_gate(
                ledger,
                asset,
                None,
                Some(&offer.owner),
                AssetMovementOpKind::OfferFill,
                block_height,
            ),
            AssetGateOutcome::Allow
        ) {
            return false;
        }
    }
    true
}

fn cancel_offer_and_refund_gated(
    ledger: &mut LedgerState,
    offer_index: usize,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    let offer = ledger.offers[offer_index].clone();
    if offer.taker_gets_amount_remaining > 0 {
        if offer.taker_gets_asset_id == NATIVE_PFT_ESCROW_ASSET_ID {
            refund_offer_sell_side(
                ledger,
                &offer.owner,
                &offer.taker_gets_asset_id,
                offer.taker_gets_amount_remaining,
                Some(&offer.offer_id),
            )?;
        } else {
            let asset = ledger
                .asset_definition(&offer.taker_gets_asset_id)
                .cloned()
                .ok_or_else(|| {
                    (
                        "missing_asset",
                        format!("asset `{}` does not exist", offer.taker_gets_asset_id),
                    )
                })?;
            let _ = asset_gate(
                ledger,
                &asset,
                Some(&offer.owner),
                Some(&offer.owner),
                AssetMovementOpKind::OfferCancelRefund,
                block_height,
            );
            let owner_index = issued_asset_credit_recipient_line_index_gated(
                ledger,
                &asset,
                &offer.owner,
                offer.taker_gets_amount_remaining,
                "offer cancel refund",
            )?;
            let owner_after = ledger.trustlines[owner_index]
                .balance
                .checked_add(offer.taker_gets_amount_remaining)
                .ok_or_else(|| {
                    (
                        "issued_balance_overflow",
                        "offer issued-asset refund would overflow".to_string(),
                    )
                })?;
            let reserved_after_cancel = issued_asset_reserved_total_for_account(
                ledger,
                &offer.owner,
                &offer.taker_gets_asset_id,
                None,
                Some(&offer.offer_id),
            )?;
            let required_limit = owner_after
                .checked_add(reserved_after_cancel)
                .ok_or_else(|| {
                    (
                        "issued_balance_overflow",
                        "offer issued-asset refund reservations would overflow".to_string(),
                    )
                })?;
            apply_prepared_issued_asset_credit(ledger, owner_index, owner_after, required_limit);
        }
    }
    refund_offer_reserve(ledger, &offer.owner, offer.reserve_paid)?;
    let offer_mut = &mut ledger.offers[offer_index];
    offer_mut.taker_gets_amount_remaining = 0;
    offer_mut.taker_pays_amount_remaining = 0;
    offer_mut.state = OFFER_STATE_CANCELED.to_string();
    offer_mut.reserve_paid = 0;
    Ok(())
}

fn apply_offer_operation(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Result<OfferApplyOutcome, (&'static str, String)> {
    let gate_active = compatibility.asset_gate_v1_active(block_height);
    let cred_active = compatibility.credential_did_v1_active(block_height);
    let effective_gate = gate_active || cred_active;
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
            if effective_gate {
                if operation.taker_gets_asset_id != NATIVE_PFT_ESCROW_ASSET_ID {
                    let sell_asset = ledger
                        .asset_definition(&operation.taker_gets_asset_id)
                        .cloned()
                        .ok_or_else(|| {
                            (
                                "missing_asset",
                                format!("asset `{}` does not exist", operation.taker_gets_asset_id),
                            )
                        })?;
                    match asset_gate(
                        ledger,
                        &sell_asset,
                        Some(&operation.owner),
                        None,
                        AssetMovementOpKind::OfferCreate,
                        block_height,
                    ) {
                        AssetGateOutcome::Allow => {}
                        AssetGateOutcome::AllowReturnOnly => {
                            return Err((
                                "asset_gate_sender_ineligible",
                                "offer_create is not a return-only exit".to_string(),
                            ));
                        }
                        AssetGateOutcome::RouteToHold { reason_code } => {
                            return Err((
                                reason_code,
                                "offer_create sell-side rejected by asset_gate".to_string(),
                            ));
                        }
                        AssetGateOutcome::Reject { code, message } => return Err((code, message)),
                    }
                }
                if operation.taker_pays_asset_id != NATIVE_PFT_ESCROW_ASSET_ID {
                    let buy_asset = ledger
                        .asset_definition(&operation.taker_pays_asset_id)
                        .cloned()
                        .ok_or_else(|| {
                            (
                                "missing_asset",
                                format!("asset `{}` does not exist", operation.taker_pays_asset_id),
                            )
                        })?;
                    match asset_gate(
                        ledger,
                        &buy_asset,
                        None,
                        Some(&operation.owner),
                        AssetMovementOpKind::OfferCreate,
                        block_height,
                    ) {
                        AssetGateOutcome::Allow => {}
                        AssetGateOutcome::AllowReturnOnly => {
                            return Err((
                                "asset_gate_recipient_ineligible",
                                "offer_create is not a return-only exit".to_string(),
                            ));
                        }
                        AssetGateOutcome::RouteToHold { reason_code } => {
                            return Err((
                                reason_code,
                                "offer_create buy-side rejected by asset_gate".to_string(),
                            ));
                        }
                        AssetGateOutcome::Reject { code, message } => return Err((code, message)),
                    }
                }
            }
            if cred_active {
                // Pillar 8 M2: Permissionless-by-default DEX offers auto-create missing
                // buy-side implicit lines (and skip legacy authorized-flag rejection for
                // credential-approved holders already verified by `asset_gate`).
                if operation.taker_pays_asset_id != NATIVE_PFT_ESCROW_ASSET_ID {
                    let buy_asset = ledger
                        .asset_definition(&operation.taker_pays_asset_id)
                        .cloned()
                        .ok_or_else(|| {
                            (
                                "missing_asset",
                                format!("asset `{}` does not exist", operation.taker_pays_asset_id),
                            )
                        })?;
                    if operation.owner == buy_asset.issuer {
                        return Err((
                            "unsupported_issuer_offer",
                            "issued-asset offers require holder trustlines".to_string(),
                        ));
                    }
                    let idx = issued_asset_credit_recipient_line_index_gated(
                        ledger,
                        &buy_asset,
                        &operation.owner,
                        operation.taker_pays_amount,
                        "offer_create buy-side",
                    )?;
                    if ledger.trustlines[idx].reserve_paid == 0
                        && ledger.trustlines[idx].holder_limit.is_none()
                    {
                        ledger.trustlines[idx].implicit = true;
                    }
                    let reserved = issued_asset_reserved_total_for_account(
                        ledger,
                        &operation.owner,
                        &operation.taker_pays_asset_id,
                        None,
                        None,
                    )?;
                    let required = ledger.trustlines[idx]
                        .balance
                        .checked_add(reserved)
                        .and_then(|v| v.checked_add(operation.taker_pays_amount))
                        .ok_or_else(|| {
                            (
                                "issued_balance_overflow",
                                "offer buy-side issued-asset reservations would overflow"
                                    .to_string(),
                            )
                        })?;
                    if let Some(cap) = ledger.trustlines[idx].holder_limit {
                        if required > cap {
                            return Err((
                                "trustline_limit_exceeded",
                                "offer buy-side issued amount exceeds binding holder_limit"
                                    .to_string(),
                            ));
                        }
                    } else if required > ledger.trustlines[idx].limit {
                        ledger.trustlines[idx].limit = required;
                    }
                }
                lock_offer_sell_side_with_mode(
                    ledger,
                    &operation.owner,
                    &operation.taker_gets_asset_id,
                    operation.taker_gets_amount,
                    true,
                )?;
            } else {
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
            }

            let (plan, ineligible_to_cancel) = if effective_gate {
                plan_offer_create_matches_gated(ledger, operation, block_height)?
            } else {
                (plan_offer_create_matches(ledger, operation, block_height)?, Vec::new())
            };
            // Inline DEX anti-poisoning: cancel & refund skipped ineligible makers inline so
            // cancellations persist even on partial fills or resting book!
            for maker_idx in ineligible_to_cancel {
                cancel_offer_and_refund_gated(ledger, maker_idx, block_height)?;
            }
            let fills = apply_offer_fills_with_mode(ledger, operation, &plan, cred_active)?;
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
                if gate_active && operation.taker_gets_asset_id != NATIVE_PFT_ESCROW_ASSET_ID {
                    auto_prune_empty_implicit_trustline(
                        ledger,
                        &operation.owner,
                        &operation.taker_gets_asset_id,
                    )?;
                }
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
                if gate_active && operation.taker_gets_asset_id != NATIVE_PFT_ESCROW_ASSET_ID {
                    auto_prune_empty_implicit_trustline(
                        ledger,
                        &operation.owner,
                        &operation.taker_gets_asset_id,
                    )?;
                }
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
            if gate_active {
                cancel_offer_and_refund_gated(ledger, offer_index, block_height)?;
            } else {
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
            }
            Ok(OfferApplyOutcome {
                receipt_code: "accepted",
                offer_id: Some(operation.offer_id.clone()),
                fills: Vec::new(),
            })
        }
        OfferTransactionOperation::OfferPruneIneligible(operation) => {
            if transaction.unsigned.transaction_kind
                != postfiat_types::OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND
            {
                return Err((
                    "wrong_transaction_kind",
                    "offer_prune_ineligible transaction kind mismatch".to_string(),
                ));
            }
            if !gate_active {
                return Err((
                    "asset_gate_not_active",
                    format!(
                        "offer_prune_ineligible is not active at block height {block_height}"
                    ),
                ));
            }
            if operation.offer_ids.is_empty()
                || operation.offer_ids.len() > postfiat_types::MAX_OFFER_PRUNE_BATCH
            {
                return Err((
                    "offer_prune_batch_limit_exceeded",
                    format!(
                        "offer_prune_ineligible requires 1..={} offer IDs",
                        postfiat_types::MAX_OFFER_PRUNE_BATCH
                    ),
                ));
            }
            let mut indices = Vec::with_capacity(operation.offer_ids.len());
            for offer_id in &operation.offer_ids {
                let idx = offer_index(ledger, offer_id).ok_or_else(|| {
                    (
                        "missing_offer",
                        format!("offer `{offer_id}` does not exist"),
                    )
                })?;
                let offer = &ledger.offers[idx];
                if offer.state != OFFER_STATE_OPEN {
                    return Err((
                        "offer_not_open",
                        format!("offer `{offer_id}` is not open for pruning"),
                    ));
                }
                if offer_maker_is_eligible_at_height(ledger, offer, block_height) {
                    return Err((
                        "offer_still_eligible",
                        format!("offer `{offer_id}` is still eligible and unexpired"),
                    ));
                }
                indices.push(idx);
            }
            for idx in indices {
                cancel_offer_and_refund_gated(ledger, idx, block_height)?;
            }
            Ok(OfferApplyOutcome {
                receipt_code: "accepted",
                offer_id: operation.offer_ids.first().cloned(),
                fills: Vec::new(),
            })
        }
    }
}

fn plan_offer_create_matches_gated(
    ledger: &LedgerState,
    operation: &OfferCreateOperation,
    block_height: u64,
) -> Result<(OfferMatchPlan, Vec<usize>), (&'static str, String)> {
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
    let mut ineligible_to_cancel = Vec::new();
    let mut crossed = 0usize;
    let mut skipped_ineligible = 0usize;
    for offer_index in candidate_indexes {
        if taker_gets_remaining == 0
            || taker_pays_remaining == 0
            || crossed >= MAX_DEX_CROSSES_PER_TRANSACTION
        {
            break;
        }
        let offer = &ledger.offers[offer_index];
        if !offer_maker_is_eligible_at_height(ledger, offer, block_height) {
            if skipped_ineligible >= postfiat_types::MAX_DEX_INELIGIBLE_SKIPS_PER_TRANSACTION {
                break;
            }
            skipped_ineligible = skipped_ineligible.saturating_add(1);
            ineligible_to_cancel.push(offer_index);
            continue;
        }
        crossed = crossed.saturating_add(1);
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

    Ok((
        OfferMatchPlan {
            fills,
            taker_gets_remaining,
            taker_pays_remaining,
        },
        ineligible_to_cancel,
    ))
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

fn apply_offer_fills_with_mode(
    ledger: &mut LedgerState,
    operation: &OfferCreateOperation,
    plan: &OfferMatchPlan,
    cred_active: bool,
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

        b23_facility_transfer_offer_inventory(ledger, &maker.taker_gets_asset_id,
            &maker.owner, &operation.owner, fill.maker_sends_amount)?;
        b23_facility_transfer_offer_inventory(ledger, &maker.taker_pays_asset_id,
            &operation.owner, &maker.owner, fill.taker_sends_amount)?;
        credit_offer_received_asset_with_mode(
            ledger,
            &operation.owner,
            &maker.taker_gets_asset_id,
            fill.maker_sends_amount,
            cred_active,
        )?;
        credit_offer_received_asset_with_mode(
            ledger,
            &maker.owner,
            &maker.taker_pays_asset_id,
            fill.taker_sends_amount,
            cred_active,
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
