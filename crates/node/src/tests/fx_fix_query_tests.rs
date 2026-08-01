    fn fx_fix_query_fixture() -> (
        PathBuf,
        postfiat_types::FxFixPacketV1,
        postfiat_types::FxFixReservationV1,
    ) {
        let data_dir = unique_test_dir("postfiat-fx-fix-query-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-fx-fix-query".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init fx fix query fixture");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let operator = read_transfer_key_file(&data_dir, None)
            .expect("operator key")
            .address;
        let mut ledger = store.read_ledger().expect("ledger");
        let base = AssetDefinition::new(
            &genesis.chain_id,
            operator.clone(),
            "PFUSDC".to_string(),
            1,
            6,
        )
        .expect("pfUSDC definition");
        let quote = AssetDefinition::new(
            &genesis.chain_id,
            operator.clone(),
            "PNOK".to_string(),
            1,
            0,
        )
        .expect("pNOK definition");
        let mut packet = postfiat_types::FxFixPacketV1 {
            version: postfiat_types::FX_FIX_PACKET_VERSION_V1,
            schema: postfiat_types::FX_FIX_PACKET_SCHEMA_V1.to_string(),
            operator: operator.clone(),
            base_asset_id: base.asset_id.clone(),
            quote_asset_id: quote.asset_id.clone(),
            epoch: 1,
            ratio_numerator: 21,
            ratio_denominator: 2_000_000,
            band_bps: 0,
            fee_bps: 0,
            valid_from_height: 0,
            expires_at_height: 100,
            minimum_base_atoms: 20_000_000,
            capacity_base_atoms: 40_000_000,
            capacity_quote_atoms: 420,
            max_fills: 2,
            source_label: "demo_fix".to_string(),
            source_observation_commitment: "11".repeat(48),
            governance_policy_hash: "22".repeat(48),
            previous_fix_hash: None,
            packet_hash: String::new(),
        };
        packet.packet_hash = packet.canonical_hash();
        packet.validate().expect("valid fix packet");
        let action_binding_hash = "33".repeat(64);
        let base_atoms = 20_000_000;
        let quote_atoms = 210;
        let wallet_intent_hash = "34".repeat(48);
        let reservation_nonce = "44".repeat(48);
        let reservation = postfiat_types::FxFixReservationV1 {
            reservation_id: postfiat_types::fx_fix_reservation_id(
                &packet.packet_hash,
                &operator,
                &action_binding_hash,
                base_atoms,
                quote_atoms,
                &wallet_intent_hash,
                &reservation_nonce,
            )
            .expect("reservation id"),
            fix_packet_hash: packet.packet_hash.clone(),
            operator,
            action_binding_hash,
            base_atoms,
            quote_atoms,
            wallet_intent_hash,
            reservation_nonce,
            created_at_height: 0,
            expires_at_height: 50,
            state: postfiat_types::FX_FIX_RESERVATION_STATE_ACTIVE.to_string(),
            terminal_at_height: 0,
        };
        reservation.validate().expect("valid reservation");
        ledger.asset_definitions.extend([base, quote]);
        ledger.fx_fix_states.push(postfiat_types::FxFixStateV1 {
            packet: packet.clone(),
            paused: false,
            fill_count: 0,
            registered_at_height: 0,
            last_updated_height: 0,
        });
        ledger.fx_fix_reservations.push(reservation.clone());
        store.write_ledger(&ledger).expect("write fx fix ledger");
        (data_dir, packet, reservation)
    }

    #[test]
    fn fx_fix_queries_are_bounded_deterministic_and_wallet_ready() {
        let (data_dir, packet, reservation) = fx_fix_query_fixture();
        let list = fx_fix_list(FxFixListOptions {
            data_dir: data_dir.clone(),
            base_asset_id: Some(packet.base_asset_id.clone()),
            quote_asset_id: Some(packet.quote_asset_id.clone()),
            active_only: true,
            limit: Some(1),
        })
        .expect("active fix list");
        assert_eq!(list.schema, "postfiat-fx-fix-list-v1");
        assert_eq!(list.fix_count, 1);
        assert!(!list.truncated);
        assert_eq!(list.fixes[0].status, "active");
        assert_eq!(list.fixes[0].remaining_fill_slots, 1);
        assert_eq!(list.fixes[0].active_reservation_count, 1);
        assert_eq!(list.fixes[0].committed_base_atoms, 20_000_000);
        assert_eq!(list.fixes[0].committed_quote_atoms, 210);
        assert_eq!(list.fixes[0].remaining_base_atoms, 20_000_000);
        assert_eq!(list.fixes[0].remaining_quote_atoms, 210);
        assert_eq!(list.fixes[0].state.packet.source_label, "demo_fix");
        assert_eq!(list.fixes[0].base_asset.code, "PFUSDC");
        assert_eq!(list.fixes[0].base_asset.precision, 6);
        assert_eq!(list.fixes[0].quote_asset.code, "PNOK");
        assert_eq!(list.fixes[0].quote_asset.precision, 0);
        assert_eq!(list.fixes[0].pricing_claim.mode, "negotiated");
        assert_eq!(
            list.fixes[0].pricing_claim.reserve_packet_hash,
            packet.packet_hash
        );

        let info = fx_fix_info(FxFixInfoOptions {
            data_dir: data_dir.clone(),
            fix_packet_hash: packet.packet_hash.clone(),
        })
        .expect("fix info");
        assert!(info.found);
        assert_eq!(info.fix.expect("fix row").status, "active");

        let reservation_info = fx_fix_reservation_info(FxFixReservationInfoOptions {
            data_dir: data_dir.clone(),
            reservation_id: reservation.reservation_id.clone(),
        })
        .expect("reservation info");
        assert!(reservation_info.found);
        assert!(reservation_info.active);
        assert_eq!(reservation_info.fix_status.as_deref(), Some("active"));

        let quote = fx_fix_quote(FxFixQuoteOptions {
            data_dir: data_dir.clone(),
            fix_packet_hash: packet.packet_hash.clone(),
            base_atoms: 20_000_000,
        })
        .expect("exact demo quote");
        assert_eq!(quote.source_label, "demo_fix");
        assert_eq!(quote.base_atoms, 20_000_000);
        assert_eq!(quote.quote_atoms, 210);
        assert!(quote.exact_division);
        assert_eq!(quote.fee_atoms, 0);
        assert_eq!(quote.price_impact_bps, 0);
        let encoded = serde_json::to_string(&quote).expect("quote json");
        assert!(encoded.contains("\"base_asset_tag_lo\":\""));
        assert!(encoded.contains("\"quote_asset_tag_hi\":\""));

        let below_minimum = fx_fix_quote(FxFixQuoteOptions {
            data_dir: data_dir.clone(),
            fix_packet_hash: packet.packet_hash.clone(),
            base_atoms: 19_999_999,
        })
        .expect_err("below-minimum quote must fail");
        assert_eq!(below_minimum.kind(), io::ErrorKind::InvalidInput);

        let above_uncommitted_capacity = fx_fix_quote(FxFixQuoteOptions {
            data_dir: data_dir.clone(),
            fix_packet_hash: packet.packet_hash.clone(),
            base_atoms: 40_000_000,
        })
        .expect_err("quote must account for active reservations");
        assert_eq!(above_uncommitted_capacity.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn fx_fix_query_rejects_unbounded_or_malformed_input() {
        let (data_dir, packet, _) = fx_fix_query_fixture();
        let over_limit = fx_fix_list(FxFixListOptions {
            data_dir: data_dir.clone(),
            base_asset_id: None,
            quote_asset_id: None,
            active_only: false,
            limit: Some(MAX_READ_QUERY_LIMIT + 1),
        })
        .expect_err("oversized query limit must fail");
        assert_eq!(over_limit.kind(), io::ErrorKind::InvalidInput);

        let malformed = fx_fix_info(FxFixInfoOptions {
            data_dir,
            fix_packet_hash: packet.packet_hash.to_uppercase(),
        })
        .expect_err("uppercase packet hash must fail");
        assert_eq!(malformed.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn asset_orchard_action_status_proves_exact_finalized_elements() {
        let data_dir = unique_test_dir("postfiat-asset-orchard-action-status-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-asset-orchard-action-status".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init action status fixture");
        let store = NodeStore::new(&data_dir);
        let nullifiers = ["11".repeat(32), "22".repeat(32)];
        let output_commitments = ["33".repeat(32), "44".repeat(32)];
        let mut shielded = store.read_shielded().expect("shielded state");
        let mut pool = OrchardPoolState::empty(ASSET_ORCHARD_POOL_ID_V1);
        pool.nullifiers.extend(nullifiers.clone());
        pool.output_commitments.extend(output_commitments.clone());
        shielded.orchard = Some(pool);
        store.write_shielded(&shielded).expect("write shielded state");

        let report = asset_orchard_action_status(AssetOrchardActionStatusOptions {
            data_dir: data_dir.clone(),
            nullifiers: nullifiers.clone(),
            output_commitments: output_commitments.clone(),
        })
        .expect("action status");
        assert!(report.finalized_exactly_once);
        assert!(report
            .nullifiers
            .iter()
            .chain(report.output_commitments.iter())
            .all(|element| element.occurrence_count == 1));

        let missing = asset_orchard_action_status(AssetOrchardActionStatusOptions {
            data_dir: data_dir.clone(),
            nullifiers: [nullifiers[0].clone(), "55".repeat(32)],
            output_commitments,
        })
        .expect("missing element is a valid non-finalized status");
        assert!(!missing.finalized_exactly_once);
        assert_eq!(missing.nullifiers[1].occurrence_count, 0);

        let duplicate = asset_orchard_action_status(AssetOrchardActionStatusOptions {
            data_dir,
            nullifiers: [nullifiers[0].clone(), nullifiers[0].clone()],
            output_commitments: ["33".repeat(32), "44".repeat(32)],
        })
        .expect_err("duplicate query elements must fail");
        assert_eq!(duplicate.kind(), io::ErrorKind::InvalidInput);
    }
