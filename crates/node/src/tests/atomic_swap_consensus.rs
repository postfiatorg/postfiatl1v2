use super::*;
use postfiat_types::{
    MarketOpsAlignmentParams, MarketOpsMintLimits, MarketOpsReserveDeployLimits,
    NavEpochFinalizeOperation, NavReserveSubmitOperation, MARKET_OPS_FINALIZE_TRANSACTION_KIND,
    MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND, NAV_ASSET_REGISTER_TRANSACTION_KIND,
    NAV_EPOCH_FINALIZE_TRANSACTION_KIND, NAV_PROFILE_REGISTER_TRANSACTION_KIND,
    NAV_PROFILE_VERIFIER_PLACEHOLDER, NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
};

struct AtomicNodeFixture {
    genesis: Genesis,
    ledger: LedgerState,
    owner_0_key: MlDsa65KeyPair,
    owner_1_key: MlDsa65KeyPair,
    bystander_key: MlDsa65KeyPair,
    transaction: SignedAtomicSwapTransaction,
}

fn atomic_node_balance_row(account: &str, asset: &AssetDefinition, balance: u64) -> TrustLine {
    let mut row = TrustLine::new(
        account,
        &asset.issuer,
        &asset.asset_id,
        1_000_000,
        TRUSTLINE_STATE_EXPANSION_FEE,
    )
    .expect("atomic node balance row");
    row.authorized = true;
    row.balance = balance;
    row
}

fn atomic_node_market_ops_policy(marker: u8) -> MarketOpsPolicyRegistration {
    MarketOpsPolicyRegistration {
        program_id: [marker; 32],
        policy_hash: [marker.wrapping_add(1); 32],
        parameter_hash: [marker.wrapping_add(2); 32],
        venue_id: [marker.wrapping_add(3); 32],
        pool_config_hash: [marker.wrapping_add(4); 32],
        hook_code_hash: [marker.wrapping_add(5); 32],
        activation_epoch: 1,
        deactivation_epoch: 0,
    }
}

fn atomic_node_finalized_market_envelope(
    asset_id: &str,
    epoch: u64,
    marker: u8,
) -> (MarketOpsPolicyRegistration, FinalizedMarketOpsEnvelope) {
    let policy = atomic_node_market_ops_policy(marker);
    let envelope = MarketOpsEnvelope {
        encoding_version: 1,
        chain_id: 1,
        adapter_address: [marker; 20],
        vault_address: [marker.wrapping_add(1); 20],
        mint_controller_address: [marker.wrapping_add(2); 20],
        asset_id: market_ops_asset_id(asset_id).expect("atomic node market asset id"),
        epoch,
        program_id: policy.program_id,
        policy_hash: policy.policy_hash,
        parameter_hash: policy.parameter_hash,
        reserve_packet_hash: [marker.wrapping_add(6); 32],
        supply_packet_hash: [marker.wrapping_add(7); 32],
        evidence_root: [marker.wrapping_add(8); 32],
        previous_market_state_hash: [0; 32],
        venue_id: policy.venue_id,
        pool_config_hash: policy.pool_config_hash,
        hook_code_hash: policy.hook_code_hash,
        nav_floor_usd_e8: 100_000_000,
        valid_global_supply_atoms: 1_000_000,
        verified_net_assets_usd_e8: 100_000_000,
        funded_alignment_reserve_usd_e8: 1,
        required_alignment_reserve_usd_e8: 1,
        max_reserve_deploy_usd_e8: 1,
        max_mint_atoms: 0,
        discount_trigger_bps: 300,
        premium_trigger_bps: 1_000,
        data_window_start: 1,
        data_window_end: 2,
        valid_after: 2,
        expires_at: u64::MAX,
        cooldown_seconds: 1,
        nonce: [marker.wrapping_add(9); 32],
    };
    let record = FinalizedMarketOpsEnvelope {
        asset_id: asset_id.to_string(),
        epoch,
        envelope_hash: bytes_to_hex(&envelope.envelope_hash()),
        envelope,
        policy_inputs: None,
        finalized_at_height: 1,
    };
    policy.validate().expect("atomic node market policy");
    record.validate().expect("atomic node finalized envelope");
    (policy, record)
}

fn atomic_node_usd_e8(amount: u128) -> u128 {
    amount * 100_000_000
}

fn atomic_node_market_ops_operation(
    issuer: &str,
    asset_id: &str,
    reserve_packet_hash: &str,
    marker: u8,
) -> MarketOpsFinalizeOperation {
    let policy = atomic_node_market_ops_policy(marker);
    let discount_observations = vec![
        MarketOpsVenueObservation {
            dt_seconds: 4_200,
            price_usd_e8: atomic_node_usd_e8(475) / 100,
            volume_usd_e8: atomic_node_usd_e8(2_500),
        },
        MarketOpsVenueObservation {
            dt_seconds: 5_800,
            price_usd_e8: atomic_node_usd_e8(5),
            volume_usd_e8: atomic_node_usd_e8(7_500),
        },
    ];
    let premium_observations = vec![
        MarketOpsVenueObservation {
            dt_seconds: 1_800,
            price_usd_e8: atomic_node_usd_e8(5_625) / 1_000,
            volume_usd_e8: atomic_node_usd_e8(2_200),
        },
        MarketOpsVenueObservation {
            dt_seconds: 8_200,
            price_usd_e8: atomic_node_usd_e8(5),
            volume_usd_e8: atomic_node_usd_e8(7_800),
        },
    ];
    let policy_inputs = MarketOpsPolicyInputs {
        unit_scale: 1,
        floor_factor_bps: 10_000,
        alignment_params: MarketOpsAlignmentParams {
            policy_min_usd_e8: atomic_node_usd_e8(25_000),
            min_alignment_bps: 100,
            stress_repeat_factor_14d: 3,
            stress_repeat_factor_90d: 2,
            stale_epochs_allowed: 1,
            max_decay_per_epoch_bps: 1_000,
        },
        previous_required_alignment_reserve_usd_e8: 0,
        cost_to_restore_14d_usd_e8: vec![
            atomic_node_usd_e8(20_000),
            atomic_node_usd_e8(45_000),
            atomic_node_usd_e8(45_000),
        ],
        cost_to_restore_90d_usd_e8: vec![
            atomic_node_usd_e8(30_000),
            atomic_node_usd_e8(45_000),
            atomic_node_usd_e8(60_000),
        ],
        reserve_limits: MarketOpsReserveDeployLimits {
            available_alignment_reserve_usd_e8: atomic_node_usd_e8(150_000),
            venue_policy_cap_usd_e8: atomic_node_usd_e8(50_000),
            depth_limited_cap_usd_e8: atomic_node_usd_e8(30_000),
            cooldown_limited_cap_usd_e8: atomic_node_usd_e8(40_000),
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
        asset_id: market_ops_asset_id(asset_id).expect("derive atomic market-ops asset id"),
        epoch: 1,
        program_id: policy.program_id,
        policy_hash: policy.policy_hash,
        parameter_hash: policy.parameter_hash,
        reserve_packet_hash: market_ops_reserve_packet_hash(reserve_packet_hash)
            .expect("derive atomic market-ops reserve hash"),
        supply_packet_hash: market_ops_supply_packet_hash(asset_id, 1, 1_000_000)
            .expect("derive atomic market-ops supply hash"),
        evidence_root: market_ops_evidence_root(
            &policy_inputs.discount_observations,
            &policy_inputs.premium_observations,
        )
        .expect("derive atomic market-ops evidence root"),
        previous_market_state_hash: [0; 32],
        venue_id: policy.venue_id,
        pool_config_hash: policy.pool_config_hash,
        hook_code_hash: policy.hook_code_hash,
        nav_floor_usd_e8: atomic_node_usd_e8(5),
        valid_global_supply_atoms: 1_000_000,
        verified_net_assets_usd_e8: atomic_node_usd_e8(5_000_000),
        funded_alignment_reserve_usd_e8: atomic_node_usd_e8(150_000),
        required_alignment_reserve_usd_e8: atomic_node_usd_e8(135_000),
        max_reserve_deploy_usd_e8: atomic_node_usd_e8(25_875),
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
    MarketOpsFinalizeOperation {
        issuer: issuer.to_string(),
        asset_id: asset_id.to_string(),
        envelope_hash: bytes_to_hex(&envelope.envelope_hash()),
        envelope,
        policy_inputs,
    }
}

fn resign_atomic_node_transaction(
    fixture: &AtomicNodeFixture,
    transaction: &mut SignedAtomicSwapTransaction,
) {
    let signing_bytes = transaction.unsigned.signing_bytes();
    transaction.authorization_0.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign(&fixture.owner_0_key.private_key, &signing_bytes)
            .expect("sign atomic owner 0"),
    );
    transaction.authorization_1.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign(&fixture.owner_1_key.private_key, &signing_bytes)
            .expect("sign atomic owner 1"),
    );
}

