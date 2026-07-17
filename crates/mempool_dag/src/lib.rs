use postfiat_crypto_provider::hash_hex;
use postfiat_types::{
    FastLanePrimaryOperationV1, SignedAssetTransaction, SignedAtomicSwapTransaction,
    SignedEscrowTransaction, SignedNftTransaction, SignedOfferTransaction, SignedPaymentV2,
    SignedTransfer, TransactionBatch,
};
use serde::{Deserialize, Serialize};

pub const CRATE_PURPOSE: &str = "DAG batch dissemination and payload availability";
pub const MAX_BATCH_PAYLOAD_BYTES: usize = 1024 * 1024;
pub const MAX_BATCH_TRANSACTIONS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchReference {
    pub batch_id: String,
    pub payload_hash: String,
    pub transaction_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolBatchDomain {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableTransactionBatch {
    pub reference: BatchReference,
    pub batch: TransactionBatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailabilityError {
    message: String,
}

impl AvailabilityError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AvailabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AvailabilityError {}

pub fn build_transaction_batch(
    domain: &MempoolBatchDomain,
    transactions: Vec<SignedTransfer>,
) -> Result<AvailableTransactionBatch, AvailabilityError> {
    let reference = reference_for_transactions(domain, &transactions)?;
    let batch = TransactionBatch::new(reference.batch_id.clone(), transactions);
    Ok(AvailableTransactionBatch { reference, batch })
}

pub fn build_mixed_transaction_batch(
    domain: &MempoolBatchDomain,
    transactions: Vec<SignedTransfer>,
    payments_v2: Vec<SignedPaymentV2>,
) -> Result<AvailableTransactionBatch, AvailabilityError> {
    if payments_v2.is_empty() {
        return build_transaction_batch(domain, transactions);
    }
    let reference = reference_for_mixed_transactions(domain, &transactions, &payments_v2)?;
    let batch = TransactionBatch::new_with_payments_v2(
        reference.batch_id.clone(),
        transactions,
        payments_v2,
    );
    Ok(AvailableTransactionBatch { reference, batch })
}

pub fn build_mixed_transaction_batch_with_assets(
    domain: &MempoolBatchDomain,
    transactions: Vec<SignedTransfer>,
    payments_v2: Vec<SignedPaymentV2>,
    asset_transactions: Vec<SignedAssetTransaction>,
) -> Result<AvailableTransactionBatch, AvailabilityError> {
    if asset_transactions.is_empty() {
        return build_mixed_transaction_batch(domain, transactions, payments_v2);
    }
    let reference = reference_for_mixed_transactions_with_assets(
        domain,
        &transactions,
        &payments_v2,
        &asset_transactions,
    )?;
    let batch = TransactionBatch::new_with_asset_transactions(
        reference.batch_id.clone(),
        transactions,
        payments_v2,
        asset_transactions,
    );
    Ok(AvailableTransactionBatch { reference, batch })
}

pub fn build_mixed_transaction_batch_with_escrows(
    domain: &MempoolBatchDomain,
    transactions: Vec<SignedTransfer>,
    payments_v2: Vec<SignedPaymentV2>,
    asset_transactions: Vec<SignedAssetTransaction>,
    escrow_transactions: Vec<SignedEscrowTransaction>,
) -> Result<AvailableTransactionBatch, AvailabilityError> {
    if escrow_transactions.is_empty() {
        return build_mixed_transaction_batch_with_assets(
            domain,
            transactions,
            payments_v2,
            asset_transactions,
        );
    }
    let reference = reference_for_mixed_transactions_with_assets_and_escrows(
        domain,
        &transactions,
        &payments_v2,
        &asset_transactions,
        &escrow_transactions,
    )?;
    let batch = TransactionBatch::new_with_escrow_transactions(
        reference.batch_id.clone(),
        transactions,
        payments_v2,
        asset_transactions,
        escrow_transactions,
    );
    Ok(AvailableTransactionBatch { reference, batch })
}

pub fn build_mixed_transaction_batch_with_nfts(
    domain: &MempoolBatchDomain,
    transactions: Vec<SignedTransfer>,
    payments_v2: Vec<SignedPaymentV2>,
    asset_transactions: Vec<SignedAssetTransaction>,
    escrow_transactions: Vec<SignedEscrowTransaction>,
    nft_transactions: Vec<SignedNftTransaction>,
) -> Result<AvailableTransactionBatch, AvailabilityError> {
    if nft_transactions.is_empty() {
        return build_mixed_transaction_batch_with_escrows(
            domain,
            transactions,
            payments_v2,
            asset_transactions,
            escrow_transactions,
        );
    }
    let reference = reference_for_mixed_transactions_with_assets_escrows_and_nfts(
        domain,
        &transactions,
        &payments_v2,
        &asset_transactions,
        &escrow_transactions,
        &nft_transactions,
    )?;
    let batch = TransactionBatch::new_with_nft_transactions(
        reference.batch_id.clone(),
        transactions,
        payments_v2,
        asset_transactions,
        escrow_transactions,
        nft_transactions,
    );
    Ok(AvailableTransactionBatch { reference, batch })
}

pub fn build_mixed_transaction_batch_with_offers(
    domain: &MempoolBatchDomain,
    transactions: Vec<SignedTransfer>,
    payments_v2: Vec<SignedPaymentV2>,
    asset_transactions: Vec<SignedAssetTransaction>,
    escrow_transactions: Vec<SignedEscrowTransaction>,
    nft_transactions: Vec<SignedNftTransaction>,
    offer_transactions: Vec<SignedOfferTransaction>,
) -> Result<AvailableTransactionBatch, AvailabilityError> {
    if offer_transactions.is_empty() {
        return build_mixed_transaction_batch_with_nfts(
            domain,
            transactions,
            payments_v2,
            asset_transactions,
            escrow_transactions,
            nft_transactions,
        );
    }
    let reference = reference_for_mixed_transactions_with_assets_escrows_nfts_and_offers(
        domain,
        &transactions,
        &payments_v2,
        &asset_transactions,
        &escrow_transactions,
        &nft_transactions,
        &offer_transactions,
    )?;
    let batch = TransactionBatch::new_with_offer_transactions(
        reference.batch_id.clone(),
        transactions,
        payments_v2,
        asset_transactions,
        escrow_transactions,
        nft_transactions,
        offer_transactions,
    );
    Ok(AvailableTransactionBatch { reference, batch })
}

#[allow(clippy::too_many_arguments)]
pub fn build_mixed_transaction_batch_with_atomic_swaps(
    domain: &MempoolBatchDomain,
    transactions: Vec<SignedTransfer>,
    payments_v2: Vec<SignedPaymentV2>,
    asset_transactions: Vec<SignedAssetTransaction>,
    atomic_swap_transactions: Vec<SignedAtomicSwapTransaction>,
    escrow_transactions: Vec<SignedEscrowTransaction>,
    nft_transactions: Vec<SignedNftTransaction>,
    offer_transactions: Vec<SignedOfferTransaction>,
) -> Result<AvailableTransactionBatch, AvailabilityError> {
    if atomic_swap_transactions.is_empty() {
        return build_mixed_transaction_batch_with_offers(
            domain,
            transactions,
            payments_v2,
            asset_transactions,
            escrow_transactions,
            nft_transactions,
            offer_transactions,
        );
    }
    let reference = reference_for_mixed_transactions_with_atomic_swaps(
        domain,
        &transactions,
        &payments_v2,
        &asset_transactions,
        &atomic_swap_transactions,
        &escrow_transactions,
        &nft_transactions,
        &offer_transactions,
    )?;
    let batch = TransactionBatch::new_with_atomic_swap_transactions(
        reference.batch_id.clone(),
        transactions,
        payments_v2,
        asset_transactions,
        atomic_swap_transactions,
        escrow_transactions,
        nft_transactions,
        offer_transactions,
    );
    Ok(AvailableTransactionBatch { reference, batch })
}

pub fn reference_for_batch(
    domain: &MempoolBatchDomain,
    batch: &TransactionBatch,
) -> Result<BatchReference, AvailabilityError> {
    let reference = if !batch.fastlane_primary_transactions.is_empty() {
        reference_for_batch_with_fastlane_primary(domain, batch)?
    } else if !batch.atomic_swap_transactions.is_empty() {
        reference_for_mixed_transactions_with_atomic_swaps(
            domain,
            &batch.transactions,
            &batch.payments_v2,
            &batch.asset_transactions,
            &batch.atomic_swap_transactions,
            &batch.escrow_transactions,
            &batch.nft_transactions,
            &batch.offer_transactions,
        )?
    } else if !batch.offer_transactions.is_empty() {
        reference_for_mixed_transactions_with_assets_escrows_nfts_and_offers(
            domain,
            &batch.transactions,
            &batch.payments_v2,
            &batch.asset_transactions,
            &batch.escrow_transactions,
            &batch.nft_transactions,
            &batch.offer_transactions,
        )?
    } else if !batch.nft_transactions.is_empty() {
        reference_for_mixed_transactions_with_assets_escrows_and_nfts(
            domain,
            &batch.transactions,
            &batch.payments_v2,
            &batch.asset_transactions,
            &batch.escrow_transactions,
            &batch.nft_transactions,
        )?
    } else if !batch.escrow_transactions.is_empty() {
        reference_for_mixed_transactions_with_assets_and_escrows(
            domain,
            &batch.transactions,
            &batch.payments_v2,
            &batch.asset_transactions,
            &batch.escrow_transactions,
        )?
    } else if !batch.asset_transactions.is_empty() {
        reference_for_mixed_transactions_with_assets(
            domain,
            &batch.transactions,
            &batch.payments_v2,
            &batch.asset_transactions,
        )?
    } else if batch.payments_v2.is_empty() {
        reference_for_transactions(domain, &batch.transactions)?
    } else {
        reference_for_mixed_transactions(domain, &batch.transactions, &batch.payments_v2)?
    };
    if reference.batch_id != batch.batch_id {
        return Err(AvailabilityError::new(format!(
            "batch id mismatch expected {} got {}",
            reference.batch_id, batch.batch_id
        )));
    }
    Ok(reference)
}

pub fn attach_fastlane_primary_transactions(
    domain: &MempoolBatchDomain,
    mut batch: TransactionBatch,
    transactions: Vec<postfiat_types::FastLanePrimaryTransactionV1>,
) -> Result<AvailableTransactionBatch, AvailabilityError> {
    if transactions.is_empty() {
        let reference = reference_for_batch(domain, &batch)?;
        return Ok(AvailableTransactionBatch { reference, batch });
    }
    batch.fastlane_primary_transactions = transactions;
    let reference = reference_for_batch_with_fastlane_primary(domain, &batch)?;
    batch.batch_id = reference.batch_id.clone();
    Ok(AvailableTransactionBatch { reference, batch })
}

fn reference_for_batch_with_fastlane_primary(
    domain: &MempoolBatchDomain,
    batch: &TransactionBatch,
) -> Result<BatchReference, AvailabilityError> {
    validate_domain(domain)?;
    if batch.transaction_count() > MAX_BATCH_TRANSACTIONS {
        return Err(AvailabilityError::new(format!(
            "batch transaction count {} exceeds limit {}",
            batch.transaction_count(),
            MAX_BATCH_TRANSACTIONS
        )));
    }
    for (index, transaction) in batch.transactions.iter().enumerate() {
        validate_transaction_domain(domain, transaction, index)?;
    }
    for (index, payment) in batch.payments_v2.iter().enumerate() {
        validate_payment_v2_domain(domain, payment, index)?;
    }
    for (index, transaction) in batch.asset_transactions.iter().enumerate() {
        validate_asset_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in batch.atomic_swap_transactions.iter().enumerate() {
        validate_atomic_swap_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in batch.escrow_transactions.iter().enumerate() {
        validate_escrow_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in batch.nft_transactions.iter().enumerate() {
        validate_nft_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in batch.offer_transactions.iter().enumerate() {
        validate_offer_transaction_domain(domain, transaction, index)?;
    }
    for transaction in &batch.fastlane_primary_transactions {
        transaction.canonical_bytes().map_err(|error| {
            AvailabilityError::new(format!("invalid FastLane primary transaction: {error:?}"))
        })?;
        let chain_matches = match &transaction.operation {
            FastLanePrimaryOperationV1::Deposit { signed } => {
                fastlane_chain_domain_matches(domain, &signed.deposit.domain)
            }
            FastLanePrimaryOperationV1::OwnedDeposit { signed } => {
                fastlane_chain_domain_matches(domain, &signed.deposit.domain)
            }
            FastLanePrimaryOperationV1::Redeem { signed } => {
                fastlane_chain_domain_matches(domain, &signed.claim.committee.chain)
            }
            FastLanePrimaryOperationV1::AnchorCheckpoint { certificate } => {
                let chain = &certificate
                    .votes
                    .first()
                    .ok_or_else(|| AvailabilityError::new("empty FastLane checkpoint certificate"))?
                    .checkpoint
                    .committee
                    .chain;
                fastlane_chain_domain_matches(domain, chain)
            }
            FastLanePrimaryOperationV1::Control { certificate } => {
                let chain = &certificate
                    .votes
                    .first()
                    .ok_or_else(|| AvailabilityError::new("empty FastLane control certificate"))?
                    .committee
                    .chain;
                fastlane_chain_domain_matches(domain, chain)
            }
            FastLanePrimaryOperationV1::FastPayRecoveryReveal { certificate } => {
                let chain = certificate.domain();
                chain.chain_id == domain.chain_id
                    && chain.genesis_hash == domain.genesis_hash
                    && chain.protocol_version == domain.protocol_version
            }
            FastLanePrimaryOperationV1::FastPayRecoveryDecision { request } => {
                let chain = request.signed_order.domain();
                chain.chain_id == domain.chain_id
                    && chain.genesis_hash == domain.genesis_hash
                    && chain.protocol_version == domain.protocol_version
            }
        };
        if !chain_matches {
            return Err(AvailabilityError::new(
                "FastLane primary transaction domain mismatch",
            ));
        }
    }
    let payload = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        &batch.transactions,
        &batch.payments_v2,
        &batch.asset_transactions,
        &batch.atomic_swap_transactions,
        &batch.fastlane_primary_transactions,
        &batch.escrow_transactions,
        &batch.nft_transactions,
        &batch.offer_transactions,
    ))
    .map_err(|error| AvailabilityError::new(error.to_string()))?;
    if payload.len() > MAX_BATCH_PAYLOAD_BYTES {
        return Err(AvailabilityError::new(format!(
            "batch payload size {} exceeds limit {}",
            payload.len(),
            MAX_BATCH_PAYLOAD_BYTES
        )));
    }
    let payload_hash = hash_hex("postfiat.mempool.payload.fastlane.v1", &payload);
    let transaction_count = batch.transaction_count();
    let batch_id = hash_hex(
        "postfiat.mempool.batch_reference.fastlane.v1",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\npayload_hash={payload_hash}\ntransaction_count={transaction_count}\nfastlane_primary_count={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            batch.fastlane_primary_transactions.len(),
        )
        .as_bytes(),
    );
    Ok(BatchReference {
        batch_id,
        payload_hash,
        transaction_count: transaction_count as u64,
    })
}

fn fastlane_chain_domain_matches(
    domain: &MempoolBatchDomain,
    chain: &postfiat_types::FastSwapChainDomainV1,
) -> bool {
    chain.chain_id == domain.chain_id
        && postfiat_crypto_provider::bytes_to_hex(&chain.genesis_hash.0) == domain.genesis_hash
        && chain.protocol_version == domain.protocol_version
}

#[allow(clippy::too_many_arguments)]
fn reference_for_mixed_transactions_with_atomic_swaps(
    domain: &MempoolBatchDomain,
    transactions: &[SignedTransfer],
    payments_v2: &[SignedPaymentV2],
    asset_transactions: &[SignedAssetTransaction],
    atomic_swap_transactions: &[SignedAtomicSwapTransaction],
    escrow_transactions: &[SignedEscrowTransaction],
    nft_transactions: &[SignedNftTransaction],
    offer_transactions: &[SignedOfferTransaction],
) -> Result<BatchReference, AvailabilityError> {
    validate_domain(domain)?;
    let transaction_count = transactions
        .len()
        .saturating_add(payments_v2.len())
        .saturating_add(asset_transactions.len())
        .saturating_add(atomic_swap_transactions.len())
        .saturating_add(escrow_transactions.len())
        .saturating_add(nft_transactions.len())
        .saturating_add(offer_transactions.len());
    if transaction_count > MAX_BATCH_TRANSACTIONS {
        return Err(AvailabilityError::new(format!(
            "batch transaction count {} exceeds limit {}",
            transaction_count, MAX_BATCH_TRANSACTIONS
        )));
    }
    for (index, transaction) in transactions.iter().enumerate() {
        validate_transaction_domain(domain, transaction, index)?;
    }
    for (index, payment) in payments_v2.iter().enumerate() {
        validate_payment_v2_domain(domain, payment, index)?;
    }
    for (index, transaction) in asset_transactions.iter().enumerate() {
        validate_asset_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in atomic_swap_transactions.iter().enumerate() {
        validate_atomic_swap_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in escrow_transactions.iter().enumerate() {
        validate_escrow_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in nft_transactions.iter().enumerate() {
        validate_nft_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in offer_transactions.iter().enumerate() {
        validate_offer_transaction_domain(domain, transaction, index)?;
    }
    let payload = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        transactions,
        payments_v2,
        asset_transactions,
        atomic_swap_transactions,
        escrow_transactions,
        nft_transactions,
        offer_transactions,
    ))
    .map_err(|error| AvailabilityError::new(error.to_string()))?;
    if payload.len() > MAX_BATCH_PAYLOAD_BYTES {
        return Err(AvailabilityError::new(format!(
            "batch payload size {} exceeds limit {}",
            payload.len(),
            MAX_BATCH_PAYLOAD_BYTES
        )));
    }
    let payload_hash = hash_hex("postfiat.mempool.payload.v8", &payload);
    let batch_id = hash_hex(
        "postfiat.mempool.batch_reference.v7",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\npayload_hash={payload_hash}\ntransaction_count={transaction_count}\nlegacy_transfer_count={}\npayment_v2_count={}\nasset_transaction_count={}\natomic_swap_transaction_count={}\nescrow_transaction_count={}\nnft_transaction_count={}\noffer_transaction_count={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            transactions.len(),
            payments_v2.len(),
            asset_transactions.len(),
            atomic_swap_transactions.len(),
            escrow_transactions.len(),
            nft_transactions.len(),
            offer_transactions.len()
        )
        .as_bytes(),
    );
    Ok(BatchReference {
        batch_id,
        payload_hash,
        transaction_count: transaction_count as u64,
    })
}

pub fn verify_batch_payload(
    domain: &MempoolBatchDomain,
    batch: &TransactionBatch,
    reference: &BatchReference,
) -> Result<(), AvailabilityError> {
    let actual = reference_for_batch(domain, batch)?;
    if &actual != reference {
        return Err(AvailabilityError::new(format!(
            "batch reference mismatch expected {} got {}",
            reference.batch_id, actual.batch_id
        )));
    }
    Ok(())
}

fn reference_for_mixed_transactions_with_assets_and_escrows(
    domain: &MempoolBatchDomain,
    transactions: &[SignedTransfer],
    payments_v2: &[SignedPaymentV2],
    asset_transactions: &[SignedAssetTransaction],
    escrow_transactions: &[SignedEscrowTransaction],
) -> Result<BatchReference, AvailabilityError> {
    validate_domain(domain)?;
    let transaction_count = transactions
        .len()
        .saturating_add(payments_v2.len())
        .saturating_add(asset_transactions.len())
        .saturating_add(escrow_transactions.len());
    if transaction_count > MAX_BATCH_TRANSACTIONS {
        return Err(AvailabilityError::new(format!(
            "batch transaction count {} exceeds limit {}",
            transaction_count, MAX_BATCH_TRANSACTIONS
        )));
    }
    for (index, transaction) in transactions.iter().enumerate() {
        validate_transaction_domain(domain, transaction, index)?;
    }
    for (index, payment) in payments_v2.iter().enumerate() {
        validate_payment_v2_domain(domain, payment, index)?;
    }
    for (index, transaction) in asset_transactions.iter().enumerate() {
        validate_asset_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in escrow_transactions.iter().enumerate() {
        validate_escrow_transaction_domain(domain, transaction, index)?;
    }
    let payload = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        transactions,
        payments_v2,
        asset_transactions,
        escrow_transactions,
    ))
    .map_err(|error| AvailabilityError::new(error.to_string()))?;
    if payload.len() > MAX_BATCH_PAYLOAD_BYTES {
        return Err(AvailabilityError::new(format!(
            "batch payload size {} exceeds limit {}",
            payload.len(),
            MAX_BATCH_PAYLOAD_BYTES
        )));
    }
    let payload_hash = hash_hex("postfiat.mempool.payload.v5", &payload);
    let batch_id = hash_hex(
        "postfiat.mempool.batch_reference.v4",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\npayload_hash={payload_hash}\ntransaction_count={transaction_count}\nlegacy_transfer_count={}\npayment_v2_count={}\nasset_transaction_count={}\nescrow_transaction_count={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            transactions.len(),
            payments_v2.len(),
            asset_transactions.len(),
            escrow_transactions.len()
        )
        .as_bytes(),
    );
    Ok(BatchReference {
        batch_id,
        payload_hash,
        transaction_count: transaction_count as u64,
    })
}

fn reference_for_mixed_transactions_with_assets_escrows_and_nfts(
    domain: &MempoolBatchDomain,
    transactions: &[SignedTransfer],
    payments_v2: &[SignedPaymentV2],
    asset_transactions: &[SignedAssetTransaction],
    escrow_transactions: &[SignedEscrowTransaction],
    nft_transactions: &[SignedNftTransaction],
) -> Result<BatchReference, AvailabilityError> {
    validate_domain(domain)?;
    let transaction_count = transactions
        .len()
        .saturating_add(payments_v2.len())
        .saturating_add(asset_transactions.len())
        .saturating_add(escrow_transactions.len())
        .saturating_add(nft_transactions.len());
    if transaction_count > MAX_BATCH_TRANSACTIONS {
        return Err(AvailabilityError::new(format!(
            "batch transaction count {} exceeds limit {}",
            transaction_count, MAX_BATCH_TRANSACTIONS
        )));
    }
    for (index, transaction) in transactions.iter().enumerate() {
        validate_transaction_domain(domain, transaction, index)?;
    }
    for (index, payment) in payments_v2.iter().enumerate() {
        validate_payment_v2_domain(domain, payment, index)?;
    }
    for (index, transaction) in asset_transactions.iter().enumerate() {
        validate_asset_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in escrow_transactions.iter().enumerate() {
        validate_escrow_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in nft_transactions.iter().enumerate() {
        validate_nft_transaction_domain(domain, transaction, index)?;
    }
    let payload = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        transactions,
        payments_v2,
        asset_transactions,
        escrow_transactions,
        nft_transactions,
    ))
    .map_err(|error| AvailabilityError::new(error.to_string()))?;
    if payload.len() > MAX_BATCH_PAYLOAD_BYTES {
        return Err(AvailabilityError::new(format!(
            "batch payload size {} exceeds limit {}",
            payload.len(),
            MAX_BATCH_PAYLOAD_BYTES
        )));
    }
    let payload_hash = hash_hex("postfiat.mempool.payload.v6", &payload);
    let batch_id = hash_hex(
        "postfiat.mempool.batch_reference.v5",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\npayload_hash={payload_hash}\ntransaction_count={transaction_count}\nlegacy_transfer_count={}\npayment_v2_count={}\nasset_transaction_count={}\nescrow_transaction_count={}\nnft_transaction_count={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            transactions.len(),
            payments_v2.len(),
            asset_transactions.len(),
            escrow_transactions.len(),
            nft_transactions.len()
        )
        .as_bytes(),
    );
    Ok(BatchReference {
        batch_id,
        payload_hash,
        transaction_count: transaction_count as u64,
    })
}

fn reference_for_mixed_transactions_with_assets_escrows_nfts_and_offers(
    domain: &MempoolBatchDomain,
    transactions: &[SignedTransfer],
    payments_v2: &[SignedPaymentV2],
    asset_transactions: &[SignedAssetTransaction],
    escrow_transactions: &[SignedEscrowTransaction],
    nft_transactions: &[SignedNftTransaction],
    offer_transactions: &[SignedOfferTransaction],
) -> Result<BatchReference, AvailabilityError> {
    validate_domain(domain)?;
    let transaction_count = transactions
        .len()
        .saturating_add(payments_v2.len())
        .saturating_add(asset_transactions.len())
        .saturating_add(escrow_transactions.len())
        .saturating_add(nft_transactions.len())
        .saturating_add(offer_transactions.len());
    if transaction_count > MAX_BATCH_TRANSACTIONS {
        return Err(AvailabilityError::new(format!(
            "batch transaction count {} exceeds limit {}",
            transaction_count, MAX_BATCH_TRANSACTIONS
        )));
    }
    for (index, transaction) in transactions.iter().enumerate() {
        validate_transaction_domain(domain, transaction, index)?;
    }
    for (index, payment) in payments_v2.iter().enumerate() {
        validate_payment_v2_domain(domain, payment, index)?;
    }
    for (index, transaction) in asset_transactions.iter().enumerate() {
        validate_asset_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in escrow_transactions.iter().enumerate() {
        validate_escrow_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in nft_transactions.iter().enumerate() {
        validate_nft_transaction_domain(domain, transaction, index)?;
    }
    for (index, transaction) in offer_transactions.iter().enumerate() {
        validate_offer_transaction_domain(domain, transaction, index)?;
    }
    let payload = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        transactions,
        payments_v2,
        asset_transactions,
        escrow_transactions,
        nft_transactions,
        offer_transactions,
    ))
    .map_err(|error| AvailabilityError::new(error.to_string()))?;
    if payload.len() > MAX_BATCH_PAYLOAD_BYTES {
        return Err(AvailabilityError::new(format!(
            "batch payload size {} exceeds limit {}",
            payload.len(),
            MAX_BATCH_PAYLOAD_BYTES
        )));
    }
    let payload_hash = hash_hex("postfiat.mempool.payload.v7", &payload);
    let batch_id = hash_hex(
        "postfiat.mempool.batch_reference.v6",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\npayload_hash={payload_hash}\ntransaction_count={transaction_count}\nlegacy_transfer_count={}\npayment_v2_count={}\nasset_transaction_count={}\nescrow_transaction_count={}\nnft_transaction_count={}\noffer_transaction_count={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            transactions.len(),
            payments_v2.len(),
            asset_transactions.len(),
            escrow_transactions.len(),
            nft_transactions.len(),
            offer_transactions.len()
        )
        .as_bytes(),
    );
    Ok(BatchReference {
        batch_id,
        payload_hash,
        transaction_count: transaction_count as u64,
    })
}

fn reference_for_mixed_transactions_with_assets(
    domain: &MempoolBatchDomain,
    transactions: &[SignedTransfer],
    payments_v2: &[SignedPaymentV2],
    asset_transactions: &[SignedAssetTransaction],
) -> Result<BatchReference, AvailabilityError> {
    validate_domain(domain)?;
    let transaction_count = transactions
        .len()
        .saturating_add(payments_v2.len())
        .saturating_add(asset_transactions.len());
    if transaction_count > MAX_BATCH_TRANSACTIONS {
        return Err(AvailabilityError::new(format!(
            "batch transaction count {} exceeds limit {}",
            transaction_count, MAX_BATCH_TRANSACTIONS
        )));
    }
    for (index, transaction) in transactions.iter().enumerate() {
        validate_transaction_domain(domain, transaction, index)?;
    }
    for (index, payment) in payments_v2.iter().enumerate() {
        validate_payment_v2_domain(domain, payment, index)?;
    }
    for (index, transaction) in asset_transactions.iter().enumerate() {
        validate_asset_transaction_domain(domain, transaction, index)?;
    }
    let payload = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        transactions,
        payments_v2,
        asset_transactions,
    ))
    .map_err(|error| AvailabilityError::new(error.to_string()))?;
    if payload.len() > MAX_BATCH_PAYLOAD_BYTES {
        return Err(AvailabilityError::new(format!(
            "batch payload size {} exceeds limit {}",
            payload.len(),
            MAX_BATCH_PAYLOAD_BYTES
        )));
    }
    let payload_hash = hash_hex("postfiat.mempool.payload.v4", &payload);
    let batch_id = hash_hex(
        "postfiat.mempool.batch_reference.v3",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\npayload_hash={payload_hash}\ntransaction_count={transaction_count}\nlegacy_transfer_count={}\npayment_v2_count={}\nasset_transaction_count={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            transactions.len(),
            payments_v2.len(),
            asset_transactions.len()
        )
        .as_bytes(),
    );
    Ok(BatchReference {
        batch_id,
        payload_hash,
        transaction_count: transaction_count as u64,
    })
}

fn reference_for_mixed_transactions(
    domain: &MempoolBatchDomain,
    transactions: &[SignedTransfer],
    payments_v2: &[SignedPaymentV2],
) -> Result<BatchReference, AvailabilityError> {
    validate_domain(domain)?;
    let transaction_count = transactions.len().saturating_add(payments_v2.len());
    if transaction_count > MAX_BATCH_TRANSACTIONS {
        return Err(AvailabilityError::new(format!(
            "batch transaction count {} exceeds limit {}",
            transaction_count, MAX_BATCH_TRANSACTIONS
        )));
    }
    for (index, transaction) in transactions.iter().enumerate() {
        validate_transaction_domain(domain, transaction, index)?;
    }
    for (index, payment) in payments_v2.iter().enumerate() {
        validate_payment_v2_domain(domain, payment, index)?;
    }
    let payload = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        transactions,
        payments_v2,
    ))
    .map_err(|error| AvailabilityError::new(error.to_string()))?;
    if payload.len() > MAX_BATCH_PAYLOAD_BYTES {
        return Err(AvailabilityError::new(format!(
            "batch payload size {} exceeds limit {}",
            payload.len(),
            MAX_BATCH_PAYLOAD_BYTES
        )));
    }
    let payload_hash = hash_hex("postfiat.mempool.payload.v3", &payload);
    let batch_id = hash_hex(
        "postfiat.mempool.batch_reference.v2",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\npayload_hash={payload_hash}\ntransaction_count={transaction_count}\nlegacy_transfer_count={}\npayment_v2_count={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            transactions.len(),
            payments_v2.len()
        )
        .as_bytes(),
    );
    Ok(BatchReference {
        batch_id,
        payload_hash,
        transaction_count: transaction_count as u64,
    })
}

fn reference_for_transactions(
    domain: &MempoolBatchDomain,
    transactions: &[SignedTransfer],
) -> Result<BatchReference, AvailabilityError> {
    validate_domain(domain)?;
    if transactions.len() > MAX_BATCH_TRANSACTIONS {
        return Err(AvailabilityError::new(format!(
            "batch transaction count {} exceeds limit {}",
            transactions.len(),
            MAX_BATCH_TRANSACTIONS
        )));
    }
    for (index, transaction) in transactions.iter().enumerate() {
        validate_transaction_domain(domain, transaction, index)?;
    }
    let payload = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        transactions,
    ))
    .map_err(|error| AvailabilityError::new(error.to_string()))?;
    if payload.len() > MAX_BATCH_PAYLOAD_BYTES {
        return Err(AvailabilityError::new(format!(
            "batch payload size {} exceeds limit {}",
            payload.len(),
            MAX_BATCH_PAYLOAD_BYTES
        )));
    }
    let payload_hash = hash_hex("postfiat.mempool.payload.v2", &payload);
    let batch_id = hash_hex(
        "postfiat.mempool.batch_reference.v1",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\npayload_hash={payload_hash}\ntransaction_count={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            transactions.len()
        )
        .as_bytes(),
    );
    Ok(BatchReference {
        batch_id,
        payload_hash,
        transaction_count: transactions.len() as u64,
    })
}

