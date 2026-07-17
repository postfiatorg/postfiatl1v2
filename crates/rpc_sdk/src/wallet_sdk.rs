use super::*;

pub fn wallet_backup_from_master_seed(
    chain_id: impl Into<String>,
    master_seed_hex: impl AsRef<str>,
    account_index: u32,
) -> Result<WalletBackupFile, WalletSdkError> {
    let chain_id = chain_id.into();
    validate_wallet_chain_id(&chain_id)?;
    let backup = WalletBackupFile {
        schema: WALLET_BACKUP_FILE_SCHEMA.to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        kdf: WALLET_DERIVATION_KDF.to_string(),
        derivation_domain: WALLET_DERIVATION_DOMAIN.to_string(),
        chain_id,
        account_index,
        key_role: WALLET_KEY_ROLE_TRANSPARENT_SPEND.to_string(),
        master_seed_hex: normalized_wallet_master_seed_hex(master_seed_hex.as_ref())?,
    };
    validate_wallet_backup_file(&backup)?;
    Ok(backup)
}

pub fn wallet_identity_from_backup(
    backup: &WalletBackupFile,
) -> Result<WalletIdentity, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    let key_pair = derive_wallet_key_pair(backup)?;
    Ok(WalletIdentity {
        algorithm_id: backup.algorithm_id.clone(),
        kdf: backup.kdf.clone(),
        derivation_domain: backup.derivation_domain.clone(),
        chain_id: backup.chain_id.clone(),
        account_index: backup.account_index,
        key_role: backup.key_role.clone(),
        address: address_from_public_key(&key_pair.public_key),
        public_key_hex: crypto_bytes_to_hex(&key_pair.public_key),
        private_key_material_redacted: true,
    })
}

pub fn wallet_sign_transfer_from_quote(
    backup: &WalletBackupFile,
    quote: &TransferFeeQuoteSummary,
) -> Result<SignedTransfer, WalletSdkError> {
    let identity = wallet_identity_from_backup(backup)?;
    if quote.from != identity.address {
        return Err(WalletSdkError::new(format!(
            "transfer quote sender `{}` does not match wallet address `{}`",
            quote.from, identity.address
        )));
    }
    wallet_sign_transfer_from_fields(
        backup,
        WalletSignTransferFields {
            chain_id: quote.chain_id.clone(),
            genesis_hash: quote.genesis_hash.clone(),
            protocol_version: quote.protocol_version,
            to: quote.to.clone(),
            amount: quote.amount,
            fee: quote.minimum_fee,
            sequence: quote.sequence,
        },
    )
}

pub fn wallet_sign_transfer_from_fields(
    backup: &WalletBackupFile,
    fields: WalletSignTransferFields,
) -> Result<SignedTransfer, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    if fields.chain_id != backup.chain_id {
        return Err(WalletSdkError::new(format!(
            "transfer chain_id `{}` does not match wallet backup chain_id `{}`",
            fields.chain_id, backup.chain_id
        )));
    }
    if fields.amount == 0 {
        return Err(WalletSdkError::new(
            "wallet transfer amount must be nonzero",
        ));
    }
    if fields.fee == 0 {
        return Err(WalletSdkError::new("wallet transfer fee must be nonzero"));
    }
    if fields.sequence == 0 {
        return Err(WalletSdkError::new(
            "wallet transfer sequence must be nonzero",
        ));
    }

    let key_pair = derive_wallet_key_pair(backup)?;
    let from = address_from_public_key(&key_pair.public_key);
    let public_key_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let unsigned = UnsignedTransfer {
        chain_id: fields.chain_id,
        genesis_hash: fields.genesis_hash,
        protocol_version: fields.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: postfiat_types::TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from,
        to: fields.to,
        amount: fields.amount,
        fee: fields.fee,
        sequence: fields.sequence,
    };
    unsigned.validate().map_err(WalletSdkError::new)?;
    let signing_bytes = unsigned.signing_bytes();
    let signature = ml_dsa_65_sign(&key_pair.private_key, &signing_bytes)
        .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify(&key_pair.public_key, &signing_bytes, &signature) {
        return Err(WalletSdkError::new(
            "wallet transfer signature verification failed",
        ));
    }
    let signed = SignedTransfer {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        signature_hex: crypto_bytes_to_hex(&signature),
    };
    signed.validate().map_err(WalletSdkError::new)?;
    Ok(signed)
}