fn atomic_node_fixture() -> AtomicNodeFixture {
    let genesis = Genesis::new("postfiat-local");
    let owner_0_key = ml_dsa_65_keygen().expect("owner 0 key");
    let owner_1_key = ml_dsa_65_keygen().expect("owner 1 key");
    let bystander_key = ml_dsa_65_keygen().expect("bystander key");
    let issuer_0_key = ml_dsa_65_keygen().expect("issuer 0 key");
    let issuer_1_key = ml_dsa_65_keygen().expect("issuer 1 key");
    let owner_0 = address_from_public_key(&owner_0_key.public_key);
    let owner_1 = address_from_public_key(&owner_1_key.public_key);
    let bystander = address_from_public_key(&bystander_key.public_key);
    let issuer_0 = address_from_public_key(&issuer_0_key.public_key);
    let issuer_1 = address_from_public_key(&issuer_1_key.public_key);
    let asset_a =
        AssetDefinition::new(&genesis.chain_id, &issuer_0, "NODEA", 1, 6).expect("asset a");
    let asset_b =
        AssetDefinition::new(&genesis.chain_id, &issuer_1, "NODEB", 1, 6).expect("asset b");
    let (asset_0, asset_1) = if asset_a.asset_id < asset_b.asset_id {
        (asset_a, asset_b)
    } else {
        (asset_b, asset_a)
    };
    let mut ledger = LedgerState::new(vec![
        Account::new(
            owner_0.clone(),
            100_000,
            Some(bytes_to_hex(&owner_0_key.public_key)),
        ),
        Account::new(
            owner_1.clone(),
            100_000,
            Some(bytes_to_hex(&owner_1_key.public_key)),
        ),
        Account::new(
            bystander,
            100_000,
            Some(bytes_to_hex(&bystander_key.public_key)),
        ),
        Account::new(
            asset_0.issuer.clone(),
            100_000,
            Some(bytes_to_hex(if asset_0.issuer == issuer_0 {
                &issuer_0_key.public_key
            } else {
                &issuer_1_key.public_key
            })),
        ),
        Account::new(
            asset_1.issuer.clone(),
            100_000,
            Some(bytes_to_hex(if asset_1.issuer == issuer_0 {
                &issuer_0_key.public_key
            } else {
                &issuer_1_key.public_key
            })),
        ),
    ]);
    ledger.asset_definitions = vec![asset_0.clone(), asset_1.clone()];
    ledger.trustlines = vec![
        atomic_node_balance_row(&owner_0, &asset_0, 50),
        atomic_node_balance_row(&owner_1, &asset_1, 80),
    ];
    let price_nav_profile = NavProofProfile::new(
        asset_0.issuer.clone(),
        NAV_PROFILE_VERIFIER_SP1_GROTH16,
        "a651-market-nav",
        100,
        1,
        100,
        0,
        0,
        0,
        0,
        "22".repeat(32),
        format!("0x{}", "11".repeat(32)),
        "groth16",
        0,
        0,
    )
    .expect("price NAV profile");
    let bridge_accounting_profile = NavProofProfile::new(
        asset_1.issuer.clone(),
        NAV_PROFILE_VERIFIER_SP1_GROTH16,
        "pfusdc-bridge-accounting",
        100,
        1,
        100,
        0,
        0,
        0,
        0,
        "33".repeat(32),
        format!("0x{}", "44".repeat(32)),
        "groth16",
        0,
        0,
    )
    .expect("bridge-accounting NAV profile");
    let mut price_nav_asset = NavTrackedAsset::new(
        asset_0.asset_id.clone(),
        asset_0.issuer.clone(),
        asset_0.issuer.clone(),
        price_nav_profile.profile_id.clone(),
        "usd_e8",
        asset_0.issuer.clone(),
    )
    .expect("price NAV asset");
    price_nav_asset.finalized_epoch = 59;
    price_nav_asset.circulating_supply = 50;
    let mut bridge_accounting_asset = NavTrackedAsset::new(
        asset_1.asset_id.clone(),
        asset_1.issuer.clone(),
        asset_1.issuer.clone(),
        bridge_accounting_profile.profile_id.clone(),
        "usd_e8",
        asset_1.issuer.clone(),
    )
    .expect("bridge-accounting NAV asset");
    bridge_accounting_asset.finalized_epoch = 4;
    bridge_accounting_asset.circulating_supply = 80;
    ledger.nav_proof_profiles = vec![price_nav_profile, bridge_accounting_profile];
    ledger.nav_assets = vec![price_nav_asset, bridge_accounting_asset];
    let (market_policy, market_envelope) =
        atomic_node_finalized_market_envelope(&asset_0.asset_id, 59, 0x31);
    ledger.market_ops_policies.push(market_policy);
    ledger.market_ops_envelopes.push(market_envelope.clone());
    let unsigned = UnsignedAtomicSwapTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        rfq_hash: "11".repeat(48),
        market_envelope_hash: market_envelope.envelope_hash,
        nav_epoch: 59,
        expires_at_height: 100,
        swap_nonce: "22".repeat(48),
        leg_0: AtomicSwapLeg {
            owner: owner_0.clone(),
            recipient: owner_1.clone(),
            issuer: asset_0.issuer.clone(),
            asset_id: asset_0.asset_id.clone(),
            amount: 50,
            sequence: 1,
            fee: 1_000,
        },
        leg_1: AtomicSwapLeg {
            owner: owner_1.clone(),
            recipient: owner_0.clone(),
            issuer: asset_1.issuer.clone(),
            asset_id: asset_1.asset_id.clone(),
            amount: 80,
            sequence: 1,
            fee: 1_000,
        },
    };
    let mut transaction = SignedAtomicSwapTransaction {
        authorization_0: AtomicSwapAuthorization {
            owner: owner_0,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_0_key.public_key),
            signature_hex: String::new(),
        },
        authorization_1: AtomicSwapAuthorization {
            owner: owner_1,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_1_key.public_key),
            signature_hex: String::new(),
        },
        unsigned,
    };
    let signing_bytes = transaction.unsigned.signing_bytes();
    transaction.authorization_0.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign(&owner_0_key.private_key, &signing_bytes).expect("sign owner 0"),
    );
    transaction.authorization_1.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign(&owner_1_key.private_key, &signing_bytes).expect("sign owner 1"),
    );
    assert!(transaction.validate().is_ok());
    AtomicNodeFixture {
        genesis,
        ledger,
        owner_0_key,
        owner_1_key,
        bystander_key,
        transaction,
    }
}

fn atomic_node_batch(transaction: SignedAtomicSwapTransaction) -> TransactionBatch {
    let mut batch = TransactionBatch::new("atomic-node-batch", Vec::new());
    batch.atomic_swap_transactions.push(transaction);
    batch
}

fn atomic_node_activation_amendment(height: u32) -> GovernanceAmendment {
    GovernanceAmendment {
        amendment_id: format!("atomic-swap-activation-{height}"),
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "00".repeat(48),
        protocol_version: 1,
        instance_id: "atomic-swap-instance".to_string(),
        proposal_id: "atomic-swap-proposal".to_string(),
        certificate_id: "atomic-swap-certificate".to_string(),
        proposer: "validator-0".to_string(),
        validators: vec!["validator-0".to_string()],
        quorum: 1,
        kind: GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT.to_string(),
        value: height,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        support: vec!["validator-0".to_string()],
        votes: Vec::new(),
        signed_authorizations: Vec::new(),
    }
}

fn atomic_node_dev_key_file(key_pair: &MlDsa65KeyPair) -> DevKeyFile {
    DevKeyFile {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        address: address_from_public_key(&key_pair.public_key),
        public_key_hex: bytes_to_hex(&key_pair.public_key),
        private_key_hex: bytes_to_hex(&key_pair.private_key),
    }
}

fn active_atomic_node_store(fixture: &AtomicNodeFixture, label: &str) -> (PathBuf, NodeStore) {
    let data_dir = unique_test_dir(label);
    fs::create_dir_all(&data_dir).expect("create active atomic node directory");
    let store = NodeStore::new(&data_dir);
    store
        .write_genesis(&fixture.genesis)
        .expect("write genesis");
    store
        .write_node_state(&NodeState::initialized("validator-0"))
        .expect("write node state");
    store.write_ledger(&fixture.ledger).expect("write ledger");
    store
        .write_mempool(&MempoolState::empty())
        .expect("write empty mempool");
    store
        .write_chain_tip(&ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_string(),
            chain_id: fixture.genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&fixture.genesis),
            protocol_version: fixture.genesis.protocol_version,
            height: 0,
            block_hash: "genesis".to_string(),
            state_root: "genesis-state".to_string(),
            ordered_batch_count: 0,
            receipt_count: 0,
            history_base_height: 0,
        })
        .expect("write chain tip");
    store
        .write_receipts(&Vec::new())
        .expect("write empty receipts");
    store
        .write_blocks(&BlockLog::empty())
        .expect("write empty blocks");
    let mut governance = GovernanceState::new(1);
    governance.apply(atomic_node_activation_amendment(1));
    store
        .write_governance(&governance)
        .expect("write active governance");
    (data_dir, store)
}

