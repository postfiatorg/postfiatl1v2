use super::*;
use postfiat_crypto_provider::*;
use postfiat_types::*;

// Synthetic ledger fixtures exercise signed public redemption admission and
// accounting. They do not establish reserve-proof or release qualification.
fn facility_fixture(
    source_mode: bool,
) -> (
    Genesis,
    LedgerState,
    MlDsa65KeyPair,
    PftlUniswapPrimaryRedeemOperation,
) {
    facility_fixture_for_issuer(source_mode, "burn6-issuer")
}

fn facility_fixture_for_issuer(
    source_mode: bool,
    fixture_issuer: &str,
) -> (
    Genesis,
    LedgerState,
    MlDsa65KeyPair,
    PftlUniswapPrimaryRedeemOperation,
) {
    let genesis = Genesis::new("burn6-swap-settlement");
    let key = ml_dsa_65_keygen_from_seed(&[0x63; 32]);
    let owner = address_from_public_key(&key.public_key);
    let issuer = fixture_issuer.to_string();
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
        spread_recipient: None,
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

fn submit_asset(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    key: &MlDsa65KeyPair,
    kind: &str,
    operation: AssetTransactionOperation,
) -> Receipt {
    let owner = address_from_public_key(&key.public_key);
    let tx = signed_asset_transaction_with_minimum_fee(
        genesis,
        ledger,
        key,
        kind,
        ledger.account(&owner).unwrap().sequence + 1,
        operation,
    );
    execute_asset_transaction_with_unverified_pftl_uniswap_fixture(genesis, ledger, &tx, 11)
}

fn market_buy_then_redeem(partial: bool) {
    let (genesis, mut ledger, alice_key, mut redeem) = facility_fixture(false);
    let alice = redeem.owner.clone();
    enable_facility(&mut ledger, &mut redeem, &alice);

    let bob_key = ml_dsa_65_keygen_from_seed(&[0x64; 32]);
    let bob = address_from_public_key(&bob_key.public_key);
    ledger.accounts.push(Account::new(
        bob.clone(),
        100_000,
        Some(bytes_to_hex(&bob_key.public_key)),
    ));
    let nav = ledger.pftl_uniswap_routes[0].native_nav_asset_id.clone();
    let usd = ledger.pftl_uniswap_routes[0].settlement_asset_id.clone();
    for (asset, balance) in [(&nav, 0), (&usd, 20_000_000)] {
        let issuer = ledger.asset_definition(asset).unwrap().issuer.clone();
        let mut line = TrustLine::new(bob.clone(), issuer, asset.clone(), 100_000_000, 0).unwrap();
        line.balance = balance;
        ledger.trustlines.push(line);
    }
    let before_supply = ledger.nav_assets[0].circulating_supply;
    let before_reserve = ledger.pftl_uniswap_routes[0].settlement_reserve_atoms;
    let sell_amount = if partial { 2_000_000 } else { 1_000_000 };
    let tx = signed_offer_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &alice_key,
        OFFER_CREATE_TRANSACTION_KIND,
        1,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: alice.clone(),
            taker_gets_asset_id: nav.clone(),
            taker_gets_amount: sell_amount,
            taker_pays_asset_id: usd.clone(),
            taker_pays_amount: sell_amount * 7,
            expiration_height: 50,
        }),
        11,
    );
    let receipt = execute_offer_transaction(&genesis, &mut ledger, &tx, 11);
    assert!(receipt.accepted, "Alice posts: {receipt:?}");
    let tx = signed_offer_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &bob_key,
        OFFER_CREATE_TRANSACTION_KIND,
        1,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: bob.clone(),
            taker_gets_asset_id: usd.clone(),
            taker_gets_amount: 7_000_000,
            taker_pays_asset_id: nav.clone(),
            taker_pays_amount: 1_000_000,
            expiration_height: 50,
        }),
        11,
    );
    let receipt = execute_offer_transaction(&genesis, &mut ledger, &tx, 11);
    assert!(receipt.accepted, "Bob buys: {receipt:?}");
    assert_eq!(receipt.offer_fills.len(), 1);
    assert_eq!(
        ledger
            .trustline_for_account_asset(&bob, &nav)
            .unwrap()
            .balance,
        1_000_000
    );
    assert_eq!(ledger.nav_assets[0].circulating_supply, before_supply);
    assert_eq!(
        ledger.pftl_uniswap_routes[0].settlement_reserve_atoms,
        before_reserve
    );
    if partial {
        let tx = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &alice_key,
            OFFER_CANCEL_TRANSACTION_KIND,
            2,
            OfferTransactionOperation::OfferCancel(OfferCancelOperation {
                owner: alice.clone(),
                offer_id: offer_id(&genesis.chain_id, &alice, 1).unwrap(),
            }),
            11,
        );
        let receipt = execute_offer_transaction(&genesis, &mut ledger, &tx, 11);
        assert!(receipt.accepted, "Alice cancels residual: {receipt:?}");
        assert_eq!(
            ledger
                .trustline_for_account_asset(&alice, &nav)
                .unwrap()
                .balance,
            1_000_000
        );
    }
    redeem.owner = bob.clone();
    redeem.settlement_recipient = bob.clone();
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &bob_key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem),
    );
    assert!(
        receipt.accepted,
        "Fresh Bob must redeem purchased NAV: {receipt:?}"
    );
    assert_eq!(
        ledger
            .trustline_for_account_asset(&bob, &nav)
            .unwrap()
            .balance,
        0
    );
    assert_eq!(
        ledger.nav_assets[0].circulating_supply,
        before_supply - 1_000_000
    );
    assert_eq!(
        ledger.pftl_uniswap_routes[0].settlement_reserve_atoms,
        before_reserve - 7_000_000
    );
    assert_eq!(
        ledger.pftl_uniswap_routes[0]
            .native_spendable_balances_atoms
            .get(&alice),
        Some(&1_000_000)
    );
    assert!(!ledger.pftl_uniswap_routes[0]
        .native_spendable_balances_atoms
        .contains_key(&bob));
}

