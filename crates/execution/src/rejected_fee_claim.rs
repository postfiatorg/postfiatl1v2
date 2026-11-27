// Fee claim for rejected transactions (XRPL `tec` semantics) and the governed
// destination-memo rule.
//
// A certified transparent block may contain a transaction that execution
// rejects. Before `rejected_tx_fee_activation_height` such a rejection is free.
// From that height, the batch executor passes every rejected receipt through a
// `claim_rejected_*_fee` function. The claim re-verifies, against the ledger as
// it stood before the transaction (rejections never mutate it), that the
// transaction:
//
// 1. passes its envelope `validate()` (well formed, within size limits);
// 2. matches this chain's domain: chain id, genesis hash, protocol version,
//    address namespace, transaction kind;
// 3. authenticates: ML-DSA-65, signature valid, the key derives the payer's
//    address, and the key equals any key already bound to the account;
// 4. uses exactly the payer's next sequence;
// 5. declares at least the minimum fee, and the payer can pay that fee while
//    keeping the account reserve.
//
// If all five hold and the rejection is not an expiry or a disabled feature
// (`FEE_CLAIM_EXCLUDED_REJECTION_CODES`), the declared fee is debited and
// burned, the sequence is consumed, a still-unbound public key is bound (as on
// any first spend), and the receipt code becomes `tec_<original code>`.
// Nothing else changes. Otherwise the original receipt is returned unchanged.

/// Rejections that are never fee-claimed even when the transaction
/// authenticates and can pay: the signer's own validity window has passed, or
/// the feature the transaction uses is disabled at this height (XRPL
/// `tefMAX_LEDGER` / `temDISABLED`).
pub const FEE_CLAIM_EXCLUDED_REJECTION_CODES: &[&str] = &[
    "swap_expired",
    "atomic_swap_paused",
    "atomic_swap_not_active",
    "account_set_not_active",
    "issued_payment_v2_not_active",
    "yolo_target_inactive",
];

/// Whether a rejection code may be fee-claimed, provided the transaction also
/// passes the authentication, sequence and fee-affordability checks.
pub fn rejection_code_is_fee_claimable(code: &str) -> bool {
    !FEE_CLAIM_EXCLUDED_REJECTION_CODES.contains(&code)
        && !code.starts_with(postfiat_types::RECEIPT_CODE_FEE_CLAIMED_PREFIX)
}

/// Rejects a covered payment that carries no memo when the destination has
/// `ACCOUNT_FLAG_REQUIRE_MEMO` and `require_memo_activation_height` is active.
fn require_destination_memo(
    ledger: &LedgerState,
    destination: &str,
    memo_count: usize,
    compatibility: AssetExecutionCompatibility,
    block_height: u64,
) -> Result<(), (&'static str, String)> {
    if memo_count > 0 || !compatibility.require_memo_active(block_height) {
        return Ok(());
    }
    if ledger
        .account(destination)
        .is_some_and(postfiat_types::Account::requires_memo)
    {
        return Err((
            postfiat_types::RECEIPT_CODE_DESTINATION_MEMO_REQUIRED,
            format!("destination `{destination}` requires at least one memo"),
        ));
    }
    Ok(())
}

struct FeeClaimDebit {
    payer: String,
    public_key_hex: String,
    balance_after_fee: u64,
    sequence: u64,
}

struct SingleSignerFeeClaim<'a> {
    chain_id: &'a str,
    genesis_hash: &'a str,
    protocol_version: u32,
    address_namespace: &'a str,
    transaction_kind: &'a str,
    expected_transaction_kind: &'a str,
    signature_algorithm_id: &'a str,
    envelope_algorithm_id: &'a str,
    payer: &'a str,
    public_key_hex: &'a str,
    signature_hex: &'a str,
    signing_bytes: &'a [u8],
    sequence: u64,
    fee: u64,
    minimum_fee: u64,
}

