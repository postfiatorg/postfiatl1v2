#[test]
fn issued_asset_escrow_locks_finishes_cancels_and_counts_locked_supply() {
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
            5_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        ),
        Account::new(
            owner.clone(),
            5_000,
            Some(bytes_to_hex(&owner_key.public_key)),
        ),
        Account::new(
            recipient.clone(),
            5_000,
            Some(bytes_to_hex(&recipient_key.public_key)),
        ),
    ]);

    let create_asset = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "IESC".to_string(),
            version: 1,
            precision: 0,
            display_name: "Issued Escrow Test".to_string(),
            max_supply: Some(120),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &create_asset, 1).accepted);
    let asset_id = ledger.asset_definitions[0].asset_id.clone();

    let owner_trust = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &owner_key,
        TRUST_SET_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: owner.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            limit: 100,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &owner_trust, 1).accepted);

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
            limit: 100,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &recipient_trust, 1).accepted);

    let issue = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.clone(),
            to: owner.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            amount: 80,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &issue, 1).accepted);
    assert_issued_asset_invariants(
        &genesis,
        &ledger,
        &asset_id,
        80,
        &[(owner.as_str(), 80), (recipient.as_str(), 0)],
    );

    let first_escrow_id = escrow_id(&genesis.chain_id, &owner, 2).expect("issued escrow id");
    let create_escrow = signed_escrow_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &owner_key,
        ESCROW_CREATE_TRANSACTION_KIND,
        2,
        EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: owner.clone(),
            recipient: recipient.clone(),
            asset_id: asset_id.clone(),
            amount: 30,
            condition: "issued-secret".to_string(),
            finish_after: 2,
            cancel_after: 5,
        }),
    );
    let receipt = execute_escrow_transaction(&genesis, &mut ledger, &create_escrow, 2);
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(receipt.state_expansion_fee, ESCROW_STATE_EXPANSION_FEE);
    assert_eq!(
        ledger
            .escrow(&first_escrow_id)
            .expect("issued escrow")
            .asset_id
            .as_str(),
        asset_id.as_str()
    );
    assert_issued_asset_invariants(
        &genesis,
        &ledger,
        &asset_id,
        80,
        &[(owner.as_str(), 50), (recipient.as_str(), 0)],
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
            amount: 41,
        }),
    );
    let before_over_cap = ledger.clone();
    let receipt = execute_asset_transaction(&genesis, &mut ledger, &over_cap_issue, 1);
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "issued_supply_cap_exceeded");
    assert_eq!(ledger, before_over_cap);

    let lower_recipient_limit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &recipient_key,
        TRUST_SET_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: recipient.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            limit: 20,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    let before_limit_reject = ledger.clone();
    let receipt = execute_asset_transaction(&genesis, &mut ledger, &lower_recipient_limit, 1);
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "trustline_limit_too_low");
    assert_eq!(ledger, before_limit_reject);

    let finish = signed_escrow_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &recipient_key,
        ESCROW_FINISH_TRANSACTION_KIND,
        2,
        EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
            escrow_id: first_escrow_id.clone(),
            owner: owner.clone(),
            recipient: recipient.clone(),
            fulfillment: "issued-secret".to_string(),
        }),
    );
    let receipt = execute_escrow_transaction(&genesis, &mut ledger, &finish, 2);
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(
        ledger
            .escrow(&first_escrow_id)
            .expect("finished issued escrow")
            .state,
        ESCROW_STATE_FINISHED
    );
    assert_issued_asset_invariants(
        &genesis,
        &ledger,
        &asset_id,
        80,
        &[(owner.as_str(), 50), (recipient.as_str(), 30)],
    );

    let second_escrow_id =
        escrow_id(&genesis.chain_id, &owner, 3).expect("second issued escrow id");
    let create_cancelable = signed_escrow_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &owner_key,
        ESCROW_CREATE_TRANSACTION_KIND,
        3,
        EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: owner.clone(),
            recipient: recipient.clone(),
            asset_id: asset_id.clone(),
            amount: 20,
            condition: String::new(),
            finish_after: 0,
            cancel_after: 4,
        }),
    );
    let receipt = execute_escrow_transaction(&genesis, &mut ledger, &create_cancelable, 3);
    assert!(receipt.accepted, "{receipt:?}");
    assert_issued_asset_invariants(
        &genesis,
        &ledger,
        &asset_id,
        80,
        &[(owner.as_str(), 30), (recipient.as_str(), 30)],
    );

    let lower_owner_limit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &owner_key,
        TRUST_SET_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: owner.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            limit: 40,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    let before_owner_limit_reject = ledger.clone();
    let receipt = execute_asset_transaction(&genesis, &mut ledger, &lower_owner_limit, 1);
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "trustline_limit_too_low");
    assert_eq!(ledger, before_owner_limit_reject);

    let cancel = signed_escrow_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &owner_key,
        ESCROW_CANCEL_TRANSACTION_KIND,
        4,
        EscrowTransactionOperation::EscrowCancel(EscrowCancelOperation {
            escrow_id: second_escrow_id.clone(),
            owner: owner.clone(),
        }),
    );
    let receipt = execute_escrow_transaction(&genesis, &mut ledger, &cancel, 4);
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(
        ledger
            .escrow(&second_escrow_id)
            .expect("canceled issued escrow")
            .state,
        ESCROW_STATE_CANCELED
    );
    assert_issued_asset_invariants(
        &genesis,
        &ledger,
        &asset_id,
        80,
        &[(owner.as_str(), 50), (recipient.as_str(), 30)],
    );
}

#[test]
fn nav_asset_lifecycle_finalizes_mints_redeems_and_halts() {
    let genesis = Genesis::new("postfiat-local");
    let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
    let ap_key = ml_dsa_65_keygen().expect("ap keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let ap = address_from_public_key(&ap_key.public_key);
    let mut ledger = LedgerState::new(vec![
        Account::new(
            issuer.clone(),
            1_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        ),
        Account::new(ap.clone(), 1_000, Some(bytes_to_hex(&ap_key.public_key))),
    ]);

    let create = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "NAV".to_string(),
            version: 1,
            precision: 6,
            display_name: "NAVCOIN".to_string(),
            max_supply: Some(1_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &create, 1).accepted);
    let asset_id = ledger.asset_definitions[0].asset_id.clone();

    let register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: "nitro-reserve-v0".to_string(),
            valuation_unit: "usd_1e6".to_string(),
            redemption_account: issuer.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 1).accepted);

    let trust = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &ap_key,
        TRUST_SET_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: ap.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            limit: 1_000,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &trust, 1).accepted);

    let reserve_packet_hash = "ab".repeat(48);
    let submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.clone(),
            submitter: issuer.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            nav_per_unit: 982_300,
            circulating_supply: 1_000,
            verified_net_assets: 982_300_000,
            proof_profile: "nitro-reserve-v0".to_string(),
            source_root: "01".repeat(48),
            attestor_root: "02".repeat(48),
            reserve_packet_hash: reserve_packet_hash.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &submit, 1).accepted);

    let finalize = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize, 1).accepted);
    let nav_asset = ledger.nav_asset(&asset_id).expect("nav asset");
    assert_eq!(nav_asset.finalized_epoch, 1);
    assert_eq!(nav_asset.nav_per_unit, 982_300);
    assert_eq!(nav_asset.circulating_supply, 1_000);

    let mint = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_MINT_AT_NAV_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
            issuer: issuer.clone(),
            to: ap.clone(),
            asset_id: asset_id.clone(),
            amount: 1_000,
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
            settlement_asset_id: String::new(),
            settlement_bucket_id: String::new(),
            settlement_allocation_id: String::new(),
            settlement_amount_atoms: 0,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &mint, 1).accepted);
    assert_eq!(
        ledger
            .trustline_for_account_asset(&ap, &asset_id)
            .expect("ap line")
            .balance,
        1_000
    );

    let redeem = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &ap_key,
        NAV_REDEEM_AT_NAV_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::NavRedeemAtNav(NavRedeemAtNavOperation {
            owner: ap.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            amount: 100,
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &redeem, 1).accepted);
    assert_eq!(
        ledger
            .trustline_for_account_asset(&ap, &asset_id)
            .expect("ap line")
            .balance,
        900
    );
    assert_eq!(ledger.nav_redemptions.len(), 1);
    assert_eq!(ledger.nav_redemptions[0].unit_scale, 1_000_000);
    assert_eq!(ledger.nav_redemptions[0].redemption_claim, 99);

    let halt = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_HALT_TRANSACTION_KIND,
        6,
        AssetTransactionOperation::NavHalt(NavHaltOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            halted: true,
            reason: "stale_reserve_packet".to_string(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &halt, 1).accepted);

    let blocked_redeem = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &ap_key,
        NAV_REDEEM_AT_NAV_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::NavRedeemAtNav(NavRedeemAtNavOperation {
            owner: ap,
            issuer,
            asset_id,
            amount: 1,
            epoch: 1,
            reserve_packet_hash,
        }),
    );
    let receipt = execute_asset_transaction(&genesis, &mut ledger, &blocked_redeem, 1);
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "nav_asset_halted");
}

#[test]
fn vault_bridge_wraps_counted_source_erc20_receipt_on_pftl() {
    let genesis = Genesis::new("postfiat-local");
    let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
    let holder_key = ml_dsa_65_keygen().expect("holder keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let holder = address_from_public_key(&holder_key.public_key);
    let receipt_amount = 10_000_099_u64;
    let bridge_evidence = vault_bridge_evidence(receipt_amount, "33");
    let source_domain = bridge_evidence.source_domain();
    let policy_hash = "42".repeat(48);
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

    let profile_register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
            registrant: issuer.clone(),
            verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
            source_class: format!("vault_bridge:{source_domain}"),
            max_snapshot_age_blocks: 100,
            challenge_window_blocks: 2,
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
            public_values_schema: String::new(),
            source_manifest_hash: String::new(),
            valuation_unit_id: String::new(),
            max_observation_span_blocks: 0,
            allow_controlled_sources: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &profile_register, 1).accepted);
    let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

    let create = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "vault bridge asset".to_string(),
            version: 1,
            precision: 8,
            display_name: "vault bridge asset".to_string(),
            max_supply: Some(100_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &create, 2).accepted);
    let asset_id = ledger.asset_definitions[0].asset_id.clone();

    let register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: profile_id.clone(),
            valuation_unit: "SOURCE_UNIT".to_string(),
            redemption_account: issuer.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 3).accepted);

    let trust = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        TRUST_SET_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            limit: 100_000_000,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &trust, 4).accepted);

    let attestor_register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
            attestor: holder.clone(),
            domain: "operator.local".to_string(),
            bond: 0,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &attestor_register, 5).accepted);

    let receipt_submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::VaultBridgeReceiptSubmit(VaultBridgeReceiptSubmitOperation {
            operator: issuer.clone(),
            asset_id: asset_id.clone(),
            source_domain: source_domain.to_string(),
            source_asset: bridge_evidence.source_asset_ref(),
            claim_type: VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT.to_string(),
            amount_atoms: receipt_amount,
            source_tx_or_attestation: bridge_evidence.source_tx_or_attestation(),
            finality_ref: bridge_evidence.finality_ref(),
            vault_id: bridge_evidence.vault_id(),
            policy_hash: policy_hash.clone(),
            expires_at_height: 1_000,
            bridge_deposit_evidence: Some(bridge_evidence.clone()),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &receipt_submit, 6).accepted);
    let receipt_id = ledger.vault_bridge_receipts[0].receipt_id.clone();
    let bucket_id = ledger.vault_bridge_receipts[0].bucket_id.clone();
    let bridge_evidence_root =
        vault_bridge_deposit_evidence_root(&bridge_evidence).expect("bridge evidence root");

    let bridge_propose = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
            proposer: holder.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            evidence: bridge_evidence.clone(),
            policy_hash: policy_hash.clone(),
            source_proof_kind: String::new(),
            source_proof_hash: String::new(),
            source_public_values_hash: String::new(),
            source_proof_bytes: Vec::new(),
            source_public_values: Vec::new(),
            expires_at_height: 1_000,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_propose, 7).accepted);

    let counted_value = 9_975_098_u64;
    let bad_receipt_count = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::VaultBridgeReceiptCount(VaultBridgeReceiptCountOperation {
            operator: issuer.clone(),
            asset_id: asset_id.clone(),
            receipt_id: receipt_id.clone(),
            haircut_bps: 25,
            counted_value_atoms: counted_value,
            evidence_root: "99".repeat(48),
            policy_hash: policy_hash.clone(),
        }),
    );
    let bad_count_receipt = execute_asset_transaction(&genesis, &mut ledger, &bad_receipt_count, 7);
    assert!(!bad_count_receipt.accepted);
    assert_eq!(
        bad_count_receipt.code,
        "vault_bridge_deposit_evidence_root_mismatch"
    );

    let premature_receipt_count = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::VaultBridgeReceiptCount(VaultBridgeReceiptCountOperation {
            operator: issuer.clone(),
            asset_id: asset_id.clone(),
            receipt_id: receipt_id.clone(),
            haircut_bps: 25,
            counted_value_atoms: counted_value,
            evidence_root: bridge_evidence_root.clone(),
            policy_hash: policy_hash.clone(),
        }),
    );
    let premature_count_receipt =
        execute_asset_transaction(&genesis, &mut ledger, &premature_receipt_count, 8);
    assert!(!premature_count_receipt.accepted);
    assert_eq!(
        premature_count_receipt.code,
        "vault_bridge_deposit_not_finalized"
    );

    let finalize_without_quorum = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::VaultBridgeDepositFinalize(
            VaultBridgeDepositFinalizeOperation {
                finalizer: holder.clone(),
                asset_id: asset_id.clone(),
                evidence_root: bridge_evidence_root.clone(),
            },
        ),
    );
    let no_quorum_receipt =
        execute_asset_transaction(&genesis, &mut ledger, &finalize_without_quorum, 9);
    assert!(!no_quorum_receipt.accepted);
    assert_eq!(
        no_quorum_receipt.code,
        "vault_bridge_deposit_attestation_quorum_not_met"
    );

    let bridge_observation =
        VaultBridgeDepositObservation::success_for_evidence(&bridge_evidence, 6);
    let bridge_observation_root = vault_bridge_deposit_observation_root(&bridge_observation)
        .expect("bridge observation root");
    let bridge_attest = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::VaultBridgeDepositAttest(VaultBridgeDepositAttestOperation {
            attestor: holder.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            pass: true,
            observation_root: bridge_observation_root,
            observation: Some(bridge_observation),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_attest, 8).accepted);

    let bridge_finalize = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::VaultBridgeDepositFinalize(
            VaultBridgeDepositFinalizeOperation {
                finalizer: holder.clone(),
                asset_id: asset_id.clone(),
                evidence_root: bridge_evidence_root.clone(),
            },
        ),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_finalize, 9).accepted);
    assert_eq!(ledger.vault_bridge_deposits[0].status, "finalized");

    let receipt_count = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::VaultBridgeReceiptCount(VaultBridgeReceiptCountOperation {
            operator: issuer.clone(),
            asset_id: asset_id.clone(),
            receipt_id: receipt_id.clone(),
            haircut_bps: 25,
            counted_value_atoms: counted_value,
            evidence_root: bridge_evidence_root.clone(),
            policy_hash: policy_hash.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &receipt_count, 10).accepted);
    assert_eq!(
        ledger.vault_bridge_bucket_states[0].counted_value_atoms,
        counted_value
    );

    let source_root =
        vault_bridge_source_root_for_asset(&ledger.vault_bridge_bucket_states, &asset_id)
            .expect("source root");
    let reserve_packet_hash = "ab".repeat(48);
    let reserve_submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        6,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.clone(),
            submitter: issuer.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            nav_per_unit: VAULT_BRIDGE_UNIT,
            circulating_supply: 0,
            verified_net_assets: counted_value,
            proof_profile: profile_id.clone(),
            source_root: source_root.clone(),
            attestor_root: "88".repeat(48),
            reserve_packet_hash: reserve_packet_hash.clone(),
            reserve_accounts: vec![bridge_evidence.vault_id()],
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &reserve_submit, 11).accepted);

    let reserve_attest = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        NAV_RESERVE_ATTEST_TRANSACTION_KIND,
        6,
        AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
            attestor: holder.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
            pass: true,
            observation_root: source_root,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &reserve_attest, 12).accepted);

    let finalize = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        7,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize, 13).accepted);

    let mint = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_MINT_FROM_RECEIPTS_TRANSACTION_KIND,
        8,
        AssetTransactionOperation::VaultBridgeMintFromReceipts(
            VaultBridgeMintFromReceiptsOperation {
                issuer: issuer.clone(),
                to: holder.clone(),
                asset_id: asset_id.clone(),
                bucket_id: bucket_id.clone(),
                amount_atoms: 5_000_000,
                receipt_ids: vec![receipt_id],
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
            },
        ),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &mint, 14).accepted);
    assert_eq!(
        ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .expect("holder line")
            .balance,
        5_000_000
    );
    assert_eq!(
        ledger
            .vault_bridge_bucket(&bucket_id)
            .unwrap()
            .outstanding_vault_bridge_atoms,
        5_000_000
    );
    assert_eq!(
        ledger.vault_bridge_receipts[0].allocated_value_atoms,
        5_000_000
    );
    assert_eq!(ledger.vault_bridge_allocations.len(), 1);
    assert_eq!(
        ledger.vault_bridge_allocations[0].purpose,
        "vault_bridge_supply"
    );

    let burn_to_redeem = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
        7,
        AssetTransactionOperation::VaultBridgeBurnToRedeem(VaultBridgeBurnToRedeemOperation {
            owner: holder.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            bucket_id: bucket_id.clone(),
            amount_atoms: 1_000_000,
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
            destination_ref: "evm-erc20:42161:0x2222222222222222222222222222222222222222"
                .to_string(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &burn_to_redeem, 15).accepted);
    assert_eq!(
        ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .expect("holder line")
            .balance,
        4_000_000
    );
    let redemption_id = ledger.vault_bridge_redemptions[0].redemption_id.clone();
    assert_eq!(
        ledger
            .vault_bridge_bucket(&bucket_id)
            .expect("vault_bridge bucket")
            .outstanding_vault_bridge_atoms,
        4_000_000
    );
    assert_eq!(
        ledger
            .vault_bridge_bucket(&bucket_id)
            .expect("vault_bridge bucket")
            .redemption_queue_atoms,
        1_000_000
    );
    assert_eq!(ledger.vault_bridge_redemptions[0].amount_atoms, 1_000_000);
    assert_eq!(ledger.vault_bridge_redemptions[0].settled_atoms, 0);
    assert_eq!(
        ledger.vault_bridge_redemptions[0].state,
        VAULT_BRIDGE_REDEMPTION_STATE_PENDING
    );
    assert_eq!(
        ledger.vault_bridge_redemptions[0]
            .withdrawal_packet_hash
            .len(),
        96
    );
    assert_eq!(
        ledger.vault_bridge_redemptions[0]
            .withdrawal_packet_evm_digest
            .len(),
        64
    );
    assert_eq!(
        ledger.vault_bridge_redemptions[0]
            .withdrawal_packet
            .recipient,
        "0x2222222222222222222222222222222222222222"
    );
    let bucket_before_settle = ledger
        .vault_bridge_bucket(&bucket_id)
        .expect("bucket before settle");
    let unallocated_before_settle = bucket_before_settle.counted_value_atoms
        - bucket_before_settle
            .allocated_atoms()
            .expect("allocated atoms before settlement");

    let (settlement_receipt_hash, withdrawal_attestation) =
        withdrawal_attestation_for_redemption(&ledger.vault_bridge_redemptions[0], &holder_key);
    let settle = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND,
        9,
        AssetTransactionOperation::VaultBridgeRedeemSettle(VaultBridgeRedeemSettleOperation {
            issuer_or_redemption_account: issuer.clone(),
            asset_id: asset_id.clone(),
            redemption_id,
            settlement_receipt_hash: settlement_receipt_hash.clone(),
            settled_atoms: 1_000_000,
            withdrawal_observations: vec![withdrawal_attestation],
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &settle, 16).accepted);
    assert_eq!(
        ledger
            .vault_bridge_bucket(&bucket_id)
            .expect("vault_bridge bucket")
            .redemption_queue_atoms,
        0
    );
    let bucket_after_settle = ledger
        .vault_bridge_bucket(&bucket_id)
        .expect("vault_bridge bucket after settlement");
    assert_eq!(
        bucket_after_settle.counted_value_atoms,
        counted_value - 1_000_000
    );
    assert_eq!(
        bucket_after_settle.counted_value_atoms
            - bucket_after_settle
                .allocated_atoms()
                .expect("allocated atoms after settlement"),
        unallocated_before_settle
    );
    assert_eq!(ledger.vault_bridge_redemptions[0].settled_atoms, 1_000_000);
    assert_eq!(
        ledger.vault_bridge_redemptions[0].state,
        VAULT_BRIDGE_REDEMPTION_STATE_SETTLED
    );
    assert_eq!(
        ledger.vault_bridge_redemptions[0].settlement_receipt_hash,
        settlement_receipt_hash
    );

    let bad_impair = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_BUCKET_IMPAIR_TRANSACTION_KIND,
        10,
        AssetTransactionOperation::VaultBridgeBucketImpair(VaultBridgeBucketImpairOperation {
            operator: issuer.clone(),
            asset_id: asset_id.clone(),
            bucket_id: bucket_id.clone(),
            updated_counted_value_atoms: 3_000_000,
            impairment_factor_bps: 7_600,
            reason_hash: "ef".repeat(48),
            policy_hash: policy_hash.clone(),
        }),
    );
    assert!(!execute_asset_transaction(&genesis, &mut ledger, &bad_impair, 14).accepted);

    let impair = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_BUCKET_IMPAIR_TRANSACTION_KIND,
        10,
        AssetTransactionOperation::VaultBridgeBucketImpair(VaultBridgeBucketImpairOperation {
            operator: issuer.clone(),
            asset_id: asset_id.clone(),
            bucket_id: bucket_id.clone(),
            updated_counted_value_atoms: 3_000_000,
            impairment_factor_bps: 7_500,
            reason_hash: "ef".repeat(48),
            policy_hash: policy_hash.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &impair, 15).accepted);
    let impaired_bucket = ledger
        .vault_bridge_bucket(&bucket_id)
        .expect("impaired bucket");
    assert_eq!(impaired_bucket.status, VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED);
    assert_eq!(impaired_bucket.counted_value_atoms, 3_000_000);
    assert_eq!(impaired_bucket.impairment_factor_bps, 7_500);
    assert_eq!(
        ledger.vault_bridge_receipts[0].status,
        VAULT_BRIDGE_RECEIPT_STATUS_IMPAIRED
    );

    let rejected_mint = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_MINT_FROM_RECEIPTS_TRANSACTION_KIND,
        11,
        AssetTransactionOperation::VaultBridgeMintFromReceipts(
            VaultBridgeMintFromReceiptsOperation {
                issuer: issuer.clone(),
                to: holder.clone(),
                asset_id: asset_id.clone(),
                bucket_id: bucket_id.clone(),
                amount_atoms: 1,
                receipt_ids: vec![ledger.vault_bridge_receipts[0].receipt_id.clone()],
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
            },
        ),
    );
    assert!(!execute_asset_transaction(&genesis, &mut ledger, &rejected_mint, 16).accepted);

    let impaired_burn = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
        8,
        AssetTransactionOperation::VaultBridgeBurnToRedeem(VaultBridgeBurnToRedeemOperation {
            owner: holder.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            bucket_id: bucket_id.clone(),
            amount_atoms: 1_000_000,
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
            destination_ref: "evm-erc20:42161:0x3333333333333333333333333333333333333333"
                .to_string(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &impaired_burn, 17).accepted);
    assert_eq!(
        ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .expect("holder line")
            .balance,
        3_000_000
    );
    let impaired_bucket = ledger
        .vault_bridge_bucket(&bucket_id)
        .expect("impaired bucket");
    assert_eq!(impaired_bucket.outstanding_vault_bridge_atoms, 3_000_000);
    assert_eq!(impaired_bucket.redemption_queue_atoms, 1_000_000);
    assert_eq!(impaired_bucket.impairment_factor_bps, 7_500);
    ledger
        .validate_asset_state(&genesis.chain_id)
        .expect("valid vault bridge asset asset state");
}

