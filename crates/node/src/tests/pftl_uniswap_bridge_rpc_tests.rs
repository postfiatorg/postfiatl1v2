    use postfiat_bridge::{
        pftl_uniswap_apply_primary_subscription, pftl_uniswap_bridge_ledger_from_config,
        pftl_uniswap_export_debit, pftl_uniswap_launch_config_digest,
        pftl_uniswap_mark_destination_consumed, pftl_uniswap_packet_id,
        pftl_uniswap_return_burn_id, pftl_uniswap_route_config_digest,
        pftl_uniswap_verify_transition_receipt_replay, PftlUniswapBridgeLedger,
        PftlUniswapExportDebitRequest, PftlUniswapForkRehearsalEvidence,
        PftlUniswapLaunchConfig, PftlUniswapMintAndSwapPacket,
        PftlUniswapOfficialUniswapV4Deployments, PftlUniswapPoolSeedConfig,
        PftlUniswapPrimarySubscriptionRequest, PftlUniswapRefundRequest,
        PftlUniswapReturnBurnRequest, PftlUniswapRouteConfig, PrimarySubscriptionQuoteInput,
    };
    use postfiat_types::{
        pftl_uniswap_non_consumption_proof_hash, EthereumRouteVerificationPolicyV1,
        FastSwapCommitteeRootV1, LedgerState,
        PftlUniswapConsensusExportPacket, PftlUniswapConsensusRouteState,
        PftlUniswapConsensusReceipt,
        PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED,
        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED,
        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED,
    };

    fn pftl_uniswap_test_hex(byte: &str, bytes: usize) -> String {
        byte.repeat(bytes)
    }

    fn pftl_uniswap_test_address(byte: &str) -> String {
        format!("0x{}", pftl_uniswap_test_hex(byte, 20))
    }

    fn pftl_uniswap_test_config(route_id: &str) -> PftlUniswapRouteConfig {
        PftlUniswapRouteConfig {
            schema: "postfiat-pftl-uniswap-route-config-v1".to_string(),
            route_id: route_id.to_string(),
            route_family: "primary_pftl_mint".to_string(),
            native_nav_asset_id: pftl_uniswap_test_hex("11", 48),
            settlement_asset_id: pftl_uniswap_test_hex("22", 48),
            wrapped_navcoin_token: pftl_uniswap_test_address("33"),
            handoff_controller: pftl_uniswap_test_address("44"),
            settlement_adapter: pftl_uniswap_test_address("45"),
            verifier_mode: "controlled_threshold_v1".to_string(),
            route_trust_class: "CONTROLLED".to_string(),
            uniswap_pool_id_or_path: format!("0x{}", pftl_uniswap_test_hex("55", 32)),
            router: pftl_uniswap_test_address("66"),
            failure_behavior: "refund_unconsumed_pftl_packet".to_string(),
            route_supply_cap_atoms: 1_000_000_000,
            packet_notional_cap_atoms: 100_000_000,
            seed_nav_epoch: 7,
            seed_usdc_atoms: 1_000_000,
            seed_wrapped_navcoin_atoms: 1_000_000,
            lp_recipient: pftl_uniswap_test_address("77"),
            lp_custody_policy: "controlled_launch_multisig".to_string(),
        }
    }

    fn pftl_uniswap_test_launch_config(config: &PftlUniswapRouteConfig) -> PftlUniswapLaunchConfig {
        PftlUniswapLaunchConfig {
            schema: "postfiat-pftl-uniswap-launch-config-v1".to_string(),
            route_id: config.route_id.clone(),
            route_config_digest: pftl_uniswap_route_config_digest(config)
                .expect("route config digest"),
            route_trust_class: config.route_trust_class.clone(),
            native_nav_asset_id: config.native_nav_asset_id.clone(),
            settlement_asset_id: config.settlement_asset_id.clone(),
            wrapped_navcoin_token: config.wrapped_navcoin_token.clone(),
            usdc_token: pftl_uniswap_test_address("aa"),
            handoff_controller: config.handoff_controller.clone(),
            receipt_verifier: pftl_uniswap_test_address("ab"),
            settlement_adapter: config.settlement_adapter.clone(),
            official_uniswap: PftlUniswapOfficialUniswapV4Deployments {
                chain_id: 42_161,
                deployments_source_url:
                    "https://developers.uniswap.org/docs/protocols/v4/deployments".to_string(),
                deployments_table_hash: pftl_uniswap_test_hex("ac", 32),
                checked_at_utc: "2026-07-01T00:00:00Z".to_string(),
                pool_manager: pftl_uniswap_test_address("ad"),
                position_manager: pftl_uniswap_test_address("ae"),
                universal_router: pftl_uniswap_test_address("af"),
                permit2: pftl_uniswap_test_address("b0"),
                state_view: pftl_uniswap_test_address("b1"),
            },
            uniswap_pool_key_hash: pftl_uniswap_test_hex("b2", 32),
            uniswap_pool_id: config.uniswap_pool_id_or_path.clone(),
            seed: PftlUniswapPoolSeedConfig {
                pricing_nav_epoch: config.seed_nav_epoch,
                pricing_reserve_packet_hash: pftl_uniswap_test_hex("b3", 48),
                seed_usdc_atoms: config.seed_usdc_atoms,
                seed_wrapped_navcoin_atoms: config.seed_wrapped_navcoin_atoms,
                nav_price_settlement_atoms_per_nav_atom: 1,
                tick_lower: -120,
                tick_upper: 120,
                fee_pips: 3_000,
                lp_recipient: config.lp_recipient.clone(),
                position_recipient: pftl_uniswap_test_address("b4"),
                lp_custody_policy: config.lp_custody_policy.clone(),
            },
            fork_rehearsal_required: true,
        }
    }

    fn pftl_uniswap_test_fork_rehearsal(
        launch_config: &PftlUniswapLaunchConfig,
    ) -> PftlUniswapForkRehearsalEvidence {
        PftlUniswapForkRehearsalEvidence {
            schema: "postfiat-pftl-uniswap-fork-rehearsal-evidence-v1".to_string(),
            rehearsal_id: "gate-3-fork-rehearsal-test".to_string(),
            launch_config_digest: pftl_uniswap_launch_config_digest(launch_config)
                .expect("launch config digest"),
            route_config_digest: launch_config.route_config_digest.clone(),
            fork_chain_id: launch_config.official_uniswap.chain_id,
            fork_block_number: 22_000_000,
            official_uniswap: launch_config.official_uniswap.clone(),
            uniswap_pool_key_hash: launch_config.uniswap_pool_key_hash.clone(),
            uniswap_pool_id: launch_config.uniswap_pool_id.clone(),
            seed_export_packet_hash: pftl_uniswap_test_hex("b5", 48),
            seed_receipt_root: pftl_uniswap_test_hex("b6", 48),
            seed_mint_tx_hash: pftl_uniswap_test_hex("b7", 32),
            seed_lp_tx_hash: pftl_uniswap_test_hex("b8", 32),
            external_buy_tx_hash: pftl_uniswap_test_hex("b9", 32),
            external_sell_tx_hash: pftl_uniswap_test_hex("ba", 32),
            mint_only_packet_tx_hash: pftl_uniswap_test_hex("bb", 32),
            mint_and_swap_packet_tx_hash: pftl_uniswap_test_hex("bc", 32),
            state_view_liquidity_after_seed: 1_000_000,
            state_view_liquidity_after_buy: 1_000_100,
            state_view_liquidity_after_sell: 999_900,
            user_buy_usdc_spent_atoms: 10_000,
            user_buy_wrapped_received_atoms: 9_900,
            user_sell_wrapped_spent_atoms: 5_000,
            user_sell_usdc_received_atoms: 4_900,
            canonical_supply_before_external_trades_atoms: config_seed_supply(launch_config),
            canonical_supply_after_external_trades_atoms: config_seed_supply(launch_config),
            packet_consumed_without_manual_mint: true,
            min_output_failure_reverted_without_consume: true,
        }
    }

    fn config_seed_supply(launch_config: &PftlUniswapLaunchConfig) -> u64 {
        launch_config.seed.seed_wrapped_navcoin_atoms
    }

    fn pftl_uniswap_test_packet(
        config: &PftlUniswapRouteConfig,
        launch_config: &PftlUniswapLaunchConfig,
    ) -> PftlUniswapMintAndSwapPacket {
        PftlUniswapMintAndSwapPacket {
            schema: "postfiat-pftl-uniswap-mint-and-swap-packet-v1".to_string(),
            route_id: config.route_id.clone(),
            config_digest: launch_config.route_config_digest.clone(),
            source_packet_hash: pftl_uniswap_test_hex("c0", 48),
            source_receipt_hash: pftl_uniswap_test_hex("c1", 48),
            source_receipt_root: pftl_uniswap_test_hex("c2", 48),
            source_wallet: "pfsourcewallet".to_string(),
            settlement_asset_id: launch_config.settlement_asset_id.clone(),
            native_nav_asset_id: launch_config.native_nav_asset_id.clone(),
            wrapped_navcoin_token: launch_config.wrapped_navcoin_token.clone(),
            ethereum_recipient: pftl_uniswap_test_address("c3"),
            token_out: launch_config.usdc_token.clone(),
            settlement_amount_atoms: 1_000,
            mint_amount_atoms: 1_000,
            pricing_nav_epoch: launch_config.seed.pricing_nav_epoch,
            pricing_reserve_packet_hash: launch_config.seed.pricing_reserve_packet_hash.clone(),
            uniswap_pool_id_or_path: launch_config.uniswap_pool_id.clone(),
            swap_path_hash: pftl_uniswap_test_hex("c4", 32),
            router: config.router.clone(),
            minimum_output_atoms: 950,
            deadline_seconds: 1_924_992_000,
            nonce: pftl_uniswap_test_hex("c5", 32),
        }
    }

    fn pftl_uniswap_test_ledger(route_id: &str) -> (PftlUniswapBridgeLedger, String) {
        let config = pftl_uniswap_test_config(route_id);
        let mut ledger =
            pftl_uniswap_bridge_ledger_from_config(&config, 42_161, 7, 64)
                .expect("ledger from config");
        pftl_uniswap_apply_primary_subscription(
            &mut ledger,
            PftlUniswapPrimarySubscriptionRequest {
                route_id: route_id.to_string(),
                source_wallet: "pfsourcewallet".to_string(),
                settlement_asset_id: config.settlement_asset_id.clone(),
                subscription_nonce: pftl_uniswap_test_hex("88", 32),
                quote: PrimarySubscriptionQuoteInput {
                    settlement_value_atoms: 200,
                    nav_price_settlement_atoms_per_nav_atom: 2,
                    pricing_nav_epoch: 7,
                    pricing_reserve_packet_hash: pftl_uniswap_test_hex("99", 48),
                },
            },
        )
        .expect("primary subscription");
        let packet_hash = pftl_uniswap_test_hex("aa", 48);
        pftl_uniswap_export_debit(
            &mut ledger,
            PftlUniswapExportDebitRequest {
                route_id: route_id.to_string(),
                packet_hash: packet_hash.clone(),
                nonce: pftl_uniswap_test_hex("bb", 32),
                source_wallet: "pfsourcewallet".to_string(),
                ethereum_recipient: pftl_uniswap_test_address("cc"),
                amount_atoms: 40,
                source_height: 10,
                destination_deadline_seconds: 1_800,
                refund_not_before_height: 20,
            },
        )
        .expect("export debit");
        (ledger, packet_hash)
    }

    fn write_pftl_uniswap_test_ledgers(data_dir: &Path, ledgers: &[PftlUniswapBridgeLedger]) {
        let json = serde_json::to_string_pretty(ledgers).expect("ledger json");
        std::fs::write(
            data_dir.join(PFTL_UNISWAP_BRIDGE_LEDGER_FILE),
            format!("{json}\n"),
        )
        .expect("write ledger sidecar");
    }

    fn pftl_uniswap_consensus_route_from_legacy_fixture(
        ledger: &PftlUniswapBridgeLedger,
    ) -> PftlUniswapConsensusRouteState {
        let export_packets = ledger
            .export_packets
            .iter()
            .map(|(packet_hash, packet)| {
                let status = match packet.status {
                    postfiat_bridge::PftlUniswapExportPacketStatus::SourceDebited => {
                        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED
                    }
                    postfiat_bridge::PftlUniswapExportPacketStatus::DestinationConsumed => {
                        PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED
                    }
                    postfiat_bridge::PftlUniswapExportPacketStatus::SourceRefunded => {
                        PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED
                    }
                };
                (
                    packet_hash.clone(),
                    PftlUniswapConsensusExportPacket {
                        packet_hash: packet.packet_hash.clone(),
                        nonce: packet.nonce.clone(),
                        source_wallet: packet.source_wallet.clone(),
                        ethereum_recipient: packet.ethereum_recipient.clone(),
                        amount_atoms: packet.amount_atoms,
                        source_height: packet.source_height,
                        destination_deadline_seconds: packet.destination_deadline_seconds,
                        refund_not_before_height: packet.refund_not_before_height,
                        status: status.to_string(),
                        ethereum_packet_digest: None,
                        ethereum_packet_schema_version: None,
                    },
                )
            })
            .collect();
        PftlUniswapConsensusRouteState {
            route_id: ledger.route_id.clone(),
            route_family: ledger.route_family.clone(),
            route_config_digest: ledger.route_config_digest.clone(),
            route_trust_class: ledger.route_trust_class.clone(),
            native_nav_asset_id: ledger.native_nav_asset_id.clone(),
            settlement_asset_id: ledger.settlement_asset_id.clone(),
            handoff_controller: ledger.handoff_controller.clone(),
            settlement_adapter: ledger.settlement_adapter.clone(),
            wrapped_navcoin_token: ledger.wrapped_navcoin_token.clone(),
            ethereum_chain_id: ledger.ethereum_chain_id,
            route_supply_cap_atoms: ledger.route_supply_cap_atoms,
            packet_notional_cap_atoms: ledger.packet_notional_cap_atoms,
            latest_finalized_nav_epoch: ledger.latest_finalized_nav_epoch,
            return_finality_blocks: ledger.return_finality_blocks,
            ethereum_verification_policy: None,
            authorized_valid_supply_atoms: ledger.authorized_valid_supply_atoms,
            pftl_spendable_supply_atoms: ledger.pftl_spendable_supply_atoms,
            native_spendable_balances_atoms: ledger.native_spendable_balances_atoms.clone(),
            ethereum_spendable_supply_atoms: ledger.ethereum_spendable_supply_atoms,
            other_registered_venue_supply_atoms: ledger.other_registered_venue_supply_atoms,
            outstanding_bridge_claims_atoms: ledger.outstanding_bridge_claims_atoms,
            pending_return_import_claims_atoms: ledger.pending_return_import_claims_atoms,
            settlement_reserve_atoms: ledger.settlement_reserve_atoms,
            primary_subscription_nonces: ledger.primary_subscription_nonces.clone(),
            export_packets,
            export_nonces: ledger.export_nonces.clone(),
            return_imports: std::collections::BTreeMap::new(),
            paused: ledger.paused,
        }
    }

    fn write_pftl_uniswap_test_json<T: serde::Serialize>(
        data_dir: &Path,
        name: &str,
        value: &T,
    ) -> PathBuf {
        let path = data_dir.join(name);
        let json = serde_json::to_string_pretty(value).expect("test json");
        std::fs::write(&path, format!("{json}\n")).expect("write test json");
        path
    }

    fn assert_navcoin_bridge_packet_preflight_error(
        data_dir: &Path,
        route_id: &str,
        packet_name: &str,
        packet: &PftlUniswapMintAndSwapPacket,
        expected_error: &str,
    ) {
        let packet_file = write_pftl_uniswap_test_json(data_dir, packet_name, packet);
        let error = navcoin_bridge_packet_preflight(NavcoinBridgePacketPreflightOptions {
            data_dir: data_dir.to_path_buf(),
            route_id: route_id.to_string(),
            packet_file,
        })
        .expect_err("packet preflight mismatch must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            error.to_string().contains(expected_error),
            "expected {expected_error}, got {error}"
        );
    }

    #[test]
    fn navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-status-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let (ledger, packet_hash) = pftl_uniswap_test_ledger("pftl-uniswap-a666");
        write_pftl_uniswap_test_ledgers(&data_dir, &[ledger]);

        let routes = navcoin_bridge_routes(NavcoinBridgeRoutesOptions {
            data_dir: data_dir.clone(),
        })
        .expect("routes status");
        assert_eq!(routes.route_count, 1);
        assert_eq!(routes.routes[0].route_id, "pftl-uniswap-a666");
        assert_eq!(routes.routes[0].route_family, "primary_pftl_mint");
        assert_eq!(routes.routes[0].outstanding_bridge_claims_atoms, 40);

        let packet = navcoin_bridge_packet(NavcoinBridgePacketOptions {
            data_dir: data_dir.clone(),
            route_id: "pftl-uniswap-a666".to_string(),
            packet_hash: packet_hash.clone(),
        })
        .expect("packet status");
        assert_eq!(packet.packet_hash, packet_hash);
        assert_eq!(packet.packet.amount_atoms, 40);

        let claims = navcoin_bridge_claims(NavcoinBridgeClaimsOptions {
            data_dir: data_dir.clone(),
            route_id: "pftl-uniswap-a666".to_string(),
            limit: Some(1),
            include_terminal: false,
        })
        .expect("claims status");
        assert_eq!(claims.export_claim_count, 1);
        assert_eq!(claims.exports.len(), 1);
        assert_eq!(claims.return_claim_count, 0);

        let supply = navcoin_bridge_supply_status(NavcoinBridgeSupplyStatusOptions {
            data_dir,
            route_id: "pftl-uniswap-a666".to_string(),
        })
        .expect("supply status");
        assert!(supply.invariant_holds);
        assert_eq!(supply.authorized_valid_supply_atoms, 100);
        assert_eq!(supply.outstanding_bridge_claims_atoms, 40);
    }

    #[test]
    fn navcoin_bridge_status_reads_consensus_pftl_uniswap_routes() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-consensus-status-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let (legacy_ledger, packet_hash) = pftl_uniswap_test_ledger("pftl-uniswap-a777");
        let mut ledger = LedgerState::new(Vec::new());
        ledger
            .pftl_uniswap_routes
            .push(pftl_uniswap_consensus_route_from_legacy_fixture(
                &legacy_ledger,
            ));
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write consensus ledger");

        let routes = navcoin_bridge_routes(NavcoinBridgeRoutesOptions {
            data_dir: data_dir.clone(),
        })
        .expect("consensus routes status");
        assert_eq!(routes.route_count, 1);
        assert_eq!(routes.routes[0].route_id, "pftl-uniswap-a777");
        assert_eq!(
            routes.routes[0].route_config_digest,
            legacy_ledger.route_config_digest
        );
        assert_eq!(routes.routes[0].outstanding_bridge_claims_atoms, 40);
        assert_eq!(routes.routes[0].outstanding_export_packet_count, 1);

        let packet = navcoin_bridge_packet(NavcoinBridgePacketOptions {
            data_dir: data_dir.clone(),
            route_id: "pftl-uniswap-a777".to_string(),
            packet_hash: packet_hash.clone(),
        })
        .expect("consensus packet status");
        assert_eq!(packet.packet_hash, packet_hash);
        assert_eq!(packet.packet.amount_atoms, 40);
        assert_eq!(packet.packet.claim_class, "outstanding_bridge_claim");
        assert_eq!(packet.ledger_hash, routes.routes[0].ledger_hash);

        let supply = navcoin_bridge_supply_status(NavcoinBridgeSupplyStatusOptions {
            data_dir,
            route_id: "pftl-uniswap-a777".to_string(),
        })
        .expect("consensus supply status");
        assert_eq!(supply.authorized_valid_supply_atoms, 100);
        assert_eq!(supply.pftl_spendable_supply_atoms, 60);
        assert_eq!(supply.outstanding_bridge_claims_atoms, 40);
        assert!(supply.invariant_holds);
    }

    #[test]
    fn pftl_uniswap_terminal_state_and_receipt_recover_atomically_after_crash_prefixes() {
        let route_id = "pftl-uniswap-restart";
        let (mut bridge_ledger, packet_hash) = pftl_uniswap_test_ledger(route_id);
        pftl_uniswap_mark_destination_consumed(&mut bridge_ledger, &packet_hash)
            .expect("mark destination consumed in fixture");
        let mut route = pftl_uniswap_consensus_route_from_legacy_fixture(&bridge_ledger);
        route.route_trust_class = "BFT_CHECKPOINT".to_string();
        route.ethereum_verification_policy = Some(EthereumRouteVerificationPolicyV1 {
            authority_epoch: 9,
            committee_root: FastSwapCommitteeRootV1([0x91; 48]),
            minimum_confirmations: u32::try_from(route.return_finality_blocks)
                .expect("fixture finality fits u32"),
            handoff_controller_code_hash: [0x92; 32],
            wrapped_navcoin_code_hash: [0x93; 32],
        });
        for packet in route.export_packets.values_mut() {
            packet.ethereum_packet_digest = Some("94".repeat(32));
            packet.ethereum_packet_schema_version = Some(1);
        }
        route.validate().expect("valid terminal live route state");

        let consensus_receipt = PftlUniswapConsensusReceipt {
            receipt_hash: "95".repeat(48),
            transition: "destination_consume".to_string(),
            route_id: route_id.to_string(),
            state_before_hash: "96".repeat(48),
            state_after_hash: "97".repeat(48),
            packet_hash: Some(packet_hash.clone()),
            burn_event_hash: None,
            wallet: None,
            amount_atoms: Some(40),
            block_height: 1,
        };
        consensus_receipt
            .validate()
            .expect("valid bridge replay receipt");

        let tx_id = "98".repeat(48);
        let accepted_receipt = Receipt::accepted(&tx_id, "bridge destination consumed");
        let mut block = dummy_block_record(1);
        block.header.parent_hash = "genesis".to_string();
        block.header.batch_id = "99".repeat(48);
        block.header.receipt_count = 1;
        block.receipt_ids = vec![tx_id.clone()];
        let archive_entry = BatchArchiveEntry {
            batch_kind: block.header.batch_kind.clone(),
            batch_id: block.header.batch_id.clone(),
            payload_hash: "9a".repeat(48),
            payload_json: "{}".to_string(),
        };

        for write_prefix in 0..=6 {
            let data_dir = unique_test_dir(&format!(
                "postfiat-pftl-uniswap-restart-{write_prefix}"
            ));
            init(InitOptions {
                data_dir: data_dir.clone(),
                chain_id: "postfiat-local".to_string(),
                node_id: "validator-0".to_string(),
                validator_count: 1,
            })
            .expect("init bridge restart node");
            let store = NodeStore::new(&data_dir);
            let mut terminal_ledger = store.read_ledger().expect("initial ledger");
            terminal_ledger.pftl_uniswap_routes.push(route.clone());
            terminal_ledger
                .pftl_uniswap_receipts
                .push(consensus_receipt.clone());
            let initial_tip = read_chain_tip_or_reconstruct_for_genesis(
                &store,
                &store.read_genesis().expect("restart genesis"),
            )
            .expect("initial tip");
            let journal = OrderedCommitDeltaJournal {
                schema: "postfiat-ordered-commit-delta-journal-v1".to_string(),
                height: 1,
                ledger: Some(terminal_ledger.clone()),
                governance: None,
                shielded: None,
                bridge: None,
                receipt_delta: vec![accepted_receipt.clone()],
                ordered_batch_id: block.header.batch_id.clone(),
                archive_entry: archive_entry.clone(),
                block: block.clone(),
                validator_registry: None,
            };
            let terminal_tip = chain_tip_after_delta(&initial_tip, &journal)
                .expect("terminal bridge tip");
            store
                .write_ordered_commit_journal(&journal)
                .expect("persist bridge journal before mutation");

            if write_prefix >= 1 {
                store
                    .write_ledger(&terminal_ledger)
                    .expect("write terminal bridge ledger prefix");
            }
            if write_prefix >= 2 {
                store
                    .append_receipt_record(&accepted_receipt)
                    .expect("append bridge receipt prefix");
            }
            if write_prefix >= 3 {
                store
                    .append_ordered_batch_record(&journal.ordered_batch_id)
                    .expect("append bridge ordered batch prefix");
            }
            if write_prefix >= 4 {
                store
                    .append_batch_archive_entry(archive_entry.clone())
                    .expect("append bridge archive prefix");
            }
            if write_prefix >= 5 {
                store
                    .append_block_record(&block)
                    .expect("append bridge block prefix");
            }
            if write_prefix >= 6 {
                store
                    .write_chain_tip(&terminal_tip)
                    .expect("write bridge tip prefix");
            }

            status(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("restart recovers bridge journal");
            let restarted_store = NodeStore::new(&data_dir);
            let recovered = restarted_store.read_ledger().expect("recovered bridge ledger");
            assert_eq!(recovered, terminal_ledger);
            assert_eq!(
                recovered.pftl_uniswap_routes[0].export_packets[&packet_hash].status,
                PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED
            );
            assert_eq!(
                restarted_store.read_receipts().expect("recovered receipts"),
                vec![accepted_receipt.clone()]
            );
            assert_eq!(
                restarted_store.read_chain_tip().expect("recovered tip"),
                terminal_tip
            );
            assert!(restarted_store
                .read_ordered_commit_journal::<OrderedCommitDeltaJournal>()
                .expect("journal read after recovery")
                .is_none());

            status(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("second restart is idempotent");
            assert_eq!(
                NodeStore::new(&data_dir)
                    .read_ledger()
                    .expect("idempotent bridge ledger"),
                terminal_ledger
            );
            std::fs::remove_dir_all(data_dir).expect("remove bridge restart dir");
        }
    }

    #[test]
    fn replicated_state_root_counts_external_bridge_inventory_in_global_supply() {
        let genesis = Genesis::new("postfiat-local");
        let governance = GovernanceState::new(1);
        let issuer = "pfbridgeissuer";
        let holder = "pfbridgeholder";
        let mut asset = AssetDefinition::new(&genesis.chain_id, issuer, "BRGCAP", 1, 0)
            .expect("bridge-cap asset");
        asset.max_supply = Some(99);
        let mut line = TrustLine::new(holder, issuer, asset.asset_id.clone(), 60, 60)
            .expect("bridge-cap trustline");
        line.balance = 60;
        line.authorized = true;
        let mut ledger = LedgerState::new(vec![
            Account::new(issuer, 1, None),
            Account::new(holder, 1, None),
        ]);
        ledger.asset_definitions.push(asset.clone());
        ledger.trustlines.push(line);

        let (mut bridge_ledger, packet_hash) = pftl_uniswap_test_ledger("external-supply");
        pftl_uniswap_mark_destination_consumed(&mut bridge_ledger, &packet_hash)
            .expect("move bridge claim to Ethereum inventory");
        let mut route = pftl_uniswap_consensus_route_from_legacy_fixture(&bridge_ledger);
        route.native_nav_asset_id = asset.asset_id.clone();
        route.route_trust_class = "BFT_CHECKPOINT".to_string();
        route.ethereum_verification_policy = Some(EthereumRouteVerificationPolicyV1 {
            authority_epoch: 9,
            committee_root: FastSwapCommitteeRootV1([0xa1; 48]),
            minimum_confirmations: u32::try_from(route.return_finality_blocks)
                .expect("fixture finality fits u32"),
            handoff_controller_code_hash: [0xa2; 32],
            wrapped_navcoin_code_hash: [0xa3; 32],
        });
        route
            .validate()
            .expect("valid route with 60 public plus 40 Ethereum atoms");
        ledger.pftl_uniswap_routes.push(route);

        let error = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &[],
            &ShieldedState::empty(),
            &BridgeState::empty(),
        )
        .expect_err("60 public plus 40 Ethereum atoms must exceed cap 99");
        assert!(
            error.to_string().contains("issued asset supply cap exceeded"),
            "{error}"
        );

        ledger.asset_definitions[0].max_supply = Some(100);
        replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &[],
            &ShieldedState::empty(),
            &BridgeState::empty(),
        )
        .expect("60 public plus 40 Ethereum atoms exactly meets cap 100");
    }

    #[test]
    fn navcoin_bridge_receipt_replay_accepts_clean_empty_route() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-empty-replay-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let route_id = "pftl-uniswap-a666";
        let config = pftl_uniswap_test_config(route_id);
        let config_file = write_pftl_uniswap_test_json(&data_dir, "route-config.json", &config);
        navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
            data_dir: data_dir.clone(),
            config_file,
            ethereum_chain_id: 42_161,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 64,
            replace: false,
        })
        .expect("route init");

        let report = navcoin_bridge_receipt_replay(NavcoinBridgeReceiptReplayOptions {
            data_dir,
            route_id: route_id.to_string(),
        })
        .expect("empty receipt replay");
        assert_eq!(report.schema, "postfiat-navcoin-bridge-receipt-replay-v1");
        assert_eq!(report.route_id, route_id);
        assert_eq!(report.receipt_count, 0);
        assert_eq!(report.status, "empty_clean");
        assert!(report.receipt_root.is_none());
        assert!(report.replay.is_none());
        assert_eq!(report.initial_ledger_hash, report.final_ledger_hash);
    }

    #[test]
    fn navcoin_bridge_status_rejects_duplicate_persisted_route_ids() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-duplicate-route-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let (ledger, _) = pftl_uniswap_test_ledger("pftl-uniswap-a666");
        write_pftl_uniswap_test_ledgers(&data_dir, &[ledger.clone(), ledger]);

        let error = navcoin_bridge_routes(NavcoinBridgeRoutesOptions { data_dir })
            .expect_err("duplicate route id must fail");
        assert!(
            error.to_string().contains("duplicate PFTL-to-Uniswap bridge route id"),
            "{error}"
        );
    }

    #[test]
    fn navcoin_bridge_launch_config_template_writes_digest_bound_config() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-launch-template-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let route_id = "pftl-uniswap-a666";
        let config = pftl_uniswap_test_config(route_id);
        let expected_launch_config = pftl_uniswap_test_launch_config(&config);
        let config_file = write_pftl_uniswap_test_json(&data_dir, "route-config.json", &config);
        let official_uniswap_file = write_pftl_uniswap_test_json(
            &data_dir,
            "official-uniswap.json",
            &expected_launch_config.official_uniswap,
        );
        let output_file = data_dir.join("generated-launch-config.json");

        let report =
            navcoin_bridge_launch_config_template(NavcoinBridgeLaunchConfigTemplateOptions {
                route_config_file: config_file.clone(),
                official_uniswap_file,
                usdc_token: expected_launch_config.usdc_token.clone(),
                receipt_verifier: expected_launch_config.receipt_verifier.clone(),
                uniswap_pool_key_hash: expected_launch_config.uniswap_pool_key_hash.clone(),
                pricing_reserve_packet_hash: expected_launch_config
                    .seed
                    .pricing_reserve_packet_hash
                    .clone(),
                nav_price_settlement_atoms_per_nav_atom: expected_launch_config
                    .seed
                    .nav_price_settlement_atoms_per_nav_atom,
                tick_lower: expected_launch_config.seed.tick_lower,
                tick_upper: expected_launch_config.seed.tick_upper,
                fee_pips: expected_launch_config.seed.fee_pips,
                position_recipient: expected_launch_config.seed.position_recipient.clone(),
                output_file: output_file.clone(),
                overwrite: false,
            })
            .expect("launch config template");

        assert_eq!(report.schema, "postfiat-navcoin-bridge-launch-config-template-v1");
        assert_eq!(report.route_id, route_id);
        assert_eq!(
            report.route_config_digest,
            pftl_uniswap_route_config_digest(&config).expect("route config digest")
        );
        assert_eq!(report.launch_config, expected_launch_config);
        assert_eq!(
            report.launch_config_digest,
            pftl_uniswap_launch_config_digest(&report.launch_config)
                .expect("launch config digest")
        );

        let written: PftlUniswapLaunchConfig =
            read_json_file(&output_file, "generated launch config")
                .expect("read generated launch config");
        assert_eq!(written, report.launch_config);

        let duplicate = navcoin_bridge_launch_config_template(
            NavcoinBridgeLaunchConfigTemplateOptions {
                route_config_file: config_file.clone(),
                official_uniswap_file: data_dir.join("official-uniswap.json"),
                usdc_token: expected_launch_config.usdc_token.clone(),
                receipt_verifier: expected_launch_config.receipt_verifier.clone(),
                uniswap_pool_key_hash: expected_launch_config.uniswap_pool_key_hash.clone(),
                pricing_reserve_packet_hash: expected_launch_config
                    .seed
                    .pricing_reserve_packet_hash
                    .clone(),
                nav_price_settlement_atoms_per_nav_atom: expected_launch_config
                    .seed
                    .nav_price_settlement_atoms_per_nav_atom,
                tick_lower: expected_launch_config.seed.tick_lower,
                tick_upper: expected_launch_config.seed.tick_upper,
                fee_pips: expected_launch_config.seed.fee_pips,
                position_recipient: expected_launch_config.seed.position_recipient.clone(),
                output_file: output_file.clone(),
                overwrite: false,
            },
        )
        .expect_err("existing template output must require overwrite");
        assert_eq!(duplicate.kind(), std::io::ErrorKind::AlreadyExists);

        navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
            data_dir: data_dir.clone(),
            config_file,
            ethereum_chain_id: 42_161,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 64,
            replace: false,
        })
        .expect("route init");
        let init_report =
            navcoin_bridge_launch_config_init(NavcoinBridgeLaunchConfigInitOptions {
                data_dir,
                launch_config_file: output_file,
                replace: false,
            })
            .expect("generated launch config init");
        assert_eq!(init_report.route_id, route_id);
        assert_eq!(init_report.launch_config_count, 1);
    }

    #[test]
    fn navcoin_bridge_records_launch_config_and_fork_rehearsal_evidence() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-launch-rehearsal-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let route_id = "pftl-uniswap-a666";
        let config = pftl_uniswap_test_config(route_id);
        let config_file = write_pftl_uniswap_test_json(&data_dir, "route-config.json", &config);
        navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
            data_dir: data_dir.clone(),
            config_file,
            ethereum_chain_id: 42_161,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 64,
            replace: false,
        })
        .expect("route init");

        let launch_config = pftl_uniswap_test_launch_config(&config);
        let launch_config_file =
            write_pftl_uniswap_test_json(&data_dir, "launch-config.json", &launch_config);
        let launch_report =
            navcoin_bridge_launch_config_init(NavcoinBridgeLaunchConfigInitOptions {
                data_dir: data_dir.clone(),
                launch_config_file,
                replace: false,
            })
            .expect("launch config init");
        assert_eq!(launch_report.route_id, route_id);
        assert_eq!(launch_report.route_config_digest, launch_config.route_config_digest);
        assert_eq!(launch_report.launch_config_count, 1);

        let evidence = pftl_uniswap_test_fork_rehearsal(&launch_config);
        let evidence_file = write_pftl_uniswap_test_json(&data_dir, "fork-evidence.json", &evidence);
        let evidence_report =
            navcoin_bridge_record_fork_rehearsal(NavcoinBridgeRecordForkRehearsalOptions {
                data_dir: data_dir.clone(),
                route_id: route_id.to_string(),
                evidence_file: evidence_file.clone(),
            })
            .expect("fork rehearsal record");
        assert_eq!(evidence_report.route_id, route_id);
        assert_eq!(evidence_report.rehearsal_id, evidence.rehearsal_id);
        assert_eq!(evidence_report.evidence_count, 1);

        let duplicate =
            navcoin_bridge_record_fork_rehearsal(NavcoinBridgeRecordForkRehearsalOptions {
                data_dir: data_dir.clone(),
                route_id: route_id.to_string(),
                evidence_file,
            })
            .expect_err("duplicate rehearsal must fail");
        assert_eq!(duplicate.kind(), std::io::ErrorKind::AlreadyExists);

        let mut bad_evidence = pftl_uniswap_test_fork_rehearsal(&launch_config);
        bad_evidence.rehearsal_id = "gate-3-fork-rehearsal-bad-supply".to_string();
        bad_evidence.canonical_supply_after_external_trades_atoms += 1;
        let bad_evidence_file =
            write_pftl_uniswap_test_json(&data_dir, "bad-fork-evidence.json", &bad_evidence);
        let error = navcoin_bridge_record_fork_rehearsal(NavcoinBridgeRecordForkRehearsalOptions {
            data_dir: data_dir.clone(),
            route_id: route_id.to_string(),
            evidence_file: bad_evidence_file,
        })
        .expect_err("supply-changing rehearsal must fail");
        assert!(
            error.to_string().contains("external_trade_supply_changed"),
            "{error}"
        );

        let launch_configs: Vec<PftlUniswapLaunchConfig> = read_json_file(
            &data_dir.join(PFTL_UNISWAP_LAUNCH_CONFIG_FILE),
            "test launch configs",
        )
        .expect("launch config sidecar");
        assert_eq!(launch_configs.len(), 1);
        let evidences: Vec<PftlUniswapForkRehearsalEvidence> = read_json_file(
            &data_dir.join(PFTL_UNISWAP_FORK_REHEARSAL_FILE),
            "test fork rehearsals",
        )
        .expect("fork rehearsal sidecar");
        assert_eq!(evidences.len(), 1);
        assert_eq!(evidences[0].rehearsal_id, "gate-3-fork-rehearsal-test");
    }

    #[test]
    fn navcoin_bridge_packet_preflight_enforces_launch_bound_pricing() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-packet-preflight-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let route_id = "pftl-uniswap-a666";
        let config = pftl_uniswap_test_config(route_id);
        let config_file = write_pftl_uniswap_test_json(&data_dir, "route-config.json", &config);
        navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
            data_dir: data_dir.clone(),
            config_file,
            ethereum_chain_id: 42_161,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 64,
            replace: false,
        })
        .expect("route init");

        let launch_config = pftl_uniswap_test_launch_config(&config);
        let launch_config_file =
            write_pftl_uniswap_test_json(&data_dir, "launch-config.json", &launch_config);
        navcoin_bridge_launch_config_init(NavcoinBridgeLaunchConfigInitOptions {
            data_dir: data_dir.clone(),
            launch_config_file,
            replace: false,
        })
        .expect("launch config init");

        let packet = pftl_uniswap_test_packet(&config, &launch_config);
        let packet_file = write_pftl_uniswap_test_json(&data_dir, "packet.json", &packet);
        let report = navcoin_bridge_packet_preflight(NavcoinBridgePacketPreflightOptions {
            data_dir: data_dir.clone(),
            route_id: route_id.to_string(),
            packet_file,
        })
        .expect("packet preflight");
        assert_eq!(report.schema, "postfiat-navcoin-bridge-packet-preflight-v1");
        assert_eq!(report.route_id, route_id);
        assert_eq!(report.route_config_digest, launch_config.route_config_digest);
        assert_eq!(
            report.launch_config_digest,
            pftl_uniswap_launch_config_digest(&launch_config).expect("launch config digest")
        );
        assert_eq!(
            report.packet_digest,
            pftl_uniswap_packet_id(&packet).expect("packet digest")
        );
        assert_eq!(report.status, "ready");

        let mut stale_reserve_packet = packet;
        stale_reserve_packet.pricing_reserve_packet_hash = pftl_uniswap_test_hex("de", 48);
        assert_navcoin_bridge_packet_preflight_error(
            &data_dir,
            route_id,
            "stale-reserve-packet.json",
            &stale_reserve_packet,
            "launch_pricing_reserve_packet_mismatch",
        );
    }

    #[test]
    fn navcoin_bridge_packet_preflight_rejects_any_launch_handoff_mismatch() {
        let data_dir =
            unique_test_dir("postfiat-navcoin-bridge-packet-preflight-mismatch-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let route_id = "pftl-uniswap-a666";
        let config = pftl_uniswap_test_config(route_id);
        let config_file = write_pftl_uniswap_test_json(&data_dir, "route-config.json", &config);
        navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
            data_dir: data_dir.clone(),
            config_file,
            ethereum_chain_id: 42_161,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 64,
            replace: false,
        })
        .expect("route init");

        let launch_config = pftl_uniswap_test_launch_config(&config);
        let launch_config_file =
            write_pftl_uniswap_test_json(&data_dir, "launch-config.json", &launch_config);
        navcoin_bridge_launch_config_init(NavcoinBridgeLaunchConfigInitOptions {
            data_dir: data_dir.clone(),
            launch_config_file,
            replace: false,
        })
        .expect("launch config init");

        let packet = pftl_uniswap_test_packet(&config, &launch_config);

        let mut wrong_route_digest = packet.clone();
        wrong_route_digest.config_digest = pftl_uniswap_test_hex("d0", 48);
        assert_navcoin_bridge_packet_preflight_error(
            &data_dir,
            route_id,
            "wrong-route-digest-packet.json",
            &wrong_route_digest,
            "route_config_digest_mismatch",
        );

        let mut wrong_settlement_asset = packet.clone();
        wrong_settlement_asset.settlement_asset_id = pftl_uniswap_test_hex("d1", 48);
        assert_navcoin_bridge_packet_preflight_error(
            &data_dir,
            route_id,
            "wrong-settlement-asset-packet.json",
            &wrong_settlement_asset,
            "launch_packet_config_mismatch",
        );

        let mut wrong_native_asset = packet.clone();
        wrong_native_asset.native_nav_asset_id = pftl_uniswap_test_hex("d2", 48);
        assert_navcoin_bridge_packet_preflight_error(
            &data_dir,
            route_id,
            "wrong-native-asset-packet.json",
            &wrong_native_asset,
            "launch_packet_config_mismatch",
        );

        let mut wrong_wrapped_token = packet.clone();
        wrong_wrapped_token.wrapped_navcoin_token = pftl_uniswap_test_address("d6");
        assert_navcoin_bridge_packet_preflight_error(
            &data_dir,
            route_id,
            "wrong-wrapped-token-packet.json",
            &wrong_wrapped_token,
            "launch_packet_config_mismatch",
        );

        let mut wrong_pool = packet.clone();
        wrong_pool.uniswap_pool_id_or_path = format!("0x{}", pftl_uniswap_test_hex("d3", 32));
        assert_navcoin_bridge_packet_preflight_error(
            &data_dir,
            route_id,
            "wrong-pool-packet.json",
            &wrong_pool,
            "launch_packet_config_mismatch",
        );

        let mut wrong_usdc_output = packet.clone();
        wrong_usdc_output.token_out = pftl_uniswap_test_address("d4");
        assert_navcoin_bridge_packet_preflight_error(
            &data_dir,
            route_id,
            "wrong-usdc-output-packet.json",
            &wrong_usdc_output,
            "launch_packet_config_mismatch",
        );

        let mut wrong_nav_epoch = packet.clone();
        wrong_nav_epoch.pricing_nav_epoch += 1;
        assert_navcoin_bridge_packet_preflight_error(
            &data_dir,
            route_id,
            "wrong-nav-epoch-packet.json",
            &wrong_nav_epoch,
            "launch_pricing_nav_epoch_mismatch",
        );

        let mut wrong_reserve_packet = packet;
        wrong_reserve_packet.pricing_reserve_packet_hash = pftl_uniswap_test_hex("d5", 48);
        assert_navcoin_bridge_packet_preflight_error(
            &data_dir,
            route_id,
            "wrong-reserve-packet.json",
            &wrong_reserve_packet,
            "launch_pricing_reserve_packet_mismatch",
        );
    }

    #[test]
    fn navcoin_bridge_operator_mutations_persist_ledger_and_receipts() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-mutation-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let route_id = "pftl-uniswap-a666";
        let config = pftl_uniswap_test_config(route_id);
        let config_file = write_pftl_uniswap_test_json(&data_dir, "route-config.json", &config);

        let init = navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
            data_dir: data_dir.clone(),
            config_file,
            ethereum_chain_id: 42_161,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 64,
            replace: false,
        })
        .expect("route init");
        assert_eq!(init.route_id, route_id);
        assert_eq!(init.route_count, 1);

        let primary = PftlUniswapPrimarySubscriptionRequest {
            route_id: route_id.to_string(),
            source_wallet: "pfsourcewallet".to_string(),
            settlement_asset_id: config.settlement_asset_id.clone(),
            subscription_nonce: pftl_uniswap_test_hex("88", 32),
            quote: PrimarySubscriptionQuoteInput {
                settlement_value_atoms: 200,
                nav_price_settlement_atoms_per_nav_atom: 2,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pftl_uniswap_test_hex("99", 48),
            },
        };
        let primary_file = write_pftl_uniswap_test_json(&data_dir, "primary.json", &primary);
        let primary_report =
            navcoin_bridge_primary_subscribe(NavcoinBridgePrimarySubscribeOptions {
                data_dir: data_dir.clone(),
                request_file: primary_file.clone(),
            })
            .expect("primary subscription");
        assert_eq!(primary_report.transition, "primary_subscription");
        assert_eq!(primary_report.result["minted_nav_atoms"], serde_json::json!(100));
        assert_eq!(
            primary_report.result["requested_settlement_atoms"],
            serde_json::json!(200)
        );
        assert_eq!(
            primary_report.result["accepted_settlement_atoms"],
            serde_json::json!(200)
        );
        assert_eq!(
            primary_report.result["refund_settlement_atoms"],
            serde_json::json!(0)
        );
        let duplicate_primary =
            navcoin_bridge_primary_subscribe(NavcoinBridgePrimarySubscribeOptions {
                data_dir: data_dir.clone(),
                request_file: primary_file,
            })
            .expect_err("duplicate primary subscription nonce must fail");
        assert!(
            duplicate_primary
                .to_string()
                .contains("duplicate_primary_subscription_nonce"),
            "{duplicate_primary}"
        );

        let packet_hash = pftl_uniswap_test_hex("aa", 48);
        let export = PftlUniswapExportDebitRequest {
            route_id: route_id.to_string(),
            packet_hash: packet_hash.clone(),
            nonce: pftl_uniswap_test_hex("bb", 32),
            source_wallet: "pfsourcewallet".to_string(),
            ethereum_recipient: pftl_uniswap_test_address("cc"),
            amount_atoms: 40,
            source_height: 10,
            destination_deadline_seconds: 1_800,
            refund_not_before_height: 20,
        };
        let export_file = write_pftl_uniswap_test_json(&data_dir, "export.json", &export);
        let export_report = navcoin_bridge_export_debit(NavcoinBridgeExportDebitOptions {
            data_dir: data_dir.clone(),
            request_file: export_file,
        })
        .expect("export debit");
        assert_eq!(export_report.transition, "export_debit");

        let consume_report =
            navcoin_bridge_destination_consume(NavcoinBridgeDestinationConsumeOptions {
                data_dir: data_dir.clone(),
                route_id: route_id.to_string(),
                packet_hash: packet_hash.clone(),
            })
            .expect("destination consume");
        assert_eq!(consume_report.transition, "destination_consumed");

        let mut burn = PftlUniswapReturnBurnRequest {
            burn_event_hash: pftl_uniswap_test_hex("00", 32),
            ethereum_chain_id: 42_161,
            bridge_controller: config.handoff_controller.clone(),
            wrapped_navcoin_token: config.wrapped_navcoin_token.clone(),
            native_nav_asset_id: config.native_nav_asset_id.clone(),
            ethereum_sender: pftl_uniswap_test_address("dd"),
            pftl_recipient: "pfreturnrecipient".to_string(),
            amount_atoms: 25,
            return_nonce: pftl_uniswap_test_hex("ee", 32),
            burn_height: 100,
            finalized_height: 164,
        };
        burn.burn_event_hash = pftl_uniswap_return_burn_id(&burn).expect("return burn id");
        let burn_hash = burn.burn_event_hash.clone();
        let burn_file = write_pftl_uniswap_test_json(&data_dir, "return-burn.json", &burn);
        let burn_report =
            navcoin_bridge_record_return_burn(NavcoinBridgeRecordReturnBurnOptions {
                data_dir: data_dir.clone(),
                route_id: route_id.to_string(),
                request_file: burn_file,
            })
            .expect("record return burn");
        assert_eq!(burn_report.transition, "return_burn_observed");

        let import_report = navcoin_bridge_import_return(NavcoinBridgeImportReturnOptions {
            data_dir: data_dir.clone(),
            route_id: route_id.to_string(),
            burn_event_hash: burn_hash,
            pftl_recipient: "pfreturnrecipient".to_string(),
        })
        .expect("import return");
        assert_eq!(import_report.transition, "return_imported");

        let supply = navcoin_bridge_supply_status(NavcoinBridgeSupplyStatusOptions {
            data_dir: data_dir.clone(),
            route_id: route_id.to_string(),
        })
        .expect("supply status");
        assert!(supply.invariant_holds);
        assert_eq!(supply.authorized_valid_supply_atoms, 100);
        assert_eq!(supply.pftl_spendable_supply_atoms, 85);
        assert_eq!(supply.native_spendable_balance_count, 2);
        assert_eq!(
            supply.native_spendable_balance_limit,
            PFTL_UNISWAP_STATUS_MAX_ROWS as u64
        );
        assert!(!supply.native_spendable_balances_truncated);
        assert_eq!(supply.native_spendable_balance_sum_atoms, 85);
        assert_eq!(supply.native_spendable_balances.len(), 2);
        assert_eq!(supply.native_spendable_balances[0].wallet, "pfreturnrecipient");
        assert_eq!(supply.native_spendable_balances[0].amount_atoms, 25);
        assert_eq!(supply.native_spendable_balances[1].wallet, "pfsourcewallet");
        assert_eq!(supply.native_spendable_balances[1].amount_atoms, 60);
        assert_eq!(supply.ethereum_spendable_supply_atoms, 15);
        assert_eq!(supply.outstanding_bridge_claims_atoms, 0);
        assert_eq!(supply.pending_return_import_claims_atoms, 0);

        let receipts: Vec<PftlUniswapTransitionReceipt> =
            read_json_file(&data_dir.join(PFTL_UNISWAP_BRIDGE_RECEIPTS_FILE), "test receipts")
                .expect("receipt sidecar");
        assert_eq!(receipts.len(), 5);
        assert_eq!(receipts[0].transition, "primary_subscription");
        assert_eq!(receipts[0].nonce.as_deref(), Some(primary.subscription_nonce.as_str()));
        assert_eq!(receipts[0].source_wallet.as_deref(), Some(primary.source_wallet.as_str()));
        assert_eq!(
            receipts[0].settlement_asset_id.as_deref(),
            Some(primary.settlement_asset_id.as_str())
        );
        assert_eq!(receipts[0].requested_settlement_atoms, Some(200));
        assert_eq!(receipts[0].accepted_settlement_atoms, Some(200));
        assert_eq!(receipts[0].refund_settlement_atoms, Some(0));
        assert_eq!(receipts[0].minted_nav_atoms, Some(100));
        assert_eq!(receipts[0].pricing_nav_epoch, Some(primary.quote.pricing_nav_epoch));
        assert_eq!(
            receipts[0].pricing_reserve_packet_hash.as_deref(),
            Some(primary.quote.pricing_reserve_packet_hash.as_str())
        );
        assert_eq!(receipts[4].transition, "return_imported");

        let initial_ledger =
            pftl_uniswap_bridge_ledger_from_config(&config, 42_161, 7, 64)
                .expect("initial replay ledger");
        let final_ledgers: Vec<PftlUniswapBridgeLedger> = read_json_file(
            &data_dir.join(PFTL_UNISWAP_BRIDGE_LEDGER_FILE),
            "test final ledgers",
        )
        .expect("final ledger sidecar");
        let final_ledger = final_ledgers
            .iter()
            .find(|ledger| ledger.route_id == route_id)
            .expect("final route ledger");
        assert_eq!(
            final_ledger
                .primary_subscription_nonces
                .get(&primary.subscription_nonce)
                .map(String::as_str),
            Some(primary.source_wallet.as_str())
        );
        let replay_report = pftl_uniswap_verify_transition_receipt_replay(
            &initial_ledger,
            &receipts,
            final_ledger,
        )
        .expect("persisted receipt replay");
        assert_eq!(replay_report.receipt_count, 5);
        assert_eq!(replay_report.final_ledger_hash, supply.ledger_hash);

        let sidecar_replay = navcoin_bridge_receipt_replay(NavcoinBridgeReceiptReplayOptions {
            data_dir: data_dir.clone(),
            route_id: route_id.to_string(),
        })
        .expect("sidecar receipt replay");
        assert_eq!(sidecar_replay.status, "verified");
        assert_eq!(sidecar_replay.receipt_count, 5);
        assert_eq!(
            sidecar_replay.receipt_root.as_deref(),
            Some(replay_report.receipt_root.as_str())
        );
        assert_eq!(
            sidecar_replay
                .replay
                .as_ref()
                .expect("nested replay report")
                .final_ledger_hash,
            supply.ledger_hash
        );

        let mut tampered = receipts.clone();
        tampered[1].amount_atoms = Some(41);
        write_pftl_uniswap_test_json(
            &data_dir,
            PFTL_UNISWAP_BRIDGE_RECEIPTS_FILE,
            &tampered,
        );
        let tamper_error = navcoin_bridge_receipt_replay(NavcoinBridgeReceiptReplayOptions {
            data_dir,
            route_id: route_id.to_string(),
        })
        .expect_err("tampered sidecar replay must fail");
        assert!(
            tamper_error.to_string().contains("receipt_replay_mismatch"),
            "{tamper_error}"
        );
    }

    #[test]
    fn navcoin_bridge_refund_source_persists_receipt_and_replays() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-refund-source-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let route_id = "pftl-uniswap-a666";
        let config = pftl_uniswap_test_config(route_id);
        let config_file = write_pftl_uniswap_test_json(&data_dir, "route-config.json", &config);

        navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
            data_dir: data_dir.clone(),
            config_file,
            ethereum_chain_id: 42_161,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 64,
            replace: false,
        })
        .expect("route init");

        let primary = PftlUniswapPrimarySubscriptionRequest {
            route_id: route_id.to_string(),
            source_wallet: "pfsourcewallet".to_string(),
            settlement_asset_id: config.settlement_asset_id.clone(),
            subscription_nonce: pftl_uniswap_test_hex("88", 32),
            quote: PrimarySubscriptionQuoteInput {
                settlement_value_atoms: 200,
                nav_price_settlement_atoms_per_nav_atom: 2,
                pricing_nav_epoch: 7,
                pricing_reserve_packet_hash: pftl_uniswap_test_hex("99", 48),
            },
        };
        let primary_file = write_pftl_uniswap_test_json(&data_dir, "primary.json", &primary);
        navcoin_bridge_primary_subscribe(NavcoinBridgePrimarySubscribeOptions {
            data_dir: data_dir.clone(),
            request_file: primary_file,
        })
        .expect("primary subscription");

        let packet_hash = pftl_uniswap_test_hex("aa", 48);
        let export = PftlUniswapExportDebitRequest {
            route_id: route_id.to_string(),
            packet_hash: packet_hash.clone(),
            nonce: pftl_uniswap_test_hex("bb", 32),
            source_wallet: primary.source_wallet.clone(),
            ethereum_recipient: pftl_uniswap_test_address("cc"),
            amount_atoms: 40,
            source_height: 10,
            destination_deadline_seconds: 1_800,
            refund_not_before_height: 20,
        };
        let export_file = write_pftl_uniswap_test_json(&data_dir, "export.json", &export);
        navcoin_bridge_export_debit(NavcoinBridgeExportDebitOptions {
            data_dir: data_dir.clone(),
            request_file: export_file,
        })
        .expect("export debit");

        let refund = PftlUniswapRefundRequest {
            packet_hash: packet_hash.clone(),
            current_height: export.refund_not_before_height,
            non_consumption_proof_hash: pftl_uniswap_non_consumption_proof_hash(
                route_id,
                &packet_hash,
                export.refund_not_before_height,
            )
            .expect("non-consumption proof commitment"),
        };
        let refund_file = write_pftl_uniswap_test_json(&data_dir, "refund.json", &refund);
        let refund_report = navcoin_bridge_refund_source(NavcoinBridgeRefundSourceOptions {
            data_dir: data_dir.clone(),
            route_id: route_id.to_string(),
            request_file: refund_file,
        })
        .expect("refund source");
        assert_eq!(refund_report.transition, "source_refunded");
        assert_eq!(
            refund_report.result["status"],
            serde_json::json!("SourceRefunded")
        );
        assert_eq!(
            refund_report.result["packet_hash"],
            serde_json::json!(packet_hash)
        );

        let supply = navcoin_bridge_supply_status(NavcoinBridgeSupplyStatusOptions {
            data_dir: data_dir.clone(),
            route_id: route_id.to_string(),
        })
        .expect("supply status");
        assert!(supply.invariant_holds);
        assert_eq!(supply.authorized_valid_supply_atoms, 100);
        assert_eq!(supply.pftl_spendable_supply_atoms, 100);
        assert_eq!(supply.outstanding_bridge_claims_atoms, 0);
        assert_eq!(supply.native_spendable_balance_count, 1);
        assert_eq!(supply.native_spendable_balance_sum_atoms, 100);
        assert_eq!(supply.native_spendable_balances[0].wallet, "pfsourcewallet");
        assert_eq!(supply.native_spendable_balances[0].amount_atoms, 100);

        let receipts: Vec<PftlUniswapTransitionReceipt> =
            read_json_file(&data_dir.join(PFTL_UNISWAP_BRIDGE_RECEIPTS_FILE), "test receipts")
                .expect("receipt sidecar");
        assert_eq!(receipts.len(), 3);
        assert_eq!(receipts[0].transition, "primary_subscription");
        assert_eq!(receipts[1].transition, "export_debit");
        assert_eq!(receipts[2].transition, "source_refunded");
        assert_eq!(receipts[2].packet_hash.as_deref(), Some(packet_hash.as_str()));
        assert_eq!(receipts[2].nonce.as_deref(), Some(export.nonce.as_str()));
        assert_eq!(
            receipts[2].source_wallet.as_deref(),
            Some(primary.source_wallet.as_str())
        );
        assert_eq!(receipts[2].amount_atoms, Some(export.amount_atoms));
        assert_eq!(
            receipts[2].refund_not_before_height,
            Some(export.refund_not_before_height)
        );
        assert_eq!(
            receipts[2].non_consumption_proof_hash.as_deref(),
            Some(refund.non_consumption_proof_hash.as_str())
        );

        let replay = navcoin_bridge_receipt_replay(NavcoinBridgeReceiptReplayOptions {
            data_dir,
            route_id: route_id.to_string(),
        })
        .expect("sidecar receipt replay");
        assert_eq!(replay.status, "verified");
        assert_eq!(replay.receipt_count, 3);
        assert_eq!(
            replay
                .replay
                .as_ref()
                .expect("nested replay report")
                .final_ledger_hash,
            supply.ledger_hash
        );
    }

    #[test]
    fn navcoin_bridge_return_burn_request_derives_canonical_burn_id() {
        let data_dir = unique_test_dir("postfiat-navcoin-bridge-return-burn-request-test");
        std::fs::create_dir_all(&data_dir).expect("test dir");
        let route_id = "pftl-uniswap-a666";
        let config = pftl_uniswap_test_config(route_id);
        let config_file = write_pftl_uniswap_test_json(&data_dir, "route-config.json", &config);
        navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
            data_dir: data_dir.clone(),
            config_file,
            ethereum_chain_id: 42_161,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 64,
            replace: false,
        })
        .expect("route init");

        let output_file = data_dir.join("return-burn.json");
        let report = navcoin_bridge_return_burn_request(NavcoinBridgeReturnBurnRequestOptions {
            data_dir: data_dir.clone(),
            route_id: route_id.to_string(),
            ethereum_sender: pftl_uniswap_test_address("dd"),
            pftl_recipient: "pfreturnrecipient".to_string(),
            amount_atoms: 25,
            return_nonce: pftl_uniswap_test_hex("ee", 32),
            burn_height: 100,
            output_file: output_file.clone(),
            overwrite: false,
        })
        .expect("return burn request");
        assert_eq!(
            report.schema,
            "postfiat-navcoin-bridge-return-burn-request-v1"
        );
        assert_eq!(report.route_id, route_id);
        assert_eq!(report.request.ethereum_chain_id, 42_161);
        assert_eq!(report.request.bridge_controller, config.handoff_controller);
        assert_eq!(
            report.request.wrapped_navcoin_token,
            config.wrapped_navcoin_token
        );
        assert_eq!(report.request.native_nav_asset_id, config.native_nav_asset_id);
        assert_eq!(report.request.finalized_height, 164);
        assert_eq!(
            report.burn_event_hash,
            pftl_uniswap_return_burn_id(&report.request).expect("canonical burn id")
        );

        let persisted: PftlUniswapReturnBurnRequest =
            read_json_file(&output_file, "persisted return burn request")
                .expect("persisted return burn request");
        assert_eq!(persisted, report.request);
    }
