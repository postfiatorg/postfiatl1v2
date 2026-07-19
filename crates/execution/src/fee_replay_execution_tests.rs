    #[test]
    fn offer_transactions_create_cancel_and_lock_pft() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let owner = address_from_public_key(&key_pair.public_key);
        let issuer = "pfissuer000000000000000000000000000000000";
        let asset = AssetDefinition::new(&genesis.chain_id, issuer, "USD", 1, 6).expect("asset");
        let trustline =
            TrustLine::new(owner.clone(), issuer, asset.asset_id.clone(), 100, 10).expect("line");
        let mut ledger = LedgerState::new_with_assets(
            vec![Account::new(
                owner.clone(),
                200,
                Some(bytes_to_hex(&key_pair.public_key)),
            )],
            vec![asset.clone()],
            vec![trustline],
        );
        let create = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &key_pair,
            OFFER_CREATE_TRANSACTION_KIND,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: owner.clone(),
                taker_gets_asset_id: "PFT".to_string(),
                taker_gets_amount: 40,
                taker_pays_asset_id: asset.asset_id,
                taker_pays_amount: 25,
                expiration_height: 20,
            }),
            1,
        );
        let create_fee = create.unsigned.fee;
        let receipt = execute_offer_transaction(&genesis, &mut ledger, &create, 1);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
        assert_eq!(receipt.tx_id, offer_transaction_tx_id(&create));
        assert_eq!(receipt.state_expansion_fee, OFFER_STATE_EXPANSION_FEE);
        assert_eq!(
            200 - create_fee - OFFER_OBJECT_RESERVE - 40,
            ledger.account(&owner).expect("owner").balance
        );
        assert_eq!(1, ledger.account(&owner).expect("owner").sequence);
        let created_offer_id = offer_id(&genesis.chain_id, &owner, 1).expect("offer id");
        let offer = ledger.offer(&created_offer_id).expect("created offer");
        assert_eq!(OFFER_STATE_OPEN, offer.state);
        assert_eq!(OFFER_OBJECT_RESERVE, offer.reserve_paid);
        assert_eq!(40, offer.taker_gets_amount_remaining);

        let replay = execute_offer_transaction(&genesis, &mut ledger.clone(), &create, 1);
        assert!(!replay.accepted);
        assert_eq!("bad_sequence", replay.code);

        let cancel = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &key_pair,
            OFFER_CANCEL_TRANSACTION_KIND,
            2,
            OfferTransactionOperation::OfferCancel(OfferCancelOperation {
                offer_id: created_offer_id.clone(),
                owner: owner.clone(),
            }),
            2,
        );
        let cancel_fee = cancel.unsigned.fee;
        let receipt = execute_offer_transaction(&genesis, &mut ledger, &cancel, 2);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
        assert_eq!(
            200 - create_fee - cancel_fee,
            ledger.account(&owner).expect("owner").balance
        );
        assert_eq!(2, ledger.account(&owner).expect("owner").sequence);
        let offer = ledger.offer(&created_offer_id).expect("canceled offer");
        assert_eq!(OFFER_STATE_CANCELED, offer.state);
        assert_eq!(0, offer.reserve_paid);
    }

    #[test]
    fn offer_transactions_lock_and_refund_issued_sell_side_without_supply_drift() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let owner = address_from_public_key(&key_pair.public_key);
        let issuer = "pfissuer000000000000000000000000000000000";
        let asset = AssetDefinition::new(&genesis.chain_id, issuer, "USD", 1, 6).expect("asset");
        let mut trustline =
            TrustLine::new(owner.clone(), issuer, asset.asset_id.clone(), 100, 10).expect("line");
        trustline.balance = 100;
        let mut ledger = LedgerState::new_with_assets(
            vec![Account::new(
                owner.clone(),
                200,
                Some(bytes_to_hex(&key_pair.public_key)),
            )],
            vec![asset.clone()],
            vec![trustline],
        );
        assert_eq!(
            100,
            issued_asset_supply(&ledger, &asset.asset_id).expect("initial supply")
        );

        let create = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &key_pair,
            OFFER_CREATE_TRANSACTION_KIND,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: owner.clone(),
                taker_gets_asset_id: asset.asset_id.clone(),
                taker_gets_amount: 40,
                taker_pays_asset_id: "PFT".to_string(),
                taker_pays_amount: 25,
                expiration_height: 20,
            }),
            1,
        );
        let receipt = execute_offer_transaction(&genesis, &mut ledger, &create, 1);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
        assert_eq!(
            60,
            ledger
                .trustline_for_account_asset(&owner, &asset.asset_id)
                .expect("line")
                .balance
        );
        assert_eq!(
            100,
            issued_asset_supply(&ledger, &asset.asset_id).expect("locked supply")
        );

        let created_offer_id = offer_id(&genesis.chain_id, &owner, 1).expect("offer id");
        let cancel = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &key_pair,
            OFFER_CANCEL_TRANSACTION_KIND,
            2,
            OfferTransactionOperation::OfferCancel(OfferCancelOperation {
                offer_id: created_offer_id,
                owner: owner.clone(),
            }),
            2,
        );
        let receipt = execute_offer_transaction(&genesis, &mut ledger, &cancel, 2);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
        assert_eq!(
            100,
            ledger
                .trustline_for_account_asset(&owner, &asset.asset_id)
                .expect("line")
                .balance
        );
        assert_eq!(
            100,
            issued_asset_supply(&ledger, &asset.asset_id).expect("refunded supply")
        );
    }

    #[test]
    fn offer_create_fully_crosses_best_maker_and_emits_fill_receipt() {
        let genesis = Genesis::new("postfiat-local");
        let maker_key = ml_dsa_65_keygen().expect("maker keygen");
        let taker_key = ml_dsa_65_keygen().expect("taker keygen");
        let maker = address_from_public_key(&maker_key.public_key);
        let taker = address_from_public_key(&taker_key.public_key);
        let issuer = "pfissuer000000000000000000000000000000000";
        let asset = AssetDefinition::new(&genesis.chain_id, issuer, "USD", 1, 6).expect("asset");
        let mut maker_line =
            TrustLine::new(maker.clone(), issuer, asset.asset_id.clone(), 100, 10).expect("line");
        maker_line.balance = 100;
        let taker_line =
            TrustLine::new(taker.clone(), issuer, asset.asset_id.clone(), 100, 10).expect("line");
        let mut ledger = LedgerState::new_with_assets(
            vec![
                Account::new(
                    maker.clone(),
                    200,
                    Some(bytes_to_hex(&maker_key.public_key)),
                ),
                Account::new(
                    taker.clone(),
                    300,
                    Some(bytes_to_hex(&taker_key.public_key)),
                ),
            ],
            vec![asset.clone()],
            vec![maker_line, taker_line],
        );
        let maker_create = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &maker_key,
            OFFER_CREATE_TRANSACTION_KIND,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: maker.clone(),
                taker_gets_asset_id: asset.asset_id.clone(),
                taker_gets_amount: 40,
                taker_pays_asset_id: "PFT".to_string(),
                taker_pays_amount: 80,
                expiration_height: 20,
            }),
            1,
        );
        let maker_fee = maker_create.unsigned.fee;
        let maker_receipt = execute_offer_transaction(&genesis, &mut ledger, &maker_create, 1);
        assert!(maker_receipt.accepted, "{maker_receipt:?}");
        let maker_offer_id = offer_id(&genesis.chain_id, &maker, 1).expect("maker offer id");

        let taker_create = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &taker_key,
            OFFER_CREATE_TRANSACTION_KIND,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: taker.clone(),
                taker_gets_asset_id: "PFT".to_string(),
                taker_gets_amount: 80,
                taker_pays_asset_id: asset.asset_id.clone(),
                taker_pays_amount: 40,
                expiration_height: 20,
            }),
            2,
        );
        let taker_fee = taker_create.unsigned.fee;
        let taker_receipt = execute_offer_transaction(&genesis, &mut ledger, &taker_create, 2);
        assert!(taker_receipt.accepted, "{taker_receipt:?}");
        assert_eq!(taker_receipt.code, "filled");
        assert!(taker_receipt.offer_id.is_none());
        assert_eq!(taker_receipt.offer_fills.len(), 1);
        assert_eq!(taker_receipt.offer_fills[0].maker_offer_id, maker_offer_id);
        assert_eq!(taker_receipt.offer_fills[0].maker_sends_amount, 40);
        assert_eq!(taker_receipt.offer_fills[0].taker_sends_amount, 80);
        assert_eq!(
            taker_receipt.offer_fills[0].terminal_maker_state.as_deref(),
            Some(OFFER_STATE_FILLED)
        );
        let maker_offer = ledger.offer(&maker_offer_id).expect("maker offer");
        assert_eq!(maker_offer.state, OFFER_STATE_FILLED);
        assert_eq!(maker_offer.reserve_paid, 0);
        assert!(ledger
            .offer(&offer_id(&genesis.chain_id, &taker, 1).expect("taker offer"))
            .is_none());
        assert_eq!(
            ledger.account(&maker).expect("maker").balance,
            200 - maker_fee + 80
        );
        assert_eq!(
            ledger.account(&taker).expect("taker").balance,
            300 - taker_fee - 80
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&maker, &asset.asset_id)
                .expect("maker line")
                .balance,
            60
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&taker, &asset.asset_id)
                .expect("taker line")
                .balance,
            40
        );
        assert_eq!(
            100,
            issued_asset_supply(&ledger, &asset.asset_id).expect("supply")
        );
    }

    #[test]
    fn offer_create_partial_cross_leaves_residual_offer() {
        let genesis = Genesis::new("postfiat-local");
        let maker_key = ml_dsa_65_keygen().expect("maker keygen");
        let taker_key = ml_dsa_65_keygen().expect("taker keygen");
        let maker = address_from_public_key(&maker_key.public_key);
        let taker = address_from_public_key(&taker_key.public_key);
        let issuer = "pfissuer000000000000000000000000000000000";
        let asset = AssetDefinition::new(&genesis.chain_id, issuer, "USD", 1, 6).expect("asset");
        let mut maker_line =
            TrustLine::new(maker.clone(), issuer, asset.asset_id.clone(), 100, 10).expect("line");
        maker_line.balance = 100;
        let taker_line =
            TrustLine::new(taker.clone(), issuer, asset.asset_id.clone(), 100, 10).expect("line");
        let mut ledger = LedgerState::new_with_assets(
            vec![
                Account::new(
                    maker.clone(),
                    200,
                    Some(bytes_to_hex(&maker_key.public_key)),
                ),
                Account::new(
                    taker.clone(),
                    300,
                    Some(bytes_to_hex(&taker_key.public_key)),
                ),
            ],
            vec![asset.clone()],
            vec![maker_line, taker_line],
        );
        let maker_create = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &maker_key,
            OFFER_CREATE_TRANSACTION_KIND,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: maker.clone(),
                taker_gets_asset_id: asset.asset_id.clone(),
                taker_gets_amount: 20,
                taker_pays_asset_id: "PFT".to_string(),
                taker_pays_amount: 40,
                expiration_height: 20,
            }),
            1,
        );
        assert!(execute_offer_transaction(&genesis, &mut ledger, &maker_create, 1).accepted);
        let maker_offer_id = offer_id(&genesis.chain_id, &maker, 1).expect("maker offer id");

        let taker_create = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &taker_key,
            OFFER_CREATE_TRANSACTION_KIND,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: taker.clone(),
                taker_gets_asset_id: "PFT".to_string(),
                taker_gets_amount: 80,
                taker_pays_asset_id: asset.asset_id.clone(),
                taker_pays_amount: 40,
                expiration_height: 20,
            }),
            2,
        );
        let taker_fee = taker_create.unsigned.fee;
        let taker_receipt = execute_offer_transaction(&genesis, &mut ledger, &taker_create, 2);
        assert!(taker_receipt.accepted, "{taker_receipt:?}");
        assert_eq!(taker_receipt.code, "partially_filled");
        let taker_offer_id = offer_id(&genesis.chain_id, &taker, 1).expect("taker offer id");
        assert_eq!(
            taker_receipt.offer_id.as_deref(),
            Some(taker_offer_id.as_str())
        );
        assert_eq!(taker_receipt.offer_fills.len(), 1);
        assert_eq!(taker_receipt.offer_fills[0].maker_offer_id, maker_offer_id);
        let taker_offer = ledger.offer(&taker_offer_id).expect("residual taker offer");
        assert_eq!(taker_offer.state, OFFER_STATE_OPEN);
        assert_eq!(taker_offer.taker_gets_amount_remaining, 40);
        assert_eq!(taker_offer.taker_pays_amount_remaining, 20);
        assert_eq!(taker_offer.reserve_paid, OFFER_OBJECT_RESERVE);
        assert_eq!(
            ledger.account(&taker).expect("taker").balance,
            300 - taker_fee - 80 - OFFER_OBJECT_RESERVE
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&taker, &asset.asset_id)
                .expect("taker line")
                .balance,
            20
        );
        assert_eq!(
            100,
            issued_asset_supply(&ledger, &asset.asset_id).expect("supply")
        );
    }

    #[test]
    fn offer_transactions_conserve_native_and_issued_assets_through_partial_fill_cancel_and_reject()
    {
        let genesis = Genesis::new("postfiat-local");
        let maker_key = ml_dsa_65_keygen().expect("maker keygen");
        let taker_key = ml_dsa_65_keygen().expect("taker keygen");
        let maker = address_from_public_key(&maker_key.public_key);
        let taker = address_from_public_key(&taker_key.public_key);
        let issuer = "pfissuer000000000000000000000000000000000";
        let asset = AssetDefinition::new(&genesis.chain_id, issuer, "USD", 1, 6).expect("asset");
        let mut maker_line =
            TrustLine::new(maker.clone(), issuer, asset.asset_id.clone(), 100, 10).expect("line");
        maker_line.balance = 100;
        let taker_line =
            TrustLine::new(taker.clone(), issuer, asset.asset_id.clone(), 200, 10).expect("line");
        let mut ledger = LedgerState::new_with_assets(
            vec![
                Account::new(
                    maker.clone(),
                    500,
                    Some(bytes_to_hex(&maker_key.public_key)),
                ),
                Account::new(
                    taker.clone(),
                    600,
                    Some(bytes_to_hex(&taker_key.public_key)),
                ),
            ],
            vec![asset.clone()],
            vec![maker_line, taker_line],
        );
        let initial_native_pft = native_pft_account_offer_total(&ledger);
        let initial_issued_supply =
            issued_asset_supply(&ledger, &asset.asset_id).expect("initial issued supply");
        let mut burned_fees = 0_u64;

        let maker_create = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &maker_key,
            OFFER_CREATE_TRANSACTION_KIND,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: maker.clone(),
                taker_gets_asset_id: asset.asset_id.clone(),
                taker_gets_amount: 60,
                taker_pays_asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
                taker_pays_amount: 120,
                expiration_height: 20,
            }),
            1,
        );
        let maker_create_fee = maker_create.unsigned.fee;
        let maker_receipt = execute_offer_transaction(&genesis, &mut ledger, &maker_create, 1);
        assert!(maker_receipt.accepted, "{maker_receipt:?}");
        assert_eq!(maker_receipt.code, "accepted");
        burned_fees = burned_fees
            .checked_add(maker_create_fee)
            .expect("fee total");
        let maker_offer_id = offer_id(&genesis.chain_id, &maker, 1).expect("maker offer id");
        assert_eq!(
            ledger.offer(&maker_offer_id).expect("maker offer").state,
            OFFER_STATE_OPEN
        );
        assert_offer_conservation(
            &genesis,
            &ledger,
            &asset.asset_id,
            initial_issued_supply,
            initial_native_pft - burned_fees,
        );

        let taker_exact_fill = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &taker_key,
            OFFER_CREATE_TRANSACTION_KIND,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: taker.clone(),
                taker_gets_asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
                taker_gets_amount: 80,
                taker_pays_asset_id: asset.asset_id.clone(),
                taker_pays_amount: 40,
                expiration_height: 20,
            }),
            2,
        );
        let taker_exact_fee = taker_exact_fill.unsigned.fee;
        let taker_exact_receipt =
            execute_offer_transaction(&genesis, &mut ledger, &taker_exact_fill, 2);
        assert!(taker_exact_receipt.accepted, "{taker_exact_receipt:?}");
        assert_eq!(taker_exact_receipt.code, "filled");
        assert!(taker_exact_receipt.offer_id.is_none());
        assert_eq!(taker_exact_receipt.offer_fills.len(), 1);
        assert_eq!(
            taker_exact_receipt.offer_fills[0].maker_offer_id,
            maker_offer_id
        );
        assert_eq!(taker_exact_receipt.offer_fills[0].maker_sends_amount, 40);
        assert_eq!(taker_exact_receipt.offer_fills[0].taker_sends_amount, 80);
        let maker_offer = ledger.offer(&maker_offer_id).expect("maker offer");
        assert_eq!(maker_offer.state, OFFER_STATE_OPEN);
        assert_eq!(maker_offer.taker_gets_amount_remaining, 20);
        assert_eq!(maker_offer.taker_pays_amount_remaining, 40);
        burned_fees = burned_fees.checked_add(taker_exact_fee).expect("fee total");
        assert_offer_conservation(
            &genesis,
            &ledger,
            &asset.asset_id,
            initial_issued_supply,
            initial_native_pft - burned_fees,
        );

        let taker_partial_fill = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &taker_key,
            OFFER_CREATE_TRANSACTION_KIND,
            2,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: taker.clone(),
                taker_gets_asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
                taker_gets_amount: 100,
                taker_pays_asset_id: asset.asset_id.clone(),
                taker_pays_amount: 50,
                expiration_height: 20,
            }),
            3,
        );
        let taker_partial_fee = taker_partial_fill.unsigned.fee;
        let taker_partial_receipt =
            execute_offer_transaction(&genesis, &mut ledger, &taker_partial_fill, 3);
        assert!(taker_partial_receipt.accepted, "{taker_partial_receipt:?}");
        assert_eq!(taker_partial_receipt.code, "partially_filled");
        assert_eq!(taker_partial_receipt.offer_fills.len(), 1);
        assert_eq!(
            taker_partial_receipt.offer_fills[0]
                .terminal_maker_state
                .as_deref(),
            Some(OFFER_STATE_FILLED)
        );
        burned_fees = burned_fees
            .checked_add(taker_partial_fee)
            .expect("fee total");
        let taker_residual_offer_id =
            offer_id(&genesis.chain_id, &taker, 2).expect("taker residual offer id");
        assert_eq!(
            taker_partial_receipt.offer_id.as_deref(),
            Some(taker_residual_offer_id.as_str())
        );
        let maker_offer = ledger.offer(&maker_offer_id).expect("maker offer");
        assert_eq!(maker_offer.state, OFFER_STATE_FILLED);
        assert_eq!(maker_offer.reserve_paid, 0);
        let taker_offer = ledger
            .offer(&taker_residual_offer_id)
            .expect("taker residual offer");
        assert_eq!(taker_offer.state, OFFER_STATE_OPEN);
        assert_eq!(taker_offer.taker_gets_amount_remaining, 60);
        assert_eq!(taker_offer.taker_pays_amount_remaining, 30);
        assert_eq!(taker_offer.reserve_paid, OFFER_OBJECT_RESERVE);
        assert_offer_conservation(
            &genesis,
            &ledger,
            &asset.asset_id,
            initial_issued_supply,
            initial_native_pft - burned_fees,
        );

        let cancel_residual = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &taker_key,
            OFFER_CANCEL_TRANSACTION_KIND,
            3,
            OfferTransactionOperation::OfferCancel(OfferCancelOperation {
                offer_id: taker_residual_offer_id.clone(),
                owner: taker.clone(),
            }),
            4,
        );
        let cancel_fee = cancel_residual.unsigned.fee;
        let cancel_receipt = execute_offer_transaction(&genesis, &mut ledger, &cancel_residual, 4);
        assert!(cancel_receipt.accepted, "{cancel_receipt:?}");
        assert_eq!(cancel_receipt.code, "accepted");
        burned_fees = burned_fees.checked_add(cancel_fee).expect("fee total");
        let taker_offer = ledger
            .offer(&taker_residual_offer_id)
            .expect("canceled taker offer");
        assert_eq!(taker_offer.state, OFFER_STATE_CANCELED);
        assert_eq!(taker_offer.reserve_paid, 0);
        assert_eq!(
            ledger.account(&maker).expect("maker").balance,
            500 - maker_create_fee + 120
        );
        assert_eq!(
            ledger.account(&taker).expect("taker").balance,
            600 - taker_exact_fee - 80 - taker_partial_fee - 40 - cancel_fee
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&maker, &asset.asset_id)
                .expect("maker line")
                .balance,
            40
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&taker, &asset.asset_id)
                .expect("taker line")
                .balance,
            60
        );
        assert_offer_conservation(
            &genesis,
            &ledger,
            &asset.asset_id,
            initial_issued_supply,
            initial_native_pft - burned_fees,
        );

        let reject_cancel_filled_maker = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &maker_key,
            OFFER_CANCEL_TRANSACTION_KIND,
            2,
            OfferTransactionOperation::OfferCancel(OfferCancelOperation {
                offer_id: maker_offer_id,
                owner: maker,
            }),
            5,
        );
        let before_reject = ledger.clone();
        let reject_receipt =
            execute_offer_transaction(&genesis, &mut ledger, &reject_cancel_filled_maker, 5);
        assert!(!reject_receipt.accepted);
        assert_eq!(reject_receipt.code, "offer_not_open");
        assert_eq!(ledger, before_reject);
        assert_offer_conservation(
            &genesis,
            &ledger,
            &asset.asset_id,
            initial_issued_supply,
            initial_native_pft - burned_fees,
        );
    }

    #[test]
    fn offer_create_rejects_issued_buy_capacity_over_limit() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let owner = address_from_public_key(&key_pair.public_key);
        let issuer = "pfissuer000000000000000000000000000000000";
        let asset = AssetDefinition::new(&genesis.chain_id, issuer, "USD", 1, 6).expect("asset");
        let mut trustline =
            TrustLine::new(owner.clone(), issuer, asset.asset_id.clone(), 50, 10).expect("line");
        trustline.balance = 30;
        let mut ledger = LedgerState::new_with_assets(
            vec![Account::new(
                owner.clone(),
                200,
                Some(bytes_to_hex(&key_pair.public_key)),
            )],
            vec![asset.clone()],
            vec![trustline],
        );
        let create = signed_offer_transaction(
            &genesis,
            &key_pair,
            OFFER_CREATE_TRANSACTION_KIND,
            100,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner,
                taker_gets_asset_id: "PFT".to_string(),
                taker_gets_amount: 10,
                taker_pays_asset_id: asset.asset_id,
                taker_pays_amount: 30,
                expiration_height: 20,
            }),
        );
        let receipt = execute_offer_transaction(&genesis, &mut ledger, &create, 1);
        assert!(!receipt.accepted);
        assert_eq!("trustline_limit_exceeded", receipt.code);
        assert!(ledger.offers.is_empty());
    }

    #[test]
    fn offer_create_rejection_does_not_lock_partial_reserve() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let owner = address_from_public_key(&key_pair.public_key);
        let issuer = "pfissuer000000000000000000000000000000000";
        let asset = AssetDefinition::new(&genesis.chain_id, issuer, "USD", 1, 6).expect("asset");
        let trustline =
            TrustLine::new(owner.clone(), issuer, asset.asset_id.clone(), 100, 10).expect("line");
        let mut ledger = LedgerState::new_with_assets(
            vec![Account::new(
                owner.clone(),
                200,
                Some(bytes_to_hex(&key_pair.public_key)),
            )],
            vec![asset.clone()],
            vec![trustline],
        );
        let create = signed_offer_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &key_pair,
            OFFER_CREATE_TRANSACTION_KIND,
            1,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: owner.clone(),
                taker_gets_asset_id: "PFT".to_string(),
                taker_gets_amount: 1,
                taker_pays_asset_id: asset.asset_id,
                taker_pays_amount: 10,
                expiration_height: 20,
            }),
            1,
        );
        ledger.account_mut(&owner).expect("owner").balance =
            ACCOUNT_RESERVE + OFFER_OBJECT_RESERVE + create.unsigned.fee;
        let before = ledger.clone();

        let receipt = execute_offer_transaction(&genesis, &mut ledger, &create, 1);
        assert!(!receipt.accepted);
        assert_eq!("below_account_reserve", receipt.code);
        assert_eq!(ledger, before);
        assert!(ledger.offers.is_empty());
    }

    #[test]
    fn transfer_charges_state_expansion_fee_only_for_new_recipient_accounts() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let from = address_from_public_key(&key_pair.public_key);
        let existing = "pfexistingrecipient000000000000000000000".to_string();
        let new_recipient = "pfnewrecipient0000000000000000000000000".to_string();
        let mut ledger = LedgerState::new(vec![
            Account::new(from.clone(), 200, Some(public_key_hex.clone())),
            Account::new(existing.clone(), ACCOUNT_RESERVE, None),
        ]);

        let underpriced_new = signed_transfer(
            &genesis,
            &key_pair,
            new_recipient.clone(),
            ACCOUNT_RESERVE,
            MIN_TRANSFER_FEE,
            1,
        );
        let base_fee = minimum_transfer_fee(&underpriced_new);
        let underpriced_new = signed_transfer(
            &genesis,
            &key_pair,
            new_recipient.clone(),
            ACCOUNT_RESERVE,
            base_fee,
            1,
        );
        let underpriced_receipt = execute_transfer(&genesis, &mut ledger.clone(), &underpriced_new);
        assert!(!underpriced_receipt.accepted);
        assert_eq!(underpriced_receipt.code, "fee_too_low");
        assert_eq!(
            underpriced_receipt.state_expansion_fee,
            TRANSFER_ACCOUNT_CREATION_FEE
        );
        assert_eq!(
            underpriced_receipt.minimum_fee,
            minimum_transfer_fee(&underpriced_new) + TRANSFER_ACCOUNT_CREATION_FEE
        );

        let funded_new = signed_transfer_with_minimum_fee(
            &genesis,
            &key_pair,
            new_recipient,
            ACCOUNT_RESERVE,
            1,
        );
        let funded_new_receipt = execute_transfer(&genesis, &mut ledger, &funded_new);
        assert!(funded_new_receipt.accepted, "{funded_new_receipt:?}");
        assert_eq!(
            funded_new_receipt.state_expansion_fee,
            TRANSFER_ACCOUNT_CREATION_FEE
        );

        let existing_probe = signed_transfer(
            &genesis,
            &key_pair,
            existing.clone(),
            1,
            MIN_TRANSFER_FEE,
            2,
        );
        let existing_minimum = minimum_transfer_fee_for_ledger(&ledger, &existing_probe);
        let existing_transfer =
            signed_transfer(&genesis, &key_pair, existing, 1, existing_minimum, 2);
        let existing_receipt = execute_transfer(&genesis, &mut ledger, &existing_transfer);
        assert!(existing_receipt.accepted, "{existing_receipt:?}");
        assert_eq!(existing_receipt.state_expansion_fee, 0);
    }

    #[test]
    fn first_spend_binds_funded_account_public_key() {
        let genesis = Genesis::new("postfiat-local");
        let faucet_key_pair = ml_dsa_65_keygen().expect("faucet keygen");
        let faucet_public_key_hex = bytes_to_hex(&faucet_key_pair.public_key);
        let faucet = address_from_public_key(&faucet_key_pair.public_key);
        let wallet_key_pair = ml_dsa_65_keygen().expect("wallet keygen");
        let wallet_public_key_hex = bytes_to_hex(&wallet_key_pair.public_key);
        let wallet = address_from_public_key(&wallet_key_pair.public_key);
        let mut ledger =
            LedgerState::new(vec![Account::new(faucet, 150, Some(faucet_public_key_hex))]);

        let funding =
            signed_transfer_with_minimum_fee(&genesis, &faucet_key_pair, wallet.clone(), 70, 1);
        let funding_receipt = execute_transfer(&genesis, &mut ledger, &funding);
        assert!(funding_receipt.accepted, "{funding_receipt:?}");
        assert_eq!(ledger.account(&wallet).unwrap().balance, 70);
        assert_eq!(ledger.account(&wallet).unwrap().public_key_hex, None);

        let spend = signed_transfer_with_minimum_fee(
            &genesis,
            &wallet_key_pair,
            "pfwalletrecipient0000000000000000000001".to_string(),
            15,
            1,
        );
        let spend_receipt = execute_transfer(&genesis, &mut ledger, &spend);

        assert!(spend_receipt.accepted, "{spend_receipt:?}");
        assert_eq!(
            ledger.account(&wallet).unwrap().public_key_hex.as_deref(),
            Some(wallet_public_key_hex.as_str())
        );
        assert_eq!(ledger.account(&wallet).unwrap().sequence, 1);
    }

    #[test]
    fn transfer_rejects_missing_fee() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let from = address_from_public_key(&key_pair.public_key);
        let mut ledger = LedgerState::new(vec![Account::new(
            from.clone(),
            100,
            Some(public_key_hex.clone()),
        )]);

        let transfer = signed_transfer(
            &genesis,
            &key_pair,
            "bridge-recipient-000000000000000000000000".to_string(),
            25,
            0,
            1,
        );

        let receipt = execute_transfer(&genesis, &mut ledger, &transfer);

        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "fee_too_low");
    }

    #[test]
    fn transfer_rejects_sender_or_recipient_below_reserve() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let from = address_from_public_key(&key_pair.public_key);

        let mut sender_reserve_ledger = LedgerState::new(vec![Account::new(
            from.clone(),
            100,
            Some(public_key_hex.clone()),
        )]);
        let sender_reserve_transfer = signed_transfer_with_minimum_fee(
            &genesis,
            &key_pair,
            "bridge-recipient-000000000000000000000000".to_string(),
            25,
            1,
        );
        sender_reserve_ledger.accounts[0].balance =
            ACCOUNT_RESERVE + 25 + sender_reserve_transfer.unsigned.fee - 1;
        let sender_receipt = execute_transfer(
            &genesis,
            &mut sender_reserve_ledger,
            &sender_reserve_transfer,
        );
        assert!(!sender_receipt.accepted);
        assert_eq!(sender_receipt.code, "below_account_reserve");

        let mut recipient_reserve_ledger =
            LedgerState::new(vec![Account::new(from, 100, Some(public_key_hex))]);
        let recipient_reserve_transfer = signed_transfer_with_minimum_fee(
            &genesis,
            &key_pair,
            "pfdustrecipient000000000000000000000000".to_string(),
            ACCOUNT_RESERVE - 1,
            1,
        );
        let recipient_receipt = execute_transfer(
            &genesis,
            &mut recipient_reserve_ledger,
            &recipient_reserve_transfer,
        );
        assert!(!recipient_receipt.accepted);
        assert_eq!(recipient_receipt.code, "below_account_reserve");
    }

    #[test]
    fn transfer_to_fee_collector_keeps_amount_and_burns_fee() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let from = address_from_public_key(&key_pair.public_key);
        let mut ledger = LedgerState::new(vec![Account::new(
            from.clone(),
            100,
            Some(public_key_hex.clone()),
        )]);

        let transfer = signed_transfer_with_minimum_fee(
            &genesis,
            &key_pair,
            FEE_COLLECTOR_ADDRESS.to_string(),
            25,
            1,
        );

        let receipt = execute_transfer(&genesis, &mut ledger, &transfer);

        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(ledger.account(FEE_COLLECTOR_ADDRESS).unwrap().balance, 25);
        assert_eq!(receipt.fee_burned, transfer.unsigned.fee);
    }

    #[test]
    fn transfer_rejects_wrong_signed_domain_fields() {
        let genesis = Genesis::new("postfiat-local");
        let key_pair = ml_dsa_65_keygen().expect("keygen");
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let from = address_from_public_key(&key_pair.public_key);
        let ledger = LedgerState::new(vec![Account::new(
            from.clone(),
            100,
            Some(public_key_hex.clone()),
        )]);
        let transfer = signed_transfer_with_minimum_fee(
            &genesis,
            &key_pair,
            "bridge-recipient-000000000000000000000000".to_string(),
            25,
            1,
        );

        let mut wrong_genesis = transfer.clone();
        wrong_genesis.unsigned.genesis_hash = "e".repeat(96);
        assert_eq!(
            execute_transfer(&genesis, &mut ledger.clone(), &wrong_genesis).code,
            "wrong_genesis"
        );

        let mut wrong_protocol = transfer.clone();
        wrong_protocol.unsigned.protocol_version += 1;
        assert_eq!(
            execute_transfer(&genesis, &mut ledger.clone(), &wrong_protocol).code,
            "wrong_protocol_version"
        );

        let mut wrong_namespace = transfer.clone();
        wrong_namespace.unsigned.address_namespace = "wrong".to_string();
        assert_eq!(
            execute_transfer(&genesis, &mut ledger.clone(), &wrong_namespace).code,
            "wrong_address_namespace"
        );

        let mut wrong_kind = transfer.clone();
        wrong_kind.unsigned.transaction_kind = "wrong".to_string();
        assert_eq!(
            execute_transfer(&genesis, &mut ledger.clone(), &wrong_kind).code,
            "wrong_transaction_kind"
        );

        let mut mismatched_algorithm = transfer;
        mismatched_algorithm.algorithm_id = "wrong".to_string();
        assert_eq!(
            execute_transfer(&genesis, &mut ledger.clone(), &mismatched_algorithm).code,
            "signature_algorithm_mismatch"
        );

        let mut malformed_domain = signed_transfer_with_minimum_fee(
            &genesis,
            &key_pair,
            "bridge-recipient-000000000000000000000000".to_string(),
            25,
            1,
        );
        malformed_domain.unsigned.chain_id = " postfiat-local".to_string();
        assert_eq!(
            execute_transfer(&genesis, &mut ledger.clone(), &malformed_domain).code,
            "bad_transfer_envelope"
        );
    }

    #[test]
    fn nav_sp1_groth16_submit_rejects_missing_proof_and_accepts_known_good_fixture() {
        const FIXTURE_DIR: &str = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sp1-aggregate-regen-monero-crypto"
        );
        let public_values =
            std::fs::read(format!("{FIXTURE_DIR}/aggregate-public-values.bin")).expect("pv");
        let proof =
            std::fs::read(format!("{FIXTURE_DIR}/aggregate-proof-calldata.bin")).expect("proof");
        let verified_net_assets = 2_364_869_341_670u64;
        let circulating_supply = 4_000u64;
        let nav_per_unit =
            nav_per_unit_floor(verified_net_assets, circulating_supply).expect("floor nav");
        assert_eq!(nav_per_unit, 591_217_335);
        assert!(verified_net_assets > circulating_supply * nav_per_unit);

        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let mut ledger = LedgerState::new(vec![Account::new(
            issuer.clone(),
            10_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        )]);

        let register_profile = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_PROFILE_REGISTER_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
                registrant: issuer.clone(),
                verifier_kind: NAV_PROFILE_VERIFIER_SP1_GROTH16.to_string(),
                source_class: "stakehub-pol-v2".to_string(),
                min_attestations: 0,
                tolerance_bp: 0,
                bridge_observer_min_confirmations: 0,
                valuation_policy_hash:
                    "8fcf3cd44c8180744563e85579ed91b7fd3882e560dc41ea4dc0c18cb01f289d".to_string(),
                vault_bridge_route_policy_hash: String::new(),
                max_snapshot_age_blocks: 100_000,
                challenge_window_blocks: 1,
                max_epoch_gap_blocks: 100_000,
                settle_deadline_blocks: 0,
                min_challenge_bond: 0,
                sp1_program_vkey:
                    "0x004d1cd3f36e6ea60662af428edbea9d3aba45f04fe496da909d6bbe9fbf9258".to_string(),
                sp1_proof_encoding: "groth16".to_string(),
                max_proof_bytes: 0,
                max_public_values_bytes: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_profile, 1).accepted);
        let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

        let create_asset = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "a651".to_string(),
                version: 1,
                precision: 6,
                display_name: "a651 sp1 test".to_string(),
                max_supply: Some(4_000_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &create_asset, 1).accepted);
        let asset_id = ledger.asset_definitions[0].asset_id.clone();

        let register_nav = signed_asset_transaction_with_minimum_fee(
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
                valuation_unit: "usd_1e8".to_string(),
                redemption_account: issuer.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_nav, 1).accepted);

        let missing_proof = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit,
                circulating_supply: circulating_supply,
                verified_net_assets,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: "03".repeat(48),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        );
        let missing = execute_asset_transaction(&genesis, &mut ledger, &missing_proof, 2);
        assert!(!missing.accepted);
        assert_eq!(missing.code, "missing_sp1_proof");

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
                nav_per_unit,
                circulating_supply,
                verified_net_assets,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: "04".repeat(48),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: proof.clone(),
                sp1_public_values: public_values.clone(),
            }),
        );
        let submit_receipt = execute_asset_transaction(&genesis, &mut ledger, &submit, 2);
        assert!(
            submit_receipt.accepted,
            "submit rejected: code={} message={}",
            submit_receipt.code,
            submit_receipt.message
        );
        assert_eq!(ledger.nav_reserve_packets[0].sp1_proof_bytes.len(), 356);
        assert_eq!(
            ledger.nav_reserve_packets[0].verified_net_assets,
            verified_net_assets
        );

        let create_settlement_asset = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "PFUSDC".to_string(),
                version: 1,
                precision: 6,
                display_name: "pfUSDC test".to_string(),
                max_supply: None,
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &create_settlement_asset, 3).accepted);
        let settlement_asset_id = ledger.asset_definitions[1].asset_id.clone();

        let register_settlement_nav = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_ASSET_REGISTER_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
                issuer: issuer.clone(),
                asset_id: settlement_asset_id.clone(),
                reserve_operator: issuer.clone(),
                proof_profile: "vault-bridge-test".to_string(),
                valuation_unit: "USDC".to_string(),
                redemption_account: issuer.clone(),
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_settlement_nav, 4).accepted);

        let evidence = vault_bridge_evidence(5_082_364, "77");
        let policy_hash =
            "853b8c0478fbfdf488a48f48ca58c4dde5decef53a68c260aa44ebdc44eeb9fffdee81431370c2b091d9819042655daa".to_string();
        let mut receipt = VaultBridgeReceipt::new(
            &genesis.chain_id,
            settlement_asset_id.clone(),
            evidence.source_domain(),
            evidence.source_asset_ref(),
            VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT,
            evidence.amount_atoms,
            evidence.source_tx_or_attestation(),
            evidence.finality_ref(),
            evidence.vault_id(),
            policy_hash.clone(),
            5,
            1_000_000,
            Some(evidence),
        )
        .expect("receipt");
        receipt.status = VAULT_BRIDGE_RECEIPT_STATUS_COUNTED.to_string();
        receipt.haircut_bps = 0;
        receipt.counted_value_atoms = 5_082_364;
        receipt.allocated_value_atoms = 5_082_364;
        receipt.finalized_at_height = 5;
        receipt.counted_at_height = 5;
        receipt.validate_for_chain(&genesis.chain_id).expect("valid receipt");

        let mut bucket = VaultBridgeBucketState::new(
            settlement_asset_id.clone(),
            receipt.source_domain.clone(),
            policy_hash,
            6,
        )
        .expect("bucket");
        bucket.gross_receipt_atoms = 5_082_364;
        bucket.counted_value_atoms = 5_082_364;
        bucket.nav_subscription_allocations_atoms = 5_082_364;
        bucket.validate().expect("valid bucket");

        let mut allocation = VaultBridgeAllocation::new(
            &genesis.chain_id,
            receipt.receipt_id.clone(),
            settlement_asset_id.clone(),
            bucket.bucket_id.clone(),
            5_082_364,
            VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,
            nav_subscription_consumer_id(&asset_id),
            6,
        )
        .expect("allocation");
        allocation.retired_at_height = 7;
        allocation.validate_for_chain(&genesis.chain_id).expect("valid allocation");

        ledger.vault_bridge_bucket_states.push(bucket);
        ledger.vault_bridge_receipts.push(receipt);
        ledger.vault_bridge_allocations.push(allocation);

        let nav_asset = ledger.nav_asset(&asset_id).expect("nav asset").clone();
        let profile = ledger.nav_proof_profile(&profile_id).expect("profile").clone();
        let overlay = nav_subscription_reserve_overlay(&ledger, &nav_asset)
            .expect("overlay")
            .expect("overlay present");
        assert_eq!(overlay.value_nav_units, 508_236_400);
        let decoded = verify_sp1_groth16(&profile, verified_net_assets, &proof, &public_values)
            .expect("base sp1 proof verifies");
        let composite_source_root = nav_sp1_subscription_source_root(
            &nav_asset,
            &profile,
            &decoded,
            &public_values,
            &overlay,
        )
        .expect("composite source root");
        let verified_with_subscription = verified_net_assets + overlay.value_nav_units;
        let nav_per_unit_with_subscription =
            nav_per_unit_floor(verified_with_subscription, circulating_supply)
                .expect("subscription floor nav");
        assert_eq!(verified_with_subscription, 2_365_377_578_070);
        assert_eq!(nav_per_unit_with_subscription, 591_344_394);

        let bad_source_root = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit: nav_per_unit_with_subscription,
                circulating_supply,
                verified_net_assets: verified_with_subscription,
                proof_profile: profile_id.clone(),
                source_root: "01".repeat(48),
                attestor_root: "02".repeat(48),
                reserve_packet_hash: "05".repeat(48),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: proof.clone(),
                sp1_public_values: public_values.clone(),
            }),
        );
        let bad_receipt = execute_asset_transaction(&genesis, &mut ledger, &bad_source_root, 8);
        assert!(!bad_receipt.accepted);
        assert_eq!(bad_receipt.code, "nav_subscription_source_root_mismatch");

        let submit_with_subscription = signed_asset_transaction_with_minimum_fee(
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
                nav_per_unit: nav_per_unit_with_subscription,
                circulating_supply,
                verified_net_assets: verified_with_subscription,
                proof_profile: profile_id,
                source_root: composite_source_root,
                attestor_root: "02".repeat(48),
                reserve_packet_hash: "06".repeat(48),
                reserve_accounts: Vec::new(),
                sp1_proof_bytes: proof,
                sp1_public_values: public_values,
            }),
        );
        let overlay_receipt =
            execute_asset_transaction(&genesis, &mut ledger, &submit_with_subscription, 8);
        assert!(
            overlay_receipt.accepted,
            "overlay submit rejected: code={} message={}",
            overlay_receipt.code,
            overlay_receipt.message
        );
        assert_eq!(
            ledger.nav_reserve_packets[1].verified_net_assets,
            verified_with_subscription
        );
    }

    #[test]
    fn legacy_nav_profile_register_block_3_receipt_id_matches_committed() {
        use std::path::Path;
        use postfiat_types::TransactionBatch;

        // Use the canonical tracked catch-up fixture rather than the operator-only
        // report tree, so the legacy receipt vector is reproducible in a clean clone.
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../node/testdata/wan-devnet-catchup-block-3/batch.json");
        let batch_json = std::fs::read_to_string(&fixture).unwrap_or_else(|error| {
            panic!(
                "read block-3 batch fixture at {}: {error}",
                fixture.display()
            )
        });
        let batch: TransactionBatch =
            serde_json::from_str(&batch_json).expect("parse block-3 nav profile batch");
        let transaction = batch
            .asset_transactions
            .first()
            .expect("block-3 batch includes nav profile register");
        assert_eq!(
            asset_transaction_tx_id(transaction),
            "c652ae315cd2b5b7664ebd5a3c3b3143456c9dd36158b02f4e2c1cb265c48bbb90230f6345d2e5871ef6f183e561a613"
        );
    }

    fn vault_bridge_stage1_fixture(
        source_domain: String,
        policy_hash: String,
    ) -> (LedgerState, String, String) {
        let issuer = "stage1-issuer".to_string();
        let profile = postfiat_types::NavProofProfile::new(
            issuer.clone(),
            NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
            format!("vault_bridge:{source_domain}"),
            100,
            0,
            100,
            0,
            0,
            1,
            0,
            policy_hash.clone(),
            String::new(),
            String::new(),
            0,
            0,
        )
        .expect("stage1 profile");
        let profile_id = profile.profile_id.clone();
        let asset_id = "51".repeat(48);
        let nav_asset = NavTrackedAsset::new(
            asset_id.clone(),
            issuer.clone(),
            issuer.clone(),
            profile_id,
            "SOURCE_UNIT",
            issuer,
        )
        .expect("stage1 nav asset");
        let mut ledger = LedgerState::new(Vec::new());
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_assets.push(nav_asset);
        (ledger, asset_id, policy_hash)
    }

    fn vault_bridge_stage1_propose_operation(
        asset_id: String,
        policy_hash: String,
        evidence: VaultBridgeDepositEvidence,
    ) -> VaultBridgeDepositProposeOperation {
        let evidence_root =
            vault_bridge_deposit_evidence_root(&evidence).unwrap_or_else(|_| "f1".repeat(48));
        VaultBridgeDepositProposeOperation {
            proposer: "stage1-proposer".to_string(),
            asset_id,
            evidence_root,
            evidence,
            policy_hash,
            source_proof_kind: String::new(),
            source_proof_hash: String::new(),
            source_public_values_hash: String::new(),
            source_proof_bytes: Vec::new(),
            source_public_values: Vec::new(),
            expires_at_height: 1_000,
        }
    }

    fn vault_bridge_stage2_fixture(
        min_attestations: u64,
        min_confirmations: u64,
    ) -> (LedgerState, String, String, VaultBridgeDepositEvidence, String) {
        let evidence = vault_bridge_evidence(1_000_000, "d1");
        let policy_hash = "42".repeat(48);
        let issuer = "stage2-issuer".to_string();
        let profile = postfiat_types::NavProofProfile::new_with_bridge_observer_min_confirmations(
            issuer.clone(),
            NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
            format!("vault_bridge:{}", evidence.source_domain()),
            100,
            0,
            100,
            0,
            0,
            min_attestations,
            0,
            min_confirmations,
            policy_hash.clone(),
            String::new(),
            String::new(),
            0,
            0,
        )
        .expect("stage2 vault bridge profile");
        let profile_id = profile.profile_id.clone();
        let asset = AssetDefinition::new("postfiat-local", &issuer, "pfUSDC", 1, 6)
            .expect("stage2 vault bridge asset");
        let asset_id = asset.asset_id.clone();
        let nav_asset = NavTrackedAsset::new(
            asset_id.clone(),
            issuer.clone(),
            issuer.clone(),
            profile_id,
            "SOURCE_UNIT",
            issuer,
        )
        .expect("stage2 nav asset");
        let evidence_root = vault_bridge_deposit_evidence_root(&evidence)
            .expect("stage2 evidence root");
        let mut ledger = LedgerState::new(Vec::new());
        ledger.asset_definitions.push(asset);
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_assets.push(nav_asset);
        ledger.nav_attestors.push(NavAttestor {
            address: "stage2-observer".to_string(),
            domain: "stage2.local".to_string(),
            bond: 0,
            registered_at_height: 1,
        });
        (ledger, asset_id, policy_hash, evidence, evidence_root)
    }

    fn vault_bridge_stage2_attest_operation(
        asset_id: String,
        evidence_root: String,
        observation: VaultBridgeDepositObservation,
    ) -> VaultBridgeDepositAttestOperation {
        let observation_root =
            vault_bridge_deposit_observation_root(&observation).expect("observation root");
        VaultBridgeDepositAttestOperation {
            attestor: "stage2-observer".to_string(),
            asset_id,
            evidence_root,
            pass: true,
            observation_root,
            observation: Some(observation),
        }
    }

    #[test]
    fn vault_bridge_stage2_deposit_observation_quorum_finalizes() {
        let (mut ledger, asset_id, policy_hash, evidence, evidence_root) =
            vault_bridge_stage2_fixture(1, 6);
        let propose = vault_bridge_stage1_propose_operation(
            asset_id.clone(),
            policy_hash,
            evidence.clone(),
        );
        apply_vault_bridge_deposit_propose(&mut ledger, &propose, 1)
            .expect("stage2 propose");
        let observation = VaultBridgeDepositObservation::success_for_evidence(&evidence, 6);
        let attest = vault_bridge_stage2_attest_operation(
            asset_id.clone(),
            evidence_root.clone(),
            observation,
        );
        apply_vault_bridge_deposit_attest(&mut ledger, &attest, 2)
            .expect("stage2 attest");
        apply_vault_bridge_deposit_finalize(
            &mut ledger,
            &VaultBridgeDepositFinalizeOperation {
                finalizer: "stage2-finalizer".to_string(),
                asset_id: asset_id.clone(),
                evidence_root: evidence_root.clone(),
            },
            3,
        )
        .expect("stage2 finalize");
        assert_eq!(ledger.vault_bridge_deposits[0].status, VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED);
    }

    #[test]
    fn vault_bridge_stage2_deposit_observation_mismatch_rejected() {
        let (mut ledger, asset_id, policy_hash, evidence, evidence_root) =
            vault_bridge_stage2_fixture(1, 6);
        let propose = vault_bridge_stage1_propose_operation(
            asset_id.clone(),
            policy_hash,
            evidence.clone(),
        );
        apply_vault_bridge_deposit_propose(&mut ledger, &propose, 1)
            .expect("stage2 propose");
        let mut observation = VaultBridgeDepositObservation::success_for_evidence(&evidence, 6);
        observation.amount_atoms += 1;
        let attest = vault_bridge_stage2_attest_operation(asset_id, evidence_root, observation);
        let err = apply_vault_bridge_deposit_attest(&mut ledger, &attest, 2)
            .expect_err("mismatched observation");
        assert_eq!(err.0, "vault_bridge_deposit_observation_mismatch");
    }

    #[test]
    fn vault_bridge_stage2_deposit_confirmation_depth_enforced() {
        let (mut ledger, asset_id, policy_hash, evidence, evidence_root) =
            vault_bridge_stage2_fixture(1, 6);
        let propose = vault_bridge_stage1_propose_operation(
            asset_id.clone(),
            policy_hash,
            evidence.clone(),
        );
        apply_vault_bridge_deposit_propose(&mut ledger, &propose, 1)
            .expect("stage2 propose");
        let observation = VaultBridgeDepositObservation::success_for_evidence(&evidence, 5);
        let attest = vault_bridge_stage2_attest_operation(asset_id, evidence_root, observation);
        let err = apply_vault_bridge_deposit_attest(&mut ledger, &attest, 2)
            .expect_err("shallow observation");
        assert_eq!(err.0, "vault_bridge_deposit_confirmation_depth_too_low");
    }

    #[test]
    fn vault_bridge_stage2_deposit_unregistered_observer_rejected() {
        let (mut ledger, asset_id, policy_hash, evidence, evidence_root) =
            vault_bridge_stage2_fixture(1, 6);
        ledger.nav_attestors.clear();
        let propose = vault_bridge_stage1_propose_operation(
            asset_id.clone(),
            policy_hash,
            evidence.clone(),
        );
        apply_vault_bridge_deposit_propose(&mut ledger, &propose, 1)
            .expect("stage2 propose");
        let observation = VaultBridgeDepositObservation::success_for_evidence(&evidence, 6);
        let attest = vault_bridge_stage2_attest_operation(asset_id, evidence_root, observation);
        let err = apply_vault_bridge_deposit_attest(&mut ledger, &attest, 2)
            .expect_err("unregistered observer");
        assert_eq!(err.0, "unregistered_nav_attestor");
    }

    #[test]
    fn vault_bridge_stage2_deposit_duplicate_observer_rejected() {
        let (mut ledger, asset_id, policy_hash, evidence, evidence_root) =
            vault_bridge_stage2_fixture(1, 6);
        let propose = vault_bridge_stage1_propose_operation(
            asset_id.clone(),
            policy_hash,
            evidence.clone(),
        );
        apply_vault_bridge_deposit_propose(&mut ledger, &propose, 1)
            .expect("stage2 propose");
        let observation = VaultBridgeDepositObservation::success_for_evidence(&evidence, 6);
        let attest = vault_bridge_stage2_attest_operation(
            asset_id,
            evidence_root,
            observation,
        );
        apply_vault_bridge_deposit_attest(&mut ledger, &attest, 2)
            .expect("first observer");
        let err = apply_vault_bridge_deposit_attest(&mut ledger, &attest, 3)
            .expect_err("duplicate observer");
        assert_eq!(err.0, "duplicate_vault_bridge_deposit_attestation");
    }

    #[test]
    fn vault_bridge_stage1_finality_ref_chain_id_mismatch_rejected() {
        let evidence = vault_bridge_evidence(1_000_000, "b1");
        let mismatched_source_domain = format!(
            "erc20_bridge_vault:1:{}:{}",
            evidence.vault_address, evidence.token_address
        );
        let (mut ledger, asset_id, policy_hash) =
            vault_bridge_stage1_fixture(mismatched_source_domain, "42".repeat(48));
        let operation = vault_bridge_stage1_propose_operation(asset_id, policy_hash, evidence);

        let err =
            apply_vault_bridge_deposit_propose(&mut ledger, &operation, 1).expect_err("mismatch");
        assert_eq!(err.0, "vault_bridge_finality_ref_chain_id_mismatch");
    }

    #[test]
    fn vault_bridge_stage1_evidence_policy_mismatch_rejected() {
        let evidence = vault_bridge_evidence(1_000_000, "b2");
        let mismatched_source_domain = format!(
            "erc20_bridge_vault:{}:{}:{}",
            evidence.source_chain_id,
            evidence.vault_address,
            "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
        let (mut ledger, asset_id, policy_hash) =
            vault_bridge_stage1_fixture(mismatched_source_domain, "42".repeat(48));
        let operation = vault_bridge_stage1_propose_operation(asset_id, policy_hash, evidence);

        let err =
            apply_vault_bridge_deposit_propose(&mut ledger, &operation, 1).expect_err("mismatch");
        assert_eq!(err.0, "vault_bridge_evidence_policy_mismatch");
    }

    #[test]
    fn vault_bridge_stage1_zero_amount_rejected() {
        let mut evidence = vault_bridge_evidence(1_000_000, "b3");
        evidence.amount_atoms = 0;
        evidence.deposit_id = "f2".repeat(32);
        let (mut ledger, asset_id, policy_hash) =
            vault_bridge_stage1_fixture(evidence.source_domain(), "42".repeat(48));
        let operation = vault_bridge_stage1_propose_operation(asset_id, policy_hash, evidence);

        let err = apply_vault_bridge_deposit_propose(&mut ledger, &operation, 1).expect_err("zero");
        assert_eq!(err.0, "vault_bridge_zero_amount");
    }

    #[test]
    fn vault_bridge_stage1_deposit_id_replay_rejected() {
        let first = vault_bridge_evidence(1_000_000, "b5");
        let mut replay = first.clone();
        replay.block_hash = "66".repeat(32);
        let replay_root = vault_bridge_deposit_evidence_root(&replay).expect("replay evidence root");
        let (mut ledger, asset_id, policy_hash) =
            vault_bridge_stage1_fixture(first.source_domain(), "42".repeat(48));
        let first_operation =
            vault_bridge_stage1_propose_operation(asset_id.clone(), policy_hash.clone(), first);
        apply_vault_bridge_deposit_propose(&mut ledger, &first_operation, 1).expect("first propose");

        let mut replay_operation = vault_bridge_stage1_propose_operation(asset_id, policy_hash, replay);
        replay_operation.evidence_root = replay_root;
        let err = apply_vault_bridge_deposit_propose(&mut ledger, &replay_operation, 2)
            .expect_err("deposit_id replay");
        assert_eq!(err.0, "duplicate_vault_bridge_deposit_id");
    }

    fn vault_bridge_profiled_asset_fixture() -> (
        Genesis,
        LedgerState,
        MlDsa65KeyPair,
        String,
        String,
        String,
        String,
    ) {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let holder = "stage1-holder".to_string();
        let evidence = vault_bridge_evidence(1_000_000, "c1");
        let policy_hash = "42".repeat(48);
        let profile = postfiat_types::NavProofProfile::new(
            issuer.clone(),
            NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
            format!("vault_bridge:{}", evidence.source_domain()),
            100,
            0,
            100,
            0,
            0,
            1,
            0,
            policy_hash,
            String::new(),
            String::new(),
            0,
            0,
        )
        .expect("vault bridge profile");
        let profile_id = profile.profile_id.clone();
        let asset = AssetDefinition::new(&genesis.chain_id, &issuer, "PFU", 1, 6)
            .expect("vault bridge profiled asset");
        let asset_id = asset.asset_id.clone();
        let mut holder_line =
            TrustLine::new(holder.clone(), issuer.clone(), asset_id.clone(), 100_000_000, 0)
                .expect("holder trustline");
        holder_line.authorized = true;
        holder_line.validate().expect("holder trustline valid");
        let reserve_packet_hash = "aa".repeat(48);
        let mut nav_asset = NavTrackedAsset::new(
            asset_id.clone(),
            issuer.clone(),
            issuer.clone(),
            profile_id,
            "SOURCE_UNIT",
            issuer.clone(),
        )
        .expect("vault bridge nav asset");
        nav_asset.finalized_epoch = 1;
        nav_asset.nav_per_unit = 1_000_000;
        nav_asset.circulating_supply = 100_000_000;
        nav_asset.finalized_reserve_packet_hash = reserve_packet_hash.clone();
        nav_asset.finalized_at_height = 1;
        nav_asset.validate().expect("vault bridge nav asset valid");
        let mut ledger = LedgerState::new_with_assets(
            vec![
                Account::new(
                    issuer.clone(),
                    10_000,
                    Some(bytes_to_hex(&issuer_key.public_key)),
                ),
                Account::new(holder.clone(), 10_000, None),
            ],
            vec![asset],
            vec![holder_line],
        );
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_assets.push(nav_asset);
        (
            genesis,
            ledger,
            issuer_key,
            issuer,
            holder,
            asset_id,
            reserve_packet_hash,
        )
    }

    #[test]
    fn vault_bridge_stage1_issuer_payment_direct_mint_rejected() {
        let (genesis, mut ledger, issuer_key, issuer, holder, asset_id, _) =
            vault_bridge_profiled_asset_fixture();
        let direct_mint = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.clone(),
                to: holder,
                issuer,
                asset_id,
                amount: 1,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &direct_mint, 2);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "vault_bridge_out_of_lane_issuance");
    }

    #[test]
    fn bridge_verification_activation_gates_out_of_lane_issuer_payment() {
        let (genesis, ledger, issuer_key, issuer, holder, asset_id, _) =
            vault_bridge_profiled_asset_fixture();
        let direct_mint = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.clone(),
                to: holder.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                amount: 1,
            }),
        );
        let bridge_verification_at_10 =
            AssetExecutionCompatibility::strict().with_bridge_verification_activation_height(Some(10));
        let mut pre_activation_ledger = ledger.clone();
        let pre_activation_receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut pre_activation_ledger,
            &direct_mint,
            9,
            bridge_verification_at_10,
        );
        assert!(
            pre_activation_receipt.accepted,
            "direct bridge-asset issuer payment should replay before activation: {pre_activation_receipt:?}"
        );
        let mut activation_ledger = ledger;
        let activation_receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut activation_ledger,
            &direct_mint,
            10,
            bridge_verification_at_10,
        );
        assert!(!activation_receipt.accepted);
        assert_eq!(activation_receipt.code, "vault_bridge_out_of_lane_issuance");
    }

    #[test]
    fn vault_bridge_stage1_nav_mint_at_nav_rejected_for_bridge_asset() {
        let (genesis, mut ledger, issuer_key, issuer, holder, asset_id, reserve_packet_hash) =
            vault_bridge_profiled_asset_fixture();
        let nav_mint = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            NAV_MINT_AT_NAV_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavMintAtNav(NavMintAtNavOperation {
                issuer,
                to: holder,
                asset_id,
                amount: 1,
                epoch: 1,
                reserve_packet_hash,
                settlement_asset_id: String::new(),
                settlement_bucket_id: String::new(),
                settlement_allocation_id: String::new(),
                settlement_amount_atoms: 0,
            }),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &nav_mint, 2);
        assert!(!receipt.accepted);
        assert_eq!(receipt.code, "vault_bridge_out_of_lane_issuance");
    }

    #[test]
    fn bridge_verification_activation_gates_deposit_observation_requirement() {
        let (mut pre_activation_ledger, asset_id, policy_hash, evidence, evidence_root) =
            vault_bridge_stage2_fixture(1, 6);
        let propose = vault_bridge_stage1_propose_operation(
            asset_id.clone(),
            policy_hash.clone(),
            evidence.clone(),
        );
        apply_vault_bridge_deposit_propose(&mut pre_activation_ledger, &propose, 1)
            .expect("pre-activation propose");
        let attest_without_observation = VaultBridgeDepositAttestOperation {
            attestor: "stage2-observer".to_string(),
            asset_id: asset_id.clone(),
            evidence_root: evidence_root.clone(),
            pass: true,
            observation_root: evidence_root.clone(),
            observation: None,
        };
        let bridge_verification_at_10 =
            AssetExecutionCompatibility::strict().with_bridge_verification_activation_height(Some(10));
        apply_vault_bridge_deposit_attest_with_compatibility(
            &mut pre_activation_ledger,
            &attest_without_observation,
            9,
            bridge_verification_at_10,
        )
        .expect("pre-activation attestation without observation replays");

        let (mut activation_ledger, asset_id, policy_hash, evidence, evidence_root) =
            vault_bridge_stage2_fixture(1, 6);
        let propose = vault_bridge_stage1_propose_operation(asset_id, policy_hash, evidence);
        apply_vault_bridge_deposit_propose(&mut activation_ledger, &propose, 1)
            .expect("activation propose");
        let attest_without_observation = VaultBridgeDepositAttestOperation {
            attestor: "stage2-observer".to_string(),
            asset_id: propose.asset_id,
            evidence_root,
            pass: true,
            observation_root: propose.evidence_root,
            observation: None,
        };
        let err = apply_vault_bridge_deposit_attest_with_compatibility(
            &mut activation_ledger,
            &attest_without_observation,
            10,
            bridge_verification_at_10,
        )
        .expect_err("activation requires observed EVM receipt facts");
        assert_eq!(err.0, "vault_bridge_deposit_observation_missing");
    }

    #[test]
    fn bridge_verification_activation_rejects_zero_confirmation_asset_profile() {
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let evidence = vault_bridge_evidence(1_000_000, "d2");
        let policy_hash = "43".repeat(48);
        let legacy_profile =
            postfiat_types::NavProofProfile::new_with_bridge_observer_min_confirmations(
                issuer.clone(),
                NAV_PROFILE_VERIFIER_MULTI_FETCH,
                format!("vault_bridge:{}", evidence.source_domain()),
                100,
                0,
                100,
                0,
                0,
                1,
                0,
                0,
                policy_hash.clone(),
                "",
                "",
                0,
                0,
            )
            .expect("legacy vault bridge profile");
        let observed_profile =
            postfiat_types::NavProofProfile::new_with_bridge_observer_min_confirmations(
                issuer.clone(),
                NAV_PROFILE_VERIFIER_MULTI_FETCH,
                format!("vault_bridge:{}", evidence.source_domain()),
                100,
                0,
                100,
                0,
                0,
                1,
                0,
                6,
                policy_hash,
                "",
                "",
                0,
                0,
            )
            .expect("observed vault bridge profile");
        let asset = AssetDefinition::new(&genesis.chain_id, issuer.clone(), "PFUSDC", 94, 6)
            .expect("vault bridge asset");
        let asset_id = asset.asset_id.clone();
        let mut initial_ledger = LedgerState::new(vec![Account::new(
            issuer.clone(),
            10_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        )]);
        initial_ledger.asset_definitions.push(asset);
        initial_ledger
            .nav_proof_profiles
            .push(legacy_profile.clone());
        initial_ledger
            .nav_proof_profiles
            .push(observed_profile.clone());
        let register = |ledger: &LedgerState, profile_id: String| {
            signed_asset_transaction_with_minimum_fee(
                &genesis,
                ledger,
                &issuer_key,
                NAV_ASSET_REGISTER_TRANSACTION_KIND,
                1,
                AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
                    issuer: issuer.clone(),
                    asset_id: asset_id.clone(),
                    reserve_operator: issuer.clone(),
                    proof_profile: profile_id,
                    valuation_unit: "USDC".to_string(),
                    redemption_account: issuer.clone(),
                }),
            )
        };
        let bridge_verification_at_10 =
            AssetExecutionCompatibility::strict().with_bridge_verification_activation_height(Some(10));

        let mut pre_activation_ledger = initial_ledger.clone();
        let pre_activation_registration =
            register(&pre_activation_ledger, legacy_profile.profile_id.clone());
        let pre_activation = execute_asset_transaction_with_replay_compatibility(
            &genesis,
            &mut pre_activation_ledger,
            &pre_activation_registration,
            9,
            bridge_verification_at_10,
        );
        assert!(
            pre_activation.accepted,
            "legacy history before activation must remain replayable: {pre_activation:?}"
        );

        let mut activation_ledger = initial_ledger.clone();
        let activation_registration =
            register(&activation_ledger, legacy_profile.profile_id);
        let activation = execute_asset_transaction_with_replay_compatibility(
            &genesis,
            &mut activation_ledger,
            &activation_registration,
            10,
            bridge_verification_at_10,
        );
        assert!(!activation.accepted);
        assert_eq!(
            activation.code,
            "vault_bridge_observer_policy_not_configured"
        );

        let migrated_registration = register(&initial_ledger, observed_profile.profile_id);
        let migrated = execute_asset_transaction_with_replay_compatibility(
            &genesis,
            &mut initial_ledger,
            &migrated_registration,
            10,
            bridge_verification_at_10,
        );
        assert!(
            migrated.accepted,
            "explicit observer confirmation policy must remain registerable: {migrated:?}"
        );
    }

    fn h94_fabricated_evidence_for_stage2_flip() -> VaultBridgeDepositEvidence {
        let pftl_recipient = "pf323cfe884291b17844024b43ac44962c468b51b4".to_string();
        let pftl_recipient_hash =
            vault_bridge_pftl_recipient_hash(&pftl_recipient).expect("recipient hash");
        let mut evidence = VaultBridgeDepositEvidence {
            source_chain_id: 42_161,
            vault_address: "0x6a700337663d7c4143e26a3a172077415d90e7d7".to_string(),
            token_address: "0xaf88d065e77c8cc2239327c5edb3a432268e5831".to_string(),
            depositor: "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0".to_string(),
            pftl_recipient,
            pftl_recipient_hash,
            amount_atoms: 20_000_000,
            // The W6 evidence pack preserved the fake h94 block/tx/log, but not the
            // nonce needed to rederive the exact historical deposit id. Keep the
            // fabricated source-chain facts and use a validate-consistent deposit id.
            nonce: "94".repeat(32),
            route_binding: String::new(),
            deposit_id: "00".repeat(32),
            block_hash: "88a720e1028d2720694246bcc74ff1a1951b574bd5f5b39d685804734495ec06".to_string(),
            tx_hash: "906c0952e3abcfeb5ce47d62c6b48aff33a72e3a08a9f251fb4bfb0350377367".to_string(),
            log_index: 28_676,
        };
        evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
        evidence
    }

    #[test]
    fn h94_fabricated_block_admission_stage2_must_flip() {
        // Stage 2 flips the Stage 1 documentation vector: a structurally valid
        // but fabricated EVM source claim cannot collect an observer-confirmed
        // receipt quorum, so it must not mint.
        let genesis = Genesis::new("postfiat-local");
        let issuer_key = ml_dsa_65_keygen().expect("issuer keygen");
        let holder_key = ml_dsa_65_keygen().expect("holder keygen");
        let issuer = address_from_public_key(&issuer_key.public_key);
        let holder = address_from_public_key(&holder_key.public_key);
        let evidence = h94_fabricated_evidence_for_stage2_flip();
        evidence
            .validate()
            .expect("h94 evidence is structurally valid");
        let recipient = evidence.pftl_recipient.clone();
        let source_domain = evidence.source_domain();
        let policy_hash =
            "15afdae7a9e68a261daa6ea7593739b8d8d5c71b47239d2459ff20fc2dd515f7c4fc2f78748246ef0c79d53daa9134a0"
                .to_string();
        let evidence_root = vault_bridge_deposit_evidence_root(&evidence).expect("h94 evidence root");
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
            Account::new(recipient.clone(), 10_000, None),
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
                max_snapshot_age_blocks: 10_000,
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
        assert!(
            execute_asset_transaction(&genesis, &mut ledger, &profile_register, 1).accepted
        );
        let profile_id = ledger.nav_proof_profiles[0].profile_id.clone();

        let create_asset = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &issuer_key,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.clone(),
                code: "PFUSDC".to_string(),
                version: 94,
                precision: 6,
                display_name: "h94 pfUSDC vector".to_string(),
                max_supply: Some(100_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &create_asset, 2).accepted);
        let asset_id = ledger.asset_definitions[0].asset_id.clone();

        let register_asset = signed_asset_transaction_with_minimum_fee(
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
        assert!(execute_asset_transaction(&genesis, &mut ledger, &register_asset, 3).accepted);

        let attestor_register = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
                attestor: holder.clone(),
                domain: "h94.local".to_string(),
                bond: 0,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut ledger, &attestor_register, 4).accepted);

        let propose = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
                proposer: holder.clone(),
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
            }),
        );
        let propose_receipt = execute_asset_transaction(&genesis, &mut ledger, &propose, 5);
        assert!(
            propose_receipt.accepted,
            "current admission should accept the h94 well-formed lie before Stage 2: {propose_receipt:?}"
        );

        let attest = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::VaultBridgeDepositAttest(VaultBridgeDepositAttestOperation {
                attestor: holder.clone(),
                asset_id: asset_id.clone(),
                evidence_root: evidence_root.clone(),
                pass: true,
                observation_root: evidence_root.clone(),
                observation: None,
            }),
        );
        let attest_receipt = execute_asset_transaction(&genesis, &mut ledger, &attest, 6);
        assert!(!attest_receipt.accepted);
        assert_eq!(
            attest_receipt.code,
            "vault_bridge_deposit_observation_missing"
        );

        let finalize = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::VaultBridgeDepositFinalize(
                VaultBridgeDepositFinalizeOperation {
                    finalizer: holder.clone(),
                    asset_id: asset_id.clone(),
                    evidence_root: evidence_root.clone(),
                },
            ),
        );
        let finalize_receipt = execute_asset_transaction(&genesis, &mut ledger, &finalize, 7);
        assert!(!finalize_receipt.accepted);
        assert_eq!(
            finalize_receipt.code,
            "vault_bridge_deposit_attestation_quorum_not_met"
        );

        let claim = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &holder_key,
            VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::VaultBridgeDepositClaim(VaultBridgeDepositClaimOperation {
                claimer: holder,
                asset_id,
                evidence_root,
                policy_hash,
                recipient,
                amount_atoms: evidence.amount_atoms,
            }),
        );
        let claim_receipt = execute_asset_transaction(&genesis, &mut ledger, &claim, 8);
        assert!(!claim_receipt.accepted);
        assert_eq!(claim_receipt.code, "vault_bridge_deposit_not_finalized");
    }

    #[test]
    fn rotated_route_uses_pinned_profile_for_in_flight_deposit_challenge() {
        let genesis = Genesis::new("postfiat-local");
        let issuer = "rotation-issuer";
        let challenger = "rotation-challenger";
        let asset = AssetDefinition::new(&genesis.chain_id, issuer, "pfUSDC", 1, 6)
            .expect("bridge asset");
        let mut evidence = vault_bridge_evidence(5, "71");
        evidence.route_binding = "72".repeat(32);
        evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("v2 deposit id");
        let source_domain = evidence.source_domain();
        let old_policy_hash = "31".repeat(48);
        let current_policy_hash = "32".repeat(48);
        let old_profile = NavProofProfile::new(
            issuer,
            NAV_PROFILE_VERIFIER_MULTI_FETCH,
            format!("vault_bridge:{source_domain}"),
            100,
            10,
            100,
            100,
            1,
            1,
            0,
            old_policy_hash.clone(),
            "",
            "",
            0,
            0,
        )
        .expect("old profile");
        let current_profile = NavProofProfile::new(
            issuer,
            NAV_PROFILE_VERIFIER_MULTI_FETCH,
            format!("vault_bridge:{source_domain}"),
            100,
            1,
            100,
            100,
            100,
            1,
            0,
            current_policy_hash,
            "",
            "",
            0,
            0,
        )
        .expect("current profile");
        let nav_asset = NavTrackedAsset::new(
            asset.asset_id.clone(),
            issuer,
            issuer,
            current_profile.profile_id.clone(),
            "USDC",
            issuer,
        )
        .expect("NAV asset");
        let mut ledger = LedgerState::new_with_assets(
            vec![Account::new(challenger, ACCOUNT_RESERVE + 10, None)],
            vec![asset.clone()],
            Vec::new(),
        );
        ledger.nav_proof_profiles = vec![old_profile, current_profile.clone()];
        ledger.nav_assets.push(nav_asset);
        let evidence_root =
            vault_bridge_deposit_evidence_root(&evidence).expect("deposit root");
        ledger.vault_bridge_deposits.push(
            VaultBridgeDepositRecord::new(
                asset.asset_id.clone(),
                evidence_root.clone(),
                evidence,
                old_policy_hash,
                "",
                "",
                "",
                issuer,
                1,
                100,
            )
            .expect("pinned deposit"),
        );
        let operation = VaultBridgeDepositChallengeOperation {
            challenger: challenger.to_string(),
            asset_id: asset.asset_id,
            evidence_root,
            challenge_hash: "73".repeat(48),
            bond: 1,
        };
        apply_vault_bridge_deposit_challenge(&mut ledger, &operation, 2)
            .expect("old pinned challenge policy must remain usable after rotation");
        assert_eq!(
            VAULT_BRIDGE_DEPOSIT_STATUS_CHALLENGED,
            ledger.vault_bridge_deposits[0].status
        );
        assert_eq!(ACCOUNT_RESERVE + 9, ledger.account(challenger).unwrap().balance);

        let mut missing_historical = ledger.clone();
        missing_historical.vault_bridge_deposits[0].status =
            VAULT_BRIDGE_DEPOSIT_STATUS_PENDING.to_string();
        missing_historical.vault_bridge_deposits[0].challenger.clear();
        missing_historical.vault_bridge_deposits[0].challenge_hash.clear();
        missing_historical.vault_bridge_deposits[0].challenge_bond = 0;
        missing_historical.nav_proof_profiles = vec![current_profile];
        let before = missing_historical.clone();
        let error = apply_vault_bridge_deposit_challenge(&mut missing_historical, &operation, 2)
            .expect_err("unregistered current profile must fail closed");
        assert_eq!("missing_vault_bridge_pinned_profile", error.0);
        assert_eq!(before, missing_historical, "failed lookup mutated state");
    }
