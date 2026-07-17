use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hash_hex, hex_to_bytes, ml_dsa_65_verify,
    ML_DSA_65_ALGORITHM,
};
use postfiat_types::{
    escrow_id, market_ops_asset_id, market_ops_evidence_root, market_ops_reserve_packet_hash,
    market_ops_supply_packet_hash, nav_redemption_id, nft_id, offer_id,
    pftl_uniswap_non_consumption_proof_hash, pftl_uniswap_return_burn_id_from_fields,
    vault_bridge_deposit_evidence_root, vault_bridge_deposit_observation_root,
    vault_bridge_deposit_public_values_hash,
    vault_bridge_counted_value_for_asset, vault_bridge_source_root_for_asset,
    validate_nav_reserve_collateralization_with_unit_scale,
    vault_bridge_withdrawal_execution_observation_root,
    vault_bridge_withdrawal_packet_legacy_domainless_evm_digest,
    vault_bridge_withdrawal_packet_legacy_domainless_hash, AssetDefinition,
    AssetTransactionOperation, Escrow, EscrowCreateOperation, EscrowTransactionOperation,
    FinalizedMarketOpsEnvelope, Genesis, IssuedPaymentOperation, LedgerState,
    MarketOpsAlignmentParams, MarketOpsEnvelope, MarketOpsFinalizeOperation, MarketOpsMintLimits,
    MarketOpsReserveDeployLimits, MarketOpsVenueObservation, NavAttestor, NavMintAtNavOperation,
    NavProofProfile, NavRedeemSettleOperation, NavRedemption, NavReserveAttestation, NavReservePacket,
    NavReserveSubmitOperation, NavTrackedAsset, NftDefinition, NftTransactionOperation, NftTransferOperation, Offer,
    OfferCreateOperation, OfferFillReceipt, OfferTransactionOperation, PftlUniswapConsensusExportPacket,
    PftlUniswapConsensusReceipt, PftlUniswapConsensusReturnImport, PftlUniswapConsensusRouteState,
    PftlUniswapDestinationConsumeOperation, PftlUniswapExportDebitOperation,
    PftlUniswapPrimarySubscribeOperation, PftlUniswapRefundSourceOperation,
    PftlUniswapReturnImportOperation, PftlUniswapRouteInitOperation, VaultBridgeAllocation,
    VaultBridgeBucketImpairOperation, VaultBridgeBucketState, VaultBridgeBurnToRedeemOperation,
    VaultBridgeDepositAttestOperation, VaultBridgeDepositAttestation, VaultBridgeDepositEvidence,
    VaultBridgeDepositObservation,
    VaultBridgeDepositChallengeOperation, VaultBridgeDepositClaimOperation,
    VaultBridgeDepositFinalizeOperation,
    VaultBridgeDepositProposeOperation, VaultBridgeDepositRecord,
    VaultBridgeMintFromReceiptsOperation, VaultBridgeNavSubscriptionAllocateOperation, VaultBridgeReceipt,
    VaultBridgeReceiptCountOperation, VaultBridgeReceiptSubmitOperation, VaultBridgeRedeemSettleOperation,
    VaultBridgeRedemption, VaultBridgeWithdrawalExecutionAttestation,
    AtomicSwapAuthorization, AtomicSwapLeg, AtomicSwapLegReceipt, Receipt,
    SignedAssetTransaction, SignedAtomicSwapTransaction, SignedEscrowTransaction,
    SignedNftTransaction, SignedOfferTransaction, SignedPaymentV2, SignedTransfer, TrustLine,
    ADDRESS_NAMESPACE, ASSET_BURN_TRANSACTION_KIND, ASSET_CLAWBACK_TRANSACTION_KIND,
    ASSET_CREATE_TRANSACTION_KIND, ESCROW_CANCEL_TRANSACTION_KIND, ESCROW_CREATE_TRANSACTION_KIND,
    ESCROW_FINISH_TRANSACTION_KIND, ESCROW_STATE_CANCELED, ESCROW_STATE_FINISHED,
    ESCROW_STATE_OPEN, ISSUED_PAYMENT_TRANSACTION_KIND, MARKET_OPS_FINALIZE_TRANSACTION_KIND,
    MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND,
    MAX_DEX_CROSSES_PER_TRANSACTION, MAX_NAV_ATTESTATIONS_PER_PACKET,
    NAV_ASSET_REGISTER_TRANSACTION_KIND, NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
    NAV_EPOCH_FINALIZE_TRANSACTION_KIND, NAV_HALT_TRANSACTION_KIND,
    NAV_MINT_AT_NAV_TRANSACTION_KIND, NAV_PROFILE_ID_HEX_LEN, NAV_PROFILE_REGISTER_TRANSACTION_KIND,
    NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT, NAV_PROFILE_VERIFIER_MULTI_FETCH,
    NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1, NAV_PROFILE_VERIFIER_SP1_GROTH16,
    NAV_REDEEM_AT_NAV_TRANSACTION_KIND,
    NAV_REDEEM_SETTLE_TRANSACTION_KIND, NAV_REDEMPTION_STATE_PENDING,
    NAV_REDEMPTION_STATE_SETTLED, NAV_RESERVE_ATTEST_TRANSACTION_KIND,
    NAV_RESERVE_CHALLENGE_TRANSACTION_KIND, NAV_RESERVE_STATE_CHALLENGED,
    NAV_RESERVE_STATE_FINALIZED, NAV_RESERVE_STATE_SUBMITTED, NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
    NFT_BURN_TRANSACTION_KIND, NFT_COLLECTION_FLAG_BURN_LOCKED, NFT_COLLECTION_FLAG_TRANSFER_LOCKED,
    NFT_FLAG_TRANSFERABLE, NFT_MINT_TRANSACTION_KIND, NFT_TRANSFER_TRANSACTION_KIND,
    OFFER_CANCEL_TRANSACTION_KIND, OFFER_CREATE_TRANSACTION_KIND, OFFER_OBJECT_RESERVE,
    OFFER_STATE_CANCELED, OFFER_STATE_FILLED, OFFER_STATE_OPEN, PAYMENT_V2_TRANSACTION_KIND,
    MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS, MAX_PFTL_UNISWAP_ROUTES_PER_NATIVE_ISSUER,
    PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND, PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
    PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED, PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED,
    PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED, PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
    PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND, PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
    PFTL_UNISWAP_RETURN_STATUS_IMPORTED, PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT,
    PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
    VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION, VAULT_BRIDGE_ALLOCATION_PURPOSE_REDEMPTION,
    VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,
    VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
    VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
    VAULT_BRIDGE_DEPOSIT_CHALLENGE_TRANSACTION_KIND,
    VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
    VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND, VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX,
    VAULT_BRIDGE_DEPOSIT_STATUS_CHALLENGED,
    VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED, VAULT_BRIDGE_DEPOSIT_STATUS_PENDING,
    VAULT_BRIDGE_BUCKET_IMPAIR_TRANSACTION_KIND, VAULT_BRIDGE_BUCKET_STATUS_ACTIVE,
    VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED, VAULT_BRIDGE_BUCKET_STATUS_PAUSED,
    VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND, VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT,
    VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND,
    VAULT_BRIDGE_MINT_FROM_RECEIPTS_TRANSACTION_KIND, VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX,
    VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND, VAULT_BRIDGE_RECEIPT_STATUS_COUNTED,
    VAULT_BRIDGE_RECEIPT_STATUS_FINALIZED, VAULT_BRIDGE_RECEIPT_STATUS_IMPAIRED,
    VAULT_BRIDGE_RECEIPT_STATUS_PENDING, VAULT_BRIDGE_RECEIPT_STATUS_REJECTED,
    VAULT_BRIDGE_RECEIPT_STATUS_RETIRED, VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND,
    VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND, VAULT_BRIDGE_REDEMPTION_STATE_PENDING,
    VAULT_BRIDGE_REDEMPTION_STATE_SETTLED, VAULT_BRIDGE_UNIT, TRANSFER_TRANSACTION_KIND,
    TRUST_SET_TRANSACTION_KIND,
};