#[test]
fn b23_market_buyer_redeems() {
    market_buy_then_redeem(false);
}
#[test]
fn b23_partial_fill_cancel_buyer_redeems() {
    market_buy_then_redeem(true);
}

#[test]
fn b23_legacy_hash_baseline() {
    let (_, mut ledger, _, _) = facility_fixture(false);
    let route = &ledger.pftl_uniswap_routes[0];
    let policy = &route.v2.as_ref().unwrap().primary_market_policy;
    assert_eq!(policy.computed_hash(), "d605f00ac76759780669cea1d35a99f3c31a18a318dbb2a550a1dc8353d4c980d9f3bb2492f8eb0fa8c30608de48f905");
    assert!(serde_json::to_value(policy)
        .unwrap()
        .get("spread_recipient")
        .is_none());
    assert_eq!(pftl_uniswap_route_state_hash(route), "c889ab772a676dc93ce26c3cfd2198e3769fefbeeb6624685730e3944d537c8fee6b1ff047038559e976c903b3a2a411");
    let route_id = route.route_id.clone();
    let route_hash = pftl_uniswap_route_state_hash(route);
    append_pftl_uniswap_consensus_receipt(
        &mut ledger,
        PftlUniswapReceiptPlan {
            transition: "primary_redeemed",
            route_id: &route_id,
            state_before_hash: route_hash.clone(),
            state_after_hash: route_hash,
            packet_hash: None,
            burn_event_hash: None,
            wallet: None,
            amount_atoms: Some(1_000_000),
            block_height: 11,
        },
    )
    .unwrap();
    let receipt = &ledger.pftl_uniswap_receipts[0];
    assert_eq!(receipt.receipt_hash, "51ba4154deae52947c122240255bd24b27aaa48503922cbc7eec4fa4f1e37d60cc4c6bed518e594d78245124fc504090");
    assert_eq!(
        receipt.receipt_hash,
        postfiat_types::pftl_uniswap_consensus_receipt_computed_hash(receipt)
    );
    assert!(serde_json::to_value(receipt)
        .unwrap()
        .get("redemption_settlement")
        .is_none());
}

