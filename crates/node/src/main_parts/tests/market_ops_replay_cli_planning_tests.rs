use super::*;
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, ml_dsa_65_keygen, ml_dsa_65_sign, MlDsa65KeyPair,
    ML_DSA_65_ALGORITHM,
};
use postfiat_execution::{execute_asset_transaction, execute_offer_transaction, genesis_hash};
use postfiat_types::{
    issued_asset_id, market_ops_asset_id, market_ops_evidence_root, market_ops_reserve_packet_hash,
    market_ops_supply_packet_hash, nav_proof_profile_id, offer_id,
    vault_bridge_deposit_evidence_root, vault_bridge_deposit_id, vault_bridge_pftl_recipient_hash,
    vault_bridge_route_binding, vault_bridge_source_root_for_asset, Account, AssetCreateOperation,
    AssetDefinition, AssetTransactionOperation, FinalizedMarketOpsEnvelope, Genesis, LedgerState,
    MarketOpsAlignmentParams, MarketOpsEnvelope, MarketOpsMintLimits, MarketOpsPolicyInputs,
    MarketOpsPolicyRegistration, MarketOpsReserveDeployLimits, MarketOpsVenueObservation,
    NavAssetRegisterOperation, NavAttestor, NavAttestorRegisterOperation,
    NavEpochFinalizeOperation, NavProfileRegisterOperation, NavReserveAttestOperation,
    NavReservePacket, NavReserveSubmitOperation, NavTrackedAsset, OfferCreateOperation,
    OfferTransactionOperation, SignedAssetTransaction, SignedOfferTransaction, TrustSetOperation,
    UnsignedAssetTransaction, UnsignedOfferTransaction, VaultBridgeDepositAttestation,
    VaultBridgeDepositEvidence, VaultBridgeDepositRecord, VaultBridgeRedemption, ADDRESS_NAMESPACE,
    ASSET_CREATE_TRANSACTION_KIND, NAV_ASSET_REGISTER_TRANSACTION_KIND,
    NAV_ATTESTOR_REGISTER_TRANSACTION_KIND, NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
    NAV_PROFILE_REGISTER_TRANSACTION_KIND, NAV_PROFILE_VERIFIER_MULTI_FETCH,
    NAV_RESERVE_ATTEST_TRANSACTION_KIND, NAV_RESERVE_STATE_FINALIZED,
    NAV_RESERVE_SUBMIT_TRANSACTION_KIND, OFFER_CREATE_TRANSACTION_KIND, OFFER_STATE_FILLED,
    TRUST_SET_TRANSACTION_KIND, VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
    VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT, VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
    VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND, VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
    VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND, VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED,
    VAULT_BRIDGE_REDEMPTION_STATE_PENDING, VAULT_BRIDGE_UNIT,
};

const PRODUCT_E2E_FEE: u64 = 1_000_000;

fn cli_test_sp1_nav_profile(issuer: &str) -> postfiat_types::NavProofProfile {
    postfiat_types::NavProofProfile::new(
        issuer,
        postfiat_types::NAV_PROFILE_VERIFIER_SP1_GROTH16,
        "stakehub-pol-v2",
        100_000,
        1,
        100_000,
        0,
        0,
        0,
        0,
        "8fcf3cd44c8180744563e85579ed91b7fd3882e560dc41ea4dc0c18cb01f289d",
        "0x004d1cd3f36e6ea60662af428edbea9d3aba45f04fe496da909d6bbe9fbf9258",
        "groth16",
        0,
        0,
    )
    .expect("CLI test SP1 profile")
}

fn copy_test_dir_all(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).expect("create copied dir");
    for entry in std::fs::read_dir(src).expect("read source dir") {
        let entry = entry.expect("read source entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_test_dir_all(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("copy file");
        }
    }
}