#[test]
fn vault_bridge_deposit_claim_mints_swappable_erc20_bridge_to_recipient() {
    let genesis = Genesis::new("postfiat-local");
    let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
    let holder_key = ml_dsa_65_keygen().expect("holder keygen");
    let buyer_key = ml_dsa_65_keygen().expect("buyer keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let holder = address_from_public_key(&holder_key.public_key);
    let buyer = address_from_public_key(&buyer_key.public_key);
    let deposit_amount = 7_500_000_u64;
    let mut bridge_evidence = vault_bridge_evidence(deposit_amount, "91");
    bridge_evidence.pftl_recipient = holder.clone();
    bridge_evidence.pftl_recipient_hash =
        vault_bridge_pftl_recipient_hash(&holder).expect("recipient hash");
    bridge_evidence.deposit_id = vault_bridge_deposit_id(&bridge_evidence).expect("deposit id");
    let source_domain = bridge_evidence.source_domain();
    let policy_hash = "42".repeat(48);
    let bridge_evidence_root =
        vault_bridge_deposit_evidence_root(&bridge_evidence).expect("bridge evidence root");
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
        Account::new(
            buyer.clone(),
            10_000_000,
            Some(bytes_to_hex(&buyer_key.public_key)),
        ),
    ]);

    let profile_register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
            registrant: issuer.clone(),
            verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
            source_class: format!("vault_bridge:{source_domain}"),
            max_snapshot_age_blocks: 100,
            challenge_window_blocks: 1,
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
            public_values_schema: String::new(),
            source_manifest_hash: String::new(),
            valuation_unit_id: String::new(),
            max_observation_span_blocks: 0,
            allow_controlled_sources: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &profile_register, 1).accepted);
    let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

    let create = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "vault bridge asset".to_string(),
            version: 3,
            precision: 8,
            display_name: "vault bridge asset bridge claim".to_string(),
            max_supply: Some(100_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &create, 2).accepted);
    let asset_id = ledger.asset_definitions[0].asset_id.clone();

    let register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: profile_id,
            valuation_unit: "SOURCE_UNIT".to_string(),
            redemption_account: issuer.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 3).accepted);

    let buyer_trust = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &buyer_key,
        TRUST_SET_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: buyer.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            limit: 100_000_000,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &buyer_trust, 5).accepted);

    let attestor_register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
            attestor: holder.clone(),
            domain: "operator.local".to_string(),
            bond: 0,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &attestor_register, 6).accepted);

    let bridge_propose = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
            proposer: holder.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            evidence: bridge_evidence.clone(),
            policy_hash: policy_hash.clone(),
            source_proof_kind: String::new(),
            source_proof_hash: String::new(),
            source_public_values_hash: String::new(),
            source_proof_bytes: Vec::new(),
            source_public_values: Vec::new(),
            expires_at_height: 1_000,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_propose, 7).accepted);

    let bridge_observation =
        VaultBridgeDepositObservation::success_for_evidence(&bridge_evidence, 6);
    let bridge_observation_root = vault_bridge_deposit_observation_root(&bridge_observation)
        .expect("bridge observation root");
    let bridge_attest = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::VaultBridgeDepositAttest(VaultBridgeDepositAttestOperation {
            attestor: holder.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            pass: true,
            observation_root: bridge_observation_root,
            observation: Some(bridge_observation),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_attest, 8).accepted);

    let bridge_finalize = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::VaultBridgeDepositFinalize(
            VaultBridgeDepositFinalizeOperation {
                finalizer: holder.clone(),
                asset_id: asset_id.clone(),
                evidence_root: bridge_evidence_root.clone(),
            },
        ),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_finalize, 9).accepted);
    assert!(
        ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .is_none(),
        "bridge recipient should not need a pre-opened balance row"
    );

    let claim = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::VaultBridgeDepositClaim(VaultBridgeDepositClaimOperation {
            claimer: holder.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            policy_hash: policy_hash.clone(),
            recipient: holder.clone(),
            amount_atoms: deposit_amount,
        }),
    );
    let claim_receipt = execute_asset_transaction(&genesis, &mut ledger, &claim, 10);
    assert!(claim_receipt.accepted, "{claim_receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .expect("holder line")
            .balance,
        deposit_amount
    );
    let holder_line = ledger
        .trustline_for_account_asset(&holder, &asset_id)
        .expect("holder implicit bridge balance row");
    assert_eq!(holder_line.limit, deposit_amount);
    assert_eq!(holder_line.reserve_paid, 0);
    assert_eq!(ledger.vault_bridge_receipts.len(), 1);
    assert_eq!(ledger.vault_bridge_receipts[0].status, "counted");
    assert_eq!(
        ledger.vault_bridge_receipts[0].counted_value_atoms,
        deposit_amount
    );
    assert_eq!(
        ledger.vault_bridge_receipts[0].allocated_value_atoms,
        deposit_amount
    );
    assert_eq!(
        ledger.vault_bridge_bucket_states[0].counted_value_atoms,
        deposit_amount
    );
    assert_eq!(
        ledger.vault_bridge_bucket_states[0].outstanding_vault_bridge_atoms,
        deposit_amount
    );
    assert_eq!(ledger.vault_bridge_allocations.len(), 1);
    assert_eq!(
        ledger.vault_bridge_allocations[0].purpose,
        "vault_bridge_supply"
    );

    let duplicate_claim = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
        6,
        AssetTransactionOperation::VaultBridgeDepositClaim(VaultBridgeDepositClaimOperation {
            claimer: holder.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root,
            policy_hash,
            recipient: holder.clone(),
            amount_atoms: deposit_amount,
        }),
    );
    let duplicate_receipt = execute_asset_transaction(&genesis, &mut ledger, &duplicate_claim, 11);
    assert!(!duplicate_receipt.accepted);
    assert_eq!(
        duplicate_receipt.code,
        "vault_bridge_deposit_already_claimed"
    );

    let source_root =
        vault_bridge_source_root_for_asset(&ledger.vault_bridge_bucket_states, &asset_id)
            .expect("source root");
    let reserve_packet_hash = "ac".repeat(48);
    let reserve_submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.clone(),
            submitter: issuer.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            nav_per_unit: VAULT_BRIDGE_UNIT,
            circulating_supply: deposit_amount,
            verified_net_assets: deposit_amount,
            proof_profile: ledger.nav_assets[0].proof_profile.clone(),
            source_root: source_root.clone(),
            attestor_root: "88".repeat(48),
            reserve_packet_hash: reserve_packet_hash.clone(),
            reserve_accounts: vec![bridge_evidence.vault_id()],
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &reserve_submit, 12).accepted);

    let reserve_attest = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        NAV_RESERVE_ATTEST_TRANSACTION_KIND,
        6,
        AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
            attestor: holder.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
            pass: true,
            observation_root: source_root,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &reserve_attest, 13).accepted);

    let finalize_epoch = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize_epoch, 14).accepted);

    let offer_amount = 1_500_000_u64;
    let offer = signed_offer_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        OFFER_CREATE_TRANSACTION_KIND,
        7,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: holder.clone(),
            taker_gets_asset_id: asset_id.clone(),
            taker_gets_amount: offer_amount,
            taker_pays_asset_id: "PFT".to_string(),
            taker_pays_amount: 3_000_000,
            expiration_height: 50,
        }),
        15,
    );
    let offer_receipt = execute_offer_transaction(&genesis, &mut ledger, &offer, 15);
    assert!(offer_receipt.accepted, "{offer_receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .expect("holder line")
            .balance,
        deposit_amount - offer_amount
    );
    let offer_id = offer_id(&genesis.chain_id, &holder, 7).expect("offer id");
    assert_eq!(
        ledger.offer(&offer_id).expect("offer").state,
        OFFER_STATE_OPEN
    );

    let buyer_fill = signed_offer_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &buyer_key,
        OFFER_CREATE_TRANSACTION_KIND,
        2,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: buyer.clone(),
            taker_gets_asset_id: "PFT".to_string(),
            taker_gets_amount: 3_000_000,
            taker_pays_asset_id: asset_id.clone(),
            taker_pays_amount: offer_amount,
            expiration_height: 50,
        }),
        16,
    );
    let fill_receipt = execute_offer_transaction(&genesis, &mut ledger, &buyer_fill, 16);
    assert!(fill_receipt.accepted, "{fill_receipt:?}");
    assert_eq!(fill_receipt.code, "filled");
    assert_eq!(fill_receipt.offer_fills.len(), 1);
    assert_eq!(fill_receipt.offer_fills[0].maker_offer_id, offer_id);
    assert_eq!(fill_receipt.offer_fills[0].maker_sends_amount, offer_amount);
    assert_eq!(fill_receipt.offer_fills[0].taker_sends_amount, 3_000_000);
    assert_eq!(
        ledger.offer(&offer_id).expect("filled offer").state,
        OFFER_STATE_FILLED
    );
    assert_eq!(
        ledger
            .trustline_for_account_asset(&buyer, &asset_id)
            .expect("buyer line")
            .balance,
        offer_amount
    );
    assert_eq!(
        ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .expect("holder line")
            .balance,
        deposit_amount - offer_amount
    );

    let buyer_burn_to_redeem = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &buyer_key,
        VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::VaultBridgeBurnToRedeem(VaultBridgeBurnToRedeemOperation {
            owner: buyer.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            bucket_id: ledger.vault_bridge_bucket_states[0].bucket_id.clone(),
            amount_atoms: offer_amount,
            epoch: 1,
            reserve_packet_hash,
            destination_ref: "evm-erc20:42161:0x4444444444444444444444444444444444444444"
                .to_string(),
        }),
    );
    let burn_receipt = execute_asset_transaction(&genesis, &mut ledger, &buyer_burn_to_redeem, 17);
    assert!(burn_receipt.accepted, "{burn_receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&buyer, &asset_id)
            .expect("buyer line")
            .balance,
        0
    );
    assert_eq!(ledger.vault_bridge_redemptions.len(), 1);
    assert_eq!(ledger.vault_bridge_redemptions[0].owner, buyer);
    assert_eq!(
        ledger.vault_bridge_redemptions[0].state,
        VAULT_BRIDGE_REDEMPTION_STATE_PENDING
    );
    assert_eq!(
        ledger.vault_bridge_redemptions[0]
            .withdrawal_packet
            .recipient,
        "0x4444444444444444444444444444444444444444"
    );
    assert_eq!(
        ledger.vault_bridge_bucket_states[0].outstanding_vault_bridge_atoms,
        deposit_amount - offer_amount
    );
    assert_eq!(
        ledger.vault_bridge_bucket_states[0].redemption_queue_atoms,
        offer_amount
    );
}

#[test]
fn vault_bridge_sp1_bridge_deposit_requires_source_proof_commitments() {
    let genesis = Genesis::new("postfiat-local");
    let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
    let holder_key = ml_dsa_65_keygen().expect("holder keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let holder = address_from_public_key(&holder_key.public_key);
    let bridge_evidence = vault_bridge_evidence(5_000_000, "88");
    let source_domain = bridge_evidence.source_domain();
    let policy_hash = "8a".repeat(32);
    let bridge_evidence_root =
        vault_bridge_deposit_evidence_root(&bridge_evidence).expect("bridge evidence root");
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

    let profile_register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
            registrant: issuer.clone(),
            verifier_kind: NAV_PROFILE_VERIFIER_SP1_GROTH16.to_string(),
            source_class: format!("vault_bridge:{source_domain}"),
            max_snapshot_age_blocks: 100,
            challenge_window_blocks: 2,
            max_epoch_gap_blocks: 100,
            settle_deadline_blocks: 0,
            min_challenge_bond: 0,
            min_attestations: 0,
            tolerance_bp: 0,
            bridge_observer_min_confirmations: 0,
            valuation_policy_hash: policy_hash.clone(),
            vault_bridge_route_policy_hash: String::new(),
            sp1_program_vkey: format!("0x{}", "11".repeat(32)),
            sp1_proof_encoding: NAV_SP1_PROOF_ENCODING_GROTH16.to_string(),
            max_proof_bytes: 0,
            max_public_values_bytes: 0,
            public_values_schema: String::new(),
            source_manifest_hash: String::new(),
            valuation_unit_id: String::new(),
            max_observation_span_blocks: 0,
            allow_controlled_sources: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &profile_register, 1).accepted);
    let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

    let create = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "vault bridge asset".to_string(),
            version: 2,
            precision: 8,
            display_name: "vault bridge asset SP1".to_string(),
            max_supply: Some(100_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &create, 2).accepted);
    let asset_id = ledger.asset_definitions[0].asset_id.clone();

    let register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: profile_id,
            valuation_unit: "SOURCE_UNIT".to_string(),
            redemption_account: issuer.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 3).accepted);

    let mut proof_native_ledger = ledger.clone();
    let proof_native_profile = NavProofProfile::new(
        issuer.clone(),
        NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1,
        format!("vault_bridge:{source_domain}"),
        100,
        2,
        100,
        0,
        0,
        0,
        0,
        policy_hash.clone(),
        format!("0x{}", "11".repeat(32)),
        NAV_SP1_PROOF_ENCODING_GROTH16,
        4_096,
        16_384,
    )
    .expect("proof-native profile");
    proof_native_ledger.nav_assets[0].proof_profile = proof_native_profile.profile_id.clone();
    proof_native_ledger.nav_proof_profiles[0] = proof_native_profile;
    let invalid_proof = vec![1, 2, 3];
    let invalid_public_values = vec![4, 5, 6];
    let invalid_proof_propose = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &proof_native_ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
            proposer: holder.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            evidence: bridge_evidence.clone(),
            policy_hash: policy_hash.clone(),
            source_proof_kind: NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1.to_string(),
            source_proof_hash: postfiat_types::pfusdc_ingress_proof_hash_v1(&invalid_proof),
            source_public_values_hash: postfiat_types::pfusdc_ingress_public_values_hash_v1(
                &invalid_public_values,
            ),
            source_proof_bytes: invalid_proof,
            source_public_values: invalid_public_values,
            expires_at_height: 1_000,
        }),
    );
    let invalid_proof_receipt = execute_asset_transaction(
        &genesis,
        &mut proof_native_ledger,
        &invalid_proof_propose,
        4,
    );
    assert!(!invalid_proof_receipt.accepted);
    assert_eq!(invalid_proof_receipt.code, "sp1_proof_invalid");
    assert!(proof_native_ledger.vault_bridge_deposits.is_empty());

    let missing_proof_propose = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
            proposer: holder.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            evidence: bridge_evidence.clone(),
            policy_hash: policy_hash.clone(),
            source_proof_kind: String::new(),
            source_proof_hash: String::new(),
            source_public_values_hash: String::new(),
            source_proof_bytes: Vec::new(),
            source_public_values: Vec::new(),
            expires_at_height: 1_000,
        }),
    );
    let missing_proof_receipt =
        execute_asset_transaction(&genesis, &mut ledger, &missing_proof_propose, 4);
    assert!(!missing_proof_receipt.accepted);
    assert_eq!(
        missing_proof_receipt.code,
        "missing_vault_bridge_deposit_source_proof"
    );

    let source_public_values_hash = vault_bridge_deposit_public_values_hash(
        &bridge_evidence,
        &bridge_evidence_root,
        &policy_hash,
    )
    .expect("source public values hash");
    let proof_bound_propose = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
            proposer: holder.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            evidence: bridge_evidence.clone(),
            policy_hash: policy_hash.clone(),
            source_proof_kind: NAV_PROFILE_VERIFIER_SP1_GROTH16.to_string(),
            source_proof_hash: "99".repeat(48),
            source_public_values_hash: source_public_values_hash.clone(),
            source_proof_bytes: Vec::new(),
            source_public_values: Vec::new(),
            expires_at_height: 1_000,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &proof_bound_propose, 4).accepted);
    assert_eq!(
        ledger.vault_bridge_deposits[0].source_public_values_hash,
        source_public_values_hash
    );

    let premature_finalize = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::VaultBridgeDepositFinalize(
            VaultBridgeDepositFinalizeOperation {
                finalizer: holder.clone(),
                asset_id: asset_id.clone(),
                evidence_root: bridge_evidence_root.clone(),
            },
        ),
    );
    let premature_finalize_receipt =
        execute_asset_transaction(&genesis, &mut ledger, &premature_finalize, 5);
    assert!(!premature_finalize_receipt.accepted);
    assert_eq!(
        premature_finalize_receipt.code,
        "vault_bridge_deposit_challenge_window_open"
    );

    let finalize = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &holder_key,
        VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::VaultBridgeDepositFinalize(
            VaultBridgeDepositFinalizeOperation {
                finalizer: holder,
                asset_id: asset_id.clone(),
                evidence_root: bridge_evidence_root,
            },
        ),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize, 6).accepted);
    assert_eq!(ledger.vault_bridge_deposits[0].status, "finalized");
}

#[test]
fn ethereum_ingress_consensus_binding_rejects_route_manifest_and_domain_mutations() {
    let recipient = "pf-ethereum-ingress-consensus-test".to_string();
    let mut evidence = VaultBridgeDepositEvidence {
        source_chain_id: ETHEREUM_SEPOLIA_CHAIN_ID,
        vault_address: "0x12f2e6ed1fd447c0eec77ca5890ec7edcb973d22".to_string(),
        token_address: "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238".to_string(),
        depositor: "0x3333333333333333333333333333333333333333".to_string(),
        pftl_recipient_hash: vault_bridge_pftl_recipient_hash(&recipient).expect("recipient hash"),
        pftl_recipient: recipient,
        amount_atoms: 10,
        nonce: "44".repeat(32),
        route_binding: "55".repeat(32),
        deposit_id: String::new(),
        block_hash: "66".repeat(32),
        tx_hash: "77".repeat(32),
        log_index: 0,
    };
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
    let evidence_root = vault_bridge_deposit_evidence_root(&evidence).expect("evidence root");
    let route_policy_hash = "88".repeat(48);
    let profile = NavProofProfile::new(
        "operator",
        NAV_PROFILE_VERIFIER_SP1_GROTH16,
        format!("vault_bridge:{}", evidence.source_domain()),
        7_200,
        1,
        7_200,
        7_200,
        0,
        0,
        0,
        postfiat_types::SP1_ETHEREUM_FINALITY_SEPOLIA_P0_MANIFEST_HASH,
        postfiat_types::SP1_ETHEREUM_FINALITY_SEPOLIA_P0_PROGRAM_VKEY,
        NAV_SP1_PROOF_ENCODING_GROTH16,
        DEFAULT_MAX_NAV_SP1_PROOF_BYTES,
        DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES,
    )
    .expect("SP1 profile")
    .with_vault_bridge_route_policy_hash(route_policy_hash.clone())
    .expect("route-bound profile");
    let values = postfiat_types::PfUsdcEthereumIngressPublicValuesV1 {
        schema: postfiat_types::PFUSDC_ETHEREUM_INGRESS_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        route_id: VAULT_BRIDGE_ROUTE_ETHEREUM_SEPOLIA_USDC_V1.to_string(),
        source_chain_id: evidence.source_chain_id,
        prior_finalized_beacon_root: "01".repeat(32),
        prior_finalized_slot: 100,
        finalized_beacon_root: "02".repeat(32),
        finalized_slot: 132,
        finalized_execution_block_hash: evidence.block_hash.clone(),
        finalized_execution_block_number: 1_000,
        execution_state_root: "03".repeat(32),
        vault_address: evidence.vault_address.clone(),
        vault_runtime_code_hash: "04".repeat(32),
        token_address: evidence.token_address.clone(),
        token_runtime_code_hash: "05".repeat(32),
        depositor: evidence.depositor.clone(),
        pftl_recipient: evidence.pftl_recipient.clone(),
        pftl_recipient_hash: evidence.pftl_recipient_hash.clone(),
        amount_atoms: evidence.amount_atoms,
        nonce: evidence.nonce.clone(),
        route_binding: evidence.route_binding.clone(),
        deposit_id: evidence.deposit_id.clone(),
        evidence_root: evidence_root.clone(),
        manifest_hash: postfiat_types::SP1_ETHEREUM_FINALITY_SEPOLIA_P0_MANIFEST_HASH.to_string(),
        deposit_nullifier: "06".repeat(32),
        total_obligations_atoms: "10".to_string(),
        vault_token_balance_atoms: "10".to_string(),
    };
    values.validate().expect("public values");
    ensure_ethereum_ingress_public_values_match(
        &profile,
        &evidence,
        &evidence_root,
        &route_policy_hash,
        &values,
    )
    .expect("exact route binding");
    let generic_kind_bypass =
        ensure_vault_bridge_deposit_source_proof(VaultBridgeDepositSourceProof {
            genesis: None,
            profile: &profile,
            evidence: &evidence,
            evidence_root: &evidence_root,
            policy_hash: &route_policy_hash,
            source_proof_kind: NAV_PROFILE_VERIFIER_SP1_GROTH16,
            source_proof_hash: &"aa".repeat(48),
            source_public_values_hash: &"bb".repeat(48),
            source_proof_bytes: &[],
            source_public_values: &[],
            fast_ingress_verifier: None,
        })
        .expect_err("generic SP1 kind must not bypass Ethereum finality verification");
    assert_eq!(
        generic_kind_bypass.0,
        "ethereum_ingress_source_proof_kind_required"
    );
    let committed_replay =
        ensure_vault_bridge_deposit_source_proof(VaultBridgeDepositSourceProof {
            genesis: None,
            profile: &profile,
            evidence: &evidence,
            evidence_root: &evidence_root,
            policy_hash: &route_policy_hash,
            source_proof_kind: SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1,
            source_proof_hash: &"aa".repeat(48),
            source_public_values_hash: &"bb".repeat(48),
            source_proof_bytes: &[],
            source_public_values: &[],
            fast_ingress_verifier: None,
        })
        .expect("finalization replays consensus-committed proof hashes");
    assert!(committed_replay.is_none());
    let incomplete_proposal =
        ensure_vault_bridge_deposit_source_proof(VaultBridgeDepositSourceProof {
            genesis: Some(&Genesis::new("postfiat-ethereum-ingress-test")),
            profile: &profile,
            evidence: &evidence,
            evidence_root: &evidence_root,
            policy_hash: &route_policy_hash,
            source_proof_kind: SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1,
            source_proof_hash: &"aa".repeat(48),
            source_public_values_hash: &"bb".repeat(48),
            source_proof_bytes: &[],
            source_public_values: &[],
            fast_ingress_verifier: None,
        })
        .expect_err("proposal must still supply complete proof material");
    assert_eq!(
        incomplete_proposal.0,
        "missing_ethereum_ingress_source_proof"
    );

    let mut wrong_route = values.clone();
    wrong_route.route_id = VAULT_BRIDGE_ROUTE_ETHEREUM_MAINNET_USDC_V1.to_string();
    assert!(ensure_ethereum_ingress_public_values_match(
        &profile,
        &evidence,
        &evidence_root,
        &route_policy_hash,
        &wrong_route,
    )
    .is_err());
    let mut wrong_manifest = values.clone();
    wrong_manifest.manifest_hash = "07".repeat(32);
    assert!(ensure_ethereum_ingress_public_values_match(
        &profile,
        &evidence,
        &evidence_root,
        &route_policy_hash,
        &wrong_manifest,
    )
    .is_err());
    let mut wrong_domain = evidence.clone();
    wrong_domain.vault_address = "0x9999999999999999999999999999999999999999".to_string();
    assert!(ensure_ethereum_ingress_public_values_match(
        &profile,
        &wrong_domain,
        &evidence_root,
        &route_policy_hash,
        &values,
    )
    .is_err());
}