fn validate_domain(domain: &MempoolBatchDomain) -> Result<(), AvailabilityError> {
    if domain.chain_id.trim().is_empty() {
        return Err(AvailabilityError::new("batch domain chain_id is empty"));
    }
    if domain.genesis_hash.trim().is_empty() {
        return Err(AvailabilityError::new("batch domain genesis_hash is empty"));
    }
    if !is_lower_hex_len(&domain.genesis_hash, 96) {
        return Err(AvailabilityError::new(
            "batch domain genesis_hash must be 96 lowercase hex characters",
        ));
    }
    if domain.protocol_version == 0 {
        return Err(AvailabilityError::new(
            "batch domain protocol_version must be nonzero",
        ));
    }
    Ok(())
}

fn validate_payment_v2_domain(
    domain: &MempoolBatchDomain,
    payment: &SignedPaymentV2,
    index: usize,
) -> Result<(), AvailabilityError> {
    payment
        .validate()
        .map_err(|error| AvailabilityError::new(format!("batch payment_v2 {index}: {error}")))?;
    if payment.unsigned.chain_id != domain.chain_id {
        return Err(AvailabilityError::new(format!(
            "batch payment_v2 {index} chain_id mismatch expected {} got {}",
            domain.chain_id, payment.unsigned.chain_id
        )));
    }
    if payment.unsigned.genesis_hash != domain.genesis_hash {
        return Err(AvailabilityError::new(format!(
            "batch payment_v2 {index} genesis_hash mismatch expected {} got {}",
            domain.genesis_hash, payment.unsigned.genesis_hash
        )));
    }
    if payment.unsigned.protocol_version != domain.protocol_version {
        return Err(AvailabilityError::new(format!(
            "batch payment_v2 {index} protocol_version mismatch expected {} got {}",
            domain.protocol_version, payment.unsigned.protocol_version
        )));
    }
    Ok(())
}

