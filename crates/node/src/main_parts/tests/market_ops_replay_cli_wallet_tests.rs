    #[test]
    fn nav_roundtrip_strict_round_report_rejects_unresolved_or_skipped_targets() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-strict-round-report-{}",
            process::id()
        ));
        let round_dir = root.join("round");
        let failure_dir = root.join("failure-artifacts");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&round_dir).expect("create round dir");
        let report = CertifiedAssetOpsBatchReport {
            schema: CERTIFIED_ASSET_OPS_REPORT_SCHEMA.to_string(),
            request_schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            data_dir: "data".to_string(),
            topology_file: "topology.json".to_string(),
            artifact_dir: round_dir.display().to_string(),
            operation_count: 1,
            max_transactions: 1,
            allow_existing_mempool: false,
            prepare_only: false,
            batch_only: false,
            start_height: 1,
            start_state_root: "11".repeat(48),
            start_mempool_pending: 0,
            end_height: Some(2),
            end_state_root: Some("22".repeat(48)),
            end_mempool_pending: Some(0),
            operations: Vec::new(),
            dependency_report: CertifiedAssetOpsDependencyReport::default(),
            batch_file: None,
            round_artifact_dir: Some(round_dir.display().to_string()),
            round_ok: Some(true),
            timings_ms: CertifiedAssetOpsTimingsReport::default(),
        };
        for (field, expected) in [
            (
                "unresolved_vote_targets",
                "non-empty `unresolved_vote_targets`",
            ),
            (
                "skipped_certified_send_targets",
                "non-empty `skipped_certified_send_targets`",
            ),
        ] {
            write_json_file(
                &round_dir.join("peer-certified-mempool-round.report.json"),
                &serde_json::json!({
                    "round": {
                        "allow_peer_failures": false,
                        "certified_sends_deferred": false,
                        "unresolved_vote_targets": if field == "unresolved_vote_targets" {
                            serde_json::json!(["validator-4"])
                        } else {
                            serde_json::json!([])
                        },
                        "vote_request_failures": [],
                        "send_failures": [],
                        "skipped_certified_send_targets": if field == "skipped_certified_send_targets" {
                            serde_json::json!(["validator-4"])
                        } else {
                            serde_json::json!([])
                        },
                        "all_vote_requests_verified": true,
                        "all_sends_verified": true,
                        "round_ok": true
                    }
                }),
            )
            .expect("write strict round report");
            let error = nav_roundtrip_require_certified_ops_ok("strict-stage", &report, &failure_dir)
                .expect_err("strict live-value mode must reject unresolved/skipped targets");
            assert!(error.contains(expected), "{error}");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_strict_round_report_accepts_quorum_early_unresolved_after_full_send() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-strict-quorum-early-full-send-{}",
            process::id()
        ));
        let round_dir = root.join("round");
        let failure_dir = root.join("failure-artifacts");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&round_dir).expect("create round dir");
        let report = CertifiedAssetOpsBatchReport {
            schema: CERTIFIED_ASSET_OPS_REPORT_SCHEMA.to_string(),
            request_schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            data_dir: "data".to_string(),
            topology_file: "topology.json".to_string(),
            artifact_dir: round_dir.display().to_string(),
            operation_count: 1,
            max_transactions: 1,
            allow_existing_mempool: false,
            prepare_only: false,
            batch_only: false,
            start_height: 1,
            start_state_root: "11".repeat(48),
            start_mempool_pending: 0,
            end_height: Some(2),
            end_state_root: Some("22".repeat(48)),
            end_mempool_pending: Some(0),
            operations: Vec::new(),
            dependency_report: CertifiedAssetOpsDependencyReport::default(),
            batch_file: None,
            round_artifact_dir: Some(round_dir.display().to_string()),
            round_ok: Some(true),
            timings_ms: CertifiedAssetOpsTimingsReport::default(),
        };
        write_json_file(
            &round_dir.join("peer-certified-mempool-round.report.json"),
            &serde_json::json!({
                "round": {
                    "allow_peer_failures": false,
                    "certified_sends_deferred": false,
                    "quorum_early_full_propagation": true,
                    "local_apply_before_certified_send": true,
                    "unresolved_vote_targets": ["validator-4"],
                    "vote_request_failures": [],
                    "send_failures": [],
                    "skipped_certified_send_targets": [],
                    "all_vote_requests_verified": true,
                    "all_sends_verified": true,
                    "round_ok": true
                }
            }),
        )
        .expect("write strict round report");

        nav_roundtrip_require_certified_ops_ok("strict-stage", &report, &failure_dir)
            .expect("quorum-early unresolved target should pass after full send verification");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_strict_round_report_accepts_unresolved_vote_when_operator_quorum_caught_up() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-strict-operator-quorum-caught-up-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let round_dir = root.join("round");
        let failure_dir = root.join("failure-artifacts");
        let topology_file = root.join("topology.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&round_dir).expect("create round dir");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 3,
        })
        .expect("init node store");
        let local_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("local status");

        let mut peers = Vec::new();
        let mut rpc_threads = Vec::new();
        for node_id in ["validator-0", "validator-1", "validator-2"] {
            let public_rpc_listener =
                TcpListener::bind(("127.0.0.1", 0)).expect("bind fake public RPC");
            let public_rpc_port = public_rpc_listener
                .local_addr()
                .expect("fake public RPC addr")
                .port();
            peers.push(serde_json::json!({
                "node_id": node_id,
                "host": "127.0.0.1",
                "p2p_port": public_rpc_port.saturating_sub(1),
                "rpc_port": public_rpc_port,
                "p2p_address": format!("127.0.0.1:{}", public_rpc_port.saturating_sub(1))
            }));
            let expected_node_id = node_id.to_string();
            let mut public_status = local_status.clone();
            public_status.node_id = expected_node_id.clone();
            if expected_node_id == local_status.node_id {
                public_status.block_height = public_status
                    .block_height
                    .checked_sub(1)
                    .unwrap_or(public_status.block_height);
                public_status.state_root = format!("stale-{}", public_status.state_root);
            }
            rpc_threads.push(std::thread::spawn(move || {
                let (mut stream, _) = public_rpc_listener
                    .accept()
                    .expect("accept fake public RPC");
                set_stream_timeout(&stream, 5_000).expect("set fake RPC timeout");
                let line = read_transport_line(&stream, "fake public RPC request read")
                    .expect("read fake public RPC request");
                let request: RpcRequest =
                    serde_json::from_str(&line).expect("parse fake public RPC request");
                assert_eq!("status", request.method);
                let response = success_response(
                    &request.id,
                    &public_status,
                    vec![RpcEvent::new(
                        "status",
                        &expected_node_id,
                        "status queried",
                    )],
                )
                .expect("fake public RPC response");
                write_json_line(&mut stream, &response).expect("write fake public RPC response");
            }));
        }
        write_json_file(
            &topology_file,
            &serde_json::json!({
                "topology_id": "strict-operator-quorum-caught-up-test",
                "chain_id": local_status.chain_id,
                "genesis_hash": local_status.genesis_hash,
                "protocol_version": local_status.protocol_version,
                "peers": peers
            }),
        )
        .expect("write topology");

        let report = CertifiedAssetOpsBatchReport {
            schema: CERTIFIED_ASSET_OPS_REPORT_SCHEMA.to_string(),
            request_schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            data_dir: data_dir.display().to_string(),
            topology_file: topology_file.display().to_string(),
            artifact_dir: round_dir.display().to_string(),
            operation_count: 1,
            max_transactions: 1,
            allow_existing_mempool: false,
            prepare_only: false,
            batch_only: false,
            start_height: local_status.block_height.saturating_sub(1),
            start_state_root: "11".repeat(48),
            start_mempool_pending: 0,
            end_height: Some(local_status.block_height),
            end_state_root: Some(local_status.state_root.clone()),
            end_mempool_pending: Some(0),
            operations: Vec::new(),
            dependency_report: CertifiedAssetOpsDependencyReport::default(),
            batch_file: None,
            round_artifact_dir: Some(round_dir.display().to_string()),
            round_ok: Some(true),
            timings_ms: CertifiedAssetOpsTimingsReport::default(),
        };
        write_json_file(
            &round_dir.join("peer-certified-mempool-round.report.json"),
            &serde_json::json!({
                "round": {
                    "allow_peer_failures": false,
                    "certified_sends_deferred": false,
                    "unresolved_vote_targets": ["validator-2"],
                    "vote_request_failures": [],
                    "send_failures": [],
                    "skipped_certified_send_targets": [],
                    "all_vote_requests_verified": true,
                    "all_sends_verified": true,
                    "round_ok": true
                }
            }),
        )
        .expect("write strict round report");

        nav_roundtrip_require_certified_ops_ok("strict-stage", &report, &failure_dir)
            .expect("operator-local quorum catch-up should satisfy strict unresolved-vote fallback");
        for rpc_thread in rpc_threads {
            rpc_thread.join().expect("fake public RPC thread");
        }
        let catchup = serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(failure_dir.join("strict-public-fleet-caught-up.json"))
                .expect("read catch-up artifact"),
        )
        .expect("parse catch-up artifact");
        assert_eq!(
            catchup["schema"],
            "postfiat-nav-roundtrip-strict-public-fleet-caught-up-v1"
        );
        assert!(
            catchup["validator_states"]
                .as_array()
                .expect("validator states")
                .iter()
                .any(|state| state["source"] == "operator_local_state")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_strict_round_report_accepts_peer_certified_batch_report() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-strict-batch-round-report-{}",
            process::id()
        ));
        let round_dir = root.join("round");
        let failure_dir = root.join("failure-artifacts");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&round_dir).expect("create round dir");
        let report = CertifiedAssetOpsBatchReport {
            schema: CERTIFIED_ASSET_OPS_REPORT_SCHEMA.to_string(),
            request_schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            data_dir: "data".to_string(),
            topology_file: "topology.json".to_string(),
            artifact_dir: round_dir.display().to_string(),
            operation_count: 1,
            max_transactions: 1,
            allow_existing_mempool: false,
            prepare_only: false,
            batch_only: false,
            start_height: 1,
            start_state_root: "11".repeat(48),
            start_mempool_pending: 0,
            end_height: Some(2),
            end_state_root: Some("22".repeat(48)),
            end_mempool_pending: Some(0),
            operations: Vec::new(),
            dependency_report: CertifiedAssetOpsDependencyReport::default(),
            batch_file: None,
            round_artifact_dir: Some(round_dir.join("peer-certified-batch-round").display().to_string()),
            round_ok: Some(true),
            timings_ms: CertifiedAssetOpsTimingsReport::default(),
        };
        write_json_file(
            &round_dir.join("peer-certified-batch-round.report.json"),
            &serde_json::json!({
                "allow_peer_failures": false,
                "certified_sends_deferred": false,
                "unresolved_vote_targets": [],
                "vote_request_failures": [],
                "send_failures": [],
                "skipped_certified_send_targets": [],
                "all_vote_requests_verified": true,
                "all_sends_verified": true,
                "round_ok": true,
                "local_state": {
                    "node_id": "validator-0",
                    "block_height": 2,
                    "state_root": "22".repeat(48)
                },
                "sends": []
            }),
        )
        .expect("write strict batch round report");
        nav_roundtrip_require_certified_ops_ok("strict-stage", &report, &failure_dir)
            .expect("strict live-value mode should accept direct batch round report");
        assert!(
            !failure_dir.join("roundtrip-failure.json").exists(),
            "strict check should not write a failure artifact"
        );
        let states = nav_roundtrip_certified_round_validator_states(&report)
            .expect("direct batch round states");
        assert_eq!(1, states.len());
        assert_eq!("validator-0", states[0].node_id);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_public_validator_consensus_rejects_stale_endpoint() {
        let states = vec![
            NavRoundtripValidatorStateEvidence {
                node_id: "validator-0".to_string(),
                block_height: 119,
                state_root: "aa".repeat(48),
                source: "rpc://validator-0".to_string(),
            },
            NavRoundtripValidatorStateEvidence {
                node_id: "validator-4".to_string(),
                block_height: 114,
                state_root: "bb".repeat(48),
                source: "rpc://validator-4".to_string(),
            },
        ];
        assert!(!nav_roundtrip_validator_states_consensus_ok(&states));
        assert!(!nav_roundtrip_validator_states_consensus_ok(&[]));
    }

    #[test]
    fn rpc_catch_up_rejects_zero_max_blocks_before_work_dir_mutation() {
        let root = env::temp_dir().join(format!(
            "postfiat-rpc-catch-up-zero-max-blocks-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let work_dir = root.join("catch-up-work");
        let _ = std::fs::remove_dir_all(&root);
        let error = rpc_catch_up(RpcCatchUpOptions {
            data_dir,
            source_host: "127.0.0.1".to_string(),
            source_rpc_port: 1,
            work_dir: work_dir.clone(),
            max_blocks: 0,
            timeout_ms: 1,
        })
        .expect_err("zero max_blocks must fail before preflight");
        assert!(
            error.contains("--max-blocks must be between 1"),
            "{error}"
        );
        assert!(
            !work_dir.exists(),
            "work dir should not be created before max_blocks validation"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_evm_withdrawal_uses_old_vault_abi_agent_and_verifies_deltas() {
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;
        use std::sync::{Arc, Mutex};

        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-evm-withdrawal-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let artifact_dir = root.join("artifacts");
        let stakehub_home = root.join("stakehub");
        let fake_cast = root.join("cast");
        let claim_marker = root.join("claim-landed");
        let signatures_file = root.join("signatures.json");
        let burn_report_file = root.join("burn-to-redeem.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&stakehub_home).expect("create stakehub home");
        let socket_path = stakehub_home.join("agent.sock");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");

        let owner = "pf07381735ddb7de134e8be8402b465c9cd8ec7546";
        let issuer = "pf65c9783ceafc0f519a74195e78cc7909f92429c3";
        let vault = "0x1111111111111111111111111111111111111111";
        let verifier = "0x2222222222222222222222222222222222222222";
        let usdc = "0x3333333333333333333333333333333333333333";
        let wallet = "0x4444444444444444444444444444444444444444";
        let asset_id = "87".repeat(48);
        let bucket_id = "66".repeat(48);
        let reserve_packet_hash = "ab".repeat(48);
        let amount_atoms = 5_083_635_u64;
        let source_domain = format!("erc20_bridge_vault:42161:{vault}:{usdc}");
        let destination_ref = format!("evm-erc20:42161:{wallet}");
        let redemption = postfiat_types::VaultBridgeRedemption::new(
            DEFAULT_CHAIN_ID,
            owner,
            issuer,
            asset_id.clone(),
            bucket_id.clone(),
            source_domain,
            7,
            amount_atoms,
            3,
            reserve_packet_hash.clone(),
            destination_ref.clone(),
            "99".repeat(48),
            19,
        )
        .expect("redemption");

        let mut ledger = LedgerState::new(vec![
            Account::new(owner, 0, None),
            Account::new(issuer, 0, None),
        ]);
        ledger.vault_bridge_redemptions.push(redemption.clone());
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write ledger");

        let burn_operation =
            postfiat_types::AssetTransactionOperation::VaultBridgeBurnToRedeem(
                postfiat_types::VaultBridgeBurnToRedeemOperation {
                    owner: owner.to_string(),
                    issuer: issuer.to_string(),
                    asset_id: asset_id.clone(),
                    bucket_id: bucket_id.clone(),
                    amount_atoms,
                    epoch: 3,
                    reserve_packet_hash: reserve_packet_hash.clone(),
                    destination_ref: destination_ref.clone(),
                },
            );
        let certified_ops = CertifiedAssetOpsBatchReport {
            schema: CERTIFIED_ASSET_OPS_REPORT_SCHEMA.to_string(),
            request_schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            data_dir: data_dir.display().to_string(),
            topology_file: root.join("topology.json").display().to_string(),
            artifact_dir: root.join("burn-certified").display().to_string(),
            operation_count: 1,
            max_transactions: 1,
            allow_existing_mempool: false,
            prepare_only: false,
            batch_only: false,
            start_height: 18,
            start_state_root: "11".repeat(48),
            start_mempool_pending: 0,
            end_height: Some(19),
            end_state_root: Some("12".repeat(48)),
            end_mempool_pending: Some(0),
            operations: Vec::new(),
            dependency_report: CertifiedAssetOpsDependencyReport::default(),
            batch_file: None,
            round_artifact_dir: None,
            round_ok: Some(true),
            timings_ms: CertifiedAssetOpsTimingsReport::default(),
        };
        let burn_report = NavRoundtripBurnToRedeemReport {
            schema: NAV_ROUNDTRIP_BURN_TO_REDEEM_REPORT_SCHEMA.to_string(),
            artifact_file: burn_report_file.display().to_string(),
            nav_exit_report_file: root.join("nav-exit.json").display().to_string(),
            settlement_asset_id: asset_id.clone(),
            owner: owner.to_string(),
            amount_atoms,
            destination_ref: destination_ref.clone(),
            owner_balance_before: Some(amount_atoms),
            owner_balance_after: Some(0),
            redemption_id: Some(redemption.redemption_id.clone()),
            settlement_status_before: postfiat_node::VaultBridgeStatusReport {
                schema: postfiat_node::VAULT_BRIDGE_STATUS_REPORT_SCHEMA.to_string(),
                asset_id: asset_id.clone(),
                issuer: issuer.to_string(),
                proof_profile: "test".to_string(),
                valuation_unit: "USDC".to_string(),
                finalized_epoch: 3,
                nav_per_unit: postfiat_types::VAULT_BRIDGE_UNIT,
                circulating_supply: amount_atoms,
                finalized_reserve_packet_hash: reserve_packet_hash.clone(),
                issued_supply_atoms: amount_atoms,
                counted_value_atoms: amount_atoms,
                unallocated_counted_capacity_atoms: 0,
                source_root: "13".repeat(48),
                bucket_count: 1,
                receipt_count: 0,
                bridge_deposit_count: 0,
                allocation_count: 0,
                redemption_count: 1,
                buckets: Vec::new(),
                receipts: Vec::new(),
                bridge_deposits: Vec::new(),
                allocations: Vec::new(),
                redemptions: Vec::new(),
                disclosure: "test".to_string(),
            },
            settlement_status_after: None,
            bundle_dir: root.join("burn-bundle").display().to_string(),
            bundle: postfiat_node::VaultBridgeBurnToRedeemBundleReport {
                schema: postfiat_node::VAULT_BRIDGE_BURN_TO_REDEEM_BUNDLE_SCHEMA.to_string(),
                bundle_dir: root.join("burn-bundle").display().to_string(),
                operation_file: root
                    .join("burn-bundle")
                    .join("burn-to-redeem.operation.json")
                    .display()
                    .to_string(),
                commands_file: root.join("burn-bundle").join("commands.sh").display().to_string(),
                owner: owner.to_string(),
                issuer: issuer.to_string(),
                asset_id: asset_id.clone(),
                bucket_id,
                amount_atoms,
                epoch: 3,
                reserve_packet_hash,
                destination_ref,
                operation: burn_operation,
                commands: Vec::new(),
                trust_boundary: "test".to_string(),
            },
            certified_ops_file: root.join("burn.certified-ops.json").display().to_string(),
            certified_ops_artifact_dir: root.join("burn-certified").display().to_string(),
            certified_ops,
        };
        write_json_file(&burn_report_file, &burn_report).expect("write burn report");
        write_json_file(
            &signatures_file,
            &vec![format!("0x{}", "01".repeat(65))],
        )
        .expect("write signatures");

        let old_digest = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let pending_proof = "0x2222222222222222222222222222222222222222222222222222222222222222";
        let proof_digest = "0x3333333333333333333333333333333333333333333333333333333333333333";
        let pending_withdrawal = "0x4444444444444444444444444444444444444444444444444444444444444444";
        let fake_cast_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = "calldata" ]; then
  case "$2" in
    "submitProof(bytes32,bytes32,uint64,bytes[])") echo 0xaaaabbbb; exit 0 ;;
    "finalizeProof(bytes32)") echo 0xccccdddd; exit 0 ;;
    "{old_submit}") echo 0xeeeeffff; exit 0 ;;
    "finalizeWithdrawal(bytes32)") echo 0x11112222; exit 0 ;;
    "claimWithdrawal(bytes32)") echo 0x33334444; exit 0 ;;
  esac
