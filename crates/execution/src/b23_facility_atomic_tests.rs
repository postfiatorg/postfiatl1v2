use super::*;
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

struct AtomicExecutionFixture {
    genesis: Genesis,
    ledger: LedgerState,
    owner_0_key: MlDsa65KeyPair,
    owner_1_key: MlDsa65KeyPair,
    transaction: SignedAtomicSwapTransaction,
}

fn atomic_test_line(account: &str, asset: &AssetDefinition, balance: u64) -> TrustLine {
    let mut line = TrustLine::new(
        account,
        &asset.issuer,
        &asset.asset_id,
        1_000_000,
        TRUSTLINE_STATE_EXPANSION_FEE,
    )
    .expect("atomic test trustline");
    line.authorized = true;
    line.balance = balance;
    line
}

fn sign_atomic_swap(
    mut unsigned: postfiat_types::UnsignedAtomicSwapTransaction,
    owner_0_key: &MlDsa65KeyPair,
    owner_1_key: &MlDsa65KeyPair,
) -> SignedAtomicSwapTransaction {
    let owner_0 = address_from_public_key(&owner_0_key.public_key);
    let owner_1 = address_from_public_key(&owner_1_key.public_key);
    unsigned.leg_0.owner = owner_0.clone();
    unsigned.leg_0.recipient = owner_1.clone();
    unsigned.leg_1.owner = owner_1.clone();
    unsigned.leg_1.recipient = owner_0.clone();

    let signing_bytes = unsigned.signing_bytes();
    SignedAtomicSwapTransaction {
        unsigned,
        authorization_0: AtomicSwapAuthorization {
            owner: owner_0,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_0_key.public_key),
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign(&owner_0_key.private_key, &signing_bytes)
                    .expect("final sign atomic owner 0"),
            ),
        },
        authorization_1: AtomicSwapAuthorization {
            owner: owner_1,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_1_key.public_key),
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign(&owner_1_key.private_key, &signing_bytes)
                    .expect("final sign atomic owner 1"),
            ),
        },
    }
}

fn resign_atomic_swap(
    transaction: &mut SignedAtomicSwapTransaction,
    owner_0_key: &MlDsa65KeyPair,
    owner_1_key: &MlDsa65KeyPair,
) {
    let signing_bytes = transaction.unsigned.signing_bytes();
    transaction.authorization_0.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign(&owner_0_key.private_key, &signing_bytes).expect("resign atomic owner 0"),
    );
    transaction.authorization_1.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign(&owner_1_key.private_key, &signing_bytes).expect("resign atomic owner 1"),
    );
}

fn atomic_market_envelope_record(
    leg: &AtomicSwapLeg,
    epoch: u64,
    reserve_packet_hash: &str,
) -> FinalizedMarketOpsEnvelope {
    let mut operation =
        market_ops_operation_fixture(&leg.issuer, &leg.asset_id, reserve_packet_hash);
    operation.envelope.epoch = epoch;
    operation.envelope.supply_packet_hash = market_ops_supply_packet_hash(
        &leg.asset_id,
        epoch,
        operation.envelope.valid_global_supply_atoms,
    )
    .expect("atomic market-ops supply packet hash");
    operation.envelope_hash = bytes_to_hex(&operation.envelope.envelope_hash());
    FinalizedMarketOpsEnvelope {
        asset_id: leg.asset_id.clone(),
        epoch,
        envelope_hash: operation.envelope_hash,
        envelope: operation.envelope,
        policy_inputs: Some(operation.policy_inputs),
        finalized_at_height: 9,
    }
}

