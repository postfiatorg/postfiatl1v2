    fn receipt_test_id(label: &str) -> String {
        hash_hex("postfiat.test.private_pool_receipts", label.as_bytes())
    }

    /// One admission record; `inclusion` is (height, receipt id) for an
    /// included batch, `final_state` None for a still-pending record.
    fn receipt_test_record(
        label: &str,
        final_state: Option<PrivateActionState>,
        inclusion: Option<u64>,
        received_height: u64,
        scope: &str,
        action_json: serde_json::Value,
    ) -> PrivateActionRecord {
        let outcome = final_state.map(|state| PrivateActionOutcome {
            accepted: state == PrivateActionState::Accepted,
            code: if state == PrivateActionState::Accepted {
                "accepted".to_string()
            } else {
                "asset_orchard_swap_proof_verification_failed".to_string()
            },
            message: String::new(),
            fee_charged: if inclusion.is_some() { 7 } else { 0 },
            state_effects: if inclusion.is_some() { "applied" } else { "none" }.to_string(),
            inclusion: inclusion.map(|height| PrivateActionInclusion {
                height,
                block_hash: receipt_test_id(&format!("block-{height}")),
                state_root: receipt_test_id(&format!("root-{height}")),
                receipt_ids: vec![receipt_test_id(&format!("tx-{label}"))],
                certificate_id: receipt_test_id(&format!("cert-{height}")),
                certificate: serde_json::json!({}),
                certificate_json: "{}".to_string(),
                consensus_v2_commit: serde_json::json!({}),
            }),
        });
        PrivateActionRecord {
            schema: PRIVATE_ACTION_RECORD_SCHEMA.to_string(),
            intent_id: receipt_test_id(&format!("intent-{label}")),
            domain: PrivateActionSubmitDomain {
                chain_id: "postfiat-asset-orchard-pricing-read".to_string(),
                genesis_hash: "00".repeat(48),
                protocol_version: 1,
                pool_id: PRIVATE_ACTION_POOL_ID.to_string(),
                session_scope: scope.to_string(),
            },
            action_hash: receipt_test_id(&format!("action-{label}")),
            action_kind: action_json["kind"].as_str().unwrap_or("shielded_swap_v1").to_string(),
            action_json: action_json.to_string(),
            batch_id: receipt_test_id(&format!("batch-{label}")),
            batch_json: "{}".to_string(),
            state: final_state.unwrap_or(PrivateActionState::Queued),
            received_height,
            dispatch_count: 1,
            outcome,
        }
    }

    struct ReceiptFixture {
        fixture: PricingReadFixture,
        token: String,
        holder: String,
        /// The final records at or before the token's height, in page order.
        ordered: Vec<String>,
    }

    fn receipt_fixture() -> ReceiptFixture {
        let fixture = pricing_read_fixture(700, |_, _| {});
        let holder = address_from_public_key(&[21u8; 1952]);
        let other = address_from_public_key(&[22u8; 1952]);
        let swap = serde_json::json!({ "kind": "shielded_swap_v1", "swap_json": "{}" });
        let egress = |to: &str| serde_json::json!({ "kind": "asset_orchard_private_egress_v1", "to": to });
        let ingress = |source: &str| {
            serde_json::json!({
                "kind": "asset_orchard_ingress_v2",
                "burn_transaction": { "unsigned": { "source": source } }
            })
        };
        let accepted = Some(PrivateActionState::Accepted);
        let rejected = Some(PrivateActionState::Rejected);
        let records = [
            receipt_test_record("a", accepted, Some(680), 670, "session-a", swap.clone()),
            receipt_test_record("b", accepted, Some(650), 640, "session-a", egress(&holder)),
            receipt_test_record("c", accepted, Some(650), 641, "session-b", ingress(&holder)),
            receipt_test_record("d", accepted, Some(700), 699, "session-b", egress(&other)),
            receipt_test_record("e", rejected, None, 690, "session-a", swap.clone()),
            // Included after the token's height, and still queued: pending.
            receipt_test_record("f", accepted, Some(701), 700, "session-a", swap.clone()),
            receipt_test_record("g", None, None, 700, "session-a", egress(&holder)),
        ];
        let mut store = PrivateActionAdmissionStore::open(&fixture.data_dir).expect("admission store");
        for record in &records {
            store.write(record.clone()).expect("write admission record");
        }
        drop(store);
        let mut final_rows = records[..5]
            .iter()
            .map(|record| {
                let height = record
                    .outcome
                    .as_ref()
                    .and_then(|outcome| outcome.inclusion.as_ref())
                    .map_or(record.received_height, |inclusion| inclusion.height);
                (height, record.intent_id.clone())
            })
            .collect::<Vec<_>>();
        final_rows.sort();
        let token = private_pool_snapshot_for(&fixture.data_dir).snapshot_token;
        ReceiptFixture {
            fixture,
            token,
            holder,
            ordered: final_rows.into_iter().map(|(_, intent)| intent).collect(),
        }
    }

    fn receipts_page(
        fixture: &ReceiptFixture,
        cursor: Option<String>,
        limit: Option<usize>,
        address: Option<String>,
        session_scope: Option<&str>,
    ) -> PrivatePoolReceiptsReport {
        private_pool_receipts(PrivatePoolReceiptsOptions {
            data_dir: fixture.fixture.data_dir.clone(),
            snapshot_token: fixture.token.clone(),
            cursor,
            limit,
            address,
            session_scope: session_scope.map(str::to_string),
        })
        .expect("receipts page")
    }

    fn receipts_intents(page: &PrivatePoolReceiptsReport) -> Vec<String> {
        page.items
            .as_ref()
            .expect("items")
            .iter()
            .map(|row| row.intent_id.clone())
            .collect()
    }

    #[test]
    fn private_pool_receipts_page_final_outcomes_in_order_and_count_pending() {
        let fixture = receipt_fixture();
        let mut joined = Vec::new();
        let mut cursor = None;
        loop {
            let page = receipts_page(&fixture, cursor.clone(), Some(2), None, None);
            assert_eq!(page.status, "ok", "{page:?}");
            assert_eq!((page.total, page.pending), (5, 2));
            assert!(page.item_count <= 2);
            assert_eq!(page.offset as usize, joined.len(), "no gap or overlap");
            // The same page read twice is byte-identical.
            assert_eq!(
                serde_json::to_vec(&page).expect("page json"),
                serde_json::to_vec(&receipts_page(&fixture, cursor.clone(), Some(2), None, None))
                    .expect("page json")
            );
            joined.extend(receipts_intents(&page));
            cursor = page.next_cursor.clone();
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(joined, fixture.ordered, "inclusion height, then intent id");

        let all = receipts_page(&fixture, None, None, None, None);
        let rows = all.items.as_ref().expect("items");
        let rejected = rows.iter().find(|row| row.state == "rejected").expect("rejected row");
        assert_eq!((rejected.inclusion_height, rejected.final_height), (None, 690));
        assert_eq!((rejected.fee_charged, rejected.state_effects.as_str()), (0, "none"));
        let included = rows.iter().find(|row| row.inclusion_height == Some(700)).expect("height 700");
        assert_eq!(included.block_hash, Some(receipt_test_id("block-700")));
        assert_eq!(included.transaction_ids, vec![receipt_test_id("tx-d")]);
        assert_eq!((included.code.as_str(), included.fee_charged), ("accepted", 7));
        assert!(rows.iter().all(|row| row.final_height <= 700));

        // Malformed, past-the-end and foreign cursors.
        let second = receipts_page(&fixture, None, Some(2), None, None)
            .next_cursor
            .expect("cursor");
        for (cursor, code) in [
            ("garbage".to_string(), "cursor_malformed"),
            (second.replace(":2:", ":5:"), "cursor_out_of_range"),
        ] {
            let page = receipts_page(&fixture, Some(cursor), Some(2), None, None);
            assert_eq!(page.refusal.expect("refusal").code, code);
            assert!(page.items.is_none());
        }
        let filtered = receipts_page(&fixture, Some(second.clone()), Some(2), None, Some("session-a"));
        assert_eq!(filtered.refusal.expect("refusal").code, "snapshot_unavailable");

        // An outcome recorded after the cursor was issued changes the set.
        let mut store = PrivateActionAdmissionStore::open(&fixture.fixture.data_dir).expect("store");
        let mut late = store
            .get(&receipt_test_id("intent-g"))
            .cloned()
            .expect("pending record");
        late.state = PrivateActionState::Rejected;
        late.outcome = Some(PrivateActionOutcome {
            accepted: false,
            code: "late".to_string(),
            message: String::new(),
            fee_charged: 0,
            state_effects: "none".to_string(),
            inclusion: None,
        });
        store.write(late).expect("late outcome");
        drop(store);
        let changed = receipts_page(&fixture, Some(second), Some(2), None, None);
        assert_eq!(changed.refusal.expect("refusal").code, "snapshot_unavailable");
        assert_eq!(receipts_page(&fixture, None, None, None, None).total, 6);
        fs::remove_dir_all(&fixture.fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_pool_receipts_filter_by_address_and_session_scope() {
        let fixture = receipt_fixture();
        let by_address = receipts_page(&fixture, None, None, Some(fixture.holder.clone()), None);
        let mut expected = vec![receipt_test_id("intent-b"), receipt_test_id("intent-c")];
        expected.sort_by_key(|intent| fixture.ordered.iter().position(|ordered| ordered == intent));
        assert_eq!(receipts_intents(&by_address), expected);
        assert_eq!((by_address.total, by_address.pending), (2, 1), "the queued egress to the holder is pending");
        let by_scope = receipts_page(&fixture, None, None, None, Some("session-b"));
        let mut expected = vec![receipt_test_id("intent-c"), receipt_test_id("intent-d")];
        expected.sort_by_key(|intent| fixture.ordered.iter().position(|ordered| ordered == intent));
        assert_eq!(receipts_intents(&by_scope), expected);
        assert_eq!(by_scope.pending, 0);
        assert!(private_pool_receipts(PrivatePoolReceiptsOptions {
            data_dir: fixture.fixture.data_dir.clone(),
            snapshot_token: fixture.token.clone(),
            cursor: None,
            limit: None,
            address: Some("PF-NOT-AN-ADDRESS".to_string()),
            session_scope: None,
        })
        .is_err());
        fs::remove_dir_all(&fixture.fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_pool_receipt_and_market_reads_refuse_a_token_from_another_tip() {
        let fixture = receipt_fixture();
        commit_private_pool_tip(&fixture.fixture.data_dir, 701);
        let receipts = receipts_page(&fixture, None, None, None, None);
        assert_eq!(receipts.refusal.expect("refusal").code, "snapshot_unavailable");
        let market = private_pool_market(PrivatePoolMarketOptions {
            data_dir: fixture.fixture.data_dir.clone(),
            snapshot_token: fixture.token.clone(),
            base_asset_id: fixture.fixture.base_asset_id.clone(),
            quote_asset_id: fixture.fixture.quote_asset_id.clone(),
            inclusion_height: None,
        })
        .expect("market");
        assert_eq!(market.status, "refused");
        assert_eq!(market.refusal.expect("refusal").code, "snapshot_unavailable");
        assert!(market.pricing.is_none());
        let malformed = private_pool_market(PrivatePoolMarketOptions {
            data_dir: fixture.fixture.data_dir.clone(),
            snapshot_token: "pps1:bad".to_string(),
            base_asset_id: fixture.fixture.base_asset_id.clone(),
            quote_asset_id: fixture.fixture.quote_asset_id.clone(),
            inclusion_height: None,
        })
        .expect("market");
        assert_eq!(malformed.refusal.expect("refusal").code, "snapshot_token_malformed");
        fs::remove_dir_all(&fixture.fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_pool_market_agrees_with_the_pricing_and_egress_reads_at_the_same_height() {
        let fixture = pricing_read_fixture(700, |ledger, governance| {
            let base = ledger.nav_assets[0].asset_id.clone();
            let quote = ledger
                .asset_definitions
                .iter()
                .find(|asset| asset.code == "PFUSDC")
                .expect("quote")
                .asset_id
                .clone();
            ledger.pftl_uniswap_routes.push(governed_pricing_route(&base, &quote));
            governance.apply(pricing_enforcement_amendment(
                GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_ENFORCEMENT_ACTIVATION_HEIGHT,
                750,
            ));
            let record = egress_policy_record(&base, 701);
            governance.apply(pricing_enforcement_amendment(&record.governance_kind(), 701));
        });
        let token = private_pool_snapshot_for(&fixture.data_dir).snapshot_token;
        let market_at = |inclusion_height: Option<u64>| {
            private_pool_market(PrivatePoolMarketOptions {
                data_dir: fixture.data_dir.clone(),
                snapshot_token: token.clone(),
                base_asset_id: fixture.base_asset_id.clone(),
                quote_asset_id: fixture.quote_asset_id.clone(),
                inclusion_height,
            })
            .expect("market")
        };
        let market = market_at(None);
        assert_eq!(market.schema, PRIVATE_POOL_MARKET_SCHEMA);
        assert_eq!((market.status.as_str(), market.inclusion_height), ("ok", 701));
        let pricing = asset_orchard_pricing_claim(pricing_read_options(&fixture, 701)).expect("pricing");
        assert_eq!(market.pricing.as_ref(), Some(&pricing));
        let egress =
            asset_orchard_private_egress_binding(pricing_read_options(&fixture, 701)).expect("egress");
        assert_eq!(market.egress_bindings.as_ref(), Some(&egress.bindings));
        assert_eq!(market.egress_status.as_deref(), Some(egress.status.as_str()));
        assert!(market.egress_bindings.as_ref().expect("bindings")[0].governed);
        assert_eq!(
            market.governed_route,
            Some(PrivatePoolMarketRoute {
                route_id: "a666-pfusdc-route".to_string(),
                paused: false
            })
        );
        let nav = market.nav.as_ref().expect("nav");
        assert_eq!((nav.nav_epoch, nav.reserve_packet_hash.as_str()), (7, "ab".repeat(48).as_str()));
        assert_eq!(nav.finalized_at_height, PRICING_READ_FINALIZED_AT);
        assert_eq!(nav.age_blocks_at_inclusion, Some(701 - PRICING_READ_FINALIZED_AT));
        let enforcement = market.pricing_enforcement.as_ref().expect("enforcement");
        assert_eq!((enforcement.activation_height, enforcement.active_at_inclusion), (Some(750), false));

        // At a later inclusion height the enforcement applies and the reads still agree.
        let later = market_at(Some(750));
        assert!(later.pricing_enforcement.as_ref().expect("enforcement").active_at_inclusion);
        assert_eq!(
            later.pricing.as_ref(),
            Some(&asset_orchard_pricing_claim(pricing_read_options(&fixture, 750)).expect("pricing"))
        );
        assert_eq!(
            serde_json::to_vec(&market).expect("json"),
            serde_json::to_vec(&market_at(None)).expect("json"),
            "repeat reads are byte-identical"
        );
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
        }

    #[test]
    fn private_pool_receipts_serve_a_retained_older_token_and_the_market_refuses_it() {
        let fixture = receipt_fixture();
        record_private_pool_point(&fixture.fixture.data_dir);
        let before = serde_json::to_vec(&receipts_page(&fixture, None, Some(2), None, None)).expect("json");
        let cursor = receipts_page(&fixture, None, Some(2), None, None).next_cursor;
        commit_private_pool_tip(&fixture.fixture.data_dir, 701);
        record_private_pool_point(&fixture.fixture.data_dir);
        // Receipts final at or before the token's height, as before the block.
        assert_eq!(
            serde_json::to_vec(&receipts_page(&fixture, None, Some(2), None, None)).expect("json"),
            before
        );
        let second = receipts_page(&fixture, cursor, Some(2), None, None);
        assert_eq!((second.status.as_str(), second.offset), ("ok", 2));
        let market = private_pool_market(PrivatePoolMarketOptions {
            data_dir: fixture.fixture.data_dir.clone(),
            snapshot_token: fixture.token.clone(),
            base_asset_id: fixture.fixture.base_asset_id.clone(),
            quote_asset_id: fixture.fixture.quote_asset_id.clone(),
            inclusion_height: None,
        })
        .expect("market");
        assert_eq!(market.refusal.expect("refusal").code, "snapshot_not_current");
        assert!(market.pricing.is_none());
        fs::remove_dir_all(&fixture.fixture.data_dir).expect("cleanup");
    }