pub const CRATE_PURPOSE: &str = "deterministic state transition and receipts";
pub const MIN_TRANSFER_FEE: u64 = 1;
pub const TRANSFER_FEE_BYTE_QUANTUM: usize = 512;
pub const TRANSFER_FEE_PER_QUANTUM: u64 = 1;
pub const ACCOUNT_RESERVE: u64 = 10;
pub const TRANSFER_ACCOUNT_CREATION_FEE: u64 = 10;
pub const ASSET_DEFINITION_STATE_EXPANSION_FEE: u64 = 20;
pub const TRUSTLINE_STATE_EXPANSION_FEE: u64 = 10;
pub const ESCROW_STATE_EXPANSION_FEE: u64 = 10;
pub const NFT_STATE_EXPANSION_FEE: u64 = 10;
pub const OFFER_STATE_EXPANSION_FEE: u64 = 10;
pub const OFFER_MATCH_CROSS_FEE: u64 = 1;
pub const NATIVE_PFT_ESCROW_ASSET_ID: &str = "PFT";
// Historical reserved sink address. Fees burn at execution and are not credited here.
pub const FEE_COLLECTOR_ADDRESS: &str = "pffee000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq)]
struct OfferApplyOutcome {
    receipt_code: &'static str,
    offer_id: Option<String>,
    fills: Vec<OfferFillReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OfferMatchPlan {
    fills: Vec<OfferFillPlan>,
    taker_gets_remaining: u64,
    taker_pays_remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OfferFillPlan {
    maker_offer_index: usize,
    maker_sends_amount: u64,
    taker_sends_amount: u64,
}

pub fn execute_transfer(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transfer: &SignedTransfer,
) -> Receipt {
    let tx_id = transfer_tx_id(transfer);

    if let Err(error) = transfer.validate() {
        return Receipt::rejected(tx_id, "bad_transfer_envelope", error);
    }

    if transfer.unsigned.chain_id != genesis.chain_id {
        return Receipt::rejected(
            tx_id,
            "wrong_chain",
            format!(
                "expected chain `{}`, got `{}`",
                genesis.chain_id, transfer.unsigned.chain_id
            ),
        );
    }

    let expected_genesis_hash = genesis_hash(genesis);
    if transfer.unsigned.genesis_hash != expected_genesis_hash {
        return Receipt::rejected(
            tx_id,
            "wrong_genesis",
            format!(
                "expected genesis `{expected_genesis_hash}`, got `{}`",
                transfer.unsigned.genesis_hash
            ),
        );
    }

    if transfer.unsigned.protocol_version != genesis.protocol_version {
        return Receipt::rejected(
            tx_id,
            "wrong_protocol_version",
            format!(
                "expected protocol version {}, got {}",
                genesis.protocol_version, transfer.unsigned.protocol_version
            ),
        );
    }

    if transfer.unsigned.address_namespace != ADDRESS_NAMESPACE {
        return Receipt::rejected(
            tx_id,
            "wrong_address_namespace",
            format!(
                "expected address namespace `{ADDRESS_NAMESPACE}`, got `{}`",
                transfer.unsigned.address_namespace
            ),
        );
    }

    if transfer.unsigned.transaction_kind != TRANSFER_TRANSACTION_KIND {
        return Receipt::rejected(
            tx_id,
            "wrong_transaction_kind",
            format!(
                "expected transaction kind `{TRANSFER_TRANSACTION_KIND}`, got `{}`",
                transfer.unsigned.transaction_kind
            ),
        );
    }

    if transfer.unsigned.signature_algorithm_id != ML_DSA_65_ALGORITHM {
        return Receipt::rejected(
            tx_id,
            "unsupported_signature_algorithm",
            format!(
                "unsupported algorithm `{}`",
                transfer.unsigned.signature_algorithm_id
            ),
        );
    }

    if transfer.algorithm_id != transfer.unsigned.signature_algorithm_id {
        return Receipt::rejected(
            tx_id,
            "signature_algorithm_mismatch",
            "signed envelope algorithm does not match signed payload",
        );
    }

    if transfer.unsigned.amount == 0 {
        return Receipt::rejected(tx_id, "zero_amount", "transfer amount must be nonzero");
    }

    let state_expansion_fee = transfer_state_expansion_fee(ledger, transfer);
    let minimum_fee = minimum_transfer_fee_for_ledger(ledger, transfer);
    if transfer.unsigned.fee < minimum_fee {
        return Receipt::rejected(
            tx_id,
            "fee_too_low",
            format!("minimum transfer fee is {minimum_fee}"),
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    }

    let public_key = match hex_to_bytes(&transfer.public_key_hex) {
        Ok(public_key) => public_key,
        Err(error) => {
            return Receipt::rejected(tx_id, "bad_public_key", error.to_string());
        }
    };
    let signature = match hex_to_bytes(&transfer.signature_hex) {
        Ok(signature) => signature,
        Err(error) => {
            return Receipt::rejected(tx_id, "bad_signature", error.to_string());
        }
    };

    let derived_from = address_from_public_key(&public_key);
    if derived_from != transfer.unsigned.from {
        return Receipt::rejected(
            tx_id,
            "sender_mismatch",
            "sender address does not match public key",
        );
    }

    if !ml_dsa_65_verify(&public_key, &transfer.unsigned.signing_bytes(), &signature) {
        return Receipt::rejected(tx_id, "bad_signature", "signature verification failed");
    }

    let Some(sender) = ledger.account(&transfer.unsigned.from) else {
        return Receipt::rejected(tx_id, "missing_sender", "sender account does not exist");
    };

    if let Some(sender_public_key_hex) = sender.public_key_hex.as_deref() {
        if sender_public_key_hex != transfer.public_key_hex.as_str() {
            return Receipt::rejected(
                tx_id,
                "sender_key_mismatch",
                "sender public key does not match ledger account",
            );
        }
    }

    let Some(expected_sequence) = sender.sequence.checked_add(1) else {
        return Receipt::rejected(
            tx_id,
            "sequence_overflow",
            "sender sequence is exhausted",
        );
    };
    if transfer.unsigned.sequence != expected_sequence {
        return Receipt::rejected(
            tx_id,
            "bad_sequence",
            format!(
                "expected sequence {expected_sequence}, got {}",
                transfer.unsigned.sequence
            ),
        );
    }

    let total_debit = match transfer.unsigned.amount.checked_add(transfer.unsigned.fee) {
        Some(total) => total,
        None => return Receipt::rejected(tx_id, "amount_overflow", "amount plus fee overflowed"),
    };

    if sender.balance < total_debit {
        return Receipt::rejected(
            tx_id,
            "insufficient_funds",
            "sender balance is too low for amount plus fee",
        );
    }

    let sender_after_debit = sender.balance - total_debit;
    let recipient_base = if transfer.unsigned.to == transfer.unsigned.from {
        sender_after_debit
    } else {
        ledger
            .account(&transfer.unsigned.to)
            .map(|account| account.balance)
            .unwrap_or_default()
    };
    let Some(recipient_after_credit) = recipient_base.checked_add(transfer.unsigned.amount) else {
        return Receipt::rejected(
            tx_id,
            "balance_overflow",
            "recipient balance would overflow",
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    };

    let sender_final_balance = if transfer.unsigned.from == transfer.unsigned.to {
        recipient_after_credit
    } else {
        sender_after_debit
    };
    if let Some(message) = account_reserve_violation(&transfer.unsigned.from, sender_final_balance)
    {
        return Receipt::rejected(tx_id, "below_account_reserve", message)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    if transfer.unsigned.to != transfer.unsigned.from {
        if let Some(message) =
            account_reserve_violation(&transfer.unsigned.to, recipient_after_credit)
        {
            return Receipt::rejected(tx_id, "below_account_reserve", message)
                .with_fee_policy_and_state_expansion(
                    0,
                    0,
                    minimum_fee,
                    ACCOUNT_RESERVE,
                    state_expansion_fee,
                );
        }
    }

    let Some(sender) = ledger.account_mut(&transfer.unsigned.from) else {
        return Receipt::rejected(
            tx_id,
            "missing_sender",
            "sender account does not exist before state mutation",
        );
    };
    if sender.public_key_hex.is_none() {
        sender.public_key_hex = Some(transfer.public_key_hex.clone());
    }
    sender.balance = sender_after_debit;
    sender.sequence = expected_sequence;

    let recipient = ledger.ensure_account(&transfer.unsigned.to);
    recipient.balance = recipient_after_credit;

    Receipt::accepted(tx_id, "transfer applied; fee burned").with_fee_policy_and_state_expansion(
        transfer.unsigned.fee,
        transfer.unsigned.fee,
        minimum_fee,
        ACCOUNT_RESERVE,
        state_expansion_fee,
    )
}

pub fn execute_payment_v2(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    payment: &SignedPaymentV2,
) -> Receipt {
    let tx_id = payment_v2_tx_id(payment);

    if let Err(error) = payment.validate() {
        return Receipt::rejected(tx_id, "bad_payment_envelope", error);
    }

    if payment.unsigned.chain_id != genesis.chain_id {
        return Receipt::rejected(
            tx_id,
            "wrong_chain",
            format!(
                "expected chain `{}`, got `{}`",
                genesis.chain_id, payment.unsigned.chain_id
            ),
        );
    }

    let expected_genesis_hash = genesis_hash(genesis);
    if payment.unsigned.genesis_hash != expected_genesis_hash {
        return Receipt::rejected(
            tx_id,
            "wrong_genesis",
            format!(
                "expected genesis `{expected_genesis_hash}`, got `{}`",
                payment.unsigned.genesis_hash
            ),
        );
    }

    if payment.unsigned.protocol_version != genesis.protocol_version {
        return Receipt::rejected(
            tx_id,
            "wrong_protocol_version",
            format!(
                "expected protocol version {}, got {}",
                genesis.protocol_version, payment.unsigned.protocol_version
            ),
        );
    }

    if payment.unsigned.address_namespace != ADDRESS_NAMESPACE {
        return Receipt::rejected(
            tx_id,
            "wrong_address_namespace",
            format!(
                "expected address namespace `{ADDRESS_NAMESPACE}`, got `{}`",
                payment.unsigned.address_namespace
            ),
        );
    }

    if payment.unsigned.transaction_kind != PAYMENT_V2_TRANSACTION_KIND {
        return Receipt::rejected(
            tx_id,
            "wrong_transaction_kind",
            format!(
                "expected transaction kind `{PAYMENT_V2_TRANSACTION_KIND}`, got `{}`",
                payment.unsigned.transaction_kind
            ),
        );
    }

    if payment.unsigned.signature_algorithm_id != ML_DSA_65_ALGORITHM {
        return Receipt::rejected(
            tx_id,
            "unsupported_signature_algorithm",
            format!(
                "unsupported algorithm `{}`",
                payment.unsigned.signature_algorithm_id
            ),
        );
    }

    if payment.algorithm_id != payment.unsigned.signature_algorithm_id {
        return Receipt::rejected(
            tx_id,
            "signature_algorithm_mismatch",
            "signed envelope algorithm does not match signed payload",
        );
    }

    let state_expansion_fee = payment_v2_state_expansion_fee(ledger, payment);
    let minimum_fee = minimum_payment_v2_fee_for_ledger(ledger, payment);
    if payment.unsigned.fee < minimum_fee {
        return Receipt::rejected(
            tx_id,
            "fee_too_low",
            format!("minimum payment fee is {minimum_fee}"),
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    }

    let public_key = match hex_to_bytes(&payment.public_key_hex) {
        Ok(public_key) => public_key,
        Err(error) => {
            return Receipt::rejected(tx_id, "bad_public_key", error.to_string());
        }
    };
    let signature = match hex_to_bytes(&payment.signature_hex) {
        Ok(signature) => signature,
        Err(error) => {
            return Receipt::rejected(tx_id, "bad_signature", error.to_string());
        }
    };

    let derived_from = address_from_public_key(&public_key);
    if derived_from != payment.unsigned.from {
        return Receipt::rejected(
            tx_id,
            "sender_mismatch",
            "sender address does not match public key",
        );
    }

    if !ml_dsa_65_verify(&public_key, &payment.unsigned.signing_bytes(), &signature) {
        return Receipt::rejected(tx_id, "bad_signature", "signature verification failed");
    }

    let Some(sender) = ledger.account(&payment.unsigned.from) else {
        return Receipt::rejected(tx_id, "missing_sender", "sender account does not exist");
    };

    if let Some(sender_public_key_hex) = sender.public_key_hex.as_deref() {
        if sender_public_key_hex != payment.public_key_hex.as_str() {
            return Receipt::rejected(
                tx_id,
                "sender_key_mismatch",
                "sender public key does not match ledger account",
            );
        }
    }

    let Some(expected_sequence) = sender.sequence.checked_add(1) else {
        return Receipt::rejected(
            tx_id,
            "sequence_overflow",
            "sender sequence is exhausted",
        );
    };
    if payment.unsigned.sequence != expected_sequence {
        return Receipt::rejected(
            tx_id,
            "bad_sequence",
            format!(
                "expected sequence {expected_sequence}, got {}",
                payment.unsigned.sequence
            ),
        );
    }

    let total_debit = match payment.unsigned.amount.checked_add(payment.unsigned.fee) {
        Some(total) => total,
        None => return Receipt::rejected(tx_id, "amount_overflow", "amount plus fee overflowed"),
    };

    if sender.balance < total_debit {
        return Receipt::rejected(
            tx_id,
            "insufficient_funds",
            "sender balance is too low for amount plus fee",
        );
    }

    let sender_after_debit = sender.balance - total_debit;
    let recipient_base = if payment.unsigned.to == payment.unsigned.from {
        sender_after_debit
    } else {
        ledger
            .account(&payment.unsigned.to)
            .map(|account| account.balance)
            .unwrap_or_default()
    };
    let Some(recipient_after_credit) = recipient_base.checked_add(payment.unsigned.amount) else {
        return Receipt::rejected(
            tx_id,
            "balance_overflow",
            "recipient balance would overflow",
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    };

    let sender_final_balance = if payment.unsigned.from == payment.unsigned.to {
        recipient_after_credit
    } else {
        sender_after_debit
    };
    if let Some(message) = account_reserve_violation(&payment.unsigned.from, sender_final_balance) {
        return Receipt::rejected(tx_id, "below_account_reserve", message)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    if payment.unsigned.to != payment.unsigned.from {
        if let Some(message) =
            account_reserve_violation(&payment.unsigned.to, recipient_after_credit)
        {
            return Receipt::rejected(tx_id, "below_account_reserve", message)
                .with_fee_policy_and_state_expansion(
                    0,
                    0,
                    minimum_fee,
                    ACCOUNT_RESERVE,
                    state_expansion_fee,
                );
        }
    }

    let Some(sender) = ledger.account_mut(&payment.unsigned.from) else {
        return Receipt::rejected(
            tx_id,
            "missing_sender",
            "sender account does not exist before state mutation",
        );
    };
    if sender.public_key_hex.is_none() {
        sender.public_key_hex = Some(payment.public_key_hex.clone());
    }
    sender.balance = sender_after_debit;
    sender.sequence = expected_sequence;

    let recipient = ledger.ensure_account(&payment.unsigned.to);
    recipient.balance = recipient_after_credit;

    Receipt::accepted(tx_id, "payment_v2 applied; fee burned").with_fee_policy_and_state_expansion(
        payment.unsigned.fee,
        payment.unsigned.fee,
        minimum_fee,
        ACCOUNT_RESERVE,
        state_expansion_fee,
    )
}

pub fn execute_asset_transaction(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
) -> Receipt {
    let tx_id = asset_transaction_tx_id(transaction);
    let signing_bytes = transaction.unsigned.signing_bytes();
    execute_asset_transaction_with_checked_preimage(
        genesis,
        ledger,
        transaction,
        block_height,
        tx_id,
        &signing_bytes,
        AssetExecutionCompatibility::strict(),
    )
}

#[cfg(test)]
pub(crate) fn execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
) -> Receipt {
    execute_asset_transaction_with_compatibility(
        genesis,
        ledger,
        transaction,
        block_height,
        AssetExecutionCompatibility::strict().with_unverified_pftl_uniswap_bridge_fixture(),
    )
}

pub fn execute_asset_transaction_with_compatibility(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    let tx_id = asset_transaction_tx_id(transaction);
    let signing_bytes = transaction.unsigned.signing_bytes();
    execute_asset_transaction_with_checked_preimage(
        genesis,
        ledger,
        transaction,
        block_height,
        tx_id,
        &signing_bytes,
        compatibility,
    )
}

pub fn execute_asset_transaction_with_replay_preimage(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
    tx_id: String,
    signing_bytes: &[u8],
) -> Receipt {
    execute_asset_transaction_with_checked_preimage(
        genesis,
        ledger,
        transaction,
        block_height,
        tx_id,
        signing_bytes,
        AssetExecutionCompatibility::strict(),
    )
}

pub fn execute_asset_transaction_with_replay_preimage_and_compatibility(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
    tx_id: String,
    signing_bytes: &[u8],
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    execute_asset_transaction_with_checked_preimage(
        genesis,
        ledger,
        transaction,
        block_height,
        tx_id,
        signing_bytes,
        compatibility,
    )
}

pub fn execute_asset_transaction_with_replay_compatibility(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    let tx_id = asset_transaction_tx_id(transaction);
    let signing_bytes = transaction.unsigned.signing_bytes();
    execute_asset_transaction_with_checked_preimage(
        genesis,
        ledger,
        transaction,
        block_height,
        tx_id,
        &signing_bytes,
        compatibility,
    )
}

fn execute_asset_transaction_with_checked_preimage(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
    tx_id: String,
    signing_bytes: &[u8],
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    let allow_legacy_vault_bridge_consume_supply_operator =
        !compatibility.bridge_verification_rules_active(block_height);
    if let Err(error) = transaction
        .validate_with_legacy_vault_bridge_consume_supply_operator(
            allow_legacy_vault_bridge_consume_supply_operator,
        )
    {
        return Receipt::rejected(tx_id, "bad_asset_transaction_envelope", error);
    }

    if transaction.unsigned.chain_id != genesis.chain_id {
        return Receipt::rejected(
            tx_id,
            "wrong_chain",
            format!(
                "expected chain `{}`, got `{}`",
                genesis.chain_id, transaction.unsigned.chain_id
            ),
        );
    }

    let expected_genesis_hash = genesis_hash(genesis);
    if transaction.unsigned.genesis_hash != expected_genesis_hash {
        return Receipt::rejected(
            tx_id,
            "wrong_genesis",
            format!(
                "expected genesis `{expected_genesis_hash}`, got `{}`",
                transaction.unsigned.genesis_hash
            ),
        );
    }

    if transaction.unsigned.protocol_version != genesis.protocol_version {
        return Receipt::rejected(
            tx_id,
            "wrong_protocol_version",
            format!(
                "expected protocol version {}, got {}",
                genesis.protocol_version, transaction.unsigned.protocol_version
            ),
        );
    }

    if transaction.unsigned.address_namespace != ADDRESS_NAMESPACE {
        return Receipt::rejected(
            tx_id,
            "wrong_address_namespace",
            format!(
                "expected address namespace `{ADDRESS_NAMESPACE}`, got `{}`",
                transaction.unsigned.address_namespace
            ),
        );
    }

    if transaction.unsigned.signature_algorithm_id != ML_DSA_65_ALGORITHM {
        return Receipt::rejected(
            tx_id,
            "unsupported_signature_algorithm",
            format!(
                "unsupported algorithm `{}`",
                transaction.unsigned.signature_algorithm_id
            ),
        );
    }

    if transaction.algorithm_id != transaction.unsigned.signature_algorithm_id {
        return Receipt::rejected(
            tx_id,
            "signature_algorithm_mismatch",
            "signed envelope algorithm does not match signed payload",
        );
    }

    let public_key = match hex_to_bytes(&transaction.public_key_hex) {
        Ok(public_key) => public_key,
        Err(error) => return Receipt::rejected(tx_id, "bad_public_key", error.to_string()),
    };
    let signature = match hex_to_bytes(&transaction.signature_hex) {
        Ok(signature) => signature,
        Err(error) => return Receipt::rejected(tx_id, "bad_signature", error.to_string()),
    };

    let derived_source = address_from_public_key(&public_key);
    if derived_source != transaction.unsigned.source {
        return Receipt::rejected(
            tx_id,
            "sender_mismatch",
            "asset transaction source does not match public key",
        );
    }

    if !ml_dsa_65_verify(&public_key, signing_bytes, &signature) {
        return Receipt::rejected(tx_id, "bad_signature", "signature verification failed");
    }

    let Some(source) = ledger.account(&transaction.unsigned.source) else {
        return Receipt::rejected(tx_id, "missing_sender", "source account does not exist");
    };

    if let Some(source_public_key_hex) = source.public_key_hex.as_deref() {
        if source_public_key_hex != transaction.public_key_hex.as_str() {
            return Receipt::rejected(
                tx_id,
                "sender_key_mismatch",
                "source public key does not match ledger account",
            );
        }
    }

    let Some(expected_sequence) = source.sequence.checked_add(1) else {
        return Receipt::rejected(
            tx_id,
            "sequence_overflow",
            "source sequence is exhausted",
        );
    };
    if transaction.unsigned.sequence != expected_sequence {
        return Receipt::rejected(
            tx_id,
            "bad_sequence",
            format!(
                "expected sequence {expected_sequence}, got {}",
                transaction.unsigned.sequence
            ),
        );
    }

    let state_expansion_fee = asset_transaction_state_expansion_fee(ledger, transaction);
    let minimum_fee = minimum_asset_transaction_fee_for_ledger(ledger, transaction);
    if transaction.unsigned.fee < minimum_fee {
        return Receipt::rejected(
            tx_id,
            "fee_too_low",
            format!("minimum asset transaction fee is {minimum_fee}"),
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    }

    if source.balance < transaction.unsigned.fee {
        return Receipt::rejected(
            tx_id,
            "insufficient_funds",
            "source balance is too low for fee",
        );
    }
    let source_after_fee = source.balance - transaction.unsigned.fee;
    if let Some(message) = account_reserve_violation(&transaction.unsigned.source, source_after_fee)
    {
        return Receipt::rejected(tx_id, "below_account_reserve", message)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    let mut next_ledger = ledger.clone();
    let Some(source) = next_ledger.account_mut(&transaction.unsigned.source) else {
        return Receipt::rejected(
            tx_id,
            "missing_sender",
            "source account does not exist before state mutation",
        );
    };
    if source.public_key_hex.is_none() {
        source.public_key_hex = Some(transaction.public_key_hex.clone());
    }
    source.balance = source_after_fee;
    source.sequence = expected_sequence;

    if let Err((code, message)) = apply_asset_operation(
        genesis,
        &mut next_ledger,
        transaction,
        block_height,
        compatibility,
    ) {
        return Receipt::rejected(tx_id, code, message).with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    }
    if let Err(error) = next_ledger.validate_asset_state(&genesis.chain_id) {
        return Receipt::rejected(tx_id, "invalid_asset_state", error)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }
    if compatibility.reject_legacy_domainless_withdrawal_packet_state {
        if let Some(error) = legacy_domainless_withdrawal_packet_state_error(&next_ledger) {
            return Receipt::rejected(tx_id, "invalid_asset_state", error)
                .with_fee_policy_and_state_expansion(
                    0,
                    0,
                    minimum_fee,
                    ACCOUNT_RESERVE,
                    state_expansion_fee,
                );
        }
    }
    if let Err(error) = next_ledger.validate_nav_state(&genesis.chain_id) {
        return Receipt::rejected(tx_id, "invalid_nav_state", error)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    *ledger = next_ledger;
    Receipt::accepted(tx_id, "asset transaction applied; fee burned")
        .with_fee_policy_and_state_expansion(
            transaction.unsigned.fee,
            transaction.unsigned.fee,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AtomicSwapOwnerPreparation {
    balance_after_fee: u64,
    pre_sequence: u64,
}

pub fn execute_atomic_swap_transaction(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAtomicSwapTransaction,
    block_height: u64,
) -> Receipt {
    execute_atomic_swap_transaction_with_compatibility(
        genesis,
        ledger,
        transaction,
        block_height,
        AssetExecutionCompatibility::strict().with_atomic_swap_activation_height(None),
    )
}

pub fn execute_atomic_swap_transaction_with_compatibility(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAtomicSwapTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    let tx_id = atomic_swap_transaction_tx_id(transaction);
    let issuer_endpoint = [&transaction.unsigned.leg_0, &transaction.unsigned.leg_1]
        .iter()
        .any(|leg| leg.owner == leg.issuer || leg.recipient == leg.issuer);
    if issuer_endpoint {
        return Receipt::rejected(
            tx_id,
            "issuer_leg_not_supported",
            "atomic swap v1 permits holder-to-holder issued-asset legs only",
        );
    }
    if let Err(error) = transaction.validate() {
        return Receipt::rejected(tx_id, "bad_atomic_swap_envelope", error);
    }
    if transaction.unsigned.chain_id != genesis.chain_id {
        return Receipt::rejected(
            tx_id,
            "wrong_chain",
            format!(
                "expected chain `{}`, got `{}`",
                genesis.chain_id, transaction.unsigned.chain_id
            ),
        );
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if transaction.unsigned.genesis_hash != expected_genesis_hash {
        return Receipt::rejected(
            tx_id,
            "wrong_genesis",
            format!(
                "expected genesis `{expected_genesis_hash}`, got `{}`",
                transaction.unsigned.genesis_hash
            ),
        );
    }
    if transaction.unsigned.protocol_version != genesis.protocol_version {
        return Receipt::rejected(
            tx_id,
            "wrong_protocol_version",
            format!(
                "expected protocol version {}, got {}",
                genesis.protocol_version, transaction.unsigned.protocol_version
            ),
        );
    }
    if transaction.unsigned.address_namespace != ADDRESS_NAMESPACE {
        return Receipt::rejected(
            tx_id,
            "wrong_address_namespace",
            format!(
                "expected address namespace `{ADDRESS_NAMESPACE}`, got `{}`",
                transaction.unsigned.address_namespace
            ),
        );
    }
    if compatibility.atomic_swap_paused {
        return Receipt::rejected(
            tx_id,
            "atomic_swap_paused",
            "atomic swap execution is paused by governance",
        );
    }
    if !compatibility.atomic_swap_active(block_height) {
        return Receipt::rejected(
            tx_id,
            "atomic_swap_not_active",
            format!("atomic swap is not active at block height {block_height}"),
        );
    }
    if block_height > transaction.unsigned.expires_at_height {
        return Receipt::rejected(
            tx_id,
            "swap_expired",
            format!(
                "atomic swap expired at height {}, current height is {block_height}",
                transaction.unsigned.expires_at_height
            ),
        );
    }
    if transaction.unsigned.signature_algorithm_id != ML_DSA_65_ALGORITHM {
        return Receipt::rejected(
            tx_id,
            "unsupported_signature_algorithm",
            format!(
                "unsupported algorithm `{}`",
                transaction.unsigned.signature_algorithm_id
            ),
        );
    }

    let minimum_fee_0 = minimum_atomic_swap_leg_fee_for_ledger(
        ledger,
        transaction,
        &transaction.unsigned.leg_0,
    );
    let minimum_fee_1 = minimum_atomic_swap_leg_fee_for_ledger(
        ledger,
        transaction,
        &transaction.unsigned.leg_1,
    );
    let state_expansion_fee_0 =
        atomic_swap_leg_state_expansion_fee(ledger, &transaction.unsigned.leg_0);
    let state_expansion_fee_1 =
        atomic_swap_leg_state_expansion_fee(ledger, &transaction.unsigned.leg_1);
    let Some(minimum_fee) = minimum_fee_0.checked_add(minimum_fee_1) else {
        return Receipt::rejected(
            tx_id,
            "fee_overflow",
            "atomic swap minimum fee total overflowed",
        );
    };
    let Some(state_expansion_fee) = state_expansion_fee_0.checked_add(state_expansion_fee_1)
    else {
        return Receipt::rejected(
            tx_id,
            "fee_overflow",
            "atomic swap state-expansion fee total overflowed",
        );
    };
    let reject = |code: &'static str, message: String| {
        Receipt::rejected(tx_id.clone(), code, message).with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        )
    };
    let signing_bytes = transaction.unsigned.signing_bytes();
    let prepared_0 = match prepare_atomic_swap_owner(
        ledger,
        &transaction.unsigned.leg_0,
        &transaction.authorization_0,
        &signing_bytes,
        minimum_fee_0,
    ) {
        Ok(prepared) => prepared,
        Err((code, message)) => return reject(code, message),
    };
    let prepared_1 = match prepare_atomic_swap_owner(
        ledger,
        &transaction.unsigned.leg_1,
        &transaction.authorization_1,
        &signing_bytes,
        minimum_fee_1,
    ) {
        Ok(prepared) => prepared,
        Err((code, message)) => return reject(code, message),
    };
    if let Err((code, message)) = validate_atomic_swap_market_binding(ledger, transaction) {
        return reject(code, message);
    }
    let Some(fee_charged) = transaction
        .unsigned
        .leg_0
        .fee
        .checked_add(transaction.unsigned.leg_1.fee)
    else {
        return reject(
            "fee_overflow",
            "atomic swap charged fee total overflowed".to_string(),
        );
    };

    let mut next_ledger = ledger.clone();
    if let Err((code, message)) = apply_atomic_swap_owner_fee(
        &mut next_ledger,
        &transaction.unsigned.leg_0,
        &transaction.authorization_0,
        &prepared_0,
    ) {
        return reject(code, message);
    }
    if let Err((code, message)) = apply_atomic_swap_owner_fee(
        &mut next_ledger,
        &transaction.unsigned.leg_1,
        &transaction.authorization_1,
        &prepared_1,
    ) {
        return reject(code, message);
    }
    for leg in [&transaction.unsigned.leg_0, &transaction.unsigned.leg_1] {
        let operation = IssuedPaymentOperation {
            from: leg.owner.clone(),
            to: leg.recipient.clone(),
            issuer: leg.issuer.clone(),
            asset_id: leg.asset_id.clone(),
            amount: leg.amount,
        };
        if let Err(error) =
            apply_issued_payment(&mut next_ledger, &operation, block_height, compatibility)
        {
            let (code, message) = atomic_swap_issued_payment_error(error);
            return reject(code, message);
        }
    }
    if let Err(error) = next_ledger.validate_asset_state(&genesis.chain_id) {
        return reject("invalid_asset_state", error);
    }
    if let Err(error) = next_ledger.validate_nav_state(&genesis.chain_id) {
        return reject("invalid_nav_state", error);
    }

    *ledger = next_ledger;
    Receipt::accepted(tx_id, "atomic swap applied; both fees burned")
        .with_fee_policy_and_state_expansion(
            fee_charged,
            fee_charged,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        )
        .with_atomic_swap_legs(vec![
            atomic_swap_leg_receipt(&transaction.unsigned.leg_0, prepared_0.pre_sequence),
            atomic_swap_leg_receipt(&transaction.unsigned.leg_1, prepared_1.pre_sequence),
        ])
}

fn prepare_atomic_swap_owner(
    ledger: &LedgerState,
    leg: &AtomicSwapLeg,
    authorization: &AtomicSwapAuthorization,
    signing_bytes: &[u8],
    minimum_fee: u64,
) -> Result<AtomicSwapOwnerPreparation, (&'static str, String)> {
    if authorization.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err((
            "signature_algorithm_mismatch",
            "atomic swap authorization algorithm does not match ML-DSA-65".to_string(),
        ));
    }
    let public_key = hex_to_bytes(&authorization.public_key_hex)
        .map_err(|error| ("bad_public_key", error.to_string()))?;
    let signature = hex_to_bytes(&authorization.signature_hex)
        .map_err(|error| ("bad_signature", error.to_string()))?;
    if address_from_public_key(&public_key) != leg.owner {
        return Err((
            "sender_mismatch",
            format!("atomic swap authorization does not match owner `{}`", leg.owner),
        ));
    }
    if !ml_dsa_65_verify(&public_key, signing_bytes, &signature) {
        return Err((
            "bad_signature",
            format!("atomic swap signature verification failed for `{}`", leg.owner),
        ));
    }
    let account = ledger.account(&leg.owner).ok_or_else(|| {
        (
            "missing_sender",
            format!("atomic swap owner account `{}` does not exist", leg.owner),
        )
    })?;
    if let Some(stored_public_key) = account.public_key_hex.as_deref() {
        if stored_public_key != authorization.public_key_hex {
            return Err((
                "sender_key_mismatch",
                format!("atomic swap owner `{}` public key does not match ledger", leg.owner),
            ));
        }
    }
    let expected_sequence = account.sequence.checked_add(1).ok_or_else(|| {
        (
            "sequence_overflow",
            format!("atomic swap owner `{}` sequence cannot advance", leg.owner),
        )
    })?;
    if leg.sequence != expected_sequence {
        return Err((
            "bad_sequence",
            format!(
                "expected sequence {expected_sequence} for `{}`, got {}",
                leg.owner, leg.sequence
            ),
        ));
    }
    if leg.fee < minimum_fee {
        return Err((
            "fee_too_low",
            format!("minimum atomic swap fee for `{}` is {minimum_fee}", leg.owner),
        ));
    }
    let balance_after_fee = account.balance.checked_sub(leg.fee).ok_or_else(|| {
        (
            "insufficient_funds",
            format!("atomic swap owner `{}` balance is too low for fee", leg.owner),
        )
    })?;
    if let Some(message) = account_reserve_violation(&leg.owner, balance_after_fee) {
        return Err(("below_account_reserve", message));
    }
    Ok(AtomicSwapOwnerPreparation {
        balance_after_fee,
        pre_sequence: account.sequence,
    })
}

fn apply_atomic_swap_owner_fee(
    ledger: &mut LedgerState,
    leg: &AtomicSwapLeg,
    authorization: &AtomicSwapAuthorization,
    prepared: &AtomicSwapOwnerPreparation,
) -> Result<(), (&'static str, String)> {
    let account = ledger.account_mut(&leg.owner).ok_or_else(|| {
        (
            "missing_sender",
            format!("atomic swap owner account `{}` disappeared", leg.owner),
        )
    })?;
    if account.public_key_hex.is_none() {
        account.public_key_hex = Some(authorization.public_key_hex.clone());
    }
    account.balance = prepared.balance_after_fee;
    account.sequence = leg.sequence;
    Ok(())
}

pub fn validate_atomic_swap_market_binding(
    ledger: &LedgerState,
    transaction: &SignedAtomicSwapTransaction,
) -> Result<(), (&'static str, String)> {
    // `nav_assets` also contains bridge-backed assets for reserve accounting.
    // Price-NAV classification is therefore derived from finalized market
    // envelopes, once per leg (not once per historical envelope row).
    let mut price_nav_legs = [&transaction.unsigned.leg_0, &transaction.unsigned.leg_1]
        .into_iter()
        .filter(|leg| {
            ledger
                .market_ops_envelopes
                .iter()
                .any(|record| record.asset_id == leg.asset_id)
        });
    let Some(leg) = price_nav_legs.next() else {
        return Err((
            "wrong_market_envelope",
            "atomic swap v1 requires exactly one price-NAV leg backed by a finalized market-ops envelope"
                .to_string(),
        ));
    };
    if price_nav_legs.next().is_some() {
        return Err((
            "nav_pair_not_supported",
            "atomic swap v1 does not support two price-NAV legs".to_string(),
        ));
    }
    let nav_asset = ledger.nav_asset(&leg.asset_id).ok_or_else(|| {
        (
            "wrong_nav_epoch",
            format!(
                "price-NAV asset `{}` has no NAV accounting record",
                leg.asset_id
            ),
        )
    })?;
    if nav_asset.finalized_epoch != transaction.unsigned.nav_epoch {
        return Err((
            "wrong_nav_epoch",
            format!(
                "asset `{}` finalized NAV epoch is {}, transaction binds {}",
                leg.asset_id, nav_asset.finalized_epoch, transaction.unsigned.nav_epoch
            ),
        ));
    }
    let record = ledger
        .market_ops_envelope(&leg.asset_id, transaction.unsigned.nav_epoch)
        .ok_or_else(|| {
            (
                "wrong_market_envelope",
                format!(
                    "asset `{}` has no finalized market-ops envelope at epoch {}",
                    leg.asset_id, transaction.unsigned.nav_epoch
                ),
            )
        })?;
    if transaction.unsigned.market_envelope_hash != record.envelope_hash {
        return Err((
            "wrong_market_envelope",
            format!(
                "expected finalized market envelope `{}`, got `{}`",
                record.envelope_hash, transaction.unsigned.market_envelope_hash
            ),
        ));
    }
    Ok(())
}

fn atomic_swap_leg_receipt(leg: &AtomicSwapLeg, pre_sequence: u64) -> AtomicSwapLegReceipt {
    AtomicSwapLegReceipt {
        owner: leg.owner.clone(),
        recipient: leg.recipient.clone(),
        asset_id: leg.asset_id.clone(),
        amount: leg.amount,
        fee_charged: leg.fee,
        pre_sequence,
        post_sequence: leg.sequence,
    }
}

fn atomic_swap_issued_payment_error(
    error: (&'static str, String),
) -> (&'static str, String) {
    match error.0 {
        "missing_trustline" => (
            "missing_asset_balance",
            "atomic swap owner has no balance for the issued asset".to_string(),
        ),
        "insufficient_issued_balance" => (
            "insufficient_issued_balance",
            "atomic swap amount exceeds the owner's issued-asset balance".to_string(),
        ),
        "missing_issuer_authorization" => (
            "asset_transfer_not_authorized",
            "atomic swap transfer is not authorized under the existing asset policy".to_string(),
        ),
        "frozen_trustline" => (
            "asset_balance_frozen",
            "atomic swap issued-asset balance is frozen".to_string(),
        ),
        _ => error,
    }
}

fn legacy_domainless_withdrawal_packet_state_error(ledger: &LedgerState) -> Option<String> {
    ledger
        .vault_bridge_redemptions
        .iter()
        .find(|redemption| redemption.withdrawal_packet.is_legacy_domainless())
        .map(|_| "vault_bridge_withdrawal_packet.source_chain_id must be nonzero".to_string())
}

pub fn execute_escrow_transaction(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedEscrowTransaction,
    block_height: u64,
) -> Receipt {
    let tx_id = escrow_transaction_tx_id(transaction);

    if let Err(error) = transaction.validate() {
        return Receipt::rejected(tx_id, "bad_escrow_transaction_envelope", error);
    }

    if transaction.unsigned.chain_id != genesis.chain_id {
        return Receipt::rejected(
            tx_id,
            "wrong_chain",
            format!(
                "expected chain `{}`, got `{}`",
                genesis.chain_id, transaction.unsigned.chain_id
            ),
        );
    }

    let expected_genesis_hash = genesis_hash(genesis);
    if transaction.unsigned.genesis_hash != expected_genesis_hash {
        return Receipt::rejected(
            tx_id,
            "wrong_genesis",
            format!(
                "expected genesis `{expected_genesis_hash}`, got `{}`",
                transaction.unsigned.genesis_hash
            ),
        );
    }

    if transaction.unsigned.protocol_version != genesis.protocol_version {
        return Receipt::rejected(
            tx_id,
            "wrong_protocol_version",
            format!(
                "expected protocol version {}, got {}",
                genesis.protocol_version, transaction.unsigned.protocol_version
            ),
        );
    }

    if transaction.unsigned.address_namespace != ADDRESS_NAMESPACE {
        return Receipt::rejected(
            tx_id,
            "wrong_address_namespace",
            format!(
                "expected address namespace `{ADDRESS_NAMESPACE}`, got `{}`",
                transaction.unsigned.address_namespace
            ),
        );
    }

    if transaction.unsigned.signature_algorithm_id != ML_DSA_65_ALGORITHM {
        return Receipt::rejected(
            tx_id,
            "unsupported_signature_algorithm",
            format!(
                "unsupported algorithm `{}`",
                transaction.unsigned.signature_algorithm_id
            ),
        );
    }

    if transaction.algorithm_id != transaction.unsigned.signature_algorithm_id {
        return Receipt::rejected(
            tx_id,
            "signature_algorithm_mismatch",
            "signed envelope algorithm does not match signed payload",
        );
    }

    let public_key = match hex_to_bytes(&transaction.public_key_hex) {
        Ok(public_key) => public_key,
        Err(error) => return Receipt::rejected(tx_id, "bad_public_key", error.to_string()),
    };
    let signature = match hex_to_bytes(&transaction.signature_hex) {
        Ok(signature) => signature,
        Err(error) => return Receipt::rejected(tx_id, "bad_signature", error.to_string()),
    };

    let derived_source = address_from_public_key(&public_key);
    if derived_source != transaction.unsigned.source {
        return Receipt::rejected(
            tx_id,
            "sender_mismatch",
            "escrow transaction source does not match public key",
        );
    }

    if !ml_dsa_65_verify(
        &public_key,
        &transaction.unsigned.signing_bytes(),
        &signature,
    ) {
        return Receipt::rejected(tx_id, "bad_signature", "signature verification failed");
    }

    let Some(source) = ledger.account(&transaction.unsigned.source) else {
        return Receipt::rejected(tx_id, "missing_sender", "source account does not exist");
    };

    if let Some(source_public_key_hex) = source.public_key_hex.as_deref() {
        if source_public_key_hex != transaction.public_key_hex.as_str() {
            return Receipt::rejected(
                tx_id,
                "sender_key_mismatch",
                "source public key does not match ledger account",
            );
        }
    }

    let Some(expected_sequence) = source.sequence.checked_add(1) else {
        return Receipt::rejected(
            tx_id,
            "sequence_overflow",
            "source sequence is exhausted",
        );
    };
    if transaction.unsigned.sequence != expected_sequence {
        return Receipt::rejected(
            tx_id,
            "bad_sequence",
            format!(
                "expected sequence {expected_sequence}, got {}",
                transaction.unsigned.sequence
            ),
        );
    }

    let state_expansion_fee = escrow_transaction_state_expansion_fee(ledger, transaction);
    let minimum_fee = minimum_escrow_transaction_fee_for_ledger(ledger, transaction);
    if transaction.unsigned.fee < minimum_fee {
        return Receipt::rejected(
            tx_id,
            "fee_too_low",
            format!("minimum escrow transaction fee is {minimum_fee}"),
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    }

    if source.balance < transaction.unsigned.fee {
        return Receipt::rejected(
            tx_id,
            "insufficient_funds",
            "source balance is too low for fee",
        );
    }
    let source_after_fee = source.balance - transaction.unsigned.fee;
    if let Some(message) = account_reserve_violation(&transaction.unsigned.source, source_after_fee)
    {
        return Receipt::rejected(tx_id, "below_account_reserve", message)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    let mut next_ledger = ledger.clone();
    let Some(source) = next_ledger.account_mut(&transaction.unsigned.source) else {
        return Receipt::rejected(
            tx_id,
            "missing_sender",
            "source account does not exist before state mutation",
        );
    };
    if source.public_key_hex.is_none() {
        source.public_key_hex = Some(transaction.public_key_hex.clone());
    }
    source.balance = source_after_fee;
    source.sequence = expected_sequence;

    if let Err((code, message)) =
        apply_escrow_operation(genesis, &mut next_ledger, transaction, block_height)
    {
        return Receipt::rejected(tx_id, code, message).with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    }
    if let Err(error) = next_ledger.validate_escrow_state(&genesis.chain_id) {
        return Receipt::rejected(tx_id, "invalid_escrow_state", error)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }
    if let Err(error) = next_ledger.validate_asset_state(&genesis.chain_id) {
        return Receipt::rejected(tx_id, "invalid_asset_state", error)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    *ledger = next_ledger;
    Receipt::accepted(tx_id, "escrow transaction applied; fee burned")
        .with_fee_policy_and_state_expansion(
            transaction.unsigned.fee,
            transaction.unsigned.fee,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        )
}

pub fn execute_nft_transaction(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedNftTransaction,
) -> Receipt {
    let tx_id = nft_transaction_tx_id(transaction);

    if let Err(error) = transaction.validate() {
        return Receipt::rejected(tx_id, "bad_nft_transaction_envelope", error);
    }

    if transaction.unsigned.chain_id != genesis.chain_id {
        return Receipt::rejected(
            tx_id,
            "wrong_chain",
            format!(
                "expected chain `{}`, got `{}`",
                genesis.chain_id, transaction.unsigned.chain_id
            ),
        );
    }

    let expected_genesis_hash = genesis_hash(genesis);
    if transaction.unsigned.genesis_hash != expected_genesis_hash {
        return Receipt::rejected(
            tx_id,
            "wrong_genesis",
            format!(
                "expected genesis `{expected_genesis_hash}`, got `{}`",
                transaction.unsigned.genesis_hash
            ),
        );
    }

    if transaction.unsigned.protocol_version != genesis.protocol_version {
        return Receipt::rejected(
            tx_id,
            "wrong_protocol_version",
            format!(
                "expected protocol version {}, got {}",
                genesis.protocol_version, transaction.unsigned.protocol_version
            ),
        );
    }

    if transaction.unsigned.address_namespace != ADDRESS_NAMESPACE {
        return Receipt::rejected(
            tx_id,
            "wrong_address_namespace",
            format!(
                "expected address namespace `{ADDRESS_NAMESPACE}`, got `{}`",
                transaction.unsigned.address_namespace
            ),
        );
    }

    if transaction.unsigned.signature_algorithm_id != ML_DSA_65_ALGORITHM {
        return Receipt::rejected(
            tx_id,
            "unsupported_signature_algorithm",
            format!(
                "unsupported algorithm `{}`",
                transaction.unsigned.signature_algorithm_id
            ),
        );
    }

    if transaction.algorithm_id != transaction.unsigned.signature_algorithm_id {
        return Receipt::rejected(
            tx_id,
            "signature_algorithm_mismatch",
            "signed envelope algorithm does not match signed payload",
        );
    }

    let public_key = match hex_to_bytes(&transaction.public_key_hex) {
        Ok(public_key) => public_key,
        Err(error) => return Receipt::rejected(tx_id, "bad_public_key", error.to_string()),
    };
    let signature = match hex_to_bytes(&transaction.signature_hex) {
        Ok(signature) => signature,
        Err(error) => return Receipt::rejected(tx_id, "bad_signature", error.to_string()),
    };

    let derived_source = address_from_public_key(&public_key);
    if derived_source != transaction.unsigned.source {
        return Receipt::rejected(
            tx_id,
            "sender_mismatch",
            "nft transaction source does not match public key",
        );
    }

    if !ml_dsa_65_verify(
        &public_key,
        &transaction.unsigned.signing_bytes(),
        &signature,
    ) {
        return Receipt::rejected(tx_id, "bad_signature", "signature verification failed");
    }

    let Some(source) = ledger.account(&transaction.unsigned.source) else {
        return Receipt::rejected(tx_id, "missing_sender", "source account does not exist");
    };

    if let Some(source_public_key_hex) = source.public_key_hex.as_deref() {
        if source_public_key_hex != transaction.public_key_hex.as_str() {
            return Receipt::rejected(
                tx_id,
                "sender_key_mismatch",
                "source public key does not match ledger account",
            );
        }
    }

    let Some(expected_sequence) = source.sequence.checked_add(1) else {
        return Receipt::rejected(
            tx_id,
            "sequence_overflow",
            "source sequence is exhausted",
        );
    };
    if transaction.unsigned.sequence != expected_sequence {
        return Receipt::rejected(
            tx_id,
            "bad_sequence",
            format!(
                "expected sequence {expected_sequence}, got {}",
                transaction.unsigned.sequence
            ),
        );
    }

    let state_expansion_fee = nft_transaction_state_expansion_fee(ledger, transaction);
    let minimum_fee = minimum_nft_transaction_fee_for_ledger(ledger, transaction);
    if transaction.unsigned.fee < minimum_fee {
        return Receipt::rejected(
            tx_id,
            "fee_too_low",
            format!("minimum nft transaction fee is {minimum_fee}"),
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    }

    if source.balance < transaction.unsigned.fee {
        return Receipt::rejected(
            tx_id,
            "insufficient_funds",
            "source balance is too low for fee",
        );
    }
    let source_after_fee = source.balance - transaction.unsigned.fee;
    if let Some(message) = account_reserve_violation(&transaction.unsigned.source, source_after_fee)
    {
        return Receipt::rejected(tx_id, "below_account_reserve", message)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    let mut next_ledger = ledger.clone();
    let Some(source) = next_ledger.account_mut(&transaction.unsigned.source) else {
        return Receipt::rejected(
            tx_id,
            "missing_sender",
            "source account does not exist before state mutation",
        );
    };
    if source.public_key_hex.is_none() {
        source.public_key_hex = Some(transaction.public_key_hex.clone());
    }
    source.balance = source_after_fee;
    source.sequence = expected_sequence;

    if let Err((code, message)) = apply_nft_operation(genesis, &mut next_ledger, transaction) {
        return Receipt::rejected(tx_id, code, message).with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    }
    if let Err(error) = next_ledger.validate_nft_state(&genesis.chain_id) {
        return Receipt::rejected(tx_id, "invalid_nft_state", error)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    *ledger = next_ledger;
    let mut receipt = Receipt::accepted(tx_id, "nft transaction applied; fee burned")
        .with_fee_policy_and_state_expansion(
            transaction.unsigned.fee,
            transaction.unsigned.fee,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    if let NftTransactionOperation::NftTransfer(operation) = &transaction.unsigned.operation {
        if operation.issuer_transfer_fee != 0 {
            receipt = receipt.with_nft_issuer_transfer_fee(
                operation.issuer_transfer_fee,
                operation.issuer.clone(),
            );
        }
    } else if let NftTransactionOperation::NftMint(operation) = &transaction.unsigned.operation {
        if operation.collection_flags != 0 {
            receipt = receipt.with_nft_collection_flags(operation.collection_flags);
        }
    }
    receipt
}

pub fn execute_offer_transaction(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedOfferTransaction,
    block_height: u64,
) -> Receipt {
    let tx_id = offer_transaction_tx_id(transaction);

    if let Err(error) = transaction.validate() {
        return Receipt::rejected(tx_id, "bad_offer_transaction_envelope", error);
    }

    if transaction.unsigned.chain_id != genesis.chain_id {
        return Receipt::rejected(
            tx_id,
            "wrong_chain",
            format!(
                "expected chain `{}`, got `{}`",
                genesis.chain_id, transaction.unsigned.chain_id
            ),
        );
    }

    let expected_genesis_hash = genesis_hash(genesis);
    if transaction.unsigned.genesis_hash != expected_genesis_hash {
        return Receipt::rejected(
            tx_id,
            "wrong_genesis",
            format!(
                "expected genesis `{expected_genesis_hash}`, got `{}`",
                transaction.unsigned.genesis_hash
            ),
        );
    }

    if transaction.unsigned.protocol_version != genesis.protocol_version {
        return Receipt::rejected(
            tx_id,
            "wrong_protocol_version",
            format!(
                "expected protocol version {}, got {}",
                genesis.protocol_version, transaction.unsigned.protocol_version
            ),
        );
    }

    if transaction.unsigned.address_namespace != ADDRESS_NAMESPACE {
        return Receipt::rejected(
            tx_id,
            "wrong_address_namespace",
            format!(
                "expected address namespace `{ADDRESS_NAMESPACE}`, got `{}`",
                transaction.unsigned.address_namespace
            ),
        );
    }

    if transaction.unsigned.signature_algorithm_id != ML_DSA_65_ALGORITHM {
        return Receipt::rejected(
            tx_id,
            "unsupported_signature_algorithm",
            format!(
                "unsupported algorithm `{}`",
                transaction.unsigned.signature_algorithm_id
            ),
        );
    }

    if transaction.algorithm_id != transaction.unsigned.signature_algorithm_id {
        return Receipt::rejected(
            tx_id,
            "signature_algorithm_mismatch",
            "signed envelope algorithm does not match signed payload",
        );
    }

    let public_key = match hex_to_bytes(&transaction.public_key_hex) {
        Ok(public_key) => public_key,
        Err(error) => return Receipt::rejected(tx_id, "bad_public_key", error.to_string()),
    };
    let signature = match hex_to_bytes(&transaction.signature_hex) {
        Ok(signature) => signature,
        Err(error) => return Receipt::rejected(tx_id, "bad_signature", error.to_string()),
    };

    let derived_source = address_from_public_key(&public_key);
    if derived_source != transaction.unsigned.source {
        return Receipt::rejected(
            tx_id,
            "sender_mismatch",
            "offer transaction source does not match public key",
        );
    }

    if !ml_dsa_65_verify(
        &public_key,
        &transaction.unsigned.signing_bytes(),
        &signature,
    ) {
        return Receipt::rejected(tx_id, "bad_signature", "signature verification failed");
    }

    let Some(source) = ledger.account(&transaction.unsigned.source) else {
        return Receipt::rejected(tx_id, "missing_sender", "source account does not exist");
    };

    if let Some(source_public_key_hex) = source.public_key_hex.as_deref() {
        if source_public_key_hex != transaction.public_key_hex.as_str() {
            return Receipt::rejected(
                tx_id,
                "sender_key_mismatch",
                "source public key does not match ledger account",
            );
        }
    }

    let Some(expected_sequence) = source.sequence.checked_add(1) else {
        return Receipt::rejected(
            tx_id,
            "sequence_overflow",
            "source sequence is exhausted",
        );
    };
    if transaction.unsigned.sequence != expected_sequence {
        return Receipt::rejected(
            tx_id,
            "bad_sequence",
            format!(
                "expected sequence {expected_sequence}, got {}",
                transaction.unsigned.sequence
            ),
        );
    }

    let state_expansion_fee =
        offer_transaction_state_expansion_fee(ledger, transaction, block_height);
    let minimum_fee = minimum_offer_transaction_fee_for_ledger(ledger, transaction, block_height);
    if transaction.unsigned.fee < minimum_fee {
        return Receipt::rejected(
            tx_id,
            "fee_too_low",
            format!("minimum offer transaction fee is {minimum_fee}"),
        )
        .with_fee_policy_and_state_expansion(
            0,
            0,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    }

    if source.balance < transaction.unsigned.fee {
        return Receipt::rejected(
            tx_id,
            "insufficient_funds",
            "source balance is too low for fee",
        );
    }
    let source_after_fee = source.balance - transaction.unsigned.fee;
    if let Some(message) = account_reserve_violation(&transaction.unsigned.source, source_after_fee)
    {
        return Receipt::rejected(tx_id, "below_account_reserve", message)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    let mut next_ledger = ledger.clone();
    let Some(source) = next_ledger.account_mut(&transaction.unsigned.source) else {
        return Receipt::rejected(
            tx_id,
            "missing_sender",
            "source account does not exist before state mutation",
        );
    };
    if source.public_key_hex.is_none() {
        source.public_key_hex = Some(transaction.public_key_hex.clone());
    }
    source.balance = source_after_fee;
    source.sequence = expected_sequence;

    let offer_outcome =
        match apply_offer_operation(genesis, &mut next_ledger, transaction, block_height) {
            Ok(outcome) => outcome,
            Err((code, message)) => {
                return Receipt::rejected(tx_id, code, message)
                    .with_fee_policy_and_state_expansion(
                        0,
                        0,
                        minimum_fee,
                        ACCOUNT_RESERVE,
                        state_expansion_fee,
                    );
            }
        };
    if let Err(error) = next_ledger.validate_offer_state(&genesis.chain_id) {
        return Receipt::rejected(tx_id, "invalid_offer_state", error)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }
    if let Err(error) = next_ledger.validate_asset_state(&genesis.chain_id) {
        return Receipt::rejected(tx_id, "invalid_asset_state", error)
            .with_fee_policy_and_state_expansion(
                0,
                0,
                minimum_fee,
                ACCOUNT_RESERVE,
                state_expansion_fee,
            );
    }

    *ledger = next_ledger;
    let mut receipt = Receipt::accepted(tx_id, "offer transaction applied; fee burned")
        .with_code(offer_outcome.receipt_code)
        .with_offer_fills(offer_outcome.fills)
        .with_fee_policy_and_state_expansion(
            transaction.unsigned.fee,
            transaction.unsigned.fee,
            minimum_fee,
            ACCOUNT_RESERVE,
            state_expansion_fee,
        );
    if let Some(offer_id) = offer_outcome.offer_id {
        receipt = receipt.with_offer_id(offer_id);
    }
    receipt
}