fn enable_facility(
    ledger: &mut LedgerState,
    redeem: &mut PftlUniswapPrimaryRedeemOperation,
    management: &str,
) {
    let policy = &mut ledger.pftl_uniswap_routes[0]
        .v2
        .as_mut()
        .unwrap()
        .primary_market_policy;
    policy.spread_recipient = Some(management.to_string());
    policy.policy_hash = policy.computed_hash();
    redeem.policy_hash = policy.policy_hash.clone();
}

fn management_account(ledger: &mut LedgerState, payout: &str) -> String {
    let key = ml_dsa_65_keygen_from_seed(&[0x65; 32]);
    let address = address_from_public_key(&key.public_key);
    ledger.accounts.push(Account::new(
        address.clone(),
        100_000,
        Some(bytes_to_hex(&key.public_key)),
    ));
    let issuer = ledger.asset_definition(payout).unwrap().issuer.clone();
    ledger.trustlines.push(
        TrustLine::new(
            address.clone(),
            issuer,
            payout.to_string(),
            2_000_000_000,
            0,
        )
        .unwrap(),
    );
    address
}

fn fund_without_mint(source: bool) {
    let (genesis, mut ledger, key, mut redeem) = facility_fixture(source);
    let owner = redeem.owner.clone();
    enable_facility(&mut ledger, &mut redeem, &owner);
    let payout = redeem
        .settlement_source_asset_id
        .clone()
        .unwrap_or_else(|| ledger.pftl_uniswap_routes[0].settlement_asset_id.clone());
    ledger.pftl_uniswap_routes[0].paused = false;
    ledger.pftl_uniswap_routes[0].live_value_enabled = true;
    ledger.pftl_uniswap_routes[0].settlement_reserve_atoms = 0;
    if source {
        ledger.pftl_uniswap_source_custody[0].principal_atoms = 0;
        ledger.pftl_uniswap_source_custody[0].enabled_for_issue = true;
    }
    let total = issued_asset_supply(&ledger, &payout).unwrap();
    let nav_supply = ledger.nav_assets[0].circulating_supply;
    let inventory = ledger.pftl_uniswap_routes[0]
        .native_spendable_balances_atoms
        .clone();
    let balance = ledger
        .trustline_for_account_asset(&owner, &payout)
        .unwrap()
        .balance;
    let op = PftlUniswapRedemptionFundOperation {
        funder: owner.clone(),
        route_id: redeem.route_id.clone(),
        settlement_asset_id: payout.clone(),
        funding_nonce: "78".repeat(32),
        amount_atoms: 5_000_000,
    };
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRedemptionFund(op.clone()),
    );
    assert!(receipt.accepted, "fund: {receipt:?}");
    assert_eq!(
        ledger.pftl_uniswap_routes[0].settlement_reserve_atoms,
        5_000_000
    );
    // NAV is 7 settlement atoms per NAV atom; reserve-limited capacity rises from zero.
    assert_eq!(
        ledger.pftl_uniswap_routes[0].settlement_reserve_atoms / 7,
        714_285
    );
    assert_eq!(ledger.nav_assets[0].circulating_supply, nav_supply);
    assert_eq!(
        ledger.pftl_uniswap_routes[0].native_spendable_balances_atoms,
        inventory
    );
    assert_eq!(
        ledger
            .trustline_for_account_asset(&owner, &payout)
            .unwrap()
            .balance,
        balance - 5_000_000
    );
    assert_eq!(issued_asset_supply(&ledger, &payout).unwrap(), total);
    if source {
        assert_eq!(
            ledger.pftl_uniswap_source_custody[0].principal_atoms,
            5_000_000
        );
    }
    assert_eq!(
        ledger.pftl_uniswap_receipts.last().unwrap().transition,
        "redemption_funded"
    );
    let before = ledger.clone();
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRedemptionFund(op.clone()),
    );
    assert_eq!(receipt.code, "pftl_uniswap_funding_invalid");
    assert_eq!(ledger, before, "duplicate funding must roll back");
    let mut too_much = op.clone();
    too_much.funding_nonce = "79".repeat(32);
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRedemptionFund(too_much),
    );
    assert_eq!(receipt.code, "insufficient_issued_balance");
    assert_eq!(
        ledger, before,
        "source accounting and all balances must roll back"
    );
    ledger.validate_pftl_source_custody().unwrap();
    println!("D1 source={source}: accepted=true reserve=0->5000000 capacity=0->714285 B23_supply={nav_supply} unchanged; replay/insufficient-balance rollback verified");
}
#[test]
fn b23_fund_without_mint_pooled() {
    fund_without_mint(false);
}
#[test]
fn b23_fund_without_mint_source() {
    fund_without_mint(true);
}

