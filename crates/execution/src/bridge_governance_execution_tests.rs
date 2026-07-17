    fn usd_e8(amount: u128) -> u128 {
        amount * 100_000_000
    }

    fn market_ops_policy_fixture() -> MarketOpsPolicyRegistration {
        MarketOpsPolicyRegistration {
            program_id: [0x31; 32],
            policy_hash: [0x32; 32],
            parameter_hash: [0x33; 32],
            venue_id: [0x37; 32],
            pool_config_hash: [0x38; 32],
            hook_code_hash: [0x39; 32],
            activation_epoch: 1,
            deactivation_epoch: 0,
        }
    }

    fn market_ops_operation_fixture(
        issuer: &str,
        asset_id: &str,
        reserve_packet_hash: &str,
    ) -> MarketOpsFinalizeOperation {
        let policy = market_ops_policy_fixture();
        let discount_observations = vec![
            MarketOpsVenueObservation {
                dt_seconds: 4_200,
                price_usd_e8: usd_e8(475) / 100,
                volume_usd_e8: usd_e8(2_500),
            },
            MarketOpsVenueObservation {
                dt_seconds: 5_800,
                price_usd_e8: usd_e8(5),
                volume_usd_e8: usd_e8(7_500),
            },
        ];
        let premium_observations = vec![
            MarketOpsVenueObservation {
                dt_seconds: 1_800,
                price_usd_e8: usd_e8(5625) / 1_000,
                volume_usd_e8: usd_e8(2_200),
            },
            MarketOpsVenueObservation {
                dt_seconds: 8_200,
                price_usd_e8: usd_e8(5),
                volume_usd_e8: usd_e8(7_800),
            },
        ];
        let policy_inputs = MarketOpsPolicyInputs {
            unit_scale: 1,
            floor_factor_bps: 10_000,
            alignment_params: MarketOpsAlignmentParams {
                policy_min_usd_e8: usd_e8(25_000),
                min_alignment_bps: 100,
                stress_repeat_factor_14d: 3,
                stress_repeat_factor_90d: 2,
                stale_epochs_allowed: 1,
                max_decay_per_epoch_bps: 1_000,
            },
            previous_required_alignment_reserve_usd_e8: 0,
            cost_to_restore_14d_usd_e8: vec![usd_e8(20_000), usd_e8(45_000), usd_e8(45_000)],
            cost_to_restore_90d_usd_e8: vec![usd_e8(30_000), usd_e8(45_000), usd_e8(60_000)],
            reserve_limits: MarketOpsReserveDeployLimits {
                available_alignment_reserve_usd_e8: usd_e8(150_000),
                venue_policy_cap_usd_e8: usd_e8(50_000),
                depth_limited_cap_usd_e8: usd_e8(30_000),
                cooldown_limited_cap_usd_e8: usd_e8(40_000),
            },
            mint_limits: MarketOpsMintLimits {
                policy_max_mint_atoms: 50_000,
                venue_bid_depth_atoms: 12_000,
                cooldown_mint_atoms: 10_000,
            },
            discount_observations,
            premium_observations,
        };
        let envelope = MarketOpsEnvelope {
            encoding_version: 1,
            chain_id: 1,
            adapter_address: [0x11; 20],
            vault_address: [0x12; 20],
            mint_controller_address: [0x13; 20],
            asset_id: market_ops_asset_id(asset_id).expect("market ops asset id"),
            epoch: 1,
            program_id: policy.program_id,
            policy_hash: policy.policy_hash,
            parameter_hash: policy.parameter_hash,
            reserve_packet_hash: market_ops_reserve_packet_hash(reserve_packet_hash)
                .expect("reserve packet hash"),
            supply_packet_hash: market_ops_supply_packet_hash(asset_id, 1, 1_000_000)
                .expect("supply packet hash"),
            evidence_root: market_ops_evidence_root(
                &policy_inputs.discount_observations,
                &policy_inputs.premium_observations,
            )
            .expect("evidence root"),
            previous_market_state_hash: [0u8; 32],
            venue_id: policy.venue_id,
            pool_config_hash: policy.pool_config_hash,
            hook_code_hash: policy.hook_code_hash,
            nav_floor_usd_e8: usd_e8(5),
            valid_global_supply_atoms: 1_000_000,
            verified_net_assets_usd_e8: usd_e8(5_000_000),
            funded_alignment_reserve_usd_e8: usd_e8(150_000),
            required_alignment_reserve_usd_e8: usd_e8(135_000),
            max_reserve_deploy_usd_e8: usd_e8(25_875),
            max_mint_atoms: 0,
            discount_trigger_bps: 300,
            premium_trigger_bps: 1_000,
            data_window_start: 100,
            data_window_end: 10_100,
            valid_after: 10_100,
            expires_at: 20_100,
            cooldown_seconds: 600,
            nonce: [0x55; 32],
        };
        let envelope_hash = bytes_to_hex(&envelope.envelope_hash());
        MarketOpsFinalizeOperation {
            issuer: issuer.to_string(),
            asset_id: asset_id.to_string(),
            envelope_hash,
            envelope,
            policy_inputs,
        }
    }

    fn finalized_nav_market_ops_fixture() -> (
        Genesis,
        LedgerState,
        MlDsa65KeyPair,
        String,
        MarketOpsFinalizeOperation,
    ) {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let mut ledger = LedgerState::new(vec![Account::new(
            issuer.clone(),
            10_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        )]);

        let create = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "NAVMOPS".to_string(),
                version: 1,
                precision: 6,
                display_name: "NAV Market Ops".to_string(),
                max_supply: Some(2_000_000),
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
                valuation_unit: "usd_e8".to_string(),
                redemption_account: issuer.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 2).accepted);

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
                nav_per_unit: usd_e8(5) as u64,
                circulating_supply: 1_000_000,
                verified_net_assets: usd_e8(5_000_000) as u64,
                proof_profile: "nitro-reserve-v0".to_string(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: reserve_packet_hash.clone(),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &submit, 3).accepted);

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
        assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize, 4).accepted);
        ledger.market_ops_policies.push(market_ops_policy_fixture());
        let operation = market_ops_operation_fixture(&issuer, &asset_id, &reserve_packet_hash);
        (genesis, ledger, issuer_key, asset_id, operation)
    }

    #[test]
    fn market_ops_finalize_records_policy_replayed_envelope() {
        let (genesis, mut ledger, issuer_key, asset_id, operation) =
            finalized_nav_market_ops_fixture();
        let tx = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            MARKET_OPS_FINALIZE_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::MarketOpsFinalize(operation.clone()),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &tx, 5);
        assert!(receipt.accepted, "{}", receipt.message);
        let record = ledger
            .market_ops_envelope(&asset_id, 1)
            .expect("finalized market ops envelope");
        assert_eq!(operation.envelope_hash, record.envelope_hash);
        assert_eq!(operation.envelope, record.envelope);
        assert_eq!(5, record.finalized_at_height);
    }

    #[test]
    fn market_ops_policy_register_allows_later_finalize() {
        let (genesis, mut ledger, issuer_key, asset_id, operation) =
            finalized_nav_market_ops_fixture();
        ledger.market_ops_policies.clear();
        let policy_register = MarketOpsPolicyRegisterOperation {
            issuer: operation.issuer.clone(),
            asset_id: asset_id.clone(),
            policy: market_ops_policy_fixture(),
        };
        let register_tx = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::MarketOpsPolicyRegister(policy_register.clone()),
        );
        let register_receipt = execute_asset_transaction(&genesis, &mut ledger, &register_tx, 5);
        assert!(
            register_receipt.accepted,
            "{}",
            register_receipt.message
        );
        assert_eq!(vec![policy_register.policy.clone()], ledger.market_ops_policies);

        let duplicate_tx = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::MarketOpsPolicyRegister(policy_register),
        );
        let duplicate_receipt = execute_asset_transaction(&genesis, &mut ledger, &duplicate_tx, 6);
        assert!(!duplicate_receipt.accepted);
        assert_eq!("duplicate_market_ops_policy", duplicate_receipt.code);

        let finalize_tx = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            MARKET_OPS_FINALIZE_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::MarketOpsFinalize(operation.clone()),
        );
        let finalize_receipt = execute_asset_transaction(&genesis, &mut ledger, &finalize_tx, 7);
        assert!(
            finalize_receipt.accepted,
            "{}",
            finalize_receipt.message
        );
        assert_eq!(
            operation.envelope_hash,
            ledger
                .market_ops_envelope(&asset_id, 1)
                .expect("market ops envelope")
                .envelope_hash
        );
    }

    #[test]
    fn market_ops_finalize_rejects_unregistered_policy_and_bad_evidence() {
        let (genesis, ledger, issuer_key, _asset_id, operation) =
            finalized_nav_market_ops_fixture();

        let mut unregistered_ledger = ledger.clone();
        unregistered_ledger.market_ops_policies.clear();
        let unregistered_tx = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &unregistered_ledger,
            &issuer_key,
            MARKET_OPS_FINALIZE_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::MarketOpsFinalize(operation.clone()),
        );
        let receipt = execute_asset_transaction(
            &genesis,
            &mut unregistered_ledger,
            &unregistered_tx,
            5,
        );
        assert!(!receipt.accepted);
        assert_eq!("unregistered_market_ops_policy", receipt.code);

        let mut bad_evidence_operation = operation;
        bad_evidence_operation.policy_inputs.discount_observations[0].volume_usd_e8 += 1;
        let mut bad_evidence_ledger = ledger;
        let bad_evidence_tx = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &bad_evidence_ledger,
            &issuer_key,
            MARKET_OPS_FINALIZE_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::MarketOpsFinalize(bad_evidence_operation),
        );
        let receipt = execute_asset_transaction(
            &genesis,
            &mut bad_evidence_ledger,
            &bad_evidence_tx,
            5,
        );
        assert!(!receipt.accepted);
        assert_eq!("invalid_market_ops_evidence_root", receipt.code);
    }

    #[test]
    fn nav_profile_ledger_transparent_gates_enforce_window_staleness_and_deadman() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let ap_key = ml_dsa_65_keygen().expect("ap keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let ap = address_from_public_key(&ap_key.public_key);
        let reserve_wallet = format!("pf{}", "7".repeat(40));
        let mut ledger = LedgerState::new(vec![
            Account::new(issuer.clone(), 2_000, Some(bytes_to_hex(&issuer_key.public_key))),
            Account::new(ap.clone(), 1_000, Some(bytes_to_hex(&ap_key.public_key))),
            Account::new(reserve_wallet.clone(), 300, None),
        ]);

        let register_profile = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_PROFILE_REGISTER_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
                registrant: issuer.clone(),
                verifier_kind: NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT.to_string(),
                source_class: "ledger".to_string(),
                min_attestations: 0,
                tolerance_bp: 0,
                bridge_observer_min_confirmations: 0,
                valuation_policy_hash: String::new(),
                vault_bridge_route_policy_hash: String::new(),
                max_snapshot_age_blocks: 10,
                challenge_window_blocks: 2,
                max_epoch_gap_blocks: 20,
                settle_deadline_blocks: 5,
                min_challenge_bond: 50,
                sp1_program_vkey: String::new(),
                sp1_proof_encoding: String::new(),
                max_proof_bytes: 0,
                max_public_values_bytes: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_profile, 1).accepted);
        assert_eq!(ledger.nav_proof_profiles.len(), 1);
        let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();
        // Cross-language consensus vector: python/postfiat_rpc/navcoin.py
        // nav_proof_profile_id("ledger-transparent", "ledger", 10, 2, 20, 5, 50, 0, 0, "", "", "", 0, 0)
        assert_eq!(
            profile_id,
            "2911d88adff1737c7fa758370e4bf564eb118da2e4a594b9bdf0805eb3cebcc1e24ddd36043b77c6481937efa88e1d64"
        );

        let duplicate_profile = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_PROFILE_REGISTER_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
                registrant: issuer.clone(),
                verifier_kind: NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT.to_string(),
                source_class: "ledger".to_string(),
                min_attestations: 0,
                tolerance_bp: 0,
                bridge_observer_min_confirmations: 0,
                valuation_policy_hash: String::new(),
                vault_bridge_route_policy_hash: String::new(),
                max_snapshot_age_blocks: 10,
                challenge_window_blocks: 2,
                max_epoch_gap_blocks: 20,
                settle_deadline_blocks: 5,
                min_challenge_bond: 50,
                sp1_program_vkey: String::new(),
                sp1_proof_encoding: String::new(),
                max_proof_bytes: 0,
                max_public_values_bytes: 0,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &duplicate_profile, 1);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "duplicate_nav_profile");

        let create = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "GLD".to_string(),
                version: 1,
                precision: 6,
                display_name: "GOLDNAV".to_string(),
                max_supply: Some(10_000),
                requires_authorization: false,
                freeze_enabled: false,
                clawback_enabled: false,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &create, 1).accepted);
        let asset_id = ledger.asset_definitions[0].asset_id.clone();

        let bogus_profile_register = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_ASSET_REGISTER_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                reserve_operator: issuer.clone(),
                proof_profile: "9".repeat(96),
                valuation_unit: "pft".to_string(),
                redemption_account: issuer.clone(),
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &bogus_profile_register, 1);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "unknown_nav_profile");

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
                valuation_unit: "pft".to_string(),
                redemption_account: issuer.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 1).accepted);

        let ap_trust = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &ap_key,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: ap.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                limit: 10_000,
                authorized: false,
                frozen: false,
                reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &ap_trust, 1).accepted);

        let reserve_packet_hash = "ab".repeat(48);
        let wrong_sum_submit = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit: 4,
                circulating_supply: 100,
                verified_net_assets: 400,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: reserve_packet_hash.clone(),
                reserve_accounts: vec![reserve_wallet.clone()],
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &wrong_sum_submit, 5);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "reserve_sum_mismatch");

        let no_accounts_submit = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit: 3,
                circulating_supply: 100,
                verified_net_assets: 300,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: reserve_packet_hash.clone(),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &no_accounts_submit, 5);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "missing_reserve_accounts");

        let submit = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit: 3,
                circulating_supply: 100,
                verified_net_assets: 300,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: reserve_packet_hash.clone(),
                reserve_accounts: vec![reserve_wallet.clone()],
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &submit, 5).accepted);
        assert_eq!(ledger.nav_reserve_packets[0].submitted_at_height, 5);

        let challenge = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &ap_key,
            NAV_RESERVE_CHALLENGE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::NavReserveChallenge(NavReserveChallengeOperation {
                challenger: ap.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
                challenge_hash: "0c".repeat(48),
                bond: 100,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &challenge, 6);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "nav_packet_consensus_verified");

        let finalize_early = signed_asset_transaction_with_minimum_fee(
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
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &finalize_early, 6);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "nav_challenge_window_open");

        let finalize = signed_asset_transaction_with_minimum_fee(
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
        assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize, 7).accepted);
        assert_eq!(ledger.nav_asset(&asset_id).expect("nav asset").finalized_at_height, 7);

        let stale_hash = "cd".repeat(48);
        let stale_submit = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
                issuer: issuer.clone(),
                submitter: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 2,
                nav_per_unit: 3,
                circulating_supply: 100,
                verified_net_assets: 300,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: stale_hash.clone(),
                reserve_accounts: vec![reserve_wallet.clone()],
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &stale_submit, 8).accepted);
        let finalize_stale = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
            7,
            AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 2,
                reserve_packet_hash: stale_hash.clone(),
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &finalize_stale, 19);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "stale_nav_reserve_packet");

        let mint = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_MINT_AT_NAV_TRANSACTION_KIND,
            7,
            AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
                issuer: issuer.clone(),
                to: ap.clone(),
                asset_id: asset_id.clone(),
                amount: 100,
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
                settlement_asset_id: String::new(),
                settlement_bucket_id: String::new(),
                settlement_allocation_id: String::new(),
                settlement_amount_atoms: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &mint, 20).accepted);

        let deadman_mint = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_MINT_AT_NAV_TRANSACTION_KIND,
            8,
            AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
                issuer: issuer.clone(),
                to: ap.clone(),
                asset_id: asset_id.clone(),
                amount: 1,
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
                settlement_asset_id: String::new(),
                settlement_bucket_id: String::new(),
                settlement_allocation_id: String::new(),
                settlement_amount_atoms: 0,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &deadman_mint, 28);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "nav_reserve_stale_deadman");
    }

    #[test]
    fn nav_redeem_settlement_deadline_blocks_mint_until_settled() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let ap_key = ml_dsa_65_keygen().expect("ap keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let ap = address_from_public_key(&ap_key.public_key);
        let reserve_wallet = format!("pf{}", "8".repeat(40));
        let mut ledger = LedgerState::new(vec![
            Account::new(issuer.clone(), 2_000, Some(bytes_to_hex(&issuer_key.public_key))),
            Account::new(ap.clone(), 1_000, Some(bytes_to_hex(&ap_key.public_key))),
            Account::new(reserve_wallet.clone(), 300, None),
        ]);

        let register_profile = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_PROFILE_REGISTER_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
                registrant: issuer.clone(),
                verifier_kind: NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT.to_string(),
                source_class: "ledger".to_string(),
                min_attestations: 0,
                tolerance_bp: 0,
                bridge_observer_min_confirmations: 0,
                valuation_policy_hash: String::new(),
                vault_bridge_route_policy_hash: String::new(),
                max_snapshot_age_blocks: 0,
                challenge_window_blocks: 0,
                max_epoch_gap_blocks: 0,
                settle_deadline_blocks: 5,
                min_challenge_bond: 0,
                sp1_program_vkey: String::new(),
                sp1_proof_encoding: String::new(),
                max_proof_bytes: 0,
                max_public_values_bytes: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_profile, 1).accepted);
        let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

        let create = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "SLV".to_string(),
                version: 1,
                precision: 6,
                display_name: "SILVERNAV".to_string(),
                max_supply: Some(10_000),
                requires_authorization: false,
                freeze_enabled: false,
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
            3,
            AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                reserve_operator: issuer.clone(),
                proof_profile: profile_id.clone(),
                valuation_unit: "pft".to_string(),
                redemption_account: issuer.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 1).accepted);

        let ap_trust = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &ap_key,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: ap.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                limit: 10_000,
                authorized: false,
                frozen: false,
                reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &ap_trust, 1).accepted);

        let reserve_packet_hash = "ef".repeat(48);
        let submit = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit: 3,
                circulating_supply: 100,
                verified_net_assets: 300,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: reserve_packet_hash.clone(),
                reserve_accounts: vec![reserve_wallet.clone()],
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &submit, 2).accepted);

        let finalize = signed_asset_transaction_with_minimum_fee(
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
        assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize, 3).accepted);

        let mint = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_MINT_AT_NAV_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
                issuer: issuer.clone(),
                to: ap.clone(),
                asset_id: asset_id.clone(),
                amount: 100,
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
                settlement_asset_id: String::new(),
                settlement_bucket_id: String::new(),
                settlement_allocation_id: String::new(),
                settlement_amount_atoms: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &mint, 4).accepted);

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
                amount: 10,
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &redeem, 5).accepted);
        assert_eq!(ledger.nav_redemptions[0].created_at_height, 5);
        let redemption_id = ledger.nav_redemptions[0].redemption_id.clone();

        let overdue_mint = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_MINT_AT_NAV_TRANSACTION_KIND,
            7,
            AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
                issuer: issuer.clone(),
                to: ap.clone(),
                asset_id: asset_id.clone(),
                amount: 1,
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
                settlement_asset_id: String::new(),
                settlement_bucket_id: String::new(),
                settlement_allocation_id: String::new(),
                settlement_amount_atoms: 0,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &overdue_mint, 11);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "nav_redemptions_overdue");

        let settle = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_REDEEM_SETTLE_TRANSACTION_KIND,
            7,
            AssetTransactionOperation::NavRedeemSettle(NavRedeemSettleOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                redemption_id: redemption_id.clone(),
                settlement_receipt_hash: "0d".repeat(48),
                settlement_asset_id: String::new(),
                settlement_bucket_id: String::new(),
                settlement_allocation_id: String::new(),
                settlement_amount_atoms: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &settle, 11).accepted);
        assert_eq!(ledger.nav_redemptions[0].state, NAV_REDEMPTION_STATE_SETTLED);

        let mint_after_settle = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_MINT_AT_NAV_TRANSACTION_KIND,
            8,
            AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
                issuer: issuer.clone(),
                to: ap.clone(),
                asset_id: asset_id.clone(),
                amount: 1,
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
                settlement_asset_id: String::new(),
                settlement_bucket_id: String::new(),
                settlement_allocation_id: String::new(),
                settlement_amount_atoms: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &mint_after_settle, 12).accepted);
    }

    #[test]
    fn nav_bonded_challenge_resolves_refund_and_forfeit_paths() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let challenger_key = ml_dsa_65_keygen().expect("challenger keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let challenger = address_from_public_key(&challenger_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(issuer.clone(), 2_000, Some(bytes_to_hex(&issuer_key.public_key))),
            Account::new(
                challenger.clone(),
                1_000,
                Some(bytes_to_hex(&challenger_key.public_key)),
            ),
        ]);

        let register_profile = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_PROFILE_REGISTER_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
                registrant: issuer.clone(),
                verifier_kind: NAV_PROFILE_VERIFIER_PLACEHOLDER.to_string(),
                source_class: "ledger".to_string(),
                min_attestations: 0,
                tolerance_bp: 0,
                bridge_observer_min_confirmations: 0,
                valuation_policy_hash: String::new(),
                vault_bridge_route_policy_hash: String::new(),
                max_snapshot_age_blocks: 0,
                challenge_window_blocks: 0,
                max_epoch_gap_blocks: 0,
                settle_deadline_blocks: 0,
                min_challenge_bond: 50,
                sp1_program_vkey: String::new(),
                sp1_proof_encoding: String::new(),
                max_proof_bytes: 0,
                max_public_values_bytes: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_profile, 1).accepted);
        let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

        let create = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "PLD".to_string(),
                version: 1,
                precision: 6,
                display_name: "PALLADIUMNAV".to_string(),
                max_supply: Some(10_000),
                requires_authorization: false,
                freeze_enabled: false,
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
            3,
            AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                reserve_operator: issuer.clone(),
                proof_profile: profile_id.clone(),
                valuation_unit: "usd_1e6".to_string(),
                redemption_account: issuer.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 1).accepted);

        let bad_hash = "aa".repeat(48);
        let submit_bad = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit: 7,
                circulating_supply: 10,
                verified_net_assets: 70,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: bad_hash.clone(),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &submit_bad, 2).accepted);

        let low_bond_challenge = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &challenger_key,
            NAV_RESERVE_CHALLENGE_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavReserveChallenge(NavReserveChallengeOperation {
                challenger: challenger.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: bad_hash.clone(),
                challenge_hash: "0c".repeat(48),
                bond: 10,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &low_bond_challenge, 3);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "challenge_bond_too_low");

        let challenger_balance_before =
            ledger.account(&challenger).expect("challenger").balance;
        let challenge = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &challenger_key,
            NAV_RESERVE_CHALLENGE_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavReserveChallenge(NavReserveChallengeOperation {
                challenger: challenger.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: bad_hash.clone(),
                challenge_hash: "0c".repeat(48),
                bond: 60,
            }),
        );
        let challenge_fee = challenge.unsigned.fee;
        assert!(execute_asset_transaction(&genesis, &mut ledger, &challenge, 3).accepted);
        assert_eq!(
            ledger.account(&challenger).expect("challenger").balance,
            challenger_balance_before - 60 - challenge_fee
        );
        assert!(ledger.nav_asset(&asset_id).expect("nav asset").halted);

        let good_hash = "bb".repeat(48);
        let submit_good = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
                issuer: issuer.clone(),
                submitter: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                nav_per_unit: 6,
                circulating_supply: 10,
                verified_net_assets: 60,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: good_hash.clone(),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &submit_good, 4).accepted);

        let balance_before_refund =
            ledger.account(&challenger).expect("challenger").balance;
        let finalize_good = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: good_hash.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize_good, 5).accepted);
        assert_eq!(
            ledger.account(&challenger).expect("challenger").balance,
            balance_before_refund + 60,
            "same-epoch replacement refunds the challenger bond"
        );

        let epoch2_hash = "cc".repeat(48);
        let submit_epoch2 = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
            7,
            AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
                issuer: issuer.clone(),
                submitter: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 2,
                nav_per_unit: 6,
                circulating_supply: 10,
                verified_net_assets: 60,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: epoch2_hash.clone(),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &submit_epoch2, 6).accepted);

        let challenge_epoch2 = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &challenger_key,
            NAV_RESERVE_CHALLENGE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::NavReserveChallenge(NavReserveChallengeOperation {
                challenger: challenger.clone(),
                asset_id: asset_id.clone(),
                epoch: 2,
                reserve_packet_hash: epoch2_hash.clone(),
                challenge_hash: "0e".repeat(48),
                bond: 60,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &challenge_epoch2, 7).accepted);

        let epoch3_hash = "dd".repeat(48);
        let submit_epoch3 = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
            8,
            AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
                issuer: issuer.clone(),
                submitter: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 3,
                nav_per_unit: 6,
                circulating_supply: 10,
                verified_net_assets: 60,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: epoch3_hash.clone(),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &submit_epoch3, 8).accepted);

        let issuer_balance_before =
            ledger.account(&issuer).expect("issuer").balance;
        let finalize_epoch3 = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
            9,
            AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 3,
                reserve_packet_hash: epoch3_hash.clone(),
            }),
        );
        let finalize_fee = finalize_epoch3.unsigned.fee;
        assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize_epoch3, 9).accepted);
        assert_eq!(
            ledger.account(&issuer).expect("issuer").balance,
            issuer_balance_before + 60 - finalize_fee,
            "abandoned-epoch challenge forfeits the bond to the issuer"
        );
    }

    #[test]
    fn nav_multi_fetch_quorum_gates_finalize_on_attestations() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let obs1_key = ml_dsa_65_keygen().expect("obs1 keygen");
        let obs2_key = ml_dsa_65_keygen().expect("obs2 keygen");
        let obs3_key = ml_dsa_65_keygen().expect("obs3 keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let obs1 = address_from_public_key(&obs1_key.public_key);
        let obs2 = address_from_public_key(&obs2_key.public_key);
        let obs3 = address_from_public_key(&obs3_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(issuer.clone(), 2_000, Some(bytes_to_hex(&issuer_key.public_key))),
            Account::new(obs1.clone(), 500, Some(bytes_to_hex(&obs1_key.public_key))),
            Account::new(obs2.clone(), 500, Some(bytes_to_hex(&obs2_key.public_key))),
            Account::new(obs3.clone(), 500, Some(bytes_to_hex(&obs3_key.public_key))),
        ]);

        let register_profile = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_PROFILE_REGISTER_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
                registrant: issuer.clone(),
                verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
                source_class: "hyperliquid-testnet".to_string(),
                min_attestations: 2,
                tolerance_bp: 10,
                bridge_observer_min_confirmations: 0,
                valuation_policy_hash: String::new(),
                vault_bridge_route_policy_hash: String::new(),
                max_snapshot_age_blocks: 0,
                challenge_window_blocks: 0,
                max_epoch_gap_blocks: 0,
                settle_deadline_blocks: 0,
                min_challenge_bond: 0,
                sp1_program_vkey: String::new(),
                sp1_proof_encoding: String::new(),
                max_proof_bytes: 0,
                max_public_values_bytes: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_profile, 1).accepted);
        let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

        let create = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "HLG".to_string(),
                version: 1,
                precision: 6,
                display_name: "HLGOLD".to_string(),
                max_supply: Some(10_000),
                requires_authorization: false,
                freeze_enabled: false,
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
            3,
            AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                reserve_operator: issuer.clone(),
                proof_profile: profile_id.clone(),
                valuation_unit: "usd_1e6".to_string(),
                redemption_account: issuer.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register, 1).accepted);

        for (key, addr, domain) in [
            (&obs1_key, &obs1, "obs1.example"),
            (&obs2_key, &obs2, "obs2.example"),
            (&obs3_key, &obs3, "obs3.example"),
        ] {
            let register_attestor = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                key,
                NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
                1,
                AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
                    attestor: addr.clone(),
                    domain: domain.to_string(),
                    bond: 25,
                }),
            );
            assert!(
                execute_asset_transaction(&genesis, &mut ledger, &register_attestor, 1).accepted
            );
        }
        assert_eq!(ledger.nav_attestors.len(), 3);

        let packet_hash = "fe".repeat(48);
        let submit_missing_accounts = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit: 5,
                circulating_supply: 100,
                verified_net_assets: 500,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: packet_hash.clone(),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        let receipt =
            execute_asset_transaction(&genesis, &mut ledger, &submit_missing_accounts, 2);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "missing_reserve_accounts");

        let submit = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit: 5,
                circulating_supply: 100,
                verified_net_assets: 500,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: packet_hash.clone(),
                reserve_accounts: vec!["hyperliquid:0xabc0000000000000000000000000000000000001".to_string()],
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &submit, 2).accepted);

        let finalize_no_quorum = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: packet_hash.clone(),
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &finalize_no_quorum, 3);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "nav_attestation_quorum_not_met");

        let attest1 = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &obs1_key,
            NAV_RESERVE_ATTEST_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
                attestor: obs1.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: packet_hash.clone(),
                pass: true,
                observation_root: "0a".repeat(48),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &attest1, 3).accepted);

        let attest1_dup = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &obs1_key,
            NAV_RESERVE_ATTEST_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
                attestor: obs1.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: packet_hash.clone(),
                pass: true,
                observation_root: "0a".repeat(48),
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &attest1_dup, 3);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "duplicate_nav_attestation");

        let attest_fail = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &obs2_key,
            NAV_RESERVE_ATTEST_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
                attestor: obs2.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: packet_hash.clone(),
                pass: false,
                observation_root: "0b".repeat(48),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &attest_fail, 4).accepted);

        let finalize_with_fail = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: packet_hash.clone(),
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &finalize_with_fail, 4);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "nav_failed_attestations_present");

        let packet_hash2 = "fd".repeat(48);
        let submit2 = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
                issuer: issuer.clone(),
                submitter: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                nav_per_unit: 5,
                circulating_supply: 100,
                verified_net_assets: 500,
                proof_profile: profile_id.clone(),
                source_root: "03".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: packet_hash2.clone(),
                reserve_accounts: vec!["hyperliquid:0xabc0000000000000000000000000000000000001".to_string()],
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &submit2, 5).accepted);

        for (key, addr, seq) in [(&obs1_key, &obs1, 3u64), (&obs2_key, &obs2, 3u64), (&obs3_key, &obs3, 2u64)] {
            let attest = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                key,
                NAV_RESERVE_ATTEST_TRANSACTION_KIND,
                seq,
                AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
                    attestor: addr.clone(),
                    asset_id: asset_id.clone(),
                    epoch: 1,
                    reserve_packet_hash: packet_hash2.clone(),
                    pass: true,
                    observation_root: "0c".repeat(48),
                }),
            );
            assert!(execute_asset_transaction(&genesis, &mut ledger, &attest, 6).accepted);
        }

        let finalize = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: packet_hash2.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &finalize, 7).accepted);
        let nav_asset = ledger.nav_asset(&asset_id).expect("nav asset");
        assert_eq!(nav_asset.finalized_epoch, 1);
        assert_eq!(nav_asset.finalized_reserve_packet_hash, packet_hash2);
    }

    #[test]
    fn escrow_transactions_lock_finish_cancel_and_reject_replay() {
        let genesis = Genesis::new("postfiat-local");
        let owner_key = ml_dsa_65_keygen().expect("owner keygen");
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let owner = address_from_public_key(&owner_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                owner.clone(),
                300,
                Some(bytes_to_hex(&owner_key.public_key)),
            ),
            Account::new(
                recipient.clone(),
                100,
                Some(bytes_to_hex(&recipient_key.public_key)),
            ),
        ]);

        let first_escrow_id = escrow_id(&genesis.chain_id, &owner, 1).expect("first escrow id");
        let create = signed_escrow_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            ESCROW_CREATE_TRANSACTION_KIND,
            1,
            EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
                owner: owner.clone(),
                recipient: recipient.clone(),
                asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
                amount: 50,
                condition: "secret".to_string(),
                finish_after: 2,
                cancel_after: 5,
            }),
        );
        let create_fee = create.unsigned.fee;
        let receipt = execute_escrow_transaction(&genesis, &mut ledger, &create, 1);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.tx_id, escrow_transaction_tx_id(&create));
        assert_eq!(receipt.state_expansion_fee, ESCROW_STATE_EXPANSION_FEE);
        assert_eq!(
            ledger.account(&owner).expect("owner").balance,
            300 - create_fee - 50
        );
        assert_eq!(ledger.account(&owner).expect("owner").sequence, 1);
        let first_escrow = ledger.escrow(&first_escrow_id).expect("first escrow");
        assert_eq!(first_escrow.state, ESCROW_STATE_OPEN);
        assert_eq!(first_escrow.created_height, 1);
        assert_eq!(first_escrow.amount, 50);

        let before_replay = ledger.clone();
        let replay = execute_escrow_transaction(&genesis, &mut ledger, &create, 1);
        assert!(!replay.accepted);
        assert_eq!(replay.code, "bad_sequence");
        assert_eq!(ledger, before_replay);

        let early_finish = signed_escrow_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &recipient_key,
            ESCROW_FINISH_TRANSACTION_KIND,
            1,
            EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
                escrow_id: first_escrow_id.clone(),
                owner: owner.clone(),
                recipient: recipient.clone(),
                fulfillment: "secret".to_string(),
            }),
        );
        let before_early_finish = ledger.clone();
        let receipt = execute_escrow_transaction(&genesis, &mut ledger, &early_finish, 1);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "escrow_finish_too_early");
        assert_eq!(ledger, before_early_finish);

        let wrong_fulfillment = signed_escrow_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &recipient_key,
            ESCROW_FINISH_TRANSACTION_KIND,
            1,
            EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
                escrow_id: first_escrow_id.clone(),
                owner: owner.clone(),
                recipient: recipient.clone(),
                fulfillment: "wrong".to_string(),
            }),
        );
        let before_wrong_fulfillment = ledger.clone();
        let receipt = execute_escrow_transaction(&genesis, &mut ledger, &wrong_fulfillment, 2);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "escrow_condition_unsatisfied");
        assert_eq!(ledger, before_wrong_fulfillment);

        let finish = signed_escrow_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &recipient_key,
            ESCROW_FINISH_TRANSACTION_KIND,
            1,
            EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
                escrow_id: first_escrow_id.clone(),
                owner: owner.clone(),
                recipient: recipient.clone(),
                fulfillment: "secret".to_string(),
            }),
        );
        let finish_fee = finish.unsigned.fee;
        let receipt = execute_escrow_transaction(&genesis, &mut ledger, &finish, 2);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.tx_id, escrow_transaction_tx_id(&finish));
        assert_eq!(
            ledger.account(&recipient).expect("recipient").balance,
            100 - finish_fee + 50
        );
        assert_eq!(ledger.account(&recipient).expect("recipient").sequence, 1);
        assert_eq!(
            ledger.escrow(&first_escrow_id).expect("first escrow").state,
            ESCROW_STATE_FINISHED
        );

        let cancel_finished = signed_escrow_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            ESCROW_CANCEL_TRANSACTION_KIND,
            2,
            EscrowTransactionOperation::EscrowCancel(EscrowCancelOperation {
                escrow_id: first_escrow_id.clone(),
                owner: owner.clone(),
            }),
        );
        let before_cancel_finished = ledger.clone();
        let receipt = execute_escrow_transaction(&genesis, &mut ledger, &cancel_finished, 5);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "escrow_not_open");
        assert_eq!(ledger, before_cancel_finished);

        let second_escrow_id = escrow_id(&genesis.chain_id, &owner, 2).expect("second escrow id");
        let create_cancelable = signed_escrow_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            ESCROW_CREATE_TRANSACTION_KIND,
            2,
            EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
                owner: owner.clone(),
                recipient: recipient.clone(),
                asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
                amount: 40,
                condition: String::new(),
                finish_after: 0,
                cancel_after: 3,
            }),
        );
        let create_cancelable_fee = create_cancelable.unsigned.fee;
        let receipt = execute_escrow_transaction(&genesis, &mut ledger, &create_cancelable, 2);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.state_expansion_fee, ESCROW_STATE_EXPANSION_FEE);
        assert_eq!(
            ledger
                .escrow(&second_escrow_id)
                .expect("second escrow")
                .state,
            ESCROW_STATE_OPEN
        );

        let cancel = signed_escrow_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &owner_key,
            ESCROW_CANCEL_TRANSACTION_KIND,
            3,
            EscrowTransactionOperation::EscrowCancel(EscrowCancelOperation {
                escrow_id: second_escrow_id.clone(),
                owner: owner.clone(),
            }),
        );
        let before_early_cancel = ledger.clone();
        let receipt = execute_escrow_transaction(&genesis, &mut ledger, &cancel, 2);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "escrow_cancel_too_early");
        assert_eq!(ledger, before_early_cancel);

        let cancel_fee = cancel.unsigned.fee;
        let receipt = execute_escrow_transaction(&genesis, &mut ledger, &cancel, 3);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.tx_id, escrow_transaction_tx_id(&cancel));
        assert_eq!(
            ledger.account(&owner).expect("owner").balance,
            300 - create_fee - 50 - create_cancelable_fee - cancel_fee
        );
        assert_eq!(ledger.account(&owner).expect("owner").sequence, 3);
        assert_eq!(
            ledger
                .escrow(&second_escrow_id)
                .expect("second escrow")
                .state,
            ESCROW_STATE_CANCELED
        );
        ledger
            .validate_escrow_state(&genesis.chain_id)
            .expect("valid escrow state");
    }

    #[test]
    fn payment_v2_fee_policy_counts_memo_weight_and_state_expansion() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let from = address_from_public_key(&key_pair.public_key);
        let to = "pfnewrecipient0000000000000000000000000".to_string();
        let ledger = LedgerState::new(vec![Account::new(from, 200, Some(public_key_hex))]);
        let no_memo = signed_payment_v2(&genesis, &key_pair, to.clone(), 10, 1, 1, Vec::new());
        let with_memo = signed_payment_v2(
            &genesis,
            &key_pair,
            to.clone(),
            10,
            1,
            1,
            vec![PaymentMemo {
                memo_type: String::new(),
                memo_format: String::new(),
                memo_data: "aa".repeat(256),
            }],
        );
        assert!(payment_v2_weight_bytes(&with_memo) > payment_v2_weight_bytes(&no_memo));

        let base_fee = minimum_payment_v2_fee(&with_memo);
        let underpriced = signed_payment_v2(
            &genesis,
            &key_pair,
            to,
            ACCOUNT_RESERVE,
            base_fee,
            1,
            with_memo.unsigned.memos.clone(),
        );
        let receipt = execute_payment_v2(&genesis, &mut ledger.clone(), &underpriced);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "fee_too_low");
        assert_eq!(receipt.state_expansion_fee, TRANSFER_ACCOUNT_CREATION_FEE);
        assert_eq!(
            receipt.minimum_fee,
            minimum_payment_v2_fee(&underpriced) + TRANSFER_ACCOUNT_CREATION_FEE
        );
    }

    #[test]
    fn payment_v2_tx_id_is_domain_separated_from_legacy_transfer() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let to = "bridge-recipient-000000000000000000000000".to_string();
        let transfer = signed_transfer(&genesis, &key_pair, to.clone(), 25, 3, 1);
        let payment = signed_payment_v2(&genesis, &key_pair, to, 25, 3, 1, Vec::new());

        assert_eq!(96, transfer_tx_id(&transfer).len());
        assert_eq!(96, payment_v2_tx_id(&payment).len());
        assert_ne!(transfer_tx_id(&transfer), payment_v2_tx_id(&payment));
    }

    #[test]
    fn offer_transaction_tx_id_is_domain_separated() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let transfer = signed_transfer(
            &genesis,
            &key_pair,
            "bridge-recipient-000000000000000000000000".to_string(),
            25,
            3,
            11,
        );
        let offer = SignedOfferTransaction {
            unsigned: UnsignedOfferTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: OFFER_CREATE_TRANSACTION_KIND.to_string(),
                signature_algorithm_id: "test".to_string(),
                source: "pfowner0000000000000000000000000000000000".to_string(),
                fee: 5,
                sequence: 11,
                operation: OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                    owner: "pfowner0000000000000000000000000000000000".to_string(),
                    taker_gets_asset_id: "PFT".to_string(),
                    taker_gets_amount: 125,
                    taker_pays_asset_id: "01".repeat(ISSUED_ASSET_ID_HEX_LEN / 2),
                    taker_pays_amount: 50,
                    expiration_height: 25,
                }),
            },
            algorithm_id: "test".to_string(),
            public_key_hex: "00".to_string(),
            signature_hex: "11".to_string(),
        };
        let mut resigned = offer.clone();
        resigned.signature_hex = "22".to_string();

        assert_eq!(96, offer_transaction_tx_id(&offer).len());
        assert_ne!(transfer_tx_id(&transfer), offer_transaction_tx_id(&offer));
        assert_ne!(
            offer_transaction_tx_id(&offer),
            offer_transaction_tx_id(&resigned)
        );
    }
