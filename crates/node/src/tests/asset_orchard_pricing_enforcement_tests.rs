    const PRICING_ENFORCEMENT_HEIGHT: u64 = 750;

    fn pricing_enforcement_amendment(kind: &str, value: u32) -> GovernanceAmendment {
        GovernanceAmendment {
            amendment_id: format!("{kind}-{value}"),
            chain_id: "postfiat-asset-orchard-pricing-read".to_string(),
            genesis_hash: "00".repeat(48),
            protocol_version: 1,
            instance_id: "pricing-enforcement-instance".to_string(),
            proposal_id: "pricing-enforcement-proposal".to_string(),
            certificate_id: "pricing-enforcement-certificate".to_string(),
            proposer: "validator-0".to_string(),
            validators: vec!["validator-0".to_string()],
            quorum: 1,
            kind: kind.to_string(),
            value,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            support: vec!["validator-0".to_string()],
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
        }
    }

    fn governed_pricing_route(base: &str, quote: &str) -> PftlUniswapConsensusRouteState {
        PftlUniswapConsensusRouteState {
            route_id: "a666-pfusdc-route".to_string(),
            route_family: "pftl_uniswap".to_string(),
            route_config_digest: "11".repeat(48),
            route_trust_class: "governed".to_string(),
            native_nav_asset_id: base.to_string(),
            settlement_asset_id: quote.to_string(),
            handoff_controller: "pf-controller".to_string(),
            settlement_adapter: "pf-adapter".to_string(),
            wrapped_navcoin_token: "0x".to_string() + &"22".repeat(20),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 1_000_000,
            packet_notional_cap_atoms: 1_000,
            latest_finalized_nav_epoch: 7,
            return_finality_blocks: 10,
            live_value_enabled: false,
            ethereum_verification_policy: None,
            authorized_valid_supply_atoms: 0,
            pftl_spendable_supply_atoms: 0,
            native_spendable_balances_atoms: std::collections::BTreeMap::new(),
            ethereum_spendable_supply_atoms: 0,
            other_registered_venue_supply_atoms: 0,
            outstanding_bridge_claims_atoms: 0,
            pending_return_import_claims_atoms: 0,
            settlement_reserve_atoms: 0,
            primary_subscription_nonces: std::collections::BTreeMap::new(),
            export_packets: std::collections::BTreeMap::new(),
            export_nonces: std::collections::BTreeMap::new(),
            return_imports: std::collections::BTreeMap::new(),
            paused: false,
            v2: None,
        }
    }

    fn pricing_enforcement(max_nav_age_blocks: Option<u64>, height: u64) -> Option<AssetOrchardPricingEnforcement> {
        asset_orchard_pricing_enforcement_at(
            &AssetExecutionCompatibility::strict().with_asset_orchard_pricing_enforcement(
                Some(PRICING_ENFORCEMENT_HEIGHT),
                max_nav_age_blocks,
            ),
            height,
        )
    }

    /// The pricing-read fixture's ledger with a governed A666/pfUSDC route,
    /// and a verified swap bound to the claim the read builds at 701.
    fn pricing_enforcement_ledger(
        with_route: bool,
    ) -> (PricingReadFixture, LedgerState, VerifiedAssetOrchardSwap) {
        let fixture = pricing_read_fixture(700, |_, _| {});
        let claim = asset_orchard_pricing_claim(pricing_read_options(&fixture, 701))
            .expect("pricing read")
            .pricing_claim
            .expect("claim");
        let mut ledger = NodeStore::new(&fixture.data_dir)
            .read_ledger()
            .expect("ledger");
        if with_route {
            ledger.pftl_uniswap_routes.push(governed_pricing_route(
                &fixture.base_asset_id,
                &fixture.quote_asset_id,
            ));
        }
        let (_, mut verified) =
            asset_orchard_test_action_and_verified(asset_orchard_test_encrypted_outputs());
        verified.pricing.claim = claim;
        (fixture, ledger, verified)
    }

    fn pricing_plan_code(
        ledger: &LedgerState,
        verified: &VerifiedAssetOrchardSwap,
        height: u64,
        max_nav_age_blocks: Option<u64>,
    ) -> Option<String> {
        asset_orchard_swap_pricing_plan_with_enforcement(
            ledger,
            verified,
            height,
            pricing_enforcement(max_nav_age_blocks, height),
        )
        .err()
        .map(|error| error.code().to_string())
    }

    #[test]
    fn asset_orchard_pricing_enforcement_is_governed_and_cannot_be_postponed() {
        let mut governance = GovernanceState::new(1);
        assert_eq!(governance.asset_orchard_pricing_enforcement_activation_height(), None);
        assert_eq!(governance.asset_orchard_pricing_max_nav_age_blocks(), None);
        let genesis = Genesis::new("postfiat-asset-orchard-pricing-read");
        let compatibility = asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
        assert!(!compatibility.asset_orchard_pricing_enforced(u64::MAX));

        let kind = GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_ENFORCEMENT_ACTIVATION_HEIGHT;
        governance.apply(pricing_enforcement_amendment(kind, 900));
        governance.apply(pricing_enforcement_amendment(kind, 800));
        let mut paused = pricing_enforcement_amendment(kind, 700);
        paused.paused = true;
        governance.apply(paused);
        governance.apply(pricing_enforcement_amendment(kind, 1_000));
        assert_eq!(governance.asset_orchard_pricing_enforcement_activation_height(), Some(800));
        let age = GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_MAX_NAV_AGE_BLOCKS;
        governance.apply(pricing_enforcement_amendment(age, 40));
        governance.apply(pricing_enforcement_amendment(age, 25));
        assert_eq!(governance.asset_orchard_pricing_max_nav_age_blocks(), Some(25));
        let compatibility = asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
        assert!(!compatibility.asset_orchard_pricing_enforced(799));
        assert!(compatibility.asset_orchard_pricing_enforced(800));
        assert_eq!(
            asset_orchard_pricing_enforcement_at(&compatibility, 800),
            Some(AssetOrchardPricingEnforcement {
                max_nav_age_blocks: Some(25)
            })
        );
        assert_eq!(asset_orchard_pricing_enforcement_at(&compatibility, 799), None);

        assert_eq!(
            governance_amendment_lifecycle_rejection(&pricing_enforcement_amendment(kind, 50), 50)
                .expect("not after the amendment block")
                .0,
            "invalid_asset_orchard_pricing_enforcement_activation_height"
        );
        assert!(governance_amendment_lifecycle_rejection(
            &pricing_enforcement_amendment(kind, 51),
            50
        )
        .is_none());
        assert_eq!(
            governance_amendment_lifecycle_rejection(&pricing_enforcement_amendment(age, 0), 50)
                .expect("zero age")
                .0,
            "invalid_asset_orchard_pricing_max_nav_age_blocks"
        );
    }

    #[test]
    fn asset_orchard_pricing_plan_is_unchanged_before_the_effective_height() {
        let (fixture, ledger, verified) = pricing_enforcement_ledger(false);
        let mut at_nav = verified.clone();
        at_nav.pricing.claim.mode = "at_nav".to_string();
        let mut own_quote = verified.clone();
        own_quote.pricing.claim.quote_asset_tag_lo = verified.pricing.claim.base_asset_tag_lo;
        own_quote.pricing.claim.quote_asset_tag_hi = verified.pricing.claim.base_asset_tag_hi;
        let mut older_epoch = ledger.clone();
        older_epoch.nav_assets[0].finalized_epoch += 1;
        older_epoch.nav_assets[0].finalized_reserve_packet_hash = "cd".repeat(48);
        let height = PRICING_ENFORCEMENT_HEIGHT - 1;
        for (ledger, verified) in [
            (&ledger, &verified),
            (&ledger, &at_nav),
            (&ledger, &own_quote),
            (&older_epoch, &verified),
        ] {
            let legacy = format!("{:?}", asset_orchard_swap_pricing_plan(ledger, verified, height));
            for max_nav_age_blocks in [None, Some(1)] {
                assert_eq!(pricing_enforcement(max_nav_age_blocks, height), None);
                assert_eq!(
                    format!(
                        "{:?}",
                        asset_orchard_swap_pricing_plan_with_enforcement(
                            ledger,
                            verified,
                            height,
                            pricing_enforcement(max_nav_age_blocks, height),
                        )
                    ),
                    legacy
                );
            }
        }
        // Before activation the three gaps are still open.
        assert_eq!(pricing_plan_code(&ledger, &at_nav, height, None), None);
        assert_eq!(pricing_plan_code(&ledger, &own_quote, height, None), None);
        assert_eq!(pricing_plan_code(&ledger, &verified, height, None), None);
        fs::remove_dir_all(fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_pricing_freshness_is_enforced_from_the_effective_height() {
        let (fixture, ledger, verified) = pricing_enforcement_ledger(true);
        for height in [PRICING_ENFORCEMENT_HEIGHT, PRICING_ENFORCEMENT_HEIGHT + 1] {
            assert_eq!(pricing_plan_code(&ledger, &verified, height, None), None);
        }
        let mut newer_epoch = ledger.clone();
        newer_epoch.nav_assets[0].finalized_epoch += 1;
        newer_epoch.nav_assets[0].finalized_reserve_packet_hash = "cd".repeat(48);
        let mut replaced_packet = ledger.clone();
        replaced_packet.nav_assets[0].finalized_reserve_packet_hash = "cd".repeat(48);
        for (stale, before) in [
            (&newer_epoch, "asset_orchard_pricing_epoch_mismatch"),
            (&replaced_packet, "asset_orchard_pricing_packet_mismatch"),
        ] {
            assert_eq!(
                pricing_plan_code(stale, &verified, PRICING_ENFORCEMENT_HEIGHT - 1, None).as_deref(),
                Some(before)
            );
            assert_eq!(
                pricing_plan_code(stale, &verified, PRICING_ENFORCEMENT_HEIGHT, None).as_deref(),
                Some("asset_orchard_pricing_nav_stale")
            );
        }

        // Default bound: the NAV profile's max_epoch_gap_blocks (590 + 200).
        let live_through = PRICING_READ_FINALIZED_AT + PRICING_READ_EPOCH_GAP;
        assert_eq!(pricing_plan_code(&ledger, &verified, live_through, None), None);
        assert_eq!(
            pricing_plan_code(&ledger, &verified, live_through + 1, None).as_deref(),
            Some("asset_orchard_pricing_nav_stale")
        );
        assert_eq!(
            pricing_plan_code(&ledger, &verified, PRICING_ENFORCEMENT_HEIGHT - 1, Some(1)),
            None,
            "the governed age bound is inert before activation"
        );
        // Governed bound: 590 + 160 = 750 is the last fresh height.
        assert_eq!(
            pricing_plan_code(&ledger, &verified, PRICING_ENFORCEMENT_HEIGHT, Some(160)),
            None
        );
        assert_eq!(
            pricing_plan_code(&ledger, &verified, PRICING_ENFORCEMENT_HEIGHT + 1, Some(160)).as_deref(),
            Some("asset_orchard_pricing_nav_stale")
        );
        let mut unknown_age = ledger.clone();
        unknown_age.nav_assets[0].finalized_at_height = 0;
        assert_eq!(
            pricing_plan_code(&unknown_age, &verified, PRICING_ENFORCEMENT_HEIGHT, None).as_deref(),
            Some("asset_orchard_pricing_nav_stale")
        );
        fs::remove_dir_all(fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_pricing_pair_and_mode_are_enforced_from_the_effective_height() {
        let (fixture, ledger, verified) = pricing_enforcement_ledger(true);
        let mut own_quote = verified.clone();
        own_quote.pricing.claim.quote_asset_tag_lo = verified.pricing.claim.base_asset_tag_lo;
        own_quote.pricing.claim.quote_asset_tag_hi = verified.pricing.claim.base_asset_tag_hi;
        let mut at_nav = verified.clone();
        at_nav.pricing.claim.mode = "at_nav".to_string();
        let mut ungoverned = ledger.clone();
        ungoverned.pftl_uniswap_routes.clear();
        let mut reversed_route = ledger.clone();
        reversed_route.pftl_uniswap_routes[0] =
            governed_pricing_route(&fixture.quote_asset_id, &fixture.base_asset_id);
        for height in [PRICING_ENFORCEMENT_HEIGHT, PRICING_ENFORCEMENT_HEIGHT + 1] {
            assert_eq!(pricing_plan_code(&ledger, &verified, height, None), None);
            for (ledger, verified) in [
                (&ledger, &own_quote),
                (&ungoverned, &verified),
                (&reversed_route, &verified),
            ] {
                assert_eq!(
                    pricing_plan_code(ledger, verified, height, None).as_deref(),
                    Some("asset_orchard_pricing_pair_not_governed")
                );
            }
            assert_eq!(
                pricing_plan_code(&ledger, &at_nav, height, None).as_deref(),
                Some("asset_orchard_pricing_mode_mismatch")
            );
        }
        let before = PRICING_ENFORCEMENT_HEIGHT - 1;
        assert_eq!(pricing_plan_code(&ledger, &own_quote, before, None), None);
        assert_eq!(pricing_plan_code(&ungoverned, &verified, before, None), None);
        assert_eq!(pricing_plan_code(&ledger, &at_nav, before, None), None);
        fs::remove_dir_all(fixture.data_dir).expect("cleanup");
    }

    #[test]
    fn asset_orchard_pricing_claim_read_refuses_what_consensus_refuses() {
        let enforced = |route: bool, max_age: Option<u32>| {
            pricing_read_fixture(700, move |ledger, governance| {
                if route {
                    let (base, quote) = (
                        ledger.nav_assets[0].asset_id.clone(),
                        ledger
                            .asset_definitions
                            .iter()
                            .find(|asset| asset.code == "PFUSDC")
                            .expect("quote")
                            .asset_id
                            .clone(),
                    );
                    ledger.pftl_uniswap_routes.push(governed_pricing_route(&base, &quote));
                }
                governance.apply(pricing_enforcement_amendment(
                    GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_ENFORCEMENT_ACTIVATION_HEIGHT,
                    PRICING_ENFORCEMENT_HEIGHT as u32,
                ));
                if let Some(max_age) = max_age {
                    governance.apply(pricing_enforcement_amendment(
                        GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_MAX_NAV_AGE_BLOCKS,
                        max_age,
                    ));
                }
            })
        };
        let consensus_code = |fixture: &PricingReadFixture,
                              claim: &AssetOrchardPricingClaim,
                              height: u64| {
            let store = NodeStore::new(&fixture.data_dir);
            let ledger = store.read_ledger().expect("ledger");
            let governance = store.read_governance().expect("governance");
            let genesis = store.read_genesis().expect("genesis");
            let (_, mut verified) =
                asset_orchard_test_action_and_verified(asset_orchard_test_encrypted_outputs());
            verified.pricing.claim = claim.clone();
            asset_orchard_swap_pricing_plan_with_enforcement(
                &ledger,
                &verified,
                height,
                asset_orchard_pricing_enforcement_at(
                    &asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance),
                    height,
                ),
            )
            .err()
            .map(|error| error.code().to_string())
        };
        let after = PRICING_ENFORCEMENT_HEIGHT + 1;
        let before = PRICING_ENFORCEMENT_HEIGHT - 1;

        let governed = enforced(true, None);
        let report = asset_orchard_pricing_claim(pricing_read_options(&governed, after))
            .expect("governed read");
        assert_eq!(report.status, "ok", "{report:?}");
        assert!(report.pricing_enforced);
        assert_eq!(report.pricing_enforcement_activation_height, Some(PRICING_ENFORCEMENT_HEIGHT));
        assert_eq!(report.governed_route_id.as_deref(), Some("a666-pfusdc-route"));
        let claim = report.pricing_claim.expect("claim");
        assert_eq!(consensus_code(&governed, &claim, after), None);
        fs::remove_dir_all(&governed.data_dir).expect("cleanup");

        // No governed route: the read accepts before activation and refuses
        // after it with the code consensus gives the pre-activation claim.
        let ungoverned = enforced(false, None);
        let early = asset_orchard_pricing_claim(pricing_read_options(&ungoverned, before))
            .expect("pre-activation read");
        assert_eq!(early.status, "ok");
        assert!(!early.pricing_enforced);
        let claim = early.pricing_claim.expect("claim");
        assert_eq!(consensus_code(&ungoverned, &claim, before), None);
        let late = asset_orchard_pricing_claim(pricing_read_options(&ungoverned, after))
            .expect("enforced read");
        assert_eq!(late.status, "mismatch");
        let code = late.refusal.expect("refusal").code;
        assert_eq!(code, "asset_orchard_pricing_pair_not_governed");
        assert_eq!(consensus_code(&ungoverned, &claim, after).as_deref(), Some(code.as_str()));
        let own_quote = AssetOrchardPairReadOptions {
            quote_asset_id: ungoverned.base_asset_id.clone(),
            ..pricing_read_options(&ungoverned, after)
        };
        assert_eq!(
            asset_orchard_pricing_claim(own_quote)
                .expect("own-quote read")
                .refusal
                .expect("refusal")
                .code,
            "asset_orchard_pricing_pair_not_governed"
        );
        fs::remove_dir_all(&ungoverned.data_dir).expect("cleanup");

        // Governed age bound 160: 590 + 160 = 750 is the last fresh height.
        let aged = enforced(true, Some(160));
        let fresh = asset_orchard_pricing_claim(pricing_read_options(&aged, PRICING_ENFORCEMENT_HEIGHT))
            .expect("fresh read");
        assert_eq!(fresh.status, "ok", "{fresh:?}");
        let claim = fresh.pricing_claim.expect("claim");
        let stale = asset_orchard_pricing_claim(pricing_read_options(&aged, after))
            .expect("stale read");
        assert_eq!(stale.status, "stale");
        let code = stale.refusal.expect("refusal").code;
        assert_eq!(code, "asset_orchard_pricing_nav_stale");
        assert_eq!(consensus_code(&aged, &claim, after).as_deref(), Some(code.as_str()));
        fs::remove_dir_all(&aged.data_dir).expect("cleanup");
    }