fn validate_asset_transaction_domain(
    domain: &MempoolBatchDomain,
    transaction: &SignedAssetTransaction,
    index: usize,
) -> Result<(), AvailabilityError> {
    transaction.validate().map_err(|error| {
        AvailabilityError::new(format!("batch asset transaction {index}: {error}"))
    })?;
    if transaction.unsigned.chain_id != domain.chain_id {
        return Err(AvailabilityError::new(format!(
            "batch asset transaction {index} chain_id mismatch expected {} got {}",
            domain.chain_id, transaction.unsigned.chain_id
        )));
    }
    if transaction.unsigned.genesis_hash != domain.genesis_hash {
        return Err(AvailabilityError::new(format!(
            "batch asset transaction {index} genesis_hash mismatch expected {} got {}",
            domain.genesis_hash, transaction.unsigned.genesis_hash
        )));
    }
    if transaction.unsigned.protocol_version != domain.protocol_version {
        return Err(AvailabilityError::new(format!(
            "batch asset transaction {index} protocol_version mismatch expected {} got {}",
            domain.protocol_version, transaction.unsigned.protocol_version
        )));
    }
    Ok(())
}

fn validate_atomic_swap_transaction_domain(
    domain: &MempoolBatchDomain,
    transaction: &SignedAtomicSwapTransaction,
    index: usize,
) -> Result<(), AvailabilityError> {
    transaction.validate().map_err(|error| {
        AvailabilityError::new(format!("batch atomic swap transaction {index}: {error}"))
    })?;
    if transaction.unsigned.chain_id != domain.chain_id {
        return Err(AvailabilityError::new(format!(
            "batch atomic swap transaction {index} chain_id mismatch expected {} got {}",
            domain.chain_id, transaction.unsigned.chain_id
        )));
    }
    if transaction.unsigned.genesis_hash != domain.genesis_hash {
        return Err(AvailabilityError::new(format!(
            "batch atomic swap transaction {index} genesis_hash mismatch expected {} got {}",
            domain.genesis_hash, transaction.unsigned.genesis_hash
        )));
    }
    if transaction.unsigned.protocol_version != domain.protocol_version {
        return Err(AvailabilityError::new(format!(
            "batch atomic swap transaction {index} protocol_version mismatch expected {} got {}",
            domain.protocol_version, transaction.unsigned.protocol_version
        )));
    }
    Ok(())
}