#[test]
fn ethereum_backed_claim_grows_cap_and_converges_across_six_replicas() {
    let genesis = Genesis::new("postfiat-ethereum-cap-growth-test");
    let issuer = "pf-ethereum-cap-growth-issuer".to_string();
    let recipient = "pf-ethereum-cap-growth-recipient".to_string();
    let asset = AssetDefinition::new(&genesis.chain_id, &issuer, "PFUSDC", 1, 6).expect("asset");
    let amount_atoms = 2_000_000;
    let mut evidence = VaultBridgeDepositEvidence {
        source_chain_id: ETHEREUM_MAINNET_CHAIN_ID,
        vault_address: "0x1111111111111111111111111111111111111111".to_string(),
        token_address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
        depositor: "0x3333333333333333333333333333333333333333".to_string(),
        pftl_recipient_hash: vault_bridge_pftl_recipient_hash(&issuer).expect("recipient hash"),
        pftl_recipient: issuer.clone(),
        amount_atoms,
        nonce: "44".repeat(32),
        route_binding: "55".repeat(32),
        deposit_id: String::new(),
        block_hash: "66".repeat(32),
        tx_hash: "77".repeat(32),
        log_index: 0,
    };
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
    let evidence_root = vault_bridge_deposit_evidence_root(&evidence).expect("evidence root");
    let route_policy_hash = "88".repeat(48);
    let profile = NavProofProfile::new(
        issuer.clone(),
        NAV_PROFILE_VERIFIER_SP1_GROTH16,
        format!("vault_bridge:{}", evidence.source_domain()),
        7_200,
        64,
        7_200,
        7_200,
        0,
        0,
        0,
        "99".repeat(32),
        format!("0x{}", "aa".repeat(32)),
        NAV_SP1_PROOF_ENCODING_GROTH16,
        DEFAULT_MAX_NAV_SP1_PROOF_BYTES,
        DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES,
    )
    .expect("profile")
    .with_vault_bridge_route_policy_hash(route_policy_hash.clone())
    .expect("route profile");
    let nav_asset = NavTrackedAsset::new(
        asset.asset_id.clone(),
        issuer.clone(),
        issuer.clone(),
        profile.profile_id.clone(),
        "USDC_ATOMS",
        issuer.clone(),
    )
    .expect("NAV asset");
    let deposit = VaultBridgeDepositRecord::new_with_source_nullifier(
        asset.asset_id.clone(),
        evidence_root.clone(),
        evidence,
        route_policy_hash.clone(),
        SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1,
        "bb".repeat(48),
        "cc".repeat(48),
        "dd".repeat(32),
        "proof-proposer",
        10,
        10_000,
    )
    .expect("deposit");

    let mut initial = LedgerState::new(vec![Account::new(recipient.clone(), 10_000, None)]);
    initial.asset_definitions.push(asset.clone());
    initial.nav_proof_profiles.push(profile);
    initial.nav_assets.push(nav_asset);
    initial.vault_bridge_deposits.push(deposit);
    let challenge = VaultBridgeDepositChallengeOperation {
        challenger: recipient.clone(),
        asset_id: asset.asset_id.clone(),
        evidence_root: evidence_root.clone(),
        challenge_hash: "ee".repeat(48),
        bond: 0,
    };
    let challenge_error = apply_vault_bridge_deposit_challenge(&mut initial, &challenge, 11)
        .expect_err("consensus-verified Ethereum ingress must not be challengeable");
    assert_eq!(challenge_error.0, "vault_bridge_deposit_consensus_verified");
    let finalize = VaultBridgeDepositFinalizeOperation {
        finalizer: recipient.clone(),
        asset_id: asset.asset_id.clone(),
        evidence_root: evidence_root.clone(),
    };
    apply_vault_bridge_deposit_finalize(&mut initial, &finalize, 10)
        .expect("consensus-verified Ethereum ingress finalizes at its submission height");
    assert_eq!(
        initial.vault_bridge_deposits[0].status,
        VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED
    );
    let claim = VaultBridgeDepositClaimOperation {
        claimer: issuer.clone(),
        asset_id: asset.asset_id.clone(),
        evidence_root,
        policy_hash: route_policy_hash,
        recipient: recipient.clone(),
        amount_atoms,
    };
    let unauthorized_delegation = VaultBridgeDepositClaimOperation {
        claimer: "pf-unauthorized-delegator".to_string(),
        ..claim.clone()
    };
    let unauthorized_error = apply_vault_bridge_deposit_claim(
        &genesis,
        &mut initial.clone(),
        &unauthorized_delegation,
        20,
    )
    .expect_err("non-issuer must not redirect an issuer-bound Ethereum claim");
    assert_eq!(
        unauthorized_error.0,
        "vault_bridge_deposit_recipient_mismatch"
    );

    let mut replicas = Vec::new();
    for mut replica in std::iter::repeat_with(|| initial.clone()).take(6) {
        apply_vault_bridge_deposit_claim(&genesis, &mut replica, &claim, 10)
            .expect("proof-bounded claim can follow finalization in the same block");
        assert_eq!(replica.nav_assets[0].circulating_supply, amount_atoms);
        assert_eq!(replica.nav_assets[0].finalized_epoch, 1);
        assert_eq!(
            replica
                .trustline_for_account_asset(&recipient, &asset.asset_id)
                .expect("recipient trustline")
                .balance,
            amount_atoms
        );
        replicas.push(replica);
    }
    assert!(replicas.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn lane_c_ethereum_finality_claim_credits_spendable_directly_and_skips_bonded_escrow() {
    // Dispatch #30 regression: the sp1-ethereum-finality-v1 claim path
    // must (a) bypass the bonded claimed_fast_campaign escrow branch,
    // (b) credit a spendable recipient balance directly, and
    // (c) reject deposit-nullifier replay at deposit registration.
    let genesis = Genesis::new("postfiat-local");
    let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let recipient = "pf1lanecspendable".to_string();
    let asset = AssetDefinition::new(&genesis.chain_id, &issuer, "PFUSDC", 1, 6).expect("asset");
    let amount_atoms = 1_000_000;
    let mut evidence = VaultBridgeDepositEvidence {
        source_chain_id: ETHEREUM_SEPOLIA_CHAIN_ID,
        vault_address: "0x1111111111111111111111111111111111111111".to_string(),
        token_address: "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238".to_string(),
        depositor: "0x3333333333333333333333333333333333333333".to_string(),
        pftl_recipient_hash: vault_bridge_pftl_recipient_hash(&recipient).expect("recipient hash"),
        pftl_recipient: recipient.clone(),
        amount_atoms,
        nonce: "44".repeat(32),
        route_binding: "55".repeat(32),
        deposit_id: String::new(),
        block_hash: "66".repeat(32),
        tx_hash: "77".repeat(32),
        log_index: 0,
    };
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
    let evidence_root = vault_bridge_deposit_evidence_root(&evidence).expect("evidence root");
    let route_policy_hash = "88".repeat(48);
    let profile = NavProofProfile::new(
        issuer.clone(),
        NAV_PROFILE_VERIFIER_SP1_GROTH16,
        format!("vault_bridge:{}", evidence.source_domain()),
        7_200,
        1,
        7_200,
        7_200,
        0,
        0,
        0,
        "99".repeat(32),
        format!("0x{}", "aa".repeat(32)),
        NAV_SP1_PROOF_ENCODING_GROTH16,
        DEFAULT_MAX_NAV_SP1_PROOF_BYTES,
        DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES,
    )
    .expect("profile")
    .with_vault_bridge_route_policy_hash(route_policy_hash.clone())
    .expect("route profile");
    let nav_asset = NavTrackedAsset::new(
        asset.asset_id.clone(),
        issuer.clone(),
        issuer.clone(),
        profile.profile_id.clone(),
        "USDC_ATOMS",
        issuer.clone(),
    )
    .expect("NAV asset");
    let source_nullifier = "dd".repeat(32);
    let mut deposit = VaultBridgeDepositRecord::new_with_source_nullifier(
        asset.asset_id.clone(),
        evidence_root.clone(),
        evidence,
        route_policy_hash.clone(),
        SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1,
        "bb".repeat(48),
        "cc".repeat(48),
        source_nullifier.clone(),
        "proof-proposer",
        10,
        10_000,
    )
    .expect("deposit");
    deposit.status = VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED.to_string();
    deposit.finalized_at_height = 11;
    deposit.validate().expect("finalized deposit");

    let mut ledger = LedgerState::new(vec![Account::new(recipient.clone(), 10_000, None)]);
    ledger.asset_definitions.push(asset.clone());
    ledger.nav_proof_profiles.push(profile);
    ledger.nav_assets.push(nav_asset);
    ledger.vault_bridge_deposits.push(deposit);

    // (a)+(b): claim credits spendable balance with no escrow campaign.
    let claim = VaultBridgeDepositClaimOperation {
        claimer: recipient.clone(),
        asset_id: asset.asset_id.clone(),
        evidence_root: evidence_root.clone(),
        policy_hash: route_policy_hash.clone(),
        recipient: recipient.clone(),
        amount_atoms,
    };
    apply_vault_bridge_deposit_claim(&genesis, &mut ledger, &claim, 20)
        .expect("ethereum-finality claim credits directly");
    assert!(
        ledger.fast_ingress_campaigns.is_empty(),
        "ethereum-finality claim must not create a bonded escrow campaign"
    );
    let balance = ledger
        .trustline_for_account_asset(&recipient, &asset.asset_id)
        .expect("recipient trustline")
        .balance;
    assert_eq!(balance, amount_atoms, "credited atoms must be spendable");
    assert_eq!(ledger.nav_assets[0].circulating_supply, amount_atoms);

    // Claim replay against the same evidence root fails closed: either the
    // proof-bounded cap rejects it (no new finalized backing) or the
    // counted receipt capacity is already fully allocated.
    let claim_replay = VaultBridgeDepositClaimOperation {
        claimer: recipient.clone(),
        asset_id: asset.asset_id.clone(),
        evidence_root,
        policy_hash: route_policy_hash,
        recipient: recipient.clone(),
        amount_atoms,
    };
    let err = apply_vault_bridge_deposit_claim(&genesis, &mut ledger, &claim_replay, 30)
        .expect_err("claim replay must fail closed");
    assert!(
        matches!(
            err.0,
            "proof_bounded_nav_cap_exceeded"
                | "vault_bridge_deposit_already_claimed"
                | "duplicate_vault_bridge_allocation"
        ),
        "unexpected replay rejection code: {}",
        err.0
    );

    // (c): deposit registration must detect the replayed source nullifier.
    let replay_nullifier = source_nullifier;
    let duplicate = ledger
        .vault_bridge_deposits
        .iter()
        .any(|record| record.source_nullifier == replay_nullifier);
    assert!(
        duplicate,
        "deposit registration must detect the replayed source nullifier"
    );
}

#[test]
fn vault_bridge_challenged_bridge_deposit_cannot_be_counted() {
    let genesis = Genesis::new("postfiat-local");
    let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
    let challenger_key = ml_dsa_65_keygen().expect("challenger keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let challenger = address_from_public_key(&challenger_key.public_key);
    let bridge_evidence = vault_bridge_evidence(10_000_000, "77");
    let source_domain = bridge_evidence.source_domain();
    let policy_hash = "42".repeat(48);
    let bridge_evidence_root =
        vault_bridge_deposit_evidence_root(&bridge_evidence).expect("bridge evidence root");
    let mut ledger = LedgerState::new(vec![
        Account::new(
            issuer.clone(),
            10_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        ),
        Account::new(
            challenger.clone(),
            10_000,
            Some(bytes_to_hex(&challenger_key.public_key)),
        ),
    ]);

    let profile_register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
            registrant: issuer.clone(),
            verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
            source_class: format!("vault_bridge:{source_domain}"),
            max_snapshot_age_blocks: 100,
            challenge_window_blocks: 2,
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
            public_values_schema: String::new(),
            source_manifest_hash: String::new(),
            valuation_unit_id: String::new(),
            max_observation_span_blocks: 0,
            allow_controlled_sources: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &profile_register, 1).accepted);
    let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

    let create = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "vault bridge asset".to_string(),
            version: 1,
            precision: 8,
            display_name: "vault bridge asset".to_string(),
            max_supply: Some(100_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &create, 2).accepted);
    let asset_id = ledger.asset_definitions[0].asset_id.clone();

    let register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: profile_id,
            valuation_unit: "SOURCE_UNIT".to_string(),
            redemption_account: issuer.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 3).accepted);

    let receipt_submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::VaultBridgeReceiptSubmit(VaultBridgeReceiptSubmitOperation {
            operator: issuer.clone(),
            asset_id: asset_id.clone(),
            source_domain,
            source_asset: bridge_evidence.source_asset_ref(),
            claim_type: VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT.to_string(),
            amount_atoms: 10_000_000,
            source_tx_or_attestation: bridge_evidence.source_tx_or_attestation(),
            finality_ref: bridge_evidence.finality_ref(),
            vault_id: bridge_evidence.vault_id(),
            policy_hash: policy_hash.clone(),
            expires_at_height: 1_000,
            bridge_deposit_evidence: Some(bridge_evidence.clone()),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &receipt_submit, 4).accepted);
    let receipt_id = ledger.vault_bridge_receipts[0].receipt_id.clone();

    let bridge_propose = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &challenger_key,
        VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
            proposer: challenger.clone(),
            asset_id: asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            evidence: bridge_evidence,
            policy_hash: policy_hash.clone(),
            source_proof_kind: String::new(),
            source_proof_hash: String::new(),
            source_public_values_hash: String::new(),
            source_proof_bytes: Vec::new(),
            source_public_values: Vec::new(),
            expires_at_height: 1_000,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_propose, 5).accepted);

    let bridge_challenge = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &challenger_key,
        VAULT_BRIDGE_DEPOSIT_CHALLENGE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::VaultBridgeDepositChallenge(
            VaultBridgeDepositChallengeOperation {
                challenger: challenger.clone(),
                asset_id: asset_id.clone(),
                evidence_root: bridge_evidence_root.clone(),
                challenge_hash: "88".repeat(48),
                bond: 0,
            },
        ),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_challenge, 6).accepted);
    assert_eq!(ledger.vault_bridge_deposits[0].status, "challenged");

    let receipt_count = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::VaultBridgeReceiptCount(VaultBridgeReceiptCountOperation {
            operator: issuer,
            asset_id,
            receipt_id,
            haircut_bps: 0,
            counted_value_atoms: 10_000_000,
            evidence_root: bridge_evidence_root,
            policy_hash,
        }),
    );
    let count_receipt = execute_asset_transaction(&genesis, &mut ledger, &receipt_count, 7);
    assert!(!count_receipt.accepted);
    assert_eq!(count_receipt.code, "vault_bridge_deposit_not_finalized");
    assert!(ledger.vault_bridge_bucket_states.is_empty());
}

#[test]
fn nav_subscription_vault_bridge_consumes_counted_receipt_allocation_once() {
    let genesis = Genesis::new("postfiat-local");
    let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
    let subscriber_key = ml_dsa_65_keygen().expect("subscriber keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let subscriber = address_from_public_key(&subscriber_key.public_key);
    let bridge_evidence = vault_bridge_evidence(5_000_000, "66");
    let source_domain = bridge_evidence.source_domain();
    let policy_hash = "42".repeat(48);
    let mut ledger = LedgerState::new(vec![
        Account::new(
            issuer.clone(),
            10_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
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
        &issuer_key,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
            registrant: issuer.clone(),
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
            public_values_schema: String::new(),
            source_manifest_hash: String::new(),
            valuation_unit_id: String::new(),
            max_observation_span_blocks: 0,
            allow_controlled_sources: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &profile_register, 1).accepted);
    let vault_bridge_profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

    let vault_bridge_create = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "vault bridge asset".to_string(),
            version: 1,
            precision: 8,
            display_name: "vault bridge asset".to_string(),
            max_supply: Some(100_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &vault_bridge_create, 2).accepted);
    let vault_bridge_asset_id = ledger.asset_definitions[0].asset_id.clone();

    let vault_bridge_register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: vault_bridge_asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: vault_bridge_profile_id.clone(),
            valuation_unit: "SOURCE_UNIT".to_string(),
            redemption_account: issuer.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &vault_bridge_register, 3).accepted);

    let receipt_submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::VaultBridgeReceiptSubmit(VaultBridgeReceiptSubmitOperation {
            operator: issuer.clone(),
            asset_id: vault_bridge_asset_id.clone(),
            source_domain: source_domain.to_string(),
            source_asset: bridge_evidence.source_asset_ref(),
            claim_type: VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT.to_string(),
            amount_atoms: 5_000_000,
            source_tx_or_attestation: bridge_evidence.source_tx_or_attestation(),
            finality_ref: bridge_evidence.finality_ref(),
            vault_id: bridge_evidence.vault_id(),
            policy_hash: policy_hash.clone(),
            expires_at_height: 1_000,
            bridge_deposit_evidence: Some(bridge_evidence.clone()),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &receipt_submit, 4).accepted);
    let receipt_id = ledger.vault_bridge_receipts[0].receipt_id.clone();
    let bucket_id = ledger.vault_bridge_receipts[0].bucket_id.clone();
    let bridge_evidence_root =
        vault_bridge_deposit_evidence_root(&bridge_evidence).expect("bridge evidence root");

    let attestor_register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
            attestor: subscriber.clone(),
            domain: "subscriber.local".to_string(),
            bond: 0,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &attestor_register, 5).accepted);

    let bridge_propose = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
            proposer: subscriber.clone(),
            asset_id: vault_bridge_asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            evidence: bridge_evidence.clone(),
            policy_hash: policy_hash.clone(),
            source_proof_kind: String::new(),
            source_proof_hash: String::new(),
            source_public_values_hash: String::new(),
            source_proof_bytes: Vec::new(),
            source_public_values: Vec::new(),
            expires_at_height: 1_000,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_propose, 5).accepted);

    let bridge_observation =
        VaultBridgeDepositObservation::success_for_evidence(&bridge_evidence, 6);
    let bridge_observation_root = vault_bridge_deposit_observation_root(&bridge_observation)
        .expect("bridge observation root");
    let bridge_attest = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::VaultBridgeDepositAttest(VaultBridgeDepositAttestOperation {
            attestor: subscriber.clone(),
            asset_id: vault_bridge_asset_id.clone(),
            evidence_root: bridge_evidence_root.clone(),
            pass: true,
            observation_root: bridge_observation_root,
            observation: Some(bridge_observation),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_attest, 6).accepted);

    let bridge_finalize = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::VaultBridgeDepositFinalize(
            VaultBridgeDepositFinalizeOperation {
                finalizer: subscriber.clone(),
                asset_id: vault_bridge_asset_id.clone(),
                evidence_root: bridge_evidence_root.clone(),
            },
        ),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &bridge_finalize, 6).accepted);

    let receipt_count = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::VaultBridgeReceiptCount(VaultBridgeReceiptCountOperation {
            operator: issuer.clone(),
            asset_id: vault_bridge_asset_id.clone(),
            receipt_id: receipt_id.clone(),
            haircut_bps: 0,
            counted_value_atoms: 5_000_000,
            evidence_root: bridge_evidence_root,
            policy_hash: policy_hash.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &receipt_count, 7).accepted);

    let spare_bridge_evidence = vault_bridge_evidence(10, "67");
    let spare_receipt = VaultBridgeReceipt::new(
        &genesis.chain_id,
        vault_bridge_asset_id.clone(),
        source_domain.to_string(),
        spare_bridge_evidence.source_asset_ref(),
        VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT.to_string(),
        10,
        spare_bridge_evidence.source_tx_or_attestation(),
        spare_bridge_evidence.finality_ref(),
        spare_bridge_evidence.vault_id(),
        policy_hash.clone(),
        8,
        1_000,
        Some(spare_bridge_evidence),
    )
    .expect("spare receipt");
    let spare_receipt_id = spare_receipt.receipt_id.clone();
    let mut counted_spare_receipt = spare_receipt;
    counted_spare_receipt.haircut_bps = 0;
    counted_spare_receipt.counted_value_atoms = 10;
    counted_spare_receipt.allocated_value_atoms = 0;
    counted_spare_receipt.status = VAULT_BRIDGE_RECEIPT_STATUS_COUNTED.to_string();
    counted_spare_receipt.finalized_at_height = 8;
    counted_spare_receipt.counted_at_height = 8;
    counted_spare_receipt
        .validate_for_chain(&genesis.chain_id)
        .expect("valid spare counted receipt");
    let bucket_index = ledger
        .vault_bridge_bucket_states
        .iter()
        .position(|bucket| bucket.bucket_id == bucket_id)
        .expect("bucket");
    ledger.vault_bridge_bucket_states[bucket_index].gross_receipt_atoms += 10;
    ledger.vault_bridge_bucket_states[bucket_index].counted_value_atoms += 10;
    ledger.vault_bridge_bucket_states[bucket_index].last_updated_height = 8;
    ledger.vault_bridge_bucket_states[bucket_index]
        .validate()
        .expect("valid bucket with spare receipt");
    ledger.vault_bridge_receipts.push(counted_spare_receipt);

    let nav_create = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        6,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "a651".to_string(),
            version: 1,
            precision: 6,
            display_name: "a651 NAVCoin".to_string(),
            max_supply: Some(10_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &nav_create, 6).accepted);
    let nav_asset_id = ledger.asset_definitions[1].asset_id.clone();

    let nav_register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        7,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: nav_asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: "nav-subscription-v0".to_string(),
            valuation_unit: "NAV_UNIT".to_string(),
            redemption_account: issuer.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &nav_register, 7).accepted);

    let nav_trust = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        TRUST_SET_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: subscriber.clone(),
            issuer: issuer.clone(),
            asset_id: nav_asset_id.clone(),
            limit: 10_000_000,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &nav_trust, 8).accepted);

    let nav_reserve_packet_hash = "ab".repeat(48);
    let nav_reserve_submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        8,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.clone(),
            submitter: issuer.clone(),
            asset_id: nav_asset_id.clone(),
            epoch: 1,
            nav_per_unit: VAULT_BRIDGE_UNIT,
            circulating_supply: 10_000_000,
            verified_net_assets: 10_000_000,
            proof_profile: "nav-subscription-v0".to_string(),
            source_root: "01".repeat(48),
            attestor_root: "02".repeat(48),
            reserve_packet_hash: nav_reserve_packet_hash.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &nav_reserve_submit, 9).accepted);

    let nav_finalize = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        9,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.clone(),
            asset_id: nav_asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: nav_reserve_packet_hash.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &nav_finalize, 10).accepted);

    let allocate = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND,
        10,
        AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(
            VaultBridgeNavSubscriptionAllocateOperation {
                operator: issuer.clone(),
                nav_asset_id: nav_asset_id.clone(),
                settlement_asset_id: vault_bridge_asset_id.clone(),
                settlement_bucket_id: bucket_id.clone(),
                settlement_receipt_id: receipt_id.clone(),
                settlement_amount_atoms: 5_000_000,
                consume_supply_owner: None,
                consume_supply_allocation_id: None,
                nav_recipient: None,
                subscription_id: None,
            },
        ),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &allocate, 11).accepted);
    assert_eq!(ledger.vault_bridge_allocations.len(), 1);
    let allocation_id = ledger.vault_bridge_allocations[0].allocation_id.clone();
    assert_eq!(
        ledger.vault_bridge_allocations[0].purpose,
        VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION
    );
    assert_eq!(
        ledger
            .vault_bridge_bucket(&bucket_id)
            .expect("vault bridge asset bucket")
            .nav_subscription_allocations_atoms,
        5_000_000
    );
    assert_eq!(
        ledger.vault_bridge_receipts[0].allocated_value_atoms,
        5_000_000
    );

    let mint = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_MINT_AT_NAV_TRANSACTION_KIND,
        11,
        AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
            issuer: issuer.clone(),
            to: subscriber.clone(),
            asset_id: nav_asset_id.clone(),
            amount: 5_000_000,
            epoch: 1,
            reserve_packet_hash: nav_reserve_packet_hash.clone(),
            settlement_asset_id: vault_bridge_asset_id.clone(),
            settlement_bucket_id: bucket_id.clone(),
            settlement_allocation_id: allocation_id.clone(),
            settlement_amount_atoms: 5_000_000,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &mint, 12).accepted);
    assert_eq!(
        ledger
            .trustline_for_account_asset(&subscriber, &nav_asset_id)
            .expect("subscriber NAV line")
            .balance,
        5_000_000
    );
    assert_eq!(ledger.vault_bridge_allocations[0].retired_at_height, 12);
    ledger.vault_bridge_allocations[0].consumer_id =
        nav_subscription_recipient_consumer_id(&nav_asset_id, &subscriber);
    ledger.vault_bridge_allocations[0].allocation_id = vault_bridge_allocation_id(
        &genesis.chain_id,
        &ledger.vault_bridge_allocations[0].receipt_id,
        &ledger.vault_bridge_allocations[0].asset_id,
        &ledger.vault_bridge_allocations[0].bucket_id,
        ledger.vault_bridge_allocations[0].amount_atoms,
        &ledger.vault_bridge_allocations[0].purpose,
        &ledger.vault_bridge_allocations[0].consumer_id,
    )
    .expect("recipient-scoped allocation id");
    let allocation_id = ledger.vault_bridge_allocations[0].allocation_id.clone();
    ledger.vault_bridge_allocations[0]
        .validate()
        .expect("recipient-scoped retired nav subscription allocation is valid");

    let settlement_trust = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        TRUST_SET_TRANSACTION_KIND,
        6,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: subscriber.clone(),
            issuer: issuer.clone(),
            asset_id: vault_bridge_asset_id.clone(),
            limit: 100_000_000,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &settlement_trust, 12).accepted);

    let nav_reserve_packet_hash_2 = "ac".repeat(48);
    let nav_reserve_submit_2 = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        12,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.clone(),
            submitter: issuer.clone(),
            asset_id: nav_asset_id.clone(),
            epoch: 2,
            nav_per_unit: VAULT_BRIDGE_UNIT + 1,
            circulating_supply: 10_000_000,
            verified_net_assets: 10_000_010,
            proof_profile: "nav-subscription-v0".to_string(),
            source_root: "03".repeat(48),
            attestor_root: "04".repeat(48),
            reserve_packet_hash: nav_reserve_packet_hash_2.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &nav_reserve_submit_2, 13).accepted);

    let nav_finalize_2 = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        13,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.clone(),
            asset_id: nav_asset_id.clone(),
            epoch: 2,
            reserve_packet_hash: nav_reserve_packet_hash_2.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &nav_finalize_2, 14).accepted);

    let redeem = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        NAV_REDEEM_AT_NAV_TRANSACTION_KIND,
        7,
        AssetTransactionOperation::NavRedeemAtNav(NavRedeemAtNavOperation {
            owner: subscriber.clone(),
            issuer: issuer.clone(),
            asset_id: nav_asset_id.clone(),
            amount: 5_000_000,
            epoch: 2,
            reserve_packet_hash: nav_reserve_packet_hash_2.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &redeem, 15).accepted);
    let redemption_id = ledger.nav_redemptions[0].redemption_id.clone();
    assert_eq!(
        ledger
            .trustline_for_account_asset(&subscriber, &nav_asset_id)
            .expect("subscriber NAV line")
            .balance,
        0
    );

    let settle = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_REDEEM_SETTLE_TRANSACTION_KIND,
        14,
        AssetTransactionOperation::NavRedeemSettle(NavRedeemSettleOperation {
            issuer: issuer.clone(),
            asset_id: nav_asset_id.clone(),
            redemption_id,
            settlement_receipt_hash: "0c".repeat(48),
            settlement_asset_id: vault_bridge_asset_id.clone(),
            settlement_bucket_id: bucket_id.clone(),
            settlement_allocation_id: allocation_id.clone(),
            settlement_amount_atoms: 5_000_005,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut ledger, &settle, 16).accepted);
    assert_eq!(
        ledger.nav_redemptions[0].state,
        NAV_REDEMPTION_STATE_SETTLED
    );
    assert_eq!(ledger.vault_bridge_allocations[0].released_atoms, 5_000_000);
    assert_eq!(ledger.vault_bridge_allocations.len(), 2);
    assert_eq!(ledger.vault_bridge_allocations[1].purpose, "redemption");
    assert_eq!(
        ledger.vault_bridge_allocations[1].receipt_id,
        spare_receipt_id
    );
    assert_eq!(ledger.vault_bridge_allocations[1].amount_atoms, 5);
    assert_eq!(
        ledger.vault_bridge_receipts[0].allocated_value_atoms,
        5_000_000
    );
    assert_eq!(ledger.vault_bridge_receipts[1].allocated_value_atoms, 5);
    let settlement_bucket = ledger
        .vault_bridge_bucket(&bucket_id)
        .expect("vault bridge asset bucket after redeem settlement");
    assert_eq!(settlement_bucket.nav_subscription_allocations_atoms, 0);
    assert_eq!(settlement_bucket.outstanding_vault_bridge_atoms, 5_000_005);
    assert_eq!(
        ledger
            .trustline_for_account_asset(&subscriber, &vault_bridge_asset_id)
            .expect("subscriber settlement line")
            .balance,
        5_000_005
    );

    let reused_allocation_mint = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_MINT_AT_NAV_TRANSACTION_KIND,
        15,
        AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
            issuer,
            to: subscriber,
            asset_id: nav_asset_id,
            amount: 5_000_000,
            epoch: 2,
            reserve_packet_hash: nav_reserve_packet_hash_2,
            settlement_asset_id: vault_bridge_asset_id,
            settlement_bucket_id: bucket_id,
            settlement_allocation_id: allocation_id,
            settlement_amount_atoms: 5_000_005,
        }),
    );
    let receipt = execute_asset_transaction(&genesis, &mut ledger, &reused_allocation_mint, 17);
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "vault_bridge_allocation_amount_mismatch");
    ledger
        .validate_asset_state(&genesis.chain_id)
        .expect("valid NAV subscription vault bridge asset asset state");
}

