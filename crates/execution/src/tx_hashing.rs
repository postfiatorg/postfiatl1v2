pub fn transfer_tx_id(transfer: &SignedTransfer) -> String {
    let mut bytes = transfer.unsigned.signing_bytes();
    bytes.extend_from_slice(b"algorithm=");
    bytes.extend_from_slice(transfer.algorithm_id.as_bytes());
    bytes.extend_from_slice(b"\npublic_key=");
    bytes.extend_from_slice(transfer.public_key_hex.as_bytes());
    bytes.extend_from_slice(b"\nsignature=");
    bytes.extend_from_slice(transfer.signature_hex.as_bytes());
    bytes.extend_from_slice(b"\n");
    hash_hex("postfiat.tx_id.v1", &bytes)
}

pub fn payment_v2_tx_id(payment: &SignedPaymentV2) -> String {
    hash_hex(
        "postfiat.payment_v2.tx_id.v1",
        &payment.tx_id_preimage_bytes(),
    )
}

pub fn asset_transaction_tx_id(transaction: &SignedAssetTransaction) -> String {
    hash_hex(
        "postfiat.asset_transaction.tx_id.v1",
        &transaction.tx_id_preimage_bytes(),
    )
}

pub fn atomic_swap_transaction_tx_id(transaction: &SignedAtomicSwapTransaction) -> String {
    hash_hex(
        postfiat_types::ATOMIC_SWAP_TRANSACTION_TX_ID_DOMAIN,
        &transaction.tx_id_preimage_bytes(),
    )
}

pub fn escrow_transaction_tx_id(transaction: &SignedEscrowTransaction) -> String {
    hash_hex(
        "postfiat.escrow_transaction.tx_id.v1",
        &transaction.tx_id_preimage_bytes(),
    )
}

pub fn nft_transaction_tx_id(transaction: &SignedNftTransaction) -> String {
    hash_hex(
        "postfiat.nft_transaction.tx_id.v1",
        &transaction.tx_id_preimage_bytes(),
    )
}

pub fn offer_transaction_tx_id(transaction: &SignedOfferTransaction) -> String {
    hash_hex(
        "postfiat.offer_transaction.tx_id.v1",
        &transaction.tx_id_preimage_bytes(),
    )
}

pub fn genesis_hash(genesis: &Genesis) -> String {
    let json = genesis.to_json().expect("genesis JSON serialization failed");
    hash_hex("postfiat.genesis.v1", json.as_bytes())
}
