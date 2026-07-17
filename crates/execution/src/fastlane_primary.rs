use crate::apply_owned_deposit;
use crate::fastswap_bridge::{execute_fastlane_deposit, execute_fastlane_redeem};
use crate::fastswap_checkpoint::anchor_fastlane_checkpoint;
use crate::fastswap_control::execute_fastlane_control;
use postfiat_crypto_provider::bytes_to_hex;
use postfiat_types::{
    FastLanePrimaryOperationV1, FastLanePrimaryTransactionV1, FastPayOrderRecoveryV1, LedgerState,
    OwnedCertificateDomain, Receipt,
};

fn fastpay_recovery_context(
    ledger: &LedgerState,
    domain: &OwnedCertificateDomain,
    recovery: &FastPayOrderRecoveryV1,
    finalized_height: u64,
) -> Result<
    (
        postfiat_types::FastPayRecoveryPolicyV1,
        postfiat_types::FastPayRecoveryCommitteeV1,
    ),
    &'static str,
> {
    let policy = ledger
        .fastpay_recovery_policy
        .clone()
        .ok_or("FastPay recovery policy is not active")?;
    policy
        .validate()
        .map_err(|_| "FastPay recovery policy is invalid")?;
    if finalized_height < policy.activation_height {
        return Err("FastPay recovery policy has not activated");
    }
    let committee = ledger
        .fastpay_recovery_committees
        .iter()
        .find(|committee| {
            committee.committee_epoch == recovery.committee_epoch
                && committee.registry_root == domain.registry_id
        })
        .cloned()
        .ok_or("FastPay recovery committee is unknown")?;
    committee
        .validate()
        .map_err(|_| "FastPay recovery committee is invalid")?;
    if committee.certificate_domain() != *domain
        || recovery.valid_from_height < committee.valid_from_height
        || recovery.valid_from_height > committee.new_orders_through_height
    {
        return Err("FastPay order is outside its committee domain or admission window");
    }
    Ok((policy, committee))
}