fn validate_escrow_transaction_domain(
    domain: &MempoolBatchDomain,
    transaction: &SignedEscrowTransaction,
    index: usize,
) -> Result<(), AvailabilityError> {
    transaction.validate().map_err(|error| {
        AvailabilityError::new(format!("batch escrow transaction {index}: {error}"))
    })?;
    if transaction.unsigned.chain_id != domain.chain_id {
        return Err(AvailabilityError::new(format!(
            "batch escrow transaction {index} chain_id mismatch expected {} got {}",
            domain.chain_id, transaction.unsigned.chain_id
        )));
    }
    if transaction.unsigned.genesis_hash != domain.genesis_hash {
        return Err(AvailabilityError::new(format!(
            "batch escrow transaction {index} genesis_hash mismatch expected {} got {}",
            domain.genesis_hash, transaction.unsigned.genesis_hash
        )));
    }
    if transaction.unsigned.protocol_version != domain.protocol_version {
        return Err(AvailabilityError::new(format!(
            "batch escrow transaction {index} protocol_version mismatch expected {} got {}",
            domain.protocol_version, transaction.unsigned.protocol_version
        )));
    }
    Ok(())
}

fn validate_nft_transaction_domain(
    domain: &MempoolBatchDomain,
    transaction: &SignedNftTransaction,
    index: usize,
) -> Result<(), AvailabilityError> {
    transaction.validate().map_err(|error| {
        AvailabilityError::new(format!("batch nft transaction {index}: {error}"))
    })?;
    if transaction.unsigned.chain_id != domain.chain_id {
        return Err(AvailabilityError::new(format!(
            "batch nft transaction {index} chain_id mismatch expected {} got {}",
            domain.chain_id, transaction.unsigned.chain_id
        )));
    }
    if transaction.unsigned.genesis_hash != domain.genesis_hash {
        return Err(AvailabilityError::new(format!(
            "batch nft transaction {index} genesis_hash mismatch expected {} got {}",
            domain.genesis_hash, transaction.unsigned.genesis_hash
        )));
    }
    if transaction.unsigned.protocol_version != domain.protocol_version {
        return Err(AvailabilityError::new(format!(
            "batch nft transaction {index} protocol_version mismatch expected {} got {}",
            domain.protocol_version, transaction.unsigned.protocol_version
        )));
    }
    Ok(())
}

