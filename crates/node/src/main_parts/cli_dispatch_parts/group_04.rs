fn run_cli_group_04(command: &str, flags: &[String]) -> Result<(), String> {
    match command {
        "governance-amendment-replay-verify" => {
            let bundle_file = flag_value(flags, "--bundle-file").ok_or("missing --bundle-file")?;
            let report =
                verify_governance_amendment_replay_bundle(GovernanceAmendmentReplayVerifyOptions {
                    bundle_file: PathBuf::from(bundle_file),
                })
                .map_err(|error| format!("governance-amendment-replay-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance amendment replay verification serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-replay-build" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let genesis_bundle_file = flag_value(flags, "--genesis-bundle-file").map(PathBuf::from);
            let previous_registry_file = flag_value(flags, "--previous-registry-file")
                .ok_or("missing --previous-registry-file")?;
            let update_file = flag_value(flags, "--update-file").ok_or("missing --update-file")?;
            let new_registry_file =
                flag_value(flags, "--new-registry-file").ok_or("missing --new-registry-file")?;
            let amendment_replay_bundle_file =
                flag_value(flags, "--amendment-replay-bundle-file").map(PathBuf::from);
            let governance_batch_file =
                flag_value(flags, "--governance-batch-file").map(PathBuf::from);
            let post_change_block_file =
                flag_value(flags, "--post-change-block-file").map(PathBuf::from);
            let post_change_batch_file =
                flag_value(flags, "--post-change-batch-file").map(PathBuf::from);
            let post_change_certificate_file =
                flag_value(flags, "--post-change-certificate-file").map(PathBuf::from);
            let output_file = flag_value(flags, "--output").ok_or("missing --output")?;
            let package = create_governance_replay_package(GovernanceReplayBuildOptions {
                data_dir: PathBuf::from(data_dir),
                genesis_bundle_file,
                previous_registry_file: PathBuf::from(previous_registry_file),
                update_file: PathBuf::from(update_file),
                new_registry_file: PathBuf::from(new_registry_file),
                amendment_replay_bundle_file,
                governance_batch_file,
                post_change_block_file,
                post_change_batch_file,
                post_change_certificate_file,
                output_file: PathBuf::from(output_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-replay-build failed: {error}"))?;
            let json = serde_json::to_string_pretty(&package).map_err(|error| {
                format!("governance replay package serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "operator-manifest-create" => {
            let master_key_file =
                flag_value(flags, "--master-key-file").ok_or("missing --master-key-file")?;
            let chain_id = flag_value(flags, "--chain-id").ok_or("missing --chain-id")?;
            let network = flag_value(flags, "--network").ok_or("missing --network")?;
            let validator_id =
                flag_value(flags, "--validator-id").ok_or("missing --validator-id")?;
            let hot_public_key_hex =
                flag_value(flags, "--hot-public-key-hex").ok_or("missing --hot-public-key-hex")?;
            let operator = flag_value(flags, "--operator").ok_or("missing --operator")?;
            let contact = flag_value(flags, "--contact").ok_or("missing --contact")?;
            let provider_group =
                flag_value(flags, "--provider-group").ok_or("missing --provider-group")?;
            let region_group =
                flag_value(flags, "--region-group").ok_or("missing --region-group")?;
            let jurisdiction_group =
                flag_value(flags, "--jurisdiction-group").ok_or("missing --jurisdiction-group")?;
            let legal_domain_group =
                flag_value(flags, "--legal-domain-group").ok_or("missing --legal-domain-group")?;
            let funding_domain_group = flag_value(flags, "--funding-domain-group")
                .ok_or("missing --funding-domain-group")?;
            let rotation_state = flag_value(flags, "--rotation-state").unwrap_or("active");
            let effective_height = flag_value(flags, "--effective-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--effective-height must be a u64".to_string())?;
            let trust_graph_root = flag_value(flags, "--trust-graph-root").map(str::to_string);
            let trust_graph_version = flag_value(flags, "--trust-graph-version")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--trust-graph-version must be a u64".to_string())
                })
                .transpose()?;
            let trust_view_id = flag_value(flags, "--trust-view-id").map(str::to_string);
            let trust_view_version = flag_value(flags, "--trust-view-version")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--trust-view-version must be a u64".to_string())
                })
                .transpose()?;
            let output_file = flag_value(flags, "--output").ok_or("missing --output")?;
            let manifest = create_operator_manifest(OperatorManifestCreateOptions {
                master_key_file: PathBuf::from(master_key_file),
                chain_id: chain_id.to_string(),
                network: network.to_string(),
                validator_id: validator_id.to_string(),
                hot_public_key_hex: hot_public_key_hex.to_string(),
                operator: operator.to_string(),
                contact: contact.to_string(),
                provider_group: provider_group.to_string(),
                region_group: region_group.to_string(),
                jurisdiction_group: jurisdiction_group.to_string(),
                legal_domain_group: legal_domain_group.to_string(),
                funding_domain_group: funding_domain_group.to_string(),
                rotation_state: rotation_state.to_string(),
                effective_height,
                trust_graph_root,
                trust_graph_version,
                trust_view_id,
                trust_view_version,
                output_file: PathBuf::from(output_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("operator-manifest-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&manifest)
                .map_err(|error| format!("operator manifest serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "operator-manifest-verify" => {
            let manifest_file =
                flag_value(flags, "--manifest-file").ok_or("missing --manifest-file")?;
            let report = verify_operator_manifest(OperatorManifestVerifyOptions {
                manifest_file: PathBuf::from(manifest_file),
            })
            .map_err(|error| format!("operator-manifest-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("operator manifest serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "governance-genesis-bundle" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let manifest_dir =
                flag_value(flags, "--manifest-dir").ok_or("missing --manifest-dir")?;
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let quorum = flag_value(flags, "--quorum")
                .ok_or("missing --quorum")?
                .parse::<usize>()
                .map_err(|_| "--quorum must be a usize".to_string())?;
            let network = flag_value(flags, "--network")
                .ok_or("missing --network")?
                .to_string();
            let output_file = flag_value(flags, "--output").ok_or("missing --output")?;
            let bundle = create_governance_genesis_bundle(GovernanceGenesisBundleOptions {
                data_dir: PathBuf::from(data_dir),
                manifest_dir: PathBuf::from(manifest_dir),
                validators,
                quorum,
                network,
                output_file: PathBuf::from(output_file),
            })
            .map_err(|error| format!("governance-genesis-bundle failed: {error}"))?;
            let json = serde_json::to_string_pretty(&bundle).map_err(|error| {
                format!("governance genesis bundle serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-genesis-verify" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let bundle_file = flag_value(flags, "--bundle-file").ok_or("missing --bundle-file")?;
            let report = verify_governance_genesis_bundle(GovernanceGenesisVerifyOptions {
                data_dir: PathBuf::from(data_dir),
                bundle_file: PathBuf::from(bundle_file),
            })
            .map_err(|error| format!("governance-genesis-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance genesis verify serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate1-5" => {
            let agent_dir =
                flag_value(flags, "--agent-dir").unwrap_or(DEFAULT_GOVERNANCE_AGENT_DIR);
            let output_file =
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_1_5_REPORT);
            let report = governance_agent_gate_1_5(GovernanceAgentGateOptions {
                agent_dir: PathBuf::from(agent_dir),
                output_file: PathBuf::from(output_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate1-5 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 1.5 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-model-request" => {
            let agent_dir =
                flag_value(flags, "--agent-dir").unwrap_or(DEFAULT_GOVERNANCE_AGENT_DIR);
            let output_file = flag_value(flags, "--output")
                .unwrap_or(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE);
            let round_seed_input = governance_agent_round_seed_input_from_optional_parts(
                flag_value(flags, "--cobalt-certificate-hash").map(str::to_string),
                flag_value(flags, "--round-id").map(str::to_string),
                flag_value(flags, "--round-domain").map(str::to_string),
            )
            .map_err(|error| format!("governance-agent-model-request seed failed: {error}"))?;
            let request = governance_agent_model_request(GovernanceAgentModelRequestOptions {
                agent_dir: PathBuf::from(agent_dir),
                output_file: PathBuf::from(output_file),
                round_seed_input,
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-model-request failed: {error}"))?;
            let json = serde_json::to_string_pretty(&request).map_err(|error| {
                format!("governance agent model request serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate3-5" => {
            let model_request_file = flag_value(flags, "--model-request")
                .unwrap_or(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE);
            let outputs_dir = flag_value(flags, "--outputs-dir").ok_or("missing --outputs-dir")?;
            let output_file =
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_3_5_REPORT);
            let expected_count = flag_value(flags, "--expected-count")
                .unwrap_or("50")
                .parse::<usize>()
                .map_err(|_| "--expected-count must be a usize".to_string())?;
            let round_seed_input = governance_agent_round_seed_input_from_optional_parts(
                flag_value(flags, "--cobalt-certificate-hash").map(str::to_string),
                flag_value(flags, "--round-id").map(str::to_string),
                flag_value(flags, "--round-domain").map(str::to_string),
            )
            .map_err(|error| format!("governance-agent-gate3-5 seed failed: {error}"))?;
            let report = governance_agent_gate_3_5(GovernanceAgentGate3_5Options {
                model_request_file: PathBuf::from(model_request_file),
                outputs_dir: PathBuf::from(outputs_dir),
                output_file: PathBuf::from(output_file),
                expected_count,
                round_seed_input,
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate3-5 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 3.5 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate3-6" => {
            let agent_dir =
                flag_value(flags, "--agent-dir").unwrap_or(DEFAULT_GOVERNANCE_AGENT_DIR);
            let output_file =
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_3_6_REPORT);
            let round_seed_input = governance_agent_round_seed_input_from_optional_parts(
                flag_value(flags, "--cobalt-certificate-hash").map(str::to_string),
                flag_value(flags, "--round-id").map(str::to_string),
                flag_value(flags, "--round-domain").map(str::to_string),
            )
            .map_err(|error| format!("governance-agent-gate3-6 seed failed: {error}"))?;
            let report = governance_agent_gate_3_6(GovernanceAgentGate3_6Options {
                agent_dir: PathBuf::from(agent_dir),
                output_file: PathBuf::from(output_file),
                round_seed_input,
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate3-6 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 3.6 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate7-5" => {
            let ruleset_file = flag_value(flags, "--ruleset")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(DEFAULT_GOVERNANCE_AGENT_DIR).join("fixtures/valid_ruleset.json")
                });
            let evidence_file =
                flag_value(flags, "--evidence").unwrap_or(DEFAULT_GOVERNANCE_AGENT_EVIDENCE_FILE);
            let output_file =
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_7_5_REPORT);
            let report = governance_agent_gate_7_5(GovernanceAgentGate7_5Options {
                ruleset_file,
                evidence_file: PathBuf::from(evidence_file),
                output_file: PathBuf::from(output_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate7-5 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 7.5 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate7-6" => {
            let ruleset_file = flag_value(flags, "--ruleset")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(DEFAULT_GOVERNANCE_AGENT_DIR).join("fixtures/valid_ruleset.json")
                });
            let comparison_dir = flag_value(flags, "--comparison-dir")
                .unwrap_or(DEFAULT_GOVERNANCE_AGENT_COMPARISON_DIR);
            let output_file =
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_7_6_REPORT);
            let report = governance_agent_gate_7_6(GovernanceAgentGate7_6Options {
                ruleset_file,
                comparison_dir: PathBuf::from(comparison_dir),
                output_file: PathBuf::from(output_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate7-6 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 7.6 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate8-5" => {
            let agent_dir = PathBuf::from(
                flag_value(flags, "--agent-dir").unwrap_or(DEFAULT_GOVERNANCE_AGENT_DIR),
            );
            let ruleset_file = flag_value(flags, "--ruleset")
                .map(PathBuf::from)
                .unwrap_or_else(|| agent_dir.join("fixtures/valid_ruleset.json"));
            let evidence_file = PathBuf::from(
                flag_value(flags, "--evidence")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_EVIDENCE_FILE),
            );
            let output_file = PathBuf::from(
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_8_5_REPORT),
            );
            let replay_bundle_file = PathBuf::from(
                flag_value(flags, "--replay-bundle-output")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_8_5_REPLAY_BUNDLE),
            );
            let report = governance_agent_gate_8_5(GovernanceAgentGate8_5Options {
                agent_dir,
                ruleset_file,
                evidence_file,
                output_file,
                replay_bundle_file,
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate8-5 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 8.5 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate9-5" => {
            let agent_dir = PathBuf::from(
                flag_value(flags, "--agent-dir").unwrap_or(DEFAULT_GOVERNANCE_AGENT_DIR),
            );
            let ruleset_file = PathBuf::from(
                flag_value(flags, "--ruleset")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            );
            let evidence_file = PathBuf::from(
                flag_value(flags, "--evidence").unwrap_or(DEFAULT_GOVERNANCE_AGENT_EVIDENCE_FILE),
            );
            let output_file = PathBuf::from(
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_9_5_REPORT),
            );
            let report = governance_agent_gate_9_5(GovernanceAgentGate9_5Options {
                agent_dir,
                ruleset_file,
                evidence_file,
                output_file,
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate9-5 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 9.5 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate10-1" => {
            let model_request_file = PathBuf::from(
                flag_value(flags, "--model-request")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            );
            let ruleset_file = PathBuf::from(
                flag_value(flags, "--ruleset")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            );
            let gate_9_5_report_file = PathBuf::from(
                flag_value(flags, "--gate-9_5-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_9_5_REPORT),
            );
            let output_file = PathBuf::from(
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_10_1_REPORT),
            );
            let report = governance_agent_gate_10_1(GovernanceAgentGate10_1Options {
                model_request_file,
                ruleset_file,
                gate_9_5_report_file,
                output_file,
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate10-1 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 10.1 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate10-5" => {
            let model_request_file = PathBuf::from(
                flag_value(flags, "--model-request")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            );
            let ruleset_file = PathBuf::from(
                flag_value(flags, "--ruleset")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            );
            let gate_9_5_report_file = PathBuf::from(
                flag_value(flags, "--gate-9_5-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_9_5_REPORT),
            );
            let output_file = PathBuf::from(
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_10_5_REPORT),
            );
            let report = governance_agent_gate_10_5(GovernanceAgentGate10_5Options {
                model_request_file,
                ruleset_file,
                gate_9_5_report_file,
                output_file,
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate10-5 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 10.5 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate14" => {
            let model_request_file = PathBuf::from(
                flag_value(flags, "--model-request")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            );
            let ruleset_file = PathBuf::from(
                flag_value(flags, "--ruleset")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            );
            let gate_9_5_report_file = PathBuf::from(
                flag_value(flags, "--gate-9_5-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_9_5_REPORT),
            );
            let receipt_report_file = PathBuf::from(
                flag_value(flags, "--receipt-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_10_5_REPORT),
            );
            let output_file = PathBuf::from(
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_14_REPORT),
            );
            let report = governance_agent_gate_14(GovernanceAgentGate14Options {
                model_request_file,
                ruleset_file,
                gate_9_5_report_file,
                receipt_report_file,
                output_file,
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate14 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 14 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-gate15" => {
            let model_request_file = PathBuf::from(
                flag_value(flags, "--model-request")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            );
            let ruleset_file = PathBuf::from(
                flag_value(flags, "--ruleset")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE),
            );
            let gate_9_5_report_file = PathBuf::from(
                flag_value(flags, "--gate-9_5-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_9_5_REPORT),
            );
            let receipt_report_file = PathBuf::from(
                flag_value(flags, "--receipt-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_10_5_REPORT),
            );
            let gate_14_report_file = PathBuf::from(
                flag_value(flags, "--gate-14-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_14_REPORT),
            );
            let output_file = PathBuf::from(
                flag_value(flags, "--output").unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_15_REPORT),
            );
            let report = governance_agent_gate_15(GovernanceAgentGate15Options {
                model_request_file,
                ruleset_file,
                gate_9_5_report_file,
                receipt_report_file,
                gate_14_report_file,
                output_file,
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("governance-agent-gate15 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent Gate 15 report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-agent-evidence-lineage-audit" => {
            let model_request_file = PathBuf::from(
                flag_value(flags, "--model-request")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE),
            );
            let gate_3_5_report_file = PathBuf::from(
                flag_value(flags, "--gate-3_5-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_3_5_REPORT),
            );
            let gate_3_6_report_file = PathBuf::from(
                flag_value(flags, "--gate-3_6-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_3_6_REPORT),
            );
            let gate_10_1_report_file = PathBuf::from(
                flag_value(flags, "--gate-10_1-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_10_1_REPORT),
            );
            let receipt_report_file = PathBuf::from(
                flag_value(flags, "--gate-10_5-report")
                    .or_else(|| flag_value(flags, "--receipt-report"))
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_10_5_REPORT),
            );
            let gate_14_report_file = PathBuf::from(
                flag_value(flags, "--gate-14-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_14_REPORT),
            );
            let gate_15_report_file = PathBuf::from(
                flag_value(flags, "--gate-15-report")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_GATE_15_REPORT),
            );
            let report = governance_agent_evidence_lineage_audit(
                GovernanceAgentEvidenceLineageAuditOptions {
                    model_request_file,
                    gate_3_5_report_file,
                    gate_3_6_report_file,
                    gate_10_1_report_file,
                    receipt_report_file,
                    gate_14_report_file,
                    gate_15_report_file,
                },
            )
            .map_err(|error| format!("governance-agent-evidence-lineage-audit failed: {error}"))?;
            let verified = report.verified;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance agent evidence lineage audit serialization failed: {error}")
            })?;
            println!("{json}");
            if !verified {
                return Err(
                    "governance-agent-evidence-lineage-audit found lineage drift".to_string(),
                );
            }
            Ok(())
        }
        "governance-agent-implementation-execution" => {
            let work_item_file = PathBuf::from(
                flag_value(flags, "--work-item")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_IMPLEMENTATION_WORK_ITEM_FILE),
            );
            let output_file = PathBuf::from(
                flag_value(flags, "--output")
                    .unwrap_or(DEFAULT_GOVERNANCE_AGENT_IMPLEMENTATION_EXECUTION_REPORT),
            );
            let report = governance_agent_implementation_execution(
                GovernanceAgentImplementationExecutionOptions {
                    work_item_file,
                    output_file,
                    overwrite: flag_present(flags, "--overwrite"),
                },
            )
            .map_err(|error| {
                format!("governance-agent-implementation-execution failed: {error}")
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!(
                    "governance agent implementation execution report serialization failed: {error}"
                )
            })?;
            println!("{json}");
            Ok(())
        }
        "apply-amendment" => {
            require_direct_state_enabled("apply-amendment")?;
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let amendment = apply_amendment(ApplyAmendmentOptions {
                data_dir: PathBuf::from(data_dir),
                amendment_file: PathBuf::from(amendment_file),
            })
            .map_err(|error| format!("apply-amendment failed: {error}"))?;
            let json = serde_json::to_string_pretty(&amendment)
                .map_err(|error| format!("amendment serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "governance-batch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let amendment_file = flag_value(flags, "--amendment-file").map(PathBuf::from);
            let registry_update_file =
                flag_value(flags, "--registry-update-file").map(PathBuf::from);
            if amendment_file.is_none() && registry_update_file.is_none() {
                return Err("missing --amendment-file or --registry-update-file".to_string());
            }
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_governance_batch(GovernanceBatchOptions {
                data_dir: PathBuf::from(data_dir),
                amendment_file,
                registry_update_file,
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("governance-batch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "fastswap-governance-bootstrap" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let activation_height = flag_value(flags, "--governance-activation-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--governance-activation-height must be a u64".to_string())?;
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let payload_file =
                flag_value(flags, "--payload-file").ok_or("missing --payload-file")?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_fastswap_governance_bootstrap(
                FastSwapGovernanceBootstrapOptions {
                    data_dir: PathBuf::from(data_dir),
                    validators,
                    support,
                    activation_height,
                    veto_until_height,
                    paused: flag_present(flags, "--paused"),
                    payload_file: PathBuf::from(payload_file),
                    amendment_file: PathBuf::from(amendment_file),
                    batch_file: PathBuf::from(batch_file),
                },
            )
            .map_err(|error| format!("fastswap-governance-bootstrap failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("FastSwap bootstrap serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "fastswap-governance-bootstrap-assemble" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let payload_file =
                flag_value(flags, "--payload-file").ok_or("missing --payload-file")?;
            let signed_amendment_file = flag_value(flags, "--signed-amendment-file")
                .ok_or("missing --signed-amendment-file")?;
            let proposal_slot = flag_value(flags, "--proposal-slot")
                .ok_or("missing --proposal-slot")?
                .parse::<u64>()
                .map_err(|_| "--proposal-slot must be a u64".to_string())?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = assemble_signed_fastswap_governance_bootstrap(
                SignedFastSwapGovernanceBootstrapOptions {
                    data_dir: PathBuf::from(data_dir),
                    payload_file: PathBuf::from(payload_file),
                    signed_amendment_file: PathBuf::from(signed_amendment_file),
                    proposal_slot,
                    batch_file: PathBuf::from(batch_file),
                },
            )
            .map_err(|error| {
                format!("fastswap-governance-bootstrap-assemble failed: {error}")
            })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&batch).map_err(|error| {
                    format!("FastSwap bootstrap serialization failed: {error}")
                })?
            );
            Ok(())
        }
        "fastpay-recovery-governance-bootstrap" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let payload_file =
                flag_value(flags, "--payload-file").ok_or("missing --payload-file")?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_fastpay_recovery_governance_bootstrap(
                FastPayRecoveryGovernanceBootstrapOptions {
                    data_dir: PathBuf::from(data_dir),
                    validators,
                    support,
                    veto_until_height,
                    payload_file: PathBuf::from(payload_file),
                    amendment_file: PathBuf::from(amendment_file),
                    batch_file: PathBuf::from(batch_file),
                },
            )
            .map_err(|error| {
                format!("fastpay-recovery-governance-bootstrap failed: {error}")
            })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&batch).map_err(|error| {
                    format!("FastPay recovery bootstrap serialization failed: {error}")
                })?
            );
            Ok(())
        }
        "fastpay-recovery-governance-bootstrap-assemble" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let payload_file =
                flag_value(flags, "--payload-file").ok_or("missing --payload-file")?;
            let signed_amendment_file = flag_value(flags, "--signed-amendment-file")
                .ok_or("missing --signed-amendment-file")?;
            let proposal_slot = flag_value(flags, "--proposal-slot")
                .ok_or("missing --proposal-slot")?
                .parse::<u64>()
                .map_err(|_| "--proposal-slot must be a u64".to_string())?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = assemble_signed_fastpay_recovery_governance_bootstrap(
                SignedFastPayRecoveryGovernanceBootstrapOptions {
                    data_dir: PathBuf::from(data_dir),
                    payload_file: PathBuf::from(payload_file),
                    signed_amendment_file: PathBuf::from(signed_amendment_file),
                    proposal_slot,
                    batch_file: PathBuf::from(batch_file),
                },
            )
            .map_err(|error| {
                format!("fastpay-recovery-governance-bootstrap-assemble failed: {error}")
            })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&batch).map_err(|error| {
                    format!("FastPay recovery bootstrap serialization failed: {error}")
                })?
            );
            Ok(())
        }
        "vault-bridge-route-profile-governance" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let profile_file =
                flag_value(flags, "--profile-file").ok_or("missing --profile-file")?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_vault_bridge_route_profile_governance(
                VaultBridgeRouteProfileGovernanceOptions {
                    data_dir: PathBuf::from(data_dir),
                    validators,
                    support,
                    veto_until_height,
                    profile_file: PathBuf::from(profile_file),
                    tier4_finality_bootstrap_file: flag_value(
                        flags,
                        "--tier4-finality-bootstrap-file",
                    )
                    .map(PathBuf::from),
                    amendment_file: PathBuf::from(amendment_file),
                    batch_file: PathBuf::from(batch_file),
                },
            )
            .map_err(|error| {
                format!("vault-bridge-route-profile-governance failed: {error}")
            })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&batch)
                    .map_err(|error| format!("route batch serialization failed: {error}"))?
            );
            Ok(())
        }
        "vault-bridge-route-profile-governance-assemble" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let profile_file =
                flag_value(flags, "--profile-file").ok_or("missing --profile-file")?;
            let signed_amendment_file = flag_value(flags, "--signed-amendment-file")
                .ok_or("missing --signed-amendment-file")?;
            let proposal_slot = flag_value(flags, "--proposal-slot")
                .ok_or("missing --proposal-slot")?
                .parse::<u64>()
                .map_err(|_| "--proposal-slot must be a u64".to_string())?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = assemble_signed_vault_bridge_route_profile_governance(
                SignedVaultBridgeRouteProfileGovernanceOptions {
                    data_dir: PathBuf::from(data_dir),
                    profile_file: PathBuf::from(profile_file),
                    tier4_finality_bootstrap_file: flag_value(
                        flags,
                        "--tier4-finality-bootstrap-file",
                    )
                    .map(PathBuf::from),
                    signed_amendment_file: PathBuf::from(signed_amendment_file),
                    proposal_slot,
                    batch_file: PathBuf::from(batch_file),
                },
            )
            .map_err(|error| {
                format!("vault-bridge-route-profile-governance-assemble failed: {error}")
            })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&batch)
                    .map_err(|error| format!("signed route batch serialization failed: {error}"))?
            );
            Ok(())
        }
        "apply-governance-batch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let certificate_file = flag_value(flags, "--certificate-file").map(PathBuf::from);
            let receipts = apply_governance_batch(ApplyBatchOptions {
                data_dir: PathBuf::from(data_dir),
                batch_file: PathBuf::from(batch_file),
                certificate_file,
            })
            .map_err(|error| format!("apply-governance-batch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&receipts)
                .map_err(|error| format!("receipt serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "account" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let address = flag_value(flags, "--address").ok_or("missing --address")?;
            let account = account(
                NodeOptions {
                    data_dir: PathBuf::from(data_dir),
                },
                address,
            )
            .map_err(|error| format!("account failed: {error}"))?;
            let json = serde_json::to_string_pretty(&account)
                .map_err(|error| format!("account serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "account-tx" | "account_tx" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let address = flag_value(flags, "--address").ok_or("missing --address")?;
            let from_height = parse_optional_u64_flag(flags, "--from-height")?;
            let to_height = parse_optional_u64_flag(flags, "--to-height")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_tx(AccountTxQueryOptions {
                data_dir: PathBuf::from(data_dir),
                address: address.to_string(),
                from_height,
                to_height,
                limit,
            })
            .map_err(|error| format!("account-tx failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("account_tx serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "account-tx-index-build" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = rebuild_account_tx_index(AccountTxIndexOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("account-tx-index-build failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("account tx index build serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "account-tx-index-status" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = account_tx_index_status(AccountTxIndexOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("account-tx-index-status failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("account tx index status serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transfer-fee-quote" | "transfer_fee_quote" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let from = flag_value(flags, "--from").ok_or("missing --from")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = transfer_fee_quote(TransferFeeQuoteOptions {
                data_dir: PathBuf::from(data_dir),
                from: from.to_string(),
                to: to.to_string(),
                amount,
                sequence,
                memo_type: flag_value(flags, "--memo-type").map(ToString::to_string),
                memo_format: flag_value(flags, "--memo-format").map(ToString::to_string),
                memo_data: flag_value(flags, "--memo-data").map(ToString::to_string),
            })
            .map_err(|error| format!("transfer-fee-quote failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transfer fee quote serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "asset-fee-quote" | "asset_fee_quote" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = asset_fee_quote(AssetFeeQuoteOptions {
                data_dir: PathBuf::from(data_dir),
                source: source.to_string(),
                operation_json: operation_json.to_string(),
                sequence,
            })
            .map_err(|error| format!("asset-fee-quote failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("asset fee quote serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "escrow-fee-quote" | "escrow_fee_quote" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = escrow_fee_quote(EscrowFeeQuoteOptions {
                data_dir: PathBuf::from(data_dir),
                source: source.to_string(),
                operation_json: operation_json.to_string(),
                sequence,
            })
            .map_err(|error| format!("escrow-fee-quote failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("escrow fee quote serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "nft-fee-quote" | "nft_fee_quote" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = nft_fee_quote(NftFeeQuoteOptions {
                data_dir: PathBuf::from(data_dir),
                source: source.to_string(),
                operation_json: operation_json.to_string(),
                sequence,
            })
            .map_err(|error| format!("nft-fee-quote failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("nft fee quote serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "offer-fee-quote" | "offer_fee_quote" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = offer_fee_quote(OfferFeeQuoteOptions {
                data_dir: PathBuf::from(data_dir),
                source: source.to_string(),
                operation_json: operation_json.to_string(),
                sequence,
            })
            .map_err(|error| format!("offer-fee-quote failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("offer fee quote serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "offer-info" | "offer_info" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let offer_id = flag_value(flags, "--offer-id").ok_or("missing --offer-id")?;
            let report = offer_info(OfferInfoOptions {
                data_dir: PathBuf::from(data_dir),
                offer_id: offer_id.to_string(),
            })
            .map_err(|error| format!("offer-info failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("offer info serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "account-offers" | "account_offers" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_offers(AccountOffersOptions {
                data_dir: PathBuf::from(data_dir),
                account: account.to_string(),
                state: flag_value(flags, "--state").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("account-offers failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("account offers serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "book-offers" | "book_offers" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let taker_gets_asset_id = flag_value(flags, "--taker-gets-asset-id")
                .ok_or("missing --taker-gets-asset-id")?;
            let taker_pays_asset_id = flag_value(flags, "--taker-pays-asset-id")
                .ok_or("missing --taker-pays-asset-id")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = book_offers(BookOffersOptions {
                data_dir: PathBuf::from(data_dir),
                taker_gets_asset_id: taker_gets_asset_id.to_string(),
                taker_pays_asset_id: taker_pays_asset_id.to_string(),
                limit,
            })
            .map_err(|error| format!("book-offers failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("book offers serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "atomic-settlement-template" | "atomic_settlement_template" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = atomic_settlement_template(AtomicSettlementTemplateOptions {
                data_dir: PathBuf::from(data_dir),
                left_owner: flag_value(flags, "--left-owner")
                    .ok_or("missing --left-owner")?
                    .to_string(),
                left_recipient: flag_value(flags, "--left-recipient")
                    .ok_or("missing --left-recipient")?
                    .to_string(),
                left_asset_id: flag_value(flags, "--left-asset-id")
                    .ok_or("missing --left-asset-id")?
                    .to_string(),
                left_amount: parse_u64_flag(flags, "--left-amount")?,
                right_owner: flag_value(flags, "--right-owner")
                    .ok_or("missing --right-owner")?
                    .to_string(),
                right_recipient: flag_value(flags, "--right-recipient")
                    .ok_or("missing --right-recipient")?
                    .to_string(),
                right_asset_id: flag_value(flags, "--right-asset-id")
                    .ok_or("missing --right-asset-id")?
                    .to_string(),
                right_amount: parse_u64_flag(flags, "--right-amount")?,
                condition: flag_value(flags, "--condition")
                    .ok_or("missing --condition")?
                    .to_string(),
                finish_after: parse_optional_u64_flag(flags, "--finish-after")?.unwrap_or(0),
                cancel_after: parse_u64_flag(flags, "--cancel-after")?,
                left_sequence: parse_optional_u64_flag(flags, "--left-sequence")?,
                right_sequence: parse_optional_u64_flag(flags, "--right-sequence")?,
            })
            .map_err(|error| format!("atomic-settlement-template failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("atomic settlement template serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "asset-info" | "asset_info" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let report = asset_info(AssetInfoOptions {
                data_dir: PathBuf::from(data_dir),
                asset_id: asset_id.to_string(),
            })
            .map_err(|error| format!("asset-info failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("asset info serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "account-lines" | "account_lines" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_lines(AccountLinesOptions {
                data_dir: PathBuf::from(data_dir),
                account: account.to_string(),
                issuer: flag_value(flags, "--issuer").map(ToString::to_string),
                asset_id: flag_value(flags, "--asset-id").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("account-lines failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("account lines serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "account-assets" | "account_assets" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_assets(AccountAssetsOptions {
                data_dir: PathBuf::from(data_dir),
                account: account.to_string(),
                asset_id: flag_value(flags, "--asset-id").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("account-assets failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("account assets serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "owned-objects" | "owned_objects" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let owner_public_key_hex = flag_value(flags, "--owner-public-key-hex")
                .ok_or("missing --owner-public-key-hex")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = owned_objects(OwnedObjectsOptions {
                data_dir: PathBuf::from(data_dir),
                owner_public_key_hex: owner_public_key_hex.to_string(),
                asset: flag_value(flags, "--asset").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("owned-objects failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("owned objects serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "issuer-assets" | "issuer_assets" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let issuer = flag_value(flags, "--issuer").ok_or("missing --issuer")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = issuer_assets(IssuerAssetsOptions {
                data_dir: PathBuf::from(data_dir),
                issuer: issuer.to_string(),
                limit,
            })
            .map_err(|error| format!("issuer-assets failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("issuer assets serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "escrow-info" | "escrow_info" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let escrow_id = flag_value(flags, "--escrow-id").ok_or("missing --escrow-id")?;
            let report = escrow_info(EscrowInfoOptions {
                data_dir: PathBuf::from(data_dir),
                escrow_id: escrow_id.to_string(),
            })
            .map_err(|error| format!("escrow-info failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("escrow info serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "account-escrows" | "account_escrows" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_escrows(AccountEscrowsOptions {
                data_dir: PathBuf::from(data_dir),
                account: account.to_string(),
                role: flag_value(flags, "--role").map(ToString::to_string),
                state: flag_value(flags, "--state").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("account-escrows failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("account escrows serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "nft-info" | "nft_info" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let nft_id = flag_value(flags, "--nft-id").ok_or("missing --nft-id")?;
            let report = nft_info(NftInfoOptions {
                data_dir: PathBuf::from(data_dir),
                nft_id: nft_id.to_string(),
            })
            .map_err(|error| format!("nft-info failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("nft info serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "account-nfts" | "account_nfts" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_nfts(AccountNftsOptions {
                data_dir: PathBuf::from(data_dir),
                account: account.to_string(),
                include_burned: flag_present(flags, "--include-burned"),
                limit,
            })
            .map_err(|error| format!("account-nfts failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("account nfts serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "issuer-nfts" | "issuer_nfts" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let issuer = flag_value(flags, "--issuer").ok_or("missing --issuer")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = issuer_nfts(IssuerNftsOptions {
                data_dir: PathBuf::from(data_dir),
                issuer: issuer.to_string(),
                collection_id: flag_value(flags, "--collection-id").map(ToString::to_string),
                include_burned: flag_present(flags, "--include-burned"),
                limit,
            })
            .map_err(|error| format!("issuer-nfts failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("issuer nfts serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "receipts" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let tx_id = flag_value(flags, "--tx-id").map(str::to_string);
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let receipt_log = receipts(ReceiptQueryOptions {
                data_dir: PathBuf::from(data_dir),
                tx_id,
                limit,
            })
            .map_err(|error| format!("receipts failed: {error}"))?;
            let json = serde_json::to_string_pretty(&receipt_log)
                .map_err(|error| format!("receipt serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "blocks" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let from_height = flag_value(flags, "--from-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--from-height must be a u64".to_string())
                })
                .transpose()?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let block_log = blocks(BlockQueryOptions {
                data_dir: PathBuf::from(data_dir),
                from_height,
                limit,
            })
            .map_err(|error| format!("blocks failed: {error}"))?;
            let json = serde_json::to_string_pretty(&block_log)
                .map_err(|error| format!("block serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "pfusdc-egress-witness" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let withdrawal_id = flag_value(flags, "--withdrawal-id")
                .ok_or("missing --withdrawal-id")?;
            let witness = pfusdc_egress_witness(PfUsdcEgressWitnessOptions {
                data_dir: PathBuf::from(data_dir),
                withdrawal_id: withdrawal_id.to_string(),
            })
            .map_err(|error| format!("pfusdc-egress-witness failed: {error}"))?;
            let json = serde_json::to_string_pretty(&witness)
                .map_err(|error| format!("pfUSDC egress witness serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "block-vote" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let key_file = flag_value(flags, "--key-file")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(data_dir).join(VALIDATOR_KEYS_FILE));
            let validator_id = flag_value(flags, "--validator").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").map(PathBuf::from);
            let proposal_file = flag_value(flags, "--proposal-file").map(PathBuf::from);
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let vote_file =
                PathBuf::from(flag_value(flags, "--vote-file").ok_or("missing --vote-file")?);
            let vote = create_block_vote(BlockVoteOptions {
                data_dir: PathBuf::from(data_dir),
                verify_block_log: !flags.contains(&"--skip-block-log-verify".to_string()),
                key_file,
                validator_id,
                batch_file,
                proposal_file,
                timeout_certificate_file,
                block_height,
                vote_file,
            })
            .map_err(|error| format!("block-vote failed: {error}"))?;
            let json = serde_json::to_string_pretty(&vote)
                .map_err(|error| format!("block vote serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "block-vote-equivocation" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let first_proposal_file = PathBuf::from(
                flag_value(flags, "--first-proposal-file")
                    .ok_or("missing --first-proposal-file")?,
            );
            let second_proposal_file = PathBuf::from(
                flag_value(flags, "--second-proposal-file")
                    .ok_or("missing --second-proposal-file")?,
            );
            let first_vote_file = PathBuf::from(
                flag_value(flags, "--first-vote-file").ok_or("missing --first-vote-file")?,
            );
            let second_vote_file = PathBuf::from(
                flag_value(flags, "--second-vote-file").ok_or("missing --second-vote-file")?,
            );
            let evidence_file = PathBuf::from(
                flag_value(flags, "--evidence-file").ok_or("missing --evidence-file")?,
            );
            let evidence = detect_block_vote_equivocation(BlockVoteEquivocationOptions {
                data_dir: PathBuf::from(data_dir),
                first_proposal_file,
                second_proposal_file,
                first_vote_file,
                second_vote_file,
                evidence_file,
            })
            .map_err(|error| format!("block-vote-equivocation failed: {error}"))?;
            let json = serde_json::to_string_pretty(&evidence).map_err(|error| {
                format!("block vote equivocation serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "block-vote-equivocation-verify" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let first_proposal_file = PathBuf::from(
                flag_value(flags, "--first-proposal-file")
                    .ok_or("missing --first-proposal-file")?,
            );
            let second_proposal_file = PathBuf::from(
                flag_value(flags, "--second-proposal-file")
                    .ok_or("missing --second-proposal-file")?,
            );
            let first_vote_file = PathBuf::from(
                flag_value(flags, "--first-vote-file").ok_or("missing --first-vote-file")?,
            );
            let second_vote_file = PathBuf::from(
                flag_value(flags, "--second-vote-file").ok_or("missing --second-vote-file")?,
            );
            let evidence_file = PathBuf::from(
                flag_value(flags, "--evidence-file").ok_or("missing --evidence-file")?,
            );
            let evidence = verify_block_vote_equivocation(BlockVoteEquivocationOptions {
                data_dir: PathBuf::from(data_dir),
                first_proposal_file,
                second_proposal_file,
                first_vote_file,
                second_vote_file,
                evidence_file,
            })
            .map_err(|error| format!("block-vote-equivocation-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&evidence).map_err(|error| {
                format!("block vote equivocation verification serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "block-proposal-equivocation" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let first_proposal_file = PathBuf::from(
                flag_value(flags, "--first-proposal-file")
                    .ok_or("missing --first-proposal-file")?,
            );
            let second_proposal_file = PathBuf::from(
                flag_value(flags, "--second-proposal-file")
                    .ok_or("missing --second-proposal-file")?,
            );
            let evidence_file = PathBuf::from(
                flag_value(flags, "--evidence-file").ok_or("missing --evidence-file")?,
            );
            let evidence = detect_block_proposal_equivocation(BlockProposalEquivocationOptions {
                data_dir: PathBuf::from(data_dir),
                first_proposal_file,
                second_proposal_file,
                evidence_file,
            })
            .map_err(|error| format!("block-proposal-equivocation failed: {error}"))?;
            let json = serde_json::to_string_pretty(&evidence).map_err(|error| {
                format!("block proposal equivocation serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "block-proposal-equivocation-verify" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let first_proposal_file = PathBuf::from(
                flag_value(flags, "--first-proposal-file")
                    .ok_or("missing --first-proposal-file")?,
            );
            let second_proposal_file = PathBuf::from(
                flag_value(flags, "--second-proposal-file")
                    .ok_or("missing --second-proposal-file")?,
            );
            let evidence_file = PathBuf::from(
                flag_value(flags, "--evidence-file").ok_or("missing --evidence-file")?,
            );
            let evidence = verify_block_proposal_equivocation(BlockProposalEquivocationOptions {
                data_dir: PathBuf::from(data_dir),
                first_proposal_file,
                second_proposal_file,
                evidence_file,
            })
            .map_err(|error| format!("block-proposal-equivocation-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&evidence).map_err(|error| {
                format!("block proposal equivocation verification serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "block-certificate" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_file = flag_value(flags, "--batch-file").map(PathBuf::from);
            let proposal_file = flag_value(flags, "--proposal-file").map(PathBuf::from);
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let vote_files =
                parse_csv_values(flag_value(flags, "--vote-files").ok_or("missing --vote-files")?)?
                    .into_iter()
                    .map(PathBuf::from)
                    .collect();
            let certificate_file = PathBuf::from(
                flag_value(flags, "--certificate-file").ok_or("missing --certificate-file")?,
            );
            let certificate = aggregate_block_certificate(BlockCertificateOptions {
                data_dir: PathBuf::from(data_dir),
                verify_block_log: !flags.contains(&"--skip-block-log-verify".to_string()),
                batch_file,
                proposal_file,
                timeout_certificate_file,
                block_height,
                vote_files,
                certificate_file,
            })
            .map_err(|error| format!("block-certificate failed: {error}"))?;
            let json = serde_json::to_string_pretty(&certificate)
                .map_err(|error| format!("block certificate serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "block-certificate-from-archive" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let block_file =
                PathBuf::from(flag_value(flags, "--block-file").ok_or("missing --block-file")?);
            let batch_file =
                PathBuf::from(flag_value(flags, "--batch-file").ok_or("missing --batch-file")?);
            let certificate_file = PathBuf::from(
                flag_value(flags, "--certificate-file").ok_or("missing --certificate-file")?,
            );
            let certificate = postfiat_node::reconstruct_block_certificate_from_archive(
                BlockCertificateFromArchiveOptions {
                    data_dir: PathBuf::from(data_dir),
                    block_file,
                    batch_file,
                    certificate_file,
                },
            )
            .map_err(|error| format!("block-certificate-from-archive failed: {error}"))?;
            let json = serde_json::to_string_pretty(&certificate).map_err(|error| {
                format!("block certificate from archive serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "rpc-catch-up" | "rpc-catch-up-certified-delta" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let source_host = flag_value(flags, "--source-host")
                .ok_or("missing --source-host")?
                .to_string();
            let source_rpc_port = flag_value(flags, "--source-rpc-port")
                .ok_or("missing --source-rpc-port")?
                .parse::<u16>()
                .map_err(|_| "--source-rpc-port must be a u16".to_string())?;
            let max_blocks = flag_value(flags, "--max-blocks")
                .unwrap_or("64")
                .parse::<usize>()
                .map_err(|_| "--max-blocks must be a usize".to_string())?;
            if max_blocks == 0 || max_blocks > DEFAULT_RPC_CATCH_UP_MAX_BLOCKS {
                return Err(format!(
                    "--max-blocks must be between 1 and {DEFAULT_RPC_CATCH_UP_MAX_BLOCKS}"
                ));
            }
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let work_dir = flag_value(flags, "--work-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join("rpc_catchup"));
            let options = RpcCatchUpOptions {
                data_dir,
                source_host,
                source_rpc_port,
                work_dir,
                max_blocks,
                timeout_ms,
            };
            let report = if command == "rpc-catch-up-certified-delta" {
                let expected_height = flag_value(flags, "--expected-height")
                    .ok_or("missing --expected-height")?
                    .parse::<u64>()
                    .map_err(|_| "--expected-height must be a u64".to_string())?;
                let expected_block_hash = flag_value(flags, "--expected-block-hash")
                    .ok_or("missing --expected-block-hash")?
                    .to_string();
                let expected_state_root = flag_value(flags, "--expected-state-root")
                    .ok_or("missing --expected-state-root")?
                    .to_string();
                rpc_catch_up_certified_delta(
                    options,
                    expected_height,
                    expected_block_hash,
                    expected_state_root,
                )?
            } else {
                rpc_catch_up(options)?
            };
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("rpc catch-up serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "block-timeout-vote" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let key_file = flag_value(flags, "--key-file")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(data_dir).join(VALIDATOR_KEYS_FILE));
            let validator_id = flag_value(flags, "--validator").map(str::to_string);
            let block_height = flag_value(flags, "--height")
                .ok_or("missing --height")?
                .parse::<u64>()
                .map_err(|_| "--height must be a u64".to_string())?;
            let view = flag_value(flags, "--view")
                .ok_or("missing --view")?
                .parse::<u64>()
                .map_err(|_| "--view must be a u64".to_string())?;
            let high_qc_id = flag_value(flags, "--high-qc").ok_or("missing --high-qc")?;
            let vote_file =
                PathBuf::from(flag_value(flags, "--vote-file").ok_or("missing --vote-file")?);
            let vote = create_block_timeout_vote(BlockTimeoutVoteOptions {
                data_dir: PathBuf::from(data_dir),
                verify_block_log: !flag_present(flags, "--skip-block-log-verify"),
                key_file,
                validator_id,
                block_height,
                view,
                high_qc_id: high_qc_id.to_string(),
                vote_file,
            })
            .map_err(|error| format!("block-timeout-vote failed: {error}"))?;
            let json = serde_json::to_string_pretty(&vote)
                .map_err(|error| format!("block timeout vote serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "block-timeout-certificate" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let block_height = flag_value(flags, "--height")
                .ok_or("missing --height")?
                .parse::<u64>()
                .map_err(|_| "--height must be a u64".to_string())?;
            let view = flag_value(flags, "--view")
                .ok_or("missing --view")?
                .parse::<u64>()
                .map_err(|_| "--view must be a u64".to_string())?;
            let vote_files =
                parse_csv_values(flag_value(flags, "--vote-files").ok_or("missing --vote-files")?)?
                    .into_iter()
                    .map(PathBuf::from)
                    .collect();
            let certificate_file = PathBuf::from(
                flag_value(flags, "--certificate-file").ok_or("missing --certificate-file")?,
            );
            let certificate = aggregate_block_timeout_certificate(BlockTimeoutCertificateOptions {
                data_dir: PathBuf::from(data_dir),
                verify_block_log: !flag_present(flags, "--skip-block-log-verify"),
                block_height,
                view,
                vote_files,
                certificate_file,
            })
            .map_err(|error| format!("block-timeout-certificate failed: {error}"))?;
            let json = serde_json::to_string_pretty(&certificate).map_err(|error| {
                format!("block timeout certificate serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "block-timeout-verify" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let certificate_file = PathBuf::from(
                flag_value(flags, "--certificate-file").ok_or("missing --certificate-file")?,
            );
            let certificate =
                verify_block_timeout_certificate_file(BlockTimeoutCertificateVerifyOptions {
                    data_dir: PathBuf::from(data_dir),
                    verify_block_log: !flag_present(flags, "--skip-block-log-verify"),
                    certificate_file,
                })
                .map_err(|error| format!("block-timeout-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&certificate).map_err(|error| {
                format!("block timeout verification serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "certify-batch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let validator_key_dir =
                flag_value(flags, "--validator-key-dir").ok_or("missing --validator-key-dir")?;
            let proposal_file =
                flag_value(flags, "--proposal-file").ok_or("missing --proposal-file")?;
            let vote_dir = flag_value(flags, "--vote-dir").ok_or("missing --vote-dir")?;
            let certificate_file =
                flag_value(flags, "--certificate-file").ok_or("missing --certificate-file")?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let report = certify_batch_round(BatchCertificateRoundOptions {
                data_dir: PathBuf::from(data_dir),
                batch_kind,
                batch_file: PathBuf::from(batch_file),
                validator_key_dir: PathBuf::from(validator_key_dir),
                vote_dir: PathBuf::from(vote_dir),
                proposal_file: PathBuf::from(proposal_file),
                certificate_file: PathBuf::from(certificate_file),
                block_height,
                view,
                timeout_certificate_file,
                skip_block_log_verify: flags.contains(&"--skip-block-log-verify".to_string()),
            })
            .map_err(|error| format!("certify-batch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("certify-batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "batch-archive" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_id = flag_value(flags, "--batch-id").map(str::to_string);
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let archive = batch_archive(BatchArchiveQueryOptions {
                data_dir: PathBuf::from(data_dir),
                batch_kind,
                batch_id,
                limit,
            })
            .map_err(|error| format!("batch-archive failed: {error}"))?;
            let json = serde_json::to_string_pretty(&archive)
                .map_err(|error| format!("batch archive serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "export-envelope-bundle" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let epoch = flag_value(flags, "--epoch")
                .ok_or("missing --epoch")?
                .parse::<u64>()
                .map_err(|_| "--epoch must be a u64".to_string())?;
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let bundle = export_market_ops_replay_bundle(MarketOpsReplayBundleExportOptions {
                data_dir: PathBuf::from(data_dir),
                asset_id: asset_id.to_string(),
                epoch,
                bundle_dir: PathBuf::from(bundle_dir),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("export-envelope-bundle failed: {error}"))?;
            let json = serde_json::to_string_pretty(&bundle).map_err(|error| {
                format!("market ops replay bundle serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "replay-envelope" => {
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let report = replay_market_ops_bundle(MarketOpsReplayBundleVerifyOptions {
                bundle_dir: PathBuf::from(bundle_dir),
            })
            .map_err(|error| format!("replay-envelope failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("market ops replay report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        _ => unreachable!("run_cli_group_04 dispatch mismatch"),
    }
}
