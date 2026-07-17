    use postfiat_crypto_provider::{
        address_from_public_key, bytes_to_hex, hex_to_bytes, ml_dsa_65_keygen,
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign, ml_dsa_65_sign_with_context_seed,
        MlDsa65KeyPair, ML_DSA_65_ALGORITHM,
    };
    use postfiat_types::{
        market_ops_asset_id, market_ops_evidence_root, market_ops_reserve_packet_hash,
        market_ops_supply_packet_hash, nav_per_unit_floor, pftl_uniswap_non_consumption_proof_hash,
        pftl_uniswap_return_burn_id_from_fields, vault_bridge_allocation_id,
        vault_bridge_deposit_evidence_root, vault_bridge_deposit_id,
        vault_bridge_deposit_observation_root, vault_bridge_deposit_public_values_hash,
        vault_bridge_pftl_recipient_hash, vault_bridge_withdrawal_execution_observation_root, Account,
        AssetBurnOperation, AssetClawbackOperation, AssetCreateOperation, AssetDefinition,
        AssetTransactionOperation,
        EscrowCancelOperation, EscrowCreateOperation, EscrowFinishOperation,
        EthereumCheckpointCertificateV1, EthereumCheckpointVoteV1,
        EthereumExternalEventProofV1, EthereumFinalizedCheckpointV1, EthereumReceiptProofV1,
        EthereumRouteVerificationPolicyV1, FastAssetIdV1, FastLaneReserveBalanceV1,
        FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
        FastSwapCommitteeV1, FastSwapOpaqueHashV1, FastSwapValidatorV1, IssuedPaymentOperation,
        LedgerState,
        MarketOpsAlignmentParams, MarketOpsEnvelope,
        MarketOpsFinalizeOperation, MarketOpsMintLimits, MarketOpsPolicyInputs,
        MarketOpsPolicyRegisterOperation, MarketOpsPolicyRegistration,
        MarketOpsReserveDeployLimits, MarketOpsVenueObservation,
        NavAssetRegisterOperation, NavAttestor, NavAttestorRegisterOperation, NavEpochFinalizeOperation,
        NavHaltOperation, NavMintAtNavOperation, NavProfileRegisterOperation,
        NavRedeemAtNavOperation, NavRedeemSettleOperation, NavReserveAttestOperation,
        NavReserveChallengeOperation, NavReserveSubmitOperation, NftBurnOperation,
        NftMintOperation, NftTransactionOperation, NftTransferOperation, OfferCancelOperation,
        OfferCreateOperation, OfferTransactionOperation, PaymentMemo,
        PftlUniswapDestinationConsumeOperation, PftlUniswapExportDebitOperation,
        PftlUniswapPrimarySubscribeOperation, PftlUniswapRefundSourceOperation,
        PftlUniswapReturnImportOperation, PftlUniswapRouteInitOperation,
        VaultBridgeDepositEvidence, VaultBridgeDepositAttestOperation, VaultBridgeDepositChallengeOperation,
        VaultBridgeDepositClaimOperation, VaultBridgeDepositFinalizeOperation,
        VaultBridgeDepositObservation, VaultBridgeDepositProposeOperation, VaultBridgeBucketImpairOperation,
        VaultBridgeBurnToRedeemOperation, VaultBridgeMintFromReceiptsOperation,
        VaultBridgeNavSubscriptionAllocateOperation, VaultBridgeReceiptCountOperation,
        VaultBridgeReceiptSubmitOperation, VaultBridgeRedeemSettleOperation,
        VaultBridgeRedemption, VaultBridgeBucketState, VaultBridgeReceipt,
        VaultBridgeWithdrawalExecutionAttestation, VaultBridgeWithdrawalExecutionObservation,
        TrustLine, TrustSetOperation,
        UnsignedAssetTransaction, UnsignedEscrowTransaction, UnsignedNftTransaction,
        UnsignedOfferTransaction, UnsignedPaymentV2, UnsignedTransfer,
        ISSUED_ASSET_ID_HEX_LEN, VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,
        VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,
        MARKET_OPS_FINALIZE_TRANSACTION_KIND, MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND,
        NAV_PROFILE_VERIFIER_MULTI_FETCH,
        NAV_PROFILE_VERIFIER_PLACEHOLDER, NAV_PROFILE_VERIFIER_SP1_GROTH16,
        NFT_COLLECTION_FLAG_BURN_LOCKED,
        NAV_SP1_PROOF_ENCODING_GROTH16, NFT_COLLECTION_FLAG_TRANSFER_LOCKED,
        NFT_FLAG_TRANSFERABLE, OFFER_CANCEL_TRANSACTION_KIND, OFFER_CREATE_TRANSACTION_KIND,
        MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS, PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
        PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED,
        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND, PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
        PFTL_UNISWAP_RETURN_STATUS_IMPORTED, PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
        VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
        VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
        VAULT_BRIDGE_DEPOSIT_CHALLENGE_TRANSACTION_KIND, VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
        VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND, VAULT_BRIDGE_BUCKET_IMPAIR_TRANSACTION_KIND,
        VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED, VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
        VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT, VAULT_BRIDGE_MINT_FROM_RECEIPTS_TRANSACTION_KIND,
        VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND, VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND,
        VAULT_BRIDGE_RECEIPT_STATUS_IMPAIRED, VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND,
        VAULT_BRIDGE_RECEIPT_STATUS_COUNTED, VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND, VAULT_BRIDGE_REDEMPTION_STATE_PENDING,
        VAULT_BRIDGE_REDEMPTION_STATE_SETTLED, VAULT_BRIDGE_UNIT, vault_bridge_source_root_for_asset,
        NavTrackedAsset,
        ETHEREUM_CHECKPOINT_SCHEMA_V1, ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
        PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V1,
        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED,
    };

    use super::*;

    fn p0_ethereum_rlp_bytes(bytes: &[u8]) -> Vec<u8> {
        if bytes.len() == 1 && bytes[0] <= 0x7f {
            return bytes.to_vec();
        }
        if bytes.len() < 56 {
            let mut encoded = vec![0x80 + bytes.len() as u8];
            encoded.extend_from_slice(bytes);
            return encoded;
        }
        let length_bytes = bytes.len().to_be_bytes();
        let first = length_bytes
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(length_bytes.len() - 1);
        let length = &length_bytes[first..];
        let mut encoded = vec![0xb7 + length.len() as u8];
        encoded.extend_from_slice(length);
        encoded.extend_from_slice(bytes);
        encoded
    }

    fn p0_ethereum_rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
        let payload = items.concat();
        if payload.len() < 56 {
            let mut encoded = vec![0xc0 + payload.len() as u8];
            encoded.extend_from_slice(&payload);
            return encoded;
        }
        let length_bytes = payload.len().to_be_bytes();
        let first = length_bytes
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(length_bytes.len() - 1);
        let length = &length_bytes[first..];
        let mut encoded = vec![0xf7 + length.len() as u8];
        encoded.extend_from_slice(length);
        encoded.extend_from_slice(&payload);
        encoded
    }

    fn p0_ethereum_abi_u64(value: u64) -> [u8; 32] {
        let mut word = [0_u8; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }

    fn p0_ethereum_abi_address(value: [u8; 20]) -> [u8; 32] {
        let mut word = [0_u8; 32];
        word[12..].copy_from_slice(&value);
        word
    }

    fn p0_ethereum_abi_dynamic(value: &[u8]) -> Vec<u8> {
        let mut encoded = p0_ethereum_abi_u64(
            u64::try_from(value.len()).expect("test ABI value length fits u64"),
        )
        .to_vec();
        encoded.extend_from_slice(value);
        let padded_len = encoded.len().div_ceil(32) * 32;
        encoded.resize(padded_len, 0);
        encoded
    }

    fn p0_ethereum_receipt_proof(
        emitter: [u8; 20],
        topics: &[[u8; 32]],
        data: &[u8],
    ) -> ([u8; 32], EthereumReceiptProofV1) {
        let topics = topics
            .iter()
            .map(|topic| p0_ethereum_rlp_bytes(topic))
            .collect::<Vec<_>>();
        let log = p0_ethereum_rlp_list(&[
            p0_ethereum_rlp_bytes(&emitter),
            p0_ethereum_rlp_list(&topics),
            p0_ethereum_rlp_bytes(data),
        ]);
        let receipt = p0_ethereum_rlp_list(&[
            p0_ethereum_rlp_bytes(&[1]),
            p0_ethereum_rlp_bytes(&[1]),
            p0_ethereum_rlp_bytes(&[0; 256]),
            p0_ethereum_rlp_list(&[log]),
        ]);
        let leaf = p0_ethereum_rlp_list(&[
            p0_ethereum_rlp_bytes(&[0x20, 0x80]),
            p0_ethereum_rlp_bytes(&receipt),
        ]);
        let root = postfiat_bridge::ethereum_keccak256(&leaf);
        (
            root,
            EthereumReceiptProofV1 {
                transaction_index: 0,
                receipt_rlp: receipt,
                proof_nodes_rlp: vec![leaf],
            },
        )
    }

    fn p0_ethereum_checkpoint_certificate(
        committee: &FastSwapCommitteeV1,
        authority_keys: &[MlDsa65KeyPair],
        checkpoint: EthereumFinalizedCheckpointV1,
    ) -> EthereumCheckpointCertificateV1 {
        let votes = authority_keys
            .iter()
            .enumerate()
            .take(usize::from(committee.domain.quorum))
            .map(|(index, key)| {
                let mut vote = EthereumCheckpointVoteV1 {
                    validator_id: committee.validators[index].validator_id.clone(),
                    signature: vec![1],
                };
                vote.signature = ml_dsa_65_sign_with_context_seed(
                    &key.private_key,
                    &vote.signing_bytes(&checkpoint).expect("checkpoint vote bytes"),
                    ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
                    &[0xb0 + index as u8; 32],
                )
                .expect("checkpoint vote signature");
                vote
            })
            .collect();
        EthereumCheckpointCertificateV1 { checkpoint, votes }
    }

    fn vault_bridge_evidence(amount_atoms: u64, deposit_byte: &str) -> VaultBridgeDepositEvidence {
        let pftl_recipient = "bridge-recipient-000000000000000000000000".to_string();
        let pftl_recipient_hash =
            vault_bridge_pftl_recipient_hash(&pftl_recipient).expect("recipient hash");
        let mut evidence = VaultBridgeDepositEvidence {
            source_chain_id: 42_161,
            vault_address: "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0".to_string(),
            token_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            depositor: "0x1111111111111111111111111111111111111111".to_string(),
            pftl_recipient,
            pftl_recipient_hash,
            amount_atoms,
            nonce: "22".repeat(32),
            route_binding: String::new(),
            deposit_id: deposit_byte.repeat(32),
            block_hash: "44".repeat(32),
            tx_hash: "55".repeat(32),
            log_index: 7,
        };
        evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
        evidence
    }

    #[test]
    fn nav_subscription_settlement_converts_usd_1e8_nav_to_usdc_atoms() {
        assert_eq!(
            required_vault_bridge_settlement_atoms(1, 0, 508_236_346, "usd_1e8", "USDC", 6)
                .expect("converted settlement"),
            5_082_364
        );
        assert_eq!(
            required_vault_bridge_settlement_atoms(5, 0, 508_236_346, "usd_1e8", "USDC", 6)
                .expect("converted settlement"),
            25_411_818
        );
        assert_eq!(
            required_vault_bridge_settlement_atoms(5, 0, VAULT_BRIDGE_UNIT, "NAV_UNIT", "SOURCE_UNIT", 6)
                .expect("legacy settlement"),
            5_000_000
        );
        assert_eq!(
            required_vault_bridge_settlement_atoms(
                500_000,
                6,
                508_236_346,
                "usd_1e8",
                "USDC",
                6
            )
            .expect("fractional precision-scaled settlement"),
            2_541_182
        );
    }

    #[test]
    fn nav_subscription_can_consume_issued_vault_bridge_supply_for_primary_mint() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let settlement_issuer_key = ml_dsa_65_keygen().expect("settlement issuer keygen");
        let subscriber_key = ml_dsa_65_keygen().expect("subscriber keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let settlement_issuer = address_from_public_key(&settlement_issuer_key.public_key);
        let subscriber = address_from_public_key(&subscriber_key.public_key);
        let bridge_evidence = vault_bridge_evidence(10_000_000, "a1");
        let source_domain = bridge_evidence.source_domain();
        let policy_hash = "42".repeat(48);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                10_000,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                settlement_issuer.clone(),
                10_000,
                Some(bytes_to_hex(&settlement_issuer_key.public_key)),
            ),
            Account::new(
                subscriber.clone(),
                10_000,
                Some(bytes_to_hex(&subscriber_key.public_key)),
            ),
        ]);

        let profile_register = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &settlement_issuer_key,
            NAV_PROFILE_REGISTER_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
                registrant: settlement_issuer.clone(),
                verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
                source_class: format!("vault_bridge:{source_domain}"),
                max_snapshot_age_blocks: 100,
                challenge_window_blocks: 0,
                max_epoch_gap_blocks: 100,
                settle_deadline_blocks: 0,
                min_challenge_bond: 0,
                min_attestations: 1,
                tolerance_bp: 0,
                bridge_observer_min_confirmations: 6,
                valuation_policy_hash: policy_hash.clone(),
                vault_bridge_route_policy_hash: String::new(),
                sp1_program_vkey: String::new(),
                sp1_proof_encoding: String::new(),
                max_proof_bytes: 0,
                max_public_values_bytes: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &profile_register, 1).accepted);
        let vault_bridge_profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

        let create_pfusdc = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &settlement_issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: settlement_issuer.clone(),
                code: "PFUSDC".to_string(),
                version: 1,
                precision: 6,
                display_name: "pfUSDC".to_string(),
                max_supply: Some(100_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &create_pfusdc, 2).accepted);
        let pfusdc_asset_id = ledger.asset_definitions[0].asset_id.clone();

        let register_pfusdc = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &settlement_issuer_key,
            NAV_ASSET_REGISTER_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
                issuer: settlement_issuer.clone(),
                asset_id: pfusdc_asset_id.clone(),
                reserve_operator: settlement_issuer.clone(),
                proof_profile: vault_bridge_profile_id,
                valuation_unit: "SOURCE_UNIT".to_string(),
                redemption_account: settlement_issuer.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_pfusdc, 3).accepted);

        let trust_pfusdc = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &subscriber_key,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: subscriber.clone(),
                issuer: settlement_issuer.clone(),
                asset_id: pfusdc_asset_id.clone(),
                limit: 100_000_000,
                authorized: false,
                frozen: false,
                reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &trust_pfusdc, 4).accepted);

        let create_nav = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "a651".to_string(),
                version: 1,
                precision: 6,
                display_name: "a651 NAVCoin".to_string(),
                max_supply: Some(100_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &create_nav, 5).accepted);
        let nav_asset_id = ledger.asset_definitions[1].asset_id.clone();

        let register_nav = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_ASSET_REGISTER_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
                issuer: issuer.clone(),
                asset_id: nav_asset_id.clone(),
                reserve_operator: issuer.clone(),
                proof_profile: "nav-subscription-v0".to_string(),
                valuation_unit: "NAV_UNIT".to_string(),
                redemption_account: issuer.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_nav, 6).accepted);

        let reserve_packet_hash = "ab".repeat(48);
        let reserve_submit = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
                issuer: issuer.clone(),
                submitter: issuer.clone(),
                asset_id: nav_asset_id.clone(),
                epoch: 1,
                nav_per_unit: VAULT_BRIDGE_UNIT,
                circulating_supply: 100_000_000,
                verified_net_assets: 100_000_000,
                proof_profile: "nav-subscription-v0".to_string(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: reserve_packet_hash.clone(),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &reserve_submit, 8).accepted);
        let reserve_finalize = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
            4,
            AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
                issuer: issuer.clone(),
                asset_id: nav_asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &reserve_finalize, 9).accepted);

        let mut receipt = VaultBridgeReceipt::new(
            &genesis.chain_id,
            pfusdc_asset_id.clone(),
            source_domain.to_string(),
            bridge_evidence.source_asset_ref(),
            VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT.to_string(),
            10_000_000,
            bridge_evidence.source_tx_or_attestation(),
            bridge_evidence.finality_ref(),
            bridge_evidence.vault_id(),
            policy_hash,
            10,
            1_000,
            Some(bridge_evidence),
        )
        .expect("receipt");
        receipt.status = VAULT_BRIDGE_RECEIPT_STATUS_COUNTED.to_string();
        receipt.haircut_bps = 0;
        receipt.counted_value_atoms = 10_000_000;
        receipt.allocated_value_atoms = 10_000_000;
        receipt.finalized_at_height = 10;
        receipt.counted_at_height = 10;
        receipt.validate_for_chain(&genesis.chain_id).expect("valid receipt");
        let receipt_id = receipt.receipt_id.clone();

        let mut bucket = VaultBridgeBucketState::new(
            pfusdc_asset_id.clone(),
            receipt.source_domain.clone(),
            receipt.policy_hash.clone(),
            10,
        )
        .expect("bucket");
        bucket.gross_receipt_atoms = 10_000_000;
        bucket.counted_value_atoms = 10_000_000;
        bucket.outstanding_vault_bridge_atoms = 10_000_000;
        bucket.validate().expect("valid bucket");
        let bucket_id = bucket.bucket_id.clone();

        let supply_allocation = VaultBridgeAllocation::new(
            &genesis.chain_id,
            receipt_id.clone(),
            pfusdc_asset_id.clone(),
            bucket_id.clone(),
            10_000_000,
            VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,
            "vault_bridge_supply:test".to_string(),
            10,
        )
        .expect("supply allocation");
        let supply_allocation_id = supply_allocation.allocation_id.clone();

        ledger.vault_bridge_receipts.push(receipt);
        ledger.vault_bridge_bucket_states.push(bucket);
        ledger.vault_bridge_allocations.push(supply_allocation);
        let pfusdc_line_index = trustline_index(&ledger, &subscriber, &pfusdc_asset_id)
            .expect("subscriber pfUSDC trustline");
        ledger.trustlines[pfusdc_line_index].balance = 10_000_000;

        let wrong_signer_consume_supply = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &settlement_issuer_key,
            VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND,
            4,
            AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(
                VaultBridgeNavSubscriptionAllocateOperation {
                    operator: issuer.clone(),
                    nav_asset_id: nav_asset_id.clone(),
                    settlement_asset_id: pfusdc_asset_id.clone(),
                    settlement_bucket_id: bucket_id.clone(),
                    settlement_receipt_id: receipt_id.clone(),
                    settlement_amount_atoms: 5_000_000,
                    consume_supply_owner: Some(subscriber.clone()),
                    consume_supply_allocation_id: Some(supply_allocation_id.clone()),
                    nav_recipient: Some(subscriber.clone()),
                    subscription_id: Some("navsub-wrong-signer".to_string()),
                },
            ),
        );
        let mut wrong_signer_ledger = ledger.clone();
        let wrong_signer_receipt = execute_asset_transaction(
            &genesis,
            &mut wrong_signer_ledger,
            &wrong_signer_consume_supply,
            11,
        );
        assert!(!wrong_signer_receipt.accepted);
        assert_eq!(
            wrong_signer_receipt.code, "bad_asset_transaction_envelope",
            "non-owner signers must be rejected before consuming owned settlement"
        );

        let legacy_operator_signed_consume_supply = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(
                VaultBridgeNavSubscriptionAllocateOperation {
                    operator: issuer.clone(),
                    nav_asset_id: nav_asset_id.clone(),
                    settlement_asset_id: pfusdc_asset_id.clone(),
                    settlement_bucket_id: bucket_id.clone(),
                    settlement_receipt_id: receipt_id.clone(),
                    settlement_amount_atoms: 5_000_000,
                    consume_supply_owner: Some(subscriber.clone()),
                    consume_supply_allocation_id: Some(supply_allocation_id.clone()),
                    nav_recipient: Some(subscriber.clone()),
                    subscription_id: Some("navsub-legacy-operator".to_string()),
                },
            ),
        );
        let bridge_verification_at_11 =
            AssetExecutionCompatibility::strict().with_bridge_verification_activation_height(Some(11));
        let mut pre_activation_ledger = ledger.clone();
        let pre_activation_receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut pre_activation_ledger,
            &legacy_operator_signed_consume_supply,
            10,
            bridge_verification_at_11,
        );
        assert!(
            pre_activation_receipt.accepted,
            "legacy operator-signed consume-supply should replay before activation: {pre_activation_receipt:?}"
        );
        let mut activation_ledger = ledger.clone();
        let activation_receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut activation_ledger,
            &legacy_operator_signed_consume_supply,
            11,
            bridge_verification_at_11,
        );
        assert!(!activation_receipt.accepted);
        assert_eq!(activation_receipt.code, "bad_asset_transaction_envelope");

        let consume_supply = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &subscriber_key,
            VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(
                VaultBridgeNavSubscriptionAllocateOperation {
                    operator: issuer.clone(),
                    nav_asset_id: nav_asset_id.clone(),
                    settlement_asset_id: pfusdc_asset_id.clone(),
                    settlement_bucket_id: bucket_id.clone(),
                    settlement_receipt_id: receipt_id.clone(),
                    settlement_amount_atoms: 5_000_000,
                    consume_supply_owner: Some(subscriber.clone()),
                    consume_supply_allocation_id: Some(supply_allocation_id.clone()),
                    nav_recipient: Some(subscriber.clone()),
                    subscription_id: Some("navsub-0001".to_string()),
                },
            ),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &consume_supply, 11).accepted);
        let consume_supply_again = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &subscriber_key,
            VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(
                VaultBridgeNavSubscriptionAllocateOperation {
                    operator: issuer.clone(),
                    nav_asset_id: nav_asset_id.clone(),
                    settlement_asset_id: pfusdc_asset_id.clone(),
                    settlement_bucket_id: bucket_id.clone(),
                    settlement_receipt_id: receipt_id.clone(),
                    settlement_amount_atoms: 5_000_000,
                    consume_supply_owner: Some(subscriber.clone()),
                    consume_supply_allocation_id: Some(supply_allocation_id.clone()),
                    nav_recipient: Some(subscriber.clone()),
                    subscription_id: Some("navsub-0002".to_string()),
                },
            ),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut ledger, &consume_supply_again, 12)
                .accepted
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&subscriber, &pfusdc_asset_id)
                .expect("subscriber pfUSDC")
                .balance,
            0
        );
        let bucket_after = ledger.vault_bridge_bucket(&bucket_id).expect("bucket after");
        assert_eq!(bucket_after.outstanding_vault_bridge_atoms, 0);
        assert_eq!(bucket_after.nav_subscription_allocations_atoms, 10_000_000);
        assert_eq!(ledger.vault_bridge_receipts[0].allocated_value_atoms, 10_000_000);
        assert_eq!(ledger.vault_bridge_allocations[0].released_atoms, 10_000_000);
        assert_eq!(ledger.vault_bridge_allocations[0].retired_at_height, 12);
        assert_eq!(ledger.vault_bridge_allocations.len(), 3);
        let nav_allocation_id = ledger.vault_bridge_allocations[1].allocation_id.clone();
        let second_nav_allocation_id = ledger.vault_bridge_allocations[2].allocation_id.clone();
        assert_ne!(nav_allocation_id, second_nav_allocation_id);
        assert_eq!(
            ledger.vault_bridge_allocations[1].consumer_id,
            nav_subscription_recipient_order_consumer_id(&nav_asset_id, &subscriber, "navsub-0001")
        );
        assert_eq!(
            ledger.vault_bridge_allocations[2].consumer_id,
            nav_subscription_recipient_order_consumer_id(&nav_asset_id, &subscriber, "navsub-0002")
        );
        assert_eq!(ledger.vault_bridge_allocations[1].retired_at_height, 0);
        assert_eq!(ledger.vault_bridge_allocations[2].retired_at_height, 0);
        assert!(
            ledger
                .trustline_for_account_asset(&subscriber, &nav_asset_id)
                .is_none(),
            "NAV recipient should not need a pre-opened output balance row"
        );

        let mint = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_MINT_AT_NAV_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
                issuer: issuer.clone(),
                to: subscriber.clone(),
                asset_id: nav_asset_id.clone(),
                amount: 5_000_000,
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
                settlement_asset_id: pfusdc_asset_id.clone(),
                settlement_bucket_id: bucket_id.clone(),
                settlement_allocation_id: nav_allocation_id,
                settlement_amount_atoms: 5_000_000,
            }),
        );
        let mint_receipt = execute_asset_transaction(&genesis, &mut ledger, &mint, 13);
        assert!(
            mint_receipt.accepted,
            "{}: {}",
            mint_receipt.code, mint_receipt.message
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&subscriber, &nav_asset_id)
                .expect("subscriber NAV")
                .balance,
            5_000_000
        );
        let subscriber_nav_line = ledger
            .trustline_for_account_asset(&subscriber, &nav_asset_id)
            .expect("subscriber implicit NAV balance row");
        assert_eq!(subscriber_nav_line.limit, 5_000_000);
        assert_eq!(subscriber_nav_line.reserve_paid, 0);
        assert_eq!(ledger.vault_bridge_allocations[1].retired_at_height, 13);
        assert_eq!(ledger.vault_bridge_allocations[2].retired_at_height, 0);
        let overlay = nav_subscription_reserve_overlay(
            &ledger,
            ledger.nav_asset(&nav_asset_id).expect("tracked nav"),
        )
        .expect("overlay")
        .expect("overlay present");
        assert_eq!(overlay.value_nav_units, 5_000_000);
    }

    #[test]
    fn vault_bridge_redeem_settle_reduces_queue_and_counted_value() {
        let issuer = "issuer".to_string();
        let asset = AssetDefinition::new("postfiat-local", &issuer, "pfUSDC", 1, 6)
            .expect("vault bridge asset");
        let asset_id = asset.asset_id.clone();
        let policy_hash = "b2".repeat(48);
        let source_domain = "erc20_bridge_vault:42161:0x1111111111111111111111111111111111111111:0x2222222222222222222222222222222222222222";
        let profile = postfiat_types::NavProofProfile::new(
            issuer.clone(),
            NAV_PROFILE_VERIFIER_MULTI_FETCH,
            format!("vault_bridge:{source_domain}"),
            100,
            0,
            100,
            0,
            0,
            1,
            0,
            policy_hash.clone(),
            "",
            "",
            0,
            0,
        )
        .expect("vault bridge profile");
        let profile_id = profile.profile_id.clone();
        let mut ledger = LedgerState::new(Vec::new());
        ledger.asset_definitions.push(asset);
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_assets.push(
            NavTrackedAsset::new(
                asset_id.clone(),
                issuer.clone(),
                issuer.clone(),
                profile_id,
                "USDC",
                issuer.clone(),
            )
            .expect("tracked vault bridge asset"),
        );

        let mut bucket = VaultBridgeBucketState::new(
            asset_id.clone(),
            source_domain,
            policy_hash,
            10,
        )
        .expect("vault bridge bucket");
        bucket.gross_receipt_atoms = 10_000_000;
        bucket.counted_value_atoms = 10_000_000;
        bucket.outstanding_vault_bridge_atoms = 4_000_000;
        bucket.redemption_queue_atoms = 1_000_000;
        bucket.validate().expect("valid pre-settlement bucket");
        let bucket_id = bucket.bucket_id.clone();
        ledger.vault_bridge_bucket_states.push(bucket);

        let redemption = VaultBridgeRedemption::new(
            "postfiat-local",
            "holder",
            issuer.clone(),
            asset_id.clone(),
            bucket_id.clone(),
            source_domain,
            7,
            1_000_000,
            1,
            "c3".repeat(48),
            "evm-erc20:42161:0x3333333333333333333333333333333333333333",
            "d4".repeat(48),
            11,
        )
        .expect("pending vault bridge redemption");
        let redemption_id = redemption.redemption_id.clone();
        ledger.vault_bridge_redemptions.push(redemption);

        let bucket_before = ledger.vault_bridge_bucket(&bucket_id).expect("bucket before");
        let unallocated_before = bucket_before.counted_value_atoms
            - bucket_before.allocated_atoms().expect("allocated before");

        apply_vault_bridge_redeem_settle(
            &mut ledger,
            &VaultBridgeRedeemSettleOperation {
                issuer_or_redemption_account: issuer,
                asset_id,
                redemption_id,
                settlement_receipt_hash: "e5".repeat(48),
                settled_atoms: 1_000_000,
                withdrawal_observations: Vec::new(),
            },
            12,
        )
        .expect("settlement applies");

        let bucket_after = ledger.vault_bridge_bucket(&bucket_id).expect("bucket after");
        let unallocated_after = bucket_after.counted_value_atoms
            - bucket_after.allocated_atoms().expect("allocated after");
        assert_eq!(bucket_after.redemption_queue_atoms, 0);
        assert_eq!(bucket_after.counted_value_atoms, 9_000_000);
        assert_eq!(bucket_after.outstanding_vault_bridge_atoms, 4_000_000);
        assert_eq!(unallocated_after, unallocated_before);
        assert_eq!(ledger.vault_bridge_redemptions[0].settled_atoms, 1_000_000);
        assert_eq!(ledger.vault_bridge_redemptions[0].state, VAULT_BRIDGE_REDEMPTION_STATE_SETTLED);
    }

    fn vault_bridge_stage2_redemption_fixture() -> (
        LedgerState,
        MlDsa65KeyPair,
        String,
        String,
        String,
    ) {
        let issuer = "issuer".to_string();
        let observer_key = ml_dsa_65_keygen().expect("observer keygen");
        let observer = address_from_public_key(&observer_key.public_key);
        let asset = AssetDefinition::new("postfiat-local", &issuer, "pfUSDC", 1, 6)
            .expect("stage2 vault bridge asset");
        let asset_id = asset.asset_id.clone();
        let policy_hash = "b3".repeat(48);
        let source_domain = "erc20_bridge_vault:42161:0x1111111111111111111111111111111111111111:0x2222222222222222222222222222222222222222";
        let profile = postfiat_types::NavProofProfile::new_with_bridge_observer_min_confirmations(
            issuer.clone(),
            NAV_PROFILE_VERIFIER_MULTI_FETCH,
            format!("vault_bridge:{source_domain}"),
            100,
            0,
            100,
            0,
            0,
            1,
            0,
            6,
            policy_hash.clone(),
            "",
            "",
            0,
            0,
        )
        .expect("stage2 vault bridge profile");
        let profile_id = profile.profile_id.clone();
        let mut ledger = LedgerState::new(vec![Account::new(
            observer.clone(),
            10_000,
            Some(bytes_to_hex(&observer_key.public_key)),
        )]);
        ledger.asset_definitions.push(asset);
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_attestors.push(NavAttestor {
            address: observer.clone(),
            domain: "stage2.local".to_string(),
            bond: 0,
            registered_at_height: 1,
        });
        ledger.nav_assets.push(
            NavTrackedAsset::new(
                asset_id.clone(),
                issuer.clone(),
                issuer.clone(),
                profile_id,
                "USDC",
                issuer.clone(),
            )
            .expect("tracked vault bridge asset"),
        );
        let mut bucket = VaultBridgeBucketState::new(
            asset_id.clone(),
            source_domain,
            policy_hash,
            10,
        )
        .expect("vault bridge bucket");
        bucket.gross_receipt_atoms = 10_000_000;
        bucket.counted_value_atoms = 10_000_000;
        bucket.outstanding_vault_bridge_atoms = 4_000_000;
        bucket.redemption_queue_atoms = 1_000_000;
        bucket.validate().expect("valid pre-settlement bucket");
        let bucket_id = bucket.bucket_id.clone();
        ledger.vault_bridge_bucket_states.push(bucket);
        let redemption = VaultBridgeRedemption::new(
            "postfiat-local",
            "holder",
            issuer.clone(),
            asset_id.clone(),
            bucket_id.clone(),
            source_domain,
            7,
            1_000_000,
            1,
            "c4".repeat(48),
            "evm-erc20:42161:0x3333333333333333333333333333333333333333",
            "d5".repeat(48),
            11,
        )
        .expect("pending vault bridge redemption");
        let redemption_id = redemption.redemption_id.clone();
        ledger.vault_bridge_redemptions.push(redemption);
        (ledger, observer_key, issuer, asset_id, redemption_id)
    }

    fn withdrawal_attestation_for_redemption(
        redemption: &VaultBridgeRedemption,
        observer_key: &MlDsa65KeyPair,
    ) -> (String, VaultBridgeWithdrawalExecutionAttestation) {
        let observation = VaultBridgeWithdrawalExecutionObservation::success_for_packet(
            &redemption.withdrawal_packet,
            redemption.withdrawal_packet_hash.clone(),
            "a7".repeat(32),
            "b8".repeat(32),
            4,
            6,
        );
        let observation_root =
            vault_bridge_withdrawal_execution_observation_root(&observation)
                .expect("withdrawal observation root");
        let signature = ml_dsa_65_sign(&observer_key.private_key, &observation.signing_bytes())
            .expect("sign withdrawal observation");
        let attestation = VaultBridgeWithdrawalExecutionAttestation {
            attestor: address_from_public_key(&observer_key.public_key),
            observation_root: observation_root.clone(),
            signature_hex: bytes_to_hex(&signature),
            observation,
        };
        (observation_root, attestation)
    }

    #[test]
    fn vault_bridge_stage2_redeem_settle_requires_observed_withdrawal_execution() {
        let (mut ledger, observer_key, issuer, asset_id, redemption_id) =
            vault_bridge_stage2_redemption_fixture();
        let redemption = ledger.vault_bridge_redemptions[0].clone();
        let (settlement_receipt_hash, attestation) =
            withdrawal_attestation_for_redemption(&redemption, &observer_key);

        apply_vault_bridge_redeem_settle(
            &mut ledger,
            &VaultBridgeRedeemSettleOperation {
                issuer_or_redemption_account: issuer,
                asset_id,
                redemption_id,
                settlement_receipt_hash,
                settled_atoms: 1_000_000,
                withdrawal_observations: vec![attestation],
            },
            12,
        )
        .expect("stage2 observed settlement applies");

        assert_eq!(ledger.vault_bridge_bucket_states[0].redemption_queue_atoms, 0);
        assert_eq!(ledger.vault_bridge_bucket_states[0].counted_value_atoms, 9_000_000);
        assert_eq!(ledger.vault_bridge_redemptions[0].withdrawal_observations.len(), 1);
        assert_eq!(ledger.vault_bridge_redemptions[0].state, VAULT_BRIDGE_REDEMPTION_STATE_SETTLED);
    }

    #[test]
    fn rotated_route_uses_pinned_profile_for_in_flight_redemption_settlement() {
        let (mut ledger, observer_key, issuer, asset_id, redemption_id) =
            vault_bridge_stage2_redemption_fixture();
        let old_profile_id = ledger.nav_assets[0].proof_profile.clone();
        let old_profile = ledger
            .nav_proof_profile(&old_profile_id)
            .expect("old pinned profile")
            .clone();
        let current_profile = postfiat_types::NavProofProfile::new_with_bridge_observer_min_confirmations(
            issuer.clone(),
            NAV_PROFILE_VERIFIER_MULTI_FETCH,
            old_profile.source_class.clone(),
            100,
            0,
            100,
            0,
            0,
            1,
            0,
            1,
            "c5".repeat(48),
            "",
            "",
            0,
            0,
        )
        .expect("rotated current profile");
        ledger.nav_assets[0].proof_profile = current_profile.profile_id.clone();
        ledger.nav_proof_profiles.push(current_profile.clone());

        let redemption = ledger.vault_bridge_redemptions[0].clone();
        let (settlement_receipt_hash, attestation) =
            withdrawal_attestation_for_redemption(&redemption, &observer_key);
        let operation = VaultBridgeRedeemSettleOperation {
            issuer_or_redemption_account: issuer,
            asset_id,
            redemption_id,
            settlement_receipt_hash,
            settled_atoms: 1_000_000,
            withdrawal_observations: vec![attestation],
        };

        let mut missing_historical = ledger.clone();
        missing_historical.nav_proof_profiles = vec![current_profile];
        let before = missing_historical.clone();
        let error = apply_vault_bridge_redeem_settle(&mut missing_historical, &operation, 12)
            .expect_err("missing pinned redemption profile must fail closed");
        assert_eq!("missing_vault_bridge_pinned_profile", error.0);
        assert_eq!(before, missing_historical, "failed lookup mutated state");

        apply_vault_bridge_redeem_settle(&mut ledger, &operation, 12)
            .expect("old pinned redemption profile must remain usable after rotation");
        assert_eq!(0, ledger.vault_bridge_bucket_states[0].redemption_queue_atoms);
        assert_eq!(
            VAULT_BRIDGE_REDEMPTION_STATE_SETTLED,
            ledger.vault_bridge_redemptions[0].state
        );
    }

    #[test]
    fn vault_bridge_stage2_redeem_settle_without_observation_rejected() {
        let (mut ledger, _observer_key, issuer, asset_id, redemption_id) =
            vault_bridge_stage2_redemption_fixture();

        let err = apply_vault_bridge_redeem_settle(
            &mut ledger,
            &VaultBridgeRedeemSettleOperation {
                issuer_or_redemption_account: issuer,
                asset_id,
                redemption_id,
                settlement_receipt_hash: "e6".repeat(48),
                settled_atoms: 1_000_000,
                withdrawal_observations: Vec::new(),
            },
            12,
        )
        .expect_err("missing withdrawal observation");
        assert_eq!(err.0, "vault_bridge_withdrawal_observation_quorum_not_met");
    }

    #[test]
    fn bridge_verification_activation_gates_withdrawal_execution_observation() {
        let (pre_activation_ledger, _observer_key, issuer, asset_id, redemption_id) =
            vault_bridge_stage2_redemption_fixture();
        let settle_without_observation = VaultBridgeRedeemSettleOperation {
            issuer_or_redemption_account: issuer.clone(),
            asset_id: asset_id.clone(),
            redemption_id: redemption_id.clone(),
            settlement_receipt_hash: "f7".repeat(48),
            settled_atoms: 1_000_000,
            withdrawal_observations: Vec::new(),
        };
        let bridge_verification_at_12 =
            AssetExecutionCompatibility::strict().with_bridge_verification_activation_height(Some(12));
        let mut pre_activation_ledger = pre_activation_ledger;
        apply_vault_bridge_redeem_settle_with_compatibility(
            &mut pre_activation_ledger,
            &settle_without_observation,
            11,
            bridge_verification_at_12,
        )
        .expect("pre-activation withdrawal settlement without observation replays");

        let (mut activation_ledger, _observer_key, issuer, asset_id, redemption_id) =
            vault_bridge_stage2_redemption_fixture();
        let settle_without_observation = VaultBridgeRedeemSettleOperation {
            issuer_or_redemption_account: issuer,
            asset_id,
            redemption_id,
            settlement_receipt_hash: "f8".repeat(48),
            settled_atoms: 1_000_000,
            withdrawal_observations: Vec::new(),
        };
        let err = apply_vault_bridge_redeem_settle_with_compatibility(
            &mut activation_ledger,
            &settle_without_observation,
            12,
            bridge_verification_at_12,
        )
        .expect_err("activation requires observed withdrawal execution");
        assert_eq!(err.0, "vault_bridge_withdrawal_observation_quorum_not_met");
    }

    #[test]
    fn genesis_hash_has_stable_canonical_test_vector() {
        let genesis = Genesis::new("postfiat-local");

        assert_eq!(
            genesis_hash(&genesis),
            "16843cd2ee2a22a98063f2269d1a06279d33598d07331e32cbf838ee9798c9219c61bc0f6509871ec74c5760277d889f"
        );

        let mut prior_v2_genesis = genesis;
        prior_v2_genesis.native_supply_atoms = None;
        assert_eq!(
            genesis_hash(&prior_v2_genesis),
            "f340b4b169edcd2b3706a46be4bc5c112b7ea28c6254ff63e265ba0546253ab03e86128e24f994c617f6b1456026b83f"
        );

        let mut legacy_genesis = prior_v2_genesis;
        legacy_genesis.replicated_state_v2_activation_height = None;
        assert_eq!(
            genesis_hash(&legacy_genesis),
            "97982d730c6adadfa21b7662bfe12d8ca69b4192bba0f4905e4090acc441d572fd17a81f0c23ff7bc8ccd7c4091aa04a"
        );
    }

    fn signed_transfer(
        genesis: &Genesis,
        key_pair: &MlDsa65KeyPair,
        to: String,
        amount: u64,
        fee: u64,
        sequence: u64,
    ) -> SignedTransfer {
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let unsigned = UnsignedTransfer {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            from: address_from_public_key(&key_pair.public_key),
            to,
            amount,
            fee,
            sequence,
        };
        let signature = ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes())
            .expect("sign transfer");
        SignedTransfer {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex,
            signature_hex: bytes_to_hex(&signature),
        }
    }

    fn signed_transfer_with_minimum_fee(
        genesis: &Genesis,
        key_pair: &MlDsa65KeyPair,
        to: String,
        amount: u64,
        sequence: u64,
    ) -> SignedTransfer {
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let transfer = signed_transfer(genesis, key_pair, to.clone(), amount, fee, sequence);
            let state_expansion_fee = if transfer.unsigned.to != transfer.unsigned.from {
                TRANSFER_ACCOUNT_CREATION_FEE
            } else {
                0
            };
            let minimum_fee = minimum_transfer_fee(&transfer).saturating_add(state_expansion_fee);
            if fee >= minimum_fee {
                return transfer;
            }
            fee = minimum_fee;
        }
        panic!("minimum transfer fee did not converge");
    }

    fn signed_payment_v2(
        genesis: &Genesis,
        key_pair: &MlDsa65KeyPair,
        to: String,
        amount: u64,
        fee: u64,
        sequence: u64,
        memos: Vec<PaymentMemo>,
    ) -> SignedPaymentV2 {
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let unsigned = UnsignedPaymentV2 {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            from: address_from_public_key(&key_pair.public_key),
            to,
            amount,
            fee,
            sequence,
            memos,
        };
        let signature = ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes())
            .expect("sign payment_v2");
        SignedPaymentV2 {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex,
            signature_hex: bytes_to_hex(&signature),
        }
    }

    fn signed_asset_transaction(
        genesis: &Genesis,
        key_pair: &MlDsa65KeyPair,
        transaction_kind: &str,
        fee: u64,
        sequence: u64,
        operation: AssetTransactionOperation,
    ) -> SignedAssetTransaction {
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let unsigned = UnsignedAssetTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: transaction_kind.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            source: address_from_public_key(&key_pair.public_key),
            fee,
            sequence,
            operation,
        };
        let signature = ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes())
            .expect("sign asset transaction");
        SignedAssetTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex,
            signature_hex: bytes_to_hex(&signature),
        }
    }

    fn signed_asset_transaction_with_minimum_fee(
        genesis: &Genesis,
        ledger: &LedgerState,
        key_pair: &MlDsa65KeyPair,
        transaction_kind: &str,
        sequence: u64,
        operation: AssetTransactionOperation,
    ) -> SignedAssetTransaction {
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let transaction = signed_asset_transaction(
                genesis,
                key_pair,
                transaction_kind,
                fee,
                sequence,
                operation.clone(),
            );
            let minimum_fee = minimum_asset_transaction_fee_for_ledger(ledger, &transaction);
            if fee >= minimum_fee {
                return transaction;
            }
            fee = minimum_fee;
        }
        panic!("minimum asset transaction fee did not converge");
    }

    fn signed_escrow_transaction(
        genesis: &Genesis,
        key_pair: &MlDsa65KeyPair,
        transaction_kind: &str,
        fee: u64,
        sequence: u64,
        operation: EscrowTransactionOperation,
    ) -> SignedEscrowTransaction {
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let unsigned = UnsignedEscrowTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: transaction_kind.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            source: address_from_public_key(&key_pair.public_key),
            fee,
            sequence,
            operation,
        };
        let signature = ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes())
            .expect("sign escrow transaction");
        SignedEscrowTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex,
            signature_hex: bytes_to_hex(&signature),
        }
    }

    fn signed_escrow_transaction_with_minimum_fee(
        genesis: &Genesis,
        ledger: &LedgerState,
        key_pair: &MlDsa65KeyPair,
        transaction_kind: &str,
        sequence: u64,
        operation: EscrowTransactionOperation,
    ) -> SignedEscrowTransaction {
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let transaction = signed_escrow_transaction(
                genesis,
                key_pair,
                transaction_kind,
                fee,
                sequence,
                operation.clone(),
            );
            let minimum_fee = minimum_escrow_transaction_fee_for_ledger(ledger, &transaction);
            if fee >= minimum_fee {
                return transaction;
            }
            fee = minimum_fee;
        }
        panic!("minimum escrow transaction fee did not converge");
    }

    fn signed_nft_transaction(
        genesis: &Genesis,
        key_pair: &MlDsa65KeyPair,
        transaction_kind: &str,
        fee: u64,
        sequence: u64,
        operation: NftTransactionOperation,
    ) -> SignedNftTransaction {
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let unsigned = UnsignedNftTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: transaction_kind.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            source: address_from_public_key(&key_pair.public_key),
            fee,
            sequence,
            operation,
        };
        let signature = ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes())
            .expect("sign nft transaction");
        SignedNftTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex,
            signature_hex: bytes_to_hex(&signature),
        }
    }

    fn signed_nft_transaction_with_minimum_fee(
        genesis: &Genesis,
        ledger: &LedgerState,
        key_pair: &MlDsa65KeyPair,
        transaction_kind: &str,
        sequence: u64,
        operation: NftTransactionOperation,
    ) -> SignedNftTransaction {
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let transaction = signed_nft_transaction(
                genesis,
                key_pair,
                transaction_kind,
                fee,
                sequence,
                operation.clone(),
            );
            let minimum_fee = minimum_nft_transaction_fee_for_ledger(ledger, &transaction);
            if fee >= minimum_fee {
                return transaction;
            }
            fee = minimum_fee;
        }
        panic!("minimum nft transaction fee did not converge");
    }

    fn signed_offer_transaction(
        genesis: &Genesis,
        key_pair: &MlDsa65KeyPair,
        transaction_kind: &str,
        fee: u64,
        sequence: u64,
        operation: OfferTransactionOperation,
    ) -> SignedOfferTransaction {
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let unsigned = UnsignedOfferTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: transaction_kind.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            source: address_from_public_key(&key_pair.public_key),
            fee,
            sequence,
            operation,
        };
        let signature = ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes())
            .expect("sign offer transaction");
        SignedOfferTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex,
            signature_hex: bytes_to_hex(&signature),
        }
    }

    fn signed_offer_transaction_with_minimum_fee(
        genesis: &Genesis,
        ledger: &LedgerState,
        key_pair: &MlDsa65KeyPair,
        transaction_kind: &str,
        sequence: u64,
        operation: OfferTransactionOperation,
        block_height: u64,
    ) -> SignedOfferTransaction {
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let transaction = signed_offer_transaction(
                genesis,
                key_pair,
                transaction_kind,
                fee,
                sequence,
                operation.clone(),
            );
            let minimum_fee =
                minimum_offer_transaction_fee_for_ledger(ledger, &transaction, block_height);
            if fee >= minimum_fee {
                let mut dry_run = ledger.clone();
                let receipt =
                    execute_offer_transaction(genesis, &mut dry_run, &transaction, block_height);
                if receipt.code != "fee_too_low" {
                    return transaction;
                }
            }
            fee = minimum_fee.max(fee.saturating_add(1));
        }
        panic!("minimum offer transaction fee did not converge");
    }

    fn nft_mint_operation(
        issuer: String,
        owner: String,
        serial: u64,
        flags: u32,
    ) -> NftTransactionOperation {
        NftTransactionOperation::NftMint(NftMintOperation {
            issuer,
            collection_id: "ART-2026".to_string(),
            serial,
            owner,
            metadata_hash: "abababababababababababababababababababababababababababababababab"
                .to_string(),
            metadata_uri: "ipfs://bafybeigdyrzt".to_string(),
            flags,
            collection_flags: 0,
            issuer_transfer_fee: 0,
        })
    }

    fn signed_payment_v2_with_minimum_fee(
        genesis: &Genesis,
        key_pair: &MlDsa65KeyPair,
        to: String,
        amount: u64,
        sequence: u64,
        memos: Vec<PaymentMemo>,
    ) -> SignedPaymentV2 {
        let mut fee = MIN_TRANSFER_FEE;
        for _ in 0..8 {
            let payment = signed_payment_v2(
                genesis,
                key_pair,
                to.clone(),
                amount,
                fee,
                sequence,
                memos.clone(),
            );
            let state_expansion_fee = if payment.unsigned.to != payment.unsigned.from {
                TRANSFER_ACCOUNT_CREATION_FEE
            } else {
                0
            };
            let minimum_fee = minimum_payment_v2_fee(&payment).saturating_add(state_expansion_fee);
            if fee >= minimum_fee {
                return payment;
            }
            fee = minimum_fee;
        }
        panic!("minimum payment_v2 fee did not converge");
    }

    fn sample_memo() -> PaymentMemo {
        PaymentMemo {
            memo_type: "7061796d656e74".to_string(),
            memo_format: "746578742f706c61696e".to_string(),
            memo_data: "68656c6c6f".to_string(),
        }
    }

    fn assert_issued_asset_invariants(
        genesis: &Genesis,
        ledger: &LedgerState,
        asset_id: &str,
        expected_supply: u64,
        expected_balances: &[(&str, u64)],
    ) {
        ledger
            .validate_asset_state(&genesis.chain_id)
            .expect("valid issued asset state");
        let asset = ledger
            .asset_definition(asset_id)
            .expect("asset definition exists");
        let mut observed_supply = 0_u64;
        for line in ledger
            .trustlines
            .iter()
            .filter(|line| line.asset_id == asset_id)
        {
            assert_eq!(line.issuer, asset.issuer);
            assert!(line.balance <= line.limit);
            assert_eq!(
                line.trustline_id,
                postfiat_types::trustline_id(&line.account, &line.issuer, &line.asset_id)
                    .expect("deterministic trustline id")
            );
            observed_supply = observed_supply
                .checked_add(line.balance)
                .expect("supply total does not overflow");
        }
        for escrow in ledger
            .escrows
            .iter()
            .filter(|escrow| escrow.asset_id == asset_id && escrow.state == ESCROW_STATE_OPEN)
        {
            observed_supply = observed_supply
                .checked_add(escrow.amount)
                .expect("supply total does not overflow");
        }
        assert_eq!(observed_supply, expected_supply);
        if let Some(max_supply) = asset.max_supply {
            assert!(observed_supply <= max_supply);
        }
        for (account, expected_balance) in expected_balances {
            assert_eq!(
                ledger
                    .trustline_for_account_asset(account, asset_id)
                    .expect("expected trustline")
                    .balance,
                *expected_balance
            );
        }
    }

    fn native_pft_account_offer_total(ledger: &LedgerState) -> u64 {
        let account_total = ledger.accounts.iter().fold(0_u64, |total, account| {
            total
                .checked_add(account.balance)
                .expect("account PFT total does not overflow")
        });
        let open_offer_sell_side_total = ledger
            .offers
            .iter()
            .filter(|offer| {
                offer.state == OFFER_STATE_OPEN
                    && offer.taker_gets_asset_id == NATIVE_PFT_ESCROW_ASSET_ID
            })
            .fold(0_u64, |total, offer| {
                total
                    .checked_add(offer.taker_gets_amount_remaining)
                    .expect("open offer PFT total does not overflow")
            });
        let offer_reserve_total = ledger.offers.iter().fold(0_u64, |total, offer| {
            total
                .checked_add(offer.reserve_paid)
                .expect("offer reserve total does not overflow")
        });
        account_total
            .checked_add(open_offer_sell_side_total)
            .and_then(|total| total.checked_add(offer_reserve_total))
            .expect("native PFT conservation total does not overflow")
    }

    fn assert_offer_conservation(
        genesis: &Genesis,
        ledger: &LedgerState,
        asset_id: &str,
        expected_initial_issued_supply: u64,
        expected_native_pft_after_burn: u64,
    ) {
        ledger
            .validate_offer_state(&genesis.chain_id)
            .expect("valid offer state");
        ledger
            .validate_asset_state(&genesis.chain_id)
            .expect("valid issued asset state");
        assert_eq!(
            expected_native_pft_after_burn,
            native_pft_account_offer_total(ledger)
        );
        assert_eq!(
            expected_initial_issued_supply,
            issued_asset_supply(ledger, asset_id).expect("issued asset supply")
        );
    }

    #[test]
    fn transfer_applies_once_with_sequence() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let from = address_from_public_key(&key_pair.public_key);
        let to = "bridge-recipient-000000000000000000000000".to_string();
        let mut ledger = LedgerState::new(vec![Account::new(
            from.clone(),
            100,
            Some(public_key_hex.clone()),
        )]);

        let transfer = signed_transfer_with_minimum_fee(&genesis, &key_pair, to.clone(), 25, 1);

        let receipt = execute_transfer(&genesis, &mut ledger, &transfer);

        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(
            ledger.account(&transfer.unsigned.from).unwrap().balance,
            100 - 25 - transfer.unsigned.fee
        );
        assert_eq!(ledger.account(&transfer.unsigned.from).unwrap().sequence, 1);
        assert_eq!(ledger.account(&to).unwrap().balance, 25);
        assert!(ledger.account(FEE_COLLECTOR_ADDRESS).is_none());
        assert_eq!(receipt.fee_charged, transfer.unsigned.fee);
        assert_eq!(receipt.fee_burned, transfer.unsigned.fee);
        assert!(receipt.minimum_fee > 0);
        assert!(receipt.minimum_fee <= transfer.unsigned.fee);
        assert_eq!(receipt.account_reserve, ACCOUNT_RESERVE);
        assert_eq!(receipt.state_expansion_fee, TRANSFER_ACCOUNT_CREATION_FEE);

        let replay = execute_transfer(&genesis, &mut ledger, &transfer);
        assert!(!replay.accepted);
        assert_eq!(replay.code, "bad_sequence");
    }

    #[test]
    fn transfer_rejects_exhausted_sequence_without_panicking_or_mutating() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let from = address_from_public_key(&key_pair.public_key);
        let mut ledger = LedgerState::new(vec![Account::new(
            from.clone(),
            100,
            Some(public_key_hex),
        )]);
        ledger.account_mut(&from).expect("sender").sequence = u64::MAX;
        let before = ledger.clone();
        let transfer = signed_transfer_with_minimum_fee(&genesis, &key_pair, from, 1, 0);

        let receipt = execute_transfer(&genesis, &mut ledger, &transfer);

        assert!(!receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "sequence_overflow");
        assert_eq!(ledger, before);
    }

    #[test]
    fn payment_v2_applies_once_with_memo_and_sequence() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let from = address_from_public_key(&key_pair.public_key);
        let to = "bridge-recipient-000000000000000000000000".to_string();
        let mut ledger = LedgerState::new(vec![Account::new(
            from.clone(),
            150,
            Some(public_key_hex.clone()),
        )]);
        let payment = signed_payment_v2_with_minimum_fee(
            &genesis,
            &key_pair,
            to.clone(),
            25,
            1,
            vec![sample_memo()],
        );

        let receipt = execute_payment_v2(&genesis, &mut ledger, &payment);

        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.tx_id, payment_v2_tx_id(&payment));
        assert_eq!(
            ledger.account(&payment.unsigned.from).unwrap().balance,
            150 - 25 - payment.unsigned.fee
        );
        assert_eq!(ledger.account(&payment.unsigned.from).unwrap().sequence, 1);
        assert_eq!(ledger.account(&to).unwrap().balance, 25);
        assert_eq!(receipt.fee_charged, payment.unsigned.fee);
        assert_eq!(receipt.fee_burned, payment.unsigned.fee);
        assert!(receipt.minimum_fee <= payment.unsigned.fee);
        assert_eq!(receipt.state_expansion_fee, TRANSFER_ACCOUNT_CREATION_FEE);

        let replay = execute_payment_v2(&genesis, &mut ledger, &payment);
        assert!(!replay.accepted);
        assert_eq!(replay.code, "bad_sequence");
    }

    #[test]
    fn nft_transaction_mint_transfer_burn_and_reject_replay() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let owner_key = ml_dsa_65_keygen().expect("owner keygen");
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let owner = address_from_public_key(&owner_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                100,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                owner.clone(),
                100,
                Some(bytes_to_hex(&owner_key.public_key)),
            ),
            Account::new(
                recipient.clone(),
                100,
                Some(bytes_to_hex(&recipient_key.public_key)),
            ),
        ]);

        let mint = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            postfiat_types::NFT_MINT_TRANSACTION_KIND,
            1,
            nft_mint_operation(issuer.clone(), owner.clone(), 42, NFT_FLAG_TRANSFERABLE),
        );
        let mint_minimum_fee = minimum_nft_transaction_fee_for_ledger(&ledger, &mint);
        let mint_receipt = execute_nft_transaction(&genesis, &mut ledger, &mint);
        assert!(mint_receipt.accepted, "{mint_receipt:?}");
        assert_eq!(mint_receipt.tx_id, nft_transaction_tx_id(&mint));
        assert_eq!(mint_receipt.fee_charged, mint.unsigned.fee);
        assert_eq!(mint_receipt.fee_burned, mint.unsigned.fee);
        assert_eq!(mint_receipt.minimum_fee, mint_minimum_fee);
        assert_eq!(mint_receipt.state_expansion_fee, NFT_STATE_EXPANSION_FEE);
        assert_eq!(ledger.account(&issuer).unwrap().sequence, 1);
        let nft_id =
            postfiat_types::nft_id(&genesis.chain_id, &issuer, "ART-2026", 42).expect("nft id");
        assert_eq!(ledger.nft(&nft_id).expect("minted nft").owner, owner);

        let replay = execute_nft_transaction(&genesis, &mut ledger, &mint);
        assert!(!replay.accepted);
        assert_eq!(replay.code, "bad_sequence");

        let transfer = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            postfiat_types::NFT_TRANSFER_TRANSACTION_KIND,
            1,
            NftTransactionOperation::NftTransfer(NftTransferOperation {
                nft_id: nft_id.clone(),
                from: owner.clone(),
                to: recipient.clone(),
                issuer: String::new(),
                issuer_transfer_fee: 0,
            }),
        );
        let transfer_receipt = execute_nft_transaction(&genesis, &mut ledger, &transfer);
        assert!(transfer_receipt.accepted, "{transfer_receipt:?}");
        assert_eq!(
            ledger.nft(&nft_id).expect("transferred nft").owner,
            recipient
        );
        assert_eq!(ledger.account(&owner).unwrap().sequence, 1);

        let burn = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &recipient_key,
            postfiat_types::NFT_BURN_TRANSACTION_KIND,
            1,
            NftTransactionOperation::NftBurn(NftBurnOperation {
                nft_id: nft_id.clone(),
                owner: recipient.clone(),
            }),
        );
        let burn_receipt = execute_nft_transaction(&genesis, &mut ledger, &burn);
        assert!(burn_receipt.accepted, "{burn_receipt:?}");
        assert!(ledger.nft(&nft_id).expect("burned nft").burned);
        assert_eq!(ledger.account(&recipient).unwrap().sequence, 1);
        assert!(!ledger
            .nft_indexes(&genesis.chain_id)
            .expect("nft indexes")
            .by_owner
            .contains_key(&recipient));

        let burned_transfer = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &recipient_key,
            postfiat_types::NFT_TRANSFER_TRANSACTION_KIND,
            2,
            NftTransactionOperation::NftTransfer(NftTransferOperation {
                nft_id,
                from: recipient.clone(),
                to: owner.clone(),
                issuer: String::new(),
                issuer_transfer_fee: 0,
            }),
        );
        let rejected = execute_nft_transaction(&genesis, &mut ledger, &burned_transfer);
        assert!(!rejected.accepted);
        assert_eq!(rejected.code, "nft_burned");
    }

    #[test]
    fn nft_transaction_enforces_issuer_transfer_fee() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let owner_key = ml_dsa_65_keygen().expect("owner keygen");
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let owner = address_from_public_key(&owner_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                200,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                owner.clone(),
                100,
                Some(bytes_to_hex(&owner_key.public_key)),
            ),
            Account::new(
                recipient.clone(),
                100,
                Some(bytes_to_hex(&recipient_key.public_key)),
            ),
        ]);
        let mint_operation = NftTransactionOperation::NftMint(NftMintOperation {
            issuer: issuer.clone(),
            collection_id: "ART-2026".to_string(),
            serial: 99,
            owner: owner.clone(),
            metadata_hash: "abababababababababababababababababababababababababababababababab"
                .to_string(),
            metadata_uri: String::new(),
            flags: NFT_FLAG_TRANSFERABLE,
            collection_flags: 0,
            issuer_transfer_fee: 7,
        });
        let mint = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            postfiat_types::NFT_MINT_TRANSACTION_KIND,
            1,
            mint_operation,
        );
        let mint_receipt = execute_nft_transaction(&genesis, &mut ledger, &mint);
        assert!(mint_receipt.accepted, "{mint_receipt:?}");
        let nft_id =
            postfiat_types::nft_id(&genesis.chain_id, &issuer, "ART-2026", 99).expect("nft id");
        assert_eq!(
            ledger.nft(&nft_id).expect("minted nft").issuer_transfer_fee,
            7
        );

        let missing_fee_transfer = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            postfiat_types::NFT_TRANSFER_TRANSACTION_KIND,
            1,
            NftTransactionOperation::NftTransfer(NftTransferOperation {
                nft_id: nft_id.clone(),
                from: owner.clone(),
                to: recipient.clone(),
                issuer: String::new(),
                issuer_transfer_fee: 0,
            }),
        );
        let rejected = execute_nft_transaction(&genesis, &mut ledger, &missing_fee_transfer);
        assert!(!rejected.accepted);
        assert_eq!(rejected.code, "nft_issuer_transfer_fee_mismatch");

        let owner_balance_before = ledger.account(&owner).expect("owner").balance;
        let issuer_balance_before = ledger.account(&issuer).expect("issuer").balance;
        let transfer = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            postfiat_types::NFT_TRANSFER_TRANSACTION_KIND,
            1,
            NftTransactionOperation::NftTransfer(NftTransferOperation {
                nft_id: nft_id.clone(),
                from: owner.clone(),
                to: recipient.clone(),
                issuer: issuer.clone(),
                issuer_transfer_fee: 7,
            }),
        );
        let transfer_receipt = execute_nft_transaction(&genesis, &mut ledger, &transfer);
        assert!(transfer_receipt.accepted, "{transfer_receipt:?}");
        assert_eq!(transfer_receipt.nft_issuer_transfer_fee, 7);
        assert_eq!(
            transfer_receipt
                .nft_issuer_transfer_fee_recipient
                .as_deref(),
            Some(issuer.as_str())
        );
        assert_eq!(
            ledger.account(&owner).expect("owner").balance,
            owner_balance_before - transfer.unsigned.fee - 7
        );
        assert_eq!(
            ledger.account(&issuer).expect("issuer").balance,
            issuer_balance_before + 7
        );
        assert_eq!(
            ledger.nft(&nft_id).expect("transferred nft").owner,
            recipient
        );
    }

    #[test]
    fn nft_transaction_enforces_collection_policy_flags() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let owner_key = ml_dsa_65_keygen().expect("owner keygen");
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let owner = address_from_public_key(&owner_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                200,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                owner.clone(),
                200,
                Some(bytes_to_hex(&owner_key.public_key)),
            ),
            Account::new(
                recipient.clone(),
                200,
                Some(bytes_to_hex(&recipient_key.public_key)),
            ),
        ]);
        let collection_flags =
            NFT_COLLECTION_FLAG_TRANSFER_LOCKED | NFT_COLLECTION_FLAG_BURN_LOCKED;
        let mint_operation = NftTransactionOperation::NftMint(NftMintOperation {
            issuer: issuer.clone(),
            collection_id: "POLICY-2026".to_string(),
            serial: 1,
            owner: owner.clone(),
            metadata_hash: "abababababababababababababababababababababababababababababababab"
                .to_string(),
            metadata_uri: String::new(),
            flags: NFT_FLAG_TRANSFERABLE,
            collection_flags,
            issuer_transfer_fee: 0,
        });
        let mint = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            postfiat_types::NFT_MINT_TRANSACTION_KIND,
            1,
            mint_operation,
        );
        let mint_receipt = execute_nft_transaction(&genesis, &mut ledger, &mint);
        assert!(mint_receipt.accepted, "{mint_receipt:?}");
        assert_eq!(mint_receipt.nft_collection_flags, collection_flags);
        let nft_id =
            postfiat_types::nft_id(&genesis.chain_id, &issuer, "POLICY-2026", 1).expect("nft id");
        assert_eq!(
            ledger.nft(&nft_id).expect("minted nft").collection_flags,
            collection_flags
        );

        let mismatched_policy_mint = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            postfiat_types::NFT_MINT_TRANSACTION_KIND,
            2,
            NftTransactionOperation::NftMint(NftMintOperation {
                issuer: issuer.clone(),
                collection_id: "POLICY-2026".to_string(),
                serial: 2,
                owner: owner.clone(),
                metadata_hash: "abababababababababababababababababababababababababababababababab"
                    .to_string(),
                metadata_uri: String::new(),
                flags: NFT_FLAG_TRANSFERABLE,
                collection_flags: 0,
                issuer_transfer_fee: 0,
            }),
        );
        let mismatched_policy_receipt =
            execute_nft_transaction(&genesis, &mut ledger, &mismatched_policy_mint);
        assert!(!mismatched_policy_receipt.accepted);
        assert_eq!(
            mismatched_policy_receipt.code,
            "nft_collection_policy_mismatch"
        );

        let transfer = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            postfiat_types::NFT_TRANSFER_TRANSACTION_KIND,
            1,
            NftTransactionOperation::NftTransfer(NftTransferOperation {
                nft_id: nft_id.clone(),
                from: owner.clone(),
                to: recipient,
                issuer: String::new(),
                issuer_transfer_fee: 0,
            }),
        );
        let transfer_receipt = execute_nft_transaction(&genesis, &mut ledger, &transfer);
        assert!(!transfer_receipt.accepted);
        assert_eq!(transfer_receipt.code, "nft_collection_transfer_locked");

        let burn = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            postfiat_types::NFT_BURN_TRANSACTION_KIND,
            1,
            NftTransactionOperation::NftBurn(NftBurnOperation {
                nft_id: nft_id.clone(),
                owner: owner.clone(),
            }),
        );
        let burn_receipt = execute_nft_transaction(&genesis, &mut ledger, &burn);
        assert!(!burn_receipt.accepted);
        assert_eq!(burn_receipt.code, "nft_collection_burn_locked");
        let nft = ledger.nft(&nft_id).expect("locked nft");
        assert_eq!(nft.owner, owner);
        assert!(!nft.burned);
    }

    #[test]
    fn nft_transaction_rejects_duplicate_unauthorized_and_nontransferable_paths() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let owner_key = ml_dsa_65_keygen().expect("owner keygen");
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let owner = address_from_public_key(&owner_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                100,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                owner.clone(),
                100,
                Some(bytes_to_hex(&owner_key.public_key)),
            ),
            Account::new(
                recipient.clone(),
                100,
                Some(bytes_to_hex(&recipient_key.public_key)),
            ),
        ]);

        let mint = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            postfiat_types::NFT_MINT_TRANSACTION_KIND,
            1,
            nft_mint_operation(issuer.clone(), owner.clone(), 7, 0),
        );
        let mint_receipt = execute_nft_transaction(&genesis, &mut ledger, &mint);
        assert!(mint_receipt.accepted, "{mint_receipt:?}");
        let nft_id =
            postfiat_types::nft_id(&genesis.chain_id, &issuer, "ART-2026", 7).expect("nft id");

        let duplicate = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            postfiat_types::NFT_MINT_TRANSACTION_KIND,
            2,
            nft_mint_operation(issuer.clone(), owner.clone(), 7, 0),
        );
        let duplicate_receipt = execute_nft_transaction(&genesis, &mut ledger, &duplicate);
        assert!(!duplicate_receipt.accepted);
        assert_eq!(duplicate_receipt.code, "duplicate_nft");

        let unauthorized = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &recipient_key,
            postfiat_types::NFT_TRANSFER_TRANSACTION_KIND,
            1,
            NftTransactionOperation::NftTransfer(NftTransferOperation {
                nft_id: nft_id.clone(),
                from: recipient.clone(),
                to: issuer.clone(),
                issuer: String::new(),
                issuer_transfer_fee: 0,
            }),
        );
        let unauthorized_receipt = execute_nft_transaction(&genesis, &mut ledger, &unauthorized);
        assert!(!unauthorized_receipt.accepted);
        assert_eq!(unauthorized_receipt.code, "nft_owner_mismatch");

        let nontransferable = signed_nft_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            postfiat_types::NFT_TRANSFER_TRANSACTION_KIND,
            1,
            NftTransactionOperation::NftTransfer(NftTransferOperation {
                nft_id,
                from: owner.clone(),
                to: recipient.clone(),
                issuer: String::new(),
                issuer_transfer_fee: 0,
            }),
        );
        let nontransferable_receipt =
            execute_nft_transaction(&genesis, &mut ledger, &nontransferable);
        assert!(!nontransferable_receipt.accepted);
        assert_eq!(nontransferable_receipt.code, "nft_not_transferable");

        let mut wrong_chain = mint.clone();
        wrong_chain.unsigned.chain_id = "postfiat-other".to_string();
        let wrong_chain = execute_nft_transaction(&genesis, &mut ledger, &wrong_chain);
        assert!(!wrong_chain.accepted);
        assert_eq!(wrong_chain.code, "wrong_chain");
    }

    #[test]
    fn issued_mint_counts_fastlane_reserve_against_supply_cap() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let holder_key = ml_dsa_65_keygen().expect("holder keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let holder = address_from_public_key(&holder_key.public_key);
        let mut asset =
            AssetDefinition::new(&genesis.chain_id, &issuer, "FASTCAP", 1, 0).expect("asset");
        asset.max_supply = Some(10);
        let mut line = TrustLine::new(&holder, &issuer, &asset.asset_id, 20, 0)
            .expect("holder trustline");
        line.authorized = true;
        let fast_asset_id = FastAssetIdV1(
            hex_to_bytes(&asset.asset_id)
                .expect("asset id hex")
                .try_into()
                .expect("48-byte asset id"),
        );
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                10_000,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                holder.clone(),
                10_000,
                Some(bytes_to_hex(&holder_key.public_key)),
            ),
        ]);
        ledger.asset_definitions.push(asset.clone());
        ledger.trustlines.push(line);
        ledger.fast_lane_reserves.push(FastLaneReserveBalanceV1 {
            asset_id: fast_asset_id,
            amount_atoms: 10,
        });

        let issue = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.clone(),
                to: holder,
                issuer,
                asset_id: asset.asset_id,
                amount: 1,
            }),
        );
        let before = ledger.clone();
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &issue, 1);
        assert!(!receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "issued_supply_cap_exceeded");
        assert_eq!(ledger, before);
    }

    #[test]
    fn asset_transaction_property_conserves_supply_and_trustline_limits() {
        let genesis = Genesis::new("postfiat-local");
        let scenarios = [
            (10_u64, 1_u64, 1_u64, 10_u64, 5_u64),
            (40, 15, 4, 50, 30),
            (99, 45, 44, 100, 50),
            (250, 200, 199, 300, 250),
        ];

        for (index, (issue_amount, transfer_amount, burn_amount, holder_limit, recipient_limit)) in
            scenarios.into_iter().enumerate()
        {
            let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
            let holder_key = ml_dsa_65_keygen().expect("holder keygen");
            let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
            let issuer = address_from_public_key(&issuer_key.public_key);
            let holder = address_from_public_key(&holder_key.public_key);
            let recipient = address_from_public_key(&recipient_key.public_key);
            let mut ledger = LedgerState::new(vec![
                Account::new(
                    issuer.clone(),
                    5_000,
                    Some(bytes_to_hex(&issuer_key.public_key)),
                ),
                Account::new(
                    holder.clone(),
                    5_000,
                    Some(bytes_to_hex(&holder_key.public_key)),
                ),
                Account::new(
                    recipient.clone(),
                    5_000,
                    Some(bytes_to_hex(&recipient_key.public_key)),
                ),
            ]);

            let create = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &issuer_key,
                ASSET_CREATE_TRANSACTION_KIND,
                1,
                AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                    issuer: issuer.clone(),
                    code: format!("PROP{index}"),
                    version: 1,
                    precision: 0,
                    display_name: String::new(),
                    max_supply: Some(issue_amount),
                    requires_authorization: false,
                    freeze_enabled: true,
                    clawback_enabled: false,
                }),
            );
            let receipt = execute_asset_transaction(&genesis, &mut ledger, &create, 1);
            assert!(receipt.accepted, "{receipt:?}");
            let asset_id = ledger.asset_definitions[0].asset_id.clone();
            assert_issued_asset_invariants(&genesis, &ledger, &asset_id, 0, &[]);

            let holder_trust = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &holder_key,
                TRUST_SET_TRANSACTION_KIND,
                1,
                AssetTransactionOperation::TrustSet(TrustSetOperation {
                    account: holder.clone(),
                    issuer: issuer.clone(),
                    asset_id: asset_id.clone(),
                    limit: holder_limit,
                    authorized: false,
                    frozen: false,
                    reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
                }),
            );
            let receipt = execute_asset_transaction(&genesis, &mut ledger, &holder_trust, 1);
            assert!(receipt.accepted, "{receipt:?}");
            assert_issued_asset_invariants(
                &genesis,
                &ledger,
                &asset_id,
                0,
                &[(holder.as_str(), 0)],
            );

            let recipient_trust = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &recipient_key,
                TRUST_SET_TRANSACTION_KIND,
                1,
                AssetTransactionOperation::TrustSet(TrustSetOperation {
                    account: recipient.clone(),
                    issuer: issuer.clone(),
                    asset_id: asset_id.clone(),
                    limit: recipient_limit,
                    authorized: false,
                    frozen: false,
                    reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
                }),
            );
            let receipt = execute_asset_transaction(&genesis, &mut ledger, &recipient_trust, 1);
            assert!(receipt.accepted, "{receipt:?}");
            assert_issued_asset_invariants(
                &genesis,
                &ledger,
                &asset_id,
                0,
                &[(holder.as_str(), 0), (recipient.as_str(), 0)],
            );

            let issue = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &issuer_key,
                ISSUED_PAYMENT_TRANSACTION_KIND,
                2,
                AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                    from: issuer.clone(),
                    to: holder.clone(),
                    issuer: issuer.clone(),
                    asset_id: asset_id.clone(),
                    amount: issue_amount,
                }),
            );
            let receipt = execute_asset_transaction(&genesis, &mut ledger, &issue, 1);
            assert!(receipt.accepted, "{receipt:?}");
            assert_issued_asset_invariants(
                &genesis,
                &ledger,
                &asset_id,
                issue_amount,
                &[(holder.as_str(), issue_amount), (recipient.as_str(), 0)],
            );

            let holder_to_recipient = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &holder_key,
                ISSUED_PAYMENT_TRANSACTION_KIND,
                2,
                AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                    from: holder.clone(),
                    to: recipient.clone(),
                    issuer: issuer.clone(),
                    asset_id: asset_id.clone(),
                    amount: transfer_amount,
                }),
            );
            let receipt = execute_asset_transaction(&genesis, &mut ledger, &holder_to_recipient, 1);
            assert!(receipt.accepted, "{receipt:?}");
            assert_issued_asset_invariants(
                &genesis,
                &ledger,
                &asset_id,
                issue_amount,
                &[
                    (holder.as_str(), issue_amount - transfer_amount),
                    (recipient.as_str(), transfer_amount),
                ],
            );

            let recipient_burn = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &recipient_key,
                ASSET_BURN_TRANSACTION_KIND,
                2,
                AssetTransactionOperation::AssetBurn(AssetBurnOperation {
                    owner: recipient.clone(),
                    issuer: issuer.clone(),
                    asset_id: asset_id.clone(),
                    amount: burn_amount,
                }),
            );
            let receipt = execute_asset_transaction(&genesis, &mut ledger, &recipient_burn, 1);
            assert!(receipt.accepted, "{receipt:?}");
            let holder_balance = issue_amount - transfer_amount;
            let recipient_balance = transfer_amount - burn_amount;
            let supply_after_burn = issue_amount - burn_amount;
            assert_issued_asset_invariants(
                &genesis,
                &ledger,
                &asset_id,
                supply_after_burn,
                &[
                    (holder.as_str(), holder_balance),
                    (recipient.as_str(), recipient_balance),
                ],
            );

            let over_cap_issue = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &issuer_key,
                ISSUED_PAYMENT_TRANSACTION_KIND,
                3,
                AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                    from: issuer.clone(),
                    to: recipient.clone(),
                    issuer: issuer.clone(),
                    asset_id: asset_id.clone(),
                    amount: burn_amount + 1,
                }),
            );
            let before_reject = ledger.clone();
            let receipt = execute_asset_transaction(&genesis, &mut ledger, &over_cap_issue, 1);
            assert!(!receipt.accepted);
            assert_eq!(receipt.code, "issued_supply_cap_exceeded");
            assert_eq!(ledger, before_reject);

            let excessive_holder_payment = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &holder_key,
                ISSUED_PAYMENT_TRANSACTION_KIND,
                3,
                AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                    from: holder.clone(),
                    to: recipient.clone(),
                    issuer: issuer.clone(),
                    asset_id: asset_id.clone(),
                    amount: holder_balance + 1,
                }),
            );
            let before_reject = ledger.clone();
            let receipt =
                execute_asset_transaction(&genesis, &mut ledger, &excessive_holder_payment, 1);
            assert!(!receipt.accepted);
            assert_eq!(receipt.code, "insufficient_issued_balance");
            assert_eq!(ledger, before_reject);

            if recipient_balance > 1 {
                let shrink_below_balance = signed_asset_transaction_with_minimum_fee(
                    &genesis,
                    &ledger,
                    &recipient_key,
                    TRUST_SET_TRANSACTION_KIND,
                    3,
                    AssetTransactionOperation::TrustSet(TrustSetOperation {
                        account: recipient.clone(),
                        issuer: issuer.clone(),
                        asset_id: asset_id.clone(),
                        limit: recipient_balance - 1,
                        authorized: false,
                        frozen: false,
                        reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
                    }),
                );
                let before_reject = ledger.clone();
                let receipt =
                    execute_asset_transaction(&genesis, &mut ledger, &shrink_below_balance, 1);
                assert!(!receipt.accepted);
                assert_eq!(receipt.code, "trustline_limit_too_low");
                assert_eq!(ledger, before_reject);
            }

            assert_issued_asset_invariants(
                &genesis,
                &ledger,
                &asset_id,
                supply_after_burn,
                &[
                    (holder.as_str(), holder_balance),
                    (recipient.as_str(), recipient_balance),
                ],
            );
        }
    }

    #[test]
    fn asset_transactions_create_trust_pay_burn_and_reject_frozen_lines() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let holder_key = ml_dsa_65_keygen().expect("holder keygen");
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let holder = address_from_public_key(&holder_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                1_000,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                holder.clone(),
                1_000,
                Some(bytes_to_hex(&holder_key.public_key)),
            ),
            Account::new(
                recipient.clone(),
                1_000,
                Some(bytes_to_hex(&recipient_key.public_key)),
            ),
        ]);

        let create = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "USD".to_string(),
                version: 1,
                precision: 6,
                display_name: "US Dollar".to_string(),
                max_supply: Some(100),
                requires_authorization: true,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        let create_receipt = execute_asset_transaction(&genesis, &mut ledger, &create, 1);
        assert!(create_receipt.accepted, "{create_receipt:?}");
        assert_eq!(create_receipt.tx_id, asset_transaction_tx_id(&create));
        assert_eq!(
            create_receipt.state_expansion_fee,
            ASSET_DEFINITION_STATE_EXPANSION_FEE
        );
        let asset_id = ledger.asset_definitions[0].asset_id.clone();

        let holder_trust = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                limit: 50,
                authorized: false,
                frozen: false,
                reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        let holder_trust_receipt = execute_asset_transaction(&genesis, &mut ledger, &holder_trust, 1);
        assert!(holder_trust_receipt.accepted, "{holder_trust_receipt:?}");
        assert_eq!(
            holder_trust_receipt.state_expansion_fee,
            TRUSTLINE_STATE_EXPANSION_FEE
        );
        assert!(
            !ledger
                .trustline_for_account_asset(&holder, &asset_id)
                .expect("holder line")
                .authorized
        );

        let authorize_holder = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            TRUST_SET_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                limit: 50,
                authorized: true,
                frozen: false,
                reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        let authorize_holder_receipt =
            execute_asset_transaction(&genesis, &mut ledger, &authorize_holder, 1);
        assert!(
            authorize_holder_receipt.accepted,
            "{authorize_holder_receipt:?}"
        );

        let issue_to_holder = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.clone(),
                to: holder.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                amount: 25,
            }),
        );
        let issue_receipt = execute_asset_transaction(&genesis, &mut ledger, &issue_to_holder, 1);
        assert!(issue_receipt.accepted, "{issue_receipt:?}");
        assert_eq!(
            ledger
                .trustline_for_account_asset(&holder, &asset_id)
                .expect("holder line")
                .balance,
            25
        );

        let implicit_recipient_payment = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: holder.clone(),
                to: recipient.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                amount: 10,
            }),
        );
        let implicit_recipient_receipt =
            execute_asset_transaction(&genesis, &mut ledger, &implicit_recipient_payment, 1);
        assert!(
            implicit_recipient_receipt.accepted,
            "{implicit_recipient_receipt:?}"
        );
        let implicit_line = ledger
            .trustline_for_account_asset(&recipient, &asset_id)
            .expect("recipient implicit line");
        assert_eq!(implicit_line.balance, 10);
        assert_eq!(implicit_line.limit, 10);
        assert_eq!(implicit_line.reserve_paid, 0);
        assert!(implicit_line.authorized);

        let recipient_trust = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &recipient_key,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: recipient.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                limit: 50,
                authorized: false,
                frozen: false,
                reserve_paid: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &recipient_trust, 1).accepted);

        let authorize_recipient = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            TRUST_SET_TRANSACTION_KIND,
            4,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: recipient.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                limit: 50,
                authorized: true,
                frozen: false,
                reserve_paid: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &authorize_recipient, 1).accepted);

        let holder_to_recipient = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: holder.clone(),
                to: recipient.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                amount: 10,
            }),
        );
        let transfer_receipt =
            execute_asset_transaction(&genesis, &mut ledger, &holder_to_recipient, 1);
        assert!(transfer_receipt.accepted, "{transfer_receipt:?}");
        assert_eq!(
            ledger
                .trustline_for_account_asset(&holder, &asset_id)
                .expect("holder line")
                .balance,
            5
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&recipient, &asset_id)
                .expect("recipient line")
                .balance,
            20
        );

        let freeze_recipient = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            TRUST_SET_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: recipient.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                limit: 50,
                authorized: true,
                frozen: true,
                reserve_paid: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &freeze_recipient, 1).accepted);

        let frozen_burn = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &recipient_key,
            ASSET_BURN_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetBurn(AssetBurnOperation {
                owner: recipient.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                amount: 1,
            }),
        );
        let frozen_burn_receipt = execute_asset_transaction(&genesis, &mut ledger, &frozen_burn, 1);
        assert!(!frozen_burn_receipt.accepted);
        assert_eq!(frozen_burn_receipt.code, "frozen_trustline");
        assert_eq!(
            ledger
                .trustline_for_account_asset(&recipient, &asset_id)
                .expect("recipient line")
                .balance,
            20
        );

        let unfreeze_recipient = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            TRUST_SET_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: recipient.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                limit: 50,
                authorized: true,
                frozen: false,
                reserve_paid: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &unfreeze_recipient, 1).accepted);

        let unfrozen_burn = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &recipient_key,
            ASSET_BURN_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetBurn(AssetBurnOperation {
                owner: recipient.clone(),
                issuer,
                asset_id: asset_id.clone(),
                amount: 1,
            }),
        );
        let unfrozen_burn_receipt =
            execute_asset_transaction(&genesis, &mut ledger, &unfrozen_burn, 1);
        assert!(unfrozen_burn_receipt.accepted, "{unfrozen_burn_receipt:?}");
        assert_eq!(
            ledger
                .trustline_for_account_asset(&recipient, &asset_id)
                .expect("recipient line")
                .balance,
            19
        );
    }

    #[test]
    fn issued_payment_auto_expands_implicit_recipient_balance_record() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                1_000,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                recipient.clone(),
                1_000,
                Some(bytes_to_hex(&recipient_key.public_key)),
            ),
        ]);

        let create = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "USD".to_string(),
                version: 1,
                precision: 6,
                display_name: "US Dollar".to_string(),
                max_supply: Some(100),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &create, 1).accepted);
        let asset_id = ledger.asset_definitions[0].asset_id.clone();

        let first_issue = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.clone(),
                to: recipient.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                amount: 10,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &first_issue, 1).accepted);
        let implicit_line = ledger
            .trustline_for_account_asset(&recipient, &asset_id)
            .expect("recipient implicit line");
        assert_eq!(implicit_line.balance, 10);
        assert_eq!(implicit_line.limit, 10);
        assert_eq!(implicit_line.reserve_paid, 0);

        let second_issue = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.clone(),
                to: recipient.clone(),
                issuer,
                asset_id: asset_id.clone(),
                amount: 15,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &second_issue, 1);
        assert!(receipt.accepted, "{receipt:?}");

        let expanded_line = ledger
            .trustline_for_account_asset(&recipient, &asset_id)
            .expect("recipient expanded line");
        assert_eq!(expanded_line.balance, 25);
        assert_eq!(expanded_line.limit, 25);
        assert_eq!(expanded_line.reserve_paid, 0);
        assert_issued_asset_invariants(
            &genesis,
            &ledger,
            &asset_id,
            25,
            &[(recipient.as_str(), 25)],
        );
    }

    #[test]
    fn asset_clawback_requires_issuer_policy_and_rejects_native_pft() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let holder_key = ml_dsa_65_keygen().expect("holder keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let holder = address_from_public_key(&holder_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer.clone(),
                2_000,
                Some(bytes_to_hex(&issuer_key.public_key)),
            ),
            Account::new(
                holder.clone(),
                2_000,
                Some(bytes_to_hex(&holder_key.public_key)),
            ),
        ]);

        let create_no_clawback = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "NCB".to_string(),
                version: 1,
                precision: 0,
                display_name: String::new(),
                max_supply: Some(100),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &create_no_clawback, 1).accepted);
        let no_clawback_asset_id = ledger.asset_definitions[0].asset_id.clone();

        let holder_no_clawback_line = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder.clone(),
                issuer: issuer.clone(),
                asset_id: no_clawback_asset_id.clone(),
                limit: 50,
                authorized: false,
                frozen: false,
                reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut ledger, &holder_no_clawback_line, 1).accepted
        );

        let issue_no_clawback = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.clone(),
                to: holder.clone(),
                issuer: issuer.clone(),
                asset_id: no_clawback_asset_id.clone(),
                amount: 20,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &issue_no_clawback, 1).accepted);
        let reject_without_policy = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CLAWBACK_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::AssetClawback(AssetClawbackOperation {
                owner: holder.clone(),
                issuer: issuer.clone(),
                asset_id: no_clawback_asset_id.clone(),
                amount: 5,
            }),
        );
        let before_reject = ledger.clone();
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &reject_without_policy, 1);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "clawback_not_enabled");
        assert_eq!(ledger, before_reject);

        let create_clawback = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "CLAW".to_string(),
                version: 1,
                precision: 0,
                display_name: String::new(),
                max_supply: Some(100),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: true,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &create_clawback, 1).accepted);
        let clawback_asset_id = ledger.asset_definitions[1].asset_id.clone();

        let holder_clawback_line = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            TRUST_SET_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder.clone(),
                issuer: issuer.clone(),
                asset_id: clawback_asset_id.clone(),
                limit: 50,
                authorized: false,
                frozen: false,
                reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &holder_clawback_line, 1).accepted);

        let issue_clawback_asset = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            4,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.clone(),
                to: holder.clone(),
                issuer: issuer.clone(),
                asset_id: clawback_asset_id.clone(),
                amount: 20,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &issue_clawback_asset, 1).accepted);

        let freeze_holder = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            TRUST_SET_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder.clone(),
                issuer: issuer.clone(),
                asset_id: clawback_asset_id.clone(),
                limit: 50,
                authorized: true,
                frozen: true,
                reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &freeze_holder, 1).accepted);

        let clawback = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CLAWBACK_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::AssetClawback(AssetClawbackOperation {
                owner: holder.clone(),
                issuer: issuer.clone(),
                asset_id: clawback_asset_id.clone(),
                amount: 7,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &clawback, 1);
        assert!(receipt.accepted, "{receipt:?}");
        let frozen_line = ledger
            .trustline_for_account_asset(&holder, &clawback_asset_id)
            .expect("holder clawback line");
        assert!(frozen_line.frozen);
        assert_eq!(frozen_line.balance, 13);
        assert_issued_asset_invariants(
            &genesis,
            &ledger,
            &clawback_asset_id,
            13,
            &[(holder.as_str(), 13)],
        );

        let excessive_clawback = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CLAWBACK_TRANSACTION_KIND,
            7,
            AssetTransactionOperation::AssetClawback(AssetClawbackOperation {
                owner: holder.clone(),
                issuer: issuer.clone(),
                asset_id: clawback_asset_id,
                amount: 14,
            }),
        );
        let before_reject = ledger.clone();
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &excessive_clawback, 1);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "insufficient_issued_balance");
        assert_eq!(ledger, before_reject);

        let native_pft_clawback = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CLAWBACK_TRANSACTION_KIND,
            7,
            AssetTransactionOperation::AssetClawback(AssetClawbackOperation {
                owner: holder,
                issuer,
                asset_id: "PFT".to_string(),
                amount: 1,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &native_pft_clawback, 1);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "bad_asset_transaction_envelope");
        assert!(receipt.message.contains("asset_clawback.asset_id"));
    }
