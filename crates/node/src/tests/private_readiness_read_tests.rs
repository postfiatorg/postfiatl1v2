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
        pricing_read_fixture(700, move |ledger, governance| {
            if with_route {
                let base = ledger.nav_assets[0].asset_id.clone();
                let quote = ledger
                    .asset_definitions
                    .iter()
                    .find(|asset| asset.code == "PFUSDC")
                    .expect("quote")
                    .asset_id
                    .clone();
                let mut route = governed_pricing_route(&base, &quote);
                route.paused = route_paused;
                ledger.pftl_uniswap_routes.push(route);
            }
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

    fn readiness_for(
        fixture: &PricingReadFixture,
        verifiers: &ReadinessTestVerifiers,
    ) -> PrivateReadinessReport {
        super::private_readiness_read::private_readiness_with_probe(
            PrivateReadinessOptions {
                data_dir: fixture.data_dir.clone(),
                pool_id: None,
            },
            verifiers,
        )
        .expect("readiness")
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
        })
        .expect("readiness");
        for verifier in [&report.swap_verifier, &report.private_egress_verifier] {
            assert!(verifier.loaded, "{verifier:?}");
            assert!(verifier.matches_pin, "{verifier:?}");
            assert_eq!(verifier.loaded_identity.as_ref(), Some(&verifier.pinned));
        }
        assert!(report.ready, "{report:?}");
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");
    }