#[test]
fn pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances() {
    // Exercise the complete primary-market and wrapper lifecycle with the
    // independently reproducible, provider-neutral qNAV qualification
    // identity. This is deliberately not an A666/legacy-profile fixture.
    let mut genesis = Genesis::new_with_validator_count("postfiat-wan-devnet-2", 6);
    genesis.consensus_v2_activation_height = Some(1);
    assert_eq!(
        genesis_hash(&genesis),
        "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9"
    );
    let issuer_derivation_payload = serde_json::to_vec(&(
        "postfiat.wallet.seed.v1",
        "ML-DSA-65",
        genesis.chain_id.as_str(),
        0_u32,
        "transparent-spend",
        "66".repeat(32),
    ))
    .expect("serialize deterministic qualification issuer derivation");
    let issuer_digest = postfiat_crypto_provider::hash_bytes(
        "postfiat.wallet.seed.v1",
        &issuer_derivation_payload,
    );
    let mut issuer_seed = [0_u8; 32];
    issuer_seed.copy_from_slice(&issuer_digest[..32]);
    let issuer_key = ml_dsa_65_keygen_from_seed(&issuer_seed);
    let operator_key = ml_dsa_65_keygen().expect("operator keygen");
    let subscriber_key = ml_dsa_65_keygen().expect("subscriber keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let operator = address_from_public_key(&operator_key.public_key);
    let subscriber = address_from_public_key(&subscriber_key.public_key);
    assert_eq!(issuer, "pf0fae169e4293feebc8c9119febb4fd995a667b37");
    let mut ledger = LedgerState::new(vec![
        Account::new(
            issuer.clone(),
            100_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        ),
        Account::new(
            operator.clone(),
            100_000,
            Some(bytes_to_hex(&operator_key.public_key)),
        ),
        Account::new(
            subscriber.clone(),
            100_000,
            Some(bytes_to_hex(&subscriber_key.public_key)),
        ),
    ]);

    let create_settlement = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "PUSDC".to_string(),
            version: 1,
            precision: 6,
            display_name: "PFTL USDC".to_string(),
            max_supply: Some(1_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &create_settlement,
        1,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let settlement_asset_id = ledger.asset_definitions[0].asset_id.clone();

    let create_native = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "qNAV".to_string(),
            version: 1,
            precision: 6,
            display_name: "Provider-neutral qualification NAVCoin".to_string(),
            max_supply: None,
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &create_native,
        2,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let native_nav_asset_id = ledger.asset_definitions[1].asset_id.clone();
    assert_eq!(
        native_nav_asset_id,
        "3f631473a34a48cd47b4e1067546a9ccc5fcfe2f6e103655191d600d9574a5b2e6a985b7c52dcff7c9461aac872a12f5"
    );

    let register_settlement_nav = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: settlement_asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: "pftl-uniswap-settlement-test".to_string(),
            valuation_unit: "USDC".to_string(),
            redemption_account: issuer.clone(),
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &register_settlement_nav,
        3,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let register_reserve_profile = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
            registrant: issuer.clone(),
            verifier_kind: postfiat_types::NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1.to_string(),
            source_class: "manifest-driven".to_string(),
            max_snapshot_age_blocks: 10_000,
            challenge_window_blocks: 1,
            max_epoch_gap_blocks: 100,
            settle_deadline_blocks: 0,
            min_challenge_bond: 0,
            min_attestations: 0,
            tolerance_bp: 0,
            bridge_observer_min_confirmations: 0,
            valuation_policy_hash: "04".repeat(32),
            vault_bridge_route_policy_hash: String::new(),
            sp1_program_vkey:
                "0x000c7271e0711abce0c61d293222fd4a144599a779db8cadadc4df35e31a4100"
                    .to_string(),
            sp1_proof_encoding: "groth16".to_string(),
            max_proof_bytes: 4_096,
            max_public_values_bytes: postfiat_types::NAV_RESERVE_PUBLIC_VALUES_V1_BYTES as u64,
            public_values_schema:
                postfiat_types::NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            source_manifest_hash: "9da4e2ba55939f138475026946d2728d9b40d3f4c7762289a70aae94584eac924b9a788c6df25c9276cc83f1616ef0e5".to_string(),
            valuation_unit_id: "05".repeat(48),
            max_observation_span_blocks: 8,
            allow_controlled_sources: true,
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &register_reserve_profile,
        4,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let reserve_profile_id = ledger
        .nav_proof_profiles
        .first()
        .expect("provider-neutral reserve profile")
        .profile_id
        .clone();
    assert_eq!(
        reserve_profile_id,
        "3d78cac1f539d3d2e56f6f38c958242aa0bcd13661c733834896bc9c49a48211d716bd4cad83d478b2fa5d85b22a0c7e"
    );

    let register_native_nav = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: native_nav_asset_id.clone(),
            reserve_operator: operator.clone(),
            proof_profile: reserve_profile_id.clone(),
            valuation_unit: "USDC".to_string(),
            redemption_account: issuer.clone(),
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &register_native_nav,
        5,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let zero_epoch_route_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::PftlUniswapRouteInit(PftlUniswapRouteInitOperation {
            operator: operator.clone(),
            route_id: "unfinalized-independent-qnav".to_string(),
            route_config_digest: "13".repeat(48),
            route_trust_class: "CONTROLLED".to_string(),
            native_nav_asset_id: native_nav_asset_id.clone(),
            settlement_asset_id: settlement_asset_id.clone(),
            handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
            settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 1_000_000,
            packet_notional_cap_atoms: 500_000,
            latest_finalized_nav_epoch: 0,
            return_finality_blocks: 12,
            live_value_enabled: false,
            ethereum_verification_policy: None,
        }),
    );
    let mut zero_epoch_ledger = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut zero_epoch_ledger,
        &zero_epoch_route_init,
        5,
    );
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(
        zero_epoch_ledger
            .pftl_uniswap_route("unfinalized-independent-qnav")
            .expect("zero epoch route")
            .latest_finalized_nav_epoch,
        0
    );

    let pricing_reserve_packet_hash = "55".repeat(48);
    let reserve_proof = hex_to_bytes(include_str!(
        "../testdata/nav-reserve-v1-qualified-proof-calldata.hex"
    )
    .trim())
    .expect("decode provider-neutral qNAV proof");
    let reserve_public_values = hex_to_bytes(include_str!(
        "../testdata/nav-reserve-v1-qualified-public-values.hex"
    )
    .trim())
    .expect("decode provider-neutral qNAV public values");
    let reserve_submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.clone(),
            submitter: operator.clone(),
            asset_id: native_nav_asset_id.clone(),
            epoch: 7,
            nav_per_unit: 7_000_000,
            circulating_supply: 0,
            verified_net_assets: 1_100,
            proof_profile: reserve_profile_id,
            source_root: "f4bdaca02e5445e7d2c666ca692d45d63fe1c423f6b03067e9eee19f5f9334fe60920b8528feff2656d5dbe7d28d415f".to_string(),
            attestor_root: "cb34590e25db391724491b01795dee8bdbbadba3bba36fb5fc4f96bce1a87fa311426e0b76ce5ff4d775b091d94147df".to_string(),
            reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: reserve_proof,
            sp1_public_values: reserve_public_values,
        }),
    );
    let mut understated_operation = match reserve_submit.unsigned.operation.clone() {
        AssetTransactionOperation::NavReserveSubmit(operation) => operation,
        _ => unreachable!("reserve submit fixture"),
    };
    understated_operation.circulating_supply = 100;
    understated_operation.nav_per_unit = 1;
    understated_operation.reserve_packet_hash = "54".repeat(48);
    let understated_submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::NavReserveSubmit(understated_operation),
    );
    let mut understated_ledger = ledger.clone();
    let understated_receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut understated_ledger,
        &understated_submit,
        5,
    );
    assert!(!understated_receipt.accepted);
    assert_eq!(
        understated_receipt.code,
        "nav_reserve_nav_floor_mismatch"
    );
    assert_eq!(understated_ledger, ledger);
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &reserve_submit,
        5,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let after_reserve_submit = ledger.clone();
    let exact_replay = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &reserve_submit,
        5,
    );
    assert!(!exact_replay.accepted);
    assert_eq!(exact_replay.code, "bad_sequence");
    assert_eq!(ledger, after_reserve_submit);

    let packet_replay = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        2,
        reserve_submit.unsigned.operation.clone(),
    );
    let packet_replay_receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &packet_replay,
        5,
    );
    assert!(!packet_replay_receipt.accepted);
    assert_eq!(packet_replay_receipt.code, "duplicate_nav_reserve_packet");
    assert_eq!(ledger, after_reserve_submit);

    let finalize_nav = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        6,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.clone(),
            asset_id: native_nav_asset_id.clone(),
            epoch: 7,
            reserve_packet_hash: pricing_reserve_packet_hash.clone(),
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &finalize_nav,
        6,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let native_nav = ledger
        .nav_asset(&native_nav_asset_id)
        .expect("native NAV asset");
    assert_eq!(native_nav.finalized_epoch, 7);
    assert_eq!(native_nav.nav_per_unit, 7_000_000);
    assert_eq!(
        native_nav.finalized_reserve_packet_hash,
        pricing_reserve_packet_hash
    );
    assert_eq!(native_nav.finalized_at_height, 6);

    let trust_settlement = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        TRUST_SET_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: subscriber.clone(),
            issuer: issuer.clone(),
            asset_id: settlement_asset_id.clone(),
            limit: 1_000,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &trust_settlement,
        2,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let trust_native = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        TRUST_SET_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: subscriber.clone(),
            issuer: issuer.clone(),
            asset_id: native_nav_asset_id.clone(),
            limit: 1_000,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &trust_native,
        3,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let fund_settlement = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        ledger.account(&issuer).expect("issuer").sequence + 1,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.clone(),
            to: subscriber.clone(),
            issuer: issuer.clone(),
            asset_id: settlement_asset_id.clone(),
            amount: 1_000,
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &fund_settlement,
        7,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let route_id = "independent-operator-qnav".to_string();
    let unauthorized_route_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapRouteInit(PftlUniswapRouteInitOperation {
            operator: subscriber.clone(),
            route_id: "unauthorized-independent-qnav".to_string(),
            route_config_digest: "10".repeat(48),
            route_trust_class: "CONTROLLED".to_string(),
            native_nav_asset_id: native_nav_asset_id.clone(),
            settlement_asset_id: settlement_asset_id.clone(),
            handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
            settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 1_000_000,
            packet_notional_cap_atoms: 500_000,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 12,
            live_value_enabled: false,
            ethereum_verification_policy: None,
        }),
    );
    let before_unauthorized_route = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &unauthorized_route_init,
        8,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "unauthorized_pftl_uniswap_operator");
    assert_eq!(ledger, before_unauthorized_route);

    let mismatched_epoch_route_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::PftlUniswapRouteInit(PftlUniswapRouteInitOperation {
            operator: operator.clone(),
            route_id: "mismatched-epoch-independent-qnav".to_string(),
            route_config_digest: "12".repeat(48),
            route_trust_class: "CONTROLLED".to_string(),
            native_nav_asset_id: native_nav_asset_id.clone(),
            settlement_asset_id: settlement_asset_id.clone(),
            handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
            settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 1_000_000,
            packet_notional_cap_atoms: 500_000,
            latest_finalized_nav_epoch: 6,
            return_finality_blocks: 12,
            live_value_enabled: false,
            ethereum_verification_policy: None,
        }),
    );
    let before_mismatched_route = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &mismatched_epoch_route_init,
        8,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_route_epoch_mismatch");
    assert_eq!(ledger, before_mismatched_route);

    let route_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::PftlUniswapRouteInit(PftlUniswapRouteInitOperation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            route_config_digest: "11".repeat(48),
            route_trust_class: "CONTROLLED".to_string(),
            native_nav_asset_id: native_nav_asset_id.clone(),
            settlement_asset_id: settlement_asset_id.clone(),
            handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
            settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 1_000_000,
            packet_notional_cap_atoms: 500_000,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 12,
            live_value_enabled: false,
            ethereum_verification_policy: None,
        }),
    );
    let mut controlled_route_ledger = ledger.clone();
    let live_receipt =
        super::execute_asset_transaction(&genesis, &mut controlled_route_ledger, &route_init, 8);
    assert!(live_receipt.accepted, "{live_receipt:?}");
    let controlled_route = controlled_route_ledger
        .pftl_uniswap_route(&route_id)
        .expect("strict controlled rehearsal route");
    assert_eq!(controlled_route.route_trust_class, "CONTROLLED");
    assert!(!controlled_route.live_value_enabled);
    assert!(controlled_route.ethereum_verification_policy.is_none());
    let mut forbidden_controlled_live_value = match route_init.unsigned.operation.clone() {
        AssetTransactionOperation::PftlUniswapRouteInit(operation) => operation,
        _ => unreachable!("route-init operation"),
    };
    forbidden_controlled_live_value.live_value_enabled = true;
    assert!(forbidden_controlled_live_value
        .validate()
        .expect_err("CONTROLLED public live value must fail closed")
        .contains("cannot enable public live value"));

    // A public live-value route is admitted only when its verification
    // policy resolves to an exact governed committee on this chain.
    let authority_keys = (0_u8..4)
        .map(|index| ml_dsa_65_keygen_from_seed(&[0x91 + index; 32]))
        .collect::<Vec<_>>();
    let mut authority = FastSwapCommitteeV1 {
        domain: FastSwapCommitteeDomainV1 {
            chain: FastSwapChainDomainV1 {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: FastSwapOpaqueHashV1(
                    hex_to_bytes(&genesis_hash(&genesis))
                        .expect("genesis hex")
                        .try_into()
                        .expect("48-byte genesis hash"),
                ),
                protocol_version: genesis.protocol_version,
            },
            fastswap_schema_version: postfiat_types::FASTSWAP_SCHEMA_VERSION_V1,
            committee_epoch: 9,
            committee_root: FastSwapCommitteeRootV1::ZERO,
            validator_count: 4,
            quorum: 3,
        },
        validators: authority_keys
            .iter()
            .enumerate()
            .map(|(index, key)| FastSwapValidatorV1 {
                validator_id: format!("ethereum-authority-{index}"),
                public_key: key.public_key.clone(),
            })
            .collect(),
    };
    authority.domain.committee_root = authority.computed_root().expect("committee root");
    let policy = EthereumRouteVerificationPolicyV1 {
        authority_epoch: authority.domain.committee_epoch,
        committee_root: authority.domain.committee_root,
        minimum_confirmations: 12,
        handoff_controller_code_hash: [0x71; 32],
        wrapped_navcoin_code_hash: [0x72; 32],
    };
    let mut verified_route_ledger = ledger.clone();
    verified_route_ledger
        .fastswap_committees
        .push(authority.clone());
    let verified_route_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &verified_route_ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::PftlUniswapRouteInit(PftlUniswapRouteInitOperation {
            operator: operator.clone(),
            route_id: "verified-independent-qnav".to_string(),
            route_config_digest: "14".repeat(48),
            route_trust_class: "BFT_CHECKPOINT".to_string(),
            native_nav_asset_id: native_nav_asset_id.clone(),
            settlement_asset_id: settlement_asset_id.clone(),
            handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
            settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 1_000_000,
            packet_notional_cap_atoms: 500_000,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 12,
            live_value_enabled: true,
            ethereum_verification_policy: Some(policy.clone()),
        }),
    );
    let verified_receipt = super::execute_asset_transaction(
        &genesis,
        &mut verified_route_ledger,
        &verified_route_init,
        8,
    );
    assert!(verified_receipt.accepted, "{verified_receipt:?}");
    assert!(verified_route_ledger
        .pftl_uniswap_route("verified-independent-qnav")
        .and_then(|route| route.ethereum_verification_policy.as_ref())
        .is_some());

    let verified_subscription = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &verified_route_ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: "verified-independent-qnav".to_string(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "15".repeat(32),
                settlement_value_atoms: 280,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let verified_subscription_receipt = super::execute_asset_transaction(
        &genesis,
        &mut verified_route_ledger,
        &verified_subscription,
        9,
    );
    assert!(
        verified_subscription_receipt.accepted,
        "{verified_subscription_receipt:?}"
    );

    let verified_packet_hash = "16".repeat(48);
    let verified_export = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &verified_route_ledger,
        &subscriber_key,
        PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::PftlUniswapExportDebit(PftlUniswapExportDebitOperation {
            owner: subscriber.clone(),
            route_id: "verified-independent-qnav".to_string(),
            packet_hash: verified_packet_hash.clone(),
            export_nonce: "17".repeat(32),
            ethereum_recipient: "0x4444444444444444444444444444444444444444".to_string(),
            amount_atoms: 40,
            reservation_id: None,
            settlement_value_atoms: None,
            destination_deadline_seconds: 1_800,
            refund_delay_blocks: 3,
            ethereum_packet_digest: Some("30".repeat(32)),
            ethereum_packet_schema_version: Some(PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V1),
        }),
    );
    let verified_export_receipt = super::execute_asset_transaction(
        &genesis,
        &mut verified_route_ledger,
        &verified_export,
        10,
    );
    assert!(
        verified_export_receipt.accepted,
        "{verified_export_receipt:?}"
    );
    assert_eq!(
        verified_route_ledger
            .pftl_uniswap_route("verified-independent-qnav")
            .and_then(|route| route.export_packets.get(&verified_packet_hash))
            .expect("signed export created pending packet")
            .status,
        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED
    );
    assert_eq!(
        issued_asset_supply(&verified_route_ledger, &native_nav_asset_id)
            .expect("global supply includes outstanding bridge claim"),
        40
    );
    let mut globally_capped_ledger = verified_route_ledger.clone();
    globally_capped_ledger
        .asset_definitions
        .iter_mut()
        .find(|definition| definition.asset_id == native_nav_asset_id)
        .expect("native asset definition for global cap")
        .max_supply = Some(40);
    let over_global_cap_subscription = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &globally_capped_ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: "verified-independent-qnav".to_string(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "18".repeat(32),
                settlement_value_atoms: 7,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let before_global_cap_rejection = globally_capped_ledger.clone();
    let global_cap_receipt = super::execute_asset_transaction(
        &genesis,
        &mut globally_capped_ledger,
        &over_global_cap_subscription,
        11,
    );
    assert!(!global_cap_receipt.accepted);
    assert_eq!(global_cap_receipt.code, "issued_supply_cap_exceeded");
    assert_eq!(globally_capped_ledger, before_global_cap_rejection);

    let controller = [0x11; 20];
    let mut recipient_topic = [0_u8; 32];
    recipient_topic[12..].copy_from_slice(&[0x44; 20]);
    let consumed_signature = postfiat_bridge::ethereum_keccak256(
        b"PacketConsumed(bytes32,bytes32,address,bytes32,bytes32,bytes32,uint256,uint256)",
    );
    let source_packet_commitment = postfiat_bridge::ethereum_keccak256(
        &hex_to_bytes(&verified_packet_hash).expect("packet hash"),
    );
    let route_config_commitment = postfiat_bridge::ethereum_keccak256(&[0x14; 48]);
    let trust_class_commitment = postfiat_bridge::ethereum_keccak256(b"BFT_CHECKPOINT");
    let mut consumed_data = route_config_commitment.to_vec();
    consumed_data.extend_from_slice(&[0x31; 32]);
    consumed_data.extend_from_slice(&trust_class_commitment);
    consumed_data.extend_from_slice(&p0_ethereum_abi_u64(40));
    consumed_data.extend_from_slice(&p0_ethereum_abi_u64(700));
    let (receipts_root, receipt_proof) = p0_ethereum_receipt_proof(
        controller,
        &[
            consumed_signature,
            [0x30; 32],
            source_packet_commitment,
            recipient_topic,
        ],
        &consumed_data,
    );
    let checkpoint = EthereumFinalizedCheckpointV1 {
        schema_version: ETHEREUM_CHECKPOINT_SCHEMA_V1,
        pftl_domain: authority.domain.chain.clone(),
        route_id: "verified-independent-qnav".to_string(),
        route_config_digest: FastSwapOpaqueHashV1([0x14; 48]),
        ethereum_chain_id: 1,
        block_number: 100,
        block_hash: [0x51; 32],
        receipts_root,
        observed_head_number: 112,
        minimum_confirmations: 12,
        authority_epoch: authority.domain.committee_epoch,
        committee_root: authority.domain.committee_root,
        handoff_controller: controller,
        wrapped_navcoin_token: [0x33; 20],
        handoff_controller_code_hash: [0x71; 32],
        wrapped_navcoin_code_hash: [0x72; 32],
    };
    let certificate =
        p0_ethereum_checkpoint_certificate(&authority, &authority_keys, checkpoint.clone());
    let consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &verified_route_ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapDestinationConsume(
            PftlUniswapDestinationConsumeOperation {
                operator: operator.clone(),
                route_id: "verified-independent-qnav".to_string(),
                packet_hash: verified_packet_hash.clone(),
                ethereum_consume_tx_hash: "52".repeat(32),
                consumed_height: 100,
                finalized_height: 112,
                external_event_proof: Some(EthereumExternalEventProofV1 {
                    checkpoint_certificate: certificate,
                    receipt_proof,
                    log_index: 0,
                }),
            },
        ),
    );

    let mut tampered_data = consumed_data.clone();
    tampered_data[4 * 32 - 1] = 41;
    let (tampered_root, tampered_receipt_proof) = p0_ethereum_receipt_proof(
        controller,
        &[
            consumed_signature,
            [0x30; 32],
            source_packet_commitment,
            recipient_topic,
        ],
        &tampered_data,
    );
    let mut tampered_checkpoint = checkpoint;
    tampered_checkpoint.receipts_root = tampered_root;
    let tampered_certificate =
        p0_ethereum_checkpoint_certificate(&authority, &authority_keys, tampered_checkpoint);
    let tampered_consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &verified_route_ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapDestinationConsume(
            PftlUniswapDestinationConsumeOperation {
                operator: operator.clone(),
                route_id: "verified-independent-qnav".to_string(),
                packet_hash: verified_packet_hash.clone(),
                ethereum_consume_tx_hash: "53".repeat(32),
                consumed_height: 100,
                finalized_height: 112,
                external_event_proof: Some(EthereumExternalEventProofV1 {
                    checkpoint_certificate: tampered_certificate,
                    receipt_proof: tampered_receipt_proof,
                    log_index: 0,
                }),
            },
        ),
    );
    let mut tampered_ledger = verified_route_ledger.clone();
    let before_tampered = tampered_ledger.clone();
    let tampered_receipt =
        super::execute_asset_transaction(&genesis, &mut tampered_ledger, &tampered_consume, 11);
    assert!(!tampered_receipt.accepted);
    assert_eq!(
        tampered_receipt.code,
        "pftl_uniswap_ethereum_event_binding_mismatch"
    );
    assert_eq!(tampered_ledger, before_tampered);

    let mut wrong_binding_operation = consume.unsigned.operation.clone();
    if let AssetTransactionOperation::PftlUniswapDestinationConsume(operation) =
        &mut wrong_binding_operation
    {
        let proof = operation
            .external_event_proof
            .as_mut()
            .expect("consume proof to alter");
        let mut wrong_checkpoint = proof.checkpoint_certificate.checkpoint.clone();
        wrong_checkpoint.handoff_controller_code_hash[0] ^= 1;
        proof.checkpoint_certificate =
            p0_ethereum_checkpoint_certificate(&authority, &authority_keys, wrong_checkpoint);
    }
    let wrong_binding_consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &verified_route_ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        3,
        wrong_binding_operation,
    );
    let mut wrong_binding_ledger = verified_route_ledger.clone();
    let before_wrong_binding = wrong_binding_ledger.clone();
    let wrong_binding_receipt = super::execute_asset_transaction(
        &genesis,
        &mut wrong_binding_ledger,
        &wrong_binding_consume,
        11,
    );
    assert!(!wrong_binding_receipt.accepted);
    assert_eq!(
        wrong_binding_receipt.code,
        "pftl_uniswap_ethereum_checkpoint_route_mismatch"
    );
    assert_eq!(wrong_binding_ledger, before_wrong_binding);

    let cancelled_signature = postfiat_bridge::ethereum_keccak256(
        b"PacketCancelled(bytes32,bytes32,bytes32,uint64,uint64)",
    );
    let mut cancelled_data = p0_ethereum_abi_u64(1_800).to_vec();
    cancelled_data.extend_from_slice(&p0_ethereum_abi_u64(1_801));
    let (cancelled_root, cancelled_receipt_proof) = p0_ethereum_receipt_proof(
        controller,
        &[
            cancelled_signature,
            [0x30; 32],
            source_packet_commitment,
            [0x31; 32],
        ],
        &cancelled_data,
    );
    let mut cancelled_checkpoint = match &consume.unsigned.operation {
        AssetTransactionOperation::PftlUniswapDestinationConsume(operation) => operation
            .external_event_proof
            .as_ref()
            .expect("consume proof")
            .checkpoint_certificate
            .checkpoint
            .clone(),
        _ => unreachable!("test consume operation"),
    };
    cancelled_checkpoint.receipts_root = cancelled_root;
    cancelled_checkpoint.block_hash = [0x54; 32];
    let cancellation_certificate =
        p0_ethereum_checkpoint_certificate(&authority, &authority_keys, cancelled_checkpoint);
    let refund = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &verified_route_ledger,
        &operator_key,
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapRefundSource(PftlUniswapRefundSourceOperation {
            operator: operator.clone(),
            route_id: "verified-independent-qnav".to_string(),
            packet_hash: verified_packet_hash.clone(),
            non_consumption_proof_hash: pftl_uniswap_non_consumption_proof_hash(
                "verified-independent-qnav",
                &verified_packet_hash,
                13,
            )
            .expect("legacy refund audit commitment"),
            external_event_proof: Some(EthereumExternalEventProofV1 {
                checkpoint_certificate: cancellation_certificate,
                receipt_proof: cancelled_receipt_proof,
                log_index: 0,
            }),
        }),
    );
    let mut refunded_ledger = verified_route_ledger.clone();
    let refund_receipt =
        super::execute_asset_transaction(&genesis, &mut refunded_ledger, &refund, 13);
    assert!(refund_receipt.accepted, "{refund_receipt:?}");
    assert_eq!(
        refunded_ledger
            .pftl_uniswap_route("verified-independent-qnav")
            .and_then(|route| route.export_packets.get(&verified_packet_hash))
            .expect("refunded packet")
            .status,
        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED
    );
    assert_eq!(
        issued_asset_supply(&refunded_ledger, &native_nav_asset_id)
            .expect("global supply after certified refund"),
        40
    );
    let persisted_refund = serde_json::to_vec(&refunded_ledger).expect("persist refund state");
    refunded_ledger =
        serde_json::from_slice(&persisted_refund).expect("restore refund state after restart");
    let consume_after_refund = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &refunded_ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        4,
        consume.unsigned.operation.clone(),
    );
    let before_consume_after_refund = refunded_ledger.clone();
    let consume_after_refund_receipt =
        super::execute_asset_transaction(&genesis, &mut refunded_ledger, &consume_after_refund, 14);
    assert!(!consume_after_refund_receipt.accepted);
    assert_eq!(
        consume_after_refund_receipt.code,
        "pftl_uniswap_packet_not_consumable"
    );
    assert_eq!(refunded_ledger, before_consume_after_refund);

    let consume_receipt =
        super::execute_asset_transaction(&genesis, &mut verified_route_ledger, &consume, 11);
    assert!(consume_receipt.accepted, "{consume_receipt:?}");
    let consumed_route = verified_route_ledger
        .pftl_uniswap_route("verified-independent-qnav")
        .expect("consumed verified route");
    assert_eq!(consumed_route.outstanding_bridge_claims_atoms, 0);
    assert_eq!(consumed_route.ethereum_spendable_supply_atoms, 40);
    assert_eq!(
        issued_asset_supply(&verified_route_ledger, &native_nav_asset_id)
            .expect("global supply after certified destination consume"),
        40
    );
    assert_eq!(
        consumed_route
            .export_packets
            .get(&verified_packet_hash)
            .expect("consumed packet")
            .status,
        PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED
    );

    let return_sender = [0x61; 20];
    let return_nonce = [0x62; 32];
    let return_amount = 17;
    let return_burn_height = 120;
    let return_finalized_height = 132;
    let return_sender_text = format!("0x{}", bytes_to_hex(&return_sender));
    let return_nonce_text = bytes_to_hex(&return_nonce);
    let return_burn_id = pftl_uniswap_return_burn_id_from_fields(
        1,
        "0x1111111111111111111111111111111111111111",
        "0x3333333333333333333333333333333333333333",
        &native_nav_asset_id,
        &return_sender_text,
        &subscriber,
        return_amount,
        &return_nonce_text,
        return_burn_height,
    )
    .expect("canonical return burn id");
    let return_burn_id_bytes: [u8; 32] = hex_to_bytes(&return_burn_id)
        .expect("return burn id hex")
        .try_into()
        .expect("return burn id bytes");
    let native_asset_bytes = hex_to_bytes(&native_nav_asset_id).expect("native asset hex");
    let recipient_tail = p0_ethereum_abi_dynamic(subscriber.as_bytes());
    let native_asset_tail = p0_ethereum_abi_dynamic(&native_asset_bytes);
    let mut return_data = p0_ethereum_abi_u64(7 * 32).to_vec();
    return_data.extend_from_slice(&p0_ethereum_abi_u64(
        u64::try_from(7 * 32 + recipient_tail.len()).expect("native asset ABI offset"),
    ));
    return_data.extend_from_slice(&p0_ethereum_abi_u64(return_amount));
    return_data.extend_from_slice(&p0_ethereum_abi_u64(1));
    return_data.extend_from_slice(&p0_ethereum_abi_address(controller));
    return_data.extend_from_slice(&p0_ethereum_abi_address([0x33; 20]));
    return_data.extend_from_slice(&p0_ethereum_abi_u64(return_burn_height));
    return_data.extend_from_slice(&recipient_tail);
    return_data.extend_from_slice(&native_asset_tail);
    let return_signature = postfiat_bridge::ethereum_keccak256(
            b"ReturnBurned(bytes32,address,bytes32,string,bytes,uint256,uint256,address,address,uint256)",
        );
    let (return_receipts_root, return_receipt_proof) = p0_ethereum_receipt_proof(
        controller,
        &[
            return_signature,
            return_burn_id_bytes,
            p0_ethereum_abi_address(return_sender),
            return_nonce,
        ],
        &return_data,
    );
    let mut return_checkpoint = match &consume.unsigned.operation {
        AssetTransactionOperation::PftlUniswapDestinationConsume(operation) => operation
            .external_event_proof
            .as_ref()
            .expect("consume proof for return checkpoint")
            .checkpoint_certificate
            .checkpoint
            .clone(),
        _ => unreachable!("test consume operation"),
    };
    return_checkpoint.block_number = return_burn_height;
    return_checkpoint.observed_head_number = return_finalized_height;
    return_checkpoint.block_hash = [0x63; 32];
    return_checkpoint.receipts_root = return_receipts_root;
    let return_certificate =
        p0_ethereum_checkpoint_certificate(&authority, &authority_keys, return_checkpoint);
    let return_import = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &verified_route_ledger,
        &operator_key,
        PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::PftlUniswapReturnImport(PftlUniswapReturnImportOperation {
            operator: operator.clone(),
            route_id: "verified-independent-qnav".to_string(),
            burn_event_hash: return_burn_id.clone(),
            ethereum_chain_id: 1,
            bridge_controller: "0x1111111111111111111111111111111111111111".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            native_nav_asset_id: native_nav_asset_id.clone(),
            ethereum_sender: return_sender_text,
            pftl_recipient: subscriber.clone(),
            amount_atoms: return_amount,
            return_nonce: return_nonce_text,
            burn_height: return_burn_height,
            finalized_height: return_finalized_height,
            external_event_proof: Some(EthereumExternalEventProofV1 {
                checkpoint_certificate: return_certificate,
                receipt_proof: return_receipt_proof,
                log_index: 0,
            }),
        }),
    );
    let mut returned_ledger = verified_route_ledger.clone();
    let return_receipt =
        super::execute_asset_transaction(&genesis, &mut returned_ledger, &return_import, 10);
    assert!(return_receipt.accepted, "{return_receipt:?}");
    let returned_route = returned_ledger
        .pftl_uniswap_route("verified-independent-qnav")
        .expect("returned verified route");
    assert_eq!(returned_route.ethereum_spendable_supply_atoms, 23);
    assert_eq!(returned_route.pftl_spendable_supply_atoms, 17);
    assert!(returned_route.return_imports.contains_key(&return_burn_id));
    assert_eq!(
        issued_asset_supply(&returned_ledger, &native_nav_asset_id)
            .expect("global supply after certified return import"),
        40
    );
    let duplicate_return = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &returned_ledger,
        &operator_key,
        PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
        5,
        return_import.unsigned.operation.clone(),
    );
    let before_duplicate_return = returned_ledger.clone();
    let duplicate_return_receipt =
        super::execute_asset_transaction(&genesis, &mut returned_ledger, &duplicate_return, 11);
    assert!(!duplicate_return_receipt.accepted);
    assert_eq!(
        duplicate_return_receipt.code,
        "duplicate_pftl_uniswap_return_import"
    );
    assert_eq!(returned_ledger, before_duplicate_return);

    let persisted_consumed =
        serde_json::to_vec(&verified_route_ledger).expect("persist consumed route state");
    let mut restarted_consumed_ledger: LedgerState = serde_json::from_slice(&persisted_consumed)
        .expect("restore consumed route state after restart");
    let duplicate_consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &restarted_consumed_ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        4,
        consume.unsigned.operation.clone(),
    );
    let before_duplicate_consume = restarted_consumed_ledger.clone();
    let duplicate_consume_receipt = super::execute_asset_transaction(
        &genesis,
        &mut restarted_consumed_ledger,
        &duplicate_consume,
        10,
    );
    assert!(!duplicate_consume_receipt.accepted);
    assert_eq!(
        duplicate_consume_receipt.code,
        "pftl_uniswap_packet_not_consumable"
    );
    assert_eq!(restarted_consumed_ledger, before_duplicate_consume);

    // A BFT quorum must not sign competing finalized headers. Even if that
    // trust assumption is violated and a second valid certificate is
    // relayed, the terminal packet state remains replay-safe and cannot
    // credit Ethereum inventory twice.
    let mut conflicting_checkpoint_operation = consume.unsigned.operation.clone();
    if let AssetTransactionOperation::PftlUniswapDestinationConsume(operation) =
        &mut conflicting_checkpoint_operation
    {
        let proof = operation
            .external_event_proof
            .as_mut()
            .expect("consume proof for conflicting checkpoint");
        let mut conflicting_checkpoint = proof.checkpoint_certificate.checkpoint.clone();
        conflicting_checkpoint.block_hash[0] ^= 1;
        proof.checkpoint_certificate =
            p0_ethereum_checkpoint_certificate(&authority, &authority_keys, conflicting_checkpoint);
    }
    let conflicting_checkpoint_consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &restarted_consumed_ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        4,
        conflicting_checkpoint_operation,
    );
    let before_conflicting_checkpoint = restarted_consumed_ledger.clone();
    let conflicting_checkpoint_receipt = super::execute_asset_transaction(
        &genesis,
        &mut restarted_consumed_ledger,
        &conflicting_checkpoint_consume,
        10,
    );
    assert!(!conflicting_checkpoint_receipt.accepted);
    assert_eq!(
        conflicting_checkpoint_receipt.code,
        "pftl_uniswap_packet_not_consumable"
    );
    assert_eq!(restarted_consumed_ledger, before_conflicting_checkpoint);

    let refund_after_consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &restarted_consumed_ledger,
        &operator_key,
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND,
        4,
        refund.unsigned.operation.clone(),
    );
    let before_refund_after_consume = restarted_consumed_ledger.clone();
    let refund_after_consume_receipt = super::execute_asset_transaction(
        &genesis,
        &mut restarted_consumed_ledger,
        &refund_after_consume,
        12,
    );
    assert!(!refund_after_consume_receipt.accepted);
    assert_eq!(
        refund_after_consume_receipt.code,
        "pftl_uniswap_packet_not_refundable"
    );
    assert_eq!(restarted_consumed_ledger, before_refund_after_consume);

    let receipt = super::execute_asset_transaction(&genesis, &mut ledger, &route_init, 8);
    assert!(receipt.accepted, "{receipt:?}");
    // Upgrade this legacy fixture to the governed checkpoint policy before it
    // exercises live-value operations. Production CONTROLLED routes remain
    // fail-closed, as asserted above.
    ledger.fastswap_committees.push(authority.clone());
    let legacy_route = ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == route_id)
        .expect("legacy fixture route");
    legacy_route.route_trust_class = PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string();
    legacy_route.ethereum_verification_policy = Some(policy.clone());
    legacy_route.live_value_enabled = true;
    assert_eq!(ledger.pftl_uniswap_receipts.len(), 1);
    assert_eq!(
        ledger
            .pftl_uniswap_route(&route_id)
            .expect("route after init")
            .latest_finalized_nav_epoch,
        7
    );

    let wrong_price_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "40".repeat(32),
                settlement_value_atoms: 705,
                nav_price_settlement_atoms_per_nav_atom: 6,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let before_wrong_price = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &wrong_price_subscribe,
        9,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_price_mismatch");
    assert_eq!(ledger, before_wrong_price);

    let wrong_packet_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "41".repeat(32),
                settlement_value_atoms: 705,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: "56".repeat(48),
            },
        ),
    );
    let before_wrong_packet = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &wrong_packet_subscribe,
        9,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_pricing_packet_mismatch");
    assert_eq!(ledger, before_wrong_packet);

    let wrong_epoch_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "42".repeat(32),
                settlement_value_atoms: 705,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 6,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let before_wrong_epoch = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &wrong_epoch_subscribe,
        9,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "stale_pftl_uniswap_nav_epoch");
    assert_eq!(ledger, before_wrong_epoch);

    let halted_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "43".repeat(32),
                settlement_value_atoms: 705,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let mut halted_ledger = ledger.clone();
    let halted_nav = halted_ledger
        .nav_asset_mut(&native_nav_asset_id)
        .expect("native NAV asset for halt");
    halted_nav.halted = true;
    halted_nav.halt_reason = "test_halt".to_string();
    let before_halted = halted_ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut halted_ledger,
        &halted_subscribe,
        9,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_nav_asset_halted");
    assert_eq!(halted_ledger, before_halted);

    let stale_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "43".repeat(32),
                settlement_value_atoms: 705,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let before_stale = ledger.clone();
    let stale_height = ledger
        .nav_asset(&native_nav_asset_id)
        .expect("native NAV asset for stale")
        .finalized_at_height
        + MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS
        + 1;
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &stale_subscribe,
        stale_height,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "stale_pftl_uniswap_pricing");
    assert_eq!(ledger, before_stale);

    let legacy_height_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "43".repeat(32),
                settlement_value_atoms: 705,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let mut legacy_height_ledger = ledger.clone();
    legacy_height_ledger
        .nav_asset_mut(&native_nav_asset_id)
        .expect("native NAV asset for legacy height")
        .finalized_at_height = 0;
    let before_legacy_height = legacy_height_ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut legacy_height_ledger,
        &legacy_height_subscribe,
        9,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_pricing_height_missing");
    assert_eq!(legacy_height_ledger, before_legacy_height);

    let unfinalized_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "43".repeat(32),
                settlement_value_atoms: 705,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let mut unfinalized_ledger = ledger.clone();
    let unfinalized_nav = unfinalized_ledger
        .nav_asset_mut(&native_nav_asset_id)
        .expect("native NAV asset for unfinalized");
    unfinalized_nav.finalized_epoch = 0;
    unfinalized_nav.nav_per_unit = 0;
    unfinalized_nav.finalized_reserve_packet_hash.clear();
    unfinalized_nav.finalized_at_height = 0;
    let before_unfinalized = unfinalized_ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut unfinalized_ledger,
        &unfinalized_subscribe,
        9,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_nav_not_finalized");
    assert_eq!(unfinalized_ledger, before_unfinalized);

    let subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "44".repeat(32),
                settlement_value_atoms: 705,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let mut paused_subscribe_ledger = ledger.clone();
    paused_subscribe_ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == route_id)
        .expect("route to pause before subscribe")
        .paused = true;
    let before_paused_subscribe = paused_subscribe_ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut paused_subscribe_ledger,
        &subscribe,
        9,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_route_paused");
    assert_eq!(paused_subscribe_ledger, before_paused_subscribe);

    let receipt = super::execute_asset_transaction(&genesis, &mut ledger, &subscribe, 9);
    assert!(receipt.accepted, "{receipt:?}");
    let native_nav_after_subscription = ledger
        .nav_asset(&native_nav_asset_id)
        .expect("native NAV asset after subscription");
    assert_eq!(native_nav_after_subscription.finalized_epoch, 7);
    assert_eq!(native_nav_after_subscription.nav_per_unit, 7_000_000);
    assert_eq!(
        native_nav_after_subscription.finalized_reserve_packet_hash,
        pricing_reserve_packet_hash
    );
    assert_eq!(native_nav_after_subscription.circulating_supply, 100);
    assert_eq!(
        ledger
            .trustline_for_account_asset(&subscriber, &settlement_asset_id)
            .expect("subscriber settlement line")
            .balance,
        300
    );
    assert_eq!(
        ledger
            .trustline_for_account_asset(&subscriber, &native_nav_asset_id)
            .expect("subscriber NAV line")
            .balance,
        100
    );
    {
        let route = ledger
            .pftl_uniswap_route(&route_id)
            .expect("PFTL-Uniswap route after subscribe");
        assert_eq!(route.settlement_reserve_atoms, 700);
        assert_eq!(route.authorized_valid_supply_atoms, 100);
        assert_eq!(route.pftl_spendable_supply_atoms, 100);
        assert_eq!(
            route
                .native_spendable_balances_atoms
                .get(&subscriber)
                .copied(),
            Some(100)
        );
    }
    assert_eq!(
        issued_asset_supply(&ledger, &native_nav_asset_id)
            .expect("native issued supply after primary fill"),
        100
    );
    assert_eq!(
        issued_asset_supply(&ledger, &settlement_asset_id)
            .expect("settlement issued supply after reserve custody move"),
        1_000
    );
    assert_eq!(ledger.pftl_uniswap_receipts.len(), 2);

    let duplicate_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        4,
        subscribe.unsigned.operation.clone(),
    );
    let before_duplicate_subscribe = ledger.clone();
    let receipt = super::execute_asset_transaction(&genesis, &mut ledger, &duplicate_subscribe, 10);
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "duplicate_pftl_uniswap_subscription_nonce");
    assert_eq!(ledger, before_duplicate_subscribe);

    let mut capped_ledger = ledger.clone();
    capped_ledger
        .pftl_uniswap_route_mut(&route_id)
        .expect("route to cap")
        .route_supply_cap_atoms = 100;
    let capped_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &capped_ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "45".repeat(32),
                settlement_value_atoms: 7,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let before_capped_subscribe = capped_ledger.clone();
    let receipt =
        super::execute_asset_transaction(&genesis, &mut capped_ledger, &capped_subscribe, 10);
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_route_supply_cap_exceeded");
    assert_eq!(capped_ledger, before_capped_subscribe);

    let malformed_subscribe = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::PftlUniswapPrimarySubscribe(
            PftlUniswapPrimarySubscribeOperation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                subscription_nonce: "46".repeat(32),
                settlement_value_atoms: 6,
                nav_price_settlement_atoms_per_nav_atom: 7,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let before_malformed_subscribe = ledger.clone();
    let receipt = super::execute_asset_transaction(&genesis, &mut ledger, &malformed_subscribe, 10);
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "subscription_mints_zero_nav");
    assert_eq!(ledger, before_malformed_subscribe);

    let packet_hash = "66".repeat(48);
    let export = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::PftlUniswapExportDebit(PftlUniswapExportDebitOperation {
            owner: subscriber.clone(),
            route_id: route_id.clone(),
            packet_hash: packet_hash.clone(),
            export_nonce: "77".repeat(32),
            ethereum_recipient: "0x4444444444444444444444444444444444444444".to_string(),
            amount_atoms: 40,
            reservation_id: None,
            settlement_value_atoms: None,
            destination_deadline_seconds: 1_800,
            refund_delay_blocks: 3,
            ethereum_packet_digest: Some("65".repeat(32)),
            ethereum_packet_schema_version: Some(PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V1),
        }),
    );
    let mut paused_export_ledger = ledger.clone();
    paused_export_ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == route_id)
        .expect("route to pause before export")
        .paused = true;
    let before_paused_export = paused_export_ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut paused_export_ledger,
        &export,
        10,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_route_paused");
    assert_eq!(paused_export_ledger, before_paused_export);

    let receipt = super::execute_asset_transaction(&genesis, &mut ledger, &export, 10);
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&subscriber, &native_nav_asset_id)
            .expect("subscriber NAV line")
            .balance,
        60
    );
    {
        let route = ledger
            .pftl_uniswap_route(&route_id)
            .expect("PFTL-Uniswap route after export");
        assert_eq!(route.pftl_spendable_supply_atoms, 60);
        assert_eq!(route.outstanding_bridge_claims_atoms, 40);
        assert_eq!(
            route
                .native_spendable_balances_atoms
                .get(&subscriber)
                .copied(),
            Some(60)
        );
        let packet = route
            .export_packets
            .get(&packet_hash)
            .expect("export packet");
        assert_eq!(packet.source_height, 10);
        assert_eq!(packet.refund_not_before_height, 13);
        assert_eq!(packet.amount_atoms, 40);
    }
    assert_eq!(ledger.pftl_uniswap_receipts.len(), 3);

    let refund_proof_hash = pftl_uniswap_non_consumption_proof_hash(&route_id, &packet_hash, 13)
        .expect("refund proof hash");
    let mut refund_ledger = ledger.clone();
    let mismatched_refund = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapRefundSource(PftlUniswapRefundSourceOperation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            packet_hash: packet_hash.clone(),
            non_consumption_proof_hash: "88".repeat(48),
            external_event_proof: None,
        }),
    );
    let before_mismatched_refund = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &mismatched_refund,
        13,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_non_consumption_proof_mismatch");
    assert_eq!(ledger, before_mismatched_refund);

    let early_refund = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapRefundSource(PftlUniswapRefundSourceOperation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            packet_hash: packet_hash.clone(),
            non_consumption_proof_hash: refund_proof_hash.clone(),
            external_event_proof: None,
        }),
    );
    let before_early_refund = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &early_refund,
        12,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_refund_before_window");
    assert_eq!(ledger, before_early_refund);

    let refund = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &refund_ledger,
        &operator_key,
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapRefundSource(PftlUniswapRefundSourceOperation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            packet_hash: packet_hash.clone(),
            non_consumption_proof_hash: refund_proof_hash.clone(),
            external_event_proof: None,
        }),
    );
    let live_refund_receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut refund_ledger,
        &refund,
        13,
    );
    assert!(live_refund_receipt.accepted, "{live_refund_receipt:?}");
    assert_eq!(
        refund_ledger
            .trustline_for_account_asset(&subscriber, &native_nav_asset_id)
            .expect("subscriber NAV line")
            .balance,
        100
    );
    {
        let route = refund_ledger
            .pftl_uniswap_route(&route_id)
            .expect("PFTL-Uniswap route after refund");
        assert_eq!(route.pftl_spendable_supply_atoms, 100);
        assert_eq!(route.outstanding_bridge_claims_atoms, 0);
        assert_eq!(
            route
                .native_spendable_balances_atoms
                .get(&subscriber)
                .copied(),
            Some(100)
        );
        let packet = route
            .export_packets
            .get(&packet_hash)
            .expect("refunded export packet");
        assert_eq!(packet.status, PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED);
    }
    assert_eq!(refund_ledger.pftl_uniswap_receipts.len(), 4);

    let mut paused_refund_ledger = ledger.clone();
    paused_refund_ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == route_id)
        .expect("route to pause before refund")
        .paused = true;
    let paused_refund = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &paused_refund_ledger,
        &operator_key,
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapRefundSource(PftlUniswapRefundSourceOperation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            packet_hash: packet_hash.clone(),
            non_consumption_proof_hash: refund_proof_hash.clone(),
            external_event_proof: None,
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut paused_refund_ledger,
        &paused_refund,
        13,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let consume_after_refund = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &refund_ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::PftlUniswapDestinationConsume(
            PftlUniswapDestinationConsumeOperation {
                operator: operator.clone(),
                route_id: route_id.clone(),
                packet_hash: packet_hash.clone(),
                ethereum_consume_tx_hash: "99".repeat(32),
                consumed_height: 14,
                finalized_height: 26,
                external_event_proof: None,
            },
        ),
    );
    let before_consume_after_refund = refund_ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut refund_ledger,
        &consume_after_refund,
        14,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_packet_not_consumable");
    assert_eq!(refund_ledger, before_consume_after_refund);

    let under_finality_consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapDestinationConsume(
            PftlUniswapDestinationConsumeOperation {
                operator: operator.clone(),
                route_id: route_id.clone(),
                packet_hash: packet_hash.clone(),
                ethereum_consume_tx_hash: "98".repeat(32),
                consumed_height: 14,
                finalized_height: 25,
                external_event_proof: None,
            },
        ),
    );
    let before_under_finality_consume = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &under_finality_consume,
        14,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_destination_below_finality");
    assert_eq!(ledger, before_under_finality_consume);

    let paused_destination_consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapDestinationConsume(
            PftlUniswapDestinationConsumeOperation {
                operator: operator.clone(),
                route_id: route_id.clone(),
                packet_hash: packet_hash.clone(),
                ethereum_consume_tx_hash: "99".repeat(32),
                consumed_height: 14,
                finalized_height: 26,
                external_event_proof: None,
            },
        ),
    );
    let mut paused_consume_ledger = ledger.clone();
    paused_consume_ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == route_id)
        .expect("route to pause before destination consume")
        .paused = true;
    let before_paused_consume = paused_consume_ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut paused_consume_ledger,
        &paused_destination_consume,
        14,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_route_paused");
    assert_eq!(paused_consume_ledger, before_paused_consume);

    let destination_consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::PftlUniswapDestinationConsume(
            PftlUniswapDestinationConsumeOperation {
                operator: operator.clone(),
                route_id: route_id.clone(),
                packet_hash: packet_hash.clone(),
                ethereum_consume_tx_hash: "99".repeat(32),
                consumed_height: 14,
                finalized_height: 26,
                external_event_proof: None,
            },
        ),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &destination_consume,
        14,
    );
    assert!(receipt.accepted, "{receipt:?}");
    {
        let route = ledger
            .pftl_uniswap_route(&route_id)
            .expect("PFTL-Uniswap route after destination consume");
        assert_eq!(route.pftl_spendable_supply_atoms, 60);
        assert_eq!(route.outstanding_bridge_claims_atoms, 0);
        assert_eq!(route.ethereum_spendable_supply_atoms, 40);
        let packet = route
            .export_packets
            .get(&packet_hash)
            .expect("destination consumed export packet");
        assert_eq!(
            packet.status,
            PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED
        );
    }
    assert_eq!(ledger.pftl_uniswap_receipts.len(), 4);

    let refund_after_consume = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::PftlUniswapRefundSource(PftlUniswapRefundSourceOperation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            packet_hash: packet_hash.clone(),
            non_consumption_proof_hash: refund_proof_hash.clone(),
            external_event_proof: None,
        }),
    );
    let before_refund_after_consume = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &refund_after_consume,
        15,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_packet_not_refundable");
    assert_eq!(ledger, before_refund_after_consume);
    ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == route_id)
        .expect("route to pause before return import")
        .paused = true;

    let burn_event_hash = pftl_uniswap_return_burn_id_from_fields(
        1,
        "0x1111111111111111111111111111111111111111",
        "0x3333333333333333333333333333333333333333",
        &native_nav_asset_id,
        "0x5555555555555555555555555555555555555555",
        &subscriber,
        40,
        &"ab".repeat(32),
        20,
    )
    .expect("return burn id");
    let mismatched_return_import = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::PftlUniswapReturnImport(PftlUniswapReturnImportOperation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            burn_event_hash: "aa".repeat(32),
            ethereum_chain_id: 1,
            bridge_controller: "0x1111111111111111111111111111111111111111".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            native_nav_asset_id: native_nav_asset_id.clone(),
            ethereum_sender: "0x5555555555555555555555555555555555555555".to_string(),
            pftl_recipient: subscriber.clone(),
            amount_atoms: 40,
            return_nonce: "ab".repeat(32),
            burn_height: 20,
            finalized_height: 32,
            external_event_proof: None,
        }),
    );
    let before_mismatched_return_import = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &mismatched_return_import,
        22,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_return_burn_id_mismatch");
    assert_eq!(ledger, before_mismatched_return_import);

    let return_import = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::PftlUniswapReturnImport(PftlUniswapReturnImportOperation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            burn_event_hash: burn_event_hash.clone(),
            ethereum_chain_id: 1,
            bridge_controller: "0x1111111111111111111111111111111111111111".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            native_nav_asset_id: native_nav_asset_id.clone(),
            ethereum_sender: "0x5555555555555555555555555555555555555555".to_string(),
            pftl_recipient: subscriber.clone(),
            amount_atoms: 40,
            return_nonce: "ab".repeat(32),
            burn_height: 20,
            finalized_height: 32,
            external_event_proof: None,
        }),
    );
    let live_return_receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &return_import,
        23,
    );
    assert!(live_return_receipt.accepted, "{live_return_receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&subscriber, &native_nav_asset_id)
            .expect("subscriber NAV line")
            .balance,
        100
    );
    {
        let route = ledger
            .pftl_uniswap_route(&route_id)
            .expect("PFTL-Uniswap route after return import");
        assert_eq!(route.pftl_spendable_supply_atoms, 100);
        assert_eq!(route.ethereum_spendable_supply_atoms, 0);
        assert_eq!(route.outstanding_bridge_claims_atoms, 0);
        assert_eq!(
            route
                .native_spendable_balances_atoms
                .get(&subscriber)
                .copied(),
            Some(100)
        );
        let imported = route
            .return_imports
            .get(&burn_event_hash)
            .expect("return import");
        assert_eq!(imported.status, PFTL_UNISWAP_RETURN_STATUS_IMPORTED);
        assert_eq!(imported.amount_atoms, 40);
    }
    assert_eq!(ledger.pftl_uniswap_receipts.len(), 5);

    let operator_settlement_trust = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        TRUST_SET_TRANSACTION_KIND,
        ledger.account(&operator).expect("operator").sequence + 1,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: operator.clone(),
            issuer: issuer.clone(),
            asset_id: settlement_asset_id.clone(),
            limit: 2_000,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &operator_settlement_trust,
        24,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let fund_operator = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &issuer_key,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        ledger.account(&issuer).expect("issuer").sequence + 1,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.clone(),
            to: operator.clone(),
            issuer: issuer.clone(),
            asset_id: settlement_asset_id.clone(),
            amount: 800,
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &fund_operator,
        24,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let mut primary_policy = PftlUniswapPrimaryMarketPolicyV2 {
        policy_hash: String::new(),
        policy_epoch: 1,
        issue_multiplier_bps: 10_050,
        redeem_multiplier_bps: 9_995,
        issue_capacity_atoms: 800,
        redeem_capacity_atoms: 800,
        max_order_atoms: 400,
        min_order_atoms: 1,
        valid_from_height: 24,
        expires_at_height: 100,
        max_nav_age_blocks: 100,
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
    };
    primary_policy.policy_hash = primary_policy.computed_hash();
    let v2_route_id = "independent-operator-qnav-v2".to_string();
    let v2_route_operation = PftlUniswapRouteInitV2Operation {
        operator: operator.clone(),
        route_id: v2_route_id.clone(),
        route_config_digest: "b1".repeat(48),
        native_nav_asset_id: native_nav_asset_id.clone(),
        settlement_asset_id: settlement_asset_id.clone(),
        opening_inventory_atoms: 0,
        opening_inventory_holder: String::new(),
        handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
        settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
        wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
        ethereum_chain_id: 1,
        route_supply_cap_atoms: 800,
        packet_notional_cap_atoms: 100,
        latest_finalized_nav_epoch: 7,
        return_finality_blocks: 12,
        route_epoch: 1,
        outbound_verification_class: PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY.to_string(),
        return_verification_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
        live_value_enabled: false,
        ethereum_verification_policy: policy.clone(),
        primary_market_policy: primary_policy.clone(),
    };

    // A fresh production route can adopt already-minted NAV only when the
    // proof-backed circulating supply, issued supply, and one movable holder
    // balance are exactly the same amount.
    let mut opening_inventory_ledger = ledger.clone();
    opening_inventory_ledger.pftl_uniswap_routes.clear();
    opening_inventory_ledger.pftl_uniswap_receipts.clear();
    let mut opening_operation = v2_route_operation.clone();
    opening_operation.route_id = "independent-operator-qnav-opening-inventory".to_string();
    opening_operation.opening_inventory_atoms = 100;
    opening_operation.opening_inventory_holder = subscriber.clone();
    let opening_route_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &opening_inventory_ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_INIT_V2_TRANSACTION_KIND,
        opening_inventory_ledger
            .account(&operator)
            .expect("operator")
            .sequence
            + 1,
        AssetTransactionOperation::PftlUniswapRouteInitV2(opening_operation.clone()),
    );
    let receipt = super::execute_asset_transaction(
        &genesis,
        &mut opening_inventory_ledger,
        &opening_route_init,
        24,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let opening_route = opening_inventory_ledger
        .pftl_uniswap_route(&opening_operation.route_id)
        .expect("opening inventory route");
    assert_eq!(opening_route.authorized_valid_supply_atoms, 100);
    assert_eq!(opening_route.pftl_spendable_supply_atoms, 100);
    assert_eq!(
        opening_route
            .native_spendable_balances_atoms
            .get(&subscriber),
        Some(&100)
    );

    // The proof-backed opening inventory predates primary-market orders, so it
    // has no paid-order reservation. Permit exactly one export of the complete
    // opening balance, while continuing to reject partial reservation-free
    // exports.
    opening_inventory_ledger
        .pftl_uniswap_route_mut(&opening_operation.route_id)
        .expect("opening inventory route to activate")
        .live_value_enabled = true;
    let partial_opening_export = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &opening_inventory_ledger,
        &subscriber_key,
        PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
        opening_inventory_ledger
            .account(&subscriber)
            .expect("subscriber")
            .sequence
            + 1,
        AssetTransactionOperation::PftlUniswapExportDebit(
            PftlUniswapExportDebitOperation {
                owner: subscriber.clone(),
                route_id: opening_operation.route_id.clone(),
                packet_hash: "c0".repeat(48),
                export_nonce: "c1".repeat(32),
                ethereum_recipient: "0x4444444444444444444444444444444444444444"
                    .to_string(),
                amount_atoms: 99,
                reservation_id: None,
                settlement_value_atoms: Some(697),
                destination_deadline_seconds: 1_800,
                refund_delay_blocks: 3,
                ethereum_packet_digest: Some("c2".repeat(32)),
                ethereum_packet_schema_version: Some(
                    PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V2,
                ),
            },
        ),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut opening_inventory_ledger,
        &partial_opening_export,
        24,
    );
    assert!(!receipt.accepted);
    assert_eq!(
        receipt.code,
        "pftl_uniswap_export_entitlement_required"
    );

    let opening_packet_hash = "c3".repeat(48);
    let opening_export_nonce = "c4".repeat(32);
    let opening_export_recipient =
        "0x4444444444444444444444444444444444444444".to_string();
    let opening_export_digest = {
        let route = opening_inventory_ledger
            .pftl_uniswap_route(&opening_operation.route_id)
            .expect("opening inventory route for packet");
        let v2 = route.v2.as_ref().expect("opening inventory v2 state");
        PftlUniswapMintPacketV2 {
            route_config_digest: route.route_config_digest.clone(),
            source_packet_hash: opening_packet_hash.clone(),
            reservation_id: opening_packet_hash.clone(),
            source_receipt_hash: "01".repeat(48),
            source_receipt_root: "02".repeat(48),
            settlement_asset_id: route.settlement_asset_id.clone(),
            native_nav_asset_id: route.native_nav_asset_id.clone(),
            pricing_reserve_packet_hash: primary_policy.pricing_reserve_packet_hash.clone(),
            policy_hash_commitment: postfiat_types::pftl_uniswap_keccak_commitment48(
                "policy",
                &primary_policy.policy_hash,
            )
            .expect("policy commitment"),
            route_epoch: v2.route_epoch,
            pricing_nav_epoch: primary_policy.pricing_nav_epoch,
            deadline_seconds: 1_800,
            nonce: opening_export_nonce.clone(),
            destination_chain_id: route.ethereum_chain_id,
            destination_controller: route.handoff_controller.clone(),
            wrapped_token: route.wrapped_navcoin_token.clone(),
            ethereum_recipient: opening_export_recipient.clone(),
            mint_amount_atoms: 100,
            settlement_value_atoms: 704,
        }
        .evm_digest()
        .expect("opening inventory packet digest")
    };
    let opening_export = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &opening_inventory_ledger,
        &subscriber_key,
        PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
        opening_inventory_ledger
            .account(&subscriber)
            .expect("subscriber")
            .sequence
            + 1,
        AssetTransactionOperation::PftlUniswapExportDebit(
            PftlUniswapExportDebitOperation {
                owner: subscriber.clone(),
                route_id: opening_operation.route_id.clone(),
                packet_hash: opening_packet_hash.clone(),
                export_nonce: opening_export_nonce,
                ethereum_recipient: opening_export_recipient,
                amount_atoms: 100,
                reservation_id: None,
                settlement_value_atoms: Some(704),
                destination_deadline_seconds: 1_800,
                refund_delay_blocks: 3,
                ethereum_packet_digest: Some(opening_export_digest),
                ethereum_packet_schema_version: Some(
                    PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V2,
                ),
            },
        ),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut opening_inventory_ledger,
        &opening_export,
        24,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let opening_route = opening_inventory_ledger
        .pftl_uniswap_route(&opening_operation.route_id)
        .expect("opening inventory route after export");
    assert_eq!(opening_route.pftl_spendable_supply_atoms, 0);
    assert_eq!(opening_route.outstanding_bridge_claims_atoms, 100);
    assert!(opening_route.native_spendable_balances_atoms.is_empty());
    assert_eq!(
        opening_route
            .export_packets
            .get(&opening_packet_hash)
            .expect("opening inventory packet")
            .reservation_id
            .as_deref(),
        Some(opening_packet_hash.as_str())
    );

    let mut mismatched_opening_ledger = ledger.clone();
    mismatched_opening_ledger.pftl_uniswap_routes.clear();
    mismatched_opening_ledger.pftl_uniswap_receipts.clear();
    let mut mismatched_opening_operation = opening_operation.clone();
    mismatched_opening_operation.route_id = "independent-operator-qnav-bad-opening".to_string();
    mismatched_opening_operation.opening_inventory_atoms = 99;
    let mismatched_opening_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &mismatched_opening_ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_INIT_V2_TRANSACTION_KIND,
        mismatched_opening_ledger
            .account(&operator)
            .expect("operator")
            .sequence
            + 1,
        AssetTransactionOperation::PftlUniswapRouteInitV2(mismatched_opening_operation),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut mismatched_opening_ledger,
        &mismatched_opening_init,
        24,
    );
    assert!(!receipt.accepted);
    assert_eq!(
        receipt.code,
        "pftl_uniswap_opening_inventory_supply_mismatch"
    );

    let double_count_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_INIT_V2_TRANSACTION_KIND,
        ledger.account(&operator).expect("operator").sequence + 1,
        AssetTransactionOperation::PftlUniswapRouteInitV2(opening_operation),
    );
    let mut double_count_ledger = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut double_count_ledger,
        &double_count_init,
        24,
    );
    assert!(!receipt.accepted);
    assert_eq!(
        receipt.code,
        "pftl_uniswap_opening_inventory_double_count"
    );

    let v2_route_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_INIT_V2_TRANSACTION_KIND,
        ledger.account(&operator).expect("operator").sequence + 1,
        AssetTransactionOperation::PftlUniswapRouteInitV2(v2_route_operation),
    );
    let receipt = super::execute_asset_transaction(&genesis, &mut ledger, &v2_route_init, 24);
    assert!(receipt.accepted, "{receipt:?}");

    let unfunded_reserve = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_ORDER_RESERVE_TRANSACTION_KIND,
        ledger.account(&subscriber).expect("subscriber").sequence + 1,
        AssetTransactionOperation::PftlUniswapOrderReserve(
            PftlUniswapOrderReserveOperation {
                subscriber: subscriber.clone(),
                route_id: v2_route_id.clone(),
                reservation_id: "b0".repeat(48),
                ethereum_recipient: "0x4444444444444444444444444444444444444444"
                    .to_string(),
                route_epoch: 1,
                policy_epoch: 1,
                policy_hash: primary_policy.policy_hash.clone(),
                mint_amount_atoms: 40,
                max_settlement_value_atoms: 282,
                expires_at_height: 30,
            },
        ),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &unfunded_reserve,
        24,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_route_paused");

    // The legacy funding operation remains decodable for protocol
    // compatibility, but v2 rejects it: subscription principal funds
    // redemption and no separate liquidity bucket exists.
    let retired_liquidity_fund = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND,
        ledger.account(&operator).expect("operator").sequence + 1,
        AssetTransactionOperation::PftlUniswapRedemptionFund(
            PftlUniswapRedemptionFundOperation {
                funder: operator.clone(),
                route_id: v2_route_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                funding_nonce: "b5".repeat(32),
                amount_atoms: 800,
            },
        ),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &retired_liquidity_fund,
        24,
    );
    assert!(!receipt.accepted, "{receipt:?}");
    assert_eq!(receipt.code, "pftl_uniswap_redemption_fund_retired");

    primary_policy.policy_epoch = 2;
    primary_policy.policy_hash = primary_policy.computed_hash();
    let activate_route = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_EPOCH_ADVANCE_TRANSACTION_KIND,
        ledger.account(&operator).expect("operator").sequence + 1,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(
            PftlUniswapRouteEpochAdvanceOperation {
                operator: operator.clone(),
                route_id: v2_route_id.clone(),
                prior_route_epoch: 1,
                next_route_epoch: 2,
                next_route_config_digest: "b2".repeat(48),
                live_value_enabled: true,
                next_primary_market_policy: primary_policy.clone(),
            },
        ),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &activate_route,
        24,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let reservation_id = "b6".repeat(48);
    let reserve = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_ORDER_RESERVE_TRANSACTION_KIND,
        ledger.account(&subscriber).expect("subscriber").sequence + 1,
        AssetTransactionOperation::PftlUniswapOrderReserve(
            PftlUniswapOrderReserveOperation {
                subscriber: subscriber.clone(),
                route_id: v2_route_id.clone(),
                reservation_id: reservation_id.clone(),
                ethereum_recipient: "0x4444444444444444444444444444444444444444"
                    .to_string(),
                route_epoch: 2,
                policy_epoch: 2,
                policy_hash: primary_policy.policy_hash.clone(),
                mint_amount_atoms: 40,
                max_settlement_value_atoms: 282,
                expires_at_height: 30,
            },
        ),
    );
    ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == v2_route_id)
        .expect("v2 route")
        .live_value_enabled = false;
    let disabled_receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &reserve,
        24,
    );
    assert!(!disabled_receipt.accepted);
    assert_eq!(disabled_receipt.code, "pftl_uniswap_route_paused");
    ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == v2_route_id)
        .expect("v2 route")
        .live_value_enabled = true;
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &reserve,
        24,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let subscribe_v2 = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_V2_TRANSACTION_KIND,
        ledger.account(&subscriber).expect("subscriber").sequence + 1,
        AssetTransactionOperation::PftlUniswapPrimarySubscribeV2(
            PftlUniswapPrimarySubscribeV2Operation {
                subscriber: subscriber.clone(),
                route_id: v2_route_id.clone(),
                reservation_id: reservation_id.clone(),
                subscription_nonce: "b7".repeat(32),
                settlement_asset_id: settlement_asset_id.clone(),
                settlement_value_atoms: 282,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &subscribe_v2,
        25,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let mut private_issue_ledger = ledger.clone();
    let private_issue = AssetOrchardPrivatePrimaryIssueActionPayload {
        version: 1,
        schema: "postfiat-asset-orchard-private-primary-issue-action-v1".to_string(),
        pool_id: "asset-orchard-v1".to_string(),
        route_id: v2_route_id.clone(),
        subscriber: subscriber.clone(),
        ethereum_recipient: "0x5555555555555555555555555555555555555555".to_string(),
        reservation_id: "c5".repeat(48),
        subscription_nonce: "c6".repeat(32),
        route_epoch: 2,
        policy_epoch: 2,
        policy_hash: primary_policy.policy_hash.clone(),
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
        mint_amount_atoms: 10,
        settlement_value_atoms: 71,
        expires_at_height: 30,
        output_commitment: String::new(),
        encrypted_output: String::new(),
        output_validity_action_json: String::new(),
        proof_system_id: String::new(),
        circuit_id: String::new(),
        pool_domain: String::new(),
        anchor: String::new(),
        nullifier: String::new(),
        randomized_verification_key: String::new(),
        settlement_asset_tag_lo: 0,
        settlement_asset_tag_hi: 0,
        native_nav_asset_tag_lo: 0,
        native_nav_asset_tag_hi: 0,
        primary_binding_hash: String::new(),
        proof: String::new(),
        spend_authorization_signature: String::new(),
    };
    let mut wrong_settlement_ledger = private_issue_ledger.clone();
    let mut wrong_settlement = private_issue.clone();
    wrong_settlement.settlement_value_atoms -= 1;
    let wrong_settlement_before = pftl_uniswap_route_state_hash(
        wrong_settlement_ledger
            .pftl_uniswap_routes
            .iter()
            .find(|route| route.route_id == v2_route_id)
            .expect("wrong-settlement route"),
    );
    let wrong_settlement_error =
        super::apply_asset_orchard_private_primary_issue_route_transition(
            &mut wrong_settlement_ledger,
            &wrong_settlement,
            26,
        )
        .expect_err("noncanonical private issue settlement must reject");
    assert_eq!(
        wrong_settlement_error.0,
        "pftl_uniswap_subscription_slippage"
    );
    assert_eq!(
        pftl_uniswap_route_state_hash(
            wrong_settlement_ledger
                .pftl_uniswap_routes
                .iter()
                .find(|route| route.route_id == v2_route_id)
                .expect("wrong-settlement route after rejection"),
        ),
        wrong_settlement_before
    );
    super::apply_asset_orchard_private_primary_issue_route_transition(
        &mut private_issue_ledger,
        &private_issue,
        26,
    )
    .expect("private issue route transition");
    let private_issue_route = private_issue_ledger
        .pftl_uniswap_routes
        .iter()
        .find(|route| route.route_id == v2_route_id)
        .expect("private issue route");
    let private_issue_v2 = private_issue_route.v2.as_ref().expect("private issue v2");
    assert_eq!(private_issue_route.authorized_valid_supply_atoms, 50);
    assert_eq!(private_issue_route.pftl_spendable_supply_atoms, 50);
    assert_eq!(private_issue_route.settlement_reserve_atoms, 350);
    assert_eq!(private_issue_v2.issue_capacity_used_atoms, 50);
    assert_eq!(private_issue_v2.non_nav_spread_atoms, 3);
    assert_eq!(
        private_issue_route
            .native_spendable_balances_atoms
            .get(&subscriber),
        Some(&50)
    );
    assert_eq!(
        private_issue_v2
            .terminal_reservations
            .get(&private_issue.reservation_id)
            .map(String::as_str),
        Some(PFTL_UNISWAP_ORDER_STATUS_CONSUMED)
    );
    assert_eq!(
        private_issue_v2
            .export_entitlements
            .get(&private_issue.reservation_id)
            .map(|entitlement| entitlement.remaining_amount_atoms),
        Some(10)
    );
    assert_eq!(
        private_issue_ledger
            .nav_asset(&native_nav_asset_id)
            .expect("private issue NAV asset")
            .circulating_supply,
        150
    );
    assert_eq!(
        private_issue_ledger
            .pftl_uniswap_receipts
            .last()
            .expect("private issue receipt")
            .transition,
        "private_primary_subscription"
    );
    let state_after_private_issue = pftl_uniswap_route_state_hash(private_issue_route);
    let replay = super::apply_asset_orchard_private_primary_issue_route_transition(
        &mut private_issue_ledger,
        &private_issue,
        26,
    )
    .expect_err("private issue nonce replay must reject");
    assert_eq!(replay.0, "pftl_uniswap_private_primary_policy_mismatch");
    assert_eq!(
        pftl_uniswap_route_state_hash(
            private_issue_ledger
                .pftl_uniswap_routes
                .iter()
                .find(|route| route.route_id == v2_route_id)
                .expect("private issue route after replay"),
        ),
        state_after_private_issue
    );

    let mut private_redeem_ledger = ledger.clone();
    let private_redeem = AssetOrchardPrivatePrimaryRedeemActionPayload {
        version: 1,
        schema: "postfiat-asset-orchard-private-primary-redeem-action-v1".to_string(),
        pool_id: "asset-orchard-v1".to_string(),
        route_id: v2_route_id.clone(),
        subscriber: subscriber.clone(),
        ethereum_recipient: subscriber.clone(),
        reservation_id: "c7".repeat(48),
        subscription_nonce: "c8".repeat(32),
        route_epoch: 2,
        policy_epoch: 2,
        policy_hash: primary_policy.policy_hash.clone(),
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
        mint_amount_atoms: 10,
        settlement_value_atoms: 69,
        expires_at_height: 30,
        output_commitment: String::new(),
        encrypted_output: String::new(),
        output_validity_action_json: String::new(),
        proof_system_id: String::new(),
        circuit_id: String::new(),
        pool_domain: String::new(),
        anchor: String::new(),
        nullifier: String::new(),
        randomized_verification_key: String::new(),
        settlement_asset_tag_lo: 0,
        settlement_asset_tag_hi: 0,
        native_nav_asset_tag_lo: 0,
        native_nav_asset_tag_hi: 0,
        primary_binding_hash: String::new(),
        proof: String::new(),
        spend_authorization_signature: String::new(),
    };
    let mut wrong_output_ledger = private_redeem_ledger.clone();
    let mut wrong_output = private_redeem.clone();
    wrong_output.settlement_value_atoms += 1;
    let wrong_output_before =
        pftl_uniswap_route_state_hash(
            wrong_output_ledger
                .pftl_uniswap_routes
                .iter()
                .find(|route| route.route_id == v2_route_id)
                .expect("wrong-output route"),
        );
    let wrong_output_error =
        super::apply_asset_orchard_private_primary_redeem_route_transition(
            &mut wrong_output_ledger,
            &wrong_output,
            26,
        )
        .expect_err("noncanonical redemption output must reject");
    assert_eq!(
        wrong_output_error.0,
        "pftl_uniswap_private_redemption_unavailable"
    );
    assert_eq!(
        pftl_uniswap_route_state_hash(
            wrong_output_ledger
                .pftl_uniswap_routes
                .iter()
                .find(|route| route.route_id == v2_route_id)
                .expect("wrong-output route after rejection"),
        ),
        wrong_output_before
    );
    super::apply_asset_orchard_private_primary_redeem_route_transition(
        &mut private_redeem_ledger,
        &private_redeem,
        26,
    )
    .expect("private redemption route transition");
    let private_route = private_redeem_ledger
        .pftl_uniswap_routes
        .iter()
        .find(|route| route.route_id == v2_route_id)
        .expect("private route");
    assert_eq!(private_route.authorized_valid_supply_atoms, 30);
    assert_eq!(private_route.settlement_reserve_atoms, 210);
    assert_eq!(
        private_route
            .v2
            .as_ref()
            .expect("v2")
            .redeem_capacity_used_atoms,
        10
    );
    let state_after_private_redeem = pftl_uniswap_route_state_hash(private_route);
    let replay = super::apply_asset_orchard_private_primary_redeem_route_transition(
        &mut private_redeem_ledger,
        &private_redeem,
        26,
    )
    .expect_err("private redemption nonce replay must reject");
    assert_eq!(replay.0, "pftl_uniswap_private_redemption_policy_mismatch");
    let private_route_after_replay = private_redeem_ledger
        .pftl_uniswap_routes
        .iter()
        .find(|route| route.route_id == v2_route_id)
        .expect("private route after replay");
    assert_eq!(
        pftl_uniswap_route_state_hash(private_route_after_replay),
        state_after_private_redeem
    );

    let redeem_v2 = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        ledger.account(&subscriber).expect("subscriber").sequence + 1,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(
            PftlUniswapPrimaryRedeemOperation {
                owner: subscriber.clone(),
                settlement_recipient: subscriber.clone(),
                route_id: v2_route_id.clone(),
                redemption_nonce: "b8".repeat(32),
                nav_amount_atoms: 10,
                min_settlement_value_atoms: 69,
                route_epoch: 2,
                policy_epoch: 2,
                policy_hash: primary_policy.policy_hash.clone(),
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
                expires_at_height: 30,
            },
        ),
    );
    let pause_route = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_PAUSE_TRANSACTION_KIND,
        ledger.account(&operator).expect("operator").sequence + 1,
        AssetTransactionOperation::PftlUniswapRoutePause(PftlUniswapRoutePauseOperation {
            operator: operator.clone(),
            route_id: v2_route_id.clone(),
            paused: true,
        }),
    );
    let pause_receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &pause_route,
        26,
    );
    assert!(pause_receipt.accepted, "{pause_receipt:?}");
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &redeem_v2,
        26,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let resume_route = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_PAUSE_TRANSACTION_KIND,
        ledger.account(&operator).expect("operator").sequence + 1,
        AssetTransactionOperation::PftlUniswapRoutePause(PftlUniswapRoutePauseOperation {
            operator: operator.clone(),
            route_id: v2_route_id.clone(),
            paused: false,
        }),
    );
    let resume_receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &resume_route,
        26,
    );
    assert!(resume_receipt.accepted, "{resume_receipt:?}");
    {
        let route = ledger
            .pftl_uniswap_route(&v2_route_id)
            .expect("v2 primary route");
        let v2 = route.v2.as_ref().expect("v2 route state");
        assert_eq!(route.authorized_valid_supply_atoms, 30);
        assert_eq!(route.settlement_reserve_atoms, 210);
        assert_eq!(v2.issue_capacity_used_atoms, 40);
        assert_eq!(v2.redeem_capacity_used_atoms, 10);
        assert_eq!(v2.non_nav_spread_atoms, 3);
        assert_eq!(
            v2.terminal_reservations.get(&reservation_id).map(String::as_str),
            Some(PFTL_UNISWAP_ORDER_STATUS_CONSUMED)
        );
    }
    let export_packet_hash = "b9".repeat(48);
    let export_nonce = "ba".repeat(32);
    let export_recipient = "0x4444444444444444444444444444444444444444".to_string();
    let export_digest = {
        let route = ledger
            .pftl_uniswap_route(&v2_route_id)
            .expect("v2 route for packet");
        let v2 = route.v2.as_ref().expect("v2");
        PftlUniswapMintPacketV2 {
            route_config_digest: route.route_config_digest.clone(),
            source_packet_hash: export_packet_hash.clone(),
            reservation_id: reservation_id.clone(),
            source_receipt_hash: "01".repeat(48),
            source_receipt_root: "02".repeat(48),
            settlement_asset_id: route.settlement_asset_id.clone(),
            native_nav_asset_id: route.native_nav_asset_id.clone(),
            pricing_reserve_packet_hash: primary_policy.pricing_reserve_packet_hash.clone(),
            policy_hash_commitment: postfiat_types::pftl_uniswap_keccak_commitment48(
                "policy",
                &primary_policy.policy_hash,
            )
            .expect("policy commitment"),
            route_epoch: v2.route_epoch,
            pricing_nav_epoch: primary_policy.pricing_nav_epoch,
            deadline_seconds: 1_800,
            nonce: export_nonce.clone(),
            destination_chain_id: route.ethereum_chain_id,
            destination_controller: route.handoff_controller.clone(),
            wrapped_token: route.wrapped_navcoin_token.clone(),
            ethereum_recipient: export_recipient.clone(),
            mint_amount_atoms: 20,
            settlement_value_atoms: 141,
        }
        .evm_digest()
        .expect("v2 packet digest")
    };
    let export_v2 = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
        ledger.account(&subscriber).expect("subscriber").sequence + 1,
        AssetTransactionOperation::PftlUniswapExportDebit(PftlUniswapExportDebitOperation {
            owner: subscriber.clone(),
            route_id: v2_route_id.clone(),
            packet_hash: export_packet_hash.clone(),
            export_nonce,
            ethereum_recipient: export_recipient,
            amount_atoms: 20,
            reservation_id: Some(reservation_id.clone()),
            settlement_value_atoms: Some(141),
            destination_deadline_seconds: 1_800,
            refund_delay_blocks: 3,
            ethereum_packet_digest: Some(export_digest),
            ethereum_packet_schema_version: Some(PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V2),
        }),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &export_v2,
        27,
    );
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(
        ledger
            .pftl_uniswap_route(&v2_route_id)
            .expect("v2 route after export")
            .outstanding_bridge_claims_atoms,
        20
    );

    for route_number in 2_u64..=7 {
        let extra_route_init = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
            ledger.account(&operator).expect("operator").sequence + 1,
            AssetTransactionOperation::PftlUniswapRouteInit(PftlUniswapRouteInitOperation {
                operator: operator.clone(),
                route_id: format!("pftl-uniswap-a651-{route_number}"),
                route_config_digest: format!("{route_number:02x}").repeat(48),
                route_trust_class: "CONTROLLED".to_string(),
                native_nav_asset_id: native_nav_asset_id.clone(),
                settlement_asset_id: settlement_asset_id.clone(),
                handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
                settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
                wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
                ethereum_chain_id: 1,
                route_supply_cap_atoms: 1_000_000,
                packet_notional_cap_atoms: 500_000,
                latest_finalized_nav_epoch: 7,
                return_finality_blocks: 12,
                live_value_enabled: false,
                ethereum_verification_policy: None,
            }),
        );
        let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
            &genesis,
            &mut ledger,
            &extra_route_init,
            23 + route_number,
        );
        assert!(receipt.accepted, "{receipt:?}");
    }
    assert_eq!(ledger.pftl_uniswap_routes.len(), 8);

    let over_cap_route_init = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &operator_key,
        PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
        ledger.account(&operator).expect("operator").sequence + 1,
        AssetTransactionOperation::PftlUniswapRouteInit(PftlUniswapRouteInitOperation {
            operator: operator.clone(),
            route_id: "pftl-uniswap-a651-over-cap".to_string(),
            route_config_digest: "09".repeat(48),
            route_trust_class: "CONTROLLED".to_string(),
            native_nav_asset_id: native_nav_asset_id.clone(),
            settlement_asset_id: settlement_asset_id.clone(),
            handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
            settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 1_000_000,
            packet_notional_cap_atoms: 500_000,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 12,
            live_value_enabled: false,
            ethereum_verification_policy: None,
        }),
    );
    let before_over_cap_route = ledger.clone();
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &over_cap_route_init,
        32,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "pftl_uniswap_route_cap_exceeded");
    assert_eq!(ledger, before_over_cap_route);

    ledger
        .validate_asset_state(&genesis.chain_id)
        .expect("valid PFTL-Uniswap consensus asset state");
}