fn configure_pf_usdc_a651_market_binding(
    fixture: &mut AtomicExecutionFixture,
    a651_leg_index: usize,
) {
    let (a651_leg, pf_usdc_leg) = match a651_leg_index {
        0 => (
            fixture.transaction.unsigned.leg_0.clone(),
            fixture.transaction.unsigned.leg_1.clone(),
        ),
        1 => (
            fixture.transaction.unsigned.leg_1.clone(),
            fixture.transaction.unsigned.leg_0.clone(),
        ),
        _ => panic!("atomic fixture price-NAV leg index must be 0 or 1"),
    };
    let pf_usdc = NavTrackedAsset::new(
        pf_usdc_leg.asset_id.clone(),
        pf_usdc_leg.issuer.clone(),
        pf_usdc_leg.issuer.clone(),
        "pfusdc-bridge-accounting-profile",
        "usd_e8",
        pf_usdc_leg.issuer.clone(),
    )
    .expect("pfUSDC bridge-accounting NAV record");
    let mut a651 = NavTrackedAsset::new(
        a651_leg.asset_id.clone(),
        a651_leg.issuer.clone(),
        a651_leg.issuer.clone(),
        "a651-price-nav-profile",
        "usd_e8",
        a651_leg.issuer.clone(),
    )
    .expect("a651 price-NAV record");
    a651.finalized_epoch = 1;
    let envelope = atomic_market_envelope_record(&a651_leg, 1, &"ab".repeat(48));

    fixture.ledger.nav_assets = vec![pf_usdc, a651];
    fixture.ledger.market_ops_policies = vec![market_ops_policy_fixture()];
    fixture.ledger.market_ops_envelopes = vec![envelope.clone()];
    fixture.transaction.unsigned.nav_epoch = envelope.epoch;
    fixture.transaction.unsigned.market_envelope_hash = envelope.envelope_hash;
    resign_atomic_swap(
        &mut fixture.transaction,
        &fixture.owner_0_key,
        &fixture.owner_1_key,
    );
}

fn atomic_execution_fixture() -> AtomicExecutionFixture {
    let genesis = Genesis::new("postfiat-local");
    let owner_0_key = ml_dsa_65_keygen().expect("owner 0 keygen");
    let owner_1_key = ml_dsa_65_keygen().expect("owner 1 keygen");
    let issuer_a_key = ml_dsa_65_keygen().expect("issuer a keygen");
    let issuer_b_key = ml_dsa_65_keygen().expect("issuer b keygen");
    let owner_0 = address_from_public_key(&owner_0_key.public_key);
    let owner_1 = address_from_public_key(&owner_1_key.public_key);
    let issuer_a = address_from_public_key(&issuer_a_key.public_key);
    let issuer_b = address_from_public_key(&issuer_b_key.public_key);
    let asset_a =
        AssetDefinition::new(&genesis.chain_id, &issuer_a, "ASWAPA", 1, 6).expect("asset a");
    let asset_b =
        AssetDefinition::new(&genesis.chain_id, &issuer_b, "ASWAPB", 1, 6).expect("asset b");
    let (asset_0, asset_1) = if asset_a.asset_id < asset_b.asset_id {
        (asset_a, asset_b)
    } else {
        (asset_b, asset_a)
    };
    let mut ledger = LedgerState::new(vec![
        Account::new(
            owner_0.clone(),
            10_000,
            Some(bytes_to_hex(&owner_0_key.public_key)),
        ),
        Account::new(
            owner_1.clone(),
            10_000,
            Some(bytes_to_hex(&owner_1_key.public_key)),
        ),
        Account::new(
            asset_0.issuer.clone(),
            10_000,
            Some(bytes_to_hex(if asset_0.issuer == issuer_a {
                &issuer_a_key.public_key
            } else {
                &issuer_b_key.public_key
            })),
        ),
        Account::new(
            asset_1.issuer.clone(),
            10_000,
            Some(bytes_to_hex(if asset_1.issuer == issuer_a {
                &issuer_a_key.public_key
            } else {
                &issuer_b_key.public_key
            })),
        ),
    ]);
    ledger.asset_definitions = vec![asset_0.clone(), asset_1.clone()];
    ledger.trustlines = vec![
        atomic_test_line(&owner_0, &asset_0, 20_000),
        atomic_test_line(&owner_1, &asset_1, 164_020),
    ];
    let mut fee_0 = MIN_TRANSFER_FEE;
    let mut fee_1 = MIN_TRANSFER_FEE;
    let mut transaction = None;
    for _ in 0..8 {
        let unsigned = postfiat_types::UnsignedAtomicSwapTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            rfq_hash: "11".repeat(48),
            market_envelope_hash: "00".repeat(48),
            nav_epoch: 0,
            expires_at_height: 100,
            swap_nonce: "33".repeat(48),
            leg_0: AtomicSwapLeg {
                owner: owner_0.clone(),
                recipient: owner_1.clone(),
                issuer: asset_0.issuer.clone(),
                asset_id: asset_0.asset_id.clone(),
                amount: 20_000,
                sequence: 1,
                fee: fee_0,
            },
            leg_1: AtomicSwapLeg {
                owner: owner_1.clone(),
                recipient: owner_0.clone(),
                issuer: asset_1.issuer.clone(),
                asset_id: asset_1.asset_id.clone(),
                amount: 164_020,
                sequence: 1,
                fee: fee_1,
            },
        };
        let candidate = sign_atomic_swap(unsigned, &owner_0_key, &owner_1_key);
        let minimum_0 =
            minimum_atomic_swap_leg_fee_for_ledger(&ledger, &candidate, &candidate.unsigned.leg_0);
        let minimum_1 =
            minimum_atomic_swap_leg_fee_for_ledger(&ledger, &candidate, &candidate.unsigned.leg_1);
        transaction = Some(candidate);
        if fee_0 >= minimum_0 && fee_1 >= minimum_1 {
            break;
        }
        fee_0 = minimum_0;
        fee_1 = minimum_1;
    }
    let mut fixture = AtomicExecutionFixture {
        genesis,
        ledger,
        owner_0_key,
        owner_1_key,
        transaction: transaction.expect("atomic transaction converged"),
    };
    configure_pf_usdc_a651_market_binding(&mut fixture, 1);
    fixture
}

