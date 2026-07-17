    #[test]
    fn nav_roundtrip_replay_corpus_verify_gates_live_compression_claims() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-replay-corpus-verify-{}",
            process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create replay corpus root");
        let unsafe_corpus_file = root.join("same-round-asset-create-equivalence.json");
        write_json_file(
            &unsafe_corpus_file,
            &serde_json::json!({
                "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
                "case": "same-source-asset-create-pair",
                "candidate_batch_class": "same_round_consecutive_asset_ops",
                "unbatched_block_height": 2,
                "batched_block_height": 1,
                "unbatched_state_root": "ab".repeat(48),
                "batched_state_root": "cd".repeat(48),
                "state_root_match": false,
                "intended_state_root_difference": "unbatched commits two ordered blocks while same-round batching commits one ordered block",
                "ledger_facing_asset_definitions_match": true,
                "safe_for_live_round_compression": false,
                "gate": "do not use as live state-root-equivalent batching until operator approval",
            }),
        )
        .expect("write unsafe corpus");

        let evidence_report =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(unsafe_corpus_file.clone()),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: false,
                required_candidate_classes: Vec::new(),
                strict_exit: false,
            })
            .expect("valid unsafe corpus evidence");
        assert!(evidence_report.passed, "{:?}", evidence_report.failure_reasons);
        assert_eq!(1, evidence_report.case_count);
        assert_eq!(1, evidence_report.valid_case_count);
        assert_eq!(0, evidence_report.live_ready_case_count);
        assert!(evidence_report.cases[0].valid_corpus_case);
        assert!(!evidence_report.cases[0].live_round_compression_ready);

        let live_ready_report =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(unsafe_corpus_file.clone()),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: true,
                required_candidate_classes: Vec::new(),
                strict_exit: false,
            })
            .expect("strict live-ready report");
        assert!(!live_ready_report.passed);
        assert!(live_ready_report
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("not live-round-compression ready")));

        let strict_error =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(unsafe_corpus_file.clone()),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: true,
                required_candidate_classes: Vec::new(),
                strict_exit: true,
            })
            .expect_err("strict live-ready mode must reject unsafe corpus");
        assert!(strict_error.contains("replay corpus verification failed"));
        let cli_strict_error = run_cli(vec![
            "nav-roundtrip-replay-corpus-verify".to_string(),
            "--corpus-file".to_string(),
            unsafe_corpus_file.display().to_string(),
            "--require-live-compression-ready".to_string(),
            "--strict".to_string(),
        ])
        .expect_err("CLI strict live-ready mode must reject unsafe corpus");
        assert!(cli_strict_error.contains("replay corpus verification failed"));

        let safe_corpus_file = root.join("same-round-safe-equivalence.json");
        write_json_file(
            &safe_corpus_file,
            &serde_json::json!({
                "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
                "case": "same-round-state-root-equivalent-pair",
                "candidate_batch_class": "same_round_state_root_equivalent_ops",
                "unbatched_block_height": 1,
                "batched_block_height": 1,
                "unbatched_state_root": "ef".repeat(48),
                "batched_state_root": "ef".repeat(48),
                "state_root_match": true,
                "ledger_facing_asset_definitions_match": true,
                "safe_for_live_round_compression": true,
                "gate": "state-root-equivalent replay corpus green",
            }),
        )
        .expect("write safe corpus");
        let safe_report =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(safe_corpus_file.clone()),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: true,
                required_candidate_classes: Vec::new(),
                strict_exit: true,
            })
            .expect("safe corpus passes live-ready mode");
        assert!(safe_report.passed, "{:?}", safe_report.failure_reasons);
        assert_eq!(1, safe_report.live_ready_case_count);
        assert!(safe_report.required_candidate_classes.is_empty());
        assert_eq!(
            vec!["same_round_state_root_equivalent_ops".to_string()],
            safe_report.live_ready_candidate_classes
        );
        assert!(safe_report.missing_required_candidate_classes.is_empty());
        run_cli(vec![
            "nav-roundtrip-replay-corpus-verify".to_string(),
            "--corpus-file".to_string(),
            safe_corpus_file.display().to_string(),
            "--require-live-compression-ready".to_string(),
            "--strict".to_string(),
        ])
        .expect("CLI accepts safe live-ready corpus");

        let required_safe_report =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(safe_corpus_file.clone()),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: true,
                required_candidate_classes: vec![
                    "same_round_state_root_equivalent_ops".to_string()
                ],
                strict_exit: true,
            })
            .expect("required live-ready class passes");
        assert!(required_safe_report.passed, "{:?}", required_safe_report.failure_reasons);
        assert_eq!(
            vec!["same_round_state_root_equivalent_ops".to_string()],
            required_safe_report.required_candidate_classes
        );
        assert!(required_safe_report
            .missing_required_candidate_classes
            .is_empty());

        let missing_required_report =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(safe_corpus_file.clone()),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: true,
                required_candidate_classes: vec![
                    "nav_mint_subscription_same_round".to_string()
                ],
                strict_exit: false,
            })
            .expect("missing required class report");
        assert!(!missing_required_report.passed);
        assert_eq!(
            vec!["nav_mint_subscription_same_round".to_string()],
            missing_required_report.required_candidate_classes
        );
        assert_eq!(
            vec!["nav_mint_subscription_same_round".to_string()],
            missing_required_report.missing_required_candidate_classes
        );
        assert!(missing_required_report
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("no live-ready replay corpus case")));

        let missing_required_cli_error = run_cli(vec![
            "nav-roundtrip-replay-corpus-verify".to_string(),
            "--corpus-file".to_string(),
            safe_corpus_file.display().to_string(),
            "--require-live-compression-ready".to_string(),
            "--require-candidate-classes".to_string(),
            "nav_mint_subscription_same_round".to_string(),
            "--strict".to_string(),
        ])
        .expect_err("CLI strict required class mode must reject missing class");
        assert!(missing_required_cli_error.contains("replay corpus verification failed"));

        let dir_report_file = root.join("replay-corpus-verify-report.json");
        let dir_report =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: None,
                corpus_dir: Some(root.clone()),
                report_file: Some(dir_report_file.clone()),
                require_live_compression_ready: false,
                required_candidate_classes: Vec::new(),
                strict_exit: false,
            })
            .expect("corpus dir verification");
        assert!(dir_report.passed, "{:?}", dir_report.failure_reasons);
        assert_eq!(2, dir_report.case_count);
        assert_eq!(2, dir_report.valid_case_count);
        assert_eq!(1, dir_report.live_ready_case_count);
        assert!(dir_report_file.exists());

        let bad_claim_file = root.join("bad-safe-claim.json");
        write_json_file(
            &bad_claim_file,
            &serde_json::json!({
                "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
                "case": "bad-safe-claim",
                "candidate_batch_class": "same_round_bad_claim",
                "unbatched_block_height": 2,
                "batched_block_height": 1,
                "unbatched_state_root": "12".repeat(48),
                "batched_state_root": "34".repeat(48),
                "state_root_match": false,
                "intended_state_root_difference": "documented but not approved",
                "ledger_facing_asset_definitions_match": true,
                "safe_for_live_round_compression": true,
                "gate": "do not use as live batching",
            }),
        )
        .expect("write bad safe claim");
        let bad_claim_report =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(bad_claim_file),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: false,
                required_candidate_classes: Vec::new(),
                strict_exit: false,
            })
            .expect("bad safe claim report");
        assert!(!bad_claim_report.passed);
        assert!(bad_claim_report
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("prohibitive")));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn certified_asset_ops_submit_accepts_bundle_input() {
        let root = env::temp_dir().join(format!(
            "postfiat-certified-asset-ops-submit-bundle-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let bundle_dir = root.join("bundle");
        let artifact_dir = root.join("artifacts");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&bundle_dir).expect("create bundle dir");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init");
        write_local_topology(TopologyOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            validators: 1,
            base_port: 44_020,
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
        })
        .expect("write local topology");
        let faucet = faucet_key(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("faucet key");
        let faucet_key_file = data_dir.join("faucet_key.json");
        let operation = postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: faucet.address,
                code: "BUNDLEIN".to_string(),
                version: 1,
                precision: 6,
                display_name: "Bundle Input Asset".to_string(),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        );
        std::fs::write(
            bundle_dir.join("propose.operation.json"),
            serde_json::to_string_pretty(&operation).expect("operation json"),
        )
        .expect("write bundle operation");

        run_cli(vec![
            "pftl-submit-certified-asset-ops".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            data_dir.join(VALIDATOR_KEYS_FILE).display().to_string(),
            "--bundle".to_string(),
            bundle_dir.display().to_string(),
            "--proposer-key-file".to_string(),
            faucet_key_file.display().to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--batch-only".to_string(),
        ])
        .expect("submit certified asset ops from bundle");

        assert!(artifact_dir.with_extension("certified-ops.request.json").exists());
        assert!(artifact_dir.join("mempool-batch.json").exists());
        assert!(artifact_dir.join("propose").join("submit.json").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn certified_asset_ops_rejects_duplicate_labels() {
        let root = env::temp_dir().join(format!(
            "postfiat-certified-asset-ops-duplicate-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let key_file = root.join("issuer-key.json");
        let backup_file = root.join("issuer-backup.json");
        let ops_file = root.join("ops.json");
        let artifact_dir = root.join("artifacts");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init");
        let key_report = wallet_keygen(WalletKeygenOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            master_seed_hex: "44".repeat(32),
            account_index: 0,
            key_file: key_file.clone(),
            backup_file,
            overwrite: true,
        })
        .expect("wallet keygen");
        let op = postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: key_report.address.clone(),
                code: "DUP".to_string(),
                version: 1,
                precision: 6,
                display_name: "Duplicate Label Test".to_string(),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        );
        let request = serde_json::json!({
            "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
            "operations": [
                {
                    "label": "dup",
                    "source": key_report.address,
                    "key_file": key_file.display().to_string(),
                    "operation": op,
                },
                {
                    "label": "dup",
                    "source": key_report.address,
                    "key_file": key_file.display().to_string(),
                    "operation": op,
                }
            ]
        });
        std::fs::write(
            &ops_file,
            serde_json::to_string_pretty(&request).expect("request json"),
        )
        .expect("write ops request");

        let error = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
            data_dir,
            topology_file: root.join("unused-topology.json"),
            key_file,
            proposal_key_file: None,
            ops_file,
            artifact_dir,
            max_transactions: None,
            require_local_proposer: false,
            require_signed_proposal: true,
            allow_peer_failures: false,
            quorum_early_full_propagation: false,
            local_apply_before_certified_send: false,
            defer_certified_sends: false,
            block_height: None,
            view: None,
            timeout_certificate_file: None,
            timeout_ms: 5_000,
            send_retries: 0,
            retry_backoff_ms: 250,
            allow_existing_mempool: false,
            resume: false,
            overwrite: false,
            prepare_only: true,
            batch_only: false,
        })
        .expect_err("duplicate labels must fail");
        assert!(error.contains("duplicate certified asset op label `dup`"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn certified_asset_ops_from_bundle_writes_valid_request() {
        let root = env::temp_dir().join(format!(
            "postfiat-certified-asset-ops-bundle-{}",
            process::id()
        ));
        let bundle_dir = root.join("bundle");
        let output_file = root.join("ops").join("certified-ops.json");
        let key_file = root.join("issuer-key.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&bundle_dir).expect("create bundle dir");
        std::fs::write(&key_file, "{}\n").expect("write placeholder key");

        let issuer = "pfassetopsissuer".to_string();
        let operation = postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: issuer.clone(),
                code: "BUNDLE".to_string(),
                version: 1,
                precision: 6,
                display_name: "Bundle Adapter Test".to_string(),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        );
        std::fs::write(
            bundle_dir.join("propose.operation.json"),
            serde_json::to_string_pretty(&operation).expect("operation json"),
        )
        .expect("write operation");

        let report = certified_asset_ops_from_bundle(CertifiedAssetOpsFromBundleOptions {
            bundle_dir: bundle_dir.clone(),
            output_file: output_file.clone(),
            proposer_key_file: Some(key_file.clone()),
            attestor_key_file: None,
            finalizer_key_file: None,
            claimer_key_file: None,
            owner_key_file: None,
            include_deposit_claim: true,
            overwrite: false,
        })
        .expect("convert bundle to certified ops");
        assert_eq!(CERTIFIED_ASSET_OPS_FROM_BUNDLE_REPORT_SCHEMA, report.schema);
        assert_eq!(1, report.operation_count);
        assert_eq!(vec!["propose".to_string()], report.labels);
        assert!(output_file.exists());

        let request = read_certified_asset_ops_request(&output_file).expect("read generated request");
        validate_certified_asset_ops_request(&request).expect("generated request validates");
        assert_eq!(1, request.operations.len());
        assert_eq!("propose", request.operations[0].label);
        assert_eq!(issuer, request.operations[0].source);
        assert_eq!(key_file, request.operations[0].key_file);
        assert_eq!(
            postfiat_types::ASSET_CREATE_TRANSACTION_KIND,
            request.operations[0].operation.transaction_kind()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn certified_asset_ops_from_bundle_can_skip_deposit_claim() {
        let root = env::temp_dir().join(format!(
            "postfiat-certified-asset-ops-skip-claim-{}",
            process::id()
        ));
        let bundle_dir = root.join("bundle");
        let output_file = root.join("ops").join("certified-ops.json");
        let receipt_file = root.join("receipt.json");
        let key_file = root.join("relayer-key.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        std::fs::write(&key_file, "{}\n").expect("write placeholder key");

        let evidence = vault_bridge_deposit_evidence_fixture();
        let mut deposit_log = vault_bridge_vault_deposit_log_json(&evidence);
        deposit_log
            .as_object_mut()
            .expect("deposit log object")
            .remove("blockHash");
        deposit_log
            .as_object_mut()
            .expect("deposit log object")
            .remove("transactionHash");
        let receipt = serde_json::json!({
            "blockHash": format!("0x{}", evidence.block_hash),
            "transactionHash": format!("0x{}", evidence.tx_hash),
            "logs": [deposit_log],
        });
        std::fs::write(
            &receipt_file,
            serde_json::to_string_pretty(&receipt).expect("receipt json"),
        )
        .expect("write receipt");

        vault_bridge_deposit_relay_bundle(VaultBridgeDepositRelayBundleOptions {
            plan_options: VaultBridgeDepositPlanOptions {
                log_file: None,
                receipt_file: Some(receipt_file),
                vault_address: Some(evidence.vault_address.clone()),
                token_address: Some(evidence.token_address.clone()),
                asset_id: "33".repeat(48),
                policy_hash: "24".repeat(32),
                proposer: "bridge-relayer".to_string(),
                finalizer: "bridge-finalizer".to_string(),
                claimer: "bridge-claimer".to_string(),
                attestor: Some("bridge-attestor".to_string()),
                observer_confirmation_depth: None,
                expires_at_height: 100,
                source_proof_kind: None,
                source_proof_hash: None,
                source_public_values_hash: None,
            },
            bundle_dir: bundle_dir.clone(),
            overwrite: false,
        })
        .expect("write relay bundle");
        assert!(bundle_dir.join("claim.operation.json").exists());

        let report = certified_asset_ops_from_bundle(CertifiedAssetOpsFromBundleOptions {
            bundle_dir,
            output_file: output_file.clone(),
            proposer_key_file: Some(key_file.clone()),
            attestor_key_file: Some(key_file.clone()),
            finalizer_key_file: Some(key_file),
            claimer_key_file: None,
            owner_key_file: None,
            include_deposit_claim: false,
            overwrite: false,
        })
        .expect("convert bundle without claim");
        assert_eq!(3, report.operation_count);
        assert_eq!(
            vec![
                "propose".to_string(),
                "attest".to_string(),
                "finalize".to_string()
            ],
            report.labels
        );

        let request = read_certified_asset_ops_request(&output_file).expect("read generated request");
        assert_eq!(3, request.operations.len());
        assert!(request.operations.iter().all(|operation| operation.label != "claim"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn certified_asset_ops_from_bundle_requires_key_for_present_operation() {
        let root = env::temp_dir().join(format!(
            "postfiat-certified-asset-ops-bundle-key-{}",
            process::id()
        ));
        let bundle_dir = root.join("bundle");
        let output_file = root.join("certified-ops.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&bundle_dir).expect("create bundle dir");

        let operation = postfiat_types::AssetTransactionOperation::AssetCreate(
            postfiat_types::AssetCreateOperation {
                issuer: "pfmissingkeyissuer".to_string(),
                code: "NOKEY".to_string(),
                version: 1,
                precision: 6,
                display_name: "Missing Key Test".to_string(),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            },
        );
        std::fs::write(
            bundle_dir.join("propose.operation.json"),
            serde_json::to_string_pretty(&operation).expect("operation json"),
        )
        .expect("write operation");

        let error = certified_asset_ops_from_bundle(CertifiedAssetOpsFromBundleOptions {
            bundle_dir,
            output_file,
            proposer_key_file: None,
            attestor_key_file: None,
            finalizer_key_file: None,
            claimer_key_file: None,
            owner_key_file: None,
            include_deposit_claim: true,
            overwrite: false,
        })
        .expect_err("bundle operation without key must fail");
        assert!(error.contains("--proposer-key-file"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_preflight_classifies_old_vault_and_writes_artifact() {
        use std::os::unix::fs::PermissionsExt;

        let root = env::temp_dir().join(format!("postfiat-nav-roundtrip-preflight-{}", process::id()));
        let data_dir = root.join("node");
        let artifact_dir = root.join("artifacts");
        let fake_cast = root.join("cast");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init");

        let vault = "0x1111111111111111111111111111111111111111";
        let verifier = "0x2222222222222222222222222222222222222222";
        let usdc = "0x3333333333333333333333333333333333333333";
        let wallet = "0x4444444444444444444444444444444444444444";
        let fake_cast_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = "balance" ]; then
  echo 2000000000000000
  exit 0
fi
if [ "$1" = "code" ]; then
  echo 0x60016000
  exit 0
fi
if [ "$1" = "call" ]; then
  target="$2"
  sig="$3"
  arg="${{4:-}}"
  if [ "$target" = "{usdc}" ] && [ "$sig" = "balanceOf(address)(uint256)" ]; then
    if [ "$arg" = "{wallet}" ]; then
      echo 2000000
    else
      echo 1915094
    fi
    exit 0
  fi
  if [ "$target" = "{usdc}" ] && [ "$sig" = "allowance(address,address)(uint256)" ]; then
    echo 1000000
    exit 0
  fi
  if [ "$sig" = "challenge_delay()(uint64)" ]; then
    echo 5
    exit 0
  fi
  if [ "$sig" = "execution_window()(uint64)" ]; then
    echo 3600
    exit 0
  fi
  if [ "$sig" = "{fixed_sig}" ]; then
    echo "execution reverted" >&2
    exit 1
  fi
  if [ "$sig" = "{old_sig}" ]; then
    echo 0x4e8d3a274b90785f82b1fddee7f6f2ebbea48a33f6055bb0977f5cb83b01cd75
    exit 0
  fi
fi
echo "unexpected cast args: $*" >&2
exit 9
"#,
            fixed_sig = NAV_ROUNDTRIP_FIXED_WITHDRAWAL_DIGEST_SIGNATURE,
            old_sig = NAV_ROUNDTRIP_OLD_WITHDRAWAL_DIGEST_SIGNATURE,
        );
        std::fs::write(&fake_cast, fake_cast_script).expect("write fake cast");
        let mut permissions = std::fs::metadata(&fake_cast)
            .expect("fake cast metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&fake_cast, permissions).expect("chmod fake cast");

        let report = nav_roundtrip_live_demo_preflight(NavRoundtripPreflightOptions {
            data_dir: data_dir.clone(),
            artifact_dir: artifact_dir.clone(),
            source_rpc_url: "https://arb.example.invalid/rpc".to_string(),
            cast_binary: fake_cast.display().to_string(),
            vault_address: vault.to_string(),
            verifier_address: verifier.to_string(),
            usdc_address: usdc.to_string(),
            stakehub_wallet: wallet.to_string(),
            amount_atoms: 1_000_000,
            min_gas_wei: 1_000_000_000_000_000,
            resume: false,
            overwrite: false,
        })
        .expect("NAV roundtrip preflight");

        assert_eq!(NAV_ROUNDTRIP_PREFLIGHT_REPORT_SCHEMA, report.schema);
        assert!(report.preflight_ok, "{:?}", report.failure_reasons);
        assert_eq!(NAV_ROUNDTRIP_BRIDGE_CLASS_CONTROLLED_LAUNCH, report.bridge_class);
        assert_eq!(Some(5), report.vault_challenge_delay_seconds);
        assert_eq!(Some(3600), report.vault_execution_window_seconds);
        assert_eq!("2000000", report.wallet_usdc_atoms);
        assert_eq!("1915094", report.vault_usdc_atoms);
        assert!(artifact_dir.join("preflight.json").exists());

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--preflight-only".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--artifact-dir".to_string(),
            root.join("cli-artifacts").display().to_string(),
            "--source-rpc-url".to_string(),
            "https://arb.example.invalid/rpc".to_string(),
            "--cast-bin".to_string(),
            fake_cast.display().to_string(),
            "--vault".to_string(),
            vault.to_string(),
            "--verifier".to_string(),
            verifier.to_string(),
            "--usdc".to_string(),
            usdc.to_string(),
            "--stakehub-wallet".to_string(),
            wallet.to_string(),
            "--amount-atoms".to_string(),
            "1000000".to_string(),
        ])
        .expect("NAV roundtrip preflight cli");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_evm_deposit_uses_agent_and_verifies_deltas() {
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;
        use std::sync::{Arc, Mutex};

        let root = env::temp_dir().join(format!("postfiat-nav-roundtrip-evm-deposit-{}", process::id()));
        let artifact_dir = root.join("artifacts");
        let stakehub_home = root.join("stakehub");
        let fake_cast = root.join("cast");
        let deposit_marker = root.join("deposit-landed");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&stakehub_home).expect("create stakehub home");
        let socket_path = stakehub_home.join("agent.sock");

        let vault = "0x1111111111111111111111111111111111111111";
        let usdc = "0x2222222222222222222222222222222222222222";
        let wallet = "0x3333333333333333333333333333333333333333";
        let buyer = "pf07381735ddb7de134e8be8402b465c9cd8ec7546";
        let nonce = "0x5ce1dfc7d8030b1b39098e74ddb586102335b4118269b29e95a3494e4d54de3a";
        let fake_cast_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = "calldata" ]; then
  if [ "$2" = "approve(address,uint256)" ]; then
    echo 0x095ea7b3
    exit 0
  fi
  if [ "$2" = "deposit(uint256,string,bytes32)" ]; then
    echo 0xabcdef01
    exit 0
  fi
fi
if [ "$1" = "call" ]; then
  sig="$3"
  arg="${{4:-}}"
	  if [ "$sig" = "balanceOf(address)(uint256)" ]; then
	    if [ "$arg" = "{wallet}" ]; then
	      if [ -f "{deposit_marker}" ]; then echo 1000000; else echo 2000000; fi
	      exit 0
	    fi
	    if [ "$arg" = "{vault}" ]; then
	      if [ -f "{deposit_marker}" ]; then echo 2000000; else echo 1000000; fi
	      exit 0
	    fi
	  fi
	  if [ "$sig" = "allowance(address,address)(uint256)" ]; then
	    echo 0
	    exit 0
	  fi
	fi
if [ "$1" = "balance" ]; then
  echo 2000000000000000
  exit 0
fi
echo "unexpected cast args: $*" >&2
exit 9
"#,
            deposit_marker = deposit_marker.display(),
        );
        std::fs::write(&fake_cast, fake_cast_script).expect("write fake cast");
        let mut permissions = std::fs::metadata(&fake_cast)
            .expect("fake cast metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&fake_cast, permissions).expect("chmod fake cast");

        let listener = UnixListener::bind(&socket_path).expect("bind fake agent");
        let requests = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let requests_for_thread = Arc::clone(&requests);
        let marker_for_thread = deposit_marker.clone();
        let agent_thread = std::thread::spawn(move || {
            for _ in 0..6 {
                let (stream, _) = listener.accept().expect("accept fake agent request");
                let mut reader = std::io::BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).expect("read fake agent request");
                let request: serde_json::Value =
                    serde_json::from_str(&line).expect("parse fake agent request");
                requests_for_thread
                    .lock()
                    .expect("requests lock")
                    .push(request.clone());
                let op = request.get("op").and_then(serde_json::Value::as_str).unwrap_or("");
                let response = match op {
                    "status" => serde_json::json!({"ok": true, "unlocked": true}),
                    "close_launch_session" => serde_json::json!({"ok": true, "closed": true}),
                    "open_launch_session" => serde_json::json!({
                        "ok": true,
                        "session": {
                            "id": request.get("session_id").cloned().unwrap_or(serde_json::Value::Null),
                        }
                    }),
                    "evm_contract_tx" => {
                        let action = request
                            .get("session_action")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("");
                        if action == "deposit_pfusdc_vault" {
                            std::fs::write(&marker_for_thread, "ok\n").expect("write deposit marker");
                            serde_json::json!({"ok": true, "tx": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "gas_used": 87500})
                        } else {
                            serde_json::json!({"ok": true, "tx": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "gas_used": 55835})
                        }
                    }
                    _ => serde_json::json!({"ok": false, "error": format!("unexpected op {op}")}),
                };
                let mut stream = reader.into_inner();
                writeln!(stream, "{}", serde_json::to_string(&response).expect("response json"))
                    .expect("write fake agent response");
            }
        });

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--evm-deposit-only".to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--source-rpc-url".to_string(),
            "https://arb.example.invalid/rpc".to_string(),
            "--cast-bin".to_string(),
            fake_cast.display().to_string(),
            "--stakehub-home".to_string(),
            stakehub_home.display().to_string(),
            "--source-chain-id".to_string(),
            "42161".to_string(),
            "--vault".to_string(),
            vault.to_string(),
            "--usdc".to_string(),
            usdc.to_string(),
            "--stakehub-wallet".to_string(),
            wallet.to_string(),
            "--pftl-recipient".to_string(),
            buyer.to_string(),
            "--amount-atoms".to_string(),
            "1000000".to_string(),
            "--nonce".to_string(),
            nonce.to_string(),
            "--session-id".to_string(),
            "nav-roundtrip-test-session".to_string(),
        ])
        .expect("EVM deposit stage cli");
        agent_thread.join().expect("fake agent thread");

        let report_file = artifact_dir.join("evm-deposit.json");
        let report = serde_json::from_str::<NavRoundtripEvmDepositReport>(
            &std::fs::read_to_string(&report_file).expect("read deposit report"),
        )
        .expect("parse deposit report");
        assert_eq!(NAV_ROUNDTRIP_EVM_DEPOSIT_REPORT_SCHEMA, report.schema);
        assert!(report.delta_ok, "{:?}", report.failure_reasons);
        assert_eq!("1000000", report.wallet_usdc_after_atoms);
        assert_eq!("2000000", report.vault_usdc_after_atoms);
        assert_eq!(Some("0"), report.allowance_before_atoms.as_deref());
        assert!(!report.launch_session_managed_externally);
        assert!(!report.approve_skipped);
        assert_eq!(
            Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            report.approve_tx.as_deref()
        );
        assert_eq!(Some(55_835), report.approve_gas_used);
        assert_eq!("87500", report.deposit_gas_used.to_string());
        assert_eq!(2, report.receipt_watches.len());
        assert_eq!("approve_pfusdc_vault", report.receipt_watches[0].label);
        assert_eq!(
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            report.receipt_watches[0].tx_hash
        );
        assert_eq!("deposit_pfusdc_vault", report.receipt_watches[1].label);
        assert_eq!(
            "public_or_unknown_http",
            report.receipt_watches[1].source_rpc_provider_class
        );
        assert_eq!("stakehub_agent_response", report.receipt_watches[1].confirmation_source);
        assert_eq!("confirmed", report.receipt_watches[1].status);
        assert_eq!(87_500, report.receipt_watches[1].gas_used);
        assert!(artifact_dir.join("approve.calldata.txt").exists());
        assert!(artifact_dir.join("deposit.calldata.txt").exists());
        let observed = requests.lock().expect("requests lock");
        assert_eq!(6, observed.len());
        assert_eq!(
            Some("open_launch_session"),
            observed[2].get("op").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("approve_pfusdc_vault"),
            observed[3].get("session_action").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("deposit_pfusdc_vault"),
            observed[4].get("session_action").and_then(serde_json::Value::as_str)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_evm_deposit_skips_approval_when_allowance_is_warm() {
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;
        use std::sync::{Arc, Mutex};

        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-evm-deposit-warm-allowance-{}",
            process::id()
        ));
        let artifact_dir = root.join("artifacts");
        let stakehub_home = root.join("stakehub");
        let fake_cast = root.join("cast");
        let deposit_marker = root.join("deposit-landed");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&stakehub_home).expect("create stakehub home");
        let socket_path = stakehub_home.join("agent.sock");

        let vault = "0x1111111111111111111111111111111111111111";
        let usdc = "0x2222222222222222222222222222222222222222";
        let wallet = "0x3333333333333333333333333333333333333333";
        let buyer = "pf07381735ddb7de134e8be8402b465c9cd8ec7546";
        let nonce = "0x5ce1dfc7d8030b1b39098e74ddb586102335b4118269b29e95a3494e4d54de3a";
        let fake_cast_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = "calldata" ]; then
  if [ "$2" = "approve(address,uint256)" ]; then
    echo 0x095ea7b3
    exit 0
  fi
  if [ "$2" = "deposit(uint256,string,bytes32)" ]; then
    echo 0xabcdef01
    exit 0
  fi
fi
if [ "$1" = "call" ]; then
  sig="$3"
  arg="${{4:-}}"
  if [ "$sig" = "balanceOf(address)(uint256)" ]; then
    if [ "$arg" = "{wallet}" ]; then
      if [ -f "{deposit_marker}" ]; then echo 1000000; else echo 2000000; fi
      exit 0
    fi
    if [ "$arg" = "{vault}" ]; then
      if [ -f "{deposit_marker}" ]; then echo 2000000; else echo 1000000; fi
      exit 0
    fi
  fi
  if [ "$sig" = "allowance(address,address)(uint256)" ]; then
    echo 1000000
    exit 0
  fi
fi
echo "unexpected cast args: $*" >&2
exit 9
"#,
            deposit_marker = deposit_marker.display(),
        );
        std::fs::write(&fake_cast, fake_cast_script).expect("write fake cast");
        let mut permissions = std::fs::metadata(&fake_cast)
            .expect("fake cast metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&fake_cast, permissions).expect("chmod fake cast");

        let listener = UnixListener::bind(&socket_path).expect("bind fake agent");
        let requests = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let requests_for_thread = Arc::clone(&requests);
        let marker_for_thread = deposit_marker.clone();
        let agent_thread = std::thread::spawn(move || {
            for _ in 0..5 {
                let (stream, _) = listener.accept().expect("accept fake agent request");
                let mut reader = std::io::BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).expect("read fake agent request");
                let request: serde_json::Value =
                    serde_json::from_str(&line).expect("parse fake agent request");
                requests_for_thread
                    .lock()
                    .expect("requests lock")
                    .push(request.clone());
                let op = request.get("op").and_then(serde_json::Value::as_str).unwrap_or("");
                let response = match op {
                    "status" => serde_json::json!({"ok": true, "unlocked": true}),
                    "close_launch_session" => serde_json::json!({"ok": true, "closed": true}),
                    "open_launch_session" => serde_json::json!({
                        "ok": true,
                        "session": {
                            "id": request.get("session_id").cloned().unwrap_or(serde_json::Value::Null),
                        }
                    }),
                    "evm_contract_tx" => {
                        let action = request
                            .get("session_action")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("");
                        assert_eq!("deposit_pfusdc_vault", action);
                        std::fs::write(&marker_for_thread, "ok\n").expect("write deposit marker");
                        serde_json::json!({"ok": true, "tx": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "gas_used": 87500})
                    }
                    _ => serde_json::json!({"ok": false, "error": format!("unexpected op {op}")}),
                };
                let mut stream = reader.into_inner();
                writeln!(stream, "{}", serde_json::to_string(&response).expect("response json"))
                    .expect("write fake agent response");
            }
        });

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--evm-deposit-only".to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--source-rpc-url".to_string(),
            "https://arb.example.invalid/rpc".to_string(),
            "--cast-bin".to_string(),
            fake_cast.display().to_string(),
            "--stakehub-home".to_string(),
            stakehub_home.display().to_string(),
            "--source-chain-id".to_string(),
            "42161".to_string(),
            "--vault".to_string(),
            vault.to_string(),
            "--usdc".to_string(),
            usdc.to_string(),
            "--stakehub-wallet".to_string(),
            wallet.to_string(),
            "--pftl-recipient".to_string(),
            buyer.to_string(),
            "--amount-atoms".to_string(),
            "1000000".to_string(),
            "--nonce".to_string(),
            nonce.to_string(),
            "--session-id".to_string(),
            "nav-roundtrip-test-session".to_string(),
            "--require-warm-usdc-allowance".to_string(),
        ])
        .expect("EVM deposit stage cli");
        agent_thread.join().expect("fake agent thread");

        let report_file = artifact_dir.join("evm-deposit.json");
        let report = serde_json::from_str::<NavRoundtripEvmDepositReport>(
            &std::fs::read_to_string(&report_file).expect("read deposit report"),
        )
        .expect("parse deposit report");
        assert!(report.delta_ok, "{:?}", report.failure_reasons);
        assert_eq!(Some("1000000"), report.allowance_before_atoms.as_deref());
        assert!(!report.launch_session_managed_externally);
        assert!(report.approve_skipped);
        assert_eq!(None, report.approve_tx);
        assert_eq!(None, report.approve_gas_used);
        assert_eq!(1, report.receipt_watches.len());
        assert_eq!("deposit_pfusdc_vault", report.receipt_watches[0].label);
        assert_eq!("stakehub_agent_response", report.receipt_watches[0].confirmation_source);
        assert_eq!("confirmed", report.receipt_watches[0].status);
        let approve_artifact: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(artifact_dir.join("agent-approve.json"))
                .expect("read approve artifact"),
        )
        .expect("parse approve artifact");
        assert_eq!(
            Some(true),
            approve_artifact
                .get("skipped")
                .and_then(serde_json::Value::as_bool)
        );
        let observed = requests.lock().expect("requests lock");
        assert_eq!(5, observed.len());
        assert!(observed.iter().all(|request| {
            request
                .get("session_action")
                .and_then(serde_json::Value::as_str)
                != Some("approve_pfusdc_vault")
        }));
        assert_eq!(
            Some("deposit_pfusdc_vault"),
            observed[3].get("session_action").and_then(serde_json::Value::as_str)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_warm_usdc_allowance_only_approves_bounded_amount() {
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;
        use std::sync::{Arc, Mutex};

        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-warm-usdc-allowance-{}",
            process::id()
        ));
        let artifact_dir = root.join("artifacts");
        let stakehub_home = root.join("stakehub");
        let fake_cast = root.join("cast");
        let allowance_marker = root.join("allowance-warmed");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&stakehub_home).expect("create stakehub home");
        let socket_path = stakehub_home.join("agent.sock");

        let vault = "0x1111111111111111111111111111111111111111";
        let verifier = "0x2222222222222222222222222222222222222222";
        let usdc = "0x3333333333333333333333333333333333333333";
        let wallet = "0x4444444444444444444444444444444444444444";
        let fake_cast_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = "calldata" ]; then
  if [ "$2" = "approve(address,uint256)" ]; then
    echo 0x095ea7b3
    exit 0
  fi