pub fn wallet_sign_payment_v2_from_fields(
    backup: &WalletBackupFile,
    fields: WalletSignPaymentV2Fields,
) -> Result<SignedPaymentV2, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    if fields.chain_id != backup.chain_id {
        return Err(WalletSdkError::new(format!(
            "payment chain_id `{}` does not match wallet backup chain_id `{}`",
            fields.chain_id, backup.chain_id
        )));
    }
    if fields.amount == 0 {
        return Err(WalletSdkError::new("wallet payment amount must be nonzero"));
    }
    if fields.fee == 0 {
        return Err(WalletSdkError::new("wallet payment fee must be nonzero"));
    }
    if fields.sequence == 0 {
        return Err(WalletSdkError::new(
            "wallet payment sequence must be nonzero",
        ));
    }

    let key_pair = derive_wallet_key_pair(backup)?;
    let from = address_from_public_key(&key_pair.public_key);
    let public_key_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let unsigned = UnsignedPaymentV2 {
        chain_id: fields.chain_id,
        genesis_hash: fields.genesis_hash,
        protocol_version: fields.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from,
        to: fields.to,
        amount: fields.amount,
        fee: fields.fee,
        sequence: fields.sequence,
        memos: fields.memos,
    };
    unsigned.validate().map_err(WalletSdkError::new)?;
    let signing_bytes = unsigned.signing_bytes();
    let signature = ml_dsa_65_sign(&key_pair.private_key, &signing_bytes)
        .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify(&key_pair.public_key, &signing_bytes, &signature) {
        return Err(WalletSdkError::new(
            "wallet payment v2 signature verification failed",
        ));
    }
    let signed = SignedPaymentV2 {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        signature_hex: crypto_bytes_to_hex(&signature),
    };
    signed.validate().map_err(WalletSdkError::new)?;
    Ok(signed)
}

pub fn wallet_sign_asset_transaction_from_quote(
    backup: &WalletBackupFile,
    quote_response: &RpcResponse,
) -> Result<SignedAssetTransaction, WalletSdkError> {
    let result = validated_summary_result(quote_response, RpcResponseKind::AssetFeeQuote)
        .map_err(|error| WalletSdkError::new(format!("asset fee quote invalid: {error}")))?;
    let operation: AssetTransactionOperation = serde_json::from_value(
        field(result, "operation")
            .map_err(|error| {
                WalletSdkError::new(format!("asset fee quote operation missing: {error}"))
            })?
            .clone(),
    )
    .map_err(|error| {
        WalletSdkError::new(format!("asset fee quote operation parse failed: {error}"))
    })?;
    operation.validate().map_err(WalletSdkError::new)?;
    wallet_sign_asset_transaction_from_fields(
        backup,
        WalletSignAssetTransactionFields {
            chain_id: clean_string_field(result, "chain_id")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            genesis_hash: string_field(result, "genesis_hash")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            protocol_version: nonzero_u32_field(result, "protocol_version")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            source: clean_string_field(result, "source")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            fee: nonzero_u64_field(result, "minimum_fee")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            sequence: nonzero_u64_field(result, "sequence")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            operation,
        },
    )
}

pub fn wallet_sign_asset_transaction_from_fields(
    backup: &WalletBackupFile,
    fields: WalletSignAssetTransactionFields,
) -> Result<SignedAssetTransaction, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    if fields.chain_id != backup.chain_id {
        return Err(WalletSdkError::new(format!(
            "asset transaction chain_id `{}` does not match wallet backup chain_id `{}`",
            fields.chain_id, backup.chain_id
        )));
    }
    if fields.fee == 0 {
        return Err(WalletSdkError::new(
            "wallet asset transaction fee must be nonzero",
        ));
    }
    if fields.sequence == 0 {
        return Err(WalletSdkError::new(
            "wallet asset transaction sequence must be nonzero",
        ));
    }

    let key_pair = derive_wallet_key_pair(backup)?;
    let source = address_from_public_key(&key_pair.public_key);
    if fields.source != source {
        return Err(WalletSdkError::new(format!(
            "asset transaction source `{}` does not match wallet address `{}`",
            fields.source, source
        )));
    }
    let public_key_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let unsigned = UnsignedAssetTransaction {
        chain_id: fields.chain_id,
        genesis_hash: fields.genesis_hash,
        protocol_version: fields.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: fields.operation.transaction_kind().to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source,
        fee: fields.fee,
        sequence: fields.sequence,
        operation: fields.operation,
    };
    unsigned.validate().map_err(WalletSdkError::new)?;
    let signing_bytes = unsigned.signing_bytes();
    let signature = ml_dsa_65_sign(&key_pair.private_key, &signing_bytes)
        .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify(&key_pair.public_key, &signing_bytes, &signature) {
        return Err(WalletSdkError::new(
            "wallet asset transaction signature verification failed",
        ));
    }
    let signed = SignedAssetTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        signature_hex: crypto_bytes_to_hex(&signature),
    };
    signed.validate().map_err(WalletSdkError::new)?;
    Ok(signed)
}