fn copy_atomic_node_test_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create atomic node test destination");
    for entry in fs::read_dir(source).expect("read atomic node test source") {
        let entry = entry.expect("read atomic node test entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry
            .file_type()
            .expect("read atomic node test entry type")
            .is_dir()
        {
            copy_atomic_node_test_dir(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).expect("copy atomic node test file");
        }
    }
}

#[test]
fn atomic_swap_batch_gate_hard_fails_before_activation_and_while_paused() {
    let fixture = atomic_node_fixture();
    let batch = atomic_node_batch(fixture.transaction);
    let governance = GovernanceState::new(1);
    let inactive =
        asset_execution_compatibility_for_genesis_and_governance(&fixture.genesis, &governance);
    let error = ensure_atomic_swap_batch_allowed(&batch, 1, inactive)
        .expect_err("missing activation accepted");
    assert!(
        error.to_string().contains("atomic_swap_not_active"),
        "{error}"
    );

    let mut active_genesis = fixture.genesis.clone();
    active_genesis.atomic_swap_activation_height = Some(2);
    let active =
        asset_execution_compatibility_for_genesis_and_governance(&active_genesis, &governance);
    assert!(ensure_atomic_swap_batch_allowed(&batch, 1, active).is_err());
    ensure_atomic_swap_batch_allowed(&batch, 2, active).expect("activation boundary rejected");

    let mut paused_governance = governance;
    paused_governance.atomic_swap_paused = true;
    let paused = asset_execution_compatibility_for_genesis_and_governance(
        &active_genesis,
        &paused_governance,
    );
    let error = ensure_atomic_swap_batch_allowed(&batch, 2, paused)
        .expect_err("paused atomic batch accepted");
    assert!(error.to_string().contains("atomic_swap_paused"));
}

#[test]
fn atomic_swap_archive_replay_rejects_preactivation_without_mutation() {
    let fixture = atomic_node_fixture();
    let tx_id = atomic_swap_transaction_tx_id(&fixture.transaction);
    let batch = atomic_node_batch(fixture.transaction);
    let mut block = dummy_block_record(1);
    block.receipt_ids = vec![tx_id];
    block.header.receipt_count = 1;
    let governance = GovernanceState::new(1);
    let mut ledger = fixture.ledger.clone();
    let before = ledger.clone();
    let error = execute_transparent_batch_for_archive_replay(
        &fixture.genesis,
        &mut ledger,
        &batch,
        &block,
        &governance,
    )
    .expect_err("preactivation archive swap replayed");
    assert!(error.to_string().contains("atomic_swap_not_active"));
    assert_eq!(ledger, before);

    let active_genesis = fixture.genesis;
    let mut active_governance = governance;
    active_governance.apply(atomic_node_activation_amendment(1));
    let receipts = execute_transparent_batch_for_archive_replay(
        &active_genesis,
        &mut ledger,
        &batch,
        &block,
        &active_governance,
    )
    .expect("active archive swap failed");
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted);
}

#[test]
fn atomic_swap_mempool_verification_tracks_both_owners_and_conflicts() {
    let fixture = atomic_node_fixture();
    let entry = MempoolAtomicSwapEntry::new(
        atomic_swap_transaction_tx_id(&fixture.transaction),
        fixture.transaction.clone(),
    );
    let mut mempool = MempoolState::empty();
    mempool.pending_atomic_swaps.push(entry);
    let report = verify_mempool_state(
        &fixture.genesis,
        &fixture.ledger,
        &ShieldedState::empty(),
        &mempool,
        1,
        AssetExecutionCompatibility::strict(),
    )
    .expect("valid atomic mempool rejected");
    assert_eq!(report.pending_count, 1);
    assert_eq!(report.sender_count, 2);
    assert_eq!(report.total_amount, 130);
    assert_eq!(report.total_fee, 2_000);

    for conflict_leg in 0..2 {
        let mut conflicting = fixture.transaction.clone();
        conflicting.unsigned.swap_nonce = format!("{:02x}", 48 + conflict_leg).repeat(48);
        if conflict_leg == 0 {
            conflicting.unsigned.leg_1.sequence = 2;
        } else {
            conflicting.unsigned.leg_0.sequence = 2;
        }
        resign_atomic_node_transaction(&fixture, &mut conflicting);
        let conflicting_owner = if conflict_leg == 0 {
            conflicting.unsigned.leg_0.owner.clone()
        } else {
            conflicting.unsigned.leg_1.owner.clone()
        };
        let mut conflicted = mempool.clone();
        conflicted
            .pending_atomic_swaps
            .push(MempoolAtomicSwapEntry::new(
                atomic_swap_transaction_tx_id(&conflicting),
                conflicting,
            ));
        let error = verify_mempool_state(
            &fixture.genesis,
            &fixture.ledger,
            &ShieldedState::empty(),
            &conflicted,
            1,
            AssetExecutionCompatibility::strict(),
        )
        .expect_err("duplicate atomic owner sequence accepted");
        assert!(error.to_string().contains(&conflicting_owner));
    }
}

#[test]
fn atomic_swap_and_conflicting_single_reject_in_both_orders_for_both_owners() {
    let fixture = atomic_node_fixture();
    let bystander = address_from_public_key(&fixture.bystander_key.public_key);
    let expected_atomic_tx_id = atomic_swap_transaction_tx_id(&fixture.transaction);
    for (owner_index, (owner_key, owner)) in [
        (
            &fixture.owner_0_key,
            fixture.transaction.unsigned.leg_0.owner.as_str(),
        ),
        (
            &fixture.owner_1_key,
            fixture.transaction.unsigned.leg_1.owner.as_str(),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let owner_key_file = atomic_node_dev_key_file(owner_key);
        let transfer = build_signed_transfer_for_key(
            &fixture.genesis,
            &fixture.ledger,
            &owner_key_file,
            bystander.clone(),
            1,
            1,
        )
        .expect("build owner transfer");
        let expected_transfer_tx_id = transfer_tx_id(&transfer);
        let transfer_json = serde_json::to_string(&transfer).expect("serialize owner transfer");

        for swap_first in [true, false] {
            let label = format!(
                "postfiat-atomic-swap-single-conflict-owner-{owner_index}-swap-first-{swap_first}"
            );
            let (data_dir, store) = active_atomic_node_store(&fixture, &label);
            if swap_first {
                admit_signed_atomic_swap_to_mempool(&data_dir, fixture.transaction.clone())
                    .expect("admit atomic before conflicting single");
                let error =
                    submit_signed_transfer_json_to_mempool(SignedTransferJsonSubmitOptions {
                        data_dir: data_dir.clone(),
                        signed_transfer_json: transfer_json.clone(),
                    })
                    .expect_err("conflicting single admitted after atomic swap");
                assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
                assert!(error.to_string().contains(owner), "{error}");
                assert!(
                    error
                        .to_string()
                        .contains("participates in a pending atomic swap"),
                    "{error}"
                );
                let mempool = store.read_mempool().expect("read swap-first mempool");
                assert!(mempool.pending.is_empty());
                assert_eq!(mempool.pending_atomic_swaps.len(), 1);
                assert_eq!(mempool.pending_atomic_swaps[0].tx_id, expected_atomic_tx_id);
            } else {
                submit_signed_transfer_json_to_mempool(SignedTransferJsonSubmitOptions {
                    data_dir: data_dir.clone(),
                    signed_transfer_json: transfer_json.clone(),
                })
                .expect("admit conflicting single before atomic");
                let error =
                    admit_signed_atomic_swap_to_mempool(&data_dir, fixture.transaction.clone())
                        .expect_err("atomic swap admitted after conflicting single");
                assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
                assert!(error.to_string().contains(owner), "{error}");
                assert!(
                    error
                        .to_string()
                        .contains("already has a pending transaction"),
                    "{error}"
                );
                let mempool = store.read_mempool().expect("read single-first mempool");
                assert_eq!(mempool.pending.len(), 1);
                assert_eq!(mempool.pending[0].tx_id, expected_transfer_tx_id);
                assert!(mempool.pending_atomic_swaps.is_empty());
            }
            fs::remove_dir_all(data_dir).expect("remove conflict test directory");
        }
    }
}

#[test]
fn stale_atomic_swap_is_evicted_before_proposal_without_disturbing_unrelated_transfer() {
    let fixture = atomic_node_fixture();
    let (data_dir, store) =
        active_atomic_node_store(&fixture, "postfiat-stale-atomic-swap-eviction");
    let atomic_tx_id = atomic_swap_transaction_tx_id(&fixture.transaction);
    admit_signed_atomic_swap_to_mempool(&data_dir, fixture.transaction.clone())
        .expect("admit atomic before competing finality");

    let bystander = address_from_public_key(&fixture.bystander_key.public_key);
    let owner_0_key_file = atomic_node_dev_key_file(&fixture.owner_0_key);
    let competing_finalized_transfer = build_signed_transfer_for_key(
        &fixture.genesis,
        &fixture.ledger,
        &owner_0_key_file,
        bystander.clone(),
        1,
        1,
    )
    .expect("build competing finalized transfer");
    let mut advanced_ledger = fixture.ledger.clone();
    let competing_receipt = execute_transfer(
        &fixture.genesis,
        &mut advanced_ledger,
        &competing_finalized_transfer,
    );
    assert!(competing_receipt.accepted, "{competing_receipt:?}");
    store
        .write_ledger(&advanced_ledger)
        .expect("write legitimately advanced ledger");

    let bystander_key_file = atomic_node_dev_key_file(&fixture.bystander_key);
    let unrelated = build_signed_transfer_for_key(
        &fixture.genesis,
        &advanced_ledger,
        &bystander_key_file,
        fixture.transaction.unsigned.leg_1.owner.clone(),
        1,
        1,
    )
    .expect("build unrelated transfer");
    submit_signed_transfer_json_to_mempool(SignedTransferJsonSubmitOptions {
        data_dir: data_dir.clone(),
        signed_transfer_json: serde_json::to_string(&unrelated)
            .expect("serialize unrelated transfer"),
    })
    .expect("stale atomic swap blocked unrelated admission");

    let stale_error = verify_mempool(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("stale atomic swap passed mempool verification");
    assert!(
        stale_error.to_string().contains(&atomic_tx_id),
        "{stale_error}"
    );
    assert!(
        stale_error.to_string().contains("bad_sequence"),
        "{stale_error}"
    );

    let selected = create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: data_dir.join("stale-atomic-swap-eviction.batch.json"),
        max_transactions: 2,
    })
    .expect("stale atomic swap blocked unrelated batch selection");
    assert_eq!(selected.transactions, vec![unrelated]);
    assert!(selected.atomic_swap_transactions.is_empty());
    assert!(store
        .read_mempool()
        .expect("read drained mempool")
        .is_empty());
    assert_eq!(
        store.read_ledger().expect("read post-selection ledger"),
        advanced_ledger
    );

    let governance = store.read_governance().expect("read active governance");
    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&fixture.genesis, &governance);
    let mut execution_ledger = advanced_ledger;
    let receipts = execute_transparent_batch(
        &fixture.genesis,
        &governance,
        &mut execution_ledger,
        &selected,
        1,
        compatibility,
    );
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted, "{:?}", receipts[0]);
    fs::remove_dir_all(data_dir).expect("remove stale eviction test directory");
}