fi
if [ "$1" = "call" ]; then
  sig="$3"
  if [ "$sig" = "allowance(address,address)(uint256)" ]; then
    if [ -f "{allowance_marker}" ]; then echo 3000000; else echo 0; fi
    exit 0
  fi
fi
echo "unexpected cast args: $*" >&2
exit 9
"#,
            allowance_marker = allowance_marker.display(),
        );
        std::fs::write(&fake_cast, fake_cast_script).expect("write fake cast");
        let mut permissions = std::fs::metadata(&fake_cast)
            .expect("fake cast metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&fake_cast, permissions).expect("chmod fake cast");

        let listener = UnixListener::bind(&socket_path).expect("bind fake agent");
        let requests = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let requests_for_thread = Arc::clone(&requests);
        let marker_for_thread = allowance_marker.clone();
        let agent_thread = std::thread::spawn(move || {
            for _ in 0..5 {
                let (stream, _) = listener.accept().expect("accept fake agent request");
                let mut reader = std::io::BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).expect("read fake agent request");
                let request: serde_json::Value =
                    serde_json::from_str(&line).expect("parse fake agent request");
                requests_for_thread
                    .lock()
                    .expect("requests lock")
                    .push(request.clone());
                let op = request.get("op").and_then(serde_json::Value::as_str).unwrap_or("");
                let response = match op {
                    "status" => serde_json::json!({"ok": true, "unlocked": true}),
                    "close_launch_session" => serde_json::json!({"ok": true, "closed": true}),
                    "open_launch_session" => serde_json::json!({
                        "ok": true,
                        "session": {
                            "id": request.get("session_id").cloned().unwrap_or(serde_json::Value::Null),
                        }
                    }),
                    "evm_contract_tx" => {
                        assert_eq!(
                            Some("approve_pfusdc_vault"),
                            request
                                .get("session_action")
                                .and_then(serde_json::Value::as_str)
                        );
                        assert_eq!(
                            Some(usdc),
                            request.get("to").and_then(serde_json::Value::as_str)
                        );
                        std::fs::write(&marker_for_thread, "ok\n")
                            .expect("write allowance marker");
                        serde_json::json!({"ok": true, "tx": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "gas_used": 55835})
                    }
                    _ => serde_json::json!({"ok": false, "error": format!("unexpected op {op}")}),
                };
                let mut stream = reader.into_inner();
                writeln!(stream, "{}", serde_json::to_string(&response).expect("response json"))
                    .expect("write fake agent response");
            }
        });

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--warm-usdc-allowance-only".to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--source-rpc-url".to_string(),
            "https://arb.example.invalid/rpc".to_string(),
            "--cast-bin".to_string(),
            fake_cast.display().to_string(),
            "--stakehub-home".to_string(),
            stakehub_home.display().to_string(),
            "--source-chain-id".to_string(),
            "42161".to_string(),
            "--vault".to_string(),
            vault.to_string(),
            "--verifier".to_string(),
            verifier.to_string(),
            "--usdc".to_string(),
            usdc.to_string(),
            "--stakehub-wallet".to_string(),
            wallet.to_string(),
            "--required-allowance-atoms".to_string(),
            "3000000".to_string(),
            "--session-id".to_string(),
            "nav-roundtrip-allowance-test".to_string(),
        ])
        .expect("warm USDC allowance cli");
        agent_thread.join().expect("fake agent thread");

        let report = serde_json::from_str::<NavRoundtripUsdcAllowanceSetupReport>(
            &std::fs::read_to_string(artifact_dir.join("allowance-setup.json"))
                .expect("read allowance setup report"),
        )
        .expect("parse allowance setup report");
        assert_eq!(NAV_ROUNDTRIP_USDC_ALLOWANCE_SETUP_REPORT_SCHEMA, report.schema);
        assert!(report.allowance_ok, "{:?}", report.failure_reasons);
        assert_eq!("3000000", report.required_allowance_atoms);
        assert_eq!("0", report.allowance_before_atoms);
        assert_eq!("3000000", report.allowance_after_atoms);
        assert!(!report.approve_skipped);
        assert_eq!(
            Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            report.approve_tx.as_deref()
        );
        assert_eq!(Some(55_835), report.approve_gas_used);
        assert!(report.stakehub_launch_session_open_file.is_some());
        assert!(report.stakehub_launch_session_close_file.is_some());
        assert_eq!(1, report.receipt_watches.len());
        assert_eq!("approve_pfusdc_vault", report.receipt_watches[0].label);
        let observed = requests.lock().expect("requests lock");
        assert_eq!(5, observed.len());
        assert_eq!(
            Some("open_launch_session"),
            observed[2].get("op").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some(3_000_000),
            observed[2].get("usdc_budget").and_then(serde_json::Value::as_u64)
        );
        assert_eq!(
            Some("evm_contract_tx"),
            observed[3].get("op").and_then(serde_json::Value::as_str)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_evm_deposit_reuses_externally_managed_launch_session() {
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;
        use std::sync::{Arc, Mutex};

        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-evm-deposit-external-session-{}",
            process::id()
        ));
        let artifact_dir = root.join("artifacts");
        let stakehub_home = root.join("stakehub");
        let fake_cast = root.join("cast");
        let deposit_marker = root.join("deposit-landed");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&stakehub_home).expect("create stakehub home");
        let socket_path = stakehub_home.join("agent.sock");

        let vault = "0x1111111111111111111111111111111111111111";
        let usdc = "0x2222222222222222222222222222222222222222";
        let wallet = "0x3333333333333333333333333333333333333333";
        let buyer = "pf07381735ddb7de134e8be8402b465c9cd8ec7546";
        let nonce = "0x5ce1dfc7d8030b1b39098e74ddb586102335b4118269b29e95a3494e4d54de3a";
        let fake_cast_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = "calldata" ]; then
  if [ "$2" = "approve(address,uint256)" ]; then
    echo 0x095ea7b3
    exit 0
  fi
  if [ "$2" = "deposit(uint256,string,bytes32)" ]; then
    echo 0xabcdef01
    exit 0
  fi
