    use postfiat_crypto_provider::{
        bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed,
        ml_dsa_65_verify_with_context,
    };

    use super::*;

    fn test_domain() -> CobaltDomain {
        CobaltDomain {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            protocol_version: 1,
        }
    }

    fn registry_entry(node_id: &str, public_key_hex: &str, active: bool) -> ValidatorRegistryEntry {
        ValidatorRegistryEntry {
            node_id: node_id.to_string(),
            algorithm_id: "ML-DSA-65".to_string(),
            public_key_hex: public_key_hex.to_string(),
            active,
        }
    }

    fn root(byte: char) -> String {
        std::iter::repeat_n(byte, 96).collect()
    }

    #[test]
    fn governance_accepts_only_canonical_vault_bridge_route_authority_kinds() {
        assert!(validate_amendment_kind(
            GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT
        )
        .is_ok());
        let canonical = format!(
            "{GOVERNANCE_VAULT_BRIDGE_ROUTE_KIND_PREFIX_V1}:{}:{}",
            root('a'),
            root('b')
        );
        assert!(validate_amendment_kind(&canonical).is_ok());
        assert!(validate_amendment_value(&canonical, 1).is_ok());

        for malformed in [
            format!("{GOVERNANCE_VAULT_BRIDGE_ROUTE_KIND_PREFIX_V1}:{}", root('a')),
            format!(
                "{GOVERNANCE_VAULT_BRIDGE_ROUTE_KIND_PREFIX_V1}:{}:{}",
                root('A'),
                root('b')
            ),
            format!(
                "{GOVERNANCE_VAULT_BRIDGE_ROUTE_KIND_PREFIX_V1}:{}:{}",
                "aa",
                root('b')
            ),
        ] {
            assert!(validate_amendment_kind(&malformed).is_err(), "{malformed}");
        }
    }

    fn validators(count: usize) -> Vec<String> {
        (0..count)
            .map(|index| format!("validator-{index}"))
            .collect()
    }

    #[test]
    fn example_report_path_rejects_escape_and_absolute_without_override() {
        assert!(resolve_example_report_path("../report.json", None, false).is_err());
        assert!(resolve_example_report_path("/tmp/report.json", None, false).is_err());
        assert_eq!(
            resolve_example_report_path("/tmp/report.json", None, true)
                .expect("absolute report path with explicit override"),
            std::path::PathBuf::from("/tmp/report.json")
        );
    }

    #[test]
    fn example_report_path_uses_configured_root_for_relative_reports() {
        let root = std::path::Path::new("/tmp/postfiat-cobalt-reports");

        assert_eq!(
            resolve_example_report_path("scenario/report.json", Some(root), false)
                .expect("rooted relative report path"),
            root.join("scenario/report.json")
        );
        assert!(resolve_example_report_path("/tmp/report.json", Some(root), true).is_err());
    }

    fn subset(domain: &CobaltDomain, members: &[&str], t_s: usize, q_s: usize) -> EssentialSubset {
        build_essential_subset(
            domain,
            members
                .iter()
                .map(|validator| (*validator).to_string())
                .collect(),
            t_s,
            q_s,
            Vec::new(),
            1,
            None,
        )
        .expect("build essential subset")
    }

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn safety_witness_profile() -> CobaltSafetyWitnessProfile {
        CobaltSafetyWitnessProfile {
            byzantine_budget: 2,
            max_cover_subsets: 16,
            require_cleared_challenge_state: true,
        }
    }

    fn safety_witness_input(
        old_graph: &TrustGraph,
        new_graph: &TrustGraph,
        profile: CobaltSafetyWitnessProfile,
    ) -> CobaltSafetyWitnessInput {
        CobaltSafetyWitnessInput {
            previous_registry_root: old_graph.registry_root.clone(),
            new_registry_root: new_graph.registry_root.clone(),
            previous_trust_graph_root: old_graph.trust_graph_root.clone(),
            new_trust_graph_root: new_graph.trust_graph_root.clone(),
            activation_height: new_graph.activation_height,
            challenge_state: COBALT_CHALLENGE_STATE_CLEARED.to_string(),
            profile,
        }
    }

    fn canonical_transition_fixture(
        old_validators: Vec<String>,
        new_validators: Vec<String>,
    ) -> (CobaltDomain, TrustGraph, TrustGraph) {
        let domain = test_domain();
        let old_graph = build_canonical_unl_trust_graph(
            &domain,
            1,
            root('a'),
            10,
            None,
            old_validators,
            5,
        )
        .expect("old trust graph");
        let new_graph = build_canonical_unl_trust_graph(
            &domain,
            2,
            root('b'),
            11,
            Some(old_graph.trust_graph_root.clone()),
            new_validators,
            5,
        )
        .expect("new trust graph");
        (domain, old_graph, new_graph)
    }

    #[test]
    fn cobalt_safety_witness_accepts_bounded_single_rotation() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let report = verify_cobalt_safety_witness(
            &domain,
            &old_graph,
            &new_graph,
            safety_witness_input(&old_graph, &new_graph, safety_witness_profile()),
        )
        .expect("safety witness report");

        assert!(report.accepted);
        assert_eq!(report.reason, "accepted");
        assert_eq!(report.old_cover.len(), 1);
        assert_eq!(report.new_cover.len(), 1);
        assert_eq!(report.intersections.len(), 1);
        assert_eq!(report.intersections[0].intersection_size, 6);
        assert!(report.rejected_counterexamples.is_empty());
        assert_eq!(
            report.report_hash,
            cobalt_safety_witness_report_hash(&report).expect("report hash")
        );
    }

    #[test]
    fn cobalt_safety_witness_rejects_ab_to_hijkl_unsafe_transition() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "H", "I", "J", "K", "L"]),
        );
        let report = verify_cobalt_safety_witness(
            &domain,
            &old_graph,
            &new_graph,
            safety_witness_input(&old_graph, &new_graph, safety_witness_profile()),
        )
        .expect("safety witness report");

        assert!(!report.accepted);
        assert_eq!(report.reason, "old-new intersection bound failed");
        assert_eq!(report.rejected_counterexamples.len(), 1);
        assert_eq!(report.rejected_counterexamples[0].intersection, ids(&["A", "B"]));
        assert_eq!(report.rejected_counterexamples[0].intersection_size, 2);
        assert!(!report.rejected_counterexamples[0].safe);
    }

    #[test]
    fn cobalt_safety_witness_rejects_stale_parent_root() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let mut input = safety_witness_input(&old_graph, &new_graph, safety_witness_profile());
        input.previous_trust_graph_root = root('f');
        let report = verify_cobalt_safety_witness(&domain, &old_graph, &new_graph, input)
            .expect("safety witness report");

        assert!(!report.accepted);
        assert_eq!(report.reason, "previous graph root mismatch");
    }

    #[test]
    fn cobalt_safety_witness_rejects_missing_challenge_clearance() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let mut input = safety_witness_input(&old_graph, &new_graph, safety_witness_profile());
        input.challenge_state = "open".to_string();
        let report = verify_cobalt_safety_witness(&domain, &old_graph, &new_graph, input)
            .expect("safety witness report");

        assert!(!report.accepted);
        assert_eq!(report.reason, "challenge state not cleared");
    }

    #[test]
    fn cobalt_safety_witness_rejects_oversized_cover() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let mut profile = safety_witness_profile();
        profile.max_cover_subsets = 1;
        let report = verify_cobalt_safety_witness(
            &domain,
            &old_graph,
            &new_graph,
            safety_witness_input(&old_graph, &new_graph, profile),
        )
        .expect("safety witness report");

        assert!(!report.accepted);
        assert_eq!(report.reason, "essential subset cover exceeds profile limit");
    }

    #[test]
    fn cobalt_safety_witness_rejects_budget_above_weakest_subset() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let mut profile = safety_witness_profile();
        profile.byzantine_budget = 3;
        let report = verify_cobalt_safety_witness(
            &domain,
            &old_graph,
            &new_graph,
            safety_witness_input(&old_graph, &new_graph, profile),
        )
        .expect("safety witness report");

        assert!(!report.accepted);
        assert_eq!(
            report.reason,
            "byzantine budget exceeds weakest covered subset"
        );
    }

    #[test]
    fn cobalt_cover_extractor_derives_complete_cover_and_hash() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let profile = safety_witness_profile();
        let report = extract_cobalt_safety_cover(&domain, &old_graph, &new_graph, &profile)
            .expect("cover extraction");

        assert!(report.accepted);
        assert!(report.complete);
        assert_eq!(report.reason, "cover extraction complete");
        assert_eq!(report.old_cover.len(), 1);
        assert_eq!(report.new_cover.len(), 1);
        assert_eq!(report.total_cover_subsets, 2);
        assert_eq!(
            report.report_hash,
            cobalt_cover_extraction_report_hash(&report).expect("cover hash")
        );
        verify_cobalt_cover_extraction_report(
            &domain, &old_graph, &new_graph, &profile, &report,
        )
        .expect("cover report verifies");
    }

    #[test]
    fn cobalt_cover_extractor_dedupes_shared_subset_rows() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let report = extract_cobalt_safety_cover(
            &domain,
            &old_graph,
            &new_graph,
            &safety_witness_profile(),
        )
        .expect("cover extraction");

        assert_eq!(old_graph.trust_views.len(), 7);
        assert_eq!(new_graph.trust_views.len(), 7);
        assert_eq!(report.old_cover.len(), 1);
        assert_eq!(report.new_cover.len(), 1);
    }

    #[test]
    fn cobalt_cover_extractor_fails_closed_on_inactive_subset() {
        let domain = test_domain();
        let old_graph = build_canonical_unl_trust_graph(
            &domain,
            1,
            root('a'),
            10,
            None,
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            5,
        )
        .expect("old graph");
        let inactive = build_essential_subset(
            &domain,
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
            2,
            5,
            Vec::new(),
            1,
            Some(10),
        )
        .expect("inactive subset");
        let views = inactive
            .validators
            .iter()
            .map(|validator| {
                build_trust_view(&domain, validator.as_str(), 2, vec![inactive.clone()], "")
                    .expect("trust view")
            })
            .collect();
        let new_graph = build_trust_graph(
            &domain,
            2,
            root('b'),
            11,
            Some(old_graph.trust_graph_root.clone()),
            views,
        )
        .expect("new graph");
        let report = extract_cobalt_safety_cover(
            &domain,
            &old_graph,
            &new_graph,
            &safety_witness_profile(),
        )
        .expect("cover extraction");

        assert!(!report.accepted);
        assert!(!report.complete);
        assert_eq!(
            report.reason,
            "cover extraction requires at least one active subset per graph"
        );
        assert_eq!(report.rejected_subsets.len(), 7);
        assert!(report
            .rejected_subsets
            .iter()
            .all(|row| row.reason == "subset deactivated at or before graph activation"));
    }

    #[test]
    fn cobalt_cover_extractor_rejects_oversized_profile_limit() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let mut profile = safety_witness_profile();
        profile.max_cover_subsets = 1;
        let report = extract_cobalt_safety_cover(&domain, &old_graph, &new_graph, &profile)
            .expect("cover extraction");

        assert!(!report.accepted);
        assert_eq!(report.reason, "essential subset cover exceeds profile limit");
    }

    #[test]
    fn cobalt_cover_extractor_rejects_budget_above_weakest_subset() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let mut profile = safety_witness_profile();
        profile.byzantine_budget = 3;
        let report = extract_cobalt_safety_cover(&domain, &old_graph, &new_graph, &profile)
            .expect("cover extraction");

        assert!(!report.accepted);
        assert_eq!(
            report.reason,
            "byzantine budget exceeds weakest covered subset"
        );
    }

    #[test]
    fn cobalt_cover_extractor_detects_witness_cover_omission() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "C", "D", "E", "F", "H"]),
        );
        let profile = safety_witness_profile();
        let cover_report = extract_cobalt_safety_cover(&domain, &old_graph, &new_graph, &profile)
            .expect("cover extraction");
        let safety_report = verify_cobalt_safety_witness(
            &domain,
            &old_graph,
            &new_graph,
            safety_witness_input(&old_graph, &new_graph, profile),
        )
        .expect("safety witness");
        verify_cobalt_cover_extraction_matches_safety_witness(&cover_report, &safety_report)
            .expect("complete cover matches witness");

        let mut omitted = safety_report.clone();
        omitted.new_cover.clear();
        omitted.report_hash = cobalt_safety_witness_report_hash(&omitted).expect("tampered hash");
        let error = verify_cobalt_cover_extraction_matches_safety_witness(&cover_report, &omitted)
            .expect_err("omitted cover rejected");
        assert_eq!(
            error,
            "Cobalt safety witness does not match extracted complete cover"
        );
    }

    #[test]
    fn cobalt_cover_extractor_completes_before_unsafe_intersection_rejection() {
        let (domain, old_graph, new_graph) = canonical_transition_fixture(
            ids(&["A", "B", "C", "D", "E", "F", "G"]),
            ids(&["A", "B", "H", "I", "J", "K", "L"]),
        );
        let profile = safety_witness_profile();
        let cover_report = extract_cobalt_safety_cover(&domain, &old_graph, &new_graph, &profile)
            .expect("cover extraction");
        let safety_report = verify_cobalt_safety_witness(
            &domain,
            &old_graph,
            &new_graph,
            safety_witness_input(&old_graph, &new_graph, profile),
        )
        .expect("safety witness");

        assert!(cover_report.accepted);
        assert!(!safety_report.accepted);
        assert_eq!(
            safety_report.reason,
            "old-new intersection bound failed"
        );
        verify_cobalt_cover_extraction_matches_safety_witness(&cover_report, &safety_report)
            .expect("unsafe witness still used complete cover");
    }

    fn all_support_subsets(validators: &[String]) -> Vec<Vec<String>> {
        let mut subsets = Vec::new();
        for mask in 0..(1usize << validators.len()) {
            let mut subset = Vec::new();
            for (index, validator) in validators.iter().enumerate() {
                if (mask & (1usize << index)) != 0 {
                    subset.push(validator.clone());
                }
            }
            subsets.push(subset);
        }
        subsets
    }

    fn sign_rbc_payload(payload: &[u8], seed: u8) -> String {
        let key = ml_dsa_65_keygen_from_seed(&[seed; 32]);
        let signature = ml_dsa_65_sign_with_context_seed(
            &key.private_key,
            payload,
            RBC_MESSAGE_SIGNATURE_CONTEXT,
            &[seed.wrapping_add(1); 32],
        )
        .expect("sign RBC payload");
        assert!(ml_dsa_65_verify_with_context(
            &key.public_key,
            payload,
            &signature,
            RBC_MESSAGE_SIGNATURE_CONTEXT,
        ));
        bytes_to_hex(&signature)
    }

    fn sign_abba_payload(payload: &[u8], seed: u8) -> String {
        let key = ml_dsa_65_keygen_from_seed(&[seed; 32]);
        let signature = ml_dsa_65_sign_with_context_seed(
            &key.private_key,
            payload,
            ABBA_MESSAGE_SIGNATURE_CONTEXT,
            &[seed.wrapping_add(1); 32],
        )
        .expect("sign ABBA payload");
        assert!(ml_dsa_65_verify_with_context(
            &key.public_key,
            payload,
            &signature,
            ABBA_MESSAGE_SIGNATURE_CONTEXT,
        ));
        bytes_to_hex(&signature)
    }

    fn nonuniform_certificate_fixture() -> (
        CobaltDomain,
        TrustGraph,
        LinkageReport,
        CobaltProposal,
        Vec<String>,
    ) {
        let domain = test_domain();
        let validator_ids = validators(7);
        let all = subset(
            &domain,
            &[
                "validator-0",
                "validator-1",
                "validator-2",
                "validator-3",
                "validator-4",
                "validator-5",
                "validator-6",
            ],
            2,
            5,
        );
        let first_five = subset(
            &domain,
            &[
                "validator-0",
                "validator-1",
                "validator-2",
                "validator-3",
                "validator-4",
            ],
            1,
            4,
        );
        let last_five = subset(
            &domain,
            &[
                "validator-2",
                "validator-3",
                "validator-4",
                "validator-5",
                "validator-6",
            ],
            1,
            4,
        );
        let views = vec![
            build_trust_view(&domain, "validator-0", 1, vec![all.clone()], "").expect("view 0"),
            build_trust_view(&domain, "validator-1", 1, vec![all.clone(), first_five], "")
                .expect("view 1"),
            build_trust_view(&domain, "validator-2", 1, vec![all.clone(), last_five], "")
                .expect("view 2"),
            build_trust_view(&domain, "validator-3", 1, vec![all.clone()], "").expect("view 3"),
            build_trust_view(&domain, "validator-4", 1, vec![all.clone()], "").expect("view 4"),
            build_trust_view(&domain, "validator-5", 1, vec![all.clone()], "").expect("view 5"),
            build_trust_view(&domain, "validator-6", 1, vec![all], "").expect("view 6"),
        ];
        let graph = build_trust_graph(&domain, 2, root('b'), 7, None, views).expect("trust graph");
        let linkage_report =
            analyze_trust_graph(&domain, &graph, &CobaltFaultModel::default()).expect("linkage");
        assert!(linkage_report.unsafe_pairs.is_empty());
        let proposal = propose_nonuniform_governance_amendment(
            &domain,
            &graph,
            GOVERNANCE_KIND_CRYPTO_POLICY,
            2,
        )
        .expect("proposal");
        let support = validator_ids.into_iter().take(5).collect();
        (domain, graph, linkage_report, proposal, support)
    }

    fn dabc_two_amendment_chain_fixture() -> (
        CobaltDomain,
        TrustGraph,
        DabcRatifiedAmendment,
        DabcRatifiedAmendment,
    ) {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let view_1 = trust_view_for_validator(&graph, "validator-1").expect("view 1");
        let view_2 = trust_view_for_validator(&graph, "validator-2").expect("view 2");

        let propose_a = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            11,
            root('a'),
            "",
        )
        .expect("propose a");
        let accept_a = build_rbc_accept(&domain, &propose_a, "validator-1", "").expect("accept a");
        let candidate_a =
            mvba_candidate_from_rbc_accept(&domain, &propose_a, &accept_a).expect("candidate a");
        let set_a = build_mvba_valid_input_set(
            &domain,
            view_1,
            hash_hex("postfiat.test.mvba.agreement", b"dabc-slot-11"),
            vec![candidate_a],
        )
        .expect("set a");
        let first = ratify_dabc_amendment(&domain, &graph, &set_a, None, 20).expect("first");

        let propose_b = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-3",
            12,
            root('c'),
            "",
        )
        .expect("propose b");
        let accept_b = build_rbc_accept(&domain, &propose_b, "validator-2", "").expect("accept b");
        let candidate_b =
            mvba_candidate_from_rbc_accept(&domain, &propose_b, &accept_b).expect("candidate b");
        let set_b = build_mvba_valid_input_set(
            &domain,
            view_2,
            hash_hex("postfiat.test.mvba.agreement", b"dabc-slot-12"),
            vec![candidate_b],
        )
        .expect("set b");
        let second =
            ratify_dabc_amendment(&domain, &graph, &set_b, Some(&first), 21).expect("second");

        (domain, graph, first, second)
    }

    fn dabc_full_knowledge_checkpoint_fixture(
        domain: &CobaltDomain,
        graph: &TrustGraph,
        first: &DabcRatifiedAmendment,
        second: &DabcRatifiedAmendment,
    ) -> DabcFullKnowledgeCheckpoint {
        let support = validators(7).into_iter().take(5).collect::<Vec<_>>();
        let mut checks = Vec::new();
        for height in [10_u64, 20_u64] {
            for sender in &support {
                let pending_pairs = if height == 20 {
                    vec![DabcPendingPair {
                        amendment_slot: second.amendment_slot,
                        output_candidate_id: second.output_candidate_id.clone(),
                    }]
                } else {
                    vec![DabcPendingPair {
                        amendment_slot: first.amendment_slot,
                        output_candidate_id: first.output_candidate_id.clone(),
                    }]
                };
                checks.push(
                    build_dabc_full_knowledge_check(
                        domain,
                        graph.trust_graph_root.clone(),
                        sender,
                        height,
                        pending_pairs,
                        "",
                    )
                    .expect("check"),
                );
            }
        }
        build_dabc_full_knowledge_checkpoint(
            domain,
            graph,
            "validator-1",
            10,
            second.activation_height,
            checks,
        )
        .expect("checkpoint")
    }

    #[test]
    fn builds_nonuniform_trust_graph_and_linkage_report() {
        let domain = test_domain();
        let all = subset(
            &domain,
            &[
                "validator-0",
                "validator-1",
                "validator-2",
                "validator-3",
                "validator-4",
            ],
            1,
            4,
        );
        let abcd = subset(
            &domain,
            &["validator-0", "validator-1", "validator-2", "validator-3"],
            1,
            3,
        );
        let abce = subset(
            &domain,
            &["validator-0", "validator-1", "validator-2", "validator-4"],
            1,
            3,
        );
        let abde = subset(
            &domain,
            &["validator-0", "validator-1", "validator-3", "validator-4"],
            1,
            3,
        );

        let views = vec![
            build_trust_view(&domain, "validator-0", 1, vec![all.clone()], "").expect("view 0"),
            build_trust_view(
                &domain,
                "validator-1",
                1,
                vec![all.clone(), abcd.clone()],
                "",
            )
            .expect("view 1"),
            build_trust_view(&domain, "validator-2", 1, vec![all.clone(), abce], "")
                .expect("view 2"),
            build_trust_view(&domain, "validator-3", 1, vec![all.clone(), abde], "")
                .expect("view 3"),
            build_trust_view(&domain, "validator-4", 1, vec![all], "").expect("view 4"),
        ];
        assert_ne!(views[0].trust_view_id, views[1].trust_view_id);
        assert_eq!(views[1].derived_unl, validators(5));

        let graph =
            build_trust_graph(&domain, 1, root('a'), 7, None, views).expect("build trust graph");
        assert!(is_lower_hex_len(&graph.trust_graph_root, 96));
        assert_eq!(graph.trust_views.len(), 5);

        let report =
            analyze_trust_graph(&domain, &graph, &CobaltFaultModel::default()).expect("linkage");
        assert_eq!(report.trust_view_count, 5);
        assert_eq!(report.linked_pairs.len(), 10);
        assert_eq!(report.fully_linked_pairs.len(), 10);
        assert!(report.unsafe_pairs.is_empty());
        assert_eq!(report.weakly_connected_validators, validators(5));
        assert_eq!(report.strongly_connected_validators, validators(5));
        assert!(is_lower_hex_len(&report.report_hash, 96));

        let mut tampered = graph.clone();
        tampered.trust_views[0].view_version += 1;
        let error = validate_trust_graph(&domain, &tampered)
            .expect_err("tampered view should break trust graph validation");
        assert!(
            error.contains("trust view id mismatch") || error.contains("trust graph root mismatch"),
            "{error}"
        );
    }

    #[test]
    fn canonical_unl_trust_graph_g0_reproduces_canonical_validator_set() {
        let domain = test_domain();
        let canonical_validators = validators(7);
        let graph = build_canonical_unl_trust_graph(
            &domain,
            1,
            root('a'),
            11,
            None,
            canonical_validators.clone(),
            5,
        )
        .expect("canonical G0 trust graph");
        assert_eq!(graph.graph_version, 1);
        assert_eq!(graph.activation_height, 11);
        assert_eq!(graph.trust_views.len(), canonical_validators.len());
        for view in &graph.trust_views {
            assert_eq!(view.derived_unl, canonical_validators);
            assert_eq!(view.essential_subsets.len(), 1);
            let subset = &view.essential_subsets[0];
            assert_eq!(subset.validators, canonical_validators);
            assert_eq!(subset.validator_count, canonical_validators.len());
            assert_eq!(subset.max_active_byzantine, 2);
            assert_eq!(subset.quorum, 5);
            assert_eq!(subset.activation_height, graph.activation_height);
        }

        let report =
            analyze_trust_graph(&domain, &graph, &CobaltFaultModel::default()).expect("linkage");
        assert!(report.unsafe_pairs.is_empty());
        assert_eq!(report.weakly_connected_validators, canonical_validators);
        assert_eq!(report.strongly_connected_validators, canonical_validators);

        let duplicate_error = build_canonical_unl_trust_graph(
            &domain,
            1,
            root('a'),
            11,
            None,
            vec![
                "validator-0".to_string(),
                "validator-0".to_string(),
                "validator-1".to_string(),
            ],
            2,
        )
        .expect_err("duplicate validator should fail");
        assert!(
            duplicate_error.contains("sorted unique"),
            "{duplicate_error}"
        );

        let quorum_error =
            build_canonical_unl_trust_graph(&domain, 1, root('a'), 11, None, validators(4), 2)
                .expect_err("unsafe canonical quorum should fail Cobalt subset math");
        assert!(
            quorum_error.contains("t_S < 2q_S - n_S") || quorum_error.contains("2t_S < q_S"),
            "{quorum_error}"
        );
    }

    #[test]
    fn validates_essential_subset_math_from_cobalt_paper() {
        let domain = test_domain();
        build_essential_subset(
            &domain,
            vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
                "validator-3".to_string(),
            ],
            1,
            3,
            Vec::new(),
            1,
            None,
        )
        .expect("valid paper-style 3f+1 subset");

        let error = build_essential_subset(
            &domain,
            vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
            ],
            1,
            2,
            Vec::new(),
            1,
            None,
        )
        .expect_err("invalid t_S < 2q_S - n_S should fail");
        assert!(error.contains("t_S < 2q_S - n_S"), "{error}");

        let error = build_essential_subset(
            &domain,
            vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
                "validator-3".to_string(),
                "validator-4".to_string(),
            ],
            2,
            4,
            Vec::new(),
            1,
            None,
        )
        .expect_err("invalid 2t_S < q_S should fail");
        assert!(error.contains("2t_S < q_S"), "{error}");
    }

    #[test]
    fn evaluates_strong_and_weak_support_against_local_view() {
        let domain = test_domain();
        let all = subset(
            &domain,
            &[
                "validator-0",
                "validator-1",
                "validator-2",
                "validator-3",
                "validator-4",
            ],
            1,
            4,
        );
        let abc = subset(
            &domain,
            &["validator-0", "validator-1", "validator-2"],
            0,
            2,
        );
        let view =
            build_trust_view(&domain, "validator-0", 1, vec![all, abc], "").expect("trust view");

        let strong = vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
            "validator-2".to_string(),
            "validator-3".to_string(),
        ];
        assert!(has_strong_support(&view, &strong).expect("strong support"));
        assert!(has_weak_support(&view, &strong).expect("weak support"));

        let weak_only = vec!["validator-0".to_string(), "validator-1".to_string()];
        assert!(!has_strong_support(&view, &weak_only).expect("not strong"));
        assert!(has_weak_support(&view, &weak_only).expect("weak"));

        let unsorted = vec!["validator-1".to_string(), "validator-0".to_string()];
        assert!(has_strong_support(&view, &unsorted).is_err());
    }

    #[test]
    fn verifies_nonuniform_governance_certificate_against_local_views() {
        let (domain, graph, linkage_report, proposal, support) = nonuniform_certificate_fixture();

        let validator_0_certificate = certify_nonuniform_governance_amendment(
            &domain,
            &graph,
            &linkage_report,
            "validator-0",
            &proposal,
            support.clone(),
            7,
        )
        .expect("validator-0 non-uniform certificate");
        let validator_1_certificate = certify_nonuniform_governance_amendment(
            &domain,
            &graph,
            &linkage_report,
            "validator-1",
            &proposal,
            support.clone(),
            7,
        )
        .expect("validator-1 non-uniform certificate");

        assert_eq!(validator_0_certificate.registry_root, graph.registry_root);
        assert_eq!(
            validator_0_certificate.trust_graph_root,
            graph.trust_graph_root
        );
        assert_ne!(
            validator_0_certificate.trust_view_id,
            validator_1_certificate.trust_view_id
        );
        assert_eq!(validator_0_certificate.proposal_id, proposal.proposal_id);
        assert_eq!(validator_0_certificate.support, support);
        assert_eq!(validator_0_certificate.satisfied_subsets.len(), 1);
        assert_eq!(validator_1_certificate.satisfied_subsets.len(), 2);
        assert!(is_lower_hex_len(
            &validator_0_certificate.certificate_id,
            96
        ));

        verify_nonuniform_governance_certificate(
            &domain,
            &graph,
            &linkage_report,
            &proposal,
            &validator_0_certificate,
            7,
        )
        .expect("verify validator-0 certificate");
        verify_nonuniform_governance_certificate(
            &domain,
            &graph,
            &linkage_report,
            &proposal,
            &validator_1_certificate,
            7,
        )
        .expect("verify validator-1 certificate");
    }

    #[test]
    fn rejects_nonuniform_certificate_for_wrong_local_view() {
        let (domain, graph, linkage_report, proposal, support) = nonuniform_certificate_fixture();
        certify_nonuniform_governance_amendment(
            &domain,
            &graph,
            &linkage_report,
            "validator-0",
            &proposal,
            support.clone(),
            7,
        )
        .expect("validator-0 certificate");

        let wrong_view_error = certify_nonuniform_governance_amendment(
            &domain,
            &graph,
            &linkage_report,
            "validator-2",
            &proposal,
            support.clone(),
            7,
        )
        .expect_err("support should not satisfy validator-2 local view");
        assert!(
            wrong_view_error.contains("does not satisfy local trust view"),
            "{wrong_view_error}"
        );

        let mut stale_view_certificate = certify_nonuniform_governance_amendment(
            &domain,
            &graph,
            &linkage_report,
            "validator-0",
            &proposal,
            support,
            7,
        )
        .expect("validator-0 certificate");
        stale_view_certificate.trust_view_id = graph.trust_views[1].trust_view_id.clone();
        let stale_view_error = verify_nonuniform_governance_certificate(
            &domain,
            &graph,
            &linkage_report,
            &proposal,
            &stale_view_certificate,
            7,
        )
        .expect_err("stale trust view id should fail");
        assert!(
            stale_view_error.contains("stale trust view id"),
            "{stale_view_error}"
        );
    }

    #[test]
    fn rejects_nonuniform_certificate_with_inactive_or_stale_graph_evidence() {
        let (domain, graph, linkage_report, proposal, support) = nonuniform_certificate_fixture();
        let certificate = certify_nonuniform_governance_amendment(
            &domain,
            &graph,
            &linkage_report,
            "validator-0",
            &proposal,
            support,
            7,
        )
        .expect("non-uniform certificate");

        let inactive_error = verify_nonuniform_governance_certificate(
            &domain,
            &graph,
            &linkage_report,
            &proposal,
            &certificate,
            6,
        )
        .expect_err("inactive graph should fail");
        assert!(
            inactive_error.contains("trust graph is not active"),
            "{inactive_error}"
        );

        let mut stale_root = certificate.clone();
        stale_root.trust_graph_root = root('c');
        let stale_root_error = verify_nonuniform_governance_certificate(
            &domain,
            &graph,
            &linkage_report,
            &proposal,
            &stale_root,
            7,
        )
        .expect_err("stale graph root should fail");
        assert!(
            stale_root_error.contains("trust graph root mismatch"),
            "{stale_root_error}"
        );

        let mut stale_linkage = linkage_report.clone();
        stale_linkage.report_hash = root('d');
        let stale_linkage_error = verify_nonuniform_governance_certificate(
            &domain,
            &graph,
            &stale_linkage,
            &proposal,
            &certificate,
            7,
        )
        .expect_err("stale linkage report should fail");
        assert!(
            stale_linkage_error.contains("linkage report hash mismatch"),
            "{stale_linkage_error}"
        );

        let mut forged_linkage = linkage_report.clone();
        forged_linkage.weakly_connected_validators.pop();
        forged_linkage.report_hash = linkage_report_hash(LinkageReportHashInput {
            domain: &domain,
            graph: &graph,
            actively_byzantine: &forged_linkage.actively_byzantine,
            linked_pairs: &forged_linkage.linked_pairs,
            fully_linked_pairs: &forged_linkage.fully_linked_pairs,
            unsafe_pairs: &forged_linkage.unsafe_pairs,
            weakly_connected_validators: &forged_linkage.weakly_connected_validators,
            strongly_connected_validators: &forged_linkage.strongly_connected_validators,
            connectivity: &forged_linkage.connectivity,
        })
        .expect("forged linkage hash");
        let forged_linkage_error = verify_nonuniform_governance_certificate(
            &domain,
            &graph,
            &forged_linkage,
            &proposal,
            &certificate,
            7,
        )
        .expect_err("forged linkage report should fail");
        assert!(
            forged_linkage_error.contains("does not match trust graph"),
            "{forged_linkage_error}"
        );
    }

    #[test]
    fn rbc_messages_have_canonical_ids_and_signed_payloads() {
        let domain = test_domain();
        let trust_graph_root = root('e');
        let payload_hash = root('f');
        let unsigned_propose = build_rbc_propose(
            &domain,
            trust_graph_root.clone(),
            "validator-0",
            9,
            payload_hash.clone(),
            "",
        )
        .expect("unsigned propose");
        let propose_payload =
            rbc_propose_signing_payload_bytes(&unsigned_propose).expect("propose payload");
        let propose_signature = sign_rbc_payload(&propose_payload, 11);
        let propose = build_rbc_propose(
            &domain,
            trust_graph_root.clone(),
            "validator-0",
            9,
            payload_hash.clone(),
            propose_signature.clone(),
        )
        .expect("signed propose");
        assert_eq!(propose.message_id, unsigned_propose.message_id);
        assert_eq!(
            propose.message_id,
            rbc_propose_message_id(&propose).expect("propose id")
        );
        assert_eq!(propose.signature_hex, propose_signature);
        validate_rbc_propose(&domain, &propose).expect("validate propose");

        let unsigned_echo =
            build_rbc_echo(&domain, &propose, "validator-1", "").expect("unsigned echo");
        let echo_signature = sign_rbc_payload(
            &rbc_echo_signing_payload_bytes(&unsigned_echo).expect("echo payload"),
            12,
        );
        let echo = build_rbc_echo(&domain, &propose, "validator-1", echo_signature).expect("echo");
        assert_eq!(echo.message_id, unsigned_echo.message_id);
        validate_rbc_echo(&domain, &echo, &propose).expect("validate echo");

        let unsigned_ready =
            build_rbc_ready(&domain, &propose, "validator-2", "").expect("unsigned ready");
        let ready_signature = sign_rbc_payload(
            &rbc_ready_signing_payload_bytes(&unsigned_ready).expect("ready payload"),
            13,
        );
        let ready =
            build_rbc_ready(&domain, &propose, "validator-2", ready_signature).expect("ready");
        assert_eq!(ready.message_id, unsigned_ready.message_id);
        validate_rbc_ready(&domain, &ready, &propose).expect("validate ready");

        let unsigned_accept =
            build_rbc_accept(&domain, &propose, "validator-3", "").expect("unsigned accept");
        let accept_signature = sign_rbc_payload(
            &rbc_accept_signing_payload_bytes(&unsigned_accept).expect("accept payload"),
            14,
        );
        let accept =
            build_rbc_accept(&domain, &propose, "validator-3", accept_signature).expect("accept");
        assert_eq!(accept.message_id, unsigned_accept.message_id);
        validate_rbc_accept(&domain, &accept, &propose).expect("validate accept");

        assert_ne!(propose.message_id, echo.message_id);
        assert_ne!(echo.message_id, ready.message_id);
        assert_ne!(ready.message_id, accept.message_id);
    }

    #[test]
    fn rbc_validation_rejects_tampered_bindings() {
        let domain = test_domain();
        let propose = build_rbc_propose(&domain, root('e'), "validator-0", 9, root('f'), "")
            .expect("propose");
        let echo = build_rbc_echo(&domain, &propose, "validator-1", "").expect("echo");

        let mut bad_propose = propose.clone();
        bad_propose.message_id = root('a');
        let bad_propose_error =
            validate_rbc_propose(&domain, &bad_propose).expect_err("bad propose id should fail");
        assert!(
            bad_propose_error.contains("message id mismatch"),
            "{bad_propose_error}"
        );

        let mut bad_echo = echo.clone();
        bad_echo.payload_hash = root('b');
        let bad_echo_error = validate_rbc_echo(&domain, &bad_echo, &propose)
            .expect_err("echo payload mismatch should fail");
        assert!(
            bad_echo_error.contains("does not match RBC propose"),
            "{bad_echo_error}"
        );

        let mut bad_signature = echo;
        bad_signature.signature_hex = "ABCD".to_string();
        let bad_signature_error = validate_rbc_echo(&domain, &bad_signature, &propose)
            .expect_err("uppercase signature should fail");
        assert!(
            bad_signature_error.contains("signature must be lowercase hex"),
            "{bad_signature_error}"
        );
    }

    #[test]
    fn evaluates_rbc_echo_and_ready_support_against_local_view() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let propose = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            9,
            root('f'),
            "",
        )
        .expect("propose");
        let first_five = validators(7).into_iter().take(5).collect::<Vec<_>>();
        let echo_messages = first_five
            .iter()
            .map(|sender| build_rbc_echo(&domain, &propose, sender, "").expect("echo"))
            .collect::<Vec<_>>();
        let validator_1_view = trust_view_for_validator(&graph, "validator-1").expect("view 1");
        let echo_evaluation =
            evaluate_rbc_echo_support(&domain, validator_1_view, &propose, &echo_messages)
                .expect("echo support");
        assert!(echo_evaluation.weak_support);
        assert!(echo_evaluation.strong_support);
        assert_eq!(echo_evaluation.strong_satisfied_subsets.len(), 2);
        assert!(rbc_ready_allowed_from_echo(&echo_evaluation));

        let weak_ready_messages = ["validator-0", "validator-1"]
            .into_iter()
            .map(|sender| build_rbc_ready(&domain, &propose, sender, "").expect("ready"))
            .collect::<Vec<_>>();
        let weak_ready_evaluation =
            evaluate_rbc_ready_support(&domain, validator_1_view, &propose, &weak_ready_messages)
                .expect("weak ready support");
        assert!(weak_ready_evaluation.weak_support);
        assert!(!weak_ready_evaluation.strong_support);
        assert!(rbc_ready_allowed_from_ready(&weak_ready_evaluation));
        assert!(!rbc_accept_allowed_from_ready(&weak_ready_evaluation));

        let strong_ready_messages = first_five
            .iter()
            .map(|sender| build_rbc_ready(&domain, &propose, sender, "").expect("ready"))
            .collect::<Vec<_>>();
        let strong_ready_evaluation =
            evaluate_rbc_ready_support(&domain, validator_1_view, &propose, &strong_ready_messages)
                .expect("strong ready support");
        assert!(strong_ready_evaluation.strong_support);
        assert!(rbc_accept_allowed_from_ready(&strong_ready_evaluation));

        let validator_2_view = trust_view_for_validator(&graph, "validator-2").expect("view 2");
        let wrong_view_evaluation =
            evaluate_rbc_ready_support(&domain, validator_2_view, &propose, &weak_ready_messages)
                .expect("wrong view ready support");
        assert!(!wrong_view_evaluation.weak_support);
        assert!(!wrong_view_evaluation.strong_support);
        assert!(!rbc_ready_allowed_from_ready(&wrong_view_evaluation));
    }

    #[test]
    fn rbc_nonidentical_trust_views_accept_one_payload() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let propose = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            15,
            root('f'),
            "",
        )
        .expect("propose");
        let validator_ids = validators(7);
        let echo_messages = validator_ids
            .iter()
            .map(|sender| build_rbc_echo(&domain, &propose, sender, "").expect("echo"))
            .collect::<Vec<_>>();
        let ready_messages = validator_ids
            .iter()
            .map(|sender| build_rbc_ready(&domain, &propose, sender, "").expect("ready"))
            .collect::<Vec<_>>();
        let accept_messages = validator_ids
            .iter()
            .map(|sender| build_rbc_accept(&domain, &propose, sender, "").expect("accept"))
            .collect::<Vec<_>>();

        let mut accepted_by = Vec::new();
        let mut distinct_trust_views = BTreeSet::new();
        for view in &graph.trust_views {
            distinct_trust_views.insert(view.trust_view_id.clone());
            let echo_evaluation =
                evaluate_rbc_echo_support(&domain, view, &propose, &echo_messages)
                    .expect("echo support");
            assert!(echo_evaluation.weak_support);
            assert!(echo_evaluation.strong_support);
            assert!(rbc_ready_allowed_from_echo(&echo_evaluation));

            let ready_evaluation =
                evaluate_rbc_ready_support(&domain, view, &propose, &ready_messages)
                    .expect("ready support");
            assert!(ready_evaluation.weak_support);
            assert!(ready_evaluation.strong_support);
            assert!(rbc_accept_allowed_from_ready(&ready_evaluation));

            let local_accept = accept_messages
                .iter()
                .find(|message| message.sender == view.validator)
                .expect("local accept");
            validate_rbc_accept(&domain, local_accept, &propose).expect("validate local accept");
            accepted_by.push(view.validator.clone());
        }

        assert_eq!(accepted_by, validator_ids);
        assert!(distinct_trust_views.len() >= 3);
        for pair in accept_messages.windows(2) {
            let no_conflict = detect_rbc_conflicting_accept(
                &domain, &graph, &propose, &pair[0], &propose, &pair[1],
            )
            .expect("same payload conflict check");
            assert!(no_conflict.is_none());
        }
    }

    #[test]
    fn detects_conflicting_rbc_accepts_from_linked_validators() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let left_propose = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            9,
            root('f'),
            "",
        )
        .expect("left propose");
        let right_propose = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            9,
            root('d'),
            "",
        )
        .expect("right propose");
        let left_accept =
            build_rbc_accept(&domain, &left_propose, "validator-1", "").expect("left accept");
        let right_accept =
            build_rbc_accept(&domain, &right_propose, "validator-2", "").expect("right accept");

        let evidence = detect_rbc_conflicting_accept(
            &domain,
            &graph,
            &left_propose,
            &left_accept,
            &right_propose,
            &right_accept,
        )
        .expect("conflict detection")
        .expect("linked conflict evidence");
        assert_eq!(evidence.trust_graph_root, graph.trust_graph_root);
        assert_eq!(evidence.amendment_slot, 9);
        assert_eq!(evidence.proposer, "validator-0");
        assert_eq!(evidence.left_sender, "validator-1");
        assert_eq!(evidence.right_sender, "validator-2");
        assert_ne!(evidence.left_payload_hash, evidence.right_payload_hash);
        assert!(evidence.linked);
        assert!(evidence.fully_linked);
        assert!(is_lower_hex_len(&evidence.evidence_id, 96));

        let same_payload_accept =
            build_rbc_accept(&domain, &left_propose, "validator-2", "").expect("same accept");
        let no_conflict = detect_rbc_conflicting_accept(
            &domain,
            &graph,
            &left_propose,
            &left_accept,
            &left_propose,
            &same_payload_accept,
        )
        .expect("same payload check");
        assert!(no_conflict.is_none());
    }

    #[test]
    fn abba_messages_have_round_state_and_signed_payloads() {
        let domain = test_domain();
        let trust_graph_root = root('e');
        let agreement_id = hash_hex("postfiat.test.abba.agreement", b"agreement-1");
        let round_state = build_abba_round_state(trust_graph_root.clone(), agreement_id.clone(), 3)
            .expect("round state");
        assert_eq!(round_state.round, 3);
        assert!(round_state.init_messages.is_empty());

        let unsigned_init = build_abba_init(
            &domain,
            trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            3,
            true,
            "",
        )
        .expect("unsigned init");
        let init_signature = sign_abba_payload(
            &abba_init_signing_payload_bytes(&unsigned_init).expect("init payload"),
            21,
        );
        let init = build_abba_init(
            &domain,
            trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            3,
            true,
            init_signature,
        )
        .expect("init");
        assert_eq!(init.message_id, unsigned_init.message_id);
        validate_abba_init(&domain, &init).expect("validate init");

        let aux = build_abba_aux(
            &domain,
            trust_graph_root.clone(),
            "validator-1",
            agreement_id.clone(),
            3,
            true,
            sign_abba_payload(
                &abba_aux_signing_payload_bytes(
                    &build_abba_aux(
                        &domain,
                        trust_graph_root.clone(),
                        "validator-1",
                        agreement_id.clone(),
                        3,
                        true,
                        "",
                    )
                    .expect("unsigned aux"),
                )
                .expect("aux payload"),
                22,
            ),
        )
        .expect("aux");
        validate_abba_aux(&domain, &aux).expect("validate aux");

        let conf = build_abba_conf(
            &domain,
            trust_graph_root.clone(),
            "validator-2",
            agreement_id.clone(),
            3,
            false,
            "",
        )
        .expect("conf");
        validate_abba_conf(&domain, &conf).expect("validate conf");

        let finish = build_abba_finish(
            &domain,
            trust_graph_root,
            "validator-3",
            agreement_id,
            3,
            false,
            "",
        )
        .expect("finish");
        validate_abba_finish(&domain, &finish).expect("validate finish");

        assert_ne!(init.message_id, aux.message_id);
        assert_ne!(aux.message_id, conf.message_id);
        assert_ne!(conf.message_id, finish.message_id);
    }

    #[test]
    fn abba_validation_rejects_bad_round_and_tampered_id() {
        let domain = test_domain();
        let trust_graph_root = root('e');
        let agreement_id = hash_hex("postfiat.test.abba.agreement", b"agreement-2");
        let bad_round = build_abba_init(
            &domain,
            trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            0,
            true,
            "",
        )
        .expect_err("zero round should fail");
        assert!(bad_round.contains("round must be nonzero"), "{bad_round}");

        let mut init = build_abba_init(
            &domain,
            trust_graph_root,
            "validator-0",
            agreement_id,
            1,
            true,
            "",
        )
        .expect("init");
        init.message_id = root('a');
        let tamper_error = validate_abba_init(&domain, &init).expect_err("tampered id should fail");
        assert!(
            tamper_error.contains("message id mismatch"),
            "{tamper_error}"
        );
    }

    #[test]
    fn evaluates_abba_support_and_finish_consistency_against_local_view() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let agreement_id = hash_hex("postfiat.test.abba.agreement", b"agreement-3");
        let first_five = validators(7).into_iter().take(5).collect::<Vec<_>>();
        let aux_messages = first_five
            .iter()
            .map(|sender| {
                build_abba_aux(
                    &domain,
                    graph.trust_graph_root.clone(),
                    sender,
                    agreement_id.clone(),
                    1,
                    true,
                    "",
                )
                .expect("aux")
            })
            .collect::<Vec<_>>();
        let validator_1_view = trust_view_for_validator(&graph, "validator-1").expect("view 1");
        let aux_evaluation = evaluate_abba_aux_support(
            &domain,
            validator_1_view,
            &agreement_id,
            1,
            true,
            &aux_messages,
        )
        .expect("aux support");
        assert!(abba_weak_support(&aux_evaluation));
        assert!(abba_strong_support(&aux_evaluation));
        assert_eq!(aux_evaluation.strong_satisfied_subsets.len(), 2);

        let weak_conf_messages = ["validator-0", "validator-1"]
            .into_iter()
            .map(|sender| {
                build_abba_conf(
                    &domain,
                    graph.trust_graph_root.clone(),
                    sender,
                    agreement_id.clone(),
                    1,
                    true,
                    "",
                )
                .expect("conf")
            })
            .collect::<Vec<_>>();
        let weak_conf = evaluate_abba_conf_support(
            &domain,
            validator_1_view,
            &agreement_id,
            1,
            true,
            &weak_conf_messages,
        )
        .expect("weak conf");
        assert!(abba_weak_support(&weak_conf));
        assert!(!abba_strong_support(&weak_conf));

        let validator_2_view = trust_view_for_validator(&graph, "validator-2").expect("view 2");
        let wrong_view_conf = evaluate_abba_conf_support(
            &domain,
            validator_2_view,
            &agreement_id,
            1,
            true,
            &weak_conf_messages,
        )
        .expect("wrong view conf");
        assert!(!wrong_view_conf.weak_support);
        assert!(!wrong_view_conf.strong_support);

        let finish_true = build_abba_finish(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-1",
            agreement_id.clone(),
            1,
            true,
            "",
        )
        .expect("finish true");
        let finish_false = build_abba_finish(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-2",
            agreement_id.clone(),
            1,
            false,
            "",
        )
        .expect("finish false");
        let evidence = detect_abba_conflicting_finish(&domain, &graph, &finish_true, &finish_false)
            .expect("finish conflict")
            .expect("linked finish conflict evidence");
        assert_eq!(evidence.agreement_id, agreement_id);
        assert_eq!(evidence.round, 1);
        assert!(evidence.linked);
        assert!(evidence.fully_linked);
        assert_ne!(evidence.left_value, evidence.right_value);
        assert!(is_lower_hex_len(&evidence.evidence_id, 96));

        let same_finish = build_abba_finish(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-2",
            evidence.agreement_id,
            1,
            true,
            "",
        )
        .expect("same finish");
        assert!(
            detect_abba_conflicting_finish(&domain, &graph, &finish_true, &same_finish)
                .expect("same finish check")
                .is_none()
        );
    }

    #[test]
    fn detects_abba_byzantine_equivocation_without_divergent_decision() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let agreement_id = hash_hex("postfiat.test.abba.agreement", b"agreement-equivocation");
        let mut round_state =
            build_abba_round_state(graph.trust_graph_root.clone(), agreement_id.clone(), 1)
                .expect("round state");

        let init_true = build_abba_init(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            1,
            true,
            "",
        )
        .expect("init true");
        let init_false = build_abba_init(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            1,
            false,
            "",
        )
        .expect("init false");
        let aux_true = build_abba_aux(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            1,
            true,
            "",
        )
        .expect("aux true");
        let aux_false = build_abba_aux(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            1,
            false,
            "",
        )
        .expect("aux false");
        let conf_true = build_abba_conf(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            1,
            true,
            "",
        )
        .expect("conf true");
        let conf_false = build_abba_conf(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            1,
            false,
            "",
        )
        .expect("conf false");
        let finish_false = build_abba_finish(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            agreement_id.clone(),
            1,
            false,
            "",
        )
        .expect("finish false");
        let finish_true_messages = [
            "validator-0",
            "validator-1",
            "validator-2",
            "validator-3",
            "validator-4",
            "validator-5",
        ]
        .into_iter()
        .map(|sender| {
            build_abba_finish(
                &domain,
                graph.trust_graph_root.clone(),
                sender,
                agreement_id.clone(),
                1,
                true,
                "",
            )
            .expect("finish true")
        })
        .collect::<Vec<_>>();

        round_state.init_messages = vec![init_true.clone(), init_false.clone()];
        round_state.aux_messages = vec![aux_true.clone(), aux_false.clone()];
        round_state.conf_messages = vec![conf_true.clone(), conf_false.clone()];
        round_state.finish_messages = finish_true_messages.clone();
        round_state.finish_messages.push(finish_false.clone());

        let init_evidence = detect_abba_init_equivocation(&domain, &init_true, &init_false)
            .expect("init equivocation")
            .expect("init equivocation evidence");
        assert_eq!(init_evidence.message_kind, "init");
        assert_eq!(init_evidence.sender, "validator-0");
        assert!(!init_evidence.left_value);
        assert!(init_evidence.right_value);
        assert!(is_lower_hex_len(&init_evidence.evidence_id, 96));

        let finish_evidence =
            detect_abba_finish_equivocation(&domain, &finish_true_messages[0], &finish_false)
                .expect("finish equivocation")
                .expect("finish equivocation evidence");
        assert_eq!(finish_evidence.message_kind, "finish");
        assert_eq!(finish_evidence.sender, "validator-0");
        assert_ne!(
            finish_evidence.left_message_id,
            finish_evidence.right_message_id
        );

        let all_evidence =
            detect_abba_round_equivocations(&domain, &round_state).expect("round equivocations");
        let kinds = all_evidence
            .iter()
            .map(|evidence| evidence.message_kind.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            kinds,
            BTreeSet::from([
                "aux".to_string(),
                "conf".to_string(),
                "finish".to_string(),
                "init".to_string()
            ])
        );
        assert_eq!(all_evidence.len(), 4);
        assert!(all_evidence
            .iter()
            .all(|evidence| evidence.sender == "validator-0"));

        let mut tampered_state = round_state.clone();
        tampered_state.finish_messages[0].message_id = root('c');
        let tamper_error = detect_abba_round_equivocations(&domain, &tampered_state)
            .expect_err("tampered round-state message should fail");
        assert!(
            tamper_error.contains("message id mismatch"),
            "{tamper_error}"
        );

        let validator_1_view = trust_view_for_validator(&graph, "validator-1").expect("view 1");
        let true_finish = evaluate_abba_finish_support(
            &domain,
            validator_1_view,
            &agreement_id,
            1,
            true,
            &round_state.finish_messages,
        )
        .expect("true finish support");
        let false_finish = evaluate_abba_finish_support(
            &domain,
            validator_1_view,
            &agreement_id,
            1,
            false,
            &round_state.finish_messages,
        )
        .expect("false finish support");
        assert!(abba_strong_support(&true_finish));
        assert_eq!(
            true_finish.support,
            vec![
                "validator-1".to_string(),
                "validator-2".to_string(),
                "validator-3".to_string(),
                "validator-4".to_string(),
                "validator-5".to_string()
            ]
        );
        assert!(!abba_weak_support(&false_finish));
        assert!(false_finish.support.is_empty());

        let no_evidence =
            detect_abba_aux_equivocation(&domain, &aux_true, &aux_true).expect("same aux");
        assert!(no_evidence.is_none());
    }

    #[test]
    fn deterministic_abba_test_crs_is_simulation_only() {
        let domain = test_domain();
        let agreement_id = hash_hex("postfiat.test.abba.agreement", b"agreement-4");
        let source = AbbaCommonRandomSource::DeterministicTest {
            seed_hex: "11".repeat(32),
        };
        let first_coin = abba_common_coin(
            &domain,
            &agreement_id,
            1,
            &source,
            CobaltRuntimeMode::Simulation,
        )
        .expect("simulation coin");
        let second_coin = abba_common_coin(
            &domain,
            &agreement_id,
            1,
            &source,
            CobaltRuntimeMode::Simulation,
        )
        .expect("deterministic simulation coin");
        assert_eq!(first_coin, second_coin);

        let live_error =
            abba_common_coin(&domain, &agreement_id, 1, &source, CobaltRuntimeMode::Live)
                .expect_err("deterministic test CRS must fail live mode");
        assert!(
            live_error.contains("cannot be used in live mode"),
            "{live_error}"
        );

        let beacon = AbbaCommonRandomSource::SignedBeacon {
            beacon_id: root('a'),
            output_hash: root('c'),
        };
        assert!(
            !abba_common_coin(&domain, &agreement_id, 1, &beacon, CobaltRuntimeMode::Live)
                .expect("live signed beacon coin")
        );
    }

    #[test]
    fn mvba_valid_input_selection_is_deterministic_across_linked_views() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let agreement_id = hash_hex("postfiat.test.mvba.agreement", b"agreement-5");
        let propose_a = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            11,
            root('a'),
            "",
        )
        .expect("propose a");
        let accept_a = build_rbc_accept(&domain, &propose_a, "validator-1", "").expect("accept a");
        let candidate_a =
            mvba_candidate_from_rbc_accept(&domain, &propose_a, &accept_a).expect("candidate a");

        let propose_b = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            11,
            root('b'),
            "",
        )
        .expect("propose b");
        let accept_b = build_rbc_accept(&domain, &propose_b, "validator-2", "").expect("accept b");
        let candidate_b =
            mvba_candidate_from_rbc_accept(&domain, &propose_b, &accept_b).expect("candidate b");
        assert_ne!(candidate_a.candidate_id, candidate_b.candidate_id);

        let view_1 = trust_view_for_validator(&graph, "validator-1").expect("view 1");
        let view_2 = trust_view_for_validator(&graph, "validator-2").expect("view 2");
        let set_1 = build_mvba_valid_input_set(
            &domain,
            view_1,
            agreement_id.clone(),
            vec![candidate_b.clone(), candidate_a.clone()],
        )
        .expect("set 1");
        let set_2 = build_mvba_valid_input_set(
            &domain,
            view_2,
            agreement_id,
            vec![candidate_a.clone(), candidate_b.clone()],
        )
        .expect("set 2");
        assert_eq!(set_1.output_candidate_id, set_2.output_candidate_id);
        assert_eq!(
            mvba_output_candidate(&set_1).expect("output 1"),
            mvba_output_candidate(&set_2).expect("output 2")
        );

        let mut tampered = candidate_a;
        tampered.payload_hash = root('c');
        let tamper_error = build_mvba_valid_input_set(&domain, view_1, root('d'), vec![tampered])
            .expect_err("tampered candidate should fail");
        assert!(
            tamper_error.contains("candidate propose message id mismatch"),
            "{tamper_error}"
        );
    }

    #[test]
    fn mvba_valid_input_set_rejects_candidate_flood_before_dedup() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let view_1 = trust_view_for_validator(&graph, "validator-1").expect("view 1");
        let propose = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            11,
            root('a'),
            "",
        )
        .expect("propose");
        let accept = build_rbc_accept(&domain, &propose, "validator-1", "").expect("accept");
        let candidate =
            mvba_candidate_from_rbc_accept(&domain, &propose, &accept).expect("candidate");
        let flooded = vec![candidate; MAX_MVBA_CANDIDATES_PER_SET + 1];
        let error = build_mvba_valid_input_set(&domain, view_1, root('d'), flooded)
            .expect_err("candidate flood should fail");
        assert!(error.contains("too many candidates"), "{error}");
    }

    #[test]
    fn ratifies_dabc_amendments_as_linear_parent_hash_chain() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let view_1 = trust_view_for_validator(&graph, "validator-1").expect("view 1");
        let view_2 = trust_view_for_validator(&graph, "validator-2").expect("view 2");

        let propose_a = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            11,
            root('a'),
            "",
        )
        .expect("propose a");
        let accept_a = build_rbc_accept(&domain, &propose_a, "validator-1", "").expect("accept a");
        let candidate_a =
            mvba_candidate_from_rbc_accept(&domain, &propose_a, &accept_a).expect("candidate a");
        let set_a = build_mvba_valid_input_set(
            &domain,
            view_1,
            hash_hex("postfiat.test.mvba.agreement", b"dabc-slot-11"),
            vec![candidate_a],
        )
        .expect("set a");
        let first = ratify_dabc_amendment(&domain, &graph, &set_a, None, 20).expect("first");

        assert_eq!(first.sequence, 1);
        assert_eq!(first.parent_ratification_id, dabc_genesis_parent_id());
        assert_eq!(first.amendment_slot, 11);
        assert_eq!(first.candidate.proposer, "validator-0");
        assert!(is_lower_hex_len(&first.ratification_id, 96));
        validate_dabc_ratified_amendment(&domain, &graph, &first, None).expect("validate first");

        let propose_b = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-3",
            12,
            root('c'),
            "",
        )
        .expect("propose b");
        let accept_b = build_rbc_accept(&domain, &propose_b, "validator-2", "").expect("accept b");
        let candidate_b =
            mvba_candidate_from_rbc_accept(&domain, &propose_b, &accept_b).expect("candidate b");
        let set_b = build_mvba_valid_input_set(
            &domain,
            view_2,
            hash_hex("postfiat.test.mvba.agreement", b"dabc-slot-12"),
            vec![candidate_b],
        )
        .expect("set b");
        let second =
            ratify_dabc_amendment(&domain, &graph, &set_b, Some(&first), 21).expect("second");

        assert_eq!(second.sequence, 2);
        assert_eq!(second.parent_ratification_id, first.ratification_id);
        assert_eq!(second.amendment_slot, 12);
        assert_eq!(second.candidate.proposer, "validator-3");
        assert_ne!(first.candidate.proposer, second.candidate.proposer);
        assert!(is_lower_hex_len(&second.ratification_id, 96));
        validate_dabc_ratified_amendment(&domain, &graph, &second, Some(&first))
            .expect("validate second");

        let propose_c = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-4",
            13,
            root('e'),
            "",
        )
        .expect("propose c");
        let accept_c = build_rbc_accept(&domain, &propose_c, "validator-5", "").expect("accept c");
        let candidate_c =
            mvba_candidate_from_rbc_accept(&domain, &propose_c, &accept_c).expect("candidate c");
        let set_c = build_mvba_valid_input_set(
            &domain,
            view_1,
            hash_hex("postfiat.test.mvba.agreement", b"dabc-slot-13"),
            vec![candidate_c],
        )
        .expect("set c");
        let third =
            ratify_dabc_amendment(&domain, &graph, &set_c, Some(&second), 22).expect("third");
        assert_eq!(third.sequence, 3);
        assert_eq!(third.parent_ratification_id, second.ratification_id);
        assert_eq!(third.amendment_slot, 13);
        validate_dabc_ratified_amendment(&domain, &graph, &third, Some(&second))
            .expect("validate third");

        let mut tampered_parent = second.clone();
        tampered_parent.parent_ratification_id = root('d');
        tampered_parent.ratification_id =
            dabc_ratification_id(&domain, &tampered_parent).expect("tampered id");
        let parent_error =
            validate_dabc_ratified_amendment(&domain, &graph, &tampered_parent, Some(&first))
                .expect_err("tampered parent should fail");
        assert!(parent_error.contains("parent mismatch"), "{parent_error}");

        let mut tampered_sequence = second.clone();
        tampered_sequence.sequence = 3;
        tampered_sequence.ratification_id =
            dabc_ratification_id(&domain, &tampered_sequence).expect("tampered id");
        let sequence_error =
            validate_dabc_ratified_amendment(&domain, &graph, &tampered_sequence, Some(&first))
                .expect_err("tampered sequence should fail");
        assert!(
            sequence_error.contains("sequence must extend previous"),
            "{sequence_error}"
        );
    }

    #[test]
    fn full_knowledge_checkpoint_gates_dabc_activation() {
        let (domain, graph, first, second) = dabc_two_amendment_chain_fixture();
        let ratified_chain = vec![first.clone(), second.clone()];
        let support = validators(7).into_iter().take(5).collect::<Vec<_>>();

        let mut checks = Vec::new();
        for height in [10_u64, 20_u64] {
            for sender in &support {
                let pending_pairs = if height == 20 {
                    vec![DabcPendingPair {
                        amendment_slot: second.amendment_slot,
                        output_candidate_id: second.output_candidate_id.clone(),
                    }]
                } else {
                    vec![DabcPendingPair {
                        amendment_slot: first.amendment_slot,
                        output_candidate_id: first.output_candidate_id.clone(),
                    }]
                };
                checks.push(
                    build_dabc_full_knowledge_check(
                        &domain,
                        graph.trust_graph_root.clone(),
                        sender,
                        height,
                        pending_pairs,
                        "",
                    )
                    .expect("check"),
                );
            }
        }
        let checkpoint = build_dabc_full_knowledge_checkpoint(
            &domain,
            &graph,
            "validator-1",
            10,
            second.activation_height,
            checks,
        )
        .expect("checkpoint");
        assert_eq!(checkpoint.covered_heights, vec![10, 20]);

        let activation = validate_dabc_activation_with_full_knowledge(
            &domain,
            &graph,
            &ratified_chain,
            &second,
            &checkpoint,
        )
        .expect("activation evidence");
        assert_eq!(activation.ratification_id, second.ratification_id);
        assert_eq!(activation.checkpoint_id, checkpoint.checkpoint_id);
        assert_eq!(activation.activation_height, second.activation_height);
        assert!(is_lower_hex_len(&activation.activation_id, 96));

        let mut incomplete_checks = Vec::new();
        for height in [10_u64, 20_u64] {
            for sender in support.iter().take(4) {
                incomplete_checks.push(
                    build_dabc_full_knowledge_check(
                        &domain,
                        graph.trust_graph_root.clone(),
                        sender,
                        height,
                        Vec::new(),
                        "",
                    )
                    .expect("incomplete check"),
                );
            }
        }
        let incomplete_error = build_dabc_full_knowledge_checkpoint(
            &domain,
            &graph,
            "validator-1",
            10,
            second.activation_height,
            incomplete_checks,
        )
        .expect_err("incomplete checkpoint should fail");
        assert!(
            incomplete_error.contains("lacks strong support"),
            "{incomplete_error}"
        );

        let early_checks = support
            .iter()
            .map(|sender| {
                build_dabc_full_knowledge_check(
                    &domain,
                    graph.trust_graph_root.clone(),
                    sender,
                    10,
                    Vec::new(),
                    "",
                )
                .expect("early check")
            })
            .collect::<Vec<_>>();
        let early_checkpoint = build_dabc_full_knowledge_checkpoint(
            &domain,
            &graph,
            "validator-1",
            10,
            second.activation_height - 2,
            early_checks,
        )
        .expect("early checkpoint");
        let early_error = validate_dabc_activation_with_full_knowledge(
            &domain,
            &graph,
            &ratified_chain,
            &second,
            &early_checkpoint,
        )
        .expect_err("early checkpoint should fail activation");
        assert!(
            early_error.contains("before activation height"),
            "{early_error}"
        );

        let mut unresolved_checks = Vec::new();
        for height in [10_u64, 20_u64] {
            for sender in &support {
                let pending_pairs = if height == 20 {
                    vec![DabcPendingPair {
                        amendment_slot: 99,
                        output_candidate_id: root('e'),
                    }]
                } else {
                    Vec::new()
                };
                unresolved_checks.push(
                    build_dabc_full_knowledge_check(
                        &domain,
                        graph.trust_graph_root.clone(),
                        sender,
                        height,
                        pending_pairs,
                        "",
                    )
                    .expect("unresolved check"),
                );
            }
        }
        let unresolved_checkpoint = build_dabc_full_knowledge_checkpoint(
            &domain,
            &graph,
            "validator-1",
            10,
            second.activation_height,
            unresolved_checks,
        )
        .expect("unresolved checkpoint");
        let unresolved_error = validate_dabc_activation_with_full_knowledge(
            &domain,
            &graph,
            &ratified_chain,
            &second,
            &unresolved_checkpoint,
        )
        .expect_err("unresolved pending slot should fail activation");
        assert!(
            unresolved_error.contains("pending slot 99 is not ratified"),
            "{unresolved_error}"
        );
    }

    #[test]
    fn dabc_replay_bundle_verifies_order_and_activation_heights() {
        let (domain, graph, first, second) = dabc_two_amendment_chain_fixture();
        let checkpoint = dabc_full_knowledge_checkpoint_fixture(&domain, &graph, &first, &second);
        let ratified_chain = vec![first.clone(), second.clone()];
        let first_activation = validate_dabc_activation_with_full_knowledge(
            &domain,
            &graph,
            &ratified_chain,
            &first,
            &checkpoint,
        )
        .expect("first activation");
        let second_activation = validate_dabc_activation_with_full_knowledge(
            &domain,
            &graph,
            &ratified_chain,
            &second,
            &checkpoint,
        )
        .expect("second activation");

        let bundle = build_dabc_replay_bundle(
            &domain,
            &graph,
            ratified_chain.clone(),
            vec![checkpoint.clone()],
            vec![second_activation.clone(), first_activation.clone()],
        )
        .expect("replay bundle");
        let report = verify_dabc_replay_bundle(&domain, &graph, &bundle).expect("replay report");
        assert_eq!(report.ratified_count, 2);
        assert_eq!(report.activation_count, 2);
        assert_eq!(report.checkpoint_count, 1);
        assert_eq!(report.highest_sequence, 2);
        assert_eq!(report.highest_activation_height, second.activation_height);
        assert_eq!(
            report.ratification_ids,
            vec![
                first.ratification_id.clone(),
                second.ratification_id.clone()
            ]
        );
        assert!(is_lower_hex_len(&bundle.bundle_id, 96));

        let unordered_error = build_dabc_replay_bundle(
            &domain,
            &graph,
            vec![second.clone(), first.clone()],
            vec![checkpoint.clone()],
            vec![first_activation.clone(), second_activation.clone()],
        )
        .expect_err("unordered chain should fail");
        assert!(
            unordered_error.contains("sorted by sequence")
                || unordered_error.contains("first ratified amendment sequence"),
            "{unordered_error}"
        );

        let missing_activation_error = build_dabc_replay_bundle(
            &domain,
            &graph,
            ratified_chain,
            vec![checkpoint],
            vec![second_activation],
        )
        .expect_err("missing activation should fail");
        assert!(
            missing_activation_error.contains("missing activation evidence"),
            "{missing_activation_error}"
        );
    }

    #[test]
    fn reports_unsafe_nonuniform_graph_with_counterexample_pair() {
        let domain = test_domain();
        let view_0 = build_trust_view(
            &domain,
            "validator-0",
            1,
            vec![subset(&domain, &["validator-0"], 0, 1)],
            "",
        )
        .expect("view 0");
        let view_1 = build_trust_view(
            &domain,
            "validator-1",
            1,
            vec![subset(&domain, &["validator-1"], 0, 1)],
            "",
        )
        .expect("view 1");
        let graph =
            build_trust_graph(&domain, 1, root('b'), 1, None, vec![view_0, view_1]).expect("graph");
        let report =
            analyze_trust_graph(&domain, &graph, &CobaltFaultModel::default()).expect("report");

        assert!(report.linked_pairs.is_empty());
        assert!(report.fully_linked_pairs.is_empty());
        assert_eq!(report.unsafe_pairs.len(), 1);
        assert_eq!(report.unsafe_pairs[0].left, "validator-0");
        assert_eq!(report.unsafe_pairs[0].right, "validator-1");
        assert!(report.unsafe_pairs[0]
            .reason
            .contains("no shared essential subset"));
    }

    #[test]
    fn linked_views_cannot_have_disjoint_strong_support_outside_fault_bound() {
        let domain = test_domain();
        let shared = subset(
            &domain,
            &[
                "validator-0",
                "validator-1",
                "validator-2",
                "validator-3",
                "validator-4",
            ],
            1,
            4,
        );
        let left =
            build_trust_view(&domain, "validator-0", 1, vec![shared.clone()], "").expect("left");
        let right = build_trust_view(&domain, "validator-1", 1, vec![shared], "").expect("right");
        let active_faults = BTreeSet::from(["validator-4"]);
        assert!(linked_shared_subset(&left, &right, &active_faults).is_some());

        for left_support in all_support_subsets(&left.derived_unl) {
            if !has_strong_support(&left, &left_support).expect("left support") {
                continue;
            }
            for right_support in all_support_subsets(&right.derived_unl) {
                if !has_strong_support(&right, &right_support).expect("right support") {
                    continue;
                }
                let left_set: BTreeSet<&str> = left_support.iter().map(String::as_str).collect();
                let right_set: BTreeSet<&str> = right_support.iter().map(String::as_str).collect();
                let honest_intersection = left_set
                    .intersection(&right_set)
                    .filter(|validator| !active_faults.contains(**validator))
                    .count();
                assert!(
                    honest_intersection > 0,
                    "linked strong supports must share at least one honest validator: left={left_support:?} right={right_support:?}"
                );
            }
        }
    }

    #[test]
    fn ratifies_with_full_essential_subset_support() {
        let domain = test_domain();
        let config = EssentialSubsetConfig::all_of(vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
            "validator-2".to_string(),
        ]);
        let amendment = ratify_validator_set_amendment(
            &domain,
            &config,
            3,
            vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
            ],
        )
        .expect("ratify");

        assert_eq!(amendment.kind, "validator_set");
        assert_eq!(amendment.value, 3);
        assert_eq!(amendment.chain_id, domain.chain_id);
        assert_eq!(amendment.genesis_hash, domain.genesis_hash);
        assert_eq!(amendment.protocol_version, domain.protocol_version);
        assert_eq!(amendment.support.len(), 3);
        assert_eq!(amendment.validators, config.validators.clone());
        assert_eq!(amendment.quorum, config.quorum);
        assert_eq!(amendment.votes.len(), 3);
        assert!(!amendment.instance_id.is_empty());
        assert!(!amendment.proposal_id.is_empty());
        assert!(!amendment.certificate_id.is_empty());
        verify_governance_amendment(&domain, &amendment).expect("verify amendment evidence");
    }

    #[test]
    fn ratifies_crypto_policy_and_bridge_witness_epoch() {
        let domain = test_domain();
        let config = EssentialSubsetConfig::all_of(vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
        ]);

        let crypto_policy = ratify_governance_amendment(
            &domain,
            &config,
            GOVERNANCE_KIND_CRYPTO_POLICY,
            2,
            vec!["validator-0".to_string(), "validator-1".to_string()],
        )
        .expect("ratify crypto policy");
        assert_eq!(crypto_policy.kind, GOVERNANCE_KIND_CRYPTO_POLICY);
        assert_eq!(crypto_policy.value, 2);
        verify_governance_amendment(&domain, &crypto_policy).expect("verify crypto policy");

        let bridge_epoch = ratify_governance_amendment(
            &domain,
            &config,
            GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH,
            7,
            vec!["validator-0".to_string(), "validator-1".to_string()],
        )
        .expect("ratify bridge witness epoch");
        assert_eq!(bridge_epoch.kind, GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH);
        assert_eq!(bridge_epoch.value, 7);
        verify_governance_amendment(&domain, &bridge_epoch).expect("verify bridge epoch");

        assert_ne!(crypto_policy.instance_id, bridge_epoch.instance_id);
        assert_ne!(crypto_policy.amendment_id, bridge_epoch.amendment_id);
    }

    #[test]
    fn fastswap_bootstrap_governance_kind_is_exact_hash_bound() {
        let domain = test_domain();
        let config = EssentialSubsetConfig::all_of(vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
        ]);
        let support = vec!["validator-0".to_string(), "validator-1".to_string()];
        let kind = format!(
            "{}{}",
            FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1,
            "ab".repeat(48)
        );
        let amendment = ratify_governance_amendment(
            &domain,
            &config,
            &kind,
            FASTSWAP_SCHEMA_VERSION_V1,
            support.clone(),
        )
        .expect("ratify FastSwap bootstrap binding");
        verify_governance_amendment(&domain, &amendment)
            .expect("verify FastSwap bootstrap binding");

        assert!(ratify_governance_amendment(
            &domain,
            &config,
            &format!(
                "{}{}",
                FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1,
                "AB".repeat(48)
            ),
            FASTSWAP_SCHEMA_VERSION_V1,
            support.clone(),
        )
        .is_err());
        assert!(ratify_governance_amendment(
            &domain,
            &config,
            &format!(
                "{}{}",
                FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1,
                "ab".repeat(47)
            ),
            FASTSWAP_SCHEMA_VERSION_V1,
            support.clone(),
        )
        .is_err());
        assert!(ratify_governance_amendment(
            &domain,
            &config,
            &kind,
            FASTSWAP_SCHEMA_VERSION_V1 + 1,
            support,
        )
        .is_err());

        let fastpay_kind = format!(
            "{}{}",
            FASTPAY_RECOVERY_GOVERNANCE_KIND_PREFIX_V1,
            "cd".repeat(48)
        );
        let fastpay = ratify_governance_amendment(
            &domain,
            &config,
            &fastpay_kind,
            FASTPAY_RECOVERY_GOVERNANCE_VERSION_V1,
            vec!["validator-0".to_string(), "validator-1".to_string()],
        )
        .expect("ratify FastPay recovery payload binding");
        verify_governance_amendment(&domain, &fastpay)
            .expect("verify FastPay recovery payload binding");
        assert!(ratify_governance_amendment(
            &domain,
            &config,
            &format!(
                "{}{}",
                FASTPAY_RECOVERY_GOVERNANCE_KIND_PREFIX_V1,
                "CD".repeat(48)
            ),
            FASTPAY_RECOVERY_GOVERNANCE_VERSION_V1,
            vec!["validator-0".to_string(), "validator-1".to_string()],
        )
        .is_err());
        assert!(ratify_governance_amendment(
            &domain,
            &config,
            &fastpay_kind,
            FASTPAY_RECOVERY_GOVERNANCE_VERSION_V1 + 1,
            vec!["validator-0".to_string(), "validator-1".to_string()],
        )
        .is_err());
    }

    #[test]
    fn ratifies_atomic_swap_activation_and_boolean_pause_values() {
        let domain = test_domain();
        let config = EssentialSubsetConfig::all_of(vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
        ]);
        let support = vec!["validator-0".to_string(), "validator-1".to_string()];

        let activation = ratify_governance_amendment(
            &domain,
            &config,
            GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT,
            512,
            support.clone(),
        )
        .expect("ratify atomic swap activation");
        verify_governance_amendment(&domain, &activation).expect("verify activation");
        for value in [0, 1] {
            let pause = ratify_governance_amendment(
                &domain,
                &config,
                GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE,
                value,
                support.clone(),
            )
            .expect("ratify atomic swap pause value");
            verify_governance_amendment(&domain, &pause).expect("verify pause");
        }
        assert!(ratify_governance_amendment(
            &domain,
            &config,
            GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE,
            2,
            support.clone(),
        )
        .is_err());
        assert!(ratify_governance_amendment(
            &domain,
            &config,
            GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT,
            0,
            support,
        )
        .is_err());
    }

    #[test]
    fn governance_amendment_lifecycle_metadata_is_bound() {
        let domain = test_domain();
        let config = EssentialSubsetConfig::all_of(vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
        ]);
        let support = vec!["validator-0".to_string(), "validator-1".to_string()];
        let lifecycle = GovernanceAmendmentLifecycle {
            activation_height: 8,
            veto_until_height: 7,
            paused: false,
        };
        let delayed = ratify_governance_amendment_with_lifecycle(
            &domain,
            &config,
            GOVERNANCE_KIND_CRYPTO_POLICY,
            3,
            support.clone(),
            lifecycle,
        )
        .expect("ratify delayed amendment");
        assert_eq!(delayed.activation_height, 8);
        assert_eq!(delayed.veto_until_height, 7);
        assert!(!delayed.paused);
        verify_governance_amendment(&domain, &delayed).expect("verify delayed amendment");

        let immediate = ratify_governance_amendment(
            &domain,
            &config,
            GOVERNANCE_KIND_CRYPTO_POLICY,
            3,
            support.clone(),
        )
        .expect("ratify immediate amendment");
        assert_ne!(delayed.instance_id, immediate.instance_id);
        assert_ne!(delayed.amendment_id, immediate.amendment_id);

        let mut tampered = delayed.clone();
        tampered.activation_height += 1;
        let error = verify_governance_amendment(&domain, &tampered)
            .expect_err("tampered activation height should fail");
        assert!(error.contains("governance amendment instance mismatch"));

        let paused = ratify_governance_amendment_with_lifecycle(
            &domain,
            &config,
            GOVERNANCE_KIND_CRYPTO_POLICY,
            4,
            support.clone(),
            GovernanceAmendmentLifecycle {
                activation_height: 10,
                veto_until_height: 9,
                paused: true,
            },
        )
        .expect("ratify paused amendment with lifecycle metadata");
        assert!(paused.paused);
        verify_governance_amendment(&domain, &paused).expect("verify paused amendment");

        assert!(ratify_governance_amendment_with_lifecycle(
            &domain,
            &config,
            GOVERNANCE_KIND_CRYPTO_POLICY,
            5,
            support,
            GovernanceAmendmentLifecycle {
                activation_height: 0,
                veto_until_height: 0,
                paused: true,
            },
        )
        .is_err());
    }

    #[test]
    fn rejects_insufficient_support() {
        let config = EssentialSubsetConfig::all_of(vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
        ]);
        let error = ratify_validator_set_amendment(
            &test_domain(),
            &config,
            2,
            vec!["validator-0".to_string()],
        )
        .expect_err("should reject");

        assert!(error.contains("insufficient support"));
    }

    #[test]
    fn certificate_tracks_sorted_unique_votes() {
        let domain = test_domain();
        let config = EssentialSubsetConfig::all_of(vec![
            "validator-1".to_string(),
            "validator-0".to_string(),
            "validator-1".to_string(),
        ]);
        let (_proposal, certificate, support) = certify_validator_set(
            &domain,
            &config,
            2,
            vec![
                "validator-1".to_string(),
                "outsider".to_string(),
                "validator-0".to_string(),
                "validator-1".to_string(),
            ],
        )
        .expect("certify");

        assert_eq!(
            support,
            vec!["validator-0".to_string(), "validator-1".to_string()]
        );
        assert_eq!(certificate.votes.len(), 2);
        assert_eq!(certificate.votes[0].validator, "validator-0");
        assert_eq!(certificate.votes[1].validator, "validator-1");
        assert_eq!(certificate.chain_id, domain.chain_id);
        assert_eq!(certificate.genesis_hash, domain.genesis_hash);
        assert_eq!(certificate.protocol_version, domain.protocol_version);
        assert!(!certificate.certificate_id.is_empty());
    }

    #[test]
    fn domain_changes_cobalt_ids() {
        let domain = test_domain();
        let mut other_domain = domain.clone();
        other_domain.genesis_hash = "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111".to_string();
        let config = EssentialSubsetConfig::all_of(vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
        ]);

        let (proposal, certificate, support) = certify_validator_set(
            &domain,
            &config,
            2,
            vec!["validator-0".to_string(), "validator-1".to_string()],
        )
        .expect("certify original");
        let (other_proposal, other_certificate, _) = certify_validator_set(
            &other_domain,
            &config,
            2,
            vec!["validator-0".to_string(), "validator-1".to_string()],
        )
        .expect("certify other domain");

        assert_ne!(proposal.instance_id, other_proposal.instance_id);
        assert_ne!(proposal.proposal_id, other_proposal.proposal_id);
        assert_ne!(
            certificate.votes[0].vote_id,
            other_certificate.votes[0].vote_id
        );
        assert_ne!(certificate.certificate_id, other_certificate.certificate_id);
        assert_ne!(
            governance_amendment_id(
                &domain,
                &proposal.instance_id,
                &certificate.certificate_id,
                "validator_set",
                2,
                &support
            )
            .expect("original amendment id"),
            governance_amendment_id(
                &other_domain,
                &proposal.instance_id,
                &certificate.certificate_id,
                "validator_set",
                2,
                &support
            )
            .expect("other amendment id")
        );
    }

    #[test]
    fn rejects_malformed_cobalt_domain() {
        let config = EssentialSubsetConfig::all_of(vec!["validator-0".to_string()]);
        let valid = ratify_validator_set_amendment(
            &test_domain(),
            &config,
            1,
            vec!["validator-0".to_string()],
        )
        .expect("valid amendment");

        let mut empty_chain = test_domain();
        empty_chain.chain_id = " ".to_string();
        assert!(ratify_validator_set_amendment(
            &empty_chain,
            &config,
            1,
            vec!["validator-0".to_string()]
        )
        .is_err());
        assert!(verify_governance_amendment(&empty_chain, &valid).is_err());
        assert!(governance_amendment_id(
            &empty_chain,
            &valid.instance_id,
            &valid.certificate_id,
            &valid.kind,
            valid.value,
            &valid.support,
        )
        .is_err());

        let mut empty_genesis = test_domain();
        empty_genesis.genesis_hash.clear();
        assert!(
            certify_validator_set(&empty_genesis, &config, 1, vec!["validator-0".to_string()])
                .is_err()
        );

        let mut malformed_genesis = test_domain();
        malformed_genesis.genesis_hash = "not-a-genesis-hash".to_string();
        assert!(certify_validator_set(
            &malformed_genesis,
            &config,
            1,
            vec!["validator-0".to_string()]
        )
        .is_err());

        let mut zero_protocol = test_domain();
        zero_protocol.protocol_version = 0;
        assert!(
            certify_validator_set(&zero_protocol, &config, 1, vec!["validator-0".to_string()])
                .is_err()
        );
    }

    #[test]
    fn rejects_malformed_essential_subset_config() {
        let domain = test_domain();
        let support = vec!["validator-0".to_string()];
        let cases = [
            EssentialSubsetConfig {
                validators: Vec::new(),
                quorum: 0,
            },
            EssentialSubsetConfig {
                validators: vec!["validator-0".to_string()],
                quorum: 0,
            },
            EssentialSubsetConfig {
                validators: vec!["validator-0".to_string()],
                quorum: 2,
            },
            EssentialSubsetConfig {
                validators: vec!["validator-1".to_string(), "validator-0".to_string()],
                quorum: 1,
            },
            EssentialSubsetConfig {
                validators: vec!["validator-0".to_string(), "validator-0".to_string()],
                quorum: 1,
            },
            EssentialSubsetConfig {
                validators: vec![" ".to_string()],
                quorum: 1,
            },
        ];

        for config in cases {
            assert!(
                ratify_validator_set_amendment(&domain, &config, 1, support.clone()).is_err(),
                "malformed config should fail: {config:?}"
            );
        }
    }

    #[test]
    fn verifies_certificate_backed_amendment_and_rejects_tampering() {
        let domain = test_domain();
        let config = EssentialSubsetConfig::all_of(vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
        ]);
        let amendment = ratify_validator_set_amendment(
            &domain,
            &config,
            2,
            vec!["validator-0".to_string(), "validator-1".to_string()],
        )
        .expect("ratify");

        verify_governance_amendment(&domain, &amendment).expect("verify original amendment");

        let mut bad_vote = amendment.clone();
        bad_vote.votes[0].vote_id = "tampered-vote".to_string();
        let vote_error = verify_governance_amendment(&domain, &bad_vote)
            .expect_err("tampered vote id should fail");
        assert!(vote_error.contains("vote id mismatch"), "{vote_error}");

        let mut bad_certificate = amendment.clone();
        bad_certificate.certificate_id = "tampered-certificate".to_string();
        let certificate_error = verify_governance_amendment(&domain, &bad_certificate)
            .expect_err("tampered certificate should fail");
        assert!(
            certificate_error.contains("certificate mismatch"),
            "{certificate_error}"
        );

        let mut bad_support = amendment;
        bad_support.support.reverse();
        let support_error = verify_governance_amendment(&domain, &bad_support)
            .expect_err("unsorted support should fail");
        assert!(
            support_error.contains("support must be sorted unique"),
            "{support_error}"
        );
    }

    #[test]
    fn nonuniform_mode_rejects_canonical_governance_amendments() {
        let domain = test_domain();
        let config = EssentialSubsetConfig::all_of(validators(4));
        let support = config.validators.clone();
        let amendment = ratify_governance_amendment(
            &domain,
            &config,
            GOVERNANCE_KIND_CRYPTO_POLICY,
            2,
            support,
        )
        .expect("canonical amendment");

        verify_governance_amendment_for_mode(&domain, &amendment, CobaltGovernanceMode::Canonical)
            .expect("canonical mode accepts canonical amendment");
        let nonuniform_error = verify_governance_amendment_for_mode(
            &domain,
            &amendment,
            CobaltGovernanceMode::NonUniform,
        )
        .expect_err("non-uniform mode rejects canonical evidence");
        assert!(
            nonuniform_error.contains("canonical governance amendment evidence"),
            "{nonuniform_error}"
        );
    }

    #[test]
    fn certifies_validator_registry_update_and_rejects_tampering() {
        let domain = test_domain();
        let config = EssentialSubsetConfig {
            validators: vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
            ],
            quorum: 2,
        };
        let request = ValidatorRegistryUpdateRequest {
            activation_height: 12,
            previous_registry_root: root('0'),
            new_registry_root: root('1'),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
            ],
            new_validators: vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
                "validator-3".to_string(),
            ],
            operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
            subject_node_id: "validator-3".to_string(),
            previous_record: None,
            new_record: Some(registry_entry("validator-3", "ab12", true)),
        };

        let update = certify_validator_registry_update(
            &domain,
            &config,
            request,
            vec![
                "validator-2".to_string(),
                "validator-0".to_string(),
                "outsider".to_string(),
            ],
        )
        .expect("certify registry update");

        assert_eq!(update.schema, VALIDATOR_REGISTRY_UPDATE_SCHEMA);
        assert_eq!(update.operation, VALIDATOR_REGISTRY_OP_ADMIT);
        assert_eq!(update.proposer, "validator-0");
        assert_eq!(
            update.support,
            vec!["validator-0".to_string(), "validator-2".to_string()]
        );
        assert_eq!(update.votes.len(), 2);
        assert!(!update.instance_id.is_empty());
        assert!(!update.proposal_id.is_empty());
        assert!(!update.certificate_id.is_empty());
        assert!(!update.update_id.is_empty());
        verify_validator_registry_update(&domain, &update).expect("verify registry update");

        let mut bad_root = update.clone();
        bad_root.new_registry_root = root('2');
        let root_error = verify_validator_registry_update(&domain, &bad_root)
            .expect_err("tampered root should fail");
        assert!(
            root_error.contains("instance mismatch") || root_error.contains("id mismatch"),
            "{root_error}"
        );

        let mut bad_vote = update.clone();
        bad_vote.votes[0].vote_id = "tampered-vote".to_string();
        let vote_error = verify_validator_registry_update(&domain, &bad_vote)
            .expect_err("tampered vote should fail");
        assert!(vote_error.contains("vote id mismatch"), "{vote_error}");

        let mut bad_support = update;
        bad_support.support.reverse();
        let support_error = verify_validator_registry_update(&domain, &bad_support)
            .expect_err("unsorted support should fail");
        assert!(
            support_error.contains("support must be sorted unique"),
            "{support_error}"
        );
    }

    #[test]
    fn registry_update_binds_trust_graph_transition() {
        let domain = test_domain();
        let config = EssentialSubsetConfig {
            validators: vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
            ],
            quorum: 2,
        };
        let request = ValidatorRegistryUpdateRequest {
            activation_height: 12,
            previous_registry_root: root('0'),
            new_registry_root: root('1'),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
            ],
            new_validators: vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
                "validator-3".to_string(),
            ],
            operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
            subject_node_id: "validator-3".to_string(),
            previous_record: None,
            new_record: Some(registry_entry("validator-3", "ab12", true)),
        };
        let transition = build_trust_graph_transition(
            &domain,
            request.previous_registry_root.clone(),
            request.new_registry_root.clone(),
            root('c'),
            root('d'),
            request.activation_height,
        )
        .expect("transition");
        let update = certify_validator_registry_update_with_trust_graph_transition(
            &domain,
            &config,
            request.clone(),
            transition.clone(),
            vec!["validator-0".to_string(), "validator-2".to_string()],
        )
        .expect("bound registry update");
        assert_eq!(
            update.previous_trust_graph_root.as_deref(),
            Some(transition.previous_trust_graph_root.as_str())
        );
        assert_eq!(
            update.new_trust_graph_root.as_deref(),
            Some(transition.new_trust_graph_root.as_str())
        );
        assert_eq!(
            update.trust_graph_transition_id.as_deref(),
            Some(transition.transition_id.as_str())
        );
        verify_validator_registry_update(&domain, &update).expect("verify bound update");

        let mut tampered_root = update.clone();
        tampered_root.new_trust_graph_root = Some(root('e'));
        let tampered_error = verify_validator_registry_update(&domain, &tampered_root)
            .expect_err("tampered trust graph root should fail");
        assert!(
            tampered_error.contains("transition id mismatch")
                || tampered_error.contains("instance mismatch")
                || tampered_error.contains("id mismatch"),
            "{tampered_error}"
        );

        let mut partial = update.clone();
        partial.trust_graph_transition_id = None;
        let partial_error = verify_validator_registry_update(&domain, &partial)
            .expect_err("partial trust graph binding should fail");
        assert!(
            partial_error.contains("must include old root, new root, and transition id"),
            "{partial_error}"
        );

        let mismatched_transition = build_trust_graph_transition(
            &domain,
            request.previous_registry_root.clone(),
            root('2'),
            root('c'),
            root('d'),
            request.activation_height,
        )
        .expect("mismatched transition");
        let mismatch_error = certify_validator_registry_update_with_trust_graph_transition(
            &domain,
            &config,
            request,
            mismatched_transition,
            vec!["validator-0".to_string(), "validator-2".to_string()],
        )
        .expect_err("mismatched transition should fail");
        assert!(
            mismatch_error.contains("does not match trust graph transition"),
            "{mismatch_error}"
        );
    }

    #[test]
    fn dabc_ratifies_validator_lifecycle_payloads() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let config = EssentialSubsetConfig {
            validators: validators(7),
            quorum: 5,
        };
        let registry_update_request = ValidatorRegistryUpdateRequest {
            activation_height: 40,
            previous_registry_root: graph.registry_root.clone(),
            new_registry_root: root('f'),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: validators(7),
            new_validators: validators(7),
            operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
            subject_node_id: "validator-1".to_string(),
            previous_record: Some(registry_entry("validator-1", "ab12", true)),
            new_record: Some(registry_entry("validator-1", "cd34", true)),
        };
        let transition = build_trust_graph_transition(
            &domain,
            registry_update_request.previous_registry_root.clone(),
            registry_update_request.new_registry_root.clone(),
            graph.trust_graph_root.clone(),
            root('e'),
            registry_update_request.activation_height,
        )
        .expect("transition");
        let update = certify_validator_registry_update_with_trust_graph_transition(
            &domain,
            &config,
            registry_update_request,
            transition,
            validators(5),
        )
        .expect("registry update");
        let payload_hash =
            validator_registry_lifecycle_payload_hash(&domain, &update).expect("payload hash");

        let propose = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            41,
            payload_hash.clone(),
            "",
        )
        .expect("propose lifecycle");
        let accept =
            build_rbc_accept(&domain, &propose, "validator-1", "").expect("accept lifecycle");
        let candidate = mvba_candidate_from_rbc_accept(&domain, &propose, &accept)
            .expect("lifecycle candidate");
        let view_1 = trust_view_for_validator(&graph, "validator-1").expect("view 1");
        let input_set = build_mvba_valid_input_set(
            &domain,
            view_1,
            hash_hex("postfiat.test.mvba.agreement", b"validator-lifecycle"),
            vec![candidate],
        )
        .expect("input set");
        let ratified =
            ratify_dabc_amendment(&domain, &graph, &input_set, None, update.activation_height)
                .expect("ratified lifecycle");
        let lifecycle = bind_dabc_ratification_to_validator_registry_update(
            &domain, &graph, &ratified, None, &update,
        )
        .expect("lifecycle binding");
        assert_eq!(lifecycle.operation, VALIDATOR_REGISTRY_OP_ROTATE_KEY);
        assert_eq!(lifecycle.subject_node_id, "validator-1");
        assert_eq!(lifecycle.registry_update_id, update.update_id);
        assert_eq!(lifecycle.dabc_ratification_id, ratified.ratification_id);
        assert_eq!(lifecycle.payload_hash, payload_hash);
        assert!(is_lower_hex_len(&lifecycle.lifecycle_ratification_id, 96));

        let wrong_propose = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            42,
            root('a'),
            "",
        )
        .expect("wrong propose");
        let wrong_accept =
            build_rbc_accept(&domain, &wrong_propose, "validator-1", "").expect("wrong accept");
        let wrong_candidate =
            mvba_candidate_from_rbc_accept(&domain, &wrong_propose, &wrong_accept)
                .expect("wrong candidate");
        let wrong_set = build_mvba_valid_input_set(
            &domain,
            view_1,
            hash_hex("postfiat.test.mvba.agreement", b"wrong-validator-lifecycle"),
            vec![wrong_candidate],
        )
        .expect("wrong set");
        let wrong_ratified =
            ratify_dabc_amendment(&domain, &graph, &wrong_set, None, update.activation_height)
                .expect("wrong ratified");
        let payload_error = bind_dabc_ratification_to_validator_registry_update(
            &domain,
            &graph,
            &wrong_ratified,
            None,
            &update,
        )
        .expect_err("wrong payload should fail");
        assert!(
            payload_error.contains("payload hash mismatch"),
            "{payload_error}"
        );
    }

    #[test]
    fn transaction_network_membership_binds_cobalt_block_metadata() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let membership =
            build_transaction_network_membership(&domain, &graph, 3, validators(5), 4, 20)
                .expect("membership");
        assert_eq!(membership.registry_root, graph.registry_root);
        assert_eq!(membership.trust_graph_root, graph.trust_graph_root);
        assert!(is_lower_hex_len(&membership.transaction_network_id, 96));
        let payload_hash = transaction_network_membership_payload_hash(&domain, &membership)
            .expect("membership payload");
        assert!(is_lower_hex_len(&payload_hash, 96));

        let binding = build_cobalt_block_membership_binding(
            &domain,
            &membership,
            root('a'),
            21,
            "validator-0",
        )
        .expect("block binding");
        assert_eq!(binding.registry_root, membership.registry_root);
        assert_eq!(binding.trust_graph_root, membership.trust_graph_root);
        assert_eq!(binding.governance_epoch, membership.governance_epoch);
        assert_eq!(
            binding.transaction_network_id,
            membership.transaction_network_id
        );
        assert!(is_lower_hex_len(&binding.binding_id, 96));
        validate_cobalt_block_membership_binding(&domain, &membership, &binding)
            .expect("validate binding");

        let mut wrong_root = binding.clone();
        wrong_root.trust_graph_root = root('b');
        wrong_root.binding_id =
            cobalt_block_membership_binding_id(&domain, &wrong_root).expect("wrong id");
        let root_error =
            validate_cobalt_block_membership_binding(&domain, &membership, &wrong_root)
                .expect_err("wrong root should fail");
        assert!(root_error.contains("metadata mismatch"), "{root_error}");

        let outsider_error = build_cobalt_block_membership_binding(
            &domain,
            &membership,
            root('c'),
            21,
            "validator-6",
        )
        .expect_err("outsider proposer should fail");
        assert!(
            outsider_error.contains("outside transaction network"),
            "{outsider_error}"
        );

        let early_error = build_cobalt_block_membership_binding(
            &domain,
            &membership,
            root('d'),
            19,
            "validator-0",
        )
        .expect_err("early block should fail");
        assert!(
            early_error.contains("before transaction network activation"),
            "{early_error}"
        );
    }

    #[test]
    fn transaction_network_transition_rejects_old_set_after_activation() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let previous =
            build_transaction_network_membership(&domain, &graph, 3, validators(5), 4, 20)
                .expect("previous network");
        let next = build_transaction_network_membership(
            &domain,
            &graph,
            4,
            validators(6).into_iter().skip(1).collect(),
            4,
            30,
        )
        .expect("next network");
        validate_transaction_network_transition(&previous, &next).expect("transition");

        let preactivation_old =
            build_cobalt_block_membership_binding(&domain, &previous, root('a'), 29, "validator-0")
                .expect("old preactivation binding");
        validate_cobalt_block_against_transaction_network_transition(
            &domain,
            &previous,
            &next,
            &preactivation_old,
        )
        .expect("old network valid before transition");

        let postactivation_old =
            build_cobalt_block_membership_binding(&domain, &previous, root('b'), 30, "validator-0")
                .expect("old postactivation binding shape");
        let old_error = validate_cobalt_block_against_transaction_network_transition(
            &domain,
            &previous,
            &next,
            &postactivation_old,
        )
        .expect_err("old network should fail after transition");
        assert!(
            old_error.contains("metadata mismatch")
                || old_error.contains("outside transaction network"),
            "{old_error}"
        );

        let postactivation_new =
            build_cobalt_block_membership_binding(&domain, &next, root('c'), 30, "validator-5")
                .expect("new postactivation binding");
        validate_cobalt_block_against_transaction_network_transition(
            &domain,
            &previous,
            &next,
            &postactivation_new,
        )
        .expect("new network valid after transition");
    }

    #[test]
    fn transaction_network_replacement_drill_ratifies_new_network_after_ordering_failure() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let previous =
            build_transaction_network_membership(&domain, &graph, 3, validators(5), 4, 20)
                .expect("previous network");
        let replacement = build_transaction_network_membership(
            &domain,
            &graph,
            4,
            validators(6).into_iter().skip(1).collect(),
            4,
            30,
        )
        .expect("replacement network");
        validate_transaction_network_transition(&previous, &replacement).expect("transition");

        let replacement_payload =
            transaction_network_membership_payload_hash(&domain, &replacement)
                .expect("replacement payload");
        let propose = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            301,
            replacement_payload.clone(),
            "",
        )
        .expect("replacement propose");
        let accept =
            build_rbc_accept(&domain, &propose, "validator-1", "").expect("replacement accept");
        let candidate = mvba_candidate_from_rbc_accept(&domain, &propose, &accept)
            .expect("replacement candidate");
        let view_1 = trust_view_for_validator(&graph, "validator-1").expect("view 1");
        let input_set = build_mvba_valid_input_set(
            &domain,
            view_1,
            hash_hex(
                "postfiat.test.mvba.agreement",
                b"transaction-network-replacement",
            ),
            vec![candidate],
        )
        .expect("replacement input set");
        let ratified = ratify_dabc_amendment(
            &domain,
            &graph,
            &input_set,
            None,
            replacement.activation_height,
        )
        .expect("replacement ratification");
        let dabc_replacement = bind_dabc_ratification_to_transaction_network_membership(
            &domain,
            &graph,
            &ratified,
            None,
            &replacement,
        )
        .expect("DABC replacement binding");
        assert_eq!(
            dabc_replacement.dabc_ratification_id,
            ratified.ratification_id
        );
        assert_eq!(
            dabc_replacement.transaction_network_id,
            replacement.transaction_network_id
        );
        assert_eq!(dabc_replacement.payload_hash, replacement_payload);
        assert!(is_lower_hex_len(
            &dabc_replacement.transaction_network_ratification_id,
            96
        ));

        let failed_old_network_block =
            build_cobalt_block_membership_binding(&domain, &previous, root('b'), 30, "validator-0")
                .expect("failed old network block binding shape");
        let failed_old_error = validate_cobalt_block_against_transaction_network_transition(
            &domain,
            &previous,
            &replacement,
            &failed_old_network_block,
        )
        .expect_err("old network should fail after replacement activation");
        assert!(
            failed_old_error.contains("metadata mismatch")
                || failed_old_error.contains("outside transaction network"),
            "{failed_old_error}"
        );

        let resumed_block = build_cobalt_block_membership_binding(
            &domain,
            &replacement,
            root('c'),
            30,
            "validator-5",
        )
        .expect("replacement block");
        validate_cobalt_block_against_transaction_network_transition(
            &domain,
            &previous,
            &replacement,
            &resumed_block,
        )
        .expect("replacement network resumes finality");

        let wrong_propose = build_rbc_propose(
            &domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            302,
            root('d'),
            "",
        )
        .expect("wrong replacement propose");
        let wrong_accept =
            build_rbc_accept(&domain, &wrong_propose, "validator-1", "").expect("wrong accept");
        let wrong_candidate =
            mvba_candidate_from_rbc_accept(&domain, &wrong_propose, &wrong_accept)
                .expect("wrong candidate");
        let wrong_input_set = build_mvba_valid_input_set(
            &domain,
            view_1,
            hash_hex(
                "postfiat.test.mvba.agreement",
                b"wrong-transaction-network-replacement",
            ),
            vec![wrong_candidate],
        )
        .expect("wrong replacement input set");
        let wrong_ratified = ratify_dabc_amendment(
            &domain,
            &graph,
            &wrong_input_set,
            None,
            replacement.activation_height,
        )
        .expect("wrong replacement ratification");
        let wrong_payload_error = bind_dabc_ratification_to_transaction_network_membership(
            &domain,
            &graph,
            &wrong_ratified,
            None,
            &replacement,
        )
        .expect_err("wrong replacement payload should fail");
        assert!(
            wrong_payload_error.contains("payload hash mismatch"),
            "{wrong_payload_error}"
        );
    }

    #[test]
    fn trust_graph_lifecycle_updates_reject_unsafe_graphs_before_activation() {
        let (domain, graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let previous_view = trust_view_for_validator(&graph, "validator-1").expect("view 1");

        let version_bump = build_trust_view(
            &domain,
            "validator-1",
            previous_view.view_version + 1,
            previous_view.essential_subsets.clone(),
            "",
        )
        .expect("version bump");
        let (view_update_graph, view_record) = build_trust_view_update_transition(
            &domain,
            &graph,
            version_bump,
            30,
            &CobaltFaultModel::default(),
        )
        .expect("safe trust view update");
        assert_eq!(
            view_update_graph.previous_trust_graph_root.as_deref(),
            Some(graph.trust_graph_root.as_str())
        );
        assert_eq!(
            view_record.operation,
            TRUST_GRAPH_LIFECYCLE_OP_TRUST_VIEW_UPDATE
        );
        assert_eq!(view_record.previous_registry_root, graph.registry_root);
        assert_eq!(
            view_record.new_registry_root,
            view_update_graph.registry_root
        );
        assert_ne!(
            view_record.previous_trust_graph_root,
            view_record.new_trust_graph_root
        );

        let view_update_linkage =
            analyze_trust_graph(&domain, &view_update_graph, &CobaltFaultModel::default())
                .expect("view update linkage");
        validate_trust_graph_lifecycle_record(
            &domain,
            &graph,
            &view_update_graph,
            &view_update_linkage,
            &view_record,
        )
        .expect("validate lifecycle record");

        let mut updated_subsets = previous_view.essential_subsets.clone();
        updated_subsets.push(subset(
            &domain,
            &[
                "validator-2",
                "validator-3",
                "validator-4",
                "validator-5",
                "validator-6",
            ],
            1,
            4,
        ));
        let essential_subset_update = build_trust_view(
            &domain,
            "validator-1",
            previous_view.view_version + 2,
            updated_subsets,
            "",
        )
        .expect("essential subset update");
        let (_subset_update_graph, subset_record) = build_essential_subset_update_transition(
            &domain,
            &graph,
            essential_subset_update,
            31,
            &CobaltFaultModel::default(),
        )
        .expect("safe essential subset update");
        assert_eq!(
            subset_record.operation,
            TRUST_GRAPH_LIFECYCLE_OP_ESSENTIAL_SUBSET_UPDATE
        );
        assert_ne!(
            subset_record.previous_subset_ids,
            subset_record.new_subset_ids
        );

        let unsafe_view = build_trust_view(
            &domain,
            "validator-1",
            previous_view.view_version + 3,
            vec![subset(&domain, &["validator-1"], 0, 1)],
            "",
        )
        .expect("unsafe view shape");
        let unsafe_error = build_essential_subset_update_transition(
            &domain,
            &graph,
            unsafe_view,
            32,
            &CobaltFaultModel::default(),
        )
        .expect_err("unsafe graph should fail before activation");
        assert!(
            unsafe_error.contains("unsafe before activation"),
            "{unsafe_error}"
        );
    }

    #[test]
    fn trust_graph_rollback_restores_authority_graph_after_bad_activation() {
        let (domain, authority_graph, _linkage_report, _proposal, _support) =
            nonuniform_certificate_fixture();
        let previous_view =
            trust_view_for_validator(&authority_graph, "validator-1").expect("view 1");
        let unsafe_view = build_trust_view(
            &domain,
            "validator-1",
            previous_view.view_version + 1,
            vec![subset(&domain, &["validator-1"], 0, 1)],
            "",
        )
        .expect("unsafe view");
        let mut bad_views = authority_graph.trust_views.clone();
        let bad_slot = bad_views
            .iter_mut()
            .find(|view| view.validator == "validator-1")
            .expect("bad view slot");
        *bad_slot = unsafe_view;
        let bad_graph = build_trust_graph(
            &domain,
            authority_graph.graph_version + 1,
            authority_graph.registry_root.clone(),
            40,
            Some(authority_graph.trust_graph_root.clone()),
            bad_views,
        )
        .expect("bad graph shape");
        let bad_linkage = analyze_trust_graph(&domain, &bad_graph, &CobaltFaultModel::default())
            .expect("bad linkage");
        assert!(!bad_linkage.unsafe_pairs.is_empty());

        let (rollback_graph, rollback_linkage, rollback_record) =
            build_trust_graph_rollback_transition(
                &domain,
                &authority_graph,
                &bad_graph,
                45,
                &bad_linkage,
            )
            .expect("rollback transition");
        assert_eq!(
            rollback_graph.previous_trust_graph_root.as_deref(),
            Some(bad_graph.trust_graph_root.as_str())
        );
        assert_eq!(rollback_graph.trust_views, authority_graph.trust_views);
        assert!(rollback_linkage.unsafe_pairs.is_empty());
        assert_eq!(
            rollback_record.reason,
            TRUST_GRAPH_ROLLBACK_REASON_UNSAFE_LINKAGE
        );
        assert_eq!(
            rollback_record.authority_trust_graph_root,
            authority_graph.trust_graph_root
        );
        assert_eq!(
            rollback_record.failed_trust_graph_root,
            bad_graph.trust_graph_root
        );
        assert_eq!(
            rollback_record.rollback_trust_graph_root,
            rollback_graph.trust_graph_root
        );
        validate_trust_graph_rollback_record(
            &domain,
            &authority_graph,
            &bad_graph,
            &rollback_graph,
            &bad_linkage,
            &rollback_linkage,
            &rollback_record,
        )
        .expect("validate rollback record");

        let payload_hash = trust_graph_rollback_payload_hash(&domain, &rollback_record)
            .expect("rollback payload hash");
        let propose = build_rbc_propose(
            &domain,
            authority_graph.trust_graph_root.clone(),
            "validator-0",
            201,
            payload_hash.clone(),
            "",
        )
        .expect("rollback propose");
        let accept =
            build_rbc_accept(&domain, &propose, "validator-1", "").expect("rollback accept");
        let candidate =
            mvba_candidate_from_rbc_accept(&domain, &propose, &accept).expect("rollback candidate");
        let view_1 =
            trust_view_for_validator(&authority_graph, "validator-1").expect("authority view 1");
        let input_set = build_mvba_valid_input_set(
            &domain,
            view_1,
            hash_hex("postfiat.test.mvba.agreement", b"trust-graph-rollback"),
            vec![candidate],
        )
        .expect("rollback input set");
        let ratified = ratify_dabc_amendment(
            &domain,
            &authority_graph,
            &input_set,
            None,
            rollback_record.rollback_activation_height,
        )
        .expect("rollback ratification");
        let dabc_rollback = bind_dabc_ratification_to_trust_graph_rollback_record(
            TrustGraphRollbackRatificationInput {
                domain: &domain,
                authority_graph: &authority_graph,
                failed_graph: &bad_graph,
                rollback_graph: &rollback_graph,
                bad_linkage_report: &bad_linkage,
                rollback_linkage_report: &rollback_linkage,
                ratified: &ratified,
                previous_ratified: None,
                record: &rollback_record,
            },
        )
        .expect("DABC rollback binding");
        assert_eq!(dabc_rollback.dabc_ratification_id, ratified.ratification_id);
        assert_eq!(dabc_rollback.rollback_record_id, rollback_record.record_id);
        assert_eq!(dabc_rollback.payload_hash, payload_hash);
        assert!(is_lower_hex_len(
            &dabc_rollback.rollback_ratification_id,
            96
        ));

        let mut tampered_record = rollback_record.clone();
        tampered_record.reason = "operator_override".to_string();
        let tampered_error = validate_trust_graph_rollback_record(
            &domain,
            &authority_graph,
            &bad_graph,
            &rollback_graph,
            &bad_linkage,
            &rollback_linkage,
            &tampered_record,
        )
        .expect_err("tampered rollback record should fail");
        assert!(
            tampered_error.contains("reason mismatch")
                || tampered_error.contains("record id mismatch"),
            "{tampered_error}"
        );

        let wrong_propose = build_rbc_propose(
            &domain,
            authority_graph.trust_graph_root.clone(),
            "validator-0",
            202,
            root('d'),
            "",
        )
        .expect("wrong rollback propose");
        let wrong_accept =
            build_rbc_accept(&domain, &wrong_propose, "validator-1", "").expect("wrong accept");
        let wrong_candidate =
            mvba_candidate_from_rbc_accept(&domain, &wrong_propose, &wrong_accept)
                .expect("wrong candidate");
        let wrong_input_set = build_mvba_valid_input_set(
            &domain,
            view_1,
            hash_hex(
                "postfiat.test.mvba.agreement",
                b"wrong-trust-graph-rollback",
            ),
            vec![wrong_candidate],
        )
        .expect("wrong input set");
        let wrong_ratified = ratify_dabc_amendment(
            &domain,
            &authority_graph,
            &wrong_input_set,
            None,
            rollback_record.rollback_activation_height,
        )
        .expect("wrong rollback ratification");
        let payload_error = bind_dabc_ratification_to_trust_graph_rollback_record(
            TrustGraphRollbackRatificationInput {
                domain: &domain,
                authority_graph: &authority_graph,
                failed_graph: &bad_graph,
                rollback_graph: &rollback_graph,
                bad_linkage_report: &bad_linkage,
                rollback_linkage_report: &rollback_linkage,
                ratified: &wrong_ratified,
                previous_ratified: None,
                record: &rollback_record,
            },
        )
        .expect_err("wrong rollback payload should fail");
        assert!(
            payload_error.contains("payload hash mismatch"),
            "{payload_error}"
        );
    }

    #[test]
    fn dabc_ratifies_first_nonidentical_trust_graph_g1() {
        let domain = test_domain();
        let g0 = build_canonical_unl_trust_graph(&domain, 1, root('a'), 1, None, validators(7), 5)
            .expect("canonical G0");
        let validator_1_g0 = trust_view_for_validator(&g0, "validator-1").expect("validator-1 G0");
        let mut g1_subsets = validator_1_g0.essential_subsets.clone();
        g1_subsets.push(subset(
            &domain,
            &[
                "validator-0",
                "validator-1",
                "validator-2",
                "validator-3",
                "validator-4",
            ],
            1,
            4,
        ));
        let validator_1_g1 = build_trust_view(&domain, "validator-1", 2, g1_subsets, "")
            .expect("validator-1 G1 view");
        let (g1, record) = build_trust_view_update_transition(
            &domain,
            &g0,
            validator_1_g1,
            30,
            &CobaltFaultModel::default(),
        )
        .expect("G1 lifecycle transition");
        assert_ne!(g0.trust_graph_root, g1.trust_graph_root);
        assert_eq!(record.operation, TRUST_GRAPH_LIFECYCLE_OP_TRUST_VIEW_UPDATE);

        let g1_linkage =
            analyze_trust_graph(&domain, &g1, &CobaltFaultModel::default()).expect("G1 linkage");
        validate_trust_graph_lifecycle_record(&domain, &g0, &g1, &g1_linkage, &record)
            .expect("validate G1 lifecycle record");
        let payload_hash =
            trust_graph_lifecycle_payload_hash(&domain, &record).expect("G1 payload hash");
        let propose = build_rbc_propose(
            &domain,
            g0.trust_graph_root.clone(),
            "validator-0",
            101,
            payload_hash.clone(),
            "",
        )
        .expect("propose G1 lifecycle");
        let accept = build_rbc_accept(&domain, &propose, "validator-1", "").expect("accept G1");
        let candidate =
            mvba_candidate_from_rbc_accept(&domain, &propose, &accept).expect("candidate G1");
        let g0_view = trust_view_for_validator(&g0, "validator-1").expect("G0 view");
        let input_set = build_mvba_valid_input_set(
            &domain,
            g0_view,
            hash_hex("postfiat.test.mvba.agreement", b"g1-trust-graph"),
            vec![candidate],
        )
        .expect("G1 input set");
        let ratified =
            ratify_dabc_amendment(&domain, &g0, &input_set, None, record.activation_height)
                .expect("ratify G1");
        let lifecycle = bind_dabc_ratification_to_trust_graph_lifecycle_record(
            &domain,
            &g0,
            &g1,
            &g1_linkage,
            &ratified,
            None,
            &record,
        )
        .expect("bind DABC G1 lifecycle");
        assert_eq!(
            lifecycle.operation,
            TRUST_GRAPH_LIFECYCLE_OP_TRUST_VIEW_UPDATE
        );
        assert_eq!(lifecycle.subject_validator, "validator-1");
        assert_eq!(lifecycle.previous_trust_graph_root, g0.trust_graph_root);
        assert_eq!(lifecycle.new_trust_graph_root, g1.trust_graph_root);
        assert_eq!(lifecycle.payload_hash, payload_hash);
        assert!(is_lower_hex_len(&lifecycle.lifecycle_ratification_id, 96));

        let wrong_propose = build_rbc_propose(
            &domain,
            g0.trust_graph_root.clone(),
            "validator-0",
            102,
            root('f'),
            "",
        )
        .expect("wrong propose");
        let wrong_accept =
            build_rbc_accept(&domain, &wrong_propose, "validator-1", "").expect("wrong accept");
        let wrong_candidate =
            mvba_candidate_from_rbc_accept(&domain, &wrong_propose, &wrong_accept)
                .expect("wrong candidate");
        let wrong_input_set = build_mvba_valid_input_set(
            &domain,
            g0_view,
            hash_hex("postfiat.test.mvba.agreement", b"wrong-g1-trust-graph"),
            vec![wrong_candidate],
        )
        .expect("wrong input set");
        let wrong_ratified = ratify_dabc_amendment(
            &domain,
            &g0,
            &wrong_input_set,
            None,
            record.activation_height,
        )
        .expect("wrong ratified");
        let payload_error = bind_dabc_ratification_to_trust_graph_lifecycle_record(
            &domain,
            &g0,
            &g1,
            &g1_linkage,
            &wrong_ratified,
            None,
            &record,
        )
        .expect_err("wrong payload should fail");
        assert!(
            payload_error.contains("payload hash mismatch"),
            "{payload_error}"
        );
    }

    #[test]
    fn validates_registry_update_lifecycle_shapes() {
        let domain = test_domain();
        let config = EssentialSubsetConfig {
            validators: vec!["validator-0".to_string(), "validator-1".to_string()],
            quorum: 2,
        };
        let support = config.validators.clone();

        let rotate = ValidatorRegistryUpdateRequest {
            activation_height: 7,
            previous_registry_root: root('a'),
            new_registry_root: root('b'),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: config.validators.clone(),
            new_validators: config.validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
            subject_node_id: "validator-1".to_string(),
            previous_record: Some(registry_entry("validator-1", "ab12", true)),
            new_record: Some(registry_entry("validator-1", "cd34", true)),
        };
        let update = certify_validator_registry_update(&domain, &config, rotate, support.clone())
            .expect("valid key rotation");
        verify_validator_registry_update(&domain, &update).expect("verify key rotation");

        let inactive_config = EssentialSubsetConfig {
            validators: vec!["validator-0".to_string(), "validator-2".to_string()],
            quorum: 2,
        };
        let inactive_support = inactive_config.validators.clone();
        let inactive_rotate = ValidatorRegistryUpdateRequest {
            activation_height: 8,
            previous_registry_root: root('b'),
            new_registry_root: root('c'),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: inactive_config.validators.clone(),
            new_validators: inactive_config.validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
            subject_node_id: "validator-1".to_string(),
            previous_record: Some(registry_entry("validator-1", "ab12", false)),
            new_record: Some(registry_entry("validator-1", "cd34", false)),
        };
        let inactive_update = certify_validator_registry_update(
            &domain,
            &inactive_config,
            inactive_rotate,
            inactive_support.clone(),
        )
        .expect("valid inactive key rotation");
        verify_validator_registry_update(&domain, &inactive_update)
            .expect("verify inactive key rotation");

        let inactive_rotate_with_scope = ValidatorRegistryUpdateRequest {
            activation_height: 8,
            previous_registry_root: root('b'),
            new_registry_root: root('c'),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: config.validators.clone(),
            new_validators: config.validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
            subject_node_id: "validator-1".to_string(),
            previous_record: Some(registry_entry("validator-1", "ab12", false)),
            new_record: Some(registry_entry("validator-1", "cd34", false)),
        };
        let inactive_scope_error = certify_validator_registry_update(
            &domain,
            &config,
            inactive_rotate_with_scope,
            support.clone(),
        )
        .expect_err("inactive key rotation cannot keep subject in active scope");
        assert!(
            inactive_scope_error.contains("subject unexpectedly present"),
            "{inactive_scope_error}"
        );

        let rotate_and_reactivate = ValidatorRegistryUpdateRequest {
            activation_height: 8,
            previous_registry_root: root('b'),
            new_registry_root: root('c'),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: inactive_config.validators.clone(),
            new_validators: config.validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
            subject_node_id: "validator-1".to_string(),
            previous_record: Some(registry_entry("validator-1", "ab12", false)),
            new_record: Some(registry_entry("validator-1", "cd34", true)),
        };
        let status_error = certify_validator_registry_update(
            &domain,
            &config,
            rotate_and_reactivate,
            inactive_support,
        )
        .expect_err("key rotation cannot also reactivate");
        assert!(
            status_error.contains("cannot change active status"),
            "{status_error}"
        );

        let unchanged_key = ValidatorRegistryUpdateRequest {
            activation_height: 7,
            previous_registry_root: root('a'),
            new_registry_root: root('b'),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: config.validators.clone(),
            new_validators: config.validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
            subject_node_id: "validator-1".to_string(),
            previous_record: Some(registry_entry("validator-1", "ab12", true)),
            new_record: Some(registry_entry("validator-1", "ab12", true)),
        };
        let error =
            certify_validator_registry_update(&domain, &config, unchanged_key, support.clone())
                .expect_err("unchanged key should fail");
        assert!(error.contains("must change public key"), "{error}");

        let malformed_root = ValidatorRegistryUpdateRequest {
            activation_height: 7,
            previous_registry_root: "not-a-root".to_string(),
            new_registry_root: root('b'),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: config.validators.clone(),
            new_validators: vec!["validator-0".to_string()],
            operation: VALIDATOR_REGISTRY_OP_REMOVE.to_string(),
            subject_node_id: "validator-1".to_string(),
            previous_record: Some(registry_entry("validator-1", "ab12", true)),
            new_record: None,
        };
        let error = certify_validator_registry_update(&domain, &config, malformed_root, support)
            .expect_err("malformed root should fail");
        assert!(error.contains("roots must be"), "{error}");
    }

    fn admission_control_group(
        validator_id: &str,
        operator_group: &str,
        release_manager_group: &str,
        key_management_group: &str,
        funding_source_group: &str,
    ) -> ValidatorAdmissionControlGroup {
        ValidatorAdmissionControlGroup {
            validator_id: validator_id.to_string(),
            operator_group: operator_group.to_string(),
            release_manager_group: release_manager_group.to_string(),
            key_management_group: key_management_group.to_string(),
            funding_source_group: funding_source_group.to_string(),
        }
    }

    fn admission_evidence_ref(field_id: &str) -> ValidatorAdmissionEvidenceRef {
        ValidatorAdmissionEvidenceRef {
            field_id: field_id.to_string(),
            source_hash: root('e'),
            missing: false,
            stale: false,
            conflicting: false,
        }
    }

    fn clean_admission_packet() -> ValidatorAdmissionEvidencePacket {
        let mut evidence_refs = vec![
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_ACCOUNTABILITY),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_RHO),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_COBALT_LINKEDNESS),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_OPERATOR_MANIFEST),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_RELIABILITY),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_KEY_MANAGEMENT),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_OPERATOR_GROUP),
            admission_evidence_ref(VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER),
        ];
        evidence_refs.sort_by(|left, right| left.field_id.cmp(&right.field_id));
        ValidatorAdmissionEvidencePacket {
            packet_id: "admission-packet-clean".to_string(),
            registry_root: root('a'),
            candidate: ValidatorAdmissionCandidateEvidence {
                validator_id: "validator-new".to_string(),
                public_key_hash: root('b'),
                reliability_bps: Some(9_980),
                accountability_score: Some(85),
                rho_score: Some(0),
                operator_manifest_signed: Some(true),
                domain_control_proved: Some(true),
                cobalt_linkedness_safe: Some(true),
                control_group: admission_control_group(
                    "validator-new",
                    "operator-new",
                    "release-new",
                    "kms-new",
                    "funding-new",
                ),
            },
            active_validators: vec![
                admission_control_group(
                    "validator-0",
                    "operator-0",
                    "release-0",
                    "kms-0",
                    "funding-0",
                ),
                admission_control_group(
                    "validator-1",
                    "operator-1",
                    "release-1",
                    "kms-1",
                    "funding-1",
                ),
            ],
            evidence_refs,
            model_output: Some(ValidatorAdmissionModelOutput {
                classification: "independent".to_string(),
                cited_fields: vec![
                    VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL.to_string(),
                    VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE.to_string(),
                    VALIDATOR_ADMISSION_FIELD_KEY_MANAGEMENT.to_string(),
                    VALIDATOR_ADMISSION_FIELD_OPERATOR_GROUP.to_string(),
                    VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER.to_string(),
                ],
                parsed_output_root: root('c'),
                replay_certificate_root: root('d'),
            }),
        }
    }

    #[test]
    fn validator_admission_policy_admits_clean_independent_candidate() {
        let domain = test_domain();
        let policy = ValidatorAdmissionPolicy::controlled_testnet_v1();
        let decision = evaluate_validator_admission(&domain, &policy, &clean_admission_packet())
            .expect("clean admission decision");

        assert_eq!(decision.action, VALIDATOR_ADMISSION_ACTION_ADMIT);
        assert_eq!(decision.reason_codes, vec!["all_gates_passed"]);
        assert_eq!(
            decision.registry_delta_candidate.delta_kind,
            VALIDATOR_ADMISSION_DELTA_ADD
        );
        assert_eq!(decision.registry_delta_candidate.mutation_count, 1);
        assert!(decision
            .registry_delta_candidate
            .candidate_record_hash
            .is_some());
    }

    #[test]
    fn validator_admission_policy_rejects_shared_control_groups() {
        let domain = test_domain();
        let policy = ValidatorAdmissionPolicy::controlled_testnet_v1();
        let mut packet = clean_admission_packet();
        packet.packet_id = "admission-packet-shared-control".to_string();
        packet.candidate.control_group.release_manager_group = "release-1".to_string();
        packet.candidate.control_group.key_management_group = "kms-1".to_string();

        let decision =
            evaluate_validator_admission(&domain, &policy, &packet).expect("shared control");

        assert_eq!(decision.action, VALIDATOR_ADMISSION_ACTION_REJECT);
        assert!(decision
            .reason_codes
            .contains(&"shared_release_manager".to_string()));
        assert!(decision
            .reason_codes
            .contains(&"shared_key_management".to_string()));
        assert!(decision
            .correlation_cluster
            .contains(&"validator-1".to_string()));
        assert_eq!(
            decision.registry_delta_candidate.delta_kind,
            VALIDATOR_ADMISSION_DELTA_NO_OP
        );
        assert_eq!(decision.registry_delta_candidate.mutation_count, 0);
    }

    #[test]
    fn validator_admission_policy_holds_missing_domain_proof() {
        let domain = test_domain();
        let policy = ValidatorAdmissionPolicy::controlled_testnet_v1();
        let mut packet = clean_admission_packet();
        packet.packet_id = "admission-packet-missing-domain".to_string();
        packet.candidate.domain_control_proved = None;
        for reference in &mut packet.evidence_refs {
            if reference.field_id == VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL {
                reference.missing = true;
            }
        }

        let decision =
            evaluate_validator_admission(&domain, &policy, &packet).expect("missing domain hold");

        assert_eq!(decision.action, VALIDATOR_ADMISSION_ACTION_HOLD);
        assert!(decision
            .reason_codes
            .contains(&"missing_required_evidence".to_string()));
        assert!(decision
            .reason_codes
            .contains(&"missing_domain_control".to_string()));
        assert!(decision
            .required_followup_evidence
            .contains(&VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL.to_string()));
        assert_eq!(decision.registry_delta_candidate.mutation_count, 0);
    }

    #[test]
    fn validator_admission_policy_holds_contradictory_release_and_funding_evidence() {
        let domain = test_domain();
        let policy = ValidatorAdmissionPolicy::controlled_testnet_v1();
        let mut packet = clean_admission_packet();
        packet.packet_id = "admission-packet-conflicting-evidence".to_string();
        for reference in &mut packet.evidence_refs {
            if reference.field_id == VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER
                || reference.field_id == VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE
            {
                reference.conflicting = true;
            }
        }
        packet.model_output = Some(ValidatorAdmissionModelOutput {
            classification: "contradictory".to_string(),
            cited_fields: vec![
                VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE.to_string(),
                VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER.to_string(),
            ],
            parsed_output_root: root('c'),
            replay_certificate_root: root('d'),
        });

        let decision =
            evaluate_validator_admission(&domain, &policy, &packet).expect("conflict hold");

        assert_eq!(decision.action, VALIDATOR_ADMISSION_ACTION_HOLD);
        assert!(decision
            .reason_codes
            .contains(&"conflicting_required_evidence".to_string()));
        assert!(decision
            .reason_codes
            .contains(&"model_classified_contradictory".to_string()));
        assert_eq!(decision.registry_delta_candidate.mutation_count, 0);
    }

    #[test]
    fn validator_admission_policy_fails_closed_on_unknown_model_field() {
        let domain = test_domain();
        let policy = ValidatorAdmissionPolicy::controlled_testnet_v1();
        let mut packet = clean_admission_packet();
        packet.packet_id = "admission-packet-unknown-model-field".to_string();
        packet.model_output = Some(ValidatorAdmissionModelOutput {
            classification: "independent".to_string(),
            cited_fields: vec!["validator.private.kyc_status".to_string()],
            parsed_output_root: root('c'),
            replay_certificate_root: root('d'),
        });

        let decision =
            evaluate_validator_admission(&domain, &policy, &packet).expect("unknown model field");

        assert_eq!(decision.action, VALIDATOR_ADMISSION_ACTION_HOLD);
        assert!(decision
            .reason_codes
            .contains(&"model_cited_unknown_field".to_string()));
        assert!(decision
            .failed_fields
            .contains(&"validator.private.kyc_status".to_string()));
        assert_eq!(decision.registry_delta_candidate.mutation_count, 0);
    }
