// Synthetic ledger fixtures exercise signed public redemption admission and
// accounting. They do not establish reserve-proof or release qualification.
fn burn6_redeem_fixture(
    source_mode: bool,
) -> (
    Genesis,
    LedgerState,
    MlDsa65KeyPair,
    PftlUniswapPrimaryRedeemOperation,
) {
    let genesis = Genesis::new("burn6-swap-settlement");
    let key = ml_dsa_65_keygen_from_seed(&[0x63; 32]);
    let owner = address_from_public_key(&key.public_key);
    let issuer = "burn6-issuer".to_string();
    let native = AssetDefinition::new(&genesis.chain_id, &issuer, "NAV", 1, 6).unwrap();
    let family = AssetDefinition::new(&genesis.chain_id, &issuer, "USD", 1, 6).unwrap();
    let mut ledger = LedgerState::new(vec![
        Account::new(owner.clone(), 100_000, Some(bytes_to_hex(&key.public_key))),
        Account::new(issuer.clone(), 100_000, None),
    ]);
    ledger
        .asset_definitions
        .extend([native.clone(), family.clone()]);
    let mut nav = NavTrackedAsset::new(
        native.asset_id.clone(),
        issuer.clone(),
        issuer.clone(),
        "burn6-fixture",
        "USDC",
        issuer.clone(),
    )
    .unwrap();
    nav.finalized_epoch = 7;
    nav.finalized_at_height = 10;
    nav.finalized_reserve_packet_hash = "55".repeat(48);
    nav.nav_per_unit = 7_000_000;
    nav.circulating_supply = 2_000_000;
    ledger.nav_assets.push(nav);
    ledger.nav_assets.push(
        NavTrackedAsset::new(
            family.asset_id.clone(),
            issuer.clone(),
            issuer.clone(),
            "burn6-fixture",
            "USDC",
            issuer.clone(),
        )
        .unwrap(),
    );
    let mut native_line = TrustLine::new(
        owner.clone(),
        issuer.clone(),
        native.asset_id.clone(),
        100_000_000,
        0,
    )
    .unwrap();
    native_line.balance = 2_000_000;
    ledger.trustlines.push(native_line);

    let route_id = "burn6-redemption".to_string();
    let source_id = if source_mode {
        let domain = "erc20_bridge_vault:1:0x1111111111111111111111111111111111111111:0x2222222222222222222222222222222222222222";
        let mut bucket =
            VaultBridgeBucketState::new(family.asset_id.clone(), domain, "11".repeat(48), 10)
                .unwrap();
        bucket.gross_receipt_atoms = 20_000_000;
        bucket.counted_value_atoms = 20_000_000;
        bucket.outstanding_vault_bridge_atoms = 20_000_000;
        bucket.validate().unwrap();
        let id = postfiat_types::pfusdc_source_series_id(
            &genesis.chain_id,
            &family.asset_id,
            1,
            "0x1111111111111111111111111111111111111111",
            "0x2222222222222222222222222222222222222222",
            1,
            &bucket.policy_hash,
        )
        .unwrap();
        ledger.asset_definitions.push(
            AssetDefinition::new_source_series(&family, &id, &bucket.bucket_id, "Burn 6 source")
                .unwrap(),
        );
        ledger.vault_bridge_bucket_states.push(bucket);
        ledger
            .pftl_uniswap_source_custody
            .push(postfiat_types::PftlUniswapSourceCustody {
                route_id: route_id.clone(),
                asset_id: id.clone(),
                enabled_for_issue: false,
                principal_atoms: 14_000_000,
                spread_atoms: 70_000,
                reservation_escrows: Default::default(),
            });
        Some(id)
    } else {
        None
    };
    let mut settlement_line = TrustLine::new(
        owner.clone(),
        issuer,
        source_id.clone().unwrap_or_else(|| family.asset_id.clone()),
        100_000_000,
        0,
    )
    .unwrap();
    settlement_line.balance = 5_930_000;
    ledger.trustlines.push(settlement_line);
    let mut policy = PftlUniswapPrimaryMarketPolicyV2 {
        policy_hash: String::new(),
        policy_epoch: 1,
        issue_multiplier_bps: PFTL_UNISWAP_A666_ISSUE_MULTIPLIER_BPS,
        redeem_multiplier_bps: PFTL_UNISWAP_A666_REDEEM_MULTIPLIER_BPS,
        issue_capacity_atoms: 100_000_000,
        redeem_capacity_atoms: 100_000_000,
        max_order_atoms: 10_000_000,
        min_order_atoms: 1_000_000,
        valid_from_height: 1,
        expires_at_height: 100,
        max_nav_age_blocks: 5,
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: "55".repeat(48),
    };
    policy.policy_hash = policy.computed_hash();
    let operation = PftlUniswapPrimaryRedeemOperation {
        settlement_source_asset_id: source_id,
        owner: owner.clone(),
        settlement_recipient: owner.clone(),
        route_id: route_id.clone(),
        redemption_nonce: "66".repeat(32),
        nav_amount_atoms: 1_000_000,
        min_settlement_value_atoms: 6_996_500,
        route_epoch: 1,
        policy_epoch: 1,
        policy_hash: policy.policy_hash.clone(),
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: policy.pricing_reserve_packet_hash.clone(),
        expires_at_height: 100,
    };
    let route = PftlUniswapConsensusRouteState {
        route_id,
        route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
        route_config_digest: "77".repeat(48),
        route_trust_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
        native_nav_asset_id: native.asset_id,
        settlement_asset_id: family.asset_id,
        handoff_controller: "0x1111111111111111111111111111111111111111".to_string(),
        settlement_adapter: "0x2222222222222222222222222222222222222222".to_string(),
        wrapped_navcoin_token: "0x3333333333333333333333333333333333333333".to_string(),
        ethereum_chain_id: 1,
        route_supply_cap_atoms: 100_000_000,
        packet_notional_cap_atoms: 10_000_000,
        latest_finalized_nav_epoch: 7,
        return_finality_blocks: 12,
        live_value_enabled: false,
        ethereum_verification_policy: None,
        authorized_valid_supply_atoms: 2_000_000,
        pftl_spendable_supply_atoms: 2_000_000,
        native_spendable_balances_atoms: std::collections::BTreeMap::from([(owner, 2_000_000)]),
        ethereum_spendable_supply_atoms: 0,
        other_registered_venue_supply_atoms: 0,
        outstanding_bridge_claims_atoms: 0,
        pending_return_import_claims_atoms: 0,
        settlement_reserve_atoms: 14_000_000,
        primary_subscription_nonces: Default::default(),
        export_packets: Default::default(),
        export_nonces: Default::default(),
        return_imports: Default::default(),
        paused: true,
        v2: Some(PftlUniswapRouteV2State {
            route_schema_version: PFTL_UNISWAP_ROUTE_SCHEMA_V2,
            route_epoch: 1,
            outbound_verification_class: PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY.to_string(),
            return_verification_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
            primary_market_policy: policy,
            issue_capacity_used_atoms: 2_000_000,
            redeem_capacity_used_atoms: 0,
            non_nav_spread_atoms: 70_000,
            active_reservations: Default::default(),
            export_entitlements: Default::default(),
            terminal_reservations: Default::default(),
            redemption_nonces: Default::default(),
        }),
    };
    route.validate().unwrap();
    ledger.pftl_uniswap_routes.push(route);
    ledger.validate_pftl_source_custody().unwrap();
    (genesis, ledger, key, operation)
}

