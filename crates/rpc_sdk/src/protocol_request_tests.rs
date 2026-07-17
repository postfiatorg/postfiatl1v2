    use std::fs;

    use serde::{Deserialize, Serialize};
    use serde_json::json;

    use super::*;

    fn valid_signed_transfer(hex96: &str) -> serde_json::Value {
        json!({
            "unsigned": {
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "address_namespace": TRANSPARENT_ADDRESS_NAMESPACE,
                "transaction_kind": TRANSPARENT_TRANSFER_KIND,
                "signature_algorithm_id": ML_DSA_65_ALGORITHM,
                "from": "pffaucet",
                "to": "pfrecipient",
                "amount": 42,
                "fee": 1,
                "sequence": 0
            },
            "algorithm_id": ML_DSA_65_ALGORITHM,
            "public_key_hex": hex96,
            "signature_hex": hex96
        })
    }

    fn valid_signed_atomic_swap() -> postfiat_types::SignedAtomicSwapTransaction {
        let owner_0 = format!("pf{}", "01".repeat(20));
        let owner_1 = format!("pf{}", "02".repeat(20));
        postfiat_types::SignedAtomicSwapTransaction {
            unsigned: postfiat_types::UnsignedAtomicSwapTransaction {
                chain_id: "postfiat-local".to_string(),
                genesis_hash: "aa".repeat(48),
                protocol_version: 1,
                address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
                signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                rfq_hash: "bb".repeat(48),
                market_envelope_hash: "cc".repeat(48),
                nav_epoch: 7,
                expires_at_height: 99,
                swap_nonce: "dd".repeat(48),
                leg_0: postfiat_types::AtomicSwapLeg {
                    owner: owner_0.clone(),
                    recipient: owner_1.clone(),
                    issuer: format!("pf{}", "03".repeat(20)),
                    asset_id: "10".repeat(48),
                    amount: 20_000,
                    sequence: 3,
                    fee: 22,
                },
                leg_1: postfiat_types::AtomicSwapLeg {
                    owner: owner_1.clone(),
                    recipient: owner_0.clone(),
                    issuer: format!("pf{}", "04".repeat(20)),
                    asset_id: "20".repeat(48),
                    amount: 164_020,
                    sequence: 5,
                    fee: 22,
                },
            },
            authorization_0: postfiat_types::AtomicSwapAuthorization {
                owner: owner_0,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: "aa".to_string(),
                signature_hex: "bb".to_string(),
            },
            authorization_1: postfiat_types::AtomicSwapAuthorization {
                owner: owner_1,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: "cc".to_string(),
                signature_hex: "dd".to_string(),
            },
        }
    }

    fn fastswap_test_domain() -> postfiat_types::FastSwapCommitteeDomainV1 {
        postfiat_types::FastSwapCommitteeDomainV1 {
            chain: postfiat_types::FastSwapChainDomainV1 {
                chain_id: "postfiat-fastswap-sdk-test".to_string(),
                genesis_hash: postfiat_types::FastSwapOpaqueHashV1([1; 48]),
                protocol_version: 1,
            },
            fastswap_schema_version: postfiat_types::FASTSWAP_SCHEMA_VERSION_V1,
            committee_epoch: 1,
            committee_root: postfiat_types::FastSwapCommitteeRootV1([2; 48]),
            validator_count: 4,
            quorum: 3,
        }
    }

    fn fastswap_test_vote(
        validator_id: &str,
        round: u64,
    ) -> postfiat_types::FastSwapVoteV1 {
        postfiat_types::FastSwapVoteV1 {
            domain: fastswap_test_domain(),
            swap_id: postfiat_types::FastSwapIdV1([3; 48]),
            phase: postfiat_types::FastSwapPhaseV1::Commit,
            round,
            decision: Some(postfiat_types::FastSwapDecisionV1::Confirm),
            justification_digest: None,
            effects_digest: postfiat_types::FastSwapEffectsDigestV1([4; 48]),
            receipt_digest: Some(postfiat_types::FastSwapReceiptDigestV1([5; 48])),
            validator_id: validator_id.to_string(),
            signature: vec![6; 64],
        }
    }

    fn fastswap_test_checkpoint() -> postfiat_types::FastLaneCheckpointV1 {
        postfiat_types::FastLaneCheckpointV1 {
            previous_checkpoint_id: None,
            committee: fastswap_test_domain(),
            live_object_root: postfiat_types::FastSwapOpaqueHashV1([7; 48]),
            live_object_totals: Vec::new(),
            exit_claim_root: postfiat_types::FastSwapOpaqueHashV1([8; 48]),
            exit_claim_totals: Vec::new(),
            pending_fee_burn_totals: Vec::new(),
            terminal_root: postfiat_types::FastSwapOpaqueHashV1([9; 48]),
            highest_wal_sequence: 1,
            active_policy_hashes: Vec::new(),
            imported_deposit_root: postfiat_types::FastSwapOpaqueHashV1([10; 48]),
            redeemed_exit_claim_root: postfiat_types::FastSwapOpaqueHashV1([11; 48]),
            drain_ready: true,
            fenced_policy_epochs: vec![1],
        }
    }

    #[test]
    fn fastswap_recovery_and_checkpoint_responses_fail_closed_semantically() {
        let certificate = postfiat_types::FastSwapCertificateV1 {
            votes: vec![
                fastswap_test_vote("validator-0", 7),
                fastswap_test_vote("validator-1", 7),
            ],
        };
        let evidence = postfiat_types::FastSwapVoteEvidenceResponseV1 {
            schema: "postfiat-fastswap-vote-evidence-v1".to_string(),
            validator_id: "validator-0".to_string(),
            swap_id: postfiat_types::FastSwapIdV1([3; 48]),
            phase: postfiat_types::FastSwapPhaseV1::Commit,
            round: 7,
            vote: None,
            new_round_vote: None,
            certificate: Some(certificate),
        };
        let evidence_json = serde_json::to_value(&evidence).expect("vote evidence JSON");
        validate_fastswap_vote_evidence_result(&evidence_json).expect("valid vote evidence");

        let mut wrong_round = evidence.clone();
        wrong_round
            .certificate
            .as_mut()
            .expect("certificate")
            .votes[1]
            .round = 8;
        assert!(validate_fastswap_vote_evidence_result(
            &serde_json::to_value(wrong_round).expect("wrong-round evidence JSON")
        )
        .is_err());

        let mut empty_signature = evidence;
        empty_signature
            .certificate
            .as_mut()
            .expect("certificate")
            .votes[1]
            .signature
            .clear();
        assert!(validate_fastswap_vote_evidence_result(
            &serde_json::to_value(empty_signature).expect("empty-signature evidence JSON")
        )
        .is_err());

        let checkpoint = fastswap_test_checkpoint();
        let status = postfiat_types::FastLaneCheckpointStatusV1 {
            schema: "postfiat-fastlane-checkpoint-status-v1".to_string(),
            vote: postfiat_types::FastLaneCheckpointVoteV1 {
                checkpoint: checkpoint.clone(),
                validator_id: "validator-0".to_string(),
                signature: vec![12; 64],
            },
            checkpoint,
            drain_ready: true,
            rotation_ready: true,
        };
        validate_fastswap_checkpoint_status_result(
            &serde_json::to_value(&status).expect("checkpoint status JSON"),
        )
        .expect("valid checkpoint status");

        let mut inconsistent_drain = status.clone();
        inconsistent_drain.drain_ready = false;
        assert!(validate_fastswap_checkpoint_status_result(
            &serde_json::to_value(inconsistent_drain).expect("inconsistent checkpoint JSON")
        )
        .is_err());

        let mut unsigned_vote = status;
        unsigned_vote.vote.signature.clear();
        assert!(validate_fastswap_checkpoint_status_result(
            &serde_json::to_value(unsigned_vote).expect("unsigned checkpoint JSON")
        )
        .is_err());
    }

    #[test]
    fn fastswap_request_builders_are_typed_and_fail_closed() {
        let cases = vec![
            (fastswap_capabilities_request("cap"), RpcRequestKind::FastSwapCapabilities),
            (fastswap_preview_request("preview", "{}"), RpcRequestKind::FastSwapPreview),
            (fastswap_prepare_request("prepare", "{}"), RpcRequestKind::FastSwapPrepare),
            (fastswap_commit_request("commit", "{}"), RpcRequestKind::FastSwapCommit),
            (fastswap_apply_request("apply", "{}", "{}"), RpcRequestKind::FastSwapApply),
            (
                fastswap_catch_up_request("catch", "{}", "{}", "{}"),
                RpcRequestKind::FastSwapCatchUp,
            ),
            (fastswap_status_request("status", "11"), RpcRequestKind::FastSwapStatus),
            (fastswap_effects_request("effects", "11"), RpcRequestKind::FastSwapEffects),
            (
                fastswap_votes_request(
                    "votes",
                    "11",
                    postfiat_types::FastSwapPhaseV1::Precommit,
                    0,
                ),
                RpcRequestKind::FastSwapVotes,
            ),
            (
                fastswap_new_round_vote_request("new-round", "11", 1),
                RpcRequestKind::FastSwapNewRoundVote,
            ),
            (
                fastswap_propose_round_request("propose", "{}"),
                RpcRequestKind::FastSwapProposeRound,
            ),
            (fastswap_precommit_request("precommit", "{}"), RpcRequestKind::FastSwapPrecommit),
            (
                fastswap_commit_round_request("commit-round", "{}"),
                RpcRequestKind::FastSwapCommitRound,
            ),
            (
                fastswap_cancel_apply_request("cancel", "{}"),
                RpcRequestKind::FastSwapCancelApply,
            ),
            (fastlane_exit_request("exit", "{}"), RpcRequestKind::FastLaneExit),
            (
                fastswap_checkpoint_status_request("checkpoint", None),
                RpcRequestKind::FastSwapCheckpointStatus,
            ),
            (
                mempool_submit_fastlane_primary_request("primary", "{}"),
                RpcRequestKind::MempoolSubmitFastLanePrimary,
            ),
            (
                fastswap_objects_request("objects", "aa", None, None, 10),
                RpcRequestKind::FastSwapObjects,
            ),
            (
                fastswap_policy_by_hash_request("policy", "aa"),
                RpcRequestKind::FastSwapPolicy,
            ),
            (
                fastlane_asset_control_preview_request("control-preview", "{}"),
                RpcRequestKind::FastLaneAssetControlPreview,
            ),
            (
                fastlane_asset_control_prepare_request("control-prepare", "{}"),
                RpcRequestKind::FastLaneAssetControlPrepare,
            ),
            (
                fastlane_asset_control_apply_request("control-apply", "{}", "{}"),
                RpcRequestKind::FastLaneAssetControlApply,
            ),
            (
                fastlane_asset_control_catch_up_request("control-catch", "{}", "{}", "{}"),
                RpcRequestKind::FastLaneAssetControlCatchUp,
            ),
        ];
        for (request, kind) in cases {
            validate_request(&request, Some(&request.id), Some(kind))
                .unwrap_or_else(|error| panic!("{} failed validation: {error:?}", request.method));
        }

        let mut extra = fastswap_apply_request("bad", "{}", "{}");
        extra.params.as_object_mut().expect("params").insert("unsafe".to_owned(), json!(true));
        assert!(validate_request(&extra, None, Some(RpcRequestKind::FastSwapApply)).is_err());
    }

    #[test]
    fn fastlane_primary_finality_request_uses_exact_public_method() {
        let request = mempool_submit_fastlane_primary_finality_request("primary-finality", "{}");
        request.validate_protocol().expect("valid request envelope");
        assert_eq!(request.method, METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY_FINALITY);
        assert_eq!(request.params["fastlane_primary_json"], "{}");
    }

    #[test]
    fn atomic_swap_transaction_tx_id_is_canonical_and_field_bound() {
        let swap = valid_signed_atomic_swap();
        let tx_id = atomic_swap_transaction_tx_id(&swap);
        let expected = hash_hex(
            postfiat_types::ATOMIC_SWAP_TRANSACTION_TX_ID_DOMAIN,
            &swap.tx_id_preimage_bytes(),
        );
        assert_eq!(tx_id, expected);
        assert_eq!(
            tx_id,
            "f38e8cde0690ed857ba747de329ca7d9dafd685b62002a989ef946ae923a4e84e35ffbc38e3455668371f8212bccda2a"
        );

        let mut changed = swap.clone();
        changed.unsigned.leg_0.amount += 1;
        assert_ne!(
            atomic_swap_transaction_tx_id(&swap),
            atomic_swap_transaction_tx_id(&changed)
        );
    }

    #[test]
    fn atomic_swap_request_builders_validate_flat_quote_signed_json_and_parent_pins() {
        let swap = valid_signed_atomic_swap();
        let quote = atomic_swap_fee_quote_request(
            "atomic-quote",
            swap.unsigned.rfq_hash.clone(),
            swap.unsigned.market_envelope_hash.clone(),
            swap.unsigned.nav_epoch,
            swap.unsigned.expires_at_height,
            swap.unsigned.swap_nonce.clone(),
            swap.unsigned.leg_0.owner.clone(),
            swap.unsigned.leg_0.recipient.clone(),
            swap.unsigned.leg_0.issuer.clone(),
            swap.unsigned.leg_0.asset_id.clone(),
            swap.unsigned.leg_0.amount,
            swap.unsigned.leg_1.owner.clone(),
            swap.unsigned.leg_1.recipient.clone(),
            swap.unsigned.leg_1.issuer.clone(),
            swap.unsigned.leg_1.asset_id.clone(),
            swap.unsigned.leg_1.amount,
        );
        validate_request(
            &quote,
            Some("atomic-quote"),
            Some(RpcRequestKind::AtomicSwapFeeQuote),
        )
        .expect("valid flat atomic swap quote request");
        assert!(quote.params.get("leg_0").is_none());

        let signed_json = serde_json::to_string(&swap).expect("signed atomic swap json");
        let submit = mempool_submit_signed_atomic_swap_transaction_json_request(
            "atomic-submit",
            signed_json.clone(),
        );
        validate_request(
            &submit,
            Some("atomic-submit"),
            Some(RpcRequestKind::MempoolSubmitSignedAtomicSwapTransaction),
        )
        .expect("valid signed atomic swap submit request");

        let finality = mempool_submit_signed_atomic_swap_transaction_finality_request(
            "atomic-finality",
            signed_json,
            8,
            &"11".repeat(48),
            &"22".repeat(48),
            Some(5_000),
        );
        validate_request(
            &finality,
            Some("atomic-finality"),
            Some(RpcRequestKind::MempoolSubmitSignedAtomicSwapTransactionFinality),
        )
        .expect("valid pinned atomic swap finality request");
    }

    #[test]
    fn atomic_swap_request_validation_rejects_oversize_json() {
        let oversized = mempool_submit_signed_atomic_swap_transaction_json_request(
            "atomic-oversize",
            "x".repeat(MAX_RPC_SIGNED_TRANSFER_JSON_BYTES + 1),
        );
        assert!(matches!(
            validate_request(
                &oversized,
                None,
                Some(RpcRequestKind::MempoolSubmitSignedAtomicSwapTransaction)
            ),
            Err(RpcRequestValidationError::Protocol(
                RpcProtocolError::ParamStringTooLong { key, .. }
            )) if key == "signed_atomic_swap_transaction_json"
        ));
    }

    #[test]
    fn atomic_swap_finality_parent_pins_are_mandatory_and_all_or_none() {
        const HEIGHT: &str = "proxy_required_current_height";
        const STATE_ROOT: &str = "proxy_required_state_root";
        const PARENT_HASH: &str = "proxy_required_parent_hash";
        const PIN_FIELDS: [&str; 3] = [HEIGHT, STATE_ROOT, PARENT_HASH];

        let signed_json = serde_json::to_string(&valid_signed_atomic_swap())
            .expect("signed atomic swap json");
        let pinned = mempool_submit_signed_atomic_swap_transaction_finality_request(
            "atomic-pinned",
            signed_json,
            8,
            &"11".repeat(48),
            &"22".repeat(48),
            None,
        );
        let cases: &[(&str, &[&str], &str)] = &[
            ("no pins", &[], HEIGHT),
            ("height only", &[HEIGHT], STATE_ROOT),
            ("state root only", &[STATE_ROOT], HEIGHT),
            ("parent hash only", &[PARENT_HASH], HEIGHT),
            ("missing parent hash", &[HEIGHT, STATE_ROOT], PARENT_HASH),
            ("missing state root", &[HEIGHT, PARENT_HASH], STATE_ROOT),
            ("missing height", &[STATE_ROOT, PARENT_HASH], HEIGHT),
        ];

        for (case, retained_pins, expected_missing) in cases {
            let mut request = pinned.clone();
            let params = request.params.as_object_mut().expect("request params object");
            for field in PIN_FIELDS {
                if !retained_pins.contains(&field) {
                    params.remove(field);
                }
            }
            assert!(
                matches!(
                    validate_request(
                        &request,
                        None,
                        Some(RpcRequestKind::MempoolSubmitSignedAtomicSwapTransactionFinality)
                    ),
                    Err(RpcRequestValidationError::InvalidParams { field, .. })
                        if field == *expected_missing
                ),
                "partial parent pin case `{case}` was accepted",
            );
        }
    }

    fn valid_signed_payment_v2(hex96: &str) -> serde_json::Value {
        json!({
            "unsigned": {
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "address_namespace": TRANSPARENT_ADDRESS_NAMESPACE,
                "transaction_kind": PAYMENT_V2_TRANSACTION_KIND,
                "signature_algorithm_id": ML_DSA_65_ALGORITHM,
                "from": "pffaucet",
                "to": "pfrecipient",
                "amount": 42,
                "fee": 1,
                "sequence": 1,
                "memos": [{"memo_type": "74657874", "memo_format": "", "memo_data": "6869"}]
            },
            "algorithm_id": ML_DSA_65_ALGORITHM,
            "public_key_hex": hex96,
            "signature_hex": hex96
        })
    }

    fn valid_signed_asset_transaction(hex96: &str) -> serde_json::Value {
        json!({
            "unsigned": {
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "address_namespace": TRANSPARENT_ADDRESS_NAMESPACE,
                "transaction_kind": ASSET_CREATE_TRANSACTION_KIND,
                "signature_algorithm_id": ML_DSA_65_ALGORITHM,
                "source": "pffaucet",
                "fee": 1,
                "sequence": 2,
                "operation": ASSET_CREATE_TRANSACTION_KIND,
                "issuer": "pffaucet",
                "code": "USD",
                "version": 1,
                "precision": 2,
                "display_name": "US Dollar",
                "max_supply": 1000,
                "requires_authorization": false,
                "freeze_enabled": true,
                "clawback_enabled": false
            },
            "algorithm_id": ML_DSA_65_ALGORITHM,
            "public_key_hex": hex96,
            "signature_hex": hex96
        })
    }

    fn valid_signed_pftl_uniswap_destination_consume(hex96: &str) -> serde_json::Value {
        let hex64 = &hex96[..64];
        json!({
            "unsigned": {
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "address_namespace": TRANSPARENT_ADDRESS_NAMESPACE,
                "transaction_kind": PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
                "signature_algorithm_id": ML_DSA_65_ALGORITHM,
                "source": "pfoperator",
                "fee": 1,
                "sequence": 2,
                "operation": PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND,
                "operator": "pfoperator",
                "route_id": "pftl-uniswap-a651",
                "packet_hash": hex96,
                "ethereum_consume_tx_hash": hex64,
                "consumed_height": 14,
                "finalized_height": 26
            },
            "algorithm_id": ML_DSA_65_ALGORITHM,
            "public_key_hex": hex96,
            "signature_hex": hex96
        })
    }

    fn valid_signed_offer_transaction(hex96: &str) -> serde_json::Value {
        json!({
            "unsigned": {
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "address_namespace": TRANSPARENT_ADDRESS_NAMESPACE,
                "transaction_kind": OFFER_CREATE_TRANSACTION_KIND,
                "signature_algorithm_id": ML_DSA_65_ALGORITHM,
                "source": "pfofferowner",
                "fee": 12,
                "sequence": 3,
                "operation": OFFER_CREATE_TRANSACTION_KIND,
                "owner": "pfofferowner",
                "taker_gets_asset_id": "PFT",
                "taker_gets_amount": 25,
                "taker_pays_asset_id": hex96,
                "taker_pays_amount": 10,
                "expiration_height": 50
            },
            "algorithm_id": ML_DSA_65_ALGORITHM,
            "public_key_hex": hex96,
            "signature_hex": hex96
        })
    }

    fn valid_transparent_archive_payload(hex96: &str) -> String {
        json!({
            "batch_id": hex96,
            "transactions": [valid_signed_transfer(hex96)]
        })
        .to_string()
    }

    fn valid_orchard_action_json(fee: u64) -> String {
        json!({
            "pool_id": "orchard-v1",
            "proof_system_id": "orchard-halo2",
            "circuit_id": "orchard-action-v1",
            "flags": 0,
            "anchor": "00".repeat(32),
            "nullifiers": [],
            "rk": [],
            "spend_auth_sigs": [],
            "value_commitments": [],
            "output_commitments": [],
            "encrypted_outputs": [],
            "value_balance": 0,
            "fee": fee,
            "proof": "",
            "binding_signature": "00".repeat(64)
        })
        .to_string()
    }

    fn valid_asset_orchard_swap_json() -> String {
        json!({
            "version": 1,
            "schema": ASSET_ORCHARD_SWAP_ACTION_SCHEMA_V1,
            "pool_id": ASSET_ORCHARD_POOL_ID_V1,
            "proof_system_id": ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
            "circuit_id": ASSET_ORCHARD_CIRCUIT_ID_V1,
            "pool_domain": "00".repeat(32),
            "anchor": "01".repeat(32),
            "nullifiers": ["02".repeat(32), "03".repeat(32)],
            "randomized_verification_keys": ["04".repeat(32), "05".repeat(32)],
            "output_commitments": ["06".repeat(32), "07".repeat(32)],
            "encrypted_outputs": ["08".repeat(64), "09".repeat(64)],
            "pricing_claim": {
                "nav_epoch": 59,
                "reserve_packet_hash": "0e".repeat(48),
                "ratio_numerator": 9,
                "ratio_denominator": 5,
                "mode": "at_nav_with_band",
                "band_bps": 0,
                "base_asset_tag_lo": "0f".repeat(16),
                "base_asset_tag_hi": "10".repeat(16),
                "quote_asset_tag_lo": "11".repeat(16),
                "quote_asset_tag_hi": "12".repeat(16)
            },
            "swap_binding_hash": "0a".repeat(64),
            "fee": 0,
            "proof": "0b".repeat(128),
            "spend_authorization_signatures": ["0c".repeat(64), "0d".repeat(64)]
        })
        .to_string()
    }

    fn valid_block_certificate(hex96: &str) -> serde_json::Value {
        json!({
            "validators": ["validator-0"],
            "quorum": 1,
            "registry_root": hex96,
            "votes": [
                {
                    "vote_id": hex96,
                    "validator": "validator-0",
                    "accept": true,
                    "algorithm_id": ML_DSA_65_ALGORITHM,
                    "registry_root": hex96,
                    "signature_hex": hex96
                }
            ]
        })
    }

    #[test]
    fn response_round_trip() {
        let response = success_response(
            "1",
            &json!({"state_root": "abc"}),
            vec![RpcEvent::new("status", "node", "status queried")],
        )
        .expect("response");
        let json = to_pretty_json(&response).expect("json");
        let parsed: RpcResponse = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed, response);
        assert!(parsed.ok);
        assert_eq!(parsed.version, RPC_VERSION);
    }

    #[test]
    fn response_file_helpers_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "postfiat-rpc-sdk-response-{}-{}.json",
            std::process::id(),
            "round-trip"
        ));
        let response = success_response(
            "status-file",
            &json!({"state_root": "abc", "block_height": 7}),
            vec![RpcEvent::new("status", "validator-0", "status queried")],
        )
        .expect("response");
        write_response_file(&path, &response).expect("write response file");
        let parsed = read_response_file(&path).expect("read response file");
        assert_eq!(parsed, response);
        let decoded = parsed.result_as::<StatusLike>().expect("typed result");
        assert_eq!(
            decoded,
            StatusLike {
                state_root: "abc".to_string(),
                block_height: 7,
            }
        );
        fs::remove_file(path).expect("remove response file");
    }

    #[test]
    fn request_constructs_with_version() {
        let request = RpcRequest::new("2", "status", json!({}));
        assert_eq!(request.version, RPC_VERSION);
        assert_eq!(request.method, "status");
    }

    #[test]
    fn typed_request_builders_emit_object_params() {
        let status = status_request("status-1");
        assert_eq!(status.version, RPC_VERSION);
        assert_eq!(status.method, METHOD_STATUS);
        assert_eq!(status.params, json!({}));

        let server_info = server_info_request("server-info-1");
        assert_eq!(server_info.method, METHOD_SERVER_INFO);
        assert_eq!(server_info.params, json!({}));

        let metrics = metrics_request("metrics-1");
        assert_eq!(metrics.method, METHOD_METRICS);
        assert_eq!(metrics.params, json!({}));

        let ledger = ledger_request("ledger-1", Some(2));
        assert_eq!(ledger.method, METHOD_LEDGER);
        assert_eq!(ledger.params, json!({"limit": 2}));

        let state = verify_state_request("state-1");
        assert_eq!(state.method, METHOD_VERIFY_STATE);
        assert_eq!(state.params, json!({}));

        let keys = validate_local_keys_request("keys-1", 4);
        assert_eq!(keys.method, METHOD_VALIDATE_LOCAL_KEYS);
        assert_eq!(keys.params, json!({"validators": 4}));

        let account = account_request("account-1", "pf1-test");
        assert_eq!(account.method, METHOD_ACCOUNT);
        assert_eq!(account.params, json!({"address": "pf1-test"}));

        let account_tx = account_tx_request("account-tx-1", "pf1-test", Some(2), Some(9), Some(5));
        assert_eq!(account_tx.method, METHOD_ACCOUNT_TX);
        assert_eq!(
            account_tx.params,
            json!({"address": "pf1-test", "from_height": 2, "to_height": 9, "limit": 5})
        );

        let fee = fee_request("fee-1");
        assert_eq!(fee.method, METHOD_FEE);
        assert_eq!(fee.params, json!({}));

        let fee_quote =
            transfer_fee_quote_request("fee-quote-1", "pf1-sender", "pf1-recipient", 25, Some(3));
        assert_eq!(fee_quote.method, METHOD_TRANSFER_FEE_QUOTE);
        assert_eq!(
            fee_quote.params,
            json!({"from": "pf1-sender", "to": "pf1-recipient", "amount": 25, "sequence": 3})
        );

        let asset_operation = json!({
            "operation": ASSET_CREATE_TRANSACTION_KIND,
            "issuer": "pf1-sender",
            "code": "USD",
            "version": 1,
            "precision": 2
        })
        .to_string();
        let asset_fee_quote = asset_fee_quote_request(
            "asset-fee-quote-1",
            "pf1-sender",
            asset_operation.clone(),
            Some(4),
        );
        assert_eq!(asset_fee_quote.method, METHOD_ASSET_FEE_QUOTE);
        assert_eq!(
            asset_fee_quote.params,
            json!({"source": "pf1-sender", "operation_json": asset_operation, "sequence": 4})
        );

        let offer_operation = json!({
            "operation": OFFER_CREATE_TRANSACTION_KIND,
            "owner": "pf1-owner",
            "taker_gets_asset_id": "PFT",
            "taker_gets_amount": 25,
            "taker_pays_asset_id": "ab".repeat(48),
            "taker_pays_amount": 10,
            "expiration_height": 50
        })
        .to_string();
        let offer_fee_quote = offer_fee_quote_request(
            "offer-fee-quote-1",
            "pf1-owner",
            offer_operation.clone(),
            Some(5),
        );
        assert_eq!(offer_fee_quote.method, METHOD_OFFER_FEE_QUOTE);
        assert_eq!(
            offer_fee_quote.params,
            json!({"source": "pf1-owner", "operation_json": offer_operation, "sequence": 5})
        );

        let offer_id = "ef".repeat(48);
        let offer_info = offer_info_request("offer-info-1", offer_id.clone());
        assert_eq!(offer_info.method, METHOD_OFFER_INFO);
        assert_eq!(offer_info.params, json!({"offer_id": offer_id}));

        let account_offers = account_offers_request(
            "account-offers-1",
            "pf1-owner",
            Some(OFFER_STATE_OPEN),
            Some(7),
        );
        assert_eq!(account_offers.method, METHOD_ACCOUNT_OFFERS);
        assert_eq!(
            account_offers.params,
            json!({"account": "pf1-owner", "state": "open", "limit": 7})
        );

        let book_offers = book_offers_request("book-offers-1", "PFT", "ab".repeat(48), Some(8));
        assert_eq!(book_offers.method, METHOD_BOOK_OFFERS);
        assert_eq!(
            book_offers.params,
            json!({"taker_gets_asset_id": "PFT", "taker_pays_asset_id": "ab".repeat(48), "limit": 8})
        );

        let escrow_id = "ab".repeat(48);
        let escrow_info = escrow_info_request("escrow-info-1", escrow_id.clone());
        assert_eq!(escrow_info.method, METHOD_ESCROW_INFO);
        assert_eq!(escrow_info.params, json!({"escrow_id": escrow_id}));

        let account_escrows = account_escrows_request(
            "account-escrows-1",
            "pf1-owner",
            Some("owner"),
            Some(ESCROW_STATE_OPEN),
            Some(5),
        );
        assert_eq!(account_escrows.method, METHOD_ACCOUNT_ESCROWS);
        assert_eq!(
            account_escrows.params,
            json!({"account": "pf1-owner", "role": "owner", "state": "open", "limit": 5})
        );

        let nft_id = "cd".repeat(48);
        let nft_info = nft_info_request("nft-info-1", nft_id.clone());
        assert_eq!(nft_info.method, METHOD_NFT_INFO);
        assert_eq!(nft_info.params, json!({"nft_id": nft_id}));

        let account_nfts = account_nfts_request("account-nfts-1", "pf1-owner", Some(true), Some(5));
        assert_eq!(account_nfts.method, METHOD_ACCOUNT_NFTS);
        assert_eq!(
            account_nfts.params,
            json!({"account": "pf1-owner", "include_burned": true, "limit": 5})
        );

        let issuer_nfts = issuer_nfts_request(
            "issuer-nfts-1",
            "pf1-issuer",
            Some("collection-1"),
            Some(false),
            Some(6),
        );
        assert_eq!(issuer_nfts.method, METHOD_ISSUER_NFTS);
        assert_eq!(
            issuer_nfts.params,
            json!({
                "issuer": "pf1-issuer",
                "collection_id": "collection-1",
                "include_burned": false,
                "limit": 6
            })
        );

        let receipts = receipts_request("receipts-1", Some("tx-1"), Some(5));
        assert_eq!(receipts.method, METHOD_RECEIPTS);
        assert_eq!(receipts.params, json!({"tx_id": "tx-1", "limit": 5}));

        let blocks = blocks_request("blocks-1", Some(3));
        assert_eq!(blocks.method, METHOD_BLOCKS);
        assert_eq!(blocks.params, json!({"limit": 3}));

        let blocks_from_height = blocks_request_from_height("blocks-from-1", Some(7), Some(3));
        assert_eq!(blocks_from_height.method, METHOD_BLOCKS);
        assert_eq!(
            blocks_from_height.params,
            json!({"from_height": 7, "limit": 3})
        );

        let validators = validators_request("validators-1");
        assert_eq!(validators.method, METHOD_VALIDATORS);
        assert_eq!(validators.params, json!({}));

        let manifests = manifests_request("manifests-1");
        assert_eq!(manifests.method, METHOD_MANIFESTS);
        assert_eq!(manifests.params, json!({}));

        let archive =
            batch_archive_request("archive-1", Some("transparent"), Some("batch-1"), Some(2));
        assert_eq!(archive.method, METHOD_BATCH_ARCHIVE);
        assert_eq!(
            archive.params,
            json!({"batch_kind": "transparent", "batch_id": "batch-1", "limit": 2})
        );

        let mempool_submit =
            mempool_submit_transfer_request("mempool-submit-1", "pf1-recipient", 42, None);
        assert_eq!(mempool_submit.method, METHOD_MEMPOOL_SUBMIT_TRANSFER);
        assert_eq!(
            mempool_submit.params,
            json!({"to": "pf1-recipient", "amount": 42})
        );

        let mempool_submit_with_key = mempool_submit_transfer_request(
            "mempool-submit-2",
            "pf1-recipient",
            7,
            Some("devnet/local/node0/faucet_key.json"),
        );
        assert_eq!(
            mempool_submit_with_key.params,
            json!({
                "to": "pf1-recipient",
                "amount": 7,
                "key_file": "devnet/local/node0/faucet_key.json"
            })
        );

        let asset_submit = mempool_submit_signed_asset_transaction_json_request(
            "mempool-submit-asset-1",
            "{\"unsigned\":{\"transaction_kind\":\"asset_create\"}}",
        );
        assert_eq!(
            asset_submit.method,
            METHOD_MEMPOOL_SUBMIT_SIGNED_ASSET_TRANSACTION
        );
        assert_eq!(
            asset_submit.params,
            json!({"signed_asset_transaction_json": "{\"unsigned\":{\"transaction_kind\":\"asset_create\"}}"})
        );

        let offer_submit = mempool_submit_signed_offer_transaction_json_request(
            "mempool-submit-offer-1",
            "{\"unsigned\":{\"transaction_kind\":\"offer_create\"}}",
        );
        assert_eq!(
            offer_submit.method,
            METHOD_MEMPOOL_SUBMIT_SIGNED_OFFER_TRANSACTION
        );
        assert_eq!(
            offer_submit.params,
            json!({"signed_offer_transaction_json": "{\"unsigned\":{\"transaction_kind\":\"offer_create\"}}"})
        );

        let escrow_fee_quote = escrow_fee_quote_request(
            "escrow-fee-quote-1",
            "pf1-owner",
            "{\"operation\":\"escrow_create\"}",
            Some(4),
        );
        assert_eq!(escrow_fee_quote.method, METHOD_ESCROW_FEE_QUOTE);
        assert_eq!(
            escrow_fee_quote.params,
            json!({
                "source": "pf1-owner",
                "operation_json": "{\"operation\":\"escrow_create\"}",
                "sequence": 4
            })
        );

        let issued_asset_id = "cd".repeat(48);
        let atomic_template = atomic_settlement_template_request(
            "atomic-template-1",
            "pf1-pft-owner",
            "pf1-issued-owner",
            "PFT",
            100,
            "pf1-issued-owner",
            "pf1-pft-owner",
            issued_asset_id.clone(),
            25,
            "shared-secret",
            7,
            12,
            Some(4),
            Some(5),
        );
        assert_eq!(atomic_template.method, METHOD_ATOMIC_SETTLEMENT_TEMPLATE);
        assert_eq!(
            atomic_template.params,
            json!({
                "left_owner": "pf1-pft-owner",
                "left_recipient": "pf1-issued-owner",
                "left_asset_id": "PFT",
                "left_amount": 100,
                "right_owner": "pf1-issued-owner",
                "right_recipient": "pf1-pft-owner",
                "right_asset_id": issued_asset_id,
                "right_amount": 25,
                "condition": "shared-secret",
                "finish_after": 7,
                "cancel_after": 12,
                "left_sequence": 4,
                "right_sequence": 5
            })
        );

        let escrow_submit = mempool_submit_signed_escrow_transaction_json_request(
            "mempool-submit-escrow-1",
            "{\"unsigned\":{\"transaction_kind\":\"escrow_create\"}}",
        );
        assert_eq!(
            escrow_submit.method,
            METHOD_MEMPOOL_SUBMIT_SIGNED_ESCROW_TRANSACTION
        );
        assert_eq!(
            escrow_submit.params,
            json!({"signed_escrow_transaction_json": "{\"unsigned\":{\"transaction_kind\":\"escrow_create\"}}"})
        );

        let mempool_status = mempool_status_request("mempool-status-1");
        assert_eq!(mempool_status.method, METHOD_MEMPOOL_STATUS);
        assert_eq!(mempool_status.params, json!({}));

        let mempool_batch = mempool_batch_request(
            "mempool-batch-1",
            "devnet/local/batches/batch.json",
            Some(10),
        );
        assert_eq!(mempool_batch.method, METHOD_MEMPOOL_BATCH);
        assert_eq!(
            mempool_batch.params,
            json!({"batch_file": "devnet/local/batches/batch.json", "max_transactions": 10})
        );

        let apply_batch = apply_batch_request("apply-batch-1", "devnet/local/batches/batch.json");
        assert_eq!(apply_batch.method, METHOD_APPLY_BATCH);
        assert_eq!(
            apply_batch.params,
            json!({"batch_file": "devnet/local/batches/batch.json"})
        );

        let shield_mint = shield_batch_mint_request(
            "shield-mint-1",
            "alice",
            250,
            Some("POSTFIAT"),
            Some("sdk shield mint"),
            "devnet/local/batches/shield-mint.json",
        );
        assert_eq!(shield_mint.method, METHOD_SHIELD_BATCH_MINT);
        assert_eq!(
            shield_mint.params,
            json!({
                "owner": "alice",
                "amount": 250,
                "asset_id": "POSTFIAT",
                "memo": "sdk shield mint",
                "batch_file": "devnet/local/batches/shield-mint.json"
            })
        );

        let shield_spend = shield_batch_spend_request(
            "shield-spend-1",
            "note-1",
            "bob",
            125,
            Some("sdk shield spend"),
            "devnet/local/batches/shield-spend.json",
        );
        assert_eq!(shield_spend.method, METHOD_SHIELD_BATCH_SPEND);
        assert_eq!(
            shield_spend.params,
            json!({
                "note_id": "note-1",
                "to": "bob",
                "amount": 125,
                "memo": "sdk shield spend",
                "batch_file": "devnet/local/batches/shield-spend.json"
            })
        );

        let shield_migrate = shield_batch_migrate_request(
            "shield-migrate-1",
            "note-1",
            "debug-shielded-pool-v2",
            Some("sdk shield migrate"),
            "devnet/local/batches/shield-migrate.json",
        );
        assert_eq!(shield_migrate.method, METHOD_SHIELD_BATCH_MIGRATE);
        assert_eq!(
            shield_migrate.params,
            json!({
                "note_id": "note-1",
                "target_pool": "debug-shielded-pool-v2",
                "memo": "sdk shield migrate",
                "batch_file": "devnet/local/batches/shield-migrate.json"
            })
        );

        let shield_orchard = shield_batch_orchard_request(
            "shield-orchard-1",
            "devnet/local/batches/orchard.action.json",
            "devnet/local/batches/orchard.batch.json",
        );
        assert_eq!(shield_orchard.method, METHOD_SHIELD_BATCH_ORCHARD);
        assert_eq!(
            shield_orchard.params,
            json!({
                "action_file": "devnet/local/batches/orchard.action.json",
                "batch_file": "devnet/local/batches/orchard.batch.json"
            })
        );
        let orchard_action_json = valid_orchard_action_json(0);
        let shield_orchard_json =
            shield_batch_orchard_json_request("shield-orchard-json-1", orchard_action_json.clone());
        assert_eq!(shield_orchard_json.method, METHOD_SHIELD_BATCH_ORCHARD);
        assert_eq!(
            shield_orchard_json.params,
            json!({"action_json": orchard_action_json})
        );

        let shield_orchard_withdraw = shield_batch_orchard_withdraw_request(
            "shield-orchard-withdraw-1",
            "devnet/local/batches/orchard-withdraw.action.json",
            "pffaucet",
            25,
            0,
            Some(ORCHARD_WITHDRAW_POLICY_ID),
            None,
            "devnet/local/batches/orchard-withdraw.batch.json",
        );
        assert_eq!(
            shield_orchard_withdraw.method,
            METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW
        );
        assert_eq!(
            shield_orchard_withdraw.params,
            json!({
                "action_file": "devnet/local/batches/orchard-withdraw.action.json",
                "to": "pffaucet",
                "amount": 25,
                "fee": 0,
                "policy_id": ORCHARD_WITHDRAW_POLICY_ID,
                "batch_file": "devnet/local/batches/orchard-withdraw.batch.json"
            })
        );
        let orchard_withdraw_action_json = valid_orchard_action_json(3);
        let shield_orchard_withdraw_json = shield_batch_orchard_withdraw_json_request(
            "shield-orchard-withdraw-json-1",
            orchard_withdraw_action_json.clone(),
            "pffaucet",
            25,
            3,
            Some(ORCHARD_WITHDRAW_POLICY_ID),
            None,
        );
        assert_eq!(
            shield_orchard_withdraw_json.method,
            METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW
        );
        assert_eq!(
            shield_orchard_withdraw_json.params,
            json!({
                "action_json": orchard_withdraw_action_json,
                "to": "pffaucet",
                "amount": 25,
                "fee": 3,
                "policy_id": ORCHARD_WITHDRAW_POLICY_ID
            })
        );

        let shield_swap = shield_batch_swap_request(
            "shield-swap-1",
            "devnet/local/batches/asset-orchard-swap.json",
            "devnet/local/batches/asset-orchard-swap.batch.json",
        );
        assert_eq!(shield_swap.method, METHOD_SHIELD_BATCH_SWAP);
        assert_eq!(
            shield_swap.params,
            json!({
                "swap_file": "devnet/local/batches/asset-orchard-swap.json",
                "batch_file": "devnet/local/batches/asset-orchard-swap.batch.json"
            })
        );
        let asset_orchard_swap_json = valid_asset_orchard_swap_json();
        let shield_swap_json =
            shield_batch_swap_json_request("shield-swap-json-1", asset_orchard_swap_json.clone());
        assert_eq!(shield_swap_json.method, METHOD_SHIELD_BATCH_SWAP);
        assert_eq!(
            shield_swap_json.params,
            json!({"swap_json": asset_orchard_swap_json})
        );

        let apply_shield =
            apply_shield_batch_request("apply-shield-1", "devnet/local/batches/shield.json");
        assert_eq!(apply_shield.method, METHOD_APPLY_SHIELD_BATCH);
        assert_eq!(
            apply_shield.params,
            json!({"batch_file": "devnet/local/batches/shield.json"})
        );

        let shield_scan = shield_scan_request("shield-scan-1", "alice");
        assert_eq!(shield_scan.method, METHOD_SHIELD_SCAN);
        assert_eq!(shield_scan.params, json!({"owner": "alice"}));

        let shield_disclose = shield_disclose_request("shield-disclose-1", "note-1");
        assert_eq!(shield_disclose.method, METHOD_SHIELD_DISCLOSE);
        assert_eq!(shield_disclose.params, json!({"note_id": "note-1"}));

        let shield_turnstile = shield_turnstile_request("shield-turnstile-1");
        assert_eq!(shield_turnstile.method, METHOD_SHIELD_TURNSTILE);
        assert_eq!(shield_turnstile.params, json!({}));

        let bridge_status = bridge_status_request("bridge-status-1");
        assert_eq!(bridge_status.method, METHOD_BRIDGE_STATUS);
        assert_eq!(bridge_status.params, json!({}));

        let bridge_domain = bridge_batch_domain_request(
            "bridge-domain-1",
            BridgeBatchDomainParams {
                domain_id: "local-sim".to_string(),
                name: "Local Simulation".to_string(),
                source_chain: Some("xrpl-devnet".to_string()),
                target_chain: Some("postfiat-local".to_string()),
                bridge_id: Some("local-sim".to_string()),
                door_account: Some("door:local-sim".to_string()),
                inbound_cap: 100,
                outbound_cap: 60,
                batch_file: "devnet/local/batches/bridge-domain.json".to_string(),
            },
        );
        assert_eq!(bridge_domain.method, METHOD_BRIDGE_BATCH_DOMAIN);
        assert_eq!(
            bridge_domain.params,
            json!({
                "domain_id": "local-sim",
                "name": "Local Simulation",
                "source_chain": "xrpl-devnet",
                "target_chain": "postfiat-local",
                "bridge_id": "local-sim",
                "door_account": "door:local-sim",
                "inbound_cap": 100,
                "outbound_cap": 60,
                "batch_file": "devnet/local/batches/bridge-domain.json"
            })
        );

        let bridge_transfer = bridge_batch_transfer_request(
            "bridge-transfer-1",
            BridgeBatchTransferParams {
                domain_id: "local-sim".to_string(),
                direction: "inbound".to_string(),
                from: "external:alice".to_string(),
                to: "pfrecipient".to_string(),
                asset_id: "POSTFIAT".to_string(),
                amount: 40,
                witness_id: "bridge-witness-1".to_string(),
                witness_epoch: Some(2),
                witness_signer: Some("validator-0".to_string()),
                batch_file: "devnet/local/batches/bridge-transfer.json".to_string(),
            },
        );
        assert_eq!(bridge_transfer.method, METHOD_BRIDGE_BATCH_TRANSFER);
        assert_eq!(
            bridge_transfer.params,
            json!({
                "domain_id": "local-sim",
                "direction": "inbound",
                "from": "external:alice",
                "to": "pfrecipient",
                "asset_id": "POSTFIAT",
                "amount": 40,
                "witness_id": "bridge-witness-1",
                "witness_epoch": 2,
                "witness_signer": "validator-0",
                "batch_file": "devnet/local/batches/bridge-transfer.json"
            })
        );

        let bridge_pause =
            bridge_batch_pause_request("bridge-pause-1", "local-sim", "bridge-pause.json");
        assert_eq!(bridge_pause.method, METHOD_BRIDGE_BATCH_PAUSE);
        assert_eq!(
            bridge_pause.params,
            json!({"domain_id": "local-sim", "batch_file": "bridge-pause.json"})
        );

        let bridge_resume =
            bridge_batch_resume_request("bridge-resume-1", "local-sim", "bridge-resume.json");
        assert_eq!(bridge_resume.method, METHOD_BRIDGE_BATCH_RESUME);
        assert_eq!(
            bridge_resume.params,
            json!({"domain_id": "local-sim", "batch_file": "bridge-resume.json"})
        );

        let apply_bridge =
            apply_bridge_batch_request("apply-bridge-1", "devnet/local/batches/bridge.json");
        assert_eq!(apply_bridge.method, METHOD_APPLY_BRIDGE_BATCH);
        assert_eq!(
            apply_bridge.params,
            json!({"batch_file": "devnet/local/batches/bridge.json"})
        );
    }

    #[test]
    fn request_builder_serializes_params_and_round_trips() {
        let request = RpcRequest::empty("account-1", "account")
            .with_param("address", "pf1-test")
            .expect("address param")
            .with_param("limit", 5_u32)
            .expect("limit param");
        let json = request.to_pretty_json().expect("json");
        let parsed = request_from_json(&json).expect("parse");
        assert_eq!(parsed, request);
        assert_eq!(parsed.params, json!({"address": "pf1-test", "limit": 5}));
    }

    #[test]
    fn request_file_helpers_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "postfiat-rpc-sdk-request-{}-{}.json",
            std::process::id(),
            "round-trip"
        ));
        let request = validate_local_keys_request("keys-file", 7);
        write_request_file(&path, &request).expect("write request file");
        let parsed = read_request_file(&path).expect("read request file");
        assert_eq!(parsed, request);
        fs::remove_file(path).expect("remove request file");
    }

    #[test]
    fn request_validation_accepts_supported_kinds() {
        let hex96 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        validate_request(
            &status_request("status-1"),
            Some("status-1"),
            Some(RpcRequestKind::Status),
        )
        .expect("status request");
        validate_request(
            &server_info_request("server-info-1"),
            Some("server-info-1"),
            Some(RpcRequestKind::ServerInfo),
        )
        .expect("server info request");
        validate_request(
            &metrics_request("metrics-1"),
            Some("metrics-1"),
            Some(RpcRequestKind::Metrics),
        )
        .expect("metrics request");
        validate_request(
            &ledger_request("ledger-1", Some(2)),
            Some("ledger-1"),
            Some(RpcRequestKind::Ledger),
        )
        .expect("ledger request");
        validate_request(
            &verify_state_request("state-1"),
            Some("state-1"),
            Some(RpcRequestKind::VerifyState),
        )
        .expect("state request");
        validate_request(
            &validate_local_keys_request("keys-1", 4),
            Some("keys-1"),
            Some(RpcRequestKind::ValidateLocalKeys {
                validators: Some(4),
            }),
        )
        .expect("local key request");
        validate_request(
            &account_request("account-1", "pf1-test"),
            Some("account-1"),
            Some(RpcRequestKind::Account),
        )
        .expect("account request");
        validate_request(
            &account_tx_request("account-tx-1", "pf1-test", Some(0), Some(9), Some(5)),
            Some("account-tx-1"),
            Some(RpcRequestKind::AccountTx),
        )
        .expect("account_tx request");
        validate_request(
            &fee_request("fee-1"),
            Some("fee-1"),
            Some(RpcRequestKind::Fee),
        )
        .expect("fee request");
        validate_request(
            &transfer_fee_quote_request("fee-quote-1", "pf1-sender", "pf1-recipient", 25, None),
            Some("fee-quote-1"),
            Some(RpcRequestKind::TransferFeeQuote),
        )
        .expect("transfer fee quote request");
        validate_request(
            &asset_fee_quote_request(
                "asset-fee-quote-1",
                "pf1-sender",
                "{\"operation\":\"asset_create\",\"issuer\":\"pf1-sender\",\"code\":\"USD\",\"version\":1,\"precision\":2}",
                None,
            ),
            Some("asset-fee-quote-1"),
            Some(RpcRequestKind::AssetFeeQuote),
        )
        .expect("asset fee quote request");
        validate_request(
            &offer_fee_quote_request(
                "offer-fee-quote-1",
                "pf1-owner",
                "{\"operation\":\"offer_create\",\"owner\":\"pf1-owner\",\"taker_gets_asset_id\":\"PFT\",\"taker_gets_amount\":25,\"taker_pays_asset_id\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"taker_pays_amount\":10,\"expiration_height\":50}",
                None,
            ),
            Some("offer-fee-quote-1"),
            Some(RpcRequestKind::OfferFeeQuote),
        )
        .expect("offer fee quote request");
        validate_request(
            &offer_info_request("offer-info-1", hex96),
            Some("offer-info-1"),
            Some(RpcRequestKind::OfferInfo),
        )
        .expect("offer info request");
        validate_request(
            &account_offers_request(
                "account-offers-1",
                "pf1-owner",
                Some(OFFER_STATE_OPEN),
                Some(5),
            ),
            Some("account-offers-1"),
            Some(RpcRequestKind::AccountOffers),
        )
        .expect("account offers request");
        validate_request(
            &book_offers_request("book-offers-1", "PFT", hex96, Some(5)),
            Some("book-offers-1"),
            Some(RpcRequestKind::BookOffers),
        )
        .expect("book offers request");
        validate_request(
            &asset_info_request("asset-info-1", hex96),
            Some("asset-info-1"),
            Some(RpcRequestKind::AssetInfo),
        )
        .expect("asset info request");
        validate_request(
            &account_lines_request(
                "account-lines-1",
                "pf1-holder",
                Some("pf1-issuer"),
                Some(hex96),
                Some(5),
            ),
            Some("account-lines-1"),
            Some(RpcRequestKind::AccountLines),
        )
        .expect("account lines request");
        validate_request(
            &account_assets_request("account-assets-1", "pf1-holder", Some(hex96), Some(5)),
            Some("account-assets-1"),
            Some(RpcRequestKind::AccountAssets),
        )
        .expect("account assets request");
        validate_request(
            &issuer_assets_request("issuer-assets-1", "pf1-issuer", Some(5)),
            Some("issuer-assets-1"),
            Some(RpcRequestKind::IssuerAssets),
        )
        .expect("issuer assets request");
        validate_request(
            &atomic_settlement_template_request(
                "atomic-template-1",
                "pf1-pft-owner",
                "pf1-issued-owner",
                "PFT",
                100,
                "pf1-issued-owner",
                "pf1-pft-owner",
                hex96,
                25,
                "shared-secret",
                7,
                12,
                None,
                Some(5),
            ),
            Some("atomic-template-1"),
            Some(RpcRequestKind::AtomicSettlementTemplate),
        )
        .expect("atomic settlement template request");
        validate_request(
            &escrow_info_request("escrow-info-1", hex96),
            Some("escrow-info-1"),
            Some(RpcRequestKind::EscrowInfo),
        )
        .expect("escrow info request");
        validate_request(
            &account_escrows_request(
                "account-escrows-1",
                "pf1-owner",
                Some("owner"),
                Some(ESCROW_STATE_OPEN),
                Some(5),
            ),
            Some("account-escrows-1"),
            Some(RpcRequestKind::AccountEscrows),
        )
        .expect("account escrows request");
        validate_request(
            &nft_info_request("nft-info-1", hex96),
            Some("nft-info-1"),
            Some(RpcRequestKind::NftInfo),
        )
        .expect("nft info request");
        validate_request(
            &account_nfts_request("account-nfts-1", "pf1-owner", Some(true), Some(5)),
            Some("account-nfts-1"),
            Some(RpcRequestKind::AccountNfts),
        )
        .expect("account nfts request");
        validate_request(
            &issuer_nfts_request(
                "issuer-nfts-1",
                "pf1-issuer",
                Some("collection-1"),
                Some(false),
                Some(5),
            ),
            Some("issuer-nfts-1"),
            Some(RpcRequestKind::IssuerNfts),
        )
        .expect("issuer nfts request");
        validate_request(
            &receipts_request("receipts-1", Some(hex96), Some(5)),
            Some("receipts-1"),
            Some(RpcRequestKind::Receipts),
        )
        .expect("receipts request");
        validate_request(
            &tx_request("tx-1", hex96),
            Some("tx-1"),
            Some(RpcRequestKind::Tx),
        )
        .expect("tx request");
        validate_request(
            &tx_request_with_audit("tx-audit-1", hex96, true),
            Some("tx-audit-1"),
            Some(RpcRequestKind::Tx),
        )
        .expect("audited tx request");
        validate_request(
            &blocks_request("blocks-1", Some(3)),
            Some("blocks-1"),
            Some(RpcRequestKind::Blocks),
        )
        .expect("blocks request");
        validate_request(
            &blocks_request_from_height("blocks-from-1", Some(0), Some(3)),
            Some("blocks-from-1"),
            Some(RpcRequestKind::Blocks),
        )
        .expect("blocks from_height request");
        validate_request(
            &validators_request("validators-1"),
            Some("validators-1"),
            Some(RpcRequestKind::Validators),
        )
        .expect("validators request");
        validate_request(
            &manifests_request("manifests-1"),
            Some("manifests-1"),
            Some(RpcRequestKind::Manifests),
        )
        .expect("manifests request");
        validate_request(
            &batch_archive_request("archive-1", Some("transparent"), Some(hex96), Some(2)),
            Some("archive-1"),
            Some(RpcRequestKind::BatchArchive),
        )
        .expect("batch archive request");
        validate_request(
            &archive_window_request("archive-window-1", 1, 2, Some("archive://window")),
            Some("archive-window-1"),
            Some(RpcRequestKind::ArchiveWindow),
        )
        .expect("archive window request");
        validate_request(
            &mempool_submit_transfer_request("mempool-submit-1", "pf1-recipient", 42, None),
            Some("mempool-submit-1"),
            Some(RpcRequestKind::MempoolSubmitTransfer),
        )
        .expect("mempool submit request");
        validate_request(
            &mempool_submit_signed_transfer_request(
                "mempool-submit-signed-1",
                "devnet/local/batches/external-transfer.json",
            ),
            Some("mempool-submit-signed-1"),
            Some(RpcRequestKind::MempoolSubmitSignedTransfer),
        )
        .expect("mempool signed submit request");
        validate_request(
            &mempool_submit_signed_transfer_json_request(
                "mempool-submit-signed-json-1",
                "{\"unsigned\":{\"to\":\"pf1-recipient\"}}",
            ),
            Some("mempool-submit-signed-json-1"),
            Some(RpcRequestKind::MempoolSubmitSignedTransfer),
        )
        .expect("mempool signed JSON submit request");
        validate_request(
            &mempool_submit_signed_payment_v2_json_request(
                "mempool-submit-payment-v2-1",
                "{\"unsigned\":{\"transaction_kind\":\"payment_v2\"}}",
            ),
            Some("mempool-submit-payment-v2-1"),
            Some(RpcRequestKind::MempoolSubmitSignedPaymentV2),
        )
        .expect("mempool signed payment v2 JSON submit request");
        validate_request(
            &mempool_submit_signed_asset_transaction_json_request(
                "mempool-submit-asset-1",
                "{\"unsigned\":{\"transaction_kind\":\"asset_create\"}}",
            ),
            Some("mempool-submit-asset-1"),
            Some(RpcRequestKind::MempoolSubmitSignedAssetTransaction),
        )
        .expect("mempool signed asset transaction JSON submit request");
        validate_request(
            &mempool_submit_signed_offer_transaction_json_request(
                "mempool-submit-offer-1",
                "{\"unsigned\":{\"transaction_kind\":\"offer_create\"}}",
            ),
            Some("mempool-submit-offer-1"),
            Some(RpcRequestKind::MempoolSubmitSignedOfferTransaction),
        )
        .expect("mempool signed offer transaction JSON submit request");
        validate_request(
            &mempool_status_request("mempool-status-1"),
            Some("mempool-status-1"),
            Some(RpcRequestKind::MempoolStatus),
        )
        .expect("mempool status request");
        validate_request(
            &mempool_batch_request(
                "mempool-batch-1",
                "devnet/local/batches/batch.json",
                Some(10),
            ),
            Some("mempool-batch-1"),
            Some(RpcRequestKind::MempoolBatch),
        )
        .expect("mempool batch request");
        validate_request(
            &apply_batch_request("apply-batch-1", "devnet/local/batches/batch.json"),
            Some("apply-batch-1"),
            Some(RpcRequestKind::ApplyBatch),
        )
        .expect("apply batch request");
        validate_request(
            &shield_batch_mint_request(
                "shield-mint-1",
                "alice",
                250,
                Some("POSTFIAT"),
                Some("sdk shield mint"),
                "devnet/local/batches/shield-mint.json",
            ),
            Some("shield-mint-1"),
            Some(RpcRequestKind::ShieldBatchMint),
        )
        .expect("shield mint batch request");
        validate_request(
            &shield_batch_spend_request(
                "shield-spend-1",
                "note-1",
                "bob",
                125,
                Some("sdk shield spend"),
                "devnet/local/batches/shield-spend.json",
            ),
            Some("shield-spend-1"),
            Some(RpcRequestKind::ShieldBatchSpend),
        )
        .expect("shield spend batch request");
        validate_request(
            &shield_batch_migrate_request(
                "shield-migrate-1",
                "note-1",
                "debug-shielded-pool-v2",
                Some("sdk shield migrate"),
                "devnet/local/batches/shield-migrate.json",
            ),
            Some("shield-migrate-1"),
            Some(RpcRequestKind::ShieldBatchMigrate),
        )
        .expect("shield migrate batch request");
        validate_request(
            &shield_batch_orchard_request(
                "shield-orchard-1",
                "devnet/local/batches/orchard.action.json",
                "devnet/local/batches/orchard.batch.json",
            ),
            Some("shield-orchard-1"),
            Some(RpcRequestKind::ShieldBatchOrchard),
        )
        .expect("shield orchard batch request");
        validate_request(
            &shield_batch_orchard_json_request(
                "shield-orchard-json-1",
                valid_orchard_action_json(0),
            ),
            Some("shield-orchard-json-1"),
            Some(RpcRequestKind::ShieldBatchOrchard),
        )
        .expect("shield orchard JSON batch request");
        validate_request(
            &shield_batch_orchard_withdraw_request(
                "shield-orchard-withdraw-1",
                "devnet/local/batches/orchard-withdraw.action.json",
                "pffaucet",
                25,
                0,
                Some(ORCHARD_WITHDRAW_POLICY_ID),
                Some(hex96),
                "devnet/local/batches/orchard-withdraw.batch.json",
            ),
            Some("shield-orchard-withdraw-1"),
            Some(RpcRequestKind::ShieldBatchOrchardWithdraw),
        )
        .expect("shield orchard withdraw batch request");
        validate_request(
            &shield_batch_orchard_withdraw_json_request(
                "shield-orchard-withdraw-json-1",
                valid_orchard_action_json(7),
                "pffaucet",
                25,
                7,
                Some(ORCHARD_WITHDRAW_POLICY_ID),
                Some(hex96),
            ),
            Some("shield-orchard-withdraw-json-1"),
            Some(RpcRequestKind::ShieldBatchOrchardWithdraw),
        )
        .expect("shield orchard withdraw JSON batch request");
        validate_request(
            &shield_batch_swap_json_request("shield-swap-json-1", valid_asset_orchard_swap_json()),
            Some("shield-swap-json-1"),
            Some(RpcRequestKind::ShieldBatchSwap),
        )
        .expect("shield swap JSON batch request");
        validate_request(
            &apply_shield_batch_request("apply-shield-1", "devnet/local/batches/shield.json"),
            Some("apply-shield-1"),
            Some(RpcRequestKind::ApplyShieldBatch),
        )
        .expect("apply shield batch request");
        validate_request(
            &shield_scan_request("shield-scan-1", "alice"),
            Some("shield-scan-1"),
            Some(RpcRequestKind::ShieldScan),
        )
        .expect("shield scan request");
        validate_request(
            &shield_disclose_request("shield-disclose-1", "note-1"),
            Some("shield-disclose-1"),
            Some(RpcRequestKind::ShieldDisclose),
        )
        .expect("shield disclose request");
        validate_request(
            &shield_turnstile_request("shield-turnstile-1"),
            Some("shield-turnstile-1"),
            Some(RpcRequestKind::ShieldTurnstile),
        )
        .expect("shield turnstile request");
        validate_request(
            &bridge_status_request("bridge-status-1"),
            Some("bridge-status-1"),
            Some(RpcRequestKind::BridgeStatus),
        )
        .expect("bridge status request");
        validate_request(
            &navcoin_bridge_routes_request("navcoin-bridge-routes-1"),
            Some("navcoin-bridge-routes-1"),
            Some(RpcRequestKind::NavcoinBridgeRoutes),
        )
        .expect("NAVCoin bridge routes request");
        validate_request(
            &navcoin_bridge_packet_request(
                "navcoin-bridge-packet-1",
                NavcoinBridgePacketParams {
                    route_id: "pftl-a666-ethereum-wA666-usdc-v1".to_string(),
                    packet_hash: hex96.to_string(),
                },
            ),
            Some("navcoin-bridge-packet-1"),
            Some(RpcRequestKind::NavcoinBridgePacket),
        )
        .expect("NAVCoin bridge packet request");
        validate_request(
            &navcoin_bridge_claims_request(
                "navcoin-bridge-claims-1",
                NavcoinBridgeClaimsParams {
                    route_id: "pftl-a666-ethereum-wA666-usdc-v1".to_string(),
                    limit: Some(25),
                    include_terminal: Some(true),
                },
            ),
            Some("navcoin-bridge-claims-1"),
            Some(RpcRequestKind::NavcoinBridgeClaims),
        )
        .expect("NAVCoin bridge claims request");
        validate_request(
            &navcoin_bridge_supply_status_request(
                "navcoin-bridge-supply-1",
                NavcoinBridgeSupplyStatusParams {
                    route_id: "pftl-a666-ethereum-wA666-usdc-v1".to_string(),
                },
            ),
            Some("navcoin-bridge-supply-1"),
            Some(RpcRequestKind::NavcoinBridgeSupplyStatus),
        )
        .expect("NAVCoin bridge supply status request");
        validate_request(
            &navcoin_bridge_receipt_replay_request(
                "navcoin-bridge-replay-1",
                NavcoinBridgeReceiptReplayParams {
                    route_id: "pftl-a666-ethereum-wA666-usdc-v1".to_string(),
                },
            ),
            Some("navcoin-bridge-replay-1"),
            Some(RpcRequestKind::NavcoinBridgeReceiptReplay),
        )
        .expect("NAVCoin bridge receipt replay request");
        validate_request(
            &navcoin_bridge_packet_preflight_request(
                "navcoin-bridge-packet-preflight-1",
                NavcoinBridgePacketPreflightParams {
                    route_id: "pftl-a666-ethereum-wA666-usdc-v1".to_string(),
                    packet_file: "devnet/local/pftl-uniswap/packet.json".to_string(),
                },
            ),
            Some("navcoin-bridge-packet-preflight-1"),
            Some(RpcRequestKind::NavcoinBridgePacketPreflight),
        )
        .expect("NAVCoin bridge packet preflight request");
        validate_request(
            &bridge_batch_domain_request(
                "bridge-domain-1",
                BridgeBatchDomainParams {
                    domain_id: "local-sim".to_string(),
                    name: "Local Simulation".to_string(),
                    source_chain: None,
                    target_chain: None,
                    bridge_id: None,
                    door_account: None,
                    inbound_cap: 100,
                    outbound_cap: 60,
                    batch_file: "devnet/local/batches/bridge-domain.json".to_string(),
                },
            ),
            Some("bridge-domain-1"),
            Some(RpcRequestKind::BridgeBatchDomain),
        )
        .expect("bridge domain batch request");
        validate_request(
            &bridge_batch_transfer_request(
                "bridge-transfer-1",
                BridgeBatchTransferParams {
                    domain_id: "local-sim".to_string(),
                    direction: "inbound".to_string(),
                    from: "external:alice".to_string(),
                    to: "pfrecipient".to_string(),
                    asset_id: "POSTFIAT".to_string(),
                    amount: 40,
                    witness_id: "bridge-witness-1".to_string(),
                    witness_epoch: Some(2),
                    witness_signer: Some("validator-0".to_string()),
                    batch_file: "devnet/local/batches/bridge-transfer.json".to_string(),
                },
            ),
            Some("bridge-transfer-1"),
            Some(RpcRequestKind::BridgeBatchTransfer),
        )
        .expect("bridge transfer batch request");
        validate_request(
            &bridge_batch_pause_request("bridge-pause-1", "local-sim", "bridge-pause.json"),
            Some("bridge-pause-1"),
            Some(RpcRequestKind::BridgeBatchPause),
        )
        .expect("bridge pause batch request");
        validate_request(
            &bridge_batch_resume_request("bridge-resume-1", "local-sim", "bridge-resume.json"),
            Some("bridge-resume-1"),
            Some(RpcRequestKind::BridgeBatchResume),
        )
        .expect("bridge resume batch request");
        validate_request(
            &apply_bridge_batch_request("apply-bridge-1", "devnet/local/batches/bridge.json"),
            Some("apply-bridge-1"),
            Some(RpcRequestKind::ApplyBridgeBatch),
        )
        .expect("apply bridge batch request");
    }

    #[test]
    fn request_validation_rejects_wrong_method_bad_params_and_key_leaks() {
        assert_eq!(
            validate_request(
                &status_request("status-1"),
                Some("status-1"),
                Some(RpcRequestKind::Metrics),
            )
            .expect_err("wrong method"),
            RpcRequestValidationError::UnexpectedMethod {
                expected: METHOD_METRICS.to_string(),
                found: METHOD_STATUS.to_string(),
            }
        );

        let bad_status = RpcRequest::empty("status-1", METHOD_STATUS)
            .with_param("limit", 1_u32)
            .expect("limit param");
        assert!(matches!(
            validate_request(&bad_status, Some("status-1"), Some(RpcRequestKind::Status)),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "params"
        ));

        let bad_keys = RpcRequest::empty("keys-1", METHOD_VALIDATE_LOCAL_KEYS)
            .with_param("validators", 3_u32)
            .expect("validators param");
        assert!(matches!(
            validate_request(
                &bad_keys,
                Some("keys-1"),
                Some(RpcRequestKind::ValidateLocalKeys {
                    validators: Some(4)
                })
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "validators"
        ));

        let leaky_request = RpcRequest::empty("leak-1", METHOD_STATUS)
            .with_param("private_key_hex", "leaked")
            .expect("private key param");
        assert!(matches!(
            validate_request(&leaky_request, Some("leak-1"), None),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "params"
        ));

        let leaky_orchard_request = RpcRequest::empty("leak-2", METHOD_STATUS)
            .with_param("spending_key_hex", "leaked")
            .expect("spending key param");
        assert!(matches!(
            validate_request(&leaky_orchard_request, Some("leak-2"), None),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "params"
        ));

        let missing_address = RpcRequest::empty("account-1", METHOD_ACCOUNT);
        assert!(matches!(
            validate_request(
                &missing_address,
                Some("account-1"),
                Some(RpcRequestKind::Account)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "address"
        ));

        let bad_account_tx_range =
            account_tx_request("account-tx-1", "pf1-test", Some(9), Some(2), Some(5));
        assert!(matches!(
            validate_request(
                &bad_account_tx_range,
                Some("account-tx-1"),
                Some(RpcRequestKind::AccountTx)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "to_height"
        ));

        let zero_fee_quote_amount =
            transfer_fee_quote_request("fee-quote-1", "pf1-sender", "pf1-recipient", 0, None);
        assert!(matches!(
            validate_request(
                &zero_fee_quote_amount,
                Some("fee-quote-1"),
                Some(RpcRequestKind::TransferFeeQuote)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "amount"
        ));

        let bad_limit = RpcRequest::empty("blocks-1", METHOD_BLOCKS)
            .with_param("limit", "five")
            .expect("limit param");
        assert!(matches!(
            validate_request(&bad_limit, Some("blocks-1"), Some(RpcRequestKind::Blocks)),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "limit"
        ));

        let bad_from_height = RpcRequest::empty("blocks-1", METHOD_BLOCKS)
            .with_param("from_height", "zero")
            .expect("from_height param");
        assert!(matches!(
            validate_request(
                &bad_from_height,
                Some("blocks-1"),
                Some(RpcRequestKind::Blocks)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "from_height"
        ));

        let zero_receipts_limit = RpcRequest::empty("receipts-1", METHOD_RECEIPTS)
            .with_param("limit", 0_usize)
            .expect("limit param");
        assert!(matches!(
            validate_request(
                &zero_receipts_limit,
                Some("receipts-1"),
                Some(RpcRequestKind::Receipts)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "limit"
        ));

        let zero_blocks_limit = RpcRequest::empty("blocks-1", METHOD_BLOCKS)
            .with_param("limit", 0_usize)
            .expect("limit param");
        assert!(matches!(
            validate_request(
                &zero_blocks_limit,
                Some("blocks-1"),
                Some(RpcRequestKind::Blocks)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "limit"
        ));

        let oversized_blocks_limit = RpcRequest::empty("blocks-1", METHOD_BLOCKS)
            .with_param("limit", MAX_RPC_READ_QUERY_LIMIT + 1)
            .expect("limit param");
        assert!(matches!(
            validate_request(
                &oversized_blocks_limit,
                Some("blocks-1"),
                Some(RpcRequestKind::Blocks)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "limit"
        ));

        let zero_archive_limit = RpcRequest::empty("archive-1", METHOD_BATCH_ARCHIVE)
            .with_param("limit", 0_usize)
            .expect("limit param");
        assert!(matches!(
            validate_request(
                &zero_archive_limit,
                Some("archive-1"),
                Some(RpcRequestKind::BatchArchive)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "limit"
        ));

        let bad_archive_kind = RpcRequest::empty("archive-1", METHOD_BATCH_ARCHIVE)
            .with_param("batch_kind", "unsupported_batch_kind")
            .expect("batch kind param");
        assert!(matches!(
            validate_request(
                &bad_archive_kind,
                Some("archive-1"),
                Some(RpcRequestKind::BatchArchive)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "batch_kind"
        ));

        let bad_archive_batch_id = RpcRequest::empty("archive-1", METHOD_BATCH_ARCHIVE)
            .with_param("batch_id", "batch-1")
            .expect("batch id param");
        assert!(matches!(
            validate_request(
                &bad_archive_batch_id,
                Some("archive-1"),
                Some(RpcRequestKind::BatchArchive)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "batch_id"
        ));

        let missing_amount = RpcRequest::empty("mempool-submit-1", METHOD_MEMPOOL_SUBMIT_TRANSFER)
            .with_param("to", "pf1-recipient")
            .expect("to param");
        assert!(matches!(
            validate_request(
                &missing_amount,
                Some("mempool-submit-1"),
                Some(RpcRequestKind::MempoolSubmitTransfer)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "amount"
        ));

        let zero_amount = RpcRequest::empty("mempool-submit-1", METHOD_MEMPOOL_SUBMIT_TRANSFER)
            .with_param("to", "pf1-recipient")
            .expect("to param")
            .with_param("amount", 0_u64)
            .expect("amount param");
        assert!(matches!(
            validate_request(
                &zero_amount,
                Some("mempool-submit-1"),
                Some(RpcRequestKind::MempoolSubmitTransfer)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "amount"
        ));

        let missing_transfer_file = RpcRequest::empty(
            "mempool-submit-signed-1",
            METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER,
        );
        assert!(matches!(
            validate_request(
                &missing_transfer_file,
                Some("mempool-submit-signed-1"),
                Some(RpcRequestKind::MempoolSubmitSignedTransfer)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "transfer_file"
        ));

        let conflicting_signed_transfer_params = RpcRequest::empty(
            "mempool-submit-signed-1",
            METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER,
        )
        .with_param("transfer_file", "transfer.json")
        .expect("transfer file param")
        .with_param("signed_transfer_json", "{}")
        .expect("signed JSON param");
        assert!(matches!(
            validate_request(
                &conflicting_signed_transfer_params,
                Some("mempool-submit-signed-1"),
                Some(RpcRequestKind::MempoolSubmitSignedTransfer)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "params"
        ));

        let oversized_signed_transfer_json = RpcRequest::empty(
            "mempool-submit-signed-1",
            METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER,
        )
        .with_param(
            "signed_transfer_json",
            "x".repeat(MAX_RPC_SIGNED_TRANSFER_JSON_BYTES + 1),
        )
        .expect("oversized signed JSON param");
        assert!(matches!(
            validate_request(
                &oversized_signed_transfer_json,
                Some("mempool-submit-signed-1"),
                Some(RpcRequestKind::MempoolSubmitSignedTransfer)
            ),
            Err(RpcRequestValidationError::Protocol(
                RpcProtocolError::ParamStringTooLong { key, .. }
            )) if key == "signed_transfer_json"
        ));

        let oversized_signed_payment_v2_json = RpcRequest::empty(
            "mempool-submit-payment-v2-1",
            METHOD_MEMPOOL_SUBMIT_SIGNED_PAYMENT_V2,
        )
        .with_param(
            "signed_payment_v2_json",
            "x".repeat(MAX_RPC_SIGNED_TRANSFER_JSON_BYTES + 1),
        )
        .expect("oversized signed payment v2 JSON param");
        assert!(matches!(
            validate_request(
                &oversized_signed_payment_v2_json,
                Some("mempool-submit-payment-v2-1"),
                Some(RpcRequestKind::MempoolSubmitSignedPaymentV2)
            ),
            Err(RpcRequestValidationError::Protocol(
                RpcProtocolError::ParamStringTooLong { key, .. }
            )) if key == "signed_payment_v2_json"
        ));

        let fastpay_order_json = RpcRequest::empty("owned-sign-1", "owned_sign")
            .with_param("order_json", "x".repeat(MAX_RPC_PARAM_STRING_BYTES + 1))
            .expect("FastPay order JSON param");
        validate_request(&fastpay_order_json, Some("owned-sign-1"), None)
            .expect("FastPay order JSON has a larger method-specific cap");

        let compressed_fastpay_order = RpcRequest::empty("owned-sign-compressed-1", "owned_sign")
            .with_param(
                "order_json_gzip_base64",
                "x".repeat(MAX_RPC_PARAM_STRING_BYTES + 1),
            )
            .expect("compressed FastPay order param");
        validate_request(&compressed_fastpay_order, Some("owned-sign-compressed-1"), None)
            .expect("compressed FastPay order has the same larger method-specific cap");

        let oversized_fastpay_cert_json = RpcRequest::empty("owned-apply-1", "owned_apply")
            .with_param("cert_json", "x".repeat(MAX_RPC_FASTPAY_JSON_BYTES + 1))
            .expect("FastPay cert JSON param");
        assert!(matches!(
            validate_request(&oversized_fastpay_cert_json, Some("owned-apply-1"), None),
            Err(RpcRequestValidationError::Protocol(
                RpcProtocolError::ParamStringTooLong { key, .. }
            )) if key == "cert_json"
        ));

        let bad_max_transactions = RpcRequest::empty("mempool-batch-1", METHOD_MEMPOOL_BATCH)
            .with_param("batch_file", "devnet/local/batches/batch.json")
            .expect("batch file param")
            .with_param("max_transactions", "ten")
            .expect("bad max param");
        assert!(matches!(
            validate_request(
                &bad_max_transactions,
                Some("mempool-batch-1"),
                Some(RpcRequestKind::MempoolBatch)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "max_transactions"
        ));

        let zero_max_transactions = RpcRequest::empty("mempool-batch-1", METHOD_MEMPOOL_BATCH)
            .with_param("batch_file", "devnet/local/batches/batch.json")
            .expect("batch file param")
            .with_param("max_transactions", 0_usize)
            .expect("max param");
        assert!(matches!(
            validate_request(
                &zero_max_transactions,
                Some("mempool-batch-1"),
                Some(RpcRequestKind::MempoolBatch)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "max_transactions"
        ));

        let missing_shield_owner = RpcRequest::empty("shield-mint-1", METHOD_SHIELD_BATCH_MINT)
            .with_param("amount", 250_u64)
            .expect("amount param")
            .with_param("batch_file", "devnet/local/batches/shield-mint.json")
            .expect("batch file param");
        assert!(matches!(
            validate_request(
                &missing_shield_owner,
                Some("shield-mint-1"),
                Some(RpcRequestKind::ShieldBatchMint)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "owner"
        ));

        let bad_shield_amount = RpcRequest::empty("shield-spend-1", METHOD_SHIELD_BATCH_SPEND)
            .with_param("note_id", "note-1")
            .expect("note param")
            .with_param("to", "bob")
            .expect("to param")
            .with_param("amount", "125")
            .expect("amount param")
            .with_param("batch_file", "devnet/local/batches/shield-spend.json")
            .expect("batch file param");
        assert!(matches!(
            validate_request(
                &bad_shield_amount,
                Some("shield-spend-1"),
                Some(RpcRequestKind::ShieldBatchSpend)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "amount"
        ));

        let zero_shield_amount = RpcRequest::empty("shield-mint-1", METHOD_SHIELD_BATCH_MINT)
            .with_param("owner", "alice")
            .expect("owner param")
            .with_param("amount", 0_u64)
            .expect("amount param")
            .with_param("batch_file", "devnet/local/batches/shield-mint.json")
            .expect("batch file param");
        assert!(matches!(
            validate_request(
                &zero_shield_amount,
                Some("shield-mint-1"),
                Some(RpcRequestKind::ShieldBatchMint)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "amount"
        ));

        let bad_turnstile = RpcRequest::empty("shield-turnstile-1", METHOD_SHIELD_TURNSTILE)
            .with_param("owner", "alice")
            .expect("owner param");
        assert!(matches!(
            validate_request(
                &bad_turnstile,
                Some("shield-turnstile-1"),
                Some(RpcRequestKind::ShieldTurnstile)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "params"
        ));

        let missing_bridge_domain =
            RpcRequest::empty("bridge-domain-1", METHOD_BRIDGE_BATCH_DOMAIN)
                .with_param("name", "Local Simulation")
                .expect("name param")
                .with_param("inbound_cap", 100_u64)
                .expect("inbound param")
                .with_param("outbound_cap", 60_u64)
                .expect("outbound param")
                .with_param("batch_file", "devnet/local/batches/bridge-domain.json")
                .expect("batch file param");
        assert!(matches!(
            validate_request(
                &missing_bridge_domain,
                Some("bridge-domain-1"),
                Some(RpcRequestKind::BridgeBatchDomain)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "domain_id"
        ));

        let bad_witness_epoch =
            RpcRequest::empty("bridge-transfer-1", METHOD_BRIDGE_BATCH_TRANSFER)
                .with_param("domain_id", "local-sim")
                .expect("domain param")
                .with_param("direction", "inbound")
                .expect("direction param")
                .with_param("from", "external:alice")
                .expect("from param")
                .with_param("to", "pfrecipient")
                .expect("to param")
                .with_param("asset_id", "POSTFIAT")
                .expect("asset param")
                .with_param("amount", 40_u64)
                .expect("amount param")
                .with_param("witness_id", "bridge-witness-1")
                .expect("witness param")
                .with_param("witness_epoch", "two")
                .expect("witness epoch param")
                .with_param("batch_file", "devnet/local/batches/bridge-transfer.json")
                .expect("batch file param");
        assert!(matches!(
            validate_request(
                &bad_witness_epoch,
                Some("bridge-transfer-1"),
                Some(RpcRequestKind::BridgeBatchTransfer)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "witness_epoch"
        ));

        let bad_bridge_direction =
            RpcRequest::empty("bridge-transfer-1", METHOD_BRIDGE_BATCH_TRANSFER)
                .with_param("domain_id", "local-sim")
                .expect("domain param")
                .with_param("direction", "sideways")
                .expect("direction param")
                .with_param("from", "external:alice")
                .expect("from param")
                .with_param("to", "pfrecipient")
                .expect("to param")
                .with_param("asset_id", "POSTFIAT")
                .expect("asset param")
                .with_param("amount", 40_u64)
                .expect("amount param")
                .with_param("witness_id", "bridge-witness-1")
                .expect("witness param")
                .with_param("batch_file", "devnet/local/batches/bridge-transfer.json")
                .expect("batch file param");
        assert!(matches!(
            validate_request(
                &bad_bridge_direction,
                Some("bridge-transfer-1"),
                Some(RpcRequestKind::BridgeBatchTransfer)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "direction"
        ));

        let bad_bridge_status = RpcRequest::empty("bridge-status-1", METHOD_BRIDGE_STATUS)
            .with_param("domain_id", "local-sim")
            .expect("domain param");
        assert!(matches!(
            validate_request(
                &bad_bridge_status,
                Some("bridge-status-1"),
                Some(RpcRequestKind::BridgeStatus)
            ),
            Err(RpcRequestValidationError::InvalidParams { field, .. }) if field == "params"
        ));
    }

    #[test]
    fn request_validation_file_rejects_expected_id_mismatch() {
        let path = std::env::temp_dir().join(format!(
            "postfiat-rpc-sdk-request-{}-{}.json",
            std::process::id(),
            "validate-id"
        ));
        write_request_file(&path, &status_request("actual-id")).expect("write request file");
        let read_error =
            validate_request_file(&path, Some("expected-id"), Some(RpcRequestKind::Status))
                .expect_err("id mismatch");
        assert_eq!(read_error.kind(), io::ErrorKind::InvalidData);
        fs::remove_file(path).expect("remove request file");
    }

    #[test]
    fn request_protocol_validation_rejects_bad_envelopes() {
        let mut bad_version = status_request("status-1");
        bad_version.version = "postfiat-local-rpc-v0".to_string();
        assert_eq!(
            bad_version.validate_protocol().expect_err("bad version"),
            RpcProtocolError::UnsupportedVersion {
                found: "postfiat-local-rpc-v0".to_string(),
            }
        );

        let empty_id = status_request(" ");
        assert_eq!(
            empty_id.validate_protocol().expect_err("empty id"),
            RpcProtocolError::EmptyId,
        );

        let empty_method = RpcRequest::empty("empty-method", " ");
        assert_eq!(
            empty_method.validate_protocol().expect_err("empty method"),
            RpcProtocolError::EmptyMethod,
        );

        let oversized_id = status_request("i".repeat(MAX_RPC_PARAM_STRING_BYTES + 1));
        assert_eq!(
            oversized_id.validate_protocol().expect_err("oversized id"),
            RpcProtocolError::FieldTooLong {
                field: "id",
                max_bytes: MAX_RPC_PARAM_STRING_BYTES,
            }
        );

        let oversized_param_key = "p".repeat(MAX_RPC_PARAM_NAME_BYTES + 1);
        let oversized_param_name = RpcRequest::new(
            "oversized-param-name",
            METHOD_STATUS,
            Value::Object(
                [(
                    oversized_param_key.clone(),
                    Value::String("value".to_string()),
                )]
                .into_iter()
                .collect(),
            ),
        );
        assert_eq!(
            oversized_param_name
                .validate_protocol()
                .expect_err("oversized param name"),
            RpcProtocolError::ParamNameTooLong {
                key: oversized_param_key,
                max_bytes: MAX_RPC_PARAM_NAME_BYTES,
            }
        );

        let oversized_param = RpcRequest::new(
            "oversized-param",
            METHOD_ACCOUNT,
            json!({"address": "a".repeat(MAX_RPC_PARAM_STRING_BYTES + 1)}),
        );
        assert_eq!(
            oversized_param
                .validate_protocol()
                .expect_err("oversized param value"),
            RpcProtocolError::ParamStringTooLong {
                key: "address".to_string(),
                max_bytes: MAX_RPC_PARAM_STRING_BYTES,
            }
        );

        let shield_batch = RpcRequest::new(
            "shield-batch",
            "shield_batch_finality",
            json!({"batch_json": "b".repeat(MAX_RPC_PARAM_STRING_BYTES + 1)}),
        );
        shield_batch
            .validate_protocol()
            .expect("shield_batch_finality accepts a bounded inline batch above the generic string cap");

        let same_key_other_method = RpcRequest::new(
            "batch-other-method",
            METHOD_ACCOUNT,
            json!({"batch_json": "b".repeat(MAX_RPC_PARAM_STRING_BYTES + 1)}),
        );
        assert_eq!(
            same_key_other_method
                .validate_protocol()
                .expect_err("batch_json exception must be method-scoped"),
            RpcProtocolError::ParamStringTooLong {
                key: "batch_json".to_string(),
                max_bytes: MAX_RPC_PARAM_STRING_BYTES,
            }
        );

        let oversized_shield_batch = RpcRequest::new(
            "oversized-shield-batch",
            "shield_batch_finality",
            json!({"batch_json": "b".repeat(MAX_RPC_SHIELD_BATCH_JSON_BYTES + 1)}),
        );
        assert_eq!(
            oversized_shield_batch
                .validate_protocol()
                .expect_err("shield batch must retain an explicit cap"),
            RpcProtocolError::ParamStringTooLong {
                key: "batch_json".to_string(),
                max_bytes: MAX_RPC_SHIELD_BATCH_JSON_BYTES,
            }
        );

        let deposit_at_limit = shield_batch_orchard_deposit_json_request(
            "deposit-at-limit",
            "d".repeat(MAX_RPC_ORCHARD_DEPOSIT_JSON_BYTES),
        );
        deposit_at_limit
            .validate_protocol()
            .expect("deposit_json uses bounded shielded payload cap");

        let oversized_deposit = shield_batch_orchard_deposit_json_request(
            "oversized-deposit",
            "d".repeat(MAX_RPC_ORCHARD_DEPOSIT_JSON_BYTES + 1),
        );
        assert_eq!(
            oversized_deposit
                .validate_protocol()
                .expect_err("oversized deposit_json value"),
            RpcProtocolError::ParamStringTooLong {
                key: "deposit_json".to_string(),
                max_bytes: MAX_RPC_ORCHARD_DEPOSIT_JSON_BYTES,
            }
        );

        let non_object_params = RpcRequest::new("bad-params", METHOD_STATUS, json!([]));
        assert_eq!(
            non_object_params
                .validate_protocol()
                .expect_err("non-object params"),
            RpcProtocolError::ParamsNotObject,
        );

        let nested_params =
            RpcRequest::new("nested", METHOD_STATUS, json!({"filter": {"node": 0}}));
        assert_eq!(
            nested_params
                .validate_protocol()
                .expect_err("nested params"),
            RpcProtocolError::NestedParamObject("filter".to_string()),
        );
    }

    #[test]
    fn request_file_helpers_reject_invalid_protocol() {
        let path = std::env::temp_dir().join(format!(
            "postfiat-rpc-sdk-request-{}-{}.json",
            std::process::id(),
            "invalid"
        ));
        let invalid = RpcRequest::new("bad-params", METHOD_STATUS, json!([]));
        let write_error = write_request_file(&path, &invalid).expect_err("invalid write");
        assert_eq!(write_error.kind(), io::ErrorKind::InvalidInput);

        fs::write(
            &path,
            r#"{"version":"postfiat-local-rpc-v0","id":"bad","method":"status","params":{}}"#,
        )
        .expect("write invalid request");
        let read_error = read_request_file(&path).expect_err("invalid read");
        assert_eq!(read_error.kind(), io::ErrorKind::InvalidData);
        fs::write(&path, "x".repeat(MAX_RPC_REQUEST_BYTES + 1)).expect("write oversized request");
        let read_error = read_request_file(&path).expect_err("oversized read");
        assert_eq!(read_error.kind(), io::ErrorKind::InvalidData);
        fs::remove_file(path).expect("remove invalid request file");
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct StatusLike {
        state_root: String,
        block_height: u64,
    }

    #[test]
    fn response_decodes_typed_success_result() {
        let expected = StatusLike {
            state_root: "abc".to_string(),
            block_height: 42,
        };
        let response = success_response("status-1", &expected, vec![]).expect("response");
        let decoded = response.result_as::<StatusLike>().expect("typed result");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn response_decode_surfaces_rpc_error() {
        let response = error_response("bad-1", "rpc_error", "unknown rpc method", vec![]);
        let error = response
            .result_as::<serde_json::Value>()
            .expect_err("rpc error");
        assert_eq!(
            error,
            RpcResponseDecodeError::RpcError {
                code: "rpc_error".to_string(),
                message: "unknown rpc method".to_string(),
            }
        );
    }

    #[test]
    fn response_decode_rejects_missing_result() {
        let mut response =
            success_response("missing-1", &json!({"ok": true}), vec![]).expect("response");
        response.result = None;
        let error = response
            .result_as::<serde_json::Value>()
            .expect_err("missing result");
        assert_eq!(error, RpcResponseDecodeError::MissingResult);
    }

    #[test]
    fn response_validation_checks_expected_id_and_success() {
        let ok_response =
            success_response("status-1", &json!({"ok": true}), vec![]).expect("success response");
        validate_response(&ok_response, Some("status-1"), true).expect("valid response");

        assert_eq!(
            validate_response(&ok_response, Some("other-id"), true).expect_err("unexpected id"),
            RpcResponseValidationError::UnexpectedId {
                expected: "other-id".to_string(),
                found: "status-1".to_string(),
            }
        );

        let error_response = error_response("bad-1", "rpc_error", "unknown method", vec![]);
        validate_response(&error_response, Some("bad-1"), false)
            .expect("structured error response is valid when success is not required");
        assert_eq!(
            validate_response(&error_response, Some("bad-1"), true).expect_err("expected success"),
            RpcResponseValidationError::ExpectedSuccess {
                code: "rpc_error".to_string(),
                message: "unknown method".to_string(),
            }
        );
    }

    #[test]
    fn response_validation_file_rejects_expected_id_mismatch() {
        let path = std::env::temp_dir().join(format!(
            "postfiat-rpc-sdk-response-{}-{}.json",
            std::process::id(),
            "validate-id"
        ));
        let response =
            success_response("actual-id", &json!({"ok": true}), vec![]).expect("success response");
        write_response_file(&path, &response).expect("write response file");
        let read_error =
            validate_response_file(&path, Some("expected-id"), true).expect_err("id mismatch");
        assert_eq!(read_error.kind(), io::ErrorKind::InvalidData);
        fs::remove_file(path).expect("remove response file");
    }

    #[test]
    fn health_response_validation_accepts_supported_results() {
        let status = success_response(
            "status-1",
            &json!({
                "chain_id": "postfiat-local",
                "genesis_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "protocol_version": 1,
                "validator_count": 4,
                "node_id": "validator-0",
                "status": "running",
                "last_run_unix": 1,
                "state_root": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "block_height": 7,
                "block_tip_hash": "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
                "mempool_pending": 0
            }),
            vec![],
        )
        .expect("status response");
        validate_health_response(&status, RpcResponseKind::Status).expect("valid status");

        let metrics = success_response(
            "metrics-1",
            &json!({
                "schema": METRICS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "protocol_version": 1,
                "node_id": "validator-0",
                "consensus": {
                    "active_validator_count": 4,
                    "crypto_policy_version": 1,
                    "bridge_witness_epoch": 1,
                    "amendment_count": 0
                },
                "ordering": {
                    "block_height": 7,
                    "block_tip_hash": "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
                    "ordered_batch_count": 7,
                    "archived_batch_count": 7
                },
                "execution": {
                    "account_count": 1,
                    "receipt_count": 7,
                    "burned_fee_total": 3,
                    "account_reserve": 10,
                    "minimum_transfer_fee": 1,
                    "transfer_account_creation_fee": 10,
                    "transfer_fee_byte_quantum": 512,
                    "transfer_fee_per_quantum": 1,
                    "state_root": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "assets": {
                    "asset_count": 1,
                    "trustline_count": 2,
                    "holder_count": 1,
                    "total_outstanding_supply": 100,
                    "open_issued_escrow_count": 1,
                    "open_issued_escrow_amount": 25,
                    "open_issued_offer_count": 1,
                    "open_issued_offer_amount": 30,
                    "authorization_required_asset_count": 1,
                    "freeze_enabled_asset_count": 1,
                    "clawback_enabled_asset_count": 1,
                    "unauthorized_trustline_count": 1,
                    "frozen_trustline_count": 1
                },
                "mempool": {"pending": 0},
                "storage": {"replicated_state_file_count": 12},
                "shielded": {"note_count": 0, "nullifier_count": 0, "turnstile_event_count": 0},
                "bridge": {"domain_count": 0, "transfer_count": 0, "replay_cache_count": 0}
            }),
            vec![],
        )
        .expect("metrics response");
        validate_health_response(&metrics, RpcResponseKind::Metrics).expect("valid metrics");

        let state = success_response(
            "state-1",
            &json!({
                "schema": STATE_VERIFICATION_SCHEMA,
                "verified": true,
                "chain_id": "postfiat-local",
                "genesis_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "protocol_version": 1,
                "block_log": {
                    "verified": true,
                    "block_count": 7,
                    "tip_hash": "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
                    "state_root": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "governance": {
                    "verified": true,
                    "active_validator_count": 4,
                    "crypto_policy_version": 1,
                    "bridge_witness_epoch": 1,
                    "amendment_count": 1,
                    "latest_amendment_id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "bridge": {
                    "verified": true,
                    "domain_count": 1,
                    "transfer_count": 1,
                    "attestation_count": 1,
                    "replay_cache_count": 1,
                    "inbound_used": 10,
                    "outbound_used": 5,
                    "latest_transfer_id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "shielded": {
                    "verified": true,
                    "note_count": 2,
                    "nullifier_count": 1,
                    "turnstile_event_count": 1,
                    "tree_root": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                    "bootstrap_deposit_total": 100,
                    "migration_total": 50,
                    "orchard_deposit_total": 0,
                    "spent_note_count": 1,
                    "live_note_count": 1,
                    "latest_turnstile_event_id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                },
                "mempool": {
                    "verified": true,
                    "pending_count": 0,
                    "sender_count": 0,
                    "total_amount": 0,
                    "total_fee": 0,
                    "latest_tx_id": ""
                }
            }),
            vec![],
        )
        .expect("state response");
        validate_health_response(&state, RpcResponseKind::VerifyState)
            .expect("valid state verification");

        let keys = success_response(
            "keys-1",
            &json!({
                "schema": LOCAL_KEY_VALIDATION_SCHEMA,
                "node_id": "validator-0",
                "faucet_address": "pffaucet",
                "required_validator_count": 4,
                "validator_key_count": 4,
                "faucet_key_valid": true,
                "faucet_key_permissions_valid": true,
                "validator_keys_valid": true,
                "validator_key_permissions_valid": true
            }),
            vec![],
        )
        .expect("keys response");
        validate_health_response(
            &keys,
            RpcResponseKind::ValidateLocalKeys {
                validators: Some(4),
            },
        )
        .expect("valid key validation");
    }

    #[test]
    fn read_response_validation_accepts_supported_results() {
        let hex96 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let trustline_id = hex96;
        let transparent_transfer = valid_signed_transfer(hex96);
        let payment_v2 = valid_signed_payment_v2(hex96);
        let asset_transaction = valid_signed_asset_transaction(hex96);
        let pftl_destination_consume =
            valid_signed_pftl_uniswap_destination_consume(hex96);
        let offer_transaction = valid_signed_offer_transaction(hex96);

        let account = success_response(
            "account-1",
            &json!({
                "address": "pf1-test",
                "balance": 10,
                "sequence": 2,
                "public_key_hex": hex96
            }),
            vec![],
        )
        .expect("account response");
        validate_response_kind(&account, RpcResponseKind::Account).expect("valid account");

        let account_tx = success_response(
            "account-tx-1",
            &json!({
                "schema": "postfiat-account-tx-v1",
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "address": "pf1-sender",
                "from_height": 0,
                "to_height": null,
                "scan_limit": 5,
                "scanned_block_count": 1,
                "archive_lookup_count": 1,
                "truncated": false,
                "row_count": 5,
                "rows": [
                    {
                        "tx_id": hex96,
                        "block_height": 1,
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "transaction_index": 0,
                        "from": "pf1-sender",
                        "to": "pf1-recipient",
                        "amount": 25,
                        "fee": 1,
                        "sequence": 0,
                        "accepted": true,
                        "receipt_code": "accepted"
                    },
                    {
                        "tx_id": hex96,
                        "block_height": 1,
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "transaction_index": 1,
                        "transaction_kind": TRUST_SET_TRANSACTION_KIND,
                        "from": "pf1-sender",
                        "to": "pf1-recipient",
                        "amount": 0,
                        "fee": 2,
                        "sequence": 1,
                        "asset_id": hex96,
                        "issuer": "pf1-issuer",
                        "trustline_authorized": true,
                        "trustline_frozen": false,
                        "accepted": true,
                        "receipt_code": "accepted"
                    },
                    {
                        "tx_id": hex96,
                        "block_height": 1,
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "transaction_index": 2,
                        "transaction_kind": ESCROW_CREATE_TRANSACTION_KIND,
                        "from": "pf1-sender",
                        "to": "pf1-recipient",
                        "amount": 50,
                        "fee": 3,
                        "sequence": 2,
                        "asset_id": hex96,
                        "escrow_id": hex96,
                        "condition_hash": hex96,
                        "accepted": true,
                        "receipt_code": "accepted"
                    },
                    {
                        "tx_id": hex96,
                        "block_height": 1,
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "transaction_index": 3,
                        "transaction_kind": NFT_MINT_TRANSACTION_KIND,
                        "from": "pf1-sender",
                        "to": "pf1-owner",
                        "amount": 0,
                        "fee": 4,
                        "sequence": 3,
                        "issuer": "pf1-sender",
                        "nft_id": hex96,
                        "nft_collection_flags": NFT_COLLECTION_FLAG_TRANSFER_LOCKED,
                        "accepted": true,
                        "receipt_code": "accepted"
                    },
                    {
                        "tx_id": hex96,
                        "block_height": 1,
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "transaction_index": 4,
                        "transaction_kind": OFFER_CREATE_TRANSACTION_KIND,
                        "from": "pf1-sender",
                        "to": "pf1-sender",
                        "amount": 25,
                        "fee": 5,
                        "sequence": 4,
                        "asset_id": hex96,
                        "offer_id": hex96,
                        "tx_role": OFFER_TX_ROLE_TAKER,
                        "accepted": true,
                        "receipt_code": "accepted"
                    }
                ]
            }),
            vec![],
        )
        .expect("account_tx response");
        validate_response_kind(&account_tx, RpcResponseKind::AccountTx).expect("valid account_tx");

        let server_info = success_response(
            "server-info-1",
            &json!({
                "schema": SERVER_INFO_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "node_id": "validator-0",
                "status": "running",
                "ledger": {
                    "height": 1,
                    "hash": hex96,
                    "state_root": hex96
                },
                "validators": {
                    "active_count": 1,
                    "registry_update_count": 0
                },
                "fees": {
                    "minimum_transfer_fee": 1,
                    "account_reserve": 10,
                    "transfer_account_creation_fee": 10
                },
                "mempool": {"pending": 0},
                "rpc": {
                    "version": RPC_VERSION,
                    "read_aliases": ["server_info", "ledger", "tx"]
                }
            }),
            vec![],
        )
        .expect("server info response");
        validate_response_kind(&server_info, RpcResponseKind::ServerInfo)
            .expect("valid server info");

        let fee = success_response(
            "fee-1",
            &json!({
                "schema": FEE_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "minimum_transfer_fee": 1,
                "account_reserve": 10,
                "transfer_account_creation_fee": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "burned_fee_total": 0
            }),
            vec![],
        )
        .expect("fee response");
        validate_response_kind(&fee, RpcResponseKind::Fee).expect("valid fee");

        let fee_quote = success_response(
            "fee-quote-1",
            &json!({
                "schema": TRANSFER_FEE_QUOTE_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "from": "pf1-sender",
                "to": "pf1-recipient",
                "amount": 25,
                "sequence": 1,
                "sequence_source": "ledger_mempool",
                "sender_balance": 100,
                "sender_sequence": 0,
                "mempool_pending_for_sender": 0,
                "recipient_exists": false,
                "will_create_recipient_account": true,
                "base_transfer_fee": 22,
                "state_expansion_fee": 10,
                "minimum_fee": 32,
                "account_reserve": 10,
                "transfer_account_creation_fee": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "transfer_weight_bytes": 11000,
                "sender_balance_after_amount_and_fee": 43,
                "sender_meets_reserve_after_transfer": true,
                "recipient_balance_after_amount": 25,
                "recipient_meets_reserve_after_transfer": true
            }),
            vec![],
        )
        .expect("transfer fee quote response");
        validate_response_kind(&fee_quote, RpcResponseKind::TransferFeeQuote)
            .expect("valid transfer fee quote");

        let asset_fee_quote = success_response(
            "asset-fee-quote-1",
            &json!({
                "schema": ASSET_FEE_QUOTE_SCHEMA,
                "transaction_kind": ASSET_CREATE_TRANSACTION_KIND,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "source": "pf1-sender",
                "sequence": 2,
                "sequence_source": "ledger_mempool",
                "sender_balance": 100,
                "sender_sequence": 1,
                "mempool_pending_for_sender": 0,
                "base_asset_fee": 22,
                "state_expansion_fee": 20,
                "minimum_fee": 42,
                "account_reserve": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "asset_weight_bytes": 11000,
                "sender_balance_after_fee": 58,
                "sender_meets_reserve_after_fee": true,
                "operation": {
                    "operation": ASSET_CREATE_TRANSACTION_KIND,
                    "issuer": "pf1-sender",
                    "code": "USD",
                    "version": 1,
                    "precision": 2
                }
            }),
            vec![],
        )
        .expect("asset fee quote response");
        validate_response_kind(&asset_fee_quote, RpcResponseKind::AssetFeeQuote)
            .expect("valid asset fee quote");

        let clawback_fee_quote = success_response(
            "asset-clawback-fee-quote-1",
            &json!({
                "schema": ASSET_FEE_QUOTE_SCHEMA,
                "transaction_kind": ASSET_CLAWBACK_TRANSACTION_KIND,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "source": "pf1-issuer",
                "sequence": 3,
                "sequence_source": "ledger_mempool",
                "sender_balance": 100,
                "sender_sequence": 2,
                "mempool_pending_for_sender": 0,
                "base_asset_fee": 22,
                "state_expansion_fee": 0,
                "minimum_fee": 22,
                "account_reserve": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "asset_weight_bytes": 11000,
                "sender_balance_after_fee": 78,
                "sender_meets_reserve_after_fee": true,
                "operation": {
                    "operation": ASSET_CLAWBACK_TRANSACTION_KIND,
                    "owner": "pf1-holder",
                    "issuer": "pf1-issuer",
                    "asset_id": hex96,
                    "amount": 12
                }
            }),
            vec![],
        )
        .expect("asset clawback fee quote response");
        validate_response_kind(&clawback_fee_quote, RpcResponseKind::AssetFeeQuote)
            .expect("valid asset clawback fee quote");

        let offer_fee_quote = success_response(
            "offer-fee-quote-1",
            &json!({
                "schema": OFFER_FEE_QUOTE_SCHEMA,
                "transaction_kind": OFFER_CREATE_TRANSACTION_KIND,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "source": "pf1-sender",
                "sequence": 4,
                "sequence_source": "ledger_mempool",
                "sender_balance": 100,
                "sender_sequence": 3,
                "mempool_pending_for_sender": 0,
                "base_offer_fee": 22,
                "match_fee": 1,
                "state_expansion_fee": 10,
                "estimated_cross_count": 1,
                "max_dex_crosses_per_transaction": 64,
                "will_create_residual_offer": true,
                "offer_object_reserve": 10,
                "minimum_fee": 33,
                "account_reserve": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "offer_weight_bytes": 11000,
                "sender_balance_after_fee": 68,
                "sender_balance_after_fee_and_reserve": 58,
                "sender_meets_reserve_after_fee": true,
                "sender_meets_reserve_after_fee_and_reserve": true,
                "operation": {
                    "operation": OFFER_CREATE_TRANSACTION_KIND,
                    "owner": "pf1-sender",
                    "taker_gets_asset_id": "PFT",
                    "taker_gets_amount": 25,
                    "taker_pays_asset_id": hex96,
                    "taker_pays_amount": 10,
                    "expiration_height": 50
                }
            }),
            vec![],
        )
        .expect("offer fee quote response");
        validate_response_kind(&offer_fee_quote, RpcResponseKind::OfferFeeQuote)
            .expect("valid offer fee quote");

        let offer_report = json!({
            "offer_id": hex96,
            "owner": "pf1-sender",
            "owner_sequence": 4,
            "taker_gets_asset_id": "PFT",
            "taker_gets_amount_remaining": 25,
            "taker_pays_asset_id": hex96,
            "taker_pays_amount_remaining": 10,
            "original_taker_gets_amount": 25,
            "original_taker_pays_amount": 10,
            "created_height": 3,
            "expiration_height": 50,
            "reserve_paid": 10,
            "state": OFFER_STATE_OPEN
        });
        let offer_info = success_response(
            "offer-info-1",
            &json!({
                "schema": OFFER_INFO_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "offer_id": hex96,
                "found": true,
                "offer": offer_report.clone()
            }),
            vec![],
        )
        .expect("offer info response");
        validate_response_kind(&offer_info, RpcResponseKind::OfferInfo).expect("valid offer info");

        let account_offers = success_response(
            "account-offers-1",
            &json!({
                "schema": ACCOUNT_OFFERS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "account": "pf1-sender",
                "state": OFFER_STATE_OPEN,
                "limit": 10,
                "truncated": false,
                "offer_count": 1,
                "offers": [offer_report.clone()]
            }),
            vec![],
        )
        .expect("account offers response");
        validate_response_kind(&account_offers, RpcResponseKind::AccountOffers)
            .expect("valid account offers");

        let book_offers = success_response(
            "book-offers-1",
            &json!({
                "schema": BOOK_OFFERS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "taker_gets_asset_id": "PFT",
                "taker_pays_asset_id": hex96,
                "limit": 10,
                "truncated": false,
                "offer_count": 1,
                "offers": [offer_report.clone()]
            }),
            vec![],
        )
        .expect("book offers response");
        validate_response_kind(&book_offers, RpcResponseKind::BookOffers)
            .expect("valid book offers");

        let issued_asset = json!({
            "asset_id": hex96,
            "issuer": "pf1-issuer",
            "code": "USD",
            "version": 1,
            "precision": 2,
            "display_name": "US Dollar",
            "max_supply": null,
            "requires_authorization": false,
            "freeze_enabled": true,
            "clawback_enabled": false,
            "outstanding_supply": 40,
            "trustline_count": 1,
            "holder_count": 1
        });
        let asset_line = json!({
            "trustline_id": trustline_id,
            "account": "pf1-holder",
            "issuer": "pf1-issuer",
            "asset_id": hex96,
            "code": "USD",
            "version": 1,
            "precision": 2,
            "balance": 40,
            "limit": 100,
            "authorized": true,
            "frozen": false,
            "reserve_paid": 10
        });
        let asset_info = success_response(
            "asset-info-1",
            &json!({
                "schema": ASSET_INFO_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "asset_id": hex96,
                "found": true,
                "asset": issued_asset.clone()
            }),
            vec![],
        )
        .expect("asset info response");
        validate_response_kind(&asset_info, RpcResponseKind::AssetInfo).expect("valid asset info");

        let account_lines = success_response(
            "account-lines-1",
            &json!({
                "schema": ACCOUNT_LINES_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "account": "pf1-holder",
                "issuer": "pf1-issuer",
                "asset_id": hex96,
                "limit": 10,
                "truncated": false,
                "line_count": 1,
                "lines": [asset_line.clone()]
            }),
            vec![],
        )
        .expect("account lines response");
        validate_response_kind(&account_lines, RpcResponseKind::AccountLines)
            .expect("valid account lines");

        let account_assets = success_response(
            "account-assets-1",
            &json!({
                "schema": ACCOUNT_ASSETS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "account": "pf1-holder",
                "asset_id": null,
                "limit": 10,
                "truncated": false,
                "asset_count": 1,
                "assets": [asset_line.clone()]
            }),
            vec![],
        )
        .expect("account assets response");
        validate_response_kind(&account_assets, RpcResponseKind::AccountAssets)
            .expect("valid account assets");

        let issuer_assets = success_response(
            "issuer-assets-1",
            &json!({
                "schema": ISSUER_ASSETS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "issuer": "pf1-issuer",
                "limit": 10,
                "truncated": false,
                "asset_count": 1,
                "assets": [issued_asset.clone()]
            }),
            vec![],
        )
        .expect("issuer assets response");
        validate_response_kind(&issuer_assets, RpcResponseKind::IssuerAssets)
            .expect("valid issuer assets");

        let escrow_report = json!({
            "escrow_id": hex96,
            "owner": "pf1-sender",
            "owner_sequence": 2,
            "recipient": "pf1-recipient",
            "asset_id": "PFT",
            "amount": 50,
            "fee": 3,
            "condition_hash": hex96,
            "finish_after": 2,
            "cancel_after": 5,
            "state": ESCROW_STATE_OPEN,
            "created_height": 1
        });
        let escrow_info = success_response(
            "escrow-info-1",
            &json!({
                "schema": ESCROW_INFO_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "escrow_id": hex96,
                "found": true,
                "escrow": escrow_report.clone()
            }),
            vec![],
        )
        .expect("escrow info response");
        validate_response_kind(&escrow_info, RpcResponseKind::EscrowInfo)
            .expect("valid escrow info");

        let account_escrows = success_response(
            "account-escrows-1",
            &json!({
                "schema": ACCOUNT_ESCROWS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "account": "pf1-sender",
                "role": "owner",
                "state": ESCROW_STATE_OPEN,
                "limit": 10,
                "truncated": false,
                "escrow_count": 1,
                "escrows": [escrow_report.clone()]
            }),
            vec![],
        )
        .expect("account escrows response");
        validate_response_kind(&account_escrows, RpcResponseKind::AccountEscrows)
            .expect("valid account escrows");

        let nft_report = json!({
            "nft_id": hex96,
            "issuer": "pf1-issuer",
            "collection_id": "collection-1",
            "serial": 1,
            "owner": "pf1-owner",
            "metadata_hash": "ab".repeat(32),
            "metadata_uri": "ipfs://postfiat-nft",
            "flags": NFT_FLAG_TRANSFERABLE,
            "collection_flags": NFT_COLLECTION_FLAG_TRANSFER_LOCKED,
            "issuer_transfer_fee": 7,
            "transferable": true,
            "issuer_burnable": false,
            "collection_transfer_locked": true,
            "collection_burn_locked": false,
            "burned": false
        });
        let nft_info = success_response(
            "nft-info-1",
            &json!({
                "schema": NFT_INFO_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "nft_id": hex96,
                "found": true,
                "nft": nft_report.clone()
            }),
            vec![],
        )
        .expect("nft info response");
        validate_response_kind(&nft_info, RpcResponseKind::NftInfo).expect("valid nft info");

        let account_nfts = success_response(
            "account-nfts-1",
            &json!({
                "schema": ACCOUNT_NFTS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "account": "pf1-owner",
                "include_burned": false,
                "limit": 10,
                "truncated": false,
                "nft_count": 1,
                "nfts": [nft_report.clone()]
            }),
            vec![],
        )
        .expect("account nfts response");
        validate_response_kind(&account_nfts, RpcResponseKind::AccountNfts)
            .expect("valid account nfts");

        let issuer_nfts = success_response(
            "issuer-nfts-1",
            &json!({
                "schema": ISSUER_NFTS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "issuer": "pf1-issuer",
                "collection_id": "collection-1",
                "include_burned": false,
                "limit": 10,
                "truncated": false,
                "nft_count": 1,
                "nfts": [nft_report.clone()]
            }),
            vec![],
        )
        .expect("issuer nfts response");
        validate_response_kind(&issuer_nfts, RpcResponseKind::IssuerNfts)
            .expect("valid issuer nfts");

        let atomic_template = success_response(
            "atomic-template-1",
            &json!({
                "schema": ATOMIC_SETTLEMENT_TEMPLATE_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "settlement_id": hex96,
                "condition_hash": hex96,
                "condition": "shared-secret",
                "finish_after": 2,
                "cancel_after": 5,
                "left": {
                    "owner": "pf1-pft-owner",
                    "recipient": "pf1-issued-owner",
                    "asset_id": "PFT",
                    "amount": 50,
                    "sequence": 2,
                    "sequence_source": "ledger_mempool",
                    "escrow_id": hex96,
                    "transaction_kind": ESCROW_CREATE_TRANSACTION_KIND,
                    "base_escrow_fee": 22,
                    "state_expansion_fee": 10,
                    "minimum_fee": 32,
                    "escrow_weight_bytes": 11000,
                    "sender_balance": 100,
                    "sender_sequence": 1,
                    "mempool_pending_for_sender": 0,
                    "sender_balance_after_fee": 68,
                    "sender_meets_reserve_after_fee": true,
                    "operation": {
                        "operation": ESCROW_CREATE_TRANSACTION_KIND,
                        "owner": "pf1-pft-owner",
                        "recipient": "pf1-issued-owner",
                        "asset_id": "PFT",
                        "amount": 50,
                        "condition": "shared-secret",
                        "finish_after": 2,
                        "cancel_after": 5
                    }
                },
                "right": {
                    "owner": "pf1-issued-owner",
                    "recipient": "pf1-pft-owner",
                    "asset_id": hex96,
                    "amount": 25,
                    "sequence": 3,
                    "sequence_source": "explicit",
                    "escrow_id": hex96,
                    "transaction_kind": ESCROW_CREATE_TRANSACTION_KIND,
                    "base_escrow_fee": 22,
                    "state_expansion_fee": 10,
                    "minimum_fee": 32,
                    "escrow_weight_bytes": 11000,
                    "sender_balance": 100,
                    "sender_sequence": 2,
                    "mempool_pending_for_sender": 0,
                    "sender_balance_after_fee": 68,
                    "sender_meets_reserve_after_fee": true,
                    "operation": {
                        "operation": ESCROW_CREATE_TRANSACTION_KIND,
                        "owner": "pf1-issued-owner",
                        "recipient": "pf1-pft-owner",
                        "asset_id": hex96,
                        "amount": 25,
                        "condition": "shared-secret",
                        "finish_after": 2,
                        "cancel_after": 5
                    }
                }
            }),
            vec![],
        )
        .expect("atomic template response");
        validate_response_kind(&atomic_template, RpcResponseKind::AtomicSettlementTemplate)
            .expect("valid atomic settlement template");

        let receipts = success_response(
            "receipts-1",
            &json!([
                {
                    "tx_id": hex96,
                    "accepted": true,
                    "code": "accepted",
                    "message": "transfer applied",
                    "nft_issuer_transfer_fee": 7,
                    "nft_issuer_transfer_fee_recipient": "pf1-issuer"
                }
            ]),
            vec![],
        )
        .expect("receipts response");
        validate_response_kind(&receipts, RpcResponseKind::Receipts).expect("valid receipts");

        let tx = success_response(
            "tx-1",
            &json!({
                "schema": "postfiat-tx-finality-v1",
                "proof_id": hex96,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "tx_id": hex96,
                "confirmed": true,
                "receipt": {
                    "tx_id": hex96,
                    "accepted": true,
                    "code": "accepted",
                    "message": "transfer applied"
                },
                "receipt_index": 0,
                "receipt_count": 1,
                "block": {
                    "header": {
                        "height": 1,
                        "parent_hash": "genesis",
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "block_hash": hex96,
                        "state_root": hex96,
                        "receipt_count": 1,
                        "certificate_id": hex96,
                        "certificate": valid_block_certificate(hex96)
                    },
                    "receipt_ids": [hex96]
                },
                "verification_mode": "selected-block-hot-path",
                "block_log_verified": false,
                "block_count": 1,
                "tip_hash": hex96,
                "tip_state_root": hex96
            }),
            vec![],
        )
        .expect("tx response");
        validate_response_kind(&tx, RpcResponseKind::Tx).expect("valid tx finality");

        let blocks = success_response(
            "blocks-1",
            &json!([
                {
                    "header": {
                        "height": 1,
                        "parent_hash": "genesis",
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "block_hash": hex96,
                        "state_root": hex96,
                        "receipt_count": 1,
                        "certificate_id": hex96,
                        "certificate": valid_block_certificate(hex96)
                    },
                    "receipt_ids": [hex96]
                }
            ]),
            vec![],
        )
        .expect("blocks response");
        validate_response_kind(&blocks, RpcResponseKind::Blocks).expect("valid blocks");

        let ledger = success_response(
            "ledger-1",
            &json!({
                "schema": LEDGER_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "ledger_index": 1,
                "ledger_hash": hex96,
                "state_root": hex96,
                "account_count": 1,
                "receipt_count": 1,
                "burned_fee_total": 1,
                "returned_block_count": 1,
                "blocks": [
                    {
                        "header": {
                            "height": 1,
                            "parent_hash": "genesis",
                            "batch_kind": "transparent",
                            "batch_id": hex96,
                            "block_hash": hex96,
                            "state_root": hex96,
                            "receipt_count": 1,
                            "certificate_id": hex96,
                            "certificate": valid_block_certificate(hex96)
                        },
                        "receipt_ids": [hex96]
                    }
                ]
            }),
            vec![],
        )
        .expect("ledger response");
        validate_response_kind(&ledger, RpcResponseKind::Ledger).expect("valid ledger");

        let validators = success_response(
            "validators-1",
            &json!({
                "schema": VALIDATORS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "validator_count": 1,
                "registry_root": hex96,
                "source_file": "validator_registry.json",
                "validators": [{
                    "node_id": "validator-0",
                    "algorithm_id": ML_DSA_65_ALGORITHM,
                    "public_key_hex": hex96
                }]
            }),
            vec![],
        )
        .expect("validators response");
        validate_response_kind(&validators, RpcResponseKind::Validators).expect("valid validators");

        let manifests = success_response(
            "manifests-1",
            &json!({
                "schema": MANIFESTS_SCHEMA,
                "chain_id": "postfiat-local",
                "genesis_hash": hex96,
                "protocol_version": 1,
                "available": true,
                "source": "governance-genesis-bundle.json",
                "network": "controlled-testnet",
                "quorum": 1,
                "bundle_hash": hex96,
                "manifest_count": 1,
                "manifests": [{
                    "validator_id": "validator-0",
                    "manifest_file": "validator-0.operator-manifest.json",
                    "manifest_hash": hex96,
                    "hot_public_key_hex": hex96,
                    "provider_group": "provider-a",
                    "region_group": "region-a",
                    "jurisdiction_group": "jurisdiction-a",
                    "legal_domain_group": "legal-a",
                    "funding_domain_group": "funding-a"
                }]
            }),
            vec![],
        )
        .expect("manifests response");
        validate_response_kind(&manifests, RpcResponseKind::Manifests).expect("valid manifests");

        let archive = success_response(
            "archive-1",
            &json!([
                {
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "payload_hash": hex96,
                    "payload_json": valid_transparent_archive_payload(hex96)
                }
            ]),
            vec![],
        )
        .expect("archive response");
        validate_response_kind(&archive, RpcResponseKind::BatchArchive).expect("valid archive");

        let archive_window = success_response(
            "archive-window-1",
            &json!({
                "schema": HISTORY_ARCHIVE_WINDOW_SCHEMA,
                "proof": {
                    "schema": HISTORY_ARCHIVE_HANDOFF_SCHEMA,
                    "chain_id": "postfiat-local",
                    "genesis_hash": hex96,
                    "protocol_version": 1,
                    "archive_uri": "archive://window",
                    "from_height": 1,
                    "to_height": 1,
                    "block_count": 1,
                    "batch_count": 1,
                    "receipt_count": 1,
                    "first_block_hash": hex96,
                    "last_block_hash": hex96,
                    "block_range_root": hex96,
                    "batch_payload_root": hex96,
                    "receipt_root": hex96,
                    "proof_hash": hex96
                },
                "blocks": [
                    {
                        "header": {
                            "height": 1,
                            "parent_hash": "genesis",
                            "batch_kind": "transparent",
                            "batch_id": hex96,
                            "block_hash": hex96,
                            "state_root": hex96,
                            "receipt_count": 1,
                            "certificate_id": hex96,
                            "certificate": valid_block_certificate(hex96)
                        },
                        "receipt_ids": [hex96]
                    }
                ],
                "batches": [
                    {
                        "batch_kind": "transparent",
                        "batch_id": hex96,
                        "payload_hash": hex96,
                        "payload_json": valid_transparent_archive_payload(hex96)
                    }
                ],
                "receipts": [
                    {
                        "tx_id": hex96,
                        "accepted": true,
                        "code": "accepted",
                        "message": "transfer applied"
                    }
                ],
                "bundle_hash": hex96
            }),
            vec![],
        )
        .expect("archive window response");
        validate_response_kind(&archive_window, RpcResponseKind::ArchiveWindow)
            .expect("valid archive window");

        let archive_context = BatchArchiveValidationContext {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: hex96.to_string(),
            protocol_version: 1,
        };
        let archived_payload_json = valid_transparent_archive_payload(hex96);
        let archived_payload_hash = batch_archive_payload_hash(
            &archive_context,
            "transparent",
            hex96,
            &archived_payload_json,
        )
        .expect("archive payload hash");
        let hash_bound_archive = success_response(
            "archive-2",
            &json!([
                {
                    "batch_kind": "transparent",
                    "batch_id": hex96,
                    "payload_hash": archived_payload_hash,
                    "payload_json": archived_payload_json
                }
            ]),
            vec![],
        )
        .expect("archive response");
        validate_response_kind_with_context(
            &hash_bound_archive,
            RpcResponseKind::BatchArchive,
            Some(&archive_context),
        )
        .expect("valid hash-bound archive");

        let shielded_archive = success_response(
            "archive-shielded-1",
            &json!([
                {
                    "batch_kind": "shielded",
                    "batch_id": hex96,
                    "payload_hash": hex96,
                    "payload_json": json!({
                        "batch_id": hex96,
                        "actions": [
                            {
                                "kind": "shield_mint",
                                "owner": "pfshieldowner",
                                "asset_id": "POSTFIAT",
                                "amount": 42,
                                "memo": "shielded mint"
                            }
                        ]
                    }).to_string()
                }
            ]),
            vec![],
        )
        .expect("shielded archive response");
        validate_response_kind(&shielded_archive, RpcResponseKind::BatchArchive)
            .expect("valid shielded archive payload");

        let bridge_archive = success_response(
            "archive-bridge-1",
            &json!([
                {
                    "batch_kind": "bridge",
                    "batch_id": hex96,
                    "payload_hash": hex96,
                    "payload_json": json!({
                        "batch_id": hex96,
                        "actions": [
                            {
                                "kind": "bridge_domain",
                                "domain_id": "local-sim",
                                "name": "Local Simulation",
                                "source_chain": "xrpl-devnet",
                                "target_chain": "postfiat-local",
                                "bridge_id": "local-sim",
                                "door_account": "door:local-sim",
                                "inbound_cap": 100,
                                "outbound_cap": 60
                            }
                        ]
                    }).to_string()
                }
            ]),
            vec![],
        )
        .expect("bridge archive response");
        validate_response_kind(&bridge_archive, RpcResponseKind::BatchArchive)
            .expect("valid bridge archive payload");

        let governance_archive = success_response(
            "archive-governance-1",
            &json!([
                {
                    "batch_kind": "governance",
                    "batch_id": hex96,
                    "payload_hash": hex96,
                    "payload_json": json!({
                        "batch_id": hex96,
                        "amendments": [
                            {
                                "amendment_id": hex96,
                                "chain_id": "postfiat-local",
                                "genesis_hash": hex96,
                                "protocol_version": 1,
                                "instance_id": hex96,
                                "proposal_id": hex96,
                                "certificate_id": hex96,
                                "proposer": "validator-0",
                                "validators": ["validator-0"],
                                "quorum": 1,
                                "kind": "validator_set",
                                "value": 4,
                                "support": ["validator-0"],
                                "votes": [
                                    {
                                        "vote_id": hex96,
                                        "validator": "validator-0",
                                        "accept": true
                                    }
                                ]
                            }
                        ]
                    }).to_string()
                }
            ]),
            vec![],
        )
        .expect("governance archive response");
        validate_response_kind(&governance_archive, RpcResponseKind::BatchArchive)
            .expect("valid governance archive payload");

        let mempool_submit = success_response(
            "mempool-submit-1",
            &json!({
                "tx_id": hex96,
                "transfer": transparent_transfer.clone()
            }),
            vec![],
        )
        .expect("mempool submit response");
        validate_response_kind(&mempool_submit, RpcResponseKind::MempoolSubmitTransfer)
            .expect("valid mempool submit");
        validate_response_kind(
            &mempool_submit,
            RpcResponseKind::MempoolSubmitSignedTransfer,
        )
        .expect("valid signed mempool submit");
        let payment_v2_submit = success_response(
            "mempool-submit-payment-v2-1",
            &json!({
                "tx_id": hex96,
                "payment": payment_v2.clone()
            }),
            vec![],
        )
        .expect("payment v2 mempool submit response");
        validate_response_kind(
            &payment_v2_submit,
            RpcResponseKind::MempoolSubmitSignedPaymentV2,
        )
        .expect("valid payment v2 mempool submit");
        let asset_submit = success_response(
            "mempool-submit-asset-1",
            &json!({
                "tx_id": hex96,
                "transaction": asset_transaction.clone()
            }),
            vec![],
        )
        .expect("asset mempool submit response");
        validate_response_kind(
            &asset_submit,
            RpcResponseKind::MempoolSubmitSignedAssetTransaction,
        )
        .expect("valid asset mempool submit");
        let destination_consume_submit = success_response(
            "mempool-submit-pftl-destination-consume-1",
            &json!({
                "tx_id": hex96,
                "transaction": pftl_destination_consume
            }),
            vec![],
        )
        .expect("PFTL-Uniswap destination consume mempool submit response");
        validate_response_kind(
            &destination_consume_submit,
            RpcResponseKind::MempoolSubmitSignedAssetTransaction,
        )
        .expect("valid PFTL-Uniswap destination consume mempool submit");
        let offer_submit = success_response(
            "mempool-submit-offer-1",
            &json!({
                "tx_id": hex96,
                "transaction": offer_transaction.clone()
            }),
            vec![],
        )
        .expect("offer mempool submit response");
        validate_response_kind(
            &offer_submit,
            RpcResponseKind::MempoolSubmitSignedOfferTransaction,
        )
        .expect("valid offer mempool submit");

        let mempool_status = success_response(
            "mempool-status-1",
            &json!({
                "pending": [
                    {
                        "tx_id": hex96,
                        "transfer": transparent_transfer.clone()
                    }
                ],
                "pending_payment_v2": [
                    {
                        "tx_id": hex96,
                        "payment": payment_v2.clone()
                    }
                ],
                "pending_asset_transactions": [
                    {
                        "tx_id": hex96,
                        "transaction": asset_transaction.clone()
                    }
                ],
                "pending_offer_transactions": [
                    {
                        "tx_id": hex96,
                        "transaction": offer_transaction.clone()
                    }
                ]
            }),
            vec![],
        )
        .expect("mempool status response");
        validate_response_kind(&mempool_status, RpcResponseKind::MempoolStatus)
            .expect("valid mempool status");

        let mempool_batch = success_response(
            "mempool-batch-1",
            &json!({
                "batch_id": hex96,
                "transactions": [transparent_transfer],
                "payments_v2": [payment_v2],
                "asset_transactions": [asset_transaction],
                "offer_transactions": [offer_transaction]
            }),
            vec![],
        )
        .expect("mempool batch response");
        validate_response_kind(&mempool_batch, RpcResponseKind::MempoolBatch)
            .expect("valid mempool batch");

        let apply_batch = success_response(
            "apply-batch-1",
            &json!([
                {
                    "tx_id": hex96,
                    "accepted": true,
                    "code": "accepted",
                    "message": "transfer applied"
                }
            ]),
            vec![],
        )
        .expect("apply batch response");
        validate_response_kind(&apply_batch, RpcResponseKind::ApplyBatch)
            .expect("valid transparent apply receipts");

        let shield_note = json!({
            "note_id": hex96,
            "commitment": hex96,
            "position": 1,
            "owner": "pfshieldowner",
            "asset_id": "POSTFIAT",
            "value": 42,
            "rho": hex96,
            "memo": "shielded note",
            "created_by": hex96
        });

        let shield_mint = success_response(
            "shield-mint-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "shield_mint",
                        "owner": "pfshieldowner",
                        "asset_id": "POSTFIAT",
                        "amount": 42,
                        "memo": "shielded mint"
                    }
                ]
            }),
            vec![],
        )
        .expect("shield mint response");
        validate_response_kind(&shield_mint, RpcResponseKind::ShieldBatchMint)
            .expect("valid shield mint batch");

        let shield_spend = success_response(
            "shield-spend-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "shield_spend",
                        "note_id": hex96,
                        "to": "pfrecipient",
                        "amount": 10,
                        "memo": "shielded spend"
                    }
                ]
            }),
            vec![],
        )
        .expect("shield spend response");
        validate_response_kind(&shield_spend, RpcResponseKind::ShieldBatchSpend)
            .expect("valid shield spend batch");

        let shield_migrate = success_response(
            "shield-migrate-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "shield_migrate",
                        "note_id": hex96,
                        "target_pool": "debug-shielded-pool-v2",
                        "memo": "shielded migration"
                    }
                ]
            }),
            vec![],
        )
        .expect("shield migrate response");
        validate_response_kind(&shield_migrate, RpcResponseKind::ShieldBatchMigrate)
            .expect("valid shield migrate batch");

        let orchard_action_json = json!({
            "pool_id": "orchard-v1",
            "proof_system_id": "orchard-halo2",
            "circuit_id": "orchard-action-v1",
            "flags": {},
            "anchor": hex96,
            "nullifiers": [hex96],
            "randomized_verification_keys": [hex96],
            "value_commitments": [hex96],
            "output_commitments": [hex96],
            "encrypted_outputs": [],
            "value_balance": 0,
            "fee": 0,
            "external_binding_hash": hex96,
            "proof": hex96,
            "spend_authorization_signatures": [hex96],
            "binding_signature": hex96
        })
        .to_string();
        let shield_orchard = success_response(
            "shield-orchard-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "orchard_action_v1",
                        "action_json": orchard_action_json
                    }
                ]
            }),
            vec![],
        )
        .expect("shield orchard response");
        validate_response_kind(&shield_orchard, RpcResponseKind::ShieldBatchOrchard)
            .expect("valid shield orchard batch");

        let shield_orchard_deposit = success_response(
            "shield-orchard-deposit-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "orchard_deposit_v1",
                        "action_json": orchard_action_json,
                        "funding_transfer": valid_signed_transfer(hex96),
                        "amount": 25,
                        "fee": 0,
                        "policy_id": ORCHARD_DEPOSIT_POLICY_ID,
                        "disclosure_hash": ""
                    }
                ]
            }),
            vec![],
        )
        .expect("shield orchard deposit response");
        validate_response_kind(
            &shield_orchard_deposit,
            RpcResponseKind::ShieldBatchOrchardDeposit,
        )
        .expect("valid shield orchard deposit batch");

        let shield_orchard_withdraw = success_response(
            "shield-orchard-withdraw-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "orchard_withdraw_v1",
                        "action_json": orchard_action_json,
                        "to": "pffaucet",
                        "amount": 25,
                        "fee": 0,
                        "policy_id": ORCHARD_WITHDRAW_POLICY_ID,
                        "disclosure_hash": ""
                    }
                ]
            }),
            vec![],
        )
        .expect("shield orchard withdraw response");
        validate_response_kind(
            &shield_orchard_withdraw,
            RpcResponseKind::ShieldBatchOrchardWithdraw,
        )
        .expect("valid shield orchard withdraw batch");

        let apply_shield = success_response(
            "apply-shield-1",
            &json!([
                {
                    "tx_id": hex96,
                    "accepted": true,
                    "code": "accepted",
                    "message": "shielded action applied"
                }
            ]),
            vec![],
        )
        .expect("apply shield response");
        validate_response_kind(&apply_shield, RpcResponseKind::ApplyShieldBatch)
            .expect("valid shield apply receipts");

        let shield_scan = success_response("shield-scan-1", &json!([shield_note.clone()]), vec![])
            .expect("shield scan response");
        validate_response_kind(&shield_scan, RpcResponseKind::ShieldScan)
            .expect("valid shield scan");

        let shield_disclose = success_response(
            "shield-disclose-1",
            &json!({
                "note": shield_note,
                "nullifier": hex96,
                "spent": true
            }),
            vec![],
        )
        .expect("shield disclosure response");
        validate_response_kind(&shield_disclose, RpcResponseKind::ShieldDisclose)
            .expect("valid shield disclosure");

        let shield_turnstile = success_response(
            "shield-turnstile-1",
            &json!({
                "event_count": 1,
                "bootstrap_deposit_total": 42,
                "migration_total": 0,
                "orchard_deposit_total": 0,
                "events": [
                    {
                        "event_id": hex96,
                        "kind": "bootstrap_deposit",
                        "owner": "pfshieldowner",
                        "asset_id": "POSTFIAT",
                        "amount": 42,
                        "note_id": hex96,
                        "source_pool": "transparent-bootstrap",
                        "target_pool": "debug-shielded-pool-v1"
                    }
                ]
            }),
            vec![],
        )
        .expect("shield turnstile response");
        validate_response_kind(&shield_turnstile, RpcResponseKind::ShieldTurnstile)
            .expect("valid shield turnstile");

        let bridge_status = success_response(
            "bridge-status-1",
            &json!({
                "domains": [
                    {
                        "domain_id": "local-sim",
                        "name": "Local Simulation",
                        "source_chain": "xrpl-devnet",
                        "target_chain": "postfiat-local",
                        "bridge_id": "local-sim",
                        "door_account": "door:local-sim",
                        "inbound_cap": 100,
                        "outbound_cap": 60,
                        "inbound_used": 40,
                        "outbound_used": 25,
                        "paused": false
                    }
                ],
                "transfers": [
                    {
                        "transfer_id": hex96,
                        "domain_id": "local-sim",
                        "source_chain": "xrpl-devnet",
                        "target_chain": "postfiat-local",
                        "bridge_id": "local-sim",
                        "door_account": "door:local-sim",
                        "direction": "inbound",
                        "from": "external:alice",
                        "to": "pfrecipient",
                        "asset_id": "POSTFIAT",
                        "amount": 40,
                        "witness_id": "witness-1",
                        "witness_epoch": 1,
                        "sequence": 1,
                        "witness_attestation": {
                            "attestation_id": hex96,
                            "chain_id": "postfiat-local",
                            "genesis_hash": hex96,
                            "protocol_version": 1,
                            "signer": "validator-0",
                            "algorithm_id": "ML-DSA-65",
                            "public_key_hex": hex96,
                            "signature_hex": hex96
                        }
                    }
                ],
                "replay_cache": ["local-sim:1:witness-1"]
            }),
            vec![],
        )
        .expect("bridge status response");
        validate_response_kind(&bridge_status, RpcResponseKind::BridgeStatus)
            .expect("valid bridge status");

        let hex64 = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";
        let navcoin_routes = success_response(
            "navcoin-bridge-routes-1",
            &json!({
                "schema": "postfiat-pftl-uniswap-routes-status-v1",
                "route_count": 1,
                "routes": [
                    {
                        "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                        "route_family": "primary_pftl_mint",
                        "route_config_digest": hex96,
                        "route_trust_class": "CONTROLLED",
                        "route_live": true,
                        "paused": false,
                        "native_nav_asset_id": hex96,
                        "settlement_asset_id": hex96,
                        "wrapped_navcoin_token": "0x1111111111111111111111111111111111111111",
                        "handoff_controller": "0x2222222222222222222222222222222222222222",
                        "settlement_adapter": "0x3333333333333333333333333333333333333333",
                        "ethereum_chain_id": 1,
                        "latest_finalized_nav_epoch": 7,
                        "route_supply_cap_atoms": 1000,
                        "packet_notional_cap_atoms": 500,
                        "authorized_valid_supply_atoms": 300,
                        "supply_cap_remaining_atoms": 700,
                        "outstanding_bridge_claims_atoms": 100,
                        "pending_return_import_claims_atoms": 30,
                        "primary_subscription_count": 1,
                        "export_packet_count": 2,
                        "outstanding_export_packet_count": 1,
                        "consumed_export_packet_count": 1,
                        "refunded_export_packet_count": 0,
                        "return_burn_count": 1,
                        "pending_return_burn_count": 1,
                        "imported_return_burn_count": 0,
                        "ledger_hash": hex96
                    }
                ]
            }),
            vec![],
        )
        .expect("NAVCoin bridge routes response");
        validate_response_kind(&navcoin_routes, RpcResponseKind::NavcoinBridgeRoutes)
            .expect("valid NAVCoin bridge routes");

        let navcoin_packet = success_response(
            "navcoin-bridge-packet-1",
            &json!({
                "schema": "postfiat-pftl-uniswap-packet-status-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "packet_hash": hex96,
                "packet": {
                    "packet_hash": hex96,
                    "nonce": hex64,
                    "source_wallet": "pf124071fd53a12ca4556b7aa1f5ec98b585e73468",
                    "ethereum_recipient": "0x6666666666666666666666666666666666666666",
                    "amount_atoms": 100,
                    "source_height": 20,
                    "destination_deadline_seconds": 1924992000,
                    "refund_not_before_height": 30,
                    "status": "SourceDebited",
                    "claim_class": "outstanding_bridge_claim"
                },
                "ledger_hash": hex96
            }),
            vec![],
        )
        .expect("NAVCoin bridge packet response");
        validate_response_kind(&navcoin_packet, RpcResponseKind::NavcoinBridgePacket)
            .expect("valid NAVCoin bridge packet");

        let navcoin_claims = success_response(
            "navcoin-bridge-claims-1",
            &json!({
                "schema": "postfiat-pftl-uniswap-claims-status-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "ledger_hash": hex96,
                "limit": 2,
                "truncated": false,
                "outstanding_bridge_claims_atoms": 100,
                "pending_return_import_claims_atoms": 30,
                "export_claim_count": 1,
                "return_claim_count": 1,
                "exports": [
                    {
                        "packet_hash": hex96,
                        "nonce": hex64,
                        "source_wallet": "pf124071fd53a12ca4556b7aa1f5ec98b585e73468",
                        "ethereum_recipient": "0x6666666666666666666666666666666666666666",
                        "amount_atoms": 100,
                        "source_height": 20,
                        "destination_deadline_seconds": 1924992000,
                        "refund_not_before_height": 30,
                        "status": "SourceDebited",
                        "claim_class": "outstanding_bridge_claim"
                    }
                ],
                "returns": [
                    {
                        "burn_event_hash": hex64,
                        "ethereum_chain_id": 1,
                        "bridge_controller": "0x2222222222222222222222222222222222222222",
                        "wrapped_navcoin_token": "0x1111111111111111111111111111111111111111",
                        "native_nav_asset_id": hex96,
                        "ethereum_sender": "0x6666666666666666666666666666666666666666",
                        "pftl_recipient": "pf124071fd53a12ca4556b7aa1f5ec98b585e73468",
                        "amount_atoms": 30,
                        "return_nonce": hex64,
                        "burn_height": 1000,
                        "finalized_height": 1064,
                        "status": "BurnObserved",
                        "claim_class": "pending_return_import_claim"
                    }
                ]
            }),
            vec![],
        )
        .expect("NAVCoin bridge claims response");
        validate_response_kind(&navcoin_claims, RpcResponseKind::NavcoinBridgeClaims)
            .expect("valid NAVCoin bridge claims");

        let navcoin_supply = success_response(
            "navcoin-bridge-supply-1",
            &json!({
                "schema": "postfiat-pftl-uniswap-supply-status-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "native_nav_asset_id": hex96,
                "settlement_asset_id": hex96,
                "wrapped_navcoin_token": "0x1111111111111111111111111111111111111111",
                "authorized_valid_supply_atoms": 300,
                "pftl_spendable_supply_atoms": 150,
                "native_spendable_balances": [
                    {
                        "wallet": "pfsourcewallet",
                        "amount_atoms": 150
                    }
                ],
                "native_spendable_balance_count": 1,
                "native_spendable_balance_limit": 512,
                "native_spendable_balances_truncated": false,
                "native_spendable_balance_sum_atoms": 150,
                "ethereum_spendable_supply_atoms": 20,
                "other_registered_venue_supply_atoms": 0,
                "outstanding_bridge_claims_atoms": 100,
                "pending_return_import_claims_atoms": 30,
                "live_supply_sum_atoms": 300,
                "route_supply_cap_atoms": 1000,
                "supply_cap_remaining_atoms": 700,
                "packet_notional_cap_atoms": 500,
                "settlement_reserve_atoms": 300000,
                "invariant_holds": true,
                "ledger_hash": hex96
            }),
            vec![],
        )
        .expect("NAVCoin bridge supply response");
        validate_response_kind(
            &navcoin_supply,
            RpcResponseKind::NavcoinBridgeSupplyStatus,
        )
        .expect("valid NAVCoin bridge supply");

        let navcoin_truncated_supply = success_response(
            "navcoin-bridge-supply-truncated-1",
            &json!({
                "schema": "postfiat-pftl-uniswap-supply-status-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "native_nav_asset_id": hex96,
                "settlement_asset_id": hex96,
                "wrapped_navcoin_token": "0x1111111111111111111111111111111111111111",
                "authorized_valid_supply_atoms": 325,
                "pftl_spendable_supply_atoms": 175,
                "native_spendable_balances": [
                    {
                        "wallet": "pfsourcewallet-001",
                        "amount_atoms": 100
                    },
                    {
                        "wallet": "pfsourcewallet-002",
                        "amount_atoms": 50
                    }
                ],
                "native_spendable_balance_count": 3,
                "native_spendable_balance_limit": 2,
                "native_spendable_balances_truncated": true,
                "native_spendable_balance_sum_atoms": 175,
                "ethereum_spendable_supply_atoms": 20,
                "other_registered_venue_supply_atoms": 0,
                "outstanding_bridge_claims_atoms": 100,
                "pending_return_import_claims_atoms": 30,
                "live_supply_sum_atoms": 325,
                "route_supply_cap_atoms": 1000,
                "supply_cap_remaining_atoms": 675,
                "packet_notional_cap_atoms": 500,
                "settlement_reserve_atoms": 300000,
                "invariant_holds": true,
                "ledger_hash": hex96
            }),
            vec![],
        )
        .expect("truncated NAVCoin bridge supply response");
        validate_response_kind(
            &navcoin_truncated_supply,
            RpcResponseKind::NavcoinBridgeSupplyStatus,
        )
        .expect("valid truncated NAVCoin bridge supply");

        let navcoin_receipt_replay = success_response(
            "navcoin-bridge-replay-1",
            &json!({
                "schema": "postfiat-navcoin-bridge-receipt-replay-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "initial_ledger_hash": hex96,
                "final_ledger_hash": hex96,
                "receipt_root": hex96,
                "receipt_count": 1,
                "ledger_file": "pftl_uniswap_bridge_ledgers.json",
                "receipt_file": "pftl_uniswap_bridge_receipts.json",
                "status": "verified",
                "replay": {
                    "schema": "postfiat-pftl-uniswap-receipt-replay-report-v1",
                    "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                    "initial_ledger_hash": hex96,
                    "final_ledger_hash": hex96,
                    "receipt_root": hex96,
                    "receipt_count": 1
                }
            }),
            vec![],
        )
        .expect("NAVCoin bridge receipt replay response");
        validate_response_kind(
            &navcoin_receipt_replay,
            RpcResponseKind::NavcoinBridgeReceiptReplay,
        )
        .expect("valid NAVCoin bridge receipt replay");

        let navcoin_empty_replay = success_response(
            "navcoin-bridge-replay-empty-1",
            &json!({
                "schema": "postfiat-navcoin-bridge-receipt-replay-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "initial_ledger_hash": hex96,
                "final_ledger_hash": hex96,
                "receipt_root": null,
                "receipt_count": 0,
                "ledger_file": "pftl_uniswap_bridge_ledgers.json",
                "receipt_file": "pftl_uniswap_bridge_receipts.json",
                "status": "empty_clean",
                "replay": null
            }),
            vec![],
        )
        .expect("NAVCoin bridge empty receipt replay response");
        validate_response_kind(
            &navcoin_empty_replay,
            RpcResponseKind::NavcoinBridgeReceiptReplay,
        )
        .expect("valid empty NAVCoin bridge receipt replay");

        let navcoin_preflight = success_response(
            "navcoin-bridge-packet-preflight-1",
            &json!({
                "schema": "postfiat-navcoin-bridge-packet-preflight-v1",
                "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
                "route_config_digest": hex96,
                "launch_config_digest": hex96,
                "packet_digest": hex96,
                "ledger_hash": hex96,
                "packet_file": "devnet/local/pftl-uniswap/packet.json",
                "status": "ready"
            }),
            vec![],
        )
        .expect("NAVCoin bridge packet preflight response");
        validate_response_kind(
            &navcoin_preflight,
            RpcResponseKind::NavcoinBridgePacketPreflight,
        )
        .expect("valid NAVCoin bridge packet preflight");

        let bridge_domain = success_response(
            "bridge-domain-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "bridge_domain",
                        "domain_id": "local-sim",
                        "name": "Local Simulation",
                        "source_chain": "xrpl-devnet",
                        "target_chain": "postfiat-local",
                        "bridge_id": "local-sim",
                        "door_account": "door:local-sim",
                        "inbound_cap": 100,
                        "outbound_cap": 60
                    }
                ]
            }),
            vec![],
        )
        .expect("bridge domain response");
        validate_response_kind(&bridge_domain, RpcResponseKind::BridgeBatchDomain)
            .expect("valid bridge domain batch");

        let bridge_transfer = success_response(
            "bridge-transfer-1",
            &json!({
                "batch_id": hex96,
                "actions": [
                    {
                        "kind": "bridge_transfer",
                        "domain_id": "local-sim",
                        "direction": "outbound",
                        "from": "pfholder",
                        "to": "external:bob",
                        "asset_id": "POSTFIAT",
                        "amount": 25,
                        "witness_id": "witness-2",
                        "witness_epoch": 1,
                        "witness_attestation": {
                            "attestation_id": hex96,
                            "chain_id": "postfiat-local",
                            "genesis_hash": hex96,
                            "protocol_version": 1,
                            "signer": "validator-0",
                            "algorithm_id": "ML-DSA-65",
                            "public_key_hex": hex96,
                            "signature_hex": hex96
                        }
                    }
                ]
            }),
            vec![],
        )
        .expect("bridge transfer response");
        validate_response_kind(&bridge_transfer, RpcResponseKind::BridgeBatchTransfer)
            .expect("valid bridge transfer batch");

        let bridge_pause = success_response(
            "bridge-pause-1",
            &json!({
                "batch_id": hex96,
                "actions": [{"kind": "bridge_pause", "domain_id": "local-sim", "paused": true}]
            }),
            vec![],
        )
        .expect("bridge pause response");
        validate_response_kind(&bridge_pause, RpcResponseKind::BridgeBatchPause)
            .expect("valid bridge pause batch");

        let bridge_resume = success_response(
            "bridge-resume-1",
            &json!({
                "batch_id": hex96,
                "actions": [{"kind": "bridge_pause", "domain_id": "local-sim", "paused": false}]
            }),
            vec![],
        )
        .expect("bridge resume response");
        validate_response_kind(&bridge_resume, RpcResponseKind::BridgeBatchResume)
            .expect("valid bridge resume batch");

        let apply_bridge = success_response(
            "apply-bridge-1",
            &json!([
                {
                    "tx_id": hex96,
                    "accepted": false,
                    "code": "duplicate_witness",
                    "message": "witness already processed"
                }
            ]),
            vec![],
        )
        .expect("apply bridge response");
        validate_response_kind(&apply_bridge, RpcResponseKind::ApplyBridgeBatch)
            .expect("valid bridge apply receipts");

        let apply_bridge_domain = success_response(
            "apply-bridge-domain-1",
            &json!([
                {
                    "tx_id": "local-sim",
                    "accepted": true,
                    "code": "accepted",
                    "message": "bridge domain action applied"
                }
            ]),
            vec![],
        )
        .expect("apply bridge domain response");
        validate_response_kind(&apply_bridge_domain, RpcResponseKind::ApplyBridgeBatch)
            .expect("valid semantic bridge receipt id");
    }
