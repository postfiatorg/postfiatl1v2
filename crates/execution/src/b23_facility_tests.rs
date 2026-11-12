use super::*;
use postfiat_types::*;
use postfiat_crypto_provider::*;

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


fn submit_asset(genesis: &Genesis, ledger: &mut LedgerState, key: &MlDsa65KeyPair,
    kind: &str, operation: AssetTransactionOperation) -> Receipt {
    let owner = address_from_public_key(&key.public_key);
    let tx = signed_asset_transaction_with_minimum_fee(genesis, ledger, key, kind,
        ledger.account(&owner).unwrap().sequence + 1, operation);
    execute_asset_transaction_with_unverified_pftl_uniswap_fixture(genesis, ledger, &tx, 11)
}

fn market_buy_then_redeem(partial: bool) {
    let (genesis, mut ledger, alice_key, mut redeem) = facility_fixture(false);
    let alice = redeem.owner.clone();
    let bob_key = ml_dsa_65_keygen_from_seed(&[0x64;32]);
    let bob = address_from_public_key(&bob_key.public_key);
    ledger.accounts.push(Account::new(bob.clone(),100_000,Some(bytes_to_hex(&bob_key.public_key))));
    let nav = ledger.pftl_uniswap_routes[0].native_nav_asset_id.clone();
    let usd = ledger.pftl_uniswap_routes[0].settlement_asset_id.clone();
    for (asset, balance) in [(&nav, 0), (&usd, 20_000_000)] {
        let issuer = ledger.asset_definition(asset).unwrap().issuer.clone();
        let mut line = TrustLine::new(bob.clone(),issuer,asset.clone(),100_000_000,0).unwrap();
        line.balance=balance;
        ledger.trustlines.push(line);
    }
    let before_supply = ledger.nav_assets[0].circulating_supply;
    let before_reserve = ledger.pftl_uniswap_routes[0].settlement_reserve_atoms;
    let sell_amount = if partial {2_000_000} else {1_000_000};
    let tx = signed_offer_transaction_with_minimum_fee(&genesis,&ledger,&alice_key,
        OFFER_CREATE_TRANSACTION_KIND,1,OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner:alice.clone(),taker_gets_asset_id:nav.clone(),taker_gets_amount:sell_amount,
            taker_pays_asset_id:usd.clone(),taker_pays_amount:sell_amount*7,expiration_height:50,
        }),11);
    let receipt = execute_offer_transaction(&genesis,&mut ledger,&tx,11);
    assert!(receipt.accepted,"Alice posts: {receipt:?}");
    let tx = signed_offer_transaction_with_minimum_fee(&genesis,&ledger,&bob_key,
        OFFER_CREATE_TRANSACTION_KIND,1,OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner:bob.clone(),taker_gets_asset_id:usd.clone(),taker_gets_amount:7_000_000,
            taker_pays_asset_id:nav.clone(),taker_pays_amount:1_000_000,expiration_height:50,
        }),11);
    let receipt = execute_offer_transaction(&genesis,&mut ledger,&tx,11);
    assert!(receipt.accepted,"Bob buys: {receipt:?}");
    assert_eq!(receipt.offer_fills.len(),1);
    assert_eq!(ledger.trustline_for_account_asset(&bob,&nav).unwrap().balance,1_000_000);
    assert_eq!(ledger.nav_assets[0].circulating_supply,before_supply);
    assert_eq!(ledger.pftl_uniswap_routes[0].settlement_reserve_atoms,before_reserve);
    if partial {
        let tx = signed_offer_transaction_with_minimum_fee(&genesis,&ledger,&alice_key,
            OFFER_CANCEL_TRANSACTION_KIND,2,OfferTransactionOperation::OfferCancel(OfferCancelOperation {
                owner:alice.clone(),offer_id:offer_id(&genesis.chain_id,&alice,1).unwrap(),
            }),11);
        let receipt = execute_offer_transaction(&genesis,&mut ledger,&tx,11);
        assert!(receipt.accepted,"Alice cancels residual: {receipt:?}");
        assert_eq!(ledger.trustline_for_account_asset(&alice,&nav).unwrap().balance,1_000_000);
    }
    redeem.owner=bob.clone();
    redeem.settlement_recipient=bob.clone();
    let receipt=submit_asset(&genesis,&mut ledger,&bob_key,PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem));
    assert!(receipt.accepted,"Fresh Bob must redeem purchased NAV: {receipt:?}");
    assert_eq!(ledger.trustline_for_account_asset(&bob,&nav).unwrap().balance,0);
    assert_eq!(ledger.nav_assets[0].circulating_supply,before_supply-1_000_000);
    assert_eq!(ledger.pftl_uniswap_routes[0].settlement_reserve_atoms,before_reserve-7_000_000);
    assert_eq!(ledger.pftl_uniswap_routes[0].native_spendable_balances_atoms.get(&alice),Some(&1_000_000));
    assert!(!ledger.pftl_uniswap_routes[0].native_spendable_balances_atoms.contains_key(&bob));
}

#[test]
fn b23_market_buyer_redeems() { market_buy_then_redeem(false); }
#[test]
fn b23_partial_fill_cancel_buyer_redeems() { market_buy_then_redeem(true); }

#[test]
fn b23_legacy_hash_baseline() {
    let (_, mut ledger, _, _) = facility_fixture(false);
    let route = &ledger.pftl_uniswap_routes[0];
    let policy = &route.v2.as_ref().unwrap().primary_market_policy;
    println!("LEGACY_POLICY_HASH={}",policy.computed_hash());
    println!("LEGACY_ROUTE_HASH={}",pftl_uniswap_route_state_hash(route));
    let route_id = route.route_id.clone();
    let route_hash = pftl_uniswap_route_state_hash(route);
    append_pftl_uniswap_consensus_receipt(&mut ledger,PftlUniswapReceiptPlan {
        transition:"primary_redeemed",route_id:&route_id,state_before_hash:route_hash.clone(),
        state_after_hash:route_hash,packet_hash:None,burn_event_hash:None,wallet:None,
        amount_atoms:Some(1_000_000),block_height:11,
    }).unwrap();
    println!("LEGACY_RECEIPT_HASH={}",ledger.pftl_uniswap_receipts[0].receipt_hash);
}
