    struct ReadinessTestVerifiers {
        swap: Result<PrivateVerifierIdentity, String>,
        private_egress: Result<PrivateVerifierIdentity, String>,
    }

    impl ReadinessTestVerifiers {
        fn pinned() -> Self {
            Self {
                swap: Ok(pinned_swap_verifier_identity()),
                private_egress: Ok(pinned_private_egress_verifier_identity()),
            }
        }
    }

    impl super::private_readiness_read::PrivateVerifierProbe for ReadinessTestVerifiers {
        fn swap(&self) -> Result<PrivateVerifierIdentity, String> {
            self.swap.clone()
        }

        fn private_egress(&self) -> Result<PrivateVerifierIdentity, String> {
            self.private_egress.clone()
        }
    }

    /// The pricing-read chain at height 700 with the governed A666/pfUSDC
    /// route, pricing enforcement active, and atomic shielded batches
    /// governed at `atomic_height` (absent when `None`).
    fn readiness_fixture(
        atomic_height: Option<u32>,
        with_route: bool,
        route_paused: bool,
    ) -> PricingReadFixture {
        readiness_fixture_with_routes(atomic_height, move |ledger, base, quote| {
            if with_route {
                let mut route = governed_pricing_route(base, quote);
                route.paused = route_paused;
                ledger.pftl_uniswap_routes.push(route);
            }
        })
    }

    /// As `readiness_fixture`, with `routes` given the ledger and the
    /// fixture's NAV base and pfUSDC quote asset ids.
    fn readiness_fixture_with_routes(
        atomic_height: Option<u32>,
        routes: impl FnOnce(&mut LedgerState, &str, &str),
    ) -> PricingReadFixture {
        pricing_read_fixture(700, move |ledger, governance| {
            let base = ledger.nav_assets[0].asset_id.clone();
            let quote = ledger
                .asset_definitions
                .iter()
                .find(|asset| asset.code == "PFUSDC")
                .expect("quote")
                .asset_id
                .clone();
            routes(ledger, &base, &quote);
            governance.apply(pricing_enforcement_amendment(
                GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_ENFORCEMENT_ACTIVATION_HEIGHT,
                650,
            ));
            governance.apply(pricing_enforcement_amendment(
                GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_MAX_NAV_AGE_BLOCKS,
                160,
            ));
            if let Some(height) = atomic_height {
                governance.apply(pricing_enforcement_amendment(
                    GOVERNANCE_KIND_SHIELDED_ATOMIC_BATCH_ACTIVATION_HEIGHT,
                    height,
                ));
            }
        })
    }

    /// The running service's custody with the test-only hardened signer and
    /// an owner-only key file: with an active transactional store, the one
    /// combination that satisfies custody.
    fn hardened_test_service(data_dir: &Path) -> PrivateReadinessService {
        PrivateReadinessService {
            key_files: vec![owner_only_key_file(data_dir, "hardened-test-key.json")],
            signer: PrivateValidatorSigner::TestHardened,
            signer_socket: None,
        }
    }

    /// A single-validator chain whose transactional store is active: genesis
    /// schedules it at height 1 and one empty block is committed through it.
    fn active_transactional_store(label: &str) -> PathBuf {
        let data_dir = unique_read_fixture_dir(label);
        let mut genesis = Genesis::try_new_with_validator_count(label, 1).expect("genesis");
        genesis.ordered_history_v2_activation_height = Some(1);
        lifecycle_queries::init_with_genesis(data_dir.clone(), "validator-0".to_owned(), genesis.clone())
            .expect("init");
        let store = NodeStore::new(&data_dir);
        let batch = postfiat_mempool_dag::build_transaction_batch(
            &mempool_batch_domain(&genesis),
            Vec::new(),
        )
        .expect("empty batch")
        .batch;
        let journal = replicated_state_activation::activation_test_journal(
            &store,
            &genesis,
            &store.read_governance().expect("governance"),
            &store.read_ledger().expect("ledger"),
            BATCH_KIND_TRANSPARENT,
            &batch.batch_id,
            &batch,
            &[],
            false,
        );
        replicated_state_activation::apply_activation_journal(&store, &journal);
        assert!(store.transactional_storage_active().expect("active"));
        data_dir
    }

    fn owner_only_key_file(data_dir: &Path, name: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = data_dir.join(name);
        fs::write(&path, b"{}").expect("key file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("key file mode");
        path
    }

    /// The pricing fixture is a JSON store written at height 700, which no
    /// transactional store holds, so its reads carry `store_not_transactional`
    /// and nothing else from custody. This returns the report without that
    /// one reason, for the governance and verifier checks; custody is checked
    /// on an active store below.
    fn readiness_for(
        fixture: &PricingReadFixture,
        verifiers: &ReadinessTestVerifiers,
    ) -> PrivateReadinessReport {
        let mut report = super::private_readiness_read::private_readiness_for_pair(
            PrivateReadinessOptions {
                data_dir: fixture.data_dir.clone(),
                pool_id: None,
                admission_switch: None,
                service: Some(hardened_test_service(&fixture.data_dir)),
            },
            verifiers,
            PrivateAssetPair {
                base_asset_id: fixture.base_asset_id.clone(),
                quote_asset_id: fixture.quote_asset_id.clone(),
            },
        )
        .expect("readiness");
        assert!(!report.custody.transactional_active);
        assert!(!report.ready);
        assert_eq!(report.reasons.first().map(String::as_str), Some("store_not_transactional"));
        report.reasons.remove(0);
        report.ready = report.reasons.is_empty();
        report
    }

    #[test]
    fn private_readiness_follows_the_governed_atomic_batch_activation() {
        for (atomic_height, ready, reason) in [
            (None, false, Some("shielded_atomic_batch_activation_absent")),
            (Some(800), false, Some("shielded_atomic_batch_activation_pending")),
            (Some(701), true, None),
        ] {
            let fixture = readiness_fixture(atomic_height, true, false);
            let report = readiness_for(&fixture, &ReadinessTestVerifiers::pinned());
            let store = NodeStore::new(&fixture.data_dir);
            let governance = store.read_governance().expect("governance");
            let tip = store.read_chain_tip().expect("tip");
            // The same governed height and the same rule execute_shielded_batch applies.
            let consensus_height = governance.shielded_atomic_batch_activation_height();
            assert_eq!(report.atomic_shielded_batches.effective_height, consensus_height);
            assert_eq!(
                report.atomic_shielded_batches.active_at_next_height,
                consensus_height.is_some_and(|height| tip.height + 1 >= height)
            );
            assert_eq!(report.ready, ready, "{report:?}");
            match reason {
                Some(reason) => assert!(report.reasons.iter().any(|r| r == reason), "{report:?}"),
                None => assert!(report.reasons.is_empty(), "{report:?}"),
            }
            assert_eq!(report.schema, PRIVATE_READINESS_SCHEMA);
            assert_eq!(
                (report.finalized_height, report.finalized_block_hash.as_str(), report.finalized_state_root.as_str()),
                (tip.height, tip.block_hash.as_str(), tip.state_root.as_str())
            );
            assert_eq!(report.pricing_enforcement.effective_height, Some(650));
            assert!(report.pricing_enforcement.active_at_next_height);
            assert_eq!(report.pricing_max_nav_age_blocks, Some(160));
            assert_eq!(report.admission_switch.state, "unknown");
            fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
        }
    }

    #[test]
    fn private_readiness_requires_built_verifiers_with_the_pinned_identity() {
        let fixture = readiness_fixture(Some(701), true, false);
        let missing = ReadinessTestVerifiers {
            swap: Err("asset_orchard_swap_vk_build_failed".to_string()),
            private_egress: Ok(pinned_private_egress_verifier_identity()),
        };
        let report = readiness_for(&fixture, &missing);
        assert!(!report.ready);
        assert!(!report.swap_verifier.loaded);
        assert_eq!(report.reasons, ["swap_verifier_not_loaded"]);
        assert_eq!(report.swap_verifier.pinned, pinned_swap_verifier_identity());

        let mut other = pinned_private_egress_verifier_identity();
        other.vk_hash = "00".repeat(48);
        let mismatched = ReadinessTestVerifiers {
            swap: Ok(pinned_swap_verifier_identity()),
            private_egress: Ok(other),
        };
        let report = readiness_for(&fixture, &mismatched);
        assert!(!report.ready);
        assert!(report.private_egress_verifier.loaded);
        assert!(!report.private_egress_verifier.matches_pin);
        assert_eq!(report.reasons, ["private_egress_verifier_identity_mismatch"]);
        assert_eq!(
            report.swap_verifier.pinned.circuit_id,
            "asset_orchard.swap.pricing_bound.v4"
        );
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_readiness_reports_the_governed_a666_pfusdc_route() {
        let absent = readiness_fixture(Some(701), false, false);
        let report = readiness_for(&absent, &ReadinessTestVerifiers::pinned());
        assert!(report.governed_routes.is_empty());
        assert_eq!(report.a666_pfusdc_route, None);
        assert!(report.a666_pfusdc_route_matches.is_empty());
        assert_eq!(report.a666_pfusdc_pair.base_asset_id, absent.base_asset_id);
        assert_eq!(report.a666_pfusdc_pair.quote_asset_id, absent.quote_asset_id);
        assert_eq!(report.reasons, ["a666_pfusdc_route_absent"]);
        fs::remove_dir_all(&absent.data_dir).expect("cleanup");

        let present = readiness_fixture(Some(701), true, false);
        let report = readiness_for(&present, &ReadinessTestVerifiers::pinned());
        let route = report.a666_pfusdc_route.clone().expect("route");
        assert_eq!(route.route_id, "a666-pfusdc-route");
        assert_eq!(route.base_asset_id, present.base_asset_id);
        assert_eq!(route.quote_asset_id, present.quote_asset_id);
        assert_eq!(
            (route.base_code.as_deref(), route.quote_code.as_deref()),
            (Some("A666"), Some("PFUSDC"))
        );
        assert!(report.ready, "{report:?}");
        fs::remove_dir_all(&present.data_dir).expect("cleanup");

        let paused = readiness_fixture(Some(701), true, true);
        let report = readiness_for(&paused, &ReadinessTestVerifiers::pinned());
        assert_eq!(report.reasons, ["a666_pfusdc_route_paused"]);
        fs::remove_dir_all(&paused.data_dir).expect("cleanup");
    }

    #[test]
    fn private_readiness_matches_the_route_by_asset_id_when_codes_repeat() {
        // The devnet's case: a second route whose base asset is coded `a666`
        // under another asset id. Matching by code found two and reported
        // the route absent; the pricing rule matches by asset id.
        let fixture = readiness_fixture_with_routes(Some(701), |ledger, base, quote| {
            let mut other_base = ledger
                .asset_definitions
                .iter()
                .find(|asset| asset.asset_id == base)
                .expect("base definition")
                .clone();
            other_base.asset_id = "33".repeat(48);
            other_base.code = "a666".to_string();
            let mut other = governed_pricing_route(&other_base.asset_id, quote);
            other.route_id = "a666-sepolia-pfusdc-route".to_string();
            ledger.asset_definitions.push(other_base);
            ledger.pftl_uniswap_routes.push(other);
            ledger
                .pftl_uniswap_routes
                .push(governed_pricing_route(base, quote));
        });
        let report = readiness_for(&fixture, &ReadinessTestVerifiers::pinned());
        assert_eq!(report.governed_routes.len(), 2);
        assert!(report.governed_routes.iter().all(|route| {
            route
                .base_code
                .as_deref()
                .is_some_and(|code| code.eq_ignore_ascii_case("A666"))
                && route.quote_code.as_deref() == Some("PFUSDC")
        }));
        assert_eq!(report.a666_pfusdc_route_matches, ["a666-pfusdc-route"]);
        let route = report.a666_pfusdc_route.clone().expect("route");
        assert_eq!(route.route_id, "a666-pfusdc-route");
        assert_eq!(route.base_asset_id, fixture.base_asset_id);
        assert_eq!(route.quote_asset_id, fixture.quote_asset_id);
        assert!(!route.paused);
        assert!(report.reasons.is_empty(), "{:?}", report.reasons);
        assert!(report.ready);
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_readiness_reports_two_routes_with_the_same_pair_as_ambiguous() {
        let fixture = readiness_fixture_with_routes(Some(701), |ledger, base, quote| {
            let mut second = governed_pricing_route(base, quote);
            second.route_id = "a666-pfusdc-route-2".to_string();
            ledger
                .pftl_uniswap_routes
                .push(governed_pricing_route(base, quote));
            ledger.pftl_uniswap_routes.push(second);
        });
        let report = readiness_for(&fixture, &ReadinessTestVerifiers::pinned());
        assert_eq!(report.a666_pfusdc_route, None);
        assert_eq!(
            report.a666_pfusdc_route_matches,
            ["a666-pfusdc-route", "a666-pfusdc-route-2"]
        );
        assert_eq!(report.reasons, ["a666_pfusdc_route_ambiguous"]);
        assert!(!report.ready);
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_readiness_is_byte_identical_on_repeat_reads_and_refuses_torn_state() {
        let fixture = readiness_fixture(Some(701), true, false);
        let verifiers = ReadinessTestVerifiers::pinned();
        let first = serde_json::to_vec(&readiness_for(&fixture, &verifiers)).expect("json");
        let second = serde_json::to_vec(&readiness_for(&fixture, &verifiers)).expect("json");
        assert_eq!(first, second);

        // Governance edited without a committed tip is never attested.
        let store = NodeStore::new(&fixture.data_dir);
        let mut governance = store.read_governance().expect("governance");
        governance.apply(pricing_enforcement_amendment(
            GOVERNANCE_KIND_SHIELDED_ATOMIC_BATCH_ACTIVATION_HEIGHT,
            1,
        ));
        store.write_governance(&governance).expect("edit governance");
        let error = super::private_readiness_read::private_readiness_with_probe(
            PrivateReadinessOptions {
                data_dir: fixture.data_dir.clone(),
                pool_id: None,
                admission_switch: None,
                service: None,
            },
            &verifiers,
        )
        .expect_err("torn state");
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_readiness_builds_the_real_verifiers_with_their_pinned_identity() {
        let fixture = readiness_fixture(Some(701), true, false);
        let report = private_readiness(PrivateReadinessOptions {
            data_dir: fixture.data_dir.clone(),
            pool_id: None,
            admission_switch: None,
            service: None,
        })
        .expect("readiness");
        for verifier in [&report.swap_verifier, &report.private_egress_verifier] {
            assert!(verifier.loaded, "{verifier:?}");
            assert!(verifier.matches_pin, "{verifier:?}");
            assert_eq!(verifier.loaded_identity.as_ref(), Some(&verifier.pinned));
        }
        // The served read matches the W-1 contract's devnet pair; the
        // fixture's assets are not those, so only the route is missing.
        assert_eq!(report.a666_pfusdc_pair, PrivateAssetPair::w1_a666_pfusdc());
        assert_eq!(
            report.reasons,
            [
                "readiness_not_published_by_running_service",
                "store_not_transactional",
                "a666_pfusdc_route_absent"
            ],
            "{report:?}"
        );
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn private_readiness_custody_refuses_a_local_read_a_file_signer_and_a_shared_key() {
        use std::os::unix::fs::PermissionsExt;
        let data_dir = active_transactional_store("postfiat-private-readiness-custody");
        let verifiers = ReadinessTestVerifiers::pinned();
        let read = |service: Option<PrivateReadinessService>| {
            let report = super::private_readiness_read::private_readiness_with_probe(
                PrivateReadinessOptions {
                    data_dir: data_dir.clone(),
                    pool_id: None,
                    admission_switch: Some(true),
                    service,
                },
                &verifiers,
            )
            .expect("readiness");
            // The custody reasons only: this chain has no governed records.
            let custody = report
                .reasons
                .iter()
                .filter(|reason| PRIVATE_READINESS_CUSTODY_REASONS.contains(&reason.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            (report, custody)
        };
        let hardened = |key_files: Vec<PathBuf>| {
            Some(PrivateReadinessService {
                key_files,
                signer: PrivateValidatorSigner::TestHardened,
                signer_socket: None,
            })
        };

        // Only the running service over an active transactional store
        // satisfies custody, and only with a hardened signer over owner-only
        // key files.
        let (passes, reasons) = read(Some(hardened_test_service(&data_dir)));
        assert!(reasons.is_empty(), "{passes:?}");
        assert_eq!(passes.custody.published_by, "rpc_serve");
        assert_eq!(passes.custody.store_backend, "transactional");
        assert!(passes.custody.transactional_active);
        assert_eq!(passes.custody.key_files, ["owner_only"]);

        // A local read of a data directory, which may be a copy, never does.
        let (local, reasons) = read(None);
        assert_eq!(local.custody.published_by, "local_read");
        assert_eq!(local.custody.validator_signer, "none");
        assert!(local.custody.key_files.is_empty());
        assert_eq!(reasons, ["readiness_not_published_by_running_service"]);

        // A server with no signing role signs with no key: `none`, unmet,
        // and its key file is not read.
        let (unsigned, reasons) = read(Some(PrivateReadinessService {
            key_files: vec![data_dir.join("no-such-key.json")],
            signer: PrivateValidatorSigner::NoSigningRole,
            signer_socket: None,
        }));
        assert_eq!(unsigned.custody.validator_signer, "none");
        assert!(unsigned.custody.key_files.is_empty());
        assert_eq!(reasons, ["validator_signer_none"]);

        // The plaintext key file every validator signs with today never does.
        let key_file = owner_only_key_file(&data_dir, "validator-file-key.json");
        let (file_signer, reasons) = read(Some(PrivateReadinessService {
            key_files: vec![key_file.clone()],
            signer: PrivateValidatorSigner::PlaintextKeyFile,
            signer_socket: None,
        }));
        assert_eq!(file_signer.custody.validator_signer, "plaintext_key_file");
        assert_eq!(reasons, ["validator_signer_unsafe_file"]);

        // Every key file the signer uses is checked: a shared proposal key
        // fails even when the finality key file is owner-only.
        let proposal = owner_only_key_file(&data_dir, "validator-proposal-key.json");
        fs::set_permissions(&proposal, fs::Permissions::from_mode(0o640)).expect("mode");
        let (split, reasons) = read(hardened(vec![key_file.clone(), proposal.clone()]));
        assert_eq!(split.custody.key_files, ["owner_only", "not_owner_only"]);
        assert_eq!(reasons, ["validator_key_file_not_owner_only"]);

        // A key file another local user can read, a missing one or one that
        // cannot be inspected never does.
        let (missing, reasons) = read(hardened(vec![data_dir.join("no-such-key.json")]));
        assert_eq!(missing.custody.key_files, ["missing"]);
        assert_eq!(reasons, ["validator_key_file_missing"]);
        let (unreadable, reasons) = read(hardened(vec![key_file.join("validator_keys.json")]));
        assert_eq!(unreadable.custody.key_files, ["unreadable"]);
        assert_eq!(reasons, ["validator_key_file_unreadable"]);
        fs::remove_dir_all(&data_dir).expect("cleanup");
    }

    #[test]
    fn private_readiness_key_file_must_be_owned_by_the_serving_user() {
        use std::os::unix::fs::PermissionsExt;
        let dir = unique_read_fixture_dir("postfiat-private-readiness-key-owner");
        fs::create_dir_all(&dir).expect("dir");
        let key_file = owner_only_key_file(&dir, "validator_keys.json");
        // SAFETY: geteuid has no preconditions and cannot fail.
        let euid = unsafe { libc::geteuid() };
        let state = super::private_readiness_read::key_file_state;
        assert_eq!(state(&key_file, euid), "owner_only");
        // 0600 owned by another user: this process could not even read it.
        assert_eq!(state(&key_file, euid.wrapping_add(1)), "not_owner_only");
        // Any mode without group or other access passes; any such bit fails.
        fs::set_permissions(&key_file, fs::Permissions::from_mode(0o400)).expect("mode");
        assert_eq!(state(&key_file, euid), "owner_only");
        fs::set_permissions(&key_file, fs::Permissions::from_mode(0o604)).expect("mode");
        assert_eq!(state(&key_file, euid), "not_owner_only");
        assert_eq!(state(&dir, euid), "not_owner_only", "a directory is not a key file");
        // EACCES on the parent directory, where the test user is not root.
        if euid != 0 {
            let locked = dir.join("locked");
            fs::create_dir_all(&locked).expect("locked dir");
            let inner = owner_only_key_file(&locked, "validator_keys.json");
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("lock");
            assert_eq!(state(&inner, euid), "unreadable");
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).expect("unlock");
        }
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn private_readiness_custody_refuses_an_inactive_store() {
        for (backend, active, refused) in [
            (postfiat_storage::StorageBackendMode::Transactional, true, false),
            (postfiat_storage::StorageBackendMode::Transactional, false, true),
            (postfiat_storage::StorageBackendMode::BoundedJsonl, false, true),
            (postfiat_storage::StorageBackendMode::LegacyJsonl, false, true),
        ] {
            let custody = PrivateReadinessCustody {
                signer_double_sign_protection: None,
                data_dir_key_files: Vec::new(),
                published_by: "rpc_serve".to_string(),
                store_backend: backend.as_str().to_string(),
                transactional_active: active,
                validator_signer: "hardened".to_string(),
                key_files: vec!["owner_only".to_string()],
                signer_socket: None,
                signer_uid: None,
                serving_uid: 1000,
                signer_key_matches: None,
                validator_registry_readable: None,
            };
            let reasons = super::private_readiness_read::custody_reasons(&custody);
            assert_eq!(
                reasons.iter().any(|reason| reason == "store_not_transactional"),
                refused,
                "{backend:?} active {active}: {reasons:?}"
            );
        }
    }

    #[test]
    fn private_readiness_socket_signer_needs_another_user_and_no_key_file() {
        let custody = |signer_uid: Option<u32>,
                       signer_key_matches: Option<bool>,
                       validator_registry_readable: Option<bool>| PrivateReadinessCustody {
            signer_double_sign_protection: Some(true),
            data_dir_key_files: Vec::new(),
            published_by: "rpc_serve".to_string(),
            store_backend: "transactional".to_string(),
            transactional_active: true,
            validator_signer: "socket".to_string(),
            key_files: Vec::new(),
            signer_socket: Some("/run/postfiat-signer/validator-0.sock".to_string()),
            signer_uid,
            serving_uid: 1000,
            signer_key_matches,
            validator_registry_readable,
        };
        let reasons = |uid, keys, registry| {
            super::private_readiness_read::custody_reasons(&custody(uid, keys, registry))
        };
        // A signer under another user, with no key file in the serving
        // process, satisfies custody: none of the file reasons apply.
        assert_eq!(reasons(Some(1001), Some(true), Some(true)), Vec::<String>::new());
        assert_eq!(reasons(Some(1000), Some(true), Some(true)), ["validator_signer_same_user"]);
        assert_eq!(reasons(None, None, None), ["validator_signer_unreachable"]);
        // A signer that answers with keys other than the registry's.
        assert_eq!(reasons(Some(1001), Some(false), Some(true)), ["validator_signer_key_mismatch"]);
        // A key copy left in the serving data directory, readable by rpc-serve.
        let mut copied = custody(Some(1001), Some(true), Some(true));
        copied.data_dir_key_files = vec!["validator_keys.json".to_string()];
        assert_eq!(super::private_readiness_read::custody_reasons(&copied),
            ["validator_key_copy_in_data_dir"]);
        for available in [None, Some(false)] {
            let mut unguarded = custody(Some(1001), Some(true), Some(true));
            unguarded.signer_double_sign_protection = available;
            assert_eq!(super::private_readiness_read::custody_reasons(&unguarded),
                ["validator_signer_guard_unavailable"]);
        }
    }

    #[test]
    fn private_readiness_reports_an_unreadable_registry_apart_from_a_key_mismatch() {
        let custody = PrivateReadinessCustody {
            signer_double_sign_protection: Some(true),
            data_dir_key_files: Vec::new(),
            published_by: "rpc_serve".to_string(),
            store_backend: "transactional".to_string(),
            transactional_active: true,
            validator_signer: "socket".to_string(),
            key_files: Vec::new(),
            signer_socket: Some("/run/postfiat-signer/validator-0.sock".to_string()),
            signer_uid: Some(1001),
            serving_uid: 1000,
            // The signer answered, but the registry could not be read, so
            // the comparison is unknown rather than false.
            signer_key_matches: None,
            validator_registry_readable: Some(false),
        };
        assert_eq!(
            super::private_readiness_read::custody_reasons(&custody),
            ["validator_registry_unreadable"]
        );
        assert!(super::private_readiness_read::PRIVATE_READINESS_CUSTODY_REASONS
            .contains(&"validator_registry_unreadable"));
    }

    #[test]
    fn private_readiness_custody_refuses_a_plain_init_store() {
        // `init` writes a JSON store with no backend-mode record and no
        // transactional database: the default mode is not an active store.
        let data_dir = unique_read_fixture_dir("postfiat-private-readiness-plain-init");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-private-readiness-plain-init".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init");
        let report = super::private_readiness_read::private_readiness_with_probe(
            PrivateReadinessOptions {
                data_dir: data_dir.clone(),
                pool_id: None,
                admission_switch: Some(true),
                service: Some(hardened_test_service(&data_dir)),
            },
            &ReadinessTestVerifiers::pinned(),
        )
        .expect("readiness");
        assert!(!report.ready);
        // The mode reads as the default; the store is not active.
        assert_eq!(report.custody.store_backend, "transactional");
        assert!(!report.custody.transactional_active);
        assert_eq!(
            report.reasons.first().map(String::as_str),
            Some("store_not_transactional"),
            "{report:?}"
        );
        fs::remove_dir_all(&data_dir).expect("cleanup");
    }

    #[test]
    fn private_readiness_types_an_unreadable_key_file() {
        // A stat error other than NotFound (here ENOTDIR; EACCES on the parent
        // directory alike) is a typed reason, not a failed read.
        let fixture = readiness_fixture(Some(701), true, false);
        let not_a_directory = owner_only_key_file(&fixture.data_dir, "plain-file");
        let report = super::private_readiness_read::private_readiness_with_probe(
            PrivateReadinessOptions {
                data_dir: fixture.data_dir.clone(),
                pool_id: None,
                admission_switch: Some(true),
                service: Some(PrivateReadinessService {
                    key_files: vec![not_a_directory.join("validator_keys.json")],
                    signer: PrivateValidatorSigner::TestHardened,
                    signer_socket: None,
                }),
            },
            &ReadinessTestVerifiers::pinned(),
        )
        .expect("readiness");
        assert_eq!(report.custody.key_files, ["unreadable"]);
        assert!(report
            .reasons
            .iter()
            .any(|reason| reason == "validator_key_file_unreadable"));
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }

