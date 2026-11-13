fn rev3_funding_fixture() -> (
    Genesis,
    LedgerState,
    MlDsa65KeyPair,
    PftlUniswapRedemptionFundOperation,
) {
    let (genesis, mut ledger, key, mut redeem) = facility_fixture(false);
    let owner = redeem.owner.clone();
    enable_facility(&mut ledger, &mut redeem, &owner);
    ledger.nav_assets[0].reserve_operator = owner.clone();
    let route = &mut ledger.pftl_uniswap_routes[0];
    route.paused = false;
    route.live_value_enabled = true;
    let op = PftlUniswapRedemptionFundOperation {
        funder: owner,
        route_id: route.route_id.clone(),
        settlement_asset_id: route.settlement_asset_id.clone(),
        funding_nonce: "78".repeat(32),
        amount_atoms: 1_000_000,
        funding_sequence: Some(1),
    };
    (genesis, ledger, key, op)
}

#[test]
fn b23_rev3_funding_authority_dust_sequences_and_subscription_isolation() {
    let (genesis, ledger, key, op) = rev3_funding_fixture();
    for case in 0..6 {
        let mut state = ledger.clone();
        let mut fund = op.clone();
        let code = match case {
            0 => {
                state.nav_assets[0].reserve_operator = state.nav_assets[0].issuer.clone();
                fund.amount_atoms = 1;
                "unauthorized_pftl_uniswap_operator"
            }
            1 => {
                fund.amount_atoms -= 1;
                "pftl_uniswap_funding_below_minimum"
            }
            2 => {
                fund.funding_sequence = None;
                "pftl_uniswap_funding_sequence_mismatch"
            }
            3 => {
                fund.funding_sequence = Some(2);
                "pftl_uniswap_funding_sequence_mismatch"
            }
            4 => {
                fund.funding_sequence = Some(0);
                "pftl_uniswap_funding_sequence_mismatch"
            }
            _ => {
                state.pftl_uniswap_routes[0]
                    .v2
                    .as_mut()
                    .unwrap()
                    .funding_sequence = Some(u64::MAX);
                "pftl_uniswap_funding_sequence_exhausted"
            }
        };
        let before = state.clone();
        let receipt = submit_asset(
            &genesis,
            &mut state,
            &key,
            PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapRedemptionFund(fund),
        );
        assert!(!receipt.accepted, "case {case}: {receipt:?}");
        assert_eq!(receipt.code, code, "case {case}");
        assert_eq!(state, before);
    }
    let mut state = ledger;
    // An already-full subscription map (including the chosen fund tag) cannot
    // be enlarged, exhausted, or consumed by a permitted reserve contribution.
    for i in 0..8191 {
        state.pftl_uniswap_routes[0]
            .primary_subscription_nonces
            .insert(format!("{i:064x}"), op.funder.clone());
    }
    state.pftl_uniswap_routes[0]
        .primary_subscription_nonces
        .insert(op.funding_nonce.clone(), op.funder.clone());
    let nonces = state.pftl_uniswap_routes[0]
        .primary_subscription_nonces
        .clone();
    for sequence in 1..=2 {
        let mut fund = op.clone();
        fund.funding_sequence = Some(sequence);
        let receipt = submit_asset(
            &genesis,
            &mut state,
            &key,
            PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapRedemptionFund(fund),
        );
        assert!(receipt.accepted, "sequence {sequence}: {receipt:?}");
        assert_eq!(
            state.pftl_uniswap_routes[0].primary_subscription_nonces,
            nonces
        );
        assert_eq!(
            state.pftl_uniswap_routes[0]
                .v2
                .as_ref()
                .unwrap()
                .funding_sequence,
            Some(sequence)
        );
    }
    assert_eq!(
        state.pftl_uniswap_routes[0].settlement_reserve_atoms,
        16_000_000
    );
    assert_eq!(
        state.pftl_uniswap_routes[0].authorized_valid_supply_atoms,
        2_000_000
    );
}