pub fn execute_fastlane_primary_transaction(
    ledger: &mut LedgerState,
    transaction: &FastLanePrimaryTransactionV1,
    finalized_height: u64,
) -> Receipt {
    let tx_id = match transaction.tx_id() {
        Ok(id) => bytes_to_hex(&id.0),
        Err(error) => {
            return Receipt::rejected(
                "fastlane-invalid-id",
                "fastlane_bad_envelope",
                format!("FastLane primary encoding failed: {error:?}"),
            );
        }
    };
    match &transaction.operation {
        FastLanePrimaryOperationV1::Deposit { signed } => {
            let Some(rule) = ledger.fast_lane_asset_rules.iter().find(|rule| {
                rule.rule_hash().ok() == Some(signed.deposit.asset_rule_hash)
                    && rule.asset_id == signed.deposit.asset_id
            }) else {
                return Receipt::rejected(
                    tx_id,
                    "fastlane_unknown_asset_rule",
                    "FastLane deposit asset rule is not canonically registered",
                );
            };
            if !ledger
                .fastswap_committees
                .iter()
                .any(|committee| committee.domain.chain == signed.deposit.domain)
            {
                return Receipt::rejected(
                    tx_id,
                    "fastlane_unknown_domain",
                    "FastLane deposit domain has no canonical committee",
                );
            }
            let expected_domain = signed.deposit.domain.clone();
            let rule = rule.clone();
            match execute_fastlane_deposit(
                ledger,
                signed,
                &expected_domain,
                &rule,
                finalized_height,
            ) {
                Ok(receipt) if receipt.accepted && receipt.code == "fastlane_deposit_applied" => {
                    Receipt::accepted(tx_id, "FastLane deposit applied")
                        .with_code("fastlane_deposit_applied")
                        .with_fee_policy(signed.deposit.fee_pft, signed.deposit.fee_pft, 1, 0)
                }
                Ok(_) => Receipt::rejected(
                    tx_id,
                    "fastlane_deposit_nonterminal",
                    "FastLane deposit did not produce its accepted receipt code",
                ),
                Err(error) => Receipt::rejected(
                    tx_id,
                    "fastlane_deposit_rejected",
                    format!("FastLane deposit rejected: {error:?}"),
                ),
            }
        }
        FastLanePrimaryOperationV1::OwnedDeposit { signed } => {
            match apply_owned_deposit(ledger, signed, finalized_height) {
                Ok(_) => Receipt::accepted(tx_id, "signed account-to-FastPay deposit applied")
                    .with_code("owned_deposit_applied")
                    .with_fee_policy(signed.deposit.fee_pft, signed.deposit.fee_pft, 1, 0),
                Err(error) => Receipt::rejected(
                    tx_id,
                    "owned_deposit_rejected",
                    format!("signed account-to-FastPay deposit rejected: {error:?}"),
                ),
            }
        }
        FastLanePrimaryOperationV1::Redeem { signed } => {
            let Some(committee) = ledger
                .fastswap_committees
                .iter()
                .find(|committee| committee.domain == signed.claim.committee)
                .cloned()
            else {
                return Receipt::rejected(
                    tx_id,
                    "fastlane_unknown_exit_committee",
                    "FastLane exit committee is not canonically registered",
                );
            };
            // V1 rotation requires the final drain checkpoint to contain no
            // outstanding exit claims. Without a checkpoint membership proof,
            // accepting an arbitrary retired-epoch claim could debit a reserve
            // while its pre-exit object was migrated as live. Active-committee
            // claims remain redeemable normally; retired claims fail closed.
            let authorized = fastlane_redemption_committee_active(ledger, &committee);
            match execute_fastlane_redeem(ledger, signed, &committee, authorized) {
                Ok(receipt) if receipt.accepted && receipt.code == "fastlane_exit_redeemed" => {
                    Receipt::accepted(tx_id, "FastLane exit redeemed")
                        .with_code("fastlane_exit_redeemed")
                }
                Ok(_) => Receipt::rejected(
                    tx_id,
                    "fastlane_redeem_nonterminal",
                    "FastLane redemption did not produce its accepted receipt code",
                ),
                Err(error) => Receipt::rejected(
                    tx_id,
                    "fastlane_redeem_rejected",
                    format!("FastLane redemption rejected: {error:?}"),
                ),
            }
        }
        FastLanePrimaryOperationV1::AnchorCheckpoint { certificate } => {
            let native_fee_burn = match certificate
                .votes
                .first()
                .and_then(|vote| {
                    vote.checkpoint
                        .pending_fee_burn_totals
                        .iter()
                        .find(|row| row.asset_id == postfiat_types::FastAssetIdV1::native_pft())
                })
                .map(|row| u64::try_from(row.amount_atoms))
                .transpose()
            {
                Ok(value) => value.unwrap_or(0),
                Err(_) => {
                    return Receipt::rejected(
                        tx_id,
                        "fastlane_checkpoint_native_burn_overflow",
                        "FastLane checkpoint native fee burn does not fit the canonical receipt amount",
                    );
                }
            };
            match anchor_fastlane_checkpoint(ledger, certificate) {
                Ok(_) => {
                    let receipt = Receipt::accepted(tx_id, "FastLane checkpoint anchored")
                        .with_code("fastlane_checkpoint_anchored");
                    if native_fee_burn == 0 {
                        receipt
                    } else {
                        receipt.with_fee_policy(native_fee_burn, native_fee_burn, 0, 0)
                    }
                }
                Err(error) => Receipt::rejected(
                    tx_id,
                    "fastlane_checkpoint_rejected",
                    format!("FastLane checkpoint rejected: {error:?}"),
                ),
            }
        }
        FastLanePrimaryOperationV1::Control { certificate } => {
            match execute_fastlane_control(ledger, certificate, finalized_height) {
                Ok(()) => Receipt::accepted(tx_id, "FastLane control applied")
                    .with_code("fastlane_control_applied"),
                Err(error) => Receipt::rejected(
                    tx_id,
                    "fastlane_control_rejected",
                    format!("FastLane control rejected: {error:?}"),
                ),
            }
        }
        FastLanePrimaryOperationV1::FastPayRecoveryReveal { certificate } => {
            let (policy, committee) = match fastpay_recovery_context(
                ledger,
                certificate.domain(),
                certificate.recovery(),
                finalized_height,
            ) {
                Ok(context) => context,
                Err(error) => {
                    return Receipt::rejected(tx_id, "fastpay_recovery_reveal_rejected", error);
                }
            };
            let mut prospective = ledger.clone();
            let validator_public_keys = committee.validator_public_keys();
            let domain = committee.certificate_domain();
            let context = crate::FastPayRecoveryVerificationContext {
                validator_public_keys: &validator_public_keys,
                expected_domain: &domain,
                committee_epoch: committee.committee_epoch,
                policy: &policy,
                quorum: committee.quorum,
            };
            match crate::record_fastpay_recovery_reveal_v1(
                &mut prospective,
                certificate.clone(),
                context,
                finalized_height,
            ) {
                Ok(_) => {
                    *ledger = prospective;
                    Receipt::accepted(tx_id, "FastPay recovery certificate revealed")
                        .with_code("fastpay_recovery_certificate_revealed")
                }
                Err(error) => Receipt::rejected(
                    tx_id,
                    "fastpay_recovery_reveal_rejected",
                    format!("FastPay recovery reveal rejected: {error:?}"),
                ),
            }
        }
        FastLanePrimaryOperationV1::FastPayRecoveryDecision { request } => {
            let (policy, committee) = match fastpay_recovery_context(
                ledger,
                request.signed_order.domain(),
                request.signed_order.recovery(),
                finalized_height,
            ) {
                Ok(context) => context,
                Err(error) => {
                    return Receipt::rejected(tx_id, "fastpay_recovery_decision_rejected", error);
                }
            };
            let mut prospective = ledger.clone();
            let validator_public_keys = committee.validator_public_keys();
            let domain = committee.certificate_domain();
            let context = crate::FastPayRecoveryVerificationContext {
                validator_public_keys: &validator_public_keys,
                expected_domain: &domain,
                committee_epoch: committee.committee_epoch,
                policy: &policy,
                quorum: committee.quorum,
            };
            match crate::execute_fastpay_recovery_decision_v1(
                &mut prospective,
                request,
                context,
                finalized_height,
            ) {
                Ok(fence) => {
                    *ledger = prospective;
                    match fence.decision {
                        postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. } => {
                            Receipt::accepted(tx_id, "FastPay recovery confirmed certified payment")
                                .with_code("fastpay_recovery_confirmed")
                        }
                        postfiat_types::FastPayRecoveryDecisionV1::Cancelled => {
                            Receipt::accepted(tx_id, "FastPay abandoned lock cancelled and fenced")
                                .with_code("fastpay_recovery_cancelled")
                        }
                    }
                }
                Err(error) => Receipt::rejected(
                    tx_id,
                    "fastpay_recovery_decision_rejected",
                    format!("FastPay recovery decision rejected: {error:?}"),
                ),
            }
        }
    }
}