fi
if [ "$1" = "call" ]; then
  target="$2"
  sig="$3"
  arg="${{4:-}}"
  if [ "$sig" = "balanceOf(address)(uint256)" ]; then
    if [ "$arg" = "{wallet}" ]; then
      if [ -f "{claim_marker}" ]; then echo $(({amount}+1000000)); else echo 1000000; fi
      exit 0
    fi
    if [ "$arg" = "{vault}" ]; then
      if [ -f "{claim_marker}" ]; then echo 1000000; else echo $(({amount}+1000000)); fi
      exit 0
    fi
  fi
  if [ "$sig" = "challenge_delay()(uint64)" ]; then
    echo 0
    exit 0
  fi
  if [ "$target" = "{vault}" ] && [ "$sig" = "{fixed_sig}" ]; then
    echo "execution reverted" >&2
    exit 1
  fi
  if [ "$target" = "{vault}" ] && [ "$sig" = "{old_sig}" ]; then
    echo {old_digest}
    exit 0
  fi
  if [ "$target" = "{verifier}" ] && [ "$sig" = "pendingProofId(bytes32,bytes32,uint64)(bytes32)" ]; then
    echo {pending_proof}
    exit 0
  fi
  if [ "$target" = "{verifier}" ] && [ "$sig" = "proofDigest(bytes32,bytes32,uint64)(bytes32)" ]; then
    echo {proof_digest}
    exit 0
  fi
  if [ "$target" = "{vault}" ] && [ "$sig" = "withdrawalPendingId((uint64,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes),bytes32)(bytes32)" ]; then
    echo {pending_withdrawal}
    exit 0
  fi