fn rev3_epoch(
    ledger: &LedgerState,
    owner: &str,
    index: usize,
) -> PftlUniswapRouteEpochAdvanceOperation {
    let route = &ledger.pftl_uniswap_routes[index];
    let v2 = route.v2.as_ref().unwrap();
    let mut policy = v2.primary_market_policy.clone();
    policy.policy_epoch += 1;
    policy.spread_recipient = Some(owner.to_string());
    policy.policy_hash = policy.computed_hash();
    PftlUniswapRouteEpochAdvanceOperation {
        operator: owner.to_string(),
        route_id: route.route_id.clone(),
        prior_route_epoch: v2.route_epoch,
        next_route_epoch: v2.route_epoch + 1,
        next_route_config_digest: "79".repeat(48),
        live_value_enabled: true,
        next_primary_market_policy: policy,
        settlement_source_asset_ids: None,
    }
}

#[test]
fn b23_rev3_opt_in_requires_exact_holders_and_one_route() {
    let (genesis, mut ledger, key, redeem) = facility_fixture(false);
    ledger.nav_assets[0].reserve_operator = redeem.owner.clone();
    let mut dormant = ledger.pftl_uniswap_routes[0].clone();
    dormant.route_id = "dormant-route".into();
    dormant.authorized_valid_supply_atoms = 0;
    dormant.pftl_spendable_supply_atoms = 0;
    dormant.native_spendable_balances_atoms.clear();
    ledger.pftl_uniswap_routes.push(dormant);
    let op = rev3_epoch(&ledger, &redeem.owner, 0);
    let mut stale = ledger.clone();
    stale.trustlines[0].account = "bob-pre-existing".into();
    let before = stale.clone();
    let receipt = submit_asset(
        &genesis,
        &mut stale,
        &key,
        PFTL_UNISWAP_ROUTE_EPOCH_ADVANCE_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(op.clone()),
    );
    assert_eq!(receipt.code, "pftl_uniswap_inventory_mismatch");
    assert_eq!(stale, before);
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_ROUTE_EPOCH_ADVANCE_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(op),
    );
    assert!(receipt.accepted, "aligned opt-in: {receipt:?}");
    let op = rev3_epoch(&ledger, &redeem.owner, 1);
    let before = ledger.clone();
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_ROUTE_EPOCH_ADVANCE_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(op),
    );
    assert_eq!(receipt.code, "multiple_pftl_uniswap_facility_routes");
    assert_eq!(ledger, before);
    // Corrupt imported state also fails closed instead of duplicating inventory.
    ledger.pftl_uniswap_routes[1]
        .v2
        .as_mut()
        .unwrap()
        .primary_market_policy
        .spread_recipient = Some(redeem.owner);
    let before = ledger.clone();
    assert_eq!(
        b23_facility_transfer_offer_inventory(
            &mut ledger,
            &before.pftl_uniswap_routes[0].native_nav_asset_id,
            "alice",
            "bob",
            1
        )
        .unwrap_err()
        .0,
        "multiple_pftl_uniswap_facility_routes"
    );
    assert_eq!(ledger, before);
}

fn rev3_add_bob(ledger: &mut LedgerState) -> (MlDsa65KeyPair, String) {
    let key = ml_dsa_65_keygen_from_seed(&[0x74; 32]);
    let bob = address_from_public_key(&key.public_key);
    ledger.accounts.push(Account::new(
        bob.clone(),
        100_000,
        Some(bytes_to_hex(&key.public_key)),
    ));
    let route = &ledger.pftl_uniswap_routes[0];
    for asset in [&route.native_nav_asset_id, &route.settlement_asset_id] {
        ledger.trustlines.push(
            TrustLine::new(
                bob.clone(),
                ledger.asset_definition(asset).unwrap().issuer.clone(),
                asset.clone(),
                100_000_000,
                0,
            )
            .unwrap(),
        );
    }
    (key, bob)
}