fn fastlane_redemption_committee_active(
    ledger: &LedgerState,
    committee: &postfiat_types::FastSwapCommitteeV1,
) -> bool {
    ledger.fastswap_committees.last() == Some(committee)
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{
        address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed,
        ml_dsa_65_sign_with_context,
    };
    use postfiat_types::{
        Account, FastAssetDefinitionHashV1, FastAssetIdV1, FastAssetRuleV1, FastLaneDepositV1,
        FastPayCertificateV1, FastPayOrderRecoveryV1, FastPayRecoveryCommitteeV1,
        FastPayRecoveryDecisionRequestV1, FastPayRecoveryPolicyV1, FastPaySignedOrderV1,
        FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
        FastSwapCommitteeV1, FastSwapOpaqueHashV1, FastSwapValidatorV1, OwnedDepositV1,
        OwnedObject, OwnedObjectRef, OwnedOutputSpec, OwnedTransferCertificateV3,
        OwnedTransferOrderV3, OwnedTransferVote, SignedFastLaneDepositV1, SignedOwnedDepositV1,
        SignedOwnedTransferOrderV3, FASTLANE_DEPOSIT_CONTEXT_V1, FASTPAY_ORDER_RECOVERY_SCHEMA_V1,
        FASTPAY_RECOVERY_DECISION_REQUEST_SCHEMA_V1, FASTPAY_RECOVERY_POLICY_SCHEMA_V1,
        FASTSWAP_ML_DSA_65, FASTSWAP_SCHEMA_VERSION_V1, OWNED_DEPOSIT_CONTEXT_V1,
    };

    fn fastpay_ordered_recovery_fixture(
        input_id: &str,
    ) -> (
        LedgerState,
        OwnedTransferCertificateV3,
        SignedOwnedTransferOrderV3,
    ) {
        let owner = ml_dsa_65_keygen_from_seed(&[71; 32]);
        let owner_pubkey_hex = bytes_to_hex(&owner.public_key);
        let validators = (0..4)
            .map(|index| {
                (
                    format!("validator-{index}"),
                    ml_dsa_65_keygen_from_seed(&[72 + index as u8; 32]),
                )
            })
            .collect::<Vec<_>>();
        let committee = FastPayRecoveryCommitteeV1::from_public_keys(
            "fastpay-ordered-recovery".to_string(),
            "73".repeat(48),
            1,
            1,
            90,
            120,
            validators
                .iter()
                .map(|(validator_id, keypair)| {
                    (validator_id.clone(), bytes_to_hex(&keypair.public_key))
                })
                .collect(),
        )
        .expect("recovery committee");
        let mut order = OwnedTransferOrderV3 {
            domain: committee.certificate_domain(),
            recovery: FastPayOrderRecoveryV1 {
                schema: FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
                committee_epoch: 1,
                lock_id: "00".repeat(48),
                valid_from_height: 100,
                expires_at_height: 105,
                recovery_closes_at_height: 110,
            },
            inputs: vec![OwnedObjectRef {
                id: input_id.to_string(),
                version: 1,
            }],
            outputs: vec![OwnedOutputSpec {
                owner_pubkey_hex: "ordered-recovery-recipient".to_string(),
                value: 99,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 1,
            memos: Vec::new(),
        };
        order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
        let signing_bytes = crate::owned_transfer_v3_signing_bytes(&order);
        let owner_signature = ml_dsa_65_sign_with_context(
            &owner.private_key,
            &signing_bytes,
            crate::OWNED_TRANSFER_CONTEXT_V3,
        )
        .expect("owner signature");
        let signed = SignedOwnedTransferOrderV3 {
            order: order.clone(),
            owner_pubkey_hex: owner_pubkey_hex.clone(),
            owner_signature_hex: bytes_to_hex(&owner_signature),
        };
        let votes = validators
            .iter()
            .map(|(validator_id, keypair)| OwnedTransferVote {
                validator_id: validator_id.clone(),
                signature_hex: bytes_to_hex(
                    &ml_dsa_65_sign_with_context(
                        &keypair.private_key,
                        &signing_bytes,
                        crate::OWNED_TRANSFER_CONTEXT_V3,
                    )
                    .expect("validator signature"),
                ),
            })
            .collect();
        let certificate = OwnedTransferCertificateV3 {
            order,
            owner_pubkey_hex: owner_pubkey_hex.clone(),
            owner_signature_hex: signed.owner_signature_hex.clone(),
            votes,
        };
        let mut ledger = LedgerState::empty();
        ledger.fastpay_recovery_policy = Some(FastPayRecoveryPolicyV1 {
            schema: FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
            activation_height: 90,
            max_validity_blocks: 20,
            max_recovery_blocks: 20,
        });
        ledger.fastpay_recovery_committees.push(committee);
        ledger.owned_objects.push(OwnedObject {
            id: input_id.to_string(),
            version: 1,
            owner_pubkey_hex,
            value: 100,
            asset: "PFT".to_string(),
        });
        (ledger, certificate, signed)
    }

    #[test]
    fn canonical_primary_deposit_is_accepted_once_with_exact_code() {
        let owner = ml_dsa_65_keygen_from_seed(&[41; 32]);
        let validator = ml_dsa_65_keygen_from_seed(&[42; 32]);
        let chain = FastSwapChainDomainV1 {
            chain_id: "fastlane-primary-test".to_owned(),
            genesis_hash: FastSwapOpaqueHashV1([43; 48]),
            protocol_version: 1,
        };
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: chain.clone(),
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 1,
                quorum: 1,
            },
            validators: vec![FastSwapValidatorV1 {
                validator_id: "validator-0".to_owned(),
                public_key: validator.public_key,
            }],
        };
        committee.domain.committee_root = committee.computed_root().expect("root");
        let native = FastAssetIdV1::native_pft();
        let rule = FastAssetRuleV1 {
            asset_id: native,
            asset_definition_hash: FastAssetDefinitionHashV1::ZERO,
            issuer_address: "native".to_owned(),
            issuer_control_pubkey: vec![1],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 1,
            valid_through_height: 100,
        };
        let address = address_from_public_key(&owner.public_key);
        let mut ledger = LedgerState::new(vec![Account {
            address: address.clone(),
            balance: 100,
            sequence: 0,
            public_key_hex: Some(bytes_to_hex(&owner.public_key)),
        }]);
        ledger.fastswap_committees.push(committee);
        ledger.fast_lane_asset_rules.push(rule.clone());
        let deposit = FastLaneDepositV1 {
            domain: chain,
            source_address: address,
            source_pubkey: owner.public_key.clone(),
            sequence: 1,
            fee_pft: 2,
            destination_owner_pubkey: owner.public_key.clone(),
            destination_holder_permit_id: None,
            asset_id: native,
            asset_rule_hash: rule.rule_hash().expect("rule hash"),
            amount_atoms: 10,
            nonce: [44; 32],
        };
        let signed = SignedFastLaneDepositV1 {
            signature: ml_dsa_65_sign_with_context(
                &owner.private_key,
                &deposit.signing_bytes().expect("deposit bytes"),
                FASTLANE_DEPOSIT_CONTEXT_V1,
            )
            .expect("sign"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            deposit,
        };
        let transaction = FastLanePrimaryTransactionV1 {
            operation: FastLanePrimaryOperationV1::Deposit { signed },
        };
        let receipt = execute_fastlane_primary_transaction(&mut ledger, &transaction, 10);
        assert!(receipt.accepted);
        assert_eq!(receipt.code, "fastlane_deposit_applied");
        assert_eq!(receipt.fee_charged, 2);
        assert_eq!(receipt.fee_burned, 2);
        assert_eq!(ledger.accounts[0].balance, 88);
        assert_eq!(ledger.fast_lane_reserves[0].amount_atoms, 10);
        assert_eq!(ledger.fast_lane_deposit_receipts.len(), 1);
        let after = ledger.clone();
        let duplicate = execute_fastlane_primary_transaction(&mut ledger, &transaction, 10);
        assert!(!duplicate.accepted);
        assert_eq!(ledger, after);

        let retired = ledger.fastswap_committees[0].clone();
        assert!(fastlane_redemption_committee_active(&ledger, &retired));
        let mut replacement = retired.clone();
        replacement.domain.committee_epoch = 2;
        replacement.domain.committee_root = FastSwapCommitteeRootV1::ZERO;
        replacement.domain.committee_root = replacement.computed_root().expect("replacement root");
        ledger.fastswap_committees.push(replacement.clone());
        assert!(!fastlane_redemption_committee_active(&ledger, &retired));
        assert!(fastlane_redemption_committee_active(&ledger, &replacement));
    }

    #[test]
    fn signed_owned_deposit_is_atomic_conserving_and_replay_safe() {
        let owner = ml_dsa_65_keygen_from_seed(&[51; 32]);
        let address = address_from_public_key(&owner.public_key);
        let mut ledger = LedgerState::new(vec![Account {
            address: address.clone(),
            balance: 100,
            sequence: 0,
            public_key_hex: None,
        }]);
        let deposit = OwnedDepositV1 {
            domain: FastSwapChainDomainV1 {
                chain_id: "owned-deposit-test".to_owned(),
                genesis_hash: FastSwapOpaqueHashV1([52; 48]),
                protocol_version: 1,
            },
            source_address: address,
            source_pubkey: owner.public_key.clone(),
            sequence: 1,
            fee_pft: 2,
            destination_owner_pubkey: owner.public_key.clone(),
            asset: "PFT".to_owned(),
            amount_atoms: 40,
            valid_through_height: 20,
            nonce: [53; 32],
        };
        let signed = SignedOwnedDepositV1 {
            signature: ml_dsa_65_sign_with_context(
                &owner.private_key,
                &deposit.signing_bytes().expect("deposit bytes"),
                OWNED_DEPOSIT_CONTEXT_V1,
            )
            .expect("sign"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            deposit,
        };
        let transaction = FastLanePrimaryTransactionV1 {
            operation: FastLanePrimaryOperationV1::OwnedDeposit { signed },
        };

        let receipt = execute_fastlane_primary_transaction(&mut ledger, &transaction, 10);
        assert!(receipt.accepted);
        assert_eq!(receipt.code, "owned_deposit_applied");
        assert_eq!(receipt.fee_charged, 2);
        assert_eq!(receipt.fee_burned, 2);
        assert_eq!(ledger.accounts[0].balance, 58);
        assert_eq!(ledger.accounts[0].sequence, 1);
        assert_eq!(
            ledger.accounts[0].public_key_hex.as_deref(),
            Some(bytes_to_hex(&owner.public_key).as_str())
        );
        assert_eq!(ledger.owned_objects.len(), 1);
        assert_eq!(ledger.owned_objects[0].value, 40);
        assert_eq!(ledger.owned_objects[0].asset, "PFT");

        let conserved = ledger.accounts[0]
            .balance
            .checked_add(ledger.owned_objects[0].value)
            .and_then(|value| value.checked_add(receipt.fee_burned))
            .expect("conserved total");
        assert_eq!(conserved, 100);

        let after = ledger.clone();
        let replay = execute_fastlane_primary_transaction(&mut ledger, &transaction, 10);
        assert!(!replay.accepted);
        assert_eq!(replay.code, "owned_deposit_rejected");
        assert_eq!(ledger, after);
    }

    #[test]
    fn signed_owned_deposit_failure_matrix_never_mutates() {
        let owner = ml_dsa_65_keygen_from_seed(&[61; 32]);
        let address = address_from_public_key(&owner.public_key);
        let base = LedgerState::new(vec![Account {
            address: address.clone(),
            balance: 100,
            sequence: 0,
            public_key_hex: None,
        }]);
        let deposit = OwnedDepositV1 {
            domain: FastSwapChainDomainV1 {
                chain_id: "owned-deposit-test".to_owned(),
                genesis_hash: FastSwapOpaqueHashV1([62; 48]),
                protocol_version: 1,
            },
            source_address: address,
            source_pubkey: owner.public_key.clone(),
            sequence: 1,
            fee_pft: 1,
            destination_owner_pubkey: owner.public_key.clone(),
            asset: "PFT".to_owned(),
            amount_atoms: 10,
            valid_through_height: 20,
            nonce: [63; 32],
        };
        let sign = |deposit: OwnedDepositV1| SignedOwnedDepositV1 {
            signature: ml_dsa_65_sign_with_context(
                &owner.private_key,
                &deposit.signing_bytes().expect("deposit bytes"),
                OWNED_DEPOSIT_CONTEXT_V1,
            )
            .expect("sign"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            deposit,
        };
        let mut cases = Vec::new();
        let mut wrong_asset = deposit.clone();
        wrong_asset.asset = "pfUSDC".to_owned();
        cases.push((sign(wrong_asset), 10));
        let mut wrong_sequence = deposit.clone();
        wrong_sequence.sequence = 2;
        cases.push((sign(wrong_sequence), 10));
        let mut too_large = deposit.clone();
        too_large.amount_atoms = 100;
        cases.push((sign(too_large), 10));
        cases.push((sign(deposit.clone()), 21));
        let mut bad_signature = sign(deposit);
        bad_signature.signature[0] ^= 1;
        cases.push((bad_signature, 10));

        for (signed, height) in cases {
            let mut ledger = base.clone();
            let transaction = FastLanePrimaryTransactionV1 {
                operation: FastLanePrimaryOperationV1::OwnedDeposit { signed },
            };
            let receipt = execute_fastlane_primary_transaction(&mut ledger, &transaction, height);
            assert!(!receipt.accepted);
            assert_eq!(receipt.code, "owned_deposit_rejected");
            assert_eq!(ledger, base);
        }
    }

    #[test]
    fn ordered_fastpay_recovery_cancels_and_permanently_fences_a_late_certificate() {
        let (mut ledger, certificate, signed_order) =
            fastpay_ordered_recovery_fixture("ordered-cancel-input");
        let decision = FastLanePrimaryTransactionV1 {
            operation: FastLanePrimaryOperationV1::FastPayRecoveryDecision {
                request: FastPayRecoveryDecisionRequestV1 {
                    schema: FASTPAY_RECOVERY_DECISION_REQUEST_SCHEMA_V1.to_string(),
                    submitted_at_height: 110,
                    signed_order: FastPaySignedOrderV1::Transfer(signed_order),
                },
            },
        };
        let receipt = execute_fastlane_primary_transaction(&mut ledger, &decision, 110);
        assert!(receipt.accepted);
        assert_eq!(receipt.code, "fastpay_recovery_cancelled");
        assert_eq!(ledger.owned_objects[0].version, 2);
        assert_eq!(ledger.owned_objects[0].value, 100);
        assert_eq!(ledger.fastpay_version_fences.len(), 1);

        let cancelled = ledger.clone();
        let late_reveal = FastLanePrimaryTransactionV1 {
            operation: FastLanePrimaryOperationV1::FastPayRecoveryReveal {
                certificate: FastPayCertificateV1::Transfer(certificate),
            },
        };
        let late = execute_fastlane_primary_transaction(&mut ledger, &late_reveal, 111);
        assert!(!late.accepted);
        assert_eq!(late.code, "fastpay_recovery_reveal_rejected");
        assert_eq!(ledger, cancelled);
    }

    #[test]
    fn ordered_fastpay_recovery_reveal_confirms_exactly_once_with_conservation() {
        let (mut ledger, certificate, signed_order) =
            fastpay_ordered_recovery_fixture("ordered-confirm-input");
        let reveal = FastLanePrimaryTransactionV1 {
            operation: FastLanePrimaryOperationV1::FastPayRecoveryReveal {
                certificate: FastPayCertificateV1::Transfer(certificate.clone()),
            },
        };
        let revealed = execute_fastlane_primary_transaction(&mut ledger, &reveal, 106);
        assert!(revealed.accepted);
        assert_eq!(revealed.code, "fastpay_recovery_certificate_revealed");
        assert_eq!(ledger.fastpay_recovery_reveals.len(), 1);
        assert_eq!(ledger.owned_objects[0].value, 100);

        let decision = FastLanePrimaryTransactionV1 {
            operation: FastLanePrimaryOperationV1::FastPayRecoveryDecision {
                request: FastPayRecoveryDecisionRequestV1 {
                    schema: FASTPAY_RECOVERY_DECISION_REQUEST_SCHEMA_V1.to_string(),
                    submitted_at_height: 110,
                    signed_order: FastPaySignedOrderV1::Transfer(signed_order),
                },
            },
        };
        let confirmed = execute_fastlane_primary_transaction(&mut ledger, &decision, 110);
        assert!(confirmed.accepted);
        assert_eq!(confirmed.code, "fastpay_recovery_confirmed");
        assert!(ledger
            .owned_objects
            .iter()
            .all(|object| object.id != "ordered-confirm-input"));
        assert_eq!(
            ledger
                .owned_objects
                .iter()
                .map(|object| object.value)
                .sum::<u64>()
                + certificate.order.fee,
            100
        );
        assert_eq!(ledger.fastpay_version_fences.len(), 1);
        assert_eq!(
            ledger.fastpay_version_fences[0].certificate,
            Some(FastPayCertificateV1::Transfer(certificate))
        );

        let terminal = ledger.clone();
        let replay = execute_fastlane_primary_transaction(&mut ledger, &decision, 110);
        assert!(replay.accepted);
        assert_eq!(replay.code, "fastpay_recovery_confirmed");
        assert_eq!(ledger, terminal);
    }
}