fn sign_asset_e2e(
    genesis: &Genesis,
    key_pair: &MlDsa65KeyPair,
    transaction_kind: &str,
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
        fee: PRODUCT_E2E_FEE,
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

fn sign_offer_e2e(
    genesis: &Genesis,
    key_pair: &MlDsa65KeyPair,
    transaction_kind: &str,
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
        fee: PRODUCT_E2E_FEE,
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

fn market_ops_policy_inputs_fixture() -> MarketOpsPolicyInputs {
    MarketOpsPolicyInputs {
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
        discount_observations: vec![
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
        ],
        premium_observations: vec![
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
        ],
    }
}

fn finalized_market_ops_fixture() -> (
    LedgerState,
    String,
    FinalizedMarketOpsEnvelope,
    MarketOpsPolicyRegistration,
) {
    let issuer = "pfissuer";
    let asset_id = "aa".repeat(48);
    let reserve_packet_hash = "ab".repeat(48);
    let policy = market_ops_policy_fixture();
    let policy_inputs = market_ops_policy_inputs_fixture();
    let proof_profile = cli_test_sp1_nav_profile(issuer);

    let mut nav_asset = NavTrackedAsset::new(
        asset_id.clone(),
        issuer,
        issuer,
        proof_profile.profile_id.clone(),
        "usd_e8",
        issuer,
    )
    .expect("nav asset fixture");
    nav_asset.finalized_epoch = 1;
    nav_asset.nav_per_unit = usd_e8(5) as u64;
    nav_asset.circulating_supply = 1_000_000;
    nav_asset.finalized_reserve_packet_hash = reserve_packet_hash.clone();
    nav_asset.finalized_at_height = 5;

    let mut reserve_packet = NavReservePacket::new(
        asset_id.clone(),
        issuer,
        issuer,
        1,
        usd_e8(5) as u64,
        1_000_000,
        usd_e8(5_000_000) as u64,
        proof_profile.profile_id.clone(),
        "01".repeat(48),
        "02".repeat(48),
        reserve_packet_hash.clone(),
    )
    .expect("reserve packet fixture");
    reserve_packet.state = NAV_RESERVE_STATE_FINALIZED.to_string();
    reserve_packet.submitted_at_height = 3;

    let envelope = MarketOpsEnvelope {
        encoding_version: 1,
        chain_id: 1,
        adapter_address: [0x11; 20],
        vault_address: [0x12; 20],
        mint_controller_address: [0x13; 20],
        asset_id: market_ops_asset_id(&asset_id).expect("market ops asset id"),
        epoch: 1,
        program_id: policy.program_id,
        policy_hash: policy.policy_hash,
        parameter_hash: policy.parameter_hash,
        reserve_packet_hash: market_ops_reserve_packet_hash(&reserve_packet_hash)
            .expect("reserve packet hash"),
        supply_packet_hash: market_ops_supply_packet_hash(&asset_id, 1, 1_000_000)
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
        expires_at: 4_000_000_000,
        cooldown_seconds: 600,
        nonce: [0x55; 32],
    };
    let envelope_hash = bytes_to_hex(&envelope.envelope_hash());
    let record = FinalizedMarketOpsEnvelope {
        asset_id: asset_id.clone(),
        epoch: 1,
        envelope_hash,
        envelope,
        policy_inputs: Some(policy_inputs),
        finalized_at_height: 5,
    };
    record.validate().expect("finalized envelope fixture");
    reserve_packet.validate().expect("reserve packet fixture");
    nav_asset.validate().expect("nav asset fixture");
    policy.validate().expect("policy fixture");

    let mut ledger = LedgerState::empty();
    ledger
        .accounts
        .push(Account::new(issuer, 100_000_000, None));
    ledger.nav_proof_profiles.push(proof_profile);
    ledger.nav_assets.push(nav_asset);
    ledger.nav_reserve_packets.push(reserve_packet);
    ledger.market_ops_policies.push(policy.clone());
    ledger.market_ops_envelopes.push(record.clone());

    (ledger, asset_id, record, policy)
}

#[test]
fn market_ops_replay_bundle_cli_exports_and_replays() {
    let root = env::temp_dir().join(format!("postfiat-market-ops-replay-{}", process::id()));
    let data_dir = root.join("node");
    let bundle_dir = root.join("bundle");
    let _ = std::fs::remove_dir_all(&root);

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init node store");

    let (ledger, asset_id, record, policy) = finalized_market_ops_fixture();
    postfiat_storage::NodeStore::new(&data_dir)
        .write_ledger(&ledger)
        .expect("write market ops ledger");

    run_cli(vec![
        "export-envelope-bundle".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--asset-id".to_string(),
        asset_id,
        "--epoch".to_string(),
        "1".to_string(),
        "--bundle".to_string(),
        bundle_dir.display().to_string(),
        "--overwrite".to_string(),
    ])
    .expect("export envelope bundle");

    let bundle_file = bundle_dir.join("bundle.json");
    let bundle: postfiat_node::MarketOpsReplayBundle =
        serde_json::from_str(&std::fs::read_to_string(&bundle_file).expect("read bundle"))
            .expect("parse bundle");
    assert_eq!(record.envelope_hash, bundle.expected_envelope_hash);
    assert_eq!(
        bytes_to_hex(&record.envelope.reserve_packet_hash),
        bundle.reserve_packet_hash
    );
    assert_eq!(
        bytes_to_hex(&record.envelope.supply_packet_hash),
        bundle.supply_packet_hash
    );
    assert_eq!(
        bytes_to_hex(&record.envelope.evidence_root),
        bundle.evidence_root
    );
    assert_eq!(bytes_to_hex(&policy.program_id), bundle.program_id);
    assert_eq!(bytes_to_hex(&policy.policy_hash), bundle.policy_hash);
    assert_eq!(bytes_to_hex(&policy.parameter_hash), bundle.parameter_hash);

    run_cli(vec![
        "replay-envelope".to_string(),
        "--bundle".to_string(),
        bundle_dir.display().to_string(),
    ])
    .expect("replay envelope bundle");

    let report = replay_market_ops_bundle(MarketOpsReplayBundleVerifyOptions {
        bundle_dir: bundle_dir.clone(),
    })
    .expect("replay report");
    assert!(report.verified);
    assert_eq!(record.envelope_hash, report.computed_envelope_hash);
    assert_eq!(record.envelope_hash, report.expected_envelope_hash);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn market_ops_operation_bundle_cli_builds_replayable_operations() {
    let root = env::temp_dir().join(format!(
        "postfiat-market-ops-operation-bundle-{}",
        process::id()
    ));
    let data_dir = root.join("node");
    let bundle_dir = root.join("bundle");
    let policy_file = root.join("policy.json");
    let policy_inputs_file = root.join("policy-inputs.json");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create root");

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init node store");

    let (ledger, asset_id, record, policy) = finalized_market_ops_fixture();
    let policy_inputs = record.policy_inputs.clone().expect("fixture policy inputs");
    postfiat_storage::NodeStore::new(&data_dir)
        .write_ledger(&ledger)
        .expect("write market ops ledger");
    std::fs::write(
        &policy_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&policy).expect("serialize policy")
        ),
    )
    .expect("write policy file");
    std::fs::write(
        &policy_inputs_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&policy_inputs).expect("serialize policy inputs")
        ),
    )
    .expect("write policy inputs file");

    run_cli(vec![
        "market-ops-operation-bundle".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--asset-id".to_string(),
        asset_id.clone(),
        "--policy-file".to_string(),
        policy_file.display().to_string(),
        "--policy-inputs-file".to_string(),
        policy_inputs_file.display().to_string(),
        "--bundle".to_string(),
        bundle_dir.display().to_string(),
        "--evm-chain-id".to_string(),
        "1".to_string(),
        "--adapter-address".to_string(),
        "11".repeat(20),
        "--vault-address".to_string(),
        "12".repeat(20),
        "--mint-controller-address".to_string(),
        "13".repeat(20),
        "--funded-alignment-reserve-usd-e8".to_string(),
        usd_e8(150_000).to_string(),
        "--discount-trigger-bps".to_string(),
        "300".to_string(),
        "--premium-trigger-bps".to_string(),
        "1000".to_string(),
        "--data-window-start".to_string(),
        "100".to_string(),
        "--data-window-end".to_string(),
        "10100".to_string(),
        "--valid-after".to_string(),
        "10100".to_string(),
        "--expires-at".to_string(),
        "4000000000".to_string(),
        "--cooldown-seconds".to_string(),
        "600".to_string(),
        "--nonce".to_string(),
        "55".repeat(32),
        "--overwrite".to_string(),
    ])
    .expect("build market ops operation bundle");

    let operation_bundle_file = bundle_dir.join("operation-bundle.json");
    let bundle: postfiat_node::MarketOpsOperationBundle = serde_json::from_str(
        &std::fs::read_to_string(&operation_bundle_file).expect("read operation bundle"),
    )
    .expect("parse operation bundle");
    assert_eq!(record.envelope_hash, bundle.expected_envelope_hash);
    assert_eq!(record.envelope, bundle.envelope);
    assert!(matches!(
        bundle.policy_register_operation,
        AssetTransactionOperation::MarketOpsPolicyRegister(_)
    ));
    assert!(matches!(
        bundle.market_ops_finalize_operation,
        AssetTransactionOperation::MarketOpsFinalize(_)
    ));
    assert!(bundle_dir.join("policy-register.operation.json").is_file());
    assert!(bundle_dir
        .join("market-ops-finalize.operation.json")
        .is_file());
    assert!(bundle_dir.join("commands.sh").is_file());

    let policy_register_operation_json =
        std::fs::read_to_string(bundle_dir.join("policy-register.operation.json"))
            .expect("read policy register operation");
    let policy_quote = asset_fee_quote(AssetFeeQuoteOptions {
        data_dir: data_dir.clone(),
        source: "pfissuer".to_string(),
        operation_json: policy_register_operation_json,
        sequence: None,
    })
    .expect("quote policy register operation");
    assert_eq!("market_ops_policy_register", policy_quote.transaction_kind);

    let market_ops_finalize_operation_json =
        std::fs::read_to_string(bundle_dir.join("market-ops-finalize.operation.json"))
            .expect("read market ops finalize operation");
    let finalize_quote = asset_fee_quote(AssetFeeQuoteOptions {
        data_dir: data_dir.clone(),
        source: "pfissuer".to_string(),
        operation_json: market_ops_finalize_operation_json,
        sequence: Some(policy_quote.sequence + 1),
    })
    .expect("quote market ops finalize operation");
    assert_eq!("market_ops_finalize", finalize_quote.transaction_kind);

    let report = replay_market_ops_bundle(MarketOpsReplayBundleVerifyOptions {
        bundle_dir: bundle_dir.join("replay"),
    })
    .expect("replay generated bundle");
    assert!(report.verified);
    assert_eq!(record.envelope_hash, report.computed_envelope_hash);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn market_ops_status_cli_reports_required_public_fields() {
    let root = env::temp_dir().join(format!("postfiat-market-ops-status-{}", process::id()));
    let data_dir = root.join("node");
    let _ = std::fs::remove_dir_all(&root);

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init node store");

    let (ledger, asset_id, record, policy) = finalized_market_ops_fixture();
    postfiat_storage::NodeStore::new(&data_dir)
        .write_ledger(&ledger)
        .expect("write market ops ledger");

    run_cli(vec![
        "market-ops-status".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--asset-id".to_string(),
        asset_id.clone(),
    ])
    .expect("market ops status cli");

    let report = market_ops_status(MarketOpsStatusOptions {
        data_dir: data_dir.clone(),
        asset_id,
        epoch: None,
    })
    .expect("market ops status report");
    assert_eq!(
        postfiat_node::MARKET_OPS_PUBLIC_STATUS_SCHEMA,
        report.schema
    );
    assert_eq!(
        postfiat_node::MARKET_OPS_STATUS_ACTIVE,
        report.market_operations_status
    );
    assert_eq!(record.envelope.nav_floor_usd_e8, report.nav_floor_usd_e8);
    assert_eq!(
        record.envelope.verified_net_assets_usd_e8,
        report.verified_net_assets_usd_e8
    );
    assert_eq!(
        record.envelope.valid_global_supply_atoms,
        report.valid_global_supply_atoms
    );
    assert!(report.reserve_packet_fresh);
    assert!(report.supply_packet_fresh);
    assert_eq!(
        record.envelope.funded_alignment_reserve_usd_e8,
        report.funded_alignment_reserve_usd_e8
    );
    assert_eq!(
        record.envelope.required_alignment_reserve_usd_e8,
        report.required_alignment_reserve_usd_e8
    );
    assert_eq!(
        record.envelope.max_reserve_deploy_usd_e8,
        report.current_reserve_deploy_cap_usd_e8
    );
    assert_eq!(
        record.envelope.max_mint_atoms,
        report.current_mint_cap_atoms
    );
    assert_eq!(
        bytes_to_hex(&policy.policy_hash),
        report.accepted_policy_hash
    );
    assert_eq!(record.envelope_hash, report.envelope_hash);
    assert_eq!(record.epoch, report.envelope_epoch);
    assert_eq!(record.envelope.expires_at, report.packet_expires_at);
    assert_eq!(
        postfiat_node::MARKET_OPS_PUBLIC_DISCLOSURE,
        report.disclosure
    );
    for forbidden in [
        "redemption facility",
        "guaranteed support",
        "guaranteed liquidity",
        "stable value",
        "instant exit at NAV",
    ] {
        assert!(
            !report
                .disclosure
                .to_ascii_lowercase()
                .contains(&forbidden.to_ascii_lowercase()),
            "disclosure contains forbidden phrase {forbidden}"
        );
    }

    let mut malformed_hash = report.clone();
    malformed_hash.envelope_hash = "ff".repeat(47);
    let error = malformed_hash
        .validate()
        .expect_err("short finalized envelope hash must fail response validation");
    assert!(error.contains("envelope_hash"), "{error}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn atomic_swap_quote_accepts_exact_finalized_single_nav_envelope_and_sdk_binds_it() {
    let root = env::temp_dir().join(format!("postfiat-atomic-swap-nav-quote-{}", process::id()));
    let data_dir = root.join("node");
    let _ = std::fs::remove_dir_all(&root);

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init node store");

    let store = postfiat_storage::NodeStore::new(&data_dir);
    let mut genesis = store.read_genesis().expect("read genesis");
    genesis.atomic_swap_activation_height = Some(0);
    store
        .write_genesis(&genesis)
        .expect("activate atomic swaps");
    let mut chain_tip = store.read_chain_tip().expect("read chain tip");
    chain_tip.genesis_hash = genesis_hash(&genesis);
    store
        .write_chain_tip(&chain_tip)
        .expect("bind chain tip to activated genesis");

    let owner_0 = format!("pf{}", "01".repeat(20));
    let owner_1 = format!("pf{}", "02".repeat(20));
    let issuer_0 = format!("pf{}", "03".repeat(20));
    let issuer_1 = format!("pf{}", "04".repeat(20));
    let non_nav_asset_id = "bb".repeat(48);
    let (mut ledger, nav_asset_id, record, _) = finalized_market_ops_fixture();
    let nav_asset = ledger
        .nav_assets
        .iter_mut()
        .find(|asset| asset.asset_id == nav_asset_id)
        .expect("NAV asset fixture");
    nav_asset.issuer = issuer_0.clone();
    nav_asset.reserve_operator = issuer_0.clone();
    nav_asset.redemption_account = issuer_0.clone();
    ledger.asset_definitions.extend([
        AssetDefinition {
            asset_id: nav_asset_id.clone(),
            issuer: issuer_0.clone(),
            code: "NAVA".to_string(),
            version: 1,
            precision: 6,
            display_name: String::new(),
            max_supply: None,
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
        },
        AssetDefinition {
            asset_id: non_nav_asset_id.clone(),
            issuer: issuer_1.clone(),
            code: "SETB".to_string(),
            version: 1,
            precision: 6,
            display_name: String::new(),
            max_supply: None,
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
        },
    ]);
    ledger.accounts.extend([
        Account::new(&owner_0, 100_000_000, None),
        Account::new(&owner_1, 100_000_000, None),
    ]);
    store.write_ledger(&ledger).expect("write quote ledger");

    let request = postfiat_rpc_sdk::atomic_swap_fee_quote_request(
        "atomic-nav-quote",
        "c1".repeat(48),
        record.envelope_hash.clone(),
        record.epoch,
        100,
        "d2".repeat(48),
        owner_0.clone(),
        owner_1.clone(),
        issuer_0.clone(),
        nav_asset_id.clone(),
        20_000,
        owner_1.clone(),
        owner_0.clone(),
        issuer_1.clone(),
        non_nav_asset_id.clone(),
        30_000,
    );
    let report = atomic_swap_fee_quote(AtomicSwapFeeQuoteOptions {
        data_dir: data_dir.clone(),
        rfq_hash: "c1".repeat(48),
        market_envelope_hash: record.envelope_hash.clone(),
        nav_epoch: record.epoch,
        expires_at_height: 100,
        swap_nonce: "d2".repeat(48),
        leg_0: AtomicSwapQuoteLegInput {
            owner: owner_0.clone(),
            recipient: owner_1.clone(),
            issuer: issuer_0,
            asset_id: nav_asset_id.clone(),
            amount: 20_000,
        },
        leg_1: AtomicSwapQuoteLegInput {
            owner: owner_1,
            recipient: owner_0,
            issuer: issuer_1,
            asset_id: non_nav_asset_id.clone(),
            amount: 30_000,
        },
    })
    .expect("exact finalized one-NAV envelope quote");
    let response = postfiat_rpc_sdk::success_response(&request.id, &report, Vec::new())
        .expect("serialize quote response");
    let summary = postfiat_rpc_sdk::decode_atomic_swap_fee_quote_summary(&response, &request)
        .expect("SDK binds exact one-NAV quote response to request");

    assert_eq!(
        record.envelope_hash,
        summary.unsigned_transaction.market_envelope_hash
    );
    assert_eq!(record.epoch, summary.unsigned_transaction.nav_epoch);
    assert_eq!(nav_asset_id, summary.unsigned_transaction.leg_0.asset_id);
    assert_eq!(
        non_nav_asset_id,
        summary.unsigned_transaction.leg_1.asset_id
    );
    assert_eq!(
        ledger,
        store.read_ledger().expect("quote leaves ledger unchanged")
    );
    let serialized = serde_json::to_string(&response).expect("serialize response");
    for forbidden in ["trustline", "trust_set", "line_create"] {
        assert!(!serialized.contains(forbidden), "found `{forbidden}`");
    }

    let _ = std::fs::remove_dir_all(root);
}

fn abi_word_u64(value: u64) -> Vec<u8> {
    let mut word = vec![0_u8; 24];
    word.extend_from_slice(&value.to_be_bytes());
    word
}

fn abi_word_bytes32(hex: &str) -> Vec<u8> {
    hex_to_bytes(hex).expect("bytes32 hex")
}

fn abi_word_address(address: &str) -> Vec<u8> {
    let mut word = vec![0_u8; 12];
    word.extend_from_slice(&hex_to_bytes(&address[2..]).expect("address hex"));
    word
}

fn indexed_address_topic(address: &str) -> String {
    format!("0x{}{}", "00".repeat(12), &address[2..])
}

fn abi_string_tail(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut out = abi_word_u64(bytes.len() as u64);
    out.extend_from_slice(bytes);
    let padding = (32 - (bytes.len() % 32)) % 32;
    out.extend(std::iter::repeat_n(0_u8, padding));
    out
}

fn vault_bridge_vault_deposit_log_json(evidence: &VaultBridgeDepositEvidence) -> serde_json::Value {
    let mut data = Vec::new();
    data.extend(abi_word_u64(7 * 32));
    data.extend(abi_word_u64(evidence.amount_atoms));
    data.extend(abi_word_bytes32(&evidence.nonce));
    data.extend(abi_word_bytes32(&evidence.route_binding));
    data.extend(abi_word_u64(evidence.source_chain_id));
    data.extend(abi_word_address(&evidence.vault_address));
    data.extend(abi_word_address(&evidence.token_address));
    data.extend(abi_string_tail(&evidence.pftl_recipient));

    serde_json::json!({
        "address": evidence.vault_address,
        "blockHash": format!("0x{}", evidence.block_hash),
        "transactionHash": format!("0x{}", evidence.tx_hash),
        "logIndex": format!("0x{:x}", evidence.log_index),
        "topics": [
            "0x7564437da24aa33f24442c214d7047d8bf275a86555bc57b83be448783cd6d81",
            format!("0x{}", evidence.deposit_id),
            indexed_address_topic(&evidence.depositor),
            format!("0x{}", evidence.pftl_recipient_hash),
        ],
        "data": format!("0x{}", bytes_to_hex(&data)),
    })
}

fn vault_bridge_deposit_evidence_fixture() -> VaultBridgeDepositEvidence {
    let pftl_recipient = "vault_bridge-holder".to_string();
    let pftl_recipient_hash =
        vault_bridge_pftl_recipient_hash(&pftl_recipient).expect("recipient hash");
    let mut evidence = VaultBridgeDepositEvidence {
        source_chain_id: 42_161,
        vault_address: "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0".to_string(),
        token_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        depositor: "0x1111111111111111111111111111111111111111".to_string(),
        pftl_recipient,
        pftl_recipient_hash,
        amount_atoms: 10_000_099,
        nonce: "22".repeat(32),
        route_binding: vault_bridge_route_binding(&"24".repeat(48), 7).expect("route binding"),
        deposit_id: String::new(),
        block_hash: "44".repeat(32),
        tx_hash: "55".repeat(32),
        log_index: 7,
    };
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
    evidence
}

#[test]
fn vault_bridge_asset_id_writes_predeploy_env() {
    let root = env::temp_dir().join(format!("postfiat-vault-bridge-asset-id-{}", process::id()));
    let env_file = root.join("predeploy.env");
    let cli_env_file = root.join("predeploy-cli.env");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");

    let issuer = "vault_bridge-issuer";
    let asset_code = "BRIDGEASSET";
    let asset_version = 3;
    let expected_asset_id =
        issued_asset_id(DEFAULT_CHAIN_ID, issuer, asset_code, asset_version).expect("asset id");
    let expected_evm_asset_id = format!("0x{expected_asset_id}");

    let report = vault_bridge_asset_id(VaultBridgeAssetIdOptions {
        pftl_chain_id: DEFAULT_CHAIN_ID.to_string(),
        issuer: issuer.to_string(),
        asset_code: asset_code.to_string(),
        asset_version,
        env_file: Some(env_file.clone()),
        overwrite: false,
    })
    .expect("derive vault bridge asset id");

    assert_eq!(
        report.schema,
        postfiat_node::VAULT_BRIDGE_ASSET_ID_REPORT_SCHEMA
    );
    assert_eq!(report.asset_id, expected_asset_id);
    assert_eq!(report.evm_asset_id, expected_evm_asset_id);
    assert_eq!(report.env_file, Some(env_file.display().to_string()));
    let env_contents = std::fs::read_to_string(&env_file).expect("read env file");
    assert!(env_contents.contains(&format!("VAULT_BRIDGE_ASSET_ID={}", report.evm_asset_id)));
    assert!(env_contents.contains("VAULT_BRIDGE_ASSET_CODE=BRIDGEASSET"));
    assert!(env_contents.contains("VAULT_BRIDGE_ASSET_VERSION=3"));

    let error = vault_bridge_asset_id(VaultBridgeAssetIdOptions {
        pftl_chain_id: DEFAULT_CHAIN_ID.to_string(),
        issuer: issuer.to_string(),
        asset_code: asset_code.to_string(),
        asset_version,
        env_file: Some(env_file),
        overwrite: false,
    })
    .expect_err("refuse accidental overwrite");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);

    run_cli(vec![
        "vault-bridge-asset-id".to_string(),
        "--pftl-chain-id".to_string(),
        DEFAULT_CHAIN_ID.to_string(),
        "--issuer".to_string(),
        issuer.to_string(),
        "--asset-code".to_string(),
        asset_code.to_string(),
        "--asset-version".to_string(),
        asset_version.to_string(),
        "--env-file".to_string(),
        cli_env_file.display().to_string(),
    ])
    .expect("vault bridge asset id cli");
    let cli_env_contents = std::fs::read_to_string(cli_env_file).expect("read cli env file");
    assert!(cli_env_contents.contains(&format!("VAULT_BRIDGE_ASSET_ID={}", report.evm_asset_id)));
}