pub fn wallet_sign_escrow_transaction_from_quote(
    backup: &WalletBackupFile,
    quote_response: &RpcResponse,
) -> Result<SignedEscrowTransaction, WalletSdkError> {
    let result = validated_summary_result(quote_response, RpcResponseKind::EscrowFeeQuote)
        .map_err(|error| WalletSdkError::new(format!("escrow fee quote invalid: {error}")))?;
    let operation: EscrowTransactionOperation = serde_json::from_value(
        field(result, "operation")
            .map_err(|error| {
                WalletSdkError::new(format!("escrow fee quote operation missing: {error}"))
            })?
            .clone(),
    )
    .map_err(|error| {
        WalletSdkError::new(format!("escrow fee quote operation parse failed: {error}"))
    })?;
    operation.validate().map_err(WalletSdkError::new)?;
    wallet_sign_escrow_transaction_from_fields(
        backup,
        WalletSignEscrowTransactionFields {
            chain_id: clean_string_field(result, "chain_id")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            genesis_hash: string_field(result, "genesis_hash")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            protocol_version: nonzero_u32_field(result, "protocol_version")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            source: clean_string_field(result, "source")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            fee: nonzero_u64_field(result, "minimum_fee")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            sequence: nonzero_u64_field(result, "sequence")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            operation,
        },
    )
}

pub fn wallet_sign_escrow_transaction_from_fields(
    backup: &WalletBackupFile,
    fields: WalletSignEscrowTransactionFields,
) -> Result<SignedEscrowTransaction, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    if fields.chain_id != backup.chain_id {
        return Err(WalletSdkError::new(format!(
            "escrow transaction chain_id `{}` does not match wallet backup chain_id `{}`",
            fields.chain_id, backup.chain_id
        )));
    }
    if fields.fee == 0 {
        return Err(WalletSdkError::new(
            "wallet escrow transaction fee must be nonzero",
        ));
    }
    if fields.sequence == 0 {
        return Err(WalletSdkError::new(
            "wallet escrow transaction sequence must be nonzero",
        ));
    }

    let key_pair = derive_wallet_key_pair(backup)?;
    let source = address_from_public_key(&key_pair.public_key);
    if fields.source != source {
        return Err(WalletSdkError::new(format!(
            "escrow transaction source `{}` does not match wallet address `{}`",
            fields.source, source
        )));
    }
    let public_key_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let unsigned = UnsignedEscrowTransaction {
        chain_id: fields.chain_id,
        genesis_hash: fields.genesis_hash,
        protocol_version: fields.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: fields.operation.transaction_kind().to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source,
        fee: fields.fee,
        sequence: fields.sequence,
        operation: fields.operation,
    };
    unsigned.validate().map_err(WalletSdkError::new)?;
    let signing_bytes = unsigned.signing_bytes();
    let signature = ml_dsa_65_sign(&key_pair.private_key, &signing_bytes)
        .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify(&key_pair.public_key, &signing_bytes, &signature) {
        return Err(WalletSdkError::new(
            "wallet escrow transaction signature verification failed",
        ));
    }
    let signed = SignedEscrowTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        signature_hex: crypto_bytes_to_hex(&signature),
    };
    signed.validate().map_err(WalletSdkError::new)?;
    Ok(signed)
}

pub fn wallet_sign_nft_transaction_from_quote(
    backup: &WalletBackupFile,
    quote_response: &RpcResponse,
) -> Result<SignedNftTransaction, WalletSdkError> {
    let result = validated_summary_result(quote_response, RpcResponseKind::NftFeeQuote)
        .map_err(|error| WalletSdkError::new(format!("nft fee quote invalid: {error}")))?;
    let operation: NftTransactionOperation = serde_json::from_value(
        field(result, "operation")
            .map_err(|error| {
                WalletSdkError::new(format!("nft fee quote operation missing: {error}"))
            })?
            .clone(),
    )
    .map_err(|error| {
        WalletSdkError::new(format!("nft fee quote operation parse failed: {error}"))
    })?;
    operation.validate().map_err(WalletSdkError::new)?;
    wallet_sign_nft_transaction_from_fields(
        backup,
        WalletSignNftTransactionFields {
            chain_id: clean_string_field(result, "chain_id")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            genesis_hash: string_field(result, "genesis_hash")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            protocol_version: nonzero_u32_field(result, "protocol_version")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            source: clean_string_field(result, "source")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            fee: nonzero_u64_field(result, "minimum_fee")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            sequence: nonzero_u64_field(result, "sequence")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            operation,
        },
    )
}