fn thousand_b23_fixture(
    source: bool,
) -> (
    Genesis,
    LedgerState,
    MlDsa65KeyPair,
    PftlUniswapPrimaryRedeemOperation,
    String,
) {
    let (genesis, mut ledger, key, mut redeem) = facility_fixture(source);
    let nav = ledger.pftl_uniswap_routes[0].native_nav_asset_id.clone();
    let payout = redeem
        .settlement_source_asset_id
        .clone()
        .unwrap_or_else(|| ledger.pftl_uniswap_routes[0].settlement_asset_id.clone());
    let management = management_account(&mut ledger, &payout);
    enable_facility(&mut ledger, &mut redeem, &management);
    let route = &mut ledger.pftl_uniswap_routes[0];
    route.authorized_valid_supply_atoms = 1_000_000_000;
    route.pftl_spendable_supply_atoms = 1_000_000_000;
    route
        .native_spendable_balances_atoms
        .insert(redeem.owner.clone(), 1_000_000_000);
    route.settlement_reserve_atoms = 1_000_000_000;
    route.route_supply_cap_atoms = 2_000_000_000;
    let policy = &mut route.v2.as_mut().unwrap().primary_market_policy;
    policy.redeem_multiplier_bps = 9980;
    policy.max_order_atoms = 1_000_000_000;
    policy.issue_capacity_atoms = 2_000_000_000;
    policy.redeem_capacity_atoms = 2_000_000_000;
    policy.policy_hash = policy.computed_hash();
    redeem.policy_hash = policy.policy_hash.clone();
    redeem.nav_amount_atoms = 1_000_000_000;
    redeem.min_settlement_value_atoms = 998_000_000;
    let line = ledger
        .trustlines
        .iter_mut()
        .find(|line| line.account == redeem.owner && line.asset_id == nav)
        .unwrap();
    line.balance = 1_000_000_000;
    line.limit = 2_000_000_000;
    let line = ledger
        .trustlines
        .iter_mut()
        .find(|line| line.account == redeem.owner && line.asset_id == payout)
        .unwrap();
    line.limit = 2_000_000_000;
    ledger.nav_assets[0].circulating_supply = 1_000_000_000;
    ledger.nav_assets[0].nav_per_unit = 1_000_000;
    if source {
        ledger.pftl_uniswap_source_custody[0].principal_atoms = 1_000_000_000;
        let bucket = &mut ledger.vault_bridge_bucket_states[0];
        bucket.gross_receipt_atoms = 2_000_000_000;
        bucket.counted_value_atoms = 2_000_000_000;
        bucket.outstanding_vault_bridge_atoms = 2_000_000_000;
    }
    ledger.pftl_uniswap_routes[0].validate().unwrap();
    (genesis, ledger, key, redeem, management)
}