fn validate_offer_transaction_domain(
    domain: &MempoolBatchDomain,
    transaction: &SignedOfferTransaction,
    index: usize,
) -> Result<(), AvailabilityError> {
    transaction.validate().map_err(|error| {
        AvailabilityError::new(format!("batch offer transaction {index}: {error}"))
    })?;
    if transaction.unsigned.chain_id != domain.chain_id {
        return Err(AvailabilityError::new(format!(
            "batch offer transaction {index} chain_id mismatch expected {} got {}",
            domain.chain_id, transaction.unsigned.chain_id
        )));
    }
    if transaction.unsigned.genesis_hash != domain.genesis_hash {
        return Err(AvailabilityError::new(format!(
            "batch offer transaction {index} genesis_hash mismatch expected {} got {}",
            domain.genesis_hash, transaction.unsigned.genesis_hash
        )));
    }
    if transaction.unsigned.protocol_version != domain.protocol_version {
        return Err(AvailabilityError::new(format!(
            "batch offer transaction {index} protocol_version mismatch expected {} got {}",
            domain.protocol_version, transaction.unsigned.protocol_version
        )));
    }
    Ok(())
}

fn validate_transaction_domain(
    domain: &MempoolBatchDomain,
    transaction: &SignedTransfer,
    index: usize,
) -> Result<(), AvailabilityError> {
    transaction
        .validate()
        .map_err(|error| AvailabilityError::new(format!("batch transaction {index}: {error}")))?;
    if transaction.unsigned.chain_id != domain.chain_id {
        return Err(AvailabilityError::new(format!(
            "batch transaction {index} chain_id mismatch expected {} got {}",
            domain.chain_id, transaction.unsigned.chain_id
        )));
    }
    if transaction.unsigned.genesis_hash != domain.genesis_hash {
        return Err(AvailabilityError::new(format!(
            "batch transaction {index} genesis_hash mismatch expected {} got {}",
            domain.genesis_hash, transaction.unsigned.genesis_hash
        )));
    }
    if transaction.unsigned.protocol_version != domain.protocol_version {
        return Err(AvailabilityError::new(format!(
            "batch transaction {index} protocol_version mismatch expected {} got {}",
            domain.protocol_version, transaction.unsigned.protocol_version
        )));
    }
    Ok(())
}