pub fn wallet_sign_nft_transaction_from_fields(
    backup: &WalletBackupFile,
    fields: WalletSignNftTransactionFields,
) -> Result<SignedNftTransaction, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    if fields.chain_id != backup.chain_id {
        return Err(WalletSdkError::new(format!(
            "nft transaction chain_id `{}` does not match wallet backup chain_id `{}`",
            fields.chain_id, backup.chain_id
        )));
    }
    if fields.fee == 0 {
        return Err(WalletSdkError::new(
            "wallet nft transaction fee must be nonzero",
        ));
    }
    if fields.sequence == 0 {
        return Err(WalletSdkError::new(
            "wallet nft transaction sequence must be nonzero",
        ));
    }

    let key_pair = derive_wallet_key_pair(backup)?;
    let source = address_from_public_key(&key_pair.public_key);
    if fields.source != source {
        return Err(WalletSdkError::new(format!(
            "nft transaction source `{}` does not match wallet address `{}`",
            fields.source, source
        )));
    }
    let public_key_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let unsigned = UnsignedNftTransaction {
        chain_id: fields.chain_id,
        genesis_hash: fields.genesis_hash,
        protocol_version: fields.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: fields.operation.transaction_kind().to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source,
        fee: fields.fee,
        sequence: fields.sequence,
        operation: fields.operation,
    };
    unsigned.validate().map_err(WalletSdkError::new)?;
    let signing_bytes = unsigned.signing_bytes();
    let signature = ml_dsa_65_sign(&key_pair.private_key, &signing_bytes)
        .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify(&key_pair.public_key, &signing_bytes, &signature) {
        return Err(WalletSdkError::new(
            "wallet nft transaction signature verification failed",
        ));
    }
    let signed = SignedNftTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        signature_hex: crypto_bytes_to_hex(&signature),
    };
    signed.validate().map_err(WalletSdkError::new)?;
    Ok(signed)
}

pub fn wallet_sign_offer_transaction_from_quote(
    backup: &WalletBackupFile,
    quote_response: &RpcResponse,
) -> Result<SignedOfferTransaction, WalletSdkError> {
    let result = validated_summary_result(quote_response, RpcResponseKind::OfferFeeQuote)
        .map_err(|error| WalletSdkError::new(format!("offer fee quote invalid: {error}")))?;
    let operation: OfferTransactionOperation = serde_json::from_value(
        field(result, "operation")
            .map_err(|error| {
                WalletSdkError::new(format!("offer fee quote operation missing: {error}"))
            })?
            .clone(),
    )
    .map_err(|error| {
        WalletSdkError::new(format!("offer fee quote operation parse failed: {error}"))
    })?;
    operation.validate().map_err(WalletSdkError::new)?;
    wallet_sign_offer_transaction_from_fields(
        backup,
        WalletSignOfferTransactionFields {
            chain_id: clean_string_field(result, "chain_id")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            genesis_hash: string_field(result, "genesis_hash")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            protocol_version: nonzero_u32_field(result, "protocol_version")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            source: clean_string_field(result, "source")
                .map_err(|error| WalletSdkError::new(error.to_string()))?
                .to_string(),
            fee: nonzero_u64_field(result, "minimum_fee")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            sequence: nonzero_u64_field(result, "sequence")
                .map_err(|error| WalletSdkError::new(error.to_string()))?,
            operation,
        },
    )
}

pub fn wallet_sign_offer_transaction_from_fields(
    backup: &WalletBackupFile,
    fields: WalletSignOfferTransactionFields,
) -> Result<SignedOfferTransaction, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    if fields.chain_id != backup.chain_id {
        return Err(WalletSdkError::new(format!(
            "offer transaction chain_id `{}` does not match wallet backup chain_id `{}`",
            fields.chain_id, backup.chain_id
        )));
    }
    if fields.fee == 0 {
        return Err(WalletSdkError::new(
            "wallet offer transaction fee must be nonzero",
        ));
    }
    if fields.sequence == 0 {
        return Err(WalletSdkError::new(
            "wallet offer transaction sequence must be nonzero",
        ));
    }

    let key_pair = derive_wallet_key_pair(backup)?;
    let source = address_from_public_key(&key_pair.public_key);
    if fields.source != source {
        return Err(WalletSdkError::new(format!(
            "offer transaction source `{}` does not match wallet address `{}`",
            fields.source, source
        )));
    }
    let public_key_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let unsigned = UnsignedOfferTransaction {
        chain_id: fields.chain_id,
        genesis_hash: fields.genesis_hash,
        protocol_version: fields.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: fields.operation.transaction_kind().to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source,
        fee: fields.fee,
        sequence: fields.sequence,
        operation: fields.operation,
    };
    unsigned.validate().map_err(WalletSdkError::new)?;
    let signing_bytes = unsigned.signing_bytes();
    let signature = ml_dsa_65_sign(&key_pair.private_key, &signing_bytes)
        .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify(&key_pair.public_key, &signing_bytes, &signature) {
        return Err(WalletSdkError::new(
            "wallet offer transaction signature verification failed",
        ));
    }
    let signed = SignedOfferTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        signature_hex: crypto_bytes_to_hex(&signature),
    };
    signed.validate().map_err(WalletSdkError::new)?;
    Ok(signed)
}