#[test]
fn b23_rev3_payment_buyer_redeems_ignoring_dormant_route() {
    for opted in [false, true] {
        let (genesis, mut ledger, key, mut redeem) = facility_fixture(false);
        let owner = redeem.owner.clone();
        if opted {
            enable_facility(&mut ledger, &mut redeem, &owner);
        }
        let mut dormant = ledger.pftl_uniswap_routes[0].clone();
        dormant.route_id = "dormant-route".into();
        dormant
            .v2
            .as_mut()
            .unwrap()
            .primary_market_policy
            .spread_recipient = None;
        dormant
            .v2
            .as_mut()
            .unwrap()
            .primary_market_policy
            .policy_hash = dormant
            .v2
            .as_ref()
            .unwrap()
            .primary_market_policy
            .computed_hash();
        dormant.authorized_valid_supply_atoms = 0;
        dormant.pftl_spendable_supply_atoms = 0;
        dormant.native_spendable_balances_atoms.clear();
        ledger.pftl_uniswap_routes.push(dormant.clone());
        let (bob_key, bob) = rev3_add_bob(&mut ledger);
        let native = ledger.pftl_uniswap_routes[0].native_nav_asset_id.clone();
        let before_routes = ledger.pftl_uniswap_routes.clone();
        let pay = IssuedPaymentOperation {
            from: owner.clone(),
            to: bob.clone(),
            issuer: ledger.asset_definition(&native).unwrap().issuer.clone(),
            asset_id: native.clone(),
            amount: 1_000_000,
        };
        let receipt = submit_asset(
            &genesis,
            &mut ledger,
            &key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            AssetTransactionOperation::IssuedPayment(pay),
        );
        assert!(receipt.accepted, "payment: {receipt:?}");
        assert_eq!(ledger.pftl_uniswap_routes[1], dormant);
        if !opted {
            assert_eq!(ledger.pftl_uniswap_routes, before_routes);
            continue;
        }
        assert_eq!(
            ledger.pftl_uniswap_routes[0]
                .native_spendable_balances_atoms
                .get(&bob),
            Some(&1_000_000)
        );
        redeem.owner = bob.clone();
        redeem.settlement_recipient = bob;
        let receipt = submit_asset(
            &genesis,
            &mut ledger,
            &bob_key,
            PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem),
        );
        assert!(receipt.accepted, "Bob redeem: {receipt:?}");
        ledger.pftl_uniswap_routes[0].validate().unwrap();
    }
}

#[test]
fn b23_rev3_management_credit_creates_line_and_raises_limit() {
    for remove in [false, true] {
        let (genesis, mut ledger, key, redeem, management) = thousand_b23_fixture(false);
        let payout = ledger.pftl_uniswap_routes[0].settlement_asset_id.clone();
        let index = ledger
            .trustlines
            .iter()
            .position(|l| l.account == management && l.asset_id == payout)
            .unwrap();
        if remove {
            ledger.trustlines.remove(index);
        } else {
            ledger.trustlines[index].limit = 1;
        }
        let receipt = submit_asset(
            &genesis,
            &mut ledger,
            &key,
            PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem),
        );
        assert!(receipt.accepted, "management credit: {receipt:?}");
        let line = ledger
            .trustline_for_account_asset(&management, &payout)
            .unwrap();
        assert_eq!(line.balance, 2_000_000);
        assert!(line.limit >= line.balance);
    }
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