fi
echo "unexpected cast args: $*" >&2
exit 9
"#,
            old_submit = NAV_ROUNDTRIP_OLD_SUBMIT_WITHDRAWAL_SIGNATURE,
            fixed_sig = NAV_ROUNDTRIP_FIXED_WITHDRAWAL_DIGEST_SIGNATURE,
            old_sig = NAV_ROUNDTRIP_OLD_WITHDRAWAL_DIGEST_SIGNATURE,
            claim_marker = claim_marker.display(),
            amount = amount_atoms,
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
        let marker_for_thread = claim_marker.clone();
        let agent_thread = std::thread::spawn(move || {
            for _ in 0..9 {
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
                    "open_launch_session" => serde_json::json!({"ok": true, "session": {"id": request.get("session_id").cloned().unwrap_or(serde_json::Value::Null)}}),
                    "evm_contract_tx" => {
                        let action = request
                            .get("session_action")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("");
                        if action == "claim-withdrawal" {
                            std::fs::write(&marker_for_thread, "ok\n")
                                .expect("write claim marker");
                        }
                        serde_json::json!({
                            "ok": true,
                            "tx": format!("0x{:064x}", action.len()),
                            "gas_used": 55_000 + action.len() as u64,
                        })
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
            "--evm-withdrawal-only".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--burn-to-redeem-report".to_string(),
            burn_report_file.display().to_string(),
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
            "--pfusdc".to_string(),
            asset_id.clone(),
            "--signatures-file".to_string(),
            signatures_file.display().to_string(),
            "--session-id".to_string(),
            "nav-roundtrip-withdrawal-test-session".to_string(),
            "--challenge-wait-secs".to_string(),
            "0".to_string(),
        ])
        .expect("EVM withdrawal stage cli");
        agent_thread.join().expect("fake agent thread");

        let report = serde_json::from_str::<NavRoundtripEvmWithdrawalReport>(
            &std::fs::read_to_string(artifact_dir.join("evm-withdrawal.json"))
                .expect("read withdrawal report"),
        )
        .expect("parse withdrawal report");
        assert_eq!(NAV_ROUNDTRIP_EVM_WITHDRAWAL_REPORT_SCHEMA, report.schema);
        assert_eq!(NAV_ROUNDTRIP_BRIDGE_CLASS_CONTROLLED_LAUNCH, report.bridge_class);
        assert_eq!(old_digest, report.withdrawal_packet_digest);
        assert_eq!(pending_proof, report.verifier_pending_proof_id);
        assert_eq!(proof_digest, report.verifier_proof_digest_to_sign);
        assert_eq!(pending_withdrawal, report.vault_pending_withdrawal_id);
        assert!(report.delta_ok, "{:?}", report.failure_reasons);
        assert!(!report.launch_session_managed_externally);
        assert_eq!("1000000", report.wallet_usdc_before_atoms);
        assert_eq!((amount_atoms + 1_000_000).to_string(), report.wallet_usdc_after_atoms);
        assert_eq!((amount_atoms + 1_000_000).to_string(), report.vault_usdc_before_atoms);
        assert_eq!("1000000", report.vault_usdc_after_atoms);
        assert!(artifact_dir.join("submit-withdrawal.calldata.txt").exists());
        assert_eq!(5, report.receipt_watches.len());
        assert_eq!(
            vec![
                "submit-proof",
                "finalize-proof",
                "submit-withdrawal",
                "finalize-withdrawal",
                "claim-withdrawal",
            ],
            report
                .receipt_watches
                .iter()
                .map(|watch| watch.label.as_str())
                .collect::<Vec<_>>()
        );
        assert!(report
            .receipt_watches
            .iter()
            .all(|watch| watch.status == "confirmed"
                && watch.confirmation_source == "stakehub_agent_response"
                && watch.source_rpc_provider_class == "public_or_unknown_http"
                && watch.elapsed_ms >= 0.0));

        let agent_requests = requests.lock().expect("requests lock");
        let actions = agent_requests
            .iter()
            .filter_map(|request| request.get("session_action").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(
            actions,
            vec![
                "submit-proof",
                "finalize-proof",
                "submit-withdrawal",
                "finalize-withdrawal",
                "claim-withdrawal",
            ]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_pftl_settle_closes_redemption_accounting() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-pftl-settle-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let artifact_dir = root.join("artifacts");
        let issuer_key_file = root.join("issuer.key.json");
        let issuer_backup_file = root.join("issuer.backup.json");
        let evm_report_file = root.join("evm-withdrawal.json");
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
            base_port: 44_250,
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
        })
        .expect("write topology");
        let issuer_key = wallet_keygen(WalletKeygenOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            master_seed_hex: "31".repeat(32),
            account_index: 0,
            key_file: issuer_key_file.clone(),
            backup_file: issuer_backup_file,
            overwrite: true,
        })
        .expect("issuer keygen");
        let profile = cli_test_sp1_nav_profile(&issuer_key.address);

        let wallet = "0x4444444444444444444444444444444444444444";
        let vault = "0x1111111111111111111111111111111111111111";
        let verifier = "0x2222222222222222222222222222222222222222";
        let usdc = "0x3333333333333333333333333333333333333333";
        let asset = postfiat_types::AssetDefinition::new(
            DEFAULT_CHAIN_ID,
            issuer_key.address.clone(),
            "PFUSDC",
            1,
            6,
        )
        .expect("asset");
        let mut nav_asset = postfiat_types::NavTrackedAsset::new(
            asset.asset_id.clone(),
            issuer_key.address.clone(),
            issuer_key.address.clone(),
            profile.profile_id.clone(),
            "USDC",
            issuer_key.address.clone(),
        )
        .expect("nav asset");
        nav_asset.finalized_epoch = 3;
        nav_asset.finalized_reserve_packet_hash = "ac".repeat(48);
        nav_asset.nav_per_unit = postfiat_types::VAULT_BRIDGE_UNIT;
        nav_asset.circulating_supply = 5_083_635;
        let amount_atoms = 5_083_635_u64;
        let source_domain = format!("erc20_bridge_vault:42161:{vault}:{usdc}");
        let mut bucket = postfiat_types::VaultBridgeBucketState::new(
            asset.asset_id.clone(),
            source_domain.clone(),
            "42".repeat(48),
            12,
        )
        .expect("bucket");
        bucket.counted_value_atoms = amount_atoms + 1_000_000;
        bucket.redemption_queue_atoms = amount_atoms;
        bucket.last_packet_epoch = 3;
        bucket.validate().expect("bucket valid");
        let redemption = postfiat_types::VaultBridgeRedemption::new(
            DEFAULT_CHAIN_ID,
            "pf07381735ddb7de134e8be8402b465c9cd8ec7546",
            issuer_key.address.clone(),
            asset.asset_id.clone(),
            bucket.bucket_id.clone(),
            source_domain,
            5,
            amount_atoms,
            3,
            nav_asset.finalized_reserve_packet_hash.clone(),
            format!("evm-erc20:42161:{wallet}"),
            "99".repeat(48),
            20,
        )
        .expect("redemption");

        let mut ledger = LedgerState::new(vec![Account::new(
            issuer_key.address.clone(),
            25_000_000,
            Some(issuer_key.public_key_hex.clone()),
        )]);
        ledger.asset_definitions.push(asset.clone());
        ledger.nav_proof_profiles.push(profile);
        ledger.nav_assets.push(nav_asset);
        ledger.vault_bridge_bucket_states.push(bucket.clone());
        ledger.vault_bridge_redemptions.push(redemption.clone());
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write ledger");

        let evm_report = NavRoundtripEvmWithdrawalReport {
            schema: NAV_ROUNDTRIP_EVM_WITHDRAWAL_REPORT_SCHEMA.to_string(),
            artifact_file: evm_report_file.display().to_string(),
            burn_to_redeem_report_file: root.join("burn.json").display().to_string(),
            source_rpc_url: "https://arb.example.invalid/rpc".to_string(),
            source_rpc_provider_class: "public_or_unknown_http".to_string(),
            source_chain_id: 42_161,
            bridge_class: NAV_ROUNDTRIP_BRIDGE_CLASS_CONTROLLED_LAUNCH.to_string(),
            vault_address: vault.to_string(),
            verifier_address: verifier.to_string(),
            usdc_address: usdc.to_string(),
            stakehub_wallet: wallet.to_string(),
            settlement_asset_id: asset.asset_id.clone(),
            redemption_id: redemption.redemption_id.clone(),
            amount_atoms,
            pftl_finalized_height: 20,
            pftl_withdrawal_hash: format!("0x{}", "aa".repeat(48)),
            pftl_withdrawal_hash_commitment: format!("0x{}", "bb".repeat(32)),
            withdrawal_packet_digest: format!("0x{}", "11".repeat(32)),
            verifier_pending_proof_id: format!("0x{}", "22".repeat(32)),
            verifier_proof_digest_to_sign: format!("0x{}", "33".repeat(32)),
            vault_pending_withdrawal_id: format!("0x{}", "44".repeat(32)),
            verifier_challenge_wait_secs: 0,
            vault_challenge_wait_secs: 0,
            session_id: "settle-test".to_string(),
            wallet_usdc_before_atoms: "1000000".to_string(),
            wallet_usdc_after_atoms: (amount_atoms + 1_000_000).to_string(),
            vault_usdc_before_atoms: (amount_atoms + 1_000_000).to_string(),
            vault_usdc_after_atoms: "1000000".to_string(),
            launch_session_managed_externally: false,
            submit_proof_tx: format!("0x{}", "55".repeat(32)),
            submit_proof_gas_used: 10,
            finalize_proof_tx: format!("0x{}", "66".repeat(32)),
            finalize_proof_gas_used: 10,
            submit_withdrawal_tx: format!("0x{}", "77".repeat(32)),
            submit_withdrawal_gas_used: 10,
            finalize_withdrawal_tx: format!("0x{}", "88".repeat(32)),
            finalize_withdrawal_gas_used: 10,
            claim_withdrawal_tx: format!("0x{}", "99".repeat(32)),
            claim_withdrawal_gas_used: 10,
            submit_proof_calldata_file: "submit".to_string(),
            finalize_proof_calldata_file: "finalize-proof".to_string(),
            submit_withdrawal_calldata_file: "submit-withdrawal".to_string(),
            finalize_withdrawal_calldata_file: "finalize-withdrawal".to_string(),
            claim_withdrawal_calldata_file: "claim".to_string(),
            agent_open_session_file: "open".to_string(),
            agent_submit_proof_file: "submit-proof".to_string(),
            agent_finalize_proof_file: "finalize-proof".to_string(),
            agent_submit_withdrawal_file: "submit-withdrawal".to_string(),
            agent_finalize_withdrawal_file: "finalize-withdrawal".to_string(),
            agent_claim_withdrawal_file: "claim".to_string(),
            agent_close_session_file: "close".to_string(),
            receipt_watches: Vec::new(),
            delta_ok: true,
            failure_reasons: Vec::new(),
        };
        write_json_file(&evm_report_file, &evm_report).expect("write evm report");

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--pftl-settle-only".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            data_dir.join(VALIDATOR_KEYS_FILE).display().to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--evm-withdrawal-report".to_string(),
            evm_report_file.display().to_string(),
            "--pfusdc".to_string(),
            asset.asset_id.clone(),
            "--settlement-key-file".to_string(),
            issuer_key_file.display().to_string(),
            "--local-apply-before-certified-send".to_string(),
            "--quorum-early-full-propagation".to_string(),
            "--prepare-only".to_string(),
        ])
        .expect("PFTL settle stage cli");

        let report = serde_json::from_str::<NavRoundtripPftlSettleReport>(
            &std::fs::read_to_string(artifact_dir.join("pftl-settle.json"))
                .expect("read settle report"),
        )
        .expect("parse settle report");
        assert_eq!(NAV_ROUNDTRIP_PFTL_SETTLE_REPORT_SCHEMA, report.schema);
        assert_eq!(None, report.accounting_ok);
        assert!(report.failure_reasons.is_empty(), "{:?}", report.failure_reasons);
        assert_eq!(None, report.redemption_state_after);
        assert_eq!(Some(amount_atoms), report.redemption_queue_before_atoms);
        assert_eq!(None, report.redemption_queue_after_atoms);
        assert_eq!(Some(amount_atoms + 1_000_000), report.counted_value_before_atoms);
        assert_eq!(None, report.counted_value_after_atoms);
        assert_eq!(
            nav_roundtrip_vault_bridge_settlement_receipt_hash(&evm_report),
            report.settlement_receipt_hash
        );
        let operation = serde_json::from_str::<postfiat_types::AssetTransactionOperation>(
            &std::fs::read_to_string(&report.operation_file).expect("read settle operation"),
        )
        .expect("parse settle operation");
        let postfiat_types::AssetTransactionOperation::VaultBridgeRedeemSettle(operation) =
            operation
        else {
            panic!("expected vault bridge settle operation");
        };
        assert_eq!(issuer_key.address, operation.issuer_or_redemption_account);
        assert_eq!(asset.asset_id, operation.asset_id);
        assert_eq!(redemption.redemption_id, operation.redemption_id);
        assert_eq!(amount_atoms, operation.settled_atoms);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn wallet_sign_offer_transaction_signs_offer_fee_quote() {
        let root = env::temp_dir().join(format!("postfiat-wallet-sign-offer-{}", process::id()));
        let key_file = root.join("operator-key.json");
        let backup_file = root.join("operator-backup.json");
        let quote_file = root.join("offer-quote.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");

        let key_report = wallet_keygen(WalletKeygenOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            master_seed_hex: "22".repeat(32),
            account_index: 0,
            key_file: key_file.clone(),
            backup_file: backup_file.clone(),
            overwrite: true,
        })
        .expect("wallet keygen");
        let genesis = postfiat_types::Genesis::new(DEFAULT_CHAIN_ID);
        let issued_asset_id = "11".repeat(postfiat_types::ISSUED_ASSET_ID_HEX_LEN / 2);
        let operation =
            postfiat_types::OfferTransactionOperation::OfferCreate(
                postfiat_types::OfferCreateOperation {
                    owner: key_report.address.clone(),
                    taker_gets_asset_id: issued_asset_id,
                    taker_gets_amount: 100,
                    taker_pays_asset_id: "PFT".to_string(),
                    taker_pays_amount: 200,
                    expiration_height: 99,
                },
            );
        let quote = OfferFeeQuoteReport {
            schema: "postfiat-offer-fee-quote-v1".to_string(),
            transaction_kind: operation.transaction_kind().to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: postfiat_execution::genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            source: key_report.address.clone(),
            sequence: 1,
            sequence_source: "provided".to_string(),
            sender_balance: 10_000,
            sender_sequence: 0,
            mempool_pending_for_sender: 0,
            base_offer_fee: 1,
            match_fee: 0,
            state_expansion_fee: 0,
            estimated_cross_count: 0,
            max_dex_crosses_per_transaction: postfiat_types::MAX_DEX_CROSSES_PER_TRANSACTION
                as u64,
            will_create_residual_offer: true,
            offer_object_reserve: postfiat_types::OFFER_OBJECT_RESERVE,
            minimum_fee: 1,
            account_reserve: postfiat_execution::ACCOUNT_RESERVE,
            transfer_fee_byte_quantum: postfiat_execution::TRANSFER_FEE_BYTE_QUANTUM as u64,
            transfer_fee_per_quantum: postfiat_execution::TRANSFER_FEE_PER_QUANTUM,
            offer_weight_bytes: 1,
            sender_balance_after_fee: Some(9_999),
            sender_balance_after_fee_and_reserve: Some(9_999 - postfiat_types::OFFER_OBJECT_RESERVE),
            sender_meets_reserve_after_fee: true,
            sender_meets_reserve_after_fee_and_reserve: true,
            operation: operation.clone(),
        };
        std::fs::write(
            &quote_file,
            serde_json::to_string_pretty(&quote).expect("serialize quote"),
        )
        .expect("write quote");

        let parsed_quote =
            read_wallet_sign_offer_transaction_quote_file(&quote_file).expect("read quote");
        let signed = wallet_sign_offer_transaction(WalletSignOfferTransactionOptions {
            key_file: key_file.clone(),
            chain_id: parsed_quote.chain_id,
            genesis_hash: parsed_quote.genesis_hash,
            protocol_version: parsed_quote.protocol_version,
            fee: parsed_quote.minimum_fee,
            sequence: parsed_quote.sequence,
            expected_source: Some(parsed_quote.source),
            operation: parsed_quote.operation,
        })
        .expect("sign offer transaction");
        assert_eq!(signed.unsigned.source, key_report.address);
        assert_eq!(signed.unsigned.operation, operation);
        assert_eq!(signed.unsigned.fee, 1);

        run_cli(vec![
            "wallet-sign-offer-transaction".to_string(),
            "--key-file".to_string(),
            key_file.display().to_string(),
            "--quote-file".to_string(),
            quote_file.display().to_string(),
        ])
        .expect("wallet sign offer transaction cli");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn market_ops_status_zeros_caps_when_stale() {
        let root = env::temp_dir().join(format!(
            "postfiat-market-ops-status-stale-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let _ = std::fs::remove_dir_all(&root);

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");

        let (mut ledger, asset_id, _record, _policy) = finalized_market_ops_fixture();
        ledger.nav_reserve_packets[0].state = "challenged".to_string();
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write stale market ops ledger");

        let report = market_ops_status(MarketOpsStatusOptions {
            data_dir: data_dir.clone(),
            asset_id,
            epoch: Some(1),
        })
        .expect("market ops stale status report");
        assert_eq!(
            postfiat_node::MARKET_OPS_STATUS_STALE,
            report.market_operations_status
        );
        assert!(!report.reserve_packet_fresh);
        assert_eq!(0, report.current_reserve_deploy_cap_usd_e8);
        assert_eq!(0, report.current_mint_cap_atoms);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn market_ops_status_zeros_caps_when_underfunded() {
        let root = env::temp_dir().join(format!(
            "postfiat-market-ops-status-underfunded-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let _ = std::fs::remove_dir_all(&root);

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");

        let (mut ledger, asset_id, _record, _policy) = finalized_market_ops_fixture();
        let record = &mut ledger.market_ops_envelopes[0];
        record.envelope.funded_alignment_reserve_usd_e8 =
            record.envelope.required_alignment_reserve_usd_e8 - 1;
        record.envelope_hash = bytes_to_hex(&record.envelope.envelope_hash());
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write underfunded market ops ledger");

        let report = market_ops_status(MarketOpsStatusOptions {
            data_dir: data_dir.clone(),
            asset_id,
            epoch: Some(1),
        })
        .expect("market ops underfunded status report");
        assert_eq!(
            postfiat_node::MARKET_OPS_STATUS_UNDERFUNDED,
            report.market_operations_status
        );
        assert!(report.reserve_packet_fresh);
        assert!(report.supply_packet_fresh);
        assert_eq!(0, report.current_reserve_deploy_cap_usd_e8);
        assert_eq!(0, report.current_mint_cap_atoms);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn market_ops_status_rejects_forbidden_disclosure_language() {
        let root = env::temp_dir().join(format!(
            "postfiat-market-ops-status-disclosure-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let _ = std::fs::remove_dir_all(&root);

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");

        let (ledger, asset_id, _record, _policy) = finalized_market_ops_fixture();
        postfiat_storage::NodeStore::new(&data_dir)
            .write_ledger(&ledger)
            .expect("write market ops ledger");

        let mut report = market_ops_status(MarketOpsStatusOptions {
            data_dir: data_dir.clone(),
            asset_id,
            epoch: Some(1),
        })
        .expect("status report");
        report.disclosure = "This is a peg with guaranteed liquidity.".to_string();
        let error = report
            .validate()
            .expect_err("forbidden disclosure language must reject");
        assert!(error.contains("disclosure"), "{error}");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_pftl_only_rejects_split_owner_subscriber() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-pftl-only-split-owner-{}",
            process::id()
        ));
        let error = nav_roundtrip_pftl_only(NavRoundtripPftlOnlyOptions {
            data_dir: root.join("node"),
            topology_file: root.join("topology.json"),
            validator_key_file: root.join("validator.key.json"),
            proposal_key_file: None,
            artifact_dir: root.join("artifacts"),
            nav_asset_id: "a651".to_string(),
            settlement_asset_id: "pfusdc".to_string(),
            subscriber: "subscriber".to_string(),
            owner: "owner".to_string(),
            issuer_key_file: root.join("issuer.key.json"),
            owner_key_file: root.join("owner.key.json"),
            submitter_key_file: None,
            mint_amount: 1,
            settlement_amount_atoms: None,
            settlement_receipt_id: None,
            settlement_supply_allocation_id: None,
            same_round_nav_exit: false,
            destination_ref: None,
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
            reuse_final_certified_state: false,
            fast_demo_preflight: false,
            background_audit: false,
            resume: false,
            overwrite: false,
            batch_only: false,
        })
        .expect_err("PFTL-only split owner/subscriber must reject before side effects");
        assert!(
            error.contains("requires --subscriber and --owner to be the same account"),
            "{error}"
        );
    }

    #[test]
    fn nav_roundtrip_pftl_only_rejects_stage_flag_conflict() {
        let error = run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--pftl-only".to_string(),
            "--nav-exit-only".to_string(),
        ])
        .expect_err("PFTL-only cannot be combined with stage-only modes");
        assert!(
            error.contains("--pftl-only cannot be combined with a --*-only stage flag"),
            "{error}"
        );
    }

    #[test]
    fn nav_roundtrip_dashboard_status_distinguishes_pftl_only_bridge_deferred() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-dashboard-status-{}",
            process::id()
        ));
        let summary_file = root.join("pftl-only-summary.json");
        let report_file = root.join("dashboard-status.json");
        let bridge_resume_file = root.join("bridge-out-resume.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");

        write_json_file(
            &summary_file,
            &serde_json::json!({
                "schema": NAV_ROUNDTRIP_PFTL_ONLY_REPORT_SCHEMA,
                "artifact_file": summary_file.display().to_string(),
                "run_class": NAV_ROUNDTRIP_RUN_CLASS_PFTL_ONLY,
                "completion_status": NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED,
                "custody_location": NAV_ROUNDTRIP_CUSTODY_PFTL_SETTLEMENT_ASSET_BALANCE,
                "bridge_out_resume_file": bridge_resume_file.display().to_string(),
                "bridge_out_resume": {
                    "artifact_file": bridge_resume_file.display().to_string(),
                    "suggested_command": "postfiat-node nav-roundtrip-live-demo --burn-to-redeem-only --resume"
                },
                "timing_scope": nav_roundtrip_default_pftl_only_timing_scope(),
                "protocol_clock_started_at_stage": nav_roundtrip_default_pftl_protocol_clock_start(),
                "protocol_clock_stopped_at_stage": nav_roundtrip_default_pftl_protocol_clock_stop(),
                "setup_or_recovery_work_included_in_total": false,
                "source_rpc_provider_class": "local",
                "background_audit_enabled": true,
                "final_audit_profile": "background_audit_certified_round_hot_path",
                "final_validator_state_source": "certified_round",
                "timings_ms": {
                    "total_ms": 25.0,
                    "readiness_preflight_ms": 4.0,
                    "protocol_clock_ms": 21.0,
                    "fleet_preflight_ms": 4.0,
                    "preflight_ms": 0.0,
                    "stakehub_session_ms": 0.0,
                    "stakehub_session_close_ms": 0.0,
                    "evm_deposit_ms": 0.0,
                    "deposit_relay_ms": 0.0,
                    "primary_mint_ms": 5.0,
                    "nav_money_in_ms": 5.0,
                    "nav_exit_ms": 4.0,
                    "nav_money_out_ms": 5.0,
                    "burn_to_redeem_ms": 0.0,
                    "withdrawal_signature_ms": 0.0,
                    "evm_withdrawal_ms": 0.0,
                    "pftl_settle_ms": 0.0,
                    "final_verification_ms": 2.0
                },
                "final_summary_ok": true,
                "final_validator_consensus_ok": true,
                "final_mempool_pending": 0,
                "final_height": 55,
                "final_state_root": "ab".repeat(48),
                "nav_money_in": { "delta_ok": true },
                "nav_money_out": { "delta_ok": true }
            }),
        )
        .expect("write PFTL-only summary");

        run_cli(vec![
            "nav-roundtrip-dashboard-status".to_string(),
            "--summary".to_string(),
            summary_file.display().to_string(),
            "--report".to_string(),
            report_file.display().to_string(),
        ])
        .expect("dashboard status cli");
        let report = serde_json::from_str::<NavRoundtripDashboardStatusReport>(
            &std::fs::read_to_string(&report_file).expect("read dashboard status"),
        )
        .expect("parse dashboard status");
        assert_eq!(NAV_ROUNDTRIP_DASHBOARD_STATUS_SCHEMA, report.schema);
        assert_eq!(NAV_ROUNDTRIP_RUN_CLASS_PFTL_ONLY, report.run_class);
        assert_eq!(
            NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED,
            report.completion_status
        );
        assert_eq!(
            NAV_ROUNDTRIP_CUSTODY_PFTL_SETTLEMENT_ASSET_BALANCE,
            report.custody_location
        );
        assert!(!report.full_arbitrum_roundtrip_complete);
        assert!(report.pftl_only_complete);
        assert!(report.bridge_out_deferred);
        assert_eq!(
            "PFTL-only complete; bridge-out deferred",
            report.display_status
        );
        assert_eq!(
            Some(bridge_resume_file.display().to_string()),
            report.bridge_out_resume_file
        );
        assert_eq!(
            Some(
                "postfiat-node nav-roundtrip-live-demo --burn-to-redeem-only --resume".to_string()
            ),
            report.bridge_out_resume_command
        );
        assert_eq!(
            Some("pftl_only_protocol_clock_with_blocking_safety_checks".to_string()),
            report.timing_scope
        );
        assert_eq!(
            Some("primary_mint".to_string()),
            report.protocol_clock_started_at_stage
        );
        assert_eq!(
            Some("final_verification".to_string()),
            report.protocol_clock_stopped_at_stage
        );
        assert_eq!(Some(false), report.setup_or_recovery_work_included_in_total);
        assert!(report.timing_boundary_ok);
        assert!(report.benchmark_clean_timing);
        assert_eq!(Some(25.0), report.total_ms);
        assert_eq!(Some(4.0), report.readiness_preflight_ms);
        assert_eq!(Some(21.0), report.protocol_clock_ms);
        assert!(report.timings_ms.is_some());
        assert_eq!(
            Some("local".to_string()),
            report.source_rpc_provider_class
        );
        assert_eq!(None, report.bridge_class);
        assert_eq!(Some(true), report.background_audit_enabled);
        assert_eq!(
            Some("background_audit_certified_round_hot_path".to_string()),
            report.final_audit_profile
        );
        assert_eq!(
            Some("certified_round".to_string()),
            report.final_validator_state_source
        );
        assert!(report.failure_reasons.is_empty(), "{:?}", report.failure_reasons);

        let _ = std::fs::remove_dir_all(root);
    }