pub fn wallet_sign_owned_transfer_order(
    backup: &WalletBackupFile,
    order: OwnedTransferOrder,
) -> Result<WalletOwnedTransferSignature, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    validate_owned_certificate_domain_for_wallet(backup, &order.domain)?;
    require_owned_certificate_domain_schema(
        &order.domain,
        postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2,
    )?;
    if order.inputs.is_empty() {
        return Err(WalletSdkError::new(
            "owned-transfer order must consume at least one input",
        ));
    }
    if order.outputs.is_empty() {
        return Err(WalletSdkError::new(
            "owned-transfer order must create at least one output",
        ));
    }

    let key_pair = derive_wallet_key_pair(backup)?;
    let owner_pubkey_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let signing_bytes = owned_transfer_signing_bytes(&order);
    let signature = ml_dsa_65_sign_with_context(
        &key_pair.private_key,
        &signing_bytes,
        OWNED_TRANSFER_CONTEXT,
    )
    .map_err(|error| WalletSdkError::new(format!("owned-transfer sign failed: {error}")))?;
    if !ml_dsa_65_verify_with_context(
        &key_pair.public_key,
        &signing_bytes,
        &signature,
        OWNED_TRANSFER_CONTEXT,
    ) {
        return Err(WalletSdkError::new(
            "owned-transfer owner signature verification failed",
        ));
    }

    Ok(WalletOwnedTransferSignature {
        owner_pubkey_hex,
        owner_signature_hex: crypto_bytes_to_hex(&signature),
        order,
    })
}

pub fn wallet_sign_owned_unwrap_order(
    backup: &WalletBackupFile,
    order: OwnedUnwrapOrder,
) -> Result<WalletOwnedUnwrapSignature, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    validate_owned_certificate_domain_for_wallet(backup, &order.domain)?;
    require_owned_certificate_domain_schema(
        &order.domain,
        postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2,
    )?;
    if order.inputs.is_empty() {
        return Err(WalletSdkError::new(
            "owned-unwrap order must consume at least one input",
        ));
    }
    if order.to_address.is_empty() {
        return Err(WalletSdkError::new(
            "owned-unwrap order requires a destination account",
        ));
    }
    if order.amount == 0 {
        return Err(WalletSdkError::new("owned-unwrap amount must be positive"));
    }
    if order.asset.is_empty() {
        return Err(WalletSdkError::new("owned-unwrap asset is required"));
    }

    let key_pair = derive_wallet_key_pair(backup)?;
    let owner_pubkey_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let signing_bytes = owned_unwrap_signing_bytes(&order);
    let signature =
        ml_dsa_65_sign_with_context(&key_pair.private_key, &signing_bytes, OWNED_UNWRAP_CONTEXT)
            .map_err(|error| WalletSdkError::new(format!("owned-unwrap sign failed: {error}")))?;
    if !ml_dsa_65_verify_with_context(
        &key_pair.public_key,
        &signing_bytes,
        &signature,
        OWNED_UNWRAP_CONTEXT,
    ) {
        return Err(WalletSdkError::new(
            "owned-unwrap owner signature verification failed",
        ));
    }

    Ok(WalletOwnedUnwrapSignature {
        owner_pubkey_hex,
        owner_signature_hex: crypto_bytes_to_hex(&signature),
        order,
    })
}