#[test]
fn atomic_swap_quote_resolves_both_owners_and_submits_the_exact_unsigned_envelope() {
    let fixture = atomic_node_fixture();
    let (data_dir, store) = active_atomic_node_store(&fixture, "postfiat-atomic-swap-quote");
    let source = &fixture.transaction.unsigned;
    let report = atomic_swap_fee_quote(AtomicSwapFeeQuoteOptions {
        data_dir: data_dir.clone(),
        rfq_hash: source.rfq_hash.clone(),
        market_envelope_hash: source.market_envelope_hash.clone(),
        nav_epoch: source.nav_epoch,
        expires_at_height: source.expires_at_height,
        swap_nonce: source.swap_nonce.clone(),
        leg_0: AtomicSwapQuoteLegInput {
            owner: source.leg_0.owner.clone(),
            recipient: source.leg_0.recipient.clone(),
            issuer: source.leg_0.issuer.clone(),
            asset_id: source.leg_0.asset_id.clone(),
            amount: source.leg_0.amount,
        },
        leg_1: AtomicSwapQuoteLegInput {
            owner: source.leg_1.owner.clone(),
            recipient: source.leg_1.recipient.clone(),
            issuer: source.leg_1.issuer.clone(),
            asset_id: source.leg_1.asset_id.clone(),
            amount: source.leg_1.amount,
        },
    })
    .expect("quote atomic swap");
    assert_eq!(report.schema, ATOMIC_SWAP_FEE_QUOTE_SCHEMA);
    assert_eq!(report.quote_height, 1);
    assert_eq!(report.leg_0.sequence, 1);
    assert_eq!(report.leg_1.sequence, 1);
    assert_eq!(report.leg_0.mempool_pending_for_owner, 0);
    assert_eq!(report.leg_1.mempool_pending_for_owner, 0);
    assert!(report.leg_0.minimum_fee >= report.leg_0.base_atomic_swap_fee);
    assert!(report.leg_1.minimum_fee >= report.leg_1.base_atomic_swap_fee);
    assert!(report.leg_0.state_expansion_fee > 0);
    assert!(report.leg_1.state_expansion_fee > 0);
    assert_eq!(
        report.leg_0.minimum_fee,
        report.leg_0.base_atomic_swap_fee + report.leg_0.state_expansion_fee
    );
    assert_eq!(
        report.leg_1.minimum_fee,
        report.leg_1.base_atomic_swap_fee + report.leg_1.state_expansion_fee
    );
    assert_eq!(
        report.unsigned_transaction.leg_0.fee,
        report.leg_0.minimum_fee
    );
    assert_eq!(
        report.unsigned_transaction.leg_1.fee,
        report.leg_1.minimum_fee
    );

    let mut signed = SignedAtomicSwapTransaction {
        unsigned: report.unsigned_transaction.clone(),
        authorization_0: fixture.transaction.authorization_0.clone(),
        authorization_1: fixture.transaction.authorization_1.clone(),
    };
    resign_atomic_node_transaction(&fixture, &mut signed);
    let entry = submit_signed_atomic_swap_transaction_json_to_mempool(
        SignedAtomicSwapTransactionJsonSubmitOptions {
            data_dir: data_dir.clone(),
            signed_atomic_swap_transaction_json: serde_json::to_string(&signed)
                .expect("serialize quoted atomic swap"),
        },
    )
    .expect("submit quoted atomic swap");
    assert_eq!(entry.tx_id, atomic_swap_transaction_tx_id(&signed));
    let quote_json = serde_json::to_string(&report).expect("serialize quote report");
    for forbidden_surface in ["trustline", "trust_set", "authorized", "line_create"] {
        assert!(
            !quote_json.contains(forbidden_surface),
            "quote exposed forbidden user-facing balance-row surface `{forbidden_surface}`"
        );
    }
    let mut executed = fixture.ledger.clone();
    let receipt = execute_atomic_swap_transaction_with_compatibility(
        &fixture.genesis,
        &mut executed,
        &signed,
        report.quote_height,
        AssetExecutionCompatibility::strict(),
    );
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(
        executed
            .trustline_for_account_asset(
                &signed.unsigned.leg_0.recipient,
                &signed.unsigned.leg_0.asset_id,
            )
            .expect("implicitly created leg 0 recipient balance row")
            .balance,
        signed.unsigned.leg_0.amount
    );
    assert_eq!(
        executed
            .trustline_for_account_asset(
                &signed.unsigned.leg_1.recipient,
                &signed.unsigned.leg_1.asset_id,
            )
            .expect("implicitly created leg 1 recipient balance row")
            .balance,
        signed.unsigned.leg_1.amount
    );

    let pending_error = atomic_swap_fee_quote(AtomicSwapFeeQuoteOptions {
        data_dir: data_dir.clone(),
        rfq_hash: source.rfq_hash.clone(),
        market_envelope_hash: source.market_envelope_hash.clone(),
        nav_epoch: source.nav_epoch,
        expires_at_height: source.expires_at_height,
        swap_nonce: "33".repeat(48),
        leg_0: AtomicSwapQuoteLegInput {
            owner: source.leg_0.owner.clone(),
            recipient: source.leg_0.recipient.clone(),
            issuer: source.leg_0.issuer.clone(),
            asset_id: source.leg_0.asset_id.clone(),
            amount: source.leg_0.amount,
        },
        leg_1: AtomicSwapQuoteLegInput {
            owner: source.leg_1.owner.clone(),
            recipient: source.leg_1.recipient.clone(),
            issuer: source.leg_1.issuer.clone(),
            asset_id: source.leg_1.asset_id.clone(),
            amount: source.leg_1.amount,
        },
    })
    .expect_err("quote ignored owner pending state");
    assert_eq!(pending_error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(store.read_mempool().expect("read quoted mempool").len(), 1);
    fs::remove_dir_all(data_dir).expect("remove quote test directory");
}

#[test]
fn atomic_swap_quote_uses_consensus_market_binding_before_returning_a_quote() {
    let fixture = atomic_node_fixture();
    let (data_dir, store) =
        active_atomic_node_store(&fixture, "postfiat-atomic-swap-quote-market-binding");
    let source = &fixture.transaction.unsigned;
    let quote_options = |market_envelope_hash: String, nav_epoch: u64| {
        AtomicSwapFeeQuoteOptions {
            data_dir: data_dir.clone(),
            rfq_hash: source.rfq_hash.clone(),
            market_envelope_hash,
            nav_epoch,
            expires_at_height: source.expires_at_height,
            swap_nonce: source.swap_nonce.clone(),
            leg_0: AtomicSwapQuoteLegInput {
                owner: source.leg_0.owner.clone(),
                recipient: source.leg_0.recipient.clone(),
                issuer: source.leg_0.issuer.clone(),
                asset_id: source.leg_0.asset_id.clone(),
                amount: source.leg_0.amount,
            },
            leg_1: AtomicSwapQuoteLegInput {
                owner: source.leg_1.owner.clone(),
                recipient: source.leg_1.recipient.clone(),
                issuer: source.leg_1.issuer.clone(),
                asset_id: source.leg_1.asset_id.clone(),
                amount: source.leg_1.amount,
            },
        }
    };

    let valid = atomic_swap_fee_quote(quote_options(
        source.market_envelope_hash.clone(),
        source.nav_epoch,
    ))
    .expect("bridge-accounting plus one price-NAV asset must quote");
    assert_eq!(
        valid.unsigned_transaction.market_envelope_hash,
        source.market_envelope_hash
    );
    assert_eq!(valid.unsigned_transaction.nav_epoch, source.nav_epoch);

    let wrong_epoch = atomic_swap_fee_quote(quote_options(
        source.market_envelope_hash.clone(),
        source.nav_epoch + 1,
    ))
    .expect_err("price-NAV quote accepted the wrong finalized epoch");
    assert_eq!(wrong_epoch.kind(), io::ErrorKind::InvalidInput);
    assert!(
        wrong_epoch.to_string().starts_with("wrong_nav_epoch:"),
        "{wrong_epoch}"
    );

    let wrong_envelope =
        atomic_swap_fee_quote(quote_options("ff".repeat(48), source.nav_epoch))
            .expect_err("price-NAV quote accepted the wrong market envelope");
    assert_eq!(wrong_envelope.kind(), io::ErrorKind::InvalidInput);
    assert!(
        wrong_envelope
            .to_string()
            .starts_with("wrong_market_envelope:"),
        "{wrong_envelope}"
    );

    let mut zero_price_ledger = fixture.ledger.clone();
    zero_price_ledger.market_ops_envelopes.clear();
    store
        .write_ledger(&zero_price_ledger)
        .expect("write zero-price-envelope ledger");
    let zero_price = atomic_swap_fee_quote(quote_options(
        source.market_envelope_hash.clone(),
        source.nav_epoch,
    ))
    .expect_err("atomic quote accepted zero price-NAV legs");
    assert_eq!(zero_price.kind(), io::ErrorKind::InvalidInput);
    assert!(
        zero_price
            .to_string()
            .starts_with("wrong_market_envelope:"),
        "{zero_price}"
    );

    let mut historical_only_ledger = fixture.ledger.clone();
    let historical = historical_only_ledger
        .market_ops_envelopes
        .first_mut()
        .expect("price envelope");
    historical.epoch -= 1;
    historical.envelope.epoch -= 1;
    historical.envelope_hash = bytes_to_hex(&historical.envelope.envelope_hash());
    historical.validate().expect("historical envelope");
    store
        .write_ledger(&historical_only_ledger)
        .expect("write historical-only price ledger");
    let missing_exact = atomic_swap_fee_quote(quote_options(
        source.market_envelope_hash.clone(),
        source.nav_epoch,
    ))
    .expect_err("atomic quote accepted an absent exact envelope tuple");
    assert_eq!(missing_exact.kind(), io::ErrorKind::InvalidInput);
    assert!(
        missing_exact
            .to_string()
            .starts_with("wrong_market_envelope:"),
        "{missing_exact}"
    );

    let mut dual_price_ledger = fixture.ledger.clone();
    let (second_policy, second_envelope) =
        atomic_node_finalized_market_envelope(&source.leg_1.asset_id, 4, 0x51);
    dual_price_ledger.market_ops_policies.push(second_policy);
    dual_price_ledger
        .market_ops_envelopes
        .push(second_envelope);
    store
        .write_ledger(&dual_price_ledger)
        .expect("write dual-price quote ledger");
    let dual_price = atomic_swap_fee_quote(quote_options(
        source.market_envelope_hash.clone(),
        source.nav_epoch,
    ))
    .expect_err("atomic quote accepted two price-NAV assets");
    assert_eq!(dual_price.kind(), io::ErrorKind::InvalidInput);
    assert!(
        dual_price
            .to_string()
            .starts_with("nav_pair_not_supported:"),
        "{dual_price}"
    );

    fs::remove_dir_all(data_dir).expect("remove quote market-binding test directory");
}

#[test]
fn targeted_atomic_batch_preserves_unrelated_traffic_and_fails_closed_without_target() {
    let fixture = atomic_node_fixture();
    let (data_dir, store) = active_atomic_node_store(&fixture, "postfiat-targeted-atomic-batch");
    let bystander = address_from_public_key(&fixture.bystander_key.public_key);
    let unrelated = build_signed_transfer_for_key(
        &fixture.genesis,
        &fixture.ledger,
        &atomic_node_dev_key_file(&fixture.bystander_key),
        fixture.transaction.unsigned.leg_0.owner.clone(),
        1,
        1,
    )
    .expect("build unrelated transfer");
    let unrelated_entry = submit_signed_transfer_json_to_mempool(SignedTransferJsonSubmitOptions {
        data_dir: data_dir.clone(),
        signed_transfer_json: serde_json::to_string(&unrelated)
            .expect("serialize unrelated transfer"),
    })
    .expect("admit unrelated transfer");
    assert_eq!(unrelated.unsigned.from, bystander);

    let atomic_entry = admit_signed_atomic_swap_to_mempool(&data_dir, fixture.transaction.clone())
        .expect("admit target atomic swap");
    let batch_path = data_dir.join("targeted-atomic.batch.json");
    let batch = create_atomic_swap_mempool_batch_for_tx_id(AtomicSwapTargetBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_path,
        tx_id: atomic_entry.tx_id.clone(),
    })
    .expect("select target atomic swap");
    assert!(batch.transactions.is_empty());
    assert_eq!(batch.atomic_swap_transactions, vec![fixture.transaction]);
    let remaining = store.read_mempool().expect("read remaining mempool");
    assert_eq!(remaining.pending.len(), 1);
    assert_eq!(remaining.pending[0].tx_id, unrelated_entry.tx_id);
    assert!(remaining.pending_atomic_swaps.is_empty());

    let before_missing = remaining;
    let missing_path = data_dir.join("missing-target.batch.json");
    let missing = create_atomic_swap_mempool_batch_for_tx_id(AtomicSwapTargetBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: missing_path.clone(),
        tx_id: "44".repeat(48),
    })
    .expect_err("missing target produced a batch");
    assert_eq!(missing.kind(), io::ErrorKind::NotFound);
    assert!(!missing_path.exists());
    assert_eq!(
        store.read_mempool().expect("read after missing target"),
        before_missing
    );
    fs::remove_dir_all(data_dir).expect("remove target batch test directory");
}