fi
if [ "$1" = "call" ]; then
  sig="$3"
  arg="${{4:-}}"
  if [ "$sig" = "balanceOf(address)(uint256)" ]; then
    if [ "$arg" = "{wallet}" ]; then
      if [ -f "{deposit_marker}" ]; then echo 1000000; else echo 2000000; fi
      exit 0
    fi
    if [ "$arg" = "{vault}" ]; then
      if [ -f "{deposit_marker}" ]; then echo 2000000; else echo 1000000; fi
      exit 0
    fi
  fi
  if [ "$sig" = "allowance(address,address)(uint256)" ]; then
    echo 1000000
    exit 0
  fi
fi
echo "unexpected cast args: $*" >&2
exit 9
"#,
            deposit_marker = deposit_marker.display(),
        );
        std::fs::write(&fake_cast, fake_cast_script).expect("write fake cast");
        let mut permissions = std::fs::metadata(&fake_cast)
            .expect("fake cast metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&fake_cast, permissions).expect("chmod fake cast");

        let listener = UnixListener::bind(&socket_path).expect("bind fake agent");
        let requests = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let requests_for_thread = Arc::clone(&requests);
        let marker_for_thread = deposit_marker.clone();
        let agent_thread = std::thread::spawn(move || {
            for _ in 0..2 {
                let (stream, _) = listener.accept().expect("accept fake agent request");
                let mut reader = std::io::BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).expect("read fake agent request");
                let request: serde_json::Value =
                    serde_json::from_str(&line).expect("parse fake agent request");
                requests_for_thread
                    .lock()
                    .expect("requests lock")
                    .push(request.clone());
                let op = request.get("op").and_then(serde_json::Value::as_str).unwrap_or("");
                let response = match op {
                    "status" => serde_json::json!({"ok": true, "unlocked": true}),
                    "evm_contract_tx" => {
                        assert_eq!(
                            Some("deposit_pfusdc_vault"),
                            request
                                .get("session_action")
                                .and_then(serde_json::Value::as_str)
                        );
                        std::fs::write(&marker_for_thread, "ok\n")
                            .expect("write deposit marker");
                        serde_json::json!({"ok": true, "tx": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "gas_used": 87500})
                    }
                    _ => serde_json::json!({"ok": false, "error": format!("unexpected op {op}")}),
                };
                let mut stream = reader.into_inner();
                writeln!(stream, "{}", serde_json::to_string(&response).expect("response json"))
                    .expect("write fake agent response");
            }
        });

        let report = nav_roundtrip_live_demo_evm_deposit(NavRoundtripEvmDepositOptions {
            artifact_dir: artifact_dir.clone(),
            source_rpc_url: "https://arb.example.invalid/rpc".to_string(),
            cast_binary: fake_cast.display().to_string(),
            stakehub_home: stakehub_home.clone(),
            source_chain_id: 42161,
            vault_address: vault.to_string(),
            usdc_address: usdc.to_string(),
            stakehub_wallet: wallet.to_string(),
            pftl_recipient: buyer.to_string(),
            amount_atoms: 1_000_000,
            nonce: nonce.to_string(),
            session_id: "nav-roundtrip-test-session".to_string(),
            resume: false,
            overwrite: false,
            agent_timeout_secs: 120,
            launch_session_managed_externally: true,
            require_warm_allowance: true,
        })
        .expect("EVM deposit with externally managed session");
        agent_thread.join().expect("fake agent thread");

        assert!(report.delta_ok, "{:?}", report.failure_reasons);
        assert!(report.launch_session_managed_externally);
        assert!(report.approve_skipped);
        let observed = requests.lock().expect("requests lock");
        assert_eq!(2, observed.len());
        assert_eq!(
            Some("status"),
            observed[0].get("op").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("evm_contract_tx"),
            observed[1].get("op").and_then(serde_json::Value::as_str)
        );
        assert!(observed.iter().all(|request| {
            !matches!(
                request.get("op").and_then(serde_json::Value::as_str),
                Some("open_launch_session" | "close_launch_session")
            )
        }));
        let open_artifact: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(artifact_dir.join("agent-open-session.json"))
                .expect("read open artifact"),
        )
        .expect("parse open artifact");
        assert_eq!(
            Some(true),
            open_artifact
                .get("skipped")
                .and_then(serde_json::Value::as_bool)
        );
        let close_artifact: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(artifact_dir.join("agent-close-session.json"))
                .expect("read close artifact"),
        )
        .expect("parse close artifact");
        assert_eq!(
            Some(true),
            close_artifact
                .get("skipped")
                .and_then(serde_json::Value::as_bool)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_evm_deposit_warns_on_wallet_delta_offset_when_vault_credit_matches() {
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;
        use std::sync::{Arc, Mutex};

        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-evm-deposit-offset-{}",
            process::id()
        ));
        let artifact_dir = root.join("artifacts");
        let stakehub_home = root.join("stakehub");
        let fake_cast = root.join("cast");
        let deposit_marker = root.join("deposit-landed");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&stakehub_home).expect("create stakehub home");
        let socket_path = stakehub_home.join("agent.sock");

        let vault = "0x1111111111111111111111111111111111111111";
        let usdc = "0x2222222222222222222222222222222222222222";
        let wallet = "0x3333333333333333333333333333333333333333";
        let buyer = "pf07381735ddb7de134e8be8402b465c9cd8ec7546";
        let nonce = "0x5ce1dfc7d8030b1b39098e74ddb586102335b4118269b29e95a3494e4d54de3a";
        let fake_cast_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = "calldata" ]; then
  if [ "$2" = "approve(address,uint256)" ]; then echo 0x095ea7b3; exit 0; fi
  if [ "$2" = "deposit(uint256,string,bytes32)" ]; then echo 0xabcdef01; exit 0; fi
fi
if [ "$1" = "call" ]; then
  sig="$3"
  arg="${{4:-}}"
  if [ "$sig" = "balanceOf(address)(uint256)" ]; then
    if [ "$arg" = "{wallet}" ]; then echo 2000000; exit 0; fi
    if [ "$arg" = "{vault}" ]; then
      if [ -f "{deposit_marker}" ]; then echo 2000000; else echo 1000000; fi
      exit 0
    fi
  fi
  if [ "$sig" = "allowance(address,address)(uint256)" ]; then echo 1000000; exit 0; fi