fn two_credit_redemption(source: bool) {
    let (genesis, mut ledger, key, redeem, management) = thousand_b23_fixture(source);
    let payout = redeem
        .settlement_source_asset_id
        .clone()
        .unwrap_or_else(|| ledger.pftl_uniswap_routes[0].settlement_asset_id.clone());
    let balance = ledger
        .trustline_for_account_asset(&redeem.owner, &payout)
        .unwrap()
        .balance;
    let total = issued_asset_supply(&ledger, &payout).unwrap();
    // Custody release, not issuance: exact supply cap must not block either credit.
    ledger
        .asset_definitions
        .iter_mut()
        .find(|a| a.asset_id == payout)
        .unwrap()
        .max_supply = Some(total);
    let before = ledger.clone();
    let management_index = ledger
        .trustlines
        .iter()
        .position(|l| l.account == management && l.asset_id == payout)
        .unwrap();
    ledger.trustlines[management_index].frozen = true;
    let frozen = ledger.clone();
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem.clone()),
    );
    assert_eq!(receipt.code, "frozen_trustline");
    assert_eq!(
        ledger, frozen,
        "failed management credit must roll back redeem, reserve, nonce, and redeemer credit"
    );
    ledger = before;
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem.clone()),
    );
    assert!(receipt.accepted, "redeem: {receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&redeem.owner, &payout)
            .unwrap()
            .balance
            - balance,
        998_000_000
    );
    assert_eq!(
        ledger
            .trustline_for_account_asset(&management, &payout)
            .unwrap()
            .balance,
        2_000_000
    );
    assert_eq!(issued_asset_supply(&ledger, &payout).unwrap(), total);
    assert_eq!(ledger.pftl_uniswap_routes[0].settlement_reserve_atoms, 0);
    assert_eq!(
        ledger.pftl_uniswap_routes[0]
            .v2
            .as_ref()
            .unwrap()
            .non_nav_spread_atoms,
        70_000
    );
    assert_eq!(ledger.nav_assets[0].circulating_supply, 0);
    if source {
        assert_eq!(ledger.pftl_uniswap_source_custody[0].principal_atoms, 0);
        assert_eq!(ledger.pftl_uniswap_source_custody[0].spread_atoms, 70_000);
    }
    ledger.validate_pftl_source_custody().unwrap();
    let consensus = ledger.pftl_uniswap_receipts.last().unwrap();
    let settlement = consensus.redemption_settlement.as_ref().unwrap();
    assert_eq!(
        settlement.settlement_value_atoms + settlement.spread_value_atoms,
        1_000_000_000
    );
    assert_eq!(settlement.spread_recipient, management);
    assert_eq!(settlement.settlement_asset_id, payout);
    assert_eq!(
        consensus.receipt_hash,
        postfiat_types::pftl_uniswap_consensus_receipt_computed_hash(consensus)
    );
    println!(
        "D2 source={source}: accepted=true {}",
        serde_json::to_string(consensus).unwrap()
    );
}
#[test]
fn b23_redeem_1000_pays_both_pooled() {
    two_credit_redemption(false);
}
#[test]
fn b23_redeem_1000_pays_both_source() {
    two_credit_redemption(true);
}

