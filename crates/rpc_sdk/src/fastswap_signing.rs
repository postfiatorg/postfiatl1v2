use crate::{derive_wallet_key_pair, WalletBackupFile, WalletSdkError};
use postfiat_crypto_provider::{
    address_from_public_key, ml_dsa_65_sign_with_context, ml_dsa_65_verify_with_context,
};
use postfiat_types::{
    FastAssetControlCommandV1, FastLaneDepositV1, FastLanePrimaryOperationV1,
    FastLanePrimaryTransactionV1, FastSwapAuthorizationV1, FastSwapIntentV1, OwnedDepositV1,
    SignedFastAssetControlCommandV1, SignedFastLaneDepositV1, SignedFastSwapIntentV1,
    SignedOwnedDepositV1, FASTLANE_ASSET_CONTROL_CONTEXT_V1, FASTLANE_DEPOSIT_CONTEXT_V1,
    FASTSWAP_INTENT_CONTEXT_V1, FASTSWAP_ML_DSA_65, OWNED_DEPOSIT_CONTEXT_V1,
};

/// Sign one canonical consensus-to-FastLane deposit without changing its
/// source, destination owner, asset rule, amount, sequence, fee, or nonce.
pub fn wallet_sign_fastlane_deposit(
    source_backup: &WalletBackupFile,
    deposit: FastLaneDepositV1,
) -> Result<SignedFastLaneDepositV1, WalletSdkError> {
    if source_backup.chain_id != deposit.domain.chain_id {
        return Err(WalletSdkError::new(
            "FastLane deposit chain_id does not match source wallet backup",
        ));
    }
    let key = derive_wallet_key_pair(source_backup)?;
    if key.public_key != deposit.source_pubkey
        || address_from_public_key(&key.public_key) != deposit.source_address
    {
        return Err(WalletSdkError::new(
            "FastLane deposit source does not match wallet backup",
        ));
    }
    let signing_bytes = deposit
        .signing_bytes()
        .map_err(|error| WalletSdkError::new(format!("invalid FastLane deposit: {error:?}")))?;
    let signature = ml_dsa_65_sign_with_context(
        &key.private_key,
        &signing_bytes,
        FASTLANE_DEPOSIT_CONTEXT_V1,
    )
    .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify_with_context(
        &key.public_key,
        &signing_bytes,
        &signature,
        FASTLANE_DEPOSIT_CONTEXT_V1,
    ) {
        return Err(WalletSdkError::new(
            "FastLane deposit wallet signature self-verification failed",
        ));
    }
    Ok(SignedFastLaneDepositV1 {
        deposit,
        algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
        signature,
    })
}

/// Sign a consensus-ordered account-to-FastPay deposit and return the exact
/// primary-lane transaction submitted by the wallet.
pub fn wallet_sign_owned_deposit(
    source_backup: &WalletBackupFile,
    deposit: OwnedDepositV1,
) -> Result<FastLanePrimaryTransactionV1, WalletSdkError> {
    if source_backup.chain_id != deposit.domain.chain_id {
        return Err(WalletSdkError::new(
            "owned deposit chain_id does not match source wallet backup",
        ));
    }
    let key = derive_wallet_key_pair(source_backup)?;
    if key.public_key != deposit.source_pubkey
        || address_from_public_key(&key.public_key) != deposit.source_address
    {
        return Err(WalletSdkError::new(
            "owned deposit source does not match wallet backup",
        ));
    }
    let signing_bytes = deposit
        .signing_bytes()
        .map_err(|error| WalletSdkError::new(format!("invalid owned deposit: {error:?}")))?;
    let signature =
        ml_dsa_65_sign_with_context(&key.private_key, &signing_bytes, OWNED_DEPOSIT_CONTEXT_V1)
            .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify_with_context(
        &key.public_key,
        &signing_bytes,
        &signature,
        OWNED_DEPOSIT_CONTEXT_V1,
    ) {
        return Err(WalletSdkError::new(
            "owned deposit wallet signature self-verification failed",
        ));
    }
    Ok(FastLanePrimaryTransactionV1 {
        operation: FastLanePrimaryOperationV1::OwnedDeposit {
            signed: SignedOwnedDepositV1 {
                deposit,
                algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
                signature,
            },
        },
    })
}