fi
if [ "$1" = "balance" ]; then echo 2000000000000000; exit 0; fi
echo "unexpected cast args: $*" >&2
exit 9
"#,
            deposit_marker = deposit_marker.display(),
        );
        std::fs::write(&fake_cast, fake_cast_script).expect("write fake cast");
        let mut permissions = std::fs::metadata(&fake_cast)
            .expect("fake cast metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&fake_cast, permissions).expect("chmod fake cast");

        let listener = UnixListener::bind(&socket_path).expect("bind fake agent");
        let requests = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let requests_for_thread = Arc::clone(&requests);
        let marker_for_thread = deposit_marker.clone();
        let agent_thread = std::thread::spawn(move || {
            for _ in 0..2 {
                let (stream, _) = listener.accept().expect("accept fake agent request");
                let mut reader = std::io::BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).expect("read fake agent request");
                let request: serde_json::Value =
                    serde_json::from_str(&line).expect("parse fake agent request");
                requests_for_thread
                    .lock()
                    .expect("requests lock")
                    .push(request.clone());
                let op = request.get("op").and_then(serde_json::Value::as_str).unwrap_or("");
                let response = match op {
                    "status" => serde_json::json!({"ok": true, "unlocked": true}),
                    "evm_contract_tx" => {
                        assert_eq!(
                            Some("deposit_pfusdc_vault"),
                            request
                                .get("session_action")
                                .and_then(serde_json::Value::as_str)
                        );
                        std::fs::write(&marker_for_thread, "ok\n")
                            .expect("write deposit marker");
                        serde_json::json!({"ok": true, "tx": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "gas_used": 87500})
                    }
                    _ => serde_json::json!({"ok": false, "error": format!("unexpected op {op}")}),
                };
                let mut stream = reader.into_inner();
                writeln!(stream, "{}", serde_json::to_string(&response).expect("response json"))
                    .expect("write fake agent response");
            }
        });

        let report = nav_roundtrip_live_demo_evm_deposit(NavRoundtripEvmDepositOptions {
            artifact_dir: artifact_dir.clone(),
            source_rpc_url: "https://arb.example.invalid/rpc".to_string(),
            cast_binary: fake_cast.display().to_string(),
            stakehub_home: stakehub_home.clone(),
            source_chain_id: 42161,
            vault_address: vault.to_string(),
            usdc_address: usdc.to_string(),
            stakehub_wallet: wallet.to_string(),
            pftl_recipient: buyer.to_string(),
            amount_atoms: 1_000_000,
            nonce: nonce.to_string(),
            session_id: "nav-roundtrip-test-session".to_string(),
            resume: false,
            overwrite: false,
            agent_timeout_secs: 120,
            launch_session_managed_externally: true,
            require_warm_allowance: true,
        })
        .expect("EVM deposit with offset wallet delta");
        agent_thread.join().expect("fake agent thread");

        assert!(report.delta_ok, "{:?}", report.failure_reasons);
        assert!(report.failure_reasons.is_empty());
        assert_eq!(
            vec!["wallet USDC delta was 0, expected 1000000".to_string()],
            report.delta_warnings
        );
        assert_eq!("2000000", report.wallet_usdc_before_atoms);
        assert_eq!("2000000", report.wallet_usdc_after_atoms);
        assert_eq!("1000000", report.vault_usdc_before_atoms);
        assert_eq!("2000000", report.vault_usdc_after_atoms);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_warm_stakehub_session_opens_union_allowlist_and_closes() {
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::net::UnixListener;
        use std::sync::{Arc, Mutex};

        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-warm-session-{}",
            process::id()
        ));
        let stakehub_home = root.join("stakehub");
        let artifact_dir = root.join("artifacts");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&stakehub_home).expect("create stakehub home");
        let socket_path = stakehub_home.join("agent.sock");

        let listener = UnixListener::bind(&socket_path).expect("bind fake agent");
        let requests = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let requests_for_thread = Arc::clone(&requests);
        let agent_thread = std::thread::spawn(move || {
            for _ in 0..4 {
                let (stream, _) = listener.accept().expect("accept fake agent request");
                let mut reader = std::io::BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).expect("read fake agent request");
                let request: serde_json::Value =
                    serde_json::from_str(&line).expect("parse fake agent request");
                requests_for_thread
                    .lock()
                    .expect("requests lock")
                    .push(request.clone());
                let response = match request
                    .get("op")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                {
                    "status" => serde_json::json!({"ok": true, "unlocked": true}),
                    "close_launch_session" => serde_json::json!({"ok": true, "closed": true}),
                    "open_launch_session" => serde_json::json!({"ok": true, "session": {"id": request.get("session_id").cloned().unwrap_or(serde_json::Value::Null)}}),
                    op => serde_json::json!({"ok": false, "error": format!("unexpected op {op}")}),
                };
                let mut stream = reader.into_inner();
                writeln!(stream, "{}", serde_json::to_string(&response).expect("response json"))
                    .expect("write fake agent response");
            }
        });

        let mut guard = NavRoundtripStakeHubLaunchSessionGuard::open(
            &stakehub_home,
            &artifact_dir,
            "nav-roundtrip-warm-session",
            42161,
            "0x1111111111111111111111111111111111111111",
            "0x2222222222222222222222222222222222222222",
            "0x3333333333333333333333333333333333333333",
            "0x4444444444444444444444444444444444444444",
            1_000_000,
            120,
        )
        .expect("open warm StakeHub session");
        guard.close().expect("close warm StakeHub session");
        agent_thread.join().expect("fake agent thread");

        assert!(artifact_dir.join("agent-status.json").exists());
        assert!(artifact_dir.join("agent-close-existing-session.json").exists());
        assert!(artifact_dir.join("agent-open-session.json").exists());
        assert!(artifact_dir.join("agent-close-session.json").exists());
        let observed = requests.lock().expect("requests lock");
        assert_eq!(4, observed.len());
        assert_eq!(
            Some("status"),
            observed[0].get("op").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("close_launch_session"),
            observed[1].get("op").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("open_launch_session"),
            observed[2].get("op").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("close_launch_session"),
            observed[3].get("op").and_then(serde_json::Value::as_str)
        );
        let allowlist = observed[2]
            .get("allowlist")
            .and_then(serde_json::Value::as_array)
            .expect("open allowlist");
        assert_eq!(4, allowlist.len());
        assert_eq!(
            Some("claim-withdrawal"),
            observed[2]
                .get("close_after_action")
                .and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some(1_000_000),
            observed[2]
                .get("usdc_budget")
                .and_then(serde_json::Value::as_u64)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_deposit_relay_builds_bundle_and_batches_ops() {
        use std::os::unix::fs::PermissionsExt;

        let root = env::temp_dir().join(format!("postfiat-nav-roundtrip-deposit-relay-{}", process::id()));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let artifact_dir = root.join("artifacts");
        let evm_deposit_report_file = root.join("evm-deposit.json");
        let fake_cast = root.join("cast");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init");
        write_local_topology(TopologyOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            validators: 1,
            base_port: 44_040,
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
        })
        .expect("write local topology");
        let faucet = faucet_key(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("faucet key");
        let faucet_key_file = data_dir.join("faucet_key.json");
        let evidence = vault_bridge_deposit_evidence_fixture();
        let source_domain = evidence.source_domain();
        let policy_hash = "24".repeat(48);
        let pfusdc_definition =
            postfiat_types::AssetDefinition::new(DEFAULT_CHAIN_ID, &faucet.address, "PFUSDC", 1, 6)
                .expect("pfusdc asset definition");
        let pfusdc_asset_id = pfusdc_definition.asset_id.clone();
        let source_class = format!(
            "{}{}",
            postfiat_types::VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX,
            source_domain
        );
        let pfusdc_profile = postfiat_types::NavProofProfile::new(
            faucet.address.clone(),
            postfiat_types::NAV_PROFILE_VERIFIER_MULTI_FETCH,
            source_class,
            100,
            1,
            100,
            0,
            0,
            1,
            0,
            policy_hash.clone(),
            "",
            "",
            0,
            0,
        )
        .expect("pfusdc profile");
        let pfusdc_nav_asset = postfiat_types::NavTrackedAsset::new(
            pfusdc_asset_id.clone(),
            faucet.address.clone(),
            faucet.address.clone(),
            pfusdc_profile.profile_id.clone(),
            "USDC",
            faucet.address.clone(),
        )
        .expect("pfusdc nav asset");
        let evidence_root =
            vault_bridge_deposit_evidence_root(&evidence).expect("bridge evidence root");
        let store = postfiat_storage::NodeStore::new(&data_dir);
        let mut ledger = store.read_ledger().expect("read test ledger");
        ledger.asset_definitions.push(pfusdc_definition);
        ledger.nav_proof_profiles.push(pfusdc_profile);
        ledger.nav_assets.push(pfusdc_nav_asset);
        ledger.nav_attestors.push(NavAttestor {
            address: faucet.address.clone(),
            domain: "operator.local".to_string(),
            bond: 0,
            registered_at_height: 1,
        });
        store.write_ledger(&ledger).expect("write clean bridge ledger");

        let propose_attest_base_data_dir = root.join("propose-attest-base-node");
        copy_test_dir_all(&data_dir, &propose_attest_base_data_dir);

        let mut finalized_deposit = postfiat_types::VaultBridgeDepositRecord::new(
            pfusdc_asset_id.clone(),
            evidence_root.clone(),
            evidence.clone(),
            policy_hash.clone(),
            "",
            "",
            "",
            faucet.address.clone(),
            1,
            100,
        )
        .expect("finalized bridge deposit");
        finalized_deposit.status =
            postfiat_types::VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED.to_string();
        finalized_deposit.finalized_at_height = 1;
        finalized_deposit
            .validate()
            .expect("valid finalized bridge deposit");
        let mut ledger = store.read_ledger().expect("read clean bridge ledger");
        ledger.vault_bridge_deposits.push(finalized_deposit);
        store.write_ledger(&ledger).expect("write test ledger");

        let mut deposit_log = vault_bridge_vault_deposit_log_json(&evidence);
        deposit_log
            .as_object_mut()
            .expect("deposit log object")
            .remove("blockHash");
        deposit_log
            .as_object_mut()
            .expect("deposit log object")
            .remove("transactionHash");
        let receipt = serde_json::json!({
            "status": "0x1",
            "blockHash": format!("0x{}", evidence.block_hash),
            "blockNumber": "0x64",
            "transactionHash": format!("0x{}", evidence.tx_hash),
            "logs": [deposit_log],
        });
        let block = serde_json::json!({
            "hash": format!("0x{}", evidence.block_hash),
            "number": "0x64",
        });
        let fake_cast_script = format!(
            "#!/usr/bin/env bash\nset -euo pipefail\ncase \"$1\" in\n  receipt)\n    if [ \"$2\" != \"--rpc-url\" ]; then exit 3; fi\n    if [ \"$4\" != \"--json\" ]; then exit 4; fi\n    cat <<'JSON'\n{}\nJSON\n    ;;\n  block)\n    if [ \"$2\" != \"--rpc-url\" ]; then exit 5; fi\n    if [ \"$4\" != \"0x{}\" ]; then exit 6; fi\n    if [ \"$5\" != \"--json\" ]; then exit 7; fi\n    cat <<'JSON'\n{}\nJSON\n    ;;\n  block-number)\n    if [ \"$2\" != \"--rpc-url\" ]; then exit 8; fi\n    printf '0x6d\\n'\n    ;;\n  *)\n    exit 2\n    ;;\nesac\n",
            serde_json::to_string_pretty(&receipt).expect("receipt json"),
            evidence.block_hash,
            serde_json::to_string_pretty(&block).expect("block json")
        );
        std::fs::write(&fake_cast, fake_cast_script).expect("write fake cast");
        let mut permissions = std::fs::metadata(&fake_cast)
            .expect("fake cast metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&fake_cast, permissions).expect("chmod fake cast");

        let evm_report = NavRoundtripEvmDepositReport {
            schema: NAV_ROUNDTRIP_EVM_DEPOSIT_REPORT_SCHEMA.to_string(),
            artifact_file: evm_deposit_report_file.display().to_string(),
            source_rpc_url: "https://source-chain.example.invalid/rpc".to_string(),
            source_rpc_provider_class: "public_or_unknown_http".to_string(),
            source_chain_id: 42161,
            vault_address: evidence.vault_address.clone(),
            usdc_address: evidence.token_address.clone(),
            stakehub_wallet: "0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0".to_string(),
            pftl_recipient: faucet.address.clone(),
            amount_atoms: evidence.amount_atoms,
            nonce: format!("0x{}", evidence.nonce),
            session_id: "relay-test".to_string(),
            wallet_usdc_before_atoms: "2000000".to_string(),
            wallet_usdc_after_atoms: "1000000".to_string(),
            vault_usdc_before_atoms: "1000000".to_string(),
            vault_usdc_after_atoms: "2000000".to_string(),
            launch_session_managed_externally: false,
            allowance_before_atoms: Some(evidence.amount_atoms.to_string()),
            approve_skipped: true,
            approve_tx: None,
            approve_gas_used: None,
            deposit_tx: format!("0x{}", evidence.tx_hash),
            deposit_gas_used: 87_500,
            approve_calldata_file: root.join("approve.calldata.txt").display().to_string(),
            deposit_calldata_file: root.join("deposit.calldata.txt").display().to_string(),
            agent_open_session_file: root.join("agent-open-session.json").display().to_string(),
            agent_approve_file: root.join("agent-approve.json").display().to_string(),
            agent_deposit_file: root.join("agent-deposit.json").display().to_string(),
            agent_close_session_file: root.join("agent-close-session.json").display().to_string(),
            receipt_watches: Vec::new(),
            delta_ok: true,
            delta_warnings: Vec::new(),
            failure_reasons: Vec::new(),
        };
        write_json_file(&evm_deposit_report_file, &evm_report).expect("write evm report");

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--deposit-relay-only".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            data_dir.join(VALIDATOR_KEYS_FILE).display().to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--evm-deposit-report".to_string(),
            evm_deposit_report_file.display().to_string(),
            "--source-rpc-url".to_string(),
            "https://source-chain.example.invalid/rpc".to_string(),
            "--cast-bin".to_string(),
            fake_cast.display().to_string(),
            "--vault".to_string(),
            evidence.vault_address.clone(),
            "--usdc".to_string(),
            evidence.token_address.clone(),
            "--pfusdc".to_string(),
            pfusdc_asset_id.clone(),
            "--policy-hash".to_string(),
            policy_hash,
            "--proposer".to_string(),
            faucet.address.clone(),
            "--attestor".to_string(),
            faucet.address.clone(),
            "--finalizer".to_string(),
            faucet.address.clone(),
            "--claimer".to_string(),
            faucet.address.clone(),
            "--proposer-key-file".to_string(),
            faucet_key_file.display().to_string(),
            "--attestor-key-file".to_string(),
            faucet_key_file.display().to_string(),
            "--finalizer-key-file".to_string(),
            faucet_key_file.display().to_string(),
            "--claimer-key-file".to_string(),
            faucet_key_file.display().to_string(),
            "--issuer-key-file".to_string(),
            faucet_key_file.display().to_string(),
            "--expires-at-height".to_string(),
            "100".to_string(),
            "--prepare-only".to_string(),
        ])
        .expect("deposit relay stage cli");

        let report = serde_json::from_str::<NavRoundtripDepositRelayReport>(
            &std::fs::read_to_string(artifact_dir.join("deposit-relay.json"))
                .expect("read deposit relay report"),
        )
        .expect("parse deposit relay report");
        assert_eq!(NAV_ROUNDTRIP_DEPOSIT_RELAY_REPORT_SCHEMA, report.schema);
        assert_eq!(format!("0x{}", evidence.tx_hash), report.deposit_tx);
        assert!(!report.claim_deposit);
        assert_eq!(5, report.certified_ops.operation_count);
        assert_eq!(
            vec![
                "propose".to_string(),
                "attest".to_string(),
                "finalize".to_string(),
                "receipt-submit".to_string(),
                "receipt-count".to_string()
            ],
            report.certified_ops.operations
                .iter()
                .map(|operation| operation.label.clone())
                .collect::<Vec<_>>()
        );
        assert!(report.certified_ops.prepare_only);
        assert_eq!(
            1,
            report
                .certified_ops
                .dependency_report
                .same_round_dependency_count
        );
        assert_eq!(
            0,
            report
                .certified_ops
                .dependency_report
                .prior_round_dependency_count
        );
        assert_eq!(
            vec!["vault_bridge_receipt_submit_count".to_string()],
            report.certified_ops.dependency_report.candidate_batch_classes
        );
        assert!(artifact_dir.join("deposit-relay-bundle").join("commands.sh").exists());
        assert!(artifact_dir
            .join("deposit-relay-certified")
            .join("request.normalized.json")
            .exists());

        let replay_request =
            read_certified_asset_ops_request(std::path::Path::new(&report.certified_ops_file))
                .expect("read deposit relay replay request");
        validate_certified_asset_ops_request(&replay_request)
            .expect("deposit relay replay request validates");
        let propose_op = replay_request
            .operations
            .iter()
            .find(|operation| operation.label == "propose")
            .cloned()
            .expect("propose op");
        let mut attest_op = replay_request
            .operations
            .iter()
            .find(|operation| operation.label == "attest")
            .cloned()
            .expect("attest op");
        attest_op.dependencies.push(CertifiedAssetOpDependency {
            label: "propose".to_string(),
            mode: "same_round".to_string(),
            reason: Some(
                "attestation signs the same deterministic bridge evidence proposed in this round"
                    .to_string(),
            ),
        });
        let propose_attest_replay_request = CertifiedAssetOpsRequest {
            schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            operations: vec![propose_op.clone(), attest_op.clone()],
        };
        validate_certified_asset_ops_request(&propose_attest_replay_request)
            .expect("propose/attest replay request validates");
        assert_eq!(
            vec!["vault_bridge_deposit_propose_attest".to_string()],
            certified_asset_ops_dependency_report(&propose_attest_replay_request)
                .candidate_batch_classes
        );
        let receipt_submit_op = replay_request
            .operations
            .iter()
            .find(|operation| operation.label == "receipt-submit")
            .cloned()
            .expect("receipt submit op");
        let receipt_count_op = replay_request
            .operations
            .iter()
            .find(|operation| operation.label == "receipt-count")
            .cloned()
            .expect("receipt count op");
        let receipt_replay_request = CertifiedAssetOpsRequest {
            schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            operations: vec![receipt_submit_op.clone(), receipt_count_op.clone()],
        };
        validate_certified_asset_ops_request(&receipt_replay_request)
            .expect("receipt replay request validates");
        assert_eq!(
            vec!["vault_bridge_receipt_submit_count".to_string()],
            certified_asset_ops_dependency_report(&receipt_replay_request).candidate_batch_classes
        );

        let propose_attest_replay_artifact_dir =
            root.join("deposit-relay-propose-attest-replay-signing");
        let propose_attest_replay_options = CertifiedAssetOpsBatchOptions {
            data_dir: propose_attest_base_data_dir.clone(),
            topology_file: topology_file.clone(),
            key_file: data_dir.join(VALIDATOR_KEYS_FILE),
            proposal_key_file: None,
            ops_file: std::path::PathBuf::from(&report.certified_ops_file),
            artifact_dir: propose_attest_replay_artifact_dir,
            max_transactions: None,
            require_local_proposer: true,
            require_signed_proposal: true,
            allow_peer_failures: false,
            quorum_early_full_propagation: false,
            local_apply_before_certified_send: true,
            defer_certified_sends: false,
            block_height: None,
            view: None,
            timeout_certificate_file: None,
            timeout_ms: 5_000,
            send_retries: 0,
            retry_backoff_ms: 250,
            allow_existing_mempool: false,
            resume: false,
            overwrite: false,
            prepare_only: false,
            batch_only: false,
        };
        let propose_report =
            run_certified_asset_op_stage(&propose_op, &propose_attest_replay_options, false, None)
                .expect("sign replay deposit propose op");
        let attest_report = run_certified_asset_op_stage(
            &attest_op,
            &propose_attest_replay_options,
            false,
            propose_report.sequence.map(|sequence| sequence + 1),
        )
        .expect("sign replay deposit attest op");
        let propose_signed = propose_report
            .signed_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .expect("deposit propose signed file");
        let attest_signed = attest_report
            .signed_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .expect("deposit attest signed file");

        let propose_attest_unbatched_data_dir = root.join("propose-attest-unbatched-node");
        let propose_attest_batched_data_dir = root.join("propose-attest-batched-node");
        copy_test_dir_all(
            &propose_attest_base_data_dir,
            &propose_attest_unbatched_data_dir,
        );
        copy_test_dir_all(
            &propose_attest_base_data_dir,
            &propose_attest_batched_data_dir,
        );

        let unbatched_propose_batch = root.join("propose-attest-unbatched-propose.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: propose_attest_unbatched_data_dir.clone(),
            batch_file: unbatched_propose_batch.clone(),
            signed_asset_transaction_files: vec![propose_signed.clone()],
        })
        .expect("create unbatched deposit propose batch");
        let propose_receipts = apply_batch(ApplyBatchOptions {
            data_dir: propose_attest_unbatched_data_dir.clone(),
            batch_file: unbatched_propose_batch,
            certificate_file: None,
        })
        .expect("apply unbatched deposit propose");
        assert!(propose_receipts.iter().all(|receipt| receipt.accepted), "{propose_receipts:?}");

        let unbatched_attest_batch = root.join("propose-attest-unbatched-attest.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: propose_attest_unbatched_data_dir.clone(),
            batch_file: unbatched_attest_batch.clone(),
            signed_asset_transaction_files: vec![attest_signed.clone()],
        })
        .expect("create unbatched deposit attest batch");
        let attest_receipts = apply_batch(ApplyBatchOptions {
            data_dir: propose_attest_unbatched_data_dir.clone(),
            batch_file: unbatched_attest_batch,
            certificate_file: None,
        })
        .expect("apply unbatched deposit attest");
        assert!(attest_receipts.iter().all(|receipt| receipt.accepted), "{attest_receipts:?}");

        let propose_attest_batched_file = root.join("propose-attest-batched.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: propose_attest_batched_data_dir.clone(),
            batch_file: propose_attest_batched_file.clone(),
            signed_asset_transaction_files: vec![propose_signed, attest_signed],
        })
        .expect("create deposit propose/attest same-round batch");
        let propose_attest_batched_receipts = apply_batch(ApplyBatchOptions {
            data_dir: propose_attest_batched_data_dir.clone(),
            batch_file: propose_attest_batched_file,
            certificate_file: None,
        })
        .expect("apply deposit propose/attest same-round batch");
        assert!(
            propose_attest_batched_receipts
                .iter()
                .all(|receipt| receipt.accepted),
            "{propose_attest_batched_receipts:?}"
        );

        let propose_attest_unbatched_status = status(NodeOptions {
            data_dir: propose_attest_unbatched_data_dir.clone(),
        })
        .expect("unbatched deposit propose/attest status");
        let propose_attest_batched_status = status(NodeOptions {
            data_dir: propose_attest_batched_data_dir.clone(),
        })
        .expect("batched deposit propose/attest status");
        let propose_attest_unbatched_ledger =
            postfiat_storage::NodeStore::new(&propose_attest_unbatched_data_dir)
                .read_ledger()
                .expect("read unbatched deposit propose/attest ledger");
        let propose_attest_batched_ledger =
            postfiat_storage::NodeStore::new(&propose_attest_batched_data_dir)
                .read_ledger()
                .expect("read batched deposit propose/attest ledger");
        assert_eq!(1, propose_attest_unbatched_ledger.vault_bridge_deposits.len());
        assert_eq!(1, propose_attest_batched_ledger.vault_bridge_deposits.len());
        assert_eq!(
            1,
            propose_attest_unbatched_ledger.vault_bridge_deposits[0]
                .attestations
                .len()
        );
        assert_eq!(
            1,
            propose_attest_batched_ledger.vault_bridge_deposits[0]
                .attestations
                .len()
        );
        let mut propose_attest_unbatched_ledger_facing_state =
            propose_attest_unbatched_ledger.clone();
        let mut propose_attest_batched_ledger_facing_state =
            propose_attest_batched_ledger.clone();
        for deposit in &mut propose_attest_unbatched_ledger_facing_state.vault_bridge_deposits {
            deposit.submitted_at_height = 1;
            for attestation in &mut deposit.attestations {
                attestation.attested_at_height = 1;
            }
        }
        for deposit in &mut propose_attest_batched_ledger_facing_state.vault_bridge_deposits {
            deposit.submitted_at_height = 1;
            for attestation in &mut deposit.attestations {
                attestation.attested_at_height = 1;
            }
        }
        assert_eq!(
            propose_attest_unbatched_ledger_facing_state,
            propose_attest_batched_ledger_facing_state,
            "deposit propose/attest batching must preserve ledger-facing evidence state; block-height provenance may differ"
        );
        assert_eq!(2, propose_attest_unbatched_status.block_height);
        assert_eq!(1, propose_attest_batched_status.block_height);

        let propose_attest_state_root_match =
            propose_attest_unbatched_status.state_root == propose_attest_batched_status.state_root;
        let mut propose_attest_corpus_report = serde_json::json!({
            "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
            "case": "vault-bridge-deposit-propose-attest",
            "candidate_batch_class": "vault_bridge_deposit_propose_attest",
            "unbatched_block_height": propose_attest_unbatched_status.block_height,
            "batched_block_height": propose_attest_batched_status.block_height,
            "unbatched_state_root": propose_attest_unbatched_status.state_root,
            "batched_state_root": propose_attest_batched_status.state_root,
            "state_root_match": propose_attest_state_root_match,
            "ledger_facing_state_match": true,
            "safe_for_live_round_compression": true,
            "gate": "ledger-facing evidence-equivalent deposit propose/attest replay; deposit is pending and attested in both paths, while block-height provenance may differ because the unbatched path commits two ordered blocks and same-round batching commits one",
        });
        if !propose_attest_state_root_match {
            propose_attest_corpus_report["intended_state_root_difference"] = serde_json::json!(
                "unbatched replay commits deposit propose and attest as two ordered blocks while same-round batching commits one block; ledger-facing evidence state is identical after normalizing submitted and attested block-height provenance"
            );
        }
        let propose_attest_corpus_file =
            root.join("vault-bridge-deposit-propose-attest-equivalence.json");
        write_json_file(&propose_attest_corpus_file, &propose_attest_corpus_report)
            .expect("write deposit propose/attest corpus");
        let propose_attest_corpus_verify =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(propose_attest_corpus_file),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: true,
                required_candidate_classes: vec![
                    "vault_bridge_deposit_propose_attest".to_string()
                ],
                strict_exit: true,
            })
            .expect("deposit propose/attest corpus verifies");
        assert!(
            propose_attest_corpus_verify.passed,
            "{:?}",
            propose_attest_corpus_verify.failure_reasons
        );
        assert_eq!(1, propose_attest_corpus_verify.live_ready_case_count);
        assert_eq!(
            Some(true),
            propose_attest_corpus_verify.cases[0].ledger_facing_state_match
        );

        let replay_artifact_dir = root.join("deposit-relay-receipt-replay-signing");
        let replay_options = CertifiedAssetOpsBatchOptions {
            data_dir: data_dir.clone(),
            topology_file: topology_file.clone(),
            key_file: data_dir.join(VALIDATOR_KEYS_FILE),
            proposal_key_file: None,
            ops_file: std::path::PathBuf::from(&report.certified_ops_file),
            artifact_dir: replay_artifact_dir,
            max_transactions: None,
            require_local_proposer: true,
            require_signed_proposal: true,
            allow_peer_failures: false,
            quorum_early_full_propagation: false,
            local_apply_before_certified_send: true,
            defer_certified_sends: false,
            block_height: None,
            view: None,
            timeout_certificate_file: None,
            timeout_ms: 5_000,
            send_retries: 0,
            retry_backoff_ms: 250,
            allow_existing_mempool: false,
            resume: false,
            overwrite: false,
            prepare_only: false,
            batch_only: false,
        };
        let submit_report =
            run_certified_asset_op_stage(&receipt_submit_op, &replay_options, false, None)
                .expect("sign replay receipt submit op");
        let count_report = run_certified_asset_op_stage(
            &receipt_count_op,
            &replay_options,
            false,
            submit_report.sequence.map(|sequence| sequence + 1),
        )
        .expect("sign replay receipt count op");
        let submit_signed = submit_report
            .signed_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .expect("receipt submit signed file");
        let count_signed = count_report
            .signed_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .expect("receipt count signed file");

        let unbatched_data_dir = root.join("receipt-unbatched-node");
        let batched_data_dir = root.join("receipt-batched-node");
        copy_test_dir_all(&data_dir, &unbatched_data_dir);
        copy_test_dir_all(&data_dir, &batched_data_dir);

        let unbatched_submit_batch = root.join("receipt-unbatched-submit.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_submit_batch.clone(),
            signed_asset_transaction_files: vec![submit_signed.clone()],
        })
        .expect("create unbatched receipt submit batch");
        let submit_receipts = apply_batch(ApplyBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_submit_batch,
            certificate_file: None,
        })
        .expect("apply unbatched receipt submit");
        assert!(submit_receipts.iter().all(|receipt| receipt.accepted), "{submit_receipts:?}");

        let unbatched_count_batch = root.join("receipt-unbatched-count.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_count_batch.clone(),
            signed_asset_transaction_files: vec![count_signed.clone()],
        })
        .expect("create unbatched receipt count batch");
        let count_receipts = apply_batch(ApplyBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_count_batch,
            certificate_file: None,
        })
        .expect("apply unbatched receipt count");
        assert!(count_receipts.iter().all(|receipt| receipt.accepted), "{count_receipts:?}");

        let batched_file = root.join("receipt-batched.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: batched_data_dir.clone(),
            batch_file: batched_file.clone(),
            signed_asset_transaction_files: vec![submit_signed, count_signed],
        })
        .expect("create receipt same-round batch");
        let batched_receipts = apply_batch(ApplyBatchOptions {
            data_dir: batched_data_dir.clone(),
            batch_file: batched_file,
            certificate_file: None,
        })
        .expect("apply receipt same-round batch");
        assert!(batched_receipts.iter().all(|receipt| receipt.accepted), "{batched_receipts:?}");

        let unbatched_status = status(NodeOptions {
            data_dir: unbatched_data_dir.clone(),
        })
        .expect("unbatched receipt status");
        let batched_status = status(NodeOptions {
            data_dir: batched_data_dir.clone(),
        })
        .expect("batched receipt status");
        let unbatched_ledger = postfiat_storage::NodeStore::new(&unbatched_data_dir)
            .read_ledger()
            .expect("read unbatched receipt ledger");
        let batched_ledger = postfiat_storage::NodeStore::new(&batched_data_dir)
            .read_ledger()
            .expect("read batched receipt ledger");
        assert_eq!(
            1,
            unbatched_ledger
                .vault_bridge_receipts
                .iter()
                .filter(|receipt| {
                    receipt.status == postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
                })
                .count()
        );
        assert_eq!(
            1,
            batched_ledger
                .vault_bridge_receipts
                .iter()
                .filter(|receipt| {
                    receipt.status == postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
                })
                .count()
        );
        let mut unbatched_ledger_facing_state = unbatched_ledger.clone();
        let mut batched_ledger_facing_state = batched_ledger.clone();
        for receipt in &mut unbatched_ledger_facing_state.vault_bridge_receipts {
            if receipt.status == postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED {
                receipt.finalized_at_height = 1;
                receipt.counted_at_height = 1;
            }
        }
        for receipt in &mut batched_ledger_facing_state.vault_bridge_receipts {
            if receipt.status == postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED {
                receipt.finalized_at_height = 1;
                receipt.counted_at_height = 1;
            }
        }
        for bucket in &mut unbatched_ledger_facing_state.vault_bridge_bucket_states {
            bucket.last_updated_height = 1;
        }
        for bucket in &mut batched_ledger_facing_state.vault_bridge_bucket_states {
            bucket.last_updated_height = 1;
        }
        assert_eq!(
            unbatched_ledger_facing_state, batched_ledger_facing_state,
            "receipt submit/count batching must preserve ledger-facing accounting state; block-height provenance may differ"
        );
        assert_eq!(2, unbatched_status.block_height);
        assert_eq!(1, batched_status.block_height);

        let state_root_match = unbatched_status.state_root == batched_status.state_root;
        let mut corpus_report = serde_json::json!({
            "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
            "case": "vault-bridge-receipt-submit-count",
            "candidate_batch_class": "vault_bridge_receipt_submit_count",
            "unbatched_block_height": unbatched_status.block_height,
            "batched_block_height": batched_status.block_height,
            "unbatched_state_root": unbatched_status.state_root,
            "batched_state_root": batched_status.state_root,
            "state_root_match": state_root_match,
            "ledger_facing_state_match": true,
            "safe_for_live_round_compression": true,
            "gate": "ledger-facing accounting-equivalent receipt submit/count replay; receipt is counted and bucket value is identical in both paths, while block-height provenance may differ because the unbatched path commits two ordered blocks and same-round batching commits one",
        });
        if !state_root_match {
            corpus_report["intended_state_root_difference"] = serde_json::json!(
                "unbatched replay commits receipt submit and receipt count as two ordered blocks while same-round batching commits one block; ledger-facing accounting state is identical after normalizing receipt and bucket block-height provenance"
            );
        }
        let corpus_file = root.join("vault-bridge-receipt-submit-count-equivalence.json");
        write_json_file(&corpus_file, &corpus_report).expect("write receipt corpus");
        let corpus_verify =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(corpus_file),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: true,
                required_candidate_classes: vec![
                    "vault_bridge_receipt_submit_count".to_string()
                ],
                strict_exit: true,
            })
            .expect("receipt corpus verifies");
        assert!(corpus_verify.passed, "{:?}", corpus_verify.failure_reasons);
        assert_eq!(1, corpus_verify.live_ready_case_count);
        assert_eq!(
            Some(true),
            corpus_verify.cases[0].ledger_facing_state_match
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_primary_mint_discovers_receipt_and_writes_ops() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-primary-mint-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let artifact_dir = root.join("artifacts");
        let issuer_key_file = root.join("issuer.key.json");
        let issuer_backup_file = root.join("issuer.backup.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");
        write_local_topology(TopologyOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            validators: 1,
            base_port: 44_140,
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
        })
        .expect("write local topology");

        let issuer_key = wallet_keygen(WalletKeygenOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            master_seed_hex: "17".repeat(32),
            account_index: 0,
            key_file: issuer_key_file.clone(),
            backup_file: issuer_backup_file,
            overwrite: true,
        })
        .expect("issuer keygen");
        let subscriber = "pf07381735ddb7de134e8be8402b465c9cd8ec7546".to_string();
        let genesis = Genesis::new(DEFAULT_CHAIN_ID);

        let settlement_asset = postfiat_types::AssetDefinition::new(
            DEFAULT_CHAIN_ID,
            issuer_key.address.clone(),
            "PFUSDC",
            1,
            6,
        )
        .expect("settlement asset");
        let mut nav_definition = postfiat_types::AssetDefinition::new(
            DEFAULT_CHAIN_ID,
            issuer_key.address.clone(),
            "a651",
            1,
            6,
        )
        .expect("nav asset");
        nav_definition.max_supply = Some(4_000_000_000);
        let mut evidence = vault_bridge_deposit_evidence_fixture();
        evidence.amount_atoms = 5_083_635;
        evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
        let source_domain = evidence.source_domain();
        let source_asset = evidence.source_asset_ref();
        let policy_hash = "24".repeat(48);
        let source_class = format!(
            "{}{}",
            postfiat_types::VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX,
            source_domain
        );
        let settlement_profile = postfiat_types::NavProofProfile::new(
            issuer_key.address.clone(),
            postfiat_types::NAV_PROFILE_VERIFIER_MULTI_FETCH,
            source_class,
            100,
            1,
            100,
            0,
            0,
            1,
            0,
            policy_hash.clone(),
            "",
            "",
            0,
            0,
        )
        .expect("settlement profile");
        let nav_profile = cli_test_sp1_nav_profile(&issuer_key.address);
        let mut settlement_nav_asset = postfiat_types::NavTrackedAsset::new(
            settlement_asset.asset_id.clone(),
            issuer_key.address.clone(),
            issuer_key.address.clone(),
            settlement_profile.profile_id.clone(),
            "USDC",
            issuer_key.address.clone(),
        )
        .expect("settlement nav asset");
        settlement_nav_asset.finalized_epoch = 1;
        settlement_nav_asset.nav_per_unit = postfiat_types::VAULT_BRIDGE_UNIT;
        settlement_nav_asset.circulating_supply = 5_083_635;
        settlement_nav_asset.finalized_reserve_packet_hash = "5b".repeat(48);
        let mut nav_asset = postfiat_types::NavTrackedAsset::new(
            nav_definition.asset_id.clone(),
            issuer_key.address.clone(),
            issuer_key.address.clone(),
            nav_profile.profile_id.clone(),
            "usd_1e8",
            issuer_key.address.clone(),
        )
        .expect("tracked nav asset");
        nav_asset.finalized_epoch = 3;
        nav_asset.nav_per_unit = 508_363_405;
        nav_asset.circulating_supply = 4_000;
        nav_asset.finalized_reserve_packet_hash = "ab".repeat(48);
        let nav_line = postfiat_types::TrustLine::new(
            subscriber.clone(),
            issuer_key.address.clone(),
            nav_definition.asset_id.clone(),
            10_000,
            10,
        )
        .expect("subscriber nav trustline");
        nav_line.validate().expect("valid subscriber nav trustline");

        let mut bucket = postfiat_types::VaultBridgeBucketState::new(
            settlement_asset.asset_id.clone(),
            source_domain.clone(),
            policy_hash.clone(),
            10,
        )
        .expect("bucket");
        bucket.gross_receipt_atoms = 5_083_635;
        bucket.counted_value_atoms = 5_083_635;
        bucket.last_packet_epoch = 1;
        bucket.validate().expect("valid bucket");
        let mut receipt = postfiat_types::VaultBridgeReceipt::new(
            &genesis.chain_id,
            settlement_asset.asset_id.clone(),
            source_domain,
            source_asset,
            postfiat_types::VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT,
            5_083_635,
            evidence.source_tx_or_attestation(),
            evidence.finality_ref(),
            evidence.vault_id(),
            policy_hash,
            9,
            1_000,
            Some(evidence),
        )
        .expect("receipt");
        receipt.status = postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED.to_string();
        receipt.finalized_at_height = 9;
        receipt.counted_at_height = 10;
        receipt.counted_value_atoms = 5_083_635;
        receipt
            .validate_for_chain(&genesis.chain_id)
            .expect("valid receipt");

        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer_key.address.clone(),
                25_000_000,
                Some(issuer_key.public_key_hex.clone()),
            ),
            Account::new(subscriber.clone(), 25_000_000, None),
        ]);
        ledger.asset_definitions.push(settlement_asset.clone());
        ledger.asset_definitions.push(nav_definition.clone());
        ledger.nav_proof_profiles.push(settlement_profile);
        ledger.nav_proof_profiles.push(nav_profile);
        ledger.nav_assets.push(settlement_nav_asset);
        ledger.nav_assets.push(nav_asset);
        ledger.trustlines.push(nav_line);
        ledger.vault_bridge_bucket_states.push(bucket);
        ledger.vault_bridge_receipts.push(receipt);
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write primary mint ledger");

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--primary-mint-only".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            data_dir.join(VALIDATOR_KEYS_FILE).display().to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--nav-asset".to_string(),
            nav_definition.asset_id.clone(),
            "--pfusdc".to_string(),
            settlement_asset.asset_id.clone(),
            "--subscriber".to_string(),
            subscriber,
            "--issuer-key-file".to_string(),
            issuer_key_file.display().to_string(),
            "--mint-amount".to_string(),
            "1".to_string(),
            "--prepare-only".to_string(),
        ])
        .expect("primary mint prepare-only cli");

        let report = serde_json::from_str::<NavRoundtripPrimaryMintReport>(
            &std::fs::read_to_string(artifact_dir.join("primary-mint.json"))
                .expect("read primary mint report"),
        )
        .expect("parse primary mint report");
        assert_eq!(NAV_ROUNDTRIP_PRIMARY_MINT_REPORT_SCHEMA, report.schema);
        assert_eq!(6, report.settlement_amount_atoms);
        assert_eq!(1, report.mint_amount);
        assert_eq!(2, report.certified_ops.operation_count);
        assert_eq!(
            1,
            report
                .certified_ops
                .dependency_report
                .same_round_dependency_count
        );
        assert!(report
            .certified_ops
            .dependency_report
            .same_round_batch_eligible);
        assert_eq!(
            vec!["nav_subscription_allocate_mint_at_nav".to_string()],
            report.certified_ops.dependency_report.candidate_batch_classes
        );
        assert!(report.certified_ops.prepare_only);
        assert_eq!(report.settlement_receipt_id, ledger.vault_bridge_receipts[0].receipt_id);
        assert!(artifact_dir
            .join("primary-mint-certified")
            .join("request.normalized.json")
            .exists());

        let mint_operation = serde_json::from_str::<postfiat_types::AssetTransactionOperation>(
            &std::fs::read_to_string(artifact_dir.join("nav-mint-at-nav.operation.json"))
                .expect("read mint operation"),
        )
        .expect("parse mint operation");
        let postfiat_types::AssetTransactionOperation::NavMintAtNav(mint_operation) =
            mint_operation
        else {
            panic!("expected nav mint operation");
        };
        assert_eq!(
            report.settlement_allocation_id,
            mint_operation.settlement_allocation_id
        );
        assert_eq!(6, mint_operation.settlement_amount_atoms);

        let replay_request =
            read_certified_asset_ops_request(std::path::Path::new(&report.operations_file))
                .expect("read primary mint replay request");
        validate_certified_asset_ops_request(&replay_request)
            .expect("primary mint replay request validates");
        assert_eq!(2, replay_request.operations.len());
        assert_eq!(
            vec!["nav_subscription_allocate_mint_at_nav".to_string()],
            certified_asset_ops_dependency_report(&replay_request).candidate_batch_classes
        );

        let replay_artifact_dir = root.join("primary-mint-replay-signing");
        let replay_options = CertifiedAssetOpsBatchOptions {
            data_dir: data_dir.clone(),
            topology_file: topology_file.clone(),
            key_file: data_dir.join(VALIDATOR_KEYS_FILE),
            proposal_key_file: None,
            ops_file: std::path::PathBuf::from(&report.operations_file),
            artifact_dir: replay_artifact_dir,
            max_transactions: None,
            require_local_proposer: true,
            require_signed_proposal: true,
            allow_peer_failures: false,
            quorum_early_full_propagation: false,
            local_apply_before_certified_send: true,
            defer_certified_sends: false,
            block_height: None,
            view: None,
            timeout_certificate_file: None,
            timeout_ms: 5_000,
            send_retries: 0,
            retry_backoff_ms: 250,
            allow_existing_mempool: false,
            resume: false,
            overwrite: false,
            prepare_only: false,
            batch_only: false,
        };
        let allocate_report = run_certified_asset_op_stage(
            &replay_request.operations[0],
            &replay_options,
            false,
            None,
        )
        .expect("sign replay allocation op");
        let mint_report = run_certified_asset_op_stage(
            &replay_request.operations[1],
            &replay_options,
            false,
            allocate_report.sequence.map(|sequence| sequence + 1),
        )
        .expect("sign replay mint op");
        let allocate_signed = allocate_report
            .signed_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .expect("allocation signed file");
        let mint_signed = mint_report
            .signed_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .expect("mint signed file");

        let unbatched_data_dir = root.join("primary-mint-unbatched-node");
        let batched_data_dir = root.join("primary-mint-batched-node");
        copy_test_dir_all(&data_dir, &unbatched_data_dir);
        copy_test_dir_all(&data_dir, &batched_data_dir);

        let unbatched_allocate_batch = root.join("primary-mint-unbatched-allocate.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_allocate_batch.clone(),
            signed_asset_transaction_files: vec![allocate_signed.clone()],
        })
        .expect("create unbatched allocation batch");
        let allocate_receipts = apply_batch(ApplyBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_allocate_batch,
            certificate_file: None,
        })
        .expect("apply unbatched allocation");
        assert!(allocate_receipts.iter().all(|receipt| receipt.accepted), "{allocate_receipts:?}");

        let unbatched_mint_batch = root.join("primary-mint-unbatched-mint.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_mint_batch.clone(),
            signed_asset_transaction_files: vec![mint_signed.clone()],
        })
        .expect("create unbatched mint batch");
        let mint_receipts = apply_batch(ApplyBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_mint_batch,
            certificate_file: None,
        })
        .expect("apply unbatched mint");
        assert!(mint_receipts.iter().all(|receipt| receipt.accepted), "{mint_receipts:?}");

        let batched_file = root.join("primary-mint-batched.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: batched_data_dir.clone(),
            batch_file: batched_file.clone(),
            signed_asset_transaction_files: vec![allocate_signed, mint_signed],
        })
        .expect("create primary mint same-round batch");
        let batched_receipts = apply_batch(ApplyBatchOptions {
            data_dir: batched_data_dir.clone(),
            batch_file: batched_file,
            certificate_file: None,
        })
        .expect("apply primary mint same-round batch");
        assert!(batched_receipts.iter().all(|receipt| receipt.accepted), "{batched_receipts:?}");

        let unbatched_status = status(NodeOptions {
            data_dir: unbatched_data_dir.clone(),
        })
        .expect("unbatched primary mint status");
        let batched_status = status(NodeOptions {
            data_dir: batched_data_dir.clone(),
        })
        .expect("batched primary mint status");
        let unbatched_ledger = postfiat_storage::NodeStore::new(&unbatched_data_dir)
            .read_ledger()
            .expect("read unbatched ledger");
        let batched_ledger = postfiat_storage::NodeStore::new(&batched_data_dir)
            .read_ledger()
            .expect("read batched ledger");
        assert!(unbatched_ledger
            .vault_bridge_allocations
            .iter()
            .all(|allocation| allocation.retired_at_height > 0));
        assert!(batched_ledger
            .vault_bridge_allocations
            .iter()
            .all(|allocation| allocation.retired_at_height > 0));
        let mut unbatched_ledger_facing_state = unbatched_ledger.clone();
        let mut batched_ledger_facing_state = batched_ledger.clone();
        for allocation in &mut unbatched_ledger_facing_state.vault_bridge_allocations {
            allocation.retired_at_height = 1;
        }
        for allocation in &mut batched_ledger_facing_state.vault_bridge_allocations {
            allocation.retired_at_height = 1;
        }
        assert_eq!(
            unbatched_ledger_facing_state, batched_ledger_facing_state,
            "primary mint batching must preserve ledger-facing accounting state; block-height provenance may differ"
        );
        assert_eq!(2, unbatched_status.block_height);
        assert_eq!(1, batched_status.block_height);

        let state_root_match = unbatched_status.state_root == batched_status.state_root;
        let mut corpus_report = serde_json::json!({
            "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
            "case": "nav-subscription-allocation-mint",
            "candidate_batch_class": "nav_subscription_allocate_mint_at_nav",
            "unbatched_block_height": unbatched_status.block_height,
            "batched_block_height": batched_status.block_height,
            "unbatched_state_root": unbatched_status.state_root,
            "batched_state_root": batched_status.state_root,
            "state_root_match": state_root_match,
            "ledger_facing_state_match": true,
            "safe_for_live_round_compression": true,
            "gate": "ledger-facing accounting-equivalent primary mint replay; allocation is retired in both paths, while block-height provenance may differ because the unbatched path commits two ordered blocks and same-round batching commits one",
        });
        if !state_root_match {
            corpus_report["intended_state_root_difference"] = serde_json::json!(
                "unbatched replay commits allocation and mint as two ordered blocks while same-round batching commits one block; ledger-facing accounting state is identical after normalizing retired allocation block-height provenance"
            );
        }
        let corpus_file = root.join("nav-subscription-allocation-mint-equivalence.json");
        write_json_file(&corpus_file, &corpus_report).expect("write primary mint corpus");
        let corpus_verify =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(corpus_file),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: true,
                required_candidate_classes: vec![
                    "nav_subscription_allocate_mint_at_nav".to_string()
                ],
                strict_exit: true,
            })
            .expect("primary mint corpus verifies");
        assert!(corpus_verify.passed, "{:?}", corpus_verify.failure_reasons);
        assert_eq!(1, corpus_verify.live_ready_case_count);
        assert_eq!(
            Some(true),
            corpus_verify.cases[0].ledger_facing_state_match
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_nav_checkpoint_writes_sp1_subscription_overlay_ops() {
        const FIXTURE_DIR: &str = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../execution/tests/fixtures/sp1-aggregate-regen-monero-crypto"
        );
        let public_values =
            std::fs::read(format!("{FIXTURE_DIR}/aggregate-public-values.bin")).expect("pv");
        let proof =
            std::fs::read(format!("{FIXTURE_DIR}/aggregate-proof-calldata.bin")).expect("proof");
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-nav-checkpoint-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let artifact_dir = root.join("artifacts");
        let issuer_key_file = root.join("issuer.key.json");
        let issuer_backup_file = root.join("issuer.backup.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");
        write_local_topology(TopologyOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            validators: 1,
            base_port: 44_260,
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
        })
        .expect("write local topology");
        let issuer_key = wallet_keygen(WalletKeygenOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            master_seed_hex: "32".repeat(32),
            account_index: 0,
            key_file: issuer_key_file.clone(),
            backup_file: issuer_backup_file,
            overwrite: true,
        })
        .expect("issuer keygen");
        let genesis = Genesis::new(DEFAULT_CHAIN_ID);
        let verified_net_assets = 2_364_869_341_670_u64;
        let circulating_supply = 4_000_u64;
        let nav_per_unit =
            postfiat_types::nav_per_unit_floor(verified_net_assets, circulating_supply)
                .expect("base nav");
        assert_eq!(591_217_335, nav_per_unit);
        let profile = postfiat_types::NavProofProfile::new(
            issuer_key.address.clone(),
            postfiat_types::NAV_PROFILE_VERIFIER_SP1_GROTH16,
            "stakehub-pol-v2",
            100_000,
            1,
            100_000,
            0,
            0,
            0,
            0,
            "8fcf3cd44c8180744563e85579ed91b7fd3882e560dc41ea4dc0c18cb01f289d",
            "0x004d1cd3f36e6ea60662af428edbea9d3aba45f04fe496da909d6bbe9fbf9258",
            "groth16",
            0,
            0,
        )
        .expect("sp1 profile");
        let settlement_asset = postfiat_types::AssetDefinition::new(
            DEFAULT_CHAIN_ID,
            issuer_key.address.clone(),
            "PFUSDC",
            1,
            6,
        )
        .expect("settlement asset");
        let mut nav_definition = postfiat_types::AssetDefinition::new(
            DEFAULT_CHAIN_ID,
            issuer_key.address.clone(),
            "a651",
            1,
            6,
        )
        .expect("nav asset");
        nav_definition.max_supply = Some(4_000_000_000);
        let mut settlement_nav_asset = postfiat_types::NavTrackedAsset::new(
            settlement_asset.asset_id.clone(),
            issuer_key.address.clone(),
            issuer_key.address.clone(),
            profile.profile_id.clone(),
            "USDC",
            issuer_key.address.clone(),
        )
        .expect("settlement nav asset");
        settlement_nav_asset.finalized_epoch = 1;
        settlement_nav_asset.nav_per_unit = postfiat_types::VAULT_BRIDGE_UNIT;
        settlement_nav_asset.circulating_supply = 5_082_364;
        settlement_nav_asset.finalized_reserve_packet_hash = "5d".repeat(48);
        let mut nav_asset = postfiat_types::NavTrackedAsset::new(
            nav_definition.asset_id.clone(),
            issuer_key.address.clone(),
            issuer_key.address.clone(),
            profile.profile_id.clone(),
            "usd_1e8",
            issuer_key.address.clone(),
        )
        .expect("tracked nav asset");
        nav_asset.finalized_epoch = 1;
        nav_asset.nav_per_unit = nav_per_unit;
        nav_asset.circulating_supply = circulating_supply;
        nav_asset.finalized_reserve_packet_hash = "04".repeat(48);

        let mut base_packet = postfiat_types::NavReservePacket::new(
            nav_definition.asset_id.clone(),
            issuer_key.address.clone(),
            issuer_key.address.clone(),
            1,
            nav_per_unit,
            circulating_supply,
            verified_net_assets,
            profile.profile_id.clone(),
            "01".repeat(48),
            "02".repeat(48),
            nav_asset.finalized_reserve_packet_hash.clone(),
        )
        .expect("base packet");
        base_packet.state = postfiat_types::NAV_RESERVE_STATE_FINALIZED.to_string();
        base_packet.submitted_at_height = 2;
        base_packet.sp1_proof_bytes = proof;
        base_packet.sp1_public_values = public_values;
        base_packet.validate().expect("valid base packet");

        let subscriber = "pf07381735ddb7de134e8be8402b465c9cd8ec7546".to_string();
        let mut nav_line = postfiat_types::TrustLine::new(
            subscriber.clone(),
            issuer_key.address.clone(),
            nav_definition.asset_id.clone(),
            10_000,
            10,
        )
        .expect("nav trustline");
        nav_line.balance = circulating_supply;

        let mut evidence = vault_bridge_deposit_evidence_fixture();
        evidence.amount_atoms = 5_082_364;
        evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
        let source_domain = evidence.source_domain();
        let policy_hash = "24".repeat(48);
        let mut bucket = postfiat_types::VaultBridgeBucketState::new(
            settlement_asset.asset_id.clone(),
            source_domain.clone(),
            policy_hash.clone(),
            10,
        )
        .expect("bucket");
        bucket.gross_receipt_atoms = 5_082_364;
        bucket.counted_value_atoms = 5_082_364;
        bucket.nav_subscription_allocations_atoms = 5_082_364;
        bucket.last_packet_epoch = 1;
        bucket.validate().expect("valid bucket");
        let mut receipt = postfiat_types::VaultBridgeReceipt::new(
            &genesis.chain_id,
            settlement_asset.asset_id.clone(),
            source_domain,
            evidence.source_asset_ref(),
            postfiat_types::VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT,
            5_082_364,
            evidence.source_tx_or_attestation(),
            evidence.finality_ref(),
            evidence.vault_id(),
            policy_hash,
            9,
            1_000,
            Some(evidence),
        )
        .expect("receipt");
        receipt.status = postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED.to_string();
        receipt.finalized_at_height = 9;
        receipt.counted_at_height = 10;
        receipt.counted_value_atoms = 5_082_364;
        receipt.allocated_value_atoms = 5_082_364;
        receipt
            .validate_for_chain(&genesis.chain_id)
            .expect("valid receipt");
        let mut allocation = postfiat_types::VaultBridgeAllocation::new(
            &genesis.chain_id,
            receipt.receipt_id.clone(),
            settlement_asset.asset_id.clone(),
            bucket.bucket_id.clone(),
            5_082_364,
            postfiat_types::VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,
            nav_roundtrip_nav_subscription_recipient_consumer_id(
                &nav_definition.asset_id,
                &subscriber,
            ),
            11,
        )
        .expect("allocation");
        allocation.retired_at_height = 12;
        allocation
            .validate_for_chain(&genesis.chain_id)
            .expect("valid allocation");

        let mut ledger = LedgerState::new(vec![Account::new(
            issuer_key.address.clone(),
            25_000_000,
            Some(issuer_key.public_key_hex.clone()),
        )]);
        ledger.asset_definitions.push(settlement_asset.clone());
        ledger.asset_definitions.push(nav_definition.clone());
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_assets.push(settlement_nav_asset);
        ledger.nav_assets.push(nav_asset);
        ledger.nav_reserve_packets.push(base_packet);
        ledger.trustlines.push(nav_line);
        ledger.vault_bridge_bucket_states.push(bucket);
        ledger.vault_bridge_receipts.push(receipt);
        ledger.vault_bridge_allocations.push(allocation);
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write checkpoint ledger");

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--nav-checkpoint-only".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            data_dir.join(VALIDATOR_KEYS_FILE).display().to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--nav-asset".to_string(),
            nav_definition.asset_id.clone(),
            "--issuer-key-file".to_string(),
            issuer_key_file.display().to_string(),
            "--expected-vna-delta".to_string(),
            "508236400".to_string(),
            "--prepare-only".to_string(),
        ])
        .expect("nav checkpoint prepare-only cli");

        let report = serde_json::from_str::<NavRoundtripNavCheckpointReport>(
            &std::fs::read_to_string(artifact_dir.join("nav-checkpoint.json"))
                .expect("read checkpoint report"),
        )
        .expect("parse checkpoint report");
        assert_eq!(NAV_ROUNDTRIP_NAV_CHECKPOINT_REPORT_SCHEMA, report.schema);
        assert_eq!(1, report.epoch_before);
        assert_eq!(2, report.checkpoint_epoch);
        assert_eq!(Some(508_236_400), report.overlay_value_nav_units);
        assert_eq!(Some(verified_net_assets), report.sp1_base_verified_net_assets);
        assert_eq!(2_365_377_578_070, report.verified_net_assets);
        assert_eq!(591_344_394, report.nav_per_unit);
        assert_eq!(4_000_000_000, report.circulating_supply);
        assert!(report.verified_net_assets_after.is_none());
        assert!(report.submit_certified_ops.prepare_only);
        assert!(report.finalize_certified_ops.prepare_only);
        assert_eq!(1, report.submit_certified_ops.operation_count);
        assert_eq!(1, report.finalize_certified_ops.operation_count);
        assert_eq!(
            1,
            report
                .finalize_certified_ops
                .dependency_report
                .prior_round_dependency_count
        );
        assert!(!report
            .finalize_certified_ops
            .dependency_report
            .same_round_batch_eligible);

        let submit_operation = serde_json::from_str::<postfiat_types::AssetTransactionOperation>(
            &std::fs::read_to_string(&report.submit_operation_file)
                .expect("read submit operation"),
        )
        .expect("parse submit operation");
        let postfiat_types::AssetTransactionOperation::NavReserveSubmit(submit_operation) =
            submit_operation
        else {
            panic!("expected nav reserve submit operation");
        };
        assert_eq!(2, submit_operation.epoch);
        assert_eq!(2_365_377_578_070, submit_operation.verified_net_assets);
        assert_eq!(591_344_394, submit_operation.nav_per_unit);
        assert_eq!(4_000_000_000, submit_operation.circulating_supply);
        assert_eq!(report.source_root, submit_operation.source_root);
        assert_eq!(report.reserve_packet_hash, submit_operation.reserve_packet_hash);
        assert_eq!(356, submit_operation.sp1_proof_bytes.len());
        assert_eq!(2720, submit_operation.sp1_public_values.len());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_vault_bridge_checkpoint_includes_reserve_account() {
        let issuer = "pff3e396f771a8f490ca330e1720472d473bcfcb6d".to_string();
        let holder = "pf07381735ddb7de134e8be8402b465c9cd8ec7546".to_string();
        let vault = "0x1a15e6103d6af4e88924f748e13b829d3948dea9";
        let usdc = "0xaf88d065e77c8cc2239327c5edb3a432268e5831";
        let source_domain = format!("erc20_bridge_vault:42161:{vault}:{usdc}");
        let source_class = format!("vault_bridge:{source_domain}");
        let policy_hash = "24".repeat(48);
        let asset = postfiat_types::AssetDefinition::new(
            DEFAULT_CHAIN_ID,
            issuer.clone(),
            "PFUSDC",
            2,
            6,
        )
        .expect("asset");
        let profile = postfiat_types::NavProofProfile::new(
            issuer.clone(),
            postfiat_types::NAV_PROFILE_VERIFIER_MULTI_FETCH,
            source_class,
            100,
            1,
            100,
            0,
            0,
            1,
            0,
            policy_hash.clone(),
            "",
            "",
            0,
            0,
        )
        .expect("profile");
        let mut nav_asset = postfiat_types::NavTrackedAsset::new(
            asset.asset_id.clone(),
            issuer.clone(),
            issuer.clone(),
            profile.profile_id.clone(),
            "USDC",
            issuer.clone(),
        )
        .expect("nav asset");
        nav_asset.finalized_epoch = 1;
        nav_asset.nav_per_unit = postfiat_types::VAULT_BRIDGE_UNIT;
        nav_asset.circulating_supply = 2_000_000;
        nav_asset.finalized_reserve_packet_hash = "5b".repeat(48);

        let mut trustline =
            postfiat_types::TrustLine::new(holder, issuer.clone(), asset.asset_id.clone(), 5_000_000, 10)
                .expect("trustline");
        trustline.balance = 2_000_000;
        trustline.validate().expect("valid trustline");

        let mut bucket = postfiat_types::VaultBridgeBucketState::new(
            asset.asset_id.clone(),
            source_domain,
            policy_hash,
            9,
        )
        .expect("bucket");
        bucket.gross_receipt_atoms = 2_000_000;
        bucket.counted_value_atoms = 2_000_000;
        bucket.outstanding_vault_bridge_atoms = 2_000_000;
        bucket.last_packet_epoch = 1;
        bucket.validate().expect("valid bucket");

        let mut ledger = LedgerState::new(vec![Account::new(issuer, 25_000_000, None)]);
        ledger.asset_definitions.push(asset);
        ledger.nav_assets.push(nav_asset.clone());
        ledger.nav_proof_profiles.push(profile.clone());
        ledger.trustlines.push(trustline);
        ledger.vault_bridge_bucket_states.push(bucket);

        let checkpoint = nav_roundtrip_build_nav_checkpoint_fields(
            &ledger,
            &nav_asset,
            None,
            Some(&profile),
            2,
            None,
            None,
        )
        .expect("checkpoint fields");

        assert_eq!(2_000_000, checkpoint.circulating_supply);
        assert_eq!(2_000_000, checkpoint.verified_net_assets);
        assert_eq!(
            vec![format!("evm:42161:{vault}:{usdc}")],
            checkpoint.reserve_accounts
        );
    }

    #[test]
    fn nav_roundtrip_nav_exit_writes_redeem_from_primary_report() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-nav-exit-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let primary_artifact_dir = root.join("primary");
        let exit_artifact_dir = root.join("exit");
        let issuer_key_file = root.join("issuer.key.json");
        let issuer_backup_file = root.join("issuer.backup.json");
        let owner_key_file = root.join("owner.key.json");
        let owner_backup_file = root.join("owner.backup.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");
        write_local_topology(TopologyOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            validators: 1,
            base_port: 44_150,
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
        })
        .expect("write local topology");

        let issuer_key = wallet_keygen(WalletKeygenOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            master_seed_hex: "18".repeat(32),
            account_index: 0,
            key_file: issuer_key_file.clone(),
            backup_file: issuer_backup_file,
            overwrite: true,
        })
        .expect("issuer keygen");
        let owner_key = wallet_keygen(WalletKeygenOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            master_seed_hex: "19".repeat(32),
            account_index: 0,
            key_file: owner_key_file.clone(),
            backup_file: owner_backup_file,
            overwrite: true,
        })
        .expect("owner keygen");
        let genesis = Genesis::new(DEFAULT_CHAIN_ID);
        let profile = cli_test_sp1_nav_profile(&issuer_key.address);

        let settlement_asset = postfiat_types::AssetDefinition::new(
            DEFAULT_CHAIN_ID,
            issuer_key.address.clone(),
            "PFUSDC",
            1,
            6,
        )
        .expect("settlement asset");
        let nav_definition = postfiat_types::AssetDefinition::new(
            DEFAULT_CHAIN_ID,
            issuer_key.address.clone(),
            "a651",
            1,
            6,
        )
        .expect("nav asset");
        let mut settlement_nav_asset = postfiat_types::NavTrackedAsset::new(
            settlement_asset.asset_id.clone(),
            issuer_key.address.clone(),
            issuer_key.address.clone(),
            profile.profile_id.clone(),
            "USDC",
            issuer_key.address.clone(),
        )
        .expect("settlement nav asset");
        settlement_nav_asset.finalized_epoch = 1;
        settlement_nav_asset.nav_per_unit = postfiat_types::VAULT_BRIDGE_UNIT;
        settlement_nav_asset.circulating_supply = 5_083_635;
        settlement_nav_asset.finalized_reserve_packet_hash = "5c".repeat(48);
        let mut nav_asset = postfiat_types::NavTrackedAsset::new(
            nav_definition.asset_id.clone(),
            issuer_key.address.clone(),
            issuer_key.address.clone(),
            profile.profile_id.clone(),
            "usd_1e8",
            issuer_key.address.clone(),
        )
        .expect("tracked nav asset");
        nav_asset.finalized_epoch = 3;
        nav_asset.nav_per_unit = 508_236_346;
        nav_asset.circulating_supply = 4_001;
        nav_asset.finalized_reserve_packet_hash = "ac".repeat(48);

        let mut evidence = vault_bridge_deposit_evidence_fixture();
        evidence.amount_atoms = 5_083_635;
        evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
        let source_domain = evidence.source_domain();
        let source_asset = evidence.source_asset_ref();
        let policy_hash = "25".repeat(48);
        let mut bucket = postfiat_types::VaultBridgeBucketState::new(
            settlement_asset.asset_id.clone(),
            source_domain.clone(),
            policy_hash.clone(),
            10,
        )
        .expect("bucket");
        bucket.gross_receipt_atoms = 5_083_635;
        bucket.counted_value_atoms = 5_083_635;
        bucket.last_packet_epoch = 1;
        bucket.validate().expect("valid bucket");
        let mut receipt = postfiat_types::VaultBridgeReceipt::new(
            &genesis.chain_id,
            settlement_asset.asset_id.clone(),
            source_domain,
            source_asset,
            postfiat_types::VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT,
            5_083_635,
            evidence.source_tx_or_attestation(),
            evidence.finality_ref(),
            evidence.vault_id(),
            policy_hash,
            9,
            1_000,
            Some(evidence),
        )
        .expect("receipt");
        receipt.status = postfiat_types::VAULT_BRIDGE_RECEIPT_STATUS_COUNTED.to_string();
        receipt.finalized_at_height = 9;
        receipt.counted_at_height = 10;
        receipt.counted_value_atoms = 5_083_635;
        receipt
            .validate_for_chain(&genesis.chain_id)
            .expect("valid receipt");

        let mut nav_line = postfiat_types::TrustLine::new(
            owner_key.address.clone(),
            issuer_key.address.clone(),
            nav_definition.asset_id.clone(),
            100,
            10,
        )
        .expect("nav trustline");
        nav_line.balance = 1;
        let settlement_line = postfiat_types::TrustLine::new(
            owner_key.address.clone(),
            issuer_key.address.clone(),
            settlement_asset.asset_id.clone(),
            10_000_000,
            10,
        )
        .expect("settlement trustline");

        let mut ledger = LedgerState::new(vec![
            Account::new(
                issuer_key.address.clone(),
                25_000_000,
                Some(issuer_key.public_key_hex.clone()),
            ),
            Account::new(
                owner_key.address.clone(),
                25_000_000,
                Some(owner_key.public_key_hex.clone()),
            ),
        ]);
        ledger.asset_definitions.push(settlement_asset.clone());
        ledger.asset_definitions.push(nav_definition.clone());
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_assets.push(settlement_nav_asset);
        ledger.nav_assets.push(nav_asset);
        ledger.trustlines.push(nav_line);
        ledger.trustlines.push(settlement_line);
        ledger.vault_bridge_bucket_states.push(bucket);
        ledger.vault_bridge_receipts.push(receipt);
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write nav exit ledger");

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--primary-mint-only".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            data_dir.join(VALIDATOR_KEYS_FILE).display().to_string(),
            "--artifact-dir".to_string(),
            primary_artifact_dir.display().to_string(),
            "--nav-asset".to_string(),
            nav_definition.asset_id.clone(),
            "--pfusdc".to_string(),
            settlement_asset.asset_id.clone(),
            "--subscriber".to_string(),
            owner_key.address.clone(),
            "--issuer-key-file".to_string(),
            issuer_key_file.display().to_string(),
            "--mint-amount".to_string(),
            "1".to_string(),
            "--prepare-only".to_string(),
        ])
        .expect("primary mint prepare-only cli");
        let primary_report = serde_json::from_str::<NavRoundtripPrimaryMintReport>(
            &std::fs::read_to_string(primary_artifact_dir.join("primary-mint.json"))
                .expect("read primary report"),
        )
        .expect("parse primary report");
        assert_eq!(6, primary_report.settlement_amount_atoms);

        let mut ledger = postfiat_storage::NodeStore::new(&data_dir)
            .read_ledger()
            .expect("read ledger before NAV update");
        let nav_asset = ledger
            .nav_assets
            .iter_mut()
            .find(|asset| asset.asset_id == nav_definition.asset_id)
            .expect("nav asset before NAV update");
        nav_asset.finalized_epoch = 4;
        nav_asset.nav_per_unit = 508_363_405;
        nav_asset.finalized_reserve_packet_hash = "ad".repeat(48);
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write updated NAV ledger");

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--nav-exit-only".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            data_dir.join(VALIDATOR_KEYS_FILE).display().to_string(),
            "--artifact-dir".to_string(),
            exit_artifact_dir.display().to_string(),
            "--primary-mint-report".to_string(),
            primary_artifact_dir.join("primary-mint.json").display().to_string(),
            "--nav-asset".to_string(),
            nav_definition.asset_id.clone(),
            "--pfusdc".to_string(),
            settlement_asset.asset_id.clone(),
            "--owner-key-file".to_string(),
            owner_key_file.display().to_string(),
            "--issuer-key-file".to_string(),
            issuer_key_file.display().to_string(),
            "--prepare-only".to_string(),
        ])
        .expect("nav exit prepare-only cli");

        let report = serde_json::from_str::<NavRoundtripNavExitReport>(
            &std::fs::read_to_string(exit_artifact_dir.join("nav-exit.json"))
                .expect("read nav exit report"),
        )
        .expect("parse nav exit report");
        assert_eq!(NAV_ROUNDTRIP_NAV_EXIT_REPORT_SCHEMA, report.schema);
        assert_eq!(1, report.redeem_amount);
        assert_eq!(6, report.settlement_amount_atoms);
        assert_eq!(Some(1), report.nav_balance_before);
        assert_eq!(Some(0), report.settlement_balance_before);
        assert!(report.settle_certified_ops.is_none());
        assert!(report.redemption_id.is_none());
        assert!(exit_artifact_dir
            .join("nav-exit-redeem-certified")
            .join("request.normalized.json")
            .exists());

        let redeem_operation = serde_json::from_str::<postfiat_types::AssetTransactionOperation>(
            &std::fs::read_to_string(exit_artifact_dir.join("nav-redeem-at-nav.operation.json"))
                .expect("read redeem operation"),
        )
        .expect("parse redeem operation");
        let postfiat_types::AssetTransactionOperation::NavRedeemAtNav(redeem_operation) =
            redeem_operation
        else {
            panic!("expected nav redeem operation");
        };
        assert_eq!(owner_key.address, redeem_operation.owner);
        assert_eq!(issuer_key.address, redeem_operation.issuer);
        assert_eq!(nav_definition.asset_id, redeem_operation.asset_id);
        assert_eq!(1, redeem_operation.amount);

        let replay_base_data_dir = root.join("nav-exit-replay-base-node");
        copy_test_dir_all(&data_dir, &replay_base_data_dir);
        let replay_store = postfiat_storage::NodeStore::new(&replay_base_data_dir);
        let mut replay_base_ledger = replay_store
            .read_ledger()
            .expect("read NAV exit replay base ledger");
        let source_domain = replay_base_ledger.vault_bridge_receipts[0]
            .source_domain
            .clone();
        let source_class = format!(
            "{}{}",
            postfiat_types::VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX,
            source_domain
        );
        let settlement_profile = postfiat_types::NavProofProfile::new(
            issuer_key.address.clone(),
            NAV_PROFILE_VERIFIER_MULTI_FETCH,
            source_class,
            100,
            1,
            100,
            0,
            0,
            1,
            0,
            "25".repeat(48),
            "",
            "",
            0,
            0,
        )
        .expect("settlement profile for NAV exit replay");
        replay_base_ledger
            .nav_assets
            .iter_mut()
            .find(|asset| asset.asset_id == settlement_asset.asset_id)
            .expect("settlement NAV asset for replay")
            .proof_profile = settlement_profile.profile_id.clone();
        replay_base_ledger.nav_proof_profiles.push(settlement_profile);
        let settlement_receipt_id = replay_base_ledger.vault_bridge_receipts[0].receipt_id.clone();
        let mut settlement_allocation = postfiat_types::VaultBridgeAllocation::new(
            &genesis.chain_id,
            settlement_receipt_id.clone(),
            settlement_asset.asset_id.clone(),
            primary_report.settlement_bucket_id.clone(),
            primary_report.settlement_amount_atoms,
            postfiat_types::VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,
            nav_roundtrip_nav_subscription_consumer_id(&nav_definition.asset_id),
            11,
        )
        .expect("seed NAV subscription allocation");
        assert_eq!(
            primary_report.settlement_allocation_id,
            settlement_allocation.allocation_id
        );
        settlement_allocation.retired_at_height = 12;
        settlement_allocation
            .validate()
            .expect("valid retired settlement allocation");
        replay_base_ledger
            .vault_bridge_allocations
            .push(settlement_allocation);
        let replay_receipt = replay_base_ledger
            .vault_bridge_receipts
            .iter_mut()
            .find(|receipt| receipt.receipt_id == settlement_receipt_id)
            .expect("settlement receipt for replay");
        replay_receipt.allocated_value_atoms = replay_receipt
            .allocated_value_atoms
            .checked_add(primary_report.settlement_amount_atoms)
            .expect("receipt allocation fits");
        replay_receipt.validate().expect("valid replay receipt");
        let replay_bucket = replay_base_ledger
            .vault_bridge_bucket_states
            .iter_mut()
            .find(|bucket| bucket.bucket_id == primary_report.settlement_bucket_id)
            .expect("settlement bucket for replay");
        replay_bucket.nav_subscription_allocations_atoms = replay_bucket
            .nav_subscription_allocations_atoms
            .checked_add(primary_report.settlement_amount_atoms)
            .expect("bucket nav subscription allocation fits");
        replay_bucket.validate().expect("valid replay bucket");
        replay_store
            .write_ledger(&replay_base_ledger)
            .expect("write seeded NAV exit replay ledger");

        let redeem_replay_request =
            read_certified_asset_ops_request(std::path::Path::new(&report.redeem_operations_file))
                .expect("read NAV exit redeem replay request");
        validate_certified_asset_ops_request(&redeem_replay_request)
            .expect("NAV exit redeem replay request validates");
        let redeem_op = redeem_replay_request.operations[0].clone();
        let replay_signing_artifact_dir = root.join("nav-exit-replay-signing");
        let replay_options = CertifiedAssetOpsBatchOptions {
            data_dir: replay_base_data_dir.clone(),
            topology_file: topology_file.clone(),
            key_file: data_dir.join(VALIDATOR_KEYS_FILE),
            proposal_key_file: None,
            ops_file: std::path::PathBuf::from(&report.redeem_operations_file),
            artifact_dir: replay_signing_artifact_dir,
            max_transactions: None,
            require_local_proposer: true,
            require_signed_proposal: true,
            allow_peer_failures: false,
            quorum_early_full_propagation: false,
            local_apply_before_certified_send: true,
            defer_certified_sends: false,
            block_height: None,
            view: None,
            timeout_certificate_file: None,
            timeout_ms: 5_000,
            send_retries: 0,
            retry_backoff_ms: 250,
            allow_existing_mempool: false,
            resume: false,
            overwrite: false,
            prepare_only: false,
            batch_only: false,
        };
        let redeem_report =
            run_certified_asset_op_stage(&redeem_op, &replay_options, false, None)
                .expect("sign replay NAV redeem op");
        let redemption_id = postfiat_types::nav_redemption_id(
            &genesis.chain_id,
            &owner_key.address,
            &nav_definition.asset_id,
            redeem_report.sequence.expect("redeem sequence"),
        )
        .expect("derive replay NAV redemption id");
        let settlement_receipt_hash = nav_roundtrip_nav_exit_settlement_receipt_hash(
            &genesis.chain_id,
            &nav_definition.asset_id,
            &settlement_asset.asset_id,
            &redemption_id,
            &primary_report.settlement_allocation_id,
            report.settlement_amount_atoms,
        );
        let settle_operation = postfiat_types::AssetTransactionOperation::NavRedeemSettle(
            postfiat_types::NavRedeemSettleOperation {
                issuer: issuer_key.address.clone(),
                asset_id: nav_definition.asset_id.clone(),
                redemption_id: redemption_id.clone(),
                settlement_receipt_hash: settlement_receipt_hash.clone(),
                settlement_asset_id: settlement_asset.asset_id.clone(),
                settlement_bucket_id: primary_report.settlement_bucket_id.clone(),
                settlement_allocation_id: primary_report.settlement_allocation_id.clone(),
                settlement_amount_atoms: report.settlement_amount_atoms,
            },
        );
        settle_operation
            .validate()
            .expect("valid replay NAV redeem settle op");
        let settle_op = CertifiedAssetOpRequest {
            label: "nav-redeem-settle".to_string(),
            source: issuer_key.address.clone(),
            key_file: issuer_key_file.clone(),
            operation: settle_operation,
            dependencies: vec![CertifiedAssetOpDependency {
                label: "nav-redeem-at-nav".to_string(),
                mode: "same_round".to_string(),
                reason: Some(
                    "settlement consumes a redemption id deterministically derived from the signed redeem sequence"
                        .to_string(),
                ),
            }],
        };
        let replay_request = CertifiedAssetOpsRequest {
            schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            operations: vec![redeem_op.clone(), settle_op.clone()],
        };
        validate_certified_asset_ops_request(&replay_request)
            .expect("NAV redeem/settle replay request validates");
        assert_eq!(
            vec!["nav_redeem_at_nav_settle".to_string()],
            certified_asset_ops_dependency_report(&replay_request).candidate_batch_classes
        );
        let settle_report =
            run_certified_asset_op_stage(&settle_op, &replay_options, false, None)
                .expect("sign replay NAV redeem settle op");
        let redeem_signed = redeem_report
            .signed_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .expect("redeem signed file");
        let settle_signed = settle_report
            .signed_file
            .as_ref()
            .map(std::path::PathBuf::from)
            .expect("settle signed file");

        let unbatched_data_dir = root.join("nav-exit-unbatched-node");
        let batched_data_dir = root.join("nav-exit-batched-node");
        copy_test_dir_all(&replay_base_data_dir, &unbatched_data_dir);
        copy_test_dir_all(&replay_base_data_dir, &batched_data_dir);

        let unbatched_redeem_batch = root.join("nav-exit-unbatched-redeem.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_redeem_batch.clone(),
            signed_asset_transaction_files: vec![redeem_signed.clone()],
        })
        .expect("create unbatched NAV redeem batch");
        let redeem_receipts = apply_batch(ApplyBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_redeem_batch,
            certificate_file: None,
        })
        .expect("apply unbatched NAV redeem");
        assert!(
            redeem_receipts.iter().all(|receipt| receipt.accepted),
            "{redeem_receipts:?}"
        );

        let unbatched_settle_batch = root.join("nav-exit-unbatched-settle.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_settle_batch.clone(),
            signed_asset_transaction_files: vec![settle_signed.clone()],
        })
        .expect("create unbatched NAV settle batch");
        let settle_receipts = apply_batch(ApplyBatchOptions {
            data_dir: unbatched_data_dir.clone(),
            batch_file: unbatched_settle_batch,
            certificate_file: None,
        })
        .expect("apply unbatched NAV settle");
        assert!(
            settle_receipts.iter().all(|receipt| receipt.accepted),
            "{settle_receipts:?}"
        );

        let batched_file = root.join("nav-exit-batched.json");
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: batched_data_dir.clone(),
            batch_file: batched_file.clone(),
            signed_asset_transaction_files: vec![redeem_signed, settle_signed],
        })
        .expect("create NAV redeem/settle same-round batch");
        let batched_receipts = apply_batch(ApplyBatchOptions {
            data_dir: batched_data_dir.clone(),
            batch_file: batched_file,
            certificate_file: None,
        })
        .expect("apply NAV redeem/settle same-round batch");
        assert!(
            batched_receipts.iter().all(|receipt| receipt.accepted),
            "{batched_receipts:?}"
        );

        let unbatched_status = status(NodeOptions {
            data_dir: unbatched_data_dir.clone(),
        })
        .expect("unbatched NAV exit status");
        let batched_status = status(NodeOptions {
            data_dir: batched_data_dir.clone(),
        })
        .expect("batched NAV exit status");
        let unbatched_ledger = postfiat_storage::NodeStore::new(&unbatched_data_dir)
            .read_ledger()
            .expect("read unbatched NAV exit ledger");
        let batched_ledger = postfiat_storage::NodeStore::new(&batched_data_dir)
            .read_ledger()
            .expect("read batched NAV exit ledger");
        let unbatched_redemption = unbatched_ledger
            .nav_redemption(&redemption_id)
            .expect("unbatched redemption");
        let batched_redemption = batched_ledger
            .nav_redemption(&redemption_id)
            .expect("batched redemption");
        assert_eq!(postfiat_types::NAV_REDEMPTION_STATE_SETTLED, unbatched_redemption.state);
        assert_eq!(postfiat_types::NAV_REDEMPTION_STATE_SETTLED, batched_redemption.state);
        assert_eq!(
            settlement_receipt_hash,
            unbatched_redemption.settlement_receipt_hash
        );
        assert_eq!(
            settlement_receipt_hash,
            batched_redemption.settlement_receipt_hash
        );
        assert_eq!(
            Some(0),
            nav_roundtrip_trustline_balance(
                &unbatched_ledger,
                &owner_key.address,
                &nav_definition.asset_id
            )
        );
        assert_eq!(
            Some(report.settlement_amount_atoms),
            nav_roundtrip_trustline_balance(
                &unbatched_ledger,
                &owner_key.address,
                &settlement_asset.asset_id
            )
        );
        let mut unbatched_ledger_facing_state = unbatched_ledger.clone();
        let mut batched_ledger_facing_state = batched_ledger.clone();
        for bucket in &mut unbatched_ledger_facing_state.vault_bridge_bucket_states {
            if bucket.bucket_id == primary_report.settlement_bucket_id {
                bucket.last_updated_height = 1;
            }
        }
        for bucket in &mut batched_ledger_facing_state.vault_bridge_bucket_states {
            if bucket.bucket_id == primary_report.settlement_bucket_id {
                bucket.last_updated_height = 1;
            }
        }
        for allocation in &mut unbatched_ledger_facing_state.vault_bridge_allocations {
            if allocation.consumer_id == format!("nav_redemption:{redemption_id}") {
                allocation.created_at_height = 1;
            }
        }
        for allocation in &mut batched_ledger_facing_state.vault_bridge_allocations {
            if allocation.consumer_id == format!("nav_redemption:{redemption_id}") {
                allocation.created_at_height = 1;
            }
        }
        assert_eq!(
            unbatched_ledger_facing_state, batched_ledger_facing_state,
            "NAV redeem/settle batching must preserve ledger-facing accounting state; block-height provenance may differ"
        );
        assert_eq!(2, unbatched_status.block_height);
        assert_eq!(1, batched_status.block_height);

        let state_root_match = unbatched_status.state_root == batched_status.state_root;
        let mut corpus_report = serde_json::json!({
            "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
            "case": "nav-redeem-at-nav-settle",
            "candidate_batch_class": "nav_redeem_at_nav_settle",
            "unbatched_block_height": unbatched_status.block_height,
            "batched_block_height": batched_status.block_height,
            "unbatched_state_root": unbatched_status.state_root,
            "batched_state_root": batched_status.state_root,
            "state_root_match": state_root_match,
            "ledger_facing_state_match": true,
            "safe_for_live_round_compression": true,
            "gate": "ledger-facing accounting-equivalent NAV redeem/settle replay; the redemption id is derived from the signed redeem sequence before apply, while block-height provenance may differ because the unbatched path commits two ordered blocks and same-round batching commits one",
        });
        if !state_root_match {
            corpus_report["intended_state_root_difference"] = serde_json::json!(
                "unbatched replay commits NAV redeem and settle as two ordered blocks while same-round batching commits one block; ledger-facing accounting state is identical after normalizing settlement bucket and redemption top-up allocation block-height provenance"
            );
        }
        let corpus_file = root.join("nav-redeem-at-nav-settle-equivalence.json");
        write_json_file(&corpus_file, &corpus_report).expect("write NAV redeem/settle corpus");
        let corpus_verify =
            nav_roundtrip_replay_corpus_verify(NavRoundtripReplayCorpusVerifyOptions {
                corpus_file: Some(corpus_file),
                corpus_dir: None,
                report_file: None,
                require_live_compression_ready: true,
                required_candidate_classes: vec!["nav_redeem_at_nav_settle".to_string()],
                strict_exit: true,
            })
            .expect("NAV redeem/settle corpus verifies");
        assert!(corpus_verify.passed, "{:?}", corpus_verify.failure_reasons);
        assert_eq!(1, corpus_verify.live_ready_case_count);
        assert_eq!(
            Some(true),
            corpus_verify.cases[0].ledger_facing_state_match
        );

        let same_round_cli_data_dir = root.join("nav-exit-same-round-cli-node");
        let same_round_cli_artifact_dir = root.join("exit-same-round-cli");
        copy_test_dir_all(&replay_base_data_dir, &same_round_cli_data_dir);
        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--nav-exit-only".to_string(),
            "--same-round-nav-exit".to_string(),
            "--batch-only".to_string(),
            "--data-dir".to_string(),
            same_round_cli_data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            same_round_cli_data_dir
                .join(VALIDATOR_KEYS_FILE)
                .display()
                .to_string(),
            "--artifact-dir".to_string(),
            same_round_cli_artifact_dir.display().to_string(),
            "--primary-mint-report".to_string(),
            primary_artifact_dir.join("primary-mint.json").display().to_string(),
            "--nav-asset".to_string(),
            nav_definition.asset_id.clone(),
            "--pfusdc".to_string(),
            settlement_asset.asset_id.clone(),
            "--owner-key-file".to_string(),
            owner_key_file.display().to_string(),
            "--issuer-key-file".to_string(),
            issuer_key_file.display().to_string(),
        ])
        .expect("same-round NAV exit batch-only cli");
        let same_round_cli_report = serde_json::from_str::<NavRoundtripNavExitReport>(
            &std::fs::read_to_string(same_round_cli_artifact_dir.join("nav-exit.json"))
                .expect("read same-round nav exit report"),
        )
        .expect("parse same-round nav exit report");
        assert!(same_round_cli_report.same_round_settlement);
        assert!(same_round_cli_report.redemption_id.is_some());
        assert!(same_round_cli_report.settlement_receipt_hash.is_some());
        assert_eq!(2, same_round_cli_report.redeem_certified_ops.operation_count);
        assert!(same_round_cli_report.redeem_certified_ops.batch_only);
        assert_eq!(
            vec!["nav_redeem_at_nav_settle".to_string()],
            same_round_cli_report
                .redeem_certified_ops
                .dependency_report
                .candidate_batch_classes
        );
        assert_eq!(
            same_round_cli_report
                .redeem_certified_ops
                .artifact_dir,
            same_round_cli_report
                .settle_certified_ops
                .as_ref()
                .expect("same-round settle certified ops")
                .artifact_dir
        );
        assert!(same_round_cli_artifact_dir
            .join("nav-exit-redeem-settle-certified")
            .join("mempool-batch.json")
            .exists());

        let mut ledger = postfiat_storage::NodeStore::new(&data_dir)
            .read_ledger()
            .expect("read ledger before burn");
        let owner_settlement_line = ledger
            .trustlines
            .iter_mut()
            .find(|line| {
                line.account == owner_key.address && line.asset_id == settlement_asset.asset_id
            })
            .expect("owner settlement line");
        owner_settlement_line.balance = 5_083_635;
        let settlement_bucket = ledger
            .vault_bridge_bucket_states
            .iter_mut()
            .find(|bucket| bucket.asset_id == settlement_asset.asset_id)
            .expect("settlement bucket");
        settlement_bucket.outstanding_vault_bridge_atoms = 5_083_635;
        settlement_bucket.validate().expect("valid burn-ready bucket");
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write burn-ready ledger");

        let burn_artifact_dir = root.join("burn");
        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--burn-to-redeem-only".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            data_dir.join(VALIDATOR_KEYS_FILE).display().to_string(),
            "--artifact-dir".to_string(),
            burn_artifact_dir.display().to_string(),
            "--nav-exit-report".to_string(),
            exit_artifact_dir.join("nav-exit.json").display().to_string(),
            "--pfusdc".to_string(),
            settlement_asset.asset_id.clone(),
            "--owner-key-file".to_string(),
            owner_key_file.display().to_string(),
            "--destination-ref".to_string(),
            "evm-erc20:42161:0x5555555555555555555555555555555555555555".to_string(),
            "--prepare-only".to_string(),
        ])
        .expect("burn-to-redeem prepare-only cli");

        let burn_report = serde_json::from_str::<NavRoundtripBurnToRedeemReport>(
            &std::fs::read_to_string(burn_artifact_dir.join("burn-to-redeem.json"))
                .expect("read burn report"),
        )
        .expect("parse burn report");
        assert_eq!(
            NAV_ROUNDTRIP_BURN_TO_REDEEM_REPORT_SCHEMA,
            burn_report.schema
        );
        assert_eq!(owner_key.address, burn_report.owner);
        assert_eq!(6, burn_report.amount_atoms);
        assert_eq!(Some(5_083_635), burn_report.owner_balance_before);
        assert!(burn_report.redemption_id.is_none());
        assert_eq!(1, burn_report.certified_ops.operation_count);
        assert!(burn_report.certified_ops.prepare_only);
        assert!(burn_artifact_dir
            .join("burn-to-redeem-bundle")
            .join("burn-to-redeem.operation.json")
            .exists());
        assert!(burn_artifact_dir
            .join("burn-to-redeem-certified")
            .join("request.normalized.json")
            .exists());

        let _ = std::fs::remove_dir_all(root);
    }
