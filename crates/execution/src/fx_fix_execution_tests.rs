    fn fx_fix_packet_fixture(
        operator: String,
        base_asset_id: String,
        quote_asset_id: String,
    ) -> postfiat_types::FxFixPacketV1 {
        let mut packet = postfiat_types::FxFixPacketV1 {
            version: postfiat_types::FX_FIX_PACKET_VERSION_V1,
            schema: postfiat_types::FX_FIX_PACKET_SCHEMA_V1.to_string(),
            operator,
            base_asset_id,
            quote_asset_id,
            epoch: 1,
            ratio_numerator: 21,
            ratio_denominator: 2_000_000,
            band_bps: 0,
            fee_bps: 0,
            valid_from_height: 10,
            expires_at_height: 100,
            minimum_base_atoms: 20_000_000,
            capacity_base_atoms: 20_000_000,
            capacity_quote_atoms: 210,
            max_fills: 1,
            source_label: "demo_fix".to_string(),
            source_observation_commitment: "11".repeat(48),
            governance_policy_hash: "22".repeat(48),
            previous_fix_hash: None,
            packet_hash: String::new(),
        };
        packet.packet_hash = packet.canonical_hash();
        packet
    }

    fn fx_fix_ledger_fixture() -> (
        Genesis,
        LedgerState,
        MlDsa65KeyPair,
        postfiat_types::FxFixPacketV1,
    ) {
        let genesis = Genesis::new("postfiat-local");
        let operator_key = ml_dsa_65_keygen().expect("operator keygen");
        let base_issuer_key = ml_dsa_65_keygen().expect("base issuer keygen");
        let quote_issuer_key = ml_dsa_65_keygen().expect("quote issuer keygen");
        let operator = address_from_public_key(&operator_key.public_key);
        let base_issuer = address_from_public_key(&base_issuer_key.public_key);
        let quote_issuer = address_from_public_key(&quote_issuer_key.public_key);
        let mut ledger = LedgerState::new(vec![
            Account::new(
                operator.clone(),
                1_000_000,
                Some(bytes_to_hex(&operator_key.public_key)),
            ),
            Account::new(
                base_issuer.clone(),
                1_000_000,
                Some(bytes_to_hex(&base_issuer_key.public_key)),
            ),
            Account::new(
                quote_issuer.clone(),
                1_000_000,
                Some(bytes_to_hex(&quote_issuer_key.public_key)),
            ),
        ]);
        let base = AssetDefinition::new(&genesis.chain_id, base_issuer, "PFUSDC".to_string(), 1, 6)
            .expect("base asset");
        let quote = AssetDefinition::new(&genesis.chain_id, quote_issuer, "PNOK".to_string(), 1, 0)
            .expect("quote asset");
        let packet = fx_fix_packet_fixture(
            operator,
            base.asset_id.clone(),
            quote.asset_id.clone(),
        );
        ledger.asset_definitions.extend([base, quote]);
        (genesis, ledger, operator_key, packet)
    }

    #[test]
    fn fx_fix_register_reserve_release_is_bounded_and_replay_safe() {
        let (genesis, mut ledger, operator_key, packet) = fx_fix_ledger_fixture();
        let register = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_REGISTER_TRANSACTION_KIND_V1,
            1,
            AssetTransactionOperation::FxFixRegisterV1(
                postfiat_types::FxFixRegisterOperationV1 {
                    operator: packet.operator.clone(),
                    packet: packet.clone(),
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &register, 10);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
        assert_eq!(ledger.fx_fix_states.len(), 1);

        let duplicate = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_REGISTER_TRANSACTION_KIND_V1,
            2,
            AssetTransactionOperation::FxFixRegisterV1(
                postfiat_types::FxFixRegisterOperationV1 {
                    operator: packet.operator.clone(),
                    packet: packet.clone(),
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &duplicate, 11);
        assert_eq!(receipt.code, "duplicate_fx_fix_packet");

        let reserve_operation = postfiat_types::FxFixReservationCreateOperationV1 {
            operator: packet.operator.clone(),
            fix_packet_hash: packet.packet_hash.clone(),
            action_binding_hash: "ab".repeat(64),
            base_atoms: 20_000_000,
            quote_atoms: 210,
            wallet_intent_hash: "bc".repeat(48),
            reservation_nonce: "cd".repeat(48),
            expires_at_height: 90,
        };
        let wrong_quote = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_RESERVATION_CREATE_TRANSACTION_KIND_V1,
            2,
            AssetTransactionOperation::FxFixReservationCreateV1(
                postfiat_types::FxFixReservationCreateOperationV1 {
                    quote_atoms: 209,
                    ..reserve_operation.clone()
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &wrong_quote, 11);
        assert_eq!(receipt.code, "fx_fix_reservation_quote_mismatch");
        assert!(ledger.fx_fix_reservations.is_empty());

        let reservation_id = reserve_operation.reservation_id().expect("reservation id");
        let reserve = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_RESERVATION_CREATE_TRANSACTION_KIND_V1,
            2,
            AssetTransactionOperation::FxFixReservationCreateV1(reserve_operation.clone()),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &reserve, 11);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
        assert_eq!(ledger.fx_fix_reservations[0].reservation_id, reservation_id);

        let duplicate_action = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_RESERVATION_CREATE_TRANSACTION_KIND_V1,
            3,
            AssetTransactionOperation::FxFixReservationCreateV1(
                postfiat_types::FxFixReservationCreateOperationV1 {
                    wallet_intent_hash: "ef".repeat(48),
                    reservation_nonce: "de".repeat(48),
                    ..reserve_operation
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &duplicate_action, 12);
        assert_eq!(receipt.code, "fx_fix_fill_capacity_reserved");

        let release = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_RESERVATION_RELEASE_TRANSACTION_KIND_V1,
            3,
            AssetTransactionOperation::FxFixReservationReleaseV1(
                postfiat_types::FxFixReservationReleaseOperationV1 {
                    operator: packet.operator,
                    reservation_id,
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &release, 12);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
        assert_eq!(
            ledger.fx_fix_reservations[0].state,
            postfiat_types::FX_FIX_RESERVATION_STATE_RELEASED
        );
    }

    #[test]
    fn fx_fix_reservation_rejects_expired_packet_without_effect() {
        let (genesis, mut ledger, operator_key, packet) = fx_fix_ledger_fixture();
        let register = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_REGISTER_TRANSACTION_KIND_V1,
            1,
            AssetTransactionOperation::FxFixRegisterV1(
                postfiat_types::FxFixRegisterOperationV1 {
                    operator: packet.operator.clone(),
                    packet: packet.clone(),
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &register, 10);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);

        let reserve = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_RESERVATION_CREATE_TRANSACTION_KIND_V1,
            2,
            AssetTransactionOperation::FxFixReservationCreateV1(
                postfiat_types::FxFixReservationCreateOperationV1 {
                    operator: packet.operator,
                    fix_packet_hash: packet.packet_hash,
                    action_binding_hash: "ab".repeat(64),
                    base_atoms: 20_000_000,
                    quote_atoms: 210,
                    wallet_intent_hash: "bc".repeat(48),
                    reservation_nonce: "cd".repeat(48),
                    expires_at_height: 100,
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &reserve, 101);
        assert_eq!(receipt.code, "fx_fix_not_active");
        assert!(ledger.fx_fix_reservations.is_empty());
        assert_eq!(ledger.fx_fix_states[0].fill_count, 0);
    }

    #[test]
    fn fx_fix_reservation_rejects_wallet_intent_replay_before_new_action() {
        let (genesis, mut ledger, operator_key, mut packet) = fx_fix_ledger_fixture();
        packet.capacity_base_atoms = 40_000_000;
        packet.capacity_quote_atoms = 420;
        packet.max_fills = 2;
        packet.packet_hash = packet.canonical_hash();
        let register = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_REGISTER_TRANSACTION_KIND_V1,
            1,
            AssetTransactionOperation::FxFixRegisterV1(
                postfiat_types::FxFixRegisterOperationV1 {
                    operator: packet.operator.clone(),
                    packet: packet.clone(),
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &register, 10);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);

        let wallet_intent_hash = "bc".repeat(48);
        let first = postfiat_types::FxFixReservationCreateOperationV1 {
            operator: packet.operator.clone(),
            fix_packet_hash: packet.packet_hash.clone(),
            action_binding_hash: "ab".repeat(64),
            base_atoms: 20_000_000,
            quote_atoms: 210,
            wallet_intent_hash: wallet_intent_hash.clone(),
            reservation_nonce: "cd".repeat(48),
            expires_at_height: 90,
        };
        let transaction = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_RESERVATION_CREATE_TRANSACTION_KIND_V1,
            2,
            AssetTransactionOperation::FxFixReservationCreateV1(first),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &transaction, 11);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);

        let replayed_intent = postfiat_types::FxFixReservationCreateOperationV1 {
            operator: packet.operator,
            fix_packet_hash: packet.packet_hash,
            action_binding_hash: "ef".repeat(64),
            base_atoms: 20_000_000,
            quote_atoms: 210,
            wallet_intent_hash,
            reservation_nonce: "de".repeat(48),
            expires_at_height: 90,
        };
        let transaction = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_RESERVATION_CREATE_TRANSACTION_KIND_V1,
            3,
            AssetTransactionOperation::FxFixReservationCreateV1(replayed_intent),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &transaction, 12);
        assert_eq!(receipt.code, "duplicate_fx_fix_wallet_intent");
        assert_eq!(ledger.fx_fix_reservations.len(), 1);
    }

    #[test]
    fn fx_fix_reservation_enforces_atom_capacity_independently_of_fill_slots() {
        let (genesis, mut ledger, operator_key, mut packet) = fx_fix_ledger_fixture();
        packet.minimum_base_atoms = 10_000_000;
        packet.capacity_base_atoms = 30_000_000;
        packet.capacity_quote_atoms = 315;
        packet.max_fills = 4;
        packet.packet_hash = packet.canonical_hash();
        let register = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_REGISTER_TRANSACTION_KIND_V1,
            1,
            AssetTransactionOperation::FxFixRegisterV1(
                postfiat_types::FxFixRegisterOperationV1 {
                    operator: packet.operator.clone(),
                    packet: packet.clone(),
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &register, 10);
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);

        for (nonce, action_byte, intent_byte, reservation_byte) in
            [(2, "ab", "bc", "cd"), (3, "de", "ef", "12")]
        {
            let operation = postfiat_types::FxFixReservationCreateOperationV1 {
                operator: packet.operator.clone(),
                fix_packet_hash: packet.packet_hash.clone(),
                action_binding_hash: action_byte.repeat(64),
                base_atoms: 20_000_000,
                quote_atoms: 210,
                wallet_intent_hash: intent_byte.repeat(48),
                reservation_nonce: reservation_byte.repeat(48),
                expires_at_height: 90,
            };
            let transaction = signed_asset_transaction_with_minimum_fee(
                &genesis,
                &ledger,
                &operator_key,
                postfiat_types::FX_FIX_RESERVATION_CREATE_TRANSACTION_KIND_V1,
                nonce,
                AssetTransactionOperation::FxFixReservationCreateV1(operation),
            );
            let receipt = execute_asset_transaction(&genesis, &mut ledger, &transaction, 11);
            if nonce == 2 {
                assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
            } else {
                assert_eq!(receipt.code, "fx_fix_capacity_exhausted");
            }
        }
        assert_eq!(ledger.fx_fix_reservations.len(), 1);
        assert_eq!(ledger.fx_fix_states[0].fill_count, 0);
    }

    #[test]
    fn fx_fix_register_rejects_public_capacity_that_disagrees_with_ratio() {
        let (genesis, mut ledger, operator_key, mut packet) = fx_fix_ledger_fixture();
        packet.capacity_quote_atoms = 211;
        packet.packet_hash = packet.canonical_hash();
        let register = signed_asset_transaction_with_minimum_fee(
            &genesis,
            &ledger,
            &operator_key,
            postfiat_types::FX_FIX_REGISTER_TRANSACTION_KIND_V1,
            1,
            AssetTransactionOperation::FxFixRegisterV1(
                postfiat_types::FxFixRegisterOperationV1 {
                    operator: packet.operator.clone(),
                    packet,
                },
            ),
        );
        let receipt = execute_asset_transaction(&genesis, &mut ledger, &register, 10);
        assert_eq!(receipt.code, "fx_fix_capacity_ratio_mismatch");
        assert!(ledger.fx_fix_states.is_empty());
    }

    #[test]
    fn fx_fix_reservation_id_matches_cross_language_demo_vector() {
        let reservation_id = postfiat_types::fx_fix_reservation_id(
            &"22".repeat(48),
            &format!("pf{}", "11".repeat(20)),
            &"33".repeat(64),
            20_000_000,
            210,
            &"44".repeat(48),
            &"55".repeat(48),
        )
        .expect("valid reservation vector");
        assert_eq!(
            reservation_id,
            "b1ca42c657d42037af4c701b9b22e88e2c904c7181f5650af3237c04a59aa9cc5cd4c62e302b337f9aa6370330feac47"
        );
    }