fn burn6_submit_redeem(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    key: &MlDsa65KeyPair,
    operation: PftlUniswapPrimaryRedeemOperation,
    height: u64,
) -> Receipt {
    let transaction = signed_asset_transaction_with_minimum_fee(
        genesis,
        ledger,
        key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        ledger.account(&operation.owner).unwrap().sequence + 1,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(operation),
    );
    execute_asset_transaction_with_unverified_pftl_uniswap_fixture(
        genesis,
        ledger,
        &transaction,
        height,
    )
}

#[test]
fn burn6_public_redemption_rejects_policy_stale_nav_without_mutation() {
    for source_mode in [false, true] {
        let (genesis, mut ledger, key, operation) = burn6_redeem_fixture(source_mode);
        let before = ledger.clone();
        let receipt = burn6_submit_redeem(&genesis, &mut ledger, &key, operation, 16);
        assert!(!receipt.accepted, "source_mode={source_mode}: {receipt:?}");
        assert_eq!(receipt.code, "stale_pftl_uniswap_policy_pricing");
        assert_eq!(
            ledger, before,
            "stale rejection must preserve the entire ledger"
        );
    }
}

#[test]
fn burn6_public_redemption_accepts_last_fresh_height_and_preserves_conservation() {
    for source_mode in [false, true] {
        let (genesis, mut ledger, key, operation) = burn6_redeem_fixture(source_mode);
        let family = ledger.pftl_uniswap_routes[0].settlement_asset_id.clone();
        let supply_before = issued_asset_family_supply(&ledger, &family).unwrap();
        let receipt = burn6_submit_redeem(&genesis, &mut ledger, &key, operation.clone(), 15);
        assert!(receipt.accepted, "source_mode={source_mode}: {receipt:?}");
        assert_eq!(ledger.nav_assets[0].circulating_supply, 1_000_000);
        let route = &ledger.pftl_uniswap_routes[0];
        assert_eq!(route.authorized_valid_supply_atoms, 1_000_000);
        assert_eq!(route.pftl_spendable_supply_atoms, 1_000_000);
        assert_eq!(route.settlement_reserve_atoms, 7_000_000);
        assert_eq!(route.v2.as_ref().unwrap().non_nav_spread_atoms, 73_500);
        assert_eq!(
            issued_asset_family_supply(&ledger, &family).unwrap(),
            supply_before
        );
        assert_eq!(ledger.trustlines[1].balance, 5_930_000 + 6_996_500);
        ledger.validate_pftl_source_custody().unwrap();
        if source_mode {
            assert_eq!(
                ledger.pftl_uniswap_source_custody[0].principal_atoms,
                7_000_000
            );
            assert_eq!(ledger.pftl_uniswap_source_custody[0].spread_atoms, 73_500);
        }
        let before_replay = ledger.clone();
        let replay = burn6_submit_redeem(&genesis, &mut ledger, &key, operation, 15);
        assert!(!replay.accepted);
        assert_eq!(replay.code, "pftl_uniswap_redemption_policy_mismatch");
        assert_eq!(ledger, before_replay);
    }
}