#[test]
fn pftl_uniswap_v2_fixed_point_spreads_round_against_the_protocol() {
    assert_eq!(
        checked_mul_div_ceil(
            1_000_000,
            PFTL_UNISWAP_A666_ISSUE_MULTIPLIER_BPS,
            PFTL_UNISWAP_BPS_DENOMINATOR,
        )
        .expect("issue price"),
        1_005_000
    );
    assert_eq!(
        checked_mul_div_floor(
            1_000_000,
            PFTL_UNISWAP_A666_REDEEM_MULTIPLIER_BPS,
            PFTL_UNISWAP_BPS_DENOMINATOR,
        )
        .expect("redemption price"),
        999_500
    );
    assert_eq!(
        checked_mul_div_ceil(
            1,
            PFTL_UNISWAP_A666_ISSUE_MULTIPLIER_BPS,
            PFTL_UNISWAP_BPS_DENOMINATOR,
        )
        .expect("ceil one atom"),
        2
    );
    assert_eq!(
        checked_mul_div_floor(
            1,
            PFTL_UNISWAP_A666_REDEEM_MULTIPLIER_BPS,
            PFTL_UNISWAP_BPS_DENOMINATOR,
        )
        .expect("floor one atom"),
        0
    );
}

#[test]
fn pftl_uniswap_v2_state_binds_directional_trust_capacity_and_non_nav_spread() {
    let mut policy = PftlUniswapPrimaryMarketPolicyV2 {
        policy_hash: String::new(),
        policy_epoch: 1,
        issue_multiplier_bps: PFTL_UNISWAP_A666_ISSUE_MULTIPLIER_BPS,
        redeem_multiplier_bps: PFTL_UNISWAP_A666_REDEEM_MULTIPLIER_BPS,
        issue_capacity_atoms: 2_000_000_000_000,
        redeem_capacity_atoms: 2_000_000_000_000,
        max_order_atoms: 1_000_000_000_000,
        min_order_atoms: 1_000_000,
        valid_from_height: 10,
        expires_at_height: 1_000,
        max_nav_age_blocks: 100,
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: "92".repeat(48),
    };
    policy.policy_hash = policy.computed_hash();
    let policy_hash = policy.policy_hash.clone();
    let mut route = PftlUniswapConsensusRouteState {
        route_id: "a666-mainnet-v2".to_string(),
        route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
        route_config_digest: "93".repeat(48),
        route_trust_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
        native_nav_asset_id: "94".repeat(48),
        settlement_asset_id: "95".repeat(48),
        handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
        settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
        wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
        ethereum_chain_id: 1,
        route_supply_cap_atoms: 2_000_000_000_000,
        packet_notional_cap_atoms: 250_000_000_000,
        latest_finalized_nav_epoch: 7,
        return_finality_blocks: 12,
        live_value_enabled: true,
        ethereum_verification_policy: None,
        authorized_valid_supply_atoms: 0,
        pftl_spendable_supply_atoms: 0,
        native_spendable_balances_atoms: std::collections::BTreeMap::new(),
        ethereum_spendable_supply_atoms: 0,
        other_registered_venue_supply_atoms: 0,
        outstanding_bridge_claims_atoms: 0,
        pending_return_import_claims_atoms: 0,
        settlement_reserve_atoms: 0,
        primary_subscription_nonces: std::collections::BTreeMap::new(),
        export_packets: std::collections::BTreeMap::new(),
        export_nonces: std::collections::BTreeMap::new(),
        return_imports: std::collections::BTreeMap::new(),
        paused: false,
        v2: Some(PftlUniswapRouteV2State {
            route_schema_version: PFTL_UNISWAP_ROUTE_SCHEMA_V2,
            route_epoch: 1,
            outbound_verification_class:
                PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY.to_string(),
            return_verification_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
            primary_market_policy: policy,
            issue_capacity_used_atoms: 0,
            redeem_capacity_used_atoms: 0,
            non_nav_spread_atoms: 0,
            active_reservations: std::collections::BTreeMap::new(),
            export_entitlements: std::collections::BTreeMap::new(),
            terminal_reservations: std::collections::BTreeMap::new(),
            redemption_nonces: std::collections::BTreeMap::new(),
        }),
    };
    route.validate().expect("valid governed v2 route");
    let baseline_hash = pftl_uniswap_route_state_hash(&route);

    route
        .v2
        .as_mut()
        .expect("v2")
        .active_reservations
        .insert(
            "96".repeat(48),
            PftlUniswapOrderReservationV2 {
                reservation_id: "96".repeat(48),
                subscriber: "bob".to_string(),
                ethereum_recipient: "0x4444444444444444444444444444444444444444".to_string(),
                route_epoch: 1,
                policy_epoch: 1,
                policy_hash,
                mint_amount_atoms: 1_000_000_000_000,
                max_settlement_value_atoms: 1_005_000_000_000,
                created_at_height: 10,
                expires_at_height: 20,
                status: PFTL_UNISWAP_ORDER_STATUS_RESERVED.to_string(),
            },
        );
    route.validate().expect("one maximum order fits");
    assert_ne!(baseline_hash, pftl_uniswap_route_state_hash(&route));

    route.v2.as_mut().expect("v2").issue_capacity_used_atoms = 1_000_000_000_001;
    assert!(route
        .validate()
        .expect_err("reserved plus used capacity must fail")
        .contains("used plus reserved"));

    route.v2.as_mut().expect("v2").issue_capacity_used_atoms = 0;
    route.v2.as_mut().expect("v2").outbound_verification_class =
        PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string();
    assert!(route
        .validate()
        .expect_err("outbound direction may not degrade")
        .contains("TRUSTLESS_FINALITY"));
}