#[test]
fn b23_live_route_mint_then_redeem() {
    let issuer_key = ml_dsa_65_keygen_from_seed(&[0x67; 32]);
    let issuer = address_from_public_key(&issuer_key.public_key);
    let (genesis, mut ledger, key, mut redeem) = facility_fixture_for_issuer(false, &issuer);
    let owner = redeem.owner.clone();
    enable_facility(&mut ledger, &mut redeem, &owner);
    ledger.pftl_uniswap_routes[0].live_value_enabled = true;
    ledger.pftl_uniswap_routes[0].paused = false;
    let nav = ledger.pftl_uniswap_routes[0].native_nav_asset_id.clone();
    // A new reserve packet authorizes a one-token top-up beyond the two issued tokens.
    ledger.nav_assets[0].circulating_supply = 3_000_000;
    let before_reserve = ledger.pftl_uniswap_routes[0].settlement_reserve_atoms;
    let op = NavMintAtNavOperation {
        issuer: issuer.clone(),
        to: owner.clone(),
        asset_id: nav.clone(),
        amount: 1_000_000,
        epoch: redeem.pricing_nav_epoch,
        reserve_packet_hash: redeem.pricing_reserve_packet_hash.clone(),
        settlement_asset_id: String::new(),
        settlement_bucket_id: String::new(),
        settlement_allocation_id: String::new(),
        settlement_amount_atoms: 0,
    };
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &issuer_key,
        NAV_MINT_AT_NAV_TRANSACTION_KIND,
        AssetTransactionOperation::NavMintAtNav(op.clone()),
    );
    assert!(receipt.accepted, "mint: {receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&owner, &nav)
            .unwrap()
            .balance,
        3_000_000
    );
    assert_eq!(
        ledger.pftl_uniswap_routes[0].authorized_valid_supply_atoms, 3_000_000,
        "route must account for the new mint"
    );
    assert_eq!(
        ledger.pftl_uniswap_routes[0].pftl_spendable_supply_atoms,
        3_000_000
    );
    assert_eq!(
        ledger.pftl_uniswap_routes[0]
            .native_spendable_balances_atoms
            .get(&owner),
        Some(&3_000_000)
    );
    assert_eq!(
        ledger.pftl_uniswap_routes[0].settlement_reserve_atoms,
        before_reserve
    );
    ledger.pftl_uniswap_routes[0].validate().unwrap();
    let before = ledger.clone();
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &issuer_key,
        NAV_MINT_AT_NAV_TRANSACTION_KIND,
        AssetTransactionOperation::NavMintAtNav(op),
    );
    assert_eq!(receipt.code, "nav_supply_cap_exceeded");
    assert_eq!(ledger, before);
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem),
    );
    assert!(receipt.accepted, "redeem top-up: {receipt:?}");
    assert_eq!(ledger.nav_assets[0].circulating_supply, 2_000_000);
    assert_eq!(
        ledger.pftl_uniswap_routes[0].authorized_valid_supply_atoms,
        2_000_000
    );
    assert_eq!(
        ledger.pftl_uniswap_routes[0]
            .native_spendable_balances_atoms
            .get(&owner),
        Some(&2_000_000)
    );
    ledger.pftl_uniswap_routes[0].validate().unwrap();
    println!("C3 live route mint accepted: route/native inventory 2000000->3000000; reserve unchanged; redeem accepted ->2000000; invariant holds");
}