/// Sign one canonical issuer-control command with the wallet key named by the
/// command. The command is never rewritten: chain domain, object version,
/// action, expiry, and nonce are all signature-bound.
pub fn wallet_sign_fast_asset_control_command(
    issuer_backup: &WalletBackupFile,
    command: FastAssetControlCommandV1,
) -> Result<SignedFastAssetControlCommandV1, WalletSdkError> {
    let signing_bytes = command.canonical_bytes().map_err(|error| {
        WalletSdkError::new(format!("invalid FastLane asset-control command: {error:?}"))
    })?;
    if issuer_backup.chain_id != command.domain.chain.chain_id {
        return Err(WalletSdkError::new(
            "asset-control chain_id does not match issuer wallet backup",
        ));
    }
    let key = derive_wallet_key_pair(issuer_backup)?;
    if key.public_key != command.issuer_control_pubkey
        || address_from_public_key(&key.public_key) != command.issuer_address
    {
        return Err(WalletSdkError::new(
            "asset-control issuer does not match wallet backup",
        ));
    }
    let signature = ml_dsa_65_sign_with_context(
        &key.private_key,
        &signing_bytes,
        FASTLANE_ASSET_CONTROL_CONTEXT_V1,
    )
    .map_err(|error| WalletSdkError::new(error.to_string()))?;
    if !ml_dsa_65_verify_with_context(
        &key.public_key,
        &signing_bytes,
        &signature,
        FASTLANE_ASSET_CONTROL_CONTEXT_V1,
    ) {
        return Err(WalletSdkError::new(
            "asset-control wallet signature self-verification failed",
        ));
    }
    Ok(SignedFastAssetControlCommandV1 {
        command,
        algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
        signature,
    })
}

