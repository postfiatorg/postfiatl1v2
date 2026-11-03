// Destination address format rule.
//
// Mempool admission applies this check immediately (non-consensus). Block
// execution applies it only from the governed
// `recipient_format_activation_height`; before that height execution is
// unchanged so historical blocks replay byte-for-byte.
//
// Address rejection itself changes no balances. In a certified transparent
// block, the separately governed rejected-fee rule can claim the signed fee
// and consume the sequence after rechecking authentication and affordability.

/// Destination address fields of a transparent transfer.
pub fn transfer_destination_addresses(transfer: &SignedTransfer) -> Vec<(&'static str, &str)> {
    vec![("transfer.to", transfer.unsigned.to.as_str())]
}

/// Destination address fields of a `payment_v2`.
pub fn payment_v2_destination_addresses(payment: &SignedPaymentV2) -> Vec<(&'static str, &str)> {
    vec![("payment.to", payment.unsigned.to.as_str())]
}

/// Destination address fields of an asset transaction: the ledger accounts
/// the operation credits with an issued asset or NAV coin.
pub fn asset_transaction_destination_addresses(
    transaction: &SignedAssetTransaction,
) -> Vec<(&'static str, &str)> {
    match &transaction.unsigned.operation {
        AssetTransactionOperation::IssuedPayment(operation) => {
            vec![("issued_payment.to", operation.to.as_str())]
        }
        AssetTransactionOperation::IssuedPaymentV2(operation) => {
            vec![("issued_payment_v2.to", operation.to.as_str())]
        }
        AssetTransactionOperation::NavMintAtNav(operation) => {
            vec![("nav_mint_at_nav.to", operation.to.as_str())]
        }
        AssetTransactionOperation::VaultBridgeMintFromReceipts(operation) => {
            vec![("vault_bridge_mint_from_receipts.to", operation.to.as_str())]
        }
        AssetTransactionOperation::VaultBridgeDepositClaim(operation) => {
            vec![(
                "vault_bridge_deposit_claim.recipient",
                operation.recipient.as_str(),
            )]
        }
        AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(operation) => operation
            .nav_recipient
            .as_deref()
            .map(|recipient| {
                vec![(
                    "vault_bridge_nav_subscription_allocate.nav_recipient",
                    recipient,
                )]
            })
            .unwrap_or_default(),
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(operation) => vec![(
            "pftl_uniswap_primary_redeem.settlement_recipient",
            operation.settlement_recipient.as_str(),
        )],
        AssetTransactionOperation::PftlUniswapReturnImport(operation) => vec![(
            "pftl_uniswap_return_import.pftl_recipient",
            operation.pftl_recipient.as_str(),
        )],
        AssetTransactionOperation::ExternalLaneReleaseV1(operation) => vec![(
            "external_lane_release_v1.recipient",
            operation.recipient.as_str(),
        )],
        _ => Vec::new(),
    }
}

/// Destination address fields of an escrow transaction.
pub fn escrow_transaction_destination_addresses(
    transaction: &SignedEscrowTransaction,
) -> Vec<(&'static str, &str)> {
    match &transaction.unsigned.operation {
        EscrowTransactionOperation::EscrowCreate(operation) => {
            vec![("escrow_create.recipient", operation.recipient.as_str())]
        }
        EscrowTransactionOperation::EscrowFinish(_)
        | EscrowTransactionOperation::EscrowCancel(_) => Vec::new(),
    }
}

/// Destination address fields of an NFT transaction.
pub fn nft_transaction_destination_addresses(
    transaction: &SignedNftTransaction,
) -> Vec<(&'static str, &str)> {
    match &transaction.unsigned.operation {
        NftTransactionOperation::NftMint(operation) => {
            vec![("nft_mint.owner", operation.owner.as_str())]
        }
        NftTransactionOperation::NftTransfer(operation) => {
            vec![("nft_transfer.to", operation.to.as_str())]
        }
        NftTransactionOperation::NftBurn(_) => Vec::new(),
    }
}

/// First destination field that is not a canonical PFTL address, as a
/// stable receipt code and a human-readable message.
pub fn destination_address_rejection(
    fields: &[(&'static str, &str)],
) -> Option<(&'static str, String)> {
    fields.iter().find_map(|(field, value)| {
        postfiat_crypto_provider::validate_canonical_address(value)
            .err()
            .map(|error| {
                (
                    error.recipient_error_code(),
                    format!(
                        "{field} `{}` is not a valid PFTL address: {error}",
                        destination_address_excerpt(value)
                    ),
                )
            })
    })
}

/// Bounded, printable excerpt of an untrusted field for error messages.
fn destination_address_excerpt(value: &str) -> String {
    const MAX_CHARS: usize = 64;
    let mut excerpt = value
        .chars()
        .take(MAX_CHARS)
        .map(|character| {
            if character.is_ascii_graphic() || character == ' ' {
                character
            } else {
                '?'
            }
        })
        .collect::<String>();
    if value.chars().count() > MAX_CHARS {
        excerpt.push_str("...");
    }
    excerpt
}

/// Executes an escrow transaction with the governed recipient-format rule.
pub fn execute_escrow_transaction_with_compatibility(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedEscrowTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if compatibility.recipient_format_active(block_height) && transaction.validate().is_ok() {
        if let Some((code, message)) =
            destination_address_rejection(&escrow_transaction_destination_addresses(transaction))
        {
            return Receipt::rejected(escrow_transaction_tx_id(transaction), code, message);
        }
    }
    execute_escrow_transaction(genesis, ledger, transaction, block_height)
}

/// Executes an NFT transaction with the governed recipient-format rule.
pub fn execute_nft_transaction_with_compatibility(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedNftTransaction,
    block_height: u64,
    compatibility: AssetExecutionCompatibility,
) -> Receipt {
    if compatibility.recipient_format_active(block_height) && transaction.validate().is_ok() {
        if let Some((code, message)) =
            destination_address_rejection(&nft_transaction_destination_addresses(transaction))
        {
            return Receipt::rejected(nft_transaction_tx_id(transaction), code, message);
        }
    }
    execute_nft_transaction(genesis, ledger, transaction)
}