#[test]
fn targeted_atomic_batch_rejects_a_stale_target_without_any_mempool_or_batch_mutation() {
    let fixture = atomic_node_fixture();
    let (data_dir, store) = active_atomic_node_store(&fixture, "postfiat-stale-targeted-atomic");
    let entry = admit_signed_atomic_swap_to_mempool(&data_dir, fixture.transaction.clone())
        .expect("admit target before staling it");
    let before_mempool = store.read_mempool().expect("read admitted mempool");
    let mut advanced = fixture.ledger.clone();
    advanced
        .account_mut(&fixture.transaction.unsigned.leg_0.owner)
        .expect("owner 0 account")
        .sequence = 1;
    store
        .write_ledger(&advanced)
        .expect("advance owner sequence");
    let batch_path = data_dir.join("stale-target.batch.json");
    let error = create_atomic_swap_mempool_batch_for_tx_id(AtomicSwapTargetBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_path.clone(),
        tx_id: entry.tx_id,
    })
    .expect_err("stale target produced a batch");
    assert!(error.to_string().contains("bad_sequence"), "{error}");
    assert!(!batch_path.exists());
    assert_eq!(
        store.read_mempool().expect("read after stale target"),
        before_mempool
    );
    fs::remove_dir_all(data_dir).expect("remove stale target test directory");
}