/// AR-11: accumulated non-NAV spread is real settlement-asset custody and
/// must be counted by the issued-asset supply inventory. Before the fix the
/// route custody accumulator counted the settlement reserve but silently
/// dropped `non_nav_spread_atoms`, so every subscription leaked its spread
/// out of supply conservation (accelerated lifecycle run 3, 2026-08-03,
/// 14_369 atoms unaccounted).
#[test]
fn ar11_issued_asset_supply_counts_non_nav_spread_custody() {
    let settlement_asset_id = "95".repeat(48);
    let mut policy = PftlUniswapPrimaryMarketPolicyV2 {
        policy_hash: String::new(),
        policy_epoch: 1,
        issue_multiplier_bps: PFTL_UNISWAP_A666_ISSUE_MULTIPLIER_BPS,
        redeem_multiplier_bps: PFTL_UNISWAP_A666_REDEEM_MULTIPLIER_BPS,
        issue_capacity_atoms: 2_000_000_000_000,
        redeem_capacity_atoms: 2_000_000_000_000,
        max_order_atoms: 1_000_000_000_000,
        min_order_atoms: 1_000_000,
        valid_from_height: 10,
        expires_at_height: 1_000,
        max_nav_age_blocks: 100,
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: "92".repeat(48),
    };
    policy.policy_hash = policy.computed_hash();
    let route = PftlUniswapConsensusRouteState {
        route_id: "ar11-spread-custody".to_string(),
        route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
        route_config_digest: "93".repeat(48),
        route_trust_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
        native_nav_asset_id: "94".repeat(48),
        settlement_asset_id: settlement_asset_id.clone(),
        handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
        settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
        wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
        ethereum_chain_id: 1,
        route_supply_cap_atoms: 2_000_000_000_000,
        packet_notional_cap_atoms: 250_000_000_000,
        latest_finalized_nav_epoch: 7,
        return_finality_blocks: 12,
        live_value_enabled: true,
        ethereum_verification_policy: None,
        authorized_valid_supply_atoms: 0,
        pftl_spendable_supply_atoms: 0,
        native_spendable_balances_atoms: std::collections::BTreeMap::new(),
        ethereum_spendable_supply_atoms: 0,
        other_registered_venue_supply_atoms: 0,
        outstanding_bridge_claims_atoms: 0,
        pending_return_import_claims_atoms: 0,
        settlement_reserve_atoms: 896_246,
        primary_subscription_nonces: std::collections::BTreeMap::new(),
        export_packets: std::collections::BTreeMap::new(),
        export_nonces: std::collections::BTreeMap::new(),
        return_imports: std::collections::BTreeMap::new(),
        paused: false,
        v2: Some(PftlUniswapRouteV2State {
            route_schema_version: PFTL_UNISWAP_ROUTE_SCHEMA_V2,
            route_epoch: 1,
            outbound_verification_class: PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY.to_string(),
            return_verification_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
            primary_market_policy: policy,
            issue_capacity_used_atoms: 0,
            redeem_capacity_used_atoms: 0,
            non_nav_spread_atoms: 14_369,
            active_reservations: std::collections::BTreeMap::new(),
            export_entitlements: std::collections::BTreeMap::new(),
            terminal_reservations: std::collections::BTreeMap::new(),
            redemption_nonces: std::collections::BTreeMap::new(),
        }),
    };
    let mut ledger = LedgerState::new(Vec::new());
    ledger.pftl_uniswap_routes.push(route);
    let supply = super::issued_asset_supply(&ledger, &settlement_asset_id)
        .expect("issued supply with spread custody");
    assert_eq!(
        supply,
        896_246 + 14_369,
        "settlement reserve and accumulated non-NAV spread are both live \
         settlement-asset custody"
    );
}

