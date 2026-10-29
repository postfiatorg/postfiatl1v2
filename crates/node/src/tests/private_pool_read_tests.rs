    const PRIVATE_POOL_TEST_POOL: &str = "asset-orchard-v1";

    fn private_pool_hex(tag: &str, index: usize, bytes: usize) -> String {
        let seed = hash_hex("postfiat.private_pool_test", format!("{tag}:{index}").as_bytes());
        seed.repeat(bytes * 2 / seed.len() + 1)[..bytes * 2].to_string()
    }

    /// Commits the store's current state under a chain tip at `height`.
    fn commit_private_pool_tip(data_dir: &Path, height: u64) {
        let store = NodeStore::new(data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let mut tip = read_chain_tip_or_reconstruct_for_genesis_read_only(&store, &genesis)
            .expect("tip");
        tip.height = height;
        tip.state_root = current_replicated_state_root(&store, &genesis).expect("state root");
        store.write_chain_tip(&tip).expect("write tip");
    }

    /// Records the committed tip's snapshot point, as the commit hook does.
    fn record_private_pool_point(data_dir: &Path) {
        let store = NodeStore::new(data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let tip = read_chain_tip_or_reconstruct_for_genesis_read_only(&store, &genesis)
            .expect("tip");
        let point = crate::private_pool_snapshot_points::snapshot_point(
            &tip,
            &store.read_ledger().expect("ledger"),
            &store.read_shielded().expect("shielded"),
        )
        .expect("point");
        crate::private_pool_snapshot_points::record_snapshot_point(
            data_dir,
            point,
            PRIVATE_POOL_SNAPSHOT_POINTS_DEFAULT,
        )
        .expect("record point");
    }

    struct PrivatePoolIssuedFixture {
        data_dir: PathBuf,
        asset_id: String,
        holder: String,
        issuer: String,
    }

    /// An issued asset with a holder line of 40 atoms, shielded through a
    /// real Asset-Orchard ingress of `ingress_amount` atoms when nonzero.
    fn private_pool_issued_fixture(ingress_amount: u64) -> PrivatePoolIssuedFixture {
        let data_dir = unique_read_fixture_dir("postfiat-private-pool-read");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let issuer_key = read_transfer_key_file(&data_dir, None).expect("issuer key");
        let holder_key_file = data_dir.join("holder.key.json");
        let holder_key = write_test_master_key(&holder_key_file, [29u8; 32]);
        let ledger = store.read_ledger().expect("ledger");
        let funding = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &issuer_key,
            holder_key.address.clone(),
            ACCOUNT_RESERVE + 1_000,
            1,
        )
        .expect("fund holder");
        let funding_batch = build_transaction_batch(&mempool_batch_domain(&genesis), vec![funding])
            .expect("funding batch")
            .batch;
        let funding_file = data_dir.join("fund.batch.json");
        write_batch_file(&funding_file, &funding_batch).expect("write funding batch");
        assert!(apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_file,
            certificate_file: None,
        })
        .expect("apply funding")[0]
            .accepted);

        let ledger = store.read_ledger().expect("funded ledger");
        let create = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &issuer_key.address,
            &issuer_key.public_key_hex,
            &issuer_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer_key.address.clone(),
                code: "PFUSDC".to_string(),
                version: 1,
                precision: 6,
                display_name: "pfUSDC".to_string(),
                max_supply: Some(1_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        let mut dry_run = ledger.clone();
        assert!(execute_asset_transaction(&genesis, &mut dry_run, &create, 1).accepted);
        let asset_id = dry_run.asset_definitions[0].asset_id.clone();
        let trust_set = signed_asset_transaction_for_test(
            &genesis,
            &dry_run,
            &holder_key.address,
            &holder_key.public_key_hex,
            &holder_key.private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder_key.address.clone(),
                issuer: issuer_key.address.clone(),
                asset_id: asset_id.clone(),
                limit: 100,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut dry_run, &trust_set, 1).accepted);
        let issue = signed_asset_transaction_for_test(
            &genesis,
            &dry_run,
            &issuer_key.address,
            &issuer_key.public_key_hex,
            &issuer_key.private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer_key.address.clone(),
                to: holder_key.address.clone(),
                issuer: issuer_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 40,
            }),
        );
        let setup_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            vec![create, trust_set, issue],
        )
        .expect("asset setup batch")
        .batch;
        let setup_file = data_dir.join("setup.batch.json");
        write_batch_file(&setup_file, &setup_batch).expect("write setup batch");
        assert!(apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: setup_file,
            certificate_file: None,
        })
        .expect("apply setup")
        .iter()
        .all(|receipt| receipt.accepted));

        if ingress_amount > 0 {
            let ingress_file = data_dir.join("ingress.json");
            create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
                data_dir: data_dir.clone(),
                key_file: holder_key_file,
                asset_id: asset_id.clone(),
                amount: ingress_amount,
                fee: 0,
                note_seed_hex: bytes_to_hex(&[31u8; 32]),
                encrypted_output_hex: None,
                ingress_file: ingress_file.clone(),
                note_file: data_dir.join("ingress-note.json"),
                overwrite: false,
            })
            .expect("create ingress");
            let batch_file = data_dir.join("ingress.batch.json");
            create_asset_orchard_ingress_batch(AssetOrchardIngressBatchOptions {
                data_dir: data_dir.clone(),
                ingress_file,
                batch_file: batch_file.clone(),
            })
            .expect("ingress batch");
            let receipts = apply_shielded_batch(ApplyBatchOptions {
                data_dir: data_dir.clone(),
                batch_file,
                certificate_file: None,
            })
            .expect("apply ingress");
            assert!(receipts[0].accepted, "{receipts:?}");
            // Republish the chain tip from the authenticated block history,
            // as a restarting node does, so reads see one committed state.
            read_chain_tip_or_reconstruct_for_genesis(&store, &genesis).expect("republish tip");
        }
        PrivatePoolIssuedFixture {
            data_dir,
            asset_id,
            holder: holder_key.address,
            issuer: issuer_key.address,
        }
    }

    fn private_pool_snapshot_for(data_dir: &Path) -> PrivatePoolSnapshotReport {
        private_pool_snapshot(PrivatePoolSnapshotOptions {
            data_dir: data_dir.to_path_buf(),
            pool_id: PRIVATE_POOL_TEST_POOL.to_string(),
            address: None,
            base_asset_id: None,
            quote_asset_id: None,
            inclusion_height: None,
        })
        .expect("snapshot")
    }

    fn private_pool_page_for(
        data_dir: &Path,
        token: &str,
        section: &str,
        cursor: Option<String>,
        limit: Option<usize>,
    ) -> PrivatePoolPageReport {
        private_pool_page(PrivatePoolPageOptions {
            data_dir: data_dir.to_path_buf(),
            snapshot_token: token.to_string(),
            section: section.to_string(),
            cursor,
            limit,
        })
        .expect("page")
    }

    fn private_pool_refusal_code(report: &PrivatePoolPageReport) -> String {
        assert_eq!(report.status, "refused", "{report:?}");
        assert!(report.items.is_none());
        report.refusal.as_ref().expect("refusal").code.clone()
    }

    /// Every page of `section`, following cursors from the first page.
    fn private_pool_all_pages(
        data_dir: &Path,
        token: &str,
        section: &str,
        limit: Option<usize>,
    ) -> Vec<PrivatePoolPageReport> {
        let mut pages = Vec::new();
        let mut cursor = None;
        loop {
            let page = private_pool_page_for(data_dir, token, section, cursor, limit);
            assert_eq!(page.status, "ok", "{page:?}");
            cursor = page.next_cursor.clone();
            pages.push(page);
            if cursor.is_none() {
                return pages;
            }
        }
    }

    /// A synthetic retained pool of `commitments` outputs (the first
    /// `large_ciphertexts` with 40 KiB ciphertexts),
    /// 700 nullifiers, four retained prefix roots and a spent-anchor list
    /// that must not be served.
    fn private_pool_synthetic_fixture(
        commitments: usize,
        large_ciphertexts: usize,
    ) -> (PathBuf, OrchardPoolState) {
        let data_dir = unique_read_fixture_dir("postfiat-private-pool-pages");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-private-pool-pages".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");
        let store = NodeStore::new(&data_dir);
        let mut pool = OrchardPoolState::empty(PRIVATE_POOL_TEST_POOL);
        pool.output_commitments = (0..commitments)
            .map(|index| private_pool_hex("commitment", index, 32))
            .collect();
        pool.asset_orchard_outputs = pool
            .output_commitments
            .iter()
            .enumerate()
            .map(|(index, commitment)| AssetOrchardEncryptedOutputRecord {
                output_commitment: commitment.clone(),
                encrypted_output: private_pool_hex(
                    "ciphertext",
                    index,
                    if index < large_ciphertexts {
                        40 * 1024
                    } else {
                        128
                    },
                ),
            })
            .collect();
        pool.nullifiers = (0..700)
            .map(|index| private_pool_hex("nullifier", index, 32))
            .collect();
        pool.root_history = [0u64, 1, 600, commitments as u64]
            .into_iter()
            .map(|output_count| OrchardRootRecord {
                root: private_pool_hex("root", output_count as usize, 32),
                output_count,
            })
            .collect();
        pool.accepted_anchors = vec![private_pool_hex("spent-anchor", 0, 32)];
        let mut shielded = store.read_shielded().expect("shielded");
        shielded.orchard = Some(pool.clone());
        store.write_shielded(&shielded).expect("write shielded");
        commit_private_pool_tip(&data_dir, 40);
        (data_dir, pool)
    }

    #[test]
    fn private_pool_snapshot_is_empty_and_complete_before_the_first_ingress() {
        let fixture = private_pool_issued_fixture(0);
        let snapshot = private_pool_snapshot(PrivatePoolSnapshotOptions {
            data_dir: fixture.data_dir.clone(),
            pool_id: PRIVATE_POOL_TEST_POOL.to_string(),
            address: Some(fixture.holder.clone()),
            base_asset_id: None,
            quote_asset_id: None,
            inclusion_height: None,
        })
        .expect("snapshot");
        assert_eq!(snapshot.schema, PRIVATE_POOL_SNAPSHOT_SCHEMA);
        assert_eq!(snapshot.address.as_deref(), Some(fixture.holder.as_str()));
        assert!(!snapshot.pool_present);
        assert_eq!(snapshot.counts.commitments, 0);
        assert_eq!(snapshot.counts.accepted_anchors, 0);
        assert!(snapshot.completeness.empty_pool_before_ingress);
        assert!(snapshot.completeness.complete);
        assert_eq!(snapshot.pricing_status, "unavailable");
        assert_eq!(snapshot.inclusion_height, snapshot.finalized_height + 1);
        for section in PRIVATE_POOL_SECTIONS {
            let page = private_pool_page_for(&fixture.data_dir, &snapshot.snapshot_token, section, None, None);
            assert_eq!(page.status, "ok", "{page:?}");
            assert_eq!((page.total, page.item_count, page.next_cursor.clone()), (0, 0, None));
        }
        let lines = private_account_lines(PrivateAccountLinesOptions {
            data_dir: fixture.data_dir.clone(),
            snapshot_token: snapshot.snapshot_token.clone(),
            address: fixture.holder.clone(),
        })
        .expect("lines");
        assert_eq!(lines.status, "ok");
        assert_eq!(lines.line_count, 1);
        assert_eq!(lines.lines[0].balance, 40);
        assert_eq!(lines.positive_asset_count, 1);
        assert!(lines.account_assets_consistent);
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_pool_reads_serve_a_full_shield_and_keep_the_zero_balance_line() {
        let fixture = private_pool_issued_fixture(40);
        let store = NodeStore::new(&fixture.data_dir);
        let tip = store.read_chain_tip().expect("tip");
        let snapshot = private_pool_snapshot_for(&fixture.data_dir);
        assert_eq!(
            (snapshot.finalized_height, snapshot.finalized_block_hash.as_str(), snapshot.finalized_state_root.as_str()),
            (tip.height, tip.block_hash.as_str(), tip.state_root.as_str())
        );
        assert!(snapshot.pool_present);
        assert_eq!(snapshot.counts.commitments, 1);
        assert_eq!(snapshot.counts.outputs, 1);
        assert_eq!(snapshot.counts.assets, 1);
        assert!(!snapshot.completeness.empty_pool_before_ingress);
        assert!(snapshot.completeness.complete, "{:?}", snapshot.completeness);

        let token = snapshot.snapshot_token.clone();
        let pool = store.read_shielded().expect("shielded").orchard.expect("pool");
        let outputs = private_pool_page_for(&fixture.data_dir, &token, "outputs", None, None);
        let Some(PrivatePoolPageItems::Outputs(rows)) = outputs.items else {
            panic!("outputs page: {outputs:?}");
        };
        assert_eq!(rows[0].kind, "asset_orchard");
        assert_eq!(rows[0].commitment, pool.output_commitments[0]);
        assert_eq!(
            rows[0].ciphertext.as_deref(),
            Some(pool.asset_orchard_outputs[0].encrypted_output.as_str())
        );
        let anchors = private_pool_page_for(&fixture.data_dir, &token, "accepted_anchors", None, None);
        let Some(PrivatePoolPageItems::Anchors(anchor_rows)) = anchors.items else {
            panic!("anchors page: {anchors:?}");
        };
        assert_eq!(
            anchor_rows.iter().map(|row| (row.root.clone(), row.output_count)).collect::<Vec<_>>(),
            pool.root_history
                .iter()
                .map(|record| (record.root.clone(), record.output_count))
                .collect::<Vec<_>>()
        );
        assert_eq!(anchor_rows.last().expect("current root").output_count, 1);
        let assets = private_pool_page_for(&fixture.data_dir, &token, "assets", None, None);
        let Some(PrivatePoolPageItems::Assets(asset_rows)) = assets.items else {
            panic!("assets page: {assets:?}");
        };
        assert_eq!(asset_rows[0].asset_id, fixture.asset_id);
        assert_eq!(asset_rows[0].live_total, 40);
        assert!(asset_rows[0].asset_info_bound);
        assert_eq!(asset_rows[0].issuer.as_deref(), Some(fixture.issuer.as_str()));
        assert_eq!(asset_rows[0].code.as_deref(), Some("PFUSDC"));
        assert_eq!(asset_rows[0].precision, Some(6));
        assert_eq!(asset_rows[0].requires_authorization, Some(false));
        assert_eq!(asset_rows[0].proof_profile_id, None, "not NAV-tracked");

        let lines = private_account_lines(PrivateAccountLinesOptions {
            data_dir: fixture.data_dir.clone(),
            snapshot_token: token.clone(),
            address: fixture.holder.clone(),
        })
        .expect("lines");
        assert_eq!(lines.status, "ok", "{lines:?}");
        assert_eq!(lines.line_count, 1, "the zero-balance line stays after a full shield");
        let line = &lines.lines[0];
        assert_eq!((line.balance, line.issuer.as_str(), line.code.as_str(), line.precision), (0, fixture.issuer.as_str(), "PFUSDC", 6));
        assert_eq!(line.line_status, "ok");
        assert_eq!(lines.positive_asset_count, 0);
        assert!(lines.account_assets_consistent);
        assert!(lines.all_lines_usable);

        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_pool_pages_are_deterministic_bounded_and_concatenate_to_the_ordered_sets() {
        let (data_dir, pool) = private_pool_synthetic_fixture(1_100, 0);
        let snapshot = private_pool_snapshot_for(&data_dir);
        assert_eq!(snapshot.counts.commitments, 1_100);
        assert_eq!(snapshot.counts.nullifiers, 700);
        assert_eq!(snapshot.counts.accepted_anchors, 4);
        assert!(snapshot.completeness.complete, "{:?}", snapshot.completeness);
        let token = snapshot.snapshot_token.clone();

        for (section, expected) in [
            ("commitments", pool.output_commitments.clone()),
            ("nullifiers", pool.nullifiers.clone()),
        ] {
            for limit in [None, Some(500), Some(10_000)] {
                let pages = private_pool_all_pages(&data_dir, &token, section, limit);
                let mut joined = Vec::new();
                for page in &pages {
                    assert!(page.item_count as usize <= PRIVATE_POOL_PAGE_MAX_ITEMS);
                    assert_eq!(page.limit as usize, limit.unwrap_or(PRIVATE_POOL_PAGE_MAX_ITEMS).min(PRIVATE_POOL_PAGE_MAX_ITEMS));
                    assert_eq!(page.offset as usize, joined.len(), "no gap or overlap");
                    let Some(PrivatePoolPageItems::Strings(items)) = &page.items else {
                        panic!("{section} page: {page:?}");
                    };
                    joined.extend(items.clone());
                }
                assert_eq!(joined, expected, "{section} pages concatenate to the ordered set");
            }
        }
        // The anchors are the retained prefix roots, not the spent-anchor list.
        let anchors = private_pool_page_for(&data_dir, &token, "accepted_anchors", None, None);
        let Some(PrivatePoolPageItems::Anchors(rows)) = &anchors.items else {
            panic!("anchors page: {anchors:?}");
        };
        assert_eq!(rows.iter().map(|row| row.output_count).collect::<Vec<_>>(), [0, 1, 600, 1_100]);
        assert!(rows.iter().all(|row| row.root != pool.accepted_anchors[0]));

        // Outputs pages stop at 64 rows.
        let output_pages = private_pool_all_pages(&data_dir, &token, "outputs", None);
        let mut positions = Vec::new();
        for page in &output_pages {
            let Some(PrivatePoolPageItems::Outputs(rows)) = &page.items else {
                panic!("outputs page: {page:?}");
            };
            assert!(rows.len() <= PRIVATE_POOL_PAGE_MAX_OUTPUTS);
            let bytes: usize = rows
                .iter()
                .map(|row| row.ciphertext.as_ref().expect("retained ciphertext").len() / 2)
                .sum();
            assert!(bytes <= PRIVATE_POOL_PAGE_MAX_CIPHERTEXT_BYTES, "{bytes}");
            assert!(serde_json::to_vec(page).expect("page json").len() <= 4 * 1024 * 1024);
            positions.extend(rows.iter().map(|row| row.position));
            for row in rows {
                assert_eq!(row.commitment, pool.output_commitments[row.position as usize]);
            }
        }
        assert_eq!(output_pages[0].item_count as usize, PRIVATE_POOL_PAGE_MAX_OUTPUTS);
        assert_eq!(positions, (0..1_100).collect::<Vec<u64>>());

        // Two reads of the same page are byte-identical.
        let first = private_pool_page_for(&data_dir, &token, "commitments", None, Some(100));
        let cursor = first.next_cursor.clone();
        let again = |cursor: Option<String>| {
            serde_json::to_vec(&private_pool_page_for(&data_dir, &token, "commitments", cursor, Some(100)))
                .expect("page json")
        };
        assert_eq!(serde_json::to_vec(&first).expect("first json"), again(None));
        assert_eq!(again(cursor.clone()), again(cursor.clone()));

        // Malformed, foreign-section, past-the-end and unknown-section requests.
        let cursor = cursor.expect("second page cursor");
        assert_eq!(
            private_pool_refusal_code(&private_pool_page_for(&data_dir, &token, "commitments", Some("garbage".to_string()), None)),
            "cursor_malformed"
        );
        assert_eq!(
            private_pool_refusal_code(&private_pool_page_for(&data_dir, &token, "nullifiers", Some(cursor.clone()), None)),
            "cursor_malformed"
        );
        let past_end = cursor.replace(":100:", ":1100:");
        assert_eq!(
            private_pool_refusal_code(&private_pool_page_for(&data_dir, &token, "commitments", Some(past_end), None)),
            "cursor_out_of_range"
        );
        assert_eq!(
            private_pool_refusal_code(&private_pool_page_for(&data_dir, &token, "notes", None, None)),
            "section_unknown"
        );
        assert_eq!(
            private_pool_refusal_code(&private_pool_page_for(&data_dir, "pps1:bad", "commitments", None, None)),
            "snapshot_token_malformed"
        );
        fs::remove_dir_all(&data_dir).expect("cleanup");
    }

    #[test]
    fn private_pool_reads_refuse_a_token_or_cursor_from_another_tip() {
        let (data_dir, _) = private_pool_synthetic_fixture(1_100, 0);
        let old = private_pool_snapshot_for(&data_dir);
        let old_cursor = private_pool_page_for(&data_dir, &old.snapshot_token, "commitments", None, None)
            .next_cursor
            .expect("cursor");

        // Uncommitted state is never served as a snapshot.
        let store = NodeStore::new(&data_dir);
        let mut shielded = store.read_shielded().expect("shielded");
        shielded
            .orchard
            .as_mut()
            .expect("pool")
            .nullifiers
            .push(private_pool_hex("nullifier", 9_999, 32));
        store.write_shielded(&shielded).expect("write shielded");
        assert_eq!(
            private_pool_snapshot(PrivatePoolSnapshotOptions {
                data_dir: data_dir.clone(),
                pool_id: PRIVATE_POOL_TEST_POOL.to_string(),
                address: None,
                base_asset_id: None,
                quote_asset_id: None,
                inclusion_height: None,
            })
            .expect_err("torn state")
            .kind(),
            io::ErrorKind::Interrupted
        );

        commit_private_pool_tip(&data_dir, old.finalized_height + 1);
        let new = private_pool_snapshot_for(&data_dir);
        assert_ne!(new.snapshot_token, old.snapshot_token);
        assert_eq!(new.counts.nullifiers, 701);
        for section in ["commitments", "nullifiers"] {
            assert_eq!(
                private_pool_refusal_code(&private_pool_page_for(&data_dir, &old.snapshot_token, section, None, None)),
                "snapshot_unavailable"
            );
        }
        assert_eq!(
            private_pool_refusal_code(&private_pool_page_for(&data_dir, &new.snapshot_token, "commitments", Some(old_cursor), None)),
            "cursor_malformed"
        );
        let holder = address_from_public_key(&[7u8; 1952]);
        let lines = private_account_lines(PrivateAccountLinesOptions {
            data_dir: data_dir.clone(),
            snapshot_token: old.snapshot_token.clone(),
            address: holder,
        })
        .expect("lines");
        assert_eq!(lines.status, "refused");
        assert_eq!(lines.refusal.expect("refusal").code, "snapshot_unavailable");
        fs::remove_dir_all(&data_dir).expect("cleanup");
    }

    #[test]
    fn private_pool_output_pages_stop_at_the_ciphertext_byte_maximum() {
        let (data_dir, pool) = private_pool_synthetic_fixture(30, 30);
        let token = private_pool_snapshot_for(&data_dir).snapshot_token;
        let pages = private_pool_all_pages(&data_dir, &token, "outputs", None);
        let mut positions = Vec::new();
        for page in &pages {
            let Some(PrivatePoolPageItems::Outputs(rows)) = &page.items else {
                panic!("outputs page: {page:?}");
            };
            let bytes: usize = rows
                .iter()
                .map(|row| row.ciphertext.as_ref().expect("retained ciphertext").len() / 2)
                .sum();
            assert!(bytes <= PRIVATE_POOL_PAGE_MAX_CIPHERTEXT_BYTES, "{bytes}");
            assert!(serde_json::to_vec(page).expect("page json").len() <= 4 * 1024 * 1024);
            positions.extend(rows.iter().map(|row| row.position));
        }
        // 40 KiB ciphertexts: 25 fit in 1 MiB, the remaining 5 follow.
        assert_eq!(
            pages.iter().map(|page| page.item_count).collect::<Vec<_>>(),
            [25, 5]
        );
        assert_eq!(positions, (0..pool.output_commitments.len() as u64).collect::<Vec<_>>());
        fs::remove_dir_all(&data_dir).expect("cleanup");
    }

    #[test]
    fn private_account_lines_mark_frozen_unauthorized_and_issuer_mismatched_lines() {
        let data_dir = unique_read_fixture_dir("postfiat-private-account-lines");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-private-account-lines".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let issuer = read_transfer_key_file(&data_dir, None).expect("issuer").address;
        let holder = address_from_public_key(&[9u8; 1952]);
        let mut ledger = store.read_ledger().expect("ledger");
        let mut line_for = |code: &str, requires_authorization: bool, balance: u64| {
            let mut asset = AssetDefinition::new(&genesis.chain_id, issuer.clone(), code.to_string(), 1, 6)
                .expect("asset");
            asset.requires_authorization = requires_authorization;
            let mut line = TrustLine::new(holder.clone(), issuer.clone(), asset.asset_id.clone(), 100, 0)
                .expect("line");
            line.balance = balance;
            ledger.asset_definitions.push(asset.clone());
            ledger.trustlines.push(line);
            asset.asset_id
        };
        let ok = line_for("OKAY", false, 0);
        let frozen = line_for("FROZ", false, 5);
        let unauthorized = line_for("AUTH", true, 0);
        let mismatched = line_for("MISS", false, 0);
        for line in ledger.trustlines.iter_mut() {
            if line.asset_id == frozen {
                line.frozen = true;
            }
            if line.asset_id == mismatched {
                line.issuer = holder.clone();
            }
        }
        store.write_ledger(&ledger).expect("write ledger");
        commit_private_pool_tip(&data_dir, 12);
        let token = private_pool_snapshot_for(&data_dir).snapshot_token;
        let report = private_account_lines(PrivateAccountLinesOptions {
            data_dir: data_dir.clone(),
            snapshot_token: token,
            address: holder.clone(),
        })
        .expect("lines");
        assert_eq!(report.status, "ok", "{report:?}");
        let status_of = |asset_id: &str| {
            report
                .lines
                .iter()
                .find(|line| line.asset_id == asset_id)
                .map(|line| line.line_status.clone())
                .expect("line")
        };
        assert_eq!(report.line_count, 4, "zero-balance lines are kept");
        assert_eq!(status_of(&ok), "ok");
        assert_eq!(status_of(&frozen), "frozen");
        assert_eq!(status_of(&unauthorized), "unauthorized");
        assert_eq!(status_of(&mismatched), "issuer_mismatch");
        assert!(!report.all_lines_usable);
        assert_eq!(report.positive_asset_count, 1);
        assert!(report.account_assets_consistent);
        fs::remove_dir_all(&data_dir).expect("cleanup");
    }

    #[test]
    fn private_pool_snapshot_flags_missing_history_and_missing_ciphertexts() {
        let data_dir = unique_read_fixture_dir("postfiat-private-pool-incomplete");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-private-pool-incomplete".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");
        let store = NodeStore::new(&data_dir);
        let write_pool = |pool: OrchardPoolState, height: u64| {
            let mut shielded = store.read_shielded().expect("shielded");
            shielded.orchard = Some(pool);
            store.write_shielded(&shielded).expect("write shielded");
            commit_private_pool_tip(&data_dir, height);
            private_pool_snapshot_for(&data_dir)
        };

        // A spend recorded but no retained commitment: history is missing.
        let mut lost = OrchardPoolState::empty(PRIVATE_POOL_TEST_POOL);
        lost.nullifiers.push(private_pool_hex("nullifier", 0, 32));
        let snapshot = write_pool(lost, 3);
        assert!(!snapshot.completeness.empty_pool_before_ingress);
        assert!(!snapshot.completeness.history_complete);
        assert!(!snapshot.completeness.complete);

        // A commitment whose ciphertext is not retained is served as missing.
        let mut unscannable = OrchardPoolState::empty(PRIVATE_POOL_TEST_POOL);
        unscannable.output_commitments = vec![private_pool_hex("commitment", 0, 32)];
        unscannable.root_history = vec![OrchardRootRecord {
            root: private_pool_hex("root", 1, 32),
            output_count: 1,
        }];
        let snapshot = write_pool(unscannable, 4);
        assert!(snapshot.completeness.history_complete);
        assert!(!snapshot.completeness.outputs_complete);
        assert!(!snapshot.completeness.complete);
        let page = private_pool_page_for(&data_dir, &snapshot.snapshot_token, "outputs", None, None);
        let Some(PrivatePoolPageItems::Outputs(rows)) = page.items else {
            panic!("outputs page: {page:?}");
        };
        assert_eq!((rows[0].kind.as_str(), rows[0].ciphertext.as_deref()), ("missing", None));
        fs::remove_dir_all(&data_dir).expect("cleanup");
        }

    /// Every page of every section under `token`, serialized.
    fn private_pool_page_bytes(data_dir: &Path, token: &str) -> Vec<Vec<u8>> {
        PRIVATE_POOL_SECTIONS
            .iter()
            .flat_map(|section| private_pool_all_pages(data_dir, token, section, Some(16)))
            .map(|page| serde_json::to_vec(&page).expect("page json"))
            .collect()
    }

    #[test]
    fn private_pool_tokens_outlive_new_blocks_until_the_ring_expires() {
        // A small retained pool holding an issued asset's definition.
        let fixture = private_pool_issued_fixture(0);
        // The durable-commit hook recorded the applied blocks' points.
        let ring: serde_json::Value = serde_json::from_slice(
            &fs::read(fixture.data_dir.join(PRIVATE_POOL_SNAPSHOT_POINTS_FILE)).expect("ring"),
        )
        .expect("ring json");
        assert_eq!(
            ring["points"].as_array().expect("points").last().expect("point")["height"],
            private_pool_snapshot_for(&fixture.data_dir).finalized_height
        );
        let definition = NodeStore::new(&fixture.data_dir)
            .read_ledger()
            .expect("ledger")
            .asset_definitions[0]
            .clone();
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
        let data_dir = unique_read_fixture_dir("postfiat-private-pool-ring");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-private-pool-ring".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");
        let store = NodeStore::new(&data_dir);
        let mut ledger = store.read_ledger().expect("ledger");
        ledger.asset_definitions.push(definition.clone());
        store.write_ledger(&ledger).expect("write ledger");
        let mut pool = OrchardPoolState::empty(PRIVATE_POOL_TEST_POOL);
        pool.output_commitments = (0..40)
            .map(|index| private_pool_hex("commitment", index, 32))
            .collect();
        pool.asset_orchard_outputs = pool
            .output_commitments
            .iter()
            .enumerate()
            .map(|(index, commitment)| AssetOrchardEncryptedOutputRecord {
                output_commitment: commitment.clone(),
                encrypted_output: private_pool_hex("ciphertext", index, 128),
            })
            .collect();
        pool.nullifiers = (0..12)
            .map(|index| private_pool_hex("nullifier", index, 32))
            .collect();
        pool.root_history = [1u64, 20, 40]
            .into_iter()
            .map(|output_count| OrchardRootRecord {
                root: private_pool_hex("root", output_count as usize, 32),
                output_count,
            })
            .collect();
        pool.asset_orchard_balances = vec![AssetOrchardAssetBalance {
            asset_id: definition.asset_id.clone(),
            ingress_total: 10,
            egress_total: 0,
            live_total: 10,
        }];
        let mut shielded = store.read_shielded().expect("shielded");
        shielded.orchard = Some(pool);
        store.write_shielded(&shielded).expect("write shielded");
        commit_private_pool_tip(&data_dir, 40);
        record_private_pool_point(&data_dir);
        let old = private_pool_snapshot_for(&data_dir);
        let before = private_pool_page_bytes(&data_dir, &old.snapshot_token);
        let first = private_pool_page_for(&data_dir, &old.snapshot_token, "commitments", None, Some(16));
        let second_before = private_pool_page_for(
            &data_dir,
            &old.snapshot_token,
            "commitments",
            first.next_cursor.clone(),
            Some(16),
        );

        // Every later block appends to each list and moves the asset balance.
        let k = PRIVATE_POOL_SNAPSHOT_POINTS_DEFAULT as u64;
        let mut tokens = Vec::new();
        for height in 41..=40 + k {
            let mut shielded = store.read_shielded().expect("shielded");
            let pool = shielded.orchard.as_mut().expect("pool");
            let commitment = private_pool_hex("late-commitment", height as usize, 32);
            pool.output_commitments.push(commitment.clone());
            pool.asset_orchard_outputs.push(AssetOrchardEncryptedOutputRecord {
                output_commitment: commitment,
                encrypted_output: private_pool_hex("late-ciphertext", height as usize, 128),
            });
            pool.nullifiers.push(private_pool_hex("late-nullifier", height as usize, 32));
            pool.root_history.push(OrchardRootRecord {
                root: private_pool_hex("late-root", height as usize, 32),
                output_count: pool.output_commitments.len() as u64,
            });
            pool.asset_orchard_balances[0].live_total += 1;
            store.write_shielded(&shielded).expect("write shielded");
            commit_private_pool_tip(&data_dir, height);
            record_private_pool_point(&data_dir);
            tokens.push(private_pool_snapshot_for(&data_dir).snapshot_token);
            if height < 40 + k {
                // Through K-1 later blocks every page is byte-identical and
                // nothing added after the token's height appears.
                assert_eq!(private_pool_page_bytes(&data_dir, &old.snapshot_token), before, "height {height}");
            }
        }
        let tip = private_pool_snapshot_for(&data_dir);
        assert_eq!(tip.counts.commitments, 40 + k);

        // Expired at K; the next-oldest token is still served.
        for section in PRIVATE_POOL_SECTIONS {
            assert_eq!(
                private_pool_refusal_code(&private_pool_page_for(&data_dir, &old.snapshot_token, section, None, None)),
                "snapshot_expired"
            );
            assert_eq!(
                private_pool_refusal_code(&private_pool_page_for(&data_dir, &old.snapshot_token, section, first.next_cursor.clone(), None)),
                "snapshot_expired"
            );
        }
        let oldest = &tokens[0];
        let commitments = private_pool_all_pages(&data_dir, oldest, "commitments", None)
            .into_iter()
            .flat_map(|page| match page.items {
                Some(PrivatePoolPageItems::Strings(items)) => items,
                other => panic!("{other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(commitments.len(), 41);
        assert!(!commitments.contains(&private_pool_hex("late-commitment", 42, 32)));
        let outputs = private_pool_page_for(&data_dir, oldest, "outputs", Some(encode_cursor(oldest, "outputs", 39)), None);
        let Some(PrivatePoolPageItems::Outputs(rows)) = &outputs.items else {
            panic!("outputs page: {outputs:?}");
        };
        assert_eq!((outputs.total, rows.len()), (41, 2));
        assert_eq!(rows[1].commitment, private_pool_hex("late-commitment", 41, 32));
        let anchors = private_pool_page_for(&data_dir, oldest, "accepted_anchors", None, None);
        assert_eq!(anchors.total, 4);
        let assets = private_pool_page_for(&data_dir, oldest, "assets", None, None);
        let Some(PrivatePoolPageItems::Assets(rows)) = &assets.items else {
            panic!("assets page: {assets:?}");
        };
        assert_eq!(rows[0].live_total, 11, "the asset row as of the token's height");
        let tip_assets = private_pool_page_for(&data_dir, &tip.snapshot_token, "assets", None, None);
        let Some(PrivatePoolPageItems::Assets(rows)) = &tip_assets.items else {
            panic!("assets page: {tip_assets:?}");
        };
        assert_eq!(rows[0].live_total, 10 + k);

        // A token whose hash or root was never final at its height, a height
        // above the tip, and a cursor from another token are refused as before.
        let mut parts = oldest.split(':').collect::<Vec<_>>();
        parts[4] = "ab";
        let wrong_root = parts.join(":");
        parts[2] = "999";
        let above = parts.join(":");
        for token in [wrong_root.as_str(), above.as_str()] {
            assert_eq!(
                private_pool_refusal_code(&private_pool_page_for(&data_dir, token, "commitments", None, None)),
                "snapshot_unavailable",
                "{token}"
            );
        }
        let tip_cursor = private_pool_page_for(&data_dir, &tip.snapshot_token, "commitments", None, Some(16)).next_cursor;
        assert_eq!(
            private_pool_refusal_code(&private_pool_page_for(&data_dir, oldest, "commitments", tip_cursor, Some(16))),
            "cursor_malformed"
        );
        // An older token's own cursor keeps working.
        let oldest_first = private_pool_page_for(&data_dir, oldest, "commitments", None, Some(16));
        let oldest_second = private_pool_page_for(&data_dir, oldest, "commitments", oldest_first.next_cursor, Some(16));
        assert_eq!(oldest_second.status, "ok");
        assert_eq!(oldest_second.offset, 16);
        assert_eq!(second_before.items, oldest_second.items, "append-only prefix pages agree");

        // Account lines need balances at the token's height, which are not kept.
        let lines = private_account_lines(PrivateAccountLinesOptions {
            data_dir: data_dir.clone(),
            snapshot_token: oldest.clone(),
            address: address_from_public_key(&[7u8; 1952]),
        })
        .expect("lines");
        assert_eq!(lines.refusal.expect("refusal").code, "snapshot_not_current");
        fs::remove_dir_all(&data_dir).expect("cleanup");
    }
