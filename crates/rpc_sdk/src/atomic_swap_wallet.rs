use super::*;

pub fn wallet_sign_atomic_swap_from_quote(
    owner_0_backup: &WalletBackupFile,
    owner_1_backup: &WalletBackupFile,
    quote_request: &RpcRequest,
    quote: &AtomicSwapFeeQuoteSummary,
) -> Result<postfiat_types::SignedAtomicSwapTransaction, WalletSdkError> {
    validate_atomic_swap_quote_summary_for_request(quote, quote_request).map_err(|error| {
        WalletSdkError::new(format!(
            "atomic swap quote does not match its request: {error}"
        ))
    })?;
    let unsigned = quote.unsigned_transaction.clone();
    if owner_0_backup.chain_id != unsigned.chain_id || owner_1_backup.chain_id != unsigned.chain_id
    {
        return Err(WalletSdkError::new(
            "atomic swap quote chain_id does not match both wallet backups",
        ));
    }
    let key_0 = derive_wallet_key_pair(owner_0_backup)?;
    let key_1 = derive_wallet_key_pair(owner_1_backup)?;
    let owner_0 = address_from_public_key(&key_0.public_key);
    let owner_1 = address_from_public_key(&key_1.public_key);
    if unsigned.leg_0.owner != owner_0 {
        return Err(WalletSdkError::new(format!(
            "atomic swap leg_0 owner `{}` does not match wallet address `{owner_0}`",
            unsigned.leg_0.owner
        )));
    }
    if unsigned.leg_1.owner != owner_1 {
        return Err(WalletSdkError::new(format!(
            "atomic swap leg_1 owner `{}` does not match wallet address `{owner_1}`",
            unsigned.leg_1.owner
        )));
    }
    unsigned.validate().map_err(WalletSdkError::new)?;
    let signing_bytes = unsigned.signing_bytes();
    let signature_0 = ml_dsa_65_sign(&key_0.private_key, &signing_bytes)
        .map_err(|error| WalletSdkError::new(error.to_string()))?;
    let signature_1 = ml_dsa_65_sign(&key_1.private_key, &signing_bytes)
        .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify(&key_0.public_key, &signing_bytes, &signature_0)
        || !ml_dsa_65_verify(&key_1.public_key, &signing_bytes, &signature_1)
    {
        return Err(WalletSdkError::new(
            "atomic swap wallet signature self-verification failed",
        ));
    }
    let signed = postfiat_types::SignedAtomicSwapTransaction {
        authorization_0: postfiat_types::AtomicSwapAuthorization {
            owner: owner_0,
            algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: crypto_bytes_to_hex(&key_0.public_key),
            signature_hex: crypto_bytes_to_hex(&signature_0),
        },
        authorization_1: postfiat_types::AtomicSwapAuthorization {
            owner: owner_1,
            algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: crypto_bytes_to_hex(&key_1.public_key),
            signature_hex: crypto_bytes_to_hex(&signature_1),
        },
        unsigned,
    };
    signed.validate().map_err(WalletSdkError::new)?;
    Ok(signed)
}
