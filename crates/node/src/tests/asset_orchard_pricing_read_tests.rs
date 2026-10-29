    const PRICING_READ_NAV_PER_UNIT: u64 = 1_234_500_000;
    const PRICING_READ_FINALIZED_AT: u64 = 590;
    const PRICING_READ_EPOCH_GAP: u64 = 200;

    struct PricingReadFixture {
        data_dir: PathBuf,
        base_asset_id: String,
        quote_asset_id: String,
        profile_id: String,
        issuer: String,
    }

    /// A666-like NAV base and pfUSDC-like quote with one finalized NAV epoch,
    /// committed under a chain tip at `tip_height` whose state root is the
    /// root of exactly this state.
    fn pricing_read_fixture(
        tip_height: u64,
        mutate: impl FnOnce(&mut LedgerState, &mut GovernanceState),
    ) -> PricingReadFixture {
        let data_dir = unique_test_dir("postfiat-asset-orchard-pricing-read");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-asset-orchard-pricing-read".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init pricing read fixture");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let issuer = read_transfer_key_file(&data_dir, None)
            .expect("issuer key")
            .address;
        let base = AssetDefinition::new(&genesis.chain_id, issuer.clone(), "A666".to_string(), 1, 6)
            .expect("A666 definition");
        let quote =
            AssetDefinition::new(&genesis.chain_id, issuer.clone(), "PFUSDC".to_string(), 1, 6)
                .expect("pfUSDC definition");
        let profile = postfiat_types::NavProofProfile::new(
            issuer.clone(),
            postfiat_types::NAV_PROFILE_VERIFIER_PLACEHOLDER,
            "test-finalized-nav",
            100,
            1,
            PRICING_READ_EPOCH_GAP,
            0,
            0,
            0,
            0,
            "",
            "",
            "",
            0,
            0,
        )
        .expect("pricing profile");
        let mut nav = postfiat_types::NavTrackedAsset::new(
            base.asset_id.clone(),
            issuer.clone(),
            issuer.clone(),
            profile.profile_id.clone(),
            "USD",
            issuer.clone(),
        )
        .expect("NAV asset");
        nav.finalized_epoch = 7;
        nav.circulating_supply = 40;
        nav.nav_per_unit = PRICING_READ_NAV_PER_UNIT;
        nav.finalized_reserve_packet_hash = "ab".repeat(48);
        nav.finalized_at_height = PRICING_READ_FINALIZED_AT;
        let fixture = PricingReadFixture {
            data_dir: data_dir.clone(),
            base_asset_id: base.asset_id.clone(),
            quote_asset_id: quote.asset_id.clone(),
            profile_id: profile.profile_id.clone(),
            issuer,
        };
        let mut ledger = store.read_ledger().expect("ledger");
        ledger.asset_definitions.extend([base, quote]);
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_assets.push(nav);
        let mut governance = store.read_governance().expect("governance");
        mutate(&mut ledger, &mut governance);
        store.write_ledger(&ledger).expect("write ledger");
        store.write_governance(&governance).expect("write governance");
        let mut tip = read_chain_tip_or_reconstruct_for_genesis_read_only(&store, &genesis)
            .expect("genesis tip");
        tip.height = tip_height;
        tip.state_root = current_replicated_state_root(&store, &genesis).expect("state root");
        store.write_chain_tip(&tip).expect("write tip");
        fixture
    }

    fn pricing_read_options(
        fixture: &PricingReadFixture,
        inclusion_height: u64,
    ) -> AssetOrchardPairReadOptions {
        AssetOrchardPairReadOptions {
            data_dir: fixture.data_dir.clone(),
            base_asset_id: fixture.base_asset_id.clone(),
            quote_asset_id: fixture.quote_asset_id.clone(),
            inclusion_height,
        }
    }

    fn pricing_read_status(fixture: &PricingReadFixture, inclusion_height: u64) -> (String, String) {
        let report = asset_orchard_pricing_claim(pricing_read_options(fixture, inclusion_height))
            .expect("pricing claim read");
        assert!(report.pricing_claim.is_none() || report.status == "ok");
        let code = report.refusal.map(|refusal| refusal.code).unwrap_or_default();
        (report.status, code)
    }

    #[test]
    fn asset_orchard_pricing_claim_round_trips_through_consensus_pricing_plan() {
        let fixture = pricing_read_fixture(700, |_, _| {});
        let report = asset_orchard_pricing_claim(pricing_read_options(&fixture, 701))
            .expect("pricing claim read");
        assert_eq!(report.schema, ASSET_ORCHARD_PRICING_CLAIM_READ_SCHEMA);
        assert_eq!(report.status, "ok", "{report:?}");
        assert!(report.refusal.is_none());
        let store = NodeStore::new(&fixture.data_dir);
        let tip = store.read_chain_tip().expect("tip");
        assert_eq!(report.finalized_height, 700);
        assert_eq!(report.finalized_block_hash, tip.block_hash);
        assert_eq!(report.finalized_state_root, tip.state_root);
        let base = report.base_asset.as_ref().expect("base asset");
        assert_eq!(base.asset_id, fixture.base_asset_id);
        assert_eq!(base.issuer, fixture.issuer);
        assert_eq!(base.proof_profile_id.as_deref(), Some(fixture.profile_id.as_str()));
        assert_eq!(base.finalized_epoch, Some(7));
        assert_eq!(
            base.nav_live_through_height,
            Some(PRICING_READ_FINALIZED_AT + PRICING_READ_EPOCH_GAP)
        );
        let quote = report.quote_asset.as_ref().expect("quote asset");
        assert_eq!(quote.asset_id, fixture.quote_asset_id);
        assert_eq!(quote.issuer, fixture.issuer);
        assert!(!quote.nav_tracked);
        assert_eq!(quote.proof_profile_id, None);

        let claim = report.pricing_claim.clone().expect("pricing claim");
        let base_tag = AssetTag::derive(&fixture.base_asset_id).expect("base tag");
        let quote_tag = AssetTag::derive(&fixture.quote_asset_id).expect("quote tag");
        assert_eq!(
            claim,
            AssetOrchardPricingClaim {
                nav_epoch: 7,
                reserve_packet_hash: "ab".repeat(48),
                ratio_numerator: PRICING_READ_NAV_PER_UNIT,
                ratio_denominator: postfiat_types::NAV_USD_E8_UNIT,
                mode: "at_nav_with_band".to_string(),
                band_bps: 0,
                base_asset_tag_lo: base_tag.lo,
                base_asset_tag_hi: base_tag.hi,
                quote_asset_tag_lo: quote_tag.lo,
                quote_asset_tag_hi: quote_tag.hi,
            }
        );
        assert_eq!(base.asset_tag_lo, format!("{:032x}", base_tag.lo));

        // Consensus accepts a verified swap bound to exactly this claim at the
        // intended inclusion height, and rejects any drift from it.
        let ledger = store.read_ledger().expect("ledger");
        let (_, mut verified) =
            asset_orchard_test_action_and_verified(asset_orchard_test_encrypted_outputs());
        verified.pricing.claim = claim.clone();
        assert_eq!(
            asset_orchard_swap_pricing_plan(&ledger, &verified, 701).expect("consensus accepts"),
            None
        );
        let mut stale = verified.clone();
        stale.pricing.claim.nav_epoch -= 1;
        assert_eq!(
            asset_orchard_swap_pricing_plan(&ledger, &stale, 701)
                .expect_err("older epoch")
                .code(),
            "asset_orchard_pricing_epoch_mismatch"
        );
        let mut off_band = verified.clone();
        off_band.pricing.claim.ratio_numerator += 1;
        assert_eq!(
            asset_orchard_swap_pricing_plan(&ledger, &off_band, 701)
                .expect_err("ratio drift")
                .code(),
            "asset_orchard_pricing_off_band"
        );
        let mut unknown_quote = verified.clone();
        let unknown_tag = AssetTag::derive(&"cd".repeat(48)).expect("unknown tag");
        unknown_quote.pricing.claim.quote_asset_tag_lo = unknown_tag.lo;
        unknown_quote.pricing.claim.quote_asset_tag_hi = unknown_tag.hi;
        assert_eq!(
            asset_orchard_swap_pricing_plan(&ledger, &unknown_quote, 701)
                .expect_err("unregistered quote")
                .code(),
            "asset_orchard_pricing_quote_asset_mismatch"
        );

        let encoded = serde_json::to_string(&report).expect("report json");
        assert!(encoded.contains("\"base_asset_tag_lo\":\""));
        fs::remove_dir_all(fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_pricing_claim_changes_denominator_units_at_activation_height() {
        let fixture = pricing_read_fixture(600, |_, _| {});
        let before = asset_orchard_pricing_claim(pricing_read_options(
            &fixture,
            ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT - 1,
        ))
        .expect("pre-activation read");
        assert_eq!(before.status, "mismatch");
        assert_eq!(
            before.refusal.expect("refusal").code,
            "asset_orchard_pricing_before_nav_usd_e8_activation"
        );
        assert_eq!(before.ratio_denominator_unit, postfiat_types::VAULT_BRIDGE_UNIT);
        assert!(before.pricing_claim.is_none());

        let at = asset_orchard_pricing_claim(pricing_read_options(
            &fixture,
            ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT,
        ))
        .expect("activation read");
        assert_eq!(at.status, "ok", "{at:?}");
        assert_eq!(at.ratio_denominator_unit, postfiat_types::NAV_USD_E8_UNIT);
        let claim = at.pricing_claim.expect("claim");
        assert_eq!(claim.ratio_denominator, postfiat_types::NAV_USD_E8_UNIT);

        let ledger = NodeStore::new(&fixture.data_dir)
            .read_ledger()
            .expect("ledger");
        let (_, mut verified) =
            asset_orchard_test_action_and_verified(asset_orchard_test_encrypted_outputs());
        verified.pricing.claim = claim;
        asset_orchard_swap_pricing_plan(&ledger, &verified, ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT)
            .expect("consensus accepts the claim at the activation height");
        assert_eq!(
            asset_orchard_swap_pricing_plan(
                &ledger,
                &verified,
                ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT - 1
            )
            .expect_err("legacy units one block earlier")
            .code(),
            "asset_orchard_pricing_off_band"
        );
        fs::remove_dir_all(fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_pricing_claim_refuses_stale_and_halted_state() {
        let fixture = pricing_read_fixture(700, |_, _| {});
        assert_eq!(
            pricing_read_status(&fixture, 700),
            (
                "stale".to_string(),
                "asset_orchard_inclusion_height_not_after_finalized".to_string()
            )
        );
        let live_through = PRICING_READ_FINALIZED_AT + PRICING_READ_EPOCH_GAP;
        assert_eq!(pricing_read_status(&fixture, live_through).0, "ok");
        assert_eq!(
            pricing_read_status(&fixture, live_through + 1),
            ("stale".to_string(), "asset_orchard_pricing_nav_stale".to_string())
        );
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");

        let unfinalized = pricing_read_fixture(700, |ledger, _| {
            ledger.nav_assets[0].finalized_epoch = 0;
        });
        assert_eq!(
            pricing_read_status(&unfinalized, 701),
            ("stale".to_string(), "asset_orchard_pricing_not_finalized".to_string())
        );
        fs::remove_dir_all(&unfinalized.data_dir).expect("cleanup");

        let halted = pricing_read_fixture(700, |ledger, _| {
            ledger.nav_assets[0].halted = true;
            ledger.nav_assets[0].halt_reason = "reserve shortfall".to_string();
        });
        let report = asset_orchard_pricing_claim(pricing_read_options(&halted, 701))
            .expect("halted read");
        assert_eq!(report.status, "halted");
        let refusal = report.refusal.expect("refusal");
        assert_eq!(refusal.code, "asset_orchard_pricing_halted");
        assert!(refusal.reason.contains("reserve shortfall"));
        assert!(report.pricing_claim.is_none());
        fs::remove_dir_all(&halted.data_dir).expect("cleanup");

        let paused = pricing_read_fixture(700, |_, governance| {
            governance.orchard_pool_paused = true;
        });
        assert_eq!(
            pricing_read_status(&paused, 701),
            ("halted".to_string(), "orchard_pool_paused".to_string())
        );
        fs::remove_dir_all(&paused.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_pricing_claim_refuses_unknown_and_ungoverned_assets() {
        let fixture = pricing_read_fixture(700, |_, _| {});
        let unknown = AssetOrchardPairReadOptions {
            quote_asset_id: "cd".repeat(48),
            ..pricing_read_options(&fixture, 701)
        };
        let report = asset_orchard_pricing_claim(unknown).expect("unknown asset read");
        assert_eq!(report.status, "mismatch");
        assert_eq!(report.refusal.expect("refusal").code, "asset_orchard_asset_unknown");
        assert!(report.quote_asset.is_none());

        let reversed = AssetOrchardPairReadOptions {
            base_asset_id: fixture.quote_asset_id.clone(),
            quote_asset_id: fixture.base_asset_id.clone(),
            ..pricing_read_options(&fixture, 701)
        };
        let report = asset_orchard_pricing_claim(reversed).expect("non-NAV base read");
        assert_eq!(report.status, "mismatch");
        assert_eq!(
            report.refusal.expect("refusal").code,
            "asset_orchard_pricing_nav_asset_mismatch"
        );

        let identical = AssetOrchardPairReadOptions {
            quote_asset_id: fixture.base_asset_id.clone(),
            ..pricing_read_options(&fixture, 701)
        };
        assert_eq!(
            asset_orchard_pricing_claim(identical).expect("identical read").status,
            "mismatch"
        );

        let malformed = AssetOrchardPairReadOptions {
            base_asset_id: "A666".to_string(),
            ..pricing_read_options(&fixture, 701)
        };
        assert_eq!(
            asset_orchard_pricing_claim(malformed)
                .expect_err("symbols are not asset ids")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        fs::remove_dir_all(fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_pricing_reads_refuse_state_not_committed_by_the_tip() {
        let fixture = pricing_read_fixture(700, |_, _| {});
        let store = NodeStore::new(&fixture.data_dir);
        let mut ledger = store.read_ledger().expect("ledger");
        ledger.nav_assets[0].nav_per_unit += 1;
        store.write_ledger(&ledger).expect("uncommitted ledger write");
        for error in [
            asset_orchard_pricing_claim(pricing_read_options(&fixture, 701)).map(|_| ()),
            asset_orchard_private_egress_binding(pricing_read_options(&fixture, 701)).map(|_| ()),
        ] {
            assert_eq!(
                error.expect_err("torn state").kind(),
                io::ErrorKind::Interrupted
            );
        }
        fs::remove_dir_all(fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_private_egress_binding_reads_governed_state_only() {
        let fixture = pricing_read_fixture(700, |_, _| {});
        let report = asset_orchard_private_egress_binding(pricing_read_options(&fixture, 701))
            .expect("egress binding read");
        assert_eq!(report.schema, ASSET_ORCHARD_PRIVATE_EGRESS_BINDING_READ_SCHEMA);
        assert_eq!(report.status, "ungoverned");
        assert_eq!(
            report.refusal.as_ref().expect("refusal").code,
            "asset_orchard_private_egress_policy_ungoverned"
        );
        assert_eq!(report.finalized_height, 700);
        assert_eq!(
            report.finalized_state_root,
            NodeStore::new(&fixture.data_dir)
                .read_chain_tip()
                .expect("tip")
                .state_root
        );
        assert_eq!(
            report
                .bindings
                .iter()
                .map(|binding| binding.asset_id.as_str())
                .collect::<Vec<_>>(),
            [fixture.base_asset_id.as_str(), fixture.quote_asset_id.as_str()]
        );
        assert!(report
            .bindings
            .iter()
            .all(|binding| binding.policy_id.is_none() && binding.disclosure_hash.is_none()));
        let encoded = serde_json::to_string(&report.bindings).expect("bindings json");
        assert!(!encoded.contains("tnapp-owned-v3"));
        assert!(!encoded.contains(&"0".repeat(64)));
        assert_eq!(
            report.base_asset.expect("base").proof_profile_id.as_deref(),
            Some(fixture.profile_id.as_str())
        );

        let unknown = AssetOrchardPairReadOptions {
            base_asset_id: "cd".repeat(48),
            ..pricing_read_options(&fixture, 701)
        };
        let report = asset_orchard_private_egress_binding(unknown).expect("unknown read");
        assert_eq!(report.status, "mismatch");
        assert_eq!(report.refusal.expect("refusal").code, "asset_orchard_asset_unknown");
        assert_eq!(
            asset_orchard_private_egress_binding(pricing_read_options(&fixture, 700))
                .expect("stale read")
                .status,
            "stale"
        );
        fs::remove_dir_all(&fixture.data_dir).expect("cleanup");

        let halted = pricing_read_fixture(700, |ledger, _| {
            ledger.nav_assets[0].halted = true;
        });
        assert_eq!(
            asset_orchard_private_egress_binding(pricing_read_options(&halted, 701))
                .expect("halted read")
                .status,
            "halted"
        );
        fs::remove_dir_all(&halted.data_dir).expect("cleanup");
    }