/// Dual-sign one already-quoted canonical FastSwap intent. This function does
/// not select objects, change amounts, or requote: both wallets sign the same
/// immutable economic preimage after their roles and chain are verified.
pub fn wallet_dual_sign_fastswap_intent(
    owner_0_backup: &WalletBackupFile,
    owner_1_backup: &WalletBackupFile,
    intent: FastSwapIntentV1,
) -> Result<SignedFastSwapIntentV1, WalletSdkError> {
    intent
        .validate_canonical_shape()
        .map_err(|error| WalletSdkError::new(format!("invalid FastSwap intent: {error:?}")))?;
    if owner_0_backup.chain_id != intent.domain.chain.chain_id
        || owner_1_backup.chain_id != intent.domain.chain.chain_id
    {
        return Err(WalletSdkError::new(
            "FastSwap chain_id does not match both wallet backups",
        ));
    }
    let keys = [
        derive_wallet_key_pair(owner_0_backup)?,
        derive_wallet_key_pair(owner_1_backup)?,
    ];
    for (role, (key, party)) in keys
        .iter()
        .zip([&intent.party_0, &intent.party_1])
        .enumerate()
    {
        if key.public_key != party.owner_pubkey
            || address_from_public_key(&key.public_key) != party.owner_address
        {
            return Err(WalletSdkError::new(format!(
                "FastSwap party_{role} does not match its wallet backup"
            )));
        }
    }
    let signing_bytes = intent
        .canonical_bytes()
        .map_err(|error| WalletSdkError::new(format!("FastSwap encoding failed: {error:?}")))?;
    let sign_and_verify = |index: usize| -> Result<Vec<u8>, String> {
        let signature = ml_dsa_65_sign_with_context(
            &keys[index].private_key,
            &signing_bytes,
            FASTSWAP_INTENT_CONTEXT_V1,
        )
        .map_err(|error| error.to_string())?;
        if !ml_dsa_65_verify_with_context(
            &keys[index].public_key,
            &signing_bytes,
            &signature,
            FASTSWAP_INTENT_CONTEXT_V1,
        ) {
            return Err("FastSwap wallet signature self-verification failed".to_owned());
        }
        Ok(signature)
    };
    let (signature_0, signature_1) = std::thread::scope(|scope| {
        let owner_0 = scope.spawn(|| sign_and_verify(0));
        let owner_1 = scope.spawn(|| sign_and_verify(1));
        let signature_0 = owner_0
            .join()
            .map_err(|_| "FastSwap owner-0 signing worker panicked".to_owned())??;
        let signature_1 = owner_1
            .join()
            .map_err(|_| "FastSwap owner-1 signing worker panicked".to_owned())??;
        Ok::<_, String>((signature_0, signature_1))
    })
    .map_err(WalletSdkError::new)?;
    Ok(SignedFastSwapIntentV1 {
        intent,
        authorization_0: FastSwapAuthorizationV1 {
            role: 0,
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            public_key: keys[0].public_key.clone(),
            signature: signature_0,
        },
        authorization_1: FastSwapAuthorizationV1 {
            role: 1,
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            public_key: keys[1].public_key.clone(),
            signature: signature_1,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet_backup_from_master_seed;
    use postfiat_types::{
        FastAssetControlActionV1, FastAssetIdV1, FastAssetRuleHashV1, FastObjectIdV1,
        FastObjectKeyV1, FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
        FastSwapMarketEnvelopeHashV1, FastSwapOpaqueHashV1, FastSwapPartyV1, FastSwapPolicyHashV1,
        FastSwapRfqHashV1, FASTSWAP_SCHEMA_VERSION_V1,
    };

    fn key(byte: u8) -> FastObjectKeyV1 {
        FastObjectKeyV1 {
            object_id: FastObjectIdV1([byte; 32]),
            version: 1,
        }
    }

    #[test]
    fn wallet_signs_exact_fastlane_deposit_and_rejects_source_substitution() {
        let chain_id = "fastlane-deposit-signing-test";
        let backup = wallet_backup_from_master_seed(chain_id, "05".repeat(32), 0).expect("backup");
        let other = wallet_backup_from_master_seed(chain_id, "06".repeat(32), 0).expect("other");
        let source = derive_wallet_key_pair(&backup).expect("source key");
        let deposit = FastLaneDepositV1 {
            domain: FastSwapChainDomainV1 {
                chain_id: chain_id.to_owned(),
                genesis_hash: FastSwapOpaqueHashV1([1; 48]),
                protocol_version: 1,
            },
            source_address: address_from_public_key(&source.public_key),
            source_pubkey: source.public_key.clone(),
            sequence: 7,
            fee_pft: 1,
            destination_owner_pubkey: vec![9; 1952],
            destination_holder_permit_id: None,
            asset_id: FastAssetIdV1([2; 48]),
            asset_rule_hash: FastAssetRuleHashV1([3; 48]),
            amount_atoms: 9,
            nonce: [4; 32],
        };
        let signed = wallet_sign_fastlane_deposit(&backup, deposit.clone()).expect("signed");
        assert_eq!(signed.deposit, deposit);
        assert_eq!(signed.algorithm_id, FASTSWAP_ML_DSA_65);
        assert!(ml_dsa_65_verify_with_context(
            &source.public_key,
            &signed.deposit.signing_bytes().expect("signing bytes"),
            &signed.signature,
            FASTLANE_DEPOSIT_CONTEXT_V1,
        ));
        assert!(wallet_sign_fastlane_deposit(&other, deposit).is_err());
    }

    #[test]
    fn wallet_signs_exact_owned_deposit_transaction_and_rejects_source_substitution() {
        let chain_id = "owned-deposit-signing-test";
        let backup = wallet_backup_from_master_seed(chain_id, "07".repeat(32), 0).expect("backup");
        let other = wallet_backup_from_master_seed(chain_id, "08".repeat(32), 0).expect("other");
        let source = derive_wallet_key_pair(&backup).expect("source key");
        let deposit = OwnedDepositV1 {
            domain: FastSwapChainDomainV1 {
                chain_id: chain_id.to_owned(),
                genesis_hash: FastSwapOpaqueHashV1([9; 48]),
                protocol_version: 1,
            },
            source_address: address_from_public_key(&source.public_key),
            source_pubkey: source.public_key.clone(),
            sequence: 3,
            fee_pft: 1,
            destination_owner_pubkey: source.public_key.clone(),
            asset: "PFT".to_owned(),
            amount_atoms: 17,
            valid_through_height: 100,
            nonce: [10; 32],
        };
        let transaction = wallet_sign_owned_deposit(&backup, deposit.clone()).expect("signed");
        let FastLanePrimaryOperationV1::OwnedDeposit { signed } = transaction.operation else {
            panic!("expected owned deposit operation");
        };
        assert_eq!(signed.deposit, deposit);
        assert_eq!(signed.algorithm_id, FASTSWAP_ML_DSA_65);
        assert!(ml_dsa_65_verify_with_context(
            &source.public_key,
            &signed.deposit.signing_bytes().expect("signing bytes"),
            &signed.signature,
            OWNED_DEPOSIT_CONTEXT_V1,
        ));
        assert!(wallet_sign_owned_deposit(&other, signed.deposit).is_err());
    }

    #[test]
    fn wallet_signs_exact_asset_control_command_and_rejects_substitution() {
        let chain_id = "fastswap-control-signing-test";
        let backup = wallet_backup_from_master_seed(chain_id, "03".repeat(32), 0).expect("backup");
        let other = wallet_backup_from_master_seed(chain_id, "04".repeat(32), 0).expect("other");
        let issuer = derive_wallet_key_pair(&backup).expect("issuer key");
        let command = FastAssetControlCommandV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: chain_id.to_owned(),
                    genesis_hash: FastSwapOpaqueHashV1([1; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 7,
                committee_root: FastSwapCommitteeRootV1([2; 48]),
                validator_count: 6,
                quorum: 5,
            },
            action: FastAssetControlActionV1::Freeze,
            input: key(9),
            issuer_address: address_from_public_key(&issuer.public_key),
            issuer_control_pubkey: issuer.public_key,
            expires_at_height: 100,
            nonce: [3; 32],
        };
        let signed = wallet_sign_fast_asset_control_command(&backup, command.clone())
            .expect("signed command");
        assert_eq!(signed.command, command);
        assert_eq!(signed.algorithm_id, FASTSWAP_ML_DSA_65);
        assert!(wallet_sign_fast_asset_control_command(&other, command.clone()).is_err());

        let mut changed = command;
        changed.domain.chain.chain_id = "different-chain".to_owned();
        assert!(wallet_sign_fast_asset_control_command(&backup, changed).is_err());
    }

    #[test]
    fn wallet_dual_signs_one_exact_fastswap_preimage_and_rejects_role_substitution() {
        let chain_id = "fastswap-signing-test";
        let backup_0 =
            wallet_backup_from_master_seed(chain_id, "01".repeat(32), 0).expect("backup 0");
        let backup_1 =
            wallet_backup_from_master_seed(chain_id, "02".repeat(32), 0).expect("backup 1");
        let key_0 = derive_wallet_key_pair(&backup_0).expect("key 0");
        let key_1 = derive_wallet_key_pair(&backup_1).expect("key 1");
        let party = |role: u8,
                     owner_pubkey: Vec<u8>,
                     offered: FastAssetIdV1,
                     received: FastAssetIdV1| FastSwapPartyV1 {
            owner_address: address_from_public_key(&owner_pubkey),
            owner_pubkey,
            offered_asset_id: offered,
            offered_asset_rule_hash: FastAssetRuleHashV1([offered.0[0] + 10; 48]),
            offered_amount: if role == 0 { 8 } else { 1 },
            receives_asset_id: received,
            receives_asset_rule_hash: FastAssetRuleHashV1([received.0[0] + 10; 48]),
            receives_holder_permit_id: None,
            receives_amount: if role == 0 { 1 } else { 8 },
            asset_inputs: vec![key(role + 1)],
            fee_inputs: vec![key(role + 11)],
            asset_change: 1,
            fee_change: 1,
            fee_burn_pft: 1,
        };
        let intent = FastSwapIntentV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: chain_id.to_owned(),
                    genesis_hash: FastSwapOpaqueHashV1([3; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1([4; 48]),
                validator_count: 6,
                quorum: 5,
            },
            policy_hash: FastSwapPolicyHashV1([5; 48]),
            rfq_hash: FastSwapRfqHashV1([6; 48]),
            market_envelope_hash: FastSwapMarketEnvelopeHashV1([7; 48]),
            nav_epoch: 59,
            expires_at_height: 100,
            nonce: [8; 32],
            party_0: party(
                0,
                key_0.public_key.clone(),
                FastAssetIdV1([1; 48]),
                FastAssetIdV1([2; 48]),
            ),
            party_1: party(
                1,
                key_1.public_key.clone(),
                FastAssetIdV1([2; 48]),
                FastAssetIdV1([1; 48]),
            ),
        };
        let signed = wallet_dual_sign_fastswap_intent(&backup_0, &backup_1, intent.clone())
            .expect("dual signed intent");
        assert_eq!(signed.intent, intent);
        assert_eq!(signed.authorization_0.public_key, key_0.public_key);
        assert_eq!(signed.authorization_1.public_key, key_1.public_key);

        assert!(wallet_dual_sign_fastswap_intent(&backup_1, &backup_0, intent).is_err());
    }
}