#[test]
fn b23_rev3_private_redemption_is_explicitly_disabled() {
    let (_, mut ledger, _, mut redeem) = facility_fixture(false);
    let owner = redeem.owner.clone();
    enable_facility(&mut ledger, &mut redeem, &owner);
    let private_redeem = AssetOrchardPrivatePrimaryRedeemActionPayload {
        version: 1,
        schema: "postfiat-asset-orchard-private-primary-redeem-action-v1".to_string(),
        pool_id: "asset-orchard-v1".to_string(),
        route_id: redeem.route_id.clone(),
        subscriber: redeem.owner.clone(),
        ethereum_recipient: redeem.owner.clone(),
        reservation_id: "c7".repeat(48),
        subscription_nonce: "c8".repeat(32),
        route_epoch: 2,
        policy_epoch: 2,
        policy_hash: redeem.policy_hash.clone(),
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: redeem.pricing_reserve_packet_hash.clone(),
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

    let before = ledger.clone();
    let error = apply_asset_orchard_private_primary_redeem_route_transition(
        &mut ledger,
        &private_redeem,
        11,
    )
    .unwrap_err();
    assert_eq!(error.0, "pftl_uniswap_private_redemption_disabled");
    assert_eq!(ledger, before);
    ledger.pftl_uniswap_routes[0]
        .v2
        .as_mut()
        .unwrap()
        .primary_market_policy
        .spread_recipient = None;
    assert_ne!(
        apply_asset_orchard_private_primary_redeem_route_transition(
            &mut ledger,
            &private_redeem,
            11
        )
        .unwrap_err()
        .0,
        "pftl_uniswap_private_redemption_disabled"
    );
}

#[test]
fn b23_rev3_escrow_finish_buyer_redeems_and_cancel_conserves() {
    for cancel in [false, true] {
        let (genesis, mut ledger, key, mut redeem) = facility_fixture(false);
        let owner = redeem.owner.clone();
        enable_facility(&mut ledger, &mut redeem, &owner);
        let (bob_key, bob) = rev3_add_bob(&mut ledger);
        let native = ledger.pftl_uniswap_routes[0].native_nav_asset_id.clone();
        let before_inventory = ledger.pftl_uniswap_routes[0]
            .native_spendable_balances_atoms
            .clone();
        let id = escrow_id(&genesis.chain_id, &owner, 1).unwrap();
        let tx = signed_escrow_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &key,
            ESCROW_CREATE_TRANSACTION_KIND,
            1,
            EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
                owner: owner.clone(),
                recipient: bob.clone(),
                asset_id: native.clone(),
                amount: 1_000_000,
                condition: "b23-test".into(),
                finish_after: 11,
                cancel_after: 12,
            }),
        );
        let receipt = execute_escrow_transaction(&genesis, &mut ledger, &tx, 11);
        assert!(receipt.accepted, "create: {receipt:?}");
        assert_eq!(
            ledger.pftl_uniswap_routes[0].native_spendable_balances_atoms,
            before_inventory
        );
        let tx = if cancel {
            signed_escrow_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &key,
                ESCROW_CANCEL_TRANSACTION_KIND,
                2,
                EscrowTransactionOperation::EscrowCancel(EscrowCancelOperation {
                    escrow_id: id,
                    owner: owner.clone(),
                }),
            )
        } else {
            signed_escrow_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &bob_key,
                ESCROW_FINISH_TRANSACTION_KIND,
                1,
                EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
                    escrow_id: id,
                    owner: owner.clone(),
                    recipient: bob.clone(),
                    fulfillment: "b23-test".into(),
                }),
            )
        };
        let receipt =
            execute_escrow_transaction(&genesis, &mut ledger, &tx, if cancel { 12 } else { 11 });
        assert!(receipt.accepted, "finish/cancel: {receipt:?}");
        if cancel {
            assert_eq!(
                ledger.pftl_uniswap_routes[0].native_spendable_balances_atoms,
                before_inventory
            );
            assert_eq!(
                ledger
                    .trustline_for_account_asset(&owner, &native)
                    .unwrap()
                    .balance,
                2_000_000
            );
            continue;
        }
        assert_eq!(
            ledger.pftl_uniswap_routes[0]
                .native_spendable_balances_atoms
                .get(&bob),
            Some(&1_000_000)
        );
        redeem.owner = bob.clone();
        redeem.settlement_recipient = bob;
        let receipt = submit_asset(
            &genesis,
            &mut ledger,
            &bob_key,
            PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND,
            AssetTransactionOperation::PftlUniswapPrimaryRedeem(redeem),
        );
        assert!(receipt.accepted, "escrow buyer redeem: {receipt:?}");
        ledger.pftl_uniswap_routes[0].validate().unwrap();
    }
}