pub fn wallet_sign_owned_transfer_order_v3(
    backup: &WalletBackupFile,
    order: postfiat_types::OwnedTransferOrderV3,
    capabilities: &postfiat_types::FastPayRecoveryCapabilitiesV1,
) -> Result<postfiat_types::SignedOwnedTransferOrderV3, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    validate_fastpay_v3_wallet_context(backup, &order.domain, &order.recovery, capabilities)?;
    if order.inputs.is_empty() || order.outputs.is_empty() {
        return Err(WalletSdkError::new(
            "FastPay v3 transfer requires inputs and outputs",
        ));
    }
    if order.recovery.lock_id != postfiat_types::fastpay_transfer_lock_id_v1(&order) {
        return Err(WalletSdkError::new(
            "FastPay v3 transfer lock ID does not match its order",
        ));
    }
    let key_pair = derive_wallet_key_pair(backup)?;
    let owner_pubkey_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let signing_bytes = postfiat_execution::owned_transfer_v3_signing_bytes(&order);
    let signature = ml_dsa_65_sign_with_context(
        &key_pair.private_key,
        &signing_bytes,
        postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
    )
    .map_err(|error| WalletSdkError::new(format!("FastPay v3 transfer sign failed: {error}")))?;
    if !ml_dsa_65_verify_with_context(
        &key_pair.public_key,
        &signing_bytes,
        &signature,
        postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
    ) {
        return Err(WalletSdkError::new(
            "FastPay v3 transfer signature verification failed",
        ));
    }
    Ok(postfiat_types::SignedOwnedTransferOrderV3 {
        order,
        owner_pubkey_hex,
        owner_signature_hex: crypto_bytes_to_hex(&signature),
    })
}

pub fn wallet_sign_owned_unwrap_order_v3(
    backup: &WalletBackupFile,
    order: postfiat_types::OwnedUnwrapOrderV3,
    capabilities: &postfiat_types::FastPayRecoveryCapabilitiesV1,
) -> Result<postfiat_types::SignedOwnedUnwrapOrderV3, WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    validate_fastpay_v3_wallet_context(backup, &order.domain, &order.recovery, capabilities)?;
    if order.inputs.is_empty() || order.to_address.is_empty() || order.amount == 0 {
        return Err(WalletSdkError::new(
            "FastPay v3 unwrap requires inputs, amount, and destination",
        ));
    }
    if order.recovery.lock_id != postfiat_types::fastpay_unwrap_lock_id_v1(&order) {
        return Err(WalletSdkError::new(
            "FastPay v3 unwrap lock ID does not match its order",
        ));
    }
    let key_pair = derive_wallet_key_pair(backup)?;
    let owner_pubkey_hex = crypto_bytes_to_hex(&key_pair.public_key);
    let signing_bytes = postfiat_execution::owned_unwrap_v3_signing_bytes(&order);
    let signature = ml_dsa_65_sign_with_context(
        &key_pair.private_key,
        &signing_bytes,
        postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
    )
    .map_err(|error| WalletSdkError::new(format!("FastPay v3 unwrap sign failed: {error}")))?;
    if !ml_dsa_65_verify_with_context(
        &key_pair.public_key,
        &signing_bytes,
        &signature,
        postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
    ) {
        return Err(WalletSdkError::new(
            "FastPay v3 unwrap signature verification failed",
        ));
    }
    Ok(postfiat_types::SignedOwnedUnwrapOrderV3 {
        order,
        owner_pubkey_hex,
        owner_signature_hex: crypto_bytes_to_hex(&signature),
    })
}

pub fn wallet_fastpay_transfer_lock_id_v1(order: &postfiat_types::OwnedTransferOrderV3) -> String {
    postfiat_types::fastpay_transfer_lock_id_v1(order)
}

pub fn wallet_fastpay_unwrap_lock_id_v1(order: &postfiat_types::OwnedUnwrapOrderV3) -> String {
    postfiat_types::fastpay_unwrap_lock_id_v1(order)
}

pub fn wallet_fastpay_transfer_certificate_digest_v3(
    certificate: &postfiat_types::OwnedTransferCertificateV3,
) -> Result<String, WalletSdkError> {
    postfiat_execution::fastpay_transfer_certificate_digest_v3(certificate)
        .map_err(|error| WalletSdkError::new(format!("FastPay certificate digest: {error:?}")))
}

pub fn wallet_fastpay_unwrap_certificate_digest_v3(
    certificate: &postfiat_types::OwnedUnwrapCertificateV3,
) -> Result<String, WalletSdkError> {
    postfiat_execution::fastpay_unwrap_certificate_digest_v3(certificate)
        .map_err(|error| WalletSdkError::new(format!("FastPay certificate digest: {error:?}")))
}

pub fn wallet_verify_fastpay_apply_ack_v1(
    acknowledgement: &postfiat_types::FastPayApplyAckV1,
    validator_public_key_hex: &str,
) -> Result<(), WalletSdkError> {
    acknowledgement
        .validate_shape()
        .map_err(|error| WalletSdkError::new(format!("FastPay apply acknowledgement: {error}")))?;
    if !postfiat_execution::verify_fastpay_apply_ack_v1(acknowledgement, validator_public_key_hex) {
        return Err(WalletSdkError::new(
            "FastPay apply acknowledgement signature is invalid",
        ));
    }
    Ok(())
}

