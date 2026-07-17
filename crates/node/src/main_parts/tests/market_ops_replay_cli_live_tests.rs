    #[test]
    fn nav_roundtrip_auto_signs_withdrawal_signature_bundle() {
        let root = std::env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-auto-sign-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        let signature_request_file = root.join("signature-request.json");
        let signatures_file = root.join("signatures.json");
        let signer_key_file = root.join("withdrawal-signer.json");
        let digest = "0x49d3df75501522ff2cd08d03f252aa52b487a802f440e570564c27f01e5bb7a0";
        let expected_signer = "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf";

        write_json_file(
            &signature_request_file,
            &serde_json::json!({
                "verifier_proof_digest_to_sign": digest,
            }),
        )
        .expect("write signature request");
        write_json_file(
            &signer_key_file,
            &serde_json::json!({
                "address": expected_signer,
                "private_key": format!("{:0>64}", "1"),
            }),
        )
        .expect("write signer key");

        let report = nav_roundtrip_auto_sign_withdrawal_bundle(
            &signature_request_file,
            &signatures_file,
            &signer_key_file,
        )
        .expect("auto sign withdrawal bundle");
        assert_eq!(
            NAV_ROUNDTRIP_WITHDRAWAL_AUTO_SIGNATURE_REPORT_SCHEMA,
            report.schema
        );
        assert_eq!(expected_signer, report.signer_address);
        assert_eq!(1, report.signature_count);
        assert!(report.private_key_material_redacted);

        let signatures =
            nav_roundtrip_read_evm_signatures(&signatures_file).expect("read generated signature");
        assert_eq!(1, signatures.len());
        assert_eq!(
            expected_signer,
            nav_roundtrip_recover_evm_signer(digest, &signatures[0]).expect("recover signer")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_aligns_withdrawal_signature_request_to_live_old_vault_digest() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-align-signature-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        let plan_file = root.join("plan.json");
        let request_file = root.join("signature-request.json");
        let fake_cast = root.join("cast");
        let vault = "0x1111111111111111111111111111111111111111";
        let verifier = "0x2222222222222222222222222222222222222222";
        let usdc = "0x3333333333333333333333333333333333333333";
        let wallet = "0x4444444444444444444444444444444444444444";
        let generic_packet_digest = format!("0x{}", "11".repeat(32));
        let live_packet_digest = format!("0x{}", "22".repeat(32));
        let generic_proof_digest = format!("0x{}", "33".repeat(32));
        let live_proof_digest = format!("0x{}", "44".repeat(32));
        let live_pending_proof = format!("0x{}", "55".repeat(32));
        let live_pending_withdrawal = format!("0x{}", "66".repeat(32));
        let pftl_hash_commitment = format!("0x{}", "77".repeat(32));

        write_json_file(
            &plan_file,
            &serde_json::json!({
                "schema": postfiat_node::VAULT_BRIDGE_WITHDRAWAL_PLAN_SCHEMA,
                "asset_id": "87".repeat(48),
                "redemption_id": "a1".repeat(48),
                "redemption_state": postfiat_types::VAULT_BRIDGE_REDEMPTION_STATE_PENDING,
                "pftl_finalized_height": 8,
                "withdrawal_packet": {
                    "pftl_chain_id": 1,
                    "source_chain_id": 42161,
                    "vault_address": vault,
                    "token_address": usdc,
                    "vault_bridge_asset_id": "87".repeat(48),
                    "burn_tx_id": "b2".repeat(48),
                    "withdrawal_id": "a1".repeat(48),
                    "recipient": wallet,
                    "amount_atoms": 5_083_635_u64,
                    "source_bucket_id": "c3".repeat(48),
                    "destination_hash": "d4".repeat(48),
                    "finalized_height": 8,
                    "evidence_root": "e5".repeat(48)
                },
                "withdrawal_packet_evm_args": {
                    "pftl_chain_id": 1,
                    "source_chain_id": 42161,
                    "vault_address": vault,
                    "token_address": usdc,
                    "vault_bridge_asset_id": format!("0x{}", "87".repeat(48)),
                    "burn_tx_id": format!("0x{}", "b2".repeat(48)),
                    "withdrawal_id": format!("0x{}", "a1".repeat(48)),
                    "recipient": wallet,
                    "amount": 5_083_635_u64,
                    "source_bucket_id": format!("0x{}", "c3".repeat(48)),
                    "destination_hash": format!("0x{}", "d4".repeat(48)),
                    "finalized_height": 8,
                    "evidence_root": format!("0x{}", "e5".repeat(48))
                },
                "withdrawal_packet_tuple_arg": "(unused)",
                "withdrawal_packet_hash": "f6".repeat(48),
                "pftl_withdrawal_hash": format!("0x{}", "f6".repeat(48)),
                "pftl_withdrawal_hash_commitment": pftl_hash_commitment,
                "withdrawal_packet_evm_digest": generic_packet_digest,
                "verifier_pending_proof_id": format!("0x{}", "88".repeat(32)),
                "verifier_withdrawal_key": format!("0x{}", "99".repeat(32)),
                "verifier_proof_digest_to_sign": generic_proof_digest,
                "verifier_submit_proof_signature": "submitProof(bytes32,bytes32,uint64,bytes[])",
                "verifier_submit_proof_cast_args": [],
                "verifier_submit_proof_cast_command": "",
                "vault_pending_withdrawal_id": format!("0x{}", "aa".repeat(32)),
                "vault_submit_withdrawal_signature": NAV_ROUNDTRIP_FIXED_SUBMIT_WITHDRAWAL_SIGNATURE,
                "vault_submit_withdrawal_cast_args": [],
                "vault_submit_withdrawal_cast_command": "",
                "signatures": [],
                "trust_boundary": "test"
            }),
        )
        .expect("write plan");
        write_json_file(
            &request_file,
            &serde_json::json!({
                "schema": postfiat_node::VAULT_BRIDGE_WITHDRAWAL_SIGNATURE_REQUEST_SCHEMA,
                "asset_id": "87".repeat(48),
                "redemption_id": "a1".repeat(48),
                "pftl_finalized_height": 8,
                "evm_chain_id": 42161,
                "verifier_address": verifier,
                "withdrawal_packet_evm_digest": generic_packet_digest,
                "pftl_withdrawal_hash_commitment": pftl_hash_commitment,
                "verifier_proof_digest_to_sign": generic_proof_digest,
                "verifier_pending_proof_id": format!("0x{}", "88".repeat(32)),
                "verifier_withdrawal_key": format!("0x{}", "99".repeat(32)),
                "cast_wallet_sign_command": "cast wallet sign --no-hash old",
                "signatures_file_note": "test"
            }),
        )
        .expect("write request");

        let fake_cast_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = "call" ]; then
  target="$2"
  sig="$3"
  if [ "$target" = "{vault}" ] && [ "$sig" = "{fixed_sig}" ]; then
    echo "execution reverted" >&2
    exit 1
  fi
  if [ "$target" = "{vault}" ] && [ "$sig" = "{old_sig}" ]; then
    echo {live_packet_digest}
    exit 0
  fi
  if [ "$target" = "{verifier}" ] && [ "$sig" = "pendingProofId(bytes32,bytes32,uint64)(bytes32)" ]; then
    echo {live_pending_proof}
    exit 0
  fi
  if [ "$target" = "{verifier}" ] && [ "$sig" = "proofDigest(bytes32,bytes32,uint64)(bytes32)" ]; then
    echo {live_proof_digest}
    exit 0
  fi
  if [ "$target" = "{vault}" ] && [ "$sig" = "withdrawalPendingId((uint64,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes),bytes32)(bytes32)" ]; then
    echo {live_pending_withdrawal}
    exit 0
  fi
fi
echo "unexpected cast args: $*" >&2
exit 9
"#,
            fixed_sig = NAV_ROUNDTRIP_FIXED_WITHDRAWAL_DIGEST_SIGNATURE,
            old_sig = NAV_ROUNDTRIP_OLD_WITHDRAWAL_DIGEST_SIGNATURE,
        );
        let fake_cast_staging = root.join("cast.tmp");
        std::fs::write(&fake_cast_staging, fake_cast_script).expect("write staged fake cast");
        let mut permissions = std::fs::metadata(&fake_cast_staging)
            .expect("fake cast metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&fake_cast_staging, permissions).expect("chmod staged fake cast");
        std::fs::rename(&fake_cast_staging, &fake_cast).expect("publish fake cast atomically");

        nav_roundtrip_align_withdrawal_signature_request_with_live_abi(
            fake_cast.to_str().expect("fake cast path"),
            "https://arb.example.invalid/rpc",
            vault,
            verifier,
            usdc,
            wallet,
            &plan_file,
            &request_file,
        )
        .expect("align signature request");

        let request = serde_json::from_str::<postfiat_node::VaultBridgeWithdrawalSignatureRequest>(
            &std::fs::read_to_string(&request_file).expect("read request"),
        )
        .expect("parse request");
        assert_eq!(live_packet_digest, request.withdrawal_packet_evm_digest);
        assert_eq!(live_proof_digest, request.verifier_proof_digest_to_sign);
        assert_eq!(live_pending_proof, request.verifier_pending_proof_id);
        assert!(request.cast_wallet_sign_command.contains(&live_proof_digest));
        assert!(!request.cast_wallet_sign_command.contains(&generic_proof_digest));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_full_resume_writes_strict_summary() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-full-resume-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let artifact_dir = root.join("roundtrip");
        let topology_file = root.join("topology.json");
        let key_file = root.join("validator.key.json");
        let issuer_key_file = root.join("issuer.key.json");
        let owner_key_file = root.join("owner.key.json");
        let proposer_key_file = root.join("proposer.key.json");
        let finalizer_key_file = root.join("finalizer.key.json");
        let claimer_key_file = root.join("claimer.key.json");
        let signatures_file = root.join("signatures.json");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&artifact_dir).expect("create artifact root");
        for key in [
            &key_file,
            &issuer_key_file,
            &owner_key_file,
            &proposer_key_file,
            &finalizer_key_file,
            &claimer_key_file,
        ] {
            std::fs::write(key, "{}\n").expect("write key placeholder");
        }
        write_json_file(
            &signatures_file,
            &vec![format!("0x{}", "11".repeat(65))],
        )
        .expect("write signatures");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");
        let final_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("final status");
        let public_rpc_listener =
            TcpListener::bind(("127.0.0.1", 0)).expect("bind fake public RPC");
        let public_rpc_port = public_rpc_listener
            .local_addr()
            .expect("fake public RPC addr")
            .port();
        write_local_topology(TopologyOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            validators: 1,
            base_port: public_rpc_port
                .checked_sub(1)
                .expect("fake public RPC port has p2p predecessor"),
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
        })
        .expect("write local topology with fake public RPC port");

        let nav_asset = "aa".repeat(48);
        let pfusdc = "87".repeat(48);
        let issuer = "pf65c9783ceafc0f519a74195e78cc7909f92429c3";
        let bridge_proposer = "pf1111111111111111111111111111111111111111";
        let bridge_attestor = "pf2222222222222222222222222222222222222222";
        let bridge_finalizer = "pf3333333333333333333333333333333333333333";
        let owner = "pf07381735ddb7de134e8be8402b465c9cd8ec7546";
        let vault = "0x1111111111111111111111111111111111111111";
        let verifier = "0x2222222222222222222222222222222222222222";
        let usdc = "0x3333333333333333333333333333333333333333";
        let wallet = "0x4444444444444444444444444444444444444444";
        let amount_atoms = 5_082_364_u64;
        let expected_delta = 508_236_400_i128;
        let bucket_id = "66".repeat(48);
        let reserve_hash_before = "ab".repeat(48);
        let reserve_hash_after_in = "ac".repeat(48);
        let reserve_hash_after_out = "ad".repeat(48);
        let redemption_id = "77".repeat(48);
        let settlement_receipt_hash = "99".repeat(48);

        let certified_ops = |dir: &std::path::Path, start_height: u64, end_height: u64| {
            std::fs::create_dir_all(dir).expect("create certified ops dir");
            write_json_file(
                &dir.join("peer-certified-mempool-round.report.json"),
                &serde_json::json!({
                    "round": {
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
                            "node_id": final_status.node_id,
                            "block_height": final_status.block_height,
                            "state_root": final_status.state_root
                        },
                        "sends": []
                    }
                }),
            )
            .expect("write strict certified round report");
            serde_json::json!({
                "schema": CERTIFIED_ASSET_OPS_REPORT_SCHEMA,
                "request_schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
                "data_dir": data_dir.display().to_string(),
                "topology_file": topology_file.display().to_string(),
                "artifact_dir": dir.display().to_string(),
                "operation_count": 1,
                "max_transactions": 1,
                "allow_existing_mempool": false,
                "prepare_only": false,
                "batch_only": false,
                "start_height": start_height,
                "start_state_root": "10".repeat(48),
                "start_mempool_pending": 0,
                "end_height": end_height,
                "end_state_root": final_status.state_root,
                "end_mempool_pending": 0,
                "operations": [],
                "batch_file": dir.join("mempool-batch.json").display().to_string(),
                "round_artifact_dir": dir.join("peer-certified-mempool-round").display().to_string(),
                "round_ok": true,
                "timings_ms": {
                    "total_ms": 1.0,
                    "preflight_ms": 0.1,
                    "operations_ms": 0.2,
                    "certify_ms": 0.6,
                    "final_status_ms": 0.1
                }
            })
        };
        let certified_ops_same_round_candidate_with_labels =
            |dir: &std::path::Path,
             start_height: u64,
             end_height: u64,
             operation_label: &str,
             depends_on_label: &str,
             reason: &str| {
                let mut value = certified_ops(dir, start_height, end_height);
                value["operation_count"] = serde_json::json!(2);
                value["max_transactions"] = serde_json::json!(2);
                value["dependency_report"] = serde_json::json!({
                    "declared_dependency_count": 1,
                    "same_round_dependency_count": 1,
                    "prior_round_dependency_count": 0,
                    "same_round_batch_eligible": true,
                    "replay_equivalence_required": true,
                    "live_round_compression_ready": false,
                    "live_round_compression_blockers": [
                        "same_round dependency candidates require replay-equivalence corpus evidence before live round compression"
                    ],
                    "declarations": [{
                        "operation": operation_label,
                        "depends_on": depends_on_label,
                        "mode": "same_round",
                        "reason": reason
                    }]
                });
                value
            };
        let certified_ops_same_round_candidate =
            |dir: &std::path::Path, start_height: u64, end_height: u64| {
                certified_ops_same_round_candidate_with_labels(
                    dir,
                    start_height,
                    end_height,
                    "nav-mint-at-nav",
                    "nav-subscription-allocate",
                    "allocation id is deterministic from signed input",
                )
            };
        let status_value = serde_json::json!({
            "schema": postfiat_node::VAULT_BRIDGE_STATUS_REPORT_SCHEMA,
            "asset_id": pfusdc,
            "issuer": issuer,
            "proof_profile": "test",
            "valuation_unit": "USDC",
            "finalized_epoch": 3,
            "nav_per_unit": 1_000_000_u64,
            "circulating_supply": amount_atoms,
            "finalized_reserve_packet_hash": reserve_hash_before,
            "issued_supply_atoms": amount_atoms,
            "counted_value_atoms": amount_atoms,
            "unallocated_counted_capacity_atoms": amount_atoms,
            "source_root": "13".repeat(48),
            "bucket_count": 1,
            "receipt_count": 1,
            "bridge_deposit_count": 1,
            "allocation_count": 1,
            "redemption_count": 1,
            "buckets": [],
            "receipts": [],
            "bridge_deposits": [],
            "allocations": [],
            "redemptions": [],
            "disclosure": "test"
        });

        let write_stage = |relative: &str, file: &str, value: serde_json::Value| {
            let dir = artifact_dir.join(relative);
            std::fs::create_dir_all(&dir).expect("create stage dir");
            write_json_file(&dir.join(file), &value).expect("write stage artifact");
        };

        write_stage(
            "flow0-preflight",
            "preflight.json",
            serde_json::json!({
                "schema": NAV_ROUNDTRIP_PREFLIGHT_REPORT_SCHEMA,
                "artifact_file": artifact_dir.join("flow0-preflight/preflight.json").display().to_string(),
                "source_rpc_url": "https://arb.example.invalid/rpc",
                "vault_address": vault,
                "verifier_address": verifier,
                "usdc_address": usdc,
                "stakehub_wallet": wallet,
                "amount_atoms": amount_atoms,
                "min_gas_wei": "1000000000000000",
                "start_height": 0,
                "start_state_root": final_status.state_root,
                "start_mempool_pending": 0,
                "wallet_usdc_atoms": "10000000",
                "vault_usdc_atoms": "1000000",
                "usdc_allowance_atoms": amount_atoms.to_string(),
                "wallet_gas_wei": "1000000000000000",
                "vault_code_bytes": 100,
                "verifier_code_bytes": 100,
                "vault_challenge_delay_seconds": 0,
                "vault_execution_window_seconds": 3600,
                "verifier_challenge_delay_seconds": 0,
                "verifier_execution_window_seconds": 3600,
                "bridge_class": NAV_ROUNDTRIP_BRIDGE_CLASS_CONTROLLED_LAUNCH,
                "withdrawal_digest_signature": NAV_ROUNDTRIP_OLD_WITHDRAWAL_DIGEST_SIGNATURE,
                "submit_withdrawal_signature": NAV_ROUNDTRIP_OLD_SUBMIT_WITHDRAWAL_SIGNATURE,
                "preflight_ok": true,
                "failure_reasons": []
            }),
        );
        write_stage(
            "flow1-evm-deposit",
            "evm-deposit.json",
            serde_json::json!({
                "schema": NAV_ROUNDTRIP_EVM_DEPOSIT_REPORT_SCHEMA,
                "artifact_file": artifact_dir.join("flow1-evm-deposit/evm-deposit.json").display().to_string(),
                "source_rpc_url": "https://arb.example.invalid/rpc",
                "source_chain_id": 42161,
                "vault_address": vault,
                "usdc_address": usdc,
                "stakehub_wallet": wallet,
                "pftl_recipient": owner,
                "amount_atoms": amount_atoms,
                "nonce": "0x".to_string() + &"01".repeat(32),
                "session_id": "nav-roundtrip-test",
                "wallet_usdc_before_atoms": "10000000",
                "wallet_usdc_after_atoms": (10_000_000_u64 - amount_atoms).to_string(),
                "vault_usdc_before_atoms": "1000000",
                "vault_usdc_after_atoms": (1_000_000_u64 + amount_atoms).to_string(),
                "launch_session_managed_externally": true,
                "allowance_before_atoms": amount_atoms.to_string(),
                "approve_skipped": true,
                "approve_tx": null,
                "approve_gas_used": null,
                "deposit_tx": "0x".to_string() + &"02".repeat(32),
                "deposit_gas_used": 60000,
                "approve_calldata_file": "approve.calldata.txt",
                "deposit_calldata_file": "deposit.calldata.txt",
                "agent_open_session_file": "agent-open-session.json",
                "agent_approve_file": "agent-approve.json",
                "agent_deposit_file": "agent-deposit.json",
                "agent_close_session_file": "agent-close-session.json",
                "delta_ok": true,
                "failure_reasons": []
            }),
        );
        write_stage(
            "flow2-deposit-relay",
            "deposit-relay.json",
            serde_json::json!({
                "schema": NAV_ROUNDTRIP_DEPOSIT_RELAY_REPORT_SCHEMA,
                "artifact_file": artifact_dir.join("flow2-deposit-relay/deposit-relay.json").display().to_string(),
                "evm_deposit_report_file": artifact_dir.join("flow1-evm-deposit/evm-deposit.json").display().to_string(),
                "deposit_tx": "0x".to_string() + &"02".repeat(32),
                "relay_bundle_dir": artifact_dir.join("flow2-deposit-relay/deposit-relay-bundle").display().to_string(),
                "certified_ops_file": artifact_dir.join("flow2-deposit-relay/deposit-relay.certified-ops.json").display().to_string(),
                "certified_ops_artifact_dir": artifact_dir.join("flow2-deposit-relay/deposit-relay-certified").display().to_string(),
                "relay_bundle": {
                    "ok": true,
                    "relay_bundle": {
                        "plan": {
                            "policy_hash": "85".repeat(48),
                            "propose_operation": {
                                "proposer": bridge_proposer,
                                "expires_at_height": 1000
                            },
                            "attest_operation": {
                                "attestor": bridge_attestor
                            },
                            "finalize_operation": {
                                "finalizer": bridge_finalizer
                            }
                        }
                    }
                },
                "certified_ops_stages": [
                    certified_ops_same_round_candidate_with_labels(
                        &artifact_dir.join("flow2-deposit-relay/deposit-relay-certified/propose-attest"),
                        1,
                        2,
                        "attest",
                        "propose",
                        "attestation signs the same deterministic bridge evidence proposed in this round",
                    ),
                    certified_ops(&artifact_dir.join("flow2-deposit-relay/deposit-relay-certified/finalize-claim"), 2, 3),
                    certified_ops_same_round_candidate_with_labels(
                        &artifact_dir.join("flow2-deposit-relay/deposit-relay-certified/receipt"),
                        3,
                        4,
                        "receipt-count",
                        "receipt-submit",
                        "receipt id is deterministic from the submitted bridge-deposit evidence",
                    )
                ],
                "certified_ops": certified_ops(&artifact_dir.join("flow2-deposit-relay/deposit-relay-certified"), 1, 2)
            }),
        );
        write_stage(
            "flow3-primary-mint",
            "primary-mint.json",
            serde_json::json!({
                "schema": NAV_ROUNDTRIP_PRIMARY_MINT_REPORT_SCHEMA,
                "artifact_file": artifact_dir.join("flow3-primary-mint/primary-mint.json").display().to_string(),
                "deposit_relay_report_file": artifact_dir.join("flow2-deposit-relay/deposit-relay.json").display().to_string(),
                "nav_asset_id": nav_asset,
                "settlement_asset_id": pfusdc,
                "issuer": issuer,
                "subscriber": owner,
                "nav_epoch": 2,
                "nav_reserve_packet_hash": reserve_hash_before,
                "nav_per_unit": 508236346_u64,
                "nav_valuation_unit": "usd_1e8",
                "settlement_valuation_unit": "USDC",
                "settlement_asset_precision": 6,
                "mint_amount": 1,
                "settlement_amount_atoms": amount_atoms,
                "settlement_receipt_id": "receipt-1",
                "settlement_bucket_id": bucket_id,
                "settlement_allocation_id": "allocation-1",
                "matched_deposit_tx": "02".repeat(32),
                "settlement_status_before": status_value,
                "settlement_status_after": status_value,
                "operations_file": "primary-mint.certified-ops.json",
                "allocate_operation_file": "nav-subscription-allocate.operation.json",
                "mint_operation_file": "nav-mint-at-nav.operation.json",
                "certified_ops_artifact_dir": artifact_dir.join("flow3-primary-mint/primary-mint-certified").display().to_string(),
                "certified_ops": certified_ops_same_round_candidate(&artifact_dir.join("flow3-primary-mint/primary-mint-certified"), 2, 3)
            }),
        );
        for (flow, epoch_before, epoch_after, before_vna, after_vna, expected, reserve_hash) in [
            ("flow4-nav-money-in", 2_u64, 3_u64, 2_032_945_386_170_u64, 2_033_453_622_570_u64, expected_delta, reserve_hash_after_in.clone()),
            ("flow6-nav-money-out", 3_u64, 4_u64, 2_033_453_622_570_u64, 2_032_945_386_170_u64, -expected_delta, reserve_hash_after_out.clone()),
        ] {
            write_stage(
                flow,
                "nav-checkpoint.json",
                serde_json::json!({
                    "schema": NAV_ROUNDTRIP_NAV_CHECKPOINT_REPORT_SCHEMA,
                    "artifact_file": artifact_dir.join(flow).join("nav-checkpoint.json").display().to_string(),
                    "nav_asset_id": nav_asset,
                    "issuer": issuer,
                    "submitter": issuer,
                    "verifier_kind": "sp1",
                    "source_class": "sp1",
                    "epoch_before": epoch_before,
                    "epoch_after": epoch_after,
                    "checkpoint_epoch": epoch_after,
                    "reserve_packet_hash_before": reserve_hash_before,
                    "reserve_packet_hash_after": reserve_hash,
                    "reserve_packet_hash": reserve_hash,
                    "nav_per_unit_before": 508236346_u64,
                    "nav_per_unit_after": 508363405_u64,
                    "nav_per_unit": 508363405_u64,
                    "circulating_supply_before": 4000_u64,
                    "circulating_supply_after": 4000_u64,
                    "circulating_supply": 4000_u64,
                    "verified_net_assets_before": before_vna,
                    "verified_net_assets_after": after_vna,
                    "verified_net_assets": after_vna,
                    "verified_net_assets_delta": expected,
                    "expected_verified_net_assets_delta": expected,
                    "delta_ok": true,
                    "source_root": "21".repeat(48),
                    "attestor_root": "22".repeat(48),
                    "overlay_value_nav_units": expected_delta.unsigned_abs() as u64,
                    "overlay_source_root": "23".repeat(48),
                    "sp1_base_verified_net_assets": 2_032_945_386_170_u64,
                    "submit_operation_file": "nav-reserve-submit.operation.json",
                    "finalize_operation_file": "nav-epoch-finalize.operation.json",
                    "submit_operations_file": "nav-checkpoint-submit.certified-ops.json",
                    "finalize_operations_file": "nav-checkpoint-finalize.certified-ops.json",
                    "submit_certified_ops_artifact_dir": artifact_dir.join(flow).join("nav-checkpoint-submit-certified").display().to_string(),
                    "finalize_certified_ops_artifact_dir": artifact_dir.join(flow).join("nav-checkpoint-finalize-certified").display().to_string(),
                    "submit_certified_ops": certified_ops(&artifact_dir.join(flow).join("nav-checkpoint-submit-certified"), 3, 4),
                    "finalize_certified_ops": certified_ops(&artifact_dir.join(flow).join("nav-checkpoint-finalize-certified"), 4, 5),
                    "failure_reasons": []
                }),
            );
        }
        write_stage(
            "flow5-nav-exit",
            "nav-exit.json",
            serde_json::json!({
                "schema": NAV_ROUNDTRIP_NAV_EXIT_REPORT_SCHEMA,
                "artifact_file": artifact_dir.join("flow5-nav-exit/nav-exit.json").display().to_string(),
                "primary_mint_report_file": artifact_dir.join("flow3-primary-mint/primary-mint.json").display().to_string(),
                "nav_asset_id": nav_asset,
                "settlement_asset_id": pfusdc,
                "owner": owner,
                "issuer": issuer,
                "nav_epoch": 3,
                "nav_reserve_packet_hash": reserve_hash_after_in,
                "redeem_amount": 1_u64,
                "settlement_amount_atoms": amount_atoms,
                "settlement_bucket_id": bucket_id,
                "settlement_allocation_id": "allocation-1",
                "settlement_receipt_hash": settlement_receipt_hash,
                "redemption_id": redemption_id,
                "nav_balance_before": 1_u64,
                "nav_balance_after": 0_u64,
                "settlement_balance_before": 0_u64,
                "settlement_balance_after": amount_atoms,
                "settlement_status_before": status_value,
                "settlement_status_after": status_value,
                "redeem_operations_file": "nav-exit-redeem.certified-ops.json",
                "redeem_operation_file": "nav-redeem-at-nav.operation.json",
                "redeem_certified_ops_artifact_dir": artifact_dir.join("flow5-nav-exit/nav-exit-redeem-certified").display().to_string(),
                "redeem_certified_ops": certified_ops(&artifact_dir.join("flow5-nav-exit/nav-exit-redeem-certified"), 5, 6),
                "settle_operations_file": "nav-exit-settle.certified-ops.json",
                "settle_operation_file": "nav-redeem-settle.operation.json",
                "settle_certified_ops_artifact_dir": artifact_dir.join("flow5-nav-exit/nav-exit-settle-certified").display().to_string(),
                "settle_certified_ops": certified_ops(&artifact_dir.join("flow5-nav-exit/nav-exit-settle-certified"), 6, 7)
            }),
        );

        let burn_operation =
            postfiat_types::AssetTransactionOperation::VaultBridgeBurnToRedeem(
                postfiat_types::VaultBridgeBurnToRedeemOperation {
                    owner: owner.to_string(),
                    issuer: issuer.to_string(),
                    asset_id: pfusdc.clone(),
                    bucket_id: bucket_id.clone(),
                    amount_atoms,
                    epoch: 3,
                    reserve_packet_hash: reserve_hash_before.clone(),
                    destination_ref: format!("evm-erc20:42161:{wallet}"),
                },
            );
        write_stage(
            "flow7-burn-to-redeem",
            "burn-to-redeem.json",
            serde_json::json!({
                "schema": NAV_ROUNDTRIP_BURN_TO_REDEEM_REPORT_SCHEMA,
                "artifact_file": artifact_dir.join("flow7-burn-to-redeem/burn-to-redeem.json").display().to_string(),
                "nav_exit_report_file": artifact_dir.join("flow5-nav-exit/nav-exit.json").display().to_string(),
                "settlement_asset_id": pfusdc,
                "owner": owner,
                "amount_atoms": amount_atoms,
                "destination_ref": format!("evm-erc20:42161:{wallet}"),
                "owner_balance_before": amount_atoms,
                "owner_balance_after": 0_u64,
                "redemption_id": redemption_id,
                "settlement_status_before": status_value,
                "settlement_status_after": status_value,
                "bundle_dir": artifact_dir.join("flow7-burn-to-redeem/burn-to-redeem-bundle").display().to_string(),
                "bundle": {
                    "schema": postfiat_node::VAULT_BRIDGE_BURN_TO_REDEEM_BUNDLE_SCHEMA,
                    "bundle_dir": artifact_dir.join("flow7-burn-to-redeem/burn-to-redeem-bundle").display().to_string(),
                    "operation_file": "burn-to-redeem.operation.json",
                    "commands_file": "commands.sh",
                    "owner": owner,
                    "issuer": issuer,
                    "asset_id": pfusdc,
                    "bucket_id": bucket_id,
                    "amount_atoms": amount_atoms,
                    "epoch": 3_u64,
                    "reserve_packet_hash": reserve_hash_before,
                    "destination_ref": format!("evm-erc20:42161:{wallet}"),
                    "operation": burn_operation,
                    "commands": [],
                    "trust_boundary": "test"
                },
                "certified_ops_file": "burn-to-redeem.certified-ops.json",
                "certified_ops_artifact_dir": artifact_dir.join("flow7-burn-to-redeem/burn-to-redeem-certified").display().to_string(),
                "certified_ops": certified_ops(&artifact_dir.join("flow7-burn-to-redeem/burn-to-redeem-certified"), 7, 8)
            }),
        );
        let evm_withdrawal_dir = artifact_dir.join("flow8-evm-withdrawal");
        std::fs::create_dir_all(&evm_withdrawal_dir).expect("create EVM withdrawal dir");
        write_json_file(
            &evm_withdrawal_dir.join("evm-withdrawal.json"),
            &NavRoundtripEvmWithdrawalReport {
                schema: NAV_ROUNDTRIP_EVM_WITHDRAWAL_REPORT_SCHEMA.to_string(),
                artifact_file: evm_withdrawal_dir
                    .join("evm-withdrawal.json")
                    .display()
                    .to_string(),
                burn_to_redeem_report_file: artifact_dir
                    .join("flow7-burn-to-redeem/burn-to-redeem.json")
                    .display()
                    .to_string(),
                source_rpc_url: "https://arb.example.invalid/rpc".to_string(),
                source_rpc_provider_class: "public_or_unknown_http".to_string(),
                source_chain_id: 42161,
                bridge_class: NAV_ROUNDTRIP_BRIDGE_CLASS_CONTROLLED_LAUNCH.to_string(),
                vault_address: vault.to_string(),
                verifier_address: verifier.to_string(),
                usdc_address: usdc.to_string(),
                stakehub_wallet: wallet.to_string(),
                settlement_asset_id: pfusdc.clone(),
                redemption_id: redemption_id.clone(),
                amount_atoms,
                pftl_finalized_height: 8,
                pftl_withdrawal_hash: "0x".to_string() + &"31".repeat(32),
                pftl_withdrawal_hash_commitment: "0x".to_string() + &"32".repeat(32),
                withdrawal_packet_digest: "0x".to_string() + &"33".repeat(32),
                verifier_pending_proof_id: "0x".to_string() + &"34".repeat(32),
                verifier_proof_digest_to_sign: "0x".to_string() + &"35".repeat(32),
                vault_pending_withdrawal_id: "0x".to_string() + &"36".repeat(32),
                verifier_challenge_wait_secs: 0,
                vault_challenge_wait_secs: 0,
                session_id: "nav-roundtrip-test".to_string(),
                wallet_usdc_before_atoms: "4917636".to_string(),
                wallet_usdc_after_atoms: "10000000".to_string(),
                vault_usdc_before_atoms: (1_000_000_u64 + amount_atoms).to_string(),
                vault_usdc_after_atoms: "1000000".to_string(),
                launch_session_managed_externally: false,
                submit_proof_tx: "0x".to_string() + &"41".repeat(32),
                submit_proof_gas_used: 50_000,
                finalize_proof_tx: "0x".to_string() + &"42".repeat(32),
                finalize_proof_gas_used: 50_000,
                submit_withdrawal_tx: "0x".to_string() + &"43".repeat(32),
                submit_withdrawal_gas_used: 50_000,
                finalize_withdrawal_tx: "0x".to_string() + &"44".repeat(32),
                finalize_withdrawal_gas_used: 50_000,
                claim_withdrawal_tx: "0x".to_string() + &"45".repeat(32),
                claim_withdrawal_gas_used: 50_000,
                submit_proof_calldata_file: "submit-proof.calldata.txt".to_string(),
                finalize_proof_calldata_file: "finalize-proof.calldata.txt".to_string(),
                submit_withdrawal_calldata_file: "submit-withdrawal.calldata.txt".to_string(),
                finalize_withdrawal_calldata_file: "finalize-withdrawal.calldata.txt".to_string(),
                claim_withdrawal_calldata_file: "claim-withdrawal.calldata.txt".to_string(),
                agent_open_session_file: "agent-open-session.json".to_string(),
                agent_submit_proof_file: "agent-submit-proof.json".to_string(),
                agent_finalize_proof_file: "agent-finalize-proof.json".to_string(),
                agent_submit_withdrawal_file: "agent-submit-withdrawal.json".to_string(),
                agent_finalize_withdrawal_file: "agent-finalize-withdrawal.json".to_string(),
                agent_claim_withdrawal_file: "agent-claim-withdrawal.json".to_string(),
                agent_close_session_file: "agent-close-session.json".to_string(),
                receipt_watches: Vec::new(),
                delta_ok: true,
                failure_reasons: Vec::new(),
            },
        )
        .expect("write EVM withdrawal report");
        let final_certified_dir = artifact_dir.join("flow9-pftl-settle/pftl-settle-certified");
        write_stage(
            "flow9-pftl-settle",
            "pftl-settle.json",
            serde_json::json!({
                "schema": NAV_ROUNDTRIP_PFTL_SETTLE_REPORT_SCHEMA,
                "artifact_file": artifact_dir.join("flow9-pftl-settle/pftl-settle.json").display().to_string(),
                "evm_withdrawal_report_file": artifact_dir.join("flow8-evm-withdrawal/evm-withdrawal.json").display().to_string(),
                "settlement_asset_id": pfusdc,
                "issuer_or_redemption_account": issuer,
                "redemption_id": redemption_id,
                "settlement_receipt_hash": "55".repeat(48),
                "settled_atoms": amount_atoms,
                "redemption_state_before": postfiat_types::VAULT_BRIDGE_REDEMPTION_STATE_PENDING,
                "redemption_state_after": postfiat_types::VAULT_BRIDGE_REDEMPTION_STATE_SETTLED,
                "redemption_queue_before_atoms": amount_atoms,
                "redemption_queue_after_atoms": 0_u64,
                "counted_value_before_atoms": amount_atoms,
                "counted_value_after_atoms": 0_u64,
                "operation_file": "vault-bridge-redeem-settle.operation.json",
                "operations_file": "pftl-settle.certified-ops.json",
                "certified_ops_artifact_dir": final_certified_dir.display().to_string(),
                "certified_ops": certified_ops(&final_certified_dir, 8, final_status.block_height),
                "accounting_ok": true,
                "failure_reasons": []
            }),
        );
        std::fs::create_dir_all(&final_certified_dir).expect("create final certified dir");
        write_json_file(
            &final_certified_dir.join("peer-certified-mempool-round.report.json"),
            &serde_json::json!({
                "round": {
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
                        "node_id": final_status.node_id,
                        "block_height": final_status.block_height,
                        "state_root": final_status.state_root
                    },
                    "sends": []
                }
            }),
        )
        .expect("write final round report");

        let final_status_for_rpc = final_status.clone();
        let rpc_thread = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = public_rpc_listener.accept().expect("accept fake public RPC");
                set_stream_timeout(&stream, 5_000).expect("set fake RPC timeout");
                let line = read_transport_line(&stream, "fake public RPC request read")
                    .expect("read fake public RPC request");
                let request: RpcRequest =
                    serde_json::from_str(&line).expect("parse fake public RPC request");
                assert_eq!("status", request.method);
                let response = success_response(
                    &request.id,
                    &final_status_for_rpc,
                    vec![RpcEvent::new("status", "validator-0", "status queried")],
                )
                .expect("fake public RPC response");
                write_json_line(&mut stream, &response).expect("write fake public RPC response");
            }
        });

        run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--data-dir".to_string(),
            data_dir.display().to_string(),
            "--topology".to_string(),
            topology_file.display().to_string(),
            "--key-file".to_string(),
            key_file.display().to_string(),
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--source-rpc-url".to_string(),
            "https://arb.example.invalid/rpc".to_string(),
            "--vault".to_string(),
            vault.to_string(),
            "--verifier".to_string(),
            verifier.to_string(),
            "--usdc".to_string(),
            usdc.to_string(),
            "--stakehub-wallet".to_string(),
            wallet.to_string(),
            "--nav-asset".to_string(),
            nav_asset.clone(),
            "--pfusdc".to_string(),
            pfusdc.clone(),
            "--policy-hash".to_string(),
            "85".repeat(48),
            "--pftl-recipient".to_string(),
            owner.to_string(),
            "--proposer".to_string(),
            issuer.to_string(),
            "--finalizer".to_string(),
            issuer.to_string(),
            "--claimer".to_string(),
            owner.to_string(),
            "--proposer-key-file".to_string(),
            proposer_key_file.display().to_string(),
            "--finalizer-key-file".to_string(),
            finalizer_key_file.display().to_string(),
            "--claimer-key-file".to_string(),
            claimer_key_file.display().to_string(),
            "--issuer-key-file".to_string(),
            issuer_key_file.display().to_string(),
            "--owner-key-file".to_string(),
            owner_key_file.display().to_string(),
            "--amount-atoms".to_string(),
            amount_atoms.to_string(),
            "--mint-amount".to_string(),
            "1".to_string(),
            "--nonce".to_string(),
            "0x".to_string() + &"01".repeat(32),
            "--session-id".to_string(),
            "nav-roundtrip-test".to_string(),
            "--signatures-file".to_string(),
            signatures_file.display().to_string(),
            "--expires-at-height".to_string(),
            "100".to_string(),
            "--resume".to_string(),
        ])
        .expect("full roundtrip resume cli");
        rpc_thread.join().expect("fake public RPC thread");

        let summary_raw = std::fs::read_to_string(artifact_dir.join("roundtrip-summary.json"))
            .expect("read roundtrip summary");
        let summary_value: serde_json::Value =
            serde_json::from_str(&summary_raw).expect("parse roundtrip summary value");
        let report =
            serde_json::from_str::<NavRoundtripLiveDemoReport>(&summary_raw)
                .expect("parse roundtrip summary");
        assert_eq!(NAV_ROUNDTRIP_LIVE_DEMO_REPORT_SCHEMA, report.schema);
        assert_eq!(
            NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP,
            report.run_class
        );
        assert_eq!(
            NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP,
            report.completion_status
        );
        assert_eq!(
            NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC,
            report.custody_location
        );
        assert_eq!(
            "full_arbitrum_roundtrip_protocol_clock_with_blocking_safety_checks",
            report.timing_scope
        );
        assert_eq!("evm_deposit", report.protocol_clock_started_at_stage);
        assert_eq!("final_verification", report.protocol_clock_stopped_at_stage);
        assert!(report.setup_or_recovery_work_included_in_total);
        assert!(report.final_summary_ok, "{:?}", report.failure_reasons);
        assert_eq!(
            Some("resume_existing_evm_artifacts_no_session"),
            report.stakehub_launch_session_mode.as_deref()
        );
        assert_eq!("conservative_blocking", report.preflight_profile);
        assert_eq!(
            "public_or_unknown_http",
            report.source_rpc_provider_class
        );
        assert!(!report.background_audit_enabled);
        assert_eq!("blocking_public_rpc", report.final_audit_profile);
        assert!(report.background_audit_request_file.is_none());
        assert!(report.stakehub_launch_session_open_file.is_none());
        assert!(report.stakehub_launch_session_close_file.is_none());
        assert_eq!(12, report.pftl_certified_round_count);
        assert_eq!(15, report.pftl_certified_operation_count);
        assert_eq!(12, report.pftl_certified_rounds.len());
        assert_eq!(3, report.pftl_replay_equivalence_required_count);
        assert_eq!(
            vec![
                "nav_subscription_allocate_mint_at_nav".to_string(),
                "vault_bridge_deposit_propose_attest".to_string(),
                "vault_bridge_receipt_submit_count".to_string()
            ],
            report.pftl_candidate_batch_classes
        );
        assert!(!report.pftl_live_round_compression_ready);
        assert_eq!(3, report.pftl_live_round_compression_blockers.len());
        assert!(report
            .pftl_live_round_compression_blockers
            .iter()
            .any(|blocker| blocker.contains("deposit_relay/stage-1")));
        assert!(report
            .pftl_live_round_compression_blockers
            .iter()
            .any(|blocker| blocker.contains("deposit_relay/stage-3")));
        assert!(report
            .pftl_live_round_compression_blockers
            .iter()
            .any(|blocker| blocker.contains("primary_mint/batch")));
        assert!(report
            .pftl_live_round_compression_blockers
            .iter()
            .all(|blocker| blocker.contains("replay-equivalence corpus evidence")));
        assert!(report
            .pftl_certified_rounds
            .iter()
            .any(|round| round.stage == "nav_money_in" && round.round == "reserve_submit"));
        assert!(report
            .pftl_certified_rounds
            .iter()
            .all(|round| round.round_ok == Some(true)));
        assert_eq!(expected_delta, report.expected_money_in_vna_delta);
        assert_eq!(-expected_delta, report.expected_money_out_vna_delta);
        assert_eq!(Some(expected_delta), report.nav_money_in.verified_net_assets_delta);
        assert_eq!(Some(-expected_delta), report.nav_money_out.verified_net_assets_delta);
        assert!(report.final_validator_consensus_ok);
        let fleet_preflight = report
            .fleet_preflight
            .as_ref()
            .expect("fleet preflight report");
        assert!(fleet_preflight.preflight_ok);
        assert!(fleet_preflight.public_validator_consensus_ok);
        assert!(fleet_preflight.operator_matches_public_endpoint);
        assert_eq!(1, report.final_validator_states.len());
        assert_eq!(1, report.public_validator_states.len());
        assert_eq!("validator-0", report.public_validator_states[0].node_id);
        assert_eq!(
            Some("validator-0"),
            report
                .operator_local_state
                .as_ref()
                .map(|state| state.node_id.as_str())
        );
        assert_eq!(1, report.certified_round_validator_states.len());
        assert_eq!(final_status.block_height, report.final_height);
        assert_eq!(final_status.state_root, report.final_state_root);
        let timings = summary_value
            .get("timings_ms")
            .and_then(serde_json::Value::as_object)
            .expect("timings object");
        for key in [
            "total_ms",
            "readiness_preflight_ms",
            "protocol_clock_ms",
            "fleet_preflight_ms",
            "preflight_ms",
            "stakehub_session_ms",
            "stakehub_session_close_ms",
            "evm_deposit_ms",
            "deposit_relay_ms",
            "primary_mint_ms",
            "nav_money_in_ms",
            "nav_exit_ms",
            "nav_money_out_ms",
            "burn_to_redeem_ms",
            "withdrawal_signature_ms",
            "evm_withdrawal_ms",
            "pftl_settle_ms",
            "final_verification_ms",
        ] {
            assert!(timings.contains_key(key), "missing timing field {key}");
        }
        assert!(report.timings_ms.total_ms > 0.0);
        assert!(
            ((report.timings_ms.fleet_preflight_ms
                + report.timings_ms.preflight_ms
                + report.timings_ms.stakehub_session_ms)
                - report.timings_ms.readiness_preflight_ms)
                .abs()
                <= 0.000_001
        );
        let segment_sum_ms = report.timings_ms.fleet_preflight_ms
            + report.timings_ms.preflight_ms
            + report.timings_ms.stakehub_session_ms
            + report.timings_ms.stakehub_session_close_ms
            + report.timings_ms.evm_deposit_ms
            + report.timings_ms.deposit_relay_ms
            + report.timings_ms.primary_mint_ms
            + report.timings_ms.nav_money_in_ms
            + report.timings_ms.nav_exit_ms
            + report.timings_ms.nav_money_out_ms
            + report.timings_ms.burn_to_redeem_ms
            + report.timings_ms.withdrawal_signature_ms
            + report.timings_ms.evm_withdrawal_ms
            + report.timings_ms.pftl_settle_ms
            + report.timings_ms.final_verification_ms;
        assert!(
            segment_sum_ms <= report.timings_ms.total_ms + 5.0,
            "segment timings exceed total: segments={segment_sum_ms} total={}",
            report.timings_ms.total_ms
        );
        let protocol_clock_sum_ms = report.timings_ms.evm_deposit_ms
            + report.timings_ms.deposit_relay_ms
            + report.timings_ms.primary_mint_ms
            + report.timings_ms.nav_money_in_ms
            + report.timings_ms.nav_exit_ms
            + report.timings_ms.nav_money_out_ms
            + report.timings_ms.burn_to_redeem_ms
            + report.timings_ms.withdrawal_signature_ms
            + report.timings_ms.evm_withdrawal_ms
            + report.timings_ms.pftl_settle_ms
            + report.timings_ms.final_verification_ms;
        assert!(
            (report.timings_ms.protocol_clock_ms - protocol_clock_sum_ms).abs() <= 0.000_001,
            "protocol clock split drifted: protocol={} sum={protocol_clock_sum_ms}",
            report.timings_ms.protocol_clock_ms
        );
        let summary_file = artifact_dir.join("roundtrip-summary.json");
        let dashboard_status =
            nav_roundtrip_dashboard_status(NavRoundtripDashboardStatusOptions {
                summary_file: summary_file.clone(),
                report_file: None,
            })
            .expect("full roundtrip dashboard status");
        assert!(dashboard_status.full_arbitrum_roundtrip_complete);
        assert!(!dashboard_status.pftl_only_complete);
        assert!(!dashboard_status.bridge_out_deferred);
        assert_eq!(
            "full Arbitrum roundtrip complete",
            dashboard_status.display_status
        );
        assert_eq!(
            Some("full_arbitrum_roundtrip_protocol_clock_with_blocking_safety_checks".to_string()),
            dashboard_status.timing_scope
        );
        assert_eq!(
            Some("evm_deposit".to_string()),
            dashboard_status.protocol_clock_started_at_stage
        );
        assert_eq!(
            Some("final_verification".to_string()),
            dashboard_status.protocol_clock_stopped_at_stage
        );
        assert_eq!(
            Some(true),
            dashboard_status.setup_or_recovery_work_included_in_total
        );
        assert!(dashboard_status.timing_boundary_ok);
        assert!(!dashboard_status.benchmark_clean_timing);
        assert_eq!(
            Some(report.timings_ms.total_ms),
            dashboard_status.total_ms
        );
        assert_eq!(
            Some(report.timings_ms.readiness_preflight_ms),
            dashboard_status.readiness_preflight_ms
        );
        assert_eq!(
            Some(report.timings_ms.protocol_clock_ms),
            dashboard_status.protocol_clock_ms
        );
        assert!(dashboard_status.timings_ms.is_some());
        assert_eq!(
            Some("public_or_unknown_http".to_string()),
            dashboard_status.source_rpc_provider_class
        );
        assert_eq!(
            Some(report.bridge_class.clone()),
            dashboard_status.bridge_class
        );
        assert_eq!(
            Some(false),
            dashboard_status.background_audit_enabled
        );
        assert_eq!(
            Some("blocking_public_rpc".to_string()),
            dashboard_status.final_audit_profile
        );
        assert_eq!(
            Some("public_validator_rpc".to_string()),
            dashboard_status.final_validator_state_source
        );
        assert!(dashboard_status.failure_reasons.is_empty());
        let setup_included_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase1".to_string(),
                summary_file: Some(summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: Some(600_000.0),
                strict_exit: false,
            })
            .expect("phase1 setup/recovery benchmark rejection report");
        assert!(!setup_included_gate.passed);
        assert!(setup_included_gate.failure_reasons.iter().any(|reason| {
            reason.contains("setup_or_recovery_work_included_in_total=true")
        }));
        let mut benchmark_summary_value = summary_value.clone();
        benchmark_summary_value["setup_or_recovery_work_included_in_total"] =
            serde_json::json!(false);
        write_json_file(&summary_file, &benchmark_summary_value)
            .expect("write benchmark-clean summary");
        let mut cold_approval_summary_value = benchmark_summary_value.clone();
        cold_approval_summary_value["preflight"]["usdc_allowance_atoms"] =
            serde_json::json!("0");
        cold_approval_summary_value["evm_deposit"]["allowance_before_atoms"] =
            serde_json::json!("0");
        cold_approval_summary_value["evm_deposit"]["approve_skipped"] =
            serde_json::json!(false);
        cold_approval_summary_value["evm_deposit"]["approve_tx"] =
            serde_json::json!("0x".to_string() + &"01".repeat(32));
        cold_approval_summary_value["evm_deposit"]["approve_gas_used"] =
            serde_json::json!(50_000);
        write_json_file(&summary_file, &cold_approval_summary_value)
            .expect("write cold-approval summary");
        let cold_approval_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase1".to_string(),
                summary_file: Some(summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: Some(600_000.0),
                strict_exit: false,
            })
            .expect("phase1 cold approval benchmark rejection report");
        assert!(!cold_approval_gate.passed);
        assert!(cold_approval_gate.failure_reasons.iter().any(|reason| {
            reason.contains("EVM deposit included USDC approval")
        }));
        assert!(cold_approval_gate.failure_reasons.iter().any(|reason| {
            reason.contains("preflight USDC allowance 0 atoms is below benchmark amount")
        }));
        write_json_file(&summary_file, &benchmark_summary_value)
            .expect("restore benchmark-clean summary");
        let phase1_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase1".to_string(),
                summary_file: Some(summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: Some(600_000.0),
                strict_exit: false,
            })
            .expect("phase1 benchmark verification");
        assert!(phase1_gate.passed, "{:?}", phase1_gate.failure_reasons);
        assert_eq!(1, phase1_gate.run_count);
        assert_eq!(1, phase1_gate.clean_run_count);
        assert_eq!(
            vec![artifact_dir.display().to_string()],
            phase1_gate.artifact_roots
        );
        assert!(phase1_gate
            .clean_run_definition
            .contains("full Arbitrum roundtrip only"));
        assert!(
            phase1_gate.provenance.failure_reasons.is_empty(),
            "{:?}",
            phase1_gate.provenance.failure_reasons
        );
        assert!(!phase1_gate.provenance.package_version.is_empty());
        assert!(phase1_gate.provenance.binary_path.is_some());
        assert_eq!(
            96,
            phase1_gate
                .provenance
                .binary_sha3_384
                .as_deref()
                .expect("binary hash")
                .len()
        );
        assert_eq!(
            40,
            phase1_gate
                .provenance
                .git_commit
                .as_deref()
                .expect("git commit")
                .len()
        );
        assert!(phase1_gate.provenance.git_dirty.is_some());
        assert!(phase1_gate
            .provenance
            .git_status_porcelain_line_count
            .is_some());
        assert_eq!(phase1_gate.average_ms, phase1_gate.mean_ms);
        assert_eq!("protocol_clock_ms", phase1_gate.benchmark_runtime_metric);
        assert_eq!(vec![report.timings_ms.total_ms], phase1_gate.total_ms_values);
        assert_eq!(
            vec![report.timings_ms.readiness_preflight_ms],
            phase1_gate.readiness_preflight_ms_values
        );
        assert_eq!(
            vec![report.timings_ms.protocol_clock_ms],
            phase1_gate.protocol_clock_ms_values
        );
        assert_eq!(
            Some(report.timings_ms.protocol_clock_ms),
            phase1_gate.median_ms
        );
        assert_eq!(
            Some(report.timings_ms.protocol_clock_ms),
            phase1_gate.best_ms
        );
        assert_eq!(
            Some(report.timings_ms.protocol_clock_ms),
            phase1_gate.worst_ms
        );
        assert_eq!(
            vec!["public_or_unknown_http".to_string()],
            phase1_gate.source_rpc_provider_classes
        );
        assert_eq!(vec![report.bridge_class.clone()], phase1_gate.bridge_classes);
        assert_eq!(vec![vault.to_string()], phase1_gate.vault_addresses);
        assert_eq!(
            vec![verifier.to_string()],
            phase1_gate.verifier_addresses
        );
        assert_eq!(vec![usdc.to_string()], phase1_gate.usdc_addresses);
        assert_eq!(
            vec![wallet.to_string()],
            phase1_gate.stakehub_wallets
        );
        assert_eq!(vec![0], phase1_gate.vault_challenge_delay_seconds);
        assert_eq!(vec![3600], phase1_gate.vault_execution_window_seconds);
        assert_eq!(vec![0], phase1_gate.verifier_challenge_delay_seconds);
        assert_eq!(
            vec![3600],
            phase1_gate.verifier_execution_window_seconds
        );
        assert_eq!(
            vec!["validator-0".to_string()],
            phase1_gate.final_validator_node_ids
        );
        assert_eq!(3, phase1_gate.slowest_stages.len());
        assert!(phase1_gate
            .slowest_stages
            .windows(2)
            .all(|window| window[0].mean_ms >= window[1].mean_ms));
        for stage in &phase1_gate.slowest_stages {
            assert_eq!(1, stage.sample_count);
            let expected =
                nav_roundtrip_benchmark_stage_timing_value(&report.timings_ms, &stage.stage);
            assert_eq!(expected, stage.mean_ms);
            assert_eq!(Some(expected), stage.median_ms);
            assert_eq!(Some(expected), stage.p90_ms);
            assert_eq!(Some(expected), stage.best_ms);
            assert_eq!(Some(expected), stage.worst_ms);
        }
        assert_eq!(
            report.timings_ms.protocol_clock_ms,
            phase1_gate.summaries[0].protocol_clock_ms
        );
        assert_eq!(
            report.timings_ms.total_ms,
            phase1_gate.summaries[0].timings_ms.total_ms
        );
        assert_eq!(
            report.timings_ms.readiness_preflight_ms,
            phase1_gate.summaries[0].readiness_preflight_ms
        );
        assert_eq!(
            report.timing_scope,
            phase1_gate.summaries[0].timing_scope
        );
        assert!(!phase1_gate.summaries[0].setup_or_recovery_work_included_in_total);
        assert_eq!(
            report.source_rpc_provider_class,
            phase1_gate.summaries[0].source_rpc_provider_class
        );
        assert_eq!(
            report.preflight.vault_challenge_delay_seconds,
            phase1_gate.summaries[0].vault_challenge_delay_seconds
        );
        assert_eq!(
            report.preflight.verifier_challenge_delay_seconds,
            phase1_gate.summaries[0].verifier_challenge_delay_seconds
        );
        assert_eq!(
            report.evm_withdrawal.verifier_challenge_wait_secs,
            phase1_gate.summaries[0].evm_withdrawal_verifier_challenge_wait_secs
        );
        assert_eq!(
            report.evm_withdrawal.vault_challenge_wait_secs,
            phase1_gate.summaries[0].evm_withdrawal_vault_challenge_wait_secs
        );
        assert_eq!(
            report.evm_deposit.approve_skipped,
            phase1_gate.summaries[0].approve_skipped
        );
        assert_eq!(
            report.stakehub_launch_session_mode,
            phase1_gate.summaries[0].stakehub_launch_session_mode
        );
        assert_eq!(
            report.background_audit_enabled,
            phase1_gate.summaries[0].background_audit_enabled
        );
        assert_eq!(
            report.final_audit_profile,
            phase1_gate.summaries[0].final_audit_profile
        );
        assert_eq!(
            report.final_validator_state_source,
            phase1_gate.summaries[0].final_validator_state_source
        );
        assert_eq!(
            report.evm_deposit.wallet_usdc_before_atoms,
            phase1_gate.summaries[0].evm_deposit_wallet_usdc_before_atoms
        );
        assert_eq!(
            report.evm_withdrawal.wallet_usdc_after_atoms,
            phase1_gate.summaries[0].evm_withdrawal_wallet_usdc_after_atoms
        );
        assert_eq!(
            report.nav_money_in.expected_verified_net_assets_delta,
            phase1_gate.summaries[0].nav_money_in_expected_vna_delta
        );
        assert_eq!(
            report.nav_money_in.verified_net_assets_delta,
            phase1_gate.summaries[0].nav_money_in_actual_vna_delta
        );
        assert_eq!(
            report.nav_money_out.expected_verified_net_assets_delta,
            phase1_gate.summaries[0].nav_money_out_expected_vna_delta
        );
        assert_eq!(
            report.pftl_settle.redemption_queue_before_atoms,
            phase1_gate.summaries[0].pftl_redemption_queue_before_atoms
        );
        assert_eq!(
            report.pftl_settle.counted_value_after_atoms,
            phase1_gate.summaries[0].pftl_counted_value_after_atoms
        );
        let phase1_dir_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase1".to_string(),
                summary_file: None,
                benchmark_dir: Some(artifact_dir.clone()),
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: Some(600_000.0),
                strict_exit: false,
            })
            .expect("phase1 benchmark dir verification");
        assert!(phase1_dir_gate.passed, "{:?}", phase1_dir_gate.failure_reasons);
        assert_eq!(vec![summary_file.display().to_string()], phase1_dir_gate.summary_files);
        let base_args_file = root.join("generated-base-args.json");
        let base_args_report_file = root.join("generated-base-args.report.json");
        let base_args_report =
            nav_roundtrip_benchmark_base_args(NavRoundtripBenchmarkBaseArgsOptions {
                summary_file: summary_file.clone(),
                output_file: base_args_file.clone(),
                data_dir: None,
                topology_file: None,
                key_file: key_file.clone(),
                proposal_key_file: None,
                proposer_key_file: proposer_key_file.clone(),
                attestor_key_file: None,
                finalizer_key_file: finalizer_key_file.clone(),
                claimer_key_file: claimer_key_file.clone(),
                issuer_key_file: issuer_key_file.clone(),
                owner_key_file: owner_key_file.clone(),
                settlement_key_file: None,
                submitter_key_file: None,
                withdrawal_signer_key_file: owner_key_file.clone(),
                nonce_base: format!("0x{}", "10".repeat(32)),
                session_id_base: "nav-roundtrip-generated".to_string(),
                timeout_ms: Some(7000),
                send_retries: Some(2),
                retry_backoff_ms: Some(10),
                agent_timeout_secs: Some(30),
                min_gas_wei: Some("1000000000000000".to_string()),
                destination_ref: None,
                overwrite: false,
                report_file: Some(base_args_report_file.clone()),
            })
            .expect("generate benchmark base args");
        assert!(base_args_report.validation_ok);
        assert!(base_args_file.is_file());
        assert!(base_args_report_file.is_file());
        assert!(base_args_report
            .args
            .windows(2)
            .any(|window| window == ["--policy-hash", "858585858585858585858585858585858585858585858585858585858585858585858585858585858585858585858585"]));
        assert!(base_args_report
            .args
            .windows(2)
            .any(|window| window == ["--expires-at-height", "1000"]));
        assert!(base_args_report
            .args
            .windows(2)
            .any(|window| window == ["--proposer", bridge_proposer]));
        assert!(base_args_report
            .args
            .windows(2)
            .any(|window| window == ["--attestor", bridge_attestor]));
        assert!(base_args_report
            .args
            .windows(2)
            .any(|window| window == ["--finalizer", bridge_finalizer]));
        assert!(base_args_report
            .args
            .iter()
            .any(|arg| arg == "--withdrawal-signer-key-file"));
        assert!(base_args_report
            .args
            .iter()
            .any(|arg| arg == "--require-warm-usdc-allowance"));
        let generated_plan =
            nav_roundtrip_benchmark_plan(NavRoundtripBenchmarkPlanOptions {
                phase: "phase1".to_string(),
                base_args_file: base_args_file.clone(),
                benchmark_dir: root.join("generated-benchmark"),
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                run_count: 2,
                run_prefix: "generated-".to_string(),
                binary: "postfiat-node".to_string(),
                max_median_ms: Some(95_000.0),
                max_p90_ms: Some(105_000.0),
                overwrite: false,
            })
            .expect("generated base args feed benchmark plan");
        assert_eq!(2, generated_plan.runs.len());
        assert!(generated_plan
            .allowance_setup_command
            .command
            .iter()
            .any(|arg| arg == "--warm-usdc-allowance-only"));
        assert!(generated_plan.runs[0]
            .run_command
            .command
            .iter()
            .any(|arg| arg == "--fast-demo-preflight"));
        assert!(generated_plan.runs[0]
            .run_command
            .command
            .iter()
            .any(|arg| arg == "--require-warm-usdc-allowance"));
        assert_eq!(
            Some("nav-roundtrip-generated-generated-01"),
            flag_value(&generated_plan.runs[0].run_command.command, "--session-id")
        );
        let phase1_runtime_fail =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase1".to_string(),
                summary_file: Some(summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(0.01),
                max_p90_ms: Some(600_000.0),
                strict_exit: false,
            })
            .expect("phase1 benchmark runtime failure report");
        assert!(!phase1_runtime_fail.passed);
        assert!(phase1_runtime_fail
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("median protocol runtime")));
        let phase1_runtime_fail_cli = run_cli(vec![
            "nav-roundtrip-benchmark-verify".to_string(),
            "--phase".to_string(),
            "phase1".to_string(),
            "--summary".to_string(),
            summary_file.display().to_string(),
            "--min-clean-runs".to_string(),
            "1".to_string(),
            "--max-median-ms".to_string(),
            "0.01".to_string(),
            "--max-p90-ms".to_string(),
            "600000".to_string(),
            "--strict".to_string(),
        ])
        .expect_err("CLI strict benchmark verify must reject failed gate");
        assert!(phase1_runtime_fail_cli.contains("benchmark verification failed"));
        let phase2_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase2".to_string(),
                summary_file: Some(summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: None,
                strict_exit: false,
            })
            .expect("phase2 benchmark verification");
        assert!(!phase2_gate.passed);
        assert!(phase2_gate
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("replay-equivalence closure")));
        assert!(phase2_gate
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("Phase 2 live round compression is not ready")));
        assert!(phase2_gate
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("requires replay corpus evidence")));
        assert_eq!(
            vec![
                "nav_subscription_allocate_mint_at_nav".to_string(),
                "vault_bridge_deposit_propose_attest".to_string(),
                "vault_bridge_receipt_submit_count".to_string()
            ],
            phase2_gate.phase2_summary_candidate_batch_classes
        );
        assert_eq!(
            nav_roundtrip_phase2_default_candidate_classes(),
            phase2_gate.phase2_required_candidate_classes
        );

        let unrelated_phase2_corpus_file = root.join("phase2-unrelated-corpus.json");
        write_json_file(
            &unrelated_phase2_corpus_file,
            &serde_json::json!({
                "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
                "case": "unrelated-live-ready-class",
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
        .expect("write unrelated phase2 corpus");
        let phase2_unrelated_corpus_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase2".to_string(),
                summary_file: Some(summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: Some(unrelated_phase2_corpus_file),
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: None,
                strict_exit: false,
            })
            .expect("phase2 unrelated corpus verification");
        assert!(!phase2_unrelated_corpus_gate.passed);
        let replay_report = phase2_unrelated_corpus_gate
            .replay_corpus_report
            .as_ref()
            .expect("phase2 replay report");
        assert_eq!(
            nav_roundtrip_phase2_default_candidate_classes(),
            replay_report.required_candidate_classes
        );
        assert_eq!(
            nav_roundtrip_phase2_default_candidate_classes(),
            replay_report.missing_required_candidate_classes
        );
        assert!(phase2_unrelated_corpus_gate
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("no live-ready replay corpus case")));

        let phase2_matching_unmutated_corpus_file =
            root.join("phase2-nav-subscription-unmutated-corpus.json");
        write_json_file(
            &phase2_matching_unmutated_corpus_file,
            &serde_json::json!({
                "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
                "case": "nav-subscription-allocation-mint-ledger-facing-equivalent",
                "candidate_batch_class": "nav_subscription_allocate_mint_at_nav",
                "unbatched_block_height": 2,
                "batched_block_height": 1,
                "unbatched_state_root": "ab".repeat(48),
                "batched_state_root": "cd".repeat(48),
                "state_root_match": false,
                "intended_state_root_difference": "unbatched replay commits allocation and mint as two ordered blocks while same-round batching commits one block; ledger-facing accounting state is identical after normalizing retired allocation block-height provenance",
                "ledger_facing_state_match": true,
                "safe_for_live_round_compression": true,
                "gate": "ledger-facing accounting-equivalent primary mint replay",
            }),
        )
        .expect("write matching unmutated phase2 corpus");
        let phase2_primary_only_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase2".to_string(),
                summary_file: Some(summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: Some(phase2_matching_unmutated_corpus_file),
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: None,
                strict_exit: false,
            })
            .expect("phase2 primary-only corpus verification");
        assert!(
            !phase2_primary_only_gate.passed,
            "{:?}",
            phase2_primary_only_gate.failure_reasons
        );
        assert!(
            phase2_primary_only_gate
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("vault_bridge_deposit_propose_attest"))
        );
        assert!(
            phase2_primary_only_gate
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("vault_bridge_receipt_submit_count"))
        );
        assert!(
            phase2_primary_only_gate
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("nav_redeem_at_nav_settle"))
        );

        let phase2_matching_corpus_dir = root.join("phase2-matching-corpus-dir");
        std::fs::create_dir_all(&phase2_matching_corpus_dir).expect("create phase2 corpus dir");
        for (file_name, case_name, candidate_class) in [
            (
                "nav-redeem-at-nav-settle.json",
                "nav-redeem-at-nav-settle-ledger-facing-equivalent",
                "nav_redeem_at_nav_settle",
            ),
            (
                "nav-subscription-allocation-mint.json",
                "nav-subscription-allocation-mint-ledger-facing-equivalent",
                "nav_subscription_allocate_mint_at_nav",
            ),
            (
                "vault-bridge-deposit-propose-attest.json",
                "vault-bridge-deposit-propose-attest-ledger-facing-equivalent",
                "vault_bridge_deposit_propose_attest",
            ),
            (
                "vault-bridge-receipt-submit-count.json",
                "vault-bridge-receipt-submit-count-ledger-facing-equivalent",
                "vault_bridge_receipt_submit_count",
            ),
        ] {
            write_json_file(
                &phase2_matching_corpus_dir.join(file_name),
                &serde_json::json!({
                    "schema": CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA,
                    "case": case_name,
                    "candidate_batch_class": candidate_class,
                    "unbatched_block_height": 2,
                    "batched_block_height": 1,
                    "unbatched_state_root": "ab".repeat(48),
                    "batched_state_root": "cd".repeat(48),
                    "state_root_match": false,
                    "intended_state_root_difference": "unbatched replay commits two ordered blocks while same-round batching commits one block; ledger-facing state is identical after normalizing expected block-height provenance",
                    "ledger_facing_state_match": true,
                    "safe_for_live_round_compression": true,
                    "gate": "ledger-facing accounting-equivalent replay",
                }),
            )
            .expect("write matching corpus case");
        }
        let phase2_unmutated_ready_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase2".to_string(),
                summary_file: Some(summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: Some(phase2_matching_corpus_dir),
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: None,
                strict_exit: false,
            })
            .expect("phase2 unmutated summary corpus-dir verification");
        assert!(
            !phase2_unmutated_ready_gate.passed,
            "{:?}",
            phase2_unmutated_ready_gate.failure_reasons
        );
        assert!(phase2_unmutated_ready_gate
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("missing required candidate batch class")
                && reason.contains("nav_redeem_at_nav_settle")));
        assert_eq!(
            nav_roundtrip_phase2_default_candidate_classes(),
            phase2_unmutated_ready_gate.phase2_required_candidate_classes
        );

        let phase2_ready_summary_file = artifact_dir.join("roundtrip-summary-phase2-ready.json");
        let mut phase2_ready_summary: serde_json::Value =
            serde_json::from_str(&summary_raw).expect("phase2 summary json");
        phase2_ready_summary["setup_or_recovery_work_included_in_total"] =
            serde_json::json!(false);
        phase2_ready_summary["pftl_replay_equivalence_required_count"] = serde_json::json!(0);
        phase2_ready_summary["pftl_candidate_batch_classes"] =
            serde_json::json!(nav_roundtrip_phase2_default_candidate_classes());
        phase2_ready_summary["pftl_live_round_compression_ready"] = serde_json::json!(true);
        phase2_ready_summary["pftl_live_round_compression_blockers"] = serde_json::json!([]);
        std::fs::write(
            &phase2_ready_summary_file,
            serde_json::to_string_pretty(&phase2_ready_summary).expect("phase2 summary serialize"),
        )
        .expect("write phase2 ready summary");
        let phase2_ready_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase2".to_string(),
                summary_file: Some(phase2_ready_summary_file),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: Some(root.join("phase2-matching-corpus-dir")),
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: None,
                strict_exit: false,
            })
            .expect("phase2 derived candidate corpus verification");
        assert!(phase2_ready_gate.passed, "{:?}", phase2_ready_gate.failure_reasons);
        assert_eq!(
            nav_roundtrip_phase2_default_candidate_classes(),
            phase2_ready_gate.phase2_required_candidate_classes
        );
        let phase3_old_bridge_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase3".to_string(),
                summary_file: Some(summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: None,
                strict_exit: false,
            })
            .expect("phase3 old bridge verification");
        assert!(!phase3_old_bridge_gate.passed);
        assert!(phase3_old_bridge_gate
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("fixed_contracts_redeployed_consolidated")));
        assert_eq!(
            Some(false),
            phase3_old_bridge_gate.summaries[0].phase3_consolidated_bridge_evidence_ok
        );

        let consolidated_summary_file =
            artifact_dir.join("roundtrip-summary-phase3-consolidated.json");
        let mut consolidated_summary: serde_json::Value =
            serde_json::from_str(&summary_raw).expect("phase3 summary json");
        consolidated_summary["setup_or_recovery_work_included_in_total"] =
            serde_json::json!(false);
        consolidated_summary["bridge_class"] =
            serde_json::json!(NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED_CONSOLIDATED);
        consolidated_summary["evm_withdrawal"]["bridge_class"] =
            serde_json::json!(NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED_CONSOLIDATED);
        consolidated_summary["evm_withdrawal"]["receipt_watches"] = serde_json::json!([
            {
                "label": "submit-proof",
                "tx_hash": "0x1111111111111111111111111111111111111111111111111111111111111111",
                "source_rpc_provider_class": "dedicated_or_gateway_http",
                "confirmation_source": "stakehub_agent_response",
                "status": "confirmed",
                "gas_used": 10,
                "elapsed_ms": 1.0
            },
            {
                "label": "finalize-proof-and-submit-withdrawal",
                "tx_hash": "0x2222222222222222222222222222222222222222222222222222222222222222",
                "source_rpc_provider_class": "dedicated_or_gateway_http",
                "confirmation_source": "stakehub_agent_response",
                "status": "confirmed",
                "gas_used": 20,
                "elapsed_ms": 2.0
            },
            {
                "label": "finalize-withdrawal-and-claim",
                "tx_hash": "0x3333333333333333333333333333333333333333333333333333333333333333",
                "source_rpc_provider_class": "dedicated_or_gateway_http",
                "confirmation_source": "stakehub_agent_response",
                "status": "confirmed",
                "gas_used": 30,
                "elapsed_ms": 3.0
            }
        ]);
        std::fs::write(
            &consolidated_summary_file,
            serde_json::to_string_pretty(&consolidated_summary).expect("phase3 summary serialize"),
        )
        .expect("write phase3 summary");
        let phase3_gate =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase3".to_string(),
                summary_file: Some(consolidated_summary_file.clone()),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: None,
                strict_exit: false,
            })
            .expect("phase3 consolidated benchmark verification");
        assert!(phase3_gate.passed, "{:?}", phase3_gate.failure_reasons);
        assert_eq!(1, phase3_gate.clean_run_count);
        assert!(phase3_gate.phase3_consolidated_bridge_required);
        assert_eq!(
            Some(true),
            phase3_gate.summaries[0].phase3_consolidated_bridge_evidence_ok
        );
        assert_eq!(
            vec![
                "submit-proof".to_string(),
                "finalize-proof-and-submit-withdrawal".to_string(),
                "finalize-withdrawal-and-claim".to_string()
            ],
            phase3_gate.summaries[0].evm_withdrawal_receipt_watch_labels
        );
        let strict_phase2_error =
            nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: "phase2".to_string(),
                summary_file: Some(summary_file),
                benchmark_dir: None,
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                min_clean_runs: Some(1),
                max_median_ms: Some(600_000.0),
                max_p90_ms: None,
                strict_exit: true,
            })
            .expect_err("strict phase2 gate should fail");
        assert!(strict_phase2_error.contains("benchmark verification failed"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_benchmark_plan_generates_phase1_battery_commands() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-benchmark-plan-{}",
            process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create root");
        let base_args_file = root.join("base-args.json");
        let benchmark_dir = root.join("benchmark");
        let report_file = benchmark_dir.join("reports").join("phase1-plan.json");
        let base_args = nav_roundtrip_benchmark_plan_test_args(&root);
        write_json_file(
            &base_args_file,
            &serde_json::json!({
                "args": base_args,
            }),
        )
        .expect("write base args");

        let report = nav_roundtrip_benchmark_plan(NavRoundtripBenchmarkPlanOptions {
            phase: "phase1".to_string(),
            base_args_file: base_args_file.clone(),
            benchmark_dir: benchmark_dir.clone(),
            replay_corpus_file: None,
            replay_corpus_dir: None,
            required_candidate_classes: Vec::new(),
            report_file: Some(report_file.clone()),
            run_count: 3,
            run_prefix: "run-".to_string(),
            binary: "target/release/postfiat-node".to_string(),
            max_median_ms: Some(95_000.0),
            max_p90_ms: Some(105_000.0),
            overwrite: false,
        })
        .expect("benchmark plan");

        assert_eq!(NAV_ROUNDTRIP_BENCHMARK_PLAN_SCHEMA, report.schema);
        assert_eq!("phase1", report.phase);
        assert_eq!(3, report.run_count);
        assert_eq!(3, report.runs.len());
        assert!(report_file.is_file());
        assert_eq!(
            vec![
                "--fast-demo-preflight".to_string(),
                "--background-audit".to_string(),
                "--reuse-final-certified-state".to_string(),
                "--require-warm-usdc-allowance".to_string()
            ],
            report.required_flags
        );
        assert_eq!(3, report.verifier_thresholds.min_clean_runs);
        assert_eq!(Some(95_000.0), report.verifier_thresholds.max_median_ms);
        assert_eq!(Some(105_000.0), report.verifier_thresholds.max_p90_ms);
        assert_eq!(
            Some(benchmark_dir.join("allowance-setup").display().to_string()),
            report.allowance_setup_command.artifact_dir
        );
        assert_eq!(
            Some(
                benchmark_dir
                    .join("allowance-setup")
                    .join("allowance-setup.json")
                    .display()
                    .to_string()
            ),
            report.allowance_setup_command.summary_file
        );
        assert!(report
            .allowance_setup_command
            .command
            .iter()
            .any(|arg| arg == "--warm-usdc-allowance-only"));
        assert_eq!(
            Some("4000000"),
            flag_value(
                &report.allowance_setup_command.command,
                "--required-allowance-atoms"
            )
        );
        assert_eq!(
            Some("nav-roundtrip-bench-allowance-setup"),
            flag_value(&report.allowance_setup_command.command, "--session-id")
        );

        assert_eq!(0, report.smoke_run.run_index);
        assert_eq!("smoke", report.smoke_run.run_label);
        let smoke_dir = root.join("benchmark-smoke");
        assert_eq!(smoke_dir.display().to_string(), report.smoke_run.artifact_dir);
        assert_eq!(
            smoke_dir
                .join("roundtrip-summary.json")
                .display()
                .to_string(),
            report.smoke_run.summary_file
        );
        assert_eq!(
            Some(
                smoke_dir
                    .join("fleet-preflight")
                    .display()
                    .to_string()
            ),
            report.smoke_run.fleet_preflight_command.artifact_dir
        );
        assert!(report
            .smoke_run
            .fleet_preflight_command
            .command
            .iter()
            .any(|arg| arg == "--fleet-preflight-only"));
        for required_flag in [
            "--fast-demo-preflight",
            "--background-audit",
            "--reuse-final-certified-state",
            "--require-warm-usdc-allowance",
        ] {
            assert!(
                report
                    .smoke_run
                    .run_command
                    .command
                    .iter()
                    .any(|arg| arg == required_flag),
                "missing smoke {required_flag}"
            );
        }
        assert_eq!(
            Some("0x0000000000000000000000000000000000000000000000000000000000000004"),
            flag_value(&report.smoke_run.run_command.command, "--nonce")
        );
        assert_eq!(
            Some("nav-roundtrip-bench-smoke"),
            flag_value(&report.smoke_run.run_command.command, "--session-id")
        );
        assert!(report
            .smoke_verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--summary", report.smoke_run.summary_file.as_str()]));
        assert!(report
            .smoke_verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--min-clean-runs", "1"]));
        assert!(report
            .smoke_verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--max-median-ms", "600000"]));
        assert!(!report
            .smoke_verifier_command
            .command
            .iter()
            .any(|arg| arg == "--benchmark-dir"));

        let first = &report.runs[0];
        assert_eq!("run-01", first.run_label);
        assert_eq!(
            benchmark_dir.join("run-01").display().to_string(),
            first.artifact_dir
        );
        assert_eq!(
            benchmark_dir
                .join("run-01")
                .join("roundtrip-summary.json")
                .display()
                .to_string(),
            first.summary_file
        );
        assert_eq!(
            Some(
                benchmark_dir
                    .join("run-01")
                    .join("fleet-preflight")
                    .display()
                    .to_string()
            ),
            first.fleet_preflight_command.artifact_dir
        );
        assert!(first
            .fleet_preflight_command
            .command
            .iter()
            .any(|arg| arg == "--fleet-preflight-only"));
        for required_flag in [
            "--fast-demo-preflight",
            "--background-audit",
            "--reuse-final-certified-state",
            "--require-warm-usdc-allowance",
        ] {
            assert!(
                first.run_command.command.iter().any(|arg| arg == required_flag),
                "missing {required_flag}"
            );
        }
        assert_eq!(
            Some("0x0000000000000000000000000000000000000000000000000000000000000001"),
            flag_value(&first.run_command.command, "--nonce")
        );
        assert_eq!(
            Some("nav-roundtrip-bench-run-01"),
            flag_value(&first.run_command.command, "--session-id")
        );
        let second = &report.runs[1];
        assert_eq!(
            Some("0x0000000000000000000000000000000000000000000000000000000000000002"),
            flag_value(&second.run_command.command, "--nonce")
        );
        assert_eq!(
            Some("nav-roundtrip-bench-run-02"),
            flag_value(&second.run_command.command, "--session-id")
        );
        assert!(report
            .verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--min-clean-runs", "3"]));
        assert!(report
            .verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--max-median-ms", "95000"]));
        assert!(report
            .verifier_command
            .command_line
            .contains("nav-roundtrip-benchmark-verify"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_benchmark_plan_generates_phase2_replay_gated_commands() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-benchmark-plan-phase2-{}",
            process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create root");
        let base_args_file = root.join("base-args.json");
        let benchmark_dir = root.join("benchmark");
        let report_file = root.join("phase2-plan.json");
        let replay_corpus_dir = root.join("replay-corpus");
        let base_args = nav_roundtrip_benchmark_plan_test_args(&root);
        write_json_file(
            &base_args_file,
            &serde_json::json!({
                "args": base_args,
            }),
        )
        .expect("write base args");

        let report = nav_roundtrip_benchmark_plan(NavRoundtripBenchmarkPlanOptions {
            phase: "phase2".to_string(),
            base_args_file: base_args_file.clone(),
            benchmark_dir: benchmark_dir.clone(),
            replay_corpus_file: None,
            replay_corpus_dir: Some(replay_corpus_dir.clone()),
            required_candidate_classes: vec![
                "vault_bridge_deposit_propose_attest".to_string(),
                "vault_bridge_receipt_submit_count".to_string(),
                "nav_subscription_allocate_mint_at_nav".to_string(),
                "nav_redeem_at_nav_settle".to_string(),
            ],
            report_file: Some(report_file.clone()),
            run_count: 2,
            run_prefix: "phase2-".to_string(),
            binary: "postfiat-node".to_string(),
            max_median_ms: None,
            max_p90_ms: None,
            overwrite: true,
        })
        .expect("phase2 benchmark plan");

        assert_eq!("phase2", report.phase);
        assert_eq!(2, report.run_count);
        assert!(report_file.is_file());
        assert_eq!(
            Some(replay_corpus_dir.display().to_string()),
            report.replay_corpus_dir
        );
        assert_eq!(Some(75_000.0), report.verifier_thresholds.max_median_ms);
        assert_eq!(None, report.verifier_thresholds.max_p90_ms);
        assert!(report
            .required_flags
            .iter()
            .any(|flag| flag == "--same-round-nav-exit"));
        assert!(report
            .smoke_run
            .run_command
            .command
            .iter()
            .any(|arg| arg == "--same-round-nav-exit"));
        assert!(report
            .smoke_verifier_command
            .command
            .windows(2)
            .any(|window| window
                == [
                    "--replay-corpus-dir",
                    replay_corpus_dir.display().to_string().as_str()
                ]));
        assert!(report
            .smoke_verifier_command
            .command
            .windows(2)
            .any(|window| window[0] == "--require-candidate-classes"
                && window[1].contains("nav_redeem_at_nav_settle")));
        for run in &report.runs {
            assert!(
                run.run_command
                    .command
                    .iter()
                    .any(|arg| arg == "--same-round-nav-exit"),
                "missing phase2 same-round exit flag in {}",
                run.run_label
            );
            assert!(
                run.run_command.command.iter().any(|arg| arg == "--overwrite"),
                "phase2 overwrite flag should propagate to timed runs"
            );
        }
        assert!(report
            .verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--phase", "phase2"]));
        assert!(report
            .verifier_command
            .command
            .windows(2)
            .any(|window| window
                == [
                    "--replay-corpus-dir",
                    replay_corpus_dir.display().to_string().as_str()
                ]));
        assert!(report
            .verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--max-median-ms", "75000"]));
        assert!(!report
            .verifier_command
            .command
            .iter()
            .any(|arg| arg == "--max-p90-ms"));
        assert!(report
            .verifier_command
            .command
            .windows(2)
            .any(|window| window[0] == "--require-candidate-classes"
                && window[1].contains("nav_redeem_at_nav_settle")));

        let default_classes_report = nav_roundtrip_benchmark_plan(NavRoundtripBenchmarkPlanOptions {
            phase: "phase2".to_string(),
            base_args_file: base_args_file.clone(),
            benchmark_dir: root.join("benchmark-default-classes"),
            replay_corpus_file: None,
            replay_corpus_dir: Some(replay_corpus_dir.clone()),
            required_candidate_classes: Vec::new(),
            report_file: None,
            run_count: 1,
            run_prefix: "phase2-default-".to_string(),
            binary: "postfiat-node".to_string(),
            max_median_ms: None,
            max_p90_ms: None,
            overwrite: true,
        })
        .expect("phase2 benchmark plan with default candidate classes");
        assert_eq!(
            nav_roundtrip_phase2_default_candidate_classes(),
            default_classes_report.required_candidate_classes
        );
        assert!(default_classes_report
            .verifier_command
            .command
            .windows(2)
            .any(|window| window[0] == "--require-candidate-classes"
                && window[1].contains("nav_redeem_at_nav_settle")));

        let missing_corpus_error =
            nav_roundtrip_benchmark_plan(NavRoundtripBenchmarkPlanOptions {
                phase: "phase2".to_string(),
                base_args_file,
                benchmark_dir,
                replay_corpus_file: None,
                replay_corpus_dir: None,
                required_candidate_classes: Vec::new(),
                report_file: None,
                run_count: 1,
                run_prefix: "phase2-".to_string(),
                binary: "postfiat-node".to_string(),
                max_median_ms: None,
                max_p90_ms: None,
                overwrite: false,
            })
            .expect_err("phase2 plan must require replay corpus evidence");
        assert!(
            missing_corpus_error.contains("replay-corpus"),
            "{missing_corpus_error}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_benchmark_plan_generates_phase3_consolidated_gate_commands() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-benchmark-plan-phase3-{}",
            process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create root");
        let base_args_file = root.join("base-args.json");
        let benchmark_dir = root.join("benchmark");
        let report_file = root.join("phase3-plan.json");
        let base_args = nav_roundtrip_benchmark_plan_test_args(&root);
        write_json_file(
            &base_args_file,
            &serde_json::json!({
                "args": base_args,
            }),
        )
        .expect("write base args");

        let report = nav_roundtrip_benchmark_plan(NavRoundtripBenchmarkPlanOptions {
            phase: "phase3".to_string(),
            base_args_file,
            benchmark_dir: benchmark_dir.clone(),
            replay_corpus_file: None,
            replay_corpus_dir: None,
            required_candidate_classes: Vec::new(),
            report_file: Some(report_file.clone()),
            run_count: 2,
            run_prefix: "phase3-".to_string(),
            binary: "postfiat-node".to_string(),
            max_median_ms: None,
            max_p90_ms: None,
            overwrite: false,
        })
        .expect("phase3 benchmark plan");

        assert_eq!("phase3", report.phase);
        assert_eq!(2, report.run_count);
        assert!(report_file.is_file());
        assert_eq!(Some(55_000.0), report.verifier_thresholds.max_median_ms);
        assert_eq!(None, report.verifier_thresholds.max_p90_ms);
        assert_eq!(None, report.replay_corpus_file);
        assert_eq!(None, report.replay_corpus_dir);
        assert!(report
            .required_flags
            .iter()
            .any(|flag| flag == "--fast-demo-preflight"));
        assert!(!report
            .required_flags
            .iter()
            .any(|flag| flag == "--same-round-nav-exit"));
        assert_eq!(
            root.join("benchmark-smoke").display().to_string(),
            report.smoke_run.artifact_dir
        );
        assert!(report
            .smoke_verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--phase", "phase3"]));
        assert!(!report
            .smoke_run
            .run_command
            .command
            .iter()
            .any(|arg| arg == "--same-round-nav-exit"));
        let first = &report.runs[0];
        assert_eq!("phase3-01", first.run_label);
        assert_eq!(
            benchmark_dir.join("phase3-01").display().to_string(),
            first.artifact_dir
        );
        assert!(report
            .verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--phase", "phase3"]));
        assert!(report
            .verifier_command
            .command
            .windows(2)
            .any(|window| window == ["--max-median-ms", "55000"]));
        assert!(!report
            .verifier_command
            .command
            .iter()
            .any(|arg| arg == "--max-p90-ms"));
        assert!(!report
            .verifier_command
            .command
            .iter()
            .any(|arg| arg == "--replay-corpus-dir"));
        assert!(report
            .verifier_command
            .command_line
            .contains("nav-roundtrip-benchmark-verify"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_benchmark_plan_rejects_static_signatures_file() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-benchmark-plan-reject-{}",
            process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create root");
        let base_args_file = root.join("base-args.json");
        let mut base_args = nav_roundtrip_benchmark_plan_test_args(&root);
        let signer_flag_index = base_args
            .iter()
            .position(|arg| arg == "--withdrawal-signer-key-file")
            .expect("signer flag");
        base_args[signer_flag_index] = "--signatures-file".to_string();
        write_json_file(&base_args_file, &base_args).expect("write base args");

        let error = nav_roundtrip_benchmark_plan(NavRoundtripBenchmarkPlanOptions {
            phase: "phase1".to_string(),
            base_args_file,
            benchmark_dir: root.join("benchmark"),
            replay_corpus_file: None,
            replay_corpus_dir: None,
            required_candidate_classes: Vec::new(),
            report_file: None,
            run_count: 10,
            run_prefix: "run-".to_string(),
            binary: "postfiat-node".to_string(),
            max_median_ms: None,
            max_p90_ms: None,
            overwrite: false,
        })
        .expect_err("static signatures file should not be benchmarkable");
        assert!(error.contains("--withdrawal-signer-key-file"), "{error}");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_benchmark_plan_rejects_pftl_only_base_args() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-benchmark-plan-pftl-only-reject-{}",
            process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create root");
        let base_args_file = root.join("base-args.json");
        let mut base_args = nav_roundtrip_benchmark_plan_test_args(&root);
        base_args.push("--pftl-only".to_string());
        write_json_file(&base_args_file, &base_args).expect("write base args");

        let error = nav_roundtrip_benchmark_plan(NavRoundtripBenchmarkPlanOptions {
            phase: "phase1".to_string(),
            base_args_file,
            benchmark_dir: root.join("benchmark"),
            replay_corpus_file: None,
            replay_corpus_dir: None,
            required_candidate_classes: Vec::new(),
            report_file: None,
            run_count: 10,
            run_prefix: "run-".to_string(),
            binary: "postfiat-node".to_string(),
            max_median_ms: None,
            max_p90_ms: None,
            overwrite: false,
        })
        .expect_err("PFTL-only warm path must not be benchmarked as full roundtrip");
        assert!(
            error.contains("full Arbitrum roundtrip") && error.contains("--pftl-only"),
            "{error}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_live_demo_rejects_degraded_finality_flags() {
        let allow_error = run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--allow-peer-failures".to_string(),
        ])
        .expect_err("live NAV roundtrip must reject peer failures");
        assert!(
            allow_error.contains("rejects --allow-peer-failures"),
            "{allow_error}"
        );

        let defer_error = run_cli(vec![
            "nav-roundtrip-live-demo".to_string(),
            "--defer-certified-sends".to_string(),
        ])
        .expect_err("live NAV roundtrip must reject deferred certified sends");
        assert!(
            defer_error.contains("rejects --defer-certified-sends"),
            "{defer_error}"
        );
    }

    fn nav_roundtrip_benchmark_plan_test_args(root: &std::path::Path) -> Vec<String> {
        let withdrawal_key = root.join("withdrawal.key").display().to_string();
        vec![
            "postfiat-node".to_string(),
            "nav-roundtrip-live-demo".to_string(),
            "--data-dir".to_string(),
            ".postfiat/node0".to_string(),
            "--topology".to_string(),
            "/tmp/topology.json".to_string(),
            "--key-file".to_string(),
            "/tmp/validator.key".to_string(),
            "--artifact-dir".to_string(),
            "/tmp/ignored-artifact".to_string(),
            "--source-rpc-url".to_string(),
            "https://arb.example.invalid/rpc".to_string(),
            "--vault".to_string(),
            "0x1111111111111111111111111111111111111111".to_string(),
            "--verifier".to_string(),
            "0x2222222222222222222222222222222222222222".to_string(),
            "--usdc".to_string(),
            "0x3333333333333333333333333333333333333333".to_string(),
            "--stakehub-wallet".to_string(),
            "0x4444444444444444444444444444444444444444".to_string(),
            "--nav-asset".to_string(),
            "a651".to_string(),
            "--pfusdc".to_string(),
            "pfusdc".to_string(),
            "--policy-hash".to_string(),
            "0x5555555555555555555555555555555555555555555555555555555555555555".to_string(),
            "--pftl-recipient".to_string(),
            "pf1recipient".to_string(),
            "--proposer".to_string(),
            "pf1proposer".to_string(),
            "--finalizer".to_string(),
            "pf1finalizer".to_string(),
            "--claimer".to_string(),
            "pf1claimer".to_string(),
            "--proposer-key-file".to_string(),
            "/tmp/proposer.key".to_string(),
            "--finalizer-key-file".to_string(),
            "/tmp/finalizer.key".to_string(),
            "--claimer-key-file".to_string(),
            "/tmp/claimer.key".to_string(),
            "--issuer-key-file".to_string(),
            "/tmp/issuer.key".to_string(),
            "--owner-key-file".to_string(),
            "/tmp/owner.key".to_string(),
            "--amount-atoms".to_string(),
            "1000000".to_string(),
            "--mint-amount".to_string(),
            "1".to_string(),
            "--nonce".to_string(),
            "0x0000000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            "--session-id".to_string(),
            "nav-roundtrip-bench".to_string(),
            "--withdrawal-signer-key-file".to_string(),
            withdrawal_key,
            "--expires-at-height".to_string(),
            "1000".to_string(),
            "--timeout-ms".to_string(),
            "7000".to_string(),
            "--fast-demo-preflight".to_string(),
            "--resume".to_string(),
        ]
    }

    #[test]
    fn nav_roundtrip_final_validator_evidence_can_reuse_certified_round_state() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-final-certified-state-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let missing_topology_file = root.join("missing-topology.json");
        let artifact_dir = root.join("artifacts");
        let summary_file = artifact_dir.join("roundtrip-summary.json");
        let _ = std::fs::remove_dir_all(&root);
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");
        let final_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("final status");
        let certified_state =
            nav_roundtrip_validator_state_from_status(&final_status, "certified-round-test");
        let certified_states = vec![certified_state.clone()];
        assert_eq!(
            "websocket",
            nav_roundtrip_source_rpc_provider_class("wss://arb.example.invalid/rpc")
        );
        assert_eq!(
            "local",
            nav_roundtrip_source_rpc_provider_class("http://127.0.0.1:8547")
        );
        assert_eq!(
            "dedicated_or_gateway_http",
            nav_roundtrip_source_rpc_provider_class("https://arb-mainnet.g.alchemy.com/v2/key")
        );
        assert_eq!(
            "public_or_unknown_http",
            nav_roundtrip_source_rpc_provider_class("https://arb.example.invalid/rpc")
        );
        assert!(!nav_roundtrip_should_reuse_final_certified_state(
            false, false
        ));
        assert!(nav_roundtrip_should_reuse_final_certified_state(
            true, false
        ));
        assert!(nav_roundtrip_should_reuse_final_certified_state(
            false, true
        ));
        assert_eq!(
            "blocking_public_rpc",
            nav_roundtrip_final_audit_profile(false, false)
        );
        assert_eq!(
            "certified_round_hot_path",
            nav_roundtrip_final_audit_profile(true, false)
        );
        assert_eq!(
            "background_audit_certified_round_hot_path",
            nav_roundtrip_final_audit_profile(false, true)
        );

        let (public_states, final_states, source) = nav_roundtrip_select_final_validator_states(
            &missing_topology_file,
            &final_status,
            5_000,
            &certified_states,
            true,
        )
        .expect("reuse mode should not need public topology");
        assert!(public_states.is_empty());
        assert_eq!("certified_round", source);
        assert_eq!(1, final_states.len());
        assert_eq!(certified_state.state_root, final_states[0].state_root);

        let public_error = nav_roundtrip_select_final_validator_states(
            &missing_topology_file,
            &final_status,
            5_000,
            &certified_states,
            false,
        )
        .expect_err("public mode should still require topology/RPC evidence");
        assert!(
            public_error.contains("topology") || public_error.contains("No such file"),
            "{public_error}"
        );

        let audit_request_file = nav_roundtrip_write_background_audit_request(
            &artifact_dir,
            &summary_file,
            &data_dir,
            &missing_topology_file,
            5_000,
            &final_status,
            "certified_round",
            &certified_states,
        )
        .expect("write background audit request");
        let audit_request = serde_json::from_str::<NavRoundtripBackgroundAuditRequest>(
            &std::fs::read_to_string(&audit_request_file).expect("read audit request"),
        )
        .expect("parse audit request");
        assert_eq!(
            NAV_ROUNDTRIP_BACKGROUND_AUDIT_REQUEST_SCHEMA,
            audit_request.schema
        );
        assert_eq!(summary_file.display().to_string(), audit_request.roundtrip_summary_file);
        assert_eq!("certified_round", audit_request.final_validator_state_source);
        assert_eq!(final_status.block_height, audit_request.final_height);
        assert_eq!(final_status.state_root, audit_request.final_state_root);
        assert!(audit_request
            .suggested_command
            .contains("--fleet-preflight-only"));
        assert!(audit_request
            .required_checks
            .iter()
            .any(|check| check.contains("public validator status")));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_fleet_preflight_accepts_matching_nonloopback_public_endpoint() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-fleet-matching-public-endpoint-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let artifact_dir = root.join("artifacts");
        let _ = std::fs::remove_dir_all(&root);
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");
        let local_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("local status");
        let public_rpc_listener =
            TcpListener::bind(("0.0.0.0", 0)).expect("bind fake non-loopback public RPC");
        let public_rpc_port = public_rpc_listener
            .local_addr()
            .expect("fake public RPC addr")
            .port();
        write_json_file(
            &topology_file,
            &serde_json::json!({
                "topology_id": "matching-public-endpoint-test",
                "chain_id": local_status.chain_id,
                "genesis_hash": local_status.genesis_hash,
                "protocol_version": local_status.protocol_version,
                "peers": [{
                    "node_id": local_status.node_id,
                    "host": "0.0.0.0",
                    "p2p_port": public_rpc_port.saturating_sub(1),
                    "rpc_port": public_rpc_port,
                    "p2p_address": format!("0.0.0.0:{}", public_rpc_port.saturating_sub(1))
                }]
            }),
        )
        .expect("write topology");

        let public_status = local_status.clone();
        let rpc_thread = std::thread::spawn(move || {
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
                vec![RpcEvent::new("status", "validator-0", "status queried")],
            )
            .expect("fake public RPC response");
            write_json_line(&mut stream, &response).expect("write fake public RPC response");
        });

        let report = nav_roundtrip_live_fleet_preflight(
            &data_dir,
            &topology_file,
            &artifact_dir,
            5_000,
            false,
            false,
            false,
        )
        .expect("matching public validator mirror should pass");
        rpc_thread.join().expect("fake public RPC thread");
        assert!(report.preflight_ok);
        assert!(report.local_node_id_in_topology);
        assert_eq!(Some("0.0.0.0"), report.local_node_id_public_host.as_deref());
        assert!(report.operator_matches_public_endpoint);
        assert!(!report.reused_artifact);
        assert!(report.failure_reasons.is_empty(), "{:?}", report.failure_reasons);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_fleet_preflight_accepts_lagging_public_self_endpoint_when_operator_matches_quorum() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-fleet-lagging-public-self-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let artifact_dir = root.join("artifacts");
        let _ = std::fs::remove_dir_all(&root);
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
                "topology_id": "lagging-public-self-endpoint-test",
                "chain_id": local_status.chain_id,
                "genesis_hash": local_status.genesis_hash,
                "protocol_version": local_status.protocol_version,
                "peers": peers
            }),
        )
        .expect("write topology");

        let report = nav_roundtrip_live_fleet_preflight(
            &data_dir,
            &topology_file,
            &artifact_dir,
            5_000,
            false,
            false,
            false,
        )
        .expect("operator-local quorum state should pass despite lagging public self endpoint");
        for rpc_thread in rpc_threads {
            rpc_thread.join().expect("fake public RPC thread");
        }
        assert!(report.preflight_ok);
        assert!(report.public_validator_consensus_ok);
        assert!(!report.operator_matches_public_endpoint);
        assert!(report.operator_matches_public_quorum);
        assert!(report.failure_reasons.is_empty(), "{:?}", report.failure_reasons);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_fleet_preflight_resume_reuses_matching_artifact() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-fleet-resume-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let artifact_dir = root.join("artifacts");
        let _ = std::fs::remove_dir_all(&root);
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");
        let local_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("local status");
        let public_rpc_listener =
            TcpListener::bind(("0.0.0.0", 0)).expect("bind fake non-loopback public RPC");
        let public_rpc_port = public_rpc_listener
            .local_addr()
            .expect("fake public RPC addr")
            .port();
        write_json_file(
            &topology_file,
            &serde_json::json!({
                "topology_id": "resume-public-endpoint-test",
                "chain_id": local_status.chain_id,
                "genesis_hash": local_status.genesis_hash,
                "protocol_version": local_status.protocol_version,
                "peers": [{
                    "node_id": local_status.node_id,
                    "host": "0.0.0.0",
                    "p2p_port": public_rpc_port.saturating_sub(1),
                    "rpc_port": public_rpc_port,
                    "p2p_address": format!("0.0.0.0:{}", public_rpc_port.saturating_sub(1))
                }]
            }),
        )
        .expect("write topology");

        let public_status = local_status.clone();
        let rpc_thread = std::thread::spawn(move || {
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
                vec![RpcEvent::new("status", "validator-0", "status queried")],
            )
            .expect("fake public RPC response");
            write_json_line(&mut stream, &response).expect("write fake public RPC response");
        });

        let first = nav_roundtrip_live_fleet_preflight(
            &data_dir,
            &topology_file,
            &artifact_dir,
            5_000,
            false,
            false,
            false,
        )
        .expect("initial fleet preflight should pass");
        rpc_thread.join().expect("fake public RPC thread");
        let resumed = nav_roundtrip_live_fleet_preflight(
            &data_dir,
            &topology_file,
            &artifact_dir,
            5_000,
            true,
            false,
            true,
        )
        .expect("matching cached fleet preflight should resume without RPC");
        assert!(!first.reused_artifact);
        assert!(resumed.reused_artifact);
        assert_eq!(first.operator_local_state.state_root, resumed.operator_local_state.state_root);
        assert_eq!(first.public_validator_states.len(), resumed.public_validator_states.len());

        let artifact_file = artifact_dir.join("fleet-preflight.json");
        let mut cached = serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(&artifact_file).expect("read fleet preflight"),
        )
        .expect("parse fleet preflight");
        cached["operator_local_state"]["state_root"] = serde_json::json!("00");
        write_json_file(&artifact_file, &cached).expect("write stale cached fleet preflight");
        let stale_error = nav_roundtrip_live_fleet_preflight(
            &data_dir,
            &topology_file,
            &artifact_dir,
            5_000,
            true,
            false,
            true,
        )
        .expect_err("stale cached fleet preflight should fail");
        assert!(
            stale_error.contains("does not match current local status"),
            "{stale_error}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_fleet_preflight_fast_mode_requires_cached_artifact() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-fleet-fast-requires-cache-{}",
            process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let error = nav_roundtrip_live_fleet_preflight(
            &root.join("node"),
            &root.join("topology.json"),
            &root.join("artifacts"),
            5_000,
            true,
            false,
            true,
        )
        .expect_err("fast preflight must require precomputed fleet evidence");
        assert!(
            error.contains("requires precomputed NAV roundtrip fleet preflight"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nav_roundtrip_fleet_preflight_rejects_mismatched_public_endpoint() {
        let root = env::temp_dir().join(format!(
            "postfiat-nav-roundtrip-fleet-mismatched-public-endpoint-{}",
            process::id()
        ));
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let artifact_dir = root.join("artifacts");
        let _ = std::fs::remove_dir_all(&root);
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init node store");
        let local_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("local status");
        let public_rpc_listener =
            TcpListener::bind(("0.0.0.0", 0)).expect("bind fake non-loopback public RPC");
        let public_rpc_port = public_rpc_listener
            .local_addr()
            .expect("fake public RPC addr")
            .port();
        write_json_file(
            &topology_file,
            &serde_json::json!({
                "topology_id": "mismatched-public-endpoint-test",
                "chain_id": local_status.chain_id,
                "genesis_hash": local_status.genesis_hash,
                "protocol_version": local_status.protocol_version,
                "peers": [{
                    "node_id": local_status.node_id,
                    "host": "0.0.0.0",
                    "p2p_port": public_rpc_port.saturating_sub(1),
                    "rpc_port": public_rpc_port,
                    "p2p_address": format!("0.0.0.0:{}", public_rpc_port.saturating_sub(1))
                }]
            }),
        )
        .expect("write topology");

        let mut public_status = local_status.clone();
        public_status.block_height = public_status
            .block_height
            .checked_add(1)
            .expect("advance public height");
        let rpc_thread = std::thread::spawn(move || {
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
                vec![RpcEvent::new("status", "validator-0", "status queried")],
            )
            .expect("fake public RPC response");
            write_json_line(&mut stream, &response).expect("write fake public RPC response");
        });

        let error = nav_roundtrip_live_fleet_preflight(
            &data_dir,
            &topology_file,
            &artifact_dir,
            5_000,
            false,
            false,
            false,
        )
        .expect_err("mismatched public validator state must be rejected");
        rpc_thread.join().expect("fake public RPC thread");
        assert!(
            error.contains("does not match its public validator endpoint state"),
            "{error}"
        );
        let report = serde_json::from_str::<NavRoundtripFleetPreflightReport>(
            &std::fs::read_to_string(artifact_dir.join("fleet-preflight.json"))
                .expect("read fleet preflight report"),
        )
        .expect("parse fleet preflight report");
        assert!(!report.preflight_ok);
        assert!(report.local_node_id_in_topology);
        assert_eq!(
            Some("0.0.0.0"),
            report.local_node_id_public_host.as_deref()
        );
        assert!(
            report
                .failure_reasons
                .iter()
                .any(|reason| reason.contains("does not match its public validator endpoint state")),
            "{:?}",
            report.failure_reasons
        );
        let _ = std::fs::remove_dir_all(root);
    }