fn execute_active_atomic_swap(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAtomicSwapTransaction,
    block_height: u64,
) -> Receipt {
    execute_atomic_swap_transaction_with_compatibility(
        genesis,
        ledger,
        transaction,
        block_height,
        AssetExecutionCompatibility::strict(),
    )
}

#[test]
fn b23_rev3_atomic_swap_moves_inventory_and_recipient_redeems() {
    let mut fixture = atomic_execution_fixture();
    let leg = fixture.transaction.unsigned.leg_1.clone();
    let settlement = fixture.transaction.unsigned.leg_0.clone();
    let (_, original, _, _) = facility_fixture(false);
    let mut route = original.pftl_uniswap_routes[0].clone();
    route.native_nav_asset_id = leg.asset_id.clone();
    route.settlement_asset_id = settlement.asset_id.clone();
    route.authorized_valid_supply_atoms = leg.amount;
    route.pftl_spendable_supply_atoms = leg.amount;
    route.native_spendable_balances_atoms =
        std::collections::BTreeMap::from([(leg.owner.clone(), leg.amount)]);
    route.settlement_reserve_atoms = 1_000_000;
    let policy = &mut route.v2.as_mut().unwrap().primary_market_policy;
    policy.spread_recipient = Some(leg.owner.clone());
    policy.min_order_atoms = 1;
    policy.pricing_nav_epoch = 1;
    policy.policy_hash = policy.computed_hash();
    fixture.ledger.pftl_uniswap_routes.push(route.clone());
    add_dormant_route(&mut fixture.ledger);
    let dormant = fixture.ledger.pftl_uniswap_routes[1].clone();
    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut fixture.ledger,
        &fixture.transaction,
        10,
    );
    assert!(receipt.accepted, "atomic swap: {receipt:?}");
    assert_eq!(fixture.ledger.pftl_uniswap_routes[1], dormant);
    assert_eq!(
        fixture.ledger.pftl_uniswap_routes[0].native_spendable_balances_atoms,
        std::collections::BTreeMap::from([(leg.recipient.clone(), leg.amount)])
    );
    let nav = fixture.ledger.nav_asset_mut(&leg.asset_id).unwrap();
    nav.nav_per_unit = 1_000_000;
    nav.circulating_supply = leg.amount;
    nav.finalized_at_height = 10;
    nav.finalized_reserve_packet_hash = "55".repeat(48);
    let policy = &fixture.ledger.pftl_uniswap_routes[0]
        .v2
        .as_ref()
        .unwrap()
        .primary_market_policy;
    let redeem = PftlUniswapPrimaryRedeemOperation {
        settlement_source_asset_id: None,
        owner: leg.recipient.clone(),
        settlement_recipient: leg.recipient,
        route_id: route.route_id,
        redemption_nonce: "66".repeat(32),
        nav_amount_atoms: leg.amount,
        min_settlement_value_atoms: 1,
        route_epoch: 1,
        policy_epoch: 1,
        policy_hash: policy.policy_hash.clone(),
        pricing_nav_epoch: 1,
        pricing_reserve_packet_hash: "55".repeat(48),
        expires_at_height: 100,
    };
    let receipt = submit_asset(
        &fixture.genesis,
        &mut fixture.ledger,
        &fixture.owner_0_key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem),
    );
    assert!(receipt.accepted, "atomic recipient redeem: {receipt:?}");
    fixture.ledger.pftl_uniswap_routes[0].validate().unwrap();
}