#[test]
fn b23_policy_epoch_changes_next_redemption_fee() {
    let (genesis, mut ledger, key, mut redeem, management) = thousand_b23_fixture(false);
    let payout = ledger.pftl_uniswap_routes[0].settlement_asset_id.clone();
    ledger.nav_assets[0].reserve_operator = redeem.owner.clone();
    let mut next = ledger.pftl_uniswap_routes[0]
        .v2
        .as_ref()
        .unwrap()
        .primary_market_policy
        .clone();
    next.policy_epoch = 2;
    next.redeem_multiplier_bps = 9900;
    next.policy_hash = next.computed_hash();
    let advance = PftlUniswapRouteEpochAdvanceOperation {
        operator: redeem.owner.clone(),
        route_id: redeem.route_id.clone(),
        prior_route_epoch: 1,
        next_route_epoch: 2,
        next_route_config_digest: "79".repeat(48),
        live_value_enabled: true,
        next_primary_market_policy: next.clone(),
        settlement_source_asset_ids: None,
    };
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_ROUTE_EPOCH_ADVANCE_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(advance),
    );
    assert!(receipt.accepted, "epoch advance: {receipt:?}");
    let before = ledger.clone();
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem.clone()),
    );
    assert_eq!(receipt.code, "pftl_uniswap_redemption_policy_mismatch");
    assert_eq!(ledger, before);
    redeem.route_epoch = 2;
    redeem.policy_epoch = 2;
    redeem.policy_hash = next.policy_hash;
    redeem.min_settlement_value_atoms = 990_000_000;
    let balance = ledger
        .trustline_for_account_asset(&redeem.owner, &payout)
        .unwrap()
        .balance;
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem.clone()),
    );
    assert!(receipt.accepted, "new fee redemption: {receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&redeem.owner, &payout)
            .unwrap()
            .balance
            - balance,
        990_000_000
    );
    assert_eq!(
        ledger
            .trustline_for_account_asset(&management, &payout)
            .unwrap()
            .balance,
        10_000_000
    );
    println!("D7 signed route epoch advance accepted: x 20->100; old binding rejected; next redemption pays 990000000+10000000");
}

#[test]
fn b23_reserve_cap_slippage_and_nonce_fail_atomically() {
    let (genesis, ledger, key, redeem, _) = thousand_b23_fixture(false);
    for mode in 0..3 {
        let mut state = ledger.clone();
        let mut op = redeem.clone();
        let expected = match mode {
            0 => {
                state.pftl_uniswap_routes[0].settlement_reserve_atoms -= 1;
                "pftl_uniswap_redemption_unavailable"
            }
            1 => {
                op.min_settlement_value_atoms += 1;
                "pftl_uniswap_redemption_unavailable"
            }
            _ => {
                state.pftl_uniswap_routes[0]
                    .v2
                    .as_mut()
                    .unwrap()
                    .redemption_nonces
                    .insert(op.redemption_nonce.clone(), op.owner.clone());
                "pftl_uniswap_redemption_policy_mismatch"
            }
        };
        let before = state.clone();
        let receipt = submit_asset(
            &genesis,
            &mut state,
            &key,
            PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapPrimaryRedeem(op),
        );
        assert_eq!(receipt.code, expected);
        assert_eq!(state, before);
    }
}

#[test]
fn b23_zero_fee_and_rounding_conserve_reserve() {
    for multiplier in [10_000, 9999, 9500] {
        let (genesis, mut ledger, key, mut redeem, management) = thousand_b23_fixture(false);
        let payout = ledger.pftl_uniswap_routes[0].settlement_asset_id.clone();
        let policy = &mut ledger.pftl_uniswap_routes[0]
            .v2
            .as_mut()
            .unwrap()
            .primary_market_policy;
        policy.redeem_multiplier_bps = multiplier;
        policy.policy_hash = policy.computed_hash();
        redeem.policy_hash = policy.policy_hash.clone();
        redeem.nav_amount_atoms = 1_000_003;
        let output = redeem.nav_amount_atoms * u64::from(multiplier) / 10_000;
        let spread = redeem.nav_amount_atoms - output;
        redeem.min_settlement_value_atoms = output;
        let before = ledger
            .trustline_for_account_asset(&redeem.owner, &payout)
            .unwrap()
            .balance;
        let receipt = submit_asset(
            &genesis,
            &mut ledger,
            &key,
            PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem),
        );
        assert!(receipt.accepted, "rounding: {receipt:?}");
        assert_eq!(
            ledger
                .trustline_for_account_asset(&management, &payout)
                .unwrap()
                .balance,
            spread
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&address_from_public_key(&key.public_key), &payout)
                .unwrap()
                .balance
                - before,
            output
        );
        assert_eq!(
            ledger.pftl_uniswap_routes[0].settlement_reserve_atoms,
            1_000_000_000 - output - spread
        );
    }
}