fn fee_claim_requested(
    receipt: &Receipt,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> bool {
    !receipt.accepted
        && compatibility.rejected_tx_fee_active(block_height)
        && rejection_code_is_fee_claimable(&receipt.code)
}

/// Checks 2-5 of the module comment for a single-signer envelope.
fn single_signer_fee_claim_debit(
    genesis: &Genesis,
    ledger: &LedgerState,
    claim: SingleSignerFeeClaim<'_>,
) -> Option<FeeClaimDebit> {
    if claim.chain_id != genesis.chain_id
        || claim.genesis_hash != genesis_hash(genesis)
        || claim.protocol_version != genesis.protocol_version
        || claim.address_namespace != ADDRESS_NAMESPACE
        || claim.transaction_kind != claim.expected_transaction_kind
        || claim.signature_algorithm_id != ML_DSA_65_ALGORITHM
        || claim.envelope_algorithm_id != claim.signature_algorithm_id
    {
        return None;
    }
    let public_key = hex_to_bytes(claim.public_key_hex).ok()?;
    let signature = hex_to_bytes(claim.signature_hex).ok()?;
    if address_from_public_key(&public_key) != claim.payer
        || !ml_dsa_65_verify(&public_key, claim.signing_bytes, &signature)
    {
        return None;
    }
    let account = ledger.account(claim.payer)?;
    if account
        .public_key_hex
        .as_deref()
        .is_some_and(|bound| bound != claim.public_key_hex)
    {
        return None;
    }
    let expected_sequence = account.sequence.checked_add(1)?;
    if claim.sequence != expected_sequence || claim.fee < claim.minimum_fee {
        return None;
    }
    let balance_after_fee = account.balance.checked_sub(claim.fee)?;
    if account_reserve_violation(claim.payer, balance_after_fee).is_some() {
        return None;
    }
    Some(FeeClaimDebit {
        payer: claim.payer.to_string(),
        public_key_hex: claim.public_key_hex.to_string(),
        balance_after_fee,
        sequence: expected_sequence,
    })
}

fn apply_fee_claim_debit(ledger: &mut LedgerState, debit: &FeeClaimDebit) -> bool {
    let Some(account) = ledger.account_mut(&debit.payer) else {
        return false;
    };
    if account.public_key_hex.is_none() {
        account.public_key_hex = Some(debit.public_key_hex.clone());
    }
    account.balance = debit.balance_after_fee;
    account.sequence = debit.sequence;
    true
}

fn fee_claimed_receipt(
    rejected: Receipt,
    fee_charged: u64,
    minimum_fee: u64,
    state_expansion_fee: u64,
) -> Receipt {
    let message = format!(
        "{}; fee of {fee_charged} atoms claimed and burned, sequence consumed",
        rejected.message
    );
    Receipt::rejected(
        rejected.tx_id,
        format!(
            "{}{}",
            postfiat_types::RECEIPT_CODE_FEE_CLAIMED_PREFIX,
            rejected.code
        ),
        message,
    )
    .with_fee_policy_and_state_expansion(
        fee_charged,
        fee_charged,
        minimum_fee,
        ACCOUNT_RESERVE,
        state_expansion_fee,
    )
}

fn claim_single_signer_fee(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    receipt: Receipt,
    claim: SingleSignerFeeClaim<'_>,
    state_expansion_fee: u64,
) -> Receipt {
    let fee = claim.fee;
    let minimum_fee = claim.minimum_fee;
    let Some(debit) = single_signer_fee_claim_debit(genesis, ledger, claim) else {
        return receipt;
    };
    if !apply_fee_claim_debit(ledger, &debit) {
        return receipt;
    }
    fee_claimed_receipt(receipt, fee, minimum_fee, state_expansion_fee)
}

/// Fee claim for a rejected `transparent_transfer` receipt.
pub fn claim_rejected_transfer_fee(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transfer: &SignedTransfer,
    receipt: Receipt,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if !fee_claim_requested(&receipt, block_height, compatibility) || transfer.validate().is_err() {
        return receipt;
    }
    let unsigned = &transfer.unsigned;
    let minimum_fee = minimum_transfer_fee_for_ledger(ledger, transfer);
    let state_expansion_fee = transfer_state_expansion_fee(ledger, transfer);
    let signing_bytes = unsigned.signing_bytes();
    claim_single_signer_fee(
        genesis,
        ledger,
        receipt,
        SingleSignerFeeClaim {
            chain_id: &unsigned.chain_id,
            genesis_hash: &unsigned.genesis_hash,
            protocol_version: unsigned.protocol_version,
            address_namespace: &unsigned.address_namespace,
            transaction_kind: &unsigned.transaction_kind,
            expected_transaction_kind: TRANSFER_TRANSACTION_KIND,
            signature_algorithm_id: &unsigned.signature_algorithm_id,
            envelope_algorithm_id: &transfer.algorithm_id,
            payer: &unsigned.from,
            public_key_hex: &transfer.public_key_hex,
            signature_hex: &transfer.signature_hex,
            signing_bytes: &signing_bytes,
            sequence: unsigned.sequence,
            fee: unsigned.fee,
            minimum_fee,
        },
        state_expansion_fee,
    )
}

/// Fee claim for a rejected `payment_v2` receipt.
pub fn claim_rejected_payment_v2_fee(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    payment: &SignedPaymentV2,
    receipt: Receipt,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if !fee_claim_requested(&receipt, block_height, compatibility) || payment.validate().is_err() {
        return receipt;
    }
    let unsigned = &payment.unsigned;
    let minimum_fee = minimum_payment_v2_fee_for_ledger(ledger, payment);
    let state_expansion_fee = payment_v2_state_expansion_fee(ledger, payment);
    let signing_bytes = unsigned.signing_bytes();
    claim_single_signer_fee(
        genesis,
        ledger,
        receipt,
        SingleSignerFeeClaim {
            chain_id: &unsigned.chain_id,
            genesis_hash: &unsigned.genesis_hash,
            protocol_version: unsigned.protocol_version,
            address_namespace: &unsigned.address_namespace,
            transaction_kind: &unsigned.transaction_kind,
            expected_transaction_kind: PAYMENT_V2_TRANSACTION_KIND,
            signature_algorithm_id: &unsigned.signature_algorithm_id,
            envelope_algorithm_id: &payment.algorithm_id,
            payer: &unsigned.from,
            public_key_hex: &payment.public_key_hex,
            signature_hex: &payment.signature_hex,
            signing_bytes: &signing_bytes,
            sequence: unsigned.sequence,
            fee: unsigned.fee,
            minimum_fee,
        },
        state_expansion_fee,
    )
}

/// Fee claim for a rejected asset-transaction receipt, including the
/// governed vault-bridge route rejection raised before execution.
pub fn claim_rejected_asset_transaction_fee(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    receipt: Receipt,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if !fee_claim_requested(&receipt, block_height, compatibility)
        || transaction
            .validate_with_legacy_vault_bridge_consume_supply_operator(
                !compatibility.bridge_verification_rules_active(block_height),
            )
            .is_err()
    {
        return receipt;
    }
    let unsigned = &transaction.unsigned;
    let minimum_fee = minimum_asset_transaction_fee_for_ledger(ledger, transaction);
    let state_expansion_fee = asset_transaction_state_expansion_fee(ledger, transaction);
    let signing_bytes = unsigned.signing_bytes();
    claim_single_signer_fee(
        genesis,
        ledger,
        receipt,
        SingleSignerFeeClaim {
            chain_id: &unsigned.chain_id,
            genesis_hash: &unsigned.genesis_hash,
            protocol_version: unsigned.protocol_version,
            address_namespace: &unsigned.address_namespace,
            transaction_kind: &unsigned.transaction_kind,
            expected_transaction_kind: unsigned.operation.transaction_kind(),
            signature_algorithm_id: &unsigned.signature_algorithm_id,
            envelope_algorithm_id: &transaction.algorithm_id,
            payer: &unsigned.source,
            public_key_hex: &transaction.public_key_hex,
            signature_hex: &transaction.signature_hex,
            signing_bytes: &signing_bytes,
            sequence: unsigned.sequence,
            fee: unsigned.fee,
            minimum_fee,
        },
        state_expansion_fee,
    )
}

/// Fee claim for a rejected escrow-transaction receipt.
pub fn claim_rejected_escrow_transaction_fee(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedEscrowTransaction,
    receipt: Receipt,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if !fee_claim_requested(&receipt, block_height, compatibility)
        || transaction.validate().is_err()
    {
        return receipt;
    }
    let unsigned = &transaction.unsigned;
    let minimum_fee = minimum_escrow_transaction_fee_for_ledger_with_compatibility(
        ledger,
        transaction,
        block_height,
        compatibility,
    );
    let state_expansion_fee = escrow_transaction_state_expansion_fee_with_compatibility(
        ledger,
        transaction,
        block_height,
        compatibility,
    );
    let signing_bytes = unsigned.signing_bytes();
    claim_single_signer_fee(
        genesis,
        ledger,
        receipt,
        SingleSignerFeeClaim {
            chain_id: &unsigned.chain_id,
            genesis_hash: &unsigned.genesis_hash,
            protocol_version: unsigned.protocol_version,
            address_namespace: &unsigned.address_namespace,
            transaction_kind: &unsigned.transaction_kind,
            expected_transaction_kind: unsigned.operation.transaction_kind(),
            signature_algorithm_id: &unsigned.signature_algorithm_id,
            envelope_algorithm_id: &transaction.algorithm_id,
            payer: &unsigned.source,
            public_key_hex: &transaction.public_key_hex,
            signature_hex: &transaction.signature_hex,
            signing_bytes: &signing_bytes,
            sequence: unsigned.sequence,
            fee: unsigned.fee,
            minimum_fee,
        },
        state_expansion_fee,
    )
}

/// Fee claim for a rejected NFT-transaction receipt.
pub fn claim_rejected_nft_transaction_fee(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedNftTransaction,
    receipt: Receipt,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if !fee_claim_requested(&receipt, block_height, compatibility)
        || transaction.validate().is_err()
    {
        return receipt;
    }
    let unsigned = &transaction.unsigned;
    let minimum_fee = minimum_nft_transaction_fee_for_ledger(ledger, transaction);
    let state_expansion_fee = nft_transaction_state_expansion_fee(ledger, transaction);
    let signing_bytes = unsigned.signing_bytes();
    claim_single_signer_fee(
        genesis,
        ledger,
        receipt,
        SingleSignerFeeClaim {
            chain_id: &unsigned.chain_id,
            genesis_hash: &unsigned.genesis_hash,
            protocol_version: unsigned.protocol_version,
            address_namespace: &unsigned.address_namespace,
            transaction_kind: &unsigned.transaction_kind,
            expected_transaction_kind: unsigned.operation.transaction_kind(),
            signature_algorithm_id: &unsigned.signature_algorithm_id,
            envelope_algorithm_id: &transaction.algorithm_id,
            payer: &unsigned.source,
            public_key_hex: &transaction.public_key_hex,
            signature_hex: &transaction.signature_hex,
            signing_bytes: &signing_bytes,
            sequence: unsigned.sequence,
            fee: unsigned.fee,
            minimum_fee,
        },
        state_expansion_fee,
    )
}

/// Fee claim for a rejected offer-transaction receipt.
pub fn claim_rejected_offer_transaction_fee(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedOfferTransaction,
    receipt: Receipt,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if !fee_claim_requested(&receipt, block_height, compatibility)
        || transaction.validate().is_err()
    {
        return receipt;
    }
    let unsigned = &transaction.unsigned;
    let minimum_fee = minimum_offer_transaction_fee_for_ledger_with_compatibility(
        ledger,
        transaction,
        block_height,
        compatibility,
    );
    let state_expansion_fee = offer_transaction_state_expansion_fee_with_compatibility(
        ledger,
        transaction,
        block_height,
        compatibility,
    );
    let signing_bytes = unsigned.signing_bytes();
    claim_single_signer_fee(
        genesis,
        ledger,
        receipt,
        SingleSignerFeeClaim {
            chain_id: &unsigned.chain_id,
            genesis_hash: &unsigned.genesis_hash,
            protocol_version: unsigned.protocol_version,
            address_namespace: &unsigned.address_namespace,
            transaction_kind: &unsigned.transaction_kind,
            expected_transaction_kind: unsigned.operation.transaction_kind(),
            signature_algorithm_id: &unsigned.signature_algorithm_id,
            envelope_algorithm_id: &transaction.algorithm_id,
            payer: &unsigned.source,
            public_key_hex: &transaction.public_key_hex,
            signature_hex: &transaction.signature_hex,
            signing_bytes: &signing_bytes,
            sequence: unsigned.sequence,
            fee: unsigned.fee,
            minimum_fee,
        },
        state_expansion_fee,
    )
}

/// Fee claim for a rejected atomic swap. Both owners signed the same swap
/// bytes, so both legs pay their declared fee and consume their sequence, but
/// only when both legs authenticate, are correctly sequenced and can pay.
/// An expired swap is never claimed.
pub fn claim_rejected_atomic_swap_fee(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAtomicSwapTransaction,
    receipt: Receipt,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if !fee_claim_requested(&receipt, block_height, compatibility)
        || transaction.validate().is_err()
    {
        return receipt;
    }
    let unsigned = &transaction.unsigned;
    if unsigned.chain_id != genesis.chain_id
        || unsigned.genesis_hash != genesis_hash(genesis)
        || unsigned.protocol_version != genesis.protocol_version
        || unsigned.address_namespace != ADDRESS_NAMESPACE
        || unsigned.signature_algorithm_id != ML_DSA_65_ALGORITHM
        || compatibility.atomic_swap_paused
        || !compatibility.atomic_swap_active(block_height)
        || block_height > unsigned.expires_at_height
    {
        return receipt;
    }
    let minimum_fee_0 =
        minimum_atomic_swap_leg_fee_for_ledger(ledger, transaction, &unsigned.leg_0);
    let minimum_fee_1 =
        minimum_atomic_swap_leg_fee_for_ledger(ledger, transaction, &unsigned.leg_1);
    let (Some(minimum_fee), Some(state_expansion_fee), Some(fee_charged)) = (
        minimum_fee_0.checked_add(minimum_fee_1),
        atomic_swap_leg_state_expansion_fee(ledger, &unsigned.leg_0)
            .checked_add(atomic_swap_leg_state_expansion_fee(ledger, &unsigned.leg_1)),
        unsigned.leg_0.fee.checked_add(unsigned.leg_1.fee),
    ) else {
        return receipt;
    };
    let signing_bytes = unsigned.signing_bytes();
    let (Ok(prepared_0), Ok(prepared_1)) = (
        prepare_atomic_swap_owner(
            ledger,
            &unsigned.leg_0,
            &transaction.authorization_0,
            &signing_bytes,
            minimum_fee_0,
        ),
        prepare_atomic_swap_owner(
            ledger,
            &unsigned.leg_1,
            &transaction.authorization_1,
            &signing_bytes,
            minimum_fee_1,
        ),
    ) else {
        return receipt;
    };
    // Owners differ (envelope validation), so the two debits are independent;
    // both accounts were read above, so neither application can fail.
    if apply_atomic_swap_owner_fee(
        ledger,
        &unsigned.leg_0,
        &transaction.authorization_0,
        &prepared_0,
    )
    .is_err()
        || apply_atomic_swap_owner_fee(
            ledger,
            &unsigned.leg_1,
            &transaction.authorization_1,
            &prepared_1,
        )
        .is_err()
    {
        return receipt;
    }
    fee_claimed_receipt(receipt, fee_charged, minimum_fee, state_expansion_fee)
        .with_atomic_swap_legs(vec![
            atomic_swap_leg_receipt(&unsigned.leg_0, prepared_0.pre_sequence),
            atomic_swap_leg_receipt(&unsigned.leg_1, prepared_1.pre_sequence),
        ])
}