#[test]
fn vault_bridge_bootstrap_bundle_writes_pftl_setup_operations() {
    let root = env::temp_dir().join(format!(
        "postfiat-vault-bridge-bootstrap-bundle-{}",
        process::id()
    ));
    let bundle_dir = root.join("bootstrap-bundle");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");

    let evidence = vault_bridge_deposit_evidence_fixture();
    let issuer = "vault_bridge-issuer";
    let holder = "vault_bridge-holder";
    let buyer = "vault_bridge-buyer";
    let asset_code = "BRIDGEASSET";
    let asset_version = 3;
    let policy_hash = "42".repeat(48);
    let report = vault_bridge_bootstrap_bundle(VaultBridgeBootstrapBundleOptions {
        pftl_chain_id: DEFAULT_CHAIN_ID.to_string(),
        source_chain_id: evidence.source_chain_id,
        vault_address: evidence.vault_address.clone(),
        token_address: evidence.token_address.clone(),
        issuer: issuer.to_string(),
        reserve_operator: issuer.to_string(),
        redemption_account: issuer.to_string(),
        asset_code: asset_code.to_string(),
        asset_version,
        asset_precision: 8,
        asset_display_name: "vault bridge asset".to_string(),
        max_supply: Some(100_000_000),
        valuation_unit: "SOURCE_UNIT".to_string(),
        verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
        max_snapshot_age_blocks: 100,
        challenge_window_blocks: 1,
        max_epoch_gap_blocks: 100,
        settle_deadline_blocks: 0,
        min_challenge_bond: 0,
        min_attestations: 1,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash: policy_hash.clone(),
        trust_accounts: vec![holder.to_string(), buyer.to_string()],
        trust_limit: 100_000_000,
        trust_reserve_paid: 10,
        bundle_dir: bundle_dir.clone(),
        overwrite: false,
    })
    .expect("write bootstrap bundle");

    let source_domain = evidence.source_domain();
    let source_class = format!("vault_bridge:{source_domain}");
    let expected_asset_id =
        issued_asset_id(DEFAULT_CHAIN_ID, issuer, asset_code, asset_version).expect("asset id");
    let expected_profile_id = nav_proof_profile_id(
        NAV_PROFILE_VERIFIER_MULTI_FETCH,
        &source_class,
        100,
        1,
        100,
        0,
        0,
        1,
        0,
        &policy_hash,
        "",
        "",
        0,
        0,
    )
    .expect("profile id");

    assert_eq!(
        report.schema,
        postfiat_node::VAULT_BRIDGE_BOOTSTRAP_BUNDLE_SCHEMA
    );
    assert_eq!(report.source_domain, source_domain);
    assert_eq!(report.source_class, source_class);
    assert_eq!(report.asset_id, expected_asset_id);
    assert_eq!(report.profile_id, expected_profile_id);
    assert!(bundle_dir.join("profile-register.operation.json").exists());
    assert!(bundle_dir.join("asset-create.operation.json").exists());
    assert!(bundle_dir
        .join("nav-asset-register.operation.json")
        .exists());
    assert!(bundle_dir.join("trust-set-0.operation.json").exists());
    assert!(bundle_dir.join("trust-set-1.operation.json").exists());
    assert!(bundle_dir.join("commands.sh").exists());
    assert_eq!(report.trust_set_operation_files.len(), 2);
    assert_eq!(report.trust_set_operations.len(), 2);
    assert!(report
        .commands
        .iter()
        .any(|command| command.contains("profile-register.operation.json")));
    assert!(report
        .commands
        .iter()
        .any(|command| command.contains("ISSUER_KEY_FILE")));
    assert!(report
        .commands
        .iter()
        .any(|command| command.contains("trust-set-0.operation.json")));
    assert!(report
        .commands
        .iter()
        .any(|command| command.contains("TRUST_ACCOUNT_0_KEY_FILE")));
    let commands =
        std::fs::read_to_string(bundle_dir.join("commands.sh")).expect("read commands script");
    assert!(commands.contains("ISSUER_KEY_FILE"));
    assert!(commands.contains("TRUST_ACCOUNT_0_KEY_FILE"));
    assert!(commands.contains("TRUST_ACCOUNT_1_KEY_FILE"));

    let cli_bundle_dir = root.join("bootstrap-bundle-cli");
    run_cli(vec![
        "vault-bridge-bootstrap-bundle".to_string(),
        "--pftl-chain-id".to_string(),
        DEFAULT_CHAIN_ID.to_string(),
        "--source-chain-id".to_string(),
        evidence.source_chain_id.to_string(),
        "--vault-address".to_string(),
        evidence.vault_address.clone(),
        "--token-address".to_string(),
        evidence.token_address.clone(),
        "--issuer".to_string(),
        issuer.to_string(),
        "--asset-code".to_string(),
        asset_code.to_string(),
        "--asset-version".to_string(),
        asset_version.to_string(),
        "--asset-precision".to_string(),
        "8".to_string(),
        "--asset-display-name".to_string(),
        "vault bridge asset".to_string(),
        "--valuation-unit".to_string(),
        "SOURCE_UNIT".to_string(),
        "--valuation-policy-hash".to_string(),
        policy_hash.clone(),
        "--trust-accounts".to_string(),
        format!("{holder},{buyer}"),
        "--bundle".to_string(),
        cli_bundle_dir.display().to_string(),
    ])
    .expect("vault bridge bootstrap bundle cli");
    assert!(cli_bundle_dir
        .join("profile-register.operation.json")
        .exists());
    assert!(cli_bundle_dir.join("asset-create.operation.json").exists());
    assert!(cli_bundle_dir
        .join("nav-asset-register.operation.json")
        .exists());
    assert!(cli_bundle_dir.join("commands.sh").exists());

    let error = vault_bridge_bootstrap_bundle(VaultBridgeBootstrapBundleOptions {
        pftl_chain_id: DEFAULT_CHAIN_ID.to_string(),
        source_chain_id: evidence.source_chain_id,
        vault_address: evidence.vault_address,
        token_address: evidence.token_address,
        issuer: issuer.to_string(),
        reserve_operator: issuer.to_string(),
        redemption_account: issuer.to_string(),
        asset_code: asset_code.to_string(),
        asset_version,
        asset_precision: 8,
        asset_display_name: "vault bridge asset".to_string(),
        max_supply: Some(100_000_000),
        valuation_unit: "SOURCE_UNIT".to_string(),
        verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
        max_snapshot_age_blocks: 100,
        challenge_window_blocks: 1,
        max_epoch_gap_blocks: 100,
        settle_deadline_blocks: 0,
        min_challenge_bond: 0,
        min_attestations: 1,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash: policy_hash,
        trust_accounts: vec![holder.to_string()],
        trust_limit: 100_000_000,
        trust_reserve_paid: 10,
        bundle_dir,
        overwrite: false,
    })
    .expect_err("refuse overwrite by default");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn vault_bridge_deposit_intent_prepares_source_chain_deposit() {
    let evidence = vault_bridge_deposit_evidence_fixture();
    let asset_id = "33".repeat(48);
    let policy_hash = "24".repeat(48);
    let route_epoch = 7;
    let report = vault_bridge_deposit_intent(VaultBridgeDepositIntentOptions {
        source_chain_id: evidence.source_chain_id,
        vault_address: evidence.vault_address.clone(),
        token_address: evidence.token_address.clone(),
        depositor: evidence.depositor.clone(),
        amount_atoms: evidence.amount_atoms,
        pftl_recipient: evidence.pftl_recipient.clone(),
        nonce: evidence.nonce.clone(),
        asset_id: asset_id.clone(),
        policy_hash: policy_hash.clone(),
        route_epoch,
        proposer: Some("bridge-relayer".to_string()),
        expires_at_height: Some(100),
        bundle_dir: Some(PathBuf::from("deposit-relay-bundle")),
    })
    .expect("build deposit intent");

    assert_eq!(
        report.schema,
        postfiat_node::VAULT_BRIDGE_DEPOSIT_INTENT_SCHEMA
    );
    assert_eq!(report.source_chain_id, evidence.source_chain_id);
    assert_eq!(report.vault_address, evidence.vault_address);
    assert_eq!(report.token_address, evidence.token_address);
    assert_eq!(report.depositor, evidence.depositor);
    assert_eq!(report.amount_atoms, evidence.amount_atoms);
    assert_eq!(report.pftl_recipient, evidence.pftl_recipient);
    assert_eq!(report.pftl_recipient_hash, evidence.pftl_recipient_hash);
    assert_eq!(report.nonce, format!("0x{}", evidence.nonce));
    let mut bound_evidence = evidence.clone();
    bound_evidence.route_binding =
        vault_bridge_route_binding(&policy_hash, route_epoch).expect("route binding");
    bound_evidence.deposit_id = vault_bridge_deposit_id(&bound_evidence).expect("deposit id");
    assert_eq!(
        report.expected_deposit_id,
        format!("0x{}", bound_evidence.deposit_id)
    );
    assert_eq!(
        report.route_binding,
        format!("0x{}", bound_evidence.route_binding)
    );
    assert_eq!(report.route_epoch, route_epoch);
    assert_eq!(report.source_domain, evidence.source_domain());
    assert_eq!(report.source_asset, evidence.source_asset_ref());
    assert_eq!(
        report.source_tx_or_attestation,
        evidence.source_tx_or_attestation()
    );
    assert_eq!(
        report.approve_signature,
        "approve(address,uint256)".to_string()
    );
    assert_eq!(
        report.deposit_signature,
        "depositV2(uint256,string,bytes32,bytes32)".to_string()
    );
    assert!(report
        .approve_cast_command
        .contains(&format!("--from {}", evidence.depositor)));
    assert!(report
        .approve_cast_command
        .contains(&evidence.token_address));
    assert!(report
        .approve_cast_command
        .contains(&evidence.vault_address));
    assert!(report
        .deposit_cast_command
        .contains(&format!("--from {}", evidence.depositor)));
    assert!(report
        .deposit_cast_command
        .contains(&evidence.vault_address));
    assert!(report
        .deposit_cast_command
        .contains(&evidence.amount_atoms.to_string()));
    assert!(report
        .deposit_cast_command
        .contains(&format!("0x{}", evidence.nonce)));
    assert!(report.relay_bundle_command.contains(&asset_id));
    assert!(report.relay_bundle_command.contains(&policy_hash));
    assert!(report.relay_bundle_command.contains("bridge-relayer"));
    assert!(report.relay_bundle_command.contains("deposit-receipt.json"));
    assert!(report
        .relay_rpc_bundle_command
        .contains("vault-bridge-deposit-relay-rpc-bundle"));
    assert!(report.relay_rpc_bundle_command.contains(&asset_id));
    assert!(report.relay_rpc_bundle_command.contains(&policy_hash));
    assert!(report
        .relay_rpc_bundle_command
        .contains("--tx-hash <deposit_tx_hash>"));
}

#[test]
fn vault_bridge_deposit_plan_builds_canonical_operations_from_vault_log() {
    let root = env::temp_dir().join(format!(
        "postfiat-vault-bridge-deposit-plan-{}",
        process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let log_file = root.join("deposit-log.json");
    let evidence = vault_bridge_deposit_evidence_fixture();
    let log = vault_bridge_vault_deposit_log_json(&evidence);
    std::fs::write(
        &log_file,
        serde_json::to_string_pretty(&log).expect("log json"),
    )
    .expect("write log");

    let asset_id = "33".repeat(48);
    let policy_hash = "24".repeat(48);
    let report = vault_bridge_deposit_plan(VaultBridgeDepositPlanOptions {
        log_file: Some(log_file),
        receipt_file: None,
        vault_address: None,
        token_address: None,
        asset_id: asset_id.clone(),
        policy_hash: policy_hash.clone(),
        proposer: "bridge-relayer".to_string(),
        finalizer: "bridge-finalizer".to_string(),
        claimer: "bridge-claimer".to_string(),
        attestor: Some("bridge-attestor".to_string()),
        observer_confirmation_depth: None,
        expires_at_height: 100,
        source_proof_kind: None,
        source_proof_hash: None,
        source_public_values_hash: None,
        source_proof_file: None,
        source_public_values_file: None,
    })
    .expect("plan bridge deposit");

    assert_eq!(
        report.schema,
        postfiat_node::VAULT_BRIDGE_DEPOSIT_PLAN_SCHEMA
    );
    assert_eq!(report.asset_id, asset_id);
    assert_eq!(report.policy_hash, policy_hash);
    assert_eq!(report.evidence, evidence);
    assert_eq!(
        report.evidence_root,
        vault_bridge_deposit_evidence_root(&evidence).expect("evidence root")
    );
    assert_eq!(report.source_domain, evidence.source_domain());
    assert_eq!(
        report.source_tx_or_attestation,
        evidence.source_tx_or_attestation()
    );
    assert_eq!(report.finality_ref, evidence.finality_ref());
    assert_eq!(report.vault_id, evidence.vault_id());
    assert!(!report.source_public_values_hash.is_empty());

    match &report.propose_operation {
        AssetTransactionOperation::VaultBridgeDepositPropose(operation) => {
            assert_eq!(operation.proposer, "bridge-relayer");
            assert_eq!(operation.evidence_root, report.evidence_root);
            assert_eq!(operation.evidence, evidence);
            assert!(operation.source_proof_kind.is_empty());
            assert!(operation.source_proof_hash.is_empty());
            assert!(operation.source_public_values_hash.is_empty());
        }
        other => panic!("unexpected propose operation: {other:?}"),
    }
    match report.attest_operation.as_ref().expect("attest operation") {
        AssetTransactionOperation::VaultBridgeDepositAttest(operation) => {
            assert_eq!(operation.attestor, "bridge-attestor");
            assert!(operation.pass);
            assert_eq!(operation.observation_root, report.evidence_root);
        }
        other => panic!("unexpected attest operation: {other:?}"),
    }
    match &report.finalize_operation {
        AssetTransactionOperation::VaultBridgeDepositFinalize(operation) => {
            assert_eq!(operation.finalizer, "bridge-finalizer");
            assert_eq!(operation.evidence_root, report.evidence_root);
        }
        other => panic!("unexpected finalize operation: {other:?}"),
    }
    match &report.claim_operation {
        AssetTransactionOperation::VaultBridgeDepositClaim(operation) => {
            assert_eq!(operation.claimer, "bridge-claimer");
            assert_eq!(operation.recipient, evidence.pftl_recipient);
            assert_eq!(operation.amount_atoms, evidence.amount_atoms);
            assert_eq!(operation.evidence_root, report.evidence_root);
        }
        other => panic!("unexpected claim operation: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn vault_bridge_deposit_plan_selects_deposit_log_from_receipt() {
    let root = env::temp_dir().join(format!(
        "postfiat-vault-bridge-deposit-receipt-plan-{}",
        process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let receipt_file = root.join("receipt.json");
    let evidence = vault_bridge_deposit_evidence_fixture();
    let mut deposit_log = vault_bridge_vault_deposit_log_json(&evidence);
    deposit_log
        .as_object_mut()
        .expect("deposit log object")
        .remove("blockHash");
    deposit_log
        .as_object_mut()
        .expect("deposit log object")
        .remove("transactionHash");
    let receipt = serde_json::json!({
        "blockHash": format!("0x{}", evidence.block_hash),
        "transactionHash": format!("0x{}", evidence.tx_hash),
        "logs": [
            {
                "address": evidence.vault_address,
                "topics": ["0x0000000000000000000000000000000000000000000000000000000000000000"],
                "data": "0x",
                "logIndex": "0x6"
            },
            deposit_log
        ]
    });
    std::fs::write(
        &receipt_file,
        serde_json::to_string_pretty(&receipt).expect("receipt json"),
    )
    .expect("write receipt");

    let report = vault_bridge_deposit_plan(VaultBridgeDepositPlanOptions {
        log_file: None,
        receipt_file: Some(receipt_file),
        vault_address: Some(evidence.vault_address.clone()),
        token_address: Some(evidence.token_address.clone()),
        asset_id: "33".repeat(48),
        policy_hash: "24".repeat(32),
        proposer: "bridge-relayer".to_string(),
        finalizer: "bridge-finalizer".to_string(),
        claimer: "bridge-claimer".to_string(),
        attestor: None,
        observer_confirmation_depth: None,
        expires_at_height: 100,
        source_proof_kind: None,
        source_proof_hash: None,
        source_public_values_hash: None,
        source_proof_file: None,
        source_public_values_file: None,
    })
    .expect("plan bridge deposit from receipt");

    assert_eq!(report.evidence, evidence);
    assert_eq!(report.source_domain, evidence.source_domain());
    assert_eq!(report.finality_ref, evidence.finality_ref());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn vault_bridge_deposit_relay_bundle_writes_operations_and_commands() {
    let root = env::temp_dir().join(format!(
        "postfiat-vault-bridge-deposit-relay-bundle-{}",
        process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let receipt_file = root.join("receipt.json");
    let bundle_dir = root.join("bundle");
    let evidence = vault_bridge_deposit_evidence_fixture();
    let mut deposit_log = vault_bridge_vault_deposit_log_json(&evidence);
    deposit_log
        .as_object_mut()
        .expect("deposit log object")
        .remove("blockHash");
    deposit_log
        .as_object_mut()
        .expect("deposit log object")
        .remove("transactionHash");
    let receipt = serde_json::json!({
        "blockHash": format!("0x{}", evidence.block_hash),
        "transactionHash": format!("0x{}", evidence.tx_hash),
        "logs": [deposit_log],
    });
    std::fs::write(
        &receipt_file,
        serde_json::to_string_pretty(&receipt).expect("receipt json"),
    )
    .expect("write receipt");

    let plan_options = VaultBridgeDepositPlanOptions {
        log_file: None,
        receipt_file: Some(receipt_file),
        vault_address: Some(evidence.vault_address.clone()),
        token_address: Some(evidence.token_address.clone()),
        asset_id: "33".repeat(48),
        policy_hash: "24".repeat(32),
        proposer: "bridge-relayer".to_string(),
        finalizer: "bridge-finalizer".to_string(),
        claimer: "bridge-claimer".to_string(),
        attestor: Some("bridge-attestor".to_string()),
        observer_confirmation_depth: None,
        expires_at_height: 100,
        source_proof_kind: None,
        source_proof_hash: None,
        source_public_values_hash: None,
        source_proof_file: None,
        source_public_values_file: None,
    };
    let report = vault_bridge_deposit_relay_bundle(VaultBridgeDepositRelayBundleOptions {
        plan_options: plan_options.clone(),
        bundle_dir: bundle_dir.clone(),
        overwrite: false,
    })
    .expect("write relay bundle");

    assert_eq!(
        report.schema,
        postfiat_node::VAULT_BRIDGE_DEPOSIT_RELAY_BUNDLE_SCHEMA
    );
    assert!(bundle_dir.join("plan.json").exists());
    assert!(bundle_dir.join("propose.operation.json").exists());
    assert!(bundle_dir.join("attest.operation.json").exists());
    assert!(bundle_dir.join("finalize.operation.json").exists());
    assert!(bundle_dir.join("claim.operation.json").exists());
    assert!(bundle_dir.join("commands.sh").exists());

    let propose_operation: AssetTransactionOperation = serde_json::from_str(
        &std::fs::read_to_string(bundle_dir.join("propose.operation.json"))
            .expect("read propose operation"),
    )
    .expect("parse propose operation");
    match propose_operation {
        AssetTransactionOperation::VaultBridgeDepositPropose(operation) => {
            assert_eq!(operation.proposer, "bridge-relayer");
            assert_eq!(operation.evidence, evidence);
            assert_eq!(operation.evidence_root, report.plan.evidence_root);
        }
        other => panic!("unexpected propose operation: {other:?}"),
    }

    let commands =
        std::fs::read_to_string(bundle_dir.join("commands.sh")).expect("read commands script");
    assert!(commands.contains("asset-fee-quote"));
    assert!(commands.contains("wallet-sign-asset-transaction"));
    assert!(commands.contains("mempool-submit-signed-asset-transaction"));
    assert!(commands.contains("PROPOSER_KEY_FILE"));
    assert!(commands.contains("ATTESTOR_KEY_FILE"));
    assert!(commands.contains("FINALIZER_KEY_FILE"));
    assert!(commands.contains("CLAIMER_KEY_FILE"));

    let error = vault_bridge_deposit_relay_bundle(VaultBridgeDepositRelayBundleOptions {
        plan_options,
        bundle_dir,
        overwrite: false,
    })
    .expect_err("refuse overwrite by default");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn vault_bridge_deposit_relay_rpc_bundle_fetches_receipt_and_writes_bundle() {
    use std::os::unix::fs::PermissionsExt;

    let root = env::temp_dir().join(format!(
        "postfiat-vault-bridge-deposit-relay-rpc-bundle-{}",
        process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let bundle_dir = root.join("bundle");
    let cli_bundle_dir = root.join("bundle-cli");
    let fake_cast = root.join("cast");

    let evidence = vault_bridge_deposit_evidence_fixture();
    let mut deposit_log = vault_bridge_vault_deposit_log_json(&evidence);
    deposit_log
        .as_object_mut()
        .expect("deposit log object")
        .remove("blockHash");
    deposit_log
        .as_object_mut()
        .expect("deposit log object")
        .remove("transactionHash");
    let receipt = serde_json::json!({
        "status": "0x1",
        "blockHash": format!("0x{}", evidence.block_hash),
        "blockNumber": "0x64",
        "transactionHash": format!("0x{}", evidence.tx_hash),
        "logs": [deposit_log],
    });
    let block = serde_json::json!({
        "hash": format!("0x{}", evidence.block_hash),
        "number": "0x64",
    });
    let fake_cast_script = format!(
            "#!/usr/bin/env bash\nset -euo pipefail\ncase \"$1\" in\n  receipt)\n    if [ \"$2\" != \"--rpc-url\" ]; then exit 3; fi\n    if [ \"$4\" != \"--json\" ]; then exit 4; fi\n    cat <<'JSON'\n{}\nJSON\n    ;;\n  block)\n    if [ \"$2\" != \"--rpc-url\" ]; then exit 5; fi\n    if [ \"$4\" != \"0x{}\" ]; then exit 6; fi\n    if [ \"$5\" != \"--json\" ]; then exit 7; fi\n    cat <<'JSON'\n{}\nJSON\n    ;;\n  block-number)\n    if [ \"$2\" != \"--rpc-url\" ]; then exit 8; fi\n    printf '0x6d\\n'\n    ;;\n  *)\n    exit 2\n    ;;\nesac\n",
            serde_json::to_string_pretty(&receipt).expect("receipt json"),
            evidence.block_hash,
            serde_json::to_string_pretty(&block).expect("block json")
        );
    std::fs::write(&fake_cast, fake_cast_script).expect("write fake cast");
    let mut permissions = std::fs::metadata(&fake_cast)
        .expect("fake cast metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&fake_cast, permissions).expect("chmod fake cast");

    let plan_options = VaultBridgeDepositPlanOptions {
        log_file: None,
        receipt_file: None,
        vault_address: Some(evidence.vault_address.clone()),
        token_address: Some(evidence.token_address.clone()),
        asset_id: "33".repeat(48),
        policy_hash: "24".repeat(32),
        proposer: "bridge-relayer".to_string(),
        finalizer: "bridge-finalizer".to_string(),
        claimer: "bridge-claimer".to_string(),
        attestor: Some("bridge-attestor".to_string()),
        observer_confirmation_depth: None,
        expires_at_height: 100,
        source_proof_kind: None,
        source_proof_hash: None,
        source_public_values_hash: None,
        source_proof_file: None,
        source_public_values_file: None,
    };
    let report = vault_bridge_deposit_relay_rpc_bundle(VaultBridgeDepositRelayRpcBundleOptions {
        source_rpc_url: "https://source-chain.example.invalid/rpc".to_string(),
        tx_hash: evidence.tx_hash.clone(),
        cast_binary: fake_cast.display().to_string(),
        plan_options: plan_options.clone(),
        bundle_dir: bundle_dir.clone(),
        overwrite: false,
    })
    .expect("write RPC relay bundle");

    assert_eq!(
        report.schema,
        postfiat_node::VAULT_BRIDGE_DEPOSIT_RELAY_RPC_BUNDLE_SCHEMA
    );
    assert_eq!(report.tx_hash, format!("0x{}", evidence.tx_hash));
    assert_eq!(
        report.receipt_transaction_hash,
        format!("0x{}", evidence.tx_hash)
    );
    assert_eq!(
        report.receipt_block_hash,
        format!("0x{}", evidence.block_hash)
    );
    assert_eq!(report.receipt_block_number, 100);
    assert_eq!(report.current_block_number, 109);
    assert_eq!(report.confirmation_depth, 10);
    assert!(bundle_dir.join("source-receipt.json").exists());
    assert!(bundle_dir.join("plan.json").exists());
    assert_eq!(report.relay_bundle.plan.evidence, evidence);
    assert_eq!(
        report.relay_bundle.plan.deposit_confirmation_depth,
        Some(10)
    );
    assert!(report
        .relay_bundle
        .plan
        .deposit_observation_root
        .as_deref()
        .is_some_and(|root| root.len() == 96));
    let attest_operation = report
        .relay_bundle
        .plan
        .attest_operation
        .as_ref()
        .expect("attest operation");
    match attest_operation {
        AssetTransactionOperation::VaultBridgeDepositAttest(operation) => {
            assert!(operation.observation.is_some());
            assert_eq!(
                Some(operation.observation_root.clone()),
                report.relay_bundle.plan.deposit_observation_root
            );
        }
        other => panic!("unexpected attest operation: {other:?}"),
    }
    assert!(report
        .relay_bundle
        .commands
        .iter()
        .any(|command| command.contains("propose.operation.json")));

    run_cli(vec![
        "vault-bridge-deposit-relay-rpc-bundle".to_string(),
        "--source-rpc-url".to_string(),
        "https://source-chain.example.invalid/rpc".to_string(),
        "--tx-hash".to_string(),
        report.tx_hash.clone(),
        "--cast-bin".to_string(),
        fake_cast.display().to_string(),
        "--vault-address".to_string(),
        report.relay_bundle.plan.evidence.vault_address.clone(),
        "--token-address".to_string(),
        report.relay_bundle.plan.evidence.token_address.clone(),
        "--asset-id".to_string(),
        "33".repeat(48),
        "--policy-hash".to_string(),
        "24".repeat(32),
        "--proposer".to_string(),
        "bridge-relayer".to_string(),
        "--attestor".to_string(),
        "bridge-attestor".to_string(),
        "--expires-at-height".to_string(),
        "100".to_string(),
        "--bundle".to_string(),
        cli_bundle_dir.display().to_string(),
    ])
    .expect("RPC relay bundle cli");
    assert!(cli_bundle_dir.join("source-receipt.json").exists());
    assert!(cli_bundle_dir.join("commands.sh").exists());

    let error = vault_bridge_deposit_relay_rpc_bundle(VaultBridgeDepositRelayRpcBundleOptions {
        source_rpc_url: "https://source-chain.example.invalid/rpc".to_string(),
        tx_hash: report.tx_hash,
        cast_binary: fake_cast.display().to_string(),
        plan_options,
        bundle_dir,
        overwrite: false,
    })
    .expect_err("refuse overwrite by default");
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn vault_bridge_product_e2e_receipt_to_swap_burn_and_withdrawal_plan() {
    let root = env::temp_dir().join(format!(
        "postfiat-vault-bridge-product-e2e-{}",
        process::id()
    ));
    let data_dir = root.join("node");
    let receipt_file = root.join("deposit-receipt.json");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init node store");

    let genesis = Genesis::new(DEFAULT_CHAIN_ID);
    let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
    let holder_key = ml_dsa_65_keygen().expect("holder keygen");
    let buyer_key = ml_dsa_65_keygen().expect("buyer keygen");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let holder = address_from_public_key(&holder_key.public_key);
    let buyer = address_from_public_key(&buyer_key.public_key);
    let deposit_amount = 7_500_000_u64;
    let offer_amount = 1_500_000_u64;

    let mut evidence = vault_bridge_deposit_evidence_fixture();
    evidence.amount_atoms = deposit_amount;
    evidence.pftl_recipient = holder.clone();
    evidence.pftl_recipient_hash =
        vault_bridge_pftl_recipient_hash(&holder).expect("recipient hash");
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
    let mut deposit_log = vault_bridge_vault_deposit_log_json(&evidence);
    deposit_log
        .as_object_mut()
        .expect("deposit log object")
        .remove("blockHash");
    deposit_log
        .as_object_mut()
        .expect("deposit log object")
        .remove("transactionHash");
    let receipt = serde_json::json!({
        "blockHash": format!("0x{}", evidence.block_hash),
        "transactionHash": format!("0x{}", evidence.tx_hash),
        "logs": [deposit_log],
    });
    std::fs::write(
        &receipt_file,
        serde_json::to_string_pretty(&receipt).expect("receipt json"),
    )
    .expect("write receipt");

    let source_domain = evidence.source_domain();
    let policy_hash = "42".repeat(48);
    let mut ledger = LedgerState::new(vec![
        Account::new(
            issuer.clone(),
            25_000_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        ),
        Account::new(
            holder.clone(),
            25_000_000,
            Some(bytes_to_hex(&holder_key.public_key)),
        ),
        Account::new(
            buyer.clone(),
            25_000_000,
            Some(bytes_to_hex(&buyer_key.public_key)),
        ),
    ]);

    let profile_register = sign_asset_e2e(
        &genesis,
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
        }),
    );
    let profile_receipt = execute_asset_transaction(&genesis, &mut ledger, &profile_register, 1);
    assert!(profile_receipt.accepted, "{profile_receipt:?}");
    let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

    let create = sign_asset_e2e(
        &genesis,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "vault bridge asset".to_string(),
            version: 7,
            precision: 8,
            display_name: "vault bridge asset".to_string(),
            max_supply: Some(100_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    let create_receipt = execute_asset_transaction(&genesis, &mut ledger, &create, 2);
    assert!(create_receipt.accepted, "{create_receipt:?}");
    let asset_id = ledger.asset_definitions[0].asset_id.clone();

    let register = sign_asset_e2e(
        &genesis,
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
    let register_receipt = execute_asset_transaction(&genesis, &mut ledger, &register, 3);
    assert!(register_receipt.accepted, "{register_receipt:?}");

    for (account, key, sequence) in [(&holder, &holder_key, 1_u64), (&buyer, &buyer_key, 1_u64)] {
        let trust = sign_asset_e2e(
            &genesis,
            key,
            TRUST_SET_TRANSACTION_KIND,
            sequence,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: account.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                limit: 100_000_000,
                authorized: false,
                frozen: false,
                reserve_paid: 10,
            }),
        );
        let trust_receipt = execute_asset_transaction(&genesis, &mut ledger, &trust, 4);
        assert!(trust_receipt.accepted, "{trust_receipt:?}");
    }

    let attestor_register = sign_asset_e2e(
        &genesis,
        &holder_key,
        NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
            attestor: holder.clone(),
            domain: "operator.local".to_string(),
            bond: 0,
        }),
    );
    let attestor_receipt = execute_asset_transaction(&genesis, &mut ledger, &attestor_register, 5);
    assert!(attestor_receipt.accepted, "{attestor_receipt:?}");

    let plan = vault_bridge_deposit_plan(VaultBridgeDepositPlanOptions {
        log_file: None,
        receipt_file: Some(receipt_file),
        vault_address: Some(evidence.vault_address.clone()),
        token_address: Some(evidence.token_address.clone()),
        asset_id: asset_id.clone(),
        policy_hash: policy_hash.clone(),
        proposer: holder.clone(),
        finalizer: holder.clone(),
        claimer: holder.clone(),
        attestor: Some(holder.clone()),
        observer_confirmation_depth: Some(6),
        expires_at_height: 1_000,
        source_proof_kind: None,
        source_proof_hash: None,
        source_public_values_hash: None,
        source_proof_file: None,
        source_public_values_file: None,
    })
    .expect("plan bridge deposit from source-chain receipt");
    assert_eq!(plan.evidence, evidence);

    let planned_operations = [
        (
            VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
            3_u64,
            plan.propose_operation.clone(),
            6_u64,
        ),
        (
            VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
            4_u64,
            plan.attest_operation
                .clone()
                .expect("planned attest operation"),
            7_u64,
        ),
        (
            VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
            5_u64,
            plan.finalize_operation.clone(),
            8_u64,
        ),
        (
            VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
            6_u64,
            plan.claim_operation.clone(),
            9_u64,
        ),
    ];
    for (transaction_kind, sequence, operation, height) in planned_operations {
        let tx = sign_asset_e2e(&genesis, &holder_key, transaction_kind, sequence, operation);
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &tx, height);
        assert!(receipt.accepted, "{transaction_kind} failed: {receipt:?}");
    }

    assert_eq!(
        ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .expect("holder line")
            .balance,
        deposit_amount
    );
    assert_eq!(ledger.vault_bridge_receipts.len(), 1);
    assert_eq!(ledger.vault_bridge_receipts[0].status, "counted");
    assert_eq!(ledger.vault_bridge_receipts[0].amount_atoms, deposit_amount);
    assert_eq!(
        ledger.vault_bridge_receipts[0].counted_value_atoms,
        deposit_amount
    );

    let source_root =
        vault_bridge_source_root_for_asset(&ledger.vault_bridge_bucket_states, &asset_id)
            .expect("source root");
    let reserve_packet_hash = "ac".repeat(48);
    let reserve_submit = sign_asset_e2e(
        &genesis,
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
            reserve_accounts: vec![evidence.vault_id()],
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        }),
    );
    let reserve_submit_receipt =
        execute_asset_transaction(&genesis, &mut ledger, &reserve_submit, 10);
    assert!(
        reserve_submit_receipt.accepted,
        "{reserve_submit_receipt:?}"
    );

    let reserve_attest = sign_asset_e2e(
        &genesis,
        &holder_key,
        NAV_RESERVE_ATTEST_TRANSACTION_KIND,
        7,
        AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
            attestor: holder.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
            pass: true,
            observation_root: source_root,
        }),
    );
    let reserve_attest_receipt =
        execute_asset_transaction(&genesis, &mut ledger, &reserve_attest, 11);
    assert!(
        reserve_attest_receipt.accepted,
        "{reserve_attest_receipt:?}"
    );

    let finalize_epoch = sign_asset_e2e(
        &genesis,
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
    let finalize_epoch_receipt =
        execute_asset_transaction(&genesis, &mut ledger, &finalize_epoch, 12);
    assert!(
        finalize_epoch_receipt.accepted,
        "{finalize_epoch_receipt:?}"
    );

    let holder_offer = sign_offer_e2e(
        &genesis,
        &holder_key,
        OFFER_CREATE_TRANSACTION_KIND,
        8,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: holder.clone(),
            taker_gets_asset_id: asset_id.clone(),
            taker_gets_amount: offer_amount,
            taker_pays_asset_id: "PFT".to_string(),
            taker_pays_amount: 3_000_000,
            expiration_height: 50,
        }),
    );
    let offer_receipt = execute_offer_transaction(&genesis, &mut ledger, &holder_offer, 13);
    assert!(offer_receipt.accepted, "{offer_receipt:?}");
    let maker_offer_id = offer_id(&genesis.chain_id, &holder, 8).expect("offer id");

    let buyer_fill = sign_offer_e2e(
        &genesis,
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
    );
    let fill_receipt = execute_offer_transaction(&genesis, &mut ledger, &buyer_fill, 14);
    assert!(fill_receipt.accepted, "{fill_receipt:?}");
    assert_eq!(fill_receipt.code, "filled");
    assert_eq!(
        ledger.offer(&maker_offer_id).expect("filled offer").state,
        OFFER_STATE_FILLED
    );
    assert_eq!(
        ledger
            .trustline_for_account_asset(&buyer, &asset_id)
            .expect("buyer line")
            .balance,
        offer_amount
    );

    postfiat_storage::NodeStore::new(&data_dir)
        .write_ledger(&ledger)
        .expect("write e2e ledger before burn bundle");
    let destination_ref = "evm-erc20:42161:0x4444444444444444444444444444444444444444".to_string();
    let burn_bundle_dir = root.join("burn-to-redeem-bundle");
    let burn_bundle = vault_bridge_burn_to_redeem_bundle(VaultBridgeBurnToRedeemBundleOptions {
        data_dir: data_dir.clone(),
        owner: buyer.clone(),
        issuer: None,
        asset_id: asset_id.clone(),
        bucket_id: None,
        amount_atoms: offer_amount,
        epoch: None,
        reserve_packet_hash: None,
        destination_ref: destination_ref.clone(),
        bundle_dir: burn_bundle_dir.clone(),
        overwrite: false,
    })
    .expect("build burn-to-redeem bundle");
    assert_eq!(
        postfiat_node::VAULT_BRIDGE_BURN_TO_REDEEM_BUNDLE_SCHEMA,
        burn_bundle.schema
    );
    assert!(burn_bundle_dir
        .join("burn-to-redeem.operation.json")
        .exists());
    assert!(burn_bundle_dir.join("commands.sh").exists());
    assert_eq!(burn_bundle.owner, buyer);
    assert_eq!(burn_bundle.asset_id, asset_id);
    assert_eq!(burn_bundle.amount_atoms, offer_amount);
    assert_eq!(burn_bundle.destination_ref, destination_ref);
    assert!(burn_bundle
        .commands
        .iter()
        .any(|command| command.contains("wallet-sign-asset-transaction")));
    let burn_script =
        std::fs::read_to_string(burn_bundle_dir.join("commands.sh")).expect("burn script");
    assert!(burn_script.contains("OWNER_KEY_FILE"));

    let burn_cli_bundle_dir = root.join("burn-to-redeem-bundle-cli");
    run_cli(vec![
        "vault-bridge-burn-to-redeem-bundle".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--owner".to_string(),
        buyer.clone(),
        "--asset-id".to_string(),
        asset_id.clone(),
        "--amount-atoms".to_string(),
        offer_amount.to_string(),
        "--destination-ref".to_string(),
        "evm-erc20:42161:0x5555555555555555555555555555555555555555".to_string(),
        "--bundle".to_string(),
        burn_cli_bundle_dir.display().to_string(),
    ])
    .expect("burn-to-redeem bundle cli");
    assert!(burn_cli_bundle_dir
        .join("burn-to-redeem.operation.json")
        .exists());
    assert!(burn_cli_bundle_dir.join("commands.sh").exists());

    let buyer_burn = sign_asset_e2e(
        &genesis,
        &buyer_key,
        VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
        3,
        burn_bundle.operation.clone(),
    );
    let burn_receipt = execute_asset_transaction(&genesis, &mut ledger, &buyer_burn, 15);
    assert!(burn_receipt.accepted, "{burn_receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&buyer, &asset_id)
            .expect("buyer line")
            .balance,
        0
    );
    assert_eq!(ledger.vault_bridge_redemptions.len(), 1);
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
    ledger
        .validate_asset_state(&genesis.chain_id)
        .expect("valid end-to-end vault bridge state");

    let redemption_id = ledger.vault_bridge_redemptions[0].redemption_id.clone();
    postfiat_storage::NodeStore::new(&data_dir)
        .write_ledger(&ledger)
        .expect("write e2e ledger");
    let withdrawal_plan = vault_bridge_withdrawal_plan(VaultBridgeWithdrawalPlanOptions {
        data_dir: data_dir.clone(),
        asset_id: asset_id.clone(),
        redemption_id: redemption_id.clone(),
        pftl_finalized_height: None,
        evm_chain_id: Some(42_161),
        verifier_address: Some("0x3333333333333333333333333333333333333333".to_string()),
        signatures_file: None,
    })
    .expect("plan source-chain withdrawal");
    assert_eq!(withdrawal_plan.withdrawal_packet.amount_atoms, offer_amount);
    assert_eq!(
        withdrawal_plan.withdrawal_packet.recipient,
        "0x4444444444444444444444444444444444444444"
    );
    assert!(withdrawal_plan
        .verifier_submit_proof_cast_command
        .contains("submitProof(bytes32,bytes32,uint64,bytes[])"));
    assert!(withdrawal_plan
            .vault_submit_withdrawal_cast_command
            .contains("submitWithdrawal((uint64,uint256,address,address,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes),bytes)"));
    let withdrawal_bundle_dir = root.join("withdrawal-relay-bundle");
    let withdrawal_bundle =
        vault_bridge_withdrawal_relay_bundle(VaultBridgeWithdrawalRelayBundleOptions {
            plan_options: VaultBridgeWithdrawalPlanOptions {
                data_dir: data_dir.clone(),
                asset_id,
                redemption_id,
                pftl_finalized_height: None,
                evm_chain_id: Some(42_161),
                verifier_address: Some("0x3333333333333333333333333333333333333333".to_string()),
                signatures_file: None,
            },
            bundle_dir: withdrawal_bundle_dir.clone(),
            overwrite: false,
        })
        .expect("build source-chain withdrawal relay bundle");
    assert_eq!(
        withdrawal_bundle.schema,
        postfiat_node::VAULT_BRIDGE_WITHDRAWAL_RELAY_BUNDLE_SCHEMA
    );
    assert!(withdrawal_bundle.plan_file.ends_with("plan.json"));
    assert!(PathBuf::from(&withdrawal_bundle.plan_file).exists());
    assert!(PathBuf::from(&withdrawal_bundle.commands_file).exists());
    assert!(withdrawal_bundle
        .verifier_submit_proof_command
        .contains("submitProof(bytes32,bytes32,uint64,bytes[])"));
    assert!(withdrawal_bundle
        .verifier_finalize_proof_command
        .contains("finalizeProof(bytes32)"));
    assert!(withdrawal_bundle
            .vault_submit_withdrawal_command
            .contains("submitWithdrawal((uint64,uint256,address,address,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes),bytes)"));
    assert!(withdrawal_bundle
        .vault_finalize_withdrawal_command
        .contains("finalizeWithdrawal(bytes32)"));
    assert!(withdrawal_bundle
        .vault_claim_withdrawal_command
        .contains("claimWithdrawal(bytes32)"));
    let commands_script =
        std::fs::read_to_string(&withdrawal_bundle.commands_file).expect("read commands");
    assert!(commands_script.contains("RUN_STAGE=${RUN_STAGE:-help}"));
    assert!(commands_script.contains("submit-proof"));
    assert!(commands_script.contains("finalize-proof"));
    assert!(commands_script.contains("submit-withdrawal"));
    assert!(commands_script.contains("finalize-withdrawal"));
    assert!(commands_script.contains("claim"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn vault_bridge_deposit_plan_rejects_tampered_vault_log_amount() {
    let root = env::temp_dir().join(format!(
        "postfiat-vault-bridge-deposit-plan-tamper-{}",
        process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let log_file = root.join("deposit-log.json");
    let mut evidence = vault_bridge_deposit_evidence_fixture();
    let original_deposit_id = evidence.deposit_id.clone();
    evidence.amount_atoms += 1;
    evidence.deposit_id = original_deposit_id;
    let log = vault_bridge_vault_deposit_log_json(&evidence);
    std::fs::write(
        &log_file,
        serde_json::to_string_pretty(&log).expect("log json"),
    )
    .expect("write log");

    let error = vault_bridge_deposit_plan(VaultBridgeDepositPlanOptions {
        log_file: Some(log_file),
        receipt_file: None,
        vault_address: None,
        token_address: None,
        asset_id: "33".repeat(48),
        policy_hash: "24".repeat(32),
        proposer: "bridge-relayer".to_string(),
        finalizer: "bridge-finalizer".to_string(),
        claimer: "bridge-claimer".to_string(),
        attestor: None,
        observer_confirmation_depth: None,
        expires_at_height: 100,
        source_proof_kind: None,
        source_proof_hash: None,
        source_public_values_hash: None,
        source_proof_file: None,
        source_public_values_file: None,
    })
    .expect_err("tampered log rejected");
    assert!(
        error.to_string().contains("deposit_id"),
        "unexpected error: {error}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn vault_bridge_status_cli_reports_source_backed_receipt_capacity() {
    use postfiat_types::{
        vault_bridge_source_root_for_asset, Account, AssetDefinition, NavReservePacket,
        NavTrackedAsset, TrustLine, VaultBridgeAllocation, VaultBridgeBucketState,
        VaultBridgeReceipt, NAV_RESERVE_STATE_FINALIZED, VAULT_BRIDGE_REDEMPTION_STATE_PENDING,
    };

    let root = env::temp_dir().join(format!("postfiat-vault-bridge-status-{}", process::id()));
    let data_dir = root.join("node");
    let bundle_dir = root.join("vault_bridge-bundle");
    let _ = std::fs::remove_dir_all(&root);

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init node store");

    let issuer = "vault_bridge-issuer";
    let holder = "vault_bridge-holder";
    let pftl_recipient = holder.to_string();
    let pftl_recipient_hash =
        vault_bridge_pftl_recipient_hash(&pftl_recipient).expect("recipient hash");
    let mut evidence = VaultBridgeDepositEvidence {
        source_chain_id: 42_161,
        vault_address: "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0".to_string(),
        token_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        depositor: "0x1111111111111111111111111111111111111111".to_string(),
        pftl_recipient,
        pftl_recipient_hash,
        amount_atoms: 10_000_099,
        nonce: "22".repeat(32),
        route_binding: String::new(),
        deposit_id: String::new(),
        block_hash: "44".repeat(32),
        tx_hash: "55".repeat(32),
        log_index: 7,
    };
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
    let source_domain = evidence.source_domain();
    let policy_hash = "42".repeat(48);
    let asset = AssetDefinition::new(DEFAULT_CHAIN_ID, issuer, "vault bridge asset", 1, 8)
        .expect("vault bridge asset asset definition");
    let asset_id = asset.asset_id.clone();
    let mut nav_asset = NavTrackedAsset::new(
        asset_id.clone(),
        issuer,
        issuer,
        "vault_bridge-profile",
        "SOURCE_UNIT",
        issuer,
    )
    .expect("vault bridge asset nav asset");
    nav_asset.finalized_epoch = 1;
    nav_asset.nav_per_unit = 1_000_000;
    nav_asset.circulating_supply = 4_000_000;
    nav_asset.finalized_reserve_packet_hash = "ab".repeat(48);
    nav_asset.finalized_at_height = 13;

    let mut receipt = VaultBridgeReceipt::new(
        DEFAULT_CHAIN_ID,
        asset_id.clone(),
        source_domain.clone(),
        evidence.source_asset_ref(),
        VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT,
        10_000_099,
        evidence.source_tx_or_attestation(),
        evidence.finality_ref(),
        evidence.vault_id(),
        policy_hash.clone(),
        10,
        1_000,
        Some(evidence.clone()),
    )
    .expect("vault bridge asset receipt");
    let bridge_evidence_root =
        vault_bridge_deposit_evidence_root(&evidence).expect("bridge evidence root");
    assert_eq!(bridge_evidence_root.len(), 96);
    receipt.haircut_bps = 25;
    receipt.counted_value_atoms = 9_975_098;
    receipt.allocated_value_atoms = 5_000_000;
    receipt.status = "counted".to_string();
    receipt.counted_at_height = 12;

    let mut bridge_record = VaultBridgeDepositRecord::new(
        asset_id.clone(),
        bridge_evidence_root,
        evidence.clone(),
        policy_hash.clone(),
        String::new(),
        String::new(),
        String::new(),
        holder.to_string(),
        10,
        1_000,
    )
    .expect("vault bridge asset bridge deposit record");
    bridge_record
        .attestations
        .push(VaultBridgeDepositAttestation {
            attestor: holder.to_string(),
            pass: true,
            observation_root: bridge_record.evidence_root.clone(),
            attested_at_height: 11,
            observation: None,
        });
    bridge_record.status = VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED.to_string();
    bridge_record.finalized_at_height = 12;
    bridge_record
        .validate()
        .expect("finalized vault bridge asset bridge deposit record");

    let mut bucket = VaultBridgeBucketState::new(asset_id.clone(), source_domain, policy_hash, 12)
        .expect("vault bridge asset bucket");
    bucket.gross_receipt_atoms = 10_000_099;
    bucket.counted_value_atoms = 9_975_098;
    bucket.outstanding_vault_bridge_atoms = 4_000_000;
    bucket.redemption_queue_atoms = 1_000_000;
    let bucket_id = bucket.bucket_id.clone();
    let allocation = VaultBridgeAllocation::new(
        DEFAULT_CHAIN_ID,
        receipt.receipt_id.clone(),
        asset_id.clone(),
        bucket_id.clone(),
        5_000_000,
        "vault_bridge_supply",
        "vault_bridge_supply:8:0",
        13,
    )
    .expect("vault bridge asset allocation");
    let redemption = VaultBridgeRedemption::new(
        DEFAULT_CHAIN_ID,
        holder,
        issuer,
        asset_id.clone(),
        bucket_id.clone(),
        evidence.source_domain(),
        9,
        1_000_000,
        1,
        nav_asset.finalized_reserve_packet_hash.clone(),
        "evm-erc20:42161:0x2222222222222222222222222222222222222222",
        "99".repeat(48),
        14,
    )
    .expect("vault bridge asset redemption");
    let source_root = vault_bridge_source_root_for_asset(&[bucket.clone()], &asset_id)
        .expect("vault bridge asset source root");
    let mut reserve_packet = NavReservePacket::new(
        asset_id.clone(),
        issuer,
        issuer,
        1,
        1_000_000,
        4_000_000,
        9_975_098,
        "vault_bridge-profile",
        source_root,
        "88".repeat(48),
        nav_asset.finalized_reserve_packet_hash.clone(),
    )
    .expect("vault bridge asset reserve packet");
    reserve_packet.state = NAV_RESERVE_STATE_FINALIZED.to_string();
    reserve_packet.submitted_at_height = 13;
    reserve_packet.reserve_accounts = vec![evidence.vault_id()];

    let mut line =
        TrustLine::new(holder, issuer, asset_id.clone(), 100_000_000, 10).expect("trustline");
    line.balance = 4_000_000;

    let mut ledger = LedgerState::new(vec![
        Account::new(issuer, 0, None),
        Account::new(holder, 0, None),
    ]);
    ledger.asset_definitions.push(asset);
    ledger.trustlines.push(line);
    ledger.nav_assets.push(nav_asset);
    ledger.nav_reserve_packets.push(reserve_packet);
    ledger.vault_bridge_deposits.push(bridge_record);
    ledger.vault_bridge_receipts.push(receipt);
    ledger.vault_bridge_bucket_states.push(bucket);
    ledger.vault_bridge_allocations.push(allocation);
    ledger.vault_bridge_redemptions.push(redemption);
    postfiat_storage::NodeStore::new(&data_dir)
        .write_ledger(&ledger)
        .expect("write vault bridge asset ledger");

    run_cli(vec![
        "vault-bridge-status".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--asset-id".to_string(),
        asset_id.clone(),
    ])
    .expect("vault bridge asset status cli");

    let report = vault_bridge_status(VaultBridgeStatusOptions {
        data_dir: data_dir.clone(),
        asset_id: asset_id.clone(),
    })
    .expect("vault bridge asset status report");
    assert_eq!(
        postfiat_node::VAULT_BRIDGE_STATUS_REPORT_SCHEMA,
        report.schema
    );
    assert_eq!(4_000_000, report.issued_supply_atoms);
    assert_eq!(9_975_098, report.counted_value_atoms);
    assert_eq!(4_975_098, report.unallocated_counted_capacity_atoms);
    assert_eq!(1, report.bucket_count);
    assert_eq!(1, report.receipt_count);
    assert_eq!(1, report.allocation_count);
    assert_eq!(1, report.redemption_count);
    assert_eq!(5_000_000, report.allocations[0].amount_atoms);
    assert_eq!(0, report.allocations[0].released_atoms);
    assert_eq!(5_000_000, report.allocations[0].remaining_atoms);
    assert_eq!(64, report.redemptions[0].withdrawal_packet_evm_digest.len());
    assert_eq!(
        "0x2222222222222222222222222222222222222222",
        report.redemptions[0].withdrawal_recipient
    );
    assert_eq!(
        4_975_098,
        report.buckets[0].unallocated_counted_capacity_atoms
    );
    assert_eq!(4_975_098, report.receipts[0].unallocated_value_atoms);
    let redemption_id = report.redemptions[0].redemption_id.clone();

    run_cli(vec![
        "vault-bridge-receipts".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--asset-id".to_string(),
        asset_id.clone(),
        "--bucket-id".to_string(),
        bucket_id,
    ])
    .expect("vault bridge asset receipts cli");

    run_cli(vec![
        "vault-bridge-withdrawal-plan".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--asset-id".to_string(),
        asset_id.clone(),
        "--redemption-id".to_string(),
        redemption_id.clone(),
        "--evm-chain-id".to_string(),
        "42161".to_string(),
        "--verifier-address".to_string(),
        "0x3333333333333333333333333333333333333333".to_string(),
    ])
    .expect("vault bridge asset withdrawal plan cli");
    let withdrawal_bundle_dir = root.join("withdrawal-relay-bundle-cli");
    run_cli(vec![
        "vault-bridge-withdrawal-relay-bundle".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--asset-id".to_string(),
        asset_id.clone(),
        "--redemption-id".to_string(),
        redemption_id.clone(),
        "--evm-chain-id".to_string(),
        "42161".to_string(),
        "--verifier-address".to_string(),
        "0x3333333333333333333333333333333333333333".to_string(),
        "--bundle".to_string(),
        withdrawal_bundle_dir.display().to_string(),
    ])
    .expect("vault bridge asset withdrawal relay bundle cli");
    assert!(withdrawal_bundle_dir.join("plan.json").exists());
    assert!(withdrawal_bundle_dir.join("commands.sh").exists());

    let withdrawal_plan = vault_bridge_withdrawal_plan(VaultBridgeWithdrawalPlanOptions {
        data_dir: data_dir.clone(),
        asset_id: asset_id.clone(),
        redemption_id: redemption_id.clone(),
        pftl_finalized_height: None,
        evm_chain_id: Some(42_161),
        verifier_address: Some("0x3333333333333333333333333333333333333333".to_string()),
        signatures_file: None,
    })
    .expect("vault bridge asset withdrawal plan");
    assert_eq!(
        postfiat_node::VAULT_BRIDGE_WITHDRAWAL_PLAN_SCHEMA,
        withdrawal_plan.schema
    );
    assert_eq!(
        VAULT_BRIDGE_REDEMPTION_STATE_PENDING,
        withdrawal_plan.redemption_state
    );
    assert_eq!(
        report.redemptions[0].withdrawal_packet_evm_digest,
        withdrawal_plan
            .withdrawal_packet_evm_digest
            .trim_start_matches("0x")
    );
    assert_eq!(
        report.redemptions[0].withdrawal_packet_hash,
        withdrawal_plan
            .pftl_withdrawal_hash
            .trim_start_matches("0x")
    );
    assert_eq!(66, withdrawal_plan.pftl_withdrawal_hash_commitment.len());
    assert_eq!(66, withdrawal_plan.verifier_pending_proof_id.len());
    assert_eq!(66, withdrawal_plan.verifier_withdrawal_key.len());
    assert_eq!(66, withdrawal_plan.vault_pending_withdrawal_id.len());
    assert_eq!(
        66,
        withdrawal_plan
            .verifier_proof_digest_to_sign
            .as_ref()
            .expect("proof digest")
            .len()
    );
    assert!(withdrawal_plan
        .withdrawal_packet_tuple_arg
        .contains("0x2222222222222222222222222222222222222222"));
    assert_eq!("[]", withdrawal_plan.verifier_submit_proof_cast_args[3]);
    assert!(withdrawal_plan
        .verifier_submit_proof_cast_command
        .contains("submitProof(bytes32,bytes32,uint64,bytes[])"));
    assert!(
            withdrawal_plan
                .vault_submit_withdrawal_cast_command
                .contains("submitWithdrawal((uint64,uint256,address,address,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes),bytes)")
        );

    let signature_bundle_dir = root.join("withdrawal-signature-bundle");
    let signature_bundle =
        vault_bridge_withdrawal_signature_bundle(VaultBridgeWithdrawalSignatureBundleOptions {
            plan_options: VaultBridgeWithdrawalPlanOptions {
                data_dir: data_dir.clone(),
                asset_id: asset_id.clone(),
                redemption_id: redemption_id.clone(),
                pftl_finalized_height: None,
                evm_chain_id: Some(42_161),
                verifier_address: Some("0x3333333333333333333333333333333333333333".to_string()),
                signatures_file: None,
            },
            bundle_dir: signature_bundle_dir.clone(),
            relay_bundle_dir: None,
            overwrite: false,
        })
        .expect("vault bridge withdrawal signature bundle");
    assert_eq!(
        postfiat_node::VAULT_BRIDGE_WITHDRAWAL_SIGNATURE_BUNDLE_SCHEMA,
        signature_bundle.schema
    );
    assert!(signature_bundle_dir.join("plan.json").exists());
    assert!(signature_bundle_dir.join("signature-request.json").exists());
    assert!(signature_bundle_dir.join("signatures.json").exists());
    assert!(signature_bundle_dir.join("commands.sh").exists());
    assert_eq!(
        withdrawal_plan
            .verifier_proof_digest_to_sign
            .as_ref()
            .expect("proof digest"),
        &signature_bundle
            .signature_request
            .verifier_proof_digest_to_sign
    );
    assert!(signature_bundle
        .sign_command
        .contains("cast wallet sign --no-hash"));
    assert!(signature_bundle
        .relay_bundle_command
        .contains("vault-bridge-withdrawal-relay-bundle"));
    assert!(signature_bundle
        .relay_bundle_command
        .contains("--signatures-file"));
    let empty_signatures: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(signature_bundle_dir.join("signatures.json"))
            .expect("read signatures file"),
    )
    .expect("parse signatures file");
    assert!(empty_signatures.is_empty());
    let signature_commands = std::fs::read_to_string(signature_bundle_dir.join("commands.sh"))
        .expect("read signature commands");
    assert!(signature_commands.contains("RUN_STAGE=sign"));
    assert!(signature_commands.contains("RUN_STAGE=relay-bundle"));

    let signature_cli_bundle_dir = root.join("withdrawal-signature-bundle-cli");
    run_cli(vec![
        "vault-bridge-withdrawal-signature-bundle".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--asset-id".to_string(),
        asset_id.clone(),
        "--redemption-id".to_string(),
        redemption_id.clone(),
        "--evm-chain-id".to_string(),
        "42161".to_string(),
        "--verifier-address".to_string(),
        "0x3333333333333333333333333333333333333333".to_string(),
        "--bundle".to_string(),
        signature_cli_bundle_dir.display().to_string(),
    ])
    .expect("vault bridge withdrawal signature bundle cli");
    assert!(signature_cli_bundle_dir
        .join("signature-request.json")
        .exists());
    assert!(signature_cli_bundle_dir.join("commands.sh").exists());

    let receipts_report = vault_bridge_receipts(VaultBridgeReceiptsOptions {
        data_dir: data_dir.clone(),
        asset_id: asset_id.clone(),
        bucket_id: Some(report.buckets[0].bucket_id.clone()),
    })
    .expect("vault bridge asset receipts report");
    assert_eq!(
        postfiat_node::VAULT_BRIDGE_RECEIPTS_REPORT_SCHEMA,
        receipts_report.schema
    );
    assert_eq!(1, receipts_report.receipt_count);

    let store = NodeStore::new(&data_dir);
    let mut custody_ledger = store.read_ledger().expect("read custody ledger");
    let custody_line = custody_ledger
        .trustlines
        .iter_mut()
        .find(|line| line.asset_id == asset_id)
        .expect("issued custody trustline");
    custody_line.balance = custody_line
        .balance
        .checked_sub(3)
        .expect("move issued atoms out of public custody");
    custody_ledger
        .fast_lane_reserves
        .push(postfiat_types::FastLaneReserveBalanceV1 {
            asset_id: postfiat_types::FastAssetIdV1(
                postfiat_crypto_provider::hex_to_bytes(&asset_id)
                    .expect("issued asset hex")
                    .try_into()
                    .expect("issued FastLane asset width"),
            ),
            amount_atoms: 1,
        });
    store
        .write_ledger(&custody_ledger)
        .expect("write mixed issued custody ledger");
    let mut custody_shielded = store.read_shielded().expect("read shielded custody");
    let pool = custody_shielded.orchard.get_or_insert_with(|| {
        postfiat_types::OrchardPoolState::empty("vault-replay-mixed-custody")
    });
    pool.asset_orchard_balances
        .push(postfiat_types::AssetOrchardAssetBalance {
            asset_id: asset_id.clone(),
            ingress_total: 2,
            egress_total: 0,
            live_total: 2,
        });
    store
        .write_shielded(&custody_shielded)
        .expect("write mixed issued Orchard custody");
    assert_eq!(
        vault_bridge_status(VaultBridgeStatusOptions {
            data_dir: data_dir.clone(),
            asset_id: asset_id.clone(),
        })
        .expect("mixed-custody vault bridge status")
        .issued_supply_atoms,
        4_000_000
    );

    run_cli(vec![
        "vault-bridge-export-reserve-packet".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--asset-id".to_string(),
        asset_id.clone(),
        "--epoch".to_string(),
        "1".to_string(),
        "--bundle".to_string(),
        bundle_dir.display().to_string(),
        "--overwrite".to_string(),
    ])
    .expect("vault bridge asset reserve replay bundle export");

    let bundle_file = bundle_dir.join("bundle.json");
    let bundle: postfiat_node::VaultBridgeReserveReplayBundle =
        serde_json::from_str(&std::fs::read_to_string(&bundle_file).expect("read bundle"))
            .expect("parse vault bridge asset bundle");
    assert_eq!(
        postfiat_node::VAULT_BRIDGE_RESERVE_REPLAY_BUNDLE_SCHEMA,
        bundle.schema
    );
    assert_eq!(9_975_098, bundle.expected_counted_value_atoms);
    assert_eq!(4_000_000, bundle.expected_issued_supply_atoms);
    assert_eq!(1, bundle.fast_lane_reserves.len());
    assert_eq!(1, bundle.fast_lane_reserves[0].amount_atoms);
    assert_eq!(1, bundle.asset_orchard_balances.len());
    assert_eq!(2, bundle.asset_orchard_balances[0].live_total);

    run_cli(vec![
        "vault-bridge-replay-reserve-packet".to_string(),
        "--bundle".to_string(),
        bundle_dir.display().to_string(),
    ])
    .expect("vault bridge asset reserve replay bundle verify");

    let replay_report =
        replay_vault_bridge_reserve_bundle(VaultBridgeReserveReplayBundleVerifyOptions {
            bundle_dir: bundle_dir.clone(),
        })
        .expect("vault bridge asset replay report");
    assert!(replay_report.verified);
    assert_eq!(
        bundle.expected_reserve_packet_hash,
        replay_report.expected_reserve_packet_hash
    );
    assert_eq!(9_975_098, replay_report.counted_value_atoms);
    assert_eq!(4_000_000, replay_report.issued_supply_atoms);

    let mut fast_lane_tamper = bundle.clone();
    fast_lane_tamper.fast_lane_reserves[0].amount_atoms = fast_lane_tamper.fast_lane_reserves[0]
        .amount_atoms
        .checked_add(1)
        .expect("tampered FastLane reserve remains in range");
    std::fs::write(
        &bundle_file,
        serde_json::to_vec_pretty(&fast_lane_tamper)
            .expect("serialize tampered FastLane replay bundle"),
    )
    .expect("write tampered FastLane replay bundle");
    let error = replay_vault_bridge_reserve_bundle(VaultBridgeReserveReplayBundleVerifyOptions {
        bundle_dir: bundle_dir.clone(),
    })
    .expect_err("tampered FastLane issued custody must fail replay");
    assert!(
        error
            .to_string()
            .contains("issued supply does not match exported expectation"),
        "unexpected FastLane tamper error: {error}"
    );

    let mut orchard_tamper = bundle.clone();
    orchard_tamper.asset_orchard_balances[0].ingress_total = orchard_tamper.asset_orchard_balances
        [0]
    .ingress_total
    .checked_add(1)
    .expect("tampered Orchard ingress remains in range");
    orchard_tamper.asset_orchard_balances[0].live_total = orchard_tamper.asset_orchard_balances[0]
        .live_total
        .checked_add(1)
        .expect("tampered Orchard custody remains in range");
    std::fs::write(
        &bundle_file,
        serde_json::to_vec_pretty(&orchard_tamper)
            .expect("serialize tampered Orchard replay bundle"),
    )
    .expect("write tampered Orchard replay bundle");
    let error = replay_vault_bridge_reserve_bundle(VaultBridgeReserveReplayBundleVerifyOptions {
        bundle_dir: bundle_dir.clone(),
    })
    .expect_err("tampered Orchard issued custody must fail replay");
    assert!(
        error
            .to_string()
            .contains("issued supply does not match exported expectation"),
        "unexpected Orchard tamper error: {error}"
    );

    std::fs::write(
        &bundle_file,
        serde_json::to_vec_pretty(&bundle).expect("serialize restored replay bundle"),
    )
    .expect("restore valid replay bundle");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn wallet_sign_asset_transaction_signs_asset_fee_quote() {
    let root = env::temp_dir().join(format!("postfiat-wallet-sign-asset-{}", process::id()));
    let key_file = root.join("operator-key.json");
    let backup_file = root.join("operator-backup.json");
    let quote_file = root.join("asset-quote.json");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");

    let key_report = wallet_keygen(WalletKeygenOptions {
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        master_seed_hex: "11".repeat(32),
        account_index: 0,
        key_file: key_file.clone(),
        backup_file: backup_file.clone(),
        overwrite: true,
    })
    .expect("wallet keygen");
    let genesis = postfiat_types::Genesis::new(DEFAULT_CHAIN_ID);
    let operation = postfiat_types::AssetTransactionOperation::AssetCreate(
        postfiat_types::AssetCreateOperation {
            issuer: key_report.address.clone(),
            code: "vault bridge asset".to_string(),
            version: 1,
            precision: 6,
            display_name: "vault bridge asset".to_string(),
            max_supply: Some(100_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        },
    );
    let quote = AssetFeeQuoteReport {
        schema: "postfiat-asset-fee-quote-v1".to_string(),
        transaction_kind: operation.transaction_kind().to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: postfiat_execution::genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        source: key_report.address.clone(),
        sequence: 1,
        sequence_source: "provided".to_string(),
        sender_balance: 10_000,
        sender_sequence: 0,
        mempool_pending_for_sender: 0,
        base_asset_fee: 1,
        state_expansion_fee: 0,
        minimum_fee: 1,
        account_reserve: postfiat_execution::ACCOUNT_RESERVE,
        transfer_fee_byte_quantum: postfiat_execution::TRANSFER_FEE_BYTE_QUANTUM as u64,
        transfer_fee_per_quantum: postfiat_execution::TRANSFER_FEE_PER_QUANTUM,
        asset_weight_bytes: 1,
        sender_balance_after_fee: Some(9_999),
        sender_meets_reserve_after_fee: true,
        operation: operation.clone(),
    };
    std::fs::write(
        &quote_file,
        serde_json::to_string_pretty(&quote).expect("serialize quote"),
    )
    .expect("write quote");

    let parsed_quote =
        read_wallet_sign_asset_transaction_quote_file(&quote_file).expect("read quote");
    let signed = wallet_sign_asset_transaction(WalletSignAssetTransactionOptions {
        key_file: key_file.clone(),
        chain_id: parsed_quote.chain_id,
        genesis_hash: parsed_quote.genesis_hash,
        protocol_version: parsed_quote.protocol_version,
        fee: parsed_quote.minimum_fee,
        sequence: parsed_quote.sequence,
        expected_source: Some(parsed_quote.source),
        operation: parsed_quote.operation,
    })
    .expect("sign asset transaction");
    assert_eq!(signed.unsigned.source, key_report.address);
    assert_eq!(signed.unsigned.operation, operation);
    assert_eq!(signed.unsigned.fee, 1);

    run_cli(vec![
        "wallet-sign-asset-transaction".to_string(),
        "--key-file".to_string(),
        key_file.display().to_string(),
        "--quote-file".to_string(),
        quote_file.display().to_string(),
    ])
    .expect("wallet sign asset transaction cli");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn wallet_sign_escrow_transaction_signs_escrow_fee_quote() {
    let root = env::temp_dir().join(format!("postfiat-wallet-sign-escrow-{}", process::id()));
    let key_file = root.join("operator-key.json");
    let backup_file = root.join("operator-backup.json");
    let quote_file = root.join("escrow-quote.json");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");

    let key_report = wallet_keygen(WalletKeygenOptions {
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        master_seed_hex: "12".repeat(32),
        account_index: 0,
        key_file: key_file.clone(),
        backup_file: backup_file.clone(),
        overwrite: true,
    })
    .expect("wallet keygen");
    let genesis = postfiat_types::Genesis::new(DEFAULT_CHAIN_ID);
    let recipient = "pfrecipient0000000000000000000000000000000000".to_string();
    let operation = postfiat_types::EscrowTransactionOperation::EscrowCreate(
        postfiat_types::EscrowCreateOperation {
            owner: key_report.address.clone(),
            recipient,
            asset_id: "PFT".to_string(),
            amount: 25,
            condition: "shared-secret".to_string(),
            finish_after: 0,
            cancel_after: 5,
        },
    );
    let quote = EscrowFeeQuoteReport {
        schema: "postfiat-escrow-fee-quote-v1".to_string(),
        transaction_kind: operation.transaction_kind().to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: postfiat_execution::genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        source: key_report.address.clone(),
        sequence: 1,
        sequence_source: "provided".to_string(),
        sender_balance: 10_000,
        sender_sequence: 0,
        mempool_pending_for_sender: 0,
        base_escrow_fee: 1,
        state_expansion_fee: 0,
        minimum_fee: 1,
        account_reserve: postfiat_execution::ACCOUNT_RESERVE,
        transfer_fee_byte_quantum: postfiat_execution::TRANSFER_FEE_BYTE_QUANTUM as u64,
        transfer_fee_per_quantum: postfiat_execution::TRANSFER_FEE_PER_QUANTUM,
        escrow_weight_bytes: 1,
        sender_balance_after_fee: Some(9_999),
        sender_meets_reserve_after_fee: true,
        operation: operation.clone(),
    };
    std::fs::write(
        &quote_file,
        serde_json::to_string_pretty(&quote).expect("serialize quote"),
    )
    .expect("write quote");
    let quote_json = std::fs::read_to_string(&quote_file).expect("read quote json");
    assert!(
        !quote_json.contains("\"finish_after\""),
        "zero finish_after should remain omitted in the quote fixture"
    );

    let parsed_quote =
        read_wallet_sign_escrow_transaction_quote_file(&quote_file).expect("read quote");
    let signed = wallet_sign_escrow_transaction(WalletSignEscrowTransactionOptions {
        key_file: key_file.clone(),
        chain_id: parsed_quote.chain_id,
        genesis_hash: parsed_quote.genesis_hash,
        protocol_version: parsed_quote.protocol_version,
        fee: parsed_quote.minimum_fee,
        sequence: parsed_quote.sequence,
        expected_source: Some(parsed_quote.source),
        operation: parsed_quote.operation,
    })
    .expect("sign escrow transaction");
    assert_eq!(signed.unsigned.source, key_report.address);
    assert_eq!(signed.unsigned.operation, operation);
    assert_eq!(signed.unsigned.fee, 1);

    run_cli(vec![
        "wallet-sign-escrow-transaction".to_string(),
        "--key-file".to_string(),
        key_file.display().to_string(),
        "--quote-file".to_string(),
        quote_file.display().to_string(),
    ])
    .expect("wallet sign escrow transaction cli");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn certified_asset_ops_prepare_only_writes_resumable_summary() {
    let root = env::temp_dir().join(format!(
        "postfiat-certified-asset-ops-prepare-{}",
        process::id()
    ));
    let data_dir = root.join("node");
    let key_file = root.join("issuer-key.json");
    let backup_file = root.join("issuer-backup.json");
    let ops_file = root.join("ops.json");
    let artifact_dir = root.join("artifacts");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init");
    let key_report = wallet_keygen(WalletKeygenOptions {
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        master_seed_hex: "33".repeat(32),
        account_index: 0,
        key_file: key_file.clone(),
        backup_file,
        overwrite: true,
    })
    .expect("wallet keygen");
    let operation = postfiat_types::AssetTransactionOperation::AssetCreate(
        postfiat_types::AssetCreateOperation {
            issuer: key_report.address.clone(),
            code: "CERTBATCH".to_string(),
            version: 1,
            precision: 6,
            display_name: "Certified Batch Test".to_string(),
            max_supply: Some(10_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        },
    );
    let request = serde_json::json!({
        "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
        "operations": [{
            "label": "asset-create",
            "source": key_report.address,
            "key_file": key_file.display().to_string(),
            "operation": operation,
        }]
    });
    std::fs::write(
        &ops_file,
        serde_json::to_string_pretty(&request).expect("request json"),
    )
    .expect("write ops request");

    let report = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
        data_dir: data_dir.clone(),
        topology_file: root.join("unused-topology.json"),
        key_file: key_file.clone(),
        proposal_key_file: None,
        ops_file: ops_file.clone(),
        artifact_dir: artifact_dir.clone(),
        max_transactions: None,
        require_local_proposer: false,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: false,
        local_apply_before_certified_send: false,
        defer_certified_sends: false,
        block_height: None,
        view: None,
        timeout_certificate_file: None,
        timeout_ms: 5_000,
        send_retries: 0,
        retry_backoff_ms: 250,
        allow_existing_mempool: false,
        resume: false,
        overwrite: false,
        prepare_only: true,
        batch_only: false,
    })
    .expect("prepare certified asset ops");
    assert_eq!(CERTIFIED_ASSET_OPS_REPORT_SCHEMA, report.schema);
    assert!(report.prepare_only);
    assert_eq!(1, report.operation_count);
    assert_eq!(None, report.round_ok);
    assert!(artifact_dir.join("summary.json").exists());
    assert!(artifact_dir
        .join("asset-create")
        .join("operation.json")
        .exists());

    let resumed = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
        data_dir,
        topology_file: root.join("missing-topology-after-resume.json"),
        key_file,
        proposal_key_file: None,
        ops_file,
        artifact_dir,
        max_transactions: None,
        require_local_proposer: false,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: false,
        local_apply_before_certified_send: false,
        defer_certified_sends: false,
        block_height: None,
        view: None,
        timeout_certificate_file: None,
        timeout_ms: 5_000,
        send_retries: 0,
        retry_backoff_ms: 250,
        allow_existing_mempool: false,
        resume: true,
        overwrite: false,
        prepare_only: true,
        batch_only: false,
    })
    .expect("resume certified asset ops");
    assert_eq!(report.operation_count, resumed.operation_count);
    assert_eq!(report.start_state_root, resumed.start_state_root);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn certified_asset_ops_dependencies_validate_and_report_same_round() {
    let root = env::temp_dir().join(format!(
        "postfiat-certified-asset-ops-dependencies-ok-{}",
        process::id()
    ));
    let key_file = root.join("issuer-key.json");
    let backup_file = root.join("issuer-backup.json");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let key_report = wallet_keygen(WalletKeygenOptions {
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        master_seed_hex: "35".repeat(32),
        account_index: 0,
        key_file: key_file.clone(),
        backup_file,
        overwrite: true,
    })
    .expect("wallet keygen");
    let asset_create = |code: &str| {
        postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: key_report.address.clone(),
                code: code.to_string(),
                version: 1,
                precision: 6,
                display_name: format!("Dependency {code}"),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        )
    };
    let request = CertifiedAssetOpsRequest {
        schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
        operations: vec![
            CertifiedAssetOpRequest {
                label: "first".to_string(),
                source: key_report.address.clone(),
                key_file: key_file.clone(),
                operation: asset_create("DEP1"),
                dependencies: Vec::new(),
            },
            CertifiedAssetOpRequest {
                label: "second".to_string(),
                source: key_report.address.clone(),
                key_file,
                operation: asset_create("DEP2"),
                dependencies: vec![CertifiedAssetOpDependency {
                    label: "first".to_string(),
                    mode: "same_round".to_string(),
                    reason: Some("deterministic fixture dependency".to_string()),
                }],
            },
        ],
    };

    validate_certified_asset_ops_request(&request).expect("same-round dependency validates");
    let dependency_report = certified_asset_ops_dependency_report(&request);
    assert_eq!(1, dependency_report.declared_dependency_count);
    assert_eq!(1, dependency_report.same_round_dependency_count);
    assert_eq!(0, dependency_report.prior_round_dependency_count);
    assert!(dependency_report.same_round_batch_eligible);
    assert_eq!(
        vec!["asset_create_then_asset_create".to_string()],
        dependency_report.candidate_batch_classes
    );
    assert!(dependency_report.replay_equivalence_required);
    assert!(!dependency_report.live_round_compression_ready);
    assert_eq!(1, dependency_report.live_round_compression_blockers.len());
    assert!(dependency_report.live_round_compression_blockers[0]
        .contains("replay-equivalence corpus evidence"));
    assert_eq!(
        Some("asset_create_then_asset_create"),
        dependency_report.declarations[0]
            .candidate_batch_class
            .as_deref()
    );
    let normalized = request_to_json(&request).expect("normalize request");
    assert_eq!(
        Some("first"),
        normalized["operations"][1]["dependencies"][0]["label"].as_str()
    );
    assert_eq!(
        Some("same_round"),
        normalized["operations"][1]["dependencies"][0]["mode"].as_str()
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn certified_asset_ops_dependencies_reject_prior_round_same_request() {
    let root = env::temp_dir().join(format!(
        "postfiat-certified-asset-ops-dependencies-bad-{}",
        process::id()
    ));
    let key_file = root.join("issuer-key.json");
    let backup_file = root.join("issuer-backup.json");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let key_report = wallet_keygen(WalletKeygenOptions {
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        master_seed_hex: "36".repeat(32),
        account_index: 0,
        key_file: key_file.clone(),
        backup_file,
        overwrite: true,
    })
    .expect("wallet keygen");
    let operation = postfiat_types::AssetTransactionOperation::AssetCreate(
        postfiat_types::AssetCreateOperation {
            issuer: key_report.address.clone(),
            code: "DEPBAD".to_string(),
            version: 1,
            precision: 6,
            display_name: "Dependency Bad".to_string(),
            max_supply: Some(10_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        },
    );
    let request = CertifiedAssetOpsRequest {
        schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
        operations: vec![
            CertifiedAssetOpRequest {
                label: "first".to_string(),
                source: key_report.address.clone(),
                key_file: key_file.clone(),
                operation: operation.clone(),
                dependencies: Vec::new(),
            },
            CertifiedAssetOpRequest {
                label: "second".to_string(),
                source: key_report.address.clone(),
                key_file,
                operation,
                dependencies: vec![CertifiedAssetOpDependency {
                    label: "first".to_string(),
                    mode: "prior_round".to_string(),
                    reason: Some("must not batch".to_string()),
                }],
            },
        ],
    };

    let error = validate_certified_asset_ops_request(&request)
        .expect_err("prior-round dependency in same request must fail");
    assert!(
        error.contains("requires prior_round but is present in the same request"),
        "{error}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn certified_asset_ops_dependencies_reject_adversarial_declarations() {
    let root = env::temp_dir().join(format!(
        "postfiat-certified-asset-ops-dependencies-adversarial-{}",
        process::id()
    ));
    let key_file = root.join("issuer-key.json");
    let backup_file = root.join("issuer-backup.json");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");
    let key_report = wallet_keygen(WalletKeygenOptions {
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        master_seed_hex: "37".repeat(32),
        account_index: 0,
        key_file: key_file.clone(),
        backup_file,
        overwrite: true,
    })
    .expect("wallet keygen");
    let asset_create = |code: &str| {
        postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: key_report.address.clone(),
                code: code.to_string(),
                version: 1,
                precision: 6,
                display_name: format!("Dependency Attack {code}"),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        )
    };
    let build_request =
        |first_dependencies: Vec<CertifiedAssetOpDependency>,
         second_dependencies: Vec<CertifiedAssetOpDependency>| {
            CertifiedAssetOpsRequest {
                schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
                operations: vec![
                    CertifiedAssetOpRequest {
                        label: "first".to_string(),
                        source: key_report.address.clone(),
                        key_file: key_file.clone(),
                        operation: asset_create("ADV1"),
                        dependencies: first_dependencies,
                    },
                    CertifiedAssetOpRequest {
                        label: "second".to_string(),
                        source: key_report.address.clone(),
                        key_file: key_file.clone(),
                        operation: asset_create("ADV2"),
                        dependencies: second_dependencies,
                    },
                ],
            }
        };
    let same_round_dependency = |label: &str| CertifiedAssetOpDependency {
        label: label.to_string(),
        mode: "same_round".to_string(),
        reason: Some("adversarial dependency fixture".to_string()),
    };
    let prior_round_dependency = |label: &str| CertifiedAssetOpDependency {
        label: label.to_string(),
        mode: "prior_round".to_string(),
        reason: Some("adversarial dependency fixture".to_string()),
    };

    for (request, expected_error) in [
        (
            build_request(vec![same_round_dependency("second")], Vec::new()),
            "must appear earlier",
        ),
        (
            build_request(Vec::new(), vec![same_round_dependency("missing")]),
            "is not present in this request",
        ),
        (
            build_request(vec![same_round_dependency("first")], Vec::new()),
            "cannot depend on itself",
        ),
        (
            build_request(
                Vec::new(),
                vec![
                    same_round_dependency("first"),
                    prior_round_dependency("first"),
                ],
            ),
            "declares duplicate dependency",
        ),
        (
            build_request(
                Vec::new(),
                vec![CertifiedAssetOpDependency {
                    label: "first".to_string(),
                    mode: "after_network_catches_up".to_string(),
                    reason: Some("unsupported mode fixture".to_string()),
                }],
            ),
            "uses unsupported mode",
        ),
    ] {
        let error = validate_certified_asset_ops_request(&request)
            .expect_err("adversarial dependency declaration must fail");
        assert!(
            error.contains(expected_error),
            "expected `{expected_error}` in `{error}`"
        );
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn certified_asset_ops_batch_only_signs_submits_and_batches_asset_create() {
    let root = env::temp_dir().join(format!(
        "postfiat-certified-asset-ops-execute-{}",
        process::id()
    ));
    let data_dir = root.join("node");
    let topology_file = root.join("topology.json");
    let ops_file = root.join("ops.json");
    let artifact_dir = root.join("artifacts");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init");
    write_local_topology(TopologyOptions {
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        validators: 1,
        base_port: 44_000,
        rpc_base_port: None,
        hosts: None,
        output_file: topology_file.clone(),
    })
    .expect("write local topology");
    let faucet = faucet_key(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("faucet key");
    let faucet_key_file = data_dir.join("faucet_key.json");
    let first_operation = postfiat_types::AssetTransactionOperation::AssetCreate(
        postfiat_types::AssetCreateOperation {
            issuer: faucet.address.clone(),
            code: "FASTOPS1".to_string(),
            version: 1,
            precision: 6,
            display_name: "Fast Ops Asset 1".to_string(),
            max_supply: Some(10_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        },
    );
    let second_operation = postfiat_types::AssetTransactionOperation::AssetCreate(
        postfiat_types::AssetCreateOperation {
            issuer: faucet.address.clone(),
            code: "FASTOPS2".to_string(),
            version: 1,
            precision: 6,
            display_name: "Fast Ops Asset 2".to_string(),
            max_supply: Some(10_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        },
    );
    let request = serde_json::json!({
        "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
        "operations": [
        {
            "label": "asset-create-1",
            "source": faucet.address.clone(),
            "key_file": faucet_key_file.display().to_string(),
            "operation": first_operation,
        },
        {
            "label": "asset-create-2",
            "source": faucet.address.clone(),
            "key_file": faucet_key_file.display().to_string(),
            "operation": second_operation,
        }]
    });
    std::fs::write(
        &ops_file,
        serde_json::to_string_pretty(&request).expect("request json"),
    )
    .expect("write ops request");

    let report = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
        data_dir: data_dir.clone(),
        topology_file,
        key_file: data_dir.join(VALIDATOR_KEYS_FILE),
        proposal_key_file: None,
        ops_file,
        artifact_dir: artifact_dir.clone(),
        max_transactions: None,
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: false,
        local_apply_before_certified_send: true,
        defer_certified_sends: false,
        block_height: None,
        view: None,
        timeout_certificate_file: None,
        timeout_ms: 5_000,
        send_retries: 0,
        retry_backoff_ms: 250,
        allow_existing_mempool: false,
        resume: false,
        overwrite: false,
        prepare_only: false,
        batch_only: true,
    })
    .expect("batch certified asset ops");
    assert_eq!(CERTIFIED_ASSET_OPS_REPORT_SCHEMA, report.schema);
    assert_eq!(None, report.round_ok);
    assert_eq!(Some(0), report.end_height);
    assert_eq!(Some(0), report.end_mempool_pending);
    assert_eq!(2, report.operations.len());
    assert_eq!(
        postfiat_types::ASSET_CREATE_TRANSACTION_KIND,
        report.operations[0].transaction_kind
    );
    assert!(report.operations[0].tx_id.is_some());
    assert_eq!(
        report.operations[0].sequence.map(|sequence| sequence + 1),
        report.operations[1].sequence
    );
    assert!(report.operations[1].tx_id.is_some());
    assert!(artifact_dir.join("mempool-batch.json").exists());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn certified_asset_ops_direct_signing_advances_same_source_sequences() {
    let root = env::temp_dir().join(format!(
        "postfiat-certified-asset-ops-direct-sequence-{}",
        process::id()
    ));
    let data_dir = root.join("node");
    let topology_file = root.join("topology.json");
    let artifact_dir = root.join("artifacts");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init");
    write_local_topology(TopologyOptions {
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        validators: 1,
        base_port: 44_030,
        rpc_base_port: None,
        hosts: None,
        output_file: topology_file.clone(),
    })
    .expect("write local topology");
    let faucet = faucet_key(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("faucet key");
    let faucet_key_file = data_dir.join("faucet_key.json");
    let options = CertifiedAssetOpsBatchOptions {
        data_dir: data_dir.clone(),
        topology_file,
        key_file: data_dir.join(VALIDATOR_KEYS_FILE),
        proposal_key_file: None,
        ops_file: root.join("unused-ops.json"),
        artifact_dir: artifact_dir.clone(),
        max_transactions: None,
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: false,
        local_apply_before_certified_send: true,
        defer_certified_sends: false,
        block_height: None,
        view: None,
        timeout_certificate_file: None,
        timeout_ms: 5_000,
        send_retries: 0,
        retry_backoff_ms: 250,
        allow_existing_mempool: false,
        resume: false,
        overwrite: false,
        prepare_only: false,
        batch_only: false,
    };
    let first = CertifiedAssetOpRequest {
        label: "asset-create-1".to_string(),
        source: faucet.address.clone(),
        key_file: faucet_key_file.clone(),
        operation: postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: faucet.address.clone(),
                code: "SEQOPS1".to_string(),
                version: 1,
                precision: 6,
                display_name: "Sequence Ops 1".to_string(),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        ),
        dependencies: Vec::new(),
    };
    let second = CertifiedAssetOpRequest {
        label: "asset-create-2".to_string(),
        source: faucet.address.clone(),
        key_file: faucet_key_file,
        operation: postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: faucet.address.clone(),
                code: "SEQOPS2".to_string(),
                version: 1,
                precision: 6,
                display_name: "Sequence Ops 2".to_string(),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        ),
        dependencies: Vec::new(),
    };

    let first_report =
        run_certified_asset_op_stage(&first, &options, false, None).expect("sign first op");
    let second_report = run_certified_asset_op_stage(
        &second,
        &options,
        false,
        first_report.sequence.map(|sequence| sequence + 1),
    )
    .expect("sign second op");
    assert_eq!(
        first_report.sequence.map(|sequence| sequence + 1),
        second_report.sequence
    );

    let first_signed = first_report
        .signed_file
        .as_ref()
        .map(std::path::PathBuf::from)
        .expect("first signed file");
    let second_signed = second_report
        .signed_file
        .as_ref()
        .map(std::path::PathBuf::from)
        .expect("second signed file");
    create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
        data_dir,
        batch_file: artifact_dir.join("signed-asset-batch.json"),
        signed_asset_transaction_files: vec![first_signed, second_signed],
    })
    .expect("same-source signed batch dry-run accepts consecutive sequences");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn certified_asset_ops_same_round_batch_replay_documents_state_root_difference() {
    fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) {
        std::fs::create_dir_all(dst).expect("create copied dir");
        for entry in std::fs::read_dir(src).expect("read source dir") {
            let entry = entry.expect("read source entry");
            let path = entry.path();
            let target = dst.join(entry.file_name());
            if path.is_dir() {
                copy_dir_all(&path, &target);
            } else {
                std::fs::copy(&path, &target).expect("copy file");
            }
        }
    }

    let root = env::temp_dir().join(format!(
        "postfiat-certified-asset-ops-replay-equivalence-{}",
        process::id()
    ));
    let seed_data_dir = root.join("seed-node");
    let unbatched_data_dir = root.join("unbatched-node");
    let batched_data_dir = root.join("batched-node");
    let topology_file = root.join("topology.json");
    let artifact_dir = root.join("artifacts");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp root");

    init(InitOptions {
        data_dir: seed_data_dir.clone(),
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        node_id: DEFAULT_NODE_ID.to_string(),
        validator_count: 1,
    })
    .expect("init");
    write_local_topology(TopologyOptions {
        chain_id: DEFAULT_CHAIN_ID.to_string(),
        validators: 1,
        base_port: 44_040,
        rpc_base_port: None,
        hosts: None,
        output_file: topology_file.clone(),
    })
    .expect("write local topology");
    let faucet = faucet_key(NodeOptions {
        data_dir: seed_data_dir.clone(),
    })
    .expect("faucet key");
    let faucet_key_file = seed_data_dir.join("faucet_key.json");
    let options = CertifiedAssetOpsBatchOptions {
        data_dir: seed_data_dir.clone(),
        topology_file,
        key_file: seed_data_dir.join(VALIDATOR_KEYS_FILE),
        proposal_key_file: None,
        ops_file: root.join("unused-ops.json"),
        artifact_dir: artifact_dir.clone(),
        max_transactions: None,
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: false,
        local_apply_before_certified_send: true,
        defer_certified_sends: false,
        block_height: None,
        view: None,
        timeout_certificate_file: None,
        timeout_ms: 5_000,
        send_retries: 0,
        retry_backoff_ms: 250,
        allow_existing_mempool: false,
        resume: false,
        overwrite: false,
        prepare_only: false,
        batch_only: false,
    };
    let first = CertifiedAssetOpRequest {
        label: "asset-create-1".to_string(),
        source: faucet.address.clone(),
        key_file: faucet_key_file.clone(),
        operation: postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: faucet.address.clone(),
                code: "EQUIV1".to_string(),
                version: 1,
                precision: 6,
                display_name: "Equivalence Asset 1".to_string(),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        ),
        dependencies: Vec::new(),
    };
    let second = CertifiedAssetOpRequest {
        label: "asset-create-2".to_string(),
        source: faucet.address.clone(),
        key_file: faucet_key_file,
        operation: postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: faucet.address.clone(),
                code: "EQUIV2".to_string(),
                version: 1,
                precision: 6,
                display_name: "Equivalence Asset 2".to_string(),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        ),
        dependencies: vec![CertifiedAssetOpDependency {
            label: "asset-create-1".to_string(),
            mode: "same_round".to_string(),
            reason: Some("state-root replay equivalence fixture".to_string()),
        }],
    };
    let request = CertifiedAssetOpsRequest {
        schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
        operations: vec![first.clone(), second.clone()],
    };
    validate_certified_asset_ops_request(&request).expect("same-round fixture validates");
    let dependency_report = certified_asset_ops_dependency_report(&request);
    assert!(
        dependency_report.same_round_batch_eligible,
        "fixture must remain eligible for same-round replay"
    );
    assert!(dependency_report.replay_equivalence_required);
    assert!(!dependency_report.live_round_compression_ready);

    let first_report =
        run_certified_asset_op_stage(&first, &options, false, None).expect("sign first op");
    let second_report = run_certified_asset_op_stage(
        &second,
        &options,
        false,
        first_report.sequence.map(|sequence| sequence + 1),
    )
    .expect("sign second op");
    let first_signed = first_report
        .signed_file
        .as_ref()
        .map(std::path::PathBuf::from)
        .expect("first signed file");
    let second_signed = second_report
        .signed_file
        .as_ref()
        .map(std::path::PathBuf::from)
        .expect("second signed file");

    copy_dir_all(&seed_data_dir, &unbatched_data_dir);
    copy_dir_all(&seed_data_dir, &batched_data_dir);

    let unbatched_first_batch = root.join("unbatched-first.json");
    create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
        data_dir: unbatched_data_dir.clone(),
        batch_file: unbatched_first_batch.clone(),
        signed_asset_transaction_files: vec![first_signed.clone()],
    })
    .expect("create first unbatched signed batch");
    let first_receipts = apply_batch(ApplyBatchOptions {
        data_dir: unbatched_data_dir.clone(),
        batch_file: unbatched_first_batch,
        certificate_file: None,
    })
    .expect("apply first unbatched batch");
    assert!(
        first_receipts.iter().all(|receipt| receipt.accepted),
        "{first_receipts:?}"
    );

    let unbatched_second_batch = root.join("unbatched-second.json");
    create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
        data_dir: unbatched_data_dir.clone(),
        batch_file: unbatched_second_batch.clone(),
        signed_asset_transaction_files: vec![second_signed.clone()],
    })
    .expect("create second unbatched signed batch");
    let second_receipts = apply_batch(ApplyBatchOptions {
        data_dir: unbatched_data_dir.clone(),
        batch_file: unbatched_second_batch,
        certificate_file: None,
    })
    .expect("apply second unbatched batch");
    assert!(
        second_receipts.iter().all(|receipt| receipt.accepted),
        "{second_receipts:?}"
    );

    let batched_file = root.join("batched.json");
    create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
        data_dir: batched_data_dir.clone(),
        batch_file: batched_file.clone(),
        signed_asset_transaction_files: vec![first_signed, second_signed],
    })
    .expect("create same-round signed batch");
    let batched_receipts = apply_batch(ApplyBatchOptions {
        data_dir: batched_data_dir.clone(),
        batch_file: batched_file,
        certificate_file: None,
    })
    .expect("apply same-round batch");
    assert!(
        batched_receipts.iter().all(|receipt| receipt.accepted),
        "{batched_receipts:?}"
    );

    verify_blocks(NodeOptions {
        data_dir: unbatched_data_dir.clone(),
    })
    .expect("unbatched replay verification");
    verify_blocks(NodeOptions {
        data_dir: batched_data_dir.clone(),
    })
    .expect("batched replay verification");
    let unbatched_status = status(NodeOptions {
        data_dir: unbatched_data_dir.clone(),
    })
    .expect("unbatched status");
    let batched_status = status(NodeOptions {
        data_dir: batched_data_dir.clone(),
    })
    .expect("batched status");
    let unbatched_assets = issuer_assets(IssuerAssetsOptions {
        data_dir: unbatched_data_dir,
        issuer: faucet.address.clone(),
        limit: Some(10),
    })
    .expect("unbatched issuer assets");
    let batched_assets = issuer_assets(IssuerAssetsOptions {
        data_dir: batched_data_dir,
        issuer: faucet.address,
        limit: Some(10),
    })
    .expect("batched issuer assets");

    assert_eq!(2, unbatched_status.block_height);
    assert_eq!(1, batched_status.block_height);
    assert_eq!(
        unbatched_assets.assets, batched_assets.assets,
        "same-round batching must preserve ledger-facing asset definitions"
    );
    assert_ne!(
            unbatched_status.state_root, batched_status.state_root,
            "one-block batching and two-block replay should not be classified as exact state-root equivalent"
        );

    let corpus_report = serde_json::json!({
        "schema": "postfiat-certified-asset-ops-batch-equivalence-corpus-v1",
        "case": "same-source-asset-create-pair",
        "candidate_batch_class": "same_round_consecutive_asset_ops",
        "unbatched_block_height": unbatched_status.block_height,
        "batched_block_height": batched_status.block_height,
        "unbatched_state_root": unbatched_status.state_root,
        "batched_state_root": batched_status.state_root,
        "state_root_match": false,
        "intended_state_root_difference": "unbatched replay commits two ordered blocks while same-round batching commits one ordered block; block history differs even though ledger-facing asset definitions match",
        "ledger_facing_asset_definitions_match": true,
        "safe_for_live_round_compression": false,
        "gate": "do not use as live state-root-equivalent batching until the batch class has a protocol-level proof or an explicit operator-approved root-difference gate",
    });
    write_json_file(
        &root.join("same-round-asset-create-equivalence.json"),
        &corpus_report,
    )
    .expect("write equivalence corpus report");
    assert_eq!(
        Some(false),
        corpus_report["safe_for_live_round_compression"].as_bool()
    );

    let _ = std::fs::remove_dir_all(root);
}