#[test]
fn b23_rev3_funding_signing_bytes_keep_absent_extension_unchanged() {
    let (genesis, ledger, key, mut op) = rev3_funding_fixture();
    op.funding_sequence = None;
    let tx = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &key,
        PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::PftlUniswapRedemptionFund(op.clone()),
    );
    let bytes = tx.unsigned.signing_bytes();
    let legacy_suffix = format!(
        "funder={}\nroute_id={}\nsettlement_asset_id={}\nfunding_nonce={}\namount_atoms={}\n",
        op.funder, op.route_id, op.settlement_asset_id, op.funding_nonce, op.amount_atoms
    );
    assert!(bytes.ends_with(legacy_suffix.as_bytes()));
    assert_eq!(hash_hex("b23.legacy-fund-payload", &bytes[bytes.len()-legacy_suffix.len()..]), "12df9aebd349f10bd7b4fba06f8b5944932e7f96a09b14ee5434c469c57d5b753e86c7a8c99446820228b32c7babac2d");
    assert!(!serde_json::to_string(&tx.unsigned.operation)
        .unwrap()
        .contains("funding_sequence"));
    op.funding_sequence = Some(1);
    let tx = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &key,
        PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::PftlUniswapRedemptionFund(op),
    );
    assert!(tx
        .unsigned
        .signing_bytes()
        .ends_with(b"funding_sequence=1\n"));
    let mut route = ledger.pftl_uniswap_routes[0].clone();
    let hash = pftl_uniswap_route_state_hash(&route);
    assert!(serde_json::to_value(&route).unwrap()["v2"]
        .get("funding_sequence")
        .is_none());
    route.v2.as_mut().unwrap().funding_sequence = Some(1);
    assert_ne!(hash, pftl_uniswap_route_state_hash(&route));
}

#[test]
fn b23_rev3_init_rejects_second_opted_in_route() {
    let (genesis, mut ledger, key, mut redeem) = facility_fixture(false);
    let owner = redeem.owner.clone();
    enable_facility(&mut ledger, &mut redeem, &owner);
    ledger.nav_assets[0].reserve_operator = owner.clone();
    let route = ledger.pftl_uniswap_routes[0].clone();
    let v2 = route.v2.as_ref().unwrap();
    let op = PftlUniswapRouteInitV2Operation {
        operator: owner,
        route_id: "second-opted-route".into(),
        route_config_digest: route.route_config_digest,
        native_nav_asset_id: route.native_nav_asset_id,
        settlement_asset_id: route.settlement_asset_id,
        opening_inventory_atoms: 0,
        opening_inventory_holder: String::new(),
        handoff_controller: route.handoff_controller,
        settlement_adapter: route.settlement_adapter,
        wrapped_navcoin_token: route.wrapped_navcoin_token,
        ethereum_chain_id: 1,
        route_supply_cap_atoms: route.route_supply_cap_atoms,
        packet_notional_cap_atoms: route.packet_notional_cap_atoms,
        latest_finalized_nav_epoch: 7,
        return_finality_blocks: 12,
        route_epoch: 1,
        outbound_verification_class: v2.outbound_verification_class.clone(),
        return_verification_class: v2.return_verification_class.clone(),
        live_value_enabled: false,
        ethereum_verification_policy: EthereumRouteVerificationPolicyV1 {
            authority_epoch: 1,
            committee_root: FastSwapCommitteeRootV1([0x71; 48]),
            minimum_confirmations: 12,
            handoff_controller_code_hash: [0x72; 32],
            wrapped_navcoin_code_hash: [0x73; 32],
        },
        primary_market_policy: v2.primary_market_policy.clone(),
    };
    let before = ledger.clone();
    let receipt = submit_asset(
        &genesis,
        &mut ledger,
        &key,
        PFTL_UNISWAP_ROUTE_INIT_V2_TRANSACTION_KIND,
        AssetTransactionOperation::PftlUniswapRouteInitV2(op),
    );
    assert_eq!(receipt.code, "multiple_pftl_uniswap_facility_routes");
    assert_eq!(ledger, before);
}