fn validate_fastpay_v3_wallet_context(
    backup: &WalletBackupFile,
    domain: &OwnedCertificateDomain,
    recovery: &postfiat_types::FastPayOrderRecoveryV1,
    capabilities: &postfiat_types::FastPayRecoveryCapabilitiesV1,
) -> Result<(), WalletSdkError> {
    capabilities.validate().map_err(WalletSdkError::new)?;
    validate_owned_certificate_domain_for_wallet(backup, domain)?;
    require_owned_certificate_domain_schema(
        domain,
        postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3,
    )?;
    if domain != &capabilities.domain
        || recovery.committee_epoch != capabilities.committee_epoch
        || capabilities.current_height < recovery.valid_from_height
        || capabilities.current_height > recovery.expires_at_height
    {
        return Err(WalletSdkError::new(
            "FastPay v3 order does not match the live recovery capability",
        ));
    }
    recovery
        .validate(&capabilities.policy)
        .map_err(WalletSdkError::new)
}

pub(crate) fn owned_transfer_signing_bytes(order: &OwnedTransferOrder) -> Vec<u8> {
    let mut out = b"postfiat.owned-transfer.v2\0".to_vec();
    append_owned_certificate_domain(&mut out, &order.domain);
    out.extend(&(order.inputs.len() as u64).to_le_bytes());
    for input in &order.inputs {
        out.extend(&(input.id.len() as u64).to_le_bytes());
        out.extend(input.id.as_bytes());
        out.extend(&input.version.to_le_bytes());
    }
    out.extend(&(order.outputs.len() as u64).to_le_bytes());
    for output in &order.outputs {
        out.extend(&(output.owner_pubkey_hex.len() as u64).to_le_bytes());
        out.extend(output.owner_pubkey_hex.as_bytes());
        out.extend(&output.value.to_le_bytes());
        out.extend(&(output.asset.len() as u64).to_le_bytes());
        out.extend(output.asset.as_bytes());
    }
    out.extend(&order.fee.to_le_bytes());
    out.extend(&order.nonce.to_le_bytes());
    out.extend(&(order.memos.len() as u64).to_le_bytes());
    for memo in &order.memos {
        out.extend(memo.memo_type.as_bytes());
        out.push(0);
        out.extend(memo.memo_format.as_bytes());
        out.push(0);
        out.extend(memo.memo_data.as_bytes());
        out.push(0);
    }
    out
}

pub(crate) fn owned_unwrap_signing_bytes(order: &OwnedUnwrapOrder) -> Vec<u8> {
    let mut out = b"postfiat.owned-unwrap.v2\0".to_vec();
    append_owned_certificate_domain(&mut out, &order.domain);
    out.extend(&(order.inputs.len() as u64).to_le_bytes());
    for input in &order.inputs {
        out.extend(&(input.id.len() as u64).to_le_bytes());
        out.extend(input.id.as_bytes());
        out.extend(&input.version.to_le_bytes());
    }
    out.extend(&(order.to_address.len() as u64).to_le_bytes());
    out.extend(order.to_address.as_bytes());
    out.extend(&order.amount.to_le_bytes());
    out.extend(&(order.asset.len() as u64).to_le_bytes());
    out.extend(order.asset.as_bytes());
    out.extend(&order.fee.to_le_bytes());
    out.extend(&order.nonce.to_le_bytes());
    out.extend(&(order.memos.len() as u64).to_le_bytes());
    for memo in &order.memos {
        out.extend(memo.memo_type.as_bytes());
        out.push(0);
        out.extend(memo.memo_format.as_bytes());
        out.push(0);
        out.extend(memo.memo_data.as_bytes());
        out.push(0);
    }
    out
}

fn append_owned_certificate_domain(out: &mut Vec<u8>, domain: &OwnedCertificateDomain) {
    for value in [
        domain.schema.as_str(),
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.registry_id.as_str(),
    ] {
        out.extend(&(value.len() as u64).to_le_bytes());
        out.extend(value.as_bytes());
    }
    out.extend(&domain.protocol_version.to_le_bytes());
}