fn is_lower_hex_len(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_types::{
        AssetCreateOperation, AssetTransactionOperation, AtomicSwapAuthorization, AtomicSwapLeg,
        EscrowCreateOperation, EscrowTransactionOperation, NftMintOperation,
        NftTransactionOperation, OfferCreateOperation, OfferTransactionOperation,
        SignedAssetTransaction, SignedAtomicSwapTransaction, SignedEscrowTransaction,
        SignedNftTransaction, SignedOfferTransaction, UnsignedAssetTransaction,
        UnsignedAtomicSwapTransaction, UnsignedEscrowTransaction, UnsignedNftTransaction,
        UnsignedOfferTransaction, UnsignedTransfer, ADDRESS_NAMESPACE,
        ASSET_CREATE_TRANSACTION_KIND, ESCROW_CREATE_TRANSACTION_KIND, ISSUED_ASSET_ID_HEX_LEN,
        MAX_TRANSFER_PUBLIC_KEY_HEX_LEN, MAX_TRANSFER_SIGNATURE_HEX_LEN, NFT_MINT_TRANSACTION_KIND,
        OFFER_CREATE_TRANSACTION_KIND, TRANSFER_TRANSACTION_KIND,
    };

    const TEST_GENESIS_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn batch_domain(chain_id: &str) -> MempoolBatchDomain {
        MempoolBatchDomain {
            chain_id: chain_id.to_string(),
            genesis_hash: TEST_GENESIS_HASH.to_string(),
            protocol_version: 1,
        }
    }

    fn signed(sequence: u64) -> SignedTransfer {
        SignedTransfer {
            unsigned: UnsignedTransfer {
                chain_id: "postfiat-local".to_string(),
                genesis_hash: TEST_GENESIS_HASH.to_string(),
                protocol_version: 1,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
                signature_algorithm_id: "test".to_string(),
                from: "pfsource".to_string(),
                to: "pfdest".to_string(),
                amount: 10,
                fee: 1,
                sequence,
            },
            algorithm_id: "test".to_string(),
            public_key_hex: "00".to_string(),
            signature_hex: "11".to_string(),
        }
    }

    fn heavy_signed(sequence: u64) -> SignedTransfer {
        let mut transaction = signed(sequence);
        transaction.public_key_hex = "aa".repeat(MAX_TRANSFER_PUBLIC_KEY_HEX_LEN / 2);
        transaction.signature_hex = "bb".repeat(MAX_TRANSFER_SIGNATURE_HEX_LEN / 2);
        transaction
    }

    fn signed_asset(sequence: u64) -> SignedAssetTransaction {
        SignedAssetTransaction {
            unsigned: UnsignedAssetTransaction {
                chain_id: "postfiat-local".to_string(),
                genesis_hash: TEST_GENESIS_HASH.to_string(),
                protocol_version: 1,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: ASSET_CREATE_TRANSACTION_KIND.to_string(),
                signature_algorithm_id: "test".to_string(),
                source: "pfissuer".to_string(),
                fee: 1,
                sequence,
                operation: AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                    issuer: "pfissuer".to_string(),
                    code: "USD".to_string(),
                    version: 1,
                    precision: 6,
                    display_name: String::new(),
                    max_supply: Some(1_000),
                    requires_authorization: false,
                    freeze_enabled: true,
                    clawback_enabled: false,
                }),
            },
            algorithm_id: "test".to_string(),
            public_key_hex: "00".to_string(),
            signature_hex: "11".to_string(),
        }
    }

    fn signed_atomic_swap() -> SignedAtomicSwapTransaction {
        let owner_0 = format!("pf{}", "01".repeat(20));
        let owner_1 = format!("pf{}", "02".repeat(20));
        SignedAtomicSwapTransaction {
            unsigned: UnsignedAtomicSwapTransaction {
                chain_id: "postfiat-local".to_string(),
                genesis_hash: TEST_GENESIS_HASH.to_string(),
                protocol_version: 1,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                signature_algorithm_id: "test".to_string(),
                rfq_hash: "03".repeat(48),
                market_envelope_hash: "00".repeat(48),
                nav_epoch: 0,
                expires_at_height: 25,
                swap_nonce: "04".repeat(48),
                leg_0: AtomicSwapLeg {
                    owner: owner_0.clone(),
                    recipient: owner_1.clone(),
                    issuer: format!("pf{}", "05".repeat(20)),
                    asset_id: "06".repeat(48),
                    amount: 10,
                    sequence: 1,
                    fee: 1,
                },
                leg_1: AtomicSwapLeg {
                    owner: owner_1.clone(),
                    recipient: owner_0.clone(),
                    issuer: format!("pf{}", "07".repeat(20)),
                    asset_id: "08".repeat(48),
                    amount: 20,
                    sequence: 1,
                    fee: 1,
                },
            },
            authorization_0: AtomicSwapAuthorization {
                owner: owner_0,
                algorithm_id: "test".to_string(),
                public_key_hex: "09".to_string(),
                signature_hex: "0a".to_string(),
            },
            authorization_1: AtomicSwapAuthorization {
                owner: owner_1,
                algorithm_id: "test".to_string(),
                public_key_hex: "0b".to_string(),
                signature_hex: "0c".to_string(),
            },
        }
    }

    fn signed_escrow(sequence: u64) -> SignedEscrowTransaction {
        SignedEscrowTransaction {
            unsigned: UnsignedEscrowTransaction {
                chain_id: "postfiat-local".to_string(),
                genesis_hash: TEST_GENESIS_HASH.to_string(),
                protocol_version: 1,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: ESCROW_CREATE_TRANSACTION_KIND.to_string(),
                signature_algorithm_id: "test".to_string(),
                source: "pfowner".to_string(),
                fee: 1,
                sequence,
                operation: EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
                    owner: "pfowner".to_string(),
                    recipient: "pfrecipient".to_string(),
                    asset_id: "PFT".to_string(),
                    amount: 10,
                    condition: "time_lock".to_string(),
                    finish_after: 10,
                    cancel_after: 20,
                }),
            },
            algorithm_id: "test".to_string(),
            public_key_hex: "00".to_string(),
            signature_hex: "11".to_string(),
        }
    }

    fn signed_nft(sequence: u64) -> SignedNftTransaction {
        SignedNftTransaction {
            unsigned: UnsignedNftTransaction {
                chain_id: "postfiat-local".to_string(),
                genesis_hash: TEST_GENESIS_HASH.to_string(),
                protocol_version: 1,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: NFT_MINT_TRANSACTION_KIND.to_string(),
                signature_algorithm_id: "test".to_string(),
                source: "pfissuer".to_string(),
                fee: 1,
                sequence,
                operation: NftTransactionOperation::NftMint(NftMintOperation {
                    issuer: "pfissuer".to_string(),
                    collection_id: "ART".to_string(),
                    serial: 1,
                    owner: "pfowner".to_string(),
                    metadata_hash: "aa".to_string(),
                    metadata_uri: String::new(),
                    flags: 1,
                    collection_flags: 0,
                    issuer_transfer_fee: 0,
                }),
            },
            algorithm_id: "test".to_string(),
            public_key_hex: "00".to_string(),
            signature_hex: "11".to_string(),
        }
    }

    fn signed_offer(sequence: u64) -> SignedOfferTransaction {
        SignedOfferTransaction {
            unsigned: UnsignedOfferTransaction {
                chain_id: "postfiat-local".to_string(),
                genesis_hash: TEST_GENESIS_HASH.to_string(),
                protocol_version: 1,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: OFFER_CREATE_TRANSACTION_KIND.to_string(),
                signature_algorithm_id: "test".to_string(),
                source: "pfowner".to_string(),
                fee: 1,
                sequence,
                operation: OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                    owner: "pfowner".to_string(),
                    taker_gets_asset_id: "PFT".to_string(),
                    taker_gets_amount: 10,
                    taker_pays_asset_id: "01".repeat(ISSUED_ASSET_ID_HEX_LEN / 2),
                    taker_pays_amount: 4,
                    expiration_height: 25,
                }),
            },
            algorithm_id: "test".to_string(),
            public_key_hex: "00".to_string(),
            signature_hex: "11".to_string(),
        }
    }

    #[test]
    fn builds_deterministic_references() {
        let domain = batch_domain("postfiat-local");
        let first = build_transaction_batch(&domain, vec![signed(1)]).expect("build first");
        let second = build_transaction_batch(&domain, vec![signed(1)]).expect("build second");

        assert_eq!(first.reference, second.reference);
        assert_eq!(
            first.reference.batch_id,
            "71ae23fe7ed18554c5e6f70eb8704fd6c403165c33eb2a8aa646766ff0b7195858072902be0dbf450f2cd88ab9e38608"
        );
        assert_eq!(
            first.reference.payload_hash,
            "f015485c48ecc67a0a3c33770b488e1fc07df57690ef900e1f6151de903757a251ebd008a9eabc48f9d5190c28f5fe8b"
        );
        assert_eq!(first.batch.batch_id, first.reference.batch_id);
        assert_eq!(first.reference.transaction_count, 1);
        verify_batch_payload(&domain, &first.batch, &first.reference).expect("payload available");
    }

    #[test]
    fn asset_transactions_are_committed_to_batch_reference() {
        let domain = batch_domain("postfiat-local");
        let available = build_mixed_transaction_batch_with_assets(
            &domain,
            Vec::new(),
            Vec::new(),
            vec![signed_asset(1)],
        )
        .expect("asset batch");
        assert_eq!(available.reference.transaction_count, 1);
        assert_eq!(available.batch.asset_transactions.len(), 1);
        assert_eq!(
            reference_for_batch(&domain, &available.batch).expect("reference"),
            available.reference
        );

        let mut tampered = available.batch.clone();
        tampered.asset_transactions[0].unsigned.sequence = 2;
        assert!(reference_for_batch(&domain, &tampered).is_err());
    }

    #[test]
    fn atomic_swap_transactions_use_new_reference_domain_and_cover_signed_bytes() {
        let domain = batch_domain("postfiat-local");
        let available = build_mixed_transaction_batch_with_atomic_swaps(
            &domain,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![signed_atomic_swap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("atomic swap batch");
        assert_eq!(available.reference.transaction_count, 1);
        assert_eq!(available.batch.atomic_swap_transactions.len(), 1);
        assert_eq!(
            reference_for_batch(&domain, &available.batch).expect("reference"),
            available.reference
        );

        let mut amount_tampered = available.batch.clone();
        amount_tampered.atomic_swap_transactions[0]
            .unsigned
            .leg_0
            .amount += 1;
        assert!(reference_for_batch(&domain, &amount_tampered).is_err());

        let mut authorization_tampered = available.batch.clone();
        authorization_tampered.atomic_swap_transactions[0]
            .authorization_0
            .signature_hex = "0d".to_string();
        assert!(reference_for_batch(&domain, &authorization_tampered).is_err());
    }

    #[test]
    fn atomic_swap_batch_keeps_fixed_family_order() {
        let domain = batch_domain("postfiat-local");
        let available = build_mixed_transaction_batch_with_atomic_swaps(
            &domain,
            Vec::new(),
            Vec::new(),
            vec![signed_asset(1)],
            vec![signed_atomic_swap()],
            vec![signed_escrow(1)],
            Vec::new(),
            Vec::new(),
        )
        .expect("mixed atomic swap batch");
        assert_eq!(available.reference.transaction_count, 3);
        let json = serde_json::to_string(&available.batch).expect("serialize mixed batch");
        let asset_index = json.find("asset_transactions").expect("asset family");
        let atomic_index = json
            .find("atomic_swap_transactions")
            .expect("atomic family");
        let escrow_index = json.find("escrow_transactions").expect("escrow family");
        assert!(asset_index < atomic_index && atomic_index < escrow_index);
    }

    #[test]
    fn empty_atomic_swap_family_delegates_to_legacy_reference_path() {
        let domain = batch_domain("postfiat-local");
        let legacy = build_transaction_batch(&domain, vec![signed(1)]).expect("legacy batch");
        let delegated = build_mixed_transaction_batch_with_atomic_swaps(
            &domain,
            vec![signed(1)],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("delegated legacy batch");
        assert_eq!(delegated, legacy);
        assert!(!serde_json::to_string(&delegated.batch)
            .expect("serialize delegated batch")
            .contains("atomic_swap_transactions"));
    }

    #[test]
    fn escrow_transactions_are_committed_to_batch_reference() {
        let domain = batch_domain("postfiat-local");
        let available = build_mixed_transaction_batch_with_escrows(
            &domain,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![signed_escrow(1)],
        )
        .expect("escrow batch");
        assert_eq!(available.reference.transaction_count, 1);
        assert_eq!(available.batch.escrow_transactions.len(), 1);
        assert_eq!(
            reference_for_batch(&domain, &available.batch).expect("reference"),
            available.reference
        );

        let mut tampered = available.batch.clone();
        tampered.escrow_transactions[0].unsigned.sequence = 2;
        assert!(reference_for_batch(&domain, &tampered).is_err());
    }

    #[test]
    fn nft_transactions_are_committed_to_batch_reference() {
        let domain = batch_domain("postfiat-local");
        let available = build_mixed_transaction_batch_with_nfts(
            &domain,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![signed_nft(1)],
        )
        .expect("nft batch");
        assert_eq!(available.reference.transaction_count, 1);
        assert_eq!(available.batch.nft_transactions.len(), 1);
        assert_eq!(
            reference_for_batch(&domain, &available.batch).expect("reference"),
            available.reference
        );

        let mut tampered = available.batch.clone();
        tampered.nft_transactions[0].unsigned.sequence = 2;
        assert!(reference_for_batch(&domain, &tampered).is_err());
    }

    #[test]
    fn offer_transactions_are_committed_to_batch_reference() {
        let domain = batch_domain("postfiat-local");
        let available = build_mixed_transaction_batch_with_offers(
            &domain,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![signed_offer(1)],
        )
        .expect("offer batch");
        assert_eq!(available.reference.transaction_count, 1);
        assert_eq!(available.batch.offer_transactions.len(), 1);
        assert_eq!(
            reference_for_batch(&domain, &available.batch).expect("reference"),
            available.reference
        );

        let mut tampered = available.batch.clone();
        tampered.offer_transactions[0].unsigned.sequence = 2;
        assert!(reference_for_batch(&domain, &tampered).is_err());
    }

    #[test]
    fn reference_commits_to_chain_domain() {
        let local = build_transaction_batch(&batch_domain("postfiat-local"), Vec::new())
            .expect("local batch");
        let other = build_transaction_batch(&batch_domain("postfiat-other"), Vec::new())
            .expect("other batch");

        assert_ne!(local.reference.payload_hash, other.reference.payload_hash);
        assert_ne!(local.reference.batch_id, other.reference.batch_id);
    }

    #[test]
    fn rejects_tampered_batch_id() {
        let domain = batch_domain("postfiat-local");
        let mut available = build_transaction_batch(&domain, vec![signed(1)]).expect("build batch");
        available.batch.batch_id = "bad".to_string();

        assert!(reference_for_batch(&domain, &available.batch).is_err());
    }

    #[test]
    fn rejects_malformed_batch_domain() {
        let mut domain = batch_domain("postfiat-local");
        domain.chain_id = " ".to_string();
        assert!(build_transaction_batch(&domain, Vec::new()).is_err());

        let mut domain = batch_domain("postfiat-local");
        domain.genesis_hash.clear();
        assert!(build_transaction_batch(&domain, Vec::new()).is_err());

        let mut domain = batch_domain("postfiat-local");
        domain.genesis_hash = "not-a-genesis-hash".to_string();
        assert!(build_transaction_batch(&domain, Vec::new()).is_err());

        let mut domain = batch_domain("postfiat-local");
        domain.protocol_version = 0;
        assert!(build_transaction_batch(&domain, Vec::new()).is_err());
    }

    #[test]
    fn rejects_batches_above_transaction_limit() {
        let domain = batch_domain("postfiat-local");
        let transactions = (0..=MAX_BATCH_TRANSACTIONS)
            .map(|index| signed(index as u64 + 1))
            .collect::<Vec<_>>();

        assert!(build_transaction_batch(&domain, transactions).is_err());
    }

    #[test]
    fn rejects_malformed_batch_transactions() {
        let domain = batch_domain("postfiat-local");
        let mut transaction = signed(1);
        transaction.public_key_hex = "not-lower-hex".to_string();

        assert!(build_transaction_batch(&domain, vec![transaction]).is_err());
    }

    #[test]
    fn rejects_cross_domain_batch_transactions() {
        let domain = batch_domain("postfiat-local");
        let mut transaction = signed(1);
        transaction.unsigned.chain_id = "postfiat-other".to_string();

        assert!(build_transaction_batch(&domain, vec![transaction]).is_err());
    }

    #[test]
    fn rejects_batches_above_payload_limit() {
        let domain = batch_domain("postfiat-local");
        let transactions = (0..100)
            .map(|index| heavy_signed(index as u64 + 1))
            .collect::<Vec<_>>();

        assert!(build_transaction_batch(&domain, transactions).is_err());
    }
}