#[test]
fn atomic_swap_required_parent_rejects_before_proposal_artifact_or_state_mutation() {
    let fixture = atomic_node_fixture();
    let (data_dir, store) = active_atomic_node_store(&fixture, "postfiat-atomic-parent-pin");
    let before_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("read parent-pin status");
    let before_mempool = store.read_mempool().expect("read parent-pin mempool");
    let proposal_file = data_dir.join("must-not-exist-proposal.json");
    let error = propose_batch_with_required_parent_with_timings(
        BatchProposalOptions {
            data_dir: data_dir.clone(),
            verify_block_log: false,
            batch_kind: Some("transparent".to_string()),
            batch_file: data_dir.join("unused-batch.json"),
            proposal_file: proposal_file.clone(),
            view: None,
            timeout_certificate_file: None,
            key_file: None,
            validator_id: None,
        },
        &RequiredBlockParent {
            height: before_status.block_height,
            block_hash: before_status.block_tip_hash.clone(),
            state_root: "ff".repeat(48),
        },
    )
    .expect_err("wrong required parent root produced a proposal");
    assert!(error
        .to_string()
        .contains("required proposal parent mismatch"));
    assert!(!proposal_file.exists());
    assert_eq!(
        status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("read unchanged parent-pin status"),
        before_status
    );
    assert_eq!(
        store
            .read_mempool()
            .expect("read unchanged parent-pin mempool"),
        before_mempool
    );
    fs::remove_dir_all(data_dir).expect("remove parent-pin test directory");
}

#[test]
fn atomic_swap_parsed_admission_is_activation_gated_and_deduplicated() {
    let fixture = atomic_node_fixture();
    let data_dir = unique_test_dir("postfiat-atomic-swap-admission");
    fs::create_dir_all(&data_dir).expect("create atomic admission directory");
    let store = NodeStore::new(&data_dir);
    store
        .write_genesis(&fixture.genesis)
        .expect("write genesis");
    store.write_ledger(&fixture.ledger).expect("write ledger");
    store
        .write_mempool(&MempoolState::empty())
        .expect("write mempool");
    store
        .write_chain_tip(&ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_string(),
            chain_id: fixture.genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&fixture.genesis),
            protocol_version: fixture.genesis.protocol_version,
            height: 0,
            block_hash: "genesis".to_string(),
            state_root: "genesis-state".to_string(),
            ordered_batch_count: 0,
            receipt_count: 0,
            history_base_height: 0,
        })
        .expect("write chain tip");

    let governance = GovernanceState::new(1);
    store
        .write_governance(&governance)
        .expect("write inactive governance");
    let error = admit_signed_atomic_swap_to_mempool(&data_dir, fixture.transaction.clone())
        .expect_err("preactivation swap admitted");
    assert!(
        error.to_string().contains("atomic_swap_not_active"),
        "{error}"
    );
    assert!(store.read_mempool().expect("read mempool").is_empty());

    let mut active_governance = governance;
    active_governance.apply(atomic_node_activation_amendment(1));
    store
        .write_governance(&active_governance)
        .expect("write active governance");
    let entry = admit_signed_atomic_swap_to_mempool(&data_dir, fixture.transaction.clone())
        .expect("active swap admission failed");
    assert_eq!(
        entry.tx_id,
        atomic_swap_transaction_tx_id(&fixture.transaction)
    );
    assert_eq!(
        store
            .read_mempool()
            .expect("read admitted mempool")
            .pending_atomic_swaps
            .len(),
        1
    );
    let replay = admit_signed_atomic_swap_to_mempool(&data_dir, fixture.transaction)
        .expect_err("duplicate swap admitted");
    assert!(replay.to_string().contains("already pending"));
    let selected = create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: data_dir.join("atomic-swap.batch.json"),
        max_transactions: 1,
    })
    .expect("select atomic swap batch");
    assert_eq!(selected.transaction_count(), 1);
    assert_eq!(selected.atomic_swap_transactions.len(), 1);
    assert!(store
        .read_mempool()
        .expect("read selected mempool")
        .is_empty());
    fs::remove_dir_all(data_dir).expect("remove atomic admission directory");
}

#[test]
fn pausing_an_admitted_atomic_swap_does_not_wedge_unrelated_mempool_traffic() {
    let fixture = atomic_node_fixture();
    let data_dir = unique_test_dir("postfiat-paused-atomic-mempool-isolation");
    fs::create_dir_all(&data_dir).expect("create paused atomic directory");
    let store = NodeStore::new(&data_dir);
    store
        .write_genesis(&fixture.genesis)
        .expect("write genesis");
    store.write_ledger(&fixture.ledger).expect("write ledger");
    store
        .write_mempool(&MempoolState::empty())
        .expect("write mempool");
    store
        .write_chain_tip(&ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_string(),
            chain_id: fixture.genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&fixture.genesis),
            protocol_version: fixture.genesis.protocol_version,
            height: 0,
            block_hash: "genesis".to_string(),
            state_root: "genesis-state".to_string(),
            ordered_batch_count: 0,
            receipt_count: 0,
            history_base_height: 0,
        })
        .expect("write chain tip");

    let mut governance = GovernanceState::new(1);
    governance.apply(atomic_node_activation_amendment(1));
    store
        .write_governance(&governance)
        .expect("write active governance");
    admit_signed_atomic_swap_to_mempool(&data_dir, fixture.transaction.clone())
        .expect("admit atomic before pause");

    governance.atomic_swap_paused = true;
    store
        .write_governance(&governance)
        .expect("write paused governance");
    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&fixture.genesis, &governance);
    ledger_after_executable_mempool(
        &fixture.genesis,
        fixture.ledger.clone(),
        &store.read_mempool().expect("read paused mempool"),
        1,
        compatibility,
    )
    .expect("paused atomic wedged unrelated dry-run");

    let bystander = DevKeyFile {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        address: address_from_public_key(&fixture.bystander_key.public_key),
        public_key_hex: bytes_to_hex(&fixture.bystander_key.public_key),
        private_key_hex: bytes_to_hex(&fixture.bystander_key.private_key),
    };
    let transfer = build_signed_transfer_for_key(
        &fixture.genesis,
        &fixture.ledger,
        &bystander,
        fixture.transaction.unsigned.leg_0.owner.clone(),
        1,
        1,
    )
    .expect("build unrelated transfer");
    submit_signed_transfer_json_to_mempool(SignedTransferJsonSubmitOptions {
        data_dir: data_dir.clone(),
        signed_transfer_json: serde_json::to_string(&transfer).expect("serialize transfer"),
    })
    .expect("paused atomic blocked unrelated transfer admission");

    let selected = create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: data_dir.join("paused-atomic-isolation.batch.json"),
        max_transactions: 2,
    })
    .expect("paused atomic blocked unrelated batch selection");
    assert_eq!(selected.transactions.len(), 1);
    assert!(selected.atomic_swap_transactions.is_empty());
    assert!(store
        .read_mempool()
        .expect("read drained mempool")
        .is_empty());
    fs::remove_dir_all(data_dir).expect("remove paused atomic directory");
}

#[test]
fn atomic_swap_account_index_emits_two_plain_balance_rows_without_trustline_fields() {
    let fixture = atomic_node_fixture();
    let batch = atomic_node_batch(fixture.transaction);
    let mut ledger = fixture.ledger;
    let receipts = execute_transparent_batch(
        &fixture.genesis,
        &GovernanceState::new(6),
        &mut ledger,
        &batch,
        1,
        AssetExecutionCompatibility::strict(),
    );
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted);
    let tx_id = receipts[0].tx_id.clone();
    let mut block = dummy_block_record(1);
    block.header.batch_id = batch.batch_id.clone();
    block.header.receipt_count = 1;
    block.receipt_ids = vec![tx_id];
    let archive = BatchArchive {
        batches: vec![BatchArchiveEntry {
            batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
            batch_id: batch.batch_id.clone(),
            payload_hash: String::new(),
            payload_json: serde_json::to_string(&batch).expect("serialize atomic batch"),
        }],
    };
    let receipt_map = receipt_by_tx_id(&receipts).expect("receipt map");
    let rows = account_tx_rows_for_transparent_block(&block, &archive, &receipt_map)
        .expect("atomic account rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].transaction_kind, ATOMIC_SWAP_TRANSACTION_KIND);
    assert_eq!(rows[0].tx_role.as_deref(), Some("leg_0"));
    assert_eq!(rows[1].tx_role.as_deref(), Some("leg_1"));
    for row in rows {
        assert_eq!(row.trustline_authorized, None);
        assert_eq!(row.trustline_frozen, None);
    }
}

#[test]
fn atomic_swap_pause_changes_governance_commitment_only_when_enabled() {
    let governance = GovernanceState::new(1);
    let mut base = Vec::new();
    append_governance_state(&mut base, &governance);
    let mut explicit_false = governance.clone();
    explicit_false.atomic_swap_paused = false;
    let mut unchanged = Vec::new();
    append_governance_state(&mut unchanged, &explicit_false);
    assert_eq!(base, unchanged);

    let mut paused = governance;
    paused.atomic_swap_paused = true;
    let mut changed = Vec::new();
    append_governance_state(&mut changed, &paused);
    assert_ne!(base, changed);
    assert!(String::from_utf8(changed)
        .expect("governance commitment text")
        .contains("governance.atomic_swap_paused"));
}