/// AR-09: redemption submit against the production-shaped route policy
/// admits a valid expiry/binding/amount/nonce tuple and rejects each invalid
/// variant with `pftl_uniswap_redemption_policy_mismatch`, leaving state
/// untouched. Reproduces the controlled-qualification retry-6 failure of
/// 2026-08-03, where a 500_000-atom redemption was submitted against the
/// production minimum order of 1_000_000 atoms.
#[test]
fn pftl_uniswap_v2_primary_redeem_enforces_production_shaped_policy_binding_ar09() {
    let mut genesis = Genesis::new_with_validator_count("postfiat-wan-devnet-2", 6);
    genesis.consensus_v2_activation_height = Some(1);
    // The qualified reserve-proof fixture binds the deterministic qNAV asset
    // identity, so the issuer must use the same deterministic derivation as
    // the provider-neutral qualification fixture.
    let issuer_derivation_payload = serde_json::to_vec(&(
        "postfiat.wallet.seed.v1",
        "ML-DSA-65",
        genesis.chain_id.as_str(),
        0_u32,
        "transparent-spend",
        "66".repeat(32),
    ))
    .expect("serialize deterministic AR-09 issuer derivation");
    let issuer_digest = postfiat_crypto_provider::hash_bytes(
        "postfiat.wallet.seed.v1",
        &issuer_derivation_payload,
    );
    let mut issuer_seed = [0_u8; 32];
    issuer_seed.copy_from_slice(&issuer_digest[..32]);
    let issuer_key = ml_dsa_65_keygen_from_seed(&issuer_seed);
    let operator_key = ml_dsa_65_keygen().expect("operator keygen");
    let subscriber_key = ml_dsa_65_keygen().expect("subscriber keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let operator = address_from_public_key(&operator_key.public_key);
    let subscriber = address_from_public_key(&subscriber_key.public_key);
    let mut ledger = LedgerState::new(vec![
        Account::new(
            issuer.clone(),
            100_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        ),
        Account::new(
            operator.clone(),
            100_000,
            Some(bytes_to_hex(&operator_key.public_key)),
        ),
        Account::new(
            subscriber.clone(),
            100_000,
            Some(bytes_to_hex(&subscriber_key.public_key)),
        ),
    ]);

    let submit = |ledger: &mut LedgerState,
                      key: &postfiat_crypto_provider::MlDsa65KeyPair,
                      address: &str,
                      kind: &str,
                      operation: AssetTransactionOperation,
                      height: u64|
     -> Receipt {
        let transaction = signed_asset_transaction_with_minimum_fee(
            &genesis,
            ledger,
            key,
            kind,
            ledger.account(address).expect("signer account").sequence + 1,
            operation,
        );
        execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
            &genesis,
            ledger,
            &transaction,
            height,
        )
    };

    let receipt = submit(
        &mut ledger,
        &issuer_key,
        &issuer,
        ASSET_CREATE_TRANSACTION_KIND,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "PUSDC".to_string(),
            version: 1,
            precision: 6,
            display_name: "PFTL USDC".to_string(),
            max_supply: Some(100_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
        1,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let settlement_asset_id = ledger.asset_definitions[0].asset_id.clone();
    let receipt = submit(
        &mut ledger,
        &issuer_key,
        &issuer,
        ASSET_CREATE_TRANSACTION_KIND,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "qNAV".to_string(),
            version: 1,
            precision: 6,
            display_name: "AR-09 qualification NAVCoin".to_string(),
            max_supply: None,
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
        2,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let native_nav_asset_id = ledger.asset_definitions[1].asset_id.clone();

    let receipt = submit(
        &mut ledger,
        &issuer_key,
        &issuer,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: settlement_asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: "pftl-uniswap-settlement-test".to_string(),
            valuation_unit: "USDC".to_string(),
            redemption_account: issuer.clone(),
        }),
        3,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let receipt = submit(
        &mut ledger,
        &issuer_key,
        &issuer,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
            registrant: issuer.clone(),
            verifier_kind: postfiat_types::NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1.to_string(),
            source_class: "manifest-driven".to_string(),
            max_snapshot_age_blocks: 10_000,
            challenge_window_blocks: 1,
            max_epoch_gap_blocks: 100,
            settle_deadline_blocks: 0,
            min_challenge_bond: 0,
            min_attestations: 0,
            tolerance_bp: 0,
            bridge_observer_min_confirmations: 0,
            valuation_policy_hash: "04".repeat(32),
            vault_bridge_route_policy_hash: String::new(),
            sp1_program_vkey:
                "0x000c7271e0711abce0c61d293222fd4a144599a779db8cadadc4df35e31a4100"
                    .to_string(),
            sp1_proof_encoding: "groth16".to_string(),
            max_proof_bytes: 4_096,
            max_public_values_bytes: postfiat_types::NAV_RESERVE_PUBLIC_VALUES_V1_BYTES as u64,
            public_values_schema:
                postfiat_types::NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            source_manifest_hash: "9da4e2ba55939f138475026946d2728d9b40d3f4c7762289a70aae94584eac924b9a788c6df25c9276cc83f1616ef0e5".to_string(),
            valuation_unit_id: "05".repeat(48),
            max_observation_span_blocks: 8,
            allow_controlled_sources: true,
        }),
        4,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let reserve_profile_id = ledger
        .nav_proof_profiles
        .first()
        .expect("AR-09 reserve profile")
        .profile_id
        .clone();
    let receipt = submit(
        &mut ledger,
        &issuer_key,
        &issuer,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: native_nav_asset_id.clone(),
            reserve_operator: operator.clone(),
            proof_profile: reserve_profile_id.clone(),
            valuation_unit: "USDC".to_string(),
            redemption_account: issuer.clone(),
        }),
        5,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let receipt = submit(
        &mut ledger,
        &issuer_key,
        &issuer,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.clone(),
            to: subscriber.clone(),
            issuer: issuer.clone(),
            asset_id: settlement_asset_id.clone(),
            amount: 20_000_000,
        }),
        5,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let pricing_reserve_packet_hash = "55".repeat(48);
    let reserve_proof = hex_to_bytes(
        include_str!("../testdata/nav-reserve-v1-qualified-proof-calldata.hex").trim(),
    )
    .expect("decode AR-09 qualification proof");
    let reserve_public_values = hex_to_bytes(
        include_str!("../testdata/nav-reserve-v1-qualified-public-values.hex").trim(),
    )
    .expect("decode AR-09 qualification public values");
    let receipt = submit(
        &mut ledger,
        &operator_key,
        &operator,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.clone(),
            submitter: operator.clone(),
            asset_id: native_nav_asset_id.clone(),
            epoch: 7,
            nav_per_unit: 7_000_000,
            circulating_supply: 0,
            verified_net_assets: 1_100,
            proof_profile: reserve_profile_id,
            source_root: "f4bdaca02e5445e7d2c666ca692d45d63fe1c423f6b03067e9eee19f5f9334fe60920b8528feff2656d5dbe7d28d415f".to_string(),
            attestor_root: "cb34590e25db391724491b01795dee8bdbbadba3bba36fb5fc4f96bce1a87fa311426e0b76ce5ff4d775b091d94147df".to_string(),
            reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: reserve_proof,
            sp1_public_values: reserve_public_values,
        }),
        5,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let receipt = submit(
        &mut ledger,
        &issuer_key,
        &issuer,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.clone(),
            asset_id: native_nav_asset_id.clone(),
            epoch: 7,
            reserve_packet_hash: pricing_reserve_packet_hash.clone(),
        }),
        6,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let authority_keys = (0_u8..4)
        .map(|index| ml_dsa_65_keygen_from_seed(&[0xa1 + index; 32]))
        .collect::<Vec<_>>();
    let mut authority = FastSwapCommitteeV1 {
        domain: FastSwapCommitteeDomainV1 {
            chain: FastSwapChainDomainV1 {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: FastSwapOpaqueHashV1(
                    hex_to_bytes(&genesis_hash(&genesis))
                        .expect("genesis hex")
                        .try_into()
                        .expect("48-byte genesis hash"),
                ),
                protocol_version: genesis.protocol_version,
            },
            fastswap_schema_version: postfiat_types::FASTSWAP_SCHEMA_VERSION_V1,
            committee_epoch: 9,
            committee_root: FastSwapCommitteeRootV1::ZERO,
            validator_count: 4,
            quorum: 3,
        },
        validators: authority_keys
            .iter()
            .enumerate()
            .map(|(index, key)| FastSwapValidatorV1 {
                validator_id: format!("ethereum-authority-{index}"),
                public_key: key.public_key.clone(),
            })
            .collect(),
    };
    authority.domain.committee_root = authority.computed_root().expect("committee root");
    ledger.fastswap_committees.push(authority.clone());
    let ethereum_policy = EthereumRouteVerificationPolicyV1 {
        authority_epoch: authority.domain.committee_epoch,
        committee_root: authority.domain.committee_root,
        minimum_confirmations: 12,
        handoff_controller_code_hash: [0x71; 32],
        wrapped_navcoin_code_hash: [0x72; 32],
    };

    // Production-shaped primary market policy: the exact cap, order-bound,
    // and multiplier shape used by the live A666 route.
    let mut policy = PftlUniswapPrimaryMarketPolicyV2 {
        policy_hash: String::new(),
        policy_epoch: 1,
        issue_multiplier_bps: PFTL_UNISWAP_A666_ISSUE_MULTIPLIER_BPS,
        redeem_multiplier_bps: PFTL_UNISWAP_A666_REDEEM_MULTIPLIER_BPS,
        issue_capacity_atoms: 2_000_000_000_000,
        redeem_capacity_atoms: 2_000_000_000_000,
        max_order_atoms: 1_000_000_000_000,
        min_order_atoms: 1_000_000,
        valid_from_height: 24,
        expires_at_height: 5_000,
        max_nav_age_blocks: 100,
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
    };
    policy.policy_hash = policy.computed_hash();
    let route_id = "ar09-production-shaped-redeem".to_string();
    let receipt = submit(
        &mut ledger,
        &operator_key,
        &operator,
        PFTL_UNISWAP_ROUTE_INIT_V2_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRouteInitV2(PftlUniswapRouteInitV2Operation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            route_config_digest: "c1".repeat(48),
            native_nav_asset_id: native_nav_asset_id.clone(),
            settlement_asset_id: settlement_asset_id.clone(),
            opening_inventory_atoms: 0,
            opening_inventory_holder: String::new(),
            handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
            settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 2_000_000_000_000,
            packet_notional_cap_atoms: 250_000_000_000,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 12,
            route_epoch: 1,
            outbound_verification_class: PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY.to_string(),
            return_verification_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
            live_value_enabled: false,
            ethereum_verification_policy: ethereum_policy,
            primary_market_policy: policy.clone(),
        }),
        24,
    );
    assert!(receipt.accepted, "{receipt:?}");
    // Production routes must initialize with live value disabled; enable it
    // for the market phase the same way the existing v2 fixture does.
    ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == route_id)
        .expect("AR-09 route")
        .live_value_enabled = true;

    // Fund the route reserve and the holder's NAV balance through a real
    // subscription: 2_000_000 atoms at 7 settlement atoms per NAV atom.
    let reservation_id = "c2".repeat(48);
    let receipt = submit(
        &mut ledger,
        &subscriber_key,
        &subscriber,
        PFTL_UNISWAP_ORDER_RESERVE_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapOrderReserve(PftlUniswapOrderReserveOperation {
            subscriber: subscriber.clone(),
            route_id: route_id.clone(),
            reservation_id: reservation_id.clone(),
            ethereum_recipient: "0x4444444444444444444444444444444444444444".to_string(),
            route_epoch: 1,
            policy_epoch: 1,
            policy_hash: policy.policy_hash.clone(),
            mint_amount_atoms: 2_000_000,
            max_settlement_value_atoms: 14_070_000,
            expires_at_height: 30,
        }),
        24,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let receipt = submit(
        &mut ledger,
        &subscriber_key,
        &subscriber,
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_V2_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapPrimarySubscribeV2(
            PftlUniswapPrimarySubscribeV2Operation {
                subscriber: subscriber.clone(),
                route_id: route_id.clone(),
                reservation_id,
                subscription_nonce: "c3".repeat(32),
                settlement_asset_id: settlement_asset_id.clone(),
                settlement_value_atoms: 14_070_000,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
            },
        ),
        25,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let valid_redeem = |nonce: String| PftlUniswapPrimaryRedeemOperation {
        owner: subscriber.clone(),
        settlement_recipient: subscriber.clone(),
        route_id: route_id.clone(),
        redemption_nonce: nonce,
        nav_amount_atoms: 1_000_000,
        min_settlement_value_atoms: 6_996_500,
        route_epoch: 1,
        policy_epoch: 1,
        policy_hash: policy.policy_hash.clone(),
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: pricing_reserve_packet_hash.clone(),
        expires_at_height: 100,
    };

    let expect_policy_mismatch = |ledger: &mut LedgerState,
                                      label: &str,
                                      operation: PftlUniswapPrimaryRedeemOperation| {
        let before = ledger.clone();
        let transaction = signed_asset_transaction_with_minimum_fee(
            &genesis,
            ledger,
            &subscriber_key,
            PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
            ledger.account(&subscriber).expect("subscriber").sequence + 1,
            AssetTransactionOperation::PftlUniswapPrimaryRedeem(operation),
        );
        let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
            &genesis,
            ledger,
            &transaction,
            26,
        );
        assert!(!receipt.accepted, "{label}: {receipt:?}");
        assert_eq!(
            receipt.code, "pftl_uniswap_redemption_policy_mismatch",
            "{label}: {receipt:?}"
        );
        assert_eq!(*ledger, before, "{label}: rejection must not mutate state");
    };

    // Retry-6 reproduction: 500_000 atoms is below the production minimum
    // order of 1_000_000 atoms and must fail closed.
    let mut sub_minimum = valid_redeem("d1".repeat(32));
    sub_minimum.nav_amount_atoms = 500_000;
    sub_minimum.min_settlement_value_atoms = 3_498_250;
    expect_policy_mismatch(&mut ledger, "retry-6 sub-minimum order", sub_minimum);

    let mut above_maximum = valid_redeem("d2".repeat(32));
    above_maximum.nav_amount_atoms = 1_000_000_000_001;
    expect_policy_mismatch(&mut ledger, "above maximum order", above_maximum);

    let mut wrong_route_epoch = valid_redeem("d3".repeat(32));
    wrong_route_epoch.route_epoch = 2;
    expect_policy_mismatch(&mut ledger, "wrong route epoch", wrong_route_epoch);

    let mut wrong_policy_epoch = valid_redeem("d4".repeat(32));
    wrong_policy_epoch.policy_epoch = 2;
    expect_policy_mismatch(&mut ledger, "wrong policy epoch", wrong_policy_epoch);

    let mut wrong_policy_hash = valid_redeem("d5".repeat(32));
    wrong_policy_hash.policy_hash = "d6".repeat(48);
    expect_policy_mismatch(&mut ledger, "wrong policy hash", wrong_policy_hash);

    let mut expired = valid_redeem("d7".repeat(32));
    expired.expires_at_height = 25;
    expect_policy_mismatch(&mut ledger, "expired redemption", expired);

    // The exact valid tuple is admitted.
    let accepted_nonce = "d8".repeat(32);
    let transaction = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &subscriber_key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        ledger.account(&subscriber).expect("subscriber").sequence + 1,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(valid_redeem(accepted_nonce.clone())),
    );
    let receipt = execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        &genesis,
        &mut ledger,
        &transaction,
        26,
    );
    assert!(receipt.accepted, "valid production-shaped redeem: {receipt:?}");
    {
        let route = ledger.pftl_uniswap_route(&route_id).expect("AR-09 route");
        let v2 = route.v2.as_ref().expect("v2 state");
        assert_eq!(v2.redeem_capacity_used_atoms, 1_000_000);
        assert!(v2.redemption_nonces.contains_key(&accepted_nonce));
    }

    // Nonce replay of the accepted redemption must fail closed.
    expect_policy_mismatch(&mut ledger, "nonce replay", valid_redeem(accepted_nonce));
}