/// Fail closed before a wallet signs a FastPay order for a foreign or malformed
/// certificate domain. The genesis and registry values are signed exactly; the
/// validator independently compares them with its live chain configuration.
pub fn validate_owned_certificate_domain_for_wallet(
    backup: &WalletBackupFile,
    domain: &OwnedCertificateDomain,
) -> Result<(), WalletSdkError> {
    if !matches!(
        domain.schema.as_str(),
        postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2
            | postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3
    ) {
        return Err(WalletSdkError::new(
            "FastPay certificate domain uses an unsupported schema",
        ));
    }
    if domain.chain_id != backup.chain_id {
        return Err(WalletSdkError::new(format!(
            "FastPay certificate domain chain `{}` does not match wallet chain `{}`",
            domain.chain_id, backup.chain_id
        )));
    }
    if domain.protocol_version == 0 {
        return Err(WalletSdkError::new(
            "FastPay certificate domain protocol version must be nonzero",
        ));
    }
    for (field, value) in [
        ("genesis_hash", domain.genesis_hash.as_str()),
        ("registry_id", domain.registry_id.as_str()),
    ] {
        let bytes = crypto_hex_to_bytes(value).map_err(|error| {
            WalletSdkError::new(format!(
                "FastPay certificate domain {field} is invalid: {error}"
            ))
        })?;
        if bytes.len() != 48 {
            return Err(WalletSdkError::new(format!(
                "FastPay certificate domain {field} must encode 48 bytes"
            )));
        }
    }
    Ok(())
}

fn require_owned_certificate_domain_schema(
    domain: &OwnedCertificateDomain,
    expected: &str,
) -> Result<(), WalletSdkError> {
    if domain.schema != expected {
        return Err(WalletSdkError::new(format!(
            "FastPay order requires certificate domain schema `{expected}`"
        )));
    }
    Ok(())
}

fn validate_wallet_backup_file(backup: &WalletBackupFile) -> Result<(), WalletSdkError> {
    if backup.schema != WALLET_BACKUP_FILE_SCHEMA {
        return Err(WalletSdkError::new(format!(
            "wallet backup uses unsupported schema `{}`",
            backup.schema
        )));
    }
    if backup.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(WalletSdkError::new(format!(
            "wallet backup uses unsupported algorithm `{}`",
            backup.algorithm_id
        )));
    }
    if backup.kdf != WALLET_DERIVATION_KDF {
        return Err(WalletSdkError::new(format!(
            "wallet backup uses unsupported KDF `{}`",
            backup.kdf
        )));
    }
    if backup.derivation_domain != WALLET_DERIVATION_DOMAIN {
        return Err(WalletSdkError::new(format!(
            "wallet backup uses unsupported derivation domain `{}`",
            backup.derivation_domain
        )));
    }
    validate_wallet_chain_id(&backup.chain_id)?;
    if backup.key_role != WALLET_KEY_ROLE_TRANSPARENT_SPEND {
        return Err(WalletSdkError::new(format!(
            "wallet backup uses unsupported key role `{}`",
            backup.key_role
        )));
    }
    let normalized_seed = normalized_wallet_master_seed_hex(&backup.master_seed_hex)?;
    if backup.master_seed_hex != normalized_seed {
        return Err(WalletSdkError::new(
            "wallet backup master seed must be lowercase canonical hex",
        ));
    }
    Ok(())
}

fn validate_wallet_chain_id(chain_id: &str) -> Result<(), WalletSdkError> {
    postfiat_types::Genesis::try_new(chain_id.to_string()).map_err(WalletSdkError::new)?;
    Ok(())
}

pub fn derive_wallet_key_pair(
    backup: &WalletBackupFile,
) -> Result<postfiat_crypto_provider::MlDsa65KeyPair, WalletSdkError> {
    let seed = derive_wallet_seed(backup)?;
    Ok(ml_dsa_65_keygen_from_seed(&seed))
}

fn derive_wallet_seed(backup: &WalletBackupFile) -> Result<[u8; 32], WalletSdkError> {
    validate_wallet_backup_file(backup)?;
    let master_seed = Zeroizing::new(wallet_master_seed_bytes(&backup.master_seed_hex)?);
    let derivation_payload = serde_json::to_vec(&(
        WALLET_DERIVATION_DOMAIN,
        backup.algorithm_id.as_str(),
        backup.chain_id.as_str(),
        backup.account_index,
        backup.key_role.as_str(),
        crypto_bytes_to_hex(&master_seed[..]),
    ))
    .map_err(|error| WalletSdkError::new(error.to_string()))?;
    let digest = crypto_hash_bytes(WALLET_DERIVATION_DOMAIN, &derivation_payload);
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&digest[..32]);
    Ok(seed)
}

fn normalized_wallet_master_seed_hex(seed_hex: &str) -> Result<String, WalletSdkError> {
    Ok(crypto_bytes_to_hex(&wallet_master_seed_bytes(seed_hex)?))
}

fn wallet_master_seed_bytes(seed_hex: &str) -> Result<[u8; 32], WalletSdkError> {
    let bytes = crypto_hex_to_bytes(seed_hex).map_err(|error| {
        WalletSdkError::new(format!("wallet master seed hex is invalid: {error}"))
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        WalletSdkError::new(format!(
            "wallet master seed hex must decode to 32 bytes, got {}",
            bytes.len()
        ))
    })
}