#[test]
fn atomic_swap_delta_journal_recovery_never_exposes_a_half_swap() {
    let seed_data_dir = unique_test_dir("postfiat-atomic-swap-journal-seed");
    init(InitOptions {
        data_dir: seed_data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init atomic journal seed node");
    let seed_store = NodeStore::new(&seed_data_dir);
    let mut genesis = seed_store.read_genesis().expect("read seed genesis");
    genesis.atomic_swap_activation_height = Some(2);
    seed_store
        .write_genesis(&genesis)
        .expect("activate atomic swaps in seed genesis");
    let mut genesis_tip = seed_store.read_chain_tip().expect("read genesis chain tip");
    genesis_tip.genesis_hash = genesis_hash(&genesis);
    genesis_tip.state_root =
        current_replicated_state_root(&seed_store, &genesis).expect("compute genesis state root");
    seed_store
        .write_chain_tip(&genesis_tip)
        .expect("write activated genesis chain tip");

    let faucet_key = read_transfer_key_file(&seed_data_dir, None).expect("read faucet key");
    let owner_0_key = ml_dsa_65_keygen().expect("owner 0 key");
    let owner_1_key = ml_dsa_65_keygen().expect("owner 1 key");
    let owner_0_file = atomic_node_dev_key_file(&owner_0_key);
    let owner_1_file = atomic_node_dev_key_file(&owner_1_key);
    let initial_ledger = seed_store.read_ledger().expect("read initial ledger");
    let funding_0 = build_signed_transfer_for_key(
        &genesis,
        &initial_ledger,
        &faucet_key,
        owner_0_file.address.clone(),
        ACCOUNT_RESERVE + 100_000,
        1,
    )
    .expect("build owner 0 funding");
    let funding_1 = build_signed_transfer_for_key(
        &genesis,
        &initial_ledger,
        &faucet_key,
        owner_1_file.address.clone(),
        ACCOUNT_RESERVE + 100_000,
        2,
    )
    .expect("build owner 1 funding");
    let mut setup_ledger = initial_ledger.clone();
    assert!(execute_transfer(&genesis, &mut setup_ledger, &funding_0).accepted);
    assert!(execute_transfer(&genesis, &mut setup_ledger, &funding_1).accepted);

    let create_a = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ASSET_CREATE_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: faucet_key.address.clone(),
            code: "JRA".to_string(),
            version: 1,
            precision: 0,
            display_name: "Journal Asset A".to_string(),
            max_supply: Some(1_000),
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut setup_ledger, &create_a, 1).accepted);
    let create_b = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ASSET_CREATE_TRANSACTION_KIND,
        4,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: faucet_key.address.clone(),
            code: "JRB".to_string(),
            version: 1,
            precision: 0,
            display_name: "Journal Asset B".to_string(),
            max_supply: Some(1_000),
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut setup_ledger, &create_b, 1).accepted);
    let mut assets = setup_ledger.asset_definitions.clone();
    assets.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    let asset_0 = assets[0].clone();
    let asset_1 = assets[1].clone();

    let issue_0 = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        5,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: faucet_key.address.clone(),
            to: owner_0_file.address.clone(),
            issuer: asset_0.issuer.clone(),
            asset_id: asset_0.asset_id.clone(),
            amount: 50,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut setup_ledger, &issue_0, 1).accepted);
    let issue_1 = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        6,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: faucet_key.address.clone(),
            to: owner_1_file.address.clone(),
            issuer: asset_1.issuer.clone(),
            asset_id: asset_1.asset_id.clone(),
            amount: 80,
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut setup_ledger, &issue_1, 1).accepted);

    let profile_operation = NavProfileRegisterOperation {
        registrant: faucet_key.address.clone(),
        verifier_kind: NAV_PROFILE_VERIFIER_PLACEHOLDER.to_string(),
        source_class: "atomic-journal-price-nav".to_string(),
        max_snapshot_age_blocks: 0,
        challenge_window_blocks: 0,
        max_epoch_gap_blocks: 0,
        settle_deadline_blocks: 0,
        min_challenge_bond: 0,
        min_attestations: 0,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash: String::new(),
        vault_bridge_route_policy_hash: String::new(),
        sp1_program_vkey: String::new(),
        sp1_proof_encoding: String::new(),
        max_proof_bytes: 0,
        max_public_values_bytes: 0,
    };
    let profile_id = NavProofProfile::new(
        profile_operation.registrant.clone(),
        profile_operation.verifier_kind.clone(),
        profile_operation.source_class.clone(),
        profile_operation.max_snapshot_age_blocks,
        profile_operation.challenge_window_blocks,
        profile_operation.max_epoch_gap_blocks,
        profile_operation.settle_deadline_blocks,
        profile_operation.min_challenge_bond,
        profile_operation.min_attestations,
        profile_operation.tolerance_bp,
        profile_operation.valuation_policy_hash.clone(),
        profile_operation.sp1_program_vkey.clone(),
        profile_operation.sp1_proof_encoding.clone(),
        profile_operation.max_proof_bytes,
        profile_operation.max_public_values_bytes,
    )
    .expect("derive journal price-NAV profile")
    .profile_id;
    let register_profile = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        7,
        AssetTransactionOperation::NavProfileRegister(profile_operation),
    );
    assert!(
        execute_asset_transaction(&genesis, &mut setup_ledger, &register_profile, 1).accepted
    );
    let register_nav_asset = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        8,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: faucet_key.address.clone(),
            asset_id: asset_0.asset_id.clone(),
            reserve_operator: faucet_key.address.clone(),
            proof_profile: profile_id.clone(),
            valuation_unit: "usd_e8".to_string(),
            redemption_account: faucet_key.address.clone(),
        }),
    );
    assert!(
        execute_asset_transaction(&genesis, &mut setup_ledger, &register_nav_asset, 1).accepted
    );
    let reserve_packet_hash = "a6".repeat(48);
    let submit_nav_reserve = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        9,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: faucet_key.address.clone(),
            submitter: faucet_key.address.clone(),
            asset_id: asset_0.asset_id.clone(),
            epoch: 1,
            nav_per_unit: 500_000_000,
            circulating_supply: 1_000_000,
            verified_net_assets: 500_000_000_000_000,
            proof_profile: profile_id,
            source_root: "01".repeat(48),
            attestor_root: "02".repeat(48),
            reserve_packet_hash: reserve_packet_hash.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        }),
    );
    assert!(
        execute_asset_transaction(&genesis, &mut setup_ledger, &submit_nav_reserve, 1).accepted
    );
    let finalize_nav = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        10,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: faucet_key.address.clone(),
            asset_id: asset_0.asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.clone(),
        }),
    );
    assert!(execute_asset_transaction(&genesis, &mut setup_ledger, &finalize_nav, 1).accepted);
    let market_policy = atomic_node_market_ops_policy(0x31);
    let register_market_policy = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND,
        11,
        AssetTransactionOperation::MarketOpsPolicyRegister(MarketOpsPolicyRegisterOperation {
            issuer: faucet_key.address.clone(),
            asset_id: asset_0.asset_id.clone(),
            policy: market_policy,
        }),
    );
    assert!(
        execute_asset_transaction(&genesis, &mut setup_ledger, &register_market_policy, 1).accepted
    );
    let market_finalize_operation = atomic_node_market_ops_operation(
        &faucet_key.address,
        &asset_0.asset_id,
        &reserve_packet_hash,
        0x31,
    );
    let market_envelope_hash = market_finalize_operation.envelope_hash.clone();
    let finalize_market = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        MARKET_OPS_FINALIZE_TRANSACTION_KIND,
        12,
        AssetTransactionOperation::MarketOpsFinalize(market_finalize_operation),
    );
    assert!(
        execute_asset_transaction(&genesis, &mut setup_ledger, &finalize_market, 1).accepted
    );

    let setup_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
        &mempool_batch_domain(&genesis),
        vec![funding_0, funding_1],
        Vec::new(),
        vec![
            create_a,
            create_b,
            issue_0,
            issue_1,
            register_profile,
            register_nav_asset,
            submit_nav_reserve,
            finalize_nav,
            register_market_policy,
            finalize_market,
        ],
    )
    .expect("build atomic journal setup batch")
    .batch;
    let setup_batch_file = seed_data_dir.join("atomic-journal-setup-batch.json");
    write_batch_file(&setup_batch_file, &setup_batch).expect("write atomic journal setup batch");
    let setup_receipts = apply_batch(ApplyBatchOptions {
        data_dir: seed_data_dir.clone(),
        batch_file: setup_batch_file,
        certificate_file: None,
    })
    .expect("commit atomic journal setup batch");
    assert_eq!(setup_receipts.len(), 12);
    assert!(setup_receipts.iter().all(|receipt| receipt.accepted));
    assert_eq!(
        seed_store.read_ledger().expect("read setup ledger"),
        setup_ledger
    );

    let unsigned = UnsignedAtomicSwapTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        rfq_hash: "11".repeat(48),
        market_envelope_hash,
        nav_epoch: 1,
        expires_at_height: 100,
        swap_nonce: "22".repeat(48),
        leg_0: AtomicSwapLeg {
            owner: owner_0_file.address.clone(),
            recipient: owner_1_file.address.clone(),
            issuer: asset_0.issuer.clone(),
            asset_id: asset_0.asset_id.clone(),
            amount: 50,
            sequence: 1,
            fee: 10_000,
        },
        leg_1: AtomicSwapLeg {
            owner: owner_1_file.address.clone(),
            recipient: owner_0_file.address.clone(),
            issuer: asset_1.issuer.clone(),
            asset_id: asset_1.asset_id.clone(),
            amount: 80,
            sequence: 1,
            fee: 10_000,
        },
    };
    let signing_bytes = unsigned.signing_bytes();
    let transaction = SignedAtomicSwapTransaction {
        authorization_0: AtomicSwapAuthorization {
            owner: owner_0_file.address.clone(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: owner_0_file.public_key_hex.clone(),
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign(&owner_0_key.private_key, &signing_bytes)
                    .expect("sign journal swap owner 0"),
            ),
        },
        authorization_1: AtomicSwapAuthorization {
            owner: owner_1_file.address.clone(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: owner_1_file.public_key_hex.clone(),
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign(&owner_1_key.private_key, &signing_bytes)
                    .expect("sign journal swap owner 1"),
            ),
        },
        unsigned,
    };
    let available = build_mixed_transaction_batch_with_atomic_swaps(
        &mempool_batch_domain(&genesis),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![transaction],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("build atomic journal swap batch");
    let batch = available.batch;
    let governance = seed_store.read_governance().expect("read setup governance");
    let mut post_ledger = setup_ledger.clone();
    let receipts = execute_transparent_batch(
        &genesis,
        &governance,
        &mut post_ledger,
        &batch,
        2,
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance),
    );
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted, "{receipts:?}");
    assert_eq!(
        receipts[0]
            .atomic_swap_legs
            .as_ref()
            .expect("accepted swap leg receipts")
            .len(),
        2
    );

    let pre_tip = seed_store.read_chain_tip().expect("read setup chain tip");
    let pre_receipts = seed_store.read_receipts().expect("read setup receipts");
    let pre_ordered = seed_store
        .read_ordered_batches()
        .expect("read setup ordered batches");
    let pre_archive = seed_store.read_batch_archive().expect("read setup archive");
    let pre_blocks = seed_store.read_blocks().expect("read setup blocks");
    let shielded = seed_store
        .read_shielded()
        .expect("read setup shielded state");
    let bridge = seed_store.read_bridge().expect("read setup bridge state");
    let validator_keys = read_validator_key_file(&seed_data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("read setup validator keys");
    let certificate_validators = active_validator_ids(&governance).expect("active validators");
    let commit = prepare_ordered_commit(OrderedCommitPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &post_ledger,
        ordered_batches: &pre_ordered,
        shielded: &shielded,
        bridge: &bridge,
        block_height: 2,
        parent_hash: pre_tip.block_hash.clone(),
        batch_kind: BATCH_KIND_TRANSPARENT,
        batch_id: &batch.batch_id,
        payload: &batch,
        batch_receipts: &receipts,
        archived_payload_json: None,
        validator_keys: Some(&validator_keys),
        external_certificate: None,
        external_validator_registry: None,
        external_certificate_preverified: false,
        historical_replay: None,
        certificate_validators: &certificate_validators,
        fastpay_pre_state_effects: &[],
    })
    .expect("prepare production atomic swap commit");
    let validator_registry = read_validator_registry_file(
        &seed_data_dir.join(VALIDATOR_REGISTRY_FILE),
    )
    .expect("read setup validator registry");
    let journal = ordered_commit_delta_journal(OrderedCommitWrite {
        ledger: Some(post_ledger.clone()),
        governance: Some(governance.clone()),
        shielded: Some(shielded.clone()),
        bridge: Some(bridge.clone()),
        commit,
        validator_registry: Some(validator_registry.clone()),
    })
    .expect("build production atomic swap delta journal");
    assert_eq!(journal.block.header.height, 2);
    assert!(!journal.block.header.block_hash.is_empty());
    assert!(!journal.block.header.state_root.is_empty());
    assert!(!journal.block.header.certificate_id.is_empty());
    assert_eq!(
        journal.block.header.certificate.validators,
        certificate_validators
    );
    assert_eq!(journal.block.header.certificate.quorum, 1);
    assert_eq!(journal.block.header.certificate.votes.len(), 1);
    assert!(!journal.block.header.certificate.registry_root.is_empty());
    assert_eq!(
        serde_json::from_str::<TransactionBatch>(&journal.archive_entry.payload_json)
            .expect("parse production archive payload"),
        batch
    );

    let mut expected_receipts = pre_receipts;
    expected_receipts.extend(journal.receipt_delta.clone());
    let mut expected_ordered = pre_ordered;
    expected_ordered.push(journal.ordered_batch_id.clone());
    let mut expected_archive = pre_archive;
    expected_archive.batches.push(journal.archive_entry.clone());
    let mut expected_blocks = pre_blocks;
    expected_blocks.blocks.push(journal.block.clone());
    let expected_tip = chain_tip_after_delta(&pre_tip, &journal).expect("expected recovered tip");

    for write_prefix in 0..=10 {
        let data_dir = unique_test_dir(&format!(
            "postfiat-atomic-swap-journal-recovery-{write_prefix}"
        ));
        copy_atomic_node_test_dir(&seed_data_dir, &data_dir);
        let store = NodeStore::new(&data_dir);
        store
            .write_ordered_commit_journal(&journal)
            .expect("write atomic delta journal");
        if write_prefix >= 1 {
            store
                .write_ledger(&post_ledger)
                .expect("write prefix ledger");
        }
        if write_prefix >= 2 {
            store
                .write_governance(&governance)
                .expect("write prefix governance");
        }
        if write_prefix >= 3 {
            store
                .write_shielded(&shielded)
                .expect("write prefix shielded state");
        }
        if write_prefix >= 4 {
            store
                .write_bridge(&bridge)
                .expect("write prefix bridge state");
        }
        if write_prefix >= 5 {
            for receipt in &journal.receipt_delta {
                store
                    .append_receipt_record(receipt)
                    .expect("append prefix receipt");
            }
        }
        if write_prefix >= 6 {
            store
                .append_ordered_batch_record(&journal.ordered_batch_id)
                .expect("append prefix ordered batch");
        }
        if write_prefix >= 7 {
            store
                .append_batch_archive_entry(journal.archive_entry.clone())
                .expect("append prefix archive entry");
        }
        if write_prefix >= 8 {
            store
                .append_block_record(&journal.block)
                .expect("append prefix block");
        }
        if write_prefix >= 9 {
            store
                .write_chain_tip(&expected_tip)
                .expect("write prefix chain tip");
        }
        if write_prefix >= 10 {
            write_validator_registry_file(
                &data_dir.join(VALIDATOR_REGISTRY_FILE),
                &validator_registry,
            )
            .expect("write prefix validator registry");
        }

        status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("recover atomic delta journal prefix");
        let recovered_ledger = store.read_ledger().expect("post-recovery ledger");
        assert_eq!(recovered_ledger, post_ledger);
        assert_eq!(
            store.read_governance().expect("post-recovery governance"),
            governance
        );
        assert_eq!(
            store.read_shielded().expect("post-recovery shielded state"),
            shielded
        );
        assert_eq!(store.read_bridge().expect("post-recovery bridge"), bridge);
        assert_eq!(
            read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
                .expect("post-recovery validator registry"),
            validator_registry
        );
        assert_eq!(
            recovered_ledger
                .account(&owner_0_file.address)
                .expect("recovered owner 0")
                .sequence,
            1
        );
        assert_eq!(
            recovered_ledger
                .account(&owner_1_file.address)
                .expect("recovered owner 1")
                .sequence,
            1
        );
        assert_eq!(
            store.read_receipts().expect("post-recovery receipts"),
            expected_receipts
        );
        assert_eq!(
            store
                .read_ordered_batches()
                .expect("post-recovery ordered batches"),
            expected_ordered
        );
        assert_eq!(
            store.read_batch_archive().expect("post-recovery archive"),
            expected_archive
        );
        assert_eq!(
            store.read_blocks().expect("post-recovery blocks"),
            expected_blocks
        );
        assert_eq!(
            store.read_chain_tip().expect("post-recovery chain tip"),
            expected_tip
        );
        assert!(store
            .read_ordered_commit_journal::<OrderedCommitDeltaJournal>()
            .expect("post-recovery journal")
            .is_none());
        let verification = verify_state(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify recovered atomic swap state");
        assert!(verification.verified, "{verification:?}");
        assert_eq!(verification.block_log.block_count, 2);
        assert_eq!(
            verification.block_log.tip_hash,
            journal.block.header.block_hash
        );
        assert_eq!(
            verification.block_log.state_root,
            journal.block.header.state_root
        );
        fs::remove_dir_all(data_dir).expect("remove atomic journal directory");
    }
    fs::remove_dir_all(seed_data_dir).expect("remove atomic journal seed directory");
}