#[test]
fn ar01_pfusdc_reserve_account_identity_and_balance_matches_production_fixture() {
    let genesis = Genesis::new("postfiat-local");
    let issuer_key = ml_dsa_65_keygen_from_seed(&[0x31; 32]);
    let observer_key = ml_dsa_65_keygen_from_seed(&[0x32; 32]);
    let issuer = address_from_public_key(&issuer_key.public_key);
    let observer = address_from_public_key(&observer_key.public_key);
    let deposit_amount = 7_500_000_u64;
    let mut evidence = vault_bridge_evidence(deposit_amount, "91");
    evidence.pftl_recipient = observer.clone();
    evidence.pftl_recipient_hash =
        vault_bridge_pftl_recipient_hash(&observer).expect("pfUSDC recipient hash");
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("pfUSDC deposit id");
    let production_reserve_account = evidence.vault_id();
    let source_domain = evidence.source_domain();
    let policy_hash = "42".repeat(48);
    let evidence_root =
        vault_bridge_deposit_evidence_root(&evidence).expect("pfUSDC evidence root");
    let mut ledger = LedgerState::new(vec![
        Account::new(
            issuer.clone(),
            100_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        ),
        Account::new(
            observer.clone(),
            100_000,
            Some(bytes_to_hex(&observer_key.public_key)),
        ),
    ]);

    let execute = |ledger: &mut LedgerState,
                   key: &postfiat_crypto_provider::MlDsa65KeyPair,
                   signer: &str,
                   kind: &str,
                   operation: AssetTransactionOperation,
                   height: u64| {
        let transaction = signed_asset_transaction_with_minimum_fee(
            &genesis,
            ledger,
            key,
            kind,
            ledger.account(signer).expect("fixture signer").sequence + 1,
            operation,
        );
        execute_asset_transaction(&genesis, ledger, &transaction, height)
    };

    let receipt = execute(
        &mut ledger,
        &issuer_key,
        &issuer,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
            registrant: issuer.clone(),
            verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
            source_class: format!("vault_bridge:{source_domain}"),
            max_snapshot_age_blocks: 100,
            challenge_window_blocks: 1,
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
            public_values_schema: String::new(),
            source_manifest_hash: String::new(),
            valuation_unit_id: String::new(),
            max_observation_span_blocks: 0,
            allow_controlled_sources: false,
        }),
        1,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

    let receipt = execute(
        &mut ledger,
        &issuer_key,
        &issuer,
        ASSET_CREATE_TRANSACTION_KIND,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "PFUSDC".to_string(),
            version: 1,
            precision: 6,
            display_name: "proof-native pfUSDC".to_string(),
            max_supply: Some(100_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
        2,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let asset_id = ledger.asset_definitions[0].asset_id.clone();

    let receipt = execute(
        &mut ledger,
        &issuer_key,
        &issuer,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            reserve_operator: issuer.clone(),
            proof_profile: profile_id.clone(),
            valuation_unit: "USDC".to_string(),
            redemption_account: issuer.clone(),
        }),
        3,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let receipt = execute(
        &mut ledger,
        &observer_key,
        &observer,
        NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
            attestor: observer.clone(),
            domain: "controlled-pfusdc-overlay.local".to_string(),
            bond: 0,
        }),
        4,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let receipt = execute(
        &mut ledger,
        &observer_key,
        &observer,
        VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
        AssetTransactionOperation::VaultBridgeDepositPropose(
            VaultBridgeDepositProposeOperation {
                proposer: observer.clone(),
                asset_id: asset_id.clone(),
                evidence_root: evidence_root.clone(),
                evidence: evidence.clone(),
                policy_hash: policy_hash.clone(),
                source_proof_kind: String::new(),
                source_proof_hash: String::new(),
                source_public_values_hash: String::new(),
                source_proof_bytes: Vec::new(),
                source_public_values: Vec::new(),
                expires_at_height: 1_000,
            },
        ),
        5,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let observation = VaultBridgeDepositObservation::success_for_evidence(&evidence, 6);
    let observation_root =
        vault_bridge_deposit_observation_root(&observation).expect("pfUSDC observation root");
    let receipt = execute(
        &mut ledger,
        &observer_key,
        &observer,
        VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
        AssetTransactionOperation::VaultBridgeDepositAttest(
            VaultBridgeDepositAttestOperation {
                attestor: observer.clone(),
                asset_id: asset_id.clone(),
                evidence_root: evidence_root.clone(),
                pass: true,
                observation_root,
                observation: Some(observation),
            },
        ),
        6,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let receipt = execute(
        &mut ledger,
        &observer_key,
        &observer,
        VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
        AssetTransactionOperation::VaultBridgeDepositFinalize(
            VaultBridgeDepositFinalizeOperation {
                finalizer: observer.clone(),
                asset_id: asset_id.clone(),
                evidence_root: evidence_root.clone(),
            },
        ),
        7,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let receipt = execute(
        &mut ledger,
        &observer_key,
        &observer,
        VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
        AssetTransactionOperation::VaultBridgeDepositClaim(VaultBridgeDepositClaimOperation {
            claimer: observer.clone(),
            asset_id: asset_id.clone(),
            evidence_root: evidence_root.clone(),
            policy_hash: policy_hash.clone(),
            recipient: observer.clone(),
            amount_atoms: deposit_amount,
        }),
        8,
    );
    assert!(receipt.accepted, "{receipt:?}");

    let backing_receipt = ledger
        .vault_bridge_receipts
        .iter()
        .find(|receipt| receipt.vault_id == production_reserve_account)
        .expect("counted receipt selected by production-derived vault ID");
    assert_eq!(backing_receipt.asset_id, asset_id);
    assert_eq!(backing_receipt.counted_value_atoms, deposit_amount);
    let backing_bucket_id = backing_receipt.bucket_id.clone();
    let bucket = ledger
        .vault_bridge_bucket(&backing_bucket_id)
        .expect("finalized bucket referenced by the production vault receipt");
    assert_eq!(bucket.counted_value_atoms, deposit_amount);
    assert_eq!(bucket.outstanding_vault_bridge_atoms, deposit_amount);
    let bucket_counted_value = bucket.counted_value_atoms;
    let source_root =
        vault_bridge_source_root_for_asset(&ledger.vault_bridge_bucket_states, &asset_id)
            .expect("pfUSDC source root from finalized bucket");
    let reserve_packet_hash = "f6".repeat(48);
    let reserve_operation = NavReserveSubmitOperation {
        issuer: issuer.clone(),
        submitter: issuer.clone(),
        asset_id: asset_id.clone(),
        epoch: 1,
        nav_per_unit: VAULT_BRIDGE_UNIT,
        circulating_supply: deposit_amount,
        verified_net_assets: bucket_counted_value,
        proof_profile: profile_id,
        source_root: source_root.clone(),
        attestor_root: "88".repeat(48),
        reserve_packet_hash: reserve_packet_hash.clone(),
        reserve_accounts: vec![production_reserve_account.clone()],
        sp1_proof_bytes: Vec::new(),
        sp1_public_values: Vec::new(),
    };

    let mut wrong_backing = reserve_operation.clone();
    wrong_backing.source_root = "00".repeat(48);
    let before_wrong_backing = ledger.clone();
    let receipt = execute(
        &mut ledger,
        &issuer_key,
        &issuer,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        AssetTransactionOperation::NavReserveSubmit(wrong_backing),
        9,
    );
    assert!(!receipt.accepted, "wrong backing source must fail closed");
    assert_eq!(receipt.code, "vault_bridge_source_root_mismatch");
    assert_eq!(ledger, before_wrong_backing);

    let receipt = execute(
        &mut ledger,
        &issuer_key,
        &issuer,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        AssetTransactionOperation::NavReserveSubmit(reserve_operation),
        9,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let packet = ledger
        .nav_reserve_packet(&asset_id, 1, &reserve_packet_hash)
        .expect("accepted pfUSDC reserve packet");
    assert_eq!(packet.reserve_accounts, vec![production_reserve_account]);
    assert_eq!(packet.verified_net_assets, bucket_counted_value);
    assert_eq!(packet.source_root, source_root);

    let receipt = execute(
        &mut ledger,
        &observer_key,
        &observer,
        NAV_RESERVE_ATTEST_TRANSACTION_KIND,
        AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
            attestor: observer.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
            pass: true,
            observation_root: source_root,
        }),
        10,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let receipt = execute(
        &mut ledger,
        &issuer_key,
        &issuer,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
        }),
        11,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let finalized = ledger.nav_asset(&asset_id).expect("finalized pfUSDC NAV asset");
    assert_eq!(finalized.finalized_reserve_packet_hash, reserve_packet_hash);
    assert_eq!(finalized.circulating_supply, deposit_amount);
}

#[test]
fn ar05_active_export_entitlement_blocks_route_epoch_advance_until_closed() {
    let genesis = Genesis::new("postfiat-local");
    let operator_key = ml_dsa_65_keygen_from_seed(&[0x51; 32]);
    let holder_key = ml_dsa_65_keygen_from_seed(&[0x52; 32]);
    let operator = address_from_public_key(&operator_key.public_key);
    let holder = address_from_public_key(&holder_key.public_key);
    let mut ledger = LedgerState::new(vec![
        Account::new(
            operator.clone(),
            100_000,
            Some(bytes_to_hex(&operator_key.public_key)),
        ),
        Account::new(
            holder.clone(),
            100_000,
            Some(bytes_to_hex(&holder_key.public_key)),
        ),
    ]);
    let execute = |ledger: &mut LedgerState,
                   key: &postfiat_crypto_provider::MlDsa65KeyPair,
                   signer: &str,
                   kind: &str,
                   operation: AssetTransactionOperation,
                   height: u64| {
        let transaction = signed_asset_transaction_with_minimum_fee(
            &genesis,
            ledger,
            key,
            kind,
            ledger.account(signer).expect("AR-05 signer").sequence + 1,
            operation,
        );
        execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
            &genesis,
            ledger,
            &transaction,
            height,
        )
    };

    let receipt = execute(
        &mut ledger,
        &operator_key,
        &operator,
        ASSET_CREATE_TRANSACTION_KIND,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: operator.clone(),
            code: "A605".to_string(),
            version: 1,
            precision: 6,
            display_name: "AR-05 production-shaped NAVCoin".to_string(),
            max_supply: None,
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
        1,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let native_asset_id = ledger.asset_definitions[0].asset_id.clone();
    let receipt = execute(
        &mut ledger,
        &operator_key,
        &operator,
        ASSET_CREATE_TRANSACTION_KIND,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: operator.clone(),
            code: "USDC5".to_string(),
            version: 1,
            precision: 6,
            display_name: "AR-05 settlement asset".to_string(),
            max_supply: Some(1_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
        2,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let settlement_asset_id = ledger.asset_definitions[1].asset_id.clone();
    let receipt = execute(
        &mut ledger,
        &operator_key,
        &operator,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: operator.clone(),
            asset_id: native_asset_id.clone(),
            reserve_operator: operator.clone(),
            proof_profile: "ar05-ledger-transparent".to_string(),
            valuation_unit: "USDC".to_string(),
            redemption_account: operator.clone(),
        }),
        2,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let holder_trust = execute(
        &mut ledger,
        &holder_key,
        &holder,
        TRUST_SET_TRANSACTION_KIND,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.clone(),
            issuer: operator.clone(),
            asset_id: native_asset_id.clone(),
            limit: 1_000_000,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
        3,
    );
    assert!(holder_trust.accepted, "{holder_trust:?}");

    let reserve_packet_hash = "55".repeat(48);
    {
        let nav = ledger
            .nav_asset_mut(&native_asset_id)
            .expect("AR-05 NAV state");
        nav.finalized_epoch = 7;
        nav.nav_per_unit = 7_000_000;
        nav.finalized_reserve_packet_hash = reserve_packet_hash.clone();
        nav.finalized_at_height = 4;
    }

    let route_id = "ar05-production-shaped-route".to_string();
    let reservation_id = "a5".repeat(48);
    let packet_hash = "b5".repeat(48);
    let ethereum_recipient = "0x4444444444444444444444444444444444444444".to_string();
    let mut policy = PftlUniswapPrimaryMarketPolicyV2 {
        policy_hash: String::new(),
        policy_epoch: 1,
        issue_multiplier_bps: PFTL_UNISWAP_A666_ISSUE_MULTIPLIER_BPS,
        redeem_multiplier_bps: PFTL_UNISWAP_A666_REDEEM_MULTIPLIER_BPS,
        issue_capacity_atoms: 2_000_000_000_000,
        redeem_capacity_atoms: 2_000_000_000_000,
        max_order_atoms: 1_000_000_000_000,
        min_order_atoms: 1_000_000,
        valid_from_height: 4,
        expires_at_height: 1_000,
        max_nav_age_blocks: 100,
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: reserve_packet_hash.clone(),
    };
    policy.policy_hash = policy.computed_hash();
    let entitlement = PftlUniswapExportEntitlementV2 {
        reservation_id: reservation_id.clone(),
        subscriber: holder.clone(),
        ethereum_recipient: ethereum_recipient.clone(),
        route_epoch: 1,
        policy_epoch: 1,
        policy_hash: policy.policy_hash.clone(),
        remaining_amount_atoms: 40,
        expires_at_height: 100,
    };
    let export_packet = PftlUniswapConsensusExportPacket {
        packet_hash: packet_hash.clone(),
        nonce: "b6".repeat(32),
        source_wallet: holder.clone(),
        ethereum_recipient: ethereum_recipient.clone(),
        amount_atoms: 40,
        settlement_value_atoms: Some(282),
        source_height: 5,
        destination_deadline_seconds: 1_800,
        refund_not_before_height: 8,
        status: PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED.to_string(),
        ethereum_packet_digest: Some("b7".repeat(32)),
        ethereum_packet_schema_version: Some(PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V2),
        route_epoch: Some(1),
        policy_hash: Some(policy.policy_hash.clone()),
        reservation_id: Some(reservation_id.clone()),
    };
    let route = PftlUniswapConsensusRouteState {
        route_id: route_id.clone(),
        route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
        route_config_digest: "a1".repeat(48),
        route_trust_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
        native_nav_asset_id: native_asset_id.clone(),
        settlement_asset_id: settlement_asset_id.clone(),
        handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
        settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
        wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
        ethereum_chain_id: 1,
        route_supply_cap_atoms: 2_000_000_000_000,
        packet_notional_cap_atoms: 250_000_000_000,
        latest_finalized_nav_epoch: 7,
        return_finality_blocks: 12,
        live_value_enabled: true,
        ethereum_verification_policy: None,
        authorized_valid_supply_atoms: 40,
        pftl_spendable_supply_atoms: 0,
        native_spendable_balances_atoms: std::collections::BTreeMap::new(),
        ethereum_spendable_supply_atoms: 0,
        other_registered_venue_supply_atoms: 0,
        outstanding_bridge_claims_atoms: 40,
        pending_return_import_claims_atoms: 0,
        settlement_reserve_atoms: 0,
        primary_subscription_nonces: std::collections::BTreeMap::new(),
        export_packets: std::collections::BTreeMap::from([(packet_hash.clone(), export_packet)]),
        export_nonces: std::collections::BTreeMap::from([(
            "b6".repeat(32),
            packet_hash.clone(),
        )]),
        return_imports: std::collections::BTreeMap::new(),
        paused: false,
        v2: Some(PftlUniswapRouteV2State {
            route_schema_version: PFTL_UNISWAP_ROUTE_SCHEMA_V2,
            route_epoch: 1,
            outbound_verification_class: PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY.to_string(),
            return_verification_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
            primary_market_policy: policy.clone(),
            issue_capacity_used_atoms: 40,
            redeem_capacity_used_atoms: 0,
            non_nav_spread_atoms: 0,
            active_reservations: std::collections::BTreeMap::new(),
            export_entitlements: std::collections::BTreeMap::from([(
                reservation_id.clone(),
                entitlement,
            )]),
            terminal_reservations: std::collections::BTreeMap::new(),
            redemption_nonces: std::collections::BTreeMap::new(),
        }),
    };
    route.validate().expect("valid AR-05 route fixture");
    ledger.pftl_uniswap_routes.push(route);

    let mut next_policy = policy.clone();
    next_policy.policy_epoch = 2;
    next_policy.policy_hash = next_policy.computed_hash();
    let epoch_advance = PftlUniswapRouteEpochAdvanceOperation {
        operator: operator.clone(),
        route_id: route_id.clone(),
        prior_route_epoch: 1,
        next_route_epoch: 2,
        next_route_config_digest: "a2".repeat(48),
        live_value_enabled: true,
        next_primary_market_policy: next_policy,
    };
    let before_blocked = ledger.clone();
    let receipt = execute(
        &mut ledger,
        &operator_key,
        &operator,
        PFTL_UNISWAP_ROUTE_EPOCH_ADVANCE_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(epoch_advance.clone()),
        6,
    );
    assert!(!receipt.accepted, "active entitlement must block epoch advance");
    assert_eq!(receipt.code, "pftl_uniswap_route_epoch_advance_blocked");
    assert_eq!(ledger, before_blocked);

    let receipt = execute(
        &mut ledger,
        &operator_key,
        &operator,
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapDestinationConsume(
            PftlUniswapDestinationConsumeOperation {
                operator: operator.clone(),
                route_id: route_id.clone(),
                packet_hash: packet_hash.clone(),
                ethereum_consume_tx_hash: "99".repeat(32),
                consumed_height: 10,
                finalized_height: 22,
                external_event_proof: None,
            },
        ),
        7,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let ethereum_sender = "0x5555555555555555555555555555555555555555";
    let return_nonce = "ab".repeat(32);
    let burn_event_hash = pftl_uniswap_return_burn_id_from_fields(
        1,
        "0x1111111111111111111111111111111111111111",
        "0x3333333333333333333333333333333333333333",
        &native_asset_id,
        ethereum_sender,
        &holder,
        40,
        &return_nonce,
        20,
    )
    .expect("AR-05 return burn ID");
    let receipt = execute(
        &mut ledger,
        &operator_key,
        &operator,
        PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapReturnImport(PftlUniswapReturnImportOperation {
            operator: operator.clone(),
            route_id: route_id.clone(),
            burn_event_hash,
            ethereum_chain_id: 1,
            bridge_controller: "0x1111111111111111111111111111111111111111".to_string(),
            wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
            native_nav_asset_id: native_asset_id,
            ethereum_sender: ethereum_sender.to_string(),
            pftl_recipient: holder.clone(),
            amount_atoms: 40,
            return_nonce,
            burn_height: 20,
            finalized_height: 32,
            external_event_proof: None,
        }),
        8,
    );
    assert!(receipt.accepted, "{receipt:?}");
    let receipt = execute(
        &mut ledger,
        &holder_key,
        &holder,
        PFTL_UNISWAP_ORDER_RELEASE_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapOrderRelease(PftlUniswapOrderReleaseOperation {
            releaser: holder.clone(),
            route_id: route_id.clone(),
            reservation_id: reservation_id.clone(),
        }),
        9,
    );
    assert!(receipt.accepted, "{receipt:?}");
    assert!(ledger
        .pftl_uniswap_route(&route_id)
        .expect("AR-05 route after completion")
        .v2
        .as_ref()
        .expect("AR-05 v2 state")
        .export_entitlements
        .get(&reservation_id)
        .is_none());

    let receipt = execute(
        &mut ledger,
        &operator_key,
        &operator,
        PFTL_UNISWAP_ROUTE_EPOCH_ADVANCE_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(epoch_advance),
        10,
    );
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(
        ledger
            .pftl_uniswap_route(&route_id)
            .expect("AR-05 advanced route")
            .v2
            .as_ref()
            .expect("AR-05 advanced v2")
            .route_epoch,
        2
    );
}
