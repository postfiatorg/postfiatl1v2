    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static GOVERNANCE_AGENT_TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn governance_agent_canonical_json_sorts_object_keys() {
        let left: serde_json::Value =
            serde_json::from_str(r#"{"b":2,"a":1,"nested":{"z":0,"m":1}}"#).unwrap();
        let right: serde_json::Value =
            serde_json::from_str(r#"{"nested":{"m":1,"z":0},"a":1,"b":2}"#).unwrap();
        assert_eq!(
            governance_agent_canonical_json_bytes(&left).unwrap(),
            governance_agent_canonical_json_bytes(&right).unwrap()
        );
    }

    #[test]
    fn governance_agent_canonical_json_rejects_float_numbers() {
        let value: serde_json::Value = serde_json::from_str(r#"{"a":1.25}"#).unwrap();
        let error = governance_agent_canonical_json_bytes(&value).unwrap_err();
        assert!(error.to_string().contains("floating-point"));
    }

    #[test]
    fn governance_agent_fixtures_validate_and_fail_closed() {
        let agent_dir = repo_root().join(DEFAULT_GOVERNANCE_AGENT_DIR);
        let validation = validate_governance_agent_source_bundle(&agent_dir).unwrap();
        assert!(validation.valid_fixture.accepted);
        assert_eq!(validation.invalid_fixtures.len(), 6);
        assert!(validation
            .invalid_fixtures
            .iter()
            .all(|fixture| !fixture.accepted && fixture.error.is_some()));
        let missing_packet_input = validation
            .invalid_fixtures
            .iter()
            .find(|fixture| fixture.name == "missing_validator_evidence_packet_input")
            .expect("missing packet-input invalid fixture");
        assert!(missing_packet_input
            .error
            .as_ref()
            .unwrap()
            .contains(GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND));
        assert!(validation.canonical_json_key_order_stable);
        assert!(validation.statement_hash_one_byte_edit_detected);
        assert_eq!(validation.bundle_hash.len(), 96);
        assert_eq!(validation.model_request_hash.len(), 96);
    }

    #[test]
    fn governance_agent_gate_writes_redacted_report() {
        let root = repo_root();
        let output_file = unique_governance_agent_test_dir().join("gate-1_5.json");
        let report = governance_agent_gate_1_5(GovernanceAgentGateOptions {
            agent_dir: root.join(DEFAULT_GOVERNANCE_AGENT_DIR),
            output_file: output_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert!(report.redaction_checked);
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGateReport = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.bundle_hash, report.bundle_hash);
        let _ = std::fs::remove_file(output_file);
    }

    #[test]
    fn governance_agent_model_request_is_deterministic_and_json_only() {
        let agent_dir = repo_root().join(DEFAULT_GOVERNANCE_AGENT_DIR);
        let first = build_governance_agent_model_request(&agent_dir).unwrap();
        let second = build_governance_agent_model_request(&agent_dir).unwrap();
        assert_eq!(first.request_hash, second.request_hash);
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
        assert_eq!(first.runtime_manifest.model_id, GOVERNANCE_AGENT_MODEL_ID);
        assert_eq!(first.runtime_manifest.runtime, GOVERNANCE_AGENT_RUNTIME_ENGINE);
        assert_eq!(first.runtime_manifest.image, GOVERNANCE_AGENT_SGLANG_IMAGE);
        assert_eq!(
            first.runtime_manifest.max_running_requests,
            GOVERNANCE_AGENT_MAX_RUNNING_REQUESTS
        );
        assert!(first
            .runtime_manifest
            .deterministic_flags
            .iter()
            .any(|flag| flag == "--enable-deterministic-inference"));
        assert_eq!(
            first.round_seed_input,
            default_governance_agent_round_seed_input()
        );
        assert_eq!(first.round_seed.len(), 96);
        assert_eq!(
            first.governed_inputs.validator_evidence_packet_schema_hash.len(),
            96
        );
        assert_eq!(
            first.governed_inputs
                .validator_evidence_field_registry_hash
                .len(),
            96
        );
        assert!(first
            .request_hash_includes
            .iter()
            .any(|field| field == "round_seed_input"));
        let valid_inputs = first.governed_inputs.valid_fixture["inputs"]
            .as_array()
            .unwrap();
        assert!(valid_inputs.iter().any(|input| {
            input["kind"] == GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND
                && input["required"] == true
        }));
        ensure_governance_agent_model_request_json_only(&first).unwrap();
        assert_eq!(
            first.openai_chat_request["response_format"]["type"],
            "json_object"
        );
        assert!(first.openai_chat_request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["content"]
                .as_str()
                .unwrap()
                .contains("Return exactly one JSON object")));
        assert!(first.openai_chat_request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["content"]
                .as_str()
                .unwrap()
                .contains(GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND)));
        assert!(first.openai_chat_request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["content"]
                .as_str()
                .unwrap()
                .contains(&first.governed_inputs.validator_evidence_packet_schema_hash)));
        assert!(first.openai_chat_request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["content"]
                .as_str()
                .unwrap()
                .contains(&first.governed_inputs.validator_evidence_field_registry_hash)));
    }

    #[test]
    fn governance_agent_model_request_writes_redacted_fixture() {
        let root = repo_root();
        let output_file = unique_governance_agent_test_dir().join("model-request.json");
        let request = governance_agent_model_request(GovernanceAgentModelRequestOptions {
            agent_dir: root.join(DEFAULT_GOVERNANCE_AGENT_DIR),
            output_file: output_file.clone(),
            round_seed_input: None,
            overwrite: false,
        })
        .unwrap();
        assert!(request.redaction_checked);
        assert_eq!(request.request_hash.len(), 96);
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentModelRequest = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.request_hash, request.request_hash);
        let _ = std::fs::remove_file(output_file);
    }

    #[test]
    fn governance_agent_gate_3_5_accepts_identical_ruleset_outputs() {
        let root = repo_root();
        let temp = unique_governance_agent_test_dir();
        let outputs_dir = temp.join("outputs");
        std::fs::create_dir_all(&outputs_dir).unwrap();
        let fixture = root
            .join(DEFAULT_GOVERNANCE_AGENT_DIR)
            .join("fixtures/valid_ruleset.json");
        for index in 1..=3 {
            std::fs::copy(
                &fixture,
                outputs_dir.join(format!("model_output_{index:04}.json")),
            )
            .unwrap();
        }
        let report_file = temp.join("gate-3_5.json");
        let report = governance_agent_gate_3_5(GovernanceAgentGate3_5Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            outputs_dir,
            output_file: report_file.clone(),
            expected_count: 3,
            round_seed_input: None,
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert_eq!(report.observed_count, 3);
        assert_eq!(report.valid_count, 3);
        assert_eq!(report.distinct_ruleset_hashes.len(), 1);
        assert_eq!(report.distinct_compiled_policy_hashes.len(), 1);
        assert_eq!(report.ruleset_hash.as_ref().unwrap().len(), 96);
        assert_eq!(report.compiled_policy_hash.as_ref().unwrap().len(), 96);
        assert_report_validator_evidence_lineage(
            &report.validator_evidence_packet_schema_hash,
            &report.validator_evidence_field_registry_hash,
        );
        let raw = std::fs::read_to_string(&report_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
    }

    #[test]
    fn governance_agent_round_seed_changes_request_hash_and_fails_closed() {
        let agent_dir = repo_root().join(DEFAULT_GOVERNANCE_AGENT_DIR);
        let expected_seed = default_governance_agent_round_seed_input();
        let expected = build_governance_agent_model_request_with_seed(
            &agent_dir,
            expected_seed.clone(),
        )
        .unwrap();
        let replay =
            build_governance_agent_model_request_with_seed(&agent_dir, expected_seed.clone())
                .unwrap();
        assert_eq!(expected.request_hash, replay.request_hash);
        ensure_governance_agent_model_request_seed(&expected, &expected_seed).unwrap();

        let wrong_seed = governance_agent_wrong_round_seed_input(&expected_seed);
        let wrong = build_governance_agent_model_request_with_seed(&agent_dir, wrong_seed).unwrap();
        assert_ne!(expected.request_hash, wrong.request_hash);
        assert!(ensure_governance_agent_model_request_seed(&wrong, &expected_seed).is_err());

        let stale_seed = governance_agent_stale_round_seed_input(&expected_seed);
        let stale = build_governance_agent_model_request_with_seed(&agent_dir, stale_seed).unwrap();
        assert_ne!(expected.request_hash, stale.request_hash);
        assert!(ensure_governance_agent_model_request_seed(&stale, &expected_seed).is_err());

        let mut missing_seed = expected_seed;
        missing_seed.cobalt_certificate_hash.clear();
        let error =
            build_governance_agent_model_request_with_seed(&agent_dir, missing_seed).unwrap_err();
        assert!(error.to_string().contains("cobalt_certificate_hash"));
    }

    #[test]
    fn governance_agent_gate_3_6_writes_timelock_report() {
        let root = repo_root();
        let output_file = unique_governance_agent_test_dir().join("gate-3_6.json");
        let report = governance_agent_gate_3_6(GovernanceAgentGate3_6Options {
            agent_dir: root.join(DEFAULT_GOVERNANCE_AGENT_DIR),
            output_file: output_file.clone(),
            round_seed_input: None,
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert!(report.same_seed_replays);
        assert_eq!(report.model_request_hash, report.replay_model_request_hash);
        assert_report_validator_evidence_lineage(
            &report.validator_evidence_packet_schema_hash,
            &report.validator_evidence_field_registry_hash,
        );
        assert_eq!(report.rejection_checks.len(), 3);
        assert!(report
            .rejection_checks
            .iter()
            .all(|check| !check.accepted && check.error.is_some()));
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGate3_6Report = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.round_seed, report.round_seed);
    }

    #[test]
    fn governance_agent_policy_interpreter_is_deterministic_on_frozen_evidence() {
        let root = repo_root();
        let ruleset_file = root
            .join(DEFAULT_GOVERNANCE_AGENT_DIR)
            .join("fixtures/valid_ruleset.json");
        let evidence_file = root.join(DEFAULT_GOVERNANCE_AGENT_EVIDENCE_FILE);
        let (ruleset_value, ruleset) = read_governance_ruleset_file(&ruleset_file).unwrap();
        let ruleset_hash = governance_agent_canonical_json_hash(
            "postfiat.governance_agent.ruleset_output.v1",
            &ruleset_value,
        )
        .unwrap();
        let evidence = read_governance_agent_evidence_snapshot(&evidence_file).unwrap();
        let evidence_value = serde_json::to_value(&evidence).unwrap();
        let evidence_hash = governance_agent_canonical_json_hash(
            "postfiat.governance_agent.frozen_evidence.v1",
            &evidence_value,
        )
        .unwrap();
        let policy = compile_governance_agent_ruleset_policy(&ruleset, &ruleset_hash).unwrap();
        let first =
            execute_governance_agent_policy(&ruleset, &policy, &evidence, &evidence_hash).unwrap();
        let second =
            execute_governance_agent_policy(&ruleset, &policy, &evidence, &evidence_hash).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.action, "no_op");
        assert!(first.mutations.is_empty());
        assert_eq!(first.mutation_count, 0);
        assert_eq!(first.candidate_hash.len(), 96);
        assert!(!policy.sandbox.network_access);
        assert!(!policy.sandbox.model_access);
        assert!(!policy.sandbox.filesystem_access);
        assert!(!policy.sandbox.direct_state_mutation);
    }

    #[test]
    fn governance_agent_frozen_evidence_requires_validator_packet_root() {
        let root = repo_root();
        let evidence_file = root.join(DEFAULT_GOVERNANCE_AGENT_EVIDENCE_FILE);
        let evidence = read_governance_agent_evidence_snapshot(&evidence_file).unwrap();
        assert_eq!(evidence.validator_evidence_packet_root.len(), 96);
        assert!(evidence
            .available_inputs
            .iter()
            .any(|input| input == GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND));

        let mut missing_input = evidence.clone();
        missing_input
            .available_inputs
            .retain(|input| input != GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND);
        let missing_input_error =
            validate_governance_agent_evidence_snapshot(&missing_input).unwrap_err();
        assert!(missing_input_error
            .to_string()
            .contains("missing validator evidence packet input"));

        let mut bad_root = evidence.clone();
        bad_root.validator_evidence_packet_root = "abc".to_string();
        let bad_root_error = validate_governance_agent_evidence_snapshot(&bad_root).unwrap_err();
        assert!(bad_root_error
            .to_string()
            .contains("validator_evidence_packet_root"));

        let mut missing_root_value =
            read_governance_agent_json_value(&evidence_file, "governance agent frozen evidence")
                .unwrap();
        missing_root_value
            .as_object_mut()
            .unwrap()
            .remove("validator_evidence_packet_root");
        let missing_root_error =
            serde_json::from_value::<GovernanceAgentFrozenEvidenceSnapshot>(missing_root_value)
                .unwrap_err();
        assert!(missing_root_error
            .to_string()
            .contains("validator_evidence_packet_root"));
    }

    #[test]
    fn governance_agent_policy_engine_rejects_malformed_rules_and_evidence() {
        let root = repo_root();
        let ruleset_file = root
            .join(DEFAULT_GOVERNANCE_AGENT_DIR)
            .join("fixtures/valid_ruleset.json");
        let evidence_file = root.join(DEFAULT_GOVERNANCE_AGENT_EVIDENCE_FILE);
        let (ruleset_value, _ruleset) = read_governance_ruleset_file(&ruleset_file).unwrap();
        let evidence = read_governance_agent_evidence_snapshot(&evidence_file).unwrap();
        let checks = governance_agent_gate_7_5_malformed_checks(&ruleset_value, &evidence).unwrap();
        assert_eq!(checks.len(), 4);
        assert!(checks
            .iter()
            .all(|check| !check.accepted && check.error.is_some()));
        assert!(checks.iter().any(|check| check.name == "invalid_weight"));
        assert!(checks
            .iter()
            .any(|check| check.name == "unknown_evidence_field"));
        assert!(checks
            .iter()
            .any(|check| check.name == "missing_evidence_ref"));
        assert!(checks.iter().any(|check| check.name == "unsafe_action"));
    }

    #[test]
    fn governance_agent_gate_7_5_writes_ruleset_compiler_report() {
        let root = repo_root();
        let output_file = unique_governance_agent_test_dir().join("gate-7_5.json");
        let report = governance_agent_gate_7_5(GovernanceAgentGate7_5Options {
            ruleset_file: root
                .join(DEFAULT_GOVERNANCE_AGENT_DIR)
                .join("fixtures/valid_ruleset.json"),
            evidence_file: root.join(DEFAULT_GOVERNANCE_AGENT_EVIDENCE_FILE),
            output_file: output_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert!(report.deterministic_replay);
        assert_eq!(report.registry_delta_candidate.action, "no_op");
        assert_eq!(report.registry_delta_candidate.mutation_count, 0);
        assert!(report
            .malformed_rule_checks
            .iter()
            .all(|check| !check.accepted && check.error.is_some()));
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGate7_5Report = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            parsed.registry_delta_candidate_hash,
            report.registry_delta_candidate_hash
        );
    }

    #[test]
    fn governance_agent_comparison_fixtures_cover_gate_7_6_classes() {
        let root = repo_root();
        let fixtures = read_governance_agent_comparison_fixtures(
            &root.join(DEFAULT_GOVERNANCE_AGENT_COMPARISON_DIR),
        )
        .unwrap();
        assert_eq!(fixtures.len(), 4);
        let classes = fixtures
            .iter()
            .map(|(_, fixture)| fixture.case_class.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(classes.len(), 4);
        assert!(classes.contains("high_confidence"));
        assert!(classes.contains("ambiguous"));
        assert!(classes.contains("concentration_risk"));
        assert!(classes.contains("stale_evidence"));
        assert!(fixtures
            .iter()
            .all(|(_, fixture)| !fixture.direct_baseline.authoritative));
    }

    #[test]
    fn governance_agent_gate_7_6_compares_policy_to_direct_baseline() {
        let root = repo_root();
        let output_file = unique_governance_agent_test_dir().join("gate-7_6.json");
        let report = governance_agent_gate_7_6(GovernanceAgentGate7_6Options {
            ruleset_file: root
                .join(DEFAULT_GOVERNANCE_AGENT_DIR)
                .join("fixtures/valid_ruleset.json"),
            comparison_dir: root.join(DEFAULT_GOVERNANCE_AGENT_COMPARISON_DIR),
            output_file: output_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert!(!report.direct_baseline_authoritative);
        assert_eq!(report.fixture_count, 4);
        assert_eq!(report.high_confidence_agreement_count, 1);
        assert_eq!(report.unsafe_direct_delta_rejection_count, 3);
        assert_eq!(report.false_add_count, 0);
        assert_eq!(report.false_remove_count, 0);
        assert!(report
            .comparison_checks
            .iter()
            .all(|check| check.passed));
        assert!(report
            .comparison_checks
            .iter()
            .filter(|check| check.case_class != "high_confidence")
            .all(|check| check.policy_action == "no_op" && check.policy_mutation_count == 0));
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGate7_6Report = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.fixture_count, report.fixture_count);
    }

    #[test]
    fn governance_agent_gate_8_5_records_cobalt_dry_run_without_registry_mutation() {
        let root = repo_root();
        let temp = unique_governance_agent_test_dir();
        let output_file = temp.join("gate-8_5.json");
        let replay_bundle_file = temp.join("gate-8_5.replay-bundle.json");
        let report = governance_agent_gate_8_5(GovernanceAgentGate8_5Options {
            agent_dir: root.join(DEFAULT_GOVERNANCE_AGENT_DIR),
            ruleset_file: root
                .join(DEFAULT_GOVERNANCE_AGENT_DIR)
                .join("fixtures/valid_ruleset.json"),
            evidence_file: root.join(DEFAULT_GOVERNANCE_AGENT_EVIDENCE_FILE),
            output_file: output_file.clone(),
            replay_bundle_file: replay_bundle_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert_eq!(
            report.action_mode,
            GOVERNANCE_AGENT_ACTION_MODE_DRY_RUN_VALIDATE
        );
        assert!(report.cobalt_batch_verified);
        assert!(report.dry_run_recorded);
        assert_eq!(report.governance_agent_record_count, 1);
        assert!(report.registry_unchanged);
        assert_eq!(report.registry_mutation_count, 0);
        assert!(report.stale_ruleset_rejected);
        assert!(report.wrong_bundle_rejected);
        assert!(report.missing_replay_root_rejected);
        assert!(report.replay_bundle_retrievable);
        assert_eq!(report.rejection_checks.len(), 3);
        assert!(report
            .rejection_checks
            .iter()
            .all(|check| check.rejected && check.error.is_some()));
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGate8_5Report = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.dry_run_id, report.dry_run_id);
        let replay_bundle: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&replay_bundle_file).unwrap()).unwrap();
        assert_eq!(
            replay_bundle["validator_evidence_packet_root"].as_str().unwrap(),
            report.validator_evidence_packet_root
        );
        let replay_root = governance_agent_canonical_json_hash(
            "postfiat.governance_agent.dry_run_replay_bundle.v1",
            &replay_bundle,
        )
        .unwrap();
        assert_eq!(replay_root, report.replay_bundle_root);
    }

    #[test]
    fn governance_agent_gate_9_5_applies_guarded_update_and_rolls_back() {
        let root = repo_root();
        let output_file = unique_governance_agent_test_dir().join("gate-9_5.json");
        let report = governance_agent_gate_9_5(GovernanceAgentGate9_5Options {
            agent_dir: root.join(DEFAULT_GOVERNANCE_AGENT_DIR),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            evidence_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_EVIDENCE_FILE),
            output_file: output_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert_eq!(report.candidate.action, VALIDATOR_REGISTRY_OP_ADMIT);
        assert_eq!(report.candidate.mutation_count, 1);
        assert_eq!(report.candidate.mutations[0].subject_node_id, "validator-3");
        assert_eq!(report.initial_validator_count, 3);
        assert_eq!(report.post_apply_validator_count, 4);
        assert_ne!(report.initial_registry_root, report.guarded_apply_registry_root);
        assert_eq!(
            report.guarded_apply_registry_root,
            report.post_apply_registry_root
        );
        assert_eq!(report.rollback_registry_root, report.initial_registry_root);
        assert!(report.cobalt_acceptance_verified);
        assert!(report.rollback_cobalt_acceptance_verified);
        assert!(report.registry_changes_only_after_cobalt_acceptance);
        assert!(report.max_one_add);
        assert!(report.zero_routine_removals);
        assert!(report.evidence_refs_valid);
        assert!(report.concentration_caps_passed);
        assert!(report.trust_graph_linkedness_passed);
        assert!(report.rollback_available);
        assert!(report.rollback_restored_registry);
        assert!(report.no_human_approval_after_activation);
        assert_eq!(report.rejection_checks.len(), 4);
        assert!(report
            .rejection_checks
            .iter()
            .all(|check| check.rejected && check.error.is_some()));
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGate9_5Report = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.candidate_hash, report.candidate_hash);
        assert_eq!(parsed.cobalt_update_id.len(), 96);
        assert_eq!(parsed.rollback_update_id.len(), 96);
    }

    #[test]
    fn governance_agent_gate_10_1_measures_verifier_cost_on_postfiat_artifacts() {
        let root = repo_root();
        let temp = unique_governance_agent_test_dir();
        let gate_9_5_file = write_gate_9_5_test_report(&root, &temp);
        let output_file = temp.join("gate-10_1.json");
        let report = governance_agent_gate_10_1(GovernanceAgentGate10_1Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_file,
            output_file: output_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert!(report.request_hash_recomputed);
        assert!(report.ruleset_hash_recomputed);
        assert!(report.candidate_hash_recomputed);
        assert!(report.gate_9_5_report_verified);
        assert!(report.cost_measured_not_assumed);
        assert!(!report.benchmark.generic_one_percent_claim_assumed);
        assert!(report.benchmark.full_inference_work_units > 0);
        assert!(report.benchmark.measured_verifier_work_units > 0);
        assert!(report.benchmark.verifier_to_full_cost_bps > 0);
        assert_report_validator_evidence_lineage(
            &report.validator_evidence_packet_schema_hash,
            &report.validator_evidence_field_registry_hash,
        );
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGate10_1Report = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.gate_9_5_report_hash, report.gate_9_5_report_hash);
    }

    #[test]
    fn governance_agent_gate_10_5_records_compact_receipt_and_verifier_outcomes() {
        let root = repo_root();
        let temp = unique_governance_agent_test_dir();
        let gate_9_5_file = write_gate_9_5_test_report(&root, &temp);
        let output_file = temp.join("gate-10_5.json");
        let report = governance_agent_gate_10_5(GovernanceAgentGate10_5Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_file,
            output_file: output_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert!(report.prototype_only);
        assert!(!report.consensus_critical);
        assert_eq!(report.accepted_verifier_count, 3);
        assert_eq!(report.verifier_quorum, 3);
        assert!(report.correct_receipt_accepted);
        assert!(report.incorrect_receipt_rejected);
        assert!(report.verifier_disagreement_recorded);
        assert!(report.compact_commitment.chunk_count > 0);
        assert!(report.compact_commitment.compact_bytes < report.compact_commitment.direct_embedding_bytes);
        assert_eq!(report.receipt.receipt_id.len(), 96);
        assert_eq!(report.receipt.verifier_attestation_root.len(), 96);
        assert_report_validator_evidence_lineage(
            &report.validator_evidence_packet_schema_hash,
            &report.validator_evidence_field_registry_hash,
        );
        assert!(report
            .verifier_attestations
            .iter()
            .any(|attestation| !attestation.accepted && attestation.error.is_some()));
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGate10_5Report = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.receipt.receipt_id, report.receipt.receipt_id);
    }

    #[test]
    fn governance_agent_gate_14_keeps_tp_greater_than_one_out_of_admission() {
        let root = repo_root();
        let temp = unique_governance_agent_test_dir();
        let gate_9_5_file = write_gate_9_5_test_report(&root, &temp);
        let receipt_report_file = temp.join("gate-10_5.json");
        let receipt_report = governance_agent_gate_10_5(GovernanceAgentGate10_5Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_file.clone(),
            output_file: receipt_report_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(receipt_report.verified);
        let output_file = temp.join("gate-14.json");
        let report = governance_agent_gate_14(GovernanceAgentGate14Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_file,
            receipt_report_file,
            output_file: output_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert!(report.receipt_report_verified);
        assert_eq!(report.canonical_tensor_parallelism, 1);
        assert!(!report.tp_greater_than_one_admitted);
        assert!(!report.tp_invariant_admission_ready);
        assert!(report.validator_side_shadow_path_defined);
        assert!(!report.authority_transfer_live);
        assert!(!report.shadow_plan.sidecars_live);
        assert!(!report.shadow_plan.commit_reveal_live);
        assert_report_validator_evidence_lineage(
            &report.validator_evidence_packet_schema_hash,
            &report.validator_evidence_field_registry_hash,
        );
        assert!(report
            .tp_checks
            .iter()
            .filter(|check| check.tensor_parallelism > 1)
            .all(|check| !check.admitted && check.evidence_hash.is_none()));
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGate14Report = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.canonical_output_hash, report.canonical_output_hash);
    }

    #[test]
    fn governance_agent_gate_15_rejects_adversarial_governance_escalation() {
        let root = repo_root();
        let temp = unique_governance_agent_test_dir();
        let gate_9_5_file = write_gate_9_5_test_report(&root, &temp);
        let receipt_report_file = temp.join("gate-10_5.json");
        let receipt_report = governance_agent_gate_10_5(GovernanceAgentGate10_5Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_file.clone(),
            output_file: receipt_report_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(receipt_report.verified);
        let gate_14_report_file = temp.join("gate-14.json");
        let gate_14_report = governance_agent_gate_14(GovernanceAgentGate14Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_file.clone(),
            receipt_report_file: receipt_report_file.clone(),
            output_file: gate_14_report_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(gate_14_report.verified);
        let output_file = temp.join("gate-15.json");
        let report = governance_agent_gate_15(GovernanceAgentGate15Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_file,
            receipt_report_file,
            gate_14_report_file,
            output_file: output_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        assert_eq!(report.receipt_probe_count, 3);
        assert_eq!(report.evidence_probe_count, 7);
        assert_eq!(report.disagreement_probe_count, 2);
        assert_eq!(report.authority_probe_count, 4);
        assert!(report.tampered_receipts_rejected);
        assert!(report.stale_or_missing_evidence_rejected);
        assert!(report.verifier_disagreement_shadow_only);
        assert!(report.authority_transfer_guarded);
        assert!(report.tp_greater_than_one_still_inadmissible);
        assert!(!report.sidecars_live);
        assert!(!report.commit_reveal_live);
        assert!(!report.authority_transfer_live);
        assert_report_validator_evidence_lineage(
            &report.validator_evidence_packet_schema_hash,
            &report.validator_evidence_field_registry_hash,
        );
        assert!(report
            .probes
            .iter()
            .all(|probe| probe.rejected && !probe.authority_changed && probe.error.is_some()));
        assert!(report
            .probes
            .iter()
            .any(|probe| probe.name == "tampered_verifier_attestation_root"));
        assert!(report
            .probes
            .iter()
            .any(|probe| probe.name == "tp2_admission_without_cross_tp_evidence"));
        assert!(report
            .probes
            .iter()
            .any(|probe| probe.name == "drifted_gate_10_5_packet_schema_hash"));
        assert!(report
            .probes
            .iter()
            .any(|probe| probe.name == "drifted_gate_14_field_registry_hash"));
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentGate15Report = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.probes.len(), report.probes.len());
    }

    #[test]
    fn governance_agent_evidence_lineage_audit_rejects_report_drift() {
        let root = repo_root();
        let temp = unique_governance_agent_test_dir();
        let options = write_evidence_lineage_audit_test_reports(&root, &temp);
        let report = governance_agent_evidence_lineage_audit(options.clone()).unwrap();
        assert!(report.verified);
        assert_eq!(report.report_count, 6);
        assert_eq!(report.drift_count, 0);
        assert!(report.all_reports_verified);
        assert!(report.no_network_access);
        assert!(report.no_model_access);
        assert!(report.no_filesystem_mutation);
        assert!(report.no_direct_state_mutation);
        assert!(report.zero_cobalt_registry_mutation);
        assert!(report.redaction_checked);
        assert!(report
            .reports
            .iter()
            .all(|item| item.lineage_fields_present
                && item.hashes_well_formed
                && item.matches_model_request
                && item.verified));

        let mut drifted_gate_14: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&options.gate_14_report_file).unwrap(),
        )
        .unwrap();
        drifted_gate_14["validator_evidence_field_registry_hash"] = serde_json::Value::String(
            hash_hex(
                "postfiat.governance_agent.evidence_lineage_audit.drift_test.v1",
                report
                    .validator_evidence_field_registry_hash
                    .as_bytes(),
            ),
        );
        std::fs::write(
            &options.gate_14_report_file,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&drifted_gate_14).unwrap()
            ),
        )
        .unwrap();
        let drift_report = governance_agent_evidence_lineage_audit(options).unwrap();
        assert!(!drift_report.verified);
        assert_eq!(drift_report.drift_count, 1);
        let gate_14 = drift_report
            .reports
            .iter()
            .find(|item| item.name == "gate_14")
            .unwrap();
        assert!(!gate_14.matches_model_request);
        assert!(gate_14.schema_matches_expected);
        assert!(gate_14.gate_matches_expected);
    }

    #[test]
    fn governance_agent_implementation_execution_verifies_authorized_work_item() {
        let root = repo_root();
        let output_file = unique_governance_agent_test_dir().join("implementation-execution.json");
        let report =
            governance_agent_implementation_execution(GovernanceAgentImplementationExecutionOptions {
                work_item_file: root.join(DEFAULT_GOVERNANCE_AGENT_IMPLEMENTATION_WORK_ITEM_FILE),
                output_file: output_file.clone(),
                overwrite: false,
            })
            .unwrap();
        assert!(report.verified);
        assert_eq!(
            report.work_item_id,
            GOVERNANCE_AGENT_DGA_200_WORK_ITEM_ID
        );
        assert!(report.touched_surfaces_authorized);
        assert!(report.forbidden_actions_bound);
        assert!(report.live_actions_forbidden);
        assert!(report.required_gates_bound);
        assert!(report.rollback_or_noop_fallback_defined);
        assert!(!report.provider_spend_command_executed);
        assert!(!report.paid_replay_regenerated);
        assert!(!report.live_authority_change);
        assert!(report.no_spend);
        assert_eq!(report.work_item_hash.len(), 96);
        let raw = std::fs::read_to_string(&output_file).unwrap();
        ensure_governance_agent_report_redacted(&raw).unwrap();
        let parsed: GovernanceAgentImplementationExecutionReport =
            serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.work_item_hash, report.work_item_hash);
    }

    #[test]
    fn governance_agent_implementation_execution_rejects_scope_and_live_action_expansion() {
        let root = repo_root();
        let fixture =
            root.join(DEFAULT_GOVERNANCE_AGENT_IMPLEMENTATION_WORK_ITEM_FILE);
        let mut work_item: GovernanceAgentImplementationWorkItem =
            serde_json::from_str(&std::fs::read_to_string(&fixture).unwrap()).unwrap();
        let temp = unique_governance_agent_test_dir();

        work_item.touched_surfaces.push("Cargo.toml".to_string());
        let unauthorized_file = temp.join("unauthorized-surface.json");
        std::fs::write(
            &unauthorized_file,
            serde_json::to_string_pretty(&work_item).unwrap(),
        )
        .unwrap();
        let unauthorized_report = temp.join("unauthorized-report.json");
        let error =
            governance_agent_implementation_execution(GovernanceAgentImplementationExecutionOptions {
                work_item_file: unauthorized_file,
                output_file: unauthorized_report.clone(),
                overwrite: false,
            })
            .unwrap_err();
        assert!(error.to_string().contains("implementation execution failed"));
        let unauthorized: GovernanceAgentImplementationExecutionReport =
            serde_json::from_str(&std::fs::read_to_string(&unauthorized_report).unwrap()).unwrap();
        assert!(!unauthorized.verified);
        assert!(!unauthorized.touched_surfaces_authorized);

        let mut live_action: GovernanceAgentImplementationWorkItem =
            serde_json::from_str(&std::fs::read_to_string(&fixture).unwrap()).unwrap();
        live_action.live_action_flags.authority_transfer = true;
        let live_action_file = temp.join("live-action.json");
        std::fs::write(
            &live_action_file,
            serde_json::to_string_pretty(&live_action).unwrap(),
        )
        .unwrap();
        let live_action_report = temp.join("live-action-report.json");
        let error =
            governance_agent_implementation_execution(GovernanceAgentImplementationExecutionOptions {
                work_item_file: live_action_file,
                output_file: live_action_report.clone(),
                overwrite: false,
            })
            .unwrap_err();
        assert!(error.to_string().contains("implementation execution failed"));
        let live_action: GovernanceAgentImplementationExecutionReport =
            serde_json::from_str(&std::fs::read_to_string(&live_action_report).unwrap()).unwrap();
        assert!(!live_action.verified);
        assert!(!live_action.live_actions_forbidden);
    }

    fn write_gate_9_5_test_report(root: &Path, temp: &Path) -> PathBuf {
        let output_file = temp.join("gate-9_5.json");
        let report = governance_agent_gate_9_5(GovernanceAgentGate9_5Options {
            agent_dir: root.join(DEFAULT_GOVERNANCE_AGENT_DIR),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            evidence_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_EVIDENCE_FILE),
            output_file: output_file.clone(),
            overwrite: false,
        })
        .unwrap();
        assert!(report.verified);
        output_file
    }

    fn write_evidence_lineage_audit_test_reports(
        root: &Path,
        temp: &Path,
    ) -> GovernanceAgentEvidenceLineageAuditOptions {
        let outputs_dir = temp.join("gate-3_5-outputs");
        std::fs::create_dir_all(&outputs_dir).unwrap();
        let fixture = root
            .join(DEFAULT_GOVERNANCE_AGENT_DIR)
            .join("fixtures/valid_ruleset.json");
        for index in 1..=2 {
            std::fs::copy(
                &fixture,
                outputs_dir.join(format!("model_output_{index:04}.json")),
            )
            .unwrap();
        }
        let gate_3_5_report_file = temp.join("gate-3_5.json");
        governance_agent_gate_3_5(GovernanceAgentGate3_5Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            outputs_dir,
            output_file: gate_3_5_report_file.clone(),
            expected_count: 2,
            round_seed_input: None,
            overwrite: false,
        })
        .unwrap();

        let gate_3_6_report_file = temp.join("gate-3_6.json");
        governance_agent_gate_3_6(GovernanceAgentGate3_6Options {
            agent_dir: root.join(DEFAULT_GOVERNANCE_AGENT_DIR),
            output_file: gate_3_6_report_file.clone(),
            round_seed_input: None,
            overwrite: false,
        })
        .unwrap();

        let gate_9_5_report_file = write_gate_9_5_test_report(root, temp);
        let gate_10_1_report_file = temp.join("gate-10_1.json");
        governance_agent_gate_10_1(GovernanceAgentGate10_1Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_report_file.clone(),
            output_file: gate_10_1_report_file.clone(),
            overwrite: false,
        })
        .unwrap();

        let receipt_report_file = temp.join("gate-10_5.json");
        governance_agent_gate_10_5(GovernanceAgentGate10_5Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_report_file.clone(),
            output_file: receipt_report_file.clone(),
            overwrite: false,
        })
        .unwrap();

        let gate_14_report_file = temp.join("gate-14.json");
        governance_agent_gate_14(GovernanceAgentGate14Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file: gate_9_5_report_file.clone(),
            receipt_report_file: receipt_report_file.clone(),
            output_file: gate_14_report_file.clone(),
            overwrite: false,
        })
        .unwrap();

        let gate_15_report_file = temp.join("gate-15.json");
        governance_agent_gate_15(GovernanceAgentGate15Options {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            ruleset_file: root.join(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            gate_9_5_report_file,
            receipt_report_file: receipt_report_file.clone(),
            gate_14_report_file: gate_14_report_file.clone(),
            output_file: gate_15_report_file.clone(),
            overwrite: false,
        })
        .unwrap();

        GovernanceAgentEvidenceLineageAuditOptions {
            model_request_file: root.join(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            gate_3_5_report_file,
            gate_3_6_report_file,
            gate_10_1_report_file,
            receipt_report_file,
            gate_14_report_file,
            gate_15_report_file,
        }
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn assert_report_validator_evidence_lineage(
        validator_evidence_packet_schema_hash: &str,
        validator_evidence_field_registry_hash: &str,
    ) {
        validate_governance_agent_hash_hex(
            "test validator_evidence_packet_schema_hash",
            validator_evidence_packet_schema_hash,
        )
        .unwrap();
        validate_governance_agent_hash_hex(
            "test validator_evidence_field_registry_hash",
            validator_evidence_field_registry_hash,
        )
        .unwrap();
        let request =
            build_governance_agent_model_request(&repo_root().join(DEFAULT_GOVERNANCE_AGENT_DIR))
                .unwrap();
        assert_eq!(
            validator_evidence_packet_schema_hash,
            request
                .governed_inputs
                .validator_evidence_packet_schema_hash
                .as_str()
        );
        assert_eq!(
            validator_evidence_field_registry_hash,
            request
                .governed_inputs
                .validator_evidence_field_registry_hash
                .as_str()
        );
    }

    fn unique_governance_agent_test_dir() -> PathBuf {
        let sequence = GOVERNANCE_AGENT_TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let path = std::env::temp_dir().join(format!(
            "postfiat-governance-agent-gate-test-{}-{millis}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
